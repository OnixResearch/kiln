pub(super) fn build_report(
    schema_version: &str,
    mut issues: Vec<super::super::SemanticReviewIssue>,
) -> super::super::SemanticReviewReport {
    issues.sort_by(|left, right| {
        left.field_path
            .cmp(&right.field_path)
            .then(left.code.cmp(&right.code))
            .then(left.message.cmp(&right.message))
    });
    let valid = issues.is_empty();
    let verdict = if valid { "PASS" } else { "FAIL" }.to_string();
    let receipt_hash = crate::verified::evidence::canonical_json_hash(&serde_json::json!({
        "schema_version": schema_version,
        "valid": valid,
        "verdict": verdict,
        "issues": super::super::common::issues_json(&issues),
    }));
    super::super::SemanticReviewReport {
        valid,
        verdict,
        issues,
        receipt_hash,
    }
}
