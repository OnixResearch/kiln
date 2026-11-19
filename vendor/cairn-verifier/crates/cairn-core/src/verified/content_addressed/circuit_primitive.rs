// r[impl electrical_circuit_primitives.topology]
// r[impl electrical_circuit_primitives.topology.reference_node]
// r[impl electrical_circuit_primitives.topology.connection_arity]
// r[impl electrical_circuit_primitives.components]
// r[impl electrical_circuit_primitives.components.passive_primitives]
// r[impl electrical_circuit_primitives.components.model_caveats]
// r[impl electrical_circuit_primitives.units]
// r[impl electrical_circuit_primitives.units.dimensioned_values]
// r[impl electrical_circuit_primitives.receipts]
// r[impl electrical_circuit_primitives.receipts.portable_hashes]
// r[impl electrical_circuit_primitive_receipts.validator]
// r[impl electrical_circuit_primitive_receipts.validator.core]
// r[impl electrical_circuit_primitive_receipts.fixtures]
// r[impl electrical_circuit_primitive_receipts.fixtures.positive]
// r[impl electrical_circuit_primitive_receipts.fixtures.negative]
// r[impl electrical_circuit_primitive_receipts.circuit_primitive_core_split.focused_modules]
// r[impl electrical_circuit_primitive_receipts.circuit_primitive_core_split.focused_modules.visible_boundaries]
// r[impl electrical_circuit_primitive_receipts.circuit_primitive_core_split.focused_modules.narrow_bridges]
// r[verify electrical_circuit_primitive_receipts.circuit_primitive_core_split.behavior_preserved]

mod components;
mod hash;
mod issues;
mod topology;

#[cfg(test)]
mod tests;

pub use hash::{circuit_primitive_graph_hash, circuit_primitive_hash_material};
pub use issues::CircuitPrimitiveIssue;

pub const CIRCUIT_PRIMITIVE_SCHEMA_V1: &str = "electrical.circuit-primitive-graph.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CircuitPrimitiveReport {
    pub valid: bool,
    pub verdict: String,
    pub issues: Vec<CircuitPrimitiveIssue>,
    pub computed_graph_hash: String,
}

pub fn validate_circuit_primitive_receipt(receipt: &serde_json::Value) -> CircuitPrimitiveReport {
    let computed_graph_hash = circuit_primitive_graph_hash(receipt);
    let issues = issues::circuit_primitive_issues(receipt, &computed_graph_hash);
    let is_valid = issues.is_empty();
    CircuitPrimitiveReport {
        valid: is_valid,
        verdict: if is_valid { "PASS" } else { "FAIL" }.to_string(),
        issues,
        computed_graph_hash,
    }
}
