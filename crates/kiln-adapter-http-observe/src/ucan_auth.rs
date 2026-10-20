use std::sync::Mutex;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Serialize;
use ucan::token::DenyUnknownCaveats;
use ucan::{
    CompactToken, Ed25519PublicKey, Ed25519VerificationKey, HolderInvocationLimits,
    HolderInvocationNonce, HolderInvocationPolicyContext, HolderInvocationReplayAdmission,
    HolderInvocationReplayCandidate, HolderInvocationReplayError, HolderInvocationRequest,
    HolderTokenVerificationContext, IssuerDid, KeyResolutionContext, OperationBinding,
    OperationProfileAdmission, OperationProfileAdmissionError, OperationProfileId, ProofCollection,
    ProofReference, RequestBindingDigest, RevocationList, SignerSignature, VerificationLimits,
    VerificationTime, parse_compact_token, proof_reference, validate_decoded_token,
    verify_holder_signed_invocation,
};

use crate::auth::{AuthRequest, ReplayState, body_digest, parse_principal};
use crate::profile::{AuthProfile, UCAN_AUTH_KIND, UCAN_OBSERVATION_OPERATION_PROFILE};
use crate::{ObserveError, ObserveResult};

const UCAN_HEADER_SCHEME: &str = "UCAN ";
const HOLDER_NONCE_BYTES: usize = 32;
const HOLDER_SIGNATURE_BYTES: usize = 64;

#[derive(Serialize)]
struct RequestBinding<'a> {
    method: &'a str,
    target: &'a str,
    body_blake3: &'a str,
    state_revision: u64,
}

struct ObservationOperationProfile {
    expected: OperationProfileId,
}

impl OperationProfileAdmission for ObservationOperationProfile {
    fn admit_operation_profile(
        &self,
        profile: &OperationProfileId,
    ) -> Result<(), OperationProfileAdmissionError> {
        if profile == &self.expected {
            Ok(())
        } else {
            Err(OperationProfileAdmissionError::UnknownProfile)
        }
    }
}

struct UcanReplay<'a> {
    state: &'a Mutex<ReplayState>,
    now_unix: u64,
}

impl HolderInvocationReplayAdmission for UcanReplay<'_> {
    fn admit_holder_invocation(
        &self,
        candidate: &HolderInvocationReplayCandidate<'_>,
    ) -> Result<(), HolderInvocationReplayError> {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.admit_ucan(candidate, self.now_unix)
    }
}

struct ProofNode {
    reference: Vec<u8>,
    issuer: String,
    audience: String,
    proofs: Vec<Vec<u8>>,
}

fn proof_nodes(auth: &AuthProfile) -> ObserveResult<Vec<ProofNode>> {
    let mut nodes = Vec::with_capacity(auth.proof_tokens.len());
    for token in &auth.proof_tokens {
        let compact = CompactToken::new(token).map_err(|_| {
            ObserveError::new("invalid_proof_tokens", "UCAN proof token is malformed")
        })?;
        let decoded = parse_compact_token(&compact).map_err(|_| {
            ObserveError::new("invalid_proof_tokens", "UCAN proof token is malformed")
        })?;
        let valid = validate_decoded_token(decoded).map_err(|_| {
            ObserveError::new(
                "invalid_proof_tokens",
                "UCAN proof token claims are invalid",
            )
        })?;
        let claims = valid.claims();
        nodes.push(ProofNode {
            reference: proof_reference(&compact).as_bytes().to_vec(),
            issuer: claims.issuer_did().to_owned(),
            audience: claims.audience_did().to_owned(),
            proofs: claims
                .proofs()
                .iter()
                .map(|reference| reference.as_bytes().to_vec())
                .collect(),
        });
    }
    Ok(nodes)
}

fn anchored_nodes(nodes: &[ProofNode], root: &str) -> Vec<bool> {
    let mut anchored = nodes
        .iter()
        .map(|node| node.issuer == root)
        .collect::<Vec<_>>();
    loop {
        let mut changed = false;
        for (index, node) in nodes.iter().enumerate() {
            let Some(is_anchored) = anchored.get(index) else {
                continue;
            };
            if *is_anchored {
                continue;
            }
            let linked = node.proofs.iter().any(|reference| {
                nodes
                    .iter()
                    .zip(&anchored)
                    .any(|(parent, parent_anchored)| {
                        *parent_anchored
                            && parent.reference == *reference
                            && parent.audience == node.issuer
                    })
            });
            if linked && let Some(slot) = anchored.get_mut(index) {
                *slot = true;
                changed = true;
            }
        }
        if !changed {
            return anchored;
        }
    }
}

// r[impl kiln.observe.http.ucan_profile.scenario.anchor]
pub(crate) fn verify_ucan_proof_anchors(auth: &AuthProfile) -> ObserveResult<()> {
    let nodes = proof_nodes(auth)?;
    let anchored = anchored_nodes(&nodes, &auth.issuer);
    if anchored.iter().all(|value| *value) {
        Ok(())
    } else {
        Err(ObserveError::new(
            "invalid_ucan_anchor",
            "UCAN proof token is not anchored to the profile root",
        ))
    }
}

// r[impl kiln.observe.http.ucan_holder.scenario.root]
fn verify_ucan_root_anchor(auth: &AuthProfile, compact: &CompactToken) -> ObserveResult<()> {
    let decoded = parse_compact_token(compact)
        .map_err(|_| ObserveError::new("ucan_token", "UCAN compact token is invalid"))?;
    let valid = validate_decoded_token(decoded)
        .map_err(|_| ObserveError::new("ucan_token", "UCAN compact token claims are invalid"))?;
    let claims = valid.claims();
    if claims.issuer_did() == auth.issuer {
        return Ok(());
    }
    let nodes = proof_nodes(auth)?;
    let anchored = anchored_nodes(&nodes, &auth.issuer);
    let linked = claims.proofs().iter().any(|reference| {
        nodes.iter().zip(&anchored).any(|(node, is_anchored)| {
            *is_anchored
                && node.reference == reference.as_bytes()
                && node.audience == claims.issuer_did()
        })
    });
    if linked {
        Ok(())
    } else {
        Err(ObserveError::new(
            "ucan_denied",
            "UCAN token is not anchored to the profile root",
        ))
    }
}

// r[impl kiln.observe.http.ucan_holder]
// r[impl kiln.observe.http.ucan_holder.scenario.denied]
// r[impl kiln.observe.http.ucan_replay]
// r[impl kiln.observe.http.ucan_nonclaims]
pub fn verify_ucan_header(
    header: &str,
    auth: &AuthProfile,
    request: &AuthRequest<'_>,
    now_unix: u64,
    replay: &Mutex<ReplayState>,
) -> ObserveResult<()> {
    if auth.kind != UCAN_AUTH_KIND {
        return Err(ObserveError::new(
            "unsupported_auth_kind",
            "UCAN verifier received another auth kind",
        ));
    }
    let token = header
        .strip_prefix(UCAN_HEADER_SCHEME)
        .ok_or_else(|| ObserveError::new("ucan_shape", "authorization scheme is not UCAN"))?;
    let (token_part, rest) = token
        .split_once('.')
        .ok_or_else(|| ObserveError::new("ucan_shape", "UCAN authorization token is malformed"))?;
    let (signature_part, nonce_part) = rest
        .split_once('.')
        .ok_or_else(|| ObserveError::new("ucan_shape", "UCAN authorization token is malformed"))?;
    let max_token = usize::try_from(auth.max_token_bytes).unwrap_or(usize::MAX);
    if token_part.len() > max_token || signature_part.len() > max_token {
        return Err(ObserveError::new(
            "ucan_bound",
            "UCAN authorization field exceeds its bound",
        ));
    }
    let token_bytes = URL_SAFE_NO_PAD
        .decode(token_part)
        .map_err(|_| ObserveError::new("ucan_shape", "UCAN compact token is not base64url"))?;
    let token_text = std::str::from_utf8(&token_bytes)
        .map_err(|_| ObserveError::new("ucan_shape", "UCAN compact token is not UTF-8"))?;
    let compact = CompactToken::new(token_text)
        .map_err(|_| ObserveError::new("ucan_token", "UCAN compact token is invalid"))?;
    verify_ucan_root_anchor(auth, &compact)?;
    let signature_bytes = URL_SAFE_NO_PAD
        .decode(signature_part)
        .map_err(|_| ObserveError::new("ucan_shape", "UCAN holder signature is not base64url"))?;
    let nonce_bytes = URL_SAFE_NO_PAD
        .decode(nonce_part)
        .map_err(|_| ObserveError::new("ucan_shape", "UCAN holder nonce is not base64url"))?;
    if signature_bytes.len() != HOLDER_SIGNATURE_BYTES || nonce_bytes.len() != HOLDER_NONCE_BYTES {
        return Err(ObserveError::new(
            "ucan_shape",
            "UCAN holder signature or nonce has the wrong width",
        ));
    }
    let signature = SignerSignature::try_from_slice(&signature_bytes)
        .map_err(|_| ObserveError::new("ucan_shape", "UCAN holder signature is malformed"))?;
    let nonce = HolderInvocationNonce::try_from_slice(&nonce_bytes)
        .map_err(|_| ObserveError::new("ucan_shape", "UCAN holder nonce is malformed"))?;
    let request_binding = ucan_request_binding(
        request.method,
        request.path,
        request.body,
        request.state_revision,
    )?;
    let profile_id = ucan_operation_profile_id();
    let operation = OperationBinding::new(profile_id, request_binding);
    let root_key = parse_principal(&auth.issuer)?;
    let issuer = IssuerDid::new(auth.issuer.clone())
        .map_err(|_| ObserveError::new("ucan_issuer", "UCAN issuer DID is invalid"))?;
    let public_key = Ed25519PublicKey::try_from_slice(&root_key)
        .map_err(|_| ObserveError::new("ucan_issuer", "UCAN issuer key is invalid"))?;
    let verification_key = Ed25519VerificationKey::new(issuer.clone(), public_key)
        .map_err(|_| ObserveError::new("ucan_issuer", "UCAN issuer key does not match its DID"))?;
    let mut keys = vec![verification_key];
    for holder in &auth.holder_keys {
        let holder_key = parse_principal(holder)?;
        let public_key = Ed25519PublicKey::try_from_slice(&holder_key)
            .map_err(|_| ObserveError::new("ucan_holder_key", "UCAN holder key is invalid"))?;
        let holder_issuer = public_key
            .issuer()
            .map_err(|_| ObserveError::new("ucan_holder_key", "UCAN holder key DID is invalid"))?;
        keys.push(
            Ed25519VerificationKey::new(holder_issuer, public_key).map_err(|_| {
                ObserveError::new("ucan_holder_key", "UCAN holder key does not match its DID")
            })?,
        );
    }
    let keys = KeyResolutionContext::new(keys);
    let proofs = auth
        .proof_tokens
        .iter()
        .map(|token| CompactToken::new(token))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ObserveError::new("ucan_proof", "UCAN proof token is invalid"))?;
    let proofs = ProofCollection::from_tokens(proofs);
    let revocations = auth
        .revoked_proof_references
        .iter()
        .map(|reference| {
            decode_hex(reference).map(ProofReference::new).map_err(|_| {
                ObserveError::new("ucan_revocation", "UCAN revocation reference is invalid")
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let revocations = RevocationList::from_references(revocations);
    let max_proof_depth = usize::try_from(auth.max_proof_depth).unwrap_or(usize::MAX);
    let max_proof_count = usize::try_from(auth.max_proof_count).unwrap_or(usize::MAX);
    let max_token_bytes = usize::try_from(auth.max_token_bytes).unwrap_or(usize::MAX);
    let verification_limits =
        VerificationLimits::new(max_proof_depth, max_proof_count, max_token_bytes)
            .with_clock_skew_tolerance_seconds(auth.max_clock_skew_seconds);
    let holder_limits = HolderInvocationLimits::try_new(
        usize::try_from(auth.max_holder_audience_bytes).unwrap_or(usize::MAX),
        usize::try_from(auth.max_holder_resource_bytes).unwrap_or(usize::MAX),
        usize::try_from(auth.max_holder_ability_bytes).unwrap_or(usize::MAX),
    )
    .map_err(|_| ObserveError::new("ucan_bounds", "UCAN holder limits are invalid"))?;
    let token_context = HolderTokenVerificationContext::new(
        VerificationTime::from_unix_seconds(now_unix),
        &keys,
        &proofs,
        &revocations,
        verification_limits,
    );
    let profiles = ObservationOperationProfile {
        expected: profile_id,
    };
    let replay = UcanReplay {
        state: replay,
        now_unix,
    };
    let policy_context =
        HolderInvocationPolicyContext::new(&DenyUnknownCaveats, &profiles, &replay, holder_limits);
    let holder_request = HolderInvocationRequest::new(
        compact.as_str(),
        &auth.resource,
        &auth.ability,
        operation,
        nonce,
        &signature,
    );
    verify_holder_signed_invocation(&holder_request, &token_context, &policy_context)
        .map(|_authorized| ())
        .map_err(|_| ObserveError::new("ucan_denied", "UCAN holder invocation was not authorized"))
}

#[must_use]
pub(crate) fn ucan_operation_profile_id() -> OperationProfileId {
    OperationProfileId::from_bytes(
        *blake3::hash(UCAN_OBSERVATION_OPERATION_PROFILE.as_bytes()).as_bytes(),
    )
}

pub(crate) fn ucan_request_binding(
    method: &str,
    target: &str,
    body: &[u8],
    state_revision: u64,
) -> ObserveResult<RequestBindingDigest> {
    let body_digest = body_digest(body);
    let binding_bytes = serde_json::to_vec(&RequestBinding {
        method,
        target,
        body_blake3: &body_digest,
        state_revision,
    })
    .map_err(|_| ObserveError::new("ucan_binding", "UCAN request binding could not be encoded"))?;
    Ok(RequestBindingDigest::from_bytes(
        *blake3::hash(&binding_bytes).as_bytes(),
    ))
}

fn decode_hex(value: &str) -> Result<Vec<u8>, ()> {
    if value.len() != 64 {
        return Err(());
    }
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(32);
    let mut index = 0;
    while index < 32 {
        let high = hex_value(*bytes.get(index * 2).ok_or(())?).ok_or(())?;
        let low = hex_value(*bytes.get(index * 2 + 1).ok_or(())?).ok_or(())?;
        output.push((high << 4) | low);
        index += 1;
    }
    Ok(output)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
