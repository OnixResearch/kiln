//! Verified drain-state admission protocol.
//!
//! Models the check-increment-recheck pattern for graceful shutdown.
//! Functions operate on (`is_draining`: bool, `in_flight`: u64) state
//! to control operation admission, track in-flight work, and determine
//! when shutdown is complete.

use vstd::prelude::*;

verus! {

// --- can_start_op ---

// r[depends drain.can-start-op]
/// Spec: whether new operations can start.
/// Only admit when not draining.
pub open spec fn can_start_op_spec(is_draining: bool) -> bool {
    !is_draining
}

// r[impl drain.can-start-op]
/// Returns true if new operations can start.
/// Only admits when not currently draining.
pub fn can_start_op(is_draining: bool) -> (result: bool)
    ensures result == can_start_op_spec(is_draining)
{
    !is_draining
}

// r[verify drain.can-start-op]
/// Cannot start operations during drain.
proof fn can_start_during_drain()
    ensures !can_start_op_spec(true)
{}

// r[verify drain.can-start-op]
/// Can start operations when not draining.
proof fn can_start_when_not_draining()
    ensures can_start_op_spec(false)
{}

// r[verify drain.can-start-op]
/// If can start, then not draining.
proof fn can_start_implies_not_draining(is_draining: bool)
    requires can_start_op_spec(is_draining)
    ensures !is_draining
{}

// --- after_start_op ---

// r[depends drain.after-start-op]
/// Spec: in-flight count after starting an operation.
/// Increments by 1, saturating at u64::MAX.
pub open spec fn after_start_op_spec(in_flight: u64) -> u64 {
    if in_flight < u64::MAX { (in_flight + 1) as u64 } else { in_flight }
}

// r[impl drain.after-start-op]
/// Returns in-flight count after starting an operation.
/// Increments by 1, requires `in_flight` < `u64::MAX`.
pub fn after_start_op(in_flight: u64) -> (result: u64)
    requires in_flight < u64::MAX
    ensures result == after_start_op_spec(in_flight)
{
    in_flight + 1
}

// r[verify drain.after-start-op]
/// Starting an operation increases the count (when not saturated).
proof fn start_increases_count(in_flight: u64)
    requires in_flight < u64::MAX
    ensures after_start_op_spec(in_flight) == in_flight + 1
{}

// r[verify drain.after-start-op]
/// Result is always >= input.
proof fn start_non_decreasing(in_flight: u64)
    ensures after_start_op_spec(in_flight) >= in_flight
{}

// --- after_finish_op ---

// r[depends drain.after-finish-op]
/// Spec: in-flight count after finishing an operation.
/// Decrements by 1, flooring at 0.
pub open spec fn after_finish_op_spec(in_flight: u64) -> u64 {
    if in_flight > 0 { (in_flight - 1) as u64 } else { 0 }
}

// r[impl drain.after-finish-op]
/// Returns in-flight count after finishing an operation.
/// Decrements by 1, requires `in_flight` > 0.
pub fn after_finish_op(in_flight: u64) -> (result: u64)
    requires in_flight > 0
    ensures result == after_finish_op_spec(in_flight)
{
    in_flight - 1
}

// r[verify drain.after-finish-op]
/// Finishing an operation decreases the count (when positive).
proof fn finish_decreases_count(in_flight: u64)
    requires in_flight > 0
    ensures after_finish_op_spec(in_flight) == in_flight - 1
{}

// r[verify drain.after-finish-op]
/// Result is always <= input.
proof fn finish_non_increasing(in_flight: u64)
    ensures after_finish_op_spec(in_flight) <= in_flight
{}

// --- is_drain_complete ---

// r[depends drain.is-drain-complete]
/// Spec: whether drain has completed.
/// Requires drain active AND zero `in_flight` operations.
pub open spec fn is_drain_complete_spec(is_draining: bool, in_flight: u64) -> bool {
    is_draining && in_flight == 0
}

// r[impl drain.is-drain-complete]
/// Returns true if drain has completed.
/// Both draining and zero in-flight must hold.
pub fn is_drain_complete(is_draining: bool, in_flight: u64) -> (result: bool)
    ensures result == is_drain_complete_spec(is_draining, in_flight)
{
    is_draining && in_flight == 0
}

// r[verify drain.is-drain-complete]
/// Drain complete requires drain active.
proof fn drain_complete_requires_drain_active(is_draining: bool, in_flight: u64)
    requires is_drain_complete_spec(is_draining, in_flight)
    ensures is_draining
{}

// r[verify drain.is-drain-complete]
/// Drain complete requires zero in-flight.
proof fn drain_complete_requires_zero_in_flight(is_draining: bool, in_flight: u64)
    requires is_drain_complete_spec(is_draining, in_flight)
    ensures in_flight == 0
{}

// r[verify drain.is-drain-complete]
/// Not draining means drain cannot be complete.
proof fn not_draining_not_complete(in_flight: u64)
    ensures !is_drain_complete_spec(false, in_flight)
{}

// r[verify drain.is-drain-complete]
/// In-flight operations mean drain not complete.
proof fn in_flight_not_complete(is_draining: bool, in_flight: u64)
    requires in_flight > 0
    ensures !is_drain_complete_spec(is_draining, in_flight)
{}

// --- should_notify ---

// r[depends drain.should-notify]
/// Spec: whether drain waiters should be notified.
/// True when draining, old count > 0, and new count == 0.
/// This detects the transition when the last operation finishes.
pub open spec fn should_notify_spec(
    is_draining: bool,
    old_in_flight: u64,
    new_in_flight: u64,
) -> bool {
    is_draining && old_in_flight > 0 && new_in_flight == 0
}

// r[impl drain.should-notify]
/// Returns true if drain waiters should be notified.
/// Detects transition to zero in-flight during drain.
pub fn should_notify(
    is_draining: bool,
    old_in_flight: u64,
    new_in_flight: u64,
) -> (result: bool)
    ensures result == should_notify_spec(is_draining, old_in_flight, new_in_flight)
{
    is_draining && old_in_flight > 0 && new_in_flight == 0
}

// r[verify drain.should-notify]
/// Notification only during drain.
proof fn notify_requires_draining(
    is_draining: bool,
    old_in_flight: u64,
    new_in_flight: u64,
)
    requires should_notify_spec(is_draining, old_in_flight, new_in_flight)
    ensures is_draining
{}

// r[verify drain.should-notify]
/// Notification requires transition from positive to zero.
proof fn notify_requires_transition(
    is_draining: bool,
    old_in_flight: u64,
    new_in_flight: u64,
)
    requires should_notify_spec(is_draining, old_in_flight, new_in_flight)
    ensures old_in_flight > 0 && new_in_flight == 0
{}

// r[verify drain.should-notify]
/// Notification implies drain complete after transition.
proof fn notify_implies_drain_complete(
    is_draining: bool,
    old_in_flight: u64,
    new_in_flight: u64,
)
    requires should_notify_spec(is_draining, old_in_flight, new_in_flight)
    ensures is_drain_complete_spec(is_draining, new_in_flight)
{}

// --- start_drain_decision ---

// r[depends drain.start-drain]
/// Spec: decision when starting drain.
/// 0: already draining
/// 1: drain completes immediately (no in-flight ops)
/// 2: drain starts but must wait for ops to finish
pub open spec fn start_drain_decision_spec(is_draining: bool, in_flight: u64) -> u8 {
    if is_draining {
        0
    } else if in_flight == 0 {
        1
    } else {
        2
    }
}

// r[impl drain.start-drain]
/// Returns drain decision when starting drain.
/// 0: already draining, 1: immediate completion, 2: must wait.
pub fn start_drain_decision(is_draining: bool, in_flight: u64) -> (result: u8)
    ensures result == start_drain_decision_spec(is_draining, in_flight)
{
    if is_draining {
        0
    } else if in_flight == 0 {
        1
    } else {
        2
    }
}

// r[verify drain.start-drain]
/// Decision covers all cases.
proof fn decision_covers_all_cases(is_draining: bool, in_flight: u64)
    ensures
        start_drain_decision_spec(is_draining, in_flight) == 0 ||
        start_drain_decision_spec(is_draining, in_flight) == 1 ||
        start_drain_decision_spec(is_draining, in_flight) == 2
{}

// r[verify drain.start-drain]
/// Already draining case.
proof fn decision_already_draining(in_flight: u64)
    ensures start_drain_decision_spec(true, in_flight) == 0
{}

// r[verify drain.start-drain]
/// Immediate completion case.
proof fn decision_immediate_completion()
    ensures start_drain_decision_spec(false, 0) == 1
{}

// r[verify drain.start-drain]
/// Must wait case.
proof fn decision_must_wait(in_flight: u64)
    requires in_flight > 0
    ensures start_drain_decision_spec(false, in_flight) == 2
{}

// r[verify drain.start-drain]
/// Decision meanings are mutually exclusive.
proof fn decision_meanings_exclusive(is_draining: bool, in_flight: u64)
    ensures
        (start_drain_decision_spec(is_draining, in_flight) == 0) ==
        (is_draining == true),
        (start_drain_decision_spec(is_draining, in_flight) == 1) ==
        (is_draining == false && in_flight == 0),
        (start_drain_decision_spec(is_draining, in_flight) == 2) ==
        (is_draining == false && in_flight > 0)
{}

// --- roundtrip properties ---

// r[verify drain.after-start-op]
// r[verify drain.after-finish-op]
/// Start then finish returns to original count.
proof fn start_finish_roundtrip(in_flight: u64)
    requires in_flight > 0, in_flight < u64::MAX
    ensures after_finish_op_spec(after_start_op_spec(in_flight)) == in_flight
{}

// r[verify drain.after-start-op]
// r[verify drain.after-finish-op]
/// Finish then start returns to original count (when positive).
proof fn finish_start_roundtrip(in_flight: u64)
    requires in_flight > 0, in_flight < u64::MAX
    ensures after_start_op_spec(after_finish_op_spec(in_flight)) == in_flight
{}

// --- cross-function properties ---

// r[verify drain.can-start-op]
// r[verify drain.is-drain-complete]
/// Cannot start when drain complete.
proof fn cannot_start_when_complete(is_draining: bool, in_flight: u64)
    requires is_drain_complete_spec(is_draining, in_flight)
    ensures !can_start_op_spec(is_draining)
{}

// r[verify drain.should-notify]
// r[verify drain.after-finish-op]
/// Notification happens when finishing last operation during drain.
proof fn notify_on_finish_last_during_drain(is_draining: bool)
    requires is_draining
    ensures should_notify_spec(is_draining, 1, after_finish_op_spec(1))
{}

// r[verify drain.start-drain]
// r[verify drain.is-drain-complete]
/// Decision 1 means immediate completion.
proof fn decision_1_immediate_complete()
    ensures
        start_drain_decision_spec(false, 0) == 1,
        is_drain_complete_spec(true, 0)
{}

// r[verify drain.start-drain]
// r[verify drain.after-start-op]
/// Starting operation changes decision from 1 to 2.
proof fn start_op_changes_decision(in_flight: u64)
    requires in_flight < u64::MAX
    ensures
        start_drain_decision_spec(false, in_flight) == 1 ==>
        start_drain_decision_spec(false, after_start_op_spec(in_flight)) == 2
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify drain.can-start-op]
    #[test]
    fn test_can_start_op() {
        assert!(can_start_op(false));
        assert!(!can_start_op(true));
    }

    // r[verify drain.after-start-op]
    #[test]
    fn test_after_start_op() {
        assert_eq!(after_start_op(0), 1);
        assert_eq!(after_start_op(10), 11);
        assert_eq!(after_start_op(u64::MAX - 1), u64::MAX);
    }

    // r[verify drain.after-finish-op]
    #[test]
    fn test_after_finish_op() {
        assert_eq!(after_finish_op(1), 0);
        assert_eq!(after_finish_op(10), 9);
        assert_eq!(after_finish_op(u64::MAX), u64::MAX - 1);
    }

    // r[verify drain.is-drain-complete]
    #[test]
    fn test_is_drain_complete() {
        assert!(!is_drain_complete(false, 0));
        assert!(!is_drain_complete(false, 5));
        assert!(!is_drain_complete(true, 5));
        assert!(is_drain_complete(true, 0));
    }

    // r[verify drain.should-notify]
    #[test]
    fn test_should_notify() {
        assert!(!should_notify(false, 1, 0));
        assert!(!should_notify(true, 0, 0));
        assert!(!should_notify(true, 1, 1));
        assert!(should_notify(true, 1, 0));
        assert!(should_notify(true, 5, 0));
    }

    // r[verify drain.start-drain]
    #[test]
    fn test_start_drain_decision() {
        assert_eq!(start_drain_decision(true, 0), 0);
        assert_eq!(start_drain_decision(true, 5), 0);
        assert_eq!(start_drain_decision(false, 0), 1);
        assert_eq!(start_drain_decision(false, 5), 2);
    }

    proptest! {
        // r[verify drain.after-start-op]
        // r[verify drain.after-finish-op]
        #[test]
        fn prop_start_finish_roundtrip(in_flight in 1u64..u64::MAX) {
            let started = after_start_op(in_flight);
            let finished = after_finish_op(started);
            prop_assert_eq!(finished, in_flight);
        }

        // r[verify drain.after-start-op]
        // r[verify drain.after-finish-op]
        #[test]
        fn prop_finish_start_roundtrip(in_flight in 1u64..u64::MAX) {
            let finished = after_finish_op(in_flight);
            let started = after_start_op(finished);
            prop_assert_eq!(started, in_flight);
        }

        // r[verify drain.can-start-op]
        #[test]
        fn prop_can_start_iff_not_draining(is_draining: bool) {
            prop_assert_eq!(can_start_op(is_draining), !is_draining);
        }

        // r[verify drain.is-drain-complete]
        #[test]
        fn prop_drain_complete_iff_draining_and_zero(is_draining: bool, in_flight: u64) {
            prop_assert_eq!(
                is_drain_complete(is_draining, in_flight),
                is_draining && in_flight == 0
            );
        }

        // r[verify drain.should-notify]
        #[test]
        fn prop_notify_transition_to_zero(
            is_draining: bool,
            old_in_flight: u64,
            new_in_flight: u64
        ) {
            if should_notify(is_draining, old_in_flight, new_in_flight) {
                prop_assert!(is_draining);
                prop_assert!(old_in_flight > 0);
                prop_assert_eq!(new_in_flight, 0);
            }
        }

        // r[verify drain.should-notify]
        // r[verify drain.is-drain-complete]
        #[test]
        fn prop_notify_implies_complete(
            is_draining: bool,
            old_in_flight: u64,
            new_in_flight: u64
        ) {
            if should_notify(is_draining, old_in_flight, new_in_flight) {
                prop_assert!(is_drain_complete(is_draining, new_in_flight));
            }
        }

        // r[verify drain.start-drain]
        #[test]
        fn prop_decision_covers_cases(is_draining: bool, in_flight: u64) {
            let decision = start_drain_decision(is_draining, in_flight);
            prop_assert!(decision == 0 || decision == 1 || decision == 2);
        }

        // r[verify drain.start-drain]
        #[test]
        fn prop_decision_meanings(is_draining: bool, in_flight: u64) {
            let decision = start_drain_decision(is_draining, in_flight);
            match decision {
                0 => prop_assert!(is_draining),
                1 => prop_assert!(!is_draining && in_flight == 0),
                2 => prop_assert!(!is_draining && in_flight > 0),
                _ => unreachable!(),
            }
        }

        // r[verify drain.can-start-op]
        // r[verify drain.is-drain-complete]
        #[test]
        fn prop_cannot_start_when_complete(is_draining: bool, in_flight: u64) {
            if is_drain_complete(is_draining, in_flight) {
                prop_assert!(!can_start_op(is_draining));
            }
        }

        // r[verify drain.should-notify]
        // r[verify drain.after-finish-op]
        #[test]
        fn prop_notify_on_finish_last(is_draining: bool) {
            if is_draining {
                prop_assert!(should_notify(is_draining, 1, after_finish_op(1)));
            }
        }

        // r[verify drain.after-start-op]
        #[test]
        fn prop_start_increases_when_not_max(in_flight in 0u64..u64::MAX) {
            prop_assert!(after_start_op(in_flight) >= in_flight);
            if in_flight < u64::MAX {
                prop_assert_eq!(after_start_op(in_flight), in_flight + 1);
            }
        }

        // r[verify drain.after-finish-op]
        #[test]
        fn prop_finish_decreases_when_positive(in_flight in 1u64..=u64::MAX) {
            prop_assert_eq!(after_finish_op(in_flight), in_flight - 1);
        }

        // r[verify drain.after-finish-op]
        #[test]
        fn prop_finish_floors_at_zero(in_flight: u64) {
            prop_assert!(after_finish_op(in_flight) <= in_flight);
        }
    }
}
