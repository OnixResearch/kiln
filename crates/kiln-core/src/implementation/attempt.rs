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

pub const ATTEMPT_SCHEMA: &str = "kiln.implementation-attempt.v2";
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
    /// Exact request bytes sealed to the dispatch plan before any transport.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dispatch_request_binding: Option<super::DispatchRequestBinding>,
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
    /// Immutable worker facts, not authority to advance this attempt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_observation: Option<super::WorkerObservation>,
    /// Independent verifier observation; only `verification` admits a pass.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_outcome: Option<super::VerificationOutcome>,
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
    /// Explicit provider termination observations for ambiguous outcomes. An
    /// absent observation is a retry barrier, not evidence of worker death.
    #[serde(default)]
    resolved_unknown: BTreeMap<String, ExternalRef>,
    /// Transport intent persisted before dispatch; repeat invocations recover.
    #[serde(default)]
    dispatch_started: alloc::collections::BTreeSet<String>,
    #[serde(default)]
    dispatch_plans: BTreeMap<String, super::ImplementationDispatchPlan>,
    /// Attempts that may record a workflow only after a recorded transport
    /// start. Bound atomically with the claim, so no out-of-band launch can
    /// precede it. Empty unless an owner such as a host-local owner binds it.
    #[serde(
        default,
        skip_serializing_if = "alloc::collections::BTreeSet::is_empty"
    )]
    transport_bound: alloc::collections::BTreeSet<String>,
}

/// The exact provider program and state root of an attempt's first launch.
///
/// Recorded in the same transaction as the transport intent, so the launch
/// that won the intent is the only one recovery may use.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransportLaunch {
    pub lattice_program: String,
    pub executable_blake3: ContentId,
    pub state_directory: String,
}

/// What persisting the transport intent established.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransportStart {
    /// This call persisted the intent (and its launch, when given).
    Fresh,
    /// An intent already existed: recovery is mandatory and must use the
    /// recorded launch, which is `None` for an intent persisted before
    /// launches were recorded or without one.
    Recover(Option<TransportLaunch>),
}

impl TransportStart {
    #[must_use]
    pub const fn is_recovery(&self) -> bool {
        matches!(self, Self::Recover(_))
    }
}

/// Recorded first launches, kept apart from [`AttemptLedger`] so that a
/// binary which reads and re-saves the attempt ledger without knowing about
/// launches can never erase one.
///
/// A record is consulted only while its attempt's transport intent exists, so
/// a record left behind after another writer cleared the intent is inert.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransportLaunches {
    #[serde(default)]
    launches: BTreeMap<String, TransportLaunch>,
}

impl TransportLaunches {
    /// Combine the attempt ledger's transport-intent transition with the
    /// launch record: a fresh intent records `launch`, never overwriting a
    /// different record; an existing intent recalls its recorded launch.
    ///
    /// # Errors
    /// Refuses a fresh intent for which a different launch is already
    /// recorded, so the enclosing transaction rolls back.
    // r[impl kiln.implementation.lattice_cli.scenario.bound_root]
    pub fn start(
        &mut self,
        identity: &ContentId,
        previously_started: bool,
        launch: Option<&TransportLaunch>,
    ) -> CoreResult<TransportStart> {
        if previously_started {
            return Ok(TransportStart::Recover(self.recall(identity)));
        }
        if let Some(launch) = launch {
            match self.launches.get(identity.as_str()) {
                Some(recorded) if recorded != launch => {
                    return Err(refusal(
                        "implementation_transport_launch_conflict",
                        "dispatch.launch",
                        "a different launch is already recorded for this attempt",
                    ));
                }
                Some(_) => {}
                None => {
                    self.launches
                        .insert(identity.as_str().to_string(), launch.clone());
                }
            }
        }
        Ok(TransportStart::Fresh)
    }

    /// The launch recorded for this attempt, if any.
    #[must_use]
    pub fn recall(&self, identity: &ContentId) -> Option<TransportLaunch> {
        self.launches.get(identity.as_str()).cloned()
    }

    /// Remove the record when the attempt's transport intent is settled.
    pub fn remove(&mut self, identity: &ContentId) {
        self.launches.remove(identity.as_str());
    }
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
        if self.attempts.values().any(|attempt| {
            attempt.canonical_key == canonical_key
                && attempt.failure == Some(FailureClass::Unknown)
                && !self
                    .resolved_unknown
                    .contains_key(attempt.identity.as_str())
        }) {
            return Err(refusal(
                "implementation_reconciliation_required",
                "attempt.workflow",
                "an unknown provider outcome must be reconciled before another attempt",
            ));
        }
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
        if self.attempts.values().any(|attempt| {
            attempt.canonical_key == canonical_key
                && attempt.work_identity == work_identity
                && attempt.phase == AttemptPhase::Verified
        }) {
            return Err(refusal(
                "implementation_already_verified",
                "attempt.claim",
                "work version already has a verified implementation attempt",
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
            dispatch_request_binding: None,
            workflow: None,
            candidate: None,
            handoff: None,
            verification: None,
            publication: None,
            metrics: None,
            worker_observation: None,
            verification_outcome: None,
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
        self.require_phase(
            identity,
            fence,
            now_ms,
            &[AttemptPhase::Claimed, AttemptPhase::Executing],
        )?;
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
        let reservation =
            self.reserve_dispatch(identity, fence, now_ms, &digest, &plan.provider_profile)?;
        self.dispatch_plans
            .insert(identity.as_str().to_string(), plan.clone());
        Ok(reservation)
    }

    pub fn dispatch_plan(
        &self,
        identity: &ContentId,
    ) -> Option<&super::ImplementationDispatchPlan> {
        self.dispatch_plans.get(identity.as_str())
    }
    pub fn bind_dispatch_request(
        &mut self,
        work: &ImplementationWork,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        request_digest: &ContentId,
    ) -> CoreResult<super::DispatchRequestBinding> {
        let plan = self.dispatch_plans.get(identity.as_str()).ok_or_else(|| {
            refusal(
                "implementation_dispatch_plan_missing",
                "dispatch.plan",
                "request binding requires a retained dispatch plan",
            )
        })?;
        let attempt = self.authorized(identity, fence, now_ms)?;
        let plan_id = super::admit_dispatch_plan(work, attempt, plan)?;
        if attempt.work_identity != work.identity()?
            || attempt.work_revision != work.revision
            || attempt
                .dispatch
                .as_ref()
                .map(|reservation| &reservation.plan)
                != Some(&plan_id)
        {
            return Err(refusal(
                "implementation_dispatch_binding",
                "dispatch.plan",
                "request seal does not match the exact reserved work, attempt, fence and plan",
            ));
        }
        let binding =
            super::DispatchRequestBinding::new(identity, fence, &plan_id, request_digest)?;
        if let Some(retained) = self
            .get(identity)
            .and_then(|attempt| attempt.dispatch_request_binding.as_ref())
        {
            if retained != &binding {
                return Err(refusal(
                    "implementation_request_binding_conflict",
                    "dispatch.request",
                    "attempt already retains a different request byte binding",
                ));
            }
            return Ok(retained.clone());
        }
        if self.dispatch_started.contains(identity.as_str()) {
            return Err(refusal(
                "implementation_request_binding_late",
                "dispatch.request",
                "request binding must be persisted before transport intent",
            ));
        }
        self.attempt_mut(identity)?.dispatch_request_binding = Some(binding.clone());
        Ok(binding)
    }

    pub fn dispatch_request_binding(
        &self,
        identity: &ContentId,
    ) -> Option<&super::DispatchRequestBinding> {
        self.get(identity)?.dispatch_request_binding.as_ref()
    }

    /// Require a recorded transport start before any workflow is recorded for
    /// this attempt. Idempotent.
    pub fn bind_transport_start(&mut self, identity: &ContentId) -> CoreResult<()> {
        self.attempt_mut(identity)?;
        self.transport_bound.insert(identity.as_str().to_string());
        Ok(())
    }

    /// Whether this attempt may record a workflow only after a transport start.
    #[must_use]
    pub fn transport_start_bound(&self, identity: &ContentId) -> bool {
        self.transport_bound.contains(identity.as_str())
    }

    /// Refuse an out-of-band workflow acknowledgement for a bound attempt.
    fn require_bound_transport_start(&self, identity: &ContentId) -> CoreResult<()> {
        if self.transport_bound.contains(identity.as_str())
            && !self.dispatch_started.contains(identity.as_str())
        {
            return Err(refusal(
                "implementation_transport_start_required",
                "attempt.dispatch",
                "this attempt's owner admits only a dispatch route that records the transport start before any spawn or delivery",
            ));
        }
        Ok(())
    }

    pub fn dispatch_requires_recovery(&self, identity: &ContentId) -> bool {
        self.dispatch_started.contains(identity.as_str())
    }

    /// Retain immutable facts for the exact historical attempt and fence.
    /// This deliberately does not require or establish a live claim.
    // r[impl kiln.implementation.outcome_metrics.scenario.binding]
    pub fn record_worker_observation(
        &mut self,
        identity: &ContentId,
        fence: u64,
        observation: &super::WorkerObservation,
    ) -> CoreResult<()> {
        let attempt = self.attempt_mut(identity)?;
        if attempt.fence != fence {
            return Err(refusal(
                "implementation_fence_stale",
                "attempt.fence",
                "attempt fence is stale",
            ));
        }
        observation.validate()?;
        if let Some(retained) = &attempt.worker_observation {
            if retained == observation {
                return Ok(());
            }
            return Err(refusal(
                "implementation_worker_observation_conflict",
                "attempt.worker_observation",
                "attempt already retains a different worker observation",
            ));
        }
        attempt.worker_observation = Some(observation.clone());
        Ok(())
    }

    /// Retain verifier metadata without changing phase or admitting evidence.
    /// Incomplete observations may progress to an unknown canonical run;
    /// uncertainty may refine to a definitive result, never the reverse.
    pub fn record_verification_outcome(
        &mut self,
        identity: &ContentId,
        fence: u64,
        outcome: super::VerificationOutcome,
    ) -> CoreResult<()> {
        use super::VerificationOutcome::{Incomplete, Rejected, Unknown, Verified};
        let attempt = self.attempt_mut(identity)?;
        if attempt.fence != fence {
            return Err(refusal(
                "implementation_fence_stale",
                "attempt.fence",
                "attempt fence is stale",
            ));
        }
        if outcome == Verified && attempt.verification.is_none() {
            return Err(refusal(
                "implementation_verification_evidence_missing",
                "attempt.verification_outcome",
                "verified outcome requires already admitted verification evidence",
            ));
        }
        if let Some(retained) = attempt.verification_outcome {
            if retained == outcome {
                return Ok(());
            }
            if !matches!(
                (retained, outcome),
                (Incomplete, Unknown) | (Unknown | Incomplete, Verified | Rejected)
            ) {
                return Err(refusal(
                    "implementation_verification_outcome_conflict",
                    "attempt.verification_outcome",
                    "attempt already retains a conflicting verifier outcome",
                ));
            }
        }
        attempt.verification_outcome = Some(outcome);
        Ok(())
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
        self.require_phase(
            identity,
            fence,
            now_ms,
            &[AttemptPhase::Claimed, AttemptPhase::Executing],
        )?;
        workflow.validate()?;
        self.refuse_crossed_run(identity, workflow)?;
        self.require_bound_transport_start(identity)?;
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
        self.refuse_crossed_run(identity, observed)?;
        self.require_bound_transport_start(identity)?;
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

    /// A provider workflow execution identity binds at most one attempt.
    // r[impl kiln.implementation.execution_identity]
    fn refuse_crossed_run(&self, identity: &ContentId, run: &ExternalRef) -> CoreResult<()> {
        if self
            .attempts
            .values()
            .any(|attempt| attempt.identity != *identity && attempt.workflow.as_ref() == Some(run))
        {
            return Err(refusal(
                "implementation_dispatch_crossed",
                "attempt.workflow",
                "the workflow run is already recorded on another attempt",
            ));
        }
        Ok(())
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
        let existing = self.get(identity).ok_or_else(|| {
            refusal(
                "implementation_attempt_missing",
                "attempt.identity",
                "attempt is not retained",
            )
        })?;
        if existing.fence != fence {
            return Err(refusal(
                "implementation_fence_stale",
                "attempt.fence",
                "attempt fence is stale",
            ));
        }
        if let Some(retained) = &existing.verification {
            if retained == verification {
                let digest =
                    super::admit_candidate_verification(work, existing, handoff, verification)?;
                self.attempt_mut(identity)?.verification_outcome =
                    Some(super::VerificationOutcome::Verified);
                return Ok(digest);
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
            attempt.verification_outcome = Some(super::VerificationOutcome::Verified);
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
        let needs_reconciliation = self.dispatch_started.contains(identity.as_str())
            || self
                .get(identity)
                .is_some_and(|attempt| attempt.workflow.is_some());
        let canonical_key = {
            let attempt = self.attempt_mut(identity)?;
            attempt.phase = AttemptPhase::Cancelled;
            attempt.ended_at_ms = Some(now_ms);
            attempt.cause = Some(bounded_cause(cause)?);
            if needs_reconciliation {
                // Revoking the claim does not prove that provider resources stopped.
                attempt.failure = Some(FailureClass::Unknown);
            }
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
            let unacknowledged = self.dispatch_started.contains(identity.as_str())
                && self
                    .get(&identity)
                    .is_some_and(|attempt| attempt.workflow.is_none());
            let canonical_key = {
                let attempt = self.attempt_mut(&identity)?;
                attempt.phase = AttemptPhase::Blocked;
                attempt.ended_at_ms = Some(now_ms);
                attempt.cause = Some("lease_expired".to_string());
                if unacknowledged {
                    attempt.failure = Some(FailureClass::Unknown);
                    attempt.cause = Some("dispatch_outcome_unknown".to_string());
                }
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
                // Revoking the fence does not observe a started worker stop.
                let started = self.dispatch_started.contains(active.identity.as_str())
                    || active.workflow.is_some();
                let attempt = self.attempt_mut(&active.identity)?;
                attempt.phase = AttemptPhase::Superseded;
                attempt.ended_at_ms = Some(now_ms);
                attempt.superseded_by = Some(new_revision);
                if started
                    && attempt.failure.is_none()
                    && attempt.candidate.is_none()
                    && attempt.handoff.is_none()
                    && attempt.worker_observation.is_none()
                {
                    attempt.failure = Some(FailureClass::Unknown);
                }
                self.clear_current(canonical_key);
            }
            SupersessionRule::FinishActive => {
                let attempt = self.attempt_mut(&active.identity)?;
                attempt.superseded_by = Some(new_revision);
            }
        }
        Ok(Some(active.identity))
    }

    /// Reserve transport before dispatch. True means recovery is mandatory.
    // r[impl kiln.implementation.unknown_barrier]
    pub fn begin_dispatch_transport(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        plan: &ContentId,
    ) -> CoreResult<bool> {
        let attempt = self.authorized(identity, fence, now_ms)?;
        if attempt.dispatch.as_ref().map(|reserved| &reserved.plan) != Some(plan) {
            return Err(refusal(
                "implementation_dispatch_reservation_missing",
                "attempt.dispatch",
                "transport requires the exact durable dispatch reservation",
            ));
        }
        let binding = attempt.dispatch_request_binding.as_ref().ok_or_else(|| {
            refusal(
                "implementation_request_binding_missing",
                "dispatch.request",
                "dispatch requires a durable exact-request seal before any effect",
            )
        })?;
        binding.validate_for(identity, fence, plan, &binding.request_digest)?;
        Ok(!self.dispatch_started.insert(identity.as_str().to_string()))
    }

    /// End an attempt whose provider states that no worker effect of it can
    /// have started, such as a typed before-effect refusal or a recovery that
    /// closed the attempt before execution.
    ///
    /// Only a current claimed attempt with its reservation, a persisted
    /// transport intent and no recorded workflow qualifies. The attempt fails
    /// as infrastructure, never unknown, and its transport intent is cleared,
    /// so nothing remains to reconcile and the attempt cannot dispatch again.
    // r[impl kiln.implementation.lattice_cli.scenario.refusal] r[impl kiln.implementation.lattice_cli.scenario.closed]
    pub fn settle_dispatch_without_execution(
        &mut self,
        identity: &ContentId,
        fence: u64,
        now_ms: u64,
        cause: &str,
    ) -> CoreResult<()> {
        self.require_phase(identity, fence, now_ms, &[AttemptPhase::Claimed])?;
        let cause = bounded_cause(cause)?;
        let started = self.dispatch_started.contains(identity.as_str());
        let canonical_key = {
            let attempt = self.attempt_mut(identity)?;
            if attempt.dispatch.is_none() || attempt.workflow.is_some() || !started {
                return Err(refusal(
                    "implementation_settlement_binding",
                    "attempt.dispatch",
                    "only a reserved, started transport with no recorded execution settles without execution",
                ));
            }
            attempt.phase = AttemptPhase::Failed;
            attempt.ended_at_ms = Some(now_ms);
            attempt.failure = Some(FailureClass::Infrastructure);
            attempt.cause = Some(cause);
            attempt.canonical_key.clone()
        };
        self.dispatch_started.remove(identity.as_str());
        self.clear_current(&canonical_key);
        Ok(())
    }

    /// Retain an independently observed provider termination after an unknown
    /// outcome. This does not accept a candidate or refund consumed budget.
    // r[impl kiln.implementation.unknown_barrier]
    pub fn reconcile_unknown(
        &mut self,
        identity: &ContentId,
        fence: u64,
        termination: &ExternalRef,
    ) -> CoreResult<()> {
        termination.validate()?;
        let attempt = self.get(identity).ok_or_else(|| {
            refusal(
                "implementation_attempt_missing",
                "attempt.identity",
                "attempt is not retained",
            )
        })?;
        if attempt.fence != fence || attempt.failure != Some(FailureClass::Unknown) {
            return Err(refusal(
                "implementation_reconciliation_binding",
                "attempt.fence",
                "termination observation does not bind an unknown attempt and fence",
            ));
        }
        if let Some(existing) = self.resolved_unknown.get(identity.as_str()) {
            if existing == termination {
                return Ok(());
            }
            return Err(refusal(
                "implementation_reconciliation_conflict",
                "attempt.workflow",
                "a different termination observation is already retained",
            ));
        }
        self.resolved_unknown
            .insert(identity.as_str().to_string(), termination.clone());
        Ok(())
    }

    /// Unknown provider outcomes requiring independent termination observation.
    pub fn unresolved(&self) -> impl Iterator<Item = &ImplementationAttempt> {
        self.attempts.values().filter(|attempt| {
            attempt.failure == Some(FailureClass::Unknown)
                && !self
                    .resolved_unknown
                    .contains_key(attempt.identity.as_str())
        })
    }

    /// Whether an independent termination observation was reconciled for this
    /// unknown attempt.
    #[must_use]
    pub fn termination_observed(&self, identity: &ContentId) -> bool {
        self.resolved_unknown.contains_key(identity.as_str())
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

    /// Verify terminal candidate authority independently of the worker lease.
    /// A superseded/replaced attempt cannot publish a previously valid result.
    pub fn authorize_publication(
        &self,
        identity: &ContentId,
        fence: u64,
    ) -> CoreResult<&ImplementationAttempt> {
        let attempt = self.authorized_fence(identity, fence)?;
        if attempt.phase != AttemptPhase::Verified {
            return Err(refusal(
                "implementation_attempt_unverified",
                "attempt.phase",
                "publication requires an independently verified attempt",
            ));
        }
        Ok(attempt)
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
        if attempt.superseded_by.is_some()
            || self.attempts.values().any(|other| {
                other.canonical_key == attempt.canonical_key && other.fence > attempt.fence
            })
        {
            return Err(refusal(
                "implementation_fence_stale",
                "attempt.fence",
                "attempt was superseded or replaced and cannot publish",
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
        if !self.is_current(identity, fence, now_ms) {
            return Err(refusal(
                "implementation_fence_stale",
                "attempt.fence",
                "attempt no longer owns the current active claim",
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
        AcceptanceObligation, CANDIDATE_HANDOFF_SCHEMA, CandidateEvidence, CandidateHandoff,
        EvidenceKind, EvidenceOutcome, IMPLEMENTATION_WORK_SCHEMA, ImplementationWork, SourceRef,
        TerminalPolicy, WorkBudget, WorkSourceKind, acceptance_digest,
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

    /// Claim, reserve and acknowledge one attempt executing `run`.
    fn executing(
        ledger: &mut AttemptLedger,
        work: &ImplementationWork,
        now_ms: u64,
        lease_ms: u64,
        run: &str,
    ) -> ImplementationAttempt {
        let attempt = ledger
            .claim(work, now_ms, lease_ms)
            .unwrap_or_else(|_| unreachable!());
        ledger
            .reserve_dispatch(
                &attempt.identity,
                attempt.fence,
                now_ms,
                &plan(),
                &provider(),
            )
            .unwrap_or_else(|_| unreachable!());
        ledger
            .start_execution(
                &attempt.identity,
                attempt.fence,
                now_ms,
                &reference("run", run),
            )
            .unwrap_or_else(|_| unreachable!());
        attempt
    }

    /// A handoff the attempt would admit while it is current and executing.
    fn handoff_for(work: &ImplementationWork, attempt: &ImplementationAttempt) -> CandidateHandoff {
        CandidateHandoff {
            schema: CANDIDATE_HANDOFF_SCHEMA.to_string(),
            work_identity: attempt.work_identity.clone(),
            work_revision: attempt.work_revision,
            attempt_identity: attempt.identity.clone(),
            fence: attempt.fence,
            repository: work.repository.clone(),
            base: work.base.clone(),
            candidate: reference("commit", "candidate"),
            tree: reference("tree", "tree"),
            changed_paths: vec!["crates/src/lib.rs".to_string()],
            acceptance_digest: acceptance_digest(work).unwrap_or_else(|_| unreachable!()),
            verification_profile: work.verification_profile.clone(),
            toolchain: content('7'),
            environment: content('8'),
            evidence: vec![CandidateEvidence {
                identity: content('e'),
                kind: EvidenceKind::Check,
                outcome: EvidenceOutcome::Passed,
                reference: reference("run", "candidate-check"),
            }],
        }
    }

    /// The ledger as the durable store restores it.
    fn restored(ledger: &AttemptLedger) -> AttemptLedger {
        let bytes = serde_json::to_vec(ledger).unwrap_or_else(|_| unreachable!());
        serde_json::from_slice(&bytes).unwrap_or_else(|_| unreachable!())
    }

    fn code<T>(result: CoreResult<T>) -> String {
        result.err().map(|error| error.code).unwrap_or_default()
    }

    // r[verify kiln.implementation.execution_identity.scenario.late] r[verify kiln.implementation.unknown_barrier] r[verify kiln.implementation.attempt_states.scenario.cancel]
    #[test]
    fn late_callbacks_after_cancellation_never_attach_or_resurrect() {
        let work = work();
        let mut ledger = AttemptLedger::default();
        let attempt = executing(&mut ledger, &work, 0, DEFAULT_LEASE_MS, "worker");
        let handoff = handoff_for(&work, &attempt);
        ledger
            .cancel(&attempt.identity, attempt.fence, 1, "operator cancelled")
            .unwrap_or_else(|_| unreachable!());
        let cancelled = ledger
            .get(&attempt.identity)
            .cloned()
            .unwrap_or_else(|| unreachable!());
        // The cancellation request does not prove the provider stopped.
        assert_eq!(
            (cancelled.phase, cancelled.failure),
            (AttemptPhase::Cancelled, Some(FailureClass::Unknown))
        );
        // Each callback twice, with a restore between every pair.
        for _ in 0..2 {
            ledger = restored(&ledger);
            assert_eq!(
                code(ledger.admit_candidate_handoff(
                    &work,
                    &attempt.identity,
                    attempt.fence,
                    2,
                    &handoff
                )),
                "implementation_fence_stale"
            );
            ledger = restored(&ledger);
            assert!(
                ledger
                    .start_execution(
                        &attempt.identity,
                        attempt.fence,
                        2,
                        &reference("run", "worker")
                    )
                    .is_err()
            );
            assert!(
                ledger
                    .fail(
                        &attempt.identity,
                        attempt.fence,
                        2,
                        FailureClass::Implementation,
                        "late exit"
                    )
                    .is_err()
            );
            assert!(
                ledger
                    .cancel(&attempt.identity, attempt.fence, 2, "repeated cancel")
                    .is_err()
            );
            assert_eq!(ledger.get(&attempt.identity), Some(&cancelled));
            assert_eq!(ledger.attempts().count(), 1);
        }
        assert!(
            ledger
                .authorize_publication(&attempt.identity, attempt.fence)
                .is_err()
        );
        // No callback settles the unknown outcome; only an observed termination does.
        assert_eq!(
            code(ledger.claim(&work, 3, DEFAULT_LEASE_MS)),
            "implementation_reconciliation_required"
        );
        ledger
            .reconcile_unknown(
                &attempt.identity,
                attempt.fence,
                &reference("termination", "stopped"),
            )
            .unwrap_or_else(|_| unreachable!());
        let next = ledger
            .claim(&work, 4, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        // Refused callbacks spent no attempt budget.
        assert_eq!(next.ordinal, 2);
    }

    // r[verify kiln.implementation.execution_identity] r[verify kiln.implementation.execution_identity.scenario.crossed] r[verify kiln.implementation.fencing.scenario.stale]
    #[test]
    fn predecessor_callbacks_never_settle_or_feed_the_replacement() {
        let work = work();
        let mut ledger = AttemptLedger::default();
        let lease_ms = 100;
        let old = executing(&mut ledger, &work, 0, lease_ms, "old-run");
        let old_handoff = handoff_for(&work, &old);
        ledger.expire(lease_ms).unwrap_or_else(|_| unreachable!());
        let new = ledger
            .claim(&work, lease_ms, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        assert!(new.fence > old.fence);
        ledger
            .reserve_dispatch(&new.identity, new.fence, lease_ms, &plan(), &provider())
            .unwrap_or_else(|_| unreachable!());
        let old_before = ledger.get(&old.identity).cloned();
        let new_before = ledger.get(&new.identity).cloned();

        // The predecessor's own late callbacks are refused.
        ledger = restored(&ledger);
        assert!(
            ledger
                .admit_candidate_handoff(&work, &old.identity, old.fence, 101, &old_handoff)
                .is_err()
        );
        assert!(
            ledger
                .start_execution(&old.identity, old.fence, 101, &reference("run", "old-run"))
                .is_err()
        );
        assert!(
            ledger
                .fail(
                    &old.identity,
                    old.fence,
                    101,
                    FailureClass::Implementation,
                    "late exit"
                )
                .is_err()
        );
        assert!(
            ledger
                .cancel(&old.identity, old.fence, 101, "late cancel")
                .is_err()
        );

        // Its run cannot become the replacement's execution.
        ledger = restored(&ledger);
        assert_eq!(
            code(ledger.start_execution(
                &new.identity,
                new.fence,
                102,
                &reference("run", "old-run")
            )),
            "implementation_dispatch_crossed"
        );
        ledger = restored(&ledger);
        assert_eq!(
            code(ledger.reconcile_dispatch(
                &new.identity,
                new.fence,
                102,
                &reference("run", "old-run")
            )),
            "implementation_dispatch_crossed"
        );
        assert_eq!(ledger.get(&old.identity).cloned(), old_before);
        assert_eq!(ledger.get(&new.identity).cloned(), new_before);
        assert!(ledger.is_current(&new.identity, new.fence, 102));

        // The replacement's own execution proceeds; the predecessor's candidate cannot attach.
        ledger
            .start_execution(&new.identity, new.fence, 103, &reference("run", "new-run"))
            .unwrap_or_else(|_| unreachable!());
        assert!(
            ledger
                .admit_candidate_handoff(&work, &new.identity, new.fence, 104, &old_handoff)
                .is_err()
        );
        assert!(
            ledger
                .admit_candidate_handoff(
                    &work,
                    &new.identity,
                    new.fence,
                    104,
                    &handoff_for(&work, &new)
                )
                .is_ok()
        );
        assert_eq!(ledger.get(&old.identity).cloned(), old_before);
    }

    // r[verify kiln.implementation.execution_identity.scenario.late]
    #[test]
    fn callbacks_for_an_unadmitted_attempt_create_nothing() {
        let work = work();
        let mut ledger = AttemptLedger::default();
        let real = executing(&mut ledger, &work, 0, DEFAULT_LEASE_MS, "worker");
        let before = serde_json::to_vec(&ledger).unwrap_or_else(|_| unreachable!());
        let absent = content('7');
        let mut forged = handoff_for(&work, &real);
        forged.attempt_identity = absent.clone();
        let codes = vec![
            code(ledger.start_execution(&absent, real.fence, 1, &reference("run", "ghost"))),
            code(ledger.reconcile_dispatch(&absent, real.fence, 1, &reference("run", "ghost"))),
            code(ledger.admit_candidate_handoff(&work, &absent, real.fence, 1, &forged)),
            code(ledger.fail(&absent, real.fence, 1, FailureClass::Implementation, "exit")),
            code(ledger.cancel(&absent, real.fence, 1, "cancel")),
            code(ledger.reconcile_unknown(&absent, real.fence, &reference("termination", "ghost"))),
        ];
        assert!(
            codes
                .iter()
                .all(|code| code == "implementation_attempt_missing"),
            "{codes:?}"
        );
        assert_eq!(
            serde_json::to_vec(&ledger).unwrap_or_else(|_| unreachable!()),
            before
        );
    }

    fn worker_observation() -> crate::implementation::WorkerObservation {
        crate::implementation::WorkerObservation {
            terminal: crate::implementation::WorkerTerminalState::Completed,
            worker_profile: Some(content('d')),
            harness: Some("harness".into()),
            model: Some("model".into()),
            usage: crate::implementation::AttemptUsage::default(),
        }
    }

    #[test]
    fn historical_worker_observations_are_immutable_and_never_restore_authority() {
        let mut ledger = AttemptLedger::default();
        let work = work();
        let first = ledger
            .claim(&work, 0, 100)
            .unwrap_or_else(|_| unreachable!());
        ledger.expire(100).unwrap_or_else(|_| unreachable!());
        let next = ledger
            .claim(&work, 100, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        let old_before = ledger
            .get(&first.identity)
            .cloned()
            .unwrap_or_else(|| unreachable!());
        let next_before = ledger.get(&next.identity).cloned();
        let worker = worker_observation();
        for (identity, fence) in [
            (&first.identity, next.fence),
            (&next.identity, first.fence),
            (&content('7'), first.fence),
        ] {
            assert!(
                ledger
                    .record_worker_observation(identity, fence, &worker)
                    .is_err()
            );
        }
        assert_eq!(ledger.get(&first.identity), Some(&old_before));
        ledger
            .record_worker_observation(&first.identity, first.fence, &worker)
            .unwrap_or_else(|_| unreachable!());
        ledger
            .record_worker_observation(&first.identity, first.fence, &worker)
            .unwrap_or_else(|_| unreachable!());
        let mut conflicting = worker.clone();
        conflicting.terminal = crate::implementation::WorkerTerminalState::Failed;
        assert!(
            ledger
                .record_worker_observation(&first.identity, first.fence, &conflicting)
                .is_err()
        );
        let retained = ledger
            .get(&first.identity)
            .unwrap_or_else(|| unreachable!());
        let mut expected = old_before;
        expected.worker_observation = Some(worker);
        assert_eq!(retained, &expected);
        assert_eq!(ledger.get(&next.identity).cloned(), next_before);
        assert!(!ledger.is_current(&first.identity, first.fence, 101));
        assert!(ledger.is_current(&next.identity, next.fence, 101));
        assert!(
            ledger
                .produce_candidate(
                    &first.identity,
                    first.fence,
                    101,
                    &reference("commit", "late")
                )
                .is_err()
        );
    }

    #[test]
    fn malformed_worker_metadata_is_refused_without_retention() {
        use crate::implementation::metrics::MAX_WORKER_METADATA_BYTES;
        let mut ledger = AttemptLedger::default();
        let attempt = ledger
            .claim(&work(), 0, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        for name in [
            String::new(),
            " ".into(),
            "model\nname".into(),
            "a".repeat(MAX_WORKER_METADATA_BYTES + 1),
        ] {
            for model in [false, true] {
                let mut worker = worker_observation();
                if model {
                    worker.model = Some(name.clone());
                } else {
                    worker.harness = Some(name.clone());
                }
                assert!(
                    ledger
                        .record_worker_observation(&attempt.identity, attempt.fence, &worker)
                        .is_err()
                );
                assert_eq!(ledger.get(&attempt.identity), Some(&attempt));
            }
        }
        let mut worker = worker_observation();
        worker.harness = Some("a".repeat(MAX_WORKER_METADATA_BYTES));
        ledger
            .record_worker_observation(&attempt.identity, attempt.fence, &worker)
            .unwrap_or_else(|_| unreachable!());
    }

    #[test]
    fn verifier_observations_refine_uncertainty_without_admitting_a_pass() {
        use crate::implementation::VerificationOutcome::{Incomplete, Rejected, Unknown, Verified};
        for uncertain in [Unknown, Incomplete] {
            let mut ledger = AttemptLedger::default();
            let attempt = ledger
                .claim(&work(), 0, DEFAULT_LEASE_MS)
                .unwrap_or_else(|_| unreachable!());
            assert!(
                ledger
                    .record_verification_outcome(&content('7'), attempt.fence, uncertain)
                    .is_err()
            );
            assert!(
                ledger
                    .record_verification_outcome(&attempt.identity, attempt.fence + 1, uncertain)
                    .is_err()
            );
            assert!(
                ledger
                    .record_verification_outcome(&attempt.identity, attempt.fence, Verified)
                    .is_err()
            );
            assert_eq!(ledger.get(&attempt.identity), Some(&attempt));
            ledger
                .record_verification_outcome(&attempt.identity, attempt.fence, uncertain)
                .unwrap_or_else(|_| unreachable!());
            ledger
                .record_verification_outcome(&attempt.identity, attempt.fence, uncertain)
                .unwrap_or_else(|_| unreachable!());
            if uncertain == Incomplete {
                ledger
                    .record_verification_outcome(&attempt.identity, attempt.fence, Unknown)
                    .unwrap_or_else(|_| unreachable!());
                assert!(
                    ledger
                        .record_verification_outcome(&attempt.identity, attempt.fence, Incomplete)
                        .is_err()
                );
            }
            ledger
                .record_verification_outcome(&attempt.identity, attempt.fence, Rejected)
                .unwrap_or_else(|_| unreachable!());
            assert!(
                ledger
                    .record_verification_outcome(&attempt.identity, attempt.fence, uncertain)
                    .is_err()
            );
            assert!(
                ledger
                    .record_verification_outcome(&attempt.identity, attempt.fence, Verified)
                    .is_err()
            );
            let mut expected = attempt.clone();
            expected.verification_outcome = Some(Rejected);
            assert_eq!(ledger.get(&attempt.identity), Some(&expected));
            assert!(ledger.is_current(&attempt.identity, attempt.fence, 1));
            assert!(
                ledger
                    .authorize_publication(&attempt.identity, attempt.fence)
                    .is_err()
            );
        }
    }

    #[test]
    fn old_attempt_rows_default_optional_observations_without_admission_changes() {
        let mut ledger = AttemptLedger::default();
        let attempt = ledger
            .claim(&work(), 0, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        let mut row = serde_json::to_value(&attempt).unwrap_or_else(|_| unreachable!());
        row.as_object_mut()
            .unwrap_or_else(|| unreachable!())
            .remove("worker_observation");
        row.as_object_mut()
            .unwrap_or_else(|| unreachable!())
            .remove("verification_outcome");
        let old: ImplementationAttempt =
            serde_json::from_value(row).unwrap_or_else(|_| unreachable!());
        assert_eq!(old, attempt);
    }
}
