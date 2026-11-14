mod compatibility;
mod mapping;
mod projection;

#[cfg(test)]
mod compatibility_tests;
#[cfg(test)]
mod tests;

use artifact_auth_core::{ArtifactStatement, AuthenticationPolicy, AuthenticationScope};
use serde::Deserialize;
use serde::Serialize;

use crate::AuthenticatedEvidenceIssue;

pub use compatibility::{
    compare_cairn_artifact_auth, CairnArtifactAuthCompatibilityClassification,
    CairnArtifactAuthCompatibilityDimension, CairnArtifactAuthCompatibilityDisposition,
    CairnArtifactAuthCompatibilityReport, CAIRN_ARTIFACT_AUTH_COMPATIBILITY_BOUNDARY,
    CAIRN_ARTIFACT_AUTH_COMPATIBILITY_SCHEMA_V1,
};
pub use mapping::map_cairn_artifact_auth;
pub use projection::evaluate_cairn_artifact_auth;

pub const CAIRN_ARTIFACT_AUTH_PROFILE_V1: &str = "cairn.artifact-auth.v1";
pub const CAIRN_ARTIFACT_AUTH_PARENT_PROFILE_V1: &str = "cairn.authenticated-parent.v1";
pub const CAIRN_ARTIFACT_AUTH_VERIFIER_CONTEXT_PROFILE_V1: &str = "valence.integrity-receipt.v1";
pub const CAIRN_ARTIFACT_AUTH_BOUNDARY: &str = "The standalone authentication decision is one bounded Cairn observation; Cairn retains filesystem measurement, receipt canonicalization, trust selection, currentness collection, lifecycle policy, readiness composition, JSON presentation, and process exit authority, and authentication does not independently authorize or promote a release";

pub(crate) const CAIRN_LEGACY_KEY_GENERATION: u64 = 1;
pub(crate) const CAIRN_LEGACY_KEY_THRESHOLD: u16 = 1;
pub(crate) const ED25519_PUBLIC_KEY_BYTES: usize = 32;

/// Pure, fully mapped input for one standalone Cairn authentication evaluation.
///
/// The caller retains cryptographic-verifier authority and must verify `statement`
/// directly. A legacy Cairn verification boolean is never promoted into this value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CairnArtifactAuthInput {
    pub policy: AuthenticationPolicy,
    pub scope: AuthenticationScope,
    pub statement: ArtifactStatement,
    pub generation: u64,
    pub public_key_bytes: [u8; ED25519_PUBLIC_KEY_BYTES],
    pub legacy_public_key_blake3: String,
    pub standalone_public_key_blake3: String,
}

/// Cairn-owned compatibility projection of a bounded standalone decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CairnArtifactAuthReport {
    pub evidence_kind: String,
    pub decision_passed: bool,
    pub authentication_hash: Option<String>,
    pub measured_subject_hash: String,
    pub producer: String,
    pub key_id: String,
    pub issues: Vec<AuthenticatedEvidenceIssue>,
    pub boundary: String,
    pub standalone_statement_blake3: Option<String>,
    pub standalone_policy_blake3: String,
    pub standalone_scope_blake3: String,
    pub standalone_decision_blake3: String,
    pub standalone_verified_key_blake3: Vec<String>,
    pub legacy_public_key_blake3: String,
    pub standalone_public_key_blake3: String,
    pub non_claims: Vec<String>,
}
