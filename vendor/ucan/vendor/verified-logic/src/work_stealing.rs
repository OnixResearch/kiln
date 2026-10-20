//! Verified work-stealing deque bounds (Chase-Lev style).
//!
//! A work-stealing deque has an owner thread that pushes and pops from
//! the bottom, and thief threads that steal from the top. The `top` and
//! `bottom` indices define the valid range. Correctness requires tracking
//! that size stays non-negative, push respects capacity, and steal/pop
//! don't underflow.

use vstd::prelude::*;

verus! {

// --- deque_size ---

// r[depends work_stealing.deque-size]
/// Number of elements in the deque. bottom >= top always holds in a
/// well-formed deque; if violated, returns 0.
pub open spec fn deque_size_spec(top: u64, bottom: u64) -> u64 {
    if bottom >= top { (bottom - top) as u64 } else { 0 }
}

// r[impl work_stealing.deque-size]
pub fn deque_size(top: u64, bottom: u64) -> (result: u64)
    ensures result == deque_size_spec(top, bottom)
{
    if bottom >= top { bottom - top } else { 0 }
}

// --- is_empty ---

// r[depends work_stealing.is-empty]
/// The deque is empty when top >= bottom.
pub open spec fn is_empty_spec(top: u64, bottom: u64) -> bool {
    bottom <= top
}

// r[impl work_stealing.is-empty]
pub fn is_empty(top: u64, bottom: u64) -> (result: bool)
    ensures result == is_empty_spec(top, bottom)
{
    bottom <= top
}

// --- can_push ---

// r[depends work_stealing.can-push]
/// The owner can push when the deque is not full.
pub open spec fn can_push_spec(top: u64, bottom: u64, capacity: u64) -> bool {
    deque_size_spec(top, bottom) < capacity
}

// r[impl work_stealing.can-push]
pub fn can_push(top: u64, bottom: u64, capacity: u64) -> (result: bool)
    ensures result == can_push_spec(top, bottom, capacity)
{
    deque_size(top, bottom) < capacity
}

// --- can_pop ---

// r[depends work_stealing.can-pop]
/// The owner can pop when the deque is not empty.
pub open spec fn can_pop_spec(top: u64, bottom: u64) -> bool {
    !is_empty_spec(top, bottom)
}

// r[impl work_stealing.can-pop]
pub fn can_pop(top: u64, bottom: u64) -> (result: bool)
    ensures result == can_pop_spec(top, bottom)
{
    !is_empty(top, bottom)
}

// --- can_steal ---

// r[depends work_stealing.can-steal]
/// A thief can steal when the deque is not empty.
pub open spec fn can_steal_spec(top: u64, bottom: u64) -> bool {
    !is_empty_spec(top, bottom)
}

// r[impl work_stealing.can-steal]
pub fn can_steal(top: u64, bottom: u64) -> (result: bool)
    ensures result == can_steal_spec(top, bottom)
{
    !is_empty(top, bottom)
}

// --- after_push ---

// r[depends work_stealing.after-push]
/// Bottom index after a push (owner side).
pub open spec fn after_push_spec(bottom: u64) -> u64 {
    (bottom + 1) as u64
}

// r[impl work_stealing.after-push]
pub fn after_push(bottom: u64) -> (result: u64)
    requires bottom < u64::MAX
    ensures result == after_push_spec(bottom)
{
    bottom + 1
}

// --- after_pop ---

// r[depends work_stealing.after-pop]
/// Bottom index after a pop (owner side).
pub open spec fn after_pop_spec(bottom: u64) -> u64 {
    (bottom - 1) as u64
}

// r[impl work_stealing.after-pop]
pub fn after_pop(bottom: u64) -> (result: u64)
    requires bottom > 0
    ensures result == after_pop_spec(bottom)
{
    bottom - 1
}

// --- after_steal ---

// r[depends work_stealing.after-steal]
/// Top index after a successful steal (thief side).
pub open spec fn after_steal_spec(top: u64) -> u64 {
    (top + 1) as u64
}

// r[impl work_stealing.after-steal]
pub fn after_steal(top: u64) -> (result: u64)
    requires top < u64::MAX
    ensures result == after_steal_spec(top)
{
    top + 1
}

// --- buffer_index ---

// r[depends work_stealing.buffer-index]
/// Map a logical index to a physical buffer position (circular).
pub open spec fn buffer_index_spec(logical: u64, capacity: u64) -> u64
    recommends capacity > 0
{
    (logical % capacity) as u64
}

// r[impl work_stealing.buffer-index]
pub fn buffer_index(logical: u64, capacity: u64) -> (result: u64)
    requires capacity > 0
    ensures result == buffer_index_spec(logical, capacity)
{
    logical % capacity
}

// === Proofs ===

// r[verify work_stealing.deque-size]
/// Empty deque has size zero.
proof fn empty_size_zero(top: u64)
    ensures deque_size_spec(top, top) == 0
{}

// r[verify work_stealing.deque-size]
// r[verify work_stealing.is-empty]
/// Size zero iff empty.
proof fn size_zero_iff_empty(top: u64, bottom: u64)
    requires bottom >= top
    ensures (deque_size_spec(top, bottom) == 0) == is_empty_spec(top, bottom)
{}

// r[verify work_stealing.after-push]
// r[verify work_stealing.deque-size]
/// Push increases size by 1.
proof fn push_increases_size(top: u64, bottom: u64)
    requires bottom >= top, bottom < u64::MAX
    ensures deque_size_spec(top, after_push_spec(bottom))
        == (deque_size_spec(top, bottom) + 1) as u64
{}

// r[verify work_stealing.after-pop]
// r[verify work_stealing.deque-size]
/// Pop decreases size by 1.
proof fn pop_decreases_size(top: u64, bottom: u64)
    requires bottom > top
    ensures deque_size_spec(top, after_pop_spec(bottom))
        == (deque_size_spec(top, bottom) - 1) as u64
{}

// r[verify work_stealing.after-steal]
// r[verify work_stealing.deque-size]
/// Steal decreases size by 1.
proof fn steal_decreases_size(top: u64, bottom: u64)
    requires bottom > top, top < u64::MAX
    ensures deque_size_spec(after_steal_spec(top), bottom)
        == (deque_size_spec(top, bottom) - 1) as u64
{}

// r[verify work_stealing.after-push]
// r[verify work_stealing.after-pop]
/// Push then pop is identity on bottom.
proof fn push_pop_roundtrip(bottom: u64)
    requires bottom > 0, bottom < u64::MAX
    ensures after_pop_spec(after_push_spec(bottom)) == bottom
{}

// r[verify work_stealing.after-push]
// r[verify work_stealing.is-empty]
/// Push into empty deque makes it non-empty.
proof fn push_into_empty_non_empty(top: u64)
    requires top < u64::MAX
    ensures !is_empty_spec(top, after_push_spec(top))
{}

// r[verify work_stealing.after-pop]
// r[verify work_stealing.is-empty]
/// Pop from size-1 deque makes it empty.
proof fn pop_last_empties(top: u64)
    requires top < u64::MAX
    ensures is_empty_spec(top, after_pop_spec((top + 1) as u64))
{}

// r[verify work_stealing.after-steal]
// r[verify work_stealing.is-empty]
/// Steal from size-1 deque makes it empty.
proof fn steal_last_empties(top: u64, bottom: u64)
    requires bottom == top + 1, top < u64::MAX
    ensures is_empty_spec(after_steal_spec(top), bottom)
{}

// r[verify work_stealing.can-pop]
// r[verify work_stealing.can-steal]
/// can_pop and can_steal are the same predicate.
proof fn pop_steal_same(top: u64, bottom: u64)
    ensures can_pop_spec(top, bottom) == can_steal_spec(top, bottom)
{}

// r[verify work_stealing.buffer-index]
/// Buffer index is always less than capacity.
proof fn buffer_index_in_range(logical: u64, capacity: u64)
    requires capacity > 0
    ensures buffer_index_spec(logical, capacity) < capacity
{}

// r[verify work_stealing.can-push]
/// Empty deque with non-zero capacity can push.
proof fn empty_can_push(top: u64, capacity: u64)
    requires capacity > 0
    ensures can_push_spec(top, top, capacity)
{}

// r[verify work_stealing.can-push]
/// Full deque cannot push.
proof fn full_cannot_push(top: u64, capacity: u64)
    requires capacity > 0, (top as int) + (capacity as int) <= (u64::MAX as int)
    ensures !can_push_spec(top, (top + capacity) as u64, capacity)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify work_stealing.deque-size]
    #[test]
    fn test_deque_size() {
        assert_eq!(deque_size(0, 0), 0);
        assert_eq!(deque_size(0, 5), 5);
        assert_eq!(deque_size(3, 7), 4);
        assert_eq!(deque_size(5, 3), 0); // malformed
    }

    // r[verify work_stealing.is-empty]
    #[test]
    fn test_is_empty() {
        assert!(is_empty(0, 0));
        assert!(is_empty(5, 3));
        assert!(!is_empty(0, 1));
    }

    // r[verify work_stealing.can-push]
    #[test]
    fn test_can_push() {
        assert!(can_push(0, 0, 8));
        assert!(can_push(0, 7, 8));
        assert!(!can_push(0, 8, 8));
    }

    // r[verify work_stealing.can-pop]
    // r[verify work_stealing.can-steal]
    #[test]
    fn test_can_pop_steal() {
        assert!(!can_pop(0, 0));
        assert!(can_pop(0, 1));
        assert_eq!(can_pop(0, 5), can_steal(0, 5));
    }

    // r[verify work_stealing.after-push]
    // r[verify work_stealing.after-pop]
    #[test]
    fn test_push_pop() {
        let b0 = 3u64;
        let b1 = after_push(b0);
        assert_eq!(b1, 4);
        let b2 = after_pop(b1);
        assert_eq!(b2, 3);
    }

    // r[verify work_stealing.after-steal]
    #[test]
    fn test_steal() {
        assert_eq!(after_steal(0), 1);
        assert_eq!(after_steal(5), 6);
    }

    // r[verify work_stealing.buffer-index]
    #[test]
    fn test_buffer_index() {
        assert_eq!(buffer_index(0, 8), 0);
        assert_eq!(buffer_index(7, 8), 7);
        assert_eq!(buffer_index(8, 8), 0);
        assert_eq!(buffer_index(15, 8), 7);
    }

    proptest! {
        // r[verify work_stealing.deque-size]
        #[test]
        fn prop_size_consistent(top in 0u64..1000, delta in 0u64..1000) {
            let bottom = top + delta;
            prop_assert_eq!(deque_size(top, bottom), delta);
        }

        // r[verify work_stealing.deque-size]
        // r[verify work_stealing.is-empty]
        #[test]
        fn prop_size_zero_iff_empty(top in 0u64..1000, delta in 0u64..1000) {
            let bottom = top + delta;
            prop_assert_eq!(deque_size(top, bottom) == 0, is_empty(top, bottom));
        }

        // r[verify work_stealing.after-push]
        // r[verify work_stealing.deque-size]
        #[test]
        fn prop_push_increases_size(top in 0u64..1000, delta in 0u64..1000) {
            let bottom = top + delta;
            let new_bottom = after_push(bottom);
            prop_assert_eq!(deque_size(top, new_bottom), delta + 1);
        }

        // r[verify work_stealing.after-pop]
        // r[verify work_stealing.deque-size]
        #[test]
        fn prop_pop_decreases_size(top in 0u64..999, delta in 1u64..1000) {
            let bottom = top + delta;
            let new_bottom = after_pop(bottom);
            prop_assert_eq!(deque_size(top, new_bottom), delta - 1);
        }

        // r[verify work_stealing.after-steal]
        // r[verify work_stealing.deque-size]
        #[test]
        fn prop_steal_decreases_size(top in 0u64..999, delta in 1u64..1000) {
            let bottom = top + delta;
            let new_top = after_steal(top);
            prop_assert_eq!(deque_size(new_top, bottom), delta - 1);
        }

        // r[verify work_stealing.after-push]
        // r[verify work_stealing.after-pop]
        #[test]
        fn prop_push_pop_roundtrip(bottom in 1u64..u64::MAX) {
            let pushed = after_push(bottom);
            let popped = after_pop(pushed);
            prop_assert_eq!(popped, bottom);
        }

        // r[verify work_stealing.can-pop]
        // r[verify work_stealing.can-steal]
        #[test]
        fn prop_pop_steal_identical(top: u64, bottom: u64) {
            prop_assert_eq!(can_pop(top, bottom), can_steal(top, bottom));
        }

        // r[verify work_stealing.buffer-index]
        #[test]
        fn prop_buffer_index_in_range(logical: u64, capacity in 1u64..1000) {
            prop_assert!(buffer_index(logical, capacity) < capacity);
        }

        // r[verify work_stealing.can-push]
        #[test]
        fn prop_empty_can_push(top in 0u64..1000, capacity in 1u64..1000) {
            prop_assert!(can_push(top, top, capacity));
        }
    }
}
