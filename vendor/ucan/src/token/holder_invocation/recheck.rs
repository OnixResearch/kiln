//! Re-observe an admitted invocation without another replay admission.

/// Non-consuming policies for the original operation. No replay port is accepted.
pub struct HolderRecheckPolicies<'a> {
    pub caveats: &'a dyn super::CaveatPolicySet,
    pub profiles: &'a dyn super::OperationProfileAdmission,
}

#[derive(Debug)]
pub enum HolderRecheckError {
    Binding(ucan_core::holder_invocation::recheck::Error),
    Verification(super::HolderInvocationError),
}

impl core::fmt::Display for HolderRecheckError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Binding(error) => error.fmt(formatter),
            Self::Verification(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for HolderRecheckError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Binding(error) => Some(error),
            Self::Verification(error) => Some(error),
        }
    }
}

/// Borrowed observation of the original holder authorization at supplied time.
///
/// This is not a new holder invocation, dispatch capability, continuous-authority
/// proof, or resource-release receipt. The consumer owns authenticated backend
/// freshness and atomic effect checks. Backend state can change after this call.
/// The value has no public constructor, Clone, Debug, serde, or owned conversion.
///
/// ```compile_fail
/// fn forge(holder: &ucan::HolderInvocationAuthorized) {
///     let _ = ucan::HolderAuthorizationObservation {
///         authorization: holder,
///         observed_at: ucan::VerificationTime::from_unix_seconds(1),
///     };
/// }
/// ```
///
/// ```compile_fail
/// fn duplicate(observed: ucan::HolderAuthorizationObservation<'_>) {
///     let _ = observed.clone();
/// }
/// ```
///
/// ```compile_fail
/// fn dispatch(_: ucan::HolderInvocationAuthorized) {}
/// fn reuse(observed: ucan::HolderAuthorizationObservation<'_>) {
///     dispatch(observed);
/// }
/// ```
///
/// ```compile_fail
/// fn requires_serialize<T: serde::Serialize>() {}
/// requires_serialize::<ucan::HolderAuthorizationObservation<'static>>();
/// ```
///
/// ```compile_fail
/// fn requires_deserialize<T: for<'de> serde::Deserialize<'de>>() {}
/// requires_deserialize::<ucan::HolderAuthorizationObservation<'static>>();
/// ```
///
/// ```compile_fail
/// fn escape<'a>(observed: ucan::HolderAuthorizationObservation<'a>)
///     -> ucan::HolderAuthorizationObservation<'static> {
///     observed
/// }
/// ```
pub struct HolderAuthorizationObservation<'live> {
    authorization: &'live super::HolderInvocationAuthorized,
    observed_at: super::VerificationTime,
}

impl<'live> HolderAuthorizationObservation<'live> {
    /// Original live admission, not a reconstructed or newly admitted invocation.
    #[must_use]
    pub const fn authorization(&self) -> &'live super::HolderInvocationAuthorized {
        self.authorization
    }

    /// Explicit time supplied for this successful recheck, not an ambient clock.
    #[must_use]
    pub const fn observed_at(&self) -> super::VerificationTime {
        self.observed_at
    }
}

/// Recheck the same admitted token and original operation without spending replay.
///
/// Uses the existing token/proof/signature/revocation verifier and non-consuming
/// profile/caveat policies. No new holder signature or invocation is admitted.
/// Failures preserve the original value; the consumer must stop or quarantine work.
/// UCAN's inclusive temporal rules stay unchanged. Consumers retain stricter
/// deadlines and monotonic time across successive observations.
///
/// # Errors
/// Rejects oversized or substituted tokens, time before initial verification,
/// token/proof/revocation failures, unknown profiles, and rejected caveats.
// r[impl ucan.holder_recheck.observation]
pub fn recheck_holder_authorization<'live>(
    authorization: &'live super::HolderInvocationAuthorized,
    compact_token: &str,
    token_context: &super::HolderTokenVerificationContext<'_>,
    policies: &HolderRecheckPolicies<'_>,
) -> Result<HolderAuthorizationObservation<'live>, HolderRecheckError> {
    super::enforce_signing_token_bound(compact_token, token_context.limits)
        .map_err(HolderRecheckError::Verification)?;
    let candidate = super::token_reference(compact_token);
    ucan_core::holder_invocation::recheck::admit(ucan_core::holder_invocation::recheck::Input {
        authorized_token: *authorization.token_reference().as_bytes(),
        candidate_token: *candidate.as_bytes(),
        authorized_at_seconds: authorization.verified_at.as_unix_seconds(),
        observed_at_seconds: token_context.time.as_unix_seconds(),
    })
    .map_err(HolderRecheckError::Binding)?;
    let verified = super::verify_compact_token_str_with_resolvers_and_revocations(
        compact_token,
        token_context.time,
        token_context.key_resolver,
        token_context.proof_store,
        token_context.revocations,
        token_context.limits,
    )
    .map_err(|source| HolderRecheckError::Verification(super::HolderInvocationError::Token { source }))?;
    policies.profiles.admit_operation_profile(&authorization.operation().profile()).map_err(|source| {
        HolderRecheckError::Verification(super::HolderInvocationError::OperationProfile { source })
    })?;
    if let super::AuthorizationDecision::Denied { reason } =
        verified.authorize_with_policies(authorization.resource(), authorization.ability(), policies.caveats)
    {
        return Err(HolderRecheckError::Verification(super::HolderInvocationError::Authorization { reason }));
    }
    Ok(HolderAuthorizationObservation {
        authorization,
        observed_at: token_context.time,
    })
}
