use std::error::Error;

use ucan::AudienceDid;
use ucan::CapabilitySet;
use ucan::ED25519_SECRET_KEY_BYTES;
use ucan::Ed25519InMemorySigner;
use ucan::KeyResolutionContext;
use ucan::NoRevocations;
use ucan::ProofCollection;
use ucan::ProofReference;
use ucan::ProofReferences;
use ucan::RevocationChecker;
use ucan::RevocationError;
use ucan::RevocationList;
use ucan::TokenError;
use ucan::TokenSigner;
use ucan::TokenTimeBounds;
use ucan::VerificationLimits;
use ucan::VerificationTime;
use ucan::issue_token_with_signer;
use ucan::token::proof_reference;
use ucan::verify_compact_token_with_resolvers_and_revocations;

const ROOT_KEY_BYTE: u8 = 13;
const CHILD_KEY_BYTE: u8 = 17;
const AUDIENCE_KEY_BYTE: u8 = 19;
const ROOT_NOT_BEFORE_SECONDS: u64 = 1_000;
const ROOT_EXPIRES_AT_SECONDS: u64 = 2_000;
const CHILD_NOT_BEFORE_SECONDS: u64 = 1_100;
const CHILD_EXPIRES_AT_SECONDS: u64 = 1_900;
const VERIFY_AT_SECONDS: u64 = 1_500;
const ROOT_RESOURCE_DID: &str = "did:key:z6Mkresource";
const CHILD_RESOURCE_DID: &str = "did:key:z6Mkresource/photos";
const ROOT_ABILITY: &str = "store/*";
const CHILD_ABILITY: &str = "store/add";

struct UnavailableRevocationBackend;

impl RevocationChecker for UnavailableRevocationBackend {
    fn is_revoked(&self, reference: &ProofReference) -> Result<bool, RevocationError> {
        Err(RevocationError::Backend {
            reference: reference.clone(),
            message: "revocation store unavailable".to_owned(),
        })
    }
}

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
        &ProofReferences::from_references(vec![parent_reference.clone()]),
        TokenTimeBounds::new(CHILD_NOT_BEFORE_SECONDS, CHILD_EXPIRES_AT_SECONDS)?,
    )?;

    let keys = KeyResolutionContext::new(vec![root.verification_key()?, child.verification_key()?]);
    let proofs = ProofCollection::from_tokens(vec![parent_token]);
    let verification_time = VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS);

    let verified = verify_compact_token_with_resolvers_and_revocations(
        &child_token,
        verification_time,
        &keys,
        &proofs,
        &NoRevocations,
        VerificationLimits::default(),
    )?;
    println!("verified child issuer: {}", verified.issuer());
    println!("direct proof count: {}", verified.proofs().as_slice().len());

    let revoked_parent = RevocationList::from_references(vec![parent_reference.clone()]);
    let error = verify_compact_token_with_resolvers_and_revocations(
        &child_token,
        verification_time,
        &keys,
        &proofs,
        &revoked_parent,
        VerificationLimits::default(),
    )
    .expect_err("revoked parent proof must fail verification");

    match error {
        TokenError::Revoked { reference } if reference == parent_reference => {
            println!("revoked parent proof denied: {reference}");
        }
        other => return Err(format!("unexpected verification error: {other}").into()),
    }

    let error = verify_compact_token_with_resolvers_and_revocations(
        &child_token,
        verification_time,
        &keys,
        &proofs,
        &UnavailableRevocationBackend,
        VerificationLimits::default(),
    )
    .expect_err("unavailable revocation backend must fail closed");

    match error {
        TokenError::Revocation {
            source: RevocationError::Backend { reference, message },
        } => {
            println!("revocation backend failed closed for {reference}: {message}");
        }
        other => return Err(format!("unexpected verification error: {other}").into()),
    }

    Ok(())
}
