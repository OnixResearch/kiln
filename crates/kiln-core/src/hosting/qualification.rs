use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::types::{CONTRACT_VERSION_V1, ContentId, CoreResult, Diagnostic, ExternalRef};

pub const HOST_QUALIFICATION_OBSERVATIONS: usize = 18;

/// Exact inputs supplied by the operator; admission does not attest their truth.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostQualificationCohort {
    pub source_revision: ExternalRef,
    pub source_archive: ContentId,
    pub host_executable: ContentId,
    pub extension_executable: ContentId,
    pub provider_executable: ContentId,
    pub runtime_profile: ContentId,
    pub collaboration_profile: ContentId,
    pub provider_profile: ContentId,
    pub host_identity: ContentId,
    pub publication_binding: ContentId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(usize)]
pub enum HostObservationKind {
    Durability,
    UncertainRecovery,
    Corruption,
    ProviderTeardown,
    FiniteLoad,
    BackupRestore,
    Rollback,
    Authority,
    Publication,
    HostGate,
    ProviderGate,
    ClippyGate,
    OctetGate,
    WorkspaceGate,
    NoStdGate,
    NickelGate,
    NixGate,
    LifecycleGate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostObservationVerdict {
    Passed,
    Failed,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostQualificationClaim {
    BoundedLocalHost,
    GlobalDurability,
    ExactlyOnceDelivery,
    CiCorrectness,
    ProviderTruth,
    HostSandboxing,
    Availability,
    ReleaseEligibility,
    DeploymentAuthority,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostQualificationObservation {
    pub kind: HostObservationKind,
    pub cohort: ContentId,
    pub artifact: ContentId,
    pub verdict: HostObservationVerdict,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostQualificationReceipt {
    pub contract_version: u16,
    pub cohort: ContentId,
    pub claims: Vec<HostQualificationClaim>,
    pub observations: Vec<HostQualificationObservation>,
}

fn validate_identity(identity: &ContentId) -> CoreResult<()> {
    let text = identity.as_str();
    if text.len() != crate::types::BLAKE3_HEX_BYTES
        || !text.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(Diagnostic::new(
            "qualification_identity",
            "qualification.cohort",
            "qualification identities must be exact BLAKE3 digests",
        ));
    }
    Ok(())
}

// r[impl kiln.aspen_host.production_evidence]
// r[impl kiln.aspen_host.production_evidence.accepted]
// r[impl kiln.aspen_host.production_evidence.rejected]
/// Checks completeness and identity/claim consistency, not observation truth or authority.
/// `cohort_identity` identifies the exact expected cohort document observed by the shell.
/// Required observation kinds cover local execution cases and focused/broad gates;
/// passed verdicts remain supplied facts, not proof that those processes ran.
pub fn validate_host_qualification(
    expected: &HostQualificationCohort,
    cohort_identity: &ContentId,
    receipt: &HostQualificationReceipt,
) -> CoreResult<()> {
    expected.source_revision.validate()?;
    for text in [
        &expected.source_revision.provider_namespace,
        &expected.source_revision.native_kind,
        &expected.source_revision.native_id,
    ] {
        if text.as_str().is_empty() || text.as_str().len() > crate::types::DEFAULT_MAX_ID_BYTES {
            return Err(Diagnostic::new(
                "qualification_source",
                "qualification.source_revision",
                "source identity fields must satisfy their native identifier bounds",
            ));
        }
    }
    validate_identity(cohort_identity)?;
    for identity in [
        &expected.source_archive,
        &expected.host_executable,
        &expected.extension_executable,
        &expected.provider_executable,
        &expected.runtime_profile,
        &expected.collaboration_profile,
        &expected.provider_profile,
        &expected.host_identity,
        &expected.publication_binding,
    ] {
        validate_identity(identity)?;
    }
    if receipt.contract_version != CONTRACT_VERSION_V1 {
        return Err(Diagnostic::new(
            "qualification_version",
            "qualification.contract_version",
            "unsupported qualification contract version",
        ));
    }
    if receipt.claims.as_slice() != [HostQualificationClaim::BoundedLocalHost] {
        return Err(Diagnostic::new(
            "qualification_stronger_claim",
            "qualification.claims",
            "local observations cannot establish broader truth or release/deployment authority",
        ));
    }
    if &receipt.cohort != cohort_identity {
        return Err(Diagnostic::new(
            "qualification_cohort_mismatch",
            "qualification.cohort",
            "receipt does not identify the exact expected cohort document",
        ));
    }
    if receipt.observations.len() != HOST_QUALIFICATION_OBSERVATIONS {
        return Err(Diagnostic::new(
            "qualification_incomplete",
            "qualification.observations",
            "all required execution and gate observations must occur exactly once",
        ));
    }
    let mut observed = [false; HOST_QUALIFICATION_OBSERVATIONS];
    for observation in &receipt.observations {
        validate_identity(&observation.artifact)?;
        if &observation.cohort != cohort_identity {
            return Err(Diagnostic::new(
                "qualification_mixed_cohort",
                "qualification.observations.cohort",
                "an observation belongs to another cohort",
            ));
        }
        if observation.verdict != HostObservationVerdict::Passed {
            return Err(Diagnostic::new(
                "qualification_gate_not_passed",
                "qualification.observations.verdict",
                "failed or uncertain observations cannot qualify a host",
            ));
        }
        let Some(seen) = observed.get_mut(observation.kind as usize) else {
            return Err(Diagnostic::new(
                "qualification_kind",
                "qualification.observations.kind",
                "unsupported qualification observation kind",
            ));
        };
        if *seen {
            return Err(Diagnostic::new(
                "qualification_duplicate",
                "qualification.observations.kind",
                "a duplicate observation cannot replace a missing qualification case",
            ));
        }
        *seen = true;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
