use std::collections::BTreeSet;

use serde::Deserialize;
use serde::Serialize;

use super::types::IdentityIssue;
use crate::verified::evidence::canonical_json_hash;

pub const NOMINAL_IDENTITY_RUNTIME_SCHEMA_V1: &str = "cairn.nominal-evidence-identity-runtime.v1";
pub const NOMINAL_IDENTITY_OCTET_SCHEMA_V1: &str = "octet-nominal-domain-policy/v1";
pub const NOMINAL_IDENTITY_POLICY_ID: &str = "cairn.nominal-evidence-identities.v1";
pub const NOMINAL_IDENTITY_REQUIRED_NON_CLAIM: &str = "nominal identities prove configured local syntax and role separation only; they do not prove evidence truth, authority, policy correctness, verifier soundness, mutation safety outside checked plans, or release eligibility";

const MAX_DOMAIN_COUNT: usize = 64;
const MAX_DOMAIN_SYMBOL_COUNT: usize = 16;
const MAX_DOMAIN_SCOPE_COUNT: usize = 32;
const FIELD_POLICY_ID: &str = "nominal_identity_policy.policy_id";
const FIELD_RUNTIME: &str = "nominal_identity_policy.runtime";
const FIELD_OCTET: &str = "nominal_identity_policy.octet";
const FIELD_NON_CLAIMS: &str = "nominal_identity_policy.non_claims";
const SUPPORTED_CATEGORIES: &[&str] = &[
    "domain-id",
    "domain-ref",
    "blake3-digest",
    "imported-digest",
];
const SUPPORTED_ALGORITHMS: &[&str] = &["none", "BLAKE3", "SHA-256", "Git-OID-SHA-1"];
const SUPPORTED_WRAPPERS: &[&str] = &["refined-newtype", "marker-generic"];
const SUPPORTED_CONSTRUCTORS: &[&str] = &["private-fallible", "marker-only"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NominalIdentityPolicy {
    pub policy_id: String,
    pub runtime: NominalIdentityRuntimeProjection,
    pub octet: NominalIdentityOctetProjection,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NominalIdentityRuntimeProjection {
    pub schema_version: String,
    pub policy_id: String,
    pub domains: Vec<NominalIdentityDomainPolicy>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NominalIdentityDomainPolicy {
    pub domain_id: String,
    pub category: String,
    pub algorithm: String,
    pub required_prefix: String,
    pub max_bytes: usize,
    pub rust_symbols: Vec<String>,
    pub core_scopes: Vec<String>,
    pub wire_compatibility_scopes: Vec<String>,
    pub raw_representation: String,
    pub wrapper_kind: String,
    pub constructor_policy: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NominalIdentityOctetProjection {
    pub schema_version: String,
    pub policy_id: String,
    pub default_mode: String,
    pub heuristic_suffixes: Vec<String>,
    pub domains: Vec<NominalIdentityOctetDomain>,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NominalIdentityOctetDomain {
    pub domain_id: String,
    pub symbols: Vec<String>,
    pub raw_representation: String,
    pub core_scopes: Vec<String>,
    pub compatibility_scopes: Vec<String>,
    pub compatibility_required: bool,
    pub wrapper_kind: String,
    pub constructor_policy: String,
}

#[must_use]
pub fn nominal_identity_policy_diagnostics(policy: &NominalIdentityPolicy) -> Vec<IdentityIssue> {
    let mut issues = Vec::new();
    validate_headers(policy, &mut issues);
    validate_runtime_domains(&policy.runtime.domains, &mut issues);
    validate_octet_domains(&policy.octet, &mut issues);
    validate_projection_parity(policy, &mut issues);
    validate_non_claims(policy, &mut issues);
    issues.sort();
    issues.dedup();
    issues
}

#[must_use]
pub fn nominal_runtime_projection_hash(policy: &NominalIdentityPolicy) -> String {
    let value = serde_json::json!(&policy.runtime);
    canonical_json_hash(&value)
}

#[must_use]
pub fn nominal_octet_projection_hash(policy: &NominalIdentityPolicy) -> String {
    let value = serde_json::json!(&policy.octet);
    canonical_json_hash(&value)
}

fn validate_headers(policy: &NominalIdentityPolicy, issues: &mut Vec<IdentityIssue>) {
    if policy.policy_id != NOMINAL_IDENTITY_POLICY_ID
        || policy.runtime.policy_id != policy.policy_id
        || policy.octet.policy_id != policy.policy_id
    {
        issues.push(IdentityIssue::new(
            FIELD_POLICY_ID,
            "policy_identity_mismatch",
        ));
    }
    if policy.runtime.schema_version != NOMINAL_IDENTITY_RUNTIME_SCHEMA_V1 {
        issues.push(IdentityIssue::new(
            FIELD_RUNTIME,
            "runtime_schema_unsupported",
        ));
    }
    if policy.octet.schema_version != NOMINAL_IDENTITY_OCTET_SCHEMA_V1 {
        issues.push(IdentityIssue::new(FIELD_OCTET, "octet_schema_unsupported"));
    }
    if policy.octet.default_mode != "deny" {
        issues.push(IdentityIssue::new(FIELD_OCTET, "octet_mode_not_deny"));
    }
}

fn validate_runtime_domains(
    domains: &[NominalIdentityDomainPolicy],
    issues: &mut Vec<IdentityIssue>,
) {
    if domains.is_empty() || domains.len() > MAX_DOMAIN_COUNT {
        issues.push(IdentityIssue::new(
            FIELD_RUNTIME,
            "runtime_domain_count_invalid",
        ));
        return;
    }
    let mut domain_ids = BTreeSet::new();
    let mut symbols = BTreeSet::<String>::new();
    let mut previous = None;
    for domain in domains {
        if !domain_ids.insert(domain.domain_id.as_str()) {
            issues.push(IdentityIssue::new(
                FIELD_RUNTIME,
                "runtime_domain_duplicate",
            ));
        }
        if previous.is_some_and(|value: &str| value >= domain.domain_id.as_str()) {
            issues.push(IdentityIssue::new(
                FIELD_RUNTIME,
                "runtime_domains_not_sorted",
            ));
        }
        previous = Some(domain.domain_id.as_str());
        validate_runtime_domain(domain, &mut symbols, issues);
    }
}

fn validate_runtime_domain(
    domain: &NominalIdentityDomainPolicy,
    symbols: &mut BTreeSet<String>,
    issues: &mut Vec<IdentityIssue>,
) {
    if domain.domain_id.is_empty()
        || !SUPPORTED_CATEGORIES.contains(&domain.category.as_str())
        || !SUPPORTED_ALGORITHMS.contains(&domain.algorithm.as_str())
        || domain.max_bytes == 0
        || domain.raw_representation != "string"
        || !SUPPORTED_WRAPPERS.contains(&domain.wrapper_kind.as_str())
        || !SUPPORTED_CONSTRUCTORS.contains(&domain.constructor_policy.as_str())
    {
        issues.push(IdentityIssue::new(FIELD_RUNTIME, "runtime_domain_invalid"));
    }
    if domain.rust_symbols.is_empty()
        || domain.rust_symbols.len() > MAX_DOMAIN_SYMBOL_COUNT
        || domain.core_scopes.is_empty()
        || domain.core_scopes.len() > MAX_DOMAIN_SCOPE_COUNT
        || domain.wire_compatibility_scopes.len() > MAX_DOMAIN_SCOPE_COUNT
    {
        issues.push(IdentityIssue::new(
            FIELD_RUNTIME,
            "runtime_domain_bounds_invalid",
        ));
    }
    for symbol in &domain.rust_symbols {
        if symbol.is_empty() || !symbols.insert(symbol.clone()) {
            issues.push(IdentityIssue::new(FIELD_RUNTIME, "runtime_symbol_invalid"));
        }
    }
    let expected_algorithm = match domain.category.as_str() {
        "blake3-digest" => "BLAKE3",
        "imported-digest" => domain.algorithm.as_str(),
        _ => "none",
    };
    if domain.algorithm != expected_algorithm {
        issues.push(IdentityIssue::new(
            FIELD_RUNTIME,
            "runtime_algorithm_mismatch",
        ));
    }
    if domain.wrapper_kind == "refined-newtype" && domain.constructor_policy != "private-fallible" {
        issues.push(IdentityIssue::new(
            FIELD_RUNTIME,
            "runtime_constructor_mismatch",
        ));
    }
    if domain.wrapper_kind == "marker-generic" && domain.constructor_policy != "marker-only" {
        issues.push(IdentityIssue::new(
            FIELD_RUNTIME,
            "runtime_constructor_mismatch",
        ));
    }
}

fn validate_octet_domains(octet: &NominalIdentityOctetProjection, issues: &mut Vec<IdentityIssue>) {
    if octet.domains.is_empty() || octet.domains.len() > MAX_DOMAIN_COUNT {
        issues.push(IdentityIssue::new(
            FIELD_OCTET,
            "octet_domain_count_invalid",
        ));
    }
    if octet.heuristic_suffixes.is_empty() {
        issues.push(IdentityIssue::new(FIELD_OCTET, "octet_suffixes_empty"));
    }
    let mut domain_ids = BTreeSet::new();
    for domain in &octet.domains {
        if domain.domain_id.is_empty() || !domain_ids.insert(domain.domain_id.as_str()) {
            issues.push(IdentityIssue::new(FIELD_OCTET, "octet_domain_invalid"));
        }
        if domain.symbols.is_empty()
            || domain.core_scopes.is_empty()
            || domain.raw_representation != "string"
            || !domain.compatibility_required
            || domain.compatibility_scopes.is_empty()
        {
            issues.push(IdentityIssue::new(FIELD_OCTET, "octet_domain_invalid"));
        }
    }
}

fn validate_projection_parity(policy: &NominalIdentityPolicy, issues: &mut Vec<IdentityIssue>) {
    if policy.runtime.domains.len() != policy.octet.domains.len() {
        issues.push(IdentityIssue::new(
            FIELD_OCTET,
            "projection_domain_count_mismatch",
        ));
        return;
    }
    for runtime in &policy.runtime.domains {
        let Some(octet) = policy
            .octet
            .domains
            .iter()
            .find(|candidate| candidate.domain_id == runtime.domain_id)
        else {
            issues.push(IdentityIssue::new(FIELD_OCTET, "projection_domain_missing"));
            continue;
        };
        if octet.symbols != runtime.rust_symbols
            || octet.raw_representation != runtime.raw_representation
            || octet.core_scopes != runtime.core_scopes
            || octet.compatibility_scopes != runtime.wire_compatibility_scopes
            || octet.wrapper_kind != runtime.wrapper_kind
            || octet.constructor_policy != runtime.constructor_policy
        {
            issues.push(IdentityIssue::new(
                FIELD_OCTET,
                "projection_domain_mismatch",
            ));
        }
    }
}

fn validate_non_claims(policy: &NominalIdentityPolicy, issues: &mut Vec<IdentityIssue>) {
    if !policy
        .non_claims
        .iter()
        .any(|value| value == NOMINAL_IDENTITY_REQUIRED_NON_CLAIM)
        || !policy
            .octet
            .non_claims
            .iter()
            .any(|value| value == NOMINAL_IDENTITY_REQUIRED_NON_CLAIM)
    {
        issues.push(IdentityIssue::new(FIELD_NON_CLAIMS, "non_claim_missing"));
    }
}
