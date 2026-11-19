// r[impl cairn.supersede.agent_run_receipts.plan_link]
// r[impl cairn.supersede.agent_run_receipts.plan_link.side_effects]
// r[impl cairn.supersede.agent_run_receipts.plan_link.read_only]
pub(super) fn plan_linkage_issues(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Vec<super::types::AgentRunGateIssue> {
    let input_hash = object
        .get("input_hash")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if !super::common::is_digest::<crate::verified::nominal_identity::InputDigestRole>(input_hash) {
        return vec![super::common::issue(
            "input.invalid_hash",
            "agent-run input_hash must be a BLAKE3 input digest",
        )];
    }
    let has_side_effects = object
        .get("mutation_manifest")
        .and_then(serde_json::Value::as_object)
        .map(mutation_manifest_has_side_effects)
        .unwrap_or(false);
    let plan_hash = object
        .get("plan_hash")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    if has_side_effects && plan_hash.is_empty() {
        return vec![super::common::issue(
            "plan.missing_hash",
            "side-effecting agent-run receipts require a non-empty plan_hash",
        )];
    }
    if has_side_effects && plan_hash == super::constants::NO_MUTATION_PLAN_HASH {
        return vec![super::common::issue(
            "plan.no_mutation_plan_with_side_effects",
            "side-effecting agent-run receipts cannot cite the no-mutation plan hash",
        )];
    }
    if plan_hash != super::constants::NO_MUTATION_PLAN_HASH
        && !super::common::is_digest::<crate::verified::nominal_identity::PlanDigestRole>(plan_hash)
    {
        return vec![super::common::issue(
            "plan.invalid_hash",
            "agent-run plan_hash must be a BLAKE3 plan digest or the no-mutation sentinel",
        )];
    }
    Vec::new()
}

fn mutation_manifest_has_side_effects(
    manifest: &serde_json::Map<String, serde_json::Value>,
) -> bool {
    super::constants::SIDE_EFFECT_FIELDS
        .iter()
        .any(|field| manifest.get(*field).map(value_has_entries).unwrap_or(false))
}

fn value_has_entries(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Array(entries) => !entries.is_empty(),
        serde_json::Value::Object(entries) => !entries.is_empty(),
        serde_json::Value::String(text) => !text.is_empty(),
        serde_json::Value::Bool(flag) => *flag,
        serde_json::Value::Number(_) => true,
        serde_json::Value::Null => false,
    }
}
