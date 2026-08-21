use core::sync::atomic::AtomicBool;
use std::collections::BTreeSet;

use kiln_core::build::{
    BuildEffectPlan, BuildEvaluationObservation, BuildLogChunk, BuildLogReadRequest,
};
use kiln_core::cob::{CobEffectPlan, CobObservation};
use kiln_core::discovery::{DiscoveryEffectPlan, DiscoveryObservation, TaskProposal};
use kiln_core::domain::Outcome;
use kiln_core::gating::{GateEffectPlan, GateEvaluationObservation};
use kiln_core::operations::{JournalRecord, OutboxRecord};
use kiln_core::provider::{
    CapabilityProfile, ProviderFailureClass, ProviderFailureDisposition, ProviderObservation,
    ProviderRole, classify_provider_failure,
};
use kiln_core::remote::{RemoteAction, RemoteObservation};
use kiln_core::source::{PijulSnapshotEntry, PijulSnapshotLimits};
use kiln_core::types::{
    Alias, ContentId, CoreResult, Diagnostic, EffectId, ExternalRef, RunId, TaskId,
};
use kiln_core::workflow::{
    WorkflowControlRequest, WorkflowEffectPlan, WorkflowLogChunk, WorkflowLogReadRequest,
    WorkflowObservation,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderRequest {
    pub effect_id: EffectId,
    pub idempotency_key: ExternalRef,
    pub payload_identity: ExternalRef,
}

pub trait ProviderPort {
    fn profile(&self) -> &CapabilityProfile;
    fn dispatch(&mut self, request: &ProviderRequest) -> CoreResult<ProviderObservation>;
    fn reconcile(&mut self, effect_id: &EffectId) -> CoreResult<ProviderObservation>;
}

pub trait SourcePort: ProviderPort {}
pub trait CollaborationPort: ProviderPort {}
pub trait TriggerPort: ProviderPort {}
pub trait StatusPort: ProviderPort {}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WorkflowExchangeOperation {
    Dispatch,
    RecoverDispatch,
    Observe,
    Cancel,
    Pause,
    Resume,
    ReadLog,
    Reconcile,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkflowExchangeRequest {
    pub operation: WorkflowExchangeOperation,
    pub body_identity: ContentId,
    pub body: Vec<u8>,
    pub max_response_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkflowExchangeResponse {
    pub body: Vec<u8>,
    pub log_bytes: Vec<u8>,
    pub disconnected_after_acceptance: bool,
}

pub trait WorkflowExchangePort {
    fn exchange(
        &mut self,
        request: WorkflowExchangeRequest,
    ) -> PortResult<WorkflowExchangeResponse>;
}

pub trait WorkflowPort {
    fn profile(&self) -> &CapabilityProfile;
    fn dispatch(&mut self, plan: &WorkflowEffectPlan) -> PortResult<WorkflowObservation>;
    fn recover_dispatch(&mut self, plan: &WorkflowEffectPlan) -> PortResult<WorkflowObservation>;
    fn observe(&mut self, effect_id: &EffectId) -> PortResult<WorkflowObservation>;
    fn control(&mut self, request: &WorkflowControlRequest) -> PortResult<WorkflowObservation>;
    fn read_log(&mut self, request: &WorkflowLogReadRequest) -> PortResult<WorkflowLogChunk>;
    fn reconcile(&mut self, effect_id: &EffectId) -> PortResult<WorkflowObservation>;
}

pub trait DiscoveryPort {
    fn profile(&self) -> &CapabilityProfile;
    fn discover(&mut self, plan: &DiscoveryEffectPlan) -> PortResult<DiscoveryObservation>;
    fn discover_with_cancellation(
        &mut self,
        plan: &DiscoveryEffectPlan,
        cancellation: &AtomicBool,
    ) -> PortResult<DiscoveryObservation>;
    fn observe(&mut self, effect_id: &EffectId) -> PortResult<DiscoveryObservation>;
    fn reconcile(&mut self, plan: &DiscoveryEffectPlan) -> PortResult<DiscoveryObservation>;
    fn handoff_gc_roots(
        &mut self,
        plan: &DiscoveryEffectPlan,
        durable_tasks: &BTreeSet<TaskId>,
    ) -> PortResult<Vec<TaskProposal>>;
    fn cleanup_gc_roots(&mut self, plan: &DiscoveryEffectPlan) -> PortResult<DiscoveryObservation>;
}

pub trait BuildPort {
    fn profile(&self) -> &CapabilityProfile;
    fn dispatch(&mut self, plan: &BuildEffectPlan) -> PortResult<BuildEvaluationObservation>;
    fn dispatch_with_cancellation(
        &mut self,
        plan: &BuildEffectPlan,
        cancellation: &AtomicBool,
    ) -> PortResult<BuildEvaluationObservation>;
    fn observe(&mut self, effect_id: &EffectId) -> PortResult<BuildEvaluationObservation>;
    fn read_log(&mut self, request: &BuildLogReadRequest) -> PortResult<BuildLogChunk>;
    fn reconcile(&mut self, plan: &BuildEffectPlan) -> PortResult<BuildEvaluationObservation>;
}

// r[impl kiln.providers.port_ownership]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SourceRepositoryBackend {
    Git,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceRepositoryObservation {
    pub repository: ExternalRef,
    pub backend: SourceRepositoryBackend,
    pub operation: Option<ExternalRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceResolutionObservation {
    pub symbolic: ExternalRef,
    pub native_change: Option<ExternalRef>,
    pub exact_revisions: Vec<ExternalRef>,
    pub alias: Option<Alias>,
    pub conflicted_alias: bool,
    pub divergent_change: bool,
}

pub trait SourceRepositoryPort {
    fn observe(&self) -> PortResult<SourceRepositoryObservation>;
    fn resolve(&self, symbolic: &ExternalRef) -> PortResult<SourceResolutionObservation>;
    fn acquire_canonical_manifest(&self, commit: &ExternalRef) -> PortResult<Vec<u8>>;
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PijulReadRequest {
    pub channel: ExternalRef,
    pub limits: PijulSnapshotLimits,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PijulReadObservation {
    pub repository: ExternalRef,
    pub channel: ExternalRef,
    pub observed_state_before: ExternalRef,
    pub observed_state_after: ExternalRef,
    pub change_sequence: Vec<ExternalRef>,
    pub format_cohort: ExternalRef,
    pub entries: Vec<PijulSnapshotEntry>,
}

// r[impl kiln.source.pijul_adapter]
pub trait PijulRepositoryPort {
    fn discover_channels(&self, max_channels: usize) -> PortResult<Vec<ExternalRef>>;
    fn resolve_and_acquire(&self, request: &PijulReadRequest) -> PortResult<PijulReadObservation>;
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CollaborationMutationOutcome {
    Applied(Box<CobObservation>),
    Denied,
    Cancelled,
    TimedOut,
    ProviderError,
    Unknown,
}

pub trait CollaborationMutationPort {
    fn observe(&mut self, plan: &CobEffectPlan) -> PortResult<CobObservation>;
    fn acquire_signer(&mut self, actor: &ExternalRef) -> PortResult<()>;
    fn preview(&mut self, plan: &CobEffectPlan) -> PortResult<()>;
    fn dispatch(&mut self, plan: &CobEffectPlan) -> PortResult<CollaborationMutationOutcome>;
    fn reconcile(&mut self, plan: &CobEffectPlan) -> PortResult<CollaborationMutationOutcome>;
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RemoteTransportObservation {
    Observation(Box<RemoteObservation>),
    DisconnectedAfterAcceptance {
        action: ExternalRef,
        operation: Option<ExternalRef>,
    },
}

pub trait RemoteExecutionPort {
    fn upload(&mut self, action: &RemoteAction) -> PortResult<()>;
    fn execute(&mut self, action: &RemoteAction) -> PortResult<RemoteTransportObservation>;
    fn cancel(&mut self, operation: &ExternalRef) -> PortResult<RemoteTransportObservation>;
}

pub trait RemoteLifecyclePort: RemoteExecutionPort {
    fn poll(&mut self, operation: &ExternalRef) -> PortResult<RemoteTransportObservation>;
    fn wait(&mut self, operation: &ExternalRef) -> PortResult<RemoteTransportObservation>;
    fn download(&mut self, output: &ExternalRef, max_bytes: usize) -> PortResult<Vec<u8>>;
    fn read_log(&mut self, log: &ExternalRef, max_bytes: usize) -> PortResult<Vec<u8>>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerEventKind {
    Push,
    Patch,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TriggerRequest {
    pub repository: ExternalRef,
    pub actor: ExternalRef,
    pub before: Option<ExternalRef>,
    pub after: ExternalRef,
    pub branch: Option<Alias>,
    pub patch: Option<ExternalRef>,
    pub event: TriggerEventKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerTerminal {
    Success,
    Failure,
    Denied,
    Cancelled,
    TimedOut,
    ProviderError,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TriggerRun {
    pub run_id: RunId,
    pub terminal: TriggerTerminal,
    pub log_url: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerWaitMode {
    Wait,
    Poll,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TriggerLifecyclePolicy {
    pub mode: TriggerWaitMode,
    pub max_polls: usize,
}

pub trait TriggerIntakePort {
    fn accept(&mut self, trigger: TriggerRequest) -> PortResult<TriggerRun>;
    fn observe(
        &mut self,
        accepted: &TriggerRun,
        policy: TriggerLifecyclePolicy,
    ) -> PortResult<TriggerRun>;
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EventAppendRequest {
    pub stream: ExternalRef,
    pub expected_revision: u64,
    pub records: Vec<JournalRecord>,
    pub outbox: Vec<OutboxRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EventStoreSnapshot {
    pub records: Vec<JournalRecord>,
    pub outbox: Vec<OutboxRecord>,
}

pub trait EventStorePort {
    fn append(&mut self, request: &EventAppendRequest) -> Result<u64, PortFailure>;

    fn load(
        &self,
        stream: &ExternalRef,
        max_records: usize,
    ) -> Result<EventStoreSnapshot, PortFailure>;
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationRequest {
    pub effect_id: EffectId,
    pub run_id: RunId,
    pub outcome: Outcome,
    pub notification_policy: ExternalRef,
    pub endpoint_profile: ExternalRef,
    pub body_identity: ContentId,
    pub idempotency_key: ExternalRef,
    pub body: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum NotificationOutcome {
    Accepted,
    Rejected,
    RateLimited,
    TimedOut,
    ProviderError,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationObservation {
    pub effect_id: EffectId,
    pub endpoint_profile: ExternalRef,
    pub body_identity: ContentId,
    pub outcome: NotificationOutcome,
    pub external_reference: Option<ExternalRef>,
    pub retryable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InfrastructureFailure {
    pub code: String,
    pub message: String,
}

// r[impl kiln.providers.error_ownership]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PortFailure {
    Domain(Diagnostic),
    Infrastructure(InfrastructureFailure),
    Retryable(InfrastructureFailure),
    TimedOut(InfrastructureFailure),
    Cancelled(InfrastructureFailure),
    Unknown(InfrastructureFailure),
}

pub type PortResult<T> = Result<T, PortFailure>;

impl PortFailure {
    #[must_use]
    pub const fn disposition(&self) -> ProviderFailureDisposition {
        let class = match self {
            Self::Domain(_) => ProviderFailureClass::Domain,
            Self::Infrastructure(_) => ProviderFailureClass::Infrastructure,
            Self::Retryable(_) => ProviderFailureClass::RetryableInfrastructure,
            Self::TimedOut(_) => ProviderFailureClass::TimedOut,
            Self::Cancelled(_) => ProviderFailureClass::Cancelled,
            Self::Unknown(_) => ProviderFailureClass::Unknown,
        };
        classify_provider_failure(class)
    }
}

impl From<Diagnostic> for PortFailure {
    fn from(error: Diagnostic) -> Self {
        Self::Domain(error)
    }
}

pub trait NotificationPort {
    fn deliver(
        &mut self,
        request: &NotificationRequest,
    ) -> Result<NotificationObservation, PortFailure>;

    fn reconcile(&mut self, effect_id: &EffectId) -> Result<NotificationObservation, PortFailure>;
}

pub trait EvidencePort: ProviderPort {}

pub trait GatePort {
    fn profile(&self) -> &CapabilityProfile;
    fn evaluate(&mut self, plan: &GateEffectPlan) -> PortResult<GateEvaluationObservation>;
    fn reconcile(&mut self, plan: &GateEffectPlan) -> PortResult<GateEvaluationObservation>;
}

pub fn role_matches_port(profile: &CapabilityProfile, expected: ProviderRole) -> CoreResult<()> {
    if profile.role != expected {
        return Err(kiln_core::Diagnostic::new(
            "provider_role_mismatch",
            "provider.role",
            "provider profile does not match the selected narrow port",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_remain_distinct() {
        assert_ne!(ProviderRole::Source, ProviderRole::Collaboration);
        assert_ne!(ProviderRole::Discovery, ProviderRole::Workflow);
        assert_ne!(ProviderRole::Discovery, ProviderRole::Build);
        assert_ne!(ProviderRole::Workflow, ProviderRole::Build);
    }

    // r[verify kiln.providers.error_ownership]
    #[test]
    fn infrastructure_outcomes_remain_distinct() {
        let failure = InfrastructureFailure {
            code: "fixture".to_string(),
            message: "fixture infrastructure result".to_string(),
        };
        assert_ne!(
            PortFailure::TimedOut(failure.clone()),
            PortFailure::Cancelled(failure.clone())
        );
        assert_ne!(
            PortFailure::Infrastructure(failure.clone()),
            PortFailure::Unknown(failure.clone())
        );
        assert_eq!(
            PortFailure::Infrastructure(failure.clone()).disposition(),
            ProviderFailureDisposition::ProviderFailure
        );
        assert_eq!(
            PortFailure::Retryable(failure.clone()).disposition(),
            ProviderFailureDisposition::RetryInfrastructure
        );
        assert_eq!(
            PortFailure::Unknown(failure).disposition(),
            ProviderFailureDisposition::ReconcileUnknown
        );
    }
}
