mod identity;
mod issues;
mod non_claims;

use serde::{Deserialize, Serialize};

use crate::{AuthenticatedEvidenceIssue, AuthenticatedEvidenceReport};

use super::{CairnArtifactAuthReport, CAIRN_ARTIFACT_AUTH_BOUNDARY};

pub const CAIRN_ARTIFACT_AUTH_COMPATIBILITY_SCHEMA_V1: &str =
    "cairn.artifact-auth.compatibility.v1";
pub const CAIRN_ARTIFACT_AUTH_COMPATIBILITY_BOUNDARY: &str = "Compatibility evidence classifies one legacy/standalone case only; the legacy Cairn report remains authoritative, the standalone report cannot enter release readiness directly, and case parity does not admit standalone authority or prove verifier correctness, corpus completeness, lifecycle conformance, readiness, or release eligibility";

const BLAKE3_HEX_LENGTH: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CairnArtifactAuthCompatibilityDimension {
    Preimage,
    Identity,
    Decision,
    Issue,
    NonClaim,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CairnArtifactAuthCompatibilityDisposition {
    Equal,
    IntentionallyMapped,
    Blocking,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CairnArtifactAuthCompatibilityClassification {
    pub dimension: CairnArtifactAuthCompatibilityDimension,
    pub code: String,
    pub legacy: Option<String>,
    pub standalone: Option<String>,
    pub disposition: CairnArtifactAuthCompatibilityDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CairnArtifactAuthCompatibilityReport {
    pub schema_version: String,
    pub case_explained: bool,
    pub legacy_authoritative: bool,
    pub standalone_authority_admitted: bool,
    pub rollback_available: bool,
    pub classifications: Vec<CairnArtifactAuthCompatibilityClassification>,
    pub blockers: Vec<String>,
    pub boundary: String,
}

/// Compare one legacy and standalone run without transferring Cairn release authority.
///
/// Both reports must derive from the same measured `AuthenticatedEvidenceInput`. The
/// separate preimage identities make the intentional schema transition explicit.
#[must_use]
// r[impl cairn.artifact_auth.repository_extraction.cutover]
// r[impl cairn.artifact_auth.repository_extraction.cutover.negative]
// r[impl cairn.artifact_auth.repository_extraction.conformance]
pub fn compare_cairn_artifact_auth(
    legacy_preimage_blake3: &str,
    standalone_preimage_blake3: Option<&str>,
    legacy: &AuthenticatedEvidenceReport,
    standalone: Option<&CairnArtifactAuthReport>,
    standalone_blockers: &[AuthenticatedEvidenceIssue],
) -> CairnArtifactAuthCompatibilityReport {
    let mut classifications = Vec::new();
    identity::classify_preimages(
        legacy_preimage_blake3,
        standalone_preimage_blake3,
        &mut classifications,
    );
    match standalone {
        Some(report) => {
            identity::classify_identities(
                standalone_preimage_blake3,
                legacy,
                report,
                &mut classifications,
            );
            identity::classify_decisions(legacy, report, &mut classifications);
            issues::classify_issues(legacy, report, &mut classifications);
            non_claims::classify_non_claims(legacy, report, &mut classifications);
        }
        None => classify_unavailable_standalone(&mut classifications),
    }
    classify_standalone_blockers(standalone_blockers, &mut classifications);
    classifications.sort_by(|left, right| {
        left.dimension
            .cmp(&right.dimension)
            .then(left.code.cmp(&right.code))
            .then(left.legacy.cmp(&right.legacy))
            .then(left.standalone.cmp(&right.standalone))
    });
    classifications.dedup();
    let blockers = blocking_codes(&classifications);
    CairnArtifactAuthCompatibilityReport {
        schema_version: CAIRN_ARTIFACT_AUTH_COMPATIBILITY_SCHEMA_V1.to_string(),
        case_explained: blockers.is_empty(),
        legacy_authoritative: true,
        standalone_authority_admitted: false,
        rollback_available: true,
        classifications,
        blockers,
        boundary: CAIRN_ARTIFACT_AUTH_COMPATIBILITY_BOUNDARY.to_string(),
    }
}

fn classify_unavailable_standalone(output: &mut Vec<CairnArtifactAuthCompatibilityClassification>) {
    for dimension in [
        CairnArtifactAuthCompatibilityDimension::Identity,
        CairnArtifactAuthCompatibilityDimension::Decision,
        CairnArtifactAuthCompatibilityDimension::Issue,
        CairnArtifactAuthCompatibilityDimension::NonClaim,
    ] {
        push_classification(
            output,
            dimension,
            "artifact_auth.compatibility.standalone.unavailable",
            None,
            None,
            CairnArtifactAuthCompatibilityDisposition::Blocking,
        );
    }
}

fn classify_standalone_blockers(
    blockers: &[AuthenticatedEvidenceIssue],
    output: &mut Vec<CairnArtifactAuthCompatibilityClassification>,
) {
    for blocker in blockers {
        push_classification(
            output,
            CairnArtifactAuthCompatibilityDimension::Issue,
            "artifact_auth.compatibility.standalone.blocker",
            None,
            Some(&blocker.code),
            CairnArtifactAuthCompatibilityDisposition::Blocking,
        );
    }
}

fn blocking_codes(classifications: &[CairnArtifactAuthCompatibilityClassification]) -> Vec<String> {
    let mut blockers = classifications
        .iter()
        .filter(|item| item.disposition == CairnArtifactAuthCompatibilityDisposition::Blocking)
        .map(|item| item.code.clone())
        .collect::<Vec<_>>();
    blockers.sort();
    blockers.dedup();
    blockers
}

fn push_classification(
    output: &mut Vec<CairnArtifactAuthCompatibilityClassification>,
    dimension: CairnArtifactAuthCompatibilityDimension,
    code: &str,
    legacy: Option<&str>,
    standalone: Option<&str>,
    disposition: CairnArtifactAuthCompatibilityDisposition,
) {
    output.push(CairnArtifactAuthCompatibilityClassification {
        dimension,
        code: code.to_string(),
        legacy: legacy.map(ToString::to_string),
        standalone: standalone.map(ToString::to_string),
        disposition,
    });
}

fn valid_blake3(value: &str) -> bool {
    value.len() == BLAKE3_HEX_LENGTH
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
