//! Verified reference counting primitives.
//!
//! Models strong and weak reference counts for shared ownership.
//! Increment/decrement with saturation, zero-triggers-dealloc,
//! uniqueness detection, and weak-to-strong upgrade conditions.

use vstd::prelude::*;

verus! {

// --- increment ---

// r[depends ref_count.increment]
/// Increment a reference count. Saturates at u64::MAX.
pub open spec fn increment_spec(count: u64) -> u64 {
    if count < u64::MAX { (count + 1) as u64 } else { u64::MAX }
}

// r[impl ref_count.increment]
pub fn increment(count: u64) -> (result: u64)
    requires count < u64::MAX
    ensures result == increment_spec(count)
{
    count + 1
}

// --- decrement ---

// r[depends ref_count.decrement]
/// Decrement a reference count. Floors at 0.
pub open spec fn decrement_spec(count: u64) -> u64 {
    if count > 0 { (count - 1) as u64 } else { 0 }
}

// r[impl ref_count.decrement]
pub fn decrement(count: u64) -> (result: u64)
    requires count > 0
    ensures result == decrement_spec(count)
{
    count - 1
}

// --- should_deallocate ---

// r[depends ref_count.should-deallocate]
/// The object should be deallocated when the strong count reaches zero.
pub open spec fn should_deallocate_spec(strong_count: u64) -> bool {
    strong_count == 0
}

// r[impl ref_count.should-deallocate]
pub fn should_deallocate(strong_count: u64) -> (result: bool)
    ensures result == should_deallocate_spec(strong_count)
{
    strong_count == 0
}

// --- is_unique ---

// r[depends ref_count.is-unique]
/// The reference is unique when the strong count is exactly 1.
pub open spec fn is_unique_spec(strong_count: u64) -> bool {
    strong_count == 1
}

// r[impl ref_count.is-unique]
pub fn is_unique(strong_count: u64) -> (result: bool)
    ensures result == is_unique_spec(strong_count)
{
    strong_count == 1
}

// --- can_upgrade_weak ---

// r[depends ref_count.can-upgrade-weak]
/// A weak reference can be upgraded to strong when the strong count > 0.
/// If strong count is 0, the object is (being) deallocated.
pub open spec fn can_upgrade_weak_spec(strong_count: u64) -> bool {
    strong_count > 0
}

// r[impl ref_count.can-upgrade-weak]
pub fn can_upgrade_weak(strong_count: u64) -> (result: bool)
    ensures result == can_upgrade_weak_spec(strong_count)
{
    strong_count > 0
}

// --- should_drop_weak_data ---

// r[depends ref_count.should-drop-weak-data]
/// The weak allocation (control block) should be freed when both
/// strong and weak counts are zero.
pub open spec fn should_drop_weak_data_spec(strong_count: u64, weak_count: u64) -> bool {
    strong_count == 0 && weak_count == 0
}

// r[impl ref_count.should-drop-weak-data]
pub fn should_drop_weak_data(strong_count: u64, weak_count: u64) -> (result: bool)
    ensures result == should_drop_weak_data_spec(strong_count, weak_count)
{
    strong_count == 0 && weak_count == 0
}

// --- total_refs ---

// r[depends ref_count.total-refs]
/// Total reference count (strong + weak). Saturates at u64::MAX.
pub open spec fn total_refs_spec(strong_count: u64, weak_count: u64) -> u64 {
    if (strong_count as int) + (weak_count as int) > (u64::MAX as int) {
        u64::MAX
    } else {
        (strong_count + weak_count) as u64
    }
}

// r[impl ref_count.total-refs]
pub fn total_refs(strong_count: u64, weak_count: u64) -> (result: u64)
    ensures result == total_refs_spec(strong_count, weak_count)
{
    if strong_count > u64::MAX - weak_count {
        u64::MAX
    } else {
        strong_count + weak_count
    }
}

// === Proofs ===

// r[verify ref_count.increment]
/// Increment is monotonic.
proof fn increment_monotonic(count: u64)
    ensures increment_spec(count) >= count
{}

// r[verify ref_count.increment]
/// Increment from non-max strictly increases.
proof fn increment_strict(count: u64)
    requires count < u64::MAX
    ensures increment_spec(count) > count
{}

// r[verify ref_count.decrement]
/// Decrement is monotonic downward.
proof fn decrement_monotonic(count: u64)
    ensures decrement_spec(count) <= count
{}

// r[verify ref_count.decrement]
/// Decrement from positive strictly decreases.
proof fn decrement_strict(count: u64)
    requires count > 0
    ensures decrement_spec(count) < count
{}

// r[verify ref_count.increment]
// r[verify ref_count.decrement]
/// Increment then decrement is identity (non-saturated case).
proof fn inc_dec_roundtrip(count: u64)
    requires count < u64::MAX
    ensures decrement_spec(increment_spec(count)) == count
{}

// r[verify ref_count.increment]
// r[verify ref_count.decrement]
/// Decrement then increment is identity (positive case).
proof fn dec_inc_roundtrip(count: u64)
    requires count > 0, count < u64::MAX
    ensures increment_spec(decrement_spec(count)) == count
{}

// r[verify ref_count.decrement]
// r[verify ref_count.should-deallocate]
/// Decrement from 1 triggers deallocation.
proof fn last_decrement_deallocates()
    ensures should_deallocate_spec(decrement_spec(1))
{}

// r[verify ref_count.decrement]
// r[verify ref_count.should-deallocate]
/// Decrement from n > 1 does not trigger deallocation.
proof fn non_last_decrement_keeps(count: u64)
    requires count > 1
    ensures !should_deallocate_spec(decrement_spec(count))
{}

// r[verify ref_count.is-unique]
// r[verify ref_count.decrement]
/// Decrementing a unique reference triggers deallocation.
proof fn unique_decrement_deallocates(count: u64)
    requires is_unique_spec(count)
    ensures should_deallocate_spec(decrement_spec(count))
{}

// r[verify ref_count.is-unique]
// r[verify ref_count.can-upgrade-weak]
/// Unique implies upgrade is possible (strong > 0).
proof fn unique_can_upgrade(count: u64)
    requires is_unique_spec(count)
    ensures can_upgrade_weak_spec(count)
{}

// r[verify ref_count.should-deallocate]
// r[verify ref_count.can-upgrade-weak]
/// Deallocate and upgrade are mutually exclusive.
proof fn deallocate_upgrade_exclusive(count: u64)
    ensures !(should_deallocate_spec(count) && can_upgrade_weak_spec(count))
{}

// r[verify ref_count.should-drop-weak-data]
// r[verify ref_count.should-deallocate]
/// drop_weak_data implies should_deallocate.
proof fn weak_drop_implies_dealloc(strong: u64, weak: u64)
    requires should_drop_weak_data_spec(strong, weak)
    ensures should_deallocate_spec(strong)
{}

// r[verify ref_count.total-refs]
/// Total refs with zero weak equals strong.
proof fn total_zero_weak(strong: u64)
    ensures total_refs_spec(strong, 0) == strong
{}

// r[verify ref_count.total-refs]
/// Total refs with zero strong equals weak.
proof fn total_zero_strong(weak: u64)
    ensures total_refs_spec(0, weak) == weak
{}

// r[verify ref_count.total-refs]
/// Total refs >= each component.
proof fn total_ge_strong(strong: u64, weak: u64)
    ensures total_refs_spec(strong, weak) >= strong
{}

// r[verify ref_count.total-refs]
proof fn total_ge_weak(strong: u64, weak: u64)
    ensures total_refs_spec(strong, weak) >= weak
{}

// r[verify ref_count.increment]
// r[verify ref_count.should-deallocate]
/// Incrementing from any count means not deallocating.
proof fn increment_prevents_dealloc(count: u64)
    requires count < u64::MAX
    ensures !should_deallocate_spec(increment_spec(count))
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify ref_count.increment]
    #[test]
    fn test_increment() {
        assert_eq!(increment(0), 1);
        assert_eq!(increment(5), 6);
        assert_eq!(increment(u64::MAX - 1), u64::MAX);
    }

    // r[verify ref_count.decrement]
    #[test]
    fn test_decrement() {
        assert_eq!(decrement(1), 0);
        assert_eq!(decrement(5), 4);
        assert_eq!(decrement(u64::MAX), u64::MAX - 1);
    }

    // r[verify ref_count.should-deallocate]
    #[test]
    fn test_should_deallocate() {
        assert!(should_deallocate(0));
        assert!(!should_deallocate(1));
        assert!(!should_deallocate(100));
    }

    // r[verify ref_count.is-unique]
    #[test]
    fn test_is_unique() {
        assert!(!is_unique(0));
        assert!(is_unique(1));
        assert!(!is_unique(2));
    }

    // r[verify ref_count.can-upgrade-weak]
    #[test]
    fn test_can_upgrade_weak() {
        assert!(!can_upgrade_weak(0));
        assert!(can_upgrade_weak(1));
        assert!(can_upgrade_weak(100));
    }

    // r[verify ref_count.should-drop-weak-data]
    #[test]
    fn test_should_drop_weak_data() {
        assert!(should_drop_weak_data(0, 0));
        assert!(!should_drop_weak_data(0, 1));
        assert!(!should_drop_weak_data(1, 0));
        assert!(!should_drop_weak_data(1, 1));
    }

    // r[verify ref_count.total-refs]
    #[test]
    fn test_total_refs() {
        assert_eq!(total_refs(3, 2), 5);
        assert_eq!(total_refs(0, 0), 0);
        assert_eq!(total_refs(u64::MAX, 1), u64::MAX);
        assert_eq!(total_refs(u64::MAX, u64::MAX), u64::MAX);
    }

    proptest! {
        // r[verify ref_count.increment]
        // r[verify ref_count.decrement]
        #[test]
        fn prop_inc_dec_roundtrip(count in 0u64..u64::MAX) {
            let inc = increment(count);
            let dec = decrement(inc);
            prop_assert_eq!(dec, count);
        }

        // r[verify ref_count.increment]
        // r[verify ref_count.decrement]
        #[test]
        fn prop_dec_inc_roundtrip(count in 1u64..u64::MAX) {
            let dec = decrement(count);
            let inc = increment(dec);
            prop_assert_eq!(inc, count);
        }

        // r[verify ref_count.decrement]
        // r[verify ref_count.should-deallocate]
        #[test]
        fn prop_last_dec_deallocates(count in 1u64..=1u64) {
            prop_assert!(should_deallocate(decrement(count)));
        }

        // r[verify ref_count.decrement]
        // r[verify ref_count.should-deallocate]
        #[test]
        fn prop_non_last_dec_no_dealloc(count in 2u64..=u64::MAX) {
            prop_assert!(!should_deallocate(decrement(count)));
        }

        // r[verify ref_count.should-deallocate]
        // r[verify ref_count.can-upgrade-weak]
        #[test]
        fn prop_dealloc_upgrade_exclusive(count: u64) {
            prop_assert!(!(should_deallocate(count) && can_upgrade_weak(count)));
        }

        // r[verify ref_count.is-unique]
        // r[verify ref_count.can-upgrade-weak]
        #[test]
        fn prop_unique_can_upgrade(count: u64) {
            if is_unique(count) {
                prop_assert!(can_upgrade_weak(count));
            }
        }

        // r[verify ref_count.total-refs]
        #[test]
        fn prop_total_ge_components(strong: u64, weak: u64) {
            let total = total_refs(strong, weak);
            prop_assert!(total >= strong);
            prop_assert!(total >= weak);
        }

        // r[verify ref_count.increment]
        // r[verify ref_count.should-deallocate]
        #[test]
        fn prop_increment_prevents_dealloc(count in 0u64..u64::MAX) {
            prop_assert!(!should_deallocate(increment(count)));
        }

        // r[verify ref_count.should-drop-weak-data]
        #[test]
        fn prop_drop_weak_needs_both_zero(strong: u64, weak: u64) {
            prop_assert_eq!(
                should_drop_weak_data(strong, weak),
                strong == 0 && weak == 0
            );
        }
    }
}
