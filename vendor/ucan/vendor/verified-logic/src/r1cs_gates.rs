//! Finite gate-to-R1CS adapter primitives.
//!
//! This module maps a tiny bounded gate vocabulary into the existing single-row
//! R1CS semantics. It deliberately stops at deterministic gate admission and row
//! encoding; it does not model circuit builders, allocation namespaces, proof
//! systems, transcripts, commitments, or cryptographic soundness.

use vstd::prelude::*;

use crate::r1cs_constraint::R1csRow;
use crate::r1cs_constraint::R1csWitness;
use crate::r1cs_constraint::r1cs_element_valid;
#[cfg(verus_keep_ghost)]
use crate::r1cs_constraint::r1cs_element_valid_spec;
use crate::r1cs_constraint::r1cs_len_valid;
#[cfg(verus_keep_ghost)]
use crate::r1cs_constraint::r1cs_len_valid_spec;
use crate::r1cs_constraint::r1cs_modulus_valid;
#[cfg(verus_keep_ghost)]
use crate::r1cs_constraint::r1cs_modulus_valid_spec;
use crate::r1cs_constraint::r1cs_row_satisfied;
#[cfg(verus_keep_ghost)]
use crate::r1cs_constraint::r1cs_row_satisfied_spec;
#[cfg(verus_keep_ghost)]
use crate::r1cs_constraint::r1cs_row_valid_spec;
use crate::r1cs_constraint::r1cs_witness_valid;
#[cfg(verus_keep_ghost)]
use crate::r1cs_constraint::r1cs_witness_valid_spec;

verus! {
pub const R1CS_GATE_MUL: u8 = 0;
pub const R1CS_GATE_ADD: u8 = 1;
pub const R1CS_GATE_CONST: u8 = 2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct R1csGate {
    pub kind: u8,
    pub witness_len: u8,
    pub modulus: u16,
    pub x: u8,
    pub y: u8,
    pub z: u8,
    pub value: u16,
}

// r[depends rust-primitives.r1cs-gate-shape]
pub open spec fn r1cs_gate_kind_valid_spec(kind: u8) -> bool {
    kind == R1CS_GATE_MUL || kind == R1CS_GATE_ADD || kind == R1CS_GATE_CONST
}

// r[depends rust-primitives.r1cs-gate-shape]
pub open spec fn r1cs_gate_requires_one_wire_spec(gate: R1csGate) -> bool {
    gate.kind == R1CS_GATE_ADD || gate.kind == R1CS_GATE_CONST
}

// r[depends rust-primitives.r1cs-gate-shape]
pub open spec fn r1cs_gate_indices_valid_spec(gate: R1csGate) -> bool {
    gate.x < gate.witness_len
        && (gate.kind == R1CS_GATE_CONST || gate.y < gate.witness_len)
        && (gate.kind == R1CS_GATE_CONST || gate.z < gate.witness_len)
        && (gate.kind != R1CS_GATE_ADD || gate.x != gate.y)
}

// r[depends rust-primitives.r1cs-gate-shape]
pub open spec fn r1cs_gate_valid_spec(gate: R1csGate) -> bool {
    r1cs_gate_kind_valid_spec(gate.kind)
        && r1cs_len_valid_spec(gate.witness_len)
        && r1cs_modulus_valid_spec(gate.modulus)
        && r1cs_gate_indices_valid_spec(gate)
        && (gate.kind != R1CS_GATE_CONST || r1cs_element_valid_spec(gate.value))
}

// r[depends rust-primitives.r1cs-gate-row-encoding]
pub open spec fn r1cs_one_at_spec(index: u8, target: u8) -> u16 {
    if index == target { 1 } else { 0 }
}

// r[depends rust-primitives.r1cs-gate-row-encoding]
pub open spec fn r1cs_add_a_at_spec(gate: R1csGate, target: u8) -> u16 {
    if target == gate.x || target == gate.y { 1 } else { 0 }
}

// r[depends rust-primitives.r1cs-gate-row-encoding]
pub open spec fn r1cs_gate_row_spec(gate: R1csGate) -> R1csRow {
    if gate.kind == R1CS_GATE_MUL {
        R1csRow {
            len: gate.witness_len,
            modulus: gate.modulus,
            a0: r1cs_one_at_spec(gate.x, 0),
            a1: r1cs_one_at_spec(gate.x, 1),
            a2: r1cs_one_at_spec(gate.x, 2),
            b0: r1cs_one_at_spec(gate.y, 0),
            b1: r1cs_one_at_spec(gate.y, 1),
            b2: r1cs_one_at_spec(gate.y, 2),
            c0: r1cs_one_at_spec(gate.z, 0),
            c1: r1cs_one_at_spec(gate.z, 1),
            c2: r1cs_one_at_spec(gate.z, 2),
        }
    } else if gate.kind == R1CS_GATE_ADD {
        R1csRow {
            len: gate.witness_len,
            modulus: gate.modulus,
            a0: r1cs_add_a_at_spec(gate, 0),
            a1: r1cs_add_a_at_spec(gate, 1),
            a2: r1cs_add_a_at_spec(gate, 2),
            b0: 1,
            b1: 0,
            b2: 0,
            c0: r1cs_one_at_spec(gate.z, 0),
            c1: r1cs_one_at_spec(gate.z, 1),
            c2: r1cs_one_at_spec(gate.z, 2),
        }
    } else {
        R1csRow {
            len: gate.witness_len,
            modulus: gate.modulus,
            a0: r1cs_one_at_spec(gate.x, 0),
            a1: r1cs_one_at_spec(gate.x, 1),
            a2: r1cs_one_at_spec(gate.x, 2),
            b0: 1,
            b1: 0,
            b2: 0,
            c0: gate.value,
            c1: 0,
            c2: 0,
        }
    }
}

// r[depends rust-primitives.r1cs-gate-satisfaction]
pub open spec fn r1cs_gate_one_wire_satisfied_spec(gate: R1csGate, witness: R1csWitness) -> bool {
    !r1cs_gate_requires_one_wire_spec(gate) || witness.w0 == 1
}

// r[depends rust-primitives.r1cs-gate-satisfaction]
pub open spec fn r1cs_gate_satisfied_spec(gate: R1csGate, witness: R1csWitness) -> bool {
    r1cs_gate_valid_spec(gate)
        && r1cs_witness_valid_spec(witness)
        && witness.len == gate.witness_len
        && r1cs_gate_one_wire_satisfied_spec(gate, witness)
        && r1cs_row_satisfied_spec(r1cs_gate_row_spec(gate), witness)
}

// r[impl rust-primitives.r1cs-gate-shape]
// r[impl rust-primitives.r1cs-gate-verification-coverage]
pub fn r1cs_gate_kind_valid(kind: u8) -> (result: bool)
    ensures result == r1cs_gate_kind_valid_spec(kind)
{
    kind == R1CS_GATE_MUL || kind == R1CS_GATE_ADD || kind == R1CS_GATE_CONST
}

// r[impl rust-primitives.r1cs-gate-shape]
pub fn r1cs_gate_requires_one_wire(gate: R1csGate) -> (result: bool)
    ensures result == r1cs_gate_requires_one_wire_spec(gate)
{
    gate.kind == R1CS_GATE_ADD || gate.kind == R1CS_GATE_CONST
}

// r[impl rust-primitives.r1cs-gate-shape]
pub fn r1cs_gate_indices_valid(gate: R1csGate) -> (result: bool)
    ensures result == r1cs_gate_indices_valid_spec(gate)
{
    gate.x < gate.witness_len
        && (gate.kind == R1CS_GATE_CONST || gate.y < gate.witness_len)
        && (gate.kind == R1CS_GATE_CONST || gate.z < gate.witness_len)
        && (gate.kind != R1CS_GATE_ADD || gate.x != gate.y)
}

// r[impl rust-primitives.r1cs-gate-shape]
pub fn r1cs_gate_valid(gate: R1csGate) -> (result: bool)
    ensures result == r1cs_gate_valid_spec(gate)
{
    r1cs_gate_kind_valid(gate.kind)
        && r1cs_len_valid(gate.witness_len)
        && r1cs_modulus_valid(gate.modulus)
        && r1cs_gate_indices_valid(gate)
        && (gate.kind != R1CS_GATE_CONST || r1cs_element_valid(gate.value))
}

// r[impl rust-primitives.r1cs-gate-row-encoding]
pub fn r1cs_one_at(index: u8, target: u8) -> (result: u16)
    ensures result == r1cs_one_at_spec(index, target)
{
    if index == target { 1 } else { 0 }
}

// r[impl rust-primitives.r1cs-gate-row-encoding]
pub fn r1cs_add_a_at(gate: R1csGate, target: u8) -> (result: u16)
    requires gate.kind == R1CS_GATE_ADD, gate.x != gate.y
    ensures result == r1cs_add_a_at_spec(gate, target)
{
    if target == gate.x || target == gate.y { 1 } else { 0 }
}

// r[impl rust-primitives.r1cs-gate-row-encoding]
pub fn r1cs_gate_row(gate: R1csGate) -> (result: R1csRow)
    requires r1cs_gate_valid_spec(gate)
    ensures
        result == r1cs_gate_row_spec(gate),
        r1cs_row_valid_spec(result),
{
    if gate.kind == R1CS_GATE_MUL {
        R1csRow {
            len: gate.witness_len,
            modulus: gate.modulus,
            a0: r1cs_one_at(gate.x, 0),
            a1: r1cs_one_at(gate.x, 1),
            a2: r1cs_one_at(gate.x, 2),
            b0: r1cs_one_at(gate.y, 0),
            b1: r1cs_one_at(gate.y, 1),
            b2: r1cs_one_at(gate.y, 2),
            c0: r1cs_one_at(gate.z, 0),
            c1: r1cs_one_at(gate.z, 1),
            c2: r1cs_one_at(gate.z, 2),
        }
    } else if gate.kind == R1CS_GATE_ADD {
        R1csRow {
            len: gate.witness_len,
            modulus: gate.modulus,
            a0: r1cs_add_a_at(gate, 0),
            a1: r1cs_add_a_at(gate, 1),
            a2: r1cs_add_a_at(gate, 2),
            b0: 1,
            b1: 0,
            b2: 0,
            c0: r1cs_one_at(gate.z, 0),
            c1: r1cs_one_at(gate.z, 1),
            c2: r1cs_one_at(gate.z, 2),
        }
    } else {
        R1csRow {
            len: gate.witness_len,
            modulus: gate.modulus,
            a0: r1cs_one_at(gate.x, 0),
            a1: r1cs_one_at(gate.x, 1),
            a2: r1cs_one_at(gate.x, 2),
            b0: 1,
            b1: 0,
            b2: 0,
            c0: gate.value,
            c1: 0,
            c2: 0,
        }
    }
}

// r[impl rust-primitives.r1cs-gate-satisfaction]
pub fn r1cs_gate_one_wire_satisfied(gate: R1csGate, witness: R1csWitness) -> (result: bool)
    ensures result == r1cs_gate_one_wire_satisfied_spec(gate, witness)
{
    !r1cs_gate_requires_one_wire(gate) || witness.w0 == 1
}

// r[impl rust-primitives.r1cs-gate-satisfaction]
pub fn r1cs_gate_satisfied(gate: R1csGate, witness: R1csWitness) -> (result: bool)
    ensures result == r1cs_gate_satisfied_spec(gate, witness)
{
    if !r1cs_gate_valid(gate) {
        return false;
    }
    if !r1cs_witness_valid(witness) {
        return false;
    }
    if witness.len != gate.witness_len {
        return false;
    }
    if !r1cs_gate_one_wire_satisfied(gate, witness) {
        return false;
    }
    let row = r1cs_gate_row(gate);
    r1cs_row_satisfied(row, witness)
}

// r[verify rust-primitives.r1cs-gate-shape]
pub proof fn malformed_gate_rejected(gate: R1csGate, witness: R1csWitness)
    requires !r1cs_gate_valid_spec(gate)
    ensures !r1cs_gate_satisfied_spec(gate, witness)
{}

// r[verify rust-primitives.r1cs-gate-row-encoding]
pub proof fn admitted_gate_encodes_valid_row(gate: R1csGate)
    requires r1cs_gate_valid_spec(gate)
    ensures r1cs_row_valid_spec(r1cs_gate_row_spec(gate))
{}

// r[verify rust-primitives.r1cs-gate-satisfaction]
pub proof fn malformed_gate_witness_rejected(gate: R1csGate, witness: R1csWitness)
    requires !r1cs_witness_valid_spec(witness) || witness.len != gate.witness_len
    ensures !r1cs_gate_satisfied_spec(gate, witness)
{}

// r[verify rust-primitives.r1cs-gate-satisfaction]
pub proof fn missing_one_wire_rejects_add_or_const(gate: R1csGate, witness: R1csWitness)
    requires r1cs_gate_requires_one_wire_spec(gate), witness.w0 != 1
    ensures !r1cs_gate_satisfied_spec(gate, witness)
{}

// r[verify rust-primitives.r1cs-gate-satisfaction]
pub proof fn unsatisfied_encoded_row_rejects_gate(gate: R1csGate, witness: R1csWitness)
    requires
        r1cs_gate_valid_spec(gate),
        r1cs_witness_valid_spec(witness),
        witness.len == gate.witness_len,
        r1cs_gate_one_wire_satisfied_spec(gate, witness),
        !r1cs_row_satisfied_spec(r1cs_gate_row_spec(gate), witness),
    ensures !r1cs_gate_satisfied_spec(gate, witness)
{}

// r[verify rust-primitives.r1cs-gate-verification-coverage]
pub proof fn r1cs_gate_primitive_coverage_catalogued(gate: R1csGate, witness: R1csWitness)
    ensures
        !r1cs_gate_valid_spec(gate) ==> !r1cs_gate_satisfied_spec(gate, witness),
        (!r1cs_witness_valid_spec(witness) || witness.len != gate.witness_len)
            ==> !r1cs_gate_satisfied_spec(gate, witness),
        (r1cs_gate_requires_one_wire_spec(gate) && witness.w0 != 1)
            ==> !r1cs_gate_satisfied_spec(gate, witness),
        (r1cs_gate_valid_spec(gate)
            && r1cs_witness_valid_spec(witness)
            && witness.len == gate.witness_len
            && r1cs_gate_one_wire_satisfied_spec(gate, witness)
            && !r1cs_row_satisfied_spec(r1cs_gate_row_spec(gate), witness))
            ==> !r1cs_gate_satisfied_spec(gate, witness),
        r1cs_gate_satisfied_spec(gate, witness)
            ==> (r1cs_gate_valid_spec(gate)
                && r1cs_witness_valid_spec(witness)
                && witness.len == gate.witness_len
                && r1cs_gate_one_wire_satisfied_spec(gate, witness)
                && r1cs_row_satisfied_spec(r1cs_gate_row_spec(gate), witness)),
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    const UNSUPPORTED_GATE_KIND: u8 = 3;
    const MISMATCHED_WITNESS_LEN: u8 = 2;
    const OVERLARGE_WITNESS_VALUE: u16 = 16;
    const BAD_ADDITION_OUTPUT: u16 = 7;

    fn mul_witness() -> R1csWitness {
        R1csWitness {
            len: 3,
            w0: 3,
            w1: 4,
            w2: 12,
        }
    }

    fn one_wire_witness() -> R1csWitness {
        R1csWitness {
            len: 3,
            w0: 1,
            w1: 5,
            w2: 6,
        }
    }

    fn mul_gate() -> R1csGate {
        R1csGate {
            kind: R1CS_GATE_MUL,
            witness_len: 3,
            modulus: 17,
            x: 0,
            y: 1,
            z: 2,
            value: 0,
        }
    }

    fn add_gate() -> R1csGate {
        R1csGate {
            kind: R1CS_GATE_ADD,
            witness_len: 3,
            modulus: 17,
            x: 0,
            y: 1,
            z: 2,
            value: 0,
        }
    }

    fn const_gate() -> R1csGate {
        R1csGate {
            kind: R1CS_GATE_CONST,
            witness_len: 3,
            modulus: 17,
            x: 1,
            y: 0,
            z: 0,
            value: 5,
        }
    }

    #[test]
    fn multiplication_gate_encodes_satisfied_row() {
        let gate = mul_gate();
        let witness = mul_witness();
        let row = r1cs_gate_row(gate);
        assert_eq!(row.a0, 1);
        assert_eq!(row.b1, 1);
        assert_eq!(row.c2, 1);
        assert!(r1cs_row_satisfied(row, witness));
        assert!(r1cs_gate_satisfied(gate, witness));
    }

    #[test]
    fn multiplication_gate_rejects_bad_product() {
        let gate = mul_gate();
        let mut witness = mul_witness();
        witness.w2 = 13;
        assert!(r1cs_gate_valid(gate));
        assert!(!r1cs_gate_satisfied(gate, witness));
    }

    #[test]
    fn addition_gate_encodes_satisfied_row_with_one_wire() {
        let gate = add_gate();
        let witness = one_wire_witness();
        let row = r1cs_gate_row(gate);
        assert_eq!(row.a0, 1);
        assert_eq!(row.a1, 1);
        assert_eq!(row.b0, 1);
        assert_eq!(row.c2, 1);
        assert!(r1cs_gate_satisfied(gate, witness));
    }

    #[test]
    fn addition_gate_requires_one_wire() {
        let gate = add_gate();
        let mut witness = one_wire_witness();
        witness.w0 = 2;
        assert!(!r1cs_gate_satisfied(gate, witness));
    }

    #[test]
    fn constant_gate_encodes_satisfied_row_with_one_wire() {
        let gate = const_gate();
        let witness = one_wire_witness();
        let row = r1cs_gate_row(gate);
        assert_eq!(row.a1, 1);
        assert_eq!(row.b0, 1);
        assert_eq!(row.c0, 5);
        assert!(r1cs_gate_satisfied(gate, witness));
    }

    #[test]
    fn constant_gate_rejects_mismatched_value() {
        let mut gate = const_gate();
        gate.value = 6;
        assert!(!r1cs_gate_satisfied(gate, one_wire_witness()));
    }

    #[test]
    fn out_of_range_variable_index_is_rejected() {
        let mut gate = mul_gate();
        gate.z = 3;
        assert!(!r1cs_gate_valid(gate));
        assert!(!r1cs_gate_satisfied(gate, mul_witness()));
    }

    #[test]
    fn unsupported_gate_kind_is_rejected() {
        let mut gate = mul_gate();
        gate.kind = UNSUPPORTED_GATE_KIND;
        assert!(!r1cs_gate_kind_valid(gate.kind));
        assert!(!r1cs_gate_valid(gate));
        assert!(!r1cs_gate_satisfied(gate, mul_witness()));
    }

    #[test]
    fn mismatched_witness_length_is_rejected() {
        let gate = mul_gate();
        let mut witness = mul_witness();
        witness.len = MISMATCHED_WITNESS_LEN;
        assert!(r1cs_witness_valid(witness));
        assert!(!r1cs_gate_satisfied(gate, witness));
    }

    #[test]
    fn malformed_witness_is_rejected() {
        let gate = mul_gate();
        let mut witness = mul_witness();
        witness.w0 = OVERLARGE_WITNESS_VALUE;
        assert!(!r1cs_witness_valid(witness));
        assert!(!r1cs_gate_satisfied(gate, witness));
    }

    #[test]
    fn valid_addition_gate_rejects_unsatisfied_encoded_row() {
        let gate = add_gate();
        let mut witness = one_wire_witness();
        witness.w2 = BAD_ADDITION_OUTPUT;
        let row = r1cs_gate_row(gate);
        assert!(r1cs_gate_valid(gate));
        assert!(r1cs_witness_valid(witness));
        assert!(r1cs_gate_one_wire_satisfied(gate, witness));
        assert!(!r1cs_row_satisfied(row, witness));
        assert!(!r1cs_gate_satisfied(gate, witness));
    }

    #[test]
    fn malformed_constant_value_is_rejected() {
        let mut gate = const_gate();
        gate.value = 16;
        assert!(!r1cs_gate_valid(gate));
        assert!(!r1cs_gate_satisfied(gate, one_wire_witness()));
    }

    #[test]
    fn duplicate_add_inputs_are_rejected_to_keep_encoding_finite() {
        let mut gate = add_gate();
        gate.y = gate.x;
        assert!(!r1cs_gate_valid(gate));
    }
}
