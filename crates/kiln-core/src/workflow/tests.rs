use alloc::string::ToString;
use alloc::vec;

use super::*;

const IDENTITY_HEX_BYTES: usize = 64;
const MAX_NODES: usize = 8;
const MAX_EDGES: usize = 16;
const MAX_LOG_REFERENCES: usize = 8;
const MAX_LOG_READ_BYTES: usize = 4096;
const MAX_TOTAL_LOG_BYTES: usize = 16384;
const TERMINAL_SEQUENCE: u64 = 4;

fn content(character: char) -> ContentId {
    ContentId::from_blake3_hex(character.to_string().repeat(IDENTITY_HEX_BYTES))
        .unwrap_or_else(|_| unreachable!())
}

fn alias(value: &str) -> Alias {
    Alias::new(value).unwrap_or_else(|_| unreachable!())
}

fn external(kind: &str, value: &str) -> ExternalRef {
    ExternalRef::new("lattice", kind, value).unwrap_or_else(|_| unreachable!())
}

fn plan() -> WorkflowEffectPlan {
    WorkflowEffectPlan {
        effect_id: EffectId::new("effect").unwrap_or_else(|_| unreachable!()),
        attempt_id: AttemptId::new("attempt").unwrap_or_else(|_| unreachable!()),
        candidate_id: CandidateId::new("candidate").unwrap_or_else(|_| unreachable!()),
        pipeline_revision_id: PipelineRevisionId::new("pipeline")
            .unwrap_or_else(|_| unreachable!()),
        workflow_policy_id: PolicyId::new("workflow-policy").unwrap_or_else(|_| unreachable!()),
        provider_profile: ProviderProfileId::new("lattice-v1").unwrap_or_else(|_| unreachable!()),
        idempotency_key: content('0'),
        platform: alias("linux-x86_64"),
        source: None,
        nodes: vec![
            WorkflowNode {
                id: alias("build"),
                kind: alias("build"),
                effect_class: WorkflowEffectClass::Build,
                adapter_kind: alias("local"),
                operation_identity: content('1'),
            },
            WorkflowNode {
                id: alias("test"),
                kind: alias("test"),
                effect_class: WorkflowEffectClass::Test,
                adapter_kind: alias("remote"),
                operation_identity: content('2'),
            },
        ],
        edges: vec![WorkflowEdge {
            source: alias("build"),
            target: alias("test"),
        }],
        controls: WorkflowControls {
            cancel: true,
            pause: true,
            resume: true,
        },
        bounds: WorkflowBounds {
            max_nodes: MAX_NODES,
            max_edges: MAX_EDGES,
            max_log_references: MAX_LOG_REFERENCES,
            max_log_read_bytes: MAX_LOG_READ_BYTES,
            max_total_log_bytes: MAX_TOTAL_LOG_BYTES,
        },
    }
}

fn completed(plan: &WorkflowEffectPlan) -> WorkflowObservation {
    WorkflowObservation {
        effect_id: plan.effect_id.clone(),
        attempt_id: plan.attempt_id.clone(),
        candidate_id: plan.candidate_id.clone(),
        pipeline_revision_id: plan.pipeline_revision_id.clone(),
        workflow_policy_id: plan.workflow_policy_id.clone(),
        provider_profile: plan.provider_profile.clone(),
        idempotency_key: plan.idempotency_key.clone(),
        workflow: external("workflow", "workflow"),
        execution: external("execution", "execution"),
        sequence: TERMINAL_SEQUENCE,
        state: WorkflowState::Completed,
        terminal: true,
        successful: true,
        outcome: Outcome::Success,
        nodes: vec![
            WorkflowNodeObservation {
                node_id: alias("build"),
                state: WorkflowNodeState::Completed,
                terminal: true,
                successful: true,
                adapter: external("adapter", "build-adapter"),
                receipt: Some(external("receipt", "build-receipt")),
            },
            WorkflowNodeObservation {
                node_id: alias("test"),
                state: WorkflowNodeState::Completed,
                terminal: true,
                successful: true,
                adapter: external("adapter", "test-adapter"),
                receipt: Some(external("receipt", "test-receipt")),
            },
        ],
        logs: Vec::new(),
        receipt: Some(external("receipt", "workflow-receipt")),
    }
}

// r[verify kiln.lattice.request.scenario.valid]
#[test]
fn ordered_acyclic_plan_is_admitted() {
    assert_eq!(validate_workflow_plan(&plan()), Ok(()));
}

// r[verify kiln.lattice.request.scenario.graph]
#[test]
fn cyclic_plan_is_rejected() {
    let mut plan = plan();
    plan.edges.push(WorkflowEdge {
        source: alias("test"),
        target: alias("build"),
    });
    assert!(validate_workflow_plan(&plan).is_err());
}

// r[verify kiln.lattice.observation.scenario.complete]
#[test]
fn linked_completion_maps_to_success() {
    let plan = plan();
    assert_eq!(
        admit_workflow_observation(&plan, &completed(&plan)),
        Ok(Outcome::Success)
    );
}

// r[verify kiln.lattice.observation.scenario.pause]
#[test]
fn paused_execution_cannot_claim_terminal_success() {
    let plan = plan();
    let mut observation = completed(&plan);
    observation.state = WorkflowState::Paused;
    assert!(admit_workflow_observation(&plan, &observation).is_err());
}

// r[verify kiln.lattice.reconciliation.scenario.stale]
#[test]
fn stale_attempt_is_rejected() {
    let plan = plan();
    let mut observation = completed(&plan);
    observation.attempt_id = AttemptId::new("stale-attempt").unwrap_or_else(|_| unreachable!());
    assert!(admit_workflow_observation(&plan, &observation).is_err());
}
