//! Verified ballot / vote tracking for Raft elections.
//!
//! Models the vote-granting decision: a node grants its vote at most once
//! per term, to the candidate with the most up-to-date log. Election safety
//! follows from quorum intersection: two winners in the same term would
//! require a voter who voted for both, contradicting single-vote-per-term.

#![allow(clippy::wildcard_imports)]

use vstd::prelude::*;

use crate::epoch::*;
use crate::quorum::*;

verus! {

/// Per-node vote state: the term we last voted in and who we voted for.
pub struct VoteState {
    /// The term in which the last vote was cast (0 = never voted).
    pub voted_term: u64,
    /// The candidate we voted for (meaningful only when `voted_term` > 0).
    pub voted_for: u64,
}

// --- can_grant_vote ---

// r[depends ballot.can-grant-vote]
/// Spec: can we grant a vote to `candidate` in `candidate_term`?
///
/// Requirements:
/// 1. candidate_term > voted_term (new term), OR
///    candidate_term == voted_term AND voted_for == candidate (re-vote for same)
/// 2. candidate's log (last_term, last_index) is at least as up-to-date as ours
pub open spec fn can_grant_vote_spec(
    state: VoteState,
    candidate: u64,
    candidate_term: u64,
    candidate_last: Epoch,
    my_last: Epoch,
) -> bool {
    let term_ok = candidate_term > state.voted_term
        || (candidate_term == state.voted_term && state.voted_for == candidate);
    let log_ok = epoch_ge_spec(candidate_last, my_last);
    term_ok && log_ok
}

// r[impl ballot.can-grant-vote]
/// Returns true if we should grant our vote to this candidate.
pub fn can_grant_vote(
    state: &VoteState,
    candidate: u64,
    candidate_term: u64,
    candidate_last: &Epoch,
    my_last: &Epoch,
) -> (result: bool)
    ensures result == can_grant_vote_spec(*state, candidate, candidate_term, *candidate_last, *my_last)
{
    let term_ok = candidate_term > state.voted_term
        || (candidate_term == state.voted_term && state.voted_for == candidate);
    let log_ok = epoch_ge(candidate_last, my_last);
    term_ok && log_ok
}

// --- grant_vote ---

// r[depends ballot.grant-vote]
/// Spec: record a vote grant. Returns updated VoteState.
pub open spec fn grant_vote_spec(
    candidate: u64,
    candidate_term: u64,
) -> VoteState {
    VoteState { voted_term: candidate_term, voted_for: candidate }
}

// r[impl ballot.grant-vote]
/// Record that we voted for `candidate` in `candidate_term`.
pub fn grant_vote(
    candidate: u64,
    candidate_term: u64,
) -> (result: VoteState)
    ensures result == grant_vote_spec(candidate, candidate_term)
{
    VoteState { voted_term: candidate_term, voted_for: candidate }
}

// --- has_won_election ---

// r[depends ballot.has-won-election]
/// Spec: has a candidate won with `votes_received` out of `cluster_size`?
pub open spec fn has_won_election_spec(votes_received: u64, cluster_size: u64) -> bool
    recommends cluster_size > 0
{
    is_quorum_spec(votes_received, cluster_size)
}

// r[impl ballot.has-won-election]
/// Returns true if the vote count constitutes a winning majority.
pub fn has_won_election(votes_received: u64, cluster_size: u64) -> (result: bool)
    requires cluster_size > 0
    ensures result == has_won_election_spec(votes_received, cluster_size)
{
    is_quorum(votes_received, cluster_size)
}

// --- Safety proofs ---

// r[verify ballot.can-grant-vote]
/// Single vote per term: after granting a vote, the only candidate
/// who can receive a re-vote in the same term is the same candidate.
proof fn single_vote_per_term(candidate_a: u64, candidate_b: u64, term: u64, log_a: Epoch, log_b: Epoch, my_log: Epoch)
    requires
        // We granted a vote to candidate_a
        epoch_ge_spec(log_a, my_log),
        // Now candidate_b asks for a vote in the same term
        candidate_a != candidate_b,
    ensures
        // The state after voting for A rejects B in the same term
        !can_grant_vote_spec(
            grant_vote_spec(candidate_a, term),
            candidate_b,
            term,
            log_b,
            my_log,
        )
{}

// r[verify ballot.has-won-election]
/// Election safety: if two candidates both win in the same term with
/// the same cluster size, the quorum intersection means a voter voted
/// for both — but single-vote-per-term says that voter is the same
/// candidate. Therefore both winners are the same candidate.
///
/// We state this as: two quorums overlap (proved by quorum module),
/// and the overlapping voter voted for only one candidate.
proof fn election_safety_quorum_overlap(votes_a: u64, votes_b: u64, n: u64)
    requires
        n > 0,
        has_won_election_spec(votes_a, n),
        has_won_election_spec(votes_b, n),
    ensures
        votes_a as int + votes_b as int > n as int
{
    prove_quorum_intersection(votes_a, votes_b, n);
}

// r[verify ballot.can-grant-vote]
/// Stale candidate rejection: a candidate with a lower term than our
/// voted_term is always rejected, regardless of log state.
proof fn stale_candidate_rejected(state: VoteState, candidate: u64, stale_term: u64, log: Epoch, my_log: Epoch)
    requires
        stale_term < state.voted_term,
    ensures
        !can_grant_vote_spec(state, candidate, stale_term, log, my_log)
{}

// r[verify ballot.can-grant-vote]
/// A candidate with an outdated log is rejected even in a new term.
proof fn outdated_log_rejected(state: VoteState, candidate: u64, new_term: u64, candidate_log: Epoch, my_log: Epoch)
    requires
        new_term > state.voted_term,
        !epoch_ge_spec(candidate_log, my_log),
    ensures
        !can_grant_vote_spec(state, candidate, new_term, candidate_log, my_log)
{}

// r[verify ballot.grant-vote]
/// After granting a vote, the state reflects the granted candidate and term.
proof fn grant_vote_records_correctly(candidate: u64, term: u64)
    ensures
        grant_vote_spec(candidate, term).voted_term == term,
        grant_vote_spec(candidate, term).voted_for == candidate,
{}

// r[verify ballot.can-grant-vote]
/// Re-vote for same candidate in same term is allowed (idempotent grant).
proof fn revote_same_candidate(candidate: u64, term: u64, log: Epoch, my_log: Epoch)
    requires epoch_ge_spec(log, my_log)
    ensures
        can_grant_vote_spec(
            grant_vote_spec(candidate, term),
            candidate,
            term,
            log,
            my_log,
        )
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn fresh_state() -> VoteState {
        VoteState {
            voted_term: 0,
            voted_for: 0,
        }
    }

    fn ep(term: u64, index: u64) -> Epoch {
        Epoch { term, index }
    }

    // r[verify ballot.can-grant-vote]
    #[test]
    fn test_can_grant_vote_fresh() {
        let state = fresh_state();
        // Fresh state, candidate in term 1 with up-to-date log
        assert!(can_grant_vote(&state, 1, 1, &ep(1, 0), &ep(0, 0)));
    }

    // r[verify ballot.can-grant-vote]
    #[test]
    fn test_can_grant_vote_stale_term() {
        let state = VoteState {
            voted_term: 5,
            voted_for: 1,
        };
        // Candidate in term 3 (stale) should be rejected
        assert!(!can_grant_vote(&state, 2, 3, &ep(3, 10), &ep(3, 0)));
    }

    // r[verify ballot.can-grant-vote]
    #[test]
    fn test_can_grant_vote_same_term_different_candidate() {
        let state = VoteState {
            voted_term: 5,
            voted_for: 1,
        };
        // Different candidate in same term should be rejected
        assert!(!can_grant_vote(&state, 2, 5, &ep(5, 10), &ep(5, 0)));
    }

    // r[verify ballot.can-grant-vote]
    #[test]
    fn test_can_grant_vote_same_term_same_candidate() {
        let state = VoteState {
            voted_term: 5,
            voted_for: 1,
        };
        // Same candidate re-requesting in same term is OK if log is up-to-date
        assert!(can_grant_vote(&state, 1, 5, &ep(5, 10), &ep(5, 0)));
    }

    // r[verify ballot.can-grant-vote]
    #[test]
    fn test_can_grant_vote_outdated_log() {
        let state = fresh_state();
        // New term but candidate's log is behind ours
        assert!(!can_grant_vote(&state, 1, 1, &ep(0, 5), &ep(1, 0)));
    }

    // r[verify ballot.has-won-election]
    #[test]
    fn test_has_won_election() {
        assert!(has_won_election(2, 3)); // 2/3 majority
        assert!(!has_won_election(1, 3)); // 1/3 not enough
        assert!(has_won_election(3, 5)); // 3/5 majority
        assert!(has_won_election(1, 1)); // singleton
    }

    // r[verify ballot.grant-vote]
    #[test]
    fn test_grant_vote() {
        let state = grant_vote(42, 7);
        assert_eq!(state.voted_term, 7);
        assert_eq!(state.voted_for, 42);
    }

    proptest! {
        // r[verify ballot.can-grant-vote]
        #[test]
        fn prop_single_vote_per_term(
            candidate_a: u64, candidate_b: u64, term: u64,
            lt: u64, li: u64, mt: u64, mi: u64,
        ) {
            if candidate_a != candidate_b {
                let state = grant_vote(candidate_a, term);
                let candidate_log = ep(lt, li);
                let my_log = ep(mt, mi);
                // B cannot get a vote in the same term as A
                prop_assert!(!can_grant_vote(&state, candidate_b, term, &candidate_log, &my_log));
            }
        }

        // r[verify ballot.can-grant-vote]
        #[test]
        fn prop_stale_candidate_always_rejected(
            voted_term in 1u64..=u64::MAX,
            voted_for: u64,
            candidate: u64,
            stale_term_delta in 1u64..=1000,
            lt: u64, li: u64, mt: u64, mi: u64,
        ) {
            let stale_term = voted_term.saturating_sub(stale_term_delta);
            if stale_term < voted_term {
                let state = VoteState { voted_term, voted_for };
                prop_assert!(!can_grant_vote(&state, candidate, stale_term, &ep(lt, li), &ep(mt, mi)));
            }
        }

        // r[verify ballot.can-grant-vote]
        #[test]
        fn prop_revote_idempotent(
            candidate: u64, term: u64,
            ct: u64, ci: u64,
            mt: u64, mi: u64,
        ) {
            let state = grant_vote(candidate, term);
            let candidate_log = ep(ct, ci);
            let my_log = ep(mt, mi);
            if epoch_ge(&candidate_log, &my_log) {
                prop_assert!(can_grant_vote(&state, candidate, term, &candidate_log, &my_log));
            }
        }

        // r[verify ballot.has-won-election]
        #[test]
        fn prop_election_quorum_overlap(
            n in 1u64..=10000,
            extra_a in 0u64..=100,
            extra_b in 0u64..=100,
        ) {
            let m = crate::quorum::majority(n);
            let a = m.saturating_add(extra_a).min(n);
            let b = m.saturating_add(extra_b).min(n);
            if has_won_election(a, n) && has_won_election(b, n) {
                prop_assert!(u128::from(a) + u128::from(b) > u128::from(n));
            }
        }
    }
}
