use serde_json::{Map, Value};

use super::common::bundle_issue;
use super::constants::{EXPORT_BUNDLE_REQUIRED_FIELDS, EXPORT_BUNDLE_SCHEMA_V1};
use super::types::ExportBundleIssue;

pub(super) fn export_bundle_required_issues(object: &Map<String, Value>) -> Vec<ExportBundleIssue> {
    let mut issues = Vec::with_capacity(EXPORT_BUNDLE_REQUIRED_FIELDS.len() + 1);
    for field in EXPORT_BUNDLE_REQUIRED_FIELDS {
        if !object.contains_key(*field) {
            issues.push(bundle_issue(
                "export_bundle.missing_field",
                format!("missing required field: {field}"),
            ));
        }
    }
    if object.get("schema").and_then(Value::as_str) != Some(EXPORT_BUNDLE_SCHEMA_V1) {
        issues.push(bundle_issue(
            "export_bundle.unsupported_schema",
            "schema must be octet.export-bundle.v1",
        ));
    }
    issues
}
