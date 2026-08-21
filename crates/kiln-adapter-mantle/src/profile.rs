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

use crate::wire::{
    NATIVE_REPORT_SCHEMA, OBSERVATION_SCHEMA, RECEIPT_SCHEMA, REQUEST_SCHEMA, REQUIRED_NON_CLAIMS,
};

pub const PINNED_MANTLE_SOURCE: &str = "ssh://git@github.com/OnixResearch/mantle.git";
pub const PINNED_MANTLE_REVISION: &str = "0bbab77e23ab95caa4e4184b9945cfaa5c8040e2";
pub const PINNED_MANTLE_CONTRACT_LICENSE: &str = "MIT OR Apache-2.0";
pub const PINNED_MANTLE_CLI_LICENSE: &str = "AGPL-3.0-or-later";
pub const PINNED_MANTLE_PLATFORM: &str = "x86_64-linux";
pub const PINNED_MANTLE_COMMAND: &str = "build";
pub const PINNED_MANTLE_EXECUTABLE_PATH: &str =
    "/nix/store/s16cv8qj77zsp2wg03qlvxj577frb6jr-mantle-0.1.0/bin/mantle";
pub const PINNED_MANTLE_EXECUTABLE_BLAKE3: &str =
    "d8090e908e05f8fbd3c1154a77b95db9d20294a17a47856669199b11457c35c8";
pub const PINNED_BOUNDED_EXEC_SOURCE: &str =
    "https://git.onix.computer/z2CpqLFpdP36fZXYUK5ZNWxMibpCo.git";
pub const PINNED_BOUNDED_EXEC_REVISION: &str = "29dac88ecded94457572db3fdfaaaab95fa91525";
pub const PINNED_BOUNDED_EXEC_LICENSE: &str = "AGPL-3.0-or-later";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MantleAdapterLimits {
    pub max_required_products: usize,
    pub max_products: usize,
    pub max_logs: usize,
    pub max_metrics: usize,
    pub max_request_bytes: usize,
    pub max_result_bytes: usize,
    pub max_stderr_bytes: usize,
    pub max_log_bytes: usize,
    pub max_executable_bytes: u64,
    pub max_plan_bytes: u64,
    pub max_store_identity_bytes: u64,
    pub max_in_flight: usize,
    pub stdin_max_bytes: usize,
    pub timeout_milliseconds: u64,
    pub poll_interval_milliseconds: u64,
    pub teardown_timeout_milliseconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MantleRuntimeBinding {
    pub executable_path: PathBuf,
    pub executable_identity: ContentId,
    pub plan_path: PathBuf,
    pub plan_identity: ContentId,
    pub policy_identity: ContentId,
    pub store_path: PathBuf,
    pub state_path: PathBuf,
    pub store_identity_path: PathBuf,
    pub builder_identity: ContentId,
    pub worker_identity: ContentId,
    pub store_identity: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MantleProfile {
    pub source: String,
    pub revision: String,
    pub contract_license: String,
    pub cli_license: String,
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
    pub runtime: MantleRuntimeBinding,
    pub limits: MantleAdapterLimits,
    pub required_non_claims: Vec<String>,
}

impl MantleProfile {
    pub fn pinned(
        profile_id: ProviderProfileId,
        runtime: MantleRuntimeBinding,
        limits: MantleAdapterLimits,
    ) -> PortResult<Self> {
        Ok(Self {
            source: PINNED_MANTLE_SOURCE.to_string(),
            revision: PINNED_MANTLE_REVISION.to_string(),
            contract_license: PINNED_MANTLE_CONTRACT_LICENSE.to_string(),
            cli_license: PINNED_MANTLE_CLI_LICENSE.to_string(),
            bounded_exec_source: PINNED_BOUNDED_EXEC_SOURCE.to_string(),
            bounded_exec_revision: PINNED_BOUNDED_EXEC_REVISION.to_string(),
            bounded_exec_license: PINNED_BOUNDED_EXEC_LICENSE.to_string(),
            request_schema: REQUEST_SCHEMA.to_string(),
            observation_schema: OBSERVATION_SCHEMA.to_string(),
            receipt_schema: RECEIPT_SCHEMA.to_string(),
            native_report_schema: NATIVE_REPORT_SCHEMA.to_string(),
            command: PINNED_MANTLE_COMMAND.to_string(),
            platform: PINNED_MANTLE_PLATFORM.to_string(),
            provider: CapabilityProfile {
                contract_version: CONTRACT_VERSION_V1,
                profile_id,
                role: ProviderRole::Build,
                namespace: ExternalRef::new("mantle", "build-provider", PINNED_MANTLE_REVISION)?,
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

// r[impl kiln.mantle.profile] r[impl kiln.mantle.nonclaims]
pub fn admit_profile(profile: &MantleProfile) -> PortResult<()> {
    if profile.source != PINNED_MANTLE_SOURCE
        || profile.revision != PINNED_MANTLE_REVISION
        || profile.contract_license != PINNED_MANTLE_CONTRACT_LICENSE
        || profile.cli_license != PINNED_MANTLE_CLI_LICENSE
        || profile.bounded_exec_source != PINNED_BOUNDED_EXEC_SOURCE
        || profile.bounded_exec_revision != PINNED_BOUNDED_EXEC_REVISION
        || profile.bounded_exec_license != PINNED_BOUNDED_EXEC_LICENSE
        || profile.request_schema != REQUEST_SCHEMA
        || profile.observation_schema != OBSERVATION_SCHEMA
        || profile.receipt_schema != RECEIPT_SCHEMA
        || profile.native_report_schema != NATIVE_REPORT_SCHEMA
        || profile.command != PINNED_MANTLE_COMMAND
        || profile.platform != PINNED_MANTLE_PLATFORM
    {
        return Err(domain(
            "mantle_profile_cohort",
            "mantle.profile",
            "Mantle profile differs from the pinned contract cohort",
        ));
    }
    if profile.runtime.executable_path != PathBuf::from(PINNED_MANTLE_EXECUTABLE_PATH)
        || profile.runtime.executable_identity.as_str() != PINNED_MANTLE_EXECUTABLE_BLAKE3
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
            "mantle_profile_binding",
            "mantle.profile",
            "Mantle profile identities, capabilities, or non-claims differ from the cohort",
        ));
    }
    validate_paths(profile)?;
    validate_limits(profile.limits)?;
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

fn validate_paths(profile: &MantleProfile) -> PortResult<()> {
    let runtime = &profile.runtime;
    if !runtime.executable_path.is_absolute()
        || !runtime.plan_path.is_absolute()
        || !runtime.store_path.is_absolute()
        || !runtime.state_path.is_absolute()
        || !runtime.store_identity_path.is_absolute()
    {
        return Err(domain(
            "mantle_profile_path",
            "mantle.profile.runtime",
            "Mantle executable, plan, store, and state paths must be absolute",
        ));
    }
    Ok(())
}

fn validate_limits(limits: MantleAdapterLimits) -> PortResult<()> {
    if limits.max_required_products == 0
        || limits.max_required_products > crate::wire::MAXIMUM_REQUIRED_PRODUCTS
        || limits.max_products < limits.max_required_products
        || limits.max_products > crate::wire::MAXIMUM_PRODUCTS
        || limits.max_logs == 0
        || limits.max_logs > crate::wire::MAXIMUM_LOGS
        || limits.max_metrics == 0
        || limits.max_metrics > crate::wire::MAXIMUM_METRICS
        || limits.max_request_bytes == 0
        || limits.max_result_bytes == 0
        || limits.max_stderr_bytes == 0
        || limits.max_log_bytes == 0
        || limits.max_executable_bytes == 0
        || limits.max_plan_bytes == 0
        || limits.max_store_identity_bytes == 0
        || limits.max_in_flight == 0
        || limits.stdin_max_bytes == 0
        || limits.timeout_milliseconds == 0
        || limits.poll_interval_milliseconds == 0
        || limits.poll_interval_milliseconds > limits.timeout_milliseconds
        || limits.teardown_timeout_milliseconds == 0
    {
        return Err(domain(
            "mantle_profile_limit",
            "mantle.profile.limits",
            "Mantle limits are zero, inconsistent, or exceed the contract",
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
