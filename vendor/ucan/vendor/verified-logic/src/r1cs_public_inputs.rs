//! Finite R1CS public-input binding primitives.
//!
//! This module binds a bounded list of public values to witness indices and
//! composes those bindings with `r1cs_system` satisfaction. It models statement
//! admission only: no commitments, transcripts, proof objects, verifier keys, or
//! cryptographic soundness are claimed here.

use vstd::prelude::*;

use crate::r1cs_constraint::R1csRow;
use crate::r1cs_constraint::R1csWitness;
use crate::r1cs_constraint::r1cs_element_valid;
#[cfg(verus_keep_ghost)]
use crate::r1cs_constraint::r1cs_element_valid_spec;
use crate::r1cs_constraint::r1cs_modulus_valid;
#[cfg(verus_keep_ghost)]
use crate::r1cs_constraint::r1cs_modulus_valid_spec;
use crate::r1cs_constraint::r1cs_witness_valid;
#[cfg(verus_keep_ghost)]
use crate::r1cs_constraint::r1cs_witness_valid_spec;
use crate::r1cs_system::R1csSystem;
use crate::r1cs_system::r1cs_system_satisfied;
#[cfg(verus_keep_ghost)]
use crate::r1cs_system::r1cs_system_satisfied_spec;

verus! {
pub const R1CS_PUBLIC_INPUT_MAX_LEN: u8 = 3;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct R1csPublicInputs {
    pub len: u8,
    pub index0: u8,
    pub index1: u8,
    pub index2: u8,
    pub value0: u16,
    pub value1: u16,
    pub value2: u16,
}

// r[depends rust-primitives.r1cs-public-input-binding-shape]
pub open spec fn r1cs_public_inputs_len_valid_spec(len: u8) -> bool {
    len <= R1CS_PUBLIC_INPUT_MAX_LEN
}

// r[depends rust-primitives.r1cs-public-input-binding-shape]
pub open spec fn r1cs_witness_value_at_spec(witness: R1csWitness, index: u8) -> u16 {
    if index == 0 { witness.w0 } else if index == 1 { witness.w1 } else { witness.w2 }
}

// r[depends rust-primitives.r1cs-public-input-binding-shape]
pub open spec fn r1cs_public_binding_slot_satisfied_spec(
    index: u8,
    value: u16,
    witness: R1csWitness,
    modulus: u16,
) -> bool {
    index < witness.len
        && r1cs_element_valid_spec(value)
        && (value as int) % (modulus as int)
            == (r1cs_witness_value_at_spec(witness, index) as int) % (modulus as int)
}

// r[depends rust-primitives.r1cs-public-input-binding-shape]
pub open spec fn r1cs_public_inputs_satisfied_spec(
    public_inputs: R1csPublicInputs,
    witness: R1csWitness,
    modulus: u16,
) -> bool {
    r1cs_public_inputs_len_valid_spec(public_inputs.len)
        && r1cs_modulus_valid_spec(modulus)
        && r1cs_witness_valid_spec(witness)
        && (public_inputs.len < 1 || r1cs_public_binding_slot_satisfied_spec(public_inputs.index0, public_inputs.value0, witness, modulus))
        && (public_inputs.len < 2 || r1cs_public_binding_slot_satisfied_spec(public_inputs.index1, public_inputs.value1, witness, modulus))
        && (public_inputs.len < 3 || r1cs_public_binding_slot_satisfied_spec(public_inputs.index2, public_inputs.value2, witness, modulus))
}

// r[depends rust-primitives.r1cs-public-statement-satisfaction]
pub open spec fn r1cs_public_statement_satisfied_spec(
    system: R1csSystem,
    witness: R1csWitness,
    public_inputs: R1csPublicInputs,
) -> bool {
    r1cs_system_satisfied_spec(system, witness)
        && r1cs_public_inputs_satisfied_spec(public_inputs, witness, system.modulus)
}

// r[impl rust-primitives.r1cs-public-input-binding-shape]
// r[impl rust-primitives.r1cs-public-input-verification-coverage]
pub fn r1cs_public_inputs_len_valid(len: u8) -> (result: bool)
    ensures result == r1cs_public_inputs_len_valid_spec(len)
{
    len <= R1CS_PUBLIC_INPUT_MAX_LEN
}

// r[impl rust-primitives.r1cs-public-input-binding-shape]
pub fn r1cs_witness_value_at(witness: R1csWitness, index: u8) -> (result: u16)
    ensures result == r1cs_witness_value_at_spec(witness, index)
{
    if index == 0 {
        witness.w0
    } else if index == 1 {
        witness.w1
    } else {
        witness.w2
    }
}

// r[impl rust-primitives.r1cs-public-input-binding-shape]
pub fn r1cs_public_binding_slot_satisfied(
    index: u8,
    value: u16,
    witness: R1csWitness,
    modulus: u16,
) -> (result: bool)
    requires r1cs_modulus_valid_spec(modulus)
    ensures result == r1cs_public_binding_slot_satisfied_spec(index, value, witness, modulus)
{
    if !(index < witness.len && r1cs_element_valid(value)) {
        return false;
    }
    let witness_value = r1cs_witness_value_at(witness, index);
    value % modulus == witness_value % modulus
}

// r[impl rust-primitives.r1cs-public-input-binding-shape]
pub fn r1cs_public_inputs_satisfied(
    public_inputs: R1csPublicInputs,
    witness: R1csWitness,
    modulus: u16,
) -> (result: bool)
    ensures result == r1cs_public_inputs_satisfied_spec(public_inputs, witness, modulus)
{
    if !r1cs_public_inputs_len_valid(public_inputs.len) {
        return false;
    }
    if !r1cs_modulus_valid(modulus) {
        return false;
    }
    if !r1cs_witness_valid(witness) {
        return false;
    }
    if public_inputs.len >= 1
        && !r1cs_public_binding_slot_satisfied(public_inputs.index0, public_inputs.value0, witness, modulus)
    {
        return false;
    }
    if public_inputs.len >= 2
        && !r1cs_public_binding_slot_satisfied(public_inputs.index1, public_inputs.value1, witness, modulus)
    {
        return false;
    }
    if public_inputs.len >= 3
        && !r1cs_public_binding_slot_satisfied(public_inputs.index2, public_inputs.value2, witness, modulus)
    {
        return false;
    }
    true
}

// r[impl rust-primitives.r1cs-public-statement-satisfaction]
pub fn r1cs_public_statement_satisfied(
    system: R1csSystem,
    witness: R1csWitness,
    public_inputs: R1csPublicInputs,
) -> (result: bool)
    ensures result == r1cs_public_statement_satisfied_spec(system, witness, public_inputs)
{
    if !r1cs_system_satisfied(system, witness) {
        return false;
    }
    r1cs_public_inputs_satisfied(public_inputs, witness, system.modulus)
}

// r[verify rust-primitives.r1cs-public-input-binding-shape]
pub proof fn invalid_public_input_lengths_rejected(
    public_inputs: R1csPublicInputs,
    witness: R1csWitness,
    modulus: u16,
)
    requires public_inputs.len > R1CS_PUBLIC_INPUT_MAX_LEN
    ensures !r1cs_public_inputs_satisfied_spec(public_inputs, witness, modulus)
{}

// r[verify rust-primitives.r1cs-public-input-binding-shape]
pub proof fn out_of_range_public_index_rejected(
    public_inputs: R1csPublicInputs,
    witness: R1csWitness,
    modulus: u16,
)
    requires
        r1cs_modulus_valid_spec(modulus),
        r1cs_witness_valid_spec(witness),
        (public_inputs.len >= 1 && public_inputs.index0 >= witness.len)
            || (public_inputs.len >= 2 && public_inputs.index1 >= witness.len)
            || (public_inputs.len >= 3 && public_inputs.index2 >= witness.len),
    ensures !r1cs_public_inputs_satisfied_spec(public_inputs, witness, modulus)
{}

// r[verify rust-primitives.r1cs-public-input-binding-shape]
pub proof fn mismatched_public_value_rejected(
    public_inputs: R1csPublicInputs,
    witness: R1csWitness,
    modulus: u16,
)
    requires
        r1cs_modulus_valid_spec(modulus),
        r1cs_witness_valid_spec(witness),
        r1cs_public_inputs_len_valid_spec(public_inputs.len),
        (public_inputs.len >= 1
            && public_inputs.index0 < witness.len
            && r1cs_element_valid_spec(public_inputs.value0)
            && (public_inputs.value0 as int) % (modulus as int)
                != (r1cs_witness_value_at_spec(witness, public_inputs.index0) as int) % (modulus as int))
            || (public_inputs.len >= 2
                && public_inputs.index1 < witness.len
                && r1cs_element_valid_spec(public_inputs.value1)
                && (public_inputs.value1 as int) % (modulus as int)
                    != (r1cs_witness_value_at_spec(witness, public_inputs.index1) as int) % (modulus as int))
            || (public_inputs.len >= 3
                && public_inputs.index2 < witness.len
                && r1cs_element_valid_spec(public_inputs.value2)
                && (public_inputs.value2 as int) % (modulus as int)
                    != (r1cs_witness_value_at_spec(witness, public_inputs.index2) as int) % (modulus as int)),
    ensures !r1cs_public_inputs_satisfied_spec(public_inputs, witness, modulus)
{}

// r[verify rust-primitives.r1cs-public-statement-satisfaction]
pub proof fn malformed_statement_system_rejected(
    system: R1csSystem,
    witness: R1csWitness,
    public_inputs: R1csPublicInputs,
)
    requires !r1cs_system_satisfied_spec(system, witness)
    ensures !r1cs_public_statement_satisfied_spec(system, witness, public_inputs)
{}

// r[verify rust-primitives.r1cs-public-statement-satisfaction]
pub proof fn malformed_statement_public_inputs_rejected(
    system: R1csSystem,
    witness: R1csWitness,
    public_inputs: R1csPublicInputs,
)
    requires !r1cs_public_inputs_satisfied_spec(public_inputs, witness, system.modulus)
    ensures !r1cs_public_statement_satisfied_spec(system, witness, public_inputs)
{}

// r[verify rust-primitives.r1cs-public-input-verification-coverage]
pub proof fn r1cs_public_input_primitive_coverage_catalogued(
    public_inputs: R1csPublicInputs,
    witness: R1csWitness,
    modulus: u16,
    system: R1csSystem,
)
    ensures
        public_inputs.len > R1CS_PUBLIC_INPUT_MAX_LEN
            ==> !r1cs_public_inputs_satisfied_spec(public_inputs, witness, modulus),
        !r1cs_modulus_valid_spec(modulus)
            ==> !r1cs_public_inputs_satisfied_spec(public_inputs, witness, modulus),
        !r1cs_witness_valid_spec(witness)
            ==> !r1cs_public_inputs_satisfied_spec(public_inputs, witness, modulus),
        (r1cs_modulus_valid_spec(modulus)
            && r1cs_witness_valid_spec(witness)
            && r1cs_public_inputs_len_valid_spec(public_inputs.len)
            && ((public_inputs.len >= 1 && !r1cs_element_valid_spec(public_inputs.value0))
                || (public_inputs.len >= 2 && !r1cs_element_valid_spec(public_inputs.value1))
                || (public_inputs.len >= 3 && !r1cs_element_valid_spec(public_inputs.value2))))
            ==> !r1cs_public_inputs_satisfied_spec(public_inputs, witness, modulus),
        (r1cs_modulus_valid_spec(modulus)
            && r1cs_witness_valid_spec(witness)
            && ((public_inputs.len >= 1 && public_inputs.index0 >= witness.len)
                || (public_inputs.len >= 2 && public_inputs.index1 >= witness.len)
                || (public_inputs.len >= 3 && public_inputs.index2 >= witness.len)))
            ==> !r1cs_public_inputs_satisfied_spec(public_inputs, witness, modulus),
        (r1cs_modulus_valid_spec(modulus)
            && r1cs_witness_valid_spec(witness)
            && r1cs_public_inputs_len_valid_spec(public_inputs.len)
            && ((public_inputs.len >= 1
                && public_inputs.index0 < witness.len
                && r1cs_element_valid_spec(public_inputs.value0)
                && (public_inputs.value0 as int) % (modulus as int)
                    != (r1cs_witness_value_at_spec(witness, public_inputs.index0) as int)
                        % (modulus as int))
                || (public_inputs.len >= 2
                    && public_inputs.index1 < witness.len
                    && r1cs_element_valid_spec(public_inputs.value1)
                    && (public_inputs.value1 as int) % (modulus as int)
                        != (r1cs_witness_value_at_spec(witness, public_inputs.index1) as int)
                            % (modulus as int))
                || (public_inputs.len >= 3
                    && public_inputs.index2 < witness.len
                    && r1cs_element_valid_spec(public_inputs.value2)
                    && (public_inputs.value2 as int) % (modulus as int)
                        != (r1cs_witness_value_at_spec(witness, public_inputs.index2) as int)
                            % (modulus as int))))
            ==> !r1cs_public_inputs_satisfied_spec(public_inputs, witness, modulus),
        !r1cs_system_satisfied_spec(system, witness)
            ==> !r1cs_public_statement_satisfied_spec(system, witness, public_inputs),
        !r1cs_public_inputs_satisfied_spec(public_inputs, witness, system.modulus)
            ==> !r1cs_public_statement_satisfied_spec(system, witness, public_inputs),
        r1cs_public_inputs_satisfied_spec(public_inputs, witness, modulus)
            ==> (r1cs_public_inputs_len_valid_spec(public_inputs.len)
                && r1cs_modulus_valid_spec(modulus)
                && r1cs_witness_valid_spec(witness)
                && (public_inputs.len < 1
                    || r1cs_public_binding_slot_satisfied_spec(
                        public_inputs.index0,
                        public_inputs.value0,
                        witness,
                        modulus,
                    ))
                && (public_inputs.len < 2
                    || r1cs_public_binding_slot_satisfied_spec(
                        public_inputs.index1,
                        public_inputs.value1,
                        witness,
                        modulus,
                    ))
                && (public_inputs.len < 3
                    || r1cs_public_binding_slot_satisfied_spec(
                        public_inputs.index2,
                        public_inputs.value2,
                        witness,
                        modulus,
                    ))),
        r1cs_public_statement_satisfied_spec(system, witness, public_inputs)
            ==> (r1cs_system_satisfied_spec(system, witness)
                && r1cs_public_inputs_satisfied_spec(public_inputs, witness, system.modulus)),
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY_PUBLIC_INPUT_LEN: u8 = 0;
    const ONE_PUBLIC_INPUT_LEN: u8 = 1;
    const TWO_PUBLIC_INPUT_LEN: u8 = 2;
    const THREE_PUBLIC_INPUT_LEN: u8 = 3;
    const INVALID_PUBLIC_INPUT_LEN: u8 = R1CS_PUBLIC_INPUT_MAX_LEN + 1;
    const ACTIVE_WITNESS_LEN: u8 = 2;
    const THREE_SLOT_WITNESS_LEN: u8 = 3;
    const SYSTEM_LEN: u8 = 2;
    const FIRST_WITNESS_INDEX: u8 = 0;
    const SECOND_WITNESS_INDEX: u8 = 1;
    const THIRD_WITNESS_INDEX: u8 = 2;
    const OUT_OF_RANGE_INDEX: u8 = ACTIVE_WITNESS_LEN;
    const INACTIVE_INDEX: u8 = 99;
    const SYSTEM_MODULUS: u16 = 17;
    const INVALID_MODULUS: u16 = 1;
    const WITNESS_ZERO: u16 = 3;
    const WITNESS_ONE: u16 = 4;
    const WITNESS_TWO: u16 = 15;
    const OVERLARGE_FIELD_VALUE: u16 = 99;
    const MISMATCHED_PUBLIC_VALUE: u16 = 5;
    const BAD_ROW_OUTPUT: u16 = 5;
    const FIRST_ROW_A0: u16 = 1;
    const FIRST_ROW_A1: u16 = 2;
    const FIRST_ROW_B0: u16 = 3;
    const FIRST_ROW_B1: u16 = 1;
    const FIRST_ROW_C0: u16 = 1;
    const FIRST_ROW_C1: u16 = 1;
    const SECOND_ROW_A0: u16 = 1;
    const SECOND_ROW_B1: u16 = 1;
    const SECOND_ROW_C0: u16 = 4;

    fn witness() -> R1csWitness {
        R1csWitness {
            len: ACTIVE_WITNESS_LEN,
            w0: WITNESS_ZERO,
            w1: WITNESS_ONE,
            w2: WITNESS_TWO,
        }
    }

    fn three_slot_witness() -> R1csWitness {
        R1csWitness {
            len: THREE_SLOT_WITNESS_LEN,
            w0: WITNESS_ZERO,
            w1: WITNESS_ONE,
            w2: WITNESS_TWO,
        }
    }

    fn row_product_11_times_13() -> R1csRow {
        R1csRow {
            len: ACTIVE_WITNESS_LEN,
            modulus: SYSTEM_MODULUS,
            a0: FIRST_ROW_A0,
            a1: FIRST_ROW_A1,
            a2: WITNESS_TWO,
            b0: FIRST_ROW_B0,
            b1: FIRST_ROW_B1,
            b2: WITNESS_TWO,
            c0: FIRST_ROW_C0,
            c1: FIRST_ROW_C1,
            c2: WITNESS_TWO,
        }
    }

    fn row_product_3_times_4() -> R1csRow {
        R1csRow {
            len: ACTIVE_WITNESS_LEN,
            modulus: SYSTEM_MODULUS,
            a0: SECOND_ROW_A0,
            a1: 0,
            a2: WITNESS_TWO,
            b0: 0,
            b1: SECOND_ROW_B1,
            b2: WITNESS_TWO,
            c0: SECOND_ROW_C0,
            c1: 0,
            c2: WITNESS_TWO,
        }
    }

    fn inactive_row() -> R1csRow {
        R1csRow {
            len: EMPTY_PUBLIC_INPUT_LEN,
            modulus: INVALID_MODULUS,
            a0: OVERLARGE_FIELD_VALUE,
            a1: OVERLARGE_FIELD_VALUE,
            a2: OVERLARGE_FIELD_VALUE,
            b0: OVERLARGE_FIELD_VALUE,
            b1: OVERLARGE_FIELD_VALUE,
            b2: OVERLARGE_FIELD_VALUE,
            c0: OVERLARGE_FIELD_VALUE,
            c1: OVERLARGE_FIELD_VALUE,
            c2: OVERLARGE_FIELD_VALUE,
        }
    }

    fn satisfied_system() -> R1csSystem {
        R1csSystem {
            len: SYSTEM_LEN,
            modulus: SYSTEM_MODULUS,
            row0: row_product_11_times_13(),
            row1: row_product_3_times_4(),
            row2: inactive_row(),
        }
    }

    fn public_inputs() -> R1csPublicInputs {
        R1csPublicInputs {
            len: TWO_PUBLIC_INPUT_LEN,
            index0: FIRST_WITNESS_INDEX,
            index1: SECOND_WITNESS_INDEX,
            index2: INACTIVE_INDEX,
            value0: WITNESS_ZERO,
            value1: WITNESS_ONE,
            value2: OVERLARGE_FIELD_VALUE,
        }
    }

    fn three_public_inputs() -> R1csPublicInputs {
        R1csPublicInputs {
            len: THREE_PUBLIC_INPUT_LEN,
            index0: FIRST_WITNESS_INDEX,
            index1: SECOND_WITNESS_INDEX,
            index2: THIRD_WITNESS_INDEX,
            value0: WITNESS_ZERO,
            value1: WITNESS_ONE,
            value2: WITNESS_TWO,
        }
    }

    #[test]
    fn matching_public_inputs_are_admitted() {
        assert!(r1cs_public_inputs_satisfied(public_inputs(), witness(), SYSTEM_MODULUS));
    }

    #[test]
    fn satisfied_system_with_matching_public_inputs_is_admitted() {
        assert!(r1cs_public_statement_satisfied(satisfied_system(), witness(), public_inputs()));
    }

    #[test]
    fn mismatched_public_value_rejects_statement() {
        let mut public_inputs = public_inputs();
        public_inputs.value1 = MISMATCHED_PUBLIC_VALUE;
        assert!(!r1cs_public_inputs_satisfied(public_inputs, witness(), SYSTEM_MODULUS));
        assert!(!r1cs_public_statement_satisfied(satisfied_system(), witness(), public_inputs));
    }

    #[test]
    fn out_of_range_public_index_rejects_binding() {
        let mut public_inputs = public_inputs();
        public_inputs.index1 = OUT_OF_RANGE_INDEX;
        assert!(!r1cs_public_inputs_satisfied(public_inputs, witness(), SYSTEM_MODULUS));
    }

    #[test]
    fn malformed_witness_rejects_statement() {
        let mut bad_witness = witness();
        bad_witness.len = THREE_SLOT_WITNESS_LEN;
        assert!(!r1cs_public_statement_satisfied(satisfied_system(), bad_witness, public_inputs()));
    }

    #[test]
    fn unsatisfied_system_rejects_even_matching_public_inputs() {
        let mut system = satisfied_system();
        system.row1.c0 = BAD_ROW_OUTPUT;
        assert!(!r1cs_public_statement_satisfied(system, witness(), public_inputs()));
    }

    #[test]
    fn inactive_malformed_public_slots_are_ignored() {
        let public_inputs = R1csPublicInputs {
            len: ONE_PUBLIC_INPUT_LEN,
            index0: FIRST_WITNESS_INDEX,
            index1: INACTIVE_INDEX,
            index2: INACTIVE_INDEX,
            value0: WITNESS_ZERO,
            value1: OVERLARGE_FIELD_VALUE,
            value2: OVERLARGE_FIELD_VALUE,
        };
        assert!(r1cs_public_inputs_satisfied(public_inputs, witness(), SYSTEM_MODULUS));
    }

    #[test]
    fn empty_public_input_list_is_allowed() {
        let public_inputs = R1csPublicInputs {
            len: EMPTY_PUBLIC_INPUT_LEN,
            index0: INACTIVE_INDEX,
            index1: INACTIVE_INDEX,
            index2: INACTIVE_INDEX,
            value0: OVERLARGE_FIELD_VALUE,
            value1: OVERLARGE_FIELD_VALUE,
            value2: OVERLARGE_FIELD_VALUE,
        };
        assert!(r1cs_public_statement_satisfied(satisfied_system(), witness(), public_inputs));
    }

    #[test]
    fn invalid_public_input_length_is_rejected() {
        let mut public_inputs = public_inputs();
        public_inputs.len = INVALID_PUBLIC_INPUT_LEN;
        assert!(!r1cs_public_inputs_len_valid(public_inputs.len));
        assert!(!r1cs_public_inputs_satisfied(public_inputs, witness(), SYSTEM_MODULUS));
        assert!(!r1cs_public_statement_satisfied(satisfied_system(), witness(), public_inputs));
    }

    #[test]
    fn invalid_modulus_rejects_public_inputs() {
        assert!(!r1cs_modulus_valid(INVALID_MODULUS));
        assert!(!r1cs_public_inputs_satisfied(public_inputs(), witness(), INVALID_MODULUS));
    }

    #[test]
    fn malformed_public_value_rejects_binding() {
        let mut public_inputs = public_inputs();
        public_inputs.value0 = OVERLARGE_FIELD_VALUE;
        assert!(!r1cs_element_valid(public_inputs.value0));
        assert!(!r1cs_public_inputs_satisfied(public_inputs, witness(), SYSTEM_MODULUS));
        assert!(!r1cs_public_statement_satisfied(satisfied_system(), witness(), public_inputs));
    }

    #[test]
    fn malformed_witness_value_rejects_public_inputs() {
        let mut bad_witness = witness();
        bad_witness.w1 = OVERLARGE_FIELD_VALUE;
        assert!(!r1cs_witness_valid(bad_witness));
        assert!(!r1cs_public_inputs_satisfied(public_inputs(), bad_witness, SYSTEM_MODULUS));
        assert!(!r1cs_public_statement_satisfied(satisfied_system(), bad_witness, public_inputs()));
    }

    #[test]
    fn all_three_public_inputs_are_admitted_for_three_slot_witness() {
        assert!(r1cs_public_inputs_satisfied(three_public_inputs(), three_slot_witness(), SYSTEM_MODULUS,));
    }
}
