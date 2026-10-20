use std::error::Error;
use std::time::Duration;
use std::time::UNIX_EPOCH;

use ucan::AudienceDid;
use ucan::CapabilitySet;
use ucan::ED25519_SECRET_KEY_BYTES;
use ucan::Ed25519SigningKey;
use ucan::IssueRequest;
use ucan::KeyResolutionContext;
use ucan::ProofCollection;
use ucan::ProofReferences;
use ucan::TokenTimeBounds;
use ucan::VerificationContext;
use ucan::VerificationTime;
use ucan::issue_token;
use ucan::verify_compact_token_str;

const ISSUER_KEY_BYTE: u8 = 11;
const AUDIENCE_KEY_BYTE: u8 = 12;
const VERIFY_AT_SECONDS: u64 = 1_500;
const RESOURCE_DID: &str = "did:key:z6Mkresource/photos";
const ABILITY: &str = "store/add";

fn main() -> Result<(), Box<dyn Error>> {
    let issuer_key = Ed25519SigningKey::from_bytes([ISSUER_KEY_BYTE; ED25519_SECRET_KEY_BYTES]);
    let audience_key = Ed25519SigningKey::from_bytes([AUDIENCE_KEY_BYTE; ED25519_SECRET_KEY_BYTES]);
    let audience = AudienceDid::from(audience_key.issuer()?);
    let time_bounds = TokenTimeBounds::from_system_time_and_duration(
        UNIX_EPOCH + Duration::from_secs(1_000),
        Duration::from_secs(1_000),
    )?;

    let request = IssueRequest::builder()
        .signing_key(issuer_key.clone())
        .audience(audience.clone())
        .capabilities(CapabilitySet::single(RESOURCE_DID, ABILITY)?)
        .proofs(ProofReferences::empty())
        .time_bounds(time_bounds)
        .build()?;
    let token = issue_token(&request)?;
    let authorization_header = ("Bearer", token.as_str());
    if authorization_header.0 != "Bearer" {
        return Err("missing bearer scheme".into());
    }
    let token_from_header = authorization_header.1;
    let context = VerificationContext::builder()
        .time(VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS))
        .keys(KeyResolutionContext::new(vec![ucan::Ed25519VerificationKey::from_signing_key(&issuer_key)?]))
        .proofs(ProofCollection::empty())
        .build()?;
    let verified = verify_compact_token_str(token_from_header, &context)?;
    let decision = verified.authorize(RESOURCE_DID, ABILITY);

    println!("authorization header accepted: {}", decision.is_allowed());
    println!("issuer: {}", verified.issuer());
    println!("audience: {audience}");
    Ok(())
}
