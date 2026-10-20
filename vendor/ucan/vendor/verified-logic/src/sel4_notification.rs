//! Verified seL4-style notification badge accumulation.
//!
//! Models notification pending state as a badge word plus optional binding and
//! waiter state. Signals accumulate by bitwise OR, wait/poll consume the
//! delivered pending badge exactly once, and signal can wake an eligible
//! waiting TCB.

use vstd::prelude::*;

verus! {

#[derive(Copy, Clone)]
pub struct Notification {
    pub pending_badge: u64,
    pub bound_tcb: Option<usize>,
    pub waiting_tcb: Option<usize>,
}

// r[impl sel4-notification.binding]
// r[depends sel4-notification.binding]
pub open spec fn waiter_eligible_spec(notification: Notification, waiter: int) -> bool {
    &&& 0 <= waiter
    &&& match notification.bound_tcb {
        Some(bound) => bound as int == waiter,
        None => true,
    }
}

// r[impl sel4-notification.binding]
// r[depends sel4-notification.binding]
pub open spec fn wake_tcb_spec(notification: Notification) -> Option<usize> {
    match notification.waiting_tcb {
        Some(waiter) => {
            if waiter_eligible_spec(notification, waiter as int) {
                Some(waiter)
            } else {
                None
            }
        }
        None => None,
    }
}

// r[impl sel4-notification.pending-badge]
// r[depends sel4-notification.pending-badge]
pub open spec fn signal_pending_spec(notification: Notification, badge: u64) -> u64 {
    notification.pending_badge | badge
}

// r[impl sel4-notification.consume]
// r[depends sel4-notification.consume]
pub open spec fn delivered_badge_spec(notification: Notification) -> Option<u64> {
    if notification.pending_badge == 0 {
        None
    } else {
        Some(notification.pending_badge)
    }
}

// r[impl sel4-notification.binding]
pub fn new_notification(bound_tcb: Option<usize>) -> (result: Notification)
    ensures
        result.pending_badge == 0,
        result.bound_tcb == bound_tcb,
        result.waiting_tcb.is_none(),
{
    Notification {
        pending_badge: 0,
        bound_tcb,
        waiting_tcb: None,
    }
}

// r[impl sel4-notification.binding]
pub fn bind_tcb(notification: &mut Notification, tcb: usize)
    ensures
        notification.pending_badge == old(notification).pending_badge,
        notification.bound_tcb == Some(tcb),
        notification.waiting_tcb == old(notification).waiting_tcb,
{
    notification.bound_tcb = Some(tcb);
}

// r[impl sel4-notification.binding]
pub fn unbind_tcb(notification: &mut Notification)
    ensures
        notification.pending_badge == old(notification).pending_badge,
        notification.bound_tcb.is_none(),
        notification.waiting_tcb == old(notification).waiting_tcb,
{
    notification.bound_tcb = None;
}

// r[impl sel4-notification.pending-badge]
// r[impl sel4-notification.binding]
pub fn signal(notification: &mut Notification, badge: u64) -> (result: Option<usize>)
    ensures
        notification.pending_badge == signal_pending_spec(*old(notification), badge),
        notification.bound_tcb == old(notification).bound_tcb,
        result == wake_tcb_spec(*old(notification)),
        notification.waiting_tcb == if result.is_some() { None } else { old(notification).waiting_tcb },
{
    let wake = match notification.waiting_tcb {
        Some(waiter) => {
            if match notification.bound_tcb {
                Some(bound) => bound == waiter,
                None => true,
            } {
                Some(waiter)
            } else {
                None
            }
        }
        None => None,
    };
    notification.pending_badge = notification.pending_badge | badge;
    if wake.is_some() {
        notification.waiting_tcb = None;
    }
    wake
}

// r[impl sel4-notification.consume]
// r[impl sel4-notification.binding]
pub fn wait(notification: &mut Notification, waiter: usize) -> (result: Option<u64>)
    ensures
        notification.bound_tcb == old(notification).bound_tcb,
        result == delivered_badge_spec(*old(notification)),
        if old(notification).pending_badge == 0 {
            &&& notification.pending_badge == 0
            &&& notification.waiting_tcb == if waiter_eligible_spec(*old(notification), waiter as int) {
                Some(waiter)
            } else {
                old(notification).waiting_tcb
            }
        } else {
            &&& notification.pending_badge == 0
            &&& notification.waiting_tcb == old(notification).waiting_tcb
        }
{
    if notification.pending_badge == 0 {
        if match notification.bound_tcb {
            Some(bound) => bound == waiter,
            None => true,
        } {
            notification.waiting_tcb = Some(waiter);
        }
        None
    } else {
        let delivered = notification.pending_badge;
        notification.pending_badge = 0;
        Some(delivered)
    }
}

// r[impl sel4-notification.consume]
pub fn poll(notification: &mut Notification) -> (result: Option<u64>)
    ensures
        notification.bound_tcb == old(notification).bound_tcb,
        notification.waiting_tcb == old(notification).waiting_tcb,
        result == delivered_badge_spec(*old(notification)),
        notification.pending_badge == 0,
{
    if notification.pending_badge == 0 {
        None
    } else {
        let delivered = notification.pending_badge;
        notification.pending_badge = 0;
        Some(delivered)
    }
}

// r[verify sel4-notification.pending-badge]
proof fn repeated_signal_accumulates_by_or(notification: Notification, badge1: u64, badge2: u64)
    ensures
        signal_pending_spec(Notification {
            pending_badge: signal_pending_spec(notification, badge1),
            bound_tcb: notification.bound_tcb,
            waiting_tcb: notification.waiting_tcb,
        }, badge2) == notification.pending_badge | badge1 | badge2,
{
}

// r[verify sel4-notification.consume]
proof fn delivery_returns_exact_pending(notification: Notification)
    requires
        notification.pending_badge != 0,
    ensures
        delivered_badge_spec(notification) == Some(notification.pending_badge),
{
}

// r[verify sel4-notification.binding]
proof fn eligible_bound_waiter_wakes(notification: Notification, badge: u64)
    requires
        notification.bound_tcb.is_some(),
        notification.waiting_tcb == notification.bound_tcb,
    ensures
        wake_tcb_spec(notification) == notification.bound_tcb,
        signal_pending_spec(notification, badge) == notification.pending_badge | badge,
{
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify sel4-notification.pending-badge]
    #[test]
    fn test_two_signals_accumulate() {
        let mut notification = new_notification(None);
        assert_eq!(signal(&mut notification, 0x2), None);
        assert_eq!(signal(&mut notification, 0x8), None);
        assert_eq!(notification.pending_badge, 0xA);
    }

    // r[verify sel4-notification.consume]
    #[test]
    fn test_wait_consumes_pending_badge() {
        let mut notification = new_notification(None);
        signal(&mut notification, 0xA);
        let delivered = wait(&mut notification, 7);
        assert_eq!(delivered, Some(0xA));
        assert_eq!(notification.pending_badge, 0);
    }

    // r[verify sel4-notification.consume]
    #[test]
    fn test_poll_consumes_pending_badge() {
        let mut notification = new_notification(None);
        signal(&mut notification, 0x11);
        let delivered = poll(&mut notification);
        assert_eq!(delivered, Some(0x11));
        assert_eq!(notification.pending_badge, 0);
    }

    // r[verify sel4-notification.binding]
    #[test]
    fn test_bound_waiter_wakes_on_signal() {
        let mut notification = new_notification(Some(5));
        assert_eq!(wait(&mut notification, 5), None);
        let wake = signal(&mut notification, 0x20);
        assert_eq!(wake, Some(5));
        assert_eq!(notification.waiting_tcb, None);
        assert_eq!(notification.pending_badge, 0x20);
    }

    // r[verify sel4-notification.binding]
    #[test]
    fn test_ineligible_waiter_does_not_replace_binding() {
        let mut notification = new_notification(Some(5));
        assert_eq!(wait(&mut notification, 7), None);
        assert_eq!(notification.waiting_tcb, None);
        assert_eq!(signal(&mut notification, 0x4), None);
    }

    proptest! {
        // r[verify sel4-notification.pending-badge]
        #[test]
        fn prop_signal_accumulates_or(initial_badge: u64, badge1: u64, badge2: u64) {
            let mut notification = Notification {
                pending_badge: initial_badge,
                bound_tcb: None,
                waiting_tcb: None,
            };
            signal(&mut notification, badge1);
            signal(&mut notification, badge2);
            prop_assert_eq!(notification.pending_badge, initial_badge | badge1 | badge2);
        }

        // r[verify sel4-notification.consume]
        #[test]
        fn prop_wait_returns_exact_pending(pending_badge in 1u64..=u64::MAX, waiter: usize) {
            let mut notification = Notification {
                pending_badge,
                bound_tcb: None,
                waiting_tcb: None,
            };
            let delivered = wait(&mut notification, waiter);
            prop_assert_eq!(delivered, Some(pending_badge));
            prop_assert_eq!(notification.pending_badge, 0);
        }

        // r[verify sel4-notification.binding]
        #[test]
        fn prop_bound_waiter_wakes(bound: usize, badge: u64) {
            let mut notification = new_notification(Some(bound));
            prop_assert_eq!(wait(&mut notification, bound), None);
            let wake = signal(&mut notification, badge);
            prop_assert_eq!(wake, Some(bound));
            prop_assert_eq!(notification.waiting_tcb, None);
            prop_assert_eq!(notification.pending_badge, badge);
        }
    }
}
