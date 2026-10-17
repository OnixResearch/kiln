use alloc::string::ToString;
use alloc::vec;

use super::*;

fn fixture() -> CoreResult<(HostQualificationCohort, ContentId, HostQualificationReceipt)> {
    let identity = ContentId::from_blake3_hex("1".repeat(crate::types::BLAKE3_HEX_BYTES))?;
    let cohort = HostQualificationCohort {
        source_revision: ExternalRef::new("git", "commit", "published-source-revision")?,
        source_archive: identity.clone(),
        host_executable: identity.clone(),
        extension_executable: identity.clone(),
        provider_executable: identity.clone(),
        runtime_profile: identity.clone(),
        collaboration_profile: identity.clone(),
        provider_profile: identity.clone(),
        host_identity: identity.clone(),
        publication_binding: identity.clone(),
    };
    let kinds = [
        HostObservationKind::Durability,
        HostObservationKind::UncertainRecovery,
        HostObservationKind::Corruption,
        HostObservationKind::ProviderTeardown,
        HostObservationKind::FiniteLoad,
        HostObservationKind::BackupRestore,
        HostObservationKind::Rollback,
        HostObservationKind::Authority,
        HostObservationKind::Publication,
        HostObservationKind::HostGate,
        HostObservationKind::ProviderGate,
        HostObservationKind::ClippyGate,
        HostObservationKind::OctetGate,
        HostObservationKind::WorkspaceGate,
        HostObservationKind::NoStdGate,
        HostObservationKind::NickelGate,
        HostObservationKind::NixGate,
        HostObservationKind::LifecycleGate,
    ];
    let receipt = HostQualificationReceipt {
        contract_version: CONTRACT_VERSION_V1,
        cohort: identity.clone(),
        claims: vec![HostQualificationClaim::BoundedLocalHost],
        observations: kinds
            .into_iter()
            .map(|kind| HostQualificationObservation {
                kind,
                cohort: identity.clone(),
                artifact: identity.clone(),
                verdict: HostObservationVerdict::Passed,
            })
            .collect(),
    };
    Ok((cohort, identity, receipt))
}

// r[verify kiln.aspen_host.production_evidence]
// Synthetic observations verify complete receipt admission, not native/provider execution.
// r[verify kiln.aspen_host.production_evidence.accepted]
#[test]
fn admits_complete_exact_cohort_with_only_bounded_local_claim() -> CoreResult<()> {
    let (cohort, identity, receipt) = fixture()?;
    assert_eq!(
        validate_host_qualification(&cohort, &identity, &receipt),
        Ok(())
    );
    Ok(())
}

// r[verify kiln.aspen_host.production_evidence.accepted]
#[test]
fn missing_observation_cannot_qualify() -> CoreResult<()> {
    let (cohort, identity, mut receipt) = fixture()?;
    receipt.observations.pop();
    assert_eq!(
        validate_host_qualification(&cohort, &identity, &receipt)
            .err()
            .map(|error| error.code),
        Some("qualification_incomplete".to_string())
    );
    Ok(())
}

// r[verify kiln.aspen_host.production_evidence.accepted]
#[test]
fn duplicate_cannot_replace_missing_kind() -> CoreResult<()> {
    let (cohort, identity, mut receipt) = fixture()?;
    if let Some(observation) = receipt.observations.last_mut() {
        observation.kind = HostObservationKind::Durability;
    }
    assert_eq!(
        validate_host_qualification(&cohort, &identity, &receipt)
            .err()
            .map(|error| error.code),
        Some("qualification_duplicate".to_string())
    );
    Ok(())
}

// r[verify kiln.aspen_host.production_evidence.accepted]
#[test]
fn failed_gate_cannot_be_promoted_to_qualification() -> CoreResult<()> {
    let (cohort, identity, mut receipt) = fixture()?;
    if let Some(observation) = receipt.observations.last_mut() {
        observation.verdict = HostObservationVerdict::Failed;
    }
    assert_eq!(
        validate_host_qualification(&cohort, &identity, &receipt)
            .err()
            .map(|error| error.code),
        Some("qualification_gate_not_passed".to_string())
    );
    Ok(())
}

// r[verify kiln.aspen_host.production_evidence.accepted]
#[test]
fn uncertain_case_cannot_be_promoted_to_success() -> CoreResult<()> {
    let (cohort, identity, mut receipt) = fixture()?;
    if let Some(observation) = receipt.observations.first_mut() {
        observation.verdict = HostObservationVerdict::Unknown;
    }
    assert_eq!(
        validate_host_qualification(&cohort, &identity, &receipt)
            .err()
            .map(|error| error.code),
        Some("qualification_gate_not_passed".to_string())
    );
    Ok(())
}

// r[verify kiln.aspen_host.production_evidence.accepted]
#[test]
fn stale_receipt_is_rejected_against_expected_cohort() -> CoreResult<()> {
    let (cohort, identity, mut receipt) = fixture()?;
    receipt.cohort = ContentId::from_blake3_hex("2".repeat(crate::types::BLAKE3_HEX_BYTES))?;
    assert_eq!(
        validate_host_qualification(&cohort, &identity, &receipt)
            .err()
            .map(|error| error.code),
        Some("qualification_cohort_mismatch".to_string())
    );
    Ok(())
}

// r[verify kiln.aspen_host.production_evidence.accepted]
#[test]
fn individual_observation_cannot_be_spliced_from_another_cohort() -> CoreResult<()> {
    let (cohort, identity, mut receipt) = fixture()?;
    if let Some(observation) = receipt.observations.first_mut() {
        observation.cohort =
            ContentId::from_blake3_hex("2".repeat(crate::types::BLAKE3_HEX_BYTES))?;
    }
    assert_eq!(
        validate_host_qualification(&cohort, &identity, &receipt)
            .err()
            .map(|error| error.code),
        Some("qualification_mixed_cohort".to_string())
    );
    Ok(())
}

// r[verify kiln.aspen_host.production_evidence.rejected]
#[test]
fn complete_local_evidence_does_not_establish_global_durability() -> CoreResult<()> {
    let (cohort, identity, mut receipt) = fixture()?;
    receipt.claims = vec![HostQualificationClaim::GlobalDurability];
    assert_eq!(
        validate_host_qualification(&cohort, &identity, &receipt)
            .err()
            .map(|error| error.code),
        Some("qualification_stronger_claim".to_string())
    );
    Ok(())
}

// r[verify kiln.aspen_host.production_evidence.rejected]
#[test]
fn bounded_claim_does_not_hide_added_release_authority() -> CoreResult<()> {
    let (cohort, identity, mut receipt) = fixture()?;
    receipt
        .claims
        .push(HostQualificationClaim::ReleaseEligibility);
    assert_eq!(
        validate_host_qualification(&cohort, &identity, &receipt)
            .err()
            .map(|error| error.code),
        Some("qualification_stronger_claim".to_string())
    );
    Ok(())
}

#[test]
fn unsupported_receipt_version_is_not_admitted() -> CoreResult<()> {
    let (cohort, identity, mut receipt) = fixture()?;
    receipt.contract_version = CONTRACT_VERSION_V1 + 1;
    assert_eq!(
        validate_host_qualification(&cohort, &identity, &receipt)
            .err()
            .map(|error| error.code),
        Some("qualification_version".to_string())
    );
    Ok(())
}
