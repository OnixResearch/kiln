//! Verified follower redirect primitives for Raft.
//!
//! When a client sends a request to a follower, the follower must redirect
//! to the leader. This module covers the redirect decision, leader hint
//! construction, hint freshness validation, and leader acceptance predicate.
//! Complements `client_routing.rs` (write/read routing logic) with
//! explicit hint staleness tracking.

use vstd::prelude::*;

use crate::election::*;

verus! {

/// Sentinel for "leader unknown".
pub const LEADER_UNKNOWN: u64 = u64::MAX;

// --- should_redirect ---

// r[depends follower-redirect.should-redirect]
/// Spec: should this node redirect the client to the leader?
///
/// True when the node is NOT the leader AND a leader is known.
/// A follower with no known leader cannot redirect — it returns
/// an error instead.
pub open spec fn should_redirect_spec(
    state: u8,
    known_leader_id: u64,
) -> bool {
    state != STATE_LEADER && known_leader_id != LEADER_UNKNOWN
}

// r[impl follower-redirect.should-redirect]
/// Check if this node should redirect the client.
pub fn should_redirect(
    state: u8,
    known_leader_id: u64,
) -> (result: bool)
    ensures result == should_redirect_spec(state, known_leader_id)
{
    state != STATE_LEADER && known_leader_id != LEADER_UNKNOWN
}

// --- leader_hint ---

// r[depends follower-redirect.leader-hint]
/// Spec: get the leader hint to include in the redirect response.
///
/// Returns the known leader ID, or LEADER_UNKNOWN if no leader is known.
/// The hint is best-effort — it may be stale after a leader change.
pub open spec fn leader_hint_spec(known_leader_id: u64) -> u64 {
    known_leader_id
}

// r[impl follower-redirect.leader-hint]
/// Get the leader hint for redirect response.
pub fn leader_hint(known_leader_id: u64) -> (result: u64)
    ensures result == leader_hint_spec(known_leader_id)
{
    known_leader_id
}

// --- is_hint_fresh ---

// r[depends follower-redirect.hint-freshness]
/// Spec: is the leader hint still fresh?
///
/// A hint is fresh when it was learned in the current term. A hint
/// from a prior term is stale because a new leader may have been
/// elected. Clients receiving a stale hint should retry discovery.
pub open spec fn is_hint_fresh_spec(
    hint_term: u64,
    current_term: u64,
) -> bool {
    hint_term == current_term
}

// r[impl follower-redirect.hint-freshness]
/// Check if a leader hint is fresh.
pub fn is_hint_fresh(
    hint_term: u64,
    current_term: u64,
) -> (result: bool)
    ensures result == is_hint_fresh_spec(hint_term, current_term)
{
    hint_term == current_term
}

// --- should_accept_request ---

// r[depends follower-redirect.leader-accepts]
/// Spec: should this node accept and process the client request?
///
/// Only the leader processes client requests directly. Followers
/// and candidates redirect or reject.
pub open spec fn should_accept_request_spec(state: u8) -> bool {
    state == STATE_LEADER
}

// r[impl follower-redirect.leader-accepts]
/// Check if this node should accept the request.
pub fn should_accept_request(state: u8) -> (result: bool)
    ensures result == should_accept_request_spec(state)
{
    state == STATE_LEADER
}

// --- Safety proofs ---

// r[verify follower-redirect.should-redirect]
/// Leader never redirects.
proof fn leader_never_redirects(known_leader_id: u64)
    ensures !should_redirect_spec(STATE_LEADER, known_leader_id)
{}

// r[verify follower-redirect.should-redirect]
/// Follower with unknown leader does not redirect.
proof fn unknown_leader_no_redirect(state: u8)
    ensures !should_redirect_spec(state, LEADER_UNKNOWN)
{}

// r[verify follower-redirect.should-redirect]
/// Follower with known leader redirects.
proof fn follower_with_leader_redirects(leader_id: u64)
    requires leader_id != LEADER_UNKNOWN
    ensures should_redirect_spec(STATE_FOLLOWER, leader_id)
{}

// r[verify follower-redirect.hint-freshness]
/// Prior-term hint is stale.
proof fn prior_term_stale(hint_term: u64, current_term: u64)
    requires hint_term < current_term
    ensures !is_hint_fresh_spec(hint_term, current_term)
{}

// r[verify follower-redirect.hint-freshness]
/// Same-term hint is fresh.
proof fn same_term_fresh(term: u64)
    ensures is_hint_fresh_spec(term, term)
{}

// r[verify follower-redirect.leader-accepts]
/// Only leader accepts.
proof fn follower_does_not_accept()
    ensures !should_accept_request_spec(STATE_FOLLOWER)
{}

// r[verify follower-redirect.leader-accepts]
/// Candidate does not accept.
proof fn candidate_does_not_accept()
    ensures !should_accept_request_spec(STATE_TRANSITIONING)
{}

// r[verify follower-redirect.should-redirect]
// r[verify follower-redirect.leader-accepts]
/// Accept and redirect are mutually exclusive (when leader is known).
proof fn accept_redirect_exclusive(state: u8, leader_id: u64)
    requires leader_id != LEADER_UNKNOWN
    ensures
        !(should_accept_request_spec(state) && should_redirect_spec(state, leader_id))
{}

// r[verify follower-redirect.should-redirect]
// r[verify follower-redirect.leader-accepts]
/// A node either accepts, redirects, or rejects (leader unknown + not leader).
proof fn exhaustive_routing(state: u8, leader_id: u64)
    ensures
        should_accept_request_spec(state)
        || should_redirect_spec(state, leader_id)
        || (state != STATE_LEADER && leader_id == LEADER_UNKNOWN)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify follower-redirect.should-redirect]
    #[test]
    fn test_should_redirect() {
        // Follower with known leader
        assert!(should_redirect(0, 42)); // STATE_FOLLOWER=0
        // Candidate with known leader
        assert!(should_redirect(1, 42)); // STATE_TRANSITIONING=1
        // Leader never redirects
        assert!(!should_redirect(2, 42)); // STATE_LEADER=2
        // Unknown leader
        assert!(!should_redirect(0, LEADER_UNKNOWN));
    }

    // r[verify follower-redirect.leader-hint]
    #[test]
    fn test_leader_hint() {
        assert_eq!(leader_hint(42), 42);
        assert_eq!(leader_hint(LEADER_UNKNOWN), LEADER_UNKNOWN);
    }

    // r[verify follower-redirect.hint-freshness]
    #[test]
    fn test_is_hint_fresh() {
        assert!(is_hint_fresh(3, 3));
        assert!(!is_hint_fresh(2, 3));
        assert!(!is_hint_fresh(4, 3));
    }

    // r[verify follower-redirect.leader-accepts]
    #[test]
    fn test_should_accept_request() {
        assert!(should_accept_request(2)); // Leader
        assert!(!should_accept_request(0)); // Follower
        assert!(!should_accept_request(1)); // Candidate
    }

    proptest! {
        // r[verify follower-redirect.should-redirect]
        // r[verify follower-redirect.leader-accepts]
        #[test]
        fn prop_accept_redirect_exclusive(
            state in 0u8..3,
            leader_id in 0u64..100,
        ) {
            let accepts = should_accept_request(state);
            let redirects = should_redirect(state, leader_id);
            // Cannot both accept and redirect
            prop_assert!(!(accepts && redirects));
        }

        // r[verify follower-redirect.hint-freshness]
        #[test]
        fn prop_hint_freshness_is_equality(hint: u64, current: u64) {
            prop_assert_eq!(is_hint_fresh(hint, current), hint == current);
        }

        // r[verify follower-redirect.should-redirect]
        #[test]
        fn prop_leader_never_redirects(leader_id: u64) {
            prop_assert!(!should_redirect(2, leader_id)); // STATE_LEADER=2
        }

        #[test]
        fn prop_redirect_requires_known_leader(
            state in 0u8..3,
        ) {
            // LEADER_UNKNOWN sentinel means no redirect
            prop_assert!(!should_redirect(state, LEADER_UNKNOWN));
        }

        #[test]
        fn prop_leader_hint_identity(id: u64) {
            prop_assert_eq!(leader_hint(id), id);
        }

        #[test]
        fn prop_only_leader_accepts(
            state in 0u8..3,
        ) {
            let accepts = should_accept_request(state);
            prop_assert_eq!(accepts, state == 2); // STATE_LEADER=2
        }
    }
}
