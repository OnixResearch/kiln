// Octet: wire observation values use owned collections so consumers can retain exact bounded
// provider facts.
#![allow(
    unknown_lints,
    non_trait_imports,
    path_segment_repetition,
    reason = "owned interchange types retain version-one observation names used by independent consumers"
)]

use alloc::string::String;
use alloc::vec::Vec;

use super::Identity;
use super::Label;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionState {
    Accepted,
    Queued,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
    Unknown,
}

impl ExecutionState {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeState {
    Pending,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
    Skipped,
    Unknown,
}

impl NodeState {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled | Self::Skipped)
    }

    #[must_use]
    pub const fn is_successful(self) -> bool {
        matches!(self, Self::Completed | Self::Skipped)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeObservation {
    pub node_id: Label,
    pub state: NodeState,
    pub terminal: bool,
    pub successful: bool,
    pub adapter_identity: Identity,
    pub receipt_identity: Option<Identity>,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize
)]
#[serde(rename_all = "snake_case")]
pub enum LogStream {
    Stdout,
    Stderr,
    Diagnostic,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogReference {
    pub node_id: Label,
    pub stream: LogStream,
    pub reference_identity: Identity,
    pub cursor: u64,
    pub bytes: u64,
    pub truncated: bool,
    pub gap_before: bool,
    pub retained: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionObservation {
    pub schema: String,
    pub identity: Identity,
    pub request_identity: Identity,
    pub idempotency_key: Identity,
    pub workflow_identity: Identity,
    pub execution_identity: Identity,
    pub sequence: u64,
    pub state: ExecutionState,
    pub terminal: bool,
    pub successful: bool,
    pub nodes: Vec<NodeObservation>,
    pub logs: Vec<LogReference>,
    pub receipt_identity: Option<Identity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlAction {
    Cancel,
    Pause,
    Resume,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlRequest {
    pub schema: String,
    pub request_identity: Identity,
    pub workflow_identity: Identity,
    pub execution_identity: Identity,
    pub expected_sequence: u64,
    pub action: ControlAction,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogReadRequest {
    pub schema: String,
    pub request_identity: Identity,
    pub workflow_identity: Identity,
    pub execution_identity: Identity,
    pub node_id: Label,
    pub stream: LogStream,
    pub cursor: u64,
    pub maximum_bytes: u64,
}
