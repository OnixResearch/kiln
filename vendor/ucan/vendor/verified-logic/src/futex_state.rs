//! Verified futex wait/wake state machine.
//!
//! Models the three states of a futex-based synchronization primitive:
//! idle (no waiters), waiting (thread parked on the futex word),
//! and notified (wake signal pending). The futex word is a u32 that
//! threads compare-and-swap to coordinate.

use vstd::prelude::*;

verus! {

pub const FUTEX_IDLE: u32 = 0u32;
pub const FUTEX_WAITING: u32 = 1u32;
pub const FUTEX_NOTIFIED: u32 = 2u32;

// --- is_valid_state ---

// r[depends futex_state.valid-state]
/// A futex word holds a valid state.
pub open spec fn is_valid_state_spec(state: u32) -> bool {
    state == FUTEX_IDLE || state == FUTEX_WAITING || state == FUTEX_NOTIFIED
}

// r[impl futex_state.valid-state]
pub fn is_valid_state(state: u32) -> (result: bool)
    ensures result == is_valid_state_spec(state)
{
    state == FUTEX_IDLE || state == FUTEX_WAITING || state == FUTEX_NOTIFIED
}

// --- should_park ---

// r[depends futex_state.should-park]
/// A thread should park (block) when the futex word indicates
/// no notification is pending. The thread first CAS's idle → waiting,
/// then checks: if still waiting, park.
pub open spec fn should_park_spec(state: u32) -> bool {
    state == FUTEX_WAITING
}

// r[impl futex_state.should-park]
pub fn should_park(state: u32) -> (result: bool)
    ensures result == should_park_spec(state)
{
    state == FUTEX_WAITING
}

// --- try_wait ---

// r[depends futex_state.try-wait]
/// Attempt to transition from idle to waiting. Returns the new state
/// if successful, or the current state if already waiting/notified.
pub open spec fn try_wait_spec(state: u32) -> u32 {
    if state == FUTEX_IDLE { FUTEX_WAITING }
    else if state == FUTEX_NOTIFIED { FUTEX_IDLE } // consume notification
    else { state } // already waiting
}

// r[impl futex_state.try-wait]
pub fn try_wait(state: u32) -> (result: u32)
    requires is_valid_state_spec(state)
    ensures result == try_wait_spec(state)
{
    if state == FUTEX_IDLE { FUTEX_WAITING }
    else if state == FUTEX_NOTIFIED { FUTEX_IDLE }
    else { state }
}

// --- notify ---

// r[depends futex_state.notify]
/// Wake one waiter. If waiting, transitions to notified. If idle,
/// sets notified (pre-notification). If already notified, no change.
pub open spec fn notify_spec(state: u32) -> u32 {
    if state == FUTEX_WAITING { FUTEX_NOTIFIED }
    else if state == FUTEX_IDLE { FUTEX_NOTIFIED }
    else { FUTEX_NOTIFIED } // already notified
}

// r[impl futex_state.notify]
pub fn notify(state: u32) -> (result: u32)
    requires is_valid_state_spec(state)
    ensures result == notify_spec(state)
{
    FUTEX_NOTIFIED
}

// --- consume_notification ---

// r[depends futex_state.consume]
/// After waking, consume the notification: notified → idle.
pub open spec fn consume_notification_spec(state: u32) -> u32 {
    if state == FUTEX_NOTIFIED { FUTEX_IDLE } else { state }
}

// r[impl futex_state.consume]
pub fn consume_notification(state: u32) -> (result: u32)
    requires is_valid_state_spec(state)
    ensures result == consume_notification_spec(state)
{
    if state == FUTEX_NOTIFIED { FUTEX_IDLE } else { state }
}

// --- has_waiters ---

// r[depends futex_state.has-waiters]
/// At least one thread is waiting on this futex.
pub open spec fn has_waiters_spec(state: u32) -> bool {
    state == FUTEX_WAITING
}

// r[impl futex_state.has-waiters]
pub fn has_waiters(state: u32) -> (result: bool)
    ensures result == has_waiters_spec(state)
{
    state == FUTEX_WAITING
}

// --- is_notified ---

// r[depends futex_state.is-notified]
/// A notification is pending.
pub open spec fn is_notified_spec(state: u32) -> bool {
    state == FUTEX_NOTIFIED
}

// r[impl futex_state.is-notified]
pub fn is_notified(state: u32) -> (result: bool)
    ensures result == is_notified_spec(state)
{
    state == FUTEX_NOTIFIED
}

// --- waiter_count_estimate ---

// r[depends futex_state.waiter-count]
/// Estimated waiter count from state (0 or 1 for single-waiter futex).
pub open spec fn waiter_count_estimate_spec(state: u32) -> u32 {
    if state == FUTEX_WAITING { 1 } else { 0 }
}

// r[impl futex_state.waiter-count]
pub fn waiter_count_estimate(state: u32) -> (result: u32)
    ensures result == waiter_count_estimate_spec(state)
{
    if state == FUTEX_WAITING { 1 } else { 0 }
}

// === Proofs ===

// r[verify futex_state.valid-state]
/// All three constants are valid states.
proof fn constants_valid()
    ensures
        is_valid_state_spec(FUTEX_IDLE),
        is_valid_state_spec(FUTEX_WAITING),
        is_valid_state_spec(FUTEX_NOTIFIED),
{}

// r[verify futex_state.valid-state]
/// Constants are distinct.
proof fn constants_distinct()
    ensures
        FUTEX_IDLE != FUTEX_WAITING,
        FUTEX_IDLE != FUTEX_NOTIFIED,
        FUTEX_WAITING != FUTEX_NOTIFIED,
{}

// r[verify futex_state.try-wait]
/// try_wait preserves validity.
proof fn try_wait_preserves_valid(state: u32)
    requires is_valid_state_spec(state)
    ensures is_valid_state_spec(try_wait_spec(state))
{}

// r[verify futex_state.notify]
/// notify preserves validity.
proof fn notify_preserves_valid(state: u32)
    requires is_valid_state_spec(state)
    ensures is_valid_state_spec(notify_spec(state))
{}

// r[verify futex_state.consume]
/// consume preserves validity.
proof fn consume_preserves_valid(state: u32)
    requires is_valid_state_spec(state)
    ensures is_valid_state_spec(consume_notification_spec(state))
{}

// r[verify futex_state.notify]
/// notify always produces NOTIFIED.
proof fn notify_always_notified(state: u32)
    requires is_valid_state_spec(state)
    ensures notify_spec(state) == FUTEX_NOTIFIED
{}

// r[verify futex_state.notify]
/// notify is idempotent.
proof fn notify_idempotent(state: u32)
    requires is_valid_state_spec(state)
    ensures notify_spec(notify_spec(state)) == notify_spec(state)
{}

// r[verify futex_state.try-wait]
// r[verify futex_state.notify]
/// Wait then notify transitions idle → waiting → notified.
proof fn wait_notify_sequence()
    ensures notify_spec(try_wait_spec(FUTEX_IDLE)) == FUTEX_NOTIFIED
{}

// r[verify futex_state.consume]
// r[verify futex_state.notify]
/// Notify then consume returns to idle.
proof fn notify_consume_roundtrip(state: u32)
    requires is_valid_state_spec(state)
    ensures consume_notification_spec(notify_spec(state)) == FUTEX_IDLE
{}

// r[verify futex_state.try-wait]
/// try_wait on idle produces waiting.
proof fn wait_idle_becomes_waiting()
    ensures try_wait_spec(FUTEX_IDLE) == FUTEX_WAITING
{}

// r[verify futex_state.try-wait]
/// try_wait on notified consumes notification (returns idle).
proof fn wait_notified_consumes()
    ensures try_wait_spec(FUTEX_NOTIFIED) == FUTEX_IDLE
{}

// r[verify futex_state.try-wait]
/// try_wait on waiting is a no-op.
proof fn wait_waiting_noop()
    ensures try_wait_spec(FUTEX_WAITING) == FUTEX_WAITING
{}

// r[verify futex_state.should-park]
// r[verify futex_state.try-wait]
/// After try_wait from idle, should park.
proof fn wait_from_idle_parks()
    ensures should_park_spec(try_wait_spec(FUTEX_IDLE))
{}

// r[verify futex_state.should-park]
// r[verify futex_state.try-wait]
/// After try_wait from notified, should NOT park (notification consumed).
proof fn wait_from_notified_no_park()
    ensures !should_park_spec(try_wait_spec(FUTEX_NOTIFIED))
{}

// r[verify futex_state.has-waiters]
// r[verify futex_state.notify]
/// After notify, no waiters (they've been woken).
proof fn notify_clears_waiters(state: u32)
    requires is_valid_state_spec(state)
    ensures !has_waiters_spec(notify_spec(state))
{}

// r[verify futex_state.waiter-count]
/// Waiter count is 0 or 1.
proof fn waiter_count_bounded(state: u32)
    ensures waiter_count_estimate_spec(state) <= 1
{}

// r[verify futex_state.is-notified]
// r[verify futex_state.consume]
/// After consuming, not notified.
proof fn consume_clears_notified(state: u32)
    requires is_valid_state_spec(state)
    ensures !is_notified_spec(consume_notification_spec(state))
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn test_valid_states() {
        assert!(is_valid_state(FUTEX_IDLE));
        assert!(is_valid_state(FUTEX_WAITING));
        assert!(is_valid_state(FUTEX_NOTIFIED));
        assert!(!is_valid_state(3));
        assert!(!is_valid_state(u32::MAX));
    }

    #[test]
    fn test_try_wait() {
        assert_eq!(try_wait(FUTEX_IDLE), FUTEX_WAITING);
        assert_eq!(try_wait(FUTEX_NOTIFIED), FUTEX_IDLE);
        assert_eq!(try_wait(FUTEX_WAITING), FUTEX_WAITING);
    }

    #[test]
    fn test_notify() {
        assert_eq!(notify(FUTEX_IDLE), FUTEX_NOTIFIED);
        assert_eq!(notify(FUTEX_WAITING), FUTEX_NOTIFIED);
        assert_eq!(notify(FUTEX_NOTIFIED), FUTEX_NOTIFIED);
    }

    #[test]
    fn test_consume() {
        assert_eq!(consume_notification(FUTEX_NOTIFIED), FUTEX_IDLE);
        assert_eq!(consume_notification(FUTEX_IDLE), FUTEX_IDLE);
        assert_eq!(consume_notification(FUTEX_WAITING), FUTEX_WAITING);
    }

    #[test]
    fn test_should_park() {
        assert!(!should_park(FUTEX_IDLE));
        assert!(should_park(FUTEX_WAITING));
        assert!(!should_park(FUTEX_NOTIFIED));
    }

    #[test]
    fn test_scenario_wait_notify_consume() {
        let s0 = FUTEX_IDLE;
        let s1 = try_wait(s0); // idle → waiting
        assert_eq!(s1, FUTEX_WAITING);
        assert!(should_park(s1));

        let s2 = notify(s1); // waiting → notified
        assert_eq!(s2, FUTEX_NOTIFIED);
        assert!(!should_park(s2));

        let s3 = consume_notification(s2); // notified → idle
        assert_eq!(s3, FUTEX_IDLE);
    }

    #[test]
    fn test_scenario_pre_notify() {
        // Notify before wait: notification consumed immediately
        let s0 = FUTEX_IDLE;
        let s1 = notify(s0); // idle → notified
        let s2 = try_wait(s1); // notified → idle (consumed)
        assert_eq!(s2, FUTEX_IDLE);
        assert!(!should_park(s2));
    }

    #[test]
    fn test_waiter_count() {
        assert_eq!(waiter_count_estimate(FUTEX_IDLE), 0);
        assert_eq!(waiter_count_estimate(FUTEX_WAITING), 1);
        assert_eq!(waiter_count_estimate(FUTEX_NOTIFIED), 0);
    }

    proptest! {
        #[test]
        fn prop_try_wait_valid(state in prop::sample::select(vec![FUTEX_IDLE, FUTEX_WAITING, FUTEX_NOTIFIED])) {
            prop_assert!(is_valid_state(try_wait(state)));
        }

        #[test]
        fn prop_notify_valid(state in prop::sample::select(vec![FUTEX_IDLE, FUTEX_WAITING, FUTEX_NOTIFIED])) {
            prop_assert!(is_valid_state(notify(state)));
        }

        #[test]
        fn prop_consume_valid(state in prop::sample::select(vec![FUTEX_IDLE, FUTEX_WAITING, FUTEX_NOTIFIED])) {
            prop_assert!(is_valid_state(consume_notification(state)));
        }

        #[test]
        fn prop_notify_always_notified(state in prop::sample::select(vec![FUTEX_IDLE, FUTEX_WAITING, FUTEX_NOTIFIED])) {
            prop_assert_eq!(notify(state), FUTEX_NOTIFIED);
        }

        #[test]
        fn prop_notify_idempotent(state in prop::sample::select(vec![FUTEX_IDLE, FUTEX_WAITING, FUTEX_NOTIFIED])) {
            prop_assert_eq!(notify(notify(state)), notify(state));
        }

        #[test]
        fn prop_notify_consume_idle(state in prop::sample::select(vec![FUTEX_IDLE, FUTEX_WAITING, FUTEX_NOTIFIED])) {
            prop_assert_eq!(consume_notification(notify(state)), FUTEX_IDLE);
        }

        #[test]
        fn prop_waiter_count_bounded(state: u32) {
            prop_assert!(waiter_count_estimate(state) <= 1);
        }

        #[test]
        fn prop_consume_clears_notified(state in prop::sample::select(vec![FUTEX_IDLE, FUTEX_WAITING, FUTEX_NOTIFIED])) {
            prop_assert!(!is_notified(consume_notification(state)));
        }
    }
}
