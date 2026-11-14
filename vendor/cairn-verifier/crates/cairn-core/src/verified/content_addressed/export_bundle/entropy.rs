use serde_json::Value;

use super::super::common::entropy_hash_material_messages;
use super::common::bundle_issue;
use super::types::ExportBundleIssue;

pub(super) fn export_bundle_entropy_issues(
    root_path: &str,
    root: &Value,
) -> Vec<ExportBundleIssue> {
    entropy_hash_material_messages(root_path, root)
        .into_iter()
        .map(|message| bundle_issue("export_bundle.ambient_entropy_in_hash_material", message))
        .collect()
}
