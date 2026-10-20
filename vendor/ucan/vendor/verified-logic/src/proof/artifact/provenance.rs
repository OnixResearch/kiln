//! Finite proof-artifact provenance admission.
//!
//! This module checks shell-supplied provenance facts for exportable Trellis proof
//! artifacts. It does not compute BLAKE3, run Verus, read files, validate a
//! downstream receipt, or certify downstream Octet/Valence/Mantle/Cairn behavior.
//! The shell owns bytes, paths, commands, and digest computation; this verified
//! core admits only the in-memory identity, local-property, non-claim, and handoff
//! role facts.

#![allow(clippy::match_same_arms)]
#![allow(clippy::struct_excessive_bools)]

use vstd::prelude::*;

verus! {

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Passing,
    Failing,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClaimRole {
    TrellisLocalProperty,
    ManualReviewInput,
    DownstreamCertified,
    ReleaseEligibility,
    WholeStackCorrectness,
    Unsupported,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Metadata {
    pub artifact_id_present: bool,
    pub canonical_metadata_digest_matches: bool,
    pub source_input_digests_present: bool,
    pub proof_input_digests_present: bool,
    pub verifier_receipt_digest_present: bool,
    pub verifier_identity_present: bool,
    pub normalized_invocation_present: bool,
    pub local_property_id_present: bool,
    pub content_scope_present: bool,
    pub status: Status,
    pub failure_reason_present: bool,
    pub required_non_claims_present: bool,
    pub claim_role: ClaimRole,
    pub downstream_receipt_present: bool,
}

// r[impl trellis.proof_artifact_provenance.identity]
pub open spec fn failure_reason_admitted_spec(status: Status, failure_reason_present: bool) -> bool {
    match status {
        Status::Passing => true,
        Status::Failing => failure_reason_present,
    }
}

// r[impl trellis.proof_artifact_provenance.boundary]
pub open spec fn overclaim_free_role_spec(role: ClaimRole) -> bool {
    match role {
        ClaimRole::TrellisLocalProperty => true,
        ClaimRole::ManualReviewInput => true,
        ClaimRole::DownstreamCertified => false,
        ClaimRole::ReleaseEligibility => false,
        ClaimRole::WholeStackCorrectness => false,
        ClaimRole::Unsupported => false,
    }
}

// r[impl trellis.proof_artifact_provenance.handoff]
pub open spec fn handoff_role_admitted_spec(role: ClaimRole, downstream_receipt_present: bool) -> bool {
    match role {
        ClaimRole::TrellisLocalProperty => false,
        ClaimRole::ManualReviewInput => true,
        ClaimRole::DownstreamCertified => downstream_receipt_present,
        ClaimRole::ReleaseEligibility => false,
        ClaimRole::WholeStackCorrectness => false,
        ClaimRole::Unsupported => false,
    }
}

// r[impl trellis.proof_artifact_provenance.identity]
pub open spec fn identity_valid_spec(metadata: Metadata) -> bool {
    metadata.artifact_id_present
        && metadata.canonical_metadata_digest_matches
        && metadata.source_input_digests_present
        && metadata.proof_input_digests_present
        && metadata.verifier_receipt_digest_present
        && metadata.verifier_identity_present
        && metadata.normalized_invocation_present
        && metadata.local_property_id_present
        && failure_reason_admitted_spec(metadata.status, metadata.failure_reason_present)
}

// r[impl trellis.proof_artifact_provenance.boundary]
pub open spec fn boundary_valid_spec(metadata: Metadata) -> bool {
    metadata.local_property_id_present
        && metadata.content_scope_present
        && metadata.required_non_claims_present
        && overclaim_free_role_spec(metadata.claim_role)
}

// r[impl trellis.proof_artifact_provenance.identity]
// r[impl trellis.proof_artifact_provenance.boundary]
pub open spec fn metadata_valid_spec(metadata: Metadata) -> bool {
    identity_valid_spec(metadata) && boundary_valid_spec(metadata)
}

// r[impl trellis.proof_artifact_provenance.fixtures]
pub open spec fn fixture_surface_spec() -> bool {
    true
}

// r[verify trellis.proof_artifact_provenance.fixtures]
pub proof fn fixture_surface_declared()
    ensures fixture_surface_spec()
{
}

// r[impl trellis.proof_artifact_provenance.identity]
pub fn failure_reason_admitted(status: Status, failure_reason_present: bool) -> (result: bool)
    ensures result == failure_reason_admitted_spec(status, failure_reason_present)
{
    match status {
        Status::Passing => true,
        Status::Failing => failure_reason_present,
    }
}

// r[impl trellis.proof_artifact_provenance.boundary]
pub fn overclaim_free_role(role: ClaimRole) -> (result: bool)
    ensures result == overclaim_free_role_spec(role)
{
    match role {
        ClaimRole::TrellisLocalProperty => true,
        ClaimRole::ManualReviewInput => true,
        ClaimRole::DownstreamCertified => false,
        ClaimRole::ReleaseEligibility => false,
        ClaimRole::WholeStackCorrectness => false,
        ClaimRole::Unsupported => false,
    }
}

// r[impl trellis.proof_artifact_provenance.handoff]
pub fn handoff_role_admitted(role: ClaimRole, downstream_receipt_present: bool) -> (result: bool)
    ensures result == handoff_role_admitted_spec(role, downstream_receipt_present)
{
    match role {
        ClaimRole::TrellisLocalProperty => false,
        ClaimRole::ManualReviewInput => true,
        ClaimRole::DownstreamCertified => downstream_receipt_present,
        ClaimRole::ReleaseEligibility => false,
        ClaimRole::WholeStackCorrectness => false,
        ClaimRole::Unsupported => false,
    }
}

// r[impl trellis.proof_artifact_provenance.identity]
pub fn identity_valid(metadata: Metadata) -> (result: bool)
    ensures result == identity_valid_spec(metadata)
{
    metadata.artifact_id_present
        && metadata.canonical_metadata_digest_matches
        && metadata.source_input_digests_present
        && metadata.proof_input_digests_present
        && metadata.verifier_receipt_digest_present
        && metadata.verifier_identity_present
        && metadata.normalized_invocation_present
        && metadata.local_property_id_present
        && failure_reason_admitted(metadata.status, metadata.failure_reason_present)
}

// r[impl trellis.proof_artifact_provenance.boundary]
pub fn boundary_valid(metadata: Metadata) -> (result: bool)
    ensures result == boundary_valid_spec(metadata)
{
    metadata.local_property_id_present
        && metadata.content_scope_present
        && metadata.required_non_claims_present
        && overclaim_free_role(metadata.claim_role)
}

// r[impl trellis.proof_artifact_provenance.identity]
// r[impl trellis.proof_artifact_provenance.boundary]
pub fn metadata_valid(metadata: Metadata) -> (result: bool)
    ensures result == metadata_valid_spec(metadata)
{
    identity_valid(metadata) && boundary_valid(metadata)
}

// r[verify trellis.proof_artifact_provenance.boundary]
pub proof fn valid_metadata_has_local_scope(metadata: Metadata)
    requires metadata_valid_spec(metadata)
    ensures metadata.local_property_id_present, metadata.content_scope_present
{
}

// r[verify trellis.proof_artifact_provenance.boundary]
pub proof fn valid_metadata_has_required_non_claims(metadata: Metadata)
    requires metadata_valid_spec(metadata)
    ensures metadata.required_non_claims_present
{
}

// r[verify trellis.proof_artifact_provenance.boundary]
pub proof fn valid_metadata_excludes_downstream_overclaim(metadata: Metadata)
    requires metadata_valid_spec(metadata)
    ensures overclaim_free_role_spec(metadata.claim_role)
{
}

// r[verify trellis.proof_artifact_provenance.identity]
pub proof fn stale_digest_rejected(metadata: Metadata)
    requires !metadata.canonical_metadata_digest_matches
    ensures !metadata_valid_spec(metadata)
{
}

// r[verify trellis.proof_artifact_provenance.handoff]
pub proof fn downstream_certified_handoff_requires_receipt()
    ensures !handoff_role_admitted_spec(ClaimRole::DownstreamCertified, false)
{
}

} // verus!

#[cfg(test)]
#[path = "provenance_tests.rs"]
mod tests;
