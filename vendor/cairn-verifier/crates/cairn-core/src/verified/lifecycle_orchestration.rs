use serde_json::Value;

pub const STEEL_ORCHESTRATION_RECEIPT_SCHEMA_V1: &str = "cairn.steel-orchestration-receipt.v1";
pub const STEEL_ACTION_ENVELOPE_SCHEMA_V1: &str = "cairn.steel-action-envelope.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteelActionValidationReport {
    pub valid: bool,
    pub issues: Vec<SteelActionIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteelActionIssue {
    pub code: &'static str,
    pub field_path: String,
    pub message: String,
}

pub fn validate_steel_action_envelope(
    envelope: &Value,
    allowed_host_functions: &[String],
) -> SteelActionValidationReport {
    let mut issues = Vec::new();

    expect_string(
        envelope,
        "schema_version",
        STEEL_ACTION_ENVELOPE_SCHEMA_V1,
        &mut issues,
    );
    require_nonempty_string(envelope, "profile_id", &mut issues);
    require_nonempty_string(envelope, "script_hash", &mut issues);

    match envelope.get("actions").and_then(Value::as_array) {
        Some(actions) if actions.is_empty() => issues.push(issue(
            "empty_actions",
            "actions",
            "Steel action envelope must contain at least one action",
        )),
        Some(actions) => {
            for (index, action) in actions.iter().enumerate() {
                validate_action(action, index, allowed_host_functions, &mut issues);
            }
        }
        None => issues.push(issue(
            "missing_actions",
            "actions",
            "Steel action envelope must include an actions array",
        )),
    }

    SteelActionValidationReport {
        valid: issues.is_empty(),
        issues,
    }
}

fn validate_action(
    action: &Value,
    index: usize,
    allowed_host_functions: &[String],
    issues: &mut Vec<SteelActionIssue>,
) {
    let prefix = format!("actions[{index}]");
    require_nonempty_string(action, "id", issues);
    let Some(host_function) = action.get("host_function").and_then(Value::as_str) else {
        issues.push(issue(
            "missing_host_function",
            format!("{prefix}.host_function"),
            "Steel action must request an explicit host function",
        ));
        return;
    };

    if !allowed_host_functions
        .iter()
        .any(|allowed| allowed == host_function)
    {
        issues.push(issue(
            "host_function_not_allowed",
            format!("{prefix}.host_function"),
            "Steel action requested a host function outside the selected profile",
        ));
    }

    if !matches!(action.get("args"), Some(Value::Object(_))) {
        issues.push(issue(
            "invalid_args",
            format!("{prefix}.args"),
            "Steel action args must be a JSON object for Rust-side validation",
        ));
    }
}

fn expect_string(
    value: &Value,
    field: &'static str,
    expected: &'static str,
    issues: &mut Vec<SteelActionIssue>,
) {
    match value.get(field).and_then(Value::as_str) {
        Some(actual) if actual == expected => {}
        Some(_) => issues.push(issue(
            "invalid_string",
            field,
            format!("field must equal {expected}"),
        )),
        None => issues.push(issue("missing_string", field, "field must be present")),
    }
}

fn require_nonempty_string(value: &Value, field: &'static str, issues: &mut Vec<SteelActionIssue>) {
    match value.get(field).and_then(Value::as_str) {
        Some(text) if !text.is_empty() => {}
        Some(_) => issues.push(issue("empty_string", field, "field must not be empty")),
        None => issues.push(issue("missing_string", field, "field must be present")),
    }
}

fn issue(
    code: &'static str,
    field_path: impl Into<String>,
    message: impl Into<String>,
) -> SteelActionIssue {
    SteelActionIssue {
        code,
        field_path: field_path.into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_typed_envelope_with_allowed_host_function() {
        let envelope = serde_json::json!({
            "schema_version": STEEL_ACTION_ENVELOPE_SCHEMA_V1,
            "profile_id": "comparison.lifecycle.closeout",
            "script_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "actions": [{
                "id": "validate",
                "host_function": "cairn.validate",
                "args": {"root": "."}
            }]
        });
        let allowed = vec!["cairn.validate".to_string()];

        let report = validate_steel_action_envelope(&envelope, &allowed);

        assert!(report.valid);
        assert!(report.issues.is_empty());
    }

    #[test]
    fn rejects_host_function_outside_profile() {
        let envelope = serde_json::json!({
            "schema_version": STEEL_ACTION_ENVELOPE_SCHEMA_V1,
            "profile_id": "comparison.lifecycle.closeout",
            "script_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "actions": [{
                "id": "write",
                "host_function": "host.fs.write",
                "args": {"path": ".cairn/specs/example/spec.md"}
            }]
        });
        let allowed = vec!["cairn.validate".to_string()];

        let report = validate_steel_action_envelope(&envelope, &allowed);

        assert!(!report.valid);
        assert_eq!(report.issues[0].code, "host_function_not_allowed");
    }

    #[test]
    fn rejects_malformed_action_envelope() {
        let envelope = serde_json::json!({
            "schema_version": "unexpected",
            "profile_id": "",
            "actions": []
        });
        let report = validate_steel_action_envelope(&envelope, &[]);
        let codes = report
            .issues
            .iter()
            .map(|issue| issue.code)
            .collect::<Vec<_>>();

        assert!(!report.valid);
        assert!(codes.contains(&"invalid_string"));
        assert!(codes.contains(&"empty_string"));
        assert!(codes.contains(&"missing_string"));
        assert!(codes.contains(&"empty_actions"));
    }
}
