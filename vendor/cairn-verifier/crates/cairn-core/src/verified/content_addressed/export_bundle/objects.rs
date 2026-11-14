use serde_json::{Map, Value};

use super::super::common::is_blake3_hex;
use super::common::bundle_issue;
use super::types::ExportBundleIssue;

pub(super) fn export_bundle_object_issues(
    object: &Map<String, Value>,
) -> (Vec<String>, Vec<ExportBundleIssue>) {
    let Some(objects) = object.get("objects").and_then(Value::as_array) else {
        return (
            Vec::new(),
            vec![bundle_issue(
                "export_bundle.invalid_objects",
                "objects must be an array",
            )],
        );
    };
    let mut object_hashes = Vec::with_capacity(objects.len());
    let mut issues = Vec::with_capacity(objects.len() + 1);
    let mut previous_hash = None::<String>;
    for entry in objects {
        let Some(object_entry) = entry.as_object() else {
            issues.push(bundle_issue(
                "export_bundle.invalid_object_entry",
                "each object membership entry must be a JSON object",
            ));
            continue;
        };
        let object_hash = object_entry
            .get("object_hash")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !is_blake3_hex(object_hash) {
            issues.push(bundle_issue(
                "export_bundle.invalid_object_hash",
                "objects.object_hash must be a BLAKE3 hex string",
            ));
        }
        if previous_hash
            .as_deref()
            .is_some_and(|previous| previous > object_hash)
        {
            issues.push(bundle_issue(
                "export_bundle.unsorted_objects",
                "objects must be sorted by object_hash",
            ));
        }
        previous_hash = Some(object_hash.to_string());
        object_hashes.push(object_hash.to_string());
    }
    (object_hashes, issues)
}
