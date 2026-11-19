pub(super) fn export_bundle_required_issues(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Vec<super::types::ExportBundleIssue> {
    let mut issues = Vec::with_capacity(super::constants::EXPORT_BUNDLE_REQUIRED_FIELDS.len() + 1);
    for field in super::constants::EXPORT_BUNDLE_REQUIRED_FIELDS {
        if !object.contains_key(*field) {
            issues.push(super::common::bundle_issue(
                "export_bundle.missing_field",
                format!("missing required field: {field}"),
            ));
        }
    }
    if object.get("schema").and_then(serde_json::Value::as_str)
        != Some(super::constants::EXPORT_BUNDLE_SCHEMA_V1)
    {
        issues.push(super::common::bundle_issue(
            "export_bundle.unsupported_schema",
            "schema must be octet.export-bundle.v1",
        ));
    }
    issues
}
