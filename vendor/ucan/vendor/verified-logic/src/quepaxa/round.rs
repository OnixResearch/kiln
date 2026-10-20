//! QuePaxa propagation, abstract-round, concrete-phase, fast-path, and hedging guards.

use vstd::prelude::*;

use crate::quepaxa::model::*;
use crate::quepaxa::order::*;

verus! {

// r[impl consensus.quepaxa.tcast]
pub open spec fn propagation_admitted_spec(facts: QuePaxaPropagationFacts) -> bool {
    facts.existent_quorum
        && facts.common_quorum
        && facts.universal_quorum
        && facts.universal_subset_common
        && facts.common_subset_existent
}

// r[impl consensus.quepaxa.abstract-round]
pub open spec fn abstract_decision_guard_spec(round: QuePaxaAbstractRound) -> bool {
    !round.already_decided
        && round.proposed_value_known
        && proposal_eq_key_spec(round.existent_best, round.universal_best)
}

// r[impl consensus.quepaxa.abstract-round]
pub open spec fn abstract_decision_value_spec(round: QuePaxaAbstractRound) -> u64 {
    if abstract_decision_guard_spec(round) {
        round.existent_best.value_id
    } else {
        QUEPAXA_NIL_VALUE
    }
}

// r[impl consensus.quepaxa.abstract-round]
pub open spec fn abstract_next_candidate_spec(round: QuePaxaAbstractRound) -> QuePaxaProposal {
    if abstract_decision_guard_spec(round) { round.existent_best } else { round.common_best }
}

// r[impl consensus.quepaxa.concrete-phases]
pub open spec fn valid_phase_spec(phase: u64) -> bool {
    phase < QUEPAXA_PHASE_COUNT
}

// r[impl consensus.quepaxa.concrete-phases]
pub open spec fn catch_up_spec(
    proposer_step: u64,
    proposer_template: QuePaxaProposal,
    recorder_step: u64,
    recorder_first: QuePaxaProposal,
) -> QuePaxaCatchUpResult {
    if recorder_step > proposer_step {
        QuePaxaCatchUpResult { step: recorder_step, proposal_template: recorder_first }
    } else {
        QuePaxaCatchUpResult { step: proposer_step, proposal_template: proposer_template }
    }
}

// r[impl consensus.quepaxa.concrete-phases]
pub open spec fn concrete_phase2_decision_refines_spec(
    phase: u64,
    spread_proposal: QuePaxaProposal,
    prior_best: QuePaxaProposal,
    abstract_round: QuePaxaAbstractRound,
) -> bool {
    phase == QUEPAXA_PHASE_GATHER_EXISTENT
        && proposal_eq_key_spec(spread_proposal, prior_best)
        && abstract_decision_guard_spec(abstract_round)
}

// r[impl consensus.quepaxa.fast-path]
pub open spec fn fast_path_decision_admitted_spec(facts: QuePaxaFastPathFacts) -> bool {
    facts.round == QUEPAXA_INITIAL_ROUND
        && facts.proposer_id == facts.declared_leader_id
        && facts.proposal.proposer_id == facts.declared_leader_id
        && facts.proposal.priority == QUEPAXA_LEADER_PRIORITY
        && facts.first_at_quorum
        && facts.quorum_met
        && facts.reserved_priority_unique
        && facts.abstract_guard_holds
}

// r[impl consensus.quepaxa.fast-path]
pub open spec fn fast_path_refines_abstract_spec(facts: QuePaxaFastPathFacts) -> bool {
    fast_path_decision_admitted_spec(facts) ==> facts.abstract_guard_holds
}

// r[impl consensus.quepaxa.hedging]
pub open spec fn hedging_schedule_admitted_spec(facts: QuePaxaHedgingFacts) -> bool {
    facts.delays_nondecreasing
        && facts.proposer_count > facts.crash_fault_budget
        && (!facts.leader_configured || (facts.first_is_declared_leader && facts.first_delay_zero))
}

// r[impl consensus.quepaxa.tcast]
pub fn propagation_admitted(facts: QuePaxaPropagationFacts) -> (result: bool)
    ensures result == propagation_admitted_spec(facts)
{
    facts.existent_quorum
        && facts.common_quorum
        && facts.universal_quorum
        && facts.universal_subset_common
        && facts.common_subset_existent
}

// r[impl consensus.quepaxa.abstract-round]
pub fn abstract_decision_guard(round: QuePaxaAbstractRound) -> (result: bool)
    ensures result == abstract_decision_guard_spec(round)
{
    !round.already_decided && round.proposed_value_known && proposal_eq_key(round.existent_best, round.universal_best)
}

// r[impl consensus.quepaxa.abstract-round]
pub fn abstract_decision_value(round: QuePaxaAbstractRound) -> (result: u64)
    ensures result == abstract_decision_value_spec(round)
{
    if abstract_decision_guard(round) { round.existent_best.value_id } else { QUEPAXA_NIL_VALUE }
}

// r[impl consensus.quepaxa.abstract-round]
pub fn abstract_next_candidate(round: QuePaxaAbstractRound) -> (result: QuePaxaProposal)
    ensures result == abstract_next_candidate_spec(round)
{
    if abstract_decision_guard(round) { round.existent_best } else { round.common_best }
}

// r[impl consensus.quepaxa.concrete-phases]
pub fn valid_phase(phase: u64) -> (result: bool)
    ensures result == valid_phase_spec(phase)
{
    phase < QUEPAXA_PHASE_COUNT
}

// r[impl consensus.quepaxa.concrete-phases]
pub fn catch_up(
    proposer_step: u64,
    proposer_template: QuePaxaProposal,
    recorder_step: u64,
    recorder_first: QuePaxaProposal,
) -> (result: QuePaxaCatchUpResult)
    ensures result == catch_up_spec(proposer_step, proposer_template, recorder_step, recorder_first)
{
    if recorder_step > proposer_step {
        QuePaxaCatchUpResult { step: recorder_step, proposal_template: recorder_first }
    } else {
        QuePaxaCatchUpResult { step: proposer_step, proposal_template: proposer_template }
    }
}

// r[impl consensus.quepaxa.concrete-phases]
pub fn concrete_phase2_decision_refines(
    phase: u64,
    spread_proposal: QuePaxaProposal,
    prior_best: QuePaxaProposal,
    abstract_round: QuePaxaAbstractRound,
) -> (result: bool)
    ensures result == concrete_phase2_decision_refines_spec(phase, spread_proposal, prior_best, abstract_round)
{
    phase == QUEPAXA_PHASE_GATHER_EXISTENT
        && proposal_eq_key(spread_proposal, prior_best)
        && abstract_decision_guard(abstract_round)
}

// r[impl consensus.quepaxa.fast-path]
pub fn fast_path_decision_admitted(facts: QuePaxaFastPathFacts) -> (result: bool)
    ensures result == fast_path_decision_admitted_spec(facts)
{
    facts.round == QUEPAXA_INITIAL_ROUND
        && facts.proposer_id == facts.declared_leader_id
        && facts.proposal.proposer_id == facts.declared_leader_id
        && facts.proposal.priority == QUEPAXA_LEADER_PRIORITY
        && facts.first_at_quorum
        && facts.quorum_met
        && facts.reserved_priority_unique
        && facts.abstract_guard_holds
}

// r[impl consensus.quepaxa.fast-path]
pub fn fast_path_refines_abstract(facts: QuePaxaFastPathFacts) -> (result: bool)
    ensures result == fast_path_refines_abstract_spec(facts)
{
    !fast_path_decision_admitted(facts) || facts.abstract_guard_holds
}

// r[impl consensus.quepaxa.hedging]
pub fn hedging_schedule_admitted(facts: QuePaxaHedgingFacts) -> (result: bool)
    ensures result == hedging_schedule_admitted_spec(facts)
{
    facts.delays_nondecreasing
        && facts.proposer_count > facts.crash_fault_budget
        && (!facts.leader_configured || (facts.first_is_declared_leader && facts.first_delay_zero))
}

// r[verify consensus.quepaxa.tcast]
pub proof fn propagation_admission_has_subset_chain(facts: QuePaxaPropagationFacts)
    requires propagation_admitted_spec(facts)
    ensures facts.universal_subset_common, facts.common_subset_existent
{
}

// r[verify consensus.quepaxa.abstract-round]
pub proof fn abstract_decision_validity(round: QuePaxaAbstractRound)
    requires abstract_decision_guard_spec(round)
    ensures abstract_decision_value_spec(round) == round.existent_best.value_id
{
}

// r[verify consensus.quepaxa.abstract-round]
pub proof fn abstract_integrity_rejects_second_decision(round: QuePaxaAbstractRound)
    requires round.already_decided
    ensures !abstract_decision_guard_spec(round)
{
}

// r[verify consensus.quepaxa.concrete-phases]
pub proof fn catch_up_advances_monotonically(
    proposer_step: u64,
    proposer_template: QuePaxaProposal,
    recorder_step: u64,
    recorder_first: QuePaxaProposal,
)
    requires recorder_step > proposer_step
    ensures catch_up_spec(proposer_step, proposer_template, recorder_step, recorder_first).step == recorder_step
{
}

// r[verify consensus.quepaxa.concrete-phases]
pub proof fn concrete_phase2_refinement_implies_abstract(
    phase: u64,
    spread_proposal: QuePaxaProposal,
    prior_best: QuePaxaProposal,
    abstract_round: QuePaxaAbstractRound,
)
    requires concrete_phase2_decision_refines_spec(phase, spread_proposal, prior_best, abstract_round)
    ensures abstract_decision_guard_spec(abstract_round)
{
}

// r[verify consensus.quepaxa.fast-path]
pub proof fn non_initial_fast_path_rejected(facts: QuePaxaFastPathFacts)
    requires facts.round != QUEPAXA_INITIAL_ROUND
    ensures !fast_path_decision_admitted_spec(facts)
{
}

// r[verify consensus.quepaxa.hedging]
pub proof fn admitted_hedging_has_enough_proposers(facts: QuePaxaHedgingFacts)
    requires hedging_schedule_admitted_spec(facts)
    ensures facts.proposer_count > facts.crash_fault_budget
{
}

} // verus!
