use crate::verified::change_dependencies::{
    DEPENDENCY_ISSUE_EMPTY_ID, DEPENDENCY_ISSUE_INVALID_ID, DEPENDENCY_ISSUE_INVALID_ITEM,
    DEPENDENCY_ISSUE_INVALID_TYPE,
};
use crate::verified::{
    cross_repo_dependencies::parse_cross_repo_dependencies, lifecycle::validate_change_name,
};

use super::{ChangeMetadata, ChangeStatus};

pub(super) fn parse_change_metadata(
    change_name: &str,
    metadata_json: Option<&str>,
) -> (ChangeMetadata, Vec<String>) {
    let Some(metadata_json) = metadata_json else {
        return (ChangeMetadata::default(), Vec::new());
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(metadata_json) else {
        return (
            ChangeMetadata::default(),
            vec![format!("{change_name}: metadata.json is not valid JSON")],
        );
    };
    let Some(object) = value.as_object() else {
        return (
            ChangeMetadata::default(),
            vec![format!(
                "{change_name}: metadata.json must contain an object"
            )],
        );
    };

    let mut issues = Vec::new();
    let groups = match object.get("groups") {
        None => Vec::new(),
        Some(value) => string_array(value).unwrap_or_else(|| {
            issues.push(format!(
                "{change_name}: metadata groups must be an array of strings"
            ));
            Vec::new()
        }),
    };
    let target_date = optional_string(object.get("target_date"), change_name, "target_date");
    issues.extend(target_date.1);
    let target_date = target_date.0;
    let status = optional_string(object.get("status"), change_name, "status");
    issues.extend(status.1);
    let status = status.0;
    let depends_on = parse_dependency_metadata(object.get("depends_on"), change_name, &mut issues);
    let (cross_repo_dependencies, cross_repo_issues) =
        parse_cross_repo_dependencies(object.get("cross_repo_dependencies"));
    issues.extend(
        cross_repo_issues
            .into_iter()
            .map(|issue| format!("{change_name}: {issue}")),
    );
    let mut sorted_groups = groups;
    sorted_groups.sort();
    let has_duplicates = sorted_groups
        .windows(2)
        .any(|window| window[0] == window[1]);
    if has_duplicates {
        issues.push(format!("{change_name}: metadata groups must be unique"));
        sorted_groups.dedup();
    }
    (
        ChangeMetadata {
            groups: sorted_groups,
            target_date,
            status,
            depends_on,
            cross_repo_dependencies,
        },
        issues,
    )
}

fn string_array(value: &serde_json::Value) -> Option<Vec<String>> {
    value
        .as_array()?
        .iter()
        .map(|item| item.as_str().map(str::to_string))
        .collect()
}

fn parse_dependency_metadata(
    value: Option<&serde_json::Value>,
    change_name: &str,
    issues: &mut Vec<String>,
) -> Vec<String> {
    let Some(value) = value else {
        return Vec::new();
    };
    let Some(items) = value.as_array() else {
        issues.push(format!(
            "{change_name}: {DEPENDENCY_ISSUE_INVALID_TYPE}: metadata depends_on must be an array of strings"
        ));
        return Vec::new();
    };
    let mut dependencies = Vec::with_capacity(items.len());
    for item in items {
        let Some(dependency) = item.as_str() else {
            issues.push(format!(
                "{change_name}: {DEPENDENCY_ISSUE_INVALID_ITEM}: metadata depends_on entries must be strings"
            ));
            continue;
        };
        if dependency.is_empty() {
            issues.push(format!(
                "{change_name}: {DEPENDENCY_ISSUE_EMPTY_ID}: dependency id must not be empty"
            ));
            continue;
        }
        if let Err(error) = validate_change_name(dependency) {
            issues.push(format!(
                "{change_name}: {DEPENDENCY_ISSUE_INVALID_ID}: invalid dependency id {dependency}: {error}"
            ));
            continue;
        }
        dependencies.push(dependency.to_string());
    }
    dependencies.sort();
    dependencies
}

fn optional_string(
    value: Option<&serde_json::Value>,
    change_name: &str,
    field: &str,
) -> (Option<String>, Option<String>) {
    match value {
        None => (None, None),
        Some(value) => match value.as_str() {
            Some(text) => (Some(text.to_string()), None),
            None => {
                let issue = format!("{change_name}: metadata {field} must be a string");
                (None, Some(issue))
            }
        },
    }
}

fn is_yyyy_mm_dd(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
}

pub fn validate_change_metadata_policy(
    change: &ChangeStatus,
    allowed_groups: &[String],
    group_prefixes: &[String],
    allowed_statuses: &[String],
) -> Vec<String> {
    // r[impl cairn.supersede.change_metadata.validation]
    let mut issues = change.metadata_issues.clone();
    for group in &change.metadata.groups {
        let is_allowed_group = allowed_groups.iter().any(|allowed| allowed == group)
            || group_prefixes
                .iter()
                .any(|prefix| group.starts_with(prefix));
        if !is_allowed_group {
            issues.push(format!(
                "{}: metadata group {group} is not allowed by policy",
                change.name
            ));
        }
    }
    if let Some(target_date) = &change.metadata.target_date {
        if !is_yyyy_mm_dd(target_date) {
            issues.push(format!(
                "{}: metadata target_date must be YYYY-MM-DD",
                change.name
            ));
        }
    }
    if let Some(status) = &change.metadata.status {
        if !allowed_statuses.iter().any(|allowed| allowed == status) {
            issues.push(format!(
                "{}: metadata status {status} is not allowed by policy",
                change.name
            ));
        }
    }
    issues
}
