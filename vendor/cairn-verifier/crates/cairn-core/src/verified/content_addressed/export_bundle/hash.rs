pub fn export_bundle_hash(receipt: &serde_json::Value) -> String {
    crate::verified::evidence::canonical_json_hash(&export_bundle_hash_material(receipt))
}

pub fn export_bundle_hash_material(receipt: &serde_json::Value) -> serde_json::Value {
    let mut material = receipt.clone();
    super::super::common::strip_non_hash_fields(&mut material);
    material
}

pub(super) fn export_bundle_hash_issues(
    object: &serde_json::Map<String, serde_json::Value>,
    computed_bundle_hash: &str,
) -> Vec<super::types::ExportBundleIssue> {
    let stated_hash = object
        .get("bundle_hash")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if stated_hash == computed_bundle_hash && super::super::common::is_blake3_hex(stated_hash) {
        return Vec::new();
    }
    vec![super::common::bundle_issue(
        "export_bundle.hash_mismatch",
        "bundle_hash must equal the BLAKE3 hash of canonical export-bundle content with non-hash fields excluded",
    )]
}
