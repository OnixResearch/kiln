//! Durable ordered GitHub stack publication and integration recovery (#89).
//!
//! Stack plans, publications, integrations and effect reservations are stored
//! under dedicated `implementation_state` SQLite keys so `attempts` and
//! `PublicationRecord` retain exact `#[serde(deny_unknown_fields)]` backwards
//! compatibility with earlier binaries.

use kiln_core::implementation::authority::ProtectedOperation;
use kiln_core::implementation::{
    AttemptLedger, ImplementationWork, LayerVerification, StackIntegration, StackPlan,
    StackPublication, WorkLedger, admit_stack_integration, admit_stack_plan,
    admit_stack_publication, stack_identity,
};
use kiln_core::types::ContentId;
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

use crate::publication_effects::EffectDispatch;
use crate::{ATTEMPT_KIND, Implementer, WORK_KIND, diagnostic, load, save, text};

const PLAN_PREFIX: &str = "stack-plan:";
const PUBLICATION_PREFIX: &str = "stack-publication:";
const INTEGRATION_PREFIX: &str = "stack-integration:";
const EFFECT_PREFIX: &str = "stack-effect:";

/// JSON wrapper consumed by `kiln-implementer` stack subcommands (`--stack-file`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StackPublicationInput {
    pub plan: StackPlan,
    pub layer_verifications: Vec<LayerVerification>,
}

/// Admitted stack plan and lower-layer verifications retained for an attempt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetainedStackPlan {
    pub stack: ContentId,
    pub plan: StackPlan,
    pub layer_verifications: Vec<LayerVerification>,
}

/// Which durable stack effect is being reserved or read.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StackEffectKind {
    Publish,
    Integration,
}

impl StackEffectKind {
    fn key(self) -> &'static str {
        match self {
            Self::Publish => "publish",
            Self::Integration => "integration",
        }
    }
}

/// Result retained when a stack effect reservation is acknowledged.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StackEffectResult {
    Publish(StackPublication),
    Integration(StackIntegration),
}

/// Durable reservation for one stack publication or integration recovery effect.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StackEffectReservation {
    pub operation: ContentId,
    pub work: ContentId,
    pub revision: u64,
    pub attempt: ContentId,
    pub fence: u64,
    pub stack: ContentId,
    pub kind: StackEffectKind,
    pub plan: StackPlan,
    /// `None` covers crash, transport error, or unresolved remote state.
    pub observed: Option<StackEffectResult>,
}

/// Complete durable stack state retained for one attempt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StackPublicationRecord {
    pub stack: ContentId,
    pub plan: StackPlan,
    pub layer_verifications: Vec<LayerVerification>,
    pub publication: Option<StackPublication>,
    pub integration: Option<StackIntegration>,
}

impl Implementer {
    /// Admit and durably persist an ordered stack plan for one verified attempt.
    ///
    /// Refuses conflicts with an already-prepared single-PR publication or a
    /// different retained stack plan; repeating the exact same plan is idempotent.
    ///
    /// # Errors
    /// Returns a diagnostic or redacted authority denial on invalid binding,
    /// stale fence, missing authority, or conflicting retained state.
    pub fn prepare_stack_publication(
        &mut self,
        work: &ImplementationWork,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        layer_verifications: &[LayerVerification],
        plan: &StackPlan,
    ) -> Result<ContentId, String> {
        let facts = self.authority_facts(identity, fence)?;
        let handoff = facts
            .attempt
            .handoff
            .as_ref()
            .ok_or("stack publication requires a retained candidate handoff")?;
        let verification = facts
            .attempt
            .verification
            .as_ref()
            .ok_or("stack publication requires retained verification")?;
        let stack = admit_stack_plan(
            &facts.work,
            &facts.attempt,
            handoff,
            verification,
            layer_verifications,
            plan,
        )
        .map_err(diagnostic)?;
        let top_candidate = plan
            .layers
            .last()
            .map(|layer| &layer.candidate)
            .ok_or("stack plan requires at least two layers")?;
        let expected = format!("refs/heads/{}@absent", plan.trunk_branch);
        let request = facts.request(
            ProtectedOperation::PublicationPublish,
            stack.as_str(),
            &expected,
            &[
                stack.as_str(),
                plan.handoff.as_str(),
                plan.verification.as_str(),
                plan.repository.as_str(),
                plan.trunk_branch.as_str(),
            ],
            Some(top_candidate),
        )?;
        let currentness = self.attempt_currentness(identity, fence, now_ms);
        self.authorize_and_revalidate(&request, currentness, now_ms)?;

        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let (ledger, _) = load::<WorkLedger>(&transaction, WORK_KIND)?;
        let (attempts, _) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
        current_work(&ledger, work)?;
        let attempt = attempts
            .authorize_publication(identity, fence)
            .map_err(diagnostic)?;
        let work_identity = work.identity().map_err(diagnostic)?;
        if attempt.work_identity != work_identity || attempt.work_revision != work.revision {
            return Err("stack publication work does not own the exact verified attempt".into());
        }
        if attempt.publication.is_some() {
            return Err(
                "stack publication conflicts with already-prepared single-PR publication".into(),
            );
        }
        let handoff = attempt
            .handoff
            .as_ref()
            .ok_or("stack publication requires a retained candidate handoff")?;
        let verification = attempt
            .verification
            .as_ref()
            .ok_or("stack publication requires retained verification")?;
        let stack = admit_stack_plan(
            work,
            attempt,
            handoff,
            verification,
            layer_verifications,
            plan,
        )
        .map_err(diagnostic)?;
        let key = plan_key(identity);
        let (existing, version) = load_optional::<RetainedStackPlan>(&transaction, &key)?;
        if let Some(retained) = existing {
            if retained.stack != stack
                || retained.plan != *plan
                || retained.layer_verifications != layer_verifications
            {
                return Err(
                    "stack publication conflicts with a different retained stack plan".into(),
                );
            }
            transaction.commit().map_err(text)?;
            return Ok(stack);
        }
        let retained = RetainedStackPlan {
            stack: stack.clone(),
            plan: plan.clone(),
            layer_verifications: layer_verifications.to_vec(),
        };
        save(&transaction, &key, version, &retained)?;
        transaction.commit().map_err(text)?;
        Ok(stack)
    }

    /// Verify publication authority, current work, and attempt fence before
    /// invoking a read-only route-capability probe, without reserving any
    /// publication effect.
    ///
    /// # Errors
    /// Returns a diagnostic, redacted authority denial, or probe failure.
    pub fn probe_publication_route(
        &mut self,
        work_identity: &ContentId,
        identity: &ContentId,
        fence: u64,
        repository: &str,
        probe: impl FnOnce() -> Result<bool, String>,
    ) -> Result<bool, String> {
        let now_ms = self.now_ms();
        let candidate = {
            let transaction = self.connection.transaction().map_err(text)?;
            let (ledger, _) = load::<WorkLedger>(&transaction, WORK_KIND)?;
            let (attempts, _) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
            let Some(work) = ledger.current().find(|work| {
                work.identity()
                    .is_ok_and(|current| current == *work_identity)
            }) else {
                return Err("publication work identity is not current".into());
            };
            let attempt = attempts
                .authorize_publication(identity, fence)
                .map_err(diagnostic)?;
            if attempt.work_identity != *work_identity
                || attempt.work_revision != work.revision
                || work.repository != repository
            {
                return Err(
                    "publication route probe does not bind the current verified attempt".into(),
                );
            }
            let (retained_stack, _) =
                load_optional::<RetainedStackPlan>(&transaction, &plan_key(identity))?;
            let candidate = if let Some(retained) = retained_stack {
                if retained.plan.work_identity != *work_identity
                    || retained.plan.work_revision != work.revision
                    || retained.plan.repository != repository
                {
                    return Err("publication route probe conflicts with retained stack plan".into());
                }
                retained
                    .plan
                    .layers
                    .last()
                    .map(|layer| layer.candidate.clone())
                    .ok_or("stack plan requires at least two layers")?
            } else if let Some(publication) = &attempt.publication {
                if publication.plan.work_identity != *work_identity
                    || publication.plan.work_revision != work.revision
                    || publication.plan.repository != repository
                {
                    return Err(
                        "publication route probe conflicts with retained publication plan".into(),
                    );
                }
                publication.plan.candidate.clone()
            } else {
                return Err("publication is not prepared".into());
            };
            transaction.commit().map_err(text)?;
            candidate
        };

        let facts = self.authority_facts(identity, fence)?;
        let effect = format!("{}:route-probe", identity.as_str());
        let request = facts.request(
            ProtectedOperation::PublicationPublish,
            &effect,
            repository,
            &["route-probe", repository],
            Some(&candidate),
        )?;
        let currentness = self.attempt_currentness(identity, fence, now_ms);
        let session = self.admit_authority(&request, currentness, now_ms)?;
        self.revalidate_authority(session, &request, currentness, now_ms)?;
        probe()
    }

    /// Reserve and execute or reconcile an ordered stack publication effect.
    ///
    /// A recovered reservation invokes `transport` with
    /// [`EffectDispatch::ReconcileOnly`], and an already acknowledged
    /// reservation returns immediately without invoking `transport`.
    ///
    /// # Errors
    /// Returns a diagnostic, authority denial, or transport error while keeping
    /// any pending reservation durable for reconciliation.
    pub fn consume_stack_publication_effect(
        &mut self,
        work: &ImplementationWork,
        identity: &ContentId,
        fence: u64,
        plan: &StackPlan,
        transport: impl FnOnce(EffectDispatch, &StackPlan) -> Result<Option<StackPublication>, String>,
    ) -> Result<StackEffectReservation, String> {
        let (reservation, dispatch) =
            self.reserve_stack_publication_effect(work, identity, fence, plan)?;
        if reservation.observed.is_some() {
            return Ok(reservation);
        }
        let now_ms = self.now_ms();
        let currentness = self.attempt_currentness(identity, fence, now_ms);
        self.revalidate_last_authority(currentness, now_ms)?;
        match transport(dispatch, &reservation.plan)? {
            Some(publication) => {
                self.acknowledge_stack_publication_effect(work, &reservation, publication)
            }
            None => Ok(reservation),
        }
    }

    fn reserve_stack_publication_effect(
        &mut self,
        work: &ImplementationWork,
        identity: &ContentId,
        fence: u64,
        plan: &StackPlan,
    ) -> Result<(StackEffectReservation, EffectDispatch), String> {
        let now_ms = self.now_ms();
        let facts = self.authority_facts(identity, fence)?;
        let stack = stack_identity(plan).map_err(diagnostic)?;
        let top_candidate = plan
            .layers
            .last()
            .map(|layer| &layer.candidate)
            .ok_or("stack plan requires at least two layers")?;
        let effect = format!("{}:publish", stack.as_str());
        let expected = format!("refs/heads/{}", plan.trunk_branch);
        let request = facts.request(
            ProtectedOperation::PublicationPublish,
            &effect,
            &expected,
            &[
                stack.as_str(),
                plan.handoff.as_str(),
                plan.verification.as_str(),
                plan.repository.as_str(),
                plan.trunk_branch.as_str(),
            ],
            Some(top_candidate),
        )?;
        let currentness = self.attempt_currentness(identity, fence, now_ms);
        let session = self.admit_authority(&request, currentness, now_ms)?;
        self.revalidate_authority(session, &request, currentness, now_ms)?;

        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let (ledger, _) = load::<WorkLedger>(&transaction, WORK_KIND)?;
        let (attempts, _) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
        current_work(&ledger, work)?;
        let attempt = attempts
            .authorize_publication(identity, fence)
            .map_err(diagnostic)?;
        let work_identity = work.identity().map_err(diagnostic)?;
        if attempt.work_identity != work_identity || attempt.work_revision != work.revision {
            return Err("stack publication work does not own the exact verified attempt".into());
        }
        if attempt.publication.is_some() {
            return Err(
                "stack publication conflicts with already-prepared single-PR publication".into(),
            );
        }
        let (retained, _) = load_optional::<RetainedStackPlan>(&transaction, &plan_key(identity))?;
        let retained = retained.ok_or("stack publication plan is not prepared")?;
        if retained.stack != stack || retained.plan != *plan {
            return Err("stack plan differs from the prepared stack publication".into());
        }
        let key = effect_key(identity, StackEffectKind::Publish);
        let (existing, version) = load_optional::<StackEffectReservation>(&transaction, &key)?;
        if let Some(existing) = existing {
            if existing.work != work_identity
                || existing.revision != work.revision
                || existing.attempt != *identity
                || existing.fence != fence
                || existing.stack != stack
                || existing.kind != StackEffectKind::Publish
                || existing.plan != *plan
            {
                return Err(
                    "stack publication reservation conflicts with retained exact operation".into(),
                );
            }
            transaction.commit().map_err(text)?;
            return Ok((existing, EffectDispatch::ReconcileOnly));
        }
        let mut reservation = StackEffectReservation {
            operation: work_identity.clone(),
            work: work_identity,
            revision: work.revision,
            attempt: identity.clone(),
            fence,
            stack,
            kind: StackEffectKind::Publish,
            plan: plan.clone(),
            observed: None,
        };
        reservation.operation = operation_identity(&reservation)?;
        save(&transaction, &key, version, &reservation)?;
        transaction.commit().map_err(text)?;
        Ok((reservation, EffectDispatch::FirstDispatch))
    }

    fn acknowledge_stack_publication_effect(
        &mut self,
        work: &ImplementationWork,
        reservation: &StackEffectReservation,
        publication: StackPublication,
    ) -> Result<StackEffectReservation, String> {
        let now_ms = self.now_ms();
        let facts = self.authority_facts(&reservation.attempt, reservation.fence)?;
        let top_candidate = reservation
            .plan
            .layers
            .last()
            .map(|layer| &layer.candidate)
            .ok_or("stack plan requires at least two layers")?;
        let request = facts.request(
            ProtectedOperation::PublicationPublish,
            reservation.operation.as_str(),
            reservation.stack.as_str(),
            &[
                "publish",
                reservation.stack.as_str(),
                publication.provider_stack.native_id.as_str(),
            ],
            Some(top_candidate),
        )?;
        let currentness = self.attempt_currentness(&reservation.attempt, reservation.fence, now_ms);
        let session = self.admit_authority(&request, currentness, now_ms)?;
        self.revalidate_authority(session, &request, currentness, now_ms)?;

        admit_stack_publication(&reservation.plan, &publication).map_err(diagnostic)?;

        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let (ledger, _) = load::<WorkLedger>(&transaction, WORK_KIND)?;
        let (attempts, _) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
        current_work(&ledger, work)?;
        let attempt = attempts
            .authorize_publication(&reservation.attempt, reservation.fence)
            .map_err(diagnostic)?;
        let work_identity = work.identity().map_err(diagnostic)?;
        if attempt.work_identity != work_identity || attempt.work_revision != work.revision {
            return Err("stack publication acknowledgement belongs to different work".into());
        }
        let key = effect_key(&reservation.attempt, StackEffectKind::Publish);
        let (retained, version) = load_optional::<StackEffectReservation>(&transaction, &key)?;
        let mut retained = retained.ok_or("stack publication effect was not reserved")?;
        if retained.operation != reservation.operation
            || retained.work != work_identity
            || retained.revision != work.revision
            || retained.fence != reservation.fence
            || retained.stack != reservation.stack
        {
            return Err(
                "stack publication acknowledgement does not bind retained operation".into(),
            );
        }
        if let Some(existing) = &retained.observed {
            if existing != &StackEffectResult::Publish(publication) {
                return Err("stack publication effect has a different retained observation".into());
            }
            transaction.commit().map_err(text)?;
            return Ok(retained);
        }
        let pub_key = publication_key(&reservation.attempt);
        let (existing_pub, pub_version) =
            load_optional::<StackPublication>(&transaction, &pub_key)?;
        if let Some(existing_pub) = existing_pub
            && existing_pub != publication
        {
            return Err("stack publication record conflicts with retained publication".into());
        }
        retained.observed = Some(StackEffectResult::Publish(publication.clone()));
        save(&transaction, &pub_key, pub_version, &publication)?;
        save(&transaction, &key, version, &retained)?;
        if let Some((session, request)) = self.authority_last.clone() {
            super::authority_bridge::revalidate_with(
                &mut self.authority,
                session,
                &request,
                currentness,
                now_ms,
            )?;
        }
        transaction.commit().map_err(text)?;
        Ok(retained)
    }

    /// Reserve and execute or reconcile read-only stack integration recovery.
    ///
    /// Returns immediately without calling `transport` once every layer of the
    /// stack is retained as integrated. A partial prefix can monotonically
    /// extend as higher layers land on the trunk.
    ///
    /// # Errors
    /// Returns a diagnostic, authority denial, or transport error.
    pub fn consume_stack_integration_effect(
        &mut self,
        work: &ImplementationWork,
        identity: &ContentId,
        fence: u64,
        plan: &StackPlan,
        transport: impl FnOnce(
            EffectDispatch,
            &StackPlan,
            &StackPublication,
        ) -> Result<Option<StackIntegration>, String>,
    ) -> Result<StackEffectReservation, String> {
        let (reservation, dispatch, publication) =
            self.reserve_stack_integration_effect(work, identity, fence, plan)?;
        if let Some(StackEffectResult::Integration(existing)) = &reservation.observed
            && existing.layers.len() == reservation.plan.layers.len()
        {
            return Ok(reservation);
        }
        let now_ms = self.now_ms();
        let currentness = self.attempt_currentness(identity, fence, now_ms);
        self.revalidate_last_authority(currentness, now_ms)?;
        match transport(dispatch, &reservation.plan, &publication)? {
            Some(integration) => {
                self.acknowledge_stack_integration_effect(work, &reservation, integration)
            }
            None => Ok(reservation),
        }
    }

    fn reserve_stack_integration_effect(
        &mut self,
        work: &ImplementationWork,
        identity: &ContentId,
        fence: u64,
        plan: &StackPlan,
    ) -> Result<(StackEffectReservation, EffectDispatch, StackPublication), String> {
        let now_ms = self.now_ms();
        let facts = self.authority_facts(identity, fence)?;
        let stack = stack_identity(plan).map_err(diagnostic)?;
        let top_candidate = plan
            .layers
            .last()
            .map(|layer| &layer.candidate)
            .ok_or("stack plan requires at least two layers")?;
        let effect = format!("{}:integration", stack.as_str());
        let expected = format!("refs/heads/{}", plan.trunk_branch);
        let request = facts.request(
            ProtectedOperation::IntegrationLand,
            &effect,
            &expected,
            &[
                stack.as_str(),
                plan.handoff.as_str(),
                plan.verification.as_str(),
                plan.repository.as_str(),
                plan.trunk_branch.as_str(),
            ],
            Some(top_candidate),
        )?;
        let currentness = self.attempt_currentness(identity, fence, now_ms);
        let session = self.admit_authority(&request, currentness, now_ms)?;
        self.revalidate_authority(session, &request, currentness, now_ms)?;

        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let (ledger, _) = load::<WorkLedger>(&transaction, WORK_KIND)?;
        let (attempts, _) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
        current_work(&ledger, work)?;
        let attempt = attempts
            .authorize_publication(identity, fence)
            .map_err(diagnostic)?;
        let work_identity = work.identity().map_err(diagnostic)?;
        if attempt.work_identity != work_identity || attempt.work_revision != work.revision {
            return Err("stack integration work does not own the exact verified attempt".into());
        }
        let (retained, _) = load_optional::<RetainedStackPlan>(&transaction, &plan_key(identity))?;
        let retained = retained.ok_or("stack publication plan is not prepared")?;
        if retained.stack != stack || retained.plan != *plan {
            return Err("stack plan differs from the prepared stack publication".into());
        }
        let (publication, _) =
            load_optional::<StackPublication>(&transaction, &publication_key(identity))?;
        let publication = publication.ok_or("stack publication is not retained")?;
        admit_stack_publication(plan, &publication).map_err(diagnostic)?;

        let key = effect_key(identity, StackEffectKind::Integration);
        let (existing, version) = load_optional::<StackEffectReservation>(&transaction, &key)?;
        if let Some(existing) = existing {
            if existing.work != work_identity
                || existing.revision != work.revision
                || existing.attempt != *identity
                || existing.fence != fence
                || existing.stack != stack
                || existing.kind != StackEffectKind::Integration
                || existing.plan != *plan
            {
                return Err(
                    "stack integration reservation conflicts with retained exact operation".into(),
                );
            }
            transaction.commit().map_err(text)?;
            return Ok((existing, EffectDispatch::ReconcileOnly, publication));
        }
        let mut reservation = StackEffectReservation {
            operation: work_identity.clone(),
            work: work_identity,
            revision: work.revision,
            attempt: identity.clone(),
            fence,
            stack,
            kind: StackEffectKind::Integration,
            plan: plan.clone(),
            observed: None,
        };
        reservation.operation = operation_identity(&reservation)?;
        save(&transaction, &key, version, &reservation)?;
        transaction.commit().map_err(text)?;
        Ok((reservation, EffectDispatch::FirstDispatch, publication))
    }

    fn acknowledge_stack_integration_effect(
        &mut self,
        work: &ImplementationWork,
        reservation: &StackEffectReservation,
        integration: StackIntegration,
    ) -> Result<StackEffectReservation, String> {
        let now_ms = self.now_ms();
        let facts = self.authority_facts(&reservation.attempt, reservation.fence)?;
        let top_candidate = reservation
            .plan
            .layers
            .last()
            .map(|layer| &layer.candidate)
            .ok_or("stack plan requires at least two layers")?;
        let topmost = integration
            .layers
            .last()
            .map(|layer| layer.merge_commit.native_id.as_str())
            .unwrap_or("none");
        let expected = format!("{}@{topmost}", reservation.plan.trunk_branch);
        let request = facts.request(
            ProtectedOperation::IntegrationLand,
            reservation.operation.as_str(),
            &expected,
            &["integration", reservation.stack.as_str(), topmost],
            Some(top_candidate),
        )?;
        let currentness = self.attempt_currentness(&reservation.attempt, reservation.fence, now_ms);
        let session = self.admit_authority(&request, currentness, now_ms)?;
        self.revalidate_authority(session, &request, currentness, now_ms)?;

        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let (ledger, _) = load::<WorkLedger>(&transaction, WORK_KIND)?;
        let (attempts, _) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
        current_work(&ledger, work)?;
        let attempt = attempts
            .authorize_publication(&reservation.attempt, reservation.fence)
            .map_err(diagnostic)?;
        let work_identity = work.identity().map_err(diagnostic)?;
        if attempt.work_identity != work_identity || attempt.work_revision != work.revision {
            return Err("stack integration acknowledgement belongs to different work".into());
        }
        let (publication, _) = load_optional::<StackPublication>(
            &transaction,
            &publication_key(&reservation.attempt),
        )?;
        let publication = publication.ok_or("stack publication is not retained")?;
        admit_stack_integration(&reservation.plan, &publication, &integration)
            .map_err(diagnostic)?;

        let key = effect_key(&reservation.attempt, StackEffectKind::Integration);
        let (retained, version) = load_optional::<StackEffectReservation>(&transaction, &key)?;
        let mut retained = retained.ok_or("stack integration effect was not reserved")?;
        if retained.operation != reservation.operation
            || retained.work != work_identity
            || retained.revision != work.revision
            || retained.fence != reservation.fence
            || retained.stack != reservation.stack
        {
            return Err(
                "stack integration acknowledgement does not bind retained operation".into(),
            );
        }
        if let Some(StackEffectResult::Integration(existing)) = &retained.observed {
            if existing == &integration {
                transaction.commit().map_err(text)?;
                return Ok(retained);
            }
            if integration.layers.len() <= existing.layers.len()
                || integration.layers.get(..existing.layers.len())
                    != Some(existing.layers.as_slice())
            {
                return Err("stack integration conflicts with previously admitted prefix".into());
            }
        } else if retained.observed.is_some() {
            return Err("stack integration effect has a different retained observation".into());
        }
        let int_key = integration_key(&reservation.attempt);
        let (existing_int, int_version) =
            load_optional::<StackIntegration>(&transaction, &int_key)?;
        if let Some(existing_int) = existing_int
            && (integration.layers.len() < existing_int.layers.len()
                || integration.layers.get(..existing_int.layers.len())
                    != Some(existing_int.layers.as_slice()))
        {
            return Err(
                "stack integration record conflicts with previously admitted prefix".into(),
            );
        }
        retained.observed = Some(StackEffectResult::Integration(integration.clone()));
        save(&transaction, &int_key, int_version, &integration)?;
        save(&transaction, &key, version, &retained)?;
        if let Some((session, request)) = self.authority_last.clone() {
            super::authority_bridge::revalidate_with(
                &mut self.authority,
                session,
                &request,
                currentness,
                now_ms,
            )?;
        }
        transaction.commit().map_err(text)?;
        Ok(retained)
    }

    /// Read the retained stack plan, publication, and integration prefix for one
    /// attempt without modifying any state.
    ///
    /// # Errors
    /// Returns a diagnostic if the SQLite transaction or JSON decoding fails.
    pub fn stack_publication_record(
        &mut self,
        identity: &ContentId,
    ) -> Result<Option<StackPublicationRecord>, String> {
        let transaction = self.connection.transaction().map_err(text)?;
        let (plan_row, _) = load_optional::<RetainedStackPlan>(&transaction, &plan_key(identity))?;
        let Some(plan_row) = plan_row else {
            transaction.commit().map_err(text)?;
            return Ok(None);
        };
        let (publication, _) =
            load_optional::<StackPublication>(&transaction, &publication_key(identity))?;
        let (integration, _) =
            load_optional::<StackIntegration>(&transaction, &integration_key(identity))?;
        transaction.commit().map_err(text)?;
        Ok(Some(StackPublicationRecord {
            stack: plan_row.stack,
            plan: plan_row.plan,
            layer_verifications: plan_row.layer_verifications,
            publication,
            integration,
        }))
    }

    /// Read an existing stack effect reservation without reserving or
    /// dispatching anything.
    ///
    /// # Errors
    /// Returns a diagnostic if the SQLite transaction or JSON decoding fails, or
    /// if the retained reservation fails consistency checks against current
    /// work, attempt, fence, and stack plan.
    pub fn read_stack_effect(
        &mut self,
        work: &ImplementationWork,
        identity: &ContentId,
        fence: u64,
        kind: StackEffectKind,
    ) -> Result<Option<StackEffectReservation>, String> {
        let transaction = self.connection.transaction().map_err(text)?;
        let (ledger, _) = load::<WorkLedger>(&transaction, WORK_KIND)?;
        current_work(&ledger, work)?;
        let (attempts, _) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
        let attempt = attempts
            .authorize_publication(identity, fence)
            .map_err(diagnostic)?;
        let work_identity = work.identity().map_err(diagnostic)?;
        if attempt.work_identity != work_identity || attempt.work_revision != work.revision {
            return Err("stack readback differs from current work, attempt, or fence".into());
        }
        let (retained_plan, _) =
            load_optional::<RetainedStackPlan>(&transaction, &plan_key(identity))?;
        let retained_plan = retained_plan.ok_or("stack readback has no retained plan")?;
        let (Some(reservation), _) =
            load_optional::<StackEffectReservation>(&transaction, &effect_key(identity, kind))?
        else {
            transaction.commit().map_err(text)?;
            return Ok(None);
        };
        let mut identity_input = reservation.clone();
        identity_input.operation = work_identity.clone();
        identity_input.observed = None;
        if reservation.operation != operation_identity(&identity_input)?
            || reservation.work != work_identity
            || reservation.revision != work.revision
            || reservation.attempt != *identity
            || reservation.fence != fence
            || reservation.kind != kind
            || reservation.stack != retained_plan.stack
            || reservation.plan != retained_plan.plan
        {
            return Err(
                "stack readback conflicts with canonical work, attempt, stack, or operation".into(),
            );
        }
        transaction.commit().map_err(text)?;
        Ok(Some(reservation))
    }
}

fn current_work(ledger: &WorkLedger, work: &ImplementationWork) -> Result<(), String> {
    if ledger.current().any(|current| current == work) {
        Ok(())
    } else {
        Err("stack publication requires the exact retained current source version".into())
    }
}

fn plan_key(identity: &ContentId) -> String {
    format!("{PLAN_PREFIX}{}", identity.as_str())
}

pub(crate) fn has_retained_stack_plan(
    transaction: &rusqlite::Transaction<'_>,
    identity: &ContentId,
) -> Result<bool, String> {
    let (plan_row, _) = load_optional::<RetainedStackPlan>(transaction, &plan_key(identity))?;
    Ok(plan_row.is_some())
}

fn publication_key(identity: &ContentId) -> String {
    format!("{PUBLICATION_PREFIX}{}", identity.as_str())
}

fn integration_key(identity: &ContentId) -> String {
    format!("{INTEGRATION_PREFIX}{}", identity.as_str())
}

fn effect_key(identity: &ContentId, kind: StackEffectKind) -> String {
    format!("{EFFECT_PREFIX}{}:{}", identity.as_str(), kind.key())
}

fn operation_identity(reservation: &StackEffectReservation) -> Result<ContentId, String> {
    let bytes = serde_json::to_vec(reservation).map_err(text)?;
    ContentId::from_blake3_hex(blake3::hash(&bytes).to_hex().to_string()).map_err(diagnostic)
}

fn load_optional<T: serde::de::DeserializeOwned>(
    transaction: &rusqlite::Transaction<'_>,
    key: &str,
) -> Result<(Option<T>, u64), String> {
    let row: Option<(i64, Vec<u8>)> = transaction
        .query_row(
            "SELECT version, bytes FROM implementation_state WHERE kind = ?1",
            params![key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(text)?;
    match row {
        None => Ok((None, 0)),
        Some((version, bytes)) => {
            let version = u64::try_from(version)
                .map_err(|_| "stack state has a negative stored version".to_string())?;
            let value = serde_json::from_slice(&bytes).map_err(text)?;
            Ok((Some(value), version))
        }
    }
}

#[cfg(test)]
mod tests;
