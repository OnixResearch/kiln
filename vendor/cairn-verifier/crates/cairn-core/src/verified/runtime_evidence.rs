pub const RUNTIME_EVIDENCE_SCHEMA_V1: &str = "cairn.runtime_evidence.v1";
pub const RUNTIME_EVIDENCE_BOUNDARY: &str = "runtime evidence proves presented-chain integrity and witnessed-prefix comparison only; it does not prove agent correctness, action policy enforcement, witness authority trust, or completeness without an independently controlled checkpoint";
pub const RUNTIME_HASH_ALGORITHM_BLAKE3: &str = "BLAKE3";
pub const CAPTURE_ORIGIN_BOUNDARY: &str = "boundary_captured";
pub const CAPTURE_ORIGIN_INGESTED: &str = "ingested_telemetry";
pub const CAPTURE_ORIGIN_WRAPPER_ASSERTED: &str = "wrapper_asserted";
pub const CAPTURE_ORIGIN_UNKNOWN: &str = "unknown";
pub const COMPLETENESS_INTEGRITY_ONLY: &str = "integrity_only";
pub const COMPLETENESS_WITNESSED_PREFIX: &str = "witnessed_prefix";
pub const COMPLETENESS_SUBJECT_MISMATCH: &str = "subject_mismatch";
pub const COMPLETENESS_TRUNCATED: &str = "truncated";
pub const COMPLETENESS_ALTERED_BEFORE_CHECKPOINT: &str = "altered_before_checkpoint";
pub const COMPLETENESS_CHECKPOINT_MALFORMED: &str = "checkpoint_malformed";
pub const DEFAULT_RUNTIME_SUMMARY_MAX_BYTES: u64 = 256;

const BLAKE3_HEX_LENGTH: usize = 64;
const RECORD_HASH_FIELD: &str = "record_hash";
const CHECKPOINT_HASH_FIELD: &str = "checkpoint_hash";
const MULTI_SUBJECT_AGGREGATION_PROFILE: &str = "multi_subject";
const ACTION_CLASSES: &[&str] = &[
    "tool_call",
    "model_call",
    "command_run",
    "file_write",
    "evidence_export",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEvidencePolicy {
    pub allowed_capture_origins: Vec<String>,
    pub allowed_hash_algorithms: Vec<String>,
    pub max_summary_bytes: u64,
    pub forbidden_summary_fragments: Vec<String>,
    pub aggregation_profiles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEvidenceIssue {
    pub code: String,
    pub field_path: String,
    pub expected: String,
    pub actual: String,
}

impl RuntimeEvidenceIssue {
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "code": self.code,
            "field_path": self.field_path,
            "expected": self.expected,
            "actual": self.actual,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEvidenceReport {
    pub valid: bool,
    pub verdict: String,
    pub subject: Option<String>,
    pub record_count: u64,
    pub chain_root_hash: Option<String>,
    pub head_hash: Option<String>,
    pub checkpoint_hash: Option<String>,
    pub completeness_verdict: String,
    pub capture_origin_summary: CaptureOriginSummary,
    pub issues: Vec<RuntimeEvidenceIssue>,
    pub boundary: String,
}

impl RuntimeEvidenceReport {
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "schema_version": RUNTIME_EVIDENCE_SCHEMA_V1,
            "valid": self.valid,
            "verdict": self.verdict,
            "subject": self.subject,
            "record_count": self.record_count,
            "chain_root_hash": self.chain_root_hash,
            "head_hash": self.head_hash,
            "checkpoint_hash": self.checkpoint_hash,
            "completeness": {
                "verdict": self.completeness_verdict,
                "witnessed": self.completeness_verdict == COMPLETENESS_WITNESSED_PREFIX,
            },
            "capture_origin_summary": self.capture_origin_summary.to_json(),
            "issues": self.issues.iter().map(RuntimeEvidenceIssue::to_json).collect::<Vec<_>>(),
            "boundary": self.boundary,
        })
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CaptureOriginSummary {
    pub boundary_captured: u64,
    pub ingested_telemetry: u64,
    pub wrapper_asserted: u64,
    pub unknown: u64,
}

impl CaptureOriginSummary {
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            CAPTURE_ORIGIN_BOUNDARY: self.boundary_captured,
            CAPTURE_ORIGIN_INGESTED: self.ingested_telemetry,
            CAPTURE_ORIGIN_WRAPPER_ASSERTED: self.wrapper_asserted,
            CAPTURE_ORIGIN_UNKNOWN: self.unknown,
        })
    }

    fn record(&mut self, origin: &str) {
        match origin {
            CAPTURE_ORIGIN_BOUNDARY => self.boundary_captured += 1,
            CAPTURE_ORIGIN_INGESTED => self.ingested_telemetry += 1,
            CAPTURE_ORIGIN_WRAPPER_ASSERTED => self.wrapper_asserted += 1,
            _ => self.unknown += 1,
        }
    }
}

#[must_use]
pub fn runtime_record_hash(record: &serde_json::Value) -> String {
    let mut material = record.clone();
    if let Some(object) = material.as_object_mut() {
        object.remove(RECORD_HASH_FIELD);
    }
    crate::canonical_json_hash(&material)
}

#[must_use]
pub fn witness_checkpoint_hash(checkpoint: &serde_json::Value) -> String {
    let mut material = checkpoint.clone();
    if let Some(object) = material.as_object_mut() {
        object.remove(CHECKPOINT_HASH_FIELD);
    }
    crate::canonical_json_hash(&material)
}

#[must_use]
pub fn verify_runtime_evidence(
    policy: &RuntimeEvidencePolicy,
    records: &[serde_json::Value],
    checkpoints: &[serde_json::Value],
) -> RuntimeEvidenceReport {
    // r[impl cairn.runtime_evidence.records]
    // r[impl cairn.runtime_evidence.subject_scope]
    // r[impl cairn.runtime_evidence.capture_origin]
    // r[impl cairn.runtime_evidence.hash_only]
    // r[impl cairn.runtime_evidence.external_witness]
    // r[impl cairn.runtime_evidence.completeness_verdicts]
    let mut issues = Vec::new();
    let mut summary = CaptureOriginSummary::default();
    let mut subject: Option<String> = None;
    let mut computed_hashes = Vec::new();
    let mut previous_hash: Option<String> = None;

    if records.is_empty() {
        issues.push(runtime_issue(
            "runtime_evidence.chain_empty",
            "records",
            "non-empty runtime action chain",
            "empty",
        ));
    }

    for (index, record) in records.iter().enumerate() {
        validate_record(
            policy,
            record,
            index,
            &mut subject,
            &mut summary,
            &previous_hash,
            &mut issues,
        );
        let computed = runtime_record_hash(record);
        previous_hash = Some(computed.clone());
        computed_hashes.push(computed);
    }

    validate_subject_scope(policy, records, &mut issues);
    let chain_root_hash = computed_hashes.first().cloned();
    let head_hash = computed_hashes.last().cloned();
    let (completeness_verdict, checkpoint_hash) = compare_checkpoints(
        checkpoints,
        subject.as_deref(),
        &chain_root_hash,
        &computed_hashes,
        &mut issues,
    );
    issues.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then(left.field_path.cmp(&right.field_path))
    });
    let valid = issues.is_empty();
    RuntimeEvidenceReport {
        valid,
        verdict: if valid { "pass" } else { "fail" }.to_string(),
        subject,
        record_count: records.len() as u64,
        chain_root_hash,
        head_hash,
        checkpoint_hash,
        completeness_verdict,
        capture_origin_summary: summary,
        issues,
        boundary: RUNTIME_EVIDENCE_BOUNDARY.to_string(),
    }
}

fn validate_record(
    policy: &RuntimeEvidencePolicy,
    record: &serde_json::Value,
    index: usize,
    subject: &mut Option<String>,
    summary: &mut CaptureOriginSummary,
    previous_hash: &Option<String>,
    issues: &mut Vec<RuntimeEvidenceIssue>,
) {
    let path = |field: &str| format!("records[{index}].{field}");
    if !record.is_object() {
        issues.push(runtime_issue(
            "runtime_evidence.record_not_object",
            &format!("records[{index}]"),
            "object runtime action record",
            "non-object",
        ));
        return;
    }

    let current_subject = required_string(record, &path("subject"), issues);
    if subject.is_none() {
        *subject = current_subject.clone();
    }

    let action_class = required_string(record, &path("action_class"), issues).unwrap_or_default();
    if !ACTION_CLASSES.contains(&action_class.as_str()) {
        issues.push(runtime_issue(
            "runtime_evidence.action_class_unsupported",
            &path("action_class"),
            "supported runtime action class",
            if action_class.is_empty() {
                "missing"
            } else {
                "unsupported"
            },
        ));
    }

    let origin = required_string(record, &path("capture_origin"), issues).unwrap_or_default();
    summary.record(&origin);
    if !policy
        .allowed_capture_origins
        .iter()
        .any(|allowed| allowed == &origin)
    {
        issues.push(runtime_issue(
            "runtime_evidence.capture_origin_unknown",
            &path("capture_origin"),
            "policy-declared capture origin",
            if origin.is_empty() {
                "missing"
            } else {
                "unknown"
            },
        ));
    }

    required_string(record, &path("record_id"), issues);
    required_string(record, &path("authorization"), issues);
    validate_identity(
        policy,
        record.get("input_identity"),
        &path("input_identity"),
        issues,
    );
    validate_identity(
        policy,
        record.get("outcome_identity"),
        &path("outcome_identity"),
        issues,
    );
    validate_integrity(record, &path("integrity"), previous_hash, issues);
    validate_non_claims(record, &path("non_claims"), issues);

    let computed_hash = runtime_record_hash(record);
    match record
        .get(RECORD_HASH_FIELD)
        .and_then(serde_json::Value::as_str)
    {
        Some(hash) if hash == computed_hash => {}
        Some(_) => issues.push(runtime_issue(
            "runtime_evidence.record_hash_mismatch",
            &path(RECORD_HASH_FIELD),
            "BLAKE3 hash of canonical record material",
            "mismatch",
        )),
        None => issues.push(runtime_issue(
            "runtime_evidence.record_hash_missing",
            &path(RECORD_HASH_FIELD),
            "BLAKE3 hash of canonical record material",
            "missing",
        )),
    }
}

fn validate_subject_scope(
    policy: &RuntimeEvidencePolicy,
    records: &[serde_json::Value],
    issues: &mut Vec<RuntimeEvidenceIssue>,
) {
    if policy
        .aggregation_profiles
        .iter()
        .any(|profile| profile == MULTI_SUBJECT_AGGREGATION_PROFILE)
    {
        return;
    }
    let mut seen_subject: Option<&str> = None;
    for record in records {
        let Some(subject) = record.get("subject").and_then(serde_json::Value::as_str) else {
            continue;
        };
        match seen_subject {
            Some(first) if first != subject => {
                issues.push(runtime_issue(
                    "runtime_evidence.subject_mixed",
                    "records[].subject",
                    "single-subject runtime chain",
                    "mixed-subject",
                ));
                return;
            }
            None => seen_subject = Some(subject),
            _ => {}
        }
    }
}

fn validate_identity(
    policy: &RuntimeEvidencePolicy,
    identity: Option<&serde_json::Value>,
    path: &str,
    issues: &mut Vec<RuntimeEvidenceIssue>,
) {
    let Some(identity) = identity else {
        issues.push(runtime_issue(
            "runtime_evidence.identity_missing",
            path,
            "bounded hash identity object",
            "missing",
        ));
        return;
    };
    let algorithm = identity
        .get("hash_algorithm")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if !policy
        .allowed_hash_algorithms
        .iter()
        .any(|allowed| allowed == algorithm)
    {
        issues.push(runtime_issue(
            "runtime_evidence.hash_algorithm_unsupported",
            &format!("{path}.hash_algorithm"),
            "policy-declared hash algorithm",
            if algorithm.is_empty() {
                "missing"
            } else {
                "unsupported"
            },
        ));
    }
    let hash = identity
        .get("hash")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if algorithm == RUNTIME_HASH_ALGORITHM_BLAKE3 && !is_blake3_hex(hash) {
        issues.push(runtime_issue(
            "runtime_evidence.identity_hash_invalid",
            &format!("{path}.hash"),
            "BLAKE3 hex identity",
            if hash.is_empty() {
                "missing"
            } else {
                "invalid"
            },
        ));
    }
    if let Some(summary) = identity.get("summary").and_then(serde_json::Value::as_str) {
        if summary.len() as u64 > policy.max_summary_bytes {
            issues.push(runtime_issue(
                "runtime_evidence.summary_too_large",
                &format!("{path}.summary"),
                "bounded redacted summary",
                "too-large",
            ));
        }
        if policy
            .forbidden_summary_fragments
            .iter()
            .any(|fragment| summary.contains(fragment))
        {
            issues.push(runtime_issue(
                "runtime_evidence.summary_secret_like",
                &format!("{path}.summary"),
                "redacted or hash-only summary",
                "secret-bearing",
            ));
        }
    }
}

fn validate_integrity(
    record: &serde_json::Value,
    path: &str,
    previous_hash: &Option<String>,
    issues: &mut Vec<RuntimeEvidenceIssue>,
) {
    let Some(integrity) = record.get("integrity") else {
        issues.push(runtime_issue(
            "runtime_evidence.integrity_missing",
            path,
            "integrity link object",
            "missing",
        ));
        return;
    };
    let declared_previous = integrity
        .get("previous_hash")
        .and_then(serde_json::Value::as_str);
    match previous_hash {
        Some(expected) if declared_previous == Some(expected.as_str()) => {}
        Some(_) => issues.push(runtime_issue(
            "runtime_evidence.previous_hash_mismatch",
            &format!("{path}.previous_hash"),
            "previous record hash",
            "mismatch",
        )),
        None if declared_previous.is_none() => {}
        None => issues.push(runtime_issue(
            "runtime_evidence.previous_hash_unexpected",
            &format!("{path}.previous_hash"),
            "null first-record previous hash",
            "unexpected",
        )),
    }
}

fn validate_non_claims(
    record: &serde_json::Value,
    path: &str,
    issues: &mut Vec<RuntimeEvidenceIssue>,
) {
    let Some(non_claims) = record
        .get("non_claims")
        .and_then(serde_json::Value::as_array)
    else {
        issues.push(runtime_issue(
            "runtime_evidence.non_claim_boundary_missing",
            path,
            "runtime evidence non-claim boundary",
            "missing",
        ));
        return;
    };
    if !non_claims
        .iter()
        .any(|claim| claim.as_str() == Some(RUNTIME_EVIDENCE_BOUNDARY))
    {
        issues.push(runtime_issue(
            "runtime_evidence.non_claim_boundary_missing",
            path,
            "runtime evidence non-claim boundary",
            "missing",
        ));
    }
}

fn compare_checkpoints(
    checkpoints: &[serde_json::Value],
    subject: Option<&str>,
    chain_root_hash: &Option<String>,
    computed_hashes: &[String],
    issues: &mut Vec<RuntimeEvidenceIssue>,
) -> (String, Option<String>) {
    let Some(checkpoint) = checkpoints.first() else {
        return (COMPLETENESS_INTEGRITY_ONLY.to_string(), None);
    };
    let computed_checkpoint_hash = witness_checkpoint_hash(checkpoint);
    if checkpoint
        .get(CHECKPOINT_HASH_FIELD)
        .and_then(serde_json::Value::as_str)
        != Some(computed_checkpoint_hash.as_str())
    {
        issues.push(runtime_issue(
            "runtime_evidence.checkpoint_hash_mismatch",
            CHECKPOINT_HASH_FIELD,
            "BLAKE3 hash of canonical checkpoint material",
            "mismatch",
        ));
        return (
            COMPLETENESS_CHECKPOINT_MALFORMED.to_string(),
            Some(computed_checkpoint_hash),
        );
    }
    let checkpoint_subject = checkpoint
        .get("subject")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if subject != Some(checkpoint_subject) {
        issues.push(runtime_issue(
            "runtime_evidence.checkpoint_subject_mismatch",
            "checkpoint.subject",
            "same subject as runtime chain",
            "subject-mismatch",
        ));
        return (
            COMPLETENESS_SUBJECT_MISMATCH.to_string(),
            Some(computed_checkpoint_hash),
        );
    }
    let record_count = checkpoint
        .get("record_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default();
    let checkpoint_record_count = match usize::try_from(record_count) {
        Ok(count) if count > 0 && count <= computed_hashes.len() => count,
        _ => {
            issues.push(runtime_issue(
                "runtime_evidence.checkpoint_truncated_chain",
                "checkpoint.record_count",
                "checkpoint count within presented chain",
                "truncated",
            ));
            return (
                COMPLETENESS_TRUNCATED.to_string(),
                Some(computed_checkpoint_hash),
            );
        }
    };
    if checkpoint
        .get("chain_root_hash")
        .and_then(serde_json::Value::as_str)
        != chain_root_hash.as_deref()
    {
        issues.push(runtime_issue(
            "runtime_evidence.checkpoint_chain_root_mismatch",
            "checkpoint.chain_root_hash",
            "matching chain root hash",
            "mismatch",
        ));
        return (
            COMPLETENESS_ALTERED_BEFORE_CHECKPOINT.to_string(),
            Some(computed_checkpoint_hash),
        );
    }
    let prefix_head = computed_hashes
        .get(checkpoint_record_count - 1)
        .map(String::as_str);
    if checkpoint
        .get("head_hash")
        .and_then(serde_json::Value::as_str)
        != prefix_head
    {
        issues.push(runtime_issue(
            "runtime_evidence.checkpoint_head_mismatch",
            "checkpoint.head_hash",
            "matching checkpoint prefix head hash",
            "altered-before-checkpoint",
        ));
        return (
            COMPLETENESS_ALTERED_BEFORE_CHECKPOINT.to_string(),
            Some(computed_checkpoint_hash),
        );
    }
    (
        COMPLETENESS_WITNESSED_PREFIX.to_string(),
        Some(computed_checkpoint_hash),
    )
}

fn required_string(
    value: &serde_json::Value,
    path: &str,
    issues: &mut Vec<RuntimeEvidenceIssue>,
) -> Option<String> {
    match value
        .get(path.rsplit('.').next().unwrap_or(path))
        .and_then(serde_json::Value::as_str)
    {
        Some(text) if !text.is_empty() => Some(text.to_string()),
        _ => {
            issues.push(runtime_issue(
                "runtime_evidence.required_field_missing",
                path,
                "non-empty string",
                "missing",
            ));
            None
        }
    }
}

fn runtime_issue(
    code: &str,
    field_path: &str,
    expected: &str,
    actual: &str,
) -> RuntimeEvidenceIssue {
    RuntimeEvidenceIssue {
        code: code.to_string(),
        field_path: field_path.to_string(),
        expected: expected.to_string(),
        actual: actual.to_string(),
    }
}

#[must_use]
pub fn is_runtime_capture_origin(origin: &str) -> bool {
    matches!(
        origin,
        CAPTURE_ORIGIN_BOUNDARY
            | CAPTURE_ORIGIN_INGESTED
            | CAPTURE_ORIGIN_WRAPPER_ASSERTED
            | CAPTURE_ORIGIN_UNKNOWN
    )
}

#[must_use]
pub fn is_blake3_hex(value: &str) -> bool {
    value.len() == BLAKE3_HEX_LENGTH && value.chars().all(|character| character.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    // r[impl cairn.runtime_evidence.tests]
    use super::*;

    fn policy() -> RuntimeEvidencePolicy {
        RuntimeEvidencePolicy {
            allowed_capture_origins: vec![
                CAPTURE_ORIGIN_BOUNDARY.to_string(),
                CAPTURE_ORIGIN_INGESTED.to_string(),
                CAPTURE_ORIGIN_WRAPPER_ASSERTED.to_string(),
                CAPTURE_ORIGIN_UNKNOWN.to_string(),
            ],
            allowed_hash_algorithms: vec![RUNTIME_HASH_ALGORITHM_BLAKE3.to_string()],
            max_summary_bytes: DEFAULT_RUNTIME_SUMMARY_MAX_BYTES,
            forbidden_summary_fragments: vec![
                "SECRET=".to_string(),
                "token=".to_string(),
                "/home/".to_string(),
            ],
            aggregation_profiles: Vec::new(),
        }
    }

    fn valid_records() -> Vec<serde_json::Value> {
        serde_json::from_str(include_str!(
            "../../../../fixtures/runtime-evidence/positive/chain.json"
        ))
        .expect("valid runtime chain fixture parses")
    }

    fn valid_checkpoint() -> serde_json::Value {
        serde_json::from_str(include_str!(
            "../../../../fixtures/runtime-evidence/positive/checkpoint.json"
        ))
        .expect("valid checkpoint fixture parses")
    }

    fn invalid_records(name: &str) -> Vec<serde_json::Value> {
        let text = match name {
            "mixed" => {
                include_str!("../../../../fixtures/runtime-evidence/negative/mixed-subject.json")
            }
            "secret" => {
                include_str!("../../../../fixtures/runtime-evidence/negative/secret-summary.json")
            }
            "tampered" => {
                include_str!("../../../../fixtures/runtime-evidence/negative/tampered-record.json")
            }
            _ => panic!("unknown fixture"),
        };
        serde_json::from_str(text).expect("invalid runtime chain fixture parses")
    }

    #[test]
    fn valid_chain_with_checkpoint_reports_witnessed_prefix() {
        // r[verify cairn.runtime_evidence.records.valid]
        // r[verify cairn.runtime_evidence.subject_scope.single]
        // r[verify cairn.runtime_evidence.capture_origin.boundary]
        // r[verify cairn.runtime_evidence.capture_origin.ingested]
        // r[verify cairn.runtime_evidence.hash_only.valid]
        // r[verify cairn.runtime_evidence.external_witness.match]
        // r[verify cairn.runtime_evidence.completeness_verdicts]
        let records = valid_records();
        let checkpoint = valid_checkpoint();
        let report = verify_runtime_evidence(&policy(), &records, &[checkpoint]);
        assert!(report.valid);
        assert_eq!(report.completeness_verdict, COMPLETENESS_WITNESSED_PREFIX);
        assert_eq!(report.record_count, 2);
        assert_eq!(report.capture_origin_summary.boundary_captured, 1);
        assert_eq!(report.capture_origin_summary.ingested_telemetry, 1);
    }

    #[test]
    fn valid_chain_without_checkpoint_is_integrity_only() {
        // r[verify cairn.runtime_evidence.completeness_verdicts.integrity_only]
        let records = valid_records();
        let report = verify_runtime_evidence(&policy(), &records, &[]);
        assert!(report.valid);
        assert_eq!(report.completeness_verdict, COMPLETENESS_INTEGRITY_ONLY);
    }

    #[test]
    fn mixed_subject_chain_fails_closed() {
        // r[verify cairn.runtime_evidence.subject_scope.mixed]
        let records = invalid_records("mixed");
        let report = verify_runtime_evidence(&policy(), &records, &[]);
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "runtime_evidence.subject_mixed"));
    }

    #[test]
    fn secret_summary_fails_without_echoing_secret() {
        // r[verify cairn.runtime_evidence.hash_only.secret]
        let records = invalid_records("secret");
        let report = verify_runtime_evidence(&policy(), &records, &[]);
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "runtime_evidence.summary_secret_like"));
        assert!(report
            .issues
            .iter()
            .all(|issue| !issue.actual.contains("SECRET")));
    }

    #[test]
    fn tampered_record_hash_fails() {
        // r[verify cairn.runtime_evidence.records.malformed]
        let records = invalid_records("tampered");
        let report = verify_runtime_evidence(&policy(), &records, &[]);
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "runtime_evidence.record_hash_mismatch"));
    }

    #[test]
    fn wrong_subject_checkpoint_fails() {
        // r[verify cairn.runtime_evidence.external_witness.subject_mismatch]
        let records = valid_records();
        let checkpoint: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../fixtures/runtime-evidence/negative/wrong-subject-checkpoint.json"
        ))
        .expect("wrong-subject checkpoint parses");
        let report = verify_runtime_evidence(&policy(), &records, &[checkpoint]);
        assert!(!report.valid);
        assert_eq!(report.completeness_verdict, COMPLETENESS_SUBJECT_MISMATCH);
    }

    #[test]
    fn truncated_checkpoint_fails() {
        // r[verify cairn.runtime_evidence.completeness_verdicts.truncated]
        let records = valid_records();
        let checkpoint: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../fixtures/runtime-evidence/negative/truncated-checkpoint.json"
        ))
        .expect("truncated checkpoint parses");
        let report = verify_runtime_evidence(&policy(), &records, &[checkpoint]);
        assert!(!report.valid);
        assert_eq!(report.completeness_verdict, COMPLETENESS_TRUNCATED);
    }

    #[test]
    fn oversized_checkpoint_count_fails_without_truncating() {
        let records = valid_records();
        let mut checkpoint = valid_checkpoint();
        checkpoint["record_count"] = serde_json::json!(u64::MAX);
        let checkpoint_hash = witness_checkpoint_hash(&checkpoint);
        checkpoint[CHECKPOINT_HASH_FIELD] = serde_json::json!(checkpoint_hash);
        let report = verify_runtime_evidence(&policy(), &records, &[checkpoint]);
        assert!(!report.valid);
        assert_eq!(report.completeness_verdict, COMPLETENESS_TRUNCATED);
        assert!(report.issues.iter().any(|issue| {
            issue.code == "runtime_evidence.checkpoint_truncated_chain"
                && issue.field_path == "checkpoint.record_count"
        }));
    }

    #[test]
    fn altered_prefix_checkpoint_fails() {
        // r[verify cairn.runtime_evidence.completeness_verdicts.altered]
        let records = valid_records();
        let checkpoint: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../fixtures/runtime-evidence/negative/altered-checkpoint.json"
        ))
        .expect("altered checkpoint parses");
        let report = verify_runtime_evidence(&policy(), &records, &[checkpoint]);
        assert!(!report.valid);
        assert_eq!(
            report.completeness_verdict,
            COMPLETENESS_ALTERED_BEFORE_CHECKPOINT
        );
    }
}
