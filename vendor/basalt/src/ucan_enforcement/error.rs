use super::receipt::{ReceiptContext, UcanEnforcementReceipt, build_receipt};
use super::types::UcanEnforcementReason;
use std::fmt;
use ucan::{
    AuthorizationDenialReason, HolderInvocationError, HolderInvocationReplayError,
    KeyResolverError, OperationProfileAdmissionError, ProofStoreError, TokenError,
    VerificationLimitError,
};

/// Typed fail-closed adapter error with redacted evidence only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UcanEnforcementFailure {
    reason: UcanEnforcementReason,
    receipt: Box<UcanEnforcementReceipt>,
}

impl UcanEnforcementFailure {
    /// Return the stable denial reason.
    #[must_use]
    pub const fn reason(&self) -> UcanEnforcementReason {
        self.reason
    }

    /// Return the redacted denial receipt.
    #[must_use]
    pub const fn receipt(&self) -> &UcanEnforcementReceipt {
        &self.receipt
    }
}

impl fmt::Display for UcanEnforcementFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "UCAN enforcement denied: {}",
            self.reason.as_str()
        )
    }
}

impl std::error::Error for UcanEnforcementFailure {}

pub(super) fn failure(
    reason: UcanEnforcementReason,
    context: ReceiptContext,
) -> UcanEnforcementFailure {
    UcanEnforcementFailure {
        reason,
        receipt: Box::new(build_receipt(reason, false, context)),
    }
}

pub(super) const fn token_error_reason(error: &TokenError) -> UcanEnforcementReason {
    match error {
        TokenError::LimitExceeded {
            source: VerificationLimitError::CompactTokenTooLarge { .. },
        } => UcanEnforcementReason::OversizedCredential,
        TokenError::LimitExceeded { .. } => UcanEnforcementReason::LimitExceeded,
        TokenError::Signature { .. } => UcanEnforcementReason::SignatureDenied,
        TokenError::MissingVerificationKey { .. } => UcanEnforcementReason::KeyResolutionDenied,
        TokenError::KeyResolver { source } => key_resolver_error_reason(source),
        TokenError::ProofStore { source } => proof_store_error_reason(source),
        TokenError::ProofChain { .. }
        | TokenError::Attenuation { .. }
        | TokenError::SelfDelegation { .. } => UcanEnforcementReason::ProofDenied,
        TokenError::Revoked { .. } => UcanEnforcementReason::Revoked,
        TokenError::Revocation { .. }
        | TokenError::BackendTranscript { .. }
        | TokenError::VerifiedPrimitive { .. }
        | TokenError::JsonEncode { .. }
        | TokenError::InvalidSecretKeyLength { .. }
        | TokenError::Signer { .. } => UcanEnforcementReason::BackendUnavailable,
        TokenError::Temporal { .. } => UcanEnforcementReason::TimeDenied,
        TokenError::EmptyCompactToken
        | TokenError::CompactTokenMustBeTrimmed
        | TokenError::InvalidCompactTokenStructure { .. }
        | TokenError::CompactTokenFramingMismatch
        | TokenError::Base64Decode { .. }
        | TokenError::JsonDecode { .. }
        | TokenError::UnsupportedHeader
        | TokenError::Capability { .. }
        | TokenError::MissingCapabilityField { .. }
        | TokenError::CoreClaims { .. }
        | TokenError::InvalidPublicKeyLength { .. }
        | TokenError::DidEncodingUtf8
        | TokenError::SecretCapacity { .. }
        | TokenError::JsonEscapesNotSupported { .. } => UcanEnforcementReason::MalformedCredential,
    }
}

pub(super) const fn authorization_error_reason(
    reason: &AuthorizationDenialReason,
) -> UcanEnforcementReason {
    match reason {
        AuthorizationDenialReason::CaveatRejected { .. }
        | AuthorizationDenialReason::CaveatPolicyMissing { .. } => {
            UcanEnforcementReason::CaveatDenied
        }
        AuthorizationDenialReason::MalformedRequest { .. }
        | AuthorizationDenialReason::NoMatchingCapability => UcanEnforcementReason::ScopeDenied,
    }
}

pub(super) const fn profile_error_reason(
    error: OperationProfileAdmissionError,
) -> UcanEnforcementReason {
    match error {
        OperationProfileAdmissionError::UnknownProfile => UcanEnforcementReason::OperationMismatch,
        OperationProfileAdmissionError::Backend => UcanEnforcementReason::BackendUnavailable,
    }
}

pub(super) const fn holder_error_reason(error: &HolderInvocationError) -> UcanEnforcementReason {
    match error {
        HolderInvocationError::Token { source } => token_error_reason(source),
        HolderInvocationError::Authorization { reason } => authorization_error_reason(reason),
        HolderInvocationError::Input { .. }
        | HolderInvocationError::Canonical {
            source:
                ucan::core::holder_invocation::HolderInvocationCanonicalError::EmptyField { .. }
                | ucan::core::holder_invocation::HolderInvocationCanonicalError::OutputLengthMismatch {
                    ..
                },
        } => UcanEnforcementReason::MalformedCredential,
        HolderInvocationError::Canonical { .. } | HolderInvocationError::Limit { .. } => {
            UcanEnforcementReason::LimitExceeded
        }
        HolderInvocationError::KeyResolution { source } => key_resolver_error_reason(source),
        HolderInvocationError::ResolvedAudienceKeyMismatch
        | HolderInvocationError::Signer { .. }
        | HolderInvocationError::SignerAudienceMismatch
        | HolderInvocationError::SignaturePrimitive { .. }
        | HolderInvocationError::InvalidHolderSignature => UcanEnforcementReason::HolderDenied,
        HolderInvocationError::OperationProfile { source } => profile_error_reason(*source),
        HolderInvocationError::Replay { source } => replay_error_reason(*source),
    }
}

const fn proof_store_error_reason(error: &ProofStoreError) -> UcanEnforcementReason {
    match error {
        ProofStoreError::MissingProof { .. }
        | ProofStoreError::Malformed { .. }
        | ProofStoreError::ReferenceMismatch { .. } => UcanEnforcementReason::ProofDenied,
        ProofStoreError::Stale { .. } | ProofStoreError::Backend { .. } => {
            UcanEnforcementReason::BackendUnavailable
        }
    }
}

const fn key_resolver_error_reason(error: &KeyResolverError) -> UcanEnforcementReason {
    match error {
        KeyResolverError::MissingKey { .. }
        | KeyResolverError::UnsupportedMethod { .. }
        | KeyResolverError::Malformed { .. } => UcanEnforcementReason::KeyResolutionDenied,
        KeyResolverError::Stale { .. } | KeyResolverError::Backend { .. } => {
            UcanEnforcementReason::BackendUnavailable
        }
    }
}

const fn replay_error_reason(_error: HolderInvocationReplayError) -> UcanEnforcementReason {
    UcanEnforcementReason::ReplayDenied
}
