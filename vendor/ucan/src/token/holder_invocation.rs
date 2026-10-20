use std::fmt;

use ucan_core::holder_invocation::HOLDER_INVOCATION_DIGEST_BYTES;
use ucan_core::holder_invocation::HOLDER_INVOCATION_NONCE_BYTES;
use ucan_core::holder_invocation::HolderInvocationCanonicalError;
use ucan_core::holder_invocation::HolderInvocationCanonicalFields;
use ucan_core::holder_invocation::HolderInvocationField;
use ucan_core::holder_invocation::HolderInvocationFieldLimits;
use ucan_core::holder_invocation::holder_invocation_canonical_len;
use ucan_core::holder_invocation::write_holder_invocation_canonical;

use super::AudienceDid;
use super::AuthorizationDecision;
use super::AuthorizationDenialReason;
use super::CapabilityDocument;
use super::CaveatPolicySet;
use super::IssuerDid;
use super::KeyResolver;
use super::KeyResolverError;
use super::ProofStore;
use super::RevocationChecker;
use super::SignerError;
use super::SignerSignature;
use super::TokenError;
use super::TokenSigner;
use super::TokenTimeBounds;
use super::VerificationLimits;
use super::VerificationTime;
use super::validate_signer_identity;
use super::validate_signer_signature;
use super::verify_compact_token_str_with_resolvers_and_revocations;
use crate::verified::VerifiedPrimitiveError;
use crate::verified::verify_ed25519_signature;

const DEFAULT_MAX_AUDIENCE_BYTES: usize = 512;
const DEFAULT_MAX_RESOURCE_BYTES: usize = 4 * 1024;
const DEFAULT_MAX_ABILITY_BYTES: usize = 256;
const HARD_MAX_AUDIENCE_BYTES: usize = 1024;
const HARD_MAX_RESOURCE_BYTES: usize = 16 * 1024;
const HARD_MAX_ABILITY_BYTES: usize = 1024;
const HARD_MAX_CANONICAL_BYTES: usize = 16 * 1024;

pub type HolderInvocationResult<T> = std::result::Result<T, HolderInvocationError>;
pub type HolderInvocationInputResult<T> = std::result::Result<T, HolderInvocationInputError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HolderInvocationFixedField {
    OperationProfile,
    RequestBinding,
    Nonce,
}

impl HolderInvocationFixedField {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OperationProfile => "operation profile",
            Self::RequestBinding => "request binding",
            Self::Nonce => "nonce",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HolderInvocationInputError {
    InvalidFixedWidth {
        field: HolderInvocationFixedField,
        actual: usize,
        expected: usize,
    },
}

impl fmt::Display for HolderInvocationInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFixedWidth {
                field,
                actual,
                expected,
            } => write!(formatter, "{} byte length {actual} does not equal required length {expected}", field.as_str()),
        }
    }
}

impl std::error::Error for HolderInvocationInputError {}

/// A downstream-owned stable BLAKE3 profile identity.
///
/// UCAN compares and signs this fixed identity. It does not interpret the
/// downstream action vocabulary or profile document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OperationProfileId([u8; HOLDER_INVOCATION_DIGEST_BYTES]);

impl OperationProfileId {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; HOLDER_INVOCATION_DIGEST_BYTES]) -> Self {
        Self(bytes)
    }

    /// Copy an exact-width operation-profile identity.
    ///
    /// # Errors
    ///
    /// Returns [`HolderInvocationInputError`] when the input is not one BLAKE3 digest wide.
    pub fn try_from_slice(bytes: &[u8]) -> HolderInvocationInputResult<Self> {
        copy_fixed_digest(HolderInvocationFixedField::OperationProfile, bytes).map(Self::from_bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; HOLDER_INVOCATION_DIGEST_BYTES] {
        &self.0
    }
}

/// The downstream canonical request's exact BLAKE3 digest.
///
/// The consumer owns canonical request construction and digest provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RequestBindingDigest([u8; HOLDER_INVOCATION_DIGEST_BYTES]);

impl RequestBindingDigest {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; HOLDER_INVOCATION_DIGEST_BYTES]) -> Self {
        Self(bytes)
    }

    /// Copy an exact-width downstream request-binding digest.
    ///
    /// # Errors
    ///
    /// Returns [`HolderInvocationInputError`] when the input is not one BLAKE3 digest wide.
    pub fn try_from_slice(bytes: &[u8]) -> HolderInvocationInputResult<Self> {
        copy_fixed_digest(HolderInvocationFixedField::RequestBinding, bytes).map(Self::from_bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; HOLDER_INVOCATION_DIGEST_BYTES] {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HolderInvocationNonce([u8; HOLDER_INVOCATION_NONCE_BYTES]);

impl HolderInvocationNonce {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; HOLDER_INVOCATION_NONCE_BYTES]) -> Self {
        Self(bytes)
    }

    /// Copy one exact-width holder-invocation nonce.
    ///
    /// # Errors
    ///
    /// Returns [`HolderInvocationInputError`] when the nonce has the wrong width.
    pub const fn try_from_slice(bytes: &[u8]) -> HolderInvocationInputResult<Self> {
        if bytes.len() != HOLDER_INVOCATION_NONCE_BYTES {
            return Err(HolderInvocationInputError::InvalidFixedWidth {
                field: HolderInvocationFixedField::Nonce,
                actual: bytes.len(),
                expected: HOLDER_INVOCATION_NONCE_BYTES,
            });
        }
        let mut fixed = [0_u8; HOLDER_INVOCATION_NONCE_BYTES];
        fixed.copy_from_slice(bytes);
        Ok(Self::from_bytes(fixed))
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; HOLDER_INVOCATION_NONCE_BYTES] {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HolderInvocationTokenReference([u8; HOLDER_INVOCATION_DIGEST_BYTES]);

impl HolderInvocationTokenReference {
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; HOLDER_INVOCATION_DIGEST_BYTES] {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HolderInvocationRequestDigest([u8; HOLDER_INVOCATION_DIGEST_BYTES]);

impl HolderInvocationRequestDigest {
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; HOLDER_INVOCATION_DIGEST_BYTES] {
        &self.0
    }
}

/// Fixed evidence that binds a downstream profile to one exact request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OperationBinding {
    profile: OperationProfileId,
    request: RequestBindingDigest,
}

impl OperationBinding {
    #[must_use]
    pub const fn new(profile: OperationProfileId, request: RequestBindingDigest) -> Self {
        Self { profile, request }
    }

    #[must_use]
    pub const fn profile(self) -> OperationProfileId {
        self.profile
    }

    #[must_use]
    pub const fn request_binding(self) -> RequestBindingDigest {
        self.request
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HolderInvocationLimitError {
    ConfiguredFieldLimitTooLarge {
        field: HolderInvocationField,
        configured: usize,
        hard_maximum: usize,
    },
    CanonicalRequestTooLarge {
        byte_len: usize,
        hard_maximum: usize,
    },
    AllocationFailed {
        requested_bytes: usize,
    },
    Canonical {
        source: HolderInvocationCanonicalError,
    },
}

impl fmt::Display for HolderInvocationLimitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConfiguredFieldLimitTooLarge {
                field,
                configured,
                hard_maximum,
            } => write!(
                formatter,
                "configured {} limit {configured} exceeds reviewed hard maximum {hard_maximum}",
                field.as_str()
            ),
            Self::CanonicalRequestTooLarge { byte_len, hard_maximum } => write!(
                formatter,
                "canonical request byte length {byte_len} exceeds reviewed hard maximum {hard_maximum}"
            ),
            Self::AllocationFailed { requested_bytes } => {
                write!(formatter, "failed to allocate exact canonical request buffer of {requested_bytes} bytes")
            }
            Self::Canonical { source } => write!(formatter, "canonical field limit is invalid: {source}"),
        }
    }
}

impl std::error::Error for HolderInvocationLimitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Canonical { source } => Some(source),
            Self::ConfiguredFieldLimitTooLarge { .. }
            | Self::CanonicalRequestTooLarge { .. }
            | Self::AllocationFailed { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HolderInvocationLimits {
    fields: HolderInvocationFieldLimits,
}

impl HolderInvocationLimits {
    /// Construct explicit canonical field limits under reviewed hard maxima.
    ///
    /// # Errors
    ///
    /// Returns [`HolderInvocationLimitError`] when a configured limit exceeds a
    /// reviewed maximum or cannot fit in the canonical format.
    pub fn try_new(
        max_audience_bytes: usize,
        max_resource_bytes: usize,
        max_ability_bytes: usize,
    ) -> Result<Self, HolderInvocationLimitError> {
        admit_configured_limit(HolderInvocationField::Audience, max_audience_bytes, HARD_MAX_AUDIENCE_BYTES)?;
        admit_configured_limit(HolderInvocationField::Resource, max_resource_bytes, HARD_MAX_RESOURCE_BYTES)?;
        admit_configured_limit(HolderInvocationField::Ability, max_ability_bytes, HARD_MAX_ABILITY_BYTES)?;
        let fields = HolderInvocationFieldLimits::try_new(max_audience_bytes, max_resource_bytes, max_ability_bytes)
            .map_err(|source| HolderInvocationLimitError::Canonical { source })?;
        Ok(Self { fields })
    }

    const fn fields(self) -> HolderInvocationFieldLimits {
        self.fields
    }

    #[must_use]
    pub const fn max_audience_bytes(self) -> usize {
        self.fields.audience_bytes()
    }

    #[must_use]
    pub const fn max_resource_bytes(self) -> usize {
        self.fields.resource_bytes()
    }

    #[must_use]
    pub const fn max_ability_bytes(self) -> usize {
        self.fields.ability_bytes()
    }

    #[must_use]
    pub const fn hard_max_canonical_bytes() -> usize {
        HARD_MAX_CANONICAL_BYTES
    }
}

impl Default for HolderInvocationLimits {
    fn default() -> Self {
        Self::try_new(DEFAULT_MAX_AUDIENCE_BYTES, DEFAULT_MAX_RESOURCE_BYTES, DEFAULT_MAX_ABILITY_BYTES)
            .expect("default holder-invocation limits must fit the canonical format")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationProfileAdmissionError {
    UnknownProfile,
    Backend,
}

impl fmt::Display for OperationProfileAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownProfile => formatter.write_str("operation profile is unknown"),
            Self::Backend => formatter.write_str("operation-profile admission backend failed"),
        }
    }
}

impl std::error::Error for OperationProfileAdmissionError {}

pub trait OperationProfileAdmission {
    /// Observe whether a downstream-owned operation-profile identity is admitted.
    ///
    /// This lookup must not consume quota or mutate admission state. Consumptive
    /// downstream effects occur after this UCAN API returns live success.
    ///
    /// # Errors
    ///
    /// Returns [`OperationProfileAdmissionError`] when the profile is unknown or the registry
    /// cannot answer.
    fn admit_operation_profile(
        &self,
        profile: &OperationProfileId,
    ) -> std::result::Result<(), OperationProfileAdmissionError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HolderInvocationReplayError {
    Duplicate,
    Stale,
    Malformed,
    Denied,
    Backend,
}

impl fmt::Display for HolderInvocationReplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate => formatter.write_str("holder invocation nonce is a duplicate"),
            Self::Stale => formatter.write_str("holder invocation nonce is stale"),
            Self::Malformed => formatter.write_str("holder invocation nonce is malformed for caller policy"),
            Self::Denied => formatter.write_str("holder invocation replay admission denied the request"),
            Self::Backend => formatter.write_str("holder invocation replay backend failed"),
        }
    }
}

impl std::error::Error for HolderInvocationReplayError {}

pub struct HolderInvocationReplayCandidate<'a> {
    audience: &'a AudienceDid,
    nonce: HolderInvocationNonce,
    request_digest: HolderInvocationRequestDigest,
}

impl HolderInvocationReplayCandidate<'_> {
    #[must_use]
    pub const fn audience(&self) -> &AudienceDid {
        self.audience
    }

    #[must_use]
    pub const fn nonce(&self) -> HolderInvocationNonce {
        self.nonce
    }

    #[must_use]
    pub const fn request_digest(&self) -> HolderInvocationRequestDigest {
        self.request_digest
    }
}

pub trait HolderInvocationReplayAdmission {
    /// Admit one verified audience-scoped nonce as the final commit point.
    ///
    /// This is the only state-consuming boundary in holder-invocation
    /// verification. UCAN performs no work after this method returns success.
    ///
    /// # Errors
    ///
    /// Returns [`HolderInvocationReplayError`] according to caller-owned replay policy.
    fn admit_holder_invocation(
        &self,
        candidate: &HolderInvocationReplayCandidate<'_>,
    ) -> std::result::Result<(), HolderInvocationReplayError>;
}

pub struct HolderInvocationSigningRequest<'a> {
    compact_token: &'a str,
    audience: &'a AudienceDid,
    resource: &'a str,
    ability: &'a str,
    operation: OperationBinding,
    nonce: HolderInvocationNonce,
}

impl<'a> HolderInvocationSigningRequest<'a> {
    #[must_use]
    pub const fn new(
        compact_token: &'a str,
        audience: &'a AudienceDid,
        resource: &'a str,
        ability: &'a str,
        operation: OperationBinding,
        nonce: HolderInvocationNonce,
    ) -> Self {
        Self {
            compact_token,
            audience,
            resource,
            ability,
            operation,
            nonce,
        }
    }
}

pub struct HolderInvocationRequest<'a> {
    compact_token: &'a str,
    resource: &'a str,
    ability: &'a str,
    operation: OperationBinding,
    nonce: HolderInvocationNonce,
    signature: &'a SignerSignature,
}

impl<'a> HolderInvocationRequest<'a> {
    #[must_use]
    pub const fn new(
        compact_token: &'a str,
        resource: &'a str,
        ability: &'a str,
        operation: OperationBinding,
        nonce: HolderInvocationNonce,
        signature: &'a SignerSignature,
    ) -> Self {
        Self {
            compact_token,
            resource,
            ability,
            operation,
            nonce,
            signature,
        }
    }

    #[must_use]
    pub const fn compact_token(&self) -> &str {
        self.compact_token
    }

    #[must_use]
    pub const fn resource(&self) -> &str {
        self.resource
    }

    #[must_use]
    pub const fn ability(&self) -> &str {
        self.ability
    }

    #[must_use]
    pub const fn operation(&self) -> OperationBinding {
        self.operation
    }

    #[must_use]
    pub const fn nonce(&self) -> HolderInvocationNonce {
        self.nonce
    }
}

pub struct HolderTokenVerificationContext<'a> {
    time: VerificationTime,
    key_resolver: &'a dyn KeyResolver,
    proof_store: &'a dyn ProofStore,
    revocations: &'a dyn RevocationChecker,
    limits: VerificationLimits,
}

impl<'a> HolderTokenVerificationContext<'a> {
    #[must_use]
    pub const fn new(
        time: VerificationTime,
        key_resolver: &'a dyn KeyResolver,
        proof_store: &'a dyn ProofStore,
        revocations: &'a dyn RevocationChecker,
        limits: VerificationLimits,
    ) -> Self {
        Self {
            time,
            key_resolver,
            proof_store,
            revocations,
            limits,
        }
    }
}

pub struct HolderInvocationPolicyContext<'a> {
    caveats: &'a dyn CaveatPolicySet,
    profiles: &'a dyn OperationProfileAdmission,
    replay: &'a dyn HolderInvocationReplayAdmission,
    limits: HolderInvocationLimits,
}

impl<'a> HolderInvocationPolicyContext<'a> {
    #[must_use]
    pub const fn new(
        caveats: &'a dyn CaveatPolicySet,
        profiles: &'a dyn OperationProfileAdmission,
        replay: &'a dyn HolderInvocationReplayAdmission,
        limits: HolderInvocationLimits,
    ) -> Self {
        Self {
            caveats,
            profiles,
            replay,
            limits,
        }
    }
}

/// Live holder-signed authorization returned after the final replay commit.
///
/// This value has private construction and has no clone or serde surface. It
/// does not prove transport identity, downstream action semantics, request
/// canonicalizer correctness, nonce durability, rate-limit effects, or safe execution.
pub struct HolderInvocationAuthorized {
    issuer: IssuerDid,
    audience: AudienceDid,
    resource: String,
    ability: String,
    matched_capability: CapabilityDocument,
    effective_time_bounds: TokenTimeBounds,
    token_reference: HolderInvocationTokenReference,
    operation: OperationBinding,
    nonce: HolderInvocationNonce,
    request_digest: HolderInvocationRequestDigest,
}

impl fmt::Debug for HolderInvocationAuthorized {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HolderInvocationAuthorized")
            .field("issuer", &self.issuer)
            .field("audience", &self.audience)
            .field("resource", &self.resource)
            .field("ability", &self.ability)
            .field("matched_capability", &self.matched_capability)
            .field("effective_time_bounds", &self.effective_time_bounds)
            .field("token_reference", &self.token_reference)
            .field("operation", &self.operation)
            .field("nonce", &self.nonce)
            .field("request_digest", &self.request_digest)
            .finish()
    }
}

impl HolderInvocationAuthorized {
    #[must_use]
    pub const fn issuer(&self) -> &IssuerDid {
        &self.issuer
    }

    #[must_use]
    pub const fn audience(&self) -> &AudienceDid {
        &self.audience
    }

    #[must_use]
    pub const fn resource(&self) -> &str {
        self.resource.as_str()
    }

    #[must_use]
    pub const fn ability(&self) -> &str {
        self.ability.as_str()
    }

    #[must_use]
    pub const fn matched_capability(&self) -> &CapabilityDocument {
        &self.matched_capability
    }

    #[must_use]
    pub const fn effective_time_bounds(&self) -> TokenTimeBounds {
        self.effective_time_bounds
    }

    #[must_use]
    pub const fn token_reference(&self) -> HolderInvocationTokenReference {
        self.token_reference
    }

    #[must_use]
    pub const fn operation(&self) -> OperationBinding {
        self.operation
    }

    #[must_use]
    pub const fn nonce(&self) -> HolderInvocationNonce {
        self.nonce
    }

    #[must_use]
    pub const fn request_digest(&self) -> HolderInvocationRequestDigest {
        self.request_digest
    }
}

#[derive(Debug)]
pub enum HolderInvocationError {
    Input { source: HolderInvocationInputError },
    Canonical { source: HolderInvocationCanonicalError },
    Limit { source: HolderInvocationLimitError },
    Token { source: TokenError },
    Authorization { reason: AuthorizationDenialReason },
    KeyResolution { source: KeyResolverError },
    ResolvedAudienceKeyMismatch,
    Signer { source: SignerError },
    SignerAudienceMismatch,
    OperationProfile { source: OperationProfileAdmissionError },
    SignaturePrimitive { source: VerifiedPrimitiveError },
    InvalidHolderSignature,
    Replay { source: HolderInvocationReplayError },
}

impl fmt::Display for HolderInvocationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input { source } => write!(formatter, "invalid holder invocation input: {source}"),
            Self::Canonical { source } => write!(formatter, "invalid holder invocation canonical request: {source}"),
            Self::Limit { source } => write!(formatter, "holder invocation limit failed: {source}"),
            Self::Token { source } => write!(formatter, "compact token verification failed: {source}"),
            Self::Authorization { reason } => write!(formatter, "audience capability authorization failed: {reason:?}"),
            Self::KeyResolution { source } => write!(formatter, "audience key resolution failed: {source}"),
            Self::ResolvedAudienceKeyMismatch => {
                formatter.write_str("resolved audience public key does not encode the verified audience DID")
            }
            Self::Signer { source } => write!(formatter, "holder signer failed: {source}"),
            Self::SignerAudienceMismatch => {
                formatter.write_str("holder signer DID does not equal the supplied audience DID")
            }
            Self::OperationProfile { source } => write!(formatter, "operation profile admission failed: {source}"),
            Self::SignaturePrimitive { source } => write!(formatter, "holder signature primitive failed: {source}"),
            Self::InvalidHolderSignature => {
                formatter.write_str("holder signature is invalid for the exact canonical request")
            }
            Self::Replay { source } => write!(formatter, "holder invocation replay admission failed: {source}"),
        }
    }
}

impl std::error::Error for HolderInvocationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Input { source } => Some(source),
            Self::Canonical { source } => Some(source),
            Self::Limit { source } => Some(source),
            Self::Token { source } => Some(source),
            Self::KeyResolution { source } => Some(source),
            Self::Signer { source } => Some(source),
            Self::OperationProfile { source } => Some(source),
            Self::SignaturePrimitive { source } => Some(source),
            Self::Replay { source } => Some(source),
            Self::Authorization { .. }
            | Self::ResolvedAudienceKeyMismatch
            | Self::SignerAudienceMismatch
            | Self::InvalidHolderSignature => None,
        }
    }
}

/// Sign one bounded holder request with a signer whose DID equals the supplied audience.
///
/// This helper does not verify the compact token. The verifier obtains audience
/// truth only from full token verification.
///
/// # Errors
///
/// Returns [`HolderInvocationError`] for oversized input, signer identity
/// mismatch, canonicalization failure, or signing failure.
pub fn sign_holder_invocation<S>(
    request: &HolderInvocationSigningRequest<'_>,
    signer: &S,
    token_limits: VerificationLimits,
    holder_limits: HolderInvocationLimits,
) -> HolderInvocationResult<SignerSignature>
where
    S: TokenSigner + ?Sized,
{
    enforce_signing_token_bound(request.compact_token, token_limits)?;
    let token_reference = token_reference(request.compact_token);
    let canonical = canonical_request(
        &token_reference,
        request.audience,
        request.resource,
        request.ability,
        request.operation,
        request.nonce,
        holder_limits,
    )?;
    let signer_did = validate_signer_identity(signer).map_err(|source| HolderInvocationError::Token { source })?;
    if signer_did.as_str() != request.audience.as_str() {
        return Err(HolderInvocationError::SignerAudienceMismatch);
    }
    let signature = signer.sign(canonical.as_ref()).map_err(|source| HolderInvocationError::Signer { source })?;
    validate_signer_signature(&signature).map_err(|source| HolderInvocationError::Signer { source })?;
    Ok(signature)
}

/// Verify one borrowed holder-signed invocation through explicit injected boundaries.
///
/// Token verification completes before request canonicalization. The fixed
/// order is token verification, bounds and canonicalization, audience signature,
/// non-consuming profile admission, non-consuming capability and caveat policy,
/// prebuilt success, and one replay commit. No work occurs after replay accepts.
/// Profile truth, replay truth, downstream action semantics, and execution remain caller-owned.
///
/// # Errors
///
/// Returns [`HolderInvocationError`] when any token, field, authorization,
/// profile, audience-key, signature, or replay boundary fails.
pub fn verify_holder_signed_invocation(
    request: &HolderInvocationRequest<'_>,
    token_context: &HolderTokenVerificationContext<'_>,
    policy_context: &HolderInvocationPolicyContext<'_>,
) -> HolderInvocationResult<HolderInvocationAuthorized> {
    let verified = verify_compact_token_str_with_resolvers_and_revocations(
        request.compact_token,
        token_context.time,
        token_context.key_resolver,
        token_context.proof_store,
        token_context.revocations,
        token_context.limits,
    )
    .map_err(|source| HolderInvocationError::Token { source })?;

    let token_reference = token_reference(request.compact_token);
    let canonical = canonical_request(
        &token_reference,
        verified.audience(),
        request.resource,
        request.ability,
        request.operation,
        request.nonce,
        policy_context.limits,
    )?;
    let audience_issuer = IssuerDid::new(verified.audience().as_str().to_owned())
        .map_err(|source| HolderInvocationError::Token { source })?;
    let public_key = token_context
        .key_resolver
        .resolve_key(&audience_issuer)
        .map_err(|source| HolderInvocationError::KeyResolution { source })?;
    let resolved_key_issuer = public_key.issuer().map_err(|source| HolderInvocationError::Token { source })?;
    if resolved_key_issuer != audience_issuer {
        return Err(HolderInvocationError::ResolvedAudienceKeyMismatch);
    }
    let valid = verify_ed25519_signature(public_key.as_bytes(), canonical.as_ref(), request.signature.as_bytes())
        .map_err(|source| HolderInvocationError::SignaturePrimitive { source })?;
    if !valid {
        return Err(HolderInvocationError::InvalidHolderSignature);
    }

    policy_context
        .profiles
        .admit_operation_profile(&request.operation.profile())
        .map_err(|source| HolderInvocationError::OperationProfile { source })?;
    let matched_capability = authorize_request(&verified, request, policy_context.caveats)?;
    let request_digest = HolderInvocationRequestDigest(*blake3::hash(canonical.as_ref()).as_bytes());
    let authorized = HolderInvocationAuthorized {
        issuer: verified.issuer().clone(),
        audience: verified.audience().clone(),
        resource: request.resource.to_owned(),
        ability: request.ability.to_owned(),
        matched_capability,
        effective_time_bounds: verified.effective_delegation().time_bounds(),
        token_reference,
        operation: request.operation,
        nonce: request.nonce,
        request_digest,
    };
    let replay_candidate = HolderInvocationReplayCandidate {
        audience: authorized.audience(),
        nonce: authorized.nonce(),
        request_digest: authorized.request_digest(),
    };
    policy_context
        .replay
        .admit_holder_invocation(&replay_candidate)
        .map_err(|source| HolderInvocationError::Replay { source })?;
    Ok(authorized)
}

fn authorize_request(
    verified: &super::VerifiedToken,
    request: &HolderInvocationRequest<'_>,
    caveats: &dyn CaveatPolicySet,
) -> HolderInvocationResult<CapabilityDocument> {
    match verified.authorize_with_policies(request.resource, request.ability, caveats) {
        AuthorizationDecision::Allowed { matched_capability } => Ok(matched_capability),
        AuthorizationDecision::Denied { reason } => Err(HolderInvocationError::Authorization { reason }),
    }
}

const fn enforce_signing_token_bound(token: &str, limits: VerificationLimits) -> HolderInvocationResult<()> {
    if token.len() > limits.max_compact_token_bytes() {
        return Err(HolderInvocationError::Token {
            source: TokenError::LimitExceeded {
                source: super::VerificationLimitError::CompactTokenTooLarge {
                    byte_len: token.len(),
                    max_byte_len: limits.max_compact_token_bytes(),
                },
            },
        });
    }
    Ok(())
}

fn canonical_request(
    token_reference: &HolderInvocationTokenReference,
    audience: &AudienceDid,
    resource: &str,
    ability: &str,
    operation: OperationBinding,
    nonce: HolderInvocationNonce,
    limits: HolderInvocationLimits,
) -> HolderInvocationResult<Vec<u8>> {
    let profile = operation.profile();
    let request_binding = operation.request_binding();
    let fields = HolderInvocationCanonicalFields::new(
        token_reference.as_bytes(),
        audience.as_str(),
        resource,
        ability,
        profile.as_bytes(),
        request_binding.as_bytes(),
        nonce.as_bytes(),
    );
    let length = holder_invocation_canonical_len(&fields, limits.fields())
        .map_err(|source| HolderInvocationError::Canonical { source })?;
    if length > HARD_MAX_CANONICAL_BYTES {
        return Err(HolderInvocationError::Limit {
            source: HolderInvocationLimitError::CanonicalRequestTooLarge {
                byte_len: length,
                hard_maximum: HARD_MAX_CANONICAL_BYTES,
            },
        });
    }
    let mut canonical = allocate_exact_canonical(length)?;
    write_holder_invocation_canonical(&fields, limits.fields(), canonical.as_mut_slice())
        .map_err(|source| HolderInvocationError::Canonical { source })?;
    Ok(canonical)
}

fn allocate_exact_canonical(length: usize) -> HolderInvocationResult<Vec<u8>> {
    allocate_exact_canonical_with(length, |buffer, requested| buffer.try_reserve_exact(requested).is_ok())
}

fn allocate_exact_canonical_with<F>(length: usize, reserve: F) -> HolderInvocationResult<Vec<u8>>
where F: FnOnce(&mut Vec<u8>, usize) -> bool {
    let mut buffer = Vec::new();
    if !reserve(&mut buffer, length) {
        return Err(HolderInvocationError::Limit {
            source: HolderInvocationLimitError::AllocationFailed {
                requested_bytes: length,
            },
        });
    }
    assert!(buffer.capacity() >= length, "successful exact reserve must cover the requested length");
    buffer.resize(length, 0);
    Ok(buffer)
}

const fn admit_configured_limit(
    field: HolderInvocationField,
    configured: usize,
    hard_maximum: usize,
) -> Result<(), HolderInvocationLimitError> {
    if configured > hard_maximum {
        return Err(HolderInvocationLimitError::ConfiguredFieldLimitTooLarge {
            field,
            configured,
            hard_maximum,
        });
    }
    Ok(())
}

fn token_reference(token: &str) -> HolderInvocationTokenReference {
    HolderInvocationTokenReference(*blake3::hash(token.as_bytes()).as_bytes())
}

const fn copy_fixed_digest(
    field: HolderInvocationFixedField,
    bytes: &[u8],
) -> HolderInvocationInputResult<[u8; HOLDER_INVOCATION_DIGEST_BYTES]> {
    if bytes.len() != HOLDER_INVOCATION_DIGEST_BYTES {
        return Err(HolderInvocationInputError::InvalidFixedWidth {
            field,
            actual: bytes.len(),
            expected: HOLDER_INVOCATION_DIGEST_BYTES,
        });
    }
    let mut fixed = [0_u8; HOLDER_INVOCATION_DIGEST_BYTES];
    fixed.copy_from_slice(bytes);
    Ok(fixed)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::cell::RefCell;
    use std::collections::HashSet;

    use super::super::AudienceDid;
    use super::super::AuthorizationRequest;
    use super::super::CapabilitySet;
    use super::super::CaveatDecision;
    use super::super::CaveatDocument;
    use super::super::CaveatIdentifier;
    use super::super::CaveatPolicy;
    use super::super::CaveatPolicySet;
    use super::super::CompactToken;
    use super::super::DenyUnknownCaveats;
    use super::super::ED25519_SECRET_KEY_BYTES;
    use super::super::Ed25519InMemorySigner;
    use super::super::Ed25519PublicKey;
    use super::super::IssuerDid;
    use super::super::KeyResolutionContext;
    use super::super::KeyResolver;
    use super::super::KeyResolverError;
    use super::super::NoRevocations;
    use super::super::ProofCollection;
    use super::super::ProofReferences;
    use super::super::RevocationList;
    use super::super::SignerSignature;
    use super::super::TokenSigner;
    use super::super::TokenTimeBounds;
    use super::super::VerificationLimits;
    use super::super::VerificationTime;
    use super::super::issue_token_with_signer;
    use super::super::proof_reference;
    use super::DEFAULT_MAX_ABILITY_BYTES;
    use super::DEFAULT_MAX_AUDIENCE_BYTES;
    use super::DEFAULT_MAX_RESOURCE_BYTES;
    use super::HARD_MAX_ABILITY_BYTES;
    use super::HARD_MAX_AUDIENCE_BYTES;
    use super::HARD_MAX_CANONICAL_BYTES;
    use super::HARD_MAX_RESOURCE_BYTES;
    use super::HolderInvocationAuthorized;
    use super::HolderInvocationError;
    use super::HolderInvocationFixedField;
    use super::HolderInvocationInputError;
    use super::HolderInvocationLimitError;
    use super::HolderInvocationLimits;
    use super::HolderInvocationNonce;
    use super::HolderInvocationPolicyContext;
    use super::HolderInvocationReplayAdmission;
    use super::HolderInvocationReplayCandidate;
    use super::HolderInvocationReplayError;
    use super::HolderInvocationRequest;
    use super::HolderInvocationSigningRequest;
    use super::HolderTokenVerificationContext;
    use super::OperationBinding;
    use super::OperationProfileAdmission;
    use super::OperationProfileAdmissionError;
    use super::OperationProfileId;
    use super::RequestBindingDigest;
    use super::allocate_exact_canonical_with;
    use super::canonical_request;
    use super::sign_holder_invocation;
    use super::token_reference;
    use super::verify_holder_signed_invocation;
    use crate::CapabilityDocument;

    const ISSUER_KEY_BYTE: u8 = 11;
    const DELEGATE_KEY_BYTE: u8 = 13;
    const HOLDER_KEY_BYTE: u8 = 17;
    const WRONG_HOLDER_KEY_BYTE: u8 = 19;
    const PROFILE_BYTE: u8 = 23;
    const UNKNOWN_PROFILE_BYTE: u8 = 29;
    const NONCE_BYTE: u8 = 31;
    const OTHER_NONCE_BYTE: u8 = 37;
    const SIGNATURE_CORRUPTION_BYTE: u8 = 1;
    const TOKEN_NOT_BEFORE_SECONDS: u64 = 1_000;
    const TOKEN_EXPIRES_SECONDS: u64 = 2_000;
    const ALTERNATE_TOKEN_EXPIRES_SECONDS: u64 = 2_100;
    const VERIFY_AT_SECONDS: u64 = 1_500;
    const EXPIRED_VERIFY_AT_SECONDS: u64 = 2_500;
    const RESOURCE: &str = "did:key:z6Mkresource/photos";
    const PARENT_RESOURCE: &str = "did:key:z6Mkresource";
    const OTHER_RESOURCE: &str = "did:key:z6Mkresource/videos";
    const ABILITY: &str = "store/add";
    const OTHER_ABILITY: &str = "store/remove";
    const REQUEST_BYTES: &[u8] = b"basalt-request-v1";
    const OTHER_REQUEST_BYTES: &[u8] = b"basalt-request-v2";
    const CAVEAT_DOMAIN: &str = "example";
    const CAVEAT_TYPE: &str = "approval";
    const CAVEAT_KEY: &str = "required";
    const CAVEAT_PAYLOAD: &[u8] = b"yes";
    const SHORT_FIXED_WIDTH: usize = 31;
    const SHORT_RESOURCE_LIMIT: usize = 8;
    const SIMULATED_ALLOCATION_BYTES: usize = 128;

    struct DirectFixture {
        issuer: Ed25519InMemorySigner,
        holder: Ed25519InMemorySigner,
        audience: AudienceDid,
        token: CompactToken,
        keys: KeyResolutionContext,
    }

    impl DirectFixture {
        fn new() -> Self {
            Self::with_capability_and_expiration(direct_capability(), TOKEN_EXPIRES_SECONDS)
        }

        fn with_capability_and_expiration(capability: CapabilityDocument, expires_at_seconds: u64) -> Self {
            let issuer = signer(ISSUER_KEY_BYTE);
            let holder = signer(HOLDER_KEY_BYTE);
            let audience = AudienceDid::from(holder.issuer().expect("holder DID must derive"));
            let token = issue_token_with_signer(
                &issuer,
                &audience,
                &CapabilitySet::new(vec![capability]).expect("test capability set must validate"),
                &ProofReferences::empty(),
                TokenTimeBounds::new(TOKEN_NOT_BEFORE_SECONDS, expires_at_seconds)
                    .expect("test time bounds must validate"),
            )
            .expect("direct test token must issue");
            let keys = KeyResolutionContext::new(vec![
                issuer.verification_key().expect("issuer verification key must derive"),
                holder.verification_key().expect("holder verification key must derive"),
            ]);
            Self {
                issuer,
                holder,
                audience,
                token,
                keys,
            }
        }
    }

    struct DelegatedFixture {
        holder: Ed25519InMemorySigner,
        audience: AudienceDid,
        token: CompactToken,
        keys: KeyResolutionContext,
        proofs: ProofCollection,
    }

    impl DelegatedFixture {
        fn new() -> Self {
            let issuer = signer(ISSUER_KEY_BYTE);
            let delegate = signer(DELEGATE_KEY_BYTE);
            let holder = signer(HOLDER_KEY_BYTE);
            let delegate_audience = AudienceDid::from(delegate.issuer().expect("delegate DID must derive"));
            let audience = AudienceDid::from(holder.issuer().expect("holder DID must derive"));
            let parent = issue_token_with_signer(
                &issuer,
                &delegate_audience,
                &CapabilitySet::new(vec![direct_capability()]).expect("parent capability set must validate"),
                &ProofReferences::empty(),
                test_time_bounds(),
            )
            .expect("parent token must issue");
            let parent_reference = proof_reference(&parent);
            let child = issue_token_with_signer(
                &delegate,
                &audience,
                &CapabilitySet::new(vec![direct_capability()]).expect("child capability set must validate"),
                &ProofReferences::from_references(vec![parent_reference]),
                test_time_bounds(),
            )
            .expect("child token must issue");
            let keys = KeyResolutionContext::new(vec![
                issuer.verification_key().expect("issuer verification key must derive"),
                delegate.verification_key().expect("delegate verification key must derive"),
                holder.verification_key().expect("holder verification key must derive"),
            ]);
            Self {
                holder,
                audience,
                token: child,
                keys,
                proofs: ProofCollection::from_tokens(vec![parent]),
            }
        }
    }

    struct SubstitutingAudienceResolver<'a> {
        inner: &'a KeyResolutionContext,
        audience: IssuerDid,
        substitute: Ed25519PublicKey,
    }

    impl KeyResolver for SubstitutingAudienceResolver<'_> {
        fn resolve_key(&self, issuer: &IssuerDid) -> Result<Ed25519PublicKey, KeyResolverError> {
            if issuer == &self.audience {
                Ok(self.substitute.clone())
            } else {
                self.inner.resolve_key(issuer)
            }
        }
    }

    struct KnownProfile {
        profile: OperationProfileId,
        calls: Cell<usize>,
    }

    impl KnownProfile {
        const fn new(profile: OperationProfileId) -> Self {
            Self {
                profile,
                calls: Cell::new(0),
            }
        }
    }

    impl OperationProfileAdmission for KnownProfile {
        fn admit_operation_profile(&self, profile: &OperationProfileId) -> Result<(), OperationProfileAdmissionError> {
            self.calls.set(self.calls.get() + 1);
            if profile == &self.profile {
                Ok(())
            } else {
                Err(OperationProfileAdmissionError::UnknownProfile)
            }
        }
    }

    #[derive(Default)]
    struct RecordingReplay {
        seen: RefCell<HashSet<(String, [u8; ucan_core::holder_invocation::HOLDER_INVOCATION_NONCE_BYTES])>>,
        calls: Cell<usize>,
    }

    impl HolderInvocationReplayAdmission for RecordingReplay {
        fn admit_holder_invocation(
            &self,
            candidate: &HolderInvocationReplayCandidate<'_>,
        ) -> Result<(), HolderInvocationReplayError> {
            self.calls.set(self.calls.get() + 1);
            let key = (candidate.audience().as_str().to_owned(), *candidate.nonce().as_bytes());
            if self.seen.borrow_mut().insert(key) {
                Ok(())
            } else {
                Err(HolderInvocationReplayError::Duplicate)
            }
        }
    }

    struct OrderedProfile<'a> {
        events: &'a RefCell<Vec<&'static str>>,
        admit: bool,
    }

    impl OperationProfileAdmission for OrderedProfile<'_> {
        fn admit_operation_profile(&self, _profile: &OperationProfileId) -> Result<(), OperationProfileAdmissionError> {
            self.events.borrow_mut().push("profile");
            if self.admit {
                Ok(())
            } else {
                Err(OperationProfileAdmissionError::UnknownProfile)
            }
        }
    }

    struct OrderedCaveatPolicy<'a> {
        events: &'a RefCell<Vec<&'static str>>,
        satisfy: bool,
    }

    impl CaveatPolicy for OrderedCaveatPolicy<'_> {
        fn evaluate(&self, _caveat: &CaveatDocument, _request: &AuthorizationRequest) -> CaveatDecision {
            self.events.borrow_mut().push("caveat");
            if self.satisfy {
                CaveatDecision::Satisfied
            } else {
                CaveatDecision::Rejected {
                    message: "test caveat denial".to_owned(),
                }
            }
        }
    }

    struct OrderedCaveatSet<'a> {
        policy: OrderedCaveatPolicy<'a>,
    }

    impl CaveatPolicySet for OrderedCaveatSet<'_> {
        fn policy_for(&self, _caveat: &CaveatIdentifier) -> Option<&dyn CaveatPolicy> {
            Some(&self.policy)
        }
    }

    struct OrderedReplay<'a> {
        events: &'a RefCell<Vec<&'static str>>,
        admit: bool,
        calls: Cell<usize>,
    }

    impl HolderInvocationReplayAdmission for OrderedReplay<'_> {
        fn admit_holder_invocation(
            &self,
            _candidate: &HolderInvocationReplayCandidate<'_>,
        ) -> Result<(), HolderInvocationReplayError> {
            self.calls.set(self.calls.get() + 1);
            self.events.borrow_mut().push("replay");
            if self.admit {
                Ok(())
            } else {
                Err(HolderInvocationReplayError::Denied)
            }
        }
    }

    struct TestVerifier<'a> {
        time: VerificationTime,
        keys: &'a KeyResolutionContext,
        proofs: &'a ProofCollection,
        revocations: &'a dyn super::super::RevocationChecker,
        profiles: &'a dyn OperationProfileAdmission,
        replay: &'a dyn HolderInvocationReplayAdmission,
        caveats: &'a dyn super::super::CaveatPolicySet,
        token_limits: VerificationLimits,
        holder_limits: HolderInvocationLimits,
    }

    impl TestVerifier<'_> {
        fn verify(
            &self,
            request: &HolderInvocationRequest<'_>,
        ) -> super::HolderInvocationResult<HolderInvocationAuthorized> {
            let token_context = HolderTokenVerificationContext::new(
                self.time,
                self.keys,
                self.proofs,
                self.revocations,
                self.token_limits,
            );
            let policy_context =
                HolderInvocationPolicyContext::new(self.caveats, self.profiles, self.replay, self.holder_limits);
            verify_holder_signed_invocation(request, &token_context, &policy_context)
        }
    }

    fn signer(byte: u8) -> Ed25519InMemorySigner {
        Ed25519InMemorySigner::from_seed_bytes([byte; ED25519_SECRET_KEY_BYTES])
    }

    fn direct_capability() -> CapabilityDocument {
        CapabilityDocument::new(PARENT_RESOURCE.to_owned(), ABILITY.to_owned()).expect("test capability must validate")
    }

    fn caveated_capability() -> CapabilityDocument {
        let caveat = CaveatDocument::new(
            CAVEAT_DOMAIN.to_owned(),
            CAVEAT_TYPE.to_owned(),
            CAVEAT_KEY.to_owned(),
            CAVEAT_PAYLOAD.to_vec(),
        )
        .expect("test caveat must validate");
        CapabilityDocument::with_caveats(PARENT_RESOURCE.to_owned(), ABILITY.to_owned(), vec![caveat])
            .expect("caveated capability must validate")
    }

    fn test_time_bounds() -> TokenTimeBounds {
        TokenTimeBounds::new(TOKEN_NOT_BEFORE_SECONDS, TOKEN_EXPIRES_SECONDS).expect("test time bounds must validate")
    }

    fn profile() -> OperationProfileId {
        OperationProfileId::from_bytes([PROFILE_BYTE; ucan_core::holder_invocation::HOLDER_INVOCATION_DIGEST_BYTES])
    }

    fn operation() -> OperationBinding {
        OperationBinding::new(profile(), RequestBindingDigest::from_bytes(*blake3::hash(REQUEST_BYTES).as_bytes()))
    }

    fn alternate_operation() -> OperationBinding {
        OperationBinding::new(
            OperationProfileId::from_bytes(
                [UNKNOWN_PROFILE_BYTE; ucan_core::holder_invocation::HOLDER_INVOCATION_DIGEST_BYTES],
            ),
            RequestBindingDigest::from_bytes(*blake3::hash(OTHER_REQUEST_BYTES).as_bytes()),
        )
    }

    fn nonce() -> HolderInvocationNonce {
        HolderInvocationNonce::from_bytes([NONCE_BYTE; ucan_core::holder_invocation::HOLDER_INVOCATION_NONCE_BYTES])
    }

    fn other_nonce() -> HolderInvocationNonce {
        HolderInvocationNonce::from_bytes(
            [OTHER_NONCE_BYTE; ucan_core::holder_invocation::HOLDER_INVOCATION_NONCE_BYTES],
        )
    }

    fn sign_request(
        token: &CompactToken,
        audience: &AudienceDid,
        resource: &str,
        ability: &str,
        operation: OperationBinding,
        nonce: HolderInvocationNonce,
        signer: &Ed25519InMemorySigner,
    ) -> SignerSignature {
        sign_holder_invocation(
            &HolderInvocationSigningRequest::new(token.as_str(), audience, resource, ability, operation, nonce),
            signer,
            VerificationLimits::default(),
            HolderInvocationLimits::default(),
        )
        .expect("test holder request must sign")
    }

    #[test]
    fn direct_holder_signed_invocation_succeeds() {
        let fixture = DirectFixture::new();
        let proofs = ProofCollection::empty();
        let revocations = NoRevocations;
        let profiles = KnownProfile::new(profile());
        let replay = RecordingReplay::default();
        let signature =
            sign_request(&fixture.token, &fixture.audience, RESOURCE, ABILITY, operation(), nonce(), &fixture.holder);
        let verifier = TestVerifier {
            time: VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
            keys: &fixture.keys,
            proofs: &proofs,
            revocations: &revocations,
            profiles: &profiles,
            replay: &replay,
            caveats: &DenyUnknownCaveats,
            token_limits: VerificationLimits::default(),
            holder_limits: HolderInvocationLimits::default(),
        };
        let request =
            HolderInvocationRequest::new(fixture.token.as_str(), RESOURCE, ABILITY, operation(), nonce(), &signature);

        let authorized = verifier.verify(&request).expect("direct holder invocation must verify");

        assert_eq!(authorized.issuer().as_str(), fixture.issuer.issuer().expect("issuer DID").as_str());
        assert_eq!(authorized.audience(), &fixture.audience);
        assert_eq!(authorized.resource(), RESOURCE);
        assert_eq!(authorized.ability(), ABILITY);
        assert_eq!(authorized.operation(), operation());
        assert_eq!(authorized.nonce(), nonce());
        assert_eq!(authorized.effective_time_bounds(), test_time_bounds());
        assert_eq!(authorized.token_reference().as_bytes(), blake3::hash(fixture.token.as_str().as_bytes()).as_bytes());
        assert_eq!(profiles.calls.get(), 1);
        assert_eq!(replay.calls.get(), 1);
    }

    #[test]
    fn delegated_holder_signed_invocation_succeeds() {
        let fixture = DelegatedFixture::new();
        let revocations = NoRevocations;
        let profiles = KnownProfile::new(profile());
        let replay = RecordingReplay::default();
        let signature =
            sign_request(&fixture.token, &fixture.audience, RESOURCE, ABILITY, operation(), nonce(), &fixture.holder);
        let verifier = TestVerifier {
            time: VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
            keys: &fixture.keys,
            proofs: &fixture.proofs,
            revocations: &revocations,
            profiles: &profiles,
            replay: &replay,
            caveats: &DenyUnknownCaveats,
            token_limits: VerificationLimits::default(),
            holder_limits: HolderInvocationLimits::default(),
        };
        let request =
            HolderInvocationRequest::new(fixture.token.as_str(), RESOURCE, ABILITY, operation(), nonce(), &signature);

        let authorized = verifier.verify(&request).expect("delegated holder invocation must verify");

        assert_eq!(authorized.audience(), &fixture.audience);
        assert_eq!(authorized.matched_capability().ability, ABILITY);
        assert_eq!(replay.calls.get(), 1);
    }

    #[test]
    fn wrong_holder_signature_and_signer_audience_mismatch_fail() {
        let fixture = DirectFixture::new();
        let wrong_holder = signer(WRONG_HOLDER_KEY_BYTE);
        let signer_error = sign_holder_invocation(
            &HolderInvocationSigningRequest::new(
                fixture.token.as_str(),
                &fixture.audience,
                RESOURCE,
                ABILITY,
                operation(),
                nonce(),
            ),
            &wrong_holder,
            VerificationLimits::default(),
            HolderInvocationLimits::default(),
        )
        .expect_err("signing helper must reject a different audience DID");
        assert!(matches!(signer_error, HolderInvocationError::SignerAudienceMismatch));

        let token_reference = token_reference(fixture.token.as_str());
        let canonical = canonical_request(
            &token_reference,
            &fixture.audience,
            RESOURCE,
            ABILITY,
            operation(),
            nonce(),
            HolderInvocationLimits::default(),
        )
        .expect("test canonical request must build");
        let wrong_signature = wrong_holder.sign(canonical.as_ref()).expect("wrong holder can sign its own bytes");
        let proofs = ProofCollection::empty();
        let revocations = NoRevocations;
        let profiles = KnownProfile::new(profile());
        let replay = RecordingReplay::default();
        let verifier = TestVerifier {
            time: VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
            keys: &fixture.keys,
            proofs: &proofs,
            revocations: &revocations,
            profiles: &profiles,
            replay: &replay,
            caveats: &DenyUnknownCaveats,
            token_limits: VerificationLimits::default(),
            holder_limits: HolderInvocationLimits::default(),
        };
        let error = verifier
            .verify(&HolderInvocationRequest::new(
                fixture.token.as_str(),
                RESOURCE,
                ABILITY,
                operation(),
                nonce(),
                &wrong_signature,
            ))
            .expect_err("wrong holder signature must fail");

        assert!(matches!(error, HolderInvocationError::InvalidHolderSignature));
        assert_eq!(replay.calls.get(), 0);
    }

    #[test]
    fn resolved_audience_key_must_encode_verified_audience_did() {
        let fixture = DirectFixture::new();
        let wrong_holder = signer(WRONG_HOLDER_KEY_BYTE);
        let token_reference = token_reference(fixture.token.as_str());
        let canonical = canonical_request(
            &token_reference,
            &fixture.audience,
            RESOURCE,
            ABILITY,
            operation(),
            nonce(),
            HolderInvocationLimits::default(),
        )
        .expect("test canonical request must build");
        let wrong_signature = wrong_holder.sign(canonical.as_ref()).expect("wrong key must sign test bytes");
        let audience =
            IssuerDid::new(fixture.audience.as_str().to_owned()).expect("audience issuer form must validate");
        let resolver = SubstitutingAudienceResolver {
            inner: &fixture.keys,
            audience,
            substitute: wrong_holder.public_key().expect("wrong holder public key must derive"),
        };
        let proofs = ProofCollection::empty();
        let revocations = NoRevocations;
        let profiles = KnownProfile::new(profile());
        let replay = RecordingReplay::default();
        let token_context = HolderTokenVerificationContext::new(
            VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
            &resolver,
            &proofs,
            &revocations,
            VerificationLimits::default(),
        );
        let policy_context = HolderInvocationPolicyContext::new(
            &DenyUnknownCaveats,
            &profiles,
            &replay,
            HolderInvocationLimits::default(),
        );
        let request = HolderInvocationRequest::new(
            fixture.token.as_str(),
            RESOURCE,
            ABILITY,
            operation(),
            nonce(),
            &wrong_signature,
        );

        let error = verify_holder_signed_invocation(&request, &token_context, &policy_context)
            .expect_err("substituted audience key must fail DID binding");

        assert!(matches!(error, HolderInvocationError::ResolvedAudienceKeyMismatch));
        assert_eq!(profiles.calls.get(), 0);
        assert_eq!(replay.calls.get(), 0);
    }

    #[test]
    fn duplicate_nonce_is_rejected_after_first_success() {
        let fixture = DirectFixture::new();
        let proofs = ProofCollection::empty();
        let revocations = NoRevocations;
        let profiles = KnownProfile::new(profile());
        let replay = RecordingReplay::default();
        let signature =
            sign_request(&fixture.token, &fixture.audience, RESOURCE, ABILITY, operation(), nonce(), &fixture.holder);
        let verifier = TestVerifier {
            time: VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
            keys: &fixture.keys,
            proofs: &proofs,
            revocations: &revocations,
            profiles: &profiles,
            replay: &replay,
            caveats: &DenyUnknownCaveats,
            token_limits: VerificationLimits::default(),
            holder_limits: HolderInvocationLimits::default(),
        };
        let request =
            HolderInvocationRequest::new(fixture.token.as_str(), RESOURCE, ABILITY, operation(), nonce(), &signature);

        verifier.verify(&request).expect("first nonce admission must succeed");
        let error = verifier.verify(&request).expect_err("duplicate nonce must fail");

        assert!(matches!(error, HolderInvocationError::Replay {
            source: HolderInvocationReplayError::Duplicate
        }));
        assert_eq!(replay.calls.get(), 2);
    }

    #[test]
    fn nonce_width_and_nonce_substitution_fail() {
        let short = [0_u8; SHORT_FIXED_WIDTH];
        assert_eq!(
            HolderInvocationNonce::try_from_slice(&short),
            Err(HolderInvocationInputError::InvalidFixedWidth {
                field: HolderInvocationFixedField::Nonce,
                actual: SHORT_FIXED_WIDTH,
                expected: ucan_core::holder_invocation::HOLDER_INVOCATION_NONCE_BYTES,
            })
        );

        let fixture = DirectFixture::new();
        let signature =
            sign_request(&fixture.token, &fixture.audience, RESOURCE, ABILITY, operation(), nonce(), &fixture.holder);
        let proofs = ProofCollection::empty();
        let revocations = NoRevocations;
        let profiles = KnownProfile::new(profile());
        let replay = RecordingReplay::default();
        let verifier = TestVerifier {
            time: VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
            keys: &fixture.keys,
            proofs: &proofs,
            revocations: &revocations,
            profiles: &profiles,
            replay: &replay,
            caveats: &DenyUnknownCaveats,
            token_limits: VerificationLimits::default(),
            holder_limits: HolderInvocationLimits::default(),
        };
        let error = verifier
            .verify(&HolderInvocationRequest::new(
                fixture.token.as_str(),
                RESOURCE,
                ABILITY,
                operation(),
                other_nonce(),
                &signature,
            ))
            .expect_err("nonce substitution must invalidate the signature");

        assert!(matches!(error, HolderInvocationError::InvalidHolderSignature));
        assert_eq!(replay.calls.get(), 0);
    }

    #[test]
    fn request_action_target_and_profile_substitutions_fail() {
        let fixture = DirectFixture::new();
        let signature =
            sign_request(&fixture.token, &fixture.audience, RESOURCE, ABILITY, operation(), nonce(), &fixture.holder);
        let proofs = ProofCollection::empty();
        let revocations = NoRevocations;
        let replay = RecordingReplay::default();
        let profiles = KnownProfile::new(profile());
        let substituted_request = OperationBinding::new(
            profile(),
            RequestBindingDigest::from_bytes(*blake3::hash(OTHER_REQUEST_BYTES).as_bytes()),
        );
        let cases = [
            (RESOURCE, ABILITY, substituted_request),
            (RESOURCE, OTHER_ABILITY, operation()),
            (OTHER_RESOURCE, ABILITY, operation()),
            (RESOURCE, ABILITY, alternate_operation()),
        ];

        for (resource, ability, operation_binding) in cases {
            let verifier = TestVerifier {
                time: VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
                keys: &fixture.keys,
                proofs: &proofs,
                revocations: &revocations,
                profiles: &profiles,
                replay: &replay,
                caveats: &DenyUnknownCaveats,
                token_limits: VerificationLimits::default(),
                holder_limits: HolderInvocationLimits::default(),
            };
            let error = verifier
                .verify(&HolderInvocationRequest::new(
                    fixture.token.as_str(),
                    resource,
                    ability,
                    operation_binding,
                    nonce(),
                    &signature,
                ))
                .expect_err("substituted request field must fail");
            assert!(matches!(error, HolderInvocationError::InvalidHolderSignature));
        }
        assert_eq!(replay.calls.get(), 0);
    }

    #[test]
    fn token_substitution_invalidates_holder_signature() {
        let fixture = DirectFixture::new();
        let alternate =
            DirectFixture::with_capability_and_expiration(direct_capability(), ALTERNATE_TOKEN_EXPIRES_SECONDS);
        assert_ne!(fixture.token, alternate.token);
        let signature =
            sign_request(&fixture.token, &fixture.audience, RESOURCE, ABILITY, operation(), nonce(), &fixture.holder);
        let proofs = ProofCollection::empty();
        let revocations = NoRevocations;
        let profiles = KnownProfile::new(profile());
        let replay = RecordingReplay::default();
        let verifier = TestVerifier {
            time: VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
            keys: &alternate.keys,
            proofs: &proofs,
            revocations: &revocations,
            profiles: &profiles,
            replay: &replay,
            caveats: &DenyUnknownCaveats,
            token_limits: VerificationLimits::default(),
            holder_limits: HolderInvocationLimits::default(),
        };
        let error = verifier
            .verify(&HolderInvocationRequest::new(
                alternate.token.as_str(),
                RESOURCE,
                ABILITY,
                operation(),
                nonce(),
                &signature,
            ))
            .expect_err("token substitution must invalidate the signature");

        assert!(matches!(error, HolderInvocationError::InvalidHolderSignature));
        assert_eq!(replay.calls.get(), 0);
    }

    #[test]
    fn malformed_signature_width_and_corruption_fail() {
        let short = [0_u8; SHORT_FIXED_WIDTH];
        assert!(matches!(
            SignerSignature::try_from_slice(&short),
            Err(super::super::SignerError::InvalidSignatureLength { .. })
        ));

        let fixture = DirectFixture::new();
        let valid =
            sign_request(&fixture.token, &fixture.audience, RESOURCE, ABILITY, operation(), nonce(), &fixture.holder);
        let mut corrupted = [0_u8; super::super::ED25519_SIGNATURE_BYTES];
        corrupted.copy_from_slice(valid.as_bytes());
        corrupted[0] ^= SIGNATURE_CORRUPTION_BYTE;
        let corrupted = SignerSignature::from_bytes(corrupted);
        let proofs = ProofCollection::empty();
        let revocations = NoRevocations;
        let profiles = KnownProfile::new(profile());
        let replay = RecordingReplay::default();
        let verifier = TestVerifier {
            time: VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
            keys: &fixture.keys,
            proofs: &proofs,
            revocations: &revocations,
            profiles: &profiles,
            replay: &replay,
            caveats: &DenyUnknownCaveats,
            token_limits: VerificationLimits::default(),
            holder_limits: HolderInvocationLimits::default(),
        };
        let error = verifier
            .verify(&HolderInvocationRequest::new(
                fixture.token.as_str(),
                RESOURCE,
                ABILITY,
                operation(),
                nonce(),
                &corrupted,
            ))
            .expect_err("corrupted signature must fail");

        assert!(matches!(error, HolderInvocationError::InvalidHolderSignature));
        assert_eq!(replay.calls.get(), 0);
    }

    #[test]
    fn oversized_request_fails_before_profile_or_replay_admission() {
        let fixture = DirectFixture::new();
        let signature =
            sign_request(&fixture.token, &fixture.audience, RESOURCE, ABILITY, operation(), nonce(), &fixture.holder);
        let proofs = ProofCollection::empty();
        let revocations = NoRevocations;
        let profiles = KnownProfile::new(profile());
        let replay = RecordingReplay::default();
        let holder_limits = HolderInvocationLimits::try_new(
            HolderInvocationLimits::default().max_audience_bytes(),
            SHORT_RESOURCE_LIMIT,
            HolderInvocationLimits::default().max_ability_bytes(),
        )
        .expect("test limits must fit canonical widths");
        let verifier = TestVerifier {
            time: VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
            keys: &fixture.keys,
            proofs: &proofs,
            revocations: &revocations,
            profiles: &profiles,
            replay: &replay,
            caveats: &DenyUnknownCaveats,
            token_limits: VerificationLimits::default(),
            holder_limits,
        };
        let error = verifier
            .verify(&HolderInvocationRequest::new(
                fixture.token.as_str(),
                RESOURCE,
                ABILITY,
                operation(),
                nonce(),
                &signature,
            ))
            .expect_err("oversized resource must fail");

        assert!(matches!(error, HolderInvocationError::Canonical { .. }));
        assert_eq!(profiles.calls.get(), 0);
        assert_eq!(replay.calls.get(), 0);
    }

    #[test]
    fn invalid_time_missing_proof_and_revocation_fail_before_replay() {
        let direct = DirectFixture::new();
        let direct_signature =
            sign_request(&direct.token, &direct.audience, RESOURCE, ABILITY, operation(), nonce(), &direct.holder);
        let empty_proofs = ProofCollection::empty();
        let no_revocations = NoRevocations;
        let profiles = KnownProfile::new(profile());
        let replay = RecordingReplay::default();
        let expired_verifier = TestVerifier {
            time: VerificationTime::from_unix_seconds(EXPIRED_VERIFY_AT_SECONDS),
            keys: &direct.keys,
            proofs: &empty_proofs,
            revocations: &no_revocations,
            profiles: &profiles,
            replay: &replay,
            caveats: &DenyUnknownCaveats,
            token_limits: VerificationLimits::default(),
            holder_limits: HolderInvocationLimits::default(),
        };
        let direct_request = HolderInvocationRequest::new(
            direct.token.as_str(),
            RESOURCE,
            ABILITY,
            operation(),
            nonce(),
            &direct_signature,
        );
        assert!(matches!(
            expired_verifier.verify(&direct_request),
            Err(HolderInvocationError::Token {
                source: super::super::TokenError::Temporal { .. }
            })
        ));

        let revoked = RevocationList::from_references(vec![proof_reference(&direct.token)]);
        let revoked_verifier = TestVerifier {
            time: VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
            keys: &direct.keys,
            proofs: &empty_proofs,
            revocations: &revoked,
            profiles: &profiles,
            replay: &replay,
            caveats: &DenyUnknownCaveats,
            token_limits: VerificationLimits::default(),
            holder_limits: HolderInvocationLimits::default(),
        };
        assert!(matches!(
            revoked_verifier.verify(&direct_request),
            Err(HolderInvocationError::Token {
                source: super::super::TokenError::Revoked { .. }
            })
        ));

        let delegated = DelegatedFixture::new();
        let delegated_signature = sign_request(
            &delegated.token,
            &delegated.audience,
            RESOURCE,
            ABILITY,
            operation(),
            nonce(),
            &delegated.holder,
        );
        let missing_proof_verifier = TestVerifier {
            time: VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
            keys: &delegated.keys,
            proofs: &empty_proofs,
            revocations: &no_revocations,
            profiles: &profiles,
            replay: &replay,
            caveats: &DenyUnknownCaveats,
            token_limits: VerificationLimits::default(),
            holder_limits: HolderInvocationLimits::default(),
        };
        assert!(matches!(
            missing_proof_verifier.verify(&HolderInvocationRequest::new(
                delegated.token.as_str(),
                RESOURCE,
                ABILITY,
                operation(),
                nonce(),
                &delegated_signature,
            )),
            Err(HolderInvocationError::Token {
                source: super::super::TokenError::ProofStore { .. }
            })
        ));
        assert_eq!(replay.calls.get(), 0);
    }

    #[test]
    fn caveat_denial_and_unknown_profile_fail_before_replay() {
        let caveated = DirectFixture::with_capability_and_expiration(caveated_capability(), TOKEN_EXPIRES_SECONDS);
        let signature = sign_request(
            &caveated.token,
            &caveated.audience,
            RESOURCE,
            ABILITY,
            operation(),
            nonce(),
            &caveated.holder,
        );
        let proofs = ProofCollection::empty();
        let revocations = NoRevocations;
        let profiles = KnownProfile::new(profile());
        let replay = RecordingReplay::default();
        let verifier = TestVerifier {
            time: VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
            keys: &caveated.keys,
            proofs: &proofs,
            revocations: &revocations,
            profiles: &profiles,
            replay: &replay,
            caveats: &DenyUnknownCaveats,
            token_limits: VerificationLimits::default(),
            holder_limits: HolderInvocationLimits::default(),
        };
        assert!(matches!(
            verifier.verify(&HolderInvocationRequest::new(
                caveated.token.as_str(),
                RESOURCE,
                ABILITY,
                operation(),
                nonce(),
                &signature,
            )),
            Err(HolderInvocationError::Authorization { .. })
        ));
        assert_eq!(replay.calls.get(), 0);

        let direct = DirectFixture::new();
        let unknown_operation = alternate_operation();
        let unknown_signature = sign_request(
            &direct.token,
            &direct.audience,
            RESOURCE,
            ABILITY,
            unknown_operation,
            nonce(),
            &direct.holder,
        );
        let unknown_verifier = TestVerifier {
            time: VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
            keys: &direct.keys,
            proofs: &proofs,
            revocations: &revocations,
            profiles: &profiles,
            replay: &replay,
            caveats: &DenyUnknownCaveats,
            token_limits: VerificationLimits::default(),
            holder_limits: HolderInvocationLimits::default(),
        };
        assert!(matches!(
            unknown_verifier.verify(&HolderInvocationRequest::new(
                direct.token.as_str(),
                RESOURCE,
                ABILITY,
                unknown_operation,
                nonce(),
                &unknown_signature,
            )),
            Err(HolderInvocationError::OperationProfile {
                source: OperationProfileAdmissionError::UnknownProfile
            })
        ));
        assert_eq!(replay.calls.get(), 0);
    }

    #[test]
    fn reviewed_hard_limits_and_fallible_allocation_fail_closed() {
        assert!(matches!(
            HolderInvocationLimits::try_new(
                HARD_MAX_AUDIENCE_BYTES + 1,
                DEFAULT_MAX_RESOURCE_BYTES,
                DEFAULT_MAX_ABILITY_BYTES,
            ),
            Err(HolderInvocationLimitError::ConfiguredFieldLimitTooLarge {
                field: ucan_core::holder_invocation::HolderInvocationField::Audience,
                ..
            })
        ));
        assert!(matches!(
            HolderInvocationLimits::try_new(
                DEFAULT_MAX_AUDIENCE_BYTES,
                HARD_MAX_RESOURCE_BYTES + 1,
                DEFAULT_MAX_ABILITY_BYTES,
            ),
            Err(HolderInvocationLimitError::ConfiguredFieldLimitTooLarge {
                field: ucan_core::holder_invocation::HolderInvocationField::Resource,
                ..
            })
        ));
        assert!(matches!(
            HolderInvocationLimits::try_new(
                DEFAULT_MAX_AUDIENCE_BYTES,
                DEFAULT_MAX_RESOURCE_BYTES,
                HARD_MAX_ABILITY_BYTES + 1,
            ),
            Err(HolderInvocationLimitError::ConfiguredFieldLimitTooLarge {
                field: ucan_core::holder_invocation::HolderInvocationField::Ability,
                ..
            })
        ));

        let fixture = DirectFixture::new();
        let token_reference = token_reference(fixture.token.as_str());
        let maximum_limits =
            HolderInvocationLimits::try_new(HARD_MAX_AUDIENCE_BYTES, HARD_MAX_RESOURCE_BYTES, HARD_MAX_ABILITY_BYTES)
                .expect("reviewed hard maxima must be valid configured limits");
        let maximum_resource = "r".repeat(HARD_MAX_RESOURCE_BYTES);
        let maximum_ability = "a".repeat(HARD_MAX_ABILITY_BYTES);
        assert!(matches!(
            canonical_request(
                &token_reference,
                &fixture.audience,
                maximum_resource.as_str(),
                maximum_ability.as_str(),
                operation(),
                nonce(),
                maximum_limits,
            ),
            Err(HolderInvocationError::Limit {
                source: HolderInvocationLimitError::CanonicalRequestTooLarge {
                    hard_maximum: HARD_MAX_CANONICAL_BYTES,
                    ..
                }
            })
        ));

        assert!(matches!(
            allocate_exact_canonical_with(SIMULATED_ALLOCATION_BYTES, |_buffer, _requested| false),
            Err(HolderInvocationError::Limit {
                source: HolderInvocationLimitError::AllocationFailed {
                    requested_bytes: SIMULATED_ALLOCATION_BYTES
                }
            })
        ));
    }

    #[test]
    fn bad_signature_calls_no_profile_caveat_or_replay_boundary() {
        let fixture = DirectFixture::with_capability_and_expiration(caveated_capability(), TOKEN_EXPIRES_SECONDS);
        let signature =
            sign_request(&fixture.token, &fixture.audience, RESOURCE, ABILITY, operation(), nonce(), &fixture.holder);
        let mut corrupted_bytes = [0_u8; super::super::ED25519_SIGNATURE_BYTES];
        corrupted_bytes.copy_from_slice(signature.as_bytes());
        corrupted_bytes[0] ^= SIGNATURE_CORRUPTION_BYTE;
        let corrupted_signature = SignerSignature::from_bytes(corrupted_bytes);
        let proofs = ProofCollection::empty();
        let revocations = NoRevocations;
        let events = RefCell::new(Vec::new());
        let profile = OrderedProfile {
            events: &events,
            admit: true,
        };
        let caveats = OrderedCaveatSet {
            policy: OrderedCaveatPolicy {
                events: &events,
                satisfy: true,
            },
        };
        let replay = OrderedReplay {
            events: &events,
            admit: true,
            calls: Cell::new(0),
        };
        let verifier = TestVerifier {
            time: VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
            keys: &fixture.keys,
            proofs: &proofs,
            revocations: &revocations,
            profiles: &profile,
            replay: &replay,
            caveats: &caveats,
            token_limits: VerificationLimits::default(),
            holder_limits: HolderInvocationLimits::default(),
        };
        let request = HolderInvocationRequest::new(
            fixture.token.as_str(),
            RESOURCE,
            ABILITY,
            operation(),
            nonce(),
            &corrupted_signature,
        );

        assert!(matches!(verifier.verify(&request), Err(HolderInvocationError::InvalidHolderSignature)));
        assert!(events.borrow().is_empty());
        assert_eq!(replay.calls.get(), 0);
    }

    #[test]
    fn profile_denial_calls_no_caveat_or_replay_boundary() {
        let fixture = DirectFixture::with_capability_and_expiration(caveated_capability(), TOKEN_EXPIRES_SECONDS);
        let signature =
            sign_request(&fixture.token, &fixture.audience, RESOURCE, ABILITY, operation(), nonce(), &fixture.holder);
        let proofs = ProofCollection::empty();
        let revocations = NoRevocations;
        let events = RefCell::new(Vec::new());
        let profile = OrderedProfile {
            events: &events,
            admit: false,
        };
        let caveats = OrderedCaveatSet {
            policy: OrderedCaveatPolicy {
                events: &events,
                satisfy: true,
            },
        };
        let replay = OrderedReplay {
            events: &events,
            admit: true,
            calls: Cell::new(0),
        };
        let verifier = TestVerifier {
            time: VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
            keys: &fixture.keys,
            proofs: &proofs,
            revocations: &revocations,
            profiles: &profile,
            replay: &replay,
            caveats: &caveats,
            token_limits: VerificationLimits::default(),
            holder_limits: HolderInvocationLimits::default(),
        };
        let request =
            HolderInvocationRequest::new(fixture.token.as_str(), RESOURCE, ABILITY, operation(), nonce(), &signature);

        assert!(matches!(
            verifier.verify(&request),
            Err(HolderInvocationError::OperationProfile {
                source: OperationProfileAdmissionError::UnknownProfile
            })
        ));
        assert_eq!(events.borrow().as_slice(), ["profile"]);
        assert_eq!(replay.calls.get(), 0);
    }

    #[test]
    fn caveat_denial_calls_no_replay_boundary() {
        let fixture = DirectFixture::with_capability_and_expiration(caveated_capability(), TOKEN_EXPIRES_SECONDS);
        let signature =
            sign_request(&fixture.token, &fixture.audience, RESOURCE, ABILITY, operation(), nonce(), &fixture.holder);
        let proofs = ProofCollection::empty();
        let revocations = NoRevocations;
        let events = RefCell::new(Vec::new());
        let profile = OrderedProfile {
            events: &events,
            admit: true,
        };
        let caveats = OrderedCaveatSet {
            policy: OrderedCaveatPolicy {
                events: &events,
                satisfy: false,
            },
        };
        let replay = OrderedReplay {
            events: &events,
            admit: true,
            calls: Cell::new(0),
        };
        let verifier = TestVerifier {
            time: VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
            keys: &fixture.keys,
            proofs: &proofs,
            revocations: &revocations,
            profiles: &profile,
            replay: &replay,
            caveats: &caveats,
            token_limits: VerificationLimits::default(),
            holder_limits: HolderInvocationLimits::default(),
        };
        let request =
            HolderInvocationRequest::new(fixture.token.as_str(), RESOURCE, ABILITY, operation(), nonce(), &signature);

        assert!(matches!(verifier.verify(&request), Err(HolderInvocationError::Authorization { .. })));
        assert_eq!(events.borrow().as_slice(), ["profile", "caveat"]);
        assert_eq!(replay.calls.get(), 0);
    }

    #[test]
    fn replay_denial_occurs_once_after_all_observations() {
        let fixture = DirectFixture::with_capability_and_expiration(caveated_capability(), TOKEN_EXPIRES_SECONDS);
        let signature =
            sign_request(&fixture.token, &fixture.audience, RESOURCE, ABILITY, operation(), nonce(), &fixture.holder);
        let proofs = ProofCollection::empty();
        let revocations = NoRevocations;
        let events = RefCell::new(Vec::new());
        let profile = OrderedProfile {
            events: &events,
            admit: true,
        };
        let caveats = OrderedCaveatSet {
            policy: OrderedCaveatPolicy {
                events: &events,
                satisfy: true,
            },
        };
        let replay = OrderedReplay {
            events: &events,
            admit: false,
            calls: Cell::new(0),
        };
        let verifier = TestVerifier {
            time: VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
            keys: &fixture.keys,
            proofs: &proofs,
            revocations: &revocations,
            profiles: &profile,
            replay: &replay,
            caveats: &caveats,
            token_limits: VerificationLimits::default(),
            holder_limits: HolderInvocationLimits::default(),
        };
        let request =
            HolderInvocationRequest::new(fixture.token.as_str(), RESOURCE, ABILITY, operation(), nonce(), &signature);

        assert!(matches!(
            verifier.verify(&request),
            Err(HolderInvocationError::Replay {
                source: HolderInvocationReplayError::Denied
            })
        ));
        assert_eq!(events.borrow().as_slice(), ["profile", "caveat", "replay"]);
        assert_eq!(replay.calls.get(), 1);
    }

    #[test]
    fn live_success_surface_has_no_clone_serde_or_public_construction() {
        let source = include_str!("holder_invocation.rs");
        let production = source.split("#[cfg(test)]\nmod tests").next().expect("production source prefix must exist");
        let clone_impl = format!("{}{}", "impl Clone for ", "HolderInvocationAuthorized");
        let serialize_impl = format!("{}{}", "impl Serialize for ", "HolderInvocationAuthorized");
        let deserialize_impl = format!("{}{}", "Deserialize<'de> for ", "HolderInvocationAuthorized");
        let struct_start =
            production.find("pub struct HolderInvocationAuthorized").expect("live success struct must exist");
        let impl_start = production[struct_start..]
            .find("impl fmt::Debug for HolderInvocationAuthorized")
            .map(|offset| struct_start + offset)
            .expect("debug implementation must follow live success struct");
        let struct_block = &production[struct_start..impl_start];

        assert!(!production.contains(clone_impl.as_str()));
        assert!(!production.contains(serialize_impl.as_str()));
        assert!(!production.contains(deserialize_impl.as_str()));
        assert!(!struct_block.lines().skip(1).any(|line| line.trim_start().starts_with("pub ")));

        let verify_start =
            production.find("pub fn verify_holder_signed_invocation").expect("holder verifier must exist");
        let verify_end = production[verify_start..]
            .find("\n}\n\nfn authorize_request")
            .map(|offset| verify_start + offset)
            .expect("holder verifier end must exist");
        let verify_body = &production[verify_start..verify_end];
        let success_build = verify_body
            .find("let authorized = HolderInvocationAuthorized")
            .expect("live success must be prepared");
        let replay_commit = verify_body.find(".admit_holder_invocation").expect("replay commit must exist");
        assert!(success_build < replay_commit);
        let after_replay = &verify_body[replay_commit..];
        assert!(!after_replay.contains(".clone()"));
        assert!(!after_replay.contains(".to_owned()"));
        assert!(!after_replay.contains("canonical_request("));
        assert!(!after_replay.contains("resolve_key("));
        assert!(!after_replay.contains("admit_operation_profile("));
        assert!(after_replay.contains("Ok(authorized)"));
    }
}
