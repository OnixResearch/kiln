//! Pure Nostr credential-to-subject binding and recorded-only receipts.
//!
//! The public adapter accepts only the reviewed UCAN `VerifiedNostrCredential`
//! type. All policy, request, and authorization facts are already-loaded values.
//! This module performs no filesystem, network, clock, signer, replay-store, or
//! policy-publication effects.

mod authentication;
mod authentication_model;
mod composition;
mod identity;
mod policy;
mod policy_model;
#[cfg(test)]
mod tests;

pub use authentication::{resolve_verified_nostr_subject, verify_nostr_authentication_receipt};
pub use authentication_model::*;
pub use composition::{
    NostrAuthorizationEvidence, NostrCompositionDecision, NostrCompositionReceipt,
    compose_nostr_authorization, verify_nostr_composition_receipt,
};
pub use identity::{nostr_identity_implementation_identity, nostr_identity_schema_identity};
pub use policy::{parse_nostr_identity_policy, validate_nostr_identity_policy};
pub use policy_model::*;

/// Integrated UCAN revision that contains holder admission and Nostr support.
pub const UCAN_SOURCE_REVISION: &str = "fca62a6ed6895689ff18112b497bc7a986516c20";
/// Reviewed source revision that introduced `VerifiedNostrCredential`.
pub const UCAN_NOSTR_CREDENTIAL_REVISION: &str = "c9d43a7c11dd2fab4d5cb3d100012fa9a2e5b026";
/// Exact producer contract accepted by the public adapter.
pub const UCAN_NOSTR_CREDENTIAL_CONTRACT: &str =
    "ucan-nostr::VerifiedNostrCredential@c9d43a7c11dd2fab4d5cb3d100012fa9a2e5b026";
/// Closed Nickel policy schema.
pub const NOSTR_IDENTITY_POLICY_SCHEMA: &str = "basalt.nostr-identity-policy.v1";
/// Closed authentication receipt schema.
pub const NOSTR_AUTHENTICATION_RECEIPT_SCHEMA: &str = "basalt.nostr-authentication-receipt.v1";
/// Closed authorization-composition receipt schema.
pub const NOSTR_COMPOSITION_RECEIPT_SCHEMA: &str = "basalt.nostr-composition-receipt.v1";
/// Reviewed implementation profile used to derive the producer identity.
pub const BASALT_NOSTR_IMPLEMENTATION_REVISION: &str = "basalt-nostr-identity@0.1.0|ucan-nostr-credential@c9d43a7c11dd2fab4d5cb3d100012fa9a2e5b026|policy-v1|authentication-receipt-v1|composition-receipt-v1";

const SCHEMA_IDENTITY_DOMAIN: &str = "basalt.nostr-identity-schema-identity.v1";
const IMPLEMENTATION_IDENTITY_DOMAIN: &str = "basalt.nostr-identity-implementation-identity.v1";
const POLICY_IDENTITY_DOMAIN: &str = "basalt.nostr-identity-policy.v1";
const BINDING_IDENTITY_DOMAIN: &str = "basalt.nostr-binding.v1";
const PUBLIC_FIELD_IDENTITY_DOMAIN: &str = "basalt.nostr-public-field.v1";
const AUTHENTICATION_RECEIPT_DOMAIN: &str = "basalt.nostr-authentication-receipt.v1";
const COMPOSITION_RECEIPT_DOMAIN: &str = "basalt.nostr-composition-receipt.v1";
const DIGEST_HEX_BYTES: usize = 32;
const HEX_CHARS_PER_BYTE: usize = 2;
const DIGEST_HEX_CHARS: usize = DIGEST_HEX_BYTES * HEX_CHARS_PER_BYTE;
const MAX_SUBJECTS: usize = 256;
const MAX_AUTHENTICATORS_PER_SUBJECT: usize = 16;
const MAX_FIELD_BYTES: usize = 512;
const MAX_DISPLAY_LABEL_BYTES: usize = 128;

/// Required policy and receipt boundaries.
pub const NOSTR_IDENTITY_NON_CLAIMS: &[&str] = &[
    "Nostr key control does not prove a human identity or product ownership",
    "subject authentication does not grant a resource or ability",
    "Nostr relays, NIP-05, npub text, and profile metadata are not authority",
    "clocks, replay storage, policy publication, authorization, and release remain caller-owned",
    "BLAKE3 receipt identity does not authenticate the producer or establish trust",
];
