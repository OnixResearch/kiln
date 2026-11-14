use serde_json::Value;

use super::common::{
    push_issue, require_hex_string, require_known_string_at, require_string, validate_hash_field,
    validate_optional_hash, validate_optional_receipt_hash, without_hash_field,
};
use super::{
    SemanticReviewIssue, SemanticReviewReport, REVISION_DISPOSITIONS, REVISION_HANDOFF_SCHEMA_V1,
};
use crate::verified::evidence::canonical_json_hash;

pub fn revision_handoff_hash(value: &Value) -> String {
    canonical_json_hash(&without_hash_field(value, "receipt_hash"))
}

pub fn validate_revision_handoff(value: &Value) -> SemanticReviewReport {
    // r[impl cairn.semantic_review.revision_handoff_receipts.schema]
    // r[impl cairn.semantic_review.revision_handoff_receipts.schema.schema.chain]
    // r[impl cairn.semantic_review.revision_handoff_receipts.schema.schema.dispositions]
    // r[impl cairn.semantic_review.revision_handoff_receipts.validation]
    let mut issues = Vec::new();
    require_string(
        value,
        "schema_version",
        REVISION_HANDOFF_SCHEMA_V1,
        &mut issues,
    );
    require_hex_string(value, "prior_review_receipt_hash", &mut issues);
    validate_optional_hash(value, "followup_review_receipt_hash", &mut issues);
    validate_revision_artifacts(value.get("revision_artifacts"), &mut issues);
    validate_dispositions(value.get("finding_dispositions"), &mut issues);
    validate_optional_receipt_hash(value, &mut issues, revision_handoff_hash(value));
    report(value, issues)
}

fn validate_revision_artifacts(value: Option<&Value>, issues: &mut Vec<SemanticReviewIssue>) {
    let Some(artifacts) = value.and_then(Value::as_array) else {
        push_issue(
            issues,
            "missing_field",
            "revision_artifacts",
            "missing revision artifacts",
        );
        return;
    };
    if artifacts.is_empty() {
        push_issue(
            issues,
            "empty_array",
            "revision_artifacts",
            "revision artifacts must not be empty",
        );
    }
    for (index, artifact) in artifacts.iter().enumerate() {
        let field = format!("revision_artifacts[{index}]");
        if artifact
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or("")
            .is_empty()
        {
            push_issue(
                issues,
                "missing_field",
                &format!("{field}.path"),
                "missing artifact path",
            );
        }
        validate_hash_field(artifact, "hash", &field, issues);
    }
}

fn validate_dispositions(value: Option<&Value>, issues: &mut Vec<SemanticReviewIssue>) {
    let Some(dispositions) = value.and_then(Value::as_array) else {
        push_issue(
            issues,
            "missing_field",
            "finding_dispositions",
            "missing finding dispositions",
        );
        return;
    };
    if dispositions.is_empty() {
        push_issue(
            issues,
            "empty_array",
            "finding_dispositions",
            "finding dispositions must not be empty",
        );
    }
    for (index, disposition) in dispositions.iter().enumerate() {
        let field = format!("finding_dispositions[{index}]");
        if disposition
            .get("finding_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .is_empty()
        {
            push_issue(
                issues,
                "missing_field",
                &format!("{field}.finding_id"),
                "missing finding id",
            );
        }
        let disposition_value = require_known_string_at(
            disposition,
            "disposition",
            REVISION_DISPOSITIONS,
            &field,
            issues,
        );
        if disposition_value.as_deref() == Some("addressed") {
            let has_refs = disposition
                .get("evidence_refs")
                .and_then(Value::as_array)
                .map(|refs| !refs.is_empty())
                .unwrap_or(false);
            let has_hashes = disposition
                .get("artifact_hashes")
                .and_then(Value::as_array)
                .map(|hashes| !hashes.is_empty())
                .unwrap_or(false);
            if !has_refs && !has_hashes {
                // r[impl cairn.semantic_review.revision_handoff_receipts.validation.validation.addressed_evidence]
                push_issue(
                    issues,
                    "missing_addressed_evidence",
                    &field,
                    "addressed findings require evidence refs or artifact hashes",
                );
            }
        }
        if disposition_value.as_deref() == Some("new") {
            // r[impl cairn.semantic_review.revision_handoff_receipts.validation.validation.new_findings]
            if disposition
                .get("introduced_by_followup")
                .and_then(Value::as_bool)
                != Some(true)
            {
                push_issue(
                    issues,
                    "missing_new_finding_marker",
                    &field,
                    "new findings must be marked as introduced by follow-up review",
                );
            }
        }
    }
}

fn report(value: &Value, issues: Vec<SemanticReviewIssue>) -> SemanticReviewReport {
    let valid = issues.is_empty();
    SemanticReviewReport {
        valid,
        verdict: if valid { "PASS" } else { "FAIL" }.to_string(),
        issues,
        receipt_hash: revision_handoff_hash(value),
    }
}
