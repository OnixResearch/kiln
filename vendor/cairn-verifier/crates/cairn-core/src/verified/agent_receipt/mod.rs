// r[impl cairn.nominal_evidence_identities.receipt_chains]
// r[verify cairn.nominal_evidence_identities.receipt_chains.validation]

use serde_json::Value;

mod authoritative;
mod common;
mod constants;
mod fixture;
mod hash;
mod nondeterminism;
mod plan;
mod replay;
mod schema;
mod types;
mod verification;

pub use authoritative::{
    authoritative_command_hash, authoritative_selection_hash,
    evaluate_authoritative_agent_completion, AgentArtifactReference,
    AgentAuthoritativeCompletionReport, AgentCompletionDiagnostic, AgentDiagnosticEvidence,
    AgentExecutionResult, AgentNonPassingCheck, AgentRequiredCheck, AgentSourceSpan,
    AgentVerificationContract,
};
pub use constants::{AGENT_RUN_RECEIPT_SCHEMA_VERSION, NO_MUTATION_PLAN_HASH};
pub use fixture::positive_agent_run_fixture;
pub use hash::{agent_run_preserves_receipt_hash, agent_run_receipt_hash};
pub use types::{AgentRunGateIssue, AgentRunGateReport};

use common::issue;
use hash::receipt_hash_issues;
use nondeterminism::nondeterminism_issues;
use plan::plan_linkage_issues;
use replay::replay_evidence_issues;
use schema::required_field_issues;
use verification::verification_issues;

// r[impl cli_maintainability.agent_receipt_core_modules.focused_modules]
// r[impl cli_maintainability.agent_receipt_core_modules.focused_modules.visible_boundaries]
// r[impl cli_maintainability.agent_receipt_core_modules.focused_modules.narrow_bridges]
// r[impl cli_maintainability.agent_receipt_core_modules.behavior_preserved]
// r[impl cli_maintainability.agent_receipt_core_modules.behavior_preserved.repository_checks]

// r[impl cairn.supersede.agent_run_receipts.completion_gate]
pub fn evaluate_agent_run_completion(receipt: &Value) -> AgentRunGateReport {
    let issues = agent_run_receipt_issues(receipt);
    let is_valid = issues.is_empty();
    let verdict = if is_valid { "PASS" } else { "FAIL" }.to_string();
    AgentRunGateReport {
        classification: "legacy_unscoped".to_string(),
        verdict,
        valid: is_valid,
        issues,
        receipt_hash: agent_run_receipt_hash(receipt),
    }
}

pub fn agent_run_receipt_issues(receipt: &Value) -> Vec<AgentRunGateIssue> {
    let mut issues = Vec::with_capacity(8);
    let Some(object) = receipt.as_object() else {
        return vec![issue(
            "receipt.not_object",
            "agent-run receipt must be a JSON object",
        )];
    };
    issues.extend(required_field_issues(object));
    issues.extend(receipt_hash_issues(receipt, object));
    issues.extend(plan_linkage_issues(object));
    issues.extend(verification_issues(object));
    issues.extend(nondeterminism_issues(object));
    issues.extend(replay_evidence_issues(object));
    issues
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use crate::verified::nominal_identity::ReceiptDigestRole;

    use super::common::is_digest;
    use super::*;

    #[test]
    fn canonical_agent_run_receipt_hash_is_byte_stable() {
        let first = positive_agent_run_fixture();
        let second = positive_agent_run_fixture();
        assert_eq!(first, second);
        assert_eq!(
            agent_run_receipt_hash(&first),
            agent_run_receipt_hash(&second)
        );
        let hash = first.get("receipt_hash").and_then(Value::as_str).unwrap();
        assert!(is_digest::<ReceiptDigestRole>(hash));
    }

    #[test]
    // r[verify cairn.determinism.preserves_receipt_consumer.golden_fixture]
    fn agent_run_preserves_hash_matches_positive_fixture_golden() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../../fixtures/agent-receipts/positive.json"
        ))
        .expect("checked-in positive agent-run fixture parses");
        let hash = agent_run_preserves_receipt_hash(&fixture)
            .expect("checked-in positive fixture Preserves hash");
        assert_eq!(
            hash,
            "f6b07870c0e4fdb8e56d88d6738792aed51a9b5c1368bf9e8c9fc0031e181d1b"
        );
    }

    #[test]
    // r[verify cairn.determinism.preserves_receipt_consumer.stable_hashing]
    fn agent_run_preserves_hash_ignores_object_key_order() {
        let left = json!({
            "schema_version": AGENT_RUN_RECEIPT_SCHEMA_VERSION,
            "task_id": "fixture-agent-run-success",
            "agent_identity": {"wrapper": "cairn-fixture", "model": "captured-model"},
            "receipt_hash": "ignored",
        });
        let right = json!({
            "receipt_hash": "ignored",
            "agent_identity": {"model": "captured-model", "wrapper": "cairn-fixture"},
            "task_id": "fixture-agent-run-success",
            "schema_version": AGENT_RUN_RECEIPT_SCHEMA_VERSION,
        });

        assert_eq!(
            agent_run_preserves_receipt_hash(&left).expect("left Preserves hash"),
            agent_run_preserves_receipt_hash(&right).expect("right Preserves hash")
        );
    }

    #[test]
    // r[verify cairn.determinism.preserves_receipt_consumer.stable_hashing]
    // r[verify cairn.determinism.preserves_receipt_consumer.validation]
    fn agent_run_preserves_hash_changes_when_fixture_content_changes() {
        let mut changed = positive_agent_run_fixture();
        changed["task_id"] = Value::String("different-task".to_string());

        assert_ne!(
            agent_run_preserves_receipt_hash(&positive_agent_run_fixture())
                .expect("positive fixture Preserves hash"),
            agent_run_preserves_receipt_hash(&changed).expect("changed fixture Preserves hash")
        );
    }

    #[test]
    // r[verify cairn.supersede.agent_run_receipts.schema_migration.legacy]
    // r[verify cairn.supersede.agent_run_receipts.replay.negative]
    // r[verify cli_maintainability.agent_receipt_core_modules.behavior_preserved.focused_checks]
    // r[verify cairn.supersede.agent_run_receipts.completion_authority.model_text]
    // r[verify cairn.supersede.agent_run_receipts.completion_authority.observations]
    fn legacy_success_fixture_is_inspectable_but_not_completion_authority() {
        let report = evaluate_agent_run_completion(&positive_agent_run_fixture());
        assert!(!report.valid);
        assert_eq!(report.classification, "legacy_unscoped");
        assert_eq!(report.verdict, "FAIL");
        assert_eq!(report.issues[0].code, "verification.missing_contract");
    }

    #[test]
    fn completion_gate_fails_missing_side_effect_plan() {
        let mut receipt = positive_agent_run_fixture();
        receipt["plan_hash"] = Value::String(String::new());
        let hash = agent_run_receipt_hash(&receipt);
        receipt["receipt_hash"] = Value::String(hash);
        let report = evaluate_agent_run_completion(&receipt);
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "plan.missing_hash"));
    }

    #[test]
    fn completion_gate_fails_uncaptured_nondeterminism() {
        let mut receipt = positive_agent_run_fixture();
        receipt["nondeterminism_manifest"] = json!({
            "captured": [],
            "forbidden_uncaptured": [{"class": "network"}]
        });
        let hash = agent_run_receipt_hash(&receipt);
        receipt["receipt_hash"] = Value::String(hash);
        let report = evaluate_agent_run_completion(&receipt);
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "nondeterminism.uncaptured"));
    }

    #[test]
    fn completion_gate_fails_mutation_outside_no_mutation_plan() {
        let mut receipt = positive_agent_run_fixture();
        receipt["plan_hash"] = Value::String(NO_MUTATION_PLAN_HASH.to_string());
        let hash = agent_run_receipt_hash(&receipt);
        receipt["receipt_hash"] = Value::String(hash);
        let report = evaluate_agent_run_completion(&receipt);
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "plan.no_mutation_plan_with_side_effects"));
    }
}
