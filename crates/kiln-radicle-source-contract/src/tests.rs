use alloc::string::ToString;

use super::*;

// r[verify kiln.radicle.source_contract.boundary.scenario.standalone]
// r[verify kiln.radicle.source_contract.request.scenario.exact]
// r[verify kiln.radicle.source_contract.sigrefs.scenario.stable]
// r[verify kiln.radicle.source_contract.snapshot.scenario.accepted]
// r[verify kiln.radicle.source_contract.linkage.scenario.match]
#[test]
fn accepts_exact_parent_level_receipt() {
    let request = fixture_request();
    let receipt = fixture_receipt();

    assert_eq!(admit_request(&request), Ok(()));
    assert_eq!(admit_receipt(&receipt), Ok(()));
}

// r[verify kiln.radicle.source_contract.request.scenario.floating]
#[test]
fn rejects_floating_or_malformed_request_facts() {
    let mut request = fixture_request();
    request.expected_sigrefs_commit.clear();
    assert_eq!(
        admit_request(&request),
        Err(AdmissionError::Oid {
            field: "request.expected_sigrefs_commit"
        })
    );

    let mut request = fixture_request();
    request.signed_reference = "main".to_string();
    assert_eq!(
        admit_request(&request),
        Err(AdmissionError::Reference {
            field: "request.signed_reference"
        })
    );

    let mut request = fixture_request();
    request.repository_id = "not-a-rid".to_string();
    assert_eq!(
        admit_request(&request),
        Err(AdmissionError::Reference {
            field: "request.repository_id"
        })
    );
}

#[test]
fn rejects_zero_and_inconsistent_bounds() {
    let mut request = fixture_request();
    request.limits.max_entries = 0;
    assert_eq!(
        admit_request(&request),
        Err(AdmissionError::ZeroBound {
            field: "request.limits.max_entries"
        })
    );

    let mut request = fixture_request();
    request.limits.max_file_bytes = request.limits.max_total_bytes + 1;
    assert_eq!(
        admit_request(&request),
        Err(AdmissionError::InvalidBound {
            field: "request.limits.max_file_bytes"
        })
    );
}

// r[verify kiln.radicle.source_contract.sigrefs.scenario.moved]
#[test]
fn rejects_moved_or_unexpected_signed_refs() {
    let mut receipt = fixture_receipt();
    receipt.observation.sigrefs_commit_after =
        "7777777777777777777777777777777777777777".to_string();
    assert_eq!(admit_receipt(&receipt), Err(AdmissionError::MovedSigrefs));

    let mut receipt = fixture_receipt();
    receipt.observation.sigrefs_commit_before =
        "7777777777777777777777777777777777777777".to_string();
    receipt.observation.sigrefs_commit_after =
        "7777777777777777777777777777777777777777".to_string();
    assert_eq!(
        admit_receipt(&receipt),
        Err(AdmissionError::Linkage {
            field: "observation.sigrefs_commit_before"
        })
    );
}

// r[verify kiln.radicle.source_contract.sigrefs.scenario.downgrade]
#[test]
fn rejects_feature_and_verification_downgrades() {
    let mut receipt = fixture_receipt();
    receipt.observation.feature_level = FeatureLevel::Root;
    assert_eq!(admit_receipt(&receipt), Err(AdmissionError::FeatureLevel));

    let mut receipt = fixture_receipt();
    receipt.observation.signature = Verification::NotVerified;
    assert_eq!(
        admit_receipt(&receipt),
        Err(AdmissionError::Verification {
            field: "observation.signature"
        })
    );
}

// r[verify kiln.radicle.source_contract.linkage.scenario.signer]
#[test]
fn rejects_identity_and_revision_substitution() {
    let mut receipt = fixture_receipt();
    receipt.observation.signer_node_id = "z6MkOtherSigner".to_string();
    assert_eq!(
        admit_receipt(&receipt),
        Err(AdmissionError::Linkage {
            field: "observation.signer_node_id"
        })
    );

    let mut receipt = fixture_receipt();
    receipt.snapshot.revision = "8888888888888888888888888888888888888888".to_string();
    assert_eq!(
        admit_receipt(&receipt),
        Err(AdmissionError::Linkage {
            field: "snapshot.revision"
        })
    );
}

// r[verify kiln.radicle.source_contract.snapshot.scenario.bound]
#[test]
fn rejects_digest_and_snapshot_bound_failures() {
    let mut receipt = fixture_receipt();
    receipt.snapshot.canonical_tree_blake3 = "not-a-digest".to_string();
    assert_eq!(
        admit_receipt(&receipt),
        Err(AdmissionError::Digest {
            field: "snapshot.canonical_tree_blake3"
        })
    );

    let mut receipt = fixture_receipt();
    receipt.snapshot.entry_count = receipt.request.limits.max_entries + 1;
    assert_eq!(
        admit_receipt(&receipt),
        Err(AdmissionError::SnapshotBound {
            field: "snapshot.entry_count"
        })
    );

    let mut receipt = fixture_receipt();
    receipt.snapshot.total_bytes = receipt.request.limits.max_total_bytes + 1;
    assert_eq!(
        admit_receipt(&receipt),
        Err(AdmissionError::SnapshotBound {
            field: "snapshot.total_bytes"
        })
    );
}

// r[verify kiln.radicle.source_contract.boundary.scenario.runtime]
#[test]
fn contract_manifest_has_no_provider_runtime_dependency() {
    let manifest = include_str!("../Cargo.toml");
    for forbidden in [
        "git2",
        "radicle-crypto",
        "radicle-core",
        "radicle-oid",
        "rusqlite",
        "bounded-exec",
        "kiln-core",
        "kiln-provider-api",
    ] {
        assert!(!manifest.contains(forbidden));
    }
}

// r[verify kiln.radicle.source_contract.nonclaims]
// r[verify kiln.radicle.source_contract.nonclaims.scenario.accepted]
#[test]
fn contract_publishes_explicit_non_claims() {
    assert!(SOURCE_NON_CLAIMS.iter().all(|claim| !claim.is_empty()));
    assert!(
        SOURCE_NON_CLAIMS
            .iter()
            .any(|claim| claim.contains("release readiness"))
    );
}

#[test]
fn rejects_unknown_wire_fields() {
    let encoded = serde_json::to_string(&fixture_request());
    assert!(encoded.is_ok());
    let with_unknown = encoded.map(|value| value.replacen("{", "{\"unknown\":true,", 1));
    let decoded = with_unknown.and_then(|value| serde_json::from_str::<SourceRequest>(&value));
    assert!(decoded.is_err());
}
