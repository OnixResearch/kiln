//! Verified job lifecycle state machine.
//!
//! Six states: Pending → Ready → Running → Completed / Failed / Cancelled.
//! Pending→Ready requires all dependencies completed (DAG-aware readiness).
//! Cancelled is reachable from any non-terminal state. Completed, Failed,
//! and Cancelled are terminal — no outgoing transitions.
//!
//! State encoding: 0=Pending, 1=Ready, 2=Running, 3=Completed,
//! 4=Failed, 5=Cancelled.

use vstd::prelude::*;

verus! {

/// Job state constants.
pub const PENDING: u8 = 0u8;
pub const READY: u8 = 1u8;
pub const RUNNING: u8 = 2u8;
pub const COMPLETED: u8 = 3u8;
pub const FAILED: u8 = 4u8;
pub const CANCELLED: u8 = 5u8;

// ============================================================
// is_valid_state
// ============================================================

// r[depends job-state.is-valid]
/// Spec: the value is one of the six valid job states.
pub open spec fn is_valid_state_spec(state: u8) -> bool {
    state == PENDING
    || state == READY
    || state == RUNNING
    || state == COMPLETED
    || state == FAILED
    || state == CANCELLED
}

// r[impl job-state.is-valid]
/// Check if a value is a valid job state.
pub fn is_valid_state(state: u8) -> (result: bool)
    ensures result == is_valid_state_spec(state)
{
    state <= CANCELLED
}

// ============================================================
// is_terminal
// ============================================================

// r[depends job-state.is-terminal]
/// Spec: Completed, Failed, and Cancelled are terminal.
pub open spec fn is_terminal_spec(state: u8) -> bool {
    state == COMPLETED || state == FAILED || state == CANCELLED
}

// r[impl job-state.is-terminal]
/// Check if a job state is terminal (no outgoing transitions).
pub fn is_terminal(state: u8) -> (result: bool)
    ensures result == is_terminal_spec(state)
{
    state == COMPLETED || state == FAILED || state == CANCELLED
}

// ============================================================
// is_valid_transition
// ============================================================

// r[depends job-state.valid-transition]
/// Spec: valid job state transitions.
///
/// Pending→Ready, Ready→Running, Running→Completed, Running→Failed,
/// Pending→Cancelled, Ready→Cancelled, Running→Cancelled.
pub open spec fn is_valid_transition_spec(from: u8, to: u8) -> bool {
    (from == PENDING && to == READY)
    || (from == READY && to == RUNNING)
    || (from == RUNNING && to == COMPLETED)
    || (from == RUNNING && to == FAILED)
    || (from == PENDING && to == CANCELLED)
    || (from == READY && to == CANCELLED)
    || (from == RUNNING && to == CANCELLED)
}

// r[impl job-state.valid-transition]
/// Check if a state transition is valid.
pub fn is_valid_transition(from: u8, to: u8) -> (result: bool)
    ensures result == is_valid_transition_spec(from, to)
{
    (from == PENDING && to == READY)
    || (from == READY && to == RUNNING)
    || (from == RUNNING && to == COMPLETED)
    || (from == RUNNING && to == FAILED)
    || (from == PENDING && to == CANCELLED)
    || (from == READY && to == CANCELLED)
    || (from == RUNNING && to == CANCELLED)
}

// ============================================================
// can_become_ready: DAG-aware readiness
// ============================================================

// r[depends job-state.can-become-ready]
/// Spec: all dependency states from index `from` onward are COMPLETED.
pub open spec fn all_deps_completed_from_spec(dep_states: Seq<u8>, from: int) -> bool
    decreases dep_states.len() - from
{
    if from < 0 || from >= dep_states.len() as int {
        true
    } else {
        dep_states[from] == COMPLETED
            && all_deps_completed_from_spec(dep_states, from + 1)
    }
}

/// Spec: all dependency states are COMPLETED.
pub open spec fn can_become_ready_spec(dep_states: Seq<u8>) -> bool {
    all_deps_completed_from_spec(dep_states, 0)
}

// r[impl job-state.can-become-ready]
/// Check if all dependencies are completed, enabling Pending→Ready.
pub fn can_become_ready(dep_states: &[u8]) -> (result: bool)
    ensures result == can_become_ready_spec(dep_states@)
{
    let mut i: usize = 0;
    while i < dep_states.len()
        invariant
            0 <= i <= dep_states@.len(),
            can_become_ready_spec(dep_states@)
                <==> all_deps_completed_from_spec(dep_states@, i as int),
        decreases dep_states@.len() - i,
    {
        if dep_states[i] != COMPLETED {
            return false;
        }
        i = i + 1;
    }
    true
}

// ============================================================
// Safety proofs
// ============================================================

// r[verify job-state.is-terminal]
/// Terminal states have no valid outgoing transitions.
proof fn terminal_no_outgoing(from: u8, to: u8)
    requires is_terminal_spec(from)
    ensures !is_valid_transition_spec(from, to)
{}

// r[verify job-state.valid-transition]
/// Cancelled is reachable from every non-terminal state.
proof fn cancelled_from_any_non_terminal(from: u8)
    requires is_valid_state_spec(from), !is_terminal_spec(from)
    ensures is_valid_transition_spec(from, CANCELLED)
{}

// r[verify job-state.valid-transition]
/// Self-transitions are invalid.
proof fn no_self_transition(s: u8)
    requires is_valid_state_spec(s)
    ensures !is_valid_transition_spec(s, s)
{}

// r[verify job-state.valid-transition]
/// The transition graph has exactly 7 valid edges.
proof fn exactly_seven_transitions()
    ensures
        is_valid_transition_spec(PENDING, READY),
        is_valid_transition_spec(READY, RUNNING),
        is_valid_transition_spec(RUNNING, COMPLETED),
        is_valid_transition_spec(RUNNING, FAILED),
        is_valid_transition_spec(PENDING, CANCELLED),
        is_valid_transition_spec(READY, CANCELLED),
        is_valid_transition_spec(RUNNING, CANCELLED),
        // All terminal outgoing are invalid:
        !is_valid_transition_spec(COMPLETED, PENDING),
        !is_valid_transition_spec(COMPLETED, READY),
        !is_valid_transition_spec(COMPLETED, RUNNING),
        !is_valid_transition_spec(COMPLETED, FAILED),
        !is_valid_transition_spec(COMPLETED, CANCELLED),
        !is_valid_transition_spec(COMPLETED, COMPLETED),
        !is_valid_transition_spec(FAILED, PENDING),
        !is_valid_transition_spec(FAILED, READY),
        !is_valid_transition_spec(FAILED, RUNNING),
        !is_valid_transition_spec(FAILED, COMPLETED),
        !is_valid_transition_spec(FAILED, CANCELLED),
        !is_valid_transition_spec(FAILED, FAILED),
        !is_valid_transition_spec(CANCELLED, PENDING),
        !is_valid_transition_spec(CANCELLED, READY),
        !is_valid_transition_spec(CANCELLED, RUNNING),
        !is_valid_transition_spec(CANCELLED, COMPLETED),
        !is_valid_transition_spec(CANCELLED, FAILED),
        !is_valid_transition_spec(CANCELLED, CANCELLED),
        // Invalid non-terminal edges:
        !is_valid_transition_spec(PENDING, RUNNING),
        !is_valid_transition_spec(PENDING, COMPLETED),
        !is_valid_transition_spec(PENDING, FAILED),
        !is_valid_transition_spec(PENDING, PENDING),
        !is_valid_transition_spec(READY, PENDING),
        !is_valid_transition_spec(READY, COMPLETED),
        !is_valid_transition_spec(READY, FAILED),
        !is_valid_transition_spec(READY, READY),
        !is_valid_transition_spec(RUNNING, PENDING),
        !is_valid_transition_spec(RUNNING, READY),
        !is_valid_transition_spec(RUNNING, RUNNING),
{}

// r[verify job-state.can-become-ready]
/// No dependencies means always ready.
proof fn no_deps_always_ready()
    ensures can_become_ready_spec(seq![])
{}

// r[verify job-state.can-become-ready]
/// A failed dependency blocks readiness.
proof fn failed_dep_blocks_ready(dep_states: Seq<u8>, idx: int)
    requires
        0 <= idx < dep_states.len() as int,
        dep_states[idx] == FAILED,
    ensures !can_become_ready_spec(dep_states)
{
    failed_dep_blocks_from(dep_states, idx, 0);
}

proof fn failed_dep_blocks_from(dep_states: Seq<u8>, idx: int, from: int)
    requires
        0 <= from <= idx,
        idx < dep_states.len() as int,
        dep_states[idx] == FAILED,
    ensures !all_deps_completed_from_spec(dep_states, from)
    decreases dep_states.len() - from
{
    if from == idx {
        // dep_states[from] == FAILED != COMPLETED
    } else {
        failed_dep_blocks_from(dep_states, idx, from + 1);
    }
}

// r[verify job-state.can-become-ready]
/// A cancelled dependency blocks readiness.
proof fn cancelled_dep_blocks_ready(dep_states: Seq<u8>, idx: int)
    requires
        0 <= idx < dep_states.len() as int,
        dep_states[idx] == CANCELLED,
    ensures !can_become_ready_spec(dep_states)
{
    cancelled_dep_blocks_from(dep_states, idx, 0);
}

proof fn cancelled_dep_blocks_from(dep_states: Seq<u8>, idx: int, from: int)
    requires
        0 <= from <= idx,
        idx < dep_states.len() as int,
        dep_states[idx] == CANCELLED,
    ensures !all_deps_completed_from_spec(dep_states, from)
    decreases dep_states.len() - from
{
    if from == idx {
        // dep_states[from] == CANCELLED != COMPLETED
    } else {
        cancelled_dep_blocks_from(dep_states, idx, from + 1);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify job-state.is-valid]
    #[test]
    fn test_valid_states() {
        assert!(is_valid_state(PENDING));
        assert!(is_valid_state(READY));
        assert!(is_valid_state(RUNNING));
        assert!(is_valid_state(COMPLETED));
        assert!(is_valid_state(FAILED));
        assert!(is_valid_state(CANCELLED));
        assert!(!is_valid_state(6));
        assert!(!is_valid_state(255));
    }

    // r[verify job-state.is-terminal]
    #[test]
    fn test_terminal() {
        assert!(!is_terminal(PENDING));
        assert!(!is_terminal(READY));
        assert!(!is_terminal(RUNNING));
        assert!(is_terminal(COMPLETED));
        assert!(is_terminal(FAILED));
        assert!(is_terminal(CANCELLED));
    }

    // r[verify job-state.valid-transition]
    #[test]
    fn test_valid_transitions() {
        assert!(is_valid_transition(PENDING, READY));
        assert!(is_valid_transition(READY, RUNNING));
        assert!(is_valid_transition(RUNNING, COMPLETED));
        assert!(is_valid_transition(RUNNING, FAILED));
        assert!(is_valid_transition(PENDING, CANCELLED));
        assert!(is_valid_transition(READY, CANCELLED));
        assert!(is_valid_transition(RUNNING, CANCELLED));
    }

    // r[verify job-state.valid-transition]
    #[test]
    fn test_invalid_transitions() {
        assert!(!is_valid_transition(COMPLETED, RUNNING));
        assert!(!is_valid_transition(FAILED, READY));
        assert!(!is_valid_transition(CANCELLED, PENDING));
        assert!(!is_valid_transition(PENDING, RUNNING));
        assert!(!is_valid_transition(PENDING, COMPLETED));
        assert!(!is_valid_transition(READY, PENDING));
    }

    // r[verify job-state.can-become-ready]
    #[test]
    fn test_can_become_ready() {
        assert!(can_become_ready(&[]));
        assert!(can_become_ready(&[COMPLETED, COMPLETED, COMPLETED]));
        assert!(!can_become_ready(&[COMPLETED, RUNNING, COMPLETED]));
        assert!(!can_become_ready(&[COMPLETED, FAILED]));
        assert!(!can_become_ready(&[CANCELLED]));
        assert!(!can_become_ready(&[PENDING]));
    }

    proptest! {
        // r[verify job-state.valid-transition]
        #[test]
        fn prop_no_self_transition(s in 0u8..6) {
            prop_assert!(!is_valid_transition(s, s));
        }

        // r[verify job-state.valid-transition]
        #[test]
        fn prop_terminal_no_outgoing(from in 3u8..6, to in 0u8..6) {
            prop_assert!(!is_valid_transition(from, to));
        }

        // r[verify job-state.valid-transition]
        #[test]
        fn prop_cancel_from_non_terminal(from in 0u8..3) {
            prop_assert!(is_valid_transition(from, CANCELLED));
        }

        // r[verify job-state.can-become-ready]
        #[test]
        fn prop_no_deps_always_ready(
            _x: u8,
        ) {
            let dep_states: Vec<u8> = vec![];
            prop_assert!(can_become_ready(&dep_states));
        }

        // r[verify job-state.can-become-ready]
        #[test]
        fn prop_all_completed_is_ready(
            count in 1usize..20,
        ) {
            let dep_states = vec![COMPLETED; count];
            prop_assert!(can_become_ready(&dep_states));
        }

        // r[verify job-state.can-become-ready]
        #[test]
        fn prop_any_non_completed_blocks(
            prefix_len in 0usize..10,
            bad_state in 0u8..5,
            suffix_len in 0usize..10,
        ) {
            if bad_state != COMPLETED {
                let mut deps = vec![COMPLETED; prefix_len];
                deps.push(bad_state);
                deps.extend(vec![COMPLETED; suffix_len]);
                prop_assert!(!can_become_ready(&deps));
            }
        }
    }
}
