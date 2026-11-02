use std::cell::Cell;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use kiln_core::build::{
    BuildCacheKind, BuildEffectPlan, BuildEvaluationObservation, BuildEvaluationOutcome,
    BuildLogReadRequest,
};
use kiln_core::types::{
    Alias, AttemptId, CandidateId, ContentId, EffectId, PipelineRevisionId, ProviderProfileId,
};
use kiln_nix_store::nar::{NarBounds, StoreObjectFacts};
use kiln_provider_api::{BuildPort, PortResult};
use serde::Deserialize;

use super::*;
use crate::profile::admit_profile;
use crate::wire::{
    BuildObservation as WireObservation, BuildRequest as WireRequest, validate_observation,
    validate_request,
};

const IDENTITY_HEX_BYTES: usize = 64;
const MAX_REQUIRED_PRODUCTS: usize = 8;
const MAX_PRODUCTS: usize = 8;
const MAX_LOGS: usize = 8;
const MAX_METRICS: usize = 64;
const MAX_REQUEST_BYTES: usize = 1_048_576;
const MAX_RESULT_BYTES: usize = 1_048_576;
const MAX_STDERR_BYTES: usize = 65_536;
const MAX_LOG_BYTES: usize = 65_536;
const MAX_EXECUTABLE_BYTES: u64 = 268_435_456;
const MAX_PLAN_BYTES: u64 = 1_048_576;
const MAX_STORE_IDENTITY_BYTES: u64 = 65_536;
const MAX_IN_FLIGHT: usize = 4;
const STDIN_MAX_BYTES: usize = 1;
const TIMEOUT_MILLISECONDS: u64 = 30_000;
const POLL_INTERVAL_MILLISECONDS: u64 = 5;
const TEARDOWN_TIMEOUT_MILLISECONDS: u64 = 1_000;
const SUCCESS_STATUS: i32 = 0;

fn content(character: char) -> ContentId {
    ContentId::from_blake3_hex(character.to_string().repeat(IDENTITY_HEX_BYTES))
        .unwrap_or_else(|_| unreachable!())
}

fn limits() -> MantleAdapterLimits {
    MantleAdapterLimits {
        max_required_products: MAX_REQUIRED_PRODUCTS,
        max_products: MAX_PRODUCTS,
        max_logs: MAX_LOGS,
        max_metrics: MAX_METRICS,
        max_request_bytes: MAX_REQUEST_BYTES,
        max_result_bytes: MAX_RESULT_BYTES,
        max_stderr_bytes: MAX_STDERR_BYTES,
        max_log_bytes: MAX_LOG_BYTES,
        max_executable_bytes: MAX_EXECUTABLE_BYTES,
        max_plan_bytes: MAX_PLAN_BYTES,
        max_store_identity_bytes: MAX_STORE_IDENTITY_BYTES,
        max_in_flight: MAX_IN_FLIGHT,
        stdin_max_bytes: STDIN_MAX_BYTES,
        timeout_milliseconds: TIMEOUT_MILLISECONDS,
        poll_interval_milliseconds: POLL_INTERVAL_MILLISECONDS,
        teardown_timeout_milliseconds: TEARDOWN_TIMEOUT_MILLISECONDS,
    }
}

fn profile() -> MantleProfile {
    MantleProfile::pinned(
        ProviderProfileId::new("mantle-build-v1").unwrap_or_else(|_| unreachable!()),
        MantleRuntimeBinding {
            executable_path: PathBuf::from(PINNED_MANTLE_EXECUTABLE_PATH),
            executable_identity: ContentId::from_blake3_hex(PINNED_MANTLE_EXECUTABLE_BLAKE3)
                .unwrap_or_else(|_| unreachable!()),
            plan_path: PathBuf::from("/etc/kiln/mantle-plan.ncl"),
            plan_identity: content('2'),
            policy_identity: content('3'),
            store_prefix: PathBuf::from("/mantle/store"),
            store_path: PathBuf::from("/var/lib/kiln/mantle-store"),
            state_path: PathBuf::from("/var/lib/kiln/mantle-state"),
            store_identity_path: PathBuf::from("/var/lib/kiln/mantle-state/store-identity.json"),
            builder_identity: content('4'),
            worker_identity: content('5'),
            store_identity: content('6'),
        },
        limits(),
    )
    .unwrap_or_else(|_| unreachable!())
}

fn plan() -> BuildEffectPlan {
    BuildEffectPlan {
        effect_id: EffectId::new("build-effect-a").unwrap_or_else(|_| unreachable!()),
        attempt_id: AttemptId::new("attempt-a").unwrap_or_else(|_| unreachable!()),
        idempotency_key: content('1'),
        candidate_id: CandidateId::new("candidate-a").unwrap_or_else(|_| unreachable!()),
        pipeline_revision: PipelineRevisionId::new("pipeline-a").unwrap_or_else(|_| unreachable!()),
        plan_identity: content('2'),
        policy_identity: content('3'),
        provider_profile: ProviderProfileId::new("mantle-build-v1")
            .unwrap_or_else(|_| unreachable!()),
        platform: Alias::new("x86_64-linux").unwrap_or_else(|_| unreachable!()),
        required_products: vec![
            Alias::new("binary").unwrap_or_else(|_| unreachable!()),
            Alias::new("metadata").unwrap_or_else(|_| unreachable!()),
        ],
        constraints: None,
    }
}

fn success_report(cached: bool, include_metadata: bool) -> Vec<u8> {
    serde_json::to_vec(&success_report_value(cached, include_metadata))
        .unwrap_or_else(|_| unreachable!())
}

fn success_report_value(cached: bool, include_metadata: bool) -> serde_json::Value {
    let mut outputs = vec![serde_json::json!({
        "name": "binary",
        "path": "/var/lib/kiln/mantle-store/binary",
        "artifact_attestation": {
            "logical_path": "/mantle/store/binary",
            "path": "/var/lib/kiln/mantle-state/attestations/artifacts/binary.json"
        },
        "substitution": if cached {
            Some(serde_json::json!({
                "mode": "full",
                "transferred_bytes": 55,
                "reused_bytes": 0,
                "fallback_reason": "delta-unsupported"
            }))
        } else {
            None
        },
        "cache_admission": null
    })];
    if include_metadata {
        outputs.push(serde_json::json!({
            "name": "metadata",
            "path": "/var/lib/kiln/mantle-store/metadata",
            "artifact_attestation": {
                "logical_path": "/mantle/store/metadata",
                "path": "/var/lib/kiln/mantle-state/attestations/artifacts/metadata.json"
            },
            "substitution": null,
            "cache_admission": {"reason": "cache-hit"}
        }));
    }
    serde_json::json!({
        "schema": "crunch-build-report-v1",
        "file": "/etc/kiln/mantle-plan.ncl",
        "output_dir": "/var/lib/kiln/mantle-store",
        "state_dir": "/var/lib/kiln/mantle-state",
        "store_dir": "/mantle/store",
        "scheduler_policy": {
            "schema": "mantle-scheduling-policy-v1",
            "policy_id": "mantle-lazy-priority-v1"
        },
        "hermeticity_mode": "practical",
        "hermeticity_audit_events": [],
        "build_environment_reports": [],
        "finish_gates": [],
        "network_policy_reports": [],
        "workspace_reports": [],
        "action_result_reports": [],
        "native_dynamic_plans": [],
        "plan_output_bindings": [],
        "scheduler_priority_decisions": [],
        "overlay_base_generations": [],
        "store_layer_selections": [],
        "remote_telemetry_events": [],
        "frontend_artifact_attestations": [],
        "ast_grep_structural_evidence": [],
        "ast_grep_structural_evidence_diagnostics": [],
        "cargo_build_evidence": [],
        "cargo_build_evidence_diagnostics": [],
        "diagnostic_persistence_failures": [],
        "counts": {
            "succeeded_total": 1,
            "built_total": if cached { 0 } else { 1 },
            "cached_total": if cached { 1 } else { 0 },
            "failed_total": 0
        },
        "outcomes": [{
            "drv_key": "/mantle/store/demo.drv",
            "label": "demo",
            "cached": cached,
            "log_file": "/var/lib/kiln/mantle-state/logs/mantle-a.log",
            "outputs": outputs
        }],
        "failed": [],
        "fod_mismatches": []
    })
}

fn exited(bytes: Vec<u8>) -> MantleProcessOutput {
    exited_with_status(SUCCESS_STATUS, bytes)
}

fn exited_with_status(process_status: i32, bytes: Vec<u8>) -> MantleProcessOutput {
    MantleProcessOutput::Exited {
        process_status: Some(process_status),
        stdout: bytes,
        stderr: Vec::new(),
    }
}

#[derive(Debug)]
struct FakeRuntime {
    executable_identity: ContentId,
    plan_identity: ContentId,
    identity_checks: Cell<usize>,
    outputs: VecDeque<MantleProcessOutput>,
    invocations: Vec<ProcessInvocation>,
    runs: usize,
    store_facts: bool,
}

impl FakeRuntime {
    fn with_outputs(outputs: impl IntoIterator<Item = MantleProcessOutput>) -> Self {
        Self {
            executable_identity: ContentId::from_blake3_hex(PINNED_MANTLE_EXECUTABLE_BLAKE3)
                .unwrap_or_else(|_| unreachable!()),
            plan_identity: content('2'),
            identity_checks: Cell::new(0),
            outputs: outputs.into_iter().collect(),
            invocations: Vec::new(),
            runs: 0,
            store_facts: false,
        }
    }
}

impl MantleRuntime for FakeRuntime {
    fn file_identity(&self, path: &Path, _max_bytes: u64) -> PortResult<ContentId> {
        self.identity_checks
            .set(self.identity_checks.get().saturating_add(1));
        if path == Path::new(PINNED_MANTLE_EXECUTABLE_PATH) {
            Ok(self.executable_identity.clone())
        } else if path == Path::new("/etc/kiln/mantle-plan.ncl") {
            Ok(self.plan_identity.clone())
        } else {
            Ok(content('6'))
        }
    }

    fn run(&mut self, invocation: &ProcessInvocation) -> PortResult<MantleProcessOutput> {
        self.runs = self.runs.saturating_add(1);
        self.invocations.push(invocation.clone());
        self.outputs.pop_front().ok_or_else(|| {
            kiln_provider_api::PortFailure::Unknown(kiln_provider_api::InfrastructureFailure {
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

    fn store_object_facts(
        &self,
        _path: &Path,
        _candidates: &[kiln_nix_store::derivation::ReferenceCandidate],
        _candidates_complete: bool,
        _bounds: NarBounds,
    ) -> PortResult<Option<StoreObjectFacts>> {
        Ok(self.store_facts.then(|| StoreObjectFacts {
            nar_sha256: "e".repeat(64),
            nar_size: 9,
            references: Vec::new(),
            matches: Vec::new(),
            complete: true,
        }))
    }

    fn read_log(
        &self,
        _path: &Path,
        offset: usize,
        max_bytes: usize,
        _total_bound: usize,
    ) -> PortResult<(Vec<u8>, bool)> {
        let log = b"mantle-log";
        let bytes = log
            .get(offset..)
            .unwrap_or_default()
            .iter()
            .copied()
            .take(max_bytes)
            .collect::<Vec<_>>();
        Ok((bytes, offset.saturating_add(max_bytes) >= log.len()))
    }
}

// r[verify kiln.mantle.profile.scenario.match]
// r[verify kiln.mantle.request.scenario.valid]
// r[verify kiln.mantle.completion.scenario.success]
#[test]
fn exact_native_report_admits_success_repeat_logs_and_reconciliation() {
    let runtime = FakeRuntime::with_outputs([exited(success_report(false, true))]);
    let mut adapter = MantleAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
    let first = adapter.dispatch(&plan()).unwrap_or_else(|_| unreachable!());
    let repeated = adapter.dispatch(&plan()).unwrap_or_else(|_| unreachable!());
    let reconciled = adapter
        .reconcile(&plan())
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(first.outcome, BuildEvaluationOutcome::Success);
    assert_eq!(first.products.len(), plan().required_products.len());
    assert_eq!(first, repeated);
    assert_eq!(first, reconciled);
    assert_eq!(adapter.runtime().runs, 1);
    let invocation = adapter
        .runtime()
        .invocations
        .first()
        .unwrap_or_else(|| unreachable!());
    assert_eq!(
        invocation.program,
        PathBuf::from(PINNED_MANTLE_EXECUTABLE_PATH)
    );
    assert_eq!(
        invocation.args,
        [
            "--store",
            "/var/lib/kiln/mantle-store",
            "--store-prefix",
            "/mantle/store",
            "--state-dir",
            "/var/lib/kiln/mantle-state",
            "--json",
            "build",
            "--plan",
            "/etc/kiln/mantle-plan.ncl",
        ]
        .map(std::ffi::OsString::from)
    );

    let log_identity = first
        .log_identities
        .first()
        .cloned()
        .unwrap_or_else(|| unreachable!());
    let chunk = adapter
        .read_log(&BuildLogReadRequest {
            effect_id: plan().effect_id,
            log_identity,
            offset: 0,
            max_bytes: MAX_LOG_BYTES,
        })
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(chunk.bytes, b"mantle-log");
    assert!(chunk.end_of_log);
}

// r[verify kiln.mantle.completion.scenario.cache]
#[test]
fn cache_cannot_hide_a_missing_required_product() {
    let runtime = FakeRuntime::with_outputs([exited(success_report(true, false))]);
    let mut adapter = MantleAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
    let observation = adapter.dispatch(&plan()).unwrap_or_else(|_| unreachable!());
    assert_eq!(observation.outcome, BuildEvaluationOutcome::ProviderError);
    assert!(observation.products.is_empty());
}

// r[verify kiln.mantle.request.scenario.substitution]
// r[verify kiln.mantle.reconciliation.scenario.mismatch]
#[test]
fn repeated_effect_rejects_candidate_and_plan_substitution() {
    let runtime = FakeRuntime::with_outputs([exited(success_report(false, true))]);
    let mut adapter = MantleAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
    assert!(adapter.dispatch(&plan()).is_ok());

    let mut stale = plan();
    stale.candidate_id = CandidateId::new("candidate-b").unwrap_or_else(|_| unreachable!());
    assert!(adapter.dispatch(&stale).is_err());

    let mut changed_plan = plan();
    changed_plan.plan_identity = content('f');
    assert!(adapter.dispatch(&changed_plan).is_err());
    assert_eq!(adapter.runtime().runs, 1);
}

// r[verify kiln.mantle.profile.scenario.drift]
// r[verify kiln.mantle.cohort_refresh.drift]
#[test]
fn profile_and_file_identity_drift_fail_before_dispatch() {
    let mut revision = profile();
    revision.revision = "ffffffffffffffffffffffffffffffffffffffff".to_string();
    assert!(admit_profile(&revision).is_err());

    let mut capability = profile();
    capability
        .provider
        .operations
        .remove(&kiln_core::provider::Operation::Cancel);
    assert!(admit_profile(&capability).is_err());

    let mut runtime = FakeRuntime::with_outputs([exited(success_report(false, true))]);
    runtime.executable_identity = content('f');
    assert!(MantleAdapter::new(profile(), runtime).is_err());
}

// r[verify kiln.mantle.reconciliation.scenario.disconnect]
#[test]
fn timeout_cancellation_and_unknown_remain_distinct_without_redispatch() {
    for (process, expected) in [
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
    ] {
        let runtime = FakeRuntime::with_outputs([process]);
        let mut adapter = MantleAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
        let cancellation = AtomicBool::new(expected == BuildEvaluationOutcome::Cancelled);
        let observation = adapter
            .dispatch_with_cancellation(&plan(), &cancellation)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(observation.outcome, expected);
        assert_eq!(
            adapter
                .reconcile(&plan())
                .unwrap_or_else(|_| unreachable!()),
            observation
        );
        assert_eq!(adapter.runtime().runs, 1);
    }
}

#[test]
fn restored_unknown_reconciles_without_process_redispatch() {
    let first_runtime = FakeRuntime::with_outputs([MantleProcessOutput::Unknown]);
    let mut first = MantleAdapter::new(profile(), first_runtime).unwrap_or_else(|_| unreachable!());
    let unknown = first.dispatch(&plan()).unwrap_or_else(|_| unreachable!());

    let runtime = FakeRuntime::with_outputs(Vec::new());
    let mut restored = MantleAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
    assert_eq!(restored.restore(&plan(), &unknown), Ok(()));
    assert_eq!(
        restored
            .reconcile(&plan())
            .unwrap_or_else(|_| unreachable!()),
        unknown
    );
    assert_eq!(restored.runtime().runs, 0);
}

#[derive(Debug, Deserialize)]
struct ProducerFixtures {
    schema: String,
    request: WireRequest,
    success: WireObservation,
    cache_success: WireObservation,
    failure: WireObservation,
}

// r[verify kiln.mantle.conformance]
// r[verify kiln.mantle.conformance.scenario.external]
#[test]
fn independent_consumer_accepts_exact_producer_fixtures() {
    let fixtures: ProducerFixtures = serde_json::from_slice(include_bytes!(concat!(
        "../../../fixtures/mantle/build-contract/",
        "d75158b00b080535f2ffea12430665ca3d57ea82/producer-fixtures-v1.json"
    )))
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(fixtures.schema, "mantle.build-producer-fixtures.v1");
    assert_eq!(validate_request(&fixtures.request), Ok(()));
    for observation in [
        &fixtures.success,
        &fixtures.cache_success,
        &fixtures.failure,
    ] {
        assert_eq!(
            validate_observation(
                &fixtures.request,
                &fixtures.success.builder_identity,
                observation
            ),
            Ok(())
        );
    }
}

#[test]
fn bounded_runtime_hashes_files_and_enforces_log_bounds() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let file = directory.path().join("mantle-log");
    std::fs::write(&file, b"mantle-log").unwrap_or_else(|_| unreachable!());
    let runtime = BoundedMantleRuntime;
    let identity = runtime
        .file_identity(&file, MAX_LOG_BYTES as u64)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        identity,
        runtime
            .file_identity(&file, MAX_LOG_BYTES as u64)
            .unwrap_or_else(|_| unreachable!())
    );
    let (bytes, end) = runtime
        .read_log(&file, 0, MAX_LOG_BYTES, MAX_LOG_BYTES)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(bytes, b"mantle-log");
    assert!(end);
    assert!(
        runtime
            .read_log(&file, 0, MAX_LOG_BYTES + 1, MAX_LOG_BYTES)
            .is_err()
    );
    assert!(
        runtime
            .file_identity(Path::new("relative"), MAX_LOG_BYTES as u64)
            .is_err()
    );
}

// r[verify kiln.mantle.nonclaims.scenario.scope]
#[test]
fn core_and_provider_manifests_do_not_import_mantle_or_bounded_exec() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let core_manifest = std::fs::read_to_string(root.join("crates/kiln-core/Cargo.toml"))
        .unwrap_or_else(|_| unreachable!());
    let provider_manifest =
        std::fs::read_to_string(root.join("crates/kiln-provider-api/Cargo.toml"))
            .unwrap_or_else(|_| unreachable!());
    for forbidden in [
        "mantle-build-contract",
        "bounded-exec",
        "kiln-adapter-mantle",
    ] {
        assert!(!core_manifest.contains(forbidden));
        assert!(!provider_manifest.contains(forbidden));
    }
}

// r[verify kiln.mantle.native_report.surface.complete]
// r[verify kiln.mantle.native_report.facts.metrics]
#[test]
fn complete_native_report_admits_every_native_collection() {
    let mut report = success_report_value(false, true);
    for field in [
        "hermeticity_audit_events",
        "build_environment_reports",
        "finish_gates",
        "network_policy_reports",
        "workspace_reports",
        "action_result_reports",
        "native_dynamic_plans",
        "plan_output_bindings",
        "scheduler_priority_decisions",
        "overlay_base_generations",
        "store_layer_selections",
        "remote_telemetry_events",
        "remote_observability",
        "frontend_artifact_attestations",
        "ast_grep_structural_evidence",
        "ast_grep_structural_evidence_diagnostics",
        "cargo_build_evidence",
        "cargo_build_evidence_diagnostics",
        "diagnostic_persistence_failures",
    ] {
        insert(&mut report, field, serde_json::json!([{}]));
    }
    insert(
        &mut report,
        "overlay_plan_blake3",
        serde_json::json!("a".repeat(IDENTITY_HEX_BYTES)),
    );

    let observation = observe_report(report);
    assert_eq!(observation.outcome, BuildEvaluationOutcome::Success);
    for name in [
        "native-hermeticity-audit-events",
        "native-build-environment-reports",
        "native-finish-gates",
        "native-network-policy-reports",
        "native-workspace-reports",
        "native-action-result-reports",
        "native-dynamic-plans",
        "native-plan-output-bindings",
        "native-scheduler-priority-decisions",
        "native-overlay-base-generations",
        "native-store-layer-selections",
        "native-remote-telemetry-events",
        "native-remote-observability",
        "native-frontend-artifact-attestations",
        "native-ast-grep-structural-evidence",
        "native-ast-grep-structural-evidence-diagnostics",
        "native-cargo-build-evidence",
        "native-cargo-build-evidence-diagnostics",
        "native-diagnostic-persistence-failures",
    ] {
        assert_eq!(metric_value(&observation, name), 1, "{name}");
    }
    assert_eq!(metric_value(&observation, "native-fod-mismatches"), 0);
    assert_eq!(metric_value(&observation, "native-output-count"), 2);
    assert_eq!(metric_value(&observation, "native-log-count"), 1);
    assert_eq!(
        metric_value(&observation, "native-required-product-count"),
        2
    );
    assert_eq!(metric_value(&observation, "native-overlay-plan-present"), 1);
    assert_eq!(
        metric_value(&observation, "native-hermeticity-practical"),
        1
    );
    assert_eq!(metric_value(&observation, "native-hermeticity-strict"), 0);
    assert_eq!(metric_value(&observation, "native-hermeticity-impure"), 0);
}

// r[verify kiln.mantle.native_report.surface.drift]
#[test]
fn native_report_rejects_unknown_or_missing_top_level_fields() {
    let mut unknown = success_report_value(false, true);
    insert(&mut unknown, "future_field", serde_json::json!(true));
    assert!(report_is_rejected(unknown));

    let mut missing = success_report_value(false, true);
    missing
        .as_object_mut()
        .unwrap_or_else(|| unreachable!())
        .remove("hermeticity_mode");
    assert!(report_is_rejected(missing));

    let mut missing_collection = success_report_value(false, true);
    missing_collection
        .as_object_mut()
        .unwrap_or_else(|| unreachable!())
        .remove("scheduler_priority_decisions");
    assert!(report_is_rejected(missing_collection));
}

// r[verify kiln.mantle.native_report.surface.complete]
#[test]
fn optional_native_collections_may_be_omitted() {
    let mut report = success_report_value(false, true);
    for field in [
        "action_result_reports",
        "finish_gates",
        "plan_output_bindings",
        "overlay_plan_blake3",
        "remote_observability",
        "remote_telemetry_events",
    ] {
        report
            .as_object_mut()
            .unwrap_or_else(|| unreachable!())
            .remove(field);
    }
    let observation = observe_report(report);
    assert_eq!(observation.outcome, BuildEvaluationOutcome::Success);
    assert_eq!(
        metric_value(&observation, "native-action-result-reports"),
        0
    );
}

// r[verify kiln.mantle.native_report.store_binding.match]
// r[verify kiln.mantle.native_report.store_binding.drift]
#[test]
fn native_report_binds_distinct_logical_and_physical_roots() {
    let observation = observe_report(success_report_value(false, true));
    assert_eq!(observation.outcome, BuildEvaluationOutcome::Success);

    for (field, value) in [
        ("store_dir", "/mantle/other"),
        ("output_dir", "/var/lib/kiln/other-store"),
        ("state_dir", "/var/lib/kiln/other-state"),
    ] {
        let mut report = success_report_value(false, true);
        set(&mut report, &format!("/{field}"), serde_json::json!(value));
        assert!(report_is_rejected(report), "{field}");
    }
}

// r[verify kiln.mantle.native_report.paths.valid]
// r[verify kiln.mantle.native_report.paths.escape]
#[test]
fn native_report_paths_must_stay_inside_their_declared_roots() {
    for pointer in [
        "/outcomes/0/outputs/0/path",
        "/outcomes/0/outputs/0/artifact_attestation/logical_path",
        "/outcomes/0/outputs/0/artifact_attestation/path",
        "/outcomes/0/log_file",
    ] {
        let mut report = success_report_value(false, true);
        *report
            .pointer_mut(pointer)
            .unwrap_or_else(|| unreachable!()) = serde_json::json!("/tmp/escape");
        assert!(report_is_rejected(report), "{pointer}");
    }
}

// r[verify kiln.mantle.native_report.counts.valid]
// r[verify kiln.mantle.native_report.counts.contradiction]
#[test]
fn native_counts_and_success_facts_must_agree() {
    let mut failed_count = success_report_value(false, true);
    set(
        &mut failed_count,
        "/counts/failed_total",
        serde_json::json!(1),
    );
    assert!(report_is_rejected(failed_count));

    let mut cached_count = success_report_value(true, true);
    set(
        &mut cached_count,
        "/counts/cached_total",
        serde_json::json!(0),
    );
    assert!(report_is_rejected(cached_count));

    let mut mismatch = success_report_value(false, true);
    set(
        &mut mismatch,
        "/counts",
        serde_json::json!({
            "succeeded_total": 0,
            "built_total": 0,
            "cached_total": 0,
            "failed_total": 0
        }),
    );
    set(&mut mismatch, "/outcomes", serde_json::json!([]));
    set(
        &mut mismatch,
        "/fod_mismatches",
        serde_json::json!([{
            "name": "binary",
            "expected_sri": "sha256-expected",
            "actual_sri": "sha256-actual"
        }]),
    );
    assert!(report_is_rejected(mismatch));
}

// r[verify kiln.mantle.native_report.failures.log]
// r[verify kiln.mantle.native_report.failures.vocabulary]
#[test]
fn native_failure_logs_are_bound_and_unknown_vocabulary_is_rejected() {
    let mut report = success_report_value(false, true);
    set(
        &mut report,
        "/counts",
        serde_json::json!({
            "succeeded_total": 0,
            "built_total": 0,
            "cached_total": 0,
            "failed_total": 1
        }),
    );
    set(&mut report, "/outcomes", serde_json::json!([]));
    set(
        &mut report,
        "/failed",
        serde_json::json!([{
            "root": "demo",
            "drv_key": "/mantle/store/demo.drv",
            "phase": "build",
            "error_class": "sandbox",
            "message": "sandbox failed",
            "saved_log_path": "/var/lib/kiln/mantle-state/logs/demo.drv.log"
        }]),
    );

    let runtime = FakeRuntime::with_outputs([exited_with_status(
        1,
        serde_json::to_vec(&report).unwrap_or_else(|_| unreachable!()),
    )]);
    let mut adapter = MantleAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
    let observation = adapter.dispatch(&plan()).unwrap_or_else(|_| unreachable!());
    assert_eq!(observation.outcome, BuildEvaluationOutcome::Failed);
    assert!(observation.products.is_empty());
    assert_eq!(observation.log_identities.len(), 1);
    let log_identity = observation
        .log_identities
        .first()
        .cloned()
        .unwrap_or_else(|| unreachable!());
    let chunk = adapter
        .read_log(&BuildLogReadRequest {
            effect_id: plan().effect_id,
            log_identity,
            offset: 0,
            max_bytes: MAX_LOG_BYTES,
        })
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(chunk.bytes, b"mantle-log");

    let mut unknown = report;
    set(
        &mut unknown,
        "/failed/0/error_class",
        serde_json::json!("future-class"),
    );
    assert!(report_is_rejected(unknown));
}

// r[verify kiln.mantle.native_report.failures.log]
// r[verify kiln.mantle.native_report.surface.complete]
#[test]
fn captured_native_failure_report_is_admitted() {
    let bytes = include_bytes!(concat!(
        "../../../fixtures/mantle/native-report/",
        "failure-report-v1.json"
    ));
    let runtime = FakeRuntime::with_outputs([exited_with_status(1, bytes.to_vec())]);
    let mut adapter = MantleAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
    let observation = adapter.dispatch(&plan()).unwrap_or_else(|_| unreachable!());
    assert_eq!(observation.outcome, BuildEvaluationOutcome::Failed);
    assert!(observation.products.is_empty());
    assert_eq!(observation.log_identities.len(), 1);
    assert_eq!(
        metric_value(&observation, "native-build-environment-reports"),
        1
    );
    assert_eq!(
        metric_value(&observation, "native-network-policy-reports"),
        1
    );
    assert_eq!(
        metric_value(&observation, "native-action-result-reports"),
        1
    );
    assert_eq!(
        metric_value(&observation, "native-scheduler-priority-decisions"),
        1
    );
    assert_eq!(
        metric_value(&observation, "native-remote-telemetry-events"),
        1
    );
    assert_eq!(metric_value(&observation, "failed-total"), 1);
    assert_eq!(
        metric_value(&observation, "native-hermeticity-practical"),
        1
    );
}

// r[verify kiln.mantle.native_report.facts.metrics]
#[test]
fn native_substitution_facts_are_recorded() {
    let observation = observe_report(success_report_value(true, true));
    assert_eq!(observation.cache_kind, BuildCacheKind::Substitution);
    assert_eq!(
        metric_value(&observation, "native-substitution-output-count"),
        1
    );
    assert_eq!(
        metric_value(&observation, "native-substitution-transferred-bytes"),
        55
    );
    assert_eq!(
        metric_value(&observation, "native-substitution-reused-bytes"),
        0
    );
    assert_eq!(
        metric_value(&observation, "native-substitution-fallback-count"),
        1
    );
}

fn observe_report(report: serde_json::Value) -> BuildEvaluationObservation {
    let runtime = FakeRuntime::with_outputs([exited(
        serde_json::to_vec(&report).unwrap_or_else(|_| unreachable!()),
    )]);
    let mut adapter = MantleAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
    adapter.dispatch(&plan()).unwrap_or_else(|_| unreachable!())
}

fn set(document: &mut serde_json::Value, pointer: &str, value: serde_json::Value) {
    *document
        .pointer_mut(pointer)
        .unwrap_or_else(|| unreachable!("missing JSON pointer {pointer}")) = value;
}

fn insert(document: &mut serde_json::Value, key: &str, value: serde_json::Value) {
    document
        .as_object_mut()
        .unwrap_or_else(|| unreachable!("JSON document is not an object"))
        .insert(key.to_string(), value);
}

fn report_is_rejected(report: serde_json::Value) -> bool {
    let runtime = FakeRuntime::with_outputs([exited(
        serde_json::to_vec(&report).unwrap_or_else(|_| unreachable!()),
    )]);
    let mut adapter = MantleAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
    adapter
        .dispatch(&plan())
        .is_ok_and(|observation| observation.outcome == BuildEvaluationOutcome::ProviderError)
}

fn metric_value(observation: &BuildEvaluationObservation, name: &str) -> u64 {
    observation
        .metrics
        .iter()
        .find(|metric| metric.name.as_str() == name)
        .unwrap_or_else(|| unreachable!("metric {name} missing"))
        .value
}

// r[verify kiln.nix_store.nar.scenario.product]
#[test]
fn mantle_products_carry_nar_facts_when_the_runtime_observes_them() {
    let mut runtime = FakeRuntime::with_outputs([exited(success_report(false, true))]);
    runtime.store_facts = true;
    let mut adapter = MantleAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
    let observation = adapter.dispatch(&plan()).unwrap_or_else(|_| unreachable!());
    assert_eq!(observation.products.len(), 2);
    for product in &observation.products {
        let store = product
            .store
            .as_ref()
            .unwrap_or_else(|| unreachable!("store facts absent"));
        assert_eq!(store.nar_sha256, "e".repeat(64));
        assert_eq!(store.nar_size, 9);
        assert!(store.references.is_empty());
    }
}
