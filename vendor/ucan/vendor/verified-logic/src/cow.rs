//! Verified copy-on-write decision logic.
//!
//! Models the copy-on-write pattern: shared data is copied before
//! mutation to preserve snapshot isolation for readers. Functions
//! cover reference counting, the copy decision, and value visibility
//! after writes. Every function has a machine-checked spec and proven
//! properties.

use vstd::prelude::*;

verus! {

// --- should_copy ---

// r[depends cow.should-copy]
/// Spec: whether a copy is needed before writing.
/// Copy is needed when the data is shared (ref_count > 1).
pub open spec fn should_copy_spec(ref_count: u64) -> bool {
    ref_count > 1
}

// r[impl cow.should-copy]
/// Returns true if a copy must be made before writing.
///
/// The caller holds one reference. If others exist (`ref_count` > 1),
/// the underlying data must be duplicated before mutation.
pub fn should_copy(ref_count: u64) -> (result: bool)
    requires ref_count > 0
    ensures result == should_copy_spec(ref_count)
{
    ref_count > 1
}

// r[verify cow.should-copy]
/// Exclusive owner never needs to copy.
proof fn exclusive_no_copy()
    ensures !should_copy_spec(1)
{}

// r[verify cow.should-copy]
/// Shared data always needs a copy before writing.
proof fn shared_needs_copy(ref_count: u64)
    requires ref_count > 1
    ensures should_copy_spec(ref_count)
{}

// --- ref_count_after_clone ---

// r[depends cow.ref-count-after-clone]
/// Spec: ref count after cloning a shared reference.
pub open spec fn ref_count_after_clone_spec(ref_count: u64) -> u64 {
    if ref_count < u64::MAX { (ref_count + 1) as u64 } else { ref_count }
}

// r[impl cow.ref-count-after-clone]
/// Returns the new ref count after cloning a reference.
/// Saturates at `u64::MAX`.
pub fn ref_count_after_clone(ref_count: u64) -> (result: u64)
    requires ref_count > 0
    ensures result == ref_count_after_clone_spec(ref_count)
{
    if ref_count < u64::MAX { ref_count + 1 } else { ref_count }
}

// r[verify cow.ref-count-after-clone]
/// Clone always increases ref count (when not saturated).
proof fn clone_increases(ref_count: u64)
    requires ref_count > 0, ref_count < u64::MAX
    ensures ref_count_after_clone_spec(ref_count) == ref_count + 1
{}

// r[verify cow.ref-count-after-clone]
/// Clone result is always >= input.
proof fn clone_non_decreasing(ref_count: u64)
    requires ref_count > 0
    ensures ref_count_after_clone_spec(ref_count) >= ref_count
{}

// --- ref_count_after_drop ---

// r[depends cow.ref-count-after-drop]
/// Spec: ref count after dropping a reference.
pub open spec fn ref_count_after_drop_spec(ref_count: u64) -> u64 {
    if ref_count > 0 { (ref_count - 1) as u64 } else { 0 }
}

// r[impl cow.ref-count-after-drop]
/// Returns the new ref count after dropping a reference.
/// Floors at zero.
pub fn ref_count_after_drop(ref_count: u64) -> (result: u64)
    ensures result == ref_count_after_drop_spec(ref_count)
{
    if ref_count > 0 { ref_count - 1 } else { 0 }
}

// r[verify cow.ref-count-after-drop]
/// Drop always decreases ref count (when positive).
proof fn drop_decreases(ref_count: u64)
    requires ref_count > 0
    ensures ref_count_after_drop_spec(ref_count) == ref_count - 1
{}

// r[verify cow.ref-count-after-drop]
/// Drop result is always <= input.
proof fn drop_non_increasing(ref_count: u64)
    ensures ref_count_after_drop_spec(ref_count) <= ref_count
{}

// --- is_exclusive ---

// r[depends cow.is-exclusive]
/// Spec: whether this is the sole owner.
pub open spec fn is_exclusive_spec(ref_count: u64) -> bool {
    ref_count == 1
}

// r[impl cow.is-exclusive]
/// Returns true if this is the only reference (no sharing).
pub fn is_exclusive(ref_count: u64) -> (result: bool)
    ensures result == is_exclusive_spec(ref_count)
{
    ref_count == 1
}

// r[verify cow.is-exclusive]
// r[verify cow.should-copy]
/// Exclusive and should_copy are complements for valid ref counts.
proof fn exclusive_vs_shared(ref_count: u64)
    requires ref_count > 0
    ensures is_exclusive_spec(ref_count) != should_copy_spec(ref_count)
{}

// --- can_reclaim ---

// r[depends cow.can-reclaim]
/// Spec: whether the backing storage can be freed.
pub open spec fn can_reclaim_spec(ref_count: u64) -> bool {
    ref_count == 0
}

// r[impl cow.can-reclaim]
/// Returns true if no references remain and the storage can be freed.
pub fn can_reclaim(ref_count: u64) -> (result: bool)
    ensures result == can_reclaim_spec(ref_count)
{
    ref_count == 0
}

// r[verify cow.can-reclaim]
/// A live reference (ref_count > 0) cannot be reclaimed.
proof fn live_not_reclaimable(ref_count: u64)
    requires ref_count > 0
    ensures !can_reclaim_spec(ref_count)
{}

// r[verify cow.can-reclaim]
/// Dropping the last reference makes the value reclaimable.
proof fn drop_last_reclaimable()
    ensures can_reclaim_spec(ref_count_after_drop_spec(1))
{}

// --- snapshot isolation specs ---

// r[depends cow.snapshot-isolation]
/// Spec: the value visible to the writer after a COW write.
/// The writer always sees the new value.
pub open spec fn cow_writer_value_spec(
    old_value: int,
    new_value: int,
) -> int {
    new_value
}

// r[depends cow.snapshot-isolation]
/// Spec: the value visible to other readers after a COW write.
/// When shared, readers see the old value (the copy isolated them).
/// When exclusive, no other readers exist — the value is moot but
/// returns new_value for consistency.
pub open spec fn cow_reader_value_spec(
    old_value: int,
    new_value: int,
    ref_count: u64,
) -> int {
    if ref_count > 1 {
        old_value
    } else {
        new_value
    }
}

// r[impl cow.snapshot-isolation]
/// Returns the value visible to the writer after a COW write.
pub fn cow_writer_value(
    _old_value: u64,
    new_value: u64,
) -> (result: u64)
    ensures result as int == cow_writer_value_spec(_old_value as int, new_value as int)
{
    new_value
}

// r[impl cow.snapshot-isolation]
/// Returns the value visible to other readers after a COW write.
pub fn cow_reader_value(
    old_value: u64,
    new_value: u64,
    ref_count: u64,
) -> (result: u64)
    requires ref_count > 0
    ensures result as int == cow_reader_value_spec(old_value as int, new_value as int, ref_count)
{
    if ref_count > 1 {
        old_value
    } else {
        new_value
    }
}

// r[verify cow.snapshot-isolation]
/// Writer always sees the new value after a COW write.
proof fn writer_sees_new_value(old_value: int, new_value: int)
    ensures cow_writer_value_spec(old_value, new_value) == new_value
{}

// r[verify cow.snapshot-isolation]
/// Shared readers see the old value after a COW write.
proof fn shared_readers_see_old_value(old_value: int, new_value: int, ref_count: u64)
    requires ref_count > 1
    ensures cow_reader_value_spec(old_value, new_value, ref_count) == old_value
{}

// r[verify cow.snapshot-isolation]
/// When writing the same value, readers and writer agree.
proof fn write_same_value_agreement(value: int, ref_count: u64)
    requires ref_count > 0
    ensures
        cow_writer_value_spec(value, value) == value,
        cow_reader_value_spec(value, value, ref_count) == value,
{}

// --- lifecycle proofs ---

// r[verify cow.ref-count-after-clone]
// r[verify cow.ref-count-after-drop]
/// Clone then drop returns to the original ref count.
proof fn clone_drop_roundtrip(ref_count: u64)
    requires ref_count > 0, ref_count < u64::MAX
    ensures ref_count_after_drop_spec(ref_count_after_clone_spec(ref_count)) == ref_count
{}

// r[verify cow.ref-count-after-clone]
// r[verify cow.ref-count-after-drop]
/// Drop then clone returns to the original ref count.
proof fn drop_clone_roundtrip(ref_count: u64)
    requires ref_count > 1
    ensures ref_count_after_clone_spec(ref_count_after_drop_spec(ref_count)) == ref_count
{}

// r[verify cow.ref-count-after-clone]
// r[verify cow.should-copy]
/// Cloning an exclusive reference makes it shared.
proof fn clone_exclusive_becomes_shared()
    ensures should_copy_spec(ref_count_after_clone_spec(1))
{}

// r[verify cow.ref-count-after-drop]
// r[verify cow.is-exclusive]
/// Dropping from two references makes it exclusive.
proof fn drop_to_exclusive()
    ensures is_exclusive_spec(ref_count_after_drop_spec(2))
{}

// r[verify cow.ref-count-after-drop]
// r[verify cow.can-reclaim]
/// Dropping from exclusive makes it reclaimable.
proof fn drop_exclusive_reclaimable()
    ensures can_reclaim_spec(ref_count_after_drop_spec(1))
{}

// r[verify cow.is-exclusive]
// r[verify cow.can-reclaim]
/// Exclusive, reclaimable, and should_copy are mutually exclusive
/// for any ref count.
proof fn states_mutually_exclusive(ref_count: u64)
    ensures
        !(is_exclusive_spec(ref_count) && can_reclaim_spec(ref_count)),
        !(is_exclusive_spec(ref_count) && should_copy_spec(ref_count)),
        !(can_reclaim_spec(ref_count) && should_copy_spec(ref_count)),
{}

// r[verify cow.is-exclusive]
// r[verify cow.can-reclaim]
// r[verify cow.should-copy]
/// Every valid ref count (> 0) is either exclusive or shared.
/// Zero is reclaimable.
proof fn state_coverage(ref_count: u64)
    ensures
        ref_count == 0 ==> can_reclaim_spec(ref_count),
        ref_count == 1 ==> is_exclusive_spec(ref_count),
        ref_count > 1 ==> should_copy_spec(ref_count),
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify cow.should-copy]
    #[test]
    fn test_should_copy() {
        assert!(!should_copy(1));
        assert!(should_copy(2));
        assert!(should_copy(100));
        assert!(should_copy(u64::MAX));
    }

    // r[verify cow.ref-count-after-clone]
    #[test]
    fn test_ref_count_after_clone() {
        assert_eq!(ref_count_after_clone(1), 2);
        assert_eq!(ref_count_after_clone(2), 3);
        assert_eq!(ref_count_after_clone(u64::MAX), u64::MAX);
    }

    // r[verify cow.ref-count-after-drop]
    #[test]
    fn test_ref_count_after_drop() {
        assert_eq!(ref_count_after_drop(0), 0);
        assert_eq!(ref_count_after_drop(1), 0);
        assert_eq!(ref_count_after_drop(2), 1);
        assert_eq!(ref_count_after_drop(100), 99);
    }

    // r[verify cow.is-exclusive]
    #[test]
    fn test_is_exclusive() {
        assert!(!is_exclusive(0));
        assert!(is_exclusive(1));
        assert!(!is_exclusive(2));
    }

    // r[verify cow.can-reclaim]
    #[test]
    fn test_can_reclaim() {
        assert!(can_reclaim(0));
        assert!(!can_reclaim(1));
        assert!(!can_reclaim(2));
    }

    // r[verify cow.snapshot-isolation]
    #[test]
    fn test_cow_writer_value() {
        assert_eq!(cow_writer_value(10, 20), 20);
        assert_eq!(cow_writer_value(0, 0), 0);
    }

    // r[verify cow.snapshot-isolation]
    #[test]
    fn test_cow_reader_value() {
        // Shared: readers see old value
        assert_eq!(cow_reader_value(10, 20, 2), 10);
        assert_eq!(cow_reader_value(10, 20, 5), 10);
        // Exclusive: no other readers, returns new value
        assert_eq!(cow_reader_value(10, 20, 1), 20);
    }

    proptest! {
        // r[verify cow.should-copy]
        // r[verify cow.is-exclusive]
        #[test]
        fn prop_exclusive_vs_shared(ref_count in 1u64..=u64::MAX) {
            prop_assert_ne!(is_exclusive(ref_count), should_copy(ref_count));
        }

        // r[verify cow.ref-count-after-clone]
        // r[verify cow.ref-count-after-drop]
        #[test]
        fn prop_clone_drop_roundtrip(ref_count in 1u64..u64::MAX) {
            let cloned = ref_count_after_clone(ref_count);
            let dropped = ref_count_after_drop(cloned);
            prop_assert_eq!(dropped, ref_count);
        }

        // r[verify cow.ref-count-after-clone]
        // r[verify cow.ref-count-after-drop]
        #[test]
        fn prop_drop_clone_roundtrip(ref_count in 2u64..=u64::MAX) {
            let dropped = ref_count_after_drop(ref_count);
            let cloned = ref_count_after_clone(dropped);
            prop_assert_eq!(cloned, ref_count);
        }

        // r[verify cow.ref-count-after-clone]
        #[test]
        fn prop_clone_non_decreasing(ref_count in 1u64..=u64::MAX) {
            prop_assert!(ref_count_after_clone(ref_count) >= ref_count);
        }

        // r[verify cow.ref-count-after-drop]
        #[test]
        fn prop_drop_non_increasing(ref_count: u64) {
            prop_assert!(ref_count_after_drop(ref_count) <= ref_count);
        }

        // r[verify cow.snapshot-isolation]
        #[test]
        fn prop_writer_always_sees_new(old_value: u64, new_value: u64) {
            prop_assert_eq!(cow_writer_value(old_value, new_value), new_value);
        }

        // r[verify cow.snapshot-isolation]
        #[test]
        fn prop_shared_readers_see_old(old_value: u64, new_value: u64, ref_count in 2u64..=u64::MAX) {
            prop_assert_eq!(cow_reader_value(old_value, new_value, ref_count), old_value);
        }

        // r[verify cow.snapshot-isolation]
        #[test]
        fn prop_write_same_is_noop(value: u64, ref_count in 1u64..=u64::MAX) {
            prop_assert_eq!(cow_writer_value(value, value), value);
            prop_assert_eq!(cow_reader_value(value, value, ref_count), value);
        }

        // r[verify cow.is-exclusive]
        // r[verify cow.can-reclaim]
        // r[verify cow.should-copy]
        #[test]
        fn prop_states_mutually_exclusive(ref_count: u64) {
            let exc = is_exclusive(ref_count);
            let rec = can_reclaim(ref_count);
            let cop = should_copy(ref_count.max(1)); // should_copy requires > 0
            // At most one is true (for ref_count > 0, exactly one of exclusive/shared)
            prop_assert!(!(exc && rec));
            if ref_count > 0 {
                prop_assert!(!(exc && cop));
            }
            prop_assert!(!(rec && cop));
        }

        // r[verify cow.ref-count-after-clone]
        // r[verify cow.should-copy]
        /// Cloning an exclusive reference always makes it shared.
        #[test]
        fn prop_clone_exclusive_becomes_shared(ref_count in 1u64..u64::MAX) {
            if is_exclusive(ref_count) {
                let cloned = ref_count_after_clone(ref_count);
                prop_assert!(should_copy(cloned));
            }
        }

        // r[verify cow.ref-count-after-drop]
        // r[verify cow.can-reclaim]
        /// Lifecycle: clone N times, drop N+1 times, reach reclaimable.
        #[test]
        fn prop_lifecycle_to_reclaim(n in 0u64..100) {
            let mut rc = 1u64; // start exclusive
            for _ in 0..n {
                rc = ref_count_after_clone(rc);
            }
            // Now rc == n + 1
            for _ in 0..=n {
                rc = ref_count_after_drop(rc);
            }
            prop_assert!(can_reclaim(rc));
        }
    }
}
