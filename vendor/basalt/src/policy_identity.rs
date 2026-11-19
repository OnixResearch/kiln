//! iddqd-backed policy identity maps for contracts and backend metadata.
//!
//! Basalt treats checked-in generated policy JSON as untrusted input and
//! builds validated in-memory identity indexes before enforcement. This core
//! converts parsed [`Policy`] data into iddqd `IdOrdMap` indexes keyed by each
//! value's own stable id. It uses explicit unique insertion so duplicate ids
//! fail closed instead of silently overwriting prior entries, and it compares
//! each object key with the mapped value's id before enforcement.

use crate::{ContractBackendPolicy, ContractPolicy, Policy};
use iddqd::{IdOrdItem, IdOrdMap, id_upcast};
use serde::{Deserialize, Serialize};

/// Schema marker for a validated policy identity index set.
pub const POLICY_IDENTITY_SCHEMA: &str = "basalt.policy-identity.v1";
/// Maximum accepted identity-map entry count.
pub const MAX_IDENTITY_ENTRIES: usize = 512;

/// One deterministic identity-index finding.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct PolicyIdentityIssue {
    /// Stable issue class.
    pub class: String,
    /// Stable human-readable explanation.
    pub message: String,
}

impl PolicyIdentityIssue {
    fn new(class: &str, message: &str) -> Self {
        Self {
            class: class.to_owned(),
            message: message.to_owned(),
        }
    }
}

/// Validated identity indexes built from a parsed policy artifact.
///
/// Keys are borrowed from each value's own `id`, so a caller can resolve a
/// contract or backend by its stable id without trusting a duplicated external
/// map key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyIdentityIndexes {
    /// Contracts keyed by `ContractPolicy.id`.
    contracts: IdOrdMap<ContractPolicy>,
    /// Backend metadata keyed by `ContractBackendPolicy.id`.
    backends: IdOrdMap<ContractBackendPolicy>,
}

impl PolicyIdentityIndexes {
    /// Resolve a contract by its stable id.
    #[must_use]
    pub fn contract(&self, id: &str) -> Option<&ContractPolicy> {
        self.contracts.get(id)
    }

    /// Resolve backend metadata by its stable id.
    #[must_use]
    pub fn backend(&self, id: &str) -> Option<&ContractBackendPolicy> {
        self.backends.get(id)
    }

    /// Return the number of indexed contracts.
    #[must_use]
    pub fn contract_count(&self) -> usize {
        self.contracts.len()
    }

    /// Return the number of indexed backend definitions.
    #[must_use]
    pub fn backend_count(&self) -> usize {
        self.backends.len()
    }

    /// Iterate over indexed contracts in stable id order.
    pub fn contracts(&self) -> impl Iterator<Item = &ContractPolicy> {
        self.contracts.iter()
    }
}

impl IdOrdItem for ContractPolicy {
    type Key<'a>
        = &'a str
    where
        Self: 'a;

    fn key(&self) -> Self::Key<'_> {
        self.id.as_str()
    }

    id_upcast!();
}

impl IdOrdItem for ContractBackendPolicy {
    type Key<'a>
        = &'a str
    where
        Self: 'a;

    fn key(&self) -> Self::Key<'_> {
        self.id.as_str()
    }

    id_upcast!();
}

/// Build validated policy identity indexes from a parsed policy artifact.
///
/// This is a pure deterministic conversion. Each contract and backend object
/// key must equal the mapped value's own id, every id must be non-empty, and
/// ids must be unique. Backend metadata must route through a supported backend
/// id. Any violation returns a deterministic ordered issue report and refuses
/// to hand back usable indexes.
///
/// # Errors
///
/// Returns a non-empty `Vec<PolicyIdentityIssue>` when any identity drift,
/// duplicate id, empty id, key/value mismatch, or unsupported backend id is
/// detected before enforcement.
pub fn build_policy_identity_indexes(
    policy: &Policy,
) -> Result<PolicyIdentityIndexes, Vec<PolicyIdentityIssue>> {
    let mut issues = Vec::new();
    let mut contracts = IdOrdMap::<ContractPolicy>::new();
    let mut backends = IdOrdMap::<ContractBackendPolicy>::new();

    build_contract_index(policy, &mut contracts, &mut issues);
    build_backend_index(policy, &mut backends, &mut issues);

    if issues.is_empty() {
        Ok(PolicyIdentityIndexes {
            contracts,
            backends,
        })
    } else {
        Err(issues)
    }
}

fn build_contract_index(
    policy: &Policy,
    contracts: &mut IdOrdMap<ContractPolicy>,
    issues: &mut Vec<PolicyIdentityIssue>,
) {
    for (key, value) in &policy.contracts {
        if !bounded_non_empty(value.id.as_str()) {
            issues.push(PolicyIdentityIssue::new(
                "policy-identity.contract.empty-id",
                &format!("contract object {key} carries an empty or oversized id"),
            ));
            continue;
        }
        if key.as_str() != value.id {
            issues.push(PolicyIdentityIssue::new(
                "policy-identity.contract.key-value-id-mismatch",
                &format!(
                    "contract object key {key} does not match value id {}",
                    value.id
                ),
            ));
        }
        if let Err(error) = contracts.insert_unique(value.clone()) {
            issues.push(PolicyIdentityIssue::new(
                "policy-identity.contract.duplicate-id",
                &format!("duplicate contract id {}", value.id),
            ));
            let _ = error;
        }
    }
}

fn build_backend_index(
    policy: &Policy,
    backends: &mut IdOrdMap<ContractBackendPolicy>,
    issues: &mut Vec<PolicyIdentityIssue>,
) {
    for (key, value) in &policy.backends {
        if !bounded_non_empty(value.id.as_str()) {
            issues.push(PolicyIdentityIssue::new(
                "policy-identity.backend.empty-id",
                &format!("backend object {key} carries an empty or oversized id"),
            ));
            continue;
        }
        if key.as_str() != value.id {
            issues.push(PolicyIdentityIssue::new(
                "policy-identity.backend.key-value-id-mismatch",
                &format!(
                    "backend object key {key} does not match value id {}",
                    value.id
                ),
            ));
        }
        if !value.is_supported() {
            issues.push(PolicyIdentityIssue::new(
                "policy-identity.backend.unsupported",
                &format!("backend id {} is not a supported backend", value.id),
            ));
            continue;
        }
        if let Err(error) = backends.insert_unique(value.clone()) {
            issues.push(PolicyIdentityIssue::new(
                "policy-identity.backend.duplicate-id",
                &format!("duplicate backend id {}", value.id),
            ));
            let _ = error;
        }
    }
}

fn bounded_non_empty(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EnforcementRequest, Policy, enforce, parse_policy_json};

    fn valid_policy_json() -> &'static str {
        r#"{
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
        }"#
    }

    fn policy(json: &str) -> Policy {
        parse_policy_json(json).expect("policy should parse")
    }

    #[test]
    fn positive_matching_policy_builds_valid_indexes() {
        let policy = policy(valid_policy_json());
        let indexes = build_policy_identity_indexes(&policy).expect("valid policy indexes");

        assert_eq!(indexes.contract_count(), 1);
        assert_eq!(indexes.backend_count(), 2);
        let contract = indexes
            .contract("document-write")
            .expect("contract present");
        assert_eq!(contract.id, "document-write");
        assert!(indexes.backend("nickel").is_some());
        assert!(indexes.backend("steel").is_some());
    }

    #[test]
    fn enforcement_still_resolves_via_stable_id() {
        let policy = policy(valid_policy_json());
        let indexes = build_policy_identity_indexes(&policy).expect("valid indexes");
        let request = EnforcementRequest::new(
            "document-write",
            "did:key:z6Mkresource/docs/report",
            "document/write",
        )
        .with_capability(crate::CapabilityGrant::new(
            "did:key:z6Mkresource/docs/report",
            "document/write",
        ));

        let resolved = indexes.contract("document-write").expect("resolved by id");
        assert_eq!(resolved.id, request.contract);
        assert!(enforce(&policy, &request).is_ok_and(|r| r.is_allowed()));
    }

    #[test]
    fn negative_duplicate_contract_id_fails_closed() {
        let policy = policy(
            r#"{
              "schema_version": "ucan-nickel-contracts.policy.v1",
              "backends": {},
              "contracts": {
                "a": { "id": "dup", "description": "x", "resource_prefixes": ["a/"], "abilities": ["a"] },
                "b": { "id": "dup", "description": "y", "resource_prefixes": ["b/"], "abilities": ["b"] }
              }
            }"#,
        );

        let issues = build_policy_identity_indexes(&policy).expect_err("duplicate id must fail");
        assert!(
            issues
                .iter()
                .any(|i| i.class == "policy-identity.contract.duplicate-id")
        );
    }

    #[test]
    fn negative_key_value_id_mismatch_fails_closed() {
        let policy = policy(
            r#"{
              "schema_version": "ucan-nickel-contracts.policy.v1",
              "backends": {},
              "contracts": {
                "wrong-key": { "id": "right-id", "description": "x", "resource_prefixes": ["a/"], "abilities": ["a"] }
              }
            }"#,
        );

        let issues = build_policy_identity_indexes(&policy).expect_err("mismatch must fail");
        assert!(
            issues
                .iter()
                .any(|i| i.class == "policy-identity.contract.key-value-id-mismatch")
        );
    }

    #[test]
    fn negative_empty_contract_id_fails_closed() {
        let policy = policy(
            r#"{
              "schema_version": "ucan-nickel-contracts.policy.v1",
              "backends": {},
              "contracts": {
                "": { "id": "", "description": "x", "resource_prefixes": ["a/"], "abilities": ["a"] }
              }
            }"#,
        );

        let issues = build_policy_identity_indexes(&policy).expect_err("empty id must fail");
        assert!(
            issues
                .iter()
                .any(|i| i.class == "policy-identity.contract.empty-id")
        );
    }

    #[test]
    fn negative_unsupported_backend_fails_closed() {
        let policy = policy(
            r#"{
              "schema_version": "ucan-nickel-contracts.policy.v1",
              "backends": {
                "weird": { "id": "weird", "description": "x", "supports_higher_order": false }
              },
              "contracts": {}
            }"#,
        );

        let issues =
            build_policy_identity_indexes(&policy).expect_err("unsupported backend must fail");
        assert!(
            issues
                .iter()
                .any(|i| i.class == "policy-identity.backend.unsupported")
        );
    }

    #[test]
    fn negative_empty_backend_id_fails_closed() {
        let policy = policy(
            r#"{
              "schema_version": "ucan-nickel-contracts.policy.v1",
              "backends": {
                "": { "id": "", "description": "x", "supports_higher_order": false }
              },
              "contracts": {}
            }"#,
        );

        let issues =
            build_policy_identity_indexes(&policy).expect_err("empty backend id must fail");
        assert!(
            issues
                .iter()
                .any(|i| i.class == "policy-identity.backend.empty-id")
        );
    }

    #[test]
    fn negative_backend_key_id_mismatch_fails_closed() {
        let policy = policy(
            r#"{
              "schema_version": "ucan-nickel-contracts.policy.v1",
              "backends": {
                "other-key": { "id": "nickel", "description": "x", "supports_higher_order": false }
              },
              "contracts": {}
            }"#,
        );

        let issues =
            build_policy_identity_indexes(&policy).expect_err("backend mismatch must fail");
        assert!(
            issues
                .iter()
                .any(|i| i.class == "policy-identity.backend.key-value-id-mismatch")
        );
    }

    #[test]
    fn negative_malformed_policy_fails_at_parse() {
        let result = parse_policy_json(r#"{"schema_version":"ucan-nickel-contracts.policy.v1"}"#);
        assert!(
            result.is_err(),
            "malformed policy without required contracts should fail parse"
        );
    }

    #[test]
    fn contracts_iterate_in_stable_order() {
        let policy = policy(valid_policy_json());
        let indexes = build_policy_identity_indexes(&policy).expect("valid indexes");
        let ids = indexes
            .contracts()
            .map(|c| c.id.as_str())
            .collect::<Vec<_>>();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
    }
}
