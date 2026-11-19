use std::collections::{BTreeMap, BTreeSet};

use super::identity::{has_exact_non_claims, policy_identity, valid_public_text};
use super::policy_model::IndexedBinding;
use super::{
    MAX_AUTHENTICATORS_PER_SUBJECT, MAX_DISPLAY_LABEL_BYTES, MAX_FIELD_BYTES, MAX_SUBJECTS,
    NOSTR_IDENTITY_POLICY_SCHEMA, NostrIdentityIssue, NostrIdentityPolicy,
    NostrIdentityPolicyError, ValidatedNostrIdentityPolicy,
};

/// Parse and validate a closed Nickel-exported policy.
///
/// # Errors
///
/// Returns a decode error for unknown, duplicate, missing, or mistyped fields.
/// Returns deterministic issues for invalid policy invariants.
// r[impl nostr_identity_bindings.policy_contract]
pub fn parse_nostr_identity_policy(
    input: &str,
) -> Result<ValidatedNostrIdentityPolicy, NostrIdentityPolicyError> {
    let policy: NostrIdentityPolicy = serde_json::from_str(input)
        .map_err(|error| NostrIdentityPolicyError::Decode(error.to_string()))?;
    validate_nostr_identity_policy(policy)
}

/// Validate a loaded policy and build an order-independent key index.
///
/// # Errors
///
/// Returns all deterministic structural issues and no partial index.
// r[impl nostr_identity_bindings.policy_contract]
// r[impl nostr_identity_bindings.subject_resolution]
pub fn validate_nostr_identity_policy(
    policy: NostrIdentityPolicy,
) -> Result<ValidatedNostrIdentityPolicy, NostrIdentityPolicyError> {
    let mut issues = Vec::new();
    if policy.schema != NOSTR_IDENTITY_POLICY_SCHEMA {
        issues.push(NostrIdentityIssue::new("schema", "schema-mismatch"));
    }
    if !valid_public_text(&policy.policy_revision, MAX_FIELD_BYTES) {
        issues.push(NostrIdentityIssue::new(
            "policy_revision",
            "invalid-policy-revision",
        ));
    }
    if policy.subjects.is_empty() || policy.subjects.len() > MAX_SUBJECTS {
        issues.push(NostrIdentityIssue::new(
            "subjects",
            "subject-count-out-of-bounds",
        ));
    }
    if !has_exact_non_claims(&policy.non_claims) {
        issues.push(NostrIdentityIssue::new(
            "non_claims",
            "required-non-claims-mismatch",
        ));
    }

    let bindings_by_key = index_policy_bindings(&policy, &mut issues);

    issues.sort();
    issues.dedup();
    if !issues.is_empty() {
        return Err(NostrIdentityPolicyError::Invalid(issues));
    }

    let policy_identity = policy_identity(&policy);
    Ok(ValidatedNostrIdentityPolicy {
        policy,
        policy_identity,
        bindings_by_key,
    })
}

fn index_policy_bindings(
    policy: &NostrIdentityPolicy,
    issues: &mut Vec<NostrIdentityIssue>,
) -> BTreeMap<String, IndexedBinding> {
    let mut subject_ids = BTreeSet::new();
    let mut binding_ids = BTreeSet::new();
    let mut bindings_by_key = BTreeMap::new();
    for subject in &policy.subjects {
        if !valid_public_text(&subject.subject_id, MAX_FIELD_BYTES) {
            issues.push(NostrIdentityIssue::new(
                "subjects.subject_id",
                "invalid-subject-id",
            ));
        }
        if !subject_ids.insert(subject.subject_id.clone()) {
            issues.push(NostrIdentityIssue::new(
                "subjects.subject_id",
                "duplicate-subject-id",
            ));
        }
        if subject
            .display_label
            .as_ref()
            .is_some_and(|label| !valid_public_text(label, MAX_DISPLAY_LABEL_BYTES))
        {
            issues.push(NostrIdentityIssue::new(
                "subjects.display_label",
                "invalid-public-metadata",
            ));
        }
        if subject.authenticators.is_empty()
            || subject.authenticators.len() > MAX_AUTHENTICATORS_PER_SUBJECT
        {
            issues.push(NostrIdentityIssue::new(
                "subjects.authenticators",
                "authenticator-count-out-of-bounds",
            ));
        }
        index_subject_authenticators(subject, &mut binding_ids, &mut bindings_by_key, issues);
    }
    bindings_by_key
}

fn index_subject_authenticators(
    subject: &super::NostrSubjectPolicy,
    binding_ids: &mut BTreeSet<String>,
    bindings_by_key: &mut BTreeMap<String, IndexedBinding>,
    issues: &mut Vec<NostrIdentityIssue>,
) {
    for authenticator in &subject.authenticators {
        if !valid_public_text(&authenticator.binding_id, MAX_FIELD_BYTES) {
            issues.push(NostrIdentityIssue::new(
                "subjects.authenticators.binding_id",
                "invalid-binding-id",
            ));
        }
        if !binding_ids.insert(authenticator.binding_id.clone()) {
            issues.push(NostrIdentityIssue::new(
                "subjects.authenticators.binding_id",
                "duplicate-binding-id",
            ));
        }
        if ucan_nostr::NostrPublicKey::parse(&authenticator.public_key).is_err() {
            issues.push(NostrIdentityIssue::new(
                "subjects.authenticators.public_key",
                "invalid-nostr-public-key",
            ));
            continue;
        }
        let indexed = IndexedBinding {
            subject_id: subject.subject_id.clone(),
            binding_id: authenticator.binding_id.clone(),
            public_key: authenticator.public_key.clone(),
            state: authenticator.state,
        };
        if bindings_by_key
            .insert(authenticator.public_key.clone(), indexed)
            .is_some()
        {
            issues.push(NostrIdentityIssue::new(
                "subjects.authenticators.public_key",
                "ambiguous-nostr-public-key",
            ));
        }
    }
}
