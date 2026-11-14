use serde_json::{Map, Value};

use super::common::bundle_issue;
use super::constants::EXPORT_BUNDLE_REPLAY_REQUIRED_FIELDS;
use super::types::ExportBundleIssue;

pub(super) fn export_bundle_replay_issues(object: &Map<String, Value>) -> Vec<ExportBundleIssue> {
    let Some(replay) = object.get("replay").and_then(Value::as_object) else {
        return vec![bundle_issue(
            "export_bundle.missing_object",
            "replay must be a JSON object",
        )];
    };
    let mut issues = Vec::with_capacity(EXPORT_BUNDLE_REPLAY_REQUIRED_FIELDS.len() + 2);
    for field in EXPORT_BUNDLE_REPLAY_REQUIRED_FIELDS {
        if !replay.contains_key(*field) {
            issues.push(bundle_issue(
                "export_bundle.missing_field",
                format!("missing required field: replay.{field}"),
            ));
        }
    }
    if replay
        .get("command")
        .and_then(Value::as_str)
        .is_none_or(str::is_empty)
    {
        issues.push(bundle_issue(
            "export_bundle.invalid_replay_command",
            "replay.command must be a non-empty string",
        ));
    }
    if replay
        .get("args")
        .and_then(Value::as_array)
        .is_none_or(Vec::is_empty)
    {
        issues.push(bundle_issue(
            "export_bundle.invalid_replay_args",
            "replay.args must be a non-empty array",
        ));
    }
    issues
}
