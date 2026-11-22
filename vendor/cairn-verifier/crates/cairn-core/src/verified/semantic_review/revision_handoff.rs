pub fn revision_handoff_hash(value: &serde_json::Value) -> String {
    crate::verified::evidence::canonical_json_hash(&super::common::without_hash_field(
        value,
        "receipt_hash",
    ))
}

pub fn validate_revision_handoff(value: &serde_json::Value) -> super::SemanticReviewReport {
    // r[impl cairn.semantic_review.revision_handoff_receipts.schema]
    // r[impl cairn.semantic_review.revision_handoff_receipts.schema.schema.chain]
    // r[impl cairn.semantic_review.revision_handoff_receipts.schema.schema.dispositions]
    // r[impl cairn.semantic_review.revision_handoff_receipts.validation]
    let mut issues = Vec::new();
    super::common::require_string(
        value,
        "schema_version",
        super::REVISION_HANDOFF_SCHEMA_V1,
        &mut issues,
    );
    super::common::require_hex_string(value, "prior_review_receipt_hash", &mut issues);
    super::common::validate_optional_hash(value, "followup_review_receipt_hash", &mut issues);
    validate_revision_artifacts(value.get("revision_artifacts"), &mut issues);
    validate_dispositions(value.get("finding_dispositions"), &mut issues);
    super::common::validate_optional_receipt_hash(value, &mut issues, revision_handoff_hash(value));
    report(value, issues)
}

fn validate_revision_artifacts(
    value: Option<&serde_json::Value>,
    issues: &mut Vec<super::SemanticReviewIssue>,
) {
    let Some(artifacts) = value.and_then(serde_json::Value::as_array) else {
        super::common::push_issue(
            issues,
            "missing_field",
            "revision_artifacts",
            "missing revision artifacts",
        );
        return;
    };
    if artifacts.is_empty() {
        super::common::push_issue(
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
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .is_empty()
        {
            super::common::push_issue(
                issues,
                "missing_field",
                &format!("{field}.path"),
                "missing artifact path",
            );
        }
        super::common::validate_hash_field(artifact, "hash", &field, issues);
    }
}

fn validate_dispositions(
    value: Option<&serde_json::Value>,
    issues: &mut Vec<super::SemanticReviewIssue>,
) {
    let Some(dispositions) = value.and_then(serde_json::Value::as_array) else {
        super::common::push_issue(
            issues,
            "missing_field",
            "finding_dispositions",
            "missing finding dispositions",
        );
        return;
    };
    if dispositions.is_empty() {
        super::common::push_issue(
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
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .is_empty()
        {
            super::common::push_issue(
                issues,
                "missing_field",
                &format!("{field}.finding_id"),
                "missing finding id",
            );
        }
        let disposition_value = super::common::require_known_string_at(
            disposition,
            "disposition",
            super::REVISION_DISPOSITIONS,
            &field,
            issues,
        );
        if disposition_value.as_deref() == Some("addressed") {
            let has_refs = disposition
                .get("evidence_refs")
                .and_then(serde_json::Value::as_array)
                .map(|refs| !refs.is_empty())
                .unwrap_or(false);
            let has_hashes = disposition
                .get("artifact_hashes")
                .and_then(serde_json::Value::as_array)
                .map(|hashes| !hashes.is_empty())
                .unwrap_or(false);
            if !has_refs && !has_hashes {
                // r[impl cairn.semantic_review.revision_handoff_receipts.validation.validation.addressed_evidence]
                super::common::push_issue(
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
                .and_then(serde_json::Value::as_bool)
                != Some(true)
            {
                super::common::push_issue(
                    issues,
                    "missing_new_finding_marker",
                    &field,
                    "new findings must be marked as introduced by follow-up review",
                );
            }
        }
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
        receipt_hash: revision_handoff_hash(value),
    }
}
