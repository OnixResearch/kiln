use std::error::Error;

use ucan::AudienceDid;
use ucan::CapabilitySet;
use ucan::ED25519_SECRET_KEY_BYTES;
use ucan::Ed25519InMemorySigner;
use ucan::KeyResolutionContext;
use ucan::ProofCollection;
use ucan::ProofReferences;
use ucan::TokenSigner;
use ucan::TokenTimeBounds;
use ucan::VerificationContext;
use ucan::VerificationTime;
use ucan::issue_token_with_signer;
use ucan::proof_reference;
use ucan::verify_compact_token;

const ROOT_KEY_BYTE: u8 = 7;
const CHILD_KEY_BYTE: u8 = 9;
const AUDIENCE_KEY_BYTE: u8 = 11;
const ROOT_NOT_BEFORE_SECONDS: u64 = 1_000;
const ROOT_EXPIRES_AT_SECONDS: u64 = 2_000;
const CHILD_NOT_BEFORE_SECONDS: u64 = 1_100;
const CHILD_EXPIRES_AT_SECONDS: u64 = 1_900;
const VERIFY_AT_SECONDS: u64 = 1_500;
const ROOT_RESOURCE_DID: &str = "did:key:z6Mkresource";
const CHILD_RESOURCE_DID: &str = "did:key:z6Mkresource/photos";
const ROOT_ABILITY: &str = "store/*";
const CHILD_ABILITY: &str = "store/add";

fn main() -> Result<(), Box<dyn Error>> {
    let root = Ed25519InMemorySigner::from_seed_bytes([ROOT_KEY_BYTE; ED25519_SECRET_KEY_BYTES]);
    let child = Ed25519InMemorySigner::from_seed_bytes([CHILD_KEY_BYTE; ED25519_SECRET_KEY_BYTES]);
    let audience_key = Ed25519InMemorySigner::from_seed_bytes([AUDIENCE_KEY_BYTE; ED25519_SECRET_KEY_BYTES]);

    let parent_token = issue_token_with_signer(
        &root,
        &AudienceDid::from(child.issuer()?),
        &CapabilitySet::single(ROOT_RESOURCE_DID, ROOT_ABILITY)?,
        &ProofReferences::empty(),
        TokenTimeBounds::new(ROOT_NOT_BEFORE_SECONDS, ROOT_EXPIRES_AT_SECONDS)?,
    )?;
    let parent_reference = proof_reference(&parent_token);
    let child_token = issue_token_with_signer(
        &child,
        &AudienceDid::from(audience_key.issuer()?),
        &CapabilitySet::single(CHILD_RESOURCE_DID, CHILD_ABILITY)?,
        &ProofReferences::from_references(vec![parent_reference]),
        TokenTimeBounds::new(CHILD_NOT_BEFORE_SECONDS, CHILD_EXPIRES_AT_SECONDS)?,
    )?;

    let keys = KeyResolutionContext::new(vec![root.verification_key()?, child.verification_key()?]);
    let context = VerificationContext::new(
        VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
        keys,
        ProofCollection::from_tokens(vec![parent_token]),
    );
    let verified = verify_compact_token(&child_token, &context)?;

    println!("child issuer: {}", verified.issuer());
    println!(
        "effective window: {}..{}",
        verified.effective_delegation().time_bounds().not_before_seconds(),
        verified.effective_delegation().time_bounds().expires_at_seconds()
    );
    println!("direct proof count: {}", verified.proofs().as_slice().len());
    Ok(())
}
