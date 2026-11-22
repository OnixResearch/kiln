use serde::{Deserialize, Serialize};
// This consumer bundle carries the four original producer JSON values. It is
// not a replacement producer observation schema.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Bundle {
    pub profile: Profile,
    pub request: Request,
    pub accepted: Accepted,
    pub observation: Observation,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Profile {
    pub schema: String,
    pub graph_schema: String,
    pub platforms: Vec<String>,
    pub node_kinds: Vec<String>,
    pub effect_classes: Vec<String>,
    pub adapter_kinds: Vec<String>,
    pub controls: Controls,
    pub limits: Limits,
    pub non_claims: Vec<String>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Controls {
    pub cancel: bool,
    pub pause: bool,
    pub resume: bool,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Limits {
    pub maximum_nodes: u64,
    pub maximum_edges: u64,
    pub maximum_log_references: u64,
    pub maximum_log_read_bytes: u64,
    pub maximum_total_log_bytes: u64,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ObservationPolicy {
    pub maximum_log_references: u64,
    pub maximum_log_read_bytes: u64,
    pub maximum_total_log_bytes: u64,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Request {
    pub schema: String,
    pub identity: String,
    pub idempotency_key: String,
    pub profile_identity: String,
    pub platform: String,
    pub kiln: Linkage,
    pub source: Option<Source>,
    pub graph: Graph,
    pub controls: Controls,
    pub observation: ObservationPolicy,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Linkage {
    pub effect_identity: String,
    pub attempt_identity: String,
    pub candidate_identity: String,
    pub pipeline_revision_identity: String,
    pub workflow_policy_identity: String,
}
impl Linkage {
    pub fn values(&self) -> [&str; 5] {
        [
            &self.effect_identity,
            &self.attempt_identity,
            &self.candidate_identity,
            &self.pipeline_revision_identity,
            &self.workflow_policy_identity,
        ]
    }
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Source {
    pub provider: String,
    pub repository: String,
    pub revision: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Graph {
    pub schema: String,
    pub identity: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Node {
    pub id: String,
    pub kind: String,
    pub effect_class: String,
    pub adapter_kind: String,
    pub operation_identity: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Edge {
    pub source: String,
    pub target: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Accepted {
    pub schema: String,
    pub request_identity: String,
    pub idempotency_key: String,
    pub workflow_identity: String,
    pub execution_identity: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Observation {
    pub schema: String,
    pub identity: String,
    pub request_identity: String,
    pub idempotency_key: String,
    pub workflow_identity: String,
    pub execution_identity: String,
    pub sequence: u64,
    pub state: String,
    pub terminal: bool,
    pub successful: bool,
    pub nodes: Vec<NodeObservation>,
    pub logs: Vec<Log>,
    pub receipt_identity: Option<String>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NodeObservation {
    pub node_id: String,
    pub state: String,
    pub terminal: bool,
    pub successful: bool,
    pub adapter_identity: String,
    pub receipt_identity: Option<String>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Log {
    pub node_id: String,
    pub stream: String,
    pub reference_identity: String,
    pub cursor: u64,
    pub bytes: u64,
    pub truncated: bool,
    pub gap_before: bool,
    pub retained: bool,
}
