use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use crate::canonical_json_hash;

pub const CROSS_REPO_DEPENDENCY_SCHEMA_V1: &str = "cairn.cross-repo-dependency.v1";
pub const CROSS_REPO_RECEIPT_SCHEMA_V1: &str = "cairn.cross-repo-dependency-receipt.v1";
pub const CROSS_REPO_NON_CLAIM_LINKAGE: &str =
    "dependency satisfaction proves presented receipt linkage only";
pub const CROSS_REPO_NON_CLAIM_UPSTREAM: &str =
    "dependency satisfaction does not prove upstream correctness or remote trust";
pub const CROSS_REPO_NON_CLAIM_DOWNSTREAM: &str =
    "dependency satisfaction does not prove downstream integration correctness or release eligibility";

// r[impl cairn.radicle_issue_cross_repo.receipt_kind]
// r[impl cairn.radicle_issue_cross_repo.contract]
pub const RADICLE_ISSUE_RECEIPT_KIND: &str = "radicle-issue.v1";
pub const RADICLE_ISSUE_RECEIPT_SCHEMA_V1: &str = "cairn.radicle-issue-receipt.v1";
pub const RADICLE_ISSUE_NON_CLAIM_LINKAGE: &str =
    "issue receipt proves bounded local observation of one Radicle issue only";
pub const RADICLE_ISSUE_NON_CLAIM_AUTHORITY: &str =
    "issue receipt does not prove closure authority, peer convergence, remote trust, semantic truth, or release eligibility";

const BLAKE3_PREFIX: &str = "b3:";
const HEX_CHARACTERS_PER_BYTE: usize = 2;
const BLAKE3_HEX_LENGTH: usize = blake3::OUT_LEN * HEX_CHARACTERS_PER_BYTE;
const MAX_DIAGNOSTIC_BYTES: usize = 512;
const PASS_STATUS: &str = "pass";
const SUPPORTED_RECEIPT_KINDS: &[&str] = &[
    "proposal_gate",
    "design_gate",
    "tasks_gate",
    "validation_receipt",
    "sync_receipt",
    "archive_receipt",
    RADICLE_ISSUE_RECEIPT_KIND,
];
const NON_PASS_STATUSES: &[&str] = &[
    "failed",
    "blocked",
    "skipped",
    "deferred",
    "timed_out",
    "dry_run",
    "cancelled",
    "infrastructure_failed",
    "malformed",
    "unsupported",
    "unknown",
    "stale",
];

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CrossRepoDependency {
    pub id: String,
    pub repo: String,
    pub change: String,
    #[serde(rename = "requires")]
    pub receipt_kind: String,
    pub expected_status: String,
    #[serde(default)]
    pub expected_policy_hash: Option<String>,
    #[serde(default)]
    pub expected_subject_hash: Option<String>,
    #[serde(default)]
    pub expected_receipt_hash: Option<String>,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossRepoReceiptSummary {
    pub repo: String,
    pub change: String,
    pub receipt_kind: String,
    pub status: String,
    pub policy_hash: Option<String>,
    pub subject_hash: Option<String>,
    pub receipt_hash: String,
    pub artifact_ref: Option<String>,
    pub diagnostic_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossRepoDependencyDiagnostic {
    pub diagnostic_id: String,
    pub code: String,
    pub dependency_id: String,
    pub status: String,
    pub expected: Option<String>,
    pub actual: Option<String>,
    pub remediation: String,
    pub evidence_guidance: String,
    pub artifact_ref: Option<String>,
    pub severity: String,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossRepoDependencyRowResult {
    pub dependency_id: String,
    pub repo: String,
    pub change: String,
    pub receipt_kind: String,
    pub status: String,
    pub satisfied: bool,
    pub evidence_receipt_hash: Option<String>,
    pub diagnostics: Vec<CrossRepoDependencyDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossRepoDependencyReport {
    pub dependency_count: usize,
    pub satisfied_count: usize,
    pub blocked_count: usize,
    pub rows: Vec<CrossRepoDependencyRowResult>,
    pub input_hash: String,
    pub policy_hash: String,
    pub receipt_hash: String,
    pub valid: bool,
}

impl CrossRepoDependencyReport {
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "schema": CROSS_REPO_RECEIPT_SCHEMA_V1,
            "dependency_count": self.dependency_count,
            "satisfied_count": self.satisfied_count,
            "blocked_count": self.blocked_count,
            "rows": rows_json(&self.rows),
            "input_hash": self.input_hash,
            "policy_hash": self.policy_hash,
            "receipt_hash": self.receipt_hash,
            "valid": self.valid,
            "verdict": if self.valid { "PASS" } else { "FAIL" },
            "non_claims": default_non_claims(),
        })
    }
}

#[must_use]
pub fn cross_repo_dependency_contract() -> Value {
    json!({
        "schema": CROSS_REPO_DEPENDENCY_SCHEMA_V1,
        "supported_receipt_kinds": SUPPORTED_RECEIPT_KINDS,
        "passing_status": PASS_STATUS,
        "non_passing_statuses": NON_PASS_STATUSES,
        "required_non_claims": default_non_claims(),
    })
}

// r[impl cairn.radicle_issue_cross_repo.contract]
// r[impl cairn.radicle_issue_cross_repo.contract.identity]
// r[impl cairn.radicle_issue_cross_repo.contract.privacy]
// r[impl cairn.radicle_issue_cross_repo.adapter_bounded]
// r[impl cairn.radicle_issue_cross_repo.adapter_bounded.observe]
// r[impl cairn.radicle_issue_cross_repo.adapter_bounded.non_claims]
// r[impl cairn.radicle_issue_cross_repo.validation]
// r[impl cairn.radicle_issue_cross_repo.validation.match]
// r[impl cairn.radicle_issue_cross_repo.validation.mismatch]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadicleIssueObservation {
    pub repo: String,
    pub issue_id: String,
    pub object_id: String,
    pub status: String,
    pub subject_hash: String,
}

#[must_use]
pub fn build_radicle_issue_receipt(
    observation: &RadicleIssueObservation,
) -> CrossRepoReceiptSummary {
    CrossRepoReceiptSummary {
        repo: observation.repo.clone(),
        change: observation.issue_id.clone(),
        receipt_kind: RADICLE_ISSUE_RECEIPT_KIND.to_string(),
        status: PASS_STATUS.to_string(),
        policy_hash: None,
        subject_hash: Some(observation.subject_hash.clone()),
        receipt_hash: canonical_json_hash(&json!({
            "repo": observation.repo,
            "issue_id": observation.issue_id,
            "object_id": observation.object_id,
            "observed_status": observation.status,
            "subject_hash": observation.subject_hash,
        })),
        artifact_ref: Some(format!("radicle-issue:{}", observation.object_id)),
        diagnostic_summary: None,
    }
}

#[must_use]
pub fn radicle_issue_receipt_contract() -> Value {
    json!({
        "schema": RADICLE_ISSUE_RECEIPT_SCHEMA_V1,
        "receipt_kind": RADICLE_ISSUE_RECEIPT_KIND,
        "passing_status": PASS_STATUS,
        "non_claims": [
            RADICLE_ISSUE_NON_CLAIM_LINKAGE,
            RADICLE_ISSUE_NON_CLAIM_AUTHORITY,
        ],
    })
}

#[must_use]
// r[impl cairn.cross_repo_change_dependencies.validation]
// r[impl cairn.cross_repo_change_dependencies.validation.receipt]
// r[impl cairn.cross_repo_change_dependencies.status_feedback]
pub fn evaluate_cross_repo_dependencies(
    dependencies: &[CrossRepoDependency],
    evidence: &[CrossRepoReceiptSummary],
    policy_hash: &str,
) -> CrossRepoDependencyReport {
    let mut sorted_dependencies = dependencies.to_vec();
    sorted_dependencies.sort_by(|left, right| left.id.cmp(&right.id));
    let mut sorted_evidence = evidence.to_vec();
    sorted_evidence.sort_by(|left, right| {
        (
            &left.repo,
            &left.change,
            &left.receipt_kind,
            &left.receipt_hash,
        )
            .cmp(&(
                &right.repo,
                &right.change,
                &right.receipt_kind,
                &right.receipt_hash,
            ))
    });

    let mut rows = Vec::with_capacity(sorted_dependencies.len());
    let mut seen_ids = BTreeSet::new();
    for dependency in &sorted_dependencies {
        if !seen_ids.insert(dependency.id.clone()) {
            rows.push(blocked_row(
                dependency,
                "duplicate_dependency",
                diagnostic(
                    dependency,
                    "cross_repo_dependency.duplicate_id",
                    "duplicate_dependency",
                    Some(dependency.id.clone()),
                    Some(dependency.id.clone()),
                    None,
                ),
            ));
            continue;
        }
        rows.push(evaluate_dependency(dependency, &sorted_evidence));
    }
    let satisfied_count = rows.iter().filter(|row| row.satisfied).count();
    let blocked_count = rows.len().saturating_sub(satisfied_count);
    let input_material = json!({
        "schema": CROSS_REPO_DEPENDENCY_SCHEMA_V1,
        "dependencies": dependencies_json(&sorted_dependencies),
        "evidence": evidence_json(&sorted_evidence),
        "policy_hash": policy_hash,
    });
    let input_hash = canonical_json_hash(&input_material);
    let receipt_material = json!({
        "schema": CROSS_REPO_RECEIPT_SCHEMA_V1,
        "dependency_count": rows.len(),
        "satisfied_count": satisfied_count,
        "blocked_count": blocked_count,
        "rows": rows_json(&rows),
        "input_hash": input_hash,
        "policy_hash": policy_hash,
        "non_claims": default_non_claims(),
    });
    let receipt_hash = canonical_json_hash(&receipt_material);
    CrossRepoDependencyReport {
        dependency_count: rows.len(),
        satisfied_count,
        blocked_count,
        rows,
        input_hash,
        policy_hash: policy_hash.to_string(),
        receipt_hash,
        valid: blocked_count == 0,
    }
}

fn evaluate_dependency(
    dependency: &CrossRepoDependency,
    evidence: &[CrossRepoReceiptSummary],
) -> CrossRepoDependencyRowResult {
    if let Some(diagnostic) = validate_dependency_contract(dependency) {
        return blocked_row(dependency, "malformed", diagnostic);
    }
    let candidates = evidence
        .iter()
        .filter(|summary| summary.repo == dependency.repo && summary.change == dependency.change)
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return blocked_row(
            dependency,
            "missing",
            diagnostic(
                dependency,
                "cross_repo_dependency.missing",
                "missing",
                Some(format!(
                    "{}:{}:{}",
                    dependency.repo, dependency.change, dependency.receipt_kind
                )),
                None,
                None,
            ),
        );
    }
    let Some(summary) = candidates
        .iter()
        .copied()
        .find(|summary| summary.receipt_kind == dependency.receipt_kind)
    else {
        return blocked_row(
            dependency,
            "wrong_kind",
            diagnostic(
                dependency,
                "cross_repo_dependency.wrong_kind",
                "wrong_kind",
                Some(dependency.receipt_kind.clone()),
                candidates
                    .first()
                    .map(|summary| summary.receipt_kind.clone()),
                candidates
                    .first()
                    .and_then(|summary| summary.artifact_ref.clone()),
            ),
        );
    };
    if let Some(diagnostic) = validate_summary_safety(dependency, summary) {
        return blocked_row(dependency, "unsafe_diagnostic", diagnostic);
    }
    let status = normalize_status(&summary.status);
    if status != PASS_STATUS || dependency.expected_status != PASS_STATUS {
        let blocked_status = if NON_PASS_STATUSES.contains(&status.as_str()) {
            status
        } else {
            "unknown".to_string()
        };
        return blocked_row(
            dependency,
            &blocked_status,
            diagnostic(
                dependency,
                "cross_repo_dependency.non_pass",
                &blocked_status,
                Some(PASS_STATUS.to_string()),
                Some(summary.status.clone()),
                summary.artifact_ref.clone(),
            ),
        );
    }
    if let Some(row) = compare_optional_hash(
        dependency,
        summary,
        dependency.expected_policy_hash.as_deref(),
        summary.policy_hash.as_deref(),
        "stale_policy",
    ) {
        return row;
    }
    if let Some(row) = compare_optional_hash(
        dependency,
        summary,
        dependency.expected_subject_hash.as_deref(),
        summary.subject_hash.as_deref(),
        "stale_subject",
    ) {
        return row;
    }
    if dependency
        .expected_receipt_hash
        .as_deref()
        .is_some_and(|expected| expected != summary.receipt_hash)
    {
        return blocked_row(
            dependency,
            "hash_mismatch",
            diagnostic(
                dependency,
                "cross_repo_dependency.receipt_hash_mismatch",
                "hash_mismatch",
                dependency.expected_receipt_hash.clone(),
                Some(summary.receipt_hash.clone()),
                summary.artifact_ref.clone(),
            ),
        );
    }
    CrossRepoDependencyRowResult {
        dependency_id: dependency.id.clone(),
        repo: dependency.repo.clone(),
        change: dependency.change.clone(),
        receipt_kind: dependency.receipt_kind.clone(),
        status: "satisfied".to_string(),
        satisfied: true,
        evidence_receipt_hash: Some(summary.receipt_hash.clone()),
        diagnostics: Vec::new(),
    }
}

fn validate_dependency_contract(
    dependency: &CrossRepoDependency,
) -> Option<CrossRepoDependencyDiagnostic> {
    let required = [
        ("id", dependency.id.as_str()),
        ("repo", dependency.repo.as_str()),
        ("change", dependency.change.as_str()),
        ("receipt_kind", dependency.receipt_kind.as_str()),
    ];
    if let Some((field, _)) = required.iter().find(|(_, value)| value.trim().is_empty()) {
        return Some(diagnostic(
            dependency,
            "cross_repo_dependency.missing_field",
            "malformed",
            Some((*field).to_string()),
            None,
            None,
        ));
    }
    if !safe_identifier(&dependency.id)
        || !safe_identifier(&dependency.repo)
        || !safe_identifier(&dependency.change)
    {
        return Some(diagnostic(
            dependency,
            "cross_repo_dependency.unsafe_identity",
            "malformed",
            Some("portable lowercase identifier".to_string()),
            Some(format!(
                "{}:{}:{}",
                dependency.id, dependency.repo, dependency.change
            )),
            None,
        ));
    }
    if !SUPPORTED_RECEIPT_KINDS.contains(&dependency.receipt_kind.as_str()) {
        return Some(diagnostic(
            dependency,
            "cross_repo_dependency.unsupported_kind",
            "unsupported_kind",
            Some(SUPPORTED_RECEIPT_KINDS.join(",")),
            Some(dependency.receipt_kind.clone()),
            None,
        ));
    }
    if dependency.expected_status != PASS_STATUS {
        return Some(diagnostic(
            dependency,
            "cross_repo_dependency.expected_status_invalid",
            "malformed",
            Some(PASS_STATUS.to_string()),
            Some(dependency.expected_status.clone()),
            None,
        ));
    }
    for (field, hash) in [
        (
            "expected_policy_hash",
            dependency.expected_policy_hash.as_deref(),
        ),
        (
            "expected_subject_hash",
            dependency.expected_subject_hash.as_deref(),
        ),
        (
            "expected_receipt_hash",
            dependency.expected_receipt_hash.as_deref(),
        ),
    ] {
        if hash.is_some_and(|value| !valid_blake3(value)) {
            return Some(diagnostic(
                dependency,
                "cross_repo_dependency.hash_malformed",
                "malformed",
                Some(field.to_string()),
                hash.map(str::to_string),
                None,
            ));
        }
    }
    let required_non_claims = default_non_claims();
    if required_non_claims
        .iter()
        .any(|required| !dependency.non_claims.contains(required))
    {
        return Some(diagnostic(
            dependency,
            "cross_repo_dependency.non_claim_missing",
            "malformed",
            Some(required_non_claims.join(" | ")),
            Some(dependency.non_claims.join(" | ")),
            None,
        ));
    }
    None
}

fn validate_summary_safety(
    dependency: &CrossRepoDependency,
    summary: &CrossRepoReceiptSummary,
) -> Option<CrossRepoDependencyDiagnostic> {
    if !valid_blake3(&summary.receipt_hash) {
        return Some(diagnostic(
            dependency,
            "cross_repo_dependency.receipt_hash_malformed",
            "malformed",
            Some("BLAKE3 receipt hash".to_string()),
            Some(summary.receipt_hash.clone()),
            summary.artifact_ref.clone(),
        ));
    }
    if summary
        .artifact_ref
        .as_deref()
        .is_some_and(|reference| !safe_relative_reference(reference))
    {
        return Some(diagnostic(
            dependency,
            "cross_repo_dependency.artifact_ref_unsafe",
            "unsafe_diagnostic",
            Some("safe repository-relative artifact reference".to_string()),
            summary.artifact_ref.clone(),
            None,
        ));
    }
    if summary
        .diagnostic_summary
        .as_deref()
        .is_some_and(|text| !safe_diagnostic_summary(text))
    {
        return Some(diagnostic(
            dependency,
            "cross_repo_dependency.diagnostic_unsafe",
            "unsafe_diagnostic",
            Some("bounded diagnostic without raw secrets or host paths".to_string()),
            Some("unsafe diagnostic rejected".to_string()),
            summary.artifact_ref.clone(),
        ));
    }
    None
}

fn compare_optional_hash(
    dependency: &CrossRepoDependency,
    summary: &CrossRepoReceiptSummary,
    expected: Option<&str>,
    actual: Option<&str>,
    status: &str,
) -> Option<CrossRepoDependencyRowResult> {
    let expected = expected?;
    if actual == Some(expected) {
        return None;
    }
    Some(blocked_row(
        dependency,
        status,
        diagnostic(
            dependency,
            &format!("cross_repo_dependency.{status}"),
            status,
            Some(expected.to_string()),
            actual.map(str::to_string),
            summary.artifact_ref.clone(),
        ),
    ))
}

fn blocked_row(
    dependency: &CrossRepoDependency,
    status: &str,
    diagnostic: CrossRepoDependencyDiagnostic,
) -> CrossRepoDependencyRowResult {
    CrossRepoDependencyRowResult {
        dependency_id: dependency.id.clone(),
        repo: dependency.repo.clone(),
        change: dependency.change.clone(),
        receipt_kind: dependency.receipt_kind.clone(),
        status: status.to_string(),
        satisfied: false,
        evidence_receipt_hash: None,
        diagnostics: vec![diagnostic],
    }
}

fn diagnostic(
    dependency: &CrossRepoDependency,
    code: &str,
    status: &str,
    expected: Option<String>,
    actual: Option<String>,
    artifact_ref: Option<String>,
) -> CrossRepoDependencyDiagnostic {
    let diagnostic_id = canonical_json_hash(&json!({
        "dependency_id": dependency.id,
        "code": code,
        "status": status,
        "expected": expected,
        "actual": actual,
    }));
    CrossRepoDependencyDiagnostic {
        diagnostic_id,
        code: code.to_string(),
        dependency_id: dependency.id.clone(),
        status: status.to_string(),
        expected,
        actual,
        remediation: format!(
            "supply current passing {} evidence for {}/{}",
            dependency.receipt_kind, dependency.repo, dependency.change
        ),
        evidence_guidance: format!(
            "pass an explicit receipt with --cross-repo-evidence <FILE> for dependency {}",
            dependency.id
        ),
        artifact_ref,
        severity: "error".to_string(),
        non_claims: default_non_claims(),
    }
}

fn dependencies_json(rows: &[CrossRepoDependency]) -> Vec<Value> {
    rows.iter()
        .map(|row| {
            json!({
                "id": row.id,
                "repo": row.repo,
                "change": row.change,
                "receipt_kind": row.receipt_kind,
                "expected_status": row.expected_status,
                "expected_policy_hash": row.expected_policy_hash,
                "expected_subject_hash": row.expected_subject_hash,
                "expected_receipt_hash": row.expected_receipt_hash,
                "non_claims": row.non_claims,
            })
        })
        .collect()
}

fn evidence_json(rows: &[CrossRepoReceiptSummary]) -> Vec<Value> {
    rows.iter()
        .map(|row| {
            json!({
                "repo": row.repo,
                "change": row.change,
                "receipt_kind": row.receipt_kind,
                "status": row.status,
                "policy_hash": row.policy_hash,
                "subject_hash": row.subject_hash,
                "receipt_hash": row.receipt_hash,
                "artifact_ref": row.artifact_ref,
                "diagnostic_summary": row.diagnostic_summary,
            })
        })
        .collect()
}

fn rows_json(rows: &[CrossRepoDependencyRowResult]) -> Vec<Value> {
    rows.iter()
        .map(|row| {
            json!({
                "dependency_id": row.dependency_id,
                "repo": row.repo,
                "change": row.change,
                "receipt_kind": row.receipt_kind,
                "status": row.status,
                "satisfied": row.satisfied,
                "evidence_receipt_hash": row.evidence_receipt_hash,
                "diagnostics": row.diagnostics.iter().map(diagnostic_json).collect::<Vec<_>>(),
            })
        })
        .collect()
}

fn diagnostic_json(diagnostic: &CrossRepoDependencyDiagnostic) -> Value {
    json!({
        "diagnostic_id": diagnostic.diagnostic_id,
        "code": diagnostic.code,
        "dependency_id": diagnostic.dependency_id,
        "status": diagnostic.status,
        "expected": diagnostic.expected,
        "actual": diagnostic.actual,
        "remediation": diagnostic.remediation,
        "evidence_guidance": diagnostic.evidence_guidance,
        "artifact_ref": diagnostic.artifact_ref,
        "severity": diagnostic.severity,
        "non_claims": diagnostic.non_claims,
    })
}

fn normalize_status(status: &str) -> String {
    match status.trim().to_ascii_lowercase().as_str() {
        "pass" | "passed" | "success" => PASS_STATUS.to_string(),
        "fail" | "failed" => "failed".to_string(),
        "timeout" | "timed-out" | "timed_out" => "timed_out".to_string(),
        "dry-run" | "dry_run" => "dry_run".to_string(),
        "infrastructure-error"
        | "infrastructure_error"
        | "infrastructure-failed"
        | "infrastructure_failed" => "infrastructure_failed".to_string(),
        known if NON_PASS_STATUSES.contains(&known) => known.to_string(),
        _ => "unknown".to_string(),
    }
}

fn safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        })
}

fn valid_blake3(value: &str) -> bool {
    let digest = value.strip_prefix(BLAKE3_PREFIX).unwrap_or(value);
    digest.len() == BLAKE3_HEX_LENGTH && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn safe_relative_reference(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('/')
        && !value.contains('\\')
        && !value.split('/').any(|component| component == "..")
}

fn safe_diagnostic_summary(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    value.len() <= MAX_DIAGNOSTIC_BYTES
        && !value.starts_with('/')
        && !lower.contains("password")
        && !lower.contains("token=")
        && !lower.contains("secret=")
        && !lower.contains("begin private key")
        && !lower.contains("raw prompt")
        && !lower.contains("environment=")
}

fn default_non_claims() -> Vec<String> {
    vec![
        CROSS_REPO_NON_CLAIM_LINKAGE.to_string(),
        CROSS_REPO_NON_CLAIM_UPSTREAM.to_string(),
        CROSS_REPO_NON_CLAIM_DOWNSTREAM.to_string(),
    ]
}

#[must_use]
pub fn parse_cross_repo_dependencies(
    value: Option<&Value>,
) -> (Vec<CrossRepoDependency>, Vec<String>) {
    let Some(value) = value else {
        return (Vec::new(), Vec::new());
    };
    let Some(rows) = value.as_array() else {
        return (
            Vec::new(),
            vec!["cross_repo_dependency.metadata.invalid_type".to_string()],
        );
    };
    let mut dependencies = Vec::new();
    let mut issues = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        match parse_dependency_row(row) {
            Ok(dependency) => dependencies.push(dependency),
            Err(code) => issues.push(format!("{code}: row {index}")),
        }
    }
    dependencies.sort_by(|left, right| left.id.cmp(&right.id));
    (dependencies, issues)
}

fn parse_dependency_row(value: &Value) -> Result<CrossRepoDependency, &'static str> {
    let object = value
        .as_object()
        .ok_or("cross_repo_dependency.metadata.row_invalid")?;
    let required = |field: &str| {
        object
            .get(field)
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or("cross_repo_dependency.metadata.field_missing")
    };
    let optional_hash = |field: &str| {
        object
            .get(field)
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_string)
                    .ok_or("cross_repo_dependency.metadata.hash_invalid")
            })
            .transpose()
    };
    let non_claims = object
        .get("non_claims")
        .and_then(Value::as_array)
        .ok_or("cross_repo_dependency.metadata.non_claims_invalid")?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_string)
                .ok_or("cross_repo_dependency.metadata.non_claim_invalid")
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CrossRepoDependency {
        id: required("id")?,
        repo: required("repo")?,
        change: required("change")?,
        receipt_kind: required("requires")?,
        expected_status: required("expected_status")?,
        expected_policy_hash: optional_hash("expected_policy_hash")?,
        expected_subject_hash: optional_hash("expected_subject_hash")?,
        expected_receipt_hash: optional_hash("expected_receipt_hash")?,
        non_claims,
    })
}

#[must_use]
pub fn dependency_blocker_messages(report: &CrossRepoDependencyReport) -> Vec<String> {
    report
        .rows
        .iter()
        .filter(|row| !row.satisfied)
        .flat_map(|row| {
            row.diagnostics.iter().map(|diagnostic| {
                format!(
                    "{}: {}: {}/{} requires {} ({})",
                    row.dependency_id,
                    diagnostic.code,
                    row.repo,
                    row.change,
                    row.receipt_kind,
                    row.status
                )
            })
        })
        .collect()
}

#[must_use]
pub fn summaries_by_identity(
    evidence: &[CrossRepoReceiptSummary],
) -> BTreeMap<(String, String, String), Vec<CrossRepoReceiptSummary>> {
    let mut grouped = BTreeMap::new();
    for summary in evidence {
        grouped
            .entry((
                summary.repo.clone(),
                summary.change.clone(),
                summary.receipt_kind.clone(),
            ))
            .or_insert_with(Vec::new)
            .push(summary.clone());
    }
    grouped
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const HASH_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const HASH_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

    fn dependency(kind: &str) -> CrossRepoDependency {
        CrossRepoDependency {
            id: format!("valence-{kind}"),
            repo: "valence".to_string(),
            change: "canonical-evidence".to_string(),
            receipt_kind: kind.to_string(),
            expected_status: PASS_STATUS.to_string(),
            expected_policy_hash: Some(HASH_A.to_string()),
            expected_subject_hash: Some(HASH_B.to_string()),
            expected_receipt_hash: Some(HASH_C.to_string()),
            non_claims: default_non_claims(),
        }
    }

    fn evidence(kind: &str, status: &str) -> CrossRepoReceiptSummary {
        CrossRepoReceiptSummary {
            repo: "valence".to_string(),
            change: "canonical-evidence".to_string(),
            receipt_kind: kind.to_string(),
            status: status.to_string(),
            policy_hash: Some(HASH_A.to_string()),
            subject_hash: Some(HASH_B.to_string()),
            receipt_hash: HASH_C.to_string(),
            artifact_ref: Some(format!("evidence/{kind}.json")),
            diagnostic_summary: None,
        }
    }

    #[test]
    // r[verify cairn.cross_repo_change_dependencies.fixtures]
    // r[verify cairn.cross_repo_change_dependencies.fixtures.positive]
    // r[verify cairn.cross_repo_change_dependencies.validation.match]
    // r[verify cairn.cross_repo_change_dependencies.final_validation.positive]
    fn every_supported_receipt_kind_passes_exact_matching_evidence() {
        for kind in SUPPORTED_RECEIPT_KINDS {
            let report = evaluate_cross_repo_dependencies(
                &[dependency(kind)],
                &[evidence(kind, "PASS")],
                HASH_A,
            );
            assert!(report.valid, "{kind}: {:?}", report.rows);
            assert_eq!(report.satisfied_count, 1);
            assert_eq!(report.blocked_count, 0);
            assert_eq!(report.to_json()["verdict"], "PASS");
        }
    }

    #[test]
    // r[verify cairn.cross_repo_change_dependencies.fixtures.negative]
    // r[verify cairn.cross_repo_change_dependencies.status_feedback.non_pass]
    // r[verify cairn.cross_repo_change_dependencies.final_validation.negative]
    fn every_non_pass_status_remains_blocked() {
        for status in NON_PASS_STATUSES {
            let report = evaluate_cross_repo_dependencies(
                &[dependency("archive_receipt")],
                &[evidence("archive_receipt", status)],
                HASH_A,
            );
            assert!(!report.valid, "status {status} unexpectedly passed");
            assert_eq!(report.rows[0].status, *status);
        }
    }

    #[test]
    fn mismatches_and_unsafe_diagnostics_fail_with_stable_codes() {
        type Mutation = Box<dyn Fn(&mut CrossRepoReceiptSummary)>;
        let cases: Vec<(&str, Mutation)> = vec![
            (
                "cross_repo_dependency.wrong_kind",
                Box::new(|summary| summary.receipt_kind = "sync_receipt".to_string()),
            ),
            (
                "cross_repo_dependency.stale_policy",
                Box::new(|summary| summary.policy_hash = Some(HASH_B.to_string())),
            ),
            (
                "cross_repo_dependency.stale_subject",
                Box::new(|summary| summary.subject_hash = Some(HASH_A.to_string())),
            ),
            (
                "cross_repo_dependency.receipt_hash_mismatch",
                Box::new(|summary| summary.receipt_hash = HASH_A.to_string()),
            ),
            (
                "cross_repo_dependency.diagnostic_unsafe",
                Box::new(|summary| summary.diagnostic_summary = Some("token=secret".to_string())),
            ),
        ];
        for (code, mutate) in cases {
            let dependency = dependency("archive_receipt");
            let mut summary = evidence("archive_receipt", "PASS");
            mutate(&mut summary);
            let report = evaluate_cross_repo_dependencies(&[dependency], &[summary], HASH_A);
            assert!(!report.valid);
            assert_eq!(report.rows[0].diagnostics[0].code, code);
        }
    }

    #[test]
    fn missing_wrong_repo_wrong_change_and_unsupported_kind_fail_closed() {
        let dependency = dependency("archive_receipt");
        let missing =
            evaluate_cross_repo_dependencies(std::slice::from_ref(&dependency), &[], HASH_A);
        assert_eq!(missing.rows[0].status, "missing");

        let mut wrong_repo = evidence("archive_receipt", "PASS");
        wrong_repo.repo = "octet".to_string();
        let report = evaluate_cross_repo_dependencies(
            std::slice::from_ref(&dependency),
            &[wrong_repo],
            HASH_A,
        );
        assert_eq!(report.rows[0].status, "missing");

        let mut wrong_change = evidence("archive_receipt", "PASS");
        wrong_change.change = "other".to_string();
        let report = evaluate_cross_repo_dependencies(
            std::slice::from_ref(&dependency),
            &[wrong_change],
            HASH_A,
        );
        assert_eq!(report.rows[0].status, "missing");

        let mut unsupported = dependency;
        unsupported.receipt_kind = "shell_log".to_string();
        let report = evaluate_cross_repo_dependencies(&[unsupported], &[], HASH_A);
        assert_eq!(report.rows[0].status, "malformed");
        assert_eq!(
            report.rows[0].diagnostics[0].code,
            "cross_repo_dependency.unsupported_kind"
        );
    }

    #[test]
    fn parsing_rejects_malformed_rows_without_panicking() {
        let malformed = json!([{"id":"demo"}, "not-an-object"]);
        let (rows, issues) = parse_cross_repo_dependencies(Some(&malformed));
        assert!(rows.is_empty());
        assert_eq!(issues.len(), 2);
        let (rows, issues) = parse_cross_repo_dependencies(Some(&json!({})));
        assert!(rows.is_empty());
        assert_eq!(issues, ["cross_repo_dependency.metadata.invalid_type"]);
    }

    #[test]
    // r[impl cairn.radicle_issue_cross_repo.final_validation]
    // r[impl cairn.radicle_issue_cross_repo.final_validation.positive]
    // r[impl cairn.radicle_issue_cross_repo.final_validation.gates]
    // r[verify cairn.radicle_issue_cross_repo.receipt_kind]
    // r[verify cairn.radicle_issue_cross_repo.receipt_kind.valid]
    // r[verify cairn.radicle_issue_cross_repo.contract]
    // r[verify cairn.radicle_issue_cross_repo.contract.identity]
    // r[verify cairn.radicle_issue_cross_repo.contract.privacy]
    // r[verify cairn.radicle_issue_cross_repo.adapter_bounded]
    // r[verify cairn.radicle_issue_cross_repo.adapter_bounded.observe]
    // r[verify cairn.radicle_issue_cross_repo.adapter_bounded.non_claims]
    // r[verify cairn.radicle_issue_cross_repo.validation]
    // r[verify cairn.radicle_issue_cross_repo.validation.match]
    // r[verify cairn.radicle_issue_cross_repo.final_validation]
    // r[verify cairn.radicle_issue_cross_repo.final_validation.positive]
    // r[verify cairn.radicle_issue_cross_repo.final_validation.gates]
    fn issue_receipt_binds_identity_and_satisfies_matching_linkage() {
        let contract = radicle_issue_receipt_contract();
        assert_eq!(contract["schema"], RADICLE_ISSUE_RECEIPT_SCHEMA_V1);
        assert_eq!(contract["receipt_kind"], RADICLE_ISSUE_RECEIPT_KIND);
        assert!(contract["non_claims"]
            .as_array()
            .map(|claims| claims.contains(&json!(RADICLE_ISSUE_NON_CLAIM_AUTHORITY)))
            .unwrap_or(false));

        let observation = RadicleIssueObservation {
            repo: "radicle-example".to_string(),
            issue_id: "issue-3".to_string(),
            object_id: HASH_B.to_string(),
            status: "open".to_string(),
            subject_hash: HASH_B.to_string(),
        };
        let summary = build_radicle_issue_receipt(&observation);
        assert_eq!(summary.receipt_kind, RADICLE_ISSUE_RECEIPT_KIND);
        assert_eq!(summary.repo, "radicle-example");
        assert_eq!(summary.change, "issue-3");
        assert_eq!(summary.subject_hash.as_deref(), Some(HASH_B));
        assert_eq!(
            summary.artifact_ref.as_deref(),
            Some(format!("radicle-issue:{HASH_B}")).as_deref()
        );

        let mut dependency = dependency(RADICLE_ISSUE_RECEIPT_KIND);
        dependency.change = "issue-3".to_string();
        dependency.repo = "radicle-example".to_string();
        dependency.expected_policy_hash = None;
        dependency.expected_subject_hash = Some(HASH_B.to_string());
        dependency.expected_receipt_hash = Some(summary.receipt_hash.clone());
        let report = evaluate_cross_repo_dependencies(&[dependency], &[summary], HASH_A);
        assert!(report.valid, "{:?}", report.rows);
        assert_eq!(report.satisfied_count, 1);
    }

    #[test]
    // r[impl cairn.radicle_issue_cross_repo.final_validation.negative]
    // r[verify cairn.radicle_issue_cross_repo.receipt_kind.invalid]
    // r[verify cairn.radicle_issue_cross_repo.adapter_bounded.non_claims]
    // r[verify cairn.radicle_issue_cross_repo.validation.mismatch]
    // r[verify cairn.radicle_issue_cross_repo.final_validation.negative]
    fn issue_receipt_mismatch_fails_closed_and_unsupported_kind_stays_rejected() {
        let mut dependency = dependency(RADICLE_ISSUE_RECEIPT_KIND);
        dependency.change = "issue-3".to_string();
        dependency.repo = "radicle-example".to_string();
        dependency.expected_policy_hash = None;
        dependency.expected_subject_hash = Some(HASH_B.to_string());

        let mut summary = build_radicle_issue_receipt(&RadicleIssueObservation {
            repo: "radicle-example".to_string(),
            issue_id: "issue-3".to_string(),
            object_id: HASH_B.to_string(),
            status: "open".to_string(),
            subject_hash: HASH_A.to_string(),
        });
        summary.receipt_hash = dependency
            .expected_receipt_hash
            .clone()
            .unwrap_or_else(|| HASH_C.to_string());
        let report = evaluate_cross_repo_dependencies(&[dependency.clone()], &[summary], HASH_A);
        assert!(!report.valid, "mismatched subject unexpectedly passed");
        assert_eq!(
            report.rows[0].diagnostics[0].code,
            "cross_repo_dependency.stale_subject"
        );

        let mut unsupported = dependency;
        unsupported.receipt_kind = "radicle-patch.v1".to_string();
        let report = evaluate_cross_repo_dependencies(&[unsupported], &[], HASH_A);
        assert_eq!(report.rows[0].status, "malformed");
        assert_eq!(
            report.rows[0].diagnostics[0].code,
            "cross_repo_dependency.unsupported_kind"
        );
        assert!(!RADICLE_ISSUE_NON_CLAIM_LINKAGE.is_empty());
    }
}
