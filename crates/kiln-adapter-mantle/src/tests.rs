use std::cell::Cell;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use kiln_core::build::{BuildEffectPlan, BuildEvaluationOutcome, BuildLogReadRequest};
use kiln_core::types::{
    Alias, AttemptId, CandidateId, ContentId, EffectId, PipelineRevisionId, ProviderProfileId,
};
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
const MAX_METRICS: usize = 8;
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
    }
}

fn success_report(cached: bool, include_metadata: bool) -> Vec<u8> {
    let mut outputs = vec![serde_json::json!({
        "name": "binary",
        "path": "/var/lib/kiln/mantle-store/binary",
        "artifact_attestation": {
            "logical_path": "attestations/binary.json",
            "path": "/var/lib/kiln/mantle-store/attestations/binary.json"
        },
        "substitution": if cached { Some(serde_json::json!({"mode": "copy"})) } else { None }
    })];
    if include_metadata {
        outputs.push(serde_json::json!({
            "name": "metadata",
            "path": "/var/lib/kiln/mantle-store/metadata",
            "artifact_attestation": {
                "logical_path": "attestations/metadata.json",
                "path": "/var/lib/kiln/mantle-store/attestations/metadata.json"
            },
            "substitution": null
        }));
    }
    serde_json::to_vec(&serde_json::json!({
        "schema": "crunch-build-report-v1",
        "store_dir": "/var/lib/kiln/mantle-store",
        "counts": {
            "succeeded_total": 1,
            "built_total": if cached { 0 } else { 1 },
            "cached_total": if cached { 1 } else { 0 },
            "failed_total": 0
        },
        "outcomes": [{
            "cached": cached,
            "log_file": "/var/log/kiln/mantle-a.log",
            "outputs": outputs
        }],
        "failed": []
    }))
    .unwrap_or_else(|_| unreachable!())
}

fn exited(bytes: Vec<u8>) -> MantleProcessOutput {
    MantleProcessOutput::Exited {
        process_status: Some(SUCCESS_STATUS),
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
        "0bbab77e23ab95caa4e4184b9945cfaa5c8040e2/producer-fixtures-v1.json"
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
