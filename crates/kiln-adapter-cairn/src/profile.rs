use std::collections::BTreeSet;
use std::path::PathBuf;

use cairn_gate_contract::{
    GATE_RECEIPT_SCHEMA, MACHINE_DIAGNOSTIC_SCHEMA, MACHINE_SCHEMA, MAXIMUM_EVIDENCE_IDENTITIES,
    MAXIMUM_FINDINGS, POLICY_SCHEMA, REQUEST_SCHEMA, REQUIRED_NON_CLAIMS, RESULT_SCHEMA,
};
use kiln_core::domain::Outcome;
use kiln_core::gating::LifecycleGateStage;
use kiln_core::provider::{
    CapabilityProfile, Operation, OperationRequest, ProviderLimits, ProviderRole, admit_operation,
};
use kiln_core::types::{
    CONTRACT_VERSION_V1, ContentId, Diagnostic, ExternalRef, ProviderProfileId,
};
use kiln_provider_api::{PortFailure, PortResult, role_matches_port};
use serde::{Deserialize, Serialize};

pub const PINNED_CAIRN_SOURCE: &str = "ssh://git@github.com/OnixResearch/cairn.git";
pub const PINNED_CAIRN_REVISION: &str = "15f00875562025e7ea7e0d1f4af24d1a2e2ac06f";
pub const PINNED_CAIRN_CONTRACT_LICENSE: &str = "MIT OR Apache-2.0";
pub const PINNED_CAIRN_CLI_LICENSE: &str = "AGPL-3.0-or-later";
pub const PINNED_CAIRN_EXECUTABLE_PATH: &str =
    "/nix/store/bvz6kl88qkfi15n8ml1xr7p22syykggd-cairn-0.1.0/bin/cairn";
pub const PINNED_CAIRN_EXECUTABLE_BLAKE3: &str =
    "9ade7248f9f48a0a83fc21365cc45550b09a048621ac925292ed974dbc4951d0";
pub const PINNED_CAIRN_POLICY_BLAKE3: &str =
    "be930a118f405d176ea56de5c72ba05b8061ab3a4f996113cde505710cce5c74";
pub const PINNED_BOUNDED_EXEC_SOURCE: &str =
    "https://git.onix.computer/z2CpqLFpdP36fZXYUK5ZNWxMibpCo.git";
pub const PINNED_BOUNDED_EXEC_REVISION: &str = "29dac88ecded94457572db3fdfaaaab95fa91525";
pub const PINNED_BOUNDED_EXEC_LICENSE: &str = "AGPL-3.0-or-later";
pub const PINNED_CAIRN_PLATFORM: &str = "x86_64-linux";
pub const PINNED_CAIRN_COMMAND: &str = "gate";
pub const PINNED_CAIRN_POLICY_NAME: &str = "kiln-default";
pub const PINNED_MACHINE_SUCCESS_STATUS: i32 = 0;
pub const PINNED_MACHINE_INVALID_ARGUMENTS_STATUS: i32 = 2;
pub const PINNED_MACHINE_NOT_FOUND_STATUS: i32 = 3;
pub const PINNED_MACHINE_POLICY_ERROR_STATUS: i32 = 4;
pub const PINNED_MACHINE_BLOCKED_STATUS: i32 = 5;
pub const PINNED_MACHINE_INTERNAL_ERROR_STATUS: i32 = 70;

pub const MACHINE_NON_CLAIMS: [&str; 5] = [
    "a valid machine envelope proves only that Cairn reached the controlled output boundary and serialized one typed outcome",
    "a machine envelope does not prove nested evidence truth or command correctness",
    "a machine envelope does not prove mutation safety, process reliability, or host integrity",
    "a machine envelope does not prove lifecycle readiness or release eligibility",
    "no envelope is guaranteed after host termination, process kill, memory exhaustion, pre-boundary panic, broken stdout, or failure before Cairn starts",
];

pub const NATIVE_GATE_NON_CLAIMS: [&str; 4] = [
    "workflow state proves only the selected policy graph and observed structural artifact facts",
    "workflow instructions do not prove artifact truth, review quality, or implementation correctness",
    "workflow state does not satisfy proposal, design, tasks, sync, archive, evidence, or release gates",
    "workflow commands are read-only and do not create, edit, delete, or approve lifecycle artifacts",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CairnRerunPolicy {
    Never,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CairnMachineExitStatuses {
    pub success: i32,
    pub invalid_arguments: i32,
    pub not_found: i32,
    pub policy_error: i32,
    pub blocked: i32,
    pub internal_error: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CairnAdapterLimits {
    pub max_changes: usize,
    pub max_edges: usize,
    pub max_evidence_identities: usize,
    pub max_findings: usize,
    pub max_request_bytes: usize,
    pub max_result_bytes: usize,
    pub max_stderr_bytes: usize,
    pub max_diagnostic_bytes: usize,
    pub max_executable_bytes: u64,
    pub max_in_flight: usize,
    pub stdin_max_bytes: usize,
    pub timeout_milliseconds: u64,
    pub poll_interval_milliseconds: u64,
    pub teardown_timeout_milliseconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CairnRuntimeBinding {
    pub executable_path: PathBuf,
    pub executable_identity: ContentId,
    pub repository_root: PathBuf,
    pub policy_path: PathBuf,
    pub policy_identity: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CairnProfile {
    pub source: String,
    pub revision: String,
    pub contract_license: String,
    pub cli_license: String,
    pub bounded_exec_source: String,
    pub bounded_exec_revision: String,
    pub bounded_exec_license: String,
    pub request_schema: String,
    pub result_schema: String,
    pub machine_schema: String,
    pub diagnostic_schema: String,
    pub policy_schema: String,
    pub gate_receipt_schema: String,
    pub command: String,
    pub policy_name: String,
    pub platform: String,
    pub exit_statuses: CairnMachineExitStatuses,
    pub supported_stages: BTreeSet<LifecycleGateStage>,
    pub rerun_policy: CairnRerunPolicy,
    pub provider: CapabilityProfile,
    pub runtime: CairnRuntimeBinding,
    pub limits: CairnAdapterLimits,
    pub machine_non_claims: Vec<String>,
    pub native_gate_non_claims: Vec<String>,
    pub result_non_claims: Vec<String>,
}

impl CairnProfile {
    pub fn pinned(
        profile_id: ProviderProfileId,
        executable_path: PathBuf,
        repository_root: PathBuf,
        policy_path: PathBuf,
        limits: CairnAdapterLimits,
    ) -> PortResult<Self> {
        Ok(Self {
            source: PINNED_CAIRN_SOURCE.to_string(),
            revision: PINNED_CAIRN_REVISION.to_string(),
            contract_license: PINNED_CAIRN_CONTRACT_LICENSE.to_string(),
            cli_license: PINNED_CAIRN_CLI_LICENSE.to_string(),
            bounded_exec_source: PINNED_BOUNDED_EXEC_SOURCE.to_string(),
            bounded_exec_revision: PINNED_BOUNDED_EXEC_REVISION.to_string(),
            bounded_exec_license: PINNED_BOUNDED_EXEC_LICENSE.to_string(),
            request_schema: REQUEST_SCHEMA.to_string(),
            result_schema: RESULT_SCHEMA.to_string(),
            machine_schema: MACHINE_SCHEMA.to_string(),
            diagnostic_schema: MACHINE_DIAGNOSTIC_SCHEMA.to_string(),
            policy_schema: POLICY_SCHEMA.to_string(),
            gate_receipt_schema: GATE_RECEIPT_SCHEMA.to_string(),
            command: PINNED_CAIRN_COMMAND.to_string(),
            policy_name: PINNED_CAIRN_POLICY_NAME.to_string(),
            platform: PINNED_CAIRN_PLATFORM.to_string(),
            exit_statuses: pinned_exit_statuses(),
            supported_stages: BTreeSet::from([
                LifecycleGateStage::Proposal,
                LifecycleGateStage::Design,
                LifecycleGateStage::Tasks,
            ]),
            rerun_policy: CairnRerunPolicy::Never,
            provider: CapabilityProfile {
                contract_version: CONTRACT_VERSION_V1,
                profile_id,
                role: ProviderRole::Gate,
                namespace: ExternalRef::new("cairn", "gate-provider", PINNED_CAIRN_REVISION)?,
                operations: required_operations(),
                outcomes: required_outcomes(),
                limits: ProviderLimits {
                    max_request_bytes: limits.max_request_bytes,
                    max_result_bytes: limits.max_result_bytes,
                    max_in_flight: limits.max_in_flight,
                },
            },
            runtime: CairnRuntimeBinding {
                executable_path,
                executable_identity: content_id(PINNED_CAIRN_EXECUTABLE_BLAKE3)?,
                repository_root,
                policy_path,
                policy_identity: content_id(PINNED_CAIRN_POLICY_BLAKE3)?,
            },
            limits,
            machine_non_claims: MACHINE_NON_CLAIMS
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            native_gate_non_claims: NATIVE_GATE_NON_CLAIMS
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            result_non_claims: REQUIRED_NON_CLAIMS
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
        })
    }
}

// r[impl kiln.cairn_gate.profile] r[impl kiln.cairn_gate.nonclaims]
pub fn admit_profile(profile: &CairnProfile) -> PortResult<()> {
    if profile.source != PINNED_CAIRN_SOURCE
        || profile.revision != PINNED_CAIRN_REVISION
        || profile.contract_license != PINNED_CAIRN_CONTRACT_LICENSE
        || profile.cli_license != PINNED_CAIRN_CLI_LICENSE
        || profile.bounded_exec_source != PINNED_BOUNDED_EXEC_SOURCE
        || profile.bounded_exec_revision != PINNED_BOUNDED_EXEC_REVISION
        || profile.bounded_exec_license != PINNED_BOUNDED_EXEC_LICENSE
        || profile.request_schema != REQUEST_SCHEMA
        || profile.result_schema != RESULT_SCHEMA
        || profile.machine_schema != MACHINE_SCHEMA
        || profile.diagnostic_schema != MACHINE_DIAGNOSTIC_SCHEMA
        || profile.policy_schema != POLICY_SCHEMA
        || profile.gate_receipt_schema != GATE_RECEIPT_SCHEMA
        || profile.command != PINNED_CAIRN_COMMAND
        || profile.policy_name != PINNED_CAIRN_POLICY_NAME
        || profile.platform != PINNED_CAIRN_PLATFORM
        || profile.exit_statuses != pinned_exit_statuses()
        || profile.rerun_policy != CairnRerunPolicy::Never
    {
        return Err(domain(
            "cairn_profile_cohort",
            "cairn.profile",
            "Cairn profile differs from the pinned contract cohort",
        ));
    }
    validate_paths(profile)?;
    validate_limits(profile.limits)?;
    if profile.runtime.executable_path != PathBuf::from(PINNED_CAIRN_EXECUTABLE_PATH)
        || profile.runtime.executable_identity.as_str() != PINNED_CAIRN_EXECUTABLE_BLAKE3
        || profile.runtime.policy_identity.as_str() != PINNED_CAIRN_POLICY_BLAKE3
        || profile.supported_stages
            != BTreeSet::from([
                LifecycleGateStage::Proposal,
                LifecycleGateStage::Design,
                LifecycleGateStage::Tasks,
            ])
        || profile.provider.operations != required_operations()
        || profile.provider.outcomes != required_outcomes()
        || profile.provider.limits
            != (ProviderLimits {
                max_request_bytes: profile.limits.max_request_bytes,
                max_result_bytes: profile.limits.max_result_bytes,
                max_in_flight: profile.limits.max_in_flight,
            })
        || profile.machine_non_claims
            != MACHINE_NON_CLAIMS
                .iter()
                .map(|value| (*value).to_string())
                .collect::<Vec<_>>()
        || profile.native_gate_non_claims
            != NATIVE_GATE_NON_CLAIMS
                .iter()
                .map(|value| (*value).to_string())
                .collect::<Vec<_>>()
        || profile.result_non_claims
            != REQUIRED_NON_CLAIMS
                .iter()
                .map(|value| (*value).to_string())
                .collect::<Vec<_>>()
    {
        return Err(domain(
            "cairn_profile_binding",
            "cairn.profile",
            "Cairn profile bindings or non-claims differ from the admitted cohort",
        ));
    }
    role_matches_port(&profile.provider, ProviderRole::Gate)?;
    for operation in &profile.provider.operations {
        admit_operation(
            &profile.provider,
            &OperationRequest {
                contract_version: CONTRACT_VERSION_V1,
                selected_profile: profile.provider.profile_id.clone(),
                role: ProviderRole::Gate,
                operation: *operation,
                request_bytes: 0,
                required_outcomes: profile.provider.outcomes.clone(),
            },
        )?;
    }
    Ok(())
}

fn validate_paths(profile: &CairnProfile) -> PortResult<()> {
    if !profile.runtime.executable_path.is_absolute()
        || !profile.runtime.repository_root.is_absolute()
        || !profile.runtime.policy_path.is_absolute()
    {
        return Err(domain(
            "cairn_profile_path",
            "cairn.profile.runtime",
            "Cairn executable, repository, and policy paths must be absolute",
        ));
    }
    Ok(())
}

fn validate_limits(limits: CairnAdapterLimits) -> PortResult<()> {
    if limits.max_changes == 0
        || limits.max_edges == 0
        || limits.max_evidence_identities == 0
        || limits.max_evidence_identities > MAXIMUM_EVIDENCE_IDENTITIES
        || limits.max_findings == 0
        || limits.max_findings > MAXIMUM_FINDINGS
        || limits.max_request_bytes == 0
        || limits.max_result_bytes == 0
        || limits.max_stderr_bytes == 0
        || limits.max_diagnostic_bytes == 0
        || limits.max_diagnostic_bytes > cairn_gate_contract::MAXIMUM_TEXT_BYTES
        || limits.max_executable_bytes == 0
        || limits.max_in_flight == 0
        || limits.stdin_max_bytes == 0
        || limits.timeout_milliseconds == 0
        || limits.poll_interval_milliseconds == 0
        || limits.poll_interval_milliseconds > limits.timeout_milliseconds
        || limits.teardown_timeout_milliseconds == 0
    {
        return Err(domain(
            "cairn_profile_limit",
            "cairn.profile.limits",
            "Cairn profile limits are zero, inconsistent, or exceed the contract",
        ));
    }
    Ok(())
}

const fn pinned_exit_statuses() -> CairnMachineExitStatuses {
    CairnMachineExitStatuses {
        success: PINNED_MACHINE_SUCCESS_STATUS,
        invalid_arguments: PINNED_MACHINE_INVALID_ARGUMENTS_STATUS,
        not_found: PINNED_MACHINE_NOT_FOUND_STATUS,
        policy_error: PINNED_MACHINE_POLICY_ERROR_STATUS,
        blocked: PINNED_MACHINE_BLOCKED_STATUS,
        internal_error: PINNED_MACHINE_INTERNAL_ERROR_STATUS,
    }
}

fn required_operations() -> BTreeSet<Operation> {
    BTreeSet::from([Operation::Dispatch, Operation::Reconcile])
}

fn required_outcomes() -> BTreeSet<Outcome> {
    BTreeSet::from([
        Outcome::Success,
        Outcome::DomainFailure,
        Outcome::Denied,
        Outcome::Cancelled,
        Outcome::TimedOut,
        Outcome::ProviderError,
        Outcome::Unknown,
    ])
}

fn content_id(value: &str) -> PortResult<ContentId> {
    ContentId::from_blake3_hex(value).map_err(PortFailure::Domain)
}

fn domain(code: &str, field: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, field, message))
}
