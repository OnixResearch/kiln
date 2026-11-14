use serde_json::{json, Value};

use crate::verified::evidence::canonical_json_hash;

pub const PROBE_RECEIPT_SCHEMA_V1: &str = "cairn.probe-receipt.v1";
pub const PROBE_DECLARATION_SCHEMA_V1: &str = "cairn.probe.v1";
const BLAKE3_HEX_LENGTH: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeIssue {
    pub code: String,
    pub message: String,
    pub field_path: String,
}

pub fn probe_declaration_hash(declaration: &Value) -> String {
    canonical_json_hash(&probe_declaration_hash_material(declaration))
}

pub fn probe_receipt_hash(receipt: &Value) -> String {
    let mut material = receipt.clone();
    if let Some(object) = material.as_object_mut() {
        object.remove("receipt_hash");
    }
    canonical_json_hash(&material)
}

pub fn validate_probe_declaration(declaration: &Value) -> Vec<ProbeIssue> {
    let mut issues = Vec::new();
    require_text(
        declaration,
        "schema",
        Some(PROBE_DECLARATION_SCHEMA_V1),
        &mut issues,
    );
    require_text(declaration, "id", None, &mut issues);
    require_non_empty_string_array(declaration, "requirements", &mut issues);
    require_coverage_class(
        declaration.get("coverage_class"),
        "coverage_class",
        &mut issues,
    );
    validate_command(declaration.get("command"), &mut issues);
    validate_optional_string_array(declaration, "fixtures", &mut issues);
    validate_assertions(declaration.get("assertions"), &mut issues);
    issues
}

pub fn evaluate_probe_assertions(output: &Value, assertions: &[Value]) -> Vec<Value> {
    assertions
        .iter()
        .map(|assertion| evaluate_probe_assertion(output, assertion))
        .collect()
}

pub fn validate_probe_receipt(
    declaration: &Value,
    receipt: &Value,
    expected_declaration_hash: &str,
    expected_policy_hash: Option<&str>,
) -> Vec<ProbeIssue> {
    let mut issues = validate_probe_declaration(declaration);
    require_text(
        receipt,
        "schema",
        Some(PROBE_RECEIPT_SCHEMA_V1),
        &mut issues,
    );
    require_text(
        receipt,
        "probe_id",
        expected_text(declaration, "id"),
        &mut issues,
    );
    require_text(
        receipt,
        "coverage_class",
        expected_text(declaration, "coverage_class"),
        &mut issues,
    );
    require_text(
        receipt,
        "command_identity",
        expected_text_at(declaration, &["command", "identity"]),
        &mut issues,
    );
    require_text(
        receipt,
        "lifecycle_bundle_hash",
        Some(expected_declaration_hash),
        &mut issues,
    );
    if let Some(policy_hash) = expected_policy_hash {
        require_text(receipt, "policy_hash", Some(policy_hash), &mut issues);
    }
    validate_receipt_requirements(declaration, receipt, &mut issues);
    validate_assertion_results(receipt, &mut issues);
    validate_receipt_hash(receipt, &mut issues);
    issues
}

pub fn validate_probe_completion_status(receipt: &Value) -> Vec<ProbeIssue> {
    // r[impl cairn.probe_execution_boundary.authoritative_status]
    let mut issues = Vec::new();
    if receipt.get("status_class").and_then(Value::as_str) != Some("success") {
        issues.push(issue(
            "probe.status.not_passing",
            "saved probe status_class must be success",
            "status_class",
        ));
    }
    if receipt.get("status_matches").and_then(Value::as_bool) != Some(true) {
        issues.push(issue(
            "probe.status.mismatch",
            "saved probe status must match the declared expected status",
            "status_matches",
        ));
    }
    if receipt.get("valid").and_then(Value::as_bool) != Some(true)
        || receipt.get("verdict").and_then(Value::as_str) != Some("PASS")
    {
        issues.push(issue(
            "probe.status.verdict_not_passing",
            "saved probe receipt must carry valid=true and verdict=PASS",
            "verdict",
        ));
    }
    issues
}

pub fn probe_issue_json(issue: &ProbeIssue) -> Value {
    json!({
        "code": issue.code,
        "message": issue.message,
        "field_path": issue.field_path,
    })
}

pub fn all_probe_assertions_passed(results: &[Value]) -> bool {
    results
        .iter()
        .all(|result| result.get("passed").and_then(Value::as_bool) == Some(true))
}

pub fn is_blake3_hex(value: &str) -> bool {
    value.len() == BLAKE3_HEX_LENGTH && value.chars().all(|character| character.is_ascii_hexdigit())
}

fn probe_declaration_hash_material(declaration: &Value) -> Value {
    let mut material = declaration.clone();
    if let Some(object) = material.as_object_mut() {
        object.remove("receipt_path");
    }
    material
}

fn validate_command(command: Option<&Value>, issues: &mut Vec<ProbeIssue>) {
    let Some(command) = command.and_then(Value::as_object) else {
        issues.push(issue(
            "probe.command.missing",
            "command must be an object",
            "command",
        ));
        return;
    };
    if command
        .get("identity")
        .and_then(Value::as_str)
        .is_none_or(str::is_empty)
    {
        issues.push(issue(
            "probe.command.identity_missing",
            "command.identity must be a non-empty string",
            "command.identity",
        ));
    }
    let Some(argv) = command.get("argv").and_then(Value::as_array) else {
        issues.push(issue(
            "probe.command.argv_missing",
            "command.argv must be a non-empty string array",
            "command.argv",
        ));
        return;
    };
    if argv.is_empty()
        || argv
            .iter()
            .any(|argument| argument.as_str().is_none_or(str::is_empty))
    {
        issues.push(issue(
            "probe.command.argv_invalid",
            "command.argv must be a non-empty string array",
            "command.argv",
        ));
    }
}

fn validate_assertions(assertions: Option<&Value>, issues: &mut Vec<ProbeIssue>) {
    let Some(assertions) = assertions.and_then(Value::as_array) else {
        issues.push(issue(
            "probe.assertions.missing",
            "assertions must be a non-empty array",
            "assertions",
        ));
        return;
    };
    if assertions.is_empty() {
        issues.push(issue(
            "probe.assertions.empty",
            "assertions must contain at least one assertion",
            "assertions",
        ));
    }
    for (index, assertion) in assertions.iter().enumerate() {
        validate_assertion(assertion, index, issues);
    }
}

fn validate_assertion(assertion: &Value, index: usize, issues: &mut Vec<ProbeIssue>) {
    let field_prefix = format!("assertions[{index}]");
    if assertion
        .get("id")
        .and_then(Value::as_str)
        .is_none_or(str::is_empty)
    {
        issues.push(issue(
            "probe.assertion.id_missing",
            "assertion.id must be a non-empty string",
            format!("{field_prefix}.id"),
        ));
    }
    let has_equals = assertion.get("equals").is_some();
    let has_contains = assertion.get("contains").is_some();
    let has_file_hash = assertion.get("file_blake3_equals").is_some();
    let is_path_missing = assertion
        .get("path")
        .and_then(Value::as_str)
        .is_none_or(str::is_empty);
    if (has_equals || has_contains) && is_path_missing {
        issues.push(issue(
            "probe.assertion.path_missing",
            "assertion.path must be a non-empty string",
            format!("{field_prefix}.path"),
        ));
    }
    if has_file_hash
        && assertion
            .get("file")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
    {
        issues.push(issue(
            "probe.assertion.file_missing",
            "file hash assertions must set file to a non-empty string",
            format!("{field_prefix}.file"),
        ));
    }
    let operator_count = [has_equals, has_contains, has_file_hash]
        .into_iter()
        .filter(|present| *present)
        .count();
    if operator_count != 1 {
        issues.push(issue(
            "probe.assertion.operator_invalid",
            "assertion must set exactly one supported operator",
            field_prefix,
        ));
    }
}

fn evaluate_probe_assertion(output: &Value, assertion: &Value) -> Value {
    let id = assertion
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("unnamed")
        .to_string();
    let path = assertion
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let actual = value_at_path(output, &path).cloned().unwrap_or(Value::Null);
    let (passed, expected, operator) = if let Some(expected) = assertion.get("equals") {
        (actual == *expected, expected.clone(), "equals")
    } else if let Some(expected) = assertion.get("contains") {
        (
            value_contains(&actual, expected),
            expected.clone(),
            "contains",
        )
    } else {
        (false, Value::Null, "missing")
    };
    json!({
        "id": id,
        "path": path,
        "operator": operator,
        "expected": expected,
        "actual": actual,
        "passed": passed,
    })
}

fn value_at_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = value;
    for segment in path.split('.') {
        if segment.is_empty() {
            return None;
        }
        current = current.get(segment)?;
    }
    Some(current)
}

fn value_contains(actual: &Value, expected: &Value) -> bool {
    match actual {
        Value::Array(items) => items.iter().any(|item| item == expected),
        Value::String(text) => expected
            .as_str()
            .is_some_and(|needle| text.contains(needle)),
        _ => false,
    }
}

fn validate_receipt_requirements(
    declaration: &Value,
    receipt: &Value,
    issues: &mut Vec<ProbeIssue>,
) {
    let expected = string_array(declaration.get("requirements"));
    let actual = string_array(receipt.get("requirements"));
    if expected != actual {
        issues.push(issue(
            "probe.receipt.requirements_mismatch",
            "receipt requirements must match declared probe requirements",
            "requirements",
        ));
    }
}

fn validate_assertion_results(receipt: &Value, issues: &mut Vec<ProbeIssue>) {
    let Some(results) = receipt.get("assertions").and_then(Value::as_array) else {
        issues.push(issue(
            "probe.receipt.assertions_missing",
            "receipt assertions must be an array",
            "assertions",
        ));
        return;
    };
    if !all_probe_assertions_passed(results) {
        issues.push(issue(
            "probe.receipt.assertions_failed",
            "all probe receipt assertions must pass",
            "assertions",
        ));
    }
}

fn validate_receipt_hash(receipt: &Value, issues: &mut Vec<ProbeIssue>) {
    let Some(stated_hash) = receipt.get("receipt_hash").and_then(Value::as_str) else {
        issues.push(issue(
            "probe.receipt.hash_missing",
            "receipt_hash must be present",
            "receipt_hash",
        ));
        return;
    };
    let expected_hash = probe_receipt_hash(receipt);
    if stated_hash != expected_hash || !is_blake3_hex(stated_hash) {
        issues.push(issue(
            "probe.receipt.hash_mismatch",
            "receipt_hash must equal the BLAKE3 hash of canonical receipt content with receipt_hash excluded",
            "receipt_hash",
        ));
    }
}

fn require_text(object: &Value, field: &str, expected: Option<&str>, issues: &mut Vec<ProbeIssue>) {
    let Some(actual) = object.get(field).and_then(Value::as_str) else {
        issues.push(issue(
            format!("probe.{field}.missing"),
            format!("{field} must be a non-empty string"),
            field,
        ));
        return;
    };
    if actual.is_empty() {
        issues.push(issue(
            format!("probe.{field}.empty"),
            format!("{field} must be a non-empty string"),
            field,
        ));
    }
    if let Some(expected) = expected {
        if actual != expected {
            issues.push(issue(
                format!("probe.{field}.mismatch"),
                format!("{field} does not match expected value"),
                field,
            ));
        }
    }
}

fn require_non_empty_string_array(object: &Value, field: &str, issues: &mut Vec<ProbeIssue>) {
    let values = string_array(object.get(field));
    if values.is_empty() {
        issues.push(issue(
            format!("probe.{field}.missing"),
            format!("{field} must be a non-empty string array"),
            field,
        ));
    }
}

fn validate_optional_string_array(object: &Value, field: &str, issues: &mut Vec<ProbeIssue>) {
    let Some(value) = object.get(field) else {
        return;
    };
    let Some(items) = value.as_array() else {
        issues.push(issue(
            format!("probe.{field}.invalid"),
            format!("{field} must be a string array when present"),
            field,
        ));
        return;
    };
    if items
        .iter()
        .any(|item| item.as_str().is_none_or(str::is_empty))
    {
        issues.push(issue(
            format!("probe.{field}.invalid"),
            format!("{field} must contain only non-empty strings"),
            field,
        ));
    }
}

fn require_coverage_class(value: Option<&Value>, field: &str, issues: &mut Vec<ProbeIssue>) {
    let Some(class) = value.and_then(Value::as_str) else {
        issues.push(issue(
            "probe.coverage_class.missing",
            "coverage_class must be a supported string",
            field,
        ));
        return;
    };
    if !matches!(
        class,
        "positive" | "negative" | "malformed-input" | "boundary" | "regression"
    ) {
        issues.push(issue(
            "probe.coverage_class.invalid",
            "coverage_class must be positive, negative, malformed-input, boundary, or regression",
            field,
        ));
    }
}

fn expected_text<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

fn expected_text_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    current.as_str()
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn issue(
    code: impl Into<String>,
    message: impl Into<String>,
    field_path: impl Into<String>,
) -> ProbeIssue {
    ProbeIssue {
        code: code.into(),
        message: message.into(),
        field_path: field_path.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declaration() -> Value {
        json!({
            "schema": PROBE_DECLARATION_SCHEMA_V1,
            "id": "validate-basic",
            "requirements": ["cairn.example.behavior"],
            "coverage_class": "positive",
            "command": {"identity": "cairn validate", "argv": ["validate", "--root", "."]},
            "assertions": [{"id": "valid", "path": "valid", "equals": true}]
        })
    }

    #[test]
    fn valid_declaration_has_no_issues() {
        assert!(validate_probe_declaration(&declaration()).is_empty());
    }

    #[test]
    fn invalid_declaration_reports_missing_assertions() {
        let mut value = declaration();
        value.as_object_mut().unwrap().remove("assertions");
        let issues = validate_probe_declaration(&value);
        assert!(issues
            .iter()
            .any(|issue| issue.code == "probe.assertions.missing"));
    }

    #[test]
    fn invalid_probe_assertions_fail_validation() {
        let mut value = declaration();
        value["assertions"] = json!([{
            "id": "ambiguous",
            "path": "valid",
            "equals": true,
            "contains": true
        }]);
        let issues = validate_probe_declaration(&value);
        assert!(issues
            .iter()
            .any(|issue| issue.code == "probe.assertion.operator_invalid"));
    }

    #[test]
    fn assertions_support_positive_and_negative_results() {
        let assertions = declaration()["assertions"].as_array().unwrap().clone();
        let passed = evaluate_probe_assertions(&json!({"valid": true}), &assertions);
        let failed = evaluate_probe_assertions(&json!({"valid": false}), &assertions);
        assert!(all_probe_assertions_passed(&passed));
        assert!(!all_probe_assertions_passed(&failed));
    }

    #[test]
    fn authoritative_status_accepts_only_an_explicit_current_pass() {
        // r[verify cairn.probe_execution_boundary.authoritative_status]
        let passing = json!({
            "status_class": "success",
            "status_matches": true,
            "valid": true,
            "verdict": "PASS",
        });
        assert!(validate_probe_completion_status(&passing).is_empty());
        for status in [
            "blocked",
            "skipped",
            "deferred",
            "timed-out",
            "dry-run",
            "cancelled",
            "cleanup-failed",
            "infrastructure-failed",
            "unsupported",
        ] {
            let mut non_passing = passing.clone();
            non_passing["status_class"] = Value::String(status.to_string());
            assert!(!validate_probe_completion_status(&non_passing).is_empty());
        }
    }

    #[test]
    fn receipt_validation_rejects_tampering() {
        let declaration = declaration();
        let policy_hash = canonical_json_hash(&json!({"policy": "test"}));
        let declaration_hash = probe_declaration_hash(&declaration);
        let mut receipt = json!({
            "schema": PROBE_RECEIPT_SCHEMA_V1,
            "probe_id": "validate-basic",
            "requirements": ["cairn.example.behavior"],
            "coverage_class": "positive",
            "command_identity": "cairn validate",
            "status_class": "success",
            "assertions": [{"id": "valid", "passed": true}],
            "lifecycle_bundle_hash": declaration_hash,
            "policy_hash": policy_hash
        });
        let hash = probe_receipt_hash(&receipt);
        receipt["receipt_hash"] = Value::String(hash);
        assert!(validate_probe_receipt(
            &declaration,
            &receipt,
            &probe_declaration_hash(&declaration),
            Some(policy_hash.as_str())
        )
        .is_empty());
        receipt["probe_id"] = Value::String("other".to_string());
        let issues = validate_probe_receipt(
            &declaration,
            &receipt,
            &probe_declaration_hash(&declaration),
            Some(policy_hash.as_str()),
        );
        assert!(issues
            .iter()
            .any(|issue| issue.code == "probe.probe_id.mismatch"));
        assert!(issues
            .iter()
            .any(|issue| issue.code == "probe.receipt.hash_mismatch"));
    }

    #[test]
    fn receipt_validation_rejects_stale_policy_probe_id_and_requirement_ids() {
        let declaration = declaration();
        let policy_hash = canonical_json_hash(&json!({"policy": "test"}));
        let stale_policy_hash = canonical_json_hash(&json!({"policy": "stale"}));
        let mut receipt = json!({
            "schema": PROBE_RECEIPT_SCHEMA_V1,
            "probe_id": "undeclared-probe",
            "requirements": ["cairn.example.other"],
            "coverage_class": "positive",
            "command_identity": "cairn validate",
            "status_class": "success",
            "assertions": [{"id": "valid", "passed": true}],
            "lifecycle_bundle_hash": probe_declaration_hash(&declaration),
            "policy_hash": stale_policy_hash
        });
        receipt["receipt_hash"] = Value::String(probe_receipt_hash(&receipt));
        let issues = validate_probe_receipt(
            &declaration,
            &receipt,
            &probe_declaration_hash(&declaration),
            Some(policy_hash.as_str()),
        );
        assert!(issues
            .iter()
            .any(|issue| issue.code == "probe.policy_hash.mismatch"));
        assert!(issues
            .iter()
            .any(|issue| issue.code == "probe.probe_id.mismatch"));
        assert!(issues
            .iter()
            .any(|issue| issue.code == "probe.receipt.requirements_mismatch"));
    }
}
