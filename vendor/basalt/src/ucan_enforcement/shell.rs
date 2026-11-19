use super::core::{
    ExactOperationProfile, bind_enforcement_action, build_authority_decision,
    decode_blake3_reference, operation_profile_digest,
};
use super::error::{
    UcanEnforcementFailure, authorization_error_reason, failure, holder_error_reason,
    profile_error_reason, token_error_reason,
};
use super::receipt::{ReceiptContext, UcanEnforcementReceipt, build_receipt};
use super::types::{UcanEnforcementBoundaries, UcanEnforcementReason, UcanEnforcementRequest};
use crate::EnforcementRequest;
use crate::Policy;
use crate::authority_core::{AuthorityDecision, BasaltPolicyRef};
use crate::ucan_profile::{VerifiedUcanAuthorityInput, admit_compact_profile};
use std::fmt;
use ucan::{
    AuthorizationDecision, HolderInvocationAuthorized, HolderInvocationPolicyContext,
    HolderInvocationRequest, HolderTokenVerificationContext, OperationBinding,
    OperationProfileAdmission, verify_compact_token_str_with_resolvers_and_revocations,
    verify_holder_signed_invocation,
};

/// Live exact-request authority after final UCAN replay admission.
///
/// This type has private construction and no clone or serde surface.
pub struct LiveUcanEnforcement<'request> {
    request: &'request EnforcementRequest,
    _authority_decision: AuthorityDecision,
    receipt: UcanEnforcementReceipt,
    holder_authorization: HolderInvocationAuthorized,
}

impl fmt::Debug for LiveUcanEnforcement<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LiveUcanEnforcement")
            .field("allowed", &true)
            .field("receipt_blake3", &self.receipt.receipt_blake3)
            .finish_non_exhaustive()
    }
}

impl<'request> LiveUcanEnforcement<'request> {
    /// Return the exact borrowed request admitted by this live capability.
    #[must_use]
    pub const fn request(&self) -> &'request EnforcementRequest {
        self.request
    }

    /// Return the redacted evidence receipt.
    #[must_use]
    pub const fn receipt(&self) -> &UcanEnforcementReceipt {
        &self.receipt
    }

    /// Observe the retained holder using UCAN's non-consuming recheck.
    ///
    /// This does not refresh Basalt policy, establish continuous authority, or
    /// authorize release. Consumers retain current policy and atomic effect checks.
    /// No replay port is accepted and no new live enforcement value is created.
    ///
    /// # Errors
    /// Returns UCAN binding, token, proof, revocation, profile, or caveat failures.
    ///
    /// ```compile_fail
    /// fn dispatch(_: basalt::ucan_enforcement::LiveUcanEnforcement<'_>) {}
    /// fn reuse(observation: basalt::ucan::HolderAuthorizationObservation<'_>) {
    ///     dispatch(observation);
    /// }
    /// ```
    pub fn observe_holder(
        &self,
        compact_token: &str,
        verification: &ucan::HolderTokenVerificationContext<'_>,
        policies: &ucan::HolderRecheckPolicies<'_>,
    ) -> Result<ucan::HolderAuthorizationObservation<'_>, ucan::HolderRecheckError> {
        ucan::recheck_holder_authorization(
            &self.holder_authorization,
            compact_token,
            verification,
            policies,
        )
    }

    /// Return the final live UCAN holder authorization.
    #[must_use]
    pub const fn holder_authorization(&self) -> &HolderInvocationAuthorized {
        &self.holder_authorization
    }

    pub(crate) fn into_parts(self) -> (HolderInvocationAuthorized, UcanEnforcementReceipt) {
        (self.holder_authorization, self.receipt)
    }
}

struct PreparedRequest {
    operation: OperationBinding,
    profile: ExactOperationProfile,
    receipt_context: ReceiptContext,
}

struct ProspectiveAuthority {
    decision: AuthorityDecision,
    receipt: UcanEnforcementReceipt,
    operation: OperationBinding,
    receipt_context: ReceiptContext,
}

/// Verify one raw holder invocation and return live exact-request Basalt authority.
///
/// All fallible policy work finishes before the final UCAN replay call.
///
/// # Errors
///
/// Returns [`UcanEnforcementFailure`] with no authority when a boundary denies.
pub fn verify_ucan_enforcement_request<'request>(
    policy: &Policy,
    trusted_policy_ref: &BasaltPolicyRef,
    request: &UcanEnforcementRequest<'request, '_>,
    boundaries: &UcanEnforcementBoundaries<'_>,
) -> Result<LiveUcanEnforcement<'request>, UcanEnforcementFailure> {
    let prepared = prepare_bounded_request(trusted_policy_ref, request, boundaries)?;
    let prospective =
        prepare_prospective_authority(policy, trusted_policy_ref, request, boundaries, &prepared)?;
    let holder_request = HolderInvocationRequest::new(
        request.compact_token,
        request.enforcement.resource(),
        request.enforcement.ability(),
        prospective.operation,
        request.nonce,
        request.holder_signature,
    );
    let token_context = HolderTokenVerificationContext::new(
        boundaries.verification_time,
        boundaries.key_resolver,
        boundaries.proof_store,
        boundaries.revocations,
        boundaries.verification_limits,
    );
    let policy_context = HolderInvocationPolicyContext::new(
        boundaries.caveat_policies,
        &prepared.profile,
        boundaries.replay,
        boundaries.holder_limits,
    );

    match verify_holder_signed_invocation(&holder_request, &token_context, &policy_context) {
        Ok(holder_authorization) => Ok(LiveUcanEnforcement {
            request: request.enforcement,
            _authority_decision: prospective.decision,
            receipt: prospective.receipt,
            holder_authorization,
        }),
        Err(error) => Err(failure(
            holder_error_reason(&error),
            prospective.receipt_context,
        )),
    }
}

fn prepare_bounded_request(
    trusted_policy_ref: &BasaltPolicyRef,
    request: &UcanEnforcementRequest<'_, '_>,
    boundaries: &UcanEnforcementBoundaries<'_>,
) -> Result<PreparedRequest, UcanEnforcementFailure> {
    let operation_profile = operation_profile_digest(request.action);
    let policy = decode_blake3_reference(trusted_policy_ref.artifact_ref.as_str());
    let credential_bytes = u64::try_from(request.compact_token.len()).ok();
    let initial_context = ReceiptContext {
        protocol_profile: request.profile,
        credential_bytes,
        credential: None,
        request_binding: None,
        operation_profile,
        policy,
    };
    if admit_compact_profile(request.profile).is_err() {
        return Err(failure(
            UcanEnforcementReason::ProtocolProfileDenied,
            initial_context,
        ));
    }
    if credential_bytes.is_none() {
        return Err(failure(
            UcanEnforcementReason::LimitExceeded,
            initial_context,
        ));
    }
    if request.compact_token.len() > boundaries.verification_limits.max_compact_token_bytes() {
        return Err(failure(
            UcanEnforcementReason::OversizedCredential,
            initial_context,
        ));
    }
    let credential = *blake3::hash(request.compact_token.as_bytes()).as_bytes();
    let bounded_context = ReceiptContext {
        credential: Some(credential),
        ..initial_context
    };
    let operation = bind_enforcement_action(request.action, request.enforcement)
        .map_err(|reason| failure(reason, bounded_context))?;
    let request_binding = *operation.request_binding().as_bytes();
    Ok(PreparedRequest {
        operation,
        profile: ExactOperationProfile {
            expected: operation.profile(),
        },
        receipt_context: ReceiptContext {
            request_binding: Some(request_binding),
            ..bounded_context
        },
    })
}

fn prepare_prospective_authority(
    policy: &Policy,
    trusted_policy_ref: &BasaltPolicyRef,
    request: &UcanEnforcementRequest<'_, '_>,
    boundaries: &UcanEnforcementBoundaries<'_>,
    prepared: &PreparedRequest,
) -> Result<ProspectiveAuthority, UcanEnforcementFailure> {
    let operation = prepared.operation;
    let receipt_context = prepared.receipt_context;
    let verified = verify_compact_token_str_with_resolvers_and_revocations(
        request.compact_token,
        boundaries.verification_time,
        boundaries.key_resolver,
        boundaries.proof_store,
        boundaries.revocations,
        boundaries.verification_limits,
    )
    .map_err(|error| failure(token_error_reason(&error), receipt_context))?;
    prepared
        .profile
        .admit_operation_profile(&operation.profile())
        .map_err(|error| failure(profile_error_reason(error), receipt_context))?;
    let authorization = verified.authorize_with_policies(
        request.enforcement.resource(),
        request.enforcement.ability(),
        boundaries.caveat_policies,
    );
    let AuthorizationDecision::Allowed { .. } = authorization else {
        let reason = authorization.denial_reason().map_or(
            UcanEnforcementReason::ScopeDenied,
            authorization_error_reason,
        );
        return Err(failure(reason, receipt_context));
    };
    let Some(credential) = receipt_context.credential else {
        return Err(failure(
            UcanEnforcementReason::InternalAuthorityDenied,
            receipt_context,
        ));
    };
    let Some(request_binding) = receipt_context.request_binding else {
        return Err(failure(
            UcanEnforcementReason::InternalAuthorityDenied,
            receipt_context,
        ));
    };
    let verified_input = VerifiedUcanAuthorityInput::from_compact_verified(
        request.profile,
        verified.audience().as_str(),
        receipt_context.operation_profile,
        request_binding,
        credential,
        !verified.proofs().as_slice().is_empty(),
    )
    .map_err(|_| {
        failure(
            UcanEnforcementReason::ProtocolProfileDenied,
            receipt_context,
        )
    })?;
    let decision = build_authority_decision(
        policy,
        trusted_policy_ref,
        request.enforcement,
        &verified_input,
        boundaries.verification_time,
    )
    .map_err(|reason| failure(reason, receipt_context))?;
    if !decision.is_allowed() {
        return Err(failure(
            UcanEnforcementReason::PolicyDenied,
            receipt_context,
        ));
    }
    let receipt = build_receipt(UcanEnforcementReason::Allowed, true, receipt_context);
    Ok(ProspectiveAuthority {
        decision,
        receipt,
        operation,
        receipt_context,
    })
}
