//! One live qualification of the preserve route against the retained phase-1
//! pilot inputs. Ignored by default; run only with an explicit operator
//! invocation and a fresh physical store and state directory.

use std::collections::BTreeMap;
use std::path::PathBuf;

use kiln_adapter_mantle::{
    BoundedMantleRuntime, MantleRealizeAdapter, MantleRealizeBinding, MantleRealizeLimits,
    MantleRealizeProfile, MantleRealizeRuntimeBinding, PINNED_REALIZE_MANTLE_EXECUTABLE_BLAKE3,
    PINNED_REALIZE_MANTLE_EXECUTABLE_PATH, PINNED_REALIZE_STORE_PREFIX,
};
use kiln_core::build::{BuildEffectPlan, BuildEvaluationOutcome};
use kiln_core::types::{
    Alias, AttemptId, CandidateId, ContentId, EffectId, PipelineRevisionId, ProviderProfileId,
};
use kiln_provider_api::BuildPort;

const PILOT_PLAN: &str = "edd387e529e1d4b277d726edc93901889413dc0dc393c7c7fd715981afb08a78";
const PILOT_PLAN_FILE: &str = "d4efeaa6657729d9800a4b9b1aac6836c37e3839a6483c8c9d3399e29f65ae45";
const PILOT_IMPORT: &str = "60a452562c2d845ec379bef934a0d03b97004e8959b2c409bb4804f9a2558c39";
const PILOT_IMPORT_FILE: &str = "16beddc450fa02626272aefd42d1f37cd0309fa7d4986a53c68c1d48e95c399f";
const PILOT_MANIFEST: &str = "8ed1103b5de3054ee13ea391af805e276e3cc3b6ceaa50b2149193adb1ff1777";
const PILOT_BUNDLE_FILE: &str = "b8abb366a886bb79314411a3d479a97fe8481173334c238333b9118fe7ab8030";
const PILOT_PROFILE_FILE: &str = "054ec6754f08d60088cf3388f43295ad37621e5115e979f702a2a5c5d0eefcb8";
const PILOT_POLICY: &str = "0d228941b966d100140a23dbd865f39a4d54579662747b42ec41f718832261a7";
const PILOT_POLICY_FILE: &str = "9e605fc1f94c372608452d8f49871384b077dd34fe6a748f3d3bfa0879c58194";
const PILOT_BINDING_FILE: &str = "6eb65f6cef773b281319853d1ad548eddc0928a5b98b460142071d72589a21fb";
const PILOT_ROOT: &str =
    "/nix/store/zjq6whhf2c641vkz89kgwwpbajjq5cbq-octet-mantle-aeneas-consumer-job-v1";
const PLAN_PATH: &str =
    "/home/brittonr/wt/convergence-evidence/realization/mantle-graph-preserve-http/plan.json";
const IMPORT_PATH: &str = "/home/brittonr/wt/convergence-evidence/realization/mantle-graph-preserve-http/import-receipt.json";
const BUNDLE_PATH: &str =
    "/datapool/mantle-store/convergence-pilot-20261119/source-bundle-http.json";
const POLICY_PATH: &str =
    "/home/brittonr/wt/convergence-evidence/realization/pilot-cache-closure-policy.json";
const BINDING_PATH: &str =
    "/datapool/mantle-store/convergence-pilot-20261119/binding/remote-endpoint-binding.json";
const DEFAULT_EXECUTION_PROFILE: &str =
    "/tmp/mantle-realize-clean-20261120/config/foreign-execution-profiles/generated/nix.json";
const DEFAULT_PILOT_RECEIPT: &str =
    "/home/brittonr/wt/convergence-evidence/realization/realization-receipt.json";

fn content(value: &str) -> ContentId {
    ContentId::from_blake3_hex(value).unwrap_or_else(|_| unreachable!())
}

fn field<'a>(value: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    value.get(name).unwrap_or_else(|| unreachable!())
}

fn environment(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

#[test]
#[ignore = "live preserve-route qualification; run explicitly once"]
fn realize_pilot_evidence_through_the_pinned_cohort() {
    let store = PathBuf::from(environment(
        "KILN_REALIZE_STORE",
        "/datapool/mantle-store/kiln-phase2b-20261120/store",
    ));
    let state = PathBuf::from(environment(
        "KILN_REALIZE_STATE",
        "/datapool/mantle-store/kiln-phase2b-20261120/state",
    ));
    let execution_profile = PathBuf::from(environment(
        "KILN_REALIZE_EXECUTION_PROFILE",
        DEFAULT_EXECUTION_PROFILE,
    ));
    let pilot_receipt = PathBuf::from(environment(
        "KILN_REALIZE_PILOT_RECEIPT",
        DEFAULT_PILOT_RECEIPT,
    ));

    let binding = MantleRealizeBinding {
        plan_path: PathBuf::from(PLAN_PATH),
        plan_file_identity: content(PILOT_PLAN_FILE),
        plan_identity: content(PILOT_PLAN),
        import_receipt_path: PathBuf::from(IMPORT_PATH),
        import_receipt_file_identity: content(PILOT_IMPORT_FILE),
        import_receipt_identity: content(PILOT_IMPORT),
        source_bundle_path: PathBuf::from(BUNDLE_PATH),
        source_bundle_file_identity: content(PILOT_BUNDLE_FILE),
        source_bundle_manifest_identity: content(PILOT_MANIFEST),
        execution_profile_path: execution_profile,
        execution_profile_file_identity: content(PILOT_PROFILE_FILE),
        cache_closure_policy_path: PathBuf::from(POLICY_PATH),
        cache_closure_policy_file_identity: content(PILOT_POLICY_FILE),
        cache_closure_policy_identity: content(PILOT_POLICY),
        remote_binding_config_path: PathBuf::from(BINDING_PATH),
        remote_binding_config_file_identity: content(PILOT_BINDING_FILE),
        product_roots: BTreeMap::from([("out".to_string(), PathBuf::from(PILOT_ROOT))]),
    };
    let profile = MantleRealizeProfile::pinned(
        ProviderProfileId::new("mantle-foreign-realize-v1").unwrap_or_else(|_| unreachable!()),
        MantleRealizeRuntimeBinding {
            executable_path: PathBuf::from(PINNED_REALIZE_MANTLE_EXECUTABLE_PATH),
            executable_identity: content(PINNED_REALIZE_MANTLE_EXECUTABLE_BLAKE3),
            store_prefix: PathBuf::from(PINNED_REALIZE_STORE_PREFIX),
            store_path: store.clone(),
            state_path: state.clone(),
        },
        MantleRealizeLimits {
            max_required_products: 8,
            max_products: 8,
            max_bindings: 4,
            max_roots: 1,
            max_request_bytes: 1_048_576,
            max_result_bytes: 67_108_864,
            max_stderr_bytes: 1_048_576,
            max_receipt_bytes: 67_108_864,
            max_input_bytes: 67_108_864,
            max_executable_bytes: 268_435_456,
            max_in_flight: 4,
            stdin_max_bytes: 1,
            timeout_milliseconds: 3_600_000,
            poll_interval_milliseconds: 10,
            teardown_timeout_milliseconds: 1_000,
        },
        vec![binding],
    )
    .unwrap_or_else(|_| unreachable!());
    let attempt = AttemptId::new("attempt-phase2b-live").unwrap_or_else(|_| unreachable!());
    let plan = BuildEffectPlan {
        effect_id: EffectId::new("realize-pilot-live").unwrap_or_else(|_| unreachable!()),
        attempt_id: attempt.clone(),
        idempotency_key: content(PILOT_PLAN),
        candidate_id: CandidateId::new("pilot-candidate").unwrap_or_else(|_| unreachable!()),
        pipeline_revision: PipelineRevisionId::new("pilot-pipeline")
            .unwrap_or_else(|_| unreachable!()),
        plan_identity: content(PILOT_PLAN),
        policy_identity: content(PILOT_POLICY),
        provider_profile: ProviderProfileId::new("mantle-foreign-realize-v1")
            .unwrap_or_else(|_| unreachable!()),
        platform: Alias::new("x86_64-linux").unwrap_or_else(|_| unreachable!()),
        required_products: vec![Alias::new("out").unwrap_or_else(|_| unreachable!())],
        constraints: None,
    };
    let mut adapter =
        MantleRealizeAdapter::new(profile, BoundedMantleRuntime).unwrap_or_else(|_| unreachable!());
    let observation = adapter.dispatch(&plan).unwrap_or_else(|_| unreachable!());
    assert_eq!(observation.outcome, BuildEvaluationOutcome::Success);
    assert_eq!(observation.products.len(), 1);
    assert!(observation.receipt_identity.is_some());

    let generated = std::fs::read(
        state
            .join(attempt.as_str())
            .join("realization-receipt.json"),
    )
    .unwrap_or_else(|_| unreachable!());
    let generated: serde_json::Value =
        serde_json::from_slice(&generated).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        field(&generated, "selected_root_paths"),
        &serde_json::json!([PILOT_ROOT])
    );
    assert_eq!(
        field(&generated, "plan_blake3"),
        &serde_json::json!(PILOT_PLAN)
    );
    assert_eq!(
        field(&generated, "import_receipt_blake3"),
        &serde_json::json!(PILOT_IMPORT)
    );

    let pilot: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&pilot_receipt).unwrap_or_else(|_| unreachable!()))
            .unwrap_or_else(|_| unreachable!());
    let comparison = serde_json::json!({
        "selected_root_paths_equal": field(&generated, "selected_root_paths") == field(&pilot, "selected_root_paths"),
        "plan_blake3_equal": field(&generated, "plan_blake3") == field(&pilot, "plan_blake3"),
        "import_receipt_blake3_equal": field(&generated, "import_receipt_blake3") == field(&pilot, "import_receipt_blake3"),
        "generated_receipt_blake3": field(&generated, "receipt_blake3"),
        "pilot_receipt_blake3": field(&pilot, "receipt_blake3"),
        "observation": observation,
    });
    assert_eq!(
        field(&generated, "selected_root_paths"),
        field(&pilot, "selected_root_paths")
    );
    assert_eq!(
        field(&generated, "plan_blake3"),
        field(&pilot, "plan_blake3")
    );
    assert_eq!(
        field(&generated, "import_receipt_blake3"),
        field(&pilot, "import_receipt_blake3")
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&comparison).unwrap_or_default()
    );
}
