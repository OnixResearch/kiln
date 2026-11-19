pub const NAME_POINTER_SCHEMA_V1: &str = "octet.name-pointer.v1";

const NAME_POINTER_REQUIRED_FIELDS: &[&str] = &[
    "schema",
    "pointer_hash",
    "name",
    "old_object_hash",
    "new_object_hash",
    "reason",
    "diff_summary",
    "rollback_handle",
    "mutation_manifest",
    "drift_check",
    "caveats",
];
const NAME_POINTER_ISSUE_CAPACITY: usize = 16;
const DRIFT_VALID_STATUS: &str = "valid";
const PURE_CACHE_ELIGIBLE_EFFECTS: &[&str] = &["reviewed-pure"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamePointerIssue {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamePointerReport {
    pub valid: bool,
    pub verdict: String,
    pub issues: Vec<NamePointerIssue>,
    pub computed_pointer_hash: String,
}

// Active Cairn refs: content_addressed_rust_evidence.name_pointer,
// content_addressed_rust_evidence.name_pointer.move,
// content_addressed_rust_evidence.name_pointer.drift,
// content_addressed_rust_evidence.caveats,
// content_addressed_rust_evidence.caveats.dependency_status,
// content_addressed_rust_evidence.caveats.effects_fail_closed,
// content_addressed_rust_evidence.export_bundle.mutation_manifest.
// These become Tracey r[impl ...] refs only after the delta is synced into accepted specs.
pub fn validate_name_pointer_receipt(receipt: &serde_json::Value) -> NamePointerReport {
    let computed_pointer_hash = name_pointer_hash(receipt);
    let issues = name_pointer_issues(receipt, &computed_pointer_hash);
    let is_valid = issues.is_empty();
    NamePointerReport {
        valid: is_valid,
        verdict: if is_valid { "PASS" } else { "FAIL" }.to_string(),
        issues,
        computed_pointer_hash,
    }
}

pub fn name_pointer_hash(receipt: &serde_json::Value) -> String {
    crate::verified::evidence::canonical_json_hash(&name_pointer_hash_material(receipt))
}

pub fn name_pointer_hash_material(receipt: &serde_json::Value) -> serde_json::Value {
    let mut material = receipt.clone();
    super::common::strip_non_hash_fields(&mut material);
    material
}

fn name_pointer_issues(
    receipt: &serde_json::Value,
    computed_pointer_hash: &str,
) -> Vec<NamePointerIssue> {
    let Some(object) = receipt.as_object() else {
        return vec![pointer_issue(
            "name_pointer.not_object",
            "name pointer receipt must be a JSON object",
        )];
    };
    let mut issues = Vec::with_capacity(NAME_POINTER_ISSUE_CAPACITY);
    issues.extend(name_pointer_required_issues(object));
    issues.extend(name_pointer_hash_issues(object, computed_pointer_hash));
    issues.extend(name_pointer_movement_issues(object));
    issues.extend(name_pointer_manifest_issues(object));
    issues.extend(name_pointer_drift_issues(object));
    issues.extend(name_pointer_caveat_issues(object));
    issues.extend(pointer_entropy_issues(
        "$",
        &name_pointer_hash_material(receipt),
    ));
    issues
}

fn name_pointer_required_issues(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Vec<NamePointerIssue> {
    let mut issues = Vec::with_capacity(NAME_POINTER_REQUIRED_FIELDS.len() + 1);
    for field in NAME_POINTER_REQUIRED_FIELDS {
        if !object.contains_key(*field) {
            issues.push(pointer_issue(
                "name_pointer.missing_field",
                format!("missing required field: {field}"),
            ));
        }
    }
    if object.get("schema").and_then(serde_json::Value::as_str) != Some(NAME_POINTER_SCHEMA_V1) {
        issues.push(pointer_issue(
            "name_pointer.unsupported_schema",
            "schema must be octet.name-pointer.v1",
        ));
    }
    issues
}

fn name_pointer_hash_issues(
    object: &serde_json::Map<String, serde_json::Value>,
    computed_pointer_hash: &str,
) -> Vec<NamePointerIssue> {
    let mut issues = Vec::with_capacity(3);
    let stated_hash = object
        .get("pointer_hash")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if stated_hash != computed_pointer_hash || !super::common::is_blake3_hex(stated_hash) {
        issues.push(pointer_issue(
            "name_pointer.hash_mismatch",
            "pointer_hash must equal the BLAKE3 hash of canonical name-pointer content with non-hash fields excluded",
        ));
    }
    for field in ["old_object_hash", "new_object_hash"] {
        if !object
            .get(field)
            .and_then(serde_json::Value::as_str)
            .is_some_and(super::common::is_blake3_hex)
        {
            issues.push(pointer_issue(
                "name_pointer.invalid_hash_field",
                format!("{field} must be a BLAKE3 hex string"),
            ));
        }
    }
    issues
}

fn name_pointer_movement_issues(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Vec<NamePointerIssue> {
    let mut issues = Vec::with_capacity(4);
    for field in ["name", "reason", "diff_summary", "rollback_handle"] {
        if !non_empty_string(object, field) {
            issues.push(pointer_issue(
                "name_pointer.missing_movement_field",
                format!("{field} must be a non-empty string"),
            ));
        }
    }
    if object.get("old_object_hash") == object.get("new_object_hash") {
        issues.push(pointer_issue(
            "name_pointer.noop_move",
            "pointer movement must record a new object hash distinct from the old object hash",
        ));
    }
    issues
}

fn name_pointer_manifest_issues(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Vec<NamePointerIssue> {
    let Some(manifest) = object
        .get("mutation_manifest")
        .and_then(serde_json::Value::as_object)
    else {
        return vec![pointer_issue(
            "name_pointer.missing_object",
            "mutation_manifest must be a JSON object",
        )];
    };
    let mut issues = Vec::with_capacity(6);
    for field in super::common::MUTATION_MANIFEST_REQUIRED_FIELDS {
        if !manifest.contains_key(*field) {
            issues.push(pointer_issue(
                "name_pointer.missing_field",
                format!("missing required field: mutation_manifest.{field}"),
            ));
        }
    }
    issues.extend(touched_file_issues(manifest));
    if !manifest
        .get("manifest_hash")
        .and_then(serde_json::Value::as_str)
        .is_some_and(super::common::is_blake3_hex)
    {
        issues.push(pointer_issue(
            "name_pointer.invalid_manifest_hash",
            "mutation_manifest.manifest_hash must be a BLAKE3 hex string",
        ));
    }
    issues
}

fn touched_file_issues(
    manifest: &serde_json::Map<String, serde_json::Value>,
) -> Vec<NamePointerIssue> {
    let Some(entries) = manifest
        .get("touched_files")
        .and_then(serde_json::Value::as_array)
    else {
        return vec![pointer_issue(
            "name_pointer.invalid_touched_files",
            "mutation_manifest.touched_files must be an array",
        )];
    };
    let mut issues = Vec::with_capacity(entries.len() + 1);
    let mut previous_path = None::<&str>;
    for entry in entries {
        let Some(file) = entry.as_object() else {
            issues.push(pointer_issue(
                "name_pointer.invalid_touched_file",
                "each touched file must be a JSON object",
            ));
            continue;
        };
        let path = file
            .get("path")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if path.is_empty() || path.starts_with('/') || path.contains("..") {
            issues.push(pointer_issue(
                "name_pointer.invalid_touched_file_path",
                "touched file paths must be non-empty safe relative paths",
            ));
        }
        if previous_path.is_some_and(|previous| previous > path) {
            issues.push(pointer_issue(
                "name_pointer.unsorted_mutation_manifest",
                "mutation_manifest.touched_files must be sorted by path",
            ));
        }
        previous_path = Some(path);
        for hash_field in ["before_hash", "after_hash"] {
            if !file
                .get(hash_field)
                .and_then(serde_json::Value::as_str)
                .is_some_and(super::common::is_blake3_hex)
            {
                issues.push(pointer_issue(
                    "name_pointer.invalid_touched_file_hash",
                    format!("touched_files.{hash_field} must be a BLAKE3 hex string"),
                ));
            }
        }
    }
    issues
}

fn name_pointer_drift_issues(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Vec<NamePointerIssue> {
    let Some(drift) = object
        .get("drift_check")
        .and_then(serde_json::Value::as_object)
    else {
        return vec![pointer_issue(
            "name_pointer.missing_object",
            "drift_check must be a JSON object",
        )];
    };
    if drift.get("status").and_then(serde_json::Value::as_str) == Some(DRIFT_VALID_STATUS) {
        return Vec::new();
    }
    vec![pointer_issue(
        "name_pointer.drift_detected",
        "pointer is invalid until an explicit update receipt records the new object hash",
    )]
}

fn name_pointer_caveat_issues(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Vec<NamePointerIssue> {
    let Some(caveats) = object.get("caveats").and_then(serde_json::Value::as_object) else {
        return vec![pointer_issue(
            "name_pointer.missing_object",
            "caveats must be a JSON object",
        )];
    };
    let mut issues = Vec::with_capacity(5);
    if caveats
        .get("dependency_status")
        .and_then(serde_json::Value::as_str)
        != Some(super::common::SOURCE_SCANNER_DEPENDENCIES)
    {
        issues.push(pointer_issue(
            "name_pointer.dependency_status_overclaim",
            "name-pointer receipts must preserve source-scanner-token-set dependency caveats",
        ));
    }
    if !super::common::known_value(caveats, "macro_status", super::common::KNOWN_MACRO_STATUSES) {
        issues.push(pointer_issue(
            "name_pointer.unknown_macro_status",
            "macro_status must use the closed caveat vocabulary",
        ));
    }
    let effect_status = caveats
        .get("effect_status")
        .and_then(serde_json::Value::as_str);
    if !effect_status.is_some_and(|value| super::common::KNOWN_EFFECT_STATUSES.contains(&value)) {
        issues.push(pointer_issue(
            "name_pointer.unknown_effect_status",
            "effect_status must use the closed caveat vocabulary",
        ));
    }
    let is_pure_cache_eligible = caveats
        .get("pure_cache_eligible")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let is_effect_cache_eligible =
        effect_status.is_some_and(|value| PURE_CACHE_ELIGIBLE_EFFECTS.contains(&value));
    if is_pure_cache_eligible && !is_effect_cache_eligible {
        issues.push(pointer_issue(
            "name_pointer.pure_cache_overclaim",
            "unknown, macro-hidden, or unresolved effects must not be pure-cache eligible",
        ));
    }
    issues
}

fn pointer_entropy_issues(root_path: &str, root: &serde_json::Value) -> Vec<NamePointerIssue> {
    super::common::entropy_hash_material_messages(root_path, root)
        .into_iter()
        .map(|message| pointer_issue("name_pointer.ambient_entropy_in_hash_material", message))
        .collect()
}

fn non_empty_string(object: &serde_json::Map<String, serde_json::Value>, field: &str) -> bool {
    object
        .get(field)
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| !value.is_empty())
}

fn pointer_issue(code: impl Into<String>, message: impl Into<String>) -> NamePointerIssue {
    NamePointerIssue {
        code: code.into(),
        message: message.into(),
    }
}
