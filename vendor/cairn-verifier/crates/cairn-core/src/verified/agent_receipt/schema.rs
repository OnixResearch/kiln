// r[impl cairn.supersede.agent_run_receipts.schema.required_fields]
pub(super) fn required_field_issues(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Vec<super::types::AgentRunGateIssue> {
    super::constants::REQUIRED_RECEIPT_FIELDS
        .iter()
        .filter(|field| !object.contains_key(**field))
        .map(|field| {
            super::common::issue(
                "receipt.missing_field",
                format!("missing required field: {field}"),
            )
        })
        .collect()
}
