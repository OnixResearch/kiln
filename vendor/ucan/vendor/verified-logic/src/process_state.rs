//! Verified process lifecycle state machine.
//!
//! Models the five standard process states (Created, Ready, Running,
//! Blocked, Terminated) and their valid transitions. The transition
//! function encodes the exact edges of the state graph; proofs verify
//! that terminal states have no successors and that only Ready
//! processes are schedulable.
//!
//! State encoding: 0=Created, 1=Ready, 2=Running, 3=Blocked, 4=Terminated.

#![allow(clippy::nonminimal_bool)]

use vstd::prelude::*;

verus! {

// --- is_valid_state ---

// r[depends process-state.is-valid]
/// Spec: state is one of the five process states.
pub open spec fn is_valid_state_spec(state: u64) -> bool {
    state <= 4
}

// r[impl process-state.is-valid]
/// Returns true if state is a valid process state.
pub fn is_valid_state(state: u64) -> (result: bool)
    ensures result == is_valid_state_spec(state)
{
    state <= 4
}

// r[verify process-state.is-valid]
/// All five states are valid.
proof fn all_states_valid()
    ensures
        is_valid_state_spec(0),  // Created
        is_valid_state_spec(1),  // Ready
        is_valid_state_spec(2),  // Running
        is_valid_state_spec(3),  // Blocked
        is_valid_state_spec(4),  // Terminated
{}

// r[verify process-state.is-valid]
/// Values above 4 are invalid.
proof fn out_of_range_invalid(state: u64)
    requires state > 4
    ensures !is_valid_state_spec(state)
{}

// --- can_transition ---

// r[depends process-state.can-transition]
/// Spec: whether a state transition is valid.
///
/// Valid transitions:
///   Created(0) -> Ready(1)         — process admitted
///   Ready(1)   -> Running(2)       — scheduler dispatch
///   Running(2) -> Ready(1)         — preempted
///   Running(2) -> Blocked(3)       — I/O wait
///   Running(2) -> Terminated(4)    — exit
///   Blocked(3) -> Ready(1)         — I/O complete
pub open spec fn can_transition_spec(from: u64, to: u64) -> bool {
    (from == 0 && to == 1)       // Created -> Ready
    || (from == 1 && to == 2)    // Ready -> Running
    || (from == 2 && to == 1)    // Running -> Ready
    || (from == 2 && to == 3)    // Running -> Blocked
    || (from == 2 && to == 4)    // Running -> Terminated
    || (from == 3 && to == 1)    // Blocked -> Ready
}

// r[impl process-state.can-transition]
/// Returns true if the transition from -> to is valid.
pub fn can_transition(from: u64, to: u64) -> (result: bool)
    ensures result == can_transition_spec(from, to)
{
    (from == 0 && to == 1)
    || (from == 1 && to == 2)
    || (from == 2 && to == 1)
    || (from == 2 && to == 3)
    || (from == 2 && to == 4)
    || (from == 3 && to == 1)
}

// r[verify process-state.can-transition]
/// All six transitions are accepted.
proof fn all_transitions_valid()
    ensures
        can_transition_spec(0, 1),
        can_transition_spec(1, 2),
        can_transition_spec(2, 1),
        can_transition_spec(2, 3),
        can_transition_spec(2, 4),
        can_transition_spec(3, 1),
{}

// r[verify process-state.can-transition]
/// Self-transitions are forbidden.
proof fn no_self_transition(state: u64)
    ensures !can_transition_spec(state, state)
{}

// r[verify process-state.can-transition]
/// Both endpoints of a valid transition are valid states.
proof fn transition_preserves_validity(from: u64, to: u64)
    requires can_transition_spec(from, to)
    ensures is_valid_state_spec(from), is_valid_state_spec(to)
{}

// r[verify process-state.can-transition]
/// Created can only go to Ready.
proof fn created_only_to_ready(to: u64)
    requires can_transition_spec(0, to)
    ensures to == 1
{}

// r[verify process-state.can-transition]
/// Running has exactly three successors.
proof fn running_successors(to: u64)
    requires can_transition_spec(2, to)
    ensures to == 1 || to == 3 || to == 4
{}

// --- is_terminal ---

// r[depends process-state.is-terminal]
/// Spec: state is Terminated (no further transitions possible).
pub open spec fn is_terminal_spec(state: u64) -> bool {
    state == 4
}

// r[impl process-state.is-terminal]
/// Returns true if the process is terminated.
pub fn is_terminal(state: u64) -> (result: bool)
    ensures result == is_terminal_spec(state)
{
    state == 4
}

// r[verify process-state.is-terminal]
/// Terminal states have no valid outgoing transitions.
proof fn terminal_no_successors(to: u64)
    ensures !can_transition_spec(4, to)
{}

// r[verify process-state.is-terminal]
/// Terminated is the only terminal state.
proof fn only_terminated_is_terminal(state: u64)
    requires is_valid_state_spec(state), is_terminal_spec(state)
    ensures state == 4
{}

// r[verify process-state.is-terminal]
/// Non-terminal valid states have at least one successor.
proof fn non_terminal_has_successor(state: u64)
    requires is_valid_state_spec(state), !is_terminal_spec(state)
    ensures
        can_transition_spec(state, 1) ||
        can_transition_spec(state, 2) ||
        can_transition_spec(state, 3) ||
        can_transition_spec(state, 4)
{}

// --- is_schedulable ---

// r[depends process-state.is-schedulable]
/// Spec: state is Ready (can be dispatched by the scheduler).
pub open spec fn is_schedulable_spec(state: u64) -> bool {
    state == 1
}

// r[impl process-state.is-schedulable]
/// Returns true if the process is schedulable (Ready).
pub fn is_schedulable(state: u64) -> (result: bool)
    ensures result == is_schedulable_spec(state)
{
    state == 1
}

// r[verify process-state.is-schedulable]
/// A schedulable process can transition to Running.
proof fn schedulable_can_run(state: u64)
    requires is_schedulable_spec(state)
    ensures can_transition_spec(state, 2)
{}

// r[verify process-state.is-schedulable]
/// Terminal states are never schedulable.
proof fn terminal_not_schedulable(state: u64)
    requires is_terminal_spec(state)
    ensures !is_schedulable_spec(state)
{}

// r[verify process-state.is-schedulable]
/// Only Ready is schedulable among valid states.
proof fn only_ready_is_schedulable(state: u64)
    requires is_valid_state_spec(state), is_schedulable_spec(state)
    ensures state == 1
{}

// --- lifecycle properties ---

// r[verify process-state.can-transition]
// r[verify process-state.is-terminal]
/// A process reachable from Created can terminate.
/// Created -> Ready -> Running -> Terminated is a valid path.
proof fn lifecycle_create_to_terminate()
    ensures
        can_transition_spec(0, 1),
        can_transition_spec(1, 2),
        can_transition_spec(2, 4),
        is_terminal_spec(4),
{}

// r[verify process-state.can-transition]
// r[verify process-state.is-schedulable]
/// Blocked processes become schedulable after wakeup.
proof fn blocked_wakeup_schedulable()
    ensures
        can_transition_spec(3, 1),
        is_schedulable_spec(1),
{}

// r[verify process-state.can-transition]
/// Preempted process returns to Ready (schedulable again).
proof fn preempt_returns_to_ready()
    ensures
        can_transition_spec(2, 1),
        is_schedulable_spec(1),
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify process-state.is-valid]
    #[test]
    fn test_is_valid_state() {
        assert!(is_valid_state(0)); // Created
        assert!(is_valid_state(1)); // Ready
        assert!(is_valid_state(2)); // Running
        assert!(is_valid_state(3)); // Blocked
        assert!(is_valid_state(4)); // Terminated
        assert!(!is_valid_state(5));
        assert!(!is_valid_state(u64::MAX));
    }

    // r[verify process-state.can-transition]
    #[test]
    fn test_valid_transitions() {
        // All valid transitions
        assert!(can_transition(0, 1)); // Created -> Ready
        assert!(can_transition(1, 2)); // Ready -> Running
        assert!(can_transition(2, 1)); // Running -> Ready (preempt)
        assert!(can_transition(2, 3)); // Running -> Blocked
        assert!(can_transition(2, 4)); // Running -> Terminated
        assert!(can_transition(3, 1)); // Blocked -> Ready (wakeup)
    }

    // r[verify process-state.can-transition]
    #[test]
    fn test_invalid_transitions() {
        // Self-transitions
        for s in 0..=4u64 {
            assert!(!can_transition(s, s));
        }
        // Invalid edges
        assert!(!can_transition(0, 2)); // Created -> Running
        assert!(!can_transition(0, 3)); // Created -> Blocked
        assert!(!can_transition(0, 4)); // Created -> Terminated
        assert!(!can_transition(1, 0)); // Ready -> Created
        assert!(!can_transition(1, 3)); // Ready -> Blocked
        assert!(!can_transition(3, 2)); // Blocked -> Running
        assert!(!can_transition(4, 0)); // Terminated -> anything
        assert!(!can_transition(4, 1));
        assert!(!can_transition(4, 2));
        assert!(!can_transition(4, 3));
    }

    // r[verify process-state.is-terminal]
    #[test]
    fn test_is_terminal() {
        assert!(!is_terminal(0));
        assert!(!is_terminal(1));
        assert!(!is_terminal(2));
        assert!(!is_terminal(3));
        assert!(is_terminal(4));
    }

    // r[verify process-state.is-schedulable]
    #[test]
    fn test_is_schedulable() {
        assert!(!is_schedulable(0)); // Created — not yet admitted
        assert!(is_schedulable(1)); // Ready — can run
        assert!(!is_schedulable(2)); // Running — already dispatched
        assert!(!is_schedulable(3)); // Blocked — waiting for I/O
        assert!(!is_schedulable(4)); // Terminated — done
    }

    proptest! {
        // r[verify process-state.can-transition]
        #[test]
        fn prop_no_self_transition(state: u64) {
            prop_assert!(!can_transition(state, state));
        }

        // r[verify process-state.can-transition]
        #[test]
        fn prop_transition_preserves_validity(from: u64, to: u64) {
            if can_transition(from, to) {
                prop_assert!(is_valid_state(from));
                prop_assert!(is_valid_state(to));
            }
        }

        // r[verify process-state.is-terminal]
        #[test]
        fn prop_terminal_no_successors(to: u64) {
            prop_assert!(!can_transition(4, to));
        }

        // r[verify process-state.is-terminal]
        // r[verify process-state.is-schedulable]
        #[test]
        fn prop_terminal_not_schedulable(state: u64) {
            if is_terminal(state) {
                prop_assert!(!is_schedulable(state));
            }
        }

        // r[verify process-state.is-schedulable]
        // r[verify process-state.can-transition]
        #[test]
        fn prop_schedulable_can_run(state: u64) {
            if is_schedulable(state) {
                prop_assert!(can_transition(state, 2));
            }
        }

        // r[verify process-state.is-valid]
        // r[verify process-state.is-terminal]
        #[test]
        fn prop_non_terminal_has_successor(state in 0u64..=4) {
            if !is_terminal(state) {
                let has_successor =
                    can_transition(state, 0) ||
                    can_transition(state, 1) ||
                    can_transition(state, 2) ||
                    can_transition(state, 3) ||
                    can_transition(state, 4);
                prop_assert!(has_successor);
            }
        }

        // r[verify process-state.can-transition]
        /// Exactly 6 valid transitions exist in the state graph.
        #[test]
        fn prop_transition_count(from in 0u64..=4, to in 0u64..=4) {
            let valid = can_transition(from, to);
            let expected = matches!(
                (from, to),
                (0 | 2 | 3, 1) | (1, 2) | (2, 3 | 4)
            );
            prop_assert_eq!(valid, expected);
        }
    }
}
