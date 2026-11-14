use crate::{AuthenticatedEvidenceReport, AUTHENTICATED_EVIDENCE_BOUNDARY};

use super::{
    push_classification, CairnArtifactAuthCompatibilityClassification,
    CairnArtifactAuthCompatibilityDimension, CairnArtifactAuthCompatibilityDisposition,
    CairnArtifactAuthReport, CAIRN_ARTIFACT_AUTH_BOUNDARY,
};

const REQUIRED_NON_CLAIM_COUNT: usize = 11;
const REQUIRED_NON_CLAIMS: [&str; REQUIRED_NON_CLAIM_COUNT] = [
    "authentication-does-not-prove-artifact-truth",
    "authentication-does-not-prove-verifier-correctness",
    "authentication-does-not-prove-producer-authorization",
    "authentication-does-not-prove-trust-root-correctness",
    "authentication-does-not-prove-revocation-freshness",
    "authentication-does-not-prove-provenance-correctness",
    "authentication-does-not-prove-lifecycle-conformance",
    "authentication-does-not-prove-build-correctness",
    "authentication-does-not-prove-semantic-equivalence",
    "authentication-does-not-prove-deployment-safety",
    "authentication-does-not-prove-release-eligibility",
];

pub(super) fn classify_non_claims(
    legacy: &AuthenticatedEvidenceReport,
    standalone: &CairnArtifactAuthReport,
    output: &mut Vec<CairnArtifactAuthCompatibilityClassification>,
) {
    push_classification(
        output,
        CairnArtifactAuthCompatibilityDimension::NonClaim,
        "artifact_auth.compatibility.non_claim.legacy_boundary",
        Some(&legacy.boundary),
        None,
        if legacy.boundary == AUTHENTICATED_EVIDENCE_BOUNDARY {
            CairnArtifactAuthCompatibilityDisposition::Equal
        } else {
            CairnArtifactAuthCompatibilityDisposition::Blocking
        },
    );
    push_classification(
        output,
        CairnArtifactAuthCompatibilityDimension::NonClaim,
        "artifact_auth.compatibility.non_claim.adapter_boundary",
        None,
        Some(&standalone.boundary),
        if standalone.boundary == CAIRN_ARTIFACT_AUTH_BOUNDARY {
            CairnArtifactAuthCompatibilityDisposition::IntentionallyMapped
        } else {
            CairnArtifactAuthCompatibilityDisposition::Blocking
        },
    );
    classify_required_set(standalone, output);
}

fn classify_required_set(
    standalone: &CairnArtifactAuthReport,
    output: &mut Vec<CairnArtifactAuthCompatibilityClassification>,
) {
    let mut actual = standalone.non_claims.clone();
    actual.sort();
    let mut expected = REQUIRED_NON_CLAIMS
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    expected.sort();
    push_classification(
        output,
        CairnArtifactAuthCompatibilityDimension::NonClaim,
        "artifact_auth.compatibility.non_claim.required_set",
        None,
        Some(&actual.join(",")),
        if actual == expected {
            CairnArtifactAuthCompatibilityDisposition::IntentionallyMapped
        } else {
            CairnArtifactAuthCompatibilityDisposition::Blocking
        },
    );
}
