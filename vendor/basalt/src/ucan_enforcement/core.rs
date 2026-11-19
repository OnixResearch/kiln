use super::receipt::BLAKE3_DIGEST_BYTES;
use super::types::{EnforcementAction, UcanEnforcementReason};
use crate::authority_core::{
    AuthorityAdapterError, AuthorityCaveatDisposition, AuthorityClaim, AuthorityDecision,
    AuthorityEvidence, AuthorityReplayDisposition, BasaltPolicyRef, REQUIRED_AUTHORITY_NON_CLAIMS,
    UcanVerificationReceiptRef, VerificationRefDisposition, VerifiedGrantBinding,
    VerifiedGrantKind, adapt_enforcement_request, evaluate_authority, is_blake3_reference,
};
use crate::ucan_profile::VerifiedUcanAuthorityInput;
use crate::{CapabilityGrant, EnforcementRequest, Policy};
use ucan::{
    OperationBinding, OperationProfileAdmission, OperationProfileAdmissionError,
    OperationProfileId, RequestBindingDigest, VerificationTime,
};

pub(super) const OPERATION_PROFILE_DOMAIN: &[u8] = b"basalt/operation-profile/v1";
const REQUEST_BINDING_DOMAIN: &[u8] = b"basalt/ucan-enforcement-request/v1";
const VERIFIED_GRANT_REFERENCE_DOMAIN: &[u8] = b"basalt/ucan-enforcement-grant/v1";
const VERIFICATION_RECEIPT_REFERENCE_DOMAIN: &[u8] = b"basalt/ucan-enforcement-verification/v1";
const BLAKE3_REFERENCE_PREFIX: &str = "blake3:";
const HEX_CHARACTERS_PER_BYTE: usize = 2;
const HEX_RADIX: u8 = 16;
const HEX_ALPHA_OFFSET: u8 = 10;
const BLAKE3_HEX_LENGTH: usize = BLAKE3_DIGEST_BYTES * HEX_CHARACTERS_PER_BYTE;
const MAXIMUM_REQUEST_FIELD_BYTES: usize = 4_096;

#[derive(Clone, Copy)]
pub struct ExactOperationProfile {
    pub(crate) expected: OperationProfileId,
}

impl OperationProfileAdmission for ExactOperationProfile {
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

/// Bind one typed action to an exact Basalt request for holder signing.
///
/// # Errors
///
/// Returns a typed denial for supplied capability fields, mapping mismatch, or bad bounds.
pub fn bind_enforcement_action(
    action: EnforcementAction,
    request: &EnforcementRequest,
) -> Result<OperationBinding, UcanEnforcementReason> {
    if !request.capabilities().is_empty() {
        return Err(UcanEnforcementReason::UnverifiedAuthorityFields);
    }
    if request.contract() != action.contract_id() {
        return Err(UcanEnforcementReason::OperationMismatch);
    }
    if request.ability() != action.ability() {
        return Err(UcanEnforcementReason::OperationMismatch);
    }
    if !request.resource().starts_with(action.resource_prefix()) {
        return Err(UcanEnforcementReason::ScopeDenied);
    }
    for field in [request.contract(), request.resource(), request.ability()] {
        if !bounded_request_field(field) {
            return Err(UcanEnforcementReason::LimitExceeded);
        }
    }
    if action == EnforcementAction::ProcessExecute && !exact_process_resource(request.resource()) {
        return Err(UcanEnforcementReason::ScopeDenied);
    }
    let request_binding =
        derive_request_binding(action, request).ok_or(UcanEnforcementReason::LimitExceeded)?;
    Ok(OperationBinding::new(
        action.operation_profile_id(),
        RequestBindingDigest::from_bytes(request_binding),
    ))
}

// r[impl basalt.ucan_token_flow.process_execution]
fn exact_process_resource(resource: &str) -> bool {
    let Some(digest) = resource.strip_prefix(EnforcementAction::ProcessExecute.resource_prefix())
    else {
        return false;
    };
    digest.len() == BLAKE3_HEX_LENGTH
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(super) fn build_authority_decision(
    policy: &Policy,
    trusted_policy_ref: &BasaltPolicyRef,
    request: &EnforcementRequest,
    verified: &VerifiedUcanAuthorityInput,
    verification_time: VerificationTime,
) -> Result<AuthorityDecision, UcanEnforcementReason> {
    build_authority_decision_with_time(
        policy,
        trusted_policy_ref,
        request,
        verified,
        &verification_time.as_unix_seconds().to_be_bytes(),
    )
}

pub fn build_final_authority_decision(
    policy: &Policy,
    trusted_policy_ref: &BasaltPolicyRef,
    request: &EnforcementRequest,
    verified: &VerifiedUcanAuthorityInput,
    now_seconds: i64,
) -> Result<AuthorityDecision, UcanEnforcementReason> {
    build_authority_decision_with_time(
        policy,
        trusted_policy_ref,
        request,
        verified,
        &now_seconds.to_be_bytes(),
    )
}

fn build_authority_decision_with_time(
    policy: &Policy,
    trusted_policy_ref: &BasaltPolicyRef,
    request: &EnforcementRequest,
    verified: &VerifiedUcanAuthorityInput,
    verification_time_bytes: &[u8],
) -> Result<AuthorityDecision, UcanEnforcementReason> {
    let kind = if verified.is_delegated() {
        VerifiedGrantKind::Delegated
    } else {
        VerifiedGrantKind::Direct
    };
    let grant_digest = derive_reference(
        VERIFIED_GRANT_REFERENCE_DOMAIN,
        &[
            verified.profile().as_str().as_bytes(),
            verified.source_revision().as_bytes(),
            verified.support_catalog_blake3().as_bytes(),
            verified.subject().as_bytes(),
            verified.operation_profile().as_slice(),
            verified.token_identity().as_slice(),
            verified.request_binding().as_slice(),
        ],
    )
    .ok_or(UcanEnforcementReason::LimitExceeded)?;
    let receipt_digest = derive_reference(
        VERIFICATION_RECEIPT_REFERENCE_DOMAIN,
        &[
            grant_digest.as_slice(),
            verified.request_binding().as_slice(),
            verification_time_bytes,
        ],
    )
    .ok_or(UcanEnforcementReason::LimitExceeded)?;
    let request_ref = format_digest(*verified.request_binding());
    let grant_ref = format_digest(grant_digest);
    let verification_receipt_ref = format_digest(receipt_digest);
    let internal_request = EnforcementRequest::new(
        request.contract().to_owned(),
        request.resource().to_owned(),
        request.ability().to_owned(),
    )
    .with_capability(CapabilityGrant::new(
        request.resource().to_owned(),
        request.ability().to_owned(),
    ));
    let evidence = AuthorityEvidence {
        request_ref: request_ref.clone(),
        grant_bindings: vec![VerifiedGrantBinding {
            grant_ref: grant_ref.clone(),
            verification_receipt_ref: verification_receipt_ref.clone(),
            kind,
        }],
        verification_receipts: vec![UcanVerificationReceiptRef {
            receipt_ref: verification_receipt_ref,
            request_ref,
            verified_grant_refs: vec![grant_ref],
            disposition: VerificationRefDisposition::Current,
        }],
        policy_refs: vec![trusted_policy_ref.clone()],
        caveat: AuthorityCaveatDisposition::Satisfied,
        replay: AuthorityReplayDisposition::Admitted,
        claims: vec![AuthorityClaim::ExactRequest],
        non_claims: REQUIRED_AUTHORITY_NON_CLAIMS.map(str::to_owned).to_vec(),
    };
    let input = adapt_enforcement_request(&internal_request, evidence)
        .map_err(authority_adapter_error_reason)?;
    Ok(evaluate_authority(policy, trusted_policy_ref, &input))
}

const fn authority_adapter_error_reason(_error: AuthorityAdapterError) -> UcanEnforcementReason {
    UcanEnforcementReason::InternalAuthorityDenied
}

pub fn operation_profile_digest(action: EnforcementAction) -> [u8; BLAKE3_DIGEST_BYTES] {
    derive_reference(
        OPERATION_PROFILE_DOMAIN,
        &[
            action.as_str().as_bytes(),
            action.contract_id().as_bytes(),
            action.ability().as_bytes(),
        ],
    )
    .expect("closed operation-profile fields must fit fixed u64 length prefixes")
}

pub(super) fn derive_request_binding(
    action: EnforcementAction,
    request: &EnforcementRequest,
) -> Option<[u8; BLAKE3_DIGEST_BYTES]> {
    derive_reference(
        REQUEST_BINDING_DOMAIN,
        &[
            action.as_str().as_bytes(),
            request.contract().as_bytes(),
            request.resource().as_bytes(),
            request.ability().as_bytes(),
        ],
    )
}

fn derive_reference(domain: &[u8], fields: &[&[u8]]) -> Option<[u8; BLAKE3_DIGEST_BYTES]> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    for field in fields {
        let length = u64::try_from(field.len()).ok()?;
        hasher.update(&length.to_be_bytes());
        hasher.update(field);
    }
    Some(*hasher.finalize().as_bytes())
}

fn bounded_request_field(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAXIMUM_REQUEST_FIELD_BYTES
        && !value.chars().any(char::is_control)
}

pub fn format_digest(digest: [u8; BLAKE3_DIGEST_BYTES]) -> String {
    format!(
        "{BLAKE3_REFERENCE_PREFIX}{}",
        blake3::Hash::from_bytes(digest).to_hex()
    )
}

pub fn decode_blake3_reference(reference: &str) -> Option<[u8; BLAKE3_DIGEST_BYTES]> {
    if !is_blake3_reference(reference) {
        return None;
    }
    let hex = reference.strip_prefix(BLAKE3_REFERENCE_PREFIX)?;
    if hex.len() != BLAKE3_HEX_LENGTH {
        return None;
    }
    let bytes = hex.as_bytes();
    let mut digest = [0_u8; BLAKE3_DIGEST_BYTES];
    for (index, output) in digest.iter_mut().enumerate() {
        let high_index = index.checked_mul(HEX_CHARACTERS_PER_BYTE)?;
        let low_index = high_index.checked_add(1)?;
        *output = hex_nibble(bytes[high_index])?
            .checked_mul(HEX_RADIX)?
            .checked_add(hex_nibble(bytes[low_index])?)?;
    }
    Some(digest)
}

const fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + HEX_ALPHA_OFFSET),
        _ => None,
    }
}

#[cfg(test)]
pub(super) const fn blake3_reference_prefix() -> &'static str {
    BLAKE3_REFERENCE_PREFIX
}
