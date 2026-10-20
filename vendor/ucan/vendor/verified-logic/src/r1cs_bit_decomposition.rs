//! Finite R1CS two-bit decomposition primitives.
//!
//! This module composes the bounded R1CS boolean primitive for two witness
//! slots and checks deterministic recomposition `value = bit0 + 2 * bit1`.
//! It deliberately stops at a tiny semantic boundary and does not model
//! arbitrary-width range checks, circuit builders, proof systems, transcripts,
//! commitments, or cryptographic soundness.

use vstd::prelude::*;

use crate::r1cs_boolean::R1csBoolean;
use crate::r1cs_boolean::r1cs_boolean_modulus_coeff_valid;
#[cfg(verus_keep_ghost)]
use crate::r1cs_boolean::r1cs_boolean_modulus_coeff_valid_spec;
use crate::r1cs_boolean::r1cs_boolean_one_wire_satisfied;
#[cfg(verus_keep_ghost)]
use crate::r1cs_boolean::r1cs_boolean_one_wire_satisfied_spec;
use crate::r1cs_boolean::r1cs_boolean_satisfied;
#[cfg(verus_keep_ghost)]
use crate::r1cs_boolean::r1cs_boolean_satisfied_spec;
#[cfg(verus_keep_ghost)]
use crate::r1cs_boolean::r1cs_boolean_valid_spec;
use crate::r1cs_constraint::R1csWitness;
#[cfg(verus_keep_ghost)]
use crate::r1cs_constraint::r1cs_element_valid_spec;
use crate::r1cs_constraint::r1cs_len_valid;
#[cfg(verus_keep_ghost)]
use crate::r1cs_constraint::r1cs_len_valid_spec;
use crate::r1cs_constraint::r1cs_witness_valid;
#[cfg(verus_keep_ghost)]
use crate::r1cs_constraint::r1cs_witness_valid_spec;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct R1csBitDecomposition {
    pub witness_len: u8,
    pub modulus: u16,
    pub value: u16,
    pub bit0_index: u8,
    pub bit1_index: u8,
}

// r[depends rust-primitives.r1cs-bit-decomposition-shape]
pub open spec fn r1cs_bit_decomposition_value_valid_spec(value: u16) -> bool {
    r1cs_element_valid_spec(value) && value < 4
}

// r[depends rust-primitives.r1cs-bit-decomposition-shape]
pub open spec fn r1cs_bit_decomposition_indices_valid_spec(decomposition: R1csBitDecomposition) -> bool {
    0 < decomposition.bit0_index
        && decomposition.bit0_index < decomposition.witness_len
        && 0 < decomposition.bit1_index
        && decomposition.bit1_index < decomposition.witness_len
        && decomposition.bit0_index != decomposition.bit1_index
}

// r[depends rust-primitives.r1cs-bit-decomposition-shape]
pub open spec fn r1cs_bit_decomposition_valid_spec(decomposition: R1csBitDecomposition) -> bool {
    r1cs_len_valid_spec(decomposition.witness_len)
        && r1cs_boolean_modulus_coeff_valid_spec(decomposition.modulus)
        && r1cs_bit_decomposition_value_valid_spec(decomposition.value)
        && r1cs_bit_decomposition_indices_valid_spec(decomposition)
}

// r[depends rust-primitives.r1cs-bit-decomposition-boolean-composition]
pub open spec fn r1cs_bit_decomposition_bit0_spec(decomposition: R1csBitDecomposition) -> R1csBoolean {
    R1csBoolean {
        witness_len: decomposition.witness_len,
        modulus: decomposition.modulus,
        index: decomposition.bit0_index,
    }
}

// r[depends rust-primitives.r1cs-bit-decomposition-boolean-composition]
pub open spec fn r1cs_bit_decomposition_bit1_spec(decomposition: R1csBitDecomposition) -> R1csBoolean {
    R1csBoolean {
        witness_len: decomposition.witness_len,
        modulus: decomposition.modulus,
        index: decomposition.bit1_index,
    }
}

// r[depends rust-primitives.r1cs-bit-decomposition-satisfaction]
pub open spec fn r1cs_bit_value_spec(witness: R1csWitness, index: u8) -> int {
    if index == 1 { witness.w1 as int } else if index == 2 { witness.w2 as int } else { 0int }
}

// r[depends rust-primitives.r1cs-bit-decomposition-satisfaction]
pub open spec fn r1cs_bit_recomposition_spec(decomposition: R1csBitDecomposition, witness: R1csWitness) -> int {
    r1cs_bit_value_spec(witness, decomposition.bit0_index)
        + 2int * r1cs_bit_value_spec(witness, decomposition.bit1_index)
}

// r[depends rust-primitives.r1cs-bit-decomposition-satisfaction]
pub open spec fn r1cs_bit_decomposition_satisfied_spec(decomposition: R1csBitDecomposition, witness: R1csWitness) -> bool {
    r1cs_bit_decomposition_valid_spec(decomposition)
        && r1cs_witness_valid_spec(witness)
        && witness.len == decomposition.witness_len
        && r1cs_boolean_one_wire_satisfied_spec(witness)
        && r1cs_boolean_satisfied_spec(r1cs_bit_decomposition_bit0_spec(decomposition), witness)
        && r1cs_boolean_satisfied_spec(r1cs_bit_decomposition_bit1_spec(decomposition), witness)
        && r1cs_bit_recomposition_spec(decomposition, witness) == decomposition.value as int
}

// r[impl rust-primitives.r1cs-bit-decomposition-shape]
// r[impl rust-primitives.r1cs-bit-decomposition-verification-coverage]
pub fn r1cs_bit_decomposition_value_valid(value: u16) -> (result: bool)
    ensures result == r1cs_bit_decomposition_value_valid_spec(value)
{
    value < 4
}

// r[impl rust-primitives.r1cs-bit-decomposition-shape]
pub fn r1cs_bit_decomposition_indices_valid(decomposition: R1csBitDecomposition) -> (result: bool)
    ensures result == r1cs_bit_decomposition_indices_valid_spec(decomposition)
{
    0 < decomposition.bit0_index
        && decomposition.bit0_index < decomposition.witness_len
        && 0 < decomposition.bit1_index
        && decomposition.bit1_index < decomposition.witness_len
        && decomposition.bit0_index != decomposition.bit1_index
}

// r[impl rust-primitives.r1cs-bit-decomposition-shape]
pub fn r1cs_bit_decomposition_valid(decomposition: R1csBitDecomposition) -> (result: bool)
    ensures result == r1cs_bit_decomposition_valid_spec(decomposition)
{
    r1cs_len_valid(decomposition.witness_len)
        && r1cs_boolean_modulus_coeff_valid(decomposition.modulus)
        && r1cs_bit_decomposition_value_valid(decomposition.value)
        && r1cs_bit_decomposition_indices_valid(decomposition)
}

// r[impl rust-primitives.r1cs-bit-decomposition-boolean-composition]
pub fn r1cs_bit_decomposition_bit0(decomposition: R1csBitDecomposition) -> (result: R1csBoolean)
    requires r1cs_bit_decomposition_valid_spec(decomposition)
    ensures
        result == r1cs_bit_decomposition_bit0_spec(decomposition),
        r1cs_boolean_valid_spec(result),
{
    R1csBoolean {
        witness_len: decomposition.witness_len,
        modulus: decomposition.modulus,
        index: decomposition.bit0_index,
    }
}

// r[impl rust-primitives.r1cs-bit-decomposition-boolean-composition]
pub fn r1cs_bit_decomposition_bit1(decomposition: R1csBitDecomposition) -> (result: R1csBoolean)
    requires r1cs_bit_decomposition_valid_spec(decomposition)
    ensures
        result == r1cs_bit_decomposition_bit1_spec(decomposition),
        r1cs_boolean_valid_spec(result),
{
    R1csBoolean {
        witness_len: decomposition.witness_len,
        modulus: decomposition.modulus,
        index: decomposition.bit1_index,
    }
}

// r[impl rust-primitives.r1cs-bit-decomposition-satisfaction]
pub fn r1cs_bit_value(witness: R1csWitness, index: u8) -> (result: u16)
    ensures result as int == r1cs_bit_value_spec(witness, index)
{
    if index == 1 {
        witness.w1
    } else if index == 2 {
        witness.w2
    } else {
        0
    }
}

// r[impl rust-primitives.r1cs-bit-decomposition-satisfaction]
pub fn r1cs_bit_recomposition(decomposition: R1csBitDecomposition, witness: R1csWitness) -> (result: u32)
    ensures result as int == r1cs_bit_recomposition_spec(decomposition, witness)
{
    let bit0 = r1cs_bit_value(witness, decomposition.bit0_index);
    let bit1 = r1cs_bit_value(witness, decomposition.bit1_index);
    assert((bit0 as int) <= 65535int);
    assert((bit1 as int) <= 65535int);
    assert(2int * (bit1 as int) <= 131070int) by(nonlinear_arith)
        requires bit1 <= 65535;
    assert((bit0 as int) + 2int * (bit1 as int) <= 196605int) by(nonlinear_arith)
        requires bit0 <= 65535, bit1 <= 65535;
    (bit0 as u32) + 2u32 * (bit1 as u32)
}

// r[impl rust-primitives.r1cs-bit-decomposition-satisfaction]
pub fn r1cs_bit_decomposition_satisfied(decomposition: R1csBitDecomposition, witness: R1csWitness) -> (result: bool)
    ensures result == r1cs_bit_decomposition_satisfied_spec(decomposition, witness)
{
    if !r1cs_bit_decomposition_valid(decomposition) {
        return false;
    }
    if !r1cs_witness_valid(witness) {
        return false;
    }
    if witness.len != decomposition.witness_len {
        return false;
    }
    if !r1cs_boolean_one_wire_satisfied(witness) {
        return false;
    }
    let bit0 = r1cs_bit_decomposition_bit0(decomposition);
    if !r1cs_boolean_satisfied(bit0, witness) {
        return false;
    }
    let bit1 = r1cs_bit_decomposition_bit1(decomposition);
    if !r1cs_boolean_satisfied(bit1, witness) {
        return false;
    }
    r1cs_bit_recomposition(decomposition, witness) == decomposition.value as u32
}

// r[verify rust-primitives.r1cs-bit-decomposition-shape]
pub proof fn malformed_bit_decomposition_shape_rejected(decomposition: R1csBitDecomposition, witness: R1csWitness)
    requires !r1cs_bit_decomposition_valid_spec(decomposition)
    ensures !r1cs_bit_decomposition_satisfied_spec(decomposition, witness)
{}

// r[verify rust-primitives.r1cs-bit-decomposition-boolean-composition]
pub proof fn non_boolean_bit_rejected(decomposition: R1csBitDecomposition, witness: R1csWitness)
    requires
        r1cs_bit_decomposition_valid_spec(decomposition),
        r1cs_witness_valid_spec(witness),
        witness.len == decomposition.witness_len,
        (!r1cs_boolean_satisfied_spec(r1cs_bit_decomposition_bit0_spec(decomposition), witness)
            || !r1cs_boolean_satisfied_spec(r1cs_bit_decomposition_bit1_spec(decomposition), witness)),
    ensures !r1cs_bit_decomposition_satisfied_spec(decomposition, witness)
{}

// r[verify rust-primitives.r1cs-bit-decomposition-satisfaction]
pub proof fn mismatched_bit_recomposition_rejected(decomposition: R1csBitDecomposition, witness: R1csWitness)
    requires r1cs_bit_recomposition_spec(decomposition, witness) != decomposition.value as int
    ensures !r1cs_bit_decomposition_satisfied_spec(decomposition, witness)
{}

// r[verify rust-primitives.r1cs-bit-decomposition-verification-coverage]
pub proof fn r1cs_bit_decomposition_primitive_coverage_catalogued()
    ensures true
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    fn two_bit_value(value: u16) -> R1csBitDecomposition {
        R1csBitDecomposition {
            witness_len: 3,
            modulus: 13,
            value,
            bit0_index: 1,
            bit1_index: 2,
        }
    }

    fn witness(bit0: u16, bit1: u16) -> R1csWitness {
        R1csWitness {
            len: 3,
            w0: 1,
            w1: bit0,
            w2: bit1,
        }
    }

    #[test]
    fn decomposes_zero() {
        assert!(r1cs_bit_decomposition_satisfied(two_bit_value(0), witness(0, 0)));
    }

    #[test]
    fn decomposes_three() {
        assert!(r1cs_bit_decomposition_satisfied(two_bit_value(3), witness(1, 1)));
    }

    #[test]
    fn recomposition_mismatch_is_rejected() {
        assert!(!r1cs_bit_decomposition_satisfied(two_bit_value(2), witness(1, 0)));
    }

    #[test]
    fn non_boolean_low_bit_is_rejected() {
        assert!(!r1cs_bit_decomposition_satisfied(two_bit_value(0), witness(2, 0)));
    }

    #[test]
    fn non_boolean_high_bit_is_rejected() {
        assert!(!r1cs_bit_decomposition_satisfied(two_bit_value(1), witness(1, 2)));
    }

    #[test]
    fn out_of_range_value_is_rejected() {
        assert!(!r1cs_bit_decomposition_valid(two_bit_value(4)));
        assert!(!r1cs_bit_decomposition_satisfied(two_bit_value(4), witness(0, 0)));
    }

    #[test]
    fn duplicate_bit_indices_are_rejected() {
        let mut decomposition = two_bit_value(1);
        decomposition.bit1_index = decomposition.bit0_index;
        assert!(!r1cs_bit_decomposition_valid(decomposition));
        assert!(!r1cs_bit_decomposition_satisfied(decomposition, witness(1, 0)));
    }

    #[test]
    fn zero_bit_index_is_rejected() {
        let mut decomposition = two_bit_value(1);
        decomposition.bit0_index = 0;
        assert!(!r1cs_bit_decomposition_valid(decomposition));
        assert!(!r1cs_bit_decomposition_satisfied(decomposition, witness(1, 0)));
    }

    #[test]
    fn missing_one_wire_is_rejected() {
        let mut bad = witness(1, 0);
        bad.w0 = 0;
        assert!(!r1cs_bit_decomposition_satisfied(two_bit_value(1), bad));
    }
}
