// r[impl cairn.supersede.agent_run_receipts.completion_gate.verified]
// r[impl cairn.supersede.agent_run_receipts.completion_gate.missing_verification]
// r[impl cairn.supersede.agent_run_receipts.schema_migration]
// r[impl cairn.supersede.agent_run_receipts.schema_migration.legacy]
// r[impl cairn.supersede.agent_run_receipts.verification_contract.self_authored]
pub(super) fn verification_issues(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Vec<super::types::AgentRunGateIssue> {
    let verdict = object
        .get("verdict")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    if verdict != "success" && verdict != "PASS" {
        return Vec::new();
    }
    vec![super::common::issue(
        "verification.missing_contract",
        "legacy producer-authored verification_manifest rows are inspectable but cannot establish authoritative completion without a repository-owned v2 verification contract",
    )]
}
