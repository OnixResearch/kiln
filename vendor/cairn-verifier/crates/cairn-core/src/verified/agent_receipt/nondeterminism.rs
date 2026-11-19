// r[impl cairn.supersede.agent_run_receipts.nondeterminism]
// r[impl cairn.supersede.agent_run_receipts.nondeterminism.captured]
// r[impl cairn.supersede.agent_run_receipts.nondeterminism.hidden]
pub(super) fn nondeterminism_issues(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Vec<super::types::AgentRunGateIssue> {
    let Some(manifest) = object
        .get("nondeterminism_manifest")
        .and_then(serde_json::Value::as_object)
    else {
        return Vec::new();
    };
    let captured = manifest
        .get("captured")
        .and_then(serde_json::Value::as_array)
        .map(|entries| entries.as_slice())
        .unwrap_or(&[]);
    let forbidden = manifest
        .get("forbidden_uncaptured")
        .and_then(serde_json::Value::as_array)
        .map(|entries| entries.as_slice())
        .unwrap_or(&[]);
    let mut issues = Vec::with_capacity(captured.len().saturating_add(forbidden.len()));
    for entry in captured {
        if !captured_entropy_is_valid(entry) {
            issues.push(super::common::issue(
                "nondeterminism.invalid_capture",
                "captured entropy entries require a known class and bounded metadata or content_hash",
            ));
        }
    }
    for entry in forbidden {
        let class = entry
            .get("class")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown");
        issues.push(super::common::issue(
            "nondeterminism.uncaptured",
            format!("forbidden or uncaptured entropy source: {class}"),
        ));
    }
    issues
}

fn captured_entropy_is_valid(entry: &serde_json::Value) -> bool {
    let Some(class) = entry.get("class").and_then(serde_json::Value::as_str) else {
        return false;
    };
    super::constants::KNOWN_ENTROPY_CLASSES.contains(&class)
        && (entry.get("metadata").is_some() || entry.get("content_hash").is_some())
}
