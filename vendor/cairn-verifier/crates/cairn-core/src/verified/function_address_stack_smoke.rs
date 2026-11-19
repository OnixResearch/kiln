const BLAKE3_PREFIX: &str = "b3:";
const BLAKE3_HEX_LENGTH: usize = blake3::OUT_LEN * 2;
const CLAIM_SCOPE: &str = "function_address.v1.lifecycle_policy_conformance";
const REQUIRED_ROLE: &str = "function-address-evidence-sidecar";
const REQUIRED_NON_CLAIM: &str = "not release eligibility";

#[derive(Debug)]
pub struct FunctionAddressStackSmokeInput<'a> {
    pub source: &'a [u8],
    pub expected: &'a serde_json::Value,
    pub octet: &'a serde_json::Value,
    pub kamacite: &'a serde_json::Value,
    pub valence: &'a serde_json::Value,
    pub mantle: &'a serde_json::Value,
    pub graph: &'a serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionAddressStackSmokeIssue {
    pub code: String,
    pub field: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionAddressStackSmokeReport {
    pub valid: bool,
    pub issues: Vec<FunctionAddressStackSmokeIssue>,
    pub input_hash: String,
}

impl FunctionAddressStackSmokeReport {
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "schema": "cairn.function-address-stack-smoke-report.v1",
            "valid": self.valid,
            "verdict": if self.valid { "PASS" } else { "FAIL" },
            "issues": self.issues.iter().map(|issue| serde_json::json!({
                "code": issue.code,
                "field": issue.field,
            })).collect::<Vec<_>>(),
            "input_hash": self.input_hash,
            "non_claims": [
                "stack smoke proves bounded evidence exchange only",
                "stack smoke does not prove Rust semantic correctness",
                "stack smoke does not prove compiler or verifier soundness",
                "stack smoke does not prove release eligibility"
            ]
        })
    }
}

#[must_use]
// r[impl cairn.release_evidence.function_address_stack_smoke.fixture]
// r[impl cairn.release_evidence.function_address_stack_smoke.negative]
pub fn evaluate_function_address_stack_smoke(
    input: &FunctionAddressStackSmokeInput<'_>,
) -> FunctionAddressStackSmokeReport {
    let mut issues = Vec::new();
    validate_octet(input, &mut issues);
    validate_links(input, &mut issues);
    validate_non_claims(input, &mut issues);
    issues.sort_by(|left, right| (&left.code, &left.field).cmp(&(&right.code, &right.field)));
    issues.dedup();
    let input_hash = crate::canonical_json_hash(&serde_json::json!({
        "source_hash": crate::blake3_hex(input.source),
        "expected": input.expected,
        "octet": input.octet,
        "kamacite": input.kamacite,
        "valence": input.valence,
        "mantle": input.mantle,
        "graph": input.graph,
    }));
    FunctionAddressStackSmokeReport {
        valid: issues.is_empty(),
        issues,
        input_hash,
    }
}

fn validate_octet(
    input: &FunctionAddressStackSmokeInput<'_>,
    issues: &mut Vec<FunctionAddressStackSmokeIssue>,
) {
    let expected_source = text(input.expected, "source_identity");
    let actual_source = input
        .octet
        .pointer("/source_envelopes/0/source_identity")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let computed_source = format!("{BLAKE3_PREFIX}{}", crate::blake3_hex(input.source));
    require_equal(
        expected_source,
        actual_source,
        "function_address_smoke.source_identity_stale",
        "octet.source_envelopes[0].source_identity",
        issues,
    );
    require_equal(
        expected_source,
        &computed_source,
        "function_address_smoke.source_bytes_stale",
        "source/src/lib.rs",
        issues,
    );
    let record = input
        .octet
        .pointer("/records/0")
        .unwrap_or(&serde_json::Value::Null);
    require_equal(
        text(input.expected, "function_address"),
        text(record, "function_address"),
        "function_address_smoke.function_address_stale",
        "octet.records[0].function_address",
        issues,
    );
    validate_span(record, input.source.len(), issues);
}

fn validate_span(
    record: &serde_json::Value,
    source_len: usize,
    issues: &mut Vec<FunctionAddressStackSmokeIssue>,
) {
    let start = record.get("start_byte").and_then(serde_json::Value::as_u64);
    let end = record.get("end_byte").and_then(serde_json::Value::as_u64);
    let source_len = u64::try_from(source_len).unwrap_or(u64::MAX);
    if !matches!((start, end), (Some(start), Some(end)) if start < end && end <= source_len) {
        issues.push(issue(
            "function_address_smoke.source_span_invalid",
            "octet.records[0].start_byte/end_byte",
        ));
    }
}

fn validate_links(
    input: &FunctionAddressStackSmokeInput<'_>,
    issues: &mut Vec<FunctionAddressStackSmokeIssue>,
) {
    let valence_hash = text(input.expected, "valence_receipt_hash");
    require_equal(
        valence_hash,
        text(input.valence, "receipt_hash"),
        "function_address_smoke.valence_receipt_stale",
        "valence.receipt_hash",
        issues,
    );
    require_equal(
        valence_hash,
        text(input.mantle, "valence_receipt_digest"),
        "function_address_smoke.mantle_valence_link_stale",
        "mantle.valence_receipt_digest",
        issues,
    );
    let kamacite_hash = text(input.expected, "kamacite_receipt_hash");
    require_equal(
        kamacite_hash,
        text(input.kamacite, "canonical_preserves_receipt_hash"),
        "function_address_smoke.kamacite_receipt_stale",
        "kamacite.canonical_preserves_receipt_hash",
        issues,
    );
    require_equal(
        kamacite_hash,
        text(input.valence, "kamacite_receipt_hash"),
        "function_address_smoke.valence_kamacite_link_stale",
        "valence.kamacite_receipt_hash",
        issues,
    );
    let role = text(input.expected, "valence_role");
    let has_role = input
        .valence
        .get("roles")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|roles| roles.iter().any(|value| value.as_str() == Some(role)));
    if !has_role || role != REQUIRED_ROLE {
        issues.push(issue(
            "function_address_smoke.valence_role_wrong",
            "valence.roles",
        ));
    }
    for (field, actual) in [
        ("valence.claim_scope", text(input.valence, "claim_scope")),
        ("mantle.claim_scope", text(input.mantle, "claim_scope")),
        ("kamacite.claim_scope", text(input.kamacite, "claim_scope")),
    ] {
        if actual != CLAIM_SCOPE || actual != text(input.expected, "claim_scope") {
            issues.push(issue("function_address_smoke.claim_scope_wrong", field));
        }
    }
    if input
        .graph
        .get("function_count")
        .and_then(serde_json::Value::as_u64)
        != Some(1)
    {
        issues.push(issue(
            "function_address_smoke.graph_function_count_wrong",
            "graph.function_count",
        ));
    }
}

fn validate_non_claims(
    input: &FunctionAddressStackSmokeInput<'_>,
    issues: &mut Vec<FunctionAddressStackSmokeIssue>,
) {
    let expected = text(input.expected, "required_non_claim");
    let octet_has = input
        .octet
        .get("non_claims")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|rows| rows.iter().any(|row| row.as_str() == Some(expected)));
    let kamacite_has = input
        .kamacite
        .get("non_claims")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|rows| {
            rows.iter().any(|row| {
                row.as_str().is_some_and(|text| {
                    text.contains("does not prove evidence truth or release eligibility")
                })
            })
        });
    let valence_has = text(input.valence, "non_claim_boundary").contains("did not prove")
        && text(input.valence, "non_claim_boundary").contains("release eligibility");
    let mantle_has = text(input.mantle, "non_claim_boundary").contains("did not prove")
        && text(input.mantle, "non_claim_boundary").contains("release eligibility");
    if expected != REQUIRED_NON_CLAIM || !octet_has || !kamacite_has || !valence_has || !mantle_has
    {
        issues.push(issue(
            "function_address_smoke.non_claim_missing_or_overstated",
            "non_claims",
        ));
    }
}

fn require_equal(
    expected: &str,
    actual: &str,
    code: &str,
    field: &str,
    issues: &mut Vec<FunctionAddressStackSmokeIssue>,
) {
    if expected != actual || !valid_identity_if_present(expected) {
        issues.push(issue(code, field));
    }
}

fn valid_identity_if_present(value: &str) -> bool {
    if let Some(digest) = value.strip_prefix(BLAKE3_PREFIX) {
        digest.len() == BLAKE3_HEX_LENGTH && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    } else {
        value.len() == BLAKE3_HEX_LENGTH && value.bytes().all(|byte| byte.is_ascii_hexdigit())
    }
}

fn text<'a>(value: &'a serde_json::Value, field: &str) -> &'a str {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
}

fn issue(code: &str, field: &str) -> FunctionAddressStackSmokeIssue {
    FunctionAddressStackSmokeIssue {
        code: code.to_string(),
        field: field.to_string(),
    }
}
