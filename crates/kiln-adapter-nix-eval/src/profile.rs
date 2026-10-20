use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use kiln_core::discovery::DiscoveryEvaluatorCohort;
use kiln_core::domain::Outcome;
use kiln_core::provider::{
    CapabilityProfile, Operation, OperationRequest, ProviderLimits, ProviderRole, admit_operation,
};
use kiln_core::types::{
    BLAKE3_HEX_BYTES, CONTRACT_VERSION_V1, ContentId, DEFAULT_MAX_ID_BYTES, Diagnostic,
    ExternalRef, ProviderProfileId,
};
use kiln_provider_api::{PortFailure, PortResult, role_matches_port};
use serde::{Deserialize, Serialize};

use crate::onix::{
    ONIX_ADAPTER_VERSION, ONIX_EVALUATOR_REVISION, ONIX_EVALUATOR_SOURCE, ONIX_EXECUTABLE_BLAKE3,
    ONIX_EXECUTABLE_PATH, ONIX_FORK_REVISION, ONIX_FORK_SOURCE, ONIX_NIX_VERSION,
    ONIX_NIXPKGS_REVISION, ONIX_PROFILE, ONIX_SOURCE_VERSION, OnixSourceBinding, admit_onix_cohort,
    admit_onix_source_binding, onix_cohort,
};
use crate::onix_pins::{ONIX_MANIFEST_BLAKE3, ONIX_MANIFEST_PATH};

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

pub const ONIX_REQUIRED_NON_CLAIMS: &[&str] = &[
    "discovery observation does not prove Nix or Nickel evaluation correctness",
    "source identity does not prove source trust",
    "task mapping does not prove complete dependency semantics",
    "cache advisory does not prove cache truth or output presence",
    "discovery observation does not prove build execution or output correctness",
    "exact Onix fixture compatibility does not prove general stock Nix compatibility",
    "discovery observation does not prove reproducibility",
    "discovery observation does not establish release readiness",
    "local process execution does not prove hostile-code confinement",
    "source-defined Wasm handshake does not prove a versioned public host ABI",
    "owner-approved package review source does not establish a production release",
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluator_cohort: Option<DiscoveryEvaluatorCohort>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub onix_source_binding: Option<OnixSourceBinding>,
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
        if profile_id.as_str() == ONIX_PROFILE {
            return Err(domain(
                "nix_eval_unsupported_evaluator",
                "nix_eval.profile",
                "Onix requires the explicit Onix constructor",
            ));
        }
        Self::cohort(profile_id, runtime, limits, false)
    }

    fn cohort(
        profile_id: ProviderProfileId,
        runtime: NixEvalRuntimeBinding,
        limits: NixEvalLimits,
        onix: bool,
    ) -> PortResult<Self> {
        let revision = if onix {
            ONIX_EVALUATOR_REVISION
        } else {
            PINNED_UPSTREAM_REVISION
        };
        Ok(Self {
            upstream_source: if onix {
                ONIX_EVALUATOR_SOURCE
            } else {
                PINNED_UPSTREAM_SOURCE
            }
            .to_string(),
            upstream_revision: revision.to_string(),
            upstream_license: PINNED_UPSTREAM_LICENSE.to_string(),
            adapter_version: if onix {
                ONIX_ADAPTER_VERSION
            } else {
                PINNED_ADAPTER_VERSION
            }
            .to_string(),
            source_version: if onix {
                ONIX_SOURCE_VERSION
            } else {
                PINNED_SOURCE_VERSION
            }
            .to_string(),
            nix_source: if onix {
                ONIX_FORK_SOURCE
            } else {
                PINNED_NIX_SOURCE
            }
            .to_string(),
            nix_revision: if onix {
                ONIX_FORK_REVISION
            } else {
                PINNED_NIX_REVISION
            }
            .to_string(),
            nix_version: if onix {
                ONIX_NIX_VERSION
            } else {
                PINNED_NIX_VERSION
            }
            .to_string(),
            nixpkgs_revision: if onix {
                ONIX_NIXPKGS_REVISION
            } else {
                PINNED_NIXPKGS_REVISION
            }
            .to_string(),
            platform: PINNED_PLATFORM.to_string(),
            cohort_manifest_schema: COHORT_MANIFEST_SCHEMA.to_string(),
            bounded_exec_source: PINNED_BOUNDED_EXEC_SOURCE.to_string(),
            bounded_exec_revision: PINNED_BOUNDED_EXEC_REVISION.to_string(),
            bounded_exec_license: PINNED_BOUNDED_EXEC_LICENSE.to_string(),
            provider: CapabilityProfile {
                contract_version: CONTRACT_VERSION_V1,
                profile_id,
                role: ProviderRole::Discovery,
                namespace: ExternalRef::new("nix-eval-jobs", "discovery-provider", revision)?,
                operations: required_operations(),
                outcomes: required_outcomes(),
                limits: ProviderLimits {
                    max_request_bytes: limits.max_line_bytes,
                    max_result_bytes: limits.max_total_stdout_bytes,
                    max_in_flight: limits.max_in_flight,
                },
            },
            evaluator_cohort: if onix { Some(onix_cohort()?) } else { None },
            onix_source_binding: None,
            runtime,
            policy: pinned_policy(),
            limits,
            required_non_claims: if onix {
                ONIX_REQUIRED_NON_CLAIMS
            } else {
                REQUIRED_NON_CLAIMS
            }
            .iter()
            .map(|claim| (*claim).to_string())
            .collect(),
        })
    }

    // r[impl kiln.nix_eval_jobs_onix.profile]
    // r[impl kiln.onixos_pilot.source]
    pub fn onix(
        runtime: NixEvalRuntimeBinding,
        limits: NixEvalLimits,
        binding: OnixSourceBinding,
    ) -> PortResult<Self> {
        let mut profile =
            Self::cohort(ProviderProfileId::new(ONIX_PROFILE)?, runtime, limits, true)?;
        profile.onix_source_binding = Some(binding);
        admit_profile(&profile)?;
        Ok(profile)
    }
}

// r[impl kiln.nix_eval_jobs.cohort_admission]
// r[impl kiln.nix_eval_jobs.input_admission]
// r[impl kiln.nix_eval_jobs.onix_boundary]
pub fn admit_profile(profile: &NixEvalProfile) -> PortResult<()> {
    let onix = profile.evaluator_cohort.is_some();
    // r[impl kiln.onixos_pilot.source]
    match (onix, &profile.onix_source_binding) {
        (true, Some(binding)) => admit_onix_source_binding(binding)?,
        (false, None) => {}
        _ => {
            return Err(domain(
                "onix_source_binding",
                "nix_eval.profile.onix_source_binding",
                "an explicit source binding is required only for the Onix cohort",
            ));
        }
    }
    if let Some(cohort) = &profile.evaluator_cohort {
        if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            return Err(domain(
                "onix_platform_unsupported",
                "nix_eval.profile",
                "the measured Onix evaluator cohort requires x86_64 Linux",
            ));
        }
        if profile.provider.profile_id.as_str() != ONIX_PROFILE {
            return Err(domain(
                "nix_eval_unsupported_evaluator",
                "nix_eval.profile",
                "Onix requires its explicit provider profile",
            ));
        }
        admit_onix_cohort(cohort)?;
        if profile.provider.namespace.provider_namespace.as_str() != "nix-eval-jobs"
            || profile.provider.namespace.native_kind.as_str() != "discovery-provider"
            || profile.provider.namespace.native_id.as_str() != ONIX_EVALUATOR_REVISION
        {
            return Err(domain(
                "onix_profile_mismatch",
                "nix_eval.profile",
                "Onix provider namespace differs from its source revision",
            ));
        }
    } else if profile.provider.profile_id.as_str() == ONIX_PROFILE {
        return Err(domain(
            "nix_eval_unsupported_evaluator",
            "nix_eval.profile",
            "Onix provider profile requires exact evaluator facts",
        ));
    }
    let (
        upstream_source,
        upstream_revision,
        adapter_version,
        source_version,
        nix_source,
        nix_revision,
        nix_version,
        nixpkgs_revision,
    ) = if onix {
        (
            ONIX_EVALUATOR_SOURCE,
            ONIX_EVALUATOR_REVISION,
            ONIX_ADAPTER_VERSION,
            ONIX_SOURCE_VERSION,
            ONIX_FORK_SOURCE,
            ONIX_FORK_REVISION,
            ONIX_NIX_VERSION,
            ONIX_NIXPKGS_REVISION,
        )
    } else {
        (
            PINNED_UPSTREAM_SOURCE,
            PINNED_UPSTREAM_REVISION,
            PINNED_ADAPTER_VERSION,
            PINNED_SOURCE_VERSION,
            PINNED_NIX_SOURCE,
            PINNED_NIX_REVISION,
            PINNED_NIX_VERSION,
            PINNED_NIXPKGS_REVISION,
        )
    };
    if profile.upstream_source != upstream_source
        || profile.upstream_revision != upstream_revision
        || profile.upstream_license != PINNED_UPSTREAM_LICENSE
        || profile.adapter_version != adapter_version
        || profile.source_version != source_version
        || profile.nix_source != nix_source
        || profile.nix_revision != nix_revision
        || profile.nix_version != nix_version
        || profile.nixpkgs_revision != nixpkgs_revision
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
    let (executable_path, executable_identity, manifest_path, manifest_identity, non_claims) =
        if onix {
            (
                ONIX_EXECUTABLE_PATH,
                ONIX_EXECUTABLE_BLAKE3,
                ONIX_MANIFEST_PATH,
                ONIX_MANIFEST_BLAKE3,
                ONIX_REQUIRED_NON_CLAIMS,
            )
        } else {
            (
                PINNED_EXECUTABLE_PATH,
                PINNED_EXECUTABLE_BLAKE3,
                PINNED_COHORT_MANIFEST_PATH,
                PINNED_COHORT_MANIFEST_BLAKE3,
                REQUIRED_NON_CLAIMS,
            )
        };
    if profile.runtime.executable_path != Path::new(executable_path)
        || profile.runtime.executable_identity.as_str() != executable_identity
        || profile.runtime.cohort_manifest_path != Path::new(manifest_path)
        || profile.runtime.cohort_manifest_identity.as_str() != manifest_identity
        || profile.policy != pinned_policy()
        || !profile
            .required_non_claims
            .iter()
            .map(String::as_str)
            .eq(non_claims.iter().copied())
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
    let root_base = runtime.gc_root_base.to_str().ok_or_else(|| {
        domain(
            "nix_eval_gc_root_identity",
            "nix_eval.profile.runtime.gc_root_base",
            "GC-root directory identity requires a UTF-8 path",
        )
    })?;
    let separator_bytes = if root_base.ends_with(std::path::MAIN_SEPARATOR) {
        0
    } else {
        std::path::MAIN_SEPARATOR_STR.len()
    };
    let maximum_base_bytes = DEFAULT_MAX_ID_BYTES - BLAKE3_HEX_BYTES - separator_bytes;
    if root_base.len() > maximum_base_bytes {
        return Err(domain(
            "nix_eval_gc_root_identity",
            "nix_eval.profile.runtime.gc_root_base",
            "GC-root base and its BLAKE3 directory name exceed the provider-reference bound",
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
