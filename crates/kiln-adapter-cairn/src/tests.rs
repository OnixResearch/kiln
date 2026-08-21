use std::cell::Cell;
use std::collections::{BTreeMap, VecDeque};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use bounded_exec::ExecutionLimits;
use cairn_gate_contract::{
    GateRequest as CairnGateRequest, GateResult as CairnGateResult, Identity, result_identity,
    validate_request as validate_cairn_request, validate_result as validate_cairn_result,
};
use kiln_core::gating::{
    CandidateChange, CandidateSet, GateEffectPlan, GateEvaluationOutcome, GateTarget,
    LifecycleGateStage,
};
use kiln_core::types::{
    CandidateId, ContentId, EffectId, ExternalRef, PipelineRevisionId, PolicyId, ProviderProfileId,
};
use kiln_provider_api::{GatePort, PortResult};
use serde::Deserialize;

use super::*;

const IDENTITY_HEX_BYTES: usize = 64;
const MAX_CHANGES: usize = 8;
const MAX_EDGES: usize = 16;
const MAX_EVIDENCE: usize = 16;
const MAX_FINDINGS: usize = 16;
const MAX_REQUEST_BYTES: usize = 1_048_576;
const MAX_RESULT_BYTES: usize = 1_048_576;
const MAX_STDERR_BYTES: usize = 65_536;
const MAX_DIAGNOSTIC_BYTES: usize = 4_096;
const MAX_EXECUTABLE_BYTES: u64 = 268_435_456;
const MAX_IN_FLIGHT: usize = 4;
const STDIN_MAX_BYTES: usize = 1;
const TIMEOUT_MILLISECONDS: u64 = 30_000;
const SHORT_TIMEOUT_MILLISECONDS: u64 = 20;
const POLL_INTERVAL_MILLISECONDS: u64 = 5;
const TEARDOWN_TIMEOUT_MILLISECONDS: u64 = 1_000;
const CHILD_SLEEP_MILLISECONDS: u64 = 5_000;
const SUCCESS_STATUS: i32 = 0;
const CHILD_MODE_ENV: &str = "KILN_CAIRN_RUNTIME_CHILD";
const CHILD_MODE_VALUE: &str = "sleep";

fn content(character: char) -> ContentId {
    ContentId::from_blake3_hex(character.to_string().repeat(IDENTITY_HEX_BYTES))
        .unwrap_or_else(|_| unreachable!())
}

fn limits() -> CairnAdapterLimits {
    CairnAdapterLimits {
        max_changes: MAX_CHANGES,
        max_edges: MAX_EDGES,
        max_evidence_identities: MAX_EVIDENCE,
        max_findings: MAX_FINDINGS,
        max_request_bytes: MAX_REQUEST_BYTES,
        max_result_bytes: MAX_RESULT_BYTES,
        max_stderr_bytes: MAX_STDERR_BYTES,
        max_diagnostic_bytes: MAX_DIAGNOSTIC_BYTES,
        max_executable_bytes: MAX_EXECUTABLE_BYTES,
        max_in_flight: MAX_IN_FLIGHT,
        stdin_max_bytes: STDIN_MAX_BYTES,
        timeout_milliseconds: TIMEOUT_MILLISECONDS,
        poll_interval_milliseconds: POLL_INTERVAL_MILLISECONDS,
        teardown_timeout_milliseconds: TEARDOWN_TIMEOUT_MILLISECONDS,
    }
}

fn profile() -> CairnProfile {
    CairnProfile::pinned(
        ProviderProfileId::new("cairn-gate-v1").unwrap_or_else(|_| unreachable!()),
        PathBuf::from(PINNED_CAIRN_EXECUTABLE_PATH),
        PathBuf::from("/tmp/kiln-cairn-machine-fixture"),
        PathBuf::from("/var/lib/kiln/cairn-policy.json"),
        limits(),
    )
    .unwrap_or_else(|_| unreachable!())
}

fn candidate() -> CandidateSet {
    let pipeline_revision =
        PipelineRevisionId::new("pipeline-a").unwrap_or_else(|_| unreachable!());
    let change =
        ExternalRef::new("source", "change", "change-a").unwrap_or_else(|_| unreachable!());
    CandidateSet {
        id: CandidateId::new("candidate-a").unwrap_or_else(|_| unreachable!()),
        ordered_changes: vec![CandidateChange {
            change: change.clone(),
            target_revision: ExternalRef::new("source", "revision", "revision-a")
                .unwrap_or_else(|_| unreachable!()),
            source_snapshot: content('a'),
            pipeline_revision,
            collaboration_provider: ProviderProfileId::new("collaboration-a")
                .unwrap_or_else(|_| unreachable!()),
        }],
        dependencies: Vec::new(),
        frozen_sources: BTreeMap::from([(change, content('a'))]),
        policy_id: PolicyId::new("policy-a").unwrap_or_else(|_| unreachable!()),
    }
}

fn plan() -> GateEffectPlan {
    GateEffectPlan {
        effect_id: EffectId::new("gate-effect-a").unwrap_or_else(|_| unreachable!()),
        idempotency_key: content('b'),
        candidate: candidate(),
        candidate_root: content('c'),
        pipeline_revision: PipelineRevisionId::new("pipeline-a").unwrap_or_else(|_| unreachable!()),
        policy_identity: ContentId::from_blake3_hex(PINNED_CAIRN_POLICY_BLAKE3)
            .unwrap_or_else(|_| unreachable!()),
        evidence_identities: vec![content('d'), content('e')],
        provider_profile: ProviderProfileId::new("cairn-gate-v1")
            .unwrap_or_else(|_| unreachable!()),
        target: GateTarget {
            change: ExternalRef::new("cairn", "change", "add-cairn-gate-adapter")
                .unwrap_or_else(|_| unreachable!()),
            stage: LifecycleGateStage::Tasks,
        },
    }
}

fn pass_output() -> CairnProcessOutput {
    CairnProcessOutput::Exited {
        process_status: Some(SUCCESS_STATUS),
        stdout: include_bytes!(concat!(
            "../../../fixtures/cairn/gate-machine/",
            "15f00875562025e7ea7e0d1f4af24d1a2e2ac06f/tasks-pass.json"
        ))
        .to_vec(),
        stderr: Vec::new(),
    }
}

fn block_output() -> CairnProcessOutput {
    CairnProcessOutput::Exited {
        process_status: Some(SUCCESS_STATUS),
        stdout: include_bytes!(concat!(
            "../../../fixtures/cairn/gate-machine/",
            "15f00875562025e7ea7e0d1f4af24d1a2e2ac06f/tasks-block.json"
        ))
        .to_vec(),
        stderr: Vec::new(),
    }
}

#[derive(Debug)]
struct FakeRuntime {
    executable_identity: ContentId,
    identity_checks: Cell<usize>,
    outputs: VecDeque<CairnProcessOutput>,
    runs: usize,
}

impl FakeRuntime {
    fn with_outputs(outputs: impl IntoIterator<Item = CairnProcessOutput>) -> Self {
        Self {
            executable_identity: ContentId::from_blake3_hex(PINNED_CAIRN_EXECUTABLE_BLAKE3)
                .unwrap_or_else(|_| unreachable!()),
            identity_checks: Cell::new(0),
            outputs: outputs.into_iter().collect(),
            runs: 0,
        }
    }
}

impl CairnRuntime for FakeRuntime {
    fn executable_identity(&self, _path: &Path, _max_bytes: u64) -> PortResult<ContentId> {
        self.identity_checks
            .set(self.identity_checks.get().saturating_add(1));
        Ok(self.executable_identity.clone())
    }

    fn run(&mut self, _invocation: &ProcessInvocation) -> PortResult<CairnProcessOutput> {
        self.runs = self.runs.saturating_add(1);
        self.outputs.pop_front().ok_or_else(|| {
            kiln_provider_api::PortFailure::Unknown(kiln_provider_api::InfrastructureFailure {
                code: "fixture_output_missing".to_string(),
                message: "fixture has no process output".to_string(),
            })
        })
    }

    fn run_with_cancellation(
        &mut self,
        invocation: &ProcessInvocation,
        _cancellation: &AtomicBool,
    ) -> PortResult<CairnProcessOutput> {
        self.run(invocation)
    }
}

// r[verify kiln.cairn_gate.profile.scenario.match]
// r[verify kiln.cairn_gate.result.scenario.pass]
#[test]
fn exact_machine_fixture_admits_pass_repeat_and_reconciliation() {
    let runtime = FakeRuntime::with_outputs([pass_output()]);
    let mut adapter = CairnAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
    let first = adapter.evaluate(&plan()).unwrap_or_else(|_| unreachable!());
    let repeated = adapter.evaluate(&plan()).unwrap_or_else(|_| unreachable!());
    let reconciled = adapter
        .reconcile(&plan())
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(first.outcome, GateEvaluationOutcome::Passed);
    assert_eq!(first, repeated);
    assert_eq!(first, reconciled);
    assert_eq!(adapter.runtime().runs, 1);
}

#[test]
fn exact_machine_fixture_preserves_blocked_findings() {
    let runtime = FakeRuntime::with_outputs([block_output()]);
    let mut adapter = CairnAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
    let observation = adapter.evaluate(&plan()).unwrap_or_else(|_| unreachable!());
    assert_eq!(observation.outcome, GateEvaluationOutcome::Blocked);
    assert!(!observation.findings.is_empty());
    assert!(observation.result_identity.is_some());
    assert!(observation.receipt_identity.is_some());
}

// r[verify kiln.cairn_gate.result.scenario.stale]
#[test]
fn repeated_effect_rejects_candidate_and_policy_substitution() {
    let runtime = FakeRuntime::with_outputs([pass_output()]);
    let mut adapter = CairnAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
    assert!(adapter.evaluate(&plan()).is_ok());

    let mut candidate = plan();
    candidate.candidate_root = content('f');
    assert!(adapter.evaluate(&candidate).is_err());

    let mut policy = plan();
    policy.policy_identity = content('f');
    assert!(adapter.evaluate(&policy).is_err());
    assert_eq!(adapter.runtime().runs, 1);
}

#[test]
fn profile_rejects_revision_schema_and_limit_drift() {
    let mut revision = profile();
    revision.revision = "ffffffffffffffffffffffffffffffffffffffff".to_string();
    assert!(admit_profile(&revision).is_err());

    let mut schema = profile();
    schema.request_schema = "cairn.gate-request.v2".to_string();
    assert!(admit_profile(&schema).is_err());

    let mut limit = profile();
    limit.limits.max_findings = 0;
    assert!(admit_profile(&limit).is_err());
}

// r[verify kiln.cairn_gate.profile.scenario.executable]
#[test]
fn executable_substitution_fails_before_process_execution() {
    let mut runtime = FakeRuntime::with_outputs([pass_output()]);
    runtime.executable_identity = content('f');
    let adapter = CairnAdapter::new(profile(), runtime);
    assert!(adapter.is_err());
}

// r[verify kiln.cairn_gate.reconciliation.scenario.unknown]
#[test]
fn timeout_cancellation_and_unknown_remain_distinct_without_rerun() {
    for (process, expected) in [
        (
            CairnProcessOutput::TimedOut,
            GateEvaluationOutcome::TimedOut,
        ),
        (
            CairnProcessOutput::Cancelled,
            GateEvaluationOutcome::Cancelled,
        ),
        (CairnProcessOutput::Unknown, GateEvaluationOutcome::Unknown),
    ] {
        let runtime = FakeRuntime::with_outputs([process]);
        let mut adapter = CairnAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
        let observation = adapter.evaluate(&plan()).unwrap_or_else(|_| unreachable!());
        assert_eq!(observation.outcome, expected);
        assert!(observation.result_identity.is_none());
        assert!(observation.receipt_identity.is_none());
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
fn controlled_policy_error_preserves_diagnostic_class() {
    let machine_non_claims =
        serde_json::to_value(MACHINE_NON_CLAIMS).unwrap_or_else(|_| unreachable!());
    let output = CairnProcessOutput::Exited {
        process_status: Some(PINNED_MACHINE_POLICY_ERROR_STATUS),
        stdout: serde_json::to_vec(&serde_json::json!({
            "schema_version": "cairn.cli-machine.v1",
            "ok": false,
            "command": "gate",
            "exit_class": "policy_error",
            "result": null,
            "diagnostics": [{
                "code": "cairn.cli.policy_error",
                "severity": "error",
                "message": "Cairn rejected the selected runtime policy.",
                "target": "policy",
                "fix": "Refresh and validate the checked policy projection."
            }],
            "non_claims": machine_non_claims,
        }))
        .unwrap_or_else(|_| unreachable!()),
        stderr: Vec::new(),
    };
    let runtime = FakeRuntime::with_outputs([output]);
    let mut adapter = CairnAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
    let observation = adapter.evaluate(&plan()).unwrap_or_else(|_| unreachable!());
    assert_eq!(observation.outcome, GateEvaluationOutcome::PolicyError);
    assert_eq!(observation.findings.len(), 1);
    assert!(observation.result_identity.is_none());
}

#[test]
fn tampered_native_receipt_identity_is_rejected() {
    let mut machine: serde_json::Value = serde_json::from_slice(include_bytes!(concat!(
        "../../../fixtures/cairn/gate-machine/",
        "15f00875562025e7ea7e0d1f4af24d1a2e2ac06f/tasks-pass.json"
    )))
    .unwrap_or_else(|_| unreachable!());
    let receipt_hash = machine
        .get_mut("result")
        .and_then(serde_json::Value::as_object_mut)
        .and_then(|result| result.get_mut("receipt_hash"))
        .unwrap_or_else(|| unreachable!());
    *receipt_hash = serde_json::Value::String("f".repeat(IDENTITY_HEX_BYTES));
    let output = CairnProcessOutput::Exited {
        process_status: Some(SUCCESS_STATUS),
        stdout: serde_json::to_vec(&machine).unwrap_or_else(|_| unreachable!()),
        stderr: Vec::new(),
    };
    let runtime = FakeRuntime::with_outputs([output]);
    let mut adapter = CairnAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
    assert!(adapter.evaluate(&plan()).is_err());
}

#[test]
fn malformed_and_truncated_outputs_are_provider_errors() {
    let malformed = CairnProcessOutput::Exited {
        process_status: Some(SUCCESS_STATUS),
        stdout: b"not-json".to_vec(),
        stderr: Vec::new(),
    };
    for output in [malformed, CairnProcessOutput::OutputLimitExceeded] {
        let runtime = FakeRuntime::with_outputs([output]);
        let mut adapter = CairnAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
        let observation = adapter.evaluate(&plan()).unwrap_or_else(|_| unreachable!());
        assert_eq!(observation.outcome, GateEvaluationOutcome::ProviderError);
        assert!(observation.result_identity.is_none());
    }
}

// r[verify kiln.cairn_gate.nonclaims.scenario.scope]
#[test]
fn core_and_provider_manifests_do_not_import_cairn_or_bounded_exec() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let core_manifest = std::fs::read_to_string(root.join("crates/kiln-core/Cargo.toml"))
        .unwrap_or_else(|_| unreachable!());
    let provider_manifest =
        std::fs::read_to_string(root.join("crates/kiln-provider-api/Cargo.toml"))
            .unwrap_or_else(|_| unreachable!());
    for forbidden in ["cairn-gate-contract", "bounded-exec", "kiln-adapter-cairn"] {
        assert!(!core_manifest.contains(forbidden));
        assert!(!provider_manifest.contains(forbidden));
    }
}

#[test]
fn restored_unknown_reconciles_without_process_redispatch() {
    let runtime = FakeRuntime::with_outputs([CairnProcessOutput::Unknown]);
    let mut first = CairnAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
    let unknown = first.evaluate(&plan()).unwrap_or_else(|_| unreachable!());
    assert_eq!(unknown.outcome, GateEvaluationOutcome::Unknown);

    let runtime = FakeRuntime::with_outputs(Vec::new());
    let mut restored = CairnAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
    assert_eq!(restored.restore(&plan(), &unknown), Ok(()));
    assert_eq!(
        restored
            .reconcile(&plan())
            .unwrap_or_else(|_| unreachable!()),
        unknown
    );
    assert_eq!(restored.runtime().runs, 0);
}

#[test]
fn bounded_runtime_hashes_exact_file_and_rejects_missing_executable() {
    let runtime = BoundedCairnRuntime;
    let executable = std::env::current_exe().unwrap_or_else(|_| unreachable!());
    let first = runtime.executable_identity(&executable, MAX_EXECUTABLE_BYTES);
    let second = runtime.executable_identity(&executable, MAX_EXECUTABLE_BYTES);
    assert!(first.is_ok());
    assert_eq!(first, second);
    assert!(
        runtime
            .executable_identity(Path::new("/missing/kiln-cairn"), MAX_EXECUTABLE_BYTES)
            .is_err()
    );
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProducerFixtures {
    schema: String,
    request: CairnGateRequest,
    pass_result: CairnGateResult,
    block_result: CairnGateResult,
}

// r[verify kiln.cairn_gate.conformance]
// r[verify kiln.cairn_gate.conformance.scenario.stale]
#[test]
fn independent_consumer_accepts_producer_fixture_and_rejects_substitution() {
    let fixtures: ProducerFixtures = serde_json::from_slice(include_bytes!(concat!(
        "../../../fixtures/cairn/gate-contract/",
        "15f00875562025e7ea7e0d1f4af24d1a2e2ac06f/producer-fixtures-v1.json"
    )))
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(fixtures.schema, "cairn.gate-producer-fixtures.v1");
    assert_eq!(validate_cairn_request(&fixtures.request), Ok(()));
    assert_eq!(
        validate_cairn_result(&fixtures.request, &fixtures.pass_result),
        Ok(())
    );
    assert_eq!(
        validate_cairn_result(&fixtures.request, &fixtures.block_result),
        Ok(())
    );

    let mut substituted = fixtures.pass_result;
    substituted.applicability.candidate_root_identity =
        Identity::new(format!("b3:{}", "f".repeat(IDENTITY_HEX_BYTES)))
            .unwrap_or_else(|_| unreachable!());
    substituted.identity = result_identity(&substituted);
    assert!(validate_cairn_result(&fixtures.request, &substituted).is_err());
}

#[test]
fn bounded_runtime_preserves_timeout_and_cancellation() {
    if std::env::var(CHILD_MODE_ENV).as_deref() == Ok(CHILD_MODE_VALUE) {
        std::thread::sleep(Duration::from_millis(CHILD_SLEEP_MILLISECONDS));
        return;
    }
    let executable = std::env::current_exe().unwrap_or_else(|_| unreachable!());
    let current_dir = std::env::current_dir().unwrap_or_else(|_| unreachable!());
    let invocation = ProcessInvocation {
        program: executable,
        args: vec![
            OsString::from("--exact"),
            OsString::from("tests::bounded_runtime_preserves_timeout_and_cancellation"),
        ],
        current_dir,
        environment: vec![(
            OsString::from(CHILD_MODE_ENV),
            OsString::from(CHILD_MODE_VALUE),
        )],
        limits: ExecutionLimits {
            timeout_ms: SHORT_TIMEOUT_MILLISECONDS,
            stdin_max_bytes: STDIN_MAX_BYTES,
            stdout_max_bytes: MAX_RESULT_BYTES,
            stderr_max_bytes: MAX_STDERR_BYTES,
            poll_interval_ms: POLL_INTERVAL_MILLISECONDS,
            teardown_timeout_ms: TEARDOWN_TIMEOUT_MILLISECONDS,
        },
    };
    let mut runtime = BoundedCairnRuntime;
    assert_eq!(
        runtime.run(&invocation).unwrap_or_else(|_| unreachable!()),
        CairnProcessOutput::TimedOut
    );

    let cancellation = AtomicBool::new(true);
    assert!(cancellation.load(Ordering::SeqCst));
    assert_eq!(
        runtime
            .run_with_cancellation(&invocation, &cancellation)
            .unwrap_or_else(|_| unreachable!()),
        CairnProcessOutput::Cancelled
    );
}
