//! Deterministic QuePaxa proposal ordering and leader-priority admission.

use vstd::prelude::*;

use crate::quepaxa::model::*;

verus! {

// r[impl consensus.quepaxa.proposal-order]
pub open spec fn nil_proposal_spec() -> QuePaxaProposal {
    QuePaxaProposal {
        priority: QUEPAXA_NIL_PRIORITY,
        proposer_id: QUEPAXA_NIL_PROPOSER,
        value_id: QUEPAXA_NIL_VALUE,
    }
}

// r[impl consensus.quepaxa.proposal-order]
pub open spec fn proposal_eq_key_spec(left: QuePaxaProposal, right: QuePaxaProposal) -> bool {
    left.priority == right.priority
        && left.proposer_id == right.proposer_id
        && left.value_id == right.value_id
}

// r[impl consensus.quepaxa.proposal-order]
pub open spec fn proposal_leq_spec(left: QuePaxaProposal, right: QuePaxaProposal) -> bool {
    left.priority < right.priority
        || (left.priority == right.priority
            && (left.proposer_id < right.proposer_id
                || (left.proposer_id == right.proposer_id && left.value_id <= right.value_id)))
}

// r[impl consensus.quepaxa.proposal-order]
pub open spec fn proposal_lt_spec(left: QuePaxaProposal, right: QuePaxaProposal) -> bool {
    proposal_leq_spec(left, right) && !proposal_eq_key_spec(left, right)
}

// r[impl consensus.quepaxa.proposal-order]
pub open spec fn best_proposal_spec(left: QuePaxaProposal, right: QuePaxaProposal) -> QuePaxaProposal {
    if proposal_leq_spec(left, right) { right } else { left }
}

// r[impl consensus.quepaxa.proposal-order]
pub open spec fn reserved_leader_priority_admitted_spec(
    proposal: QuePaxaProposal,
    declared_leader_id: u64,
    round: u64,
) -> bool {
    proposal.priority != QUEPAXA_LEADER_PRIORITY
        || (round == QUEPAXA_INITIAL_ROUND && proposal.proposer_id == declared_leader_id)
}

// r[impl consensus.quepaxa.proposal-order]
pub fn nil_proposal() -> (result: QuePaxaProposal)
    ensures result == nil_proposal_spec()
{
    QuePaxaProposal {
        priority: QUEPAXA_NIL_PRIORITY,
        proposer_id: QUEPAXA_NIL_PROPOSER,
        value_id: QUEPAXA_NIL_VALUE,
    }
}

// r[impl consensus.quepaxa.proposal-order]
pub fn proposal_eq_key(left: QuePaxaProposal, right: QuePaxaProposal) -> (result: bool)
    ensures result == proposal_eq_key_spec(left, right)
{
    left.priority == right.priority
        && left.proposer_id == right.proposer_id
        && left.value_id == right.value_id
}

// r[impl consensus.quepaxa.proposal-order]
pub fn proposal_leq(left: QuePaxaProposal, right: QuePaxaProposal) -> (result: bool)
    ensures result == proposal_leq_spec(left, right)
{
    left.priority < right.priority
        || (left.priority == right.priority
            && (left.proposer_id < right.proposer_id
                || (left.proposer_id == right.proposer_id && left.value_id <= right.value_id)))
}

// r[impl consensus.quepaxa.proposal-order]
pub fn proposal_lt(left: QuePaxaProposal, right: QuePaxaProposal) -> (result: bool)
    ensures result == proposal_lt_spec(left, right)
{
    proposal_leq(left, right) && !proposal_eq_key(left, right)
}

// r[impl consensus.quepaxa.proposal-order]
pub fn best_proposal(left: QuePaxaProposal, right: QuePaxaProposal) -> (result: QuePaxaProposal)
    ensures result == best_proposal_spec(left, right)
{
    if proposal_leq(left, right) { right } else { left }
}

// r[impl consensus.quepaxa.proposal-order]
pub fn reserved_leader_priority_admitted(
    proposal: QuePaxaProposal,
    declared_leader_id: u64,
    round: u64,
) -> (result: bool)
    ensures result == reserved_leader_priority_admitted_spec(proposal, declared_leader_id, round)
{
    proposal.priority != QUEPAXA_LEADER_PRIORITY
        || (round == QUEPAXA_INITIAL_ROUND && proposal.proposer_id == declared_leader_id)
}

// r[verify consensus.quepaxa.proposal-order]
pub proof fn best_proposal_returns_right_when_left_leq(left: QuePaxaProposal, right: QuePaxaProposal)
    requires proposal_leq_spec(left, right)
    ensures best_proposal_spec(left, right) == right
{
}

// r[verify consensus.quepaxa.proposal-order]
pub proof fn reserved_priority_rejects_non_leader(
    proposal: QuePaxaProposal,
    declared_leader_id: u64,
    round: u64,
)
    requires proposal.priority == QUEPAXA_LEADER_PRIORITY, proposal.proposer_id != declared_leader_id
    ensures !reserved_leader_priority_admitted_spec(proposal, declared_leader_id, round)
{
}

} // verus!
