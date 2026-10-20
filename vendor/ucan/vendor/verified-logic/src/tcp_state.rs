//! Verified TCP connection state machine.
//!
//! Models the 11 TCP states from RFC 793 and their valid transitions.
//! State encoding uses u8 constants (same pattern as circuit_breaker,
//! node_lifecycle). Proofs verify reachability, terminal properties,
//! and the separation between data-transfer and shutdown states.
//!
//! State encoding:
//!   0=CLOSED, 1=LISTEN, 2=SYN_SENT, 3=SYN_RECEIVED,
//!   4=ESTABLISHED, 5=FIN_WAIT_1, 6=FIN_WAIT_2, 7=CLOSE_WAIT,
//!   8=CLOSING, 9=LAST_ACK, 10=TIME_WAIT.

#![allow(clippy::nonminimal_bool)]

use vstd::prelude::*;

verus! {

/// TCP state constants (RFC 793).
pub const TCP_CLOSED: u8 = 0u8;
pub const TCP_LISTEN: u8 = 1u8;
pub const TCP_SYN_SENT: u8 = 2u8;
pub const TCP_SYN_RECEIVED: u8 = 3u8;
pub const TCP_ESTABLISHED: u8 = 4u8;
pub const TCP_FIN_WAIT_1: u8 = 5u8;
pub const TCP_FIN_WAIT_2: u8 = 6u8;
pub const TCP_CLOSE_WAIT: u8 = 7u8;
pub const TCP_CLOSING: u8 = 8u8;
pub const TCP_LAST_ACK: u8 = 9u8;
pub const TCP_TIME_WAIT: u8 = 10u8;

// --- is_valid_state ---

// r[depends tcp-state.is-valid]
/// Spec: state is one of the 11 TCP connection states.
pub open spec fn is_valid_state_spec(state: u8) -> bool {
    state <= 10
}

// r[impl tcp-state.is-valid]
/// Returns true if state is a valid TCP state.
pub fn is_valid_state(state: u8) -> (result: bool)
    ensures result == is_valid_state_spec(state)
{
    state <= 10
}

// r[verify tcp-state.is-valid]
/// All 11 states are valid.
proof fn all_states_valid()
    ensures
        is_valid_state_spec(TCP_CLOSED),
        is_valid_state_spec(TCP_LISTEN),
        is_valid_state_spec(TCP_SYN_SENT),
        is_valid_state_spec(TCP_SYN_RECEIVED),
        is_valid_state_spec(TCP_ESTABLISHED),
        is_valid_state_spec(TCP_FIN_WAIT_1),
        is_valid_state_spec(TCP_FIN_WAIT_2),
        is_valid_state_spec(TCP_CLOSE_WAIT),
        is_valid_state_spec(TCP_CLOSING),
        is_valid_state_spec(TCP_LAST_ACK),
        is_valid_state_spec(TCP_TIME_WAIT),
{}

// r[verify tcp-state.is-valid]
/// Values above 10 are invalid.
proof fn out_of_range_invalid(state: u8)
    requires state > 10
    ensures !is_valid_state_spec(state)
{}

// --- can_transition ---

// r[depends tcp-state.can-transition]
/// Spec: whether a TCP state transition is valid.
///
/// Transitions from RFC 793:
///   CLOSED(0)       → LISTEN(1)         passive open
///   CLOSED(0)       → SYN_SENT(2)       active open
///   LISTEN(1)       → SYN_SENT(2)       send SYN
///   LISTEN(1)       → SYN_RECEIVED(3)   recv SYN
///   SYN_SENT(2)     → SYN_RECEIVED(3)   recv SYN (simultaneous open)
///   SYN_SENT(2)     → ESTABLISHED(4)    recv SYN+ACK
///   SYN_RECEIVED(3) → ESTABLISHED(4)    recv ACK of SYN
///   SYN_RECEIVED(3) → FIN_WAIT_1(5)     close
///   ESTABLISHED(4)  → FIN_WAIT_1(5)     close
///   ESTABLISHED(4)  → CLOSE_WAIT(7)     recv FIN
///   FIN_WAIT_1(5)   → FIN_WAIT_2(6)     recv ACK of FIN
///   FIN_WAIT_1(5)   → CLOSING(8)        recv FIN (simultaneous close)
///   FIN_WAIT_1(5)   → TIME_WAIT(10)     recv FIN+ACK
///   FIN_WAIT_2(6)   → TIME_WAIT(10)     recv FIN
///   CLOSE_WAIT(7)   → LAST_ACK(9)       close
///   CLOSING(8)      → TIME_WAIT(10)     recv ACK
///   LAST_ACK(9)     → CLOSED(0)         recv ACK
///   TIME_WAIT(10)   → CLOSED(0)         timeout (2MSL)
pub open spec fn can_transition_spec(from: u8, to: u8) -> bool {
    // Connection setup
       (from == TCP_CLOSED && to == TCP_LISTEN)
    || (from == TCP_CLOSED && to == TCP_SYN_SENT)
    || (from == TCP_LISTEN && to == TCP_SYN_SENT)
    || (from == TCP_LISTEN && to == TCP_SYN_RECEIVED)
    || (from == TCP_SYN_SENT && to == TCP_SYN_RECEIVED)
    || (from == TCP_SYN_SENT && to == TCP_ESTABLISHED)
    || (from == TCP_SYN_RECEIVED && to == TCP_ESTABLISHED)
    // Shutdown initiation
    || (from == TCP_SYN_RECEIVED && to == TCP_FIN_WAIT_1)
    || (from == TCP_ESTABLISHED && to == TCP_FIN_WAIT_1)
    || (from == TCP_ESTABLISHED && to == TCP_CLOSE_WAIT)
    // Active close path
    || (from == TCP_FIN_WAIT_1 && to == TCP_FIN_WAIT_2)
    || (from == TCP_FIN_WAIT_1 && to == TCP_CLOSING)
    || (from == TCP_FIN_WAIT_1 && to == TCP_TIME_WAIT)
    || (from == TCP_FIN_WAIT_2 && to == TCP_TIME_WAIT)
    // Passive close path
    || (from == TCP_CLOSE_WAIT && to == TCP_LAST_ACK)
    // Final shutdown
    || (from == TCP_CLOSING && to == TCP_TIME_WAIT)
    || (from == TCP_LAST_ACK && to == TCP_CLOSED)
    || (from == TCP_TIME_WAIT && to == TCP_CLOSED)
}

// r[impl tcp-state.can-transition]
/// Returns true if the transition from → to is valid per RFC 793.
pub fn can_transition(from: u8, to: u8) -> (result: bool)
    ensures result == can_transition_spec(from, to)
{
       (from == TCP_CLOSED && to == TCP_LISTEN)
    || (from == TCP_CLOSED && to == TCP_SYN_SENT)
    || (from == TCP_LISTEN && to == TCP_SYN_SENT)
    || (from == TCP_LISTEN && to == TCP_SYN_RECEIVED)
    || (from == TCP_SYN_SENT && to == TCP_SYN_RECEIVED)
    || (from == TCP_SYN_SENT && to == TCP_ESTABLISHED)
    || (from == TCP_SYN_RECEIVED && to == TCP_ESTABLISHED)
    || (from == TCP_SYN_RECEIVED && to == TCP_FIN_WAIT_1)
    || (from == TCP_ESTABLISHED && to == TCP_FIN_WAIT_1)
    || (from == TCP_ESTABLISHED && to == TCP_CLOSE_WAIT)
    || (from == TCP_FIN_WAIT_1 && to == TCP_FIN_WAIT_2)
    || (from == TCP_FIN_WAIT_1 && to == TCP_CLOSING)
    || (from == TCP_FIN_WAIT_1 && to == TCP_TIME_WAIT)
    || (from == TCP_FIN_WAIT_2 && to == TCP_TIME_WAIT)
    || (from == TCP_CLOSE_WAIT && to == TCP_LAST_ACK)
    || (from == TCP_CLOSING && to == TCP_TIME_WAIT)
    || (from == TCP_LAST_ACK && to == TCP_CLOSED)
    || (from == TCP_TIME_WAIT && to == TCP_CLOSED)
}

// r[verify tcp-state.can-transition]
/// All 18 transitions are accepted.
proof fn all_transitions_valid()
    ensures
        can_transition_spec(TCP_CLOSED, TCP_LISTEN),
        can_transition_spec(TCP_CLOSED, TCP_SYN_SENT),
        can_transition_spec(TCP_LISTEN, TCP_SYN_SENT),
        can_transition_spec(TCP_LISTEN, TCP_SYN_RECEIVED),
        can_transition_spec(TCP_SYN_SENT, TCP_SYN_RECEIVED),
        can_transition_spec(TCP_SYN_SENT, TCP_ESTABLISHED),
        can_transition_spec(TCP_SYN_RECEIVED, TCP_ESTABLISHED),
        can_transition_spec(TCP_SYN_RECEIVED, TCP_FIN_WAIT_1),
        can_transition_spec(TCP_ESTABLISHED, TCP_FIN_WAIT_1),
        can_transition_spec(TCP_ESTABLISHED, TCP_CLOSE_WAIT),
        can_transition_spec(TCP_FIN_WAIT_1, TCP_FIN_WAIT_2),
        can_transition_spec(TCP_FIN_WAIT_1, TCP_CLOSING),
        can_transition_spec(TCP_FIN_WAIT_1, TCP_TIME_WAIT),
        can_transition_spec(TCP_FIN_WAIT_2, TCP_TIME_WAIT),
        can_transition_spec(TCP_CLOSE_WAIT, TCP_LAST_ACK),
        can_transition_spec(TCP_CLOSING, TCP_TIME_WAIT),
        can_transition_spec(TCP_LAST_ACK, TCP_CLOSED),
        can_transition_spec(TCP_TIME_WAIT, TCP_CLOSED),
{}

// r[verify tcp-state.can-transition]
/// Self-transitions are forbidden.
proof fn no_self_transition(state: u8)
    ensures !can_transition_spec(state, state)
{}

// r[verify tcp-state.can-transition]
/// Both endpoints of a valid transition are valid states.
proof fn transition_preserves_validity(from: u8, to: u8)
    requires can_transition_spec(from, to)
    ensures is_valid_state_spec(from), is_valid_state_spec(to)
{}

// --- is_data_state ---

// r[depends tcp-state.is-data-state]
/// Spec: state allows bidirectional data transfer.
/// Only ESTABLISHED and CLOSE_WAIT can send application data.
/// CLOSE_WAIT: remote closed, local can still send.
pub open spec fn is_data_state_spec(state: u8) -> bool {
    state == TCP_ESTABLISHED || state == TCP_CLOSE_WAIT
}

// r[impl tcp-state.is-data-state]
/// Returns true if data transfer is possible in this state.
pub fn is_data_state(state: u8) -> (result: bool)
    ensures result == is_data_state_spec(state)
{
    state == TCP_ESTABLISHED || state == TCP_CLOSE_WAIT
}

// r[verify tcp-state.is-data-state]
/// ESTABLISHED allows data.
proof fn established_is_data()
    ensures is_data_state_spec(TCP_ESTABLISHED)
{}

// r[verify tcp-state.is-data-state]
/// CLOSE_WAIT allows data (half-close: remote sent FIN, local still open).
proof fn close_wait_is_data()
    ensures is_data_state_spec(TCP_CLOSE_WAIT)
{}

// r[verify tcp-state.is-data-state]
/// Setup states do not allow data.
proof fn setup_not_data(state: u8)
    requires state == TCP_CLOSED || state == TCP_LISTEN
        || state == TCP_SYN_SENT || state == TCP_SYN_RECEIVED
    ensures !is_data_state_spec(state)
{}

// r[verify tcp-state.is-data-state]
/// Teardown states (active close side) do not allow data.
proof fn teardown_not_data(state: u8)
    requires state == TCP_FIN_WAIT_1 || state == TCP_FIN_WAIT_2
        || state == TCP_CLOSING || state == TCP_TIME_WAIT
        || state == TCP_LAST_ACK
    ensures !is_data_state_spec(state)
{}

// --- is_shutdown ---

// r[depends tcp-state.is-shutdown]
/// Spec: state is part of connection teardown.
pub open spec fn is_shutdown_spec(state: u8) -> bool {
    state == TCP_FIN_WAIT_1 || state == TCP_FIN_WAIT_2
    || state == TCP_CLOSING || state == TCP_TIME_WAIT
    || state == TCP_LAST_ACK || state == TCP_CLOSE_WAIT
}

// r[impl tcp-state.is-shutdown]
/// Returns true if the connection is in the shutdown process.
pub fn is_shutdown(state: u8) -> (result: bool)
    ensures result == is_shutdown_spec(state)
{
    state == TCP_FIN_WAIT_1 || state == TCP_FIN_WAIT_2
    || state == TCP_CLOSING || state == TCP_TIME_WAIT
    || state == TCP_LAST_ACK || state == TCP_CLOSE_WAIT
}

// r[verify tcp-state.is-shutdown]
/// ESTABLISHED is not a shutdown state.
proof fn established_not_shutdown()
    ensures !is_shutdown_spec(TCP_ESTABLISHED)
{}

// r[verify tcp-state.is-shutdown]
/// All six shutdown states are recognized.
proof fn all_shutdown_states()
    ensures
        is_shutdown_spec(TCP_FIN_WAIT_1),
        is_shutdown_spec(TCP_FIN_WAIT_2),
        is_shutdown_spec(TCP_CLOSING),
        is_shutdown_spec(TCP_TIME_WAIT),
        is_shutdown_spec(TCP_LAST_ACK),
        is_shutdown_spec(TCP_CLOSE_WAIT),
{}

// --- is_connected ---

// r[depends tcp-state.is-connected]
/// Spec: a connection exists (SYN exchange completed or in progress
/// beyond the initial SYN). CLOSED, LISTEN, and SYN_SENT have no
/// established connection.
pub open spec fn is_connected_spec(state: u8) -> bool {
    state == TCP_SYN_RECEIVED || state == TCP_ESTABLISHED
    || state == TCP_FIN_WAIT_1 || state == TCP_FIN_WAIT_2
    || state == TCP_CLOSE_WAIT || state == TCP_CLOSING
    || state == TCP_LAST_ACK || state == TCP_TIME_WAIT
}

// r[impl tcp-state.is-connected]
/// Returns true if a connection exists in this state.
pub fn is_connected(state: u8) -> (result: bool)
    ensures result == is_connected_spec(state)
{
    state == TCP_SYN_RECEIVED || state == TCP_ESTABLISHED
    || state == TCP_FIN_WAIT_1 || state == TCP_FIN_WAIT_2
    || state == TCP_CLOSE_WAIT || state == TCP_CLOSING
    || state == TCP_LAST_ACK || state == TCP_TIME_WAIT
}

// r[verify tcp-state.is-connected]
/// Data states imply connected.
proof fn data_implies_connected(state: u8)
    requires is_data_state_spec(state)
    ensures is_connected_spec(state)
{}

// r[verify tcp-state.is-connected]
/// Shutdown states imply connected.
proof fn shutdown_implies_connected(state: u8)
    requires is_shutdown_spec(state)
    ensures is_connected_spec(state)
{}

// r[verify tcp-state.is-connected]
/// CLOSED, LISTEN, and SYN_SENT are not connected.
proof fn pre_connection_not_connected()
    ensures
        !is_connected_spec(TCP_CLOSED),
        !is_connected_spec(TCP_LISTEN),
        !is_connected_spec(TCP_SYN_SENT),
{}

// --- Reachability and lifecycle proofs ---

// r[impl tcp-state.established-reachable]
// r[verify tcp-state.established-reachable]
/// Three-way handshake (client): CLOSED → SYN_SENT → ESTABLISHED.
proof fn active_open_handshake()
    ensures
        can_transition_spec(TCP_CLOSED, TCP_SYN_SENT),
        can_transition_spec(TCP_SYN_SENT, TCP_ESTABLISHED),
        is_data_state_spec(TCP_ESTABLISHED),
{}

// r[verify tcp-state.established-reachable]
/// Three-way handshake (server): CLOSED → LISTEN → SYN_RECEIVED → ESTABLISHED.
proof fn passive_open_handshake()
    ensures
        can_transition_spec(TCP_CLOSED, TCP_LISTEN),
        can_transition_spec(TCP_LISTEN, TCP_SYN_RECEIVED),
        can_transition_spec(TCP_SYN_RECEIVED, TCP_ESTABLISHED),
        is_data_state_spec(TCP_ESTABLISHED),
{}

// r[verify tcp-state.can-transition]
/// Simultaneous open: both sides send SYN independently.
/// SYN_SENT → SYN_RECEIVED → ESTABLISHED.
proof fn simultaneous_open()
    ensures
        can_transition_spec(TCP_SYN_SENT, TCP_SYN_RECEIVED),
        can_transition_spec(TCP_SYN_RECEIVED, TCP_ESTABLISHED),
{}

// r[verify tcp-state.can-transition]
/// Simultaneous close: both sides send FIN independently.
/// ESTABLISHED → FIN_WAIT_1 → CLOSING → TIME_WAIT → CLOSED.
proof fn simultaneous_close()
    ensures
        can_transition_spec(TCP_ESTABLISHED, TCP_FIN_WAIT_1),
        can_transition_spec(TCP_FIN_WAIT_1, TCP_CLOSING),
        can_transition_spec(TCP_CLOSING, TCP_TIME_WAIT),
        can_transition_spec(TCP_TIME_WAIT, TCP_CLOSED),
{}

// r[verify tcp-state.can-transition]
/// Active close: local sends FIN first.
/// ESTABLISHED → FIN_WAIT_1 → FIN_WAIT_2 → TIME_WAIT → CLOSED.
proof fn active_close_path()
    ensures
        can_transition_spec(TCP_ESTABLISHED, TCP_FIN_WAIT_1),
        can_transition_spec(TCP_FIN_WAIT_1, TCP_FIN_WAIT_2),
        can_transition_spec(TCP_FIN_WAIT_2, TCP_TIME_WAIT),
        can_transition_spec(TCP_TIME_WAIT, TCP_CLOSED),
{}

// r[verify tcp-state.can-transition]
/// Passive close: remote sends FIN first.
/// ESTABLISHED → CLOSE_WAIT → LAST_ACK → CLOSED.
proof fn passive_close_path()
    ensures
        can_transition_spec(TCP_ESTABLISHED, TCP_CLOSE_WAIT),
        can_transition_spec(TCP_CLOSE_WAIT, TCP_LAST_ACK),
        can_transition_spec(TCP_LAST_ACK, TCP_CLOSED),
{}

// r[impl tcp-state.closed-terminal]
// r[verify tcp-state.closed-terminal]
/// LISTEN has no transitions back to CLOSED.
/// (CLOSED is only reached from LAST_ACK and TIME_WAIT.)
proof fn no_direct_close_from_listen(to: u8)
    requires to == TCP_CLOSED
    ensures !can_transition_spec(TCP_LISTEN, to)
{}

// r[verify tcp-state.closed-terminal]
/// Only LAST_ACK and TIME_WAIT can reach CLOSED.
proof fn closed_predecessors(from: u8)
    requires can_transition_spec(from, TCP_CLOSED)
    ensures from == TCP_LAST_ACK || from == TCP_TIME_WAIT
{}

// r[verify tcp-state.can-transition]
/// Every connected state has at least one outgoing transition.
proof fn connected_has_successor(state: u8)
    requires is_connected_spec(state)
    ensures
        can_transition_spec(state, TCP_CLOSED)
        || can_transition_spec(state, TCP_ESTABLISHED)
        || can_transition_spec(state, TCP_FIN_WAIT_1)
        || can_transition_spec(state, TCP_FIN_WAIT_2)
        || can_transition_spec(state, TCP_CLOSE_WAIT)
        || can_transition_spec(state, TCP_CLOSING)
        || can_transition_spec(state, TCP_TIME_WAIT)
        || can_transition_spec(state, TCP_LAST_ACK)
{}

// r[verify tcp-state.can-transition]
/// FIN_WAIT_1 has three possible successors (most of any state).
proof fn fin_wait_1_successors(to: u8)
    requires can_transition_spec(TCP_FIN_WAIT_1, to)
    ensures to == TCP_FIN_WAIT_2 || to == TCP_CLOSING || to == TCP_TIME_WAIT
{}

// r[verify tcp-state.can-transition]
/// CLOSE_WAIT has exactly one successor: LAST_ACK.
proof fn close_wait_single_successor(to: u8)
    requires can_transition_spec(TCP_CLOSE_WAIT, to)
    ensures to == TCP_LAST_ACK
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify tcp-state.is-valid]
    #[test]
    fn test_all_states_valid() {
        for s in 0u8..=10 {
            assert!(is_valid_state(s));
        }
        assert!(!is_valid_state(11));
        assert!(!is_valid_state(255));
    }

    // r[verify tcp-state.can-transition]
    #[test]
    fn test_all_18_transitions() {
        let transitions = [
            (TCP_CLOSED, TCP_LISTEN),
            (TCP_CLOSED, TCP_SYN_SENT),
            (TCP_LISTEN, TCP_SYN_SENT),
            (TCP_LISTEN, TCP_SYN_RECEIVED),
            (TCP_SYN_SENT, TCP_SYN_RECEIVED),
            (TCP_SYN_SENT, TCP_ESTABLISHED),
            (TCP_SYN_RECEIVED, TCP_ESTABLISHED),
            (TCP_SYN_RECEIVED, TCP_FIN_WAIT_1),
            (TCP_ESTABLISHED, TCP_FIN_WAIT_1),
            (TCP_ESTABLISHED, TCP_CLOSE_WAIT),
            (TCP_FIN_WAIT_1, TCP_FIN_WAIT_2),
            (TCP_FIN_WAIT_1, TCP_CLOSING),
            (TCP_FIN_WAIT_1, TCP_TIME_WAIT),
            (TCP_FIN_WAIT_2, TCP_TIME_WAIT),
            (TCP_CLOSE_WAIT, TCP_LAST_ACK),
            (TCP_CLOSING, TCP_TIME_WAIT),
            (TCP_LAST_ACK, TCP_CLOSED),
            (TCP_TIME_WAIT, TCP_CLOSED),
        ];
        for (from, to) in transitions {
            assert!(can_transition(from, to), "expected {from}→{to}");
        }
    }

    // r[verify tcp-state.can-transition]
    #[test]
    fn test_no_self_transitions() {
        for s in 0u8..=10 {
            assert!(!can_transition(s, s));
        }
    }

    // r[verify tcp-state.can-transition]
    #[test]
    fn test_transition_count() {
        // Exactly 18 valid transitions among the 11x11 pairs.
        let mut count = 0u32;
        for from in 0u8..=10 {
            for to in 0u8..=10 {
                if can_transition(from, to) {
                    count += 1;
                }
            }
        }
        assert_eq!(count, 18);
    }

    // r[verify tcp-state.is-data-state]
    #[test]
    fn test_data_states() {
        assert!(is_data_state(TCP_ESTABLISHED));
        assert!(is_data_state(TCP_CLOSE_WAIT));
        for s in [
            TCP_CLOSED,
            TCP_LISTEN,
            TCP_SYN_SENT,
            TCP_SYN_RECEIVED,
            TCP_FIN_WAIT_1,
            TCP_FIN_WAIT_2,
            TCP_CLOSING,
            TCP_LAST_ACK,
            TCP_TIME_WAIT,
        ] {
            assert!(!is_data_state(s));
        }
    }

    // r[verify tcp-state.is-shutdown]
    #[test]
    fn test_shutdown_states() {
        let shutdown = [
            TCP_FIN_WAIT_1,
            TCP_FIN_WAIT_2,
            TCP_CLOSING,
            TCP_TIME_WAIT,
            TCP_LAST_ACK,
            TCP_CLOSE_WAIT,
        ];
        for s in shutdown {
            assert!(is_shutdown(s));
        }
        for s in [TCP_CLOSED, TCP_LISTEN, TCP_SYN_SENT, TCP_SYN_RECEIVED, TCP_ESTABLISHED] {
            assert!(!is_shutdown(s));
        }
    }

    // r[verify tcp-state.is-connected]
    #[test]
    fn test_connected_states() {
        assert!(!is_connected(TCP_CLOSED));
        assert!(!is_connected(TCP_LISTEN));
        assert!(!is_connected(TCP_SYN_SENT));
        for s in [
            TCP_SYN_RECEIVED,
            TCP_ESTABLISHED,
            TCP_FIN_WAIT_1,
            TCP_FIN_WAIT_2,
            TCP_CLOSE_WAIT,
            TCP_CLOSING,
            TCP_LAST_ACK,
            TCP_TIME_WAIT,
        ] {
            assert!(is_connected(s));
        }
    }

    // r[verify tcp-state.established-reachable]
    #[test]
    fn test_active_open_handshake() {
        assert!(can_transition(TCP_CLOSED, TCP_SYN_SENT));
        assert!(can_transition(TCP_SYN_SENT, TCP_ESTABLISHED));
    }

    // r[verify tcp-state.established-reachable]
    #[test]
    fn test_passive_open_handshake() {
        assert!(can_transition(TCP_CLOSED, TCP_LISTEN));
        assert!(can_transition(TCP_LISTEN, TCP_SYN_RECEIVED));
        assert!(can_transition(TCP_SYN_RECEIVED, TCP_ESTABLISHED));
    }

    // r[verify tcp-state.closed-terminal]
    #[test]
    fn test_closed_predecessors() {
        // Only LAST_ACK and TIME_WAIT can reach CLOSED
        for from in 0u8..=10 {
            if can_transition(from, TCP_CLOSED) {
                assert!(from == TCP_LAST_ACK || from == TCP_TIME_WAIT);
            }
        }
    }

    proptest! {
        // r[verify tcp-state.can-transition]
        #[test]
        fn prop_no_self_transition(state: u8) {
            prop_assert!(!can_transition(state, state));
        }

        // r[verify tcp-state.can-transition]
        #[test]
        fn prop_transition_preserves_validity(from: u8, to: u8) {
            if can_transition(from, to) {
                prop_assert!(is_valid_state(from));
                prop_assert!(is_valid_state(to));
            }
        }

        // r[verify tcp-state.is-data-state]
        // r[verify tcp-state.is-connected]
        #[test]
        fn prop_data_implies_connected(state: u8) {
            if is_data_state(state) {
                prop_assert!(is_connected(state));
            }
        }

        // r[verify tcp-state.is-shutdown]
        // r[verify tcp-state.is-connected]
        #[test]
        fn prop_shutdown_implies_connected(state: u8) {
            if is_shutdown(state) {
                prop_assert!(is_connected(state));
            }
        }

        // r[verify tcp-state.is-data-state]
        // r[verify tcp-state.is-shutdown]
        #[test]
        fn prop_data_and_shutdown_overlap(state: u8) {
            // CLOSE_WAIT is both data and shutdown.
            // No other state is both data and non-shutdown.
            if is_data_state(state) && !is_shutdown(state) {
                prop_assert_eq!(state, TCP_ESTABLISHED);
            }
        }

        // r[verify tcp-state.can-transition]
        #[test]
        fn prop_connected_has_successor(state in 0u8..=10) {
            if is_connected(state) {
                let has_next = (0u8..=10).any(|to| can_transition(state, to));
                prop_assert!(has_next);
            }
        }

        // r[verify tcp-state.can-transition]
        /// Walk a random path through the state machine.
        /// At each step pick a random successor; if none, stop.
        #[test]
        fn prop_random_walk(choices in proptest::collection::vec(0usize..5, 20)) {
            let mut state = TCP_CLOSED;
            for choice in choices {
                let succs: Vec<u8> = (0u8..=10)
                    .filter(|&to| can_transition(state, to))
                    .collect();
                if succs.is_empty() {
                    break;
                }
                state = succs[choice % succs.len()];
                prop_assert!(is_valid_state(state));
            }
        }

        // r[verify tcp-state.closed-terminal]
        #[test]
        fn prop_closed_predecessors(from: u8) {
            if can_transition(from, TCP_CLOSED) {
                prop_assert!(from == TCP_LAST_ACK || from == TCP_TIME_WAIT);
            }
        }
    }
}
