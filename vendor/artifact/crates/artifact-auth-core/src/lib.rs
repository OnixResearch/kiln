#![no_std]
//! Product-neutral, deterministic artifact-authentication statements and policy decisions.
//!
//! The core consumes explicit in-memory values. It owns no filesystem, environment,
//! process, network, clock, randomness, private-key, trust-discovery, persistence,
//! authorization, lifecycle, build, or release authority.

// r[impl artifact_auth.core.authority_free]
// r[impl artifact_auth.statement.canonical]
// r[impl artifact_auth.policy.explicit]
// r[impl artifact_auth.claims.bounded]
extern crate alloc;

use alloc::collections::BTreeSet;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

/// Canonical statement schema.
pub const STATEMENT_SCHEMA_V1: &str = "artifact-auth.statement.v1";
/// Authentication policy schema.
pub const POLICY_SCHEMA_V1: &str = "artifact-auth.policy.v1";
/// Canonical preimage domain.
pub const PREIMAGE_DOMAIN_V1: &str = "artifact-auth.statement-preimage.v1";
/// BLAKE3 algorithm label.
pub const ALGORITHM_BLAKE3: &str = "blake3";
/// SHA-256 interoperability algorithm label.
pub const ALGORITHM_SHA256: &str = "sha256";
/// Ed25519 signature algorithm label.
pub const ALGORITHM_ED25519: &str = "ed25519";
/// Public-key artifact profile.
pub const ED25519_PUBLIC_KEY_PROFILE_V1: &str = "ed25519-public-key.v1";
/// Maximum text field size.
pub const MAX_TEXT_BYTES: usize = 256;
/// Maximum parent-reference count.
pub const MAX_PARENT_REFS: usize = 64;
/// Maximum trusted-key count.
pub const MAX_TRUSTED_KEYS: usize = 64;
/// Maximum supplied signature-evidence count.
pub const MAX_SIGNATURE_EVIDENCE: usize = 64;
/// Lowercase hexadecimal characters in BLAKE3 and SHA-256 digests.
pub const DIGEST_HEX_CHARS: usize = 64;

const LENGTH_PREFIX_BYTES_MAX: usize = u32::MAX as usize;
const ADJACENT_PAIR_WINDOW: usize = 2;
const REQUIRED_NON_CLAIM_COUNT: usize = 11;
const REQUIRED_NON_CLAIMS: [&str; REQUIRED_NON_CLAIM_COUNT] = [
    "authentication-does-not-prove-artifact-truth",
    "authentication-does-not-prove-verifier-correctness",
    "authentication-does-not-prove-producer-authorization",
    "authentication-does-not-prove-trust-root-correctness",
    "authentication-does-not-prove-revocation-freshness",
    "authentication-does-not-prove-provenance-correctness",
    "authentication-does-not-prove-lifecycle-conformance",
    "authentication-does-not-prove-build-correctness",
    "authentication-does-not-prove-semantic-equivalence",
    "authentication-does-not-prove-deployment-safety",
    "authentication-does-not-prove-release-eligibility",
];

/// A typed immutable artifact reference.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct ArtifactRef {
    /// Consumer- or protocol-owned artifact profile.
    pub profile: String,
    /// Explicit digest algorithm label.
    pub algorithm: String,
    /// Lowercase hexadecimal digest bytes.
    pub digest_hex: String,
}

/// Product-neutral fields shared by every signer-specific statement.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct AuthenticationScope {
    /// Signature domain.
    pub domain: String,
    /// Signature purpose.
    pub purpose: String,
    /// Compatibility profile identity.
    pub profile_id: String,
    /// Exact subject reference.
    pub subject: ArtifactRef,
    /// Exact parent references; input order is not semantic.
    pub parents: Vec<ArtifactRef>,
    /// Exact verifier-context reference.
    pub verifier_context: ArtifactRef,
}

/// A signer-specific, versioned statement.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct ArtifactStatement {
    /// Statement schema.
    pub schema: String,
    /// Shared authentication scope.
    pub scope: AuthenticationScope,
    /// Product-supplied producer identity.
    pub producer_id: String,
    /// Product-supplied key label.
    pub key_id: String,
    /// Full public-key identity.
    pub key_identity: ArtifactRef,
}

/// Key currentness supplied by a consumer-owned observation source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum KeyCurrentness {
    /// Current for signing and verification.
    Current,
    /// Bounded verification-only rotation overlap.
    VerificationOverlap,
    /// Superseded and no longer admitted.
    Superseded,
    /// Explicitly revoked.
    Revoked,
    /// Currentness was not established.
    Unknown,
}

impl KeyCurrentness {
    const fn permits_verification(self) -> bool {
        matches!(self, Self::Current | Self::VerificationOverlap)
    }
}

/// Explicit trusted-key and currentness observation supplied by a consumer.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct TrustedKeyObservation {
    /// Producer identity bound by statements from this key.
    pub producer_id: String,
    /// Display or routing label bound by statements from this key.
    pub key_id: String,
    /// Full public-key identity used for matching and threshold counting.
    pub key_identity: ArtifactRef,
    /// Purposes admitted for this key.
    pub allowed_purposes: Vec<String>,
    /// Exact admitted key generation.
    pub generation: u64,
    /// Explicit currentness classification.
    pub currentness: KeyCurrentness,
    /// Identity of the supplied currentness observation.
    pub currentness_ref: ArtifactRef,
}

/// Product-neutral authentication policy.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct AuthenticationPolicy {
    /// Policy schema.
    pub schema: String,
    /// Consumer-selected compatibility profile.
    pub profile_id: String,
    /// Minimum distinct verified full-key identities.
    pub threshold: u16,
    /// Explicit trusted-key observations.
    pub trusted_keys: Vec<TrustedKeyObservation>,
}

/// Pure cryptographic verification observation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct CryptographicObservation {
    /// Signature algorithm.
    pub algorithm: String,
    /// Full public-key identity derived from supplied key bytes.
    pub key_identity: ArtifactRef,
    /// Whether verification succeeded.
    pub verified: bool,
    /// Stable verifier failure class, absent on success.
    pub failure_code: Option<String>,
}

/// One signer-specific statement and its supplied observations.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct SignatureEvidence {
    /// Exact statement that was verified.
    pub statement: ArtifactStatement,
    /// Key generation claimed by the signature carrier.
    pub generation: u64,
    /// Pure cryptographic observation.
    pub cryptographic: CryptographicObservation,
}

/// Stable policy or statement issue.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct AuthenticationIssue {
    /// Stable issue code.
    pub code: String,
    /// Stable field path.
    pub field: String,
}

/// Deterministic bounded authentication decision.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct AuthenticationDecision {
    /// Whether all statement, cryptographic, currentness, and threshold checks passed.
    pub passed: bool,
    /// Policy identity.
    pub policy_blake3: String,
    /// Shared-scope identity.
    pub scope_blake3: String,
    /// Sorted distinct verified full-key identities.
    pub verified_key_blake3: Vec<String>,
    /// Sorted stable issues.
    pub issues: Vec<AuthenticationIssue>,
    /// Required bounded non-claims.
    pub non_claims: Vec<String>,
    /// Identity of the full decision.
    pub decision_blake3: String,
}

/// Build canonical signer-specific statement bytes.
///
/// The encoding is schema-owned, versioned, and length-delimited. Product JSON,
/// Preserves, OCI, Radicle, and lifecycle codecs are not canonical here.
pub fn canonical_statement_bytes(
    statement: &ArtifactStatement,
) -> Result<Vec<u8>, Vec<AuthenticationIssue>> {
    let mut issues = validate_statement(statement);
    sort_issues(&mut issues);
    if !issues.is_empty() {
        return Err(issues);
    }
    let mut parents = statement.scope.parents.clone();
    parents.sort();
    assert!(
        is_strictly_sorted(&parents),
        "validated parents must be sorted and unique"
    );

    let mut output = Vec::new();
    push_text(&mut output, PREIMAGE_DOMAIN_V1)?;
    push_text(&mut output, &statement.schema)?;
    push_text(&mut output, &statement.scope.domain)?;
    push_text(&mut output, &statement.scope.purpose)?;
    push_text(&mut output, &statement.scope.profile_id)?;
    push_text(&mut output, &statement.producer_id)?;
    push_text(&mut output, &statement.key_id)?;
    push_ref(&mut output, &statement.key_identity)?;
    push_ref(&mut output, &statement.scope.subject)?;
    push_count(&mut output, parents.len())?;
    for parent in &parents {
        push_ref(&mut output, parent)?;
    }
    push_ref(&mut output, &statement.scope.verifier_context)?;
    Ok(output)
}

/// Compute the BLAKE3 identity of canonical statement bytes.
pub fn statement_identity(
    statement: &ArtifactStatement,
) -> Result<String, Vec<AuthenticationIssue>> {
    canonical_statement_bytes(statement).map(|bytes| blake3_hex(&bytes))
}

/// Compute the normalized BLAKE3 identity of a supplied policy.
pub fn policy_identity(policy: &AuthenticationPolicy) -> Result<String, Vec<AuthenticationIssue>> {
    let mut issues = validate_policy(policy);
    sort_issues(&mut issues);
    if !issues.is_empty() {
        return Err(issues);
    }
    let mut keys = policy.trusted_keys.clone();
    keys.sort_by(|left, right| {
        left.key_identity
            .cmp(&right.key_identity)
            .then(left.producer_id.cmp(&right.producer_id))
            .then(left.key_id.cmp(&right.key_id))
    });
    let mut output = Vec::new();
    push_text(&mut output, POLICY_SCHEMA_V1)?;
    push_text(&mut output, &policy.profile_id)?;
    push_text(&mut output, &policy.threshold.to_string())?;
    push_count(&mut output, keys.len())?;
    for key in &keys {
        push_text(&mut output, &key.producer_id)?;
        push_text(&mut output, &key.key_id)?;
        push_ref(&mut output, &key.key_identity)?;
        let mut purposes = key.allowed_purposes.clone();
        purposes.sort();
        push_count(&mut output, purposes.len())?;
        for purpose in &purposes {
            push_text(&mut output, purpose)?;
        }
        push_text(&mut output, &key.generation.to_string())?;
        push_text(&mut output, currentness_label(key.currentness))?;
        push_ref(&mut output, &key.currentness_ref)?;
    }
    Ok(blake3_hex(&output))
}

/// Evaluate explicit statements and observations against explicit policy.
#[must_use]
pub fn evaluate_authentication(
    policy: &AuthenticationPolicy,
    scope: &AuthenticationScope,
    evidence: &[SignatureEvidence],
) -> AuthenticationDecision {
    let mut issues = validate_policy(policy);
    validate_scope(scope, "scope", &mut issues);
    if policy.profile_id != scope.profile_id {
        issues.push(issue("auth.policy_profile_mismatch", "policy.profile_id"));
    }
    if evidence.is_empty() {
        issues.push(issue("auth.signature_evidence_missing", "evidence"));
    }
    if evidence.len() > MAX_SIGNATURE_EVIDENCE {
        issues.push(issue("auth.signature_evidence_count_exceeded", "evidence"));
    }

    let mut ordered = evidence.to_vec();
    ordered.sort_by(|left, right| {
        left.statement
            .key_identity
            .cmp(&right.statement.key_identity)
            .then(left.statement.producer_id.cmp(&right.statement.producer_id))
            .then(left.statement.key_id.cmp(&right.statement.key_id))
    });
    let mut seen_evidence_keys = BTreeSet::new();
    let mut verified_keys = BTreeSet::new();
    for (index, item) in ordered.iter().enumerate() {
        let field = alloc::format!("evidence[{index}]");
        let issue_count_before = issues.len();
        append_prefixed_issues(&mut issues, &field, validate_statement(&item.statement));
        if !scope_equivalent(&item.statement.scope, scope) {
            issues.push(issue(
                "auth.statement_scope_mismatch",
                &alloc::format!("{field}.statement.scope"),
            ));
        }
        if item.cryptographic.algorithm != ALGORITHM_ED25519 {
            issues.push(issue(
                "auth.signature_algorithm_unsupported",
                &alloc::format!("{field}.cryptographic.algorithm"),
            ));
        }
        if item.cryptographic.key_identity != item.statement.key_identity {
            issues.push(issue(
                "auth.cryptographic_key_identity_mismatch",
                &alloc::format!("{field}.cryptographic.key_identity"),
            ));
        }
        if !item.cryptographic.verified {
            issues.push(issue(
                "auth.signature_unverified",
                &alloc::format!("{field}.cryptographic.verified"),
            ));
        }
        if item.cryptographic.failure_code.is_some() {
            issues.push(issue(
                "auth.cryptographic_failure_present",
                &alloc::format!("{field}.cryptographic.failure_code"),
            ));
        }
        let key_digest = item.statement.key_identity.digest_hex.clone();
        if !seen_evidence_keys.insert(key_digest.clone()) {
            issues.push(issue(
                "auth.duplicate_key_evidence",
                &alloc::format!("{field}.statement.key_identity"),
            ));
        }
        let trusted = policy.trusted_keys.iter().find(|candidate| {
            candidate.key_identity == item.statement.key_identity
                && candidate.producer_id == item.statement.producer_id
                && candidate.key_id == item.statement.key_id
        });
        let Some(trusted) = trusted else {
            issues.push(issue(
                "auth.key_untrusted",
                &alloc::format!("{field}.statement.key_identity"),
            ));
            continue;
        };
        if item.generation == 0 || item.generation != trusted.generation {
            issues.push(issue(
                "auth.key_generation_stale",
                &alloc::format!("{field}.generation"),
            ));
        }
        if !trusted.currentness.permits_verification() {
            issues.push(issue(
                "auth.key_not_current",
                &alloc::format!("{field}.currentness"),
            ));
        }
        if !trusted.allowed_purposes.contains(&scope.purpose) {
            issues.push(issue(
                "auth.purpose_unauthorized",
                &alloc::format!("{field}.statement.scope.purpose"),
            ));
        }
        if issues.len() == issue_count_before {
            verified_keys.insert(key_digest);
        }
    }
    if verified_keys.len() < usize::from(policy.threshold) {
        issues.push(issue(
            "auth.distinct_key_threshold_unsatisfied",
            "policy.threshold",
        ));
    }
    sort_issues(&mut issues);

    let policy_blake3 =
        policy_identity(policy).unwrap_or_else(|_| blake3_hex(POLICY_SCHEMA_V1.as_bytes()));
    let scope_blake3 =
        scope_identity(scope).unwrap_or_else(|_| blake3_hex(STATEMENT_SCHEMA_V1.as_bytes()));
    let verified_key_blake3 = verified_keys.into_iter().collect::<Vec<_>>();
    let non_claims = required_non_claims();
    let decision_blake3 = decision_identity(
        &policy_blake3,
        &scope_blake3,
        &verified_key_blake3,
        &issues,
        &non_claims,
    );
    AuthenticationDecision {
        passed: issues.is_empty(),
        policy_blake3,
        scope_blake3,
        verified_key_blake3,
        issues,
        non_claims,
        decision_blake3,
    }
}

/// Return the mandatory bounded non-claims in canonical order.
#[must_use]
pub fn required_non_claims() -> Vec<String> {
    let mut claims = REQUIRED_NON_CLAIMS
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    claims.sort();
    assert!(
        is_strictly_sorted(&claims),
        "required non-claims must be unique"
    );
    claims
}

fn validate_statement(statement: &ArtifactStatement) -> Vec<AuthenticationIssue> {
    let mut issues = Vec::new();
    if statement.schema != STATEMENT_SCHEMA_V1 {
        issues.push(issue(
            "auth.statement_schema_unsupported",
            "statement.schema",
        ));
    }
    validate_scope(&statement.scope, "statement.scope", &mut issues);
    validate_text(&statement.producer_id, "statement.producer_id", &mut issues);
    validate_text(&statement.key_id, "statement.key_id", &mut issues);
    validate_key_identity(
        &statement.key_identity,
        "statement.key_identity",
        &mut issues,
    );
    issues
}

fn validate_scope(scope: &AuthenticationScope, field: &str, issues: &mut Vec<AuthenticationIssue>) {
    validate_text(&scope.domain, &alloc::format!("{field}.domain"), issues);
    validate_text(&scope.purpose, &alloc::format!("{field}.purpose"), issues);
    validate_text(
        &scope.profile_id,
        &alloc::format!("{field}.profile_id"),
        issues,
    );
    validate_ref(&scope.subject, &alloc::format!("{field}.subject"), issues);
    validate_ref(
        &scope.verifier_context,
        &alloc::format!("{field}.verifier_context"),
        issues,
    );
    if scope.parents.len() > MAX_PARENT_REFS {
        issues.push(issue(
            "auth.parent_count_exceeded",
            &alloc::format!("{field}.parents"),
        ));
    }
    let mut parents = scope.parents.clone();
    for (index, parent) in parents.iter().enumerate() {
        validate_ref(parent, &alloc::format!("{field}.parents[{index}]"), issues);
    }
    parents.sort();
    if parents
        .windows(ADJACENT_PAIR_WINDOW)
        .any(|pair| pair[0] == pair[1])
    {
        issues.push(issue(
            "auth.parent_duplicate",
            &alloc::format!("{field}.parents"),
        ));
    }
}

fn validate_policy(policy: &AuthenticationPolicy) -> Vec<AuthenticationIssue> {
    let mut issues = Vec::new();
    if policy.schema != POLICY_SCHEMA_V1 {
        issues.push(issue("auth.policy_schema_unsupported", "policy.schema"));
    }
    validate_text(&policy.profile_id, "policy.profile_id", &mut issues);
    if policy.threshold == 0 {
        issues.push(issue("auth.threshold_zero", "policy.threshold"));
    }
    if policy.trusted_keys.is_empty() || policy.trusted_keys.len() > MAX_TRUSTED_KEYS {
        issues.push(issue(
            "auth.trusted_key_count_invalid",
            "policy.trusted_keys",
        ));
    }
    if usize::from(policy.threshold) > policy.trusted_keys.len() {
        issues.push(issue(
            "auth.threshold_exceeds_key_count",
            "policy.threshold",
        ));
    }
    let mut identities = Vec::new();
    for (index, key) in policy.trusted_keys.iter().enumerate() {
        let field = alloc::format!("policy.trusted_keys[{index}]");
        validate_text(
            &key.producer_id,
            &alloc::format!("{field}.producer_id"),
            &mut issues,
        );
        validate_text(&key.key_id, &alloc::format!("{field}.key_id"), &mut issues);
        validate_key_identity(
            &key.key_identity,
            &alloc::format!("{field}.key_identity"),
            &mut issues,
        );
        validate_ref(
            &key.currentness_ref,
            &alloc::format!("{field}.currentness_ref"),
            &mut issues,
        );
        if key.allowed_purposes.is_empty() {
            issues.push(issue(
                "auth.allowed_purpose_missing",
                &alloc::format!("{field}.allowed_purposes"),
            ));
        }
        let mut purposes = key.allowed_purposes.clone();
        for (purpose_index, purpose) in purposes.iter().enumerate() {
            validate_text(
                purpose,
                &alloc::format!("{field}.allowed_purposes[{purpose_index}]"),
                &mut issues,
            );
        }
        purposes.sort();
        if purposes
            .windows(ADJACENT_PAIR_WINDOW)
            .any(|pair| pair[0] == pair[1])
        {
            issues.push(issue(
                "auth.allowed_purpose_duplicate",
                &alloc::format!("{field}.allowed_purposes"),
            ));
        }
        if key.generation == 0 {
            issues.push(issue(
                "auth.key_generation_zero",
                &alloc::format!("{field}.generation"),
            ));
        }
        identities.push(key.key_identity.clone());
    }
    identities.sort();
    if identities
        .windows(ADJACENT_PAIR_WINDOW)
        .any(|pair| pair[0] == pair[1])
    {
        issues.push(issue(
            "auth.trusted_key_identity_duplicate",
            "policy.trusted_keys",
        ));
    }
    issues
}

fn validate_key_identity(value: &ArtifactRef, field: &str, issues: &mut Vec<AuthenticationIssue>) {
    validate_ref(value, field, issues);
    if value.algorithm != ALGORITHM_BLAKE3 || value.profile != ED25519_PUBLIC_KEY_PROFILE_V1 {
        issues.push(issue("auth.key_identity_profile_unsupported", field));
    }
}

fn validate_ref(value: &ArtifactRef, field: &str, issues: &mut Vec<AuthenticationIssue>) {
    validate_text(&value.profile, &alloc::format!("{field}.profile"), issues);
    if !matches!(
        value.algorithm.as_str(),
        ALGORITHM_BLAKE3 | ALGORITHM_SHA256
    ) {
        issues.push(issue(
            "auth.digest_algorithm_unsupported",
            &alloc::format!("{field}.algorithm"),
        ));
    }
    if value.digest_hex.len() != DIGEST_HEX_CHARS || !is_lower_hex(&value.digest_hex) {
        issues.push(issue(
            "auth.digest_malformed",
            &alloc::format!("{field}.digest_hex"),
        ));
    }
}

fn validate_text(value: &str, field: &str, issues: &mut Vec<AuthenticationIssue>) {
    if value.is_empty() || value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        issues.push(issue("auth.text_invalid", field));
    }
}

fn scope_equivalent(left: &AuthenticationScope, right: &AuthenticationScope) -> bool {
    let mut left_parents = left.parents.clone();
    let mut right_parents = right.parents.clone();
    left_parents.sort();
    right_parents.sort();
    left.domain == right.domain
        && left.purpose == right.purpose
        && left.profile_id == right.profile_id
        && left.subject == right.subject
        && left_parents == right_parents
        && left.verifier_context == right.verifier_context
}

fn scope_identity(scope: &AuthenticationScope) -> Result<String, Vec<AuthenticationIssue>> {
    let statement = ArtifactStatement {
        schema: STATEMENT_SCHEMA_V1.to_string(),
        scope: scope.clone(),
        producer_id: "scope-identity".to_string(),
        key_id: "scope-identity".to_string(),
        key_identity: ArtifactRef {
            profile: ED25519_PUBLIC_KEY_PROFILE_V1.to_string(),
            algorithm: ALGORITHM_BLAKE3.to_string(),
            digest_hex: blake3_hex(b"artifact-auth.scope-identity.v1"),
        },
    };
    statement_identity(&statement)
}

fn decision_identity(
    policy_blake3: &str,
    scope_blake3: &str,
    verified_keys: &[String],
    issues: &[AuthenticationIssue],
    non_claims: &[String],
) -> String {
    let mut bytes = Vec::new();
    let operations = (|| -> Result<(), Vec<AuthenticationIssue>> {
        push_text(&mut bytes, "artifact-auth.decision.v1")?;
        push_text(&mut bytes, policy_blake3)?;
        push_text(&mut bytes, scope_blake3)?;
        push_count(&mut bytes, verified_keys.len())?;
        for key in verified_keys {
            push_text(&mut bytes, key)?;
        }
        push_count(&mut bytes, issues.len())?;
        for issue in issues {
            push_text(&mut bytes, &issue.code)?;
            push_text(&mut bytes, &issue.field)?;
        }
        push_count(&mut bytes, non_claims.len())?;
        for claim in non_claims {
            push_text(&mut bytes, claim)?;
        }
        Ok(())
    })();
    assert!(
        operations.is_ok(),
        "validated decision material must fit canonical bounds"
    );
    blake3_hex(&bytes)
}

fn push_ref(output: &mut Vec<u8>, value: &ArtifactRef) -> Result<(), Vec<AuthenticationIssue>> {
    push_text(output, &value.profile)?;
    push_text(output, &value.algorithm)?;
    push_text(output, &value.digest_hex)
}

fn push_count(output: &mut Vec<u8>, count: usize) -> Result<(), Vec<AuthenticationIssue>> {
    let value =
        u32::try_from(count).map_err(|_| vec![issue("auth.canonical_length_overflow", "count")])?;
    output.extend_from_slice(&value.to_be_bytes());
    Ok(())
}

fn push_text(output: &mut Vec<u8>, value: &str) -> Result<(), Vec<AuthenticationIssue>> {
    if value.len() > LENGTH_PREFIX_BYTES_MAX {
        return Err(vec![issue("auth.canonical_length_overflow", "text")]);
    }
    push_count(output, value.len())?;
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

fn append_prefixed_issues(
    destination: &mut Vec<AuthenticationIssue>,
    prefix: &str,
    source: Vec<AuthenticationIssue>,
) {
    destination.extend(source.into_iter().map(|item| AuthenticationIssue {
        code: item.code,
        field: alloc::format!("{prefix}.{}", item.field),
    }));
}

fn sort_issues(issues: &mut Vec<AuthenticationIssue>) {
    issues.sort();
    issues.dedup();
}

fn issue(code: &str, field: &str) -> AuthenticationIssue {
    AuthenticationIssue {
        code: code.to_string(),
        field: field.to_string(),
    }
}

fn currentness_label(value: KeyCurrentness) -> &'static str {
    match value {
        KeyCurrentness::Current => "current",
        KeyCurrentness::VerificationOverlap => "verification_overlap",
        KeyCurrentness::Superseded => "superseded",
        KeyCurrentness::Revoked => "revoked",
        KeyCurrentness::Unknown => "unknown",
    }
}

fn is_lower_hex(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_strictly_sorted<T: Ord>(values: &[T]) -> bool {
    values
        .windows(ADJACENT_PAIR_WINDOW)
        .all(|pair| pair[0] < pair[1])
}

fn blake3_hex(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    const GENERATION_ONE: u64 = 1;
    const THRESHOLD_ONE: u16 = 1;
    const THRESHOLD_TWO: u16 = 2;
    const SHA256_ZERO_HEX: &str =
        "0000000000000000000000000000000000000000000000000000000000000000";

    fn reference(profile: &str, seed: &str) -> ArtifactRef {
        ArtifactRef {
            profile: profile.to_string(),
            algorithm: ALGORITHM_BLAKE3.to_string(),
            digest_hex: blake3_hex(seed.as_bytes()),
        }
    }

    fn key(seed: &str) -> ArtifactRef {
        reference(ED25519_PUBLIC_KEY_PROFILE_V1, seed)
    }

    fn scope() -> AuthenticationScope {
        AuthenticationScope {
            domain: "onix.release-evidence.v1".to_string(),
            purpose: "release-evidence".to_string(),
            profile_id: "onix-artifact-auth.v1".to_string(),
            subject: reference("cairn-receipt.v1", "subject"),
            parents: vec![
                reference("policy.v1", "policy"),
                reference("graph.v1", "graph"),
            ],
            verifier_context: reference("verifier-context.v1", "context"),
        }
    }

    fn statement(
        scope: &AuthenticationScope,
        producer: &str,
        key_id: &str,
        key_identity: ArtifactRef,
    ) -> ArtifactStatement {
        ArtifactStatement {
            schema: STATEMENT_SCHEMA_V1.to_string(),
            scope: scope.clone(),
            producer_id: producer.to_string(),
            key_id: key_id.to_string(),
            key_identity,
        }
    }

    fn trusted(producer: &str, key_id: &str, key_identity: ArtifactRef) -> TrustedKeyObservation {
        TrustedKeyObservation {
            producer_id: producer.to_string(),
            key_id: key_id.to_string(),
            key_identity,
            allowed_purposes: vec!["release-evidence".to_string()],
            generation: GENERATION_ONE,
            currentness: KeyCurrentness::Current,
            currentness_ref: reference("currentness.v1", key_id),
        }
    }

    fn evidence(statement: ArtifactStatement) -> SignatureEvidence {
        SignatureEvidence {
            generation: GENERATION_ONE,
            cryptographic: CryptographicObservation {
                algorithm: ALGORITHM_ED25519.to_string(),
                key_identity: statement.key_identity.clone(),
                verified: true,
                failure_code: None,
            },
            statement,
        }
    }

    // r[verify artifact_auth.statement.canonical]
    #[test]
    fn canonical_statement_is_parent_order_independent_and_domain_bound() {
        let scope = scope();
        let left = statement(&scope, "cairn", "release-a", key("key-a"));
        let mut reversed_scope = scope.clone();
        reversed_scope.parents.reverse();
        let right = statement(&reversed_scope, "cairn", "release-a", key("key-a"));
        assert_eq!(
            canonical_statement_bytes(&left),
            canonical_statement_bytes(&right)
        );
        assert_eq!(statement_identity(&left), statement_identity(&right));

        let mut changed = right;
        changed.scope.domain = "different-domain".to_string();
        assert_ne!(statement_identity(&left), statement_identity(&changed));

        let mut interoperability_scope = scope;
        interoperability_scope.parents.push(ArtifactRef {
            profile: "oci-manifest.v1".to_string(),
            algorithm: ALGORITHM_SHA256.to_string(),
            digest_hex: SHA256_ZERO_HEX.to_string(),
        });
        let interoperability_statement = statement(
            &interoperability_scope,
            "mantle",
            "registry-key",
            key("registry-key"),
        );
        assert!(canonical_statement_bytes(&interoperability_statement).is_ok());
        assert!(statement_identity(&interoperability_statement).is_ok());
    }

    // r[verify artifact_auth.policy.explicit]
    // r[verify artifact_auth.claims.bounded]
    #[test]
    fn distinct_current_keys_satisfy_threshold() {
        let scope = scope();
        let key_a = key("key-a");
        let key_b = key("key-b");
        let policy = AuthenticationPolicy {
            schema: POLICY_SCHEMA_V1.to_string(),
            profile_id: scope.profile_id.clone(),
            threshold: THRESHOLD_TWO,
            trusted_keys: vec![
                trusted("cairn", "release-a", key_a.clone()),
                trusted("mantle", "release-b", key_b.clone()),
            ],
        };
        let evidence = vec![
            evidence(statement(&scope, "mantle", "release-b", key_b)),
            evidence(statement(&scope, "cairn", "release-a", key_a)),
        ];
        let decision = evaluate_authentication(&policy, &scope, &evidence);
        assert!(decision.passed, "{:?}", decision.issues);
        assert_eq!(
            decision.verified_key_blake3.len(),
            usize::from(THRESHOLD_TWO)
        );
        assert_eq!(decision.non_claims, required_non_claims());
    }

    // r[verify artifact_auth.policy.explicit]
    #[test]
    fn duplicate_key_cannot_inflate_threshold() {
        let scope = scope();
        let key_a = key("key-a");
        let mut second_trust = trusted("mantle", "same-material", key_a.clone());
        second_trust.currentness_ref = reference("currentness.v1", "same-material");
        let policy = AuthenticationPolicy {
            schema: POLICY_SCHEMA_V1.to_string(),
            profile_id: scope.profile_id.clone(),
            threshold: THRESHOLD_TWO,
            trusted_keys: vec![trusted("cairn", "release-a", key_a.clone()), second_trust],
        };
        let duplicate = evidence(statement(&scope, "cairn", "release-a", key_a));
        let decision = evaluate_authentication(&policy, &scope, &[duplicate.clone(), duplicate]);
        assert!(!decision.passed);
        assert!(
            decision
                .issues
                .iter()
                .any(|value| value.code == "auth.trusted_key_identity_duplicate")
        );
        assert!(
            decision
                .issues
                .iter()
                .any(|value| value.code == "auth.distinct_key_threshold_unsatisfied")
        );
    }

    #[test]
    fn revoked_stale_wrong_purpose_and_failed_crypto_fail_closed() {
        let scope = scope();
        let key_a = key("key-a");
        let invalid_cases = [
            (
                KeyCurrentness::Revoked,
                GENERATION_ONE,
                "release-evidence",
                true,
                "auth.key_not_current",
            ),
            (
                KeyCurrentness::Current,
                GENERATION_ONE + 1,
                "release-evidence",
                true,
                "auth.key_generation_stale",
            ),
            (
                KeyCurrentness::Current,
                GENERATION_ONE,
                "different-purpose",
                true,
                "auth.purpose_unauthorized",
            ),
            (
                KeyCurrentness::Current,
                GENERATION_ONE,
                "release-evidence",
                false,
                "auth.signature_unverified",
            ),
        ];
        for (currentness, generation, allowed, verified, expected) in invalid_cases {
            let mut trust = trusted("cairn", "release-a", key_a.clone());
            trust.currentness = currentness;
            trust.allowed_purposes = vec![allowed.to_string()];
            let policy = AuthenticationPolicy {
                schema: POLICY_SCHEMA_V1.to_string(),
                profile_id: scope.profile_id.clone(),
                threshold: THRESHOLD_ONE,
                trusted_keys: vec![trust],
            };
            let mut item = evidence(statement(&scope, "cairn", "release-a", key_a.clone()));
            item.generation = generation;
            item.cryptographic.verified = verified;
            if !verified {
                item.cryptographic.failure_code = Some("ed25519.signature_invalid".to_string());
            }
            let decision = evaluate_authentication(&policy, &scope, &[item]);
            assert!(!decision.passed, "case {expected}");
            assert!(
                decision.issues.iter().any(|value| value.code == expected),
                "{:?}",
                decision.issues
            );
        }
    }

    #[test]
    fn malformed_and_duplicate_references_are_rejected() {
        let mut scope = scope();
        scope.parents.push(scope.parents[0].clone());
        let mut value = statement(&scope, "cairn", "release-a", key("key-a"));
        value.scope.subject.algorithm = "unknown".to_string();
        let issues = canonical_statement_bytes(&value).expect_err("malformed statement must fail");
        assert!(
            issues
                .iter()
                .any(|item| item.code == "auth.parent_duplicate")
        );
        assert!(
            issues
                .iter()
                .any(|item| item.code == "auth.digest_algorithm_unsupported")
        );
    }
}
