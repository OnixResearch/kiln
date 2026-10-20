//! Verified surelock level ordering.
//!
//! Levels use plain `u64` values. Ordering is total and strict via ordinary
//! integer comparison. Higher-level derivation mirrors surelock's `new_higher`
//! rule: one parent adds one, two parents add one above the greater parent.

use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::ordering::max_spec;

verus! {

// r[depends surelock-level-order.total-order]
pub open spec fn compare_levels_spec(a: u64, b: u64) -> int {
    if a < b {
        -1int
    } else if a == b {
        0int
    } else {
        1int
    }
}

// r[impl surelock-level-order.total-order]
pub fn compare_levels(a: u64, b: u64) -> (result: i8)
    ensures result as int == compare_levels_spec(a, b)
{
    if a < b {
        -1
    } else if a == b {
        0
    } else {
        1
    }
}

// r[depends surelock-level-order.lock-after]
pub open spec fn lock_after_spec(current: u64, next: u64) -> bool {
    current <= next
}

// r[impl surelock-level-order.lock-after]
pub fn lock_after(current: u64, next: u64) -> (result: bool)
    ensures result == lock_after_spec(current, next)
{
    current <= next
}

// r[depends surelock-level-order.derived-higher]
pub open spec fn new_higher_from_spec(parent: u64) -> Option<u64> {
    if parent == u64::MAX {
        None
    } else {
        Some((parent + 1) as u64)
    }
}

// r[impl surelock-level-order.derived-higher]
pub fn new_higher_from(parent: u64) -> (result: Option<u64>)
    ensures result == new_higher_from_spec(parent)
{
    if parent == u64::MAX {
        None
    } else {
        Some(parent + 1)
    }
}

// r[depends surelock-level-order.derived-higher]
pub open spec fn new_higher_pair_spec(parent_a: u64, parent_b: u64) -> Option<u64> {
    let high = max_spec(parent_a, parent_b);
    if high == u64::MAX {
        None
    } else {
        Some((high + 1) as u64)
    }
}

// r[impl surelock-level-order.derived-higher]
pub fn new_higher_pair(parent_a: u64, parent_b: u64) -> (result: Option<u64>)
    ensures result == new_higher_pair_spec(parent_a, parent_b)
{
    let high = if parent_a >= parent_b { parent_a } else { parent_b };
    if high == u64::MAX {
        None
    } else {
        Some(high + 1)
    }
}

// r[verify surelock-level-order.total-order]
pub proof fn levels_trichotomy(a: u64, b: u64)
    ensures
        (compare_levels_spec(a, b) == -1) <==> (a < b),
        (compare_levels_spec(a, b) == 0) <==> (a == b),
        (compare_levels_spec(a, b) == 1) <==> (b < a),
{}

// r[verify surelock-level-order.total-order]
pub proof fn levels_exactly_one_relation(a: u64, b: u64)
    ensures
        (a < b || a == b || b < a),
        !(a < b && a == b),
        !(a < b && b < a),
        !(a == b && b < a),
{}

// r[verify surelock-level-order.lock-after]
pub proof fn upward_acquisition_allowed(current: u64, next: u64)
    requires current <= next,
    ensures lock_after_spec(current, next)
{}

// r[verify surelock-level-order.lock-after]
pub proof fn backward_acquisition_rejected(current: u64, next: u64)
    requires current > next,
    ensures !lock_after_spec(current, next)
{}

// r[verify surelock-level-order.derived-higher]
pub proof fn new_higher_from_exact(parent: u64)
    requires parent < u64::MAX,
    ensures new_higher_from_spec(parent).unwrap() == parent + 1
{}

// r[verify surelock-level-order.derived-higher]
pub proof fn new_higher_pair_exact(parent_a: u64, parent_b: u64)
    requires max_spec(parent_a, parent_b) < u64::MAX,
    ensures new_higher_pair_spec(parent_a, parent_b).unwrap() == max_spec(parent_a, parent_b) + 1
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify surelock-level-order.total-order]
    #[test]
    fn test_compare_levels() {
        assert_eq!(compare_levels(1, 3), -1);
        assert_eq!(compare_levels(2, 2), 0);
        assert_eq!(compare_levels(4, 1), 1);
    }

    // r[verify surelock-level-order.lock-after]
    #[test]
    fn test_lock_after() {
        assert!(lock_after(1, 1));
        assert!(lock_after(1, 3));
        assert!(!lock_after(3, 1));
    }

    // r[verify surelock-level-order.derived-higher]
    #[test]
    fn test_new_higher() {
        assert_eq!(new_higher_from(2), Some(3));
        assert_eq!(new_higher_pair(1, 4), Some(5));
        assert_eq!(new_higher_from(u64::MAX), None);
        assert_eq!(new_higher_pair(u64::MAX, 7), None);
    }

    proptest! {
        // r[verify surelock-level-order.total-order]
        #[test]
        fn prop_compare_levels_matches_order(a: u64, b: u64) {
            let cmp = compare_levels(a, b);
            prop_assert_eq!(cmp == -1, a < b);
            prop_assert_eq!(cmp == 0, a == b);
            prop_assert_eq!(cmp == 1, a > b);
        }

        // r[verify surelock-level-order.lock-after]
        #[test]
        fn prop_lock_after_matches_leq(a: u64, b: u64) {
            prop_assert_eq!(lock_after(a, b), a <= b);
        }

        // r[verify surelock-level-order.derived-higher]
        #[test]
        fn prop_new_higher_from_exact(parent in 0u64..u64::MAX) {
            prop_assert_eq!(new_higher_from(parent), Some(parent + 1));
        }

        // r[verify surelock-level-order.derived-higher]
        #[test]
        fn prop_new_higher_pair_exact(a in 0u64..u64::MAX, b in 0u64..u64::MAX) {
            prop_assert_eq!(new_higher_pair(a, b), Some(a.max(b) + 1));
        }
    }
}
