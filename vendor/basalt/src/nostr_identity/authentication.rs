use ucan_nostr::VerifiedNostrCredential;

use super::authentication_model::CredentialFacts;
use super::identity::{
    authentication_profile_name, authentication_receipt_identity, binding_identity, check_digest,
    has_exact_non_claims, public_field_identity, required_non_claims, valid_digest,
    valid_public_text,
};
use super::{
    AuthenticatedNostrSubject, MAX_FIELD_BYTES, NOSTR_AUTHENTICATION_RECEIPT_SCHEMA,
    NostrAuthenticationDecision, NostrAuthenticationReceipt, NostrAuthenticationResult,
    NostrBindingState, NostrDigestRole, NostrEvidenceRole, NostrIdentityBindingRequest,
    NostrIdentityBoundary, NostrIdentityIssue, NostrTypedDigest, UCAN_NOSTR_CREDENTIAL_CONTRACT,
    UCAN_SOURCE_REVISION, ValidatedNostrIdentityPolicy,
};

/// Resolve one opaque reviewed UCAN credential against one validated policy.
// r[impl nostr_identity_bindings.verified_input]
// r[impl nostr_identity_bindings.subject_resolution]
// r[impl nostr_identity_bindings.key_lifecycle]
#[must_use]
pub fn resolve_verified_nostr_subject(
    policy: &ValidatedNostrIdentityPolicy,
    credential: &VerifiedNostrCredential,
    request: &NostrIdentityBindingRequest,
) -> NostrAuthenticationResult {
    let facts = CredentialFacts {
        public_key: credential.public_key().to_hex(),
        event_identity: credential.event_id().to_hex(),
        profile: authentication_profile_name(credential.profile()),
        audience: credential.audience().to_owned(),
        purpose: credential.purpose().to_owned(),
        request_identity: credential.request_reference().to_hex(),
        adapter_profile_identity: credential.adapter_profile_identity().to_hex(),
        adapter_implementation_identity: credential.implementation_identity().to_hex(),
    };
    resolve_credential_facts(policy, &facts, request)
}

pub(super) fn resolve_credential_facts(
    policy: &ValidatedNostrIdentityPolicy,
    credential: &CredentialFacts,
    request: &NostrIdentityBindingRequest,
) -> NostrAuthenticationResult {
    let denied_reason = authentication_denial_reason(policy, credential, request);
    let binding = policy.bindings_by_key.get(&credential.public_key);
    let authenticated_binding = denied_reason.is_none().then_some(binding).flatten();
    let decision = if authenticated_binding.is_some() {
        NostrAuthenticationDecision::Authenticated
    } else {
        NostrAuthenticationDecision::Denied
    };
    let reason = denied_reason.unwrap_or("authenticated");
    let subject_id = authenticated_binding.map(|value| value.subject_id.clone());
    let binding_id = authenticated_binding.map(|value| value.binding_id.clone());
    let binding_identity = authenticated_binding.map(|value| binding_identity(policy, value));
    let mut receipt = NostrAuthenticationReceipt {
        schema: NOSTR_AUTHENTICATION_RECEIPT_SCHEMA.to_owned(),
        ucan_source_revision: UCAN_SOURCE_REVISION.to_owned(),
        credential_contract: UCAN_NOSTR_CREDENTIAL_CONTRACT.to_owned(),
        producer_schema_identity: super::nostr_identity_schema_identity(),
        producer_implementation_identity: super::nostr_identity_implementation_identity(),
        event_identity: NostrTypedDigest::new(
            NostrDigestRole::NostrEventSha256,
            &credential.event_identity,
        ),
        public_key: credential.public_key.clone(),
        adapter_profile_identity: NostrTypedDigest::new(
            NostrDigestRole::UcanAdapterBlake3,
            &credential.adapter_profile_identity,
        ),
        adapter_implementation_identity: NostrTypedDigest::new(
            NostrDigestRole::UcanAdapterBlake3,
            &credential.adapter_implementation_identity,
        ),
        request_identity: NostrTypedDigest::new(
            NostrDigestRole::UcanAdapterBlake3,
            &credential.request_identity,
        ),
        audience_identity: public_field_identity("audience", &credential.audience),
        purpose_identity: public_field_identity("purpose", &credential.purpose),
        policy_identity: policy.policy_identity.clone(),
        subject_id: subject_id.clone(),
        binding_id: binding_id.clone(),
        binding_identity,
        decision,
        reason: reason.to_owned(),
        verification_role: NostrEvidenceRole::RecordedOnly,
        non_claims: required_non_claims(),
        receipt_identity: NostrTypedDigest::new(NostrDigestRole::BasaltBlake3, String::new()),
    };
    receipt.receipt_identity = authentication_receipt_identity(&receipt);
    let subject = match (subject_id, binding_id) {
        (Some(subject_id), Some(binding_id)) => Some(AuthenticatedNostrSubject {
            subject_id,
            binding_id,
            request_identity: receipt.request_identity.clone(),
            policy_identity: receipt.policy_identity.clone(),
            authentication_receipt_identity: receipt.receipt_identity.clone(),
        }),
        _ => None,
    };
    NostrAuthenticationResult { receipt, subject }
}

fn authentication_denial_reason(
    policy: &ValidatedNostrIdentityPolicy,
    credential: &CredentialFacts,
    request: &NostrIdentityBindingRequest,
) -> Option<&'static str> {
    if request.boundary != NostrIdentityBoundary::PublicOnly {
        return Some("boundary-rejected");
    }
    if !valid_digest(&request.policy_identity, NostrDigestRole::BasaltBlake3)
        || request.policy_identity != policy.policy_identity
    {
        return Some("policy-identity-mismatch");
    }
    if !valid_digest(
        &request.request_identity,
        NostrDigestRole::UcanAdapterBlake3,
    ) || request.request_identity.hex != credential.request_identity
    {
        return Some("request-identity-mismatch");
    }
    if request.audience != credential.audience {
        return Some("audience-mismatch");
    }
    if request.purpose != credential.purpose {
        return Some("purpose-mismatch");
    }
    if credential.profile.is_empty() {
        return Some("credential-profile-missing");
    }
    let Some(binding) = policy.bindings_by_key.get(&credential.public_key) else {
        return Some("binding-not-found");
    };
    match binding.state {
        NostrBindingState::Active => None,
        NostrBindingState::Retired => Some("binding-retired"),
        NostrBindingState::Revoked => Some("binding-revoked"),
    }
}

/// Validate a closed authentication receipt without granting authority.
// r[impl nostr_identity_bindings.receipts]
#[must_use]
pub fn verify_nostr_authentication_receipt(
    receipt: &NostrAuthenticationReceipt,
) -> Vec<NostrIdentityIssue> {
    let mut issues = Vec::new();
    if receipt.schema != NOSTR_AUTHENTICATION_RECEIPT_SCHEMA {
        issues.push(NostrIdentityIssue::new("receipt.schema", "schema-mismatch"));
    }
    if receipt.ucan_source_revision != UCAN_SOURCE_REVISION
        || receipt.credential_contract != UCAN_NOSTR_CREDENTIAL_CONTRACT
    {
        issues.push(NostrIdentityIssue::new(
            "receipt.producer",
            "producer-identity-mismatch",
        ));
    }
    check_authentication_receipt_digests(receipt, &mut issues);
    if receipt.producer_schema_identity != super::nostr_identity_schema_identity()
        || receipt.producer_implementation_identity
            != super::nostr_identity_implementation_identity()
    {
        issues.push(NostrIdentityIssue::new(
            "receipt.producer_identity",
            "producer-identity-mismatch",
        ));
    }
    if ucan_nostr::NostrPublicKey::parse(&receipt.public_key).is_err() {
        issues.push(NostrIdentityIssue::new(
            "receipt.public_key",
            "invalid-nostr-public-key",
        ));
    }
    if receipt.verification_role != NostrEvidenceRole::RecordedOnly {
        issues.push(NostrIdentityIssue::new(
            "receipt.verification_role",
            "role-promotion-denied",
        ));
    }
    if !has_exact_non_claims(&receipt.non_claims) {
        issues.push(NostrIdentityIssue::new(
            "receipt.non_claims",
            "required-non-claims-mismatch",
        ));
    }
    if !valid_authentication_reason(receipt.decision, &receipt.reason) {
        issues.push(NostrIdentityIssue::new(
            "receipt.reason",
            "reason-decision-mismatch",
        ));
    }
    for (value, field) in [
        (receipt.subject_id.as_deref(), "receipt.subject_id"),
        (receipt.binding_id.as_deref(), "receipt.binding_id"),
    ] {
        if value.is_some_and(|text| !valid_public_text(text, MAX_FIELD_BYTES)) {
            issues.push(NostrIdentityIssue::new(field, "invalid-public-identity"));
        }
    }
    let successful_shape = receipt.subject_id.is_some()
        && receipt.binding_id.is_some()
        && receipt.binding_identity.is_some();
    let denied_shape = receipt.subject_id.is_none()
        && receipt.binding_id.is_none()
        && receipt.binding_identity.is_none();
    if (receipt.decision == NostrAuthenticationDecision::Authenticated && !successful_shape)
        || (receipt.decision == NostrAuthenticationDecision::Denied && !denied_shape)
    {
        issues.push(NostrIdentityIssue::new(
            "receipt.decision",
            "decision-shape-mismatch",
        ));
    }
    if receipt.receipt_identity != authentication_receipt_identity(receipt) {
        issues.push(NostrIdentityIssue::new(
            "receipt.receipt_identity",
            "receipt-identity-mismatch",
        ));
    }
    issues.sort();
    issues.dedup();
    issues
}

fn valid_authentication_reason(decision: NostrAuthenticationDecision, reason: &str) -> bool {
    matches!(
        (decision, reason),
        (NostrAuthenticationDecision::Authenticated, "authenticated")
            | (
                NostrAuthenticationDecision::Denied,
                "boundary-rejected"
                    | "policy-identity-mismatch"
                    | "request-identity-mismatch"
                    | "audience-mismatch"
                    | "purpose-mismatch"
                    | "credential-profile-missing"
                    | "binding-not-found"
                    | "binding-retired"
                    | "binding-revoked"
            )
    )
}

fn check_authentication_receipt_digests(
    receipt: &NostrAuthenticationReceipt,
    issues: &mut Vec<NostrIdentityIssue>,
) {
    check_digest(
        &receipt.event_identity,
        NostrDigestRole::NostrEventSha256,
        "receipt.event_identity",
        issues,
    );
    for (digest, field, role) in [
        (
            &receipt.producer_schema_identity,
            "receipt.producer_schema_identity",
            NostrDigestRole::BasaltBlake3,
        ),
        (
            &receipt.producer_implementation_identity,
            "receipt.producer_implementation_identity",
            NostrDigestRole::BasaltBlake3,
        ),
        (
            &receipt.adapter_profile_identity,
            "receipt.adapter_profile_identity",
            NostrDigestRole::UcanAdapterBlake3,
        ),
        (
            &receipt.adapter_implementation_identity,
            "receipt.adapter_implementation_identity",
            NostrDigestRole::UcanAdapterBlake3,
        ),
        (
            &receipt.request_identity,
            "receipt.request_identity",
            NostrDigestRole::UcanAdapterBlake3,
        ),
        (
            &receipt.audience_identity,
            "receipt.audience_identity",
            NostrDigestRole::BasaltBlake3,
        ),
        (
            &receipt.purpose_identity,
            "receipt.purpose_identity",
            NostrDigestRole::BasaltBlake3,
        ),
        (
            &receipt.policy_identity,
            "receipt.policy_identity",
            NostrDigestRole::BasaltBlake3,
        ),
        (
            &receipt.receipt_identity,
            "receipt.receipt_identity",
            NostrDigestRole::BasaltBlake3,
        ),
    ] {
        check_digest(digest, role, field, issues);
    }
    if receipt
        .binding_identity
        .as_ref()
        .is_some_and(|digest| !valid_digest(digest, NostrDigestRole::BasaltBlake3))
    {
        issues.push(NostrIdentityIssue::new(
            "receipt.binding_identity",
            "digest-role-mismatch",
        ));
    }
}
