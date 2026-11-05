//! Pure planning for Lattice-backed agent jobs.
//!
//! Kiln admits which agent work may be dispatched and how its evidence counts.
//! Lattice executes the agent under its own runtime, budgets, approvals, and
//! the permission check that an agent cannot widen its access. This module
//! does not call a model, start a process, or grant merge or Patchgate authority.

use alloc::collections::BTreeSet;
use alloc::string::ToString;
use alloc::vec;

use crate::change_review::AgentVotes;
use crate::domain::Outcome;
use crate::types::{
    Alias, AttemptId, CandidateId, ContentId, CoreResult, Diagnostic, EffectId, PipelineRevisionId,
    PolicyId, ProviderProfileId,
};
use crate::workflow::{
    WorkflowBounds, WorkflowControlAction, WorkflowControls, WorkflowEffectClass,
    WorkflowEffectPlan, WorkflowNode, WorkflowObservation, WorkflowState,
    admit_workflow_observation,
};

/// Domain label mixed into the operation identity.
pub const AGENT_JOB_DOMAIN: &str = "kiln.lattice-agent-job.v1";
/// One agent invocation per admitted job. A later round is a new job.
pub const MAX_AGENT_INVOCATIONS: u32 = 1;
/// Token ceiling for one job, matching Lattice's guarded per-round bound.
pub const MAX_AGENT_TOKENS: u32 = 32_768;
/// Duration ceiling for one job, in milliseconds.
pub const MAX_AGENT_DURATION_MILLIS: u64 = 900_000;
/// Platform the Lattice agent cohort admits.
pub const AGENT_PLATFORM: &str = "linux-x86_64";
/// Single node in an agent review workflow.
pub const AGENT_NODE_ID: &str = "agent-review";
/// Lattice engine node kind for an agent.
pub const LATTICE_AGENT_NODE_KIND: &str = "agent";
/// Adapter label that selects a reviewed Lattice agent route, not a shell.
pub const LATTICE_AGENT_ADAPTER_KIND: &str = "agent";
/// Required Lattice ability. The agent cannot replace it with a wider one.
pub const PERMISSION_EXECUTE: &str = "agent/execute";
/// Read the exact candidate the job names.
pub const PERMISSION_READ_CANDIDATE: &str = "read/candidate";
/// Write a review note. This is not merge, status publication, or Patchgate.
pub const PERMISSION_WRITE_REVIEW_NOTE: &str = "write/review-note";

const PERMISSION_CEILING: [&str; 3] = [
    PERMISSION_EXECUTE,
    PERMISSION_READ_CANDIDATE,
    PERMISSION_WRITE_REVIEW_NOTE,
];
const HARNESS_PI: &str = "pi";
const HARNESS_CLANKERS: &str = "clankers";
const HARNESS_ANIMUS: &str = "animus";

/// Budget submitted with the job. Lattice enforces the reviewed route's budget.
/// Kiln refuses a job that asks for more than these named ceilings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgentBudget {
    pub invocations: u32,
    pub tokens: u32,
    pub duration_millis: u64,
}

/// One agent review job. Equal admitted jobs produce equal Lattice plans.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentOrchestrationJob {
    pub effect_id: EffectId,
    pub attempt_id: AttemptId,
    pub candidate_id: CandidateId,
    pub pipeline_revision_id: PipelineRevisionId,
    pub workflow_policy_id: PolicyId,
    pub provider_profile: ProviderProfileId,
    pub idempotency_key: ContentId,
    /// Lattice target name: `pi`, `clankers`, or `animus`.
    pub harness: Alias,
    /// Declared model metadata. Lattice owns model selection inside the route.
    pub model: Option<Alias>,
    /// Identity of the reviewed prompt. The prompt body stays in Lattice.
    pub prompt_identity: ContentId,
    /// Kiln vote policy. Lattice success does not apply this policy.
    pub votes: AgentVotes,
    pub budget: AgentBudget,
    /// Closed permission set. It must include `agent/execute` and cannot grow
    /// past the ceiling.
    pub permissions: BTreeSet<Alias>,
    pub bounds: WorkflowBounds,
}

/// Evidence that Lattice observed an agent run. It is not an approval.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRunEvidence {
    pub effect_id: EffectId,
    pub candidate_id: CandidateId,
    pub execution: crate::types::ExternalRef,
    pub votes: AgentVotes,
    pub operation_identity: ContentId,
    pub process_succeeded: bool,
    /// Always false. A Lattice observation does not satisfy review policy.
    pub counts_as_approval: bool,
    /// Always false. Patchgate receipts stay on the Patchgate path.
    pub is_patchgate_receipt: bool,
}

// r[impl kiln.lattice.agent.plan]
pub fn plan_lattice_agent_job(job: &AgentOrchestrationJob) -> CoreResult<WorkflowEffectPlan> {
    admit_job(job)?;
    let operation_identity = operation_identity(job)?;
    let plan = WorkflowEffectPlan {
        effect_id: job.effect_id.clone(),
        attempt_id: job.attempt_id.clone(),
        candidate_id: job.candidate_id.clone(),
        pipeline_revision_id: job.pipeline_revision_id.clone(),
        workflow_policy_id: job.workflow_policy_id.clone(),
        provider_profile: job.provider_profile.clone(),
        idempotency_key: job.idempotency_key.clone(),
        platform: Alias::new(AGENT_PLATFORM)?,
        source: None,
        nodes: vec![WorkflowNode {
            id: Alias::new(AGENT_NODE_ID)?,
            kind: Alias::new(LATTICE_AGENT_NODE_KIND)?,
            effect_class: WorkflowEffectClass::Custom,
            adapter_kind: Alias::new(LATTICE_AGENT_ADAPTER_KIND)?,
            operation_identity,
        }],
        edges: vec![],
        controls: WorkflowControls {
            cancel: true,
            pause: false,
            resume: false,
        },
        bounds: job.bounds,
    };
    crate::workflow::validate_workflow_plan(&plan)?;
    Ok(plan)
}

/// Pause and resume are not part of the Lattice handler Kiln admits.
pub fn admit_agent_control(action: WorkflowControlAction) -> CoreResult<()> {
    match action {
        WorkflowControlAction::Cancel => Ok(()),
        WorkflowControlAction::Pause | WorkflowControlAction::Resume => Err(error(
            "agent_control_unsupported",
            "agent.control",
            "Lattice agent jobs admit cancel only",
        )),
    }
}

// r[impl kiln.lattice.agent.evidence]
pub fn admit_agent_run_evidence(
    job: &AgentOrchestrationJob,
    plan: &WorkflowEffectPlan,
    observation: &WorkflowObservation,
) -> CoreResult<AgentRunEvidence> {
    let expected = plan_lattice_agent_job(job)?;
    if plan != &expected {
        return Err(error(
            "agent_plan_mismatch",
            "agent.plan",
            "agent observation is not bound to the admitted Lattice plan",
        ));
    }
    let outcome = admit_workflow_observation(plan, observation)?;
    let process_succeeded =
        observation.state == WorkflowState::Completed && outcome == Outcome::Success;
    let operation_identity = plan
        .nodes
        .first()
        .map(|node| node.operation_identity.clone())
        .ok_or_else(|| {
            error(
                "agent_plan_node",
                "agent.plan",
                "admitted agent plan has no node",
            )
        })?;
    Ok(AgentRunEvidence {
        effect_id: job.effect_id.clone(),
        candidate_id: job.candidate_id.clone(),
        execution: observation.execution.clone(),
        votes: job.votes,
        operation_identity,
        process_succeeded,
        counts_as_approval: false,
        is_patchgate_receipt: false,
    })
}

fn admit_job(job: &AgentOrchestrationJob) -> CoreResult<()> {
    match job.harness.as_str() {
        HARNESS_PI | HARNESS_CLANKERS | HARNESS_ANIMUS => {}
        _ => {
            return Err(error(
                "agent_harness",
                "agent.harness",
                "agent harness must be a Lattice target: pi, clankers, or animus",
            ));
        }
    }
    let budget = job.budget;
    if budget.invocations == 0
        || budget.invocations > MAX_AGENT_INVOCATIONS
        || budget.tokens == 0
        || budget.tokens > MAX_AGENT_TOKENS
        || budget.duration_millis == 0
        || budget.duration_millis > MAX_AGENT_DURATION_MILLIS
    {
        return Err(error(
            "agent_budget",
            "agent.budget",
            "agent budget exceeds the named Lattice ceiling or is empty",
        ));
    }
    let mut saw_execute = false;
    for permission in &job.permissions {
        if !PERMISSION_CEILING.contains(&permission.as_str()) {
            return Err(error(
                "agent_permission_widened",
                "agent.permissions",
                "agent permissions cannot exceed the reviewed ceiling",
            ));
        }
        if permission.as_str() == PERMISSION_EXECUTE {
            saw_execute = true;
        }
    }
    if !saw_execute {
        return Err(error(
            "agent_permission_missing",
            "agent.permissions",
            "agent jobs require the lattice agent/execute ability",
        ));
    }
    Ok(())
}

fn operation_identity(job: &AgentOrchestrationJob) -> CoreResult<ContentId> {
    let mut hasher = blake3::Hasher::new();
    field(&mut hasher, "domain", AGENT_JOB_DOMAIN.as_bytes());
    field(&mut hasher, "votes", vote_label(job.votes).as_bytes());
    field(&mut hasher, "harness", job.harness.as_str().as_bytes());
    let model = match &job.model {
        Some(model) => model.as_str(),
        None => "",
    };
    field(&mut hasher, "model", model.as_bytes());
    field(
        &mut hasher,
        "prompt",
        job.prompt_identity.as_str().as_bytes(),
    );
    field(
        &mut hasher,
        "invocations",
        &job.budget.invocations.to_le_bytes(),
    );
    field(&mut hasher, "tokens", &job.budget.tokens.to_le_bytes());
    field(
        &mut hasher,
        "duration_millis",
        &job.budget.duration_millis.to_le_bytes(),
    );
    let count = u64::try_from(job.permissions.len()).unwrap_or(u64::MAX);
    field(&mut hasher, "permission_count", &count.to_le_bytes());
    for permission in &job.permissions {
        field(&mut hasher, "permission", permission.as_str().as_bytes());
    }
    ContentId::from_blake3_hex(hasher.finalize().to_hex().to_string())
}

fn vote_label(votes: AgentVotes) -> &'static str {
    match votes {
        AgentVotes::Advisory => "advisory",
        AgentVotes::Binding => "binding",
    }
}

fn field(hasher: &mut blake3::Hasher, label: &str, value: &[u8]) {
    let label_length = u64::try_from(label.len()).unwrap_or(u64::MAX);
    let value_length = u64::try_from(value.len()).unwrap_or(u64::MAX);
    hasher.update(&label_length.to_le_bytes());
    hasher.update(label.as_bytes());
    hasher.update(&value_length.to_le_bytes());
    hasher.update(value);
}

fn error(code: &str, field_name: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, field_name, message)
}

#[cfg(test)]
mod tests {
    use alloc::string::String;

    use super::*;
    use crate::types::ExternalRef;
    use crate::workflow::{WorkflowNodeObservation, WorkflowNodeState, WorkflowObservation};

    fn hex(character: char) -> ContentId {
        ContentId::from_blake3_hex(character.to_string().repeat(64))
            .unwrap_or_else(|_| unreachable!())
    }

    fn alias(value: &str) -> Alias {
        Alias::new(value).unwrap_or_else(|_| unreachable!())
    }

    fn permissions(values: &[&str]) -> BTreeSet<Alias> {
        values.iter().copied().map(alias).collect()
    }

    fn job() -> AgentOrchestrationJob {
        AgentOrchestrationJob {
            effect_id: EffectId::new("effect").unwrap_or_else(|_| unreachable!()),
            attempt_id: AttemptId::new("attempt").unwrap_or_else(|_| unreachable!()),
            candidate_id: CandidateId::new("candidate").unwrap_or_else(|_| unreachable!()),
            pipeline_revision_id: PipelineRevisionId::new("pipeline")
                .unwrap_or_else(|_| unreachable!()),
            workflow_policy_id: PolicyId::new("policy").unwrap_or_else(|_| unreachable!()),
            provider_profile: ProviderProfileId::new("lattice-agent-v1")
                .unwrap_or_else(|_| unreachable!()),
            idempotency_key: hex('a'),
            harness: alias(HARNESS_PI),
            model: Some(alias("pi-model")),
            prompt_identity: hex('b'),
            votes: AgentVotes::Advisory,
            budget: AgentBudget {
                invocations: MAX_AGENT_INVOCATIONS,
                tokens: MAX_AGENT_TOKENS,
                duration_millis: MAX_AGENT_DURATION_MILLIS,
            },
            permissions: permissions(&[
                PERMISSION_WRITE_REVIEW_NOTE,
                PERMISSION_EXECUTE,
                PERMISSION_READ_CANDIDATE,
            ]),
            bounds: WorkflowBounds {
                max_nodes: 4,
                max_edges: 4,
                max_log_references: 4,
                max_log_read_bytes: 1024,
                max_total_log_bytes: 4096,
            },
        }
    }

    fn external(kind: &str, id: &str) -> ExternalRef {
        ExternalRef::new("lattice", kind, id).unwrap_or_else(|_| unreachable!())
    }

    fn observation(plan: &WorkflowEffectPlan, state: WorkflowState) -> WorkflowObservation {
        let terminal = state.is_terminal();
        let successful = state == WorkflowState::Completed;
        let node_state = match state {
            WorkflowState::Completed => WorkflowNodeState::Completed,
            WorkflowState::Failed => WorkflowNodeState::Failed,
            WorkflowState::Cancelled => WorkflowNodeState::Cancelled,
            WorkflowState::Paused => WorkflowNodeState::Paused,
            WorkflowState::Running => WorkflowNodeState::Running,
            _ => WorkflowNodeState::Pending,
        };
        let node_id = plan
            .nodes
            .first()
            .map(|node| node.id.clone())
            .unwrap_or_else(|| unreachable!());
        WorkflowObservation {
            effect_id: plan.effect_id.clone(),
            attempt_id: plan.attempt_id.clone(),
            candidate_id: plan.candidate_id.clone(),
            pipeline_revision_id: plan.pipeline_revision_id.clone(),
            workflow_policy_id: plan.workflow_policy_id.clone(),
            provider_profile: plan.provider_profile.clone(),
            idempotency_key: plan.idempotency_key.clone(),
            workflow: external("workflow", "workflow-1"),
            execution: external("execution", "execution-1"),
            sequence: 1,
            state,
            terminal,
            successful,
            outcome: if successful {
                Outcome::Success
            } else if state == WorkflowState::Failed {
                Outcome::DomainFailure
            } else {
                Outcome::Pending
            },
            nodes: vec![WorkflowNodeObservation {
                node_id,
                state: node_state,
                terminal: node_state.is_terminal(),
                successful: node_state.is_successful(),
                adapter: external("adapter", LATTICE_AGENT_ADAPTER_KIND),
                receipt: node_state
                    .is_terminal()
                    .then(|| external("receipt", "node-receipt")),
            }],
            logs: vec![],
            receipt: terminal.then(|| external("receipt", "run-receipt")),
        }
    }

    fn error_code(result: CoreResult<WorkflowEffectPlan>) -> String {
        match result {
            Ok(_) => unreachable!(),
            Err(error) => error.code,
        }
    }

    #[test]
    fn equal_jobs_plan_the_same_cancel_only_agent_workflow() {
        let plan = plan_lattice_agent_job(&job()).unwrap_or_else(|_| unreachable!());
        let again = plan_lattice_agent_job(&job()).unwrap_or_else(|_| unreachable!());
        assert_eq!(plan, again);
        assert!(!plan.controls.pause);
        assert!(!plan.controls.resume);
        assert!(plan.controls.cancel);
        let node = plan.nodes.first().unwrap_or_else(|| unreachable!());
        assert_eq!(node.kind.as_str(), LATTICE_AGENT_NODE_KIND);
        assert_eq!(node.adapter_kind.as_str(), LATTICE_AGENT_ADAPTER_KIND);
        assert_eq!(node.effect_class, WorkflowEffectClass::Custom);
        assert!(plan.edges.is_empty());
    }

    #[test]
    fn permission_order_does_not_change_the_operation_identity() {
        let mut reversed = job();
        reversed.permissions = permissions(&[
            PERMISSION_EXECUTE,
            PERMISSION_READ_CANDIDATE,
            PERMISSION_WRITE_REVIEW_NOTE,
        ]);
        let left = plan_lattice_agent_job(&job()).unwrap_or_else(|_| unreachable!());
        let right = plan_lattice_agent_job(&reversed).unwrap_or_else(|_| unreachable!());
        assert_eq!(left.nodes, right.nodes);
    }

    #[test]
    fn budget_vote_and_model_changes_require_a_new_operation() {
        let base = plan_lattice_agent_job(&job()).unwrap_or_else(|_| unreachable!());
        let base_identity = base
            .nodes
            .first()
            .map(|node| node.operation_identity.clone())
            .unwrap_or_else(|| unreachable!());
        let mut fewer_tokens = job();
        fewer_tokens.budget.tokens = MAX_AGENT_TOKENS - 1;
        let mut binding = job();
        binding.votes = AgentVotes::Binding;
        let mut no_model = job();
        no_model.model = None;
        for changed in [fewer_tokens, binding, no_model] {
            let plan = plan_lattice_agent_job(&changed).unwrap_or_else(|_| unreachable!());
            let identity = plan
                .nodes
                .first()
                .map(|node| &node.operation_identity)
                .unwrap_or_else(|| unreachable!());
            assert_ne!(identity, &base_identity);
        }
    }

    #[test]
    fn widened_permissions_and_over_budget_fail_before_a_plan_exists() {
        let mut widened = job();
        widened.permissions.insert(alias("grant/authority"));
        assert_eq!(
            error_code(plan_lattice_agent_job(&widened)),
            "agent_permission_widened"
        );
        let mut missing = job();
        missing.permissions = permissions(&[PERMISSION_READ_CANDIDATE]);
        assert_eq!(
            error_code(plan_lattice_agent_job(&missing)),
            "agent_permission_missing"
        );
        let mut over = job();
        over.budget.invocations = MAX_AGENT_INVOCATIONS + 1;
        assert_eq!(error_code(plan_lattice_agent_job(&over)), "agent_budget");
        let mut empty = job();
        empty.budget.tokens = 0;
        assert_eq!(error_code(plan_lattice_agent_job(&empty)), "agent_budget");
        let mut harness = job();
        harness.harness = alias("omp");
        assert_eq!(
            error_code(plan_lattice_agent_job(&harness)),
            "agent_harness"
        );
    }

    #[test]
    fn pause_and_resume_are_rejected() {
        assert!(admit_agent_control(WorkflowControlAction::Cancel).is_ok());
        assert_eq!(
            admit_agent_control(WorkflowControlAction::Pause)
                .err()
                .map(|error| error.code)
                .unwrap_or_else(|| unreachable!()),
            "agent_control_unsupported"
        );
        assert_eq!(
            admit_agent_control(WorkflowControlAction::Resume)
                .err()
                .map(|error| error.code)
                .unwrap_or_else(|| unreachable!()),
            "agent_control_unsupported"
        );
    }

    #[test]
    fn binding_policy_does_not_turn_lattice_success_into_an_approval() {
        let mut binding = job();
        binding.votes = AgentVotes::Binding;
        let plan = plan_lattice_agent_job(&binding).unwrap_or_else(|_| unreachable!());
        let evidence = admit_agent_run_evidence(
            &binding,
            &plan,
            &observation(&plan, WorkflowState::Completed),
        )
        .unwrap_or_else(|_| unreachable!());
        assert!(evidence.process_succeeded);
        assert!(!evidence.counts_as_approval);
        assert!(!evidence.is_patchgate_receipt);
        assert_eq!(evidence.votes, AgentVotes::Binding);
        let failed =
            admit_agent_run_evidence(&binding, &plan, &observation(&plan, WorkflowState::Failed))
                .unwrap_or_else(|_| unreachable!());
        assert!(!failed.process_succeeded);
        assert!(!failed.counts_as_approval);
        let mut other = binding.clone();
        other.candidate_id = CandidateId::new("other").unwrap_or_else(|_| unreachable!());
        assert!(
            admit_agent_run_evidence(&other, &plan, &observation(&plan, WorkflowState::Completed))
                .is_err()
        );
    }
}
