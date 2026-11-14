// r[impl electrical_circuit_primitives.receipts]
// r[impl electrical_circuit_primitives.receipts.portable_hashes]

use serde_json::Value;

use crate::verified::evidence::canonical_json_hash;

use super::super::common::strip_non_hash_fields;

pub fn circuit_primitive_graph_hash(receipt: &Value) -> String {
    canonical_json_hash(&circuit_primitive_hash_material(receipt))
}

pub fn circuit_primitive_hash_material(receipt: &Value) -> Value {
    let mut material = receipt.clone();
    strip_non_hash_fields(&mut material);
    material
}
