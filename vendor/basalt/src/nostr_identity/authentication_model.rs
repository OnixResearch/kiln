use serde::{Deserialize, Serialize};

use super::NostrTypedDigest;

/// Public request facts that must match the opaque UCAN credential.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NostrIdentityBindingRequest {
    /// Exact UCAN adapter request identity.
    pub request_identity: NostrTypedDigest,
    /// Exact audience bound by the signed event.
    pub audience: String,
    /// Exact purpose bound by the signed event.
    pub purpose: String,
    /// Exact currently selected Basalt policy identity.
    pub policy_identity: NostrTypedDigest,
    /// Explicit boundary observation.
    pub boundary: NostrIdentityBoundary,
}

/// Caller-declared boundary posture.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NostrIdentityBoundary {
    /// Input contains only public bounded identity facts.
    PublicOnly,
    /// Input contains a private key, credential, proof, challenge, or secret.
    ContainsSecretMaterial,
    /// Input tries to replace the opaque credential with a validity boolean.
    CallerSuppliedValidity,
    /// Input promotes NIP-05, profile, or display metadata to identity proof.
    PromotesMetadataIdentity,
    /// Input asks authentication to grant authority.
    RequestsAuthority,
}

/// Recorded-only evidence role.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NostrEvidenceRole {
    /// Identity and linkage only. It does not count as a verified property.
    RecordedOnly,
    /// Forbidden property promotion used for fail-closed validation.
    Property,
    /// Forbidden authority promotion used for fail-closed validation.
    Authority,
}

/// Authentication decision retained in the public receipt.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NostrAuthenticationDecision {
    /// One active key resolved to one stable subject.
    Authenticated,
    /// Resolution failed closed.
    Denied,
}

/// Closed redacted authentication receipt consumed by Valence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NostrAuthenticationReceipt {
    /// Closed receipt schema.
    pub schema: String,
    /// Exact integrated UCAN source revision.
    pub ucan_source_revision: String,
    /// Exact reviewed credential contract.
    pub credential_contract: String,
    /// Basalt Nostr producer schema identity.
    pub producer_schema_identity: NostrTypedDigest,
    /// Basalt Nostr implementation-profile identity.
    pub producer_implementation_identity: NostrTypedDigest,
    /// External Nostr event identity.
    pub event_identity: NostrTypedDigest,
    /// Public canonical Nostr key.
    pub public_key: String,
    /// UCAN adapter profile identity.
    pub adapter_profile_identity: NostrTypedDigest,
    /// UCAN adapter implementation identity.
    pub adapter_implementation_identity: NostrTypedDigest,
    /// UCAN request identity.
    pub request_identity: NostrTypedDigest,
    /// BLAKE3 identity of the exact audience text.
    pub audience_identity: NostrTypedDigest,
    /// BLAKE3 identity of the exact purpose text.
    pub purpose_identity: NostrTypedDigest,
    /// Basalt policy identity.
    pub policy_identity: NostrTypedDigest,
    /// Stable subject when authentication succeeds.
    pub subject_id: Option<String>,
    /// Stable binding when authentication succeeds.
    pub binding_id: Option<String>,
    /// Basalt binding identity when authentication succeeds.
    pub binding_identity: Option<NostrTypedDigest>,
    /// Terminal decision.
    pub decision: NostrAuthenticationDecision,
    /// Stable public reason code.
    pub reason: String,
    /// Always recorded-only.
    pub verification_role: NostrEvidenceRole,
    /// Required claim boundaries.
    pub non_claims: Vec<String>,
    /// BLAKE3 identity of this receipt with this field empty.
    pub receipt_identity: NostrTypedDigest,
}

/// Opaque stable subject produced only after exact resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedNostrSubject {
    pub(super) subject_id: String,
    pub(super) binding_id: String,
    pub(super) request_identity: NostrTypedDigest,
    pub(super) policy_identity: NostrTypedDigest,
    pub(super) authentication_receipt_identity: NostrTypedDigest,
}

impl AuthenticatedNostrSubject {
    /// Return the stable policy subject identity.
    #[must_use]
    pub fn subject_id(&self) -> &str {
        &self.subject_id
    }

    /// Return the selected binding identity.
    #[must_use]
    pub fn binding_id(&self) -> &str {
        &self.binding_id
    }

    /// Return the exact request identity.
    #[must_use]
    pub const fn request_identity(&self) -> &NostrTypedDigest {
        &self.request_identity
    }

    /// Return the exact policy identity.
    #[must_use]
    pub const fn policy_identity(&self) -> &NostrTypedDigest {
        &self.policy_identity
    }

    /// Return the authentication receipt identity.
    #[must_use]
    pub const fn authentication_receipt_identity(&self) -> &NostrTypedDigest {
        &self.authentication_receipt_identity
    }
}

/// Authentication receipt plus the optional opaque successful subject.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NostrAuthenticationResult {
    /// Redacted terminal receipt.
    pub receipt: NostrAuthenticationReceipt,
    pub(super) subject: Option<AuthenticatedNostrSubject>,
}

impl NostrAuthenticationResult {
    /// Return the opaque subject only for an authenticated receipt.
    #[must_use]
    pub const fn subject(&self) -> Option<&AuthenticatedNostrSubject> {
        self.subject.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CredentialFacts {
    pub(super) public_key: String,
    pub(super) event_identity: String,
    pub(super) profile: &'static str,
    pub(super) audience: String,
    pub(super) purpose: String,
    pub(super) request_identity: String,
    pub(super) adapter_profile_identity: String,
    pub(super) adapter_implementation_identity: String,
}
