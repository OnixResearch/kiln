//! Verified seL4-style TCB state machine.
//!
//! Models one thread control block with explicit runnable/running states plus
//! blocked variants that carry the responsible kernel object. Transition
//! helpers are total: invalid transitions return `false` and leave the TCB
//! unchanged.

use vstd::prelude::*;

verus! {

#[derive(Copy, Clone)]
pub enum TcbState {
    Runnable,
    Running,
    BlockedOnSend { endpoint: usize },
    BlockedOnReceive { endpoint: usize },
    BlockedOnNotification { notification: usize },
    BlockedOnReply { reply: usize },
    Restart,
    Inactive,
}

#[derive(Copy, Clone)]
pub struct Tcb {
    pub tid: usize,
    pub priority: u8,
    pub state: TcbState,
}

// r[impl sel4-tcb.state]
// r[depends sel4-tcb.state]
pub open spec fn legal_state_spec(state: TcbState) -> bool {
    match state {
        TcbState::Runnable => true,
        TcbState::Running => true,
        TcbState::BlockedOnSend { endpoint: _ } => true,
        TcbState::BlockedOnReceive { endpoint: _ } => true,
        TcbState::BlockedOnNotification { notification: _ } => true,
        TcbState::BlockedOnReply { reply: _ } => true,
        TcbState::Restart => true,
        TcbState::Inactive => true,
    }
}

// r[impl sel4-tcb.state]
// r[depends sel4-tcb.state]
pub open spec fn runnable_state_spec(state: TcbState) -> bool {
    matches!(state, TcbState::Runnable)
}

// r[impl sel4-tcb.blocking-object]
// r[depends sel4-tcb.blocking-object]
pub open spec fn blocked_state_spec(state: TcbState) -> bool {
    matches!(
        state,
        TcbState::BlockedOnSend { endpoint: _ }
            | TcbState::BlockedOnReceive { endpoint: _ }
            | TcbState::BlockedOnNotification { notification: _ }
            | TcbState::BlockedOnReply { reply: _ }
    )
}

// r[impl sel4-tcb.blocking-object]
// r[depends sel4-tcb.blocking-object]
pub open spec fn blocking_object_spec(state: TcbState) -> Option<usize> {
    match state {
        TcbState::BlockedOnSend { endpoint } => Some(endpoint),
        TcbState::BlockedOnReceive { endpoint } => Some(endpoint),
        TcbState::BlockedOnNotification { notification } => Some(notification),
        TcbState::BlockedOnReply { reply } => Some(reply),
        TcbState::Runnable | TcbState::Running | TcbState::Restart | TcbState::Inactive => None,
    }
}

// r[impl sel4-tcb.transitions]
// r[depends sel4-tcb.transitions]
pub open spec fn can_block_spec(state: TcbState) -> bool {
    matches!(state, TcbState::Runnable | TcbState::Running)
}

// r[impl sel4-tcb.transitions]
// r[depends sel4-tcb.transitions]
pub open spec fn can_wake_spec(state: TcbState) -> bool {
    blocked_state_spec(state) || matches!(state, TcbState::Restart)
}

// r[impl sel4-tcb.transitions]
// r[depends sel4-tcb.transitions]
pub open spec fn can_restart_spec(state: TcbState) -> bool {
    blocked_state_spec(state)
}

// r[impl sel4-tcb.transitions]
// r[depends sel4-tcb.transitions]
pub open spec fn can_deactivate_spec(state: TcbState) -> bool {
    !matches!(state, TcbState::Inactive)
}

// r[impl sel4-tcb.transitions]
// r[depends sel4-tcb.transitions]
pub open spec fn legal_transition_spec(old_state: TcbState, new_state: TcbState) -> bool {
    match old_state {
        TcbState::Runnable | TcbState::Running => {
            matches!(
                new_state,
                TcbState::BlockedOnSend { endpoint: _ }
                    | TcbState::BlockedOnReceive { endpoint: _ }
                    | TcbState::BlockedOnNotification { notification: _ }
                    | TcbState::BlockedOnReply { reply: _ }
                    | TcbState::Inactive
            )
        }
        TcbState::BlockedOnSend { endpoint: _ }
        | TcbState::BlockedOnReceive { endpoint: _ }
        | TcbState::BlockedOnNotification { notification: _ }
        | TcbState::BlockedOnReply { reply: _ } => {
            matches!(new_state, TcbState::Runnable | TcbState::Restart | TcbState::Inactive)
        }
        TcbState::Restart => matches!(new_state, TcbState::Runnable | TcbState::Inactive),
        TcbState::Inactive => false,
    }
}

// r[impl sel4-tcb.transitions]
// r[depends sel4-tcb.transitions]
pub open spec fn block_on_send_transition_spec(tcb: Tcb, endpoint: usize) -> Tcb {
    if can_block_spec(tcb.state) {
        Tcb {
            tid: tcb.tid,
            priority: tcb.priority,
            state: TcbState::BlockedOnSend { endpoint },
        }
    } else {
        tcb
    }
}

// r[impl sel4-tcb.transitions]
// r[depends sel4-tcb.transitions]
pub open spec fn block_on_receive_transition_spec(tcb: Tcb, endpoint: usize) -> Tcb {
    if can_block_spec(tcb.state) {
        Tcb {
            tid: tcb.tid,
            priority: tcb.priority,
            state: TcbState::BlockedOnReceive { endpoint },
        }
    } else {
        tcb
    }
}

// r[impl sel4-tcb.transitions]
// r[depends sel4-tcb.transitions]
pub open spec fn block_on_notification_transition_spec(tcb: Tcb, notification: usize) -> Tcb {
    if can_block_spec(tcb.state) {
        Tcb {
            tid: tcb.tid,
            priority: tcb.priority,
            state: TcbState::BlockedOnNotification { notification },
        }
    } else {
        tcb
    }
}

// r[impl sel4-tcb.transitions]
// r[depends sel4-tcb.transitions]
pub open spec fn block_on_reply_transition_spec(tcb: Tcb, reply: usize) -> Tcb {
    if can_block_spec(tcb.state) {
        Tcb {
            tid: tcb.tid,
            priority: tcb.priority,
            state: TcbState::BlockedOnReply { reply },
        }
    } else {
        tcb
    }
}

// r[impl sel4-tcb.transitions]
// r[depends sel4-tcb.transitions]
pub open spec fn wake_transition_spec(tcb: Tcb) -> Tcb {
    if can_wake_spec(tcb.state) {
        Tcb {
            tid: tcb.tid,
            priority: tcb.priority,
            state: TcbState::Runnable,
        }
    } else {
        tcb
    }
}

// r[impl sel4-tcb.transitions]
// r[depends sel4-tcb.transitions]
pub open spec fn restart_transition_spec(tcb: Tcb) -> Tcb {
    if can_restart_spec(tcb.state) {
        Tcb {
            tid: tcb.tid,
            priority: tcb.priority,
            state: TcbState::Restart,
        }
    } else {
        tcb
    }
}

// r[impl sel4-tcb.transitions]
// r[depends sel4-tcb.transitions]
pub open spec fn deactivate_transition_spec(tcb: Tcb) -> Tcb {
    if can_deactivate_spec(tcb.state) {
        Tcb {
            tid: tcb.tid,
            priority: tcb.priority,
            state: TcbState::Inactive,
        }
    } else {
        tcb
    }
}

// r[impl sel4-tcb.state]
pub fn new_tcb(tid: usize, priority: u8) -> (result: Tcb)
    ensures
        result.tid == tid,
        result.priority == priority,
        legal_state_spec(result.state),
        runnable_state_spec(result.state),
{
    Tcb {
        tid,
        priority,
        state: TcbState::Runnable,
    }
}

// r[impl sel4-tcb.state]
pub fn is_runnable(tcb: &Tcb) -> (result: bool)
    ensures
        result == runnable_state_spec(tcb.state),
{
    matches!(tcb.state, TcbState::Runnable)
}

// r[impl sel4-tcb.blocking-object]
pub fn is_blocked(tcb: &Tcb) -> (result: bool)
    ensures
        result == blocked_state_spec(tcb.state),
{
    matches!(
        tcb.state,
        TcbState::BlockedOnSend { endpoint: _ }
            | TcbState::BlockedOnReceive { endpoint: _ }
            | TcbState::BlockedOnNotification { notification: _ }
            | TcbState::BlockedOnReply { reply: _ }
    )
}

// r[impl sel4-tcb.blocking-object]
pub fn blocking_object(tcb: &Tcb) -> (result: Option<usize>)
    ensures
        result == blocking_object_spec(tcb.state),
{
    match tcb.state {
        TcbState::BlockedOnSend { endpoint } | TcbState::BlockedOnReceive { endpoint } => Some(endpoint),
        TcbState::BlockedOnNotification { notification } => Some(notification),
        TcbState::BlockedOnReply { reply } => Some(reply),
        TcbState::Runnable | TcbState::Running | TcbState::Restart | TcbState::Inactive => None,
    }
}

// r[impl sel4-tcb.transitions]
pub fn block_on_send(tcb: &mut Tcb, endpoint: usize) -> (result: bool)
    ensures
        *tcb == block_on_send_transition_spec(*old(tcb), endpoint),
        result == can_block_spec(old(tcb).state),
        result ==> legal_transition_spec(old(tcb).state, tcb.state),
{
    match tcb.state {
        TcbState::Runnable | TcbState::Running => {
            tcb.state = TcbState::BlockedOnSend { endpoint };
            true
        }
        TcbState::BlockedOnSend { endpoint: _ }
        | TcbState::BlockedOnReceive { endpoint: _ }
        | TcbState::BlockedOnNotification { notification: _ }
        | TcbState::BlockedOnReply { reply: _ }
        | TcbState::Restart
        | TcbState::Inactive => false,
    }
}

// r[impl sel4-tcb.transitions]
pub fn block_on_receive(tcb: &mut Tcb, endpoint: usize) -> (result: bool)
    ensures
        *tcb == block_on_receive_transition_spec(*old(tcb), endpoint),
        result == can_block_spec(old(tcb).state),
        result ==> legal_transition_spec(old(tcb).state, tcb.state),
{
    match tcb.state {
        TcbState::Runnable | TcbState::Running => {
            tcb.state = TcbState::BlockedOnReceive { endpoint };
            true
        }
        TcbState::BlockedOnSend { endpoint: _ }
        | TcbState::BlockedOnReceive { endpoint: _ }
        | TcbState::BlockedOnNotification { notification: _ }
        | TcbState::BlockedOnReply { reply: _ }
        | TcbState::Restart
        | TcbState::Inactive => false,
    }
}

// r[impl sel4-tcb.transitions]
pub fn block_on_notification(tcb: &mut Tcb, notification: usize) -> (result: bool)
    ensures
        *tcb == block_on_notification_transition_spec(*old(tcb), notification),
        result == can_block_spec(old(tcb).state),
        result ==> legal_transition_spec(old(tcb).state, tcb.state),
{
    match tcb.state {
        TcbState::Runnable | TcbState::Running => {
            tcb.state = TcbState::BlockedOnNotification { notification };
            true
        }
        TcbState::BlockedOnSend { endpoint: _ }
        | TcbState::BlockedOnReceive { endpoint: _ }
        | TcbState::BlockedOnNotification { notification: _ }
        | TcbState::BlockedOnReply { reply: _ }
        | TcbState::Restart
        | TcbState::Inactive => false,
    }
}

// r[impl sel4-tcb.transitions]
pub fn block_on_reply(tcb: &mut Tcb, reply: usize) -> (result: bool)
    ensures
        *tcb == block_on_reply_transition_spec(*old(tcb), reply),
        result == can_block_spec(old(tcb).state),
        result ==> legal_transition_spec(old(tcb).state, tcb.state),
{
    match tcb.state {
        TcbState::Runnable | TcbState::Running => {
            tcb.state = TcbState::BlockedOnReply { reply };
            true
        }
        TcbState::BlockedOnSend { endpoint: _ }
        | TcbState::BlockedOnReceive { endpoint: _ }
        | TcbState::BlockedOnNotification { notification: _ }
        | TcbState::BlockedOnReply { reply: _ }
        | TcbState::Restart
        | TcbState::Inactive => false,
    }
}

// r[impl sel4-tcb.transitions]
pub fn wake(tcb: &mut Tcb) -> (result: bool)
    ensures
        *tcb == wake_transition_spec(*old(tcb)),
        result == can_wake_spec(old(tcb).state),
        result ==> legal_transition_spec(old(tcb).state, tcb.state),
{
    match tcb.state {
        TcbState::BlockedOnSend { endpoint: _ }
        | TcbState::BlockedOnReceive { endpoint: _ }
        | TcbState::BlockedOnNotification { notification: _ }
        | TcbState::BlockedOnReply { reply: _ }
        | TcbState::Restart => {
            tcb.state = TcbState::Runnable;
            true
        }
        TcbState::Runnable | TcbState::Running | TcbState::Inactive => false,
    }
}

// r[impl sel4-tcb.transitions]
pub fn restart(tcb: &mut Tcb) -> (result: bool)
    ensures
        *tcb == restart_transition_spec(*old(tcb)),
        result == can_restart_spec(old(tcb).state),
        result ==> legal_transition_spec(old(tcb).state, tcb.state),
{
    match tcb.state {
        TcbState::BlockedOnSend { endpoint: _ }
        | TcbState::BlockedOnReceive { endpoint: _ }
        | TcbState::BlockedOnNotification { notification: _ }
        | TcbState::BlockedOnReply { reply: _ } => {
            tcb.state = TcbState::Restart;
            true
        }
        TcbState::Runnable | TcbState::Running | TcbState::Restart | TcbState::Inactive => false,
    }
}

// r[impl sel4-tcb.transitions]
pub fn deactivate(tcb: &mut Tcb) -> (result: bool)
    ensures
        *tcb == deactivate_transition_spec(*old(tcb)),
        result == can_deactivate_spec(old(tcb).state),
        result ==> legal_transition_spec(old(tcb).state, tcb.state),
{
    match tcb.state {
        TcbState::Inactive => false,
        TcbState::Runnable
        | TcbState::Running
        | TcbState::BlockedOnSend { endpoint: _ }
        | TcbState::BlockedOnReceive { endpoint: _ }
        | TcbState::BlockedOnNotification { notification: _ }
        | TcbState::BlockedOnReply { reply: _ }
        | TcbState::Restart => {
            tcb.state = TcbState::Inactive;
            true
        }
    }
}

// r[verify sel4-tcb.state]
proof fn runnable_differs_from_blocked(state: TcbState)
    ensures
        blocked_state_spec(state) ==> !runnable_state_spec(state),
        legal_state_spec(state),
{
}

// r[verify sel4-tcb.blocking-object]
proof fn blocked_states_name_object(state: TcbState)
    ensures
        blocked_state_spec(state) ==> blocking_object_spec(state).is_some(),
{
}

// r[verify sel4-tcb.blocking-object]
proof fn notification_block_names_notification(notification: usize)
    ensures
        blocking_object_spec(TcbState::BlockedOnNotification { notification }) == Some(notification),
{
}

// r[verify sel4-tcb.transitions]
proof fn wake_moves_blocked_thread_to_runnable(state: TcbState)
    requires
        can_wake_spec(state),
    ensures
        runnable_state_spec(wake_transition_spec(Tcb { tid: 0, priority: 0, state }).state),
        legal_transition_spec(state, wake_transition_spec(Tcb { tid: 0, priority: 0, state }).state),
{
}

// r[verify sel4-tcb.transitions]
proof fn restart_moves_blocked_thread_to_restart(state: TcbState)
    requires
        can_restart_spec(state),
    ensures
        matches!(restart_transition_spec(Tcb { tid: 0, priority: 0, state }).state, TcbState::Restart),
        legal_transition_spec(state, restart_transition_spec(Tcb { tid: 0, priority: 0, state }).state),
{
}

// r[verify sel4-tcb.transitions]
proof fn deactivate_moves_active_thread_to_inactive(state: TcbState)
    requires
        can_deactivate_spec(state),
    ensures
        matches!(deactivate_transition_spec(Tcb { tid: 0, priority: 0, state }).state, TcbState::Inactive),
        legal_transition_spec(state, deactivate_transition_spec(Tcb { tid: 0, priority: 0, state }).state),
{
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify sel4-tcb.state]
    #[test]
    fn test_new_tcb_is_runnable() {
        let tcb = new_tcb(7, 3);
        assert_eq!(tcb.tid, 7);
        assert_eq!(tcb.priority, 3);
        assert!(is_runnable(&tcb));
        assert!(!is_blocked(&tcb));
        assert_eq!(blocking_object(&tcb), None);
    }

    // r[verify sel4-tcb.blocking-object]
    #[test]
    fn test_blocked_states_name_wait_object() {
        let send = Tcb {
            tid: 1,
            priority: 2,
            state: TcbState::BlockedOnSend { endpoint: 11 },
        };
        let recv = Tcb {
            tid: 1,
            priority: 2,
            state: TcbState::BlockedOnReceive { endpoint: 12 },
        };
        let notif = Tcb {
            tid: 1,
            priority: 2,
            state: TcbState::BlockedOnNotification { notification: 13 },
        };
        let reply = Tcb {
            tid: 1,
            priority: 2,
            state: TcbState::BlockedOnReply { reply: 14 },
        };

        assert!(is_blocked(&send));
        assert_eq!(blocking_object(&send), Some(11));
        assert_eq!(blocking_object(&recv), Some(12));
        assert_eq!(blocking_object(&notif), Some(13));
        assert_eq!(blocking_object(&reply), Some(14));
    }

    // r[verify sel4-tcb.transitions]
    #[test]
    fn test_wake_moves_blocked_receive_to_runnable() {
        let mut tcb = Tcb {
            tid: 9,
            priority: 1,
            state: TcbState::Running,
        };
        assert!(block_on_receive(&mut tcb, 44));
        assert!(is_blocked(&tcb));
        assert!(wake(&mut tcb));
        assert!(is_runnable(&tcb));
    }

    // r[verify sel4-tcb.transitions]
    #[test]
    fn test_restart_then_wake() {
        let mut tcb = new_tcb(3, 9);
        assert!(block_on_reply(&mut tcb, 55));
        assert!(restart(&mut tcb));
        assert!(wake(&mut tcb));
        assert!(is_runnable(&tcb));
    }

    // r[verify sel4-tcb.transitions]
    #[test]
    fn test_deactivate_moves_to_inactive() {
        let mut tcb = new_tcb(5, 4);
        assert!(deactivate(&mut tcb));
        assert!(matches!(tcb.state, TcbState::Inactive));
        assert!(!deactivate(&mut tcb));
    }

    proptest! {
        // r[verify sel4-tcb.blocking-object]
        #[test]
        fn prop_block_helpers_attach_object(tid: usize, priority: u8, endpoint: usize, notification: usize, reply: usize) {
            let mut send = Tcb { tid, priority, state: TcbState::Running };
            prop_assert!(block_on_send(&mut send, endpoint));
            prop_assert_eq!(blocking_object(&send), Some(endpoint));

            let mut recv = Tcb { tid, priority, state: TcbState::Running };
            prop_assert!(block_on_receive(&mut recv, endpoint));
            prop_assert_eq!(blocking_object(&recv), Some(endpoint));

            let mut notif = Tcb { tid, priority, state: TcbState::Running };
            prop_assert!(block_on_notification(&mut notif, notification));
            prop_assert_eq!(blocking_object(&notif), Some(notification));

            let mut reply_tcb = Tcb { tid, priority, state: TcbState::Running };
            prop_assert!(block_on_reply(&mut reply_tcb, reply));
            prop_assert_eq!(blocking_object(&reply_tcb), Some(reply));
        }

        // r[verify sel4-tcb.transitions]
        #[test]
        fn prop_wake_after_block_returns_runnable(tid: usize, priority: u8, endpoint: usize) {
            let mut tcb = new_tcb(tid, priority);
            prop_assert!(block_on_send(&mut tcb, endpoint));
            prop_assert!(wake(&mut tcb));
            prop_assert!(is_runnable(&tcb));
        }

        // r[verify sel4-tcb.transitions]
        #[test]
        fn prop_restart_then_wake_returns_runnable(tid: usize, priority: u8, notification: usize) {
            let mut tcb = Tcb { tid, priority, state: TcbState::Running };
            prop_assert!(block_on_notification(&mut tcb, notification));
            prop_assert!(restart(&mut tcb));
            prop_assert!(wake(&mut tcb));
            prop_assert!(is_runnable(&tcb));
        }

        // r[verify sel4-tcb.transitions]
        #[test]
        fn prop_deactivate_is_terminal_for_modeled_ops(tid: usize, priority: u8) {
            let mut tcb = new_tcb(tid, priority);
            prop_assert!(deactivate(&mut tcb));
            prop_assert!(matches!(tcb.state, TcbState::Inactive));
            prop_assert!(!wake(&mut tcb));
            prop_assert!(!restart(&mut tcb));
        }
    }
}
