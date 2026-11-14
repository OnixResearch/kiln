use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use super::model::*;

const PROFILE_SOURCE_COMPATIBILITY: &str = "compatibility_fallback";
const PROFILE_STAGE_PROPOSAL: &str = "proposal";
const PROFILE_STAGE_DESIGN: &str = "design";
const PROFILE_STAGE_TASKS: &str = "tasks";
const PROFILE_PLACEHOLDER: &str = "CAIRN_PLACEHOLDER";
const PROFILE_MAX_RATIONALE_BYTES: usize = 4096;
const ACCEPTANCE_MARKER_PREFIX: &str = "a[";
const ACCEPTANCE_MARKER_SUFFIX: char = ']';
const TASK_TODO_PREFIX: &str = "- [ ]";
const TASK_DONE_PREFIX_LOWER: &str = "- [x]";
const TASK_DONE_PREFIX_UPPER: &str = "- [X]";
const TASK_IN_PROGRESS_PREFIX: &str = "- [~]";
const REVIEW_DECISION_APPROVED: &str = "approved";
const SYNTHETIC_REQUIREMENT_PREFIX: &str = "r[";

// r[impl cairn.lifecycle_workflow_profiles.selection]
// r[impl cairn.lifecycle_workflow_profiles.functional_boundary]
// r[impl cairn.lifecycle_workflow_profiles.functional_boundary.pure]
pub fn workflow_change_profile_metadata_from_value(
    value: &Value,
) -> Result<WorkflowChangeProfileMetadata, WorkflowAdmissionError> {
    let Some(object) = value.as_object() else {
        return Err(profile_error(
            WORKFLOW_ISSUE_PROFILE_SELECTION,
            "metadata.json",
            "change metadata must contain a JSON object",
        ));
    };
    let workflow_profile = object
        .get("workflow_profile")
        .cloned()
        .map(serde_json::from_value::<WorkflowProfileRef>)
        .transpose()
        .map_err(|error| {
            profile_error(
                WORKFLOW_ISSUE_PROFILE_SELECTION,
                "metadata.json.workflow_profile",
                format!("workflow profile metadata is invalid: {error}"),
            )
        })?;
    let spec_effect = optional_string_field(object.get("spec_effect"), "spec_effect")?;
    let spec_effect_rationale =
        optional_string_field(object.get("spec_effect_rationale"), "spec_effect_rationale")?;
    Ok(WorkflowChangeProfileMetadata {
        workflow_profile,
        spec_effect,
        spec_effect_rationale,
    })
}

pub fn select_workflow_profile(
    registry: &WorkflowProfileRegistryPolicy,
    schemas: &[WorkflowSchemaPolicy],
    metadata: &WorkflowChangeProfileMetadata,
) -> Result<(WorkflowProfileSelection, WorkflowSchemaPolicy), WorkflowAdmissionError> {
    let (profile_id, requested_version, source, compatibility_fallback) =
        match &metadata.workflow_profile {
            Some(reference) => {
                if reference.source != WORKFLOW_PROFILE_SOURCE_CREATE {
                    return Err(profile_error(
                        WORKFLOW_ISSUE_PROFILE_SELECTION,
                        "metadata.json.workflow_profile.source",
                        "workflow profile source must be the immutable change-create selection",
                    ));
                }
                (
                    reference.id.as_str(),
                    Some(reference.version),
                    WORKFLOW_PROFILE_SOURCE_CREATE,
                    false,
                )
            }
            None => (
                registry.default_profile.as_str(),
                None,
                PROFILE_SOURCE_COMPATIBILITY,
                true,
            ),
        };
    if !registry
        .allowed_profiles
        .iter()
        .any(|allowed| allowed == profile_id)
    {
        return Err(profile_error(
            WORKFLOW_ISSUE_PROFILE_SELECTION,
            "metadata.json.workflow_profile.id",
            format!("workflow profile is not allowed by repository policy: {profile_id}"),
        ));
    }
    let schema = schemas
        .iter()
        .find(|schema| schema.id == profile_id)
        .cloned()
        .ok_or_else(|| {
            profile_error(
                WORKFLOW_ISSUE_PROFILE_SELECTION,
                "metadata.json.workflow_profile.id",
                format!("workflow profile is unknown: {profile_id}"),
            )
        })?;
    if requested_version.is_some_and(|version| version != schema.profile_version) {
        return Err(profile_error(
            WORKFLOW_ISSUE_PROFILE_SELECTION,
            "metadata.json.workflow_profile.version",
            format!(
                "workflow profile version does not match policy: requested {}, policy {}",
                requested_version.unwrap_or_default(),
                schema.profile_version
            ),
        ));
    }
    Ok((
        WorkflowProfileSelection {
            profile_id: schema.id.clone(),
            profile_version: schema.profile_version,
            source: source.to_string(),
            compatibility_fallback,
        },
        schema,
    ))
}

// r[impl cairn.lifecycle_workflow_profiles.no_spec]
// r[impl cairn.lifecycle_workflow_profiles.assurance]
pub fn evaluate_workflow_profile(
    profile: &WorkflowSchemaPolicy,
    evidence: &WorkflowProfileEvidence,
) -> WorkflowProfileEvaluation {
    let mut issues = Vec::new();
    let acceptance_ids = if profile.spec_effect == WORKFLOW_SPEC_EFFECT_NONE {
        evaluate_no_spec(profile, evidence, &mut issues)
    } else {
        evaluate_delta_profile(profile, evidence, &mut issues);
        Vec::new()
    };
    evaluate_reviews(profile, evidence, &mut issues);
    issues.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then(left.artifact_id.cmp(&right.artifact_id))
            .then(left.message.cmp(&right.message))
    });
    issues.dedup();
    let mut review_receipt_hashes = evidence
        .reviews
        .iter()
        .filter(|review| !review.receipt_hash.is_empty())
        .map(|review| review.receipt_hash.clone())
        .collect::<Vec<_>>();
    review_receipt_hashes.sort();
    review_receipt_hashes.dedup();
    WorkflowProfileEvaluation {
        profile_id: profile.id.clone(),
        profile_version: profile.profile_version,
        spec_effect: profile.spec_effect.clone(),
        acceptance_ids,
        review_receipt_hashes,
        issues,
        non_claims: profile.required_non_claims.clone(),
    }
}

fn evaluate_delta_profile(
    profile: &WorkflowSchemaPolicy,
    evidence: &WorkflowProfileEvidence,
    issues: &mut Vec<WorkflowProfileIssue>,
) {
    if evidence.spec_effect.as_deref() == Some(WORKFLOW_SPEC_EFFECT_NONE) {
        issues.push(issue(
            WORKFLOW_ISSUE_PROFILE_EFFECT,
            None,
            PROFILE_STAGE_PROPOSAL,
            "spec-mutating workflow profile cannot declare spec_effect=none",
        ));
    }
    if evidence.delta_spec_paths.is_empty() {
        issues.push(issue(
            WORKFLOW_ISSUE_PROFILE_ARTIFACT,
            profile_artifact_id(profile, WORKFLOW_ADMISSION_DELTA_SPECS),
            PROFILE_STAGE_DESIGN,
            "spec-mutating workflow profile requires at least one delta spec",
        ));
    }
}

fn evaluate_no_spec(
    profile: &WorkflowSchemaPolicy,
    evidence: &WorkflowProfileEvidence,
    issues: &mut Vec<WorkflowProfileIssue>,
) -> Vec<String> {
    if evidence.spec_effect.as_deref() != Some(WORKFLOW_SPEC_EFFECT_NONE) {
        issues.push(issue(
            WORKFLOW_ISSUE_PROFILE_EFFECT,
            None,
            PROFILE_STAGE_PROPOSAL,
            "no-spec workflow profile requires spec_effect=none",
        ));
    }
    let rationale = evidence
        .spec_effect_rationale
        .as_deref()
        .map(str::trim)
        .unwrap_or_default();
    if rationale.is_empty()
        || rationale.contains(PROFILE_PLACEHOLDER)
        || rationale.len() > PROFILE_MAX_RATIONALE_BYTES
    {
        issues.push(issue(
            WORKFLOW_ISSUE_PROFILE_EFFECT,
            None,
            PROFILE_STAGE_PROPOSAL,
            "no-spec workflow profile requires a bounded substantive spec-effect rationale",
        ));
    }
    if !evidence.delta_spec_paths.is_empty() {
        issues.push(issue(
            WORKFLOW_ISSUE_PROFILE_EFFECT,
            profile_artifact_id(profile, WORKFLOW_ADMISSION_ACCEPTANCE),
            PROFILE_STAGE_DESIGN,
            "no-spec workflow profile forbids delta-spec files",
        ));
    }
    let (acceptance_ids, acceptance_issues) =
        parse_acceptance_ids(evidence.acceptance_markdown.as_deref().unwrap_or_default());
    issues.extend(acceptance_issues);
    issues.extend(validate_acceptance_task_links(
        evidence.tasks_markdown.as_deref().unwrap_or_default(),
        &acceptance_ids,
    ));
    acceptance_ids
}

fn evaluate_reviews(
    profile: &WorkflowSchemaPolicy,
    evidence: &WorkflowProfileEvidence,
    issues: &mut Vec<WorkflowProfileIssue>,
) {
    let hashes_by_artifact = artifact_hashes_by_artifact(&evidence.artifact_hashes);
    for obligation in &profile.review_obligations {
        let Some(review) = evidence
            .reviews
            .iter()
            .find(|review| review.obligation_id == obligation.id)
        else {
            issues.push(issue(
                WORKFLOW_ISSUE_PROFILE_REVIEW,
                Some(obligation.artifact_id.clone()),
                PROFILE_STAGE_TASKS,
                format!("required workflow review is missing: {}", obligation.id),
            ));
            continue;
        };
        if !review.valid
            || review.review_kind != obligation.review_kind
            || review.decision != REVIEW_DECISION_APPROVED
        {
            issues.push(issue(
                WORKFLOW_ISSUE_PROFILE_REVIEW,
                Some(obligation.artifact_id.clone()),
                PROFILE_STAGE_TASKS,
                format!(
                    "workflow review is invalid or not approved: {}",
                    obligation.id
                ),
            ));
            continue;
        }
        let expected = obligation
            .required_artifacts
            .iter()
            .flat_map(|artifact_id| {
                hashes_by_artifact
                    .get(artifact_id.as_str())
                    .into_iter()
                    .flat_map(|values| values.iter())
            })
            .map(|value| (value.path.clone(), value.blake3.clone()))
            .collect::<BTreeMap<_, _>>();
        let observed = review
            .reviewed_inputs
            .iter()
            .map(|input| (input.path.clone(), input.hash.clone()))
            .collect::<BTreeMap<_, _>>();
        if expected.is_empty() || expected != observed {
            issues.push(issue(
                WORKFLOW_ISSUE_PROFILE_REVIEW,
                Some(obligation.artifact_id.clone()),
                PROFILE_STAGE_TASKS,
                format!(
                    "workflow review does not bind the exact current required artifacts: {}",
                    obligation.id
                ),
            ));
        }
    }
}

fn artifact_hashes_by_artifact(
    hashes: &[WorkflowProfileArtifactHash],
) -> BTreeMap<&str, Vec<&WorkflowProfileArtifactHash>> {
    let mut grouped = BTreeMap::<&str, Vec<&WorkflowProfileArtifactHash>>::new();
    for hash in hashes {
        grouped
            .entry(hash.artifact_id.as_str())
            .or_default()
            .push(hash);
    }
    for values in grouped.values_mut() {
        values.sort_by(|left, right| left.path.cmp(&right.path));
    }
    grouped
}

pub fn parse_acceptance_ids(markdown: &str) -> (Vec<String>, Vec<WorkflowProfileIssue>) {
    let mut ids = Vec::new();
    let mut issues = Vec::new();
    let mut remaining = markdown;
    while let Some(start) = remaining.find(ACCEPTANCE_MARKER_PREFIX) {
        let after = &remaining[start + ACCEPTANCE_MARKER_PREFIX.len()..];
        let Some(end) = after.find(ACCEPTANCE_MARKER_SUFFIX) else {
            issues.push(issue(
                WORKFLOW_ISSUE_PROFILE_ACCEPTANCE,
                Some("acceptance".to_string()),
                PROFILE_STAGE_DESIGN,
                "acceptance marker has no closing bracket",
            ));
            break;
        };
        let candidate = after[..end].trim();
        if !valid_local_acceptance_id(candidate) {
            issues.push(issue(
                WORKFLOW_ISSUE_PROFILE_ACCEPTANCE,
                Some("acceptance".to_string()),
                PROFILE_STAGE_DESIGN,
                format!("acceptance marker is malformed: a[{candidate}]"),
            ));
        } else {
            ids.push(candidate.to_string());
        }
        remaining = &after[end + ACCEPTANCE_MARKER_SUFFIX.len_utf8()..];
    }
    ids.sort();
    let duplicate = ids.windows(2).any(|pair| pair[0] == pair[1]);
    ids.dedup();
    if duplicate {
        issues.push(issue(
            WORKFLOW_ISSUE_PROFILE_ACCEPTANCE,
            Some("acceptance".to_string()),
            PROFILE_STAGE_DESIGN,
            "acceptance IDs must be unique",
        ));
    }
    if ids.is_empty() {
        issues.push(issue(
            WORKFLOW_ISSUE_PROFILE_ACCEPTANCE,
            Some("acceptance".to_string()),
            PROFILE_STAGE_DESIGN,
            "no-spec workflow profile requires at least one valid a[...] acceptance ID",
        ));
    }
    (ids, issues)
}

fn validate_acceptance_task_links(
    tasks_markdown: &str,
    acceptance_ids: &[String],
) -> Vec<WorkflowProfileIssue> {
    let known = acceptance_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut issues = Vec::new();
    for (line_index, line) in tasks_markdown.lines().enumerate() {
        let trimmed = line.trim_start();
        if !is_checkbox_task(trimmed) {
            continue;
        }
        let (line_ids, line_issues) = parse_acceptance_ids_without_required(trimmed);
        if !line_issues.is_empty() || !line_ids.iter().any(|id| known.contains(id.as_str())) {
            issues.push(issue(
                WORKFLOW_ISSUE_PROFILE_TASK_LINK,
                Some("tasks".to_string()),
                PROFILE_STAGE_TASKS,
                format!(
                    "tasks.md:{}: no-spec checkbox task requires a known a[...] acceptance reference",
                    line_index + 1
                ),
            ));
        }
    }
    issues
}

fn parse_acceptance_ids_without_required(
    markdown: &str,
) -> (Vec<String>, Vec<WorkflowProfileIssue>) {
    let (ids, mut issues) = parse_acceptance_ids(markdown);
    issues.retain(|issue| {
        !issue
            .message
            .starts_with("no-spec workflow profile requires")
    });
    (ids, issues)
}

fn valid_local_acceptance_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.first().is_some_and(u8::is_ascii_lowercase)
        && bytes
            .last()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

fn is_checkbox_task(line: &str) -> bool {
    line.starts_with(TASK_TODO_PREFIX)
        || line.starts_with(TASK_DONE_PREFIX_LOWER)
        || line.starts_with(TASK_DONE_PREFIX_UPPER)
        || line.starts_with(TASK_IN_PROGRESS_PREFIX)
}

pub fn synthetic_acceptance_spec(acceptance_ids: &[String]) -> String {
    let mut markdown = String::from(
        "# Change-local acceptance adapter\n\n## ADDED Requirements\n\n### Requirement: Change-local acceptance criteria\n\n",
    );
    for acceptance_id in acceptance_ids {
        markdown.push_str(&format!(
            "r[{acceptance_id}] Cairn MUST preserve this change-local acceptance linkage.\n\n#### Scenario: {acceptance_id}\n\n- GIVEN a reviewed local acceptance criterion\n- WHEN profile-aware validation runs\n- THEN the linked task MUST remain accountable to that criterion.\n\n"
        ));
    }
    markdown
}

pub fn tasks_with_requirement_links(tasks_markdown: &str) -> String {
    tasks_markdown.replace(ACCEPTANCE_MARKER_PREFIX, SYNTHETIC_REQUIREMENT_PREFIX)
}

pub fn profile_issue_applies_to_stage(issue: &WorkflowProfileIssue, stage: &str) -> bool {
    stage_rank(&issue.stage) <= stage_rank(stage)
}

fn stage_rank(stage: &str) -> u8 {
    const PROPOSAL_RANK: u8 = 0;
    const DESIGN_RANK: u8 = 1;
    const TASKS_RANK: u8 = 2;
    match stage {
        PROFILE_STAGE_PROPOSAL => PROPOSAL_RANK,
        PROFILE_STAGE_DESIGN => DESIGN_RANK,
        _ => TASKS_RANK,
    }
}

fn profile_artifact_id(profile: &WorkflowSchemaPolicy, admission: &str) -> Option<String> {
    profile
        .artifacts
        .iter()
        .find(|artifact| artifact.structural_admission == admission)
        .map(|artifact| artifact.id.clone())
}

fn optional_string_field(
    value: Option<&Value>,
    field: &'static str,
) -> Result<Option<String>, WorkflowAdmissionError> {
    match value {
        None => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(profile_error(
            WORKFLOW_ISSUE_PROFILE_SELECTION,
            format!("metadata.json.{field}"),
            format!("workflow profile metadata field must be a string: {field}"),
        )),
    }
}

fn issue(
    code: &str,
    artifact_id: Option<String>,
    stage: &str,
    message: impl Into<String>,
) -> WorkflowProfileIssue {
    WorkflowProfileIssue {
        code: code.to_string(),
        artifact_id,
        stage: stage.to_string(),
        message: message.into(),
    }
}

fn profile_error(
    code: &'static str,
    path: impl Into<String>,
    message: impl Into<String>,
) -> WorkflowAdmissionError {
    WorkflowAdmissionError::new(code, path, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acceptance_ids_and_task_links_accept_valid_local_criteria() {
        // r[verify cairn.lifecycle_workflow_profiles.no_spec.valid]
        let (ids, issues) = parse_acceptance_ids("a[demo.first] First.\na[demo.second] Second.\n");
        assert!(issues.is_empty());
        assert_eq!(ids, vec!["demo.first", "demo.second"]);
        assert!(validate_acceptance_task_links(
            "- [x] [serial] Implement the first criterion. a[demo.first]\n",
            &ids,
        )
        .is_empty());
    }

    #[test]
    fn malformed_acceptance_and_unlinked_tasks_fail_closed() {
        // r[verify cairn.lifecycle_workflow_profiles.no_spec.conflict]
        let (ids, issues) = parse_acceptance_ids("a[Bad/id] Invalid.\n");
        assert!(ids.is_empty());
        assert!(!issues.is_empty());
        let task_issues = validate_acceptance_task_links(
            "- [ ] [serial] Missing local acceptance linkage.\n",
            &["demo.first".to_string()],
        );
        assert_eq!(task_issues.len(), 1);
        assert_eq!(task_issues[0].code, WORKFLOW_ISSUE_PROFILE_TASK_LINK);
    }
}
