use serde_json::{Map, Value};

use super::common::issue;
use super::constants::REQUIRED_RECEIPT_FIELDS;
use super::types::AgentRunGateIssue;

// r[impl cairn.supersede.agent_run_receipts.schema.required_fields]
pub(super) fn required_field_issues(object: &Map<String, Value>) -> Vec<AgentRunGateIssue> {
    REQUIRED_RECEIPT_FIELDS
        .iter()
        .filter(|field| !object.contains_key(**field))
        .map(|field| {
            issue(
                "receipt.missing_field",
                format!("missing required field: {field}"),
            )
        })
        .collect()
}
