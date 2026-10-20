//! Finite R1CS booleanity primitives.
//!
//! This module encodes a bounded boolean witness constraint into one existing
//! R1CS row. It models only deterministic boolean admission and satisfaction;
//! it does not model bit decomposition, range checks, proof systems,
//! transcripts, commitments, or cryptographic soundness.

use vstd::prelude::*;

use crate::r1cs_constraint::R1csRow;
use crate::r1cs_constraint::R1csWitness;
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
use crate::r1cs_gates::r1cs_one_at;
#[cfg(verus_keep_ghost)]
use crate::r1cs_gates::r1cs_one_at_spec;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct R1csBoolean {
    pub witness_len: u8,
    pub modulus: u16,
    pub index: u8,
}

// r[depends rust-primitives.r1cs-boolean-shape]
pub open spec fn r1cs_boolean_modulus_coeff_valid_spec(modulus: u16) -> bool {
    r1cs_modulus_valid_spec(modulus) && modulus > 0 && r1cs_element_valid_spec((modulus - 1) as u16)
}

// r[depends rust-primitives.r1cs-boolean-shape]
pub open spec fn r1cs_boolean_index_valid_spec(boolean: R1csBoolean) -> bool {
    0 < boolean.index && boolean.index < boolean.witness_len
}

// r[depends rust-primitives.r1cs-boolean-shape]
pub open spec fn r1cs_boolean_valid_spec(boolean: R1csBoolean) -> bool {
    r1cs_len_valid_spec(boolean.witness_len)
        && r1cs_boolean_modulus_coeff_valid_spec(boolean.modulus)
        && r1cs_boolean_index_valid_spec(boolean)
}

// r[depends rust-primitives.r1cs-boolean-row-encoding]
pub open spec fn r1cs_boolean_row_spec(boolean: R1csBoolean) -> R1csRow {
    R1csRow {
        len: boolean.witness_len,
        modulus: boolean.modulus,
        a0: r1cs_one_at_spec(boolean.index, 0),
        a1: r1cs_one_at_spec(boolean.index, 1),
        a2: r1cs_one_at_spec(boolean.index, 2),
        b0: 1,
        b1: if boolean.index == 1 { (boolean.modulus - 1) as u16 } else { 0 },
        b2: if boolean.index == 2 { (boolean.modulus - 1) as u16 } else { 0 },
        c0: 0,
        c1: 0,
        c2: 0,
    }
}

// r[depends rust-primitives.r1cs-boolean-satisfaction]
pub open spec fn r1cs_boolean_one_wire_satisfied_spec(witness: R1csWitness) -> bool {
    witness.w0 == 1
}

// r[depends rust-primitives.r1cs-boolean-satisfaction]
pub open spec fn r1cs_boolean_satisfied_spec(boolean: R1csBoolean, witness: R1csWitness) -> bool {
    r1cs_boolean_valid_spec(boolean)
        && r1cs_witness_valid_spec(witness)
        && witness.len == boolean.witness_len
        && r1cs_boolean_one_wire_satisfied_spec(witness)
        && r1cs_row_satisfied_spec(r1cs_boolean_row_spec(boolean), witness)
}

// r[impl rust-primitives.r1cs-boolean-shape]
// r[impl rust-primitives.r1cs-boolean-verification-coverage]
pub fn r1cs_boolean_modulus_coeff_valid(modulus: u16) -> (result: bool)
    ensures result == r1cs_boolean_modulus_coeff_valid_spec(modulus)
{
    r1cs_modulus_valid(modulus) && modulus > 0 && modulus - 1 <= 15
}

// r[impl rust-primitives.r1cs-boolean-shape]
pub fn r1cs_boolean_index_valid(boolean: R1csBoolean) -> (result: bool)
    ensures result == r1cs_boolean_index_valid_spec(boolean)
{
    0 < boolean.index && boolean.index < boolean.witness_len
}

// r[impl rust-primitives.r1cs-boolean-shape]
pub fn r1cs_boolean_valid(boolean: R1csBoolean) -> (result: bool)
    ensures result == r1cs_boolean_valid_spec(boolean)
{
    r1cs_len_valid(boolean.witness_len)
        && r1cs_boolean_modulus_coeff_valid(boolean.modulus)
        && r1cs_boolean_index_valid(boolean)
}

// r[impl rust-primitives.r1cs-boolean-row-encoding]
pub fn r1cs_boolean_row(boolean: R1csBoolean) -> (result: R1csRow)
    requires r1cs_boolean_valid_spec(boolean)
    ensures
        result == r1cs_boolean_row_spec(boolean),
        r1cs_row_valid_spec(result),
{
    let neg_one = boolean.modulus - 1;
    assert(neg_one <= 15);
    R1csRow {
        len: boolean.witness_len,
        modulus: boolean.modulus,
        a0: r1cs_one_at(boolean.index, 0),
        a1: r1cs_one_at(boolean.index, 1),
        a2: r1cs_one_at(boolean.index, 2),
        b0: 1,
        b1: if boolean.index == 1 { neg_one } else { 0 },
        b2: if boolean.index == 2 { neg_one } else { 0 },
        c0: 0,
        c1: 0,
        c2: 0,
    }
}

// r[impl rust-primitives.r1cs-boolean-satisfaction]
pub fn r1cs_boolean_one_wire_satisfied(witness: R1csWitness) -> (result: bool)
    ensures result == r1cs_boolean_one_wire_satisfied_spec(witness)
{
    witness.w0 == 1
}

// r[impl rust-primitives.r1cs-boolean-satisfaction]
pub fn r1cs_boolean_satisfied(boolean: R1csBoolean, witness: R1csWitness) -> (result: bool)
    ensures result == r1cs_boolean_satisfied_spec(boolean, witness)
{
    if !r1cs_boolean_valid(boolean) {
        return false;
    }
    if !r1cs_witness_valid(witness) {
        return false;
    }
    if witness.len != boolean.witness_len {
        return false;
    }
    if !r1cs_boolean_one_wire_satisfied(witness) {
        return false;
    }
    let row = r1cs_boolean_row(boolean);
    r1cs_row_satisfied(row, witness)
}

// r[verify rust-primitives.r1cs-boolean-shape]
pub proof fn malformed_boolean_shape_rejected(boolean: R1csBoolean, witness: R1csWitness)
    requires !r1cs_boolean_valid_spec(boolean)
    ensures !r1cs_boolean_satisfied_spec(boolean, witness)
{}

// r[verify rust-primitives.r1cs-boolean-row-encoding]
pub proof fn admitted_boolean_encodes_valid_row(boolean: R1csBoolean)
    requires r1cs_boolean_valid_spec(boolean)
    ensures r1cs_row_valid_spec(r1cs_boolean_row_spec(boolean))
{}

// r[verify rust-primitives.r1cs-boolean-satisfaction]
pub proof fn malformed_boolean_witness_rejected(boolean: R1csBoolean, witness: R1csWitness)
    requires !r1cs_witness_valid_spec(witness) || witness.len != boolean.witness_len
    ensures !r1cs_boolean_satisfied_spec(boolean, witness)
{}

// r[verify rust-primitives.r1cs-boolean-satisfaction]
pub proof fn missing_boolean_one_wire_rejected(boolean: R1csBoolean, witness: R1csWitness)
    requires witness.w0 != 1
    ensures !r1cs_boolean_satisfied_spec(boolean, witness)
{}

// r[verify rust-primitives.r1cs-boolean-satisfaction]
pub proof fn unsatisfied_boolean_row_rejects(boolean: R1csBoolean, witness: R1csWitness)
    requires
        r1cs_boolean_valid_spec(boolean),
        r1cs_witness_valid_spec(witness),
        witness.len == boolean.witness_len,
        r1cs_boolean_one_wire_satisfied_spec(witness),
        !r1cs_row_satisfied_spec(r1cs_boolean_row_spec(boolean), witness),
    ensures !r1cs_boolean_satisfied_spec(boolean, witness)
{}

// r[verify rust-primitives.r1cs-boolean-verification-coverage]
pub proof fn r1cs_boolean_primitive_coverage_catalogued(
    boolean: R1csBoolean,
    witness: R1csWitness,
)
    ensures
        !r1cs_boolean_valid_spec(boolean) ==> !r1cs_boolean_satisfied_spec(boolean, witness),
        r1cs_boolean_valid_spec(boolean) ==> r1cs_row_valid_spec(r1cs_boolean_row_spec(boolean)),
        (!r1cs_witness_valid_spec(witness) || witness.len != boolean.witness_len)
            ==> !r1cs_boolean_satisfied_spec(boolean, witness),
        witness.w0 != 1 ==> !r1cs_boolean_satisfied_spec(boolean, witness),
        (r1cs_boolean_valid_spec(boolean)
            && r1cs_witness_valid_spec(witness)
            && witness.len == boolean.witness_len
            && r1cs_boolean_one_wire_satisfied_spec(witness)
            && !r1cs_row_satisfied_spec(r1cs_boolean_row_spec(boolean), witness))
            ==> !r1cs_boolean_satisfied_spec(boolean, witness),
        r1cs_boolean_satisfied_spec(boolean, witness)
            ==> (r1cs_boolean_valid_spec(boolean)
                && r1cs_witness_valid_spec(witness)
                && witness.len == boolean.witness_len
                && r1cs_boolean_one_wire_satisfied_spec(witness)
                && r1cs_row_satisfied_spec(r1cs_boolean_row_spec(boolean), witness)),
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_WITNESS_LEN: u8 = 3;
    const VALID_MODULUS: u16 = 13;
    const BOOLEAN_INDEX_ONE: u8 = 1;
    const BOOLEAN_INDEX_TWO: u8 = 2;
    const MODULUS_MINUS_ONE: u16 = VALID_MODULUS - 1;
    const IRRELEVANT_WITNESS_VALUE: u16 = 7;
    const NON_BOOLEAN_VALUE: u16 = 2;
    const OVERLARGE_MODULUS: u16 = 17;
    const OVERLARGE_WITNESS_VALUE: u16 = 16;
    const MISMATCHED_WITNESS_LEN: u8 = 2;

    fn boolean_at_one() -> R1csBoolean {
        R1csBoolean {
            witness_len: VALID_WITNESS_LEN,
            modulus: VALID_MODULUS,
            index: BOOLEAN_INDEX_ONE,
        }
    }

    fn boolean_at_two() -> R1csBoolean {
        R1csBoolean {
            witness_len: VALID_WITNESS_LEN,
            modulus: VALID_MODULUS,
            index: BOOLEAN_INDEX_TWO,
        }
    }

    fn witness_slots(w1: u16, w2: u16) -> R1csWitness {
        R1csWitness {
            len: VALID_WITNESS_LEN,
            w0: 1,
            w1,
            w2,
        }
    }

    fn witness(value: u16) -> R1csWitness {
        witness_slots(value, IRRELEVANT_WITNESS_VALUE)
    }

    #[test]
    fn zero_is_boolean() {
        assert!(r1cs_boolean_satisfied(boolean_at_one(), witness(0)));
    }

    #[test]
    fn one_is_boolean() {
        assert!(r1cs_boolean_satisfied(boolean_at_one(), witness(1)));
    }

    #[test]
    fn two_is_not_boolean() {
        assert!(!r1cs_boolean_satisfied(boolean_at_one(), witness(NON_BOOLEAN_VALUE)));
    }

    #[test]
    fn boolean_row_is_valid_and_satisfied_for_one() {
        let boolean = boolean_at_one();
        let row = r1cs_boolean_row(boolean);
        assert_eq!(row.a1, 1);
        assert_eq!(row.b0, 1);
        assert_eq!(row.b1, MODULUS_MINUS_ONE);
        assert_eq!(row.c0, 0);
        assert!(r1cs_row_satisfied(row, witness(1)));
    }

    #[test]
    fn zero_index_is_rejected_because_it_is_the_one_wire() {
        let mut boolean = boolean_at_one();
        boolean.index = 0;
        assert!(!r1cs_boolean_valid(boolean));
        assert!(!r1cs_boolean_satisfied(boolean, witness(1)));
    }

    #[test]
    fn out_of_range_index_is_rejected() {
        let mut boolean = boolean_at_one();
        boolean.index = VALID_WITNESS_LEN;
        assert!(!r1cs_boolean_valid(boolean));
        assert!(!r1cs_boolean_satisfied(boolean, witness(1)));
    }

    #[test]
    fn overlarge_modulus_minus_one_coefficient_is_rejected() {
        let mut boolean = boolean_at_one();
        boolean.modulus = OVERLARGE_MODULUS;
        assert!(!r1cs_boolean_valid(boolean));
        assert!(!r1cs_boolean_satisfied(boolean, witness(1)));
    }

    #[test]
    fn missing_one_wire_is_rejected() {
        let mut bad = witness(1);
        bad.w0 = 2;
        assert!(!r1cs_boolean_satisfied(boolean_at_one(), bad));
    }

    #[test]
    fn malformed_witness_length_is_rejected() {
        let mut bad = witness(1);
        bad.len = MISMATCHED_WITNESS_LEN;
        assert!(!r1cs_boolean_satisfied(boolean_at_one(), bad));
    }

    #[test]
    fn row_at_second_boolean_slot_is_valid_and_satisfied() {
        let boolean = boolean_at_two();
        let row = r1cs_boolean_row(boolean);
        assert_eq!(row.a1, 0);
        assert_eq!(row.a2, 1);
        assert_eq!(row.b1, 0);
        assert_eq!(row.b2, MODULUS_MINUS_ONE);
        assert!(r1cs_row_satisfied(row, witness_slots(IRRELEVANT_WITNESS_VALUE, 1)));
    }

    #[test]
    fn malformed_witness_value_is_rejected() {
        let mut bad = witness(1);
        bad.w1 = OVERLARGE_WITNESS_VALUE;
        assert!(!r1cs_witness_valid(bad));
        assert!(!r1cs_boolean_satisfied(boolean_at_one(), bad));
    }

    #[test]
    fn valid_but_unsatisfied_encoded_row_is_rejected() {
        let boolean = boolean_at_one();
        let bad = witness(NON_BOOLEAN_VALUE);
        let row = r1cs_boolean_row(boolean);
        assert!(r1cs_boolean_valid(boolean));
        assert!(r1cs_witness_valid(bad));
        assert_eq!(bad.len, boolean.witness_len);
        assert!(r1cs_boolean_one_wire_satisfied(bad));
        assert!(!r1cs_row_satisfied(row, bad));
        assert!(!r1cs_boolean_satisfied(boolean, bad));
    }
}
