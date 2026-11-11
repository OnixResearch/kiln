//! Deterministic Kiln-owned implementation-attempt state machine.
//!
//! Kiln owns the global claim, monotonic fence, attempt budget, supersession and
//! disposition. Execution, workflow-node cancellation and workspace lifecycle
//! belong to a provider (Lattice). This module is pure: callers persist the
//! serializable ledger through the existing journal/outbox boundary and deliver
//! provider observations back with the fence.
//!
//! Work identity, work version, attempt identity, workflow run and candidate
//! identity remain distinct. A stale fence is never admitted, even if the old
//! process still runs.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::ImplementationWork;
use crate::types::{ContentId, CoreResult, Diagnostic, EffectId, ExternalRef, ProviderProfileId};

pub const ATTEMPT_SCHEMA: &str = "kiln.implementation-attempt.v1";
pub const DISPATCH_SCHEMA: &str = "kiln.implementation-dispatch.v1";
pub const MAX_LEASE_MS: u64 = 24 * 60 * 60 * 1_000;
pub const DEFAULT_LEASE_MS: u64 = 60 * 60 * 1_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptPhase {
    Claimed,
    Executing,
    CandidateProduced,
    Verifying,
    Verified,
    Blocked,
    Failed,
    Cancelled,
    Superseded,
}

impl AttemptPhase {
    #[must_use]
    pub const fn is_active(self) -> bool {
        matches!(
            self,
            Self::Claimed | Self::Executing | Self::CandidateProduced | Self::Verifying
        )
    }

    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Verified | Self::Blocked | Self::Failed | Self::Cancelled | Self::Superseded
        )
    }
}

/// Implementation failure is distinct from provider/infrastructure failure and
/// from an ambiguous outcome that must be reconciled before retry.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    Implementation,
    Infrastructure,
    Unknown,
}

/// What happens to an active attempt when its work source version changes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupersessionRule {
    /// Freeze and cancel the active attempt; its fence is revoked immediately.
    AbortActive,
    /// The active attempt keeps its frozen contract and may finish, but no new
    /// attempt uses the superseded work version.
    FinishActive,
}

/// Durable intent to dispatch one admitted attempt exactly once.
///
/// The reservation is created before provider transport. Repeating the same
/// reservation is idempotent; a different plan or profile for the same live
/// attempt is refused. The effect and idempotency identities are deterministic
/// over the attempt, fence, plan and provider profile.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DispatchReservation {
    pub schema: String,
    pub provider_profile: ProviderProfileId,
    /// Caller-admitted provider plan identity.
    pub plan: ContentId,
    pub effect_id: EffectId,
    pub idempotency_key: ExternalRef,
    pub reserved_at_ms: u64,
}

impl DispatchReservation {
    fn new(
        attempt: &ImplementationAttempt,
        plan: &ContentId,
        provider_profile: &ProviderProfileId,
        now_ms: u64,
    ) -> CoreResult<Self> {
        let mut bytes = Vec::new();
        push(&mut bytes, DISPATCH_SCHEMA)?;
        push(&mut bytes, attempt.identity.as_str())?;
        bytes.extend_from_slice(&attempt.fence.to_be_bytes());
        push(&mut bytes, plan.as_str())?;
        push(&mut bytes, provider_profile.as_str())?;
        let digest = ContentId::from_blake3_hex(blake3::hash(&bytes).to_hex().to_string())?;
        Ok(Self {
            schema: DISPATCH_SCHEMA.to_string(),
            provider_profile: provider_profile.clone(),
            plan: plan.clone(),
            effect_id: EffectId::new(format!("dispatch-{}", digest.as_str()))?,
            idempotency_key: ExternalRef::new("kiln", "implementation-dispatch", digest.as_str())?,
            reserved_at_ms: now_ms,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplementationAttempt {
    pub schema: String,
    /// Deterministic attempt identity over the canonical work key, ordinal and
    /// fence. Never reused.
    pub identity: ContentId,
    pub canonical_key: ContentId,
    pub work_identity: ContentId,
    pub work_revision: u64,
    /// Exact base frozen when the claim was admitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<ExternalRef>,
    pub ordinal: u8,
    /// Monotonic per work unit; the only accepted authority for results.
    pub fence: u64,
    pub lease_until_ms: u64,
    /// Host-clock claim time, preserved across restart.
    #[serde(default)]
    pub claimed_at_ms: u64,
    /// Host-clock terminal time, absent while the attempt is active.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at_ms: Option<u64>,
    pub phase: AttemptPhase,
    /// Dispatch reservation recorded before provider transport.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dispatch: Option<DispatchReservation>,
    pub workflow: Option<ExternalRef>,
    pub candidate: Option<ExternalRef>,
    /// Complete candidate handoff admitted from host-derived facts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handoff: Option<super::CandidateHandoff>,
    /// Admitted independent verification evidence for the candidate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification: Option<super::CandidateVerification>,
    /// Prepared publication and remote branch/PR identities, once verified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publication: Option<super::PublicationRecord>,
    /// Opt-in metrics receipt; absent unless explicitly recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metrics: Option<super::AttemptMetrics>,
    pub failure: Option<FailureClass>,
    pub cause: Option<String>,
    /// Newer work revision that superseded this attempt, if any.
    pub superseded_by: Option<u64>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttemptLedger {
    fence_counter: u64,
    attempts: BTreeMap<String, ImplementationAttempt>,
    /// Canonical work key to the active attempt identity.
    current: BTreeMap<String, String>,
    /// Canonical work key to consumed attempt budget across lineage.
    consumed: BTreeMap<String, u8>,
}

impl AttemptLedger {
    pub fn attempts(&self) -> impl Iterator<Item = &ImplementationAttempt> {
        self.attempts.values()
    }

    pub fn get(&self, identity: &ContentId) -> Option<&ImplementationAttempt> {
        self.attempts.get(identity.as_str())
    }

    /// Canonical work key currently holding the claim, if any.
    pub fn current(&self, canonical_key: &ContentId) -> Option<&ImplementationAttempt> {
        let identity = self.current.get(canonical_key.as_str())?;
        self.attempts.get(identity)
    }

    /// Claim eligible work exactly once. A second scheduler receives no claim.
    ///
    /// A claim is admitted only when the work is complete, no lease is active,
    /// and the lineage still has attempt budget. The caller freezes the base
    /// revision in provider reservation; this ledger freezes the work revision
    /// and identity.
    // r[impl kiln.implementation.claim] r[impl kiln.implementation.fencing]
    pub fn claim(
        &mut self,
        work: &ImplementationWork,
        now_ms: u64,
        lease_ms: u64,
    ) -> CoreResult<ImplementationAttempt> {
        if lease_ms == 0 || lease_ms > MAX_LEASE_MS {
            return Err(refusal(
                "implementation_lease_bound",
                "attempt.lease_ms",
                "attempt lease is zero or exceeds its bound",
            ));
        }
        let canonical_key = work.canonical_key()?;
        let work_identity = work.identity()?;
        if let Some(active) = self.current(&canonical_key)
            && active.phase.is_active()
            && active.lease_until_ms > now_ms
        {
            return Err(refusal(
                "implementation_already_claimed",
                "attempt.claim",
                "work already has a current implementation claim",
            ));
        }
        let consumed = self
            .consumed
            .get(canonical_key.as_str())
            .copied()
            .unwrap_or(0);
        if consumed >= work.budget.attempts {
            return Err(refusal(
                "implementation_budget_exhausted",
                "attempt.budget",
                "the work lineage exhausted its reviewed attempt budget",
            ));
        }
        let ordinal = consumed.checked_add(1).ok_or_else(|| {
            refusal(
                "implementation_attempt_ordinal",
                "attempt.ordinal",
                "attempt ordinal is exhausted",
            )
        })?;
        self.fence_counter = self.fence_counter.checked_add(1).ok_or_else(|| {
            refusal(
                "implementation_fence_exhausted",
                "attempt.fence",
                "the fence counter is exhausted",
            )
        })?;
        let fence = self.fence_counter;
        let identity = attempt_identity(&canonical_key, ordinal, fence)?;
        let attempt = ImplementationAttempt {
            schema: ATTEMPT_SCHEMA.to_string(),
            identity: identity.clone(),
            canonical_key: canonical_key.clone(),
            work_identity,
            work_revision: work.revision,
            base: work.base.clone(),
            ordinal,
            fence,
            lease_until_ms: now_ms.saturating_add(lease_ms),
            claimed_at_ms: now_ms,
            ended_at_ms: None,
            phase: AttemptPhase::Claimed,
            dispatch: None,
            workflow: None,
            candidate: None,
            handoff: None,
            verification: None,
            publication: None,
            metrics: None,
            failure: None,
            cause: None,
            superseded_by: None,
        };
        self.consumed
            .insert(canonical_key.as_str().to_string(), ordinal);
        self.current.insert(
            canonical_key.as_str().to_string(),
            identity.as_str().to_string(),
        );
        self.attempts
            .insert(identity.as_str().to_string(), attempt.clone());
        Ok(attempt)
    }

    /// Extend a live claim. An expired or stale fence is refused.
    // r[impl kiln.implementation.fencing]
    pub fn renew(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        lease_ms: u64,
    ) -> CoreResult<u64> {
        if lease_ms == 0 || lease_ms > MAX_LEASE_MS {
            return Err(refusal(
                "implementation_lease_bound",
                "attempt.lease_ms",
                "attempt lease is zero or exceeds its bound",
            ));
        }
        let attempt = self.authorized(identity, fence, now_ms)?;
        if !attempt.phase.is_active() {
            return Err(refusal(
                "implementation_attempt_terminal",
                "attempt.phase",
                "a terminal attempt cannot renew its lease",
            ));
        }
        let lease_until_ms = now_ms.saturating_add(lease_ms);
        let attempt = self.attempt_mut(identity)?;
        attempt.lease_until_ms = lease_until_ms;
        Ok(lease_until_ms)
    }

    /// Reserve one provider dispatch before transport.
    ///
    /// The same plan and profile are idempotent. A different plan or profile
    /// for the current attempt, a stale fence, or an expired lease is refused.
    // r[impl kiln.implementation.dispatch_reservation]
    pub fn reserve_dispatch(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        plan: &ContentId,
        provider_profile: &ProviderProfileId,
    ) -> CoreResult<DispatchReservation> {
        self.require_phase(identity, fence, now_ms, &[AttemptPhase::Claimed])?;
        let attempt = self.attempt_mut(identity)?;
        if let Some(existing) = &attempt.dispatch {
            if &existing.plan == plan && &existing.provider_profile == provider_profile {
                return Ok(existing.clone());
            }
            return Err(refusal(
                "implementation_dispatch_reserved",
                "attempt.dispatch",
                "attempt already reserved a different provider dispatch",
            ));
        }
        let reservation = DispatchReservation::new(attempt, plan, provider_profile, now_ms)?;
        attempt.dispatch = Some(reservation.clone());
        Ok(reservation)
    }

    /// Admit and reserve one bounded dispatch plan against this attempt.
    ///
    /// The plan identity is computed from the exact work/attempt bindings and
    /// the admitted path, effect, control and budget facts before reservation.
    // r[impl kiln.implementation.dispatch_plan] r[impl kiln.implementation.dispatch_reservation]
    pub fn reserve_dispatch_plan(
        &mut self,
        work: &ImplementationWork,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        plan: &super::ImplementationDispatchPlan,
    ) -> CoreResult<DispatchReservation> {
        let digest = {
            let attempt = self.authorized(identity, fence, now_ms)?;
            super::admit_dispatch_plan(work, attempt, plan)?
        };
        self.reserve_dispatch(identity, fence, now_ms, &digest, &plan.provider_profile)
    }

    /// Opt-in observation of one attempt without retaining it.
    ///
    /// The caller decides whether to record the receipt. Observation never
    /// gates admission, verification or publication.
    // r[impl kiln.implementation.metrics]
    pub fn observe_metrics(
        &self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        usage: super::AttemptUsage,
        traces: Vec<ExternalRef>,
    ) -> CoreResult<super::AttemptMetrics> {
        let attempt = self.get(identity).ok_or_else(|| {
            refusal(
                "implementation_attempt_missing",
                "attempt.identity",
                "attempt identity is not retained",
            )
        })?;
        if attempt.fence != fence {
            return Err(refusal(
                "implementation_fence_stale",
                "attempt.fence",
                "attempt fence is stale",
            ));
        }
        super::AttemptMetrics::observe(attempt, now_ms, usage, traces)
    }

    /// Record the first opt-in metrics receipt for an attempt.
    ///
    /// Repeating the call returns the retained receipt; it never overwrites
    /// history or changes attempt state.
    // r[impl kiln.implementation.metrics]
    pub fn record_metrics(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        usage: super::AttemptUsage,
        traces: Vec<ExternalRef>,
    ) -> CoreResult<super::AttemptMetrics> {
        if let Some(existing) = self
            .get(identity)
            .and_then(|attempt| attempt.metrics.clone())
        {
            if existing.fence != fence {
                return Err(refusal(
                    "implementation_fence_stale",
                    "attempt.fence",
                    "attempt fence is stale",
                ));
            }
            return Ok(existing);
        }
        let metrics = self.observe_metrics(identity, fence, now_ms, usage, traces)?;
        let attempt = self.attempt_mut(identity)?;
        attempt.metrics = Some(metrics.clone());
        Ok(metrics)
    }

    /// Read the retained metrics receipt for one attempt.
    #[must_use]
    pub fn metrics(&self, identity: &ContentId) -> Option<&super::AttemptMetrics> {
        self.get(identity)
            .and_then(|attempt| attempt.metrics.as_ref())
    }

    /// Read the durable dispatch reservation for one attempt.
    #[must_use]
    pub fn dispatch_reservation(&self, identity: &ContentId) -> Option<&DispatchReservation> {
        self.get(identity)
            .and_then(|attempt| attempt.dispatch.as_ref())
    }

    /// Mark dispatch executed and record the provider workflow run.
    ///
    /// A reservation MUST exist first. Repeating the exact same observation is
    /// idempotent. A conflicting run identity is refused rather than silently
    /// replaced.
    // r[impl kiln.implementation.dispatch] r[impl kiln.implementation.dispatch_reservation]
    pub fn start_execution(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        workflow: &ExternalRef,
    ) -> CoreResult<()> {
        self.require_phase(identity, fence, now_ms, &[AttemptPhase::Claimed])?;
        workflow.validate()?;
        let attempt = self.attempt_mut(identity)?;
        if attempt.dispatch.is_none() {
            return Err(refusal(
                "implementation_dispatch_reservation_missing",
                "attempt.dispatch",
                "dispatch requires a durable reservation before transport",
            ));
        }
        match &attempt.workflow {
            Some(existing) if existing == workflow => return Ok(()),
            Some(_) => {
                return Err(refusal(
                    "implementation_dispatch_conflict",
                    "attempt.workflow",
                    "attempt already records a different workflow execution",
                ));
            }
            None => {}
        }
        attempt.workflow = Some(workflow.clone());
        attempt.phase = AttemptPhase::Executing;
        Ok(())
    }

    /// Reconcile an ambiguous dispatch without a blind duplicate.
    ///
    /// If no run was recorded, the observed run becomes the single execution.
    /// An equal observation is a no-op; a different one is refused.
    // r[impl kiln.implementation.dispatch]
    pub fn reconcile_dispatch(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        observed: &ExternalRef,
    ) -> CoreResult<()> {
        self.require_phase(
            identity,
            fence,
            now_ms,
            &[AttemptPhase::Claimed, AttemptPhase::Executing],
        )?;
        observed.validate()?;
        let attempt = self.attempt_mut(identity)?;
        if attempt.dispatch.is_none() {
            return Err(refusal(
                "implementation_dispatch_reservation_missing",
                "attempt.dispatch",
                "dispatch reconciliation requires a durable reservation",
            ));
        }
        match &attempt.workflow {
            None => {
                attempt.workflow = Some(observed.clone());
                attempt.phase = AttemptPhase::Executing;
                Ok(())
            }
            Some(existing) if existing == observed => Ok(()),
            Some(_) => Err(refusal(
                "implementation_dispatch_conflict",
                "attempt.workflow",
                "reconciliation observed a different workflow execution",
            )),
        }
    }

    /// Admit one complete candidate handoff and freeze it on the attempt.
    ///
    /// The exact work/version, attempt/fence, base, candidate/tree, paths,
    /// acceptance digest, verification profile and evidence are checked before
    /// the attempt becomes `candidate-produces`. Repeating the same handoff is
    /// idempotent; a different handoff is refused.
    // r[impl kiln.implementation.candidate_handoff]
    pub fn admit_candidate_handoff(
        &mut self,
        work: &ImplementationWork,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        handoff: &super::CandidateHandoff,
    ) -> CoreResult<ContentId> {
        let existing = self.authorized(identity, fence, now_ms)?;
        if let Some(existing) = &existing.handoff {
            if existing == handoff {
                let attempt = self.authorized(identity, fence, now_ms)?;
                return super::admit_candidate_handoff(work, attempt, handoff);
            }
            return Err(refusal(
                "implementation_candidate_handoff",
                "attempt.handoff",
                "attempt already retains a different candidate handoff",
            ));
        }
        if existing.phase != AttemptPhase::Executing {
            return Err(refusal(
                "implementation_attempt_phase",
                "attempt.phase",
                "attempt phase does not admit this transition",
            ));
        }
        let digest = {
            let attempt = self.authorized(identity, fence, now_ms)?;
            super::admit_candidate_handoff(work, attempt, handoff)?
        };
        let attempt = self.attempt_mut(identity)?;
        attempt.candidate = Some(handoff.candidate.clone());
        attempt.handoff = Some(handoff.clone());
        attempt.phase = AttemptPhase::CandidateProduced;
        Ok(digest)
    }

    /// Read the retained candidate handoff for one attempt.
    #[must_use]
    pub fn candidate_handoff(&self, identity: &ContentId) -> Option<&super::CandidateHandoff> {
        self.get(identity)
            .and_then(|attempt| attempt.handoff.as_ref())
    }

    /// Record exactly one produced candidate.
    // r[impl kiln.implementation.attempt_states]
    pub fn produce_candidate(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        candidate: &ExternalRef,
    ) -> CoreResult<()> {
        self.require_phase(identity, fence, now_ms, &[AttemptPhase::Executing])?;
        candidate.validate()?;
        let attempt = self.attempt_mut(identity)?;
        attempt.candidate = Some(candidate.clone());
        attempt.phase = AttemptPhase::CandidateProduced;
        Ok(())
    }

    /// Prepare one publication plan for a verified attempt.
    ///
    /// The plan binds the exact handoff, verification receipt, candidate and
    /// deterministic branch. Preparing does not perform a remote effect.
    // r[impl kiln.implementation.publication]
    pub fn prepare_publication(
        &mut self,
        work: &ImplementationWork,
        identity: &ContentId,
        fence: u64,
        handoff: &super::CandidateHandoff,
        verification: &super::CandidateVerification,
        plan: &super::PublicationPlan,
    ) -> CoreResult<ContentId> {
        let attempt = self.authorized_fence(identity, fence)?;
        if let Some(existing) = &attempt.publication {
            if &existing.plan == plan {
                return super::admit_publication_plan(work, attempt, handoff, verification, plan);
            }
            return Err(refusal(
                "implementation_publication_conflict",
                "attempt.publication",
                "attempt already retains a different publication plan",
            ));
        }
        let digest = super::admit_publication_plan(work, attempt, handoff, verification, plan)?;
        let attempt = self.attempt_mut(identity)?;
        attempt.publication = Some(super::PublicationRecord {
            schema: super::PUBLICATION_SCHEMA.to_string(),
            plan: plan.clone(),
            phase: super::PublicationPhase::Prepared,
            remote_head: None,
            pull_request: None,
            integration: None,
            source_status: None,
        });
        Ok(digest)
    }

    /// Record the observed candidate branch head.
    // r[impl kiln.implementation.publication]
    pub fn record_branch_published(
        &mut self,
        identity: &ContentId,
        fence: u64,
        remote_head: &ExternalRef,
    ) -> CoreResult<()> {
        let attempt = self.authorized_fence(identity, fence)?;
        let publication = attempt.publication.as_ref().ok_or_else(|| {
            refusal(
                "implementation_publication_missing",
                "attempt.publication",
                "branch publication requires a prepared publication plan",
            )
        })?;
        if publication.phase != super::PublicationPhase::Prepared {
            if publication.remote_head.as_ref() == Some(remote_head)
                && publication.phase == super::PublicationPhase::BranchPublished
            {
                return Ok(());
            }
            return Err(refusal(
                "implementation_publication_phase",
                "attempt.publication",
                "publication phase does not admit branch publication",
            ));
        }
        if remote_head != &publication.plan.candidate {
            return Err(refusal(
                "implementation_publication_head",
                "attempt.publication.remote_head",
                "remote branch head differs from the verified candidate",
            ));
        }
        let attempt = self.attempt_mut(identity)?;
        if let Some(publication) = &mut attempt.publication {
            publication.remote_head = Some(remote_head.clone());
            publication.phase = super::PublicationPhase::BranchPublished;
        }
        Ok(())
    }

    /// Record the observed pull request for the published branch.
    // r[impl kiln.implementation.publication]
    pub fn record_pull_request_published(
        &mut self,
        identity: &ContentId,
        fence: u64,
        pull_request: &ExternalRef,
        head: &ExternalRef,
    ) -> CoreResult<()> {
        let attempt = self.authorized_fence(identity, fence)?;
        let publication = attempt.publication.as_ref().ok_or_else(|| {
            refusal(
                "implementation_publication_missing",
                "attempt.publication",
                "pull request publication requires a prepared publication plan",
            )
        })?;
        if publication.phase == super::PublicationPhase::PullRequestPublished
            && publication.pull_request.as_ref() == Some(pull_request)
            && publication.remote_head.as_ref() == Some(head)
        {
            return Ok(());
        }
        if publication.phase != super::PublicationPhase::BranchPublished
            || publication.remote_head.as_ref() != Some(head)
            || head != &publication.plan.candidate
        {
            return Err(refusal(
                "implementation_publication_pr",
                "attempt.publication.pull_request",
                "pull request head or phase differs from the verified candidate branch",
            ));
        }
        pull_request.validate()?;
        let attempt = self.attempt_mut(identity)?;
        if let Some(publication) = &mut attempt.publication {
            publication.pull_request = Some(pull_request.clone());
            publication.phase = super::PublicationPhase::PullRequestPublished;
        }
        Ok(())
    }

    /// Read the retained publication record for one attempt.
    #[must_use]
    pub fn publication_record(&self, identity: &ContentId) -> Option<&super::PublicationRecord> {
        self.get(identity)
            .and_then(|attempt| attempt.publication.as_ref())
    }

    /// Record one independently observed integration for the published PR.
    // r[impl kiln.implementation.integration]
    pub fn record_integration(
        &mut self,
        work: &ImplementationWork,
        identity: &ContentId,
        fence: u64,
        receipt: &super::IntegrationReceipt,
    ) -> CoreResult<ContentId> {
        let attempt = self.authorized_fence(identity, fence)?;
        let record = attempt.publication.as_ref().ok_or_else(|| {
            refusal(
                "implementation_integration_missing",
                "attempt.publication",
                "integration requires a prepared publication",
            )
        })?;
        if record.phase == super::PublicationPhase::Integrated {
            if record.integration.as_ref() == Some(receipt) {
                return super::admit_integration(work, attempt, record, receipt);
            }
            return Err(refusal(
                "implementation_integration_conflict",
                "attempt.publication.integration",
                "attempt already retains different integration evidence",
            ));
        }
        let digest = super::admit_integration(work, attempt, record, receipt)?;
        let attempt = self.attempt_mut(identity)?;
        if let Some(publication) = &mut attempt.publication {
            publication.integration = Some(receipt.clone());
            publication.phase = super::PublicationPhase::Integrated;
        }
        Ok(digest)
    }

    /// Read the retained integration receipt.
    #[must_use]
    pub fn integration_receipt(&self, identity: &ContentId) -> Option<&super::IntegrationReceipt> {
        self.publication_record(identity)
            .and_then(|record| record.integration.as_ref())
    }

    /// Record the independently observed closed source state.
    // r[impl kiln.implementation.source_status]
    pub fn record_source_status(
        &mut self,
        work: &ImplementationWork,
        identity: &ContentId,
        fence: u64,
        plan: &super::SourceStatusPlan,
        receipt: &super::SourceStatusReceipt,
    ) -> CoreResult<ContentId> {
        let attempt = self.authorized_fence(identity, fence)?;
        let record = attempt.publication.as_ref().ok_or_else(|| {
            refusal(
                "implementation_source_status_publication",
                "attempt.publication",
                "source status requires a retained publication",
            )
        })?;
        if record.phase == super::PublicationPhase::SourceReconciled {
            if record.source_status.as_ref() == Some(receipt) {
                return super::admit_source_status_receipt(work, attempt, plan, receipt);
            }
            return Err(refusal(
                "implementation_source_status_conflict",
                "attempt.publication.source_status",
                "attempt already retains different source-status evidence",
            ));
        }
        if record.phase != super::PublicationPhase::Integrated {
            return Err(refusal(
                "implementation_source_status_phase",
                "attempt.publication",
                "source status requires an integrated publication",
            ));
        }
        let digest = super::admit_source_status_receipt(work, attempt, plan, receipt)?;
        let attempt = self.attempt_mut(identity)?;
        if let Some(publication) = &mut attempt.publication {
            publication.source_status = Some(receipt.clone());
            publication.phase = super::PublicationPhase::SourceReconciled;
        }
        Ok(digest)
    }

    /// Read the retained source-status receipt.
    #[must_use]
    pub fn source_status_receipt(
        &self,
        identity: &ContentId,
    ) -> Option<&super::SourceStatusReceipt> {
        self.publication_record(identity)
            .and_then(|record| record.source_status.as_ref())
    }

    /// Record independent verification evidence and complete the attempt.
    ///
    /// The receipt must bind the retained handoff and exact canonical CI run.
    /// Repeating an equal receipt is idempotent; a different receipt is refused.
    // r[impl kiln.implementation.verification]
    pub fn record_verification(
        &mut self,
        work: &ImplementationWork,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        handoff: &super::CandidateHandoff,
        verification: &super::CandidateVerification,
    ) -> CoreResult<ContentId> {
        let existing = self.authorized(identity, fence, now_ms)?;
        if let Some(existing) = &existing.verification {
            if existing == verification {
                let attempt = self.authorized(identity, fence, now_ms)?;
                return super::admit_candidate_verification(work, attempt, handoff, verification);
            }
            return Err(refusal(
                "implementation_verification_conflict",
                "attempt.verification",
                "attempt already retains different verification evidence",
            ));
        }
        self.require_phase(identity, fence, now_ms, &[AttemptPhase::Verifying])?;
        let stored = self.candidate_handoff(identity).ok_or_else(|| {
            refusal(
                "implementation_candidate_handoff_missing",
                "attempt.handoff",
                "verification requires the retained candidate handoff",
            )
        })?;
        if stored != handoff {
            return Err(refusal(
                "implementation_candidate_handoff",
                "attempt.handoff",
                "verification handoff differs from the retained handoff",
            ));
        }
        let digest = {
            let attempt = self.authorized(identity, fence, now_ms)?;
            super::admit_candidate_verification(work, attempt, handoff, verification)?
        };
        let canonical_key = {
            let attempt = self.attempt_mut(identity)?;
            attempt.verification = Some(verification.clone());
            attempt.phase = AttemptPhase::Verified;
            attempt.ended_at_ms = Some(now_ms);
            attempt.canonical_key.clone()
        };
        self.clear_current(&canonical_key);
        Ok(digest)
    }

    /// Read the retained verification evidence for one attempt.
    #[must_use]
    pub fn verification_receipt(
        &self,
        identity: &ContentId,
    ) -> Option<&super::CandidateVerification> {
        self.get(identity)
            .and_then(|attempt| attempt.verification.as_ref())
    }

    /// Begin independent verification of the produced candidate.
    // r[impl kiln.implementation.attempt_states]
    pub fn begin_verification(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
    ) -> CoreResult<()> {
        self.require_phase(identity, fence, now_ms, &[AttemptPhase::CandidateProduced])?;
        let attempt = self.attempt_mut(identity)?;
        attempt.phase = AttemptPhase::Verifying;
        Ok(())
    }

    /// Record a structured implementation or infrastructure failure.
    // r[impl kiln.implementation.attempt_states]
    pub fn fail(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        class: FailureClass,
        cause: &str,
    ) -> CoreResult<()> {
        self.require_active(identity, fence, now_ms)?;
        let canonical_key = {
            let attempt = self.attempt_mut(identity)?;
            attempt.phase = AttemptPhase::Failed;
            attempt.ended_at_ms = Some(now_ms);
            attempt.failure = Some(class);
            attempt.cause = Some(bounded_cause(cause)?);
            attempt.canonical_key.clone()
        };
        self.clear_current(&canonical_key);
        Ok(())
    }

    /// Record a blocked disposition with a structured cause. A blocked attempt
    /// consumes its budget and never produces a candidate.
    // r[impl kiln.implementation.attempt_states]
    pub fn block(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        cause: &str,
    ) -> CoreResult<()> {
        self.require_active(identity, fence, now_ms)?;
        let canonical_key = {
            let attempt = self.attempt_mut(identity)?;
            attempt.phase = AttemptPhase::Blocked;
            attempt.ended_at_ms = Some(now_ms);
            attempt.cause = Some(bounded_cause(cause)?);
            attempt.canonical_key.clone()
        };
        self.clear_current(&canonical_key);
        Ok(())
    }

    /// Cancel the active attempt. The fence is revoked immediately.
    // r[impl kiln.implementation.attempt_states]
    pub fn cancel(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        cause: &str,
    ) -> CoreResult<()> {
        self.require_active(identity, fence, now_ms)?;
        let canonical_key = {
            let attempt = self.attempt_mut(identity)?;
            attempt.phase = AttemptPhase::Cancelled;
            attempt.ended_at_ms = Some(now_ms);
            attempt.cause = Some(bounded_cause(cause)?);
            attempt.canonical_key.clone()
        };
        self.clear_current(&canonical_key);
        Ok(())
    }

    /// Expire every lease that ended at or before `now_ms`.
    ///
    /// Expired attempts are blocked with a lease-expired cause, their fence is
    /// revoked, and a replacement may claim the work. An old worker returning
    /// afterwards fails [`Self::authorized`] because the current fence differs.
    // r[impl kiln.implementation.lease]
    pub fn expire(&mut self, now_ms: u64) -> CoreResult<Vec<ContentId>> {
        let mut expired = Vec::new();
        let identities: Vec<ContentId> = self
            .attempts
            .values()
            .filter(|attempt| attempt.phase.is_active() && attempt.lease_until_ms <= now_ms)
            .map(|attempt| attempt.identity.clone())
            .collect();
        for identity in identities {
            let canonical_key = {
                let attempt = self.attempt_mut(&identity)?;
                attempt.phase = AttemptPhase::Blocked;
                attempt.ended_at_ms = Some(now_ms);
                attempt.cause = Some("lease_expired".to_string());
                attempt.canonical_key.clone()
            };
            self.clear_current(&canonical_key);
            expired.push(identity);
        }
        Ok(expired)
    }

    /// Apply the configured supersession rule to one newer work revision.
    ///
    /// The active attempt's frozen work revision is never mutated. With
    /// `AbortActive` the old fence is revoked; with `FinishActive` the attempt
    /// may finish but records the superseding revision.
    // r[impl kiln.implementation.supersession]
    pub fn supersede(
        &mut self,
        canonical_key: &ContentId,
        new_revision: u64,
        rule: SupersessionRule,
        now_ms: u64,
    ) -> CoreResult<Option<ContentId>> {
        let Some(active) = self.current(canonical_key).cloned() else {
            return Ok(None);
        };
        if active.work_revision >= new_revision {
            return Ok(None);
        }
        match rule {
            SupersessionRule::AbortActive => {
                let attempt = self.attempt_mut(&active.identity)?;
                attempt.phase = AttemptPhase::Superseded;
                attempt.ended_at_ms = Some(now_ms);
                attempt.superseded_by = Some(new_revision);
                self.clear_current(canonical_key);
            }
            SupersessionRule::FinishActive => {
                let attempt = self.attempt_mut(&active.identity)?;
                attempt.superseded_by = Some(new_revision);
            }
        }
        Ok(Some(active.identity))
    }

    /// Whether a result for this attempt may still be admitted.
    #[must_use]
    pub fn is_current(&self, identity: &ContentId, fence: u64, now_ms: u64) -> bool {
        self.get(identity).is_some_and(|attempt| {
            attempt.fence == fence
                && attempt.phase.is_active()
                && attempt.lease_until_ms > now_ms
                && self
                    .current
                    .get(attempt.canonical_key.as_str())
                    .is_some_and(|current| current == identity.as_str())
        })
    }

    fn authorized_fence(
        &self,
        identity: &ContentId,
        fence: u64,
    ) -> CoreResult<&ImplementationAttempt> {
        let attempt = self.get(identity).ok_or_else(|| {
            refusal(
                "implementation_attempt_missing",
                "attempt.identity",
                "attempt identity is not retained",
            )
        })?;
        if attempt.fence != fence {
            return Err(refusal(
                "implementation_fence_stale",
                "attempt.fence",
                "attempt fence is stale",
            ));
        }
        Ok(attempt)
    }

    fn authorized(
        &self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
    ) -> CoreResult<&ImplementationAttempt> {
        let attempt = self.get(identity).ok_or_else(|| {
            refusal(
                "implementation_attempt_missing",
                "attempt.identity",
                "attempt identity is not retained",
            )
        })?;
        if attempt.fence != fence {
            return Err(refusal(
                "implementation_fence_stale",
                "attempt.fence",
                "attempt fence is stale",
            ));
        }
        if attempt.lease_until_ms <= now_ms {
            return Err(refusal(
                "implementation_lease_expired",
                "attempt.lease",
                "attempt lease has expired",
            ));
        }
        Ok(attempt)
    }

    fn require_active(&self, identity: &ContentId, fence: u64, now_ms: u64) -> CoreResult<()> {
        let attempt = self.authorized(identity, fence, now_ms)?;
        if !attempt.phase.is_active() {
            return Err(refusal(
                "implementation_attempt_terminal",
                "attempt.phase",
                "attempt is not active",
            ));
        }
        Ok(())
    }

    fn require_phase(
        &self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        allowed: &[AttemptPhase],
    ) -> CoreResult<()> {
        let attempt = self.authorized(identity, fence, now_ms)?;
        if !allowed.contains(&attempt.phase) {
            return Err(refusal(
                "implementation_attempt_phase",
                "attempt.phase",
                "attempt phase does not admit this transition",
            ));
        }
        Ok(())
    }

    fn attempt_mut(&mut self, identity: &ContentId) -> CoreResult<&mut ImplementationAttempt> {
        self.attempts.get_mut(identity.as_str()).ok_or_else(|| {
            refusal(
                "implementation_attempt_missing",
                "attempt.identity",
                "attempt identity is not retained",
            )
        })
    }

    fn clear_current(&mut self, canonical_key: &ContentId) {
        self.current.remove(canonical_key.as_str());
    }
}

fn attempt_identity(canonical_key: &ContentId, ordinal: u8, fence: u64) -> CoreResult<ContentId> {
    let mut bytes = Vec::new();
    push(&mut bytes, ATTEMPT_SCHEMA)?;
    push(&mut bytes, canonical_key.as_str())?;
    bytes.push(ordinal);
    bytes.extend_from_slice(&fence.to_be_bytes());
    ContentId::from_blake3_hex(blake3::hash(&bytes).to_hex().to_string())
}

fn push(bytes: &mut Vec<u8>, value: &str) -> CoreResult<()> {
    let length = u32::try_from(value.len()).map_err(|_| {
        refusal(
            "implementation_attempt_encoding",
            "attempt.identity",
            "attempt identity input is over its bound",
        )
    })?;
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

fn bounded_cause(cause: &str) -> CoreResult<String> {
    if cause.is_empty() || cause.len() > 512 || cause.contains('\0') {
        return Err(refusal(
            "implementation_attempt_cause",
            "attempt.cause",
            "attempt cause is empty, oversized or malformed",
        ));
    }
    Ok(cause.to_string())
}

fn refusal(code: &str, field: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, field, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::implementation::{
        AcceptanceObligation, IMPLEMENTATION_WORK_SCHEMA, ImplementationWork, SourceRef,
        TerminalPolicy, WorkBudget, WorkSourceKind,
    };
    use alloc::collections::BTreeSet;
    use alloc::vec;

    fn content(character: char) -> ContentId {
        ContentId::from_blake3_hex(character.to_string().repeat(64))
            .unwrap_or_else(|_| unreachable!())
    }

    fn plan() -> ContentId {
        content('d')
    }

    fn provider() -> ProviderProfileId {
        ProviderProfileId::new("lattice-implementation").unwrap_or_else(|_| unreachable!())
    }

    fn work() -> ImplementationWork {
        ImplementationWork {
            schema: IMPLEMENTATION_WORK_SCHEMA.to_string(),
            revision: 1,
            actor: "operator".to_string(),
            repository: "onix/kiln".to_string(),
            project: "kiln".to_string(),
            base: None,
            title: "Attempt work".to_string(),
            requested_outcome: "implement".to_string(),
            acceptance: vec![AcceptanceObligation {
                id: "accepted".to_string(),
                statement: "verified".to_string(),
            }],
            dependencies: Vec::new(),
            permitted_paths: vec!["crates".to_string()],
            permitted_effects: BTreeSet::from(["write_source".to_string()]),
            verification_profile: content('9'),
            budget: WorkBudget {
                attempts: 2,
                minutes_per_attempt: 60,
            },
            terminal_policy: TerminalPolicy::VerifiedPullRequest,
            priority: 1,
            aliases: vec![SourceRef {
                kind: WorkSourceKind::GitHubIssue,
                source: ExternalRef::new("github", "issue", "onix/kiln#45")
                    .unwrap_or_else(|_| unreachable!()),
                native_id: "onix/kiln#45".to_string(),
                version: content('1'),
            }],
        }
    }

    fn reference(kind: &str, id: &str) -> ExternalRef {
        ExternalRef::new("lattice", kind, id).unwrap_or_else(|_| unreachable!())
    }

    // r[verify kiln.implementation.claim.scenario.race]
    #[test]
    fn racing_claims_yield_exactly_one_current_attempt() {
        let mut ledger = AttemptLedger::default();
        let work = work();
        let first = ledger
            .claim(&work, 1_000, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            ledger.claim(&work, 1_000, DEFAULT_LEASE_MS).map(|_| ()),
            Err(refusal(
                "implementation_already_claimed",
                "attempt.claim",
                "work already has a current implementation claim",
            ))
        );
        let key = work.canonical_key().unwrap_or_else(|_| unreachable!());
        assert_eq!(
            ledger.current(&key).map(|attempt| attempt.identity.clone()),
            Some(first.identity)
        );
        assert_eq!(ledger.attempts().count(), 1);
    }

    // r[verify kiln.implementation.fencing.scenario.stale] r[verify kiln.implementation.lease.scenario.expire]
    #[test]
    fn expired_lease_revokes_the_stale_fence_and_allows_replacement() {
        let mut ledger = AttemptLedger::default();
        let work = work();
        let first = ledger
            .claim(&work, 1_000, 100)
            .unwrap_or_else(|_| unreachable!());
        let expired = ledger.expire(1_100).unwrap_or_else(|_| unreachable!());
        assert_eq!(expired, vec![first.identity.clone()]);
        let replacement = ledger
            .claim(&work, 1_100, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        assert!(replacement.fence > first.fence);
        assert!(
            ledger
                .produce_candidate(
                    &first.identity,
                    first.fence,
                    1_200,
                    &reference("run", "old")
                )
                .is_err()
        );
        assert!(!ledger.is_current(&first.identity, first.fence, 1_200));
        assert!(ledger.is_current(&replacement.identity, replacement.fence, 1_200));
    }

    // r[verify kiln.implementation.dispatch_reservation] r[verify kiln.implementation.dispatch_reservation.scenario.idempotent] r[verify kiln.implementation.dispatch_reservation.scenario.conflict] r[verify kiln.implementation.dispatch_reservation.scenario.missing]
    #[test]
    fn dispatch_reservation_is_durable_idempotent_and_fenced() {
        let mut ledger = AttemptLedger::default();
        let work = work();
        let attempt = ledger
            .claim(&work, 0, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        let missing = ledger
            .start_execution(
                &attempt.identity,
                attempt.fence,
                1,
                &reference("run", "early"),
            )
            .err()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(
            missing.code.as_str(),
            "implementation_dispatch_reservation_missing"
        );
        let first = ledger
            .reserve_dispatch(&attempt.identity, attempt.fence, 1, &plan(), &provider())
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(first.plan, plan());
        let repeated = ledger
            .reserve_dispatch(&attempt.identity, attempt.fence, 2, &plan(), &provider())
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(repeated.reserved_at_ms, first.reserved_at_ms);
        assert_eq!(
            ledger
                .dispatch_reservation(&attempt.identity)
                .cloned()
                .unwrap_or_else(|| unreachable!()),
            first
        );

        let conflict = ledger
            .reserve_dispatch(
                &attempt.identity,
                attempt.fence,
                2,
                &content('e'),
                &provider(),
            )
            .err()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(conflict.code.as_str(), "implementation_dispatch_reserved");

        let stale = ledger
            .reserve_dispatch(
                &attempt.identity,
                attempt.fence + 1,
                2,
                &plan(),
                &provider(),
            )
            .err()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(stale.code.as_str(), "implementation_fence_stale");

        ledger
            .start_execution(
                &attempt.identity,
                attempt.fence,
                2,
                &reference("run", "one"),
            )
            .unwrap_or_else(|_| unreachable!());
        let stored = ledger
            .get(&attempt.identity)
            .unwrap_or_else(|| unreachable!());
        assert_eq!(stored.phase, AttemptPhase::Executing);
        assert_eq!(stored.dispatch, Some(first));
    }

    // r[verify kiln.implementation.dispatch.scenario.reconcile]
    #[test]
    fn ambiguous_dispatch_reconciles_one_execution_without_redispatch() {
        let mut ledger = AttemptLedger::default();
        let work = work();
        let attempt = ledger
            .claim(&work, 0, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        let run = reference("run", "one");
        ledger
            .reserve_dispatch(&attempt.identity, attempt.fence, 0, &plan(), &provider())
            .unwrap_or_else(|_| unreachable!());
        ledger
            .reconcile_dispatch(&attempt.identity, attempt.fence, 0, &run)
            .unwrap_or_else(|_| unreachable!());
        ledger
            .reconcile_dispatch(&attempt.identity, attempt.fence, 0, &run)
            .unwrap_or_else(|_| unreachable!());
        assert!(
            ledger
                .reconcile_dispatch(
                    &attempt.identity,
                    attempt.fence,
                    0,
                    &reference("run", "two")
                )
                .is_err()
        );
        let stored = ledger
            .get(&attempt.identity)
            .unwrap_or_else(|| unreachable!());
        assert_eq!(stored.workflow.as_ref(), Some(&run));
    }

    // r[verify kiln.implementation.attempt_states]
    #[test]
    fn budget_and_lineage_survive_failures_and_restart() {
        let mut ledger = AttemptLedger::default();
        let work = work();
        let first = ledger
            .claim(&work, 0, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        ledger
            .fail(
                &first.identity,
                first.fence,
                1,
                FailureClass::Implementation,
                "tests failed",
            )
            .unwrap_or_else(|_| unreachable!());
        let second = ledger
            .claim(&work, 2, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(second.ordinal, 2);
        ledger
            .fail(
                &second.identity,
                second.fence,
                2,
                FailureClass::Infrastructure,
                "provider unavailable",
            )
            .unwrap_or_else(|_| unreachable!());
        // Serialize and restore: the consumed budget and lineage survive.
        let encoded = serde_json::to_vec(&ledger).unwrap_or_else(|_| unreachable!());
        let mut restored: AttemptLedger =
            serde_json::from_slice(&encoded).unwrap_or_else(|_| unreachable!());
        assert_eq!(restored.attempts().count(), 2);
        assert_eq!(
            restored.claim(&work, 3, DEFAULT_LEASE_MS).map(|_| ()),
            Err(refusal(
                "implementation_budget_exhausted",
                "attempt.budget",
                "the work lineage exhausted its reviewed attempt budget",
            ))
        );
    }

    // r[verify kiln.implementation.supersession.scenario.frozen]
    #[test]
    fn supersession_freezes_the_contract_and_honors_the_rule() {
        let mut ledger = AttemptLedger::default();
        let mut work = work();
        let attempt = ledger
            .claim(&work, 0, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        let key = work.canonical_key().unwrap_or_else(|_| unreachable!());
        ledger
            .supersede(&key, 2, SupersessionRule::AbortActive, 1)
            .unwrap_or_else(|_| unreachable!());
        let stored = ledger
            .get(&attempt.identity)
            .unwrap_or_else(|| unreachable!());
        assert_eq!(stored.phase, AttemptPhase::Superseded);
        assert_eq!(stored.superseded_by, Some(2));
        assert_eq!(stored.work_revision, 1);
        assert!(
            ledger
                .produce_candidate(&attempt.identity, attempt.fence, 1, &reference("run", "x"))
                .is_err()
        );

        work.revision = 2;
        let next = ledger
            .claim(&work, 2, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        ledger
            .supersede(&key, 3, SupersessionRule::FinishActive, 3)
            .unwrap_or_else(|_| unreachable!());
        let stored = ledger.get(&next.identity).unwrap_or_else(|| unreachable!());
        assert_eq!(stored.phase, AttemptPhase::Claimed);
        assert_eq!(stored.superseded_by, Some(3));
        ledger
            .reserve_dispatch(&next.identity, next.fence, 3, &plan(), &provider())
            .unwrap_or_else(|_| unreachable!());
        ledger
            .start_execution(&next.identity, next.fence, 3, &reference("run", "next"))
            .unwrap_or_else(|_| unreachable!());
    }

    // r[verify kiln.implementation.attempt_states.scenario.cancel]
    #[test]
    fn cancellation_revokes_the_fence_and_keeps_history() {
        let mut ledger = AttemptLedger::default();
        let work = work();
        let attempt = ledger
            .claim(&work, 0, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        ledger
            .cancel(&attempt.identity, attempt.fence, 1, "operator cancelled")
            .unwrap_or_else(|_| unreachable!());
        let stored = ledger
            .get(&attempt.identity)
            .unwrap_or_else(|| unreachable!());
        assert_eq!(stored.phase, AttemptPhase::Cancelled);
        assert_eq!(stored.cause.as_deref(), Some("operator cancelled"));
        assert!(
            ledger
                .produce_candidate(
                    &attempt.identity,
                    attempt.fence,
                    1,
                    &reference("run", "late")
                )
                .is_err()
        );
        // A cancellation still consumes the attempt budget.
        assert_eq!(
            ledger
                .claim(&work, 2, DEFAULT_LEASE_MS)
                .map(|attempt| attempt.ordinal),
            Ok(2)
        );
    }

    // r[verify kiln.implementation.supersession.scenario.frozen] r[verify kiln.implementation.base.scenario.freeze]
    #[test]
    fn claim_freezes_the_resolved_base_against_later_edits() {
        let mut ledger = AttemptLedger::default();
        let mut work = work();
        let first_base = "a".repeat(40);
        let second_base = "b".repeat(40);
        let base =
            ExternalRef::new("git", "revision", &first_base).unwrap_or_else(|_| unreachable!());
        work.base = Some(base.clone());
        let attempt = ledger
            .claim(&work, 0, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(attempt.base, Some(base.clone()));
        let key = work.canonical_key().unwrap_or_else(|_| unreachable!());
        work.revision = 2;
        work.base = Some(
            ExternalRef::new("git", "revision", &second_base).unwrap_or_else(|_| unreachable!()),
        );
        ledger
            .supersede(&key, 2, SupersessionRule::AbortActive, 1)
            .unwrap_or_else(|_| unreachable!());
        let stored = ledger
            .get(&attempt.identity)
            .unwrap_or_else(|| unreachable!());
        assert_eq!(stored.base, Some(base));
        assert_eq!(stored.work_revision, 1);
    }

    #[test]
    fn attempt_history_is_distinct_from_ci_rerun_history() {
        // Attempt identities are derived from the canonical work key, ordinal
        // and fence; no CI run or rerun identity enters this module.
        let mut ledger = AttemptLedger::default();
        let work = work();
        let attempt = ledger
            .claim(&work, 0, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(attempt.ordinal, 1);
        assert_eq!(
            attempt.identity,
            attempt_identity(
                &work.canonical_key().unwrap_or_else(|_| unreachable!()),
                1,
                attempt.fence
            )
            .unwrap_or_else(|_| unreachable!())
        );
    }
}
