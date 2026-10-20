//! Finite single-row R1CS constraint primitives.
//!
//! This module models one bounded Rank-1 Constraint System row:
//! `<A,w> * <B,w> == <C,w> (mod p)`. It deliberately stops at the
//! admission boundary for one row and does not model multi-row systems,
//! proving systems, transcripts, elliptic curves, or cryptographic soundness.

use vstd::prelude::*;

verus! {
pub const R1CS_MAX_LEN: u8 = 3;
pub const R1CS_MAX_ELEMENT: u16 = 15;
pub const R1CS_MIN_MODULUS: u16 = 2;
pub const R1CS_MAX_MODULUS: u16 = 251;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct R1csRow {
    pub len: u8,
    pub modulus: u16,
    pub a0: u16,
    pub a1: u16,
    pub a2: u16,
    pub b0: u16,
    pub b1: u16,
    pub b2: u16,
    pub c0: u16,
    pub c1: u16,
    pub c2: u16,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct R1csWitness {
    pub len: u8,
    pub w0: u16,
    pub w1: u16,
    pub w2: u16,
}

// r[depends rust-primitives.r1cs-row-shape]
pub open spec fn r1cs_len_valid_spec(len: u8) -> bool {
    1 <= len && len <= R1CS_MAX_LEN
}

// r[depends rust-primitives.r1cs-row-shape]
pub open spec fn r1cs_element_valid_spec(x: u16) -> bool {
    x <= R1CS_MAX_ELEMENT
}

// r[depends rust-primitives.r1cs-row-shape]
pub open spec fn r1cs_modulus_valid_spec(modulus: u16) -> bool {
    R1CS_MIN_MODULUS <= modulus && modulus <= R1CS_MAX_MODULUS
}

// r[depends rust-primitives.r1cs-row-shape]
pub open spec fn r1cs_row_valid_spec(row: R1csRow) -> bool {
    r1cs_len_valid_spec(row.len)
        && r1cs_modulus_valid_spec(row.modulus)
        && r1cs_element_valid_spec(row.a0)
        && r1cs_element_valid_spec(row.a1)
        && r1cs_element_valid_spec(row.a2)
        && r1cs_element_valid_spec(row.b0)
        && r1cs_element_valid_spec(row.b1)
        && r1cs_element_valid_spec(row.b2)
        && r1cs_element_valid_spec(row.c0)
        && r1cs_element_valid_spec(row.c1)
        && r1cs_element_valid_spec(row.c2)
}

// r[depends rust-primitives.r1cs-row-shape]
pub open spec fn r1cs_witness_valid_spec(witness: R1csWitness) -> bool {
    r1cs_len_valid_spec(witness.len)
        && r1cs_element_valid_spec(witness.w0)
        && r1cs_element_valid_spec(witness.w1)
        && r1cs_element_valid_spec(witness.w2)
}

// r[depends rust-primitives.r1cs-dot-product]
pub open spec fn r1cs_dot_a_spec(row: R1csRow, witness: R1csWitness) -> int {
    ((row.a0 as int) * (witness.w0 as int)
        + if row.len >= 2 { (row.a1 as int) * (witness.w1 as int) } else { 0int }
        + if row.len >= 3 { (row.a2 as int) * (witness.w2 as int) } else { 0int })
        % (row.modulus as int)
}

// r[depends rust-primitives.r1cs-dot-product]
pub open spec fn r1cs_dot_b_spec(row: R1csRow, witness: R1csWitness) -> int {
    ((row.b0 as int) * (witness.w0 as int)
        + if row.len >= 2 { (row.b1 as int) * (witness.w1 as int) } else { 0int }
        + if row.len >= 3 { (row.b2 as int) * (witness.w2 as int) } else { 0int })
        % (row.modulus as int)
}

// r[depends rust-primitives.r1cs-dot-product]
pub open spec fn r1cs_dot_c_spec(row: R1csRow, witness: R1csWitness) -> int {
    ((row.c0 as int) * (witness.w0 as int)
        + if row.len >= 2 { (row.c1 as int) * (witness.w1 as int) } else { 0int }
        + if row.len >= 3 { (row.c2 as int) * (witness.w2 as int) } else { 0int })
        % (row.modulus as int)
}

// r[depends rust-primitives.r1cs-row-satisfaction]
pub open spec fn r1cs_row_satisfied_spec(row: R1csRow, witness: R1csWitness) -> bool {
    r1cs_row_valid_spec(row)
        && r1cs_witness_valid_spec(witness)
        && row.len == witness.len
        && ((r1cs_dot_a_spec(row, witness) * r1cs_dot_b_spec(row, witness)) % (row.modulus as int)
            == r1cs_dot_c_spec(row, witness))
}

// r[impl rust-primitives.r1cs-row-shape]
// r[impl rust-primitives.r1cs-verification-coverage]
pub fn r1cs_len_valid(len: u8) -> (result: bool)
    ensures result == r1cs_len_valid_spec(len)
{
    1 <= len && len <= R1CS_MAX_LEN
}

// r[impl rust-primitives.r1cs-row-shape]
pub fn r1cs_element_valid(x: u16) -> (result: bool)
    ensures result == r1cs_element_valid_spec(x)
{
    x <= R1CS_MAX_ELEMENT
}

// r[impl rust-primitives.r1cs-row-shape]
pub fn r1cs_modulus_valid(modulus: u16) -> (result: bool)
    ensures result == r1cs_modulus_valid_spec(modulus)
{
    R1CS_MIN_MODULUS <= modulus && modulus <= R1CS_MAX_MODULUS
}

// r[impl rust-primitives.r1cs-row-shape]
pub fn r1cs_row_valid(row: R1csRow) -> (result: bool)
    ensures result == r1cs_row_valid_spec(row)
{
    r1cs_len_valid(row.len)
        && r1cs_modulus_valid(row.modulus)
        && r1cs_element_valid(row.a0)
        && r1cs_element_valid(row.a1)
        && r1cs_element_valid(row.a2)
        && r1cs_element_valid(row.b0)
        && r1cs_element_valid(row.b1)
        && r1cs_element_valid(row.b2)
        && r1cs_element_valid(row.c0)
        && r1cs_element_valid(row.c1)
        && r1cs_element_valid(row.c2)
}

// r[impl rust-primitives.r1cs-row-shape]
pub fn r1cs_witness_valid(witness: R1csWitness) -> (result: bool)
    ensures result == r1cs_witness_valid_spec(witness)
{
    r1cs_len_valid(witness.len)
        && r1cs_element_valid(witness.w0)
        && r1cs_element_valid(witness.w1)
        && r1cs_element_valid(witness.w2)
}

// r[impl rust-primitives.r1cs-dot-product]
pub fn r1cs_dot_a(row: R1csRow, witness: R1csWitness) -> (result: u16)
    requires r1cs_row_valid_spec(row), r1cs_witness_valid_spec(witness), row.len == witness.len
    ensures result as int == r1cs_dot_a_spec(row, witness)
{
    assert((row.a0 as int) * (witness.w0 as int) <= 225int) by(nonlinear_arith)
        requires row.a0 <= R1CS_MAX_ELEMENT, witness.w0 <= R1CS_MAX_ELEMENT;
    let mut sum: u16 = row.a0 * witness.w0;
    if row.len >= 2 {
        assert((row.a1 as int) * (witness.w1 as int) <= 225int) by(nonlinear_arith)
            requires row.a1 <= R1CS_MAX_ELEMENT, witness.w1 <= R1CS_MAX_ELEMENT;
        assert(sum as int + (row.a1 as int) * (witness.w1 as int) <= 450int) by(nonlinear_arith)
            requires sum as int <= 225int, row.a1 <= R1CS_MAX_ELEMENT, witness.w1 <= R1CS_MAX_ELEMENT;
        sum = sum + row.a1 * witness.w1;
    }
    if row.len >= 3 {
        assert((row.a2 as int) * (witness.w2 as int) <= 225int) by(nonlinear_arith)
            requires row.a2 <= R1CS_MAX_ELEMENT, witness.w2 <= R1CS_MAX_ELEMENT;
        assert(sum as int + (row.a2 as int) * (witness.w2 as int) <= 675int) by(nonlinear_arith)
            requires sum as int <= 450int, row.a2 <= R1CS_MAX_ELEMENT, witness.w2 <= R1CS_MAX_ELEMENT;
        sum = sum + row.a2 * witness.w2;
    }
    sum % row.modulus
}

// r[impl rust-primitives.r1cs-dot-product]
pub fn r1cs_dot_b(row: R1csRow, witness: R1csWitness) -> (result: u16)
    requires r1cs_row_valid_spec(row), r1cs_witness_valid_spec(witness), row.len == witness.len
    ensures result as int == r1cs_dot_b_spec(row, witness)
{
    assert((row.b0 as int) * (witness.w0 as int) <= 225int) by(nonlinear_arith)
        requires row.b0 <= R1CS_MAX_ELEMENT, witness.w0 <= R1CS_MAX_ELEMENT;
    let mut sum: u16 = row.b0 * witness.w0;
    if row.len >= 2 {
        assert((row.b1 as int) * (witness.w1 as int) <= 225int) by(nonlinear_arith)
            requires row.b1 <= R1CS_MAX_ELEMENT, witness.w1 <= R1CS_MAX_ELEMENT;
        assert(sum as int + (row.b1 as int) * (witness.w1 as int) <= 450int) by(nonlinear_arith)
            requires sum as int <= 225int, row.b1 <= R1CS_MAX_ELEMENT, witness.w1 <= R1CS_MAX_ELEMENT;
        sum = sum + row.b1 * witness.w1;
    }
    if row.len >= 3 {
        assert((row.b2 as int) * (witness.w2 as int) <= 225int) by(nonlinear_arith)
            requires row.b2 <= R1CS_MAX_ELEMENT, witness.w2 <= R1CS_MAX_ELEMENT;
        assert(sum as int + (row.b2 as int) * (witness.w2 as int) <= 675int) by(nonlinear_arith)
            requires sum as int <= 450int, row.b2 <= R1CS_MAX_ELEMENT, witness.w2 <= R1CS_MAX_ELEMENT;
        sum = sum + row.b2 * witness.w2;
    }
    sum % row.modulus
}

// r[impl rust-primitives.r1cs-dot-product]
pub fn r1cs_dot_c(row: R1csRow, witness: R1csWitness) -> (result: u16)
    requires r1cs_row_valid_spec(row), r1cs_witness_valid_spec(witness), row.len == witness.len
    ensures result as int == r1cs_dot_c_spec(row, witness)
{
    assert((row.c0 as int) * (witness.w0 as int) <= 225int) by(nonlinear_arith)
        requires row.c0 <= R1CS_MAX_ELEMENT, witness.w0 <= R1CS_MAX_ELEMENT;
    let mut sum: u16 = row.c0 * witness.w0;
    if row.len >= 2 {
        assert((row.c1 as int) * (witness.w1 as int) <= 225int) by(nonlinear_arith)
            requires row.c1 <= R1CS_MAX_ELEMENT, witness.w1 <= R1CS_MAX_ELEMENT;
        assert(sum as int + (row.c1 as int) * (witness.w1 as int) <= 450int) by(nonlinear_arith)
            requires sum as int <= 225int, row.c1 <= R1CS_MAX_ELEMENT, witness.w1 <= R1CS_MAX_ELEMENT;
        sum = sum + row.c1 * witness.w1;
    }
    if row.len >= 3 {
        assert((row.c2 as int) * (witness.w2 as int) <= 225int) by(nonlinear_arith)
            requires row.c2 <= R1CS_MAX_ELEMENT, witness.w2 <= R1CS_MAX_ELEMENT;
        assert(sum as int + (row.c2 as int) * (witness.w2 as int) <= 675int) by(nonlinear_arith)
            requires sum as int <= 450int, row.c2 <= R1CS_MAX_ELEMENT, witness.w2 <= R1CS_MAX_ELEMENT;
        sum = sum + row.c2 * witness.w2;
    }
    sum % row.modulus
}

// r[impl rust-primitives.r1cs-row-satisfaction]
pub fn r1cs_row_satisfied(row: R1csRow, witness: R1csWitness) -> (result: bool)
    ensures result == r1cs_row_satisfied_spec(row, witness)
{
    if !(r1cs_row_valid(row) && r1cs_witness_valid(witness) && row.len == witness.len) {
        return false;
    }
    let a = r1cs_dot_a(row, witness);
    let b = r1cs_dot_b(row, witness);
    let c = r1cs_dot_c(row, witness);
    assert(a < row.modulus);
    assert(b < row.modulus);
    assert(row.modulus <= R1CS_MAX_MODULUS);
    assert((a as int) * (b as int) <= 62500int) by(nonlinear_arith)
        requires a < row.modulus, b < row.modulus, row.modulus <= R1CS_MAX_MODULUS;
    let product: u16 = a * b;
    let lhs = product % row.modulus;
    lhs == c
}

// r[verify rust-primitives.r1cs-row-shape]
pub proof fn invalid_lengths_rejected(row: R1csRow, witness: R1csWitness)
    requires row.len == 0 || row.len > R1CS_MAX_LEN || witness.len == 0 || witness.len > R1CS_MAX_LEN
    ensures !r1cs_row_valid_spec(row) || !r1cs_witness_valid_spec(witness)
{}

// r[verify rust-primitives.r1cs-row-shape]
pub proof fn length_mismatch_rejected(row: R1csRow, witness: R1csWitness)
    requires row.len != witness.len
    ensures !r1cs_row_satisfied_spec(row, witness)
{}

// r[verify rust-primitives.r1cs-dot-product]
pub proof fn dot_product_coverage_catalogued(row: R1csRow, witness: R1csWitness)
    ensures
        r1cs_dot_a_spec(row, witness)
            == ((row.a0 as int) * (witness.w0 as int)
                + if row.len >= 2 { (row.a1 as int) * (witness.w1 as int) } else { 0int }
                + if row.len >= 3 { (row.a2 as int) * (witness.w2 as int) } else { 0int })
                % (row.modulus as int),
        r1cs_dot_b_spec(row, witness)
            == ((row.b0 as int) * (witness.w0 as int)
                + if row.len >= 2 { (row.b1 as int) * (witness.w1 as int) } else { 0int }
                + if row.len >= 3 { (row.b2 as int) * (witness.w2 as int) } else { 0int })
                % (row.modulus as int),
        r1cs_dot_c_spec(row, witness)
            == ((row.c0 as int) * (witness.w0 as int)
                + if row.len >= 2 { (row.c1 as int) * (witness.w1 as int) } else { 0int }
                + if row.len >= 3 { (row.c2 as int) * (witness.w2 as int) } else { 0int })
                % (row.modulus as int),
{}

// r[verify rust-primitives.r1cs-row-satisfaction]
pub proof fn malformed_row_rejected(row: R1csRow, witness: R1csWitness)
    requires !r1cs_row_valid_spec(row)
    ensures !r1cs_row_satisfied_spec(row, witness)
{}

// r[verify rust-primitives.r1cs-row-satisfaction]
pub proof fn malformed_witness_rejected(row: R1csRow, witness: R1csWitness)
    requires !r1cs_witness_valid_spec(witness)
    ensures !r1cs_row_satisfied_spec(row, witness)
{}

// r[verify rust-primitives.r1cs-verification-coverage]
pub proof fn r1cs_primitive_coverage_catalogued(row: R1csRow, witness: R1csWitness)
    ensures
        (row.len == 0 || row.len > R1CS_MAX_LEN || witness.len == 0 || witness.len > R1CS_MAX_LEN)
            ==> (!r1cs_row_valid_spec(row) || !r1cs_witness_valid_spec(witness)),
        row.len != witness.len ==> !r1cs_row_satisfied_spec(row, witness),
        !r1cs_row_valid_spec(row) ==> !r1cs_row_satisfied_spec(row, witness),
        !r1cs_witness_valid_spec(witness) ==> !r1cs_row_satisfied_spec(row, witness),
        r1cs_row_satisfied_spec(row, witness)
            ==> (r1cs_row_valid_spec(row)
                && r1cs_witness_valid_spec(witness)
                && row.len == witness.len
                && ((r1cs_dot_a_spec(row, witness) * r1cs_dot_b_spec(row, witness)) % (row.modulus as int)
                    == r1cs_dot_c_spec(row, witness))),
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    const INVALID_ZERO_LEN: u8 = 0;
    const OVERLARGE_ELEMENT: u16 = 16;

    fn satisfied_row() -> (R1csRow, R1csWitness) {
        (
            R1csRow {
                len: 2,
                modulus: 17,
                a0: 1,
                a1: 2,
                a2: 15,
                b0: 3,
                b1: 1,
                b2: 15,
                c0: 1,
                c1: 1,
                c2: 15,
            },
            R1csWitness {
                len: 2,
                w0: 3,
                w1: 4,
                w2: 15,
            },
        )
    }

    #[test]
    fn valid_row_and_witness_shapes_are_admitted() {
        let (row, witness) = satisfied_row();
        assert!(r1cs_row_valid(row));
        assert!(r1cs_witness_valid(witness));
    }

    #[test]
    fn satisfied_row_is_admitted() {
        let (row, witness) = satisfied_row();
        assert_eq!(r1cs_dot_a(row, witness), 11);
        assert_eq!(r1cs_dot_b(row, witness), 13);
        assert_eq!(r1cs_dot_c(row, witness), 7);
        assert!(r1cs_row_satisfied(row, witness));
    }

    #[test]
    fn unsatisfied_row_is_rejected() {
        let (mut row, witness) = satisfied_row();
        row.c1 = 3;
        assert!(r1cs_row_valid(row));
        assert!(!r1cs_row_satisfied(row, witness));
    }

    #[test]
    fn length_mismatch_is_rejected() {
        let (row, mut witness) = satisfied_row();
        witness.len = 3;
        assert!(r1cs_row_valid(row));
        assert!(r1cs_witness_valid(witness));
        assert!(!r1cs_row_satisfied(row, witness));
    }

    #[test]
    fn malformed_shape_is_rejected() {
        let (mut row, witness) = satisfied_row();
        row.modulus = 1;
        assert!(!r1cs_row_valid(row));
        assert!(!r1cs_row_satisfied(row, witness));
    }

    #[test]
    fn zero_row_length_is_rejected() {
        let (mut row, witness) = satisfied_row();
        row.len = INVALID_ZERO_LEN;
        assert!(!r1cs_len_valid(row.len));
        assert!(!r1cs_row_valid(row));
        assert!(!r1cs_row_satisfied(row, witness));
    }

    #[test]
    fn malformed_witness_value_is_rejected() {
        let (row, mut witness) = satisfied_row();
        witness.w0 = OVERLARGE_ELEMENT;
        assert!(!r1cs_witness_valid(witness));
        assert!(!r1cs_row_satisfied(row, witness));
    }

    #[test]
    fn inactive_slots_do_not_affect_short_rows() {
        let (row, witness) = satisfied_row();
        let mut changed = row;
        changed.a2 = 0;
        changed.b2 = 0;
        changed.c2 = 0;
        assert_eq!(r1cs_dot_a(row, witness), r1cs_dot_a(changed, witness));
        assert_eq!(r1cs_dot_b(row, witness), r1cs_dot_b(changed, witness));
        assert_eq!(r1cs_dot_c(row, witness), r1cs_dot_c(changed, witness));
    }
}
