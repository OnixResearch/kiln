use serde_json::json;

use crate::verified::evidence::canonical_json_hash;

use super::super::common::issues_json;
use super::super::{SemanticReviewIssue, SemanticReviewReport};

pub(super) fn build_report(
    schema_version: &str,
    mut issues: Vec<SemanticReviewIssue>,
) -> SemanticReviewReport {
    issues.sort_by(|left, right| {
        left.field_path
            .cmp(&right.field_path)
            .then(left.code.cmp(&right.code))
            .then(left.message.cmp(&right.message))
    });
    let valid = issues.is_empty();
    let verdict = if valid { "PASS" } else { "FAIL" }.to_string();
    let receipt_hash = canonical_json_hash(&json!({
        "schema_version": schema_version,
        "valid": valid,
        "verdict": verdict,
        "issues": issues_json(&issues),
    }));
    SemanticReviewReport {
        valid,
        verdict,
        issues,
        receipt_hash,
    }
}
