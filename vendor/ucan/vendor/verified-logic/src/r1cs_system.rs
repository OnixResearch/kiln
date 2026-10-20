//! Finite multi-row R1CS constraint-system primitives.
//!
//! This module composes `r1cs_constraint` rows into one bounded system checked
//! against a single witness and one shared modulus. It deliberately stops at
//! deterministic constraint-system admission and does not model proving systems,
//! transcripts, commitments, or cryptographic soundness.

use vstd::prelude::*;

use crate::r1cs_constraint::R1csRow;
use crate::r1cs_constraint::R1csWitness;
use crate::r1cs_constraint::r1cs_modulus_valid;
#[cfg(verus_keep_ghost)]
use crate::r1cs_constraint::r1cs_modulus_valid_spec;
use crate::r1cs_constraint::r1cs_row_satisfied;
#[cfg(verus_keep_ghost)]
use crate::r1cs_constraint::r1cs_row_satisfied_spec;
use crate::r1cs_constraint::r1cs_row_valid;
#[cfg(verus_keep_ghost)]
use crate::r1cs_constraint::r1cs_row_valid_spec;
use crate::r1cs_constraint::r1cs_witness_valid;
#[cfg(verus_keep_ghost)]
use crate::r1cs_constraint::r1cs_witness_valid_spec;

verus! {
pub const R1CS_SYSTEM_MAX_ROWS: u8 = 3;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct R1csSystem {
    pub len: u8,
    pub modulus: u16,
    pub row0: R1csRow,
    pub row1: R1csRow,
    pub row2: R1csRow,
}

// r[depends rust-primitives.r1cs-system-shape]
pub open spec fn r1cs_system_len_valid_spec(len: u8) -> bool {
    1 <= len && len <= R1CS_SYSTEM_MAX_ROWS
}

// r[depends rust-primitives.r1cs-system-shape]
pub open spec fn r1cs_row_uses_system_modulus_spec(row: R1csRow, modulus: u16) -> bool {
    r1cs_row_valid_spec(row) && row.modulus == modulus
}

// r[depends rust-primitives.r1cs-system-shape]
pub open spec fn r1cs_system_valid_spec(system: R1csSystem) -> bool {
    r1cs_system_len_valid_spec(system.len)
        && r1cs_modulus_valid_spec(system.modulus)
        && r1cs_row_uses_system_modulus_spec(system.row0, system.modulus)
        && (system.len < 2 || r1cs_row_uses_system_modulus_spec(system.row1, system.modulus))
        && (system.len < 3 || r1cs_row_uses_system_modulus_spec(system.row2, system.modulus))
}

// r[depends rust-primitives.r1cs-system-satisfaction]
pub open spec fn r1cs_system_satisfied_spec(system: R1csSystem, witness: R1csWitness) -> bool {
    r1cs_system_valid_spec(system)
        && r1cs_witness_valid_spec(witness)
        && r1cs_row_satisfied_spec(system.row0, witness)
        && (system.len < 2 || r1cs_row_satisfied_spec(system.row1, witness))
        && (system.len < 3 || r1cs_row_satisfied_spec(system.row2, witness))
}

// r[impl rust-primitives.r1cs-system-shape]
// r[impl rust-primitives.r1cs-system-verification-coverage]
pub fn r1cs_system_len_valid(len: u8) -> (result: bool)
    ensures result == r1cs_system_len_valid_spec(len)
{
    1 <= len && len <= R1CS_SYSTEM_MAX_ROWS
}

// r[impl rust-primitives.r1cs-system-shape]
pub fn r1cs_row_uses_system_modulus(row: R1csRow, modulus: u16) -> (result: bool)
    ensures result == r1cs_row_uses_system_modulus_spec(row, modulus)
{
    r1cs_row_valid(row) && row.modulus == modulus
}

// r[impl rust-primitives.r1cs-system-shape]
pub fn r1cs_system_valid(system: R1csSystem) -> (result: bool)
    ensures result == r1cs_system_valid_spec(system)
{
    if !r1cs_system_len_valid(system.len) {
        return false;
    }
    if !r1cs_modulus_valid(system.modulus) {
        return false;
    }
    if !r1cs_row_uses_system_modulus(system.row0, system.modulus) {
        return false;
    }
    if system.len >= 2 && !r1cs_row_uses_system_modulus(system.row1, system.modulus) {
        return false;
    }
    if system.len >= 3 && !r1cs_row_uses_system_modulus(system.row2, system.modulus) {
        return false;
    }
    true
}

// r[impl rust-primitives.r1cs-system-satisfaction]
pub fn r1cs_system_satisfied(system: R1csSystem, witness: R1csWitness) -> (result: bool)
    ensures result == r1cs_system_satisfied_spec(system, witness)
{
    if !r1cs_system_valid(system) {
        return false;
    }
    if !r1cs_witness_valid(witness) {
        return false;
    }
    if !r1cs_row_satisfied(system.row0, witness) {
        return false;
    }
    if system.len >= 2 && !r1cs_row_satisfied(system.row1, witness) {
        return false;
    }
    if system.len >= 3 && !r1cs_row_satisfied(system.row2, witness) {
        return false;
    }
    true
}

// r[verify rust-primitives.r1cs-system-shape]
pub proof fn invalid_system_lengths_rejected(system: R1csSystem)
    requires system.len == 0 || system.len > R1CS_SYSTEM_MAX_ROWS
    ensures !r1cs_system_valid_spec(system)
{}

// r[verify rust-primitives.r1cs-system-shape]
pub proof fn active_modulus_mismatch_rejected(system: R1csSystem)
    requires
        r1cs_system_len_valid_spec(system.len),
        r1cs_modulus_valid_spec(system.modulus),
        (system.row0.modulus != system.modulus)
            || (system.len >= 2 && system.row1.modulus != system.modulus)
            || (system.len >= 3 && system.row2.modulus != system.modulus),
    ensures !r1cs_system_valid_spec(system)
{}

// r[verify rust-primitives.r1cs-system-satisfaction]
pub proof fn malformed_system_rejected(system: R1csSystem, witness: R1csWitness)
    requires !r1cs_system_valid_spec(system)
    ensures !r1cs_system_satisfied_spec(system, witness)
{}

// r[verify rust-primitives.r1cs-system-satisfaction]
pub proof fn malformed_system_witness_rejected(system: R1csSystem, witness: R1csWitness)
    requires !r1cs_witness_valid_spec(witness)
    ensures !r1cs_system_satisfied_spec(system, witness)
{}

// r[verify rust-primitives.r1cs-system-satisfaction]
pub proof fn failed_active_row_rejects_system(system: R1csSystem, witness: R1csWitness)
    requires
        r1cs_system_valid_spec(system),
        r1cs_witness_valid_spec(witness),
        !r1cs_row_satisfied_spec(system.row0, witness)
            || (system.len >= 2 && !r1cs_row_satisfied_spec(system.row1, witness))
            || (system.len >= 3 && !r1cs_row_satisfied_spec(system.row2, witness)),
    ensures !r1cs_system_satisfied_spec(system, witness)
{}

// r[verify rust-primitives.r1cs-system-verification-coverage]
pub proof fn r1cs_system_primitive_coverage_catalogued(system: R1csSystem, witness: R1csWitness)
    ensures
        (system.len == 0 || system.len > R1CS_SYSTEM_MAX_ROWS) ==> !r1cs_system_valid_spec(system),
        (r1cs_system_len_valid_spec(system.len)
            && r1cs_modulus_valid_spec(system.modulus)
            && ((system.row0.modulus != system.modulus)
                || (system.len >= 2 && system.row1.modulus != system.modulus)
                || (system.len >= 3 && system.row2.modulus != system.modulus)))
            ==> !r1cs_system_valid_spec(system),
        !r1cs_system_valid_spec(system) ==> !r1cs_system_satisfied_spec(system, witness),
        !r1cs_witness_valid_spec(witness) ==> !r1cs_system_satisfied_spec(system, witness),
        (r1cs_system_valid_spec(system)
            && r1cs_witness_valid_spec(witness)
            && (!r1cs_row_satisfied_spec(system.row0, witness)
                || (system.len >= 2 && !r1cs_row_satisfied_spec(system.row1, witness))
                || (system.len >= 3 && !r1cs_row_satisfied_spec(system.row2, witness))))
            ==> !r1cs_system_satisfied_spec(system, witness),
        r1cs_system_satisfied_spec(system, witness)
            ==> (r1cs_system_valid_spec(system)
                && r1cs_witness_valid_spec(witness)
                && r1cs_row_satisfied_spec(system.row0, witness)
                && (system.len < 2 || r1cs_row_satisfied_spec(system.row1, witness))
                && (system.len < 3 || r1cs_row_satisfied_spec(system.row2, witness))),
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    const TWO_ROW_SYSTEM_LEN: u8 = 2;
    const THREE_ROW_SYSTEM_LEN: u8 = 3;
    const INVALID_ZERO_SYSTEM_LEN: u8 = 0;
    const ACTIVE_WITNESS_LEN: u8 = 2;
    const SYSTEM_MODULUS: u16 = 17;
    const MISMATCHED_ACTIVE_MODULUS: u16 = 19;
    const MALFORMED_ROW_LEN: u8 = 0;
    const MALFORMED_ROW_MODULUS: u16 = 1;
    const MALFORMED_FIELD_VALUE: u16 = 99;
    const UNUSED_FIELD_VALUE: u16 = 15;
    const WITNESS_ZERO: u16 = 3;
    const WITNESS_ONE: u16 = 4;
    const FIRST_ROW_A0: u16 = 1;
    const FIRST_ROW_A1: u16 = 2;
    const FIRST_ROW_B0: u16 = 3;
    const FIRST_ROW_B1: u16 = 1;
    const FIRST_ROW_C0: u16 = 1;
    const FIRST_ROW_C1: u16 = 1;
    const SECOND_ROW_A0: u16 = 1;
    const SECOND_ROW_B1: u16 = 1;
    const SECOND_ROW_C0: u16 = 4;
    const BAD_ROW_OUTPUT: u16 = 5;
    const MISMATCHED_WITNESS_LEN: u8 = 3;

    fn witness() -> R1csWitness {
        R1csWitness {
            len: ACTIVE_WITNESS_LEN,
            w0: WITNESS_ZERO,
            w1: WITNESS_ONE,
            w2: UNUSED_FIELD_VALUE,
        }
    }

    fn row_product_11_times_13() -> R1csRow {
        R1csRow {
            len: ACTIVE_WITNESS_LEN,
            modulus: SYSTEM_MODULUS,
            a0: FIRST_ROW_A0,
            a1: FIRST_ROW_A1,
            a2: UNUSED_FIELD_VALUE,
            b0: FIRST_ROW_B0,
            b1: FIRST_ROW_B1,
            b2: UNUSED_FIELD_VALUE,
            c0: FIRST_ROW_C0,
            c1: FIRST_ROW_C1,
            c2: UNUSED_FIELD_VALUE,
        }
    }

    fn row_product_3_times_4() -> R1csRow {
        R1csRow {
            len: ACTIVE_WITNESS_LEN,
            modulus: SYSTEM_MODULUS,
            a0: SECOND_ROW_A0,
            a1: 0,
            a2: UNUSED_FIELD_VALUE,
            b0: 0,
            b1: SECOND_ROW_B1,
            b2: UNUSED_FIELD_VALUE,
            c0: SECOND_ROW_C0,
            c1: 0,
            c2: UNUSED_FIELD_VALUE,
        }
    }

    fn malformed_inactive_row() -> R1csRow {
        R1csRow {
            len: MALFORMED_ROW_LEN,
            modulus: MALFORMED_ROW_MODULUS,
            a0: MALFORMED_FIELD_VALUE,
            a1: MALFORMED_FIELD_VALUE,
            a2: MALFORMED_FIELD_VALUE,
            b0: MALFORMED_FIELD_VALUE,
            b1: MALFORMED_FIELD_VALUE,
            b2: MALFORMED_FIELD_VALUE,
            c0: MALFORMED_FIELD_VALUE,
            c1: MALFORMED_FIELD_VALUE,
            c2: MALFORMED_FIELD_VALUE,
        }
    }

    fn satisfied_system() -> R1csSystem {
        R1csSystem {
            len: TWO_ROW_SYSTEM_LEN,
            modulus: SYSTEM_MODULUS,
            row0: row_product_11_times_13(),
            row1: row_product_3_times_4(),
            row2: malformed_inactive_row(),
        }
    }

    fn three_row_satisfied_system() -> R1csSystem {
        R1csSystem {
            len: THREE_ROW_SYSTEM_LEN,
            modulus: SYSTEM_MODULUS,
            row0: row_product_11_times_13(),
            row1: row_product_3_times_4(),
            row2: row_product_3_times_4(),
        }
    }

    #[test]
    fn valid_two_row_system_is_admitted() {
        let system = satisfied_system();
        assert!(r1cs_system_valid(system));
    }

    #[test]
    fn all_active_rows_satisfied_admits_system() {
        let system = satisfied_system();
        assert!(r1cs_system_satisfied(system, witness()));
    }

    #[test]
    fn one_failed_active_row_rejects_system() {
        let mut system = satisfied_system();
        system.row1.c0 = BAD_ROW_OUTPUT;
        assert!(r1cs_system_valid(system));
        assert!(!r1cs_system_satisfied(system, witness()));
    }

    #[test]
    fn active_modulus_mismatch_rejects_system_shape() {
        let mut system = satisfied_system();
        system.row1.modulus = MISMATCHED_ACTIVE_MODULUS;
        assert!(!r1cs_system_valid(system));
        assert!(!r1cs_system_satisfied(system, witness()));
    }

    #[test]
    fn bad_witness_length_rejects_satisfaction() {
        let system = satisfied_system();
        let mut bad_witness = witness();
        bad_witness.len = MISMATCHED_WITNESS_LEN;
        assert!(r1cs_system_valid(system));
        assert!(r1cs_witness_valid(bad_witness));
        assert!(!r1cs_system_satisfied(system, bad_witness));
    }

    #[test]
    fn inactive_malformed_row_is_ignored() {
        let system = satisfied_system();
        assert!(!r1cs_row_valid(system.row2));
        assert!(r1cs_system_valid(system));
        assert!(r1cs_system_satisfied(system, witness()));
    }

    #[test]
    fn invalid_zero_system_length_is_rejected() {
        let mut system = satisfied_system();
        system.len = INVALID_ZERO_SYSTEM_LEN;
        assert!(!r1cs_system_valid(system));
        assert!(!r1cs_system_satisfied(system, witness()));
    }

    #[test]
    fn malformed_active_row_rejects_system_shape() {
        let mut system = satisfied_system();
        system.row0 = malformed_inactive_row();
        assert!(!r1cs_system_valid(system));
        assert!(!r1cs_system_satisfied(system, witness()));
    }

    #[test]
    fn malformed_witness_value_rejects_satisfaction() {
        let system = satisfied_system();
        let mut bad_witness = witness();
        bad_witness.w1 = MALFORMED_FIELD_VALUE;
        assert!(r1cs_system_valid(system));
        assert!(!r1cs_witness_valid(bad_witness));
        assert!(!r1cs_system_satisfied(system, bad_witness));
    }

    #[test]
    fn all_three_active_rows_satisfied_admits_system() {
        let system = three_row_satisfied_system();
        assert!(r1cs_system_valid(system));
        assert!(r1cs_system_satisfied(system, witness()));
    }

    #[test]
    fn failed_third_active_row_rejects_system() {
        let mut system = three_row_satisfied_system();
        system.row2.c0 = BAD_ROW_OUTPUT;
        assert!(r1cs_system_valid(system));
        assert!(!r1cs_row_satisfied(system.row2, witness()));
        assert!(!r1cs_system_satisfied(system, witness()));
    }
}
