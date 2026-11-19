pub(super) fn require_string(
    value: &serde_json::Value,
    field: &'static str,
    expected: &str,
    issues: &mut Vec<super::SemanticReviewIssue>,
) {
    match value.get(field).and_then(serde_json::Value::as_str) {
        Some(actual) if actual == expected => {}
        Some(_) => push_issue(issues, "invalid_field", field, "field has unexpected value"),
        None => push_issue(issues, "missing_field", field, "missing required field"),
    }
}

pub(super) fn require_known_string(
    value: &serde_json::Value,
    field: &'static str,
    allowed: &[&str],
    issues: &mut Vec<super::SemanticReviewIssue>,
) -> Option<String> {
    match value.get(field).and_then(serde_json::Value::as_str) {
        Some(actual) if allowed.contains(&actual) => Some(actual.to_string()),
        Some(_) => {
            push_issue(issues, "invalid_field", field, "field is not policy-known");
            None
        }
        None => {
            push_issue(issues, "missing_field", field, "missing required field");
            None
        }
    }
}

pub(super) fn require_known_string_at(
    value: &serde_json::Value,
    field: &'static str,
    allowed: &[&str],
    prefix: &str,
    issues: &mut Vec<super::SemanticReviewIssue>,
) -> Option<String> {
    match value.get(field).and_then(serde_json::Value::as_str) {
        Some(actual) if allowed.contains(&actual) => Some(actual.to_string()),
        Some(_) => {
            push_issue(
                issues,
                "invalid_field",
                &format!("{prefix}.{field}"),
                "field is not policy-known",
            );
            None
        }
        None => {
            push_issue(
                issues,
                "missing_field",
                &format!("{prefix}.{field}"),
                "missing required field",
            );
            None
        }
    }
}

pub(super) fn require_hex_string(
    value: &serde_json::Value,
    field: &'static str,
    issues: &mut Vec<super::SemanticReviewIssue>,
) {
    match value.get(field).and_then(serde_json::Value::as_str) {
        Some(actual) if is_blake3_hex(actual) => {}
        Some(_) => push_issue(issues, "invalid_hash", field, "field must be blake3 hex"),
        None => push_issue(
            issues,
            "missing_field",
            field,
            "missing required hash field",
        ),
    }
}

pub(super) fn validate_optional_hash(
    value: &serde_json::Value,
    field: &'static str,
    issues: &mut Vec<super::SemanticReviewIssue>,
) {
    if let Some(actual) = value.get(field).and_then(serde_json::Value::as_str) {
        if !is_blake3_hex(actual) {
            push_issue(issues, "invalid_hash", field, "field must be blake3 hex");
        }
    }
}

pub(super) fn validate_hash_field(
    value: &serde_json::Value,
    field: &'static str,
    prefix: &str,
    issues: &mut Vec<super::SemanticReviewIssue>,
) {
    match value.get(field).and_then(serde_json::Value::as_str) {
        Some(actual) if is_blake3_hex(actual) => {}
        Some(_) => push_issue(
            issues,
            "invalid_hash",
            &format!("{prefix}.{field}"),
            "field must be blake3 hex",
        ),
        None => push_issue(
            issues,
            "missing_field",
            &format!("{prefix}.{field}"),
            "missing hash field",
        ),
    }
}

pub(super) fn validate_optional_receipt_hash(
    value: &serde_json::Value,
    issues: &mut Vec<super::SemanticReviewIssue>,
    expected: String,
) {
    if let Some(actual) = value
        .get("receipt_hash")
        .and_then(serde_json::Value::as_str)
    {
        if actual != expected {
            push_issue(
                issues,
                "receipt_hash_mismatch",
                "receipt_hash",
                "receipt hash does not match canonical receipt material",
            );
        }
    }
}

pub fn issues_json(issues: &[super::SemanticReviewIssue]) -> serde_json::Value {
    serde_json::Value::Array(
        issues
            .iter()
            .map(|issue| {
                serde_json::json!({
                    "code": issue.code,
                    "field_path": issue.field_path,
                    "message": issue.message,
                })
            })
            .collect(),
    )
}

pub(super) fn without_hash_field(value: &serde_json::Value, field: &str) -> serde_json::Value {
    match value {
        serde_json::Value::Object(object) => {
            let mut cloned = object.clone();
            cloned.remove(field);
            serde_json::Value::Object(cloned)
        }
        other => other.clone(),
    }
}

pub(super) fn count_string_field(values: &[serde_json::Value], field: &str) -> serde_json::Value {
    let mut counts = serde_json::Map::new();
    for value in values {
        if let Some(text) = value.get(field).and_then(serde_json::Value::as_str) {
            increment_count(&mut counts, text);
        }
    }
    sort_count_map(&mut counts)
}

pub(super) fn increment_count(counts: &mut serde_json::Map<String, serde_json::Value>, key: &str) {
    let current = counts
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    counts.insert(key.to_string(), serde_json::json!(current + 1));
}

pub(super) fn sort_count_map(
    counts: &mut serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let mut pairs = counts
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<Vec<_>>();
    pairs.sort_by(|left, right| left.0.cmp(&right.0));
    let mut sorted = serde_json::Map::new();
    for (key, value) in pairs {
        sorted.insert(key, value);
    }
    serde_json::Value::Object(sorted)
}

pub(super) fn sorted_unique(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

pub(super) fn is_blake3_hex(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

pub(super) fn push_issue(
    issues: &mut Vec<super::SemanticReviewIssue>,
    code: &str,
    field_path: &str,
    message: &str,
) {
    issues.push(super::SemanticReviewIssue {
        code: code.to_string(),
        field_path: field_path.to_string(),
        message: message.to_string(),
    });
}
