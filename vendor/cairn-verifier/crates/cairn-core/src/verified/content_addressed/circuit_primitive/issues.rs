// r[impl electrical_circuit_primitive_receipts.validator.core]
// r[verify electrical_circuit_primitive_receipts.circuit_primitive_core_split.behavior_preserved.repository_checks]

const REQUIRED_TOP_LEVEL_FIELDS: &[&str] = &[
    "schema",
    "graph_hash",
    "normalization",
    "source_artifact_hash",
    "tool",
    "reference_node",
    "nodes",
    "nets",
    "components",
    "non_claims",
];
const REQUIRED_NORMALIZATION_FIELDS: &[&str] = &["version"];
const REQUIRED_TOOL_FIELDS: &[&str] = &["name", "version"];
// r[impl electrical_circuit_primitives.non_claims]
// r[impl electrical_circuit_primitives.non_claims.no_safety_claim]
const REQUIRED_NON_CLAIMS: &[&str] = &[
    "no_simulation_correctness",
    "no_analog_equivalence",
    "no_electrical_safety",
    "no_regulatory_compliance",
    "no_layout_correctness",
    "no_manufacturability",
];
const CIRCUIT_ISSUE_CAPACITY: usize = 24;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CircuitPrimitiveIssue {
    pub code: String,
    pub message: String,
}

pub(super) fn circuit_primitive_issues(
    receipt: &serde_json::Value,
    computed_graph_hash: &str,
) -> Vec<CircuitPrimitiveIssue> {
    let Some(object) = receipt.as_object() else {
        return vec![issue(
            "circuit_primitive.not_object",
            "circuit primitive receipt must be a JSON object",
        )];
    };

    let mut issues = Vec::with_capacity(CIRCUIT_ISSUE_CAPACITY);
    issues.extend(required_top_level_issues(object));
    issues.extend(hash_issues(object, computed_graph_hash));
    issues.extend(required_nested_object_issues(object));
    issues.extend(super::topology::topology_issues(object));
    issues.extend(super::components::component_issues(object));
    issues.extend(non_claim_issues(object));
    issues.extend(entropy_issues(
        "$",
        &super::hash::circuit_primitive_hash_material(receipt),
    ));
    issues
}

fn required_top_level_issues(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Vec<CircuitPrimitiveIssue> {
    let mut issues = Vec::with_capacity(REQUIRED_TOP_LEVEL_FIELDS.len() + 1);
    for field in REQUIRED_TOP_LEVEL_FIELDS {
        if !object.contains_key(*field) {
            issues.push(issue(
                "circuit_primitive.missing_field",
                format!("missing required field: {field}"),
            ));
        }
    }
    if object.get("schema").and_then(serde_json::Value::as_str)
        != Some(super::CIRCUIT_PRIMITIVE_SCHEMA_V1)
    {
        issues.push(issue(
            "circuit_primitive.unsupported_schema",
            "schema must be electrical.circuit-primitive-graph.v1",
        ));
    }
    issues
}

fn hash_issues(
    object: &serde_json::Map<String, serde_json::Value>,
    computed_graph_hash: &str,
) -> Vec<CircuitPrimitiveIssue> {
    let mut issues = Vec::with_capacity(2);
    let stated_hash = object
        .get("graph_hash")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if stated_hash != computed_graph_hash || !super::super::common::is_blake3_hex(stated_hash) {
        issues.push(issue(
            "circuit_primitive.hash_mismatch",
            "graph_hash must equal the BLAKE3 hash of canonical circuit primitive content with non-hash fields excluded",
        ));
    }
    if !object
        .get("source_artifact_hash")
        .and_then(serde_json::Value::as_str)
        .is_some_and(super::super::common::is_blake3_hex)
    {
        issues.push(issue(
            "circuit_primitive.invalid_source_artifact_hash",
            "source_artifact_hash must be a BLAKE3 hex string",
        ));
    }
    issues
}

fn required_nested_object_issues(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Vec<CircuitPrimitiveIssue> {
    let mut issues =
        Vec::with_capacity(REQUIRED_NORMALIZATION_FIELDS.len() + REQUIRED_TOOL_FIELDS.len());
    issues.extend(required_object_issues(
        object.get("normalization"),
        "normalization",
        REQUIRED_NORMALIZATION_FIELDS,
    ));
    issues.extend(required_object_issues(
        object.get("tool"),
        "tool",
        REQUIRED_TOOL_FIELDS,
    ));
    issues
}

fn non_claim_issues(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Vec<CircuitPrimitiveIssue> {
    let Some(non_claims) = object
        .get("non_claims")
        .and_then(serde_json::Value::as_array)
    else {
        return vec![issue(
            "circuit_primitive.non_claims_missing",
            "non_claims must list circuit evidence boundaries",
        )];
    };
    REQUIRED_NON_CLAIMS
        .iter()
        .filter(|claim| {
            !non_claims
                .iter()
                .any(|entry| entry.as_str() == Some(**claim))
        })
        .map(|claim| {
            issue(
                "circuit_primitive.non_claim_missing",
                format!("missing required non-claim: {claim}"),
            )
        })
        .collect()
}

fn required_object_issues(
    value: Option<&serde_json::Value>,
    name: &str,
    required_fields: &[&str],
) -> Vec<CircuitPrimitiveIssue> {
    let Some(object) = value.and_then(serde_json::Value::as_object) else {
        return vec![issue(
            "circuit_primitive.required_object_missing",
            format!("{name} must be an object"),
        )];
    };
    required_fields
        .iter()
        .filter(|field| !object.contains_key(**field))
        .map(|field| {
            issue(
                "circuit_primitive.required_object_field_missing",
                format!("{name}.{field} is required"),
            )
        })
        .collect()
}

fn entropy_issues(root_path: &str, root: &serde_json::Value) -> Vec<CircuitPrimitiveIssue> {
    super::super::common::entropy_hash_material_messages(root_path, root)
        .into_iter()
        .map(|message| {
            issue(
                "circuit_primitive.ambient_entropy_in_hash_material",
                message,
            )
        })
        .collect()
}

pub(super) fn issue(code: impl Into<String>, message: impl Into<String>) -> CircuitPrimitiveIssue {
    CircuitPrimitiveIssue {
        code: code.into(),
        message: message.into(),
    }
}
