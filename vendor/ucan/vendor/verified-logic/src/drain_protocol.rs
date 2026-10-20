//! Verified drain protocol with double-check admission.
//!
//! Extends the `drain` module with the atomic check-increment-recheck
//! pattern that prevents race conditions during graceful shutdown.
//! The protocol: check draining → increment in_flight → recheck draining.
//! If the recheck finds draining, the increment is rolled back.
//!
//! Also models the notification condition: when the last in-flight
//! operation finishes during drain, the caller should notify waiters.

use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::drain::can_start_op_spec;
#[cfg(verus_keep_ghost)]
use crate::drain::is_drain_complete_spec;

verus! {

// --- try_start_with_recheck ---

// r[depends drain-protocol.try-start-recheck]
/// Spec: double-check admission. Returns (success, new_in_flight).
///
/// 1. If draining at first check → reject (in_flight unchanged)
/// 2. Increment in_flight optimistically
/// 3. Recheck draining → if now draining, rollback and reject
/// 4. Otherwise accept with incremented in_flight
///
/// `draining_at_recheck` models the outcome of the recheck (may differ
/// from initial if drain started concurrently).
pub open spec fn try_start_with_recheck_spec(
    is_draining: bool,
    in_flight: u64,
    draining_at_recheck: bool,
) -> (bool, u64) {
    if is_draining {
        // First check: draining, reject immediately
        (false, in_flight)
    } else if draining_at_recheck {
        // Recheck: drain started between checks, rollback
        (false, in_flight)
    } else if in_flight == u64::MAX {
        // Overflow protection
        (false, in_flight)
    } else {
        // Accept: increment stands
        (true, (in_flight + 1) as u64)
    }
}

// r[impl drain-protocol.try-start-recheck]
/// Attempt to start an operation with double-check.
pub fn try_start_with_recheck(
    is_draining: bool,
    in_flight: u64,
    draining_at_recheck: bool,
) -> (result: (bool, u64))
    ensures
        result.0 == try_start_with_recheck_spec(is_draining, in_flight, draining_at_recheck).0,
        result.1 == try_start_with_recheck_spec(is_draining, in_flight, draining_at_recheck).1,
{
    if is_draining {
        (false, in_flight)
    } else if draining_at_recheck {
        (false, in_flight)
    } else if in_flight == u64::MAX {
        (false, in_flight)
    } else {
        (true, in_flight + 1)
    }
}

// --- finish_with_notify ---

// r[depends drain-protocol.finish-notify]
/// Spec: finish an operation and determine if notification is needed.
/// Returns (new_in_flight, should_notify).
/// should_notify is true when in_flight drops to 0 while draining.
pub open spec fn finish_with_notify_spec(
    is_draining: bool,
    in_flight: u64,
) -> (u64, bool) {
    if in_flight == 0 {
        (0u64, false)
    } else {
        let new_count = (in_flight - 1) as u64;
        (new_count, is_draining && new_count == 0)
    }
}

// r[impl drain-protocol.finish-notify]
/// Finish an operation, returning (new_in_flight, should_notify).
pub fn finish_with_notify(
    is_draining: bool,
    in_flight: u64,
) -> (result: (u64, bool))
    ensures
        result.0 == finish_with_notify_spec(is_draining, in_flight).0,
        result.1 == finish_with_notify_spec(is_draining, in_flight).1,
{
    if in_flight == 0 {
        (0, false)
    } else {
        let new_count = in_flight - 1;
        (new_count, is_draining && new_count == 0)
    }
}

// --- is_fully_drained ---

// r[depends drain-protocol.fully-drained]
/// Spec: drain is complete when draining and no in-flight ops.
pub open spec fn is_fully_drained_spec(is_draining: bool, in_flight: u64) -> bool {
    is_draining && in_flight == 0
}

// r[impl drain-protocol.fully-drained]
/// Check if drain is complete.
pub fn is_fully_drained(is_draining: bool, in_flight: u64) -> (result: bool)
    ensures result == is_fully_drained_spec(is_draining, in_flight)
{
    is_draining && in_flight == 0
}

// --- Safety proofs ---

// r[verify drain-protocol.try-start-recheck]
/// Double-check: if first check sees draining, count unchanged.
proof fn draining_rejects(in_flight: u64, draining_at_recheck: bool)
    ensures try_start_with_recheck_spec(true, in_flight, draining_at_recheck).0 == false
{}

// r[verify drain-protocol.try-start-recheck]
/// Double-check: if recheck sees draining, count unchanged.
proof fn recheck_rollback(in_flight: u64)
    ensures try_start_with_recheck_spec(false, in_flight, true).0 == false
{}

// r[verify drain-protocol.try-start-recheck]
/// On success, in_flight increments by 1.
proof fn success_increments(in_flight: u64)
    requires in_flight < u64::MAX
    ensures
        try_start_with_recheck_spec(false, in_flight, false).0 == true,
        try_start_with_recheck_spec(false, in_flight, false).1 == in_flight + 1,
{}

// r[verify drain-protocol.finish-notify]
/// Notification only during drain when count hits zero.
proof fn notify_condition(is_draining: bool, in_flight: u64)
    ensures
        finish_with_notify_spec(is_draining, in_flight).1 ==>
            (is_draining && finish_with_notify_spec(is_draining, in_flight).0 == 0),
{}

// r[verify drain-protocol.fully-drained]
/// Drain completeness: draining + zero in-flight = fully drained.
proof fn drain_completeness(is_draining: bool, in_flight: u64)
    ensures
        is_fully_drained_spec(is_draining, in_flight) <==>
            (is_draining && in_flight == 0),
{}

// r[verify drain-protocol.finish-notify]
/// Finishing the last op during drain triggers notification.
proof fn last_op_notifies()
    ensures finish_with_notify_spec(true, 1).1 == true
{}

// r[verify drain-protocol.finish-notify]
/// Finishing when not draining never triggers notification.
proof fn no_drain_no_notify(in_flight: u64)
    ensures finish_with_notify_spec(false, in_flight).1 == false
{}

// r[verify drain-protocol.try-start-recheck]
/// Start-then-finish roundtrip preserves in_flight count.
proof fn start_finish_roundtrip(in_flight: u64)
    requires in_flight < u64::MAX
    ensures
        finish_with_notify_spec(
            false,
            try_start_with_recheck_spec(false, in_flight, false).1,
        ).0 == in_flight,
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify drain-protocol.try-start-recheck]
    #[test]
    fn test_double_check() {
        assert_eq!(try_start_with_recheck(true, 0, false), (false, 0));
        assert_eq!(try_start_with_recheck(false, 0, true), (false, 0));
        assert_eq!(try_start_with_recheck(false, 0, false), (true, 1));
        assert_eq!(try_start_with_recheck(false, 5, false), (true, 6));
    }

    // r[verify drain-protocol.finish-notify]
    #[test]
    fn test_finish() {
        assert_eq!(finish_with_notify(true, 1), (0, true));
        assert_eq!(finish_with_notify(true, 2), (1, false));
        assert_eq!(finish_with_notify(false, 1), (0, false));
        assert_eq!(finish_with_notify(false, 0), (0, false));
    }

    // r[verify drain-protocol.fully-drained]
    #[test]
    fn test_fully_drained() {
        assert!(is_fully_drained(true, 0));
        assert!(!is_fully_drained(true, 1));
        assert!(!is_fully_drained(false, 0));
    }

    proptest! {
        // r[verify drain-protocol.try-start-recheck]
        #[test]
        fn prop_draining_rejects(in_flight: u64, recheck: bool) {
            let (ok, _) = try_start_with_recheck(true, in_flight, recheck);
            prop_assert!(!ok);
        }

        // r[verify drain-protocol.try-start-recheck]
        #[test]
        fn prop_success_increments(in_flight in 0u64..u64::MAX) {
            let (ok, new) = try_start_with_recheck(false, in_flight, false);
            prop_assert!(ok);
            prop_assert_eq!(new, in_flight + 1);
        }

        // r[verify drain-protocol.finish-notify]
        #[test]
        fn prop_notify_only_on_drain_zero(is_draining: bool, in_flight: u64) {
            let (new_count, notify) = finish_with_notify(is_draining, in_flight);
            if notify {
                prop_assert!(is_draining);
                prop_assert_eq!(new_count, 0);
            }
        }

        // r[verify drain-protocol.try-start-recheck]
        #[test]
        fn prop_roundtrip(in_flight in 0u64..u64::MAX) {
            let (_, after_start) = try_start_with_recheck(false, in_flight, false);
            let (after_finish, _) = finish_with_notify(false, after_start);
            prop_assert_eq!(after_finish, in_flight);
        }
    }
}
