use artifact_auth_core::{CryptographicObservation, ALGORITHM_ED25519};

use crate::{
    AuthenticatedEvidenceInput, AuthenticatedEvidencePolicy, AuthenticatedTrustedProducer,
    CanonicalReceiptObservation, PolicyExportObservation, SignatureVerificationObservation,
    AUTHENTICATED_EVIDENCE_CANONICALIZATION_CORRECTED_V1,
    AUTHENTICATED_EVIDENCE_KIND_STACK_PROVENANCE, AUTHENTICATED_EVIDENCE_SCHEMA_V1,
};

use super::*;

const HASH_A: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const HASH_B: &str = "123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0";
const HASH_C: &str = "23456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef01";
const PUBLIC_KEY_HEX: &str = "456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123";
const PURPOSE: &str = "onix.stack-provenance.release-evidence.v1";

fn policy() -> AuthenticatedEvidencePolicy {
    AuthenticatedEvidencePolicy {
        schema_version: AUTHENTICATED_EVIDENCE_SCHEMA_V1.to_string(),
        release_profile: "onix-release".to_string(),
        accepted_canonicalization_profiles: vec![
            AUTHENTICATED_EVIDENCE_CANONICALIZATION_CORRECTED_V1.to_string(),
        ],
        accepted_valence_integrity_receipt_path: "valence-integrity.json".to_string(),
        accepted_valence_integrity_receipt_hash: HASH_C.to_string(),
        policy_exports: vec![],
        trusted_producers: vec![AuthenticatedTrustedProducer {
            producer: "valence".to_string(),
            key_id: "release-key".to_string(),
            algorithm: ALGORITHM_ED25519.to_string(),
            public_key_hex: PUBLIC_KEY_HEX.to_string(),
            allowed_purposes: vec![PURPOSE.to_string()],
            revoked: false,
        }],
    }
}

fn input() -> AuthenticatedEvidenceInput {
    AuthenticatedEvidenceInput {
        schema_version: AUTHENTICATED_EVIDENCE_SCHEMA_V1.to_string(),
        evidence_kind: AUTHENTICATED_EVIDENCE_KIND_STACK_PROVENANCE.to_string(),
        measurements: vec![],
        receipt: CanonicalReceiptObservation {
            profile: AUTHENTICATED_EVIDENCE_CANONICALIZATION_CORRECTED_V1.to_string(),
            supplied_receipt_hash: HASH_A.to_string(),
            recomputed_receipt_hash: HASH_A.to_string(),
        },
        policy_export: PolicyExportObservation {
            path: "policy.json".to_string(),
            declared_blake3: HASH_B.to_string(),
            measured_blake3: HASH_B.to_string(),
        },
        signature: SignatureVerificationObservation {
            algorithm: ALGORITHM_ED25519.to_string(),
            verified: false,
            producer: "valence".to_string(),
            key_id: "release-key".to_string(),
            purpose: PURPOSE.to_string(),
            subject_hash: HASH_A.to_string(),
            parent_hashes: vec![HASH_B.to_string()],
            public_key_blake3: blake3::hash(PUBLIC_KEY_HEX.as_bytes()).to_hex().to_string(),
        },
        required_parent_hashes: vec![HASH_B.to_string()],
        valence_integrity_receipt_hash: HASH_C.to_string(),
    }
}

fn observation(mapped: &CairnArtifactAuthInput, verified: bool) -> CryptographicObservation {
    CryptographicObservation {
        algorithm: ALGORITHM_ED25519.to_string(),
        key_identity: mapped.statement.key_identity.clone(),
        verified,
        failure_code: (!verified).then(|| "ed25519.signature_invalid".to_string()),
    }
}

// r[verify cairn.artifact_auth.repository_extraction.consumer_authority.cairn]
#[test]
fn maps_lifecycle_observations_and_dual_records_key_identities() {
    let input = input();
    let mapped = map_cairn_artifact_auth(&policy(), &input).expect("valid mapping");
    assert_eq!(
        mapped.scope.domain,
        "cairn.authenticated-evidence.v1/stack-provenance"
    );
    assert_eq!(mapped.scope.purpose, PURPOSE);
    assert_eq!(mapped.scope.subject.digest_hex, HASH_A);
    assert_eq!(mapped.scope.parents[0].digest_hex, HASH_B);
    assert_eq!(mapped.scope.verifier_context.digest_hex, HASH_C);
    assert_eq!(
        mapped.legacy_public_key_blake3,
        input.signature.public_key_blake3
    );
    assert_ne!(
        mapped.legacy_public_key_blake3,
        mapped.standalone_public_key_blake3
    );
    assert_eq!(
        mapped.statement.key_identity.digest_hex,
        mapped.standalone_public_key_blake3
    );
}

// r[verify cairn.artifact_auth.repository_extraction.consumer_authority.no_promotion]
#[test]
fn explicit_standalone_observation_controls_the_bounded_report() {
    let mut input = input();
    input.signature.verified = false;
    let mapped = map_cairn_artifact_auth(&policy(), &input).expect("valid mapping");
    let passing = evaluate_cairn_artifact_auth(&input, &mapped, observation(&mapped, true));
    assert!(passing.decision_passed, "{:?}", passing.issues);
    assert_eq!(
        passing.authentication_hash.as_deref(),
        Some(passing.standalone_decision_blake3.as_str())
    );
    assert!(!passing.non_claims.is_empty());
    assert_eq!(passing.boundary, CAIRN_ARTIFACT_AUTH_BOUNDARY);

    input.signature.verified = true;
    let failing = evaluate_cairn_artifact_auth(&input, &mapped, observation(&mapped, false));
    assert!(!failing.decision_passed);
    assert!(failing.authentication_hash.is_none());
    assert!(failing
        .issues
        .iter()
        .any(|issue| issue.code == "auth.signature_unverified"));
}

#[test]
fn malformed_or_mismatched_key_material_fails_mapping_without_key_diagnostics() {
    let mut malformed_policy = policy();
    malformed_policy.trusted_producers[0].public_key_hex = "secret-like-invalid".to_string();
    let malformed =
        map_cairn_artifact_auth(&malformed_policy, &input()).expect_err("malformed key must fail");
    assert!(malformed
        .iter()
        .any(|issue| issue.code == "artifact_auth.adapter.public_key_malformed"));
    assert!(malformed
        .iter()
        .all(|issue| !issue.actual.contains("secret-like-invalid")));

    let mut mismatched = input();
    mismatched.signature.public_key_blake3 = HASH_A.to_string();
    let mismatch = map_cairn_artifact_auth(&policy(), &mismatched).expect_err("mismatch must fail");
    assert!(mismatch
        .iter()
        .any(|issue| issue.code == "artifact_auth.adapter.legacy_key_identity_mismatch"));

    let mut unsupported_policy = policy();
    unsupported_policy.schema_version = "unknown-policy-schema".to_string();
    let mut unsupported_input = input();
    unsupported_input.signature.algorithm = "unknown-signature-algorithm".to_string();
    let unsupported = map_cairn_artifact_auth(&unsupported_policy, &unsupported_input)
        .expect_err("unsupported source contract must fail");
    assert!(unsupported
        .iter()
        .any(|issue| issue.code == "artifact_auth.adapter.policy_schema_unsupported"));
    assert!(unsupported
        .iter()
        .any(|issue| issue.code == "artifact_auth.adapter.signature_algorithm_unsupported"));
}

#[test]
fn revoked_consumer_observation_remains_rejected_by_standalone_policy() {
    let input = input();
    let mut revoked_policy = policy();
    revoked_policy.trusted_producers[0].revoked = true;
    let mapped = map_cairn_artifact_auth(&revoked_policy, &input).expect("valid mapping");
    let report = evaluate_cairn_artifact_auth(&input, &mapped, observation(&mapped, true));
    assert!(!report.decision_passed);
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.code == "auth.key_not_current"));
}
