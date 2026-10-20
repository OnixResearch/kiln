//! Finite Rust task-waker slot primitives.
//!
//! This module models registration and notification as generation-tagged slot
//! state. It deliberately records only observable finite state: no callback
//! execution, raw-waker vtables, scheduler fairness, or OS wake delivery.

use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WakerSlot {
    Empty,
    Registered { generation: u64 },
    Notified { generation: u64 },
    Taken { generation: u64 },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NoLostWakeBridge {
    pub observed_ready: bool,
    pub slot: WakerSlot,
}

// r[depends rust-primitives.task-waker]
pub open spec fn waker_slot_generation_spec(slot: WakerSlot) -> Option<u64> {
    match slot {
        WakerSlot::Empty => None,
        WakerSlot::Registered { generation } => Some(generation),
        WakerSlot::Notified { generation } => Some(generation),
        WakerSlot::Taken { generation } => Some(generation),
    }
}

// r[depends rust-primitives.task-waker]
pub open spec fn waker_slot_pending_spec(slot: WakerSlot, generation: u64) -> bool {
    slot == WakerSlot::Notified { generation }
}

// r[depends rust-primitives.task-waker]
pub open spec fn waker_register_spec(slot: WakerSlot, generation: u64) -> WakerSlot {
    WakerSlot::Registered { generation }
}

// r[depends rust-primitives.task-waker]
pub open spec fn waker_wake_spec(slot: WakerSlot, generation: u64) -> Option<WakerSlot> {
    match slot {
        WakerSlot::Registered { generation: current } => if current == generation {
            Some(WakerSlot::Notified { generation })
        } else {
            None
        },
        WakerSlot::Notified { generation: current } => if current == generation {
            Some(WakerSlot::Notified { generation })
        } else {
            None
        },
        WakerSlot::Empty => None,
        WakerSlot::Taken { .. } => None,
    }
}

// r[depends rust-primitives.task-waker]
pub open spec fn waker_take_spec(slot: WakerSlot) -> Option<WakerSlot> {
    match slot {
        WakerSlot::Notified { generation } => Some(WakerSlot::Taken { generation }),
        WakerSlot::Empty => None,
        WakerSlot::Registered { .. } => None,
        WakerSlot::Taken { .. } => None,
    }
}

// r[depends rust-primitives.no-lost-wake]
pub open spec fn no_lost_wake_after_readiness_spec(
    task: crate::async_task_state::AsyncTaskState,
    slot: WakerSlot,
    generation: u64,
) -> Option<NoLostWakeBridge> {
    if task == crate::async_task_state::AsyncTaskState::Ready {
        Some(NoLostWakeBridge { observed_ready: true, slot })
    } else if task == crate::async_task_state::AsyncTaskState::Pending {
        match waker_wake_spec(slot, generation) {
            Some(next_slot) => Some(NoLostWakeBridge { observed_ready: true, slot: next_slot }),
            None => None,
        }
    } else {
        None
    }
}

// r[depends rust-primitives.no-lost-wake]
pub open spec fn no_lost_wake_satisfied_spec(
    task: crate::async_task_state::AsyncTaskState,
    slot: WakerSlot,
    generation: u64,
    bridge: NoLostWakeBridge,
) -> bool {
    no_lost_wake_after_readiness_spec(task, slot, generation) == Some(bridge)
        && bridge.observed_ready
        && (bridge.slot == WakerSlot::Notified { generation }
            || task == crate::async_task_state::AsyncTaskState::Ready)
}

// r[impl rust-primitives.task-waker]
pub fn waker_slot_generation(slot: WakerSlot) -> (result: Option<u64>)
    ensures result == waker_slot_generation_spec(slot)
{
    match slot {
        WakerSlot::Empty => None,
        WakerSlot::Registered { generation }
        | WakerSlot::Notified { generation }
        | WakerSlot::Taken { generation } => Some(generation),
    }
}

// r[impl rust-primitives.task-waker]
pub fn waker_register(slot: WakerSlot, generation: u64) -> (result: WakerSlot)
    ensures result == waker_register_spec(slot, generation)
{
    match slot {
        WakerSlot::Empty
        | WakerSlot::Registered { .. }
        | WakerSlot::Notified { .. }
        | WakerSlot::Taken { .. } => WakerSlot::Registered { generation },
    }
}

// r[impl rust-primitives.task-waker]
pub fn waker_wake(slot: WakerSlot, generation: u64) -> (result: Option<WakerSlot>)
    ensures result == waker_wake_spec(slot, generation)
{
    match slot {
        WakerSlot::Registered { generation: current }
        | WakerSlot::Notified { generation: current } => if current == generation {
            Some(WakerSlot::Notified { generation })
        } else {
            None
        },
        WakerSlot::Empty | WakerSlot::Taken { .. } => None,
    }
}

// r[impl rust-primitives.task-waker]
pub fn waker_take(slot: WakerSlot) -> (result: Option<WakerSlot>)
    ensures result == waker_take_spec(slot)
{
    match slot {
        WakerSlot::Notified { generation } => Some(WakerSlot::Taken { generation }),
        WakerSlot::Empty | WakerSlot::Registered { .. } | WakerSlot::Taken { .. } => None,
    }
}

// r[impl rust-primitives.no-lost-wake]
pub fn no_lost_wake_after_readiness(
    task: crate::async_task_state::AsyncTaskState,
    slot: WakerSlot,
    generation: u64,
) -> (result: Option<NoLostWakeBridge>)
    ensures result == no_lost_wake_after_readiness_spec(task, slot, generation)
{
    match task {
        crate::async_task_state::AsyncTaskState::Ready => Some(NoLostWakeBridge {
            observed_ready: true,
            slot,
        }),
        crate::async_task_state::AsyncTaskState::Pending => match waker_wake(slot, generation) {
            Some(next_slot) => Some(NoLostWakeBridge {
                observed_ready: true,
                slot: next_slot,
            }),
            None => None,
        },
        crate::async_task_state::AsyncTaskState::Completed
        | crate::async_task_state::AsyncTaskState::Cancelled => None,
    }
}

// r[verify rust-primitives.task-waker]
pub proof fn wake_after_registration_is_observable(generation: u64)
    ensures waker_wake_spec(waker_register_spec(WakerSlot::Empty, generation), generation)
        == Some(WakerSlot::Notified { generation }),
        waker_slot_pending_spec(WakerSlot::Notified { generation }, generation)
{}

// r[verify rust-primitives.task-waker]
pub proof fn repeated_wake_is_idempotent(generation: u64)
    ensures waker_wake_spec(WakerSlot::Notified { generation }, generation)
        == Some(WakerSlot::Notified { generation })
{}

// r[verify rust-primitives.task-waker]
pub proof fn replaced_waker_ignores_stale_generation(old_generation: u64, new_generation: u64)
    requires old_generation != new_generation
    ensures waker_wake_spec(waker_register_spec(WakerSlot::Registered { generation: old_generation }, new_generation), old_generation)
        == None::<WakerSlot>
{}

// r[verify rust-primitives.task-waker]
pub proof fn taking_notification_consumes_at_most_one(generation: u64)
    ensures waker_take_spec(WakerSlot::Notified { generation }) == Some(WakerSlot::Taken { generation }),
        waker_take_spec(WakerSlot::Taken { generation }) == None::<WakerSlot>
{}
// r[verify rust-primitives.no-lost-wake]
pub proof fn registered_waiter_observes_later_readiness(generation: u64)
    ensures no_lost_wake_satisfied_spec(
        crate::async_task_state::AsyncTaskState::Pending,
        WakerSlot::Registered { generation },
        generation,
        NoLostWakeBridge { observed_ready: true, slot: WakerSlot::Notified { generation } },
    )
{}

// r[verify rust-primitives.no-lost-wake]
pub proof fn ready_before_register_needs_no_new_wake(generation: u64)
    ensures no_lost_wake_satisfied_spec(
        crate::async_task_state::AsyncTaskState::Ready,
        WakerSlot::Registered { generation },
        generation,
        NoLostWakeBridge { observed_ready: true, slot: WakerSlot::Registered { generation } },
    )
{}

// r[verify rust-primitives.no-lost-wake]
pub proof fn terminal_task_creates_no_wake_bridge(generation: u64)
    ensures no_lost_wake_after_readiness_spec(
            crate::async_task_state::AsyncTaskState::Completed,
            WakerSlot::Registered { generation },
            generation,
        ) == None::<NoLostWakeBridge>
{}

// r[verify rust-primitives.no-lost-wake]
pub proof fn pending_mark_ready_notifies_registered_waiter(generation: u64)
    ensures crate::async_task_state::async_task_transition_spec(
            crate::async_task_state::AsyncTaskState::Pending,
            crate::async_task_state::AsyncTaskAction::MarkReady,
        ) == Some(crate::async_task_state::AsyncTaskState::Ready),
        no_lost_wake_satisfied_spec(
            crate::async_task_state::AsyncTaskState::Pending,
            WakerSlot::Registered { generation },
            generation,
            NoLostWakeBridge { observed_ready: true, slot: WakerSlot::Notified { generation } },
        )
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wake_after_registration_records_pending_generation() {
        let registered = waker_register(WakerSlot::Empty, 7);
        assert_eq!(registered, WakerSlot::Registered { generation: 7 });
        assert_eq!(waker_wake(registered, 7), Some(WakerSlot::Notified { generation: 7 }));
    }

    #[test]
    fn repeated_wake_is_idempotent() {
        let notified = WakerSlot::Notified { generation: 9 };
        assert_eq!(waker_wake(notified, 9), Some(notified));
        assert_eq!(waker_take(notified), Some(WakerSlot::Taken { generation: 9 }));
        assert_eq!(waker_take(WakerSlot::Taken { generation: 9 }), None);
    }

    #[test]
    fn replacement_rejects_stale_generation() {
        let replaced = waker_register(WakerSlot::Registered { generation: 1 }, 2);
        assert_eq!(replaced, WakerSlot::Registered { generation: 2 });
        assert_eq!(waker_wake(replaced, 1), None);
        assert_eq!(waker_wake(replaced, 2), Some(WakerSlot::Notified { generation: 2 }));
    }

    #[test]
    fn empty_or_registered_take_has_no_notification() {
        assert_eq!(waker_take(WakerSlot::Empty), None);
        assert_eq!(waker_take(WakerSlot::Registered { generation: 3 }), None);
    }

    #[test]
    fn no_lost_wake_notifies_registered_waiter_after_readiness() {
        let bridge = no_lost_wake_after_readiness(
            crate::async_task_state::AsyncTaskState::Pending,
            WakerSlot::Registered { generation: 11 },
            11,
        );
        assert_eq!(
            bridge,
            Some(NoLostWakeBridge {
                observed_ready: true,
                slot: WakerSlot::Notified { generation: 11 },
            })
        );
    }

    #[test]
    fn ready_before_register_needs_no_new_notification() {
        let slot = WakerSlot::Registered { generation: 12 };
        assert_eq!(
            no_lost_wake_after_readiness(crate::async_task_state::AsyncTaskState::Ready, slot, 12),
            Some(NoLostWakeBridge {
                observed_ready: true,
                slot,
            })
        );
    }

    #[test]
    fn terminal_task_does_not_create_wake_bridge() {
        assert_eq!(
            no_lost_wake_after_readiness(
                crate::async_task_state::AsyncTaskState::Completed,
                WakerSlot::Registered { generation: 13 },
                13,
            ),
            None
        );
    }
}
