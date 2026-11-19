pub const STATE_MACHINE_RECEIPT_SCHEMA_V1: &str = "cairn.state-machine-receipt.v1";
pub const STATE_MACHINE_RECEIPT_BOUNDARY: &str = "accepted state-machine receipts are bounded witnesses for named lifecycle transition properties only; they do not prove Cairn implementation correctness, do not prove repository correctness, do not prove source-code correctness, do not prove exhaustive lifecycle safety, do not prove release eligibility, do not prove verifier soundness, and do not prove downstream behavioral correctness";
pub const STATE_MACHINE_VERIFICATION_ROLE_BOUNDED_WITNESS: &str = "bounded-witness";
pub const STATE_MACHINE_RESULT_PASSED: &str = "passed";

const BLAKE3_HEX_LENGTH: usize = 64;
const MAX_VOCABULARY_ENTRIES: usize = 256;
const MAX_PROPERTY_LABELS: usize = 128;
const MAX_DIAGNOSTICS: usize = 128;
const MAX_INPUT_HASHES: usize = 128;
const ALLOWED_CHECKERS: &[&str] = &["apalache", "cairn-bounded-simulation", "tla2", "tlc"];
const REQUIRED_NON_CLAIMS: &[&str] = &[
    "bounded witnesses",
    "do not prove Cairn implementation correctness",
    "do not prove release eligibility",
    "do not prove verifier soundness",
];
const FORBIDDEN_CLAIMS: &[&str] = &[
    "proves implementation correctness",
    "proves exhaustive lifecycle safety",
    "proves release eligibility",
    "proves verifier soundness",
];

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StateMachineArtifact {
    pub path: String,
    pub declared_blake3: String,
    pub observed_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StateMachineChecker {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StateMachineNamedHash {
    pub name: String,
    pub blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StateMachineDiagnostic {
    pub id: String,
    pub severity: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StateMachineReceipt {
    pub schema_version: String,
    pub model: StateMachineArtifact,
    pub transition_profile: String,
    pub states: Vec<String>,
    pub events: Vec<String>,
    pub property_labels: Vec<String>,
    pub checker: StateMachineChecker,
    pub result_status: String,
    pub diagnostics: Vec<StateMachineDiagnostic>,
    pub input_hashes: Vec<StateMachineNamedHash>,
    pub output_hash: String,
    pub verification_role: String,
    pub non_claims: Vec<String>,
    pub receipt_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StateMachineReceiptIssue {
    pub code: String,
    pub field_path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StateMachineReceiptReport {
    pub valid: bool,
    pub receipt_hash: String,
    pub checked_properties: Vec<String>,
    pub issues: Vec<StateMachineReceiptIssue>,
    pub boundary: String,
}

#[must_use]
pub fn state_machine_receipt_hash(value: &serde_json::Value) -> String {
    let mut material = value.clone();
    if let Some(object) = material.as_object_mut() {
        object.remove("receipt_hash");
    }
    super::evidence::canonical_json_hash(&material)
}

#[must_use]
pub fn validate_state_machine_receipt_value(
    value: &serde_json::Value,
) -> StateMachineReceiptReport {
    let computed_hash = state_machine_receipt_hash(value);
    let Ok(receipt) = serde_json::from_value::<StateMachineReceipt>(value.clone()) else {
        return StateMachineReceiptReport {
            valid: false,
            receipt_hash: computed_hash,
            checked_properties: Vec::new(),
            issues: vec![issue(
                "state_machine_receipt.malformed",
                "receipt",
                "receipt must match the state-machine receipt schema",
            )],
            boundary: STATE_MACHINE_RECEIPT_BOUNDARY.to_string(),
        };
    };
    validate_state_machine_receipt(&receipt, computed_hash)
}

#[must_use]
pub fn positive_state_machine_receipt_fixture() -> serde_json::Value {
    let mut value = serde_json::json!({
        "schema_version": STATE_MACHINE_RECEIPT_SCHEMA_V1,
        "model": {
            "path": "models/lifecycle.tla",
            "declared_blake3": fixture_hash('a'),
            "observed_blake3": fixture_hash('a')
        },
        "transition_profile": "cairn.lifecycle.transitions.v1",
        "states": ["draft", "gated", "synced", "archived"],
        "events": ["gate", "sync", "archive"],
        "property_labels": ["dry-run-before-execute", "archive-after-sync"],
        "checker": {"id": "tlc", "version": "2.20"},
        "result_status": STATE_MACHINE_RESULT_PASSED,
        "diagnostics": [{
            "id": "state-space",
            "severity": "info",
            "message": "bounded state space explored without a counterexample"
        }],
        "input_hashes": [{"name": "model", "blake3": fixture_hash('a')}],
        "output_hash": fixture_hash('b'),
        "verification_role": STATE_MACHINE_VERIFICATION_ROLE_BOUNDED_WITNESS,
        "non_claims": [STATE_MACHINE_RECEIPT_BOUNDARY],
        "receipt_hash": ""
    });
    value["receipt_hash"] = serde_json::Value::String(state_machine_receipt_hash(&value));
    value
}

fn validate_state_machine_receipt(
    receipt: &StateMachineReceipt,
    computed_hash: String,
) -> StateMachineReceiptReport {
    // r[impl cairn.lifecycle_orchestration.state_machine_receipts.model]
    // r[impl cairn.lifecycle_orchestration.state_machine_receipts.validation]
    let mut issues = Vec::new();
    if receipt.schema_version != STATE_MACHINE_RECEIPT_SCHEMA_V1 {
        push_issue(
            &mut issues,
            "state_machine_receipt.schema_unsupported",
            "schema_version",
            "schema version is unsupported",
        );
    }
    validate_safe_relative_path(&receipt.model.path, "model.path", &mut issues);
    validate_hash(
        &receipt.model.declared_blake3,
        "model.declared_blake3",
        &mut issues,
    );
    validate_hash(
        &receipt.model.observed_blake3,
        "model.observed_blake3",
        &mut issues,
    );
    if receipt.model.declared_blake3 != receipt.model.observed_blake3 {
        push_issue(
            &mut issues,
            "state_machine_receipt.model_hash_stale",
            "model.observed_blake3",
            "observed model hash must match the declared model hash",
        );
    }
    validate_required_text(
        &receipt.transition_profile,
        "transition_profile",
        &mut issues,
    );
    validate_vocabulary(
        &receipt.states,
        "states",
        MAX_VOCABULARY_ENTRIES,
        &mut issues,
    );
    validate_vocabulary(
        &receipt.events,
        "events",
        MAX_VOCABULARY_ENTRIES,
        &mut issues,
    );
    validate_vocabulary(
        &receipt.property_labels,
        "property_labels",
        MAX_PROPERTY_LABELS,
        &mut issues,
    );
    if !ALLOWED_CHECKERS.contains(&receipt.checker.id.as_str()) {
        push_issue(
            &mut issues,
            "state_machine_receipt.checker_unknown",
            "checker.id",
            "checker identity is not supported",
        );
    }
    validate_required_text(&receipt.checker.version, "checker.version", &mut issues);
    if receipt.result_status != STATE_MACHINE_RESULT_PASSED {
        push_issue(
            &mut issues,
            "state_machine_receipt.result_not_passed",
            "result_status",
            "only an explicit passed result is accepted",
        );
    }
    validate_diagnostics(&receipt.diagnostics, &mut issues);
    validate_input_hashes(&receipt.input_hashes, &mut issues);
    validate_hash(&receipt.output_hash, "output_hash", &mut issues);
    if receipt.verification_role != STATE_MACHINE_VERIFICATION_ROLE_BOUNDED_WITNESS {
        push_issue(
            &mut issues,
            "state_machine_receipt.role_promoted",
            "verification_role",
            "verification role must remain bounded-witness",
        );
    }
    validate_non_claims(&receipt.non_claims, &mut issues);
    if receipt.receipt_hash != computed_hash {
        push_issue(
            &mut issues,
            "state_machine_receipt.hash_mismatch",
            "receipt_hash",
            "receipt hash must match canonical receipt material",
        );
    }
    issues.sort_by(|left, right| {
        left.field_path
            .cmp(&right.field_path)
            .then(left.code.cmp(&right.code))
            .then(left.message.cmp(&right.message))
    });
    issues.dedup();
    let report = StateMachineReceiptReport {
        valid: issues.is_empty(),
        receipt_hash: computed_hash,
        checked_properties: receipt.property_labels.clone(),
        issues,
        boundary: STATE_MACHINE_RECEIPT_BOUNDARY.to_string(),
    };
    debug_assert_eq!(report.valid, report.issues.is_empty());
    report
}

fn validate_vocabulary(
    values: &[String],
    field_path: &str,
    maximum: usize,
    issues: &mut Vec<StateMachineReceiptIssue>,
) {
    if values.is_empty() {
        push_issue(
            issues,
            "state_machine_receipt.vocabulary_empty",
            field_path,
            "vocabulary must contain at least one value",
        );
    }
    if values.len() > maximum {
        push_issue(
            issues,
            "state_machine_receipt.bound_exceeded",
            field_path,
            "vocabulary exceeds its reviewed bound",
        );
    }
    if values.iter().any(|value| value.trim().is_empty()) {
        push_issue(
            issues,
            "state_machine_receipt.vocabulary_value_empty",
            field_path,
            "vocabulary values must be non-empty",
        );
    }
    let unique = values.iter().collect::<std::collections::BTreeSet<_>>();
    if unique.len() != values.len() {
        push_issue(
            issues,
            "state_machine_receipt.vocabulary_duplicate",
            field_path,
            "vocabulary values must be unique",
        );
    }
}

fn validate_diagnostics(
    diagnostics: &[StateMachineDiagnostic],
    issues: &mut Vec<StateMachineReceiptIssue>,
) {
    if diagnostics.is_empty() {
        push_issue(
            issues,
            "state_machine_receipt.diagnostic_missing",
            "diagnostics",
            "at least one bounded diagnostic is required",
        );
    }
    if diagnostics.len() > MAX_DIAGNOSTICS {
        push_issue(
            issues,
            "state_machine_receipt.bound_exceeded",
            "diagnostics",
            "diagnostic count exceeds its reviewed bound",
        );
    }
    for (index, diagnostic) in diagnostics.iter().enumerate() {
        let field_path = format!("diagnostics[{index}]");
        validate_required_text(&diagnostic.id, &format!("{field_path}.id"), issues);
        if !matches!(diagnostic.severity.as_str(), "info" | "warning" | "error") {
            push_issue(
                issues,
                "state_machine_receipt.diagnostic_severity_unsupported",
                &format!("{field_path}.severity"),
                "diagnostic severity must be info, warning, or error",
            );
        }
        validate_required_text(
            &diagnostic.message,
            &format!("{field_path}.message"),
            issues,
        );
    }
}

fn validate_input_hashes(
    hashes: &[StateMachineNamedHash],
    issues: &mut Vec<StateMachineReceiptIssue>,
) {
    if hashes.is_empty() {
        push_issue(
            issues,
            "state_machine_receipt.input_hash_missing",
            "input_hashes",
            "at least one input hash is required",
        );
    }
    if hashes.len() > MAX_INPUT_HASHES {
        push_issue(
            issues,
            "state_machine_receipt.bound_exceeded",
            "input_hashes",
            "input hash count exceeds its reviewed bound",
        );
    }
    let mut names = std::collections::BTreeSet::new();
    for (index, hash) in hashes.iter().enumerate() {
        let field_path = format!("input_hashes[{index}]");
        validate_required_text(&hash.name, &format!("{field_path}.name"), issues);
        validate_hash(&hash.blake3, &format!("{field_path}.blake3"), issues);
        if !names.insert(hash.name.as_str()) {
            push_issue(
                issues,
                "state_machine_receipt.input_hash_duplicate",
                "input_hashes",
                "input hash names must be unique",
            );
        }
    }
}

fn validate_non_claims(non_claims: &[String], issues: &mut Vec<StateMachineReceiptIssue>) {
    let joined = non_claims.join("; ");
    for required in REQUIRED_NON_CLAIMS {
        if !joined.contains(required) {
            push_issue(
                issues,
                "state_machine_receipt.non_claim_missing",
                "non_claims",
                &format!("non-claims must include `{required}`"),
            );
        }
    }
    let normalized = joined.to_ascii_lowercase();
    for forbidden in FORBIDDEN_CLAIMS {
        if normalized.contains(forbidden) {
            push_issue(
                issues,
                "state_machine_receipt.overclaim",
                "non_claims",
                "receipt contains an unsupported correctness claim",
            );
        }
    }
}

fn validate_hash(value: &str, field_path: &str, issues: &mut Vec<StateMachineReceiptIssue>) {
    let valid = value.len() == BLAKE3_HEX_LENGTH
        && value
            .chars()
            .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase());
    if !valid {
        push_issue(
            issues,
            "state_machine_receipt.hash_malformed",
            field_path,
            "hash must be 64 lowercase BLAKE3 hexadecimal characters",
        );
    }
}

fn validate_required_text(
    value: &str,
    field_path: &str,
    issues: &mut Vec<StateMachineReceiptIssue>,
) {
    if value.trim().is_empty() {
        push_issue(
            issues,
            "state_machine_receipt.field_empty",
            field_path,
            "field must be non-empty",
        );
    }
}

fn validate_safe_relative_path(
    path: &str,
    field_path: &str,
    issues: &mut Vec<StateMachineReceiptIssue>,
) {
    let safe = !path.is_empty()
        && !path.starts_with('/')
        && !path.starts_with('~')
        && !path.contains('\\')
        && !path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..");
    if !safe {
        push_issue(
            issues,
            "state_machine_receipt.path_unsafe",
            field_path,
            "path must be a safe repository-relative path",
        );
    }
}

fn fixture_hash(character: char) -> String {
    std::iter::repeat_n(character, BLAKE3_HEX_LENGTH).collect()
}

fn issue(code: &str, field_path: &str, message: &str) -> StateMachineReceiptIssue {
    StateMachineReceiptIssue {
        code: code.to_string(),
        field_path: field_path.to_string(),
        message: message.to_string(),
    }
}

fn push_issue(
    issues: &mut Vec<StateMachineReceiptIssue>,
    code: &str,
    field_path: &str,
    message: &str,
) {
    issues.push(issue(code, field_path, message));
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPECTED_PROPERTY_COUNT: usize = 2;

    fn codes(report: &StateMachineReceiptReport) -> Vec<&str> {
        report
            .issues
            .iter()
            .map(|issue| issue.code.as_str())
            .collect()
    }

    #[test]
    fn passing_lifecycle_fsm_receipt_validates() {
        // r[verify cairn.lifecycle_orchestration.state_machine_receipts.fixtures.positive]
        // r[verify cairn.lifecycle_orchestration.state_machine_receipts.final_validation]
        let report =
            validate_state_machine_receipt_value(&positive_state_machine_receipt_fixture());
        assert!(report.valid, "{:?}", report.issues);
        assert_eq!(report.checked_properties.len(), EXPECTED_PROPERTY_COUNT);
        assert!(report.boundary.contains("bounded witnesses"));
    }

    #[test]
    fn bounded_simulation_receipt_validates() {
        // r[verify cairn.lifecycle_orchestration.state_machine_receipts.fixtures.positive]
        let mut value = positive_state_machine_receipt_fixture();
        value["checker"]["id"] = serde_json::Value::String("cairn-bounded-simulation".to_string());
        value["checker"]["version"] = serde_json::Value::String("v1".to_string());
        value["receipt_hash"] = serde_json::Value::String(state_machine_receipt_hash(&value));
        let report = validate_state_machine_receipt_value(&value);
        assert!(report.valid, "{:?}", report.issues);
    }

    #[test]
    fn invalid_state_machine_receipts_fail_closed() {
        // r[verify cairn.lifecycle_orchestration.state_machine_receipts.fixtures.negative]
        let cases = [
            ("stale-model", "state_machine_receipt.model_hash_stale"),
            ("unknown-checker", "state_machine_receipt.checker_unknown"),
            ("missing-property", "state_machine_receipt.vocabulary_empty"),
            ("unchecked", "state_machine_receipt.result_not_passed"),
            ("promoted-role", "state_machine_receipt.role_promoted"),
            (
                "missing-diagnostic",
                "state_machine_receipt.diagnostic_missing",
            ),
            (
                "missing-non-claim",
                "state_machine_receipt.non_claim_missing",
            ),
        ];
        for (case, expected) in cases {
            let mut value = positive_state_machine_receipt_fixture();
            match case {
                "stale-model" => {
                    value["model"]["observed_blake3"] = serde_json::Value::String(fixture_hash('c'))
                }
                "unknown-checker" => {
                    value["checker"]["id"] = serde_json::Value::String("unknown".to_string())
                }
                "missing-property" => {
                    value["property_labels"] = serde_json::Value::Array(Vec::new())
                }
                "unchecked" => {
                    value["result_status"] = serde_json::Value::String("unchecked".to_string())
                }
                "promoted-role" => {
                    value["verification_role"] = serde_json::Value::String("proof".to_string())
                }
                "missing-diagnostic" => value["diagnostics"] = serde_json::Value::Array(Vec::new()),
                "missing-non-claim" => value["non_claims"] = serde_json::Value::Array(Vec::new()),
                _ => unreachable!(),
            }
            value["receipt_hash"] = serde_json::Value::String(state_machine_receipt_hash(&value));
            let report = validate_state_machine_receipt_value(&value);
            assert!(!report.valid, "case {case}");
            assert!(
                codes(&report).contains(&expected),
                "case {case}: {:?}",
                report.issues
            );
        }
    }

    #[test]
    fn receipt_hash_detects_tampering() {
        let mut value = positive_state_machine_receipt_fixture();
        value["transition_profile"] = serde_json::Value::String("changed".to_string());
        let report = validate_state_machine_receipt_value(&value);
        assert!(!report.valid);
        assert!(codes(&report).contains(&"state_machine_receipt.hash_mismatch"));
    }
}
