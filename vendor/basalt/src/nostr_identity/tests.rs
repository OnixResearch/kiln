use ucan_nostr::VerifiedNostrCredential;

use crate::authority_core::{
    AUTHORITY_DECISION_SCHEMA, AuthorityDecision, AuthorityDenyDiagnostic, AuthorityRequest,
};

use super::authentication::resolve_credential_facts;
use super::authentication_model::CredentialFacts;
use super::identity::{
    authentication_receipt_identity, composition_receipt_identity, required_non_claims,
};
use super::*;

const KEY_ONE: &str = "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9";
const KEY_TWO: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
const EVENT_ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const REQUEST_ID: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const PROFILE_ID: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const IMPLEMENTATION_ID: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
const AUDIENCE: &str = "service:operator";
const PURPOSE: &str = "authenticate-control-plane";

fn policy() -> NostrIdentityPolicy {
    NostrIdentityPolicy {
        schema: NOSTR_IDENTITY_POLICY_SCHEMA.to_owned(),
        policy_revision: "reviewed-2026-08-08".to_owned(),
        subjects: vec![NostrSubjectPolicy {
            subject_id: "subject:operator".to_owned(),
            display_label: Some("Operator".to_owned()),
            authenticators: vec![NostrAuthenticatorPolicy {
                binding_id: "binding:operator-current".to_owned(),
                public_key: KEY_ONE.to_owned(),
                state: NostrBindingState::Active,
            }],
        }],
        non_claims: required_non_claims(),
    }
}

fn facts(public_key: &str) -> CredentialFacts {
    CredentialFacts {
        public_key: public_key.to_owned(),
        event_identity: EVENT_ID.to_owned(),
        profile: "application-v1",
        audience: AUDIENCE.to_owned(),
        purpose: PURPOSE.to_owned(),
        request_identity: REQUEST_ID.to_owned(),
        adapter_profile_identity: PROFILE_ID.to_owned(),
        adapter_implementation_identity: IMPLEMENTATION_ID.to_owned(),
    }
}

fn request(policy: &ValidatedNostrIdentityPolicy) -> NostrIdentityBindingRequest {
    NostrIdentityBindingRequest {
        request_identity: NostrTypedDigest::new(NostrDigestRole::UcanAdapterBlake3, REQUEST_ID),
        audience: AUDIENCE.to_owned(),
        purpose: PURPOSE.to_owned(),
        policy_identity: policy.policy_identity.clone(),
        boundary: NostrIdentityBoundary::PublicOnly,
    }
}

fn validated() -> ValidatedNostrIdentityPolicy {
    validate_nostr_identity_policy(policy()).expect("valid policy")
}

#[test]
fn exact_active_binding_authenticates_and_emits_valid_recorded_receipt() {
    let policy = validated();
    let result = resolve_credential_facts(&policy, &facts(KEY_ONE), &request(&policy));

    assert_eq!(
        result.receipt.decision,
        NostrAuthenticationDecision::Authenticated
    );
    assert_eq!(
        result.receipt.verification_role,
        NostrEvidenceRole::RecordedOnly
    );
    assert!(verify_nostr_authentication_receipt(&result.receipt).is_empty());
    let subject = result.subject().expect("authenticated subject");
    assert_eq!(subject.subject_id(), "subject:operator");
    assert_eq!(subject.binding_id(), "binding:operator-current");
}

#[test]
fn public_adapter_requires_the_opaque_reviewed_credential_type() {
    let adapter: fn(
        &ValidatedNostrIdentityPolicy,
        &VerifiedNostrCredential,
        &NostrIdentityBindingRequest,
    ) -> NostrAuthenticationResult = resolve_verified_nostr_subject;
    assert_eq!(
        UCAN_NOSTR_CREDENTIAL_REVISION,
        "c9d43a7c11dd2fab4d5cb3d100012fa9a2e5b026"
    );
    let _ = adapter;
}

#[test]
fn order_independent_policy_identity_and_rotation_preserve_subject() {
    let mut first = policy();
    first.subjects[0].authenticators.insert(
        0,
        NostrAuthenticatorPolicy {
            binding_id: "binding:operator-retired".to_owned(),
            public_key: KEY_TWO.to_owned(),
            state: NostrBindingState::Retired,
        },
    );
    let mut second = first.clone();
    second.subjects[0].authenticators.reverse();
    let first = validate_nostr_identity_policy(first).expect("first order");
    let second = validate_nostr_identity_policy(second).expect("second order");
    assert_eq!(first.policy_identity, second.policy_identity);

    let active = resolve_credential_facts(&first, &facts(KEY_ONE), &request(&first));
    let retired = resolve_credential_facts(&first, &facts(KEY_TWO), &request(&first));
    assert_eq!(
        active.receipt.subject_id.as_deref(),
        Some("subject:operator")
    );
    assert_eq!(retired.receipt.reason, "binding-retired");
    assert!(retired.subject().is_none());
}

#[test]
fn malformed_duplicate_ambiguous_and_secret_policy_fails_closed() {
    let mut duplicate = policy();
    duplicate.subjects.push(duplicate.subjects[0].clone());
    assert!(matches!(
        validate_nostr_identity_policy(duplicate),
        Err(NostrIdentityPolicyError::Invalid(_))
    ));

    let secret_json = r#"{
          "schema":"basalt.nostr-identity-policy.v1",
          "policy_revision":"reviewed",
          "subjects":[],
          "non_claims":[],
          "private_key":"secret"
        }"#;
    assert!(matches!(
        parse_nostr_identity_policy(secret_json),
        Err(NostrIdentityPolicyError::Decode(_))
    ));
}

#[test]
fn revoked_missing_stale_and_mismatched_requests_deny() {
    let mut revoked_policy = policy();
    revoked_policy.subjects[0].authenticators[0].state = NostrBindingState::Revoked;
    let revoked_policy =
        validate_nostr_identity_policy(revoked_policy).expect("revoked policy is valid");
    let revoked =
        resolve_credential_facts(&revoked_policy, &facts(KEY_ONE), &request(&revoked_policy));
    assert_eq!(revoked.receipt.reason, "binding-revoked");

    let policy = validated();
    let mut cases = Vec::new();
    let mut wrong_request = request(&policy);
    wrong_request.request_identity.hex = "e".repeat(DIGEST_HEX_CHARS);
    cases.push(wrong_request);
    let mut wrong_audience = request(&policy);
    wrong_audience.audience = "service:other".to_owned();
    cases.push(wrong_audience);
    let mut stale_policy = request(&policy);
    stale_policy.policy_identity.hex = "f".repeat(DIGEST_HEX_CHARS);
    cases.push(stale_policy);
    for boundary in [
        NostrIdentityBoundary::ContainsSecretMaterial,
        NostrIdentityBoundary::CallerSuppliedValidity,
        NostrIdentityBoundary::PromotesMetadataIdentity,
        NostrIdentityBoundary::RequestsAuthority,
    ] {
        let mut rejected = request(&policy);
        rejected.boundary = boundary;
        cases.push(rejected);
    }
    for case in cases {
        let result = resolve_credential_facts(&policy, &facts(KEY_ONE), &case);
        assert_eq!(result.receipt.decision, NostrAuthenticationDecision::Denied);
        assert!(result.subject().is_none());
        assert!(verify_nostr_authentication_receipt(&result.receipt).is_empty());
    }
}

#[test]
fn receipt_tamper_digest_substitution_role_promotion_and_overclaim_fail() {
    let policy = validated();
    let valid = resolve_credential_facts(&policy, &facts(KEY_ONE), &request(&policy));
    let mut cases = Vec::new();
    let mut event_role = valid.receipt.clone();
    event_role.event_identity.role = NostrDigestRole::BasaltBlake3;
    cases.push(event_role);
    let mut role = valid.receipt.clone();
    role.verification_role = NostrEvidenceRole::Property;
    cases.push(role);
    let mut non_claims = valid.receipt.clone();
    non_claims.non_claims.pop();
    cases.push(non_claims);
    let mut secret_reason = valid.receipt.clone();
    secret_reason.reason = "secret=credential".to_owned();
    secret_reason.receipt_identity = authentication_receipt_identity(&secret_reason);
    cases.push(secret_reason);
    let mut tamper = valid.receipt;
    tamper.subject_id = Some("subject:other".to_owned());
    cases.push(tamper);
    for case in cases {
        assert!(!verify_nostr_authentication_receipt(&case).is_empty());
    }
}

fn authority_decision(
    subject: &AuthenticatedNostrSubject,
    diagnostic: Option<AuthorityDenyDiagnostic>,
) -> AuthorityDecision {
    let request_ref = format!("blake3:{}", subject.request_identity.hex);
    let binding = diagnostic.is_none().then(|| AuthorityRequest {
        contract_id: "operator-contract".to_owned(),
        resource: "resource:control-plane".to_owned(),
        ability: "control-plane/authenticate".to_owned(),
        request_ref: request_ref.clone(),
    });
    AuthorityDecision {
        schema: AUTHORITY_DECISION_SCHEMA.to_owned(),
        allowed: diagnostic.is_none(),
        request_ref,
        binding,
        diagnostic,
        supported_claim: "exact authority request allowed".to_owned(),
        non_claims: Vec::new(),
    }
}

fn authorization(
    subject: &AuthenticatedNostrSubject,
    diagnostic: Option<AuthorityDenyDiagnostic>,
) -> NostrAuthorizationEvidence {
    NostrAuthorizationEvidence {
        authority_decision: authority_decision(subject, diagnostic),
        subject_id: subject.subject_id.clone(),
        policy_identity: subject.policy_identity.clone(),
        authorization_receipt_identity: NostrTypedDigest::new(
            NostrDigestRole::BasaltBlake3,
            "1".repeat(DIGEST_HEX_CHARS),
        ),
    }
}

#[test]
fn exact_authorization_composes_without_promoting_evidence_role() {
    let policy = validated();
    let authentication = resolve_credential_facts(&policy, &facts(KEY_ONE), &request(&policy));
    let subject = authentication.subject().expect("subject");
    let receipt = compose_nostr_authorization(subject, &authorization(subject, None));
    assert_eq!(receipt.decision, NostrCompositionDecision::ComposedAllow);
    assert_eq!(receipt.verification_role, NostrEvidenceRole::RecordedOnly);
    assert!(verify_nostr_composition_receipt(&receipt).is_empty());
}

#[test]
fn missing_denied_stale_and_mismatched_authorization_never_compose() {
    let policy = validated();
    let authentication = resolve_credential_facts(&policy, &facts(KEY_ONE), &request(&policy));
    let subject = authentication.subject().expect("subject");
    for diagnostic in [
        AuthorityDenyDiagnostic::MissingPolicyRef,
        AuthorityDenyDiagnostic::PolicyDenied,
        AuthorityDenyDiagnostic::StaleVerificationRef,
    ] {
        let receipt =
            compose_nostr_authorization(subject, &authorization(subject, Some(diagnostic)));
        assert_eq!(receipt.decision, NostrCompositionDecision::Denied);
    }
    let mut mismatch = authorization(subject, None);
    mismatch.subject_id = "subject:other".to_owned();
    let receipt = compose_nostr_authorization(subject, &mismatch);
    assert_eq!(receipt.reason, "authorization-subject-mismatch");
}

#[test]
fn composition_tamper_and_authority_promotion_fail_validation() {
    let policy = validated();
    let authentication = resolve_credential_facts(&policy, &facts(KEY_ONE), &request(&policy));
    let subject = authentication.subject().expect("subject");
    let valid = compose_nostr_authorization(subject, &authorization(subject, None));
    let mut promoted = valid.clone();
    promoted.verification_role = NostrEvidenceRole::Authority;
    assert!(!verify_nostr_composition_receipt(&promoted).is_empty());
    let mut secret_reason = valid.clone();
    secret_reason.reason = "secret=authorization".to_owned();
    secret_reason.receipt_identity = composition_receipt_identity(&secret_reason);
    assert!(!verify_nostr_composition_receipt(&secret_reason).is_empty());
    let mut tampered = valid;
    tampered.subject_id = "subject:other".to_owned();
    assert!(!verify_nostr_composition_receipt(&tampered).is_empty());
}
