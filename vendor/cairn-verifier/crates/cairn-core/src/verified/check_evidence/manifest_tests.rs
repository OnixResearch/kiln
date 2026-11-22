use super::*;
use crate::canonical_json_hash;
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn fixture() -> (CheckEvidencePlan, String, Value, BTreeMap<String, Vec<u8>>) {
    let (contract, profiles, applicability) = super::tests::fixture();
    let plan = admit_plan(&contract, &profiles, &applicability).unwrap();
    let mut observation = super::tests::observation(&plan);
    observation.producer_identity = blake3::hash(LOCAL_EXECUTOR_REVISION.as_bytes())
        .to_hex()
        .to_string();
    let attempt = observation.attempt_hash.clone();
    let facts = json!({"schema":"cairn.local-check-facts.v1", "producer_revision":LOCAL_EXECUTOR_REVISION, "attempt_hash":attempt,
        "plan_hash":plan.identity(), "check_id":"tests", "executed_tests":1, "payload_passed":true,
        "facts":{"outcome":"exited_zero","stdout_truncated":false,"stderr_truncated":false}});
    observation.evidence_hash = canonical_json_hash(&facts);
    let normalized = normalize_attempt(&plan, &observation).unwrap();
    let results = json!([normalized.result()]);
    let artifacts = BTreeMap::from([
        (
            "attempt.json".into(),
            serde_json::to_vec(
                &json!({"attempt_hash":attempt, "plan_hash":plan.identity(), "state":"started"}),
            )
            .unwrap(),
        ),
        (
            "observations.json".into(),
            serde_json::to_vec(&json!([observation])).unwrap(),
        ),
        ("results.json".into(), serde_json::to_vec(&results).unwrap()),
        (
            "tests.facts.json".into(),
            serde_json::to_vec(&facts).unwrap(),
        ),
    ]);
    let mut manifest = json!({"schema":"cairn.check-evidence-run.v1", "valid":true,"complete":true,
        "attempt_hash":attempt,"plan_hash":plan.identity(),"results":results,"blockers":[],
        "non_claims":NON_CLAIMS,"source_scope":"explicit_file_projection"});
    rehash(&mut manifest, &artifacts);
    (plan, attempt, manifest, artifacts)
}
fn rehash(manifest: &mut Value, artifacts: &BTreeMap<String, Vec<u8>>) {
    manifest["artifacts"] = json!(artifacts
        .iter()
        .map(|(leaf, bytes)| (leaf.clone(), blake3::hash(bytes).to_hex().to_string()))
        .collect::<BTreeMap<_, _>>());
}

#[test]
// r[verify cairn.check_evidence_producer.publication]
fn final_manifest_requires_exact_complete_artifacts_and_explicit_recovery() {
    let (plan, attempt, manifest, artifacts) = fixture();
    assert!(admit_manifest(&plan, &attempt, &manifest, &artifacts).is_ok());
    assert_eq!(recovery_decision(true), RecoveryDecision::Complete);
    assert_eq!(
        recovery_decision(false),
        RecoveryDecision::InspectAndUseNewOutputRoot
    );
    for (field, value) in [
        ("complete", json!(false)),
        ("valid", json!(false)),
        ("schema", json!("unknown")),
        ("attempt_hash", json!("b".repeat(64))),
        ("plan_hash", json!("c".repeat(64))),
        ("non_claims", json!([])),
        ("blockers", json!(["interrupted"])),
        ("results", json!([])),
    ] {
        let mut changed = manifest.clone();
        changed[field] = value;
        assert!(
            admit_manifest(&plan, &attempt, &changed, &artifacts).is_err(),
            "admitted {field}"
        );
    }
    for leaf in artifacts.keys() {
        let mut changed = artifacts.clone();
        changed.remove(leaf);
        assert!(admit_manifest(&plan, &attempt, &manifest, &changed).is_err());
        let mut changed = artifacts.clone();
        changed.insert(leaf.clone(), b"[]".to_vec());
        assert!(admit_manifest(&plan, &attempt, &manifest, &changed).is_err());
    }
}

#[test]
fn rehashed_nonpassing_observations_cannot_hide_behind_passing_rows() {
    let (plan, attempt, manifest, artifacts) = fixture();
    for (field, value) in [
        ("payload_passed", json!(false)),
        ("executed_tests", json!(0)),
        ("outcome", json!("timed_out")),
        ("teardown_complete", json!(false)),
        ("native_status", json!("skipped")),
        ("cache_claimed", json!(true)),
        ("execution_observed", json!(false)),
        ("role", json!("build")),
        ("applicability", json!({})),
        ("tool_cohort_hash", json!("c".repeat(64))),
    ] {
        let mut changed = artifacts.clone();
        let mut receipt = manifest.clone();
        let mut observations: Value =
            serde_json::from_slice(&changed["observations.json"]).unwrap();
        observations[0][field] = value;
        receipt["results"][0]["execution_receipt_hash"] = json!(canonical_json_hash(
            &json!({"schema":"cairn.check-observation.v1","observation":observations[0],"non_claims":NON_CLAIMS})
        ));
        changed.insert(
            "observations.json".into(),
            serde_json::to_vec(&observations).unwrap(),
        );
        changed.insert(
            "results.json".into(),
            serde_json::to_vec(&receipt["results"]).unwrap(),
        );
        rehash(&mut receipt, &changed);
        assert!(
            admit_manifest(&plan, &attempt, &receipt, &changed).is_err(),
            "admitted {field}"
        );
    }
}
