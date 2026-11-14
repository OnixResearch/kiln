// r[verify electrical_circuit_primitive_receipts.circuit_primitive_core_split.behavior_preserved.focused_checks]
use serde_json::{json, Value};

use super::*;

fn hash_placeholder() -> String {
    "0".repeat(64)
}

fn base_receipt(display_root: &str) -> Value {
    json!({
        "schema": CIRCUIT_PRIMITIVE_SCHEMA_V1,
        "graph_hash": hash_placeholder(),
        "normalization": {
            "version": "electrical-circuit-primitive-v1",
            "display_root": display_root
        },
        "source_artifact_hash": "1111111111111111111111111111111111111111111111111111111111111111",
        "tool": {
            "name": "cairn-fixture",
            "version": "0.1.0"
        },
        "reference_node": "gnd",
        "nodes": [
            {"id": "gnd"},
            {"id": "vin"},
            {"id": "vout"}
        ],
        "nets": [
            {"id": "net_gnd", "nodes": ["gnd"]},
            {"id": "net_vin", "nodes": ["vin"]},
            {"id": "net_vout", "nodes": ["vout"]}
        ],
        "components": [
            {
                "id": "r1",
                "kind": "resistor",
                "terminal_bindings": {"p": "net_vin", "n": "net_vout"},
                "value": {
                    "normalized_value": 1000.0,
                    "si_dimension": "ohm",
                    "display_unit": "kOhm",
                    "normalization_version": "si-v1"
                },
                "tolerance": "1%"
            },
            {
                "id": "c1",
                "kind": "capacitor",
                "terminal_bindings": {"p": "net_vout", "n": "net_gnd"},
                "value": {
                    "normalized_value": 0.000001,
                    "si_dimension": "farad",
                    "display_unit": "uF",
                    "normalization_version": "si-v1"
                }
            }
        ],
        "non_claims": [
            "no_simulation_correctness",
            "no_analog_equivalence",
            "no_electrical_safety",
            "no_regulatory_compliance",
            "no_layout_correctness",
            "no_manufacturability"
        ]
    })
}

fn with_current_hash(mut receipt: Value) -> Value {
    let hash = circuit_primitive_graph_hash(&receipt);
    receipt["graph_hash"] = Value::String(hash);
    receipt
}

#[test]
fn accepts_dimensioned_two_terminal_graph_with_non_claims() {
    let receipt = with_current_hash(base_receipt("/workspace/one"));
    let report = validate_circuit_primitive_receipt(&receipt);
    assert_eq!(report.verdict, "PASS");
    assert!(report.valid, "{:?}", report.issues);
    assert_eq!(receipt["graph_hash"], report.computed_graph_hash);
}

#[test]
fn graph_hash_excludes_display_root_for_portability() {
    let left = base_receipt("/workspace/one");
    let right = base_receipt("/workspace/two");
    assert_eq!(
        circuit_primitive_graph_hash(&left),
        circuit_primitive_graph_hash(&right)
    );
}

#[test]
fn rejects_missing_terminal_binding() {
    let mut receipt = base_receipt("/workspace/one");
    receipt["components"][0]["terminal_bindings"]
        .as_object_mut()
        .unwrap()
        .remove("n");
    let receipt = with_current_hash(receipt);
    let report = validate_circuit_primitive_receipt(&receipt);
    assert!(!report.valid);
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.code == "circuit_primitive.terminal_binding_missing"));
}

#[test]
fn rejects_safety_non_claim_overclaim() {
    let mut receipt = base_receipt("/workspace/one");
    receipt["non_claims"] = json!(["no_simulation_correctness"]);
    let receipt = with_current_hash(receipt);
    let report = validate_circuit_primitive_receipt(&receipt);
    assert!(!report.valid);
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.code == "circuit_primitive.non_claim_missing"));
}
