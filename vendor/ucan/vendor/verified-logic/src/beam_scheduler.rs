//! Verified BEAM reduction-budget scheduling and priority selection.
//!
//! The model separates budget arithmetic from run-queue selection. `max`
//! priority always wins over lower queues. `high` priority normally wins over
//! `normal`/`low`, but a bounded streak counter lets lower queues run for
//! starvation prevention when no `max` work is pending.

use vstd::prelude::*;

use crate::beam_process::BEAM_ALIVE;

verus! {

pub const PRIORITY_MAX: u64 = 0;
pub const PRIORITY_HIGH: u64 = 1;
pub const PRIORITY_NORMAL: u64 = 2;
pub const PRIORITY_LOW: u64 = 3;

pub const HIGH_STREAK_LIMIT: u64 = 3;

// r[depends beam-scheduler.preemption]
/// Spec: consume reductions with saturating exhaustion.
pub open spec fn consume_reductions_spec(remaining: u64, cost: u64) -> u64 {
    if cost >= remaining { 0 } else { (remaining - cost) as u64 }
}

// r[depends beam-scheduler.preemption]
/// Spec: zero remaining reductions means preempt now.
pub open spec fn should_preempt_spec(remaining: u64) -> bool {
    remaining == 0
}

// r[depends beam-scheduler.priority]
pub open spec fn valid_priority_spec(priority: u64) -> bool {
    priority <= PRIORITY_LOW
}

// r[depends beam-scheduler.priority]
pub open spec fn higher_priority_than_spec(a: u64, b: u64) -> bool {
    valid_priority_spec(a) && valid_priority_spec(b) && a < b
}

// r[depends beam-scheduler.priority]
/// Spec: which priority queue gets CPU next.
pub open spec fn next_priority_spec(max_len: int, high_len: int, normal_len: int, low_len: int, high_streak: u64) -> Option<u64> {
    if max_len > 0 {
        Some(PRIORITY_MAX)
    } else if high_len > 0 && (normal_len <= 0 && low_len <= 0 || high_streak < HIGH_STREAK_LIMIT) {
        Some(PRIORITY_HIGH)
    } else if normal_len > 0 {
        Some(PRIORITY_NORMAL)
    } else if low_len > 0 {
        Some(PRIORITY_LOW)
    } else if high_len > 0 {
        Some(PRIORITY_HIGH)
    } else {
        None
    }
}

// r[depends beam-scheduler.priority]
/// Spec: only alive processes with pending work are runnable.
pub open spec fn runnable_process_spec(state: u64, has_work: bool) -> bool {
    state == BEAM_ALIVE && has_work
}

// r[impl beam-scheduler.preemption]
/// Charge reductions against a process budget.
pub fn consume_reductions(remaining: u64, cost: u64) -> (result: u64)
    ensures result == consume_reductions_spec(remaining, cost)
{
    if cost >= remaining { 0 } else { remaining - cost }
}

// r[impl beam-scheduler.preemption]
/// Explicit yield forces preemption by setting remaining budget to zero.
pub fn yield_now(_remaining: u64) -> (result: u64)
    ensures result == 0, should_preempt_spec(result)
{
    0
}

// r[impl beam-scheduler.preemption]
/// Check whether the current budget is exhausted.
pub fn should_preempt(remaining: u64) -> (result: bool)
    ensures result == should_preempt_spec(remaining)
{
    remaining == 0
}

// r[impl beam-scheduler.priority]
/// Return the next priority queue to service.
pub fn next_priority(max_len: usize, high_len: usize, normal_len: usize, low_len: usize, high_streak: u64) -> (result: Option<u64>)
    ensures result == next_priority_spec(max_len as int, high_len as int, normal_len as int, low_len as int, high_streak)
{
    if max_len > 0 {
        Some(PRIORITY_MAX)
    } else if high_len > 0 && ((normal_len == 0 && low_len == 0) || high_streak < HIGH_STREAK_LIMIT) {
        Some(PRIORITY_HIGH)
    } else if normal_len > 0 {
        Some(PRIORITY_NORMAL)
    } else if low_len > 0 {
        Some(PRIORITY_LOW)
    } else if high_len > 0 {
        Some(PRIORITY_HIGH)
    } else {
        None
    }
}

// r[impl beam-scheduler.priority]
/// Returns true if a process is runnable.
pub fn runnable_process(state: u64, has_work: bool) -> (result: bool)
    ensures result == runnable_process_spec(state, has_work)
{
    state == BEAM_ALIVE && has_work
}

// r[verify beam-scheduler.preemption]
proof fn exhausted_budget_preempts(remaining: u64, cost: u64)
    requires cost >= remaining
    ensures should_preempt_spec(consume_reductions_spec(remaining, cost))
{}

// r[verify beam-scheduler.preemption]
proof fn yield_forces_preemption(remaining: u64)
    ensures should_preempt_spec(0)
{}

// r[verify beam-scheduler.priority]
proof fn priority_order_is_total(a: u64, b: u64)
    requires valid_priority_spec(a), valid_priority_spec(b)
    ensures a == b || higher_priority_than_spec(a, b) || higher_priority_than_spec(b, a)
{}

// r[verify beam-scheduler.priority]
proof fn max_runs_before_normal(normal_len: int, streak: u64)
    requires normal_len > 0
    ensures next_priority_spec(1, 0, normal_len, 0, streak).unwrap() == PRIORITY_MAX
{}

// r[verify beam-scheduler.priority]
proof fn low_runs_when_it_is_all_that_exists(low_len: int)
    requires low_len > 0
    ensures next_priority_spec(0, 0, 0, low_len, HIGH_STREAK_LIMIT).unwrap() == PRIORITY_LOW
{}

// r[verify beam-scheduler.priority]
proof fn exiting_or_dead_process_not_runnable(state: u64, has_work: bool)
    requires state != BEAM_ALIVE
    ensures !runnable_process_spec(state, has_work)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::beam_process::BEAM_ALIVE;
    use crate::beam_process::BEAM_DEAD;
    use crate::beam_process::BEAM_EXITING;

    // r[verify beam-scheduler.preemption]
    #[test]
    fn test_budget_exhaustion_preempts() {
        assert_eq!(consume_reductions(10, 3), 7);
        assert_eq!(consume_reductions(10, 10), 0);
        assert!(should_preempt(consume_reductions(10, 10)));
        assert!(should_preempt(yield_now(99)));
    }

    // r[verify beam-scheduler.priority]
    #[test]
    fn test_priority_selection() {
        assert_eq!(next_priority(1, 0, 1, 0, 0), Some(PRIORITY_MAX));
        assert_eq!(next_priority(1, 0, 1, 0, HIGH_STREAK_LIMIT), Some(PRIORITY_MAX));
        assert_eq!(next_priority(0, 0, 0, 2, HIGH_STREAK_LIMIT), Some(PRIORITY_LOW));
        assert_eq!(next_priority(0, 0, 0, 0, 0), None);
    }

    // r[verify beam-scheduler.priority]
    #[test]
    fn test_runnable_process_state() {
        assert!(runnable_process(BEAM_ALIVE, true));
        assert!(!runnable_process(BEAM_EXITING, true));
        assert!(!runnable_process(BEAM_DEAD, true));
        assert!(!runnable_process(BEAM_ALIVE, false));
    }

    proptest! {
        // r[verify beam-scheduler.preemption]
        #[test]
        fn prop_consumed_budget_never_negative(remaining: u64, cost: u64) {
            let left = consume_reductions(remaining, cost);
            prop_assert!(left <= remaining);
        }

        // r[verify beam-scheduler.priority]
        #[test]
        fn prop_low_priority_runs_when_alone(low_len in 1usize..16) {
            prop_assert_eq!(next_priority(0, 0, 0, low_len, HIGH_STREAK_LIMIT), Some(PRIORITY_LOW));
        }

        // r[verify beam-scheduler.priority]
        #[test]
        fn prop_max_priority_ignores_high_streak(normal_len in 1usize..16, streak: u64) {
            prop_assert_eq!(next_priority(1, 0, normal_len, 0, streak), Some(PRIORITY_MAX));
        }
    }
}
