const LEGACY_PREIMAGE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const STANDALONE_PREIMAGE: &str =
    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const LEGACY_DECISION: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const STANDALONE_AUTHENTICATION: &str =
    "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
const LEGACY_KEY: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
const STANDALONE_KEY: &str = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
const POLICY_IDENTITY: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const SCOPE_IDENTITY: &str = "2222222222222222222222222222222222222222222222222222222222222222";
const DECISION_IDENTITY: &str = "3333333333333333333333333333333333333333333333333333333333333333";
const SUBJECT_IDENTITY: &str = "4444444444444444444444444444444444444444444444444444444444444444";

#[test]
// r[verify cairn.artifact_auth.repository_extraction.cutover.positive]
fn passing_dual_run_classifies_every_dimension_without_admitting_authority() {
    let legacy = legacy_report(true, Vec::new());
    let standalone = standalone_report(true, Vec::new());
    let report = super::compare_cairn_artifact_auth(
        LEGACY_PREIMAGE,
        Some(STANDALONE_PREIMAGE),
        &legacy,
        Some(&standalone),
        &[],
    );

    assert!(report.case_explained);
    assert!(report.blockers.is_empty());
    assert!(report.legacy_authoritative);
    assert!(!report.standalone_authority_admitted);
    assert!(report.rollback_available);
    let dimensions = report
        .classifications
        .iter()
        .map(|item| item.dimension)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(dimensions.len(), 5);
    assert!(report.classifications.iter().any(|item| {
        item.dimension == super::CairnArtifactAuthCompatibilityDimension::Preimage
            && item.disposition
                == super::CairnArtifactAuthCompatibilityDisposition::IntentionallyMapped
    }));
}

#[test]
// r[verify cairn.artifact_auth.repository_extraction.conformance]
fn known_tamper_issue_expansion_is_explained_while_both_paths_reject() {
    let legacy = legacy_report(
        false,
        vec![
            issue("authenticated_inputs.measured_digest_mismatch"),
            issue("authenticated_inputs.signature_parent_mismatch"),
            issue("authenticated_inputs.signature_parent_unlinked"),
            issue("authenticated_inputs.signature_unverified"),
        ],
    );
    let standalone = standalone_report(
        false,
        vec![
            issue("auth.cryptographic_failure_present"),
            issue("auth.distinct_key_threshold_unsatisfied"),
            issue("auth.signature_unverified"),
        ],
    );
    let report = super::compare_cairn_artifact_auth(
        LEGACY_PREIMAGE,
        Some(STANDALONE_PREIMAGE),
        &legacy,
        Some(&standalone),
        &[],
    );

    assert!(report.case_explained);
    assert!(report.blockers.is_empty());
    assert!(report.classifications.iter().any(|item| {
        item.legacy.as_deref() == Some("authenticated_inputs.signature_unverified")
            && item.standalone.as_deref() == Some("auth.signature_unverified")
            && item.disposition
                == super::CairnArtifactAuthCompatibilityDisposition::IntentionallyMapped
    }));
}

#[test]
// r[verify cairn.artifact_auth.repository_extraction.cutover.negative]
fn unexplained_identity_issue_non_claim_and_preimage_drift_blocks() {
    let legacy = legacy_report(true, Vec::new());
    let mut standalone = standalone_report(false, vec![issue("auth.unknown_new_failure")]);
    standalone.producer = "different-producer".to_string();
    standalone.non_claims.pop();
    let report = super::compare_cairn_artifact_auth(
        LEGACY_PREIMAGE,
        Some(LEGACY_PREIMAGE),
        &legacy,
        Some(&standalone),
        &[],
    );

    assert!(!report.case_explained);
    assert!(report
        .blockers
        .contains(&"artifact_auth.compatibility.preimage.domain_not_distinct".to_string()));
    assert!(report
        .blockers
        .contains(&"artifact_auth.compatibility.identity.producer".to_string()));
    assert!(report
        .blockers
        .contains(&"artifact_auth.compatibility.issue.mapping".to_string()));
    assert!(report
        .blockers
        .contains(&"artifact_auth.compatibility.non_claim.required_set".to_string()));

    let malformed = super::compare_cairn_artifact_auth(
        "not-a-blake3-identity",
        Some(STANDALONE_PREIMAGE),
        &legacy,
        Some(&standalone),
        &[],
    );
    assert!(malformed
        .blockers
        .contains(&"artifact_auth.compatibility.preimage.malformed".to_string()));
}

#[test]
// r[verify cairn.artifact_auth.repository_extraction.cutover.negative]
fn unrelated_generic_signature_rejections_do_not_create_false_parity() {
    let legacy = legacy_report(
        false,
        vec![issue("authenticated_inputs.signature_unverified")],
    );
    let standalone = standalone_report(
        false,
        vec![
            issue("auth.cryptographic_failure_present"),
            issue("auth.distinct_key_threshold_unsatisfied"),
            issue("auth.signature_unverified"),
        ],
    );
    let report = super::compare_cairn_artifact_auth(
        LEGACY_PREIMAGE,
        Some(STANDALONE_PREIMAGE),
        &legacy,
        Some(&standalone),
        &[],
    );

    assert!(!report.case_explained);
    assert!(report
        .blockers
        .contains(&"artifact_auth.compatibility.decision.outcome".to_string()));
}

#[test]
// r[verify cairn.artifact_auth.repository_extraction.cutover.negative]
fn missing_standalone_run_preserves_legacy_rollback_and_reports_blocker() {
    let legacy = legacy_report(true, Vec::new());
    let blocker = issue("artifact_auth.dual_run.signature_missing");
    let report =
        super::compare_cairn_artifact_auth(LEGACY_PREIMAGE, None, &legacy, None, &[blocker]);

    assert!(!report.case_explained);
    assert!(report.legacy_authoritative);
    assert!(!report.standalone_authority_admitted);
    assert!(report.rollback_available);
    assert!(report
        .blockers
        .contains(&"artifact_auth.compatibility.standalone.unavailable".to_string()));
    assert!(report
        .blockers
        .contains(&"artifact_auth.compatibility.standalone.blocker".to_string()));
}

fn legacy_report(
    passed: bool,
    issues: Vec<crate::AuthenticatedEvidenceIssue>,
) -> crate::AuthenticatedEvidenceReport {
    crate::AuthenticatedEvidenceReport {
        evidence_kind: "stack-provenance".to_string(),
        passed,
        authentication_hash: passed.then(|| LEGACY_DECISION.to_string()),
        measured_subject_hash: SUBJECT_IDENTITY.to_string(),
        producer: "cairn".to_string(),
        key_id: "release-key".to_string(),
        issues,
        boundary: crate::AUTHENTICATED_EVIDENCE_BOUNDARY.to_string(),
    }
}

fn standalone_report(
    passed: bool,
    issues: Vec<crate::AuthenticatedEvidenceIssue>,
) -> super::CairnArtifactAuthReport {
    super::CairnArtifactAuthReport {
        evidence_kind: "stack-provenance".to_string(),
        decision_passed: passed,
        authentication_hash: passed.then(|| STANDALONE_AUTHENTICATION.to_string()),
        measured_subject_hash: SUBJECT_IDENTITY.to_string(),
        producer: "cairn".to_string(),
        key_id: "release-key".to_string(),
        issues,
        boundary: super::CAIRN_ARTIFACT_AUTH_BOUNDARY.to_string(),
        standalone_statement_blake3: Some(STANDALONE_PREIMAGE.to_string()),
        standalone_policy_blake3: POLICY_IDENTITY.to_string(),
        standalone_scope_blake3: SCOPE_IDENTITY.to_string(),
        standalone_decision_blake3: DECISION_IDENTITY.to_string(),
        standalone_verified_key_blake3: passed
            .then(|| STANDALONE_KEY.to_string())
            .into_iter()
            .collect(),
        legacy_public_key_blake3: LEGACY_KEY.to_string(),
        standalone_public_key_blake3: STANDALONE_KEY.to_string(),
        non_claims: required_non_claims(),
    }
}

fn issue(code: &str) -> crate::AuthenticatedEvidenceIssue {
    crate::AuthenticatedEvidenceIssue {
        code: code.to_string(),
        field_path: "test.field".to_string(),
        expected: "expected".to_string(),
        actual: "actual".to_string(),
    }
}

fn required_non_claims() -> Vec<String> {
    [
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
    ]
    .iter()
    .map(ToString::to_string)
    .collect()
}
