use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::evidence::canonical_json_hash;

pub const OXIDE_LIFECYCLE_PATTERN_SCHEMA_V1: &str = "cairn.oxide-lifecycle-patterns.v1";
pub const OXIDE_LIFECYCLE_PATTERN_BOUNDARY: &str = "Oxide repositories are reference-only inputs for lifecycle, test, bundle, observability, archive, operator, and diagnostic patterns; Cairn does not import their domain-specific correctness claims, and passing pattern validation does not prove lifecycle correctness or release eligibility";

const BLAKE3_HEX_LENGTH: usize = 64;
const MAX_REFERENCE_ROWS: usize = 64;
const MAX_SAGA_STEPS: usize = 256;
const MAX_DOCUMENT_PLANS: usize = 256;
const MAX_GOLDEN_ROWS: usize = 256;
const MAX_BUNDLE_MEMBERS: usize = 512;
const REQUIRED_REFERENCE_REPOS: &[&str] = &[
    "oxidecomputer/steno",
    "oxidecomputer/rfd-api",
    "oxidecomputer/rfd-site",
    "oxidecomputer/expectorate",
    "oxidecomputer/bundle-cat",
    "oxidecomputer/support-bundle-viewer",
];
const REQUIRED_BUNDLE_NON_CLAIM: &str =
    "bundle inspection does not prove evidence truth or release eligibility";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OxideReferenceRow {
    pub repository: String,
    pub adaptation: String,
    pub license_posture: String,
    pub non_adoption_boundary: String,
    pub study_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleSagaStep {
    pub id: String,
    pub input_hash: String,
    pub output_hash: String,
    pub status: String,
    pub compensation_eligible: bool,
    pub compensation_action: Option<String>,
    pub recovery_decision: String,
    pub operator_decision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleDocumentPlan {
    pub source_path: String,
    pub source_revision: String,
    pub observed_revision: String,
    pub renderer_profile: String,
    pub output_hash: String,
    pub state_transition: String,
    pub search_index_hash: String,
    pub webhook_input_hash: Option<String>,
    pub scan_profile: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoldenSnapshotPlan {
    pub id: String,
    pub expected_hash: String,
    pub current_hash: String,
    pub refresh_requested: bool,
    pub approval: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceBundleMember {
    pub path: String,
    pub role: String,
    pub declared_hash: String,
    pub observed_hash: String,
    pub validation_status: String,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OxideLifecyclePatternBundle {
    pub schema_version: String,
    pub references: Vec<OxideReferenceRow>,
    pub saga_steps: Vec<LifecycleSagaStep>,
    pub document_plans: Vec<LifecycleDocumentPlan>,
    pub golden_snapshots: Vec<GoldenSnapshotPlan>,
    pub bundle_members: Vec<EvidenceBundleMember>,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OxideLifecyclePatternIssue {
    pub code: String,
    pub field_path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OxideLifecyclePatternReport {
    pub valid: bool,
    pub input_hash: String,
    pub reference_count: usize,
    pub saga_step_count: usize,
    pub document_plan_count: usize,
    pub golden_snapshot_count: usize,
    pub bundle_member_count: usize,
    pub issues: Vec<OxideLifecyclePatternIssue>,
    pub boundary: String,
}

#[must_use]
pub fn validate_oxide_lifecycle_patterns_value(value: &Value) -> OxideLifecyclePatternReport {
    let input_hash = canonical_json_hash(value);
    let Ok(bundle) = serde_json::from_value::<OxideLifecyclePatternBundle>(value.clone()) else {
        return OxideLifecyclePatternReport {
            valid: false,
            input_hash,
            reference_count: 0,
            saga_step_count: 0,
            document_plan_count: 0,
            golden_snapshot_count: 0,
            bundle_member_count: 0,
            issues: vec![issue(
                "oxide_patterns.malformed",
                "bundle",
                "input must match the Oxide lifecycle pattern bundle schema",
            )],
            boundary: OXIDE_LIFECYCLE_PATTERN_BOUNDARY.to_string(),
        };
    };
    validate_oxide_lifecycle_patterns(&bundle, input_hash)
}

#[must_use]
pub fn positive_oxide_lifecycle_patterns_fixture() -> Value {
    serde_json::json!({
        "schema_version": OXIDE_LIFECYCLE_PATTERN_SCHEMA_V1,
        "references": REQUIRED_REFERENCE_REPOS.iter().map(|repository| serde_json::json!({
            "repository": repository,
            "adaptation": "bounded lifecycle processing pattern",
            "license_posture": "reference-only; review upstream license before copying code",
            "non_adoption_boundary": "pattern mining only; no domain correctness claims are imported",
            "study_only": false
        })).collect::<Vec<_>>(),
        "saga_steps": [{
            "id": "sync",
            "input_hash": fixture_hash('a'),
            "output_hash": fixture_hash('b'),
            "status": "passed",
            "compensation_eligible": true,
            "compensation_action": "restore accepted spec pre-state",
            "recovery_decision": "not-required",
            "operator_decision": null
        }],
        "document_plans": [{
            "source_path": ".cairn/specs/demo/spec.md",
            "source_revision": fixture_hash('c'),
            "observed_revision": fixture_hash('c'),
            "renderer_profile": "markdown-index-v1",
            "output_hash": fixture_hash('d'),
            "state_transition": "reviewed-to-rendered",
            "search_index_hash": fixture_hash('e'),
            "webhook_input_hash": null,
            "scan_profile": "local-periodic"
        }],
        "golden_snapshots": [{
            "id": "cli-status",
            "expected_hash": fixture_hash('a'),
            "current_hash": fixture_hash('b'),
            "refresh_requested": true,
            "approval": "reviewed"
        }],
        "bundle_members": [{
            "path": "evidence/receipt.json",
            "role": "release-readiness",
            "declared_hash": fixture_hash('f'),
            "observed_hash": fixture_hash('f'),
            "validation_status": "passed",
            "non_claims": [REQUIRED_BUNDLE_NON_CLAIM]
        }],
        "non_claims": [OXIDE_LIFECYCLE_PATTERN_BOUNDARY]
    })
}

fn validate_oxide_lifecycle_patterns(
    bundle: &OxideLifecyclePatternBundle,
    input_hash: String,
) -> OxideLifecyclePatternReport {
    let mut issues = Vec::new();
    if bundle.schema_version != OXIDE_LIFECYCLE_PATTERN_SCHEMA_V1 {
        push_issue(
            &mut issues,
            "oxide_patterns.schema_unsupported",
            "schema_version",
            "schema version is unsupported",
        );
    }
    validate_references(&bundle.references, &mut issues);
    validate_saga_steps(&bundle.saga_steps, &mut issues);
    validate_document_plans(&bundle.document_plans, &mut issues);
    validate_golden_snapshots(&bundle.golden_snapshots, &mut issues);
    validate_bundle_members(&bundle.bundle_members, &mut issues);
    validate_top_level_boundary(&bundle.non_claims, &mut issues);
    issues.sort_by(|left, right| {
        left.field_path
            .cmp(&right.field_path)
            .then(left.code.cmp(&right.code))
            .then(left.message.cmp(&right.message))
    });
    issues.dedup();
    let report = OxideLifecyclePatternReport {
        valid: issues.is_empty(),
        input_hash,
        reference_count: bundle.references.len(),
        saga_step_count: bundle.saga_steps.len(),
        document_plan_count: bundle.document_plans.len(),
        golden_snapshot_count: bundle.golden_snapshots.len(),
        bundle_member_count: bundle.bundle_members.len(),
        issues,
        boundary: OXIDE_LIFECYCLE_PATTERN_BOUNDARY.to_string(),
    };
    debug_assert_eq!(report.valid, report.issues.is_empty());
    report
}

fn validate_references(
    references: &[OxideReferenceRow],
    issues: &mut Vec<OxideLifecyclePatternIssue>,
) {
    // r[impl cairn.lifecycle_orchestration.oxide_patterns.reference_inventory]
    // r[impl cairn.lifecycle_orchestration.oxide_patterns.reference_inventory.non_adoption]
    // r[impl cairn.lifecycle_orchestration.oxide_patterns.reference_inventory.study_only]
    validate_bound(references.len(), MAX_REFERENCE_ROWS, "references", issues);
    let mut repositories = BTreeSet::new();
    for (index, reference) in references.iter().enumerate() {
        let field_path = format!("references[{index}]");
        validate_text(
            &reference.repository,
            &format!("{field_path}.repository"),
            issues,
        );
        if !repositories.insert(reference.repository.as_str()) {
            push_issue(
                issues,
                "oxide_patterns.reference_duplicate",
                &format!("{field_path}.repository"),
                "reference repositories must be unique",
            );
        }
        validate_text(
            &reference.adaptation,
            &format!("{field_path}.adaptation"),
            issues,
        );
        validate_text(
            &reference.license_posture,
            &format!("{field_path}.license_posture"),
            issues,
        );
        validate_text(
            &reference.non_adoption_boundary,
            &format!("{field_path}.non_adoption_boundary"),
            issues,
        );
        let boundary = reference.non_adoption_boundary.to_ascii_lowercase();
        if !boundary.contains("no domain correctness claims") {
            push_issue(
                issues,
                "oxide_patterns.reference_overclaim",
                &format!("{field_path}.non_adoption_boundary"),
                "reference boundary must reject imported domain correctness claims",
            );
        }
        if reference.study_only
            && !(boundary.contains("pattern mining") || boundary.contains("study-only"))
        {
            push_issue(
                issues,
                "oxide_patterns.study_boundary_missing",
                &format!("{field_path}.non_adoption_boundary"),
                "study-only references must be limited to pattern mining",
            );
        }
    }
    for required in REQUIRED_REFERENCE_REPOS {
        if !repositories.contains(required) {
            push_issue(
                issues,
                "oxide_patterns.reference_missing",
                "references",
                &format!("reference inventory must include {required}"),
            );
        }
    }
}

fn validate_saga_steps(steps: &[LifecycleSagaStep], issues: &mut Vec<OxideLifecyclePatternIssue>) {
    // r[impl cairn.lifecycle_orchestration.oxide_patterns.saga_logs]
    // r[impl cairn.lifecycle_orchestration.oxide_patterns.saga_logs.recoverable]
    // r[impl cairn.lifecycle_orchestration.oxide_patterns.saga_logs.missing_compensation]
    validate_bound(steps.len(), MAX_SAGA_STEPS, "saga_steps", issues);
    let mut ids = BTreeSet::new();
    for (index, step) in steps.iter().enumerate() {
        let field_path = format!("saga_steps[{index}]");
        validate_text(&step.id, &format!("{field_path}.id"), issues);
        if !ids.insert(step.id.as_str()) {
            push_issue(
                issues,
                "oxide_patterns.saga_step_duplicate",
                &format!("{field_path}.id"),
                "saga step ids must be unique",
            );
        }
        validate_hash(
            &step.input_hash,
            &format!("{field_path}.input_hash"),
            issues,
        );
        validate_hash(
            &step.output_hash,
            &format!("{field_path}.output_hash"),
            issues,
        );
        if !matches!(
            step.status.as_str(),
            "pending" | "passed" | "failed" | "compensated"
        ) {
            push_issue(
                issues,
                "oxide_patterns.saga_status_unsupported",
                &format!("{field_path}.status"),
                "saga status is unsupported",
            );
        }
        if step.compensation_eligible
            && step
                .compensation_action
                .as_deref()
                .is_none_or(|action| action.trim().is_empty())
        {
            push_issue(
                issues,
                "oxide_patterns.compensation_missing",
                &format!("{field_path}.compensation_action"),
                "compensation-eligible steps must name an action",
            );
        }
        if step.status == "failed"
            && !step.compensation_eligible
            && step.recovery_decision != "operator-intervention-required"
        {
            push_issue(
                issues,
                "oxide_patterns.operator_intervention_missing",
                &format!("{field_path}.recovery_decision"),
                "failed non-compensable steps require operator intervention",
            );
        }
        validate_text(
            &step.recovery_decision,
            &format!("{field_path}.recovery_decision"),
            issues,
        );
    }
}

fn validate_document_plans(
    plans: &[LifecycleDocumentPlan],
    issues: &mut Vec<OxideLifecyclePatternIssue>,
) {
    // r[impl cairn.lifecycle_orchestration.oxide_patterns.rfd_processing]
    // r[impl cairn.lifecycle_orchestration.oxide_patterns.rfd_processing.revision_asset]
    // r[impl cairn.lifecycle_orchestration.oxide_patterns.rfd_processing.stale_revision]
    validate_bound(plans.len(), MAX_DOCUMENT_PLANS, "document_plans", issues);
    for (index, plan) in plans.iter().enumerate() {
        let field_path = format!("document_plans[{index}]");
        validate_safe_path(
            &plan.source_path,
            &format!("{field_path}.source_path"),
            issues,
        );
        validate_hash(
            &plan.source_revision,
            &format!("{field_path}.source_revision"),
            issues,
        );
        validate_hash(
            &plan.observed_revision,
            &format!("{field_path}.observed_revision"),
            issues,
        );
        if plan.source_revision != plan.observed_revision {
            push_issue(
                issues,
                "oxide_patterns.source_revision_stale",
                &format!("{field_path}.observed_revision"),
                "observed source revision must match the planned source revision",
            );
        }
        validate_text(
            &plan.renderer_profile,
            &format!("{field_path}.renderer_profile"),
            issues,
        );
        validate_hash(
            &plan.output_hash,
            &format!("{field_path}.output_hash"),
            issues,
        );
        validate_text(
            &plan.state_transition,
            &format!("{field_path}.state_transition"),
            issues,
        );
        validate_hash(
            &plan.search_index_hash,
            &format!("{field_path}.search_index_hash"),
            issues,
        );
        if let Some(hash) = &plan.webhook_input_hash {
            validate_hash(hash, &format!("{field_path}.webhook_input_hash"), issues);
        }
        if let Some(profile) = &plan.scan_profile {
            validate_text(profile, &format!("{field_path}.scan_profile"), issues);
        }
    }
}

fn validate_golden_snapshots(
    snapshots: &[GoldenSnapshotPlan],
    issues: &mut Vec<OxideLifecyclePatternIssue>,
) {
    // r[impl cairn.lifecycle_orchestration.oxide_patterns.golden_snapshots]
    // r[impl cairn.lifecycle_orchestration.oxide_patterns.golden_snapshots.mismatch]
    // r[impl cairn.lifecycle_orchestration.oxide_patterns.golden_snapshots.unreviewed_refresh]
    validate_bound(snapshots.len(), MAX_GOLDEN_ROWS, "golden_snapshots", issues);
    for (index, snapshot) in snapshots.iter().enumerate() {
        let field_path = format!("golden_snapshots[{index}]");
        validate_text(&snapshot.id, &format!("{field_path}.id"), issues);
        validate_hash(
            &snapshot.expected_hash,
            &format!("{field_path}.expected_hash"),
            issues,
        );
        validate_hash(
            &snapshot.current_hash,
            &format!("{field_path}.current_hash"),
            issues,
        );
        let differs = snapshot.expected_hash != snapshot.current_hash;
        let approved = snapshot.approval.as_deref() == Some("reviewed");
        if differs && !snapshot.refresh_requested {
            push_issue(
                issues,
                "oxide_patterns.golden_mismatch",
                &format!("{field_path}.current_hash"),
                "golden mismatch requires an explicit refresh request",
            );
        }
        if snapshot.refresh_requested && !approved {
            push_issue(
                issues,
                "oxide_patterns.golden_refresh_unapproved",
                &format!("{field_path}.approval"),
                "golden refresh requires reviewed approval",
            );
        }
    }
}

fn validate_bundle_members(
    members: &[EvidenceBundleMember],
    issues: &mut Vec<OxideLifecyclePatternIssue>,
) {
    // r[impl cairn.lifecycle_orchestration.oxide_patterns.bundle_inspector]
    // r[impl cairn.lifecycle_orchestration.oxide_patterns.bundle_inspector.list]
    // r[impl cairn.lifecycle_orchestration.oxide_patterns.bundle_inspector.corrupt]
    validate_bound(members.len(), MAX_BUNDLE_MEMBERS, "bundle_members", issues);
    for (index, member) in members.iter().enumerate() {
        let field_path = format!("bundle_members[{index}]");
        validate_safe_path(&member.path, &format!("{field_path}.path"), issues);
        validate_text(&member.role, &format!("{field_path}.role"), issues);
        validate_hash(
            &member.declared_hash,
            &format!("{field_path}.declared_hash"),
            issues,
        );
        validate_hash(
            &member.observed_hash,
            &format!("{field_path}.observed_hash"),
            issues,
        );
        if member.declared_hash != member.observed_hash {
            push_issue(
                issues,
                "oxide_patterns.bundle_hash_mismatch",
                &format!("{field_path}.observed_hash"),
                "bundle member hash must match its declaration",
            );
        }
        if member.validation_status != "passed" {
            push_issue(
                issues,
                "oxide_patterns.bundle_status_not_passed",
                &format!("{field_path}.validation_status"),
                "bundle member validation status must be passed",
            );
        }
        if !member
            .non_claims
            .iter()
            .any(|boundary| boundary == REQUIRED_BUNDLE_NON_CLAIM)
        {
            push_issue(
                issues,
                "oxide_patterns.bundle_non_claim_missing",
                &format!("{field_path}.non_claims"),
                "bundle member must preserve the required non-claim boundary",
            );
        }
    }
}

fn validate_top_level_boundary(
    non_claims: &[String],
    issues: &mut Vec<OxideLifecyclePatternIssue>,
) {
    let joined = non_claims.join("; ");
    if !joined.contains("does not import their domain-specific correctness claims")
        || !joined.contains("does not prove lifecycle correctness or release eligibility")
    {
        push_issue(
            issues,
            "oxide_patterns.boundary_missing",
            "non_claims",
            "top-level non-claims must preserve reference-only and readiness boundaries",
        );
    }
    let normalized = joined.to_ascii_lowercase();
    if normalized.contains("proves domain correctness")
        || normalized.contains("proves release eligibility")
    {
        push_issue(
            issues,
            "oxide_patterns.overclaim",
            "non_claims",
            "top-level non-claims contain an unsupported correctness claim",
        );
    }
}

fn validate_hash(value: &str, field_path: &str, issues: &mut Vec<OxideLifecyclePatternIssue>) {
    let valid = value.len() == BLAKE3_HEX_LENGTH
        && value
            .chars()
            .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase());
    if !valid {
        push_issue(
            issues,
            "oxide_patterns.hash_malformed",
            field_path,
            "hash must be lowercase BLAKE3 hexadecimal text",
        );
    }
}

fn validate_text(value: &str, field_path: &str, issues: &mut Vec<OxideLifecyclePatternIssue>) {
    if value.trim().is_empty() {
        push_issue(
            issues,
            "oxide_patterns.field_empty",
            field_path,
            "field must be non-empty",
        );
    }
}

fn validate_safe_path(path: &str, field_path: &str, issues: &mut Vec<OxideLifecyclePatternIssue>) {
    let safe = !path.is_empty()
        && !path.starts_with('/')
        && !path.starts_with('~')
        && !path.contains('\\')
        && !path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..");
    if !safe {
        push_issue(
            issues,
            "oxide_patterns.path_unsafe",
            field_path,
            "path must be repository-relative and non-traversing",
        );
    }
}

fn validate_bound(
    actual: usize,
    maximum: usize,
    field_path: &str,
    issues: &mut Vec<OxideLifecyclePatternIssue>,
) {
    if actual > maximum {
        push_issue(
            issues,
            "oxide_patterns.bound_exceeded",
            field_path,
            "collection exceeds its reviewed bound",
        );
    }
}

fn fixture_hash(character: char) -> String {
    std::iter::repeat_n(character, BLAKE3_HEX_LENGTH).collect()
}

fn issue(code: &str, field_path: &str, message: &str) -> OxideLifecyclePatternIssue {
    OxideLifecyclePatternIssue {
        code: code.to_string(),
        field_path: field_path.to_string(),
        message: message.to_string(),
    }
}

fn push_issue(
    issues: &mut Vec<OxideLifecyclePatternIssue>,
    code: &str,
    field_path: &str,
    message: &str,
) {
    issues.push(issue(code, field_path, message));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(report: &OxideLifecyclePatternReport) -> Vec<&str> {
        report
            .issues
            .iter()
            .map(|issue| issue.code.as_str())
            .collect()
    }

    #[test]
    fn accepted_pattern_fixture_is_deterministic() {
        // r[verify cairn.lifecycle_orchestration.oxide_patterns.validation]
        // r[verify cairn.lifecycle_orchestration.oxide_patterns.validation.positive]
        let value = positive_oxide_lifecycle_patterns_fixture();
        let first = validate_oxide_lifecycle_patterns_value(&value);
        let second = validate_oxide_lifecycle_patterns_value(&value);
        assert!(first.valid, "{:?}", first.issues);
        assert_eq!(first, second);
        assert!(!first.input_hash.is_empty());
    }

    #[test]
    fn rejected_pattern_fixture_matrix_fails_closed() {
        // r[verify cairn.lifecycle_orchestration.oxide_patterns.validation.negative]
        let cases = [
            (
                "missing-compensation",
                "oxide_patterns.compensation_missing",
            ),
            (
                "missing-operator-decision",
                "oxide_patterns.operator_intervention_missing",
            ),
            ("stale-revision", "oxide_patterns.source_revision_stale"),
            ("golden-mismatch", "oxide_patterns.golden_mismatch"),
            (
                "unreviewed-refresh",
                "oxide_patterns.golden_refresh_unapproved",
            ),
            ("corrupt-bundle", "oxide_patterns.bundle_hash_mismatch"),
            ("reference-overclaim", "oxide_patterns.reference_overclaim"),
            ("overclaim", "oxide_patterns.overclaim"),
        ];
        for (case, expected) in cases {
            let mut value = positive_oxide_lifecycle_patterns_fixture();
            match case {
                "missing-compensation" => {
                    value["saga_steps"][0]["compensation_action"] = Value::Null
                }
                "missing-operator-decision" => {
                    value["saga_steps"][0]["status"] = Value::String("failed".to_string());
                    value["saga_steps"][0]["compensation_eligible"] = Value::Bool(false);
                    value["saga_steps"][0]["recovery_decision"] = Value::String("none".to_string());
                }
                "stale-revision" => {
                    value["document_plans"][0]["observed_revision"] =
                        Value::String(fixture_hash('f'))
                }
                "golden-mismatch" => {
                    value["golden_snapshots"][0]["refresh_requested"] = Value::Bool(false)
                }
                "unreviewed-refresh" => value["golden_snapshots"][0]["approval"] = Value::Null,
                "corrupt-bundle" => {
                    value["bundle_members"][0]["observed_hash"] = Value::String(fixture_hash('a'))
                }
                "reference-overclaim" => {
                    value["references"][0]["non_adoption_boundary"] =
                        Value::String("proves domain correctness".to_string())
                }
                "overclaim" => {
                    value["non_claims"] = serde_json::json!(["proves release eligibility"])
                }
                _ => unreachable!(),
            }
            let report = validate_oxide_lifecycle_patterns_value(&value);
            assert!(!report.valid, "case {case}");
            assert!(
                codes(&report).contains(&expected),
                "case {case}: {:?}",
                report.issues
            );
        }
    }
}
