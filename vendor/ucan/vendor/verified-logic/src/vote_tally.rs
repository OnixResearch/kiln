// Verus preconditions guarantee u64→usize casts are in bounds
#![allow(clippy::cast_possible_truncation)]

//! Verified vote response tally for Raft elections.
//!
//! A candidate tracks votes as a boolean array indexed by server ID.
//! Votes are deduplicated (same server voting twice counts once),
//! stale-term responses are ignored, and election victory requires
//! a quorum. The tally starts with one vote (the candidate's self-vote).

use vstd::prelude::*;

use crate::quorum::is_quorum;
#[cfg(verus_keep_ghost)]
use crate::quorum::is_quorum_spec;

verus! {

// --- init_vote_tally ---

// r[depends vote-tally.init-votes]
/// Spec: create an empty vote tally of the given size.
pub open spec fn init_vote_tally_spec(cluster_size: u64) -> Seq<bool> {
    Seq::new(cluster_size as nat, |_i| false)
}

// r[impl vote-tally.init-votes]
/// Create a fresh vote tally with all votes set to false.
pub fn init_vote_tally(cluster_size: u64) -> (result: Vec<bool>)
    requires cluster_size as int <= usize::MAX as int
    ensures
        result@.len() == cluster_size as int,
        result@ == init_vote_tally_spec(cluster_size),
{
    let mut v: Vec<bool> = Vec::new();
    let mut i: u64 = 0;
    while i < cluster_size
        invariant
            0 <= i <= cluster_size,
            v@.len() == i as int,
            v@ =~= Seq::new(i as nat, |_j| false),
        decreases cluster_size - i,
    {
        v.push(false);
        assert(v@ =~= Seq::new((i + 1) as nat, |_j| false));
        i = i + 1;
    }
    v
}

// --- init_with_self_vote ---

// r[depends vote-tally.init-with-self-vote]
/// Spec: create a tally with the candidate's self-vote already counted.
pub open spec fn init_with_self_vote_spec(cluster_size: u64, self_id: u64) -> Seq<bool>
    recommends self_id < cluster_size
{
    Seq::new(cluster_size as nat, |i| i == self_id as int)
}

// r[impl vote-tally.init-with-self-vote]
/// Create a tally with the self-vote already counted.
pub fn init_with_self_vote(cluster_size: u64, self_id: u64) -> (result: Vec<bool>)
    requires
        self_id < cluster_size,
        cluster_size as int <= usize::MAX as int,
    ensures
        result@.len() == cluster_size as int,
        result@ == init_with_self_vote_spec(cluster_size, self_id),
{
    let mut v = init_vote_tally(cluster_size);
    #[allow(clippy::cast_possible_truncation)] // self_id < cluster_size, both fit in usize
    v.set(self_id as usize, true);
    assert(v@ =~= Seq::new(cluster_size as nat, |i| i == self_id as int));
    v
}

// --- record_vote ---

// r[depends vote-tally.record-vote]
/// Spec: record a vote from server_id (idempotent).
pub open spec fn record_vote_spec(
    votes: Seq<bool>,
    server_id: u64,
) -> Seq<bool>
    recommends server_id < votes.len() as u64
{
    votes.update(server_id as int, true)
}

// r[impl vote-tally.record-vote]
/// Record a vote from a server. Idempotent: re-voting has no effect.
pub fn record_vote(
    votes: &mut Vec<bool>,
    server_id: u64,
)
    requires
        (server_id as int) < old(votes)@.len(),
        old(votes)@.len() <= usize::MAX as int,
    ensures
        votes@ == record_vote_spec(old(votes)@, server_id),
        votes@.len() == old(votes)@.len(),
{
    votes.set(server_id as usize, true);
}

// --- count_votes ---

// r[depends vote-tally.count-votes]
/// Spec: count the number of true entries.
pub open spec fn count_votes_spec(votes: Seq<bool>) -> u64 {
    count_votes_from_spec(votes, 0)
}

pub open spec fn count_votes_from_spec(votes: Seq<bool>, from: int) -> u64
    decreases votes.len() - from,
{
    if from >= votes.len() {
        0u64
    } else {
        let rest = count_votes_from_spec(votes, from + 1);
        if votes[from] {
            (rest + 1) as u64
        } else {
            rest
        }
    }
}

// Helper: count_votes_from is bounded by remaining length.
proof fn count_votes_from_bounded(votes: Seq<bool>, from: int)
    requires 0 <= from, votes.len() <= u64::MAX as int
    ensures count_votes_from_spec(votes, from) <= (votes.len() - from) as u64
    decreases votes.len() - from,
{
    if from < votes.len() {
        count_votes_from_bounded(votes, from + 1);
    }
}

// r[impl vote-tally.count-votes]
/// Count the number of votes granted.
pub fn count_votes(votes: &[bool]) -> (result: u64)
    requires votes@.len() <= u64::MAX as int
    ensures result == count_votes_spec(votes@)
{
    let mut count: u64 = 0;
    let mut i: usize = 0;
    while i < votes.len()
        invariant
            0 <= i <= votes@.len(),
            votes@.len() <= u64::MAX as int,
            count_votes_spec(votes@) == count + count_votes_from_spec(votes@, i as int),
            count <= i as u64,
        decreases votes.len() - i,
    {
        // Unfold the recursive spec one step to help Z3
        // Prove the remaining count fits in u64 (needed for `(rest + 1) as u64` cast)
        proof { count_votes_from_bounded(votes@, i as int + 1); }
        if votes[i] {
            count = count + 1;
        }
        i = i + 1;
    }
    count
}

// --- has_won_election ---

// r[depends vote-tally.has-won]
/// Spec: has the candidate won the election?
pub open spec fn has_won_election_spec(votes: Seq<bool>, cluster_size: u64) -> bool
    recommends cluster_size > 0
{
    is_quorum_spec(count_votes_spec(votes), cluster_size)
}

// r[impl vote-tally.has-won]
/// Check if the candidate has won the election (quorum of votes).
pub fn has_won_election(votes: &[bool], cluster_size: u64) -> (result: bool)
    requires
        cluster_size > 0,
        votes@.len() <= u64::MAX as int,
    ensures result == has_won_election_spec(votes@, cluster_size)
{
    let vote_count = count_votes(votes);
    is_quorum(vote_count, cluster_size)
}

// --- is_stale_vote_response ---

// r[depends vote-tally.is-stale-response]
/// Spec: is this vote response from a different term?
pub open spec fn is_stale_vote_response_spec(
    response_term: u64,
    current_term: u64,
) -> bool {
    response_term != current_term
}

// r[impl vote-tally.is-stale-response]
/// Check if a vote response is stale (from a different term).
pub fn is_stale_vote_response(
    response_term: u64,
    current_term: u64,
) -> (result: bool)
    ensures result == is_stale_vote_response_spec(response_term, current_term)
{
    response_term != current_term
}

// --- Safety proofs ---

// r[verify vote-tally.count-votes]
/// Count is bounded by the tally length.
proof fn count_bounded(votes: Seq<bool>)
    requires votes.len() <= u64::MAX as int
    ensures count_votes_spec(votes) <= votes.len() as u64
{
    count_bounded_from(votes, 0);
}

proof fn count_bounded_from(votes: Seq<bool>, from: int)
    requires
        from >= 0,
        votes.len() <= u64::MAX as int,
    ensures count_votes_from_spec(votes, from) <= (votes.len() - from) as u64
    decreases votes.len() - from,
{
    if from >= votes.len() {
    } else {
        count_bounded_from(votes, from + 1);
    }
}

// r[verify vote-tally.record-vote]
/// Recording a vote is idempotent.
proof fn record_vote_idempotent(votes: Seq<bool>, server_id: u64)
    requires (server_id as int) < votes.len()
    ensures
        record_vote_spec(record_vote_spec(votes, server_id), server_id)
            =~= record_vote_spec(votes, server_id)
{}

// r[verify vote-tally.record-vote]
/// Recording a vote does not change other servers' votes.
proof fn record_vote_preserves_others(votes: Seq<bool>, server_id: u64, other: u64)
    requires
        (server_id as int) < votes.len(),
        (other as int) < votes.len(),
        server_id != other,
    ensures record_vote_spec(votes, server_id)[other as int] == votes[other as int]
{}

// r[verify vote-tally.init-votes]
/// Fresh tally has zero votes.
proof fn fresh_tally_zero(cluster_size: u64)
    requires cluster_size <= u64::MAX
    ensures count_votes_spec(init_vote_tally_spec(cluster_size)) == 0
{
    fresh_tally_zero_from(cluster_size, 0);
}

proof fn fresh_tally_zero_from(cluster_size: u64, from: int)
    requires from >= 0
    ensures count_votes_from_spec(init_vote_tally_spec(cluster_size), from) == 0
    decreases cluster_size as int - from,
{
    if from >= cluster_size as int {
    } else {
        fresh_tally_zero_from(cluster_size, from + 1);
    }
}

// r[verify vote-tally.is-stale-response]
/// Same-term responses are not stale.
proof fn same_term_not_stale(term: u64)
    ensures !is_stale_vote_response_spec(term, term)
{}

// r[verify vote-tally.is-stale-response]
/// Different-term responses are stale.
proof fn different_term_is_stale(response_term: u64, current_term: u64)
    requires response_term != current_term
    ensures is_stale_vote_response_spec(response_term, current_term)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify vote-tally.init-votes]
    #[test]
    fn test_init_vote_tally() {
        let tally = init_vote_tally(5);
        assert_eq!(tally.len(), 5);
        assert!(tally.iter().all(|&v| !v));
    }

    // r[verify vote-tally.init-with-self-vote]
    #[test]
    fn test_init_with_self_vote() {
        let tally = init_with_self_vote(5, 2);
        assert_eq!(tally.len(), 5);
        assert!(!tally[0]);
        assert!(!tally[1]);
        assert!(tally[2]); // self-vote
        assert!(!tally[3]);
        assert!(!tally[4]);
    }

    // r[verify vote-tally.record-vote]
    #[test]
    fn test_record_vote() {
        let mut tally = init_with_self_vote(5, 0);
        assert_eq!(count_votes(&tally), 1);
        record_vote(&mut tally, 2);
        assert_eq!(count_votes(&tally), 2);
        // Idempotent
        record_vote(&mut tally, 2);
        assert_eq!(count_votes(&tally), 2);
    }

    // r[verify vote-tally.count-votes]
    #[test]
    fn test_count_votes() {
        let tally = vec![true, false, true, false, true];
        assert_eq!(count_votes(&tally), 3);
        let empty: Vec<bool> = vec![];
        assert_eq!(count_votes(&empty), 0);
        let all_true = vec![true, true, true];
        assert_eq!(count_votes(&all_true), 3);
    }

    // r[verify vote-tally.has-won]
    #[test]
    fn test_has_won_election() {
        // 3 votes in 5-node cluster (majority = 3) → won
        let tally = vec![true, false, true, false, true];
        assert!(has_won_election(&tally, 5));
        // 2 votes in 5-node cluster → not won
        let tally2 = vec![true, false, true, false, false];
        assert!(!has_won_election(&tally2, 5));
    }

    // r[verify vote-tally.is-stale-response]
    #[test]
    fn test_is_stale_vote_response() {
        assert!(!is_stale_vote_response(5, 5)); // same term
        assert!(is_stale_vote_response(4, 5)); // old term
        assert!(is_stale_vote_response(6, 5)); // future term (higher-term step-down)
    }

    proptest! {
        // r[verify vote-tally.count-votes]
        #[test]
        fn prop_count_bounded(
            votes in proptest::collection::vec(proptest::bool::ANY, 0..=20),
        ) {
            let count = count_votes(&votes);
            prop_assert!(count as usize <= votes.len());
        }

        // r[verify vote-tally.record-vote]
        #[test]
        fn prop_record_idempotent(
            cluster in 1usize..=10,
            server_id in 0usize..10,
        ) {
            if server_id < cluster {
                let mut tally = init_vote_tally(cluster as u64);
                record_vote(&mut tally, server_id as u64);
                let count1 = count_votes(&tally);
                record_vote(&mut tally, server_id as u64);
                let count2 = count_votes(&tally);
                prop_assert_eq!(count1, count2, "idempotent violation");
            }
        }

        // r[verify vote-tally.has-won]
        #[test]
        fn prop_election_model_oracle(
            votes in proptest::collection::vec(proptest::bool::ANY, 5..=5),
        ) {
            let count = votes.iter().filter(|&&v| v).count() as u64;
            let expected = count >= 3; // majority(5) = 3
            let result = has_won_election(&votes, 5);
            prop_assert_eq!(result, expected);
        }

        #[test]
        fn prop_quorum_threshold_sweep(
            cluster_size in 1u64..=15,
        ) {
            let majority = cluster_size / 2 + 1;
            // Build tally with exactly majority-1 votes: not won
            let mut tally_under: Vec<bool> = vec![false; cluster_size as usize];
            for i in 0..(majority - 1).min(cluster_size) as usize {
                tally_under[i] = true;
            }
            if majority > 1 {
                prop_assert!(!has_won_election(&tally_under, cluster_size));
            }
            // Build tally with exactly majority votes: won
            let mut tally_at: Vec<bool> = vec![false; cluster_size as usize];
            for i in 0..majority.min(cluster_size) as usize {
                tally_at[i] = true;
            }
            prop_assert!(has_won_election(&tally_at, cluster_size));
        }

        #[test]
        fn prop_record_preserves_others(
            cluster in 3usize..=10,
            voter in 0usize..10,
            other in 0usize..10,
        ) {
            if voter < cluster && other < cluster && voter != other {
                let mut tally = init_vote_tally(cluster as u64);
                let before = tally[other];
                record_vote(&mut tally, voter as u64);
                prop_assert_eq!(tally[other], before);
            }
        }

        #[test]
        fn prop_count_matches_filter(
            votes in proptest::collection::vec(proptest::bool::ANY, 0..=20),
        ) {
            let exec_count = count_votes(&votes);
            let manual = votes.iter().filter(|&&v| v).count() as u64;
            prop_assert_eq!(exec_count, manual);
        }

        #[test]
        fn prop_self_vote_starts_at_one(
            cluster in 1u64..=20,
            self_id in 0u64..20,
        ) {
            if self_id < cluster {
                let tally = init_with_self_vote(cluster, self_id);
                prop_assert_eq!(count_votes(&tally), 1);
                prop_assert!(tally[self_id as usize]);
            }
        }
    }
}
