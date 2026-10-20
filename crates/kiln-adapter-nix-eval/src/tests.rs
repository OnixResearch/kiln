use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use kiln_core::discovery::{
    DiscoveryBounds, DiscoveryEffectPlan, DiscoveryEvaluatorCohort, DiscoveryOptions,
    DiscoverySourceMode, DiscoveryTerminal, GcRootState, admit_discovery_observation,
};
use kiln_core::types::{
    Alias, BLAKE3_HEX_BYTES, ContentId, DEFAULT_MAX_ID_BYTES, EffectId, EvaluationId, ExternalRef,
    PipelineRevisionId, PolicyId, ProjectId, ProviderProfileId,
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
    file_overrides: BTreeMap<PathBuf, ContentId>,
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
            file_overrides: BTreeMap::new(),
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

    fn onix(outputs: Vec<NixEvalProcessOutput>) -> Self {
        let mut runtime = Self::new(outputs);
        runtime.executable_identity = content_id(ONIX_EXECUTABLE_BLAKE3);
        runtime.manifest_identity = content_id(ONIX_MANIFEST_BLAKE3);
        let profile = onix_profile();
        runtime.manifest = serde_json::to_vec(&CohortManifest {
            schema: profile.cohort_manifest_schema,
            upstream_revision: profile.upstream_revision,
            adapter_version: profile.adapter_version,
            source_version: profile.source_version,
            nix_revision: profile.nix_revision,
            nix_version: profile.nix_version,
            nixpkgs_revision: profile.nixpkgs_revision,
            platform: profile.platform,
            executable: ONIX_EXECUTABLE_PATH.to_string(),
            onix: Some(onix_provenance().unwrap_or_else(|_| unreachable!())),
        })
        .unwrap_or_else(|_| unreachable!());
        runtime.file_overrides.insert(
            PathBuf::from(ONIX_PLUGIN_PATH),
            content_id(ONIX_PLUGIN_BLAKE3),
        );
        runtime.file_overrides.insert(
            Path::new(ONIX_SOURCE_PATH).join("flake.nix"),
            content_id(ONIX_SOURCE_BLAKE3),
        );
        for (path, identity) in ONIX_LINKED_COMPONENTS {
            runtime
                .file_overrides
                .insert(PathBuf::from(path), content_id(identity));
        }
        runtime
    }
}

impl NixEvalRuntime for FakeRuntime {
    fn file_identity(&self, path: &Path, _max_bytes: u64) -> PortResult<ContentId> {
        if let Some(identity) = self.file_overrides.get(path) {
            Ok(identity.clone())
        } else if path == Path::new(PINNED_EXECUTABLE_PATH)
            || path == Path::new(ONIX_EXECUTABLE_PATH)
        {
            Ok(self.executable_identity.clone())
        } else if path == Path::new(PINNED_COHORT_MANIFEST_PATH)
            || path == Path::new(ONIX_MANIFEST_PATH)
        {
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
        if path == Path::new(PINNED_COHORT_MANIFEST_PATH) || path == Path::new(ONIX_MANIFEST_PATH) {
            Ok(self.manifest.clone())
        } else {
            Err(PortFailure::Domain(kiln_core::Diagnostic::new(
                "fixture_path",
                "fixture.path",
                "fixture received an unknown manifest path",
            )))
        }
    }

    fn file_identity_no_follow(&self, path: &Path, max_bytes: u64) -> PortResult<ContentId> {
        self.file_identity(path, max_bytes)
    }

    fn read_file_no_follow(&self, path: &Path, max_bytes: u64) -> PortResult<Vec<u8>> {
        self.read_file(path, max_bytes)
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
        evaluator_cohort: None,
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
        onix: None,
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

fn onix_stream(allocated_bytes: u64, wall_milliseconds: u64) -> Vec<u8> {
    let mut stream = Vec::new();
    for line in valid_stream()
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let mut record: serde_json::Value =
            serde_json::from_slice(line).unwrap_or_else(|_| unreachable!());
        record
            .as_object_mut()
            .unwrap_or_else(|| unreachable!())
            .insert(
                "stats".to_string(),
                serde_json::json!({"allocBytes": allocated_bytes, "wallMs": wall_milliseconds}),
            );
        serde_json::to_writer(&mut stream, &record).unwrap_or_else(|_| unreachable!());
        stream.push(b'\n');
    }
    stream
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
        for (selected, request, runtime) in [
            (
                profile(),
                plan(),
                FakeRuntime::new(vec![exited(bytes.clone())]),
            ),
            (
                onix_profile(),
                onix_plan(),
                FakeRuntime::onix(vec![exited(bytes.clone())]),
            ),
        ] {
            let mut adapter =
                NixEvalAdapter::new(selected, runtime).unwrap_or_else(|_| unreachable!());
            let observation = adapter
                .discover(&request)
                .unwrap_or_else(|_| unreachable!());
            assert_eq!(observation.terminal, DiscoveryTerminal::ProviderError);
            assert!(observation.tasks.is_empty());
        }
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
        for (selected, request, runtime) in [
            (profile(), plan(), FakeRuntime::new(vec![output.clone()])),
            (
                onix_profile(),
                onix_plan(),
                FakeRuntime::onix(vec![output.clone()]),
            ),
        ] {
            let mut adapter =
                NixEvalAdapter::new(selected, runtime).unwrap_or_else(|_| unreachable!());
            let observation = adapter
                .discover(&request)
                .unwrap_or_else(|_| unreachable!());
            assert_eq!(observation.terminal, expected);
            assert!(observation.tasks.is_empty());
            assert_eq!(adapter.runtime().run_count, 1);
        }
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

// r[verify kiln.nix_eval_jobs.onix_boundary]
#[test]
fn stock_profile_rejects_onix_cohort_before_process_launch() {
    let mut attempted = plan();
    attempted.evaluator_cohort = Some(DiscoveryEvaluatorCohort {
        kind: Alias::new("onix-nix").unwrap_or_else(|_| unreachable!()),
        executable_identity: repeated_id('3'),
        manifest_identity: repeated_id('4'),
        host_abi: Alias::new("nix_wasm_init_v1").unwrap_or_else(|_| unreachable!()),
        features: vec![
            Alias::new("flakes").unwrap_or_else(|_| unreachable!()),
            Alias::new("nix-command").unwrap_or_else(|_| unreachable!()),
            Alias::new("wasm-builtin").unwrap_or_else(|_| unreachable!()),
        ],
        plugin_identity: repeated_id('5'),
        components: Vec::new(),
        plugin: ExternalRef::new("nix", "store-path", "/nix/store/example/plugin.wasm")
            .unwrap_or_else(|_| unreachable!()),
    });
    let runtime = FakeRuntime::new(Vec::new());
    let mut adapter = NixEvalAdapter::new(profile(), runtime).unwrap_or_else(|_| unreachable!());
    let denied = adapter.discover(&attempted);
    assert!(matches!(
        &denied,
        Err(PortFailure::Domain(reason)) if reason.code == "nix_eval_unsupported_evaluator"
    ));
    assert_eq!(adapter.runtime().run_count, 0);
    assert!(adapter.runtime().prepared.is_empty());
}

fn fixture_source_binding() -> OnixSourceBinding {
    OnixSourceBinding {
        source: ExternalRef::new("kiln-source", "nix-store-path", ONIX_SOURCE_PATH)
            .unwrap_or_else(|_| unreachable!()),
        source_identity: content_id(ONIX_SOURCE_BLAKE3),
        root_selector: Alias::new(ONIX_ROOT_SELECTOR).unwrap_or_else(|_| unreachable!()),
        system: Alias::new(PINNED_PLATFORM).unwrap_or_else(|_| unreachable!()),
    }
}

fn onix_profile() -> NixEvalProfile {
    onix_profile_with_source(fixture_source_binding())
}

fn onix_profile_with_source(binding: OnixSourceBinding) -> NixEvalProfile {
    let mut runtime = profile().runtime;
    runtime.executable_path = PathBuf::from(ONIX_EXECUTABLE_PATH);
    runtime.executable_identity = content_id(ONIX_EXECUTABLE_BLAKE3);
    runtime.cohort_manifest_path = PathBuf::from(ONIX_MANIFEST_PATH);
    runtime.cohort_manifest_identity = content_id(ONIX_MANIFEST_BLAKE3);
    NixEvalProfile::onix(runtime, limits(), binding).unwrap_or_else(|_| unreachable!())
}

fn onix_plan() -> DiscoveryEffectPlan {
    let mut request = plan();
    request.provider_profile = onix_profile().provider.profile_id;
    request.evaluator_cohort = Some(onix_cohort().unwrap_or_else(|_| unreachable!()));
    request.source = ExternalRef::new("kiln-source", "nix-store-path", ONIX_SOURCE_PATH)
        .unwrap_or_else(|_| unreachable!());
    request.source_identity = content_id(ONIX_SOURCE_BLAKE3);
    request.root_selector = Alias::new(ONIX_ROOT_SELECTOR).unwrap_or_else(|_| unreachable!());
    request
}

fn nonfixture_source_binding() -> OnixSourceBinding {
    let request = plan();
    OnixSourceBinding {
        source: request.source,
        source_identity: request.source_identity,
        root_selector: request.root_selector,
        system: request.system,
    }
}

fn bound_onix_plan(binding: &OnixSourceBinding) -> DiscoveryEffectPlan {
    let mut request = onix_plan();
    request.source = binding.source.clone();
    request.source_identity = binding.source_identity.clone();
    request.root_selector = binding.root_selector.clone();
    request.system = binding.system.clone();
    request
}

fn nonfixture_runtime(outputs: Vec<NixEvalProcessOutput>) -> FakeRuntime {
    let binding = nonfixture_source_binding();
    let mut runtime = FakeRuntime::onix(outputs);
    runtime
        .file_overrides
        .remove(&Path::new(ONIX_SOURCE_PATH).join("flake.nix"));
    runtime.file_overrides.insert(
        Path::new(binding.source.native_id.as_str()).join("flake.nix"),
        binding.source_identity,
    );
    runtime
}

// r[verify kiln.onixos_pilot.source]
#[test]
fn explicit_nonfixture_source_admits_discovery_without_fixture_source() {
    let binding = nonfixture_source_binding();
    let selected = onix_profile_with_source(binding.clone());
    let request = bound_onix_plan(&binding);
    assert_eq!(admit_profile(&selected), Ok(()));
    assert_eq!(admit_plan(&selected, &request), Ok(()));
    let mut adapter = NixEvalAdapter::new(
        selected,
        nonfixture_runtime(vec![exited(onix_stream(0, 0))]),
    )
    .unwrap_or_else(|_| unreachable!());
    let observation = adapter
        .discover(&request)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(observation.terminal, DiscoveryTerminal::Complete);
    assert_eq!(observation.source_identity, binding.source_identity);
    assert_eq!(
        admit_discovery_observation(&request, &observation),
        Ok(observation.tasks.clone())
    );
    assert_eq!(adapter.runtime().run_count, 1);
    assert_eq!(
        observation
            .tasks
            .iter()
            .map(|task| task.dependencies.len())
            .sum::<usize>(),
        1
    );
    assert!(admit_plan(&onix_profile(), &request).is_err());
    assert!(admit_plan(&onix_profile_with_source(binding), &onix_plan()).is_err());
}

// r[verify kiln.onixos_pilot.source]
#[test]
fn explicit_nonfixture_request_drift_rejects_before_roots_or_process() {
    type Mutation = fn(&mut DiscoveryEffectPlan);
    let cases: &[Mutation] = &[
        |request| request.source = fixture_source_binding().source,
        |request| request.source_identity = repeated_id('a'),
        |request| {
            request.root_selector = Alias::new("other.jobs").unwrap_or_else(|_| unreachable!())
        },
        |request| request.system = Alias::new("aarch64-linux").unwrap_or_else(|_| unreachable!()),
        |request| request.source_mode = DiscoverySourceMode::Expression,
        |request| {
            request.source = ExternalRef::new("kiln-source", "nix-store-path", "/tmp/source")
                .unwrap_or_else(|_| unreachable!())
        },
    ];
    let binding = nonfixture_source_binding();
    let selected = onix_profile_with_source(binding.clone());
    let mut adapter = NixEvalAdapter::new(selected.clone(), nonfixture_runtime(Vec::new()))
        .unwrap_or_else(|_| unreachable!());
    for mutate in cases {
        let mut request = bound_onix_plan(&binding);
        mutate(&mut request);
        assert!(matches!(
            admit_plan(&selected, &request),
            Err(PortFailure::Domain(reason)) if reason.code == "onix_source_drift"
        ));
        assert!(matches!(
            adapter.discover(&request),
            Err(PortFailure::Domain(reason)) if reason.code == "onix_source_drift"
        ));
        assert_eq!(adapter.runtime().run_count, 0);
        assert!(adapter.runtime().prepared.is_empty());
    }
}

// r[verify kiln.onixos_pilot.source]
#[test]
fn explicit_nonfixture_flake_drift_rejects_construction_and_dispatch() {
    let binding = nonfixture_source_binding();
    let selected = onix_profile_with_source(binding.clone());
    let source_file = Path::new(binding.source.native_id.as_str()).join("flake.nix");
    let mut runtime = nonfixture_runtime(Vec::new());
    runtime
        .file_overrides
        .insert(source_file.clone(), repeated_id('a'));
    assert!(matches!(
        NixEvalAdapter::new(selected.clone(), runtime),
        Err(PortFailure::Domain(reason)) if reason.code == "onix_source_drift"
    ));
    let mut adapter = NixEvalAdapter::new(selected, nonfixture_runtime(Vec::new()))
        .unwrap_or_else(|_| unreachable!());
    adapter
        .runtime_mut()
        .file_overrides
        .insert(source_file, repeated_id('a'));
    assert!(matches!(
        adapter.discover(&bound_onix_plan(&binding)),
        Err(PortFailure::Domain(reason)) if reason.code == "onix_source_drift"
    ));
    assert_eq!(adapter.runtime().run_count, 0);
    assert!(adapter.runtime().prepared.is_empty());
}

// r[verify kiln.onixos_pilot.source]
#[test]
fn onix_binding_rejects_mutable_indirect_and_malformed_roots() {
    for path in [
        "/tmp/source",
        "path:/nix/store/00000000000000000000000000000000-source",
        "/nix/store/00000000000000000000000000000000-source/",
        "/nix/store/00000000000000000000000000000000-source/subdir",
        "/nix/store/00000000000000000000000000000000-source/../other",
        "/nix/store/../tmp/source",
        "/nix/store/short-source",
        "/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-source",
        "/nix/store/00000000000000000000000000000000-source#jobs",
        "/nix/store/00000000000000000000000000000000-source?dir=other",
    ] {
        let mut binding = nonfixture_source_binding();
        binding.source = ExternalRef::new("kiln-source", "nix-store-path", path)
            .unwrap_or_else(|_| unreachable!());
        let mut selected = onix_profile();
        selected.onix_source_binding = Some(binding.clone());
        assert!(admit_profile(&selected).is_err(), "{path}");
        assert!(
            admit_plan(&selected, &bound_onix_plan(&binding)).is_err(),
            "{path}"
        );
        assert!(
            NixEvalProfile::onix(selected.runtime, limits(), binding).is_err(),
            "{path}"
        );
    }
}

// r[verify kiln.onixos_pilot.source]
#[test]
fn onix_binding_requires_selector_platform_and_explicit_cohort_selection() {
    type Mutation = fn(&mut NixEvalProfile);
    let cases: &[Mutation] = &[
        |selected| selected.onix_source_binding = None,
        |selected| {
            selected
                .onix_source_binding
                .as_mut()
                .unwrap_or_else(|| unreachable!())
                .root_selector = Alias::new("jobs#other").unwrap_or_else(|_| unreachable!())
        },
        |selected| {
            selected
                .onix_source_binding
                .as_mut()
                .unwrap_or_else(|| unreachable!())
                .system = Alias::new("aarch64-linux").unwrap_or_else(|_| unreachable!())
        },
    ];
    for mutate in cases {
        let mut selected = onix_profile();
        mutate(&mut selected);
        assert!(matches!(
            admit_profile(&selected),
            Err(PortFailure::Domain(reason)) if reason.code == "onix_source_binding"
        ));
        assert!(admit_plan(&selected, &onix_plan()).is_err());
        assert!(NixEvalAdapter::new(selected, FakeRuntime::onix(Vec::new())).is_err());
    }
    let mut stock = profile();
    stock.onix_source_binding = Some(nonfixture_source_binding());
    assert!(admit_profile(&stock).is_err());
    assert!(admit_plan(&stock, &plan()).is_err());
    assert!(
        NixEvalProfile::pinned(
            ProviderProfileId::new(ONIX_PROFILE).unwrap_or_else(|_| unreachable!()),
            onix_profile().runtime,
            limits(),
        )
        .is_err()
    );
}

// r[verify kiln.onixos_pilot.source]
#[test]
fn onix_binding_rejects_malformed_deserialized_flake_hashes() {
    for identity in ["short".to_string(), "g".repeat(BLAKE3_HEX_BYTES)] {
        let mut binding = nonfixture_source_binding();
        binding.source_identity =
            serde_json::from_value(serde_json::json!(identity)).unwrap_or_else(|_| unreachable!());
        let mut selected = onix_profile();
        selected.onix_source_binding = Some(binding.clone());
        assert!(matches!(
            admit_profile(&selected),
            Err(PortFailure::Domain(reason)) if reason.code == "onix_source_binding"
        ));
        assert!(admit_plan(&selected, &bound_onix_plan(&binding)).is_err());
        assert!(NixEvalProfile::onix(selected.runtime, limits(), binding).is_err());
    }
}

// r[verify kiln.onixos_pilot.source]
#[test]
fn stock_profile_serialization_omits_onix_binding_and_preserves_selection() {
    let stock = profile();
    let bytes = serde_json::to_vec(&stock).unwrap_or_else(|_| unreachable!());
    let encoded: serde_json::Value =
        serde_json::from_slice(&bytes).unwrap_or_else(|_| unreachable!());
    assert!(encoded.get("onix_source_binding").is_none());
    assert!(encoded.get("evaluator_cohort").is_none());
    let restored: NixEvalProfile =
        serde_json::from_slice(&bytes).unwrap_or_else(|_| unreachable!());
    assert_eq!(admit_plan(&restored, &plan()), Ok(()));
    assert!(matches!(
        admit_plan(&restored, &onix_plan()),
        Err(PortFailure::Domain(reason)) if reason.code == "nix_eval_unsupported_evaluator"
    ));
    assert_eq!(
        serde_json::to_vec(&restored).unwrap_or_else(|_| unreachable!()),
        bytes
    );
    let mut adapter = NixEvalAdapter::new(restored, FakeRuntime::new(vec![exited(valid_stream())]))
        .unwrap_or_else(|_| unreachable!());
    let observation = adapter.discover(&plan()).unwrap_or_else(|_| unreachable!());
    assert_eq!(observation.terminal, DiscoveryTerminal::Complete);
    assert_eq!(
        admit_discovery_observation(&plan(), &observation),
        Ok(observation.tasks)
    );
}

#[test]
fn exact_onix_discovery_maps_tasks_and_retains_roots_until_handoff() {
    let request = onix_plan();
    let mut first = NixEvalAdapter::new(
        onix_profile(),
        FakeRuntime::onix(vec![exited(onix_stream(0, 0))]),
    )
    .unwrap_or_else(|_| unreachable!());
    let mut second = NixEvalAdapter::new(
        onix_profile(),
        FakeRuntime::onix(vec![exited(onix_stream(u64::MAX, u64::MAX))]),
    )
    .unwrap_or_else(|_| unreachable!());
    let observation = first.discover(&request).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        observation,
        second
            .discover(&request.clone())
            .unwrap_or_else(|_| unreachable!())
    );
    assert_eq!(observation.terminal, DiscoveryTerminal::Complete);
    assert_eq!(observation.gc_roots.state, GcRootState::Retained);
    assert_eq!(
        observation
            .job_errors
            .first()
            .map(|error| error.attribute.as_str()),
        Some("broken")
    );
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
        admit_discovery_observation(&request, &observation),
        Ok(observation.tasks.clone())
    );
    assert!(first.cleanup_gc_roots(&request).is_err());
    let accepted = observation
        .tasks
        .iter()
        .map(|task| task.task_id.clone())
        .collect();
    assert_eq!(
        first
            .handoff_gc_roots(&request, &accepted)
            .unwrap_or_else(|_| unreachable!()),
        observation.tasks
    );
    assert_eq!(
        first
            .cleanup_gc_roots(&request)
            .unwrap_or_else(|_| unreachable!())
            .gc_roots
            .state,
        GcRootState::Released
    );
    assert_eq!(first.runtime().run_count, 1);
}

// r[verify kiln.nix_eval_jobs_onix.fixtures]
#[test]
fn producer_statistics_are_typed_and_do_not_widen_the_stock_schema() {
    for statistics in [
        serde_json::json!(null),
        serde_json::json!({"allocBytes": 0}),
        serde_json::json!({"allocBytes": -1, "wallMs": 0}),
        serde_json::json!({"allocBytes": 0, "wallMs": 0, "unknown": 0}),
    ] {
        let bytes = format!(
            "{}\n",
            serde_json::json!({
                "attr": "broken", "attrPath": ["broken"], "error": "fixture error",
                "fatal": false, "stats": statistics
            })
        );
        let mut decoder = IncrementalDecoder::onix(limits());
        assert!(matches!(
            decoder.push(bytes.as_bytes()),
            Err(PortFailure::Domain(reason)) if reason.code == "nix_eval_statistics_schema"
        ));
    }
    let bytes = onix_stream(0, 0);
    let mut stock = IncrementalDecoder::new(limits());
    assert!(matches!(
        stock.push(&bytes),
        Err(PortFailure::Domain(reason)) if reason.code == "nix_eval_job_schema"
    ));
}

// r[verify kiln.nix_eval_jobs_onix.fallback]
#[test]
fn error_classification_uses_the_selected_cohort() {
    for (mut decoder, message, expected) in [
        (
            IncrementalDecoder::new(limits()),
            "error: attribute 'wasm' missing",
            "unsupported_onix_builtin",
        ),
        (
            IncrementalDecoder::onix(limits()),
            "kiln-wasm-discovery: intentional independent job error",
            "nix_job_evaluation_error",
        ),
        (
            IncrementalDecoder::onix(limits()),
            "error: attribute 'wasm' missing in a user record",
            "nix_job_evaluation_error",
        ),
    ] {
        let row = format!(
            "{}\n",
            serde_json::json!({"attr": "broken", "attrPath": ["broken"], "error": message})
        );
        decoder
            .push(row.as_bytes())
            .unwrap_or_else(|_| unreachable!());
        let admitted = decoder.finish().unwrap_or_else(|_| unreachable!());
        assert_eq!(
            admitted
                .errors
                .first()
                .map(|error| error.diagnostic.code.as_str()),
            Some(expected)
        );
    }
}

#[test]
fn onix_request_drift_rejects_before_roots_or_process() {
    type Mutation = fn(&mut DiscoveryEffectPlan);
    let cases: &[(&str, Mutation)] = &[
        ("onix_cohort_missing", |request| {
            request.evaluator_cohort = None
        }),
        ("onix_executable_drift", |request| {
            request
                .evaluator_cohort
                .as_mut()
                .unwrap_or_else(|| unreachable!())
                .executable_identity = repeated_id('a')
        }),
        ("onix_manifest_drift", |request| {
            request
                .evaluator_cohort
                .as_mut()
                .unwrap_or_else(|| unreachable!())
                .manifest_identity = repeated_id('a')
        }),
        ("onix_host_abi_drift", |request| {
            request
                .evaluator_cohort
                .as_mut()
                .unwrap_or_else(|| unreachable!())
                .host_abi = Alias::new("wrong-abi").unwrap_or_else(|_| unreachable!())
        }),
        ("onix_feature_drift", |request| {
            request
                .evaluator_cohort
                .as_mut()
                .unwrap_or_else(|| unreachable!())
                .features
                .pop();
        }),
        ("onix_plugin_drift", |request| {
            request
                .evaluator_cohort
                .as_mut()
                .unwrap_or_else(|| unreachable!())
                .plugin_identity = repeated_id('a')
        }),
        ("onix_component_drift", |request| {
            request
                .evaluator_cohort
                .as_mut()
                .unwrap_or_else(|| unreachable!())
                .components
                .first_mut()
                .unwrap_or_else(|| unreachable!())
                .identity = repeated_id('a')
        }),
        ("onix_source_drift", |request| {
            request.source_identity = repeated_id('a')
        }),
        ("onix_source_drift", |request| {
            request.source =
                ExternalRef::new("kiln-source", "nix-store-path", "/nix/store/../tmp/dirty")
                    .unwrap_or_else(|_| unreachable!())
        }),
        ("onix_source_drift", |request| {
            request.root_selector = Alias::new("jobs").unwrap_or_else(|_| unreachable!())
        }),
        ("nix_eval_plan_profile", |request| {
            request.provider_profile = profile().provider.profile_id
        }),
    ];
    let mut adapter = NixEvalAdapter::new(onix_profile(), FakeRuntime::onix(Vec::new()))
        .unwrap_or_else(|_| unreachable!());
    for (code, mutate) in cases {
        let mut request = onix_plan();
        mutate(&mut request);
        assert!(
            matches!(adapter.discover(&request), Err(PortFailure::Domain(reason)) if reason.code == *code),
            "{code}"
        );
        assert_eq!(adapter.runtime().run_count, 0);
        assert!(adapter.runtime().prepared.is_empty());
    }
}

#[test]
fn onix_runtime_drift_rejects_before_roots_or_process() {
    let cases = [
        (PathBuf::from(ONIX_EXECUTABLE_PATH), "onix_executable_drift"),
        (PathBuf::from(ONIX_MANIFEST_PATH), "onix_manifest_drift"),
        (PathBuf::from(ONIX_PLUGIN_PATH), "onix_plugin_drift"),
        (
            PathBuf::from(
                ONIX_LINKED_COMPONENTS
                    .first()
                    .unwrap_or_else(|| unreachable!())
                    .0,
            ),
            "onix_component_drift",
        ),
        (
            Path::new(ONIX_SOURCE_PATH).join("flake.nix"),
            "onix_source_drift",
        ),
    ];
    for (path, code) in cases {
        let mut adapter = NixEvalAdapter::new(onix_profile(), FakeRuntime::onix(Vec::new()))
            .unwrap_or_else(|_| unreachable!());
        adapter
            .runtime_mut()
            .file_overrides
            .insert(path, repeated_id('a'));
        assert!(
            matches!(adapter.discover(&onix_plan()), Err(PortFailure::Domain(reason)) if reason.code == code)
        );
        assert_eq!(adapter.runtime().run_count, 0);
        assert!(adapter.runtime().prepared.is_empty());
    }
    let mut adapter = NixEvalAdapter::new(onix_profile(), FakeRuntime::onix(Vec::new()))
        .unwrap_or_else(|_| unreachable!());
    adapter.runtime_mut().manifest = b"{}".to_vec();
    assert!(
        matches!(adapter.discover(&onix_plan()), Err(PortFailure::Domain(reason)) if reason.code == "onix_manifest_drift")
    );
    assert!(adapter.runtime().prepared.is_empty());
    assert_eq!(adapter.runtime().run_count, 0);
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
    let link = directory.path().join("manifest-link");
    symlink(&manifest_path, &link).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        runtime.file_identity_no_follow(&manifest_path, MAX_MANIFEST_BYTES),
        Ok(observed)
    );
    assert_eq!(
        runtime.read_file_no_follow(&manifest_path, MAX_MANIFEST_BYTES),
        Ok(b"fixture".to_vec())
    );
    assert!(
        runtime
            .file_identity_no_follow(&link, MAX_MANIFEST_BYTES)
            .is_err()
    );
    assert!(
        runtime
            .read_file_no_follow(&link, MAX_MANIFEST_BYTES)
            .is_err()
    );
    assert!(runtime.file_identity_no_follow(&manifest_path, 1).is_err());
    assert!(runtime.read_file_no_follow(&manifest_path, 1).is_err());

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

// r[verify kiln.nix_eval_jobs_onix.nonclaims]
#[test]
fn onix_policy_and_nonclaims_cannot_be_promoted() {
    let mut altered = onix_profile();
    altered.policy.allow_impure = true;
    assert!(
        matches!(admit_profile(&altered), Err(PortFailure::Domain(reason)) if reason.code == "nix_eval_profile_binding")
    );
    let mut altered = onix_profile();
    altered.required_non_claims.pop();
    assert!(
        matches!(admit_profile(&altered), Err(PortFailure::Domain(reason)) if reason.code == "nix_eval_profile_binding")
    );
    let mut altered = onix_profile();
    altered.upstream_revision = "mutable-main".to_string();
    assert!(
        matches!(admit_profile(&altered), Err(PortFailure::Domain(reason)) if reason.code == "nix_eval_cohort")
    );
}

#[test]
fn onix_manifest_provenance_and_fatal_jobs_fail_closed() {
    let mut adapter = NixEvalAdapter::new(onix_profile(), FakeRuntime::onix(Vec::new()))
        .unwrap_or_else(|_| unreachable!());
    let mut manifest: CohortManifest =
        serde_json::from_slice(&adapter.runtime().manifest).unwrap_or_else(|_| unreachable!());
    manifest
        .onix
        .as_mut()
        .unwrap_or_else(|| unreachable!())
        .fixture_revision = "mutable-main".to_string();
    adapter.runtime_mut().manifest =
        serde_json::to_vec(&manifest).unwrap_or_else(|_| unreachable!());
    assert!(
        matches!(adapter.discover(&onix_plan()), Err(PortFailure::Domain(reason)) if reason.code == "onix_manifest_drift")
    );
    assert!(adapter.runtime().prepared.is_empty());
    assert_eq!(adapter.runtime().run_count, 0);

    let fatal = String::from_utf8(valid_stream())
        .unwrap_or_else(|_| unreachable!())
        .replace("\"fatal\":false", "\"fatal\":true")
        .into_bytes();
    let mut adapter = NixEvalAdapter::new(onix_profile(), FakeRuntime::onix(vec![exited(fatal)]))
        .unwrap_or_else(|_| unreachable!());
    let observation = adapter
        .discover(&onix_plan())
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(observation.terminal, DiscoveryTerminal::Failed);
    assert!(observation.tasks.is_empty());
    assert!(observation.job_errors.iter().any(|error| error.fatal));
    assert_eq!(observation.gc_roots.state, GcRootState::Retained);
}

// r[verify kiln.onixos_pilot.failure]
#[test]
fn gc_root_identity_bound_admits_the_boundary_and_rejects_overflow_before_dispatch() {
    let component = "r".repeat(DEFAULT_MAX_ID_BYTES - BLAKE3_HEX_BYTES - "//".len());
    let mut selected = profile();
    selected.runtime.gc_root_base = PathBuf::from(format!("/{component}"));
    let mut adapter = NixEvalAdapter::new(
        selected.clone(),
        FakeRuntime::new(vec![exited(valid_stream())]),
    )
    .unwrap_or_else(|_| unreachable!());
    let observation = adapter.discover(&plan()).unwrap_or_else(|_| unreachable!());
    assert_eq!(observation.terminal, DiscoveryTerminal::Complete);
    assert_eq!(
        observation
            .gc_roots
            .directory
            .as_ref()
            .map(|root| root.native_id.as_str().len()),
        Some(DEFAULT_MAX_ID_BYTES),
    );
    selected.runtime.gc_root_base = PathBuf::from(format!("/{component}r"));
    assert!(matches!(
        admit_profile(&selected),
        Err(PortFailure::Domain(reason)) if reason.code == "nix_eval_gc_root_identity"
    ));
    assert!(matches!(
        adapter::NixEvalAdapter::new(selected, FakeRuntime::new(Vec::new())),
        Err(PortFailure::Domain(reason)) if reason.code == "nix_eval_gc_root_identity"
    ));
}
