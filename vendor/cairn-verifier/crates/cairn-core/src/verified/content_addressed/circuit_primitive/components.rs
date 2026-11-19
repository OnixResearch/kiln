// r[impl electrical_circuit_primitives.components]
// r[impl electrical_circuit_primitives.components.passive_primitives]
// r[impl electrical_circuit_primitives.components.model_caveats]
// r[impl electrical_circuit_primitives.units]
// r[impl electrical_circuit_primitives.units.dimensioned_values]

const REQUIRED_VALUE_FIELDS: &[&str] = &[
    "normalized_value",
    "si_dimension",
    "display_unit",
    "normalization_version",
];
const TWO_TERMINAL_KINDS: &[&str] = &[
    "resistor",
    "capacitor",
    "inductor",
    "voltage-source",
    "current-source",
    "switch",
];
const MODEL_CAVEATED_KINDS: &[&str] = &["behavioral-source", "vendor-model", "semiconductor-model"];
const PASSIVE_KINDS: &[&str] = &["resistor", "capacitor", "inductor"];
const REQUIRED_TERMINALS: &[&str] = &["p", "n"];

pub(super) fn component_issues(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Vec<super::issues::CircuitPrimitiveIssue> {
    let mut issues = Vec::with_capacity(8);
    let Some(components) = object
        .get("components")
        .and_then(serde_json::Value::as_array)
    else {
        issues.push(super::issues::issue(
            "circuit_primitive.components_not_array",
            "components must be an array",
        ));
        return issues;
    };
    for component in components {
        let Some(component_object) = component.as_object() else {
            issues.push(super::issues::issue(
                "circuit_primitive.component_not_object",
                "each component must be a JSON object",
            ));
            continue;
        };
        let kind = component_object
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if PASSIVE_KINDS.contains(&kind) {
            issues.extend(passive_value_issues(component_object));
        }
        if MODEL_CAVEATED_KINDS.contains(&kind) {
            issues.extend(model_caveat_issues(component_object));
        }
    }
    issues
}

pub(super) fn component_binding_issues(
    components: Option<&serde_json::Value>,
    net_ids: &[String],
) -> Vec<super::issues::CircuitPrimitiveIssue> {
    let mut issues = Vec::with_capacity(8);
    let Some(component_entries) = components.and_then(serde_json::Value::as_array) else {
        return issues;
    };
    for component in component_entries {
        let Some(component_object) = component.as_object() else {
            continue;
        };
        let kind = component_object
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if !TWO_TERMINAL_KINDS.contains(&kind) && !MODEL_CAVEATED_KINDS.contains(&kind) {
            issues.push(super::issues::issue(
                "circuit_primitive.unknown_component_kind",
                format!("component kind is outside the closed first-pass vocabulary: {kind}"),
            ));
            continue;
        }
        let Some(bindings) = component_object
            .get("terminal_bindings")
            .and_then(serde_json::Value::as_object)
        else {
            issues.push(super::issues::issue(
                "circuit_primitive.terminal_bindings_missing",
                "component must declare terminal_bindings",
            ));
            continue;
        };
        for terminal in REQUIRED_TERMINALS {
            match bindings.get(*terminal).and_then(serde_json::Value::as_str) {
                Some(net_id) if net_ids.iter().any(|declared| declared == net_id) => {}
                Some(_) => issues.push(super::issues::issue(
                    "circuit_primitive.terminal_net_unknown",
                    format!("terminal {terminal} must bind to a declared net id"),
                )),
                None => issues.push(super::issues::issue(
                    "circuit_primitive.terminal_binding_missing",
                    format!("missing required terminal binding: {terminal}"),
                )),
            }
        }
    }
    issues
}

fn passive_value_issues(
    component: &serde_json::Map<String, serde_json::Value>,
) -> Vec<super::issues::CircuitPrimitiveIssue> {
    let mut issues = Vec::with_capacity(REQUIRED_VALUE_FIELDS.len());
    let Some(value) = component
        .get("value")
        .and_then(serde_json::Value::as_object)
    else {
        return vec![super::issues::issue(
            "circuit_primitive.value_missing",
            "passive primitives must carry a dimensioned value object",
        )];
    };
    for field in REQUIRED_VALUE_FIELDS {
        if !value.contains_key(*field) {
            issues.push(super::issues::issue(
                "circuit_primitive.dimensioned_value_missing_field",
                format!("dimensioned value missing required field: {field}"),
            ));
        }
    }
    if !value
        .get("normalized_value")
        .is_some_and(|entry| entry.is_number())
    {
        issues.push(super::issues::issue(
            "circuit_primitive.normalized_value_not_number",
            "normalized_value must be numeric",
        ));
    }
    issues
}

fn model_caveat_issues(
    component: &serde_json::Map<String, serde_json::Value>,
) -> Vec<super::issues::CircuitPrimitiveIssue> {
    let has_provenance = component
        .get("model_provenance")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| !value.is_empty());
    let has_caveats = component
        .get("model_caveats")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|entries| !entries.is_empty());
    let mut issues = Vec::with_capacity(2);
    if !has_provenance {
        issues.push(super::issues::issue(
            "circuit_primitive.model_provenance_missing",
            "behavioral and vendor-model components must record model provenance",
        ));
    }
    if !has_caveats {
        issues.push(super::issues::issue(
            "circuit_primitive.model_caveats_missing",
            "behavioral and vendor-model components must record explicit caveats",
        ));
    }
    issues
}
