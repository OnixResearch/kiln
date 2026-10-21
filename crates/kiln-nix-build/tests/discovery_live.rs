#![cfg(feature = "nix-live")]

mod support;

use std::path::PathBuf;

use kiln_adapter_nix::BoundedNixBuildRuntime;
use kiln_adapter_nix_eval::{
    BoundedNixEvalRuntime, NixEvalAdapter, NixEvalLimits, NixEvalProfile, NixEvalRuntimeBinding,
    PINNED_COHORT_MANIFEST_BLAKE3, PINNED_COHORT_MANIFEST_PATH, PINNED_EXECUTABLE_BLAKE3,
    PINNED_EXECUTABLE_PATH,
};
use kiln_core::discovery::{
    DiscoveryBounds, DiscoveryEffectPlan, DiscoveryOptions, DiscoverySourceMode, DiscoveryTerminal,
};
use kiln_core::types::{
    ContentId, EffectId, EvaluationId, ExternalRef, PipelineRevisionId, PolicyId, ProjectId,
    ProviderProfileId,
};
use kiln_nix_build::build_task;
use kiln_provider_api::DiscoveryPort;
use support::{alias, base, content, intent};

fn job_source() -> String {
    std::env::var("KILN_NIX_JOB_SOURCE")
        .unwrap_or_else(|_| "/nix/store/b8bcc0qabaywjhypl93dafm4mi1dkc9w-jobset".to_string())
}

fn eval_limits() -> NixEvalLimits {
    NixEvalLimits {
        max_line_bytes: 1_048_576,
        max_total_stdout_bytes: 16_777_216,
        max_stderr_bytes: 1_048_576,
        max_records: 256,
        max_jobs: 256,
        max_job_errors: 256,
        max_metadata_bytes: 65_536,
        max_diagnostic_bytes: 65_536,
        max_outputs_per_job: 64,
        max_dependencies_per_job: 256,
        max_tasks: 256,
        max_gc_roots: 256,
        max_executable_bytes: 268_435_456,
        max_manifest_bytes: 65_536,
        max_in_flight: 4,
        max_workers: 4,
        max_memory_mebibytes: 4_096,
        stdin_max_bytes: 1,
        timeout_milliseconds: 60_000,
        poll_interval_milliseconds: 10,
        teardown_timeout_milliseconds: 1_000,
    }
}

fn eval_profile(home: &std::path::Path, roots: &std::path::Path) -> NixEvalProfile {
    NixEvalProfile::pinned(
        ProviderProfileId::new("nix-eval-jobs-v1").unwrap_or_else(|_| unreachable!()),
        NixEvalRuntimeBinding {
            executable_path: PathBuf::from(PINNED_EXECUTABLE_PATH),
            executable_identity: ContentId::from_blake3_hex(PINNED_EXECUTABLE_BLAKE3)
                .unwrap_or_else(|_| unreachable!()),
            cohort_manifest_path: PathBuf::from(PINNED_COHORT_MANIFEST_PATH),
            cohort_manifest_identity: ContentId::from_blake3_hex(PINNED_COHORT_MANIFEST_BLAKE3)
                .unwrap_or_else(|_| unreachable!()),
            gc_root_base: roots.to_path_buf(),
            home_base: home.to_path_buf(),
        },
        eval_limits(),
    )
    .unwrap_or_else(|_| unreachable!())
}

fn discovery_plan() -> DiscoveryEffectPlan {
    DiscoveryEffectPlan {
        effect_id: EffectId::new("discovery-live-a").unwrap_or_else(|_| unreachable!()),
        evaluation_id: EvaluationId::new("evaluation-live-a").unwrap_or_else(|_| unreachable!()),
        project_id: ProjectId::new("project-live-a").unwrap_or_else(|_| unreachable!()),
        pipeline_revision_id: PipelineRevisionId::new("pipeline-live-a")
            .unwrap_or_else(|_| unreachable!()),
        discovery_policy_id: PolicyId::new("policy-live-a").unwrap_or_else(|_| unreachable!()),
        provider_profile: ProviderProfileId::new("nix-eval-jobs-v1")
            .unwrap_or_else(|_| unreachable!()),
        evaluator_cohort: None,
        build_provider_profile: ProviderProfileId::new("nix-build-v1")
            .unwrap_or_else(|_| unreachable!()),
        idempotency_key: content('1'),
        source_identity: content('2'),
        source: ExternalRef::new("kiln-source", "nix-store-path", &job_source())
            .unwrap_or_else(|_| unreachable!()),
        source_mode: DiscoverySourceMode::Flake,
        root_selector: alias("jobs"),
        system: alias("x86_64-linux"),
        options: DiscoveryOptions {
            workers: 1,
            max_memory_mebibytes: 512,
            include_metadata: true,
            include_input_derivations: true,
            check_cache_status: true,
            partial_admission: false,
        },
        bounds: DiscoveryBounds {
            max_records: 16,
            max_jobs: 8,
            max_job_errors: 8,
            max_diagnostic_bytes: 4_096,
            max_outputs_per_job: 8,
            max_dependencies_per_job: 16,
            max_tasks: 8,
        },
    }
}

// r[verify kiln.nix_build.composition] r[verify kiln.nix_build.composition.scenario.e2e]
#[test]
fn real_discovery_task_builds_through_the_composition() -> Result<(), String> {
    let temp = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let home = temp.path().join("eval-home");
    std::fs::create_dir(&home).unwrap_or_else(|_| unreachable!());
    let profile = eval_profile(&home, &temp.path().join("eval-roots"));
    let mut discovery = NixEvalAdapter::new(profile, BoundedNixEvalRuntime)
        .map_err(|error| format!("pinned Nix evaluator cohort is unavailable: {error:?}"))?;
    let observation = discovery
        .discover(&discovery_plan())
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(observation.terminal, DiscoveryTerminal::Complete);
    assert!(observation.job_errors.len() <= 1);
    let task = observation
        .tasks
        .iter()
        .find(|task| task.native_attribute.native_id.as_str() == "job-a")
        .unwrap_or_else(|| unreachable!())
        .clone();

    let mut base = base();
    base.runtime.home_base = temp.path().join("build-home");
    std::fs::create_dir(&base.runtime.home_base).unwrap_or_else(|_| unreachable!());
    base.runtime.gc_root_base = temp.path().join("build-roots");
    let (mut adapter, plan, build) = build_task(&base, &task, &intent(), BoundedNixBuildRuntime)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        build.outcome,
        kiln_core::build::BuildEvaluationOutcome::Success
    );
    let output = build.products.first().unwrap_or_else(|| unreachable!());
    assert_eq!(output.name.as_str(), "out");
    let expected = task.outputs.first().unwrap_or_else(|| unreachable!());
    assert_eq!(expected.path.provider_namespace.as_str(), "nix");
    let path = std::path::PathBuf::from(expected.path.native_id.as_str());
    assert_eq!(
        std::fs::read_to_string(&path).unwrap_or_else(|_| unreachable!()),
        "job-a\n"
    );
    let root = kiln_adapter_nix::gc_root_directory(adapter.profile_binding(), &plan.effect_id)
        .unwrap_or_else(|_| unreachable!());
    assert!(root.exists());
    adapter
        .release_output_roots(&plan.effect_id)
        .unwrap_or_else(|_| unreachable!());
    assert!(!root.exists());
    Ok(())
}
