use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;
use std::sync::atomic::AtomicBool;

use kiln_core::gating::{CandidateChange, CandidateSet};
use kiln_core::provider::{CapabilityProfile, Operation, ProviderLimits, ProviderRole};
use kiln_core::review::{ReviewDecision, ReviewEffectPlan};
use kiln_core::types::{
    CandidateId, ContentId, EffectId, ExternalRef, PipelineRevisionId, PolicyId, ProviderProfileId,
};
use kiln_provider_api::PortFailure;
use serde_json::{Value, json};

use super::*;

const PROCESS_TEST_DELAY_MS: u64 = 30;
const WAITING_PROCESS: &str = "while :; do :; done & wait";

macro_rules! json_field {
    ($value:expr; $($key:literal),+) => {
        fixture_json_field(&mut $value, concat!($("/", $key),+))
    };
}

fn fixture_json_field<'a>(document: &'a mut Value, pointer: &str) -> &'a mut Value {
    let (parent, key) = pointer
        .rsplit_once('/')
        .unwrap_or_else(|| unreachable!("fixture pointer"));
    let object = document
        .pointer_mut(parent)
        .and_then(Value::as_object_mut)
        .unwrap_or_else(|| unreachable!("fixture JSON object"));
    if !object.contains_key(key) {
        object.insert(key.to_string(), Value::Null);
    }
    object
        .get_mut(key)
        .unwrap_or_else(|| unreachable!("fixture JSON field"))
}

fn must<T, E: Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| unreachable!("fixture operation failed: {error:?}"))
}

fn identity(label: &str) -> ContentId {
    must(kiln_codec::identity_from_bytes(label.as_bytes()))
}

fn hash(letter: char) -> String {
    std::iter::repeat_n(letter, 53).collect()
}

fn fixture() -> (PatchgateProfile, ReviewEffectPlan, Value) {
    let limits = PatchgateLimits {
        max_changes: 8,
        max_edges: 16,
        max_request_bytes: 65_536,
        max_report_bytes: 65_536,
        max_stderr_bytes: 1024,
        max_executable_bytes: 1024 * 1024,
        timeout_ms: 2000,
        poll_interval_ms: 5,
        teardown_timeout_ms: 1000,
    };
    let provider_id = must(ProviderProfileId::new("patchgate-test"));
    let profile = PatchgateProfile {
        source_revision: PINNED_SOURCE_REVISION.to_string(),
        report_schema: NATIVE_REPORT_SCHEMA,
        program: "/unavailable/patchgate".into(),
        executable_identity: identity("executable"),
        repository_root: "/unavailable/repository".into(),
        environment: BTreeMap::new(),
        provider: CapabilityProfile {
            contract_version: 1,
            profile_id: provider_id.clone(),
            role: ProviderRole::Collaboration,
            namespace: must(ExternalRef::new("patchgate", "review", "isolated")),
            operations: BTreeSet::from([Operation::Observe, Operation::Reconcile]),
            outcomes: BTreeSet::new(),
            limits: ProviderLimits {
                max_request_bytes: limits.max_request_bytes,
                max_result_bytes: limits.max_report_bytes,
                max_in_flight: 1,
            },
        },
        native: NativeBinding {
            main: MainBinding {
                channel: "main".to_string(),
                state: Some(hash('M')),
            },
            inbox: MainBinding {
                channel: "inbox".to_string(),
                state: Some(hash('I')),
            },
            policy: Some(PolicyBinding {
                path: "POLICY.md".to_string(),
                sha256: "a".repeat(64),
            }),
            configuration: ConfigurationEvidence {
                source: "environment".to_string(),
                path: None,
                sha256: None,
                values: ConfigurationValues {
                    policy_path: "POLICY.md".to_string(),
                    protected: vec!["src/protected/**".to_string()],
                    documentation_paths: vec!["docs/**".to_string()],
                    max_change_chars: 8000,
                    timeout_seconds: 300.0,
                    check_timeout_seconds: 900.0,
                    max_review_requests: 128,
                    review_timeout_seconds: 900.0,
                },
            },
        },
        limits,
    };
    let pipeline = must(PipelineRevisionId::new("pipeline"));
    let change = must(ExternalRef::new("pijul", "change", &hash('A')));
    let snapshot = identity("frozen-source");
    let candidate = CandidateSet {
        id: must(CandidateId::new("candidate")),
        ordered_changes: vec![CandidateChange {
            change: change.clone(),
            target_revision: must(ExternalRef::new("pijul", "state", &hash('M'))),
            source_snapshot: snapshot.clone(),
            pipeline_revision: pipeline.clone(),
            collaboration_provider: provider_id.clone(),
        }],
        dependencies: vec![],
        frozen_sources: BTreeMap::from([(change, snapshot)]),
        policy_id: must(PolicyId::new("policy")),
    };
    let plan = ReviewEffectPlan {
        effect_id: must(EffectId::new("review-effect")),
        idempotency_key: identity("request"),
        candidate_root: must(candidate_root(&candidate, limits.max_request_bytes)),
        candidate,
        pipeline_revision: pipeline,
        policy_identity: must(policy_identity(&profile)),
        provider_profile: provider_id,
        native_binding_identity: must(native_binding_identity(&profile)),
    };
    let native_hash = hash('A');
    let report = json!({
        "schema_version": 1, "dry_run": true, "authorizes_acceptance": false,
        "main": profile.native.main, "inbox": profile.native.inbox,
        "configuration": profile.native.configuration,
        "selection": {"requested_changes": [native_hash], "requested_authors": [],
            "include_dependencies": false, "selected": [native_hash], "added_dependencies": []},
        "items": [{"hash": native_hash, "message": "Documentation", "paths": ["docs/guide.md"],
            "state": "accepted", "reason": "Not parsed by Kiln", "needs": [],
            "decided": true, "merge": null, "note": ""}],
        "records": [{"hash": native_hash, "decision": "accept", "by": "gate",
            "configuration": profile.native.configuration,
            "provenance": {"requests": [], "verifiers": {"schema_version": 1,
                "patchgate": "0.1.0", "pijul": "beta.24", "nix": null}},
            "unrecognized_native_evidence": {"retain": "exactly"}}],
        "review_usage": {"requests": 0, "max_requests": 128, "elapsed_seconds": 0.1,
            "timeout_seconds": 900.0, "stop": null}
    });
    (profile, plan, report)
}

fn bytes(report: &Value) -> Vec<u8> {
    must(serde_json::to_vec_pretty(report))
}

fn reconcile(
    profile: &PatchgateProfile,
    plan: &ReviewEffectPlan,
    report: &Value,
) -> kiln_provider_api::PortResult<kiln_core::review::ReviewEvaluationObservation> {
    let raw = bytes(report);
    mapping::validate_plan(profile, plan, Operation::Reconcile)?;
    let prior = mapping::admit_report(profile, plan, &raw)?;
    must(PatchgateAdapter::new(profile.clone())).reconcile(plan, &prior, &raw)
}

// r[verify kiln.patchgate.result]
// r[verify kiln.patchgate.nonclaims.scenario.authority]
#[test]
fn deterministic_acceptance_needs_actual_landed_state_not_model_or_empty_records() {
    let (profile, plan, report) = fixture();
    let observation = must(reconcile(&profile, &plan, &report));
    assert_eq!(
        observation.reviews.first().map(|review| review.decision),
        Some(ReviewDecision::Accepted)
    );
    assert!(!observation.authorizes_acceptance);
    let mut no_acceptance = report.clone();
    (*json_field!(no_acceptance; "records")) = json!([]);
    assert!(reconcile(&profile, &plan, &no_acceptance).is_err());
    let mut unlanded = report.clone();
    (*json_field!(unlanded; "items", 0, "state")) = json!("removed");
    assert_eq!(
        must(reconcile(&profile, &plan, &unlanded))
            .reviews
            .first()
            .map(|review| review.decision),
        Some(ReviewDecision::Removed)
    );
    let mut undecided = report.clone();
    (*json_field!(undecided; "items", 0, "decided")) = json!(false);
    assert!(reconcile(&profile, &plan, &undecided).is_err());
}

// r[verify kiln.patchgate.execution.scenario.reconcile]
#[test]
fn retained_receipt_binds_original_bytes_and_the_exact_request() {
    let (profile, plan, report) = fixture();
    // The executable and repository do not exist: reconciliation must not dispatch.
    let raw = bytes(&report);
    let prior = must(mapping::admit_report(&profile, &plan, &raw));
    let adapter = must(PatchgateAdapter::new(profile));
    let first = must(adapter.reconcile(&plan, &prior, &raw));
    assert_eq!(
        first.report_identity,
        must(kiln_codec::identity_from_bytes(&raw))
    );
    let compact = must(serde_json::to_vec(&report));
    assert!(adapter.reconcile(&plan, &prior, &compact).is_err());
    let mut substituted = report.clone();
    (*json_field!(substituted; "records", 0, "unrecognized_native_evidence", "retain")) =
        json!("changed");
    assert!(
        adapter
            .reconcile(&plan, &prior, &bytes(&substituted))
            .is_err()
    );
    let mut different_plan = plan.clone();
    different_plan.effect_id = must(EffectId::new("other-effect"));
    assert!(adapter.reconcile(&different_plan, &prior, &raw).is_err());
    let mut different_observation = prior.clone();
    different_observation
        .reviews
        .first_mut()
        .unwrap_or_else(|| unreachable!("review fixture"))
        .decision = ReviewDecision::Rejected;
    assert!(
        adapter
            .reconcile(&plan, &different_observation, &raw)
            .is_err()
    );
}

// r[verify kiln.patchgate.result.scenario.complete]
#[test]
fn all_native_nonaccepting_states_remain_distinct() {
    let (profile, plan, report) = fixture();
    for (state, expected) in [
        ("rejected", ReviewDecision::Rejected),
        ("escalated", ReviewDecision::Escalated),
        ("blocked", ReviewDecision::Blocked),
        ("waiting", ReviewDecision::Waiting),
        ("pending", ReviewDecision::Pending),
        ("removed", ReviewDecision::Removed),
        ("reverted", ReviewDecision::Reverted),
        ("unknown", ReviewDecision::Unknown),
    ] {
        let mut current = report.clone();
        (*json_field!(current; "items", 0, "state")) = json!(state);
        if matches!(
            expected,
            ReviewDecision::Rejected | ReviewDecision::Escalated
        ) {
            (*json_field!(current; "records", 0, "decision")) =
                json!(if expected == ReviewDecision::Rejected {
                    "reject"
                } else {
                    "escalate"
                });
            let mut historical_failure = current.clone();
            (*json_field!(historical_failure; "items", 0, "decided")) = json!(false);
            (*json_field!(historical_failure; "records")) = json!([]);
            assert!(reconcile(&profile, &plan, &historical_failure).is_err());
        } else {
            (*json_field!(current; "items", 0, "decided")) = json!(false);
            (*json_field!(current; "records")) = json!([]);
        }
        assert_eq!(
            must(reconcile(&profile, &plan, &current))
                .reviews
                .first()
                .map(|review| review.decision),
            Some(expected)
        );
    }
    let mut conflict = report.clone();
    (*json_field!(conflict; "records", 0, "decision")) = json!("reject");
    assert!(reconcile(&profile, &plan, &conflict).is_err());
}

// r[verify kiln.patchgate.profile]
// r[verify kiln.patchgate.request.scenario.stale]
#[test]
fn native_bindings_and_configuration_cannot_be_substituted() {
    let (profile, plan, report) = fixture();
    assert!(reconcile(&profile, &plan, &report).is_ok());
    for (pointer, value) in [
        ("/schema_version", json!(2)),
        ("/dry_run", json!(false)),
        ("/authorizes_acceptance", json!(true)),
        ("/main/state", json!(hash('X'))),
        ("/inbox/state", json!(null)),
        ("/main/channel", json!("other")),
        ("/configuration/values/protected", json!([])),
        ("/configuration/values/max_review_requests", json!(1)),
        ("/configuration/sha256", json!("b".repeat(64))),
    ] {
        let mut bad = report.clone();
        *bad.pointer_mut(pointer)
            .unwrap_or_else(|| unreachable!("fixture pointer")) = value;
        assert!(
            reconcile(&profile, &plan, &bad).is_err(),
            "accepted substitution at {pointer}"
        );
    }
    let mut bad_plan = plan.clone();
    bad_plan.candidate_root = identity("wrong candidate");
    assert!(reconcile(&profile, &bad_plan, &report).is_err());
    let mut changed_profile = profile.clone();
    changed_profile
        .environment
        .insert("PATCHGATE_MODEL".to_string(), "different".to_string());
    assert!(reconcile(&changed_profile, &plan, &report).is_err());
}

// r[verify kiln.patchgate.result.scenario.invalid]
#[test]
fn exact_selection_refuses_duplicates_missing_items_and_extra_prerequisites() {
    let (profile, plan, report) = fixture();
    assert!(reconcile(&profile, &plan, &report).is_ok());
    for (pointer, value) in [
        (
            "/selection/requested_changes",
            json!([hash('A'), hash('A')]),
        ),
        ("/selection/selected", json!([])),
        ("/selection/added_dependencies", json!([hash('B')])),
        ("/selection/requested_authors", json!(["author"])),
        ("/selection/include_dependencies", json!(true)),
        ("/items", json!([])),
        ("/items/0/needs", json!([[hash('B'), "waiting"]])),
    ] {
        let mut bad = report.clone();
        *bad.pointer_mut(pointer)
            .unwrap_or_else(|| unreachable!("fixture pointer")) = value;
        assert!(
            reconcile(&profile, &plan, &bad).is_err(),
            "accepted bad selection at {pointer}"
        );
    }
    for field in ["items", "records"] {
        let mut bad = report.clone();
        let duplicate = bad
            .get(field)
            .and_then(|array| array.get(0))
            .unwrap_or_else(|| unreachable!("fixture array item"))
            .clone();
        bad.get_mut(field)
            .and_then(Value::as_array_mut)
            .unwrap_or_else(|| unreachable!("array"))
            .push(duplicate);
        assert!(reconcile(&profile, &plan, &bad).is_err());
    }
    let mut invalid_hash = plan.clone();
    let invalid_candidate = invalid_hash
        .candidate
        .ordered_changes
        .first_mut()
        .unwrap_or_else(|| unreachable!("candidate fixture"));
    invalid_candidate.change = must(ExternalRef::new("pijul", "change", "PREFIX"));
    invalid_hash.candidate.frozen_sources = BTreeMap::from([(
        invalid_candidate.change.clone(),
        invalid_candidate.source_snapshot.clone(),
    )]);
    invalid_hash.candidate_root = must(candidate_root(
        &invalid_hash.candidate,
        profile.limits.max_request_bytes,
    ));
    assert!(reconcile(&profile, &invalid_hash, &report).is_err());
}

#[test]
fn model_bindings_allow_simulated_main_advancement_but_not_foreign_policy() {
    let (profile, plan, mut report) = fixture();
    (*json_field!(report; "records", 0, "by")) = json!("model");
    (*json_field!(report; "records", 0, "model")) = json!("declared-not-attested");
    (*json_field!(report; "records", 0, "provenance", "requests")) = json!([{
        "schema_version": 1, "operation": "review", "api": "systemone", "model": "declared-not-attested",
        "declared_revision": "operator-declared", "revision_verified": false,
        "prompt_sha256": "a".repeat(64), "schema_sha256": "b".repeat(64),
    }]);
    (*json_field!(report; "records", 0, "review")) = json!({"candidates": [hash('A')],
        "policy": profile.native.policy, "main": {"channel": "main", "state": hash('N')}});
    assert_eq!(
        must(reconcile(&profile, &plan, &report))
            .reviews
            .first()
            .map(|review| review.decision),
        Some(ReviewDecision::Accepted)
    );
    // The pinned producer omits request digests on fresh short model reviews.
    // Retain that limitation rather than invent evidence or reject a valid report.
    let mut missing_requests = report.clone();
    (*json_field!(missing_requests; "records", 0, "provenance", "requests")) = json!([]);
    assert_eq!(
        must(reconcile(&profile, &plan, &missing_requests))
            .reviews
            .first()
            .map(|review| review.decision),
        Some(ReviewDecision::Accepted)
    );
    let mut malformed_request = report.clone();
    (*json_field!(malformed_request; "records", 0, "provenance", "requests", 0, "prompt_sha256")) =
        json!("not-a-digest");
    assert!(reconcile(&profile, &plan, &malformed_request).is_err());
    let mut foreign = report.clone();
    (*json_field!(foreign; "records", 0, "review", "policy", "sha256")) = json!("f".repeat(64));
    assert!(reconcile(&profile, &plan, &foreign).is_err());
    let mut foreign = report.clone();
    (*json_field!(foreign; "records", 0, "review", "candidates")) = json!([hash('B')]);
    assert!(reconcile(&profile, &plan, &foreign).is_err());
    let mut incomplete = report.clone();
    (*json_field!(incomplete; "records", 0, "coverage")) = json!({"schema_version": 1, "planned_pages": 2,
        "complete": false, "pages": []});
    assert!(reconcile(&profile, &plan, &incomplete).is_err());
    for (decision, state, expected) in [
        ("reject", "rejected", ReviewDecision::Rejected),
        ("escalate", "escalated", ReviewDecision::Escalated),
    ] {
        let mut refusal = report.clone();
        (*json_field!(refusal; "items", 0, "state")) = json!(state);
        (*json_field!(refusal; "records", 0, "decision")) = json!(decision);
        (*json_field!(refusal; "records", 0, "coverage")) = json!({"schema_version": 1, "planned_pages": 2,
            "complete": false, "pages": [{"index": 0, "decision": decision}]});
        assert_eq!(
            must(reconcile(&profile, &plan, &refusal))
                .reviews
                .first()
                .map(|review| review.decision),
            Some(expected)
        );
    }
    let mut misleading_completion = report.clone();
    (*json_field!(misleading_completion; "records", 0, "coverage")) = json!({"schema_version": 1,
        "planned_pages": 1, "complete": true, "pages": [{"index": 0, "decision": "reject"}]});
    assert!(reconcile(&profile, &plan, &misleading_completion).is_err());
}

#[test]
fn empty_native_main_and_verification_bindings_preserve_native_semantics() {
    let (mut profile, mut plan, mut report) = fixture();
    profile.native.main.state = None;
    plan.candidate
        .ordered_changes
        .first_mut()
        .unwrap_or_else(|| unreachable!("candidate fixture"))
        .target_revision = must(ExternalRef::new("pijul", "state", "empty"));
    plan.candidate_root = must(candidate_root(
        &plan.candidate,
        profile.limits.max_request_bytes,
    ));
    plan.native_binding_identity = must(native_binding_identity(&profile));
    (*json_field!(report; "main", "state")) = Value::Null;
    (*json_field!(report; "records", 0, "verification")) = json!({"schema_version": 1,
        "candidates": [hash('A')], "main": {"channel": "main", "state": null},
        "source": {"path": "/nix/store/candidate", "nar_hash": "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="},
        "definitions": {"path": "/nix/store/checks", "nar_hash": "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="},
        "inputs": {}, "results": [{"check": "build.nix", "drv_path": "/nix/store/check.drv",
            "outputs": {"out": "/nix/store/result"}}]});
    assert_eq!(
        must(reconcile(&profile, &plan, &report))
            .reviews
            .first()
            .map(|review| review.decision),
        Some(ReviewDecision::Accepted)
    );
    let mut foreign = report.clone();
    (*json_field!(foreign; "records", 0, "verification", "candidates")) = json!([hash('B')]);
    assert!(reconcile(&profile, &plan, &foreign).is_err());
    (*json_field!(report; "records", 0, "verification", "main", "channel")) = json!("inbox");
    assert!(reconcile(&profile, &plan, &report).is_err());
}

#[test]
fn operational_stops_and_native_execution_errors_are_not_rejections() {
    let (profile, plan, report) = fixture();
    assert!(reconcile(&profile, &plan, &report).is_ok());
    let mut failed = report.clone();
    (*json_field!(failed; "records", 0, "execution_error")) = json!("invalid-model-reply");
    assert!(matches!(
        reconcile(&profile, &plan, &failed),
        Err(PortFailure::Infrastructure(_))
    ));
    for stop in ["timeout", "interrupted", "request-limit"] {
        let mut stopped = report.clone();
        (*json_field!(stopped; "review_usage", "stop")) = json!(stop);
        let error = reconcile(&profile, &plan, &stopped);
        match stop {
            "timeout" => assert!(matches!(error, Err(PortFailure::TimedOut(_)))),
            "interrupted" => assert!(matches!(error, Err(PortFailure::Cancelled(_)))),
            _ => assert!(matches!(error, Err(PortFailure::Infrastructure(_)))),
        }
    }
    let prior = must(mapping::admit_report(&profile, &plan, &bytes(&report)));
    let adapter = must(PatchgateAdapter::new(profile));
    assert!(
        adapter
            .reconcile(&plan, &prior, b"{\"schema_version\":1")
            .is_err()
    );
    assert!(adapter.reconcile(&plan, &prior, b"{}").is_err());
    assert!(adapter.reconcile(&plan, &prior, &[b' '; 65_537]).is_err());
}

#[cfg(unix)]
fn process_fixture(script: &str) -> (tempfile::TempDir, PatchgateProfile, ReviewEffectPlan) {
    use std::os::unix::fs::PermissionsExt;
    let directory = must(tempfile::tempdir());
    let path = directory.path().join("executable");
    let content = format!("#!/bin/sh\n{script}\n");
    must(std::fs::write(&path, &content));
    must(std::fs::set_permissions(
        &path,
        std::fs::Permissions::from_mode(0o700),
    ));
    let (mut profile, mut plan, _) = fixture();
    profile.program = path;
    profile.repository_root = directory.path().to_path_buf();
    profile.executable_identity = must(kiln_codec::identity_from_bytes(content.as_bytes()));
    plan.native_binding_identity = must(native_binding_identity(&profile));
    (directory, profile, plan)
}

#[cfg(unix)]
#[test]
fn process_success_is_not_approval_and_process_failure_is_not_rejection() {
    for (script, infrastructure) in [("exit 0", false), ("exit 7", true)] {
        let (_directory, profile, plan) = process_fixture(script);
        let mut adapter = must(PatchgateAdapter::new(profile));
        let result = adapter.review(&plan);
        if infrastructure {
            assert!(matches!(result, Err(PortFailure::Infrastructure(_))));
        } else {
            assert!(matches!(result, Err(PortFailure::Domain(_))));
        }
        assert_eq!(adapter.raw_report(), Some(&b""[..]));
    }
}

#[cfg(unix)]
#[test]
fn real_process_gets_null_stdin_and_only_explicit_environment() {
    let (_directory, mut profile, mut plan) = process_fixture(
        "[ \"${ONLY_EXPLICIT}\" = declared ] || exit 91\n[ -z \"${HOME+x}\" ] || exit 92\nread value && exit 93\nexit 0",
    );
    profile
        .environment
        .insert("ONLY_EXPLICIT".to_string(), "declared".to_string());
    plan.native_binding_identity = must(native_binding_identity(&profile));
    must(runtime::verify_executable(&profile));
    let output = must(runtime::execute(runtime::invocation(&profile, &plan), None));
    assert_eq!(output.status, Some(0));
    assert_eq!(output.disposition, bounded_exec::Disposition::Succeeded);
}

// r[verify kiln.patchgate.execution]
// r[verify kiln.patchgate.execution.scenario.stop]
#[cfg(unix)]
#[test]
fn real_process_deadline_cancellation_and_capture_overflow_preserve_failure_classes() {
    let (_directory, mut profile, mut plan) = process_fixture(WAITING_PROCESS);
    profile.limits.timeout_ms = PROCESS_TEST_DELAY_MS;
    plan.native_binding_identity = must(native_binding_identity(&profile));
    let mut adapter = must(PatchgateAdapter::new(profile));
    let result = adapter.review(&plan);
    assert!(
        matches!(&result, Err(PortFailure::TimedOut(_))),
        "{result:?}"
    );
    let cancelled = AtomicBool::new(true);
    assert!(matches!(
        adapter.review_with_cancellation(&plan, &cancelled),
        Err(PortFailure::Cancelled(_))
    ));
    let (_directory, profile, plan) = process_fixture(WAITING_PROCESS);
    let mut adapter = must(PatchgateAdapter::new(profile));
    let cancellation = std::sync::Arc::new(AtomicBool::new(false));
    let trigger = cancellation.clone();
    let worker = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(PROCESS_TEST_DELAY_MS));
        trigger.store(true, std::sync::atomic::Ordering::Release);
    });
    assert!(matches!(
        adapter.review_with_cancellation(&plan, &cancellation),
        Err(PortFailure::Cancelled(_))
    ));
    must(worker.join());
    // A continuously running producer reaches the deadline; overflow is checked
    // on a completed process so this case isolates the capture-limit outcome.
    let (_directory, profile, plan) = process_fixture("printf '%070000d' 0");
    let report_bound = profile.limits.max_report_bytes;
    let mut adapter = must(PatchgateAdapter::new(profile));
    let result = adapter.review(&plan);
    assert!(
        matches!(&result, Err(PortFailure::Infrastructure(detail)) if detail.code == "patchgate_output_limit"),
        "{result:?}"
    );
    assert!(
        adapter
            .raw_report()
            .is_some_and(|report| report.len() <= report_bound)
    );
}

// r[verify kiln.patchgate.profile.scenario.drift]
#[cfg(unix)]
#[test]
fn executable_replacement_is_refused_before_dispatch() {
    let (_directory, profile, plan) = process_fixture("exit 7");
    must(std::fs::write(&profile.program, "#!/bin/sh\nexit 0\n"));
    let mut adapter = must(PatchgateAdapter::new(profile));
    assert!(matches!(adapter.review(&plan), Err(PortFailure::Domain(_))));
    assert_eq!(adapter.raw_report(), None);
}
