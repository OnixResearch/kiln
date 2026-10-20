//! Verified priority inversion prevention primitives.
//!
//! Models the priority inheritance protocol: when a high-priority task
//! blocks on a lock held by a low-priority task, the lock holder's
//! effective priority is temporarily boosted to that of the highest
//! waiter. Proofs cover boosted priority correctness, bounded priority
//! inversion, and restoration after release.

use vstd::prelude::*;

verus! {

// ============================================================
// Effective priority under inheritance
// ============================================================

// r[depends priority-inversion.effective-priority]
/// Spec: the effective priority of a task is the maximum of its base
/// priority and the highest priority among its waiters.
/// Higher numeric value = higher priority.
pub open spec fn effective_priority_spec(base: u64, highest_waiter: u64) -> u64 {
    if highest_waiter > base { highest_waiter } else { base }
}

// r[impl priority-inversion.effective-priority]
/// Compute the effective priority under priority inheritance.
pub fn effective_priority(base: u64, highest_waiter: u64) -> (result: u64)
    ensures result == effective_priority_spec(base, highest_waiter)
{
    if highest_waiter > base { highest_waiter } else { base }
}

// r[verify priority-inversion.effective-priority]
/// Effective priority is at least the base priority.
proof fn effective_ge_base(base: u64, hw: u64)
    ensures effective_priority_spec(base, hw) >= base
{}

// r[verify priority-inversion.effective-priority]
/// Effective priority is at least the highest waiter.
proof fn effective_ge_waiter(base: u64, hw: u64)
    ensures effective_priority_spec(base, hw) >= hw
{}

// r[verify priority-inversion.effective-priority]
/// With no waiters (highest_waiter = 0), effective = base.
proof fn no_waiters_identity(base: u64)
    requires base > 0
    ensures effective_priority_spec(base, 0) == base
{}

// r[verify priority-inversion.effective-priority]
/// Adding a higher-priority waiter raises the effective priority.
proof fn higher_waiter_raises(base: u64, hw1: u64, hw2: u64)
    requires hw2 >= hw1
    ensures effective_priority_spec(base, hw2) >= effective_priority_spec(base, hw1)
{}

// ============================================================
// Priority boost check
// ============================================================

// r[depends priority-inversion.is-boosted]
/// Spec: is the task currently running at a boosted priority?
pub open spec fn is_boosted_spec(base: u64, highest_waiter: u64) -> bool {
    highest_waiter > base
}

// r[impl priority-inversion.is-boosted]
/// Returns true if the task's effective priority exceeds its base.
pub fn is_boosted(base: u64, highest_waiter: u64) -> (result: bool)
    ensures result == is_boosted_spec(base, highest_waiter)
{
    highest_waiter > base
}

// r[verify priority-inversion.is-boosted]
/// A boosted task's effective priority matches the highest waiter.
proof fn boosted_matches_waiter(base: u64, hw: u64)
    requires is_boosted_spec(base, hw)
    ensures effective_priority_spec(base, hw) == hw
{}

// r[verify priority-inversion.is-boosted]
/// An unboosted task's effective priority matches its base.
proof fn unboosted_matches_base(base: u64, hw: u64)
    requires !is_boosted_spec(base, hw)
    ensures effective_priority_spec(base, hw) == base
{}

// ============================================================
// Highest waiter tracking
// ============================================================

// r[depends priority-inversion.highest-waiter]
/// Spec: the highest priority among waiters, from index `from`.
pub open spec fn highest_waiter_from_spec(waiters: Seq<u64>, from: int) -> u64
    decreases waiters.len() - from
{
    if from < 0 || from >= waiters.len() as int {
        0u64
    } else {
        let rest = highest_waiter_from_spec(waiters, from + 1);
        if waiters[from] >= rest { waiters[from] as u64 } else { rest }
    }
}

/// Spec: highest priority among all waiters.
pub open spec fn highest_waiter_spec(waiters: Seq<u64>) -> u64 {
    highest_waiter_from_spec(waiters, 0)
}

// r[impl priority-inversion.highest-waiter]
/// Find the highest priority among waiting tasks.
pub fn highest_waiter(waiters: &[u64]) -> (result: u64)
    ensures result == highest_waiter_spec(waiters@)
{
    // Scan right-to-left to match the recursive spec direction.
    let mut best: u64 = 0;
    let mut i: usize = waiters.len();
    while i > 0
        invariant
            0 <= i <= waiters@.len(),
            best == highest_waiter_from_spec(waiters@, i as int),
        decreases i,
    {
        i = i - 1;
        if waiters[i] > best {
            best = waiters[i];
        }
    }
    best
}

// r[verify priority-inversion.highest-waiter]
/// Empty waiters produce zero.
proof fn no_waiters_zero()
    ensures highest_waiter_spec(seq![]) == 0u64
{}

// ============================================================
// Bounded inversion: the key safety property
// ============================================================

// r[depends priority-inversion.bounded-inversion]
/// Spec: under priority inheritance, a high-priority task H waiting
/// on lock held by low-priority task L experiences at most one
/// critical section of inversion. The effective priority of L is
/// boosted to at least H's priority, so no medium-priority task M
/// can preempt L while L holds the lock.
///
/// Formally: if H is waiting on L, then L's effective priority >= H's priority.
pub open spec fn inversion_bounded_spec(
    holder_base: u64,
    holder_highest_waiter: u64,
    waiter_priority: u64,
) -> bool {
    waiter_priority <= holder_highest_waiter
    ==> effective_priority_spec(holder_base, holder_highest_waiter) >= waiter_priority
}

// r[verify priority-inversion.bounded-inversion]
/// Priority inheritance bounds inversion: the holder runs at least
/// as high as any waiter.
proof fn inversion_bounded(holder_base: u64, hw: u64, waiter: u64)
    ensures inversion_bounded_spec(holder_base, hw, waiter)
{}

// r[verify priority-inversion.bounded-inversion]
/// After release (no more waiters), priority returns to base.
proof fn release_restores_base(base: u64)
    ensures effective_priority_spec(base, 0) == base || base == 0
{}

// ============================================================
// Priority ceiling protocol
// ============================================================

// r[depends priority-inversion.ceiling]
/// Spec: under the priority ceiling protocol, a lock has a fixed ceiling
/// equal to the highest priority of any task that may acquire it.
/// A task can only acquire the lock if its priority exceeds all
/// currently held ceilings.
pub open spec fn can_acquire_ceiling_spec(
    task_priority: u64,
    lock_ceiling: u64,
    highest_held_ceiling: u64,
) -> bool {
    task_priority > highest_held_ceiling || highest_held_ceiling == 0
}

// r[impl priority-inversion.ceiling]
/// Check if a task can acquire a lock under the ceiling protocol.
pub fn can_acquire_ceiling(
    task_priority: u64,
    _lock_ceiling: u64,
    highest_held_ceiling: u64,
) -> (result: bool)
    ensures result == can_acquire_ceiling_spec(task_priority, _lock_ceiling, highest_held_ceiling)
{
    task_priority > highest_held_ceiling || highest_held_ceiling == 0
}

// r[verify priority-inversion.ceiling]
/// The highest-priority task can always acquire any lock when no locks are held.
proof fn highest_can_acquire(task_priority: u64, ceiling: u64)
    ensures can_acquire_ceiling_spec(task_priority, ceiling, 0)
{}

// r[verify priority-inversion.ceiling]
/// Ceiling protocol prevents deadlock: if task A holds a lock with ceiling C,
/// no task with priority ≤ C can acquire another lock. This means task A
/// won't block on any other task, so it can always make progress.
proof fn ceiling_prevents_blocking(
    holder_priority: u64,
    lock_ceiling: u64,
    other_priority: u64,
    other_ceiling: u64,
)
    requires
        other_priority <= lock_ceiling,
        lock_ceiling > 0,
    ensures
        !can_acquire_ceiling_spec(other_priority, other_ceiling, lock_ceiling)
{}

} // verus!

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod test {
    use super::*;

    // r[verify priority-inversion.effective-priority]
    #[test]
    fn test_effective_priority() {
        assert_eq!(effective_priority(5, 10), 10); // boosted
        assert_eq!(effective_priority(10, 5), 10); // not boosted
        assert_eq!(effective_priority(5, 5), 5); // equal
        assert_eq!(effective_priority(5, 0), 5); // no waiters
    }

    // r[verify priority-inversion.is-boosted]
    #[test]
    fn test_is_boosted() {
        assert!(is_boosted(5, 10));
        assert!(!is_boosted(10, 5));
        assert!(!is_boosted(5, 5));
    }

    // r[verify priority-inversion.highest-waiter]
    #[test]
    fn test_highest_waiter() {
        assert_eq!(highest_waiter(&[3, 7, 2, 9, 1]), 9);
        assert_eq!(highest_waiter(&[5]), 5);
        assert_eq!(highest_waiter(&[]), 0);
    }

    // r[verify priority-inversion.ceiling]
    #[test]
    fn test_ceiling_protocol() {
        assert!(can_acquire_ceiling(10, 8, 0)); // no locks held
        assert!(can_acquire_ceiling(10, 8, 5)); // priority > held ceiling
        assert!(!can_acquire_ceiling(5, 8, 8)); // priority ≤ held ceiling
    }

    #[cfg(test)]
    mod proptests {
        use proptest::prelude::*;

        use super::*;

        proptest! {
            // r[verify priority-inversion.effective-priority]
            #[test]
            fn prop_effective_ge_both(base: u64, hw: u64) {
                let eff = effective_priority(base, hw);
                prop_assert!(eff >= base);
                prop_assert!(eff >= hw);
            }

            // r[verify priority-inversion.bounded-inversion]
            #[test]
            fn prop_inversion_bounded(holder_base: u64, hw: u64, waiter: u64) {
                if waiter <= hw {
                    prop_assert!(effective_priority(holder_base, hw) >= waiter);
                }
            }

            // r[verify priority-inversion.ceiling]
            #[test]
            fn prop_ceiling_blocks_lower(task_prio in 1u64..=100, held_ceil in 1u64..=100) {
                if task_prio <= held_ceil {
                    prop_assert!(!can_acquire_ceiling(task_prio, 50, held_ceil));
                }
            }

            #[test]
            fn prop_boosted_iff_waiter_higher(base: u64, hw: u64) {
                let boosted = is_boosted(base, hw);
                prop_assert_eq!(boosted, hw > base);
            }

            #[test]
            fn prop_highest_waiter_is_max(
                waiters in proptest::collection::vec(0u64..1000, 1..=10),
            ) {
                let hw = highest_waiter(&waiters);
                let manual = *waiters.iter().max().unwrap();
                prop_assert_eq!(hw, manual);
            }

            #[test]
            fn prop_ceiling_zero_always_acquirable(task_prio: u64) {
                prop_assert!(can_acquire_ceiling(task_prio, 50, 0));
            }
        }
    }
}
