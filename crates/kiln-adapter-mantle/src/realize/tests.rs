use std::collections::{BTreeMap, VecDeque};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use kiln_core::types::{Alias, AttemptId, CandidateId, EffectId, PipelineRevisionId};

use super::*;

const PILOT_RECEIPT: &[u8] =
    include_bytes!("../../../../fixtures/mantle/foreign-realize/realization-receipt.json");
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
const EXECUTION_PROFILE_PATH: &str = "/etc/kiln/mantle-foreign-nix-v1.json";

fn content(value: &str) -> ContentId {
    ContentId::from_blake3_hex(value).unwrap_or_else(|_| unreachable!())
}

fn limits() -> MantleRealizeLimits {
    MantleRealizeLimits {
        max_required_products: 8,
        max_products: 8,
        max_bindings: 4,
        max_roots: 1,
        max_request_bytes: 1_048_576,
        max_result_bytes: 16_777_216,
        max_stderr_bytes: 65_536,
        max_receipt_bytes: 16_777_216,
        max_input_bytes: 16_777_216,
        max_executable_bytes: 268_435_456,
        max_in_flight: 4,
        stdin_max_bytes: 1,
        timeout_milliseconds: 3_600_000,
        poll_interval_milliseconds: 5,
        teardown_timeout_milliseconds: 1_000,
    }
}

fn binding() -> MantleRealizeBinding {
    MantleRealizeBinding {
        plan_path: PathBuf::from(PLAN_PATH),
        plan_file_identity: content(PILOT_PLAN_FILE),
        plan_identity: content(PILOT_PLAN),
        import_receipt_path: PathBuf::from(IMPORT_PATH),
        import_receipt_file_identity: content(PILOT_IMPORT_FILE),
        import_receipt_identity: content(PILOT_IMPORT),
        source_bundle_path: PathBuf::from(BUNDLE_PATH),
        source_bundle_file_identity: content(PILOT_BUNDLE_FILE),
        source_bundle_manifest_identity: content(PILOT_MANIFEST),
        execution_profile_path: PathBuf::from(EXECUTION_PROFILE_PATH),
        execution_profile_file_identity: content(PILOT_PROFILE_FILE),
        cache_closure_policy_path: PathBuf::from(POLICY_PATH),
        cache_closure_policy_file_identity: content(PILOT_POLICY_FILE),
        cache_closure_policy_identity: content(PILOT_POLICY),
        remote_binding_config_path: PathBuf::from(BINDING_PATH),
        remote_binding_config_file_identity: content(PILOT_BINDING_FILE),
        product_roots: BTreeMap::from([("out".to_string(), PathBuf::from(PILOT_ROOT))]),
    }
}

fn profile() -> MantleRealizeProfile {
    MantleRealizeProfile::pinned(
        ProviderProfileId::new("mantle-foreign-realize-v1").unwrap_or_else(|_| unreachable!()),
        MantleRealizeRuntimeBinding {
            executable_path: PathBuf::from(PINNED_REALIZE_MANTLE_EXECUTABLE_PATH),
            executable_identity: content(PINNED_REALIZE_MANTLE_EXECUTABLE_BLAKE3),
            store_prefix: PathBuf::from(PINNED_REALIZE_STORE_PREFIX),
            store_path: PathBuf::from("/var/lib/kiln/mantle-realize-store"),
            state_path: PathBuf::from("/var/lib/kiln/mantle-realize-state"),
        },
        limits(),
        vec![binding()],
    )
    .unwrap_or_else(|_| unreachable!())
}

fn plan() -> BuildEffectPlan {
    BuildEffectPlan {
        effect_id: EffectId::new("realize-effect-a").unwrap_or_else(|_| unreachable!()),
        attempt_id: AttemptId::new("attempt-a").unwrap_or_else(|_| unreachable!()),
        idempotency_key: content(&"1".repeat(64)),
        candidate_id: CandidateId::new("candidate-a").unwrap_or_else(|_| unreachable!()),
        pipeline_revision: PipelineRevisionId::new("pipeline-a").unwrap_or_else(|_| unreachable!()),
        plan_identity: content(PILOT_PLAN),
        policy_identity: content(&"3".repeat(64)),
        provider_profile: ProviderProfileId::new("mantle-foreign-realize-v1")
            .unwrap_or_else(|_| unreachable!()),
        platform: Alias::new("x86_64-linux").unwrap_or_else(|_| unreachable!()),
        required_products: vec![Alias::new("out").unwrap_or_else(|_| unreachable!())],
        constraints: None,
    }
}

fn mutate_receipt(mutator: impl FnOnce(&mut RealizationReceipt)) -> Vec<u8> {
    let mut receipt: RealizationReceipt =
        serde_json::from_slice(PILOT_RECEIPT).unwrap_or_else(|_| unreachable!());
    mutator(&mut receipt);
    receipt.receipt_blake3 = receipt_digest(&receipt).unwrap_or_else(|_| unreachable!());
    serde_json::to_vec(&receipt).unwrap_or_else(|_| unreachable!())
}

fn assert_code(result: PortResult<BuildEvaluationObservation>, code: &str) {
    match result {
        Err(PortFailure::Domain(diagnostic)) => assert_eq!(diagnostic.code, code),
        other => unreachable!("expected domain refusal {code}, got {other:?}"),
    }
}

fn assert_profile_code(result: PortResult<()>, code: &str) {
    match result {
        Err(PortFailure::Domain(diagnostic)) => assert_eq!(diagnostic.code, code),
        other => unreachable!("expected profile refusal {code}, got {other:?}"),
    }
}

#[derive(Debug)]
struct FakeRealizeRuntime {
    identities: BTreeMap<PathBuf, ContentId>,
    outputs: VecDeque<MantleProcessOutput>,
    receipt: Option<Vec<u8>>,
    runs: usize,
    invocations: Vec<ProcessInvocation>,
}

impl FakeRealizeRuntime {
    fn new(output: MantleProcessOutput, receipt: Option<Vec<u8>>) -> Self {
        let mut identities = BTreeMap::new();
        identities.insert(
            PathBuf::from(PINNED_REALIZE_MANTLE_EXECUTABLE_PATH),
            content(PINNED_REALIZE_MANTLE_EXECUTABLE_BLAKE3),
        );
        identities.insert(PathBuf::from(PLAN_PATH), content(PILOT_PLAN_FILE));
        identities.insert(PathBuf::from(IMPORT_PATH), content(PILOT_IMPORT_FILE));
        identities.insert(PathBuf::from(BUNDLE_PATH), content(PILOT_BUNDLE_FILE));
        identities.insert(
            PathBuf::from(EXECUTION_PROFILE_PATH),
            content(PILOT_PROFILE_FILE),
        );
        identities.insert(PathBuf::from(POLICY_PATH), content(PILOT_POLICY_FILE));
        identities.insert(PathBuf::from(BINDING_PATH), content(PILOT_BINDING_FILE));
        Self {
            identities,
            outputs: VecDeque::from([output]),
            receipt,
            runs: 0,
            invocations: Vec::new(),
        }
    }

    fn success() -> Self {
        Self::new(
            MantleProcessOutput::Exited {
                process_status: Some(0),
                stdout: Vec::new(),
                stderr: Vec::new(),
            },
            Some(PILOT_RECEIPT.to_vec()),
        )
    }
}

impl MantleRuntime for FakeRealizeRuntime {
    fn file_identity(&self, path: &Path, _max_bytes: u64) -> PortResult<ContentId> {
        self.identities.get(path).cloned().ok_or_else(|| {
            PortFailure::Domain(Diagnostic::new(
                "fake_missing_identity",
                "mantle.file",
                "fixture has no identity for this path",
            ))
        })
    }

    fn run(&mut self, invocation: &ProcessInvocation) -> PortResult<MantleProcessOutput> {
        self.runs = self.runs.saturating_add(1);
        self.invocations.push(invocation.clone());
        self.outputs.pop_front().ok_or_else(|| {
            PortFailure::Unknown(kiln_provider_api::InfrastructureFailure {
                code: "fixture_output_missing".to_string(),
                message: "fixture has no Mantle process output".to_string(),
            })
        })
    }

    fn run_with_cancellation(
        &mut self,
        invocation: &ProcessInvocation,
        _cancellation: &AtomicBool,
    ) -> PortResult<MantleProcessOutput> {
        self.run(invocation)
    }

    fn read_log(
        &self,
        _path: &Path,
        _offset: usize,
        _max_bytes: usize,
        _total_bound: usize,
    ) -> PortResult<(Vec<u8>, bool)> {
        unreachable!("realize adapter reads no logs")
    }

    fn read_bounded_file(&self, _path: &Path, _max_bytes: usize) -> PortResult<Vec<u8>> {
        self.receipt.clone().ok_or_else(|| {
            PortFailure::Domain(Diagnostic::new(
                "fake_missing_receipt",
                "mantle.file",
                "fixture has no receipt",
            ))
        })
    }

    fn prepare_directory(&self, path: &Path) -> PortResult<()> {
        if !path.is_absolute() {
            return Err(PortFailure::Domain(Diagnostic::new(
                "fake_directory",
                "mantle.directory",
                "fixture directory must be absolute",
            )));
        }
        Ok(())
    }
}

#[test]
// r[verify kiln.mantle.foreign_realize.completion.scenario.pilot]
// r[verify kiln.convergence.realize.scenario.pilot]
fn pilot_receipt_is_admitted() {
    let profile = profile();
    let plan = plan();
    let observation = admit_realization_receipt(&profile, &plan, &binding(), PILOT_RECEIPT)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(observation.outcome, BuildEvaluationOutcome::Success);
    assert_eq!(observation.products.len(), 1);
    assert_eq!(
        observation
            .products
            .first()
            .unwrap_or_else(|| unreachable!())
            .name
            .as_str(),
        "out"
    );
    assert!(observation.receipt_identity.is_some());
    assert_eq!(observation.cache_kind, BuildCacheKind::Substitution);
    assert_eq!(admit_build_evaluation(&plan, &observation), Ok(()));
}

#[test]
// r[verify kiln.mantle.foreign_realize.completion.scenario.drift]
fn unknown_and_missing_fields_are_cohort_drift() {
    let mut value: serde_json::Value =
        serde_json::from_slice(PILOT_RECEIPT).unwrap_or_else(|_| unreachable!());
    value
        .as_object_mut()
        .unwrap_or_else(|| unreachable!())
        .insert("unexpected".to_string(), serde_json::json!(true));
    let bytes = serde_json::to_vec(&value).unwrap_or_else(|_| unreachable!());
    assert_code(
        admit_realization_receipt(&profile(), &plan(), &binding(), &bytes),
        "mantle_realize_receipt_parse",
    );

    let mut value: serde_json::Value =
        serde_json::from_slice(PILOT_RECEIPT).unwrap_or_else(|_| unreachable!());
    value
        .as_object_mut()
        .unwrap_or_else(|| unreachable!())
        .remove("non_claims");
    let bytes = serde_json::to_vec(&value).unwrap_or_else(|_| unreachable!());
    assert_code(
        admit_realization_receipt(&profile(), &plan(), &binding(), &bytes),
        "mantle_realize_receipt_parse",
    );
}

#[test]
// r[verify kiln.mantle.foreign_realize.completion.scenario.drift]
fn wrong_schema_is_refused() {
    let bytes = mutate_receipt(|receipt| receipt.schema = "other-receipt-v1".to_string());
    assert_code(
        admit_realization_receipt(&profile(), &plan(), &binding(), &bytes),
        "mantle_realize_receipt_schema",
    );
}

#[test]
// r[verify kiln.mantle.foreign_realize.completion.scenario.drift]
fn incomplete_status_is_refused() {
    let bytes = mutate_receipt(|receipt| {
        receipt.status = "partial-failure".to_string();
        receipt.strongest_state = "partial-realization".to_string();
    });
    assert_code(
        admit_realization_receipt(&profile(), &plan(), &binding(), &bytes),
        "mantle_realize_receipt_status",
    );
}

#[test]
// r[verify kiln.mantle.foreign_realize.recovery.scenario.classes]
fn failure_envelope_is_a_distinct_failed_observation() {
    let bytes = mutate_receipt(|receipt| {
        receipt.status = "partial-failure".to_string();
        receipt.strongest_state = "partial-realization".to_string();
        receipt.failure = Some(ForeignRealizationFailure {
            class: "foreign-root-build-failed".to_string(),
            message: "root failed".to_string(),
            affected_roots: vec!["nix:root.drv".to_string()],
            completed_unit_count: 0,
        });
    });
    let observation = admit_realization_receipt(&profile(), &plan(), &binding(), &bytes)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(observation.outcome, BuildEvaluationOutcome::Failed);
    assert!(observation.products.is_empty());
    assert!(observation.receipt_identity.is_some());
    assert_eq!(admit_build_evaluation(&plan(), &observation), Ok(()));
}

#[test]
// r[verify kiln.convergence.realize.scenario.substitution]
// r[verify kiln.mantle.foreign_realize.completion.scenario.drift]
fn a_tampered_digest_is_refused() {
    let mut receipt: RealizationReceipt =
        serde_json::from_slice(PILOT_RECEIPT).unwrap_or_else(|_| unreachable!());
    receipt.receipt_blake3 = "0".repeat(64);
    let bytes = serde_json::to_vec(&receipt).unwrap_or_else(|_| unreachable!());
    assert_code(
        admit_realization_receipt(&profile(), &plan(), &binding(), &bytes),
        "mantle_realize_receipt_digest",
    );
}

#[test]
// r[verify kiln.convergence.realize.scenario.substitution]
// r[verify kiln.mantle.foreign_realize.completion.scenario.drift]
fn each_bound_receipt_identity_is_required() {
    for (code, mutate) in [
        (
            "mantle_realize_receipt_plan",
            Box::new(|receipt: &mut RealizationReceipt| {
                receipt.plan_blake3 = "a".repeat(64);
            }) as Box<dyn FnOnce(&mut RealizationReceipt)>,
        ),
        (
            "mantle_realize_receipt_import",
            Box::new(|receipt: &mut RealizationReceipt| {
                receipt.import_receipt_blake3 = "a".repeat(64);
            }),
        ),
        (
            "mantle_realize_receipt_source_bundle",
            Box::new(|receipt: &mut RealizationReceipt| {
                receipt.source_bundle_manifest_blake3 = "a".repeat(64);
            }),
        ),
        (
            "mantle_realize_receipt_policy",
            Box::new(|receipt: &mut RealizationReceipt| {
                receipt.cache_closure_policy_blake3 = Some("a".repeat(64));
            }),
        ),
    ] {
        let bytes = mutate_receipt(mutate);
        assert_code(
            admit_realization_receipt(&profile(), &plan(), &binding(), &bytes),
            code,
        );
    }
}

#[test]
// r[verify kiln.mantle.foreign_realize.completion.scenario.drift]
fn zero_or_two_roots_are_refused() {
    let bytes = mutate_receipt(|receipt| {
        receipt.selected_root_paths.clear();
        receipt.selected_root_node_ids.clear();
    });
    assert_code(
        admit_realization_receipt(&profile(), &plan(), &binding(), &bytes),
        "mantle_realize_receipt_root_count",
    );
    let bytes = mutate_receipt(|receipt| {
        receipt.selected_root_paths.push(PILOT_ROOT.to_string());
        receipt
            .selected_root_node_ids
            .push("nix:other.drv".to_string());
    });
    assert_code(
        admit_realization_receipt(&profile(), &plan(), &binding(), &bytes),
        "mantle_realize_receipt_root_count",
    );
}

#[test]
// r[verify kiln.mantle.foreign_realize.completion.scenario.drift]
fn a_root_outside_the_prefix_is_refused() {
    let bytes = mutate_receipt(|receipt| {
        receipt.selected_root_paths = vec!["/tmp/outside".to_string()];
    });
    assert_code(
        admit_realization_receipt(&profile(), &plan(), &binding(), &bytes),
        "mantle_realize_receipt_root_prefix",
    );
}

#[test]
// r[verify kiln.mantle.foreign_realize.completion.scenario.drift]
fn an_unmapped_or_misplaced_product_is_refused() {
    let mut mapped_plan = plan();
    mapped_plan
        .required_products
        .push(Alias::new("metadata").unwrap_or_else(|_| unreachable!()));
    assert_code(
        admit_realization_receipt(&profile(), &mapped_plan, &binding(), PILOT_RECEIPT),
        "mantle_realize_receipt_product",
    );

    let mut misplaced = binding();
    misplaced
        .product_roots
        .insert("out".to_string(), PathBuf::from("/nix/store/other"));
    assert_code(
        admit_realization_receipt(&profile(), &plan(), &misplaced, PILOT_RECEIPT),
        "mantle_realize_receipt_product",
    );
}

#[test]
// r[verify kiln.mantle.foreign_realize.completion.scenario.oversized]
fn oversized_receipt_is_a_bounded_refusal() {
    let mut profile = profile();
    profile.limits.max_receipt_bytes = 1_024;
    assert_code(
        admit_realization_receipt(&profile, &plan(), &binding(), PILOT_RECEIPT),
        "mantle_realize_receipt_bound",
    );
}

#[test]
// r[verify kiln.mantle.foreign_realize.dispatch.scenario.identity]
fn stale_profile_and_input_identities_stop_before_dispatch() {
    let mut stale = profile();
    stale.runtime.executable_identity = content(&"a".repeat(64));
    assert!(MantleRealizeAdapter::new(stale, FakeRealizeRuntime::success()).is_err());

    let mut stale_binding = binding();
    stale_binding.import_receipt_file_identity = content(&"a".repeat(64));
    let profile = MantleRealizeProfile::pinned(
        ProviderProfileId::new("mantle-foreign-realize-v1").unwrap_or_else(|_| unreachable!()),
        MantleRealizeRuntimeBinding {
            executable_path: PathBuf::from(PINNED_REALIZE_MANTLE_EXECUTABLE_PATH),
            executable_identity: content(PINNED_REALIZE_MANTLE_EXECUTABLE_BLAKE3),
            store_prefix: PathBuf::from(PINNED_REALIZE_STORE_PREFIX),
            store_path: PathBuf::from("/var/lib/kiln/mantle-realize-store"),
            state_path: PathBuf::from("/var/lib/kiln/mantle-realize-state"),
        },
        limits(),
        vec![stale_binding],
    )
    .unwrap_or_else(|_| unreachable!());
    let mut adapter = MantleRealizeAdapter::new(profile, FakeRealizeRuntime::success())
        .unwrap_or_else(|_| unreachable!());
    assert_code(
        adapter.dispatch(&plan()),
        "mantle_realize_import_receipt_identity",
    );
    assert_eq!(adapter.runtime().runs, 0);
}

#[test]
// r[verify kiln.mantle.foreign_realize.dispatch]
fn profile_or_plan_mismatch_is_refused() {
    let mut wrong_profile = plan();
    wrong_profile.provider_profile =
        ProviderProfileId::new("mantle-build-v1").unwrap_or_else(|_| unreachable!());
    let mut adapter = MantleRealizeAdapter::new(profile(), FakeRealizeRuntime::success())
        .unwrap_or_else(|_| unreachable!());
    assert_code(
        adapter.dispatch(&wrong_profile),
        "mantle_realize_request_profile",
    );
    assert_eq!(adapter.runtime().runs, 0);

    let mut unbound = plan();
    unbound.plan_identity = content(&"b".repeat(64));
    assert_code(adapter.dispatch(&unbound), "mantle_realize_binding_missing");
    assert_eq!(adapter.runtime().runs, 0);
}

#[test]
// r[verify kiln.mantle.foreign_realize.dispatch.scenario.command]
fn dispatch_runs_the_exact_reviewed_command() {
    let mut adapter = MantleRealizeAdapter::new(profile(), FakeRealizeRuntime::success())
        .unwrap_or_else(|_| unreachable!());
    adapter.dispatch(&plan()).unwrap_or_else(|_| unreachable!());
    let runtime = adapter.runtime();
    assert_eq!(runtime.runs, 1);
    let invocation = runtime
        .invocations
        .first()
        .unwrap_or_else(|| unreachable!());
    let args: Vec<_> = invocation
        .args
        .iter()
        .map(|arg| arg.to_string_lossy().to_string())
        .collect();
    assert_eq!(
        args,
        vec![
            "--json",
            "--state-dir",
            "/var/lib/kiln/mantle-realize-state",
            "--store",
            "/var/lib/kiln/mantle-realize-store",
            "--store-prefix",
            "/nix/store",
            "--remote-binding-config",
            BINDING_PATH,
            "foreign-import",
            "realize",
            "--plan",
            PLAN_PATH,
            "--import-receipt",
            IMPORT_PATH,
            "--source-bundle",
            BUNDLE_PATH,
            "--source-bundle-blake3",
            PILOT_MANIFEST,
            "--execution-profile",
            EXECUTION_PROFILE_PATH,
            "--cache-closure-policy",
            POLICY_PATH,
            "--receipt-out",
            "/var/lib/kiln/mantle-realize-state/attempt-a/realization-receipt.json",
            "--substitute",
        ]
    );
    assert_eq!(
        invocation.environment,
        vec![(
            OsString::from("PATH"),
            PathBuf::from(PINNED_REALIZE_MANTLE_EXECUTABLE_PATH)
                .parent()
                .unwrap_or_else(|| unreachable!())
                .as_os_str()
                .to_os_string()
        )]
    );
    assert!(!args.iter().any(|arg| arg == "--remote"));
    assert!(!args.iter().any(|arg| arg == "--offline"));
}

#[test]
// r[verify kiln.mantle.foreign_realize.recovery.scenario.replay]
fn replayed_dispatch_returns_the_recorded_observation() {
    let mut adapter = MantleRealizeAdapter::new(profile(), FakeRealizeRuntime::success())
        .unwrap_or_else(|_| unreachable!());
    let first = adapter.dispatch(&plan()).unwrap_or_else(|_| unreachable!());
    let second = adapter.dispatch(&plan()).unwrap_or_else(|_| unreachable!());
    assert_eq!(first, second);
    assert_eq!(adapter.runtime().runs, 1);
    assert_eq!(adapter.observe(&plan().effect_id), Ok(first.clone()));
    assert_eq!(adapter.reconcile(&plan()), Ok(first));
    assert_eq!(adapter.runtime().runs, 1);
}

#[test]
// r[verify kiln.mantle.foreign_realize.recovery.scenario.replay]
fn reconciliation_refuses_a_changed_plan() {
    let mut adapter = MantleRealizeAdapter::new(profile(), FakeRealizeRuntime::success())
        .unwrap_or_else(|_| unreachable!());
    adapter.dispatch(&plan()).unwrap_or_else(|_| unreachable!());
    let mut changed = plan();
    changed.attempt_id = AttemptId::new("attempt-b").unwrap_or_else(|_| unreachable!());
    assert_code(
        adapter.reconcile(&changed),
        "mantle_realize_reconciliation_conflict",
    );
    assert_eq!(adapter.runtime().runs, 1);
}

#[test]
// r[verify kiln.mantle.foreign_realize.completion.scenario.oversized]
fn malformed_receipt_becomes_a_provider_error() {
    let runtime = FakeRealizeRuntime::new(
        MantleProcessOutput::Exited {
            process_status: Some(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
        },
        Some(b"not json".to_vec()),
    );
    let mut adapter =
        MantleRealizeAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
    let observation = adapter.dispatch(&plan()).unwrap_or_else(|_| unreachable!());
    assert_eq!(observation.outcome, BuildEvaluationOutcome::ProviderError);
    assert!(observation.receipt_identity.is_some());
}

#[test]
// r[verify kiln.mantle.foreign_realize.recovery.scenario.classes]
fn timeout_cancellation_and_unknown_keep_their_classes() {
    for (output, expected) in [
        (
            MantleProcessOutput::TimedOut,
            BuildEvaluationOutcome::TimedOut,
        ),
        (
            MantleProcessOutput::Cancelled,
            BuildEvaluationOutcome::Cancelled,
        ),
        (
            MantleProcessOutput::Unknown,
            BuildEvaluationOutcome::Unknown,
        ),
        (
            MantleProcessOutput::OutputLimitExceeded,
            BuildEvaluationOutcome::ProviderError,
        ),
    ] {
        let mut adapter = MantleRealizeAdapter::new(
            profile(),
            FakeRealizeRuntime::new(output, Some(PILOT_RECEIPT.to_vec())),
        )
        .unwrap_or_else(|_| unreachable!());
        let observation = adapter.dispatch(&plan()).unwrap_or_else(|_| unreachable!());
        assert_eq!(observation.outcome, expected);
        if expected == BuildEvaluationOutcome::Unknown {
            assert!(observation.receipt_identity.is_none());
        } else {
            assert!(observation.receipt_identity.is_some());
        }
    }
}

#[test]
// r[verify kiln.mantle.foreign_realize.profile.scenario.drift]
fn profile_admission_refuses_cohort_drift() {
    let mut drifted = profile();
    drifted.revision = "0".repeat(40);
    assert_profile_code(
        admit_realize_profile(&drifted),
        "mantle_realize_profile_cohort",
    );

    let mut no_realize = profile();
    no_realize.provider.operations.remove(&Operation::Realize);
    assert_profile_code(
        admit_realize_profile(&no_realize),
        "mantle_realize_profile_binding",
    );

    let mut bad_limit = profile();
    bad_limit.limits.max_roots = 2;
    assert_profile_code(
        admit_realize_profile(&bad_limit),
        "mantle_realize_profile_limit",
    );

    let mut unmapped = profile();
    unmapped
        .bindings
        .first_mut()
        .unwrap_or_else(|| unreachable!())
        .product_roots
        .clear();
    assert_profile_code(
        admit_realize_profile(&unmapped),
        "mantle_realize_profile_product",
    );

    let mut outside = profile();
    outside
        .bindings
        .first_mut()
        .unwrap_or_else(|| unreachable!())
        .product_roots
        .insert("out".to_string(), PathBuf::from("/tmp/outside"));
    assert_profile_code(
        admit_realize_profile(&outside),
        "mantle_realize_profile_product",
    );
}

#[test]
// r[verify kiln.mantle.foreign_realize.recovery.scenario.replay]
fn restore_replays_only_the_exact_observation() {
    let mut adapter = MantleRealizeAdapter::new(profile(), FakeRealizeRuntime::success())
        .unwrap_or_else(|_| unreachable!());
    let observation = adapter.dispatch(&plan()).unwrap_or_else(|_| unreachable!());
    let mut restored = MantleRealizeAdapter::new(profile(), FakeRealizeRuntime::success())
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(restored.restore(&plan(), &observation), Ok(()));
    assert_eq!(restored.observe(&plan().effect_id), Ok(observation.clone()));
    let mut changed = observation;
    changed.outcome = BuildEvaluationOutcome::ProviderError;
    assert!(restored.restore(&plan(), &changed).is_err());
}

#[test]
// r[verify kiln.mantle.foreign_realize.profile.scenario.match]
fn pinned_realize_profile_is_admitted() {
    let profile = profile();
    assert!(admit_realize_profile(&profile).is_ok());
    assert!(profile.provider.operations.contains(&Operation::Realize));
    assert_eq!(profile.provider.role, ProviderRole::Build);
    assert_eq!(profile.required_non_claims.len(), REALIZE_NON_CLAIMS.len());
}

#[test]
// r[verify kiln.mantle.foreign_realize.nonclaims.scenario.release]
fn complete_realization_grants_no_release_authority() {
    let observation = admit_realization_receipt(&profile(), &plan(), &binding(), PILOT_RECEIPT)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(observation.outcome, BuildEvaluationOutcome::Success);
    assert!(
        REALIZE_NON_CLAIMS
            .iter()
            .any(|claim| claim.contains("release readiness"))
    );
    // The observation is only a build evaluation for its exact plan. It
    // carries no merge, publication or release authority field, so no such
    // consequence can follow from recording it.
    assert_eq!(admit_build_evaluation(&plan(), &observation), Ok(()));
    let value = serde_json::to_value(&observation).unwrap_or_else(|_| unreachable!());
    let fields = value.as_object().unwrap_or_else(|| unreachable!());
    for forbidden in ["release", "merge", "publication", "publish", "authorizes"] {
        assert!(
            !fields.contains_key(forbidden),
            "realization observation exposes {forbidden}"
        );
    }
}
