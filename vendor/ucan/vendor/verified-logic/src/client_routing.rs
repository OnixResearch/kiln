//! Verified client request routing for Raft.
//!
//! A non-leader node must not process client commands. Followers redirect
//! clients to the current leader. Candidates reject requests (no stable
//! leader yet). Leaders accept commands and append them to the log.
//! This module models the routing decision and the conditions under
//! which each role handles client requests.

use vstd::prelude::*;

use crate::election::*;

verus! {

/// Routing decision for a client request.
/// 0 = Accept (leader processes it)
/// 1 = Redirect (follower sends client to leader)
/// 2 = Reject (candidate or no known leader)
pub const ROUTE_ACCEPT: u8 = 0;
/// Redirect client to the known leader.
pub const ROUTE_REDIRECT: u8 = 1;
/// Reject the request (no known leader or candidate state).
pub const ROUTE_REJECT: u8 = 2;

// --- route_client_request ---

// r[depends client-routing.route]
/// Spec: determine how to handle a client request based on node state.
///
/// - Leader: accept the request.
/// - Follower with known leader: redirect to leader.
/// - Follower with no known leader: reject.
/// - Candidate (transitioning): reject.
pub open spec fn route_client_request_spec(
    state: u8,
    has_known_leader: bool,
) -> u8 {
    if state == STATE_LEADER {
        ROUTE_ACCEPT
    } else if state == STATE_FOLLOWER && has_known_leader {
        ROUTE_REDIRECT
    } else {
        ROUTE_REJECT
    }
}

// r[impl client-routing.route]
/// Route a client request.
pub fn route_client_request(
    state: u8,
    has_known_leader: bool,
) -> (result: u8)
    ensures result == route_client_request_spec(state, has_known_leader)
{
    if state == STATE_LEADER {
        ROUTE_ACCEPT
    } else if state == STATE_FOLLOWER && has_known_leader {
        ROUTE_REDIRECT
    } else {
        ROUTE_REJECT
    }
}

// --- should_accept ---

// r[depends client-routing.should-accept]
/// Spec: should this node accept client commands?
///
/// Only leaders accept commands. Non-leaders must never append
/// client entries to the log.
pub open spec fn should_accept_spec(state: u8) -> bool {
    state == STATE_LEADER
}

// r[impl client-routing.should-accept]
/// Check if this node should accept client commands.
pub fn should_accept(state: u8) -> (result: bool)
    ensures result == should_accept_spec(state)
{
    state == STATE_LEADER
}

// --- should_redirect ---

// r[depends client-routing.should-redirect]
/// Spec: should this node redirect the client to the leader?
pub open spec fn should_redirect_spec(
    state: u8,
    has_known_leader: bool,
) -> bool {
    state == STATE_FOLLOWER && has_known_leader
}

// r[impl client-routing.should-redirect]
/// Check if this node should redirect.
pub fn should_redirect(
    state: u8,
    has_known_leader: bool,
) -> (result: bool)
    ensures result == should_redirect_spec(state, has_known_leader)
{
    state == STATE_FOLLOWER && has_known_leader
}

// --- is_leader_stale ---

// r[depends client-routing.leader-stale]
/// Spec: is the known leader potentially stale?
///
/// The leader hint becomes stale when our term advances past the
/// term in which we learned the leader. Clients should be told
/// the leader may have changed.
pub open spec fn is_leader_stale_spec(
    known_leader_term: u64,
    current_term: u64,
) -> bool {
    known_leader_term < current_term
}

// r[impl client-routing.leader-stale]
/// Check if the known leader is stale.
pub fn is_leader_stale(
    known_leader_term: u64,
    current_term: u64,
) -> (result: bool)
    ensures result == is_leader_stale_spec(known_leader_term, current_term)
{
    known_leader_term < current_term
}

// --- route_read_request ---

// r[depends client-routing.route-read]
/// Spec: route a read request.
///
/// Linearizable reads require leader confirmation (read_index or lease).
/// Stale reads can be served by any node.
/// 0 = serve locally (leader with confirmation, or stale read)
/// 1 = redirect to leader
/// 2 = reject
pub open spec fn route_read_request_spec(
    state: u8,
    allow_stale_reads: bool,
    has_known_leader: bool,
) -> u8 {
    if state == STATE_LEADER {
        ROUTE_ACCEPT
    } else if allow_stale_reads {
        ROUTE_ACCEPT // Any node can serve stale reads
    } else if state == STATE_FOLLOWER && has_known_leader {
        ROUTE_REDIRECT
    } else {
        ROUTE_REJECT
    }
}

// r[impl client-routing.route-read]
/// Route a read request.
pub fn route_read_request(
    state: u8,
    allow_stale_reads: bool,
    has_known_leader: bool,
) -> (result: u8)
    ensures result == route_read_request_spec(state, allow_stale_reads, has_known_leader)
{
    if state == STATE_LEADER {
        ROUTE_ACCEPT
    } else if allow_stale_reads {
        ROUTE_ACCEPT
    } else if state == STATE_FOLLOWER && has_known_leader {
        ROUTE_REDIRECT
    } else {
        ROUTE_REJECT
    }
}

// --- Safety proofs ---

// r[verify client-routing.route]
/// Only leaders accept client requests.
proof fn only_leader_accepts(state: u8, has_known_leader: bool)
    requires route_client_request_spec(state, has_known_leader) == ROUTE_ACCEPT
    ensures state == STATE_LEADER
{}

// r[verify client-routing.route]
/// Followers with a known leader redirect.
proof fn follower_with_leader_redirects()
    ensures route_client_request_spec(STATE_FOLLOWER, true) == ROUTE_REDIRECT
{}

// r[verify client-routing.route]
/// Followers without a known leader reject.
proof fn follower_without_leader_rejects()
    ensures route_client_request_spec(STATE_FOLLOWER, false) == ROUTE_REJECT
{}

// r[verify client-routing.route]
/// Candidates reject all client requests.
proof fn candidate_rejects(has_known_leader: bool)
    ensures route_client_request_spec(STATE_TRANSITIONING, has_known_leader) == ROUTE_REJECT
{}

// r[verify client-routing.should-accept]
/// Follower does not accept.
proof fn follower_does_not_accept()
    ensures !should_accept_spec(STATE_FOLLOWER)
{}

// r[verify client-routing.should-accept]
/// Candidate does not accept.
proof fn candidate_does_not_accept()
    ensures !should_accept_spec(STATE_TRANSITIONING)
{}

// r[verify client-routing.route]
/// Routing is exhaustive: every result is one of the three.
proof fn route_exhaustive(state: u8, has_known_leader: bool)
    ensures ({
        let r = route_client_request_spec(state, has_known_leader);
        r == ROUTE_ACCEPT || r == ROUTE_REDIRECT || r == ROUTE_REJECT
    })
{}

// r[verify client-routing.leader-stale]
/// Same-term leader is not stale.
proof fn same_term_not_stale(term: u64)
    ensures !is_leader_stale_spec(term, term)
{}

// r[verify client-routing.leader-stale]
/// Higher current term makes leader stale.
proof fn higher_term_stale(leader_term: u64, current_term: u64)
    requires current_term > leader_term
    ensures is_leader_stale_spec(leader_term, current_term)
{}

// r[verify client-routing.route-read]
/// Leader always accepts reads.
proof fn leader_accepts_reads(allow_stale: bool, has_leader: bool)
    ensures route_read_request_spec(STATE_LEADER, allow_stale, has_leader) == ROUTE_ACCEPT
{}

// r[verify client-routing.route-read]
/// Stale reads accepted on any node.
proof fn stale_reads_anywhere(state: u8, has_leader: bool)
    requires state != STATE_LEADER
    ensures route_read_request_spec(state, true, has_leader) == ROUTE_ACCEPT
{}

// r[verify client-routing.route-read]
/// Non-stale reads on followers redirect to leader.
proof fn linearizable_reads_redirect()
    ensures route_read_request_spec(STATE_FOLLOWER, false, true) == ROUTE_REDIRECT
{}

// r[verify client-routing.should-accept]
// r[verify client-routing.route]
/// Accept ↔ should_accept agreement.
proof fn accept_agrees_with_should(state: u8, has_known_leader: bool)
    ensures
        (route_client_request_spec(state, has_known_leader) == ROUTE_ACCEPT)
        == should_accept_spec(state)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify client-routing.route]
    #[test]
    fn test_route_client_request() {
        assert_eq!(route_client_request(STATE_LEADER, true), ROUTE_ACCEPT);
        assert_eq!(route_client_request(STATE_LEADER, false), ROUTE_ACCEPT);
        assert_eq!(route_client_request(STATE_FOLLOWER, true), ROUTE_REDIRECT);
        assert_eq!(route_client_request(STATE_FOLLOWER, false), ROUTE_REJECT);
        assert_eq!(route_client_request(STATE_TRANSITIONING, true), ROUTE_REJECT);
        assert_eq!(route_client_request(STATE_TRANSITIONING, false), ROUTE_REJECT);
    }

    // r[verify client-routing.should-accept]
    #[test]
    fn test_should_accept() {
        assert!(should_accept(STATE_LEADER));
        assert!(!should_accept(STATE_FOLLOWER));
        assert!(!should_accept(STATE_TRANSITIONING));
    }

    // r[verify client-routing.should-redirect]
    #[test]
    fn test_should_redirect() {
        assert!(should_redirect(STATE_FOLLOWER, true));
        assert!(!should_redirect(STATE_FOLLOWER, false));
        assert!(!should_redirect(STATE_LEADER, true));
    }

    // r[verify client-routing.leader-stale]
    #[test]
    fn test_is_leader_stale() {
        assert!(!is_leader_stale(5, 5)); // same term
        assert!(is_leader_stale(3, 5)); // stale
        assert!(!is_leader_stale(5, 3)); // leader term higher (impossible but checks logic)
    }

    // r[verify client-routing.route-read]
    #[test]
    fn test_route_read_request() {
        // Leader always accepts
        assert_eq!(route_read_request(STATE_LEADER, false, false), ROUTE_ACCEPT);
        assert_eq!(route_read_request(STATE_LEADER, true, false), ROUTE_ACCEPT);

        // Stale reads accepted anywhere
        assert_eq!(route_read_request(STATE_FOLLOWER, true, false), ROUTE_ACCEPT);

        // Linearizable reads redirect
        assert_eq!(route_read_request(STATE_FOLLOWER, false, true), ROUTE_REDIRECT);

        // No leader, no stale: reject
        assert_eq!(route_read_request(STATE_FOLLOWER, false, false), ROUTE_REJECT);
    }

    proptest! {
        // r[verify client-routing.route]
        #[test]
        fn prop_only_leader_accepts(state: u8, has_leader: bool) {
            let route = route_client_request(state, has_leader);
            if route == ROUTE_ACCEPT {
                prop_assert_eq!(state, STATE_LEADER);
            }
        }

        // r[verify client-routing.route]
        #[test]
        fn prop_route_exhaustive(state: u8, has_leader: bool) {
            let route = route_client_request(state, has_leader);
            prop_assert!(route == ROUTE_ACCEPT || route == ROUTE_REDIRECT || route == ROUTE_REJECT);
        }

        // r[verify client-routing.leader-stale]
        #[test]
        fn prop_same_term_not_stale(term: u64) {
            prop_assert!(!is_leader_stale(term, term));
        }

        // r[verify client-routing.route-read]
        #[test]
        fn prop_stale_reads_always_accepted(state: u8, has_leader: bool) {
            let route = route_read_request(state, true, has_leader);
            prop_assert_eq!(route, ROUTE_ACCEPT);
        }

        // r[verify client-routing.should-accept]
        // r[verify client-routing.route]
        #[test]
        fn prop_accept_matches_should_accept(state: u8, has_leader: bool) {
            prop_assert_eq!(
                route_client_request(state, has_leader) == ROUTE_ACCEPT,
                should_accept(state),
            );
        }
    }
}
