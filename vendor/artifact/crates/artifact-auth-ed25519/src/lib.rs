#![no_std]
//! Pure Ed25519 verification over canonical `artifact-auth-core` statement bytes.
//!
//! This package does not sign, generate keys, store private material, discover trust,
//! access clocks, read files, contact networks, or perform persistence.

// r[impl artifact_auth.core.authority_free]
// r[impl artifact_auth.ed25519.verify]
extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use artifact_auth_core::{
    ALGORITHM_BLAKE3, ALGORITHM_ED25519, ArtifactRef, ArtifactStatement, AuthenticationDecision,
    AuthenticationPolicy, AuthenticationScope, CryptographicObservation,
    ED25519_PUBLIC_KEY_PROFILE_V1, SignatureEvidence, canonical_statement_bytes,
    evaluate_authentication,
};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

/// Required Ed25519 public-key byte length.
pub const ED25519_PUBLIC_KEY_BYTES: usize = 32;
/// Required Ed25519 signature byte length.
pub const ED25519_SIGNATURE_BYTES: usize = 64;

/// Stable Ed25519 verification failure classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ed25519Failure {
    /// The statement was not canonicalizable.
    StatementInvalid,
    /// Public-key bytes had the wrong length.
    PublicKeyLength,
    /// Public-key bytes were not a valid Ed25519 point.
    PublicKeyMalformed,
    /// Signature bytes had the wrong length.
    SignatureLength,
    /// Signature verification failed.
    SignatureInvalid,
}

impl Ed25519Failure {
    /// Stable issue code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::StatementInvalid => "ed25519.statement_invalid",
            Self::PublicKeyLength => "ed25519.public_key_length",
            Self::PublicKeyMalformed => "ed25519.public_key_malformed",
            Self::SignatureLength => "ed25519.signature_length",
            Self::SignatureInvalid => "ed25519.signature_invalid",
        }
    }
}

/// Compute the full BLAKE3 identity of supplied public-key bytes.
#[must_use]
pub fn public_key_identity(public_key_bytes: &[u8]) -> ArtifactRef {
    ArtifactRef {
        profile: ED25519_PUBLIC_KEY_PROFILE_V1.to_string(),
        algorithm: ALGORITHM_BLAKE3.to_string(),
        digest_hex: blake3::hash(public_key_bytes).to_hex().to_string(),
    }
}

/// Explicit bytes and metadata for one composed verification decision.
#[derive(Debug, Clone, Copy)]
pub struct Ed25519Evidence<'a> {
    /// Signer-specific canonical statement.
    pub statement: &'a ArtifactStatement,
    /// Key generation asserted by the signature carrier.
    pub generation: u64,
    /// Explicit public-key bytes.
    pub public_key_bytes: &'a [u8],
    /// Explicit detached-signature bytes.
    pub signature_bytes: &'a [u8],
}

/// Verify every supplied signature and immediately evaluate policy over those observations.
///
/// This is the normal Ed25519 composition path. Protocol adapters with a different
/// cryptographic verifier may instead supply observations directly to the core, but
/// they retain responsibility for the provenance and correctness of those observations.
#[must_use]
pub fn verify_and_evaluate(
    policy: &AuthenticationPolicy,
    scope: &AuthenticationScope,
    evidence: &[Ed25519Evidence<'_>],
) -> AuthenticationDecision {
    let observations = evidence
        .iter()
        .map(|item| SignatureEvidence {
            statement: item.statement.clone(),
            generation: item.generation,
            cryptographic: verify_statement(
                item.statement,
                item.public_key_bytes,
                item.signature_bytes,
            ),
        })
        .collect::<Vec<_>>();
    evaluate_authentication(policy, scope, &observations)
}

/// Verify one signature over canonical signer-specific statement bytes.
///
/// The function always returns a deterministic observation. Invalid inputs set
/// `verified = false` and expose one stable failure class without including raw
/// key, signature, statement, or secret-bearing diagnostics.
#[must_use]
pub fn verify_statement(
    statement: &ArtifactStatement,
    public_key_bytes: &[u8],
    signature_bytes: &[u8],
) -> CryptographicObservation {
    let key_identity = public_key_identity(public_key_bytes);
    let result = verify(statement, public_key_bytes, signature_bytes);
    match result {
        Ok(()) => CryptographicObservation {
            algorithm: ALGORITHM_ED25519.to_string(),
            key_identity,
            verified: true,
            failure_code: None,
        },
        Err(failure) => CryptographicObservation {
            algorithm: ALGORITHM_ED25519.to_string(),
            key_identity,
            verified: false,
            failure_code: Some(String::from(failure.code())),
        },
    }
}

fn verify(
    statement: &ArtifactStatement,
    public_key_bytes: &[u8],
    signature_bytes: &[u8],
) -> Result<(), Ed25519Failure> {
    let message =
        canonical_statement_bytes(statement).map_err(|_| Ed25519Failure::StatementInvalid)?;
    let public_key: &[u8; ED25519_PUBLIC_KEY_BYTES] = public_key_bytes
        .try_into()
        .map_err(|_| Ed25519Failure::PublicKeyLength)?;
    let signature: &[u8; ED25519_SIGNATURE_BYTES] = signature_bytes
        .try_into()
        .map_err(|_| Ed25519Failure::SignatureLength)?;
    let verifying_key =
        VerifyingKey::from_bytes(public_key).map_err(|_| Ed25519Failure::PublicKeyMalformed)?;
    let signature = Signature::from_bytes(signature);
    verifying_key
        .verify(&message, &signature)
        .map_err(|_| Ed25519Failure::SignatureInvalid)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use alloc::vec;

    use artifact_auth_core::{
        ALGORITHM_BLAKE3, AuthenticationScope, ED25519_PUBLIC_KEY_PROFILE_V1, STATEMENT_SCHEMA_V1,
    };
    use ed25519_dalek::{Signer, SigningKey};

    use super::*;

    const TEST_SEED_BYTE: u8 = 7;
    const CHANGED_SEED_BYTE: u8 = 9;
    const GENERATION_ONE: u64 = 1;
    const THRESHOLD_ONE: u16 = 1;

    fn reference(profile: &str, seed: &str) -> ArtifactRef {
        ArtifactRef {
            profile: profile.to_string(),
            algorithm: ALGORITHM_BLAKE3.to_string(),
            digest_hex: blake3::hash(seed.as_bytes()).to_hex().to_string(),
        }
    }

    fn statement(public_key: &[u8]) -> ArtifactStatement {
        ArtifactStatement {
            schema: STATEMENT_SCHEMA_V1.to_string(),
            scope: AuthenticationScope {
                domain: "onix.release-evidence.v1".to_string(),
                purpose: "release-evidence".to_string(),
                profile_id: "onix-artifact-auth.v1".to_string(),
                subject: reference("cairn-receipt.v1", "subject"),
                parents: vec![reference("policy.v1", "policy")],
                verifier_context: reference("verifier-context.v1", "context"),
            },
            producer_id: "cairn".to_string(),
            key_id: "release-key".to_string(),
            key_identity: public_key_identity(public_key),
        }
    }

    // r[verify artifact_auth.ed25519.verify]
    #[test]
    fn matching_signature_verifies_and_binds_full_key_identity() {
        let signing_key = SigningKey::from_bytes(&[TEST_SEED_BYTE; ED25519_PUBLIC_KEY_BYTES]);
        let statement = statement(signing_key.verifying_key().as_bytes());
        let message = canonical_statement_bytes(&statement).expect("valid statement");
        let signature = signing_key.sign(&message);
        let observation = verify_statement(
            &statement,
            signing_key.verifying_key().as_bytes(),
            &signature.to_bytes(),
        );
        assert!(observation.verified);
        assert!(observation.failure_code.is_none());
        assert_eq!(observation.key_identity, statement.key_identity);
        assert_eq!(
            observation.key_identity.profile,
            ED25519_PUBLIC_KEY_PROFILE_V1
        );
    }

    // r[verify artifact_auth.ed25519.verify]
    #[test]
    fn changed_statement_wrong_key_and_malformed_lengths_fail() {
        let signing_key = SigningKey::from_bytes(&[TEST_SEED_BYTE; ED25519_PUBLIC_KEY_BYTES]);
        let wrong_key = SigningKey::from_bytes(&[CHANGED_SEED_BYTE; ED25519_PUBLIC_KEY_BYTES]);
        let statement = statement(signing_key.verifying_key().as_bytes());
        let message = canonical_statement_bytes(&statement).expect("valid statement");
        let signature = signing_key.sign(&message).to_bytes();

        let mut changed = statement.clone();
        changed.scope.domain = "different-domain".to_string();
        let changed_observation =
            verify_statement(&changed, signing_key.verifying_key().as_bytes(), &signature);
        assert_eq!(
            changed_observation.failure_code.as_deref(),
            Some(Ed25519Failure::SignatureInvalid.code())
        );

        let mut changed_subject = statement.clone();
        changed_subject.scope.subject = reference("test-artifact.v1", "changed-subject");
        let changed_subject_observation = verify_statement(
            &changed_subject,
            signing_key.verifying_key().as_bytes(),
            &signature,
        );
        assert_eq!(
            changed_subject_observation.failure_code.as_deref(),
            Some(Ed25519Failure::SignatureInvalid.code())
        );

        let mut changed_parents = statement.clone();
        changed_parents.scope.parents[0] = reference("test-parent.v1", "changed-parent");
        let changed_parents_observation = verify_statement(
            &changed_parents,
            signing_key.verifying_key().as_bytes(),
            &signature,
        );
        assert_eq!(
            changed_parents_observation.failure_code.as_deref(),
            Some(Ed25519Failure::SignatureInvalid.code())
        );

        let wrong_key_observation =
            verify_statement(&statement, wrong_key.verifying_key().as_bytes(), &signature);
        assert_eq!(
            wrong_key_observation.failure_code.as_deref(),
            Some(Ed25519Failure::SignatureInvalid.code())
        );

        let short_key = verify_statement(&statement, &[0_u8; 1], &signature);
        assert_eq!(
            short_key.failure_code.as_deref(),
            Some(Ed25519Failure::PublicKeyLength.code())
        );
        let short_signature = verify_statement(
            &statement,
            signing_key.verifying_key().as_bytes(),
            &[0_u8; 1],
        );
        assert_eq!(
            short_signature.failure_code.as_deref(),
            Some(Ed25519Failure::SignatureLength.code())
        );
    }

    // r[verify artifact_auth.ed25519.verify]
    // r[verify artifact_auth.policy.explicit]
    #[test]
    fn composed_verification_cannot_promote_an_invalid_signature() {
        use artifact_auth_core::{
            AuthenticationPolicy, KeyCurrentness, POLICY_SCHEMA_V1, TrustedKeyObservation,
        };

        let signing_key = SigningKey::from_bytes(&[TEST_SEED_BYTE; ED25519_PUBLIC_KEY_BYTES]);
        let statement = statement(signing_key.verifying_key().as_bytes());
        let message = canonical_statement_bytes(&statement).expect("valid statement");
        let signature = signing_key.sign(&message).to_bytes();
        let policy = AuthenticationPolicy {
            schema: POLICY_SCHEMA_V1.to_string(),
            profile_id: statement.scope.profile_id.clone(),
            threshold: THRESHOLD_ONE,
            trusted_keys: vec![TrustedKeyObservation {
                producer_id: statement.producer_id.clone(),
                key_id: statement.key_id.clone(),
                key_identity: statement.key_identity.clone(),
                allowed_purposes: vec![statement.scope.purpose.clone()],
                generation: GENERATION_ONE,
                currentness: KeyCurrentness::Current,
                currentness_ref: reference("currentness.v1", "current"),
            }],
        };
        let valid = verify_and_evaluate(
            &policy,
            &statement.scope,
            &[Ed25519Evidence {
                statement: &statement,
                generation: GENERATION_ONE,
                public_key_bytes: signing_key.verifying_key().as_bytes(),
                signature_bytes: &signature,
            }],
        );
        assert!(valid.passed, "{:?}", valid.issues);

        let mut invalid_signature = signature;
        invalid_signature[0] ^= 1;
        let invalid = verify_and_evaluate(
            &policy,
            &statement.scope,
            &[Ed25519Evidence {
                statement: &statement,
                generation: GENERATION_ONE,
                public_key_bytes: signing_key.verifying_key().as_bytes(),
                signature_bytes: &invalid_signature,
            }],
        );
        assert!(!invalid.passed);
        assert!(
            invalid
                .issues
                .iter()
                .any(|issue| issue.code == "auth.signature_unverified")
        );
    }

    #[test]
    fn malformed_statement_fails_before_cryptographic_admission() {
        let signing_key = SigningKey::from_bytes(&[TEST_SEED_BYTE; ED25519_PUBLIC_KEY_BYTES]);
        let mut statement = statement(signing_key.verifying_key().as_bytes());
        statement.scope.subject.digest_hex = "malformed".to_string();
        let observation = verify_statement(
            &statement,
            signing_key.verifying_key().as_bytes(),
            &[0_u8; ED25519_SIGNATURE_BYTES],
        );
        assert_eq!(
            observation.failure_code.as_deref(),
            Some(Ed25519Failure::StatementInvalid.code())
        );
    }
}
