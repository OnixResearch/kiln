//! Verified election timer reset conditions for Raft.
//!
//! The election timer determines when a follower starts an election.
//! Three events reset the timer: receiving a valid AppendEntries from
//! the current leader, granting a vote to a candidate, and starting
//! an election as a candidate. Stale-term messages do NOT reset it.

use vstd::prelude::*;

use crate::election::*;

verus! {

// --- should_reset_on_append_entries ---

// r[depends election-timer.reset-on-append-entries]
/// Spec: should the follower reset its election timer on this AppendEntries?
///
/// True when msg_term >= current_term (from a current or newer leader)
/// AND the AppendEntries was processed successfully (log consistency
/// check passed). Stale-term messages must not reset the timer.
pub open spec fn should_reset_on_append_entries_spec(
    msg_term: u64,
    current_term: u64,
    is_success: bool,
) -> bool {
    msg_term >= current_term && is_success
}

// r[impl election-timer.reset-on-append-entries]
/// Check whether this AppendEntries should reset the election timer.
pub fn should_reset_on_append_entries(
    msg_term: u64,
    current_term: u64,
    is_success: bool,
) -> (result: bool)
    ensures result == should_reset_on_append_entries_spec(msg_term, current_term, is_success)
{
    msg_term >= current_term && is_success
}

// --- should_reset_on_vote_grant ---

// r[depends election-timer.reset-on-vote-grant]
/// Spec: should the follower reset its election timer after granting a vote?
///
/// True when the vote was actually granted. If the vote was denied,
/// no reset occurs.
pub open spec fn should_reset_on_vote_grant_spec(voted: bool) -> bool {
    voted
}

// r[impl election-timer.reset-on-vote-grant]
/// Check whether granting a vote should reset the election timer.
pub fn should_reset_on_vote_grant(voted: bool) -> (result: bool)
    ensures result == should_reset_on_vote_grant_spec(voted)
{
    voted
}

// --- should_reset_on_election_start ---

// r[depends election-timer.reset-on-election-start]
/// Spec: should the timer reset when the node starts an election?
///
/// True when the node actually became a candidate. The timer resets
/// so the candidate waits a full timeout before retrying.
pub open spec fn should_reset_on_election_start_spec(became_candidate: bool) -> bool {
    became_candidate
}

// r[impl election-timer.reset-on-election-start]
/// Check whether starting an election should reset the timer.
pub fn should_reset_on_election_start(became_candidate: bool) -> (result: bool)
    ensures result == should_reset_on_election_start_spec(became_candidate)
{
    became_candidate
}

// --- is_election_timer_expired ---

// r[depends election-timer.expired]
/// Spec: has the election timer expired?
///
/// True when elapsed_ms >= election_timeout_ms. The timer fires when
/// the full timeout has elapsed without a reset event.
pub open spec fn is_election_timer_expired_spec(
    elapsed_ms: u64,
    election_timeout_ms: u64,
) -> bool {
    elapsed_ms >= election_timeout_ms
}

// r[impl election-timer.expired]
/// Check if the election timer has expired.
pub fn is_election_timer_expired(
    elapsed_ms: u64,
    election_timeout_ms: u64,
) -> (result: bool)
    ensures result == is_election_timer_expired_spec(elapsed_ms, election_timeout_ms)
{
    elapsed_ms >= election_timeout_ms
}

// --- should_start_election ---

// r[depends election-timer.should-start-election]
/// Spec: should this node start an election?
///
/// True when the node is a follower AND the election timer has expired.
/// Leaders and candidates (Transitioning) must not start elections via timer.
pub open spec fn should_start_election_spec(
    state: u8,
    timer_expired: bool,
) -> bool {
    state == STATE_FOLLOWER && timer_expired
}

// r[impl election-timer.should-start-election]
/// Check whether this node should start an election.
pub fn should_start_election(
    state: u8,
    timer_expired: bool,
) -> (result: bool)
    ensures result == should_start_election_spec(state, timer_expired)
{
    state == STATE_FOLLOWER && timer_expired
}

// --- Safety proofs ---

// r[verify election-timer.reset-on-append-entries]
/// Stale-term messages do not reset the timer.
proof fn stale_term_no_reset(msg_term: u64, current_term: u64, is_success: bool)
    requires msg_term < current_term
    ensures !should_reset_on_append_entries_spec(msg_term, current_term, is_success)
{}

// r[verify election-timer.reset-on-append-entries]
/// Failed AppendEntries (consistency check failed) does not reset the timer.
proof fn failed_append_no_reset(msg_term: u64, current_term: u64)
    ensures !should_reset_on_append_entries_spec(msg_term, current_term, false)
{}

// r[verify election-timer.should-start-election]
/// Leaders never start elections via timer.
proof fn leader_no_election(timer_expired: bool)
    ensures !should_start_election_spec(STATE_LEADER, timer_expired)
{}

// r[verify election-timer.should-start-election]
/// Candidates (transitioning) never start elections via timer.
proof fn candidate_no_election(timer_expired: bool)
    ensures !should_start_election_spec(STATE_TRANSITIONING, timer_expired)
{}

// r[verify election-timer.expired]
/// Timer is not expired immediately after a reset (elapsed = 0).
proof fn fresh_timer_not_expired(election_timeout_ms: u64)
    requires election_timeout_ms > 0
    ensures !is_election_timer_expired_spec(0, election_timeout_ms)
{}

// r[verify election-timer.expired]
/// Timer expires at exactly the timeout value.
proof fn timer_expires_at_boundary(timeout: u64)
    ensures is_election_timer_expired_spec(timeout, timeout)
{}

// r[verify election-timer.reset-on-vote-grant]
/// Denying a vote does not reset the timer.
proof fn vote_denied_no_reset()
    ensures !should_reset_on_vote_grant_spec(false)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify election-timer.reset-on-append-entries]
    #[test]
    fn test_reset_on_append_entries() {
        // Valid AppendEntries from current leader
        assert!(should_reset_on_append_entries(3, 3, true));
        // Higher-term leader (step-down already handled elsewhere)
        assert!(should_reset_on_append_entries(4, 3, true));
        // Stale term
        assert!(!should_reset_on_append_entries(2, 3, true));
        // Failed consistency check
        assert!(!should_reset_on_append_entries(3, 3, false));
    }

    // r[verify election-timer.reset-on-vote-grant]
    #[test]
    fn test_reset_on_vote_grant() {
        assert!(should_reset_on_vote_grant(true));
        assert!(!should_reset_on_vote_grant(false));
    }

    // r[verify election-timer.reset-on-election-start]
    #[test]
    fn test_reset_on_election_start() {
        assert!(should_reset_on_election_start(true));
        assert!(!should_reset_on_election_start(false));
    }

    // r[verify election-timer.expired]
    #[test]
    fn test_election_timer_expired() {
        assert!(!is_election_timer_expired(0, 150));
        assert!(!is_election_timer_expired(149, 150));
        assert!(is_election_timer_expired(150, 150));
        assert!(is_election_timer_expired(200, 150));
        // Zero timeout always expired
        assert!(is_election_timer_expired(0, 0));
    }

    // r[verify election-timer.should-start-election]
    #[test]
    fn test_should_start_election() {
        // Follower with expired timer
        assert!(should_start_election(0, true)); // STATE_FOLLOWER = 0
        // Follower with live timer
        assert!(!should_start_election(0, false));
        // Leader never starts election
        assert!(!should_start_election(2, true)); // STATE_LEADER = 2
        // Candidate never starts via timer
        assert!(!should_start_election(1, true)); // STATE_TRANSITIONING = 1
    }

    proptest! {
        // r[verify election-timer.reset-on-append-entries]
        #[test]
        fn prop_stale_never_resets(
            msg_term in 0u64..50,
            current_term in 51u64..100,
            success: bool,
        ) {
            // msg_term < current_term always
            prop_assert!(!should_reset_on_append_entries(msg_term, current_term, success));
        }

        // r[verify election-timer.expired]
        #[test]
        fn prop_timer_monotonic(
            elapsed1 in 0u64..1000,
            elapsed2 in 0u64..1000,
            timeout in 1u64..500,
        ) {
            // If timer expired at elapsed1, it stays expired at any elapsed2 >= elapsed1
            if elapsed1 <= elapsed2 && is_election_timer_expired(elapsed1, timeout) {
                prop_assert!(is_election_timer_expired(elapsed2, timeout));
            }
        }

        // r[verify election-timer.should-start-election]
        #[test]
        fn prop_only_follower_starts_election(
            state in 0u8..3,
            expired: bool,
        ) {
            let result = should_start_election(state, expired);
            if state != 0 { // not follower
                prop_assert!(!result);
            }
        }

        #[test]
        fn prop_reset_requires_current_or_higher_term(
            msg_term in 0u64..100,
            current_term in 0u64..100,
            success: bool,
        ) {
            let reset = should_reset_on_append_entries(msg_term, current_term, success);
            if reset {
                prop_assert!(msg_term >= current_term);
                prop_assert!(success);
            }
        }

        #[test]
        fn prop_expired_at_threshold(
            timeout in 1u64..10_000,
        ) {
            prop_assert!(!is_election_timer_expired(timeout - 1, timeout));
            prop_assert!(is_election_timer_expired(timeout, timeout));
        }

        #[test]
        fn prop_start_election_iff_follower_and_expired(
            state in 0u8..3,
            expired: bool,
        ) {
            let result = should_start_election(state, expired);
            let expected = state == 0 && expired; // STATE_FOLLOWER == 0
            prop_assert_eq!(result, expected);
        }
    }
}
