use serde_json::{json, Map, Value};

use super::SemanticReviewIssue;

pub(super) fn require_string(
    value: &Value,
    field: &'static str,
    expected: &str,
    issues: &mut Vec<SemanticReviewIssue>,
) {
    match value.get(field).and_then(Value::as_str) {
        Some(actual) if actual == expected => {}
        Some(_) => push_issue(issues, "invalid_field", field, "field has unexpected value"),
        None => push_issue(issues, "missing_field", field, "missing required field"),
    }
}

pub(super) fn require_known_string(
    value: &Value,
    field: &'static str,
    allowed: &[&str],
    issues: &mut Vec<SemanticReviewIssue>,
) -> Option<String> {
    match value.get(field).and_then(Value::as_str) {
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
    value: &Value,
    field: &'static str,
    allowed: &[&str],
    prefix: &str,
    issues: &mut Vec<SemanticReviewIssue>,
) -> Option<String> {
    match value.get(field).and_then(Value::as_str) {
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
    value: &Value,
    field: &'static str,
    issues: &mut Vec<SemanticReviewIssue>,
) {
    match value.get(field).and_then(Value::as_str) {
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
    value: &Value,
    field: &'static str,
    issues: &mut Vec<SemanticReviewIssue>,
) {
    if let Some(actual) = value.get(field).and_then(Value::as_str) {
        if !is_blake3_hex(actual) {
            push_issue(issues, "invalid_hash", field, "field must be blake3 hex");
        }
    }
}

pub(super) fn validate_hash_field(
    value: &Value,
    field: &'static str,
    prefix: &str,
    issues: &mut Vec<SemanticReviewIssue>,
) {
    match value.get(field).and_then(Value::as_str) {
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
    value: &Value,
    issues: &mut Vec<SemanticReviewIssue>,
    expected: String,
) {
    if let Some(actual) = value.get("receipt_hash").and_then(Value::as_str) {
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

pub fn issues_json(issues: &[SemanticReviewIssue]) -> Value {
    Value::Array(
        issues
            .iter()
            .map(|issue| {
                json!({
                    "code": issue.code,
                    "field_path": issue.field_path,
                    "message": issue.message,
                })
            })
            .collect(),
    )
}

pub(super) fn without_hash_field(value: &Value, field: &str) -> Value {
    match value {
        Value::Object(object) => {
            let mut cloned = object.clone();
            cloned.remove(field);
            Value::Object(cloned)
        }
        other => other.clone(),
    }
}

pub(super) fn count_string_field(values: &[Value], field: &str) -> Value {
    let mut counts = Map::new();
    for value in values {
        if let Some(text) = value.get(field).and_then(Value::as_str) {
            increment_count(&mut counts, text);
        }
    }
    sort_count_map(&mut counts)
}

pub(super) fn increment_count(counts: &mut Map<String, Value>, key: &str) {
    let current = counts.get(key).and_then(Value::as_u64).unwrap_or(0);
    counts.insert(key.to_string(), json!(current + 1));
}

pub(super) fn sort_count_map(counts: &mut Map<String, Value>) -> Value {
    let mut pairs = counts
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<Vec<_>>();
    pairs.sort_by(|left, right| left.0.cmp(&right.0));
    let mut sorted = Map::new();
    for (key, value) in pairs {
        sorted.insert(key, value);
    }
    Value::Object(sorted)
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
    issues: &mut Vec<SemanticReviewIssue>,
    code: &str,
    field_path: &str,
    message: &str,
) {
    issues.push(SemanticReviewIssue {
        code: code.to_string(),
        field_path: field_path.to_string(),
        message: message.to_string(),
    });
}
