use std::collections::BTreeMap;
use std::error::Error;

use ucan::AudienceDid;
use ucan::CapabilitySet;
use ucan::CompactToken;
use ucan::ED25519_SECRET_KEY_BYTES;
use ucan::Ed25519InMemorySigner;
use ucan::KeyResolutionContext;
use ucan::ProofReferences;
use ucan::ProofToken;
use ucan::TokenSigner;
use ucan::TokenTimeBounds;
use ucan::VerificationTime;
use ucan::issue_token_with_signer;
use ucan::token::ProofReference;
use ucan::token::ProofStore;
use ucan::token::ProofStoreError;
use ucan::token::VerificationLimits;
use ucan::token::proof_reference;
use ucan::token::verify_compact_token_with_resolvers;

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

#[derive(Default)]
struct MapProofStore {
    tokens: BTreeMap<String, CompactToken>,
}

impl MapProofStore {
    fn from_tokens(tokens: Vec<CompactToken>) -> Self {
        let mut store = Self::default();
        for token in tokens {
            store.tokens.insert(proof_reference(&token).to_base64url(), token);
        }
        store
    }
}

impl ProofStore for MapProofStore {
    fn resolve_proof<'a>(&'a self, reference: &ProofReference) -> Result<ProofToken<'a>, ProofStoreError> {
        self.tokens.get(reference.to_base64url().as_str()).map(ProofToken::Borrowed).ok_or_else(|| {
            ProofStoreError::MissingProof {
                reference: reference.clone(),
            }
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
        &ProofReferences::from_references(vec![parent_reference]),
        TokenTimeBounds::new(CHILD_NOT_BEFORE_SECONDS, CHILD_EXPIRES_AT_SECONDS)?,
    )?;

    let keys = KeyResolutionContext::new(vec![root.verification_key()?, child.verification_key()?]);
    let proofs = MapProofStore::from_tokens(vec![parent_token]);
    let verified = verify_compact_token_with_resolvers(
        &child_token,
        VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
        &keys,
        &proofs,
        VerificationLimits::default(),
    )?;

    println!("verified child issuer: {}", verified.issuer());
    println!("proof store resolved {} direct proof", verified.proofs().as_slice().len());
    Ok(())
}
