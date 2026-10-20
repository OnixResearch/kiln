//! Verified request forwarding for Raft.
//!
//! When a follower receives a client write, it can forward the request
//! to the leader instead of redirecting the client. The follower tracks
//! pending forwards and correlates leader responses back to clients.
//!
//! Forwarding vs. redirect trade-offs:
//! - Forwarding: one network hop (follower→leader), client sees one connection
//! - Redirect: client must reconnect to leader, but load is off the follower
//!
//! The forwarding state tracks:
//! - pending_count: how many forwards are waiting for responses
//! - oldest_forward_time: when the oldest pending forward was sent
//! - max_pending: cap to prevent unbounded memory growth

use vstd::prelude::*;

verus! {

// --- can_forward ---

// r[depends request-forward.can-forward]
/// Spec: can a new request be forwarded?
///
/// Forwarding is allowed when:
/// 1. We know who the leader is (leader_known)
/// 2. We are a follower (not leader, not candidate)
/// 3. The pending queue is not full
pub open spec fn can_forward_spec(
    is_follower: bool,
    leader_known: bool,
    pending_count: u64,
    max_pending: u64,
) -> bool {
    is_follower && leader_known && pending_count < max_pending
}

// r[impl request-forward.can-forward]
/// Check if request forwarding is allowed.
pub fn can_forward(
    is_follower: bool,
    leader_known: bool,
    pending_count: u64,
    max_pending: u64,
) -> (result: bool)
    ensures result == can_forward_spec(is_follower, leader_known, pending_count, max_pending)
{
    is_follower && leader_known && pending_count < max_pending
}

// --- after_forward ---

// r[depends request-forward.after-forward]
/// Spec: pending count after forwarding a request.
pub open spec fn after_forward_spec(pending_count: u64) -> u64 {
    (pending_count + 1) as u64
}

// r[impl request-forward.after-forward]
/// Increment pending count after forwarding.
pub fn after_forward(pending_count: u64) -> (result: u64)
    requires pending_count < u64::MAX
    ensures result == after_forward_spec(pending_count)
{
    pending_count + 1
}

// --- after_response ---

// r[depends request-forward.after-response]
/// Spec: pending count after receiving a leader response.
pub open spec fn after_response_spec(pending_count: u64) -> u64 {
    if pending_count > 0 {
        (pending_count - 1) as u64
    } else {
        0u64
    }
}

// r[impl request-forward.after-response]
/// Decrement pending count after leader response.
pub fn after_response(pending_count: u64) -> (result: u64)
    ensures result == after_response_spec(pending_count)
{
    if pending_count > 0 {
        pending_count - 1
    } else {
        0
    }
}

// --- is_forward_expired ---

// r[depends request-forward.expired]
/// Spec: has a pending forward timed out?
///
/// If the oldest forward was sent before `now - timeout`, it has expired.
/// Expired forwards should be cleaned up and the client notified.
pub open spec fn is_forward_expired_spec(
    forward_time: u64,
    now: u64,
    timeout: u64,
) -> bool {
    if timeout > u64::MAX as u64 - forward_time {
        false
    } else {
        now >= forward_time + timeout
    }
}

// r[impl request-forward.expired]
/// Check if a forward has expired.
pub fn is_forward_expired(
    forward_time: u64,
    now: u64,
    timeout: u64,
) -> (result: bool)
    ensures result == is_forward_expired_spec(forward_time, now, timeout)
{
    if timeout > u64::MAX - forward_time {
        false
    } else {
        now >= forward_time + timeout
    }
}

// --- clear_on_leader_change ---

// r[depends request-forward.clear-on-leader-change]
/// Spec: should pending forwards be cleared?
///
/// When the leader changes (term advances), all pending forwards are
/// stale. The new leader won't have context for these requests.
pub open spec fn should_clear_pending_spec(
    old_leader_term: u64,
    new_term: u64,
) -> bool {
    new_term > old_leader_term
}

// r[impl request-forward.clear-on-leader-change]
/// Check if pending forwards should be cleared due to leader change.
pub fn should_clear_pending(
    old_leader_term: u64,
    new_term: u64,
) -> (result: bool)
    ensures result == should_clear_pending_spec(old_leader_term, new_term)
{
    new_term > old_leader_term
}

// --- forward_decision ---

// r[depends request-forward.decision]
/// Spec: decide what to do with a client write request.
///
/// 0 = Accept (leader processes it)
/// 1 = Forward (follower sends to leader)
/// 2 = Reject (can't forward — no leader or queue full)
pub open spec fn forward_decision_spec(
    is_leader: bool,
    is_follower: bool,
    leader_known: bool,
    pending_count: u64,
    max_pending: u64,
) -> u8 {
    if is_leader {
        0u8  // Accept
    } else if can_forward_spec(is_follower, leader_known, pending_count, max_pending) {
        1u8  // Forward
    } else {
        2u8  // Reject
    }
}

/// Accept: leader processes the write.
pub const DECISION_ACCEPT: u8 = 0;
/// Forward: follower sends to leader.
pub const DECISION_FORWARD: u8 = 1;
/// Reject: can't process or forward.
pub const DECISION_REJECT: u8 = 2;

// r[impl request-forward.decision]
/// Decide how to handle a client write.
pub fn forward_decision(
    is_leader: bool,
    is_follower: bool,
    leader_known: bool,
    pending_count: u64,
    max_pending: u64,
) -> (result: u8)
    ensures result == forward_decision_spec(is_leader, is_follower, leader_known, pending_count, max_pending)
{
    if is_leader {
        DECISION_ACCEPT
    } else if can_forward(is_follower, leader_known, pending_count, max_pending) {
        DECISION_FORWARD
    } else {
        DECISION_REJECT
    }
}

// --- Safety proofs ---

// r[verify request-forward.can-forward]
/// Leaders never forward.
proof fn leader_never_forwards(
    leader_known: bool,
    pending_count: u64,
    max_pending: u64,
)
    ensures !can_forward_spec(false, leader_known, pending_count, max_pending)
{}

// r[verify request-forward.after-forward]
/// Forward increases pending count by one.
proof fn forward_increments(pending_count: u64)
    requires pending_count < u64::MAX
    ensures after_forward_spec(pending_count) == pending_count + 1
{}

// r[verify request-forward.after-response]
/// Response decreases pending count by one (when positive).
proof fn response_decrements(pending_count: u64)
    requires pending_count > 0
    ensures after_response_spec(pending_count) == pending_count - 1
{}

// r[verify request-forward.after-forward]
/// Forward then response is a roundtrip.
proof fn forward_response_roundtrip(pending_count: u64)
    requires pending_count < u64::MAX
    ensures after_response_spec(after_forward_spec(pending_count)) == pending_count
{}

// r[verify request-forward.after-response]
/// Response then forward is a roundtrip (when pending > 0).
proof fn response_forward_roundtrip(pending_count: u64)
    requires
        pending_count > 0,
        pending_count - 1 < u64::MAX,
    ensures after_forward_spec(after_response_spec(pending_count)) == pending_count
{}

// r[verify request-forward.decision]
/// Leaders always accept.
proof fn leader_accepts(
    is_follower: bool,
    leader_known: bool,
    pending_count: u64,
    max_pending: u64,
)
    ensures forward_decision_spec(true, is_follower, leader_known, pending_count, max_pending) == DECISION_ACCEPT
{}

// r[verify request-forward.decision]
/// Decision is always valid (0, 1, or 2).
proof fn decision_valid(
    is_leader: bool,
    is_follower: bool,
    leader_known: bool,
    pending_count: u64,
    max_pending: u64,
)
    ensures ({
        let d = forward_decision_spec(is_leader, is_follower, leader_known, pending_count, max_pending);
        d == DECISION_ACCEPT || d == DECISION_FORWARD || d == DECISION_REJECT
    })
{}

// r[verify request-forward.clear-on-leader-change]
/// Same term does not clear.
proof fn same_term_no_clear(term: u64)
    ensures !should_clear_pending_spec(term, term)
{}

// r[verify request-forward.clear-on-leader-change]
/// Higher term always clears.
proof fn higher_term_clears(old_term: u64, new_term: u64)
    requires new_term > old_term
    ensures should_clear_pending_spec(old_term, new_term)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify request-forward.can-forward]
    #[test]
    fn test_can_forward() {
        assert!(can_forward(true, true, 5, 10));
        assert!(!can_forward(false, true, 5, 10)); // not follower
        assert!(!can_forward(true, false, 5, 10)); // no leader
        assert!(!can_forward(true, true, 10, 10)); // queue full
    }

    // r[verify request-forward.after-forward]
    #[test]
    fn test_after_forward() {
        assert_eq!(after_forward(0), 1);
        assert_eq!(after_forward(5), 6);
    }

    // r[verify request-forward.after-response]
    #[test]
    fn test_after_response() {
        assert_eq!(after_response(5), 4);
        assert_eq!(after_response(1), 0);
        assert_eq!(after_response(0), 0); // floor at 0
    }

    // r[verify request-forward.expired]
    #[test]
    fn test_is_forward_expired() {
        assert!(!is_forward_expired(100, 100, 50));
        assert!(is_forward_expired(100, 150, 50));
        assert!(is_forward_expired(100, 200, 50));
    }

    // r[verify request-forward.decision]
    #[test]
    fn test_forward_decision() {
        assert_eq!(forward_decision(true, false, true, 0, 10), DECISION_ACCEPT);
        assert_eq!(forward_decision(false, true, true, 0, 10), DECISION_FORWARD);
        assert_eq!(forward_decision(false, true, false, 0, 10), DECISION_REJECT);
        assert_eq!(forward_decision(false, false, true, 0, 10), DECISION_REJECT);
    }

    // r[verify request-forward.clear-on-leader-change]
    #[test]
    fn test_should_clear_pending() {
        assert!(should_clear_pending(3, 5));
        assert!(!should_clear_pending(5, 5));
        assert!(!should_clear_pending(5, 3));
    }

    proptest! {
        // r[verify request-forward.after-forward]
        #[test]
        fn prop_forward_response_roundtrip(pending in 0u64..u64::MAX - 1) {
            prop_assert_eq!(after_response(after_forward(pending)), pending);
        }

        // r[verify request-forward.decision]
        #[test]
        fn prop_decision_valid(
            is_leader: bool,
            is_follower: bool,
            leader_known: bool,
            pending in 0u64..100,
            max_pending in 0u64..100,
        ) {
            let d = forward_decision(is_leader, is_follower, leader_known, pending, max_pending);
            prop_assert!(d <= 2);
        }

        // r[verify request-forward.decision]
        #[test]
        fn prop_leader_always_accepts(
            is_follower: bool,
            leader_known: bool,
            pending: u64,
            max_pending: u64,
        ) {
            prop_assert_eq!(forward_decision(true, is_follower, leader_known, pending, max_pending), DECISION_ACCEPT);
        }

        #[test]
        fn prop_forward_then_response_identity(
            initial in 0u64..1000,
            forwards in 1usize..=5,
        ) {
            let mut pending = initial;
            for _ in 0..forwards {
                if pending < u64::MAX {
                    pending = after_forward(pending);
                }
            }
            for _ in 0..forwards {
                pending = after_response(pending);
            }
            prop_assert_eq!(pending, initial);
        }

        #[test]
        fn prop_can_forward_requires_capacity(
            pending in 0u64..100,
            max in 0u64..100,
        ) {
            let cf = can_forward(true, true, pending, max);
            if cf {
                prop_assert!(pending < max);
            }
        }

        #[test]
        fn prop_clear_on_term_change(
            old_term in 0u64..100,
            new_term in 0u64..100,
        ) {
            let clear = should_clear_pending(old_term, new_term);
            prop_assert_eq!(clear, old_term < new_term);
        }
    }
}
