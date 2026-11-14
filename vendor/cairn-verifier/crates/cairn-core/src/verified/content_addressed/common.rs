use serde_json::{Map, Value};

use crate::verified::nominal_identity::{ArtifactDigest, RAW_BLAKE3_PROFILE};

pub(super) const SOURCE_SCANNER_DEPENDENCIES: &str = "source-scanner-token-set";
pub(super) const NO_CORRECTNESS_CLAIM: &str = "none";
pub(super) const KNOWN_MACRO_STATUSES: &[&str] =
    &["none", "source-visible", "macro-hidden", "generated-code"];
pub(super) const KNOWN_EFFECT_STATUSES: &[&str] = &[
    "unknown",
    "reviewed-pure",
    "reviewed-impure",
    "macro-hidden",
    "unresolved",
];
pub(super) const MUTATION_MANIFEST_REQUIRED_FIELDS: &[&str] = &["touched_files", "manifest_hash"];

const ENTROPY_KEY_FRAGMENTS: &[&str] = &[
    "timestamp",
    "created_at",
    "updated_at",
    "absolute_path",
    "temp_path",
    "tmp_path",
    "environment",
    "env",
    "hostname",
    "username",
];
const HASH_TRAVERSAL_STACK_CAPACITY: usize = 32;

pub(super) fn strip_non_hash_fields(value: &mut Value) {
    let mut stack = Vec::with_capacity(HASH_TRAVERSAL_STACK_CAPACITY);
    stack.push(value);
    while let Some(current) = stack.pop() {
        match current {
            Value::Object(object) => {
                object.remove("object_hash");
                object.remove("pointer_hash");
                object.remove("bundle_hash");
                object.remove("pilot_hash");
                object.remove("graph_hash");
                object.remove("primitive_hash");
                object.remove("display_root");
                object.remove("display_path");
                object.remove("display_only");
                for child in object.values_mut() {
                    stack.push(child);
                }
            }
            Value::Array(entries) => {
                for child in entries {
                    stack.push(child);
                }
            }
            _ => {}
        }
    }
}

pub(super) fn entropy_hash_material_messages(root_path: &str, root: &Value) -> Vec<String> {
    let mut issues = Vec::with_capacity(16);
    let mut stack = Vec::with_capacity(HASH_TRAVERSAL_STACK_CAPACITY);
    stack.push((root_path.to_string(), root));
    while let Some((path, value)) = stack.pop() {
        match value {
            Value::Object(object) => {
                for (key, child) in object {
                    let child_path = format!("{path}.{key}");
                    if is_entropy_key(key) {
                        issues.push(format!(
                            "ambient entropy field is not allowed in hash material: {child_path}"
                        ));
                    }
                    stack.push((child_path, child));
                }
            }
            Value::Array(entries) => {
                for (index, child) in entries.iter().enumerate() {
                    stack.push((format!("{path}[{index}]"), child));
                }
            }
            Value::String(text) if is_temporary_path(text) => {
                issues.push(format!(
                    "temporary absolute path is not allowed in hash material: {path}"
                ));
            }
            _ => {}
        }
    }
    issues
}

pub(super) fn known_value(object: &Map<String, Value>, field: &str, known: &[&str]) -> bool {
    object
        .get(field)
        .and_then(Value::as_str)
        .is_some_and(|value| known.contains(&value))
}

pub(super) fn is_blake3_hex(value: &str) -> bool {
    ArtifactDigest::parse_compatible(value, RAW_BLAKE3_PROFILE, "content_addressed.digest").is_ok()
}

fn is_entropy_key(key: &str) -> bool {
    let lowered = key.to_ascii_lowercase();
    ENTROPY_KEY_FRAGMENTS
        .iter()
        .any(|fragment| lowered.contains(fragment))
}

fn is_temporary_path(text: &str) -> bool {
    text.starts_with("/tmp/") || text.starts_with("/var/tmp/")
}
