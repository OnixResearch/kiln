// r[impl electrical_circuit_primitives.topology]
// r[impl electrical_circuit_primitives.topology.reference_node]
// r[impl electrical_circuit_primitives.topology.connection_arity]

use serde_json::{Map, Value};

use super::components::component_binding_issues;
use super::issues::{issue, CircuitPrimitiveIssue};

pub(super) fn topology_issues(object: &Map<String, Value>) -> Vec<CircuitPrimitiveIssue> {
    let mut issues = Vec::with_capacity(8);
    let node_ids = collect_ids(object.get("nodes"));
    let net_ids = collect_ids(object.get("nets"));

    let Some(reference_node) = object.get("reference_node").and_then(Value::as_str) else {
        issues.push(issue(
            "circuit_primitive.reference_node_missing",
            "reference_node must identify a node id or an explicit none value in future schemas",
        ));
        return issues;
    };
    if !node_ids.iter().any(|node| node == reference_node) {
        issues.push(issue(
            "circuit_primitive.reference_node_unknown",
            "reference_node must match a declared node id",
        ));
    }

    issues.extend(duplicate_id_issues("node", &node_ids));
    issues.extend(duplicate_id_issues("net", &net_ids));
    issues.extend(component_binding_issues(object.get("components"), &net_ids));
    issues
}

fn collect_ids(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.get("id").and_then(Value::as_str))
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn duplicate_id_issues(kind: &str, ids: &[String]) -> Vec<CircuitPrimitiveIssue> {
    let mut issues = Vec::with_capacity(ids.len());
    for (index, id) in ids.iter().enumerate() {
        if ids.iter().skip(index + 1).any(|other| other == id) {
            issues.push(issue(
                "circuit_primitive.duplicate_id",
                format!("duplicate {kind} id: {id}"),
            ));
        }
    }
    issues
}
