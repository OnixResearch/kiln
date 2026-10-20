#![allow(clippy::module_name_repetitions)]

use core::fmt;
use core::str::FromStr;
use std::io;
use std::io::Write;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Deserialize;
use serde::Serialize;
use zeroize::Zeroizing;

use crate::shell::CapabilityDocument;
use crate::shell::CaveatDocument;
use crate::verified::TokenEnvelopeInput;
use crate::verified::TokenTimeWindow;
use crate::verified::TokenValidationFailure;
use crate::verified::VerifiedBackendOperation;
use crate::verified::VerifiedBackendOperationFact;
use crate::verified::VerifiedBackendTranscript;
use crate::verified::VerifiedCapability;
use crate::verified::VerifiedCaveatEvaluationFact;
use crate::verified::VerifiedCaveatEvaluationTranscript;
use crate::verified::VerifiedCaveatPreservation;
use crate::verified::VerifiedParsedCapabilityListShape;
use crate::verified::VerifiedParsedCapabilityShape;
use crate::verified::VerifiedParsedCaveatListShape;
use crate::verified::VerifiedParsedCaveatPayloadFact;
use crate::verified::VerifiedParsedCaveatPayloadList;
use crate::verified::VerifiedParsedCaveatShape;
use crate::verified::VerifiedParsedClaimsPresence;
use crate::verified::VerifiedParsedDidClaim;
use crate::verified::VerifiedParsedDidClaims;
use crate::verified::VerifiedParsedProofReferenceEntryShape;
use crate::verified::VerifiedParsedProofReferenceListShape;
use crate::verified::VerifiedParsedTemporalOrder;
use crate::verified::VerifiedPrimitiveError;
use crate::verified::VerifiedProofReferenceCollection;
use crate::verified::VerifiedProofReferenceDigest;
use crate::verified::VerifiedProofStoreResultFact;
use crate::verified::VerifiedProofTraversalBudget;
use crate::verified::VerifiedProofTraversalDagTranscript;
use crate::verified::VerifiedProofTraversalEntry;
use crate::verified::VerifiedProofTraversalTarget;
use crate::verified::VerifiedReplayAdmissionFact;
use crate::verified::VerifiedReplayAdmissionOutcome;
use crate::verified::VerifiedReplayAdmissionTranscript;
use crate::verified::VerifiedRevocationCheckFact;
use crate::verified::VerifiedVerificationStage;
use crate::verified::VerifiedVerificationStageTranscript;
use crate::verified::VerifiedWebOfTrustCaveat;
use crate::verified::VerifiedWebOfTrustCaveatContext;
use crate::verified::backend_transcript_accepts;
use crate::verified::backend_transcript_first_failed_operation;
use crate::verified::caveat_evaluation_first_failed_index;
use crate::verified::caveat_evaluation_transcript_accepts;
use crate::verified::caveat_preservation_is_valid;
use crate::verified::compose_time_window_chain;
use crate::verified::derive_ed25519_public_key;
use crate::verified::encode_ed25519_did_key;
use crate::verified::parsed_capability_list_shape_is_valid;
use crate::verified::parsed_caveat_list_shape_is_valid;
use crate::verified::parsed_caveat_payloads_are_valid;
use crate::verified::parsed_claims_presence_is_valid;
use crate::verified::parsed_did_claims_are_valid;
use crate::verified::parsed_proof_reference_list_shape_is_valid;
use crate::verified::parsed_temporal_order_is_valid;
use crate::verified::proof_depth_within_budget;
use crate::verified::proof_link_is_valid;
use crate::verified::proof_reference_collection_is_valid;
use crate::verified::proof_reference_digest_is_valid;
use crate::verified::proof_store_result_is_valid;
use crate::verified::proof_traversal_dag_transcript_is_valid;
use crate::verified::replay_admission_transcript_outcome;
use crate::verified::revocation_check_is_valid;
use crate::verified::revocation_reference_is_listed;
use crate::verified::sign_ed25519;
use crate::verified::signer_algorithm_is_valid;
use crate::verified::typed_caveat_ascii_decimal_payload_is_valid;
use crate::verified::typed_caveat_is_satisfied;
use crate::verified::typed_caveat_rate_limit_payload_shape_is_valid;
use crate::verified::validate_token_envelope;
use crate::verified::verification_stage_transcript_outcome;
use crate::verified::verify_ed25519_signature;

const COMPACT_TOKEN_PART_COUNT: usize = 3;
const COMPACT_SEPARATOR: char = '.';
const COMPACT_SEPARATOR_BYTE: u8 = b'.';
const BASE64_INPUT_BLOCK_BYTES: usize = 4;
const BASE64_OUTPUT_BLOCK_BYTES: usize = 3;
const BASE64_REMAINDER_TWO: usize = 2;
const BASE64_REMAINDER_THREE: usize = 3;
const BASE64_REMAINDER_TWO_OUTPUT_BYTES: usize = 1;
const BASE64_REMAINDER_THREE_OUTPUT_BYTES: usize = 2;
const JSON_ESCAPE_BYTE: u8 = b'\\';
const TOKEN_TYPE_JWT: &str = "JWT";
const TOKEN_ALGORITHM_EDDSA: &str = "EdDSA";
const REDACTED_SECRET_MATERIAL_DEBUG: &str = "<redacted>";
const CAPABILITY_RESOURCE_FIELD: &str = "with";
const CAPABILITY_ABILITY_FIELD: &str = "can";
const PROOF_REFERENCE_DIGEST_LEN: usize = blake3::OUT_LEN;
const DEFAULT_MAX_PROOF_DEPTH: usize = 16;
const DEFAULT_MAX_PROOF_COUNT: usize = 64;
const DEFAULT_MAX_COMPACT_TOKEN_BYTES: usize = 64 * 1024;
const ZERO_CLOCK_SKEW_TOLERANCE_SECONDS: u64 = 0;
pub const ED25519_SECRET_KEY_BYTES: usize = verified_logic::ucan_signature::SECRET_KEY_LEN;
pub const ED25519_PUBLIC_KEY_BYTES: usize = verified_logic::ucan_signature::PUBLIC_KEY_LEN;
pub const ED25519_SIGNATURE_BYTES: usize = verified_logic::ucan_signature::SIGNATURE_LEN;

pub type SignerResult<T> = std::result::Result<T, SignerError>;
pub type TokenResult<T> = std::result::Result<T, TokenError>;
pub type BuilderResult<T> = std::result::Result<T, BuilderError>;
pub type TimeConversionResult<T> = std::result::Result<T, TimeConversionError>;

mod holder_invocation;
pub use holder_invocation::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuilderError {
    MissingField { field: &'static str },
}

impl fmt::Display for BuilderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingField { field } => write!(formatter, "missing required builder field {field}"),
        }
    }
}

impl std::error::Error for BuilderError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeConversionError {
    BeforeUnixEpoch,
    DurationOverflow,
    InvalidBounds { message: String },
}

impl fmt::Display for TimeConversionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BeforeUnixEpoch => formatter.write_str("system time precedes the Unix epoch"),
            Self::DurationOverflow => formatter.write_str("time bounds overflow u64 Unix seconds"),
            Self::InvalidBounds { message } => write!(formatter, "invalid time bounds: {message}"),
        }
    }
}

impl std::error::Error for TimeConversionError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactPart {
    Header,
    Payload,
    Signature,
}

impl CompactPart {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Header => "header",
            Self::Payload => "payload",
            Self::Signature => "signature",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignerError {
    Refused,
    UnsupportedAlgorithm { algorithm: String },
    InvalidSignatureLength { actual: usize, expected: usize },
    IssuerPublicKeyMismatch { issuer: String, public_key_issuer: String },
}

impl fmt::Display for SignerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused => formatter.write_str("signer refused to sign"),
            Self::UnsupportedAlgorithm { algorithm } => {
                write!(formatter, "unsupported signer algorithm {algorithm:?}")
            }
            Self::InvalidSignatureLength { actual, expected } => {
                write!(formatter, "signer returned {actual} signature bytes, expected {expected}")
            }
            Self::IssuerPublicKeyMismatch {
                issuer,
                public_key_issuer,
            } => {
                write!(formatter, "signer issuer {issuer} does not match public key issuer {public_key_issuer}")
            }
        }
    }
}

impl std::error::Error for SignerError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenError {
    EmptyCompactToken,
    CompactTokenMustBeTrimmed,
    InvalidCompactTokenStructure { actual_parts: usize },
    CompactTokenFramingMismatch,
    Base64Decode { part: CompactPart, message: String },
    JsonDecode { part: CompactPart, message: String },
    JsonEncode { message: String },
    UnsupportedHeader,
    Capability { message: String },
    MissingCapabilityField { index: usize, field: &'static str },
    CoreClaims { source: ucan_core::token::Error },
    InvalidSecretKeyLength { actual: usize },
    InvalidPublicKeyLength { actual: usize },
    DidEncodingUtf8,
    MissingVerificationKey { issuer: String },
    KeyResolver { source: KeyResolverError },
    ProofStore { source: ProofStoreError },
    Revocation { source: RevocationError },
    Revoked { reference: ProofReference },
    LimitExceeded { source: VerificationLimitError },
    Signature { issuer: String },
    Temporal { message: String },
    SelfDelegation { did: String },
    ProofChain { message: String },
    BackendTranscript { message: String },
    Attenuation { resource: String, ability: String },
    Signer { source: SignerError },
    SecretCapacity { expected: usize, actual: usize },
    JsonEscapesNotSupported { part: CompactPart },
    VerifiedPrimitive { source: VerifiedPrimitiveError },
}

impl fmt::Display for TokenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCompactToken => formatter.write_str("compact token must not be empty"),
            Self::CompactTokenMustBeTrimmed => {
                formatter.write_str("compact token must not contain surrounding whitespace")
            }
            Self::InvalidCompactTokenStructure { actual_parts } => {
                write!(formatter, "compact token must contain {COMPACT_TOKEN_PART_COUNT} parts, got {actual_parts}")
            }
            Self::CompactTokenFramingMismatch => {
                formatter.write_str("compact token bytes do not match verified compact framing")
            }
            Self::Base64Decode { part, message } => {
                write!(formatter, "{} part is not valid base64url: {message}", part.as_str())
            }
            Self::JsonDecode { part, message } => {
                write!(formatter, "{} part is not valid JSON: {message}", part.as_str())
            }
            Self::JsonEncode { message } => write!(formatter, "token JSON encoding failed: {message}"),
            Self::UnsupportedHeader => formatter.write_str("compact token header is unsupported"),
            Self::Capability { message } => write!(formatter, "capability validation failed: {message}"),
            Self::MissingCapabilityField { index, field } => {
                write!(formatter, "capability at index {index} is missing field {field:?}")
            }
            Self::CoreClaims { source } => write!(formatter, "core claim validation failed: {source}"),
            Self::InvalidSecretKeyLength { actual } => {
                write!(formatter, "ed25519 secret key must be {ED25519_SECRET_KEY_BYTES} bytes, got {actual}")
            }
            Self::InvalidPublicKeyLength { actual } => {
                write!(formatter, "ed25519 public key must be {ED25519_PUBLIC_KEY_BYTES} bytes, got {actual}")
            }
            Self::DidEncodingUtf8 => formatter.write_str("verified did:key bytes were not utf-8"),
            Self::MissingVerificationKey { issuer } => {
                write!(formatter, "no verification key registered for issuer {issuer}")
            }
            Self::KeyResolver { source } => write!(formatter, "key resolver failed: {source}"),
            Self::ProofStore { source } => write!(formatter, "proof store failed: {source}"),
            Self::Revocation { source } => write!(formatter, "revocation check failed: {source}"),
            Self::Revoked { reference } => write!(formatter, "token reference {reference} is revoked"),
            Self::LimitExceeded { source } => write!(formatter, "verification limit exceeded: {source}"),
            Self::Signature { issuer } => write!(formatter, "signature failed for issuer {issuer}"),
            Self::Temporal { message } => write!(formatter, "temporal verification failed: {message}"),
            Self::SelfDelegation { did } => write!(formatter, "self-delegation rejected for {did}"),
            Self::ProofChain { message } => write!(formatter, "proof-chain verification failed: {message}"),
            Self::BackendTranscript { message } => write!(formatter, "backend transcript rejected: {message}"),
            Self::Attenuation { resource, ability } => {
                write!(formatter, "capability {resource} {ability} is not delegated by proofs")
            }
            Self::Signer { source } => write!(formatter, "signer failed: {source}"),
            Self::SecretCapacity { expected, actual } => {
                write!(formatter, "secret buffer length {actual} does not match exact length {expected}")
            }
            Self::JsonEscapesNotSupported { part } => {
                write!(formatter, "{} JSON contains unsupported escape syntax", part.as_str())
            }
            Self::VerifiedPrimitive { source } => write!(formatter, "verified primitive failed: {source}"),
        }
    }
}

impl std::error::Error for TokenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CoreClaims { source } => Some(source),
            Self::Signer { source } => Some(source),
            Self::KeyResolver { source } => Some(source),
            Self::ProofStore { source } => Some(source),
            Self::Revocation { source } => Some(source),
            Self::LimitExceeded { source } => Some(source),
            Self::VerifiedPrimitive { source } => Some(source),
            Self::EmptyCompactToken
            | Self::CompactTokenMustBeTrimmed
            | Self::InvalidCompactTokenStructure { .. }
            | Self::CompactTokenFramingMismatch
            | Self::Base64Decode { .. }
            | Self::JsonDecode { .. }
            | Self::JsonEncode { .. }
            | Self::UnsupportedHeader
            | Self::Capability { .. }
            | Self::MissingCapabilityField { .. }
            | Self::InvalidSecretKeyLength { .. }
            | Self::InvalidPublicKeyLength { .. }
            | Self::DidEncodingUtf8
            | Self::MissingVerificationKey { .. }
            | Self::Revoked { .. }
            | Self::Signature { .. }
            | Self::Temporal { .. }
            | Self::SelfDelegation { .. }
            | Self::ProofChain { .. }
            | Self::BackendTranscript { .. }
            | Self::Attenuation { .. }
            | Self::SecretCapacity { .. }
            | Self::JsonEscapesNotSupported { .. } => None,
        }
    }
}

impl From<SignerError> for TokenError {
    fn from(source: SignerError) -> Self {
        Self::Signer { source }
    }
}

impl From<ucan_core::token::Error> for TokenError {
    fn from(source: ucan_core::token::Error) -> Self {
        Self::CoreClaims { source }
    }
}

impl From<VerifiedPrimitiveError> for TokenError {
    fn from(source: VerifiedPrimitiveError) -> Self {
        Self::VerifiedPrimitive { source }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyResolverError {
    MissingKey { issuer: String },
    UnsupportedMethod { issuer: String, message: String },
    Malformed { issuer: String, message: String },
    Stale { issuer: String, message: String },
    Backend { issuer: String, message: String },
}

impl fmt::Display for KeyResolverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingKey { issuer } => write!(formatter, "missing verification key for issuer {issuer}"),
            Self::UnsupportedMethod { issuer, message } => {
                write!(formatter, "unsupported DID method for issuer {issuer}: {message}")
            }
            Self::Malformed { issuer, message } => {
                write!(formatter, "malformed verification key for issuer {issuer}: {message}")
            }
            Self::Stale { issuer, message } => {
                write!(formatter, "stale verification key for issuer {issuer}: {message}")
            }
            Self::Backend { issuer, message } => {
                write!(formatter, "backend failed resolving issuer {issuer}: {message}")
            }
        }
    }
}

impl std::error::Error for KeyResolverError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofStoreError {
    MissingProof {
        reference: ProofReference,
    },
    Stale {
        reference: ProofReference,
        message: String,
    },
    Malformed {
        reference: ProofReference,
        message: String,
    },
    Backend {
        reference: ProofReference,
        message: String,
    },
    ReferenceMismatch {
        expected: ProofReference,
        actual: ProofReference,
    },
}

impl fmt::Display for ProofStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingProof { reference } => write!(formatter, "missing proof token {reference}"),
            Self::Stale { reference, message } => {
                write!(formatter, "stale proof token {reference}: {message}")
            }
            Self::Malformed { reference, message } => {
                write!(formatter, "malformed proof token {reference}: {message}")
            }
            Self::Backend { reference, message } => {
                write!(formatter, "backend failed resolving proof {reference}: {message}")
            }
            Self::ReferenceMismatch { expected, actual } => {
                write!(formatter, "resolved proof token {actual} does not match requested reference {expected}")
            }
        }
    }
}

impl std::error::Error for ProofStoreError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RevocationError {
    Stale { reference: ProofReference, message: String },
    Malformed { reference: ProofReference, message: String },
    Backend { reference: ProofReference, message: String },
}

impl fmt::Display for RevocationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stale { reference, message } => {
                write!(formatter, "stale revocation state for {reference}: {message}")
            }
            Self::Malformed { reference, message } => {
                write!(formatter, "malformed revocation state for {reference}: {message}")
            }
            Self::Backend { reference, message } => {
                write!(formatter, "backend failed checking revocation for {reference}: {message}")
            }
        }
    }
}

impl std::error::Error for RevocationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayAdmissionError {
    Duplicate { reference: ProofReference },
    Stale { reference: ProofReference, message: String },
    Malformed { reference: ProofReference, message: String },
    Denied { reference: ProofReference, message: String },
    Backend { reference: ProofReference, message: String },
}

impl fmt::Display for ReplayAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate { reference } => {
                write!(formatter, "replay admission rejected duplicate invocation {reference}")
            }
            Self::Stale { reference, message } => {
                write!(formatter, "replay admission rejected stale invocation {reference}: {message}")
            }
            Self::Malformed { reference, message } => {
                write!(formatter, "replay admission rejected malformed invocation {reference}: {message}")
            }
            Self::Denied { reference, message } => {
                write!(formatter, "replay admission denied invocation {reference}: {message}")
            }
            Self::Backend { reference, message } => {
                write!(formatter, "backend failed admitting invocation {reference}: {message}")
            }
        }
    }
}

impl std::error::Error for ReplayAdmissionError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationLimitError {
    ProofDepthExceeded { depth: usize, max_depth: usize },
    ProofCountExceeded { count: usize, max_count: usize },
    CompactTokenTooLarge { byte_len: usize, max_byte_len: usize },
}

impl fmt::Display for VerificationLimitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProofDepthExceeded { depth, max_depth } => {
                write!(formatter, "proof depth {depth} exceeds configured maximum {max_depth}")
            }
            Self::ProofCountExceeded { count, max_count } => {
                write!(formatter, "proof count {count} exceeds configured maximum {max_count}")
            }
            Self::CompactTokenTooLarge { byte_len, max_byte_len } => {
                write!(formatter, "compact token has {byte_len} bytes, maximum is {max_byte_len}")
            }
        }
    }
}

impl std::error::Error for VerificationLimitError {}

#[cfg(test)]
static SECRET_BYTES_DROP_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

struct SecretBytes {
    bytes: Zeroizing<Box<[u8]>>,
    initialized_len: usize,
}

impl Drop for SecretBytes {
    fn drop(&mut self) {
        #[cfg(test)]
        SECRET_BYTES_DROP_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

impl SecretBytes {
    fn from_slice(bytes: &[u8]) -> Self {
        let mut owned = Zeroizing::new(vec![0_u8; bytes.len()].into_boxed_slice());
        owned.as_mut().copy_from_slice(bytes);
        Self {
            bytes: owned,
            initialized_len: bytes.len(),
        }
    }

    fn zeroed_exact_len(len: usize) -> Self {
        Self {
            bytes: Zeroizing::new(vec![0_u8; len].into_boxed_slice()),
            initialized_len: len,
        }
    }

    fn empty_exact_capacity(capacity: usize) -> Self {
        Self {
            bytes: Zeroizing::new(vec![0_u8; capacity].into_boxed_slice()),
            initialized_len: 0,
        }
    }

    fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.initialized_len]
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        self.bytes.as_mut()
    }

    const fn len(&self) -> usize {
        self.initialized_len
    }

    fn allocation_len(&self) -> usize {
        self.bytes.len()
    }

    const fn is_empty(&self) -> bool {
        self.initialized_len == 0
    }

    fn finish_exact(self, expected_len: usize) -> TokenResult<Self> {
        if self.len() != expected_len || self.allocation_len() != expected_len {
            return Err(TokenError::SecretCapacity {
                expected: expected_len,
                actual: self.len(),
            });
        }
        Ok(self)
    }
}

impl Write for SecretBytes {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        let next_len = self
            .len()
            .checked_add(input.len())
            .ok_or_else(|| io::Error::new(io::ErrorKind::WriteZero, "secret buffer length overflow"))?;
        if next_len > self.allocation_len() {
            return Err(io::Error::new(io::ErrorKind::WriteZero, "secret buffer exceeded fixed allocation"));
        }
        self.bytes[self.initialized_len..next_len].copy_from_slice(input);
        self.initialized_len = next_len;
        Ok(input.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub struct CompactToken {
    bytes: SecretBytes,
}

impl PartialEq for CompactToken {
    fn eq(&self, other: &Self) -> bool {
        self.bytes.as_slice() == other.bytes.as_slice()
    }
}

impl Eq for CompactToken {}

impl fmt::Debug for CompactToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompactToken")
            .field("byte_len", &self.bytes.len())
            .field("bearer", &RedactedSecretMaterial)
            .finish()
    }
}

impl CompactToken {
    /// Copy bounded compact-token text into a zeroizing owner.
    ///
    /// # Errors
    ///
    /// Returns [`TokenError`] when the token is empty, untrimmed, or larger than
    /// the default compact-token limit.
    pub fn new(token: &str) -> TokenResult<Self> {
        enforce_compact_token_size(token, VerificationLimits::default())?;
        validate_compact_token_string(token)?;
        Ok(Self::from_validated_bytes(token.as_bytes()))
    }

    fn from_validated_bytes(bytes: &[u8]) -> Self {
        Self {
            bytes: SecretBytes::from_slice(bytes),
        }
    }

    fn from_secret_bytes(bytes: SecretBytes) -> TokenResult<Self> {
        let token = std::str::from_utf8(bytes.as_slice()).map_err(|_| TokenError::CompactTokenFramingMismatch)?;
        enforce_compact_token_size(token, VerificationLimits::default())?;
        validate_compact_token_string(token)?;
        Ok(Self { bytes })
    }

    /// Borrow the compact token without creating an owned copy.
    ///
    /// # Panics
    ///
    /// Panics only if the private validated-byte invariant is broken.
    #[must_use]
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(self.bytes.as_slice()).expect("validated compact token bytes remain UTF-8")
    }

    #[cfg(test)]
    fn allocation_len(&self) -> usize {
        self.bytes.allocation_len()
    }
}

impl TryFrom<&str> for CompactToken {
    type Error = TokenError;

    fn try_from(value: &str) -> TokenResult<Self> {
        Self::new(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuerDid(String);

impl IssuerDid {
    /// Build an issuer DID wrapper after core DID validation.
    ///
    /// # Errors
    ///
    /// Returns [`TokenError`] when the DID value is empty or untrimmed.
    pub fn new(value: impl Into<String>) -> TokenResult<Self> {
        let value = value.into();
        ucan_core::token::DidView::new(ucan_core::token::DidField::Issuer, value.as_str())?;
        Ok(Self(value))
    }

    #[must_use]
    pub const fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for IssuerDid {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for IssuerDid {
    type Err = TokenError;

    fn from_str(input: &str) -> TokenResult<Self> {
        Self::new(input.to_owned())
    }
}

impl TryFrom<&str> for IssuerDid {
    type Error = TokenError;

    fn try_from(value: &str) -> TokenResult<Self> {
        Self::new(value.to_owned())
    }
}

impl TryFrom<String> for IssuerDid {
    type Error = TokenError;

    fn try_from(value: String) -> TokenResult<Self> {
        Self::new(value)
    }
}

impl Serialize for IssuerDid {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where S: serde::Serializer {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for IssuerDid {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudienceDid(String);

impl AudienceDid {
    /// Build an audience DID wrapper after core DID validation.
    ///
    /// # Errors
    ///
    /// Returns [`TokenError`] when the DID value is empty or untrimmed.
    pub fn new(value: impl Into<String>) -> TokenResult<Self> {
        let value = value.into();
        ucan_core::token::DidView::new(ucan_core::token::DidField::Audience, value.as_str())?;
        Ok(Self(value))
    }

    #[must_use]
    pub const fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for AudienceDid {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for AudienceDid {
    type Err = TokenError;

    fn from_str(input: &str) -> TokenResult<Self> {
        Self::new(input.to_owned())
    }
}

impl TryFrom<&str> for AudienceDid {
    type Error = TokenError;

    fn try_from(value: &str) -> TokenResult<Self> {
        Self::new(value.to_owned())
    }
}

impl TryFrom<String> for AudienceDid {
    type Error = TokenError;

    fn try_from(value: String) -> TokenResult<Self> {
        Self::new(value)
    }
}

impl Serialize for AudienceDid {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where S: serde::Serializer {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for AudienceDid {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

impl From<IssuerDid> for AudienceDid {
    fn from(value: IssuerDid) -> Self {
        Self(value.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilitySet(Vec<CapabilityDocument>);

impl CapabilitySet {
    /// Build a one-capability collection for token issuance or verification.
    ///
    /// # Errors
    ///
    /// Returns [`TokenError`] when the capability or collection fails core validation.
    pub fn single(resource: impl Into<String>, ability: impl Into<String>) -> TokenResult<Self> {
        let capability =
            CapabilityDocument::new(resource.into(), ability.into()).map_err(|error| TokenError::Capability {
                message: error.to_string(),
            })?;
        Self::new(vec![capability])
    }

    /// Build a non-empty capability collection for token issuance or verification.
    ///
    /// # Errors
    ///
    /// Returns [`TokenError`] when core claim validation rejects the collection.
    pub fn new(capabilities: Vec<CapabilityDocument>) -> TokenResult<Self> {
        // All caveat input holders live in this block and are dropped after
        // core claim validation, so no verification or issuance self-check
        // allocation is leaked.
        let caveat_holders: Vec<Vec<ucan_core::token::CaveatInput<'_>>> = capabilities
            .iter()
            .map(|capability| {
                capability
                    .caveats
                    .iter()
                    .map(|caveat| {
                        ucan_core::token::CaveatInput::new(
                            caveat.domain.as_str(),
                            caveat.caveat_type.as_str(),
                            caveat.key.as_str(),
                            caveat.payload.as_slice(),
                        )
                    })
                    .collect()
            })
            .collect();
        let inputs: Vec<ucan_core::token::CapabilityInput<'_>> = capabilities
            .iter()
            .zip(caveat_holders.iter())
            .map(|(capability, holders)| {
                ucan_core::token::CapabilityInput::with_caveats(
                    capability.resource.as_str(),
                    capability.ability.as_str(),
                    holders.as_slice(),
                )
            })
            .collect();
        let empty_proofs: [ucan_core::token::ProofReferenceInput<'_>; 0] = [];
        let input = ucan_core::token::ClaimsInput {
            issuer_did: Some("did:key:z6Mkissuer"),
            audience_did: Some("did:key:z6Mkaudience"),
            capabilities: Some(inputs.as_slice()),
            proofs: Some(empty_proofs.as_slice()),
            not_before_seconds: Some(TokenTimeBounds::SMOKE_NOT_BEFORE_SECONDS),
            expires_at_seconds: Some(TokenTimeBounds::SMOKE_EXPIRES_AT_SECONDS),
        };
        ucan_core::token::Claims::from_input(input)?;
        Ok(Self(capabilities))
    }

    #[must_use]
    pub const fn as_slice(&self) -> &[CapabilityDocument] {
        self.0.as_slice()
    }

    #[must_use]
    pub fn into_vec(self) -> Vec<CapabilityDocument> {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationRequest {
    capability: CapabilityDocument,
}

impl AuthorizationRequest {
    /// Build a concrete authorization request.
    ///
    /// # Errors
    ///
    /// Returns [`TokenError`] when the requested resource or ability is not a
    /// valid capability input, or when the requested ability contains a wildcard
    /// segment. Wildcards are valid in delegated capabilities, but not in a
    /// concrete operation request.
    pub fn new(resource: impl Into<String>, ability: impl Into<String>) -> TokenResult<Self> {
        let capability =
            CapabilityDocument::new(resource.into(), ability.into()).map_err(|error| TokenError::Capability {
                message: error.to_string(),
            })?;
        let verified_capability =
            VerifiedCapability::from_document(capability.clone()).map_err(|error| TokenError::Capability {
                message: error.to_string(),
            })?;
        if !verified_capability.is_concrete() {
            return Err(TokenError::Capability {
                message: "authorization request ability must not contain wildcard segments".to_owned(),
            });
        }
        Ok(Self { capability })
    }

    #[must_use]
    pub const fn resource(&self) -> &str {
        self.capability.resource.as_str()
    }

    #[must_use]
    pub const fn ability(&self) -> &str {
        self.capability.ability.as_str()
    }

    #[must_use]
    pub const fn capability(&self) -> &CapabilityDocument {
        &self.capability
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorizationDenialReason {
    MalformedRequest { message: String },
    CaveatRejected { caveat: CaveatIdentifier, message: String },
    CaveatPolicyMissing { caveat: CaveatIdentifier },
    NoMatchingCapability,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaveatIdentifier {
    domain: String,
    caveat_type: String,
    key: String,
}

impl CaveatIdentifier {
    /// Build a caveat identity after core validation of identity fields.
    ///
    /// # Errors
    ///
    /// Returns [`TokenError`] when any identity field is empty or untrimmed.
    pub fn new(domain: impl Into<String>, caveat_type: impl Into<String>, key: impl Into<String>) -> TokenResult<Self> {
        let domain = domain.into();
        let caveat_type = caveat_type.into();
        let key = key.into();
        ucan_core::token::CaveatView::new(domain.as_str(), caveat_type.as_str(), key.as_str(), &[0]).map_err(
            |source| TokenError::Capability {
                message: source.to_string(),
            },
        )?;
        Ok(Self {
            domain,
            caveat_type,
            key,
        })
    }

    #[must_use]
    pub const fn domain(&self) -> &str {
        self.domain.as_str()
    }
    #[must_use]
    pub const fn caveat_type(&self) -> &str {
        self.caveat_type.as_str()
    }
    #[must_use]
    pub const fn key(&self) -> &str {
        self.key.as_str()
    }
}

impl From<&CaveatDocument> for CaveatIdentifier {
    fn from(caveat: &CaveatDocument) -> Self {
        Self {
            domain: caveat.domain.clone(),
            caveat_type: caveat.caveat_type.clone(),
            key: caveat.key.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaveatDecision {
    Satisfied,
    Rejected { message: String },
}

impl CaveatDecision {
    #[must_use]
    pub const fn is_satisfied(&self) -> bool {
        matches!(self, Self::Satisfied)
    }
}

/// A non-consuming caveat observation.
///
/// Implementations must not spend quota, reserve capacity, claim nonces, or
/// mutate admission state. Consumptive effects belong in a downstream atomic
/// boundary after UCAN returns authorization.
pub trait CaveatPolicy {
    fn evaluate(&self, caveat: &CaveatDocument, request: &AuthorizationRequest) -> CaveatDecision;
}

/// A non-consuming lookup for caveat observations.
pub trait CaveatPolicySet {
    fn policy_for(&self, caveat: &CaveatIdentifier) -> Option<&dyn CaveatPolicy>;
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DenyUnknownCaveats;

impl CaveatPolicySet for DenyUnknownCaveats {
    fn policy_for(&self, _caveat: &CaveatIdentifier) -> Option<&dyn CaveatPolicy> {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypedCaveatContext {
    now_seconds: u64,
    count_in_window: u64,
}

impl TypedCaveatContext {
    #[must_use]
    pub const fn new(now_seconds: u64, count_in_window: u64) -> Self {
        Self {
            now_seconds,
            count_in_window,
        }
    }

    #[must_use]
    pub const fn now_seconds(self) -> u64 {
        self.now_seconds
    }

    #[must_use]
    pub const fn count_in_window(self) -> u64 {
        self.count_in_window
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedTypedCaveatPolicySet {
    context: TypedCaveatContext,
    time_bound_policy: VerifiedTypedCaveatPolicy,
    rate_limit_policy: VerifiedTypedCaveatPolicy,
}

impl VerifiedTypedCaveatPolicySet {
    #[must_use]
    pub const fn new(context: TypedCaveatContext) -> Self {
        Self {
            context,
            time_bound_policy: VerifiedTypedCaveatPolicy::time_bound(context),
            rate_limit_policy: VerifiedTypedCaveatPolicy::rate_limit(context),
        }
    }

    #[must_use]
    pub const fn context(self) -> TypedCaveatContext {
        self.context
    }
}

impl CaveatPolicySet for VerifiedTypedCaveatPolicySet {
    fn policy_for(&self, caveat: &CaveatIdentifier) -> Option<&dyn CaveatPolicy> {
        match (caveat.domain(), caveat.caveat_type(), caveat.key()) {
            ("time", "before", "unix") => Some(&self.time_bound_policy),
            ("rate", "limit", "window") => Some(&self.rate_limit_policy),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VerifiedTypedCaveatKind {
    TimeBound,
    RateLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VerifiedTypedCaveatPolicy {
    context: TypedCaveatContext,
    kind: VerifiedTypedCaveatKind,
}

impl VerifiedTypedCaveatPolicy {
    const fn time_bound(context: TypedCaveatContext) -> Self {
        Self {
            context,
            kind: VerifiedTypedCaveatKind::TimeBound,
        }
    }

    const fn rate_limit(context: TypedCaveatContext) -> Self {
        Self {
            context,
            kind: VerifiedTypedCaveatKind::RateLimit,
        }
    }
}

impl CaveatPolicy for VerifiedTypedCaveatPolicy {
    fn evaluate(&self, caveat: &CaveatDocument, _request: &AuthorizationRequest) -> CaveatDecision {
        let parsed = match self.kind {
            VerifiedTypedCaveatKind::TimeBound => parse_time_bound_caveat(caveat),
            VerifiedTypedCaveatKind::RateLimit => parse_rate_limit_caveat(caveat),
        };
        let verified_caveat = match parsed {
            Ok(caveat) => caveat,
            Err(message) => return CaveatDecision::Rejected { message },
        };
        let context = VerifiedWebOfTrustCaveatContext {
            now_seconds: self.context.now_seconds(),
            count_in_window: self.context.count_in_window(),
        };
        if typed_caveat_is_satisfied(&verified_caveat, &context) {
            CaveatDecision::Satisfied
        } else {
            CaveatDecision::Rejected {
                message: "typed caveat rejected by verified caveat semantics".to_owned(),
            }
        }
    }
}

fn parse_time_bound_caveat(caveat: &CaveatDocument) -> std::result::Result<VerifiedWebOfTrustCaveat, String> {
    if !typed_caveat_ascii_decimal_payload_is_valid(caveat.payload.as_slice()) {
        return Err("time bound not_after_seconds payload must be non-empty verified ASCII decimal".to_owned());
    }
    let not_after_seconds = parse_verified_ascii_u64(caveat.payload.as_slice(), "time bound not_after_seconds")?;
    Ok(VerifiedWebOfTrustCaveat::TimeBound { not_after_seconds })
}

fn parse_rate_limit_caveat(caveat: &CaveatDocument) -> std::result::Result<VerifiedWebOfTrustCaveat, String> {
    if !typed_caveat_rate_limit_payload_shape_is_valid(caveat.payload.as_slice()) {
        return Err("rate-limit caveat payload must be verified max_count:window_seconds ASCII decimal".to_owned());
    }
    let payload = std::str::from_utf8(caveat.payload.as_slice())
        .map_err(|_| "rate-limit caveat payload must be UTF-8 ASCII decimal max_count:window_seconds".to_owned())?;
    let Some((max_count, window_seconds)) = payload.split_once(':') else {
        unreachable!("verified rate-limit payload shape contains exactly one ':' delimiter");
    };
    let max_count = parse_verified_ascii_u64(max_count.as_bytes(), "rate-limit max_count")?;
    let window_seconds = parse_verified_ascii_u64(window_seconds.as_bytes(), "rate-limit window_seconds")?;
    Ok(VerifiedWebOfTrustCaveat::RateLimit {
        max_count,
        window_seconds,
    })
}

fn parse_verified_ascii_u64(payload: &[u8], name: &str) -> std::result::Result<u64, String> {
    let text = std::str::from_utf8(payload).map_err(|_| format!("{name} payload must be verified ASCII decimal"))?;
    text.parse::<u64>().map_err(|_| format!("{name} payload must fit in u64"))
}

pub struct InvocationRequest<'a> {
    token: &'a CompactToken,
    context: &'a VerificationContext,
    resource: String,
    ability: String,
    policies: &'a dyn CaveatPolicySet,
}

impl<'a> InvocationRequest<'a> {
    #[must_use]
    pub fn new(
        token: &'a CompactToken,
        context: &'a VerificationContext,
        resource: impl Into<String>,
        ability: impl Into<String>,
        policies: &'a dyn CaveatPolicySet,
    ) -> Self {
        Self {
            token,
            context,
            resource: resource.into(),
            ability: ability.into(),
            policies,
        }
    }

    #[must_use]
    pub const fn token(&self) -> &CompactToken {
        self.token
    }

    #[must_use]
    pub const fn context(&self) -> &VerificationContext {
        self.context
    }

    #[must_use]
    pub const fn resource(&self) -> &str {
        self.resource.as_str()
    }

    #[must_use]
    pub const fn ability(&self) -> &str {
        self.ability.as_str()
    }
}

/// Audience-only compatibility authorization.
///
/// This value proves token and capability authorization for the audience. It
/// does not prove that a live sender holds the audience private key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvocationAllowed {
    issuer: IssuerDid,
    audience: AudienceDid,
    matched_capability: CapabilityDocument,
    effective_time_bounds: TokenTimeBounds,
}

impl InvocationAllowed {
    #[must_use]
    pub const fn issuer(&self) -> &IssuerDid {
        &self.issuer
    }
    #[must_use]
    pub const fn audience(&self) -> &AudienceDid {
        &self.audience
    }
    #[must_use]
    pub const fn matched_capability(&self) -> &CapabilityDocument {
        &self.matched_capability
    }
    #[must_use]
    pub const fn effective_time_bounds(&self) -> TokenTimeBounds {
        self.effective_time_bounds
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvocationDenialReason {
    Authorization { reason: AuthorizationDenialReason },
    Verification { message: String },
    Replay { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvocationResult {
    Allowed { invocation: InvocationAllowed },
    Denied { reason: InvocationDenialReason },
}

impl InvocationResult {
    #[must_use]
    pub const fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed { .. })
    }
}

/// Verify audience-only token and capability authorization.
///
/// This compatibility helper does not verify sender or holder possession. Use
/// [`verify_holder_signed_invocation`] when a live audience-key signature and
/// explicit nonce admission are required.
#[must_use]
pub fn verify_invocation(request: &InvocationRequest<'_>) -> InvocationResult {
    let verified = match verify_compact_token(request.token, request.context) {
        Ok(verified) => verified,
        Err(error) => {
            return InvocationResult::Denied {
                reason: InvocationDenialReason::Verification {
                    message: error.to_string(),
                },
            };
        }
    };
    match verified.authorize_with_policies(request.resource.as_str(), request.ability.as_str(), request.policies) {
        AuthorizationDecision::Allowed { matched_capability } => InvocationResult::Allowed {
            invocation: InvocationAllowed {
                issuer: verified.issuer().clone(),
                audience: verified.audience().clone(),
                matched_capability,
                effective_time_bounds: verified.effective_delegation().time_bounds(),
            },
        },
        AuthorizationDecision::Denied { reason } => InvocationResult::Denied {
            reason: InvocationDenialReason::Authorization { reason },
        },
    }
}

#[must_use]
pub fn verify_invocation_with_replay<A>(request: &InvocationRequest<'_>, replay: &A) -> InvocationResult
where A: ReplayAdmission + ?Sized {
    let result = verify_invocation(request);
    if !result.is_allowed() {
        return result;
    }
    let token_reference = proof_reference_for_bounded_token(request.token().as_str());
    match replay.admit_invocation(&token_reference, request.resource(), request.ability()) {
        Ok(()) => verified_replay_invocation_result(result),
        Err(error) => verified_replay_denial(&error),
    }
}

fn verified_replay_invocation_result(result: InvocationResult) -> InvocationResult {
    let transcript = replay_admission_transcript(VerifiedReplayAdmissionOutcome::Allowed);
    let outcome = replay_admission_transcript_outcome(&transcript);
    if outcome.accepted && outcome.first_failed_outcome == VerifiedReplayAdmissionOutcome::Accepted {
        result
    } else {
        InvocationResult::Denied {
            reason: InvocationDenialReason::Replay {
                message: VerifiedPrimitiveError::TokenValidation {
                    failure: TokenValidationFailure::InvalidReplayAdmissionTranscript,
                }
                .to_string(),
            },
        }
    }
}

fn verified_replay_denial(error: &ReplayAdmissionError) -> InvocationResult {
    let expected = replay_error_outcome(error);
    let transcript = replay_admission_transcript(expected);
    let outcome = replay_admission_transcript_outcome(&transcript);
    if outcome.accepted || outcome.first_failed_outcome != expected {
        return InvocationResult::Denied {
            reason: InvocationDenialReason::Replay {
                message: VerifiedPrimitiveError::TokenValidation {
                    failure: TokenValidationFailure::InvalidReplayAdmissionTranscript,
                }
                .to_string(),
            },
        };
    }
    InvocationResult::Denied {
        reason: InvocationDenialReason::Replay {
            message: error.to_string(),
        },
    }
}

fn replay_admission_transcript(outcome: VerifiedReplayAdmissionOutcome) -> VerifiedReplayAdmissionTranscript {
    VerifiedReplayAdmissionTranscript {
        facts: vec![VerifiedReplayAdmissionFact { outcome }],
    }
}

const fn replay_error_outcome(error: &ReplayAdmissionError) -> VerifiedReplayAdmissionOutcome {
    match error {
        ReplayAdmissionError::Duplicate { .. } => VerifiedReplayAdmissionOutcome::Duplicate,
        ReplayAdmissionError::Stale { .. } => VerifiedReplayAdmissionOutcome::Stale,
        ReplayAdmissionError::Malformed { .. } => VerifiedReplayAdmissionOutcome::Malformed,
        ReplayAdmissionError::Denied { .. } => VerifiedReplayAdmissionOutcome::Denied,
        ReplayAdmissionError::Backend { .. } => VerifiedReplayAdmissionOutcome::Backend,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorizationDecision {
    Allowed { matched_capability: CapabilityDocument },
    Denied { reason: AuthorizationDenialReason },
}

impl AuthorizationDecision {
    #[must_use]
    pub const fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed { .. })
    }

    #[must_use]
    pub const fn matched_capability(&self) -> Option<&CapabilityDocument> {
        match self {
            Self::Allowed { matched_capability } => Some(matched_capability),
            Self::Denied { .. } => None,
        }
    }

    #[must_use]
    pub const fn denial_reason(&self) -> Option<&AuthorizationDenialReason> {
        match self {
            Self::Allowed { .. } => None,
            Self::Denied { reason } => Some(reason),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProofReference(Vec<u8>);

impl ProofReference {
    #[must_use]
    pub const fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }

    #[must_use]
    pub fn to_base64url(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.as_bytes())
    }
}

impl fmt::Display for ProofReference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.to_base64url().as_str())
    }
}

/// Synchronous shell-owned key resolver used by verification.
pub trait KeyResolver {
    /// Resolve a verification key for the supplied issuer DID.
    ///
    /// # Errors
    ///
    /// Returns [`KeyResolverError`] when the issuer key is missing, unsupported,
    /// malformed, stale, or the resolver backend fails.
    fn resolve_key(&self, issuer: &IssuerDid) -> std::result::Result<Ed25519PublicKey, KeyResolverError>;
}

/// A proof token returned by a caller-owned proof store.
///
/// Borrowed values avoid token copies. Owned values retain zeroizing ownership.
pub enum ProofToken<'a> {
    Borrowed(&'a CompactToken),
    Owned(CompactToken),
}

impl ProofToken<'_> {
    fn as_str(&self) -> &str {
        match self {
            Self::Borrowed(token) => token.as_str(),
            Self::Owned(token) => token.as_str(),
        }
    }
}

/// Synchronous shell-owned proof store used by verification.
///
/// # Reference-binding contract
///
/// The token returned for a requested reference MUST be the token whose
/// bounded-compact BLAKE3 digest equals that reference. `resolve_proof` MAY
/// return `Ok` only for a matching token. The shell enforces the binding: it
/// recomputes the digest of every returned token and rejects a mismatch with a
/// deterministic [`ProofStoreError::ReferenceMismatch`] before the token is
/// used in delegation evaluation. The built-in [`ProofCollection`] selects
/// tokens by digest equality, so it satisfies the binding by construction.
pub trait ProofStore {
    /// Resolve a compact proof token by its proof reference.
    ///
    /// # Errors
    ///
    /// Returns [`ProofStoreError`] when the proof is missing, stale, malformed,
    /// or the store backend fails. The returned value must borrow secure storage
    /// or own a zeroizing compact token.
    ///
    /// The returned token must match the requested reference by digest. The
    /// shell recomputes the digest and enforces the mismatch rejection.
    fn resolve_proof<'a>(&'a self, reference: &ProofReference) -> std::result::Result<ProofToken<'a>, ProofStoreError>;
}

#[derive(Default)]
pub struct ProofCollection(Vec<CompactToken>);

impl PartialEq for ProofCollection {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for ProofCollection {}

impl fmt::Debug for ProofCollection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("ProofCollection").field("token_count", &self.0.len()).finish()
    }
}

impl ProofCollection {
    #[must_use]
    pub const fn empty() -> Self {
        Self(Vec::new())
    }

    #[must_use]
    pub const fn from_tokens(tokens: Vec<CompactToken>) -> Self {
        Self(tokens)
    }

    #[must_use]
    pub const fn as_slice(&self) -> &[CompactToken] {
        self.0.as_slice()
    }

    #[must_use]
    pub fn into_vec(self) -> Vec<CompactToken> {
        self.0
    }
}

impl ProofStore for ProofCollection {
    fn resolve_proof<'a>(&'a self, reference: &ProofReference) -> std::result::Result<ProofToken<'a>, ProofStoreError> {
        self.as_slice()
            .iter()
            .find(|candidate| proof_reference_for_bounded_token(candidate.as_str()).as_bytes() == reference.as_bytes())
            .map(ProofToken::Borrowed)
            .ok_or_else(|| ProofStoreError::MissingProof {
                reference: reference.clone(),
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProofReferences(Vec<ProofReference>);

impl ProofReferences {
    #[must_use]
    pub const fn empty() -> Self {
        Self(Vec::new())
    }

    #[must_use]
    pub const fn from_references(references: Vec<ProofReference>) -> Self {
        Self(references)
    }

    #[must_use]
    pub const fn as_slice(&self) -> &[ProofReference] {
        self.0.as_slice()
    }
}

impl From<ProofCollection> for ProofReferences {
    fn from(proofs: ProofCollection) -> Self {
        let references = proofs.into_vec().iter().map(proof_reference).collect();
        Self::from_references(references)
    }
}

/// Synchronous shell-owned revocation checker used by verification.
pub trait RevocationChecker {
    /// Return true when the compact-token proof reference has been revoked.
    ///
    /// # Errors
    ///
    /// Returns [`RevocationError`] when the revocation backend cannot answer.
    fn is_revoked(&self, reference: &ProofReference) -> std::result::Result<bool, RevocationError>;
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NoRevocations;

impl RevocationChecker for NoRevocations {
    fn is_revoked(&self, _reference: &ProofReference) -> std::result::Result<bool, RevocationError> {
        Ok(false)
    }
}

/// Synchronous shell-owned replay/nonce admission hook used by invocation authorization.
pub trait ReplayAdmission {
    /// Admit a verified invocation exactly once according to caller-owned replay policy.
    ///
    /// # Errors
    ///
    /// Returns [`ReplayAdmissionError`] when the invocation is a duplicate, stale,
    /// malformed for the caller policy, explicitly denied, or the backend cannot answer.
    fn admit_invocation(
        &self,
        token_reference: &ProofReference,
        resource: &str,
        ability: &str,
    ) -> std::result::Result<(), ReplayAdmissionError>;
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AllowAllReplayAdmission;

impl ReplayAdmission for AllowAllReplayAdmission {
    fn admit_invocation(
        &self,
        _token_reference: &ProofReference,
        _resource: &str,
        _ability: &str,
    ) -> std::result::Result<(), ReplayAdmissionError> {
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RevocationList(Vec<ProofReference>);

impl RevocationList {
    #[must_use]
    pub const fn empty() -> Self {
        Self(Vec::new())
    }

    #[must_use]
    pub const fn from_references(references: Vec<ProofReference>) -> Self {
        Self(references)
    }

    #[must_use]
    pub const fn as_slice(&self) -> &[ProofReference] {
        self.0.as_slice()
    }

    pub fn revoke(&mut self, reference: ProofReference) {
        let revoked_references =
            self.as_slice().iter().map(|existing| existing.as_bytes().to_vec()).collect::<Vec<_>>();
        if !revocation_reference_is_listed(&revoked_references, reference.as_bytes()) {
            self.0.push(reference);
        }
    }
}

impl RevocationChecker for RevocationList {
    fn is_revoked(&self, reference: &ProofReference) -> std::result::Result<bool, RevocationError> {
        let revoked_references =
            self.as_slice().iter().map(|candidate| candidate.as_bytes().to_vec()).collect::<Vec<_>>();
        Ok(revocation_reference_is_listed(&revoked_references, reference.as_bytes()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::struct_field_names)]
pub struct VerificationLimits {
    proof_depth_limit: usize,
    proof_count_limit: usize,
    compact_token_byte_limit: usize,
    clock_skew_tolerance_seconds: u64,
}

impl VerificationLimits {
    #[must_use]
    pub const fn new(max_proof_depth: usize, max_proof_count: usize, max_compact_token_bytes: usize) -> Self {
        Self {
            proof_depth_limit: max_proof_depth,
            proof_count_limit: max_proof_count,
            compact_token_byte_limit: max_compact_token_bytes,
            clock_skew_tolerance_seconds: crate::verified::DEFAULT_CLOCK_SKEW_SECONDS,
        }
    }

    /// Build strict zero-tolerance limits with the default resource bounds.
    #[must_use]
    pub const fn strict() -> Self {
        Self {
            proof_depth_limit: DEFAULT_MAX_PROOF_DEPTH,
            proof_count_limit: DEFAULT_MAX_PROOF_COUNT,
            compact_token_byte_limit: DEFAULT_MAX_COMPACT_TOKEN_BYTES,
            clock_skew_tolerance_seconds: ZERO_CLOCK_SKEW_TOLERANCE_SECONDS,
        }
    }

    /// Select a clock-skew tolerance in seconds for this verification.
    #[must_use]
    pub const fn with_clock_skew_tolerance_seconds(mut self, seconds: u64) -> Self {
        self.clock_skew_tolerance_seconds = seconds;
        self
    }

    /// Return the clock-skew tolerance in seconds used by every gate.
    #[must_use]
    pub const fn clock_skew_tolerance_seconds(self) -> u64 {
        self.clock_skew_tolerance_seconds
    }

    #[must_use]
    pub const fn max_proof_depth(self) -> usize {
        self.proof_depth_limit
    }

    #[must_use]
    pub const fn max_proof_count(self) -> usize {
        self.proof_count_limit
    }

    #[must_use]
    pub const fn max_compact_token_bytes(self) -> usize {
        self.compact_token_byte_limit
    }
}

impl Default for VerificationLimits {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_PROOF_DEPTH, DEFAULT_MAX_PROOF_COUNT, DEFAULT_MAX_COMPACT_TOKEN_BYTES)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenTimeBounds {
    not_before_seconds: u64,
    expires_at_seconds: u64,
}

impl TokenTimeBounds {
    const SMOKE_NOT_BEFORE_SECONDS: u64 = 1;
    const SMOKE_EXPIRES_AT_SECONDS: u64 = 2;

    /// Build token time bounds with positive duration.
    ///
    /// # Errors
    ///
    /// Returns [`TokenError`] when `nbf` is not earlier than `exp`.
    pub const fn new(not_before_seconds: u64, expires_at_seconds: u64) -> TokenResult<Self> {
        match ucan_core::token::TimeBounds::new(not_before_seconds, expires_at_seconds) {
            Ok(_) => Ok(Self {
                not_before_seconds,
                expires_at_seconds,
            }),
            Err(source) => Err(TokenError::CoreClaims { source }),
        }
    }

    #[must_use]
    pub const fn not_before_seconds(self) -> u64 {
        self.not_before_seconds
    }

    #[must_use]
    pub const fn expires_at_seconds(self) -> u64 {
        self.expires_at_seconds
    }

    /// Build token bounds from a deterministic start time and positive duration.
    ///
    /// # Errors
    ///
    /// Returns [`TimeConversionError`] when the duration overflows or yields invalid bounds.
    pub fn from_unix_seconds_and_duration(not_before_seconds: u64, duration: Duration) -> TimeConversionResult<Self> {
        let expires_at_seconds =
            not_before_seconds.checked_add(duration.as_secs()).ok_or(TimeConversionError::DurationOverflow)?;
        Self::new(not_before_seconds, expires_at_seconds).map_err(|error| TimeConversionError::InvalidBounds {
            message: error.to_string(),
        })
    }

    /// Build token bounds from a [`SystemTime`] start and positive duration.
    ///
    /// # Errors
    ///
    /// Returns [`TimeConversionError`] when the start time is before the Unix epoch,
    /// the duration overflows, or the resulting bounds are invalid.
    pub fn from_system_time_and_duration(start: SystemTime, duration: Duration) -> TimeConversionResult<Self> {
        let not_before_seconds = unix_seconds_from_system_time(start)?;
        Self::from_unix_seconds_and_duration(not_before_seconds, duration)
    }

    const fn to_verified(self) -> TokenTimeWindow {
        TokenTimeWindow {
            not_before_seconds: self.not_before_seconds,
            expires_at_seconds: self.expires_at_seconds,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerificationTime {
    seconds: u64,
}

impl VerificationTime {
    #[must_use]
    pub const fn from_unix_seconds(seconds: u64) -> Self {
        Self { seconds }
    }

    #[must_use]
    pub const fn as_unix_seconds(self) -> u64 {
        self.seconds
    }

    /// Convert a [`SystemTime`] into deterministic Unix-second verification time.
    ///
    /// # Errors
    ///
    /// Returns [`TimeConversionError::BeforeUnixEpoch`] when the supplied time is
    /// earlier than the Unix epoch.
    pub fn try_from_system_time(time: SystemTime) -> TimeConversionResult<Self> {
        unix_seconds_from_system_time(time).map(Self::from_unix_seconds)
    }
}

fn unix_seconds_from_system_time(time: SystemTime) -> TimeConversionResult<u64> {
    let duration = time.duration_since(UNIX_EPOCH).map_err(|_| TimeConversionError::BeforeUnixEpoch)?;
    Ok(duration.as_secs())
}

#[derive(PartialEq, Eq)]
pub struct Ed25519SigningKey {
    bytes: Zeroizing<[u8; ED25519_SECRET_KEY_BYTES]>,
}

impl Clone for Ed25519SigningKey {
    fn clone(&self) -> Self {
        Self {
            bytes: self.bytes.clone(),
        }
    }
}

impl fmt::Debug for Ed25519SigningKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("Ed25519SigningKey");
        debug.field("secret_key", &RedactedSecretMaterial);
        debug.finish()
    }
}

struct RedactedSecretMaterial;

impl fmt::Debug for RedactedSecretMaterial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(REDACTED_SECRET_MATERIAL_DEBUG)
    }
}

impl Ed25519SigningKey {
    #[must_use]
    pub fn from_bytes(bytes: [u8; ED25519_SECRET_KEY_BYTES]) -> Self {
        Self {
            bytes: Zeroizing::new(bytes),
        }
    }

    /// Build a signing key from a byte slice.
    ///
    /// # Errors
    ///
    /// Returns [`TokenError::InvalidSecretKeyLength`] when the slice length is
    /// not the Ed25519 seed length.
    pub fn try_from_slice(bytes: &[u8]) -> TokenResult<Self> {
        let bytes = copy_secret_key(bytes)?;
        Ok(Self {
            bytes: Zeroizing::new(bytes),
        })
    }

    /// Derive the issuer DID for this signing key.
    ///
    /// # Errors
    ///
    /// Returns [`TokenError`] if verified key derivation or DID encoding fails.
    pub fn issuer(&self) -> TokenResult<IssuerDid> {
        let public_key = self.public_key()?;
        public_key.issuer()
    }

    /// Derive the matching public key.
    ///
    /// # Errors
    ///
    /// Returns [`TokenError`] if verified key derivation fails.
    pub fn public_key(&self) -> TokenResult<Ed25519PublicKey> {
        let bytes = derive_ed25519_public_key(self.bytes.as_slice())?;
        Ed25519PublicKey::try_from_slice(bytes.as_slice())
    }

    fn as_bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}

pub struct SignerSignature {
    bytes: Zeroizing<[u8; ED25519_SIGNATURE_BYTES]>,
}

impl PartialEq for SignerSignature {
    fn eq(&self, other: &Self) -> bool {
        self.bytes.as_slice() == other.bytes.as_slice()
    }
}

impl Eq for SignerSignature {}

impl fmt::Debug for SignerSignature {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("SignerSignature").field("byte_len", &self.bytes.len()).finish()
    }
}

impl SignerSignature {
    #[must_use]
    pub fn from_bytes(bytes: [u8; ED25519_SIGNATURE_BYTES]) -> Self {
        Self {
            bytes: Zeroizing::new(bytes),
        }
    }

    /// Copy an exact-width signature into fixed zeroizing storage.
    ///
    /// # Errors
    ///
    /// Returns [`SignerError::InvalidSignatureLength`] for non-Ed25519 widths.
    pub fn try_from_slice(bytes: &[u8]) -> SignerResult<Self> {
        if bytes.len() != ED25519_SIGNATURE_BYTES {
            return Err(SignerError::InvalidSignatureLength {
                actual: bytes.len(),
                expected: ED25519_SIGNATURE_BYTES,
            });
        }
        let mut fixed = [0_u8; ED25519_SIGNATURE_BYTES];
        fixed.copy_from_slice(bytes);
        Ok(Self::from_bytes(fixed))
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}

/// Signing boundary for compact-token issuance.
///
/// Implementations keep private key custody outside the issuance pipeline while
/// exposing only algorithm identity, issuer identity, verification material, and
/// signatures over caller-provided bytes.
pub trait TokenSigner {
    #[must_use]
    fn algorithm(&self) -> &str;

    /// Return the signer issuer DID.
    ///
    /// # Errors
    ///
    /// Returns [`TokenError`] when the signer cannot provide a valid issuer DID.
    fn issuer(&self) -> TokenResult<IssuerDid>;

    /// Return the public verification key for the signer.
    ///
    /// # Errors
    ///
    /// Returns [`TokenError`] when the signer cannot provide valid verification material.
    fn public_key(&self) -> TokenResult<Ed25519PublicKey>;

    /// Sign the supplied compact-token signing input bytes.
    ///
    /// # Errors
    ///
    /// Returns [`SignerError`] when the signer refuses or cannot produce a signature.
    fn sign(&self, signing_input: &[u8]) -> SignerResult<SignerSignature>;
}

#[derive(PartialEq, Eq)]
pub struct Ed25519InMemorySigner {
    signing_key: Ed25519SigningKey,
}

impl fmt::Debug for Ed25519InMemorySigner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("Ed25519InMemorySigner");
        debug.field("secret_key", &RedactedSecretMaterial);
        debug.finish()
    }
}

impl Ed25519InMemorySigner {
    #[must_use]
    pub const fn from_signing_key(signing_key: Ed25519SigningKey) -> Self {
        Self { signing_key }
    }

    #[must_use]
    pub fn from_seed_bytes(bytes: [u8; ED25519_SECRET_KEY_BYTES]) -> Self {
        Self::from_signing_key(Ed25519SigningKey::from_bytes(bytes))
    }

    /// Build an in-memory signer from a byte slice.
    ///
    /// # Errors
    ///
    /// Returns [`TokenError::InvalidSecretKeyLength`] when the slice length is
    /// not the Ed25519 seed length.
    pub fn try_from_slice(bytes: &[u8]) -> TokenResult<Self> {
        let signing_key = Ed25519SigningKey::try_from_slice(bytes)?;
        Ok(Self::from_signing_key(signing_key))
    }

    /// Build the matching verification key for this in-memory signer.
    ///
    /// # Errors
    ///
    /// Returns [`TokenError`] if public-key derivation or DID encoding fails.
    pub fn verification_key(&self) -> TokenResult<Ed25519VerificationKey> {
        Ed25519VerificationKey::from_signing_key(&self.signing_key)
    }
}

impl From<Ed25519SigningKey> for Ed25519InMemorySigner {
    fn from(signing_key: Ed25519SigningKey) -> Self {
        Self::from_signing_key(signing_key)
    }
}

impl TokenSigner for Ed25519SigningKey {
    fn algorithm(&self) -> &str {
        TOKEN_ALGORITHM_EDDSA
    }

    fn issuer(&self) -> TokenResult<IssuerDid> {
        self.issuer()
    }

    fn public_key(&self) -> TokenResult<Ed25519PublicKey> {
        self.public_key()
    }

    fn sign(&self, signing_input: &[u8]) -> SignerResult<SignerSignature> {
        let signature = sign_ed25519(self.as_bytes(), signing_input).map_err(|_| SignerError::Refused)?;
        Ok(SignerSignature::from_bytes(signature))
    }
}

impl TokenSigner for Ed25519InMemorySigner {
    fn algorithm(&self) -> &str {
        TOKEN_ALGORITHM_EDDSA
    }

    fn issuer(&self) -> TokenResult<IssuerDid> {
        self.signing_key.issuer()
    }

    fn public_key(&self) -> TokenResult<Ed25519PublicKey> {
        self.signing_key.public_key()
    }

    fn sign(&self, signing_input: &[u8]) -> SignerResult<SignerSignature> {
        let signature = sign_ed25519(self.signing_key.as_bytes(), signing_input).map_err(|_| SignerError::Refused)?;
        Ok(SignerSignature::from_bytes(signature))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ed25519PublicKey {
    bytes: [u8; ED25519_PUBLIC_KEY_BYTES],
}

impl Ed25519PublicKey {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; ED25519_PUBLIC_KEY_BYTES]) -> Self {
        Self { bytes }
    }

    /// Build a public key from a byte slice.
    ///
    /// # Errors
    ///
    /// Returns [`TokenError::InvalidPublicKeyLength`] when the slice length is
    /// not the Ed25519 public key length.
    pub fn try_from_slice(bytes: &[u8]) -> TokenResult<Self> {
        let bytes = copy_public_key(bytes)?;
        Ok(Self { bytes })
    }

    /// Encode this public key as an issuer DID.
    ///
    /// # Errors
    ///
    /// Returns [`TokenError`] if verified DID encoding fails.
    pub fn issuer(&self) -> TokenResult<IssuerDid> {
        let did_bytes = encode_ed25519_did_key(self.bytes.as_slice())?;
        let did = String::from_utf8(did_bytes).map_err(|_| TokenError::DidEncodingUtf8)?;
        IssuerDid::new(did)
    }

    const fn as_bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ed25519VerificationKey {
    issuer: IssuerDid,
    public_key: Ed25519PublicKey,
}

impl Ed25519VerificationKey {
    /// Build an issuer-bound verification key.
    ///
    /// # Errors
    ///
    /// Returns [`TokenError`] when the public key does not encode to the issuer DID.
    pub fn new(issuer: IssuerDid, public_key: Ed25519PublicKey) -> TokenResult<Self> {
        let derived = public_key.issuer()?;
        if derived != issuer {
            return Err(TokenError::ProofChain {
                message: "verification key DID does not match its public key".to_owned(),
            });
        }
        Ok(Self { issuer, public_key })
    }

    /// Build a verification key from a signing key.
    ///
    /// # Errors
    ///
    /// Returns [`TokenError`] if public-key derivation or DID encoding fails.
    pub fn from_signing_key(signing_key: &Ed25519SigningKey) -> TokenResult<Self> {
        let public_key = signing_key.public_key()?;
        let issuer = public_key.issuer()?;
        Ok(Self { issuer, public_key })
    }

    #[must_use]
    pub const fn issuer(&self) -> &IssuerDid {
        &self.issuer
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyResolutionContext {
    keys: Vec<Ed25519VerificationKey>,
}

impl KeyResolutionContext {
    #[must_use]
    pub const fn new(keys: Vec<Ed25519VerificationKey>) -> Self {
        Self { keys }
    }

    fn resolve(&self, issuer: &IssuerDid) -> Option<&Ed25519PublicKey> {
        self.keys.iter().find_map(|key| {
            if key.issuer() == issuer {
                Some(&key.public_key)
            } else {
                None
            }
        })
    }
}

impl KeyResolver for KeyResolutionContext {
    fn resolve_key(&self, issuer: &IssuerDid) -> std::result::Result<Ed25519PublicKey, KeyResolverError> {
        self.resolve(issuer).cloned().ok_or_else(|| KeyResolverError::MissingKey {
            issuer: issuer.as_str().to_owned(),
        })
    }
}

#[derive(PartialEq, Eq)]
pub struct IssueRequest {
    signing_key: Ed25519SigningKey,
    audience: AudienceDid,
    capabilities: CapabilitySet,
    proofs: ProofReferences,
    time_bounds: TokenTimeBounds,
}

impl fmt::Debug for IssueRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IssueRequest")
            .field("signing_key", &self.signing_key)
            .field("audience", &self.audience)
            .field("capabilities", &self.capabilities)
            .field("proof_count", &self.proofs.as_slice().len())
            .field("time_bounds", &self.time_bounds)
            .finish()
    }
}

impl IssueRequest {
    #[must_use]
    pub const fn new(
        signing_key: Ed25519SigningKey,
        audience: AudienceDid,
        capabilities: CapabilitySet,
        proofs: ProofReferences,
        time_bounds: TokenTimeBounds,
    ) -> Self {
        Self {
            signing_key,
            audience,
            capabilities,
            proofs,
            time_bounds,
        }
    }

    #[must_use]
    pub const fn builder() -> IssueRequestBuilder {
        IssueRequestBuilder::new()
    }
}

#[derive(Default)]
pub struct IssueRequestBuilder {
    signing_key: Option<Ed25519SigningKey>,
    audience: Option<AudienceDid>,
    capabilities: Option<CapabilitySet>,
    proofs: Option<ProofReferences>,
    time_bounds: Option<TokenTimeBounds>,
}

impl IssueRequestBuilder {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            signing_key: None,
            audience: None,
            capabilities: None,
            proofs: None,
            time_bounds: None,
        }
    }

    #[must_use]
    pub fn signing_key(mut self, signing_key: Ed25519SigningKey) -> Self {
        self.signing_key = Some(signing_key);
        self
    }

    #[must_use]
    pub fn audience(mut self, audience: AudienceDid) -> Self {
        self.audience = Some(audience);
        self
    }

    #[must_use]
    pub fn capabilities(mut self, capabilities: CapabilitySet) -> Self {
        self.capabilities = Some(capabilities);
        self
    }

    #[must_use]
    pub fn proofs(mut self, proofs: ProofReferences) -> Self {
        self.proofs = Some(proofs);
        self
    }

    #[must_use]
    pub const fn time_bounds(mut self, time_bounds: TokenTimeBounds) -> Self {
        self.time_bounds = Some(time_bounds);
        self
    }

    /// Finalize the request.
    ///
    /// # Errors
    ///
    /// Returns [`BuilderError`] naming the first required field that is absent.
    pub fn build(self) -> BuilderResult<IssueRequest> {
        Ok(IssueRequest::new(
            self.signing_key.ok_or(BuilderError::MissingField { field: "signing_key" })?,
            self.audience.ok_or(BuilderError::MissingField { field: "audience" })?,
            self.capabilities.ok_or(BuilderError::MissingField { field: "capabilities" })?,
            self.proofs.ok_or(BuilderError::MissingField { field: "proofs" })?,
            self.time_bounds.ok_or(BuilderError::MissingField { field: "time_bounds" })?,
        ))
    }
}

#[derive(PartialEq, Eq)]
pub struct VerificationContext {
    time: VerificationTime,
    keys: KeyResolutionContext,
    proofs: ProofCollection,
    limits: VerificationLimits,
}

impl fmt::Debug for VerificationContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerificationContext")
            .field("time", &self.time)
            .field("keys", &self.keys)
            .field("proof_count", &self.proofs.as_slice().len())
            .field("limits", &self.limits)
            .finish()
    }
}

impl VerificationContext {
    #[must_use]
    pub fn new(time: VerificationTime, keys: KeyResolutionContext, proofs: ProofCollection) -> Self {
        Self::with_limits(time, keys, proofs, VerificationLimits::default())
    }

    #[must_use]
    pub const fn with_limits(
        time: VerificationTime,
        keys: KeyResolutionContext,
        proofs: ProofCollection,
        limits: VerificationLimits,
    ) -> Self {
        Self {
            time,
            keys,
            proofs,
            limits,
        }
    }

    #[must_use]
    pub const fn time(&self) -> VerificationTime {
        self.time
    }

    #[must_use]
    pub const fn keys(&self) -> &KeyResolutionContext {
        &self.keys
    }

    #[must_use]
    pub const fn proofs(&self) -> &ProofCollection {
        &self.proofs
    }

    #[must_use]
    pub const fn limits(&self) -> VerificationLimits {
        self.limits
    }

    #[must_use]
    pub const fn builder() -> VerificationContextBuilder {
        VerificationContextBuilder::new()
    }
}

#[derive(Default)]
pub struct VerificationContextBuilder {
    time: Option<VerificationTime>,
    keys: Option<KeyResolutionContext>,
    proofs: Option<ProofCollection>,
    limits: Option<VerificationLimits>,
}

impl VerificationContextBuilder {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            time: None,
            keys: None,
            proofs: None,
            limits: None,
        }
    }

    #[must_use]
    pub const fn time(mut self, time: VerificationTime) -> Self {
        self.time = Some(time);
        self
    }

    #[must_use]
    pub fn keys(mut self, keys: KeyResolutionContext) -> Self {
        self.keys = Some(keys);
        self
    }

    #[must_use]
    pub fn proofs(mut self, proofs: ProofCollection) -> Self {
        self.proofs = Some(proofs);
        self
    }

    #[must_use]
    pub const fn limits(mut self, limits: VerificationLimits) -> Self {
        self.limits = Some(limits);
        self
    }

    /// Finalize the verification context.
    ///
    /// # Errors
    ///
    /// Returns [`BuilderError`] naming the first required field that is absent.
    pub fn build(self) -> BuilderResult<VerificationContext> {
        Ok(VerificationContext::with_limits(
            self.time.ok_or(BuilderError::MissingField { field: "time" })?,
            self.keys.ok_or(BuilderError::MissingField { field: "keys" })?,
            self.proofs.ok_or(BuilderError::MissingField { field: "proofs" })?,
            self.limits.unwrap_or_default(),
        ))
    }
}

pub struct DecodedToken<'a> {
    reference: ProofReference,
    compact_byte_len: usize,
    header_json: SecretBytes,
    claims_json: SecretBytes,
    signature: SecretBytes,
    signing_input: &'a [u8],
}

impl fmt::Debug for DecodedToken<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DecodedToken")
            .field("reference", &self.reference)
            .field("compact_byte_len", &self.compact_byte_len)
            .field("bearer", &RedactedSecretMaterial)
            .finish()
    }
}

pub struct StructurallyValidToken<'a> {
    reference: ProofReference,
    compact_byte_len: usize,
    claims: ucan_core::token::Claims,
    signature: SecretBytes,
    signing_input: &'a [u8],
}

impl fmt::Debug for StructurallyValidToken<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StructurallyValidToken")
            .field("reference", &self.reference)
            .field("compact_byte_len", &self.compact_byte_len)
            .field("bearer", &RedactedSecretMaterial)
            .finish()
    }
}

impl StructurallyValidToken<'_> {
    #[must_use]
    pub const fn claims(&self) -> &ucan_core::token::Claims {
        &self.claims
    }

    #[must_use]
    pub const fn reference(&self) -> &ProofReference {
        &self.reference
    }
}

pub struct SignatureVerifiedToken<'a> {
    token: StructurallyValidToken<'a>,
}

impl fmt::Debug for SignatureVerifiedToken<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("SignatureVerifiedToken").field("token", &self.token).finish()
    }
}

impl SignatureVerifiedToken<'_> {
    #[must_use]
    pub const fn claims(&self) -> &ucan_core::token::Claims {
        self.token.claims()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveDelegation {
    capabilities: CapabilitySet,
    time_bounds: TokenTimeBounds,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EffectiveDelegationEvaluation {
    effective: EffectiveDelegation,
    parent_proof_transcript: Option<LiveProofTraversalTranscript>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LiveProofTraversalTranscript {
    entries: Vec<VerifiedProofTraversalEntry>,
    parent_indices: Vec<Vec<usize>>,
    leaf_indices: Vec<usize>,
    total_input_bytes: u64,
}

impl EffectiveDelegation {
    #[must_use]
    pub const fn capabilities(&self) -> &CapabilitySet {
        &self.capabilities
    }

    #[must_use]
    pub const fn time_bounds(&self) -> TokenTimeBounds {
        self.time_bounds
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedToken {
    issuer: IssuerDid,
    audience: AudienceDid,
    capabilities: CapabilitySet,
    proofs: ProofReferences,
    effective: EffectiveDelegation,
    proof_transcript: Option<LiveProofTraversalTranscript>,
}

impl VerifiedToken {
    #[must_use]
    pub const fn issuer(&self) -> &IssuerDid {
        &self.issuer
    }

    #[must_use]
    pub const fn audience(&self) -> &AudienceDid {
        &self.audience
    }

    #[must_use]
    pub const fn capabilities(&self) -> &CapabilitySet {
        &self.capabilities
    }

    #[must_use]
    pub const fn proofs(&self) -> &ProofReferences {
        &self.proofs
    }

    /// Nominal protocol profile that produced this verified token.
    ///
    /// The three-part compact Ed25519 token is always bound to the local
    /// `onix-compact-ed25519-v1` profile. It is never labeled as final UCAN
    /// `1.0.0`.
    #[must_use]
    pub const fn protocol_profile(&self) -> crate::protocol_profile::ProtocolProfileId {
        crate::protocol_profile::ProtocolProfileId::OnixCompactEd25519V1
    }

    #[must_use]
    pub const fn effective_delegation(&self) -> &EffectiveDelegation {
        &self.effective
    }

    /// Authorize a concrete resource/ability pair against this token's effective delegation.
    #[must_use]
    pub fn authorize(&self, resource: &str, ability: &str) -> AuthorizationDecision {
        match AuthorizationRequest::new(resource.to_owned(), ability.to_owned()) {
            Ok(request) => self.authorize_request(&request),
            Err(error) => AuthorizationDecision::Denied {
                reason: AuthorizationDenialReason::MalformedRequest {
                    message: error.to_string(),
                },
            },
        }
    }

    /// Authorize a concrete resource/ability pair through explicit caveat policies.
    #[must_use]
    pub fn authorize_with_policies(
        &self,
        resource: &str,
        ability: &str,
        policies: &dyn CaveatPolicySet,
    ) -> AuthorizationDecision {
        match AuthorizationRequest::new(resource.to_owned(), ability.to_owned()) {
            Ok(request) => self.authorize_request_with_policies(&request, policies),
            Err(error) => AuthorizationDecision::Denied {
                reason: AuthorizationDenialReason::MalformedRequest {
                    message: error.to_string(),
                },
            },
        }
    }

    /// Authorize a prevalidated concrete request against this token's effective delegation.
    #[must_use]
    pub fn authorize_request(&self, request: &AuthorizationRequest) -> AuthorizationDecision {
        self.authorize_request_with_policies(request, &DenyUnknownCaveats)
    }

    /// Authorize a prevalidated concrete request through explicit caveat policies.
    #[must_use]
    pub fn authorize_request_with_policies(
        &self,
        request: &AuthorizationRequest,
        policies: &dyn CaveatPolicySet,
    ) -> AuthorizationDecision {
        let matched = self.match_authorization_request(request);
        let AuthorizationDecision::Allowed { matched_capability } = matched else {
            return matched;
        };
        if let Some(reason) = evaluate_capability_caveats(&matched_capability, request, policies) {
            return AuthorizationDecision::Denied { reason };
        }
        AuthorizationDecision::Allowed { matched_capability }
    }

    fn match_authorization_request(&self, request: &AuthorizationRequest) -> AuthorizationDecision {
        let requested = match VerifiedCapability::from_document(request.capability().clone()) {
            Ok(requested) => requested,
            Err(error) => {
                return AuthorizationDecision::Denied {
                    reason: AuthorizationDenialReason::MalformedRequest {
                        message: error.to_string(),
                    },
                };
            }
        };
        for capability in self.effective_delegation().capabilities().as_slice() {
            let parent = match VerifiedCapability::from_document(capability.clone()) {
                Ok(parent) => parent,
                Err(error) => {
                    return AuthorizationDecision::Denied {
                        reason: AuthorizationDenialReason::MalformedRequest {
                            message: error.to_string(),
                        },
                    };
                }
            };
            if parent.contains(&requested) {
                return AuthorizationDecision::Allowed {
                    matched_capability: capability.clone(),
                };
            }
        }
        AuthorizationDecision::Denied {
            reason: AuthorizationDenialReason::NoMatchingCapability,
        }
    }
}

fn evaluate_capability_caveats(
    capability: &CapabilityDocument,
    request: &AuthorizationRequest,
    policies: &dyn CaveatPolicySet,
) -> Option<AuthorizationDenialReason> {
    let mut transcript = VerifiedCaveatEvaluationTranscript {
        facts: Vec::with_capacity(capability.caveats.as_slice().len()),
    };

    for caveat in capability.caveats.as_slice() {
        let identifier = CaveatIdentifier::from(caveat);
        let Some(policy) = policies.policy_for(&identifier) else {
            transcript.facts.push(VerifiedCaveatEvaluationFact {
                policy_present: false,
                satisfied: false,
            });
            debug_assert_eq!(caveat_evaluation_first_failed_index(&transcript), transcript.facts.len() - 1);
            debug_assert!(!caveat_evaluation_transcript_accepts(&transcript));
            return Some(AuthorizationDenialReason::CaveatPolicyMissing { caveat: identifier });
        };
        match policy.evaluate(caveat, request) {
            CaveatDecision::Satisfied => {
                transcript.facts.push(VerifiedCaveatEvaluationFact {
                    policy_present: true,
                    satisfied: true,
                });
            }
            CaveatDecision::Rejected { message } => {
                transcript.facts.push(VerifiedCaveatEvaluationFact {
                    policy_present: true,
                    satisfied: false,
                });
                debug_assert_eq!(caveat_evaluation_first_failed_index(&transcript), transcript.facts.len() - 1);
                debug_assert!(!caveat_evaluation_transcript_accepts(&transcript));
                return Some(AuthorizationDenialReason::CaveatRejected {
                    caveat: identifier,
                    message,
                });
            }
        }
    }
    debug_assert!(caveat_evaluation_transcript_accepts(&transcript));
    None
}

#[derive(Serialize)]
struct EncodedHeader<'a> {
    alg: &'a str,
    typ: &'a str,
}

#[derive(Deserialize)]
struct DecodedHeader<'a> {
    #[serde(default, borrow)]
    alg: Option<&'a str>,
    #[serde(default, borrow)]
    typ: Option<&'a str>,
}

#[derive(Serialize)]
struct EncodedCapability<'a> {
    #[serde(rename = "with")]
    resource: &'a str,
    #[serde(rename = "can")]
    ability: &'a str,
    #[serde(default, skip_serializing_if = "Vec::is_empty", rename = "cav")]
    caveats: Vec<EncodedCaveat<'a>>,
}

#[derive(Serialize)]
struct EncodedCaveat<'a> {
    domain: &'a str,
    #[serde(rename = "type")]
    caveat_type: &'a str,
    key: &'a str,
    payload: EncodedBase64Url<'a>,
}

struct EncodedBase64Url<'a>(&'a [u8]);

impl Serialize for EncodedBase64Url<'_> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where S: serde::Serializer {
        let encoded = encode_base64_secret(self.0).map_err(|_| serde::ser::Error::custom("base64 encoding failed"))?;
        let text = std::str::from_utf8(encoded.as_slice())
            .map_err(|_| serde::ser::Error::custom("base64 output was not UTF-8"))?;
        serializer.serialize_str(text)
    }
}

#[derive(Deserialize)]
struct DecodedCapability<'a> {
    #[serde(default, borrow, rename = "with")]
    resource: Option<&'a str>,
    #[serde(default, borrow, rename = "can")]
    ability: Option<&'a str>,
    #[serde(default, borrow, rename = "cav")]
    caveats: Vec<DecodedCaveat<'a>>,
}

#[derive(Deserialize)]
struct DecodedCaveat<'a> {
    #[serde(default, borrow)]
    domain: Option<&'a str>,
    #[serde(default, borrow, rename = "type")]
    caveat_type: Option<&'a str>,
    #[serde(default, borrow)]
    key: Option<&'a str>,
    #[serde(default, borrow)]
    payload: Option<&'a str>,
}

#[derive(Serialize)]
struct EncodedClaims<'a> {
    iss: &'a str,
    aud: &'a str,
    att: Vec<EncodedCapability<'a>>,
    prf: Vec<EncodedBase64Url<'a>>,
    exp: u64,
    nbf: u64,
}

#[derive(Deserialize)]
struct DecodedClaims<'a> {
    #[serde(default, borrow)]
    iss: Option<&'a str>,
    #[serde(default, borrow)]
    aud: Option<&'a str>,
    #[serde(default, borrow)]
    att: Option<Vec<DecodedCapability<'a>>>,
    #[serde(default, borrow)]
    prf: Option<Vec<&'a str>>,
    #[serde(default)]
    exp: Option<u64>,
    #[serde(default)]
    nbf: Option<u64>,
}

/// Issue and sign a compact UCAN token with the compatibility signing-key request.
///
/// # Errors
///
/// Returns [`TokenError`] when signing, DID derivation, JSON encoding, or
/// structural validation fails.
pub fn issue_token(request: &IssueRequest) -> TokenResult<CompactToken> {
    issue_token_with_signer(
        &request.signing_key,
        &request.audience,
        &request.capabilities,
        &request.proofs,
        request.time_bounds,
    )
}

/// Issue and sign a compact UCAN token through a signer abstraction.
///
/// # Errors
///
/// Returns [`TokenError`] when the signer reports unsupported identity,
/// refuses to sign, returns malformed signature output, or when token encoding
/// or structural validation fails.
pub fn issue_token_with_signer<S>(
    signer: &S,
    audience: &AudienceDid,
    capabilities: &CapabilitySet,
    proofs: &ProofReferences,
    time_bounds: TokenTimeBounds,
) -> TokenResult<CompactToken>
where
    S: TokenSigner + ?Sized,
{
    let issuer = validate_signer_identity(signer)?;
    let input = IssueClaimsInput {
        issuer: &issuer,
        audience,
        capabilities,
        proofs,
        time_bounds,
    };
    issue_compact_token_with_signer(signer, &input)
}

/// Parse a compact token into decoded-but-unvalidated secret buffers.
///
/// # Errors
///
/// Returns [`TokenError`] when size, compact structure, base64url decoding, or
/// decoded JSON object admission fails.
pub fn parse_compact_token(token: &CompactToken) -> TokenResult<DecodedToken<'_>> {
    parse_compact_token_str_bounded(token.as_str(), VerificationLimits::default())
}

fn parse_compact_token_str_bounded(token: &str, limits: VerificationLimits) -> TokenResult<DecodedToken<'_>> {
    enforce_compact_token_size(token, limits)?;
    validate_compact_token_string(token)?;
    let parts = compact_parts(token)?;
    let reference = proof_reference_for_bounded_token(token);
    let header_json = decode_part(parts.header, CompactPart::Header)?;
    validate_decoded_json_object_bytes(header_json.as_slice(), CompactPart::Header)?;
    reject_json_escapes(header_json.as_slice(), CompactPart::Header)?;
    let claims_json = decode_part(parts.payload, CompactPart::Payload)?;
    validate_decoded_json_object_bytes(claims_json.as_slice(), CompactPart::Payload)?;
    reject_json_escapes(claims_json.as_slice(), CompactPart::Payload)?;
    let signature = decode_part(parts.signature, CompactPart::Signature)?;
    Ok(DecodedToken {
        reference,
        compact_byte_len: token.len(),
        header_json,
        claims_json,
        signature,
        signing_input: parts.signing_input.as_bytes(),
    })
}

/// Validate decoded token claims through `ucan-core`.
///
/// # Errors
///
/// Returns [`TokenError`] when the token header or decoded claim shape is invalid.
pub fn validate_decoded_token(decoded: DecodedToken<'_>) -> TokenResult<StructurallyValidToken<'_>> {
    let header = decode_json_bytes::<DecodedHeader<'_>>(decoded.header_json.as_slice(), CompactPart::Header)?;
    validate_header(&header)?;
    let claims = decode_json_bytes::<DecodedClaims<'_>>(decoded.claims_json.as_slice(), CompactPart::Payload)?;
    validate_parsed_claims_presence(&claims)?;
    validate_parsed_did_claims(&claims)?;
    validate_parsed_temporal_order(&claims)?;
    validate_parsed_capability_list_shape(&claims)?;
    let claims = build_core_claims(&claims)?;
    Ok(StructurallyValidToken {
        reference: decoded.reference,
        compact_byte_len: decoded.compact_byte_len,
        claims,
        signature: decoded.signature,
        signing_input: decoded.signing_input,
    })
}

/// Verify the signature on a structurally valid token.
///
/// # Errors
///
/// Returns [`TokenError`] when key resolution fails or the signature is invalid.
pub fn verify_token_signature<'a>(
    token: StructurallyValidToken<'a>,
    keys: &KeyResolutionContext,
) -> TokenResult<SignatureVerifiedToken<'a>> {
    verify_token_signature_with_resolver(token, keys)
}

/// Verify the signature on a structurally valid token through an explicit key resolver.
///
/// # Errors
///
/// Returns [`TokenError`] when key resolution fails or the signature is invalid.
pub fn verify_token_signature_with_resolver<'a, R>(
    token: StructurallyValidToken<'a>,
    resolver: &R,
) -> TokenResult<SignatureVerifiedToken<'a>>
where
    R: KeyResolver + ?Sized,
{
    let issuer = IssuerDid::new(token.claims.issuer_did().to_owned())?;
    let public_key = resolver.resolve_key(&issuer).map_err(|source| match source {
        KeyResolverError::MissingKey { issuer } => TokenError::MissingVerificationKey { issuer },
        source => TokenError::KeyResolver { source },
    })?;
    let valid = verify_ed25519_signature(public_key.as_bytes(), token.signing_input, token.signature.as_slice())?;
    if !valid {
        return Err(TokenError::Signature {
            issuer: issuer.as_str().to_owned(),
        });
    }
    Ok(SignatureVerifiedToken { token })
}

fn verify_token_signature_with_traversal<'a, R>(
    token: StructurallyValidToken<'a>,
    resolver: &R,
    traversal: &mut VerificationTraversal,
) -> TokenResult<SignatureVerifiedToken<'a>>
where
    R: KeyResolver + ?Sized,
{
    let issuer = IssuerDid::new(token.claims.issuer_did().to_owned())?;
    let public_key = match resolver.resolve_key(&issuer) {
        Ok(public_key) => {
            traversal.record_backend_operation(VerifiedBackendOperation::KeyResolution, true);
            public_key
        }
        Err(source) => {
            traversal.record_backend_operation(VerifiedBackendOperation::KeyResolution, false);
            return Err(match source {
                KeyResolverError::MissingKey { issuer } => TokenError::MissingVerificationKey { issuer },
                source => TokenError::KeyResolver { source },
            });
        }
    };
    let valid = verify_ed25519_signature(public_key.as_bytes(), token.signing_input, token.signature.as_slice())?;
    if !valid {
        return Err(TokenError::Signature {
            issuer: issuer.as_str().to_owned(),
        });
    }
    Ok(SignatureVerifiedToken { token })
}

/// Verify a compact token from parse through authorization.
///
/// # Errors
///
/// Returns [`TokenError`] when parsing, structural validation, signature
/// validation, temporal validation, or proof-chain validation fails.
pub fn verify_compact_token(token: &CompactToken, context: &VerificationContext) -> TokenResult<VerifiedToken> {
    verify_compact_token_str(token.as_str(), context)
}

/// Verify borrowed compact-token text without creating an owned bearer copy.
///
/// # Errors
///
/// Returns [`TokenError`] when size limits, parsing, structural validation,
/// signature validation, temporal validation, or proof-chain validation fails.
pub fn verify_compact_token_str(token: &str, context: &VerificationContext) -> TokenResult<VerifiedToken> {
    verify_compact_token_str_with_resolvers_and_revocations(
        token,
        context.time,
        &context.keys,
        &context.proofs,
        &NoRevocations,
        context.limits,
    )
}

/// Verify a compact-token owner through explicit resolver and proof-store inputs.
///
/// # Errors
///
/// Returns [`TokenError`] when any verification stage fails.
pub fn verify_compact_token_with_resolvers<R, S>(
    token: &CompactToken,
    time: VerificationTime,
    key_resolver: &R,
    proof_store: &S,
    limits: VerificationLimits,
) -> TokenResult<VerifiedToken>
where
    R: KeyResolver + ?Sized,
    S: ProofStore + ?Sized,
{
    verify_compact_token_str_with_resolvers(token.as_str(), time, key_resolver, proof_store, limits)
}

/// Verify borrowed compact-token text through explicit resolver and proof-store inputs.
///
/// # Errors
///
/// Returns [`TokenError`] when any verification stage fails.
pub fn verify_compact_token_str_with_resolvers<R, S>(
    token: &str,
    time: VerificationTime,
    key_resolver: &R,
    proof_store: &S,
    limits: VerificationLimits,
) -> TokenResult<VerifiedToken>
where
    R: KeyResolver + ?Sized,
    S: ProofStore + ?Sized,
{
    verify_compact_token_str_with_resolvers_and_revocations(
        token,
        time,
        key_resolver,
        proof_store,
        &NoRevocations,
        limits,
    )
}

/// Verify a compact-token owner with explicit resolver, proof-store, and revocation inputs.
///
/// # Errors
///
/// Returns [`TokenError`] when any verification stage fails.
pub fn verify_compact_token_with_resolvers_and_revocations<R, S, V>(
    token: &CompactToken,
    time: VerificationTime,
    key_resolver: &R,
    proof_store: &S,
    revocations: &V,
    limits: VerificationLimits,
) -> TokenResult<VerifiedToken>
where
    R: KeyResolver + ?Sized,
    S: ProofStore + ?Sized,
    V: RevocationChecker + ?Sized,
{
    verify_compact_token_str_with_resolvers_and_revocations(
        token.as_str(),
        time,
        key_resolver,
        proof_store,
        revocations,
        limits,
    )
}

/// Verify borrowed compact-token text with every external truth supplied explicitly.
///
/// # Errors
///
/// Returns [`TokenError`] when any verification stage fails.
pub fn verify_compact_token_str_with_resolvers_and_revocations<R, S, V>(
    token: &str,
    time: VerificationTime,
    key_resolver: &R,
    proof_store: &S,
    revocations: &V,
    limits: VerificationLimits,
) -> TokenResult<VerifiedToken>
where
    R: KeyResolver + ?Sized,
    S: ProofStore + ?Sized,
    V: RevocationChecker + ?Sized,
{
    let mut traversal = VerificationTraversal::default();
    let verified = verify_compact_token_with_traversal(
        token,
        time,
        key_resolver,
        proof_store,
        revocations,
        limits,
        &mut traversal,
        0,
    )?;
    traversal.verify_backend_transcript_accepts()?;
    verify_successful_verification_stage_flow()?;
    Ok(verified)
}

fn verify_successful_verification_stage_flow() -> TokenResult<()> {
    let transcript = VerifiedVerificationStageTranscript {
        parse_ok: true,
        structural_ok: true,
        key_resolved: true,
        signature_valid: true,
        envelope_valid: true,
        revocation_clear: true,
        proofs_loaded: true,
        proof_chain_valid: true,
        authorized: true,
    };
    let outcome = verification_stage_transcript_outcome(&transcript);
    if outcome.accepted && outcome.first_failed_stage == VerifiedVerificationStage::Accepted {
        return Ok(());
    }
    Err(TokenError::VerifiedPrimitive {
        source: VerifiedPrimitiveError::TokenValidation {
            failure: TokenValidationFailure::InvalidVerificationStageFlow,
        },
    })
}

fn verify_compact_token_with_traversal<R, S, V>(
    token: &str,
    time: VerificationTime,
    key_resolver: &R,
    proof_store: &S,
    revocations: &V,
    limits: VerificationLimits,
    traversal: &mut VerificationTraversal,
    depth: usize,
) -> TokenResult<VerifiedToken>
where
    R: KeyResolver + ?Sized,
    S: ProofStore + ?Sized,
    V: RevocationChecker + ?Sized,
{
    enforce_proof_depth(depth, limits)?;
    enforce_compact_token_size(token, limits)?;
    validate_compact_token_string(token)?;
    reject_revoked_token(token, revocations, traversal)?;
    reject_cycle(token, traversal.trail.as_slice())?;
    traversal.trail.push(proof_reference_for_bounded_token(token));
    let decoded = parse_compact_token_str_bounded(token, limits)?;
    let valid = validate_decoded_token(decoded)?;
    let signed = verify_token_signature_with_traversal(valid, key_resolver, traversal)?;
    let verified =
        authorize_signed_token(signed, time, key_resolver, proof_store, revocations, limits, traversal, depth)?;
    let _removed = traversal.trail.pop();
    Ok(verified)
}

fn authorize_signed_token<R, S, V>(
    signed: SignatureVerifiedToken<'_>,
    time: VerificationTime,
    key_resolver: &R,
    proof_store: &S,
    revocations: &V,
    limits: VerificationLimits,
    traversal: &mut VerificationTraversal,
    depth: usize,
) -> TokenResult<VerifiedToken>
where
    R: KeyResolver + ?Sized,
    S: ProofStore + ?Sized,
    V: RevocationChecker + ?Sized,
{
    let token = signed.token;
    validate_envelope_for_token(&token, time, limits)?;
    let issuer = IssuerDid::new(token.claims.issuer_did().to_owned())?;
    let audience = AudienceDid::new(token.claims.audience_did().to_owned())?;
    let capabilities = claims_capability_set(&token.claims)?;
    let time_bounds = claims_time_bounds(&token.claims)?;
    let token_reference = token.reference.clone();
    let proofs = direct_proof_references(&token, limits)?;
    let evaluation = effective_delegation(
        &issuer,
        &capabilities,
        time_bounds,
        &proofs,
        time,
        key_resolver,
        proof_store,
        revocations,
        limits,
        traversal,
        depth,
    )?;
    let proof_transcript = proof_transcript_for_verified_token(
        &token_reference,
        token.compact_byte_len,
        !proofs.as_slice().is_empty(),
        &issuer,
        &audience,
        &capabilities,
        time_bounds,
        evaluation.parent_proof_transcript.as_ref(),
    )?;
    let verified = VerifiedToken {
        issuer,
        audience,
        capabilities,
        proofs,
        effective: evaluation.effective,
        proof_transcript,
    };
    verify_live_dag_proof_transcript(&verified, time, limits)?;
    Ok(verified)
}

#[allow(clippy::too_many_arguments)]
fn effective_delegation<R, S, V>(
    child_issuer: &IssuerDid,
    capabilities: &CapabilitySet,
    time_bounds: TokenTimeBounds,
    proofs: &ProofReferences,
    time: VerificationTime,
    key_resolver: &R,
    proof_store: &S,
    revocations: &V,
    limits: VerificationLimits,
    traversal: &mut VerificationTraversal,
    depth: usize,
) -> TokenResult<EffectiveDelegationEvaluation>
where
    R: KeyResolver + ?Sized,
    S: ProofStore + ?Sized,
    V: RevocationChecker + ?Sized,
{
    if proofs.as_slice().is_empty() {
        return Ok(EffectiveDelegationEvaluation {
            effective: EffectiveDelegation {
                capabilities: capabilities.clone(),
                time_bounds,
            },
            parent_proof_transcript: None,
        });
    }
    let parent_tokens =
        verify_parent_tokens(proofs, time, key_resolver, proof_store, revocations, limits, traversal, depth + 1)?;
    verify_child_is_linked_to_parent_audience(child_issuer, &parent_tokens)?;
    let effective_time = verify_temporal_attenuation(time_bounds, parent_tokens.as_slice())?;
    let effective_capabilities = verify_capability_attenuation(capabilities, parent_tokens.as_slice())?;
    Ok(EffectiveDelegationEvaluation {
        effective: EffectiveDelegation {
            capabilities: effective_capabilities,
            time_bounds: effective_time,
        },
        parent_proof_transcript: parent_proof_transcript(parent_tokens.as_slice()),
    })
}

fn verify_parent_tokens<R, S, V>(
    proofs: &ProofReferences,
    time: VerificationTime,
    key_resolver: &R,
    proof_store: &S,
    revocations: &V,
    limits: VerificationLimits,
    traversal: &mut VerificationTraversal,
    depth: usize,
) -> TokenResult<Vec<VerifiedToken>>
where
    R: KeyResolver + ?Sized,
    S: ProofStore + ?Sized,
    V: RevocationChecker + ?Sized,
{
    let mut parents = Vec::with_capacity(proofs.as_slice().len());
    for reference in proofs.as_slice() {
        traversal.resolve_one_proof(limits)?;
        let proof = match proof_store.resolve_proof(reference) {
            Ok(proof) => {
                let result = VerifiedProofStoreResultFact {
                    reference_non_empty: !reference.as_bytes().is_empty(),
                    backend_succeeded: true,
                    compact_token_present: !proof.as_str().is_empty(),
                };
                if !proof_store_result_is_valid(result) {
                    return Err(TokenError::VerifiedPrimitive {
                        source: VerifiedPrimitiveError::TokenValidation {
                            failure: TokenValidationFailure::InvalidProofStoreResultTranscript,
                        },
                    });
                }
                traversal.record_backend_operation(VerifiedBackendOperation::ProofLoad, true);
                proof
            }
            Err(source) => {
                traversal.record_backend_operation(VerifiedBackendOperation::ProofLoad, false);
                return Err(TokenError::ProofStore { source });
            }
        };
        let proof_text = proof.as_str();
        enforce_compact_token_size(proof_text, limits)?;
        bind_proof_to_reference(proof_text, reference)?;
        let parent = verify_compact_token_with_traversal(
            proof_text,
            time,
            key_resolver,
            proof_store,
            revocations,
            limits,
            traversal,
            depth,
        )?;
        parents.push(parent);
    }
    Ok(parents)
}

fn bind_proof_to_reference(token: &str, requested: &ProofReference) -> TokenResult<()> {
    let actual = proof_reference_for_bounded_token(token);
    if actual.as_bytes() != requested.as_bytes() {
        return Err(TokenError::ProofStore {
            source: ProofStoreError::ReferenceMismatch {
                expected: requested.clone(),
                actual,
            },
        });
    }
    Ok(())
}

fn verify_child_is_linked_to_parent_audience(child_issuer: &IssuerDid, parents: &[VerifiedToken]) -> TokenResult<()> {
    for parent in parents {
        if !proof_link_is_valid(parent.audience().as_str(), child_issuer.as_str()) {
            return Err(TokenError::ProofChain {
                message: "child issuer does not match parent audience".to_owned(),
            });
        }
    }
    Ok(())
}

fn verify_temporal_attenuation(
    child_bounds: TokenTimeBounds,
    parents: &[VerifiedToken],
) -> TokenResult<TokenTimeBounds> {
    let mut windows = Vec::with_capacity(parents.len() + 1);
    for parent in parents {
        windows.push(parent.effective_delegation().time_bounds().to_verified());
    }
    windows.push(child_bounds.to_verified());
    let composed = compose_time_window_chain(windows.as_slice())?;
    if composed.not_before_seconds != child_bounds.not_before_seconds()
        || composed.expires_at_seconds != child_bounds.expires_at_seconds()
    {
        return Err(TokenError::Temporal {
            message: "child time window is wider than its proof chain".to_owned(),
        });
    }
    Ok(child_bounds)
}

fn verify_capability_attenuation(child: &CapabilitySet, parents: &[VerifiedToken]) -> TokenResult<CapabilitySet> {
    let parent_capabilities = parent_effective_capabilities(parents);
    for child_capability in child.as_slice() {
        if !capability_is_delegated(child_capability, parent_capabilities.as_slice())? {
            return Err(TokenError::Attenuation {
                resource: child_capability.resource.clone(),
                ability: child_capability.ability.clone(),
            });
        }
    }
    Ok(child.clone())
}

fn capability_is_delegated(child: &CapabilityDocument, parents: &[CapabilityDocument]) -> TokenResult<bool> {
    let child_verified = VerifiedCapability::from_document(child.clone())?;
    for parent in parents {
        let parent_verified = VerifiedCapability::from_document(parent.clone())?;
        if parent_verified.contains(&child_verified) && parent_caveats_are_preserved(parent, child)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn parent_caveats_are_preserved(parent: &CapabilityDocument, child: &CapabilityDocument) -> TokenResult<bool> {
    let preservation = VerifiedCaveatPreservation {
        parent_caveat_ids: caveat_identifier_set(parent)?,
        child_caveat_ids: caveat_identifier_set(child)?,
    };
    Ok(caveat_preservation_is_valid(&preservation))
}

fn caveat_identifier_set(capability: &CapabilityDocument) -> TokenResult<Vec<Vec<u8>>> {
    capability.caveats.iter().map(caveat_identifier_bytes).collect()
}

fn parent_effective_capabilities(parents: &[VerifiedToken]) -> Vec<CapabilityDocument> {
    parents
        .iter()
        .flat_map(|parent| parent.effective_delegation().capabilities().as_slice().iter().cloned())
        .collect()
}

fn parent_proof_transcript(parents: &[VerifiedToken]) -> Option<LiveProofTraversalTranscript> {
    let mut entries = Vec::new();
    let mut parent_indices = Vec::new();
    let mut leaf_indices = Vec::new();
    let mut total_input_bytes = 0_u64;
    for parent in parents {
        let parent_transcript = parent.proof_transcript.as_ref()?;
        let offset = entries.len();
        entries.extend(parent_transcript.entries.iter().cloned());
        parent_indices.extend(
            parent_transcript
                .parent_indices
                .iter()
                .map(|indices| indices.iter().map(|index| index.saturating_add(offset)).collect()),
        );
        leaf_indices.extend(parent_transcript.leaf_indices.iter().map(|index| index.saturating_add(offset)));
        total_input_bytes = total_input_bytes.saturating_add(parent_transcript.total_input_bytes);
    }
    Some(LiveProofTraversalTranscript {
        entries,
        parent_indices,
        leaf_indices,
        total_input_bytes,
    })
}

fn proof_transcript_for_verified_token(
    proof_reference: &ProofReference,
    compact_token_bytes: usize,
    has_proofs: bool,
    issuer: &IssuerDid,
    audience: &AudienceDid,
    capabilities: &CapabilitySet,
    time_bounds: TokenTimeBounds,
    parent_transcript: Option<&LiveProofTraversalTranscript>,
) -> TokenResult<Option<LiveProofTraversalTranscript>> {
    let token_bytes = u64::try_from(compact_token_bytes).unwrap_or(u64::MAX);
    let parent_caveat_ids = parent_transcript
        .and_then(|transcript| transcript.entries.last())
        .map(|entry| entry.caveat_ids.as_slice());
    let entry = proof_traversal_entry(proof_reference, issuer, audience, capabilities, time_bounds, parent_caveat_ids)?;
    let Some(parent_transcript) = parent_transcript else {
        if has_proofs {
            return Ok(None);
        }
        return Ok(Some(LiveProofTraversalTranscript {
            entries: vec![entry],
            parent_indices: vec![Vec::new()],
            leaf_indices: vec![0],
            total_input_bytes: token_bytes,
        }));
    };
    let mut entries = parent_transcript.entries.clone();
    let mut parent_indices = parent_transcript.parent_indices.clone();
    let child_index = entries.len();
    entries.push(entry);
    parent_indices.push(parent_transcript.leaf_indices.clone());
    Ok(Some(LiveProofTraversalTranscript {
        entries,
        parent_indices,
        leaf_indices: vec![child_index],
        total_input_bytes: parent_transcript.total_input_bytes.saturating_add(token_bytes),
    }))
}

fn proof_traversal_entry(
    proof_reference: &ProofReference,
    issuer: &IssuerDid,
    audience: &AudienceDid,
    capabilities: &CapabilitySet,
    time_bounds: TokenTimeBounds,
    parent_caveat_ids: Option<&[Vec<u8>]>,
) -> TokenResult<VerifiedProofTraversalEntry> {
    Ok(VerifiedProofTraversalEntry {
        proof_reference: proof_reference.as_bytes().to_vec(),
        issuer_did: issuer.as_str().to_owned(),
        audience_did: audience.as_str().to_owned(),
        capabilities: capabilities.as_slice().to_vec(),
        caveat_ids: effective_caveat_ids(capabilities, parent_caveat_ids)?,
        not_before_seconds: time_bounds.not_before_seconds(),
        expires_at_seconds: time_bounds.expires_at_seconds(),
        signature_valid: true,
        revoked: false,
    })
}

fn effective_caveat_ids(
    capabilities: &CapabilitySet,
    parent_caveat_ids: Option<&[Vec<u8>]>,
) -> TokenResult<Vec<Vec<u8>>> {
    let mut caveat_ids = parent_caveat_ids.map_or_else(Vec::new, <[Vec<u8>]>::to_vec);
    for capability in capabilities.as_slice() {
        for caveat in capability.caveats.as_slice() {
            let caveat_id = caveat_identifier_bytes(caveat)?;
            if !caveat_ids.iter().any(|existing| existing == &caveat_id) {
                caveat_ids.push(caveat_id);
            }
        }
    }
    Ok(caveat_ids)
}

fn caveat_identifier_bytes(caveat: &CaveatDocument) -> TokenResult<Vec<u8>> {
    let mut bytes = Vec::new();
    append_length_prefixed(caveat.domain.as_bytes(), &mut bytes)?;
    append_length_prefixed(caveat.caveat_type.as_bytes(), &mut bytes)?;
    append_length_prefixed(caveat.key.as_bytes(), &mut bytes)?;
    append_length_prefixed(caveat.payload(), &mut bytes)?;
    Ok(bytes)
}

fn append_length_prefixed(input: &[u8], output: &mut Vec<u8>) -> TokenResult<()> {
    let len = u64::try_from(input.len()).map_err(|_| TokenError::ProofChain {
        message: "caveat identifier component exceeds transcript length encoding".to_owned(),
    })?;
    output.extend_from_slice(&len.to_be_bytes());
    output.extend_from_slice(input);
    Ok(())
}

fn verify_live_dag_proof_transcript(
    verified: &VerifiedToken,
    time: VerificationTime,
    limits: VerificationLimits,
) -> TokenResult<()> {
    let Some(transcript) = &verified.proof_transcript else {
        return Ok(());
    };
    let Some(target_capability) = verified.capabilities().as_slice().first() else {
        return Err(TokenError::ProofChain {
            message: "verified proof traversal transcript has no authorization target".to_owned(),
        });
    };
    let target = VerifiedProofTraversalTarget {
        resource: target_capability.resource.clone(),
        ability: target_capability.ability.clone(),
        now_seconds: time.as_unix_seconds(),
        tolerance_seconds: limits.clock_skew_tolerance_seconds(),
    };
    let bridge_transcript = VerifiedProofTraversalDagTranscript {
        entries: transcript.entries.clone(),
        parent_indices: transcript.parent_indices.clone(),
        visited_references: transcript.entries.iter().map(|entry| entry.proof_reference.clone()).collect(),
        total_input_bytes: transcript.total_input_bytes,
        target,
    };
    let budget = VerifiedProofTraversalBudget {
        max_depth: u64::try_from(limits.max_proof_depth()).unwrap_or(u64::MAX),
        max_proofs: u64::try_from(limits.max_proof_count()).unwrap_or(u64::MAX),
        max_input_bytes: u64::try_from(limits.max_compact_token_bytes()).unwrap_or(u64::MAX),
    };
    if !proof_traversal_dag_transcript_is_valid(&bridge_transcript, &budget)? {
        return Err(TokenError::ProofChain {
            message: "verified proof traversal DAG transcript rejected live proof graph".to_owned(),
        });
    }
    Ok(())
}

fn validate_proof_reference_collection(
    references: &[ucan_core::token::ProofReference],
    limits: VerificationLimits,
) -> TokenResult<()> {
    let collection = VerifiedProofReferenceCollection {
        count: references.len(),
        max_count: limits.max_proof_count(),
        all_references_non_empty: references.iter().all(|reference| !reference.as_bytes().is_empty()),
    };
    if proof_reference_collection_is_valid(collection) {
        return Ok(());
    }
    if references.len() > limits.max_proof_count() {
        return Err(TokenError::LimitExceeded {
            source: VerificationLimitError::ProofCountExceeded {
                count: references.len(),
                max_count: limits.max_proof_count(),
            },
        });
    }
    Err(TokenError::VerifiedPrimitive {
        source: VerifiedPrimitiveError::TokenValidation {
            failure: TokenValidationFailure::InvalidProofReferenceCollection,
        },
    })
}

fn direct_proof_references(
    token: &StructurallyValidToken<'_>,
    limits: VerificationLimits,
) -> TokenResult<ProofReferences> {
    validate_proof_reference_collection(token.claims.proofs(), limits)?;
    let references = token
        .claims
        .proofs()
        .iter()
        .map(|reference| ProofReference::new(reference.as_bytes().to_vec()))
        .collect();
    Ok(ProofReferences::from_references(references))
}

fn validate_envelope_for_token(
    token: &StructurallyValidToken<'_>,
    time: VerificationTime,
    limits: VerificationLimits,
) -> TokenResult<()> {
    let envelope = token_envelope(token)?;
    validate_token_envelope(&envelope, true, time.as_unix_seconds(), limits.clock_skew_tolerance_seconds())
        .map_err(|error| map_envelope_error(error, token.claims.issuer_did()))
}

fn map_envelope_error(error: VerifiedPrimitiveError, issuer_did: &str) -> TokenError {
    match error {
        VerifiedPrimitiveError::TokenValidation { failure } => match failure {
            TokenValidationFailure::ExpiredToken | TokenValidationFailure::NotYetValid => TokenError::Temporal {
                message: failure.to_string(),
            },
            TokenValidationFailure::SelfDelegation => TokenError::SelfDelegation {
                did: issuer_did.to_owned(),
            },
            TokenValidationFailure::InvalidSignature => TokenError::Signature {
                issuer: "unknown".to_owned(),
            },
            TokenValidationFailure::MissingIssuer
            | TokenValidationFailure::MissingAudience
            | TokenValidationFailure::MissingAttenuations
            | TokenValidationFailure::MissingProofs
            | TokenValidationFailure::MissingExpiration
            | TokenValidationFailure::MissingNotBefore
            | TokenValidationFailure::InvalidTemporalOrder
            | TokenValidationFailure::InvalidDidClaims
            | TokenValidationFailure::InvalidProofReferenceListShape
            | TokenValidationFailure::InvalidProofReferenceCollection
            | TokenValidationFailure::InvalidCapabilityListShape
            | TokenValidationFailure::InvalidCaveatListShape
            | TokenValidationFailure::InvalidCaveatPayloadAdmission
            | TokenValidationFailure::InvalidProofStoreResultTranscript
            | TokenValidationFailure::InvalidRevocationCheckTranscript
            | TokenValidationFailure::InvalidReplayAdmissionTranscript
            | TokenValidationFailure::InvalidSigningInputProvenance
            | TokenValidationFailure::InvalidVerificationStageFlow => TokenError::VerifiedPrimitive {
                source: VerifiedPrimitiveError::TokenValidation { failure },
            },
        },
        other => TokenError::VerifiedPrimitive { source: other },
    }
}

fn token_envelope(token: &StructurallyValidToken<'_>) -> TokenResult<TokenEnvelopeInput> {
    Ok(TokenEnvelopeInput {
        issuer_did: token.claims.issuer_did().to_owned(),
        audience_did: token.claims.audience_did().to_owned(),
        capabilities: claims_capabilities(&token.claims)?,
        proofs: token.claims.proofs().iter().map(|proof| proof.as_bytes().to_vec()).collect(),
        expires_at_seconds: token.claims.time_bounds().expires_at_seconds(),
        not_before_seconds: token.claims.time_bounds().not_before_seconds(),
    })
}

fn claims_capability_set(claims: &ucan_core::token::Claims) -> TokenResult<CapabilitySet> {
    CapabilitySet::new(claims_capabilities(claims)?)
}

fn claims_capabilities(claims: &ucan_core::token::Claims) -> TokenResult<Vec<CapabilityDocument>> {
    claims
        .capabilities()
        .iter()
        .map(|capability| {
            let caveats = capability
                .caveats()
                .iter()
                .map(|caveat| {
                    CaveatDocument::new(
                        caveat.domain().to_owned(),
                        caveat.caveat_type().to_owned(),
                        caveat.key().to_owned(),
                        caveat.payload().to_vec(),
                    )
                })
                .collect::<crate::Result<Vec<_>>>()?;
            CapabilityDocument::with_caveats(capability.resource().to_owned(), capability.ability().to_owned(), caveats)
        })
        .collect::<crate::Result<Vec<_>>>()
        .map_err(|error| TokenError::Capability {
            message: error.to_string(),
        })
}

const fn claims_time_bounds(claims: &ucan_core::token::Claims) -> TokenResult<TokenTimeBounds> {
    TokenTimeBounds::new(claims.time_bounds().not_before_seconds(), claims.time_bounds().expires_at_seconds())
}

#[derive(Debug, Default)]
struct VerificationTraversal {
    trail: Vec<ProofReference>,
    resolved_proofs: usize,
    backend_operations: Vec<VerifiedBackendOperationFact>,
}

impl VerificationTraversal {
    fn record_backend_operation(&mut self, operation: VerifiedBackendOperation, succeeded: bool) {
        self.backend_operations.push(VerifiedBackendOperationFact { operation, succeeded });
    }

    fn verify_backend_transcript_accepts(&self) -> TokenResult<()> {
        if self.backend_operations.is_empty() {
            return Ok(());
        }
        let transcript = VerifiedBackendTranscript {
            operations: self.backend_operations.clone(),
        };
        if !backend_transcript_accepts(&transcript) {
            return Err(TokenError::BackendTranscript {
                message: format!(
                    "first failed operation: {:?}",
                    backend_transcript_first_failed_operation(&transcript)
                ),
            });
        }
        Ok(())
    }

    const fn resolve_one_proof(&mut self, limits: VerificationLimits) -> TokenResult<()> {
        let next_count = self.resolved_proofs.saturating_add(1);
        if next_count > limits.max_proof_count() {
            return Err(TokenError::LimitExceeded {
                source: VerificationLimitError::ProofCountExceeded {
                    count: next_count,
                    max_count: limits.max_proof_count(),
                },
            });
        }
        self.resolved_proofs = next_count;
        Ok(())
    }
}

fn enforce_proof_depth(depth: usize, limits: VerificationLimits) -> TokenResult<()> {
    if !proof_depth_within_budget(depth, limits.max_proof_depth()) {
        return Err(TokenError::LimitExceeded {
            source: VerificationLimitError::ProofDepthExceeded {
                depth,
                max_depth: limits.max_proof_depth(),
            },
        });
    }
    Ok(())
}

const fn enforce_compact_token_size(token: &str, limits: VerificationLimits) -> TokenResult<()> {
    let byte_len = token.len();
    if byte_len > limits.max_compact_token_bytes() {
        return Err(TokenError::LimitExceeded {
            source: VerificationLimitError::CompactTokenTooLarge {
                byte_len,
                max_byte_len: limits.max_compact_token_bytes(),
            },
        });
    }
    Ok(())
}

fn reject_revoked_token<V>(token: &str, revocations: &V, traversal: &mut VerificationTraversal) -> TokenResult<()>
where V: RevocationChecker + ?Sized {
    let reference = proof_reference_for_bounded_token(token);
    let revoked = match revocations.is_revoked(&reference) {
        Ok(revoked) => {
            traversal.record_backend_operation(VerifiedBackendOperation::Revocation, true);
            revoked
        }
        Err(source) => {
            traversal.record_backend_operation(VerifiedBackendOperation::Revocation, false);
            return Err(TokenError::Revocation { source });
        }
    };
    let check = VerifiedRevocationCheckFact {
        reference_non_empty: !reference.as_bytes().is_empty(),
        backend_succeeded: true,
        not_revoked: !revoked,
    };
    if !revocation_check_is_valid(check) {
        if revoked {
            return Err(TokenError::Revoked { reference });
        }
        return Err(TokenError::VerifiedPrimitive {
            source: VerifiedPrimitiveError::TokenValidation {
                failure: TokenValidationFailure::InvalidRevocationCheckTranscript,
            },
        });
    }
    Ok(())
}

fn reject_cycle(token: &str, trail: &[ProofReference]) -> TokenResult<()> {
    let current = proof_reference_for_bounded_token(token);
    if trail.iter().any(|reference| reference == &current) {
        return Err(TokenError::ProofChain {
            message: "proof chain contains a cycle".to_owned(),
        });
    }
    Ok(())
}

fn assert_issued_token_is_structurally_valid(
    token: &CompactToken,
    proof_references: &[ProofReference],
) -> TokenResult<()> {
    let decoded = parse_compact_token(token)?;
    let valid = validate_decoded_token(decoded)?;
    let actual = valid.claims().proofs();
    let references_match = actual.len() == proof_references.len()
        && actual
            .iter()
            .zip(proof_references)
            .all(|(actual, expected)| actual.as_bytes() == expected.as_bytes());
    if !references_match {
        return Err(TokenError::ProofChain {
            message: "issued proof references did not round-trip through core validation".to_owned(),
        });
    }
    Ok(())
}

struct IssueClaimsInput<'a> {
    issuer: &'a IssuerDid,
    audience: &'a AudienceDid,
    capabilities: &'a CapabilitySet,
    proofs: &'a ProofReferences,
    time_bounds: TokenTimeBounds,
}

fn issue_compact_token_with_signer<S>(signer: &S, input: &IssueClaimsInput<'_>) -> TokenResult<CompactToken>
where S: TokenSigner + ?Sized {
    let header = EncodedHeader {
        alg: TOKEN_ALGORITHM_EDDSA,
        typ: TOKEN_TYPE_JWT,
    };
    let claims = encoded_claims(input);
    let header_json = encode_json_exact(&header)?;
    reject_json_escapes(header_json.as_slice(), CompactPart::Header)?;
    let payload_json = encode_json_exact(&claims)?;
    reject_json_escapes(payload_json.as_slice(), CompactPart::Payload)?;
    let encoded_header = encode_base64_secret(header_json.as_slice())?;
    let encoded_payload = encode_base64_secret(payload_json.as_slice())?;
    let signing_input = join_secret_parts(&[encoded_header.as_slice(), encoded_payload.as_slice()])?;
    let signature = signer.sign(signing_input.as_slice())?;
    validate_signer_signature(&signature)?;
    let encoded_signature = encode_base64_secret(signature.as_bytes())?;
    let token_bytes = join_secret_parts(&[
        encoded_header.as_slice(),
        encoded_payload.as_slice(),
        encoded_signature.as_slice(),
    ])?;
    let token = CompactToken::from_secret_bytes(token_bytes)?;
    assert_issued_token_is_structurally_valid(&token, input.proofs.as_slice())?;
    Ok(token)
}

fn validate_signer_identity<S>(signer: &S) -> TokenResult<IssuerDid>
where S: TokenSigner + ?Sized {
    validate_signer_algorithm(signer.algorithm())?;
    let issuer = signer.issuer()?;
    let public_key_issuer = signer.public_key()?.issuer()?;
    if issuer != public_key_issuer {
        return Err(SignerError::IssuerPublicKeyMismatch {
            issuer: issuer.as_str().to_owned(),
            public_key_issuer: public_key_issuer.as_str().to_owned(),
        }
        .into());
    }
    Ok(issuer)
}

fn validate_signer_algorithm(algorithm: &str) -> SignerResult<()> {
    if signer_algorithm_is_valid(algorithm) {
        return Ok(());
    }
    Err(SignerError::UnsupportedAlgorithm {
        algorithm: algorithm.to_owned(),
    })
}

fn validate_signer_signature(signature: &SignerSignature) -> SignerResult<()> {
    let actual = signature.as_bytes().len();
    if actual == ED25519_SIGNATURE_BYTES {
        return Ok(());
    }
    Err(SignerError::InvalidSignatureLength {
        actual,
        expected: ED25519_SIGNATURE_BYTES,
    })
}

fn encoded_claims<'a>(input: &'a IssueClaimsInput<'a>) -> EncodedClaims<'a> {
    EncodedClaims {
        iss: input.issuer.as_str(),
        aud: input.audience.as_str(),
        att: input
            .capabilities
            .as_slice()
            .iter()
            .map(|capability| EncodedCapability {
                resource: capability.resource.as_str(),
                ability: capability.ability.as_str(),
                caveats: capability
                    .caveats
                    .iter()
                    .map(|caveat| EncodedCaveat {
                        domain: caveat.domain.as_str(),
                        caveat_type: caveat.caveat_type.as_str(),
                        key: caveat.key.as_str(),
                        payload: EncodedBase64Url(caveat.payload.as_slice()),
                    })
                    .collect(),
            })
            .collect(),
        prf: input.proofs.as_slice().iter().map(|reference| EncodedBase64Url(reference.as_bytes())).collect(),
        exp: input.time_bounds.expires_at_seconds(),
        nbf: input.time_bounds.not_before_seconds(),
    }
}

#[must_use]
pub fn proof_reference(token: &CompactToken) -> ProofReference {
    proof_reference_for_bounded_token(token.as_str())
}

fn proof_reference_for_bounded_token(token: &str) -> ProofReference {
    let digest = blake3::hash(token.as_bytes());
    let bytes = digest.as_bytes();
    debug_assert_eq!(bytes.len(), PROOF_REFERENCE_DIGEST_LEN);
    let reference = ProofReference::new(bytes.to_vec());
    let verified_digest = VerifiedProofReferenceDigest { bytes: bytes.to_vec() };
    let admitted = proof_reference_digest_is_valid(reference.as_bytes(), &verified_digest);
    debug_assert!(admitted);
    reference
}

struct CompactParts<'a> {
    header: &'a str,
    payload: &'a str,
    signature: &'a str,
    signing_input: &'a str,
}

fn compact_parts(token: &str) -> TokenResult<CompactParts<'_>> {
    let actual_parts = token.bytes().filter(|byte| *byte == COMPACT_SEPARATOR_BYTE).count().saturating_add(1);
    if actual_parts != COMPACT_TOKEN_PART_COUNT {
        return Err(TokenError::InvalidCompactTokenStructure { actual_parts });
    }
    let Some((header, remaining)) = token.split_once(COMPACT_SEPARATOR) else {
        return Err(TokenError::InvalidCompactTokenStructure { actual_parts });
    };
    let Some((payload, signature)) = remaining.split_once(COMPACT_SEPARATOR) else {
        return Err(TokenError::InvalidCompactTokenStructure { actual_parts });
    };
    let signing_input_len = header
        .len()
        .checked_add(1)
        .and_then(|len| len.checked_add(payload.len()))
        .ok_or(TokenError::CompactTokenFramingMismatch)?;
    let signing_input = token.get(..signing_input_len).ok_or(TokenError::CompactTokenFramingMismatch)?;
    Ok(CompactParts {
        header,
        payload,
        signature,
        signing_input,
    })
}

fn validate_compact_token_string(token: &str) -> TokenResult<()> {
    if token.is_empty() {
        return Err(TokenError::EmptyCompactToken);
    }
    if token.trim() != token {
        return Err(TokenError::CompactTokenMustBeTrimmed);
    }
    Ok(())
}

fn validate_header(header: &DecodedHeader<'_>) -> TokenResult<()> {
    let algorithm = header.alg.unwrap_or_default();
    let token_type = header.typ.unwrap_or_default();
    if algorithm == TOKEN_ALGORITHM_EDDSA && token_type == TOKEN_TYPE_JWT {
        return Ok(());
    }
    Err(TokenError::UnsupportedHeader)
}

#[derive(Default)]
struct CountingWriter {
    len: usize,
}

impl Write for CountingWriter {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        self.len = self
            .len
            .checked_add(input.len())
            .ok_or_else(|| io::Error::new(io::ErrorKind::WriteZero, "JSON length overflow"))?;
        Ok(input.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn encode_json_exact<T: Serialize>(value: &T) -> TokenResult<SecretBytes> {
    let mut counter = CountingWriter::default();
    serde_json::to_writer(&mut counter, value).map_err(|_| TokenError::JsonEncode {
        message: "JSON sizing failed".to_owned(),
    })?;
    let mut output = SecretBytes::empty_exact_capacity(counter.len);
    serde_json::to_writer(&mut output, value).map_err(|_| TokenError::JsonEncode {
        message: "JSON encoding failed".to_owned(),
    })?;
    output.finish_exact(counter.len)
}

fn decode_json_bytes<'de, T>(bytes: &'de [u8], compact_part: CompactPart) -> TokenResult<T>
where T: Deserialize<'de> {
    serde_json::from_slice(bytes).map_err(|error| TokenError::JsonDecode {
        part: compact_part,
        message: sanitized_json_error(&error),
    })
}

fn sanitized_json_error(error: &serde_json::Error) -> String {
    let class = match error.classify() {
        serde_json::error::Category::Io => "I/O",
        serde_json::error::Category::Syntax => "syntax",
        serde_json::error::Category::Data => "data",
        serde_json::error::Category::Eof => "end-of-input",
    };
    format!("{class} error at line {} column {}", error.line(), error.column())
}

fn validate_decoded_json_object_bytes(bytes: &[u8], compact_part: CompactPart) -> TokenResult<()> {
    let first = bytes.iter().position(|byte| !byte.is_ascii_whitespace());
    let last = bytes.iter().rposition(|byte| !byte.is_ascii_whitespace());
    if first.is_some_and(|index| bytes[index] == b'{') && last.is_some_and(|index| bytes[index] == b'}') {
        return Ok(());
    }
    Err(TokenError::JsonDecode {
        part: compact_part,
        message: "decoded JSON must be an object".to_owned(),
    })
}

fn reject_json_escapes(bytes: &[u8], compact_part: CompactPart) -> TokenResult<()> {
    if bytes.contains(&JSON_ESCAPE_BYTE) {
        return Err(TokenError::JsonEscapesNotSupported { part: compact_part });
    }
    Ok(())
}

fn decoded_base64_len(encoded_len: usize, compact_part: CompactPart) -> TokenResult<usize> {
    let complete_blocks = encoded_len / BASE64_INPUT_BLOCK_BYTES;
    let remainder = encoded_len % BASE64_INPUT_BLOCK_BYTES;
    let complete_bytes = complete_blocks
        .checked_mul(BASE64_OUTPUT_BLOCK_BYTES)
        .ok_or_else(|| base64_shape_error(compact_part))?;
    let remainder_bytes = match remainder {
        0 => 0,
        BASE64_REMAINDER_TWO => BASE64_REMAINDER_TWO_OUTPUT_BYTES,
        BASE64_REMAINDER_THREE => BASE64_REMAINDER_THREE_OUTPUT_BYTES,
        _ => return Err(base64_shape_error(compact_part)),
    };
    complete_bytes.checked_add(remainder_bytes).ok_or_else(|| base64_shape_error(compact_part))
}

fn base64_shape_error(part: CompactPart) -> TokenError {
    TokenError::Base64Decode {
        part,
        message: "invalid base64url-no-pad shape".to_owned(),
    }
}

fn decode_part(part: &str, compact_part: CompactPart) -> TokenResult<SecretBytes> {
    let decoded_len = decoded_base64_len(part.len(), compact_part)?;
    let mut output = SecretBytes::zeroed_exact_len(decoded_len);
    let written = URL_SAFE_NO_PAD
        .decode_slice(part.as_bytes(), output.as_mut_slice())
        .map_err(|_| base64_shape_error(compact_part))?;
    if written != decoded_len || output.allocation_len() != decoded_len {
        return Err(TokenError::SecretCapacity {
            expected: decoded_len,
            actual: written,
        });
    }
    Ok(output)
}

fn encode_base64_secret(input: &[u8]) -> TokenResult<SecretBytes> {
    let encoded_len = base64::encoded_len(input.len(), false).ok_or(TokenError::SecretCapacity {
        expected: usize::MAX,
        actual: input.len(),
    })?;
    let mut output = SecretBytes::zeroed_exact_len(encoded_len);
    let written =
        URL_SAFE_NO_PAD.encode_slice(input, output.as_mut_slice()).map_err(|_| TokenError::SecretCapacity {
            expected: encoded_len,
            actual: 0,
        })?;
    if written != encoded_len || output.allocation_len() != encoded_len {
        return Err(TokenError::SecretCapacity {
            expected: encoded_len,
            actual: written,
        });
    }
    Ok(output)
}

fn join_secret_parts(parts: &[&[u8]]) -> TokenResult<SecretBytes> {
    let separators = parts.len().saturating_sub(1);
    let exact_len = parts.iter().try_fold(separators, |total, part| total.checked_add(part.len())).ok_or(
        TokenError::SecretCapacity {
            expected: usize::MAX,
            actual: 0,
        },
    )?;
    let mut output = SecretBytes::empty_exact_capacity(exact_len);
    for (index, part) in parts.iter().enumerate() {
        if index > 0 {
            output.write_all(&[COMPACT_SEPARATOR_BYTE]).map_err(|_| TokenError::SecretCapacity {
                expected: exact_len,
                actual: output.len(),
            })?;
        }
        output.write_all(part).map_err(|_| TokenError::SecretCapacity {
            expected: exact_len,
            actual: output.len(),
        })?;
    }
    output.finish_exact(exact_len)
}

fn validate_parsed_claims_presence(claims: &DecodedClaims<'_>) -> TokenResult<()> {
    let presence = VerifiedParsedClaimsPresence {
        issuer_present: claims.iss.is_some(),
        audience_present: claims.aud.is_some(),
        attenuations_present: claims.att.is_some(),
        proofs_present: claims.prf.is_some(),
        expiration_present: claims.exp.is_some(),
        not_before_present: claims.nbf.is_some(),
    };
    if parsed_claims_presence_is_valid(presence) {
        return Ok(());
    }

    let failure = if !presence.issuer_present {
        TokenValidationFailure::MissingIssuer
    } else if !presence.audience_present {
        TokenValidationFailure::MissingAudience
    } else if !presence.attenuations_present {
        TokenValidationFailure::MissingAttenuations
    } else if !presence.proofs_present {
        TokenValidationFailure::MissingProofs
    } else if !presence.expiration_present {
        TokenValidationFailure::MissingExpiration
    } else {
        TokenValidationFailure::MissingNotBefore
    };
    Err(TokenError::VerifiedPrimitive {
        source: VerifiedPrimitiveError::TokenValidation { failure },
    })
}

fn did_claim_fact(value: Option<&str>) -> VerifiedParsedDidClaim {
    VerifiedParsedDidClaim {
        present: value.is_some(),
        non_empty: value.is_some_and(|value| !value.is_empty()),
        trimmed: value.is_some_and(|value| value.trim() == value),
    }
}

fn validate_parsed_did_claims(claims: &DecodedClaims<'_>) -> TokenResult<()> {
    let facts = VerifiedParsedDidClaims {
        issuer: did_claim_fact(claims.iss),
        audience: did_claim_fact(claims.aud),
    };
    if parsed_did_claims_are_valid(facts) {
        return Ok(());
    }
    Err(TokenError::VerifiedPrimitive {
        source: VerifiedPrimitiveError::TokenValidation {
            failure: TokenValidationFailure::InvalidDidClaims,
        },
    })
}

fn validate_parsed_temporal_order(claims: &DecodedClaims<'_>) -> TokenResult<()> {
    let order = VerifiedParsedTemporalOrder {
        not_before_seconds: claims.nbf.ok_or(TokenError::VerifiedPrimitive {
            source: VerifiedPrimitiveError::TokenValidation {
                failure: TokenValidationFailure::MissingNotBefore,
            },
        })?,
        expires_at_seconds: claims.exp.ok_or(TokenError::VerifiedPrimitive {
            source: VerifiedPrimitiveError::TokenValidation {
                failure: TokenValidationFailure::MissingExpiration,
            },
        })?,
    };
    if parsed_temporal_order_is_valid(order) {
        return Ok(());
    }
    Err(TokenError::VerifiedPrimitive {
        source: VerifiedPrimitiveError::TokenValidation {
            failure: TokenValidationFailure::InvalidTemporalOrder,
        },
    })
}

fn validate_parsed_capability_list_shape(claims: &DecodedClaims<'_>) -> TokenResult<()> {
    let capabilities = claims.att.as_ref().map_or_else(Vec::new, |capabilities| {
        capabilities
            .iter()
            .map(|capability| VerifiedParsedCapabilityShape {
                resource_present: capability.resource.is_some(),
                ability_present: capability.ability.is_some(),
            })
            .collect()
    });
    let shape = VerifiedParsedCapabilityListShape { capabilities };
    if parsed_capability_list_shape_is_valid(&shape) {
        return Ok(());
    }
    Err(TokenError::VerifiedPrimitive {
        source: VerifiedPrimitiveError::TokenValidation {
            failure: TokenValidationFailure::InvalidCapabilityListShape,
        },
    })
}

fn build_core_claims(claims: &DecodedClaims<'_>) -> TokenResult<ucan_core::token::Claims> {
    let decoded_payloads = claims
        .att
        .as_ref()
        .map(|capabilities| capabilities.iter().map(decoded_caveat_payloads).collect::<TokenResult<Vec<_>>>())
        .transpose()?;
    let caveat_inputs = claims
        .att
        .as_ref()
        .zip(decoded_payloads.as_ref())
        .map(|(capabilities, payloads)| {
            capabilities
                .iter()
                .zip(payloads)
                .map(|(capability, payloads)| decoded_caveat_inputs(capability, payloads))
                .collect::<TokenResult<Vec<_>>>()
        })
        .transpose()?;
    let capability_inputs = claims
        .att
        .as_ref()
        .zip(caveat_inputs.as_ref())
        .map(|(capabilities, caveat_inputs)| {
            capabilities
                .iter()
                .zip(caveat_inputs)
                .enumerate()
                .map(|(index, (capability, caveats))| {
                    let resource = capability.resource.ok_or(TokenError::MissingCapabilityField {
                        index,
                        field: CAPABILITY_RESOURCE_FIELD,
                    })?;
                    let ability = capability.ability.ok_or(TokenError::MissingCapabilityField {
                        index,
                        field: CAPABILITY_ABILITY_FIELD,
                    })?;
                    Ok(ucan_core::token::CapabilityInput::with_caveats(resource, ability, caveats.as_slice()))
                })
                .collect::<TokenResult<Vec<_>>>()
        })
        .transpose()?;
    let proof_bytes = decoded_proof_references(claims)?;
    let proof_inputs = proof_bytes.as_ref().map(|references| proof_reference_inputs(references.as_slice()));
    let input = ucan_core::token::ClaimsInput {
        issuer_did: claims.iss,
        audience_did: claims.aud,
        capabilities: capability_inputs.as_deref(),
        proofs: proof_inputs.as_deref(),
        not_before_seconds: claims.nbf,
        expires_at_seconds: claims.exp,
    };
    ucan_core::token::Claims::from_input(input).map_err(TokenError::from)
}

fn decoded_proof_references(claims: &DecodedClaims<'_>) -> TokenResult<Option<Vec<Vec<u8>>>> {
    let Some(proofs) = claims.prf.as_ref() else {
        return Ok(None);
    };
    let mut decoded = Vec::with_capacity(proofs.len());
    let mut entries = Vec::with_capacity(proofs.len());
    for proof in proofs {
        let reference = decode_part(proof, CompactPart::Payload)?;
        let exact_digest_width = reference.len() == PROOF_REFERENCE_DIGEST_LEN;
        entries.push(VerifiedParsedProofReferenceEntryShape {
            encoded_non_empty: !proof.is_empty(),
            encoded_base64url_no_pad: true,
            decoded_non_empty: !reference.is_empty() && exact_digest_width,
        });
        decoded.push(reference.as_slice().to_vec());
    }
    let shape = VerifiedParsedProofReferenceListShape { entries };
    if !parsed_proof_reference_list_shape_is_valid(&shape)
        || decoded.iter().any(|reference| reference.len() != PROOF_REFERENCE_DIGEST_LEN)
    {
        return Err(TokenError::VerifiedPrimitive {
            source: VerifiedPrimitiveError::TokenValidation {
                failure: TokenValidationFailure::InvalidProofReferenceListShape,
            },
        });
    }
    Ok(Some(decoded))
}

fn validate_parsed_caveat_list_shape(capability: &DecodedCapability<'_>) -> TokenResult<()> {
    let shape = VerifiedParsedCaveatListShape {
        caveats: capability
            .caveats
            .iter()
            .map(|caveat| VerifiedParsedCaveatShape {
                domain_present: caveat.domain.is_some(),
                caveat_type_present: caveat.caveat_type.is_some(),
                key_present: caveat.key.is_some(),
                payload_present: caveat.payload.is_some(),
            })
            .collect(),
    };
    if parsed_caveat_list_shape_is_valid(&shape) {
        return Ok(());
    }
    Err(TokenError::VerifiedPrimitive {
        source: VerifiedPrimitiveError::TokenValidation {
            failure: TokenValidationFailure::InvalidCaveatListShape,
        },
    })
}

fn decoded_caveat_payloads(capability: &DecodedCapability<'_>) -> TokenResult<Vec<SecretBytes>> {
    validate_parsed_caveat_list_shape(capability)?;
    let mut decoded_payloads = Vec::with_capacity(capability.caveats.len());
    let mut payload_facts = Vec::with_capacity(capability.caveats.len());
    for caveat in &capability.caveats {
        let payload = caveat.payload.ok_or_else(|| TokenError::Capability {
            message: "caveat is missing payload".to_owned(),
        })?;
        let decoded_payload = decode_part(payload, CompactPart::Payload)?;
        payload_facts.push(VerifiedParsedCaveatPayloadFact {
            payload_present: true,
            encoded_non_empty: !payload.is_empty(),
            encoded_base64url_no_pad: true,
            decoded_non_empty: !decoded_payload.is_empty(),
        });
        decoded_payloads.push(decoded_payload);
    }
    let payloads = VerifiedParsedCaveatPayloadList {
        payloads: payload_facts,
    };
    if !parsed_caveat_payloads_are_valid(&payloads) {
        return Err(TokenError::VerifiedPrimitive {
            source: VerifiedPrimitiveError::TokenValidation {
                failure: TokenValidationFailure::InvalidCaveatPayloadAdmission,
            },
        });
    }
    Ok(decoded_payloads)
}

fn decoded_caveat_inputs<'a>(
    capability: &'a DecodedCapability<'a>,
    payloads: &'a [SecretBytes],
) -> TokenResult<Vec<ucan_core::token::CaveatInput<'a>>> {
    capability
        .caveats
        .iter()
        .zip(payloads)
        .map(|(caveat, payload)| {
            let domain = caveat.domain.ok_or_else(|| TokenError::Capability {
                message: "caveat is missing domain".to_owned(),
            })?;
            let caveat_type = caveat.caveat_type.ok_or_else(|| TokenError::Capability {
                message: "caveat is missing type".to_owned(),
            })?;
            let key = caveat.key.ok_or_else(|| TokenError::Capability {
                message: "caveat is missing key".to_owned(),
            })?;
            Ok(ucan_core::token::CaveatInput::new(domain, caveat_type, key, payload.as_slice()))
        })
        .collect()
}

fn proof_reference_inputs(references: &[Vec<u8>]) -> Vec<ucan_core::token::ProofReferenceInput<'_>> {
    references
        .iter()
        .map(|reference| ucan_core::token::ProofReferenceInput::new(reference.as_slice()))
        .collect()
}

fn copy_secret_key(bytes: &[u8]) -> TokenResult<[u8; ED25519_SECRET_KEY_BYTES]> {
    bytes.try_into().map_err(|_| TokenError::InvalidSecretKeyLength { actual: bytes.len() })
}

fn copy_public_key(bytes: &[u8]) -> TokenResult<[u8; ED25519_PUBLIC_KEY_BYTES]> {
    bytes.try_into().map_err(|_| TokenError::InvalidPublicKeyLength { actual: bytes.len() })
}

#[cfg(test)]
mod tests {
    use std::alloc::GlobalAlloc;
    use std::alloc::Layout;
    use std::cell::RefCell;
    use std::sync::atomic::AtomicIsize;
    use std::sync::atomic::Ordering;

    static LIVE_ALLOCATED_BYTES: AtomicIsize = AtomicIsize::new(0);

    struct CountingAllocator;

    #[
        allow(
            unsafe_code, // test-only wrapper around the system allocator for leak accounting
            clippy::cast_possible_wrap // usize layout sizes are far below isize::MAX on 64-bit test hosts
        )
    ]
    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            LIVE_ALLOCATED_BYTES.fetch_add(layout.size() as isize, Ordering::Relaxed);
            // SAFETY: delegated to the system allocator with a validated layout.
            unsafe { std::alloc::System.alloc(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            LIVE_ALLOCATED_BYTES.fetch_sub(layout.size() as isize, Ordering::Relaxed);
            // SAFETY: delegated to the system allocator with the matching layout.
            unsafe { std::alloc::System.dealloc(ptr, layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            let new_layout = Layout::from_size_align(new_size, layout.align()).unwrap_or(layout);
            let new_ptr = unsafe { std::alloc::System.alloc(new_layout) };
            if new_ptr.is_null() {
                return new_ptr;
            }
            let copy_len = usize::min(layout.size(), new_size);
            unsafe {
                std::ptr::copy_nonoverlapping(ptr, new_ptr, copy_len);
                std::alloc::System.dealloc(ptr, layout);
            }
            LIVE_ALLOCATED_BYTES.fetch_add((new_size as isize) - (layout.size() as isize), Ordering::Relaxed);
            new_ptr
        }
    }

    #[global_allocator]
    static TEST_ALLOCATOR: CountingAllocator = CountingAllocator;
    use std::collections::HashSet;
    use std::io::Write as _;

    use base64::Engine;
    use serde_json::Value;

    use super::AllowAllReplayAdmission;
    use super::AudienceDid;
    use super::AuthorizationDecision;
    use super::AuthorizationDenialReason;
    use super::AuthorizationRequest;
    use super::BuilderError;
    use super::CapabilitySet;
    use super::CaveatDecision;
    use super::CaveatIdentifier;
    use super::CaveatPolicy;
    use super::CaveatPolicySet;
    use super::CompactToken;
    use super::Ed25519InMemorySigner;
    use super::Ed25519PublicKey;
    use super::Ed25519SigningKey;
    use super::Ed25519VerificationKey;
    use super::EffectiveDelegation;
    use super::InvocationDenialReason;
    use super::InvocationRequest;
    use super::InvocationResult;
    use super::IssueRequest;
    use super::IssuerDid;
    use super::KeyResolutionContext;
    use super::KeyResolver;
    use super::KeyResolverError;
    use super::NoRevocations;
    use super::ProofCollection;
    use super::ProofReference;
    use super::ProofReferences;
    use super::ProofStore;
    use super::ProofStoreError;
    use super::ProofToken;
    use super::ReplayAdmission;
    use super::ReplayAdmissionError;
    use super::RevocationChecker;
    use super::RevocationError;
    use super::RevocationList;
    use super::SignerError;
    use super::SignerResult;
    use super::SignerSignature;
    use super::TimeConversionError;
    use super::TokenError;
    use super::TokenResult;
    use super::TokenSigner;
    use super::TokenTimeBounds;
    use super::TypedCaveatContext;
    use super::VerificationContext;
    use super::VerificationLimitError;
    use super::VerificationLimits;
    use super::VerificationTime;
    use super::VerifiedToken;
    use super::VerifiedTypedCaveatPolicySet;
    use super::issue_token;
    use super::issue_token_with_signer;
    use super::parse_compact_token;
    use super::proof_reference;
    use super::validate_decoded_token;
    use super::verify_compact_token;
    use super::verify_compact_token_with_resolvers_and_revocations;
    use super::verify_invocation_with_replay;
    use crate::shell::CapabilityDocument;
    use crate::shell::CaveatDocument;

    const ISSUER_SECRET_BYTE: u8 = 7;
    const AUDIENCE_SECRET_BYTE: u8 = 9;
    const OTHER_SECRET_BYTE: u8 = 11;
    const CHILD_SECRET_BYTE: u8 = AUDIENCE_SECRET_BYTE;
    const TOKEN_NOT_BEFORE_SECONDS: u64 = 1_000;
    const TOKEN_EXPIRES_AT_SECONDS: u64 = 2_000;
    const CHILD_NOT_BEFORE_SECONDS: u64 = 1_100;
    const CHILD_EXPIRES_AT_SECONDS: u64 = 1_900;
    const VERIFICATION_TIME_SECONDS: u64 = 1_500;
    const EXPIRED_TIME_SECONDS: u64 = 3_000;
    const INVALID_KEY_LEN: usize = 1;
    const COMPACT_PART_COUNT: usize = 3;
    const EXPECTED_CLAIM_COUNT: usize = 6;
    const SHORT_SIGNATURE_LEN: usize = 1;
    const REDACTION_TEST_SECRET_BYTE: u8 = 0xAA;
    const REDACTION_TEST_SECRET_DECIMAL: &str = "170";
    const REDACTION_TEST_SECRET_HEX_LOWER: &str = "aa";
    const REDACTION_TEST_DIRECT_ARRAY_PREFIX: &str = "[170, 170";
    const UNSUPPORTED_TEST_ALGORITHM: &str = "ES256";

    trait SecureTestTokenCopy {
        fn clone(&self) -> CompactToken;
    }

    impl SecureTestTokenCopy for CompactToken {
        fn clone(&self) -> CompactToken {
            Self::new(self.as_str()).expect("test token copy must remain zeroizing")
        }
    }

    #[test]
    fn issues_and_verifies_token_round_trip() {
        let issuer_key = signing_key(ISSUER_SECRET_BYTE);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive");
        let token = issue_token(&issue_request(&issuer_key, AudienceDid::from(audience), ProofCollection::empty()))
            .expect("token should issue");
        let context = verification_context(vec![&issuer_key], ProofCollection::empty(), VERIFICATION_TIME_SECONDS);

        let verified = verify_compact_token(&token, &context).expect("issued token should verify");

        assert_eq!(verified.issuer().as_str(), issuer_key.issuer().expect("issuer did should derive").as_str());
        assert_eq!(verified.capabilities().as_slice().len(), 1);
        assert!(verified.proofs().as_slice().is_empty());
        assert_eq!(verified.effective_delegation().time_bounds().not_before_seconds(), TOKEN_NOT_BEFORE_SECONDS);
    }

    #[test]
    fn validate_decoded_token_routes_required_claim_presence_through_verified_bridge() {
        assert_payload_failure(
            r#"{"aud":"did:key:zAudience","att":[],"prf":[],"exp":2,"nbf":1}"#,
            crate::verified::TokenValidationFailure::MissingIssuer,
        );
        assert_payload_failure(
            r#"{"iss":"did:key:zIssuer","att":[],"prf":[],"exp":2,"nbf":1}"#,
            crate::verified::TokenValidationFailure::MissingAudience,
        );
        assert_payload_failure(
            r#"{"iss":"did:key:zIssuer","aud":"did:key:zAudience","prf":[],"exp":2,"nbf":1}"#,
            crate::verified::TokenValidationFailure::MissingAttenuations,
        );
        assert_payload_failure(
            r#"{"iss":"did:key:zIssuer","aud":"did:key:zAudience","att":[],"exp":2,"nbf":1}"#,
            crate::verified::TokenValidationFailure::MissingProofs,
        );
        assert_payload_failure(
            r#"{"iss":"did:key:zIssuer","aud":"did:key:zAudience","att":[],"prf":[],"nbf":1}"#,
            crate::verified::TokenValidationFailure::MissingExpiration,
        );
        assert_payload_failure(
            r#"{"iss":"did:key:zIssuer","aud":"did:key:zAudience","att":[],"prf":[],"exp":2}"#,
            crate::verified::TokenValidationFailure::MissingNotBefore,
        );
    }

    #[test]
    fn validate_decoded_token_routes_parsed_did_claims_through_verified_bridge() {
        for payload in [
            r#"{"iss":"","aud":"did:key:zAudience","att":[],"prf":[],"exp":2,"nbf":1}"#,
            r#"{"iss":" did:key:zIssuer","aud":"did:key:zAudience","att":[],"prf":[],"exp":2,"nbf":1}"#,
            r#"{"iss":"did:key:zIssuer","aud":"","att":[],"prf":[],"exp":2,"nbf":1}"#,
            r#"{"iss":"did:key:zIssuer","aud":"did:key:zAudience ","att":[],"prf":[],"exp":2,"nbf":1}"#,
        ] {
            assert_payload_failure(payload, crate::verified::TokenValidationFailure::InvalidDidClaims);
        }
    }

    #[test]
    fn validate_decoded_token_routes_parsed_temporal_order_through_verified_bridge() {
        assert_payload_failure(
            r#"{"iss":"did:key:zIssuer","aud":"did:key:zAudience","att":[],"prf":[],"exp":2,"nbf":2}"#,
            crate::verified::TokenValidationFailure::InvalidTemporalOrder,
        );
        assert_payload_failure(
            r#"{"iss":"did:key:zIssuer","aud":"did:key:zAudience","att":[],"prf":[],"exp":2,"nbf":3}"#,
            crate::verified::TokenValidationFailure::InvalidTemporalOrder,
        );
    }

    #[test]
    fn public_identity_wrappers_keep_serde_while_compact_tokens_use_borrowed_access() {
        let issuer = signing_key(ISSUER_SECRET_BYTE).issuer().expect("issuer should derive");
        let audience = AudienceDid::from(issuer.clone());
        let token = CompactToken::new("header.payload.signature").expect("compact wrapper accepts trimmed strings");

        assert_eq!(issuer.to_string().parse::<IssuerDid>().expect("issuer FromStr"), issuer);
        assert_eq!(IssuerDid::try_from(issuer.as_str()).expect("issuer TryFrom<&str>"), issuer);
        assert_eq!(audience.to_string().parse::<AudienceDid>().expect("audience FromStr"), audience);
        assert_eq!(AudienceDid::try_from(audience.as_str()).expect("audience TryFrom<&str>"), audience);
        assert_eq!(token.as_str(), "header.payload.signature");
        assert_eq!(CompactToken::try_from(token.as_str()).expect("secure compact copy"), token);

        let serialized_issuer = serde_json::to_string(&issuer).expect("issuer serializes");
        let serialized_audience = serde_json::to_string(&audience).expect("audience serializes");
        assert_eq!(serde_json::from_str::<IssuerDid>(&serialized_issuer).expect("issuer deserializes"), issuer);
        assert_eq!(serde_json::from_str::<AudienceDid>(&serialized_audience).expect("audience deserializes"), audience);
    }

    #[test]
    fn compact_owner_rejects_malformed_constructor_inputs() {
        assert!(matches!(CompactToken::new(""), Err(TokenError::EmptyCompactToken)));
        assert!(matches!(CompactToken::new(" token "), Err(TokenError::CompactTokenMustBeTrimmed)));
        assert!(matches!(IssuerDid::try_from(""), Err(TokenError::CoreClaims { .. })));
        assert!(matches!(AudienceDid::try_from(" did:key:z6Mkaudience "), Err(TokenError::CoreClaims { .. })));
    }

    #[test]
    fn compact_owner_and_base64_decode_keep_exact_non_growing_capacity() {
        const ENCODED_FIXTURE: &str = "aGVsbG8";
        const DECODED_FIXTURE_BYTES: usize = 5;
        let token = CompactToken::new("header.payload.signature").expect("compact token should allocate");
        assert_eq!(token.allocation_len(), token.as_str().len());

        let decoded =
            super::decode_part(ENCODED_FIXTURE, super::CompactPart::Payload).expect("base64 fixture should decode");
        assert_eq!(decoded.len(), DECODED_FIXTURE_BYTES);
        assert_eq!(decoded.allocation_len(), DECODED_FIXTURE_BYTES);
    }

    #[test]
    fn fixed_secret_writer_rejects_growth_without_reallocation() {
        const FIXED_BYTES: usize = 4;
        const OVERSIZED_BYTES: usize = FIXED_BYTES + 1;
        let mut output = super::SecretBytes::empty_exact_capacity(FIXED_BYTES);
        let oversized = [0_u8; OVERSIZED_BYTES];

        let error = output.write_all(&oversized).expect_err("fixed secret allocation must reject growth");

        assert_eq!(error.kind(), std::io::ErrorKind::WriteZero);
        assert_eq!(output.len(), 0);
        assert_eq!(output.allocation_len(), FIXED_BYTES);
    }

    #[test]
    fn borrowed_verification_succeeds_without_a_compact_owner_copy() {
        let issuer_key = signing_key(ISSUER_SECRET_BYTE);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive");
        let token = issue_token(&issue_request(&issuer_key, AudienceDid::from(audience), ProofCollection::empty()))
            .expect("token should issue");
        let context = verification_context(vec![&issuer_key], ProofCollection::empty(), VERIFICATION_TIME_SECONDS);

        let verified =
            super::verify_compact_token_str(token.as_str(), &context).expect("borrowed compact text should verify");

        assert_eq!(verified.issuer().as_str(), issuer_key.issuer().expect("issuer should derive").as_str());
    }

    #[test]
    fn oversized_borrowed_input_fails_before_parse_or_exact_identity() {
        const INPUT_BYTE_LIMIT: usize = 8;
        const OVERSIZED_INPUT_BYTES: usize = INPUT_BYTE_LIMIT + 1;
        let input = "x".repeat(OVERSIZED_INPUT_BYTES);
        let context = VerificationContext::with_limits(
            VerificationTime::from_unix_seconds(VERIFICATION_TIME_SECONDS),
            KeyResolutionContext::new(Vec::new()),
            ProofCollection::empty(),
            VerificationLimits::new(0, 0, INPUT_BYTE_LIMIT),
        );

        let error = super::verify_compact_token_str(input.as_str(), &context)
            .expect_err("oversized input must fail before malformed syntax");

        assert!(matches!(error, TokenError::LimitExceeded {
            source: VerificationLimitError::CompactTokenTooLarge {
                byte_len: OVERSIZED_INPUT_BYTES,
                max_byte_len: INPUT_BYTE_LIMIT,
            }
        }));
    }

    #[test]
    fn early_decode_error_drops_zeroizing_output_and_errors_do_not_echo_bearer_text() {
        super::SECRET_BYTES_DROP_COUNT.store(0, std::sync::atomic::Ordering::SeqCst);
        let token = CompactToken::new("abc=.payload.signature").expect("fixture should enter parser");
        let error = parse_compact_token(&token).expect_err("invalid base64 must fail");
        assert!(super::SECRET_BYTES_DROP_COUNT.load(std::sync::atomic::Ordering::SeqCst) > 0);
        let display = error.to_string();
        let debug = format!("{error:?}");
        assert!(!display.contains(token.as_str()));
        assert!(!debug.contains(token.as_str()));
    }

    #[test]
    fn serde_escape_path_is_rejected_before_borrowed_json_decode() {
        let token = token_with_header(
            r#"{"alg":"Ed\\u0044SA","typ":"JWT"}"#,
            r#"{"iss":"did:key:zIssuer","aud":"did:key:zAudience","att":[],"prf":[],"exp":2,"nbf":1}"#,
        );
        let error = parse_compact_token(&token).expect_err("escaped JSON string must fail before serde");
        assert!(matches!(error, TokenError::JsonEscapesNotSupported {
            part: super::CompactPart::Header
        }));
    }

    #[test]
    fn compact_references_bind_complete_bounded_inputs() {
        let first = CompactToken::new("header.payload.signature-a").expect("first token should wrap");
        let second = CompactToken::new("header.payload.signature-b").expect("second token should wrap");
        assert_ne!(proof_reference(&first), proof_reference(&second));
        assert_eq!(proof_reference(&first).as_bytes().len(), blake3::OUT_LEN);
    }

    #[test]
    fn compact_secret_surface_has_only_redacted_debug_and_borrowed_access() {
        let source = include_str!("token.rs");
        let production_source = source.split("#[cfg(test)]\nmod tests").next().expect("production source prefix");
        for forbidden in [
            "impl Clone for CompactToken",
            "impl fmt::Display for CompactToken",
            "impl Serialize for CompactToken",
            "Deserialize<'de> for CompactToken",
            "impl TryFrom<String> for CompactToken",
            "into_string(self)",
            "Result<CompactToken, ProofStoreError>",
            "serde_json::to_vec",
            "URL_SAFE_NO_PAD.decode(",
            "bytes: Zeroizing<Vec<u8>>",
            "pub fn new(bytes: Vec<u8>)",
            "SignerSignature::new",
        ] {
            assert!(!production_source.contains(forbidden), "forbidden bearer surface appeared: {forbidden}");
        }
        let verified_source = include_str!("verified.rs");
        let verified_production =
            verified_source.split("#[cfg(test)]\nmod tests").next().expect("verified production source prefix");
        for forbidden in [
            "pub struct VerifiedCompactToken",
            "pub struct VerifiedSigningInputByteProvenance",
            "message.to_vec()",
            "signature_bytes.to_vec()",
        ] {
            assert!(!verified_production.contains(forbidden), "forbidden verified bearer copy appeared: {forbidden}");
        }
        let token = CompactToken::new("header.payload.signature").expect("token should wrap");
        let debug = format!("{token:?}");
        assert!(debug.contains(super::REDACTED_SECRET_MATERIAL_DEBUG));
        assert!(!debug.contains(token.as_str()));
    }

    #[test]
    fn builders_match_positional_constructors_and_report_missing_fields() {
        let issuer_key = signing_key(ISSUER_SECRET_BYTE);
        let audience =
            AudienceDid::from(signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive"));
        let capabilities = capabilities("did:key:z6Mkresource", "store/*");
        let bounds = token_time_bounds();
        let positional_issue = IssueRequest::new(
            issuer_key.clone(),
            audience.clone(),
            capabilities.clone(),
            ProofReferences::empty(),
            bounds,
        );
        let built_issue = IssueRequest::builder()
            .signing_key(issuer_key.clone())
            .audience(audience)
            .capabilities(capabilities)
            .proofs(ProofReferences::empty())
            .time_bounds(bounds)
            .build()
            .expect("issue builder should complete");
        assert_eq!(built_issue, positional_issue);
        assert_eq!(IssueRequest::builder().build(), Err(BuilderError::MissingField { field: "signing_key" }));

        let keys = KeyResolutionContext::new(vec![
            Ed25519VerificationKey::from_signing_key(&issuer_key).expect("key should derive"),
        ]);
        let time = VerificationTime::from_unix_seconds(VERIFICATION_TIME_SECONDS);
        let limits = VerificationLimits::new(2, 3, 4_096);
        let positional_context = VerificationContext::with_limits(time, keys.clone(), ProofCollection::empty(), limits);
        let built_context = VerificationContext::builder()
            .time(time)
            .keys(keys)
            .proofs(ProofCollection::empty())
            .limits(limits)
            .build()
            .expect("verification builder should complete");
        assert_eq!(built_context, positional_context);
        assert_eq!(VerificationContext::builder().build(), Err(BuilderError::MissingField { field: "time" }));
    }

    #[test]
    fn system_time_and_duration_helpers_are_deterministic_and_structured() {
        let start = std::time::UNIX_EPOCH + std::time::Duration::from_secs(TOKEN_NOT_BEFORE_SECONDS);
        let verification_time = VerificationTime::try_from_system_time(start).expect("system time should convert");
        let bounds = TokenTimeBounds::from_system_time_and_duration(start, std::time::Duration::from_secs(1_000))
            .expect("duration bounds should convert");
        let direct_bounds = TokenTimeBounds::from_unix_seconds_and_duration(
            TOKEN_NOT_BEFORE_SECONDS,
            std::time::Duration::from_secs(1_000),
        )
        .expect("unix duration bounds should convert");

        assert_eq!(verification_time.as_unix_seconds(), TOKEN_NOT_BEFORE_SECONDS);
        assert_eq!(bounds.not_before_seconds(), TOKEN_NOT_BEFORE_SECONDS);
        assert_eq!(bounds.expires_at_seconds(), TOKEN_NOT_BEFORE_SECONDS + 1_000);
        assert_eq!(direct_bounds, bounds);

        let before_epoch = std::time::UNIX_EPOCH - std::time::Duration::from_secs(1);
        assert_eq!(VerificationTime::try_from_system_time(before_epoch), Err(TimeConversionError::BeforeUnixEpoch));
        assert_eq!(
            TokenTimeBounds::from_unix_seconds_and_duration(u64::MAX, std::time::Duration::from_secs(1)),
            Err(TimeConversionError::DurationOverflow)
        );
        assert!(matches!(
            TokenTimeBounds::from_unix_seconds_and_duration(TOKEN_NOT_BEFORE_SECONDS, std::time::Duration::ZERO),
            Err(TimeConversionError::InvalidBounds { .. })
        ));
    }

    #[test]
    fn ergonomics_work_does_not_add_cleartext_secret_exports() {
        let source = include_str!("token.rs");
        let forbidden = [
            format!("{}{}", "impl fmt::Display for ", "Ed25519SigningKey"),
            format!("{}{}", "impl Serialize for ", "Ed25519SigningKey"),
            format!("{}{}", "impl<'de> Deserialize<'de> for ", "Ed25519SigningKey"),
            format!("{}{}", "impl fmt::Display for ", "Ed25519InMemorySigner"),
            format!("{}{}", "impl Serialize for ", "Ed25519InMemorySigner"),
            format!("{}{}", "impl<'de> Deserialize<'de> for ", "Ed25519InMemorySigner"),
        ];
        for forbidden_impl in forbidden {
            assert!(
                !source.contains(forbidden_impl.as_str()),
                "forbidden cleartext trait implementation appeared: {forbidden_impl}"
            );
        }
        let key = signing_key(REDACTION_TEST_SECRET_BYTE);
        assert_secret_material_is_absent(format!("{key:?}").as_str());
        assert_secret_material_is_absent(format!("{:?}", Ed25519InMemorySigner::from_signing_key(key)).as_str());
    }

    #[test]
    fn verified_token_authorizes_exact_concrete_request() {
        let issuer_key = signing_key(ISSUER_SECRET_BYTE);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive");
        let token = issue_token(&issue_request_with_capability(
            &issuer_key,
            AudienceDid::from(audience),
            ProofCollection::empty(),
            "did:key:z6Mkresource/photos",
            "store/add",
        ))
        .expect("token should issue");
        let context = verification_context(vec![&issuer_key], ProofCollection::empty(), VERIFICATION_TIME_SECONDS);
        let verified = verify_compact_token(&token, &context).expect("issued token should verify");
        let request = AuthorizationRequest::new("did:key:z6Mkresource/photos", "store/add")
            .expect("concrete request should validate");

        let decision = verified.authorize_request(&request);

        assert_allowed_capability(&decision, "did:key:z6Mkresource/photos", "store/add");
    }

    #[test]
    fn verified_token_authorizes_delegated_wildcard_and_narrower_resource() {
        let issuer_key = signing_key(ISSUER_SECRET_BYTE);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive");
        let token = issue_token(&issue_request(&issuer_key, AudienceDid::from(audience), ProofCollection::empty()))
            .expect("token should issue");
        let context = verification_context(vec![&issuer_key], ProofCollection::empty(), VERIFICATION_TIME_SECONDS);
        let verified = verify_compact_token(&token, &context).expect("issued token should verify");
        let request = AuthorizationRequest::new("did:key:z6Mkresource/photos", "store/add")
            .expect("concrete request should validate");

        let decision = verified.authorize_request(&request);

        assert_allowed_capability(&decision, "did:key:z6Mkresource", "store/*");
    }

    #[test]
    fn verified_token_authorizes_satisfied_caveat_policy() {
        let issuer_key = signing_key(ISSUER_SECRET_BYTE);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive");
        let token = issue_token(&issue_request_with_caveat(
            &issuer_key,
            AudienceDid::from(audience),
            ProofCollection::empty(),
            caveat("time", "before", "unix", b"2000"),
        ))
        .expect("token should issue");
        let context = verification_context(vec![&issuer_key], ProofCollection::empty(), VERIFICATION_TIME_SECONDS);
        let verified = verify_compact_token(&token, &context).expect("issued token should verify");
        let policies = SinglePolicySet::new("time", "before", "unix", AllowPolicy);

        let decision = verified.authorize_with_policies("did:key:z6Mkresource/photos", "store/add", &policies);

        assert!(decision.is_allowed());
        assert_eq!(decision.matched_capability().expect("matched").resource, "did:key:z6Mkresource");
        assert_eq!(decision.matched_capability().expect("matched").ability, "store/*");
        assert_eq!(decision.matched_capability().expect("matched").caveats.len(), 1);
    }

    #[test]
    fn verified_token_denies_unknown_or_rejected_caveats() {
        let issuer_key = signing_key(ISSUER_SECRET_BYTE);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive");
        let token = issue_token(&issue_request_with_caveat(
            &issuer_key,
            AudienceDid::from(audience),
            ProofCollection::empty(),
            caveat("time", "before", "unix", b"2000"),
        ))
        .expect("token should issue");
        let context = verification_context(vec![&issuer_key], ProofCollection::empty(), VERIFICATION_TIME_SECONDS);
        let verified = verify_compact_token(&token, &context).expect("issued token should verify");

        let unknown = verified.authorize("did:key:z6Mkresource/photos", "store/add");
        let reject = verified.authorize_with_policies(
            "did:key:z6Mkresource/photos",
            "store/add",
            &SinglePolicySet::new("time", "before", "unix", RejectPolicy),
        );

        assert!(matches!(unknown.denial_reason(), Some(AuthorizationDenialReason::CaveatPolicyMissing { .. })));
        assert!(matches!(reject.denial_reason(), Some(AuthorizationDenialReason::CaveatRejected { .. })));
    }

    #[test]
    fn verified_typed_caveat_policy_allows_time_and_rate_limit_caveats() {
        let issuer_key = signing_key(ISSUER_SECRET_BYTE);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive");
        let capability =
            CapabilityDocument::with_caveats("did:key:z6Mkresource".to_owned(), "store/*".to_owned(), vec![
                caveat("time", "before", "unix", b"2000"),
                caveat("rate", "limit", "window", b"5:60"),
            ])
            .expect("capability should validate");
        let token = issue_token(&IssueRequest::new(
            issuer_key.clone(),
            AudienceDid::from(audience),
            CapabilitySet::new(vec![capability]).expect("capability set should validate"),
            ProofReferences::empty(),
            token_time_bounds(),
        ))
        .expect("token should issue");
        let context = verification_context(vec![&issuer_key], ProofCollection::empty(), VERIFICATION_TIME_SECONDS);
        let verified = verify_compact_token(&token, &context).expect("issued token should verify");
        let policies = VerifiedTypedCaveatPolicySet::new(TypedCaveatContext::new(1_500, 5));

        let decision = verified.authorize_with_policies("did:key:z6Mkresource/photos", "store/add", &policies);

        assert!(decision.is_allowed());
        assert_eq!(policies.context().now_seconds(), 1_500);
        assert_eq!(policies.context().count_in_window(), 5);
    }

    #[test]
    fn verified_typed_caveat_policy_denies_unsatisfied_caveats() {
        let issuer_key = signing_key(ISSUER_SECRET_BYTE);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive");
        let token = issue_token(&issue_request_with_caveat(
            &issuer_key,
            AudienceDid::from(audience),
            ProofCollection::empty(),
            caveat("time", "before", "unix", b"2000"),
        ))
        .expect("token should issue");
        let context = verification_context(vec![&issuer_key], ProofCollection::empty(), VERIFICATION_TIME_SECONDS);
        let verified = verify_compact_token(&token, &context).expect("issued token should verify");
        let policies = VerifiedTypedCaveatPolicySet::new(TypedCaveatContext::new(2_001, 0));

        let decision = verified.authorize_with_policies("did:key:z6Mkresource/photos", "store/add", &policies);

        assert!(matches!(
            decision.denial_reason(),
            Some(AuthorizationDenialReason::CaveatRejected { message, .. })
                if message == "typed caveat rejected by verified caveat semantics"
        ));
    }

    #[test]
    fn verified_typed_caveat_policy_denies_malformed_payloads() {
        let issuer_key = signing_key(ISSUER_SECRET_BYTE);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive");
        let token = issue_token(&issue_request_with_caveat(
            &issuer_key,
            AudienceDid::from(audience),
            ProofCollection::empty(),
            caveat("rate", "limit", "window", b"bad"),
        ))
        .expect("token should issue");
        let context = verification_context(vec![&issuer_key], ProofCollection::empty(), VERIFICATION_TIME_SECONDS);
        let verified = verify_compact_token(&token, &context).expect("issued token should verify");
        let policies = VerifiedTypedCaveatPolicySet::new(TypedCaveatContext::new(1_500, 0));

        let decision = verified.authorize_with_policies("did:key:z6Mkresource/photos", "store/add", &policies);

        assert!(matches!(
            decision.denial_reason(),
            Some(AuthorizationDenialReason::CaveatRejected { message, .. })
                if message == "rate-limit caveat payload must be verified max_count:window_seconds ASCII decimal"
        ));
    }

    #[test]
    fn verified_typed_caveat_policy_does_not_satisfy_unknown_caveats() {
        let issuer_key = signing_key(ISSUER_SECRET_BYTE);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive");
        let token = issue_token(&issue_request_with_caveat(
            &issuer_key,
            AudienceDid::from(audience),
            ProofCollection::empty(),
            caveat("custom", "review", "ticket", b"123"),
        ))
        .expect("token should issue");
        let context = verification_context(vec![&issuer_key], ProofCollection::empty(), VERIFICATION_TIME_SECONDS);
        let verified = verify_compact_token(&token, &context).expect("issued token should verify");
        let policies = VerifiedTypedCaveatPolicySet::new(TypedCaveatContext::new(1_500, 0));

        let decision = verified.authorize_with_policies("did:key:z6Mkresource/photos", "store/add", &policies);

        assert!(matches!(
            decision.denial_reason(),
            Some(AuthorizationDenialReason::CaveatPolicyMissing { caveat })
                if caveat.domain() == "custom" && caveat.caveat_type() == "review" && caveat.key() == "ticket"
        ));
    }

    #[test]
    fn invocation_allows_satisfied_caveat_and_exposes_effective_context() {
        let issuer_key = signing_key(ISSUER_SECRET_BYTE);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive");
        let token = issue_token(&issue_request_with_caveat(
            &issuer_key,
            AudienceDid::from(audience.clone()),
            ProofCollection::empty(),
            caveat("time", "before", "unix", b"2000"),
        ))
        .expect("token should issue");
        let context = verification_context(vec![&issuer_key], ProofCollection::empty(), VERIFICATION_TIME_SECONDS);
        let policies = SinglePolicySet::new("time", "before", "unix", AllowPolicy);
        let request = InvocationRequest::new(&token, &context, "did:key:z6Mkresource/photos", "store/add", &policies);

        let result = super::verify_invocation(&request);

        let InvocationResult::Allowed { invocation } = result else {
            panic!("invocation should be allowed");
        };
        assert_eq!(invocation.issuer().as_str(), issuer_key.issuer().expect("issuer").as_str());
        assert_eq!(invocation.audience().as_str(), audience.as_str());
        assert_eq!(invocation.matched_capability().resource, "did:key:z6Mkresource");
        assert_eq!(invocation.effective_time_bounds(), token_time_bounds());
    }

    #[test]
    fn invocation_with_concrete_matched_capability_uses_verified_invocation_admission() {
        let issuer_key = signing_key(ISSUER_SECRET_BYTE);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive");
        let token = issue_token(&issue_request_with_capability(
            &issuer_key,
            AudienceDid::from(audience.clone()),
            ProofCollection::empty(),
            "did:key:z6Mkresource",
            "store/add",
        ))
        .expect("token should issue");
        let context = verification_context(vec![&issuer_key], ProofCollection::empty(), VERIFICATION_TIME_SECONDS);
        let request = InvocationRequest::new(
            &token,
            &context,
            "did:key:z6Mkresource/photos",
            "store/add",
            &super::DenyUnknownCaveats,
        );

        let result = super::verify_invocation(&request);

        let InvocationResult::Allowed { invocation } = result else {
            panic!("concrete invocation should be allowed");
        };
        assert_eq!(invocation.audience().as_str(), audience.as_str());
        assert_eq!(invocation.matched_capability().ability, "store/add");
    }

    #[test]
    fn invocation_denies_caveat_rejection_and_verification_failure() {
        let issuer_key = signing_key(ISSUER_SECRET_BYTE);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive");
        let token = issue_token(&issue_request_with_caveat(
            &issuer_key,
            AudienceDid::from(audience),
            ProofCollection::empty(),
            caveat("time", "before", "unix", b"2000"),
        ))
        .expect("token should issue");
        let context = verification_context(vec![&issuer_key], ProofCollection::empty(), VERIFICATION_TIME_SECONDS);
        let reject_policies = SinglePolicySet::new("time", "before", "unix", RejectPolicy);
        let rejected = super::verify_invocation(&InvocationRequest::new(
            &token,
            &context,
            "did:key:z6Mkresource/photos",
            "store/add",
            &reject_policies,
        ));
        let tampered = CompactToken::new("not.enough").expect("non-empty wrapper");
        let verified_bad = super::verify_invocation(&InvocationRequest::new(
            &tampered,
            &context,
            "did:key:z6Mkresource/photos",
            "store/add",
            &reject_policies,
        ));

        assert!(matches!(rejected, InvocationResult::Denied {
            reason: InvocationDenialReason::Authorization {
                reason: AuthorizationDenialReason::CaveatRejected { .. }
            }
        }));
        assert!(matches!(verified_bad, InvocationResult::Denied {
            reason: InvocationDenialReason::Verification { .. }
        }));
    }

    #[derive(Debug, Default)]
    struct InMemoryReplayAdmission {
        admitted: RefCell<HashSet<(ProofReference, String, String)>>,
    }

    impl ReplayAdmission for InMemoryReplayAdmission {
        fn admit_invocation(
            &self,
            token_reference: &ProofReference,
            resource: &str,
            ability: &str,
        ) -> std::result::Result<(), ReplayAdmissionError> {
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

    #[test]
    fn invocation_replay_admission_accepts_first_and_denies_duplicate() {
        let issuer_key = signing_key(ISSUER_SECRET_BYTE);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive");
        let token = issue_token(&issue_request_with_capability(
            &issuer_key,
            AudienceDid::from(audience),
            ProofCollection::empty(),
            "did:key:z6Mkresource",
            "store/add",
        ))
        .expect("token should issue");
        let context = verification_context(vec![&issuer_key], ProofCollection::empty(), VERIFICATION_TIME_SECONDS);
        let request = InvocationRequest::new(
            &token,
            &context,
            "did:key:z6Mkresource/photos",
            "store/add",
            &super::DenyUnknownCaveats,
        );
        let replay = InMemoryReplayAdmission::default();

        let first = verify_invocation_with_replay(&request, &replay);
        let second = verify_invocation_with_replay(&request, &replay);

        assert!(first.is_allowed());
        assert!(matches!(second, InvocationResult::Denied {
            reason: InvocationDenialReason::Replay { .. }
        }));
    }

    #[test]
    fn allow_all_replay_admission_preserves_allowed_invocation_result() {
        let issuer_key = signing_key(ISSUER_SECRET_BYTE);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive");
        let token = issue_token(&issue_request_with_capability(
            &issuer_key,
            AudienceDid::from(audience),
            ProofCollection::empty(),
            "did:key:z6Mkresource",
            "store/add",
        ))
        .expect("token should issue");
        let context = verification_context(vec![&issuer_key], ProofCollection::empty(), VERIFICATION_TIME_SECONDS);
        let request = InvocationRequest::new(
            &token,
            &context,
            "did:key:z6Mkresource/photos",
            "store/add",
            &super::DenyUnknownCaveats,
        );

        assert!(verify_invocation_with_replay(&request, &AllowAllReplayAdmission).is_allowed());
        assert!(verify_invocation_with_replay(&request, &AllowAllReplayAdmission).is_allowed());
    }

    #[test]
    fn verified_token_denies_unmatched_concrete_request() {
        let issuer_key = signing_key(ISSUER_SECRET_BYTE);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive");
        let token = issue_token(&issue_request(&issuer_key, AudienceDid::from(audience), ProofCollection::empty()))
            .expect("token should issue");
        let context = verification_context(vec![&issuer_key], ProofCollection::empty(), VERIFICATION_TIME_SECONDS);
        let verified = verify_compact_token(&token, &context).expect("issued token should verify");

        let decision = verified.authorize("did:key:z6Mkresource/photos", "admin/delete");

        assert_eq!(decision.denial_reason(), Some(&AuthorizationDenialReason::NoMatchingCapability));
    }

    #[test]
    fn authorization_request_rejects_malformed_ability_wildcards() {
        let embedded_error = AuthorizationRequest::new("did:key:z6Mkresource/photos", "store/a*d")
            .expect_err("embedded wildcard segment should fail core ability validation");
        let full_segment_error = AuthorizationRequest::new("did:key:z6Mkresource/photos", "store/*")
            .expect_err("operation request must be concrete");

        assert!(matches!(embedded_error, TokenError::Capability { .. }));
        assert!(matches!(full_segment_error, TokenError::Capability { .. }));

        let issuer_key = signing_key(ISSUER_SECRET_BYTE);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive");
        let token = issue_token(&issue_request(&issuer_key, AudienceDid::from(audience), ProofCollection::empty()))
            .expect("token should issue");
        let context = verification_context(vec![&issuer_key], ProofCollection::empty(), VERIFICATION_TIME_SECONDS);
        let verified = verify_compact_token(&token, &context).expect("issued token should verify");

        let embedded_decision = verified.authorize("did:key:z6Mkresource/photos", "store/a*d");
        let full_segment_decision = verified.authorize("did:key:z6Mkresource/photos", "store/*");

        assert_malformed_denial(&embedded_decision);
        assert_malformed_denial(&full_segment_decision);
    }

    #[test]
    fn verified_token_denies_broader_resource_than_child_delegation() {
        let verified = verified_child_token();

        let decision = verified.authorize("did:key:z6Mkresource", "store/add");

        assert_eq!(decision.denial_reason(), Some(&AuthorizationDenialReason::NoMatchingCapability));
    }

    #[test]
    fn authorization_uses_effective_child_delegation_not_broader_proof_capability() {
        let verified = verified_child_token();
        assert_allowed_capability(
            &verified.authorize("did:key:z6Mkresource/photos", "store/add"),
            "did:key:z6Mkresource/photos",
            "store/add",
        );

        let decision = verified.authorize("did:key:z6Mkresource", "store/add");

        assert_eq!(decision.denial_reason(), Some(&AuthorizationDenialReason::NoMatchingCapability));
    }

    #[test]
    fn authorization_uses_effective_delegation_not_direct_capability_claims() {
        let verified = VerifiedToken {
            issuer: IssuerDid::new("did:key:z6Mkissuer").expect("issuer should validate"),
            audience: AudienceDid::new("did:key:z6Mkaudience").expect("audience should validate"),
            capabilities: capabilities("did:key:z6Mkresource", "store/*"),
            proofs: super::ProofReferences::default(),
            effective: EffectiveDelegation {
                capabilities: capabilities("did:key:z6Mkresource/photos", "store/add"),
                time_bounds: token_time_bounds(),
            },
            proof_transcript: None,
        };
        assert_eq!(verified.capabilities().as_slice(), capabilities("did:key:z6Mkresource", "store/*").as_slice());

        let decision = verified.authorize("did:key:z6Mkresource", "store/add");

        assert_eq!(decision.denial_reason(), Some(&AuthorizationDenialReason::NoMatchingCapability));
        assert_allowed_capability(
            &verified.authorize("did:key:z6Mkresource/photos", "store/add"),
            "did:key:z6Mkresource/photos",
            "store/add",
        );
    }

    #[test]
    fn issued_token_uses_required_compact_shape_and_claim_names() {
        let issuer_key = signing_key(ISSUER_SECRET_BYTE);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive");
        let token = issue_token(&issue_request(&issuer_key, AudienceDid::from(audience), ProofCollection::empty()))
            .expect("token should issue");
        assert_token_shape_and_claim_names(&token);
    }

    #[test]
    fn signer_based_issue_round_trips_with_in_memory_ed25519() {
        let signer = in_memory_signer(ISSUER_SECRET_BYTE);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive");
        let capabilities = capabilities("did:key:z6Mkresource", "store/*");
        let proofs = ProofReferences::empty();
        let bounds = token_time_bounds();
        let token = issue_token_with_signer(&signer, &AudienceDid::from(audience), &capabilities, &proofs, bounds)
            .expect("signer should issue");
        let context = VerificationContext::new(
            VerificationTime::from_unix_seconds(VERIFICATION_TIME_SECONDS),
            KeyResolutionContext::new(vec![signer.verification_key().expect("verification key should derive")]),
            ProofCollection::empty(),
        );

        let verified = verify_compact_token(&token, &context).expect("signer-issued token should verify");

        assert_eq!(verified.issuer().as_str(), signer.issuer().expect("issuer should derive").as_str());
        assert_eq!(verified.capabilities().as_slice(), capabilities.as_slice());
        assert_eq!(verified.effective_delegation().time_bounds(), bounds);
    }

    #[test]
    fn signer_based_issue_preserves_compact_shape() {
        let signer = in_memory_signer(ISSUER_SECRET_BYTE);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive");
        let token = issue_token_with_signer(
            &signer,
            &AudienceDid::from(audience),
            &capabilities("did:key:z6Mkresource", "store/*"),
            &ProofReferences::empty(),
            token_time_bounds(),
        )
        .expect("signer should issue");

        assert_token_shape_and_claim_names(&token);
    }

    #[test]
    fn refusing_signer_returns_structured_error() {
        let key = signing_key(ISSUER_SECRET_BYTE);
        let signer = RefusingSigner::new(&key);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive");
        let error = issue_token_with_signer(
            &signer,
            &AudienceDid::from(audience),
            &capabilities("did:key:z6Mkresource", "store/*"),
            &ProofReferences::empty(),
            token_time_bounds(),
        )
        .expect_err("refusing signer should fail");

        assert!(matches!(error, TokenError::Signer {
            source: SignerError::Refused
        }));
    }

    #[test]
    fn signer_with_unsupported_algorithm_is_rejected() {
        let key = signing_key(ISSUER_SECRET_BYTE);
        let signer = UnsupportedAlgorithmSigner::new(&key);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive");
        let error = issue_token_with_signer(
            &signer,
            &AudienceDid::from(audience),
            &capabilities("did:key:z6Mkresource", "store/*"),
            &ProofReferences::empty(),
            token_time_bounds(),
        )
        .expect_err("unsupported algorithm should fail");

        assert_eq!(error, TokenError::Signer {
            source: SignerError::UnsupportedAlgorithm {
                algorithm: UNSUPPORTED_TEST_ALGORITHM.to_owned()
            }
        });
    }

    #[test]
    fn signer_signature_constructor_rejects_wrong_length() {
        let short_signature = [0_u8; SHORT_SIGNATURE_LEN];
        let error = SignerSignature::try_from_slice(&short_signature).expect_err("short signature should fail");

        assert_eq!(error, SignerError::InvalidSignatureLength {
            actual: SHORT_SIGNATURE_LEN,
            expected: super::ED25519_SIGNATURE_BYTES,
        });
    }

    #[test]
    fn signer_with_invalid_signature_bytes_is_rejected_by_verification() {
        let key = signing_key(ISSUER_SECRET_BYTE);
        let signer = FixedSignatureSigner::new(&key, [0_u8; super::ED25519_SIGNATURE_BYTES]);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive");
        let token = issue_token_with_signer(
            &signer,
            &AudienceDid::from(audience),
            &capabilities("did:key:z6Mkresource", "store/*"),
            &ProofReferences::empty(),
            token_time_bounds(),
        )
        .expect("fixed-length malformed signature can produce a structurally valid token");
        let context = verification_context(vec![&key], ProofCollection::empty(), VERIFICATION_TIME_SECONDS);
        let error = verify_compact_token(&token, &context).expect_err("malformed signature should fail");

        assert!(matches!(error, TokenError::Signature { .. }));
    }

    #[test]
    fn signer_with_mismatched_issuer_and_public_key_is_rejected() {
        let key = signing_key(ISSUER_SECRET_BYTE);
        let other_issuer = signing_key(OTHER_SECRET_BYTE).issuer().expect("other issuer should derive");
        let signer = MismatchedIssuerSigner::new(&key, other_issuer);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive");
        let error = issue_token_with_signer(
            &signer,
            &AudienceDid::from(audience),
            &capabilities("did:key:z6Mkresource", "store/*"),
            &ProofReferences::empty(),
            token_time_bounds(),
        )
        .expect_err("issuer mismatch should fail");

        assert!(matches!(error, TokenError::Signer {
            source: SignerError::IssuerPublicKeyMismatch { .. }
        }));
    }

    #[test]
    fn malformed_compact_token_returns_structured_error() {
        let token = CompactToken::new("not.enough").expect("string wrapper should accept non-empty input");
        let error = parse_compact_token(&token).expect_err("two-part token should fail");

        assert_eq!(error, TokenError::InvalidCompactTokenStructure { actual_parts: 2 });

        let token = CompactToken::new("header.pay.load.signature")
            .expect("string wrapper should accept embedded separator shape");
        let error = parse_compact_token(&token).expect_err("four-part token should fail before decoding");

        assert_eq!(error, TokenError::InvalidCompactTokenStructure { actual_parts: 4 });
    }

    #[test]
    fn compact_codec_boundary_rejects_invalid_base64_and_json() {
        let invalid_base64 =
            CompactToken::new("not_base64.eyJpc3MiOiJkaWQ6a2V5OnoifQ.AAAA").expect("shape should wrap");
        let base64_error = parse_compact_token(&invalid_base64).expect_err("header base64 should reject");
        assert!(matches!(base64_error, TokenError::Base64Decode {
            part: super::CompactPart::Header,
            ..
        }));

        let invalid_padding = CompactToken::new("abc=.eyJpc3MiOiJkaWQ6a2V5OnoifQ.AAAA").expect("shape should wrap");
        let padding_error = parse_compact_token(&invalid_padding).expect_err("padding should reject");
        assert!(matches!(padding_error, TokenError::Base64Decode {
            part: super::CompactPart::Header,
            message,
        } if message.contains("invalid base64url-no-pad shape")));

        let invalid_length = CompactToken::new("A.eyJpc3MiOiJkaWQ6a2V5OnoifQ.AAAA").expect("shape should wrap");
        let length_error = parse_compact_token(&invalid_length).expect_err("length class should reject");
        assert!(matches!(length_error, TokenError::Base64Decode {
            part: super::CompactPart::Header,
            message,
        } if message.contains("invalid base64url-no-pad shape")));

        let invalid_json = CompactToken::new("bm90LWpzb24.eyJpc3MiOiJkaWQ6a2V5OnoifQ.AAAA").expect("shape should wrap");
        let json_error = parse_compact_token(&invalid_json).expect_err("header JSON should reject");
        assert!(matches!(json_error, TokenError::JsonDecode {
            part: super::CompactPart::Header,
            ..
        }));
    }

    #[test]
    fn compact_json_object_admission_rejects_non_object_decoded_header_and_payload() {
        let array_header = CompactToken::new("W10.eyJpc3MiOiJkaWQ6a2V5OnoifQ.AAAA").expect("shape should wrap");
        let array_error = parse_compact_token(&array_header).expect_err("array header should reject");
        assert!(matches!(array_error, TokenError::JsonDecode {
            part: super::CompactPart::Header,
            message,
        } if message.contains("decoded JSON must be an object")));

        let scalar_payload =
            CompactToken::new("IHsiYWxnIjoiRWREU0EiLCJ0eXAiOiJKV1QifSA.dHJ1ZQ.AAAA").expect("shape should wrap");
        let scalar_error = parse_compact_token(&scalar_payload).expect_err("scalar payload should reject");
        assert!(matches!(scalar_error, TokenError::JsonDecode {
            part: super::CompactPart::Payload,
            message,
        } if message.contains("decoded JSON must be an object")));

        let whitespace_payload =
            CompactToken::new("IHsiYWxnIjoiRWREU0EiLCJ0eXAiOiJKV1QifSA.ICAg.AAAA").expect("shape should wrap");
        let whitespace_error = parse_compact_token(&whitespace_payload).expect_err("whitespace payload should reject");
        assert!(matches!(whitespace_error, TokenError::JsonDecode {
            part: super::CompactPart::Payload,
            message,
        } if message.contains("decoded JSON must be an object")));
    }

    #[test]
    fn compact_signing_input_and_proof_reference_are_stable_boundary_values() {
        let header = "encoded-header";
        let payload = "encoded-payload";
        let token = test_compact_token(header, payload, "encoded-signature");
        let parts = super::compact_parts(token.as_str()).expect("compact token should split");
        assert_eq!(parts.signing_input, "encoded-header.encoded-payload");

        let first_reference = proof_reference(&token);
        let second_reference = proof_reference(&token);
        assert_eq!(first_reference, second_reference);
        assert_eq!(first_reference.as_bytes().len(), blake3::OUT_LEN);
    }

    #[test]
    fn parsed_header_admission_rejects_unsupported_fields() {
        let wrong_algorithm = token_with_header(
            r#"{"alg":"ES256","typ":"JWT"}"#,
            r#"{"iss":"did:key:zIssuer","aud":"did:key:zAudience","att":[],"prf":[],"exp":2,"nbf":1}"#,
        );
        let decoded = parse_compact_token(&wrong_algorithm).expect("token should parse before header admission");
        let error = validate_decoded_token(decoded).expect_err("unsupported algorithm should fail closed");
        assert!(matches!(error, TokenError::UnsupportedHeader));

        let wrong_type = token_with_header(
            r#"{"alg":"EdDSA","typ":"JWS"}"#,
            r#"{"iss":"did:key:zIssuer","aud":"did:key:zAudience","att":[],"prf":[],"exp":2,"nbf":1}"#,
        );
        let decoded = parse_compact_token(&wrong_type).expect("token should parse before header admission");
        let error = validate_decoded_token(decoded).expect_err("unsupported token type should fail closed");
        assert!(matches!(error, TokenError::UnsupportedHeader));
    }

    #[test]
    fn missing_required_claim_fails_through_verified_presence_admission() {
        let token = token_with_payload(r#"{"aud":"did:key:zAudience","att":[],"prf":[],"exp":2,"nbf":1}"#);
        let decoded = parse_compact_token(&token).expect("token should decode");
        let error = validate_decoded_token(decoded).expect_err("missing issuer should fail");

        assert!(matches!(error, TokenError::VerifiedPrimitive {
            source: crate::verified::VerifiedPrimitiveError::TokenValidation {
                failure: crate::verified::TokenValidationFailure::MissingIssuer
            }
        }));
    }

    #[test]
    fn signature_mismatch_is_rejected() {
        let issuer_key = signing_key(ISSUER_SECRET_BYTE);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive");
        let token = issue_token(&issue_request(&issuer_key, AudienceDid::from(audience), ProofCollection::empty()))
            .expect("token should issue");
        let tampered = tamper_signature(&token);
        let context = verification_context(vec![&issuer_key], ProofCollection::empty(), VERIFICATION_TIME_SECONDS);
        let error = verify_compact_token(&tampered, &context).expect_err("tampered signature should fail");

        assert!(matches!(error, TokenError::Signature { .. }));
    }

    #[test]
    fn malformed_public_api_inputs_fail_deterministically() {
        let short_key = [ISSUER_SECRET_BYTE; INVALID_KEY_LEN];
        let error = Ed25519SigningKey::try_from_slice(&short_key).expect_err("short key should fail");

        assert_eq!(error, TokenError::InvalidSecretKeyLength {
            actual: INVALID_KEY_LEN
        });
    }

    #[test]
    fn ed25519_signing_key_debug_redacts_secret_bytes() {
        let key = signing_key(REDACTION_TEST_SECRET_BYTE);
        let debug_output = format!("{key:?}");

        assert!(debug_output.contains("Ed25519SigningKey"));
        assert!(debug_output.contains(super::REDACTED_SECRET_MATERIAL_DEBUG));
        assert_secret_material_is_absent(debug_output.as_str());
    }

    #[test]
    fn issue_request_debug_redacts_embedded_signing_key() {
        let issuer_key = signing_key(REDACTION_TEST_SECRET_BYTE);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive");
        let request = issue_request(&issuer_key, AudienceDid::from(audience), ProofCollection::empty());
        let debug_output = format!("{request:?}");

        assert!(debug_output.contains("IssueRequest"));
        assert!(debug_output.contains(super::REDACTED_SECRET_MATERIAL_DEBUG));
        assert_secret_material_is_absent(debug_output.as_str());
    }

    #[test]
    fn in_memory_signer_debug_redacts_secret_bytes() {
        let signer = in_memory_signer(REDACTION_TEST_SECRET_BYTE);
        let debug_output = format!("{signer:?}");

        assert!(debug_output.contains("Ed25519InMemorySigner"));
        assert!(debug_output.contains(super::REDACTED_SECRET_MATERIAL_DEBUG));
        assert_secret_material_is_absent(debug_output.as_str());
    }

    #[test]
    fn assertion_failure_formatting_redacts_secret_bytes() {
        let redacted_key = signing_key(REDACTION_TEST_SECRET_BYTE);
        let other_signing_key = signing_key(OTHER_SECRET_BYTE);
        let key_failure = capture_assertion_failure_message(|| assert_eq!(redacted_key, other_signing_key));

        assert!(key_failure.contains("Ed25519SigningKey"));
        assert!(key_failure.contains(super::REDACTED_SECRET_MATERIAL_DEBUG));
        assert_secret_material_is_absent(key_failure.as_str());

        let redacted_signer = in_memory_signer(REDACTION_TEST_SECRET_BYTE);
        let other_signer = in_memory_signer(OTHER_SECRET_BYTE);
        let signer_failure = capture_assertion_failure_message(|| assert_eq!(redacted_signer, other_signer));

        assert!(signer_failure.contains("Ed25519InMemorySigner"));
        assert!(signer_failure.contains(super::REDACTED_SECRET_MATERIAL_DEBUG));
        assert_secret_material_is_absent(signer_failure.as_str());

        let issuer_key = signing_key(REDACTION_TEST_SECRET_BYTE);
        let other_issuer_key = signing_key(OTHER_SECRET_BYTE);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive");
        let request = issue_request(&issuer_key, AudienceDid::from(audience.clone()), ProofCollection::empty());
        let other_request = issue_request(&other_issuer_key, AudienceDid::from(audience), ProofCollection::empty());
        let request_failure = capture_assertion_failure_message(|| assert_eq!(request, other_request));

        assert!(request_failure.contains("IssueRequest"));
        assert!(request_failure.contains(super::REDACTED_SECRET_MATERIAL_DEBUG));
        assert_secret_material_is_absent(request_failure.as_str());
    }

    #[test]
    fn invalid_secret_key_errors_do_not_echo_input_bytes() {
        let malformed_key = [REDACTION_TEST_SECRET_BYTE; INVALID_KEY_LEN];
        let key_error = Ed25519SigningKey::try_from_slice(&malformed_key).expect_err("short key should fail");
        let signer_error = Ed25519InMemorySigner::try_from_slice(&malformed_key).expect_err("short signer should fail");

        assert!(matches!(key_error, TokenError::InvalidSecretKeyLength { .. }));
        assert_secret_material_is_absent(format!("{key_error}").as_str());
        assert_secret_material_is_absent(format!("{key_error:?}").as_str());

        assert!(matches!(signer_error, TokenError::InvalidSecretKeyLength { .. }));
        assert_secret_material_is_absent(format!("{signer_error}").as_str());
        assert_secret_material_is_absent(format!("{signer_error:?}").as_str());
    }

    #[test]
    fn exact_linear_delegation_emits_verified_proof_transcript() {
        let parent_key = signing_key(ISSUER_SECRET_BYTE);
        let child_key = signing_key(CHILD_SECRET_BYTE);
        let child_issuer = child_key.issuer().expect("child issuer should derive");
        let parent = issue_token(&issue_request_with_capability(
            &parent_key,
            AudienceDid::from(child_issuer),
            ProofCollection::empty(),
            "did:key:z6Mkresource",
            "store/add",
        ))
        .expect("parent should issue");
        let child = issue_token(&child_request(&child_key, parent.clone())).expect("child should issue");
        let context = verification_context(
            vec![&parent_key, &child_key],
            ProofCollection::from_tokens(vec![parent]),
            VERIFICATION_TIME_SECONDS,
        );

        let verified = verify_compact_token(&child, &context).expect("exact linear chain should verify");

        let transcript = verified.proof_transcript.as_ref().expect("exact linear chains should be transcript-checked");
        assert_eq!(transcript.entries.len(), 2);
        assert!(transcript.total_input_bytes > 0);
    }

    #[test]
    fn wildcard_linear_delegation_emits_verified_proof_transcript() {
        let verified = verified_child_token();

        let transcript =
            verified.proof_transcript.as_ref().expect("wildcard linear chains should be transcript-checked");
        assert_eq!(transcript.entries.len(), 2);
        assert_eq!(transcript.parent_indices, vec![Vec::<usize>::new(), vec![0]]);
        assert_eq!(transcript.leaf_indices, vec![1]);
        assert!(transcript.total_input_bytes > 0);
    }

    #[test]
    fn verified_token_reports_local_compact_protocol_profile() {
        let verified = verified_child_token();
        let profile = verified.protocol_profile();
        assert_eq!(profile.as_str(), crate::protocol_profile::ONIX_COMPACT_ED25519_V1);
        assert_ne!(profile.as_str(), crate::protocol_profile::UCAN_WG_1_0_0);
    }

    #[test]
    fn dagcbor_style_input_fails_compact_parse_without_dispatch() {
        let issuer_key = signing_key(ISSUER_SECRET_BYTE);
        let context = verification_context(vec![&issuer_key], ProofCollection::empty(), VERIFICATION_TIME_SECONDS);
        for dagcbor_style in ["", "cid:v1:dag-cbor:Qm....", "header.payload"] {
            let result = super::verify_compact_token_str(dagcbor_style, &context);
            assert!(
                result.is_err(),
                "UCAN 1.0.0-style input {dagcbor_style:?} must fail the compact parser without profile dispatch"
            );
        }
    }

    #[test]
    fn multi_parent_delegation_emits_verified_dag_proof_transcript() {
        let storage_parent_key = signing_key(ISSUER_SECRET_BYTE);
        let audit_parent_key = signing_key(OTHER_SECRET_BYTE);
        let child_key = signing_key(CHILD_SECRET_BYTE);
        let child_issuer = child_key.issuer().expect("child issuer should derive");
        let storage_parent = issue_token(&issue_request_with_capability(
            &storage_parent_key,
            AudienceDid::from(child_issuer.clone()),
            ProofCollection::empty(),
            "did:key:z6Mkresource",
            "store/*",
        ))
        .expect("first parent should issue");
        let audit_parent = issue_token(&issue_request_with_capability(
            &audit_parent_key,
            AudienceDid::from(child_issuer),
            ProofCollection::empty(),
            "did:key:z6Mkresource",
            "store/*",
        ))
        .expect("second parent should issue");
        let child =
            issue_token(&child_request_with_proofs(&child_key, vec![storage_parent.clone(), audit_parent.clone()]))
                .expect("child should issue");
        let context = verification_context(
            vec![&storage_parent_key, &audit_parent_key, &child_key],
            ProofCollection::from_tokens(vec![storage_parent, audit_parent]),
            VERIFICATION_TIME_SECONDS,
        );

        let verified = verify_compact_token(&child, &context).expect("multi-parent chain should verify");

        let transcript =
            verified.proof_transcript.as_ref().expect("multi-parent chains should be DAG transcript-checked");
        assert_eq!(transcript.entries.len(), 3);
        assert_eq!(transcript.parent_indices, vec![Vec::<usize>::new(), Vec::new(), vec![0, 1]]);
        assert_eq!(transcript.leaf_indices, vec![2]);
        assert!(transcript.total_input_bytes > 0);
    }

    #[derive(Debug, Clone, Copy)]
    struct FailingRevocationChecker;

    impl RevocationChecker for FailingRevocationChecker {
        fn is_revoked(&self, reference: &ProofReference) -> std::result::Result<bool, RevocationError> {
            Err(RevocationError::Backend {
                reference: reference.clone(),
                message: "fixture revocation backend unavailable".to_owned(),
            })
        }
    }

    #[derive(Debug, Clone, Copy)]
    struct FailingProofStore;

    impl ProofStore for FailingProofStore {
        fn resolve_proof<'a>(
            &'a self,
            reference: &ProofReference,
        ) -> std::result::Result<ProofToken<'a>, ProofStoreError> {
            Err(ProofStoreError::Backend {
                reference: reference.clone(),
                message: "fixture proof backend unavailable".to_owned(),
            })
        }
    }

    #[test]
    fn revocation_backend_failure_fails_closed_before_authorization() {
        let issuer_key = signing_key(ISSUER_SECRET_BYTE);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience should derive");
        let token = issue_token(&issue_request(&issuer_key, AudienceDid::from(audience), ProofCollection::empty()))
            .expect("token should issue");
        let keys = KeyResolutionContext::new(vec![
            Ed25519VerificationKey::from_signing_key(&issuer_key).expect("verification key should derive"),
        ]);

        let error = verify_compact_token_with_resolvers_and_revocations(
            &token,
            VerificationTime::from_unix_seconds(VERIFICATION_TIME_SECONDS),
            &keys,
            &ProofCollection::empty(),
            &FailingRevocationChecker,
            VerificationLimits::default(),
        )
        .expect_err("revocation backend failure must deny");

        assert!(matches!(error, TokenError::Revocation { .. }));
    }

    #[test]
    fn proof_store_backend_failure_fails_closed_after_successful_revocation_and_key_resolution() {
        let parent_key = signing_key(ISSUER_SECRET_BYTE);
        let child_key = signing_key(CHILD_SECRET_BYTE);
        let child_issuer = child_key.issuer().expect("child issuer should derive");
        let parent =
            issue_token(&issue_request(&parent_key, AudienceDid::from(child_issuer), ProofCollection::empty()))
                .expect("parent should issue");
        let child = issue_token(&child_request(&child_key, parent)).expect("child should issue");
        let keys = KeyResolutionContext::new(vec![
            Ed25519VerificationKey::from_signing_key(&parent_key).expect("parent verification key should derive"),
            Ed25519VerificationKey::from_signing_key(&child_key).expect("child verification key should derive"),
        ]);

        let error = verify_compact_token_with_resolvers_and_revocations(
            &child,
            VerificationTime::from_unix_seconds(VERIFICATION_TIME_SECONDS),
            &keys,
            &FailingProofStore,
            &NoRevocations,
            VerificationLimits::default(),
        )
        .expect_err("proof store backend failure must deny");

        assert!(matches!(error, TokenError::ProofStore { .. }));
    }

    #[test]
    fn valid_delegation_chain_exposes_effective_constraints() {
        let parent_key = signing_key(ISSUER_SECRET_BYTE);
        let child_key = signing_key(CHILD_SECRET_BYTE);
        let child_issuer = child_key.issuer().expect("child issuer should derive");
        let parent =
            issue_token(&issue_request(&parent_key, AudienceDid::from(child_issuer.clone()), ProofCollection::empty()))
                .expect("parent should issue");
        let child = issue_token(&child_request(&child_key, parent.clone())).expect("child should issue");
        let context = verification_context(
            vec![&parent_key, &child_key],
            ProofCollection::from_tokens(vec![parent]),
            VERIFICATION_TIME_SECONDS,
        );

        let verified = verify_compact_token(&child, &context).expect("delegation chain should verify");

        assert_eq!(verified.issuer().as_str(), child_issuer.as_str());
        assert_eq!(verified.proofs().as_slice().len(), 1);
        assert_eq!(verified.effective_delegation().time_bounds().expires_at_seconds(), CHILD_EXPIRES_AT_SECONDS);
    }

    #[test]
    fn child_must_preserve_parent_caveat_payload_for_attenuation() {
        let parent_key = signing_key(ISSUER_SECRET_BYTE);
        let child_key = signing_key(CHILD_SECRET_BYTE);
        let child_issuer = child_key.issuer().expect("child issuer should derive");
        let parent_caveat = caveat("time", "before", "unix", b"2000");
        let parent = issue_token(&issue_request_with_caveat(
            &parent_key,
            AudienceDid::from(child_issuer),
            ProofCollection::empty(),
            parent_caveat.clone(),
        ))
        .expect("parent should issue");
        let missing_caveat_child = issue_token(&child_request(&child_key, parent.clone())).expect("child should issue");
        let narrowed_child = issue_token(&child_request_with_caveat(&child_key, parent.clone(), parent_caveat))
            .expect("child should issue");
        let context = verification_context(
            vec![&parent_key, &child_key],
            ProofCollection::from_tokens(vec![parent]),
            VERIFICATION_TIME_SECONDS,
        );

        let missing_error = verify_compact_token(&missing_caveat_child, &context)
            .expect_err("child that drops parent caveat should fail attenuation");
        let verified = verify_compact_token(&narrowed_child, &context).expect("preserved caveat should verify");

        assert!(matches!(missing_error, TokenError::Attenuation { .. }));
        assert_eq!(verified.effective_delegation().capabilities().as_slice()[0].caveats.len(), 1);
    }

    #[test]
    fn child_cannot_satisfy_parent_caveat_with_unrelated_caveat_payload() {
        let parent_key = signing_key(ISSUER_SECRET_BYTE);
        let child_key = signing_key(CHILD_SECRET_BYTE);
        let child_issuer = child_key.issuer().expect("child issuer should derive");
        let required_caveat = caveat("time", "before", "unix", b"2000");
        let unrelated_caveat = caveat("time", "before", "unix", b"1999");
        let parent = issue_token(&issue_request_with_caveat(
            &parent_key,
            AudienceDid::from(child_issuer),
            ProofCollection::empty(),
            required_caveat,
        ))
        .expect("parent should issue");
        let child = issue_token(&child_request_with_caveat(&child_key, parent.clone(), unrelated_caveat))
            .expect("child should issue");
        let context = verification_context(
            vec![&parent_key, &child_key],
            ProofCollection::from_tokens(vec![parent]),
            VERIFICATION_TIME_SECONDS,
        );
        let error = verify_compact_token(&child, &context)
            .expect_err("child with unrelated caveat payload should fail attenuation");

        assert!(matches!(error, TokenError::Attenuation { .. }));
    }

    #[test]
    fn over_broad_child_capability_is_rejected() {
        let parent_key = signing_key(ISSUER_SECRET_BYTE);
        let child_key = signing_key(CHILD_SECRET_BYTE);
        let child_issuer = child_key.issuer().expect("child issuer should derive");
        let parent =
            issue_token(&issue_request(&parent_key, AudienceDid::from(child_issuer), ProofCollection::empty()))
                .expect("parent should issue");
        let child = issue_token(&over_broad_child_request(&child_key, parent.clone())).expect("child should issue");
        let context = verification_context(
            vec![&parent_key, &child_key],
            ProofCollection::from_tokens(vec![parent]),
            VERIFICATION_TIME_SECONDS,
        );
        let error = verify_compact_token(&child, &context).expect_err("over-broad child should fail");

        assert!(matches!(error, TokenError::Attenuation { .. }));
    }

    #[test]
    fn widened_child_window_is_rejected_as_temporal_error() {
        let parent_key = signing_key(ISSUER_SECRET_BYTE);
        let child_key = signing_key(CHILD_SECRET_BYTE);
        let child_issuer = child_key.issuer().expect("child issuer should derive");
        let parent =
            issue_token(&issue_request(&parent_key, AudienceDid::from(child_issuer), ProofCollection::empty()))
                .expect("parent should issue");
        let child = issue_token(&widened_child_request(&child_key, parent.clone())).expect("child should issue");
        let context = verification_context(
            vec![&parent_key, &child_key],
            ProofCollection::from_tokens(vec![parent]),
            VERIFICATION_TIME_SECONDS,
        );
        let error = verify_compact_token(&child, &context).expect_err("widened child should fail");

        assert!(matches!(error, TokenError::Temporal { .. }));
    }

    #[test]
    fn expired_parent_is_rejected_as_temporal_error() {
        let parent_key = signing_key(ISSUER_SECRET_BYTE);
        let child_key = signing_key(CHILD_SECRET_BYTE);
        let child_issuer = child_key.issuer().expect("child issuer should derive");
        let parent =
            issue_token(&issue_request(&parent_key, AudienceDid::from(child_issuer), ProofCollection::empty()))
                .expect("parent should issue");
        let child = issue_token(&child_request(&child_key, parent.clone())).expect("child should issue");
        let context = verification_context(
            vec![&parent_key, &child_key],
            ProofCollection::from_tokens(vec![parent]),
            EXPIRED_TIME_SECONDS,
        );
        let error = verify_compact_token(&child, &context).expect_err("expired parent should fail");

        assert!(matches!(error, TokenError::Temporal { .. }));
    }

    const BOUNDARY_ISSUER_BYTE: u8 = 23;
    const BOUNDARY_AUDIENCE_BYTE: u8 = 29;
    const BOUNDARY_NOT_BEFORE_SECONDS: u64 = 1_000;
    const BOUNDARY_EXPIRES_AT_SECONDS: u64 = 1_200;
    const CLOCK_SKEW_GRACE_SECONDS: u64 = crate::verified::DEFAULT_CLOCK_SKEW_SECONDS;

    fn boundary_scope_token() -> CompactToken {
        let key = signing_key(BOUNDARY_ISSUER_BYTE);
        let audience = signing_key(BOUNDARY_AUDIENCE_BYTE).issuer().expect("audience should derive");
        let request = IssueRequest::new(
            key,
            AudienceDid::from(audience),
            capabilities("did:key:z6Mkresource", "store/*"),
            ProofCollection::empty().into(),
            TokenTimeBounds::new(BOUNDARY_NOT_BEFORE_SECONDS, BOUNDARY_EXPIRES_AT_SECONDS)
                .expect("bounds should validate"),
        );
        issue_token(&request).expect("token should issue")
    }

    fn boundary_scope_keys() -> KeyResolutionContext {
        KeyResolutionContext::new(vec![
            Ed25519VerificationKey::from_signing_key(&signing_key(BOUNDARY_ISSUER_BYTE)).expect("key should derive"),
        ])
    }

    #[test]
    fn clock_skew_default_grace_admits_documented_boundaries() {
        let token = boundary_scope_token();
        let keys = boundary_scope_keys();
        for at in [
            BOUNDARY_NOT_BEFORE_SECONDS - 1,
            BOUNDARY_NOT_BEFORE_SECONDS,
            BOUNDARY_EXPIRES_AT_SECONDS,
            BOUNDARY_EXPIRES_AT_SECONDS + CLOCK_SKEW_GRACE_SECONDS,
        ] {
            let result = super::verify_compact_token_with_resolvers(
                &token,
                VerificationTime::from_unix_seconds(at),
                &keys,
                &ProofCollection::empty(),
                VerificationLimits::default(),
            );
            assert!(result.is_ok(), "default surface must admit the in-grace boundary time {at}");
        }
    }

    #[test]
    fn clock_skew_out_of_grace_reports_temporal_error() {
        let token = boundary_scope_token();
        let keys = boundary_scope_keys();
        for at in [
            BOUNDARY_NOT_BEFORE_SECONDS - CLOCK_SKEW_GRACE_SECONDS - 1,
            BOUNDARY_EXPIRES_AT_SECONDS + CLOCK_SKEW_GRACE_SECONDS + 1,
        ] {
            let error = super::verify_compact_token_with_resolvers(
                &token,
                VerificationTime::from_unix_seconds(at),
                &keys,
                &ProofCollection::empty(),
                VerificationLimits::default(),
            )
            .expect_err("out-of-grace time must be rejected");
            assert!(
                matches!(error, TokenError::Temporal { .. }),
                "boundary time {at} must fail as a temporal error, got {error:?}"
            );
        }
    }

    #[test]
    fn clock_skew_strict_opt_in_rejects_in_grace_token() {
        let token = boundary_scope_token();
        let keys = boundary_scope_keys();
        let at = BOUNDARY_EXPIRES_AT_SECONDS + 50;

        let default_result = super::verify_compact_token_with_resolvers(
            &token,
            VerificationTime::from_unix_seconds(at),
            &keys,
            &ProofCollection::empty(),
            VerificationLimits::default(),
        );
        assert!(default_result.is_ok(), "default surface must remain lenient at {at}");

        let strict_error = super::verify_compact_token_with_resolvers(
            &token,
            VerificationTime::from_unix_seconds(at),
            &keys,
            &ProofCollection::empty(),
            VerificationLimits::strict(),
        )
        .expect_err("strict surface must reject an in-grace token");
        assert!(matches!(strict_error, TokenError::Temporal { .. }));
    }

    const LEAK_REGRESSION_ISSUER_BYTE: u8 = 31;
    const LEAK_REGRESSION_AUDIENCE_BYTE: u8 = 37;
    const LARGE_CAVEAT_PAYLOAD_BYTES: usize = 4_096;
    const REPEATED_VERIFICATION_COUNT: usize = 4_096;
    const REPEATED_MULTI_PARENT_VERIFICATION_COUNT: usize = 1_024;
    const REPEATED_ISSUANCE_COUNT: usize = 1_024;
    const MAX_RETAINED_GROWTH_BYTES: isize = 64 * 1024;

    fn retained_allocator_bytes() -> isize {
        LIVE_ALLOCATED_BYTES.load(Ordering::Relaxed)
    }

    fn assert_repeated_verification_retains_bounded_memory(
        label: &str,
        token: &CompactToken,
        context: &VerificationContext,
        repetitions: usize,
    ) {
        for _ in 0..3 {
            verify_compact_token(token, context).expect("token should verify");
        }
        let before = retained_allocator_bytes();
        for _ in 0..repetitions {
            verify_compact_token(token, context).expect("token should verify");
        }
        let growth = retained_allocator_bytes() - before;
        assert!(
            growth <= MAX_RETAINED_GROWTH_BYTES,
            "{label}: retained memory grew {growth} bytes across {repetitions} verifications; the leak-free bound is {MAX_RETAINED_GROWTH_BYTES}"
        );
    }

    #[test]
    fn repeated_caveated_verification_retains_bounded_memory() {
        let issuer_key = signing_key(LEAK_REGRESSION_ISSUER_BYTE);
        let audience = signing_key(LEAK_REGRESSION_AUDIENCE_BYTE).issuer().expect("audience should derive");
        let token = issue_token(&issue_request_with_caveat(
            &issuer_key,
            AudienceDid::from(audience),
            ProofCollection::empty(),
            caveat("rate", "limit", "quota", &[b'x'; LARGE_CAVEAT_PAYLOAD_BYTES]),
        ))
        .expect("caveated token should issue");
        let context = VerificationContext::new(
            VerificationTime::from_unix_seconds(VERIFICATION_TIME_SECONDS),
            KeyResolutionContext::new(vec![
                Ed25519VerificationKey::from_signing_key(&issuer_key).expect("key should derive"),
            ]),
            ProofCollection::empty(),
        );

        assert_repeated_verification_retains_bounded_memory(
            "caveated verification",
            &token,
            &context,
            REPEATED_VERIFICATION_COUNT,
        );
    }

    #[test]
    fn repeated_caveat_free_verification_retains_bounded_memory() {
        let issuer_key = signing_key(LEAK_REGRESSION_ISSUER_BYTE);
        let audience = signing_key(LEAK_REGRESSION_AUDIENCE_BYTE).issuer().expect("audience should derive");
        let token = issue_token(&issue_request(&issuer_key, AudienceDid::from(audience), ProofCollection::empty()))
            .expect("token should issue");
        let context = VerificationContext::new(
            VerificationTime::from_unix_seconds(VERIFICATION_TIME_SECONDS),
            KeyResolutionContext::new(vec![
                Ed25519VerificationKey::from_signing_key(&issuer_key).expect("key should derive"),
            ]),
            ProofCollection::empty(),
        );

        assert_repeated_verification_retains_bounded_memory(
            "caveat-free verification",
            &token,
            &context,
            REPEATED_VERIFICATION_COUNT,
        );
    }

    #[test]
    fn repeated_single_parent_traversal_retains_bounded_memory() {
        let parent_key = signing_key(LEAK_REGRESSION_ISSUER_BYTE);
        let child_key = signing_key(LEAK_REGRESSION_AUDIENCE_BYTE);
        let child_issuer = child_key.issuer().expect("child issuer should derive");
        let shared_caveat = caveat("rate", "limit", "quota", &[b'x'; LARGE_CAVEAT_PAYLOAD_BYTES]);
        let parent = issue_token(&issue_request_with_caveat(
            &parent_key,
            AudienceDid::from(child_issuer),
            ProofCollection::empty(),
            shared_caveat.clone(),
        ))
        .expect("parent should issue");
        let child = issue_token(&child_request_with_caveat(&child_key, parent.clone(), shared_caveat))
            .expect("child should issue");
        let context = verification_context(
            vec![&parent_key, &child_key],
            ProofCollection::from_tokens(vec![parent]),
            VERIFICATION_TIME_SECONDS,
        );

        assert_repeated_verification_retains_bounded_memory(
            "single-parent traversal",
            &child,
            &context,
            REPEATED_VERIFICATION_COUNT,
        );
    }

    #[test]
    fn repeated_multi_parent_traversal_retains_bounded_memory() {
        let storage_parent_key = signing_key(LEAK_REGRESSION_ISSUER_BYTE);
        let audit_parent_key = signing_key(LEAK_REGRESSION_AUDIENCE_BYTE);
        let child_key = signing_key(CHILD_SECRET_BYTE);
        let child_issuer = child_key.issuer().expect("child issuer should derive");
        let shared_caveat = caveat("rate", "limit", "quota", &[b'x'; LARGE_CAVEAT_PAYLOAD_BYTES]);
        let storage_parent = issue_token(&issue_request_with_caveat(
            &storage_parent_key,
            AudienceDid::from(child_issuer.clone()),
            ProofCollection::empty(),
            shared_caveat.clone(),
        ))
        .expect("first parent should issue");
        let audit_parent = issue_token(&issue_request_with_caveat(
            &audit_parent_key,
            AudienceDid::from(child_issuer),
            ProofCollection::empty(),
            shared_caveat.clone(),
        ))
        .expect("second parent should issue");
        let child = issue_token(&IssueRequest::new(
            child_key.clone(),
            AudienceDid::from(signing_key(OTHER_SECRET_BYTE).issuer().expect("audience should derive")),
            CapabilitySet::new(vec![
                CapabilityDocument::with_caveats(
                    "did:key:z6Mkresource/photos".to_owned(),
                    "store/add".to_owned(),
                    vec![shared_caveat],
                )
                .expect("capability with caveat should validate"),
            ])
            .expect("capability set should validate"),
            ProofCollection::from_tokens(vec![storage_parent.clone(), audit_parent.clone()]).into(),
            TokenTimeBounds::new(CHILD_NOT_BEFORE_SECONDS, CHILD_EXPIRES_AT_SECONDS)
                .expect("child bounds should validate"),
        ))
        .expect("child should issue");
        let context = verification_context(
            vec![&storage_parent_key, &audit_parent_key, &child_key],
            ProofCollection::from_tokens(vec![storage_parent, audit_parent]),
            VERIFICATION_TIME_SECONDS,
        );

        assert_repeated_verification_retains_bounded_memory(
            "multi-parent traversal",
            &child,
            &context,
            REPEATED_MULTI_PARENT_VERIFICATION_COUNT,
        );
    }

    #[test]
    fn repeated_caveated_issuance_retains_bounded_memory() {
        let signed_issuer = in_memory_signer(LEAK_REGRESSION_ISSUER_BYTE);
        let signed_audience_key = in_memory_signer(LEAK_REGRESSION_AUDIENCE_BYTE);
        let auditor_did = AudienceDid::from(signed_audience_key.issuer().expect("audience should derive"));
        let caps =
            CapabilitySet::new(vec![
                CapabilityDocument::with_caveats("did:key:z6Mkresource".to_owned(), "store/*".to_owned(), vec![
                    caveat("rate", "limit", "quota", &[b'x'; LARGE_CAVEAT_PAYLOAD_BYTES]),
                ])
                .expect("capability with caveat should validate"),
            ])
            .expect("capability set should validate");
        let mut last_token_reference = None;
        for _ in 0..3 {
            let token = super::issue_token_with_signer(
                &signed_issuer,
                &auditor_did,
                &caps,
                &super::ProofReferences::empty(),
                token_time_bounds(),
            )
            .expect("token should issue");
            last_token_reference = Some(proof_reference(&token));
        }
        let before = retained_allocator_bytes();
        for _ in 0..REPEATED_ISSUANCE_COUNT {
            let token = super::issue_token_with_signer(
                &signed_issuer,
                &auditor_did,
                &caps,
                &super::ProofReferences::empty(),
                token_time_bounds(),
            )
            .expect("token should issue");
            last_token_reference = Some(proof_reference(&token));
        }
        let growth = retained_allocator_bytes() - before;
        assert!(
            growth <= MAX_RETAINED_GROWTH_BYTES,
            "caveated issuance: retained memory grew {growth} bytes across {REPEATED_ISSUANCE_COUNT} issuances; the leak-free bound is {MAX_RETAINED_GROWTH_BYTES}"
        );
        assert!(last_token_reference.is_some());
    }
    #[test]
    fn self_delegation_is_rejected_directly_and_in_chain() {
        let key = signing_key(ISSUER_SECRET_BYTE);
        let self_did = key.issuer().expect("issuer should derive");
        let token = issue_token(&issue_request(&key, AudienceDid::from(self_did), ProofCollection::empty()))
            .expect("self-delegating token can be issued but not verified");
        let context = verification_context(vec![&key], ProofCollection::empty(), VERIFICATION_TIME_SECONDS);
        let error = verify_compact_token(&token, &context).expect_err("self delegation should fail");

        assert!(matches!(error, TokenError::SelfDelegation { .. }));
    }

    #[test]
    fn self_delegating_parent_is_rejected_in_chain() {
        let parent_key = signing_key(ISSUER_SECRET_BYTE);
        let child_key = signing_key(CHILD_SECRET_BYTE);
        let self_did = parent_key.issuer().expect("parent issuer should derive");
        let parent = issue_token(&issue_request(&parent_key, AudienceDid::from(self_did), ProofCollection::empty()))
            .expect("parent can be issued");
        let child = issue_token(&child_request(&child_key, parent.clone())).expect("child should issue");
        let context = verification_context(
            vec![&parent_key, &child_key],
            ProofCollection::from_tokens(vec![parent]),
            VERIFICATION_TIME_SECONDS,
        );
        let error = verify_compact_token(&child, &context).expect_err("self-delegating parent should fail");

        assert!(matches!(error, TokenError::SelfDelegation { .. }));
    }

    #[test]
    fn broken_issuer_audience_link_is_proof_chain_error() {
        let parent_key = signing_key(ISSUER_SECRET_BYTE);
        let child_key = signing_key(CHILD_SECRET_BYTE);
        let wrong_audience = signing_key(OTHER_SECRET_BYTE).issuer().expect("wrong audience should derive");
        let parent =
            issue_token(&issue_request(&parent_key, AudienceDid::from(wrong_audience), ProofCollection::empty()))
                .expect("parent should issue");
        let child = issue_token(&child_request(&child_key, parent.clone())).expect("child should issue");
        let context = verification_context(
            vec![&parent_key, &child_key],
            ProofCollection::from_tokens(vec![parent]),
            VERIFICATION_TIME_SECONDS,
        );
        let error = verify_compact_token(&child, &context).expect_err("broken proof link should fail");

        assert!(matches!(error, TokenError::ProofChain { .. }));
    }

    #[test]
    fn missing_proof_material_is_proof_chain_error() {
        let parent_key = signing_key(ISSUER_SECRET_BYTE);
        let child_key = signing_key(CHILD_SECRET_BYTE);
        let child_issuer = child_key.issuer().expect("child issuer should derive");
        let parent =
            issue_token(&issue_request(&parent_key, AudienceDid::from(child_issuer), ProofCollection::empty()))
                .expect("parent should issue");
        let child = issue_token(&child_request(&child_key, parent)).expect("child should issue");
        let context =
            verification_context(vec![&parent_key, &child_key], ProofCollection::empty(), VERIFICATION_TIME_SECONDS);
        let error = verify_compact_token(&child, &context).expect_err("missing proof should fail");

        assert!(matches!(error, TokenError::ProofStore {
            source: ProofStoreError::MissingProof { .. }
        }));
    }

    #[test]
    fn resolver_backend_errors_preserve_issuer_context() {
        let issuer_key = signing_key(ISSUER_SECRET_BYTE);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive");
        let token = issue_token(&issue_request(&issuer_key, AudienceDid::from(audience), ProofCollection::empty()))
            .expect("token should issue");
        let context = verification_context(vec![], ProofCollection::empty(), VERIFICATION_TIME_SECONDS);
        let resolver = BackendFailingResolver;
        let error = super::verify_compact_token_with_resolvers(
            &token,
            VerificationTime::from_unix_seconds(VERIFICATION_TIME_SECONDS),
            &resolver,
            &context.proofs,
            context.limits(),
        )
        .expect_err("resolver backend should fail");

        assert!(matches!(error, TokenError::KeyResolver {
            source: KeyResolverError::Backend { issuer, message }
        } if issuer == issuer_key.issuer().expect("issuer should derive").as_str() && message == BACKEND_FAILURE_MESSAGE));
    }

    #[test]
    fn proof_store_backend_errors_preserve_reference_context() {
        let parent_key = signing_key(ISSUER_SECRET_BYTE);
        let child_key = signing_key(CHILD_SECRET_BYTE);
        let child_issuer = child_key.issuer().expect("child issuer should derive");
        let parent =
            issue_token(&issue_request(&parent_key, AudienceDid::from(child_issuer), ProofCollection::empty()))
                .expect("parent should issue");
        let child = issue_token(&child_request(&child_key, parent)).expect("child should issue");
        let keys = KeyResolutionContext::new(
            [&parent_key, &child_key]
                .into_iter()
                .map(Ed25519VerificationKey::from_signing_key)
                .collect::<Result<Vec<_>, _>>()
                .expect("keys should derive"),
        );
        let store = BackendFailingProofStore;
        let error = super::verify_compact_token_with_resolvers(
            &child,
            VerificationTime::from_unix_seconds(VERIFICATION_TIME_SECONDS),
            &keys,
            &store,
            VerificationLimits::default(),
        )
        .expect_err("proof store backend should fail");

        assert!(matches!(error, TokenError::ProofStore {
            source: ProofStoreError::Backend { reference, message }
        } if !reference.as_bytes().is_empty() && message == BACKEND_FAILURE_MESSAGE));
    }

    #[test]
    fn no_revocations_preserves_delegation_verification() {
        let parent_key = signing_key(ISSUER_SECRET_BYTE);
        let child_key = signing_key(CHILD_SECRET_BYTE);
        let child_issuer = child_key.issuer().expect("child issuer should derive");
        let parent =
            issue_token(&issue_request(&parent_key, AudienceDid::from(child_issuer), ProofCollection::empty()))
                .expect("parent should issue");
        let child = issue_token(&child_request(&child_key, parent.clone())).expect("child should issue");
        let context = verification_context(
            vec![&parent_key, &child_key],
            ProofCollection::from_tokens(vec![parent]),
            VERIFICATION_TIME_SECONDS,
        );

        let baseline = verify_compact_token(&child, &context).expect("baseline should verify");
        let checked = super::verify_compact_token_with_resolvers_and_revocations(
            &child,
            context.time(),
            context.keys(),
            context.proofs(),
            &NoRevocations,
            context.limits(),
        )
        .expect("no-op revocation checker should preserve verification");

        assert_eq!(baseline.issuer(), checked.issuer());
        assert_eq!(baseline.audience(), checked.audience());
        assert_eq!(baseline.proofs(), checked.proofs());
        assert_eq!(baseline.effective_delegation(), checked.effective_delegation());
    }

    #[test]
    fn revocation_list_revoke_is_idempotent_through_verified_set_membership() {
        let reference = ProofReference::new(b"proof-a".to_vec());
        let mut revocations = RevocationList::empty();

        revocations.revoke(reference.clone());
        revocations.revoke(reference.clone());

        assert_eq!(revocations.as_slice(), &[reference]);
    }

    #[test]
    fn revoked_leaf_token_is_rejected() {
        let issuer_key = signing_key(ISSUER_SECRET_BYTE);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive");
        let token = issue_token(&issue_request(&issuer_key, AudienceDid::from(audience), ProofCollection::empty()))
            .expect("token should issue");
        let context = verification_context(vec![&issuer_key], ProofCollection::empty(), VERIFICATION_TIME_SECONDS);
        let revoked = RevocationList::from_references(vec![proof_reference(&token)]);

        let error = super::verify_compact_token_with_resolvers_and_revocations(
            &token,
            context.time(),
            context.keys(),
            context.proofs(),
            &revoked,
            context.limits(),
        )
        .expect_err("revoked leaf should fail");

        assert!(matches!(error, TokenError::Revoked { reference } if reference == proof_reference(&token)));
    }

    #[test]
    fn revoked_parent_token_rejects_delegated_child() {
        let parent_key = signing_key(ISSUER_SECRET_BYTE);
        let child_key = signing_key(CHILD_SECRET_BYTE);
        let child_issuer = child_key.issuer().expect("child issuer should derive");
        let parent =
            issue_token(&issue_request(&parent_key, AudienceDid::from(child_issuer), ProofCollection::empty()))
                .expect("parent should issue");
        let child = issue_token(&child_request(&child_key, parent.clone())).expect("child should issue");
        let context = verification_context(
            vec![&parent_key, &child_key],
            ProofCollection::from_tokens(vec![parent.clone()]),
            VERIFICATION_TIME_SECONDS,
        );
        let revoked = RevocationList::from_references(vec![proof_reference(&parent)]);

        let error = super::verify_compact_token_with_resolvers_and_revocations(
            &child,
            context.time(),
            context.keys(),
            context.proofs(),
            &revoked,
            context.limits(),
        )
        .expect_err("revoked parent should fail child verification");

        assert!(matches!(error, TokenError::Revoked { reference } if reference == proof_reference(&parent)));
    }

    #[test]
    fn revocation_backend_errors_fail_closed() {
        let issuer_key = signing_key(ISSUER_SECRET_BYTE);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience did should derive");
        let token = issue_token(&issue_request(&issuer_key, AudienceDid::from(audience), ProofCollection::empty()))
            .expect("token should issue");
        let context = verification_context(vec![&issuer_key], ProofCollection::empty(), VERIFICATION_TIME_SECONDS);
        let error = super::verify_compact_token_with_resolvers_and_revocations(
            &token,
            context.time(),
            context.keys(),
            context.proofs(),
            &BackendFailingRevocationChecker,
            context.limits(),
        )
        .expect_err("revocation backend failure should fail closed");

        assert!(matches!(error, TokenError::Revocation {
            source: RevocationError::Backend { reference, message }
        } if reference == proof_reference(&token) && message == BACKEND_FAILURE_MESSAGE));
    }

    #[test]
    fn verification_limits_reject_oversized_input_before_parse() {
        let context = VerificationContext::with_limits(
            VerificationTime::from_unix_seconds(VERIFICATION_TIME_SECONDS),
            KeyResolutionContext::new(Vec::new()),
            ProofCollection::empty(),
            VerificationLimits::new(1, 1, 3),
        );
        let error = super::verify_compact_token_str("not.enough.parts", &context)
            .expect_err("oversized input should fail before parse");

        assert!(matches!(error, TokenError::LimitExceeded {
            source: VerificationLimitError::CompactTokenTooLarge {
                byte_len: 16,
                max_byte_len: 3
            }
        }));
    }

    #[test]
    fn proof_depth_limit_rejects_parent_traversal() {
        let parent_key = signing_key(ISSUER_SECRET_BYTE);
        let child_key = signing_key(CHILD_SECRET_BYTE);
        let child_issuer = child_key.issuer().expect("child issuer should derive");
        let parent =
            issue_token(&issue_request(&parent_key, AudienceDid::from(child_issuer), ProofCollection::empty()))
                .expect("parent should issue");
        let child = issue_token(&child_request(&child_key, parent.clone())).expect("child should issue");
        let context = VerificationContext::with_limits(
            VerificationTime::from_unix_seconds(VERIFICATION_TIME_SECONDS),
            KeyResolutionContext::new(
                [&parent_key, &child_key]
                    .into_iter()
                    .map(Ed25519VerificationKey::from_signing_key)
                    .collect::<Result<Vec<_>, _>>()
                    .expect("keys should derive"),
            ),
            ProofCollection::from_tokens(vec![parent]),
            VerificationLimits::new(0, 1, usize::MAX),
        );
        let error = verify_compact_token(&child, &context).expect_err("depth limit should fail");

        assert!(matches!(error, TokenError::LimitExceeded {
            source: VerificationLimitError::ProofDepthExceeded { depth: 1, max_depth: 0 }
        }));
    }

    #[test]
    fn proof_count_limit_stops_before_proof_lookup() {
        let parent_key = signing_key(ISSUER_SECRET_BYTE);
        let child_key = signing_key(CHILD_SECRET_BYTE);
        let child_issuer = child_key.issuer().expect("child issuer should derive");
        let parent =
            issue_token(&issue_request(&parent_key, AudienceDid::from(child_issuer), ProofCollection::empty()))
                .expect("parent should issue");
        let child = issue_token(&child_request(&child_key, parent)).expect("child should issue");
        let keys = KeyResolutionContext::new(
            [&parent_key, &child_key]
                .into_iter()
                .map(Ed25519VerificationKey::from_signing_key)
                .collect::<Result<Vec<_>, _>>()
                .expect("keys should derive"),
        );
        let limits = VerificationLimits::new(1, 0, usize::MAX);
        let error = super::verify_compact_token_with_resolvers(
            &child,
            VerificationTime::from_unix_seconds(VERIFICATION_TIME_SECONDS),
            &keys,
            &CountingProofStore,
            limits,
        )
        .expect_err("proof count limit should fail");

        assert!(matches!(error, TokenError::LimitExceeded {
            source: VerificationLimitError::ProofCountExceeded { count: 1, max_count: 0 }
        }));
    }

    #[test]
    fn resolved_proof_size_limit_precedes_parsing() {
        let parent_key = signing_key(ISSUER_SECRET_BYTE);
        let child_key = signing_key(CHILD_SECRET_BYTE);
        let child_issuer = child_key.issuer().expect("child issuer should derive");
        let parent =
            issue_token(&issue_request(&parent_key, AudienceDid::from(child_issuer), ProofCollection::empty()))
                .expect("parent should issue");
        let child = issue_token(&child_request(&child_key, parent)).expect("child should issue");
        let keys = KeyResolutionContext::new(
            [&parent_key, &child_key]
                .into_iter()
                .map(Ed25519VerificationKey::from_signing_key)
                .collect::<Result<Vec<_>, _>>()
                .expect("keys should derive"),
        );
        let limits = VerificationLimits::new(1, 1, child.as_str().len() + 10);
        let error = super::verify_compact_token_with_resolvers(
            &child,
            VerificationTime::from_unix_seconds(VERIFICATION_TIME_SECONDS),
            &keys,
            &OversizedProofStore,
            limits,
        )
        .expect_err("oversized resolved proof should fail before parse");

        assert!(matches!(error, TokenError::LimitExceeded {
            source: VerificationLimitError::CompactTokenTooLarge { .. }
        }));
    }

    #[test]
    fn substituting_proof_store_is_rejected_as_reference_mismatch() {
        let parent_key = signing_key(ISSUER_SECRET_BYTE);
        let child_key = signing_key(CHILD_SECRET_BYTE);
        let child_issuer = child_key.issuer().expect("child issuer should derive");
        let parent =
            issue_token(&issue_request(&parent_key, AudienceDid::from(child_issuer), ProofCollection::empty()))
                .expect("parent should issue");
        let child = issue_token(&child_request(&child_key, parent.clone())).expect("child should issue");
        let keys = KeyResolutionContext::new(
            [&parent_key, &child_key]
                .into_iter()
                .map(Ed25519VerificationKey::from_signing_key)
                .collect::<Result<Vec<_>, _>>()
                .expect("keys should derive"),
        );
        let store = SubstitutingProofStore { token: child.clone() };
        let error = super::verify_compact_token_with_resolvers(
            &child,
            VerificationTime::from_unix_seconds(VERIFICATION_TIME_SECONDS),
            &keys,
            &store,
            VerificationLimits::default(),
        )
        .expect_err("a substituted proof token must fail the reference binding");

        match error {
            TokenError::ProofStore {
                source: ProofStoreError::ReferenceMismatch { expected, actual },
            } => {
                assert_eq!(expected.as_bytes(), proof_reference(&parent).as_bytes());
                assert_eq!(actual.as_bytes(), proof_reference(&child).as_bytes());
            }
            other => panic!("substitution must fail with ReferenceMismatch, got {other:?}"),
        }
    }

    #[test]
    fn reference_respecting_store_verifies_child_chain() {
        let parent_key = signing_key(ISSUER_SECRET_BYTE);
        let child_key = signing_key(CHILD_SECRET_BYTE);
        let child_issuer = child_key.issuer().expect("child issuer should derive");
        let parent =
            issue_token(&issue_request(&parent_key, AudienceDid::from(child_issuer), ProofCollection::empty()))
                .expect("parent should issue");
        let child = issue_token(&child_request(&child_key, parent.clone())).expect("child should issue");
        let keys = KeyResolutionContext::new(
            [&parent_key, &child_key]
                .into_iter()
                .map(Ed25519VerificationKey::from_signing_key)
                .collect::<Result<Vec<_>, _>>()
                .expect("keys should derive"),
        );
        let store = ReferenceRespectingProofStore { token: parent };
        let verified = super::verify_compact_token_with_resolvers(
            &child,
            VerificationTime::from_unix_seconds(VERIFICATION_TIME_SECONDS),
            &keys,
            &store,
            VerificationLimits::default(),
        )
        .expect("a correctly resolved proof must verify");

        assert_eq!(verified.issuer().as_str(), child_key.issuer().expect("child issuer should derive").as_str());
    }

    #[test]
    fn reject_cycle_contains_repeated_proof_reference() {
        let issuer_key = signing_key(ISSUER_SECRET_BYTE);
        let audience = signing_key(AUDIENCE_SECRET_BYTE).issuer().expect("audience should derive");
        let token = issue_token(&issue_request(&issuer_key, AudienceDid::from(audience), ProofCollection::empty()))
            .expect("token should issue");
        let reference = proof_reference(&token);
        let token_text = token.as_str();

        assert!(super::reject_cycle(token_text, &[reference]).is_err());
        assert!(super::reject_cycle(token_text, &[]).is_ok());
    }

    const BACKEND_FAILURE_MESSAGE: &str = "backend unavailable";

    struct BackendFailingResolver;

    impl KeyResolver for BackendFailingResolver {
        fn resolve_key(&self, issuer: &IssuerDid) -> std::result::Result<Ed25519PublicKey, KeyResolverError> {
            Err(KeyResolverError::Backend {
                issuer: issuer.as_str().to_owned(),
                message: BACKEND_FAILURE_MESSAGE.to_owned(),
            })
        }
    }

    struct BackendFailingProofStore;

    impl ProofStore for BackendFailingProofStore {
        fn resolve_proof<'a>(
            &'a self,
            reference: &ProofReference,
        ) -> std::result::Result<ProofToken<'a>, ProofStoreError> {
            Err(ProofStoreError::Backend {
                reference: reference.clone(),
                message: BACKEND_FAILURE_MESSAGE.to_owned(),
            })
        }
    }

    #[derive(Default)]

    struct BackendFailingRevocationChecker;

    impl RevocationChecker for BackendFailingRevocationChecker {
        fn is_revoked(&self, reference: &ProofReference) -> std::result::Result<bool, RevocationError> {
            Err(RevocationError::Backend {
                reference: reference.clone(),
                message: BACKEND_FAILURE_MESSAGE.to_owned(),
            })
        }
    }

    struct CountingProofStore;

    impl ProofStore for CountingProofStore {
        fn resolve_proof<'a>(
            &'a self,
            reference: &ProofReference,
        ) -> std::result::Result<ProofToken<'a>, ProofStoreError> {
            panic!("proof store should not be called after proof-count limit for {reference}");
        }
    }

    struct OversizedProofStore;

    impl ProofStore for OversizedProofStore {
        fn resolve_proof<'a>(
            &'a self,
            _reference: &ProofReference,
        ) -> std::result::Result<ProofToken<'a>, ProofStoreError> {
            let oversized = "x".repeat(8192);
            CompactToken::new(oversized.as_str())
                .map(ProofToken::Owned)
                .map_err(|error| ProofStoreError::Backend {
                    reference: ProofReference::new(Vec::new()),
                    message: error.to_string(),
                })
        }
    }

    struct AllowPolicy;

    impl CaveatPolicy for AllowPolicy {
        fn evaluate(&self, _caveat: &CaveatDocument, _request: &AuthorizationRequest) -> CaveatDecision {
            CaveatDecision::Satisfied
        }
    }

    struct RejectPolicy;

    impl CaveatPolicy for RejectPolicy {
        fn evaluate(&self, _caveat: &CaveatDocument, _request: &AuthorizationRequest) -> CaveatDecision {
            CaveatDecision::Rejected {
                message: "rejected by test policy".to_owned(),
            }
        }
    }

    struct SinglePolicySet<P> {
        identifier: CaveatIdentifier,
        policy: P,
    }

    impl<P> SinglePolicySet<P> {
        fn new(domain: &str, caveat_type: &str, key: &str, policy: P) -> Self {
            Self {
                identifier: CaveatIdentifier::new(domain, caveat_type, key).expect("identifier should validate"),
                policy,
            }
        }
    }

    impl<P: CaveatPolicy> CaveatPolicySet for SinglePolicySet<P> {
        fn policy_for(&self, caveat: &CaveatIdentifier) -> Option<&dyn CaveatPolicy> {
            if caveat == &self.identifier {
                Some(&self.policy)
            } else {
                None
            }
        }
    }

    struct SubstitutingProofStore {
        token: CompactToken,
    }

    impl ProofStore for SubstitutingProofStore {
        fn resolve_proof<'a>(
            &'a self,
            _reference: &ProofReference,
        ) -> std::result::Result<ProofToken<'a>, ProofStoreError> {
            Ok(ProofToken::Borrowed(&self.token))
        }
    }

    struct ReferenceRespectingProofStore {
        token: CompactToken,
    }

    impl ProofStore for ReferenceRespectingProofStore {
        fn resolve_proof<'a>(
            &'a self,
            reference: &ProofReference,
        ) -> std::result::Result<ProofToken<'a>, ProofStoreError> {
            if proof_reference(&self.token).as_bytes() == reference.as_bytes() {
                Ok(ProofToken::Borrowed(&self.token))
            } else {
                Err(ProofStoreError::MissingProof {
                    reference: reference.clone(),
                })
            }
        }
    }

    #[derive(Clone)]
    struct RefusingSigner {
        issuer: IssuerDid,
        public_key: Ed25519PublicKey,
    }

    impl RefusingSigner {
        fn new(signing_key: &Ed25519SigningKey) -> Self {
            Self {
                issuer: signing_key.issuer().expect("issuer should derive"),
                public_key: signing_key.public_key().expect("public key should derive"),
            }
        }
    }

    impl TokenSigner for RefusingSigner {
        fn algorithm(&self) -> &str {
            super::TOKEN_ALGORITHM_EDDSA
        }

        fn issuer(&self) -> TokenResult<IssuerDid> {
            Ok(self.issuer.clone())
        }

        fn public_key(&self) -> TokenResult<Ed25519PublicKey> {
            Ok(self.public_key.clone())
        }

        fn sign(&self, _signing_input: &[u8]) -> SignerResult<SignerSignature> {
            Err(SignerError::Refused)
        }
    }

    #[derive(Clone)]
    struct UnsupportedAlgorithmSigner {
        issuer: IssuerDid,
        public_key: Ed25519PublicKey,
    }

    impl UnsupportedAlgorithmSigner {
        fn new(signing_key: &Ed25519SigningKey) -> Self {
            Self {
                issuer: signing_key.issuer().expect("issuer should derive"),
                public_key: signing_key.public_key().expect("public key should derive"),
            }
        }
    }

    impl TokenSigner for UnsupportedAlgorithmSigner {
        fn algorithm(&self) -> &str {
            UNSUPPORTED_TEST_ALGORITHM
        }

        fn issuer(&self) -> TokenResult<IssuerDid> {
            Ok(self.issuer.clone())
        }

        fn public_key(&self) -> TokenResult<Ed25519PublicKey> {
            Ok(self.public_key.clone())
        }

        fn sign(&self, _signing_input: &[u8]) -> SignerResult<SignerSignature> {
            Ok(SignerSignature::from_bytes([0_u8; super::ED25519_SIGNATURE_BYTES]))
        }
    }

    struct FixedSignatureSigner {
        issuer: IssuerDid,
        public_key: Ed25519PublicKey,
        signature: [u8; super::ED25519_SIGNATURE_BYTES],
    }

    impl FixedSignatureSigner {
        fn new(signing_key: &Ed25519SigningKey, signature: [u8; super::ED25519_SIGNATURE_BYTES]) -> Self {
            Self {
                issuer: signing_key.issuer().expect("issuer should derive"),
                public_key: signing_key.public_key().expect("public key should derive"),
                signature,
            }
        }
    }

    impl TokenSigner for FixedSignatureSigner {
        fn algorithm(&self) -> &str {
            super::TOKEN_ALGORITHM_EDDSA
        }

        fn issuer(&self) -> TokenResult<IssuerDid> {
            Ok(self.issuer.clone())
        }

        fn public_key(&self) -> TokenResult<Ed25519PublicKey> {
            Ok(self.public_key.clone())
        }

        fn sign(&self, _signing_input: &[u8]) -> SignerResult<SignerSignature> {
            Ok(SignerSignature::from_bytes(self.signature))
        }
    }

    #[derive(Clone)]
    struct MismatchedIssuerSigner {
        issuer: IssuerDid,
        public_key: Ed25519PublicKey,
    }

    impl MismatchedIssuerSigner {
        fn new(signing_key: &Ed25519SigningKey, issuer: IssuerDid) -> Self {
            Self {
                issuer,
                public_key: signing_key.public_key().expect("public key should derive"),
            }
        }
    }

    impl TokenSigner for MismatchedIssuerSigner {
        fn algorithm(&self) -> &str {
            super::TOKEN_ALGORITHM_EDDSA
        }

        fn issuer(&self) -> TokenResult<IssuerDid> {
            Ok(self.issuer.clone())
        }

        fn public_key(&self) -> TokenResult<Ed25519PublicKey> {
            Ok(self.public_key.clone())
        }

        fn sign(&self, _signing_input: &[u8]) -> SignerResult<SignerSignature> {
            Ok(SignerSignature::from_bytes([0_u8; super::ED25519_SIGNATURE_BYTES]))
        }
    }

    fn assert_payload_failure(payload: &str, expected: crate::verified::TokenValidationFailure) {
        let token = token_with_payload(payload);
        let decoded = parse_compact_token(&token).expect("payload should decode before structural validation");
        assert!(matches!(
            validate_decoded_token(decoded),
            Err(TokenError::VerifiedPrimitive {
                source: crate::verified::VerifiedPrimitiveError::TokenValidation { failure }
            }) if failure == expected
        ));
    }

    fn signing_key(byte: u8) -> Ed25519SigningKey {
        Ed25519SigningKey::from_bytes([byte; super::ED25519_SECRET_KEY_BYTES])
    }

    fn in_memory_signer(byte: u8) -> Ed25519InMemorySigner {
        Ed25519InMemorySigner::from_seed_bytes([byte; super::ED25519_SECRET_KEY_BYTES])
    }

    fn token_time_bounds() -> TokenTimeBounds {
        TokenTimeBounds::new(TOKEN_NOT_BEFORE_SECONDS, TOKEN_EXPIRES_AT_SECONDS).expect("bounds should validate")
    }

    fn issue_request(issuer_key: &Ed25519SigningKey, audience: AudienceDid, proofs: ProofCollection) -> IssueRequest {
        issue_request_with_capability(issuer_key, audience, proofs, "did:key:z6Mkresource", "store/*")
    }

    fn issue_request_with_capability(
        issuer_key: &Ed25519SigningKey,
        audience: AudienceDid,
        proofs: ProofCollection,
        resource: &str,
        ability: &str,
    ) -> IssueRequest {
        IssueRequest::new(
            issuer_key.clone(),
            audience,
            capabilities(resource, ability),
            proofs.into(),
            TokenTimeBounds::new(TOKEN_NOT_BEFORE_SECONDS, TOKEN_EXPIRES_AT_SECONDS).expect("bounds should validate"),
        )
    }

    fn issue_request_with_caveat(
        issuer_key: &Ed25519SigningKey,
        audience: AudienceDid,
        proofs: ProofCollection,
        caveat: CaveatDocument,
    ) -> IssueRequest {
        IssueRequest::new(
            issuer_key.clone(),
            audience,
            CapabilitySet::new(vec![
                CapabilityDocument::with_caveats("did:key:z6Mkresource".to_owned(), "store/*".to_owned(), vec![caveat])
                    .expect("capability with caveat should validate"),
            ])
            .expect("capability set should validate"),
            proofs.into(),
            TokenTimeBounds::new(TOKEN_NOT_BEFORE_SECONDS, TOKEN_EXPIRES_AT_SECONDS).expect("bounds should validate"),
        )
    }

    fn caveat(domain: &str, caveat_type: &str, key: &str, payload: &[u8]) -> CaveatDocument {
        CaveatDocument::new(domain.to_owned(), caveat_type.to_owned(), key.to_owned(), payload.to_vec())
            .expect("caveat should validate")
    }

    fn child_request(issuer_key: &Ed25519SigningKey, parent: CompactToken) -> IssueRequest {
        child_request_with_proofs(issuer_key, vec![parent])
    }

    fn child_request_with_proofs(issuer_key: &Ed25519SigningKey, parents: Vec<CompactToken>) -> IssueRequest {
        let audience = signing_key(OTHER_SECRET_BYTE).issuer().expect("audience should derive");
        IssueRequest::new(
            issuer_key.clone(),
            AudienceDid::from(audience),
            capabilities("did:key:z6Mkresource/photos", "store/add"),
            ProofCollection::from_tokens(parents).into(),
            TokenTimeBounds::new(CHILD_NOT_BEFORE_SECONDS, CHILD_EXPIRES_AT_SECONDS).expect("bounds should validate"),
        )
    }

    fn child_request_with_caveat(
        issuer_key: &Ed25519SigningKey,
        parent: CompactToken,
        caveat: CaveatDocument,
    ) -> IssueRequest {
        let audience = signing_key(OTHER_SECRET_BYTE).issuer().expect("audience should derive");
        IssueRequest::new(
            issuer_key.clone(),
            AudienceDid::from(audience),
            CapabilitySet::new(vec![
                CapabilityDocument::with_caveats(
                    "did:key:z6Mkresource/photos".to_owned(),
                    "store/add".to_owned(),
                    vec![caveat],
                )
                .expect("capability with caveat should validate"),
            ])
            .expect("capability set should validate"),
            ProofCollection::from_tokens(vec![parent]).into(),
            TokenTimeBounds::new(CHILD_NOT_BEFORE_SECONDS, CHILD_EXPIRES_AT_SECONDS).expect("bounds should validate"),
        )
    }

    fn widened_child_request(issuer_key: &Ed25519SigningKey, parent: CompactToken) -> IssueRequest {
        const WIDENED_CHILD_EXPIRES_AT_SECONDS: u64 = TOKEN_EXPIRES_AT_SECONDS + 1;
        let audience = signing_key(OTHER_SECRET_BYTE).issuer().expect("audience should derive");
        IssueRequest::new(
            issuer_key.clone(),
            AudienceDid::from(audience),
            capabilities("did:key:z6Mkresource/photos", "store/add"),
            ProofCollection::from_tokens(vec![parent]).into(),
            TokenTimeBounds::new(CHILD_NOT_BEFORE_SECONDS, WIDENED_CHILD_EXPIRES_AT_SECONDS)
                .expect("bounds should validate"),
        )
    }

    fn over_broad_child_request(issuer_key: &Ed25519SigningKey, parent: CompactToken) -> IssueRequest {
        let audience = signing_key(OTHER_SECRET_BYTE).issuer().expect("audience should derive");
        IssueRequest::new(
            issuer_key.clone(),
            AudienceDid::from(audience),
            capabilities("did:key:z6Mkresource/photos", "admin/delete"),
            ProofCollection::from_tokens(vec![parent]).into(),
            TokenTimeBounds::new(CHILD_NOT_BEFORE_SECONDS, CHILD_EXPIRES_AT_SECONDS).expect("bounds should validate"),
        )
    }

    fn capabilities(resource: &str, ability: &str) -> CapabilitySet {
        CapabilitySet::new(vec![
            CapabilityDocument::new(resource.to_owned(), ability.to_owned()).expect("capability should validate"),
        ])
        .expect("capability set should validate")
    }

    fn verified_child_token() -> VerifiedToken {
        let parent_key = signing_key(ISSUER_SECRET_BYTE);
        let child_key = signing_key(CHILD_SECRET_BYTE);
        let child_issuer = child_key.issuer().expect("child issuer should derive");
        let parent =
            issue_token(&issue_request(&parent_key, AudienceDid::from(child_issuer), ProofCollection::empty()))
                .expect("parent should issue");
        let child = issue_token(&child_request(&child_key, parent.clone())).expect("child should issue");
        let context = verification_context(
            vec![&parent_key, &child_key],
            ProofCollection::from_tokens(vec![parent]),
            VERIFICATION_TIME_SECONDS,
        );
        verify_compact_token(&child, &context).expect("delegation chain should verify")
    }

    fn assert_allowed_capability(decision: &AuthorizationDecision, resource: &str, ability: &str) {
        assert!(decision.is_allowed());
        assert_eq!(
            decision.matched_capability().expect("match should be exposed"),
            &CapabilityDocument::new(resource.to_owned(), ability.to_owned()).expect("capability should validate")
        );
    }

    fn assert_malformed_denial(decision: &AuthorizationDecision) {
        assert!(matches!(decision, AuthorizationDecision::Denied {
            reason: AuthorizationDenialReason::MalformedRequest { .. }
        }));
    }

    fn verification_context(
        signing_keys: Vec<&Ed25519SigningKey>,
        proofs: ProofCollection,
        time_seconds: u64,
    ) -> VerificationContext {
        let keys = signing_keys
            .into_iter()
            .map(Ed25519VerificationKey::from_signing_key)
            .collect::<Result<Vec<_>, _>>()
            .expect("verification keys should derive");
        VerificationContext::new(
            VerificationTime::from_unix_seconds(time_seconds),
            KeyResolutionContext::new(keys),
            proofs,
        )
    }

    fn tamper_signature(token: &CompactToken) -> CompactToken {
        const FIRST_SIGNATURE_CHAR_INDEX: usize = 0;
        const REPLACEMENT_A: u8 = b'A';
        const REPLACEMENT_B: u8 = b'B';
        let parts: Vec<&str> = token.as_str().split('.').collect();
        let mut signature = parts[2].as_bytes().to_vec();
        let original = signature[FIRST_SIGNATURE_CHAR_INDEX];
        signature[FIRST_SIGNATURE_CHAR_INDEX] = if original == REPLACEMENT_A {
            REPLACEMENT_B
        } else {
            REPLACEMENT_A
        };
        let signature = String::from_utf8(signature).expect("base64url signature should remain utf-8");
        test_compact_token(parts[0], parts[1], signature.as_str())
    }

    fn token_with_header(header_json: &str, payload_json: &str) -> CompactToken {
        let header = super::URL_SAFE_NO_PAD.encode(header_json.as_bytes());
        let payload = super::URL_SAFE_NO_PAD.encode(payload_json.as_bytes());
        let signature = super::URL_SAFE_NO_PAD.encode([0_u8; super::ED25519_SIGNATURE_BYTES]);
        test_compact_token(&header, &payload, &signature)
    }

    fn token_with_payload(payload_json: &str) -> CompactToken {
        let header = super::EncodedHeader {
            alg: super::TOKEN_ALGORITHM_EDDSA,
            typ: super::TOKEN_TYPE_JWT,
        };
        let header_json = serde_json::to_vec(&header).expect("header should encode");
        let header = super::URL_SAFE_NO_PAD.encode(header_json);
        let payload = super::URL_SAFE_NO_PAD.encode(payload_json.as_bytes());
        let signature = super::URL_SAFE_NO_PAD.encode([0_u8; super::ED25519_SIGNATURE_BYTES]);
        test_compact_token(&header, &payload, &signature)
    }

    fn test_compact_token(header: &str, payload: &str, signature: &str) -> CompactToken {
        let text = format!("{header}.{payload}.{signature}");
        CompactToken::new(text.as_str()).expect("test compact token should wrap")
    }

    fn assert_token_shape_and_claim_names(token: &CompactToken) {
        let parts: Vec<&str> = token.as_str().split('.').collect();

        assert_eq!(parts.len(), COMPACT_PART_COUNT);
        let payload = super::decode_part(parts[1], super::CompactPart::Payload).expect("payload should decode");
        let json: Value = serde_json::from_slice(payload.as_slice()).expect("payload should be JSON");
        let object = json.as_object().expect("payload should be object");
        assert_eq!(object.len(), EXPECTED_CLAIM_COUNT);
        for key in ["iss", "aud", "att", "prf", "exp", "nbf"] {
            assert!(object.contains_key(key), "missing claim key {key}");
        }
    }

    fn assert_secret_material_is_absent(output: &str) {
        assert!(!output.contains(REDACTION_TEST_SECRET_DECIMAL));
        assert!(!output.contains(REDACTION_TEST_DIRECT_ARRAY_PREFIX));
        assert!(!output.to_ascii_lowercase().contains(REDACTION_TEST_SECRET_HEX_LOWER));
    }

    fn capture_assertion_failure_message<F>(body: F) -> String
    where F: FnOnce() + std::panic::UnwindSafe {
        let panic = std::panic::catch_unwind(body).expect_err("assertion should fail");
        if let Some(message) = panic.downcast_ref::<String>() {
            return message.clone();
        }
        if let Some(message) = panic.downcast_ref::<&'static str>() {
            return (*message).to_owned();
        }
        panic!("assertion failure should use string panic payload");
    }
}
