//! Verified memory ordering model for atomic operations.
//!
//! Models the five memory orderings (Relaxed, Acquire, Release,
//! AcqRel, SeqCst) as a lattice with strength comparisons,
//! valid load/store/RMW combinations, and fence composition rules.

use vstd::prelude::*;

verus! {

// Memory ordering tags (strength-ordered).
pub const ORD_RELAXED: u8 = 0u8;
pub const ORD_ACQUIRE: u8 = 1u8;
pub const ORD_RELEASE: u8 = 2u8;
pub const ORD_ACQ_REL: u8 = 3u8;
pub const ORD_SEQ_CST: u8 = 4u8;

// --- is_valid_ordering ---

// r[depends memory_order.valid-ordering]
/// A value represents a valid memory ordering.
pub open spec fn is_valid_ordering_spec(ord: u8) -> bool {
    ord == ORD_RELAXED
    || ord == ORD_ACQUIRE
    || ord == ORD_RELEASE
    || ord == ORD_ACQ_REL
    || ord == ORD_SEQ_CST
}

// r[impl memory_order.valid-ordering]
pub fn is_valid_ordering(ord: u8) -> (result: bool)
    ensures result == is_valid_ordering_spec(ord)
{
    ord == ORD_RELAXED
    || ord == ORD_ACQUIRE
    || ord == ORD_RELEASE
    || ord == ORD_ACQ_REL
    || ord == ORD_SEQ_CST
}

// --- at_least_as_strong ---

// r[depends memory_order.strength]
/// Ordering `a` is at least as strong as `b` in the memory ordering
/// lattice. SeqCst ≥ AcqRel ≥ {Acquire, Release} ≥ Relaxed.
/// Acquire and Release are incomparable (neither subsumes the other).
pub open spec fn at_least_as_strong_spec(a: u8, b: u8) -> bool {
    // SeqCst is strongest
    a == b
    || a == ORD_SEQ_CST
    || (a == ORD_ACQ_REL && (b == ORD_ACQUIRE || b == ORD_RELEASE || b == ORD_RELAXED))
    || (a == ORD_ACQUIRE && b == ORD_RELAXED)
    || (a == ORD_RELEASE && b == ORD_RELAXED)
}

// r[impl memory_order.strength]
pub fn at_least_as_strong(a: u8, b: u8) -> (result: bool)
    ensures result == at_least_as_strong_spec(a, b)
{
    a == b
    || a == ORD_SEQ_CST
    || (a == ORD_ACQ_REL && (b == ORD_ACQUIRE || b == ORD_RELEASE || b == ORD_RELAXED))
    || (a == ORD_ACQUIRE && b == ORD_RELAXED)
    || (a == ORD_RELEASE && b == ORD_RELAXED)
}

// --- valid_for_load ---

// r[depends memory_order.load]
/// Valid orderings for atomic loads: Relaxed, Acquire, SeqCst.
/// Release and AcqRel are not valid for loads.
pub open spec fn valid_for_load_spec(ord: u8) -> bool {
    ord == ORD_RELAXED || ord == ORD_ACQUIRE || ord == ORD_SEQ_CST
}

// r[impl memory_order.load]
pub fn valid_for_load(ord: u8) -> (result: bool)
    ensures result == valid_for_load_spec(ord)
{
    ord == ORD_RELAXED || ord == ORD_ACQUIRE || ord == ORD_SEQ_CST
}

// --- valid_for_store ---

// r[depends memory_order.store]
/// Valid orderings for atomic stores: Relaxed, Release, SeqCst.
/// Acquire and AcqRel are not valid for stores.
pub open spec fn valid_for_store_spec(ord: u8) -> bool {
    ord == ORD_RELAXED || ord == ORD_RELEASE || ord == ORD_SEQ_CST
}

// r[impl memory_order.store]
pub fn valid_for_store(ord: u8) -> (result: bool)
    ensures result == valid_for_store_spec(ord)
{
    ord == ORD_RELAXED || ord == ORD_RELEASE || ord == ORD_SEQ_CST
}

// --- valid_for_rmw ---

// r[depends memory_order.rmw]
/// Valid orderings for read-modify-write (CAS, fetch_add, etc.):
/// all five orderings are valid for RMW operations.
pub open spec fn valid_for_rmw_spec(ord: u8) -> bool {
    is_valid_ordering_spec(ord)
}

// r[impl memory_order.rmw]
pub fn valid_for_rmw(ord: u8) -> (result: bool)
    ensures result == valid_for_rmw_spec(ord)
{
    is_valid_ordering(ord)
}

// --- has_acquire_semantics ---

// r[depends memory_order.acquire-semantics]
/// Whether an ordering provides acquire semantics (prevents reads
/// from being reordered before this operation).
pub open spec fn has_acquire_semantics_spec(ord: u8) -> bool {
    ord == ORD_ACQUIRE || ord == ORD_ACQ_REL || ord == ORD_SEQ_CST
}

// r[impl memory_order.acquire-semantics]
pub fn has_acquire_semantics(ord: u8) -> (result: bool)
    ensures result == has_acquire_semantics_spec(ord)
{
    ord == ORD_ACQUIRE || ord == ORD_ACQ_REL || ord == ORD_SEQ_CST
}

// --- has_release_semantics ---

// r[depends memory_order.release-semantics]
/// Whether an ordering provides release semantics (prevents writes
/// from being reordered after this operation).
pub open spec fn has_release_semantics_spec(ord: u8) -> bool {
    ord == ORD_RELEASE || ord == ORD_ACQ_REL || ord == ORD_SEQ_CST
}

// r[impl memory_order.release-semantics]
pub fn has_release_semantics(ord: u8) -> (result: bool)
    ensures result == has_release_semantics_spec(ord)
{
    ord == ORD_RELEASE || ord == ORD_ACQ_REL || ord == ORD_SEQ_CST
}

// --- combine_orderings ---

// r[depends memory_order.combine]
/// Combine two orderings into the weakest ordering that is at least
/// as strong as both. This models fence composition:
/// acquire_fence + release_fence ≥ AcqRel.
pub open spec fn combine_orderings_spec(a: u8, b: u8) -> u8 {
    if a == ORD_SEQ_CST || b == ORD_SEQ_CST {
        ORD_SEQ_CST
    } else if (has_acquire_semantics_spec(a) || has_acquire_semantics_spec(b))
        && (has_release_semantics_spec(a) || has_release_semantics_spec(b)) {
        ORD_ACQ_REL
    } else if has_acquire_semantics_spec(a) || has_acquire_semantics_spec(b) {
        ORD_ACQUIRE
    } else if has_release_semantics_spec(a) || has_release_semantics_spec(b) {
        ORD_RELEASE
    } else {
        ORD_RELAXED
    }
}

// r[impl memory_order.combine]
pub fn combine_orderings(a: u8, b: u8) -> (result: u8)
    requires is_valid_ordering_spec(a), is_valid_ordering_spec(b)
    ensures result == combine_orderings_spec(a, b)
{
    if a == ORD_SEQ_CST || b == ORD_SEQ_CST {
        ORD_SEQ_CST
    } else if (has_acquire_semantics(a) || has_acquire_semantics(b))
        && (has_release_semantics(a) || has_release_semantics(b)) {
        ORD_ACQ_REL
    } else if has_acquire_semantics(a) || has_acquire_semantics(b) {
        ORD_ACQUIRE
    } else if has_release_semantics(a) || has_release_semantics(b) {
        ORD_RELEASE
    } else {
        ORD_RELAXED
    }
}

// --- downgrade_to_load ---

// r[depends memory_order.downgrade]
/// Downgrade an ordering for use with loads: strip Release semantics.
pub open spec fn downgrade_to_load_spec(ord: u8) -> u8 {
    if ord == ORD_RELEASE || ord == ORD_RELAXED {
        ORD_RELAXED
    } else if ord == ORD_ACQ_REL || ord == ORD_ACQUIRE {
        ORD_ACQUIRE
    } else {
        ORD_SEQ_CST
    }
}

// r[impl memory_order.downgrade]
pub fn downgrade_to_load(ord: u8) -> (result: u8)
    requires is_valid_ordering_spec(ord)
    ensures result == downgrade_to_load_spec(ord)
{
    if ord == ORD_RELEASE || ord == ORD_RELAXED {
        ORD_RELAXED
    } else if ord == ORD_ACQ_REL || ord == ORD_ACQUIRE {
        ORD_ACQUIRE
    } else {
        ORD_SEQ_CST
    }
}

// --- downgrade_to_store ---

// r[depends memory_order.downgrade]
/// Downgrade an ordering for use with stores: strip Acquire semantics.
pub open spec fn downgrade_to_store_spec(ord: u8) -> u8 {
    if ord == ORD_ACQUIRE || ord == ORD_RELAXED {
        ORD_RELAXED
    } else if ord == ORD_ACQ_REL || ord == ORD_RELEASE {
        ORD_RELEASE
    } else {
        ORD_SEQ_CST
    }
}

// r[impl memory_order.downgrade]
pub fn downgrade_to_store(ord: u8) -> (result: u8)
    requires is_valid_ordering_spec(ord)
    ensures result == downgrade_to_store_spec(ord)
{
    if ord == ORD_ACQUIRE || ord == ORD_RELAXED {
        ORD_RELAXED
    } else if ord == ORD_ACQ_REL || ord == ORD_RELEASE {
        ORD_RELEASE
    } else {
        ORD_SEQ_CST
    }
}

// === Proofs ===

// r[verify memory_order.valid-ordering]
/// All five constants are valid orderings.
proof fn constants_valid()
    ensures
        is_valid_ordering_spec(ORD_RELAXED),
        is_valid_ordering_spec(ORD_ACQUIRE),
        is_valid_ordering_spec(ORD_RELEASE),
        is_valid_ordering_spec(ORD_ACQ_REL),
        is_valid_ordering_spec(ORD_SEQ_CST),
{}

// r[verify memory_order.valid-ordering]
/// All five constants are distinct.
proof fn constants_distinct()
    ensures
        ORD_RELAXED != ORD_ACQUIRE,
        ORD_RELAXED != ORD_RELEASE,
        ORD_RELAXED != ORD_ACQ_REL,
        ORD_RELAXED != ORD_SEQ_CST,
        ORD_ACQUIRE != ORD_RELEASE,
        ORD_ACQUIRE != ORD_ACQ_REL,
        ORD_ACQUIRE != ORD_SEQ_CST,
        ORD_RELEASE != ORD_ACQ_REL,
        ORD_RELEASE != ORD_SEQ_CST,
        ORD_ACQ_REL != ORD_SEQ_CST,
{}

// r[verify memory_order.strength]
/// Reflexivity: every ordering is at least as strong as itself.
proof fn strength_reflexive(ord: u8)
    ensures at_least_as_strong_spec(ord, ord)
{}

// r[verify memory_order.strength]
/// SeqCst is at least as strong as everything.
proof fn seq_cst_is_top(ord: u8)
    ensures at_least_as_strong_spec(ORD_SEQ_CST, ord)
{}

// r[verify memory_order.strength]
/// Relaxed is at most as strong as everything valid.
proof fn relaxed_is_bottom(ord: u8)
    requires is_valid_ordering_spec(ord)
    ensures at_least_as_strong_spec(ord, ORD_RELAXED)
{}

// r[verify memory_order.strength]
/// Acquire and Release are incomparable.
proof fn acquire_release_incomparable()
    ensures
        !at_least_as_strong_spec(ORD_ACQUIRE, ORD_RELEASE),
        !at_least_as_strong_spec(ORD_RELEASE, ORD_ACQUIRE),
{}

// r[verify memory_order.strength]
/// Transitivity of the strength relation (for valid orderings).
proof fn strength_transitive(a: u8, b: u8, c: u8)
    requires
        is_valid_ordering_spec(a),
        is_valid_ordering_spec(b),
        is_valid_ordering_spec(c),
        at_least_as_strong_spec(a, b),
        at_least_as_strong_spec(b, c),
    ensures at_least_as_strong_spec(a, c)
{}

// r[verify memory_order.strength]
/// Antisymmetry: if a ≥ b and b ≥ a, then a == b.
proof fn strength_antisymmetric(a: u8, b: u8)
    requires
        is_valid_ordering_spec(a),
        is_valid_ordering_spec(b),
        at_least_as_strong_spec(a, b),
        at_least_as_strong_spec(b, a),
    ensures a == b
{}

// r[verify memory_order.load]
/// Downgraded ordering is always valid for loads.
proof fn downgrade_load_valid(ord: u8)
    requires is_valid_ordering_spec(ord)
    ensures valid_for_load_spec(downgrade_to_load_spec(ord))
{}

// r[verify memory_order.store]
/// Downgraded ordering is always valid for stores.
proof fn downgrade_store_valid(ord: u8)
    requires is_valid_ordering_spec(ord)
    ensures valid_for_store_spec(downgrade_to_store_spec(ord))
{}

// r[verify memory_order.load]
// r[verify memory_order.store]
/// Release is not valid for loads.
proof fn release_not_for_load()
    ensures !valid_for_load_spec(ORD_RELEASE)
{}

// r[verify memory_order.load]
// r[verify memory_order.store]
/// Acquire is not valid for stores.
proof fn acquire_not_for_store()
    ensures !valid_for_store_spec(ORD_ACQUIRE)
{}

// r[verify memory_order.rmw]
/// Every valid ordering is valid for RMW.
proof fn all_valid_for_rmw(ord: u8)
    requires is_valid_ordering_spec(ord)
    ensures valid_for_rmw_spec(ord)
{}

// r[verify memory_order.combine]
/// Combining is commutative.
proof fn combine_commutative(a: u8, b: u8)
    requires is_valid_ordering_spec(a), is_valid_ordering_spec(b)
    ensures combine_orderings_spec(a, b) == combine_orderings_spec(b, a)
{}

// r[verify memory_order.combine]
/// Combining with Relaxed is identity.
proof fn combine_relaxed_identity(ord: u8)
    requires is_valid_ordering_spec(ord)
    ensures combine_orderings_spec(ord, ORD_RELAXED) == ord
{}

// r[verify memory_order.combine]
/// Combining Acquire + Release = AcqRel.
proof fn combine_acquire_release()
    ensures combine_orderings_spec(ORD_ACQUIRE, ORD_RELEASE) == ORD_ACQ_REL
{}

// r[verify memory_order.combine]
/// Combined ordering is at least as strong as both inputs.
proof fn combine_at_least_both(a: u8, b: u8)
    requires is_valid_ordering_spec(a), is_valid_ordering_spec(b)
    ensures
        at_least_as_strong_spec(combine_orderings_spec(a, b), a),
        at_least_as_strong_spec(combine_orderings_spec(a, b), b),
{}

// r[verify memory_order.combine]
/// Combined ordering is valid.
proof fn combine_produces_valid(a: u8, b: u8)
    requires is_valid_ordering_spec(a), is_valid_ordering_spec(b)
    ensures is_valid_ordering_spec(combine_orderings_spec(a, b))
{}

// r[verify memory_order.acquire-semantics]
// r[verify memory_order.release-semantics]
/// AcqRel has both acquire and release semantics.
proof fn acq_rel_has_both()
    ensures
        has_acquire_semantics_spec(ORD_ACQ_REL),
        has_release_semantics_spec(ORD_ACQ_REL),
{}

// r[verify memory_order.acquire-semantics]
/// Relaxed has neither acquire nor release semantics.
proof fn relaxed_has_neither()
    ensures
        !has_acquire_semantics_spec(ORD_RELAXED),
        !has_release_semantics_spec(ORD_RELAXED),
{}

// r[verify memory_order.downgrade]
/// Downgrade preserves at least as much strength for the target op.
proof fn downgrade_load_preserves_acquire(ord: u8)
    requires is_valid_ordering_spec(ord), has_acquire_semantics_spec(ord)
    ensures has_acquire_semantics_spec(downgrade_to_load_spec(ord))
{}

// r[verify memory_order.downgrade]
/// Downgrade to store preserves release semantics.
proof fn downgrade_store_preserves_release(ord: u8)
    requires is_valid_ordering_spec(ord), has_release_semantics_spec(ord)
    ensures has_release_semantics_spec(downgrade_to_store_spec(ord))
{}

// r[verify memory_order.combine]
/// Combining is idempotent.
proof fn combine_idempotent(ord: u8)
    requires is_valid_ordering_spec(ord)
    ensures combine_orderings_spec(ord, ord) == ord
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    const ALL_ORDERINGS: [u8; 5] = [ORD_RELAXED, ORD_ACQUIRE, ORD_RELEASE, ORD_ACQ_REL, ORD_SEQ_CST];

    #[test]
    fn test_valid_orderings() {
        for &ord in &ALL_ORDERINGS {
            assert!(is_valid_ordering(ord));
        }
        assert!(!is_valid_ordering(5));
        assert!(!is_valid_ordering(255));
    }

    #[test]
    fn test_strength_lattice() {
        // SeqCst ≥ everything
        for &ord in &ALL_ORDERINGS {
            assert!(at_least_as_strong(ORD_SEQ_CST, ord));
        }
        // Relaxed ≤ everything valid
        for &ord in &ALL_ORDERINGS {
            assert!(at_least_as_strong(ord, ORD_RELAXED));
        }
        // Acquire ≱ Release, Release ≱ Acquire
        assert!(!at_least_as_strong(ORD_ACQUIRE, ORD_RELEASE));
        assert!(!at_least_as_strong(ORD_RELEASE, ORD_ACQUIRE));
        // AcqRel ≥ both
        assert!(at_least_as_strong(ORD_ACQ_REL, ORD_ACQUIRE));
        assert!(at_least_as_strong(ORD_ACQ_REL, ORD_RELEASE));
    }

    #[test]
    fn test_valid_for_ops() {
        // Loads: Relaxed, Acquire, SeqCst
        assert!(valid_for_load(ORD_RELAXED));
        assert!(valid_for_load(ORD_ACQUIRE));
        assert!(!valid_for_load(ORD_RELEASE));
        assert!(!valid_for_load(ORD_ACQ_REL));
        assert!(valid_for_load(ORD_SEQ_CST));

        // Stores: Relaxed, Release, SeqCst
        assert!(valid_for_store(ORD_RELAXED));
        assert!(!valid_for_store(ORD_ACQUIRE));
        assert!(valid_for_store(ORD_RELEASE));
        assert!(!valid_for_store(ORD_ACQ_REL));
        assert!(valid_for_store(ORD_SEQ_CST));

        // RMW: all valid
        for &ord in &ALL_ORDERINGS {
            assert!(valid_for_rmw(ord));
        }
    }

    #[test]
    fn test_combine() {
        assert_eq!(combine_orderings(ORD_ACQUIRE, ORD_RELEASE), ORD_ACQ_REL);
        assert_eq!(combine_orderings(ORD_RELAXED, ORD_RELAXED), ORD_RELAXED);
        assert_eq!(combine_orderings(ORD_RELAXED, ORD_SEQ_CST), ORD_SEQ_CST);
        assert_eq!(combine_orderings(ORD_ACQUIRE, ORD_ACQUIRE), ORD_ACQUIRE);
    }

    #[test]
    fn test_downgrade() {
        assert_eq!(downgrade_to_load(ORD_ACQ_REL), ORD_ACQUIRE);
        assert_eq!(downgrade_to_load(ORD_RELEASE), ORD_RELAXED);
        assert_eq!(downgrade_to_load(ORD_SEQ_CST), ORD_SEQ_CST);

        assert_eq!(downgrade_to_store(ORD_ACQ_REL), ORD_RELEASE);
        assert_eq!(downgrade_to_store(ORD_ACQUIRE), ORD_RELAXED);
        assert_eq!(downgrade_to_store(ORD_SEQ_CST), ORD_SEQ_CST);
    }

    #[test]
    fn test_semantics() {
        assert!(has_acquire_semantics(ORD_ACQUIRE));
        assert!(has_acquire_semantics(ORD_ACQ_REL));
        assert!(has_acquire_semantics(ORD_SEQ_CST));
        assert!(!has_acquire_semantics(ORD_RELAXED));
        assert!(!has_acquire_semantics(ORD_RELEASE));

        assert!(has_release_semantics(ORD_RELEASE));
        assert!(has_release_semantics(ORD_ACQ_REL));
        assert!(has_release_semantics(ORD_SEQ_CST));
        assert!(!has_release_semantics(ORD_RELAXED));
        assert!(!has_release_semantics(ORD_ACQUIRE));
    }

    proptest! {
        #[test]
        fn prop_strength_reflexive(ord in prop::sample::select(vec![0u8, 1, 2, 3, 4])) {
            prop_assert!(at_least_as_strong(ord, ord));
        }

        #[test]
        fn prop_seq_cst_top(ord in prop::sample::select(vec![0u8, 1, 2, 3, 4])) {
            prop_assert!(at_least_as_strong(ORD_SEQ_CST, ord));
        }

        #[test]
        fn prop_relaxed_bottom(ord in prop::sample::select(vec![0u8, 1, 2, 3, 4])) {
            prop_assert!(at_least_as_strong(ord, ORD_RELAXED));
        }

        #[test]
        fn prop_combine_commutative(
            a in prop::sample::select(vec![0u8, 1, 2, 3, 4]),
            b in prop::sample::select(vec![0u8, 1, 2, 3, 4]),
        ) {
            prop_assert_eq!(combine_orderings(a, b), combine_orderings(b, a));
        }

        #[test]
        fn prop_combine_idempotent(ord in prop::sample::select(vec![0u8, 1, 2, 3, 4])) {
            prop_assert_eq!(combine_orderings(ord, ord), ord);
        }

        #[test]
        fn prop_combine_produces_valid(
            a in prop::sample::select(vec![0u8, 1, 2, 3, 4]),
            b in prop::sample::select(vec![0u8, 1, 2, 3, 4]),
        ) {
            prop_assert!(is_valid_ordering(combine_orderings(a, b)));
        }

        #[test]
        fn prop_downgrade_load_valid(ord in prop::sample::select(vec![0u8, 1, 2, 3, 4])) {
            prop_assert!(valid_for_load(downgrade_to_load(ord)));
        }

        #[test]
        fn prop_downgrade_store_valid(ord in prop::sample::select(vec![0u8, 1, 2, 3, 4])) {
            prop_assert!(valid_for_store(downgrade_to_store(ord)));
        }

        #[test]
        fn prop_combine_at_least_both(
            a in prop::sample::select(vec![0u8, 1, 2, 3, 4]),
            b in prop::sample::select(vec![0u8, 1, 2, 3, 4]),
        ) {
            let c = combine_orderings(a, b);
            prop_assert!(at_least_as_strong(c, a));
            prop_assert!(at_least_as_strong(c, b));
        }

        #[test]
        fn prop_antisymmetric(
            a in prop::sample::select(vec![0u8, 1, 2, 3, 4]),
            b in prop::sample::select(vec![0u8, 1, 2, 3, 4]),
        ) {
            if at_least_as_strong(a, b) && at_least_as_strong(b, a) {
                prop_assert_eq!(a, b);
            }
        }
    }
}
