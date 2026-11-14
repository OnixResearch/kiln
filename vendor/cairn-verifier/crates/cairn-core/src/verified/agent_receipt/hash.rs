use serde_json::{Map, Value};

use crate::verified::evidence::{canonical_json_hash, canonical_preserves_hash};
use crate::verified::nominal_identity::{Blake3Digest, ReceiptDigestRole, RAW_BLAKE3_PROFILE};

use super::common::issue;
use super::types::AgentRunGateIssue;

// r[impl cairn.supersede.agent_run_receipts.schema]
// r[impl cairn.supersede.agent_run_receipts.schema.bounded_content]
pub fn agent_run_receipt_hash(receipt: &Value) -> String {
    let canonical = receipt_without_self_referential_hashes(receipt);
    canonical_json_hash(&canonical)
}

// r[impl cairn.determinism.preserves_receipt_consumer.agent_run_internal_hash]
pub fn agent_run_preserves_receipt_hash(receipt: &Value) -> Result<String, String> {
    let canonical = receipt_without_self_referential_hashes(receipt);
    canonical_preserves_hash(&canonical)
}

fn receipt_without_self_referential_hashes(receipt: &Value) -> Value {
    let mut canonical = receipt.clone();
    if let Some(object) = canonical.as_object_mut() {
        object.remove("receipt_hash");
        object.remove("preserves_receipt_hash");
        object.remove("preserves_content_hash");
    }
    canonical
}

pub(super) fn receipt_hash_issues(
    receipt: &Value,
    object: &Map<String, Value>,
) -> Vec<AgentRunGateIssue> {
    let Some(stated_hash) = object.get("receipt_hash").and_then(Value::as_str) else {
        return Vec::new();
    };
    let expected_hash = agent_run_receipt_hash(receipt);
    let stated = Blake3Digest::<ReceiptDigestRole>::parse_compatible(
        stated_hash,
        RAW_BLAKE3_PROFILE,
        "agent_receipt.receipt_hash",
    );
    let expected = Blake3Digest::<ReceiptDigestRole>::parse(
        &expected_hash,
        RAW_BLAKE3_PROFILE,
        "agent_receipt.expected_receipt_hash",
    );
    let matches = matches!(
        (stated, expected),
        (Ok(stated), Ok(expected)) if stated.as_hex() == expected.as_hex()
    );
    if matches {
        Vec::new()
    } else {
        vec![issue(
            "receipt.hash_mismatch",
            "receipt_hash must equal the BLAKE3 hash of canonical receipt content with receipt_hash excluded",
        )]
    }
}
