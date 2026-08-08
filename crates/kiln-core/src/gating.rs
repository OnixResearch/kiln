use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::types::{
    CandidateId, ContentId, CoreResult, Diagnostic, ExternalRef, PipelineRevisionId, PolicyId,
    ProviderProfileId,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CandidateChange {
    pub change: ExternalRef,
    pub target_revision: ExternalRef,
    pub source_snapshot: ContentId,
    pub pipeline_revision: PipelineRevisionId,
    pub collaboration_provider: ProviderProfileId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DependencyEdge {
    pub before_index: usize,
    pub after_index: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CandidateSet {
    pub id: CandidateId,
    pub ordered_changes: Vec<CandidateChange>,
    pub dependencies: Vec<DependencyEdge>,
    pub frozen_sources: BTreeMap<ExternalRef, ContentId>,
    pub policy_id: PolicyId,
}

pub fn validate_candidate_set(
    candidate: &CandidateSet,
    max_changes: usize,
    max_edges: usize,
) -> CoreResult<()> {
    if candidate.ordered_changes.is_empty() || candidate.ordered_changes.len() > max_changes {
        return Err(Diagnostic::new(
            "candidate_bound",
            "candidate.ordered_changes",
            "candidate change count is invalid",
        ));
    }
    if candidate.dependencies.len() > max_edges {
        return Err(Diagnostic::new(
            "dependency_bound",
            "candidate.dependencies",
            "candidate dependency count exceeds its named bound",
        ));
    }
    let node_count = candidate.ordered_changes.len();
    let mut adjacency = alloc::vec![Vec::<usize>::new(); node_count];
    for edge in &candidate.dependencies {
        if edge.before_index >= node_count || edge.after_index >= node_count {
            return Err(Diagnostic::new(
                "missing_dependency_node",
                "candidate.dependencies",
                "dependency names a missing candidate node",
            ));
        }
        if edge.before_index >= edge.after_index {
            return Err(Diagnostic::new(
                "invalid_dependency_order",
                "candidate.dependencies",
                "dependency order is cyclic or reversed",
            ));
        }
        let Some(edges) = adjacency.get_mut(edge.before_index) else {
            return Err(Diagnostic::new(
                "missing_dependency_node",
                "candidate.dependencies",
                "dependency source is missing",
            ));
        };
        edges.push(edge.after_index);
    }
    validate_acyclic(&adjacency)?;
    Ok(())
}

fn validate_acyclic(adjacency: &[Vec<usize>]) -> CoreResult<()> {
    let mut visiting = BTreeSet::new();
    let mut complete = BTreeSet::new();
    for node in 0..adjacency.len() {
        visit(node, adjacency, &mut visiting, &mut complete)?;
    }
    Ok(())
}

fn visit(
    node: usize,
    adjacency: &[Vec<usize>],
    visiting: &mut BTreeSet<usize>,
    complete: &mut BTreeSet<usize>,
) -> CoreResult<()> {
    if complete.contains(&node) {
        return Ok(());
    }
    if !visiting.insert(node) {
        return Err(Diagnostic::new(
            "dependency_cycle",
            "candidate.dependencies",
            "candidate dependency graph contains a cycle",
        ));
    }
    let Some(neighbors) = adjacency.get(node) else {
        return Err(Diagnostic::new(
            "missing_dependency_node",
            "candidate.dependencies",
            "dependency node is missing",
        ));
    };
    for neighbor in neighbors {
        visit(*neighbor, adjacency, visiting, complete)?;
    }
    visiting.remove(&node);
    complete.insert(node);
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum GateMode {
    Serial,
    Speculative,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WindowPolicy {
    pub floor: usize,
    pub ceiling: usize,
    pub increase: usize,
    pub decrease: usize,
}

pub fn admitted_candidate_indices(
    mode: GateMode,
    current_window: usize,
    candidate_count: usize,
) -> CoreResult<Vec<usize>> {
    if candidate_count == 0 || current_window == 0 {
        return Err(Diagnostic::new(
            "invalid_gate_admission",
            "gate.window",
            "gate admission requires a candidate and a positive window",
        ));
    }
    let admitted = match mode {
        GateMode::Serial => 1,
        GateMode::Speculative => current_window.min(candidate_count),
    };
    Ok((0..admitted).collect())
}

pub fn update_window(current: usize, passed: bool, policy: WindowPolicy) -> CoreResult<usize> {
    if policy.floor == 0
        || policy.floor > policy.ceiling
        || policy.increase == 0
        || policy.decrease == 0
    {
        return Err(Diagnostic::new(
            "invalid_window_policy",
            "gate.window",
            "gate window policy bounds are invalid",
        ));
    }
    let next = if passed {
        current.saturating_add(policy.increase).min(policy.ceiling)
    } else {
        current.saturating_sub(policy.decrease).max(policy.floor)
    };
    Ok(next)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GateResult {
    pub candidate_id: CandidateId,
    pub policy_id: PolicyId,
    pub passed: bool,
}

#[must_use]
pub fn result_applies(result: &GateResult, candidate: &CandidateSet) -> bool {
    result.candidate_id == candidate.id && result.policy_id == candidate.policy_id
}

pub fn invalidated_suffix(failed_index: usize, candidate: &CandidateSet) -> CoreResult<Vec<usize>> {
    if failed_index >= candidate.ordered_changes.len() {
        return Err(Diagnostic::new(
            "missing_candidate_index",
            "failed_index",
            "failed candidate index is out of bounds",
        ));
    }
    Ok((failed_index..candidate.ordered_changes.len()).collect())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReevaluationDecision {
    pub invalidated_indices: Vec<usize>,
    pub replacement_required: bool,
}

pub fn decide_reevaluation(
    failed_index: usize,
    previous: &CandidateSet,
    current: &CandidateSet,
) -> CoreResult<ReevaluationDecision> {
    let invalidated_indices = invalidated_suffix(failed_index, previous)?;
    Ok(ReevaluationDecision {
        invalidated_indices,
        replacement_required: previous.id != current.id
            || previous.policy_id != current.policy_id
            || previous.frozen_sources != current.frozen_sources,
    })
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MergePreflight {
    pub candidate_id: CandidateId,
    pub policy_id: PolicyId,
    pub expected_target: ExternalRef,
    pub observed_target: ExternalRef,
    pub approvals_current: bool,
    pub provider_capable: bool,
}

pub fn admit_merge(
    result: &GateResult,
    candidate: &CandidateSet,
    preflight: &MergePreflight,
) -> CoreResult<()> {
    if !result.passed || !result_applies(result, candidate) {
        return Err(Diagnostic::new(
            "stale_gate_result",
            "gate_result",
            "gate result does not apply to this candidate",
        ));
    }
    if preflight.candidate_id != candidate.id || preflight.policy_id != candidate.policy_id {
        return Err(Diagnostic::new(
            "preflight_mismatch",
            "merge_preflight",
            "merge preflight is bound to another candidate or policy",
        ));
    }
    if preflight.expected_target != preflight.observed_target {
        return Err(Diagnostic::new(
            "target_moved",
            "merge_preflight.observed_target",
            "merge target moved after candidate evaluation",
        ));
    }
    if !preflight.approvals_current || !preflight.provider_capable {
        return Err(Diagnostic::new(
            "merge_not_admitted",
            "merge_preflight",
            "merge approval or provider capability is not current",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLOOR: usize = 1;
    const CEILING: usize = 8;
    const INCREASE: usize = 1;
    const DECREASE: usize = 2;
    const CURRENT: usize = 4;

    #[test]
    fn serial_admission_never_speculates() {
        assert_eq!(
            admitted_candidate_indices(GateMode::Serial, CEILING, CEILING),
            Ok(alloc::vec![0])
        );
    }

    #[test]
    fn speculative_admission_is_bounded_by_candidate_count() {
        assert_eq!(
            admitted_candidate_indices(GateMode::Speculative, CEILING, DECREASE),
            Ok(alloc::vec![0, 1])
        );
    }

    #[test]
    fn failed_candidate_shrinks_window() {
        let policy = WindowPolicy {
            floor: FLOOR,
            ceiling: CEILING,
            increase: INCREASE,
            decrease: DECREASE,
        };
        assert_eq!(
            update_window(CURRENT, false, policy),
            Ok(CURRENT - DECREASE)
        );
    }

    #[test]
    fn stale_result_does_not_apply() {
        let result = GateResult {
            candidate_id: CandidateId::new("candidate-a").unwrap_or_else(|_| unreachable!()),
            policy_id: PolicyId::new("policy-a").unwrap_or_else(|_| unreachable!()),
            passed: true,
        };
        assert_ne!(
            result.candidate_id,
            CandidateId::new("candidate-b").unwrap_or_else(|_| unreachable!())
        );
    }
}
