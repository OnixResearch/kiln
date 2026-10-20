use std::fmt;

use serde::Deserialize;
use serde::Serialize;

pub const PUBLIC_KEY_BYTES: usize = 32;
pub const EVENT_ID_BYTES: usize = 32;
pub const SIGNATURE_BYTES: usize = 64;
pub const HEX_CHARS_PER_BYTE: usize = 2;
pub const NIBBLE_BITS: u32 = 4;
pub const LOW_NIBBLE_MASK: u8 = 0x0f;
const DECIMAL_DIGIT_COUNT: u8 = 10;
const HEX_RADIX: usize = 16;

/// Largest accepted encoded Nostr event.
pub const MAX_EVENT_INPUT_BYTES: usize = 65_536;
/// Largest accepted canonical NIP-01 event serialization.
pub const MAX_CANONICAL_EVENT_BYTES: usize = 65_536;
/// Largest accepted event content.
pub const MAX_CONTENT_BYTES: usize = 32_768;
/// Largest accepted number of event tags.
pub const MAX_TAGS: usize = 64;
/// Largest accepted number of values in one tag.
pub const MAX_TAG_VALUES: usize = 16;
/// Largest accepted UTF-8 byte length of one tag value.
pub const MAX_TAG_VALUE_BYTES: usize = 4_096;

const _: () = assert!(MAX_EVENT_INPUT_BYTES > 0);
const _: () = assert!(MAX_CANONICAL_EVENT_BYTES > 0);
const _: () = assert!(MAX_CONTENT_BYTES > 0);
const _: () = assert!(MAX_TAGS > 0);
const _: () = assert!(MAX_TAG_VALUES > 0);
const _: () = assert!(MAX_TAG_VALUE_BYTES > 0);

/// Stable verification issue returned by the adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerificationIssue {
    /// Encoded event exceeded the fixed input limit.
    EventInputTooLarge,
    /// Event JSON was malformed, open-ended, or structurally unsupported.
    MalformedEventJson,
    /// Event content or tags exceeded fixed bounds.
    EventBoundsExceeded,
    /// Public key was not canonical 32-byte lowercase hexadecimal.
    InvalidPublicKeyEncoding,
    /// Public key did not identify a valid x-only secp256k1 point.
    InvalidPublicKeyPoint,
    /// Event identity was not canonical 32-byte lowercase hexadecimal.
    InvalidEventIdEncoding,
    /// Signature was not canonical 64-byte lowercase hexadecimal.
    InvalidSignatureEncoding,
    /// Recomputed NIP-01 SHA-256 identity differed from the supplied event ID.
    EventIdMismatch,
    /// BIP-340 signature verification failed.
    SignatureInvalid,
    /// A typed digest used the wrong algorithm role.
    DigestRoleMismatch,
    /// Selected authentication profile required another event kind.
    ProfileKindMismatch,
    /// Required event content shape did not match the profile.
    ProfileContentMismatch,
    /// Required authentication tag was absent.
    MissingTag,
    /// Authentication tag appeared more than once.
    DuplicateTag,
    /// Authentication tag value differed from caller expectations.
    TagMismatch,
    /// Caller-supplied audience did not match the signed profile context.
    AudienceMismatch,
    /// Caller-supplied purpose did not match the signed profile context.
    PurposeMismatch,
    /// URL, method, or another profile context value was malformed.
    InvalidAuthenticationContext,
    /// Freshness policy was zero or arithmetically invalid.
    InvalidFreshnessPolicy,
    /// Event was older than the caller-selected age.
    EventStale,
    /// Event was newer than the caller-selected future skew.
    EventFromFuture,
    /// Caller reported that the event was replayed or could not be admitted.
    ReplayDenied,
    /// Input attempted subject, authority, presentation, or secret promotion.
    BoundaryPromotion,
}

impl VerificationIssue {
    /// Return the stable machine issue code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::EventInputTooLarge => "nostr-event-input-too-large",
            Self::MalformedEventJson => "nostr-event-json-malformed",
            Self::EventBoundsExceeded => "nostr-event-bounds-exceeded",
            Self::InvalidPublicKeyEncoding => "nostr-public-key-encoding-invalid",
            Self::InvalidPublicKeyPoint => "nostr-public-key-point-invalid",
            Self::InvalidEventIdEncoding => "nostr-event-id-encoding-invalid",
            Self::InvalidSignatureEncoding => "nostr-signature-encoding-invalid",
            Self::EventIdMismatch => "nostr-event-id-mismatch",
            Self::SignatureInvalid => "nostr-signature-invalid",
            Self::DigestRoleMismatch => "nostr-digest-role-mismatch",
            Self::ProfileKindMismatch => "nostr-profile-kind-mismatch",
            Self::ProfileContentMismatch => "nostr-profile-content-mismatch",
            Self::MissingTag => "nostr-auth-tag-missing",
            Self::DuplicateTag => "nostr-auth-tag-duplicate",
            Self::TagMismatch => "nostr-auth-tag-mismatch",
            Self::AudienceMismatch => "nostr-auth-audience-mismatch",
            Self::PurposeMismatch => "nostr-auth-purpose-mismatch",
            Self::InvalidAuthenticationContext => "nostr-auth-context-invalid",
            Self::InvalidFreshnessPolicy => "nostr-freshness-policy-invalid",
            Self::EventStale => "nostr-event-stale",
            Self::EventFromFuture => "nostr-event-from-future",
            Self::ReplayDenied => "nostr-replay-denied",
            Self::BoundaryPromotion => "nostr-boundary-promotion",
        }
    }
}

/// Closed NIP-01 event shape accepted by the adapter.
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NostrEvent {
    /// Lowercase SHA-256 event identity.
    pub id: String,
    /// Lowercase x-only secp256k1 public key.
    pub pubkey: String,
    /// Source-supplied Unix timestamp in seconds.
    pub created_at: u64,
    /// Nostr event kind.
    pub kind: u32,
    /// Closed ordered tag arrays.
    pub tags: Vec<Vec<String>>,
    /// Exact event content.
    pub content: String,
    /// Lowercase BIP-340 signature.
    pub sig: String,
}

impl fmt::Debug for NostrEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NostrEvent")
            .field("id", &self.id)
            .field("pubkey", &self.pubkey)
            .field("created_at", &self.created_at)
            .field("kind", &self.kind)
            .field("tag_count", &self.tags.len())
            .field("content_bytes", &self.content.len())
            .field("sig", &"<redacted-public-signature>")
            .finish()
    }
}

/// Canonical raw x-only secp256k1 public key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NostrPublicKey([u8; PUBLIC_KEY_BYTES]);

impl NostrPublicKey {
    /// Parse canonical lowercase hexadecimal key text.
    ///
    /// # Errors
    ///
    /// Returns a deterministic encoding issue for malformed, uppercase, mixed,
    /// `npub`, or wrong-length text.
    pub fn parse(text: &str) -> Result<Self, VerificationIssue> {
        let bytes =
            decode_lower_hex::<PUBLIC_KEY_BYTES>(text).map_err(|_issue| VerificationIssue::InvalidPublicKeyEncoding)?;
        k256::schnorr::VerifyingKey::from_bytes(&bytes).map_err(|_error| VerificationIssue::InvalidPublicKeyPoint)?;
        Ok(Self(bytes))
    }

    /// Return raw x-only key bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; PUBLIC_KEY_BYTES] {
        &self.0
    }

    /// Return canonical lowercase hexadecimal.
    #[must_use]
    pub fn to_hex(self) -> String {
        encode_lower_hex(&self.0)
    }
}

/// NIP-01 SHA-256 event identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NostrEventId([u8; EVENT_ID_BYTES]);

impl NostrEventId {
    /// Parse canonical lowercase hexadecimal event identity text.
    ///
    /// # Errors
    ///
    /// Returns a deterministic encoding issue for malformed input.
    pub fn parse(text: &str) -> Result<Self, VerificationIssue> {
        decode_lower_hex::<EVENT_ID_BYTES>(text)
            .map(Self)
            .map_err(|_issue| VerificationIssue::InvalidEventIdEncoding)
    }

    /// Construct from a protocol SHA-256 digest.
    #[must_use]
    pub(crate) const fn from_sha256(bytes: [u8; EVENT_ID_BYTES]) -> Self {
        Self(bytes)
    }

    /// Return protocol digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; EVENT_ID_BYTES] {
        &self.0
    }

    /// Return canonical lowercase hexadecimal.
    #[must_use]
    pub fn to_hex(self) -> String {
        encode_lower_hex(&self.0)
    }
}

/// Adapter-owned BLAKE3 identity or request reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterBlake3([u8; EVENT_ID_BYTES]);

impl AdapterBlake3 {
    /// Construct from BLAKE3 output bytes.
    #[must_use]
    pub(crate) const fn new(bytes: [u8; EVENT_ID_BYTES]) -> Self {
        Self(bytes)
    }

    /// Return BLAKE3 bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; EVENT_ID_BYTES] {
        &self.0
    }

    /// Return canonical lowercase hexadecimal.
    #[must_use]
    pub fn to_hex(self) -> String {
        encode_lower_hex(&self.0)
    }
}

/// Algorithm role carried by a structural digest field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DigestRole {
    /// NIP-01 protocol SHA-256 event identity.
    NostrEventSha256,
    /// Adapter-owned BLAKE3 identity.
    AdapterBlake3,
}

/// Structurally decoded digest with an explicit algorithm role.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WireDigest {
    /// Declared digest algorithm role.
    pub role: DigestRole,
    /// Canonical lowercase hexadecimal digest.
    pub hex: String,
}

/// Check a structural digest role and canonical encoding.
///
/// # Errors
///
/// Returns [`VerificationIssue::DigestRoleMismatch`] if the role differs and
/// an encoding issue if the digest text is malformed.
pub fn validate_wire_digest(
    digest: &WireDigest,
    expected_role: DigestRole,
) -> Result<[u8; EVENT_ID_BYTES], VerificationIssue> {
    if digest.role != expected_role {
        return Err(VerificationIssue::DigestRoleMismatch);
    }
    decode_lower_hex::<EVENT_ID_BYTES>(&digest.hex)
}

/// Caller-declared boundary fact that must remain outside verified output.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BoundaryClaims {
    /// No metadata, secret, subject, or authority promotion is requested.
    #[default]
    None,
    /// NIP-05 metadata is presented as identity proof.
    PromotesNip05Identity,
    /// `npub` presentation text is presented as the canonical key.
    PromotesNpubKey,
    /// Input contains private keys, NIP-46 secrets, credentials, or bodies.
    ContainsSecretMaterial,
    /// Caller asks the adapter to resolve a human or product subject.
    RequestsSubjectResolution,
    /// Caller asks the adapter to grant authorization.
    RequestsAuthorization,
}

pub const fn validate_boundary(boundary: BoundaryClaims) -> Result<(), VerificationIssue> {
    if matches!(boundary, BoundaryClaims::None) {
        Ok(())
    } else {
        Err(VerificationIssue::BoundaryPromotion)
    }
}

pub fn decode_signature(text: &str) -> Result<[u8; SIGNATURE_BYTES], VerificationIssue> {
    decode_lower_hex::<SIGNATURE_BYTES>(text).map_err(|_issue| VerificationIssue::InvalidSignatureEncoding)
}

pub fn decode_lower_hex<const OUTPUT_BYTES: usize>(text: &str) -> Result<[u8; OUTPUT_BYTES], VerificationIssue> {
    let expected_chars = OUTPUT_BYTES.checked_mul(HEX_CHARS_PER_BYTE).ok_or(VerificationIssue::EventBoundsExceeded)?;
    if text.len() != expected_chars || !text.is_ascii() {
        return Err(VerificationIssue::InvalidEventIdEncoding);
    }
    let bytes = text.as_bytes();
    let mut output = [0_u8; OUTPUT_BYTES];
    for (index, output_byte) in output.iter_mut().enumerate() {
        let high_index = index.checked_mul(HEX_CHARS_PER_BYTE).ok_or(VerificationIssue::EventBoundsExceeded)?;
        let low_index = high_index.checked_add(1).ok_or(VerificationIssue::EventBoundsExceeded)?;
        let high = decode_lower_nibble(bytes[high_index])?;
        let low = decode_lower_nibble(bytes[low_index])?;
        *output_byte = (high << NIBBLE_BITS) | low;
    }
    Ok(output)
}

const fn decode_lower_nibble(byte: u8) -> Result<u8, VerificationIssue> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + DECIMAL_DIGIT_COUNT),
        _unsupported => Err(VerificationIssue::InvalidEventIdEncoding),
    }
}

pub fn encode_lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; HEX_RADIX] = b"0123456789abcdef";
    let output_capacity = bytes.len().saturating_mul(HEX_CHARS_PER_BYTE);
    let mut output = String::with_capacity(output_capacity);
    for byte in bytes {
        let high = usize::from(*byte >> NIBBLE_BITS);
        let low = usize::from(*byte & LOW_NIBBLE_MASK);
        output.push(char::from(HEX[high]));
        output.push(char::from(HEX[low]));
    }
    output
}
