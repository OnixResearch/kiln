//! Verified witness replica primitives for Raft.
//!
//! A witness is a lightweight cluster member that votes in elections
//! and leader confirmation but does NOT store the full log. Witnesses
//! only retain recent entries (a sliding window) plus snapshot metadata.
//! This reduces storage costs while maintaining quorum properties.
//!
//! Witness properties:
//! - Participates in vote granting (has epoch information)
//! - Participates in heartbeat acknowledgments (leader confirmation)
//! - Does NOT participate in log replication (no matchIndex tracking)
//! - Receives snapshot metadata but not snapshot data
//! - Cannot be elected leader (log is incomplete)

use vstd::prelude::*;

verus! {

/// Full voter: stores complete log, can be elected leader.
pub const ROLE_VOTER: u8 = 0;
/// Witness: votes but stores partial log, cannot be elected leader.
pub const ROLE_WITNESS: u8 = 1;
/// Learner: receives log but does not vote.
pub const ROLE_LEARNER: u8 = 2;

// --- is_valid_role ---

// r[depends witness.valid-role]
/// Spec: is this a valid node role?
pub open spec fn is_valid_role_spec(role: u8) -> bool {
    role == ROLE_VOTER || role == ROLE_WITNESS || role == ROLE_LEARNER
}

// r[impl witness.valid-role]
/// Check role validity.
pub fn is_valid_role(role: u8) -> (result: bool)
    ensures result == is_valid_role_spec(role)
{
    role == ROLE_VOTER || role == ROLE_WITNESS || role == ROLE_LEARNER
}

// --- can_vote ---

// r[depends witness.can-vote]
/// Spec: can this role participate in elections?
///
/// Both voters and witnesses can vote. Learners cannot.
pub open spec fn can_vote_spec(role: u8) -> bool {
    role == ROLE_VOTER || role == ROLE_WITNESS
}

// r[impl witness.can-vote]
/// Check if the role allows voting.
pub fn can_vote(role: u8) -> (result: bool)
    ensures result == can_vote_spec(role)
{
    role == ROLE_VOTER || role == ROLE_WITNESS
}

// --- can_be_leader ---

// r[depends witness.can-be-leader]
/// Spec: can this role be elected leader?
///
/// Only full voters can be leaders. Witnesses have incomplete logs and
/// cannot serve as leaders. Learners don't participate in elections.
pub open spec fn can_be_leader_spec(role: u8) -> bool {
    role == ROLE_VOTER
}

// r[impl witness.can-be-leader]
/// Check if the role allows leadership.
pub fn can_be_leader(role: u8) -> (result: bool)
    ensures result == can_be_leader_spec(role)
{
    role == ROLE_VOTER
}

// --- receives_log ---

// r[depends witness.receives-log]
/// Spec: does this role receive full log replication?
///
/// Voters and learners receive the full log. Witnesses only receive
/// recent entries within their retention window.
pub open spec fn receives_full_log_spec(role: u8) -> bool {
    role == ROLE_VOTER || role == ROLE_LEARNER
}

// r[impl witness.receives-log]
/// Check if the role receives full log replication.
pub fn receives_full_log(role: u8) -> (result: bool)
    ensures result == receives_full_log_spec(role)
{
    role == ROLE_VOTER || role == ROLE_LEARNER
}

// --- witness_can_ack_heartbeat ---

// r[depends witness.can-ack-heartbeat]
/// Spec: can a witness acknowledge a leader heartbeat?
///
/// Witnesses always acknowledge heartbeats from the current leader.
/// This maintains the leader's quorum contact for check_quorum and
/// read_index protocols.
pub open spec fn can_ack_heartbeat_spec(
    role: u8,
    leader_term: u64,
    my_term: u64,
) -> bool {
    (role == ROLE_VOTER || role == ROLE_WITNESS) && leader_term >= my_term
}

// r[impl witness.can-ack-heartbeat]
/// Check if this node should acknowledge a heartbeat.
pub fn can_ack_heartbeat(
    role: u8,
    leader_term: u64,
    my_term: u64,
) -> (result: bool)
    ensures result == can_ack_heartbeat_spec(role, leader_term, my_term)
{
    (role == ROLE_VOTER || role == ROLE_WITNESS) && leader_term >= my_term
}

// --- witness_entry_retained ---

// r[depends witness.entry-retained]
/// Spec: is a log entry within the witness's retention window?
///
/// A witness keeps entries from `snapshot_index` through the log tip.
/// Entries before the snapshot_index have been summarized in the
/// snapshot metadata and are no longer available.
pub open spec fn entry_retained_spec(
    entry_index: u64,
    snapshot_index: u64,
) -> bool {
    entry_index >= snapshot_index
}

// r[impl witness.entry-retained]
/// Check if an entry is in the witness's retention window.
pub fn entry_retained(
    entry_index: u64,
    snapshot_index: u64,
) -> (result: bool)
    ensures result == entry_retained_spec(entry_index, snapshot_index)
{
    entry_index >= snapshot_index
}

// --- quorum_with_witnesses ---

// r[depends witness.quorum-with-witnesses]
/// Spec: do voters + witnesses form a quorum?
///
/// For quorum purposes, witnesses count the same as voters. A quorum
/// of (voters + witnesses) is required for elections and commit
/// advancement. Learners do not count toward quorum.
pub open spec fn quorum_members_spec(
    voter_count: u64,
    witness_count: u64,
) -> u64 {
    (voter_count as int + witness_count as int) as u64
}

// r[impl witness.quorum-with-witnesses]
/// Count quorum-eligible members (voters + witnesses).
pub fn quorum_members(
    voter_count: u64,
    witness_count: u64,
) -> (result: u64)
    requires voter_count as int + witness_count as int <= u64::MAX as int
    ensures result == quorum_members_spec(voter_count, witness_count)
{
    voter_count + witness_count
}

// --- should_reject_candidate ---

// r[depends witness.reject-witness-candidate]
/// Spec: should a vote request from a witness be rejected?
///
/// Witnesses cannot be elected leader. Vote requests where the
/// candidate is a witness must be rejected regardless of log freshness.
pub open spec fn should_reject_candidate_spec(candidate_role: u8) -> bool {
    candidate_role == ROLE_WITNESS || candidate_role == ROLE_LEARNER
}

// r[impl witness.reject-witness-candidate]
/// Check if a candidate should be rejected based on role.
pub fn should_reject_candidate(candidate_role: u8) -> (result: bool)
    ensures result == should_reject_candidate_spec(candidate_role)
{
    candidate_role == ROLE_WITNESS || candidate_role == ROLE_LEARNER
}

// --- Safety proofs ---

// r[verify witness.valid-role]
/// All role constants are valid.
proof fn all_constants_valid()
    ensures
        is_valid_role_spec(ROLE_VOTER),
        is_valid_role_spec(ROLE_WITNESS),
        is_valid_role_spec(ROLE_LEARNER),
{}

// r[verify witness.can-vote]
/// Voters can vote.
proof fn voter_can_vote()
    ensures can_vote_spec(ROLE_VOTER)
{}

// r[verify witness.can-vote]
/// Witnesses can vote.
proof fn witness_can_vote()
    ensures can_vote_spec(ROLE_WITNESS)
{}

// r[verify witness.can-vote]
/// Learners cannot vote.
proof fn learner_cannot_vote()
    ensures !can_vote_spec(ROLE_LEARNER)
{}

// r[verify witness.can-be-leader]
/// Only voters can be leader.
proof fn only_voter_leads()
    ensures
        can_be_leader_spec(ROLE_VOTER),
        !can_be_leader_spec(ROLE_WITNESS),
        !can_be_leader_spec(ROLE_LEARNER),
{}

// r[verify witness.can-be-leader]
/// Leader implies voter.
proof fn leader_implies_voter(role: u8)
    requires can_be_leader_spec(role)
    ensures can_vote_spec(role)
{}

// r[verify witness.can-be-leader]
/// Leader implies full log.
proof fn leader_implies_full_log(role: u8)
    requires can_be_leader_spec(role)
    ensures receives_full_log_spec(role)
{}

// r[verify witness.reject-witness-candidate]
/// Witness candidates are rejected.
proof fn witness_rejected_as_candidate()
    ensures should_reject_candidate_spec(ROLE_WITNESS)
{}

// r[verify witness.reject-witness-candidate]
/// Voter candidates are not rejected.
proof fn voter_not_rejected()
    ensures !should_reject_candidate_spec(ROLE_VOTER)
{}

// r[verify witness.can-ack-heartbeat]
/// Witnesses ack heartbeats from current leader.
proof fn witness_acks_current_leader(leader_term: u64)
    ensures can_ack_heartbeat_spec(ROLE_WITNESS, leader_term, leader_term)
{}

// r[verify witness.can-ack-heartbeat]
/// Stale heartbeats are not acked.
proof fn stale_heartbeat_not_acked(role: u8, leader_term: u64, my_term: u64)
    requires leader_term < my_term
    ensures !can_ack_heartbeat_spec(role, leader_term, my_term)
{}

// r[verify witness.quorum-with-witnesses]
/// Witnesses contribute to quorum count.
proof fn witnesses_count_for_quorum(voter_count: u64, witness_count: u64)
    requires
        witness_count > 0,
        voter_count as int + witness_count as int <= u64::MAX as int,
    ensures quorum_members_spec(voter_count, witness_count) > voter_count
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify witness.valid-role]
    #[test]
    fn test_is_valid_role() {
        assert!(is_valid_role(ROLE_VOTER));
        assert!(is_valid_role(ROLE_WITNESS));
        assert!(is_valid_role(ROLE_LEARNER));
        assert!(!is_valid_role(3));
    }

    // r[verify witness.can-vote]
    #[test]
    fn test_can_vote() {
        assert!(can_vote(ROLE_VOTER));
        assert!(can_vote(ROLE_WITNESS));
        assert!(!can_vote(ROLE_LEARNER));
    }

    // r[verify witness.can-be-leader]
    #[test]
    fn test_can_be_leader() {
        assert!(can_be_leader(ROLE_VOTER));
        assert!(!can_be_leader(ROLE_WITNESS));
        assert!(!can_be_leader(ROLE_LEARNER));
    }

    // r[verify witness.receives-log]
    #[test]
    fn test_receives_full_log() {
        assert!(receives_full_log(ROLE_VOTER));
        assert!(!receives_full_log(ROLE_WITNESS));
        assert!(receives_full_log(ROLE_LEARNER));
    }

    // r[verify witness.can-ack-heartbeat]
    #[test]
    fn test_can_ack_heartbeat() {
        assert!(can_ack_heartbeat(ROLE_VOTER, 5, 5));
        assert!(can_ack_heartbeat(ROLE_WITNESS, 5, 5));
        assert!(can_ack_heartbeat(ROLE_WITNESS, 6, 5));
        assert!(!can_ack_heartbeat(ROLE_WITNESS, 4, 5));
        assert!(!can_ack_heartbeat(ROLE_LEARNER, 5, 5));
    }

    // r[verify witness.entry-retained]
    #[test]
    fn test_entry_retained() {
        assert!(entry_retained(10, 5));
        assert!(entry_retained(5, 5));
        assert!(!entry_retained(3, 5));
    }

    // r[verify witness.quorum-with-witnesses]
    #[test]
    fn test_quorum_members() {
        assert_eq!(quorum_members(3, 2), 5);
        assert_eq!(quorum_members(5, 0), 5);
        assert_eq!(quorum_members(0, 3), 3);
    }

    // r[verify witness.reject-witness-candidate]
    #[test]
    fn test_should_reject_candidate() {
        assert!(!should_reject_candidate(ROLE_VOTER));
        assert!(should_reject_candidate(ROLE_WITNESS));
        assert!(should_reject_candidate(ROLE_LEARNER));
    }

    proptest! {
        // r[verify witness.can-be-leader]
        #[test]
        fn prop_leader_implies_voter(role in 0u8..3) {
            if can_be_leader(role) {
                prop_assert!(can_vote(role));
            }
        }

        // r[verify witness.can-be-leader]
        #[test]
        fn prop_leader_implies_full_log(role in 0u8..3) {
            if can_be_leader(role) {
                prop_assert!(receives_full_log(role));
            }
        }

        // r[verify witness.quorum-with-witnesses]
        #[test]
        fn prop_witnesses_count(
            voters in 0u64..100,
            witnesses in 1u64..100,
        ) {
            let total = quorum_members(voters, witnesses);
            prop_assert!(total > voters);
        }

        // r[verify witness.can-ack-heartbeat]
        #[test]
        fn prop_stale_heartbeat_rejected(
            role in 0u8..3,
            leader_term in 0u64..1000,
            delta in 1u64..1000,
        ) {
            let my_term = leader_term.saturating_add(delta);
            if my_term > leader_term {
                prop_assert!(!can_ack_heartbeat(role, leader_term, my_term));
            }
        }
    }
}
