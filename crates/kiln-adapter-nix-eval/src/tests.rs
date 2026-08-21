use std::collections::{BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use kiln_core::discovery::{
    DiscoveryBounds, DiscoveryEffectPlan, DiscoveryOptions, DiscoverySourceMode, DiscoveryTerminal,
    GcRootState, admit_discovery_observation,
};
use kiln_core::types::{
    Alias, BLAKE3_HEX_BYTES, ContentId, EffectId, EvaluationId, ExternalRef, PipelineRevisionId,
    PolicyId, ProjectId, ProviderProfileId,
};
use kiln_provider_api::{DiscoveryPort, PortFailure, PortResult};
use tempfile::tempdir;

use super::*;

const MAX_LINE_BYTES: usize = 16_384;
const MAX_TOTAL_STDOUT_BYTES: usize = 131_072;
const MAX_STDERR_BYTES: usize = 16_384;
const MAX_RECORDS: usize = 16;
const MAX_JOBS: usize = 8;
const MAX_JOB_ERRORS: usize = 8;
const MAX_METADATA_BYTES: usize = 4_096;
const MAX_DIAGNOSTIC_BYTES: usize = 4_096;
const MAX_OUTPUTS_PER_JOB: usize = 8;
const MAX_DEPENDENCIES_PER_JOB: usize = 16;
const MAX_TASKS: usize = 8;
const MAX_GC_ROOTS: usize = 16;
const MAX_EXECUTABLE_BYTES: u64 = 268_435_456;
const MAX_MANIFEST_BYTES: u64 = 65_536;
const MAX_IN_FLIGHT: usize = 4;
const MAX_WORKERS: usize = 4;
const MAX_MEMORY_MEBIBYTES: u64 = 4_096;
const STDIN_MAX_BYTES: usize = 1;
const TIMEOUT_MILLISECONDS: u64 = 60_000;
const POLL_INTERVAL_MILLISECONDS: u64 = 10;
const TEARDOWN_TIMEOUT_MILLISECONDS: u64 = 1_000;
const WORKERS: usize = 2;
const MEMORY_MEBIBYTES: u64 = 512;
const SUCCESS_STATUS: i32 = 0;
const FAILURE_STATUS: i32 = 1;
const TERMINATION_SIGNAL: i32 = 9;
const EXPECTED_SUCCESSFUL_JOBS: usize = 2;
const EXPECTED_TASKS: usize = 2;

#[derive(Debug)]
struct FakeRuntime {
    executable_identity: ContentId,
    manifest_identity: ContentId,
    manifest: Vec<u8>,
    outputs: VecDeque<NixEvalProcessOutput>,
    roots: Vec<ExternalRef>,
    run_count: usize,
    prepared: Vec<PathBuf>,
    cleanup_count: usize,
    cleanup_fails: bool,
    roots_fail: bool,
    run_fails_before_start: bool,
}

impl FakeRuntime {
    fn new(outputs: Vec<NixEvalProcessOutput>) -> Self {
        Self {
            executable_identity: content_id(PINNED_EXECUTABLE_BLAKE3),
            manifest_identity: content_id(PINNED_COHORT_MANIFEST_BLAKE3),
            manifest: cohort_manifest(),
            outputs: outputs.into(),
            roots: vec![
                ExternalRef::new("nix", "derivation", "/nix/store/aaaaaaaa-job-a.drv")
                    .unwrap_or_else(|_| unreachable!()),
                ExternalRef::new("nix", "derivation", "/nix/store/bbbbbbbb-job-b.drv")
                    .unwrap_or_else(|_| unreachable!()),
            ],
            run_count: 0,
            prepared: Vec::new(),
            cleanup_count: 0,
            cleanup_fails: false,
            roots_fail: false,
            run_fails_before_start: false,
        }
    }
}

impl NixEvalRuntime for FakeRuntime {
    fn file_identity(&self, path: &Path, _max_bytes: u64) -> PortResult<ContentId> {
        if path == Path::new(PINNED_EXECUTABLE_PATH) {
            Ok(self.executable_identity.clone())
        } else if path == Path::new(PINNED_COHORT_MANIFEST_PATH) {
            Ok(self.manifest_identity.clone())
        } else {
            Err(PortFailure::Domain(kiln_core::Diagnostic::new(
                "fixture_path",
                "fixture.path",
                "fixture received an unknown file path",
            )))
        }
    }

    fn read_file(&self, path: &Path, _max_bytes: u64) -> PortResult<Vec<u8>> {
        if path == Path::new(PINNED_COHORT_MANIFEST_PATH) {
            Ok(self.manifest.clone())
        } else {
            Err(PortFailure::Domain(kiln_core::Diagnostic::new(
                "fixture_path",
                "fixture.path",
                "fixture received an unknown manifest path",
            )))
        }
    }

    fn prepare_gc_root_directory(&mut self, path: &Path) -> PortResult<()> {
        self.prepared.push(path.to_path_buf());
        Ok(())
    }

    fn list_gc_roots(&self, _path: &Path, _max_roots: usize) -> PortResult<Vec<ExternalRef>> {
        if self.roots_fail {
            Err(PortFailure::Infrastructure(
                kiln_provider_api::InfrastructureFailure {
                    code: "fixture_roots".to_string(),
                    message: "fixture GC-root observation failed".to_string(),
                },
            ))
        } else {
            Ok(self.roots.clone())
        }
    }

    fn cleanup_gc_root_directory(&mut self, _path: &Path, _max_roots: usize) -> PortResult<()> {
        self.cleanup_count = self
            .cleanup_count
            .checked_add(1)
            .unwrap_or_else(|| unreachable!());
        if self.cleanup_fails {
            Err(PortFailure::Infrastructure(
                kiln_provider_api::InfrastructureFailure {
                    code: "fixture_cleanup".to_string(),
                    message: "fixture cleanup failed".to_string(),
                },
            ))
        } else {
            Ok(())
        }
    }

    fn run(&mut self, _invocation: &ProcessInvocation) -> PortResult<NixEvalProcessOutput> {
        self.run_count = self
            .run_count
            .checked_add(1)
            .unwrap_or_else(|| unreachable!());
        if self.run_fails_before_start {
            return Err(PortFailure::Infrastructure(
                kiln_provider_api::InfrastructureFailure {
                    code: "fixture_spawn".to_string(),
                    message: "fixture process did not start".to_string(),
                },
            ));
        }
        self.outputs.pop_front().ok_or_else(|| {
            PortFailure::Infrastructure(kiln_provider_api::InfrastructureFailure {
                code: "fixture_output".to_string(),
                message: "fixture process output is exhausted".to_string(),
            })
        })
    }

    fn run_with_cancellation(
        &mut self,
        invocation: &ProcessInvocation,
        _cancellation: &AtomicBool,
    ) -> PortResult<NixEvalProcessOutput> {
        self.run(invocation)
    }
}

fn content_id(value: &str) -> ContentId {
    ContentId::from_blake3_hex(value).unwrap_or_else(|_| unreachable!())
}

fn repeated_id(character: char) -> ContentId {
    content_id(&character.to_string().repeat(BLAKE3_HEX_BYTES))
}

fn limits() -> NixEvalLimits {
    NixEvalLimits {
        max_line_bytes: MAX_LINE_BYTES,
        max_total_stdout_bytes: MAX_TOTAL_STDOUT_BYTES,
        max_stderr_bytes: MAX_STDERR_BYTES,
        max_records: MAX_RECORDS,
        max_jobs: MAX_JOBS,
        max_job_errors: MAX_JOB_ERRORS,
        max_metadata_bytes: MAX_METADATA_BYTES,
        max_diagnostic_bytes: MAX_DIAGNOSTIC_BYTES,
        max_outputs_per_job: MAX_OUTPUTS_PER_JOB,
        max_dependencies_per_job: MAX_DEPENDENCIES_PER_JOB,
        max_tasks: MAX_TASKS,
        max_gc_roots: MAX_GC_ROOTS,
        max_executable_bytes: MAX_EXECUTABLE_BYTES,
        max_manifest_bytes: MAX_MANIFEST_BYTES,
        max_in_flight: MAX_IN_FLIGHT,
        max_workers: MAX_WORKERS,
        max_memory_mebibytes: MAX_MEMORY_MEBIBYTES,
        stdin_max_bytes: STDIN_MAX_BYTES,
        timeout_milliseconds: TIMEOUT_MILLISECONDS,
        poll_interval_milliseconds: POLL_INTERVAL_MILLISECONDS,
        teardown_timeout_milliseconds: TEARDOWN_TIMEOUT_MILLISECONDS,
    }
}

fn profile() -> NixEvalProfile {
    NixEvalProfile::pinned(
        ProviderProfileId::new("nix-eval-jobs-v1").unwrap_or_else(|_| unreachable!()),
        NixEvalRuntimeBinding {
            executable_path: PathBuf::from(PINNED_EXECUTABLE_PATH),
            executable_identity: content_id(PINNED_EXECUTABLE_BLAKE3),
            cohort_manifest_path: PathBuf::from(PINNED_COHORT_MANIFEST_PATH),
            cohort_manifest_identity: content_id(PINNED_COHORT_MANIFEST_BLAKE3),
            gc_root_base: PathBuf::from("/var/lib/kiln/nix-eval-roots"),
            home_base: PathBuf::from("/var/lib/kiln/nix-eval-home"),
        },
        limits(),
    )
    .unwrap_or_else(|_| unreachable!())
}

fn plan() -> DiscoveryEffectPlan {
    DiscoveryEffectPlan {
        effect_id: EffectId::new("nix-discovery-one").unwrap_or_else(|_| unreachable!()),
        evaluation_id: EvaluationId::new("evaluation-one").unwrap_or_else(|_| unreachable!()),
        project_id: ProjectId::new("project-one").unwrap_or_else(|_| unreachable!()),
        pipeline_revision_id: PipelineRevisionId::new("pipeline-one")
            .unwrap_or_else(|_| unreachable!()),
        discovery_policy_id: PolicyId::new("nix-discovery-policy-one")
            .unwrap_or_else(|_| unreachable!()),
        provider_profile: profile().provider.profile_id,
        build_provider_profile: ProviderProfileId::new("mantle-build-v1")
            .unwrap_or_else(|_| unreachable!()),
        idempotency_key: repeated_id('1'),
        source_identity: repeated_id('2'),
        source: ExternalRef::new(
            "kiln-source",
            "nix-store-path",
            "/nix/store/00000000000000000000000000000000-source",
        )
        .unwrap_or_else(|_| unreachable!()),
        source_mode: DiscoverySourceMode::Flake,
        root_selector: Alias::new("jobs").unwrap_or_else(|_| unreachable!()),
        system: Alias::new("x86_64-linux").unwrap_or_else(|_| unreachable!()),
        options: DiscoveryOptions {
            workers: WORKERS,
            max_memory_mebibytes: MEMORY_MEBIBYTES,
            include_metadata: true,
            include_input_derivations: true,
            check_cache_status: true,
            partial_admission: false,
        },
        bounds: DiscoveryBounds {
            max_records: MAX_RECORDS,
            max_jobs: MAX_JOBS,
            max_job_errors: MAX_JOB_ERRORS,
            max_diagnostic_bytes: MAX_DIAGNOSTIC_BYTES,
            max_outputs_per_job: MAX_OUTPUTS_PER_JOB,
            max_dependencies_per_job: MAX_DEPENDENCIES_PER_JOB,
            max_tasks: MAX_TASKS,
        },
    }
}

fn cohort_manifest() -> Vec<u8> {
    serde_json::to_vec(&CohortManifest {
        schema: COHORT_MANIFEST_SCHEMA.to_string(),
        upstream_revision: PINNED_UPSTREAM_REVISION.to_string(),
        adapter_version: PINNED_ADAPTER_VERSION.to_string(),
        source_version: PINNED_SOURCE_VERSION.to_string(),
        nix_revision: PINNED_NIX_REVISION.to_string(),
        nix_version: PINNED_NIX_VERSION.to_string(),
        nixpkgs_revision: PINNED_NIXPKGS_REVISION.to_string(),
        platform: PINNED_PLATFORM.to_string(),
        executable: PINNED_EXECUTABLE_PATH.to_string(),
    })
    .unwrap_or_else(|_| unreachable!())
}

fn valid_stream() -> Vec<u8> {
    let first = serde_json::json!({
        "attr": "job-a",
        "attrPath": ["job-a"],
        "name": "job-a",
        "storeDir": "/nix/store",
        "system": "x86_64-linux",
        "drvPath": "/nix/store/aaaaaaaa-job-a.drv",
        "outputs": {"out": "/nix/store/aaaaaaaa-job-a"},
        "meta": {"description": "first fixture job"},
        "inputDrvs": {},
        "requiredSystemFeatures": [],
        "constituents": [],
        "namedConstituents": [],
        "globConstituents": false,
        "isCached": true,
        "cacheStatus": "local",
        "neededBuilds": [],
        "neededSubstitutes": []
    });
    let second = serde_json::json!({
        "attr": "job-b",
        "attrPath": ["job-b"],
        "name": "job-b",
        "storeDir": "/nix/store",
        "system": "x86_64-linux",
        "drvPath": "/nix/store/bbbbbbbb-job-b.drv",
        "outputs": {"out": "/nix/store/bbbbbbbb-job-b"},
        "meta": {"description": "second fixture job"},
        "inputDrvs": {"/nix/store/aaaaaaaa-job-a.drv": ["out"]},
        "requiredSystemFeatures": [],
        "constituents": [],
        "namedConstituents": [],
        "globConstituents": false,
        "isCached": false,
        "cacheStatus": "notBuilt",
        "neededBuilds": ["/nix/store/aaaaaaaa-job-a.drv"],
        "neededSubstitutes": []
    });
    let error = serde_json::json!({
        "attr": "broken",
        "attrPath": ["broken"],
        "error": "fixture evaluation error",
        "fatal": false
    });
    format!("{first}\n{second}\n{error}\n").into_bytes()
}

fn exited(stdout: Vec<u8>) -> NixEvalProcessOutput {
    NixEvalProcessOutput::Exited {
        process_status: Some(SUCCESS_STATUS),
        signal: None,
        stdout,
        stderr: Vec::new(),
    }
}

// r[verify kiln.nix_eval_jobs.conformance]
// r[verify kiln.nix_eval_jobs.task_mapping]
// r[verify kiln.nix_eval_jobs.core_shell_boundary]
#[test]
fn complete_stream_maps_tasks_and_preserves_job_error() {
    let runtime = FakeRuntime::new(vec![exited(valid_stream())]);
    let mut adapter = NixEvalAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
    let observation = adapter.discover(&plan()).unwrap_or_else(|_| unreachable!());
    assert_eq!(observation.terminal, DiscoveryTerminal::Complete);
    assert_eq!(observation.jobs.len(), EXPECTED_SUCCESSFUL_JOBS);
    assert_eq!(observation.job_errors.len(), 1);
    assert_eq!(observation.tasks.len(), EXPECTED_TASKS);
    assert_eq!(
        observation
            .tasks
            .iter()
            .map(|task| task.dependencies.len())
            .sum::<usize>(),
        1
    );
    assert_eq!(
        admit_discovery_observation(&plan(), &observation),
        Ok(observation.tasks.clone())
    );
    let repeated = adapter.discover(&plan()).unwrap_or_else(|_| unreachable!());
    let reconciled = adapter
        .reconcile(&plan())
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(repeated, observation);
    assert_eq!(reconciled, observation);
    assert_eq!(adapter.runtime().run_count, 1);
}

// r[verify kiln.nix_eval_jobs.conformance]
#[test]
fn shared_discovery_conformance_accepts_the_exact_adapter() {
    let runtime = FakeRuntime::new(vec![exited(valid_stream())]);
    let mut adapter = NixEvalAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
    let report = kiln_conformance::run_discovery_conformance(&mut adapter, &plan())
        .unwrap_or_else(|_| unreachable!());
    assert!(report.deterministic_repeat);
    assert!(report.deterministic_observation);
    assert!(report.deterministic_reconciliation);
}

// r[verify kiln.nix_eval_jobs.stream_admission]
// r[verify kiln.nix_eval_jobs.cache_advisory]
#[test]
fn malformed_truncated_and_conflicting_cache_streams_fail_closed() {
    let malformed = b"{not-json}\n".to_vec();
    let truncated = b"{\"attr\":\"partial\"".to_vec();
    let conflicting = valid_stream()
        .split(|byte| *byte == b'\n')
        .next()
        .map(|line| {
            let text = String::from_utf8_lossy(line);
            format!(
                "{}\n",
                text.replace("\"isCached\":true", "\"isCached\":false")
            )
            .into_bytes()
        })
        .unwrap_or_else(|| unreachable!());
    for bytes in [malformed, truncated, conflicting] {
        let runtime = FakeRuntime::new(vec![exited(bytes)]);
        let mut adapter =
            NixEvalAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
        let observation = adapter.discover(&plan()).unwrap_or_else(|_| unreachable!());
        assert_eq!(observation.terminal, DiscoveryTerminal::ProviderError);
        assert!(observation.tasks.is_empty());
    }
}

// r[verify kiln.nix_eval_jobs.failure_scope]
#[test]
fn timeout_cancellation_crash_and_unknown_remain_distinct() {
    let cases = [
        (
            NixEvalProcessOutput::TimedOut {
                stdout: Vec::new(),
                stderr: Vec::new(),
            },
            DiscoveryTerminal::TimedOut,
        ),
        (
            NixEvalProcessOutput::Cancelled {
                stdout: Vec::new(),
                stderr: Vec::new(),
            },
            DiscoveryTerminal::Cancelled,
        ),
        (
            NixEvalProcessOutput::Exited {
                process_status: Some(FAILURE_STATUS),
                signal: None,
                stdout: Vec::new(),
                stderr: Vec::new(),
            },
            DiscoveryTerminal::ProviderError,
        ),
        (
            NixEvalProcessOutput::Exited {
                process_status: None,
                signal: Some(TERMINATION_SIGNAL),
                stdout: Vec::new(),
                stderr: Vec::new(),
            },
            DiscoveryTerminal::Unknown,
        ),
        (
            NixEvalProcessOutput::OutputLimitExceeded {
                stdout: Vec::new(),
                stderr: vec![b'x'; MAX_STDERR_BYTES],
            },
            DiscoveryTerminal::ProviderError,
        ),
        (NixEvalProcessOutput::Unknown, DiscoveryTerminal::Unknown),
    ];
    for (output, expected) in cases {
        let runtime = FakeRuntime::new(vec![output]);
        let mut adapter =
            NixEvalAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
        let observation = adapter.discover(&plan()).unwrap_or_else(|_| unreachable!());
        assert_eq!(observation.terminal, expected);
        assert!(observation.tasks.is_empty());
        assert_eq!(adapter.runtime().run_count, 1);
    }
}

// r[verify kiln.nix_eval_jobs.conformance]
#[test]
fn incremental_decoder_is_invariant_across_chunk_boundaries() {
    let bytes = valid_stream();
    let mut whole = IncrementalDecoder::new(limits());
    whole.push(&bytes).unwrap_or_else(|_| unreachable!());
    let expected = whole.finish().unwrap_or_else(|_| unreachable!());
    for split in 1..bytes.len() {
        let mut decoder = IncrementalDecoder::new(limits());
        let (first, second) = bytes.split_at(split);
        decoder.push(first).unwrap_or_else(|_| unreachable!());
        decoder.push(second).unwrap_or_else(|_| unreachable!());
        assert_eq!(
            decoder.finish().unwrap_or_else(|_| unreachable!()),
            expected
        );
    }
}

// r[verify kiln.nix_eval_jobs.stream_admission]
#[test]
fn duplicate_jobs_oversized_lines_and_gc_root_loss_fail_closed() {
    let first_line = valid_stream()
        .split(|byte| *byte == b'\n')
        .next()
        .map(Vec::from)
        .unwrap_or_else(|| unreachable!());
    let mut duplicate = first_line.clone();
    duplicate.push(b'\n');
    duplicate.extend_from_slice(&first_line);
    duplicate.push(b'\n');
    let mut runtime = FakeRuntime::new(vec![exited(duplicate)]);
    runtime.roots = vec![
        ExternalRef::new("nix", "derivation", "/nix/store/aaaaaaaa-job-a.drv")
            .unwrap_or_else(|_| unreachable!()),
    ];
    let mut adapter = NixEvalAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
    assert!(adapter.discover(&plan()).is_err());

    let oversized_line_bytes = MAX_LINE_BYTES
        .checked_add(1)
        .unwrap_or_else(|| unreachable!());
    let mut oversized = vec![b'x'; oversized_line_bytes];
    oversized.push(b'\n');
    let runtime = FakeRuntime::new(vec![exited(oversized)]);
    let mut adapter = NixEvalAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
    let observation = adapter.discover(&plan()).unwrap_or_else(|_| unreachable!());
    assert_eq!(observation.terminal, DiscoveryTerminal::ProviderError);
    assert!(observation.tasks.is_empty());

    let mut runtime = FakeRuntime::new(vec![exited(valid_stream())]);
    runtime.roots_fail = true;
    let mut adapter = NixEvalAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
    let observation = adapter.discover(&plan()).unwrap_or_else(|_| unreachable!());
    assert_eq!(observation.terminal, DiscoveryTerminal::ProviderError);
    assert_eq!(observation.gc_roots.state, GcRootState::Unknown);
    assert!(observation.tasks.is_empty());
}

// r[verify kiln.nix_eval_jobs.failure_scope]
#[test]
fn explicit_cancellation_uses_the_cancellation_path() {
    let runtime = FakeRuntime::new(vec![NixEvalProcessOutput::Cancelled {
        stdout: Vec::new(),
        stderr: Vec::new(),
    }]);
    let mut adapter = NixEvalAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
    let cancellation = AtomicBool::new(true);
    let observation = adapter
        .discover_with_cancellation(&plan(), &cancellation)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(observation.terminal, DiscoveryTerminal::Cancelled);
    assert_eq!(adapter.runtime().run_count, 1);
}

// r[verify kiln.nix_eval_jobs.gc_root_handoff]
#[test]
fn gc_roots_require_complete_handoff_and_preserve_cleanup_failure() {
    let runtime = FakeRuntime::new(vec![exited(valid_stream())]);
    let mut adapter = NixEvalAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
    let observation = adapter.discover(&plan()).unwrap_or_else(|_| unreachable!());
    assert!(adapter.cleanup_gc_roots(&plan()).is_err());
    let first_task = observation
        .tasks
        .first()
        .map(|task| task.task_id.clone())
        .unwrap_or_else(|| unreachable!());
    assert!(
        adapter
            .handoff_gc_roots(&plan(), &BTreeSet::from([first_task]))
            .unwrap_or_else(|_| unreachable!())
            .is_empty()
    );
    let all_tasks: BTreeSet<_> = observation
        .tasks
        .iter()
        .map(|task| task.task_id.clone())
        .collect();
    let handed_off = adapter
        .handoff_gc_roots(&plan(), &all_tasks)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(handed_off, observation.tasks);
    adapter.runtime_mut().cleanup_fails = true;
    let cleaned = adapter
        .cleanup_gc_roots(&plan())
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(cleaned.gc_roots.state, GcRootState::CleanupFailed);
    assert_eq!(adapter.runtime().cleanup_count, 1);
}

// r[verify kiln.nix_eval_jobs.cohort_admission]
// r[verify kiln.nix_eval_jobs.input_admission]
// r[verify kiln.nix_eval_jobs.onix_boundary]
// r[verify kiln.nix_eval_jobs.nonclaims]
#[test]
fn executable_manifest_source_and_cohort_drift_fail_before_launch() {
    let mut runtime = FakeRuntime::new(Vec::new());
    runtime.executable_identity = repeated_id('f');
    assert!(NixEvalAdapter::new(profile(), runtime).is_err());

    let mut unsupported = profile();
    unsupported.nix_version = "onix-nix".to_string();
    assert!(admit_profile(&unsupported).is_err());

    let mut promoted_claim = profile();
    promoted_claim.required_non_claims = vec!["evaluation is correct".to_string()];
    assert!(admit_profile(&promoted_claim).is_err());

    let mut floating = plan();
    floating.source = ExternalRef::new("kiln-source", "nix-store-path", "github:NixOS/nixpkgs")
        .unwrap_or_else(|_| unreachable!());
    assert!(admit_plan(&profile(), &floating).is_err());
}

// r[verify kiln.nix_eval_jobs.gc_root_handoff]
#[test]
fn spawn_failure_cleans_the_private_gc_root_directory() {
    let mut runtime = FakeRuntime::new(Vec::new());
    runtime.run_fails_before_start = true;
    let mut adapter = NixEvalAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
    assert!(adapter.discover(&plan()).is_err());
    assert_eq!(adapter.runtime().cleanup_count, 1);
}

// r[verify kiln.nix_eval_jobs.terminal_completeness]
#[test]
fn restored_unknown_reconciles_without_process_redispatch() {
    let runtime = FakeRuntime::new(vec![NixEvalProcessOutput::Unknown]);
    let mut first = NixEvalAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
    let unknown = first.discover(&plan()).unwrap_or_else(|_| unreachable!());
    assert_eq!(unknown.terminal, DiscoveryTerminal::Unknown);

    let runtime = FakeRuntime::new(Vec::new());
    let mut restored = NixEvalAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
    restored
        .restore(&plan(), &unknown)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        restored
            .reconcile(&plan())
            .unwrap_or_else(|_| unreachable!()),
        unknown
    );
    assert_eq!(restored.runtime().run_count, 0);
}

// r[verify kiln.nix_eval_jobs.stream_admission]
// r[verify kiln.nix_eval_jobs.gc_root_handoff]
#[cfg(unix)]
#[test]
fn bounded_runtime_hashes_files_and_rejects_non_root_entries() {
    use std::os::unix::fs::symlink;

    let directory = tempdir().unwrap_or_else(|_| unreachable!());
    let manifest_path = directory.path().join("manifest.json");
    fs::write(&manifest_path, b"fixture").unwrap_or_else(|_| unreachable!());
    let runtime = BoundedNixEvalRuntime;
    let observed = runtime
        .file_identity(&manifest_path, MAX_MANIFEST_BYTES)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        observed,
        content_id(blake3::hash(b"fixture").to_hex().as_str())
    );

    let root_directory = directory.path().join("roots");
    let mut runtime = BoundedNixEvalRuntime;
    runtime
        .prepare_gc_root_directory(&root_directory)
        .unwrap_or_else(|_| unreachable!());
    symlink(
        "/nix/store/aaaaaaaa-fixture.drv",
        root_directory.join("aaaaaaaa-fixture.drv"),
    )
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        runtime
            .list_gc_roots(&root_directory, MAX_GC_ROOTS)
            .unwrap_or_else(|_| unreachable!())
            .len(),
        1
    );
    fs::write(root_directory.join("not-a-root"), b"fixture").unwrap_or_else(|_| unreachable!());
    assert!(
        runtime
            .list_gc_roots(&root_directory, MAX_GC_ROOTS)
            .is_err()
    );
}

// r[verify kiln.nix_eval_jobs.dependency_boundary]
#[test]
fn core_and_standalone_manifests_do_not_import_nix_or_adapter_code() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap_or_else(|| unreachable!());
    for relative in [
        "crates/kiln-core/Cargo.toml",
        "crates/kiln-provider-api/Cargo.toml",
        "crates/kiln-standalone/Cargo.toml",
    ] {
        let text = fs::read_to_string(root.join(relative)).unwrap_or_else(|_| unreachable!());
        assert!(!text.contains("nix-eval-jobs"));
        assert!(!text.contains("kiln-adapter-nix-eval"));
        assert!(!text.contains("nix-store"));
        assert!(!text.contains("nix-expr"));
        assert!(!text.contains("hydra"));
    }
}
