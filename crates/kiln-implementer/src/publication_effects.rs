//! Durable, separately admitted publication effects. The callback is the only
//! transport boundary; a recovered reservation grants read-only reconciliation,
//! never another blind mutation. Acknowledgement and the existing attempt
//! transition commit in the same SQLite transaction.
//! r[impl kiln.publication.exact]

use kiln_core::implementation::{
    AttemptLedger, ImplementationWork, IntegrationReceipt, PublicationRecord, SourceStatusPlan,
    SourceStatusReceipt, WorkLedger,
};
use kiln_core::types::{ContentId, ExternalRef};
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

use crate::{ATTEMPT_KIND, Implementer, WORK_KIND, load, save, text};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationEffectKind {
    Branch,
    PullRequest,
    Integration,
    SourceStatus(SourceStatusPlan),
    CairnLifecycle(CairnLifecycleBinding),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectDispatch {
    FirstDispatch,
    ReconcileOnly,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationEffectResult {
    Branch(ExternalRef),
    PullRequest {
        pull_request: ExternalRef,
        head: ExternalRef,
    },
    Integration(IntegrationReceipt),
    SourceStatus {
        plan: SourceStatusPlan,
        receipt: SourceStatusReceipt,
    },
    CairnLifecycle(CairnLifecycleObservation),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationEffectReservation {
    pub operation: ContentId,
    pub work: ContentId,
    pub revision: u64,
    pub attempt: ContentId,
    pub fence: u64,
    pub candidate: ExternalRef,
    /// Branch creation expects absence, never an unguarded branch update.
    pub expected_remote_head: Option<ExternalRef>,
    pub kind: PublicationEffectKind,
    pub publication: PublicationRecord,
    /// None includes crash, transport error and unresolved remote absence.
    pub observed: Option<PublicationEffectResult>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CairnLifecycleAction {
    ObserveReviewedSync,
    ObserveReviewedArchive,
}

/// Outer binding of Cairn's existing authoritative result. This is not a
/// second completion evaluator and cannot authorize new source mutations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CairnLifecycleBinding {
    pub source: kiln_core::implementation::SourceRef,
    pub handoff: ContentId,
    pub verification: ContentId,
    pub authoritative_completion: ContentId,
    pub candidate: ExternalRef,
    pub action: CairnLifecycleAction,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CairnLifecycleObservation {
    pub binding: CairnLifecycleBinding,
    pub observed_tree: ExternalRef,
    /// Native policy shared by authoritative completion and lifecycle receipt.
    pub lifecycle_policy: ContentId,
    /// Existing producer-owned sync/archive observation or store receipt.
    pub lifecycle_receipt: ExternalRef,
}

impl PublicationEffectKind {
    fn key(&self) -> String {
        match self {
            Self::Branch => "branch".into(),
            Self::PullRequest => "pull-request".into(),
            Self::Integration => "integration".into(),
            Self::SourceStatus(plan) => format!("source:{}", plan.source.native_id),
            Self::CairnLifecycle(binding) => {
                format!("cairn:{}:{:?}", binding.source.native_id, binding.action)
            }
        }
    }
}

impl Implementer {
    /// Reserve before invoking transport. Only the first caller may mutate.
    /// Returned pending state is not a successful publication/completion result.
    // r[impl kiln.publication.recovery]
    pub fn consume_publication_effect(
        &mut self,
        work: &ImplementationWork,
        identity: &ContentId,
        fence: u64,
        kind: PublicationEffectKind,
        transport: impl FnOnce(
            EffectDispatch,
            &PublicationRecord,
        ) -> Result<Option<PublicationEffectResult>, String>,
    ) -> Result<PublicationEffectReservation, String> {
        let (reservation, dispatch) =
            self.reserve_publication_effect(work, identity, fence, kind)?;
        if reservation.observed.is_some() {
            return Ok(reservation);
        }
        let now_ms = self.now_ms();
        let currentness = self.attempt_currentness(identity, fence, now_ms);
        self.revalidate_last_authority(currentness, now_ms)?;
        // An error intentionally leaves the committed pending reservation.
        match transport(dispatch, &reservation.publication)? {
            Some(observed) => self.acknowledge_publication_effect(work, &reservation, observed),
            None => Ok(reservation),
        }
    }

    pub fn reserve_publication_effect(
        &mut self,
        work: &ImplementationWork,
        identity: &ContentId,
        fence: u64,
        kind: PublicationEffectKind,
    ) -> Result<(PublicationEffectReservation, EffectDispatch), String> {
        let now_ms = self.now_ms();
        let (work_facts, attempt_facts) = self.authority_facts(identity, fence)?;
        let kind_label = format!("{kind:?}");
        let candidate = attempt_facts
            .publication
            .as_ref()
            .map(|publication| &publication.plan.candidate);
        let request = super::authority_bridge::request_for(
            kiln_core::implementation::authority::ProtectedOperation::PublicationPublish,
            &work_facts,
            &attempt_facts,
            &[&kind_label],
            candidate,
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
        if attempt.work_identity != work.identity().map_err(diagnostic)?
            || attempt.work_revision != work.revision
        {
            return Err("publication work does not own the exact verified attempt".into());
        }
        let publication = attempt
            .publication
            .as_ref()
            .ok_or("publication plan is not prepared")?;
        let key = effect_key(identity, &kind);
        if let Some((existing, _)) = load_effect(&transaction, &key)? {
            if existing.work != work.identity().map_err(diagnostic)?
                || existing.revision != work.revision
                || existing.attempt != *identity
                || existing.fence != fence
                || existing.kind != kind
                || existing.candidate != publication.plan.candidate
                || existing.publication.plan != publication.plan
            {
                return Err(
                    "publication reservation conflicts with retained exact operation".into(),
                );
            }
            transaction.commit().map_err(text)?;
            return Ok((existing, EffectDispatch::ReconcileOnly));
        }
        admit_stage(work, attempt, publication, &kind)?;
        let mut reservation = PublicationEffectReservation {
            operation: work.identity().map_err(diagnostic)?,
            work: work.identity().map_err(diagnostic)?,
            revision: work.revision,
            attempt: identity.clone(),
            fence,
            candidate: publication.plan.candidate.clone(),
            expected_remote_head: if matches!(kind, PublicationEffectKind::Branch) {
                None
            } else {
                publication.remote_head.clone()
            },
            kind,
            publication: publication.clone(),
            observed: None,
        };
        reservation.operation = operation_identity(&reservation)?;
        save(&transaction, &key, 0, &reservation)?;
        transaction.commit().map_err(text)?;
        Ok((reservation, EffectDispatch::FirstDispatch))
    }

    /// Read an existing publication intent without reserving or dispatching it.
    pub fn read_publication_effect(
        &mut self,
        work: &ImplementationWork,
        identity: &ContentId,
        fence: u64,
        kind: PublicationEffectKind,
    ) -> Result<Option<PublicationEffectReservation>, String> {
        let transaction = self.connection.transaction().map_err(text)?;
        let (ledger, _) = load::<WorkLedger>(&transaction, WORK_KIND)?;
        current_work(&ledger, work)?;
        let (attempts, _) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
        let attempt = attempts
            .authorize_publication(identity, fence)
            .map_err(diagnostic)?;
        if attempt.work_identity != work.identity().map_err(diagnostic)?
            || attempt.work_revision != work.revision
        {
            return Err("publication readback differs from current work, attempt, or fence".into());
        }
        let publication = attempt
            .publication
            .as_ref()
            .ok_or("publication readback has no retained plan")?;
        let Some((reservation, _)) = load_effect(&transaction, &effect_key(identity, &kind))?
        else {
            transaction.commit().map_err(text)?;
            return Ok(None);
        };
        let mut identity_input = reservation.clone();
        identity_input.operation = work.identity().map_err(diagnostic)?;
        identity_input.observed = None;
        if reservation.operation != operation_identity(&identity_input)?
            || reservation.work != work.identity().map_err(diagnostic)?
            || reservation.revision != work.revision
            || reservation.attempt != *identity
            || reservation.fence != fence
            || reservation.kind != kind
            || reservation.candidate != publication.plan.candidate
            || reservation.publication.plan != publication.plan
        {
            return Err("publication readback conflicts with canonical work, attempt, candidate, or operation".into());
        }
        transaction.commit().map_err(text)?;
        Ok(Some(reservation))
    }

    /// A recovered transport result cannot bypass current work/fence authority.
    /// Equal replay retains the original operation and observation identities.
    pub fn acknowledge_publication_effect(
        &mut self,
        work: &ImplementationWork,
        reservation: &PublicationEffectReservation,
        observed: PublicationEffectResult,
    ) -> Result<PublicationEffectReservation, String> {
        let now_ms = self.now_ms();
        let (work_facts, attempt_facts) =
            self.authority_facts(&reservation.attempt, reservation.fence)?;
        let kind_label = format!("{:?}", reservation.kind);
        let request = super::authority_bridge::request_for(
            kiln_core::implementation::authority::ProtectedOperation::PublicationPublish,
            &work_facts,
            &attempt_facts,
            &[&kind_label],
            Some(&reservation.candidate),
        )?;
        let currentness = self.attempt_currentness(&reservation.attempt, reservation.fence, now_ms);
        let session = self.admit_authority(&request, currentness, now_ms)?;
        self.revalidate_authority(session, &request, currentness, now_ms)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let (ledger, _) = load::<WorkLedger>(&transaction, WORK_KIND)?;
        let (mut attempts, attempt_version) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
        current_work(&ledger, work)?;
        let attempt = attempts
            .authorize_publication(&reservation.attempt, reservation.fence)
            .map_err(diagnostic)?;
        if attempt.work_identity != work.identity().map_err(diagnostic)?
            || attempt.work_revision != work.revision
        {
            return Err("publication acknowledgement belongs to different work".into());
        }
        let key = effect_key(&reservation.attempt, &reservation.kind);
        let (mut retained, version) =
            load_effect(&transaction, &key)?.ok_or("publication effect was not reserved")?;
        if retained.operation != reservation.operation
            || retained.work != work.identity().map_err(diagnostic)?
            || retained.revision != work.revision
            || retained.fence != reservation.fence
        {
            return Err("publication acknowledgement does not bind retained operation".into());
        }
        if let Some(existing) = &retained.observed {
            if existing != &observed {
                return Err("publication effect has a different retained observation".into());
            }
            transaction.commit().map_err(text)?;
            return Ok(retained);
        }
        let identity = &retained.attempt;
        let fence = retained.fence;
        match (&retained.kind, &observed) {
            (PublicationEffectKind::Branch, PublicationEffectResult::Branch(head)) => {
                attempts
                    .record_branch_published(identity, fence, head)
                    .map_err(diagnostic)?;
            }
            (
                PublicationEffectKind::PullRequest,
                PublicationEffectResult::PullRequest { pull_request, head },
            ) => {
                attempts
                    .record_pull_request_published(identity, fence, pull_request, head)
                    .map_err(diagnostic)?;
            }
            (PublicationEffectKind::Integration, PublicationEffectResult::Integration(receipt)) => {
                attempts
                    .record_integration(work, identity, fence, receipt)
                    .map_err(diagnostic)?;
            }
            (
                PublicationEffectKind::SourceStatus(expected),
                PublicationEffectResult::SourceStatus { plan, receipt },
            ) if expected == plan => {
                attempts
                    .record_source_status(work, identity, fence, plan, receipt)
                    .map_err(diagnostic)?;
            }
            (
                PublicationEffectKind::CairnLifecycle(binding),
                PublicationEffectResult::CairnLifecycle(observation),
            ) => {
                let attempt = attempts
                    .authorize_publication(identity, fence)
                    .map_err(diagnostic)?;
                let publication = attempt.publication.as_ref().ok_or("missing publication")?;
                admit_lifecycle(work, attempt, publication, binding)?;
                if observation.binding != *binding
                    || observation.observed_tree != publication.plan.tree
                {
                    return Err(
                        "Cairn lifecycle observation differs from reviewed candidate/tree".into(),
                    );
                }
                observation
                    .lifecycle_receipt
                    .validate()
                    .map_err(diagnostic)?;
            }
            _ => return Err("publication result kind differs from reserved effect".into()),
        }
        retained.observed = Some(observed);
        save(&transaction, ATTEMPT_KIND, attempt_version, &attempts)?;
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
    /// Read an acknowledged native Cairn lifecycle effect without reserving or dispatching anything.
    /// Current work, attempt fence, candidate, binding and admitted policy are rechecked in one read transaction.
    pub fn read_cairn_lifecycle_effect(
        &mut self,
        work: &ImplementationWork,
        identity: &ContentId,
        fence: u64,
        binding: &CairnLifecycleBinding,
    ) -> Result<Option<PublicationEffectReservation>, String> {
        use kiln_core::fair_admission::AdmissionLedger;

        let transaction = self.connection.transaction().map_err(text)?;
        let (ledger, _) = load::<WorkLedger>(&transaction, WORK_KIND)?;
        current_work(&ledger, work)?;
        let (attempts, _) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
        let attempt = attempts
            .authorize_publication(identity, fence)
            .map_err(diagnostic)?;
        if attempt.work_identity != work.identity().map_err(diagnostic)?
            || attempt.work_revision != work.revision
        {
            return Err("lifecycle readback differs from current work, attempt, or fence".into());
        }
        let publication = attempt
            .publication
            .as_ref()
            .ok_or("lifecycle readback has no retained publication")?;
        admit_lifecycle(work, attempt, publication, binding)?;
        let (admission, _) = load::<AdmissionLedger>(&transaction, "fair-admission-v1")?;
        let current_policy = admission
            .policy
            .ok_or("lifecycle readback has no current admitted policy")?
            .identity()
            .map_err(diagnostic)?;
        let kind = PublicationEffectKind::CairnLifecycle(binding.clone());
        let Some((reservation, _)) = load_effect(&transaction, &effect_key(identity, &kind))?
        else {
            transaction.commit().map_err(text)?;
            return Ok(None);
        };
        let mut identity_input = reservation.clone();
        identity_input.operation = work.identity().map_err(diagnostic)?;
        identity_input.observed = None;
        if reservation.operation != operation_identity(&identity_input)?
            || reservation.work != work.identity().map_err(diagnostic)?
            || reservation.revision != work.revision
            || reservation.attempt != *identity
            || reservation.fence != fence
            || reservation.kind != kind
            || reservation.candidate != publication.plan.candidate
            || reservation.publication.plan != publication.plan
        {
            return Err("lifecycle readback conflicts with canonical work, attempt, candidate, or operation".into());
        }
        let observation = match reservation.observed.as_ref() {
            None => {
                transaction.commit().map_err(text)?;
                return Ok(None);
            }
            Some(PublicationEffectResult::CairnLifecycle(observation)) => observation,
            Some(_) => {
                return Err("lifecycle readback contains a different owner result kind".into());
            }
        };
        if observation.binding != *binding
            || observation.observed_tree != publication.plan.tree
            || observation.lifecycle_policy != current_policy
        {
            return Err("lifecycle receipt differs from current binding, candidate tree, or admitted policy".into());
        }
        observation
            .lifecycle_receipt
            .validate()
            .map_err(diagnostic)?;
        transaction.commit().map_err(text)?;
        Ok(Some(reservation))
    }
}

fn current_work(ledger: &WorkLedger, work: &ImplementationWork) -> Result<(), String> {
    if ledger.current().any(|current| current == work) {
        Ok(())
    } else {
        Err("publication requires the exact retained current source version".into())
    }
}

fn admit_stage(
    work: &ImplementationWork,
    attempt: &kiln_core::implementation::ImplementationAttempt,
    publication: &PublicationRecord,
    kind: &PublicationEffectKind,
) -> Result<(), String> {
    use kiln_core::implementation::PublicationPhase as P;
    let admitted = match kind {
        PublicationEffectKind::Branch => publication.phase == P::Prepared,
        PublicationEffectKind::PullRequest => publication.phase == P::BranchPublished,
        PublicationEffectKind::Integration => publication.phase == P::PullRequestPublished,
        PublicationEffectKind::SourceStatus(plan) => {
            kiln_core::implementation::admit_source_status_plan(work, attempt, plan)
                .map_err(diagnostic)?;
            publication.phase == P::Integrated
        }
        PublicationEffectKind::CairnLifecycle(binding) => {
            admit_lifecycle(work, attempt, publication, binding)?;
            matches!(publication.phase, P::Integrated | P::SourceReconciled)
        }
    };
    if admitted {
        Ok(())
    } else {
        Err("publication effect is not admitted in the current phase".into())
    }
}

fn admit_lifecycle(
    work: &ImplementationWork,
    attempt: &kiln_core::implementation::ImplementationAttempt,
    publication: &PublicationRecord,
    binding: &CairnLifecycleBinding,
) -> Result<(), String> {
    let verification = attempt
        .verification
        .as_ref()
        .ok_or("Cairn lifecycle requires retained verification")?;
    let integration = publication
        .integration
        .as_ref()
        .ok_or("Cairn lifecycle requires retained integration")?;
    kiln_core::implementation::admit_integration(work, attempt, publication, integration)
        .map_err(diagnostic)?;
    let mut matching = verification
        .run
        .checks
        .iter()
        .filter(|check| check.name == "cairn-authoritative-completion");
    let accepted = matching.next();
    if work.terminal_policy != kiln_core::implementation::TerminalPolicy::VerifiedIntegrated
        || binding.source.kind != kiln_core::implementation::WorkSourceKind::CairnTask
        || !work.aliases.contains(&binding.source)
        || binding.handoff != publication.plan.handoff
        || binding.verification != publication.plan.verification
        || binding.candidate != publication.plan.candidate
        || verification.handoff != binding.handoff
        || verification.work_identity != work.identity().map_err(diagnostic)?
        || verification.work_revision != work.revision
        || verification.attempt != attempt.identity
        || verification.fence != attempt.fence
        || verification.candidate != binding.candidate
        || verification.tree != publication.plan.tree
        || matching.next().is_some()
        || accepted.is_none_or(|check| {
            check.outcome != "pass"
                || check.kind != "cairn-agent-completion-v2"
                || check.artifact_digest.as_deref()
                    != Some(binding.authoritative_completion.as_str())
        })
    {
        return Err(
            "Cairn lifecycle lacks exact task/spec/candidate/authoritative-result/terminal binding"
                .into(),
        );
    }
    Ok(())
}

fn effect_key(identity: &ContentId, kind: &PublicationEffectKind) -> String {
    format!("publication-effect:{}:{}", identity.as_str(), kind.key())
}

fn operation_identity(reservation: &PublicationEffectReservation) -> Result<ContentId, String> {
    let bytes = serde_json::to_vec(reservation).map_err(text)?;
    ContentId::from_blake3_hex(blake3::hash(&bytes).to_hex().to_string()).map_err(diagnostic)
}

fn load_effect(
    transaction: &rusqlite::Transaction<'_>,
    key: &str,
) -> Result<Option<(PublicationEffectReservation, u64)>, String> {
    let row: Option<(i64, Vec<u8>)> = transaction
        .query_row(
            "SELECT version, bytes FROM implementation_state WHERE kind = ?1",
            params![key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(text)?;
    row.map(|(version, bytes)| {
        let version = u64::try_from(version)
            .map_err(|_| "publication effect has a negative stored version".to_string())?;
        let value = serde_json::from_slice(&bytes).map_err(text)?;
        Ok((value, version))
    })
    .transpose()
}

fn diagnostic(error: kiln_core::types::Diagnostic) -> String {
    format!("{}: {}", error.code, error.message)
}

#[cfg(test)]
mod tests;
