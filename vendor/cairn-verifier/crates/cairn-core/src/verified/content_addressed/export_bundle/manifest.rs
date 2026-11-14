use serde_json::{Map, Value};

use super::super::common::is_blake3_hex;
use super::common::bundle_issue;
use super::types::ExportBundleIssue;

pub(super) fn export_bundle_manifest_issues(object: &Map<String, Value>) -> Vec<ExportBundleIssue> {
    let Some(manifest) = object.get("mutation_manifest").and_then(Value::as_object) else {
        return vec![bundle_issue(
            "export_bundle.missing_object",
            "mutation_manifest must be a JSON object",
        )];
    };
    export_bundle_touched_file_issues(manifest)
}

fn export_bundle_touched_file_issues(manifest: &Map<String, Value>) -> Vec<ExportBundleIssue> {
    let Some(entries) = manifest.get("touched_files").and_then(Value::as_array) else {
        return vec![bundle_issue(
            "export_bundle.invalid_mutation_manifest",
            "mutation_manifest.touched_files must be an array",
        )];
    };
    let mut issues = Vec::with_capacity(entries.len() + 1);
    let mut previous_path = None::<&str>;
    for entry in entries {
        let Some(file) = entry.as_object() else {
            issues.push(bundle_issue(
                "export_bundle.invalid_mutation_manifest",
                "each touched file must be a JSON object",
            ));
            continue;
        };
        let path = file.get("path").and_then(Value::as_str).unwrap_or_default();
        if path.is_empty() || path.starts_with('/') || path.contains("..") {
            issues.push(bundle_issue(
                "export_bundle.invalid_mutation_manifest",
                "touched file paths must be non-empty safe relative paths",
            ));
        }
        if previous_path.is_some_and(|previous| previous > path) {
            issues.push(bundle_issue(
                "export_bundle.invalid_mutation_manifest",
                "mutation_manifest.touched_files must be sorted by path",
            ));
        }
        previous_path = Some(path);
        for hash_field in ["before_hash", "after_hash"] {
            if !file
                .get(hash_field)
                .and_then(Value::as_str)
                .is_some_and(is_blake3_hex)
            {
                issues.push(bundle_issue(
                    "export_bundle.invalid_mutation_manifest",
                    format!("touched_files.{hash_field} must be a BLAKE3 hex string"),
                ));
            }
        }
    }
    issues
}
