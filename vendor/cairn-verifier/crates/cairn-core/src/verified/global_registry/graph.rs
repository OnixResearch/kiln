use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde_json::json;

use crate::verified::evidence::canonical_json_hash;

use super::{
    issue, issue_order, registry_non_claims, report_hash, GlobalRegistrySnapshot, RegistryEdge,
    RegistryGraphDirection, RegistryGraphProfile, RegistryGraphReport,
};

const MAX_CYCLE_WITNESS_NODES: usize = 64;

// r[impl cairn.global_registry.graph]
// r[impl cairn.global_registry.graph.unresolved]
// r[impl cairn.global_registry.dag_profiles]
// r[impl cairn.global_registry.dag_profiles.acyclic]
// r[impl cairn.global_registry.dag_profiles.cycle]
// r[impl cairn.global_registry.dag_profiles.non_dag]
pub fn evaluate_registry_graph(
    snapshot: &GlobalRegistrySnapshot,
    profile: &RegistryGraphProfile,
    start_id: Option<&str>,
    direction: RegistryGraphDirection,
) -> RegistryGraphReport {
    let mut issues = Vec::new();
    let allowed_nodes = snapshot
        .nodes
        .iter()
        .filter(|node| profile.allowed_node_kinds.contains(&node.kind))
        .map(|node| node.stable_id.clone())
        .collect::<BTreeSet<_>>();
    if profile.id.is_empty() || profile.allowed_edge_kinds.is_empty() {
        issues.push(issue(
            "registry.graph_profile_invalid",
            "graph_profile",
            &profile.id,
            "allowed_edge_kinds",
            Some("non-empty profile ID and edge-kind selection"),
            None,
            "graph profile is incomplete",
        ));
    }
    if let Some(start) = start_id {
        if !allowed_nodes.contains(start) {
            issues.push(issue(
                "registry.graph_start_unknown",
                "graph_query",
                start,
                "start_id",
                Some("node selected by graph profile"),
                Some(start),
                "graph traversal start node is absent from the selected projection",
            ));
        }
    }

    let selected_edges = snapshot
        .edges
        .iter()
        .filter(|edge| profile.allowed_edge_kinds.contains(&edge.kind))
        .cloned()
        .collect::<Vec<_>>();
    let mut unresolved = BTreeSet::new();
    let mut edge_keys = BTreeSet::new();
    for edge in &selected_edges {
        for endpoint in [&edge.from_id, &edge.to_id] {
            if !allowed_nodes.contains(endpoint) {
                unresolved.insert(endpoint.clone());
                issues.push(issue(
                    "registry.graph_unresolved_endpoint",
                    "edge",
                    &edge.stable_id,
                    "endpoint",
                    Some("node selected by profile"),
                    Some(endpoint),
                    "selected graph edge has an unresolved or profile-excluded endpoint",
                ));
            }
        }
        if edge.from_id == edge.to_id {
            issues.push(issue(
                "registry.graph_self_edge",
                "edge",
                &edge.stable_id,
                "to_id",
                Some("different node"),
                Some(&edge.to_id),
                "DAG projection contains a self-edge",
            ));
        }
        let key = (&edge.kind, &edge.from_id, &edge.to_id);
        if !edge_keys.insert(key) {
            issues.push(issue(
                "registry.graph_duplicate_edge",
                "edge",
                &edge.stable_id,
                "endpoints",
                Some("unique typed endpoint pair"),
                None,
                "graph projection contains duplicate typed edges",
            ));
        }
    }

    let traversed = traversal_set(&allowed_nodes, &selected_edges, start_id, direction);
    let traversed_edges = selected_edges
        .iter()
        .filter(|edge| traversed.contains(&edge.from_id) && traversed.contains(&edge.to_id))
        .cloned()
        .collect::<Vec<_>>();
    let cycle_witness = canonical_cycle_witness(&traversed, &traversed_edges);
    if profile.require_dag && !cycle_witness.is_empty() {
        issues.push(issue(
            "registry.graph_cycle",
            "graph_profile",
            &profile.id,
            "cycle_witness",
            Some("acyclic selected projection"),
            Some(&cycle_witness.join(" -> ")),
            "DAG-constrained graph projection contains a cycle",
        ));
    }
    let topological_order = if profile.require_dag && cycle_witness.is_empty() && issues.is_empty()
    {
        stable_topological_order(&traversed, &traversed_edges)
    } else if profile.require_dag {
        Vec::new()
    } else {
        traversed.iter().cloned().collect()
    };
    let mut selected_nodes = traversed.into_iter().collect::<Vec<_>>();
    selected_nodes.sort();
    let mut selected_edge_ids = traversed_edges
        .iter()
        .map(|edge| edge.stable_id.clone())
        .collect::<Vec<_>>();
    selected_edge_ids.sort();
    selected_edge_ids.dedup();
    issues.sort_by(issue_order);
    let result_hash = canonical_json_hash(&json!({
        "profile_id": profile.id,
        "snapshot_hash": snapshot.snapshot_hash,
        "selected_nodes": selected_nodes,
        "selected_edges": selected_edge_ids,
        "topological_order": topological_order,
        "cycle_witness": cycle_witness,
        "unresolved_nodes": unresolved,
    }));
    let valid = issues.is_empty();
    let mut report = RegistryGraphReport {
        valid,
        verdict: if valid { "PASS" } else { "FAIL" }.to_string(),
        profile_id: profile.id.clone(),
        snapshot_hash: snapshot.snapshot_hash.clone(),
        selected_nodes,
        selected_edges: selected_edge_ids,
        topological_order,
        cycle_witness,
        unresolved_nodes: unresolved.into_iter().collect(),
        issues,
        result_hash,
        receipt_hash: String::new(),
        non_claims: registry_non_claims(snapshot),
    };
    report.receipt_hash = report_hash(&report);
    report
}

fn traversal_set(
    nodes: &BTreeSet<String>,
    edges: &[RegistryEdge],
    start_id: Option<&str>,
    direction: RegistryGraphDirection,
) -> BTreeSet<String> {
    let Some(start) = start_id else {
        return nodes.clone();
    };
    if !nodes.contains(start) {
        return BTreeSet::new();
    }
    let mut forward: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut reverse: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for edge in edges {
        forward.entry(&edge.from_id).or_default().push(&edge.to_id);
        reverse.entry(&edge.to_id).or_default().push(&edge.from_id);
    }
    for values in forward.values_mut().chain(reverse.values_mut()) {
        values.sort();
        values.dedup();
    }
    let mut selected = BTreeSet::from([start.to_string()]);
    let mut queue = VecDeque::from([start]);
    while let Some(current) = queue.pop_front() {
        let neighbors = match direction {
            RegistryGraphDirection::Descendants => {
                forward.get(current).cloned().unwrap_or_default()
            }
            RegistryGraphDirection::Ancestors => reverse.get(current).cloned().unwrap_or_default(),
            RegistryGraphDirection::Both => {
                let mut values = forward.get(current).cloned().unwrap_or_default();
                values.extend(reverse.get(current).cloned().unwrap_or_default());
                values.sort();
                values.dedup();
                values
            }
        };
        for neighbor in neighbors {
            if nodes.contains(neighbor) && selected.insert(neighbor.to_string()) {
                queue.push_back(neighbor);
            }
        }
    }
    selected
}

fn stable_topological_order(nodes: &BTreeSet<String>, edges: &[RegistryEdge]) -> Vec<String> {
    let mut indegree = nodes
        .iter()
        .map(|node| (node.as_str(), 0_usize))
        .collect::<BTreeMap<_, _>>();
    let mut adjacency: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for edge in edges {
        if let Some(value) = indegree.get_mut(edge.from_id.as_str()) {
            *value += 1;
        }
        adjacency
            .entry(&edge.to_id)
            .or_default()
            .push(&edge.from_id);
    }
    for values in adjacency.values_mut() {
        values.sort();
        values.dedup();
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(node, _)| *node)
        .collect::<BTreeSet<_>>();
    let mut order = Vec::new();
    while let Some(next) = ready.pop_first() {
        order.push(next.to_string());
        for neighbor in adjacency.get(next).cloned().unwrap_or_default() {
            if let Some(value) = indegree.get_mut(neighbor) {
                *value -= 1;
                if *value == 0 {
                    ready.insert(neighbor);
                }
            }
        }
    }
    order
}

fn canonical_cycle_witness(nodes: &BTreeSet<String>, edges: &[RegistryEdge]) -> Vec<String> {
    let mut adjacency: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for edge in edges {
        adjacency
            .entry(&edge.from_id)
            .or_default()
            .push(&edge.to_id);
    }
    for values in adjacency.values_mut() {
        values.sort();
        values.dedup();
    }
    let mut state = BTreeMap::new();
    let mut stack = Vec::new();
    for node in nodes {
        if state.get(node.as_str()).copied().unwrap_or(0) == 0 {
            if let Some(witness) = visit_cycle(node, &adjacency, &mut state, &mut stack) {
                return witness;
            }
        }
    }
    Vec::new()
}

fn visit_cycle<'a>(
    node: &'a str,
    adjacency: &BTreeMap<&'a str, Vec<&'a str>>,
    state: &mut BTreeMap<&'a str, u8>,
    stack: &mut Vec<&'a str>,
) -> Option<Vec<String>> {
    state.insert(node, 1);
    stack.push(node);
    for neighbor in adjacency.get(node).cloned().unwrap_or_default() {
        match state.get(neighbor).copied().unwrap_or(0) {
            0 => {
                if let Some(witness) = visit_cycle(neighbor, adjacency, state, stack) {
                    return Some(witness);
                }
            }
            1 => {
                let start = stack.iter().position(|entry| *entry == neighbor)?;
                let mut witness = stack[start..]
                    .iter()
                    .take(MAX_CYCLE_WITNESS_NODES)
                    .map(|entry| (*entry).to_string())
                    .collect::<Vec<_>>();
                witness.push(neighbor.to_string());
                return Some(witness);
            }
            _ => {}
        }
    }
    stack.pop();
    state.insert(node, 2);
    None
}
