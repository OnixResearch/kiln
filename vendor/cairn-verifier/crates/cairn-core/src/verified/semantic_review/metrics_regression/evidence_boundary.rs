// r[impl cairn.semantic_review.metrics_regression_split.focused_helpers]
use std::collections::BTreeMap;

use serde_json::Value;

use super::super::common::push_issue;
use super::super::SemanticReviewIssue;
use super::common::non_empty_string;

pub(super) fn evidence_ref_map<'a>(
    evidence: &'a Value,
    issues: &mut Vec<SemanticReviewIssue>,
) -> BTreeMap<&'a str, &'a Value> {
    let mut refs = BTreeMap::new();
    let Some(entries) = evidence.get("evidence").and_then(Value::as_array) else {
        push_issue(
            issues,
            "missing_evidence_entries",
            "evidence",
            "evidence boundary must declare sanitized evidence entries",
        );
        return refs;
    };
    if entries.is_empty() {
        push_issue(
            issues,
            "empty_evidence_entries",
            "evidence",
            "evidence boundary must declare sanitized evidence entries",
        );
        return refs;
    }
    for (index, entry) in entries.iter().enumerate() {
        collect_evidence_entry(entry, index, &mut refs, issues);
    }
    refs
}

fn collect_evidence_entry<'a>(
    entry: &'a Value,
    index: usize,
    refs: &mut BTreeMap<&'a str, &'a Value>,
    issues: &mut Vec<SemanticReviewIssue>,
) {
    if !entry.is_object() {
        push_issue(
            issues,
            "invalid_evidence_entry",
            &format!("evidence[{index}]"),
            "evidence entry must be an object",
        );
        return;
    }
    let Some(evidence_ref) = non_empty_string(entry, "ref") else {
        push_issue(
            issues,
            "invalid_evidence_entry",
            &format!("evidence[{index}].ref"),
            "evidence entry must bind a non-empty ref",
        );
        return;
    };
    for field in [
        "category_code",
        "metrics_class",
        "metrics_stage",
        "metrics_source",
    ] {
        if non_empty_string(entry, field).is_none() {
            push_issue(
                issues,
                "missing_evidence_metadata",
                &format!("evidence[{index}].{field}"),
                "evidence entry must include safe category metadata",
            );
        }
    }
    if entry.get("provenance").and_then(Value::as_object).is_none() {
        push_issue(
            issues,
            "missing_evidence_provenance",
            &format!("evidence[{index}].provenance"),
            "evidence entry must include provenance for review",
        );
    }
    if refs.insert(evidence_ref, entry).is_some() {
        push_issue(
            issues,
            "duplicate_evidence_ref",
            &format!("evidence[{index}].ref"),
            "evidence refs must be unique",
        );
    }
}

pub(super) fn validate_category_evidence_refs(
    registry: &Value,
    evidence_refs: &BTreeMap<&str, &Value>,
    issues: &mut Vec<SemanticReviewIssue>,
) {
    let Some(categories) = registry.get("categories").and_then(Value::as_array) else {
        return;
    };
    for (index, category) in categories.iter().enumerate() {
        let Some(evidence_ref) = non_empty_string(category, "evidence_ref") else {
            continue;
        };
        match evidence_refs.get(evidence_ref) {
            Some(entry) => validate_evidence_metadata_match(category, entry, index, issues),
            None => push_issue(
                issues,
                "missing_evidence_ref",
                &format!("categories[{index}].evidence_ref"),
                "registry evidence_ref must resolve to sanitized evidence",
            ),
        }
    }
}

fn validate_evidence_metadata_match(
    category: &Value,
    entry: &Value,
    index: usize,
    issues: &mut Vec<SemanticReviewIssue>,
) {
    for (category_field, evidence_field) in [
        ("code", "category_code"),
        ("metrics_class", "metrics_class"),
        ("metrics_stage", "metrics_stage"),
        ("metrics_source", "metrics_source"),
    ] {
        let category_value = non_empty_string(category, category_field);
        let evidence_value = non_empty_string(entry, evidence_field);
        if category_value.is_some() && evidence_value.is_some() && category_value != evidence_value
        {
            push_issue(
                issues,
                "evidence_metadata_mismatch",
                &format!("categories[{index}].evidence_ref"),
                "evidence metadata must match registry category bindings",
            );
        }
    }
}

pub(super) fn validate_secret_free_json(
    value: &Value,
    field_path: &str,
    issues: &mut Vec<SemanticReviewIssue>,
) {
    match value {
        Value::Object(map) => {
            for (key, nested) in map {
                let nested_path = format!("{field_path}.{key}");
                if unsafe_evidence_key(key) {
                    push_issue(
                        issues,
                        "unsafe_evidence_field",
                        &nested_path,
                        "evidence must not include secret-bearing or raw-private fields",
                    );
                }
                validate_secret_free_json(nested, &nested_path, issues);
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                validate_secret_free_json(item, &format!("{field_path}[{index}]"), issues);
            }
        }
        Value::String(text) => {
            if unsafe_evidence_text(text) {
                push_issue(
                    issues,
                    "unsafe_evidence_text",
                    field_path,
                    "evidence text must be sanitized before it is referenced",
                );
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn unsafe_evidence_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "api_key",
        "authorization",
        "cookie",
        "credential",
        "connection_string",
        "hidden_prompt",
        "password",
        "provider_payload",
        "raw_transcript",
        "request_body",
        "response_body",
        "secret",
        "system_prompt",
        "token",
    ]
    .iter()
    .any(|needle| key.contains(needle))
        || key == "account_id"
}

fn unsafe_evidence_text(text: &str) -> bool {
    let text = text.to_ascii_lowercase();
    [
        "api_key=",
        "authorization:",
        "bearer ",
        "connection_string=",
        "password=",
        "provider payload",
        "raw transcript",
        "secret=",
        "system prompt:",
        "token=",
    ]
    .iter()
    .any(|needle| text.contains(needle))
        || looks_like_email(&text)
}

fn looks_like_email(text: &str) -> bool {
    let Some(at_index) = text.find('@') else {
        return false;
    };
    let before = &text[..at_index];
    let after = &text[at_index + 1..];
    !before.is_empty() && after.contains('.') && after.chars().any(|ch| ch.is_ascii_alphabetic())
}
