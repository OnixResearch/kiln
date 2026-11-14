// r[impl cairn.semantic_review.metrics_regression_split.focused_helpers]
use std::collections::BTreeSet;

use serde_json::Value;

use super::super::common::push_issue;
use super::super::SemanticReviewIssue;
use super::common::non_empty_string;

pub(super) struct GuidanceSyncSurfaces {
    guidance_diagnostics: BTreeSet<String>,
    guidance_anchors: BTreeSet<String>,
    task_requirements: BTreeSet<String>,
    spec_requirements: BTreeSet<String>,
}

pub(super) fn guidance_sync_surfaces(
    sync_surfaces: &Value,
    issues: &mut Vec<SemanticReviewIssue>,
) -> GuidanceSyncSurfaces {
    let guidance_diagnostics = string_set_from_entries(
        sync_surfaces,
        "guidance",
        "diagnostics",
        "missing_guidance_sync",
        "operator guidance must declare diagnostics",
        issues,
    );
    let guidance_anchors = string_set_from_scalar_entries(
        sync_surfaces,
        "guidance",
        "anchor",
        "missing_guidance_anchor",
        "operator guidance must declare anchors",
        issues,
    );
    let task_requirements = string_set_from_entries(
        sync_surfaces,
        "tasks",
        "requirement_ids",
        "missing_task_sync",
        "task artifacts must declare requirement references",
        issues,
    );
    let spec_requirements = string_set_from_entries(
        sync_surfaces,
        "specs",
        "requirement_ids",
        "missing_spec_sync",
        "spec artifacts must declare requirement references",
        issues,
    );
    GuidanceSyncSurfaces {
        guidance_diagnostics,
        guidance_anchors,
        task_requirements,
        spec_requirements,
    }
}

pub(super) fn validate_guidance_sync(
    registry: &Value,
    sync: &GuidanceSyncSurfaces,
    issues: &mut Vec<SemanticReviewIssue>,
) {
    let Some(categories) = registry.get("categories").and_then(Value::as_array) else {
        return;
    };
    for (index, category) in categories.iter().enumerate() {
        validate_category_guidance_sync(
            category,
            index,
            &sync.guidance_diagnostics,
            &sync.guidance_anchors,
            &sync.task_requirements,
            &sync.spec_requirements,
            issues,
        );
    }
}

fn validate_category_guidance_sync(
    category: &Value,
    index: usize,
    guidance_diagnostics: &BTreeSet<String>,
    guidance_anchors: &BTreeSet<String>,
    task_requirements: &BTreeSet<String>,
    spec_requirements: &BTreeSet<String>,
    issues: &mut Vec<SemanticReviewIssue>,
) {
    if let Some(diagnostic_code) = non_empty_string(category, "diagnostic_code") {
        if !guidance_diagnostics.contains(diagnostic_code) {
            push_issue(
                issues,
                "guidance_missing_diagnostic",
                &format!("categories[{index}].diagnostic_code"),
                "operator guidance must document the category diagnostic",
            );
        }
    }
    if let Some(guidance_anchor) = non_empty_string(category, "guidance_anchor") {
        if !guidance_anchors.contains(guidance_anchor) {
            push_issue(
                issues,
                "guidance_missing_anchor",
                &format!("categories[{index}].guidance_anchor"),
                "operator guidance must expose the category guidance anchor",
            );
        }
    }
    if let Some(requirement_id) = non_empty_string(category, "requirement_id") {
        if !task_requirements.contains(requirement_id) {
            push_issue(
                issues,
                "task_missing_requirement_ref",
                &format!("categories[{index}].requirement_id"),
                "task artifacts must reference the category requirement id",
            );
        }
        if !spec_requirements.contains(requirement_id) {
            push_issue(
                issues,
                "spec_missing_requirement_ref",
                &format!("categories[{index}].requirement_id"),
                "spec artifacts must reference the category requirement id",
            );
        }
    }
}

fn string_set_from_entries(
    value: &Value,
    array_field: &str,
    values_field: &str,
    missing_code: &str,
    missing_message: &str,
    issues: &mut Vec<SemanticReviewIssue>,
) -> BTreeSet<String> {
    let mut values = BTreeSet::new();
    let Some(entries) = value.get(array_field).and_then(Value::as_array) else {
        push_issue(issues, missing_code, array_field, missing_message);
        return values;
    };
    if entries.is_empty() {
        push_issue(issues, missing_code, array_field, missing_message);
        return values;
    }
    for (entry_index, entry) in entries.iter().enumerate() {
        collect_entry_strings(
            entry,
            entry_index,
            array_field,
            values_field,
            &mut values,
            issues,
        );
    }
    values
}

fn string_set_from_scalar_entries(
    value: &Value,
    array_field: &str,
    scalar_field: &str,
    missing_code: &str,
    missing_message: &str,
    issues: &mut Vec<SemanticReviewIssue>,
) -> BTreeSet<String> {
    let mut values = BTreeSet::new();
    let Some(entries) = value.get(array_field).and_then(Value::as_array) else {
        push_issue(issues, missing_code, array_field, missing_message);
        return values;
    };
    if entries.is_empty() {
        push_issue(issues, missing_code, array_field, missing_message);
        return values;
    }
    for (entry_index, entry) in entries.iter().enumerate() {
        match entry.get(scalar_field).and_then(Value::as_str) {
            Some(text) if !text.trim().is_empty() => {
                values.insert(text.to_string());
            }
            Some(_) | None => push_issue(
                issues,
                "invalid_guidance_sync",
                &format!("{array_field}[{entry_index}].{scalar_field}"),
                "sync surface entry must declare a non-empty string binding",
            ),
        }
    }
    values
}

fn collect_entry_strings(
    entry: &Value,
    entry_index: usize,
    array_field: &str,
    values_field: &str,
    values: &mut BTreeSet<String>,
    issues: &mut Vec<SemanticReviewIssue>,
) {
    let Some(strings) = entry.get(values_field).and_then(Value::as_array) else {
        push_issue(
            issues,
            "invalid_guidance_sync",
            &format!("{array_field}[{entry_index}].{values_field}"),
            "sync surface entry must declare string references",
        );
        return;
    };
    if strings.is_empty() {
        push_issue(
            issues,
            "invalid_guidance_sync",
            &format!("{array_field}[{entry_index}].{values_field}"),
            "sync surface entry must declare at least one reference",
        );
        return;
    }
    for (value_index, value) in strings.iter().enumerate() {
        match value.as_str() {
            Some(text) if !text.trim().is_empty() => {
                values.insert(text.to_string());
            }
            Some(_) | None => push_issue(
                issues,
                "invalid_guidance_sync",
                &format!("{array_field}[{entry_index}].{values_field}[{value_index}]"),
                "sync surface references must be non-empty strings",
            ),
        }
    }
}
