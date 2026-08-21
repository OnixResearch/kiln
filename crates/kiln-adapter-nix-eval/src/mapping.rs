use std::ffi::OsString;
use std::path::{Path, PathBuf};

use bounded_exec::ExecutionLimits;
use kiln_core::discovery::{
    DiscoveryEffectPlan, DiscoveryObservation, DiscoverySourceMode, DiscoveryTerminal,
    GcRootObservation, classify_discovery_terminal, compute_discovery_observation_identity,
    map_task_proposals, validate_discovery_plan,
};
use kiln_core::types::Diagnostic;
use kiln_provider_api::{PortFailure, PortResult};
use serde::{Deserialize, Serialize};

use crate::profile::{
    COHORT_MANIFEST_SCHEMA, NixEvalProfile, PINNED_ADAPTER_VERSION, PINNED_NIX_REVISION,
    PINNED_NIX_VERSION, PINNED_NIXPKGS_REVISION, PINNED_PLATFORM, PINNED_SOURCE_VERSION,
    PINNED_UPSTREAM_REVISION,
};
use crate::runtime::ProcessInvocation;
use crate::wire::StreamAdmission;

const NIX_STORE_PREFIX: &str = "/nix/store/";
const NIX_CONFIGURATION: &str = "accept-flake-config = false\nflake-registry =\nsubstituters =\n";
const EXPERIMENTAL_FEATURES: &str = "nix-command flakes";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CohortManifest {
    pub schema: String,
    pub upstream_revision: String,
    pub adapter_version: String,
    pub source_version: String,
    pub nix_revision: String,
    pub nix_version: String,
    pub nixpkgs_revision: String,
    pub platform: String,
    pub executable: String,
}

pub fn admit_cohort_manifest(profile: &NixEvalProfile, bytes: &[u8]) -> PortResult<CohortManifest> {
    let manifest: CohortManifest = serde_json::from_slice(bytes).map_err(|_| {
        domain(
            "nix_eval_manifest_schema",
            "nix_eval.cohort_manifest",
            "cohort manifest differs from its admitted JSON schema",
        )
    })?;
    if manifest.schema != COHORT_MANIFEST_SCHEMA
        || manifest.upstream_revision != PINNED_UPSTREAM_REVISION
        || manifest.adapter_version != PINNED_ADAPTER_VERSION
        || manifest.source_version != PINNED_SOURCE_VERSION
        || manifest.nix_revision != PINNED_NIX_REVISION
        || manifest.nix_version != PINNED_NIX_VERSION
        || manifest.nixpkgs_revision != PINNED_NIXPKGS_REVISION
        || manifest.platform != PINNED_PLATFORM
        || manifest.executable != profile.runtime.executable_path.to_string_lossy()
    {
        return Err(domain(
            "nix_eval_manifest_cohort",
            "nix_eval.cohort_manifest",
            "cohort manifest does not bind the selected executable and evaluator versions",
        ));
    }
    Ok(manifest)
}

// r[impl kiln.nix_eval_jobs.input_admission]
pub fn admit_plan(profile: &NixEvalProfile, plan: &DiscoveryEffectPlan) -> PortResult<()> {
    validate_discovery_plan(plan).map_err(PortFailure::Domain)?;
    if plan.provider_profile != profile.provider.profile_id
        || plan.source_mode != DiscoverySourceMode::Flake
        || plan.system.as_str() != profile.platform
        || plan.options.partial_admission != profile.policy.partial_admission
        || plan.options.check_cache_status != profile.policy.check_cache_status
        || plan.options.include_metadata != profile.policy.include_metadata
        || plan.options.include_input_derivations != profile.policy.include_input_derivations
        || plan.options.workers > profile.limits.max_workers
        || plan.options.max_memory_mebibytes > profile.limits.max_memory_mebibytes
        || plan.bounds.max_records > profile.limits.max_records
        || plan.bounds.max_jobs > profile.limits.max_jobs
        || plan.bounds.max_job_errors > profile.limits.max_job_errors
        || plan.bounds.max_diagnostic_bytes > profile.limits.max_diagnostic_bytes
        || plan.bounds.max_outputs_per_job > profile.limits.max_outputs_per_job
        || plan.bounds.max_dependencies_per_job > profile.limits.max_dependencies_per_job
        || plan.bounds.max_tasks > profile.limits.max_tasks
    {
        return Err(domain(
            "nix_eval_plan_profile",
            "nix_eval.plan",
            "discovery plan differs from the selected profile or exceeds its limits",
        ));
    }
    if plan.source.provider_namespace.as_str() != "kiln-source"
        || plan.source.native_kind.as_str() != "nix-store-path"
        || !plan.source.native_id.as_str().starts_with(NIX_STORE_PREFIX)
        || plan.source.native_id.as_str().contains('#')
        || !selector_is_supported(plan.root_selector.as_str())
    {
        return Err(domain(
            "nix_eval_source",
            "nix_eval.plan.source",
            "source must be an immutable Nix store path with a supported root selector",
        ));
    }
    Ok(())
}

#[must_use]
pub fn gc_root_directory(profile: &NixEvalProfile, plan: &DiscoveryEffectPlan) -> PathBuf {
    let mut hasher = blake3::Hasher::new();
    hasher.update(plan.effect_id.as_str().as_bytes());
    hasher.update(plan.idempotency_key.as_str().as_bytes());
    profile
        .runtime
        .gc_root_base
        .join(hasher.finalize().to_hex().as_str())
}

pub fn invocation(
    profile: &NixEvalProfile,
    plan: &DiscoveryEffectPlan,
    gc_root_directory: &Path,
) -> ProcessInvocation {
    let source = plan.source.native_id.as_str();
    let selector = plan.root_selector.as_str();
    let target = format!("{source}#{selector}");
    let mut args = vec![
        OsString::from("--gc-roots-dir"),
        gc_root_directory.as_os_str().to_os_string(),
        OsString::from("--workers"),
        OsString::from(plan.options.workers.to_string()),
        OsString::from("--max-memory-size"),
        OsString::from(plan.options.max_memory_mebibytes.to_string()),
        OsString::from("--extra-experimental-features"),
        OsString::from(EXPERIMENTAL_FEATURES),
    ];
    if plan.options.include_metadata {
        args.push(OsString::from("--meta"));
    }
    if plan.options.include_input_derivations {
        args.push(OsString::from("--show-input-drvs"));
    }
    if plan.options.check_cache_status {
        args.push(OsString::from("--check-cache-status"));
    }
    args.push(OsString::from("--flake"));
    args.push(OsString::from(target));
    ProcessInvocation {
        program: profile.runtime.executable_path.clone(),
        args,
        current_dir: PathBuf::from(source),
        environment: vec![
            (
                OsString::from("HOME"),
                profile.runtime.home_base.clone().into_os_string(),
            ),
            (
                OsString::from("NIX_CONFIG"),
                OsString::from(NIX_CONFIGURATION),
            ),
        ],
        limits: ExecutionLimits {
            timeout_ms: profile.limits.timeout_milliseconds,
            stdin_max_bytes: profile.limits.stdin_max_bytes,
            stdout_max_bytes: profile.limits.max_total_stdout_bytes,
            stderr_max_bytes: profile.limits.max_stderr_bytes,
            poll_interval_ms: profile.limits.poll_interval_milliseconds,
            teardown_timeout_ms: profile.limits.teardown_timeout_milliseconds,
        },
    }
}

pub fn replace_gc_root_observation(
    plan: &DiscoveryEffectPlan,
    observation: &DiscoveryObservation,
    gc_roots: GcRootObservation,
) -> PortResult<DiscoveryObservation> {
    let record_count = observation
        .jobs
        .len()
        .checked_add(observation.job_errors.len())
        .ok_or_else(|| {
            domain(
                "nix_eval_record_overflow",
                "nix_eval.observation",
                "discovery observation record count overflowed",
            )
        })?;
    build_observation(
        plan,
        StreamAdmission {
            jobs: observation.jobs.clone(),
            errors: observation.job_errors.clone(),
            record_count,
            total_bytes: 0,
        },
        observation.completion,
        gc_roots,
    )
}

pub fn build_observation(
    plan: &DiscoveryEffectPlan,
    stream: StreamAdmission,
    completion: kiln_core::discovery::DiscoveryCompletionFacts,
    gc_roots: GcRootObservation,
) -> PortResult<DiscoveryObservation> {
    let terminal = classify_discovery_terminal(completion);
    let tasks = if terminal == DiscoveryTerminal::Complete {
        map_task_proposals(plan, &stream.jobs).map_err(PortFailure::Domain)?
    } else {
        Vec::new()
    };
    let observation_identity = compute_discovery_observation_identity(
        plan,
        terminal,
        &stream.jobs,
        &stream.errors,
        &tasks,
        completion,
        &gc_roots,
    );
    Ok(DiscoveryObservation {
        effect_id: plan.effect_id.clone(),
        evaluation_id: plan.evaluation_id.clone(),
        project_id: plan.project_id.clone(),
        pipeline_revision_id: plan.pipeline_revision_id.clone(),
        discovery_policy_id: plan.discovery_policy_id.clone(),
        provider_profile: plan.provider_profile.clone(),
        idempotency_key: plan.idempotency_key.clone(),
        source_identity: plan.source_identity.clone(),
        observation_identity,
        terminal,
        jobs: stream.jobs,
        job_errors: stream.errors,
        tasks,
        completion,
        gc_roots,
    })
}

fn selector_is_supported(selector: &str) -> bool {
    !selector.is_empty()
        && selector
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn domain(code: &str, field: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, field, message))
}
