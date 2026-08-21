use std::collections::BTreeSet;
use std::path::PathBuf;

use kiln_core::domain::Outcome;
use kiln_core::provider::{
    CapabilityProfile, Operation, OperationRequest, ProviderLimits, ProviderRole, admit_operation,
};
use kiln_core::types::{
    CONTRACT_VERSION_V1, ContentId, Diagnostic, ExternalRef, ProviderProfileId,
};
use kiln_provider_api::{PortFailure, PortResult, role_matches_port};
use serde::{Deserialize, Serialize};

pub const PINNED_UPSTREAM_SOURCE: &str = "ssh://git@github.com/NixOS/nix-eval-jobs.git";
pub const PINNED_UPSTREAM_REVISION: &str = "a0cd02231c58974a6b5aaa3712069b071047162e";
pub const PINNED_UPSTREAM_LICENSE: &str = "GPL-3.0";
pub const PINNED_ADAPTER_VERSION: &str = "2.35.0";
pub const PINNED_SOURCE_VERSION: &str = "0.1.6";
pub const PINNED_NIX_SOURCE: &str = "ssh://git@github.com/NixOS/nix.git";
pub const PINNED_NIX_REVISION: &str = "aa2613fa02ca6199fe0ebf61a0f7f6d781d32a47";
pub const PINNED_NIX_VERSION: &str = "2.35.2";
pub const PINNED_NIXPKGS_REVISION: &str = "346dd96ad74dc4457a9db9de4f4f57dab2e5731d";
pub const PINNED_PLATFORM: &str = "x86_64-linux";
pub const PINNED_EXECUTABLE_PATH: &str =
    "/nix/store/bcv7hkwwpmfjmvxh6i106gdwr956svsp-nix-eval-jobs-2.35.0/bin/nix-eval-jobs";
pub const PINNED_EXECUTABLE_BLAKE3: &str =
    "dc42bc0e330c598ec93c2361afbb493417fa5c3364722897bd156d3f2ba9c063";
pub const COHORT_MANIFEST_SCHEMA: &str = "kiln.nix-eval-jobs-cohort.v1";
pub const PINNED_COHORT_MANIFEST_PATH: &str =
    "/nix/store/wasviibxqrxh6h8wckaifm2s6lancgpn-kiln-nix-eval-jobs-cohort.json";
pub const PINNED_COHORT_MANIFEST_BLAKE3: &str =
    "102b4af9a753d00dfb6489e1593dd4b9ac8ffe9eed42518705329c6018e63c31";
pub const PINNED_BOUNDED_EXEC_SOURCE: &str =
    "https://git.onix.computer/z2CpqLFpdP36fZXYUK5ZNWxMibpCo.git";
pub const PINNED_BOUNDED_EXEC_REVISION: &str = "29dac88ecded94457572db3fdfaaaab95fa91525";
pub const PINNED_BOUNDED_EXEC_LICENSE: &str = "AGPL-3.0-or-later";

pub const REQUIRED_NON_CLAIMS: &[&str] = &[
    "discovery observation does not prove Nix evaluation correctness",
    "source identity does not prove source trust",
    "task mapping does not prove complete dependency semantics",
    "cache advisory does not prove cache truth or output presence",
    "discovery observation does not prove build execution or output correctness",
    "stock Nix cohort evidence does not prove Onix Nix compatibility",
    "discovery observation does not prove reproducibility",
    "discovery observation does not establish release readiness",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NixEvalLimits {
    pub max_line_bytes: usize,
    pub max_total_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
    pub max_records: usize,
    pub max_jobs: usize,
    pub max_job_errors: usize,
    pub max_metadata_bytes: usize,
    pub max_diagnostic_bytes: usize,
    pub max_outputs_per_job: usize,
    pub max_dependencies_per_job: usize,
    pub max_tasks: usize,
    pub max_gc_roots: usize,
    pub max_executable_bytes: u64,
    pub max_manifest_bytes: u64,
    pub max_in_flight: usize,
    pub max_workers: usize,
    pub max_memory_mebibytes: u64,
    pub stdin_max_bytes: usize,
    pub timeout_milliseconds: u64,
    pub poll_interval_milliseconds: u64,
    pub teardown_timeout_milliseconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NixEvalRuntimeBinding {
    pub executable_path: PathBuf,
    pub executable_identity: ContentId,
    pub cohort_manifest_path: PathBuf,
    pub cohort_manifest_identity: ContentId,
    pub gc_root_base: PathBuf,
    pub home_base: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NixEvalPolicy {
    pub flake_mode: bool,
    pub expression_mode: bool,
    pub allow_apply: bool,
    pub allow_impure: bool,
    pub allow_force_recurse: bool,
    pub allow_registries: bool,
    pub retain_gc_roots: bool,
    pub partial_admission: bool,
    pub check_cache_status: bool,
    pub include_metadata: bool,
    pub include_input_derivations: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NixEvalProfile {
    pub upstream_source: String,
    pub upstream_revision: String,
    pub upstream_license: String,
    pub adapter_version: String,
    pub source_version: String,
    pub nix_source: String,
    pub nix_revision: String,
    pub nix_version: String,
    pub nixpkgs_revision: String,
    pub platform: String,
    pub cohort_manifest_schema: String,
    pub bounded_exec_source: String,
    pub bounded_exec_revision: String,
    pub bounded_exec_license: String,
    pub provider: CapabilityProfile,
    pub runtime: NixEvalRuntimeBinding,
    pub policy: NixEvalPolicy,
    pub limits: NixEvalLimits,
    pub required_non_claims: Vec<String>,
}

impl NixEvalProfile {
    pub fn pinned(
        profile_id: ProviderProfileId,
        runtime: NixEvalRuntimeBinding,
        limits: NixEvalLimits,
    ) -> PortResult<Self> {
        Ok(Self {
            upstream_source: PINNED_UPSTREAM_SOURCE.to_string(),
            upstream_revision: PINNED_UPSTREAM_REVISION.to_string(),
            upstream_license: PINNED_UPSTREAM_LICENSE.to_string(),
            adapter_version: PINNED_ADAPTER_VERSION.to_string(),
            source_version: PINNED_SOURCE_VERSION.to_string(),
            nix_source: PINNED_NIX_SOURCE.to_string(),
            nix_revision: PINNED_NIX_REVISION.to_string(),
            nix_version: PINNED_NIX_VERSION.to_string(),
            nixpkgs_revision: PINNED_NIXPKGS_REVISION.to_string(),
            platform: PINNED_PLATFORM.to_string(),
            cohort_manifest_schema: COHORT_MANIFEST_SCHEMA.to_string(),
            bounded_exec_source: PINNED_BOUNDED_EXEC_SOURCE.to_string(),
            bounded_exec_revision: PINNED_BOUNDED_EXEC_REVISION.to_string(),
            bounded_exec_license: PINNED_BOUNDED_EXEC_LICENSE.to_string(),
            provider: CapabilityProfile {
                contract_version: CONTRACT_VERSION_V1,
                profile_id,
                role: ProviderRole::Discovery,
                namespace: ExternalRef::new(
                    "nix-eval-jobs",
                    "discovery-provider",
                    PINNED_UPSTREAM_REVISION,
                )?,
                operations: required_operations(),
                outcomes: required_outcomes(),
                limits: ProviderLimits {
                    max_request_bytes: limits.max_line_bytes,
                    max_result_bytes: limits.max_total_stdout_bytes,
                    max_in_flight: limits.max_in_flight,
                },
            },
            runtime,
            policy: pinned_policy(),
            limits,
            required_non_claims: REQUIRED_NON_CLAIMS
                .iter()
                .map(|claim| (*claim).to_string())
                .collect(),
        })
    }
}

// r[impl kiln.nix_eval_jobs.cohort_admission]
// r[impl kiln.nix_eval_jobs.input_admission]
// r[impl kiln.nix_eval_jobs.onix_boundary]
pub fn admit_profile(profile: &NixEvalProfile) -> PortResult<()> {
    if profile.upstream_source != PINNED_UPSTREAM_SOURCE
        || profile.upstream_revision != PINNED_UPSTREAM_REVISION
        || profile.upstream_license != PINNED_UPSTREAM_LICENSE
        || profile.adapter_version != PINNED_ADAPTER_VERSION
        || profile.source_version != PINNED_SOURCE_VERSION
        || profile.nix_source != PINNED_NIX_SOURCE
        || profile.nix_revision != PINNED_NIX_REVISION
        || profile.nix_version != PINNED_NIX_VERSION
        || profile.nixpkgs_revision != PINNED_NIXPKGS_REVISION
        || profile.platform != PINNED_PLATFORM
        || profile.cohort_manifest_schema != COHORT_MANIFEST_SCHEMA
        || profile.bounded_exec_source != PINNED_BOUNDED_EXEC_SOURCE
        || profile.bounded_exec_revision != PINNED_BOUNDED_EXEC_REVISION
        || profile.bounded_exec_license != PINNED_BOUNDED_EXEC_LICENSE
    {
        return Err(domain(
            "nix_eval_cohort",
            "nix_eval.profile",
            "Nix evaluation profile differs from the pinned producer cohort",
        ));
    }
    if profile.runtime.executable_path != PathBuf::from(PINNED_EXECUTABLE_PATH)
        || profile.runtime.executable_identity.as_str() != PINNED_EXECUTABLE_BLAKE3
        || profile.runtime.cohort_manifest_path != PathBuf::from(PINNED_COHORT_MANIFEST_PATH)
        || profile.runtime.cohort_manifest_identity.as_str() != PINNED_COHORT_MANIFEST_BLAKE3
        || profile.policy != pinned_policy()
        || profile.required_non_claims
            != REQUIRED_NON_CLAIMS
                .iter()
                .map(|claim| (*claim).to_string())
                .collect::<Vec<_>>()
        || profile.provider.operations != required_operations()
        || profile.provider.outcomes != required_outcomes()
        || profile.provider.limits
            != (ProviderLimits {
                max_request_bytes: profile.limits.max_line_bytes,
                max_result_bytes: profile.limits.max_total_stdout_bytes,
                max_in_flight: profile.limits.max_in_flight,
            })
    {
        return Err(domain(
            "nix_eval_profile_binding",
            "nix_eval.profile",
            "Nix evaluation identities, policies, or capabilities differ from the cohort",
        ));
    }
    validate_paths(profile)?;
    validate_limits(profile.limits)?;
    role_matches_port(&profile.provider, ProviderRole::Discovery)?;
    for operation in &profile.provider.operations {
        admit_operation(
            &profile.provider,
            &OperationRequest {
                contract_version: CONTRACT_VERSION_V1,
                selected_profile: profile.provider.profile_id.clone(),
                role: ProviderRole::Discovery,
                operation: *operation,
                request_bytes: 0,
                required_outcomes: profile.provider.outcomes.clone(),
            },
        )?;
    }
    Ok(())
}

fn validate_paths(profile: &NixEvalProfile) -> PortResult<()> {
    let runtime = &profile.runtime;
    if !runtime.executable_path.is_absolute()
        || !runtime.cohort_manifest_path.is_absolute()
        || !runtime.gc_root_base.is_absolute()
        || !runtime.home_base.is_absolute()
    {
        return Err(domain(
            "nix_eval_path",
            "nix_eval.profile.runtime",
            "Nix evaluation runtime paths must be absolute",
        ));
    }
    Ok(())
}

fn validate_limits(limits: NixEvalLimits) -> PortResult<()> {
    if limits.max_line_bytes == 0
        || limits.max_total_stdout_bytes < limits.max_line_bytes
        || limits.max_stderr_bytes == 0
        || limits.max_records == 0
        || limits.max_jobs == 0
        || limits.max_job_errors == 0
        || limits.max_jobs > limits.max_records
        || limits.max_job_errors > limits.max_records
        || limits.max_metadata_bytes == 0
        || limits.max_diagnostic_bytes == 0
        || limits.max_outputs_per_job == 0
        || limits.max_dependencies_per_job == 0
        || limits.max_tasks == 0
        || limits.max_tasks > limits.max_jobs
        || limits.max_gc_roots == 0
        || limits.max_gc_roots < limits.max_tasks
        || limits.max_executable_bytes == 0
        || limits.max_manifest_bytes == 0
        || limits.max_in_flight == 0
        || limits.max_workers == 0
        || limits.max_memory_mebibytes == 0
        || limits.stdin_max_bytes == 0
        || limits.timeout_milliseconds == 0
        || limits.poll_interval_milliseconds == 0
        || limits.poll_interval_milliseconds > limits.timeout_milliseconds
        || limits.teardown_timeout_milliseconds == 0
    {
        return Err(domain(
            "nix_eval_limit",
            "nix_eval.profile.limits",
            "Nix evaluation limits are zero or internally inconsistent",
        ));
    }
    Ok(())
}

fn pinned_policy() -> NixEvalPolicy {
    NixEvalPolicy {
        flake_mode: true,
        expression_mode: false,
        allow_apply: false,
        allow_impure: false,
        allow_force_recurse: false,
        allow_registries: false,
        retain_gc_roots: true,
        partial_admission: false,
        check_cache_status: true,
        include_metadata: true,
        include_input_derivations: true,
    }
}

fn required_operations() -> BTreeSet<Operation> {
    BTreeSet::from([
        Operation::Discover,
        Operation::Observe,
        Operation::Cancel,
        Operation::Reconcile,
    ])
}

fn required_outcomes() -> BTreeSet<Outcome> {
    BTreeSet::from([
        Outcome::Success,
        Outcome::DomainFailure,
        Outcome::Cancelled,
        Outcome::TimedOut,
        Outcome::ProviderError,
        Outcome::Unknown,
    ])
}

fn domain(code: &str, field: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, field, message))
}
