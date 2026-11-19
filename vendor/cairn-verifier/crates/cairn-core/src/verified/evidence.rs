use preserves::value::{self, packed::PackedWriter};

// r[impl cairn.supersede.blake3_receipts] r[impl cairn.supersede.blake3_receipts.canonical]
// r[impl cairn.supersede.blake3_receipts.gate] r[impl cairn.supersede.blake3_receipts.lifecycle_plans]
// r[impl cairn.supersede.blake3_receipts.migration] r[impl cairn.supersede.deterministic_gates.policy_freshness]
pub fn blake3_hex(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

pub fn canonical_json(value: &serde_json::Value) -> String {
    let canonical = canonical_json_value(value);
    match serde_json::to_string(&canonical) {
        Ok(text) => text,
        Err(error) => format!("serde_json_value_serialization_error:{error}"),
    }
}

fn canonical_json_value(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(object) => {
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort();
            let mut canonical = serde_json::Map::new();
            for key in keys {
                let Some(child) = object.get(key) else {
                    continue;
                };
                canonical.insert(key.clone(), canonical_json_value(child));
            }
            serde_json::Value::Object(canonical)
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(canonical_json_value).collect())
        }
        scalar => scalar.clone(),
    }
}

pub fn canonical_json_hash(value: &serde_json::Value) -> String {
    blake3_hex(canonical_json(value).as_bytes())
}

// r[impl cairn.determinism.preserves_receipt_canonicalization.additive_internal_seam]
pub fn canonical_preserves_bytes(value: &serde_json::Value) -> Result<Vec<u8>, String> {
    let preserves_value = value::to_value(value);
    PackedWriter::encode_iovalue(&preserves_value).map_err(|error| error.to_string())
}

// r[impl cairn.determinism.preserves_receipt_canonicalization.additive_internal_seam]
// r[impl cairn.determinism.preserves_receipt_canonicalization.stable_hashing]
pub fn canonical_preserves_hash(value: &serde_json::Value) -> Result<String, String> {
    canonical_preserves_bytes(value).map(|bytes| blake3_hex(&bytes))
}

// r[impl cairn.determinism.preserves_authoritative_content_hash.field]
/// Compute the canonical Preserves content hash for a receipt-shaped value,
/// excluding the `receipt_hash` and `preserves_content_hash` fields to avoid
/// self-referential hashing.
///
/// This mirrors the exclusion rule used by `agent_run_receipt_hash` for the
/// JSON `receipt_hash` field, extended to also exclude `preserves_content_hash`
/// itself.
pub fn preserves_content_hash(receipt: &serde_json::Value) -> Result<String, String> {
    let content = receipt_without_content_hashes(receipt);
    canonical_preserves_hash(&content)
}

/// Remove self-referential hash fields from a receipt before content hashing.
fn receipt_without_content_hashes(receipt: &serde_json::Value) -> serde_json::Value {
    let mut canonical = receipt.clone();
    if let Some(object) = canonical.as_object_mut() {
        object.remove("receipt_hash");
        object.remove("preserves_content_hash");
    }
    canonical
}

#[cfg(test)]
mod content_hash_tests {

    use super::*;

    const SORTED_CANONICAL_JSON: &str = "{\"a\":{\"c\":false,\"d\":true},\"z\":null}";

    // r[verify cairn.radicle_change_cob.deterministic_reducer]
    #[test]
    fn canonical_json_sorts_nested_object_keys_explicitly() {
        let mut nested = serde_json::Map::new();
        nested.insert("d".to_string(), serde_json::Value::Bool(true));
        nested.insert("c".to_string(), serde_json::Value::Bool(false));
        let mut value = serde_json::Map::new();
        value.insert("z".to_string(), serde_json::Value::Null);
        value.insert("a".to_string(), serde_json::Value::Object(nested));
        assert_eq!(
            canonical_json(&serde_json::Value::Object(value)),
            SORTED_CANONICAL_JSON
        );
    }

    #[test]
    fn canonical_json_hash_rejects_value_equivalence_after_content_change() {
        let before = serde_json::json!({"z": null, "a": {"d": true, "c": false}});
        let after = serde_json::json!({"z": null, "a": {"d": false, "c": false}});
        assert_ne!(canonical_json_hash(&before), canonical_json_hash(&after));
    }

    #[test]
    // r[verify cairn.determinism.preserves_authoritative_content_hash.tests]
    fn preserves_content_hash_is_deterministic() {
        let receipt = serde_json::json!({
            "schema": "cairn.receipt.v1",
            "valid": true,
            "count": 2
        });
        let hash1 = preserves_content_hash(&receipt).expect("first hash");
        let hash2 = preserves_content_hash(&receipt).expect("second hash");
        assert_eq!(hash1, hash2);
    }

    #[test]
    // r[verify cairn.determinism.preserves_authoritative_content_hash.tests]
    fn preserves_content_hash_excludes_own_field() {
        let receipt = serde_json::json!({
            "receipt_hash": "abc",
            "preserves_content_hash": "def",
            "valid": true
        });
        let hash = preserves_content_hash(&receipt).expect("content hash");
        // Adding a different receipt_hash or preserves_content_hash should NOT
        // change the content hash since they are excluded.
        let receipt2 = serde_json::json!({
            "receipt_hash": "xyz",
            "preserves_content_hash": "uvw",
            "valid": true
        });
        let hash2 = preserves_content_hash(&receipt2).expect("second hash");
        assert_eq!(hash, hash2);
    }

    #[test]
    // r[verify cairn.determinism.preserves_authoritative_content_hash.tests]
    fn preserves_content_hash_changes_when_content_changes() {
        let receipt = serde_json::json!({"valid": true, "count": 1});
        let receipt2 = serde_json::json!({"valid": true, "count": 2});
        assert_ne!(
            preserves_content_hash(&receipt).expect("hash a"),
            preserves_content_hash(&receipt2).expect("hash b")
        );
    }

    #[test]
    // r[verify cairn.determinism.preserves_authoritative_content_hash.field]
    fn preserves_content_hash_is_serialization_independent() {
        let left = serde_json::json!({"b": 2, "a": 1});
        let right = serde_json::json!({"a": 1, "b": 2});
        assert_eq!(
            preserves_content_hash(&left).expect("left"),
            preserves_content_hash(&right).expect("right")
        );
    }

    #[test]
    // r[verify cairn.determinism.preserves_authoritative_content_hash.compatibility]
    fn preserves_content_hash_works_without_hash_fields() {
        let receipt = serde_json::json!({"valid": true});
        let hash = preserves_content_hash(&receipt).expect("hash");
        assert!(hash.len() == 64);
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    struct PreservesCorpusCase {
        id: String,
        description: String,
        value: serde_json::Value,
        expected_hash: String,
    }

    fn preserves_corpus_cases() -> Vec<PreservesCorpusCase> {
        let corpus: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../fixtures/preserves-canonicalization/corpus.json"
        ))
        .expect("Preserves canonicalization corpus parses");
        corpus
            .as_array()
            .expect("Preserves canonicalization corpus is an array")
            .iter()
            .map(|case| PreservesCorpusCase {
                id: case
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .expect("corpus case id")
                    .to_string(),
                description: case
                    .get("description")
                    .and_then(serde_json::Value::as_str)
                    .expect("corpus case description")
                    .to_string(),
                value: case.get("value").expect("corpus case value").clone(),
                expected_hash: case
                    .get("expected_hash")
                    .and_then(serde_json::Value::as_str)
                    .expect("corpus case expected_hash")
                    .to_string(),
            })
            .collect()
    }

    #[test]
    fn hashes_canonical_json_deterministically() {
        let left = serde_json::json!({"b": 2, "a": 1});
        let right = serde_json::json!({"a": 1, "b": 2});
        assert_eq!(canonical_json(&left), canonical_json(&right));
        assert_eq!(canonical_json_hash(&left), canonical_json_hash(&right));
    }

    #[test]
    fn hash_changes_when_content_changes() {
        assert_ne!(
            canonical_json_hash(&serde_json::json!({"actions": ["a"]})),
            canonical_json_hash(&serde_json::json!({"actions": ["b"]}))
        );
    }

    #[test]
    // r[impl cairn.determinism.preserves_receipt_canonicalization.additive_internal_seam.encode]
    // r[verify cairn.determinism.preserves_receipt_canonicalization.stable_hashing.object_order]
    fn preserves_hashes_receipt_objects_deterministically() {
        let left =
            serde_json::json!({"schema": "cairn.receipt.v1", "result": {"ok": true, "count": 2}});
        let right =
            serde_json::json!({"result": {"count": 2, "ok": true}, "schema": "cairn.receipt.v1"});

        assert_eq!(
            canonical_preserves_bytes(&left).expect("left Preserves bytes"),
            canonical_preserves_bytes(&right).expect("right Preserves bytes")
        );
        assert_eq!(
            canonical_preserves_hash(&left).expect("left Preserves hash"),
            canonical_preserves_hash(&right).expect("right Preserves hash")
        );
    }

    #[test]
    // r[verify cairn.determinism.preserves_receipt_canonicalization.stable_hashing.content_change]
    // r[verify cairn.determinism.preserves_receipt_canonicalization.validation]
    // r[verify cairn.determinism.preserves_receipt_canonicalization.validation.focused]
    fn preserves_hash_changes_when_receipt_content_changes() {
        assert_ne!(
            canonical_preserves_hash(&serde_json::json!({"actions": ["a"]})).expect("first hash"),
            canonical_preserves_hash(&serde_json::json!({"actions": ["b"]})).expect("second hash")
        );
    }

    #[test]
    // r[impl cairn.determinism.preserves_canonicalization_corpus.fixture_corpus]
    // r[verify cairn.determinism.preserves_canonicalization_corpus.golden_hashes]
    fn preserves_canonicalization_corpus_matches_checked_in_golden_hashes() {
        let cases = preserves_corpus_cases();
        let ids: Vec<&str> = cases.iter().map(|case| case.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "null_value",
                "boolean_true",
                "integer_negative",
                "unicode_string",
                "mixed_array",
                "nested_object",
                "receipt_shaped"
            ]
        );

        for case in cases {
            assert!(!case.description.is_empty(), "{} has description", case.id);
            assert_eq!(
                canonical_preserves_hash(&case.value).expect("corpus Preserves hash"),
                case.expected_hash,
                "{} hash remains stable",
                case.id
            );
        }
    }

    #[test]
    // r[verify cairn.determinism.preserves_canonicalization_corpus.golden_hashes]
    fn preserves_canonicalization_corpus_nested_object_ignores_key_order() {
        let corpus = preserves_corpus_cases();
        let nested = corpus
            .iter()
            .find(|case| case.id == "nested_object")
            .expect("nested object corpus case");
        let reordered = serde_json::json!({
            "kind": "nested",
            "outer": {
                "a": {"ok": true},
                "b": [2, 1]
            }
        });

        assert_eq!(
            canonical_preserves_bytes(&nested.value).expect("fixture bytes"),
            canonical_preserves_bytes(&reordered).expect("reordered bytes")
        );
        assert_eq!(
            canonical_preserves_hash(&reordered).expect("reordered hash"),
            nested.expected_hash
        );
    }

    #[test]
    // r[verify cairn.determinism.preserves_canonicalization_corpus.validation]
    fn preserves_canonicalization_corpus_hashes_are_content_sensitive() {
        let corpus = preserves_corpus_cases();
        let receipt = corpus
            .iter()
            .find(|case| case.id == "receipt_shaped")
            .expect("receipt-shaped corpus case");
        let mut changed = receipt.value.clone();
        changed["valid"] = serde_json::Value::Bool(false);

        assert_ne!(
            canonical_preserves_hash(&changed).expect("changed hash"),
            receipt.expected_hash
        );
    }
}
