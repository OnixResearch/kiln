// r[impl cairn.semantic_review.metrics_regression_split.focused_helpers]
use std::collections::BTreeSet;

use serde_json::Value;

use super::super::common::push_issue;
use super::super::{SemanticReviewIssue, REVIEW_CATEGORIES, REVIEW_SOURCES};

const REQUIRED_CATEGORY_FIELDS: &[&str] = &[
    "code",
    "metrics_class",
    "metrics_stage",
    "metrics_source",
    "diagnostic_code",
    "requirement_id",
    "guidance_anchor",
    "positive_fixture",
    "negative_fixture",
    "evidence_ref",
];

const METRICS_STAGES: &[&str] = &[
    "proposal", "design", "tasks", "spec", "plan", "code", "diff",
];

pub(super) fn validate_categories(value: &Value, issues: &mut Vec<SemanticReviewIssue>) {
    let Some(categories) = value.get("categories").and_then(Value::as_array) else {
        push_issue(
            issues,
            "missing_registry_categories",
            "categories",
            "registry must declare at least one category",
        );
        return;
    };
    if categories.is_empty() {
        push_issue(
            issues,
            "empty_registry_categories",
            "categories",
            "registry must declare at least one category",
        );
        return;
    }

    let mut codes = BTreeSet::new();
    let mut diagnostics = BTreeSet::new();
    let mut requirement_ids = BTreeSet::new();
    for (index, category) in categories.iter().enumerate() {
        validate_category_fields(category, index, issues);
        track_unique(category, index, "code", &mut codes, issues);
        track_unique(category, index, "diagnostic_code", &mut diagnostics, issues);
        track_unique(
            category,
            index,
            "requirement_id",
            &mut requirement_ids,
            issues,
        );
    }
}

fn validate_category_fields(category: &Value, index: usize, issues: &mut Vec<SemanticReviewIssue>) {
    if !category.is_object() {
        push_issue(
            issues,
            "invalid_registry_category",
            &format!("categories[{index}]"),
            "category entry must be an object",
        );
        return;
    }
    for field in REQUIRED_CATEGORY_FIELDS {
        match category.get(*field).and_then(Value::as_str) {
            Some(text) if !text.trim().is_empty() => {}
            Some(_) | None => push_issue(
                issues,
                "missing_registry_binding",
                &format!("categories[{index}].{field}"),
                "category registry entry is missing a required non-empty binding",
            ),
        }
    }
    validate_allowed(category, index, "metrics_class", REVIEW_CATEGORIES, issues);
    validate_allowed(category, index, "metrics_stage", METRICS_STAGES, issues);
    validate_allowed(category, index, "metrics_source", REVIEW_SOURCES, issues);
}

fn validate_allowed(
    category: &Value,
    index: usize,
    field: &str,
    allowed: &[&str],
    issues: &mut Vec<SemanticReviewIssue>,
) {
    if let Some(actual) = category.get(field).and_then(Value::as_str) {
        if !actual.trim().is_empty() && !allowed.contains(&actual) {
            push_issue(
                issues,
                "invalid_registry_binding",
                &format!("categories[{index}].{field}"),
                "category registry binding is not policy-known",
            );
        }
    }
}

fn track_unique(
    category: &Value,
    index: usize,
    field: &str,
    seen: &mut BTreeSet<String>,
    issues: &mut Vec<SemanticReviewIssue>,
) {
    if let Some(actual) = category.get(field).and_then(Value::as_str) {
        if actual.trim().is_empty() {
            return;
        }
        if !seen.insert(actual.to_string()) {
            push_issue(
                issues,
                "duplicate_registry_binding",
                &format!("categories[{index}].{field}"),
                "category registry binding must be unique",
            );
        }
    }
}
