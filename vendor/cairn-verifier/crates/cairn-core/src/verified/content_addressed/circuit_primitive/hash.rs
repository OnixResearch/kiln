// r[impl electrical_circuit_primitives.receipts]
// r[impl electrical_circuit_primitives.receipts.portable_hashes]

pub fn circuit_primitive_graph_hash(receipt: &serde_json::Value) -> String {
    crate::verified::evidence::canonical_json_hash(&circuit_primitive_hash_material(receipt))
}

pub fn circuit_primitive_hash_material(receipt: &serde_json::Value) -> serde_json::Value {
    let mut material = receipt.clone();
    super::super::common::strip_non_hash_fields(&mut material);
    material
}
