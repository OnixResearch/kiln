/// Classification of a field-level difference between two receipt snapshots.
#[derive(Debug, Clone, PartialEq)]
pub enum DriftClass {
    /// Field present in candidate but absent in baseline.
    Added,
    /// Field present in baseline but absent in candidate.
    Removed,
    /// Field present in both but leaf values differ.
    Changed,
    /// Field present in both but value types differ (e.g. scalar vs object).
    TypeMismatch,
}

impl DriftClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            DriftClass::Added => "added",
            DriftClass::Removed => "removed",
            DriftClass::Changed => "changed",
            DriftClass::TypeMismatch => "type_mismatch",
        }
    }
}

/// One field-level drift entry.
#[derive(Debug, Clone, PartialEq)]
pub struct DriftEntry {
    /// RFC 6901 JSON Pointer from the receipt root to the differing leaf.
    pub field_path: String,
    /// Classification of the difference.
    pub class: DriftClass,
    /// String representation of the baseline value at this path.
    pub expected: Option<String>,
    /// String representation of the candidate value at this path.
    pub actual: Option<String>,
    /// Source requirement ID from policy-owned path→source-id table, if mapped.
    pub source_id: Option<String>,
}

/// The complete drift report between two receipt-shaped JSON values.
#[derive(Debug, Clone, PartialEq)]
pub struct DriftReport {
    /// Sorted drift entries (one per differing leaf).
    pub entries: Vec<DriftEntry>,
    /// Whether the canonical Preserves bytes differ between the two inputs.
    pub canonical_bytes_differ: bool,
    /// Canonical Preserves hash of the baseline (None if encoding failed).
    pub baseline_preserves_hash: Option<String>,
    /// Canonical Preserves hash of the candidate (None if encoding failed).
    pub candidate_preserves_hash: Option<String>,
    /// BLAKE3 hash of canonical JSON of the baseline input.
    pub baseline_json_hash: String,
    /// BLAKE3 hash of canonical JSON of the candidate input.
    pub candidate_json_hash: String,
}

/// Compute a deterministic structural diff between two receipt-shaped JSON values.
///
/// - Object key reordering is ignored (keys are compared sorted).
/// - Leaf comparison uses exact equality; floats are compared by value.
/// - Differing leaves are addressed by RFC 6901 JSON Pointer strings.
/// - `source_map` maps field paths (RFC 6901 pointers) to requirement IDs.
///   Unmapped paths are reported with `source_id: None`.
pub fn receipt_field_drift(
    baseline: &serde_json::Value,
    candidate: &serde_json::Value,
    source_map: &std::collections::HashMap<String, String>,
) -> DriftReport {
    let baseline_bytes = super::evidence::canonical_preserves_bytes(baseline);
    let candidate_bytes = super::evidence::canonical_preserves_bytes(candidate);
    let baseline_preserves_hash = baseline_bytes.as_ref().map(|bytes| blake3_hex(bytes)).ok();
    let candidate_preserves_hash = candidate_bytes.as_ref().map(|bytes| blake3_hex(bytes)).ok();
    let canonical_bytes_differ = baseline_preserves_hash != candidate_preserves_hash;

    let mut entries = Vec::new();
    compute_drift("", baseline, candidate, &mut entries);
    entries.sort_by(|a, b| a.field_path.cmp(&b.field_path));

    // Assign source_ids from policy-owned map.  Unmapped paths retain None.
    for entry in &mut entries {
        if let Some(id) = source_map.get(&entry.field_path) {
            entry.source_id = Some(id.clone());
        }
    }

    DriftReport {
        baseline_json_hash: super::evidence::canonical_json_hash(baseline),
        candidate_json_hash: super::evidence::canonical_json_hash(candidate),
        baseline_preserves_hash,
        candidate_preserves_hash,
        canonical_bytes_differ,
        entries,
    }
}

fn blake3_hex(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

fn compute_drift(
    prefix: &str,
    baseline: &serde_json::Value,
    candidate: &serde_json::Value,
    entries: &mut Vec<DriftEntry>,
) {
    match (baseline, candidate) {
        (serde_json::Value::Object(b_map), serde_json::Value::Object(c_map)) => {
            // Collect all keys from both objects.
            let mut all_keys: Vec<&str> = b_map
                .keys()
                .chain(c_map.keys())
                .map(String::as_str)
                .collect();
            all_keys.sort();
            all_keys.dedup();

            for key in all_keys {
                let child_path = if prefix.is_empty() {
                    format!("/{}", json_pointer_escape(key))
                } else {
                    format!("{}/{}", prefix, json_pointer_escape(key))
                };
                match (b_map.get(key), c_map.get(key)) {
                    (Some(b), Some(c)) => {
                        compute_drift(&child_path, b, c, entries);
                    }
                    (Some(b), None) => {
                        entries.push(DriftEntry {
                            field_path: child_path,
                            class: DriftClass::Removed,
                            expected: Some(value_summary(b)),
                            actual: None,
                            source_id: None,
                        });
                    }
                    (None, Some(c)) => {
                        entries.push(DriftEntry {
                            field_path: child_path,
                            class: DriftClass::Added,
                            expected: None,
                            actual: Some(value_summary(c)),
                            source_id: None,
                        });
                    }
                    (None, None) => { /* impossible */ }
                }
            }
        }
        (serde_json::Value::Array(b_items), serde_json::Value::Array(c_items)) => {
            // Compare element-by-element up to min length.
            let min_len = b_items.len().min(c_items.len());
            for i in 0..min_len {
                let child_path = format!("{prefix}/{i}");
                compute_drift(&child_path, &b_items[i], &c_items[i], entries);
            }
            // Extra elements in baseline
            for (i, b) in b_items.iter().enumerate().skip(min_len) {
                let child_path = format!("{prefix}/{i}");
                entries.push(DriftEntry {
                    field_path: child_path,
                    class: DriftClass::Removed,
                    expected: Some(value_summary(b)),
                    actual: None,
                    source_id: None,
                });
            }
            // Extra elements in candidate
            for (i, c) in c_items.iter().enumerate().skip(min_len) {
                let child_path = format!("{prefix}/{i}");
                entries.push(DriftEntry {
                    field_path: child_path,
                    class: DriftClass::Added,
                    expected: None,
                    actual: Some(value_summary(c)),
                    source_id: None,
                });
            }
        }
        _ => {
            // Leaf values (scalar, null, or type mismatch at this level).
            if baseline != candidate {
                let class = if json_type_name(baseline) != json_type_name(candidate) {
                    DriftClass::TypeMismatch
                } else {
                    DriftClass::Changed
                };
                entries.push(DriftEntry {
                    field_path: prefix.to_string(),
                    class,
                    expected: Some(value_summary(baseline)),
                    actual: Some(value_summary(candidate)),
                    source_id: None,
                });
            }
        }
    }
}

/// Escape a JSON object key for use in an RFC 6901 JSON Pointer.
fn json_pointer_escape(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

/// Return a short string summary of a JSON value suitable for drift reporting.
fn value_summary(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => {
            if s.len() <= 80 {
                format!("\"{s}\"")
            } else {
                format!("\"{}…\"", &s[..77])
            }
        }
        serde_json::Value::Array(_) => "[array]".to_string(),
        serde_json::Value::Object(_) => "{{object}}".to_string(),
    }
}

/// Return the JSON type name for classification.
fn json_type_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    fn empty_map() -> std::collections::HashMap<String, String> {
        std::collections::HashMap::new()
    }

    fn map_from_pairs(pairs: &[(&str, &str)]) -> std::collections::HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    // r[verify cairn.determinism.preserves_receipt_field_drift.tests]
    // r[impl cairn.determinism.preserves_receipt_field_drift.command]
    fn identical_inputs_yield_empty_drift() {
        let receipt = serde_json::json!({
            "schema": "cairn.receipt.v1",
            "valid": true,
            "result": {"ok": true, "count": 2}
        });
        let report = receipt_field_drift(&receipt, &receipt, &empty_map());
        assert!(report.entries.is_empty());
        assert!(!report.canonical_bytes_differ);
    }

    #[test]
    // r[verify cairn.determinism.preserves_receipt_field_drift.tests]
    // r[verify cairn.determinism.preserves_receipt_field_drift.command]
    fn drifting_inputs_cite_correct_field_paths() {
        let baseline = serde_json::json!({
            "schema": "cairn.receipt.v1",
            "valid": true,
            "count": 2,
            "nested": {"inner": "old"}
        });
        let candidate = serde_json::json!({
            "schema": "cairn.receipt.v1",
            "valid": false,
            "count": 3,
            "nested": {"inner": "new"}
        });
        let report = receipt_field_drift(&baseline, &candidate, &empty_map());
        let paths: Vec<&str> = report
            .entries
            .iter()
            .map(|e| e.field_path.as_str())
            .collect();
        assert_eq!(paths, vec!["/count", "/nested/inner", "/valid"]);
        // Should also report differing bytes since the actual values differ
        assert!(report.canonical_bytes_differ);
    }

    #[test]
    // r[verify cairn.determinism.preserves_receipt_field_drift.tests]
    // r[verify cairn.determinism.preserves_receipt_field_drift.deterministic]
    fn key_reordering_is_not_drift() {
        let left = serde_json::json!({"b": 2, "a": {"ok": true}, "c": [1, 2]});
        let right = serde_json::json!({"a": {"ok": true}, "c": [1, 2], "b": 2});
        let report = receipt_field_drift(&left, &right, &empty_map());
        assert!(report.entries.is_empty());
        // canonical_bytes_differ may be true or false depending on whether
        // Preserves encoding is order-sensitive at each level.  The drift list
        // being empty is the key assertion.
    }

    #[test]
    // r[verify cairn.determinism.preserves_receipt_field_drift.tests]
    fn drift_detects_added_fields() {
        let baseline = serde_json::json!({"a": 1});
        let candidate = serde_json::json!({"a": 1, "b": 2});
        let report = receipt_field_drift(&baseline, &candidate, &empty_map());
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].field_path, "/b");
        assert_eq!(report.entries[0].class, DriftClass::Added);
        assert_eq!(report.entries[0].actual.as_deref(), Some("2"));
    }

    #[test]
    // r[verify cairn.determinism.preserves_receipt_field_drift.tests]
    fn drift_detects_removed_fields() {
        let baseline = serde_json::json!({"a": 1, "b": 2});
        let candidate = serde_json::json!({"a": 1});
        let report = receipt_field_drift(&baseline, &candidate, &empty_map());
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].field_path, "/b");
        assert_eq!(report.entries[0].class, DriftClass::Removed);
        assert_eq!(report.entries[0].expected.as_deref(), Some("2"));
    }

    #[test]
    // r[verify cairn.determinism.preserves_receipt_field_drift.tests]
    fn type_mismatch_is_detected() {
        let baseline = serde_json::json!({"a": 1});
        let candidate = serde_json::json!({"a": "1"});
        let report = receipt_field_drift(&baseline, &candidate, &empty_map());
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].class, DriftClass::TypeMismatch);
    }

    #[test]
    // r[verify cairn.determinism.preserves_receipt_field_drift.tests]
    fn float_change_is_drift() {
        let baseline = serde_json::json!({"value": 1.0});
        let candidate = serde_json::json!({"value": 2.0});
        let report = receipt_field_drift(&baseline, &candidate, &empty_map());
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].class, DriftClass::Changed);
    }

    #[test]
    // r[verify cairn.determinism.preserves_receipt_field_drift.tests]
    fn scalar_to_object_is_type_mismatch() {
        let baseline = serde_json::json!({"a": 1});
        let candidate = serde_json::json!({"a": {"b": 2}});
        let report = receipt_field_drift(&baseline, &candidate, &empty_map());
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].class, DriftClass::TypeMismatch);
    }

    #[test]
    // r[verify cairn.determinism.preserves_receipt_field_drift.tests]
    fn null_is_not_omitted() {
        let baseline = serde_json::json!({"a": null});
        let candidate = serde_json::json!({"a": "present"});
        let report = receipt_field_drift(&baseline, &candidate, &empty_map());
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].class, DriftClass::TypeMismatch);
    }

    #[test]
    // r[verify cairn.determinism.preserves_receipt_field_drift.tests]
    fn array_element_added() {
        let baseline = serde_json::json!({"items": [1, 2]});
        let candidate = serde_json::json!({"items": [1, 2, 3]});
        let report = receipt_field_drift(&baseline, &candidate, &empty_map());
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].field_path, "/items/2");
        assert_eq!(report.entries[0].class, DriftClass::Added);
    }

    #[test]
    // r[verify cairn.determinism.preserves_receipt_field_drift.tests]
    fn array_element_removed() {
        let baseline = serde_json::json!({"items": [1, 2, 3]});
        let candidate = serde_json::json!({"items": [1, 2]});
        let report = receipt_field_drift(&baseline, &candidate, &empty_map());
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].field_path, "/items/2");
        assert_eq!(report.entries[0].class, DriftClass::Removed);
    }

    #[test]
    // r[verify cairn.determinism.preserves_receipt_field_drift.tests]
    fn array_element_changed() {
        let baseline = serde_json::json!({"items": [1, 2, 3]});
        let candidate = serde_json::json!({"items": [1, 5, 3]});
        let report = receipt_field_drift(&baseline, &candidate, &empty_map());
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].field_path, "/items/1");
        assert_eq!(report.entries[0].class, DriftClass::Changed);
    }

    #[test]
    // r[verify cairn.determinism.preserves_receipt_field_drift.source_id_mapping]
    fn source_ids_from_map_are_assigned() {
        let source_map = map_from_pairs(&[
            ("/valid", "r[example.valid]"),
            ("/nested/inner", "r[example.nested]"),
        ]);
        let baseline = serde_json::json!({"valid": true, "nested": {"inner": "old", "other": 1}});
        let candidate = serde_json::json!({"valid": false, "nested": {"inner": "new", "other": 1}});
        let report = receipt_field_drift(&baseline, &candidate, &source_map);
        assert_eq!(report.entries.len(), 2);
        assert_eq!(report.entries[0].field_path, "/nested/inner");
        assert_eq!(
            report.entries[0].source_id.as_deref(),
            Some("r[example.nested]")
        );
        assert_eq!(report.entries[1].field_path, "/valid");
        assert_eq!(
            report.entries[1].source_id.as_deref(),
            Some("r[example.valid]")
        );
    }

    #[test]
    // r[verify cairn.determinism.preserves_receipt_field_drift.source_id_mapping]
    fn unmapped_paths_have_null_source_id() {
        let source_map = map_from_pairs(&[("/valid", "r[example.valid]")]);
        let baseline = serde_json::json!({"valid": true, "count": 1});
        let candidate = serde_json::json!({"valid": false, "count": 2});
        let report = receipt_field_drift(&baseline, &candidate, &source_map);
        // /count is not in the map -> source_id = None
        let count_entry = report
            .entries
            .iter()
            .find(|e| e.field_path == "/count")
            .expect("count drift entry exists");
        assert_eq!(count_entry.source_id, None);
        // /valid is mapped
        let valid_entry = report
            .entries
            .iter()
            .find(|e| e.field_path == "/valid")
            .expect("valid drift entry exists");
        assert_eq!(valid_entry.source_id.as_deref(), Some("r[example.valid]"));
    }

    #[test]
    // r[verify cairn.determinism.preserves_receipt_field_drift.tests]
    fn malformed_inputs_fail_closed_baseline_not_json() {
        // A non-Value::Object root (e.g. a plain array) is valid JSON but not
        // a receipt shape.  The helper should still produce deterministic output.
        let baseline = serde_json::json!([1, 2, 3]);
        let candidate = serde_json::json!({"a": 1});
        let report = receipt_field_drift(&baseline, &candidate, &empty_map());
        // The root types differ, so one entry at "".
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].class, DriftClass::TypeMismatch);
    }

    #[test]
    // r[verify cairn.determinism.preserves_receipt_field_drift.tests]
    fn entries_are_sorted_lexically() {
        let baseline = serde_json::json!({"z": 1, "m": 1, "a": 1});
        let candidate = serde_json::json!({"z": 2, "m": 2, "a": 2});
        let report = receipt_field_drift(&baseline, &candidate, &empty_map());
        let paths: Vec<&str> = report
            .entries
            .iter()
            .map(|e| e.field_path.as_str())
            .collect();
        assert_eq!(paths, vec!["/a", "/m", "/z"]);
    }

    #[test]
    // r[verify cairn.determinism.preserves_receipt_field_drift.deterministic]
    fn hash_fields_are_included() {
        let baseline = serde_json::json!({"valid": true});
        let candidate = serde_json::json!({"valid": false});
        let report = receipt_field_drift(&baseline, &candidate, &empty_map());
        assert!(report.baseline_json_hash.len() == 64);
        assert!(report.candidate_json_hash.len() == 64);
        assert_ne!(report.baseline_json_hash, report.candidate_json_hash);
    }
}
