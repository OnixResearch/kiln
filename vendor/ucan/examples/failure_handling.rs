use std::error::Error;

use ucan::AudienceDid;
use ucan::CapabilitySet;
use ucan::CompactToken;
use ucan::ED25519_SECRET_KEY_BYTES;
use ucan::Ed25519SigningKey;
use ucan::Ed25519VerificationKey;
use ucan::IssueRequest;
use ucan::KeyResolutionContext;
use ucan::ProofCollection;
use ucan::ProofReferences;
use ucan::ProofStoreError;
use ucan::TokenError;
use ucan::TokenTimeBounds;
use ucan::VerificationContext;
use ucan::VerificationTime;
use ucan::issue_token;
use ucan::proof_reference;
use ucan::verify_compact_token;
use ucan::verify_compact_token_str;

const ISSUER_KEY_BYTE: u8 = 21;
const AUDIENCE_KEY_BYTE: u8 = 22;
const CHILD_KEY_BYTE: u8 = 23;
const NOT_BEFORE_SECONDS: u64 = 1_000;
const EXPIRES_AT_SECONDS: u64 = 2_000;
const EXPIRED_AT_SECONDS: u64 = 1_100;
const VERIFY_AT_SECONDS: u64 = 1_500;
const RESOURCE_DID: &str = "did:key:z6Mkresource/photos";
const ABILITY: &str = "store/add";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApplicationOutcome {
    Accepted,
    BadBearerToken,
    ExpiredCredential,
    MissingProofMaterial,
    UnknownIssuer,
}

fn main() -> Result<(), Box<dyn Error>> {
    let issuer_key = Ed25519SigningKey::from_bytes([ISSUER_KEY_BYTE; ED25519_SECRET_KEY_BYTES]);
    let audience_key = Ed25519SigningKey::from_bytes([AUDIENCE_KEY_BYTE; ED25519_SECRET_KEY_BYTES]);
    let child_key = Ed25519SigningKey::from_bytes([CHILD_KEY_BYTE; ED25519_SECRET_KEY_BYTES]);
    let audience = AudienceDid::from(audience_key.issuer()?);
    let child_audience = AudienceDid::from(child_key.issuer()?);

    let valid = issue_simple_token(
        &issuer_key,
        audience.clone(),
        TokenTimeBounds::new(NOT_BEFORE_SECONDS, EXPIRES_AT_SECONDS)?,
    )?;
    let expired =
        issue_simple_token(&issuer_key, audience, TokenTimeBounds::new(NOT_BEFORE_SECONDS, EXPIRED_AT_SECONDS)?)?;
    let parent_proof = issue_simple_token(
        &issuer_key,
        AudienceDid::from(audience_key.issuer()?),
        TokenTimeBounds::new(NOT_BEFORE_SECONDS, EXPIRES_AT_SECONDS)?,
    )?;
    let parent_reference = proof_reference(&parent_proof);
    drop(parent_proof);
    let missing_proof_request = IssueRequest::builder()
        .signing_key(audience_key.clone())
        .audience(child_audience)
        .capabilities(CapabilitySet::single(RESOURCE_DID, ABILITY)?)
        .proofs(ProofReferences::from_references(vec![parent_reference]))
        .time_bounds(TokenTimeBounds::new(NOT_BEFORE_SECONDS, EXPIRES_AT_SECONDS)?)
        .build()?;
    let missing_proof_child = issue_token(&missing_proof_request)?;

    let context = VerificationContext::new(
        VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
        KeyResolutionContext::new(vec![
            Ed25519VerificationKey::from_signing_key(&issuer_key)?,
            Ed25519VerificationKey::from_signing_key(&audience_key)?,
        ]),
        ProofCollection::empty(),
    );
    let missing_key_context = VerificationContext::new(
        VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
        KeyResolutionContext::new(Vec::new()),
        ProofCollection::empty(),
    );

    assert_eq!(map_verification_str(" not-a-token ", &context), ApplicationOutcome::BadBearerToken);
    assert_eq!(map_verification(&expired, &context), ApplicationOutcome::ExpiredCredential);
    assert_eq!(map_verification(&missing_proof_child, &context), ApplicationOutcome::MissingProofMaterial);
    assert_eq!(map_verification(&valid, &missing_key_context), ApplicationOutcome::UnknownIssuer);
    assert_eq!(map_verification(&valid, &context), ApplicationOutcome::Accepted);

    println!("malformed token -> {:?}", ApplicationOutcome::BadBearerToken);
    println!("expired token -> {:?}", ApplicationOutcome::ExpiredCredential);
    println!("missing proof -> {:?}", ApplicationOutcome::MissingProofMaterial);
    println!("missing key -> {:?}", ApplicationOutcome::UnknownIssuer);
    println!("valid token -> {:?}", ApplicationOutcome::Accepted);
    Ok(())
}

fn issue_simple_token(
    signing_key: &Ed25519SigningKey,
    audience: AudienceDid,
    time_bounds: TokenTimeBounds,
) -> Result<CompactToken, Box<dyn Error>> {
    let request = IssueRequest::builder()
        .signing_key(signing_key.clone())
        .audience(audience)
        .capabilities(CapabilitySet::single(RESOURCE_DID, ABILITY)?)
        .proofs(ProofReferences::empty())
        .time_bounds(time_bounds)
        .build()?;
    Ok(issue_token(&request)?)
}

fn map_verification_str(token: &str, context: &VerificationContext) -> ApplicationOutcome {
    match verify_compact_token_str(token, context) {
        Ok(_) => ApplicationOutcome::Accepted,
        Err(error) => map_error(&error),
    }
}

fn map_verification(token: &CompactToken, context: &VerificationContext) -> ApplicationOutcome {
    match verify_compact_token(token, context) {
        Ok(_) => ApplicationOutcome::Accepted,
        Err(error) => map_error(&error),
    }
}

const fn map_error(error: &TokenError) -> ApplicationOutcome {
    match error {
        TokenError::CompactTokenMustBeTrimmed | TokenError::InvalidCompactTokenStructure { .. } => {
            ApplicationOutcome::BadBearerToken
        }
        TokenError::Temporal { .. } => ApplicationOutcome::ExpiredCredential,
        TokenError::ProofStore {
            source: ProofStoreError::MissingProof { .. },
        } => ApplicationOutcome::MissingProofMaterial,
        TokenError::MissingVerificationKey { .. } => ApplicationOutcome::UnknownIssuer,
        _ => ApplicationOutcome::BadBearerToken,
    }
}
