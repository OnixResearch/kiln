//! Frozen formal-proof readiness fixture validation for Cairn release evidence.
//!
//! The core evaluates parsed fixture values only. It does not read files,
//! inspect environments, invoke Valence, invoke Octet, run proof verifiers,
//! contact sibling repositories, or decide release eligibility outside the
//! configured readiness policy.
//!
//! r[impl cairn.release_evidence.formal_proof_readiness_fixture.contract]
//! r[impl cairn.release_evidence.formal_proof_readiness_fixture.validation]
//! r[verify cairn.release_evidence.formal_proof_readiness_fixture.docs]

use serde_json::json;

const FORMAL_PROOF_READINESS_SCHEMA_V1: &str = "cairn.formal-proof-readiness-fixture.v1";
const UPSTREAM_PROOF_STATUS_PASSED: &str = "passed";
const EMPTY_FIELD: &str = "";

/// Visible supported-claim boundary for formal-proof readiness fixture reports.
pub const FORMAL_PROOF_READINESS_BOUNDARY: &str = "formal-proof readiness fixtures prove Cairn receipt handling under policy only; they do not prove upstream source correctness, verifier soundness, Valence correctness, Octet correctness, whole-program correctness, semantic equivalence, or release eligibility outside the configured readiness policy";

const OVERCLAIM_MARKERS: &[&str] = &[
    "proves upstream source correctness",
    "proves verifier soundness",
    "proves Valence correctness",
    "proves Octet correctness",
    "proves whole-program correctness",
    "proves semantic equivalence",
    "proves release eligibility",
];

/// Parsed fixture input for one frozen formal-proof readiness case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormalProofReadinessFixture {
    pub schema_version: String,
    pub valence_input_hash: String,
    pub observed_valence_input_hash: String,
    pub valence_graph_digest: String,
    pub observed_valence_graph_digest: String,
    pub sidecar_digest: String,
    pub observed_sidecar_digest: String,
    pub upstream_octet_proof_identity: String,
    pub policy_hash: String,
    pub input_hash: String,
    pub readiness_receipt_hash: String,
    pub property_count: usize,
    pub recorded_only_count: usize,
    pub boundary_count: usize,
    pub unknown_count: usize,
    pub verified_count: usize,
    pub accepted_count: usize,
    pub blocked_count: usize,
    pub upstream_proof_status: String,
    pub non_claims: Vec<String>,
    pub supported_claim: String,
}

/// Deterministic validation issue for formal-proof readiness fixtures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormalProofReadinessIssue {
    pub code: &'static str,
    pub field: &'static str,
}

/// Report emitted by the pure fixture validator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormalProofReadinessReport {
    pub valid: bool,
    pub issues: Vec<FormalProofReadinessIssue>,
    pub property_count: usize,
    pub recorded_only_count: usize,
    pub boundary_count: usize,
    pub unknown_count: usize,
    pub verified_count: usize,
    pub accepted_count: usize,
    pub blocked_count: usize,
    pub supported_claim: String,
}

impl FormalProofReadinessReport {
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        json!({
            "valid": self.valid,
            "issues": self.issues.iter().map(FormalProofReadinessIssue::to_json).collect::<Vec<_>>(),
            "verification_role_summary": {
                "property_count": self.property_count,
                "recorded_only_count": self.recorded_only_count,
                "boundary_count": self.boundary_count,
                "unknown_count": self.unknown_count,
                "verified_count": self.verified_count,
                "accepted_count": self.accepted_count,
                "blocked_count": self.blocked_count,
            },
            "supported_claim": self.supported_claim,
        })
    }
}

impl FormalProofReadinessIssue {
    #[must_use]
    pub const fn new(code: &'static str, field: &'static str) -> Self {
        Self { code, field }
    }

    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        json!({"code": self.code, "field": self.field})
    }
}

/// Validate a frozen formal-proof readiness fixture without side effects.
// r[impl cairn.release_evidence.formal_proof_readiness_fixture.validation.core]
#[must_use]
pub fn validate_formal_proof_readiness_fixture(
    fixture: &FormalProofReadinessFixture,
) -> FormalProofReadinessReport {
    let mut issues = Vec::new();

    push_if(
        &mut issues,
        fixture.schema_version != FORMAL_PROOF_READINESS_SCHEMA_V1,
        "formal_proof_readiness.schema_version_mismatch",
        "schema_version",
    );
    validate_hash_pair::<super::nominal_identity::ValenceDigestRole>(
        &mut issues,
        "valence_input_hash",
        &fixture.valence_input_hash,
        &fixture.observed_valence_input_hash,
    );
    validate_hash_pair::<super::nominal_identity::GraphDigestRole>(
        &mut issues,
        "valence_graph_digest",
        &fixture.valence_graph_digest,
        &fixture.observed_valence_graph_digest,
    );
    validate_hash_pair::<super::nominal_identity::SidecarDigestRole>(
        &mut issues,
        "sidecar_digest",
        &fixture.sidecar_digest,
        &fixture.observed_sidecar_digest,
    );
    validate_required_hash::<super::nominal_identity::ArtifactDigestRole>(
        &mut issues,
        "upstream_octet_proof_identity",
        &fixture.upstream_octet_proof_identity,
    );
    validate_required_hash::<super::nominal_identity::PolicyDigestRole>(
        &mut issues,
        "policy_hash",
        &fixture.policy_hash,
    );
    validate_required_hash::<super::nominal_identity::InputDigestRole>(
        &mut issues,
        "input_hash",
        &fixture.input_hash,
    );
    validate_required_hash::<super::nominal_identity::ReceiptDigestRole>(
        &mut issues,
        "readiness_receipt_hash",
        &fixture.readiness_receipt_hash,
    );
    validate_role_counts(fixture, &mut issues);
    validate_upstream_status(fixture, &mut issues);
    validate_non_claims(fixture, &mut issues);

    issues.sort_by(|left, right| {
        left.code
            .cmp(right.code)
            .then_with(|| left.field.cmp(right.field))
    });
    issues.dedup();
    FormalProofReadinessReport {
        valid: issues.is_empty(),
        issues,
        property_count: fixture.property_count,
        recorded_only_count: fixture.recorded_only_count,
        boundary_count: fixture.boundary_count,
        unknown_count: fixture.unknown_count,
        verified_count: fixture.verified_count,
        accepted_count: fixture.accepted_count,
        blocked_count: fixture.blocked_count,
        supported_claim: FORMAL_PROOF_READINESS_BOUNDARY.to_owned(),
    }
}

fn push_if(
    issues: &mut Vec<FormalProofReadinessIssue>,
    condition: bool,
    code: &'static str,
    field: &'static str,
) {
    if condition {
        issues.push(FormalProofReadinessIssue::new(code, field));
    }
}

fn validate_hash_pair<Role: super::nominal_identity::DigestRoleMarker>(
    issues: &mut Vec<FormalProofReadinessIssue>,
    field: &'static str,
    expected: &str,
    observed: &str,
) {
    let expected_digest = admit_digest::<Role>(issues, field, expected);
    let observed_digest = admit_digest::<Role>(issues, field, observed);
    let stale = matches!(
        (expected_digest, observed_digest),
        (Some(expected), Some(observed)) if expected.as_hex() != observed.as_hex()
    );
    push_if(issues, stale, "formal_proof_readiness.stale_input", field);
}

fn validate_required_hash<Role: super::nominal_identity::DigestRoleMarker>(
    issues: &mut Vec<FormalProofReadinessIssue>,
    field: &'static str,
    value: &str,
) {
    let _ = admit_digest::<Role>(issues, field, value);
}

fn admit_digest<Role: super::nominal_identity::DigestRoleMarker>(
    issues: &mut Vec<FormalProofReadinessIssue>,
    field: &'static str,
    value: &str,
) -> Option<super::nominal_identity::Blake3Digest<Role>> {
    if value == EMPTY_FIELD {
        issues.push(FormalProofReadinessIssue::new(
            "formal_proof_readiness.hash_missing",
            field,
        ));
        return None;
    }
    match super::nominal_identity::Blake3Digest::<Role>::parse_compatible(
        value,
        super::nominal_identity::RAW_BLAKE3_PROFILE,
        field,
    ) {
        Ok(digest) => Some(digest),
        Err(_) => {
            issues.push(FormalProofReadinessIssue::new(
                "formal_proof_readiness.hash_malformed",
                field,
            ));
            None
        }
    }
}

fn validate_role_counts(
    fixture: &FormalProofReadinessFixture,
    issues: &mut Vec<FormalProofReadinessIssue>,
) {
    let total = fixture.property_count
        + fixture.recorded_only_count
        + fixture.boundary_count
        + fixture.unknown_count;
    push_if(
        issues,
        fixture.verified_count != fixture.property_count,
        "formal_proof_readiness.verification_role_promotion",
        "verified_count",
    );
    push_if(
        issues,
        fixture.accepted_count != fixture.verified_count,
        "formal_proof_readiness.accepted_count_mismatch",
        "accepted_count",
    );
    push_if(
        issues,
        fixture.blocked_count + fixture.accepted_count != total,
        "formal_proof_readiness.blocked_count_mismatch",
        "blocked_count",
    );
}

fn validate_upstream_status(
    fixture: &FormalProofReadinessFixture,
    issues: &mut Vec<FormalProofReadinessIssue>,
) {
    push_if(
        issues,
        fixture.property_count > 0 && fixture.upstream_proof_status != UPSTREAM_PROOF_STATUS_PASSED,
        "formal_proof_readiness.upstream_proof_not_passed",
        "upstream_proof_status",
    );
}

fn validate_non_claims(
    fixture: &FormalProofReadinessFixture,
    issues: &mut Vec<FormalProofReadinessIssue>,
) {
    if fixture.non_claims.is_empty() {
        issues.push(FormalProofReadinessIssue::new(
            "formal_proof_readiness.non_claims_missing",
            "non_claims",
        ));
        return;
    }
    let non_claim_text = fixture.non_claims.join(" ");
    push_if(
        issues,
        !non_claim_text.contains(FORMAL_PROOF_READINESS_BOUNDARY),
        "formal_proof_readiness.non_claims_weakened",
        "non_claims",
    );
    let combined_claim_text = format!("{} {}", fixture.supported_claim, non_claim_text);
    push_if(
        issues,
        OVERCLAIM_MARKERS
            .iter()
            .any(|marker| combined_claim_text.contains(marker)),
        "formal_proof_readiness.overclaim",
        "supported_claim",
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const HASH_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const HASH_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    const HASH_D: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
    const HASH_E: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    const HASH_F: &str = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
    const HASH_0: &str = "0000000000000000000000000000000000000000000000000000000000000000";
    const POSITIVE_PROPERTY_COUNT: usize = 1;
    const POSITIVE_RECORDED_ONLY_COUNT: usize = 2;
    const POSITIVE_BOUNDARY_COUNT: usize = 1;
    const POSITIVE_UNKNOWN_COUNT: usize = 0;
    const POSITIVE_VERIFIED_COUNT: usize = 1;
    const POSITIVE_ACCEPTED_COUNT: usize = 1;
    const POSITIVE_BLOCKED_COUNT: usize = 3;

    fn positive_fixture() -> FormalProofReadinessFixture {
        FormalProofReadinessFixture {
            schema_version: FORMAL_PROOF_READINESS_SCHEMA_V1.to_owned(),
            valence_input_hash: HASH_A.to_owned(),
            observed_valence_input_hash: HASH_A.to_owned(),
            valence_graph_digest: HASH_B.to_owned(),
            observed_valence_graph_digest: HASH_B.to_owned(),
            sidecar_digest: HASH_C.to_owned(),
            observed_sidecar_digest: HASH_C.to_owned(),
            upstream_octet_proof_identity: HASH_D.to_owned(),
            policy_hash: HASH_E.to_owned(),
            input_hash: HASH_F.to_owned(),
            readiness_receipt_hash: HASH_0.to_owned(),
            property_count: POSITIVE_PROPERTY_COUNT,
            recorded_only_count: POSITIVE_RECORDED_ONLY_COUNT,
            boundary_count: POSITIVE_BOUNDARY_COUNT,
            unknown_count: POSITIVE_UNKNOWN_COUNT,
            verified_count: POSITIVE_VERIFIED_COUNT,
            accepted_count: POSITIVE_ACCEPTED_COUNT,
            blocked_count: POSITIVE_BLOCKED_COUNT,
            upstream_proof_status: UPSTREAM_PROOF_STATUS_PASSED.to_owned(),
            non_claims: vec![FORMAL_PROOF_READINESS_BOUNDARY.to_owned()],
            supported_claim: FORMAL_PROOF_READINESS_BOUNDARY.to_owned(),
        }
    }

    fn assert_issue(report: &FormalProofReadinessReport, code: &str) {
        assert!(
            report.issues.iter().any(|issue| issue.code == code),
            "expected issue {code}, got {:?}",
            report.issues
        );
    }

    #[test]
    fn formal_proof_readiness_fixture_accepts_property_with_recorded_context() {
        // r[verify cairn.release_evidence.formal_proof_readiness_fixture.positive]
        // r[verify cairn.release_evidence.formal_proof_readiness_fixture.positive.property_count]
        let report = validate_formal_proof_readiness_fixture(&positive_fixture());

        assert!(report.valid, "unexpected issues: {:?}", report.issues);
        assert!(report.issues.is_empty());
        assert_eq!(report.property_count, POSITIVE_PROPERTY_COUNT);
        assert_eq!(report.recorded_only_count, POSITIVE_RECORDED_ONLY_COUNT);
        assert_eq!(report.boundary_count, POSITIVE_BOUNDARY_COUNT);
        assert_eq!(report.verified_count, POSITIVE_VERIFIED_COUNT);
        assert_eq!(report.accepted_count, POSITIVE_ACCEPTED_COUNT);
        assert_eq!(report.blocked_count, POSITIVE_BLOCKED_COUNT);
    }

    #[test]
    fn formal_proof_readiness_fixture_json_exposes_boundary_and_counts() {
        // r[verify cairn.release_evidence.formal_proof_readiness_fixture.contract.fields]
        // r[verify cairn.release_evidence.formal_proof_readiness_fixture.docs.non_claims]
        let report = validate_formal_proof_readiness_fixture(&positive_fixture());
        let json = report.to_json();

        assert_eq!(json["valid"], json!(true));
        assert_eq!(
            json["verification_role_summary"]["property_count"],
            json!(POSITIVE_PROPERTY_COUNT)
        );
        assert_eq!(
            json["verification_role_summary"]["recorded_only_count"],
            json!(POSITIVE_RECORDED_ONLY_COUNT)
        );
        assert_eq!(
            json["verification_role_summary"]["accepted_count"],
            json!(POSITIVE_ACCEPTED_COUNT)
        );
        assert_eq!(
            json["verification_role_summary"]["blocked_count"],
            json!(POSITIVE_BLOCKED_COUNT)
        );
        assert_eq!(
            json["supported_claim"],
            json!(FORMAL_PROOF_READINESS_BOUNDARY)
        );
    }

    #[test]
    fn formal_proof_readiness_fixture_rejects_role_promotion_and_failed_proof() {
        // r[verify cairn.release_evidence.formal_proof_readiness_fixture.negative]
        // r[verify cairn.release_evidence.formal_proof_readiness_fixture.negative.recorded_only]
        let mut fixture = positive_fixture();
        fixture.verified_count = fixture.property_count + fixture.recorded_only_count;
        fixture.accepted_count = fixture.verified_count;
        fixture.blocked_count = fixture.boundary_count;
        fixture.upstream_proof_status = "failed".to_owned();
        let report = validate_formal_proof_readiness_fixture(&fixture);

        assert!(!report.valid);
        assert_issue(
            &report,
            "formal_proof_readiness.verification_role_promotion",
        );
        assert_issue(&report, "formal_proof_readiness.upstream_proof_not_passed");
    }

    #[test]
    fn formal_proof_readiness_fixture_rejects_stale_or_missing_hashes() {
        // r[verify cairn.release_evidence.formal_proof_readiness_fixture.negative.stale]
        let mut fixture = positive_fixture();
        fixture.observed_valence_input_hash = HASH_B.to_owned();
        fixture.observed_sidecar_digest = EMPTY_FIELD.to_owned();
        fixture.policy_hash = "not-a-blake3".to_owned();
        let report = validate_formal_proof_readiness_fixture(&fixture);

        assert!(!report.valid);
        assert_issue(&report, "formal_proof_readiness.stale_input");
        assert_issue(&report, "formal_proof_readiness.hash_missing");
        assert_issue(&report, "formal_proof_readiness.hash_malformed");
    }

    #[test]
    fn formal_proof_readiness_fixture_rejects_missing_nonclaims_and_overclaims() {
        // r[verify cairn.release_evidence.formal_proof_readiness_fixture.negative]
        let mut missing = positive_fixture();
        missing.non_claims = Vec::new();
        let missing_report = validate_formal_proof_readiness_fixture(&missing);
        assert!(!missing_report.valid);
        assert_issue(&missing_report, "formal_proof_readiness.non_claims_missing");

        let mut overclaim = positive_fixture();
        overclaim.supported_claim = "this receipt proves release eligibility".to_owned();
        let overclaim_report = validate_formal_proof_readiness_fixture(&overclaim);
        assert!(!overclaim_report.valid);
        assert_issue(&overclaim_report, "formal_proof_readiness.overclaim");
    }

    #[test]
    fn formal_proof_readiness_fixture_rejects_count_inconsistency() {
        // r[verify cairn.release_evidence.formal_proof_readiness_fixture.final_validation]
        // r[verify cairn.release_evidence.formal_proof_readiness_fixture.final_validation.fixtures]
        let mut fixture = positive_fixture();
        fixture.accepted_count = POSITIVE_ACCEPTED_COUNT + POSITIVE_BOUNDARY_COUNT;
        fixture.blocked_count = POSITIVE_BLOCKED_COUNT;
        let report = validate_formal_proof_readiness_fixture(&fixture);

        assert!(!report.valid);
        assert_issue(&report, "formal_proof_readiness.accepted_count_mismatch");
        assert_issue(&report, "formal_proof_readiness.blocked_count_mismatch");
    }
}
