//! Finite R1CS lookup-table membership primitives.
//!
//! This module checks that a selected witness-slot value is a member of a
//! bounded active table prefix. It deliberately stops at a tiny semantic
//! boundary and does not model PLONK lookup arguments, commitments,
//! transcripts, arbitrary tables, circuit builders, or cryptographic soundness.

use vstd::prelude::*;

use crate::r1cs_constraint::R1csWitness;
use crate::r1cs_constraint::r1cs_element_valid;
#[cfg(verus_keep_ghost)]
use crate::r1cs_constraint::r1cs_element_valid_spec;
use crate::r1cs_constraint::r1cs_len_valid;
#[cfg(verus_keep_ghost)]
use crate::r1cs_constraint::r1cs_len_valid_spec;
use crate::r1cs_constraint::r1cs_witness_valid;
#[cfg(verus_keep_ghost)]
use crate::r1cs_constraint::r1cs_witness_valid_spec;

verus! {
pub const R1CS_LOOKUP_TABLE_LEN_ONE: u8 = 1;
pub const R1CS_LOOKUP_TABLE_LEN_TWO: u8 = 2;
pub const R1CS_LOOKUP_TABLE_LEN_THREE: u8 = 3;
pub const R1CS_LOOKUP_VALUE_INDEX_ONE: u8 = 1;
pub const R1CS_LOOKUP_VALUE_INDEX_TWO: u8 = 2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct R1csLookup {
    pub witness_len: u8,
    pub value_index: u8,
    pub table_len: u8,
    pub table0: u16,
    pub table1: u16,
    pub table2: u16,
}

// r[depends rust-primitives.r1cs-lookup-shape]
pub open spec fn r1cs_lookup_index_valid_spec(lookup: R1csLookup) -> bool {
    0 < lookup.value_index && lookup.value_index < lookup.witness_len
}

// r[depends rust-primitives.r1cs-lookup-shape]
pub open spec fn r1cs_lookup_table_len_valid_spec(table_len: u8) -> bool {
    R1CS_LOOKUP_TABLE_LEN_ONE <= table_len && table_len <= R1CS_LOOKUP_TABLE_LEN_THREE
}

// r[depends rust-primitives.r1cs-lookup-shape]
pub open spec fn r1cs_lookup_active_entries_valid_spec(lookup: R1csLookup) -> bool {
    (lookup.table_len >= R1CS_LOOKUP_TABLE_LEN_ONE ==> r1cs_element_valid_spec(lookup.table0))
        && (lookup.table_len >= R1CS_LOOKUP_TABLE_LEN_TWO ==> r1cs_element_valid_spec(lookup.table1))
        && (lookup.table_len >= R1CS_LOOKUP_TABLE_LEN_THREE ==> r1cs_element_valid_spec(lookup.table2))
}

// r[depends rust-primitives.r1cs-lookup-shape]
pub open spec fn r1cs_lookup_valid_spec(lookup: R1csLookup) -> bool {
    r1cs_len_valid_spec(lookup.witness_len)
        && r1cs_lookup_index_valid_spec(lookup)
        && r1cs_lookup_table_len_valid_spec(lookup.table_len)
        && r1cs_lookup_active_entries_valid_spec(lookup)
}

// r[depends rust-primitives.r1cs-lookup-active-membership]
pub open spec fn r1cs_lookup_value_spec(witness: R1csWitness, index: u8) -> int {
    if index == R1CS_LOOKUP_VALUE_INDEX_ONE { witness.w1 as int } else if index == R1CS_LOOKUP_VALUE_INDEX_TWO { witness.w2 as int } else { 0int }
}

// r[depends rust-primitives.r1cs-lookup-active-membership]
pub open spec fn r1cs_lookup_member_spec(lookup: R1csLookup, value: int) -> bool {
    (lookup.table_len >= R1CS_LOOKUP_TABLE_LEN_ONE && value == lookup.table0 as int)
        || (lookup.table_len >= R1CS_LOOKUP_TABLE_LEN_TWO && value == lookup.table1 as int)
        || (lookup.table_len >= R1CS_LOOKUP_TABLE_LEN_THREE && value == lookup.table2 as int)
}

// r[depends rust-primitives.r1cs-lookup-satisfaction]
pub open spec fn r1cs_lookup_satisfied_spec(lookup: R1csLookup, witness: R1csWitness) -> bool {
    r1cs_lookup_valid_spec(lookup)
        && r1cs_witness_valid_spec(witness)
        && witness.len == lookup.witness_len
        && r1cs_lookup_member_spec(lookup, r1cs_lookup_value_spec(witness, lookup.value_index))
}

// r[impl rust-primitives.r1cs-lookup-shape]
// r[impl rust-primitives.r1cs-lookup-verification-coverage]
pub fn r1cs_lookup_index_valid(lookup: R1csLookup) -> (result: bool)
    ensures result == r1cs_lookup_index_valid_spec(lookup)
{
    0 < lookup.value_index && lookup.value_index < lookup.witness_len
}

// r[impl rust-primitives.r1cs-lookup-shape]
pub fn r1cs_lookup_table_len_valid(table_len: u8) -> (result: bool)
    ensures result == r1cs_lookup_table_len_valid_spec(table_len)
{
    R1CS_LOOKUP_TABLE_LEN_ONE <= table_len && table_len <= R1CS_LOOKUP_TABLE_LEN_THREE
}

// r[impl rust-primitives.r1cs-lookup-shape]
pub fn r1cs_lookup_active_entries_valid(lookup: R1csLookup) -> (result: bool)
    ensures result == r1cs_lookup_active_entries_valid_spec(lookup)
{
    if lookup.table_len >= R1CS_LOOKUP_TABLE_LEN_ONE && !r1cs_element_valid(lookup.table0) {
        return false;
    }
    if lookup.table_len >= R1CS_LOOKUP_TABLE_LEN_TWO && !r1cs_element_valid(lookup.table1) {
        return false;
    }
    if lookup.table_len >= R1CS_LOOKUP_TABLE_LEN_THREE && !r1cs_element_valid(lookup.table2) {
        return false;
    }
    true
}

// r[impl rust-primitives.r1cs-lookup-shape]
pub fn r1cs_lookup_valid(lookup: R1csLookup) -> (result: bool)
    ensures result == r1cs_lookup_valid_spec(lookup)
{
    r1cs_len_valid(lookup.witness_len)
        && r1cs_lookup_index_valid(lookup)
        && r1cs_lookup_table_len_valid(lookup.table_len)
        && r1cs_lookup_active_entries_valid(lookup)
}

// r[impl rust-primitives.r1cs-lookup-active-membership]
pub fn r1cs_lookup_value(witness: R1csWitness, index: u8) -> (result: u16)
    ensures result as int == r1cs_lookup_value_spec(witness, index)
{
    if index == R1CS_LOOKUP_VALUE_INDEX_ONE {
        witness.w1
    } else if index == R1CS_LOOKUP_VALUE_INDEX_TWO {
        witness.w2
    } else {
        0
    }
}

// r[impl rust-primitives.r1cs-lookup-active-membership]
pub fn r1cs_lookup_member(lookup: R1csLookup, value: u16) -> (result: bool)
    ensures result == r1cs_lookup_member_spec(lookup, value as int)
{
    (lookup.table_len >= R1CS_LOOKUP_TABLE_LEN_ONE && value == lookup.table0)
        || (lookup.table_len >= R1CS_LOOKUP_TABLE_LEN_TWO && value == lookup.table1)
        || (lookup.table_len >= R1CS_LOOKUP_TABLE_LEN_THREE && value == lookup.table2)
}

// r[impl rust-primitives.r1cs-lookup-satisfaction]
pub fn r1cs_lookup_satisfied(lookup: R1csLookup, witness: R1csWitness) -> (result: bool)
    ensures result == r1cs_lookup_satisfied_spec(lookup, witness)
{
    if !r1cs_lookup_valid(lookup) {
        return false;
    }
    if !r1cs_witness_valid(witness) {
        return false;
    }
    if witness.len != lookup.witness_len {
        return false;
    }
    let value = r1cs_lookup_value(witness, lookup.value_index);
    r1cs_lookup_member(lookup, value)
}

// r[verify rust-primitives.r1cs-lookup-shape]
pub proof fn malformed_lookup_shape_rejected(lookup: R1csLookup, witness: R1csWitness)
    requires !r1cs_lookup_valid_spec(lookup)
    ensures !r1cs_lookup_satisfied_spec(lookup, witness)
{}

// r[verify rust-primitives.r1cs-lookup-satisfaction]
pub proof fn malformed_lookup_witness_rejected(lookup: R1csLookup, witness: R1csWitness)
    requires !r1cs_witness_valid_spec(witness) || witness.len != lookup.witness_len
    ensures !r1cs_lookup_satisfied_spec(lookup, witness)
{}

// r[verify rust-primitives.r1cs-lookup-active-membership]
pub proof fn inactive_lookup_entries_ignored(lookup: R1csLookup, value: u16)
    requires
        lookup.table_len == R1CS_LOOKUP_TABLE_LEN_ONE,
        value as int != lookup.table0 as int,
    ensures !r1cs_lookup_member_spec(lookup, value as int)
{}

// r[verify rust-primitives.r1cs-lookup-active-membership]
pub proof fn inactive_third_lookup_entry_ignored(lookup: R1csLookup, value: u16)
    requires
        lookup.table_len == R1CS_LOOKUP_TABLE_LEN_TWO,
        value as int != lookup.table0 as int,
        value as int != lookup.table1 as int,
    ensures !r1cs_lookup_member_spec(lookup, value as int)
{}

// r[verify rust-primitives.r1cs-lookup-satisfaction]
pub proof fn lookup_non_member_rejected(lookup: R1csLookup, witness: R1csWitness)
    requires !r1cs_lookup_member_spec(lookup, r1cs_lookup_value_spec(witness, lookup.value_index))
    ensures !r1cs_lookup_satisfied_spec(lookup, witness)
{}

// r[verify rust-primitives.r1cs-lookup-verification-coverage]
pub proof fn r1cs_lookup_primitive_coverage_catalogued(
    lookup: R1csLookup,
    witness: R1csWitness,
    value: u16,
)
    ensures
        !r1cs_lookup_valid_spec(lookup) ==> !r1cs_lookup_satisfied_spec(lookup, witness),
        (!r1cs_witness_valid_spec(witness) || witness.len != lookup.witness_len)
            ==> !r1cs_lookup_satisfied_spec(lookup, witness),
        (lookup.table_len == R1CS_LOOKUP_TABLE_LEN_ONE
            && value as int != lookup.table0 as int)
            ==> !r1cs_lookup_member_spec(lookup, value as int),
        (lookup.table_len == R1CS_LOOKUP_TABLE_LEN_TWO
            && value as int != lookup.table0 as int
            && value as int != lookup.table1 as int)
            ==> !r1cs_lookup_member_spec(lookup, value as int),
        r1cs_lookup_satisfied_spec(lookup, witness)
            ==> (r1cs_lookup_valid_spec(lookup)
                && r1cs_witness_valid_spec(witness)
                && witness.len == lookup.witness_len
                && r1cs_lookup_member_spec(
                    lookup,
                    r1cs_lookup_value_spec(witness, lookup.value_index),
                )),
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_WITNESS_LEN: u8 = R1CS_LOOKUP_TABLE_LEN_THREE;
    const UNIT_WITNESS_VALUE: u16 = 1;
    const FIRST_ACTIVE_ENTRY: u16 = 2;
    const SECOND_ACTIVE_ENTRY: u16 = 5;
    const THIRD_ACTIVE_ENTRY: u16 = 8;
    const NON_MEMBER_VALUE: u16 = 7;
    const OVERLARGE_ENTRY: u16 = 99;
    const ZERO_TABLE_LEN: u8 = 0;
    const OVERSIZED_TABLE_LEN: u8 = R1CS_LOOKUP_TABLE_LEN_THREE + 1;
    const ZERO_VALUE_INDEX: u8 = 0;
    const OUT_OF_RANGE_VALUE_INDEX: u8 = VALID_WITNESS_LEN;

    fn lookup_table() -> R1csLookup {
        R1csLookup {
            witness_len: VALID_WITNESS_LEN,
            value_index: R1CS_LOOKUP_VALUE_INDEX_ONE,
            table_len: R1CS_LOOKUP_TABLE_LEN_THREE,
            table0: FIRST_ACTIVE_ENTRY,
            table1: SECOND_ACTIVE_ENTRY,
            table2: THIRD_ACTIVE_ENTRY,
        }
    }

    fn witness_with_slots(w1: u16, w2: u16) -> R1csWitness {
        R1csWitness {
            len: VALID_WITNESS_LEN,
            w0: UNIT_WITNESS_VALUE,
            w1,
            w2,
        }
    }

    fn witness(value: u16) -> R1csWitness {
        witness_with_slots(value, NON_MEMBER_VALUE)
    }

    #[test]
    fn accepts_first_active_entry() {
        assert!(r1cs_lookup_satisfied(lookup_table(), witness(FIRST_ACTIVE_ENTRY)));
    }

    #[test]
    fn accepts_last_active_entry() {
        assert!(r1cs_lookup_satisfied(lookup_table(), witness(THIRD_ACTIVE_ENTRY)));
    }

    #[test]
    fn accepts_value_from_second_witness_slot() {
        let mut lookup = lookup_table();
        lookup.value_index = R1CS_LOOKUP_VALUE_INDEX_TWO;
        assert!(r1cs_lookup_satisfied(lookup, witness_with_slots(NON_MEMBER_VALUE, SECOND_ACTIVE_ENTRY)));
    }

    #[test]
    fn rejects_non_member() {
        assert!(!r1cs_lookup_satisfied(lookup_table(), witness(NON_MEMBER_VALUE)));
    }

    #[test]
    fn inactive_entries_are_ignored() {
        let mut lookup = lookup_table();
        lookup.table_len = R1CS_LOOKUP_TABLE_LEN_ONE;
        lookup.table1 = NON_MEMBER_VALUE;
        lookup.table2 = NON_MEMBER_VALUE;
        assert!(!r1cs_lookup_satisfied(lookup, witness(NON_MEMBER_VALUE)));
        assert!(r1cs_lookup_satisfied(lookup, witness(FIRST_ACTIVE_ENTRY)));
    }

    #[test]
    fn inactive_third_entry_is_ignored_for_two_entry_table() {
        let mut lookup = lookup_table();
        lookup.table_len = R1CS_LOOKUP_TABLE_LEN_TWO;
        lookup.table2 = NON_MEMBER_VALUE;
        assert!(!r1cs_lookup_satisfied(lookup, witness(NON_MEMBER_VALUE)));
        assert!(r1cs_lookup_satisfied(lookup, witness(SECOND_ACTIVE_ENTRY)));
    }

    #[test]
    fn active_match_is_accepted_when_inactive_duplicate_also_matches() {
        let mut lookup = lookup_table();
        lookup.table_len = R1CS_LOOKUP_TABLE_LEN_TWO;
        lookup.table2 = SECOND_ACTIVE_ENTRY;
        assert!(r1cs_lookup_satisfied(lookup, witness(SECOND_ACTIVE_ENTRY)));
    }

    #[test]
    fn inactive_overlarge_entry_is_ignored() {
        let mut lookup = lookup_table();
        lookup.table_len = R1CS_LOOKUP_TABLE_LEN_ONE;
        lookup.table1 = OVERLARGE_ENTRY;
        lookup.table2 = OVERLARGE_ENTRY;
        assert!(r1cs_lookup_valid(lookup));
        assert!(r1cs_lookup_satisfied(lookup, witness(FIRST_ACTIVE_ENTRY)));
    }

    #[test]
    fn active_overlarge_entry_is_rejected() {
        let mut lookup = lookup_table();
        lookup.table1 = OVERLARGE_ENTRY;
        assert!(!r1cs_lookup_valid(lookup));
        assert!(!r1cs_lookup_satisfied(lookup, witness(SECOND_ACTIVE_ENTRY)));
    }

    #[test]
    fn zero_table_length_is_rejected() {
        let mut lookup = lookup_table();
        lookup.table_len = ZERO_TABLE_LEN;
        assert!(!r1cs_lookup_valid(lookup));
        assert!(!r1cs_lookup_satisfied(lookup, witness(FIRST_ACTIVE_ENTRY)));
    }

    #[test]
    fn oversized_table_length_is_rejected() {
        let mut lookup = lookup_table();
        lookup.table_len = OVERSIZED_TABLE_LEN;
        assert!(!r1cs_lookup_valid(lookup));
        assert!(!r1cs_lookup_satisfied(lookup, witness(FIRST_ACTIVE_ENTRY)));
    }

    #[test]
    fn zero_value_index_is_rejected() {
        let mut lookup = lookup_table();
        lookup.value_index = ZERO_VALUE_INDEX;
        assert!(!r1cs_lookup_valid(lookup));
        assert!(!r1cs_lookup_satisfied(lookup, witness(FIRST_ACTIVE_ENTRY)));
    }

    #[test]
    fn out_of_range_value_index_is_rejected() {
        let mut lookup = lookup_table();
        lookup.value_index = OUT_OF_RANGE_VALUE_INDEX;
        assert!(!r1cs_lookup_valid(lookup));
        assert!(!r1cs_lookup_satisfied(lookup, witness(FIRST_ACTIVE_ENTRY)));
    }

    #[test]
    fn malformed_witness_is_rejected() {
        let mut bad = witness(FIRST_ACTIVE_ENTRY);
        bad.w1 = OVERLARGE_ENTRY;
        assert!(!r1cs_lookup_satisfied(lookup_table(), bad));
    }

    #[test]
    fn mismatched_witness_length_is_rejected() {
        let mut bad = witness(FIRST_ACTIVE_ENTRY);
        bad.len = R1CS_LOOKUP_TABLE_LEN_TWO;
        assert!(!r1cs_lookup_satisfied(lookup_table(), bad));
    }
}
