// Verus preconditions guarantee u64→usize casts are in bounds
#![allow(clippy::cast_possible_truncation)]

//! Verified pre-vote response tally for Raft elections.
//!
//! A candidate tracks pre-votes separately from real votes. Pre-vote
//! responses are tallied like regular votes (boolean array, dedup,
//! quorum check) but do NOT cause term increments or state changes.
//! Once a quorum of pre-votes is collected, the candidate starts a
//! real election.
//!
//! This mirrors `vote_tally.rs` but for the pre-vote phase.

use vstd::prelude::*;

use crate::quorum::is_quorum;
#[cfg(verus_keep_ghost)]
use crate::quorum::is_quorum_spec;

verus! {

// --- init_pre_vote_tally ---

// r[depends pre-vote-tally.init]
/// Spec: create an empty pre-vote tally.
pub open spec fn init_pre_vote_tally_spec(cluster_size: u64) -> Seq<bool> {
    Seq::new(cluster_size as nat, |_i| false)
}

// r[impl pre-vote-tally.init]
/// Create a fresh pre-vote tally with all responses unset.
pub fn init_pre_vote_tally(cluster_size: u64) -> (result: Vec<bool>)
    requires cluster_size as int <= usize::MAX as int
    ensures
        result@.len() == cluster_size as int,
        result@ == init_pre_vote_tally_spec(cluster_size),
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

// --- init_with_self_pre_vote ---

// r[depends pre-vote-tally.init-with-self]
/// Spec: create a tally with the candidate's own pre-vote.
pub open spec fn init_with_self_pre_vote_spec(cluster_size: u64, self_id: u64) -> Seq<bool>
    recommends self_id < cluster_size
{
    Seq::new(cluster_size as nat, |i| i == self_id as int)
}

// r[impl pre-vote-tally.init-with-self]
/// Create a pre-vote tally with the candidate's self-vote.
pub fn init_with_self_pre_vote(cluster_size: u64, self_id: u64) -> (result: Vec<bool>)
    requires
        self_id < cluster_size,
        cluster_size as int <= usize::MAX as int,
    ensures
        result@.len() == cluster_size as int,
        result@ == init_with_self_pre_vote_spec(cluster_size, self_id),
{
    let mut v = init_pre_vote_tally(cluster_size);
    #[allow(clippy::cast_possible_truncation)]
    v.set(self_id as usize, true);
    assert(v@ =~= Seq::new(cluster_size as nat, |i| i == self_id as int));
    v
}

// --- record_pre_vote ---

// r[depends pre-vote-tally.record]
/// Spec: record a pre-vote response from server_id (idempotent).
pub open spec fn record_pre_vote_spec(
    pre_votes: Seq<bool>,
    server_id: u64,
) -> Seq<bool>
    recommends server_id < pre_votes.len() as u64
{
    pre_votes.update(server_id as int, true)
}

// r[impl pre-vote-tally.record]
/// Record a pre-vote response. Idempotent.
pub fn record_pre_vote(
    pre_votes: &mut Vec<bool>,
    server_id: u64,
)
    requires
        (server_id as int) < old(pre_votes)@.len(),
        old(pre_votes)@.len() <= usize::MAX as int,
    ensures
        pre_votes@ == record_pre_vote_spec(old(pre_votes)@, server_id),
        pre_votes@.len() == old(pre_votes)@.len(),
{
    pre_votes.set(server_id as usize, true);
}

// --- count_pre_votes ---

// r[depends pre-vote-tally.count]
/// Spec: count pre-vote grants.
pub open spec fn count_pre_votes_spec(pre_votes: Seq<bool>) -> u64 {
    count_pre_votes_from_spec(pre_votes, 0)
}

pub open spec fn count_pre_votes_from_spec(pre_votes: Seq<bool>, from: int) -> u64
    decreases pre_votes.len() - from,
{
    if from >= pre_votes.len() {
        0u64
    } else {
        let rest = count_pre_votes_from_spec(pre_votes, from + 1);
        if pre_votes[from] {
            (rest + 1) as u64
        } else {
            rest
        }
    }
}

// Helper: count_pre_votes_from is bounded by remaining length.
proof fn count_pre_votes_from_bounded(pre_votes: Seq<bool>, from: int)
    requires 0 <= from, pre_votes.len() <= u64::MAX as int
    ensures count_pre_votes_from_spec(pre_votes, from) <= (pre_votes.len() - from) as u64
    decreases pre_votes.len() - from,
{
    if from < pre_votes.len() {
        count_pre_votes_from_bounded(pre_votes, from + 1);
    }
}

// r[impl pre-vote-tally.count]
/// Count pre-vote grants.
pub fn count_pre_votes(pre_votes: &[bool]) -> (result: u64)
    requires pre_votes@.len() <= u64::MAX as int
    ensures result == count_pre_votes_spec(pre_votes@)
{
    let mut count: u64 = 0;
    let mut i: usize = 0;
    while i < pre_votes.len()
        invariant
            0 <= i <= pre_votes@.len(),
            pre_votes@.len() <= u64::MAX as int,
            count_pre_votes_spec(pre_votes@) == count + count_pre_votes_from_spec(pre_votes@, i as int),
            count <= i as u64,
        decreases pre_votes.len() - i,
    {
        proof { count_pre_votes_from_bounded(pre_votes@, i as int + 1); }
        if pre_votes[i] {
            count = count + 1;
        }
        i = i + 1;
    }
    count
}

// --- has_pre_vote_quorum ---

// r[depends pre-vote-tally.has-quorum]
/// Spec: has the candidate gathered enough pre-votes to start a real election?
pub open spec fn has_pre_vote_quorum_spec(pre_votes: Seq<bool>, cluster_size: u64) -> bool
    recommends cluster_size > 0
{
    is_quorum_spec(count_pre_votes_spec(pre_votes), cluster_size)
}

// r[impl pre-vote-tally.has-quorum]
/// Check if pre-votes form a quorum.
pub fn has_pre_vote_quorum(pre_votes: &[bool], cluster_size: u64) -> (result: bool)
    requires
        cluster_size > 0,
        pre_votes@.len() <= u64::MAX as int,
    ensures result == has_pre_vote_quorum_spec(pre_votes@, cluster_size)
{
    let count = count_pre_votes(pre_votes);
    is_quorum(count, cluster_size)
}

// --- is_stale_pre_vote_response ---

// r[depends pre-vote-tally.stale-response]
/// Spec: is this pre-vote response from a different prospective term?
///
/// Pre-vote responses carry the prospective term (current_term + 1).
/// Responses from a different prospective term are stale.
pub open spec fn is_stale_pre_vote_response_spec(
    response_term: u64,
    prospective_term: u64,
) -> bool {
    response_term != prospective_term
}

// r[impl pre-vote-tally.stale-response]
/// Check if a pre-vote response is stale.
pub fn is_stale_pre_vote_response(
    response_term: u64,
    prospective_term: u64,
) -> (result: bool)
    ensures result == is_stale_pre_vote_response_spec(response_term, prospective_term)
{
    response_term != prospective_term
}

// --- Safety proofs ---

// r[verify pre-vote-tally.count]
/// Count bounded by tally length.
proof fn count_bounded(pre_votes: Seq<bool>)
    requires pre_votes.len() <= u64::MAX as int
    ensures count_pre_votes_spec(pre_votes) <= pre_votes.len() as u64
{
    count_bounded_from(pre_votes, 0);
}

proof fn count_bounded_from(pre_votes: Seq<bool>, from: int)
    requires
        from >= 0,
        pre_votes.len() <= u64::MAX as int,
    ensures count_pre_votes_from_spec(pre_votes, from) <= (pre_votes.len() - from) as u64
    decreases pre_votes.len() - from,
{
    if from >= pre_votes.len() {
    } else {
        count_bounded_from(pre_votes, from + 1);
    }
}

// r[verify pre-vote-tally.record]
/// Recording is idempotent.
proof fn record_idempotent(pre_votes: Seq<bool>, server_id: u64)
    requires (server_id as int) < pre_votes.len()
    ensures
        record_pre_vote_spec(record_pre_vote_spec(pre_votes, server_id), server_id)
            =~= record_pre_vote_spec(pre_votes, server_id)
{}

// r[verify pre-vote-tally.record]
/// Recording preserves other entries.
proof fn record_preserves_others(pre_votes: Seq<bool>, server_id: u64, other: u64)
    requires
        (server_id as int) < pre_votes.len(),
        (other as int) < pre_votes.len(),
        server_id != other,
    ensures record_pre_vote_spec(pre_votes, server_id)[other as int] == pre_votes[other as int]
{}

// r[verify pre-vote-tally.init]
/// Fresh tally has zero pre-votes.
proof fn fresh_tally_zero(cluster_size: u64)
    requires cluster_size <= u64::MAX
    ensures count_pre_votes_spec(init_pre_vote_tally_spec(cluster_size)) == 0
{
    fresh_tally_zero_from(cluster_size, 0);
}

proof fn fresh_tally_zero_from(cluster_size: u64, from: int)
    requires from >= 0
    ensures count_pre_votes_from_spec(init_pre_vote_tally_spec(cluster_size), from) == 0
    decreases cluster_size as int - from,
{
    if from >= cluster_size as int {
    } else {
        fresh_tally_zero_from(cluster_size, from + 1);
    }
}

// r[verify pre-vote-tally.stale-response]
/// Same prospective term is not stale.
proof fn same_term_not_stale(term: u64)
    ensures !is_stale_pre_vote_response_spec(term, term)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify pre-vote-tally.init]
    #[test]
    fn test_init_pre_vote_tally() {
        let tally = init_pre_vote_tally(5);
        assert_eq!(tally.len(), 5);
        assert!(tally.iter().all(|&v| !v));
    }

    // r[verify pre-vote-tally.init-with-self]
    #[test]
    fn test_init_with_self_pre_vote() {
        let tally = init_with_self_pre_vote(5, 2);
        assert_eq!(tally.len(), 5);
        assert!(!tally[0]);
        assert!(!tally[1]);
        assert!(tally[2]);
        assert!(!tally[3]);
        assert!(!tally[4]);
    }

    // r[verify pre-vote-tally.record]
    #[test]
    fn test_record_pre_vote() {
        let mut tally = init_with_self_pre_vote(5, 0);
        assert_eq!(count_pre_votes(&tally), 1);
        record_pre_vote(&mut tally, 2);
        assert_eq!(count_pre_votes(&tally), 2);
        record_pre_vote(&mut tally, 2);
        assert_eq!(count_pre_votes(&tally), 2);
    }

    // r[verify pre-vote-tally.count]
    #[test]
    fn test_count_pre_votes() {
        let tally = vec![true, false, true, false, true];
        assert_eq!(count_pre_votes(&tally), 3);
        let empty: Vec<bool> = vec![];
        assert_eq!(count_pre_votes(&empty), 0);
    }

    // r[verify pre-vote-tally.has-quorum]
    #[test]
    fn test_has_pre_vote_quorum() {
        let tally = vec![true, false, true, false, true];
        assert!(has_pre_vote_quorum(&tally, 5));
        let tally2 = vec![true, false, true, false, false];
        assert!(!has_pre_vote_quorum(&tally2, 5));
    }

    // r[verify pre-vote-tally.stale-response]
    #[test]
    fn test_stale_pre_vote_response() {
        assert!(!is_stale_pre_vote_response(5, 5));
        assert!(is_stale_pre_vote_response(4, 5));
        assert!(is_stale_pre_vote_response(6, 5));
    }

    proptest! {
        // r[verify pre-vote-tally.count]
        #[test]
        fn prop_count_bounded(
            votes in proptest::collection::vec(proptest::bool::ANY, 0..=20),
        ) {
            let count = count_pre_votes(&votes);
            prop_assert!(count as usize <= votes.len());
        }

        // r[verify pre-vote-tally.record]
        #[test]
        fn prop_record_idempotent(
            cluster in 1usize..=10,
            server_id in 0usize..10,
        ) {
            if server_id < cluster {
                let mut tally = init_pre_vote_tally(cluster as u64);
                record_pre_vote(&mut tally, server_id as u64);
                let c1 = count_pre_votes(&tally);
                record_pre_vote(&mut tally, server_id as u64);
                let c2 = count_pre_votes(&tally);
                prop_assert_eq!(c1, c2);
            }
        }

        // r[verify pre-vote-tally.has-quorum]
        #[test]
        fn prop_quorum_model_oracle(
            votes in proptest::collection::vec(proptest::bool::ANY, 5..=5),
        ) {
            let count = votes.iter().filter(|&&v| v).count() as u64;
            let expected = count >= 3;
            let result = has_pre_vote_quorum(&votes, 5);
            prop_assert_eq!(result, expected);
        }

        #[test]
        fn prop_quorum_threshold_sweep(
            cluster_size in 1u64..=15,
        ) {
            let majority = cluster_size / 2 + 1;
            let mut tally_under: Vec<bool> = vec![false; cluster_size as usize];
            for i in 0..(majority - 1).min(cluster_size) as usize {
                tally_under[i] = true;
            }
            if majority > 1 {
                prop_assert!(!has_pre_vote_quorum(&tally_under, cluster_size));
            }
            let mut tally_at: Vec<bool> = vec![false; cluster_size as usize];
            for i in 0..majority.min(cluster_size) as usize {
                tally_at[i] = true;
            }
            prop_assert!(has_pre_vote_quorum(&tally_at, cluster_size));
        }

        #[test]
        fn prop_count_matches_filter(
            votes in proptest::collection::vec(proptest::bool::ANY, 0..=20),
        ) {
            let exec_count = count_pre_votes(&votes);
            let manual = votes.iter().filter(|&&v| v).count() as u64;
            prop_assert_eq!(exec_count, manual);
        }

        #[test]
        fn prop_record_preserves_others(
            cluster in 3usize..=10,
            voter in 0usize..10,
            other in 0usize..10,
        ) {
            if voter < cluster && other < cluster && voter != other {
                let mut tally = init_pre_vote_tally(cluster as u64);
                let before = tally[other];
                record_pre_vote(&mut tally, voter as u64);
                prop_assert_eq!(tally[other], before);
            }
        }
    }
}
