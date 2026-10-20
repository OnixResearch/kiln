use super::*;

fn passing_metadata() -> Metadata {
    Metadata {
        artifact_id_present: true,
        canonical_metadata_digest_matches: true,
        source_input_digests_present: true,
        proof_input_digests_present: true,
        verifier_receipt_digest_present: true,
        verifier_identity_present: true,
        normalized_invocation_present: true,
        local_property_id_present: true,
        content_scope_present: true,
        status: Status::Passing,
        failure_reason_present: false,
        required_non_claims_present: true,
        claim_role: ClaimRole::TrellisLocalProperty,
        downstream_receipt_present: false,
    }
}

// r[verify trellis.proof_artifact_provenance.fixtures]
#[test]
fn passing_local_artifact_metadata_is_accepted() {
    assert!(metadata_valid(passing_metadata()));
}

#[test]
fn failing_local_artifact_with_reason_is_accepted() {
    let mut metadata = passing_metadata();
    metadata.status = Status::Failing;
    metadata.failure_reason_present = true;
    assert!(metadata_valid(metadata));
}

// r[verify trellis.proof_artifact_provenance.fixtures]
#[test]
fn stale_digest_fails_closed() {
    let mut metadata = passing_metadata();
    metadata.canonical_metadata_digest_matches = false;
    assert!(!metadata_valid(metadata));
}

#[test]
fn missing_local_property_scope_fails_closed() {
    let mut metadata = passing_metadata();
    metadata.content_scope_present = false;
    assert!(!metadata_valid(metadata));
}

#[test]
fn missing_verifier_identity_fails_closed() {
    let mut metadata = passing_metadata();
    metadata.verifier_identity_present = false;
    assert!(!metadata_valid(metadata));
}

#[test]
fn missing_non_claims_fail_closed() {
    let mut metadata = passing_metadata();
    metadata.required_non_claims_present = false;
    assert!(!metadata_valid(metadata));
}

#[test]
fn unsupported_claim_role_fails_closed() {
    let mut metadata = passing_metadata();
    metadata.claim_role = ClaimRole::Unsupported;
    assert!(!metadata_valid(metadata));
}

#[test]
fn downstream_overclaim_fails_closed() {
    let mut metadata = passing_metadata();
    metadata.claim_role = ClaimRole::DownstreamCertified;
    metadata.downstream_receipt_present = true;
    assert!(!metadata_valid(metadata));
}

#[test]
fn failed_artifact_requires_failure_reason() {
    let mut metadata = passing_metadata();
    metadata.status = Status::Failing;
    metadata.failure_reason_present = false;
    assert!(!metadata_valid(metadata));
}

#[test]
fn handoff_rejects_downstream_certified_without_receipt() {
    assert!(!handoff_role_admitted(ClaimRole::DownstreamCertified, false));
    assert!(handoff_role_admitted(ClaimRole::DownstreamCertified, true));
    assert!(handoff_role_admitted(ClaimRole::ManualReviewInput, false));
}
