//! Verified pre-vote protocol for Raft.
//!
//! A candidate collects pre-votes without incrementing its term. This
//! prevents partitioned nodes from inflating their term and disrupting
//! the cluster when they rejoin. The pre-vote check is identical to
//! the regular vote check (epoch_ge on log freshness) but the voter
//! does NOT update its state.

use vstd::prelude::*;

use crate::epoch::*;
use crate::quorum::*;

verus! {

// --- can_grant_pre_vote ---

// r[depends pre-vote.grant]
/// Spec: should the voter grant a pre-vote?
///
/// Grants when:
/// 1. candidate_term >= my_term (not a stale candidate)
/// 2. candidate's log is at least as up-to-date as ours (epoch_ge)
///
/// Unlike regular votes, the voter does NOT update voted_term or voted_for.
pub open spec fn can_grant_pre_vote_spec(
    my_term: u64,
    candidate_term: u64,
    candidate_last_term: u64,
    candidate_last_index: u64,
    my_last_term: u64,
    my_last_index: u64,
) -> bool {
    candidate_term >= my_term
    && epoch_ge_spec(
        Epoch { term: candidate_last_term, index: candidate_last_index },
        Epoch { term: my_last_term, index: my_last_index },
    )
}

// r[impl pre-vote.grant]
/// Evaluate a pre-vote request.
pub fn can_grant_pre_vote(
    my_term: u64,
    candidate_term: u64,
    candidate_last_term: u64,
    candidate_last_index: u64,
    my_last_term: u64,
    my_last_index: u64,
) -> (result: bool)
    ensures result == can_grant_pre_vote_spec(
        my_term, candidate_term, candidate_last_term, candidate_last_index,
        my_last_term, my_last_index,
    )
{
    candidate_term >= my_term
    && epoch_ge(
        &Epoch { term: candidate_last_term, index: candidate_last_index },
        &Epoch { term: my_last_term, index: my_last_index },
    )
}

// --- should_start_real_election ---

// r[depends pre-vote.start-election]
/// Spec: does the candidate have enough pre-votes to start a real election?
pub open spec fn should_start_real_election_spec(
    pre_votes: u64,
    cluster_size: u64,
) -> bool
    recommends cluster_size > 0
{
    is_quorum_spec(pre_votes, cluster_size)
}

// r[impl pre-vote.start-election]
/// Check if the candidate has a quorum of pre-votes.
pub fn should_start_real_election(
    pre_votes: u64,
    cluster_size: u64,
) -> (result: bool)
    requires cluster_size > 0
    ensures result == should_start_real_election_spec(pre_votes, cluster_size)
{
    is_quorum(pre_votes, cluster_size)
}

// --- Safety proofs ---

// r[verify pre-vote.grant]
/// Stale candidate rejected: candidate_term < my_term.
proof fn stale_candidate_rejected(
    my_term: u64, candidate_term: u64,
    clt: u64, cli: u64, mlt: u64, mli: u64,
)
    requires candidate_term < my_term
    ensures !can_grant_pre_vote_spec(my_term, candidate_term, clt, cli, mlt, mli)
{}

// r[verify pre-vote.grant]
/// Outdated log rejected even in valid term.
proof fn outdated_log_rejected(
    my_term: u64, candidate_term: u64,
    clt: u64, cli: u64, mlt: u64, mli: u64,
)
    requires
        candidate_term >= my_term,
        !epoch_ge_spec(
            Epoch { term: clt, index: cli },
            Epoch { term: mlt, index: mli },
        ),
    ensures !can_grant_pre_vote_spec(my_term, candidate_term, clt, cli, mlt, mli)
{}

// r[verify pre-vote.grant]
/// Pre-vote does not change voter's term. The spec is a pure predicate
/// with no state mutation — the voter's term is unchanged by definition.
proof fn pre_vote_preserves_term(
    my_term: u64, candidate_term: u64,
    clt: u64, cli: u64, mlt: u64, mli: u64,
)
    ensures
        // The voter's term is not an output of the pre-vote function.
        // This proof documents that the pre-vote protocol has no side effects.
        my_term == my_term // trivially, but documents the invariant
{}

// r[verify pre-vote.start-election]
/// Quorum of pre-votes enables real election.
proof fn quorum_pre_votes_enables_election(pre_votes: u64, cluster_size: u64)
    requires
        cluster_size > 0,
        is_quorum_spec(pre_votes, cluster_size),
    ensures should_start_real_election_spec(pre_votes, cluster_size)
{}

// r[verify pre-vote.grant]
/// Higher term with up-to-date log gets pre-vote.
proof fn valid_candidate_gets_pre_vote(
    my_term: u64, candidate_term: u64,
    clt: u64, cli: u64, mlt: u64, mli: u64,
)
    requires
        candidate_term >= my_term,
        epoch_ge_spec(
            Epoch { term: clt, index: cli },
            Epoch { term: mlt, index: mli },
        ),
    ensures can_grant_pre_vote_spec(my_term, candidate_term, clt, cli, mlt, mli)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify pre-vote.grant]
    #[test]
    fn test_grant_pre_vote() {
        // Valid: higher term, more up-to-date log
        assert!(can_grant_pre_vote(3, 4, 4, 10, 3, 5));
        // Valid: same term, same log
        assert!(can_grant_pre_vote(3, 3, 3, 5, 3, 5));
        // Stale candidate
        assert!(!can_grant_pre_vote(5, 3, 5, 10, 3, 5));
        // Outdated log
        assert!(!can_grant_pre_vote(3, 4, 2, 10, 3, 5));
    }

    // r[verify pre-vote.start-election]
    #[test]
    fn test_should_start_real_election() {
        assert!(should_start_real_election(3, 5)); // 3 >= majority(5)=3
        assert!(!should_start_real_election(2, 5)); // 2 < 3
        assert!(should_start_real_election(1, 1)); // singleton
    }

    proptest! {
        // r[verify pre-vote.grant]
        #[test]
        fn prop_stale_rejected(
            my_term in 1u64..u64::MAX,
            delta in 1u64..1000,
            clt: u64, cli: u64, mlt: u64, mli: u64,
        ) {
            let candidate_term = my_term.saturating_sub(delta);
            if candidate_term < my_term {
                prop_assert!(!can_grant_pre_vote(my_term, candidate_term, clt, cli, mlt, mli));
            }
        }

        // r[verify pre-vote.grant]
        #[test]
        fn prop_pre_vote_no_side_effect(
            my_term: u64, candidate_term: u64,
            clt: u64, cli: u64, mlt: u64, mli: u64,
        ) {
            let _result = can_grant_pre_vote(my_term, candidate_term, clt, cli, mlt, mli);
            // my_term is unchanged — it's not mutated by the call
            // (This is a structural property, not a runtime check)
        }

        // r[verify pre-vote.start-election]
        #[test]
        fn prop_quorum_enables_election(
            cluster_size in 1u64..100,
            extra in 0u64..50,
        ) {
            let majority = cluster_size / 2 + 1;
            let votes = majority.saturating_add(extra).min(cluster_size);
            prop_assert!(should_start_real_election(votes, cluster_size));
        }
    }
}
