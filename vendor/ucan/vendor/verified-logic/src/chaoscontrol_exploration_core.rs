//! Composite ChaosControl exploration core facade.
//!
//! This module wires together the existing coverage, oracle, and scheduler
//! primitives into one deterministic exploration step.  Rejected oracle records
//! return an explicit reason and preserve both core and branch state.

use vstd::prelude::*;

use crate::chaoscontrol_coverage::CODE_REGION_END;
use crate::chaoscontrol_coverage::count_new_edges;
use crate::chaoscontrol_coverage::merge_coverage;
use crate::chaoscontrol_coverage::new_coverage_bitmap;
use crate::chaoscontrol_coverage::record_code_hit;
use crate::chaoscontrol_oracle::OracleRecord;
use crate::chaoscontrol_oracle::Verdict;
use crate::chaoscontrol_oracle::compute_verdict;
use crate::chaoscontrol_oracle::valid_oracle_record;
use crate::chaoscontrol_scheduler::SchedulerState;
use crate::chaoscontrol_scheduler::SchedulingStrategy;
use crate::chaoscontrol_scheduler::advance_randomized;
use crate::chaoscontrol_scheduler::advance_round_robin;
use crate::chaoscontrol_scheduler::new_scheduler_state;

verus! {

/// Composite exploration state owned by the facade.
pub struct ExplorationCore {
    pub scheduler: SchedulerState,
    pub global_coverage: Vec<u8>,
    pub accepted_steps: u64,
    pub rejected_steps: u64,
}

/// Rejection reason for non-mutating step refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplorationRejectReason {
    OracleViolation,
}

/// Public outcome for one exploration step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplorationStepResult {
    Accepted { new_edges: u32, active: u32, fingerprint: u64 },
    Rejected { reason: ExplorationRejectReason },
}

/// Outcome alias requested by the composite facade requirement.
pub type ExplorationOutcome = ExplorationStepResult;

// ── Composite specs ─────────────────────────────────────────────────

pub open spec fn valid_core_spec(core: ExplorationCore) -> bool {
    &&& crate::chaoscontrol_scheduler::valid_scheduler_spec(core.scheduler)
    &&& crate::chaoscontrol_coverage::valid_coverage_bitmap_spec(core.global_coverage@)
}

pub open spec fn oracle_accepts_spec(record: OracleRecord) -> bool {
    crate::chaoscontrol_oracle::valid_oracle_record_spec(record)
        && crate::chaoscontrol_oracle::verdict_spec(record) == Verdict::Passed
}

/// Accepted-step relation exposed by the facade.
pub open spec fn accepted_spec(
    old_core: ExplorationCore,
    new_core: ExplorationCore,
    old_branch: Seq<u8>,
    new_branch: Seq<u8>,
    record: OracleRecord,
    coverage_index: usize,
    outcome: ExplorationOutcome,
) -> bool {
    &&& valid_core_spec(old_core)
    &&& crate::chaoscontrol_coverage::valid_coverage_bitmap_spec(old_branch)
    &&& coverage_index < CODE_REGION_END
    &&& oracle_accepts_spec(record)
    &&& valid_core_spec(new_core)
    &&& crate::chaoscontrol_coverage::valid_coverage_bitmap_spec(new_branch)
    &&& new_branch[coverage_index as int] == old_branch[coverage_index as int].saturating_add(1)
    &&& forall|i: int| 0 <= i < old_branch.len() && i != coverage_index as int ==> new_branch[i] == old_branch[i]
    &&& new_core.accepted_steps == old_core.accepted_steps + 1
    &&& new_core.rejected_steps == old_core.rejected_steps
    &&& match outcome {
        ExplorationStepResult::Accepted { active, fingerprint, .. } =>
            active == new_core.scheduler.active && fingerprint == new_core.scheduler.fingerprint,
        _ => false,
    }
}

/// Rejected oracle-violation relation: no core or branch mutation.
pub open spec fn reject_preserves_spec(
    old_core: ExplorationCore,
    new_core: ExplorationCore,
    old_branch: Seq<u8>,
    new_branch: Seq<u8>,
    reason: ExplorationRejectReason,
    outcome: ExplorationOutcome,
) -> bool {
    &&& valid_core_spec(old_core)
    &&& valid_core_spec(new_core)
    &&& new_core.scheduler == old_core.scheduler
    &&& new_core.global_coverage@ =~= old_core.global_coverage@
    &&& new_core.accepted_steps == old_core.accepted_steps
    &&& new_core.rejected_steps == old_core.rejected_steps + 1
    &&& new_branch =~= old_branch
    &&& outcome == ExplorationStepResult::Rejected { reason }
}

/// Source-refinement summary: accepted facade steps rest on the source module
/// predicates for coverage, oracle verdict, and scheduler validity.
pub open spec fn refines_sources(
    old_core: ExplorationCore,
    new_core: ExplorationCore,
    old_branch: Seq<u8>,
    new_branch: Seq<u8>,
    record: OracleRecord,
    coverage_index: usize,
    outcome: ExplorationOutcome,
) -> bool {
    accepted_spec(old_core, new_core, old_branch, new_branch, record, coverage_index, outcome) ==>
        (crate::chaoscontrol_scheduler::valid_scheduler_spec(old_core.scheduler)
         && crate::chaoscontrol_scheduler::valid_scheduler_spec(new_core.scheduler)
         && crate::chaoscontrol_coverage::valid_coverage_bitmap_spec(old_core.global_coverage@)
         && crate::chaoscontrol_coverage::valid_coverage_bitmap_spec(old_branch)
         && crate::chaoscontrol_oracle::valid_oracle_record_spec(record)
         && crate::chaoscontrol_oracle::verdict_spec(record) == Verdict::Passed)
}

// r[impl composite-primitives.chaoscontrol-exploration-core]
/// Create a composite exploration core with zero global coverage.
pub fn new_exploration_core(
    num_vcpus: u32,
    raw_default_quantum: u64,
    strategy: SchedulingStrategy,
) -> (core: ExplorationCore)
    requires
        num_vcpus >= 1,
        crate::chaoscontrol_scheduler::valid_strategy_spec(strategy),
    ensures
        valid_core_spec(core),
        core.accepted_steps == 0,
        core.rejected_steps == 0,
{
    let scheduler = new_scheduler_state(num_vcpus, raw_default_quantum, strategy);
    let global_coverage = new_coverage_bitmap();
    ExplorationCore { scheduler, global_coverage, accepted_steps: 0, rejected_steps: 0 }
}

// r[impl composite-primitives.chaoscontrol-exploration-core]
/// Run one deterministic, coverage-accounted, oracle-checked exploration step.
/// Invalid or failing oracle records are rejected before any covered-state update.
pub fn exploration_step(
    core: &mut ExplorationCore,
    branch_coverage: &mut Vec<u8>,
    record: &OracleRecord,
    coverage_index: usize,
    rng_word: u64,
) -> (outcome: ExplorationOutcome)
    requires
        valid_core_spec(*old(core)),
        crate::chaoscontrol_coverage::valid_coverage_bitmap_spec(old(branch_coverage)@),
        coverage_index < CODE_REGION_END,
        old(core).accepted_steps < u64::MAX,
        old(core).rejected_steps < u64::MAX,
        old(core).scheduler.remaining > 0,
    ensures
        valid_core_spec(*core),
        crate::chaoscontrol_coverage::valid_coverage_bitmap_spec(branch_coverage@),
        (outcome is Accepted) ==> accepted_spec(
            *old(core), *core, old(branch_coverage)@, branch_coverage@, *record, coverage_index, outcome,
        ),
        (outcome == ExplorationStepResult::Rejected { reason: ExplorationRejectReason::OracleViolation }) ==>
            reject_preserves_spec(
                *old(core), *core, old(branch_coverage)@, branch_coverage@,
                ExplorationRejectReason::OracleViolation, outcome,
            ),
{
    if !valid_oracle_record(record) {
        core.rejected_steps = core.rejected_steps + 1;
        return ExplorationStepResult::Rejected { reason: ExplorationRejectReason::OracleViolation };
    }

    let verdict = compute_verdict(record);
    assert(verdict == crate::chaoscontrol_oracle::verdict_spec(*record));
    match verdict {
        Verdict::Passed => {}
        Verdict::Failed => {
            core.rejected_steps = core.rejected_steps + 1;
            return ExplorationStepResult::Rejected { reason: ExplorationRejectReason::OracleViolation };
        }
    }
    assert(crate::chaoscontrol_oracle::verdict_spec(*record) == Verdict::Passed);
    assert(crate::chaoscontrol_oracle::valid_oracle_record_spec(*record));

    record_code_hit(branch_coverage, coverage_index);
    assert(branch_coverage@[coverage_index as int] == old(branch_coverage)@[coverage_index as int].saturating_add(1));
    assert forall|i: int| 0 <= i < old(branch_coverage)@.len() && i != coverage_index as int implies branch_coverage@[i] == old(branch_coverage)@[i] by {
        assert(0 <= i < crate::chaoscontrol_coverage::MAP_SIZE as int && i != coverage_index as int);
    }
    let new_edges = count_new_edges(branch_coverage, &core.global_coverage);
    merge_coverage(&mut core.global_coverage, branch_coverage);

    let switched = crate::chaoscontrol_scheduler::tick(&mut core.scheduler);
    if switched {
        match core.scheduler.strategy {
            SchedulingStrategy::RoundRobin => {
                advance_round_robin(&mut core.scheduler);
            }
            SchedulingStrategy::Randomized { .. } => {
                advance_randomized(&mut core.scheduler, rng_word);
            }
        }
    }

    core.accepted_steps = core.accepted_steps + 1;
    assert(oracle_accepts_spec(*record));
    assert(branch_coverage@[coverage_index as int] == old(branch_coverage)@[coverage_index as int].saturating_add(1));
    assert forall|i: int| 0 <= i < old(branch_coverage)@.len() && i != coverage_index as int implies branch_coverage@[i] == old(branch_coverage)@[i] by {
    }
    ExplorationStepResult::Accepted {
        new_edges,
        active: core.scheduler.active,
        fingerprint: core.scheduler.fingerprint,
    }
}

// r[verify composite-primitives.chaoscontrol-exploration-core]
pub proof fn accepted_refines_sources(
    old_core: ExplorationCore,
    new_core: ExplorationCore,
    old_branch: Seq<u8>,
    new_branch: Seq<u8>,
    record: OracleRecord,
    coverage_index: usize,
    outcome: ExplorationOutcome,
)
    requires accepted_spec(old_core, new_core, old_branch, new_branch, record, coverage_index, outcome)
    ensures refines_sources(old_core, new_core, old_branch, new_branch, record, coverage_index, outcome)
{
}

// r[verify composite-primitives.chaoscontrol-exploration-core]
pub proof fn oracle_reject_preserves_without_mutation(
    old_core: ExplorationCore,
    new_core: ExplorationCore,
    old_branch: Seq<u8>,
    new_branch: Seq<u8>,
    outcome: ExplorationOutcome,
)
    requires reject_preserves_spec(
        old_core, new_core, old_branch, new_branch,
        ExplorationRejectReason::OracleViolation, outcome,
    )
    ensures
        new_core.scheduler == old_core.scheduler,
        new_core.global_coverage@ =~= old_core.global_coverage@,
        new_core.accepted_steps == old_core.accepted_steps,
        new_branch =~= old_branch,
{
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chaoscontrol_oracle::new_boolean_always_record;
    use crate::chaoscontrol_oracle::new_unreachable_record;

    #[test]
    fn deterministic_covered_step_is_accepted_and_accounted() {
        let mut core = new_exploration_core(2, 1, SchedulingStrategy::RoundRobin);
        let mut branch = new_coverage_bitmap();
        let record = new_boolean_always_record(1, 1, 0, 1, 1);

        let outcome = exploration_step(&mut core, &mut branch, &record, 7, 0);

        assert_eq!(branch[7], 1);
        assert_eq!(core.global_coverage[7], 1);
        assert_eq!(core.accepted_steps, 1);
        assert_eq!(core.rejected_steps, 0);
        assert!(matches!(outcome, ExplorationStepResult::Accepted {
            new_edges: 1,
            active: 1,
            ..
        }));
    }

    #[test]
    fn oracle_violation_rejects_without_mutating_core_or_branch() {
        let mut core = new_exploration_core(2, 3, SchedulingStrategy::RoundRobin);
        let mut branch = new_coverage_bitmap();
        branch[7] = 9;
        let before_branch = branch.clone();
        let before_global = core.global_coverage.clone();
        let before_active = core.scheduler.active;
        let before_remaining = core.scheduler.remaining;
        let record = new_unreachable_record(1, 1);

        let outcome = exploration_step(&mut core, &mut branch, &record, 7, 0);

        assert_eq!(outcome, ExplorationStepResult::Rejected {
            reason: ExplorationRejectReason::OracleViolation
        });
        assert_eq!(branch, before_branch);
        assert_eq!(core.global_coverage, before_global);
        assert_eq!(core.scheduler.active, before_active);
        assert_eq!(core.scheduler.remaining, before_remaining);
        assert_eq!(core.accepted_steps, 0);
        assert_eq!(core.rejected_steps, 1);
    }
}
