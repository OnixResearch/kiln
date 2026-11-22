pub(super) fn export_bundle_entropy_issues(
    root_path: &str,
    root: &serde_json::Value,
) -> Vec<super::types::ExportBundleIssue> {
    super::super::common::entropy_hash_material_messages(root_path, root)
        .into_iter()
        .map(|message| {
            super::common::bundle_issue("export_bundle.ambient_entropy_in_hash_material", message)
        })
        .collect()
}
