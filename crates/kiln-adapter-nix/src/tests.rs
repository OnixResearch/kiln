use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use kiln_core::build::{
    BuildEffectPlan, BuildEvaluationOutcome, BuildLogReadRequest, admit_build_evaluation,
};
use kiln_core::types::{
    Alias, AttemptId, CandidateId, ContentId, EffectId, PipelineRevisionId, ProviderProfileId,
};
use kiln_provider_api::{BuildPort, InfrastructureFailure, PortFailure, PortResult};

use super::*;
use crate::profile::admit_profile;

const IDENTITY_HEX_BYTES: usize = 64;
const STORE_PATH: &str = "/nix/store";
const DRV_PATH: &str = "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-fixture.drv";
const OUT_PATH: &str = "/nix/store/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb-fixture";
const OUTSIDE_PATH: &str = "/tmp/kiln-nix-outside";
const MAX_REQUIRED_PRODUCTS: usize = 8;
const MAX_PRODUCTS: usize = 8;
const MAX_REQUEST_BYTES: usize = 1_048_576;
const MAX_RESULT_BYTES: usize = 1_048_576;
const MAX_STDERR_BYTES: usize = 65_536;
const MAX_LOG_BYTES: usize = 65_536;
const MAX_EXECUTABLE_BYTES: u64 = 268_435_456;
const MAX_DERIVATION_BYTES: u64 = 1_048_576;
const MAX_IN_FLIGHT: usize = 4;
const STDIN_MAX_BYTES: usize = 1;
const TIMEOUT_MILLISECONDS: u64 = 30_000;
const POLL_INTERVAL_MILLISECONDS: u64 = 5;
const TEARDOWN_TIMEOUT_MILLISECONDS: u64 = 1_000;

fn content(character: char) -> ContentId {
    ContentId::from_blake3_hex(character.to_string().repeat(IDENTITY_HEX_BYTES))
        .unwrap_or_else(|_| unreachable!())
}

fn pinned_identity() -> ContentId {
    ContentId::from_blake3_hex(PINNED_NIX_EXECUTABLE_BLAKE3).unwrap_or_else(|_| unreachable!())
}

fn alias(value: &str) -> Alias {
    Alias::new(value).unwrap_or_else(|_| unreachable!())
}

fn limits() -> NixBuildLimits {
    NixBuildLimits {
        max_required_products: MAX_REQUIRED_PRODUCTS,
        max_products: MAX_PRODUCTS,
        max_logs: 1,
        max_request_bytes: MAX_REQUEST_BYTES,
        max_result_bytes: MAX_RESULT_BYTES,
        max_stderr_bytes: MAX_STDERR_BYTES,
        max_log_bytes: MAX_LOG_BYTES,
        max_executable_bytes: MAX_EXECUTABLE_BYTES,
        max_derivation_bytes: MAX_DERIVATION_BYTES,
        max_in_flight: MAX_IN_FLIGHT,
        stdin_max_bytes: STDIN_MAX_BYTES,
        timeout_milliseconds: TIMEOUT_MILLISECONDS,
        poll_interval_milliseconds: POLL_INTERVAL_MILLISECONDS,
        teardown_timeout_milliseconds: TEARDOWN_TIMEOUT_MILLISECONDS,
    }
}

fn profile() -> NixBuildProfile {
    NixBuildProfile::pinned(
        ProviderProfileId::new("nix-build-v1").unwrap_or_else(|_| unreachable!()),
        NixRuntimeBinding {
            executable_path: PathBuf::from(PINNED_NIX_EXECUTABLE_PATH),
            executable_identity: pinned_identity(),
            derivation_path: PathBuf::from(DRV_PATH),
            derivation_identity: content('2'),
            plan_identity: content('3'),
            policy_identity: content('4'),
            store_path: PathBuf::from(STORE_PATH),
            home_base: PathBuf::from("/tmp/kiln-nix-home"),
            worker_identity: content('5'),
            store_identity: content('6'),
            required_products: vec![alias("out")],
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
        plan_identity: content('3'),
        policy_identity: content('4'),
        provider_profile: ProviderProfileId::new("nix-build-v1").unwrap_or_else(|_| unreachable!()),
        platform: alias("x86_64-linux"),
        required_products: vec![alias("out")],
    }
}

fn native_json(outputs: &[(&str, &str)]) -> Vec<u8> {
    let outputs = outputs
        .iter()
        .map(|(name, path)| {
            (
                (*name).to_string(),
                serde_json::Value::String((*path).to_string()),
            )
        })
        .collect::<serde_json::Map<String, serde_json::Value>>();
    serde_json::to_vec(&serde_json::json!([{
        "drvPath": DRV_PATH,
        "outputs": outputs,
    }]))
    .unwrap_or_else(|_| unreachable!())
}

fn success_json() -> Vec<u8> {
    native_json(&[("out", OUT_PATH)])
}

fn exited(process_status: Option<i32>, stdout: Vec<u8>, stderr: Vec<u8>) -> NixBuildProcessOutput {
    NixBuildProcessOutput::Exited {
        process_status,
        stdout,
        stderr,
    }
}

#[derive(Debug)]
struct FakeRuntime {
    executable_identity: ContentId,
    derivation_identity: ContentId,
    outputs: VecDeque<NixBuildProcessOutput>,
    invocations: Vec<ProcessInvocation>,
    runs: usize,
}

impl FakeRuntime {
    fn with_outputs(outputs: impl IntoIterator<Item = NixBuildProcessOutput>) -> Self {
        Self {
            executable_identity: pinned_identity(),
            derivation_identity: content('2'),
            outputs: outputs.into_iter().collect(),
            invocations: Vec::new(),
            runs: 0,
        }
    }
}

impl NixBuildRuntime for FakeRuntime {
    fn file_identity(&self, path: &Path, _max_bytes: u64) -> PortResult<ContentId> {
        if path == Path::new(PINNED_NIX_EXECUTABLE_PATH) {
            Ok(self.executable_identity.clone())
        } else if path == Path::new(DRV_PATH) {
            Ok(self.derivation_identity.clone())
        } else {
            Ok(content('9'))
        }
    }

    fn run(&mut self, invocation: &ProcessInvocation) -> PortResult<NixBuildProcessOutput> {
        self.runs = self.runs.saturating_add(1);
        self.invocations.push(invocation.clone());
        self.outputs.pop_front().ok_or_else(|| {
            PortFailure::Unknown(InfrastructureFailure {
                code: "fixture_output_missing".to_string(),
                message: "fixture has no Nix process output".to_string(),
            })
        })
    }

    fn run_with_cancellation(
        &mut self,
        invocation: &ProcessInvocation,
        _cancellation: &AtomicBool,
    ) -> PortResult<NixBuildProcessOutput> {
        self.run(invocation)
    }
}

fn adapter_with(
    outputs: impl IntoIterator<Item = NixBuildProcessOutput>,
) -> NixBuildAdapter<FakeRuntime> {
    NixBuildAdapter::new(profile(), FakeRuntime::with_outputs(outputs))
        .unwrap_or_else(|_| unreachable!())
}

// r[verify kiln.nix_build.cohort] r[verify kiln.nix_build.nonclaims]
// r[verify kiln.nix_build.cohort.scenario.pinned] r[verify kiln.nix_build.nonclaims.scenario.scope]
#[test]
fn pinned_profile_is_admitted() {
    assert!(admit_profile(&profile()).is_ok());
}

// r[verify kiln.nix_build.request] r[verify kiln.nix_build.execution]
// r[verify kiln.nix_build.request.scenario.exact] r[verify kiln.nix_build.execution.scenario.invocation]
// r[verify kiln.nix_build.completion.scenario.success] r[verify kiln.nix_build.logs.scenario.read]
#[test]
fn successful_build_is_recorded_and_readable() {
    let mut adapter = adapter_with([exited(Some(0), success_json(), b"build log".to_vec())]);
    let plan = plan();
    let observation = adapter.dispatch(&plan).unwrap_or_else(|_| unreachable!());
    assert_eq!(observation.outcome, BuildEvaluationOutcome::Success);
    assert_eq!(observation.products.len(), 1);
    let product = observation
        .products
        .first()
        .unwrap_or_else(|| unreachable!());
    assert_eq!(product.name.as_str(), "out");
    assert_ne!(product.artifact_identity, product.store_identity);
    assert_eq!(observation.builder_identity, pinned_identity());
    assert_eq!(observation.worker_identity, content('5'));
    assert_eq!(observation.store_identity, content('6'));
    assert!(observation.receipt_identity.is_some());
    assert_eq!(observation.log_identities.len(), 1);
    let log_identity = observation
        .log_identities
        .first()
        .unwrap_or_else(|| unreachable!())
        .clone();
    let chunk = adapter
        .read_log(&BuildLogReadRequest {
            effect_id: plan.effect_id.clone(),
            log_identity,
            offset: 0,
            max_bytes: 1024,
        })
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(chunk.bytes, b"build log");
    assert!(chunk.end_of_log);

    let invocation = adapter
        .runtime()
        .invocations
        .first()
        .unwrap_or_else(|| unreachable!());
    assert_eq!(invocation.current_dir, PathBuf::from("/tmp/kiln-nix-home"));
    assert!(
        invocation
            .args
            .iter()
            .any(|value| value.to_str() == Some("build"))
    );
    let expression = invocation
        .args
        .last()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    assert_eq!(expression, format!("import {DRV_PATH}"));
    assert!(invocation.environment.iter().any(|(name, value)| {
        name.to_str() == Some("NIX_CONFIG") && value.to_str() == Some(PINNED_NIX_CONFIG)
    }));
    assert_eq!(
        adapter
            .observe(&plan.effect_id)
            .unwrap_or_else(|_| unreachable!()),
        observation
    );
    assert_eq!(
        adapter.reconcile(&plan).unwrap_or_else(|_| unreachable!()),
        observation
    );
}

// r[verify kiln.nix_build.reconciliation] r[verify kiln.nix_build.reconciliation.scenario.duplicate]
#[test]
fn duplicate_dispatch_reuses_the_record() {
    let mut adapter = adapter_with([exited(Some(0), success_json(), Vec::new())]);
    let plan = plan();
    let first = adapter.dispatch(&plan).unwrap_or_else(|_| unreachable!());
    let second = adapter.dispatch(&plan).unwrap_or_else(|_| unreachable!());
    assert_eq!(first, second);
    assert_eq!(adapter.runtime().runs, 1);
}

// r[verify kiln.nix_build.request] r[verify kiln.nix_build.reconciliation]
// r[verify kiln.nix_build.request.scenario.mismatch] r[verify kiln.nix_build.reconciliation.scenario.conflict]
#[test]
fn conflicting_or_mismatched_plans_do_not_execute() {
    let mut adapter = adapter_with([exited(Some(0), success_json(), Vec::new())]);
    let plan = plan();
    adapter.dispatch(&plan).unwrap_or_else(|_| unreachable!());
    let mut changed = plan.clone();
    changed.idempotency_key = content('7');
    assert!(adapter.dispatch(&changed).is_err());
    assert_eq!(adapter.runtime().runs, 1);

    let mut wrong_product = plan.clone();
    wrong_product.effect_id = EffectId::new("build-effect-b").unwrap_or_else(|_| unreachable!());
    wrong_product.required_products = vec![alias("dev")];
    assert!(adapter.dispatch(&wrong_product).is_err());
    assert_eq!(adapter.runtime().runs, 1);

    let mut wrong_profile = plan.clone();
    wrong_profile.effect_id = EffectId::new("build-effect-c").unwrap_or_else(|_| unreachable!());
    wrong_profile.provider_profile =
        ProviderProfileId::new("other-build-v1").unwrap_or_else(|_| unreachable!());
    assert!(adapter.dispatch(&wrong_profile).is_err());
}

// r[verify kiln.nix_build.completion] r[verify kiln.nix_build.completion.scenario.provider_error]
#[test]
fn missing_product_is_a_provider_error() {
    let mut adapter = adapter_with([exited(
        Some(0),
        native_json(&[("dev", OUT_PATH)]),
        b"missing out".to_vec(),
    )]);
    let observation = adapter.dispatch(&plan()).unwrap_or_else(|_| unreachable!());
    assert_eq!(observation.outcome, BuildEvaluationOutcome::ProviderError);
    assert!(observation.products.is_empty());
    assert!(observation.receipt_identity.is_some());
}

// r[verify kiln.nix_build.completion] r[verify kiln.nix_build.completion.scenario.provider_error]
#[test]
fn malformed_or_outside_store_output_is_a_provider_error() {
    let mut malformed = adapter_with([exited(Some(0), b"not json".to_vec(), Vec::new())]);
    assert_eq!(
        malformed
            .dispatch(&plan())
            .unwrap_or_else(|_| unreachable!())
            .outcome,
        BuildEvaluationOutcome::ProviderError
    );

    let mut outside = adapter_with([exited(
        Some(0),
        native_json(&[("out", OUTSIDE_PATH)]),
        Vec::new(),
    )]);
    assert_eq!(
        outside
            .dispatch(&plan())
            .unwrap_or_else(|_| unreachable!())
            .outcome,
        BuildEvaluationOutcome::ProviderError
    );
}

// r[verify kiln.nix_build.completion] r[verify kiln.nix_build.completion.scenario.failed]
#[test]
fn nonzero_exit_is_failed_with_a_terminal_receipt() {
    let mut adapter = adapter_with([exited(Some(7), success_json(), b"builder failed".to_vec())]);
    let observation = adapter.dispatch(&plan()).unwrap_or_else(|_| unreachable!());
    assert_eq!(observation.outcome, BuildEvaluationOutcome::Failed);
    assert!(observation.products.is_empty());
    assert!(observation.receipt_identity.is_some());
}

// r[verify kiln.nix_build.execution] r[verify kiln.nix_build.execution.scenario.terminal]
#[test]
fn timeout_and_cancellation_are_terminal() {
    let mut timed_out = adapter_with([NixBuildProcessOutput::TimedOut {
        stdout: b"partial".to_vec(),
        stderr: b"deadline".to_vec(),
    }]);
    let timed_out_observation = timed_out
        .dispatch(&plan())
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        timed_out_observation.outcome,
        BuildEvaluationOutcome::TimedOut
    );
    assert!(timed_out_observation.receipt_identity.is_some());

    let mut cancelled = adapter_with([NixBuildProcessOutput::Cancelled {
        stdout: Vec::new(),
        stderr: b"cancelled".to_vec(),
    }]);
    let cancelled_observation = cancelled
        .dispatch(&plan())
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        cancelled_observation.outcome,
        BuildEvaluationOutcome::Cancelled
    );
    assert!(cancelled_observation.receipt_identity.is_some());
}

// r[verify kiln.nix_build.completion]
#[test]
fn unknown_outcome_has_no_terminal_receipt() {
    let mut adapter = adapter_with([NixBuildProcessOutput::Unknown]);
    let plan = plan();
    let observation = adapter.dispatch(&plan).unwrap_or_else(|_| unreachable!());
    assert_eq!(observation.outcome, BuildEvaluationOutcome::Unknown);
    assert!(observation.receipt_identity.is_none());
    assert_eq!(admit_build_evaluation(&plan, &observation), Ok(()));
}

// r[verify kiln.nix_build.cohort] r[verify kiln.nix_build.cohort.scenario.drift]
#[test]
fn executable_derivation_and_profile_drift_fail_before_dispatch() {
    let mut runtime = FakeRuntime::with_outputs([]);
    runtime.executable_identity = content('7');
    assert!(NixBuildAdapter::new(profile(), runtime).is_err());

    let mut runtime = FakeRuntime::with_outputs([]);
    runtime.derivation_identity = content('8');
    assert!(NixBuildAdapter::new(profile(), runtime).is_err());

    let mut version = profile();
    version.nix_version = "2.35.2".to_string();
    assert!(admit_profile(&version).is_err());

    let mut operations = profile();
    operations.provider.operations.clear();
    assert!(admit_profile(&operations).is_err());

    let mut executable = profile();
    executable.runtime.executable_identity = content('9');
    assert!(admit_profile(&executable).is_err());

    let mut nonclaims = profile();
    nonclaims.required_non_claims.pop();
    assert!(admit_profile(&nonclaims).is_err());

    let mut products = profile();
    products.runtime.required_products = vec![alias("out"), alias("dev")];
    assert!(admit_profile(&products).is_err());
}

// r[verify kiln.nix_build.logs] r[verify kiln.nix_build.logs.scenario.read] r[verify kiln.nix_build.logs.scenario.reject]
#[test]
fn log_requests_are_identity_bound_and_bounded() {
    let mut adapter = adapter_with([exited(Some(0), success_json(), b"build log".to_vec())]);
    let plan = plan();
    let observation = adapter.dispatch(&plan).unwrap_or_else(|_| unreachable!());
    let log_identity = observation
        .log_identities
        .first()
        .unwrap_or_else(|| unreachable!())
        .clone();
    assert!(
        adapter
            .read_log(&BuildLogReadRequest {
                effect_id: plan.effect_id.clone(),
                log_identity: content('a'),
                offset: 0,
                max_bytes: 1,
            })
            .is_err()
    );
    assert!(
        adapter
            .read_log(&BuildLogReadRequest {
                effect_id: plan.effect_id.clone(),
                log_identity: log_identity.clone(),
                offset: 0,
                max_bytes: 0,
            })
            .is_err()
    );
    assert!(
        adapter
            .read_log(&BuildLogReadRequest {
                effect_id: plan.effect_id.clone(),
                log_identity: log_identity.clone(),
                offset: 0,
                max_bytes: MAX_LOG_BYTES + 1,
            })
            .is_err()
    );
    let chunk = adapter
        .read_log(&BuildLogReadRequest {
            effect_id: plan.effect_id.clone(),
            log_identity: log_identity.clone(),
            offset: 100,
            max_bytes: 10,
        })
        .unwrap_or_else(|_| unreachable!());
    assert!(chunk.bytes.is_empty());
    assert!(chunk.end_of_log);
    let chunk = adapter
        .read_log(&BuildLogReadRequest {
            effect_id: plan.effect_id.clone(),
            log_identity,
            offset: 6,
            max_bytes: 3,
        })
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(chunk.bytes, b"log");
    assert!(chunk.end_of_log);
}

// r[verify kiln.nix_build.reconciliation] r[verify kiln.nix_build.reconciliation.scenario.conflict]
#[test]
fn observation_only_restore_has_no_log_and_rejects_conflicts() {
    let mut first = adapter_with([exited(Some(0), success_json(), b"log".to_vec())]);
    let plan = plan();
    let observation = first.dispatch(&plan).unwrap_or_else(|_| unreachable!());
    let mut restored = NixBuildAdapter::new(profile(), FakeRuntime::with_outputs([]))
        .unwrap_or_else(|_| unreachable!());
    restored
        .restore(&plan, &observation)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        restored.dispatch(&plan).unwrap_or_else(|_| unreachable!()),
        observation
    );
    assert_eq!(restored.runtime().runs, 0);
    let log_identity = observation
        .log_identities
        .first()
        .unwrap_or_else(|| unreachable!())
        .clone();
    assert!(
        restored
            .read_log(&BuildLogReadRequest {
                effect_id: plan.effect_id.clone(),
                log_identity,
                offset: 0,
                max_bytes: 1,
            })
            .is_err()
    );
    let mut conflicting = observation.clone();
    conflicting.outcome = BuildEvaluationOutcome::Failed;
    assert!(restored.restore(&plan, &conflicting).is_err());
}

// r[verify kiln.nix_build.cohort] r[verify kiln.nix_build.cohort.scenario.drift]
#[test]
fn inconsistent_limits_are_rejected() {
    let mut product_bound = profile();
    product_bound.limits.max_products = 0;
    assert!(admit_profile(&product_bound).is_err());

    let mut log_count = profile();
    log_count.limits.max_logs = 2;
    assert!(admit_profile(&log_count).is_err());

    let mut log_bound = profile();
    log_bound.limits.max_log_bytes = log_bound.limits.max_stderr_bytes - 1;
    assert!(admit_profile(&log_bound).is_err());
}

#[cfg(feature = "nix-live")]
#[test]
fn live_nix_build_is_admitted() {
    let temp = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let home = temp.path().join("home");
    std::fs::create_dir(&home).unwrap_or_else(|_| unreachable!());
    let expression = r#"derivation { name = "kiln-nix-build-adapter-live"; system = "x86_64-linux"; builder = "/bin/sh"; args = [ "-c" "echo live > $out" ]; }"#;
    let derivation_expression = format!("({expression}).drvPath");
    let output = std::process::Command::new(PINNED_NIX_EXECUTABLE_PATH)
        .args([
            "--extra-experimental-features",
            "nix-command",
            "eval",
            "--raw",
            "--expr",
            &derivation_expression,
        ])
        .env_clear()
        .env("HOME", &home)
        .env("NIX_CONFIG", PINNED_NIX_CONFIG)
        .output()
        .unwrap_or_else(|_| unreachable!());
    assert!(output.status.success());
    let derivation_path = String::from_utf8(output.stdout)
        .unwrap_or_else(|_| unreachable!())
        .trim()
        .to_string();
    let derivation_bytes = std::fs::read(&derivation_path).unwrap_or_else(|_| unreachable!());
    let derivation_identity =
        ContentId::from_blake3_hex(blake3::hash(&derivation_bytes).to_hex().to_string())
            .unwrap_or_else(|_| unreachable!());

    let mut profile = profile();
    profile.runtime.derivation_path = PathBuf::from(&derivation_path);
    profile.runtime.derivation_identity = derivation_identity;
    profile.runtime.home_base = home.clone();
    let plan = plan();
    let mut adapter =
        NixBuildAdapter::new(profile, BoundedNixBuildRuntime).unwrap_or_else(|_| unreachable!());
    let observation = adapter.dispatch(&plan).unwrap_or_else(|_| unreachable!());
    assert_eq!(observation.outcome, BuildEvaluationOutcome::Success);
    assert_eq!(observation.products.len(), 1);

    let nix_store = PathBuf::from(PINNED_NIX_EXECUTABLE_PATH).with_file_name("nix-store");
    let outputs = std::process::Command::new(nix_store)
        .args(["-q", "--outputs", &derivation_path])
        .env_clear()
        .env("HOME", &home)
        .env("NIX_CONFIG", PINNED_NIX_CONFIG)
        .output()
        .unwrap_or_else(|_| unreachable!());
    assert!(outputs.status.success());
    let output_path = String::from_utf8(outputs.stdout)
        .unwrap_or_else(|_| unreachable!())
        .trim()
        .to_string();
    assert!(output_path.starts_with("/nix/store/"));
    let content = std::fs::read_to_string(output_path).unwrap_or_else(|_| unreachable!());
    assert_eq!(content, "live\n");
}
