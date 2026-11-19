use ucan::{
    AudienceDid, CapabilitySet, CompactToken, ProofReferences, TokenSigner, TokenTimeBounds,
    issue_token_with_signer,
};

use crate::secretspec_issuer::{
    ISSUER_RESOLUTION_RESULT_SCHEMA, IssuerKeyDeclaration, IssuerResolutionReason,
    IssuerResolutionResult, REQUIRED_ISSUER_NON_CLAIMS, RotationWindow, validate_declaration,
    validate_rotation_window,
};

use super::model::IssuancePlan;

/// Stable UCAN issuance failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueFailure {
    /// Audience DID admission failed.
    Audience,
    /// Capability admission failed.
    Capability,
    /// Time bounds were invalid.
    Time,
    /// The signer or token encoder failed.
    Token,
    /// `SecretSpec` resolution, declaration, rotation, or signer binding failed.
    IssuerBinding,
}

/// Public `SecretSpec` and rotation facts required before issuance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuerAdmission {
    /// Reviewed `SecretSpec` declaration.
    pub declaration: IssuerKeyDeclaration,
    /// Bounded resolution result.
    pub resolution: IssuerResolutionResult,
    /// Public verification-key rotation window.
    pub rotation: RotationWindow,
}

/// Derive the public digest that binds a UCAN signer to a rotation window.
///
/// # Errors
///
/// Returns [`IssueFailure::IssuerBinding`] when the signer cannot provide its issuer identity.
pub fn signer_verification_digest(signer: &dyn TokenSigner) -> Result<String, IssueFailure> {
    let issuer = signer
        .issuer()
        .map_err(|_error| IssueFailure::IssuerBinding)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"basalt.oidc.ucan-signer.v1");
    let bytes = issuer.as_str().as_bytes();
    let length = u64::try_from(bytes.len()).map_err(|_error| IssueFailure::IssuerBinding)?;
    hasher.update(&length.to_le_bytes());
    hasher.update(bytes);
    Ok(format!("blake3:{}", hasher.finalize().to_hex()))
}

/// Issue one UCAN from an already verified, replay-admitted plan.
///
/// Private key custody remains in the caller-supplied signer. This function
/// cannot broaden the audience, resource, operation, or lifetime in the plan.
///
/// # Errors
///
/// Returns a stable failure when UCAN admission, signing, or encoding fails.
pub fn issue_plan(
    plan: &IssuancePlan,
    issuer: &IssuerAdmission,
    signer: &dyn TokenSigner,
) -> Result<CompactToken, IssueFailure> {
    validate_issuer_admission(issuer, signer)?;
    let audience =
        AudienceDid::new(plan.audience.clone()).map_err(|_error| IssueFailure::Audience)?;
    let capabilities = CapabilitySet::single(plan.resource.clone(), plan.operation.ability())
        .map_err(|_error| IssueFailure::Capability)?;
    let bounds = TokenTimeBounds::new(plan.not_before_seconds, plan.expires_at_seconds)
        .map_err(|_error| IssueFailure::Time)?;
    issue_token_with_signer(
        signer,
        &audience,
        &capabilities,
        &ProofReferences::empty(),
        bounds,
    )
    .map_err(|_error| IssueFailure::Token)
}

/// Validate the `SecretSpec` result, public rotation facts, and signer identity.
///
/// # Errors
///
/// Returns [`IssueFailure::IssuerBinding`] for any mismatch or weakened boundary.
pub fn validate_issuer_admission(
    issuer: &IssuerAdmission,
    signer: &dyn TokenSigner,
) -> Result<(), IssueFailure> {
    if !validate_declaration(&issuer.declaration).is_empty()
        || issuer.resolution.schema != ISSUER_RESOLUTION_RESULT_SCHEMA
        || issuer.resolution.reason != IssuerResolutionReason::Loaded
        || !issuer.resolution.allowed
        || !issuer.resolution.issues.is_empty()
        || issuer.resolution.non_claims.as_slice() != REQUIRED_ISSUER_NON_CLAIMS
        || !validate_rotation_window(&issuer.rotation).is_empty()
        || issuer.rotation.signing_key_id != issuer.declaration.key_id
        || issuer.rotation.active_verification_digest != signer_verification_digest(signer)?
    {
        return Err(IssueFailure::IssuerBinding);
    }
    Ok(())
}
