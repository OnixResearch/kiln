// Octet: graph allocation is bounded by the admitted profile before maps or adjacency lists are
// created.
#![allow(
    unknown_lints,
    non_trait_imports,
    assertion_density,
    compound_condition,
    unbounded_collection_growth,
    allocation_in_loop,
    reason = "profile node and edge limits bound each graph collection before deterministic DAG admission"
)]

use alloc::collections::BTreeMap;
use alloc::collections::BTreeSet;
use alloc::vec::Vec;

use crate::model::CapabilityProfile;
use crate::model::ContractError;
use crate::model::ErrorCode;
use crate::model::WorkflowGraph;

// r[impl lattice.workflow_interchange.graph]
pub(super) fn validate(profile: &CapabilityProfile, graph: &WorkflowGraph) -> Result<(), ContractError> {
    if graph.nodes.is_empty() {
        return Err(error(ErrorCode::GraphEmpty, "request.graph.nodes"));
    }
    if super::exceeds_u32(graph.nodes.len(), profile.limits.maximum_nodes)
        || super::exceeds_u32(graph.edges.len(), profile.limits.maximum_edges)
    {
        return Err(error(ErrorCode::GraphLimit, "request.graph"));
    }
    if !ordered_by(&graph.nodes, |node| &node.id) || !ordered(&graph.edges) {
        return Err(error(ErrorCode::GraphOrder, "request.graph"));
    }
    let mut node_ids = BTreeSet::new();
    for node in &graph.nodes {
        if !node_ids.insert(node.id.clone()) {
            return Err(error(ErrorCode::GraphDuplicate, "request.graph.nodes"));
        }
        if !profile.node_kinds.contains(&node.kind) {
            return Err(error(ErrorCode::UnsupportedNodeKind, "request.graph.nodes.kind"));
        }
        if !profile.effect_classes.contains(&node.effect_class) {
            return Err(error(ErrorCode::UnsupportedEffectClass, "request.graph.nodes.effect_class"));
        }
        if !profile.adapter_kinds.contains(&node.adapter_kind) {
            return Err(error(ErrorCode::UnsupportedAdapterKind, "request.graph.nodes.adapter_kind"));
        }
    }
    let mut edges = BTreeSet::new();
    for edge in &graph.edges {
        if !edges.insert(edge.clone()) {
            return Err(error(ErrorCode::GraphDuplicate, "request.graph.edges"));
        }
        if edge.source == edge.target || !node_ids.contains(&edge.source) || !node_ids.contains(&edge.target) {
            return Err(error(ErrorCode::GraphReference, "request.graph.edges"));
        }
    }
    if has_cycle(graph, &node_ids) {
        return Err(error(ErrorCode::GraphCycle, "request.graph.edges"));
    }
    Ok(())
}

fn has_cycle(graph: &WorkflowGraph, node_ids: &BTreeSet<crate::model::Label>) -> bool {
    let mut indegrees = BTreeMap::new();
    let mut adjacency: BTreeMap<crate::model::Label, Vec<crate::model::Label>> = BTreeMap::new();
    for node_id in node_ids {
        indegrees.insert(node_id.clone(), 0_u32);
        adjacency.insert(node_id.clone(), Vec::new());
    }
    for edge in &graph.edges {
        let Some(indegree) = indegrees.get_mut(&edge.target) else {
            return true;
        };
        let Some(next) = indegree.checked_add(1) else {
            return true;
        };
        *indegree = next;
        let Some(targets) = adjacency.get_mut(&edge.source) else {
            return true;
        };
        targets.push(edge.target.clone());
    }
    let mut ready: Vec<_> = indegrees
        .iter()
        .filter_map(|(node_id, indegree)| (*indegree == 0).then_some(node_id.clone()))
        .collect();
    let mut visited = 0_u32;
    while let Some(node_id) = ready.pop() {
        let Some(next_visited) = visited.checked_add(1) else {
            return true;
        };
        visited = next_visited;
        let Some(targets) = adjacency.get(&node_id) else {
            return true;
        };
        for target in targets {
            let Some(indegree) = indegrees.get_mut(target) else {
                return true;
            };
            let Some(next_indegree) = indegree.checked_sub(1) else {
                return true;
            };
            *indegree = next_indegree;
            if next_indegree == 0 {
                ready.push(target.clone());
            }
        }
    }
    match u32::try_from(node_ids.len()) {
        Ok(node_count) => visited != node_count,
        Err(_) => true,
    }
}

fn ordered<T: Ord>(values: &[T]) -> bool {
    ordered_with(values, |previous, current| previous <= current)
}

fn ordered_by<T, K: Ord>(values: &[T], key: impl Fn(&T) -> &K) -> bool {
    ordered_with(values, |previous, current| key(previous) <= key(current))
}

fn ordered_with<T>(values: &[T], compare: impl Fn(&T, &T) -> bool) -> bool {
    let mut values = values.iter();
    let Some(mut previous) = values.next() else {
        return true;
    };
    for current in values {
        if !compare(previous, current) {
            return false;
        }
        previous = current;
    }
    true
}

const fn error(code: ErrorCode, path: &'static str) -> ContractError {
    ContractError { code, path }
}
