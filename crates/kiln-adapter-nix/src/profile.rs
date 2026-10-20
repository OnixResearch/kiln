use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use kiln_core::domain::Outcome;
use kiln_core::provider::{
    CapabilityProfile, Operation, OperationRequest, ProviderLimits, ProviderRole, admit_operation,
};
use kiln_core::types::{
    Alias, CONTRACT_VERSION_V1, ContentId, Diagnostic, ExternalRef, ProviderProfileId,
};
use kiln_provider_api::{PortFailure, PortResult, role_matches_port};
use serde::{Deserialize, Serialize};

use crate::wire::{
    MAXIMUM_GC_ROOTS, MAXIMUM_LOGS, MAXIMUM_PRODUCTS, MAXIMUM_REQUIRED_PRODUCTS,
    NATIVE_REPORT_SCHEMA, OBSERVATION_SCHEMA, RECEIPT_SCHEMA, REQUEST_SCHEMA, REQUIRED_NON_CLAIMS,
    is_safe_absolute_path, is_valid_product_name,
};

pub const PINNED_NIXPKGS_REVISION: &str = "346dd96ad74dc4457a9db9de4f4f57dab2e5731d";
pub const PINNED_NIX_VERSION: &str = "2.34.8";
pub const PINNED_NIX_PLATFORM: &str = "x86_64-linux";
pub const PINNED_NIX_COMMAND: &str = "realise";
pub const PINNED_NIX_EXECUTABLE_PATH: &str =
    "/nix/store/g1k5iczz0l8i8d7kk5bk8rwv139g3l02-nix-2.34.8/bin/nix";
pub const PINNED_NIX_EXECUTABLE_BLAKE3: &str =
    "82a5990db42a7f104b2c6eb17edf840ac9a7ad04936c8ab2630cc766e929f852";
pub const PINNED_NIX_STORE_EXECUTABLE_PATH: &str =
    "/nix/store/g1k5iczz0l8i8d7kk5bk8rwv139g3l02-nix-2.34.8/bin/nix-store";
pub const PINNED_NIX_STORE_EXECUTABLE_BLAKE3: &str =
    "82a5990db42a7f104b2c6eb17edf840ac9a7ad04936c8ab2630cc766e929f852";
pub const PINNED_NIX_CONFIG: &str =
    "accept-flake-config = false\nflake-registry =\nsubstituters =\nbuilders =\n";
pub const PINNED_BOUNDED_EXEC_SOURCE: &str =
    "https://git.onix.computer/z2CpqLFpdP36fZXYUK5ZNWxMibpCo.git";
pub const PINNED_BOUNDED_EXEC_REVISION: &str = "29dac88ecded94457572db3fdfaaaab95fa91525";
pub const PINNED_BOUNDED_EXEC_LICENSE: &str = "AGPL-3.0-or-later";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NixBuildLimits {
    pub max_required_products: usize,
    pub max_products: usize,
    pub max_logs: usize,
    pub max_gc_roots: usize,
    pub max_request_bytes: usize,
    pub max_result_bytes: usize,
    pub max_stderr_bytes: usize,
    pub max_log_bytes: usize,
    pub max_executable_bytes: u64,
    pub max_derivation_bytes: u64,
    pub max_in_flight: usize,
    pub stdin_max_bytes: usize,
    pub timeout_milliseconds: u64,
    pub poll_interval_milliseconds: u64,
    pub teardown_timeout_milliseconds: u64,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct NixOutputBinding {
    pub name: Alias,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NixRuntimeBinding {
    pub executable_path: PathBuf,
    pub executable_identity: ContentId,
    pub nix_store_executable_path: PathBuf,
    pub nix_store_executable_identity: ContentId,
    pub derivation_path: PathBuf,
    pub derivation_identity: ContentId,
    pub plan_identity: ContentId,
    pub policy_identity: ContentId,
    pub store_path: PathBuf,
    pub gc_root_base: PathBuf,
    pub home_base: PathBuf,
    pub worker_identity: ContentId,
    pub store_identity: ContentId,
    pub expected_outputs: Vec<NixOutputBinding>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NixBuildProfileConfig {
    pub nixpkgs_revision: String,
    pub nix_version: String,
    pub bounded_exec_source: String,
    pub bounded_exec_revision: String,
    pub bounded_exec_license: String,
    pub request_schema: String,
    pub observation_schema: String,
    pub receipt_schema: String,
    pub native_report_schema: String,
    pub command: String,
    pub platform: String,
    pub provider_profile_id: String,
    pub capabilities: Vec<String>,
    pub executable_path: PathBuf,
    pub executable_identity: ContentId,
    pub nix_store_executable_path: PathBuf,
    pub nix_store_executable_identity: ContentId,
    pub derivation_path: PathBuf,
    pub derivation_identity: ContentId,
    pub plan_identity: ContentId,
    pub policy_identity: ContentId,
    pub store_path: PathBuf,
    pub gc_root_base: PathBuf,
    pub home_base: PathBuf,
    pub worker_identity: ContentId,
    pub store_identity: ContentId,
    pub expected_outputs: Vec<NixOutputBinding>,
    pub limits: NixBuildLimits,
    pub required_non_claims: Vec<String>,
}

impl NixBuildProfileConfig {
    pub fn into_profile(self) -> PortResult<NixBuildProfile> {
        let capabilities = self
            .capabilities
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        if capabilities != REQUIRED_CAPABILITIES {
            return Err(domain(
                "nix_profile_capabilities",
                "nix.profile.capabilities",
                "Nix profile capabilities differ from the pinned provider contract",
            ));
        }
        let profile_id =
            ProviderProfileId::new(self.provider_profile_id.clone()).map_err(|error| {
                domain(
                    "nix_profile_id",
                    "nix.profile.provider_profile_id",
                    &error.message,
                )
            })?;
        let runtime = NixRuntimeBinding {
            executable_path: self.executable_path,
            executable_identity: self.executable_identity,
            nix_store_executable_path: self.nix_store_executable_path,
            nix_store_executable_identity: self.nix_store_executable_identity,
            derivation_path: self.derivation_path,
            derivation_identity: self.derivation_identity,
            plan_identity: self.plan_identity,
            policy_identity: self.policy_identity,
            store_path: self.store_path,
            gc_root_base: self.gc_root_base,
            home_base: self.home_base,
            worker_identity: self.worker_identity,
            store_identity: self.store_identity,
            expected_outputs: self.expected_outputs,
        };
        let mut profile = NixBuildProfile::pinned(profile_id, runtime, self.limits)?;
        profile.nixpkgs_revision = self.nixpkgs_revision;
        profile.nix_version = self.nix_version;
        profile.bounded_exec_source = self.bounded_exec_source;
        profile.bounded_exec_revision = self.bounded_exec_revision;
        profile.bounded_exec_license = self.bounded_exec_license;
        profile.request_schema = self.request_schema;
        profile.observation_schema = self.observation_schema;
        profile.receipt_schema = self.receipt_schema;
        profile.native_report_schema = self.native_report_schema;
        profile.command = self.command;
        profile.platform = self.platform;
        profile.required_non_claims = self.required_non_claims;
        admit_profile(&profile)?;
        Ok(profile)
    }
}

const REQUIRED_CAPABILITIES: [&str; 5] =
    ["cancel", "dispatch", "observe", "read_logs", "reconcile"];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NixBuildProfile {
    pub nixpkgs_revision: String,
    pub nix_version: String,
    pub bounded_exec_source: String,
    pub bounded_exec_revision: String,
    pub bounded_exec_license: String,
    pub request_schema: String,
    pub observation_schema: String,
    pub receipt_schema: String,
    pub native_report_schema: String,
    pub command: String,
    pub platform: String,
    pub provider: CapabilityProfile,
    pub runtime: NixRuntimeBinding,
    pub limits: NixBuildLimits,
    pub required_non_claims: Vec<String>,
}

impl NixBuildProfile {
    pub fn pinned(
        profile_id: ProviderProfileId,
        runtime: NixRuntimeBinding,
        limits: NixBuildLimits,
    ) -> PortResult<Self> {
        Ok(Self {
            nixpkgs_revision: PINNED_NIXPKGS_REVISION.to_string(),
            nix_version: PINNED_NIX_VERSION.to_string(),
            bounded_exec_source: PINNED_BOUNDED_EXEC_SOURCE.to_string(),
            bounded_exec_revision: PINNED_BOUNDED_EXEC_REVISION.to_string(),
            bounded_exec_license: PINNED_BOUNDED_EXEC_LICENSE.to_string(),
            request_schema: REQUEST_SCHEMA.to_string(),
            observation_schema: OBSERVATION_SCHEMA.to_string(),
            receipt_schema: RECEIPT_SCHEMA.to_string(),
            native_report_schema: NATIVE_REPORT_SCHEMA.to_string(),
            command: PINNED_NIX_COMMAND.to_string(),
            platform: PINNED_NIX_PLATFORM.to_string(),
            provider: CapabilityProfile {
                contract_version: CONTRACT_VERSION_V1,
                profile_id,
                role: ProviderRole::Build,
                namespace: ExternalRef::new("nix", "build-provider", PINNED_NIX_VERSION)?,
                operations: required_operations(),
                outcomes: required_outcomes(),
                limits: ProviderLimits {
                    max_request_bytes: limits.max_request_bytes,
                    max_result_bytes: limits.max_result_bytes,
                    max_in_flight: limits.max_in_flight,
                },
            },
            runtime,
            limits,
            required_non_claims: REQUIRED_NON_CLAIMS
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
        })
    }
}

// r[impl kiln.nix_build.cohort] r[impl kiln.nix_build.nonclaims]
pub fn admit_profile(profile: &NixBuildProfile) -> PortResult<()> {
    if profile.nixpkgs_revision != PINNED_NIXPKGS_REVISION
        || profile.nix_version != PINNED_NIX_VERSION
        || profile.bounded_exec_source != PINNED_BOUNDED_EXEC_SOURCE
        || profile.bounded_exec_revision != PINNED_BOUNDED_EXEC_REVISION
        || profile.bounded_exec_license != PINNED_BOUNDED_EXEC_LICENSE
        || profile.request_schema != REQUEST_SCHEMA
        || profile.observation_schema != OBSERVATION_SCHEMA
        || profile.receipt_schema != RECEIPT_SCHEMA
        || profile.native_report_schema != NATIVE_REPORT_SCHEMA
        || profile.command != PINNED_NIX_COMMAND
        || profile.platform != PINNED_NIX_PLATFORM
    {
        return Err(domain(
            "nix_profile_cohort",
            "nix.profile",
            "Nix build profile differs from the pinned native cohort",
        ));
    }
    if profile.runtime.executable_path != Path::new(PINNED_NIX_EXECUTABLE_PATH)
        || profile.runtime.executable_identity.as_str() != PINNED_NIX_EXECUTABLE_BLAKE3
        || profile.runtime.nix_store_executable_path != Path::new(PINNED_NIX_STORE_EXECUTABLE_PATH)
        || profile.runtime.nix_store_executable_identity.as_str()
            != PINNED_NIX_STORE_EXECUTABLE_BLAKE3
        || profile.provider.namespace.provider_namespace.as_str() != "nix"
        || profile.provider.namespace.native_kind.as_str() != "build-provider"
        || profile.provider.namespace.native_id.as_str() != PINNED_NIX_VERSION
        || profile.required_non_claims
            != REQUIRED_NON_CLAIMS
                .iter()
                .map(|value| (*value).to_string())
                .collect::<Vec<_>>()
        || profile.provider.operations != required_operations()
        || profile.provider.outcomes != required_outcomes()
        || profile.provider.limits
            != (ProviderLimits {
                max_request_bytes: profile.limits.max_request_bytes,
                max_result_bytes: profile.limits.max_result_bytes,
                max_in_flight: profile.limits.max_in_flight,
            })
    {
        return Err(domain(
            "nix_profile_binding",
            "nix.profile",
            "Nix profile identities, capabilities, or non-claims differ from the cohort",
        ));
    }
    validate_paths(profile)?;
    validate_limits(profile.limits)?;
    validate_outputs(&profile.runtime, profile.limits)?;
    role_matches_port(&profile.provider, ProviderRole::Build)?;
    for operation in &profile.provider.operations {
        admit_operation(
            &profile.provider,
            &OperationRequest {
                contract_version: CONTRACT_VERSION_V1,
                selected_profile: profile.provider.profile_id.clone(),
                role: ProviderRole::Build,
                operation: *operation,
                request_bytes: 0,
                required_outcomes: profile.provider.outcomes.clone(),
            },
        )?;
    }
    Ok(())
}

fn validate_paths(profile: &NixBuildProfile) -> PortResult<()> {
    let runtime = &profile.runtime;
    if !runtime.executable_path.is_absolute()
        || !runtime.nix_store_executable_path.is_absolute()
        || !runtime.derivation_path.is_absolute()
        || !runtime.store_path.is_absolute()
        || !runtime.gc_root_base.is_absolute()
        || !runtime.home_base.is_absolute()
    {
        return Err(domain(
            "nix_profile_path",
            "nix.profile.runtime",
            "Nix executable, derivation, store, root, and home paths must be absolute",
        ));
    }
    let store_path = runtime.store_path.to_str().ok_or_else(|| {
        domain(
            "nix_profile_path",
            "nix.profile.runtime.store_path",
            "Nix store path must be valid UTF-8",
        )
    })?;
    let derivation_path = runtime.derivation_path.to_str().ok_or_else(|| {
        domain(
            "nix_profile_path",
            "nix.profile.runtime.derivation_path",
            "Nix derivation path must be valid UTF-8",
        )
    })?;
    if !is_safe_absolute_path(store_path)
        || !is_safe_absolute_path(derivation_path)
        || runtime.derivation_path.parent() != Some(runtime.store_path.as_path())
        || runtime.derivation_path.extension() != Some(OsStr::new("drv"))
    {
        return Err(domain(
            "nix_profile_derivation",
            "nix.profile.runtime.derivation_path",
            "Nix derivation must be one safe direct child of the admitted store",
        ));
    }
    Ok(())
}

fn validate_outputs(runtime: &NixRuntimeBinding, limits: NixBuildLimits) -> PortResult<()> {
    if runtime.expected_outputs.is_empty()
        || runtime.expected_outputs.len() > limits.max_required_products
        || runtime.expected_outputs.len() > limits.max_products
    {
        return Err(domain(
            "nix_profile_outputs",
            "nix.profile.runtime.expected_outputs",
            "Nix output count is empty or exceeds its named bound",
        ));
    }
    let mut previous = None;
    let mut paths = BTreeSet::new();
    for output in &runtime.expected_outputs {
        if !is_valid_product_name(output.name.as_str()) {
            return Err(domain(
                "nix_profile_output_name",
                "nix.profile.runtime.expected_outputs",
                "Nix output name is not a bounded output name",
            ));
        }
        if previous.is_some_and(|previous| previous >= &output.name) {
            return Err(domain(
                "nix_profile_output_order",
                "nix.profile.runtime.expected_outputs",
                "Nix outputs must be sorted by unique name",
            ));
        }
        let path = output.path.to_str().ok_or_else(|| {
            domain(
                "nix_profile_output_path",
                "nix.profile.runtime.expected_outputs",
                "Nix expected output path must be valid UTF-8",
            )
        })?;
        if !is_safe_absolute_path(path)
            || output.path.parent() != Some(runtime.store_path.as_path())
        {
            return Err(domain(
                "nix_profile_output_path",
                "nix.profile.runtime.expected_outputs",
                "Nix expected output must be one safe direct child of the admitted store",
            ));
        }
        if !paths.insert(output.path.clone()) {
            return Err(domain(
                "nix_profile_output_duplicate",
                "nix.profile.runtime.expected_outputs",
                "Nix expected output path is repeated",
            ));
        }
        previous = Some(&output.name);
    }
    Ok(())
}

fn validate_limits(limits: NixBuildLimits) -> PortResult<()> {
    if limits.max_required_products == 0
        || limits.max_required_products > MAXIMUM_REQUIRED_PRODUCTS
        || limits.max_products < limits.max_required_products
        || limits.max_products > MAXIMUM_PRODUCTS
        || limits.max_logs != MAXIMUM_LOGS
        || limits.max_gc_roots == 0
        || limits.max_gc_roots > MAXIMUM_GC_ROOTS
        || limits.max_request_bytes == 0
        || limits.max_result_bytes == 0
        || limits.max_stderr_bytes == 0
        || limits.max_log_bytes < limits.max_stderr_bytes
        || limits.max_executable_bytes == 0
        || limits.max_derivation_bytes == 0
        || limits.max_in_flight == 0
        || limits.stdin_max_bytes == 0
        || limits.timeout_milliseconds == 0
        || limits.poll_interval_milliseconds == 0
        || limits.poll_interval_milliseconds > limits.timeout_milliseconds
        || limits.teardown_timeout_milliseconds == 0
    {
        return Err(domain(
            "nix_profile_limit",
            "nix.profile.limits",
            "Nix limits are zero, inconsistent, or exceed the native contract",
        ));
    }
    Ok(())
}

fn required_operations() -> BTreeSet<Operation> {
    BTreeSet::from([
        Operation::Dispatch,
        Operation::Observe,
        Operation::Cancel,
        Operation::ReadLogs,
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
