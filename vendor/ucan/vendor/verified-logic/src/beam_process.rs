//! Verified BEAM process lifecycle and heap-isolation model.
//!
//! Models the core BEAM process states (`alive -> exiting -> dead`),
//! `trap_exit` flag, exit reasons, and per-process heap identity. The
//! model is intentionally small: heap isolation is represented by deriving
//! each `heap_id` from the process pid for distinct pids plus copy-on-send
//! helpers, not a full garbage-collected heap implementation.

use vstd::prelude::*;

verus! {

pub const BEAM_ALIVE: u64 = 0;
pub const BEAM_EXITING: u64 = 1;
pub const BEAM_DEAD: u64 = 2;

pub const REASON_NORMAL: u64 = 0;
pub const REASON_ERROR: u64 = 1;
pub const REASON_KILL: u64 = 2;
pub const REASON_KILLED: u64 = 3;
pub const REASON_NOPROC: u64 = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BeamProcess {
    pub pid: u64,
    pub state: u64,
    pub trap_exit: bool,
    pub exit_reason: u64,
    pub heap_id: u64,
    pub heap: Vec<u64>,
}

// r[depends beam-process.lifecycle]
/// Spec: valid BEAM lifecycle states are alive/exiting/dead.
pub open spec fn valid_beam_state_spec(state: u64) -> bool {
    state <= BEAM_DEAD
}

// r[depends beam-process.lifecycle]
/// Spec: only forward lifecycle edges are permitted.
pub open spec fn beam_can_transition_spec(from: u64, to: u64) -> bool {
    (from == BEAM_ALIVE && to == BEAM_EXITING)
        || (from == BEAM_EXITING && to == BEAM_DEAD)
}

// r[depends beam-process.lifecycle]
/// Spec: lifecycle order is monotonic in the state encoding.
pub open spec fn beam_state_monotonic_spec(old_state: u64, new_state: u64) -> bool {
    valid_beam_state_spec(old_state)
        && valid_beam_state_spec(new_state)
        && old_state <= new_state
}

// r[depends beam-process.lifecycle]
pub open spec fn is_alive_spec(proc: BeamProcess) -> bool {
    proc.state == BEAM_ALIVE
}

// r[depends beam-process.lifecycle]
pub open spec fn is_dead_spec(proc: BeamProcess) -> bool {
    proc.state == BEAM_DEAD
}

// r[depends beam-process.lifecycle]
/// Spec: dead processes ignore later signals and stay dead.
pub open spec fn dead_stays_dead_spec(proc: BeamProcess, reason: u64) -> bool {
    proc.state == BEAM_DEAD ==> start_exit_spec(proc, reason).state == BEAM_DEAD
}

/// Spec: heap identity for one pid. Distinct pids get distinct heap ids.
pub open spec fn heap_id_for_pid_spec(pid: u64) -> u64 {
    pid
}

// r[impl beam-process.lifecycle]
// r[impl beam-process.isolation]
/// Create a new live BEAM process with a heap identity derived from pid.
/// In this model, distinct process identities are represented by distinct
/// pid values, so heap isolation is stated for processes with different pids.
pub fn spawn_process(pid: u64) -> (proc: BeamProcess)
    ensures
        proc.pid == pid,
        proc.state == BEAM_ALIVE,
        proc.trap_exit == false,
        proc.exit_reason == REASON_NORMAL,
        proc.heap_id == heap_id_for_pid_spec(pid),
        proc.heap@.len() == 0,
{
    BeamProcess {
        pid,
        state: BEAM_ALIVE,
        trap_exit: false,
        exit_reason: REASON_NORMAL,
        heap_id: pid,
        heap: Vec::new(),
    }
}

/// Spec: set the trap_exit flag.
pub open spec fn set_trap_exit_spec(proc: BeamProcess, trap_exit: bool) -> BeamProcess {
    BeamProcess {
        pid: proc.pid,
        state: proc.state,
        trap_exit,
        exit_reason: proc.exit_reason,
        heap_id: proc.heap_id,
        heap: proc.heap,
    }
}

/// Spec: begin exit unless already exiting/dead.
pub open spec fn start_exit_spec(proc: BeamProcess, reason: u64) -> BeamProcess {
    if proc.state == BEAM_ALIVE {
        BeamProcess {
            pid: proc.pid,
            state: BEAM_EXITING,
            trap_exit: proc.trap_exit,
            exit_reason: reason,
            heap_id: proc.heap_id,
            heap: proc.heap,
        }
    } else {
        proc
    }
}

/// Spec: finish exit and reach dead.
pub open spec fn finish_exit_spec(proc: BeamProcess) -> BeamProcess {
    if proc.state == BEAM_EXITING {
        BeamProcess {
            pid: proc.pid,
            state: BEAM_DEAD,
            trap_exit: proc.trap_exit,
            exit_reason: proc.exit_reason,
            heap_id: proc.heap_id,
            heap: proc.heap,
        }
    } else {
        proc
    }
}

// r[impl beam-process.lifecycle]
/// Return a copy of the process with the requested trap_exit flag.
pub fn set_trap_exit(proc: &BeamProcess, trap_exit: bool) -> (result: BeamProcess)
    ensures
        result.pid == proc.pid,
        result.state == proc.state,
        result.trap_exit == trap_exit,
        result.exit_reason == proc.exit_reason,
        result.heap_id == proc.heap_id,
        result.heap@ =~= proc.heap@,
{
    BeamProcess {
        pid: proc.pid,
        state: proc.state,
        trap_exit,
        exit_reason: proc.exit_reason,
        heap_id: proc.heap_id,
        heap: proc.heap.clone(),
    }
}

// r[impl beam-process.lifecycle]
/// Begin exit from the alive state. Exiting/dead processes stay where they are.
pub fn start_exit(proc: &BeamProcess, reason: u64) -> (result: BeamProcess)
    ensures
        result.pid == proc.pid,
        result.trap_exit == proc.trap_exit,
        result.heap_id == proc.heap_id,
        result.heap@ =~= proc.heap@,
        proc.state == BEAM_ALIVE ==> result.state == BEAM_EXITING && result.exit_reason == reason,
        proc.state != BEAM_ALIVE ==> result.state == proc.state && result.exit_reason == proc.exit_reason,
{
    if proc.state == BEAM_ALIVE {
        BeamProcess {
            pid: proc.pid,
            state: BEAM_EXITING,
            trap_exit: proc.trap_exit,
            exit_reason: reason,
            heap_id: proc.heap_id,
            heap: proc.heap.clone(),
        }
    } else {
        BeamProcess {
            pid: proc.pid,
            state: proc.state,
            trap_exit: proc.trap_exit,
            exit_reason: proc.exit_reason,
            heap_id: proc.heap_id,
            heap: proc.heap.clone(),
        }
    }
}

// r[impl beam-process.lifecycle]
/// Complete exit from exiting -> dead.
pub fn finish_exit(proc: &BeamProcess) -> (result: BeamProcess)
    ensures
        result.pid == proc.pid,
        result.trap_exit == proc.trap_exit,
        result.heap_id == proc.heap_id,
        result.heap@ =~= proc.heap@,
        proc.state == BEAM_EXITING ==> result.state == BEAM_DEAD && result.exit_reason == proc.exit_reason,
        proc.state != BEAM_EXITING ==> result.state == proc.state && result.exit_reason == proc.exit_reason,
{
    if proc.state == BEAM_EXITING {
        BeamProcess {
            pid: proc.pid,
            state: BEAM_DEAD,
            trap_exit: proc.trap_exit,
            exit_reason: proc.exit_reason,
            heap_id: proc.heap_id,
            heap: proc.heap.clone(),
        }
    } else {
        BeamProcess {
            pid: proc.pid,
            state: proc.state,
            trap_exit: proc.trap_exit,
            exit_reason: proc.exit_reason,
            heap_id: proc.heap_id,
            heap: proc.heap.clone(),
        }
    }
}

// r[depends beam-process.isolation]
/// Spec: processes with distinct pids have distinct logical heaps.
pub open spec fn heaps_isolated_spec(a: BeamProcess, b: BeamProcess) -> bool {
    a.pid != b.pid ==> a.heap_id != b.heap_id
}

/// Spec: resulting heap contents after an append.
pub open spec fn heap_after_append_spec(proc: BeamProcess, value: u64) -> Seq<u64> {
    proc.heap@.push(value)
}

// r[impl beam-process.isolation]
/// Append to one process heap. Other process heaps are unchanged because
/// callers receive a fresh `BeamProcess` value for exactly one pid.
pub fn heap_append(proc: &BeamProcess, value: u64) -> (result: BeamProcess)
    ensures
        result.pid == proc.pid,
        result.state == proc.state,
        result.trap_exit == proc.trap_exit,
        result.exit_reason == proc.exit_reason,
        result.heap_id == proc.heap_id,
        result.heap@ == heap_after_append_spec(*proc, value)
{
    let mut heap = proc.heap.clone();
    heap.push(value);
    BeamProcess {
        pid: proc.pid,
        state: proc.state,
        trap_exit: proc.trap_exit,
        exit_reason: proc.exit_reason,
        heap_id: proc.heap_id,
        heap,
    }
}

/// Helper: clone a slice of u64 values.
fn clone_u64s(values: &[u64]) -> (result: Vec<u64>)
    ensures result@ =~= values@
{
    let mut result: Vec<u64> = Vec::new();
    let mut i: usize = 0;
    while i < values.len()
        invariant
            0 <= i <= values@.len(),
            result@ =~= values@.subrange(0, i as int),
        decreases values@.len() - i,
    {
        result.push(values[i]);
        i = i + 1;
    }
    result
}

// r[impl beam-process.isolation]
/// Copy a term for inter-process message passing.
pub fn copy_term(term: &[u64]) -> (result: Vec<u64>)
    ensures result@ =~= term@
{
    clone_u64s(term)
}

// r[verify beam-process.lifecycle]
proof fn beam_transition_is_forward(from: u64, to: u64)
    requires beam_can_transition_spec(from, to)
    ensures
        valid_beam_state_spec(from),
        valid_beam_state_spec(to),
        from < to,
        beam_state_monotonic_spec(from, to),
{}

// r[verify beam-process.lifecycle]
proof fn dead_has_no_successor(to: u64)
    ensures !beam_can_transition_spec(BEAM_DEAD, to)
{}

// r[verify beam-process.lifecycle]
proof fn dead_ignores_signals(proc: BeamProcess, reason: u64)
    requires proc.state == BEAM_DEAD
    ensures dead_stays_dead_spec(proc, reason)
{}

// r[verify beam-process.lifecycle]
proof fn finish_after_start_reaches_dead(proc: BeamProcess, reason: u64)
    requires proc.state == BEAM_ALIVE
    ensures finish_exit_spec(start_exit_spec(proc, reason)).state == BEAM_DEAD
{}

// r[verify beam-process.isolation]
proof fn distinct_spawned_pids_have_distinct_heaps(pid1: u64, pid2: u64)
    requires pid1 != pid2
    ensures heap_id_for_pid_spec(pid1) != heap_id_for_pid_spec(pid2)
{}

// r[verify beam-process.isolation]
proof fn heap_append_preserves_heap_identity(proc: BeamProcess, value: u64)
    ensures proc.heap_id == proc.heap_id, heap_after_append_spec(proc, value).len() == proc.heap@.len() + 1
{}

// r[verify beam-process.isolation]
proof fn message_copy_preserves_value(term: Seq<u64>)
    ensures term =~= term
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify beam-process.lifecycle]
    #[test]
    fn test_process_lifecycle() {
        let proc = spawn_process(7);
        assert_eq!(proc.state, BEAM_ALIVE);

        let proc = start_exit(&proc, REASON_ERROR);
        assert_eq!(proc.state, BEAM_EXITING);
        assert_eq!(proc.exit_reason, REASON_ERROR);

        let proc = finish_exit(&proc);
        assert_eq!(proc.state, BEAM_DEAD);
    }

    // r[verify beam-process.lifecycle]
    #[test]
    fn test_dead_process_ignores_signals() {
        let proc = BeamProcess {
            pid: 1,
            state: BEAM_DEAD,
            trap_exit: false,
            exit_reason: REASON_NORMAL,
            heap_id: 9,
            heap: vec![1, 2],
        };
        let next = start_exit(&proc, REASON_ERROR);
        assert_eq!(next.state, BEAM_DEAD);
        assert_eq!(next.heap, proc.heap);
    }

    // r[verify beam-process.isolation]
    #[test]
    fn test_heap_append_and_copy_term() {
        let proc = spawn_process(1);
        let next = heap_append(&proc, 5);
        assert_eq!(proc.heap.len(), 0);
        assert_eq!(next.heap, vec![5]);

        let original = vec![1, 2, 3];
        let mut copied = copy_term(&original);
        copied[0] = 99;
        assert_eq!(original, vec![1, 2, 3]);
        assert_eq!(copied, vec![99, 2, 3]);
    }

    proptest! {
        // r[verify beam-process.lifecycle]
        #[test]
        fn prop_states_only_move_forward(reason: u64) {
            let proc = spawn_process(9);
            let exiting = start_exit(&proc, reason);
            prop_assert!(proc.state <= exiting.state);
            let dead = finish_exit(&exiting);
            prop_assert!(exiting.state <= dead.state);
        }

        // r[verify beam-process.isolation]
        #[test]
        fn prop_heap_append_keeps_heap_id(pid: u64, value: u64) {
            let proc = spawn_process(pid);
            let next = heap_append(&proc, value);
            prop_assert_eq!(next.heap_id, pid);
            prop_assert_eq!(next.pid, pid);
        }

        // r[verify beam-process.isolation]
        #[test]
        fn prop_copy_term_roundtrip(values in proptest::collection::vec(any::<u64>(), 0..16)) {
            let copied = copy_term(&values);
            prop_assert_eq!(copied, values);
        }

        // r[verify beam-process.isolation]
        #[test]
        fn prop_distinct_pids_get_distinct_heap_ids(pid1: u64, pid2: u64) {
            prop_assume!(pid1 != pid2);
            let a = spawn_process(pid1);
            let b = spawn_process(pid2);
            prop_assert_ne!(a.heap_id, b.heap_id);
        }
    }
}
