//! Verified Zircon handle-right primitives.
//!
//! Zircon authority lives on handles, not objects. Rights are a u64 bitmask.
//! Duplicate/replace may preserve or attenuate rights but must never add bits.
//! Operation checks are simple gate predicates over the mask.

use vstd::prelude::*;

verus! {

pub const ZX_RIGHT_DUPLICATE: u64 = 1u64 << 0;
pub const ZX_RIGHT_TRANSFER: u64 = 1u64 << 1;
pub const ZX_RIGHT_WAIT: u64 = 1u64 << 2;
pub const ZX_RIGHT_SIGNAL: u64 = 1u64 << 3;
pub const ZX_RIGHT_SIGNAL_PEER: u64 = 1u64 << 4;

// r[depends zircon-rights.attenuation]
/// `derived` is a subset of `source` when it contains no bit outside `source`.
pub open spec fn rights_subset_spec(derived: u64, source: u64) -> bool {
    (derived & !source) == 0
}

// r[impl zircon-rights.attenuation]
pub fn is_subset(derived: u64, source: u64) -> (result: bool)
    ensures result == rights_subset_spec(derived, source)
{
    (derived & !source) == 0
}

// r[depends zircon-rights.attenuation]
/// Attenuation keeps only bits present in both `source` and `requested`.
pub open spec fn attenuate_spec(source: u64, requested: u64) -> u64 {
    source & requested
}

// r[impl zircon-rights.attenuation]
pub fn attenuate(source: u64, requested: u64) -> (result: u64)
    ensures result == attenuate_spec(source, requested)
{
    source & requested
}

spec fn has_right_spec(rights: u64, right: u64) -> bool {
    rights & right == right
}

fn has_right(rights: u64, right: u64) -> (result: bool)
    ensures result == has_right_spec(rights, right)
{
    rights & right == right
}

// r[depends zircon-rights.attenuation]
/// Duplicate is valid only when the source has DUPLICATE and the request is a subset.
pub open spec fn can_duplicate_spec(source: u64, requested: u64) -> bool {
    source & ZX_RIGHT_DUPLICATE == ZX_RIGHT_DUPLICATE && rights_subset_spec(requested, source)
}

// r[impl zircon-rights.attenuation]
pub fn can_duplicate(source: u64, requested: u64) -> (result: bool)
    ensures result == can_duplicate_spec(source, requested)
{
    has_right(source, ZX_RIGHT_DUPLICATE) && is_subset(requested, source)
}

// r[depends zircon-rights.attenuation]
/// Replace uses the same attenuation rule as duplicate.
pub open spec fn can_replace_spec(source: u64, requested: u64) -> bool {
    can_duplicate_spec(source, requested)
}

// r[impl zircon-rights.attenuation]
pub fn can_replace(source: u64, requested: u64) -> (result: bool)
    ensures result == can_replace_spec(source, requested)
{
    can_duplicate(source, requested)
}

// r[depends zircon-rights.operation-gates]
pub open spec fn can_transfer_spec(rights: u64) -> bool {
    rights & ZX_RIGHT_TRANSFER == ZX_RIGHT_TRANSFER
}

// r[impl zircon-rights.operation-gates]
pub fn can_transfer(rights: u64) -> (result: bool)
    ensures result == can_transfer_spec(rights)
{
    has_right(rights, ZX_RIGHT_TRANSFER)
}

// r[depends zircon-rights.operation-gates]
pub open spec fn can_wait_spec(rights: u64) -> bool {
    rights & ZX_RIGHT_WAIT == ZX_RIGHT_WAIT
}

// r[impl zircon-rights.operation-gates]
pub fn can_wait(rights: u64) -> (result: bool)
    ensures result == can_wait_spec(rights)
{
    has_right(rights, ZX_RIGHT_WAIT)
}

// r[depends zircon-rights.operation-gates]
pub open spec fn can_signal_spec(rights: u64) -> bool {
    rights & ZX_RIGHT_SIGNAL == ZX_RIGHT_SIGNAL
}

// r[impl zircon-rights.operation-gates]
pub fn can_signal(rights: u64) -> (result: bool)
    ensures result == can_signal_spec(rights)
{
    has_right(rights, ZX_RIGHT_SIGNAL)
}

// r[depends zircon-rights.operation-gates]
pub open spec fn can_signal_peer_spec(rights: u64) -> bool {
    rights & ZX_RIGHT_SIGNAL_PEER == ZX_RIGHT_SIGNAL_PEER
}

// r[impl zircon-rights.operation-gates]
pub fn can_signal_peer(rights: u64) -> (result: bool)
    ensures result == can_signal_peer_spec(rights)
{
    has_right(rights, ZX_RIGHT_SIGNAL_PEER)
}

// r[verify zircon-rights.attenuation]
proof fn attenuation_is_subset(source: u64, requested: u64)
    ensures rights_subset_spec(attenuate_spec(source, requested), source)
{
    assert(((source & requested) & !source) == 0u64) by(bit_vector);
}

// r[verify zircon-rights.attenuation]
proof fn duplicate_same_rights_valid(source: u64)
    requires source & ZX_RIGHT_DUPLICATE == ZX_RIGHT_DUPLICATE
    ensures can_duplicate_spec(source, source)
{
    assert((source & !source) == 0u64) by(bit_vector);
}

// r[verify zircon-rights.attenuation]
proof fn attenuate_never_adds(source: u64, requested: u64)
    ensures rights_subset_spec(attenuate_spec(source, requested), source)
{
    assert((((source & requested) & !source) == 0u64)) by(bit_vector);
}

// r[verify zircon-rights.attenuation]
proof fn duplicate_rejects_expansion(source: u64, requested: u64)
    requires !rights_subset_spec(requested, source)
    ensures !can_duplicate_spec(source, requested), !can_replace_spec(source, requested)
{}

// r[verify zircon-rights.operation-gates]
proof fn wait_requires_wait_right(rights: u64)
    requires !(rights & ZX_RIGHT_WAIT == ZX_RIGHT_WAIT)
    ensures !can_wait_spec(rights)
{}

// r[verify zircon-rights.operation-gates]
proof fn transfer_requires_transfer_right(rights: u64)
    requires !(rights & ZX_RIGHT_TRANSFER == ZX_RIGHT_TRANSFER)
    ensures !can_transfer_spec(rights)
{}

// r[verify zircon-rights.operation-gates]
proof fn signal_requires_signal_right(rights: u64)
    requires !(rights & ZX_RIGHT_SIGNAL == ZX_RIGHT_SIGNAL)
    ensures !can_signal_spec(rights)
{}

// r[verify zircon-rights.operation-gates]
proof fn signal_peer_requires_peer_right(rights: u64)
    requires !(rights & ZX_RIGHT_SIGNAL_PEER == ZX_RIGHT_SIGNAL_PEER)
    ensures !can_signal_peer_spec(rights)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn all_rights() -> u64 {
        ZX_RIGHT_DUPLICATE | ZX_RIGHT_TRANSFER | ZX_RIGHT_WAIT | ZX_RIGHT_SIGNAL | ZX_RIGHT_SIGNAL_PEER
    }

    // r[verify zircon-rights.attenuation]
    #[test]
    fn test_attenuate_subset() {
        let source = all_rights();
        let requested = ZX_RIGHT_TRANSFER | ZX_RIGHT_WAIT;
        let derived = attenuate(source, requested);
        assert_eq!(derived, requested);
        assert!(is_subset(derived, source));
    }

    // r[verify zircon-rights.attenuation]
    #[test]
    fn test_duplicate_same_rights() {
        let rights = all_rights();
        assert!(can_duplicate(rights, rights));
    }

    // r[verify zircon-rights.attenuation]
    #[test]
    fn test_duplicate_expansion_rejected() {
        let source = ZX_RIGHT_DUPLICATE | ZX_RIGHT_WAIT;
        let requested = source | ZX_RIGHT_TRANSFER;
        assert!(!can_duplicate(source, requested));
        assert!(!can_replace(source, requested));
    }

    // r[verify zircon-rights.operation-gates]
    #[test]
    fn test_operation_gates() {
        let rights = ZX_RIGHT_TRANSFER | ZX_RIGHT_SIGNAL;
        assert!(can_transfer(rights));
        assert!(!can_wait(rights));
        assert!(can_signal(rights));
        assert!(!can_signal_peer(rights));
    }

    proptest! {
        // r[verify zircon-rights.attenuation]
        #[test]
        fn prop_attenuate_is_subset(source: u64, requested: u64) {
            let derived = attenuate(source, requested);
            prop_assert!(is_subset(derived, source));
        }

        // r[verify zircon-rights.attenuation]
        #[test]
        fn prop_duplicate_accepts_only_subsets(source: u64, requested: u64) {
            let ok = can_duplicate(source | ZX_RIGHT_DUPLICATE, requested);
            if ok {
                prop_assert!(is_subset(requested, source | ZX_RIGHT_DUPLICATE));
            }
        }

        // r[verify zircon-rights.operation-gates]
        #[test]
        fn prop_wait_requires_bit(rights: u64) {
            prop_assert_eq!(can_wait(rights), rights & ZX_RIGHT_WAIT == ZX_RIGHT_WAIT);
        }

        // r[verify zircon-rights.operation-gates]
        #[test]
        fn prop_signal_peer_requires_bit(rights: u64) {
            prop_assert_eq!(
                can_signal_peer(rights),
                rights & ZX_RIGHT_SIGNAL_PEER == ZX_RIGHT_SIGNAL_PEER,
            );
        }
    }
}
