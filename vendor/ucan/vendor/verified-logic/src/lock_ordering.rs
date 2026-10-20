//! Verified lock ordering for deadlock prevention.
//!
//! Models lock acquisition order as a strict total order on lock IDs (u64).
//! Threads must acquire locks in ascending ID order. `would_deadlock`
//! detects out-of-order requests, and `is_valid_acquisition_order` proves
//! that a sequence of lock IDs is strictly ascending — which prevents
//! cycles in the wait-for graph.

use vstd::prelude::*;

verus! {

// --- is_ordered_pair ---

// r[depends lock_ordering.ordered-pair]
/// Two lock IDs are in valid acquisition order when the first is strictly
/// less than the second.
pub open spec fn is_ordered_pair_spec(first: u64, second: u64) -> bool {
    first < second
}

// r[impl lock_ordering.ordered-pair]
pub fn is_ordered_pair(first: u64, second: u64) -> (result: bool)
    ensures result == is_ordered_pair_spec(first, second)
{
    first < second
}

// --- would_deadlock ---

// r[depends lock_ordering.would-deadlock]
/// A deadlock risk exists when a thread holding `held_max` (the highest
/// lock ID it currently holds) requests a lock with ID <= held_max.
pub open spec fn would_deadlock_spec(held_max: u64, requested: u64) -> bool {
    requested <= held_max
}

// r[impl lock_ordering.would-deadlock]
pub fn would_deadlock(held_max: u64, requested: u64) -> (result: bool)
    ensures result == would_deadlock_spec(held_max, requested)
{
    requested <= held_max
}

// --- can_acquire ---

// r[depends lock_ordering.can-acquire]
/// A thread can acquire `requested` only if it exceeds `held_max`.
/// When no locks are held, `held_max` is 0 and `has_locks` is false,
/// allowing any lock to be acquired.
pub open spec fn can_acquire_spec(held_max: u64, has_locks: bool, requested: u64) -> bool {
    !has_locks || requested > held_max
}

// r[impl lock_ordering.can-acquire]
pub fn can_acquire(held_max: u64, has_locks: bool, requested: u64) -> (result: bool)
    ensures result == can_acquire_spec(held_max, has_locks, requested)
{
    !has_locks || requested > held_max
}

// --- acquisition_order ---

// r[depends lock_ordering.acquisition-order]
/// Returns the pair (first_to_acquire, second_to_acquire) given two
/// distinct lock IDs. The lower ID must be acquired first.
pub open spec fn acquisition_first_spec(a: u64, b: u64) -> u64 {
    if a < b { a } else { b }
}

// r[depends lock_ordering.acquisition-order]
pub open spec fn acquisition_second_spec(a: u64, b: u64) -> u64 {
    if a < b { b } else { a }
}

// r[impl lock_ordering.acquisition-order]
pub fn acquisition_first(a: u64, b: u64) -> (result: u64)
    ensures result == acquisition_first_spec(a, b)
{
    if a < b { a } else { b }
}

// r[impl lock_ordering.acquisition-order]
pub fn acquisition_second(a: u64, b: u64) -> (result: u64)
    ensures result == acquisition_second_spec(a, b)
{
    if a < b { b } else { a }
}

// --- is_valid_acquisition_order ---

// r[depends lock_ordering.valid-order]
/// A sequence of lock IDs is in valid acquisition order when it is
/// strictly ascending: each element is less than the next.
pub open spec fn is_valid_acquisition_order_spec(locks: Seq<u64>) -> bool {
    forall|i: int| #![trigger locks[i]] 0 <= i < locks.len() as int - 1
        ==> locks[i] < locks[i + 1]
}

// r[impl lock_ordering.valid-order]
pub fn is_valid_acquisition_order(locks: &Vec<u64>) -> (result: bool)
    ensures result == is_valid_acquisition_order_spec(locks@)
{
    if locks.len() <= 1 {
        return true;
    }
    let mut i: usize = 0;
    while i < locks.len() - 1
        invariant
            0 <= i <= locks.len() - 1,
            forall|j: int| #![trigger locks@[j]] 0 <= j < i as int
                ==> locks@[j] < locks@[j + 1],
        decreases locks.len() - 1 - i,
    {
        if locks[i] >= locks[i + 1] {
            // Found violation — all checked pairs were ok but this one fails
            assert(!is_valid_acquisition_order_spec(locks@));
            return false;
        }
        i += 1;
    }
    true
}

// --- max_held_lock ---

// r[depends lock_ordering.max-held]
/// The maximum lock ID in a non-empty valid-order sequence is the last element.
pub open spec fn max_held_lock_spec(locks: Seq<u64>) -> u64
    recommends locks.len() > 0
{
    locks[locks.len() - 1]
}

// r[impl lock_ordering.max-held]
pub fn max_held_lock(locks: &Vec<u64>) -> (result: u64)
    requires locks.len() > 0
    ensures result == max_held_lock_spec(locks@)
{
    locks[locks.len() - 1]
}

// --- lock_count ---

// r[depends lock_ordering.lock-count]
/// Number of locks held.
pub open spec fn lock_count_spec(locks: Seq<u64>) -> nat {
    locks.len()
}

// r[impl lock_ordering.lock-count]
pub fn lock_count(locks: &Vec<u64>) -> (result: usize)
    ensures result as nat == lock_count_spec(locks@)
{
    locks.len()
}

// === Proofs ===

// r[verify lock_ordering.ordered-pair]
/// Ordered pair is irreflexive: no lock can precede itself.
proof fn ordered_pair_irreflexive(a: u64)
    ensures !is_ordered_pair_spec(a, a)
{}

// r[verify lock_ordering.ordered-pair]
/// Ordered pair is asymmetric: if a < b then NOT b < a.
proof fn ordered_pair_asymmetric(a: u64, b: u64)
    requires is_ordered_pair_spec(a, b)
    ensures !is_ordered_pair_spec(b, a)
{}

// r[verify lock_ordering.ordered-pair]
/// Ordered pair is transitive.
proof fn ordered_pair_transitive(a: u64, b: u64, c: u64)
    requires is_ordered_pair_spec(a, b), is_ordered_pair_spec(b, c)
    ensures is_ordered_pair_spec(a, c)
{}

// r[verify lock_ordering.would-deadlock]
/// would_deadlock is the negation of ordered pair when locks are held.
proof fn deadlock_iff_not_ordered(held_max: u64, requested: u64)
    ensures would_deadlock_spec(held_max, requested) ==
        (requested == held_max || !is_ordered_pair_spec(held_max, requested))
{}

// r[verify lock_ordering.can-acquire]
/// can_acquire with no locks held always succeeds.
proof fn no_locks_always_acquirable(held_max: u64, requested: u64)
    ensures can_acquire_spec(held_max, false, requested)
{}

// r[verify lock_ordering.can-acquire]
/// can_acquire is exactly the negation of would_deadlock when locks are held.
proof fn acquire_negates_deadlock(held_max: u64, requested: u64)
    ensures can_acquire_spec(held_max, true, requested) ==
        !would_deadlock_spec(held_max, requested)
{}

// r[verify lock_ordering.acquisition-order]
/// Acquisition order produces a valid pair when inputs differ.
proof fn acquisition_order_valid(a: u64, b: u64)
    requires a != b
    ensures is_ordered_pair_spec(
        acquisition_first_spec(a, b),
        acquisition_second_spec(a, b),
    )
{}

// r[verify lock_ordering.acquisition-order]
/// Acquisition order covers both inputs.
proof fn acquisition_order_covers(a: u64, b: u64)
    ensures
        (acquisition_first_spec(a, b) == a && acquisition_second_spec(a, b) == b) ||
        (acquisition_first_spec(a, b) == b && acquisition_second_spec(a, b) == a)
{}

// r[verify lock_ordering.acquisition-order]
/// Acquisition order is commutative: order(a,b) == order(b,a).
proof fn acquisition_order_commutative(a: u64, b: u64)
    ensures
        acquisition_first_spec(a, b) == acquisition_first_spec(b, a),
        acquisition_second_spec(a, b) == acquisition_second_spec(b, a),
{}

// r[verify lock_ordering.valid-order]
/// Empty sequence is valid.
proof fn empty_is_valid()
    ensures is_valid_acquisition_order_spec(Seq::<u64>::empty())
{}

// r[verify lock_ordering.valid-order]
/// Singleton sequence is valid.
proof fn singleton_is_valid(lock: u64)
    ensures is_valid_acquisition_order_spec(seq![lock])
{}

// r[verify lock_ordering.valid-order]
/// In a valid order, all elements are distinct.
proof fn valid_order_implies_distinct(locks: Seq<u64>)
    requires is_valid_acquisition_order_spec(locks)
    ensures forall|i: int, j: int|
        #![trigger locks[i], locks[j]]
        0 <= i < j < locks.len() as int ==> locks[i] != locks[j]
    decreases locks.len()
{
    // Each consecutive pair has strict <, so by transitivity all are distinct
    if locks.len() > 1 {
        assert forall|i: int, j: int|
            #![trigger locks[i], locks[j]]
            0 <= i < j < locks.len() as int
        implies locks[i] != locks[j] by {
            // Induction: i < i+1 < ... < j gives locks[i] < locks[j]
            valid_order_chain(locks, i, j);
        }
    }
}

/// Helper: in a valid ascending sequence, locks[i] < locks[j] for i < j.
proof fn valid_order_chain(locks: Seq<u64>, i: int, j: int)
    requires
        is_valid_acquisition_order_spec(locks),
        0 <= i < j,
        j < locks.len() as int,
    ensures locks[i] < locks[j]
    decreases j - i
{
    if j - i == 1 {
        // Base: direct from spec
    } else {
        // Inductive: locks[i] < locks[j-1] < locks[j]
        valid_order_chain(locks, i, j - 1);
    }
}

// r[verify lock_ordering.valid-order]
// r[verify lock_ordering.max-held]
/// In a valid order, the last element is the maximum.
proof fn last_is_max(locks: Seq<u64>)
    requires
        is_valid_acquisition_order_spec(locks),
        locks.len() > 0,
    ensures forall|i: int| #![trigger locks[i]]
        0 <= i < locks.len() as int ==> locks[i] <= max_held_lock_spec(locks)
{
    let last = locks.len() as int - 1;
    assert forall|i: int| #![trigger locks[i]]
        0 <= i < locks.len() as int
    implies locks[i] <= max_held_lock_spec(locks) by {
        if i < last {
            valid_order_chain(locks, i, last);
        }
    }
}

// r[verify lock_ordering.valid-order]
// r[verify lock_ordering.can-acquire]
/// Acquiring in valid order means each lock can be acquired given the
/// previous max.
proof fn valid_order_means_acquirable(locks: Seq<u64>)
    requires
        is_valid_acquisition_order_spec(locks),
        locks.len() >= 2,
    ensures forall|i: int| #![trigger locks[i]]
        1 <= i < locks.len() as int ==>
        can_acquire_spec(locks[i - 1], true, locks[i])
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify lock_ordering.ordered-pair]
    #[test]
    fn test_ordered_pair() {
        assert!(is_ordered_pair(1, 2));
        assert!(!is_ordered_pair(2, 1));
        assert!(!is_ordered_pair(5, 5));
        assert!(is_ordered_pair(0, u64::MAX));
    }

    // r[verify lock_ordering.would-deadlock]
    #[test]
    fn test_would_deadlock() {
        assert!(would_deadlock(5, 3));
        assert!(would_deadlock(5, 5));
        assert!(!would_deadlock(5, 6));
        assert!(!would_deadlock(0, 1));
    }

    // r[verify lock_ordering.can-acquire]
    #[test]
    fn test_can_acquire() {
        assert!(can_acquire(0, false, 0)); // no locks held
        assert!(can_acquire(5, false, 1)); // no locks held
        assert!(can_acquire(5, true, 6));
        assert!(!can_acquire(5, true, 5));
        assert!(!can_acquire(5, true, 3));
    }

    // r[verify lock_ordering.acquisition-order]
    #[test]
    fn test_acquisition_order() {
        assert_eq!(acquisition_first(10, 20), 10);
        assert_eq!(acquisition_second(10, 20), 20);
        assert_eq!(acquisition_first(20, 10), 10);
        assert_eq!(acquisition_second(20, 10), 20);
    }

    // r[verify lock_ordering.valid-order]
    #[test]
    fn test_valid_acquisition_order() {
        let empty: Vec<u64> = Vec::new();
        assert!(is_valid_acquisition_order(&empty));

        let single = vec![42u64];
        assert!(is_valid_acquisition_order(&single));

        let ascending = vec![1u64, 3, 7, 15];
        assert!(is_valid_acquisition_order(&ascending));

        let descending = vec![15u64, 7, 3, 1];
        assert!(!is_valid_acquisition_order(&descending));

        let duplicate = vec![5u64, 5];
        assert!(!is_valid_acquisition_order(&duplicate));
    }

    // r[verify lock_ordering.max-held]
    #[test]
    fn test_max_held_lock() {
        let locks = vec![1u64, 5, 10];
        assert_eq!(max_held_lock(&locks), 10);
    }

    // r[verify lock_ordering.lock-count]
    #[test]
    fn test_lock_count() {
        let empty: Vec<u64> = Vec::new();
        assert_eq!(lock_count(&empty), 0);
        let locks = vec![1u64, 2, 3];
        assert_eq!(lock_count(&locks), 3);
    }

    proptest! {
        // r[verify lock_ordering.ordered-pair]
        #[test]
        fn prop_ordered_pair_irreflexive(a: u64) {
            prop_assert!(!is_ordered_pair(a, a));
        }

        // r[verify lock_ordering.ordered-pair]
        #[test]
        fn prop_ordered_pair_asymmetric(a: u64, b: u64) {
            if is_ordered_pair(a, b) {
                prop_assert!(!is_ordered_pair(b, a));
            }
        }

        // r[verify lock_ordering.ordered-pair]
        #[test]
        fn prop_ordered_pair_trichotomy(a: u64, b: u64) {
            // exactly one of: a < b, a == b, b < a
            let ab = is_ordered_pair(a, b);
            let ba = is_ordered_pair(b, a);
            let eq = a == b;
            prop_assert!(
                (ab && !ba && !eq) || (!ab && ba && !eq) || (!ab && !ba && eq)
            );
        }

        // r[verify lock_ordering.would-deadlock]
        // r[verify lock_ordering.can-acquire]
        #[test]
        fn prop_deadlock_negates_acquire(held_max: u64, requested: u64) {
            prop_assert_eq!(
                would_deadlock(held_max, requested),
                !can_acquire(held_max, true, requested)
            );
        }

        // r[verify lock_ordering.can-acquire]
        #[test]
        fn prop_no_locks_always_acquirable(held_max: u64, requested: u64) {
            prop_assert!(can_acquire(held_max, false, requested));
        }

        // r[verify lock_ordering.acquisition-order]
        #[test]
        fn prop_acquisition_order_commutative(a: u64, b: u64) {
            prop_assert_eq!(acquisition_first(a, b), acquisition_first(b, a));
            prop_assert_eq!(acquisition_second(a, b), acquisition_second(b, a));
        }

        // r[verify lock_ordering.acquisition-order]
        #[test]
        fn prop_acquisition_order_valid_when_distinct(a: u64, b: u64) {
            if a != b {
                prop_assert!(is_ordered_pair(
                    acquisition_first(a, b),
                    acquisition_second(a, b)
                ));
            }
        }

        // r[verify lock_ordering.acquisition-order]
        #[test]
        fn prop_acquisition_order_covers(a: u64, b: u64) {
            let first = acquisition_first(a, b);
            let second = acquisition_second(a, b);
            prop_assert!(
                (first == a && second == b) || (first == b && second == a)
            );
        }

        // r[verify lock_ordering.valid-order]
        #[test]
        fn prop_sorted_vec_valid_order(mut vals in proptest::collection::vec(0u64..1000, 0..8)) {
            vals.sort();
            vals.dedup();
            prop_assert!(is_valid_acquisition_order(&vals));
        }

        // r[verify lock_ordering.valid-order]
        // r[verify lock_ordering.max-held]
        #[test]
        fn prop_max_is_last_in_valid_order(mut vals in proptest::collection::vec(0u64..1000, 1..8)) {
            vals.sort();
            vals.dedup();
            let max = max_held_lock(&vals);
            for v in &vals {
                prop_assert!(*v <= max);
            }
        }
    }
}
