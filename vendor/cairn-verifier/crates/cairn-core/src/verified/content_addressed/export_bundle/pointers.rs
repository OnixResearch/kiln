pub(super) fn export_bundle_pointer_issues(
    object: &serde_json::Map<String, serde_json::Value>,
    object_hashes: &[String],
) -> Vec<super::types::ExportBundleIssue> {
    let Some(pointers) = object
        .get("name_pointers")
        .and_then(serde_json::Value::as_array)
    else {
        return vec![super::common::bundle_issue(
            "export_bundle.invalid_name_pointers",
            "name_pointers must be an array",
        )];
    };
    let mut issues = Vec::with_capacity(pointers.len() + 1);
    let mut previous_name = None::<String>;
    for entry in pointers {
        let Some(pointer) = entry.as_object() else {
            issues.push(super::common::bundle_issue(
                "export_bundle.invalid_pointer_entry",
                "each name pointer entry must be a JSON object",
            ));
            continue;
        };
        let name = pointer
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let object_hash = pointer
            .get("object_hash")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if name.is_empty() {
            issues.push(super::common::bundle_issue(
                "export_bundle.invalid_pointer_name",
                "name pointer entries must include a non-empty name",
            ));
        }
        if previous_name
            .as_deref()
            .is_some_and(|previous| previous > name)
        {
            issues.push(super::common::bundle_issue(
                "export_bundle.unsorted_name_pointers",
                "name_pointers must be sorted by name",
            ));
        }
        if !object_hashes
            .iter()
            .any(|known_hash| known_hash == object_hash)
        {
            issues.push(super::common::bundle_issue(
                "export_bundle.unknown_pointer_object",
                "name pointer object_hash must reference an object present in the bundle",
            ));
        }
        previous_name = Some(name.to_string());
    }
    issues
}
