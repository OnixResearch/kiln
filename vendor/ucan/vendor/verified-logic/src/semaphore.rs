//! Verified counting semaphore logic.
//!
//! A counting semaphore tracks a pool of permits. Threads acquire permits
//! before entering a critical section and release them afterward. When no
//! permits remain, further acquisitions block. With capacity 1 the semaphore
//! degenerates into a binary mutex.
//!
//! All functions are pure arithmetic over `(available, capacity)` pairs —
//! no OS primitives, no atomics. The proofs guarantee conservation (acquire
//! then release restores the count), boundedness (never exceeds capacity),
//! and mutual exclusion for the binary case.

use vstd::prelude::*;

verus! {

// --- can_acquire ---

// r[depends semaphore.can-acquire]
/// True when at least `permits` are available.
pub open spec fn can_acquire_spec(available: u64, permits: u64) -> bool {
    permits <= available
}

// r[impl semaphore.can-acquire]
pub fn can_acquire(available: u64, permits: u64) -> (result: bool)
    ensures result == can_acquire_spec(available, permits)
{
    permits <= available
}

// --- after_acquire ---

// r[depends semaphore.after-acquire]
/// Available count after acquiring `permits`.
pub open spec fn after_acquire_spec(available: u64, permits: u64) -> u64
    recommends permits <= available
{
    (available - permits) as u64
}

// r[impl semaphore.after-acquire]
pub fn after_acquire(available: u64, permits: u64) -> (result: u64)
    requires permits <= available
    ensures result == after_acquire_spec(available, permits)
{
    available - permits
}

// --- after_release ---

// r[depends semaphore.after-release]
/// Available count after releasing `permits` back to the pool.
pub open spec fn after_release_spec(available: u64, permits: u64) -> u64
    recommends available as int + permits as int <= u64::MAX as int
{
    (available as int + permits as int) as u64
}

// r[impl semaphore.after-release]
pub fn after_release(available: u64, permits: u64) -> (result: u64)
    requires available as int + permits as int <= u64::MAX as int
    ensures result == after_release_spec(available, permits)
{
    available + permits
}

// --- is_full ---

// r[depends semaphore.is-full]
/// All permits have been returned to the pool.
pub open spec fn is_full_spec(available: u64, capacity: u64) -> bool {
    available == capacity
}

// r[impl semaphore.is-full]
pub fn is_full(available: u64, capacity: u64) -> (result: bool)
    ensures result == is_full_spec(available, capacity)
{
    available == capacity
}

// --- is_empty ---

// r[depends semaphore.is-empty]
/// No permits remain — further acquisitions must block.
pub open spec fn is_empty_spec(available: u64) -> bool {
    available == 0
}

// r[impl semaphore.is-empty]
pub fn is_empty(available: u64) -> (result: bool)
    ensures result == is_empty_spec(available)
{
    available == 0
}

// --- acquired_count ---

// r[depends semaphore.acquired-count]
/// Number of permits currently held by threads.
pub open spec fn acquired_count_spec(available: u64, capacity: u64) -> u64
    recommends available <= capacity
{
    (capacity - available) as u64
}

// r[impl semaphore.acquired-count]
pub fn acquired_count(available: u64, capacity: u64) -> (result: u64)
    requires available <= capacity
    ensures result == acquired_count_spec(available, capacity)
{
    capacity - available
}

// === Proofs ===

// r[verify semaphore.after-acquire]
// r[verify semaphore.after-release]
/// Acquire then release restores the original count.
proof fn acquire_release_roundtrip(available: u64, permits: u64, capacity: u64)
    requires
        permits <= available,
        available <= capacity,
    ensures
        after_release_spec(after_acquire_spec(available, permits), permits)
            == available
{}

// r[verify semaphore.after-release]
// r[verify semaphore.after-acquire]
/// Release then acquire restores the original count.
proof fn release_acquire_roundtrip(available: u64, permits: u64, capacity: u64)
    requires
        available as int + permits as int <= u64::MAX as int,
    ensures
        after_acquire_spec(after_release_spec(available, permits), permits)
            == available
{}

// r[verify semaphore.after-acquire]
/// Acquiring a positive number of permits strictly decreases available count.
proof fn acquire_decreases(available: u64, permits: u64)
    requires permits > 0, permits <= available
    ensures after_acquire_spec(available, permits) < available
{}

// r[verify semaphore.after-release]
/// Releasing a positive number of permits strictly increases available count.
proof fn release_increases(available: u64, permits: u64)
    requires permits > 0, available as int + permits as int <= u64::MAX as int
    ensures after_release_spec(available, permits) > available
{}

// r[verify semaphore.after-release]
/// Released count never exceeds capacity when the release is valid.
proof fn release_bounded(available: u64, permits: u64, capacity: u64)
    requires
        available <= capacity,
        permits <= capacity - available,
    ensures after_release_spec(available, permits) <= capacity
{}

// r[verify semaphore.is-empty]
// r[verify semaphore.can-acquire]
/// An empty semaphore blocks any positive acquisition.
proof fn empty_blocks_acquire(available: u64, permits: u64)
    requires is_empty_spec(available), permits > 0
    ensures !can_acquire_spec(available, permits)
{}

// r[verify semaphore.is-full]
/// A full semaphore has zero acquired permits.
proof fn full_means_none_acquired(available: u64, capacity: u64)
    requires is_full_spec(available, capacity)
    ensures acquired_count_spec(available, capacity) == 0
{}

// r[verify semaphore.is-full]
// r[verify semaphore.is-empty]
/// Full, empty, and partial are mutually exclusive and exhaustive.
proof fn phases_exhaustive(available: u64, capacity: u64)
    requires available <= capacity, capacity > 0
    ensures ({
        let full = is_full_spec(available, capacity);
        let empty = is_empty_spec(available);
        let partial = available > 0 && available < capacity;
        (full && !empty && !partial)
        || (!full && empty && !partial)
        || (!full && !empty && partial)
    })
{}

// r[verify semaphore.can-acquire]
/// Binary semaphore (capacity 1): can_acquire implies the single permit is free.
proof fn binary_mutual_exclusion(available: u64)
    requires available <= 1, can_acquire_spec(available, 1)
    ensures available == 1
{}

// r[verify semaphore.can-acquire]
/// Monotonic in available: more permits means strictly more acquirable.
proof fn can_acquire_monotonic(a1: u64, a2: u64, permits: u64)
    requires a1 <= a2, can_acquire_spec(a1, permits)
    ensures can_acquire_spec(a2, permits)
{}

// r[verify semaphore.after-acquire]
/// Chained acquires equal a single combined acquire.
proof fn acquire_additive(available: u64, a: u64, b: u64)
    requires a as int + b as int <= available as int
    ensures
        after_acquire_spec(after_acquire_spec(available, a), b)
            == after_acquire_spec(available, (a as int + b as int) as u64)
{}

// r[verify semaphore.after-release]
/// Chained releases equal a single combined release.
proof fn release_additive(available: u64, a: u64, b: u64)
    requires
        available as int + a as int + b as int <= u64::MAX as int,
    ensures
        after_release_spec(after_release_spec(available, a), b)
            == after_release_spec(available, (a as int + b as int) as u64)
{}

// r[verify semaphore.acquired-count]
/// Acquiring k permits increases the acquired count by k.
proof fn acquire_increases_acquired(available: u64, permits: u64, capacity: u64)
    requires permits <= available, available <= capacity
    ensures
        acquired_count_spec(after_acquire_spec(available, permits), capacity)
            == acquired_count_spec(available, capacity) as int + permits as int
{}

// r[verify semaphore.acquired-count]
/// Releasing k permits decreases the acquired count by k.
proof fn release_decreases_acquired(available: u64, permits: u64, capacity: u64)
    requires
        available <= capacity,
        permits <= capacity - available,
    ensures
        acquired_count_spec(after_release_spec(available, permits), capacity) as int
            == acquired_count_spec(available, capacity) as int - permits as int
{}

// r[verify semaphore.after-acquire]
/// Acquiring zero permits is identity.
proof fn acquire_zero_identity(available: u64)
    ensures after_acquire_spec(available, 0) == available
{}

// r[verify semaphore.after-release]
/// Releasing zero permits is identity.
proof fn release_zero_identity(available: u64)
    ensures after_release_spec(available, 0) == available
{}

// r[verify semaphore.can-acquire]
/// Zero permits can always be acquired.
proof fn zero_always_acquirable(available: u64)
    ensures can_acquire_spec(available, 0)
{}

// r[verify semaphore.is-full]
// r[verify semaphore.can-acquire]
/// A full semaphore can acquire up to capacity permits.
proof fn full_can_acquire_capacity(capacity: u64)
    ensures can_acquire_spec(capacity, capacity)
{}

// r[verify semaphore.after-acquire]
// r[verify semaphore.is-empty]
/// Acquiring all available permits empties the semaphore.
proof fn acquire_all_empties(available: u64)
    ensures is_empty_spec(after_acquire_spec(available, available))
{}

} // verus!

#[cfg(test)]
#[allow(clippy::default_numeric_fallback)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify semaphore.can-acquire]
    #[test]
    fn test_can_acquire() {
        assert!(can_acquire(5, 3));
        assert!(can_acquire(5, 5));
        assert!(!can_acquire(5, 6));
        assert!(can_acquire(0, 0));
        assert!(!can_acquire(0, 1));
    }

    // r[verify semaphore.after-acquire]
    #[test]
    fn test_after_acquire() {
        assert_eq!(after_acquire(5, 3), 2);
        assert_eq!(after_acquire(5, 5), 0);
        assert_eq!(after_acquire(5, 0), 5);
    }

    // r[verify semaphore.after-release]
    #[test]
    fn test_after_release() {
        assert_eq!(after_release(2, 3), 5);
        assert_eq!(after_release(0, 5), 5);
        assert_eq!(after_release(5, 0), 5);
    }

    // r[verify semaphore.is-full]
    #[test]
    fn test_is_full() {
        assert!(is_full(10, 10));
        assert!(!is_full(9, 10));
        assert!(is_full(0, 0));
    }

    // r[verify semaphore.is-empty]
    #[test]
    fn test_is_empty() {
        assert!(is_empty(0));
        assert!(!is_empty(1));
    }

    // r[verify semaphore.acquired-count]
    #[test]
    fn test_acquired_count() {
        assert_eq!(acquired_count(7, 10), 3);
        assert_eq!(acquired_count(10, 10), 0);
        assert_eq!(acquired_count(0, 10), 10);
    }

    // r[verify semaphore.after-acquire]
    // r[verify semaphore.after-release]
    #[test]
    fn test_roundtrip() {
        let avail = 10u64;
        let after = after_acquire(avail, 3);
        assert_eq!(after, 7);
        let restored = after_release(after, 3);
        assert_eq!(restored, avail);
    }

    // r[verify semaphore.is-full]
    // r[verify semaphore.is-empty]
    #[test]
    fn test_binary_semaphore_lifecycle() {
        let cap = 1u64;
        let avail = cap;
        assert!(is_full(avail, cap));
        assert!(can_acquire(avail, 1));

        let avail = after_acquire(avail, 1);
        assert!(is_empty(avail));
        assert!(!can_acquire(avail, 1));

        let avail = after_release(avail, 1);
        assert!(is_full(avail, cap));
    }

    proptest! {
        // r[verify semaphore.after-acquire]
        // r[verify semaphore.after-release]
        #[test]
        fn prop_acquire_release_roundtrip(
            capacity in 1u64..1000,
            permits in 0u64..1000,
        ) {
            let permits = permits % (capacity + 1); // clamp to [0, capacity]
            let available = capacity; // start full
            let after_acq = after_acquire(available, permits);
            let restored = after_release(after_acq, permits);
            prop_assert_eq!(restored, available);
        }

        // r[verify semaphore.after-acquire]
        #[test]
        fn prop_acquire_decreases(
            available in 1u64..1000,
            permits in 1u64..1000,
        ) {
            let permits = permits % (available + 1); // clamp to [0, available]
            if permits > 0 {
                prop_assert!(after_acquire(available, permits) < available);
            }
        }

        // r[verify semaphore.after-release]
        #[test]
        fn prop_release_bounded(
            capacity in 1u64..1000,
            acquired_permits in 0u64..1000,
        ) {
            let acquired_permits = acquired_permits % (capacity + 1);
            let available = capacity - acquired_permits;
            let released = after_release(available, acquired_permits);
            prop_assert!(released <= capacity);
        }

        // r[verify semaphore.is-full]
        // r[verify semaphore.is-empty]
        #[test]
        fn prop_phases_exhaustive(
            capacity in 1u64..1000,
            available in 0u64..1000,
        ) {
            let available = available % (capacity + 1); // clamp to [0, capacity]
            let full = is_full(available, capacity);
            let empty = is_empty(available);
            let partial = available > 0 && available < capacity;
            let count = full as u8 + empty as u8 + partial as u8;
            prop_assert_eq!(count, 1);
        }

        // r[verify semaphore.can-acquire]
        #[test]
        fn prop_can_acquire_monotonic(
            a1 in 0u64..500,
            extra in 0u64..500,
            permits in 0u64..1000,
        ) {
            let a2 = a1 + extra;
            if can_acquire(a1, permits) {
                prop_assert!(can_acquire(a2, permits));
            }
        }

        // r[verify semaphore.after-acquire]
        #[test]
        fn prop_acquire_additive(
            available in 0u64..1000,
            a in 0u64..500,
            b in 0u64..500,
        ) {
            if a + b <= available {
                let step_by_step = after_acquire(after_acquire(available, a), b);
                let combined = after_acquire(available, a + b);
                prop_assert_eq!(step_by_step, combined);
            }
        }

        // r[verify semaphore.acquired-count]
        #[test]
        fn prop_acquired_count_conservation(
            capacity in 1u64..1000,
            available in 0u64..1000,
        ) {
            let available = available % (capacity + 1);
            // available + acquired == capacity
            prop_assert_eq!(available + acquired_count(available, capacity), capacity);
        }

        // r[verify semaphore.can-acquire]
        #[test]
        fn prop_zero_always_acquirable(available: u64) {
            prop_assert!(can_acquire(available, 0));
        }

        // r[verify semaphore.after-acquire]
        // r[verify semaphore.is-empty]
        #[test]
        fn prop_acquire_all_empties(available in 0u64..1000) {
            prop_assert!(is_empty(after_acquire(available, available)));
        }
    }
}
