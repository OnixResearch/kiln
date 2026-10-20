use super::*;

const LOW_PRIORITY: u64 = 7;
const HIGH_PRIORITY: u64 = 11;
const LOW_PROPOSER: u64 = 3;
const HIGH_PROPOSER: u64 = 5;
const LOW_VALUE: u64 = 13;
const HIGH_VALUE: u64 = 17;
const START_STEP: u64 = 20;
const STALE_STEP: u64 = 19;
const NEXT_STEP: u64 = 21;
const SKIPPED_STEP: u64 = 23;
const DECLARED_LEADER: u64 = 41;
const NON_LEADER: u64 = 43;
const SUFFICIENT_PROPOSERS: u64 = 4;
const CRASH_FAULT_BUDGET: u64 = 3;

fn low_proposal() -> QuePaxaProposal {
    QuePaxaProposal {
        priority: LOW_PRIORITY,
        proposer_id: LOW_PROPOSER,
        value_id: LOW_VALUE,
    }
}

fn high_proposal() -> QuePaxaProposal {
    QuePaxaProposal {
        priority: HIGH_PRIORITY,
        proposer_id: HIGH_PROPOSER,
        value_id: HIGH_VALUE,
    }
}

fn leader_proposal() -> QuePaxaProposal {
    QuePaxaProposal {
        priority: QUEPAXA_LEADER_PRIORITY,
        proposer_id: DECLARED_LEADER,
        value_id: HIGH_VALUE,
    }
}

fn isr_state() -> QuePaxaIsrState {
    QuePaxaIsrState {
        step: START_STEP,
        first: low_proposal(),
        current: low_proposal(),
    }
}

fn abstract_round_deciding() -> QuePaxaAbstractRound {
    QuePaxaAbstractRound {
        already_decided: false,
        proposed_value_known: true,
        existent_best: high_proposal(),
        universal_best: high_proposal(),
        common_best: low_proposal(),
    }
}

fn fast_path_facts() -> QuePaxaFastPathFacts {
    QuePaxaFastPathFacts {
        round: QUEPAXA_INITIAL_ROUND,
        proposer_id: DECLARED_LEADER,
        declared_leader_id: DECLARED_LEADER,
        proposal: leader_proposal(),
        first_at_quorum: true,
        quorum_met: true,
        reserved_priority_unique: true,
        abstract_guard_holds: true,
    }
}

#[test]
fn boundary_rejects_overclaim_metadata() {
    assert!(claim_admitted(QuePaxaClaimKind::FiniteDeterministicSafety));
    assert!(!claim_admitted(QuePaxaClaimKind::NetworkLiveness));
    assert!(!claim_admitted(QuePaxaClaimKind::BenchmarkResult));
    assert!(!claim_admitted(QuePaxaClaimKind::ProductionSmr));
}

// r[verify consensus.quepaxa.proposal-order]
#[test]
fn proposal_order_selects_deterministic_best_and_tie_policy() {
    assert_eq!(best_proposal(low_proposal(), high_proposal()), high_proposal());
    let tied_priority_high_proposer = QuePaxaProposal {
        priority: LOW_PRIORITY,
        proposer_id: HIGH_PROPOSER,
        value_id: LOW_VALUE,
    };
    assert_eq!(best_proposal(low_proposal(), tied_priority_high_proposer), tied_priority_high_proposer);
}

// r[verify consensus.quepaxa.fixtures]
#[test]
fn reserved_leader_priority_rejects_non_leader() {
    let non_leader_reserved = QuePaxaProposal {
        priority: QUEPAXA_LEADER_PRIORITY,
        proposer_id: NON_LEADER,
        value_id: HIGH_VALUE,
    };
    assert!(reserved_leader_priority_admitted(leader_proposal(), DECLARED_LEADER, QUEPAXA_INITIAL_ROUND));
    assert!(!reserved_leader_priority_admitted(non_leader_reserved, DECLARED_LEADER, QUEPAXA_INITIAL_ROUND));
}

#[test]
fn isr_same_step_and_advance_transitions() {
    let same = record(isr_state(), START_STEP, high_proposal());
    assert_eq!(same.state.first, low_proposal());
    assert_eq!(same.state.current, high_proposal());

    let advanced = record(isr_state(), NEXT_STEP, high_proposal());
    assert_eq!(advanced.state.step, NEXT_STEP);
    assert_eq!(advanced.state.first, high_proposal());
    assert_eq!(advanced.prior, low_proposal());
}

#[test]
fn isr_stale_and_skipped_steps_fail_closed() {
    let stale = record(isr_state(), STALE_STEP, high_proposal());
    assert!(stale.stale);
    assert_eq!(stale.state, isr_state());

    let skipped = record(isr_state(), SKIPPED_STEP, high_proposal());
    assert!(skipped.skipped_step);
    assert_eq!(skipped.prior, nil_proposal());
}

#[test]
fn propagation_requires_quorum_and_subset_chain() {
    let admitted = QuePaxaPropagationFacts {
        existent_quorum: true,
        common_quorum: true,
        universal_quorum: true,
        universal_subset_common: true,
        common_subset_existent: true,
    };
    assert!(propagation_admitted(admitted));
    assert!(!propagation_admitted(QuePaxaPropagationFacts {
        common_quorum: false,
        ..admitted
    }));
}

#[test]
fn abstract_round_decides_or_carries_forward() {
    let deciding = abstract_round_deciding();
    assert!(abstract_decision_guard(deciding));
    assert_eq!(abstract_decision_value(deciding), HIGH_VALUE);

    let divergent = QuePaxaAbstractRound {
        universal_best: low_proposal(),
        ..deciding
    };
    assert!(!abstract_decision_guard(divergent));
    assert_eq!(abstract_next_candidate(divergent), low_proposal());

    let already_decided = QuePaxaAbstractRound {
        already_decided: true,
        ..deciding
    };
    assert!(!abstract_decision_guard(already_decided));
}

#[test]
fn concrete_phase_and_catch_up_guards() {
    assert!(valid_phase(QUEPAXA_PHASE_GATHER_EXISTENT));
    assert!(!valid_phase(QUEPAXA_PHASE_COUNT));

    let caught_up = catch_up(START_STEP, low_proposal(), NEXT_STEP, high_proposal());
    assert_eq!(caught_up.step, NEXT_STEP);
    assert_eq!(caught_up.proposal_template, high_proposal());

    let no_backward = catch_up(START_STEP, low_proposal(), STALE_STEP, high_proposal());
    assert_eq!(no_backward.step, START_STEP);
    assert_eq!(no_backward.proposal_template, low_proposal());

    assert!(concrete_phase2_decision_refines(
        QUEPAXA_PHASE_GATHER_EXISTENT,
        high_proposal(),
        high_proposal(),
        abstract_round_deciding(),
    ));
}

#[test]
fn fast_path_accepts_leader_and_rejects_invalid_cases() {
    assert!(fast_path_decision_admitted(fast_path_facts()));
    assert!(!fast_path_decision_admitted(QuePaxaFastPathFacts {
        round: NEXT_STEP,
        ..fast_path_facts()
    }));
    assert!(!fast_path_decision_admitted(QuePaxaFastPathFacts {
        proposer_id: NON_LEADER,
        ..fast_path_facts()
    }));
    assert!(!fast_path_decision_admitted(QuePaxaFastPathFacts {
        reserved_priority_unique: false,
        ..fast_path_facts()
    }));
}

// r[verify consensus.quepaxa.fixtures]
#[test]
fn hedging_schedule_admits_leader_first_and_rejects_bad_schedules() {
    let admitted = QuePaxaHedgingFacts {
        leader_configured: true,
        first_is_declared_leader: true,
        first_delay_zero: true,
        delays_nondecreasing: true,
        proposer_count: SUFFICIENT_PROPOSERS,
        crash_fault_budget: CRASH_FAULT_BUDGET,
    };
    assert!(hedging_schedule_admitted(admitted));
    assert!(!hedging_schedule_admitted(QuePaxaHedgingFacts {
        delays_nondecreasing: false,
        ..admitted
    }));
    assert!(!hedging_schedule_admitted(QuePaxaHedgingFacts {
        proposer_count: CRASH_FAULT_BUDGET,
        ..admitted
    }));
}
