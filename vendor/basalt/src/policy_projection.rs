//! Pure structural parity check between a generated policy artifact and the
//! Rust consumer's supported contract.
//!
//! This core is deterministic and performs no I/O and no Nickel evaluation. A
//! thin shell owns file reads and Nickel invocation. Runtime enforcement never
//! evaluates Nickel.

use crate::{Policy, SUPPORTED_SCHEMA_VERSION};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const MAXIMUM_ID_BYTES: usize = 128;
const MAXIMUM_FIELD_BYTES: usize = 4_096;
const MAXIMUM_ALLOWLIST_BYTES: usize = 2_048;
const MAXIMUM_ALLOWLIST_ITEMS: usize = 256;
const MAXIMUM_TOKENS_PER_ITEM: usize = 16;
const MAXIMUM_PATH_SEGMENT_BYTES: usize = 256;

/// One deterministic projection-parity finding.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProjectionIssue {
    /// Stable issue class.
    pub class: String,
    /// Human-readable finding.
    pub message: String,
}

impl ProjectionIssue {
    /// Construct one issue.
    #[must_use]
    pub const fn new(class: String, message: String) -> Self {
        Self { class, message }
    }
}

/// Result of validating one generated policy artifact against the Rust contract.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProjectionVerdict {
    /// Whether the projection is meaning-compatible with the Rust consumer.
    pub accepted: bool,
    /// Deterministic issue classes and messages.
    pub issues: Vec<ProjectionIssue>,
}

impl ProjectionVerdict {
    /// Return true when the projection is meaning-compatible.
    #[must_use]
    pub const fn is_accepted(&self) -> bool {
        self.accepted
    }
}

/// Validate the supported schema and whole-policy structure of one generated artifact.
///
/// This is a pure deterministic decision. It checks the Rust consumer's
/// contract (schema, backend vocabulary, key/id alignment, non-empty allowlists,
/// and lexical value shape) without reimplementing UCAN semantics.
#[must_use]
pub fn validate_projection(policy: &Policy) -> ProjectionVerdict {
    let mut issues: Vec<ProjectionIssue> = Vec::new();

    check_schema(&mut issues, policy.schema_version.as_str());
    check_contracts(&mut issues, policy);
    check_backends(&mut issues, policy);
    check_lexical(&mut issues, policy);

    ProjectionVerdict {
        accepted: issues.is_empty(),
        issues,
    }
}

fn check_schema(issues: &mut Vec<ProjectionIssue>, schema: &str) {
    if schema != SUPPORTED_SCHEMA_VERSION {
        issues.push(ProjectionIssue::new(
            "projection.schema".to_owned(),
            "generated policy schema does not match the supported Rust version".to_owned(),
        ));
    }
}

fn check_contracts(issues: &mut Vec<ProjectionIssue>, policy: &Policy) {
    if policy.contracts.is_empty() {
        issues.push(ProjectionIssue::new(
            "projection.contracts.empty".to_owned(),
            "policy must declare at least one contract".to_owned(),
        ));
        return;
    }
    let mut seen_ids: HashSet<&str> = HashSet::new();
    for (key, contract) in &policy.contracts {
        check_id(issues, key.as_str(), "projection.contract-key");
        check_id(issues, contract.id.as_str(), "projection.contract-id");
        if key != &contract.id {
            issues.push(ProjectionIssue::new(
                "projection.contracts.key-id-mismatch".to_owned(),
                format!("contract key {key:?} does not match its embedded id"),
            ));
        }
        if !seen_ids.insert(contract.id.as_str()) {
            issues.push(ProjectionIssue::new(
                "projection.contracts.duplicate-id".to_owned(),
                "policy declares duplicate contract identity".to_owned(),
            ));
        }
        check_allowlist(
            issues,
            key.as_str(),
            &contract.resource_prefixes,
            "projection.contracts.resource-prefixes",
        );
        check_allowlist(
            issues,
            key.as_str(),
            &contract.abilities,
            "projection.contracts.abilities",
        );
    }
}

fn check_backends(issues: &mut Vec<ProjectionIssue>, policy: &Policy) {
    let mut seen_ids: HashSet<&str> = HashSet::new();
    for (key, backend) in &policy.backends {
        check_id(issues, key.as_str(), "projection.backend-key");
        check_id(issues, backend.id.as_str(), "projection.backend-id");
        if key != &backend.id {
            issues.push(ProjectionIssue::new(
                "projection.backends.key-id-mismatch".to_owned(),
                format!("backend key {key:?} does not match its embedded id"),
            ));
        }
        if supported_backend_negative(backend.id.as_str()) {
            issues.push(ProjectionIssue::new(
                "projection.backends.unsupported".to_owned(),
                format!(
                    "backend id {:?} is not in the supported Rust vocabulary",
                    backend.id
                ),
            ));
        }
        if !seen_ids.insert(backend.id.as_str()) {
            issues.push(ProjectionIssue::new(
                "projection.backends.duplicate-id".to_owned(),
                "policy declares duplicate backend identity".to_owned(),
            ));
        }
    }
}

fn check_lexical(issues: &mut Vec<ProjectionIssue>, policy: &Policy) {
    for contract in policy.contracts.values() {
        for prefix in &contract.resource_prefixes {
            check_lexical_value(
                issues,
                prefix.as_str(),
                "projection.lexical.resource-prefix",
            );
        }
        for ability in &contract.abilities {
            check_ability(issues, ability.as_str());
        }
    }
}

fn check_allowlist(
    issues: &mut Vec<ProjectionIssue>,
    contract_key: &str,
    allowlist: &[String],
    class: &str,
) {
    if allowlist.is_empty() {
        issues.push(ProjectionIssue::new(
            class.to_owned(),
            format!("contract {contract_key:?} must declare a non-empty allowlist"),
        ));
        return;
    }
    if allowlist.len() > MAXIMUM_ALLOWLIST_ITEMS {
        issues.push(ProjectionIssue::new(
            format!("{class}.limit"),
            "allowlist exceeds the bounded item maximum".to_owned(),
        ));
    }
    let mut seen: HashSet<&str> = HashSet::new();
    for item in allowlist {
        if item.is_empty() || item.len() > MAXIMUM_ALLOWLIST_BYTES {
            issues.push(ProjectionIssue::new(
                format!("{class}.bounded"),
                "allowlist entry is empty or exceeds the bounded byte length".to_owned(),
            ));
        }
        if !seen.insert(item.as_str()) {
            issues.push(ProjectionIssue::new(
                format!("{class}.duplicate"),
                "allowlist contains a duplicate normalized entry".to_owned(),
            ));
        }
    }
}

fn check_ability(issues: &mut Vec<ProjectionIssue>, ability: &str) {
    if ability.is_empty() || ability.len() > MAXIMUM_FIELD_BYTES {
        issues.push(ProjectionIssue::new(
            "projection.lexical.ability.bounded".to_owned(),
            "ability is empty or exceeds the bounded byte length".to_owned(),
        ));
        return;
    }
    if ability.chars().any(char::is_control) {
        issues.push(ProjectionIssue::new(
            "projection.lexical.ability.control".to_owned(),
            "ability contains a control character".to_owned(),
        ));
    }
    if ability.split_whitespace().count() != 1 {
        issues.push(ProjectionIssue::new(
            "projection.lexical.ability.whitespace".to_owned(),
            "ability must not contain internal whitespace".to_owned(),
        ));
    }
    let segments: Vec<&str> = ability.split('/').collect();
    if segments.is_empty()
        || segments.len() > MAXIMUM_TOKENS_PER_ITEM
        || segments.iter().any(|segment| segment.is_empty())
        || segments
            .iter()
            .any(|segment| segment.len() > MAXIMUM_PATH_SEGMENT_BYTES)
    {
        issues.push(ProjectionIssue::new(
            "projection.lexical.ability.path".to_owned(),
            "ability has malformed path segments".to_owned(),
        ));
    }
}

fn check_id(issues: &mut Vec<ProjectionIssue>, id: &str, class: &str) {
    if id.is_empty() || id.len() > MAXIMUM_ID_BYTES {
        issues.push(ProjectionIssue::new(
            class.to_owned(),
            "identity is empty or exceeds the bounded byte length".to_owned(),
        ));
    }
}

fn check_lexical_value(issues: &mut Vec<ProjectionIssue>, value: &str, class: &str) {
    if value.is_empty() || value.len() > MAXIMUM_FIELD_BYTES {
        issues.push(ProjectionIssue::new(
            class.to_owned(),
            "lexical value is empty or exceeds the bounded byte length".to_owned(),
        ));
        return;
    }
    if value.chars().any(char::is_control) {
        issues.push(ProjectionIssue::new(
            format!("{class}.control"),
            "lexical value contains a control character".to_owned(),
        ));
    }
    if value.split_whitespace().count() != 1 {
        issues.push(ProjectionIssue::new(
            format!("{class}.whitespace"),
            "lexical value must not contain internal whitespace".to_owned(),
        ));
    }
}

fn supported_backend_negative(backend: &str) -> bool {
    !matches!(backend, "nickel" | "steel")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ContractBackendPolicy, ContractPolicy, EnforcementRequest};
    use std::collections::BTreeMap;

    fn valid_policy() -> Policy {
        let mut contracts = BTreeMap::new();
        contracts.insert(
            "document-write".to_owned(),
            ContractPolicy {
                id: "document-write".to_owned(),
                description: String::new(),
                resource_prefixes: vec!["did:key:z6Mkresource/docs/".to_owned()],
                abilities: vec!["document/write".to_owned()],
            },
        );
        let mut backends = BTreeMap::new();
        backends.insert(
            "nickel".to_owned(),
            ContractBackendPolicy {
                id: "nickel".to_owned(),
                description: String::new(),
                supports_higher_order: false,
            },
        );
        Policy {
            schema_version: SUPPORTED_SCHEMA_VERSION.to_owned(),
            contracts,
            backends,
        }
    }

    #[test]
    fn valid_projection_passes() {
        let verdict = validate_projection(&valid_policy());
        assert!(verdict.is_accepted(), "{:?}", verdict.issues);
    }

    #[test]
    fn key_id_mismatch_fails_closed() {
        let mut policy = valid_policy();
        let entry = policy
            .contracts
            .get_mut("document-write")
            .expect("test contract");
        entry.id = "document-read".to_owned();
        let verdict = validate_projection(&policy);
        assert!(!verdict.is_accepted());
        assert!(
            verdict
                .issues
                .iter()
                .any(|i| i.class == "projection.contracts.key-id-mismatch")
        );
    }

    #[test]
    fn empty_allowlist_fails_closed() {
        let mut policy = valid_policy();
        let entry = policy
            .contracts
            .get_mut("document-write")
            .expect("test contract");
        entry.abilities.clear();
        let verdict = validate_projection(&policy);
        assert!(!verdict.is_accepted());
        assert!(
            verdict
                .issues
                .iter()
                .any(|i| i.class == "projection.contracts.abilities")
        );
    }

    #[test]
    fn duplicate_allowlist_entry_fails_closed() {
        let mut policy = valid_policy();
        let entry = policy
            .contracts
            .get_mut("document-write")
            .expect("test contract");
        entry.abilities.push("document/write".to_owned());
        let verdict = validate_projection(&policy);
        assert!(!verdict.is_accepted());
        assert!(
            verdict
                .issues
                .iter()
                .any(|i| i.class == "projection.contracts.abilities.duplicate")
        );
    }

    #[test]
    fn malformed_ability_fails_lexically() {
        let mut policy = valid_policy();
        let entry = policy
            .contracts
            .get_mut("document-write")
            .expect("test contract");
        entry.abilities = vec!["document/".to_owned()];
        let verdict = validate_projection(&policy);
        assert!(!verdict.is_accepted());
        assert!(
            verdict
                .issues
                .iter()
                .any(|i| i.class == "projection.lexical.ability.path")
        );
    }

    #[test]
    fn unsupported_backend_fails_closed() {
        let mut policy = valid_policy();
        let mut backends = BTreeMap::new();
        backends.insert(
            "rust".to_owned(),
            ContractBackendPolicy {
                id: "rust".to_owned(),
                description: String::new(),
                supports_higher_order: false,
            },
        );
        policy.backends = backends;
        let verdict = validate_projection(&policy);
        assert!(!verdict.is_accepted());
        assert!(
            verdict
                .issues
                .iter()
                .any(|i| i.class == "projection.backends.unsupported")
        );
    }

    #[test]
    fn empty_contracts_fail_closed() {
        let mut policy = valid_policy();
        policy.contracts = BTreeMap::new();
        let verdict = validate_projection(&policy);
        assert!(!verdict.is_accepted());
        assert!(
            verdict
                .issues
                .iter()
                .any(|i| i.class == "projection.contracts.empty")
        );
    }

    #[test]
    fn accepted_projection_still_requires_runtime_authorization() {
        let policy = valid_policy();
        let request = EnforcementRequest::new(
            "document-write",
            "did:key:z6Mkresource/docs/report",
            "document/write",
        )
        .with_capability(crate::CapabilityGrant::new(
            "did:key:z6Mkresource/docs/report",
            "document/write",
        ));
        let verdict = validate_projection(&policy);
        // Parity conformance does not by itself authorize anything.
        assert!(verdict.is_accepted());
        let receipt = crate::enforce(&policy, &request).expect("enforcement must run");
        assert!(receipt.is_allowed());
    }
}
