//! Demonstrates a two-key release threshold with accepted and rejected evidence.
//!
//! Signing below creates deterministic fixture data; signing remains consumer-owned.

use std::process::ExitCode;

use artifact_auth_core::{
    ALGORITHM_BLAKE3, ArtifactRef, ArtifactStatement, AuthenticationDecision, AuthenticationPolicy,
    AuthenticationScope, KeyCurrentness, POLICY_SCHEMA_V1, STATEMENT_SCHEMA_V1,
    TrustedKeyObservation, canonical_statement_bytes,
};
use artifact_auth_ed25519::{
    ED25519_PUBLIC_KEY_BYTES, Ed25519Evidence, public_key_identity, verify_and_evaluate,
};
use ed25519_dalek::{Signer, SigningKey};

const SIGNER_ONE_SEED_BYTE: u8 = 7;
const SIGNER_TWO_SEED_BYTE: u8 = 9;
const KEY_GENERATION: u64 = 1;
const TWO_SIGNER_THRESHOLD: u16 = 2;
const PROFILE_ID: &str = "example-artifact-auth.v1";
const RELEASE_PURPOSE: &str = "release-approval";
const THRESHOLD_ISSUE: &str = "auth.distinct_key_threshold_unsatisfied";

struct ExampleOutcome {
    accepted: AuthenticationDecision,
    rejected: AuthenticationDecision,
}

fn reference(profile: &str, seed: &str) -> ArtifactRef {
    ArtifactRef {
        profile: profile.to_string(),
        algorithm: ALGORITHM_BLAKE3.to_string(),
        digest_hex: blake3::hash(seed.as_bytes()).to_hex().to_string(),
    }
}

fn release_scope() -> AuthenticationScope {
    AuthenticationScope {
        domain: "example.release.v1".to_string(),
        purpose: RELEASE_PURPOSE.to_string(),
        profile_id: PROFILE_ID.to_string(),
        subject: reference("release-archive.v1", "app-v2.tar.gz bytes"),
        parents: vec![reference("build-receipt.v1", "build receipt")],
        verifier_context: reference("release-policy.v1", "production release policy"),
    }
}

fn signer_statement(
    scope: &AuthenticationScope,
    key_id: &str,
    public_key_bytes: &[u8],
) -> ArtifactStatement {
    ArtifactStatement {
        schema: STATEMENT_SCHEMA_V1.to_string(),
        scope: scope.clone(),
        producer_id: "example-release-team".to_string(),
        key_id: key_id.to_string(),
        key_identity: public_key_identity(public_key_bytes),
    }
}

fn trusted_key(statement: &ArtifactStatement, currentness_seed: &str) -> TrustedKeyObservation {
    TrustedKeyObservation {
        producer_id: statement.producer_id.clone(),
        key_id: statement.key_id.clone(),
        key_identity: statement.key_identity.clone(),
        allowed_purposes: vec![RELEASE_PURPOSE.to_string()],
        generation: KEY_GENERATION,
        currentness: KeyCurrentness::Current,
        currentness_ref: reference("key-currentness.v1", currentness_seed),
    }
}

fn authentication_policy(
    signer_one: &ArtifactStatement,
    signer_two: &ArtifactStatement,
) -> AuthenticationPolicy {
    AuthenticationPolicy {
        schema: POLICY_SCHEMA_V1.to_string(),
        profile_id: PROFILE_ID.to_string(),
        threshold: TWO_SIGNER_THRESHOLD,
        trusted_keys: vec![
            trusted_key(signer_one, "signer one is current"),
            trusted_key(signer_two, "signer two is current"),
        ],
    }
}

fn canonical_bytes(statement: &ArtifactStatement) -> Result<Vec<u8>, String> {
    canonical_statement_bytes(statement)
        .map_err(|issues| format!("example statement was invalid: {issues:?}"))
}

fn evaluate_example() -> Result<ExampleOutcome, String> {
    // Signing remains consumer-owned. These deterministic keys only make the example runnable.
    let signer_one = SigningKey::from_bytes(&[SIGNER_ONE_SEED_BYTE; ED25519_PUBLIC_KEY_BYTES]);
    let signer_two = SigningKey::from_bytes(&[SIGNER_TWO_SEED_BYTE; ED25519_PUBLIC_KEY_BYTES]);
    let signer_one_public = signer_one.verifying_key();
    let signer_two_public = signer_two.verifying_key();

    let scope = release_scope();
    let statement_one = signer_statement(&scope, "release-key-one", signer_one_public.as_bytes());
    let statement_two = signer_statement(&scope, "release-key-two", signer_two_public.as_bytes());
    let policy = authentication_policy(&statement_one, &statement_two);

    let signature_one = signer_one
        .sign(&canonical_bytes(&statement_one)?)
        .to_bytes();
    let signature_two = signer_two
        .sign(&canonical_bytes(&statement_two)?)
        .to_bytes();
    let evidence_one = Ed25519Evidence {
        statement: &statement_one,
        generation: KEY_GENERATION,
        public_key_bytes: signer_one_public.as_bytes(),
        signature_bytes: &signature_one,
    };
    let evidence_two = Ed25519Evidence {
        statement: &statement_two,
        generation: KEY_GENERATION,
        public_key_bytes: signer_two_public.as_bytes(),
        signature_bytes: &signature_two,
    };

    let accepted = verify_and_evaluate(&policy, &scope, &[evidence_one, evidence_two]);
    if !accepted.passed {
        return Err(format!(
            "two valid signatures should pass: {:?}",
            accepted.issues
        ));
    }

    let rejected = verify_and_evaluate(&policy, &scope, &[evidence_one]);
    if rejected.passed {
        return Err("one signature unexpectedly satisfied a two-key threshold".to_string());
    }
    if !rejected
        .issues
        .iter()
        .any(|issue| issue.code == THRESHOLD_ISSUE)
    {
        return Err(format!(
            "rejection omitted the threshold issue: {:?}",
            rejected.issues
        ));
    }

    Ok(ExampleOutcome { accepted, rejected })
}

fn main() -> ExitCode {
    match evaluate_example() {
        Ok(outcome) => {
            println!(
                "accepted: {} distinct current keys satisfied threshold {TWO_SIGNER_THRESHOLD}",
                outcome.accepted.verified_key_blake3.len()
            );
            println!(
                "rejected: {} valid signature did not satisfy threshold {TWO_SIGNER_THRESHOLD}",
                outcome.rejected.verified_key_blake3.len()
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("example failed: {error}");
            ExitCode::FAILURE
        }
    }
}
