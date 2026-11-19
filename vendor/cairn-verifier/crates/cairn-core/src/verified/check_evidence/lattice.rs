//! Consumer-owned JSON adapter for the pinned Lattice workflow contract.
//! Wire admission is bounded independently of producer-declared limits. Native
//! workflow success (including skipped nodes) never becomes test coverage.
mod identity;
mod model;
#[cfg(test)]
mod tests;
use super::{ProducerIssue, MAX_CHECKS};
use crate::canonical_json_hash;
use model::*;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeSet;

pub const LATTICE_REVISION: &str = "70496e67c7fd4a8b05914161a8e09de2759bebc8";
const NON_CLAIMS: [&str; 5] = [
    "workflow admission does not prove workflow correctness",
    "workflow admission does not prove host confinement",
    "workflow observation does not prove an external effect succeeded",
    "workflow observation does not prove credential freshness",
    "workflow observation does not establish release readiness",
];
const EFFECTS: [&str; 6] = ["read", "build", "test", "publish", "notify", "custom"];
const STREAMS: [&str; 3] = ["stdout", "stderr", "diagnostic"];
const EXECUTION_STATES: [&str; 8] = [
    "accepted",
    "queued",
    "running",
    "paused",
    "completed",
    "failed",
    "cancelled",
    "unknown",
];
const NODE_STATES: [&str; 8] = [
    "pending",
    "running",
    "paused",
    "completed",
    "failed",
    "cancelled",
    "skipped",
    "unknown",
];
const LABEL_BYTES: usize = 128;
const SOURCE_BYTES: usize = 256;
const DIGEST_DIGITS: usize = 64;
const CAPABILITY_ITEMS: u64 = 256;
const NATIVE_NODES: u64 = 4096;
const NATIVE_EDGES: u64 = 16384;
const NATIVE_LOG_REFERENCES: u64 = 4096;
const NATIVE_LOG_READ_BYTES: u64 = 16777216;
const NATIVE_TOTAL_LOG_BYTES: u64 = 268435456;

#[derive(Debug, Serialize)]
pub struct LatticeAdmission {
    pub producer_revision: &'static str,
    pub native_identity: String,
    pub native_status: String,
    pub native_node_states: Vec<String>,
    pub native_non_claims: Vec<String>,
    pub native_input_hash: String,
    pub completion_eligible: bool,
}

// r[impl cairn.check_evidence_producer.adapters]
pub fn admit_lattice_json(value: &Value) -> Result<LatticeAdmission, ProducerIssue> {
    let bundle: Bundle =
        serde_json::from_value(value.clone()).map_err(|_| ProducerIssue::Observation)?;
    validate_profile(&bundle.profile)?;
    validate_request(&bundle.profile, &bundle.request)?;
    let a = &bundle.accepted;
    let r = &bundle.request;
    let o = &bundle.observation;
    require(
        a.schema == "lattice.workflow-accepted.v1"
            && a.request_identity == r.identity
            && a.idempotency_key == r.idempotency_key
            && digest(&a.workflow_identity)
            && digest(&a.execution_identity),
    )?;
    require(
        o.schema == "lattice.workflow-observation.v1"
            && o.request_identity == a.request_identity
            && o.idempotency_key == a.idempotency_key
            && o.workflow_identity == a.workflow_identity
            && o.execution_identity == a.execution_identity
            && o.nodes.len() == r.graph.nodes.len()
            && o.nodes.len() <= MAX_CHECKS,
    )?;
    for (planned, node) in r.graph.nodes.iter().zip(&o.nodes) {
        require(
            node.node_id == planned.id
                && NODE_STATES.contains(&node.state.as_str())
                && node.terminal == (terminal(&node.state) || node.state == "skipped")
                && node.successful == matches!(node.state.as_str(), "completed" | "skipped")
                && terminal(&node.state) == node.receipt_identity.is_some()
                && digest(&node.adapter_identity)
                && optional_digest(&node.receipt_identity),
        )?;
    }
    require(
        EXECUTION_STATES.contains(&o.state.as_str())
            && o.terminal == terminal(&o.state)
            && o.receipt_identity.is_some() == o.terminal
            && optional_digest(&o.receipt_identity),
    )?;
    let all_terminal = o.nodes.iter().all(|node| node.terminal);
    let consistent = match o.state.as_str() {
        "completed" => all_terminal && o.successful && o.nodes.iter().all(|node| node.successful),
        "failed" => {
            all_terminal && !o.successful && o.nodes.iter().any(|node| node.state == "failed")
        }
        "cancelled" => {
            all_terminal && !o.successful && o.nodes.iter().any(|node| node.state == "cancelled")
        }
        _ => !o.terminal && !o.successful,
    };
    require(consistent)?;
    require(
        o.logs.len() <= MAX_CHECKS && o.logs.len() as u64 <= r.observation.maximum_log_references,
    )?;
    let mut total = 0_u64;
    let mut previous = None;
    for log in &o.logs {
        let stream = STREAMS
            .iter()
            .position(|value| *value == log.stream)
            .ok_or(ProducerIssue::Observation)?;
        let key = (&log.node_id, stream, log.cursor);
        require(
            previous.is_none_or(|old| old < key)
                && r.graph.nodes.iter().any(|node| node.id == log.node_id)
                && digest(&log.reference_identity)
                && log.bytes <= r.observation.maximum_log_read_bytes,
        )?;
        previous = Some(key);
        total = total.checked_add(log.bytes).ok_or(ProducerIssue::Bounds)?;
    }
    require(
        total <= r.observation.maximum_total_log_bytes && o.identity == identity::observation(o),
    )?;
    Ok(LatticeAdmission {
        producer_revision: LATTICE_REVISION,
        native_identity: o.identity.clone(),
        native_status: o.state.clone(),
        native_node_states: o.nodes.iter().map(|node| node.state.clone()).collect(),
        native_non_claims: bundle.profile.non_claims,
        native_input_hash: canonical_json_hash(value),
        completion_eligible: false,
    })
}
fn validate_profile(p: &Profile) -> Result<(), ProducerIssue> {
    require(
        p.schema == "lattice.workflow-profile.v1" && p.graph_schema == "lattice.workflow-graph.v1",
    )?;
    for values in [&p.platforms, &p.node_kinds, &p.adapter_kinds] {
        require(
            !values.is_empty()
                && values.len() as u64 <= CAPABILITY_ITEMS
                && values.iter().all(|v| label(v))
                && ordered(values),
        )?;
    }
    let effects = p
        .effect_classes
        .iter()
        .map(|effect| EFFECTS.iter().position(|v| v == effect))
        .collect::<Option<Vec<_>>>()
        .ok_or(ProducerIssue::Observation)?;
    require(!effects.is_empty() && effects.len() as u64 <= CAPABILITY_ITEMS && ordered(&effects))?;
    for (observed, maximum) in [
        (p.limits.maximum_nodes, NATIVE_NODES),
        (p.limits.maximum_edges, NATIVE_EDGES),
        (p.limits.maximum_log_references, NATIVE_LOG_REFERENCES),
        (p.limits.maximum_log_read_bytes, NATIVE_LOG_READ_BYTES),
        (p.limits.maximum_total_log_bytes, NATIVE_TOTAL_LOG_BYTES),
    ] {
        require(observed > 0 && observed <= maximum)?;
    }
    require(
        p.limits.maximum_log_read_bytes <= p.limits.maximum_total_log_bytes
            && p.non_claims == NON_CLAIMS,
    )
}
fn validate_request(p: &Profile, r: &Request) -> Result<(), ProducerIssue> {
    require(
        r.schema == "lattice.workflow-request.v2"
            && r.graph.schema == "lattice.workflow-graph.v1"
            && r.profile_identity == identity::profile(p)
            && p.platforms.contains(&r.platform)
            && digest(&r.idempotency_key)
            && r.kiln.values().iter().all(|v| digest(v))
            && (!r.controls.cancel || p.controls.cancel)
            && (!r.controls.pause || p.controls.pause)
            && (!r.controls.resume || p.controls.resume),
    )?;
    if let Some(source) = &r.source {
        require(
            label(&source.provider)
                && source_value(&source.repository)
                && source_value(&source.revision),
        )?;
    }
    for (observed, maximum) in [
        (
            r.observation.maximum_log_references,
            p.limits.maximum_log_references,
        ),
        (
            r.observation.maximum_log_read_bytes,
            p.limits.maximum_log_read_bytes,
        ),
        (
            r.observation.maximum_total_log_bytes,
            p.limits.maximum_total_log_bytes,
        ),
    ] {
        require(observed > 0 && observed <= maximum)?;
    }
    require(r.observation.maximum_log_read_bytes <= r.observation.maximum_total_log_bytes)?;
    let g = &r.graph;
    require(
        !g.nodes.is_empty()
            && g.nodes.len() <= MAX_CHECKS
            && g.edges.len() <= MAX_CHECKS
            && g.nodes.len() as u64 <= p.limits.maximum_nodes
            && g.edges.len() as u64 <= p.limits.maximum_edges,
    )?;
    let ids: Vec<_> = g.nodes.iter().map(|node| &node.id).collect();
    let edges: Vec<_> = g
        .edges
        .iter()
        .map(|edge| (&edge.source, &edge.target))
        .collect();
    require(ordered(&ids) && ordered(&edges))?;
    for node in &g.nodes {
        require(
            label(&node.id)
                && p.node_kinds.contains(&node.kind)
                && p.effect_classes.contains(&node.effect_class)
                && p.adapter_kinds.contains(&node.adapter_kind)
                && digest(&node.operation_identity),
        )?;
    }
    for edge in &g.edges {
        require(
            edge.source != edge.target
                && ids.contains(&&edge.source)
                && ids.contains(&&edge.target),
        )?;
    }
    // Bounded elimination detects cycles without recursion or source effects.
    let mut remaining: BTreeSet<_> = ids.into_iter().collect();
    while !remaining.is_empty() {
        let ready = remaining
            .iter()
            .find(|id| {
                !g.edges
                    .iter()
                    .any(|edge| &edge.target == **id && remaining.contains(&edge.source))
            })
            .copied();
        let id = ready.ok_or(ProducerIssue::Observation)?;
        remaining.remove(id);
    }
    require(g.identity == identity::graph(g) && r.identity == identity::request(r))
}
fn require(valid: bool) -> Result<(), ProducerIssue> {
    if valid {
        Ok(())
    } else {
        Err(ProducerIssue::Observation)
    }
}
fn ordered<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn terminal(state: &str) -> bool {
    matches!(state, "completed" | "failed" | "cancelled")
}
fn digest(value: &str) -> bool {
    value.strip_prefix("b3:").is_some_and(|hex| {
        hex.len() == DIGEST_DIGITS
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
fn optional_digest(value: &Option<String>) -> bool {
    value.as_deref().is_none_or(digest)
}
fn label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= LABEL_BYTES
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._:-".contains(&b))
        && value
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}
fn source_value(value: &str) -> bool {
    !value.is_empty() && value.len() <= SOURCE_BYTES && !value.bytes().any(|b| b.is_ascii_control())
}
