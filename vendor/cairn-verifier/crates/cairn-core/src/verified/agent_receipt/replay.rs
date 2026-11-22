// r[impl cairn.supersede.agent_receipt_replay_evidence.schema]
pub(super) fn replay_evidence_issues(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Vec<super::types::AgentRunGateIssue> {
    let Some(evidence) = object
        .get("replay_evidence")
        .and_then(serde_json::Value::as_object)
    else {
        return Vec::new();
    };
    let mut issues = Vec::new();
    let replay_hash = evidence
        .get("replay_receipt_hash")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if !super::common::is_digest::<crate::verified::nominal_identity::ReceiptDigestRole>(
        replay_hash,
    ) {
        issues.push(super::common::issue(
            "replay_evidence.invalid_hash",
            "replay_evidence.replay_receipt_hash must be a BLAKE3 hex hash",
        ));
    }
    let policy_hash = evidence
        .get("replay_policy_hash")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if !super::common::is_digest::<crate::verified::nominal_identity::PolicyDigestRole>(policy_hash)
    {
        issues.push(super::common::issue(
            "replay_evidence.invalid_policy_hash",
            "replay_evidence.replay_policy_hash must be a BLAKE3 hex hash",
        ));
    }
    if evidence
        .get("replay_verdict")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        != "pass"
    {
        issues.push(super::common::issue(
            "replay_evidence.verdict_not_pass",
            "replay_evidence.replay_verdict must be pass",
        ));
    }
    if let Some(attestation_hash) = evidence.get("replay_environment_attestation_hash") {
        if !attestation_hash.as_str().is_some_and(
            super::common::is_digest::<crate::verified::nominal_identity::ArtifactDigestRole>,
        ) {
            issues.push(super::common::issue(
                "replay_evidence.invalid_environment_attestation_hash",
                "replay_evidence.replay_environment_attestation_hash must be a BLAKE3 hex hash",
            ));
        }
    }
    let has_cases = evidence
        .get("replay_selected_cases")
        .and_then(serde_json::Value::as_array)
        .map(|cases| !cases.is_empty() && cases.iter().all(serde_json::Value::is_string))
        .unwrap_or(false);
    if !has_cases {
        issues.push(super::common::issue(
            "replay_evidence.missing_cases",
            "replay_evidence.replay_selected_cases must list replay case ids",
        ));
    }
    let has_groups = evidence
        .get("replay_selected_groups")
        .and_then(serde_json::Value::as_array)
        .map(|groups| !groups.is_empty() && groups.iter().all(serde_json::Value::is_string))
        .unwrap_or(false);
    if !has_groups {
        issues.push(super::common::issue(
            "replay_evidence.missing_groups",
            "replay_evidence.replay_selected_groups must list replay group ids",
        ));
    }
    issues
}
