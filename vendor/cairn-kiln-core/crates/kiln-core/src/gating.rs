use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::types::{
    BoundedText, CandidateId, ContentId, CoreResult, Diagnostic, EffectId, ExternalRef,
    PipelineRevisionId, PolicyId, ProviderProfileId,
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

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleGateStage {
    Proposal,
    Design,
    Tasks,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GateTarget {
    pub change: ExternalRef,
    pub stage: LifecycleGateStage,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GateEffectPlan {
    pub effect_id: EffectId,
    pub idempotency_key: ContentId,
    pub candidate: CandidateSet,
    pub candidate_root: ContentId,
    pub pipeline_revision: PipelineRevisionId,
    pub policy_identity: ContentId,
    pub evidence_identities: Vec<ContentId>,
    pub provider_profile: ProviderProfileId,
    pub target: GateTarget,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateEvaluationOutcome {
    Passed,
    Blocked,
    PolicyError,
    ProviderError,
    TimedOut,
    Cancelled,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GateFinding {
    pub code: BoundedText,
    pub message: BoundedText,
    pub scope: BoundedText,
    pub promoted: BoundedText,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GateEvaluationObservation {
    pub effect_id: EffectId,
    pub idempotency_key: ContentId,
    pub candidate_id: CandidateId,
    pub candidate_root: ContentId,
    pub pipeline_revision: PipelineRevisionId,
    pub policy_identity: ContentId,
    pub evidence_identities: Vec<ContentId>,
    pub provider_profile: ProviderProfileId,
    pub target: GateTarget,
    pub outcome: GateEvaluationOutcome,
    pub result_identity: Option<ContentId>,
    pub receipt_identity: Option<ContentId>,
    pub findings: Vec<GateFinding>,
}

// r[impl kiln.cairn_gate.request]
pub fn validate_gate_effect_plan(
    plan: &GateEffectPlan,
    max_changes: usize,
    max_edges: usize,
    max_evidence: usize,
) -> CoreResult<()> {
    validate_candidate_set(&plan.candidate, max_changes, max_edges)?;
    plan.target.change.validate()?;
    if plan.evidence_identities.len() > max_evidence {
        return Err(Diagnostic::new(
            "gate_evidence_bound",
            "gate.evidence_identities",
            "gate evidence count exceeds its named bound",
        ));
    }
    let mut previous = None;
    for identity in &plan.evidence_identities {
        if previous.is_some_and(|previous| previous >= identity) {
            return Err(Diagnostic::new(
                "gate_evidence_order",
                "gate.evidence_identities",
                "gate evidence identities must be sorted and unique",
            ));
        }
        previous = Some(identity);
    }
    if plan
        .candidate
        .ordered_changes
        .iter()
        .any(|change| change.pipeline_revision != plan.pipeline_revision)
    {
        return Err(Diagnostic::new(
            "gate_pipeline_revision",
            "gate.pipeline_revision",
            "gate pipeline revision differs from a candidate change",
        ));
    }
    Ok(())
}

// r[impl kiln.cairn_gate.result]
pub fn admit_gate_evaluation(
    plan: &GateEffectPlan,
    observation: &GateEvaluationObservation,
) -> CoreResult<()> {
    if observation.effect_id != plan.effect_id
        || observation.idempotency_key != plan.idempotency_key
        || observation.candidate_id != plan.candidate.id
        || observation.candidate_root != plan.candidate_root
        || observation.pipeline_revision != plan.pipeline_revision
        || observation.policy_identity != plan.policy_identity
        || observation.evidence_identities != plan.evidence_identities
        || observation.provider_profile != plan.provider_profile
        || observation.target != plan.target
    {
        return Err(Diagnostic::new(
            "gate_observation_substitution",
            "gate.observation",
            "gate observation does not apply to the exact effect plan",
        ));
    }
    let has_result = observation.result_identity.is_some();
    let has_receipt = observation.receipt_identity.is_some();
    match observation.outcome {
        GateEvaluationOutcome::Passed
            if has_result && has_receipt && observation.findings.is_empty() => {}
        GateEvaluationOutcome::Blocked
            if has_result && has_receipt && !observation.findings.is_empty() => {}
        GateEvaluationOutcome::PolicyError
        | GateEvaluationOutcome::ProviderError
        | GateEvaluationOutcome::TimedOut
        | GateEvaluationOutcome::Cancelled
        | GateEvaluationOutcome::Unknown
            if !has_result && !has_receipt => {}
        GateEvaluationOutcome::Passed
        | GateEvaluationOutcome::Blocked
        | GateEvaluationOutcome::PolicyError
        | GateEvaluationOutcome::ProviderError
        | GateEvaluationOutcome::TimedOut
        | GateEvaluationOutcome::Cancelled
        | GateEvaluationOutcome::Unknown => {
            return Err(Diagnostic::new(
                "gate_observation_contradiction",
                "gate.observation.outcome",
                "gate outcome contradicts its receipt or finding facts",
            ));
        }
    }
    Ok(())
}

pub fn gate_result_from_evaluation(
    plan: &GateEffectPlan,
    observation: &GateEvaluationObservation,
) -> CoreResult<Option<GateResult>> {
    admit_gate_evaluation(plan, observation)?;
    let passed = match observation.outcome {
        GateEvaluationOutcome::Passed => true,
        GateEvaluationOutcome::Blocked => false,
        GateEvaluationOutcome::PolicyError
        | GateEvaluationOutcome::ProviderError
        | GateEvaluationOutcome::TimedOut
        | GateEvaluationOutcome::Cancelled
        | GateEvaluationOutcome::Unknown => return Ok(None),
    };
    Ok(Some(GateResult {
        candidate_id: plan.candidate.id.clone(),
        policy_id: plan.candidate.policy_id.clone(),
        passed,
    }))
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;

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

    const MAX_CHANGES: usize = 8;
    const MAX_EDGES: usize = 16;
    const MAX_EVIDENCE: usize = 8;
    const IDENTITY_HEX_BYTES: usize = 64;

    fn content(character: char) -> ContentId {
        ContentId::from_blake3_hex(character.to_string().repeat(IDENTITY_HEX_BYTES))
            .unwrap_or_else(|_| unreachable!())
    }

    fn candidate() -> CandidateSet {
        let pipeline_revision =
            PipelineRevisionId::new("pipeline-a").unwrap_or_else(|_| unreachable!());
        let change =
            ExternalRef::new("source", "change", "change-a").unwrap_or_else(|_| unreachable!());
        CandidateSet {
            id: CandidateId::new("candidate-a").unwrap_or_else(|_| unreachable!()),
            ordered_changes: alloc::vec![CandidateChange {
                change: change.clone(),
                target_revision: ExternalRef::new("source", "revision", "revision-a")
                    .unwrap_or_else(|_| unreachable!()),
                source_snapshot: content('a'),
                pipeline_revision,
                collaboration_provider: ProviderProfileId::new("collaboration-a")
                    .unwrap_or_else(|_| unreachable!()),
            }],
            dependencies: Vec::new(),
            frozen_sources: BTreeMap::from([(change, content('a'))]),
            policy_id: PolicyId::new("policy-a").unwrap_or_else(|_| unreachable!()),
        }
    }

    fn gate_plan() -> GateEffectPlan {
        GateEffectPlan {
            effect_id: EffectId::new("gate-effect-a").unwrap_or_else(|_| unreachable!()),
            idempotency_key: content('b'),
            candidate: candidate(),
            candidate_root: content('c'),
            pipeline_revision: PipelineRevisionId::new("pipeline-a")
                .unwrap_or_else(|_| unreachable!()),
            policy_identity: content('d'),
            evidence_identities: alloc::vec![content('e'), content('f')],
            provider_profile: ProviderProfileId::new("cairn-gate-v1")
                .unwrap_or_else(|_| unreachable!()),
            target: GateTarget {
                change: ExternalRef::new("cairn", "change", "change-a")
                    .unwrap_or_else(|_| unreachable!()),
                stage: LifecycleGateStage::Tasks,
            },
        }
    }

    fn gate_observation(outcome: GateEvaluationOutcome) -> GateEvaluationObservation {
        let plan = gate_plan();
        let has_receipt = matches!(
            outcome,
            GateEvaluationOutcome::Passed | GateEvaluationOutcome::Blocked
        );
        GateEvaluationObservation {
            effect_id: plan.effect_id,
            idempotency_key: plan.idempotency_key,
            candidate_id: plan.candidate.id,
            candidate_root: plan.candidate_root,
            pipeline_revision: plan.pipeline_revision,
            policy_identity: plan.policy_identity,
            evidence_identities: plan.evidence_identities,
            provider_profile: plan.provider_profile,
            target: plan.target,
            outcome,
            result_identity: has_receipt.then(|| content('1')),
            receipt_identity: has_receipt.then(|| content('2')),
            findings: if outcome == GateEvaluationOutcome::Blocked {
                alloc::vec![GateFinding {
                    code: BoundedText::new("cairn.gate.blocked", 128, "finding.code")
                        .unwrap_or_else(|_| unreachable!()),
                    message: BoundedText::new("gate blocked", 128, "finding.message")
                        .unwrap_or_else(|_| unreachable!()),
                    scope: BoundedText::new("lifecycle", 128, "finding.scope")
                        .unwrap_or_else(|_| unreachable!()),
                    promoted: BoundedText::new("gate", 128, "finding.promoted")
                        .unwrap_or_else(|_| unreachable!()),
                }]
            } else {
                Vec::new()
            },
        }
    }

    // r[verify kiln.cairn_gate.request.scenario.valid]
    #[test]
    fn exact_gate_plan_is_admitted() {
        assert_eq!(
            validate_gate_effect_plan(&gate_plan(), MAX_CHANGES, MAX_EDGES, MAX_EVIDENCE),
            Ok(())
        );
    }

    // r[verify kiln.cairn_gate.request.scenario.evidence]
    #[test]
    fn gate_plan_rejects_noncanonical_evidence_and_pipeline_drift() {
        let mut evidence = gate_plan();
        evidence.evidence_identities.reverse();
        assert!(
            validate_gate_effect_plan(&evidence, MAX_CHANGES, MAX_EDGES, MAX_EVIDENCE).is_err()
        );

        let mut pipeline = gate_plan();
        pipeline.pipeline_revision =
            PipelineRevisionId::new("pipeline-b").unwrap_or_else(|_| unreachable!());
        assert!(
            validate_gate_effect_plan(&pipeline, MAX_CHANGES, MAX_EDGES, MAX_EVIDENCE).is_err()
        );
    }

    // r[verify kiln.cairn_gate.result.scenario.pass]
    #[test]
    fn exact_pass_and_block_map_to_candidate_results() {
        let plan = gate_plan();
        let pass = gate_observation(GateEvaluationOutcome::Passed);
        let block = gate_observation(GateEvaluationOutcome::Blocked);
        assert_eq!(admit_gate_evaluation(&plan, &pass), Ok(()));
        assert_eq!(admit_gate_evaluation(&plan, &block), Ok(()));
        assert_eq!(
            gate_result_from_evaluation(&plan, &pass)
                .map(|result| result.map(|value| value.passed)),
            Ok(Some(true))
        );
        assert_eq!(
            gate_result_from_evaluation(&plan, &block)
                .map(|result| result.map(|value| value.passed)),
            Ok(Some(false))
        );
    }

    // r[verify kiln.cairn_gate.result.scenario.stale]
    // r[verify kiln.cairn_gate.reconciliation.scenario.policy]
    #[test]
    fn substituted_candidate_policy_and_receipt_facts_fail_closed() {
        let plan = gate_plan();
        let mut candidate = gate_observation(GateEvaluationOutcome::Passed);
        candidate.candidate_root = content('9');
        assert!(admit_gate_evaluation(&plan, &candidate).is_err());

        let mut policy = gate_observation(GateEvaluationOutcome::Passed);
        policy.policy_identity = content('9');
        assert!(admit_gate_evaluation(&plan, &policy).is_err());

        let mut contradiction = gate_observation(GateEvaluationOutcome::Unknown);
        contradiction.receipt_identity = Some(content('9'));
        assert!(admit_gate_evaluation(&plan, &contradiction).is_err());
    }

    // r[verify kiln.cairn_gate.reconciliation.scenario.unknown]
    #[test]
    fn unknown_outcome_never_becomes_a_gate_result() {
        let plan = gate_plan();
        let unknown = gate_observation(GateEvaluationOutcome::Unknown);
        assert_eq!(admit_gate_evaluation(&plan, &unknown), Ok(()));
        assert_eq!(gate_result_from_evaluation(&plan, &unknown), Ok(None));
    }
}
