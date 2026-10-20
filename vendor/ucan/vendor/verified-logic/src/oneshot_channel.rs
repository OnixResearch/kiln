//! Finite Rust oneshot channel primitives.
//!
//! This module models a single-slot oneshot channel admission boundary. It
//! tracks whether a value can be sent once, received once, closed before send,
//! or cancelled before delivery. The model is finite and does not implement
//! allocation, parking, callbacks, or executor behavior.

use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OneshotState {
    Open,
    Sent { value: u64 },
    Received { value: u64 },
    Closed,
    Cancelled,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OneshotReceive {
    pub value: u64,
    pub state: OneshotState,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OneshotReadinessBridge {
    pub sent_state: OneshotState,
    pub receiver_state: crate::async_task_state::AsyncTaskState,
    pub wake_bridge: crate::task_waker::NoLostWakeBridge,
}

// r[depends rust-primitives.oneshot-channel]
pub open spec fn oneshot_terminal_spec(state: OneshotState) -> bool {
    match state {
        OneshotState::Received { .. } | OneshotState::Closed | OneshotState::Cancelled => true,
        OneshotState::Open | OneshotState::Sent { .. } => false,
    }
}

// r[depends rust-primitives.oneshot-channel]
pub open spec fn oneshot_send_spec(state: OneshotState, value: u64) -> Option<OneshotState> {
    if state == OneshotState::Open {
        Some(OneshotState::Sent { value })
    } else {
        None
    }
}

// r[depends rust-primitives.oneshot-channel]
pub open spec fn oneshot_receive_spec(state: OneshotState) -> Option<OneshotReceive> {
    match state {
        OneshotState::Sent { value } => Some(OneshotReceive {
            value,
            state: OneshotState::Received { value },
        }),
        OneshotState::Open
        | OneshotState::Received { .. }
        | OneshotState::Closed
        | OneshotState::Cancelled => None,
    }
}

// r[depends rust-primitives.oneshot-channel]
pub open spec fn oneshot_close_spec(state: OneshotState) -> Option<OneshotState> {
    if state == OneshotState::Open {
        Some(OneshotState::Closed)
    } else {
        None
    }
}

// r[depends rust-primitives.oneshot-channel]
pub open spec fn oneshot_cancel_spec(state: OneshotState) -> Option<OneshotState> {
    match state {
        OneshotState::Open | OneshotState::Sent { .. } => Some(OneshotState::Cancelled),
        OneshotState::Received { .. } | OneshotState::Closed | OneshotState::Cancelled => None,
    }
}

// r[depends rust-primitives.oneshot-readiness]
pub open spec fn oneshot_send_readiness_spec(
    channel: OneshotState,
    value: u64,
    receiver: crate::async_task_state::AsyncTaskState,
    slot: crate::task_waker::WakerSlot,
    generation: u64,
) -> Option<OneshotReadinessBridge> {
    match oneshot_send_spec(channel, value) {
        Some(sent_state) => match crate::async_task_state::async_task_transition_spec(
            receiver,
            crate::async_task_state::AsyncTaskAction::MarkReady,
        ) {
            Some(receiver_state) => match crate::task_waker::no_lost_wake_after_readiness_spec(
                receiver,
                slot,
                generation,
            ) {
                Some(wake_bridge) => Some(OneshotReadinessBridge {
                    sent_state,
                    receiver_state,
                    wake_bridge,
                }),
                None => None,
            },
            None => None,
        },
        None => None,
    }
}

// r[impl rust-primitives.oneshot-channel]
pub fn oneshot_is_terminal(state: OneshotState) -> (result: bool)
    ensures result == oneshot_terminal_spec(state)
{
    match state {
        OneshotState::Open | OneshotState::Sent { .. } => false,
        OneshotState::Received { .. } | OneshotState::Closed | OneshotState::Cancelled => true,
    }
}

// r[impl rust-primitives.oneshot-channel]
pub fn oneshot_send(state: OneshotState, value: u64) -> (result: Option<OneshotState>)
    ensures result == oneshot_send_spec(state, value)
{
    match state {
        OneshotState::Open => Some(OneshotState::Sent { value }),
        OneshotState::Sent { .. }
        | OneshotState::Received { .. }
        | OneshotState::Closed
        | OneshotState::Cancelled => None,
    }
}

// r[impl rust-primitives.oneshot-channel]
pub fn oneshot_receive(state: OneshotState) -> (result: Option<OneshotReceive>)
    ensures result == oneshot_receive_spec(state)
{
    match state {
        OneshotState::Sent { value } => Some(OneshotReceive {
            value,
            state: OneshotState::Received { value },
        }),
        OneshotState::Open
        | OneshotState::Received { .. }
        | OneshotState::Closed
        | OneshotState::Cancelled => None,
    }
}

// r[impl rust-primitives.oneshot-channel]
pub fn oneshot_close(state: OneshotState) -> (result: Option<OneshotState>)
    ensures result == oneshot_close_spec(state)
{
    match state {
        OneshotState::Open => Some(OneshotState::Closed),
        OneshotState::Sent { .. }
        | OneshotState::Received { .. }
        | OneshotState::Closed
        | OneshotState::Cancelled => None,
    }
}

// r[impl rust-primitives.oneshot-channel]
pub fn oneshot_cancel(state: OneshotState) -> (result: Option<OneshotState>)
    ensures result == oneshot_cancel_spec(state)
{
    match state {
        OneshotState::Open | OneshotState::Sent { .. } => Some(OneshotState::Cancelled),
        OneshotState::Received { .. } | OneshotState::Closed | OneshotState::Cancelled => None,
    }
}

// r[impl rust-primitives.oneshot-readiness]
pub fn oneshot_send_readiness(
    channel: OneshotState,
    value: u64,
    receiver: crate::async_task_state::AsyncTaskState,
    slot: crate::task_waker::WakerSlot,
    generation: u64,
) -> (result: Option<OneshotReadinessBridge>)
    ensures result == oneshot_send_readiness_spec(channel, value, receiver, slot, generation)
{
    match oneshot_send(channel, value) {
        Some(sent_state) => match crate::async_task_state::async_task_mark_ready(receiver) {
            Some(receiver_state) => match crate::task_waker::no_lost_wake_after_readiness(
                receiver,
                slot,
                generation,
            ) {
                Some(wake_bridge) => Some(OneshotReadinessBridge {
                    sent_state,
                    receiver_state,
                    wake_bridge,
                }),
                None => None,
            },
            None => None,
        },
        None => None,
    }
}

// r[verify rust-primitives.oneshot-channel]
pub proof fn send_then_receive_consumes_one_value(value: u64)
    ensures oneshot_receive_spec(OneshotState::Sent { value }) == Some(OneshotReceive {
            value,
            state: OneshotState::Received { value },
        }),
        oneshot_receive_spec(OneshotState::Received { value }) == None::<OneshotReceive>,
        oneshot_terminal_spec(OneshotState::Received { value })
{}

// r[verify rust-primitives.oneshot-channel]
pub proof fn open_send_succeeds_once(value: u64)
    ensures oneshot_send_spec(OneshotState::Open, value) == Some(OneshotState::Sent { value }),
        oneshot_send_spec(OneshotState::Sent { value }, value) == None::<OneshotState>
{}

// r[verify rust-primitives.oneshot-channel]
pub proof fn closed_channel_rejects_send(value: u64)
    ensures oneshot_send_spec(OneshotState::Closed, value) == None::<OneshotState>,
        oneshot_terminal_spec(OneshotState::Closed)
{}

// r[verify rust-primitives.oneshot-channel]
pub proof fn cancelled_receiver_prevents_duplicate_delivery(value: u64)
    ensures oneshot_cancel_spec(OneshotState::Sent { value }) == Some(OneshotState::Cancelled),
        oneshot_receive_spec(OneshotState::Cancelled) == None::<OneshotReceive>,
        oneshot_terminal_spec(OneshotState::Cancelled)
{}

// r[verify rust-primitives.oneshot-readiness]
pub proof fn successful_send_wakes_registered_receiver(value: u64, generation: u64)
    ensures oneshot_send_readiness_spec(
        OneshotState::Open,
        value,
        crate::async_task_state::AsyncTaskState::Pending,
        crate::task_waker::WakerSlot::Registered { generation },
        generation,
    ) == Some(OneshotReadinessBridge {
        sent_state: OneshotState::Sent { value },
        receiver_state: crate::async_task_state::AsyncTaskState::Ready,
        wake_bridge: crate::task_waker::NoLostWakeBridge {
            observed_ready: true,
            slot: crate::task_waker::WakerSlot::Notified { generation },
        },
    })
{}

// r[verify rust-primitives.oneshot-readiness]
pub proof fn failed_send_does_not_create_readiness(value: u64, generation: u64)
    ensures oneshot_send_readiness_spec(
        OneshotState::Closed,
        value,
        crate::async_task_state::AsyncTaskState::Pending,
        crate::task_waker::WakerSlot::Registered { generation },
        generation,
    ) == None::<OneshotReadinessBridge>,
        oneshot_send_readiness_spec(
            OneshotState::Sent { value },
            value,
            crate::async_task_state::AsyncTaskState::Pending,
            crate::task_waker::WakerSlot::Registered { generation },
            generation,
        ) == None::<OneshotReadinessBridge>
{}

// r[verify rust-primitives.oneshot-readiness]
pub proof fn successful_oneshot_send_satisfies_no_lost_wake(value: u64, generation: u64)
    ensures crate::task_waker::no_lost_wake_satisfied_spec(
        crate::async_task_state::AsyncTaskState::Pending,
        crate::task_waker::WakerSlot::Registered { generation },
        generation,
        crate::task_waker::NoLostWakeBridge {
            observed_ready: true,
            slot: crate::task_waker::WakerSlot::Notified { generation },
        },
    ),
        oneshot_send_readiness_spec(
            OneshotState::Open,
            value,
            crate::async_task_state::AsyncTaskState::Pending,
            crate::task_waker::WakerSlot::Registered { generation },
            generation,
        ) == Some(OneshotReadinessBridge {
            sent_state: OneshotState::Sent { value },
            receiver_state: crate::async_task_state::AsyncTaskState::Ready,
            wake_bridge: crate::task_waker::NoLostWakeBridge {
                observed_ready: true,
                slot: crate::task_waker::WakerSlot::Notified { generation },
            },
        })
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_then_receive_consumes_one_value_at_runtime() {
        let sent = oneshot_send(OneshotState::Open, 42);
        assert_eq!(sent, Some(OneshotState::Sent { value: 42 }));
        assert_eq!(
            oneshot_receive(sent.unwrap()),
            Some(OneshotReceive {
                value: 42,
                state: OneshotState::Received { value: 42 },
            })
        );
        assert_eq!(oneshot_receive(OneshotState::Received { value: 42 }), None);
    }

    #[test]
    fn send_succeeds_at_most_once() {
        assert_eq!(oneshot_send(OneshotState::Open, 7), Some(OneshotState::Sent { value: 7 }));
        assert_eq!(oneshot_send(OneshotState::Sent { value: 7 }, 8), None);
    }

    #[test]
    fn closed_channel_rejects_send() {
        assert_eq!(oneshot_close(OneshotState::Open), Some(OneshotState::Closed));
        assert_eq!(oneshot_send(OneshotState::Closed, 9), None);
        assert!(oneshot_is_terminal(OneshotState::Closed));
    }

    #[test]
    fn cancelled_receiver_prevents_duplicate_delivery() {
        assert_eq!(oneshot_cancel(OneshotState::Sent { value: 10 }), Some(OneshotState::Cancelled));
        assert_eq!(oneshot_receive(OneshotState::Cancelled), None);
        assert!(oneshot_is_terminal(OneshotState::Cancelled));
    }

    #[test]
    fn successful_send_wakes_registered_receiver_at_runtime() {
        assert_eq!(
            oneshot_send_readiness(
                OneshotState::Open,
                14,
                crate::async_task_state::AsyncTaskState::Pending,
                crate::task_waker::WakerSlot::Registered { generation: 5 },
                5,
            ),
            Some(OneshotReadinessBridge {
                sent_state: OneshotState::Sent { value: 14 },
                receiver_state: crate::async_task_state::AsyncTaskState::Ready,
                wake_bridge: crate::task_waker::NoLostWakeBridge {
                    observed_ready: true,
                    slot: crate::task_waker::WakerSlot::Notified { generation: 5 },
                },
            })
        );
    }

    #[test]
    fn failed_send_does_not_create_receiver_readiness() {
        assert_eq!(
            oneshot_send_readiness(
                OneshotState::Closed,
                15,
                crate::async_task_state::AsyncTaskState::Pending,
                crate::task_waker::WakerSlot::Registered { generation: 6 },
                6,
            ),
            None
        );
        assert_eq!(
            oneshot_send_readiness(
                OneshotState::Sent { value: 15 },
                16,
                crate::async_task_state::AsyncTaskState::Pending,
                crate::task_waker::WakerSlot::Registered { generation: 6 },
                6,
            ),
            None
        );
    }
}
