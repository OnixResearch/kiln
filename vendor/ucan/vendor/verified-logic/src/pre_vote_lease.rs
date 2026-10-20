//! Verified PreVote + CheckQuorum interaction for Raft.
//!
//! When CheckQuorum is active, a follower that has recently heard from
//! the leader should reject even pre-vote requests. This prevents a
//! partitioned node (that regains partial connectivity) from disrupting
//! the cluster by triggering unnecessary elections.
//!
//! The interaction rule: a pre-vote is rejected if the voter has received
//! a heartbeat from the current leader within the election timeout, UNLESS
//! the pre-vote candidate's term is strictly higher than the voter's term
//! (indicating the voter's leader may actually be stale).
//!
//! This module composes `pre_vote` and `check_quorum` logic into a single
//! decision primitive.

use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::epoch::Epoch;
#[cfg(verus_keep_ghost)]
use crate::epoch::epoch_ge_spec;

verus! {

// --- should_reject_pre_vote ---

// r[depends pre-vote-lease.reject]
/// Spec: should this voter reject a pre-vote request due to an active leader?
///
/// Reject when:
/// 1. The voter has heard from the leader recently (within election timeout).
/// 2. The candidate's term is NOT strictly higher than the voter's term.
///
/// If the candidate's term IS strictly higher, the voter's leader might
/// be stale (partitioned from the candidate's partition), so the pre-vote
/// should be evaluated normally.
pub open spec fn should_reject_pre_vote_spec(
    has_leader_contact: bool,
    candidate_term: u64,
    my_term: u64,
) -> bool {
    has_leader_contact && candidate_term <= my_term
}

// r[impl pre-vote-lease.reject]
/// Check if a pre-vote should be rejected due to active leader.
pub fn should_reject_pre_vote(
    has_leader_contact: bool,
    candidate_term: u64,
    my_term: u64,
) -> (result: bool)
    ensures result == should_reject_pre_vote_spec(has_leader_contact, candidate_term, my_term)
{
    has_leader_contact && candidate_term <= my_term
}

// --- combined_pre_vote_decision ---

// r[depends pre-vote-lease.combined-decision]
/// Spec: full pre-vote grant decision combining PreVote + CheckQuorum.
///
/// Returns true (grant) only when:
/// 1. NOT rejected by CheckQuorum (no active leader or candidate has higher term)
/// 2. Candidate's term >= voter's term
/// 3. Candidate's log is at least as up-to-date as voter's log
///
/// This is the single-function composition of pre_vote::can_grant_pre_vote
/// and the CheckQuorum rejection filter.
pub open spec fn combined_pre_vote_decision_spec(
    has_leader_contact: bool,
    candidate_term: u64,
    my_term: u64,
    candidate_last_term: u64,
    candidate_last_index: u64,
    my_last_term: u64,
    my_last_index: u64,
) -> bool {
    // Not rejected by CheckQuorum
    !should_reject_pre_vote_spec(has_leader_contact, candidate_term, my_term)
    // Standard pre-vote check: term >= ours and log at least as fresh
    && candidate_term >= my_term
    && epoch_ge_spec(
        Epoch { term: candidate_last_term, index: candidate_last_index },
        Epoch { term: my_last_term, index: my_last_index },
    )
}

// r[impl pre-vote-lease.combined-decision]
/// Full pre-vote decision with CheckQuorum integration.
pub fn combined_pre_vote_decision(
    has_leader_contact: bool,
    candidate_term: u64,
    my_term: u64,
    candidate_last_term: u64,
    candidate_last_index: u64,
    my_last_term: u64,
    my_last_index: u64,
) -> (result: bool)
    ensures result == combined_pre_vote_decision_spec(
        has_leader_contact, candidate_term, my_term,
        candidate_last_term, candidate_last_index,
        my_last_term, my_last_index,
    )
{
    !should_reject_pre_vote(has_leader_contact, candidate_term, my_term)
    && candidate_term >= my_term
    && crate::epoch::epoch_ge(
        &crate::epoch::Epoch { term: candidate_last_term, index: candidate_last_index },
        &crate::epoch::Epoch { term: my_last_term, index: my_last_index },
    )
}

// --- has_leader_within_timeout ---

// r[depends pre-vote-lease.leader-contact]
/// Spec: has the voter heard from the leader within the election timeout?
///
/// This is the predicate that enables the CheckQuorum pre-vote rejection.
/// It checks whether `now - last_leader_contact < election_timeout`.
pub open spec fn has_leader_within_timeout_spec(
    last_contact: u64,
    now: u64,
    election_timeout: u64,
) -> bool {
    if now >= last_contact {
        now - last_contact < election_timeout
    } else {
        true  // clock wrapped or not yet initialized: assume contact
    }
}

// r[impl pre-vote-lease.leader-contact]
/// Check if the leader was contacted within the election timeout.
pub fn has_leader_within_timeout(
    last_contact: u64,
    now: u64,
    election_timeout: u64,
) -> (result: bool)
    ensures result == has_leader_within_timeout_spec(last_contact, now, election_timeout)
{
    if now >= last_contact {
        now - last_contact < election_timeout
    } else {
        true
    }
}

// --- Safety proofs ---

// r[verify pre-vote-lease.reject]
/// No leader contact means no CheckQuorum rejection.
proof fn no_contact_no_rejection(candidate_term: u64, my_term: u64)
    ensures !should_reject_pre_vote_spec(false, candidate_term, my_term)
{}

// r[verify pre-vote-lease.reject]
/// Higher candidate term bypasses CheckQuorum.
proof fn higher_term_bypasses(candidate_term: u64, my_term: u64)
    requires candidate_term > my_term
    ensures !should_reject_pre_vote_spec(true, candidate_term, my_term)
{}

// r[verify pre-vote-lease.reject]
/// Same-term candidate with leader contact is rejected.
proof fn same_term_with_contact_rejected(term: u64)
    ensures should_reject_pre_vote_spec(true, term, term)
{}

// r[verify pre-vote-lease.combined-decision]
/// CheckQuorum rejection prevents pre-vote grant.
proof fn rejection_prevents_grant(
    candidate_term: u64,
    my_term: u64,
    clt: u64, cli: u64, mlt: u64, mli: u64,
)
    requires should_reject_pre_vote_spec(true, candidate_term, my_term)
    ensures !combined_pre_vote_decision_spec(
        true, candidate_term, my_term, clt, cli, mlt, mli,
    )
{}

// r[verify pre-vote-lease.combined-decision]
/// Without CheckQuorum, combined decision reduces to standard pre-vote.
proof fn without_check_quorum_is_standard(
    candidate_term: u64,
    my_term: u64,
    clt: u64, cli: u64, mlt: u64, mli: u64,
)
    ensures combined_pre_vote_decision_spec(
        false, candidate_term, my_term, clt, cli, mlt, mli,
    ) == (
        candidate_term >= my_term
        && epoch_ge_spec(
            Epoch { term: clt, index: cli },
            Epoch { term: mlt, index: mli },
        )
    )
{}

// r[verify pre-vote-lease.leader-contact]
/// Zero elapsed time means leader contact (if timeout > 0).
proof fn zero_elapsed_is_contact(last_contact: u64, election_timeout: u64)
    requires election_timeout > 0
    ensures has_leader_within_timeout_spec(last_contact, last_contact, election_timeout)
{}

// r[verify pre-vote-lease.leader-contact]
/// Elapsed >= timeout means no contact.
proof fn timeout_elapsed_no_contact(
    last_contact: u64,
    now: u64,
    election_timeout: u64,
)
    requires
        now >= last_contact,
        now - last_contact >= election_timeout,
    ensures !has_leader_within_timeout_spec(last_contact, now, election_timeout)
{}

// r[verify pre-vote-lease.reject]
/// Partitioned node with same term is blocked.
proof fn partitioned_node_blocked(my_term: u64, clt: u64, cli: u64, mlt: u64, mli: u64)
    ensures !combined_pre_vote_decision_spec(
        true, my_term, my_term, clt, cli, mlt, mli,
    )
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify pre-vote-lease.reject]
    #[test]
    fn test_should_reject_pre_vote() {
        assert!(should_reject_pre_vote(true, 3, 3)); // same term, has contact
        assert!(should_reject_pre_vote(true, 2, 3)); // lower term
        assert!(!should_reject_pre_vote(true, 4, 3)); // higher term bypasses
        assert!(!should_reject_pre_vote(false, 3, 3)); // no contact
    }

    // r[verify pre-vote-lease.combined-decision]
    #[test]
    fn test_combined_pre_vote_decision() {
        // Granted: no leader contact, valid pre-vote
        assert!(combined_pre_vote_decision(false, 4, 3, 4, 10, 3, 5));
        // Rejected: leader contact + same term
        assert!(!combined_pre_vote_decision(true, 3, 3, 3, 10, 3, 5));
        // Granted: leader contact + higher term (bypasses CheckQuorum)
        assert!(combined_pre_vote_decision(true, 4, 3, 4, 10, 3, 5));
        // Rejected: stale candidate log
        assert!(!combined_pre_vote_decision(false, 4, 3, 2, 10, 3, 5));
    }

    // r[verify pre-vote-lease.leader-contact]
    #[test]
    fn test_has_leader_within_timeout() {
        assert!(has_leader_within_timeout(100, 100, 50)); // just contacted
        assert!(has_leader_within_timeout(100, 140, 50)); // within timeout
        assert!(!has_leader_within_timeout(100, 150, 50)); // at timeout
        assert!(!has_leader_within_timeout(100, 200, 50)); // past timeout
    }

    proptest! {
        // r[verify pre-vote-lease.reject]
        #[test]
        fn prop_no_contact_no_rejection(candidate_term: u64, my_term: u64) {
            prop_assert!(!should_reject_pre_vote(false, candidate_term, my_term));
        }

        // r[verify pre-vote-lease.reject]
        #[test]
        fn prop_higher_term_bypasses(
            my_term in 0u64..u64::MAX - 1,
            delta in 1u64..1000,
        ) {
            let candidate_term = my_term.saturating_add(delta);
            if candidate_term > my_term {
                prop_assert!(!should_reject_pre_vote(true, candidate_term, my_term));
            }
        }

        // r[verify pre-vote-lease.leader-contact]
        #[test]
        fn prop_zero_elapsed_is_contact(
            last_contact in 0u64..u64::MAX,
            election_timeout in 1u64..u64::MAX,
        ) {
            prop_assert!(has_leader_within_timeout(last_contact, last_contact, election_timeout));
        }

        // r[verify pre-vote-lease.combined-decision]
        #[test]
        fn prop_partitioned_node_blocked(
            my_term: u64,
            clt: u64, cli: u64, mlt: u64, mli: u64,
        ) {
            // Same term + leader contact → always rejected
            prop_assert!(!combined_pre_vote_decision(true, my_term, my_term, clt, cli, mlt, mli));
        }
    }
}
