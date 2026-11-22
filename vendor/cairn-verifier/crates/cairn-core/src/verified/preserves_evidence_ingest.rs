pub const PRESERVES_EVIDENCE_INGEST_SCHEMA_V1: &str = "cairn.preserves-evidence-ingest.v1";
pub const PRESERVES_EVIDENCE_OPAQUE_SCHEMA_V1: &str = "preserves.evidence-envelope.opaque.v1";
pub const PRESERVES_EVIDENCE_ADAPTER_SCHEMA_V1: &str = "preserves.evidence-envelope.adapter.v1";
pub const PRESERVES_EVIDENCE_OPACITY_OPAQUE: &str = "opaque";
pub const PRESERVES_EVIDENCE_OPACITY_ADAPTER_BACKED: &str = "adapter-backed";
pub const PRESERVES_EVIDENCE_ROLE_OPAQUE: &str = "opaque-evidence-carrier";
pub const PRESERVES_EVIDENCE_ROLE_ADAPTER_BACKED: &str = "adapter-backed-evidence-carrier";
pub const PRESERVES_EVIDENCE_CLAIM_SCOPE_LIFECYCLE: &str = "lifecycle-policy-conformance";
pub const PRESERVES_EVIDENCE_ADAPTER_LIFECYCLE_RECEIPT: &str = "cairn.lifecycle-receipt-core.v1";
pub const PRESERVES_EVIDENCE_INGEST_BOUNDARY: &str = "Preserves evidence ingest validates canonical carrier identity, declared producer, role, scope, adapter registration, and non-claim boundaries only; it does not prove evidence truth, runtime correctness, transport correctness, producer authorization, source-code correctness, verifier soundness, or release eligibility";

pub const PRESERVES_EVIDENCE_ISSUE_EMPTY_FIELD: &str = "preserves_evidence_ingest.empty_field";
pub const PRESERVES_EVIDENCE_ISSUE_UNSUPPORTED_SCHEMA: &str =
    "preserves_evidence_ingest.unsupported_schema";
pub const PRESERVES_EVIDENCE_ISSUE_UNSUPPORTED_CONTRACT: &str =
    "preserves_evidence_ingest.unsupported_contract";
pub const PRESERVES_EVIDENCE_ISSUE_MALFORMED_DIGEST: &str =
    "preserves_evidence_ingest.malformed_digest";
pub const PRESERVES_EVIDENCE_ISSUE_STALE_DIGEST: &str = "preserves_evidence_ingest.stale_digest";
pub const PRESERVES_EVIDENCE_ISSUE_NON_CANONICAL_BYTES: &str =
    "preserves_evidence_ingest.non_canonical_bytes";
pub const PRESERVES_EVIDENCE_ISSUE_UNSUPPORTED_ROLE: &str =
    "preserves_evidence_ingest.unsupported_role";
pub const PRESERVES_EVIDENCE_ISSUE_UNSUPPORTED_SCOPE: &str =
    "preserves_evidence_ingest.unsupported_scope";
pub const PRESERVES_EVIDENCE_ISSUE_UNSUPPORTED_OPACITY: &str =
    "preserves_evidence_ingest.unsupported_opacity";
pub const PRESERVES_EVIDENCE_ISSUE_UNSUPPORTED_ADAPTER: &str =
    "preserves_evidence_ingest.unsupported_adapter";
pub const PRESERVES_EVIDENCE_ISSUE_ADAPTER_REQUIRED: &str =
    "preserves_evidence_ingest.adapter_required";
pub const PRESERVES_EVIDENCE_ISSUE_ADAPTER_FOR_OPAQUE: &str =
    "preserves_evidence_ingest.adapter_for_opaque";
pub const PRESERVES_EVIDENCE_ISSUE_WEAK_NON_CLAIM: &str =
    "preserves_evidence_ingest.weak_non_claim";
pub const PRESERVES_EVIDENCE_ISSUE_OVERCLAIM: &str = "preserves_evidence_ingest.overclaim";
pub const PRESERVES_EVIDENCE_ISSUE_MEASUREMENT_ERROR: &str =
    "preserves_evidence_ingest.measurement_error";

const BLAKE3_HEX_LENGTH: usize = 64;
const EMPTY_ADAPTER_ID: &str = "";
const FIELD_ADAPTER_ID: &str = "adapter_id";
const FIELD_CANONICAL_DIGEST: &str = "canonical_digest";
const FIELD_CLAIM: &str = "claim";
const FIELD_CLAIM_SCOPE: &str = "claim_scope";
const FIELD_ENVELOPE_PATH: &str = "envelope_path";
const FIELD_EVIDENCE_ROLE: &str = "evidence_role";
const FIELD_NON_CLAIMS: &str = "non_claims";
const FIELD_PAYLOAD_OPACITY: &str = "payload_opacity";
const FIELD_PRODUCER_REPO: &str = "producer_repo";
const FIELD_SCHEMA_LABEL: &str = "schema_label";
const EXPECTED_BLAKE3_DIGEST: &str = "lowercase BLAKE3 hex digest";
const NON_CLAIM_EVIDENCE_TRUTH: &str = "evidence truth";
const NON_CLAIM_PRODUCER_AUTHORIZATION: &str = "producer authorization";
const NON_CLAIM_RELEASE_ELIGIBILITY: &str = "release eligibility";
const NON_CLAIM_RUNTIME_CORRECTNESS: &str = "runtime correctness";
const NON_CLAIM_SOURCE_CODE_CORRECTNESS: &str = "source-code correctness";
const NON_CLAIM_TRANSPORT_CORRECTNESS: &str = "transport correctness";
const NON_CLAIM_VERIFIER_SOUNDNESS: &str = "verifier soundness";

const SUPPORTED_SCHEMA_LABELS: &[&str] = &[
    PRESERVES_EVIDENCE_OPAQUE_SCHEMA_V1,
    PRESERVES_EVIDENCE_ADAPTER_SCHEMA_V1,
];
const SUPPORTED_ROLES: &[&str] = &[
    PRESERVES_EVIDENCE_ROLE_OPAQUE,
    PRESERVES_EVIDENCE_ROLE_ADAPTER_BACKED,
];
const SUPPORTED_CLAIM_SCOPES: &[&str] = &[PRESERVES_EVIDENCE_CLAIM_SCOPE_LIFECYCLE];
const SUPPORTED_OPACITY_MODES: &[&str] = &[
    PRESERVES_EVIDENCE_OPACITY_OPAQUE,
    PRESERVES_EVIDENCE_OPACITY_ADAPTER_BACKED,
];
const SUPPORTED_ADAPTER_IDS: &[&str] = &[PRESERVES_EVIDENCE_ADAPTER_LIFECYCLE_RECEIPT];
const REQUIRED_NON_CLAIM_FRAGMENTS: &[&str] = &[
    NON_CLAIM_EVIDENCE_TRUTH,
    NON_CLAIM_RUNTIME_CORRECTNESS,
    NON_CLAIM_TRANSPORT_CORRECTNESS,
    NON_CLAIM_PRODUCER_AUTHORIZATION,
    NON_CLAIM_SOURCE_CODE_CORRECTNESS,
    NON_CLAIM_VERIFIER_SOUNDNESS,
    NON_CLAIM_RELEASE_ELIGIBILITY,
];
const OVERCLAIM_PHRASES: &[&str] = &[
    "proves evidence truth",
    "proves runtime correctness",
    "proves transport correctness",
    "proves producer authorization",
    "proves source-code correctness",
    "proves verifier soundness",
    "proves release eligibility",
    "is evidence truth",
    "is runtime correctness",
    "is producer authorization",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreservesEvidenceIngestContract {
    pub schema_label: String,
    pub evidence_role: String,
    pub payload_opacity: String,
    pub adapter_id: Option<String>,
    pub claim_scope: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreservesEvidenceEnvelopeRow {
    pub envelope_path: String,
    pub schema_label: String,
    pub producer_repo: String,
    pub canonical_digest: String,
    pub evidence_role: String,
    pub claim_scope: String,
    pub adapter_id: Option<String>,
    pub payload_opacity: String,
    pub non_claims: Vec<String>,
    pub claim: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreservesEvidenceEnvelopeMeasurement {
    pub envelope_path: String,
    pub canonical_digest: String,
    pub canonical_bytes: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreservesEvidenceIngestInput {
    pub row: PreservesEvidenceEnvelopeRow,
    pub measurement: PreservesEvidenceEnvelopeMeasurement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreservesEvidenceIngestIssue {
    pub code: String,
    pub field_path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreservesEvidenceIngestReport {
    pub valid: bool,
    pub verdict: String,
    pub envelope_path: String,
    pub canonical_digest: String,
    pub payload_opacity: String,
    pub adapter_id: Option<String>,
    pub issues: Vec<PreservesEvidenceIngestIssue>,
    pub claim: String,
    pub boundary: String,
}

pub fn preserves_evidence_ingest_contracts() -> Vec<PreservesEvidenceIngestContract> {
    // r[impl cairn.preserves_evidence_ingest.contract]
    vec![
        PreservesEvidenceIngestContract {
            schema_label: PRESERVES_EVIDENCE_OPAQUE_SCHEMA_V1.to_string(),
            evidence_role: PRESERVES_EVIDENCE_ROLE_OPAQUE.to_string(),
            payload_opacity: PRESERVES_EVIDENCE_OPACITY_OPAQUE.to_string(),
            adapter_id: None,
            claim_scope: PRESERVES_EVIDENCE_CLAIM_SCOPE_LIFECYCLE.to_string(),
        },
        PreservesEvidenceIngestContract {
            schema_label: PRESERVES_EVIDENCE_ADAPTER_SCHEMA_V1.to_string(),
            evidence_role: PRESERVES_EVIDENCE_ROLE_ADAPTER_BACKED.to_string(),
            payload_opacity: PRESERVES_EVIDENCE_OPACITY_ADAPTER_BACKED.to_string(),
            adapter_id: Some(PRESERVES_EVIDENCE_ADAPTER_LIFECYCLE_RECEIPT.to_string()),
            claim_scope: PRESERVES_EVIDENCE_CLAIM_SCOPE_LIFECYCLE.to_string(),
        },
    ]
}

pub fn default_preserves_evidence_non_claims() -> Vec<String> {
    REQUIRED_NON_CLAIM_FRAGMENTS
        .iter()
        .map(|fragment| format!("does not prove {fragment}"))
        .collect()
}

pub fn measure_preserves_evidence_envelope_bytes(
    envelope_path: &str,
    source_bytes: &[u8],
    value: &serde_json::Value,
) -> Result<PreservesEvidenceEnvelopeMeasurement, PreservesEvidenceIngestIssue> {
    // r[impl cairn.preserves_evidence_ingest.validation]
    let canonical_bytes = super::evidence::canonical_preserves_bytes(value).map_err(|error| {
        preserves_evidence_issue(
            PRESERVES_EVIDENCE_ISSUE_MEASUREMENT_ERROR,
            FIELD_CANONICAL_DIGEST,
            &format!("could not canonicalize Preserves envelope: {error}"),
        )
    })?;
    Ok(PreservesEvidenceEnvelopeMeasurement {
        envelope_path: envelope_path.to_string(),
        canonical_digest: super::evidence::blake3_hex(&canonical_bytes),
        canonical_bytes: source_bytes == canonical_bytes,
    })
}

pub fn validate_preserves_evidence_ingest(
    input: &PreservesEvidenceIngestInput,
) -> PreservesEvidenceIngestReport {
    // r[impl cairn.preserves_evidence_ingest.validation]
    let mut issues = Vec::new();
    validate_required_fields(&input.row, &mut issues);
    validate_measurement(input, &mut issues);
    validate_supported_contract(&input.row, &mut issues);
    validate_adapter_policy(&input.row, &mut issues);
    validate_non_claims(&input.row, &mut issues);
    sort_preserves_evidence_issues(&mut issues);

    let valid = issues.is_empty();
    PreservesEvidenceIngestReport {
        valid,
        verdict: if valid { "PASS" } else { "FAIL" }.to_string(),
        envelope_path: input.row.envelope_path.clone(),
        canonical_digest: input.measurement.canonical_digest.clone(),
        payload_opacity: input.row.payload_opacity.clone(),
        adapter_id: normalized_adapter_id(&input.row.adapter_id),
        issues,
        claim: if input.row.claim.is_empty() {
            PRESERVES_EVIDENCE_INGEST_BOUNDARY.to_string()
        } else {
            input.row.claim.clone()
        },
        boundary: PRESERVES_EVIDENCE_INGEST_BOUNDARY.to_string(),
    }
}

fn validate_required_fields(
    row: &PreservesEvidenceEnvelopeRow,
    issues: &mut Vec<PreservesEvidenceIngestIssue>,
) {
    for (field_path, value) in [
        (FIELD_ENVELOPE_PATH, row.envelope_path.as_str()),
        (FIELD_SCHEMA_LABEL, row.schema_label.as_str()),
        (FIELD_PRODUCER_REPO, row.producer_repo.as_str()),
        (FIELD_CANONICAL_DIGEST, row.canonical_digest.as_str()),
        (FIELD_EVIDENCE_ROLE, row.evidence_role.as_str()),
        (FIELD_CLAIM_SCOPE, row.claim_scope.as_str()),
        (FIELD_PAYLOAD_OPACITY, row.payload_opacity.as_str()),
    ] {
        if value.trim().is_empty() {
            issues.push(preserves_evidence_issue(
                PRESERVES_EVIDENCE_ISSUE_EMPTY_FIELD,
                field_path,
                "Preserves evidence ingest row requires this field",
            ));
        }
    }
}

fn validate_measurement(
    input: &PreservesEvidenceIngestInput,
    issues: &mut Vec<PreservesEvidenceIngestIssue>,
) {
    if input.row.envelope_path != input.measurement.envelope_path {
        issues.push(preserves_evidence_issue(
            PRESERVES_EVIDENCE_ISSUE_STALE_DIGEST,
            FIELD_ENVELOPE_PATH,
            "measurement path must match manifest envelope path",
        ));
    }
    if !input.measurement.canonical_bytes {
        issues.push(preserves_evidence_issue(
            PRESERVES_EVIDENCE_ISSUE_NON_CANONICAL_BYTES,
            FIELD_CANONICAL_DIGEST,
            "measured envelope bytes must match canonical Preserves bytes",
        ));
    }
    if !is_blake3_hex(&input.row.canonical_digest) {
        issues.push(preserves_evidence_issue(
            PRESERVES_EVIDENCE_ISSUE_MALFORMED_DIGEST,
            FIELD_CANONICAL_DIGEST,
            EXPECTED_BLAKE3_DIGEST,
        ));
        return;
    }
    if !is_blake3_hex(&input.measurement.canonical_digest) {
        issues.push(preserves_evidence_issue(
            PRESERVES_EVIDENCE_ISSUE_MALFORMED_DIGEST,
            "measurement.canonical_digest",
            EXPECTED_BLAKE3_DIGEST,
        ));
        return;
    }
    if input.row.canonical_digest != input.measurement.canonical_digest {
        issues.push(preserves_evidence_issue(
            PRESERVES_EVIDENCE_ISSUE_STALE_DIGEST,
            FIELD_CANONICAL_DIGEST,
            "manifest canonical digest must match measured Preserves identity",
        ));
    }
}

fn validate_supported_contract(
    row: &PreservesEvidenceEnvelopeRow,
    issues: &mut Vec<PreservesEvidenceIngestIssue>,
) {
    if !SUPPORTED_SCHEMA_LABELS.contains(&row.schema_label.as_str()) {
        issues.push(preserves_evidence_issue(
            PRESERVES_EVIDENCE_ISSUE_UNSUPPORTED_SCHEMA,
            FIELD_SCHEMA_LABEL,
            "schema label is not registered for Preserves evidence ingest",
        ));
    }
    if !SUPPORTED_ROLES.contains(&row.evidence_role.as_str()) {
        issues.push(preserves_evidence_issue(
            PRESERVES_EVIDENCE_ISSUE_UNSUPPORTED_ROLE,
            FIELD_EVIDENCE_ROLE,
            "evidence role is not registered for Preserves evidence ingest",
        ));
    }
    if !SUPPORTED_CLAIM_SCOPES.contains(&row.claim_scope.as_str()) {
        issues.push(preserves_evidence_issue(
            PRESERVES_EVIDENCE_ISSUE_UNSUPPORTED_SCOPE,
            FIELD_CLAIM_SCOPE,
            "claim scope is not registered for Preserves evidence ingest",
        ));
    }
    if !SUPPORTED_OPACITY_MODES.contains(&row.payload_opacity.as_str()) {
        issues.push(preserves_evidence_issue(
            PRESERVES_EVIDENCE_ISSUE_UNSUPPORTED_OPACITY,
            FIELD_PAYLOAD_OPACITY,
            "payload opacity mode must be opaque or adapter-backed",
        ));
    }
    if !matches_registered_contract(row) {
        issues.push(preserves_evidence_issue(
            PRESERVES_EVIDENCE_ISSUE_UNSUPPORTED_CONTRACT,
            FIELD_SCHEMA_LABEL,
            "schema, role, opacity, adapter, and claim scope must match one registered ingest contract",
        ));
    }
}

fn matches_registered_contract(row: &PreservesEvidenceEnvelopeRow) -> bool {
    let adapter_id = normalized_adapter_id(&row.adapter_id);
    preserves_evidence_ingest_contracts()
        .iter()
        .any(|contract| {
            row.schema_label == contract.schema_label
                && row.evidence_role == contract.evidence_role
                && row.payload_opacity == contract.payload_opacity
                && adapter_id == contract.adapter_id
                && row.claim_scope == contract.claim_scope
        })
}

fn validate_adapter_policy(
    row: &PreservesEvidenceEnvelopeRow,
    issues: &mut Vec<PreservesEvidenceIngestIssue>,
) {
    let adapter_id = normalized_adapter_id(&row.adapter_id);
    match row.payload_opacity.as_str() {
        PRESERVES_EVIDENCE_OPACITY_OPAQUE => {
            if adapter_id.is_some() {
                issues.push(preserves_evidence_issue(
                    PRESERVES_EVIDENCE_ISSUE_ADAPTER_FOR_OPAQUE,
                    FIELD_ADAPTER_ID,
                    "opaque Preserves evidence must not declare a Cairn adapter",
                ));
            }
            if row.evidence_role != PRESERVES_EVIDENCE_ROLE_OPAQUE {
                issues.push(preserves_evidence_issue(
                    PRESERVES_EVIDENCE_ISSUE_UNSUPPORTED_ROLE,
                    FIELD_EVIDENCE_ROLE,
                    "opaque evidence must use the opaque carrier role",
                ));
            }
        }
        PRESERVES_EVIDENCE_OPACITY_ADAPTER_BACKED => {
            let Some(adapter) = adapter_id else {
                issues.push(preserves_evidence_issue(
                    PRESERVES_EVIDENCE_ISSUE_ADAPTER_REQUIRED,
                    FIELD_ADAPTER_ID,
                    "adapter-backed Preserves evidence requires an adapter id",
                ));
                return;
            };
            if !SUPPORTED_ADAPTER_IDS.contains(&adapter.as_str()) {
                issues.push(preserves_evidence_issue(
                    PRESERVES_EVIDENCE_ISSUE_UNSUPPORTED_ADAPTER,
                    FIELD_ADAPTER_ID,
                    "adapter id is not registered for Preserves evidence ingest",
                ));
            }
            if row.evidence_role != PRESERVES_EVIDENCE_ROLE_ADAPTER_BACKED {
                issues.push(preserves_evidence_issue(
                    PRESERVES_EVIDENCE_ISSUE_UNSUPPORTED_ROLE,
                    FIELD_EVIDENCE_ROLE,
                    "adapter-backed evidence must use the adapter-backed carrier role",
                ));
            }
        }
        _ => {}
    }
}

fn validate_non_claims(
    row: &PreservesEvidenceEnvelopeRow,
    issues: &mut Vec<PreservesEvidenceIngestIssue>,
) {
    if row.non_claims.is_empty() {
        issues.push(preserves_evidence_issue(
            PRESERVES_EVIDENCE_ISSUE_WEAK_NON_CLAIM,
            FIELD_NON_CLAIMS,
            "Preserves evidence ingest requires explicit non-claims",
        ));
    }
    let joined = row.non_claims.join("; ").to_ascii_lowercase();
    for fragment in REQUIRED_NON_CLAIM_FRAGMENTS {
        if !joined.contains(fragment) {
            issues.push(preserves_evidence_issue(
                PRESERVES_EVIDENCE_ISSUE_WEAK_NON_CLAIM,
                FIELD_NON_CLAIMS,
                &format!("non-claims must state that ingest does not prove {fragment}"),
            ));
        }
    }
    let claim = row.claim.to_ascii_lowercase();
    for phrase in OVERCLAIM_PHRASES {
        if claim.contains(phrase) {
            issues.push(preserves_evidence_issue(
                PRESERVES_EVIDENCE_ISSUE_OVERCLAIM,
                FIELD_CLAIM,
                "Preserves evidence ingest claim promotes carrier validation beyond its boundary",
            ));
        }
    }
}

fn normalized_adapter_id(adapter_id: &Option<String>) -> Option<String> {
    adapter_id
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| *value != EMPTY_ADAPTER_ID)
        .map(str::to_string)
}

fn is_blake3_hex(value: &str) -> bool {
    value.len() == BLAKE3_HEX_LENGTH
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn preserves_evidence_issue(
    code: &str,
    field_path: &str,
    message: &str,
) -> PreservesEvidenceIngestIssue {
    PreservesEvidenceIngestIssue {
        code: code.to_string(),
        field_path: field_path.to_string(),
        message: message.to_string(),
    }
}

fn sort_preserves_evidence_issues(issues: &mut [PreservesEvidenceIngestIssue]) {
    issues.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then(left.field_path.cmp(&right.field_path))
            .then(left.message.cmp(&right.message))
    });
}

#[cfg(test)]
mod tests {

    use super::*;

    const ENVELOPE_PATH: &str = "evidence/preserves/function-address.preserves";

    fn envelope_value() -> serde_json::Value {
        serde_json::json!({
            "schema": PRESERVES_EVIDENCE_OPAQUE_SCHEMA_V1,
            "producer_repo": "kamacite",
            "evidence_role": PRESERVES_EVIDENCE_ROLE_OPAQUE,
            "claim_scope": PRESERVES_EVIDENCE_CLAIM_SCOPE_LIFECYCLE,
            "payload": {
                "receipt_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "summary": "opaque external evidence payload"
            }
        })
    }

    fn row_for(
        schema_label: &str,
        measurement: &PreservesEvidenceEnvelopeMeasurement,
    ) -> PreservesEvidenceEnvelopeRow {
        PreservesEvidenceEnvelopeRow {
            envelope_path: ENVELOPE_PATH.to_string(),
            schema_label: schema_label.to_string(),
            producer_repo: "kamacite".to_string(),
            canonical_digest: measurement.canonical_digest.clone(),
            evidence_role: PRESERVES_EVIDENCE_ROLE_OPAQUE.to_string(),
            claim_scope: PRESERVES_EVIDENCE_CLAIM_SCOPE_LIFECYCLE.to_string(),
            adapter_id: None,
            payload_opacity: PRESERVES_EVIDENCE_OPACITY_OPAQUE.to_string(),
            non_claims: default_preserves_evidence_non_claims(),
            claim: PRESERVES_EVIDENCE_INGEST_BOUNDARY.to_string(),
        }
    }

    fn valid_input() -> PreservesEvidenceIngestInput {
        let value = envelope_value();
        let source_bytes = super::super::evidence::canonical_preserves_bytes(&value)
            .expect("canonical Preserves bytes");
        let measurement =
            measure_preserves_evidence_envelope_bytes(ENVELOPE_PATH, &source_bytes, &value)
                .expect("measurement succeeds");
        PreservesEvidenceIngestInput {
            row: row_for(PRESERVES_EVIDENCE_OPAQUE_SCHEMA_V1, &measurement),
            measurement,
        }
    }

    fn issue_codes(report: &PreservesEvidenceIngestReport) -> Vec<String> {
        report
            .issues
            .iter()
            .map(|issue| issue.code.clone())
            .collect()
    }

    #[test]
    fn contract_documents_supported_preserves_envelopes() {
        // r[verify cairn.preserves_evidence_ingest.contract]
        let contracts = preserves_evidence_ingest_contracts();

        assert!(contracts
            .iter()
            .any(|contract| contract.schema_label == PRESERVES_EVIDENCE_OPAQUE_SCHEMA_V1));
        assert!(contracts
            .iter()
            .any(|contract| contract.schema_label == PRESERVES_EVIDENCE_ADAPTER_SCHEMA_V1));
        assert!(contracts
            .iter()
            .any(|contract| contract.payload_opacity == PRESERVES_EVIDENCE_OPACITY_OPAQUE));
        assert!(contracts
            .iter()
            .any(|contract| contract.adapter_id.as_deref()
                == Some(PRESERVES_EVIDENCE_ADAPTER_LIFECYCLE_RECEIPT)));
    }

    #[test]
    fn canonical_opaque_and_adapter_backed_envelopes_pass() {
        // r[verify cairn.preserves_evidence_ingest.fixtures.positive]
        // r[verify cairn.preserves_evidence_ingest.validation]
        // r[verify cairn.preserves_evidence_ingest.validation.opaque]
        let opaque = validate_preserves_evidence_ingest(&valid_input());
        assert!(opaque.valid, "{:?}", opaque.issues);
        assert_eq!(opaque.payload_opacity, PRESERVES_EVIDENCE_OPACITY_OPAQUE);
        assert!(opaque.boundary.contains("carrier identity"));

        let mut adapter_input = valid_input();
        adapter_input.row.schema_label = PRESERVES_EVIDENCE_ADAPTER_SCHEMA_V1.to_string();
        adapter_input.row.evidence_role = PRESERVES_EVIDENCE_ROLE_ADAPTER_BACKED.to_string();
        adapter_input.row.payload_opacity = PRESERVES_EVIDENCE_OPACITY_ADAPTER_BACKED.to_string();
        adapter_input.row.adapter_id =
            Some(PRESERVES_EVIDENCE_ADAPTER_LIFECYCLE_RECEIPT.to_string());
        let adapter_backed = validate_preserves_evidence_ingest(&adapter_input);
        assert!(adapter_backed.valid, "{:?}", adapter_backed.issues);
        assert_eq!(
            adapter_backed.adapter_id.as_deref(),
            Some(PRESERVES_EVIDENCE_ADAPTER_LIFECYCLE_RECEIPT)
        );
    }

    #[test]
    fn malformed_preserves_envelopes_fail_closed() {
        // r[verify cairn.preserves_evidence_ingest.fixtures.negative]
        // r[verify cairn.preserves_evidence_ingest.final_validation]
        // r[verify cairn.preserves_evidence_ingest.final_validation.fixtures]
        let mut input = valid_input();
        input.row.schema_label = "unknown.preserves.schema.v0".to_string();
        input.row.canonical_digest = "B3:not-a-digest".to_string();
        input.row.evidence_role = "proof-of-runtime-correctness".to_string();
        input.row.claim_scope = "universal-release-eligibility".to_string();
        input.measurement.canonical_bytes = false;
        input.row.non_claims = vec!["does not prove runtime correctness".to_string()];
        input.row.claim =
            "Preserves ingest proves evidence truth and producer authorization".to_string();

        let report = validate_preserves_evidence_ingest(&input);
        let codes = issue_codes(&report);
        assert!(!report.valid);
        assert!(codes.contains(&PRESERVES_EVIDENCE_ISSUE_UNSUPPORTED_SCHEMA.to_string()));
        assert!(codes.contains(&PRESERVES_EVIDENCE_ISSUE_MALFORMED_DIGEST.to_string()));
        assert!(codes.contains(&PRESERVES_EVIDENCE_ISSUE_NON_CANONICAL_BYTES.to_string()));
        assert!(codes.contains(&PRESERVES_EVIDENCE_ISSUE_UNSUPPORTED_ROLE.to_string()));
        assert!(codes.contains(&PRESERVES_EVIDENCE_ISSUE_UNSUPPORTED_SCOPE.to_string()));
        assert!(codes.contains(&PRESERVES_EVIDENCE_ISSUE_WEAK_NON_CLAIM.to_string()));
        assert!(codes.contains(&PRESERVES_EVIDENCE_ISSUE_OVERCLAIM.to_string()));
    }

    #[test]
    fn source_bytes_must_match_canonical_preserves_encoding() {
        // r[verify cairn.preserves_evidence_ingest.fixtures.negative]
        // r[verify cairn.preserves_evidence_ingest.final_validation.fixtures]
        let value = envelope_value();
        let non_canonical_bytes = b"not-canonical-preserves";
        let measurement =
            measure_preserves_evidence_envelope_bytes(ENVELOPE_PATH, non_canonical_bytes, &value)
                .expect("measurement succeeds");
        let report = validate_preserves_evidence_ingest(&PreservesEvidenceIngestInput {
            row: row_for(PRESERVES_EVIDENCE_OPAQUE_SCHEMA_V1, &measurement),
            measurement,
        });

        assert!(!report.valid);
        assert!(issue_codes(&report)
            .contains(&PRESERVES_EVIDENCE_ISSUE_NON_CANONICAL_BYTES.to_string()));
    }

    #[test]
    fn registered_contract_fields_cannot_be_recombined() {
        // r[verify cairn.preserves_evidence_ingest.fixtures.negative]
        let mut schema_mismatch = valid_input();
        schema_mismatch.row.schema_label = PRESERVES_EVIDENCE_ADAPTER_SCHEMA_V1.to_string();
        let schema_report = validate_preserves_evidence_ingest(&schema_mismatch);
        assert!(!schema_report.valid);
        assert!(issue_codes(&schema_report)
            .contains(&PRESERVES_EVIDENCE_ISSUE_UNSUPPORTED_CONTRACT.to_string()));

        let mut adapter_mismatch = valid_input();
        adapter_mismatch.row.payload_opacity =
            PRESERVES_EVIDENCE_OPACITY_ADAPTER_BACKED.to_string();
        adapter_mismatch.row.evidence_role = PRESERVES_EVIDENCE_ROLE_ADAPTER_BACKED.to_string();
        adapter_mismatch.row.adapter_id =
            Some(PRESERVES_EVIDENCE_ADAPTER_LIFECYCLE_RECEIPT.to_string());
        let adapter_report = validate_preserves_evidence_ingest(&adapter_mismatch);
        assert!(!adapter_report.valid);
        assert!(issue_codes(&adapter_report)
            .contains(&PRESERVES_EVIDENCE_ISSUE_UNSUPPORTED_CONTRACT.to_string()));
    }

    #[test]
    fn stale_digest_and_adapter_policy_fail_closed() {
        // r[verify cairn.preserves_evidence_ingest.fixtures.negative]
        let mut stale = valid_input();
        stale.row.canonical_digest =
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string();
        let stale_report = validate_preserves_evidence_ingest(&stale);
        assert!(!stale_report.valid);
        assert!(
            issue_codes(&stale_report).contains(&PRESERVES_EVIDENCE_ISSUE_STALE_DIGEST.to_string())
        );

        let mut missing_adapter = valid_input();
        missing_adapter.row.schema_label = PRESERVES_EVIDENCE_ADAPTER_SCHEMA_V1.to_string();
        missing_adapter.row.payload_opacity = PRESERVES_EVIDENCE_OPACITY_ADAPTER_BACKED.to_string();
        missing_adapter.row.evidence_role = PRESERVES_EVIDENCE_ROLE_ADAPTER_BACKED.to_string();
        let missing_adapter_report = validate_preserves_evidence_ingest(&missing_adapter);
        assert!(!missing_adapter_report.valid);
        assert!(issue_codes(&missing_adapter_report)
            .contains(&PRESERVES_EVIDENCE_ISSUE_ADAPTER_REQUIRED.to_string()));

        let mut opaque_with_adapter = valid_input();
        opaque_with_adapter.row.adapter_id =
            Some(PRESERVES_EVIDENCE_ADAPTER_LIFECYCLE_RECEIPT.to_string());
        let opaque_adapter_report = validate_preserves_evidence_ingest(&opaque_with_adapter);
        assert!(!opaque_adapter_report.valid);
        assert!(issue_codes(&opaque_adapter_report)
            .contains(&PRESERVES_EVIDENCE_ISSUE_ADAPTER_FOR_OPAQUE.to_string()));
    }

    #[test]
    fn non_claim_boundary_is_visible() {
        // r[verify cairn.preserves_evidence_ingest.docs]
        // r[verify cairn.preserves_evidence_ingest.docs.non_claims]
        let report = validate_preserves_evidence_ingest(&valid_input());

        assert!(report.valid);
        assert!(report.claim.contains("does not prove evidence truth"));
        assert!(report.claim.contains("producer authorization"));
        assert!(report.boundary.contains("transport correctness"));
    }
}
