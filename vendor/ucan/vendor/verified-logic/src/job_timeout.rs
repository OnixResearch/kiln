//! Verified job timeout enforcement.
//!
//! Computes deadlines from start time + timeout duration, checks
//! whether a running job has timed out, computes remaining time,
//! and extends deadlines with bounded grace periods. All arithmetic
//! uses saturating operations to prevent overflow.

use vstd::prelude::*;

verus! {

// ============================================================
// compute_deadline
// ============================================================

// r[depends job-timeout.compute-deadline]
/// Spec: deadline = start_time + timeout_ms, saturating at u64::MAX.
pub open spec fn compute_deadline_spec(start_time: u64, timeout_ms: u64) -> u64 {
    if start_time as int + timeout_ms as int > u64::MAX as int {
        u64::MAX
    } else {
        (start_time + timeout_ms) as u64
    }
}

// r[impl job-timeout.compute-deadline]
/// Compute the deadline for a job given its start time and timeout.
pub fn compute_deadline(start_time: u64, timeout_ms: u64) -> (result: u64)
    ensures result == compute_deadline_spec(start_time, timeout_ms)
{
    if timeout_ms > u64::MAX - start_time {
        u64::MAX
    } else {
        start_time + timeout_ms
    }
}

// ============================================================
// is_timed_out
// ============================================================

// r[depends job-timeout.is-timed-out]
/// Spec: the current time is at or past the deadline.
pub open spec fn is_timed_out_spec(now: u64, deadline: u64) -> bool {
    now >= deadline
}

// r[impl job-timeout.is-timed-out]
/// Check if a job has timed out.
pub fn is_timed_out(now: u64, deadline: u64) -> (result: bool)
    ensures result == is_timed_out_spec(now, deadline)
{
    now >= deadline
}

// ============================================================
// time_remaining
// ============================================================

// r[depends job-timeout.time-remaining]
/// Spec: time remaining until deadline, 0 if already past.
pub open spec fn time_remaining_spec(now: u64, deadline: u64) -> u64 {
    if now < deadline {
        (deadline - now) as u64
    } else {
        0u64
    }
}

// r[impl job-timeout.time-remaining]
/// Compute time remaining until the deadline.
pub fn time_remaining(now: u64, deadline: u64) -> (result: u64)
    ensures result == time_remaining_spec(now, deadline)
{
    if now < deadline {
        deadline - now
    } else {
        0
    }
}

// ============================================================
// elapsed_time
// ============================================================

// r[depends job-timeout.elapsed-time]
/// Spec: time elapsed since start.
pub open spec fn elapsed_time_spec(now: u64, start_time: u64) -> u64 {
    if now >= start_time {
        (now - start_time) as u64
    } else {
        0u64
    }
}

// r[impl job-timeout.elapsed-time]
/// Compute time elapsed since the job started.
pub fn elapsed_time(now: u64, start_time: u64) -> (result: u64)
    ensures result == elapsed_time_spec(now, start_time)
{
    if now >= start_time {
        now - start_time
    } else {
        0
    }
}

// ============================================================
// extend_deadline
// ============================================================

// r[depends job-timeout.extend-deadline]
/// Spec: extend a deadline with a grace period, capped by max_extension.
/// Returns min(deadline + grace, deadline + max_extension), saturating.
pub open spec fn extend_deadline_spec(
    deadline: u64,
    grace: u64,
    max_extension: u64,
) -> u64 {
    let effective_grace: int = if grace as int <= max_extension as int {
        grace as int
    } else {
        max_extension as int
    };
    if deadline as int + effective_grace > u64::MAX as int {
        u64::MAX
    } else {
        (deadline as int + effective_grace) as u64
    }
}

// r[impl job-timeout.extend-deadline]
/// Extend a deadline with a bounded grace period.
pub fn extend_deadline(
    deadline: u64,
    grace: u64,
    max_extension: u64,
) -> (result: u64)
    ensures result == extend_deadline_spec(deadline, grace, max_extension)
{
    let capped_grace = if grace <= max_extension { grace } else { max_extension };
    if capped_grace > u64::MAX - deadline {
        u64::MAX
    } else {
        deadline + capped_grace
    }
}

// ============================================================
// Safety proofs
// ============================================================

// r[verify job-timeout.is-timed-out]
/// Timeout monotonicity: once timed out, stays timed out.
proof fn timeout_monotonic(now: u64, later: u64, deadline: u64)
    requires
        now <= later,
        is_timed_out_spec(now, deadline),
    ensures
        is_timed_out_spec(later, deadline)
{}

// r[verify job-timeout.time-remaining]
/// Time remaining is zero iff timed out.
proof fn remaining_zero_iff_timed_out(now: u64, deadline: u64)
    ensures
        (time_remaining_spec(now, deadline) == 0) <==> is_timed_out_spec(now, deadline)
{}

// r[verify job-timeout.time-remaining]
/// Time remaining is bounded by deadline.
proof fn remaining_bounded(now: u64, deadline: u64)
    ensures time_remaining_spec(now, deadline) <= deadline
{}

// r[verify job-timeout.time-remaining]
/// Time remaining decreases as time advances.
proof fn remaining_decreases(now: u64, later: u64, deadline: u64)
    requires now <= later
    ensures time_remaining_spec(later, deadline) <= time_remaining_spec(now, deadline)
{}

// r[verify job-timeout.compute-deadline]
/// Deadline is >= start_time.
proof fn deadline_ge_start(start_time: u64, timeout_ms: u64)
    ensures compute_deadline_spec(start_time, timeout_ms) >= start_time
{}

// r[verify job-timeout.compute-deadline]
/// A job is not timed out at its start time (when timeout > 0 and no overflow).
proof fn not_timed_out_at_start(start_time: u64, timeout_ms: u64)
    requires
        timeout_ms > 0,
        start_time as int + timeout_ms as int <= u64::MAX as int,
    ensures !is_timed_out_spec(start_time, compute_deadline_spec(start_time, timeout_ms))
{}

// r[verify job-timeout.elapsed-time]
/// Elapsed time is zero at start.
proof fn elapsed_zero_at_start(start_time: u64)
    ensures elapsed_time_spec(start_time, start_time) == 0
{}

// r[verify job-timeout.elapsed-time]
/// Elapsed time increases with advancing time.
proof fn elapsed_monotonic(now: u64, later: u64, start_time: u64)
    requires now <= later
    ensures elapsed_time_spec(later, start_time) >= elapsed_time_spec(now, start_time)
{}

// r[verify job-timeout.time-remaining]
// r[verify job-timeout.elapsed-time]
/// Time remaining + elapsed == timeout_ms when no overflow and within window.
proof fn remaining_plus_elapsed(start_time: u64, timeout_ms: u64, now: u64)
    requires
        start_time as int + timeout_ms as int <= u64::MAX as int,
        now >= start_time,
        now <= start_time + timeout_ms,
    ensures ({
        let deadline = compute_deadline_spec(start_time, timeout_ms);
        time_remaining_spec(now, deadline) as int + elapsed_time_spec(now, start_time) as int
            == timeout_ms as int
    })
{}

// r[verify job-timeout.extend-deadline]
/// Extension only pushes the deadline forward.
proof fn extension_pushes_forward(deadline: u64, grace: u64, max_extension: u64)
    ensures extend_deadline_spec(deadline, grace, max_extension) >= deadline
{}

// r[verify job-timeout.extend-deadline]
/// Extension is bounded by deadline + max_extension.
proof fn extension_bounded(deadline: u64, grace: u64, max_extension: u64)
    ensures
        extend_deadline_spec(deadline, grace, max_extension) as int
            <= deadline as int + max_extension as int
{}

// r[verify job-timeout.extend-deadline]
/// Zero grace means no extension.
proof fn zero_grace_identity(deadline: u64, max_extension: u64)
    ensures extend_deadline_spec(deadline, 0, max_extension) == deadline
{}

// r[verify job-timeout.extend-deadline]
/// Grace within limit uses actual grace.
proof fn grace_within_limit(deadline: u64, grace: u64, max_extension: u64)
    requires
        grace <= max_extension,
        deadline as int + grace as int <= u64::MAX as int,
    ensures extend_deadline_spec(deadline, grace, max_extension) == (deadline + grace) as u64
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify job-timeout.compute-deadline]
    #[test]
    fn test_compute_deadline() {
        assert_eq!(compute_deadline(1000, 5000), 6000);
        assert_eq!(compute_deadline(0, 0), 0);
        assert_eq!(compute_deadline(u64::MAX - 100, 200), u64::MAX); // saturates
        assert_eq!(compute_deadline(u64::MAX, 0), u64::MAX);
    }

    // r[verify job-timeout.is-timed-out]
    #[test]
    fn test_is_timed_out() {
        assert!(!is_timed_out(5000, 6000));
        assert!(is_timed_out(6000, 6000)); // at deadline
        assert!(is_timed_out(7000, 6000)); // past deadline
    }

    // r[verify job-timeout.time-remaining]
    #[test]
    fn test_time_remaining() {
        assert_eq!(time_remaining(4000, 6000), 2000);
        assert_eq!(time_remaining(6000, 6000), 0);
        assert_eq!(time_remaining(7000, 6000), 0);
    }

    // r[verify job-timeout.elapsed-time]
    #[test]
    fn test_elapsed_time() {
        assert_eq!(elapsed_time(3500, 1000), 2500);
        assert_eq!(elapsed_time(1000, 1000), 0);
        assert_eq!(elapsed_time(500, 1000), 0); // before start
    }

    // r[verify job-timeout.extend-deadline]
    #[test]
    fn test_extend_deadline() {
        assert_eq!(extend_deadline(6000, 1000, 5000), 7000);
        // Grace exceeds max_extension — capped
        assert_eq!(extend_deadline(6000, 8000, 5000), 11000);
        // Overflow saturates
        assert_eq!(extend_deadline(u64::MAX - 100, 200, u64::MAX), u64::MAX);
        // Zero grace
        assert_eq!(extend_deadline(6000, 0, 5000), 6000);
    }

    // r[verify job-timeout.time-remaining]
    // r[verify job-timeout.elapsed-time]
    #[test]
    fn test_remaining_plus_elapsed() {
        let start = 1000u64;
        let timeout = 5000u64;
        let deadline = compute_deadline(start, timeout);
        for now in [1000, 2000, 3000, 4000, 5000, 6000] {
            if now >= start && now <= deadline {
                assert_eq!(time_remaining(now, deadline) + elapsed_time(now, start), timeout);
            }
        }
    }

    proptest! {
        // r[verify job-timeout.is-timed-out]
        #[test]
        fn prop_timeout_monotonic(
            now in 0u64..10000,
            delta in 0u64..5000,
            deadline in 0u64..10000,
        ) {
            let later = now.saturating_add(delta);
            if is_timed_out(now, deadline) {
                prop_assert!(is_timed_out(later, deadline));
            }
        }

        // r[verify job-timeout.time-remaining]
        #[test]
        fn prop_remaining_zero_iff_timed_out(now in 0u64..10000, deadline in 0u64..10000) {
            prop_assert_eq!(time_remaining(now, deadline) == 0, is_timed_out(now, deadline));
        }

        // r[verify job-timeout.compute-deadline]
        #[test]
        fn prop_deadline_ge_start(start in 0u64..10000, timeout in 0u64..10000) {
            prop_assert!(compute_deadline(start, timeout) >= start);
        }

        // r[verify job-timeout.compute-deadline]
        #[test]
        fn prop_not_timed_out_at_start(start in 0u64..10000, timeout in 1u64..10000) {
            let deadline = compute_deadline(start, timeout);
            prop_assert!(!is_timed_out(start, deadline));
        }

        // r[verify job-timeout.extend-deadline]
        #[test]
        fn prop_extension_pushes_forward(
            deadline in 0u64..10000,
            grace in 0u64..10000,
            max_ext in 0u64..10000,
        ) {
            prop_assert!(extend_deadline(deadline, grace, max_ext) >= deadline);
        }

        // r[verify job-timeout.extend-deadline]
        #[test]
        fn prop_extension_bounded(
            deadline in 0u64..10000,
            grace in 0u64..10000,
            max_ext in 0u64..10000,
        ) {
            let extended = extend_deadline(deadline, grace, max_ext) as u128;
            prop_assert!(extended <= deadline as u128 + max_ext as u128);
        }

        // r[verify job-timeout.time-remaining]
        #[test]
        fn prop_remaining_decreases(
            now in 0u64..10000,
            delta in 0u64..5000,
            deadline in 0u64..10000,
        ) {
            let later = now.saturating_add(delta);
            prop_assert!(time_remaining(later, deadline) <= time_remaining(now, deadline));
        }

        // r[verify job-timeout.elapsed-time]
        #[test]
        fn prop_elapsed_monotonic(
            now in 0u64..10000,
            delta in 0u64..5000,
            start in 0u64..10000,
        ) {
            let later = now.saturating_add(delta);
            prop_assert!(elapsed_time(later, start) >= elapsed_time(now, start));
        }

        // r[verify job-timeout.time-remaining]
        // r[verify job-timeout.elapsed-time]
        #[test]
        fn prop_remaining_plus_elapsed(
            start in 0u64..5000,
            timeout in 0u64..5000,
            offset in 0u64..5000,
        ) {
            let deadline = compute_deadline(start, timeout);
            let now = start.saturating_add(offset);
            if now >= start && now <= deadline && deadline == start + timeout {
                prop_assert_eq!(
                    time_remaining(now, deadline) + elapsed_time(now, start),
                    timeout
                );
            }
        }
    }
}
