// Octet: framed identities keep closed wire-enum matches and compatibility names explicit for
// stable consumer bytes.
#![allow(
    unknown_lints,
    non_trait_imports,
    ambiguous_params,
    numeric_units,
    path_segment_repetition,
    assertion_density,
    missing_const_fn,
    fragile_exhaustive_enum_match,
    reason = "field labels disambiguate frame parameters; compatibility identity names and exhaustive wire enums are contract bytes"
)]

use crate::model::CapabilityProfile;
use crate::model::EffectClass;
use crate::model::ExecutionObservation;
use crate::model::ExecutionState;
use crate::model::Identity;
use crate::model::LogStream;
use crate::model::NodeState;
use crate::model::WorkflowGraph;
use crate::model::WorkflowRequest;

const PROFILE_IDENTITY_DOMAIN: &str = "lattice.workflow-profile.identity.v1";
const GRAPH_IDENTITY_DOMAIN: &str = "lattice.workflow-graph.identity.v1";
const REQUEST_IDENTITY_DOMAIN: &str = "lattice.workflow-request.identity.v2";
const OBSERVATION_IDENTITY_DOMAIN: &str = "lattice.workflow-observation.identity.v1";

struct Frame(blake3::Hasher);

impl Frame {
    fn new(domain: &str) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(domain.as_bytes());
        Self(hasher)
    }

    fn bytes(&mut self, label: &str, value: &[u8]) {
        self.length_prefixed(label.as_bytes());
        self.length_prefixed(value);
    }

    fn string(&mut self, label: &str, value: &str) {
        self.bytes(label, value.as_bytes());
    }

    fn identity(&mut self, label: &str, value: &Identity) {
        self.string(label, value.as_str());
    }

    fn boolean(&mut self, label: &str, value: bool) {
        self.bytes(label, &[u8::from(value)]);
    }

    fn number(&mut self, label: &str, value: u64) {
        self.bytes(label, &value.to_le_bytes());
    }

    fn count(&mut self, label: &str, value: usize) {
        match u64::try_from(value) {
            Ok(value) => self.number(label, value),
            Err(_) => self.string(label, "unrepresentable-count"),
        }
    }

    fn length_prefixed(&mut self, bytes: &[u8]) {
        match u64::try_from(bytes.len()) {
            Ok(length) => {
                self.0.update(&length.to_le_bytes());
                self.0.update(bytes);
            }
            Err(_) => {
                self.0.update(&0_u64.to_le_bytes());
                self.0.update(b"unrepresentable-length");
            }
        }
    }

    fn finish(self) -> Identity {
        Identity::from_digest(self.0.finalize())
    }
}

#[must_use]
pub fn profile_identity(profile: &CapabilityProfile) -> Identity {
    let mut frame = Frame::new(PROFILE_IDENTITY_DOMAIN);
    frame.string("schema", &profile.schema);
    frame.string("graph_schema", &profile.graph_schema);
    frame.count("platform_count", profile.platforms.len());
    for platform in &profile.platforms {
        frame.string("platform", platform.as_str());
    }
    frame.count("node_kind_count", profile.node_kinds.len());
    for kind in &profile.node_kinds {
        frame.string("node_kind", kind.as_str());
    }
    frame.count("effect_class_count", profile.effect_classes.len());
    for effect_class in &profile.effect_classes {
        frame.string("effect_class", effect_class_name(*effect_class));
    }
    frame.count("adapter_kind_count", profile.adapter_kinds.len());
    for kind in &profile.adapter_kinds {
        frame.string("adapter_kind", kind.as_str());
    }
    frame.boolean("cancel", profile.controls.cancel);
    frame.boolean("pause", profile.controls.pause);
    frame.boolean("resume", profile.controls.resume);
    frame.number("maximum_nodes", u64::from(profile.limits.maximum_nodes));
    frame.number("maximum_edges", u64::from(profile.limits.maximum_edges));
    frame.number("maximum_log_references", u64::from(profile.limits.maximum_log_references));
    frame.number("maximum_log_read_bytes", profile.limits.maximum_log_read_bytes);
    frame.number("maximum_total_log_bytes", profile.limits.maximum_total_log_bytes);
    frame.count("non_claim_count", profile.non_claims.len());
    for non_claim in &profile.non_claims {
        frame.string("non_claim", non_claim);
    }
    frame.finish()
}

#[must_use]
pub fn graph_identity(graph: &WorkflowGraph) -> Identity {
    let mut frame = Frame::new(GRAPH_IDENTITY_DOMAIN);
    frame.string("schema", &graph.schema);
    frame.count("node_count", graph.nodes.len());
    for node in &graph.nodes {
        frame.string("node_id", node.id.as_str());
        frame.string("node_kind", node.kind.as_str());
        frame.string("effect_class", effect_class_name(node.effect_class));
        frame.string("adapter_kind", node.adapter_kind.as_str());
        frame.identity("operation_identity", &node.operation_identity);
    }
    frame.count("edge_count", graph.edges.len());
    for edge in &graph.edges {
        frame.string("edge_source", edge.source.as_str());
        frame.string("edge_target", edge.target.as_str());
    }
    frame.finish()
}

#[must_use]
pub fn request_identity(request: &WorkflowRequest) -> Identity {
    let mut frame = Frame::new(REQUEST_IDENTITY_DOMAIN);
    frame.string("schema", &request.schema);
    frame.identity("idempotency_key", &request.idempotency_key);
    frame.identity("profile_identity", &request.profile_identity);
    frame.string("platform", request.platform.as_str());
    frame.identity("effect_identity", &request.kiln.effect_identity);
    frame.identity("attempt_identity", &request.kiln.attempt_identity);
    frame.identity("candidate_identity", &request.kiln.candidate_identity);
    frame.identity("pipeline_revision_identity", &request.kiln.pipeline_revision_identity);
    frame.identity("workflow_policy_identity", &request.kiln.workflow_policy_identity);
    frame.boolean("has_source_context", request.source.is_some());
    if let Some(source) = &request.source {
        frame.string("source_provider", source.provider.as_str());
        frame.string("source_repository", source.repository.as_str());
        frame.string("source_revision", source.revision.as_str());
    }
    frame.identity("graph_identity", &request.graph.identity);
    frame.boolean("control_cancel", request.controls.cancel);
    frame.boolean("control_pause", request.controls.pause);
    frame.boolean("control_resume", request.controls.resume);
    frame.number("maximum_log_references", u64::from(request.observation.maximum_log_references));
    frame.number("maximum_log_read_bytes", request.observation.maximum_log_read_bytes);
    frame.number("maximum_total_log_bytes", request.observation.maximum_total_log_bytes);
    frame.finish()
}

#[must_use]
pub fn observation_identity(observation: &ExecutionObservation) -> Identity {
    let mut frame = Frame::new(OBSERVATION_IDENTITY_DOMAIN);
    frame.string("schema", &observation.schema);
    frame.identity("request_identity", &observation.request_identity);
    frame.identity("idempotency_key", &observation.idempotency_key);
    frame.identity("workflow_identity", &observation.workflow_identity);
    frame.identity("execution_identity", &observation.execution_identity);
    frame.number("sequence", observation.sequence);
    frame.string("state", execution_state_name(observation.state));
    frame.boolean("terminal", observation.terminal);
    frame.boolean("successful", observation.successful);
    frame.count("node_count", observation.nodes.len());
    for node in &observation.nodes {
        frame.string("node_id", node.node_id.as_str());
        frame.string("node_state", node_state_name(node.state));
        frame.boolean("node_terminal", node.terminal);
        frame.boolean("node_successful", node.successful);
        frame.identity("adapter_identity", &node.adapter_identity);
        frame.boolean("has_node_receipt", node.receipt_identity.is_some());
        if let Some(receipt) = &node.receipt_identity {
            frame.identity("node_receipt_identity", receipt);
        }
    }
    frame.count("log_count", observation.logs.len());
    for log in &observation.logs {
        frame.string("log_node_id", log.node_id.as_str());
        frame.string("log_stream", log_stream_name(log.stream));
        frame.identity("log_reference_identity", &log.reference_identity);
        frame.number("log_cursor", log.cursor);
        frame.number("log_bytes", log.bytes);
        frame.boolean("log_truncated", log.truncated);
        frame.boolean("log_gap_before", log.gap_before);
        frame.boolean("log_retained", log.retained);
    }
    frame.boolean("has_receipt", observation.receipt_identity.is_some());
    if let Some(receipt) = &observation.receipt_identity {
        frame.identity("receipt_identity", receipt);
    }
    frame.finish()
}

pub(crate) const fn effect_class_name(value: EffectClass) -> &'static str {
    match value {
        EffectClass::Read => "read",
        EffectClass::Build => "build",
        EffectClass::Test => "test",
        EffectClass::Publish => "publish",
        EffectClass::Notify => "notify",
        EffectClass::Custom => "custom",
    }
}

pub(crate) const fn execution_state_name(value: ExecutionState) -> &'static str {
    match value {
        ExecutionState::Accepted => "accepted",
        ExecutionState::Queued => "queued",
        ExecutionState::Running => "running",
        ExecutionState::Paused => "paused",
        ExecutionState::Completed => "completed",
        ExecutionState::Failed => "failed",
        ExecutionState::Cancelled => "cancelled",
        ExecutionState::Unknown => "unknown",
    }
}

pub(crate) const fn node_state_name(value: NodeState) -> &'static str {
    match value {
        NodeState::Pending => "pending",
        NodeState::Running => "running",
        NodeState::Paused => "paused",
        NodeState::Completed => "completed",
        NodeState::Failed => "failed",
        NodeState::Cancelled => "cancelled",
        NodeState::Skipped => "skipped",
        NodeState::Unknown => "unknown",
    }
}

pub(crate) const fn log_stream_name(value: LogStream) -> &'static str {
    match value {
        LogStream::Stdout => "stdout",
        LogStream::Stderr => "stderr",
        LogStream::Diagnostic => "diagnostic",
    }
}
