use serde_json::{json, Value};

use super::nominal_identity::{
    ArtifactDigestRole, BinaryDigestRole, Blake3Digest, DigestRoleMarker, GraphDigestRole,
    PolicyDigestRole, ReceiptDigestRole, SidecarDigestRole, RAW_BLAKE3_PROFILE,
};

pub const STACK_PROVENANCE_MODE_DISABLED: &str = "disabled";
pub const STACK_PROVENANCE_MODE_OPTIONAL: &str = "optional";
pub const STACK_PROVENANCE_MODE_REQUIRED: &str = "required";
pub const STACK_PROVENANCE_CLAIM_SCOPE_LIFECYCLE: &str =
    "stack_provenance.v1.lifecycle_policy_conformance";
pub const STACK_PROVENANCE_BOUNDARY: &str = "Cairn consumed Valence and Mantle stack provenance receipts for lifecycle/release policy conformance only; it did not prove source-code correctness, Valence verifier soundness, Mantle build correctness, or Trellis/Octet proof validity";

const BLAKE3_HEX_LENGTH: usize = 64;
const DISPOSITION_DISABLED: &str = "disabled";
const DISPOSITION_ABSENT: &str = "absent";
const DISPOSITION_PRESENT: &str = "present";
const DISPOSITION_FAILED: &str = "failed";
const STACK_PROVENANCE_CHECK_NAME: &str = "stack_provenance";
const GRAPH_U64_FIELDS: &[&str] = &[
    "artifact_count",
    "edge_count",
    "manual_review_node_count",
    "unresolved_link_count",
    "failed_link_count",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackProvenancePolicy {
    pub mode: String,
    pub allowed_claim_scopes: Vec<String>,
    pub accepted_valence_policy_hashes: Vec<String>,
    pub required_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackProvenanceEvidence {
    pub valence_receipt: Option<Value>,
    pub graph_summary: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackProvenanceIssue {
    pub code: String,
    pub field_path: String,
    pub expected: String,
    pub actual: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackProvenanceReadiness {
    pub mode: String,
    pub required: bool,
    pub passed: bool,
    pub skipped: bool,
    pub disposition: String,
    pub issues: Vec<StackProvenanceIssue>,
    pub valence_receipt_hash: Option<String>,
    pub claim_scope: Option<String>,
    pub graph_summary: Option<Value>,
    pub boundary: String,
}

struct StackProvenanceReadinessParts {
    passed: bool,
    skipped: bool,
    disposition: &'static str,
    issues: Vec<StackProvenanceIssue>,
    valence_receipt_hash: Option<String>,
    claim_scope: Option<String>,
    graph_summary: Option<Value>,
}

impl StackProvenanceReadiness {
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "name": STACK_PROVENANCE_CHECK_NAME,
            "mode": self.mode,
            "required": self.required,
            "passed": self.passed,
            "skipped": self.skipped,
            "disposition": self.disposition,
            "issues": self.issues.iter().map(StackProvenanceIssue::to_json).collect::<Vec<_>>(),
            "valence_receipt_hash": self.valence_receipt_hash,
            "claim_scope": self.claim_scope,
            "graph_summary": self.graph_summary,
            "boundary": self.boundary,
        })
    }
}

impl StackProvenanceIssue {
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "code": self.code,
            "field_path": self.field_path,
            "expected": self.expected,
            "actual": self.actual,
        })
    }
}

#[must_use]
pub fn evaluate_stack_provenance(
    policy: &StackProvenancePolicy,
    evidence: &StackProvenanceEvidence,
) -> StackProvenanceReadiness {
    // r[impl cairn.stack_provenance_gates.policy]
    // r[impl cairn.stack_provenance_gates.receipt_validation]
    // r[impl cairn.stack_provenance_gates.boundary]
    // r[impl cairn.stack_provenance_gates.graph_summary]
    if policy.mode == STACK_PROVENANCE_MODE_DISABLED {
        return stack_provenance_readiness(
            policy,
            StackProvenanceReadinessParts {
                passed: true,
                skipped: true,
                disposition: DISPOSITION_DISABLED,
                issues: Vec::new(),
                valence_receipt_hash: None,
                claim_scope: None,
                graph_summary: None,
            },
        );
    }

    let Some(receipt) = evidence.valence_receipt.as_ref() else {
        if policy.mode == STACK_PROVENANCE_MODE_OPTIONAL {
            return stack_provenance_readiness(
                policy,
                StackProvenanceReadinessParts {
                    passed: true,
                    skipped: true,
                    disposition: DISPOSITION_ABSENT,
                    issues: Vec::new(),
                    valence_receipt_hash: None,
                    claim_scope: None,
                    graph_summary: None,
                },
            );
        }
        return stack_provenance_readiness(
            policy,
            StackProvenanceReadinessParts {
                passed: false,
                skipped: false,
                disposition: DISPOSITION_FAILED,
                issues: vec![stack_issue(
                    "stack_provenance.valence_receipt_missing",
                    "valence_receipt",
                    "Valence stack-provenance verification receipt",
                    "missing",
                )],
                valence_receipt_hash: None,
                claim_scope: None,
                graph_summary: None,
            },
        );
    };

    let mut issues = Vec::new();
    let valence_receipt_hash = receipt_string(receipt, "receipt_hash");
    let claim_scope = receipt
        .get("mantle_linkage")
        .and_then(|linkage| linkage.get("claim_scope"))
        .and_then(Value::as_str)
        .map(str::to_string);

    validate_valence_receipt(policy, receipt, &mut issues);
    validate_mantle_linkage(policy, receipt, &mut issues);
    validate_artifact_hashes(receipt, &mut issues);
    validate_verification_role_summary(receipt, &mut issues);
    validate_boundary(policy, receipt, &mut issues);
    let graph_summary = validate_graph_summary(evidence.graph_summary.as_ref(), &mut issues);
    issues.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then(left.field_path.cmp(&right.field_path))
    });
    let passed = issues.is_empty();
    stack_provenance_readiness(
        policy,
        StackProvenanceReadinessParts {
            passed,
            skipped: false,
            disposition: if passed {
                DISPOSITION_PRESENT
            } else {
                DISPOSITION_FAILED
            },
            issues,
            valence_receipt_hash,
            claim_scope,
            graph_summary,
        },
    )
}

fn stack_provenance_readiness(
    policy: &StackProvenancePolicy,
    parts: StackProvenanceReadinessParts,
) -> StackProvenanceReadiness {
    StackProvenanceReadiness {
        mode: policy.mode.clone(),
        required: policy.mode == STACK_PROVENANCE_MODE_REQUIRED,
        passed: parts.passed,
        skipped: parts.skipped,
        disposition: parts.disposition.to_string(),
        issues: parts.issues,
        valence_receipt_hash: parts.valence_receipt_hash,
        claim_scope: parts.claim_scope,
        graph_summary: parts.graph_summary,
        boundary: policy.required_boundary.clone(),
    }
}

fn validate_valence_receipt(
    policy: &StackProvenancePolicy,
    receipt: &Value,
    issues: &mut Vec<StackProvenanceIssue>,
) {
    if receipt.get("valid").and_then(Value::as_bool) != Some(true)
        || !matches!(
            receipt.get("verdict").and_then(Value::as_str),
            Some("pass" | "PASS")
        )
    {
        issues.push(stack_issue(
            "stack_provenance.valence_verdict_failed",
            "valence_receipt.verdict",
            "passing Valence verification receipt",
            "failed",
        ));
    }
    match receipt_string(receipt, "receipt_hash") {
        Some(hash) if admit_digest::<ReceiptDigestRole>(&hash).is_some() => {}
        _ => issues.push(stack_issue(
            "stack_provenance.valence_receipt_hash_invalid",
            "valence_receipt.receipt_hash",
            "BLAKE3 receipt hash",
            "invalid",
        )),
    }
    let policy_hash = receipt_string(receipt, "valence_policy_hash").unwrap_or_default();
    if policy.accepted_valence_policy_hashes.is_empty() {
        return;
    }
    let observed_policy = admit_digest::<PolicyDigestRole>(&policy_hash);
    let policy_is_accepted = observed_policy.as_ref().is_some_and(|observed| {
        policy
            .accepted_valence_policy_hashes
            .iter()
            .any(|accepted| {
                admit_digest::<PolicyDigestRole>(accepted)
                    .is_some_and(|candidate| candidate.as_hex() == observed.as_hex())
            })
    });
    if !policy_is_accepted {
        issues.push(stack_issue(
            "stack_provenance.valence_policy_stale",
            "valence_receipt.valence_policy_hash",
            "accepted Valence policy hash",
            if policy_hash.is_empty() {
                "missing"
            } else {
                "stale"
            },
        ));
    }
}

fn validate_mantle_linkage(
    policy: &StackProvenancePolicy,
    receipt: &Value,
    issues: &mut Vec<StackProvenanceIssue>,
) {
    let Some(linkage) = receipt.get("mantle_linkage") else {
        issues.push(stack_issue(
            "stack_provenance.mantle_linkage_missing",
            "mantle_linkage",
            "Mantle sidecar linkage metadata",
            "missing",
        ));
        return;
    };
    let claim_scope = linkage
        .get("claim_scope")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !policy
        .allowed_claim_scopes
        .iter()
        .any(|allowed| allowed == claim_scope)
    {
        issues.push(stack_issue(
            "stack_provenance.claim_scope_unsupported",
            "mantle_linkage.claim_scope",
            "policy-owned allowed claim scope",
            if claim_scope.is_empty() {
                "missing"
            } else {
                "unsupported"
            },
        ));
    }
    validate_linkage_digest::<SidecarDigestRole>(linkage, "sidecar_digest", issues);
    validate_linkage_digest::<BinaryDigestRole>(linkage, "binary_identity_hash", issues);
}

fn validate_artifact_hashes(receipt: &Value, issues: &mut Vec<StackProvenanceIssue>) {
    let Some(artifacts) = receipt.get("artifacts").and_then(Value::as_array) else {
        issues.push(stack_issue(
            "stack_provenance.artifacts_missing",
            "artifacts",
            "artifact hash list",
            "missing",
        ));
        return;
    };
    if artifacts.is_empty() {
        issues.push(stack_issue(
            "stack_provenance.artifacts_empty",
            "artifacts",
            "non-empty artifact hash list",
            "empty",
        ));
    }
    for artifact in artifacts {
        let declared = artifact
            .get("declared_hash")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let observed = artifact
            .get("observed_hash")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let declared_digest = admit_digest::<ArtifactDigestRole>(declared);
        let observed_digest = admit_digest::<ArtifactDigestRole>(observed);
        let matches = matches!(
            (declared_digest, observed_digest),
            (Some(declared), Some(observed)) if declared.as_hex() == observed.as_hex()
        );
        if !matches {
            issues.push(stack_issue(
                "stack_provenance.artifact_digest_mismatch",
                artifact
                    .get("path")
                    .and_then(Value::as_str)
                    .unwrap_or("artifacts"),
                "matching declared and observed BLAKE3 artifact hashes",
                "mismatch",
            ));
        }
    }
}

fn validate_verification_role_summary(receipt: &Value, issues: &mut Vec<StackProvenanceIssue>) {
    let Some(summary) = receipt.get("verification_role_summary") else {
        issues.push(stack_issue(
            "stack_provenance.verification_role_summary_missing",
            "verification_role_summary",
            "verification role summary",
            "missing",
        ));
        return;
    };
    let property_count = summary.get("property_count").and_then(Value::as_u64);
    let verified_count = summary.get("verified_count").and_then(Value::as_u64);
    if property_count.is_none() || verified_count.is_none() || property_count != verified_count {
        issues.push(stack_issue(
            "stack_provenance.manual_review_counted_verified",
            "verification_role_summary.verified_count",
            "verified_count equal to property_count",
            "manual-review-or-recorded-only-counted",
        ));
    }
}

fn validate_boundary(
    policy: &StackProvenancePolicy,
    receipt: &Value,
    issues: &mut Vec<StackProvenanceIssue>,
) {
    let boundary = receipt
        .get("non_claim_boundary")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if boundary != policy.required_boundary {
        issues.push(stack_issue(
            "stack_provenance.non_claim_boundary_weakened",
            "non_claim_boundary",
            "policy-owned Cairn non-claim boundary",
            if boundary.is_empty() {
                "missing"
            } else {
                "weakened"
            },
        ));
    }
}

fn validate_graph_summary(
    graph: Option<&Value>,
    issues: &mut Vec<StackProvenanceIssue>,
) -> Option<Value> {
    let graph = graph?;
    let mut valid = true;
    for field in GRAPH_U64_FIELDS {
        if graph.get(*field).and_then(Value::as_u64).is_none() {
            issues.push(stack_issue(
                "stack_provenance.graph_summary_malformed",
                &format!("graph_summary.{field}"),
                "non-negative integer graph summary field",
                "malformed",
            ));
            valid = false;
        }
    }
    let report_hash = graph
        .get("report_hash")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if admit_digest::<GraphDigestRole>(report_hash).is_none() {
        issues.push(stack_issue(
            "stack_provenance.graph_summary_malformed",
            "graph_summary.report_hash",
            "BLAKE3 report hash",
            if report_hash.is_empty() {
                "missing"
            } else {
                "malformed"
            },
        ));
        valid = false;
    }
    valid.then(|| graph.clone())
}

fn validate_linkage_digest<Role: DigestRoleMarker>(
    linkage: &Value,
    field: &str,
    issues: &mut Vec<StackProvenanceIssue>,
) {
    let value = linkage
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default();
    if admit_digest::<Role>(value).is_some() {
        return;
    }
    issues.push(stack_issue(
        "stack_provenance.linkage_hash_invalid",
        &format!("mantle_linkage.{field}"),
        "BLAKE3 hash",
        if value.is_empty() {
            "missing"
        } else {
            "invalid"
        },
    ));
}

fn admit_digest<Role: DigestRoleMarker>(value: &str) -> Option<Blake3Digest<Role>> {
    Blake3Digest::<Role>::parse_compatible(value, RAW_BLAKE3_PROFILE, "stack_provenance.digest")
        .ok()
}

fn stack_issue(code: &str, field_path: &str, expected: &str, actual: &str) -> StackProvenanceIssue {
    StackProvenanceIssue {
        code: code.to_string(),
        field_path: field_path.to_string(),
        expected: expected.to_string(),
        actual: actual.to_string(),
    }
}

fn receipt_string(receipt: &Value, field: &str) -> Option<String> {
    receipt
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
}

#[must_use]
pub fn is_stack_provenance_mode(mode: &str) -> bool {
    matches!(
        mode,
        STACK_PROVENANCE_MODE_DISABLED
            | STACK_PROVENANCE_MODE_OPTIONAL
            | STACK_PROVENANCE_MODE_REQUIRED
    )
}

#[must_use]
pub fn is_blake3_hex(value: &str) -> bool {
    value.len() == BLAKE3_HEX_LENGTH && value.chars().all(|character| character.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    // r[impl cairn.stack_provenance_gates.fixture_matrix]
    use super::*;

    const ACCEPTED_POLICY_HASH: &str =
        "93993e99e87206d232b64efcc8631611aad3b5b0c4b81855e7e1a0cdcc391cff";
    const COMPLETE_RECEIPT: &str =
        include_str!("../../../../fixtures/stack-provenance/positive/required-complete.json");
    const GRAPH_SUMMARY: &str =
        include_str!("../../../../fixtures/stack-provenance/positive/graph-summary.json");
    const FAILED_VALENCE: &str =
        include_str!("../../../../fixtures/stack-provenance/negative/failed-valence.json");
    const STALE_POLICY: &str =
        include_str!("../../../../fixtures/stack-provenance/negative/stale-policy.json");
    const DIGEST_MISMATCH: &str =
        include_str!("../../../../fixtures/stack-provenance/negative/digest-mismatch.json");
    const UNSUPPORTED_CLAIM_SCOPE: &str =
        include_str!("../../../../fixtures/stack-provenance/negative/unsupported-claim-scope.json");
    const MISSING_MANTLE_LINKAGE: &str =
        include_str!("../../../../fixtures/stack-provenance/negative/missing-mantle-linkage.json");
    const MANUAL_REVIEW_VERIFIED: &str = include_str!(
        "../../../../fixtures/stack-provenance/negative/manual-review-counted-verified.json"
    );
    const WEAKENED_BOUNDARY: &str =
        include_str!("../../../../fixtures/stack-provenance/negative/weakened-boundary.json");
    const MALFORMED_GRAPH: &str =
        include_str!("../../../../fixtures/stack-provenance/negative/malformed-graph-summary.json");

    fn policy(mode: &str) -> StackProvenancePolicy {
        StackProvenancePolicy {
            mode: mode.to_string(),
            allowed_claim_scopes: vec![STACK_PROVENANCE_CLAIM_SCOPE_LIFECYCLE.to_string()],
            accepted_valence_policy_hashes: vec![ACCEPTED_POLICY_HASH.to_string()],
            required_boundary: STACK_PROVENANCE_BOUNDARY.to_string(),
        }
    }

    fn receipt(text: &str) -> Value {
        serde_json::from_str(text).expect("stack provenance fixture parses")
    }

    fn graph(text: &str) -> Value {
        serde_json::from_str(text).expect("graph summary fixture parses")
    }

    fn evidence(receipt: Option<Value>, graph_summary: Option<Value>) -> StackProvenanceEvidence {
        StackProvenanceEvidence {
            valence_receipt: receipt,
            graph_summary,
        }
    }

    #[test]
    fn disabled_mode_ignores_stack_provenance() {
        // r[verify cairn.stack_provenance_gates.policy.disabled]
        // r[verify cairn.stack_provenance_gates.fixture_matrix.positive]
        let readiness = evaluate_stack_provenance(
            &policy(STACK_PROVENANCE_MODE_DISABLED),
            &evidence(Some(receipt(FAILED_VALENCE)), None),
        );
        assert!(readiness.passed);
        assert!(readiness.skipped);
        assert_eq!(readiness.disposition, DISPOSITION_DISABLED);
    }

    #[test]
    fn optional_absent_records_skipped_disposition() {
        // r[verify cairn.stack_provenance_gates.policy.optional_absent]
        // r[verify cairn.stack_provenance_gates.fixture_matrix.positive]
        let readiness = evaluate_stack_provenance(
            &policy(STACK_PROVENANCE_MODE_OPTIONAL),
            &evidence(None, None),
        );
        assert!(readiness.passed);
        assert!(readiness.skipped);
        assert_eq!(readiness.disposition, DISPOSITION_ABSENT);
    }

    #[test]
    fn optional_present_verifies_receipt_wiring() {
        // r[verify cairn.stack_provenance_gates.fixture_matrix.positive]
        let readiness = evaluate_stack_provenance(
            &policy(STACK_PROVENANCE_MODE_OPTIONAL),
            &evidence(Some(receipt(COMPLETE_RECEIPT)), Some(graph(GRAPH_SUMMARY))),
        );
        assert!(readiness.passed);
        assert_eq!(readiness.disposition, DISPOSITION_PRESENT);
        assert!(readiness.graph_summary.is_some());
    }

    #[test]
    fn required_complete_chain_passes() {
        // r[verify cairn.stack_provenance_gates.receipt_validation.complete]
        // r[verify cairn.stack_provenance_gates.fixture_matrix.positive]
        let readiness = evaluate_stack_provenance(
            &policy(STACK_PROVENANCE_MODE_REQUIRED),
            &evidence(Some(receipt(COMPLETE_RECEIPT)), Some(graph(GRAPH_SUMMARY))),
        );
        assert!(readiness.passed);
        assert_eq!(
            readiness.valence_receipt_hash.as_deref(),
            Some("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee")
        );
    }

    #[test]
    fn missing_receipt_fails_required_mode() {
        // r[verify cairn.stack_provenance_gates.policy.required_missing]
        // r[verify cairn.stack_provenance_gates.fixture_matrix.negative]
        let readiness = evaluate_stack_provenance(
            &policy(STACK_PROVENANCE_MODE_REQUIRED),
            &evidence(None, None),
        );
        assert!(!readiness.passed);
        assert_eq!(
            readiness.issues[0].code,
            "stack_provenance.valence_receipt_missing"
        );
    }

    #[test]
    fn failed_valence_receipt_fails_required_mode() {
        // r[verify cairn.stack_provenance_gates.fixture_matrix.negative]
        let readiness = evaluate_stack_provenance(
            &policy(STACK_PROVENANCE_MODE_REQUIRED),
            &evidence(Some(receipt(FAILED_VALENCE)), None),
        );
        assert!(!readiness.passed);
        assert!(readiness
            .issues
            .iter()
            .any(|issue| issue.code == "stack_provenance.valence_verdict_failed"));
    }

    #[test]
    fn stale_policy_fails_required_mode() {
        // r[verify cairn.stack_provenance_gates.receipt_validation.stale_policy]
        // r[verify cairn.stack_provenance_gates.fixture_matrix.negative]
        let readiness = evaluate_stack_provenance(
            &policy(STACK_PROVENANCE_MODE_REQUIRED),
            &evidence(Some(receipt(STALE_POLICY)), None),
        );
        assert!(!readiness.passed);
        assert!(readiness
            .issues
            .iter()
            .any(|issue| issue.code == "stack_provenance.valence_policy_stale"));
    }

    #[test]
    fn digest_mismatch_fails_required_mode() {
        // r[verify cairn.stack_provenance_gates.receipt_validation.digest_mismatch]
        // r[verify cairn.stack_provenance_gates.fixture_matrix.negative]
        let readiness = evaluate_stack_provenance(
            &policy(STACK_PROVENANCE_MODE_REQUIRED),
            &evidence(Some(receipt(DIGEST_MISMATCH)), None),
        );
        assert!(!readiness.passed);
        assert!(readiness
            .issues
            .iter()
            .any(|issue| issue.code == "stack_provenance.artifact_digest_mismatch"));
    }

    #[test]
    fn unsupported_claim_scope_fails_required_mode() {
        // r[verify cairn.stack_provenance_gates.receipt_validation.claim_scope]
        // r[verify cairn.stack_provenance_gates.fixture_matrix.negative]
        let readiness = evaluate_stack_provenance(
            &policy(STACK_PROVENANCE_MODE_REQUIRED),
            &evidence(Some(receipt(UNSUPPORTED_CLAIM_SCOPE)), None),
        );
        assert!(!readiness.passed);
        assert!(readiness
            .issues
            .iter()
            .any(|issue| issue.code == "stack_provenance.claim_scope_unsupported"));
    }

    #[test]
    fn missing_mantle_linkage_fails_required_mode() {
        // r[verify cairn.stack_provenance_gates.fixture_matrix.negative]
        let readiness = evaluate_stack_provenance(
            &policy(STACK_PROVENANCE_MODE_REQUIRED),
            &evidence(Some(receipt(MISSING_MANTLE_LINKAGE)), None),
        );
        assert!(!readiness.passed);
        assert!(readiness
            .issues
            .iter()
            .any(|issue| issue.code == "stack_provenance.mantle_linkage_missing"));
    }

    #[test]
    fn manual_review_artifacts_are_not_counted_as_verified() {
        // r[verify cairn.stack_provenance_gates.receipt_validation.manual_review]
        // r[verify cairn.stack_provenance_gates.fixture_matrix.negative]
        let readiness = evaluate_stack_provenance(
            &policy(STACK_PROVENANCE_MODE_REQUIRED),
            &evidence(Some(receipt(MANUAL_REVIEW_VERIFIED)), None),
        );
        assert!(!readiness.passed);
        assert!(readiness
            .issues
            .iter()
            .any(|issue| issue.code == "stack_provenance.manual_review_counted_verified"));
    }

    #[test]
    fn weakened_boundary_fails_required_mode() {
        // r[verify cairn.stack_provenance_gates.boundary.weakened]
        // r[verify cairn.stack_provenance_gates.fixture_matrix.negative]
        let readiness = evaluate_stack_provenance(
            &policy(STACK_PROVENANCE_MODE_REQUIRED),
            &evidence(Some(receipt(WEAKENED_BOUNDARY)), None),
        );
        assert!(!readiness.passed);
        assert!(readiness
            .issues
            .iter()
            .any(|issue| issue.code == "stack_provenance.non_claim_boundary_weakened"));
    }

    #[test]
    fn malformed_graph_summary_fails_required_mode() {
        // r[verify cairn.stack_provenance_gates.graph_summary.malformed]
        // r[verify cairn.stack_provenance_gates.fixture_matrix.negative]
        let readiness = evaluate_stack_provenance(
            &policy(STACK_PROVENANCE_MODE_REQUIRED),
            &evidence(
                Some(receipt(COMPLETE_RECEIPT)),
                Some(graph(MALFORMED_GRAPH)),
            ),
        );
        assert!(!readiness.passed);
        assert!(readiness
            .issues
            .iter()
            .any(|issue| issue.code == "stack_provenance.graph_summary_malformed"));
    }

    #[test]
    fn readiness_json_includes_visible_boundary() {
        // r[verify cairn.stack_provenance_gates.boundary.visible]
        let readiness = evaluate_stack_provenance(
            &policy(STACK_PROVENANCE_MODE_REQUIRED),
            &evidence(Some(receipt(COMPLETE_RECEIPT)), None),
        );
        let json = readiness.to_json();
        assert!(json["boundary"]
            .as_str()
            .expect("boundary text")
            .contains("did not prove source-code correctness"));
    }
}
