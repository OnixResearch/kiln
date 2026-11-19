//! Public API for enforcing Nickel-authored contract policy with UCAN capability checks.
//!
//! Library consumers either parse their own Nickel-exported JSON policy or start
//! from the packaged [`DEFAULT_POLICY_JSON`] example policy, build an
//! [`EnforcementRequest`], and inspect the returned [`EnforcementReceipt`]:
//!
//! ```
//! use basalt::{CapabilityGrant, EnforcementRequest, enforce, parse_policy_json};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let policy_json = r#"{
//!   "schema_version": "ucan-nickel-contracts.policy.v1",
//!   "contracts": {
//!     "document-write": {
//!       "id": "document-write",
//!       "description": "Write bounded document resources.",
//!       "resource_prefixes": ["did:key:z6Mkresource/docs/"],
//!       "abilities": ["document/write"]
//!     }
//!   }
//! }"#;
//!
//! let policy = parse_policy_json(policy_json)?;
//! let request = EnforcementRequest::new(
//!     "document-write",
//!     "did:key:z6Mkresource/docs/report",
//!     "document/write",
//! )
//! .with_capability(CapabilityGrant::new(
//!     "did:key:z6Mkresource/docs/report",
//!     "document/write",
//! ));
//!
//! let receipt = enforce(&policy, &request)?;
//! assert!(receipt.is_allowed());
//! assert_eq!(receipt.reason(), "allowed");
//! # Ok(())
//! # }
//! ```
//!
//! The packaged example policy is also available for smoke tests:
//!
//! ```
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let policy = basalt::default_policy()?;
//! assert!(policy.contract("document-write").is_some());
//! # Ok(())
//! # }
//! ```

#![allow(clippy::multiple_crate_versions)]

mod ast_grep_audit_identity;
mod authority_identity;
mod license_consumption_identity;
mod secretspec_issuer_identity;
mod ucan_semantics_identity;

pub mod ast_grep_audit;
pub mod authority_core;
#[cfg(feature = "cli")]
pub mod cli_fs;
#[cfg(feature = "durable-authority")]
pub mod durable_authority;
pub mod kiln;
pub mod license_consumption;
pub mod mantle;
pub mod nostr_identity;
#[cfg(feature = "oidc-exchange")]
pub mod oidc_exchange;
pub mod peer_auth;
pub mod policy_cache;
pub mod policy_identity;
pub mod policy_projection;
pub mod runtime_effect;
pub mod secretspec_issuer;
pub mod ucan_enforcement;
pub mod ucan_profile;
pub mod ucan_semantics;
pub mod ucan_v1_profile;
pub mod world_branch_authority;

pub use policy_cache::{
    CachePolicy, PolicyCacheKey, PolicyError, PolicyEvaluationCache, PromotionDecision,
};
#[cfg(feature = "durable-authority")]
pub use revocation_view_core;
pub use ucan;

use serde::Deserialize;
use serde::Serialize;
use snafu::Snafu;
use std::collections::BTreeMap;

/// Policy schema version supported by this crate.
///
/// The pre-Basalt schema identifier is intentionally stable because the policy JSON
/// shape did not change during the package rename.
pub const SUPPORTED_SCHEMA_VERSION: &str = "ucan-nickel-contracts.policy.v1";

/// Packaged Nickel-exported example policy JSON.
///
/// This is generated from `contracts/policy.ncl` and is included in the crate
/// package so downstream applications can smoke-test integration without
/// carrying a Nickel toolchain. Production applications may provide their own
/// reviewed policy JSON to [`parse_policy_json`].
pub const DEFAULT_POLICY_JSON: &str = include_str!("../contracts/generated/policy.json");

/// Supported contract backend ids for UCAN contract envelopes.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ContractBackendKind {
    /// Nickel-authored static policy/config backend.
    Nickel,
    /// Steel Scheme dynamic predicate/callable backend.
    Steel,
}

impl ContractBackendKind {
    /// Return the stable backend id string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Nickel => "nickel",
            Self::Steel => "steel",
        }
    }
}

/// Backend policy metadata exported with the contract policy artifact.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ContractBackendPolicy {
    /// Stable backend id, for example `nickel` or `steel`.
    pub id: String,
    /// Human-readable backend description.
    #[serde(default)]
    pub description: String,
    /// Whether the backend may declare higher-order callable descriptors.
    #[serde(default)]
    pub supports_higher_order: bool,
}

impl ContractBackendPolicy {
    /// Return true when this metadata describes a supported backend id.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        supported_backend(self.id.as_str()).is_some()
    }
}

/// Backend-neutral contract envelope shared by Nickel and Steel contract adapters.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ContractEnvelope {
    /// Backend id such as `nickel` or `steel`.
    pub backend: String,
    /// Stable contract id.
    pub contract_id: String,
    /// Contract schema/version marker.
    pub contract_version: String,
    /// Hash of normalized source material, excluding display-only paths.
    pub normalized_source_hash: String,
    /// Stable input schema id or hash.
    pub input_schema: String,
    /// Stable output schema id or hash.
    pub output_schema: String,
    /// Receipt schema version emitted by this backend.
    pub receipt_schema_version: String,
}

impl ContractEnvelope {
    /// Construct a backend-neutral contract envelope.
    #[must_use]
    pub fn new(
        backend: impl Into<String>,
        contract_id: impl Into<String>,
        contract_version: impl Into<String>,
        normalized_source_hash: impl Into<String>,
        input_schema: impl Into<String>,
        output_schema: impl Into<String>,
        receipt_schema_version: impl Into<String>,
    ) -> Self {
        Self {
            backend: backend.into(),
            contract_id: contract_id.into(),
            contract_version: contract_version.into(),
            normalized_source_hash: normalized_source_hash.into(),
            input_schema: input_schema.into(),
            output_schema: output_schema.into(),
            receipt_schema_version: receipt_schema_version.into(),
        }
    }

    /// Return the supported backend kind when the envelope backend id is known.
    #[must_use]
    pub fn backend_kind(&self) -> Option<ContractBackendKind> {
        supported_backend(self.backend.as_str())
    }
}

/// Deterministic validation receipt for backend-neutral contract envelopes.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ContractEnvelopeReceipt {
    /// Whether the envelope is supported and sufficiently bound for evaluation.
    pub accepted: bool,
    /// Stable validation reason.
    pub reason: String,
}

impl ContractEnvelopeReceipt {
    /// Return true when the envelope may proceed to backend-specific evaluation.
    #[must_use]
    pub const fn is_accepted(&self) -> bool {
        self.accepted
    }
}

fn supported_backend(backend: &str) -> Option<ContractBackendKind> {
    match backend {
        "nickel" => Some(ContractBackendKind::Nickel),
        "steel" => Some(ContractBackendKind::Steel),
        _ => None,
    }
}

/// Validate shared envelope fields before any backend-specific evaluation is trusted.
#[must_use]
pub fn validate_contract_envelope(envelope: &ContractEnvelope) -> ContractEnvelopeReceipt {
    if supported_backend(envelope.backend.as_str()).is_none() {
        return ContractEnvelopeReceipt {
            accepted: false,
            reason: format!("unsupported contract backend {:?}", envelope.backend),
        };
    }
    if envelope.normalized_source_hash.is_empty() {
        return ContractEnvelopeReceipt {
            accepted: false,
            reason: "missing normalized source hash".to_owned(),
        };
    }
    ContractEnvelopeReceipt {
        accepted: true,
        reason: "accepted".to_owned(),
    }
}

/// Declared effect class for dynamic contract callables.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContractEffectClass {
    /// Pure deterministic predicate with no host effects.
    Pure,
    /// Predicate may inspect declared UCAN capability context only.
    ReadsDeclaredCapabilities,
    /// Predicate requests a host effect that must be separately authorized.
    HostEffect,
}

/// Higher-order callable or predicate descriptor crossing the UCAN contract boundary.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct CallableContractDescriptor {
    /// Stable callable id within the contract source.
    pub callable_id: String,
    /// Expected positional arity.
    pub arity: usize,
    /// Argument contract ids or hashes in call order.
    pub argument_contracts: Vec<String>,
    /// Return contract id or hash.
    pub return_contract: String,
    /// UCAN capabilities required before invocation is trusted.
    #[serde(default)]
    pub required_capabilities: Vec<CapabilityGrant>,
    /// Declared determinism/effect class.
    pub effect_class: ContractEffectClass,
    /// Hash of normalized callable source/content.
    pub content_hash: String,
    /// Redaction class for evidence emitted by the callable.
    pub redaction_class: String,
}

/// Bounded Steel Scheme evaluation request DTO.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct SteelEvaluationRequest {
    /// Steel contract envelope selected for evaluation.
    pub envelope: ContractEnvelope,
    /// JSON DTO input supplied to Steel.
    pub input: serde_json::Value,
    /// Maximum serialized input bytes accepted by Rust before evaluation.
    pub max_input_bytes: usize,
    /// Optional higher-order callable descriptor selected for invocation.
    #[serde(default)]
    pub callable: Option<CallableContractDescriptor>,
    /// Host capability requested by Steel; any value is denied by this crate boundary.
    #[serde(default)]
    pub requested_host_capability: Option<String>,
}

/// Steel backend receipt bound to request, authority evidence, and deterministic hash material.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct SteelEvaluationReceipt {
    /// Backend id that produced this receipt.
    pub backend: String,
    /// Contract id evaluated by the backend.
    pub contract_id: String,
    /// Normalized contract source hash.
    pub normalized_source_hash: String,
    /// Hash of the request DTO.
    pub request_hash: String,
    /// Whether Steel returned an allow decision.
    pub allowed: bool,
    /// Stable decision reason.
    pub reason: String,
    /// Structured caveat evidence with deterministic key ordering.
    #[serde(default)]
    pub caveat_evidence: BTreeMap<String, String>,
    /// Declared effect class for the evaluated predicate/callable.
    pub effect_class: ContractEffectClass,
    /// Capabilities required by the callable or receipt.
    #[serde(default)]
    pub required_capabilities: Vec<CapabilityGrant>,
    /// Capabilities Rust verified before trusting an allow decision.
    #[serde(default)]
    pub checked_capabilities: Vec<CapabilityGrant>,
    /// Optional BLAKE3 hash of this receipt with this field omitted.
    #[serde(default)]
    pub receipt_hash: Option<String>,
}

/// Validate a higher-order callable descriptor before invocation.
#[must_use]
pub fn validate_callable_descriptor(
    descriptor: &CallableContractDescriptor,
) -> ContractEnvelopeReceipt {
    if descriptor.callable_id.is_empty() {
        return ContractEnvelopeReceipt {
            accepted: false,
            reason: "missing callable id".to_owned(),
        };
    }
    if descriptor.arity != descriptor.argument_contracts.len() {
        return ContractEnvelopeReceipt {
            accepted: false,
            reason: "callable arity does not match argument contracts".to_owned(),
        };
    }
    if descriptor.content_hash.is_empty() {
        return ContractEnvelopeReceipt {
            accepted: false,
            reason: "missing callable content hash".to_owned(),
        };
    }
    if descriptor.effect_class == ContractEffectClass::HostEffect
        && descriptor.required_capabilities.is_empty()
    {
        return ContractEnvelopeReceipt {
            accepted: false,
            reason: "effectful callable lacks required UCAN capabilities".to_owned(),
        };
    }
    ContractEnvelopeReceipt {
        accepted: true,
        reason: "accepted".to_owned(),
    }
}

/// Validate a Steel evaluation request without granting any ambient host authority.
#[must_use]
pub fn validate_steel_evaluation_request(
    request: &SteelEvaluationRequest,
) -> ContractEnvelopeReceipt {
    let envelope_receipt = validate_contract_envelope(&request.envelope);
    if !envelope_receipt.accepted {
        return envelope_receipt;
    }
    if request.envelope.backend_kind() != Some(ContractBackendKind::Steel) {
        return ContractEnvelopeReceipt {
            accepted: false,
            reason: "Steel evaluation requires steel backend envelope".to_owned(),
        };
    }
    if request.requested_host_capability.is_some() {
        return ContractEnvelopeReceipt {
            accepted: false,
            reason: "Steel host capability requests are denied by default".to_owned(),
        };
    }
    let input_bytes = match serde_json::to_vec(&request.input) {
        Ok(bytes) => bytes.len(),
        Err(_) => {
            return ContractEnvelopeReceipt {
                accepted: false,
                reason: "Steel input DTO is not serializable".to_owned(),
            };
        }
    };
    if input_bytes > request.max_input_bytes {
        return ContractEnvelopeReceipt {
            accepted: false,
            reason: "Steel input DTO exceeds byte limit".to_owned(),
        };
    }
    if let Some(callable) = &request.callable {
        let callable_receipt = validate_callable_descriptor(callable);
        if !callable_receipt.accepted {
            return callable_receipt;
        }
    }
    ContractEnvelopeReceipt {
        accepted: true,
        reason: "accepted".to_owned(),
    }
}

/// Compute a stable BLAKE3 hash for a Steel request DTO.
///
/// # Panics
///
/// Panics only if `serde_json` fails to serialize the crate-owned request DTO shape.
#[must_use]
pub fn steel_request_hash(request: &SteelEvaluationRequest) -> String {
    let bytes = serde_json::to_vec(request).expect("Steel request DTO serialization is infallible");
    format!("blake3:{}", blake3::hash(bytes.as_slice()).to_hex())
}

/// Compute a stable BLAKE3 hash for a Steel receipt with `receipt_hash` omitted.
///
/// # Panics
///
/// Panics only if `serde_json` fails to serialize the crate-owned receipt DTO shape.
#[must_use]
pub fn steel_receipt_hash(receipt: &SteelEvaluationReceipt) -> String {
    let mut hash_material = receipt.clone();
    hash_material.receipt_hash = None;
    let bytes =
        serde_json::to_vec(&hash_material).expect("Steel receipt DTO serialization is infallible");
    format!("blake3:{}", blake3::hash(bytes.as_slice()).to_hex())
}

/// Validate authority evidence and optional hash binding for a Steel receipt.
#[must_use]
pub fn validate_steel_evaluation_receipt(
    receipt: &SteelEvaluationReceipt,
) -> ContractEnvelopeReceipt {
    if receipt.backend != ContractBackendKind::Steel.as_str() {
        return ContractEnvelopeReceipt {
            accepted: false,
            reason: "Steel receipt uses unsupported backend id".to_owned(),
        };
    }
    if receipt.allowed {
        let all_required_checked = receipt.required_capabilities.iter().all(|required| {
            receipt
                .checked_capabilities
                .iter()
                .any(|checked| checked == required)
        });
        if !all_required_checked {
            return ContractEnvelopeReceipt {
                accepted: false,
                reason: "Steel allow receipt lacks required UCAN authority evidence".to_owned(),
            };
        }
    }
    if let Some(expected_hash) = &receipt.receipt_hash {
        let actual_hash = steel_receipt_hash(receipt);
        if expected_hash != &actual_hash {
            return ContractEnvelopeReceipt {
                accepted: false,
                reason: "Steel receipt hash mismatch".to_owned(),
            };
        }
    }
    ContractEnvelopeReceipt {
        accepted: true,
        reason: "accepted".to_owned(),
    }
}

/// Nickel-exported policy document consumed by [`enforce`].
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Policy {
    /// Schema version marker checked by [`parse_policy_json`].
    pub schema_version: String,
    /// Policies keyed by contract id.
    pub contracts: BTreeMap<String, ContractPolicy>,
    /// Contract backend metadata keyed by backend id.
    #[serde(default)]
    pub backends: BTreeMap<String, ContractBackendPolicy>,
}

impl Policy {
    /// Return a contract policy by id.
    #[must_use]
    pub fn contract(&self, id: &str) -> Option<&ContractPolicy> {
        self.contracts.get(id)
    }

    /// Return backend metadata by id.
    #[must_use]
    pub fn backend(&self, id: &str) -> Option<&ContractBackendPolicy> {
        self.backends.get(id)
    }

    /// Return the schema version string carried by this policy.
    #[must_use]
    pub const fn schema_version(&self) -> &str {
        self.schema_version.as_str()
    }
}

/// Per-contract resource and ability allowlist loaded from policy JSON.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ContractPolicy {
    /// Contract id; must match the key in [`Policy::contracts`].
    pub id: String,
    /// Human-readable policy description.
    #[serde(default)]
    pub description: String,
    /// Resource prefixes this contract permits.
    pub resource_prefixes: Vec<String>,
    /// Abilities this contract permits.
    pub abilities: Vec<String>,
}

impl ContractPolicy {
    /// Return true when `resource` is under one of the contract's permitted prefixes.
    #[must_use]
    pub fn permits_resource(&self, resource: &str) -> bool {
        self.resource_prefixes
            .iter()
            .any(|prefix| resource.starts_with(prefix))
    }

    /// Return true when `ability` is listed by this contract.
    #[must_use]
    pub fn permits_ability(&self, ability: &str) -> bool {
        self.abilities
            .iter()
            .any(|contract_ability| contract_ability == ability)
    }
}

/// Caller-supplied UCAN-like capability grant used by [`enforce`].
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct CapabilityGrant {
    /// Authorized resource scope.
    pub resource: String,
    /// Authorized ability.
    pub ability: String,
}

impl CapabilityGrant {
    /// Construct a capability grant from resource and ability strings.
    #[must_use]
    pub fn new(resource: impl Into<String>, ability: impl Into<String>) -> Self {
        Self {
            resource: resource.into(),
            ability: ability.into(),
        }
    }

    /// Return the authorized resource scope.
    #[must_use]
    pub const fn resource(&self) -> &str {
        self.resource.as_str()
    }

    /// Return the authorized ability.
    #[must_use]
    pub const fn ability(&self) -> &str {
        self.ability.as_str()
    }
}

/// Public enforcement request consumed by [`enforce`].
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct EnforcementRequest {
    /// Contract id to enforce.
    pub contract: String,
    /// Requested resource.
    pub resource: String,
    /// Requested ability.
    pub ability: String,
    /// Caller-supplied capability grants.
    pub capabilities: Vec<CapabilityGrant>,
}

impl EnforcementRequest {
    /// Construct a request without any capability grants.
    #[must_use]
    pub fn new(
        contract: impl Into<String>,
        resource: impl Into<String>,
        ability: impl Into<String>,
    ) -> Self {
        Self {
            contract: contract.into(),
            resource: resource.into(),
            ability: ability.into(),
            capabilities: Vec::new(),
        }
    }

    /// Append one capability grant and return the updated request.
    #[must_use]
    pub fn with_capability(mut self, capability: CapabilityGrant) -> Self {
        self.capabilities.push(capability);
        self
    }

    /// Replace the request's capability grants.
    #[must_use]
    pub fn with_capabilities(mut self, capabilities: impl Into<Vec<CapabilityGrant>>) -> Self {
        self.capabilities = capabilities.into();
        self
    }

    /// Return the requested contract id.
    #[must_use]
    pub const fn contract(&self) -> &str {
        self.contract.as_str()
    }

    /// Return the requested resource.
    #[must_use]
    pub const fn resource(&self) -> &str {
        self.resource.as_str()
    }

    /// Return the requested ability.
    #[must_use]
    pub const fn ability(&self) -> &str {
        self.ability.as_str()
    }

    /// Return caller-supplied capability grants.
    #[must_use]
    pub const fn capabilities(&self) -> &[CapabilityGrant] {
        self.capabilities.as_slice()
    }
}

/// Stable JSON-serializable enforcement result.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct EnforcementReceipt {
    /// Whether the request was allowed.
    pub allowed: bool,
    /// Stable human-readable allow/deny reason.
    pub reason: String,
}

impl EnforcementReceipt {
    /// Construct an allowed receipt.
    #[must_use]
    pub fn allowed() -> Self {
        Self {
            allowed: true,
            reason: "allowed".to_owned(),
        }
    }

    /// Construct a denied receipt with a stable reason.
    #[must_use]
    pub fn denied(reason: impl Into<String>) -> Self {
        Self {
            allowed: false,
            reason: reason.into(),
        }
    }

    /// Return true when the request was allowed.
    #[must_use]
    pub const fn is_allowed(&self) -> bool {
        self.allowed
    }

    /// Return true when the request was denied.
    #[must_use]
    pub const fn is_denied(&self) -> bool {
        !self.allowed
    }

    /// Return the receipt reason.
    #[must_use]
    pub const fn reason(&self) -> &str {
        self.reason.as_str()
    }
}

#[derive(Debug, Snafu)]
pub enum EnforcementError {
    #[snafu(display("failed to parse policy JSON: {source}"))]
    PolicyJson { source: serde_json::Error },
    #[snafu(display("unsupported policy schema version {version:?}"))]
    UnsupportedSchema { version: String },
    #[snafu(display("invalid UCAN capability: {source}"))]
    InvalidCapability { source: ucan::Error },
    #[snafu(display("UCAN authorization check failed: {source}"))]
    UcanAuthorization {
        source: ucan::VerifiedPrimitiveError,
    },
}

pub type EnforcementResult<T> = Result<T, EnforcementError>;

/// Load the packaged example policy JSON.
///
/// # Errors
///
/// Returns [`EnforcementError`] if the checked-in generated policy artifact is malformed
/// or carries an unsupported schema version.
pub fn default_policy() -> EnforcementResult<Policy> {
    parse_policy_json(DEFAULT_POLICY_JSON)
}

/// Load a Nickel-exported JSON policy and validate the schema version expected by this crate.
///
/// # Errors
///
/// Returns [`EnforcementError`] when JSON parsing fails or the policy schema version is unsupported.
pub fn parse_policy_json(input: &str) -> EnforcementResult<Policy> {
    let policy: Policy =
        serde_json::from_str(input).map_err(|source| EnforcementError::PolicyJson { source })?;
    if policy.schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(EnforcementError::UnsupportedSchema {
            version: policy.schema_version,
        });
    }
    Ok(policy)
}

/// Enforce a Nickel-authored policy contract and a caller-supplied UCAN capability set.
///
/// # Errors
///
/// Returns [`EnforcementError`] when capability construction or UCAN authorization validation fails.
pub fn enforce(
    policy: &Policy,
    request: &EnforcementRequest,
) -> EnforcementResult<EnforcementReceipt> {
    let Some(contract) = policy.contracts.get(request.contract.as_str()) else {
        return Ok(EnforcementReceipt::denied(format!(
            "unknown contract {:?}",
            request.contract
        )));
    };

    if contract.id != request.contract {
        return Ok(EnforcementReceipt::denied(format!(
            "contract key {:?} does not match policy id {:?}",
            request.contract, contract.id
        )));
    }

    if !contract
        .abilities
        .iter()
        .any(|ability| ability == &request.ability)
    {
        return Ok(EnforcementReceipt::denied(format!(
            "ability {:?} is outside contract {:?}",
            request.ability, request.contract
        )));
    }

    if !contract
        .resource_prefixes
        .iter()
        .any(|prefix| request.resource.starts_with(prefix))
    {
        return Ok(EnforcementReceipt::denied(format!(
            "resource {:?} is outside contract {:?}",
            request.resource, request.contract
        )));
    }

    let capabilities = request
        .capabilities
        .iter()
        .map(|capability| {
            ucan::CapabilityDocument::new(capability.resource.clone(), capability.ability.clone())
                .map_err(|source| EnforcementError::InvalidCapability { source })
        })
        .collect::<EnforcementResult<Vec<_>>>()?;

    let authorized = ucan::capability_set_authorizes(
        capabilities.as_slice(),
        &request.resource,
        &request.ability,
    )
    .map_err(|source| EnforcementError::UcanAuthorization { source })?;

    if authorized {
        Ok(EnforcementReceipt::allowed())
    } else {
        Ok(EnforcementReceipt::denied(
            "ucan capability set does not authorize request",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const POLICY_JSON: &str = r#"{
      "schema_version": "ucan-nickel-contracts.policy.v1",
      "backends": {
        "nickel": {
          "id": "nickel",
          "description": "Nickel-authored static policy contracts.",
          "supports_higher_order": false
        },
        "steel": {
          "id": "steel",
          "description": "Steel Scheme dynamic predicate contracts.",
          "supports_higher_order": true
        }
      },
      "contracts": {
        "document-write": {
          "id": "document-write",
          "description": "Write bounded document resources.",
          "resource_prefixes": ["did:key:z6Mkresource/docs/"],
          "abilities": ["document/write"]
        }
      }
    }"#;

    fn request(resource: &str, ability: &str) -> EnforcementRequest {
        EnforcementRequest {
            contract: "document-write".to_owned(),
            resource: resource.to_owned(),
            ability: ability.to_owned(),
            capabilities: vec![CapabilityGrant::new(resource, ability)],
        }
    }

    #[test]
    fn allows_policy_and_ucan_authorized_request() {
        let policy = parse_policy_json(POLICY_JSON).expect("policy should parse");
        let receipt = enforce(
            &policy,
            &request("did:key:z6Mkresource/docs/report", "document/write"),
        )
        .expect("enforcement should not error");

        assert_eq!(receipt, EnforcementReceipt::allowed());
    }

    #[test]
    fn denies_resource_outside_nickel_contract() {
        let policy = parse_policy_json(POLICY_JSON).expect("policy should parse");
        let receipt = enforce(
            &policy,
            &request("did:key:z6Mkresource/private/report", "document/write"),
        )
        .expect("enforcement should not error");

        assert!(!receipt.allowed);
        assert!(receipt.reason.contains("outside contract"));
    }

    #[test]
    fn denies_ability_outside_nickel_contract() {
        let policy = parse_policy_json(POLICY_JSON).expect("policy should parse");
        let receipt = enforce(
            &policy,
            &request("did:key:z6Mkresource/docs/report", "document/delete"),
        )
        .expect("enforcement should not error");

        assert!(!receipt.allowed);
        assert!(receipt.reason.contains("outside contract"));
    }

    #[test]
    fn denies_missing_ucan_capability_even_when_contract_allows() {
        let policy = parse_policy_json(POLICY_JSON).expect("policy should parse");
        let mut request = request("did:key:z6Mkresource/docs/report", "document/write");
        request.capabilities = vec![CapabilityGrant::new(
            "did:key:z6Mkresource/docs/other",
            "document/write",
        )];

        let receipt = enforce(&policy, &request).expect("enforcement should not error");

        assert!(!receipt.allowed);
        assert!(receipt.reason.contains("ucan capability set"));
    }

    #[test]
    fn denies_resource_prefix_sibling_boundary() {
        let policy = parse_policy_json(POLICY_JSON).expect("policy should parse");
        let receipt = enforce(
            &policy,
            &request("did:key:z6Mkresource/docs-private/report", "document/write"),
        )
        .expect("enforcement should not error");

        assert!(receipt.is_denied());
        assert!(receipt.reason().contains("outside contract"));
    }

    #[test]
    fn allows_when_one_of_many_capabilities_authorizes_request() {
        let policy = parse_policy_json(POLICY_JSON).expect("policy should parse");
        let request = EnforcementRequest::new(
            "document-write",
            "did:key:z6Mkresource/docs/report",
            "document/write",
        )
        .with_capabilities(vec![
            CapabilityGrant::new("did:key:z6Mkresource/docs/other", "document/write"),
            CapabilityGrant::new("did:key:z6Mkresource/docs/report", "document/write"),
        ]);

        let receipt = enforce(&policy, &request).expect("enforcement should not error");

        assert!(receipt.is_allowed());
    }

    #[test]
    fn denies_unknown_contract_without_ucan_check() {
        let policy = parse_policy_json(POLICY_JSON).expect("policy should parse");
        let request = EnforcementRequest::new(
            "unknown-contract",
            "did:key:z6Mkresource/docs/report",
            "document/write",
        )
        .with_capability(CapabilityGrant::new("", ""));

        let receipt = enforce(&policy, &request).expect("unknown contract should deny");

        assert!(receipt.is_denied());
        assert!(receipt.reason().contains("unknown contract"));
    }

    #[test]
    fn denies_contract_key_and_policy_id_mismatch() {
        let policy_json = r#"{
          "schema_version": "ucan-nickel-contracts.policy.v1",
          "contracts": {
            "document-write": {
              "id": "other-contract",
              "description": "Mismatched id.",
              "resource_prefixes": ["did:key:z6Mkresource/docs/"],
              "abilities": ["document/write"]
            }
          }
        }"#;
        let policy = parse_policy_json(policy_json).expect("policy should parse");

        let receipt = enforce(
            &policy,
            &request("did:key:z6Mkresource/docs/report", "document/write"),
        )
        .expect("id mismatch should deny");

        assert!(receipt.is_denied());
        assert!(receipt.reason().contains("does not match policy id"));
    }

    #[test]
    fn malformed_contract_shape_fails_closed_at_parse() {
        let error = parse_policy_json(
            r#"{
              "schema_version": "ucan-nickel-contracts.policy.v1",
              "contracts": {
                "document-write": {
                  "id": "document-write",
                  "description": "Missing required allowlists."
                }
              }
            }"#,
        )
        .expect_err("missing allowlists should fail parsing");

        assert!(matches!(error, EnforcementError::PolicyJson { .. }));
    }

    #[test]
    fn validates_shared_contract_envelope_for_steel_backend() {
        let envelope = ContractEnvelope::new(
            "steel",
            "document-write-predicate",
            "v1",
            "blake3:steel-source",
            "ucan.contract.input.v1",
            "ucan.contract.output.v1",
            "ucan.contract.receipt.v1",
        );

        assert_eq!(envelope.backend_kind(), Some(ContractBackendKind::Steel));
        let receipt = validate_contract_envelope(&envelope);

        assert!(receipt.is_accepted());
        assert_eq!(receipt.reason, "accepted");
    }

    #[test]
    fn denies_unsupported_contract_envelope_backend() {
        let envelope = ContractEnvelope::new(
            "lua",
            "document-write-predicate",
            "v1",
            "blake3:lua-source",
            "ucan.contract.input.v1",
            "ucan.contract.output.v1",
            "ucan.contract.receipt.v1",
        );

        let receipt = validate_contract_envelope(&envelope);

        assert!(!receipt.is_accepted());
        assert!(receipt.reason.contains("unsupported contract backend"));
    }

    #[test]
    fn parses_backend_policy_metadata() {
        let policy = parse_policy_json(POLICY_JSON).expect("policy should parse");

        let nickel = policy.backend("nickel").expect("nickel backend exists");
        assert!(nickel.is_supported());
        assert!(!nickel.supports_higher_order);

        let steel = policy.backend("steel").expect("steel backend exists");
        assert!(steel.is_supported());
        assert!(steel.supports_higher_order);
    }
    fn steel_envelope() -> ContractEnvelope {
        ContractEnvelope::new(
            "steel",
            "document-write-predicate",
            "v1",
            "blake3:steel-source",
            "ucan.contract.input.v1",
            "ucan.contract.output.v1",
            "ucan.contract.receipt.v1",
        )
    }

    fn callable_descriptor() -> CallableContractDescriptor {
        CallableContractDescriptor {
            callable_id: "can-write-document".to_owned(),
            arity: 1,
            argument_contracts: vec!["document-request.v1".to_owned()],
            return_contract: "bool".to_owned(),
            required_capabilities: vec![CapabilityGrant::new(
                "did:key:z6Mkresource/docs/report",
                "document/write",
            )],
            effect_class: ContractEffectClass::ReadsDeclaredCapabilities,
            content_hash: "blake3:callable-source".to_owned(),
            redaction_class: "safe-summary".to_owned(),
        }
    }

    #[test]
    fn validates_bounded_steel_dto_request() {
        let request = SteelEvaluationRequest {
            envelope: steel_envelope(),
            input: serde_json::json!({"resource":"did:key:z6Mkresource/docs/report"}),
            max_input_bytes: 256,
            callable: Some(callable_descriptor()),
            requested_host_capability: None,
        };

        let receipt = validate_steel_evaluation_request(&request);

        assert!(receipt.is_accepted());
        assert_eq!(receipt.reason, "accepted");
        assert!(steel_request_hash(&request).starts_with("blake3:"));
    }

    #[test]
    fn denies_steel_request_for_ambient_host_capability() {
        let request = SteelEvaluationRequest {
            envelope: steel_envelope(),
            input: serde_json::json!({"resource":"did:key:z6Mkresource/docs/report"}),
            max_input_bytes: 256,
            callable: None,
            requested_host_capability: Some("filesystem.read".to_owned()),
        };

        let receipt = validate_steel_evaluation_request(&request);

        assert!(!receipt.is_accepted());
        assert!(receipt.reason.contains("denied by default"));
    }

    #[test]
    fn validates_higher_order_callable_descriptor() {
        let receipt = validate_callable_descriptor(&callable_descriptor());

        assert!(receipt.is_accepted());
    }

    #[test]
    fn denies_opaque_callable_without_content_hash() {
        let mut descriptor = callable_descriptor();
        descriptor.content_hash.clear();

        let receipt = validate_callable_descriptor(&descriptor);

        assert!(!receipt.is_accepted());
        assert!(receipt.reason.contains("content hash"));
    }

    #[test]
    fn denies_callable_arity_mismatch() {
        let mut descriptor = callable_descriptor();
        descriptor.arity = 2;

        let receipt = validate_callable_descriptor(&descriptor);

        assert!(!receipt.is_accepted());
        assert!(receipt.reason.contains("arity"));
    }

    #[test]
    fn steel_receipt_hash_is_stable() {
        let capability = CapabilityGrant::new("did:key:z6Mkresource/docs/report", "document/write");
        let mut receipt = SteelEvaluationReceipt {
            backend: "steel".to_owned(),
            contract_id: "document-write-predicate".to_owned(),
            normalized_source_hash: "blake3:steel-source".to_owned(),
            request_hash: "blake3:request".to_owned(),
            allowed: true,
            reason: "allowed".to_owned(),
            caveat_evidence: BTreeMap::from([("predicate".to_owned(), "passed".to_owned())]),
            effect_class: ContractEffectClass::ReadsDeclaredCapabilities,
            required_capabilities: vec![capability.clone()],
            checked_capabilities: vec![capability],
            receipt_hash: None,
        };
        let hash = steel_receipt_hash(&receipt);
        receipt.receipt_hash = Some(hash.clone());

        assert_eq!(steel_receipt_hash(&receipt), hash);
        assert!(validate_steel_evaluation_receipt(&receipt).is_accepted());
    }

    #[test]
    fn rejects_steel_allow_receipt_missing_authority_evidence() {
        let receipt = SteelEvaluationReceipt {
            backend: "steel".to_owned(),
            contract_id: "document-write-predicate".to_owned(),
            normalized_source_hash: "blake3:steel-source".to_owned(),
            request_hash: "blake3:request".to_owned(),
            allowed: true,
            reason: "allowed".to_owned(),
            caveat_evidence: BTreeMap::new(),
            effect_class: ContractEffectClass::ReadsDeclaredCapabilities,
            required_capabilities: vec![CapabilityGrant::new(
                "did:key:z6Mkresource/docs/report",
                "document/write",
            )],
            checked_capabilities: Vec::new(),
            receipt_hash: None,
        };

        let validation = validate_steel_evaluation_receipt(&receipt);

        assert!(!validation.is_accepted());
        assert!(validation.reason.contains("authority evidence"));
    }
}
