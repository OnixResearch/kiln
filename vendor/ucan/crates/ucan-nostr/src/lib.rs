//! Optional bounded Nostr key-control verification for UCAN consumers.
//!
//! The adapter verifies already-loaded NIP-01 events and explicit NIP-42,
//! NIP-98, or application challenge contexts. It does not access relays, DNS,
//! clocks, replay stores, signers, private keys, UCAN token paths, or authority.
//!
//! Nostr protocol event identities use SHA-256 because NIP-01 requires it.
//! Adapter profile identities and request references use BLAKE3.

#![forbid(unsafe_code)]
#![allow(clippy::cargo_common_metadata)]
#![allow(clippy::multiple_crate_versions)]

mod context;
mod model;
mod protocol;

pub use context::APPLICATION_PURPOSE_PROFILE;
pub use context::AuthenticationExpectation;
pub use context::AuthenticationProfile;
pub use context::FreshnessDisposition;
pub use context::FreshnessExpectation;
pub use context::MAX_CONTEXT_FIELD_BYTES;
pub use context::NIP42_EVENT_KIND;
pub use context::NIP42_PURPOSE;
pub use context::NIP98_EVENT_KIND;
pub use context::NIP98_PURPOSE;
pub use context::ReplayDisposition;
pub use context::ReplayObservation;
pub use context::VerificationRequest;
pub use context::VerifiedNostrCredential;
pub use context::verify_credential;
pub use model::AdapterBlake3;
pub use model::BoundaryClaims;
pub use model::DigestRole;
pub use model::MAX_CANONICAL_EVENT_BYTES;
pub use model::MAX_CONTENT_BYTES;
pub use model::MAX_EVENT_INPUT_BYTES;
pub use model::MAX_TAG_VALUE_BYTES;
pub use model::MAX_TAG_VALUES;
pub use model::MAX_TAGS;
pub use model::NostrEvent;
pub use model::NostrEventId;
pub use model::NostrPublicKey;
pub use model::VerificationIssue;
pub use model::WireDigest;
pub use model::validate_wire_digest;
pub use protocol::VerifiedNostrEvent;
pub use protocol::canonical_event_bytes;
pub use protocol::compute_event_id;
pub use protocol::parse_event_json;
pub use protocol::verify_bip340_message;
pub use protocol::verify_event;

#[cfg(test)]
mod tests;
