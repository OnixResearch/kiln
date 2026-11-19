pub const KANI_RECEIPT_SCHEMA_V1: &str = "cairn.kani_receipt.v1";
pub const KANI_NON_CLAIM_WHOLE_PROGRAM: &str = "kani.boundary.whole_program";
pub const KANI_NON_CLAIM_VERIFIER_SOUNDNESS: &str = "kani.boundary.verifier_soundness";
pub const KANI_NON_CLAIM_FFI_STUB: &str = "kani.boundary.ffi_stub";
pub const KANI_NON_CLAIM_BOUNDED_ONLY: &str = "kani.boundary.bounded_only";

const BLAKE3_HEX_LENGTH: usize = 64;
const PROPERTY_STATUS_PASSED: &str = "passed";
const PROPERTY_STATUS_FAILED: &str = "failed";
const CONTRACT_STATE_PASSING: &str = "passing";
const CONTRACT_STATE_FAILED: &str = "failed";
const CONTRACT_STATE_NONE: &str = "none";
const UNWIND_STATE_BOUNDED: &str = "bounded";
const UNWIND_STATE_COMPLETE: &str = "complete";
const UNWIND_STATE_MISSING: &str = "missing";
const STUB_CLASSIFICATION_NONE: &str = "none";
const STUB_CLASSIFICATION_VERIFIED: &str = "verified";
const STUB_CLASSIFICATION_UNVERIFIED: &str = "unverified";
const STUB_CLASSIFICATION_ENVIRONMENTAL: &str = "environmental";
const CLAIM_SCOPE_BOUNDED: &str = "bounded";
const CLAIM_SCOPE_CONTRACT: &str = "contract";
const CLAIM_SCOPE_UNBOUNDED: &str = "unbounded";
const EVIDENCE_ROLE_PROPERTY: &str = "property";
const EVIDENCE_ROLE_RECORDED_ONLY: &str = "recorded_only";
const EVIDENCE_ROLE_BOUNDARY: &str = "boundary";
const REQUIRED_NON_CLAIMS: &[&str] = &[
    KANI_NON_CLAIM_WHOLE_PROGRAM,
    KANI_NON_CLAIM_VERIFIER_SOUNDNESS,
    KANI_NON_CLAIM_FFI_STUB,
    KANI_NON_CLAIM_BOUNDED_ONLY,
];
const REQUIRED_PROFILE_FIELDS: &[&str] = &[
    "schema_version",
    "requirement_id",
    "source_scope",
    "content_hash",
    "policy_hash",
    "verifier_identity",
    "toolchain_identity",
    "property_status",
    "contract_state",
    "unwind_state",
    "stub_classification",
    "claim_scope",
    "evidence_role",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KaniReceiptIssue {
    pub code: String,
    pub field_path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KaniReceiptProfile {
    pub requirement_id: String,
    pub source_scope: String,
    pub content_hash: String,
    pub policy_hash: String,
    pub verifier_identity: String,
    pub toolchain_identity: String,
    pub property_status: String,
    pub contract_state: String,
    pub unwind_state: String,
    pub stub_classification: String,
    pub claim_scope: String,
    pub evidence_role: String,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KaniReceiptReport {
    pub valid: bool,
    pub eligible: bool,
    pub issues: Vec<KaniReceiptIssue>,
    pub profile: Option<KaniReceiptProfile>,
    pub receipt_hash: String,
}

pub fn kani_receipt_hash(value: &serde_json::Value) -> String {
    crate::verified::evidence::canonical_json_hash(&without_receipt_hash(value))
}

pub fn validate_kani_receipt(
    value: &serde_json::Value,
    expected_requirement_id: Option<&str>,
    current_content_hash: Option<&str>,
) -> KaniReceiptReport {
    // r[impl cairn.kani_receipt_gates.receipt_profile]
    // r[impl cairn.kani_receipt_gates.eligibility]
    // r[impl cairn.kani_receipt_gates.default_no_execution]
    // r[impl cairn.kani_receipt_gates.gate_diagnostics]
    let mut issues = Vec::new();
    validate_required_profile(value, &mut issues);
    validate_field_sets(value, &mut issues);
    validate_expected_requirement(value, expected_requirement_id, &mut issues);
    validate_current_content(value, current_content_hash, &mut issues);
    validate_eligibility(value, &mut issues);
    validate_non_claims(value, &mut issues);
    validate_optional_receipt_hash(value, &mut issues);
    let valid = issues.is_empty();
    KaniReceiptReport {
        valid,
        eligible: valid,
        issues,
        profile: profile_from_value(value),
        receipt_hash: kani_receipt_hash(value),
    }
}

fn validate_required_profile(value: &serde_json::Value, issues: &mut Vec<KaniReceiptIssue>) {
    match string_field(value, "schema_version") {
        Some(KANI_RECEIPT_SCHEMA_V1) => {}
        Some(_) => push_issue(
            issues,
            "schema_mismatch",
            "schema_version",
            "unsupported Kani receipt schema",
        ),
        None => push_issue(
            issues,
            "missing_field",
            "schema_version",
            "missing required Kani receipt field",
        ),
    }
    for field in REQUIRED_PROFILE_FIELDS {
        if string_field(value, field).is_none() {
            push_issue(
                issues,
                "missing_field",
                field,
                "missing required Kani receipt field",
            );
        }
    }
    validate_hash_field(value, "content_hash", issues);
    validate_hash_field(value, "policy_hash", issues);
}

fn validate_field_sets(value: &serde_json::Value, issues: &mut Vec<KaniReceiptIssue>) {
    validate_known_field(
        value,
        "property_status",
        &[PROPERTY_STATUS_PASSED, PROPERTY_STATUS_FAILED],
        issues,
    );
    validate_known_field(
        value,
        "contract_state",
        &[
            CONTRACT_STATE_PASSING,
            CONTRACT_STATE_FAILED,
            CONTRACT_STATE_NONE,
        ],
        issues,
    );
    validate_known_field(
        value,
        "unwind_state",
        &[
            UNWIND_STATE_BOUNDED,
            UNWIND_STATE_COMPLETE,
            UNWIND_STATE_MISSING,
        ],
        issues,
    );
    validate_known_field(
        value,
        "stub_classification",
        &[
            STUB_CLASSIFICATION_NONE,
            STUB_CLASSIFICATION_VERIFIED,
            STUB_CLASSIFICATION_UNVERIFIED,
            STUB_CLASSIFICATION_ENVIRONMENTAL,
        ],
        issues,
    );
    validate_known_field(
        value,
        "claim_scope",
        &[
            CLAIM_SCOPE_BOUNDED,
            CLAIM_SCOPE_CONTRACT,
            CLAIM_SCOPE_UNBOUNDED,
        ],
        issues,
    );
    validate_known_field(
        value,
        "evidence_role",
        &[
            EVIDENCE_ROLE_PROPERTY,
            EVIDENCE_ROLE_RECORDED_ONLY,
            EVIDENCE_ROLE_BOUNDARY,
        ],
        issues,
    );
}

fn validate_expected_requirement(
    value: &serde_json::Value,
    expected_requirement_id: Option<&str>,
    issues: &mut Vec<KaniReceiptIssue>,
) {
    if let (Some(expected), Some(actual)) = (
        expected_requirement_id,
        string_field(value, "requirement_id"),
    ) {
        if actual != expected {
            push_issue(
                issues,
                "requirement_mismatch",
                "requirement_id",
                "Kani receipt targets a different requirement",
            );
        }
    }
}

fn validate_current_content(
    value: &serde_json::Value,
    current_content_hash: Option<&str>,
    issues: &mut Vec<KaniReceiptIssue>,
) {
    if let (Some(current), Some(actual)) =
        (current_content_hash, string_field(value, "content_hash"))
    {
        if actual != current {
            push_issue(
                issues,
                "stale_content_hash",
                "content_hash",
                "Kani receipt content hash does not match current source identity",
            );
        }
    }
}

fn validate_eligibility(value: &serde_json::Value, issues: &mut Vec<KaniReceiptIssue>) {
    if string_field(value, "property_status") == Some(PROPERTY_STATUS_FAILED) {
        push_issue(
            issues,
            "failed_verdict",
            "property_status",
            "Kani receipt did not pass",
        );
    }
    if string_field(value, "evidence_role") == Some(EVIDENCE_ROLE_PROPERTY)
        && matches!(
            string_field(value, "stub_classification"),
            Some(STUB_CLASSIFICATION_UNVERIFIED | STUB_CLASSIFICATION_ENVIRONMENTAL)
        )
    {
        push_issue(
            issues,
            "unverified_stub_promotion",
            "stub_classification",
            "unverified or environmental stubs cannot be promoted to property evidence",
        );
    }
    match string_field(value, "claim_scope") {
        Some(CLAIM_SCOPE_BOUNDED) => validate_bounded_claim(value, issues),
        Some(CLAIM_SCOPE_CONTRACT) => validate_contract_claim(value, issues),
        Some(CLAIM_SCOPE_UNBOUNDED) => validate_unbounded_claim(value, issues),
        _ => {}
    }
}

fn validate_bounded_claim(value: &serde_json::Value, issues: &mut Vec<KaniReceiptIssue>) {
    if !matches!(
        string_field(value, "unwind_state"),
        Some(UNWIND_STATE_BOUNDED | UNWIND_STATE_COMPLETE)
    ) {
        push_issue(
            issues,
            "missing_unwind_evidence",
            "unwind_state",
            "bounded Kani claims require bounded or complete unwind evidence",
        );
    }
}

fn validate_contract_claim(value: &serde_json::Value, issues: &mut Vec<KaniReceiptIssue>) {
    if string_field(value, "contract_state") != Some(CONTRACT_STATE_PASSING) {
        push_issue(
            issues,
            "missing_contract_evidence",
            "contract_state",
            "contract Kani claims require passing contract evidence",
        );
    }
}

fn validate_unbounded_claim(value: &serde_json::Value, issues: &mut Vec<KaniReceiptIssue>) {
    let has_complete_unwind = string_field(value, "unwind_state") == Some(UNWIND_STATE_COMPLETE);
    let has_contract = string_field(value, "contract_state") == Some(CONTRACT_STATE_PASSING);
    if !has_complete_unwind && !has_contract {
        push_issue(
            issues,
            "unsupported_unbounded_claim",
            "claim_scope",
            "unbounded Kani claims require complete unwind or passing contract evidence",
        );
    }
}

fn validate_non_claims(value: &serde_json::Value, issues: &mut Vec<KaniReceiptIssue>) {
    let claims = string_array(value, "non_claims");
    if claims.is_empty() {
        push_issue(
            issues,
            "missing_field",
            "non_claims",
            "missing Kani non-claim boundaries",
        );
        return;
    }
    for required in REQUIRED_NON_CLAIMS {
        if !claims.iter().any(|claim| claim == required) {
            push_issue(
                issues,
                "weakened_non_claims",
                "non_claims",
                "Kani receipt is missing a required non-claim boundary",
            );
            break;
        }
    }
}

fn validate_optional_receipt_hash(value: &serde_json::Value, issues: &mut Vec<KaniReceiptIssue>) {
    if let Some(actual) = string_field(value, "receipt_hash") {
        let expected = kani_receipt_hash(value);
        if actual != expected {
            push_issue(
                issues,
                "receipt_hash_mismatch",
                "receipt_hash",
                "Kani receipt hash does not match canonical receipt material",
            );
        }
    }
}

fn profile_from_value(value: &serde_json::Value) -> Option<KaniReceiptProfile> {
    Some(KaniReceiptProfile {
        requirement_id: string_field(value, "requirement_id")?.to_string(),
        source_scope: string_field(value, "source_scope")?.to_string(),
        content_hash: string_field(value, "content_hash")?.to_string(),
        policy_hash: string_field(value, "policy_hash")?.to_string(),
        verifier_identity: string_field(value, "verifier_identity")?.to_string(),
        toolchain_identity: string_field(value, "toolchain_identity")?.to_string(),
        property_status: string_field(value, "property_status")?.to_string(),
        contract_state: string_field(value, "contract_state")?.to_string(),
        unwind_state: string_field(value, "unwind_state")?.to_string(),
        stub_classification: string_field(value, "stub_classification")?.to_string(),
        claim_scope: string_field(value, "claim_scope")?.to_string(),
        evidence_role: string_field(value, "evidence_role")?.to_string(),
        non_claims: string_array(value, "non_claims")
            .into_iter()
            .map(str::to_string)
            .collect(),
    })
}

fn validate_known_field(
    value: &serde_json::Value,
    field: &'static str,
    allowed: &[&str],
    issues: &mut Vec<KaniReceiptIssue>,
) {
    if let Some(actual) = string_field(value, field) {
        if !allowed.contains(&actual) {
            push_issue(
                issues,
                "invalid_field",
                field,
                "field has unsupported Kani receipt value",
            );
        }
    }
}

fn validate_hash_field(
    value: &serde_json::Value,
    field: &'static str,
    issues: &mut Vec<KaniReceiptIssue>,
) {
    if let Some(actual) = string_field(value, field) {
        if !is_blake3_hex(actual) {
            push_issue(
                issues,
                "invalid_hash",
                field,
                "field must be a BLAKE3 hex digest",
            );
        }
    }
}

fn string_field<'a>(value: &'a serde_json::Value, field: &str) -> Option<&'a str> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

fn string_array<'a>(value: &'a serde_json::Value, field: &str) -> Vec<&'a str> {
    value
        .get(field)
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .collect()
}

fn is_blake3_hex(value: &str) -> bool {
    value.len() == BLAKE3_HEX_LENGTH && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn without_receipt_hash(value: &serde_json::Value) -> serde_json::Value {
    let mut canonical = value.clone();
    if let Some(object) = canonical.as_object_mut() {
        object.remove("receipt_hash");
    }
    canonical
}

fn push_issue(issues: &mut Vec<KaniReceiptIssue>, code: &str, field_path: &str, message: &str) {
    issues.push(KaniReceiptIssue {
        code: code.to_string(),
        field_path: field_path.to_string(),
        message: message.to_string(),
    });
}

#[cfg(test)]
mod tests {

    use super::*;

    const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const HASH_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const REQUIREMENT_ID: &str = "cairn.example.kani";

    fn valid_non_claims() -> Vec<&'static str> {
        vec![
            KANI_NON_CLAIM_WHOLE_PROGRAM,
            KANI_NON_CLAIM_VERIFIER_SOUNDNESS,
            KANI_NON_CLAIM_FFI_STUB,
            KANI_NON_CLAIM_BOUNDED_ONLY,
        ]
    }

    fn bounded_receipt() -> serde_json::Value {
        serde_json::json!({
            "schema_version": KANI_RECEIPT_SCHEMA_V1,
            "requirement_id": REQUIREMENT_ID,
            "source_scope": "crates/cairn-core/src/verified/example.rs::bounded",
            "content_hash": HASH_A,
            "policy_hash": HASH_B,
            "verifier_identity": "cargo-kani fixture",
            "toolchain_identity": "rust nightly + kani + cbmc fixture",
            "property_status": PROPERTY_STATUS_PASSED,
            "contract_state": CONTRACT_STATE_NONE,
            "unwind_state": UNWIND_STATE_BOUNDED,
            "stub_classification": STUB_CLASSIFICATION_NONE,
            "claim_scope": CLAIM_SCOPE_BOUNDED,
            "evidence_role": EVIDENCE_ROLE_PROPERTY,
            "non_claims": valid_non_claims(),
        })
    }

    fn contract_receipt() -> serde_json::Value {
        let mut receipt = bounded_receipt();
        receipt["contract_state"] = serde_json::json!(CONTRACT_STATE_PASSING);
        receipt["unwind_state"] = serde_json::json!(UNWIND_STATE_MISSING);
        receipt["claim_scope"] = serde_json::json!(CLAIM_SCOPE_CONTRACT);
        receipt
    }

    fn issue_codes(report: &KaniReceiptReport) -> Vec<&str> {
        report
            .issues
            .iter()
            .map(|issue| issue.code.as_str())
            .collect()
    }

    #[test]
    fn passing_bounded_receipt_is_eligible_and_preserves_profile() {
        // r[verify cairn.kani_receipt_gates.positive_fixtures]
        // r[verify cairn.kani_receipt_gates.positive_fixtures.valid]
        // r[verify cairn.kani_receipt_gates.receipt_profile.complete]
        // r[verify cairn.kani_receipt_gates.eligibility.scoped]
        // r[verify cairn.kani_receipt_gates.default_no_execution.saved]
        // r[verify cairn.kani_receipt_gates.operator_docs]
        // r[verify cairn.kani_receipt_gates.operator_docs.visible]
        let report = validate_kani_receipt(&bounded_receipt(), Some(REQUIREMENT_ID), Some(HASH_A));

        assert!(report.valid, "{:?}", report.issues);
        assert!(report.eligible);
        let profile = report.profile.expect("profile is preserved");
        assert_eq!(profile.requirement_id, REQUIREMENT_ID);
        assert_eq!(profile.content_hash, HASH_A);
    }

    #[test]
    fn passing_contract_receipt_is_eligible() {
        // r[verify cairn.kani_receipt_gates.positive_fixtures]
        let report = validate_kani_receipt(&contract_receipt(), Some(REQUIREMENT_ID), Some(HASH_A));

        assert!(report.valid, "{:?}", report.issues);
        assert!(report.eligible);
    }

    #[test]
    fn missing_identity_and_failed_verdict_fail_closed() {
        // r[verify cairn.kani_receipt_gates.negative_fixtures]
        // r[verify cairn.kani_receipt_gates.receipt_profile.missing]
        // r[verify cairn.kani_receipt_gates.default_no_execution.missing]
        let mut receipt = bounded_receipt();
        receipt
            .as_object_mut()
            .expect("receipt object")
            .remove("requirement_id");
        receipt["property_status"] = serde_json::json!(PROPERTY_STATUS_FAILED);
        let report = validate_kani_receipt(&receipt, Some(REQUIREMENT_ID), Some(HASH_A));
        let codes = issue_codes(&report);

        assert!(!report.valid);
        assert!(codes.contains(&"missing_field"));
        assert!(codes.contains(&"failed_verdict"));
    }

    #[test]
    fn stale_content_and_requirement_mismatch_are_named() {
        // r[verify cairn.kani_receipt_gates.negative_fixtures]
        // r[verify cairn.kani_receipt_gates.negative_fixtures.stale]
        // r[verify cairn.kani_receipt_gates.gate_diagnostics]
        // r[verify cairn.kani_receipt_gates.gate_diagnostics.named]
        let report =
            validate_kani_receipt(&bounded_receipt(), Some("other.requirement"), Some(HASH_B));
        let codes = issue_codes(&report);

        assert!(!report.valid);
        assert!(codes.contains(&"requirement_mismatch"));
        assert!(codes.contains(&"stale_content_hash"));
    }

    #[test]
    fn unsupported_unbounded_claim_fails_without_contract_or_complete_unwind() {
        // r[verify cairn.kani_receipt_gates.negative_fixtures]
        // r[verify cairn.kani_receipt_gates.eligibility.unbounded]
        let mut receipt = bounded_receipt();
        receipt["claim_scope"] = serde_json::json!(CLAIM_SCOPE_UNBOUNDED);
        receipt["unwind_state"] = serde_json::json!(UNWIND_STATE_MISSING);
        let report = validate_kani_receipt(&receipt, Some(REQUIREMENT_ID), Some(HASH_A));

        assert!(!report.valid);
        assert!(issue_codes(&report).contains(&"unsupported_unbounded_claim"));
    }

    #[test]
    fn unverified_stub_promotion_and_weakened_non_claims_fail_closed() {
        // r[verify cairn.kani_receipt_gates.negative_fixtures]
        // r[verify cairn.kani_receipt_gates.negative_fixtures.stub]
        let mut receipt = bounded_receipt();
        receipt["stub_classification"] = serde_json::json!(STUB_CLASSIFICATION_UNVERIFIED);
        receipt["non_claims"] = serde_json::json!([KANI_NON_CLAIM_WHOLE_PROGRAM]);
        let report = validate_kani_receipt(&receipt, Some(REQUIREMENT_ID), Some(HASH_A));
        let codes = issue_codes(&report);

        assert!(!report.valid);
        assert!(codes.contains(&"unverified_stub_promotion"));
        assert!(codes.contains(&"weakened_non_claims"));
    }

    #[test]
    fn receipt_hash_mismatch_fails_deterministically() {
        let mut receipt = bounded_receipt();
        receipt["receipt_hash"] = serde_json::json!(HASH_B);
        let report = validate_kani_receipt(&receipt, Some(REQUIREMENT_ID), Some(HASH_A));

        assert!(!report.valid);
        assert!(issue_codes(&report).contains(&"receipt_hash_mismatch"));
    }
}
