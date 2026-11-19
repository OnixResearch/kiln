use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Digest algorithm and ownership role.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NostrDigestRole {
    /// NIP-01 protocol SHA-256 event identity.
    NostrEventSha256,
    /// BLAKE3 identity produced by the reviewed UCAN adapter.
    UcanAdapterBlake3,
    /// BLAKE3 identity produced by Basalt.
    BasaltBlake3,
}

/// Closed typed digest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NostrTypedDigest {
    /// Algorithm and ownership role.
    pub role: NostrDigestRole,
    /// Canonical lowercase 32-byte digest.
    pub hex: String,
}

impl NostrTypedDigest {
    pub(super) fn new(role: NostrDigestRole, hex: impl Into<String>) -> Self {
        Self {
            role,
            hex: hex.into(),
        }
    }
}

/// Closed key lifecycle state.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NostrBindingState {
    /// The key can authenticate a new request.
    Active,
    /// The key is retained for history and cannot authenticate.
    Retired,
    /// The key is explicitly revoked and cannot authenticate.
    Revoked,
}

/// One policy-owned Nostr authenticator.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NostrAuthenticatorPolicy {
    /// Stable binding identity.
    pub binding_id: String,
    /// Canonical lowercase x-only secp256k1 public key.
    pub public_key: String,
    /// Explicit lifecycle state.
    pub state: NostrBindingState,
}

/// One stable Basalt subject.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NostrSubjectPolicy {
    /// Stable policy subject identity.
    pub subject_id: String,
    /// Optional bounded public display label with no authority meaning.
    #[serde(default)]
    pub display_label: Option<String>,
    /// Nostr authenticators associated with the stable subject.
    pub authenticators: Vec<NostrAuthenticatorPolicy>,
}

/// Complete Nickel-authored subject-binding policy.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NostrIdentityPolicy {
    /// Closed schema identity.
    pub schema: String,
    /// Reviewable policy revision label.
    pub policy_revision: String,
    /// Stable subjects and their key lifecycle.
    pub subjects: Vec<NostrSubjectPolicy>,
    /// Exact required authority boundaries.
    pub non_claims: Vec<String>,
}

/// Stable field-scoped validation issue.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NostrIdentityIssue {
    /// Closed field path.
    pub field: &'static str,
    /// Stable machine code.
    pub code: &'static str,
}

impl NostrIdentityIssue {
    pub(super) const fn new(field: &'static str, code: &'static str) -> Self {
        Self { field, code }
    }
}

/// Policy decode or validation failure.
#[derive(Debug)]
pub enum NostrIdentityPolicyError {
    /// JSON did not match the closed policy DTO.
    Decode(String),
    /// The decoded policy violated one or more invariants.
    Invalid(Vec<NostrIdentityIssue>),
}

impl fmt::Display for NostrIdentityPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(message) => {
                write!(formatter, "Nostr identity policy decode failed: {message}")
            }
            Self::Invalid(issues) => write!(
                formatter,
                "Nostr identity policy has {} issue(s)",
                issues.len()
            ),
        }
    }
}

impl Error for NostrIdentityPolicyError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct IndexedBinding {
    pub(super) subject_id: String,
    pub(super) binding_id: String,
    pub(super) public_key: String,
    pub(super) state: NostrBindingState,
}

/// Validated policy and deterministic key index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedNostrIdentityPolicy {
    pub(super) policy: NostrIdentityPolicy,
    pub(super) policy_identity: NostrTypedDigest,
    pub(super) bindings_by_key: BTreeMap<String, IndexedBinding>,
}

impl ValidatedNostrIdentityPolicy {
    /// Return the validated policy.
    #[must_use]
    pub const fn policy(&self) -> &NostrIdentityPolicy {
        &self.policy
    }

    /// Return the order-independent Basalt policy identity.
    #[must_use]
    pub const fn policy_identity(&self) -> &NostrTypedDigest {
        &self.policy_identity
    }
}
