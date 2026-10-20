//! Verified Zircon-style futex wait/wake/requeue.
//!
//! This module refines the single-word `futex_state` intuition into
//! multi-waiter counts. Waiting is gated by value equality, wake releases
//! a bounded prefix, and requeue conserves blocked waiters except those
//! explicitly woken.

use vstd::prelude::*;

verus! {

pub struct WakeResult {
    pub released: u64,
    pub remaining: u64,
}

pub struct RequeueResult {
    pub woken: u64,
    pub moved: u64,
    pub src_remaining: u64,
    pub dst_remaining: u64,
}

// r[depends zircon-futex.wait-compare]
pub open spec fn wait_blocks_spec(observed: u32, expected: u32) -> bool {
    observed == expected
}

// r[impl zircon-futex.wait-compare]
pub fn wait_blocks(observed: u32, expected: u32) -> (result: bool)
    ensures result == wait_blocks_spec(observed, expected)
{
    observed == expected
}

// r[depends zircon-futex.wait-compare]
pub open spec fn waiter_count_after_wait_spec(waiters: u64, observed: u32, expected: u32) -> u64 {
    if wait_blocks_spec(observed, expected) { (waiters + 1) as u64 } else { waiters }
}

// r[impl zircon-futex.wait-compare]
pub fn waiter_count_after_wait(waiters: u64, observed: u32, expected: u32) -> (result: u64)
    requires waiters < u64::MAX || observed != expected
    ensures result == waiter_count_after_wait_spec(waiters, observed, expected)
{
    if observed == expected { waiters + 1 } else { waiters }
}

// r[depends zircon-futex.wake-bound]
pub open spec fn wake_released_spec(waiters: u64, count: u64) -> u64 {
    if count < waiters { count } else { waiters }
}

// r[depends zircon-futex.wake-bound]
pub open spec fn wake_result_spec(waiters: u64, count: u64) -> WakeResult {
    let released = wake_released_spec(waiters, count);
    WakeResult { released, remaining: (waiters - released) as u64 }
}

// r[impl zircon-futex.wake-bound]
pub fn wake(waiters: u64, count: u64) -> (result: WakeResult)
    ensures
        result.released == wake_result_spec(waiters, count).released,
        result.remaining == wake_result_spec(waiters, count).remaining,
{
    let released = if count < waiters { count } else { waiters };
    WakeResult { released, remaining: waiters - released }
}

// r[depends zircon-futex.requeue]
pub open spec fn requeue_result_spec(src_waiters: u64, dst_waiters: u64, wake_count: u64, move_count: u64) -> RequeueResult {
    let woken = wake_released_spec(src_waiters, wake_count);
    let after_wake = (src_waiters - woken) as u64;
    let moved = if move_count < after_wake { move_count } else { after_wake };
    RequeueResult {
        woken,
        moved,
        src_remaining: (after_wake - moved) as u64,
        dst_remaining: (dst_waiters + moved) as u64,
    }
}

// r[impl zircon-futex.requeue]
pub fn requeue(src_waiters: u64, dst_waiters: u64, wake_count: u64, move_count: u64) -> (result: RequeueResult)
    requires dst_waiters <= u64::MAX - if move_count < src_waiters { move_count } else { src_waiters }
    ensures
        result.woken == requeue_result_spec(src_waiters, dst_waiters, wake_count, move_count).woken,
        result.moved == requeue_result_spec(src_waiters, dst_waiters, wake_count, move_count).moved,
        result.src_remaining == requeue_result_spec(src_waiters, dst_waiters, wake_count, move_count).src_remaining,
        result.dst_remaining == requeue_result_spec(src_waiters, dst_waiters, wake_count, move_count).dst_remaining,
{
    let woken = if wake_count < src_waiters { wake_count } else { src_waiters };
    let after_wake = src_waiters - woken;
    let moved = if move_count < after_wake { move_count } else { after_wake };
    RequeueResult {
        woken,
        moved,
        src_remaining: after_wake - moved,
        dst_remaining: dst_waiters + moved,
    }
}

// r[verify zircon-futex.wait-compare]
proof fn mismatched_wait_does_not_block(waiters: u64, observed: u32, expected: u32)
    requires observed != expected
    ensures waiter_count_after_wait_spec(waiters, observed, expected) == waiters
{}

// r[verify zircon-futex.wake-bound]
proof fn wake_releases_at_most_requested(waiters: u64, count: u64)
    ensures wake_released_spec(waiters, count) <= count, wake_released_spec(waiters, count) <= waiters
{}

// r[verify zircon-futex.wake-bound]
proof fn wake_empty_noop(count: u64)
    ensures wake_result_spec(0, count).released == 0, wake_result_spec(0, count).remaining == 0
{}

// r[verify zircon-futex.requeue]
proof fn requeue_conserves_waiters(src_waiters: u64, dst_waiters: u64, wake_count: u64, move_count: u64)
    requires dst_waiters <= u64::MAX - if move_count < src_waiters { move_count } else { src_waiters }
    ensures
        requeue_result_spec(src_waiters, dst_waiters, wake_count, move_count).src_remaining
            + requeue_result_spec(src_waiters, dst_waiters, wake_count, move_count).dst_remaining
            == src_waiters + dst_waiters - requeue_result_spec(src_waiters, dst_waiters, wake_count, move_count).woken,
{}

// r[verify zircon-futex.requeue]
proof fn requeue_move_zero_is_wake_only(src_waiters: u64, dst_waiters: u64, wake_count: u64)
    ensures requeue_result_spec(src_waiters, dst_waiters, wake_count, 0).moved == 0
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify zircon-futex.wait-compare]
    #[test]
    fn test_wait_compare() {
        assert!(wait_blocks(7, 7));
        assert!(!wait_blocks(7, 8));
        assert_eq!(waiter_count_after_wait(3, 7, 7), 4);
        assert_eq!(waiter_count_after_wait(3, 7, 8), 3);
    }

    // r[verify zircon-futex.wake-bound]
    #[test]
    fn test_wake() {
        let r = wake(5, 2);
        assert_eq!(r.released, 2);
        assert_eq!(r.remaining, 3);
        let empty = wake(0, 10);
        assert_eq!(empty.released, 0);
        assert_eq!(empty.remaining, 0);
    }

    // r[verify zircon-futex.requeue]
    #[test]
    fn test_requeue() {
        let r = requeue(5, 3, 2, 2);
        assert_eq!(r.woken, 2);
        assert_eq!(r.moved, 2);
        assert_eq!(r.src_remaining, 1);
        assert_eq!(r.dst_remaining, 5);
    }

    proptest! {
        // r[verify zircon-futex.wake-bound]
        #[test]
        fn prop_wake_bounded(waiters: u64, count: u64) {
            let r = wake(waiters, count);
            prop_assert!(r.released <= count);
            prop_assert!(r.released <= waiters);
            prop_assert_eq!(r.released + r.remaining, waiters);
        }

        // r[verify zircon-futex.requeue]
        #[test]
        fn prop_requeue_conserves(
            src_waiters in 0u64..1_000,
            dst_waiters in 0u64..1_000,
            wake_count in 0u64..1_000,
            move_count in 0u64..1_000,
        ) {
            let r = requeue(src_waiters, dst_waiters, wake_count, move_count);
            prop_assert_eq!(r.src_remaining + r.dst_remaining, src_waiters + dst_waiters - r.woken);
        }
    }
}
