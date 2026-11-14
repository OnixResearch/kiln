use serde_json::{Map, Value};

use crate::verified::evidence::canonical_json_hash;

use super::super::common::{is_blake3_hex, strip_non_hash_fields};
use super::common::bundle_issue;
use super::types::ExportBundleIssue;

pub fn export_bundle_hash(receipt: &Value) -> String {
    canonical_json_hash(&export_bundle_hash_material(receipt))
}

pub fn export_bundle_hash_material(receipt: &Value) -> Value {
    let mut material = receipt.clone();
    strip_non_hash_fields(&mut material);
    material
}

pub(super) fn export_bundle_hash_issues(
    object: &Map<String, Value>,
    computed_bundle_hash: &str,
) -> Vec<ExportBundleIssue> {
    let stated_hash = object
        .get("bundle_hash")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if stated_hash == computed_bundle_hash && is_blake3_hex(stated_hash) {
        return Vec::new();
    }
    vec![bundle_issue(
        "export_bundle.hash_mismatch",
        "bundle_hash must equal the BLAKE3 hash of canonical export-bundle content with non-hash fields excluded",
    )]
}
