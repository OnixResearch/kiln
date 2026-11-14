use serde_json::Value;

use super::common::{
    push_issue, require_known_string, require_known_string_at, require_string,
    validate_optional_receipt_hash, without_hash_field,
};
use super::{
    SemanticReviewIssue, SemanticReviewReport, REVIEW_CATEGORIES, REVIEW_DECISIONS, REVIEW_KINDS,
    REVIEW_RECEIPT_SCHEMA_V1, REVIEW_SEVERITIES, REVIEW_SOURCES,
};
use crate::verified::evidence::canonical_json_hash;

pub fn review_receipt_hash(value: &Value) -> String {
    canonical_json_hash(&without_hash_field(value, "receipt_hash"))
}

pub fn validate_review_receipt(
    value: &Value,
    observed_artifact_hashes: &[(String, String)],
) -> SemanticReviewReport {
    // r[impl cairn.semantic_review.semantic_review_receipts.schema]
    // r[impl cairn.semantic_review.semantic_review_receipts.schema.schema.fields]
    // r[impl cairn.semantic_review.review_rejection_taxonomy.policy]
    // r[impl cairn.semantic_review.review_rejection_taxonomy.policy.policy.categories]
    // r[impl cairn.semantic_review.review_rejection_taxonomy.policy.policy.decisions]
    // r[impl cairn.semantic_review.semantic_review_boundary.boundary]
    // r[impl cairn.semantic_review.semantic_review_boundary.boundary.boundary.non_execution]
    // r[impl cairn.semantic_review.semantic_review_boundary.fcis]
    // r[impl cairn.semantic_review.semantic_review_boundary.fcis.fcis.pure_core]
    let mut issues = Vec::new();
    require_string(
        value,
        "schema_version",
        REVIEW_RECEIPT_SCHEMA_V1,
        &mut issues,
    );
    require_known_string(value, "review_kind", REVIEW_KINDS, &mut issues);
    let decision = require_known_string(value, "decision", REVIEW_DECISIONS, &mut issues);
    validate_reviewer(value.get("reviewer"), &mut issues);
    validate_reviewed_inputs(value, observed_artifact_hashes, &mut issues);
    validate_findings(value, decision.as_deref(), &mut issues);
    validate_optional_receipt_hash(value, &mut issues, review_receipt_hash(value));
    report(value, issues)
}

fn validate_reviewer(reviewer: Option<&Value>, issues: &mut Vec<SemanticReviewIssue>) {
    let Some(reviewer) = reviewer else {
        push_issue(
            issues,
            "missing_field",
            "reviewer",
            "missing reviewer metadata",
        );
        return;
    };
    require_known_string(reviewer, "source", REVIEW_SOURCES, issues);
    if reviewer
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .is_empty()
    {
        push_issue(
            issues,
            "missing_field",
            "reviewer.id",
            "missing reviewer id",
        );
    }
}

fn validate_reviewed_inputs(
    value: &Value,
    observed_artifact_hashes: &[(String, String)],
    issues: &mut Vec<SemanticReviewIssue>,
) {
    // r[impl cairn.semantic_review.semantic_review_receipts.schema.schema.hash_binding]
    let Some(inputs) = value.get("reviewed_inputs").and_then(Value::as_array) else {
        push_issue(
            issues,
            "missing_field",
            "reviewed_inputs",
            "missing reviewed inputs",
        );
        return;
    };
    if inputs.is_empty() {
        push_issue(
            issues,
            "empty_array",
            "reviewed_inputs",
            "reviewed inputs must not be empty",
        );
    }
    for (index, input) in inputs.iter().enumerate() {
        let field = format!("reviewed_inputs[{index}]");
        let path = input.get("path").and_then(Value::as_str).unwrap_or("");
        if path.is_empty() {
            push_issue(
                issues,
                "missing_field",
                &format!("{field}.path"),
                "missing input path",
            );
        }
        let stated_hash = input.get("hash").and_then(Value::as_str).unwrap_or("");
        if !super::common::is_blake3_hex(stated_hash) {
            push_issue(
                issues,
                "invalid_hash",
                &format!("{field}.hash"),
                "input hash must be blake3 hex",
            );
        }
        if let Some((_, observed)) = observed_artifact_hashes
            .iter()
            .find(|(observed_path, _)| observed_path == path)
        {
            if observed != stated_hash {
                push_issue(
                    issues,
                    "hash_mismatch",
                    &format!("{field}.hash"),
                    "input hash does not match filesystem artifact",
                );
            }
        }
    }
}

fn validate_findings(value: &Value, decision: Option<&str>, issues: &mut Vec<SemanticReviewIssue>) {
    // r[impl cairn.semantic_review.review_rejection_taxonomy.finding_validation]
    // r[impl cairn.semantic_review.review_rejection_taxonomy.finding_validation.finding_validation.required]
    // r[impl cairn.semantic_review.review_rejection_taxonomy.finding_validation.finding_validation.approved]
    let Some(findings) = value.get("findings").and_then(Value::as_array) else {
        push_issue(
            issues,
            "missing_field",
            "findings",
            "missing findings array",
        );
        return;
    };
    if decision != Some("approved") && findings.is_empty() {
        push_issue(
            issues,
            "missing_findings",
            "findings",
            "non-approved reviews require findings",
        );
    }
    for (index, finding) in findings.iter().enumerate() {
        let field = format!("findings[{index}]");
        if finding
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .is_empty()
        {
            push_issue(
                issues,
                "missing_field",
                &format!("{field}.id"),
                "missing finding id",
            );
        }
        require_known_string_at(finding, "category", REVIEW_CATEGORIES, &field, issues);
        require_known_string_at(finding, "severity", REVIEW_SEVERITIES, &field, issues);
    }
}

fn report(value: &Value, issues: Vec<SemanticReviewIssue>) -> SemanticReviewReport {
    let valid = issues.is_empty();
    SemanticReviewReport {
        valid,
        verdict: if valid { "PASS" } else { "FAIL" }.to_string(),
        issues,
        receipt_hash: review_receipt_hash(value),
    }
}
