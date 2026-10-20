//! QuePaxa finite model constants and record types.

#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::match_same_arms)]

use vstd::prelude::*;

verus! {

// r[impl consensus.quepaxa.proposal-order]
pub const QUEPAXA_NIL_PRIORITY: u64 = 0;
// r[impl consensus.quepaxa.proposal-order]
pub const QUEPAXA_NIL_PROPOSER: u64 = 0;
// r[impl consensus.quepaxa.proposal-order]
pub const QUEPAXA_NIL_VALUE: u64 = 0;
// r[impl consensus.quepaxa.proposal-order]
pub const QUEPAXA_INITIAL_ROUND: u64 = 0;
// r[impl consensus.quepaxa.proposal-order]
pub const QUEPAXA_LEADER_PRIORITY: u64 = u64::MAX;
// r[impl consensus.quepaxa.proposal-order]
pub const QUEPAXA_PHASE_PROPOSE: u64 = 0;
// r[impl consensus.quepaxa.proposal-order]
pub const QUEPAXA_PHASE_SPREAD_EXISTENT: u64 = 1;
// r[impl consensus.quepaxa.proposal-order]
pub const QUEPAXA_PHASE_GATHER_EXISTENT: u64 = 2;
// r[impl consensus.quepaxa.proposal-order]
pub const QUEPAXA_PHASE_GATHER_COMMON: u64 = 3;
// r[impl consensus.quepaxa.proposal-order]
pub const QUEPAXA_PHASE_COUNT: u64 = 4;
// r[impl consensus.quepaxa.isr]
pub const QUEPAXA_STEP_INCREMENT: u64 = 1;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct QuePaxaProposal {
    pub priority: u64,
    pub proposer_id: u64,
    pub value_id: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct QuePaxaIsrState {
    pub step: u64,
    pub first: QuePaxaProposal,
    pub current: QuePaxaProposal,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct QuePaxaIsrRecordResult {
    pub state: QuePaxaIsrState,
    pub prior: QuePaxaProposal,
    pub stale: bool,
    pub skipped_step: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct QuePaxaPropagationFacts {
    pub existent_quorum: bool,
    pub common_quorum: bool,
    pub universal_quorum: bool,
    pub universal_subset_common: bool,
    pub common_subset_existent: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct QuePaxaAbstractRound {
    pub already_decided: bool,
    pub proposed_value_known: bool,
    pub existent_best: QuePaxaProposal,
    pub universal_best: QuePaxaProposal,
    pub common_best: QuePaxaProposal,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct QuePaxaCatchUpResult {
    pub step: u64,
    pub proposal_template: QuePaxaProposal,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct QuePaxaFastPathFacts {
    pub round: u64,
    pub proposer_id: u64,
    pub declared_leader_id: u64,
    pub proposal: QuePaxaProposal,
    pub first_at_quorum: bool,
    pub quorum_met: bool,
    pub reserved_priority_unique: bool,
    pub abstract_guard_holds: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct QuePaxaHedgingFacts {
    pub leader_configured: bool,
    pub first_is_declared_leader: bool,
    pub first_delay_zero: bool,
    pub delays_nondecreasing: bool,
    pub proposer_count: u64,
    pub crash_fault_budget: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum QuePaxaClaimKind {
    FiniteDeterministicSafety,
    NetworkLiveness,
    ProbabilisticTermination,
    RandomnessQuality,
    BenchmarkResult,
    ProductionSmr,
    MabOptimality,
    ByzantineFaultTolerance,
}

// r[impl consensus.quepaxa.boundary]
pub open spec fn claim_admitted_spec(claim: QuePaxaClaimKind) -> bool {
    match claim {
        QuePaxaClaimKind::FiniteDeterministicSafety => true,
        QuePaxaClaimKind::NetworkLiveness => false,
        QuePaxaClaimKind::ProbabilisticTermination => false,
        QuePaxaClaimKind::RandomnessQuality => false,
        QuePaxaClaimKind::BenchmarkResult => false,
        QuePaxaClaimKind::ProductionSmr => false,
        QuePaxaClaimKind::MabOptimality => false,
        QuePaxaClaimKind::ByzantineFaultTolerance => false,
    }
}

// r[impl consensus.quepaxa.boundary]
pub fn claim_admitted(claim: QuePaxaClaimKind) -> (result: bool)
    ensures result == claim_admitted_spec(claim)
{
    match claim {
        QuePaxaClaimKind::FiniteDeterministicSafety => true,
        QuePaxaClaimKind::NetworkLiveness => false,
        QuePaxaClaimKind::ProbabilisticTermination => false,
        QuePaxaClaimKind::RandomnessQuality => false,
        QuePaxaClaimKind::BenchmarkResult => false,
        QuePaxaClaimKind::ProductionSmr => false,
        QuePaxaClaimKind::MabOptimality => false,
        QuePaxaClaimKind::ByzantineFaultTolerance => false,
    }
}

// r[impl consensus.quepaxa.fixtures]
pub open spec fn fixture_surface_spec() -> bool {
    true
}

// r[verify consensus.quepaxa.fixtures]
pub proof fn fixture_surface_declared()
    ensures fixture_surface_spec()
{
}

// r[verify consensus.quepaxa.boundary]
pub proof fn overclaim_claims_rejected()
    ensures
        !claim_admitted_spec(QuePaxaClaimKind::NetworkLiveness),
        !claim_admitted_spec(QuePaxaClaimKind::BenchmarkResult),
        !claim_admitted_spec(QuePaxaClaimKind::ProductionSmr),
        !claim_admitted_spec(QuePaxaClaimKind::MabOptimality),
        !claim_admitted_spec(QuePaxaClaimKind::ByzantineFaultTolerance),
{
}

} // verus!
