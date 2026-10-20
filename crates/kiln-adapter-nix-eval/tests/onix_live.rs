use std::collections::BTreeSet;
use std::path::PathBuf;

use kiln_adapter_nix_eval::{
    BoundedNixEvalRuntime, NixEvalAdapter, NixEvalLimits, NixEvalProfile, NixEvalRuntimeBinding,
    ONIX_EXECUTABLE_BLAKE3, ONIX_EXECUTABLE_PATH, ONIX_MANIFEST_BLAKE3, ONIX_MANIFEST_PATH,
    ONIX_PROFILE, ONIX_ROOT_SELECTOR, ONIX_SOURCE_BLAKE3, ONIX_SOURCE_PATH,
    PINNED_COHORT_MANIFEST_BLAKE3, PINNED_COHORT_MANIFEST_PATH, PINNED_EXECUTABLE_BLAKE3,
    PINNED_EXECUTABLE_PATH, admit_plan, gc_root_directory, onix_cohort,
};
use kiln_core::discovery::{
    DiscoveryBounds, DiscoveryEffectPlan, DiscoveryOptions, DiscoverySourceMode, DiscoveryTerminal,
    GcRootState, admit_discovery_observation,
};
use kiln_core::types::{
    Alias, ContentId, EffectId, EvaluationId, ExternalRef, PipelineRevisionId, PolicyId, ProjectId,
    ProviderProfileId,
};
use kiln_provider_api::{DiscoveryPort, PortFailure};

const LINE_BYTES: usize = 65_536;
const STDOUT_BYTES: usize = 1_048_576;
const STDERR_BYTES: usize = 65_536;
const RECORDS: usize = 8;
const JOBS: usize = 4;
const JOB_ERRORS: usize = 4;
const METADATA_BYTES: usize = 16_384;
const DIAGNOSTIC_BYTES: usize = 16_384;
const OUTPUTS: usize = 4;
const DEPENDENCIES: usize = 8;
const TASKS: usize = 4;
const GC_ROOTS: usize = 4;
const EXECUTABLE_BYTES: u64 = 536_870_912;
const MANIFEST_BYTES: u64 = 65_536;
const IN_FLIGHT: usize = 1;
const WORKERS: usize = 1;
const MEMORY_MEBIBYTES: u64 = 1_024;
const STDIN_BYTES: usize = 1;
const TIMEOUT_MILLISECONDS: u64 = 120_000;
const POLL_MILLISECONDS: u64 = 10;
const TEARDOWN_MILLISECONDS: u64 = 1_000;

fn identity(value: &str) -> ContentId {
    ContentId::from_blake3_hex(value).unwrap_or_else(|_| unreachable!())
}

fn limits() -> NixEvalLimits {
    NixEvalLimits {
        max_line_bytes: LINE_BYTES,
        max_total_stdout_bytes: STDOUT_BYTES,
        max_stderr_bytes: STDERR_BYTES,
        max_records: RECORDS,
        max_jobs: JOBS,
        max_job_errors: JOB_ERRORS,
        max_metadata_bytes: METADATA_BYTES,
        max_diagnostic_bytes: DIAGNOSTIC_BYTES,
        max_outputs_per_job: OUTPUTS,
        max_dependencies_per_job: DEPENDENCIES,
        max_tasks: TASKS,
        max_gc_roots: GC_ROOTS,
        max_executable_bytes: EXECUTABLE_BYTES,
        max_manifest_bytes: MANIFEST_BYTES,
        max_in_flight: IN_FLIGHT,
        max_workers: WORKERS,
        max_memory_mebibytes: MEMORY_MEBIBYTES,
        stdin_max_bytes: STDIN_BYTES,
        timeout_milliseconds: TIMEOUT_MILLISECONDS,
        poll_interval_milliseconds: POLL_MILLISECONDS,
        teardown_timeout_milliseconds: TEARDOWN_MILLISECONDS,
    }
}

// r[verify kiln.nix_eval_jobs_onix.fixtures]
// r[verify kiln.nix_eval_jobs_onix.shell]
// r[verify kiln.nix_eval_jobs_onix.compatibility]
#[test]
#[ignore = "requires the exact optional Onix evaluator, plugin, manifest and immutable fixture"]
fn exact_onix_discovers_nickel_jobs_maps_dependency_and_releases_roots() -> Result<(), String> {
    let temporary = tempfile::tempdir().map_err(|error| error.to_string())?;
    let home = temporary.path().join("home");
    std::fs::create_dir(&home).map_err(|error| error.to_string())?;
    let binding = NixEvalRuntimeBinding {
        executable_path: PathBuf::from(ONIX_EXECUTABLE_PATH),
        executable_identity: identity(ONIX_EXECUTABLE_BLAKE3),
        cohort_manifest_path: PathBuf::from(ONIX_MANIFEST_PATH),
        cohort_manifest_identity: identity(ONIX_MANIFEST_BLAKE3),
        gc_root_base: temporary.path().join("roots"),
        home_base: home,
    };
    let profile = NixEvalProfile::onix(binding.clone(), limits())
        .map_err(|error| format!("Onix profile: {error:?}"))?;
    let plan = DiscoveryEffectPlan {
        effect_id: EffectId::new("onix-live-discovery").unwrap_or_else(|_| unreachable!()),
        evaluation_id: EvaluationId::new("onix-live-evaluation").unwrap_or_else(|_| unreachable!()),
        project_id: ProjectId::new("onix-live-project").unwrap_or_else(|_| unreachable!()),
        pipeline_revision_id: PipelineRevisionId::new("onix-live-pipeline")
            .unwrap_or_else(|_| unreachable!()),
        discovery_policy_id: PolicyId::new("onix-live-policy").unwrap_or_else(|_| unreachable!()),
        provider_profile: ProviderProfileId::new(ONIX_PROFILE).unwrap_or_else(|_| unreachable!()),
        evaluator_cohort: Some(onix_cohort().map_err(|error| format!("cohort: {error:?}"))?),
        build_provider_profile: ProviderProfileId::new("nix-build-v1")
            .unwrap_or_else(|_| unreachable!()),
        idempotency_key: identity(ONIX_SOURCE_BLAKE3),
        source_identity: identity(ONIX_SOURCE_BLAKE3),
        source: ExternalRef::new("kiln-source", "nix-store-path", ONIX_SOURCE_PATH)
            .unwrap_or_else(|_| unreachable!()),
        source_mode: DiscoverySourceMode::Flake,
        root_selector: Alias::new(ONIX_ROOT_SELECTOR).unwrap_or_else(|_| unreachable!()),
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
            max_records: RECORDS,
            max_jobs: JOBS,
            max_job_errors: JOB_ERRORS,
            max_diagnostic_bytes: DIAGNOSTIC_BYTES,
            max_outputs_per_job: OUTPUTS,
            max_dependencies_per_job: DEPENDENCIES,
            max_tasks: TASKS,
        },
    };
    let mut stock_binding = binding;
    stock_binding.executable_path = PathBuf::from(PINNED_EXECUTABLE_PATH);
    stock_binding.executable_identity = identity(PINNED_EXECUTABLE_BLAKE3);
    stock_binding.cohort_manifest_path = PathBuf::from(PINNED_COHORT_MANIFEST_PATH);
    stock_binding.cohort_manifest_identity = identity(PINNED_COHORT_MANIFEST_BLAKE3);
    let stock = NixEvalProfile::pinned(
        ProviderProfileId::new("nix-eval-jobs-v1").unwrap_or_else(|_| unreachable!()),
        stock_binding,
        limits(),
    )
    .map_err(|error| format!("stock profile: {error:?}"))?;
    assert!(matches!(
        admit_plan(&stock, &plan),
        Err(PortFailure::Domain(_))
    ));
    let roots = gc_root_directory(&profile, &plan);
    assert!(!roots.exists());
    let mut adapter = NixEvalAdapter::new(profile, BoundedNixEvalRuntime)
        .map_err(|error| format!("preflight: {error:?}"))?;
    let observation = adapter
        .discover(&plan)
        .map_err(|error| format!("discovery: {error:?}"))?;
    assert_eq!(
        observation.terminal,
        DiscoveryTerminal::Complete,
        "{observation:?}"
    );
    admit_discovery_observation(&plan, &observation)
        .map_err(|error| format!("observation admission: {error:?}"))?;
    let seed = observation
        .jobs
        .iter()
        .find(|job| job.name.as_str() == "onix-nickel-seed")
        .ok_or("missing Nickel-evaluated seed")?;
    let report = observation
        .jobs
        .iter()
        .find(|job| job.name.as_str() == "onix-nickel-report")
        .ok_or("missing Nickel-evaluated report")?;
    assert!(report.input_derivations.contains(&seed.derivation));
    let seed_task = observation
        .tasks
        .iter()
        .find(|task| task.derivation == seed.derivation)
        .ok_or("missing seed task")?;
    let report_task = observation
        .tasks
        .iter()
        .find(|task| task.derivation == report.derivation)
        .ok_or("missing report task")?;
    assert_eq!(report_task.dependencies, vec![seed_task.task_id.clone()]);
    assert_eq!(
        observation
            .job_errors
            .iter()
            .map(|error| error.attribute.as_str())
            .collect::<Vec<_>>(),
        vec!["broken"]
    );
    assert_eq!(
        observation
            .job_errors
            .first()
            .map(|error| error.diagnostic.code.as_str()),
        Some("nix_job_evaluation_error")
    );
    assert_eq!(observation.gc_roots.state, GcRootState::Retained);
    assert!(adapter.cleanup_gc_roots(&plan).is_err());
    let durable: BTreeSet<_> = observation
        .tasks
        .iter()
        .map(|task| task.task_id.clone())
        .collect();
    let handed_off = adapter
        .handoff_gc_roots(&plan, &durable)
        .map_err(|error| format!("durable handoff: {error:?}"))?;
    assert_eq!(handed_off, observation.tasks);
    let released = adapter
        .cleanup_gc_roots(&plan)
        .map_err(|error| format!("cleanup: {error:?}"))?;
    assert_eq!(released.gc_roots.state, GcRootState::Released);
    assert!(!roots.exists());
    println!(
        "{}",
        serde_json::to_string(&released).map_err(|error| error.to_string())?
    );
    Ok(())
}
