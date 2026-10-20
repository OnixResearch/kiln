//! Verified candidate election initiation for Raft.
//!
//! When a follower's election timer fires, it starts an election:
//! increment currentTerm, vote for self, transition to candidate state,
//! and begin with one vote (self-vote). The self-vote must satisfy
//! the same rules as any other vote grant.

use vstd::prelude::*;

use crate::ballot::*;
use crate::election::*;

verus! {

// --- init_election_term ---

// r[depends election-init.new-term]
/// Spec: compute the candidate's new term.
///
/// Returns current_term + 1. Returns None (modeled as u64::MAX marker)
/// when current_term == u64::MAX (overflow protection).
pub open spec fn init_election_term_spec(current_term: u64) -> Option<u64> {
    if current_term < u64::MAX {
        Some((current_term + 1) as u64)
    } else {
        None
    }
}

// r[impl election-init.new-term]
/// Compute the new term for an election. Returns None on overflow.
pub fn init_election_term(current_term: u64) -> (result: Option<u64>)
    ensures result == init_election_term_spec(current_term)
{
    if current_term < u64::MAX {
        Some(current_term + 1)
    } else {
        None
    }
}

// --- init_self_vote ---

// r[depends election-init.self-vote]
/// Spec: construct the self-vote state.
///
/// The candidate votes for itself in the new term, producing a VoteState
/// with voted_term == new_term and voted_for == candidate_id.
pub open spec fn init_self_vote_spec(candidate_id: u64, new_term: u64) -> VoteState {
    VoteState {
        voted_term: new_term,
        voted_for: candidate_id,
    }
}

// r[impl election-init.self-vote]
/// Construct the self-vote. The candidate votes for itself in the new term.
pub fn init_self_vote(candidate_id: u64, new_term: u64) -> (result: VoteState)
    ensures result == init_self_vote_spec(candidate_id, new_term)
{
    VoteState {
        voted_term: new_term,
        voted_for: candidate_id,
    }
}

// --- init_vote_count ---

// r[depends election-init.initial-vote-count]
/// Spec: the initial vote count (self-vote = 1).
pub open spec fn init_vote_count_spec() -> u64 {
    1u64
}

// r[impl election-init.initial-vote-count]
/// Returns the initial vote count: 1 (the candidate's self-vote).
pub fn init_vote_count() -> (result: u64)
    ensures result == init_vote_count_spec()
{
    1
}

// --- init_election_state ---

// r[depends election-init.new-state]
/// Spec: the candidate's state after starting an election.
pub open spec fn init_election_state_spec() -> u8 {
    STATE_TRANSITIONING
}

// r[impl election-init.new-state]
/// Returns the state after starting an election: Transitioning (candidate).
pub fn init_election_state() -> (result: u8)
    ensures result == init_election_state_spec()
{
    STATE_TRANSITIONING
}

// --- Safety proofs ---

// r[verify election-init.new-term]
/// New term is strictly greater than current term.
proof fn new_term_is_greater(current_term: u64)
    requires current_term < u64::MAX
    ensures init_election_term_spec(current_term) == Some((current_term + 1) as u64)
{}

// r[verify election-init.new-term]
/// Overflow at u64::MAX returns None.
proof fn overflow_returns_none()
    ensures init_election_term_spec(u64::MAX) == None::<u64>
{}

// r[verify election-init.self-vote]
/// Self-vote has the correct term.
proof fn self_vote_term(candidate_id: u64, new_term: u64)
    ensures init_self_vote_spec(candidate_id, new_term).voted_term == new_term
{}

// r[verify election-init.self-vote]
/// Self-vote has the correct candidate.
proof fn self_vote_candidate(candidate_id: u64, new_term: u64)
    ensures init_self_vote_spec(candidate_id, new_term).voted_for == candidate_id
{}

// r[verify election-init.initial-vote-count]
/// Initial count is exactly 1.
proof fn initial_count_is_one()
    ensures init_vote_count_spec() == 1u64
{}

// r[verify election-init.new-state]
/// State after election start is Transitioning.
proof fn election_state_is_transitioning()
    ensures init_election_state_spec() == STATE_TRANSITIONING
{}

// r[verify election-init.new-state]
/// State after election start is a valid state.
proof fn election_state_valid()
    ensures is_valid_state_spec(init_election_state_spec())
{}

// r[impl election-init.self-vote-is-valid]
// r[verify election-init.self-vote-is-valid]
/// The self-vote is a valid vote: a node with no prior vote in the
/// new term can always vote for itself (its own log is up-to-date
/// with respect to itself).
proof fn self_vote_is_valid(
    candidate_id: u64,
    new_term: u64,
    my_last: crate::epoch::Epoch,
)
    requires new_term > 0
    ensures
        can_grant_vote_spec(
            VoteState { voted_term: 0, voted_for: 0 },
            candidate_id,
            new_term,
            my_last,
            my_last,
        )
{
    // term_ok: new_term > 0 == voted_term ✓
    // log_ok: epoch_ge(my_last, my_last) ✓ (reflexive)
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify election-init.new-term]
    #[test]
    fn test_init_election_term() {
        assert_eq!(init_election_term(0), Some(1));
        assert_eq!(init_election_term(5), Some(6));
        assert_eq!(init_election_term(u64::MAX - 1), Some(u64::MAX));
        assert_eq!(init_election_term(u64::MAX), None);
    }

    // r[verify election-init.self-vote]
    #[test]
    fn test_init_self_vote() {
        let vote = init_self_vote(42, 5);
        assert_eq!(vote.voted_term, 5);
        assert_eq!(vote.voted_for, 42);
    }

    // r[verify election-init.initial-vote-count]
    #[test]
    fn test_init_vote_count() {
        assert_eq!(init_vote_count(), 1);
    }

    // r[verify election-init.new-state]
    #[test]
    fn test_init_election_state() {
        assert_eq!(init_election_state(), 1); // STATE_TRANSITIONING
    }

    // r[verify election-init.self-vote-is-valid]
    #[test]
    fn test_self_vote_valid() {
        let vote_state = init_self_vote(7, 3);
        // Re-voting for same candidate in same term is allowed
        let my_epoch = crate::epoch::Epoch { term: 2, index: 10 };
        assert!(crate::ballot::can_grant_vote(&vote_state, 7, 3, &my_epoch, &my_epoch));
    }

    proptest! {
        // r[verify election-init.new-term]
        #[test]
        fn prop_new_term_strictly_greater(current in 0u64..u64::MAX) {
            let result = init_election_term(current);
            if let Some(new_term) = result {
                prop_assert!(new_term > current);
                prop_assert_eq!(new_term, current + 1);
            }
        }

        // r[verify election-init.self-vote]
        #[test]
        fn prop_max_term_returns_none(dummy in 0u8..1) {
            let _ = dummy;
            prop_assert!(init_election_term(u64::MAX).is_none());
        }

        #[test]
        fn prop_init_state_is_transitioning(dummy in 0u8..1) {
            let _ = dummy;
            // STATE_TRANSITIONING = 1
            prop_assert_eq!(init_election_state(), 1);
        }

        #[test]
        fn prop_init_vote_count_is_one(dummy in 0u8..1) {
            let _ = dummy;
            prop_assert_eq!(init_vote_count(), 1);
        }

        #[test]
        fn prop_self_vote_fields(
            candidate_id in 0u64..1000,
            term in 1u64..1000,
        ) {
            let vote = init_self_vote(candidate_id, term);
            prop_assert_eq!(vote.voted_term, term);
            prop_assert_eq!(vote.voted_for, candidate_id);
        }
    }
}
