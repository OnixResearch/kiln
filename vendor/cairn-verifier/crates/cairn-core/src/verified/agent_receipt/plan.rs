use serde_json::{Map, Value};

use crate::verified::nominal_identity::{InputDigestRole, PlanDigestRole};

use super::common::{is_digest, issue};
use super::constants::{NO_MUTATION_PLAN_HASH, SIDE_EFFECT_FIELDS};
use super::types::AgentRunGateIssue;

// r[impl cairn.supersede.agent_run_receipts.plan_link]
// r[impl cairn.supersede.agent_run_receipts.plan_link.side_effects]
// r[impl cairn.supersede.agent_run_receipts.plan_link.read_only]
pub(super) fn plan_linkage_issues(object: &Map<String, Value>) -> Vec<AgentRunGateIssue> {
    let input_hash = object
        .get("input_hash")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !is_digest::<InputDigestRole>(input_hash) {
        return vec![issue(
            "input.invalid_hash",
            "agent-run input_hash must be a BLAKE3 input digest",
        )];
    }
    let has_side_effects = object
        .get("mutation_manifest")
        .and_then(Value::as_object)
        .map(mutation_manifest_has_side_effects)
        .unwrap_or(false);
    let plan_hash = object
        .get("plan_hash")
        .and_then(Value::as_str)
        .unwrap_or("");
    if has_side_effects && plan_hash.is_empty() {
        return vec![issue(
            "plan.missing_hash",
            "side-effecting agent-run receipts require a non-empty plan_hash",
        )];
    }
    if has_side_effects && plan_hash == NO_MUTATION_PLAN_HASH {
        return vec![issue(
            "plan.no_mutation_plan_with_side_effects",
            "side-effecting agent-run receipts cannot cite the no-mutation plan hash",
        )];
    }
    if plan_hash != NO_MUTATION_PLAN_HASH && !is_digest::<PlanDigestRole>(plan_hash) {
        return vec![issue(
            "plan.invalid_hash",
            "agent-run plan_hash must be a BLAKE3 plan digest or the no-mutation sentinel",
        )];
    }
    Vec::new()
}

fn mutation_manifest_has_side_effects(manifest: &Map<String, Value>) -> bool {
    SIDE_EFFECT_FIELDS
        .iter()
        .any(|field| manifest.get(*field).map(value_has_entries).unwrap_or(false))
}

fn value_has_entries(value: &Value) -> bool {
    match value {
        Value::Array(entries) => !entries.is_empty(),
        Value::Object(entries) => !entries.is_empty(),
        Value::String(text) => !text.is_empty(),
        Value::Bool(flag) => *flag,
        Value::Number(_) => true,
        Value::Null => false,
    }
}
