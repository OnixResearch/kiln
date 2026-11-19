const PROFILE_STAGE_DESIGN: &str = "design";
const REVIEW_PLACEHOLDER: &str = "CAIRN_PLACEHOLDER";
const COMPLETION_AUTHORITY_REPOSITORY: &str = "repository";
const OUTCOME_MACHINE_MAX_TEXT_BYTES: usize = 4096;
const OUTCOME_MACHINE_MAX_LIST_ITEMS: usize = 64;
const OUTCOME_MACHINE_MAX_EVIDENCE_ITEMS: usize = 64;
const OUTCOME_MACHINE_BLAKE3_HEX_LENGTH: usize = 64;

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct OutcomeMachineReview {
    schema: String,
    route: OutcomeMachineRoute,
    maintenance_owner: String,
    tracked_inputs: Vec<String>,
    deterministic_checks: Vec<String>,
    ai_assistance: AiAssistance,
    fresh_state_reproduction: FreshStateDisposition,
    completion: CompletionDisposition,
    activity_counts_are_completion_evidence: bool,
    non_claims: Vec<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(
    tag = "kind",
    content = "details",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum OutcomeMachineRoute {
    Delivery(DeliveryRoute),
    Foundation(FoundationRoute),
    BoundedException(BoundedExceptionRoute),
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DeliveryRoute {
    immediate_outcome: String,
    current_consumer: String,
    durable_capability_contribution: String,
    repeatability_plan: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FoundationRoute {
    current_consumer: String,
    target_outcome: String,
    adoption_path: String,
    repeatability_plan: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BoundedExceptionRoute {
    immediate_outcome: String,
    exception_reason: String,
    recurrence_decision: String,
    revisit_trigger: String,
    capability_non_claim: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AiAssistance {
    #[serde(rename = "used")]
    _used: bool,
    hidden_session_state_required: bool,
    completion_authority: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FreshStateDisposition {
    status: FreshStateStatus,
    evidence: Vec<EvidenceReference>,
    rationale: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum FreshStateStatus {
    Planned,
    Observed,
    NotApplicable,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CompletionDisposition {
    status: CompletionStatus,
    evidence: Vec<EvidenceReference>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum CompletionStatus {
    Planned,
    Observed,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceReference {
    #[serde(rename = "kind")]
    _kind: EvidenceKind,
    path: String,
    blake3: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum EvidenceKind {
    Test,
    Proof,
    Receipt,
    Replay,
    Observation,
    Review,
}

pub fn outcome_machine_evidence_paths(value: Option<&serde_json::Value>) -> Vec<String> {
    let Some(value) = value else {
        return Vec::new();
    };
    let Ok(review) = serde_json::from_value::<OutcomeMachineReview>(value.clone()) else {
        return Vec::new();
    };
    let mut paths = review
        .fresh_state_reproduction
        .evidence
        .iter()
        .chain(review.completion.evidence.iter())
        .map(|evidence| evidence.path.clone())
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

pub fn evaluate_outcome_machine_review(
    profile: &super::model::WorkflowSchemaPolicy,
    value: Option<&serde_json::Value>,
    observed_evidence: &[super::model::WorkflowOutcomeMachineEvidenceObservation],
) -> Vec<super::model::WorkflowProfileIssue> {
    // r[impl cairn.outcome_machine_profile.routes]
    // r[impl cairn.outcome_machine_profile.completion]
    // r[impl cairn.outcome_machine_profile.fresh_state]
    // r[impl cairn.outcome_machine_profile.boundary]
    let Some(artifact_id) = profile
        .artifacts
        .iter()
        .find(|artifact| {
            artifact.structural_admission == super::model::WORKFLOW_ADMISSION_OUTCOME_MACHINE
        })
        .map(|artifact| artifact.id.clone())
    else {
        return Vec::new();
    };
    let Some(value) = value else {
        return vec![design_issue(
            super::model::WORKFLOW_ISSUE_OUTCOME_MACHINE_SHAPE,
            &artifact_id,
            "outcome-machine review is missing or invalid JSON",
        )];
    };
    let Ok(review) = serde_json::from_value::<OutcomeMachineReview>(value.clone()) else {
        return vec![design_issue(
            super::model::WORKFLOW_ISSUE_OUTCOME_MACHINE_SHAPE,
            &artifact_id,
            "outcome-machine review does not match the supported schema",
        )];
    };

    let mut issues = Vec::new();
    if review.schema != super::model::OUTCOME_MACHINE_REVIEW_SCHEMA_V1 {
        issues.push(design_issue(
            super::model::WORKFLOW_ISSUE_OUTCOME_MACHINE_SHAPE,
            &artifact_id,
            "outcome-machine review schema is unsupported",
        ));
    }
    validate_route(&review.route, &artifact_id, &mut issues);
    validate_text(
        "maintenance_owner",
        &review.maintenance_owner,
        &artifact_id,
        &mut issues,
    );
    validate_text_list(
        "tracked_inputs",
        &review.tracked_inputs,
        &artifact_id,
        &mut issues,
    );
    validate_text_list(
        "deterministic_checks",
        &review.deterministic_checks,
        &artifact_id,
        &mut issues,
    );
    validate_text_list("non_claims", &review.non_claims, &artifact_id, &mut issues);
    validate_ai_assistance(&review.ai_assistance, &artifact_id, &mut issues);
    if review.activity_counts_are_completion_evidence {
        issues.push(design_issue(
            super::model::WORKFLOW_ISSUE_OUTCOME_MACHINE_AUTHORITY,
            &artifact_id,
            "activity counts cannot serve as completion evidence",
        ));
    }
    validate_fresh_state(
        &review.fresh_state_reproduction,
        &artifact_id,
        observed_evidence,
        &mut issues,
    );
    validate_completion(
        &review.completion,
        &artifact_id,
        observed_evidence,
        &mut issues,
    );
    issues.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then(left.stage.cmp(&right.stage))
            .then(left.message.cmp(&right.message))
    });
    issues.dedup();
    issues
}

fn validate_route(
    route: &OutcomeMachineRoute,
    artifact_id: &str,
    issues: &mut Vec<super::model::WorkflowProfileIssue>,
) {
    match route {
        OutcomeMachineRoute::Delivery(delivery) => {
            validate_text(
                "route.details.immediate_outcome",
                &delivery.immediate_outcome,
                artifact_id,
                issues,
            );
            validate_text(
                "route.details.current_consumer",
                &delivery.current_consumer,
                artifact_id,
                issues,
            );
            validate_text(
                "route.details.durable_capability_contribution",
                &delivery.durable_capability_contribution,
                artifact_id,
                issues,
            );
            validate_text(
                "route.details.repeatability_plan",
                &delivery.repeatability_plan,
                artifact_id,
                issues,
            );
        }
        OutcomeMachineRoute::Foundation(foundation) => {
            validate_text(
                "route.details.current_consumer",
                &foundation.current_consumer,
                artifact_id,
                issues,
            );
            validate_text(
                "route.details.target_outcome",
                &foundation.target_outcome,
                artifact_id,
                issues,
            );
            validate_text(
                "route.details.adoption_path",
                &foundation.adoption_path,
                artifact_id,
                issues,
            );
            validate_text(
                "route.details.repeatability_plan",
                &foundation.repeatability_plan,
                artifact_id,
                issues,
            );
        }
        OutcomeMachineRoute::BoundedException(exception) => {
            validate_text(
                "route.details.immediate_outcome",
                &exception.immediate_outcome,
                artifact_id,
                issues,
            );
            validate_text(
                "route.details.exception_reason",
                &exception.exception_reason,
                artifact_id,
                issues,
            );
            validate_text(
                "route.details.recurrence_decision",
                &exception.recurrence_decision,
                artifact_id,
                issues,
            );
            validate_text(
                "route.details.revisit_trigger",
                &exception.revisit_trigger,
                artifact_id,
                issues,
            );
            validate_text(
                "route.details.capability_non_claim",
                &exception.capability_non_claim,
                artifact_id,
                issues,
            );
        }
    }
}

fn validate_ai_assistance(
    ai: &AiAssistance,
    artifact_id: &str,
    issues: &mut Vec<super::model::WorkflowProfileIssue>,
) {
    if ai.hidden_session_state_required {
        issues.push(design_issue(
            super::model::WORKFLOW_ISSUE_OUTCOME_MACHINE_AUTHORITY,
            artifact_id,
            "hidden session state cannot be a required input",
        ));
    }
    if ai.completion_authority != COMPLETION_AUTHORITY_REPOSITORY {
        issues.push(design_issue(
            super::model::WORKFLOW_ISSUE_OUTCOME_MACHINE_AUTHORITY,
            artifact_id,
            "completion authority must remain with the repository",
        ));
    }
}

fn validate_fresh_state(
    fresh_state: &FreshStateDisposition,
    artifact_id: &str,
    observed_evidence: &[super::model::WorkflowOutcomeMachineEvidenceObservation],
    issues: &mut Vec<super::model::WorkflowProfileIssue>,
) {
    match fresh_state.status {
        FreshStateStatus::Planned => {
            validate_text(
                "fresh_state_reproduction.rationale",
                &fresh_state.rationale,
                artifact_id,
                issues,
            );
            if !fresh_state.evidence.is_empty() {
                issues.push(design_issue(
                    super::model::WORKFLOW_ISSUE_OUTCOME_MACHINE_FRESH_STATE,
                    artifact_id,
                    "planned fresh-state reproduction cannot contain observed evidence",
                ));
            }
            issues.push(completion_issue(
                super::model::WORKFLOW_ISSUE_OUTCOME_MACHINE_FRESH_STATE,
                "fresh-state reproduction remains planned",
            ));
        }
        FreshStateStatus::Observed => {
            validate_optional_text(
                "fresh_state_reproduction.rationale",
                &fresh_state.rationale,
                artifact_id,
                issues,
            );
            validate_completion_evidence(
                "fresh_state_reproduction.evidence",
                &fresh_state.evidence,
                super::model::WORKFLOW_ISSUE_OUTCOME_MACHINE_FRESH_STATE,
                observed_evidence,
                issues,
            );
        }
        FreshStateStatus::NotApplicable => {
            validate_text(
                "fresh_state_reproduction.rationale",
                &fresh_state.rationale,
                artifact_id,
                issues,
            );
            if !fresh_state.evidence.is_empty() {
                issues.push(design_issue(
                    super::model::WORKFLOW_ISSUE_OUTCOME_MACHINE_FRESH_STATE,
                    artifact_id,
                    "not-applicable fresh-state reproduction cannot contain evidence",
                ));
            }
        }
    }
}

fn validate_completion(
    completion: &CompletionDisposition,
    artifact_id: &str,
    observed_evidence: &[super::model::WorkflowOutcomeMachineEvidenceObservation],
    issues: &mut Vec<super::model::WorkflowProfileIssue>,
) {
    match completion.status {
        CompletionStatus::Planned => {
            if !completion.evidence.is_empty() {
                issues.push(design_issue(
                    super::model::WORKFLOW_ISSUE_OUTCOME_MACHINE_COMPLETION,
                    artifact_id,
                    "planned completion cannot contain observed evidence",
                ));
            }
            issues.push(completion_issue(
                super::model::WORKFLOW_ISSUE_OUTCOME_MACHINE_COMPLETION,
                "observed completion evidence is missing",
            ));
        }
        CompletionStatus::Observed => validate_completion_evidence(
            "completion.evidence",
            &completion.evidence,
            super::model::WORKFLOW_ISSUE_OUTCOME_MACHINE_COMPLETION,
            observed_evidence,
            issues,
        ),
    }
}

fn validate_completion_evidence(
    field: &str,
    evidence: &[EvidenceReference],
    issue_code: &str,
    observed_evidence: &[super::model::WorkflowOutcomeMachineEvidenceObservation],
    issues: &mut Vec<super::model::WorkflowProfileIssue>,
) {
    if evidence.is_empty() || evidence.len() > OUTCOME_MACHINE_MAX_EVIDENCE_ITEMS {
        issues.push(completion_issue(
            issue_code,
            format!("{field} must contain bounded observed evidence"),
        ));
        return;
    }
    let mut unique_paths = std::collections::BTreeSet::new();
    for item in evidence {
        let observed = observed_evidence
            .iter()
            .find(|observation| observation.path == item.path)
            .and_then(|observation| observation.blake3.as_deref());
        if !evidence_path_is_valid(&item.path)
            || !blake3_is_valid(&item.blake3)
            || !unique_paths.insert(item.path.as_str())
            || observed != Some(item.blake3.as_str())
        {
            issues.push(completion_issue(
                issue_code,
                format!(
                    "{field} contains an invalid, duplicated, missing, or stale evidence binding"
                ),
            ));
        }
    }
}

fn evidence_path_is_valid(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= OUTCOME_MACHINE_MAX_TEXT_BYTES
        && !value.starts_with('/')
        && !value.contains('\\')
        && !value.contains(':')
        && value
            .split('/')
            .all(|component| !component.is_empty() && component != "." && component != "..")
}

fn blake3_is_valid(value: &str) -> bool {
    value.len() == OUTCOME_MACHINE_BLAKE3_HEX_LENGTH
        && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn validate_text_list(
    field: &str,
    values: &[String],
    artifact_id: &str,
    issues: &mut Vec<super::model::WorkflowProfileIssue>,
) {
    if values.is_empty() || values.len() > OUTCOME_MACHINE_MAX_LIST_ITEMS {
        issues.push(design_issue(
            super::model::WORKFLOW_ISSUE_OUTCOME_MACHINE_ROUTE,
            artifact_id,
            format!("{field} must contain bounded values"),
        ));
        return;
    }
    for value in values {
        if !text_is_valid(value) {
            issues.push(design_issue(
                super::model::WORKFLOW_ISSUE_OUTCOME_MACHINE_ROUTE,
                artifact_id,
                format!("{field} contains an invalid value"),
            ));
        }
    }
}

fn validate_text(
    field: &str,
    value: &str,
    artifact_id: &str,
    issues: &mut Vec<super::model::WorkflowProfileIssue>,
) {
    if !text_is_valid(value) {
        issues.push(design_issue(
            super::model::WORKFLOW_ISSUE_OUTCOME_MACHINE_ROUTE,
            artifact_id,
            format!("{field} must be substantive and bounded"),
        ));
    }
}

fn validate_optional_text(
    field: &str,
    value: &str,
    artifact_id: &str,
    issues: &mut Vec<super::model::WorkflowProfileIssue>,
) {
    if !value.is_empty() && !text_is_valid(value) {
        issues.push(design_issue(
            super::model::WORKFLOW_ISSUE_OUTCOME_MACHINE_ROUTE,
            artifact_id,
            format!("{field} must be bounded when present"),
        ));
    }
}

fn text_is_valid(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && trimmed.len() <= OUTCOME_MACHINE_MAX_TEXT_BYTES
        && !trimmed.contains(REVIEW_PLACEHOLDER)
}

fn design_issue(
    code: &str,
    artifact_id: &str,
    message: impl Into<String>,
) -> super::model::WorkflowProfileIssue {
    super::model::WorkflowProfileIssue {
        code: code.to_string(),
        artifact_id: Some(artifact_id.to_string()),
        stage: PROFILE_STAGE_DESIGN.to_string(),
        message: message.into(),
    }
}

fn completion_issue(code: &str, message: impl Into<String>) -> super::model::WorkflowProfileIssue {
    super::model::WorkflowProfileIssue {
        code: code.to_string(),
        artifact_id: None,
        stage: super::model::WORKFLOW_PROFILE_STAGE_COMPLETION.to_string(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    const TEST_BOUND: usize = 8;
    const TEST_BYTE_BOUND: usize = 4096;
    const EXPECTED_PLANNED_ISSUES: usize = 2;

    fn profile() -> super::super::model::WorkflowSchemaPolicy {
        super::super::model::WorkflowSchemaPolicy {
            schema_version: crate::verified::artifact_workflow::WORKFLOW_SCHEMA_VERSION_V1
                .to_string(),
            id: "outcome-machine".to_string(),
            profile_version: 1,
            description: "outcome-machine test".to_string(),
            spec_effect: crate::verified::artifact_workflow::WORKFLOW_SPEC_EFFECT_DELTA.to_string(),
            task_link_class: crate::verified::artifact_workflow::WORKFLOW_TASK_LINK_REQUIREMENT
                .to_string(),
            states: crate::verified::artifact_workflow::WORKFLOW_SUPPORTED_STATES
                .map(ToOwned::to_owned)
                .to_vec(),
            bounds: crate::verified::artifact_workflow::WorkflowBounds {
                max_artifacts: TEST_BOUND,
                max_dependencies: TEST_BOUND,
                max_files_per_artifact: TEST_BOUND,
                max_path_bytes: TEST_BYTE_BOUND,
                max_artifact_bytes: TEST_BYTE_BOUND,
                max_template_bytes: TEST_BYTE_BOUND,
                max_constraints: TEST_BOUND,
                max_non_claims: TEST_BOUND,
                max_issues: TEST_BOUND,
                max_directory_depth: TEST_BOUND,
                max_directory_entries: TEST_BOUND,
            },
            artifacts: vec![crate::verified::artifact_workflow::WorkflowArtifactPolicy {
                id: "outcome-machine".to_string(),
                generates: "outcome-machine.json".to_string(),
                output_path: "outcome-machine.json".to_string(),
                path_kind: crate::verified::artifact_workflow::WORKFLOW_PATH_KIND_FILE.to_string(),
                structural_admission: super::super::model::WORKFLOW_ADMISSION_OUTCOME_MACHINE
                    .to_string(),
                requires: Vec::new(),
                stage: PROFILE_STAGE_DESIGN.to_string(),
                instruction_template: "Write the outcome-machine record.".to_string(),
                constraints: Vec::new(),
                apply_prerequisites: Vec::new(),
                skipped: false,
                skip_satisfies_dependencies: false,
            }],
            review_obligations: Vec::new(),
            receipt_fields: vec![
                "workflow_profile".to_string(),
                "workflow_profile_version".to_string(),
                "spec_effect".to_string(),
                "non_claims".to_string(),
            ],
            required_non_claims: vec!["structural admission does not prove value".to_string()],
        }
    }

    fn common(
        route: serde_json::Value,
        fresh_status: &str,
        completion_status: &str,
    ) -> serde_json::Value {
        let fresh_evidence = if fresh_status == "observed" {
            vec![serde_json::json!({
                "kind": "replay",
                "path": "evidence/fresh-state.json",
                "blake3": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            })]
        } else {
            Vec::new()
        };
        let completion_evidence = if completion_status == "observed" {
            vec![serde_json::json!({
                "kind": "test",
                "path": "evidence/completion.json",
                "blake3": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
            })]
        } else {
            Vec::new()
        };
        serde_json::json!({
            "schema": super::super::model::OUTCOME_MACHINE_REVIEW_SCHEMA_V1,
            "route": route,
            "maintenance_owner": "repository maintainer",
            "tracked_inputs": ["source", "policy"],
            "deterministic_checks": ["cargo test -p demo"],
            "ai_assistance": {
                "used": true,
                "hidden_session_state_required": false,
                "completion_authority": "repository"
            },
            "fresh_state_reproduction": {
                "status": fresh_status,
                "evidence": fresh_evidence,
                "rationale": if fresh_status == "planned" { "Run from a clean checkout." } else { "" }
            },
            "completion": {
                "status": completion_status,
                "evidence": completion_evidence
            },
            "activity_counts_are_completion_evidence": false,
            "non_claims": ["This record does not prove product value."]
        })
    }

    fn observed_evidence(
        value: &serde_json::Value,
    ) -> Vec<super::super::model::WorkflowOutcomeMachineEvidenceObservation> {
        ["fresh_state_reproduction", "completion"]
            .iter()
            .flat_map(|section| value[*section]["evidence"].as_array().into_iter().flatten())
            .map(
                |evidence| super::super::model::WorkflowOutcomeMachineEvidenceObservation {
                    path: evidence["path"]
                        .as_str()
                        .expect("evidence path")
                        .to_string(),
                    blake3: Some(
                        evidence["blake3"]
                            .as_str()
                            .expect("evidence BLAKE3")
                            .to_string(),
                    ),
                },
            )
            .collect()
    }

    #[test]
    fn valid_routes_admit_with_observed_completion() {
        // r[verify cairn.outcome_machine_profile.routes]
        // r[verify cairn.outcome_machine_profile.completion]
        // r[verify cairn.outcome_machine_profile.fresh_state]
        let routes = [
            serde_json::json!({
                "kind": "delivery",
                "details": {
                    "immediate_outcome": "The operator receives a bounded result.",
                    "current_consumer": "workspace operator",
                    "durable_capability_contribution": "A reusable lifecycle profile.",
                    "repeatability_plan": "Run the same checks from tracked inputs."
                }
            }),
            serde_json::json!({
                "kind": "foundation",
                "details": {
                    "current_consumer": "workspace operator",
                    "target_outcome": "Repeatable lifecycle admission.",
                    "adoption_path": "Select the profile in a repository change.",
                    "repeatability_plan": "Run the profile fixtures."
                }
            }),
            serde_json::json!({
                "kind": "bounded_exception",
                "details": {
                    "immediate_outcome": "Restore the blocked service.",
                    "exception_reason": "The incident deadline prevents mechanism work.",
                    "recurrence_decision": "Create a follow-up change if the incident repeats.",
                    "revisit_trigger": "A second occurrence.",
                    "capability_non_claim": "This repair does not create repeatable capability."
                }
            }),
        ];

        for route in routes {
            let value = common(route, "observed", "observed");
            let evidence = observed_evidence(&value);
            assert!(
                evaluate_outcome_machine_review(&profile(), Some(&value), &evidence).is_empty()
            );
        }
    }

    #[test]
    fn planned_record_emits_only_completion_blockers() {
        // r[verify cairn.outcome_machine_profile.completion]
        let value = common(
            serde_json::json!({
                "kind": "delivery",
                "details": {
                    "immediate_outcome": "Deliver the requested result.",
                    "current_consumer": "current operator",
                    "durable_capability_contribution": "Retain a reusable profile.",
                    "repeatability_plan": "Replay from tracked inputs."
                }
            }),
            "planned",
            "planned",
        );
        let issues = evaluate_outcome_machine_review(&profile(), Some(&value), &[]);

        assert_eq!(issues.len(), EXPECTED_PLANNED_ISSUES);
        assert!(issues
            .iter()
            .all(|issue| issue.stage == super::super::model::WORKFLOW_PROFILE_STAGE_COMPLETION));
        for issue in &issues {
            assert!(
                !crate::verified::artifact_workflow::profile_issue_applies_to_stage(
                    issue, "proposal"
                )
            );
            assert!(
                !crate::verified::artifact_workflow::profile_issue_applies_to_stage(
                    issue, "design"
                )
            );
            assert!(
                !crate::verified::artifact_workflow::profile_issue_applies_to_stage(issue, "tasks")
            );
        }
    }

    #[test]
    fn not_applicable_fresh_state_requires_a_rationale() {
        // r[verify cairn.outcome_machine_profile.fresh_state]
        let mut value = common(
            serde_json::json!({
                "kind": "bounded_exception",
                "details": {
                    "immediate_outcome": "Restore the blocked service.",
                    "exception_reason": "The incident deadline prevents mechanism work.",
                    "recurrence_decision": "Create a follow-up after another occurrence.",
                    "revisit_trigger": "A second occurrence.",
                    "capability_non_claim": "This repair does not create repeatable capability."
                }
            }),
            "not_applicable",
            "observed",
        );
        value["fresh_state_reproduction"]["rationale"] =
            serde_json::json!("The incident repair has no replayable execution path.");
        let evidence = observed_evidence(&value);
        assert!(evaluate_outcome_machine_review(&profile(), Some(&value), &evidence).is_empty());

        value["fresh_state_reproduction"]["rationale"] = serde_json::json!("");
        let issues = evaluate_outcome_machine_review(&profile(), Some(&value), &evidence);
        assert!(issues
            .iter()
            .any(|issue| issue.code == super::super::model::WORKFLOW_ISSUE_OUTCOME_MACHINE_ROUTE));
    }

    #[test]
    fn invalid_foundation_and_hidden_ai_fail_design() {
        // r[verify cairn.outcome_machine_profile.routes]
        // r[verify cairn.outcome_machine_profile.boundary]
        let mut value = common(
            serde_json::json!({
                "kind": "foundation",
                "details": {
                    "current_consumer": "",
                    "target_outcome": "CAIRN_PLACEHOLDER",
                    "adoption_path": "Adopt through one repository.",
                    "repeatability_plan": "Run focused checks."
                }
            }),
            "observed",
            "observed",
        );
        value["ai_assistance"]["hidden_session_state_required"] = serde_json::json!(true);
        value["ai_assistance"]["completion_authority"] = serde_json::json!("model");
        value["activity_counts_are_completion_evidence"] = serde_json::json!(true);
        let evidence = observed_evidence(&value);
        let issues = evaluate_outcome_machine_review(&profile(), Some(&value), &evidence);

        assert!(issues.iter().any(|issue| {
            issue.code == super::super::model::WORKFLOW_ISSUE_OUTCOME_MACHINE_ROUTE
                && issue.stage == PROFILE_STAGE_DESIGN
        }));
        assert!(issues.iter().any(|issue| {
            issue.code == super::super::model::WORKFLOW_ISSUE_OUTCOME_MACHINE_AUTHORITY
                && issue.stage == PROFILE_STAGE_DESIGN
        }));
    }

    #[test]
    fn malformed_record_and_empty_observed_evidence_fail_closed() {
        // r[verify cairn.outcome_machine_profile.verification]
        let malformed =
            serde_json::json!({"schema": super::super::model::OUTCOME_MACHINE_REVIEW_SCHEMA_V1});
        let malformed_issues = evaluate_outcome_machine_review(&profile(), Some(&malformed), &[]);
        assert_eq!(malformed_issues.len(), 1);
        assert_eq!(
            malformed_issues[0].code,
            super::super::model::WORKFLOW_ISSUE_OUTCOME_MACHINE_SHAPE
        );

        let mut empty = common(
            serde_json::json!({
                "kind": "delivery",
                "details": {
                    "immediate_outcome": "Deliver the requested result.",
                    "current_consumer": "current operator",
                    "durable_capability_contribution": "Retain a reusable profile.",
                    "repeatability_plan": "Replay from tracked inputs."
                }
            }),
            "observed",
            "observed",
        );
        empty["fresh_state_reproduction"]["evidence"] = serde_json::json!([]);
        empty["completion"]["evidence"] = serde_json::json!([]);
        let empty_issues = evaluate_outcome_machine_review(&profile(), Some(&empty), &[]);
        assert!(empty_issues.iter().any(|issue| {
            issue.code == super::super::model::WORKFLOW_ISSUE_OUTCOME_MACHINE_FRESH_STATE
                && issue.stage == super::super::model::WORKFLOW_PROFILE_STAGE_COMPLETION
        }));
        assert!(empty_issues.iter().any(|issue| {
            issue.code == super::super::model::WORKFLOW_ISSUE_OUTCOME_MACHINE_COMPLETION
                && issue.stage == super::super::model::WORKFLOW_PROFILE_STAGE_COMPLETION
        }));

        let mut invalid_identity = common(
            serde_json::json!({
                "kind": "delivery",
                "details": {
                    "immediate_outcome": "Deliver the requested result.",
                    "current_consumer": "current operator",
                    "durable_capability_contribution": "Retain a reusable profile.",
                    "repeatability_plan": "Replay from tracked inputs."
                }
            }),
            "observed",
            "observed",
        );
        invalid_identity["completion"]["evidence"][0]["blake3"] = serde_json::json!("not-a-digest");
        let identity_evidence = observed_evidence(&invalid_identity);
        let identity_issues = evaluate_outcome_machine_review(
            &profile(),
            Some(&invalid_identity),
            &identity_evidence,
        );
        assert!(identity_issues
            .iter()
            .any(|issue| issue.code
                == super::super::model::WORKFLOW_ISSUE_OUTCOME_MACHINE_COMPLETION));

        let mut unsafe_path = common(
            serde_json::json!({
                "kind": "delivery",
                "details": {
                    "immediate_outcome": "Deliver the requested result.",
                    "current_consumer": "current operator",
                    "durable_capability_contribution": "Retain a reusable profile.",
                    "repeatability_plan": "Replay from tracked inputs."
                }
            }),
            "observed",
            "observed",
        );
        unsafe_path["completion"]["evidence"][0]["path"] = serde_json::json!("../outside.json");
        let unsafe_evidence = observed_evidence(&unsafe_path);
        let unsafe_issues =
            evaluate_outcome_machine_review(&profile(), Some(&unsafe_path), &unsafe_evidence);
        assert!(unsafe_issues
            .iter()
            .any(|issue| issue.code
                == super::super::model::WORKFLOW_ISSUE_OUTCOME_MACHINE_COMPLETION));

        let missing = common(
            serde_json::json!({
                "kind": "delivery",
                "details": {
                    "immediate_outcome": "Deliver the requested result.",
                    "current_consumer": "current operator",
                    "durable_capability_contribution": "Retain a reusable profile.",
                    "repeatability_plan": "Replay from tracked inputs."
                }
            }),
            "observed",
            "observed",
        );
        let missing_issues = evaluate_outcome_machine_review(&profile(), Some(&missing), &[]);
        assert!(missing_issues
            .iter()
            .any(|issue| issue.code
                == super::super::model::WORKFLOW_ISSUE_OUTCOME_MACHINE_COMPLETION));
        assert!(missing_issues
            .iter()
            .any(|issue| issue.code
                == super::super::model::WORKFLOW_ISSUE_OUTCOME_MACHINE_FRESH_STATE));
    }
}
