use super::kiln::*;
use serde_json::{json, Value};
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../fixtures/check-evidence/kiln-workflow.json"
    ))
    .expect("native fixture")
}
// r[verify cairn.check_evidence_producer.adapters]
// r[verify cairn.check_evidence_producer.conformance]
#[test]
fn native_kiln_validator_preserves_success_skipped_and_pending_without_coverage() {
    let f = fixture();
    let mut o = f["observation"].clone();
    let admitted = admit_kiln_json(&f["plan"], &o).expect("native completed");
    assert!(!admitted.completion_eligible);
    assert_eq!(json!(admitted.native_outcome), "Success");
    o["nodes"][0]["state"] = json!("Skipped");
    o["nodes"][0]["receipt"] = Value::Null;
    assert!(
        !admit_kiln_json(&f["plan"], &o)
            .expect("native skipped")
            .completion_eligible
    );
    for (state, outcome) in [
        ("Accepted", "Pending"),
        ("Queued", "Pending"),
        ("Running", "Pending"),
        ("Paused", "Pending"),
        ("Unknown", "Unknown"),
    ] {
        o["state"] = json!(state);
        o["outcome"] = json!(outcome);
        o["terminal"] = json!(false);
        o["successful"] = json!(false);
        o["receipt"] = Value::Null;
        let a = admit_kiln_json(&f["plan"], &o).expect("native nonterminal");
        assert_eq!(json!(a.native_status), state);
        assert_eq!(json!(a.native_outcome), outcome);
        assert!(!a.completion_eligible);
    }
}
#[test]
fn kiln_substitution_and_native_boundaries_remain_blockers() {
    let f = fixture();
    for (pointer, value) in [
        ("/attempt_id", json!("foreign")),
        ("/candidate_id", json!("foreign")),
        ("/provider_profile", json!("foreign")),
        ("/workflow_policy_id", json!("foreign")),
        ("/outcome", json!("Pending")),
        ("/state", json!("Paused")),
        ("/receipt", Value::Null),
        ("/nodes", json!([])),
        ("/nodes/0/receipt", Value::Null),
        ("/nodes/0/adapter/contract_version", json!(2)),
        ("/workflow/native_id", json!("")),
    ] {
        let mut o = f["observation"].clone();
        *o.pointer_mut(pointer).expect("pointer") = value;
        assert!(
            admit_kiln_json(&f["plan"], &o).is_err(),
            "admitted {pointer}"
        );
    }
    let mut plan = f["plan"].clone();
    plan["bounds"]["max_nodes"] = json!(0);
    assert!(admit_kiln_json(&plan, &f["observation"]).is_err());
    plan = f["plan"].clone();
    plan["idempotency_key"] = json!("not-a-content-id");
    let mut o = f["observation"].clone();
    o["idempotency_key"] = plan["idempotency_key"].clone();
    assert!(admit_kiln_json(&plan, &o).is_err());
}
