//! Read-only composition. Original validators retain stage authority.
use crate::{
    canonical_json_hash, evaluate_authoritative_agent_completion, GateReport,
    WorkflowArtifactState, WorkflowStatusReport,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const MAX_READINESS_BLOCKERS: usize = 512;
pub const MAX_READINESS_OBSERVATION_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_READINESS_TARGET_BYTES: usize = 4096;
mod workflow;

pub use workflow::{
    admit_workflow_readiness_policy, select_drain_readiness, workflow_readiness_policy,
    DrainChangeSelection, DrainInstructionRoute, DrainNextTask, DrainReadinessChange,
    DrainReadinessSelection, DrainSelectedAction, DrainTaskOrdering, WorkflowReadinessBounds,
    WorkflowReadinessInstruction, WorkflowReadinessPolicy, WORKFLOW_READINESS_BOUND,
    WORKFLOW_READINESS_POLICY_BOUNDS, WORKFLOW_READINESS_POLICY_INVALID,
};

pub const READINESS_STAGES: [&str; 7] = [
    "artifact",
    "proposal",
    "design",
    "tasks",
    "completion",
    "sync",
    "archive",
];
mod view;
pub use view::{
    agent_view_policy, project_agent_readiness, readiness_detail, AgentViewRequest, DetailKind,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadinessScope {
    pub change: String,
    pub policy_hash: String,
    pub source_hash: String,
    pub evidence_hash: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadinessState {
    Ready,
    Blocked,
    Unavailable,
    NotApplicable,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessCause {
    pub code: String,
    pub target: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prerequisites: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessObservation {
    pub scope: ReadinessScope,
    pub stage: String,
    pub state: ReadinessState,
    pub decision: Value,
    pub causes: Vec<ReadinessCause>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessSnapshot {
    pub scope: ReadinessScope,
    pub observations: Vec<ReadinessObservation>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessBlocker {
    pub id: String,
    pub stage: String,
    pub code: String,
    pub target: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prerequisites: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessAction {
    pub id: String,
    pub kind: String,
    pub causes: Vec<String>,
    pub target: String,
    pub prerequisites: Vec<String>,
    pub authority: String,
    pub expected_observation: String,
    pub argv: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessStage {
    pub stage: String,
    pub state: ReadinessState,
    pub decision_hash: String,
    pub decision: Value,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessReport {
    pub schema: String,
    pub scope: ReadinessScope,
    pub valid: bool,
    pub classification: String,
    pub stages: Vec<ReadinessStage>,
    pub blockers: Vec<ReadinessBlocker>,
    pub actions: Vec<ReadinessAction>,
    pub diagnostics: Vec<String>,
    pub readiness_policy_hash: String,
    pub input_hash: String,
    pub receipt_hash: String,
    pub mutation_authorized: bool,
    pub non_claims: Vec<String>,
}

/// Inputs required by a profile gate, including artifact dependency closure.
/// The admitted schema remains unchanged; observations outside this set are not
/// evidence for this gate and must never be presented as a full profile report.
pub fn readiness_gate_artifact_ids(
    schema: &crate::WorkflowSchemaPolicy,
    stage: crate::GateStage,
) -> Vec<String> {
    let mut selected = std::collections::BTreeSet::new();
    for artifact in &schema.artifacts {
        let applies = match stage {
            crate::GateStage::Proposal => artifact.stage == "proposal",
            crate::GateStage::Design => matches!(artifact.stage.as_str(), "proposal" | "design"),
            crate::GateStage::Tasks => {
                matches!(artifact.stage.as_str(), "proposal" | "design" | "tasks")
            }
        };
        // Acceptance and delta sources feed the original gate's reference
        // adapter. Outcome-machine evaluation emits design-stage issues even
        // when its artifact is assigned a later stage in a custom schema.
        let evaluator_input = matches!(
            artifact.structural_admission.as_str(),
            crate::WORKFLOW_ADMISSION_ACCEPTANCE | crate::WORKFLOW_ADMISSION_DELTA_SPECS
        ) || (stage != crate::GateStage::Proposal
            && artifact.structural_admission == crate::WORKFLOW_ADMISSION_OUTCOME_MACHINE);
        let review_input = stage == crate::GateStage::Tasks
            && schema
                .review_obligations
                .iter()
                .any(|review| review.artifact_id == artifact.id);
        if applies || evaluator_input || review_input {
            selected.insert(artifact.id.clone());
        }
    }
    // An admitted schema is finite and acyclic. At most one new node per pass
    // is needed; the schema's artifact bound therefore also bounds this walk.
    for _ in 0..schema.artifacts.len() {
        let before = selected.len();
        for artifact in &schema.artifacts {
            if selected.contains(&artifact.id) {
                selected.extend(artifact.requires.iter().cloned());
            }
        }
        // Review freshness binds required_artifacts, not only the artifact
        // workflow's requires edges. Follow both edge kinds to a fixed point.
        for review in &schema.review_obligations {
            if selected.contains(&review.artifact_id) {
                selected.extend(review.required_artifacts.iter().cloned());
            }
        }
        if before == selected.len() {
            break;
        }
    }
    selected.into_iter().collect()
}

pub fn readiness_cause(code: &str, target: &str, message: &str) -> ReadinessCause {
    ReadinessCause {
        code: code.into(),
        target: target.into(),
        message: message.into(),
        prerequisites: Vec::new(),
    }
}

fn readiness_cause_with_prerequisites(
    code: &str,
    target: &str,
    message: &str,
    prerequisites: Vec<String>,
) -> ReadinessCause {
    ReadinessCause {
        code: code.into(),
        target: target.into(),
        message: message.into(),
        prerequisites,
    }
}

// r[impl cairn.unified_readiness.decisions]
pub fn readiness_gate(scope: &ReadinessScope, report: &GateReport) -> ReadinessObservation {
    let causes = report
        .issues
        .iter()
        .map(|issue| {
            readiness_cause(
                &issue.code,
                &format!(".cairn/changes/{}/{}.md", scope.change, report.stage),
                &issue.message,
            )
        })
        .collect::<Vec<_>>();
    ReadinessObservation {
        scope: scope.clone(),
        stage: report.stage.clone(),
        state: if causes.is_empty() {
            ReadinessState::Ready
        } else {
            ReadinessState::Blocked
        },
        decision: json!({"change": report.change, "stage": report.stage, "verdict": report.verdict,
            "issues": causes}),
        causes,
    }
}

/// Bind adapter-owned source targets to the original validator's exact issues.
/// Equal display messages from different producers retain each distinct target.
pub fn readiness_gate_with_external_causes(
    scope: &ReadinessScope,
    report: &GateReport,
    external: &[ReadinessCause],
) -> ReadinessObservation {
    let mut observation = readiness_gate(scope, report);
    if external.iter().any(|cause| {
        !report
            .issues
            .iter()
            .any(|issue| issue.code == cause.code && issue.message == cause.message)
    }) {
        observation.state = ReadinessState::Unavailable;
        observation.causes.push(readiness_cause(
            "observation.cause_conflict",
            &scope.change,
            "A supplied source target has no matching validator issue",
        ));
        return observation;
    }
    observation.causes = observation
        .causes
        .into_iter()
        .flat_map(|cause| {
            let replacements = external
                .iter()
                .filter(|candidate| {
                    candidate.code == cause.code && candidate.message == cause.message
                })
                .cloned()
                .collect::<Vec<_>>();
            if replacements.is_empty() {
                vec![cause]
            } else {
                replacements
            }
        })
        .collect();
    observation
}

pub fn readiness_artifacts(
    scope: &ReadinessScope,
    report: &WorkflowStatusReport,
) -> ReadinessObservation {
    let mut causes = report
        .artifacts
        .iter()
        .filter(|a| {
            !matches!(
                a.state,
                WorkflowArtifactState::Done | WorkflowArtifactState::Skipped
            )
        })
        .map(|a| {
            readiness_cause(
                "artifact.incomplete",
                &format!(".cairn/changes/{}/{}", scope.change, a.output_path),
                &format!("{} is {}", a.id, a.state.as_str()),
            )
        })
        .collect::<Vec<_>>();
    causes.extend(
        report
            .issues
            .iter()
            .map(|issue| readiness_cause("workflow.invalid", &scope.change, issue)),
    );
    ReadinessObservation {
        scope: scope.clone(),
        stage: "artifact".into(),
        state: if causes.is_empty() && report.valid {
            ReadinessState::Ready
        } else {
            ReadinessState::Blocked
        },
        decision: json!(report),
        causes,
    }
}

pub fn readiness_profile_causes(
    scope: &ReadinessScope,
    report: &mut ReadinessObservation,
    evaluation: &crate::WorkflowProfileEvaluation,
    schema: &crate::WorkflowSchemaPolicy,
    evidence: &crate::WorkflowProfileEvidence,
) {
    for obligation in &schema.review_obligations {
        if !evaluation.issues.iter().any(|issue| {
            issue.code == "cairn.workflow.profile.review"
                && issue.artifact_id.as_ref() == Some(&obligation.artifact_id)
        }) {
            continue;
        }
        let reviewed = evidence
            .reviews
            .iter()
            .find(|r| r.obligation_id == obligation.id);
        for artifact in evidence
            .artifact_hashes
            .iter()
            .filter(|a| obligation.required_artifacts.contains(&a.artifact_id))
        {
            let current = reviewed.is_some_and(|review| {
                review
                    .reviewed_inputs
                    .iter()
                    .any(|input| input.path == artifact.path && input.hash == artifact.blake3)
            });
            if !current {
                report.causes.push(readiness_cause(
                    "cairn.workflow.profile.review",
                    &format!(".cairn/changes/{}/{}", scope.change, artifact.path),
                    &format!(
                        "Review {} must bind current artifact {}",
                        obligation.id, artifact.blake3
                    ),
                ));
            }
        }
    }
    for issue in &evaluation.issues {
        let target = issue
            .artifact_id
            .as_ref()
            .and_then(|id| schema.artifacts.iter().find(|a| &a.id == id))
            .map(|artifact| format!(".cairn/changes/{}/{}", scope.change, artifact.output_path))
            .unwrap_or_else(|| scope.change.clone());
        report
            .causes
            .push(readiness_cause(&issue.code, &target, &issue.message));
    }
    if !report.causes.is_empty() {
        report.state = ReadinessState::Blocked;
    }
    report.decision["profile_evaluation"] = json!(evaluation);
}

fn completion_producer_prerequisites(contract: &Value, check_id: &str) -> Vec<String> {
    contract
        .get("required_checks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|check| check.get("check_id").and_then(Value::as_str) == Some(check_id))
        .flat_map(|check| {
            let mut prerequisites = Vec::new();
            if let Some(command_hash) = check.get("command_hash").and_then(Value::as_str) {
                prerequisites.push(format!("producer-contract:{check_id}:{command_hash}"));
            }
            prerequisites.extend(
                string_array(&check["prerequisites"])
                    .unwrap_or_default()
                    .into_iter()
                    .map(|value| format!("producer-prerequisite:{check_id}:{value}")),
            );
            prerequisites
        })
        .collect()
}

pub fn readiness_completion(
    scope: &ReadinessScope,
    contract: &Value,
    results: &Value,
    subject: &str,
) -> ReadinessObservation {
    let report =
        evaluate_authoritative_agent_completion(contract, results, subject, &scope.policy_hash);
    let causes = report
        .diagnostics
        .iter()
        .map(|d| {
            let target = d.check_id.as_deref().unwrap_or("verification-contract");
            readiness_cause_with_prerequisites(
                &d.code,
                target,
                &d.summary,
                completion_producer_prerequisites(contract, target),
            )
        })
        .collect();
    ReadinessObservation {
        scope: scope.clone(),
        stage: "completion".into(),
        state: if report.valid && report.verdict == "PASS" {
            ReadinessState::Ready
        } else {
            ReadinessState::Blocked
        },
        decision: json!(report),
        causes,
    }
}

pub struct ReadinessMutationInputs<'a> {
    pub scope: &'a ReadinessScope,
    pub sync: &'a crate::SyncPlan,
    pub archive: &'a crate::ArchivePlan,
    pub merge_binding: Option<Value>,
    pub merge_reasons: &'a [String],
    pub gate_reasons: &'a [String],
    pub profile_reasons: &'a [String],
    pub policy: &'a crate::Policy,
    pub dependencies: &'a crate::ChangeDependencyGraph,
}

/// Match the original preview validators: a blocked base plan does not consume
/// mutation-gate observations. Profile blockers are added after this decision.
pub fn readiness_mutation_gate_requirements(
    policy: &crate::Policy,
    sync: &crate::SyncPlan,
    archive: &crate::ArchivePlan,
) -> (bool, bool) {
    (
        policy.sync_require_gate && !sync.blocked,
        policy.archive_require_gate && !archive.blocked,
    )
}

pub fn readiness_mutation_observations(
    input: ReadinessMutationInputs<'_>,
) -> Vec<ReadinessObservation> {
    let (sync_gate, archive_gate) =
        readiness_mutation_gate_requirements(input.policy, input.sync, input.archive);
    let mut sync_reasons = input.sync.reasons.clone();
    sync_reasons.extend_from_slice(input.profile_reasons);
    sync_reasons.extend_from_slice(input.merge_reasons);
    if sync_gate {
        sync_reasons.extend_from_slice(input.gate_reasons);
    }
    let mut archive_reasons = input.archive.reasons.clone();
    archive_reasons.extend_from_slice(input.profile_reasons);
    if archive_gate {
        archive_reasons.extend_from_slice(input.gate_reasons);
    }
    [("sync", sync_reasons), ("archive", archive_reasons)]
        .into_iter()
        .map(|(stage, mut reasons)| {
            reasons.sort();
            reasons.dedup();
            let mut observation = readiness_plan(
                input.scope,
                stage,
                &reasons,
                json!({"reasons": reasons, "merge_preflight": input.merge_binding}),
            );
            readiness_dependency_causes(&mut observation, input.dependencies);
            if stage == "sync" && input.merge_binding.is_none() {
                observation.state = ReadinessState::Unavailable;
                observation.causes.push(readiness_cause(
                    "observation.load_failed",
                    &input.scope.change,
                    "The sync merge observation could not be admitted",
                ));
            }
            observation
        })
        .collect()
}

pub fn readiness_dependency_causes(
    observation: &mut ReadinessObservation,
    graph: &crate::ChangeDependencyGraph,
) {
    if let Some(node) = graph.node(&observation.scope.change) {
        for dependency in &node.blocked_by {
            observation.causes.push(readiness_cause(
                "dependency.blocked",
                dependency,
                "The upstream change must satisfy the existing dependency validator",
            ));
        }
    }
    if !observation.causes.is_empty() {
        observation.state = ReadinessState::Blocked;
    }
}

pub fn readiness_plan(
    scope: &ReadinessScope,
    stage: &str,
    reasons: &[String],
    decision: Value,
) -> ReadinessObservation {
    ReadinessObservation {
        scope: scope.clone(),
        stage: stage.into(),
        state: if reasons.is_empty() {
            ReadinessState::Ready
        } else {
            ReadinessState::Blocked
        },
        decision,
        causes: reasons
            .iter()
            .map(|reason| readiness_cause("plan.blocked", &scope.change, reason))
            .collect(),
    }
}

// r[impl cairn.unified_readiness.snapshot]
// r[impl cairn.unified_readiness.boundary]
pub fn derive_readiness(snapshot: &ReadinessSnapshot) -> ReadinessReport {
    let policy: Value = serde_json::from_str(include_str!(
        "../../../../cairn-policy/generated/readiness.json"
    ))
    .expect("checked readiness policy");
    let mut diagnostics = Vec::new();
    if snapshot.observations.len() > READINESS_STAGES.len() {
        diagnostics.push("observation.bound".into());
    }
    let input = json!(snapshot);
    if input.to_string().len() > MAX_READINESS_OBSERVATION_BYTES {
        diagnostics.push("observation.byte_bound".into());
    }
    if crate::validate_change_name(&snapshot.scope.change).is_err() {
        diagnostics.push("scope.change".into());
    }
    for hash in [
        &snapshot.scope.policy_hash,
        &snapshot.scope.source_hash,
        &snapshot.scope.evidence_hash,
    ] {
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            diagnostics.push("scope.hash".into());
        }
    }
    let mut stages = Vec::new();
    let mut blockers = Vec::new();
    for stage in READINESS_STAGES {
        let observations = snapshot
            .observations
            .iter()
            .filter(|o| o.stage == stage)
            .collect::<Vec<_>>();
        let mut state = ReadinessState::Unavailable;
        let mut decision = Value::Null;
        if observations.len() == 1 {
            let observation = observations[0];
            state = observation.state;
            decision = observation.decision.clone();
            if observation.stage == "completion"
                && state == ReadinessState::Ready
                && (decision["valid"] != true || decision["verdict"] != "PASS")
            {
                diagnostics.push("completion: observation.contradictory".into());
                state = ReadinessState::Unavailable;
            }
            if decision
                .get("change")
                .and_then(Value::as_str)
                .is_some_and(|change| change != snapshot.scope.change)
            {
                diagnostics.push(format!("{stage}: decision.change_conflict"));
                state = ReadinessState::Unavailable;
            }
            if observation.scope != snapshot.scope {
                diagnostics.push(format!("{stage}: identity.conflict"));
                state = ReadinessState::Unavailable;
            }
            if observation.causes.len() > MAX_READINESS_BLOCKERS {
                diagnostics.push(format!("{stage}: blocker.bound"));
            }
            if matches!(state, ReadinessState::Ready | ReadinessState::NotApplicable)
                && !observation.causes.is_empty()
            {
                diagnostics.push(format!("{stage}: observation.contradictory"));
                state = ReadinessState::Unavailable;
            }
            for cause in observation.causes.iter().take(MAX_READINESS_BLOCKERS) {
                if !safe_target(&cause.target) {
                    diagnostics.push(format!("{stage}: target.unsafe"));
                    continue;
                }
                let id = canonical_json_hash(&json!([snapshot.scope, stage, cause]));
                blockers.push(ReadinessBlocker {
                    id,
                    stage: stage.into(),
                    code: cause.code.clone(),
                    target: cause.target.clone(),
                    message: cause.message.clone(),
                    prerequisites: cause.prerequisites.clone(),
                });
            }
        } else if observations.len() > 1 {
            diagnostics.push(format!("{stage}: observation.duplicate"));
        }
        stages.push(ReadinessStage {
            stage: stage.into(),
            state,
            decision_hash: canonical_json_hash(&decision),
            decision,
        });
    }
    if snapshot
        .observations
        .iter()
        .any(|o| !READINESS_STAGES.contains(&o.stage.as_str()))
    {
        diagnostics.push("observation.unknown".into());
    }
    if blockers.len() > MAX_READINESS_BLOCKERS {
        diagnostics.push("blocker.total_bound".into());
        blockers.truncate(MAX_READINESS_BLOCKERS);
    }
    diagnostics.sort();
    diagnostics.dedup();
    blockers.sort_by(|a, b| {
        (&a.stage, &a.code, &a.target, &a.id).cmp(&(&b.stage, &b.code, &b.target, &b.id))
    });
    blockers.dedup();
    // Missing or failed completion never becomes authority for a dependent preview.
    let completion = stages
        .iter()
        .find(|s| s.stage == "completion")
        .map(|s| s.state);
    if !matches!(
        completion,
        Some(ReadinessState::Ready | ReadinessState::NotApplicable)
    ) {
        for stage in stages
            .iter_mut()
            .filter(|s| matches!(s.stage.as_str(), "sync" | "archive"))
        {
            if stage.state == ReadinessState::Ready {
                stage.state = ReadinessState::Blocked;
            }
        }
    }
    if !diagnostics.is_empty() {
        for stage in &mut stages {
            stage.state = ReadinessState::Unavailable;
        }
    }
    let mut actions = blockers
        .iter()
        .map(|b| action_for(b, &snapshot.scope, &policy))
        .collect::<Vec<_>>();
    for stage in stages
        .iter()
        .filter(|s| s.state == ReadinessState::Unavailable)
    {
        let cause = ReadinessBlocker {
            id: canonical_json_hash(&json!([snapshot.scope, stage.stage, "unavailable"])),
            stage: stage.stage.clone(),
            code: "observation.unavailable".into(),
            target: snapshot.scope.change.clone(),
            message: "Observation is unavailable".into(),
            prerequisites: Vec::new(),
        };
        if blockers.len() < MAX_READINESS_BLOCKERS {
            actions.push(action_for(&cause, &snapshot.scope, &policy));
            blockers.push(cause);
        }
    }
    for stage in stages.iter().filter(|s| {
        s.state == ReadinessState::Ready && matches!(s.stage.as_str(), "sync" | "archive")
    }) {
        let kind = format!("{}_preview", stage.stage);
        actions.push(ReadinessAction {
            id: canonical_json_hash(&json!([snapshot.scope, kind])),
            kind,
            causes: Vec::new(),
            target: snapshot.scope.change.clone(),
            prerequisites: vec![stage.decision_hash.clone()],
            authority: "advisory".into(),
            expected_observation: "fresh existing mutation preflight".into(),
            argv: string_array(&stage.decision["preview_argv"]).unwrap_or_else(|| {
                vec![
                    "cairn".into(),
                    stage.stage.clone(),
                    snapshot.scope.change.clone(),
                ]
            }),
        });
    }
    let actions = merge_actions(actions);
    let valid = diagnostics.is_empty();
    let classification = if stages
        .iter()
        .any(|s| s.state == ReadinessState::Unavailable)
    {
        "unavailable"
    } else if stages.iter().any(|s| s.state == ReadinessState::Blocked) {
        "blocked"
    } else {
        "ready"
    };
    let mut report = ReadinessReport {
        schema: "cairn.readiness.v1".into(),
        scope: snapshot.scope.clone(),
        valid,
        classification: classification.into(),
        stages,
        blockers,
        actions,
        diagnostics,
        readiness_policy_hash: canonical_json_hash(&policy),
        input_hash: canonical_json_hash(&input),
        receipt_hash: String::new(),
        mutation_authorized: false,
        non_claims: vec![
            "Readiness does not authorize mutation or execute checks".into(),
            "Readiness does not prove evidence truth, code correctness, or release eligibility"
                .into(),
        ],
    };
    report.receipt_hash = canonical_json_hash(&json!(report));
    report
}

fn safe_target(target: &str) -> bool {
    !target.is_empty()
        && target.len() <= MAX_READINESS_TARGET_BYTES
        && !target.starts_with('/')
        && !target.contains('\\')
        && !target.chars().any(char::is_control)
        && !target.split('/').any(|part| part == "..")
}

fn merge_actions(actions: Vec<ReadinessAction>) -> Vec<ReadinessAction> {
    let mut merged = std::collections::BTreeMap::<String, ReadinessAction>::new();
    for action in actions {
        if let Some(existing) = merged.get_mut(&action.id) {
            existing.causes.extend(action.causes);
            existing.causes.sort();
            existing.causes.dedup();
        } else {
            merged.insert(action.id.clone(), action);
        }
    }
    merged.into_values().collect()
}

// r[impl cairn.unified_readiness.actions]
fn action_for(
    blocker: &ReadinessBlocker,
    scope: &ReadinessScope,
    policy: &Value,
) -> ReadinessAction {
    let kind = policy["rules"]
        .as_array()
        .and_then(|rules| rules.iter().find(|r| r["code"] == blocker.code))
        .and_then(|rule| rule["kind"].as_str())
        .unwrap_or("unresolved");
    let expected = match kind {
        "artifact_repair" => "current structurally valid artifact",
        "evidence_refresh" => "current passing evidence bound to the selected input",
        "dependency_wait" => "upstream dependency accepted by its existing validator",
        _ => "reviewed remediation rule for this exact diagnostic",
    };
    let mut prerequisites = vec![
        scope.source_hash.clone(),
        scope.policy_hash.clone(),
        scope.evidence_hash.clone(),
    ];
    if kind == "dependency_wait" {
        prerequisites.push(format!("predecessor-change:{}", blocker.target));
    }
    prerequisites.extend(blocker.prerequisites.iter().cloned());
    prerequisites.sort();
    prerequisites.dedup();
    ReadinessAction {
        id: canonical_json_hash(&json!([
            scope,
            kind,
            blocker.code,
            blocker.target,
            // Unknown diagnostics have no reviewed equivalence rule.
            if kind == "unresolved" {
                Some(&blocker.message)
            } else {
                None
            }
        ])),
        kind: kind.into(),
        causes: vec![blocker.id.clone()],
        target: blocker.target.clone(),
        prerequisites,
        authority: "advisory".into(),
        expected_observation: expected.into(),
        argv: Vec::new(),
    }
}

fn string_array(value: &Value) -> Option<Vec<String>> {
    value.as_array().map(|values| {
        values
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect()
    })
}

#[cfg(test)]
mod tests;
