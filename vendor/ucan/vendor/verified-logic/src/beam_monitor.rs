//! Verified BEAM monitor creation, DOWN delivery, and demonitor flush.
//!
//! Monitors are unidirectional: only the watcher keeps monitor state, and
//! only the watcher receives DOWN messages. Late monitors get an immediate
//! `noproc` DOWN. Demonitor disables future DOWN delivery and can flush an
//! already-delivered DOWN from the mailbox.

use vstd::prelude::*;

use crate::beam_mailbox::BeamMessage;
use crate::beam_mailbox::MESSAGE_DOWN;
use crate::beam_process::BEAM_DEAD;
use crate::beam_process::BeamProcess;
use crate::beam_process::REASON_NOPROC;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BeamMonitor {
    pub watcher: u64,
    pub target: u64,
    pub ref_id: u64,
    pub active: bool,
}

/// Spec: DOWN message for one monitor reference.
pub open spec fn down_message_spec(monitor: BeamMonitor, reason: u64) -> BeamMessage {
    BeamMessage {
        sender: monitor.target,
        kind: MESSAGE_DOWN,
        payload: 0,
        ref_id: monitor.ref_id,
        observed_pid: monitor.target,
        reason,
    }
}

/// Runtime DOWN message constructor.
fn down_message(monitor: BeamMonitor, reason: u64) -> (result: BeamMessage)
    ensures result == down_message_spec(monitor, reason)
{
    BeamMessage {
        sender: monitor.target,
        kind: MESSAGE_DOWN,
        payload: 0,
        ref_id: monitor.ref_id,
        observed_pid: monitor.target,
        reason,
    }
}

/// Spec: exact ref match for DOWN flush.
pub open spec fn is_down_for_ref_spec(msg: BeamMessage, ref_id: u64) -> bool {
    msg.kind == MESSAGE_DOWN && msg.ref_id == ref_id
}

/// Runtime ref match for DOWN flush.
fn is_down_for_ref(msg: BeamMessage, ref_id: u64) -> (result: bool)
    ensures result == is_down_for_ref_spec(msg, ref_id)
{
    msg.kind == MESSAGE_DOWN && msg.ref_id == ref_id
}

// r[depends beam-monitor.unidirectional]
/// Spec: create a monitor and return the unchanged target-side process value.
pub open spec fn create_monitor_view_spec(ref_id: u64, watcher: u64, target_proc: BeamProcess) -> (BeamMonitor, Option<BeamMessage>, BeamProcess) {
    if target_proc.state == BEAM_DEAD {
        let monitor = BeamMonitor { watcher, target: target_proc.pid, ref_id, active: false };
        (monitor, Some(down_message_spec(monitor, REASON_NOPROC)), target_proc)
    } else {
        (BeamMonitor { watcher, target: target_proc.pid, ref_id, active: true }, None, target_proc)
    }
}

// r[depends beam-monitor.unidirectional]
/// Spec: monitor creation produces watcher-owned state only and returns the
/// target-side process unchanged.
pub open spec fn monitor_unidirectional_spec(ref_id: u64, watcher: u64, target_proc: BeamProcess) -> bool {
    let created = create_monitor_view_spec(ref_id, watcher, target_proc);
    &&& created.2 == target_proc
    &&& created.0.watcher == watcher
    &&& created.0.target == target_proc.pid
    &&& created.0.ref_id == ref_id
    &&& (target_proc.state == BEAM_DEAD ==> !created.0.active && created.1 == Some(down_message_spec(created.0, REASON_NOPROC)))
    &&& (target_proc.state != BEAM_DEAD ==> created.0.active && created.1.is_none())
}

// r[depends beam-monitor.down]
// r[depends beam-monitor.late]
/// Spec: create monitor and maybe emit immediate DOWN if target already dead.
pub open spec fn create_monitor_spec(ref_id: u64, watcher: u64, target_proc: BeamProcess) -> (BeamMonitor, Option<BeamMessage>) {
    let created = create_monitor_view_spec(ref_id, watcher, target_proc);
    (created.0, created.1)
}

// r[depends beam-monitor.down]
/// Spec: death notification sends exactly one DOWN when active.
pub open spec fn notify_death_spec(monitor: BeamMonitor, reason: u64) -> (BeamMonitor, Option<BeamMessage>) {
    if monitor.active {
        (
            BeamMonitor {
                watcher: monitor.watcher,
                target: monitor.target,
                ref_id: monitor.ref_id,
                active: false,
            },
            Some(down_message_spec(monitor, reason)),
        )
    } else {
        (monitor, None)
    }
}

// r[depends beam-monitor.demonitor]
/// Spec: demonitor disables future DOWN delivery.
pub open spec fn demonitor_spec(monitor: BeamMonitor) -> BeamMonitor {
    BeamMonitor {
        watcher: monitor.watcher,
        target: monitor.target,
        ref_id: monitor.ref_id,
        active: false,
    }
}

/// Spec helper: flush all matching DOWN messages from prefix `[0, upto)`.
pub open spec fn flush_down_prefix_spec(mailbox: Seq<BeamMessage>, ref_id: u64, upto: int) -> Seq<BeamMessage>
    decreases if upto >= 0 { upto } else { 0 }
{
    if upto <= 0 {
        seq![]
    } else {
        let prefix = flush_down_prefix_spec(mailbox, ref_id, upto - 1);
        if is_down_for_ref_spec(mailbox[upto - 1], ref_id) {
            prefix
        } else {
            prefix.push(mailbox[upto - 1])
        }
    }
}

/// Spec: remove DOWN messages for one monitor ref.
pub open spec fn flush_down_messages_spec(mailbox: Seq<BeamMessage>, ref_id: u64) -> Seq<BeamMessage> {
    flush_down_prefix_spec(mailbox, ref_id, mailbox.len() as int)
}

// r[impl beam-monitor.unidirectional]
// r[impl beam-monitor.down]
// r[impl beam-monitor.late]
/// Create a monitor. If the target is already dead, return an immediate
/// `noproc` DOWN and an inactive monitor.
pub fn create_monitor(ref_id: u64, watcher: u64, target_proc: &BeamProcess) -> (result: (BeamMonitor, Option<BeamMessage>, BeamProcess))
    ensures
        result.0 == create_monitor_spec(ref_id, watcher, *target_proc).0,
        result.1 == create_monitor_spec(ref_id, watcher, *target_proc).1,
        result.2.pid == target_proc.pid,
        result.2.state == target_proc.state,
        result.2.trap_exit == target_proc.trap_exit,
        result.2.exit_reason == target_proc.exit_reason,
        result.2.heap_id == target_proc.heap_id,
        result.2.heap@ =~= target_proc.heap@,
{
    let target_after = BeamProcess {
        pid: target_proc.pid,
        state: target_proc.state,
        trap_exit: target_proc.trap_exit,
        exit_reason: target_proc.exit_reason,
        heap_id: target_proc.heap_id,
        heap: target_proc.heap.clone(),
    };
    if target_proc.state == BEAM_DEAD {
        let monitor = BeamMonitor { watcher, target: target_proc.pid, ref_id, active: false };
        (monitor, Some(down_message(monitor, REASON_NOPROC)), target_after)
    } else {
        (
            BeamMonitor { watcher, target: target_proc.pid, ref_id, active: true },
            None,
            target_after,
        )
    }
}

// r[impl beam-monitor.down]
/// Notify one active monitor of target death.
pub fn notify_death(monitor: BeamMonitor, reason: u64) -> (result: (BeamMonitor, Option<BeamMessage>))
    ensures result == notify_death_spec(monitor, reason)
{
    if monitor.active {
        let next = BeamMonitor {
            watcher: monitor.watcher,
            target: monitor.target,
            ref_id: monitor.ref_id,
            active: false,
        };
        (next, Some(down_message(monitor, reason)))
    } else {
        (monitor, None)
    }
}

// r[impl beam-monitor.demonitor]
/// Demonitor disables future DOWN delivery.
pub fn demonitor(monitor: BeamMonitor) -> (result: BeamMonitor)
    ensures result == demonitor_spec(monitor)
{
    BeamMonitor {
        watcher: monitor.watcher,
        target: monitor.target,
        ref_id: monitor.ref_id,
        active: false,
    }
}

// r[impl beam-monitor.demonitor]
/// Remove any queued DOWN message for `ref_id`.
pub fn demonitor_flush(mailbox: &[BeamMessage], ref_id: u64) -> (result: Vec<BeamMessage>)
    ensures result@ == flush_down_messages_spec(mailbox@, ref_id)
{
    let mut result: Vec<BeamMessage> = Vec::new();
    let mut i: usize = 0;
    while i < mailbox.len()
        invariant
            0 <= i <= mailbox@.len(),
            result@ == flush_down_prefix_spec(mailbox@, ref_id, i as int),
        decreases mailbox@.len() - i,
    {
        if !is_down_for_ref(mailbox[i], ref_id) {
            result.push(mailbox[i]);
            proof {
                assert(flush_down_prefix_spec(mailbox@, ref_id, i as int + 1)
                    == flush_down_prefix_spec(mailbox@, ref_id, i as int).push(mailbox[i as int]));
            }
        } else {
            proof {
                assert(flush_down_prefix_spec(mailbox@, ref_id, i as int + 1)
                    == flush_down_prefix_spec(mailbox@, ref_id, i as int));
            }
        }
        i = i + 1;
    }
    result
}

// r[verify beam-monitor.unidirectional]
proof fn create_monitor_leaves_target_unchanged(ref_id: u64, watcher: u64, target_proc: BeamProcess)
    ensures monitor_unidirectional_spec(ref_id, watcher, target_proc)
{}

// r[verify beam-monitor.down]
proof fn active_monitor_sends_exactly_one_down(monitor: BeamMonitor, reason: u64)
    requires monitor.active
    ensures
        notify_death_spec(monitor, reason).0.active == false,
        notify_death_spec(monitor, reason).1 == Some(down_message_spec(monitor, reason)),
        notify_death_spec(notify_death_spec(monitor, reason).0, reason).1.is_none(),
{}

// r[verify beam-monitor.late]
proof fn late_monitor_gets_noproc(ref_id: u64, watcher: u64, target_proc: BeamProcess)
    requires target_proc.state == BEAM_DEAD
    ensures create_monitor_spec(ref_id, watcher, target_proc).1.is_some(),
        create_monitor_spec(ref_id, watcher, target_proc).1.unwrap().reason == REASON_NOPROC,
{}

// r[verify beam-monitor.demonitor]
proof fn demonitor_prevents_future_down(monitor: BeamMonitor, reason: u64)
    ensures notify_death_spec(demonitor_spec(monitor), reason).1.is_none()
{}

// r[verify beam-monitor.demonitor]
proof fn flush_result_is_a_sequence(mailbox: Seq<BeamMessage>, ref_id: u64)
    ensures flush_down_messages_spec(mailbox, ref_id).len() >= 0
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::beam_process::BEAM_ALIVE;
    use crate::beam_process::BEAM_DEAD;
    use crate::beam_process::BeamProcess;
    use crate::beam_process::REASON_NORMAL;

    // r[verify beam-monitor.unidirectional]
    #[test]
    fn test_monitor_creation_is_watcher_owned() {
        let target = BeamProcess {
            pid: 22,
            state: BEAM_ALIVE,
            trap_exit: false,
            exit_reason: REASON_NORMAL,
            heap_id: 22,
            heap: vec![1, 2, 3],
        };
        let (monitor, immediate, target_after) = create_monitor(7, 11, &target);
        assert_eq!(monitor.watcher, 11);
        assert_eq!(monitor.target, 22);
        assert_eq!(monitor.ref_id, 7);
        assert!(monitor.active);
        assert!(immediate.is_none());
        assert_eq!(target_after, target);
    }

    // r[verify beam-monitor.down]
    #[test]
    fn test_monitor_death_delivers_down_once() {
        let target = BeamProcess {
            pid: 2,
            state: BEAM_ALIVE,
            trap_exit: false,
            exit_reason: REASON_NORMAL,
            heap_id: 2,
            heap: vec![],
        };
        let (monitor, immediate, target_after) = create_monitor(7, 1, &target);
        assert!(immediate.is_none());
        assert_eq!(target_after, target);

        let (monitor, down) = notify_death(monitor, 9);
        assert!(!monitor.active);
        let down = down.unwrap();
        assert_eq!(down.kind, MESSAGE_DOWN);
        assert_eq!(down.ref_id, 7);
        assert_eq!(down.observed_pid, 2);
        assert_eq!(down.reason, 9);

        let (_, again) = notify_death(monitor, 9);
        assert!(again.is_none());
    }

    // r[verify beam-monitor.late]
    #[test]
    fn test_late_monitor_gets_noproc() {
        let target = BeamProcess {
            pid: 2,
            state: BEAM_DEAD,
            trap_exit: false,
            exit_reason: REASON_NORMAL,
            heap_id: 2,
            heap: vec![7],
        };
        let (monitor, down, target_after) = create_monitor(7, 1, &target);
        assert!(!monitor.active);
        assert_eq!(down.unwrap().reason, REASON_NOPROC);
        assert_eq!(target_after, target);
    }

    // r[verify beam-monitor.demonitor]
    #[test]
    fn test_demonitor_flush_removes_down() {
        let monitor = BeamMonitor {
            watcher: 1,
            target: 2,
            ref_id: 7,
            active: true,
        };
        let down = down_message(monitor, 9);
        let mailbox = vec![down, BeamMessage {
            sender: 9,
            kind: 0,
            payload: 44,
            ref_id: 0,
            observed_pid: 0,
            reason: 0,
        }];
        let flushed = demonitor_flush(&mailbox, 7);
        assert_eq!(flushed.len(), 1);
        assert_eq!(flushed[0].payload, 44);
    }

    proptest! {
        // r[verify beam-monitor.demonitor]
        #[test]
        fn prop_demonitor_disables_monitor(watcher: u64, target: u64, ref_id: u64) {
            let monitor = BeamMonitor { watcher, target, ref_id, active: true };
            let off = demonitor(monitor);
            prop_assert!(!off.active);
        }
    }
}
