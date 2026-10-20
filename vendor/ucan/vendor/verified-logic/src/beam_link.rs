//! Verified BEAM link symmetry and exit propagation.
//!
//! Links are symmetric pid sets. Exit delivery follows BEAM rules: normal
//! exits do not cascade to non-trapping peers, abnormal exits do, trapped
//! exits become `EXIT` messages, and `kill` bypasses `trap_exit`.

use vstd::prelude::*;

use crate::beam_mailbox::BeamMessage;
use crate::beam_mailbox::MESSAGE_EXIT;
use crate::beam_mailbox::mailbox_append;
#[cfg(verus_keep_ghost)]
use crate::beam_mailbox::mailbox_append_spec;
use crate::beam_process::BEAM_ALIVE;
use crate::beam_process::BeamProcess;
use crate::beam_process::REASON_KILL;
use crate::beam_process::REASON_KILLED;
use crate::beam_process::REASON_NORMAL;
use crate::beam_process::start_exit;
#[cfg(verus_keep_ghost)]
use crate::beam_process::start_exit_spec;

verus! {

fn clone_u64s(links: &[u64]) -> (result: Vec<u64>)
    ensures result@ =~= links@
{
    let mut result: Vec<u64> = Vec::new();
    let mut i: usize = 0;
    while i < links.len()
        invariant
            0 <= i <= links@.len(),
            result@ =~= links@.subrange(0, i as int),
        decreases links@.len() - i,
    {
        result.push(links[i]);
        i = i + 1;
    }
    result
}

fn clone_mailbox(mailbox: &[BeamMessage]) -> (result: Vec<BeamMessage>)
    ensures result@ =~= mailbox@
{
    let mut result: Vec<BeamMessage> = Vec::new();
    let mut i: usize = 0;
    while i < mailbox.len()
        invariant
            0 <= i <= mailbox@.len(),
            result@ =~= mailbox@.subrange(0, i as int),
        decreases mailbox@.len() - i,
    {
        result.push(mailbox[i]);
        i = i + 1;
    }
    result
}

fn clone_process(proc: &BeamProcess) -> (result: BeamProcess)
    ensures
        result.pid == proc.pid,
        result.state == proc.state,
        result.trap_exit == proc.trap_exit,
        result.exit_reason == proc.exit_reason,
        result.heap_id == proc.heap_id,
        result.heap@ =~= proc.heap@,
{
    BeamProcess {
        pid: proc.pid,
        state: proc.state,
        trap_exit: proc.trap_exit,
        exit_reason: proc.exit_reason,
        heap_id: proc.heap_id,
        heap: proc.heap.clone(),
    }
}

/// Spec: pid membership in a link set.
pub open spec fn link_contains_spec(links: Seq<u64>, pid: u64) -> bool {
    exists|i: int| #![trigger links[i]] 0 <= i < links.len() && links[i] == pid
}

/// Spec: insert pid once.
pub open spec fn link_insert_spec(links: Seq<u64>, pid: u64) -> Seq<u64> {
    if link_contains_spec(links, pid) { links } else { links.push(pid) }
}

/// Spec helper: remove all `pid` entries from prefix `[0, upto)`.
pub open spec fn link_remove_prefix_spec(links: Seq<u64>, pid: u64, upto: int) -> Seq<u64>
    decreases if upto >= 0 { upto } else { 0 }
{
    if upto <= 0 {
        seq![]
    } else {
        let prefix = link_remove_prefix_spec(links, pid, upto - 1);
        if links[upto - 1] == pid { prefix } else { prefix.push(links[upto - 1]) }
    }
}

/// Spec: remove all `pid` entries.
pub open spec fn link_remove_spec(links: Seq<u64>, pid: u64) -> Seq<u64> {
    link_remove_prefix_spec(links, pid, links.len() as int)
}

// r[depends beam-link.symmetry]
/// Spec: create a symmetric link between `a_pid` and `b_pid`.
pub open spec fn link_spec(a_links: Seq<u64>, b_links: Seq<u64>, a_pid: u64, b_pid: u64) -> (Seq<u64>, Seq<u64>) {
    (link_insert_spec(a_links, b_pid), link_insert_spec(b_links, a_pid))
}

// r[depends beam-link.symmetry]
/// Spec: remove both directions of a link.
pub open spec fn unlink_spec(a_links: Seq<u64>, b_links: Seq<u64>, a_pid: u64, b_pid: u64) -> (Seq<u64>, Seq<u64>) {
    (link_remove_spec(a_links, b_pid), link_remove_spec(b_links, a_pid))
}

/// Helper: append pid unless already present.
fn add_unique(links: &[u64], pid: u64) -> (result: Vec<u64>)
    ensures result@ == link_insert_spec(links@, pid)
{
    let mut i: usize = 0;
    while i < links.len()
        invariant
            0 <= i <= links@.len(),
            forall|j: int| #![trigger links@[j]] 0 <= j < i as int ==> links@[j] != pid,
        decreases links@.len() - i,
    {
        if links[i] == pid {
            return clone_u64s(links);
        }
        i = i + 1;
    }
    let mut result = clone_u64s(links);
    result.push(pid);
    result
}

/// Helper: remove all instances of one pid.
fn remove_pid(links: &[u64], pid: u64) -> (result: Vec<u64>)
    ensures result@ == link_remove_spec(links@, pid)
{
    let mut result: Vec<u64> = Vec::new();
    let mut i: usize = 0;
    while i < links.len()
        invariant
            0 <= i <= links@.len(),
            result@ == link_remove_prefix_spec(links@, pid, i as int),
        decreases links@.len() - i,
    {
        if links[i] != pid {
            result.push(links[i]);
            proof {
                assert(link_remove_prefix_spec(links@, pid, i as int + 1)
                    == link_remove_prefix_spec(links@, pid, i as int).push(links[i as int]));
            }
        } else {
            proof {
                assert(link_remove_prefix_spec(links@, pid, i as int + 1)
                    == link_remove_prefix_spec(links@, pid, i as int));
            }
        }
        i = i + 1;
    }
    result
}

// r[impl beam-link.symmetry]
/// Add a bidirectional link.
pub fn link_processes(a_links: &[u64], b_links: &[u64], a_pid: u64, b_pid: u64) -> (result: (Vec<u64>, Vec<u64>))
    ensures
        result.0@ == link_spec(a_links@, b_links@, a_pid, b_pid).0,
        result.1@ == link_spec(a_links@, b_links@, a_pid, b_pid).1,
{
    (add_unique(a_links, b_pid), add_unique(b_links, a_pid))
}

// r[impl beam-link.symmetry]
/// Remove a bidirectional link.
pub fn unlink_processes(a_links: &[u64], b_links: &[u64], a_pid: u64, b_pid: u64) -> (result: (Vec<u64>, Vec<u64>))
    ensures
        result.0@ == unlink_spec(a_links@, b_links@, a_pid, b_pid).0,
        result.1@ == unlink_spec(a_links@, b_links@, a_pid, b_pid).1,
{
    (remove_pid(a_links, b_pid), remove_pid(b_links, a_pid))
}

/// Spec: BEAM EXIT message for a trapped exit.
pub open spec fn exit_message_spec(source_pid: u64, reason: u64) -> BeamMessage {
    BeamMessage {
        sender: source_pid,
        kind: MESSAGE_EXIT,
        payload: 0,
        ref_id: 0,
        observed_pid: source_pid,
        reason,
    }
}

// r[depends beam-link.propagation]
// r[depends beam-link.trap-exit]
/// Spec: deliver one exit signal to one linked peer.
pub open spec fn deliver_exit_signal_spec(proc: BeamProcess, mailbox: Seq<BeamMessage>, source_pid: u64, reason: u64) -> (BeamProcess, Seq<BeamMessage>) {
    if proc.state != BEAM_ALIVE {
        (proc, mailbox)
    } else if reason == REASON_KILL {
        (start_exit_spec(proc, REASON_KILLED), mailbox)
    } else if proc.trap_exit {
        (proc, mailbox_append_spec(mailbox, exit_message_spec(source_pid, reason)))
    } else if reason == REASON_NORMAL {
        (proc, mailbox)
    } else {
        (start_exit_spec(proc, reason), mailbox)
    }
}

// r[impl beam-link.propagation]
// r[impl beam-link.trap-exit]
/// Deliver one exit signal to one peer.
pub fn deliver_exit_signal(proc: &BeamProcess, mailbox: &[BeamMessage], source_pid: u64, reason: u64) -> (result: (BeamProcess, Vec<BeamMessage>))
    ensures
        result.1@ == deliver_exit_signal_spec(*proc, mailbox@, source_pid, reason).1,
        proc.state != BEAM_ALIVE ==> result.0.state == proc.state,
        proc.state == BEAM_ALIVE && reason == REASON_KILL ==> result.0.exit_reason == REASON_KILLED,
        proc.state == BEAM_ALIVE && proc.trap_exit && reason != REASON_KILL ==> result.0.state == proc.state,
        proc.state == BEAM_ALIVE && !proc.trap_exit && reason == REASON_NORMAL ==> result.0.state == proc.state,
        proc.state == BEAM_ALIVE && !proc.trap_exit && reason != REASON_NORMAL && reason != REASON_KILL ==> result.0.state == crate::beam_process::BEAM_EXITING,
{
    if proc.state != BEAM_ALIVE {
        (clone_process(proc), clone_mailbox(mailbox))
    } else if reason == REASON_KILL {
        (start_exit(proc, REASON_KILLED), clone_mailbox(mailbox))
    } else if proc.trap_exit {
        (clone_process(proc), mailbox_append(mailbox, BeamMessage {
            sender: source_pid,
            kind: MESSAGE_EXIT,
            payload: 0,
            ref_id: 0,
            observed_pid: source_pid,
            reason,
        }))
    } else if reason == REASON_NORMAL {
        (clone_process(proc), clone_mailbox(mailbox))
    } else {
        (start_exit(proc, reason), clone_mailbox(mailbox))
    }
}

// r[verify beam-link.symmetry]
proof fn link_empty_sets_grow_once(a_pid: u64, b_pid: u64)
    ensures
        link_spec(seq![], seq![], a_pid, b_pid).0.len() == 1,
        link_spec(seq![], seq![], a_pid, b_pid).1.len() == 1,
        link_spec(seq![], seq![], a_pid, b_pid).0[0] == b_pid,
        link_spec(seq![], seq![], a_pid, b_pid).1[0] == a_pid,
{
    assert(!link_contains_spec(seq![], b_pid));
    assert(!link_contains_spec(seq![], a_pid));
    assert(link_insert_spec(seq![], b_pid) == seq![b_pid]);
    assert(link_insert_spec(seq![], a_pid) == seq![a_pid]);
    assert(link_spec(seq![], seq![], a_pid, b_pid).0 == seq![b_pid]);
    assert(link_spec(seq![], seq![], a_pid, b_pid).1 == seq![a_pid]);
}

// r[verify beam-link.symmetry]
proof fn unlink_singletons_removes_both_directions(a_pid: u64, b_pid: u64)
    ensures
        unlink_spec(seq![b_pid], seq![a_pid], a_pid, b_pid).0.len() == 0,
        unlink_spec(seq![b_pid], seq![a_pid], a_pid, b_pid).1.len() == 0,
        !link_contains_spec(unlink_spec(seq![b_pid], seq![a_pid], a_pid, b_pid).0, b_pid),
        !link_contains_spec(unlink_spec(seq![b_pid], seq![a_pid], a_pid, b_pid).1, a_pid),
{
    assert(link_remove_prefix_spec(seq![b_pid], b_pid, 0).len() == 0);
    assert(link_remove_prefix_spec(seq![b_pid], b_pid, 1).len() == 0);
    assert(link_remove_spec(seq![b_pid], b_pid).len() == 0);
    assert(link_remove_prefix_spec(seq![a_pid], a_pid, 0).len() == 0);
    assert(link_remove_prefix_spec(seq![a_pid], a_pid, 1).len() == 0);
    assert(link_remove_spec(seq![a_pid], a_pid).len() == 0);
    assert(unlink_spec(seq![b_pid], seq![a_pid], a_pid, b_pid).0.len() == 0);
    assert(unlink_spec(seq![b_pid], seq![a_pid], a_pid, b_pid).1.len() == 0);
}

// r[verify beam-link.propagation]
proof fn abnormal_exit_cascades(proc: BeamProcess, mailbox: Seq<BeamMessage>, source_pid: u64, reason: u64)
    requires proc.state == BEAM_ALIVE, !proc.trap_exit, reason != REASON_NORMAL, reason != REASON_KILL
    ensures
        deliver_exit_signal_spec(proc, mailbox, source_pid, reason).0.state == crate::beam_process::BEAM_EXITING,
        deliver_exit_signal_spec(proc, mailbox, source_pid, reason).0.exit_reason == reason,
        deliver_exit_signal_spec(proc, mailbox, source_pid, reason).1 == mailbox,
{
    assert(proc.state == BEAM_ALIVE);
    assert(reason != REASON_KILL);
    assert(!proc.trap_exit);
    assert(reason != REASON_NORMAL);
    assert(deliver_exit_signal_spec(proc, mailbox, source_pid, reason).0 == start_exit_spec(proc, reason));
    assert(deliver_exit_signal_spec(proc, mailbox, source_pid, reason).1 == mailbox);
    assert(start_exit_spec(proc, reason).state == crate::beam_process::BEAM_EXITING);
    assert(start_exit_spec(proc, reason).exit_reason == reason);
}

// r[verify beam-link.propagation]
proof fn normal_exit_does_not_cascade(proc: BeamProcess, mailbox: Seq<BeamMessage>, source_pid: u64)
    requires proc.state == BEAM_ALIVE, !proc.trap_exit
    ensures
        deliver_exit_signal_spec(proc, mailbox, source_pid, REASON_NORMAL).0 == proc,
        deliver_exit_signal_spec(proc, mailbox, source_pid, REASON_NORMAL).1 == mailbox,
{
    assert(proc.state == BEAM_ALIVE);
    assert(REASON_NORMAL != REASON_KILL);
    assert(!proc.trap_exit);
    assert(deliver_exit_signal_spec(proc, mailbox, source_pid, REASON_NORMAL).0 == proc);
    assert(deliver_exit_signal_spec(proc, mailbox, source_pid, REASON_NORMAL).1 == mailbox);
}

// r[verify beam-link.trap-exit]
proof fn trapped_exit_becomes_message(proc: BeamProcess, mailbox: Seq<BeamMessage>, source_pid: u64, reason: u64)
    requires proc.state == BEAM_ALIVE, proc.trap_exit, reason != REASON_KILL
    ensures
        deliver_exit_signal_spec(proc, mailbox, source_pid, reason).0 == proc,
        deliver_exit_signal_spec(proc, mailbox, source_pid, reason).1.len() == mailbox.len() + 1,
        deliver_exit_signal_spec(proc, mailbox, source_pid, reason).1[mailbox.len() as int] == exit_message_spec(source_pid, reason),
{
    let exit_message = exit_message_spec(source_pid, reason);
    assert(proc.state == BEAM_ALIVE);
    assert(reason != REASON_KILL);
    assert(proc.trap_exit);
    assert(deliver_exit_signal_spec(proc, mailbox, source_pid, reason).0 == proc);
    assert(deliver_exit_signal_spec(proc, mailbox, source_pid, reason).1 == mailbox_append_spec(mailbox, exit_message));
    assert(mailbox_append_spec(mailbox, exit_message) == mailbox.push(exit_message));
    assert(mailbox_append_spec(mailbox, exit_message).len() == mailbox.len() + 1);
    assert(mailbox_append_spec(mailbox, exit_message)[mailbox.len() as int] == exit_message);
}

// r[verify beam-link.trap-exit]
proof fn kill_bypasses_trap(proc: BeamProcess, mailbox: Seq<BeamMessage>, source_pid: u64)
    requires proc.state == BEAM_ALIVE
    ensures
        deliver_exit_signal_spec(proc, mailbox, source_pid, REASON_KILL).0.state == crate::beam_process::BEAM_EXITING,
        deliver_exit_signal_spec(proc, mailbox, source_pid, REASON_KILL).0.exit_reason == REASON_KILLED,
        deliver_exit_signal_spec(proc, mailbox, source_pid, REASON_KILL).1 == mailbox,
{
    assert(proc.state == BEAM_ALIVE);
    assert(deliver_exit_signal_spec(proc, mailbox, source_pid, REASON_KILL).0 == start_exit_spec(proc, REASON_KILLED));
    assert(deliver_exit_signal_spec(proc, mailbox, source_pid, REASON_KILL).1 == mailbox);
    assert(start_exit_spec(proc, REASON_KILLED).state == crate::beam_process::BEAM_EXITING);
    assert(start_exit_spec(proc, REASON_KILLED).exit_reason == REASON_KILLED);
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::beam_process::BEAM_DEAD;
    use crate::beam_process::REASON_ERROR;
    use crate::beam_process::spawn_process;

    const SOURCE_PID: u64 = 1;
    const PEER_PID: u64 = 2;
    const OTHER_PID: u64 = 3;

    // r[verify beam-link.symmetry]
    #[test]
    fn test_link_and_unlink_are_symmetric() {
        let (a, b) = link_processes(&vec![], &vec![], SOURCE_PID, PEER_PID);
        assert_eq!(a, vec![PEER_PID]);
        assert_eq!(b, vec![SOURCE_PID]);

        let (a, b) = unlink_processes(&a, &b, SOURCE_PID, PEER_PID);
        assert!(a.is_empty());
        assert!(b.is_empty());
    }

    // r[verify beam-link.symmetry]
    #[test]
    fn test_repeated_link_does_not_duplicate_existing_peer() {
        let a_links = vec![PEER_PID];
        let b_links = vec![SOURCE_PID];
        let (a, b) = link_processes(&a_links, &b_links, SOURCE_PID, PEER_PID);

        assert_eq!(a, a_links);
        assert_eq!(b, b_links);
    }

    // r[verify beam-link.symmetry]
    #[test]
    fn test_unlink_removes_duplicates_and_preserves_other_links() {
        let a_links = vec![PEER_PID, OTHER_PID, PEER_PID];
        let b_links = vec![SOURCE_PID, OTHER_PID, SOURCE_PID];
        let (a, b) = unlink_processes(&a_links, &b_links, SOURCE_PID, PEER_PID);

        assert_eq!(a, vec![OTHER_PID]);
        assert_eq!(b, vec![OTHER_PID]);
        assert!(!a.contains(&PEER_PID));
        assert!(!b.contains(&SOURCE_PID));
    }

    // r[verify beam-link.propagation]
    #[test]
    fn test_abnormal_exit_cascades() {
        let proc = spawn_process(PEER_PID);
        let (next, mailbox) = deliver_exit_signal(&proc, &vec![], SOURCE_PID, REASON_ERROR);
        assert_eq!(next.state, crate::beam_process::BEAM_EXITING);
        assert_eq!(next.exit_reason, REASON_ERROR);
        assert!(mailbox.is_empty());
    }

    // r[verify beam-link.propagation]
    #[test]
    fn test_normal_exit_does_not_cascade() {
        let proc = spawn_process(PEER_PID);
        let (next, mailbox) = deliver_exit_signal(&proc, &vec![], SOURCE_PID, REASON_NORMAL);

        assert_eq!(next.state, BEAM_ALIVE);
        assert_eq!(next.exit_reason, REASON_NORMAL);
        assert!(mailbox.is_empty());
    }

    // r[verify beam-link.trap-exit]
    #[test]
    fn test_trap_exit_turns_signal_into_message() {
        let proc = crate::beam_process::set_trap_exit(&spawn_process(PEER_PID), true);
        let (next, mailbox) = deliver_exit_signal(&proc, &vec![], SOURCE_PID, REASON_ERROR);
        assert_eq!(next.state, BEAM_ALIVE);
        assert_eq!(mailbox.len(), 1);
        assert_eq!(mailbox[0].kind, MESSAGE_EXIT);
        assert_eq!(mailbox[0].sender, SOURCE_PID);
        assert_eq!(mailbox[0].observed_pid, SOURCE_PID);
        assert_eq!(mailbox[0].reason, REASON_ERROR);
    }

    // r[verify beam-link.trap-exit]
    #[test]
    fn test_kill_bypasses_trap_exit() {
        let proc = crate::beam_process::set_trap_exit(&spawn_process(PEER_PID), true);
        let (next, mailbox) = deliver_exit_signal(&proc, &vec![], SOURCE_PID, crate::beam_process::REASON_KILL);
        assert_eq!(next.state, crate::beam_process::BEAM_EXITING);
        assert_eq!(next.exit_reason, crate::beam_process::REASON_KILLED);
        assert!(mailbox.is_empty());
    }

    // r[verify beam-link.propagation]
    #[test]
    fn test_inactive_dead_process_ignores_exit_signal() {
        let mut proc = spawn_process(PEER_PID);
        proc.state = BEAM_DEAD;
        let (next, mailbox) = deliver_exit_signal(&proc, &vec![], SOURCE_PID, REASON_ERROR);
        assert_eq!(next.state, BEAM_DEAD);
        assert!(mailbox.is_empty());
    }

    proptest! {
        // r[verify beam-link.symmetry]
        #[test]
        fn prop_link_contains_peer(a: u64, b: u64) {
            let (a_links, b_links) = link_processes(&vec![], &vec![], a, b);
            prop_assert!(a_links.contains(&b));
            prop_assert!(b_links.contains(&a));
        }
    }
}
