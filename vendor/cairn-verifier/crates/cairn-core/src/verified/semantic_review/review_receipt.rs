pub fn review_receipt_hash(value: &serde_json::Value) -> String {
    crate::verified::evidence::canonical_json_hash(&super::common::without_hash_field(
        value,
        "receipt_hash",
    ))
}

pub fn validate_review_receipt(
    value: &serde_json::Value,
    observed_artifact_hashes: &[(String, String)],
) -> super::SemanticReviewReport {
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
    super::common::require_string(
        value,
        "schema_version",
        super::REVIEW_RECEIPT_SCHEMA_V1,
        &mut issues,
    );
    super::common::require_known_string(value, "review_kind", super::REVIEW_KINDS, &mut issues);
    let decision = super::common::require_known_string(
        value,
        "decision",
        super::REVIEW_DECISIONS,
        &mut issues,
    );
    validate_reviewer(value.get("reviewer"), &mut issues);
    validate_reviewed_inputs(value, observed_artifact_hashes, &mut issues);
    validate_findings(value, decision.as_deref(), &mut issues);
    super::common::validate_optional_receipt_hash(value, &mut issues, review_receipt_hash(value));
    report(value, issues)
}

fn validate_reviewer(
    reviewer: Option<&serde_json::Value>,
    issues: &mut Vec<super::SemanticReviewIssue>,
) {
    let Some(reviewer) = reviewer else {
        super::common::push_issue(
            issues,
            "missing_field",
            "reviewer",
            "missing reviewer metadata",
        );
        return;
    };
    super::common::require_known_string(reviewer, "source", super::REVIEW_SOURCES, issues);
    if reviewer
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .is_empty()
    {
        super::common::push_issue(
            issues,
            "missing_field",
            "reviewer.id",
            "missing reviewer id",
        );
    }
}

fn validate_reviewed_inputs(
    value: &serde_json::Value,
    observed_artifact_hashes: &[(String, String)],
    issues: &mut Vec<super::SemanticReviewIssue>,
) {
    // r[impl cairn.semantic_review.semantic_review_receipts.schema.schema.hash_binding]
    let Some(inputs) = value
        .get("reviewed_inputs")
        .and_then(serde_json::Value::as_array)
    else {
        super::common::push_issue(
            issues,
            "missing_field",
            "reviewed_inputs",
            "missing reviewed inputs",
        );
        return;
    };
    if inputs.is_empty() {
        super::common::push_issue(
            issues,
            "empty_array",
            "reviewed_inputs",
            "reviewed inputs must not be empty",
        );
    }
    for (index, input) in inputs.iter().enumerate() {
        let field = format!("reviewed_inputs[{index}]");
        let path = input
            .get("path")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if path.is_empty() {
            super::common::push_issue(
                issues,
                "missing_field",
                &format!("{field}.path"),
                "missing input path",
            );
        }
        let stated_hash = input
            .get("hash")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if !super::common::is_blake3_hex(stated_hash) {
            super::common::push_issue(
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
                super::common::push_issue(
                    issues,
                    "hash_mismatch",
                    &format!("{field}.hash"),
                    "input hash does not match filesystem artifact",
                );
            }
        }
    }
}

fn validate_findings(
    value: &serde_json::Value,
    decision: Option<&str>,
    issues: &mut Vec<super::SemanticReviewIssue>,
) {
    // r[impl cairn.semantic_review.review_rejection_taxonomy.finding_validation]
    // r[impl cairn.semantic_review.review_rejection_taxonomy.finding_validation.finding_validation.required]
    // r[impl cairn.semantic_review.review_rejection_taxonomy.finding_validation.finding_validation.approved]
    let Some(findings) = value.get("findings").and_then(serde_json::Value::as_array) else {
        super::common::push_issue(
            issues,
            "missing_field",
            "findings",
            "missing findings array",
        );
        return;
    };
    if decision != Some("approved") && findings.is_empty() {
        super::common::push_issue(
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
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .is_empty()
        {
            super::common::push_issue(
                issues,
                "missing_field",
                &format!("{field}.id"),
                "missing finding id",
            );
        }
        super::common::require_known_string_at(
            finding,
            "category",
            super::REVIEW_CATEGORIES,
            &field,
            issues,
        );
        super::common::require_known_string_at(
            finding,
            "severity",
            super::REVIEW_SEVERITIES,
            &field,
            issues,
        );
    }
}

fn report(
    value: &serde_json::Value,
    issues: Vec<super::SemanticReviewIssue>,
) -> super::SemanticReviewReport {
    let valid = issues.is_empty();
    super::SemanticReviewReport {
        valid,
        verdict: if valid { "PASS" } else { "FAIL" }.to_string(),
        issues,
        receipt_hash: review_receipt_hash(value),
    }
}
