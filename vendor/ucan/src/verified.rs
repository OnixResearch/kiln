use core::fmt;

use ed25519_dalek::Signer;
use ed25519_dalek::Verifier;
use ed25519_dalek::VerifyingKey;
use verified_logic::scope;
use verified_logic::token_timestamp;
use verified_logic::ucan_ability;
use verified_logic::ucan_backend_transcript;
use verified_logic::ucan_capability_merge;
use verified_logic::ucan_caveat;
use verified_logic::ucan_caveat_evaluation;
use verified_logic::ucan_caveat_preservation;
use verified_logic::ucan_chain;
use verified_logic::ucan_claims_presence;
use verified_logic::ucan_delegation_depth;
use verified_logic::ucan_did;
use verified_logic::ucan_header;
use verified_logic::ucan_invocation;
use verified_logic::ucan_parsed_capability_shape;
use verified_logic::ucan_parsed_caveat_payload;
use verified_logic::ucan_parsed_caveat_shape;
use verified_logic::ucan_parsed_did_claim;
use verified_logic::ucan_proof_reference;
use verified_logic::ucan_proof_reference_collection;
use verified_logic::ucan_proof_reference_list_shape;
use verified_logic::ucan_proof_store_result;
use verified_logic::ucan_proof_traversal;
use verified_logic::ucan_replay_admission;
use verified_logic::ucan_revocation;
use verified_logic::ucan_revocation_check;
use verified_logic::ucan_signature;
use verified_logic::ucan_temporal;
use verified_logic::ucan_temporal_order;
use verified_logic::ucan_token;
use verified_logic::ucan_verification_stage;
use verified_logic::web_of_trust_attestation;
use verified_logic::web_of_trust_policy;
use verified_logic::web_of_trust_principal;
use verified_logic::web_of_trust_revocation;
use verified_logic::web_of_trust_ucan_authorizer;
use zeroize::Zeroizing;

use crate::shell::CapabilityDocument;

const ABILITY_SEGMENT_SEPARATOR: char = '/';
pub const DEFAULT_CLOCK_SKEW_SECONDS: u64 = 60;

pub type VerifiedResult<T> = std::result::Result<T, VerifiedPrimitiveError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifiedPrimitiveError {
    InvalidCapability { source: ucan_core::CapabilityError },
    EmptyTimeWindowChain,
    InvalidSecretKeyLength { actual: usize },
    InvalidPublicKeyLength { actual: usize },
    InvalidPublicKeyEncoding,
    InvalidSignatureLength { actual: usize },
    InvalidPrincipalDigestLength { actual: usize },
    TokenValidation { failure: TokenValidationFailure },
}

impl fmt::Display for VerifiedPrimitiveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCapability { source } => write!(formatter, "invalid capability: {source}"),
            Self::EmptyTimeWindowChain => formatter.write_str("time window chain must not be empty"),
            Self::InvalidSecretKeyLength { actual } => {
                write!(formatter, "ed25519 secret keys must be {} bytes, got {actual}", ucan_signature::SECRET_KEY_LEN)
            }
            Self::InvalidPublicKeyLength { actual } => {
                write!(formatter, "ed25519 public keys must be {} bytes, got {actual}", ucan_signature::PUBLIC_KEY_LEN)
            }
            Self::InvalidPublicKeyEncoding => formatter.write_str("ed25519 public key encoding is invalid"),
            Self::InvalidSignatureLength { actual } => {
                write!(formatter, "ed25519 signatures must be {} bytes, got {actual}", ucan_signature::SIGNATURE_LEN)
            }
            Self::InvalidPrincipalDigestLength { actual } => {
                write!(
                    formatter,
                    "web-of-trust principal digests must be {} bytes, got {actual}",
                    web_of_trust_principal::PRINCIPAL_ID_LEN
                )
            }
            Self::TokenValidation { failure } => write!(formatter, "token validation failed: {failure}"),
        }
    }
}

impl std::error::Error for VerifiedPrimitiveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidCapability { source } => Some(source),
            Self::EmptyTimeWindowChain
            | Self::InvalidSecretKeyLength { .. }
            | Self::InvalidPublicKeyLength { .. }
            | Self::InvalidPublicKeyEncoding
            | Self::InvalidSignatureLength { .. }
            | Self::InvalidPrincipalDigestLength { .. }
            | Self::TokenValidation { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenValidationFailure {
    ExpiredToken,
    NotYetValid,
    SelfDelegation,
    MissingIssuer,
    MissingAudience,
    MissingAttenuations,
    MissingProofs,
    MissingExpiration,
    MissingNotBefore,
    InvalidTemporalOrder,
    InvalidDidClaims,
    InvalidProofReferenceListShape,
    InvalidProofReferenceCollection,
    InvalidCapabilityListShape,
    InvalidCaveatListShape,
    InvalidCaveatPayloadAdmission,
    InvalidProofStoreResultTranscript,
    InvalidRevocationCheckTranscript,
    InvalidReplayAdmissionTranscript,
    InvalidSigningInputProvenance,
    InvalidVerificationStageFlow,
    InvalidSignature,
}

impl fmt::Display for TokenValidationFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExpiredToken => formatter.write_str("token expired"),
            Self::NotYetValid => formatter.write_str("token not yet valid"),
            Self::SelfDelegation => formatter.write_str("issuer and audience must differ"),
            Self::MissingIssuer => formatter.write_str("issuer must be present"),
            Self::MissingAudience => formatter.write_str("audience must be present"),
            Self::MissingAttenuations => formatter.write_str("at least one capability must be present"),
            Self::MissingProofs => formatter.write_str("proof list must be present"),
            Self::MissingExpiration => formatter.write_str("expiration must be present"),
            Self::MissingNotBefore => formatter.write_str("not-before time must be present"),
            Self::InvalidTemporalOrder => formatter.write_str("not-before time must precede expiration"),
            Self::InvalidDidClaims => formatter.write_str("parsed issuer and audience DID claims are invalid"),
            Self::InvalidProofReferenceListShape => formatter.write_str("parsed proof-reference list shape is invalid"),
            Self::InvalidProofReferenceCollection => formatter.write_str("proof-reference collection is invalid"),
            Self::InvalidCapabilityListShape => formatter.write_str("parsed capability list shape is invalid"),
            Self::InvalidCaveatListShape => formatter.write_str("parsed caveat list shape is invalid"),
            Self::InvalidCaveatPayloadAdmission => formatter.write_str("parsed caveat payload admission is invalid"),
            Self::InvalidProofStoreResultTranscript => formatter.write_str("proof-store result transcript is invalid"),
            Self::InvalidRevocationCheckTranscript => formatter.write_str("revocation-check transcript is invalid"),
            Self::InvalidReplayAdmissionTranscript => formatter.write_str("replay-admission transcript is invalid"),
            Self::InvalidSigningInputProvenance => formatter.write_str("signing input byte provenance is invalid"),
            Self::InvalidVerificationStageFlow => formatter.write_str("verification stage flow is invalid"),
            Self::InvalidSignature => formatter.write_str("signature must verify"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenTimeWindow {
    pub not_before_seconds: u64,
    pub expires_at_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenEnvelopeInput {
    pub issuer_did: String,
    pub audience_did: String,
    pub capabilities: Vec<CapabilityDocument>,
    pub proofs: Vec<Vec<u8>>,
    pub expires_at_seconds: u64,
    pub not_before_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedProofReferenceDigest {
    pub bytes: Vec<u8>,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedParsedClaimsPresence {
    pub issuer_present: bool,
    pub audience_present: bool,
    pub attenuations_present: bool,
    pub proofs_present: bool,
    pub expiration_present: bool,
    pub not_before_present: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedParsedTemporalOrder {
    pub not_before_seconds: u64,
    pub expires_at_seconds: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedParsedDidClaim {
    pub present: bool,
    pub non_empty: bool,
    pub trimmed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedParsedDidClaims {
    pub issuer: VerifiedParsedDidClaim,
    pub audience: VerifiedParsedDidClaim,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedParsedProofReferenceEntryShape {
    pub encoded_non_empty: bool,
    pub encoded_base64url_no_pad: bool,
    pub decoded_non_empty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedParsedProofReferenceListShape {
    pub entries: Vec<VerifiedParsedProofReferenceEntryShape>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedProofReferenceCollection {
    pub count: usize,
    pub max_count: usize,
    pub all_references_non_empty: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedParsedCapabilityShape {
    pub resource_present: bool,
    pub ability_present: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedParsedCapabilityListShape {
    pub capabilities: Vec<VerifiedParsedCapabilityShape>,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedParsedCaveatShape {
    pub domain_present: bool,
    pub caveat_type_present: bool,
    pub key_present: bool,
    pub payload_present: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedParsedCaveatListShape {
    pub caveats: Vec<VerifiedParsedCaveatShape>,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedParsedCaveatPayloadFact {
    pub payload_present: bool,
    pub encoded_non_empty: bool,
    pub encoded_base64url_no_pad: bool,
    pub decoded_non_empty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedParsedCaveatPayloadList {
    pub payloads: Vec<VerifiedParsedCaveatPayloadFact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedProofStoreResultFact {
    pub reference_non_empty: bool,
    pub backend_succeeded: bool,
    pub compact_token_present: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedRevocationCheckFact {
    pub reference_non_empty: bool,
    pub backend_succeeded: bool,
    pub not_revoked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifiedReplayAdmissionOutcome {
    Allowed,
    Duplicate,
    Stale,
    Malformed,
    Denied,
    Backend,
    Accepted,
    Unknown { outcome: u8 },
}

impl VerifiedReplayAdmissionOutcome {
    const fn to_verified_logic(self) -> u8 {
        match self {
            Self::Allowed => ucan_replay_admission::REPLAY_ALLOWED,
            Self::Duplicate => ucan_replay_admission::REPLAY_DUPLICATE,
            Self::Stale => ucan_replay_admission::REPLAY_STALE,
            Self::Malformed => ucan_replay_admission::REPLAY_MALFORMED,
            Self::Denied => ucan_replay_admission::REPLAY_DENIED,
            Self::Backend => ucan_replay_admission::REPLAY_BACKEND,
            Self::Accepted => ucan_replay_admission::REPLAY_ACCEPTED,
            Self::Unknown { outcome } => outcome,
        }
    }

    const fn from_verified_logic(outcome: u8) -> Self {
        match outcome {
            ucan_replay_admission::REPLAY_ALLOWED => Self::Allowed,
            ucan_replay_admission::REPLAY_DUPLICATE => Self::Duplicate,
            ucan_replay_admission::REPLAY_STALE => Self::Stale,
            ucan_replay_admission::REPLAY_MALFORMED => Self::Malformed,
            ucan_replay_admission::REPLAY_DENIED => Self::Denied,
            ucan_replay_admission::REPLAY_BACKEND => Self::Backend,
            ucan_replay_admission::REPLAY_ACCEPTED => Self::Accepted,
            outcome => Self::Unknown { outcome },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedReplayAdmissionFact {
    pub outcome: VerifiedReplayAdmissionOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedReplayAdmissionTranscript {
    pub facts: Vec<VerifiedReplayAdmissionFact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedReplayAdmissionTranscriptOutcome {
    pub accepted: bool,
    pub first_failed_outcome: VerifiedReplayAdmissionOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedCaveatPreservation {
    pub parent_caveat_ids: Vec<Vec<u8>>,
    pub child_caveat_ids: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedCaveatEvaluationFact {
    pub policy_present: bool,
    pub satisfied: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedCaveatEvaluationTranscript {
    pub facts: Vec<VerifiedCaveatEvaluationFact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifiedVerificationStage {
    Parse,
    Structural,
    KeyResolution,
    Signature,
    Envelope,
    Revocation,
    ProofLoad,
    ProofChain,
    Authorization,
    Accepted,
}

impl VerifiedVerificationStage {
    const fn from_verified_logic(stage: u8) -> Self {
        match stage {
            ucan_verification_stage::STAGE_PARSE => Self::Parse,
            ucan_verification_stage::STAGE_STRUCTURAL => Self::Structural,
            ucan_verification_stage::STAGE_KEY_RESOLUTION => Self::KeyResolution,
            ucan_verification_stage::STAGE_SIGNATURE => Self::Signature,
            ucan_verification_stage::STAGE_ENVELOPE => Self::Envelope,
            ucan_verification_stage::STAGE_REVOCATION => Self::Revocation,
            ucan_verification_stage::STAGE_PROOF_LOAD => Self::ProofLoad,
            ucan_verification_stage::STAGE_PROOF_CHAIN => Self::ProofChain,
            ucan_verification_stage::STAGE_AUTHORIZATION => Self::Authorization,
            _ => Self::Accepted,
        }
    }
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedVerificationStageTranscript {
    pub parse_ok: bool,
    pub structural_ok: bool,
    pub key_resolved: bool,
    pub signature_valid: bool,
    pub envelope_valid: bool,
    pub revocation_clear: bool,
    pub proofs_loaded: bool,
    pub proof_chain_valid: bool,
    pub authorized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedVerificationStageOutcome {
    pub accepted: bool,
    pub first_failed_stage: VerifiedVerificationStage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedChainEntry {
    pub issuer_did: String,
    pub audience_did: String,
    pub capabilities: Vec<CapabilityDocument>,
    pub expires_at_seconds: u64,
    pub not_before_seconds: u64,
    pub signature_valid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedInvocationChainRequest {
    pub authorization_reference: Vec<u8>,
    pub invoker_did: String,
    pub resource: String,
    pub ability: String,
    pub leaf_capability_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedProofTraversalEntry {
    pub proof_reference: Vec<u8>,
    pub issuer_did: String,
    pub audience_did: String,
    pub capabilities: Vec<CapabilityDocument>,
    pub caveat_ids: Vec<Vec<u8>>,
    pub expires_at_seconds: u64,
    pub not_before_seconds: u64,
    pub signature_valid: bool,
    pub revoked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedProofTraversalBudget {
    pub max_depth: u64,
    pub max_proofs: u64,
    pub max_input_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedProofTraversalTarget {
    pub resource: String,
    pub ability: String,
    pub now_seconds: u64,
    pub tolerance_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedProofTraversalTranscript {
    pub entries: Vec<VerifiedProofTraversalEntry>,
    pub visited_references: Vec<Vec<u8>>,
    pub total_input_bytes: u64,
    pub target: VerifiedProofTraversalTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedProofTraversalDagTranscript {
    pub entries: Vec<VerifiedProofTraversalEntry>,
    pub parent_indices: Vec<Vec<usize>>,
    pub visited_references: Vec<Vec<u8>>,
    pub total_input_bytes: u64,
    pub target: VerifiedProofTraversalTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedDidKey {
    pub public_key_bytes: Vec<u8>,
    pub codec: u64,
    pub encoded: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifiedWebOfTrustAuthorization {
    Authorized,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifiedWebOfTrustCaveat {
    TimeBound { not_after_seconds: u64 },
    RateLimit { max_count: u64, window_seconds: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedWebOfTrustCaveatContext {
    pub now_seconds: u64,
    pub count_in_window: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedWebOfTrustAttestation {
    pub issuer_key_bytes: Vec<u8>,
    pub issuer_digest: Vec<u8>,
    pub subject_key_bytes: Vec<u8>,
    pub subject_digest: Vec<u8>,
    pub role_tag: u8,
    pub not_before_seconds: u64,
    pub not_after_seconds: u64,
    pub sequence_no: u64,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedWebOfTrustRevocation {
    pub issuer_key_bytes: Vec<u8>,
    pub issuer_digest: Vec<u8>,
    pub target_kind_tag: u8,
    pub exact_payload_target: Vec<u8>,
    pub family_subject_key_bytes: Vec<u8>,
    pub family_subject_digest: Vec<u8>,
    pub family_role_tag: u8,
    pub effective_time_seconds: u64,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct VerifiedWebOfTrustUcanAuthorizationRequest {
    pub token_envelope: TokenEnvelopeInput,
    pub signature_valid: bool,
    pub now_seconds: u64,
    pub tolerance_seconds: u64,
    pub ability_allowed: bool,
    pub caveats_contained: bool,
    pub active_caveats: Vec<VerifiedWebOfTrustCaveat>,
    pub caveat_context: VerifiedWebOfTrustCaveatContext,
    pub did_cid_linked: bool,
    pub root_principal_digests: Vec<Vec<u8>>,
    pub attestations: Vec<VerifiedWebOfTrustAttestation>,
    pub revocations: Vec<VerifiedWebOfTrustRevocation>,
    pub subject_principal_digest: Vec<u8>,
    pub threshold: usize,
    pub depth: usize,
    pub trust_time_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedCapability {
    document: CapabilityDocument,
    ability_segments: Vec<Vec<u8>>,
}

impl VerifiedCapability {
    /// Build a verified-logic-backed capability wrapper.
    ///
    /// # Errors
    ///
    /// Returns [`VerifiedPrimitiveError::InvalidCapability`] when the resource
    /// or ability violates the core capability rules.
    pub fn new(resource: String, ability: String) -> VerifiedResult<Self> {
        let document = CapabilityDocument::new(resource, ability).map_err(|error| match error {
            crate::Error::Capability { source } => VerifiedPrimitiveError::InvalidCapability { source },
            crate::Error::Io { .. } => unreachable!("CapabilityDocument::new performs validation only"),
            crate::Error::Caveat { .. } | crate::Error::TokenClaims { .. } => {
                unreachable!("CapabilityDocument::new does not include caveats")
            }
        })?;
        build_verified_capability(document)
    }

    /// Convert an owned capability document into a verified-logic wrapper.
    ///
    /// # Errors
    ///
    /// Returns [`VerifiedPrimitiveError::InvalidCapability`] when the document
    /// fails core validation.
    pub fn from_document(document: CapabilityDocument) -> VerifiedResult<Self> {
        validate_capability_document(&document)?;
        build_verified_capability(document)
    }

    #[must_use]
    pub const fn document(&self) -> &CapabilityDocument {
        &self.document
    }

    #[must_use]
    pub const fn resource(&self) -> &str {
        self.document.resource.as_str()
    }

    #[must_use]
    pub const fn ability(&self) -> &str {
        self.document.ability.as_str()
    }

    #[must_use]
    pub fn contains(&self, child: &Self) -> bool {
        let resource_covers = scope::covers(self.resource().as_bytes(), child.resource().as_bytes());
        let ability_subsumes = ucan_ability::ability_subsumes(&self.verified_ability(), &child.verified_ability());
        resource_covers && ability_subsumes
    }

    /// Return whether the verified ability contains a wildcard segment.
    #[must_use]
    pub fn has_wildcard_segment(&self) -> bool {
        ucan_ability::ability_has_glob_segment(&self.verified_ability())
    }

    /// Return whether this capability can represent a concrete operation request.
    #[must_use]
    pub fn is_concrete(&self) -> bool {
        ucan_ability::ability_is_concrete(&self.verified_ability())
    }

    /// Check whether this capability authorizes a concrete resource/ability.
    ///
    /// # Errors
    ///
    /// Returns [`VerifiedPrimitiveError::InvalidCapability`] when the
    /// requested resource or ability is not a valid capability input.
    pub fn authorizes(&self, resource: &str, ability: &str) -> VerifiedResult<bool> {
        let requested = Self::new(resource.to_owned(), ability.to_owned())?;
        Ok(self.contains(&requested))
    }

    fn verified_ability(&self) -> ucan_ability::Ability {
        ucan_ability::Ability {
            segments: self.ability_segments.clone(),
        }
    }
}

#[must_use]
pub fn token_time_valid_at(
    issued_at_seconds: u64,
    expires_at_seconds: u64,
    now_seconds: u64,
    tolerance_seconds: u64,
) -> bool {
    token_timestamp::token_time_valid(issued_at_seconds, expires_at_seconds, now_seconds, tolerance_seconds)
}

#[must_use]
pub fn token_time_valid(issued_at_seconds: u64, expires_at_seconds: u64, now_seconds: u64) -> bool {
    token_time_valid_at(issued_at_seconds, expires_at_seconds, now_seconds, DEFAULT_CLOCK_SKEW_SECONDS)
}

#[must_use]
pub fn token_needs_refresh(issued_at_seconds: u64, expires_at_seconds: u64, now_seconds: u64) -> bool {
    token_timestamp::needs_refresh(issued_at_seconds, expires_at_seconds, now_seconds)
}

/// Intersect a UCAN delegation time-window chain.
///
/// # Errors
///
/// Returns [`VerifiedPrimitiveError::EmptyTimeWindowChain`] when no windows are
/// provided.
pub fn compose_time_window_chain(windows: &[TokenTimeWindow]) -> VerifiedResult<TokenTimeWindow> {
    if windows.is_empty() {
        return Err(VerifiedPrimitiveError::EmptyTimeWindowChain);
    }

    let verified_windows = windows.iter().map(to_verified_time_window).collect();
    let composed = ucan_temporal::compose_chain(&verified_windows);
    Ok(from_verified_time_window(&composed))
}

/// Check whether a child capability list is a valid attenuation of a parent.
///
/// # Errors
///
/// Returns [`VerifiedPrimitiveError::InvalidCapability`] when any capability in
/// either list fails core validation.
pub fn attenuation_chain_is_valid(
    parent_capabilities: &[CapabilityDocument],
    child_capabilities: &[CapabilityDocument],
) -> VerifiedResult<bool> {
    let parent_capabilities = build_token_capabilities(parent_capabilities)?;
    let child_capabilities = build_token_capabilities(child_capabilities)?;
    Ok(ucan_token::check_attenuation_chain(&parent_capabilities, &child_capabilities))
}

/// Check whether a finite capability set authorizes an exact resource/ability request.
///
/// This advanced helper mirrors the vendored capability-merge authorizer: it
/// accepts when any supplied capability has a resource byte-prefix that contains
/// the requested resource and an ability that exactly equals the requested
/// ability. It intentionally does not perform wildcard ability subsumption;
/// callers needing full UCAN token/effective-delegation semantics should use the
/// shell authorization APIs instead.
///
/// # Errors
///
/// Returns [`VerifiedPrimitiveError::InvalidCapability`] when any supplied
/// capability or the requested resource/ability pair fails core validation.
pub fn capability_set_authorizes(
    capabilities: &[CapabilityDocument],
    resource: &str,
    ability: &str,
) -> VerifiedResult<bool> {
    let requested = CapabilityDocument::new(resource.to_owned(), ability.to_owned()).map_err(|error| match error {
        crate::Error::Capability { source } => VerifiedPrimitiveError::InvalidCapability { source },
        crate::Error::Io { .. } => unreachable!("CapabilityDocument::new performs validation only"),
        crate::Error::Caveat { .. } | crate::Error::TokenClaims { .. } => {
            unreachable!("CapabilityDocument::new does not include caveats")
        }
    })?;
    let capability_set = build_capability_merge_entries(capabilities)?;
    Ok(ucan_capability_merge::check_authorized_by(
        &capability_set,
        &requested.resource.as_bytes().to_vec(),
        &requested.ability.as_bytes().to_vec(),
    ))
}

/// Check whether a parent proof audience links to a child token issuer.
///
/// This is the two-entry proof-link slice of the verified UCAN chain checker;
/// it does not perform proof resolution, signature verification, revocation, or
/// temporal validation.
#[must_use]
pub fn proof_link_is_valid(parent_audience_did: &str, child_issuer_did: &str) -> bool {
    let chain = vec![
        ucan_chain::ChainEntry {
            iss: Vec::new(),
            aud: parent_audience_did.as_bytes().to_vec(),
            att: Vec::new(),
            exp: 0,
            nbf: 0,
            sig_valid: true,
        },
        ucan_chain::ChainEntry {
            iss: child_issuer_did.as_bytes().to_vec(),
            aud: Vec::new(),
            att: Vec::new(),
            exp: 0,
            nbf: 0,
            sig_valid: true,
        },
    ];
    ucan_chain::check_chain_linked(&chain)
}

/// Check structural validity for caller-supplied resolved UCAN chain entries.
///
/// This is the shell bridge to verified-logic's composed structural chain
/// checker. It checks finite issuer/audience linkage, caller-provided
/// signature-valid flags, exact attenuation narrowing, and temporal validity.
/// It does not perform proof resolution, cryptographic signature verification,
/// DID/key lookup, revocation checks, caveat policy evaluation, wildcard
/// containment replacement, networking, persistence, or ambient clock reads.
///
/// # Errors
///
/// Returns [`VerifiedPrimitiveError::InvalidCapability`] when any supplied
/// capability fails core capability validation.
pub fn structural_chain_is_valid(
    entries: &[VerifiedChainEntry],
    now_seconds: u64,
    tolerance_seconds: u64,
) -> VerifiedResult<bool> {
    let chain = entries.iter().map(to_verified_chain_entry).collect::<VerifiedResult<Vec<_>>>()?;
    Ok(ucan_chain::check_chain_structural_valid(&chain, now_seconds, tolerance_seconds))
}

/// Check structural validity and non-revocation for caller-supplied resolved chain data.
///
/// This is the shell bridge to verified-logic's composed revocation-chain
/// checker. It checks finite structural chain validity, one-to-one alignment
/// between chain entries and proof-reference bytes, and finite revoked-reference
/// non-membership.
///
/// It does not resolve proofs, derive references from compact tokens, check
/// backend availability or revocation freshness, perform cryptographic signature
/// verification, evaluate caveat policy, access registries, use storage/network
/// backends, or read an ambient clock.
///
/// # Errors
///
/// Returns [`VerifiedPrimitiveError::InvalidCapability`] when any supplied
/// chain capability fails core capability validation.
pub fn structural_chain_not_revoked_is_valid(
    entries: &[VerifiedChainEntry],
    chain_references: &[Vec<u8>],
    revoked_references: &[Vec<u8>],
    now_seconds: u64,
    tolerance_seconds: u64,
) -> VerifiedResult<bool> {
    let chain = entries.iter().map(to_verified_chain_entry).collect::<VerifiedResult<Vec<_>>>()?;
    let revocation_set = ucan_revocation::RevocationSet {
        entries: revoked_references.to_vec(),
    };
    let chain_references = chain_references.to_vec();
    Ok(ucan_revocation::check_chain_structural_valid_not_revoked(
        &revocation_set,
        &chain,
        &chain_references,
        now_seconds,
        tolerance_seconds,
    ))
}

/// Check resolved chain authorization for a concrete invocation request.
///
/// This is the shell bridge to verified-logic's composed invocation-chain
/// checker. It checks finite structural chain validity, aligned proof-reference
/// non-revocation, explicit authorization-reference binding to the chain leaf,
/// leaf capability index admission, invoker/audience equality, request-resource
/// refinement, and exact ability equality.
///
/// It does not resolve proofs, parse or verify compact UCANs, perform
/// cryptographic signature verification, evaluate caveat policy, replace
/// wildcard containment semantics, check revocation backend availability or
/// freshness, access registries/storage/network backends, persist receipts, or
/// read an ambient clock.
///
/// # Errors
///
/// Returns [`VerifiedPrimitiveError::InvalidCapability`] when any supplied
/// chain capability fails core capability validation.
pub fn invocation_chain_is_authorized(
    entries: &[VerifiedChainEntry],
    chain_references: &[Vec<u8>],
    revoked_references: &[Vec<u8>],
    request: &VerifiedInvocationChainRequest,
    now_seconds: u64,
    tolerance_seconds: u64,
) -> VerifiedResult<bool> {
    let chain = entries.iter().map(to_verified_chain_entry).collect::<VerifiedResult<Vec<_>>>()?;
    let chain_references = chain_references.to_vec();
    let revocation_set = ucan_revocation::RevocationSet {
        entries: revoked_references.to_vec(),
    };
    let invocation = ucan_invocation::Invocation {
        auth_cid: request.authorization_reference.clone(),
        invoker: request.invoker_did.as_bytes().to_vec(),
        resource: request.resource.as_bytes().to_vec(),
        ability: request.ability.as_bytes().to_vec(),
        timestamp: now_seconds,
    };

    Ok(ucan_invocation::check_invocation_chain_authorized(
        &revocation_set,
        &chain,
        &chain_references,
        &invocation,
        request.leaf_capability_index,
        now_seconds,
        tolerance_seconds,
    ))
}

/// Validate signer-reported algorithm admission before signer-based issuance.
///
/// This proves only exact byte equality over shell/signer-supplied algorithm
/// bytes: `EdDSA`. It does not prove signer honesty, signer I/O, key provenance,
/// public-key derivation, signing, verify cryptographic signatures, or token provenance.
#[must_use]
pub fn signer_algorithm_is_valid(algorithm: &str) -> bool {
    ucan_header::supported_algorithm_is_valid(&algorithm.as_bytes().to_vec())
}

/// Validate parsed UCAN claims presence facts through the vendored verified primitive.
///
/// This validates only caller- or serde-shell-supplied field-presence booleans
/// for the required UCAN payload claims. It does not validate JSON parsing,
/// duplicate-key behavior, field extraction correctness, strings, capabilities,
/// proof-reference decoding, temporal ordering, signing input construction, signatures,
/// hashes, resolver state, or token provenance.
#[must_use]
pub fn parsed_claims_presence_is_valid(presence: VerifiedParsedClaimsPresence) -> bool {
    ucan_claims_presence::parsed_claims_presence_is_valid(ucan_claims_presence::ParsedClaimsPresence {
        issuer_present: presence.issuer_present,
        audience_present: presence.audience_present,
        attenuations_present: presence.attenuations_present,
        proofs_present: presence.proofs_present,
        expiration_present: presence.expiration_present,
        not_before_present: presence.not_before_present,
    })
}

/// Validate parsed UCAN temporal ordering through the vendored verified primitive.
///
/// This validates only caller- or serde-shell-supplied numeric timestamp values
/// and accepts exactly when `nbf < exp`. It does not validate JSON parsing,
/// numeric deserialization correctness, wall-clock freshness, clock source
/// correctness, expiration-at-now policy, signing input construction, signatures,
/// resolver state, proof truth, or token provenance.
#[must_use]
pub fn parsed_temporal_order_is_valid(order: VerifiedParsedTemporalOrder) -> bool {
    ucan_temporal_order::parsed_temporal_order_is_valid(ucan_temporal_order::ParsedTemporalOrder {
        not_before_seconds: order.not_before_seconds,
        expires_at_seconds: order.expires_at_seconds,
    })
}

/// Validate parsed issuer/audience DID claim shape facts through vendored verified logic.
///
/// This validates only shell-supplied booleans for presence, non-empty value,
/// and already-trimmed value. It does not validate JSON parsing, UTF-8/string
/// extraction correctness, duplicate-key behavior, canonicalization, DID method
/// semantics, key resolution, signatures, backend state, or token provenance.
#[must_use]
pub fn parsed_did_claims_are_valid(claims: VerifiedParsedDidClaims) -> bool {
    ucan_parsed_did_claim::did_claims_are_valid(ucan_parsed_did_claim::ParsedDidClaimsFact {
        issuer: ucan_parsed_did_claim::ParsedDidClaimFact {
            present: claims.issuer.present,
            non_empty: claims.issuer.non_empty,
            trimmed: claims.issuer.trimmed,
        },
        audience: ucan_parsed_did_claim::ParsedDidClaimFact {
            present: claims.audience.present,
            non_empty: claims.audience.non_empty,
            trimmed: claims.audience.trimmed,
        },
    })
}

/// Validate parsed proof-reference list shape facts through vendored verified logic.
#[must_use]
pub fn parsed_proof_reference_list_shape_is_valid(shape: &VerifiedParsedProofReferenceListShape) -> bool {
    let entries = shape
        .entries
        .iter()
        .map(|entry| ucan_proof_reference_list_shape::ParsedProofReferenceEntryShape {
            encoded_non_empty: entry.encoded_non_empty,
            encoded_base64url_no_pad: entry.encoded_base64url_no_pad,
            decoded_non_empty: entry.decoded_non_empty,
        })
        .collect::<Vec<_>>();
    ucan_proof_reference_list_shape::proof_reference_list_shape_is_valid(&entries)
}

/// Validate proof-reference collection facts through vendored verified logic.
#[must_use]
pub fn proof_reference_collection_is_valid(collection: VerifiedProofReferenceCollection) -> bool {
    ucan_proof_reference_collection::proof_reference_collection_is_valid(
        ucan_proof_reference_collection::ProofReferenceCollectionFact {
            count: collection.count,
            max_count: collection.max_count,
            all_references_non_empty: collection.all_references_non_empty,
        },
    )
}

/// Validate parsed capability-list shape facts through vendored verified logic.
#[must_use]
pub fn parsed_capability_list_shape_is_valid(shape: &VerifiedParsedCapabilityListShape) -> bool {
    let capabilities = shape
        .capabilities
        .iter()
        .map(|capability| ucan_parsed_capability_shape::ParsedCapabilityShapeFact {
            resource_present: capability.resource_present,
            ability_present: capability.ability_present,
        })
        .collect::<Vec<_>>();
    ucan_parsed_capability_shape::capability_list_shape_is_valid(&capabilities)
}

/// Validate parsed caveat-list shape facts through vendored verified logic.
#[must_use]
pub fn parsed_caveat_list_shape_is_valid(shape: &VerifiedParsedCaveatListShape) -> bool {
    let caveats = shape
        .caveats
        .iter()
        .map(|caveat| ucan_parsed_caveat_shape::ParsedCaveatShapeFact {
            domain_present: caveat.domain_present,
            caveat_type_present: caveat.caveat_type_present,
            key_present: caveat.key_present,
            payload_present: caveat.payload_present,
        })
        .collect::<Vec<_>>();
    ucan_parsed_caveat_shape::caveat_list_shape_is_valid(&caveats)
}

/// Validate parsed caveat payload facts through vendored verified logic.
#[must_use]
pub fn parsed_caveat_payloads_are_valid(payloads: &VerifiedParsedCaveatPayloadList) -> bool {
    let facts = payloads
        .payloads
        .iter()
        .map(|payload| ucan_parsed_caveat_payload::ParsedCaveatPayloadFact {
            payload_present: payload.payload_present,
            encoded_non_empty: payload.encoded_non_empty,
            encoded_base64url_no_pad: payload.encoded_base64url_no_pad,
            decoded_non_empty: payload.decoded_non_empty,
        })
        .collect::<Vec<_>>();
    ucan_parsed_caveat_payload::caveat_payload_list_is_valid(&facts)
}

/// Validate proof-store result facts through vendored verified logic.
#[must_use]
pub fn proof_store_result_is_valid(fact: VerifiedProofStoreResultFact) -> bool {
    ucan_proof_store_result::proof_store_result_is_valid(ucan_proof_store_result::ProofStoreResultFact {
        reference_non_empty: fact.reference_non_empty,
        backend_succeeded: fact.backend_succeeded,
        compact_token_present: fact.compact_token_present,
    })
}

/// Validate revocation-check facts through vendored verified logic.
#[must_use]
pub fn revocation_check_is_valid(fact: VerifiedRevocationCheckFact) -> bool {
    ucan_revocation_check::revocation_check_is_valid(ucan_revocation_check::RevocationCheckFact {
        reference_non_empty: fact.reference_non_empty,
        backend_succeeded: fact.backend_succeeded,
        not_revoked: fact.not_revoked,
    })
}

const fn to_replay_admission_fact(fact: VerifiedReplayAdmissionFact) -> ucan_replay_admission::ReplayAdmissionFact {
    ucan_replay_admission::ReplayAdmissionFact {
        outcome: fact.outcome.to_verified_logic(),
    }
}

fn to_replay_admission_transcript(
    transcript: &VerifiedReplayAdmissionTranscript,
) -> ucan_replay_admission::ReplayAdmissionTranscript {
    ucan_replay_admission::ReplayAdmissionTranscript {
        facts: transcript.facts.iter().copied().map(to_replay_admission_fact).collect(),
    }
}

/// Evaluate a finite replay-admission transcript through vendored verified logic.
///
/// This bridge consumes only caller/shell-supplied replay admission outcomes. It
/// proves first-failure, fail-closed aggregation over the finite transcript. It
/// does not prove nonce uniqueness truth, request identity provenance, durable
/// replay storage, crash consistency, backend freshness, parser behavior,
/// consumer envelope semantics, or clock correctness.
#[must_use]
pub fn replay_admission_transcript_outcome(
    transcript: &VerifiedReplayAdmissionTranscript,
) -> VerifiedReplayAdmissionTranscriptOutcome {
    let transcript = to_replay_admission_transcript(transcript);
    VerifiedReplayAdmissionTranscriptOutcome {
        accepted: ucan_replay_admission::replay_admission_transcript_accepts(&transcript),
        first_failed_outcome: VerifiedReplayAdmissionOutcome::from_verified_logic(
            ucan_replay_admission::first_failed_replay_admission(&transcript),
        ),
    }
}

/// Return true exactly when the replay-admission transcript accepts.
#[must_use]
pub fn replay_admission_transcript_accepts(transcript: &VerifiedReplayAdmissionTranscript) -> bool {
    replay_admission_transcript_outcome(transcript).accepted
}

/// Return the first failed replay-admission class, or accepted when no failure exists.
#[must_use]
pub fn replay_admission_transcript_first_failed_outcome(
    transcript: &VerifiedReplayAdmissionTranscript,
) -> VerifiedReplayAdmissionOutcome {
    replay_admission_transcript_outcome(transcript).first_failed_outcome
}

/// Exposes the vendored opaque caveat identifier preservation predicate.
///
/// This proves only that each caller/shell-supplied parent caveat identifier
/// byte vector appears exactly in the child identifier set. It does not prove
/// caveat-document validation, length-prefix encoding provenance/injectivity,
/// payload parsing, caveat policy semantics, clocks, counters, backend truth,
/// token provenance, or cryptographic correctness.
#[must_use]
pub fn caveat_preservation_is_valid(preservation: &VerifiedCaveatPreservation) -> bool {
    ucan_caveat_preservation::parent_caveat_ids_preserved(
        &preservation.parent_caveat_ids,
        &preservation.child_caveat_ids,
    )
}

const fn to_caveat_evaluation_fact(fact: VerifiedCaveatEvaluationFact) -> ucan_caveat_evaluation::CaveatEvaluationFact {
    ucan_caveat_evaluation::CaveatEvaluationFact {
        policy_present: fact.policy_present,
        satisfied: fact.satisfied,
    }
}

fn to_caveat_evaluation_facts(
    transcript: &VerifiedCaveatEvaluationTranscript,
) -> Vec<ucan_caveat_evaluation::CaveatEvaluationFact> {
    transcript.facts.iter().copied().map(to_caveat_evaluation_fact).collect()
}

/// Admit a finite shell-emitted caveat evaluation transcript through the
/// vendored verified fail-closed aggregation predicate.
///
/// This proves only that every supplied fact has `policy_present=true` and
/// `satisfied=true`. It does not verify policy lookup truth, arbitrary policy
/// correctness, typed caveat parsing, clocks, counters, freshness, token
/// provenance, or cryptography.
#[must_use]
pub fn caveat_evaluation_transcript_accepts(transcript: &VerifiedCaveatEvaluationTranscript) -> bool {
    ucan_caveat_evaluation::caveat_evaluation_transcript_accepts(&to_caveat_evaluation_facts(transcript))
}

/// Return the first missing-policy or unsatisfied caveat index in a finite
/// shell-emitted caveat evaluation transcript, or the transcript length when all
/// supplied facts accept.
#[must_use]
pub fn caveat_evaluation_first_failed_index(transcript: &VerifiedCaveatEvaluationTranscript) -> usize {
    ucan_caveat_evaluation::first_failed_caveat_index(&to_caveat_evaluation_facts(transcript))
}

/// Validate that a shell-computed proof reference uses the supported fixed digest
///
/// This bridge accepts only fixed-width BLAKE3 digest bytes and exact
/// reference/digest byte equality. It intentionally does not compute or verify
/// BLAKE3, parse compact tokens, decode base64url, parse JSON, verify
/// signatures, resolve proofs, query revocation backends, prove token
/// provenance, access storage/network backends, persist state, replay-protect,
/// or read an ambient clock.
#[must_use]
pub fn proof_reference_digest_is_valid(reference: &[u8], digest: &VerifiedProofReferenceDigest) -> bool {
    let digest_bytes = digest.bytes.clone();
    let Some(digest) = ucan_proof_reference::proof_reference_digest_from_bytes(&digest_bytes) else {
        return false;
    };
    ucan_proof_reference::proof_reference_admits_digest(&reference.to_vec(), &digest)
}

/// Evaluate a finite verification-stage transcript through vendored verified logic.
///
/// This bridge consumes only caller/shell-supplied stage facts. It does not parse
/// tokens, decode base64url or JSON, hash proof references, verify Ed25519
/// signatures, fetch proofs, query revocation state, evaluate caveat semantics,
/// persist state, replay-protect, or read an ambient clock.
#[must_use]
pub fn verification_stage_transcript_outcome(
    transcript: &VerifiedVerificationStageTranscript,
) -> VerifiedVerificationStageOutcome {
    let transcript = ucan_verification_stage::VerificationStageTranscript {
        parse_ok: transcript.parse_ok,
        structural_ok: transcript.structural_ok,
        key_resolved: transcript.key_resolved,
        signature_valid: transcript.signature_valid,
        envelope_valid: transcript.envelope_valid,
        revocation_clear: transcript.revocation_clear,
        proofs_loaded: transcript.proofs_loaded,
        proof_chain_valid: transcript.proof_chain_valid,
        authorized: transcript.authorized,
    };
    VerifiedVerificationStageOutcome {
        accepted: ucan_verification_stage::stage_transcript_accepts(&transcript),
        first_failed_stage: VerifiedVerificationStage::from_verified_logic(
            ucan_verification_stage::first_failed_stage(&transcript),
        ),
    }
}

/// Check a finite proof traversal transcript produced by the shell.
///
/// This is the shell bridge to verified-logic's traversal-transcript model. It
/// checks caller-supplied resolved entries, visited-reference alignment,
/// duplicate-reference/cycle rejection, depth/proof/input-byte budgets,
/// issuer/audience linkage, attenuation narrowing, ordered caveat-id
/// preservation, leaf authorization, evaluation-time validity, and supplied
/// revocation-denial facts.
///
/// It does not resolve proofs, parse compact UCANs, verify signatures from
/// bytes, derive proof references, query revocation backends, evaluate opaque
/// caveat policies, access storage/network backends, persist state, sign,
/// replay-protect, or read an ambient clock.
///
/// # Errors
///
/// Returns [`VerifiedPrimitiveError::InvalidCapability`] when any supplied
/// traversal capability fails core capability validation.
pub fn proof_traversal_transcript_is_valid(
    transcript: &VerifiedProofTraversalTranscript,
    budget: &VerifiedProofTraversalBudget,
) -> VerifiedResult<bool> {
    let transcript = to_proof_traversal_transcript(transcript)?;
    let budget = ucan_proof_traversal::TraversalBudget {
        max_depth: budget.max_depth,
        max_proofs: budget.max_proofs,
        max_input_bytes: budget.max_input_bytes,
    };
    Ok(ucan_proof_traversal::traversal_transcript_valid(&transcript, &budget))
}

/// Validate a finite multi-parent proof DAG transcript with verified logic.
///
/// This is the DAG-shaped shell bridge to verified-logic's traversal-transcript
/// model. It checks caller-supplied resolved entries, topologically ordered
/// parent indices, depth/proof/input-byte budgets, issuer/audience linkage,
/// capability attenuation, ordered caveat-id preservation, leaf authorization,
/// evaluation-time validity, and supplied revocation-denial facts.
///
/// It does not resolve proofs, parse compact UCANs, verify signatures from
/// bytes, derive proof references, query revocation backends, evaluate opaque
/// caveat policies, access storage/network backends, persist state, sign,
/// replay-protect, or read an ambient clock.
///
/// # Errors
///
/// Returns [`VerifiedPrimitiveError::InvalidCapability`] when any supplied
/// traversal capability fails core capability validation.
pub fn proof_traversal_dag_transcript_is_valid(
    transcript: &VerifiedProofTraversalDagTranscript,
    budget: &VerifiedProofTraversalBudget,
) -> VerifiedResult<bool> {
    let transcript = to_proof_traversal_dag_transcript(transcript)?;
    let budget = ucan_proof_traversal::TraversalBudget {
        max_depth: budget.max_depth,
        max_proofs: budget.max_proofs,
        max_input_bytes: budget.max_input_bytes,
    };
    Ok(ucan_proof_traversal::dag_traversal_transcript_valid(&transcript, &budget))
}

/// Derive a Web-of-Trust principal digest from public key bytes.
///
/// This is a deterministic data-only bridge to the vendored WOT principal
/// primitive. It returns the fixed-width principal digest bytes used by
/// [`web_of_trust_ucan_authorization`] inputs, but it does not resolve
/// identities, fetch or validate trust records, check revocations, authorize
/// UCANs, access storage/network backends, or read an ambient clock.
#[must_use]
pub fn web_of_trust_principal_digest(public_key_bytes: &[u8]) -> Vec<u8> {
    web_of_trust_principal::principal_id_from_bytes(public_key_bytes).digest
}

/// Check whether raw Web-of-Trust principal digest bytes are admissible.
///
/// This is a deterministic data-only bridge to the vendored WOT principal
/// digest admission primitive. It accepts exactly the fixed-width digest bytes
/// used by [`web_of_trust_ucan_authorization`] inputs, but it does not resolve
/// identities, fetch or validate trust records, validate attestations, check
/// revocation freshness, authorize UCANs, access storage/network backends,
/// sign, replay-protect, or read an ambient clock.
#[must_use]
pub fn web_of_trust_principal_digest_is_valid(digest: &[u8]) -> bool {
    web_of_trust_principal::principal_id_from_digest_bytes(digest.to_vec()).is_some()
}

/// Check whether one typed caveat is satisfied by explicit caller-supplied context.
///
/// This is an opt-in data-only bridge for callers that choose the vendored typed
/// caveat semantics. It does not interpret opaque shell caveat payloads, install
/// default caveat policies, access storage/network backends, or read an ambient
/// clock.
#[must_use]
pub fn typed_caveat_is_satisfied(caveat: &VerifiedWebOfTrustCaveat, context: &VerifiedWebOfTrustCaveatContext) -> bool {
    let caveat = to_web_of_trust_caveat(caveat);
    let context = to_web_of_trust_caveat_context(context);
    ucan_caveat::caveat_satisfied(&caveat, &context)
}

/// Check non-empty ASCII decimal payload shape before runtime typed caveat parsing.
///
/// This verifies only finite byte-shape admission. Numeric conversion, overflow
/// handling, clock/counter provenance, and caveat policy semantics remain shell
/// or caller boundaries.
#[must_use]
pub fn typed_caveat_ascii_decimal_payload_is_valid(payload: &[u8]) -> bool {
    ucan_caveat::ascii_decimal_payload_is_valid(&payload.to_vec())
}

/// Check supported `max_count:window_seconds` rate-limit caveat payload shape.
///
/// This verifies only exactly-one-colon ASCII-decimal byte admission before the
/// shell parses numeric values. It does not verify UTF-8/decimal parser
/// correctness, counter freshness, or application policy semantics.
#[must_use]
pub fn typed_caveat_rate_limit_payload_shape_is_valid(payload: &[u8]) -> bool {
    ucan_caveat::rate_limit_payload_shape_is_valid(&payload.to_vec())
}

/// Check whether every supplied typed caveat is satisfied by explicit context.
///
/// This helper is an opt-in bridge to the vendored finite caveat-set predicate.
/// It does not replace shell-owned [`crate::token::CaveatPolicy`] evaluation or
/// allow unknown opaque caveats by default.
#[must_use]
pub fn typed_caveat_set_is_satisfied(
    caveats: &[VerifiedWebOfTrustCaveat],
    context: &VerifiedWebOfTrustCaveatContext,
) -> bool {
    let caveats = caveats.iter().map(to_web_of_trust_caveat).collect::<Vec<_>>();
    let context = to_web_of_trust_caveat_context(context);
    ucan_caveat::caveat_set_satisfied(&caveats, &context)
}

/// Return canonical payload bytes for caller-supplied public WOT attestation evidence.
///
/// This deterministic helper delegates to the vendored WOT attestation payload
/// primitive. It constructs public payload bytes only; it does not sign, resolve
/// identities, fetch evidence, persist state, or read an ambient clock.
///
/// # Errors
///
/// Returns [`VerifiedPrimitiveError::InvalidPrincipalDigestLength`] when a
/// supplied WOT principal digest is not full width.
pub fn web_of_trust_attestation_payload_bytes(
    attestation: &VerifiedWebOfTrustAttestation,
) -> VerifiedResult<Option<Vec<u8>>> {
    let attestation = to_web_of_trust_attestation(attestation)?;
    Ok(web_of_trust_attestation::attestation_payload_bytes(&attestation))
}

/// Check the vendored signature-valid predicate for public WOT attestation evidence.
///
/// This checks the deterministic public evidence fields and provided signature
/// bytes against the vendored facade. It does not derive keys, sign, fetch
/// evidence, or replace aggregate WOT UCAN authorization.
///
/// # Errors
///
/// Returns [`VerifiedPrimitiveError::InvalidPrincipalDigestLength`] when a
/// supplied WOT principal digest is not full width.
pub fn web_of_trust_attestation_signature_is_valid(
    attestation: &VerifiedWebOfTrustAttestation,
) -> VerifiedResult<bool> {
    let attestation = to_web_of_trust_attestation(attestation)?;
    Ok(web_of_trust_attestation::attestation_signature_valid(&attestation))
}

/// Check whether public WOT attestation evidence is active at an explicit time.
///
/// This helper delegates to vendored WOT attestation semantics over finite
/// caller-supplied revocations. It does not fetch revocations or read an ambient
/// clock.
///
/// # Errors
///
/// Returns [`VerifiedPrimitiveError::InvalidPrincipalDigestLength`] when any
/// supplied WOT principal digest is not full width.
pub fn web_of_trust_attestation_is_active_at(
    attestation: &VerifiedWebOfTrustAttestation,
    revocations: &[VerifiedWebOfTrustRevocation],
    time_seconds: u64,
) -> VerifiedResult<bool> {
    let attestation = to_web_of_trust_attestation(attestation)?;
    let revocations = revocations.iter().map(to_web_of_trust_revocation).collect::<VerifiedResult<Vec<_>>>()?;
    Ok(web_of_trust_attestation::attestation_active_at(&attestation, &revocations, time_seconds))
}

/// Return canonical payload bytes for caller-supplied public WOT revocation evidence.
///
/// This deterministic helper delegates to the vendored WOT revocation payload
/// primitive. It constructs public payload bytes only; it does not sign, fetch
/// evidence, persist state, or read an ambient clock.
///
/// # Errors
///
/// Returns [`VerifiedPrimitiveError::InvalidPrincipalDigestLength`] when a
/// supplied WOT principal digest is not full width.
pub fn web_of_trust_revocation_payload_bytes(
    revocation: &VerifiedWebOfTrustRevocation,
) -> VerifiedResult<Option<Vec<u8>>> {
    let revocation = to_web_of_trust_revocation(revocation)?;
    Ok(web_of_trust_revocation::revocation_payload_bytes(&revocation))
}

/// Check the vendored signature-valid predicate for public WOT revocation evidence.
///
/// This checks deterministic public evidence fields and provided signature bytes
/// only. It does not resolve identities, sign, fetch registries, or replace
/// aggregate WOT UCAN authorization.
///
/// # Errors
///
/// Returns [`VerifiedPrimitiveError::InvalidPrincipalDigestLength`] when a
/// supplied WOT principal digest is not full width.
pub fn web_of_trust_revocation_signature_is_valid(revocation: &VerifiedWebOfTrustRevocation) -> VerifiedResult<bool> {
    let revocation = to_web_of_trust_revocation(revocation)?;
    Ok(web_of_trust_revocation::revocation_signature_valid(&revocation))
}

/// Check whether public WOT revocation evidence matches public attestation evidence.
///
/// This helper delegates to the vendored exact/family target matching predicate
/// over finite caller-supplied evidence. It does not fetch evidence or check
/// registry freshness.
///
/// # Errors
///
/// Returns [`VerifiedPrimitiveError::InvalidPrincipalDigestLength`] when any
/// supplied WOT principal digest is not full width.
pub fn web_of_trust_revocation_matches_attestation(
    revocation: &VerifiedWebOfTrustRevocation,
    attestation: &VerifiedWebOfTrustAttestation,
) -> VerifiedResult<bool> {
    let revocation = to_web_of_trust_revocation(revocation)?;
    let attestation = to_web_of_trust_attestation(attestation)?;
    Ok(web_of_trust_revocation::revocation_matches(&revocation, &attestation))
}

/// Check whether public WOT revocation evidence is effective for an attestation at an explicit
/// time.
///
/// This helper delegates to the vendored composed revocation predicate over
/// finite caller-supplied evidence. It does not fetch revocations, consult a
/// registry, or read an ambient clock.
///
/// # Errors
///
/// Returns [`VerifiedPrimitiveError::InvalidPrincipalDigestLength`] when any
/// supplied WOT principal digest is not full width.
pub fn web_of_trust_revocation_is_effective_at(
    revocation: &VerifiedWebOfTrustRevocation,
    attestation: &VerifiedWebOfTrustAttestation,
    time_seconds: u64,
) -> VerifiedResult<bool> {
    let revocation = to_web_of_trust_revocation(revocation)?;
    let attestation = to_web_of_trust_attestation(attestation)?;
    Ok(web_of_trust_revocation::revocation_effective_at(&revocation, &attestation, time_seconds))
}

/// Check whether finite WOT evidence reaches a trust threshold for a subject.
///
/// This is an advanced data-only bridge to vendored WOT policy semantics. It
/// consumes already-materialized public roots, attestations, revocations,
/// subject digest, threshold, depth, and time. It does not resolve identities,
/// fetch trust records, check registry freshness, or authorize a UCAN envelope.
///
/// # Errors
///
/// Returns [`VerifiedPrimitiveError::InvalidPrincipalDigestLength`] when any
/// supplied WOT principal digest is not full width.
pub fn web_of_trust_threshold_accepts(
    root_principal_digests: &[Vec<u8>],
    attestations: &[VerifiedWebOfTrustAttestation],
    revocations: &[VerifiedWebOfTrustRevocation],
    subject_principal_digest: &[u8],
    threshold: usize,
    depth: usize,
    time_seconds: u64,
) -> VerifiedResult<bool> {
    let roots = root_principal_digests
        .iter()
        .map(|digest| build_web_of_trust_principal(digest.as_slice()))
        .collect::<VerifiedResult<Vec<_>>>()?;
    let attestations = attestations.iter().map(to_web_of_trust_attestation).collect::<VerifiedResult<Vec<_>>>()?;
    let revocations = revocations.iter().map(to_web_of_trust_revocation).collect::<VerifiedResult<Vec<_>>>()?;
    let subject = build_web_of_trust_principal(subject_principal_digest)?;
    Ok(web_of_trust_policy::threshold_accepts(
        &roots,
        &attestations,
        &revocations,
        &subject,
        threshold,
        depth,
        time_seconds,
    ))
}

/// Authorize a UCAN token envelope with caller-supplied Web-of-Trust evidence.
///
/// This is the shell bridge to verified-logic's Web-of-Trust UCAN authorizer.
/// It delegates a structurally valid token envelope, caller-supplied
/// ability/caveat/DID-CID facts, active caveats, root principals,
/// attestations, revocations, threshold, depth, and deterministic evaluation
/// times to the vendored composed checker.
///
/// It does not parse or verify compact UCANs, perform cryptographic signature
/// verification, decode DID/CID bytes, resolve proofs, fetch keys, check
/// revocation backend availability or freshness, evaluate application caveat
/// policy, access registries/storage/network backends, persist trust records,
/// perform replay checks, or read an ambient clock.
///
/// # Errors
///
/// Returns [`VerifiedPrimitiveError::InvalidCapability`] when a supplied UCAN
/// capability fails core validation, or
/// [`VerifiedPrimitiveError::InvalidPrincipalDigestLength`] when a supplied
/// Web-of-Trust principal digest is not full width.
pub fn web_of_trust_ucan_authorization(
    request: &VerifiedWebOfTrustUcanAuthorizationRequest,
) -> VerifiedResult<VerifiedWebOfTrustAuthorization> {
    let payload = build_token_payload(&request.token_envelope)?;
    let roots = request
        .root_principal_digests
        .iter()
        .map(|digest| build_web_of_trust_principal(digest.as_slice()))
        .collect::<VerifiedResult<Vec<_>>>()?;
    let attestations =
        request.attestations.iter().map(to_web_of_trust_attestation).collect::<VerifiedResult<Vec<_>>>()?;
    let revocations = request.revocations.iter().map(to_web_of_trust_revocation).collect::<VerifiedResult<Vec<_>>>()?;
    let subject = build_web_of_trust_principal(request.subject_principal_digest.as_slice())?;
    let active_caveats = request.active_caveats.iter().map(to_web_of_trust_caveat).collect::<Vec<_>>();
    let caveat_context = to_web_of_trust_caveat_context(&request.caveat_context);

    let decision = web_of_trust_ucan_authorizer::web_of_trust_ucan_authorizer(
        &payload,
        request.signature_valid,
        request.now_seconds,
        request.tolerance_seconds,
        request.ability_allowed,
        request.caveats_contained,
        &active_caveats,
        &caveat_context,
        request.did_cid_linked,
        &roots,
        &attestations,
        &revocations,
        &subject,
        request.threshold,
        request.depth,
        request.trust_time_seconds,
    );

    Ok(match decision {
        web_of_trust_ucan_authorizer::WebOfTrustUcanAuthorization::Authorized => {
            VerifiedWebOfTrustAuthorization::Authorized
        }
        web_of_trust_ucan_authorizer::WebOfTrustUcanAuthorization::Denied => VerifiedWebOfTrustAuthorization::Denied,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifiedBackendOperation {
    KeyResolution,
    ProofLoad,
    Revocation,
}

impl VerifiedBackendOperation {
    const fn as_verified_code(self) -> u8 {
        match self {
            Self::KeyResolution => ucan_backend_transcript::BACKEND_KEY_RESOLUTION,
            Self::ProofLoad => ucan_backend_transcript::BACKEND_PROOF_LOAD,
            Self::Revocation => ucan_backend_transcript::BACKEND_REVOCATION,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedBackendOperationFact {
    pub operation: VerifiedBackendOperation,
    pub succeeded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedBackendTranscript {
    pub operations: Vec<VerifiedBackendOperationFact>,
}

fn to_backend_transcript(transcript: &VerifiedBackendTranscript) -> ucan_backend_transcript::BackendTranscript {
    ucan_backend_transcript::BackendTranscript {
        operations: transcript
            .operations
            .iter()
            .map(|operation| ucan_backend_transcript::BackendOperationFact {
                operation: operation.operation.as_verified_code(),
                succeeded: operation.succeeded,
            })
            .collect(),
    }
}

#[must_use]
pub fn backend_transcript_accepts(transcript: &VerifiedBackendTranscript) -> bool {
    ucan_backend_transcript::backend_transcript_accepts(&to_backend_transcript(transcript))
}

#[must_use]
pub fn backend_transcript_first_failed_operation(
    transcript: &VerifiedBackendTranscript,
) -> Option<VerifiedBackendOperation> {
    match ucan_backend_transcript::first_failed_backend_operation(&to_backend_transcript(transcript)) {
        ucan_backend_transcript::BACKEND_KEY_RESOLUTION => Some(VerifiedBackendOperation::KeyResolution),
        ucan_backend_transcript::BACKEND_PROOF_LOAD => Some(VerifiedBackendOperation::ProofLoad),
        ucan_backend_transcript::BACKEND_REVOCATION => Some(VerifiedBackendOperation::Revocation),
        _ => None,
    }
}

/// Check whether a proof-reference byte string is present in a finite revocation list.
///
/// This is the shell bridge to verified-logic's finite revocation-set lookup. It
/// does not perform revocation-registry lookup, freshness checks, persistence, or
/// proof-reference derivation.
#[must_use]
pub fn revocation_reference_is_listed(revoked_references: &[Vec<u8>], reference: &[u8]) -> bool {
    let revocation_set = ucan_revocation::RevocationSet {
        entries: revoked_references.to_vec(),
    };
    ucan_revocation::is_revoked(&revocation_set, &reference.to_vec())
}

/// Check concrete invocation admission against a leaf audience and concrete leaf capability.
///
/// This is the shell bridge to verified-logic's invocation authorization primitive. It
/// checks only caller-supplied invoker/audience byte equality, request-resource prefix
/// refinement, and exact ability equality. Equality does not prove key possession,
/// sender identity, or a holder signature. Wildcard ability containment, caveat policy evaluation,
/// proof traversal, signatures, and revocation checks remain in their existing shell paths.
#[must_use]
pub fn invocation_request_is_authorized(
    invoker_did: &str,
    leaf_audience_did: &str,
    request_resource: &str,
    request_ability: &str,
    leaf_resource: &str,
    leaf_ability: &str,
) -> bool {
    ucan_invocation::check_invocation_authorized(
        &invoker_did.as_bytes().to_vec(),
        &leaf_audience_did.as_bytes().to_vec(),
        &request_resource.as_bytes().to_vec(),
        &request_ability.as_bytes().to_vec(),
        &leaf_resource.as_bytes().to_vec(),
        &leaf_ability.as_bytes().to_vec(),
    )
}

/// Check a proof traversal depth against a maximum delegation-depth budget.
///
/// The shell's `depth` value is the number of proof hops from the leaf token.
/// The verified primitive models a chain length where a leaf-only chain has
/// length one and depth zero, so this bridge translates `depth` to
/// `chain_len = depth + 1` before checking the budget. Overflow cannot be a
/// valid finite chain length and is rejected.
#[must_use]
pub fn proof_depth_within_budget(depth: usize, max_depth: usize) -> bool {
    ucan_delegation_depth::check_proof_hop_depth_budget(depth, max_depth)
}

/// Check deterministic invocation receipt execution fields.
///
/// This is the shell bridge to verified-logic's receipt-validity primitive. It
/// checks only that the receipt executor matches the invocation invoker and that
/// the receipt timestamp is not earlier than the invocation timestamp. It does
/// not perform receipt signing, persistence, replay protection, output/error
/// serialization, or ambient clock reads.
#[must_use]
pub fn receipt_execution_is_valid(
    invocation_invoker_did: &str,
    receipt_executor_did: &str,
    invocation_timestamp_seconds: u64,
    receipt_timestamp_seconds: u64,
) -> bool {
    let receipt = ucan_invocation::Receipt {
        invocation: ucan_invocation::Invocation {
            auth_cid: Vec::new(),
            invoker: invocation_invoker_did.as_bytes().to_vec(),
            resource: Vec::new(),
            ability: Vec::new(),
            timestamp: invocation_timestamp_seconds,
        },
        ran_by: receipt_executor_did.as_bytes().to_vec(),
        result: ucan_invocation::InvocationResult::Ok { output: Vec::new() },
        receipt_timestamp: receipt_timestamp_seconds,
    };
    ucan_invocation::check_receipt_valid(&receipt)
}

/// Validate a UCAN envelope using verified-logic's checked pipeline.
///
/// # Errors
///
/// Returns [`VerifiedPrimitiveError::InvalidCapability`] when the envelope
/// contains invalid capability documents. Returns
/// [`VerifiedPrimitiveError::TokenValidation`] when the verified envelope
/// checks fail.
pub fn validate_token_envelope(
    envelope: &TokenEnvelopeInput,
    signature_valid: bool,
    now_seconds: u64,
    tolerance_seconds: u64,
) -> VerifiedResult<()> {
    let payload = build_token_payload(envelope)?;
    ucan_token::validate_envelope(&payload, signature_valid, now_seconds, tolerance_seconds)
        .map(|_| ())
        .map_err(|error| VerifiedPrimitiveError::TokenValidation {
            failure: map_validation_failure(&error),
        })
}

/// Encode an Ed25519 public key as a verified did:key.
///
/// # Errors
///
/// Returns [`VerifiedPrimitiveError::InvalidPublicKeyLength`] when the public
/// key is not 32 bytes long.
pub fn encode_ed25519_did_key(public_key_bytes: &[u8]) -> VerifiedResult<Vec<u8>> {
    validate_public_key_length(public_key_bytes)?;
    let did_key = ucan_did::encode_did_key(&public_key_bytes.to_vec(), ucan_did::CODEC_ED25519);
    Ok(did_key.encoded)
}

/// Decode a `did:key` byte string through the verified DID-key decoder.
///
/// Returns decoded public-key bytes, the multicodec value, and the original
/// encoded bytes when the supplied bytes satisfy the vendored structural
/// decoder. Malformed prefixes, base58btc payloads, or multicodec structures
/// return `None`.
///
/// This helper is a deterministic structural decoder only. It does not perform
/// remote DID resolution, key lookup, cryptographic signature verification,
/// trust-chain checks, proof fetching, file or network lookup,
/// persistence, or ambient clock reads.
#[must_use]
pub fn decode_did_key(encoded: &[u8]) -> Option<VerifiedDidKey> {
    ucan_did::decode_did_key(&encoded.to_vec()).map(|did_key| VerifiedDidKey {
        public_key_bytes: did_key.public_key_bytes,
        codec: did_key.codec,
        encoded: did_key.encoded,
    })
}

/// Derive an Ed25519 public key from a secret key.
///
/// # Errors
///
/// Returns [`VerifiedPrimitiveError::InvalidSecretKeyLength`] when the secret
/// key is not 32 bytes long.
pub fn derive_ed25519_public_key(secret_key_bytes: &[u8]) -> VerifiedResult<[u8; ucan_signature::PUBLIC_KEY_LEN]> {
    let secret_key = fixed_secret_key(secret_key_bytes)?;
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&secret_key);
    Ok(signing_key.verifying_key().to_bytes())
}

/// Sign a message with an Ed25519 secret key.
///
/// # Errors
///
/// Returns [`VerifiedPrimitiveError::InvalidSecretKeyLength`] when the secret
/// key is not 32 bytes long.
pub fn sign_ed25519(secret_key_bytes: &[u8], message: &[u8]) -> VerifiedResult<[u8; ucan_signature::SIGNATURE_LEN]> {
    let secret_key = fixed_secret_key(secret_key_bytes)?;
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&secret_key);
    Ok(signing_key.sign(message).to_bytes())
}

/// Verify an Ed25519 signature against a public key and message.
///
/// # Errors
///
/// Returns a length error when the public key or signature bytes are the wrong
/// size for Ed25519.
pub fn verify_ed25519_signature(
    public_key_bytes: &[u8],
    message: &[u8],
    signature_bytes: &[u8],
) -> VerifiedResult<bool> {
    validate_public_key_length(public_key_bytes)?;
    validate_signature_length(signature_bytes)?;
    let public_key_array: &[u8; ucan_signature::PUBLIC_KEY_LEN] =
        public_key_bytes.try_into().map_err(|_| VerifiedPrimitiveError::InvalidPublicKeyLength {
            actual: public_key_bytes.len(),
        })?;
    let public_key =
        VerifyingKey::from_bytes(public_key_array).map_err(|_| VerifiedPrimitiveError::InvalidPublicKeyEncoding)?;
    let signature = ed25519_dalek::Signature::from_slice(signature_bytes).map_err(|_| {
        VerifiedPrimitiveError::InvalidSignatureLength {
            actual: signature_bytes.len(),
        }
    })?;
    Ok(public_key.verify(message, &signature).is_ok())
}

fn validate_capability_document(document: &CapabilityDocument) -> VerifiedResult<()> {
    document.view().map(|_| ()).map_err(|error| match error {
        crate::Error::Capability { source } => VerifiedPrimitiveError::InvalidCapability { source },
        crate::Error::Io { .. } | crate::Error::Caveat { .. } | crate::Error::TokenClaims { .. } => {
            unreachable!("capability document view validates resource/ability only")
        }
    })
}

fn collect_ability_segments(ability: &str) -> VerifiedResult<Vec<Vec<u8>>> {
    ucan_core::validate_ability_value(ability)
        .map_err(|source| VerifiedPrimitiveError::InvalidCapability { source })?;
    Ok(ability.split(ABILITY_SEGMENT_SEPARATOR).map(|segment| segment.as_bytes().to_vec()).collect())
}

fn build_verified_capability(document: CapabilityDocument) -> VerifiedResult<VerifiedCapability> {
    let ability_segments = collect_ability_segments(document.ability.as_str())?;
    Ok(VerifiedCapability {
        document,
        ability_segments,
    })
}

const fn to_verified_time_window(window: &TokenTimeWindow) -> ucan_temporal::TimeWindow {
    ucan_temporal::TimeWindow {
        nbf: window.not_before_seconds,
        exp: window.expires_at_seconds,
    }
}

const fn from_verified_time_window(window: &ucan_temporal::TimeWindow) -> TokenTimeWindow {
    TokenTimeWindow {
        not_before_seconds: window.nbf,
        expires_at_seconds: window.exp,
    }
}

fn build_token_capability(document: &CapabilityDocument) -> VerifiedResult<ucan_token::UcanCapability> {
    validate_capability_document(document)?;
    Ok(ucan_token::UcanCapability {
        resource: document.resource.as_bytes().to_vec(),
        ability: document.ability.as_bytes().to_vec(),
    })
}

fn build_token_capabilities(documents: &[CapabilityDocument]) -> VerifiedResult<Vec<ucan_token::UcanCapability>> {
    documents.iter().map(build_token_capability).collect()
}

fn build_capability_merge_entry(document: &CapabilityDocument) -> VerifiedResult<ucan_capability_merge::CapEntry> {
    validate_capability_document(document)?;
    Ok(ucan_capability_merge::CapEntry {
        resource: document.resource.as_bytes().to_vec(),
        ability: document.ability.as_bytes().to_vec(),
    })
}

fn build_capability_merge_entries(
    documents: &[CapabilityDocument],
) -> VerifiedResult<Vec<ucan_capability_merge::CapEntry>> {
    documents.iter().map(build_capability_merge_entry).collect()
}

fn to_chain_capability(document: &CapabilityDocument) -> VerifiedResult<(Vec<u8>, Vec<u8>)> {
    validate_capability_document(document)?;
    Ok((document.resource.as_bytes().to_vec(), document.ability.as_bytes().to_vec()))
}

fn to_verified_chain_entry(entry: &VerifiedChainEntry) -> VerifiedResult<ucan_chain::ChainEntry> {
    let att = entry.capabilities.iter().map(to_chain_capability).collect::<VerifiedResult<Vec<_>>>()?;
    Ok(ucan_chain::ChainEntry {
        iss: entry.issuer_did.as_bytes().to_vec(),
        aud: entry.audience_did.as_bytes().to_vec(),
        att,
        exp: entry.expires_at_seconds,
        nbf: entry.not_before_seconds,
        sig_valid: entry.signature_valid,
    })
}

fn to_proof_traversal_capability(
    document: &CapabilityDocument,
) -> VerifiedResult<ucan_proof_traversal::TraversalCapability> {
    validate_capability_document(document)?;
    Ok(ucan_proof_traversal::TraversalCapability {
        resource: document.resource.as_bytes().to_vec(),
        ability: ucan_ability::Ability {
            segments: collect_ability_segments(document.ability.as_str())?,
        },
    })
}

fn to_proof_traversal_entry(
    entry: &VerifiedProofTraversalEntry,
) -> VerifiedResult<ucan_proof_traversal::TraversalEntry> {
    let capabilities =
        entry.capabilities.iter().map(to_proof_traversal_capability).collect::<VerifiedResult<Vec<_>>>()?;
    Ok(ucan_proof_traversal::TraversalEntry {
        proof_ref: entry.proof_reference.clone(),
        issuer: entry.issuer_did.as_bytes().to_vec(),
        audience: entry.audience_did.as_bytes().to_vec(),
        capabilities,
        caveat_ids: entry.caveat_ids.clone(),
        nbf: entry.not_before_seconds,
        exp: entry.expires_at_seconds,
        signature_valid: entry.signature_valid,
        revoked: entry.revoked,
    })
}

fn to_proof_traversal_transcript(
    transcript: &VerifiedProofTraversalTranscript,
) -> VerifiedResult<ucan_proof_traversal::TraversalTranscript> {
    let entries = transcript.entries.iter().map(to_proof_traversal_entry).collect::<VerifiedResult<Vec<_>>>()?;
    Ok(ucan_proof_traversal::TraversalTranscript {
        entries,
        visited_refs: transcript.visited_references.clone(),
        total_input_bytes: transcript.total_input_bytes,
        target: to_proof_traversal_target(&transcript.target)?,
    })
}

fn to_proof_traversal_dag_transcript(
    transcript: &VerifiedProofTraversalDagTranscript,
) -> VerifiedResult<ucan_proof_traversal::TraversalDagTranscript> {
    let entries = transcript.entries.iter().map(to_proof_traversal_entry).collect::<VerifiedResult<Vec<_>>>()?;
    Ok(ucan_proof_traversal::TraversalDagTranscript {
        entries,
        parent_indices: transcript.parent_indices.clone(),
        visited_refs: transcript.visited_references.clone(),
        total_input_bytes: transcript.total_input_bytes,
        target: to_proof_traversal_target(&transcript.target)?,
    })
}

fn to_proof_traversal_target(
    target: &VerifiedProofTraversalTarget,
) -> VerifiedResult<ucan_proof_traversal::TraversalAuthorizationTarget> {
    Ok(ucan_proof_traversal::TraversalAuthorizationTarget {
        resource: target.resource.as_bytes().to_vec(),
        ability: ucan_ability::Ability {
            segments: collect_ability_segments(target.ability.as_str())?,
        },
        now: target.now_seconds,
        tolerance: target.tolerance_seconds,
    })
}

fn build_token_payload(envelope: &TokenEnvelopeInput) -> VerifiedResult<ucan_token::UcanPayload> {
    let capabilities = build_token_capabilities(envelope.capabilities.as_slice())?;
    Ok(ucan_token::UcanPayload {
        iss: envelope.issuer_did.as_bytes().to_vec(),
        aud: envelope.audience_did.as_bytes().to_vec(),
        att: capabilities,
        prf: envelope.proofs.clone(),
        exp: envelope.expires_at_seconds,
        nbf: envelope.not_before_seconds,
    })
}

fn build_web_of_trust_principal(digest: &[u8]) -> VerifiedResult<web_of_trust_principal::PrincipalId> {
    let actual = digest.len();
    web_of_trust_principal::principal_id_from_digest_bytes(digest.to_vec())
        .ok_or(VerifiedPrimitiveError::InvalidPrincipalDigestLength { actual })
}

const fn to_web_of_trust_caveat(caveat: &VerifiedWebOfTrustCaveat) -> ucan_caveat::Caveat {
    match caveat {
        VerifiedWebOfTrustCaveat::TimeBound { not_after_seconds } => ucan_caveat::Caveat::TimeBound {
            not_after: *not_after_seconds,
        },
        VerifiedWebOfTrustCaveat::RateLimit {
            max_count,
            window_seconds,
        } => ucan_caveat::Caveat::RateLimit {
            max_count: *max_count,
            window_secs: *window_seconds,
        },
    }
}

const fn to_web_of_trust_caveat_context(context: &VerifiedWebOfTrustCaveatContext) -> ucan_caveat::CaveatContext {
    ucan_caveat::CaveatContext {
        now: context.now_seconds,
        count_in_window: context.count_in_window,
    }
}

fn to_web_of_trust_attestation(
    attestation: &VerifiedWebOfTrustAttestation,
) -> VerifiedResult<web_of_trust_attestation::Attestation> {
    Ok(web_of_trust_attestation::Attestation {
        issuer_key_bytes: attestation.issuer_key_bytes.clone(),
        issuer: build_web_of_trust_principal(attestation.issuer_digest.as_slice())?,
        subject_key_bytes: attestation.subject_key_bytes.clone(),
        subject: build_web_of_trust_principal(attestation.subject_digest.as_slice())?,
        role_tag: attestation.role_tag,
        not_before: attestation.not_before_seconds,
        not_after: attestation.not_after_seconds,
        sequence_no: attestation.sequence_no,
        signature: attestation.signature.clone(),
    })
}

fn to_web_of_trust_revocation(
    revocation: &VerifiedWebOfTrustRevocation,
) -> VerifiedResult<web_of_trust_revocation::Revocation> {
    Ok(web_of_trust_revocation::Revocation {
        issuer_key_bytes: revocation.issuer_key_bytes.clone(),
        issuer: build_web_of_trust_principal(revocation.issuer_digest.as_slice())?,
        target_kind_tag: revocation.target_kind_tag,
        exact_payload_target: revocation.exact_payload_target.clone(),
        family_subject_key_bytes: revocation.family_subject_key_bytes.clone(),
        family_subject: build_web_of_trust_principal(revocation.family_subject_digest.as_slice())?,
        family_role_tag: revocation.family_role_tag,
        effective_time: revocation.effective_time_seconds,
        signature: revocation.signature.clone(),
    })
}

const fn map_validation_failure(error: &ucan_token::ValidationError) -> TokenValidationFailure {
    match error {
        ucan_token::ValidationError::ExpiredToken => TokenValidationFailure::ExpiredToken,
        ucan_token::ValidationError::NotYetValid => TokenValidationFailure::NotYetValid,
        ucan_token::ValidationError::SelfDelegation => TokenValidationFailure::SelfDelegation,
        ucan_token::ValidationError::MissingIssuer => TokenValidationFailure::MissingIssuer,
        ucan_token::ValidationError::MissingAudience => TokenValidationFailure::MissingAudience,
        ucan_token::ValidationError::MissingAttenuations => TokenValidationFailure::MissingAttenuations,
        ucan_token::ValidationError::InvalidSignature => TokenValidationFailure::InvalidSignature,
    }
}

fn fixed_secret_key(secret_key_bytes: &[u8]) -> VerifiedResult<Zeroizing<[u8; ucan_signature::SECRET_KEY_LEN]>> {
    validate_secret_key_length(secret_key_bytes)?;
    let mut fixed = Zeroizing::new([0_u8; ucan_signature::SECRET_KEY_LEN]);
    fixed.copy_from_slice(secret_key_bytes);
    Ok(fixed)
}

fn validate_secret_key_length(secret_key_bytes: &[u8]) -> VerifiedResult<()> {
    let actual = secret_key_bytes.len();
    if ucan_signature::secret_key_len_is_valid(actual) {
        return Ok(());
    }
    Err(VerifiedPrimitiveError::InvalidSecretKeyLength { actual })
}

fn validate_public_key_length(public_key_bytes: &[u8]) -> VerifiedResult<()> {
    let actual = public_key_bytes.len();
    if ucan_signature::public_key_len_is_valid(actual) {
        return Ok(());
    }
    Err(VerifiedPrimitiveError::InvalidPublicKeyLength { actual })
}

fn validate_signature_length(signature_bytes: &[u8]) -> VerifiedResult<()> {
    let actual = signature_bytes.len();
    if ucan_signature::signature_len_is_valid(actual) {
        return Ok(());
    }
    Err(VerifiedPrimitiveError::InvalidSignatureLength { actual })
}

#[cfg(test)]
mod tests {
    use verified_logic::ucan_signature;
    use verified_logic::web_of_trust_attestation as verified_wot_attestation;
    use verified_logic::web_of_trust_principal as verified_wot_principal;
    use verified_logic::web_of_trust_revocation as verified_wot_revocation;

    use super::TokenEnvelopeInput;
    use super::TokenTimeWindow;
    use super::TokenValidationFailure;
    use super::VerifiedBackendOperation;
    use super::VerifiedBackendOperationFact;
    use super::VerifiedBackendTranscript;
    use super::VerifiedCapability;
    use super::VerifiedCaveatEvaluationFact;
    use super::VerifiedCaveatEvaluationTranscript;
    use super::VerifiedCaveatPreservation;
    use super::VerifiedChainEntry;
    use super::VerifiedInvocationChainRequest;
    use super::VerifiedParsedClaimsPresence;
    use super::VerifiedParsedDidClaim;
    use super::VerifiedParsedDidClaims;
    use super::VerifiedParsedTemporalOrder;
    use super::VerifiedPrimitiveError;
    use super::VerifiedProofReferenceDigest;
    use super::VerifiedProofTraversalBudget;
    use super::VerifiedProofTraversalDagTranscript;
    use super::VerifiedProofTraversalEntry;
    use super::VerifiedProofTraversalTarget;
    use super::VerifiedProofTraversalTranscript;
    use super::VerifiedReplayAdmissionFact;
    use super::VerifiedReplayAdmissionOutcome;
    use super::VerifiedReplayAdmissionTranscript;
    use super::VerifiedVerificationStage;
    use super::VerifiedVerificationStageTranscript;
    use super::VerifiedWebOfTrustAttestation;
    use super::VerifiedWebOfTrustAuthorization;
    use super::VerifiedWebOfTrustCaveat;
    use super::VerifiedWebOfTrustCaveatContext;
    use super::VerifiedWebOfTrustRevocation;
    use super::VerifiedWebOfTrustUcanAuthorizationRequest;
    use super::attenuation_chain_is_valid;
    use super::backend_transcript_accepts;
    use super::backend_transcript_first_failed_operation;
    use super::capability_set_authorizes;
    use super::caveat_evaluation_first_failed_index;
    use super::caveat_evaluation_transcript_accepts;
    use super::caveat_preservation_is_valid;
    use super::compose_time_window_chain;
    use super::decode_did_key;
    use super::derive_ed25519_public_key;
    use super::encode_ed25519_did_key;
    use super::invocation_chain_is_authorized;
    use super::invocation_request_is_authorized;
    use super::parsed_claims_presence_is_valid;
    use super::parsed_did_claims_are_valid;
    use super::parsed_temporal_order_is_valid;
    use super::proof_depth_within_budget;
    use super::proof_link_is_valid;
    use super::proof_reference_digest_is_valid;
    use super::proof_traversal_dag_transcript_is_valid;
    use super::proof_traversal_transcript_is_valid;
    use super::receipt_execution_is_valid;
    use super::replay_admission_transcript_accepts;
    use super::replay_admission_transcript_first_failed_outcome;
    use super::revocation_reference_is_listed;
    use super::sign_ed25519;
    use super::signer_algorithm_is_valid;
    use super::structural_chain_is_valid;
    use super::structural_chain_not_revoked_is_valid;
    use super::token_needs_refresh;
    use super::token_time_valid;
    use super::typed_caveat_is_satisfied;
    use super::typed_caveat_set_is_satisfied;
    use super::validate_token_envelope;
    use super::verification_stage_transcript_outcome;
    use super::verify_ed25519_signature;
    use super::web_of_trust_attestation_is_active_at;
    use super::web_of_trust_attestation_payload_bytes;
    use super::web_of_trust_attestation_signature_is_valid;
    use super::web_of_trust_principal_digest;
    use super::web_of_trust_principal_digest_is_valid;
    use super::web_of_trust_revocation_is_effective_at;
    use super::web_of_trust_revocation_matches_attestation;
    use super::web_of_trust_revocation_payload_bytes;
    use super::web_of_trust_revocation_signature_is_valid;
    use super::web_of_trust_threshold_accepts;
    use super::web_of_trust_ucan_authorization;
    use crate::shell::CapabilityDocument;

    const VALID_RESOURCE: &str = "did:key:z6Mkexample";
    const VALID_RESOURCE_CHILD: &str = "did:key:z6Mkexample/photos";
    const VALID_ABILITY_GLOB: &str = "store/*";
    const VALID_ABILITY_CHILD: &str = "store/add";
    const VALID_ABILITY_OTHER_NAMESPACE: &str = "upload/add";
    const SCOPE_RESOURCE: &str = "did:key:z6Mkresource";
    const SCOPE_NESTED_RESOURCE: &str = "did:key:z6Mkresource/photos";
    const SCOPE_SIBLING_OVERMATCH_RESOURCE: &str = "did:key:z6MkresourceX";
    const SCOPE_UNRELATED_RESOURCE: &str = "did:key:z6Mkother";
    const SCOPE_ABILITY: &str = "store/add";
    const ISSUED_AT_SECONDS: u64 = 1_000;
    const EXPIRES_AT_SECONDS: u64 = 2_000;
    const NOW_SECONDS: u64 = 1_500;
    const FRESH_NOW_SECONDS: u64 = 1_000;
    const EXPIRED_NOW_SECONDS: u64 = 2_061;
    const SKEW_VALID_NOW_SECONDS: u64 = 2_060;
    const REFRESH_NOW_SECONDS: u64 = 1_800;
    const WINDOW_ONE_NOT_BEFORE_SECONDS: u64 = 100;
    const WINDOW_ONE_EXPIRES_AT_SECONDS: u64 = 400;
    const WINDOW_TWO_NOT_BEFORE_SECONDS: u64 = 200;
    const WINDOW_TWO_EXPIRES_AT_SECONDS: u64 = 300;
    const SIGNING_KEY_BYTE: u8 = 7;
    const INVALID_LENGTH_BYTES: usize = 1;
    const MESSAGE: &[u8] = b"ucan verified primitives";
    const WRONG_MESSAGE: &[u8] = b"wrong message";

    #[test]
    fn backend_transcript_accepts_successful_backend_operations() {
        let transcript = VerifiedBackendTranscript {
            operations: vec![
                VerifiedBackendOperationFact {
                    operation: VerifiedBackendOperation::Revocation,
                    succeeded: true,
                },
                VerifiedBackendOperationFact {
                    operation: VerifiedBackendOperation::KeyResolution,
                    succeeded: true,
                },
                VerifiedBackendOperationFact {
                    operation: VerifiedBackendOperation::ProofLoad,
                    succeeded: true,
                },
            ],
        };

        assert!(backend_transcript_accepts(&transcript));
        assert_eq!(backend_transcript_first_failed_operation(&transcript), None);
    }

    #[test]
    fn backend_transcript_reports_first_failed_backend_operation() {
        let transcript = VerifiedBackendTranscript {
            operations: vec![
                VerifiedBackendOperationFact {
                    operation: VerifiedBackendOperation::Revocation,
                    succeeded: true,
                },
                VerifiedBackendOperationFact {
                    operation: VerifiedBackendOperation::ProofLoad,
                    succeeded: false,
                },
                VerifiedBackendOperationFact {
                    operation: VerifiedBackendOperation::KeyResolution,
                    succeeded: false,
                },
            ],
        };

        assert!(!backend_transcript_accepts(&transcript));
        assert_eq!(backend_transcript_first_failed_operation(&transcript), Some(VerifiedBackendOperation::ProofLoad));
    }

    #[test]
    fn replay_admission_transcript_accepts_allowed_fact() {
        let transcript = VerifiedReplayAdmissionTranscript {
            facts: vec![VerifiedReplayAdmissionFact {
                outcome: VerifiedReplayAdmissionOutcome::Allowed,
            }],
        };

        assert!(replay_admission_transcript_accepts(&transcript));
        assert_eq!(
            replay_admission_transcript_first_failed_outcome(&transcript),
            VerifiedReplayAdmissionOutcome::Accepted
        );
    }

    #[test]
    fn replay_admission_transcript_is_fail_closed() {
        for outcome in [
            VerifiedReplayAdmissionOutcome::Duplicate,
            VerifiedReplayAdmissionOutcome::Stale,
            VerifiedReplayAdmissionOutcome::Malformed,
            VerifiedReplayAdmissionOutcome::Denied,
            VerifiedReplayAdmissionOutcome::Backend,
            VerifiedReplayAdmissionOutcome::Unknown { outcome: 42 },
        ] {
            let transcript = VerifiedReplayAdmissionTranscript {
                facts: vec![VerifiedReplayAdmissionFact { outcome }],
            };
            assert!(!replay_admission_transcript_accepts(&transcript));
            assert_eq!(replay_admission_transcript_first_failed_outcome(&transcript), outcome);
        }
    }

    #[test]
    fn signer_algorithm_uses_verified_admission() {
        assert!(signer_algorithm_is_valid("EdDSA"));
        assert!(!signer_algorithm_is_valid("ES256"));
        assert!(!signer_algorithm_is_valid("eddsa"));
        assert!(!signer_algorithm_is_valid("EdDSA\0"));
    }

    #[test]
    fn parsed_claims_presence_uses_verified_admission() {
        assert!(parsed_claims_presence_is_valid(VerifiedParsedClaimsPresence {
            issuer_present: true,
            audience_present: true,
            attenuations_present: true,
            proofs_present: true,
            expiration_present: true,
            not_before_present: true,
        }));
        assert!(!parsed_claims_presence_is_valid(VerifiedParsedClaimsPresence {
            issuer_present: false,
            audience_present: true,
            attenuations_present: true,
            proofs_present: true,
            expiration_present: true,
            not_before_present: true,
        }));
        assert!(!parsed_claims_presence_is_valid(VerifiedParsedClaimsPresence {
            issuer_present: true,
            audience_present: true,
            attenuations_present: true,
            proofs_present: false,
            expiration_present: true,
            not_before_present: true,
        }));
        assert!(!parsed_claims_presence_is_valid(VerifiedParsedClaimsPresence {
            issuer_present: true,
            audience_present: true,
            attenuations_present: true,
            proofs_present: true,
            expiration_present: false,
            not_before_present: true,
        }));
    }

    #[test]
    fn parsed_did_claims_use_verified_admission() {
        let accepted = VerifiedParsedDidClaims {
            issuer: VerifiedParsedDidClaim {
                present: true,
                non_empty: true,
                trimmed: true,
            },
            audience: VerifiedParsedDidClaim {
                present: true,
                non_empty: true,
                trimmed: true,
            },
        };
        assert!(parsed_did_claims_are_valid(accepted));

        let mut missing_issuer = accepted;
        missing_issuer.issuer.present = false;
        assert!(!parsed_did_claims_are_valid(missing_issuer));

        let mut empty_audience = accepted;
        empty_audience.audience.non_empty = false;
        assert!(!parsed_did_claims_are_valid(empty_audience));

        let mut untrimmed_issuer = accepted;
        untrimmed_issuer.issuer.trimmed = false;
        assert!(!parsed_did_claims_are_valid(untrimmed_issuer));
    }

    #[test]
    fn parsed_temporal_order_uses_verified_admission() {
        assert!(parsed_temporal_order_is_valid(VerifiedParsedTemporalOrder {
            not_before_seconds: 1,
            expires_at_seconds: 2,
        }));
        assert!(!parsed_temporal_order_is_valid(VerifiedParsedTemporalOrder {
            not_before_seconds: 2,
            expires_at_seconds: 2,
        }));
        assert!(!parsed_temporal_order_is_valid(VerifiedParsedTemporalOrder {
            not_before_seconds: 3,
            expires_at_seconds: 2,
        }));
    }

    #[test]
    fn parsed_proof_reference_list_shape_uses_verified_admission() {
        let accepted = super::VerifiedParsedProofReferenceListShape {
            entries: vec![super::VerifiedParsedProofReferenceEntryShape {
                encoded_non_empty: true,
                encoded_base64url_no_pad: true,
                decoded_non_empty: true,
            }],
        };
        assert!(super::parsed_proof_reference_list_shape_is_valid(&accepted));

        let rejected = super::VerifiedParsedProofReferenceListShape {
            entries: vec![super::VerifiedParsedProofReferenceEntryShape {
                encoded_non_empty: true,
                encoded_base64url_no_pad: false,
                decoded_non_empty: true,
            }],
        };
        assert!(!super::parsed_proof_reference_list_shape_is_valid(&rejected));
    }

    #[test]
    fn proof_reference_collection_uses_verified_admission() {
        assert!(super::proof_reference_collection_is_valid(super::VerifiedProofReferenceCollection {
            count: 2,
            max_count: 2,
            all_references_non_empty: true,
        }));
        assert!(!super::proof_reference_collection_is_valid(super::VerifiedProofReferenceCollection {
            count: 3,
            max_count: 2,
            all_references_non_empty: true,
        }));
        assert!(!super::proof_reference_collection_is_valid(super::VerifiedProofReferenceCollection {
            count: 1,
            max_count: 2,
            all_references_non_empty: false,
        }));
    }

    #[test]
    fn parsed_capability_list_shape_uses_verified_admission() {
        let accepted = super::VerifiedParsedCapabilityListShape {
            capabilities: vec![super::VerifiedParsedCapabilityShape {
                resource_present: true,
                ability_present: true,
            }],
        };
        assert!(super::parsed_capability_list_shape_is_valid(&accepted));

        let rejected = super::VerifiedParsedCapabilityListShape {
            capabilities: vec![super::VerifiedParsedCapabilityShape {
                resource_present: true,
                ability_present: false,
            }],
        };
        assert!(!super::parsed_capability_list_shape_is_valid(&rejected));
    }

    #[test]
    fn parsed_caveat_list_shape_uses_verified_admission() {
        let accepted = super::VerifiedParsedCaveatListShape {
            caveats: vec![super::VerifiedParsedCaveatShape {
                domain_present: true,
                caveat_type_present: true,
                key_present: true,
                payload_present: true,
            }],
        };
        assert!(super::parsed_caveat_list_shape_is_valid(&accepted));

        let rejected = super::VerifiedParsedCaveatListShape {
            caveats: vec![super::VerifiedParsedCaveatShape {
                domain_present: true,
                caveat_type_present: true,
                key_present: false,
                payload_present: true,
            }],
        };
        assert!(!super::parsed_caveat_list_shape_is_valid(&rejected));
    }

    #[test]
    fn parsed_caveat_payloads_use_verified_admission() {
        let accepted = super::VerifiedParsedCaveatPayloadList {
            payloads: vec![super::VerifiedParsedCaveatPayloadFact {
                payload_present: true,
                encoded_non_empty: true,
                encoded_base64url_no_pad: true,
                decoded_non_empty: true,
            }],
        };
        assert!(super::parsed_caveat_payloads_are_valid(&accepted));

        let invalid_encoding = super::VerifiedParsedCaveatPayloadList {
            payloads: vec![super::VerifiedParsedCaveatPayloadFact {
                payload_present: true,
                encoded_non_empty: true,
                encoded_base64url_no_pad: false,
                decoded_non_empty: true,
            }],
        };
        assert!(!super::parsed_caveat_payloads_are_valid(&invalid_encoding));

        let empty_decoded = super::VerifiedParsedCaveatPayloadList {
            payloads: vec![super::VerifiedParsedCaveatPayloadFact {
                payload_present: true,
                encoded_non_empty: true,
                encoded_base64url_no_pad: true,
                decoded_non_empty: false,
            }],
        };
        assert!(!super::parsed_caveat_payloads_are_valid(&empty_decoded));
    }

    #[test]
    fn proof_store_result_and_revocation_check_use_verified_admission() {
        assert!(super::proof_store_result_is_valid(super::VerifiedProofStoreResultFact {
            reference_non_empty: true,
            backend_succeeded: true,
            compact_token_present: true,
        }));
        assert!(!super::proof_store_result_is_valid(super::VerifiedProofStoreResultFact {
            reference_non_empty: true,
            backend_succeeded: false,
            compact_token_present: true,
        }));
        assert!(super::revocation_check_is_valid(super::VerifiedRevocationCheckFact {
            reference_non_empty: true,
            backend_succeeded: true,
            not_revoked: true,
        }));
        assert!(!super::revocation_check_is_valid(super::VerifiedRevocationCheckFact {
            reference_non_empty: true,
            backend_succeeded: true,
            not_revoked: false,
        }));
    }

    #[test]
    fn caveat_preservation_uses_verified_admission() {
        let preservation = VerifiedCaveatPreservation {
            parent_caveat_ids: vec![b"a".to_vec(), b"b".to_vec()],
            child_caveat_ids: vec![b"extra".to_vec(), b"b".to_vec(), b"a".to_vec()],
        };
        assert!(caveat_preservation_is_valid(&preservation));

        let missing = VerifiedCaveatPreservation {
            parent_caveat_ids: vec![b"a".to_vec(), b"b".to_vec()],
            child_caveat_ids: vec![b"a".to_vec()],
        };
        assert!(!caveat_preservation_is_valid(&missing));

        let changed_bytes = VerifiedCaveatPreservation {
            parent_caveat_ids: vec![b"a".to_vec()],
            child_caveat_ids: vec![b"A".to_vec()],
        };
        assert!(!caveat_preservation_is_valid(&changed_bytes));
    }

    #[test]
    fn caveat_evaluation_transcript_uses_verified_admission() {
        let accepted = VerifiedCaveatEvaluationTranscript {
            facts: vec![
                VerifiedCaveatEvaluationFact {
                    policy_present: true,
                    satisfied: true,
                },
                VerifiedCaveatEvaluationFact {
                    policy_present: true,
                    satisfied: true,
                },
            ],
        };
        assert!(caveat_evaluation_transcript_accepts(&accepted));
        assert_eq!(caveat_evaluation_first_failed_index(&accepted), accepted.facts.len());

        let missing = VerifiedCaveatEvaluationTranscript {
            facts: vec![
                VerifiedCaveatEvaluationFact {
                    policy_present: true,
                    satisfied: true,
                },
                VerifiedCaveatEvaluationFact {
                    policy_present: false,
                    satisfied: false,
                },
            ],
        };
        assert!(!caveat_evaluation_transcript_accepts(&missing));
        assert_eq!(caveat_evaluation_first_failed_index(&missing), 1);

        let rejected = VerifiedCaveatEvaluationTranscript {
            facts: vec![VerifiedCaveatEvaluationFact {
                policy_present: true,
                satisfied: false,
            }],
        };
        assert!(!caveat_evaluation_transcript_accepts(&rejected));
        assert_eq!(caveat_evaluation_first_failed_index(&rejected), 0);
    }

    #[test]
    fn proof_reference_digest_uses_verified_admission() {
        let digest = VerifiedProofReferenceDigest { bytes: vec![42u8; 32] };
        assert!(proof_reference_digest_is_valid(digest.bytes.as_slice(), &digest));

        let short_digest = VerifiedProofReferenceDigest { bytes: vec![42u8; 31] };
        assert!(!proof_reference_digest_is_valid(short_digest.bytes.as_slice(), &short_digest));

        let mismatched_reference = vec![7u8; 32];
        assert!(!proof_reference_digest_is_valid(mismatched_reference.as_slice(), &digest));
    }

    #[test]
    fn verification_stage_transcript_accepts_all_successful_stages() {
        let transcript = accepting_stage_transcript();
        let outcome = verification_stage_transcript_outcome(&transcript);
        assert!(outcome.accepted);
        assert_eq!(outcome.first_failed_stage, VerifiedVerificationStage::Accepted);
    }

    #[test]
    fn verification_stage_transcript_reports_first_failure() {
        let mut transcript = accepting_stage_transcript();
        transcript.signature_valid = false;
        transcript.proofs_loaded = false;
        let outcome = verification_stage_transcript_outcome(&transcript);
        assert!(!outcome.accepted);
        assert_eq!(outcome.first_failed_stage, VerifiedVerificationStage::Signature);

        let mut transcript = accepting_stage_transcript();
        transcript.proofs_loaded = false;
        let outcome = verification_stage_transcript_outcome(&transcript);
        assert!(!outcome.accepted);
        assert_eq!(outcome.first_failed_stage, VerifiedVerificationStage::ProofLoad);
    }

    fn accepting_stage_transcript() -> VerifiedVerificationStageTranscript {
        VerifiedVerificationStageTranscript {
            parse_ok: true,
            structural_ok: true,
            key_resolved: true,
            signature_valid: true,
            envelope_valid: true,
            revocation_clear: true,
            proofs_loaded: true,
            proof_chain_valid: true,
            authorized: true,
        }
    }

    #[test]
    fn verified_capability_contains_narrower_resource_and_globbed_ability() {
        let parent = VerifiedCapability::new(VALID_RESOURCE.to_owned(), VALID_ABILITY_GLOB.to_owned())
            .expect("parent capability should validate");
        let child = VerifiedCapability::new(VALID_RESOURCE_CHILD.to_owned(), VALID_ABILITY_CHILD.to_owned())
            .expect("child capability should validate");

        assert!(parent.contains(&child));
        assert!(!child.contains(&parent));
    }

    #[test]
    fn verified_capability_rejects_other_namespace() {
        let parent = VerifiedCapability::new(VALID_RESOURCE.to_owned(), VALID_ABILITY_GLOB.to_owned())
            .expect("parent capability should validate");
        let authorized = parent
            .authorizes(VALID_RESOURCE_CHILD, VALID_ABILITY_OTHER_NAMESPACE)
            .expect("requested capability should validate");

        assert!(!authorized);
    }

    #[test]
    fn verified_capability_reports_concrete_ability_admission() {
        let wildcard = VerifiedCapability::new(VALID_RESOURCE.to_owned(), VALID_ABILITY_GLOB.to_owned())
            .expect("wildcard delegated capability should validate");
        let concrete = VerifiedCapability::new(VALID_RESOURCE_CHILD.to_owned(), VALID_ABILITY_CHILD.to_owned())
            .expect("concrete requested capability should validate");

        assert!(wildcard.has_wildcard_segment());
        assert!(!wildcard.is_concrete());
        assert!(!concrete.has_wildcard_segment());
        assert!(concrete.is_concrete());
    }

    #[test]
    fn verified_capability_rejects_embedded_wildcard_segment() {
        let error = VerifiedCapability::new(VALID_RESOURCE.to_owned(), "store*/add".to_owned())
            .expect_err("embedded wildcard should fail core validation");

        assert!(matches!(error, VerifiedPrimitiveError::InvalidCapability { .. }));
    }

    #[test]
    fn composes_time_window_chain_by_intersection() {
        let windows = [
            TokenTimeWindow {
                not_before_seconds: WINDOW_ONE_NOT_BEFORE_SECONDS,
                expires_at_seconds: WINDOW_ONE_EXPIRES_AT_SECONDS,
            },
            TokenTimeWindow {
                not_before_seconds: WINDOW_TWO_NOT_BEFORE_SECONDS,
                expires_at_seconds: WINDOW_TWO_EXPIRES_AT_SECONDS,
            },
        ];
        let composed = compose_time_window_chain(&windows).expect("window chain should compose");

        assert_eq!(composed.not_before_seconds, WINDOW_TWO_NOT_BEFORE_SECONDS);
        assert_eq!(composed.expires_at_seconds, WINDOW_TWO_EXPIRES_AT_SECONDS);
    }

    #[test]
    fn rejects_empty_time_window_chain() {
        let error = compose_time_window_chain(&[]).expect_err("empty chain should fail");

        assert_eq!(error, VerifiedPrimitiveError::EmptyTimeWindowChain);
    }

    #[test]
    fn token_time_valid_respects_verified_clock_skew() {
        assert!(token_time_valid(ISSUED_AT_SECONDS, EXPIRES_AT_SECONDS, FRESH_NOW_SECONDS));
        assert!(token_time_valid(ISSUED_AT_SECONDS, EXPIRES_AT_SECONDS, SKEW_VALID_NOW_SECONDS));
        assert!(!token_time_valid(ISSUED_AT_SECONDS, EXPIRES_AT_SECONDS, EXPIRED_NOW_SECONDS));
    }

    #[test]
    fn token_refresh_threshold_matches_verified_logic() {
        assert!(!token_needs_refresh(ISSUED_AT_SECONDS, EXPIRES_AT_SECONDS, FRESH_NOW_SECONDS));
        assert!(token_needs_refresh(ISSUED_AT_SECONDS, EXPIRES_AT_SECONDS, REFRESH_NOW_SECONDS));
    }

    #[test]
    fn verified_attenuation_chain_accepts_narrower_resource() {
        let parent = vec![valid_capability(VALID_RESOURCE, VALID_ABILITY_CHILD)];
        let child = vec![valid_capability(VALID_RESOURCE_CHILD, VALID_ABILITY_CHILD)];
        let valid = attenuation_chain_is_valid(parent.as_slice(), child.as_slice())
            .expect("attenuation chain should validate capability inputs");

        assert!(valid);
    }

    #[test]
    fn verified_attenuation_chain_rejects_ability_change() {
        let parent = vec![valid_capability(VALID_RESOURCE, VALID_ABILITY_CHILD)];
        let child = vec![valid_capability(VALID_RESOURCE_CHILD, VALID_ABILITY_OTHER_NAMESPACE)];
        let valid = attenuation_chain_is_valid(parent.as_slice(), child.as_slice())
            .expect("attenuation chain should validate capability inputs");

        assert!(!valid);
    }

    #[test]
    fn capability_set_authorizes_exact_resource_prefix_request() {
        let capabilities = vec![
            valid_capability(VALID_RESOURCE, VALID_ABILITY_CHILD),
            valid_capability("resource:other", VALID_ABILITY_OTHER_NAMESPACE),
        ];

        let valid = capability_set_authorizes(capabilities.as_slice(), VALID_RESOURCE_CHILD, VALID_ABILITY_CHILD)
            .expect("capability-set inputs should validate");

        assert!(valid);
    }

    #[test]
    fn capability_set_authorizer_rejects_escalation_or_wildcard_subsumption() {
        let capabilities = vec![valid_capability(VALID_RESOURCE, VALID_ABILITY_GLOB)];

        assert!(
            !capability_set_authorizes(capabilities.as_slice(), VALID_RESOURCE_CHILD, VALID_ABILITY_CHILD)
                .expect("wildcard capability-set inputs should validate")
        );
        assert!(
            !capability_set_authorizes(capabilities.as_slice(), "resource:other/child", VALID_ABILITY_GLOB)
                .expect("resource-escalation inputs should validate")
        );
    }

    #[test]
    fn pins_byte_prefix_scope_admits_nested_and_sibling_resources() {
        // r[ucan.scope.fixtures]
        // This fixture pins the byte-prefix containment rule: a scope
        // covers every string that starts with its bytes.
        let capabilities = vec![valid_capability(SCOPE_RESOURCE, SCOPE_ABILITY)];

        let nested = capability_set_authorizes(capabilities.as_slice(), SCOPE_NESTED_RESOURCE, SCOPE_ABILITY)
            .expect("capability-set inputs should validate");
        assert!(nested, "scope must cover the nested delimited resource");

        let sibling =
            capability_set_authorizes(capabilities.as_slice(), SCOPE_SIBLING_OVERMATCH_RESOURCE, SCOPE_ABILITY)
                .expect("capability-set inputs should validate");
        assert!(sibling, "scope must cover the sibling string that shares its bytes");
    }

    #[test]
    fn pins_byte_prefix_scope_rejects_unrelated_resource() {
        // r[ucan.scope.fixtures]
        // This fixture pins the rejection side of the containment rule: a
        // resource that does not start with the scope bytes is not covered.
        let capabilities = vec![valid_capability(SCOPE_RESOURCE, SCOPE_ABILITY)];

        let unrelated = capability_set_authorizes(capabilities.as_slice(), SCOPE_UNRELATED_RESOURCE, SCOPE_ABILITY)
            .expect("capability-set inputs should validate");
        assert!(!unrelated, "scope must reject a resource that does not share its bytes");
    }

    #[test]
    fn proof_link_uses_verified_chain_link_semantics() {
        let parent_audience = "did:key:z6MkparentAudience";
        let child_issuer = parent_audience;
        let wrong_child_issuer = "did:key:z6MkotherIssuer";

        assert!(proof_link_is_valid(parent_audience, child_issuer));
        assert!(!proof_link_is_valid(parent_audience, wrong_child_issuer));
    }

    #[test]
    fn structural_chain_uses_verified_chain_composition() {
        let parent = chain_entry(
            "did:key:z6Mkissuer",
            "did:key:z6Mkdelegate",
            VALID_RESOURCE,
            VALID_ABILITY_CHILD,
            100,
            400,
            true,
        );
        let child = chain_entry(
            "did:key:z6Mkdelegate",
            "did:key:z6Mkleaf",
            VALID_RESOURCE_CHILD,
            VALID_ABILITY_CHILD,
            150,
            350,
            true,
        );
        let valid = structural_chain_is_valid(&[parent.clone(), child.clone()], 200, 0)
            .expect("structural chain inputs should validate");

        assert!(valid);

        let broken_link = chain_entry(
            "did:key:z6Mkwrong",
            "did:key:z6Mkleaf",
            VALID_RESOURCE_CHILD,
            VALID_ABILITY_CHILD,
            150,
            350,
            true,
        );
        let valid = structural_chain_is_valid(&[parent.clone(), broken_link], 200, 0)
            .expect("structural chain inputs should validate");
        assert!(!valid);

        let ability_escalation = chain_entry(
            "did:key:z6Mkdelegate",
            "did:key:z6Mkleaf",
            VALID_RESOURCE_CHILD,
            VALID_ABILITY_OTHER_NAMESPACE,
            150,
            350,
            true,
        );
        let valid = structural_chain_is_valid(&[parent, ability_escalation], 200, 0)
            .expect("structural chain inputs should validate");
        assert!(!valid);

        let valid = structural_chain_is_valid(&[child], 500, 0).expect("structural chain inputs should validate");
        assert!(!valid);
    }

    #[test]
    fn structural_chain_not_revoked_uses_verified_revocation_composition() {
        let parent = chain_entry(
            "did:key:z6Mkissuer",
            "did:key:z6Mkdelegate",
            VALID_RESOURCE,
            VALID_ABILITY_CHILD,
            100,
            400,
            true,
        );
        let child = chain_entry(
            "did:key:z6Mkdelegate",
            "did:key:z6Mkleaf",
            VALID_RESOURCE_CHILD,
            VALID_ABILITY_CHILD,
            150,
            350,
            true,
        );
        let chain = vec![parent.clone(), child];
        let references = vec![b"parent-proof".to_vec(), b"child-proof".to_vec()];
        let revoked = vec![b"other-proof".to_vec()];

        let valid = structural_chain_not_revoked_is_valid(&chain, &references, &revoked, 200, 0)
            .expect("chain inputs should validate");
        assert!(valid);

        let revoked = vec![b"child-proof".to_vec()];
        let valid = structural_chain_not_revoked_is_valid(&chain, &references, &revoked, 200, 0)
            .expect("chain inputs should validate");
        assert!(!valid);

        let missing_reference = vec![b"parent-proof".to_vec()];
        let valid = structural_chain_not_revoked_is_valid(&chain, &missing_reference, &[], 200, 0)
            .expect("chain inputs should validate");
        assert!(!valid);

        let broken_child = chain_entry(
            "did:key:z6Mkwrong",
            "did:key:z6Mkleaf",
            VALID_RESOURCE_CHILD,
            VALID_ABILITY_CHILD,
            150,
            350,
            true,
        );
        let valid = structural_chain_not_revoked_is_valid(&[parent, broken_child], &references, &[], 200, 0)
            .expect("chain inputs should validate");
        assert!(!valid);
    }

    #[test]
    fn invocation_chain_uses_verified_authorization_composition() {
        let parent = chain_entry(
            "did:key:z6Mkissuer",
            "did:key:z6Mkdelegate",
            VALID_RESOURCE,
            VALID_ABILITY_CHILD,
            100,
            400,
            true,
        );
        let child = chain_entry(
            "did:key:z6Mkdelegate",
            "did:key:z6Mkleaf",
            VALID_RESOURCE_CHILD,
            VALID_ABILITY_CHILD,
            150,
            350,
            true,
        );
        let chain = vec![parent.clone(), child];
        let references = vec![b"parent-proof".to_vec(), b"child-proof".to_vec()];
        let request = invocation_chain_request(
            b"child-proof",
            "did:key:z6Mkleaf",
            "did:key:z6Mkexample/photos/2026",
            VALID_ABILITY_CHILD,
            0,
        );

        let valid = invocation_chain_is_authorized(&chain, &references, &[], &request, 200, 0)
            .expect("invocation chain inputs should validate");
        assert!(valid);

        let revoked = vec![b"child-proof".to_vec()];
        let valid = invocation_chain_is_authorized(&chain, &references, &revoked, &request, 200, 0)
            .expect("invocation chain inputs should validate");
        assert!(!valid);

        let broken_child = chain_entry(
            "did:key:z6Mkwrong",
            "did:key:z6Mkleaf",
            VALID_RESOURCE_CHILD,
            VALID_ABILITY_CHILD,
            150,
            350,
            true,
        );
        let valid = invocation_chain_is_authorized(&[parent, broken_child], &references, &[], &request, 200, 0)
            .expect("invocation chain inputs should validate");
        assert!(!valid);

        let wrong_reference = invocation_chain_request(
            b"other-proof",
            "did:key:z6Mkleaf",
            "did:key:z6Mkexample/photos/2026",
            VALID_ABILITY_CHILD,
            0,
        );
        let valid = invocation_chain_is_authorized(&chain, &references, &[], &wrong_reference, 200, 0)
            .expect("invocation chain inputs should validate");
        assert!(!valid);

        let escalated_resource = invocation_chain_request(
            b"child-proof",
            "did:key:z6Mkleaf",
            "did:key:z6Mkother/photos",
            VALID_ABILITY_CHILD,
            0,
        );
        let valid = invocation_chain_is_authorized(&chain, &references, &[], &escalated_resource, 200, 0)
            .expect("invocation chain inputs should validate");
        assert!(!valid);

        let escalated_ability = invocation_chain_request(
            b"child-proof",
            "did:key:z6Mkleaf",
            "did:key:z6Mkexample/photos/2026",
            VALID_ABILITY_OTHER_NAMESPACE,
            0,
        );
        let valid = invocation_chain_is_authorized(&chain, &references, &[], &escalated_ability, 200, 0)
            .expect("invocation chain inputs should validate");
        assert!(!valid);

        let bad_index = invocation_chain_request(
            b"child-proof",
            "did:key:z6Mkleaf",
            "did:key:z6Mkexample/photos/2026",
            VALID_ABILITY_CHILD,
            1,
        );
        let valid = invocation_chain_is_authorized(&chain, &references, &[], &bad_index, 200, 0)
            .expect("invocation chain inputs should validate");
        assert!(!valid);
    }

    #[test]
    fn web_of_trust_principal_digest_uses_verified_principal_derivation() {
        let key_bytes = b"example public key bytes";
        let digest = web_of_trust_principal_digest(key_bytes);
        let repeated = web_of_trust_principal_digest(key_bytes);
        let other = web_of_trust_principal_digest(b"different public key bytes");

        assert_eq!(digest, repeated);
        assert_eq!(digest.len(), verified_wot_principal::PRINCIPAL_ID_LEN);
        assert_ne!(digest, other);
    }

    #[test]
    fn web_of_trust_principal_digest_admission_uses_verified_constructor() {
        let digest = web_of_trust_principal_digest(b"example public key bytes");
        let mut short = digest.clone();
        short.pop();
        let mut long = digest.clone();
        long.push(0);

        assert!(web_of_trust_principal_digest_is_valid(&digest));
        assert!(!web_of_trust_principal_digest_is_valid(&short));
        assert!(!web_of_trust_principal_digest_is_valid(&long));
    }

    #[test]
    fn typed_caveat_helpers_use_verified_caveat_semantics() {
        let context = VerifiedWebOfTrustCaveatContext {
            now_seconds: 5,
            count_in_window: 2,
        };
        let active = VerifiedWebOfTrustCaveat::TimeBound { not_after_seconds: 10 };
        let expired = VerifiedWebOfTrustCaveat::TimeBound { not_after_seconds: 4 };
        let rate_limit = VerifiedWebOfTrustCaveat::RateLimit {
            max_count: 3,
            window_seconds: 60,
        };
        let exceeded_rate_limit = VerifiedWebOfTrustCaveat::RateLimit {
            max_count: 1,
            window_seconds: 60,
        };

        assert!(typed_caveat_is_satisfied(&active, &context));
        assert!(!typed_caveat_is_satisfied(&expired, &context));
        assert!(typed_caveat_set_is_satisfied(&[active, rate_limit], &context));
        assert!(!typed_caveat_set_is_satisfied(&[exceeded_rate_limit], &context));
    }

    #[test]
    fn web_of_trust_evidence_helpers_route_to_verified_primitives() {
        let root_key = vec![1, 2, 3];
        let subject_key = vec![7, 8, 9];
        let attestation = signed_wot_attestation(root_key.clone(), subject_key.clone(), 1, 0, 20, 1);
        let revocation = signed_wot_family_revocation(root_key.clone(), subject_key.clone(), 1, 5);

        let attestation_payload = web_of_trust_attestation_payload_bytes(&attestation)
            .expect("attestation input should validate")
            .expect("valid attestation payload should encode");
        assert!(!attestation_payload.is_empty());
        assert_eq!(
            web_of_trust_attestation_payload_bytes(&attestation)
                .expect("attestation input should validate")
                .expect("valid attestation payload should encode"),
            attestation_payload
        );
        assert!(web_of_trust_attestation_signature_is_valid(&attestation).expect("attestation input should validate"));
        assert!(
            web_of_trust_attestation_is_active_at(&attestation, &[], 4).expect("attestation input should validate")
        );
        assert!(
            !web_of_trust_attestation_is_active_at(&attestation, std::slice::from_ref(&revocation), 6)
                .expect("revocation input should validate")
        );

        let revocation_payload = web_of_trust_revocation_payload_bytes(&revocation)
            .expect("revocation input should validate")
            .expect("valid revocation payload should encode");
        assert!(!revocation_payload.is_empty());
        assert_eq!(
            web_of_trust_revocation_payload_bytes(&revocation)
                .expect("revocation input should validate")
                .expect("valid revocation payload should encode"),
            revocation_payload
        );
        assert!(web_of_trust_revocation_signature_is_valid(&revocation).expect("revocation input should validate"));
        assert!(
            web_of_trust_revocation_matches_attestation(&revocation, &attestation)
                .expect("evidence input should validate")
        );
        assert!(
            web_of_trust_revocation_is_effective_at(&revocation, &attestation, 5)
                .expect("evidence input should validate")
        );
        assert!(
            !web_of_trust_revocation_is_effective_at(&revocation, &attestation, 4)
                .expect("evidence input should validate")
        );

        let threshold = web_of_trust_threshold_accepts(
            &[principal_digest(&root_key)],
            std::slice::from_ref(&attestation),
            &[],
            &principal_digest(&subject_key),
            1,
            1,
            4,
        )
        .expect("threshold evidence should validate");
        assert!(threshold);

        let mut invalid_digest = attestation;
        invalid_digest.subject_digest = vec![1, 2, 3];
        let error =
            web_of_trust_attestation_signature_is_valid(&invalid_digest).expect_err("short digest should reject");
        assert_eq!(error, VerifiedPrimitiveError::InvalidPrincipalDigestLength { actual: 3 });
    }

    #[test]
    fn web_of_trust_ucan_authorization_uses_verified_facade() {
        let root_key = vec![1, 2, 3];
        let subject_key = vec![7, 8, 9];
        let attestation = signed_wot_attestation(root_key.clone(), subject_key.clone(), 1, 0, 20, 1);
        let request = wot_authorization_request(
            vec![principal_digest(&root_key)],
            vec![attestation],
            Vec::new(),
            principal_digest(&subject_key),
            1,
        );

        let decision = web_of_trust_ucan_authorization(&request).expect("WOT UCAN inputs should validate");
        assert_eq!(decision, VerifiedWebOfTrustAuthorization::Authorized);

        let mut denied_fact = request.clone();
        denied_fact.did_cid_linked = false;
        let decision = web_of_trust_ucan_authorization(&denied_fact).expect("WOT UCAN inputs should validate");
        assert_eq!(decision, VerifiedWebOfTrustAuthorization::Denied);

        let mut unsatisfied_caveat = request.clone();
        unsatisfied_caveat.active_caveats = vec![VerifiedWebOfTrustCaveat::TimeBound { not_after_seconds: 1 }];
        let decision = web_of_trust_ucan_authorization(&unsatisfied_caveat).expect("WOT UCAN inputs should validate");
        assert_eq!(decision, VerifiedWebOfTrustAuthorization::Denied);

        let mut invalid_digest = request;
        invalid_digest.subject_principal_digest = vec![1, 2, 3];
        let error = web_of_trust_ucan_authorization(&invalid_digest).expect_err("short digest should reject");
        assert_eq!(error, VerifiedPrimitiveError::InvalidPrincipalDigestLength { actual: 3 });
    }

    #[test]
    fn web_of_trust_ucan_authorization_denies_revoked_or_under_threshold_trust() {
        let alpha_root_key = vec![1, 2, 3];
        let beta_root_key = vec![4, 5, 6];
        let subject_key = vec![7, 8, 9];
        let attestation_a = signed_wot_attestation(alpha_root_key.clone(), subject_key.clone(), 1, 0, 20, 1);
        let attestation_b = signed_wot_attestation(beta_root_key.clone(), subject_key.clone(), 1, 0, 20, 2);
        let roots = vec![principal_digest(&alpha_root_key), principal_digest(&beta_root_key)];
        let subject = principal_digest(&subject_key);

        let under_threshold =
            wot_authorization_request(roots.clone(), vec![attestation_a.clone()], Vec::new(), subject.clone(), 2);
        let decision = web_of_trust_ucan_authorization(&under_threshold).expect("WOT UCAN inputs should validate");
        assert_eq!(decision, VerifiedWebOfTrustAuthorization::Denied);

        let revoked = wot_authorization_request(
            roots,
            vec![attestation_a, attestation_b],
            vec![signed_wot_family_revocation(beta_root_key, subject_key, 1, 5)],
            subject,
            2,
        );
        let decision = web_of_trust_ucan_authorization(&revoked).expect("WOT UCAN inputs should validate");
        assert_eq!(decision, VerifiedWebOfTrustAuthorization::Denied);
    }

    #[test]
    fn revocation_reference_lookup_uses_verified_set_semantics() {
        let revoked = vec![b"root-proof".to_vec(), b"leaf-proof".to_vec()];

        assert!(revocation_reference_is_listed(&revoked, b"leaf-proof"));
        assert!(!revocation_reference_is_listed(&revoked, b"other-proof"));
    }

    #[test]
    fn invocation_request_uses_verified_invocation_semantics() {
        let audience = "did:key:z6Mkaudience";

        assert!(invocation_request_is_authorized(
            audience,
            audience,
            "did:key:z6Mkresource/photos",
            "store/add",
            "did:key:z6Mkresource",
            "store/add",
        ));
        assert!(!invocation_request_is_authorized(
            "did:key:z6Mkwrong",
            audience,
            "did:key:z6Mkresource/photos",
            "store/add",
            "did:key:z6Mkresource",
            "store/add",
        ));
        assert!(!invocation_request_is_authorized(
            audience,
            audience,
            "did:key:z6Mkother/photos",
            "store/add",
            "did:key:z6Mkresource",
            "store/add",
        ));
        assert!(!invocation_request_is_authorized(
            audience,
            audience,
            "did:key:z6Mkresource/photos",
            "store/remove",
            "did:key:z6Mkresource",
            "store/add",
        ));
    }

    #[test]
    fn proof_depth_budget_uses_verified_delegation_depth_semantics() {
        assert!(proof_depth_within_budget(0, 0));
        assert!(proof_depth_within_budget(3, 3));
        assert!(!proof_depth_within_budget(4, 3));
        assert!(!proof_depth_within_budget(usize::MAX, usize::MAX));
    }

    #[test]
    fn receipt_execution_uses_verified_receipt_semantics() {
        let invoker = "did:key:z6Mkaudience";

        assert!(receipt_execution_is_valid(invoker, invoker, 1_000, 1_000));
        assert!(receipt_execution_is_valid(invoker, invoker, 1_000, 1_001));
        assert!(!receipt_execution_is_valid(invoker, "did:key:z6Mkwrong", 1_000, 1_001));
        assert!(!receipt_execution_is_valid(invoker, invoker, 1_001, 1_000));
    }

    #[test]
    fn validates_token_envelope_with_verified_logic() {
        let envelope = TokenEnvelopeInput {
            issuer_did: "did:key:zIssuer".to_owned(),
            audience_did: "did:key:zAudience".to_owned(),
            capabilities: vec![valid_capability(VALID_RESOURCE, VALID_ABILITY_CHILD)],
            proofs: Vec::new(),
            expires_at_seconds: EXPIRES_AT_SECONDS,
            not_before_seconds: ISSUED_AT_SECONDS,
        };
        let result = validate_token_envelope(&envelope, true, NOW_SECONDS, super::DEFAULT_CLOCK_SKEW_SECONDS);

        assert!(result.is_ok());
    }

    #[test]
    fn token_envelope_rejects_missing_issuer() {
        let envelope = TokenEnvelopeInput {
            issuer_did: String::new(),
            audience_did: "did:key:zAudience".to_owned(),
            capabilities: vec![valid_capability(VALID_RESOURCE, VALID_ABILITY_CHILD)],
            proofs: Vec::new(),
            expires_at_seconds: EXPIRES_AT_SECONDS,
            not_before_seconds: ISSUED_AT_SECONDS,
        };
        let error = validate_token_envelope(&envelope, true, NOW_SECONDS, super::DEFAULT_CLOCK_SKEW_SECONDS)
            .expect_err("missing issuer should fail");

        assert_eq!(error, VerifiedPrimitiveError::TokenValidation {
            failure: TokenValidationFailure::MissingIssuer,
        });
    }

    #[test]
    fn encodes_ed25519_did_key_with_verified_logic() {
        let public_key = derive_ed25519_public_key(&[SIGNING_KEY_BYTE; ucan_signature::SECRET_KEY_LEN])
            .expect("secret key should derive a public key");
        let did_key = encode_ed25519_did_key(&public_key).expect("public key should encode");

        assert!(did_key.starts_with(b"did:key:z"));
    }

    #[test]
    fn decodes_did_key_with_verified_logic() {
        let public_key = derive_ed25519_public_key(&[SIGNING_KEY_BYTE; ucan_signature::SECRET_KEY_LEN])
            .expect("secret key should derive a public key");
        let encoded = encode_ed25519_did_key(&public_key).expect("public key should encode");

        let decoded = decode_did_key(&encoded).expect("encoded DID key should decode");

        assert_eq!(decoded.public_key_bytes, public_key);
        assert_eq!(decoded.codec, verified_logic::ucan_did::CODEC_ED25519);
        assert_eq!(decoded.encoded, encoded);
    }

    #[test]
    fn did_key_decoder_rejects_malformed_inputs() {
        assert_eq!(decode_did_key(b"did:web:example"), None);
        assert_eq!(decode_did_key(b"did:key:z0"), None);
        assert_eq!(decode_did_key(b"did:key:z"), None);
    }

    #[test]
    fn rejects_wrong_public_key_length_for_did_encoding() {
        let error = encode_ed25519_did_key(&[SIGNING_KEY_BYTE; INVALID_LENGTH_BYTES])
            .expect_err("short public key should fail");

        assert!(matches!(error, VerifiedPrimitiveError::InvalidPublicKeyLength {
            actual: INVALID_LENGTH_BYTES,
        }));
    }

    #[test]
    fn signs_and_verifies_ed25519_messages() {
        let secret_key = [SIGNING_KEY_BYTE; ucan_signature::SECRET_KEY_LEN];
        let public_key = derive_ed25519_public_key(&secret_key).expect("public key derivation should succeed");
        let signature = sign_ed25519(&secret_key, MESSAGE).expect("signing should succeed");
        let verified = verify_ed25519_signature(&public_key, MESSAGE, &signature).expect("verification should succeed");

        assert!(verified);
    }

    #[test]
    fn proof_traversal_transcript_uses_verified_finite_traversal_model() {
        let transcript = proof_traversal_transcript();
        let budget = VerifiedProofTraversalBudget {
            max_depth: 4,
            max_proofs: 8,
            max_input_bytes: 1024,
        };

        let valid =
            proof_traversal_transcript_is_valid(&transcript, &budget).expect("proof traversal inputs should validate");
        assert!(valid);

        let mut duplicate_reference = transcript.clone();
        duplicate_reference.visited_references[1] = duplicate_reference.visited_references[0].clone();
        let valid = proof_traversal_transcript_is_valid(&duplicate_reference, &budget)
            .expect("proof traversal inputs should validate");
        assert!(!valid);

        let mut revoked = transcript;
        revoked.entries[0].revoked = true;
        let valid =
            proof_traversal_transcript_is_valid(&revoked, &budget).expect("proof traversal inputs should validate");
        assert!(!valid);
    }

    #[test]
    fn proof_traversal_transcript_rejects_link_budget_and_caveat_failures() {
        let transcript = proof_traversal_transcript();
        let budget = VerifiedProofTraversalBudget {
            max_depth: 4,
            max_proofs: 8,
            max_input_bytes: 1024,
        };

        let mut broken_link = transcript.clone();
        broken_link.entries[1].issuer_did = "did:key:zMallory".to_owned();
        let valid =
            proof_traversal_transcript_is_valid(&broken_link, &budget).expect("proof traversal inputs should validate");
        assert!(!valid);

        let tight_depth = VerifiedProofTraversalBudget { max_depth: 0, ..budget };
        let valid = proof_traversal_transcript_is_valid(&transcript, &tight_depth)
            .expect("proof traversal inputs should validate");
        assert!(!valid);

        let tight_count = VerifiedProofTraversalBudget {
            max_proofs: 1,
            ..budget
        };
        let valid = proof_traversal_transcript_is_valid(&transcript, &tight_count)
            .expect("proof traversal inputs should validate");
        assert!(!valid);

        let tight_input = VerifiedProofTraversalBudget {
            max_input_bytes: 128,
            ..budget
        };
        let valid = proof_traversal_transcript_is_valid(&transcript, &tight_input)
            .expect("proof traversal inputs should validate");
        assert!(!valid);

        let mut missing_caveat = transcript;
        missing_caveat.entries[1].caveat_ids.clear();
        let valid = proof_traversal_transcript_is_valid(&missing_caveat, &budget)
            .expect("proof traversal inputs should validate");
        assert!(!valid);
    }

    #[test]
    fn proof_traversal_transcript_rejects_overbroad_or_mismatched_targets() {
        let transcript = proof_traversal_transcript();
        let budget = VerifiedProofTraversalBudget {
            max_depth: 4,
            max_proofs: 8,
            max_input_bytes: 1024,
        };

        let mut overbroad_child = transcript.clone();
        overbroad_child.entries[1].capabilities[0] = valid_capability("did:key:zOther", "store/add");
        let valid = proof_traversal_transcript_is_valid(&overbroad_child, &budget)
            .expect("proof traversal inputs should validate");
        assert!(!valid);

        let mut mismatched_target = transcript;
        mismatched_target.target.ability = "store/remove".to_owned();
        let valid = proof_traversal_transcript_is_valid(&mismatched_target, &budget)
            .expect("proof traversal inputs should validate");
        assert!(!valid);
    }

    #[test]
    fn proof_traversal_dag_transcript_uses_verified_multi_parent_model() {
        let transcript = proof_traversal_dag_transcript();
        let budget = VerifiedProofTraversalBudget {
            max_depth: 4,
            max_proofs: 8,
            max_input_bytes: 1024,
        };

        let valid = proof_traversal_dag_transcript_is_valid(&transcript, &budget)
            .expect("proof DAG traversal inputs should validate");
        assert!(valid);

        let mut cycle = transcript.clone();
        cycle.parent_indices[1] = vec![1];
        let valid = proof_traversal_dag_transcript_is_valid(&cycle, &budget)
            .expect("proof DAG traversal inputs should validate");
        assert!(!valid);

        let mut missing_parent_caveat = transcript.clone();
        missing_parent_caveat.entries[3].caveat_ids.retain(|id| id != b"region");
        let valid = proof_traversal_dag_transcript_is_valid(&missing_parent_caveat, &budget)
            .expect("proof DAG traversal inputs should validate");
        assert!(!valid);

        let mut wrong_parent_link = transcript.clone();
        wrong_parent_link.entries[2].audience_did = "did:key:zMallory".to_owned();
        let valid = proof_traversal_dag_transcript_is_valid(&wrong_parent_link, &budget)
            .expect("proof DAG traversal inputs should validate");
        assert!(!valid);

        let mut duplicate_reference = transcript;
        duplicate_reference.visited_references[2] = duplicate_reference.visited_references[1].clone();
        let valid = proof_traversal_dag_transcript_is_valid(&duplicate_reference, &budget)
            .expect("proof DAG traversal inputs should validate");
        assert!(!valid);
    }

    #[test]
    fn ed25519_verification_rejects_wrong_message() {
        let secret_key = [SIGNING_KEY_BYTE; ucan_signature::SECRET_KEY_LEN];
        let public_key = derive_ed25519_public_key(&secret_key).expect("public key derivation should succeed");
        let signature = sign_ed25519(&secret_key, MESSAGE).expect("signing should succeed");
        let verified =
            verify_ed25519_signature(&public_key, WRONG_MESSAGE, &signature).expect("verification should succeed");

        assert!(!verified);
    }

    fn valid_capability(resource: &str, ability: &str) -> CapabilityDocument {
        CapabilityDocument::new(resource.to_owned(), ability.to_owned()).expect("capability fixture should validate")
    }

    fn invocation_chain_request(
        authorization_reference: &[u8],
        invoker_did: &str,
        resource: &str,
        ability: &str,
        leaf_capability_index: usize,
    ) -> VerifiedInvocationChainRequest {
        VerifiedInvocationChainRequest {
            authorization_reference: authorization_reference.to_vec(),
            invoker_did: invoker_did.to_owned(),
            resource: resource.to_owned(),
            ability: ability.to_owned(),
            leaf_capability_index,
        }
    }

    fn proof_traversal_entry(
        proof_reference: &[u8],
        issuer_did: &str,
        audience_did: &str,
        resource: &str,
        caveat_ids: Vec<Vec<u8>>,
    ) -> VerifiedProofTraversalEntry {
        VerifiedProofTraversalEntry {
            proof_reference: proof_reference.to_vec(),
            issuer_did: issuer_did.to_owned(),
            audience_did: audience_did.to_owned(),
            capabilities: vec![valid_capability(resource, "store/add")],
            caveat_ids,
            expires_at_seconds: EXPIRES_AT_SECONDS,
            not_before_seconds: ISSUED_AT_SECONDS,
            signature_valid: true,
            revoked: false,
        }
    }

    fn proof_traversal_transcript() -> VerifiedProofTraversalTranscript {
        let parent = proof_traversal_entry(b"root-proof", "did:key:zRoot", "did:key:zLeaf", VALID_RESOURCE, vec![
            b"geo".to_vec(),
        ]);
        let child =
            proof_traversal_entry(b"leaf-proof", "did:key:zLeaf", "did:key:zInvoker", VALID_RESOURCE_CHILD, vec![
                b"geo".to_vec(),
                b"rate".to_vec(),
            ]);
        VerifiedProofTraversalTranscript {
            entries: vec![parent, child],
            visited_references: vec![b"root-proof".to_vec(), b"leaf-proof".to_vec()],
            total_input_bytes: 256,
            target: VerifiedProofTraversalTarget {
                resource: VALID_RESOURCE_CHILD.to_owned(),
                ability: "store/add".to_owned(),
                now_seconds: NOW_SECONDS,
                tolerance_seconds: 0,
            },
        }
    }

    fn proof_traversal_dag_transcript() -> VerifiedProofTraversalDagTranscript {
        let root = proof_traversal_entry(b"root-proof", "did:key:zRoot", "did:key:zBob", VALID_RESOURCE, Vec::new());
        let proof_a = proof_traversal_entry(b"proof-a", "did:key:zBob", "did:key:zCarol", VALID_RESOURCE_CHILD, vec![
            b"geo".to_vec(),
            b"rate".to_vec(),
        ]);
        let proof_b = proof_traversal_entry(b"proof-b", "did:key:zBob", "did:key:zCarol", VALID_RESOURCE_CHILD, vec![
            b"geo".to_vec(),
            b"rate".to_vec(),
            b"region".to_vec(),
        ]);
        let leaf = proof_traversal_entry(
            b"leaf-proof",
            "did:key:zCarol",
            "did:key:zInvoker",
            "did:key:z6Mkexample/photos/grandchild",
            vec![b"geo".to_vec(), b"rate".to_vec(), b"region".to_vec(), b"ttl".to_vec()],
        );
        VerifiedProofTraversalDagTranscript {
            entries: vec![root, proof_a, proof_b, leaf],
            parent_indices: vec![vec![], vec![0], vec![0], vec![1, 2]],
            visited_references: vec![
                b"root-proof".to_vec(),
                b"proof-a".to_vec(),
                b"proof-b".to_vec(),
                b"leaf-proof".to_vec(),
            ],
            total_input_bytes: 512,
            target: VerifiedProofTraversalTarget {
                resource: "did:key:z6Mkexample/photos/grandchild/item".to_owned(),
                ability: "store/add".to_owned(),
                now_seconds: NOW_SECONDS,
                tolerance_seconds: 0,
            },
        }
    }

    fn principal_digest(key_bytes: &[u8]) -> Vec<u8> {
        web_of_trust_principal_digest(key_bytes)
    }

    fn sign_wot_bytes(domain_tag: u8, issuer_key_bytes: &[u8], payload_bytes: &[u8]) -> Vec<u8> {
        let mut signature = Vec::new();
        signature.push(domain_tag);
        let issuer_len = u32::try_from(issuer_key_bytes.len()).expect("fixture issuer key length fits u32");
        signature.extend_from_slice(&issuer_len.to_le_bytes());
        signature.extend_from_slice(issuer_key_bytes);
        signature.extend_from_slice(payload_bytes);
        signature
    }

    fn signed_wot_attestation(
        issuer_key_bytes: Vec<u8>,
        subject_key_bytes: Vec<u8>,
        role_tag: u8,
        not_before_seconds: u64,
        not_after_seconds: u64,
        sequence_no: u64,
    ) -> VerifiedWebOfTrustAttestation {
        let issuer = verified_wot_principal::principal_id_from_bytes(issuer_key_bytes.as_slice());
        let subject = verified_wot_principal::principal_id_from_bytes(subject_key_bytes.as_slice());
        let unsigned = verified_wot_attestation::Attestation {
            issuer_key_bytes: issuer_key_bytes.clone(),
            issuer,
            subject_key_bytes: subject_key_bytes.clone(),
            subject,
            role_tag,
            not_before: not_before_seconds,
            not_after: not_after_seconds,
            sequence_no,
            signature: Vec::new(),
        };
        let payload = verified_wot_attestation::attestation_payload_bytes(&unsigned)
            .expect("fixture attestation payload should encode");
        let signature = sign_wot_bytes(0xa1, issuer_key_bytes.as_slice(), payload.as_slice());
        VerifiedWebOfTrustAttestation {
            issuer_key_bytes,
            issuer_digest: unsigned.issuer.digest,
            subject_key_bytes,
            subject_digest: unsigned.subject.digest,
            role_tag,
            not_before_seconds,
            not_after_seconds,
            sequence_no,
            signature,
        }
    }

    fn signed_wot_family_revocation(
        issuer_key_bytes: Vec<u8>,
        subject_key_bytes: Vec<u8>,
        role_tag: u8,
        effective_time_seconds: u64,
    ) -> VerifiedWebOfTrustRevocation {
        let issuer = verified_wot_principal::principal_id_from_bytes(issuer_key_bytes.as_slice());
        let family_subject = verified_wot_principal::principal_id_from_bytes(subject_key_bytes.as_slice());
        let unsigned = verified_wot_revocation::Revocation {
            issuer_key_bytes: issuer_key_bytes.clone(),
            issuer,
            target_kind_tag: 0x02,
            exact_payload_target: Vec::new(),
            family_subject_key_bytes: subject_key_bytes.clone(),
            family_subject,
            family_role_tag: role_tag,
            effective_time: effective_time_seconds,
            signature: Vec::new(),
        };
        let payload = verified_wot_revocation::revocation_payload_bytes(&unsigned)
            .expect("fixture revocation payload should encode");
        let signature = sign_wot_bytes(0xa2, issuer_key_bytes.as_slice(), payload.as_slice());
        VerifiedWebOfTrustRevocation {
            issuer_key_bytes,
            issuer_digest: unsigned.issuer.digest,
            target_kind_tag: 0x02,
            exact_payload_target: Vec::new(),
            family_subject_key_bytes: subject_key_bytes,
            family_subject_digest: unsigned.family_subject.digest,
            family_role_tag: role_tag,
            effective_time_seconds,
            signature,
        }
    }

    fn wot_authorization_request(
        root_principal_digests: Vec<Vec<u8>>,
        attestations: Vec<VerifiedWebOfTrustAttestation>,
        revocations: Vec<VerifiedWebOfTrustRevocation>,
        subject_principal_digest: Vec<u8>,
        threshold: usize,
    ) -> VerifiedWebOfTrustUcanAuthorizationRequest {
        VerifiedWebOfTrustUcanAuthorizationRequest {
            token_envelope: TokenEnvelopeInput {
                issuer_did: "did:key:zIssuer".to_owned(),
                audience_did: "did:key:zAudience".to_owned(),
                capabilities: vec![valid_capability(VALID_RESOURCE, VALID_ABILITY_CHILD)],
                proofs: vec![b"bafy-proof".to_vec()],
                expires_at_seconds: 100,
                not_before_seconds: 0,
            },
            signature_valid: true,
            now_seconds: 5,
            tolerance_seconds: 0,
            ability_allowed: true,
            caveats_contained: true,
            active_caveats: vec![VerifiedWebOfTrustCaveat::TimeBound { not_after_seconds: 50 }],
            caveat_context: VerifiedWebOfTrustCaveatContext {
                now_seconds: 5,
                count_in_window: 0,
            },
            did_cid_linked: true,
            root_principal_digests,
            attestations,
            revocations,
            subject_principal_digest,
            threshold,
            depth: 1,
            trust_time_seconds: 5,
        }
    }

    fn chain_entry(
        issuer_did: &str,
        audience_did: &str,
        resource: &str,
        ability: &str,
        not_before_seconds: u64,
        expires_at_seconds: u64,
        signature_valid: bool,
    ) -> VerifiedChainEntry {
        VerifiedChainEntry {
            issuer_did: issuer_did.to_owned(),
            audience_did: audience_did.to_owned(),
            capabilities: vec![valid_capability(resource, ability)],
            expires_at_seconds,
            not_before_seconds,
            signature_valid,
        }
    }
}
