//! Verified circuit breaker state machine.
//!
//! Three states: CLOSED (normal), OPEN (blocking), `HALF_OPEN` (probing).
//! Transitions are deterministic functions of failure counts, elapsed time,
//! and probe outcomes. Encoded as u8 constants because Verus can't match
//! on enum variants in spec functions.

#![allow(clippy::nonminimal_bool)]
#![allow(clippy::bool_to_int_with_if)]

use vstd::prelude::*;

verus! {

/// Circuit breaker states.
pub const CLOSED: u8 = 0;
/// Circuit breaker open state - failures exceed threshold.
pub const OPEN: u8 = 1;
/// Circuit breaker half-open state - testing for recovery.
pub const HALF_OPEN: u8 = 2;

// --- is_valid_state ---

// r[depends circuit-breaker.is-valid-state]
/// Spec: state is one of the three valid values.
pub open spec fn is_valid_state_spec(state: u8) -> bool {
    state == CLOSED || state == OPEN || state == HALF_OPEN
}

// r[impl circuit-breaker.is-valid-state]
/// Returns true if the state value is a valid circuit breaker state.
pub fn is_valid_state(state: u8) -> (result: bool)
    ensures result == is_valid_state_spec(state)
{
    state == CLOSED || state == OPEN || state == HALF_OPEN
}

// --- next_state ---

// r[depends circuit-breaker.next-state]
/// Spec: compute the next state given current conditions.
///
/// Transitions:
/// - CLOSED + failures >= threshold → OPEN
/// - OPEN + elapsed >= timeout → HALF_OPEN
/// - HALF_OPEN + probe succeeded → CLOSED
/// - HALF_OPEN + probe failed → OPEN
/// - Otherwise: stay in current state
pub open spec fn next_state_spec(
    current: u8,
    failure_count: u64,
    threshold: u64,
    elapsed: u64,
    timeout: u64,
    probe_succeeded: bool,
) -> u8
    recommends is_valid_state_spec(current), threshold > 0, timeout > 0
{
    if current == CLOSED && failure_count >= threshold {
        OPEN
    } else if current == OPEN && elapsed >= timeout {
        HALF_OPEN
    } else if current == HALF_OPEN && probe_succeeded {
        CLOSED
    } else if current == HALF_OPEN && !probe_succeeded {
        OPEN
    } else {
        current
    }
}

// r[impl circuit-breaker.next-state]
/// Computes the next circuit breaker state.
pub fn next_state(
    current: u8,
    failure_count: u64,
    threshold: u64,
    elapsed: u64,
    timeout: u64,
    probe_succeeded: bool,
) -> (result: u8)
    requires is_valid_state_spec(current), threshold > 0, timeout > 0
    ensures result == next_state_spec(current, failure_count, threshold, elapsed, timeout, probe_succeeded)
{
    if current == CLOSED && failure_count >= threshold {
        OPEN
    } else if current == OPEN && elapsed >= timeout {
        HALF_OPEN
    } else if current == HALF_OPEN && probe_succeeded {
        CLOSED
    } else if current == HALF_OPEN && !probe_succeeded {
        OPEN
    } else {
        current
    }
}

// --- allow_request ---

// r[depends circuit-breaker.allow-request]
/// Spec: should we allow a request through?
/// - CLOSED: always allow
/// - OPEN: never allow
/// - HALF_OPEN: allow one probe if none active
pub open spec fn allow_request_spec(state: u8, probe_active: bool) -> bool
    recommends is_valid_state_spec(state)
{
    if state == CLOSED {
        true
    } else if state == OPEN {
        false
    } else {
        // HALF_OPEN: allow if no probe is already running
        !probe_active
    }
}

// r[impl circuit-breaker.allow-request]
/// Returns true if a request should be allowed through the breaker.
pub fn allow_request(state: u8, probe_active: bool) -> (result: bool)
    requires is_valid_state_spec(state)
    ensures result == allow_request_spec(state, probe_active)
{
    if state == CLOSED {
        true
    } else if state == OPEN {
        false
    } else {
        !probe_active
    }
}

// --- Safety proofs ---

// r[verify circuit-breaker.next-state]
/// Output is always a valid state.
proof fn next_state_valid(
    current: u8, failure_count: u64, threshold: u64,
    elapsed: u64, timeout: u64, probe_succeeded: bool,
)
    requires is_valid_state_spec(current), threshold > 0, timeout > 0
    ensures is_valid_state_spec(
        next_state_spec(current, failure_count, threshold, elapsed, timeout, probe_succeeded)
    )
{}

// r[verify circuit-breaker.allow-request]
/// OPEN always blocks.
proof fn open_blocks_all(probe_active: bool)
    ensures !allow_request_spec(OPEN, probe_active)
{}

// r[verify circuit-breaker.allow-request]
/// CLOSED always allows.
proof fn closed_allows_all(probe_active: bool)
    ensures allow_request_spec(CLOSED, probe_active)
{}

// r[verify circuit-breaker.allow-request]
/// HALF_OPEN admits one probe when none is active.
proof fn half_open_admits_probe()
    ensures allow_request_spec(HALF_OPEN, false)
{}

// r[verify circuit-breaker.allow-request]
/// HALF_OPEN blocks when a probe is already active.
proof fn half_open_blocks_during_probe()
    ensures !allow_request_spec(HALF_OPEN, true)
{}

// r[verify circuit-breaker.next-state]
/// Transition legality: CLOSED can only go to OPEN or stay CLOSED.
proof fn closed_transitions(
    failure_count: u64, threshold: u64, elapsed: u64, timeout: u64, probe_succeeded: bool,
)
    requires threshold > 0, timeout > 0
    ensures
        next_state_spec(CLOSED, failure_count, threshold, elapsed, timeout, probe_succeeded) == CLOSED
        || next_state_spec(CLOSED, failure_count, threshold, elapsed, timeout, probe_succeeded) == OPEN
{}

// r[verify circuit-breaker.next-state]
/// OPEN can only go to HALF_OPEN or stay OPEN.
proof fn open_transitions(
    failure_count: u64, threshold: u64, elapsed: u64, timeout: u64, probe_succeeded: bool,
)
    requires threshold > 0, timeout > 0
    ensures
        next_state_spec(OPEN, failure_count, threshold, elapsed, timeout, probe_succeeded) == OPEN
        || next_state_spec(OPEN, failure_count, threshold, elapsed, timeout, probe_succeeded) == HALF_OPEN
{}

// r[verify circuit-breaker.next-state]
/// HALF_OPEN goes to CLOSED or OPEN (never stays).
proof fn half_open_transitions(
    failure_count: u64, threshold: u64, elapsed: u64, timeout: u64, probe_succeeded: bool,
)
    requires threshold > 0, timeout > 0
    ensures
        next_state_spec(HALF_OPEN, failure_count, threshold, elapsed, timeout, probe_succeeded) == CLOSED
        || next_state_spec(HALF_OPEN, failure_count, threshold, elapsed, timeout, probe_succeeded) == OPEN
{}

// r[verify circuit-breaker.next-state]
/// Below threshold, CLOSED stays CLOSED.
proof fn below_threshold_stays_closed(
    failure_count: u64, threshold: u64, elapsed: u64, timeout: u64, probe_succeeded: bool,
)
    requires threshold > 0, timeout > 0, failure_count < threshold
    ensures next_state_spec(CLOSED, failure_count, threshold, elapsed, timeout, probe_succeeded) == CLOSED
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify circuit-breaker.next-state]
    #[test]
    fn test_next_state_transitions() {
        // CLOSED → OPEN on threshold breach
        assert_eq!(next_state(CLOSED, 5, 5, 0, 10, false), OPEN);
        // CLOSED stays CLOSED below threshold
        assert_eq!(next_state(CLOSED, 3, 5, 0, 10, false), CLOSED);
        // OPEN → HALF_OPEN after timeout
        assert_eq!(next_state(OPEN, 0, 5, 10, 10, false), HALF_OPEN);
        // OPEN stays OPEN before timeout
        assert_eq!(next_state(OPEN, 0, 5, 5, 10, false), OPEN);
        // HALF_OPEN → CLOSED on probe success
        assert_eq!(next_state(HALF_OPEN, 0, 5, 0, 10, true), CLOSED);
        // HALF_OPEN → OPEN on probe failure
        assert_eq!(next_state(HALF_OPEN, 0, 5, 0, 10, false), OPEN);
    }

    // r[verify circuit-breaker.allow-request]
    #[test]
    fn test_allow_request() {
        assert!(allow_request(CLOSED, false));
        assert!(allow_request(CLOSED, true));
        assert!(!allow_request(OPEN, false));
        assert!(!allow_request(OPEN, true));
        assert!(allow_request(HALF_OPEN, false));
        assert!(!allow_request(HALF_OPEN, true));
    }

    // r[verify circuit-breaker.is-valid-state]
    #[test]
    fn test_valid_state() {
        assert!(is_valid_state(CLOSED));
        assert!(is_valid_state(OPEN));
        assert!(is_valid_state(HALF_OPEN));
        assert!(!is_valid_state(3));
        assert!(!is_valid_state(255));
    }

    proptest! {
        // r[verify circuit-breaker.next-state]
        #[test]
        fn prop_next_state_always_valid(
            current in 0u8..=2,
            failure_count: u64,
            threshold in 1u64..=1000,
            elapsed: u64,
            timeout in 1u64..=1000,
            probe_succeeded: bool,
        ) {
            let ns = next_state(current, failure_count, threshold, elapsed, timeout, probe_succeeded);
            prop_assert!(is_valid_state(ns));
        }

        // r[verify circuit-breaker.next-state]
        #[test]
        fn prop_closed_stays_below_threshold(
            failure_count in 0u64..100,
            threshold in 100u64..200,
            elapsed: u64,
            timeout in 1u64..=1000,
            probe_succeeded: bool,
        ) {
            // failure_count < threshold guaranteed by ranges
            let ns = next_state(CLOSED, failure_count, threshold, elapsed, timeout, probe_succeeded);
            prop_assert_eq!(ns, CLOSED);
        }

        // r[verify circuit-breaker.allow-request]
        #[test]
        fn prop_open_always_blocks(probe_active: bool) {
            prop_assert!(!allow_request(OPEN, probe_active));
        }

        // r[verify circuit-breaker.allow-request]
        #[test]
        fn prop_closed_always_allows(probe_active: bool) {
            prop_assert!(allow_request(CLOSED, probe_active));
        }

        // r[verify circuit-breaker.next-state]
        /// Walk a random sequence of transitions and verify the state
        /// machine never produces an invalid state.
        #[test]
        fn prop_state_machine_walk(
            failures in proptest::collection::vec(0u64..20, 10),
            elapseds in proptest::collection::vec(0u64..100, 10),
            probes in proptest::collection::vec(any::<bool>(), 10),
        ) {
            let mut state = CLOSED;
            let threshold = 5u64;
            let timeout = 30u64;
            for i in 0..10 {
                state = next_state(state, failures[i], threshold, elapseds[i], timeout, probes[i]);
                prop_assert!(is_valid_state(state));
            }
        }
    }
}
