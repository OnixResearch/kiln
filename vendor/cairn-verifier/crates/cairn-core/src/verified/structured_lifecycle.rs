pub const STRUCTURED_LIFECYCLE_SCHEMA_V1: &str = "cairn.structured-lifecycle.bundle.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredLifecycleIssue {
    pub code: String,
    pub message: String,
    pub field_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredLifecycleReport {
    pub valid: bool,
    pub issues: Vec<StructuredLifecycleIssue>,
}

pub fn structured_lifecycle_bundle_hash(bundle: &serde_json::Value) -> String {
    crate::verified::evidence::canonical_json_hash(bundle)
}

pub fn validate_structured_lifecycle_bundle(
    bundle: &serde_json::Value,
) -> StructuredLifecycleReport {
    let mut issues = Vec::new();
    require_schema(bundle, &mut issues);
    validate_specs(bundle.get("specs"), &mut issues);
    let requirement_ids = collect_requirement_ids(bundle.get("specs"));
    validate_changes(bundle.get("changes"), &requirement_ids, &mut issues);
    validate_global_ids(bundle, &mut issues);
    StructuredLifecycleReport {
        valid: issues.is_empty(),
        issues,
    }
}

pub fn structured_lifecycle_issue_json(issue: &StructuredLifecycleIssue) -> serde_json::Value {
    serde_json::json!({
        "code": issue.code,
        "message": issue.message,
        "field_path": issue.field_path,
    })
}

fn require_schema(bundle: &serde_json::Value, issues: &mut Vec<StructuredLifecycleIssue>) {
    match bundle.get("schema").and_then(serde_json::Value::as_str) {
        Some(STRUCTURED_LIFECYCLE_SCHEMA_V1) => {}
        Some(_) => issues.push(issue(
            "structured_lifecycle.schema.unsupported",
            "schema must be cairn.structured-lifecycle.bundle.v1",
            "schema",
        )),
        None => issues.push(issue(
            "structured_lifecycle.schema.missing",
            "schema must be present",
            "schema",
        )),
    }
}

fn validate_specs(value: Option<&serde_json::Value>, issues: &mut Vec<StructuredLifecycleIssue>) {
    let Some(specs) = value.and_then(serde_json::Value::as_array) else {
        issues.push(issue(
            "structured_lifecycle.specs.missing",
            "specs must be an array",
            "specs",
        ));
        return;
    };
    for (index, spec) in specs.iter().enumerate() {
        validate_spec(spec, &format!("specs[{index}]"), issues);
    }
}

fn validate_spec(spec: &serde_json::Value, path: &str, issues: &mut Vec<StructuredLifecycleIssue>) {
    require_stable_id(spec, "id", &format!("{path}.id"), issues);
    require_non_empty_text(spec, "purpose", &format!("{path}.purpose"), issues);
    let req_path = format!("{path}.requirements");
    let Some(requirements) = spec
        .get("requirements")
        .and_then(serde_json::Value::as_array)
    else {
        issues.push(issue(
            "structured_lifecycle.requirements.missing",
            "requirements must be an array",
            req_path,
        ));
        return;
    };
    if requirements.is_empty() {
        issues.push(issue(
            "structured_lifecycle.requirements.empty",
            "requirements must not be empty",
            req_path,
        ));
    }
    for (index, requirement) in requirements.iter().enumerate() {
        validate_requirement(
            requirement,
            &format!("{path}.requirements[{index}]"),
            issues,
        );
    }
}

fn validate_requirement(
    requirement: &serde_json::Value,
    path: &str,
    issues: &mut Vec<StructuredLifecycleIssue>,
) {
    require_stable_id(requirement, "id", &format!("{path}.id"), issues);
    validate_strength(
        requirement.get("strength"),
        &format!("{path}.strength"),
        issues,
    );
    require_non_empty_text(
        requirement,
        "statement",
        &format!("{path}.statement"),
        issues,
    );
    let scenarios_path = format!("{path}.scenarios");
    let Some(scenarios) = requirement
        .get("scenarios")
        .and_then(serde_json::Value::as_array)
    else {
        issues.push(issue(
            "structured_lifecycle.scenarios.missing",
            "scenarios must be a non-empty array",
            scenarios_path,
        ));
        return;
    };
    if scenarios.is_empty() {
        issues.push(issue(
            "structured_lifecycle.scenarios.empty",
            "scenarios must contain at least one scenario",
            scenarios_path,
        ));
    }
    for (index, scenario) in scenarios.iter().enumerate() {
        validate_scenario(scenario, &format!("{path}.scenarios[{index}]"), issues);
    }
}

fn validate_scenario(
    scenario: &serde_json::Value,
    path: &str,
    issues: &mut Vec<StructuredLifecycleIssue>,
) {
    require_stable_id(scenario, "id", &format!("{path}.id"), issues);
    require_non_empty_text_array(scenario, "given", &format!("{path}.given"), issues);
    require_non_empty_text_array(scenario, "when", &format!("{path}.when"), issues);
    require_non_empty_text_array(scenario, "then", &format!("{path}.then"), issues);
}

fn validate_changes(
    value: Option<&serde_json::Value>,
    requirement_ids: &std::collections::BTreeSet<String>,
    issues: &mut Vec<StructuredLifecycleIssue>,
) {
    let Some(changes) = value else {
        return;
    };
    let Some(changes) = changes.as_array() else {
        issues.push(issue(
            "structured_lifecycle.changes.invalid",
            "changes must be an array when present",
            "changes",
        ));
        return;
    };
    for (index, change) in changes.iter().enumerate() {
        validate_change(
            change,
            &format!("changes[{index}]"),
            requirement_ids,
            issues,
        );
    }
}

fn validate_change(
    change: &serde_json::Value,
    path: &str,
    requirement_ids: &std::collections::BTreeSet<String>,
    issues: &mut Vec<StructuredLifecycleIssue>,
) {
    require_stable_id(change, "id", &format!("{path}.id"), issues);
    validate_tasks(
        change.get("tasks"),
        &format!("{path}.tasks"),
        requirement_ids,
        issues,
    );
    validate_probes(
        change.get("probes"),
        &format!("{path}.probes"),
        requirement_ids,
        issues,
    );
}

fn validate_tasks(
    value: Option<&serde_json::Value>,
    path: &str,
    requirement_ids: &std::collections::BTreeSet<String>,
    issues: &mut Vec<StructuredLifecycleIssue>,
) {
    let Some(tasks) = value else {
        return;
    };
    let Some(tasks) = tasks.as_array() else {
        issues.push(issue(
            "structured_lifecycle.tasks.invalid",
            "tasks must be an array when present",
            path,
        ));
        return;
    };
    for (index, task) in tasks.iter().enumerate() {
        let task_path = format!("{path}[{index}]");
        require_stable_id(task, "id", &format!("{task_path}.id"), issues);
        require_non_empty_text_array(task, "covers", &format!("{task_path}.covers"), issues);
        validate_requirement_refs(
            task,
            "covers",
            &format!("{task_path}.covers"),
            requirement_ids,
            issues,
        );
    }
}

fn validate_probes(
    value: Option<&serde_json::Value>,
    path: &str,
    requirement_ids: &std::collections::BTreeSet<String>,
    issues: &mut Vec<StructuredLifecycleIssue>,
) {
    let Some(probes) = value else {
        return;
    };
    let Some(probes) = probes.as_array() else {
        issues.push(issue(
            "structured_lifecycle.probes.invalid",
            "probes must be an array when present",
            path,
        ));
        return;
    };
    for (index, probe) in probes.iter().enumerate() {
        validate_probe(probe, &format!("{path}[{index}]"), requirement_ids, issues);
    }
}

fn validate_probe(
    probe: &serde_json::Value,
    path: &str,
    requirement_ids: &std::collections::BTreeSet<String>,
    issues: &mut Vec<StructuredLifecycleIssue>,
) {
    require_stable_id(probe, "id", &format!("{path}.id"), issues);
    require_non_empty_text_array(
        probe,
        "requirements",
        &format!("{path}.requirements"),
        issues,
    );
    validate_requirement_refs(
        probe,
        "requirements",
        &format!("{path}.requirements"),
        requirement_ids,
        issues,
    );
    validate_coverage_class(
        probe.get("coverage_class"),
        &format!("{path}.coverage_class"),
        issues,
    );
    require_non_empty_text(
        probe,
        "evidence_role",
        &format!("{path}.evidence_role"),
        issues,
    );
}

fn collect_requirement_ids(
    value: Option<&serde_json::Value>,
) -> std::collections::BTreeSet<String> {
    let mut ids = std::collections::BTreeSet::new();
    let Some(specs) = value.and_then(serde_json::Value::as_array) else {
        return ids;
    };
    for spec in specs {
        let Some(requirements) = spec
            .get("requirements")
            .and_then(serde_json::Value::as_array)
        else {
            continue;
        };
        for requirement in requirements {
            if let Some(id) = requirement.get("id").and_then(serde_json::Value::as_str) {
                ids.insert(id.to_string());
            }
        }
    }
    ids
}

fn validate_requirement_refs(
    value: &serde_json::Value,
    field: &str,
    path: &str,
    requirement_ids: &std::collections::BTreeSet<String>,
    issues: &mut Vec<StructuredLifecycleIssue>,
) {
    let Some(refs) = value.get(field).and_then(serde_json::Value::as_array) else {
        return;
    };
    for (index, requirement_ref) in refs.iter().enumerate() {
        let Some(requirement_id) = requirement_ref.as_str() else {
            continue;
        };
        if !requirement_ids.contains(requirement_id) {
            issues.push(issue(
                "structured_lifecycle.requirement_ref.unknown",
                format!("unknown requirement reference: {requirement_id}"),
                format!("{path}[{index}]"),
            ));
        }
    }
}

fn validate_global_ids(bundle: &serde_json::Value, issues: &mut Vec<StructuredLifecycleIssue>) {
    let mut seen = std::collections::BTreeSet::new();
    let mut duplicated = std::collections::BTreeSet::new();
    collect_ids(bundle, &mut |id| {
        if !seen.insert(id.clone()) {
            duplicated.insert(id);
        }
    });
    for duplicate in duplicated {
        issues.push(issue(
            "structured_lifecycle.id.duplicate",
            format!("duplicate structured lifecycle id: {duplicate}"),
            "id",
        ));
    }
}

fn collect_ids(value: &serde_json::Value, push: &mut impl FnMut(String)) {
    match value {
        serde_json::Value::Object(object) => {
            if let Some(id) = object.get("id").and_then(serde_json::Value::as_str) {
                push(id.to_string());
            }
            for child in object.values() {
                collect_ids(child, push);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_ids(item, push);
            }
        }
        _ => {}
    }
}

fn validate_strength(
    value: Option<&serde_json::Value>,
    path: &str,
    issues: &mut Vec<StructuredLifecycleIssue>,
) {
    let Some(strength) = value.and_then(serde_json::Value::as_str) else {
        issues.push(issue(
            "structured_lifecycle.strength.missing",
            "strength must be MUST, SHALL, SHOULD, or MAY",
            path,
        ));
        return;
    };
    if !matches!(strength, "MUST" | "SHALL" | "SHOULD" | "MAY") {
        issues.push(issue(
            "structured_lifecycle.strength.invalid",
            "strength must be MUST, SHALL, SHOULD, or MAY",
            path,
        ));
    }
}

fn validate_coverage_class(
    value: Option<&serde_json::Value>,
    path: &str,
    issues: &mut Vec<StructuredLifecycleIssue>,
) {
    let Some(class) = value.and_then(serde_json::Value::as_str) else {
        issues.push(issue(
            "structured_lifecycle.coverage_class.missing",
            "coverage_class must be positive, negative, malformed-input, boundary, or regression",
            path,
        ));
        return;
    };
    if !matches!(
        class,
        "positive" | "negative" | "malformed-input" | "boundary" | "regression"
    ) {
        issues.push(issue(
            "structured_lifecycle.coverage_class.invalid",
            "coverage_class must be positive, negative, malformed-input, boundary, or regression",
            path,
        ));
    }
}

fn require_non_empty_text(
    value: &serde_json::Value,
    field: &str,
    path: &str,
    issues: &mut Vec<StructuredLifecycleIssue>,
) {
    if value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .is_none_or(str::is_empty)
    {
        issues.push(issue(
            format!("structured_lifecycle.{field}.missing"),
            format!("{field} must be a non-empty string"),
            path,
        ));
    }
}

fn require_stable_id(
    value: &serde_json::Value,
    field: &str,
    path: &str,
    issues: &mut Vec<StructuredLifecycleIssue>,
) {
    let Some(id) = value.get(field).and_then(serde_json::Value::as_str) else {
        issues.push(issue(
            format!("structured_lifecycle.{field}.missing"),
            format!("{field} must be a non-empty string"),
            path,
        ));
        return;
    };
    if id.is_empty() {
        issues.push(issue(
            format!("structured_lifecycle.{field}.missing"),
            format!("{field} must be a non-empty string"),
            path,
        ));
        return;
    }
    if !is_stable_id(id) {
        issues.push(issue(
            "structured_lifecycle.id.malformed",
            format!(
                "id must contain only ASCII letters, digits, underscores, dots, or hyphens: {id}"
            ),
            path,
        ));
    }
}

fn is_stable_id(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
}

fn require_non_empty_text_array(
    value: &serde_json::Value,
    field: &str,
    path: &str,
    issues: &mut Vec<StructuredLifecycleIssue>,
) {
    let Some(items) = value.get(field).and_then(serde_json::Value::as_array) else {
        issues.push(issue(
            format!("structured_lifecycle.{field}.missing"),
            format!("{field} must be a non-empty string array"),
            path,
        ));
        return;
    };
    if items.is_empty()
        || items
            .iter()
            .any(|item| item.as_str().is_none_or(str::is_empty))
    {
        issues.push(issue(
            format!("structured_lifecycle.{field}.invalid"),
            format!("{field} must be a non-empty string array"),
            path,
        ));
    }
}

fn issue(
    code: impl Into<String>,
    message: impl Into<String>,
    field_path: impl Into<String>,
) -> StructuredLifecycleIssue {
    StructuredLifecycleIssue {
        code: code.into(),
        message: message.into(),
        field_path: field_path.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle() -> serde_json::Value {
        serde_json::json!({
            "schema": STRUCTURED_LIFECYCLE_SCHEMA_V1,
            "specs": [{
                "id": "structured-lifecycle",
                "purpose": "typed lifecycle source",
                "requirements": [{
                    "id": "cairn.structured.example",
                    "strength": "MUST",
                    "statement": "structured lifecycle source must validate",
                    "scenarios": [{
                        "id": "valid",
                        "given": ["a structured lifecycle bundle"],
                        "when": ["Cairn validates it"],
                        "then": ["the bundle passes"]
                    }]
                }]
            }],
            "changes": [{
                "id": "demo-change",
                "tasks": [{"id": "implement", "covers": ["cairn.structured.example"]}],
                "probes": [{
                    "id": "demo-probe",
                    "requirements": ["cairn.structured.example"],
                    "coverage_class": "positive",
                    "evidence_role": "probe"
                }]
            }]
        })
    }

    #[test]
    fn valid_bundle_passes() {
        let report = validate_structured_lifecycle_bundle(&bundle());
        assert!(report.valid, "issues: {:?}", report.issues);
        assert!(!structured_lifecycle_bundle_hash(&bundle()).is_empty());
    }

    #[test]
    fn missing_scenario_then_fails() {
        let mut bundle = bundle();
        bundle["specs"][0]["requirements"][0]["scenarios"][0]
            .as_object_mut()
            .unwrap()
            .remove("then");
        let report = validate_structured_lifecycle_bundle(&bundle);
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "structured_lifecycle.then.missing"));
    }

    #[test]
    fn duplicate_ids_fail() {
        let mut bundle = bundle();
        bundle["changes"][0]["id"] = serde_json::Value::String("structured-lifecycle".to_string());
        let report = validate_structured_lifecycle_bundle(&bundle);
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "structured_lifecycle.id.duplicate"));
    }

    #[test]
    fn malformed_ids_fail() {
        let mut bundle = bundle();
        bundle["specs"][0]["requirements"][0]["id"] =
            serde_json::Value::String("bad id".to_string());
        let report = validate_structured_lifecycle_bundle(&bundle);
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "structured_lifecycle.id.malformed"));
    }

    #[test]
    fn invalid_task_links_fail() {
        let mut bundle = bundle();
        bundle["changes"][0]["tasks"][0]["covers"] = serde_json::json!(["missing.requirement"]);
        let report = validate_structured_lifecycle_bundle(&bundle);
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "structured_lifecycle.requirement_ref.unknown"));
    }

    #[test]
    fn invalid_probe_requirement_links_fail() {
        let mut bundle = bundle();
        bundle["changes"][0]["probes"][0]["requirements"] =
            serde_json::json!(["missing.requirement"]);
        let report = validate_structured_lifecycle_bundle(&bundle);
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "structured_lifecycle.requirement_ref.unknown"));
    }
}
