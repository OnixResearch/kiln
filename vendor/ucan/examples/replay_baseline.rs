//! Baseline exactly-once replay admission.
//!
//! This example shows the replay admission key and the default posture of the
//! audience-only invocation surface, then demonstrates an in-memory,
//! store-backed `ReplayAdmission` with exactly-once semantics.
//!
//! Run with:
//!
//! ```text
//! cargo run --example replay_baseline
//! ```
//!
//! The admission key is the BLAKE3 digest of the complete bounded compact
//! token. The token format carries no invocation-specific nonce, so identical
//! token bytes plus an identical (resource, ability) pair are
//! indistinguishable to the admission hook. See `docs/invocation-reference.md`
//! for the recorded contract.

use std::cell::RefCell;
use std::collections::HashSet;
use std::error::Error;

use ucan::AllowAllReplayAdmission;
use ucan::AudienceDid;
use ucan::CapabilitySet;
use ucan::CaveatIdentifier;
use ucan::CaveatPolicy;
use ucan::CaveatPolicySet;
use ucan::ED25519_SECRET_KEY_BYTES;
use ucan::Ed25519InMemorySigner;
use ucan::InvocationDenialReason;
use ucan::InvocationRequest;
use ucan::InvocationResult;
use ucan::KeyResolutionContext;
use ucan::ProofCollection;
use ucan::ProofReference;
use ucan::ProofReferences;
use ucan::ReplayAdmission;
use ucan::ReplayAdmissionError;
use ucan::TokenSigner;
use ucan::TokenTimeBounds;
use ucan::VerificationContext;
use ucan::VerificationTime;
use ucan::issue_token_with_signer;
use ucan::proof_reference;
use ucan::verify_invocation_with_replay;

const ISSUER_KEY_BYTE: u8 = 41;
const AUDIENCE_KEY_BYTE: u8 = 43;
const NOT_BEFORE_SECONDS: u64 = 1_000;
const EXPIRES_AT_SECONDS: u64 = 2_000;
const VERIFY_AT_SECONDS: u64 = 1_500;
const RESOURCE_DID: &str = "did:key:z6Mkresource/photos";
const ABILITY: &str = "store/add";

/// No caveat policies. This mirrors the deny-unknown posture for the example.
struct NoCaveatPolicies;

impl CaveatPolicySet for NoCaveatPolicies {
    fn policy_for(&self, _caveat: &CaveatIdentifier) -> Option<&dyn CaveatPolicy> {
        None
    }
}

/// A store-backed exactly-once replay admission hook.
///
/// The store keys an admission by the token digest plus the resource and
/// ability. It rejects a repeat of an already admitted key as a duplicate.
///
/// DURABLE-STORAGE EXTENSION POINT: replace the in-memory `HashSet` with a
/// persistent store (database table, append-only log, or object store) keyed
/// by the same (token digest, resource, ability) triple. Keep
/// `admit_invocation` as the only commit point. Do not perform the admission
/// effect, a counter mutation, or any downstream side effect before this hook
/// succeeds.
#[derive(Default)]
struct InMemoryExactlyOnce {
    admitted: RefCell<HashSet<(ProofReference, String, String)>>,
}

impl ReplayAdmission for InMemoryExactlyOnce {
    fn admit_invocation(
        &self,
        token_reference: &ProofReference,
        resource: &str,
        ability: &str,
    ) -> Result<(), ReplayAdmissionError> {
        let entry = (token_reference.clone(), resource.to_owned(), ability.to_owned());
        if self.admitted.borrow_mut().insert(entry) {
            Ok(())
        } else {
            Err(ReplayAdmissionError::Duplicate {
                reference: token_reference.clone(),
            })
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let issuer = Ed25519InMemorySigner::from_seed_bytes([ISSUER_KEY_BYTE; ED25519_SECRET_KEY_BYTES]);
    let audience_key = Ed25519InMemorySigner::from_seed_bytes([AUDIENCE_KEY_BYTE; ED25519_SECRET_KEY_BYTES]);
    let audience = AudienceDid::from(audience_key.issuer()?);
    let capabilities = CapabilitySet::single(RESOURCE_DID, ABILITY)?;
    let proofs = ProofReferences::empty();
    let time_bounds = TokenTimeBounds::new(NOT_BEFORE_SECONDS, EXPIRES_AT_SECONDS)?;
    let token = issue_token_with_signer(&issuer, &audience, &capabilities, &proofs, time_bounds)?;

    // The admission key is the BLAKE3 digest of the bounded token bytes.
    let reference = proof_reference(&token);
    println!("admission key (bounded-token BLAKE3 digest) bytes: {}", reference.as_bytes().len());
    println!("admission key base64url: {}", reference.to_base64url());

    let keys = KeyResolutionContext::new(vec![issuer.verification_key()?]);
    let context = VerificationContext::new(
        VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
        keys,
        ProofCollection::empty(),
    );
    let policies = NoCaveatPolicies;
    let request = InvocationRequest::new(&token, &context, RESOURCE_DID, ABILITY, &policies);

    // The default audience-only posture accepts every replay.
    let default_first = verify_invocation_with_replay(&request, &AllowAllReplayAdmission);
    let default_second = verify_invocation_with_replay(&request, &AllowAllReplayAdmission);
    assert!(default_first.is_allowed(), "default posture must allow the first invocation");
    assert!(default_second.is_allowed(), "default posture must allow the repeated invocation");
    println!("default posture (AllowAllReplayAdmission): first allowed, repeated allowed");

    // Direct store semantics pin the duplicate error class.
    let admission = InMemoryExactlyOnce::default();
    admission
        .admit_invocation(&reference, RESOURCE_DID, ABILITY)
        .expect("first direct admit must succeed");
    match admission.admit_invocation(&reference, RESOURCE_DID, ABILITY) {
        Err(ReplayAdmissionError::Duplicate { .. }) => {
            println!("direct store duplicate: rejected as ReplayAdmissionError::Duplicate");
        }
        other => panic!("second direct admit must be a Duplicate, got {other:?}"),
    }

    // Exactly-once baseline through the full verification flow.
    let flow = InMemoryExactlyOnce::default();
    let first = verify_invocation_with_replay(&request, &flow);
    assert!(first.is_allowed(), "first verified invocation must be allowed");
    let second = verify_invocation_with_replay(&request, &flow);
    match second {
        InvocationResult::Denied {
            reason: InvocationDenialReason::Replay { message },
        } => println!("second verified invocation: denied as replay ({message})"),
        other => panic!("second verified invocation must be denied as replay, got {other:?}"),
    }

    Ok(())
}
