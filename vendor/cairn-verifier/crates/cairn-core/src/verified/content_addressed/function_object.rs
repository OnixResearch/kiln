use serde_json::{Map, Value};

use crate::verified::evidence::canonical_json_hash;

use super::common::{
    entropy_hash_material_messages, is_blake3_hex, known_value, strip_non_hash_fields,
    KNOWN_EFFECT_STATUSES, KNOWN_MACRO_STATUSES, SOURCE_SCANNER_DEPENDENCIES,
};

pub const FUNCTION_OBJECT_SCHEMA_V1: &str = "octet.function-object.v1";

const REQUIRED_TOP_LEVEL_FIELDS: &[&str] = &[
    "schema",
    "object_hash",
    "source_token",
    "signature_hash",
    "body_hash",
    "normalization",
    "toolchain",
    "caveats",
];
const REQUIRED_NORMALIZATION_FIELDS: &[&str] = &["version", "content_kind"];
const REQUIRED_TOOLCHAIN_FIELDS: &[&str] = &["rustc", "octet_extractor"];
const REQUIRED_CAVEAT_FIELDS: &[&str] = &[
    "identity",
    "dependency_status",
    "macro_status",
    "effect_status",
    "correctness_claim",
];
const SOURCE_TOKEN_IDENTITY: &str = "source-token";
const NO_CORRECTNESS_CLAIM: &str = "none";
pub(super) const SOURCE_TOKEN_CONTENT_KIND: &str = "source-token";
const FUNCTION_OBJECT_ISSUE_CAPACITY: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionObjectIssue {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionObjectReport {
    pub valid: bool,
    pub verdict: String,
    pub issues: Vec<FunctionObjectIssue>,
    pub computed_object_hash: String,
}

pub fn validate_function_object_receipt(receipt: &Value) -> FunctionObjectReport {
    let computed_object_hash = function_object_hash(receipt);
    let issues = function_object_issues(receipt, &computed_object_hash);
    let is_valid = issues.is_empty();
    FunctionObjectReport {
        valid: is_valid,
        verdict: if is_valid { "PASS" } else { "FAIL" }.to_string(),
        issues,
        computed_object_hash,
    }
}

pub fn function_object_hash(receipt: &Value) -> String {
    canonical_json_hash(&function_object_hash_material(receipt))
}

pub fn function_object_hash_material(receipt: &Value) -> Value {
    let mut material = receipt.clone();
    strip_non_hash_fields(&mut material);
    material
}

fn function_object_issues(receipt: &Value, computed_object_hash: &str) -> Vec<FunctionObjectIssue> {
    let Some(object) = receipt.as_object() else {
        return vec![issue(
            "function_object.not_object",
            "function object receipt must be a JSON object",
        )];
    };

    let mut issues = Vec::with_capacity(FUNCTION_OBJECT_ISSUE_CAPACITY);
    issues.extend(required_top_level_issues(object));
    issues.extend(hash_issues(object, computed_object_hash));
    issues.extend(required_nested_object_issues(object));
    issues.extend(normalization_issues(object));
    issues.extend(caveat_issues(object));
    issues.extend(entropy_issues("$", &function_object_hash_material(receipt)));
    issues
}

fn required_top_level_issues(object: &Map<String, Value>) -> Vec<FunctionObjectIssue> {
    let mut issues = Vec::with_capacity(REQUIRED_TOP_LEVEL_FIELDS.len() + 1);
    for field in REQUIRED_TOP_LEVEL_FIELDS {
        if !object.contains_key(*field) {
            issues.push(issue(
                "function_object.missing_field",
                format!("missing required field: {field}"),
            ));
        }
    }
    if object.get("schema").and_then(Value::as_str) != Some(FUNCTION_OBJECT_SCHEMA_V1) {
        issues.push(issue(
            "function_object.unsupported_schema",
            "schema must be octet.function-object.v1",
        ));
    }
    issues
}

fn hash_issues(
    object: &Map<String, Value>,
    computed_object_hash: &str,
) -> Vec<FunctionObjectIssue> {
    let mut issues = Vec::with_capacity(3);
    let stated_hash = object
        .get("object_hash")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if stated_hash != computed_object_hash || !is_blake3_hex(stated_hash) {
        issues.push(issue(
            "function_object.hash_mismatch",
            "object_hash must equal the BLAKE3 hash of canonical function-object content with non-hash fields excluded",
        ));
    }
    for field in ["signature_hash", "body_hash"] {
        if !object
            .get(field)
            .and_then(Value::as_str)
            .is_some_and(is_blake3_hex)
        {
            issues.push(issue(
                "function_object.invalid_hash_field",
                format!("{field} must be a BLAKE3 hex string"),
            ));
        }
    }
    issues
}

fn required_nested_object_issues(object: &Map<String, Value>) -> Vec<FunctionObjectIssue> {
    let mut issues = Vec::with_capacity(
        REQUIRED_NORMALIZATION_FIELDS.len()
            + REQUIRED_TOOLCHAIN_FIELDS.len()
            + REQUIRED_CAVEAT_FIELDS.len(),
    );
    issues.extend(required_object_issues(
        object.get("normalization"),
        "normalization",
        REQUIRED_NORMALIZATION_FIELDS,
    ));
    issues.extend(required_object_issues(
        object.get("toolchain"),
        "toolchain",
        REQUIRED_TOOLCHAIN_FIELDS,
    ));
    issues.extend(required_object_issues(
        object.get("caveats"),
        "caveats",
        REQUIRED_CAVEAT_FIELDS,
    ));
    issues
}

fn normalization_issues(object: &Map<String, Value>) -> Vec<FunctionObjectIssue> {
    let has_source_token_kind = object
        .get("normalization")
        .and_then(|value| value.get("content_kind"))
        .and_then(Value::as_str)
        == Some(SOURCE_TOKEN_CONTENT_KIND);
    if has_source_token_kind {
        return Vec::new();
    }
    vec![issue(
        "function_object.normalization_not_source_token",
        "normalization.content_kind must declare source-token identity",
    )]
}

fn caveat_issues(object: &Map<String, Value>) -> Vec<FunctionObjectIssue> {
    let Some(caveats) = object.get("caveats").and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut issues = Vec::with_capacity(5);
    issues.extend(caveat_identity_issues(caveats));
    issues.extend(caveat_status_issues(caveats));
    issues
}

fn caveat_identity_issues(caveats: &Map<String, Value>) -> Vec<FunctionObjectIssue> {
    let mut issues = Vec::with_capacity(2);
    if caveats.get("identity").and_then(Value::as_str) != Some(SOURCE_TOKEN_IDENTITY) {
        issues.push(issue(
            "function_object.identity_overclaim",
            "caveats.identity must be source-token, not semantic compiler identity",
        ));
    }
    if caveats.get("correctness_claim").and_then(Value::as_str) != Some(NO_CORRECTNESS_CLAIM) {
        issues.push(issue(
            "function_object.correctness_overclaim",
            "function-object hash agreement is provenance/review evidence, not a correctness proof",
        ));
    }
    issues
}

fn caveat_status_issues(caveats: &Map<String, Value>) -> Vec<FunctionObjectIssue> {
    let mut issues = Vec::with_capacity(3);
    if caveats.get("dependency_status").and_then(Value::as_str) != Some(SOURCE_SCANNER_DEPENDENCIES)
    {
        issues.push(issue(
            "function_object.dependency_status_overclaim",
            "source-token function objects must use source-scanner-token-set dependency status",
        ));
    }
    if !known_value(caveats, "macro_status", KNOWN_MACRO_STATUSES) {
        issues.push(issue(
            "function_object.unknown_macro_status",
            "macro_status must use the closed function-object caveat vocabulary",
        ));
    }
    if !known_value(caveats, "effect_status", KNOWN_EFFECT_STATUSES) {
        issues.push(issue(
            "function_object.unknown_effect_status",
            "effect_status must use the closed function-object caveat vocabulary",
        ));
    }
    issues
}

fn required_object_issues(
    value: Option<&Value>,
    object_name: &str,
    required_fields: &[&str],
) -> Vec<FunctionObjectIssue> {
    let Some(object) = value.and_then(Value::as_object) else {
        return vec![issue(
            "function_object.missing_object",
            format!("{object_name} must be a JSON object"),
        )];
    };
    required_fields
        .iter()
        .filter(|field| !object.contains_key(**field))
        .map(|field| {
            issue(
                "function_object.missing_field",
                format!("missing required field: {object_name}.{field}"),
            )
        })
        .collect()
}

fn entropy_issues(root_path: &str, root: &Value) -> Vec<FunctionObjectIssue> {
    entropy_hash_material_messages(root_path, root)
        .into_iter()
        .map(|message| issue("function_object.ambient_entropy_in_hash_material", message))
        .collect()
}

fn issue(code: impl Into<String>, message: impl Into<String>) -> FunctionObjectIssue {
    FunctionObjectIssue {
        code: code.into(),
        message: message.into(),
    }
}
