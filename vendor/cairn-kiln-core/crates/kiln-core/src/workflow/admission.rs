use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use crate::domain::Outcome;
use crate::types::{CoreResult, Diagnostic};

use super::{
    WorkflowEffectPlan, WorkflowLogReference, WorkflowNodeState, WorkflowObservation, WorkflowState,
};

// r[impl kiln.lattice.request]
pub fn validate_workflow_plan(plan: &WorkflowEffectPlan) -> CoreResult<()> {
    if plan.nodes.is_empty() {
        return Err(error(
            "workflow_graph_empty",
            "workflow.nodes",
            "workflow graph has no nodes",
        ));
    }
    if plan.bounds.max_nodes == 0
        || plan.bounds.max_edges == 0
        || plan.bounds.max_log_references == 0
        || plan.bounds.max_log_read_bytes == 0
        || plan.bounds.max_total_log_bytes == 0
        || plan.bounds.max_log_read_bytes > plan.bounds.max_total_log_bytes
    {
        return Err(error(
            "workflow_bounds",
            "workflow.bounds",
            "workflow bounds must be positive and internally consistent",
        ));
    }
    if plan.nodes.len() > plan.bounds.max_nodes || plan.edges.len() > plan.bounds.max_edges {
        return Err(error(
            "workflow_graph_bound",
            "workflow.graph",
            "workflow graph exceeds its named bounds",
        ));
    }
    if let Some(source) = &plan.source {
        source.repository.validate()?;
        source.revision.validate()?;
        if source.repository.provider_namespace.as_str() != source.provider.as_str()
            || source.revision.provider_namespace.as_str() != source.provider.as_str()
            || source.repository.native_kind.as_str() != "repository"
            || source.revision.native_kind.as_str() != "revision"
        {
            return Err(error(
                "workflow_source_context",
                "workflow.source",
                "workflow source context has inconsistent provider or native kinds",
            ));
        }
    }
    if !ordered_by(&plan.nodes, |node| &node.id) || !ordered(&plan.edges) {
        return Err(error(
            "workflow_graph_order",
            "workflow.graph",
            "workflow graph values are not canonically ordered",
        ));
    }
    let mut node_ids = BTreeSet::new();
    for node in &plan.nodes {
        if !node_ids.insert(node.id.clone()) {
            return Err(error(
                "workflow_node_duplicate",
                "workflow.nodes",
                "workflow node identity is duplicated",
            ));
        }
    }
    let mut edges = BTreeSet::new();
    for edge in &plan.edges {
        if !edges.insert(edge.clone()) {
            return Err(error(
                "workflow_edge_duplicate",
                "workflow.edges",
                "workflow edge is duplicated",
            ));
        }
        if edge.source == edge.target
            || !node_ids.contains(&edge.source)
            || !node_ids.contains(&edge.target)
        {
            return Err(error(
                "workflow_edge_reference",
                "workflow.edges",
                "workflow edge references an invalid node",
            ));
        }
    }
    if has_cycle(plan, &node_ids) {
        return Err(error(
            "workflow_graph_cycle",
            "workflow.edges",
            "workflow graph contains a cycle",
        ));
    }
    Ok(())
}

// r[impl kiln.lattice.observation] r[impl kiln.lattice.reconciliation]
pub fn admit_workflow_observation(
    plan: &WorkflowEffectPlan,
    observation: &WorkflowObservation,
) -> CoreResult<Outcome> {
    validate_workflow_plan(plan)?;
    if observation.effect_id != plan.effect_id
        || observation.attempt_id != plan.attempt_id
        || observation.candidate_id != plan.candidate_id
        || observation.pipeline_revision_id != plan.pipeline_revision_id
        || observation.workflow_policy_id != plan.workflow_policy_id
        || observation.provider_profile != plan.provider_profile
        || observation.idempotency_key != plan.idempotency_key
    {
        return Err(error(
            "workflow_observation_linkage",
            "workflow.observation",
            "workflow observation does not match the active Kiln plan",
        ));
    }
    observation.workflow.validate()?;
    observation.execution.validate()?;
    validate_node_observations(plan, observation)?;
    validate_log_observations(plan, &observation.logs)?;
    let expected_outcome = classify_state(observation)?;
    if observation.outcome != expected_outcome {
        return Err(error(
            "workflow_false_outcome",
            "workflow.outcome",
            "workflow observation reports an inconsistent Kiln outcome",
        ));
    }
    Ok(expected_outcome)
}

fn validate_node_observations(
    plan: &WorkflowEffectPlan,
    observation: &WorkflowObservation,
) -> CoreResult<()> {
    if observation.nodes.is_empty() && !observation.state.is_terminal() {
        return Ok(());
    }
    if observation.nodes.len() != plan.nodes.len() {
        return Err(error(
            "workflow_node_observation_count",
            "workflow.nodes",
            "workflow observation does not cover the planned nodes",
        ));
    }
    for (planned, observed) in plan.nodes.iter().zip(&observation.nodes) {
        if planned.id != observed.node_id
            || observed.terminal != observed.state.is_terminal()
            || observed.successful != observed.state.is_successful()
        {
            return Err(error(
                "workflow_node_observation",
                "workflow.nodes",
                "workflow node observation is inconsistent",
            ));
        }
        observed.adapter.validate()?;
        let requires_receipt = matches!(
            observed.state,
            WorkflowNodeState::Completed | WorkflowNodeState::Failed | WorkflowNodeState::Cancelled
        );
        if requires_receipt != observed.receipt.is_some() {
            return Err(error(
                "workflow_node_receipt",
                "workflow.nodes.receipt",
                "workflow node receipt presence is inconsistent",
            ));
        }
        if let Some(receipt) = &observed.receipt {
            receipt.validate()?;
        }
    }
    Ok(())
}

fn validate_log_observations(
    plan: &WorkflowEffectPlan,
    logs: &[WorkflowLogReference],
) -> CoreResult<()> {
    if logs.len() > plan.bounds.max_log_references {
        return Err(error(
            "workflow_log_count",
            "workflow.logs",
            "workflow log reference count exceeds its named bound",
        ));
    }
    let mut total_bytes = 0usize;
    let mut previous = None;
    for log in logs {
        if !plan.nodes.iter().any(|node| node.id == log.node_id) {
            return Err(error(
                "workflow_log_node",
                "workflow.logs.node",
                "workflow log references an unknown node",
            ));
        }
        if let Some((node, stream, cursor)) = previous
            && (node, stream, cursor) >= (&log.node_id, log.stream, log.cursor)
        {
            return Err(error(
                "workflow_log_order",
                "workflow.logs",
                "workflow log references are not canonically ordered",
            ));
        }
        previous = Some((&log.node_id, log.stream, log.cursor));
        if log.bytes > plan.bounds.max_log_read_bytes {
            return Err(error(
                "workflow_log_bound",
                "workflow.logs.bytes",
                "workflow log reference exceeds the read bound",
            ));
        }
        total_bytes = total_bytes.checked_add(log.bytes).ok_or_else(|| {
            error(
                "workflow_log_overflow",
                "workflow.logs.bytes",
                "workflow log byte count overflowed",
            )
        })?;
        log.reference.validate()?;
    }
    if total_bytes > plan.bounds.max_total_log_bytes {
        return Err(error(
            "workflow_log_total",
            "workflow.logs.bytes",
            "workflow log references exceed the total byte bound",
        ));
    }
    Ok(())
}

fn classify_state(observation: &WorkflowObservation) -> CoreResult<Outcome> {
    if observation.terminal != observation.state.is_terminal() {
        return Err(error(
            "workflow_terminal_state",
            "workflow.terminal",
            "workflow terminal flag does not match provider state",
        ));
    }
    let expected = match observation.state {
        WorkflowState::Accepted
        | WorkflowState::Queued
        | WorkflowState::Running
        | WorkflowState::Paused => {
            if observation.successful || observation.receipt.is_some() {
                return Err(error(
                    "workflow_nonterminal_success",
                    "workflow.successful",
                    "nonterminal workflow cannot claim success or a terminal receipt",
                ));
            }
            Outcome::Pending
        }
        WorkflowState::Completed => {
            let complete = !observation.nodes.is_empty()
                && observation
                    .nodes
                    .iter()
                    .all(|node| node.terminal && node.successful)
                && observation.successful
                && observation.receipt.is_some();
            if !complete {
                return Err(error(
                    "workflow_completion",
                    "workflow.state",
                    "completed workflow lacks linked successful node results",
                ));
            }
            Outcome::Success
        }
        WorkflowState::Failed => {
            let failed = !observation.successful
                && observation.receipt.is_some()
                && observation
                    .nodes
                    .iter()
                    .any(|node| node.state == WorkflowNodeState::Failed);
            if !failed {
                return Err(error(
                    "workflow_failure",
                    "workflow.state",
                    "failed workflow lacks a linked failed node result",
                ));
            }
            Outcome::DomainFailure
        }
        WorkflowState::Cancelled => {
            let cancelled = !observation.successful
                && observation.receipt.is_some()
                && observation
                    .nodes
                    .iter()
                    .any(|node| node.state == WorkflowNodeState::Cancelled);
            if !cancelled {
                return Err(error(
                    "workflow_cancelled",
                    "workflow.state",
                    "cancelled workflow lacks a linked cancelled node result",
                ));
            }
            Outcome::Cancelled
        }
        WorkflowState::Unknown => {
            if observation.terminal || observation.successful || observation.receipt.is_some() {
                return Err(error(
                    "workflow_unknown_claim",
                    "workflow.state",
                    "unknown workflow outcome cannot claim terminal success",
                ));
            }
            Outcome::Unknown
        }
    };
    if let Some(receipt) = &observation.receipt {
        receipt.validate()?;
    }
    Ok(expected)
}

fn has_cycle(plan: &WorkflowEffectPlan, node_ids: &BTreeSet<crate::types::Alias>) -> bool {
    let mut indegrees = BTreeMap::new();
    let mut adjacency: BTreeMap<crate::types::Alias, Vec<crate::types::Alias>> = BTreeMap::new();
    for node_id in node_ids {
        indegrees.insert(node_id.clone(), 0usize);
        adjacency.insert(node_id.clone(), Vec::new());
    }
    for edge in &plan.edges {
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
    let mut visited = 0usize;
    while let Some(node_id) = ready.pop() {
        visited = visited.saturating_add(1);
        let Some(targets) = adjacency.get(&node_id) else {
            return true;
        };
        for target in targets {
            let Some(indegree) = indegrees.get_mut(target) else {
                return true;
            };
            let Some(next) = indegree.checked_sub(1) else {
                return true;
            };
            *indegree = next;
            if next == 0 {
                ready.push(target.clone());
            }
        }
    }
    visited != node_ids.len()
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

fn error(code: &str, field: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, field, message)
}
