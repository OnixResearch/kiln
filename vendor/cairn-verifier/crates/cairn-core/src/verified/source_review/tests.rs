//! Deterministic TEST-ONLY signing keys. These are not real-world review approval.
// r[verify cairn.source_review.contract.valid]
// r[verify cairn.source_review.contract.invalid]
// r[verify cairn.source_review.authentication.valid]
// r[verify cairn.source_review.authentication.invalid]
// r[verify cairn.source_review.boundary.valid]
// r[verify cairn.source_review.boundary.invalid]
#![allow(clippy::unwrap_used, clippy::expect_used)]
use super::*;
use artifact_auth_core::{KeyCurrentness, TrustedKeyObservation};
use ed25519_dalek::{Signer, SigningKey};

fn hash(text: &str) -> String {
    blake3::hash(text.as_bytes()).to_hex().to_string()
}
fn key(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}
fn policy(threshold: u16) -> SourceReviewVerificationPolicy {
    let subject = SourceReviewSubject::new(
        hash("test source archive"),
        Some("test-revision".to_owned()),
        Some(hash("test source tree")),
    )
    .unwrap();
    let context = reference(
        "test-only-currentness.v1",
        &hash("test currentness observation"),
    );
    let keys = [key(17), key(29)]
        .iter()
        .map(|key| TrustedKeyObservation {
            producer_id: "test-only-reviewer".to_owned(),
            key_id: "same-display-label".to_owned(),
            key_identity: public_key_identity(key.verifying_key().as_bytes()),
            allowed_purposes: vec![PURPOSE.to_owned()],
            generation: 3,
            currentness: KeyCurrentness::Current,
            currentness_ref: context.clone(),
        })
        .collect();
    SourceReviewVerificationPolicy {
        schema: POLICY_SCHEMA.to_owned(),
        subject,
        claim_root: ClaimRoot {
            claim_identity: hash("existing test claim"),
            subject_root: hash("existing test root"),
            revision: 4,
        },
        review_policy_blake3: hash("test review policy"),
        authentication_policy: AuthenticationPolicy {
            schema: artifact_auth_core::POLICY_SCHEMA_V1.to_owned(),
            profile_id: PROFILE.to_owned(),
            threshold,
            trusted_keys: keys,
        },
        verifier_context: context,
        author_key_blake3: vec![],
        exclude_authors: true,
        required_approvals: threshold,
        non_claims: required_non_claims(),
    }
}
fn request(
    policy: &SourceReviewVerificationPolicy,
    seed: u8,
    disposition: SourceReviewDisposition,
) -> SourceReviewSigningRequest {
    let claim = SourceReviewClaim::new(
        policy.subject.clone(),
        policy.claim_root.clone(),
        policy.review_policy_blake3.clone(),
        disposition,
    )
    .unwrap();
    signing_request(
        claim,
        policy,
        "test-only-reviewer".to_owned(),
        "same-display-label".to_owned(),
        3,
        encode_hex(key(seed).verifying_key().as_bytes()),
    )
    .unwrap()
}
fn signed(
    policy: &SourceReviewVerificationPolicy,
    seed: u8,
    disposition: SourceReviewDisposition,
) -> SignedSourceReview {
    let request = request(policy, seed, disposition);
    let signature = key(seed).sign(&signing_request_bytes(&request).unwrap());
    assemble(request, encode_hex(&signature.to_bytes()), policy).unwrap()
}
fn approved(policy: &SourceReviewVerificationPolicy, seed: u8) -> SignedSourceReview {
    signed(policy, seed, SourceReviewDisposition::Approved)
}
fn attachment(policy: &SourceReviewVerificationPolicy) -> SourceReviewAttachment {
    SourceReviewAttachment::new(vec![approved(policy, 17), approved(policy, 29)]).unwrap()
}
fn rejects(
    attachment: &SourceReviewAttachment,
    policy: &SourceReviewVerificationPolicy,
    code: &str,
) {
    assert_eq!(verify(attachment, policy).unwrap_err().0, code);
}

#[test]
fn distinct_full_keys_count_despite_identical_display_labels() {
    let policy = policy(2);
    let attachment = attachment(&policy);
    let report = verify(&attachment, &policy).unwrap();
    assert_eq!(report.outcome, SourceReviewOutcome::AcceptedManualReview);
    let mut expected = vec![
        public_key_identity(key(17).verifying_key().as_bytes()).digest_hex,
        public_key_identity(key(29).verifying_key().as_bytes()).digest_hex,
    ];
    expected.sort();
    assert_eq!(report.approved_key_blake3, expected);
    assert_eq!(report.verification_role, "manual_review");
    let single = SourceReviewAttachment::new(vec![approved(&policy, 17)]).unwrap();
    assert_eq!(
        verify(&single, &policy).unwrap().outcome,
        SourceReviewOutcome::Insufficient
    );
}

#[test]
fn needs_revision_is_authenticated_rejection_not_approval() {
    let policy = policy(2);
    let rejection = signed(&policy, 29, SourceReviewDisposition::NeedsRevision);
    let attachment = SourceReviewAttachment::new(vec![approved(&policy, 17), rejection]).unwrap();
    let report = verify(&attachment, &policy).unwrap();
    assert_eq!(report.outcome, SourceReviewOutcome::Insufficient);
    assert_eq!(
        report.approved_key_blake3,
        vec![public_key_identity(key(17).verifying_key().as_bytes()).digest_hex]
    );
    assert_eq!(
        report.needs_revision_key_blake3,
        vec![public_key_identity(key(29).verifying_key().as_bytes()).digest_hex]
    );
    let mut altered = attachment.clone();
    altered
        .reviews
        .iter_mut()
        .find(|row| row.claim.disposition == SourceReviewDisposition::NeedsRevision)
        .unwrap()
        .claim
        .disposition = SourceReviewDisposition::Approved;
    rejects(&altered, &policy, "statement_scope_mismatch");
}

#[test]
fn forged_signature_and_rescoped_disposition_fail_crypto() {
    let policy = policy(1);
    let mut row = signed(&policy, 17, SourceReviewDisposition::NeedsRevision);
    row.claim.disposition = SourceReviewDisposition::Approved;
    row.statement.scope = row.claim.scope(policy.verifier_context.clone()).unwrap();
    rejects(
        &SourceReviewAttachment::new(vec![row]).unwrap(),
        &policy,
        "signature_authentication_failed",
    );
    let mut attachment = attachment(&policy);
    attachment.reviews[0].signature_hex = "00".repeat(64);
    rejects(&attachment, &policy, "signature_authentication_failed");
}

#[test]
fn exact_source_revision_tree_archive_root_and_policy_are_bound() {
    let original = policy(1);
    let attachment = attachment(&original);
    let mut policy = original.clone();
    policy.subject.source_revision = Some("changed-revision".to_owned());
    rejects(&attachment, &policy, "source_subject_mismatch");
    policy = original.clone();
    policy.subject.source_archive_blake3 = hash("changed archive");
    rejects(&attachment, &policy, "source_subject_mismatch");
    policy = original.clone();
    policy.subject.source_tree_blake3 = Some(hash("changed tree"));
    rejects(&attachment, &policy, "source_subject_mismatch");
    policy = original.clone();
    policy.claim_root.revision += 1;
    rejects(&attachment, &policy, "claim_root_mismatch");
    policy = original.clone();
    policy.claim_root.subject_root = hash("changed root");
    rejects(&attachment, &policy, "claim_root_mismatch");
    policy = original;
    policy.review_policy_blake3 = hash("changed policy");
    rejects(&attachment, &policy, "review_policy_mismatch");
}

#[test]
fn stale_generation_unknown_revoked_and_superseded_fail_closed() {
    let original = policy(1);
    let attachment = attachment(&original);
    let mut stale_row = attachment.clone();
    stale_row.reviews[0].generation = 2;
    rejects(&stale_row, &original, "key_generation_stale");
    for currentness in [
        KeyCurrentness::Unknown,
        KeyCurrentness::Revoked,
        KeyCurrentness::Superseded,
    ] {
        let mut policy = original.clone();
        for key in &mut policy.authentication_policy.trusted_keys {
            key.currentness = currentness;
        }
        rejects(&attachment, &policy, "key_not_current");
    }
    let mut overlap = original;
    for key in &mut overlap.authentication_policy.trusted_keys {
        key.currentness = KeyCurrentness::VerificationOverlap;
    }
    assert_eq!(
        verify(&attachment, &overlap).unwrap().outcome,
        SourceReviewOutcome::AcceptedManualReview
    );
}

#[test]
fn excluded_authors_and_duplicate_full_keys_cannot_inflate_threshold() {
    let mut policy = policy(2);
    let row = approved(&policy, 17);
    let attachment = attachment(&policy);
    policy
        .author_key_blake3
        .push(row.statement.key_identity.digest_hex.clone());
    rejects(&attachment, &policy, "author_key_excluded");
    policy.exclude_authors = false;
    assert_eq!(
        verify(&attachment, &policy).unwrap().outcome,
        SourceReviewOutcome::AcceptedManualReview
    );
    assert_eq!(
        SourceReviewAttachment::new(vec![row.clone(), row])
            .unwrap_err()
            .0,
        "duplicate_review_key"
    );
    let mut forged_alias = attachment.reviews[0].clone();
    forged_alias.statement.key_id = "different-display-label".to_owned();
    assert_eq!(
        SourceReviewAttachment::new(vec![attachment.reviews[0].clone(), forged_alias])
            .unwrap_err()
            .0,
        "duplicate_review_key"
    );
}

#[test]
fn authentication_authority_cannot_be_replaced_by_labels_roles_or_context() {
    let original = policy(1);
    let attachment = attachment(&original);
    let mut wrong_key = attachment.clone();
    wrong_key.reviews[0].public_key_hex = encode_hex(key(61).verifying_key().as_bytes());
    rejects(&wrong_key, &original, "public_key_identity_mismatch");
    let mut policy = original.clone();
    policy.authentication_policy.trusted_keys[0].key_identity =
        public_key_identity(key(61).verifying_key().as_bytes());
    rejects(&attachment, &policy, "key_untrusted");
    policy = original.clone();
    policy.authentication_policy.trusted_keys[0]
        .allowed_purposes
        .push("release-signature".to_owned());
    rejects(&attachment, &policy, "key_role_not_source_review_only");
    policy = original.clone();
    policy.authentication_policy.trusted_keys[0]
        .currentness_ref
        .digest_hex = hash("stale observation");
    rejects(&attachment, &policy, "currentness_context_mismatch");
    policy = original;
    policy.authentication_policy.threshold = 2;
    rejects(&attachment, &policy, "authentication_policy_mismatch");
}

#[test]
fn unsupported_schemas_unknown_fields_and_nonclaims_are_rejected() {
    let policy = policy(1);
    let original = attachment(&policy);
    for mutate in [0, 1, 2] {
        let mut attachment = original.clone();
        match mutate {
            0 => attachment.schema = "cairn.source-review-attachment.v2".to_owned(),
            1 => attachment.reviews[0].claim.schema = "cairn.source-review-claim.v2".to_owned(),
            _ => {
                attachment.reviews[0].claim.subject.schema =
                    "cairn.source-review-subject.v2".to_owned()
            }
        }
        assert!(verify(&attachment, &policy).is_err());
    }
    let mut attachment = original.clone();
    attachment.non_claims.pop();
    rejects(&attachment, &policy, "non_claims_mismatch");
    let mut attachment = original.clone();
    attachment.reviews[0]
        .claim
        .non_claims
        .push("this-proves-source-correctness".to_owned());
    rejects(&attachment, &policy, "non_claims_mismatch");
    let mut value = serde_json::to_value(&original).unwrap();
    value["verified"] = Value::Bool(true);
    assert!(parse::<SourceReviewAttachment>(&serde_json::to_vec(&value).unwrap()).is_err());
    let duplicated = String::from_utf8(serde_json::to_vec(&original).unwrap())
        .unwrap()
        .replacen("{", "{\"schema\":\"cairn.source-review-attachment.v1\",", 1);
    assert!(parse::<SourceReviewAttachment>(duplicated.as_bytes()).is_err());
}

#[test]
fn canonical_identity_is_key_sorted_order_independent_and_byte_artifacts_stay_separate() {
    let policy = policy(2);
    let original = attachment(&policy);
    let mut reversed = original.clone();
    reversed.reviews.reverse();
    assert_eq!(original.identity().unwrap(), reversed.identity().unwrap());
    let compact = original.canonical_bytes().unwrap();
    let pretty = serde_json::to_vec_pretty(&reversed).unwrap();
    assert_ne!(blake3::hash(&compact), blake3::hash(&pretty));
    let parsed: SourceReviewAttachment = parse(&pretty).unwrap();
    assert_eq!(original.identity().unwrap(), parsed.identity().unwrap());
    // Canonicalization must be recursive, not Rust declaration order dependent.
    let value = serde_json::json!({"z": {"b": 2, "a": 1}, "a": [{"z": 0, "a": 3}]});
    assert_eq!(
        canonical_json_bytes(&value).unwrap(),
        br#"{"a":[{"a":3,"z":0}],"z":{"a":1,"b":2}}"#
    );
    let mut changed = original;
    changed.reviews.pop();
    assert_ne!(parsed.identity().unwrap(), changed.identity().unwrap());
}

#[test]
fn request_reconstruction_and_detached_signature_require_exact_unsigned_bytes() {
    let policy = policy(1);
    let original = request(&policy, 17, SourceReviewDisposition::Approved);
    let signature = encode_hex(
        &key(17)
            .sign(&signing_request_bytes(&original).unwrap())
            .to_bytes(),
    );
    let mut request = original.clone();
    request.canonical_statement_hex.push_str("00");
    assert_eq!(
        assemble(request, signature.clone(), &policy).unwrap_err().0,
        "signing_request_tampered"
    );
    let mut request = original.clone();
    request.statement_blake3 = hash("different statement");
    assert_eq!(
        assemble(request, signature, &policy).unwrap_err().0,
        "signing_request_tampered"
    );
    let wrong_bytes_signature = encode_hex(
        &key(17)
            .sign(&serde_json::to_vec(&original.statement).unwrap())
            .to_bytes(),
    );
    assert_eq!(
        assemble(original, wrong_bytes_signature, &policy)
            .unwrap_err()
            .0,
        "signature_authentication_failed"
    );
}

#[test]
fn bounds_empty_inputs_and_malformed_encodings_fail_closed() {
    let policy = policy(1);
    let empty = SourceReviewAttachment::new(vec![]).unwrap();
    assert_eq!(
        verify(&empty, &policy).unwrap().outcome,
        SourceReviewOutcome::Insufficient
    );
    let row = approved(&policy, 17);
    assert_eq!(
        SourceReviewAttachment::new(vec![row.clone(); 65])
            .unwrap_err()
            .0,
        "review_count_exceeded"
    );
    assert!(parse::<SourceReviewAttachment>(&vec![b' '; MAX_INPUT_BYTES + 1]).is_err());
    let mut malformed = row;
    malformed.signature_hex = "GG".repeat(64);
    assert_eq!(
        SourceReviewAttachment::new(vec![malformed]).unwrap_err().0,
        "hex_not_lowercase"
    );
    let mut policy = policy;
    policy.required_approvals = 0;
    policy.authentication_policy.threshold = 0;
    assert_eq!(
        verify(&empty, &policy).unwrap_err().0,
        "approval_threshold_invalid"
    );
}
