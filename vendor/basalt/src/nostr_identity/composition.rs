use serde::{Deserialize, Serialize};

use crate::authority_core::{
    AUTHORITY_DECISION_SCHEMA, AuthorityDecision, AuthorityDenyDiagnostic,
};

use super::identity::{
    check_digest, composition_receipt_identity, has_exact_non_claims, public_field_identity,
    required_non_claims, valid_digest, valid_public_text,
};
use super::{
    AuthenticatedNostrSubject, MAX_FIELD_BYTES, NOSTR_COMPOSITION_RECEIPT_SCHEMA, NostrDigestRole,
    NostrEvidenceRole, NostrIdentityIssue, NostrTypedDigest,
};

const CONTENT_REFERENCE_PREFIX: &str = "blake3:";

/// Exact linkage around a separate Basalt authority-core decision.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NostrAuthorizationEvidence {
    /// Decision produced by the existing Basalt authority core.
    pub authority_decision: AuthorityDecision,
    /// Stable subject bound by caller policy to that authority request.
    pub subject_id: String,
    /// Exact Nostr subject-policy identity used for the composition.
    pub policy_identity: NostrTypedDigest,
    /// BLAKE3 identity of the separate authority receipt.
    pub authorization_receipt_identity: NostrTypedDigest,
}

/// Recorded composition decision.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NostrCompositionDecision {
    /// Matching authentication and authority-core decisions were recorded.
    ComposedAllow,
    /// Composition failed closed.
    Denied,
}

/// Closed recorded-only composition receipt.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NostrCompositionReceipt {
    /// Closed receipt schema.
    pub schema: String,
    /// Basalt Nostr producer schema identity.
    pub producer_schema_identity: NostrTypedDigest,
    /// Basalt Nostr implementation-profile identity.
    pub producer_implementation_identity: NostrTypedDigest,
    /// Authentication receipt identity.
    pub authentication_receipt_identity: NostrTypedDigest,
    /// Separate authorization receipt identity.
    pub authorization_receipt_identity: NostrTypedDigest,
    /// Stable subject identity.
    pub subject_id: String,
    /// Exact request identity.
    pub request_identity: NostrTypedDigest,
    /// Exact subject-policy identity.
    pub policy_identity: NostrTypedDigest,
    /// Authorized resource identity. It is absent on denial.
    pub resource_identity: Option<NostrTypedDigest>,
    /// Authorized ability identity. It is absent on denial.
    pub ability_identity: Option<NostrTypedDigest>,
    /// Composition decision.
    pub decision: NostrCompositionDecision,
    /// Stable public reason.
    pub reason: String,
    /// Always recorded-only.
    pub verification_role: NostrEvidenceRole,
    /// Required claim boundaries.
    pub non_claims: Vec<String>,
    /// BLAKE3 identity of this receipt with this field empty.
    pub receipt_identity: NostrTypedDigest,
}

/// Compose an authenticated subject with a separate Basalt authority decision.
// r[impl nostr_identity_bindings.authority_separation]
#[must_use]
pub fn compose_nostr_authorization(
    subject: &AuthenticatedNostrSubject,
    authorization: &NostrAuthorizationEvidence,
) -> NostrCompositionReceipt {
    let reason = composition_denial_reason(subject, authorization);
    let decision = if reason.is_none() {
        NostrCompositionDecision::ComposedAllow
    } else {
        NostrCompositionDecision::Denied
    };
    let binding = reason
        .is_none()
        .then_some(authorization.authority_decision.binding.as_ref())
        .flatten();
    let mut receipt = NostrCompositionReceipt {
        schema: NOSTR_COMPOSITION_RECEIPT_SCHEMA.to_owned(),
        producer_schema_identity: super::nostr_identity_schema_identity(),
        producer_implementation_identity: super::nostr_identity_implementation_identity(),
        authentication_receipt_identity: subject.authentication_receipt_identity.clone(),
        authorization_receipt_identity: authorization.authorization_receipt_identity.clone(),
        subject_id: subject.subject_id.clone(),
        request_identity: subject.request_identity.clone(),
        policy_identity: subject.policy_identity.clone(),
        resource_identity: binding.map(|value| public_field_identity("resource", &value.resource)),
        ability_identity: binding.map(|value| public_field_identity("ability", &value.ability)),
        decision,
        reason: reason.unwrap_or("composed-allow").to_owned(),
        verification_role: NostrEvidenceRole::RecordedOnly,
        non_claims: required_non_claims(),
        receipt_identity: NostrTypedDigest::new(NostrDigestRole::BasaltBlake3, String::new()),
    };
    receipt.receipt_identity = composition_receipt_identity(&receipt);
    receipt
}

fn composition_denial_reason(
    subject: &AuthenticatedNostrSubject,
    authorization: &NostrAuthorizationEvidence,
) -> Option<&'static str> {
    if authorization.subject_id != subject.subject_id {
        return Some("authorization-subject-mismatch");
    }
    if authorization.policy_identity != subject.policy_identity {
        return Some("authorization-policy-mismatch");
    }
    if !valid_digest(
        &authorization.authorization_receipt_identity,
        NostrDigestRole::BasaltBlake3,
    ) {
        return Some("authorization-digest-invalid");
    }
    let decision = &authorization.authority_decision;
    if decision.schema != AUTHORITY_DECISION_SCHEMA {
        return Some("authorization-schema-mismatch");
    }
    if !decision.allowed {
        return Some(denied_authority_reason(decision.diagnostic));
    }
    if decision.diagnostic.is_some() {
        return Some("authorization-shape-invalid");
    }
    let Some(binding) = decision.binding.as_ref() else {
        return Some("authorization-shape-invalid");
    };
    let expected_request_ref =
        format!("{CONTENT_REFERENCE_PREFIX}{}", subject.request_identity.hex);
    if decision.request_ref != expected_request_ref || binding.request_ref != expected_request_ref {
        return Some("authorization-request-mismatch");
    }
    None
}

const fn denied_authority_reason(diagnostic: Option<AuthorityDenyDiagnostic>) -> &'static str {
    match diagnostic {
        Some(
            AuthorityDenyDiagnostic::MissingPolicyRef
            | AuthorityDenyDiagnostic::MissingVerifiedGrant
            | AuthorityDenyDiagnostic::MissingVerificationReceipt,
        ) => "authorization-missing",
        Some(AuthorityDenyDiagnostic::StaleVerificationRef) => "authorization-stale",
        Some(_) | None => "authorization-denied",
    }
}

/// Validate a closed recorded-only composition receipt.
// r[impl nostr_identity_bindings.receipts]
#[must_use]
pub fn verify_nostr_composition_receipt(
    receipt: &NostrCompositionReceipt,
) -> Vec<NostrIdentityIssue> {
    let mut issues = Vec::new();
    if receipt.schema != NOSTR_COMPOSITION_RECEIPT_SCHEMA {
        issues.push(NostrIdentityIssue::new(
            "composition.schema",
            "schema-mismatch",
        ));
    }
    check_composition_receipt_digests(receipt, &mut issues);
    let allowed_shape = receipt.resource_identity.is_some() && receipt.ability_identity.is_some();
    let denied_shape = receipt.resource_identity.is_none() && receipt.ability_identity.is_none();
    if (receipt.decision == NostrCompositionDecision::ComposedAllow && !allowed_shape)
        || (receipt.decision == NostrCompositionDecision::Denied && !denied_shape)
    {
        issues.push(NostrIdentityIssue::new(
            "composition.decision",
            "decision-shape-mismatch",
        ));
    }
    if !valid_composition_reason(receipt.decision, &receipt.reason) {
        issues.push(NostrIdentityIssue::new(
            "composition.reason",
            "reason-decision-mismatch",
        ));
    }
    if receipt.producer_schema_identity != super::nostr_identity_schema_identity()
        || receipt.producer_implementation_identity
            != super::nostr_identity_implementation_identity()
    {
        issues.push(NostrIdentityIssue::new(
            "composition.producer_identity",
            "producer-identity-mismatch",
        ));
    }
    if receipt.verification_role != NostrEvidenceRole::RecordedOnly {
        issues.push(NostrIdentityIssue::new(
            "composition.verification_role",
            "role-promotion-denied",
        ));
    }
    if !has_exact_non_claims(&receipt.non_claims) {
        issues.push(NostrIdentityIssue::new(
            "composition.non_claims",
            "required-non-claims-mismatch",
        ));
    }
    if !valid_public_text(&receipt.subject_id, MAX_FIELD_BYTES) {
        issues.push(NostrIdentityIssue::new(
            "composition.subject_id",
            "invalid-subject-id",
        ));
    }
    if receipt.receipt_identity != composition_receipt_identity(receipt) {
        issues.push(NostrIdentityIssue::new(
            "composition.receipt_identity",
            "receipt-identity-mismatch",
        ));
    }
    issues.sort();
    issues.dedup();
    issues
}

fn valid_composition_reason(decision: NostrCompositionDecision, reason: &str) -> bool {
    matches!(
        (decision, reason),
        (NostrCompositionDecision::ComposedAllow, "composed-allow")
            | (
                NostrCompositionDecision::Denied,
                "authorization-subject-mismatch"
                    | "authorization-policy-mismatch"
                    | "authorization-digest-invalid"
                    | "authorization-schema-mismatch"
                    | "authorization-shape-invalid"
                    | "authorization-request-mismatch"
                    | "authorization-missing"
                    | "authorization-stale"
                    | "authorization-denied"
            )
    )
}

fn check_composition_receipt_digests(
    receipt: &NostrCompositionReceipt,
    issues: &mut Vec<NostrIdentityIssue>,
) {
    for (digest, field, role) in [
        (
            &receipt.producer_schema_identity,
            "composition.producer_schema_identity",
            NostrDigestRole::BasaltBlake3,
        ),
        (
            &receipt.producer_implementation_identity,
            "composition.producer_implementation_identity",
            NostrDigestRole::BasaltBlake3,
        ),
        (
            &receipt.authentication_receipt_identity,
            "composition.authentication_receipt_identity",
            NostrDigestRole::BasaltBlake3,
        ),
        (
            &receipt.authorization_receipt_identity,
            "composition.authorization_receipt_identity",
            NostrDigestRole::BasaltBlake3,
        ),
        (
            &receipt.request_identity,
            "composition.request_identity",
            NostrDigestRole::UcanAdapterBlake3,
        ),
        (
            &receipt.policy_identity,
            "composition.policy_identity",
            NostrDigestRole::BasaltBlake3,
        ),
        (
            &receipt.receipt_identity,
            "composition.receipt_identity",
            NostrDigestRole::BasaltBlake3,
        ),
    ] {
        check_digest(digest, role, field, issues);
    }
    for (digest, field) in [
        (
            receipt.resource_identity.as_ref(),
            "composition.resource_identity",
        ),
        (
            receipt.ability_identity.as_ref(),
            "composition.ability_identity",
        ),
    ] {
        if digest.is_some_and(|value| !valid_digest(value, NostrDigestRole::BasaltBlake3)) {
            issues.push(NostrIdentityIssue::new(field, "digest-role-mismatch"));
        }
    }
}
