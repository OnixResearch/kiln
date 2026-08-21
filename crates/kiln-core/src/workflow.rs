use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::domain::Outcome;
use crate::types::{
    Alias, AttemptId, CandidateId, ContentId, EffectId, ExternalRef, PipelineRevisionId, PolicyId,
    ProviderProfileId,
};

mod admission;

pub use admission::{admit_workflow_observation, validate_workflow_plan};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum WorkflowEffectClass {
    Read,
    Build,
    Test,
    Publish,
    Notify,
    Custom,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkflowNode {
    pub id: Alias,
    pub kind: Alias,
    pub effect_class: WorkflowEffectClass,
    pub adapter_kind: Alias,
    pub operation_identity: ContentId,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct WorkflowEdge {
    pub source: Alias,
    pub target: Alias,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkflowControls {
    pub cancel: bool,
    pub pause: bool,
    pub resume: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkflowBounds {
    pub max_nodes: usize,
    pub max_edges: usize,
    pub max_log_references: usize,
    pub max_log_read_bytes: usize,
    pub max_total_log_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkflowSourceContext {
    pub provider: Alias,
    pub repository: ExternalRef,
    pub revision: ExternalRef,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkflowEffectPlan {
    pub effect_id: EffectId,
    pub attempt_id: AttemptId,
    pub candidate_id: CandidateId,
    pub pipeline_revision_id: PipelineRevisionId,
    pub workflow_policy_id: PolicyId,
    pub provider_profile: ProviderProfileId,
    pub idempotency_key: ContentId,
    pub platform: Alias,
    pub source: Option<WorkflowSourceContext>,
    pub nodes: Vec<WorkflowNode>,
    pub edges: Vec<WorkflowEdge>,
    pub controls: WorkflowControls,
    pub bounds: WorkflowBounds,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WorkflowState {
    Accepted,
    Queued,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
    Unknown,
}

impl WorkflowState {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WorkflowNodeState {
    Pending,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
    Skipped,
    Unknown,
}

impl WorkflowNodeState {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Cancelled | Self::Skipped
        )
    }

    #[must_use]
    pub const fn is_successful(self) -> bool {
        matches!(self, Self::Completed | Self::Skipped)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkflowNodeObservation {
    pub node_id: Alias,
    pub state: WorkflowNodeState,
    pub terminal: bool,
    pub successful: bool,
    pub adapter: ExternalRef,
    pub receipt: Option<ExternalRef>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum WorkflowLogStream {
    Stdout,
    Stderr,
    Diagnostic,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkflowLogReference {
    pub node_id: Alias,
    pub stream: WorkflowLogStream,
    pub reference: ExternalRef,
    pub cursor: u64,
    pub bytes: usize,
    pub truncated: bool,
    pub gap_before: bool,
    pub retained: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkflowObservation {
    pub effect_id: EffectId,
    pub attempt_id: AttemptId,
    pub candidate_id: CandidateId,
    pub pipeline_revision_id: PipelineRevisionId,
    pub workflow_policy_id: PolicyId,
    pub provider_profile: ProviderProfileId,
    pub idempotency_key: ContentId,
    pub workflow: ExternalRef,
    pub execution: ExternalRef,
    pub sequence: u64,
    pub state: WorkflowState,
    pub terminal: bool,
    pub successful: bool,
    pub outcome: Outcome,
    pub nodes: Vec<WorkflowNodeObservation>,
    pub logs: Vec<WorkflowLogReference>,
    pub receipt: Option<ExternalRef>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WorkflowControlAction {
    Cancel,
    Pause,
    Resume,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkflowControlRequest {
    pub effect_id: EffectId,
    pub execution: ExternalRef,
    pub expected_sequence: u64,
    pub action: WorkflowControlAction,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkflowLogReadRequest {
    pub effect_id: EffectId,
    pub execution: ExternalRef,
    pub node_id: Alias,
    pub stream: WorkflowLogStream,
    pub cursor: u64,
    pub max_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkflowLogChunk {
    pub reference: WorkflowLogReference,
    pub bytes: Vec<u8>,
}

#[cfg(test)]
mod tests;
