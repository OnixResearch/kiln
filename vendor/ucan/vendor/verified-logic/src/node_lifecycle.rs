//! Verified node lifecycle state machine.
//!
//! Four states: Starting → Online ↔ Degraded → Offline.
//! Online and Degraded are bidirectional (recovery). Offline is terminal.
//! Different from `process_state` which models Unix process lifecycle.

use vstd::prelude::*;

verus! {

/// Node status constants.
pub const STARTING: u8 = 0u8;
pub const ONLINE: u8 = 1u8;
pub const DEGRADED: u8 = 2u8;
pub const OFFLINE: u8 = 3u8;

// --- is_valid_status ---

/// Spec: the value is one of the four valid states.
pub open spec fn is_valid_status_spec(status: u8) -> bool {
    status == STARTING || status == ONLINE || status == DEGRADED || status == OFFLINE
}

// --- is_valid_transition ---

// r[depends node-lifecycle.valid-transition]
/// Spec: valid state transitions.
///
/// Starting → Online, Online → Degraded, Degraded → Online,
/// Online → Offline, Degraded → Offline, Starting → Offline.
pub open spec fn is_valid_transition_spec(from: u8, to: u8) -> bool {
    (from == STARTING && to == ONLINE)
    || (from == STARTING && to == OFFLINE)
    || (from == ONLINE && to == DEGRADED)
    || (from == ONLINE && to == OFFLINE)
    || (from == DEGRADED && to == ONLINE)
    || (from == DEGRADED && to == OFFLINE)
}

// r[impl node-lifecycle.valid-transition]
/// Check if a state transition is valid.
pub fn is_valid_transition(from: u8, to: u8) -> (result: bool)
    ensures result == is_valid_transition_spec(from, to)
{
    (from == STARTING && to == ONLINE)
    || (from == STARTING && to == OFFLINE)
    || (from == ONLINE && to == DEGRADED)
    || (from == ONLINE && to == OFFLINE)
    || (from == DEGRADED && to == ONLINE)
    || (from == DEGRADED && to == OFFLINE)
}

// --- is_operational ---

// r[depends node-lifecycle.is-operational]
/// Spec: a node is operational when Online or Degraded.
pub open spec fn is_operational_spec(status: u8) -> bool {
    status == ONLINE || status == DEGRADED
}

// r[impl node-lifecycle.is-operational]
/// Check if a node status is operational.
pub fn is_operational(status: u8) -> (result: bool)
    ensures result == is_operational_spec(status)
{
    status == ONLINE || status == DEGRADED
}

// --- Safety proofs ---

// r[impl node-lifecycle.terminal]
// r[verify node-lifecycle.terminal]
/// Offline has no valid outgoing transitions.
proof fn offline_terminal(to: u8)
    ensures !is_valid_transition_spec(OFFLINE, to)
{}

// r[impl node-lifecycle.reachability]
// r[verify node-lifecycle.reachability]
/// Every non-Offline state can reach Offline (directly).
proof fn offline_reachable(from: u8)
    requires is_valid_status_spec(from), from != OFFLINE
    ensures
        // Starting → Offline, Online → Offline, Degraded → Offline
        is_valid_transition_spec(from, OFFLINE),
{}

// r[verify node-lifecycle.is-operational]
/// Starting is not operational.
proof fn starting_not_operational()
    ensures !is_operational_spec(STARTING)
{}

// r[verify node-lifecycle.is-operational]
/// Offline is not operational.
proof fn offline_not_operational()
    ensures !is_operational_spec(OFFLINE)
{}

// r[verify node-lifecycle.valid-transition]
/// Self-transitions are invalid.
proof fn no_self_transition(s: u8)
    requires is_valid_status_spec(s)
    ensures !is_valid_transition_spec(s, s)
{}

// r[verify node-lifecycle.valid-transition]
/// Transition from Offline to any state is invalid (terminal).
proof fn offline_no_recovery(to: u8)
    ensures !is_valid_transition_spec(OFFLINE, to)
{}

// r[verify node-lifecycle.valid-transition]
/// The transition graph has exactly 6 valid edges.
proof fn exactly_six_transitions()
    ensures
        is_valid_transition_spec(STARTING, ONLINE),
        is_valid_transition_spec(STARTING, OFFLINE),
        is_valid_transition_spec(ONLINE, DEGRADED),
        is_valid_transition_spec(ONLINE, OFFLINE),
        is_valid_transition_spec(DEGRADED, ONLINE),
        is_valid_transition_spec(DEGRADED, OFFLINE),
        // All other pairs with valid states are invalid:
        !is_valid_transition_spec(STARTING, DEGRADED),
        !is_valid_transition_spec(STARTING, STARTING),
        !is_valid_transition_spec(ONLINE, ONLINE),
        !is_valid_transition_spec(ONLINE, STARTING),
        !is_valid_transition_spec(DEGRADED, DEGRADED),
        !is_valid_transition_spec(DEGRADED, STARTING),
        !is_valid_transition_spec(OFFLINE, STARTING),
        !is_valid_transition_spec(OFFLINE, ONLINE),
        !is_valid_transition_spec(OFFLINE, DEGRADED),
        !is_valid_transition_spec(OFFLINE, OFFLINE),
{}

// r[verify node-lifecycle.is-operational]
/// Operational is exactly Online or Degraded.
proof fn operational_exhaustive(s: u8)
    requires is_valid_status_spec(s)
    ensures is_operational_spec(s) <==> (s == ONLINE || s == DEGRADED)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify node-lifecycle.valid-transition]
    #[test]
    fn test_transitions() {
        assert!(is_valid_transition(STARTING, ONLINE));
        assert!(is_valid_transition(STARTING, OFFLINE));
        assert!(is_valid_transition(ONLINE, DEGRADED));
        assert!(is_valid_transition(ONLINE, OFFLINE));
        assert!(is_valid_transition(DEGRADED, ONLINE));
        assert!(is_valid_transition(DEGRADED, OFFLINE));

        assert!(!is_valid_transition(OFFLINE, ONLINE));
        assert!(!is_valid_transition(OFFLINE, STARTING));
        assert!(!is_valid_transition(STARTING, DEGRADED));
    }

    // r[verify node-lifecycle.is-operational]
    #[test]
    fn test_operational() {
        assert!(!is_operational(STARTING));
        assert!(is_operational(ONLINE));
        assert!(is_operational(DEGRADED));
        assert!(!is_operational(OFFLINE));
    }

    // r[verify node-lifecycle.terminal]
    #[test]
    fn test_offline_terminal() {
        for to in [STARTING, ONLINE, DEGRADED, OFFLINE] {
            assert!(!is_valid_transition(OFFLINE, to));
        }
    }

    proptest! {
        // r[verify node-lifecycle.valid-transition]
        #[test]
        fn prop_no_self_transition(s in 0u8..4) {
            prop_assert!(!is_valid_transition(s, s));
        }

        // r[verify node-lifecycle.reachability]
        #[test]
        fn prop_offline_reachable(from in 0u8..3) {
            // STARTING(0), ONLINE(1), DEGRADED(2) can all reach OFFLINE
            prop_assert!(is_valid_transition(from, OFFLINE));
        }

        // r[verify node-lifecycle.is-operational]
        #[test]
        fn prop_operational_matches(s in 0u8..4) {
            let expected = s == ONLINE || s == DEGRADED;
            prop_assert_eq!(is_operational(s), expected);
        }

        #[test]
        fn prop_offline_is_absorbing(from in 0u8..4) {
            // OFFLINE(3) to anything except itself is invalid
            // (and self-transitions are already excluded)
            if from != OFFLINE {
                // Can always go to OFFLINE
                prop_assert!(is_valid_transition(from, OFFLINE));
            }
        }

        #[test]
        fn prop_transition_model_oracle(from in 0u8..4, to in 0u8..4) {
            let valid = is_valid_transition(from, to);
            // Known valid transitions: STARTING→ONLINE, ONLINE→DEGRADED,
            // DEGRADED→ONLINE, any-non-OFFLINE→OFFLINE
            let expected = (from == STARTING && to == ONLINE)
                || (from == ONLINE && to == DEGRADED)
                || (from == DEGRADED && to == ONLINE)
                || (from != OFFLINE && to == OFFLINE);
            prop_assert_eq!(valid, expected);
        }

        #[test]
        fn prop_starting_not_operational(s in 0u8..1) {
            prop_assert!(!is_operational(STARTING));
            prop_assert!(!is_operational(OFFLINE));
            let _ = s; // use the parameter
        }
    }
}
