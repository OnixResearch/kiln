//! Verified seL4 capability-rights algebra.
//!
//! Models authority as a bitmask over semantic rights. Derivation may preserve
//! or drop rights but must never add them. Kernel operation checks are thin
//! gate predicates over the corresponding right bit.

use vstd::prelude::*;

verus! {

pub const SEL4_RIGHT_SEND: u64 = 1u64 << 0;
pub const SEL4_RIGHT_RECEIVE: u64 = 1u64 << 1;
pub const SEL4_RIGHT_GRANT: u64 = 1u64 << 2;
pub const SEL4_RIGHT_MAP: u64 = 1u64 << 3;
pub const SEL4_RIGHT_CONTROL: u64 = 1u64 << 4;

// r[depends sel4-cap-rights.attenuation]
/// `derived` is a subset of `source` when it contains no bit absent from `source`.
pub open spec fn rights_subset_spec(derived: u64, source: u64) -> bool {
    (derived & !source) == 0
}

// r[impl sel4-cap-rights.attenuation]
pub fn rights_subset(derived: u64, source: u64) -> (result: bool)
    ensures result == rights_subset_spec(derived, source)
{
    (derived & !source) == 0
}

// r[depends sel4-cap-rights.masking]
/// Masking keeps only bits present in both `rights` and `mask`.
pub open spec fn mask_rights_spec(rights: u64, mask: u64) -> u64 {
    rights & mask
}

// r[impl sel4-cap-rights.masking]
pub fn mask_rights(rights: u64, mask: u64) -> (result: u64)
    ensures result == mask_rights_spec(rights, mask)
{
    rights & mask
}

// r[depends sel4-cap-rights.attenuation]
/// Attenuation is masking with a requested derived-rights set.
pub open spec fn attenuate_spec(source: u64, requested: u64) -> u64 {
    mask_rights_spec(source, requested)
}

// r[impl sel4-cap-rights.attenuation]
pub fn attenuate(source: u64, requested: u64) -> (result: u64)
    ensures result == attenuate_spec(source, requested)
{
    mask_rights(source, requested)
}

spec fn has_right_spec(rights: u64, right: u64) -> bool {
    rights & right == right
}

fn has_right(rights: u64, right: u64) -> (result: bool)
    ensures result == has_right_spec(rights, right)
{
    rights & right == right
}

// r[depends sel4-cap-rights.operation-gates]
pub open spec fn can_send_spec(rights: u64) -> bool {
    rights & SEL4_RIGHT_SEND == SEL4_RIGHT_SEND
}

// r[impl sel4-cap-rights.operation-gates]
pub fn can_send(rights: u64) -> (result: bool)
    ensures result == can_send_spec(rights)
{
    has_right(rights, SEL4_RIGHT_SEND)
}

// r[depends sel4-cap-rights.operation-gates]
pub open spec fn can_receive_spec(rights: u64) -> bool {
    rights & SEL4_RIGHT_RECEIVE == SEL4_RIGHT_RECEIVE
}

// r[impl sel4-cap-rights.operation-gates]
pub fn can_receive(rights: u64) -> (result: bool)
    ensures result == can_receive_spec(rights)
{
    has_right(rights, SEL4_RIGHT_RECEIVE)
}

// r[depends sel4-cap-rights.operation-gates]
pub open spec fn can_grant_spec(rights: u64) -> bool {
    rights & SEL4_RIGHT_GRANT == SEL4_RIGHT_GRANT
}

// r[impl sel4-cap-rights.operation-gates]
pub fn can_grant(rights: u64) -> (result: bool)
    ensures result == can_grant_spec(rights)
{
    has_right(rights, SEL4_RIGHT_GRANT)
}

// r[depends sel4-cap-rights.operation-gates]
pub open spec fn can_map_spec(rights: u64) -> bool {
    rights & SEL4_RIGHT_MAP == SEL4_RIGHT_MAP
}

// r[impl sel4-cap-rights.operation-gates]
pub fn can_map(rights: u64) -> (result: bool)
    ensures result == can_map_spec(rights)
{
    has_right(rights, SEL4_RIGHT_MAP)
}

// r[depends sel4-cap-rights.operation-gates]
pub open spec fn can_control_spec(rights: u64) -> bool {
    rights & SEL4_RIGHT_CONTROL == SEL4_RIGHT_CONTROL
}

// r[impl sel4-cap-rights.operation-gates]
pub fn can_control(rights: u64) -> (result: bool)
    ensures result == can_control_spec(rights)
{
    has_right(rights, SEL4_RIGHT_CONTROL)
}

// r[verify sel4-cap-rights.attenuation]
pub proof fn attenuation_is_subset(source: u64, requested: u64)
    ensures rights_subset_spec(attenuate_spec(source, requested), source)
{
    assert((((source & requested) & !source) == 0u64)) by(bit_vector);
}

// r[verify sel4-cap-rights.attenuation]
pub proof fn copy_preserves_rights(source: u64)
    ensures attenuate_spec(source, source) == source
{
    assert((source & source) == source) by(bit_vector);
}

// r[verify sel4-cap-rights.attenuation]
pub proof fn subset_request_roundtrips(source: u64, requested: u64)
    requires rights_subset_spec(requested, source)
    ensures attenuate_spec(source, requested) == requested
{
    assert((source & requested) == requested) by(bit_vector)
        requires (requested & !source) == 0u64;
}

// r[verify sel4-cap-rights.masking]
pub proof fn masking_is_idempotent(rights: u64, mask: u64)
    ensures mask_rights_spec(mask_rights_spec(rights, mask), mask) == mask_rights_spec(rights, mask)
{
    assert((((rights & mask) & mask) == (rights & mask))) by(bit_vector);
}

// r[verify sel4-cap-rights.masking]
pub proof fn masking_never_adds(rights: u64, mask: u64)
    ensures rights_subset_spec(mask_rights_spec(rights, mask), rights)
{
    assert((((rights & mask) & !rights) == 0u64)) by(bit_vector);
}

// r[verify sel4-cap-rights.masking]
pub proof fn masking_is_monotone(rights: u64, smaller_mask: u64, larger_mask: u64)
    requires rights_subset_spec(smaller_mask, larger_mask)
    ensures rights_subset_spec(mask_rights_spec(rights, smaller_mask), mask_rights_spec(rights, larger_mask))
{
    assert((((rights & smaller_mask) & !(rights & larger_mask)) == 0u64)) by(bit_vector)
        requires (smaller_mask & !larger_mask) == 0u64;
}

// r[verify sel4-cap-rights.operation-gates]
pub proof fn send_requires_send_right(rights: u64)
    requires !can_send_spec(rights)
    ensures !(rights & SEL4_RIGHT_SEND == SEL4_RIGHT_SEND)
{}

// r[verify sel4-cap-rights.operation-gates]
pub proof fn receive_requires_receive_right(rights: u64)
    requires !can_receive_spec(rights)
    ensures !(rights & SEL4_RIGHT_RECEIVE == SEL4_RIGHT_RECEIVE)
{}

// r[verify sel4-cap-rights.operation-gates]
pub proof fn grant_requires_grant_right(rights: u64)
    requires !can_grant_spec(rights)
    ensures !(rights & SEL4_RIGHT_GRANT == SEL4_RIGHT_GRANT)
{}

// r[verify sel4-cap-rights.operation-gates]
pub proof fn map_requires_map_right(rights: u64)
    requires !can_map_spec(rights)
    ensures !(rights & SEL4_RIGHT_MAP == SEL4_RIGHT_MAP)
{}

// r[verify sel4-cap-rights.operation-gates]
pub proof fn control_requires_control_right(rights: u64)
    requires !can_control_spec(rights)
    ensures !(rights & SEL4_RIGHT_CONTROL == SEL4_RIGHT_CONTROL)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn all_rights() -> u64 {
        SEL4_RIGHT_SEND | SEL4_RIGHT_RECEIVE | SEL4_RIGHT_GRANT | SEL4_RIGHT_MAP | SEL4_RIGHT_CONTROL
    }

    // r[verify sel4-cap-rights.attenuation]
    #[test]
    fn test_copy_preserves_rights() {
        let rights = all_rights();
        assert_eq!(attenuate(rights, rights), rights);
        assert!(rights_subset(attenuate(rights, rights), rights));
    }

    // r[verify sel4-cap-rights.attenuation]
    #[test]
    fn test_mask_drops_rights_only() {
        let source = all_rights();
        let requested = SEL4_RIGHT_SEND | SEL4_RIGHT_MAP;
        let derived = attenuate(source, requested);
        assert_eq!(derived, requested);
        assert!(rights_subset(derived, source));
    }

    // r[verify sel4-cap-rights.masking]
    #[test]
    fn test_masking_idempotent() {
        let rights = all_rights();
        let mask = SEL4_RIGHT_SEND | SEL4_RIGHT_GRANT;
        assert_eq!(mask_rights(mask_rights(rights, mask), mask), mask_rights(rights, mask));
    }

    // r[verify sel4-cap-rights.operation-gates]
    #[test]
    fn test_operation_gates() {
        let rights = SEL4_RIGHT_SEND | SEL4_RIGHT_MAP;
        assert!(can_send(rights));
        assert!(!can_receive(rights));
        assert!(!can_grant(rights));
        assert!(can_map(rights));
        assert!(!can_control(rights));
    }

    proptest! {
        // r[verify sel4-cap-rights.attenuation]
        #[test]
        fn prop_attenuation_is_subset(source: u64, requested: u64) {
            let derived = attenuate(source, requested);
            prop_assert!(rights_subset(derived, source));
        }

        // r[verify sel4-cap-rights.masking]
        #[test]
        fn prop_masking_idempotent(rights: u64, mask: u64) {
            prop_assert_eq!(
                mask_rights(mask_rights(rights, mask), mask),
                mask_rights(rights, mask),
            );
        }

        // r[verify sel4-cap-rights.masking]
        #[test]
        fn prop_masking_never_adds(rights: u64, mask: u64) {
            let masked = mask_rights(rights, mask);
            prop_assert!(rights_subset(masked, rights));
        }

        // r[verify sel4-cap-rights.operation-gates]
        #[test]
        fn prop_send_gate_matches_bit(rights: u64) {
            prop_assert_eq!(can_send(rights), rights & SEL4_RIGHT_SEND == SEL4_RIGHT_SEND);
        }

        // r[verify sel4-cap-rights.operation-gates]
        #[test]
        fn prop_control_gate_matches_bit(rights: u64) {
            prop_assert_eq!(can_control(rights), rights & SEL4_RIGHT_CONTROL == SEL4_RIGHT_CONTROL);
        }
    }
}
