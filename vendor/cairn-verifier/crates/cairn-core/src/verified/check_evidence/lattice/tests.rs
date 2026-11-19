use super::*;
use serde_json::json;
#[test]
fn unchanged_published_producer_fixture_and_its_controls_are_admitted() {
    let c: Value = serde_json::from_str(include_str!(
        "../../../../../../fixtures/check-evidence/lattice-producer-fixtures.json"
    ))
    .expect("published fixture");
    let b = json!({"profile":c["profile"],"request":c["request"],"accepted":c["accepted"],"observation":c["observation"]});
    assert!(admit_lattice_json(&b).is_ok());
    for (name, field) in [
        ("cyclic_request", "request"),
        ("false_paused_success", "observation"),
        ("stale_execution", "observation"),
    ] {
        let mut bad = b.clone();
        bad[field] = c["rejected"][name].clone();
        assert!(admit_lattice_json(&bad).is_err());
    }
}
fn cohort() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../../fixtures/check-evidence/lattice-cohort.json"
    ))
    .expect("native generated cohort")
}
// Rebind with the consumer's protocol implementation so semantic negatives are
// not merely stale digest controls. The checked-in native cohort independently
// exercises the same framing through the original pinned producer library.
fn rebind(value: &mut Value) {
    let mut b: Bundle = serde_json::from_value(value.clone()).expect("typed fixture");
    b.request.profile_identity = identity::profile(&b.profile);
    b.request.graph.identity = identity::graph(&b.request.graph);
    b.request.identity = identity::request(&b.request);
    b.accepted.request_identity = b.request.identity.clone();
    b.observation.request_identity = b.request.identity.clone();
    b.observation.identity = identity::observation(&b.observation);
    *value = serde_json::to_value(b).expect("serialized fixture");
}
// r[verify cairn.check_evidence_producer.adapters]
// r[verify cairn.check_evidence_producer.conformance]
#[test]
fn original_producer_positive_skipped_and_negative_cohorts_agree() {
    let c = cohort();
    for name in ["positive", "skipped"] {
        let a = admit_lattice_json(&c[name]).expect("native admitted cohort");
        assert_eq!(a.native_status, "completed");
        assert!(!a.completion_eligible);
        assert_eq!(a.native_non_claims, NON_CLAIMS);
    }
    assert_eq!(
        admit_lattice_json(&c["skipped"])
            .expect("skipped")
            .native_node_states,
        vec!["skipped"]
    );
    for value in c["negative"].as_array().expect("native negatives") {
        assert!(admit_lattice_json(value).is_err());
    }
}
#[test]
fn request_profile_identity_and_order_substitutions_fail_closed() {
    for (pointer, value) in [
        ("/request/schema", json!("substituted")),
        ("/request/platform", json!("other-platform")),
        ("/profile/non_claims", json!([])),
        ("/profile/controls/cancel", json!(false)),
        ("/profile/limits/maximum_nodes", json!(0)),
        (
            "/profile/platforms",
            json!(["linux-x86_64", "linux-x86_64"]),
        ),
        ("/request/graph/nodes/0/kind", json!("unavailable")),
        ("/request/graph/nodes/0/effect_class", json!("unavailable")),
        (
            "/request/graph/nodes/0/operation_identity",
            json!("secret-identity"),
        ),
        ("/request/source/repository", json!("secret\nrepository")),
        ("/request/observation/maximum_log_read_bytes", json!(4097)),
        ("/observation/nodes/0/state", json!("pending")),
        ("/observation/logs/0/stream", json!("unknown")),
        ("/observation/logs/0/node_id", json!("unknown")),
    ] {
        let mut b = cohort()["positive"].clone();
        *b.pointer_mut(pointer).expect("pointer") = value;
        rebind(&mut b);
        assert!(admit_lattice_json(&b).is_err(), "admitted {pointer}");
    }
    for pointer in [
        "/request/identity",
        "/request/profile_identity",
        "/request/graph/identity",
        "/accepted/execution_identity",
        "/observation/identity",
    ] {
        let mut b = cohort()["positive"].clone();
        *b.pointer_mut(pointer).expect("pointer") =
            json!(format!("b3:{}", "f".repeat(DIGEST_DIGITS)));
        assert!(admit_lattice_json(&b).is_err(), "admitted {pointer}");
    }
    let mut cyclic = cohort()["positive"].clone();
    cyclic["request"]["graph"]["edges"] = json!([{"source":"build","target":"build"}]);
    rebind(&mut cyclic);
    assert!(admit_lattice_json(&cyclic).is_err());
    let mut duplicate = cohort()["positive"].clone();
    let log = duplicate["observation"]["logs"][0].clone();
    duplicate["observation"]["logs"]
        .as_array_mut()
        .expect("logs")
        .push(log);
    rebind(&mut duplicate);
    assert!(admit_lattice_json(&duplicate).is_err());
}
#[test]
fn queued_running_and_unknown_native_states_are_never_executed_tests() {
    for state in ["accepted", "queued", "running", "paused", "unknown"] {
        let mut b = cohort()["positive"].clone();
        b["observation"]["state"] = json!(state);
        b["observation"]["terminal"] = json!(false);
        b["observation"]["successful"] = json!(false);
        b["observation"]["receipt_identity"] = Value::Null;
        rebind(&mut b);
        let admitted = admit_lattice_json(&b).expect("native state");
        assert_eq!(admitted.native_status, state);
        assert!(!admitted.completion_eligible);
    }
}
