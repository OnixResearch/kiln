use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use kiln_core::gating::CandidateSet;
use kiln_core::provider::{CapabilityProfile, Operation, ProviderRole};
use kiln_core::types::{CONTRACT_VERSION_V1, ContentId};
use kiln_provider_api::PortResult;
use serde::{Deserialize, Serialize};

use crate::domain;

pub const PINNED_SOURCE_REVISION: &str = "97c1216fe13000d9dcbb97f70ddd5423180eea2c";
pub const NATIVE_REPORT_SCHEMA: u16 = 1;
// This is a linked dependency's license, not a claim about the external executable.
pub const BOUNDED_EXEC_LICENSE: &str = "AGPL-3.0-or-later";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MainBinding {
    pub channel: String,
    pub state: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyBinding {
    pub path: String,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigurationValues {
    pub policy_path: String,
    pub protected: Vec<String>,
    #[serde(default)]
    pub documentation_paths: Vec<String>,
    pub max_change_chars: usize,
    pub timeout_seconds: f64,
    pub check_timeout_seconds: f64,
    pub max_review_requests: usize,
    pub review_timeout_seconds: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigurationEvidence {
    pub source: String,
    pub path: Option<String>,
    pub sha256: Option<String>,
    pub values: ConfigurationValues,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeBinding {
    pub main: MainBinding,
    pub inbox: MainBinding,
    pub configuration: ConfigurationEvidence,
    pub policy: Option<PolicyBinding>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchgateLimits {
    pub max_changes: usize,
    pub max_edges: usize,
    pub max_request_bytes: usize,
    pub max_report_bytes: usize,
    pub max_stderr_bytes: usize,
    pub max_executable_bytes: u64,
    pub timeout_ms: u64,
    pub poll_interval_ms: u64,
    pub teardown_timeout_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchgateProfile {
    pub source_revision: String,
    pub report_schema: u16,
    pub program: PathBuf,
    pub executable_identity: ContentId,
    pub repository_root: PathBuf,
    pub environment: BTreeMap<String, String>,
    pub provider: CapabilityProfile,
    pub native: NativeBinding,
    pub limits: PatchgateLimits,
}

pub fn candidate_root(candidate: &CandidateSet, max_bytes: usize) -> PortResult<ContentId> {
    canonical_identity(candidate, max_bytes)
}

pub fn configuration_identity(
    configuration: &ConfigurationEvidence,
    max_bytes: usize,
) -> PortResult<ContentId> {
    canonical_identity(configuration, max_bytes)
}

pub fn policy_identity(profile: &PatchgateProfile) -> PortResult<ContentId> {
    canonical_identity(&profile.native.policy, profile.limits.max_request_bytes)
}

/// Binds every declared applicability and execution fact, including explicit environment.
/// This declaration does not attest executable source provenance or its runtime closure.
pub fn native_binding_identity(profile: &PatchgateProfile) -> PortResult<ContentId> {
    canonical_identity(profile, profile.limits.max_request_bytes)
}

pub(crate) fn canonical_identity<T: Serialize>(value: &T, bound: usize) -> PortResult<ContentId> {
    kiln_codec::encode_and_identify(value, bound)
        .map(|(_, identity)| identity)
        .map_err(|_| {
            domain(
                "patchgate_canonical_bound",
                "Canonical evidence exceeds its bound or cannot be encoded",
            )
        })
}

pub(crate) fn native_hash(value: &str) -> bool {
    value.len() == 53
        && value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || (b'2'..=b'7').contains(&byte))
}

pub(crate) fn sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(crate) fn validate_main(binding: &MainBinding) -> PortResult<()> {
    if binding.channel.is_empty()
        || binding.channel.chars().any(char::is_control)
        || binding
            .state
            .as_deref()
            .is_some_and(|state| !native_hash(state))
    {
        return Err(domain(
            "patchgate_channel_binding",
            "Native channel needs a name and a full state hash or null",
        ));
    }
    Ok(())
}

pub(crate) fn validate_profile(profile: &PatchgateProfile) -> PortResult<()> {
    let limits = profile.limits;
    if profile.source_revision != PINNED_SOURCE_REVISION
        || profile.report_schema != NATIVE_REPORT_SCHEMA
    {
        return Err(domain(
            "patchgate_cohort",
            "Unsupported Patchgate source revision or native report schema",
        ));
    }
    if !profile.program.is_absolute() || !profile.repository_root.is_absolute() {
        return Err(domain(
            "patchgate_path",
            "Program and repository root must be absolute paths",
        ));
    }
    if limits.max_changes == 0
        || limits.max_request_bytes == 0
        || limits.max_report_bytes == 0
        || limits.max_stderr_bytes == 0
        || limits.max_executable_bytes == 0
        || limits.timeout_ms == 0
        || limits.poll_interval_ms == 0
        || limits.poll_interval_ms > limits.timeout_ms
        || limits.teardown_timeout_ms == 0
    {
        return Err(domain(
            "patchgate_limits",
            "Patchgate execution bounds must be positive",
        ));
    }
    let provider = &profile.provider;
    provider
        .namespace
        .validate()
        .map_err(kiln_provider_api::PortFailure::Domain)?;
    if provider.contract_version != CONTRACT_VERSION_V1
        || provider.role != ProviderRole::Collaboration
        || provider.operations != BTreeSet::from([Operation::Observe, Operation::Reconcile])
        || !provider.outcomes.is_empty()
        || provider.limits.max_request_bytes != limits.max_request_bytes
        || provider.limits.max_result_bytes != limits.max_report_bytes
        || provider.limits.max_in_flight != 1
    {
        return Err(domain(
            "patchgate_capabilities",
            "Patchgate exposes only bounded isolated observation and reconciliation, not domain outcomes",
        ));
    }
    validate_main(&profile.native.main)?;
    validate_main(&profile.native.inbox)?;
    if profile.native.main.channel == profile.native.inbox.channel {
        return Err(domain(
            "patchgate_channels",
            "Main and inbox must be distinct native channels",
        ));
    }
    let configuration = &profile.native.configuration;
    let values = &configuration.values;
    if values.policy_path.is_empty()
        || values.max_change_chars == 0
        || values.max_review_requests == 0
        || [
            values.timeout_seconds,
            values.check_timeout_seconds,
            values.review_timeout_seconds,
        ]
        .iter()
        .any(|seconds| !seconds.is_finite() || *seconds <= 0.0)
        || !matches!(configuration.source.as_str(), "environment" | "main")
        || configuration
            .sha256
            .as_deref()
            .is_some_and(|digest| !sha256(digest))
    {
        return Err(domain(
            "patchgate_configuration",
            "Invalid trusted native configuration evidence",
        ));
    }
    let source_matches = match configuration.source.as_str() {
        "environment" => configuration.path.is_none() && configuration.sha256.is_none(),
        "main" => {
            configuration.path.as_deref() == Some("patchgate.json")
                && configuration.sha256.is_some()
        }
        _ => false,
    };
    if !source_matches {
        return Err(domain(
            "patchgate_configuration_source",
            "Native configuration source, path and SHA-256 evidence are inconsistent",
        ));
    }
    if let Some(policy) = &profile.native.policy {
        if policy.path != values.policy_path || !sha256(&policy.sha256) {
            return Err(domain(
                "patchgate_policy",
                "Native policy path and SHA-256 must match the trusted configuration",
            ));
        }
    }
    for (name, value) in &profile.environment {
        if name.is_empty() || name.contains(['=', '\0']) || value.contains('\0') {
            return Err(domain(
                "patchgate_environment",
                "Explicit environment contains an invalid name or value",
            ));
        }
    }
    // Enforce the canonical profile bound before any process can start.
    native_binding_identity(profile)?;
    Ok(())
}
