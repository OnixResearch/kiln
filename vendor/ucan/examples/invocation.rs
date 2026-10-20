use std::cell::RefCell;
use std::collections::HashSet;
use std::error::Error;

use ucan::AudienceDid;
use ucan::AuthorizationRequest;
use ucan::CapabilitySet;
use ucan::CaveatDecision;
use ucan::CaveatDocument;
use ucan::CaveatIdentifier;
use ucan::CaveatPolicy;
use ucan::CaveatPolicySet;
use ucan::ED25519_SECRET_KEY_BYTES;
use ucan::Ed25519InMemorySigner;
use ucan::HolderInvocationLimits;
use ucan::HolderInvocationNonce;
use ucan::HolderInvocationPolicyContext;
use ucan::HolderInvocationReplayAdmission;
use ucan::HolderInvocationReplayCandidate;
use ucan::HolderInvocationReplayError;
use ucan::HolderInvocationRequest;
use ucan::HolderInvocationSigningRequest;
use ucan::HolderTokenVerificationContext;
use ucan::KeyResolutionContext;
use ucan::NoRevocations;
use ucan::OperationBinding;
use ucan::OperationProfileAdmission;
use ucan::OperationProfileAdmissionError;
use ucan::OperationProfileId;
use ucan::ProofCollection;
use ucan::ProofReferences;
use ucan::RequestBindingDigest;
use ucan::TokenSigner;
use ucan::TokenTimeBounds;
use ucan::VerificationLimits;
use ucan::VerificationTime;
use ucan::issue_token_with_signer;
use ucan::sign_holder_invocation;
use ucan::verify_holder_signed_invocation;

const ISSUER_KEY_BYTE: u8 = 7;
const AUDIENCE_KEY_BYTE: u8 = 9;
const INVOCATION_NONCE_BYTE: u8 = 11;
const VERIFY_AT_SECONDS: u64 = 1_500;
const TOKEN_NOT_BEFORE_SECONDS: u64 = 1_000;
const TOKEN_EXPIRES_SECONDS: u64 = 2_000;
const RESOURCE: &str = "did:key:z6Mkresource/photos";
const ABILITY: &str = "store/add";
const OPERATION_PROFILE: &[u8] = b"basalt.closed-action.store-add.v1";
const DOWNSTREAM_REQUEST: &[u8] = b"basalt-request:target=photos;action=store-add";

struct PayloadEqualsPolicy;

impl CaveatPolicy for PayloadEqualsPolicy {
    fn evaluate(&self, caveat: &CaveatDocument, _request: &AuthorizationRequest) -> CaveatDecision {
        if caveat.payload() == b"allow" {
            CaveatDecision::Satisfied
        } else {
            CaveatDecision::Rejected {
                message: "payload did not equal allow".to_owned(),
            }
        }
    }
}

struct ExamplePolicies {
    identity: CaveatIdentifier,
    policy: PayloadEqualsPolicy,
}

impl CaveatPolicySet for ExamplePolicies {
    fn policy_for(&self, caveat: &CaveatIdentifier) -> Option<&dyn CaveatPolicy> {
        if caveat == &self.identity {
            Some(&self.policy)
        } else {
            None
        }
    }
}

struct ExampleProfiles {
    known: OperationProfileId,
}

impl OperationProfileAdmission for ExampleProfiles {
    fn admit_operation_profile(&self, profile: &OperationProfileId) -> Result<(), OperationProfileAdmissionError> {
        if profile == &self.known {
            Ok(())
        } else {
            Err(OperationProfileAdmissionError::UnknownProfile)
        }
    }
}

#[derive(Default)]
struct ExampleReplayAdmission {
    seen: RefCell<HashSet<(String, [u8; blake3::OUT_LEN])>>,
}

impl HolderInvocationReplayAdmission for ExampleReplayAdmission {
    fn admit_holder_invocation(
        &self,
        candidate: &HolderInvocationReplayCandidate<'_>,
    ) -> Result<(), HolderInvocationReplayError> {
        let key = (candidate.audience().as_str().to_owned(), *candidate.nonce().as_bytes());
        if self.seen.borrow_mut().insert(key) {
            Ok(())
        } else {
            Err(HolderInvocationReplayError::Duplicate)
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let issuer = Ed25519InMemorySigner::from_seed_bytes([ISSUER_KEY_BYTE; ED25519_SECRET_KEY_BYTES]);
    let audience_key = Ed25519InMemorySigner::from_seed_bytes([AUDIENCE_KEY_BYTE; ED25519_SECRET_KEY_BYTES]);
    let audience = AudienceDid::from(audience_key.issuer()?);
    let caveat =
        CaveatDocument::new("example".to_owned(), "payload-equals".to_owned(), "gate".to_owned(), b"allow".to_vec())?;
    let capabilities = CapabilitySet::new(vec![ucan::CapabilityDocument::with_caveats(
        "did:key:z6Mkresource".to_owned(),
        "store/*".to_owned(),
        vec![caveat],
    )?])?;
    let token = issue_token_with_signer(
        &issuer,
        &audience,
        &capabilities,
        &ProofReferences::empty(),
        TokenTimeBounds::new(TOKEN_NOT_BEFORE_SECONDS, TOKEN_EXPIRES_SECONDS)?,
    )?;
    let keys = KeyResolutionContext::new(vec![issuer.verification_key()?, audience_key.verification_key()?]);
    let proofs = ProofCollection::empty();
    let revocations = NoRevocations;
    let policies = ExamplePolicies {
        identity: CaveatIdentifier::new("example", "payload-equals", "gate")?,
        policy: PayloadEqualsPolicy,
    };
    let operation_profile = OperationProfileId::from_bytes(*blake3::hash(OPERATION_PROFILE).as_bytes());
    let operation = OperationBinding::new(
        operation_profile,
        RequestBindingDigest::from_bytes(*blake3::hash(DOWNSTREAM_REQUEST).as_bytes()),
    );
    let nonce = HolderInvocationNonce::from_bytes([INVOCATION_NONCE_BYTE; blake3::OUT_LEN]);
    let signature = sign_holder_invocation(
        &HolderInvocationSigningRequest::new(token.as_str(), &audience, RESOURCE, ABILITY, operation, nonce),
        &audience_key,
        VerificationLimits::default(),
        HolderInvocationLimits::default(),
    )?;
    let profiles = ExampleProfiles {
        known: operation_profile,
    };
    let replay = ExampleReplayAdmission::default();
    let token_context = HolderTokenVerificationContext::new(
        VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
        &keys,
        &proofs,
        &revocations,
        VerificationLimits::default(),
    );
    let policy_context =
        HolderInvocationPolicyContext::new(&policies, &profiles, &replay, HolderInvocationLimits::default());
    let request = HolderInvocationRequest::new(token.as_str(), RESOURCE, ABILITY, operation, nonce, &signature);
    let authorized = verify_holder_signed_invocation(&request, &token_context, &policy_context)?;

    println!("holder-signed invocation allowed for {} with {}", authorized.audience(), authorized.ability());
    Ok(())
}
