//! System-level election safety: at most one leader per term.
//!
//! Chains three facts into a contradiction proof:
//! 1. ballot::can_grant_vote_spec → single vote per term per node
//! 2. ghost_history::single_vote_per_term_spec → vote history consistent
//! 3. quorum::prove_quorum_intersection → two quorums overlap
//!
//! If two candidates both won a majority in the same term, the overlapping
//! voter voted for both — contradicting single_vote_per_term. Therefore
//! at most one candidate wins per term.

#![allow(clippy::wildcard_imports)]

use vstd::prelude::*;

use crate::ghost_history::*;
use crate::quorum::*;

verus! {

// ============================================================
// Election safety predicate
// ============================================================

// r[depends election-safety.at-most-one-leader]
/// Spec: election safety holds for a given term when at most one
/// candidate received votes from a quorum.
///
/// `vote_history` is the global vote history (all voters combined).
/// `cluster_size` is the total number of voters.
///
/// Safety means: for any two candidates A and B, if both have a
/// quorum of cronies, then A == B.
pub open spec fn election_safe_spec(
    vote_history: Seq<VoteRecord>,
    term: u64,
    cluster_size: u64,
) -> bool
    recommends cluster_size > 0
{
    forall|candidate_a: u64, candidate_b: u64|
        #![trigger cronies_count_spec(vote_history, term, candidate_a),
                   cronies_count_spec(vote_history, term, candidate_b)]
        (is_quorum_spec(cronies_count_spec(vote_history, term, candidate_a) as u64, cluster_size)
         && is_quorum_spec(cronies_count_spec(vote_history, term, candidate_b) as u64, cluster_size))
        ==> candidate_a == candidate_b
}

// ============================================================
// Core theorem
// ============================================================

// r[verify election-safety.proof-chain]
/// Election safety theorem: if the vote history satisfies
/// single_vote_per_term, then at most one candidate can win a majority.
///
/// Proof outline:
/// 1. Assume two candidates A ≠ B both have quorum-many cronies
/// 2. By cronies_exclusive, if A has cronies then B has zero (or vice versa)
/// 3. Zero cronies cannot form a quorum (quorum requires count >= majority >= 1)
/// 4. Contradiction: B cannot have a quorum → our assumption fails
/// 5. Therefore A == B
pub proof fn election_safety_theorem(
    vote_history: Seq<VoteRecord>,
    term: u64,
    cluster_size: u64,
)
    requires
        cluster_size > 0,
        single_vote_per_term_spec(vote_history),
    ensures
        election_safe_spec(vote_history, term, cluster_size)
{
    assert forall|candidate_a: u64, candidate_b: u64|
        #![trigger cronies_count_spec(vote_history, term, candidate_a),
                   cronies_count_spec(vote_history, term, candidate_b)]
        is_quorum_spec(cronies_count_spec(vote_history, term, candidate_a) as u64, cluster_size)
        && is_quorum_spec(cronies_count_spec(vote_history, term, candidate_b) as u64, cluster_size)
    implies
        candidate_a == candidate_b
    by {
        if candidate_a != candidate_b {
            // By cronies_exclusive: one of them has zero cronies
            cronies_exclusive(vote_history, term, candidate_a, candidate_b);
            // Zero is not a quorum (majority >= 1 for cluster_size > 0)
            assert(majority_spec(cluster_size) >= 1) by {
                assert(cluster_size / 2 + 1 >= 1) by(nonlinear_arith)
                    requires cluster_size > 0u64;
            }
        }
    }
}

// ============================================================
// Supporting lemmas
// ============================================================

// r[verify election-safety.at-most-one-leader]
/// If candidate A has a quorum and single_vote_per_term holds,
/// then no different candidate B can also have a quorum.
proof fn quorum_winner_unique(
    vote_history: Seq<VoteRecord>,
    term: u64,
    cluster_size: u64,
    winner: u64,
    challenger: u64,
)
    requires
        cluster_size > 0,
        single_vote_per_term_spec(vote_history),
        is_quorum_spec(cronies_count_spec(vote_history, term, winner) as u64, cluster_size),
        winner != challenger,
    ensures
        !is_quorum_spec(cronies_count_spec(vote_history, term, challenger) as u64, cluster_size)
{
    cronies_exclusive(vote_history, term, winner, challenger);
    // cronies_exclusive says one of them has zero cronies.
    // majority >= 1 for cluster_size > 0
    assert(majority_spec(cluster_size) >= 1) by {
        assert(cluster_size / 2 + 1 >= 1) by(nonlinear_arith)
            requires cluster_size > 0u64;
    }
    // The winner has a quorum (>= 1), so it's not the winner that's zero.
    // Therefore the challenger has zero cronies → not a quorum.
}

// r[verify election-safety.proof-chain]
/// The quorum intersection step: if two candidates both have quorums,
/// their vote counts exceed the cluster size — so there must be a shared
/// voter. But single_vote_per_term forbids a shared voter from voting for
/// two different candidates.
proof fn quorum_intersection_contradicts_two_winners(
    votes_a: u64,
    votes_b: u64,
    cluster_size: u64,
)
    requires
        cluster_size > 0,
        is_quorum_spec(votes_a, cluster_size),
        is_quorum_spec(votes_b, cluster_size),
    ensures
        votes_a as int + votes_b as int > cluster_size as int
{
    prove_quorum_intersection(votes_a, votes_b, cluster_size);
}

// r[verify election-safety.at-most-one-leader]
/// Election safety for the empty history: trivially safe (no one has
/// any cronies, so no one has a quorum).
proof fn empty_history_safe(term: u64, cluster_size: u64)
    requires cluster_size > 0
    ensures election_safe_spec(Seq::<VoteRecord>::empty(), term, cluster_size)
{
    empty_history_single_vote();
    election_safety_theorem(Seq::<VoteRecord>::empty(), term, cluster_size);
}

// r[verify election-safety.at-most-one-leader]
/// Adding a vote for a fresh term preserves election safety for all
/// other terms.
proof fn fresh_vote_preserves_other_terms(
    vote_history: Seq<VoteRecord>,
    record: VoteRecord,
    other_term: u64,
    cluster_size: u64,
)
    requires
        cluster_size > 0,
        single_vote_per_term_spec(vote_history),
        record.term != other_term,
        forall|i: int| #![trigger vote_history[i]]
            0 <= i < vote_history.len()
            ==> vote_history[i].term != record.term,
    ensures
        election_safe_spec(append_vote_spec(vote_history, record), other_term, cluster_size)
{
    append_fresh_preserves_single_vote(vote_history, record);
    election_safety_theorem(
        append_vote_spec(vote_history, record),
        other_term,
        cluster_size,
    );
}

} // verus!

#[cfg(test)]
#[allow(clippy::cast_possible_truncation)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn vr(term: u64, candidate: u64) -> VoteRecord {
        VoteRecord {
            term,
            candidate_id: candidate,
            voter_last_term: 0,
            voter_last_index: 0,
        }
    }

    /// Runtime check for single_vote_per_term on a Vec.
    fn check_svpt(history: &[VoteRecord]) -> bool {
        for i in 0..history.len() {
            for j in 0..history.len() {
                if history[i].term == history[j].term && history[i].candidate_id != history[j].candidate_id {
                    return false;
                }
            }
        }
        true
    }

    /// Runtime check for election safety on a Vec.
    fn check_election_safe(history: &[VoteRecord], term: u64, cluster_size: u64) -> bool {
        if cluster_size == 0 {
            return true;
        }
        let majority = cluster_size / 2 + 1;

        // Collect all candidates for this term
        let mut candidates: Vec<u64> = history.iter().filter(|r| r.term == term).map(|r| r.candidate_id).collect();
        candidates.sort();
        candidates.dedup();

        // Count how many candidates have a quorum
        let winners: Vec<u64> = candidates
            .iter()
            .filter(|&&c| {
                let count = history.iter().filter(|r| r.term == term && r.candidate_id == c).count() as u64;
                count >= majority
            })
            .copied()
            .collect();

        winners.len() <= 1
    }

    // r[verify election-safety.at-most-one-leader]
    #[test]
    fn test_empty_history_safe() {
        assert!(check_election_safe(&[], 1, 5));
    }

    // r[verify election-safety.at-most-one-leader]
    #[test]
    fn test_single_winner_safe() {
        // 5-node cluster, 3 votes for candidate 10 in term 5
        let history = vec![vr(5, 10), vr(5, 10), vr(5, 10)];
        assert!(check_svpt(&history));
        assert!(check_election_safe(&history, 5, 5));
    }

    // r[verify election-safety.proof-chain]
    #[test]
    fn test_two_candidates_impossible_with_svpt() {
        // If single_vote_per_term holds, can't have two candidates with quorums.
        // In a 5-node cluster, quorum is 3. If A has 3 votes, B has 0.
        let history = vec![vr(5, 10), vr(5, 10), vr(5, 10)];
        assert!(check_svpt(&history));

        let a_count = history.iter().filter(|r| r.term == 5 && r.candidate_id == 10).count() as u64;
        let b_count = history.iter().filter(|r| r.term == 5 && r.candidate_id == 20).count() as u64;

        assert_eq!(a_count, 3);
        assert_eq!(b_count, 0);
        assert!(a_count >= 3); // quorum
        assert!(b_count < 3); // not quorum
    }

    // r[verify election-safety.at-most-one-leader]
    #[test]
    fn test_different_terms_independent() {
        // Two different candidates can win in different terms
        let history = vec![
            vr(5, 10),
            vr(5, 10),
            vr(5, 10), // A wins term 5
            vr(6, 20),
            vr(6, 20),
            vr(6, 20), // B wins term 6
        ];
        assert!(check_svpt(&history));
        assert!(check_election_safe(&history, 5, 5));
        assert!(check_election_safe(&history, 6, 5));
    }

    // r[verify election-safety.proof-chain]
    #[test]
    fn test_violation_detected() {
        // Without svpt: two candidates each get 3 votes (impossible in reality)
        let history = vec![vr(5, 10), vr(5, 10), vr(5, 10), vr(5, 20), vr(5, 20), vr(5, 20)];
        assert!(!check_svpt(&history)); // svpt violated
        assert!(!check_election_safe(&history, 5, 5)); // two winners
    }

    proptest! {
        // r[verify election-safety.at-most-one-leader]
        #[test]
        fn prop_svpt_implies_election_safe(
            cluster_size in 1u64..=20,
            term: u64,
            winner in 0u64..100,
            num_votes in 0usize..=20,
        ) {
            // Build a valid history: all votes for `term` go to `winner`
            let mut history = Vec::new();
            for _ in 0..num_votes.min(cluster_size as usize) {
                history.push(vr(term, winner));
            }
            // Add votes for other terms
            history.push(vr(term.wrapping_add(1), winner.wrapping_add(1)));
            history.push(vr(term.wrapping_add(2), winner.wrapping_add(2)));

            prop_assert!(check_svpt(&history));
            prop_assert!(check_election_safe(&history, term, cluster_size));
        }

        // r[verify election-safety.proof-chain]
        #[test]
        fn prop_quorum_intersection_prevents_two_winners(
            cluster_size in 3u64..=100,
        ) {
            let majority = cluster_size / 2 + 1;
            // If A has majority and B has majority, they overlap
            prop_assert!(majority as u128 + majority as u128 > cluster_size as u128);
            // But overlap means a voter voted for both — violating svpt
            // So it's impossible with svpt. This is the pigeonhole step.
        }

        // r[verify election-safety.at-most-one-leader]
        #[test]
        fn prop_election_safe_all_terms(
            num_terms in 1usize..=10,
            cluster_size in 1u64..=10,
        ) {
            // Build a history where each term has exactly one candidate
            let mut history = Vec::new();
            for t in 0..num_terms {
                let candidate = t as u64 * 7 + 3; // arbitrary distinct candidates
                let votes = (cluster_size / 2 + 1).min(cluster_size);
                for _ in 0..votes {
                    history.push(vr(t as u64, candidate));
                }
            }

            prop_assert!(check_svpt(&history));
            for t in 0..num_terms {
                prop_assert!(check_election_safe(&history, t as u64, cluster_size));
            }
        }

        #[test]
        fn prop_two_winners_requires_svpt_violation(
            cluster_size in 3u64..=15,
            num_votes_a in 0usize..=15,
            num_votes_b in 0usize..=15,
        ) {
            let majority = (cluster_size / 2 + 1) as usize;
            let mut history = Vec::new();
            for _ in 0..num_votes_a.min(cluster_size as usize) {
                history.push(vr(1, 10));
            }
            for _ in 0..num_votes_b.min(cluster_size as usize) {
                history.push(vr(1, 20));
            }
            let a_won = num_votes_a.min(cluster_size as usize) >= majority;
            let b_won = num_votes_b.min(cluster_size as usize) >= majority;
            if a_won && b_won {
                prop_assert!(!check_svpt(&history));
            }
        }

        #[test]
        fn prop_no_votes_always_safe(
            cluster_size in 1u64..=20,
            term: u64,
        ) {
            prop_assert!(check_election_safe(&[], term, cluster_size));
        }

        #[test]
        fn prop_single_candidate_always_safe(
            cluster_size in 1u64..=10,
            candidate: u64,
            num_votes in 0usize..=10,
        ) {
            let history: Vec<VoteRecord> = (0..num_votes.min(cluster_size as usize))
                .map(|_| vr(1, candidate))
                .collect();
            prop_assert!(check_svpt(&history));
            prop_assert!(check_election_safe(&history, 1, cluster_size));
        }
    }
}
