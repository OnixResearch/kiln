use k256::schnorr::Signature;
use k256::schnorr::VerifyingKey;
use sha2::Digest;
use sha2::Sha256;

use crate::model::EVENT_ID_BYTES;
use crate::model::MAX_CANONICAL_EVENT_BYTES;
use crate::model::MAX_CONTENT_BYTES;
use crate::model::MAX_EVENT_INPUT_BYTES;
use crate::model::MAX_TAG_VALUE_BYTES;
use crate::model::MAX_TAG_VALUES;
use crate::model::MAX_TAGS;
use crate::model::NostrEvent;
use crate::model::NostrEventId;
use crate::model::NostrPublicKey;
use crate::model::VerificationIssue;
use crate::model::decode_signature;

/// Protocol-verified NIP-01 event facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedNostrEvent {
    pub(crate) public_key: NostrPublicKey,
    pub(crate) event_id: NostrEventId,
    pub(crate) created_at: u64,
    pub(crate) kind: u32,
    pub(crate) tags: Vec<Vec<String>>,
    pub(crate) content: String,
}

impl VerifiedNostrEvent {
    /// Return the canonical raw public key.
    #[must_use]
    pub const fn public_key(&self) -> NostrPublicKey {
        self.public_key
    }

    /// Return the NIP-01 SHA-256 event identity.
    #[must_use]
    pub const fn event_id(&self) -> NostrEventId {
        self.event_id
    }

    /// Return the signed event creation time.
    #[must_use]
    pub const fn created_at(&self) -> u64 {
        self.created_at
    }

    /// Return the signed event kind.
    #[must_use]
    pub const fn kind(&self) -> u32 {
        self.kind
    }
}

/// Parse an already-loaded bounded JSON event.
///
/// # Errors
///
/// Returns a size issue before parsing or a structural issue for malformed,
/// open-ended, or unknown event fields.
// r[impl ucan.nostr_identity_adapter.package_boundary]
pub fn parse_event_json(bytes: &[u8]) -> Result<NostrEvent, VerificationIssue> {
    if bytes.len() > MAX_EVENT_INPUT_BYTES {
        return Err(VerificationIssue::EventInputTooLarge);
    }
    serde_json::from_slice(bytes).map_err(|_error| VerificationIssue::MalformedEventJson)
}

/// Recreate exact NIP-01 canonical serialization.
///
/// # Errors
///
/// Returns a deterministic bounds or encoding issue.
pub fn canonical_event_bytes(event: &NostrEvent) -> Result<Vec<u8>, VerificationIssue> {
    validate_event_bounds(event)?;
    NostrPublicKey::parse(&event.pubkey)?;
    let marker = 0_u8;
    let canonical =
        serde_json::to_vec(&(marker, &event.pubkey, event.created_at, event.kind, &event.tags, &event.content))
            .map_err(|_error| VerificationIssue::MalformedEventJson)?;
    if canonical.len() > MAX_CANONICAL_EVENT_BYTES {
        return Err(VerificationIssue::EventBoundsExceeded);
    }
    Ok(canonical)
}

/// Compute the NIP-01 SHA-256 event identity.
///
/// # Errors
///
/// Returns the same structural diagnostics as [`canonical_event_bytes`].
pub fn compute_event_id(event: &NostrEvent) -> Result<NostrEventId, VerificationIssue> {
    let canonical = canonical_event_bytes(event)?;
    let digest: [u8; EVENT_ID_BYTES] = Sha256::digest(&canonical).into();
    Ok(NostrEventId::from_sha256(digest))
}

/// Verify NIP-01 identity and BIP-340 signature rules.
///
/// # Errors
///
/// Returns a deterministic issue for malformed keys, IDs, signatures,
/// canonical serialization, identity mismatch, or signature failure.
// r[impl ucan.nostr_identity_adapter.protocol_verification]
pub fn verify_event(event: &NostrEvent) -> Result<VerifiedNostrEvent, VerificationIssue> {
    let public_key = NostrPublicKey::parse(&event.pubkey)?;
    let verifying_key =
        VerifyingKey::from_bytes(public_key.as_bytes()).map_err(|_error| VerificationIssue::InvalidPublicKeyPoint)?;
    let supplied_id = NostrEventId::parse(&event.id)?;
    let computed_id = compute_event_id(event)?;
    if supplied_id != computed_id {
        return Err(VerificationIssue::EventIdMismatch);
    }
    let signature_bytes = decode_signature(&event.sig)?;
    let signature = Signature::try_from(signature_bytes.as_slice())
        .map_err(|_error| VerificationIssue::InvalidSignatureEncoding)?;
    verifying_key
        .verify_raw(computed_id.as_bytes(), &signature)
        .map_err(|_error| VerificationIssue::SignatureInvalid)?;

    Ok(VerifiedNostrEvent {
        public_key,
        event_id: computed_id,
        created_at: event.created_at,
        kind: event.kind,
        tags: event.tags.clone(),
        content: event.content.clone(),
    })
}

/// Verify one raw BIP-340 message vector.
///
/// # Errors
///
/// Returns exact public-key or signature diagnostics.
pub fn verify_bip340_message(
    public_key_hex: &str,
    message: &[u8],
    signature_hex: &str,
) -> Result<(), VerificationIssue> {
    let public_key = NostrPublicKey::parse(public_key_hex)?;
    let verifying_key =
        VerifyingKey::from_bytes(public_key.as_bytes()).map_err(|_error| VerificationIssue::InvalidPublicKeyPoint)?;
    let signature_bytes = decode_signature(signature_hex)?;
    let signature = Signature::try_from(signature_bytes.as_slice())
        .map_err(|_error| VerificationIssue::InvalidSignatureEncoding)?;
    verifying_key.verify_raw(message, &signature).map_err(|_error| VerificationIssue::SignatureInvalid)
}

fn validate_event_bounds(event: &NostrEvent) -> Result<(), VerificationIssue> {
    if event.content.len() > MAX_CONTENT_BYTES || event.tags.len() > MAX_TAGS {
        return Err(VerificationIssue::EventBoundsExceeded);
    }
    for tag in &event.tags {
        if tag.is_empty() || tag.len() > MAX_TAG_VALUES {
            return Err(VerificationIssue::EventBoundsExceeded);
        }
        if tag.iter().any(|value| value.len() > MAX_TAG_VALUE_BYTES) {
            return Err(VerificationIssue::EventBoundsExceeded);
        }
    }
    Ok(())
}
