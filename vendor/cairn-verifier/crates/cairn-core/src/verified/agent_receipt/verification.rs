use serde_json::{Map, Value};

use super::common::issue;
use super::types::AgentRunGateIssue;

// r[impl cairn.supersede.agent_run_receipts.completion_gate.verified]
// r[impl cairn.supersede.agent_run_receipts.completion_gate.missing_verification]
// r[impl cairn.supersede.agent_run_receipts.schema_migration]
// r[impl cairn.supersede.agent_run_receipts.schema_migration.legacy]
// r[impl cairn.supersede.agent_run_receipts.verification_contract.self_authored]
pub(super) fn verification_issues(object: &Map<String, Value>) -> Vec<AgentRunGateIssue> {
    let verdict = object.get("verdict").and_then(Value::as_str).unwrap_or("");
    if verdict != "success" && verdict != "PASS" {
        return Vec::new();
    }
    vec![issue(
        "verification.missing_contract",
        "legacy producer-authored verification_manifest rows are inspectable but cannot establish authoritative completion without a repository-owned v2 verification contract",
    )]
}
