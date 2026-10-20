//! Raft membership-manager composite facade.
//!
//! This facade composes the existing Raft membership-change primitives for the
//! OpenSpec `composite-primitives.raft-membership-manager` slice.  It admits a
//! learner promotion only when the promoted voter set is distinct, the learner is
//! caught up, the configuration change can enter joint consensus, joint quorum
//! checks hold, and any requested leadership transfer target is eligible.

use vstd::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RaftMembershipManagerOutcome {
    Accepted {
        new_voters: Vec<u64>,
        transfer_started: bool,
    },
    Rejected {
        voters: Vec<u64>,
        transfer_started: bool,
    },
}

verus! {

// r[impl composite-primitives.raft-membership-manager]
pub open spec fn raft_membership_manager_transfer_safe_spec(
    transfer_requested: bool,
    node_state: u8,
    transfer_target_id: u64,
    self_id: u64,
    transfer_target_match: u64,
    last_log_index: u64,
    transfer_deadline: u64,
    now: u64,
) -> bool {
    !transfer_requested || ({
        &&& crate::leadership_transfer::can_start_transfer_spec(
            node_state,
            transfer_target_id,
            self_id,
        )
        &&& crate::leadership_transfer::is_target_caught_up_spec(
            transfer_target_match,
            last_log_index,
        )
        &&& !crate::leadership_transfer::is_transfer_expired_spec(transfer_deadline, now)
    })
}

// r[impl composite-primitives.raft-membership-manager]
pub open spec fn raft_membership_manager_accepted_spec(
    current_phase: u8,
    old_voters: Seq<u64>,
    learner_id: u64,
    replace_id: Option<u64>,
    learner_match: u64,
    leader_log_len: u64,
    leader_id: u64,
    old_joint_count: u64,
    old_joint_total: u64,
    new_joint_count: u64,
    transfer_requested: bool,
    node_state: u8,
    transfer_target_id: u64,
    self_id: u64,
    transfer_target_match: u64,
    last_log_index: u64,
    transfer_deadline: u64,
    now: u64,
) -> bool {
    let new_voters = crate::voter_set::promote_voters_spec(old_voters, learner_id, replace_id);
    &&& old_voters.len() < u64::MAX as int
    &&& new_voters.len() < u64::MAX as int
    &&& crate::learner_change::can_add_learner_spec(false, true, 0, 0, false)
    &&& crate::learner_change::can_remove_learner_spec(true, false, false)
    &&& crate::learner_change::quorum_unchanged_spec(old_voters.len() as u64, old_voters.len() as u64)
    &&& crate::voter_set::all_distinct_spec(old_voters)
    &&& crate::voter_set::all_distinct_spec(new_voters)
    &&& crate::learner_readiness::is_caught_up_spec(learner_match, leader_log_len)
    &&& crate::learner_readiness::promotion_safe_spec(
        learner_match,
        leader_log_len,
        new_voters.len() as u64,
    )
    &&& crate::config_change::can_start_config_change_spec(
        current_phase,
        new_voters.len() as u64,
    )
    &&& crate::config_change::leader_in_new_config_spec(leader_id, new_voters)
    &&& old_joint_total > 0
    &&& new_voters.len() > 0
    &&& crate::joint_consensus::is_joint_quorum_spec(
        old_joint_count,
        old_joint_total,
        new_joint_count,
        new_voters.len() as u64,
    )
    &&& raft_membership_manager_transfer_safe_spec(
        transfer_requested,
        node_state,
        transfer_target_id,
        self_id,
        transfer_target_match,
        last_log_index,
        transfer_deadline,
        now,
    )
}

// r[impl composite-primitives.raft-membership-manager]
pub open spec fn raft_membership_manager_reject_preserves_spec(
    before_voters: Seq<u64>,
    after_voters: Seq<u64>,
    before_transfer_started: bool,
    after_transfer_started: bool,
) -> bool {
    before_voters == after_voters && before_transfer_started == after_transfer_started
}

// r[impl composite-primitives.raft-membership-manager]
pub proof fn raft_membership_manager_refines_sources(
    current_phase: u8,
    old_voters: Seq<u64>,
    learner_id: u64,
    replace_id: Option<u64>,
    learner_match: u64,
    leader_log_len: u64,
    leader_id: u64,
    old_joint_count: u64,
    old_joint_total: u64,
    new_joint_count: u64,
    transfer_requested: bool,
    node_state: u8,
    transfer_target_id: u64,
    self_id: u64,
    transfer_target_match: u64,
    last_log_index: u64,
    transfer_deadline: u64,
    now: u64,
)
    ensures
        raft_membership_manager_accepted_spec(
            current_phase,
            old_voters,
            learner_id,
            replace_id,
            learner_match,
            leader_log_len,
            leader_id,
            old_joint_count,
            old_joint_total,
            new_joint_count,
            transfer_requested,
            node_state,
            transfer_target_id,
            self_id,
            transfer_target_match,
            last_log_index,
            transfer_deadline,
            now,
        ) ==> ({
            let new_voters = crate::voter_set::promote_voters_spec(old_voters, learner_id, replace_id);
            &&& crate::learner_change::can_add_learner_spec(false, true, 0, 0, false)
            &&& crate::learner_change::can_remove_learner_spec(true, false, false)
            &&& crate::learner_change::quorum_unchanged_spec(old_voters.len() as u64, old_voters.len() as u64)
            &&& crate::voter_set::all_distinct_spec(old_voters)
            &&& crate::voter_set::all_distinct_spec(new_voters)
            &&& crate::learner_readiness::is_caught_up_spec(learner_match, leader_log_len)
            &&& crate::learner_readiness::promotion_safe_spec(
                learner_match,
                leader_log_len,
                new_voters.len() as u64,
            )
            &&& crate::config_change::can_start_config_change_spec(
                current_phase,
                new_voters.len() as u64,
            )
            &&& crate::config_change::leader_in_new_config_spec(leader_id, new_voters)
            &&& crate::joint_consensus::is_joint_quorum_spec(
                old_joint_count,
                old_joint_total,
                new_joint_count,
                new_voters.len() as u64,
            )
            &&& raft_membership_manager_transfer_safe_spec(
                transfer_requested,
                node_state,
                transfer_target_id,
                self_id,
                transfer_target_match,
                last_log_index,
                transfer_deadline,
                now,
            )
        }),
{
}

// r[impl composite-primitives.raft-membership-manager]
pub proof fn raft_membership_manager_reject_refines_sources(
    voters: Seq<u64>,
    transfer_started: bool,
)
    ensures raft_membership_manager_reject_preserves_spec(
        voters,
        voters,
        transfer_started,
        transfer_started,
    ),
{
}

// r[verify composite-primitives.raft-membership-manager]
pub proof fn raft_membership_manager_stale_learner_rejected(
    current_phase: u8,
    old_voters: Seq<u64>,
    learner_id: u64,
    replace_id: Option<u64>,
    learner_match: u64,
    leader_log_len: u64,
    leader_id: u64,
    old_joint_count: u64,
    old_joint_total: u64,
    new_joint_count: u64,
    transfer_requested: bool,
    node_state: u8,
    transfer_target_id: u64,
    self_id: u64,
    transfer_target_match: u64,
    last_log_index: u64,
    transfer_deadline: u64,
    now: u64,
)
    requires learner_match < leader_log_len,
    ensures !raft_membership_manager_accepted_spec(
        current_phase,
        old_voters,
        learner_id,
        replace_id,
        learner_match,
        leader_log_len,
        leader_id,
        old_joint_count,
        old_joint_total,
        new_joint_count,
        transfer_requested,
        node_state,
        transfer_target_id,
        self_id,
        transfer_target_match,
        last_log_index,
        transfer_deadline,
        now,
    ),
{
}

// r[verify composite-primitives.raft-membership-manager]
pub proof fn raft_membership_manager_expired_transfer_rejected(
    current_phase: u8,
    old_voters: Seq<u64>,
    learner_id: u64,
    replace_id: Option<u64>,
    learner_match: u64,
    leader_log_len: u64,
    leader_id: u64,
    old_joint_count: u64,
    old_joint_total: u64,
    new_joint_count: u64,
    node_state: u8,
    transfer_target_id: u64,
    self_id: u64,
    transfer_target_match: u64,
    last_log_index: u64,
    transfer_deadline: u64,
    now: u64,
)
    requires now > transfer_deadline,
    ensures !raft_membership_manager_accepted_spec(
        current_phase,
        old_voters,
        learner_id,
        replace_id,
        learner_match,
        leader_log_len,
        leader_id,
        old_joint_count,
        old_joint_total,
        new_joint_count,
        true,
        node_state,
        transfer_target_id,
        self_id,
        transfer_target_match,
        last_log_index,
        transfer_deadline,
        now,
    ),
{
}

// r[impl composite-primitives.raft-membership-manager]
pub open spec fn raft_membership_manager_outcome_alias_spec(accepted: bool) -> crate::composite_outcome::CompositeOutcomeTag {
    if accepted {
        crate::composite_outcome::CompositeOutcomeTag::Accept
    } else {
        crate::composite_outcome::CompositeOutcomeTag::Reject
    }
}

// r[impl composite-primitives.raft-membership-manager]
pub fn raft_membership_manager_outcome_alias(accepted: bool) -> (outcome: crate::composite_outcome::CompositeOutcomeTag)
    ensures outcome == raft_membership_manager_outcome_alias_spec(accepted)
{
    if accepted {
        crate::composite_outcome::composite_accept()
    } else {
        crate::composite_outcome::composite_reject()
    }
}

} // verus!

// r[impl composite-primitives.raft-membership-manager]
pub fn raft_membership_manager(
    current_phase: u8,
    old_voters: &[u64],
    learner_id: u64,
    replace_id: Option<u64>,
    learner_match: u64,
    leader_log_len: u64,
    leader_id: u64,
    old_joint_count: u64,
    old_joint_total: u64,
    new_joint_count: u64,
    transfer_requested: bool,
    node_state: u8,
    transfer_target_id: u64,
    self_id: u64,
    transfer_target_match: u64,
    last_log_index: u64,
    transfer_deadline: u64,
    now: u64,
) -> RaftMembershipManagerOutcome {
    if !all_distinct_runtime(old_voters) {
        return RaftMembershipManagerOutcome::Rejected {
            voters: old_voters.to_vec(),
            transfer_started: false,
        };
    }

    let new_voters = crate::voter_set::promote_voters(old_voters, learner_id, replace_id);
    if !all_distinct_runtime(&new_voters) {
        return RaftMembershipManagerOutcome::Rejected {
            voters: old_voters.to_vec(),
            transfer_started: false,
        };
    }

    if !crate::learner_readiness::promotion_safe(learner_match, leader_log_len, new_voters.len() as u64) {
        return RaftMembershipManagerOutcome::Rejected {
            voters: old_voters.to_vec(),
            transfer_started: false,
        };
    }

    if !crate::config_change::can_start_config_change(current_phase, new_voters.len() as u64)
        || !crate::config_change::leader_in_new_config(leader_id, &new_voters)
        || old_joint_total == 0
        || !crate::joint_consensus::is_joint_quorum(
            old_joint_count,
            old_joint_total,
            new_joint_count,
            new_voters.len() as u64,
        )
    {
        return RaftMembershipManagerOutcome::Rejected {
            voters: old_voters.to_vec(),
            transfer_started: false,
        };
    }

    if transfer_requested
        && (!crate::leadership_transfer::can_start_transfer(node_state, transfer_target_id, self_id)
            || !crate::leadership_transfer::is_target_caught_up(transfer_target_match, last_log_index)
            || crate::leadership_transfer::is_transfer_expired(transfer_deadline, now))
    {
        return RaftMembershipManagerOutcome::Rejected {
            voters: old_voters.to_vec(),
            transfer_started: false,
        };
    }

    RaftMembershipManagerOutcome::Accepted {
        new_voters,
        transfer_started: transfer_requested,
    }
}

fn all_distinct_runtime(xs: &[u64]) -> bool {
    for i in 0..xs.len() {
        for j in (i + 1)..xs.len() {
            if xs[i] == xs[j] {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learner_promotion_through_joint_consensus_is_safe() {
        let voters = vec![1, 2, 3];
        let outcome = raft_membership_manager(
            crate::config_change::PHASE_STABLE,
            &voters,
            4,
            None,
            10,
            10,
            1,
            2,
            3,
            3,
            false,
            crate::election::STATE_LEADER,
            2,
            1,
            10,
            10,
            100,
            50,
        );
        assert_eq!(outcome, RaftMembershipManagerOutcome::Accepted {
            new_voters: vec![1, 2, 3, 4],
            transfer_started: false,
        });
    }

    #[test]
    fn stale_learner_is_rejected_without_mutating_voters() {
        let voters = vec![1, 2, 3];
        let outcome = raft_membership_manager(
            crate::config_change::PHASE_STABLE,
            &voters,
            4,
            None,
            9,
            10,
            1,
            2,
            3,
            3,
            false,
            crate::election::STATE_LEADER,
            2,
            1,
            10,
            10,
            100,
            50,
        );
        assert_eq!(outcome, RaftMembershipManagerOutcome::Rejected {
            voters,
            transfer_started: false,
        });
    }

    #[test]
    fn unsafe_transfer_is_rejected_without_mutating_voters() {
        let voters = vec![1, 2, 3];
        let outcome = raft_membership_manager(
            crate::config_change::PHASE_STABLE,
            &voters,
            4,
            None,
            10,
            10,
            1,
            2,
            3,
            3,
            true,
            crate::election::STATE_LEADER,
            1,
            1,
            10,
            10,
            100,
            50,
        );
        assert_eq!(outcome, RaftMembershipManagerOutcome::Rejected {
            voters,
            transfer_started: false,
        });
    }
}
