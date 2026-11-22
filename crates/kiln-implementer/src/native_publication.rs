//! Durable native-Git intents and read-only recovery.
//!
//! Each effect is committed before the provider callback. A restart changes the
//! callback mode to reconciliation-only. Policy and fencing are read from the
//! durable implementer ledgers in the same transaction as reservation/receipt.

use kiln_core::fair_admission::AdmissionLedger;
use kiln_core::implementation::{
    AttemptLedger, ImplementationWork, NATIVE_GIT_RECEIPT_SCHEMA, NativeGitObservation,
    NativeGitPlan, NativeGitReceipt, admit_native_git_plan,
};
use kiln_core::types::ContentId;
use rusqlite::{Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
pub const NATIVE_GIT_QUALIFICATION_REQUEST_SCHEMA: &str =
    "kiln.native-git-qualification-request.v1";

/// Versioned non-secret shell request bound to one canonical implementation attempt.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeGitQualificationRequest<R> {
    pub schema: String,
    pub database: std::path::PathBuf,
    pub repository_path: std::path::PathBuf,
    pub change_id: String,
    pub actor: String,
    pub work: kiln_core::types::ContentId,
    pub attempt: kiln_core::types::ContentId,
    pub fence: u64,
    pub plan: NativeGitPlan,
    pub remote: R,
    /// Explicit local-operator profile selection; absent authority denies.
    #[serde(default)]
    pub local_operator: Option<String>,
}

pub const NATIVE_GIT_LIFECYCLE_REQUEST_SCHEMA: &str = "kiln.native-git-lifecycle-request.v1";

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeGitLifecycleRequest<R> {
    pub schema: String,
    pub database: std::path::PathBuf,
    pub actor: String,
    pub work: ContentId,
    pub attempt: ContentId,
    pub fence: u64,
    pub change_id: String,
    pub receipt_path: String,
    pub action: crate::publication_effects::CairnLifecycleAction,
    pub remote: R,
    /// Explicit local-operator profile selection; absent authority denies.
    #[serde(default)]
    pub local_operator: Option<String>,
}

use crate::{ATTEMPT_KIND, Implementer, WORK_KIND, load, save, text};

const POLICY_KIND: &str = "fair-admission-v1";
const PLAN_PREFIX: &str = "native-git-plan:";
const EFFECT_PREFIX: &str = "native-git-effect:";

/// Current review-store snapshot supplied by the trusted collaboration shell.
/// The implementer re-admits its exact candidate and checklist at every effect boundary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeGitReviewAuthority {
    pub state: kiln_core::change_review::ReviewState,
    pub profile_identity: ContentId,
    pub source_identity: ContentId,
    pub ci_identity: ContentId,
    pub facts: kiln_core::change_review::Facts,
    pub change_id: String,
    pub revision_id: String,
    pub candidate_id: String,
    pub actor: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeGitEffectKind {
    BranchPublication,
    TargetIntegration,
}
/// Identifies the retained attempt and plan for one native Git effect.
#[derive(Clone, Copy, Debug)]
pub struct NativeGitEffectSelection<'a> {
    pub work: &'a ImplementationWork,
    pub attempt: &'a ContentId,
    pub fence: u64,
    pub plan_identity: &'a ContentId,
    pub kind: NativeGitEffectKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeGitDispatch {
    FirstDispatch,
    ReconcileOnly,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeGitEffectReservation {
    pub operation: ContentId,
    pub plan_identity: ContentId,
    pub plan: NativeGitPlan,
    pub kind: NativeGitEffectKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed: Option<NativeGitObservation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt: Option<NativeGitReceipt>,
}

/// #47-facing completion association; the integration identity is native Git,
/// not an issue or fabricated pull request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeCairnLifecycleReceipt {
    pub schema: String,
    pub integration: ContentId,
    pub work_identity: ContentId,
    pub work_revision: u64,
    pub attempt: ContentId,
    pub fence: u64,
    pub binding: crate::publication_effects::CairnLifecycleBinding,
    pub observed_tree: kiln_core::types::ExternalRef,
    pub lifecycle_policy: ContentId,
    pub lifecycle_receipt: kiln_core::types::ExternalRef,
}

pub const NATIVE_CAIRN_LIFECYCLE_SCHEMA: &str = "kiln.cairn-native-git-lifecycle.v1";

impl NativeGitEffectKind {
    fn key(self) -> &'static str {
        match self {
            Self::BranchPublication => "branch",
            Self::TargetIntegration => "integration",
        }
    }

    fn operation(self, plan: &NativeGitPlan) -> &ContentId {
        match self {
            Self::BranchPublication => &plan.branch_operation,
            Self::TargetIntegration => &plan.integration_operation,
        }
    }
}

impl Implementer {
    /// Persist an exact plan after re-reading current work, verification, fence
    /// and the currently admitted policy from the durable state.
    pub fn prepare_native_git(
        &mut self,
        work: &ImplementationWork,
        identity: &ContentId,
        fence: u64,
        plan: &mut NativeGitPlan,
        review: &NativeGitReviewAuthority,
    ) -> Result<ContentId, String> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let plan_key = format!("{PLAN_PREFIX}{}", identity.as_str());
        let (existing, _) = load::<Option<(NativeGitPlan, ContentId)>>(&transaction, &plan_key)?;
        if let Some((retained, retained_identity)) = existing {
            if !same_native_git_request(plan, &retained)
                || retained.work_identity != work.identity().map_err(show)?
                || retained.work_revision != work.revision
                || retained.attempt != *identity
                || retained.fence != fence
            {
                return Err("native Git resume request differs from retained intent".into());
            }
            *plan = retained;
            transaction.commit().map_err(text)?;
            return Ok(retained_identity);
        }
        let (ledger, _) = load::<kiln_core::implementation::WorkLedger>(&transaction, WORK_KIND)?;
        let (attempts, _) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
        ensure_current_work(&ledger, work)?;
        let attempt = attempts
            .authorize_publication(identity, fence)
            .map_err(show)?;
        ensure_attempt_work(attempt, work)?;
        let handoff = attempt
            .handoff
            .as_ref()
            .ok_or("native Git requires a retained candidate handoff")?;
        let verification = attempt
            .verification
            .as_ref()
            .ok_or("native Git requires retained verification")?;
        let (review_identity, checks_identity) = current_review_identities(review, work, plan)?;
        plan.review = review_identity;
        plan.checks = checks_identity;
        plan.branch_operation =
            kiln_core::implementation::native_branch_operation_id(plan).map_err(show)?;
        plan.integration_operation =
            kiln_core::implementation::native_integration_operation_id(plan).map_err(show)?;
        let plan_identity =
            admit_native_git_plan(work, attempt, handoff, verification, plan).map_err(show)?;
        let (review_identity, checks_identity) = current_review_identities(review, work, plan)?;
        let policy_identity = current_policy_identity(&transaction)?;
        if plan.policy != policy_identity
            || review_identity != plan.review
            || checks_identity != plan.checks
        {
            return Err(
                "native Git plan does not bind current admitted policy, review and checks".into(),
            );
        }
        let key = format!("{PLAN_PREFIX}{}", identity.as_str());
        let (existing, version) = load::<Option<(NativeGitPlan, ContentId)>>(&transaction, &key)?;
        if let Some((retained, retained_identity)) = existing {
            if retained != *plan || retained_identity != plan_identity {
                return Err(
                    "a different native Git plan is already retained for this attempt".into(),
                );
            }
        } else {
            save(
                &transaction,
                &key,
                version,
                &Some((plan.clone(), plan_identity.clone())),
            )?;
        }
        transaction.commit().map_err(text)?;
        Ok(plan_identity)
    }

    /// Reserve one native effect before transmission. Existing pending work is
    /// reconciliation-only and cannot be dispatched a second time.
    #[cfg(test)]
    pub(crate) fn consume_native_git_effect(
        &mut self,
        selection: NativeGitEffectSelection<'_>,
        review: &NativeGitReviewAuthority,
        transport: impl FnOnce(
            NativeGitDispatch,
            &NativeGitPlan,
        ) -> Result<Option<NativeGitObservation>, String>,
    ) -> Result<NativeGitEffectReservation, String> {
        let NativeGitEffectSelection {
            work,
            attempt: identity,
            fence,
            plan_identity,
            kind,
        } = selection;
        let (mut reservation, dispatch) =
            self.reserve_native_git_effect(work, identity, fence, plan_identity, kind, review)?;
        if let Some(observed) = reservation.observed.clone()
            && !matches!(observed, NativeGitObservation::Unknown { .. })
        {
            return self.acknowledge_native_git_effect(
                work,
                identity,
                fence,
                &reservation,
                observed,
                review,
            );
        }
        if dispatch == NativeGitDispatch::ReconcileOnly && reservation.receipt.is_some() {
            return Ok(reservation);
        }
        match transport(dispatch, &reservation.plan)? {
            Some(observed) => {
                reservation = self.acknowledge_native_git_effect(
                    work,
                    identity,
                    fence,
                    &reservation,
                    observed,
                    review,
                )?;
                Ok(reservation)
            }
            None => Ok(reservation),
        }
    }

    /// Re-resolve collaboration authority immediately before reservation and after
    /// provider readback, so neither a stale snapshot nor a caller JSON document
    /// can authorize an effect acknowledgement.
    pub fn consume_native_git_effect_with_current_authority(
        &mut self,
        selection: NativeGitEffectSelection<'_>,
        mut observe_authority: impl FnMut() -> Result<NativeGitReviewAuthority, String>,
        transport: impl FnOnce(
            NativeGitDispatch,
            &NativeGitPlan,
        ) -> Result<Option<NativeGitObservation>, String>,
    ) -> Result<NativeGitEffectReservation, String> {
        let NativeGitEffectSelection {
            work,
            attempt: identity,
            fence,
            plan_identity,
            kind,
        } = selection;
        let now_ms = self.now_ms();
        let (work_facts, attempt_facts) = self.authority_facts(identity, fence)?;
        let operation = match kind {
            NativeGitEffectKind::TargetIntegration => {
                kiln_core::implementation::authority::ProtectedOperation::IntegrationLand
            }
            NativeGitEffectKind::BranchPublication => {
                kiln_core::implementation::authority::ProtectedOperation::PublicationPublish
            }
        };
        let native_plan = self.native_git_plan(identity)?;
        let candidate = native_plan
            .as_ref()
            .map(|(plan, _)| &plan.candidate)
            .or(attempt_facts.candidate.as_ref());
        let request = super::authority_bridge::request_for(
            operation,
            &work_facts,
            &attempt_facts,
            &[plan_identity.as_str()],
            candidate,
        )?;
        let currentness = self.attempt_currentness(identity, fence, now_ms);
        self.authorize_and_revalidate(&request, currentness, now_ms)?;
        let current = observe_authority()?;
        let (reservation, dispatch) =
            self.reserve_native_git_effect(work, identity, fence, plan_identity, kind, &current)?;
        if dispatch == NativeGitDispatch::FirstDispatch {
            let current = observe_authority()?;
            let transaction = self.connection.transaction().map_err(text)?;
            let (ledger, _) =
                load::<kiln_core::implementation::WorkLedger>(&transaction, WORK_KIND)?;
            let (attempts, _) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
            let attempt = attempts
                .get(identity)
                .ok_or("native Git attempt is not retained")?;
            ensure_current_work(&ledger, work)?;
            attempts
                .authorize_publication(identity, fence)
                .map_err(show)?;
            ensure_attempt_work(attempt, work)?;
            let (review_id, checks_id) =
                current_review_identities(&current, work, &reservation.plan)?;
            if review_id != reservation.plan.review
                || checks_id != reservation.plan.checks
                || current_policy_identity(&transaction)? != reservation.plan.policy
            {
                return Err("native Git authority changed after intent reservation".into());
            }
            transaction.commit().map_err(text)?;
        }
        if dispatch == NativeGitDispatch::ReconcileOnly && reservation.receipt.is_some() {
            return Ok(reservation);
        }
        let effect_time_ms = self.now_ms();
        let effect_currentness = self.attempt_currentness(identity, fence, effect_time_ms);
        self.revalidate_last_authority(effect_currentness, effect_time_ms)?;
        match transport(dispatch, &reservation.plan)? {
            Some(observed) => {
                let current = observe_authority()?;
                self.acknowledge_native_git_effect(
                    work,
                    identity,
                    fence,
                    &reservation,
                    observed,
                    &current,
                )
            }
            None => Ok(reservation),
        }
    }

    fn reserve_native_git_effect(
        &mut self,
        work: &ImplementationWork,
        identity: &ContentId,
        fence: u64,
        plan_identity: &ContentId,
        kind: NativeGitEffectKind,
        review: &NativeGitReviewAuthority,
    ) -> Result<(NativeGitEffectReservation, NativeGitDispatch), String> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let (ledger, _) = load::<kiln_core::implementation::WorkLedger>(&transaction, WORK_KIND)?;
        let (attempts, _) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
        let stored_attempt = attempts
            .get(identity)
            .ok_or("native Git attempt is not retained")?;
        ensure_attempt_work(stored_attempt, work)?;
        let plan_key = format!("{PLAN_PREFIX}{}", identity.as_str());
        let (plan_row, _) = load::<Option<(NativeGitPlan, ContentId)>>(&transaction, &plan_key)?;
        let (retained, retained_identity) = plan_row.ok_or("native Git plan was not prepared")?;
        if retained_identity != *plan_identity
            || retained.attempt != *identity
            || retained.fence != fence
        {
            return Err("native Git plan identity or fence changed".into());
        }
        let key = effect_key(identity, kind);
        let (existing, effect_version) =
            load::<Option<NativeGitEffectReservation>>(&transaction, &key)?;
        if let Some(existing) = existing {
            if existing.plan != retained
                || existing.plan_identity != *plan_identity
                || existing.operation != *kind.operation(&retained)
            {
                return Err("native Git effect conflicts with retained operation".into());
            }
            // Existing effects are read-only on every restart, even after the
            // current work, fence, policy or review has gone stale.
            transaction.commit().map_err(text)?;
            return Ok((existing, NativeGitDispatch::ReconcileOnly));
        }
        ensure_current_work(&ledger, work)?;
        attempts
            .authorize_publication(identity, fence)
            .map_err(show)?;
        let (current_review, current_checks) = current_review_identities(review, work, &retained)?;
        if current_policy_identity(&transaction)? != retained.policy
            || current_review != retained.review
            || current_checks != retained.checks
        {
            return Err("native Git authority, policy, review or checks are stale".into());
        }
        if kind == NativeGitEffectKind::TargetIntegration {
            let (branch, _) = load::<Option<NativeGitEffectReservation>>(
                &transaction,
                &effect_key(identity, NativeGitEffectKind::BranchPublication),
            )?;
            let branch = branch.ok_or("native Git branch publication is not confirmed")?;
            if !matches!(branch.observed, Some(NativeGitObservation::Branch { ref remote, ref branch_ref, ref commit })
                if remote == &retained.remote_identity && branch_ref == &retained.branch_ref && commit == &retained.candidate)
            {
                return Err(
                    "native Git branch publication does not match the exact candidate".into(),
                );
            }
        }
        let reservation = NativeGitEffectReservation {
            operation: kind.operation(&retained).clone(),
            plan_identity: retained_identity,
            plan: retained,
            kind,
            observed: None,
            receipt: None,
        };
        save(
            &transaction,
            &key,
            effect_version,
            &Some(reservation.clone()),
        )?;
        transaction.commit().map_err(text)?;
        Ok((reservation, NativeGitDispatch::FirstDispatch))
    }

    fn acknowledge_native_git_effect(
        &mut self,
        work: &ImplementationWork,
        identity: &ContentId,
        fence: u64,
        reservation: &NativeGitEffectReservation,
        observed: NativeGitObservation,
        review: &NativeGitReviewAuthority,
    ) -> Result<NativeGitEffectReservation, String> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let (ledger, _) = load::<kiln_core::implementation::WorkLedger>(&transaction, WORK_KIND)?;
        let (attempts, _) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
        let attempt = attempts
            .get(identity)
            .ok_or("native Git attempt is not retained")?;
        ensure_attempt_work(attempt, work)?;
        let authority_current = ensure_current_work(&ledger, work).is_ok()
            && attempts.authorize_publication(identity, fence).is_ok()
            && attempt.fence == fence;
        let key = effect_key(identity, reservation.kind);
        let (retained, effect_version) =
            load::<Option<NativeGitEffectReservation>>(&transaction, &key)?;
        let mut retained = retained.ok_or("native Git effect was not reserved")?;
        if retained.operation != reservation.operation
            || retained.plan_identity != reservation.plan_identity
            || retained.plan != reservation.plan
            || retained.kind != reservation.kind
        {
            return Err("native Git observation differs from durable intent".into());
        }
        if let Some(old) = &retained.observed
            && old != &observed
            && !matches!(old, NativeGitObservation::Unknown { .. })
        {
            return Err("native Git operation has a conflicting provider observation".into());
        }
        if retained.observed.as_ref() != Some(&observed) {
            validate_native_observation(&retained, &observed)?;
            retained.observed = Some(observed.clone());
        }
        // A remote observation is retained even when current policy/review was
        // revoked, but it cannot become Kiln acceptance until currentness returns.
        let current_evidence = current_review_identities(review, work, &retained.plan).ok();
        if authority_current
            && current_policy_identity(&transaction).ok().as_ref() == Some(&retained.plan.policy)
            && current_evidence
                .as_ref()
                .is_some_and(|(review_id, checks_id)| {
                    review_id == &retained.plan.review && checks_id == &retained.plan.checks
                })
            && retained.kind == NativeGitEffectKind::TargetIntegration
        {
            let (branch, _) = load::<Option<NativeGitEffectReservation>>(
                &transaction,
                &effect_key(identity, NativeGitEffectKind::BranchPublication),
            )?;
            let branch = branch.ok_or("native Git branch publication is not confirmed")?;
            if matches!(branch.observed, Some(NativeGitObservation::Branch { ref remote, ref branch_ref, ref commit })
                if remote == &retained.plan.remote_identity && branch_ref == &retained.plan.branch_ref && commit == &retained.plan.candidate)
                && matches!(retained.observed.as_ref(), Some(NativeGitObservation::Target { remote, target_ref, previous, resulting })
                    if remote == &retained.plan.remote_identity && target_ref == &retained.plan.target_ref
                        && previous == &retained.plan.expected_old_target && resulting == &retained.plan.candidate)
            {
                let receipt = receipt_for(&retained);
                kiln_core::implementation::admit_native_git_receipt(
                    work,
                    attempt,
                    &retained.plan,
                    &retained.plan_identity,
                    &receipt,
                    kiln_core::implementation::NativeGitCurrentness {
                        authority: true,
                        policy: true,
                        review: true,
                    },
                )
                .map_err(show)?;
                retained.receipt = Some(receipt);
            }
        }
        save(&transaction, &key, effect_version, &Some(retained.clone()))?;
        transaction.commit().map_err(text)?;
        Ok(retained)
    }

    /// Bind #47's producer-owned lifecycle observation to an independently
    /// confirmed native integration, without consulting or manufacturing PR data.
    pub fn record_native_cairn_lifecycle(
        &mut self,
        work: &ImplementationWork,
        identity: &ContentId,
        fence: u64,
        observation: &crate::publication_effects::CairnLifecycleObservation,
        review: &NativeGitReviewAuthority,
    ) -> Result<NativeCairnLifecycleReceipt, String> {
        let now_ms = self.now_ms();
        let (work_facts, attempt_facts) = self.authority_facts(identity, fence)?;
        let request = super::authority_bridge::request_for(
            kiln_core::implementation::authority::ProtectedOperation::LifecycleComplete,
            &work_facts,
            &attempt_facts,
            &[
                observation.binding.authoritative_completion.as_str(),
                observation.binding.handoff.as_str(),
                observation.binding.verification.as_str(),
                observation.observed_tree.native_id.as_str(),
            ],
            Some(&observation.binding.candidate),
        )?;
        let currentness = self.attempt_currentness(identity, fence, now_ms);
        let session = self.admit_authority(&request, currentness, now_ms)?;
        self.revalidate_authority(session, &request, currentness, now_ms)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(text)?;
        let (ledger, _) = load::<kiln_core::implementation::WorkLedger>(&transaction, WORK_KIND)?;
        let (attempts, _) = load::<AttemptLedger>(&transaction, ATTEMPT_KIND)?;
        ensure_current_work(&ledger, work)?;
        let attempt = attempts
            .authorize_publication(identity, fence)
            .map_err(show)?;
        ensure_attempt_work(attempt, work)?;
        let (effect, _) = load::<Option<NativeGitEffectReservation>>(
            &transaction,
            &effect_key(identity, NativeGitEffectKind::TargetIntegration),
        )?;
        let effect = effect.ok_or("native Git target integration is not retained")?;
        let integration = effect
            .receipt
            .as_ref()
            .ok_or("native Git target read-back is not accepted")?;
        let (review_identity, checks_identity) =
            current_review_identities(review, work, &effect.plan)?;
        if current_policy_identity(&transaction)? != integration.policy
            || review_identity != integration.review
            || checks_identity != integration.checks
            || observation.lifecycle_policy != integration.policy
        {
            return Err(
                "native Git lifecycle authority, policy, review or checks are stale".into(),
            );
        }
        kiln_core::implementation::admit_native_git_receipt(
            work,
            attempt,
            &effect.plan,
            &effect.plan_identity,
            integration,
            kiln_core::implementation::NativeGitCurrentness {
                authority: true,
                policy: true,
                review: true,
            },
        )
        .map_err(show)?;
        let verification = attempt
            .verification
            .as_ref()
            .ok_or("Cairn lifecycle requires retained verification")?;
        let mut completion_checks = verification
            .run
            .checks
            .iter()
            .filter(|check| check.name == "cairn-authoritative-completion");
        let completion = completion_checks.next();
        if work.terminal_policy != kiln_core::implementation::TerminalPolicy::VerifiedIntegrated
            || observation.binding.source.kind
                != kiln_core::implementation::WorkSourceKind::CairnTask
            || !work.aliases.contains(&observation.binding.source)
            || observation.binding.handoff != integration.handoff
            || observation.binding.verification != integration.verification
            || observation.binding.candidate != integration.candidate
            || verification.tree != integration.tree
            || observation.observed_tree != integration.tree
            || completion_checks.next().is_some()
            || completion.is_none_or(|check| {
                check.outcome != "pass"
                    || check.kind != "cairn-agent-completion-v2"
                    || check.artifact_digest.as_deref()
                        != Some(observation.binding.authoritative_completion.as_str())
            })
        {
            return Err("Cairn lifecycle result does not bind the exact native Git candidate and authoritative completion".into());
        }
        observation.lifecycle_receipt.validate().map_err(show)?;
        let receipt = NativeCairnLifecycleReceipt {
            schema: NATIVE_CAIRN_LIFECYCLE_SCHEMA.into(),
            integration: native_receipt_identity(integration)?,
            work_identity: work.identity().map_err(show)?,
            work_revision: work.revision,
            attempt: identity.clone(),
            fence,
            binding: observation.binding.clone(),
            observed_tree: observation.observed_tree.clone(),
            lifecycle_policy: observation.lifecycle_policy.clone(),
            lifecycle_receipt: observation.lifecycle_receipt.clone(),
        };
        let key = format!("native-git-lifecycle:{}", identity.as_str());
        let (existing, version) = load::<Option<NativeCairnLifecycleReceipt>>(&transaction, &key)?;
        if let Some(existing) = existing {
            if existing != receipt {
                return Err("a different native Cairn lifecycle result is already retained".into());
            }
        } else {
            save(&transaction, &key, version, &Some(receipt.clone()))?;
        }
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
        Ok(receipt)
    }

    pub fn native_cairn_lifecycle(
        &mut self,
        identity: &ContentId,
    ) -> Result<Option<NativeCairnLifecycleReceipt>, String> {
        let transaction = self.connection.transaction().map_err(text)?;
        let (receipt, _) = load::<Option<NativeCairnLifecycleReceipt>>(
            &transaction,
            &format!("native-git-lifecycle:{}", identity.as_str()),
        )?;
        transaction.commit().map_err(text)?;
        Ok(receipt)
    }

    /// Read a retained native-Git plan without preparing or dispatching it.
    pub fn native_git_plan(
        &mut self,
        identity: &ContentId,
    ) -> Result<Option<(NativeGitPlan, ContentId)>, String> {
        let transaction = self.connection.transaction().map_err(text)?;
        let (value, _) = load::<Option<(NativeGitPlan, ContentId)>>(
            &transaction,
            &format!("{PLAN_PREFIX}{}", identity.as_str()),
        )?;
        transaction.commit().map_err(text)?;
        Ok(value)
    }

    /// Read one durable native-Git effect intent for restart gating.
    pub fn native_git_effect_reservation(
        &mut self,
        identity: &ContentId,
        kind: NativeGitEffectKind,
    ) -> Result<Option<NativeGitEffectReservation>, String> {
        let transaction = self.connection.transaction().map_err(text)?;
        let (value, _) =
            load::<Option<NativeGitEffectReservation>>(&transaction, &effect_key(identity, kind))?;
        transaction.commit().map_err(text)?;
        Ok(value)
    }

    pub fn native_git_receipt(
        &mut self,
        identity: &ContentId,
    ) -> Result<Option<NativeGitReceipt>, String> {
        let transaction = self.connection.transaction().map_err(text)?;
        let (value, _) = load::<Option<NativeGitEffectReservation>>(
            &transaction,
            &effect_key(identity, NativeGitEffectKind::TargetIntegration),
        )?;
        let value = value.and_then(|effect| effect.receipt);
        transaction.commit().map_err(text)?;
        Ok(value)
    }
}

fn same_native_git_request(request: &NativeGitPlan, retained: &NativeGitPlan) -> bool {
    request.schema == retained.schema
        && request.work_identity == retained.work_identity
        && request.work_revision == retained.work_revision
        && request.attempt == retained.attempt
        && request.fence == retained.fence
        && request.handoff == retained.handoff
        && request.verification == retained.verification
        && request.policy == retained.policy
        && request.repository == retained.repository
        && request.remote_identity == retained.remote_identity
        && request.provider_profile == retained.provider_profile
        && request.branch_expected_old == retained.branch_expected_old
        && request.branch_ref == retained.branch_ref
        && request.target_ref == retained.target_ref
        && request.expected_old_target == retained.expected_old_target
        && request.candidate == retained.candidate
        && request.tree == retained.tree
}

fn current_review_identities(
    authority: &NativeGitReviewAuthority,
    work: &ImplementationWork,
    plan: &NativeGitPlan,
) -> Result<(ContentId, ContentId), String> {
    if authority.actor != work.actor {
        return Err("native Git review actor differs from admitted work".into());
    }
    kiln_core::change_review::validate_candidate(
        &authority.state,
        &authority.change_id,
        &authority.revision_id,
        &authority.candidate_id,
        &authority.actor,
        &authority.facts,
    )
    .map_err(|error| format!("current review admission failed: {}", error.message))?;
    let change = authority
        .state
        .changes
        .get(&authority.change_id)
        .ok_or("current review change is missing")?;
    let candidate = change
        .candidate
        .as_ref()
        .ok_or("current review candidate is absent")?;
    let source = authority
        .facts
        .source
        .as_ref()
        .ok_or("fresh current target observation is absent")?;
    if change.repository != work.repository
        || source.repository != work.repository
        || candidate.commit != plan.candidate.native_id.as_str()
        || candidate.target_ref != plan.target_ref
        || candidate.target != plan.expected_old_target.native_id.as_str()
        || source.target_ref != plan.target_ref
        || source.current_target != plan.expected_old_target.native_id.as_str()
    {
        return Err("current review candidate or target differs from native Git plan".into());
    }
    let checklist = kiln_core::change_review::checklist(
        &authority.state,
        &authority.change_id,
        &authority.facts,
    )
    .map_err(|error| format!("current review checklist failed: {}", error.message))?;
    if !checklist.ready {
        return Err("current review checklist is not ready".into());
    }
    let mut stable_facts = authority.facts.clone();
    stable_facts.now_ms = 0;
    stable_facts.event_id.clear();
    let review_bytes = serde_json::to_vec(&(
        &authority.state,
        &stable_facts,
        &authority.profile_identity,
        &authority.source_identity,
        &authority.ci_identity,
        &authority.change_id,
        &authority.revision_id,
        &authority.candidate_id,
        &authority.actor,
    ))
    .map_err(text)?;
    let checks_bytes = serde_json::to_vec(&(
        &plan.verification,
        &checklist,
        &authority.facts.ci,
        &authority.facts.ci_required,
        &authority.facts.integration,
        &authority.profile_identity,
        &authority.source_identity,
        &authority.ci_identity,
    ))
    .map_err(text)?;
    Ok((
        ContentId::from_blake3_hex(blake3::hash(&review_bytes).to_hex().to_string())
            .map_err(show)?,
        ContentId::from_blake3_hex(blake3::hash(&checks_bytes).to_hex().to_string())
            .map_err(show)?,
    ))
}

fn current_policy_identity(transaction: &Transaction<'_>) -> Result<ContentId, String> {
    let (ledger, _) = load::<AdmissionLedger>(transaction, POLICY_KIND)?;
    ledger
        .policy
        .ok_or("no current admitted policy")?
        .identity()
        .map_err(show)
}

fn ensure_current_work(
    ledger: &kiln_core::implementation::WorkLedger,
    work: &ImplementationWork,
) -> Result<(), String> {
    if ledger.current().any(|candidate| candidate == work) {
        Ok(())
    } else {
        Err("native Git work is no longer current".into())
    }
}

fn ensure_attempt_work(
    attempt: &kiln_core::implementation::ImplementationAttempt,
    work: &ImplementationWork,
) -> Result<(), String> {
    if attempt.work_identity == work.identity().map_err(show)?
        && attempt.work_revision == work.revision
    {
        Ok(())
    } else {
        Err("native Git attempt does not own current work".into())
    }
}

fn effect_key(identity: &ContentId, kind: NativeGitEffectKind) -> String {
    format!("{EFFECT_PREFIX}{}:{}", identity.as_str(), kind.key())
}

fn validate_native_observation(
    reservation: &NativeGitEffectReservation,
    observed: &NativeGitObservation,
) -> Result<(), String> {
    let plan = &reservation.plan;
    match (reservation.kind, observed) {
        (
            NativeGitEffectKind::BranchPublication,
            NativeGitObservation::Branch {
                remote,
                branch_ref,
                commit,
            },
        ) if remote == &plan.remote_identity
            && branch_ref == &plan.branch_ref
            && commit == &plan.candidate =>
        {
            Ok(())
        }
        (
            NativeGitEffectKind::TargetIntegration,
            NativeGitObservation::Target {
                remote,
                target_ref,
                previous,
                resulting,
            },
        ) if remote == &plan.remote_identity
            && target_ref == &plan.target_ref
            && previous == &plan.expected_old_target
            && resulting == &plan.candidate =>
        {
            Ok(())
        }
        (
            _,
            NativeGitObservation::Unknown { operation, .. }
            | NativeGitObservation::Rejected { operation, .. },
        ) if operation == &reservation.operation => Ok(()),
        _ => Err("native Git provider observation does not match the exact operation".into()),
    }
}

fn receipt_for(reservation: &NativeGitEffectReservation) -> NativeGitReceipt {
    let plan = &reservation.plan;
    NativeGitReceipt {
        schema: NATIVE_GIT_RECEIPT_SCHEMA.into(),
        plan: reservation.plan_identity.clone(),
        work_identity: plan.work_identity.clone(),
        work_revision: plan.work_revision,
        attempt: plan.attempt.clone(),
        fence: plan.fence,
        handoff: plan.handoff.clone(),
        verification: plan.verification.clone(),
        review: plan.review.clone(),
        checks: plan.checks.clone(),
        policy: plan.policy.clone(),
        repository: plan.repository.clone(),
        remote_identity: plan.remote_identity.clone(),
        provider_profile: plan.provider_profile.clone(),
        branch_expected_old: plan.branch_expected_old.clone(),
        branch_ref: plan.branch_ref.clone(),
        target_ref: plan.target_ref.clone(),
        expected_old_target: plan.expected_old_target.clone(),
        candidate: plan.candidate.clone(),
        tree: plan.tree.clone(),
        branch_operation: plan.branch_operation.clone(),
        integration_operation: plan.integration_operation.clone(),
        resulting_commit: plan.candidate.clone(),
    }
}

fn native_receipt_identity(receipt: &NativeGitReceipt) -> Result<ContentId, String> {
    let bytes = serde_json::to_vec(receipt).map_err(text)?;
    ContentId::from_blake3_hex(blake3::hash(&bytes).to_hex().to_string()).map_err(show)
}

fn show(error: kiln_core::types::Diagnostic) -> String {
    format!("{}: {}", error.code, error.message)
}

#[cfg(test)]
mod tests;
