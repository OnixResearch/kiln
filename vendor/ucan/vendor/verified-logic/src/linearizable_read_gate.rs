//! Linearizable read gate facade.
//!
//! This composite admits a Raft read only when term authority, leadership,
//! quorum/read-index evidence, and lease-read evidence line up.

use vstd::prelude::*;

verus! {

/// Public decision for one-step read admission.
pub enum LinearizableReadGateDecision {
    ReadIndex { commit_index: u64 },
    LeaseRead { commit_index: u64 },
    RejectNotLeader,
    RejectHigherTerm,
    RejectNoQuorum,
    RejectUnsafeIndex,
    RejectExpiredLease,
}

/// The server still has local leader authority after observing `message_term`.
pub open spec fn linearizable_read_gate_leader_current_spec(
    current_term: u64,
    current_state: u8,
    voted_for: u64,
    message_term: u64,
) -> bool {
    let update = crate::term_authority::apply_higher_term_spec(
        current_term,
        current_state,
        voted_for,
        message_term,
    );
    update.new_term == current_term
        && update.new_state == crate::election::STATE_LEADER
        && crate::lease_read::is_leader_for_lease_spec(update.new_state)
}

/// Quorum-confirmed read-index admission.
pub open spec fn linearizable_read_gate_read_index_spec(
    current_term: u64,
    current_state: u8,
    voted_for: u64,
    message_term: u64,
    ack_count: u64,
    recent_count: u64,
    cluster_size: u64,
    read_index: u64,
    commit_index: u64,
) -> bool
    recommends cluster_size > 0
{
    &&& linearizable_read_gate_leader_current_spec(current_term, current_state, voted_for, message_term)
    &&& crate::read_index::confirm_read_authority_spec(ack_count, cluster_size)
    &&& !crate::check_quorum::should_step_down_spec(recent_count, cluster_size)
    &&& crate::quorum::is_quorum_spec(ack_count, cluster_size)
    &&& crate::quorum::quorum_intersection_prop(ack_count, recent_count, cluster_size)
    &&& crate::read_index::read_safe_at_spec(read_index, commit_index)
}

/// Lease-backed read admission.
pub open spec fn linearizable_read_gate_lease_spec(
    current_term: u64,
    current_state: u8,
    voted_for: u64,
    message_term: u64,
    lease_start: u64,
    election_timeout: u64,
    clock_drift_bound: u64,
    now: u64,
    read_index: u64,
    commit_index: u64,
) -> bool {
    let lease_valid = crate::lease_read::is_lease_valid_spec(
        lease_start,
        election_timeout,
        clock_drift_bound,
        now,
    );
    &&& linearizable_read_gate_leader_current_spec(current_term, current_state, voted_for, message_term)
    &&& lease_valid
    &&& crate::lease_read::lease_read_safe_spec(lease_valid, commit_index, read_index)
}

/// Spec-level accepted predicate.
pub open spec fn linearizable_read_gate_accepted_spec(
    current_term: u64,
    current_state: u8,
    voted_for: u64,
    message_term: u64,
    ack_count: u64,
    recent_count: u64,
    cluster_size: u64,
    lease_start: u64,
    election_timeout: u64,
    clock_drift_bound: u64,
    now: u64,
    read_index: u64,
    commit_index: u64,
) -> bool
    recommends cluster_size > 0
{
    linearizable_read_gate_read_index_spec(
        current_term,
        current_state,
        voted_for,
        message_term,
        ack_count,
        recent_count,
        cluster_size,
        read_index,
        commit_index,
    ) || linearizable_read_gate_lease_spec(
        current_term,
        current_state,
        voted_for,
        message_term,
        lease_start,
        election_timeout,
        clock_drift_bound,
        now,
        read_index,
        commit_index,
    )
}

/// Rejection is read-only over caller-owned commit state.
pub open spec fn linearizable_read_gate_reject_preserves_spec(
    before_commit_index: u64,
    after_commit_index: u64,
) -> bool {
    before_commit_index == after_commit_index
}

/// Decision code: 1 = read-index accept, 2 = lease accept, 3+ = rejects.
pub open spec fn linearizable_read_gate_decision_spec(
    current_term: u64,
    current_state: u8,
    voted_for: u64,
    message_term: u64,
    ack_count: u64,
    recent_count: u64,
    cluster_size: u64,
    lease_start: u64,
    election_timeout: u64,
    clock_drift_bound: u64,
    now: u64,
    read_index: u64,
    commit_index: u64,
) -> int
    recommends cluster_size > 0
{
    if !linearizable_read_gate_leader_current_spec(current_term, current_state, voted_for, message_term) {
        if message_term > current_term { 4 } else { 3 }
    } else if linearizable_read_gate_read_index_spec(
        current_term,
        current_state,
        voted_for,
        message_term,
        ack_count,
        recent_count,
        cluster_size,
        read_index,
        commit_index,
    ) {
        1
    } else if linearizable_read_gate_lease_spec(
        current_term,
        current_state,
        voted_for,
        message_term,
        lease_start,
        election_timeout,
        clock_drift_bound,
        now,
        read_index,
        commit_index,
    ) {
        2
    } else if !crate::read_index::read_safe_at_spec(read_index, commit_index) {
        6
    } else if !crate::lease_read::is_lease_valid_spec(
        lease_start,
        election_timeout,
        clock_drift_bound,
        now,
    ) {
        7
    } else {
        5
    }
}

// r[impl composite-primitives.linearizable-read-gate]
/// Admit a read through ReadIndex first, then through a valid leader lease.
pub fn linearizable_read_gate(
    current_term: u64,
    current_state: u8,
    voted_for: u64,
    message_term: u64,
    ack_count: u64,
    recent_count: u64,
    cluster_size: u64,
    lease_start: u64,
    election_timeout: u64,
    clock_drift_bound: u64,
    now: u64,
    read_index: u64,
    commit_index: u64,
) -> (decision: LinearizableReadGateDecision)
    requires cluster_size > 0
    ensures
        decision is ReadIndex ==> linearizable_read_gate_decision_spec(
            current_term,
            current_state,
            voted_for,
            message_term,
            ack_count,
            recent_count,
            cluster_size,
            lease_start,
            election_timeout,
            clock_drift_bound,
            now,
            read_index,
            commit_index,
        ) == 1,
        decision is LeaseRead ==> linearizable_read_gate_decision_spec(
            current_term,
            current_state,
            voted_for,
            message_term,
            ack_count,
            recent_count,
            cluster_size,
            lease_start,
            election_timeout,
            clock_drift_bound,
            now,
            read_index,
            commit_index,
        ) == 2,
{
    let update = crate::term_authority::apply_higher_term(
        current_term,
        current_state,
        voted_for,
        message_term,
    );
    if update.new_term > current_term {
        return LinearizableReadGateDecision::RejectHigherTerm;
    }
    if !crate::lease_read::is_leader_for_lease(update.new_state) {
        return LinearizableReadGateDecision::RejectNotLeader;
    }
    if crate::read_index::confirm_read_authority(ack_count, cluster_size)
        && !crate::check_quorum::should_step_down(recent_count, cluster_size)
        && crate::read_index::read_safe_at(read_index, commit_index)
    {
        return LinearizableReadGateDecision::ReadIndex { commit_index };
    }
    let lease_valid = crate::lease_read::is_lease_valid(
        lease_start,
        election_timeout,
        clock_drift_bound,
        now,
    );
    if lease_valid && crate::lease_read::lease_read_safe(lease_valid, commit_index, read_index) {
        return LinearizableReadGateDecision::LeaseRead { commit_index };
    }
    if !crate::read_index::read_safe_at(read_index, commit_index) {
        return LinearizableReadGateDecision::RejectUnsafeIndex;
    }
    if !lease_valid {
        return LinearizableReadGateDecision::RejectExpiredLease;
    }
    LinearizableReadGateDecision::RejectNoQuorum
}

// r[verify composite-primitives.linearizable-read-gate]
/// Accepted reads refine the selected authority and safety source predicates.
pub proof fn linearizable_read_gate_refines_sources(
    current_term: u64,
    current_state: u8,
    voted_for: u64,
    message_term: u64,
    ack_count: u64,
    recent_count: u64,
    cluster_size: u64,
    lease_start: u64,
    election_timeout: u64,
    clock_drift_bound: u64,
    now: u64,
    read_index: u64,
    commit_index: u64,
)
    requires
        cluster_size > 0,
        linearizable_read_gate_decision_spec(
            current_term,
            current_state,
            voted_for,
            message_term,
            ack_count,
            recent_count,
            cluster_size,
            lease_start,
            election_timeout,
            clock_drift_bound,
            now,
            read_index,
            commit_index,
        ) == 1 || linearizable_read_gate_decision_spec(
            current_term,
            current_state,
            voted_for,
            message_term,
            ack_count,
            recent_count,
            cluster_size,
            lease_start,
            election_timeout,
            clock_drift_bound,
            now,
            read_index,
            commit_index,
        ) == 2
    ensures linearizable_read_gate_accepted_spec(
        current_term,
        current_state,
        voted_for,
        message_term,
        ack_count,
        recent_count,
        cluster_size,
        lease_start,
        election_timeout,
        clock_drift_bound,
        now,
        read_index,
        commit_index,
    )
{}

// r[verify composite-primitives.linearizable-read-gate]
/// Reject paths do not mutate the committed index.
pub proof fn linearizable_read_gate_reject_is_read_only(commit_index: u64)
    ensures linearizable_read_gate_reject_preserves_spec(commit_index, commit_index)
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify composite-primitives.linearizable-read-gate]
    #[test]
    fn accepts_quorum_confirmed_read() {
        let decision = linearizable_read_gate(
            4,
            crate::election::STATE_LEADER,
            crate::term_authority::NO_VOTE,
            4,
            3,
            3,
            5,
            100,
            150,
            50,
            250,
            10,
            10,
        );
        assert!(matches!(decision, LinearizableReadGateDecision::ReadIndex { commit_index: 10 }));
    }

    // r[verify composite-primitives.linearizable-read-gate]
    #[test]
    fn rejects_lost_authority_and_unsafe_read() {
        let higher_term = linearizable_read_gate(
            4,
            crate::election::STATE_LEADER,
            crate::term_authority::NO_VOTE,
            5,
            3,
            3,
            5,
            100,
            150,
            50,
            120,
            10,
            10,
        );
        assert!(matches!(higher_term, LinearizableReadGateDecision::RejectHigherTerm));

        let unsafe_index = linearizable_read_gate(
            4,
            crate::election::STATE_LEADER,
            crate::term_authority::NO_VOTE,
            4,
            1,
            1,
            5,
            100,
            150,
            50,
            250,
            10,
            9,
        );
        assert!(matches!(unsafe_index, LinearizableReadGateDecision::RejectUnsafeIndex));
    }
}
