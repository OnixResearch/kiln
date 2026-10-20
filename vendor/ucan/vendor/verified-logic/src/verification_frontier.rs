//! Finite verification-frontier admission models inspired by Lattice/Salt.
//!
//! This module adapts several verification UX patterns into small Trellis-local
//! finite models. Lattice/Salt is treated only as design inspiration: the code
//! below does not execute Lattice, trust its solver behavior, or copy benchmark
//! claims.

use vstd::prelude::*;

verus! {

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    Proven,
    Rejected,
    Unknown,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Admission {
    pub outcome: Outcome,
    pub reason: u8,
    pub evidence: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RangeFact {
    pub var: u8,
    pub lower: i64,
    pub upper: i64,
    pub known: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FiniteFacts {
    pub caller: RangeFact,
    pub path: RangeFact,
    pub type_bound: RangeFact,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Precondition {
    pub kind: u8,
    pub var: u8,
    pub lower: i64,
    pub upper: i64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ContractAdmission {
    pub outcome: Outcome,
    pub reason: u8,
    pub witness_var: u8,
    pub witness_value: i64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SliceAccessCertificate {
    pub backing_len: u64,
    pub start: u64,
    pub end: u64,
    pub offset_kind: u8,
    pub fixed_offset: u64,
    pub dynamic_max: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Provenance {
    pub is_tracked: bool,
    pub depth: u8,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ProgressSummary {
    pub main_path_cost: u64,
    pub alternate_path_cost: u64,
    pub has_yield_or_return: bool,
    pub has_no_yield_cycle: bool,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ExternalReferenceClaim {
    pub labels_reference: bool,
    pub has_local_evidence: bool,
    pub treats_external_as_trusted: bool,
    pub copies_external_benchmark: bool,
}

pub const VARIABLE_PRIMARY: u8 = 1;
pub const VARIABLE_SECONDARY: u8 = 2;

pub const PREDICATE_RANGE: u8 = 1;
pub const PREDICATE_FALSE: u8 = 2;
pub const PREDICATE_UNSUPPORTED: u8 = 3;

pub const DOMAIN_BOOL: u8 = 1;
pub const DOMAIN_U8: u8 = 2;
pub const DOMAIN_I8: u8 = 3;
pub const DOMAIN_U16: u8 = 4;
pub const DOMAIN_I16: u8 = 5;

pub const DOMAIN_BOOL_MIN: i64 = 0;
pub const DOMAIN_BOOL_MAX: i64 = 1;
pub const DOMAIN_U8_MIN: i64 = 0;
pub const DOMAIN_U8_MAX: i64 = 255;
pub const DOMAIN_I8_MIN: i64 = -128;
pub const DOMAIN_I8_MAX: i64 = 127;
pub const DOMAIN_U16_MIN: i64 = 0;
pub const DOMAIN_U16_MAX: i64 = 65535;
pub const DOMAIN_I16_MIN: i64 = -32768;
pub const DOMAIN_I16_MAX: i64 = 32767;

pub const OFFSET_FIXED: u8 = 1;
pub const OFFSET_BOUNDED_DYNAMIC: u8 = 2;
pub const OFFSET_UNBOUNDED_DYNAMIC: u8 = 3;

pub const SCOPE_DEPTH_HEAP_OR_UNTRACKED: u8 = 0;
pub const SCOPE_DEPTH_CALLER: u8 = 1;
pub const SCOPE_DEPTH_LOCAL: u8 = 2;
pub const SCOPE_DEPTH_NESTED_LOCAL: u8 = 3;

pub const PROGRESS_COST_FAST_OP: u64 = 1;
pub const PROGRESS_COST_BRANCH_OP: u64 = 2;
pub const PROGRESS_BUDGET_INTERACTIVE: u64 = 8;
pub const PROGRESS_BUDGET_BACKGROUND: u64 = 16;

pub const REASON_NONE: u8 = 0;
pub const REASON_DIRECT_FALSE: u8 = 1;
pub const REASON_UNSUPPORTED_PREDICATE: u8 = 2;
pub const REASON_MISSING_FACT: u8 = 3;
pub const REASON_LOWER_BOUND_COUNTEREXAMPLE: u8 = 4;
pub const REASON_UPPER_BOUND_COUNTEREXAMPLE: u8 = 5;
pub const REASON_INCONSISTENT_FACTS: u8 = 6;
pub const REASON_INVERTED_RANGE: u8 = 7;
pub const REASON_OUT_OF_BACKING: u8 = 8;
pub const REASON_OFFSET_OUT_OF_RANGE: u8 = 9;
pub const REASON_UNBOUNDED_OFFSET: u8 = 10;
pub const REASON_ARENA_ESCAPE: u8 = 11;
pub const REASON_STORE_ESCAPE: u8 = 12;
pub const REASON_UNNAMED_BUDGET: u8 = 13;
pub const REASON_NO_TERMINAL_PROGRESS: u8 = 14;
pub const REASON_NO_YIELD_CYCLE: u8 = 15;
pub const REASON_OVER_BUDGET: u8 = 16;
pub const REASON_EXTERNAL_TRUST: u8 = 17;
pub const REASON_EXTERNAL_BENCHMARK: u8 = 18;
pub const REASON_MISSING_LOCAL_EVIDENCE: u8 = 19;
pub const REASON_REFERENCE_NOT_LABELED: u8 = 20;
pub const REASON_MAX_NAMED: u8 = REASON_REFERENCE_NOT_LABELED;

// r[depends verification-frontier.contract-implication]
pub open spec fn outcome_is_proof_spec(outcome: Outcome) -> bool {
    matches!(outcome, Outcome::Proven)
}

// r[depends verification-frontier.outcome-invariant-bundle]
pub open spec fn stable_outcome_variant_spec(outcome: Outcome) -> bool {
    outcome == Outcome::Proven || outcome == Outcome::Rejected || outcome == Outcome::Unknown
}

// r[depends verification-frontier.metadata-stability]
pub open spec fn named_reason_code_spec(reason: u8) -> bool {
    reason <= REASON_MAX_NAMED
}

// r[impl verification-frontier.outcome-invariant-bundle]
pub open spec fn admission_is_proof_evidence_spec(admission: Admission) -> bool {
    outcome_is_proof_spec(admission.outcome)
}

// r[depends verification-frontier.outcome-invariant-bundle]
pub open spec fn contract_admission_is_proof_evidence_spec(admission: ContractAdmission) -> bool {
    outcome_is_proof_spec(admission.outcome)
}

// r[impl verification-frontier.metadata-stability]
pub open spec fn admission_metadata_stable_spec(admission: Admission) -> bool {
    stable_outcome_variant_spec(admission.outcome)
        && named_reason_code_spec(admission.reason)
        && (admission.outcome == Outcome::Proven ==> admission.reason == REASON_NONE)
        && (admission.outcome != Outcome::Proven ==> admission.reason != REASON_NONE)
}

// r[depends verification-frontier.metadata-stability]
pub open spec fn contract_metadata_stable_spec(admission: ContractAdmission) -> bool {
    stable_outcome_variant_spec(admission.outcome)
        && named_reason_code_spec(admission.reason)
        && (admission.outcome == Outcome::Proven ==> admission.reason == REASON_NONE)
        && (admission.outcome != Outcome::Proven ==> admission.reason != REASON_NONE)
}

// r[impl verification-frontier.contract-implication]
pub fn outcome_is_proof(outcome: Outcome) -> (result: bool)
    ensures result == outcome_is_proof_spec(outcome)
{
    matches!(outcome, Outcome::Proven)
}

pub open spec fn admission(outcome: Outcome, reason: u8, evidence: u64) -> Admission {
    Admission { outcome, reason, evidence }
}

pub open spec fn contract_result(
    outcome: Outcome,
    reason: u8,
    witness_var: u8,
    witness_value: i64,
) -> ContractAdmission {
    ContractAdmission { outcome, reason, witness_var, witness_value }
}

pub open spec fn absent_range_fact_spec(var: u8) -> RangeFact {
    RangeFact { var, lower: 0, upper: 0, known: false }
}

pub open spec fn fact_matches_var_spec(fact: RangeFact, var: u8) -> bool {
    fact.known && fact.var == var
}

pub open spec fn max_i64_spec(left: i64, right: i64) -> i64 {
    if left >= right { left } else { right }
}

pub open spec fn min_i64_spec(left: i64, right: i64) -> i64 {
    if left <= right { left } else { right }
}

pub open spec fn merge_range_fact_spec(left: RangeFact, right: RangeFact, var: u8) -> RangeFact {
    if fact_matches_var_spec(left, var) && fact_matches_var_spec(right, var) {
        RangeFact {
            var,
            lower: max_i64_spec(left.lower, right.lower),
            upper: min_i64_spec(left.upper, right.upper),
            known: true,
        }
    } else if fact_matches_var_spec(left, var) {
        left
    } else if fact_matches_var_spec(right, var) {
        right
    } else {
        absent_range_fact_spec(var)
    }
}

fn merge_range_fact(left: RangeFact, right: RangeFact, var: u8) -> (result: RangeFact)
    ensures result == merge_range_fact_spec(left, right, var)
{
    if left.known && left.var == var && right.known && right.var == var {
        let lower = if left.lower >= right.lower { left.lower } else { right.lower };
        let upper = if left.upper <= right.upper { left.upper } else { right.upper };
        RangeFact { var, lower, upper, known: true }
    } else if left.known && left.var == var {
        left
    } else if right.known && right.var == var {
        right
    } else {
        RangeFact { var, lower: 0, upper: 0, known: false }
    }
}

// r[depends verification-frontier.type-bound-facts]
pub open spec fn normalize_range_fact_spec(caller: RangeFact, type_bound: RangeFact) -> RangeFact {
    merge_range_fact_spec(caller, type_bound, caller.var)
}

// r[impl verification-frontier.type-bound-facts]
pub fn normalize_range_fact(caller: RangeFact, type_bound: RangeFact) -> (result: RangeFact)
    ensures result == normalize_range_fact_spec(caller, type_bound)
{
    merge_range_fact(caller, type_bound, caller.var)
}

// r[depends verification-frontier.contract-implication]
pub open spec fn combined_contract_fact_spec(facts: FiniteFacts, var: u8) -> RangeFact {
    merge_range_fact_spec(merge_range_fact_spec(facts.type_bound, facts.caller, var), facts.path, var)
}

// r[depends verification-frontier.contract-implication]
pub open spec fn contract_implication_admission_spec(
    facts: FiniteFacts,
    precondition: Precondition,
) -> ContractAdmission {
    let combined = combined_contract_fact_spec(facts, precondition.var);
    if precondition.kind == PREDICATE_FALSE || precondition.lower > precondition.upper {
        contract_result(
            Outcome::Rejected,
            REASON_DIRECT_FALSE,
            precondition.var,
            precondition.lower,
        )
    } else if precondition.kind != PREDICATE_RANGE {
        contract_result(
            Outcome::Unknown,
            REASON_UNSUPPORTED_PREDICATE,
            precondition.var,
            precondition.lower,
        )
    } else if !combined.known {
        contract_result(
            Outcome::Unknown,
            REASON_MISSING_FACT,
            precondition.var,
            precondition.lower,
        )
    } else if combined.lower > combined.upper {
        contract_result(
            Outcome::Rejected,
            REASON_INCONSISTENT_FACTS,
            precondition.var,
            combined.lower,
        )
    } else if combined.lower >= precondition.lower && combined.upper <= precondition.upper {
        contract_result(Outcome::Proven, REASON_NONE, precondition.var, combined.lower)
    } else if combined.lower < precondition.lower {
        contract_result(
            Outcome::Rejected,
            REASON_LOWER_BOUND_COUNTEREXAMPLE,
            precondition.var,
            combined.lower,
        )
    } else {
        contract_result(
            Outcome::Rejected,
            REASON_UPPER_BOUND_COUNTEREXAMPLE,
            precondition.var,
            combined.upper,
        )
    }
}

// r[impl verification-frontier.contract-implication]
pub fn contract_implication_admission(
    facts: FiniteFacts,
    precondition: Precondition,
) -> (result: ContractAdmission)
    ensures result == contract_implication_admission_spec(facts, precondition)
{
    let type_and_caller = merge_range_fact(facts.type_bound, facts.caller, precondition.var);
    let combined = merge_range_fact(type_and_caller, facts.path, precondition.var);
    if precondition.kind == PREDICATE_FALSE || precondition.lower > precondition.upper {
        return ContractAdmission {
            outcome: Outcome::Rejected,
            reason: REASON_DIRECT_FALSE,
            witness_var: precondition.var,
            witness_value: precondition.lower,
        };
    }
    if precondition.kind != PREDICATE_RANGE {
        return ContractAdmission {
            outcome: Outcome::Unknown,
            reason: REASON_UNSUPPORTED_PREDICATE,
            witness_var: precondition.var,
            witness_value: precondition.lower,
        };
    }
    if !combined.known {
        return ContractAdmission {
            outcome: Outcome::Unknown,
            reason: REASON_MISSING_FACT,
            witness_var: precondition.var,
            witness_value: precondition.lower,
        };
    }
    if combined.lower > combined.upper {
        return ContractAdmission {
            outcome: Outcome::Rejected,
            reason: REASON_INCONSISTENT_FACTS,
            witness_var: precondition.var,
            witness_value: combined.lower,
        };
    }
    if combined.lower >= precondition.lower && combined.upper <= precondition.upper {
        return ContractAdmission {
            outcome: Outcome::Proven,
            reason: REASON_NONE,
            witness_var: precondition.var,
            witness_value: combined.lower,
        };
    }
    if combined.lower < precondition.lower {
        return ContractAdmission {
            outcome: Outcome::Rejected,
            reason: REASON_LOWER_BOUND_COUNTEREXAMPLE,
            witness_var: precondition.var,
            witness_value: combined.lower,
        };
    }
    ContractAdmission {
        outcome: Outcome::Rejected,
        reason: REASON_UPPER_BOUND_COUNTEREXAMPLE,
        witness_var: precondition.var,
        witness_value: combined.upper,
    }
}

// r[depends verification-frontier.type-bound-facts]
pub open spec fn type_bound_fact_spec(domain: u8, var: u8) -> RangeFact {
    if domain == DOMAIN_BOOL {
        RangeFact { var, lower: DOMAIN_BOOL_MIN, upper: DOMAIN_BOOL_MAX, known: true }
    } else if domain == DOMAIN_U8 {
        RangeFact { var, lower: DOMAIN_U8_MIN, upper: DOMAIN_U8_MAX, known: true }
    } else if domain == DOMAIN_I8 {
        RangeFact { var, lower: DOMAIN_I8_MIN, upper: DOMAIN_I8_MAX, known: true }
    } else if domain == DOMAIN_U16 {
        RangeFact { var, lower: DOMAIN_U16_MIN, upper: DOMAIN_U16_MAX, known: true }
    } else if domain == DOMAIN_I16 {
        RangeFact { var, lower: DOMAIN_I16_MIN, upper: DOMAIN_I16_MAX, known: true }
    } else {
        absent_range_fact_spec(var)
    }
}

// r[impl verification-frontier.type-bound-facts]
pub fn type_bound_fact(domain: u8, var: u8) -> (result: RangeFact)
    ensures result == type_bound_fact_spec(domain, var)
{
    if domain == DOMAIN_BOOL {
        return RangeFact { var, lower: DOMAIN_BOOL_MIN, upper: DOMAIN_BOOL_MAX, known: true };
    }
    if domain == DOMAIN_U8 {
        return RangeFact { var, lower: DOMAIN_U8_MIN, upper: DOMAIN_U8_MAX, known: true };
    }
    if domain == DOMAIN_I8 {
        return RangeFact { var, lower: DOMAIN_I8_MIN, upper: DOMAIN_I8_MAX, known: true };
    }
    if domain == DOMAIN_U16 {
        return RangeFact { var, lower: DOMAIN_U16_MIN, upper: DOMAIN_U16_MAX, known: true };
    }
    if domain == DOMAIN_I16 {
        return RangeFact { var, lower: DOMAIN_I16_MIN, upper: DOMAIN_I16_MAX, known: true };
    }
    RangeFact { var, lower: 0, upper: 0, known: false }
}

// r[depends verification-frontier.slice-access-certificate]
pub open spec fn slice_access_admission_spec(cert: SliceAccessCertificate) -> Admission {
    if cert.start > cert.end {
        admission(Outcome::Rejected, REASON_INVERTED_RANGE, cert.start)
    } else if cert.end > cert.backing_len {
        admission(Outcome::Rejected, REASON_OUT_OF_BACKING, cert.end)
    } else if cert.offset_kind == OFFSET_FIXED {
        if cert.fixed_offset < cert.end - cert.start {
            admission(Outcome::Proven, REASON_NONE, cert.fixed_offset)
        } else {
            admission(Outcome::Rejected, REASON_OFFSET_OUT_OF_RANGE, cert.fixed_offset)
        }
    } else if cert.offset_kind == OFFSET_BOUNDED_DYNAMIC {
        if cert.dynamic_max < cert.end - cert.start {
            admission(Outcome::Proven, REASON_NONE, cert.dynamic_max)
        } else {
            admission(Outcome::Rejected, REASON_OFFSET_OUT_OF_RANGE, cert.dynamic_max)
        }
    } else if cert.offset_kind == OFFSET_UNBOUNDED_DYNAMIC {
        admission(Outcome::Unknown, REASON_UNBOUNDED_OFFSET, cert.dynamic_max)
    } else {
        admission(Outcome::Unknown, REASON_UNSUPPORTED_PREDICATE, cert.offset_kind as u64)
    }
}

// r[impl verification-frontier.slice-access-certificate]
pub fn slice_access_admission(cert: SliceAccessCertificate) -> (result: Admission)
    ensures result == slice_access_admission_spec(cert)
{
    if cert.start > cert.end {
        return Admission { outcome: Outcome::Rejected, reason: REASON_INVERTED_RANGE, evidence: cert.start };
    }
    if cert.end > cert.backing_len {
        return Admission { outcome: Outcome::Rejected, reason: REASON_OUT_OF_BACKING, evidence: cert.end };
    }
    let len = cert.end - cert.start;
    if cert.offset_kind == OFFSET_FIXED && cert.fixed_offset < len {
        return Admission { outcome: Outcome::Proven, reason: REASON_NONE, evidence: cert.fixed_offset };
    }
    if cert.offset_kind == OFFSET_FIXED {
        return Admission { outcome: Outcome::Rejected, reason: REASON_OFFSET_OUT_OF_RANGE, evidence: cert.fixed_offset };
    }
    if cert.offset_kind == OFFSET_BOUNDED_DYNAMIC && cert.dynamic_max < len {
        return Admission { outcome: Outcome::Proven, reason: REASON_NONE, evidence: cert.dynamic_max };
    }
    if cert.offset_kind == OFFSET_BOUNDED_DYNAMIC {
        return Admission { outcome: Outcome::Rejected, reason: REASON_OFFSET_OUT_OF_RANGE, evidence: cert.dynamic_max };
    }
    if cert.offset_kind == OFFSET_UNBOUNDED_DYNAMIC {
        return Admission { outcome: Outcome::Unknown, reason: REASON_UNBOUNDED_OFFSET, evidence: cert.dynamic_max };
    }
    Admission { outcome: Outcome::Unknown, reason: REASON_UNSUPPORTED_PREDICATE, evidence: cert.offset_kind as u64 }
}

// r[depends verification-frontier.arena-scope-provenance]
pub open spec fn arena_provenance_spec(depth: u8) -> Provenance {
    Provenance { is_tracked: true, depth }
}

// r[impl verification-frontier.arena-scope-provenance]
pub fn arena_provenance(depth: u8) -> (result: Provenance)
    ensures result == arena_provenance_spec(depth)
{
    Provenance { is_tracked: true, depth }
}

// r[depends verification-frontier.arena-scope-provenance]
pub open spec fn untracked_heap_provenance_spec() -> Provenance {
    Provenance { is_tracked: false, depth: SCOPE_DEPTH_HEAP_OR_UNTRACKED }
}

// r[impl verification-frontier.arena-scope-provenance]
pub fn untracked_heap_provenance() -> (result: Provenance)
    ensures result == untracked_heap_provenance_spec()
{
    Provenance { is_tracked: false, depth: SCOPE_DEPTH_HEAP_OR_UNTRACKED }
}

// r[depends verification-frontier.arena-scope-provenance]
pub open spec fn allocation_inherits_arena_spec(arena: Provenance) -> Provenance {
    arena
}

// r[impl verification-frontier.arena-scope-provenance]
pub fn allocation_inherits_arena(arena: Provenance) -> (result: Provenance)
    ensures result == allocation_inherits_arena_spec(arena)
{
    arena
}

// r[depends verification-frontier.arena-scope-provenance]
pub open spec fn allocator_wrapper_inherits_arena_spec(arena: Provenance) -> Provenance {
    arena
}

// r[impl verification-frontier.arena-scope-provenance]
pub fn allocator_wrapper_inherits_arena(arena: Provenance) -> (result: Provenance)
    ensures result == allocator_wrapper_inherits_arena_spec(arena)
{
    arena
}

// r[depends verification-frontier.arena-scope-provenance]
pub open spec fn container_inherits_allocator_spec(allocator: Provenance) -> Provenance {
    allocator
}

// r[impl verification-frontier.arena-scope-provenance]
pub fn container_inherits_allocator(allocator: Provenance) -> (result: Provenance)
    ensures result == container_inherits_allocator_spec(allocator)
{
    allocator
}

// r[depends verification-frontier.arena-scope-provenance]
pub open spec fn return_escape_admission_spec(value: Provenance, boundary_depth: u8) -> Admission {
    if !value.is_tracked || value.depth <= boundary_depth {
        admission(Outcome::Proven, REASON_NONE, value.depth as u64)
    } else {
        admission(Outcome::Rejected, REASON_ARENA_ESCAPE, value.depth as u64)
    }
}

// r[impl verification-frontier.arena-scope-provenance]
pub fn return_escape_admission(value: Provenance, boundary_depth: u8) -> (result: Admission)
    ensures result == return_escape_admission_spec(value, boundary_depth)
{
    if !value.is_tracked || value.depth <= boundary_depth {
        Admission { outcome: Outcome::Proven, reason: REASON_NONE, evidence: value.depth as u64 }
    } else {
        Admission { outcome: Outcome::Rejected, reason: REASON_ARENA_ESCAPE, evidence: value.depth as u64 }
    }
}

// r[depends verification-frontier.arena-scope-provenance]
pub open spec fn store_escape_admission_spec(source: Provenance, destination: Provenance) -> Admission {
    if !source.is_tracked || (destination.is_tracked && source.depth <= destination.depth) {
        admission(Outcome::Proven, REASON_NONE, source.depth as u64)
    } else {
        admission(Outcome::Rejected, REASON_STORE_ESCAPE, source.depth as u64)
    }
}

// r[impl verification-frontier.arena-scope-provenance]
pub fn store_escape_admission(source: Provenance, destination: Provenance) -> (result: Admission)
    ensures result == store_escape_admission_spec(source, destination)
{
    if !source.is_tracked || (destination.is_tracked && source.depth <= destination.depth) {
        Admission { outcome: Outcome::Proven, reason: REASON_NONE, evidence: source.depth as u64 }
    } else {
        Admission { outcome: Outcome::Rejected, reason: REASON_STORE_ESCAPE, evidence: source.depth as u64 }
    }
}

// r[depends verification-frontier.progress-budget]
pub open spec fn progress_named_path_cost_spec(fast_ops: u8, branch_ops: u8) -> nat {
    (fast_ops as nat) * (PROGRESS_COST_FAST_OP as nat)
        + (branch_ops as nat) * (PROGRESS_COST_BRANCH_OP as nat)
}

// r[impl verification-frontier.progress-budget]
pub fn progress_named_path_cost(fast_ops: u8, branch_ops: u8) -> (result: u64)
    ensures (result as nat) == progress_named_path_cost_spec(fast_ops, branch_ops)
{
    (fast_ops as u64) * PROGRESS_COST_FAST_OP + (branch_ops as u64) * PROGRESS_COST_BRANCH_OP
}

pub open spec fn named_progress_budget_spec(budget: u64) -> bool {
    budget == PROGRESS_BUDGET_INTERACTIVE || budget == PROGRESS_BUDGET_BACKGROUND
}

pub open spec fn max_u64_spec(left: u64, right: u64) -> u64 {
    if left >= right { left } else { right }
}

// r[depends verification-frontier.progress-budget]
pub open spec fn progress_budget_admission_spec(summary: ProgressSummary, budget: u64) -> Admission {
    let worst_path = max_u64_spec(summary.main_path_cost, summary.alternate_path_cost);
    if !named_progress_budget_spec(budget) {
        admission(Outcome::Unknown, REASON_UNNAMED_BUDGET, budget)
    } else if summary.has_no_yield_cycle {
        admission(Outcome::Rejected, REASON_NO_YIELD_CYCLE, worst_path)
    } else if !summary.has_yield_or_return {
        admission(Outcome::Rejected, REASON_NO_TERMINAL_PROGRESS, worst_path)
    } else if worst_path <= budget {
        admission(Outcome::Proven, REASON_NONE, worst_path)
    } else {
        admission(Outcome::Rejected, REASON_OVER_BUDGET, worst_path)
    }
}

// r[impl verification-frontier.progress-budget]
pub fn progress_budget_admission(summary: ProgressSummary, budget: u64) -> (result: Admission)
    ensures result == progress_budget_admission_spec(summary, budget)
{
    let worst_path = if summary.main_path_cost >= summary.alternate_path_cost {
        summary.main_path_cost
    } else {
        summary.alternate_path_cost
    };
    if !(budget == PROGRESS_BUDGET_INTERACTIVE || budget == PROGRESS_BUDGET_BACKGROUND) {
        return Admission { outcome: Outcome::Unknown, reason: REASON_UNNAMED_BUDGET, evidence: budget };
    }
    if summary.has_no_yield_cycle {
        return Admission { outcome: Outcome::Rejected, reason: REASON_NO_YIELD_CYCLE, evidence: worst_path };
    }
    if !summary.has_yield_or_return {
        return Admission { outcome: Outcome::Rejected, reason: REASON_NO_TERMINAL_PROGRESS, evidence: worst_path };
    }
    if worst_path <= budget {
        return Admission { outcome: Outcome::Proven, reason: REASON_NONE, evidence: worst_path };
    }
    Admission { outcome: Outcome::Rejected, reason: REASON_OVER_BUDGET, evidence: worst_path }
}

// r[depends verification-frontier.external-reference-boundary]
pub open spec fn external_reference_boundary_admission_spec(
    claim: ExternalReferenceClaim,
) -> Admission {
    if !claim.labels_reference {
        admission(Outcome::Rejected, REASON_REFERENCE_NOT_LABELED, 0)
    } else if !claim.has_local_evidence {
        admission(Outcome::Rejected, REASON_MISSING_LOCAL_EVIDENCE, 0)
    } else if claim.treats_external_as_trusted {
        admission(Outcome::Rejected, REASON_EXTERNAL_TRUST, 0)
    } else if claim.copies_external_benchmark {
        admission(Outcome::Rejected, REASON_EXTERNAL_BENCHMARK, 0)
    } else {
        admission(Outcome::Proven, REASON_NONE, 0)
    }
}

// r[impl verification-frontier.external-reference-boundary]
pub fn external_reference_boundary_admission(
    claim: ExternalReferenceClaim,
) -> (result: Admission)
    ensures result == external_reference_boundary_admission_spec(claim)
{
    if !claim.labels_reference {
        return Admission { outcome: Outcome::Rejected, reason: REASON_REFERENCE_NOT_LABELED, evidence: 0 };
    }
    if !claim.has_local_evidence {
        return Admission { outcome: Outcome::Rejected, reason: REASON_MISSING_LOCAL_EVIDENCE, evidence: 0 };
    }
    if claim.treats_external_as_trusted {
        return Admission { outcome: Outcome::Rejected, reason: REASON_EXTERNAL_TRUST, evidence: 0 };
    }
    if claim.copies_external_benchmark {
        return Admission { outcome: Outcome::Rejected, reason: REASON_EXTERNAL_BENCHMARK, evidence: 0 };
    }
    Admission { outcome: Outcome::Proven, reason: REASON_NONE, evidence: 0 }
}

// r[depends verification-frontier.outcome-invariant-bundle]
pub open spec fn contract_implication_safety_spec(
    facts: FiniteFacts,
    precondition: Precondition,
) -> bool {
    let combined = combined_contract_fact_spec(facts, precondition.var);
    precondition.kind == PREDICATE_RANGE
        && combined.known
        && combined.lower <= combined.upper
        && combined.lower >= precondition.lower
        && combined.upper <= precondition.upper
}

// r[depends verification-frontier.outcome-invariant-bundle]
pub open spec fn slice_access_safety_spec(cert: SliceAccessCertificate) -> bool {
    cert.start <= cert.end
        && cert.end <= cert.backing_len
        && ((cert.offset_kind == OFFSET_FIXED && cert.fixed_offset < cert.end - cert.start)
            || (cert.offset_kind == OFFSET_BOUNDED_DYNAMIC && cert.dynamic_max < cert.end - cert.start))
}

// r[depends verification-frontier.outcome-invariant-bundle]
pub open spec fn return_escape_safety_spec(value: Provenance, boundary_depth: u8) -> bool {
    !value.is_tracked || value.depth <= boundary_depth
}

// r[depends verification-frontier.outcome-invariant-bundle]
pub open spec fn store_escape_safety_spec(source: Provenance, destination: Provenance) -> bool {
    !source.is_tracked || (destination.is_tracked && source.depth <= destination.depth)
}

// r[depends verification-frontier.outcome-invariant-bundle]
pub open spec fn progress_budget_safety_spec(summary: ProgressSummary, budget: u64) -> bool {
    named_progress_budget_spec(budget)
        && summary.has_yield_or_return
        && !summary.has_no_yield_cycle
        && summary.main_path_cost <= budget
        && summary.alternate_path_cost <= budget
}

// r[depends verification-frontier.outcome-invariant-bundle]
pub open spec fn external_reference_safety_spec(claim: ExternalReferenceClaim) -> bool {
    claim.labels_reference
        && claim.has_local_evidence
        && !claim.treats_external_as_trusted
        && !claim.copies_external_benchmark
}

// r[verify verification-frontier.outcome-invariant-bundle]
pub proof fn stable_outcome_variant(outcome: Outcome)
    ensures stable_outcome_variant_spec(outcome)
{
    match outcome {
        Outcome::Proven => {},
        Outcome::Rejected => {},
        Outcome::Unknown => {},
    }
}

// r[verify verification-frontier.outcome-invariant-bundle]
pub proof fn unknown_outcome_is_not_proof(outcome: Outcome)
    requires outcome == Outcome::Unknown
    ensures !outcome_is_proof_spec(outcome)
{}

// r[verify verification-frontier.outcome-invariant-bundle]
pub proof fn rejected_outcome_is_not_proof(outcome: Outcome)
    requires outcome == Outcome::Rejected
    ensures !outcome_is_proof_spec(outcome)
{}

// r[verify verification-frontier.outcome-invariant-bundle]
pub proof fn proven_contract_admission_implies_safety(
    facts: FiniteFacts,
    precondition: Precondition,
)
    requires contract_implication_admission_spec(facts, precondition).outcome == Outcome::Proven
    ensures
        contract_implication_safety_spec(facts, precondition),
        contract_admission_is_proof_evidence_spec(contract_implication_admission_spec(facts, precondition)),
{}

// r[verify verification-frontier.outcome-invariant-bundle]
pub proof fn non_proven_contract_admission_is_not_proof(
    facts: FiniteFacts,
    precondition: Precondition,
)
    requires contract_implication_admission_spec(facts, precondition).outcome != Outcome::Proven
    ensures !contract_admission_is_proof_evidence_spec(contract_implication_admission_spec(facts, precondition))
{}

// r[verify verification-frontier.outcome-invariant-bundle]
pub proof fn proven_slice_access_implies_safety(cert: SliceAccessCertificate)
    requires slice_access_admission_spec(cert).outcome == Outcome::Proven
    ensures
        slice_access_safety_spec(cert),
        admission_is_proof_evidence_spec(slice_access_admission_spec(cert)),
{}

// r[verify verification-frontier.outcome-invariant-bundle]
pub proof fn non_proven_slice_access_is_not_proof(cert: SliceAccessCertificate)
    requires slice_access_admission_spec(cert).outcome != Outcome::Proven
    ensures !admission_is_proof_evidence_spec(slice_access_admission_spec(cert))
{}

// r[verify verification-frontier.outcome-invariant-bundle]
pub proof fn proven_return_escape_implies_safety(value: Provenance, boundary_depth: u8)
    requires return_escape_admission_spec(value, boundary_depth).outcome == Outcome::Proven
    ensures
        return_escape_safety_spec(value, boundary_depth),
        admission_is_proof_evidence_spec(return_escape_admission_spec(value, boundary_depth)),
{}

// r[verify verification-frontier.outcome-invariant-bundle]
pub proof fn proven_store_escape_implies_safety(source: Provenance, destination: Provenance)
    requires store_escape_admission_spec(source, destination).outcome == Outcome::Proven
    ensures
        store_escape_safety_spec(source, destination),
        admission_is_proof_evidence_spec(store_escape_admission_spec(source, destination)),
{}

// r[verify verification-frontier.outcome-invariant-bundle]
pub proof fn proven_progress_budget_implies_safety(summary: ProgressSummary, budget: u64)
    requires progress_budget_admission_spec(summary, budget).outcome == Outcome::Proven
    ensures
        progress_budget_safety_spec(summary, budget),
        admission_is_proof_evidence_spec(progress_budget_admission_spec(summary, budget)),
{}

// r[verify verification-frontier.outcome-invariant-bundle]
pub proof fn proven_external_reference_implies_safety(claim: ExternalReferenceClaim)
    requires external_reference_boundary_admission_spec(claim).outcome == Outcome::Proven
    ensures
        external_reference_safety_spec(claim),
        admission_is_proof_evidence_spec(external_reference_boundary_admission_spec(claim)),
{}

// r[verify verification-frontier.metadata-stability]
pub proof fn contract_admission_metadata_is_stable(facts: FiniteFacts, precondition: Precondition)
    ensures contract_metadata_stable_spec(contract_implication_admission_spec(facts, precondition))
{}

// r[verify verification-frontier.metadata-stability]
pub proof fn slice_access_metadata_is_stable(cert: SliceAccessCertificate)
    ensures admission_metadata_stable_spec(slice_access_admission_spec(cert))
{}

// r[verify verification-frontier.metadata-stability]
pub proof fn return_escape_metadata_is_stable(value: Provenance, boundary_depth: u8)
    ensures admission_metadata_stable_spec(return_escape_admission_spec(value, boundary_depth))
{}

// r[verify verification-frontier.metadata-stability]
pub proof fn store_escape_metadata_is_stable(source: Provenance, destination: Provenance)
    ensures admission_metadata_stable_spec(store_escape_admission_spec(source, destination))
{}

// r[verify verification-frontier.metadata-stability]
pub proof fn progress_budget_metadata_is_stable(summary: ProgressSummary, budget: u64)
    ensures admission_metadata_stable_spec(progress_budget_admission_spec(summary, budget))
{}

// r[verify verification-frontier.metadata-stability]
pub proof fn external_reference_metadata_is_stable(claim: ExternalReferenceClaim)
    ensures admission_metadata_stable_spec(external_reference_boundary_admission_spec(claim))
{}

// r[verify verification-frontier.contract-implication]
pub proof fn proven_contract_implication_has_covering_facts(
    facts: FiniteFacts,
    precondition: Precondition,
)
    requires contract_implication_admission_spec(facts, precondition).outcome == Outcome::Proven
    ensures
        precondition.kind == PREDICATE_RANGE,
        combined_contract_fact_spec(facts, precondition.var).known,
        combined_contract_fact_spec(facts, precondition.var).lower >= precondition.lower,
        combined_contract_fact_spec(facts, precondition.var).upper <= precondition.upper,
{}

// r[verify verification-frontier.contract-implication]
pub proof fn unknown_contract_implication_is_not_proof(
    facts: FiniteFacts,
    precondition: Precondition,
)
    requires contract_implication_admission_spec(facts, precondition).outcome == Outcome::Unknown
    ensures !outcome_is_proof_spec(contract_implication_admission_spec(facts, precondition).outcome)
{}

// r[verify verification-frontier.type-bound-facts]
pub proof fn normalized_range_preserves_stricter_caller_bound(caller: RangeFact, type_bound: RangeFact)
    requires
        caller.known,
        type_bound.known,
        caller.var == type_bound.var,
        caller.lower >= type_bound.lower,
        caller.upper <= type_bound.upper,
    ensures normalize_range_fact_spec(caller, type_bound) == caller
{}

// r[verify verification-frontier.slice-access-certificate]
pub proof fn proven_slice_access_has_in_bounds_certificate(cert: SliceAccessCertificate)
    requires slice_access_admission_spec(cert).outcome == Outcome::Proven
    ensures
        cert.start <= cert.end,
        cert.end <= cert.backing_len,
        (cert.offset_kind == OFFSET_FIXED ==> cert.fixed_offset < cert.end - cert.start),
        (cert.offset_kind == OFFSET_BOUNDED_DYNAMIC ==> cert.dynamic_max < cert.end - cert.start),
{}

// r[verify verification-frontier.arena-scope-provenance]
pub proof fn local_arena_return_escape_is_rejected(boundary_depth: u8)
    requires boundary_depth < SCOPE_DEPTH_LOCAL
    ensures return_escape_admission_spec(
        Provenance { is_tracked: true, depth: SCOPE_DEPTH_LOCAL },
        boundary_depth,
    ).outcome == Outcome::Rejected
{}

// r[verify verification-frontier.arena-scope-provenance]
pub proof fn deeper_store_escape_is_rejected(source_depth: u8, destination_depth: u8)
    requires source_depth > destination_depth
    ensures store_escape_admission_spec(
        Provenance { is_tracked: true, depth: source_depth },
        Provenance { is_tracked: true, depth: destination_depth },
    ).outcome == Outcome::Rejected
{}

// r[verify verification-frontier.progress-budget]
pub proof fn proven_progress_budget_has_terminal_bounded_paths(summary: ProgressSummary, budget: u64)
    requires progress_budget_admission_spec(summary, budget).outcome == Outcome::Proven
    ensures
        named_progress_budget_spec(budget),
        summary.has_yield_or_return,
        !summary.has_no_yield_cycle,
        summary.main_path_cost <= budget,
        summary.alternate_path_cost <= budget,
{}

// r[verify verification-frontier.external-reference-boundary]
pub proof fn admitted_external_reference_claim_uses_local_evidence(claim: ExternalReferenceClaim)
    requires external_reference_boundary_admission_spec(claim).outcome == Outcome::Proven
    ensures
        claim.labels_reference,
        claim.has_local_evidence,
        !claim.treats_external_as_trusted,
        !claim.copies_external_benchmark,
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    const CALLER_LOWER_STRICT: i64 = 10;
    const CALLER_UPPER_STRICT: i64 = 20;
    const PRECONDITION_LOWER: i64 = 5;
    const PRECONDITION_UPPER: i64 = 25;
    const COUNTEREXAMPLE_LOWER: i64 = 3;
    const OFFSET_START: u64 = 2;
    const OFFSET_END: u64 = 8;
    const BACKING_LEN: u64 = 16;
    const FIXED_OFFSET_SAFE: u64 = 3;
    const FIXED_OFFSET_PAST_END: u64 = 7;
    const DYNAMIC_MAX_SAFE: u64 = 5;
    const DYNAMIC_MAX_PAST_END: u64 = 6;
    const FAST_OP_COUNT: u8 = 2;
    const BRANCH_OP_COUNT: u8 = 1;
    const MANY_FAST_OPS: u8 = 9;
    const NO_BRANCH_OPS: u8 = 0;
    const INCONSISTENT_LOWER: i64 = 30;
    const INCONSISTENT_UPPER: i64 = 20;
    const UNSUPPORTED_OFFSET_KIND: u8 = 99;
    const UNNAMED_PROGRESS_BUDGET: u64 = PROGRESS_COST_FAST_OP;

    fn fact(var: u8, lower: i64, upper: i64) -> RangeFact {
        RangeFact {
            var,
            lower,
            upper,
            known: true,
        }
    }

    fn no_fact(var: u8) -> RangeFact {
        RangeFact {
            var,
            lower: 0,
            upper: 0,
            known: false,
        }
    }

    fn empty_facts(var: u8) -> FiniteFacts {
        FiniteFacts {
            caller: no_fact(var),
            path: no_fact(var),
            type_bound: no_fact(var),
        }
    }

    fn range_precondition() -> Precondition {
        Precondition {
            kind: PREDICATE_RANGE,
            var: VARIABLE_PRIMARY,
            lower: PRECONDITION_LOWER,
            upper: PRECONDITION_UPPER,
        }
    }

    #[test]
    fn contract_implication_proves_implied_precondition() {
        let precondition = range_precondition();
        let facts = FiniteFacts {
            caller: fact(VARIABLE_PRIMARY, CALLER_LOWER_STRICT, CALLER_UPPER_STRICT),
            path: no_fact(VARIABLE_PRIMARY),
            type_bound: type_bound_fact(DOMAIN_U8, VARIABLE_PRIMARY),
        };
        let decision = contract_implication_admission(facts, precondition);
        assert_eq!(decision.outcome, Outcome::Proven);
        assert!(outcome_is_proof(decision.outcome));
    }

    #[test]
    fn contract_implication_rejects_counterexample() {
        let precondition = range_precondition();
        let facts = FiniteFacts {
            caller: fact(VARIABLE_PRIMARY, COUNTEREXAMPLE_LOWER, CALLER_UPPER_STRICT),
            path: no_fact(VARIABLE_PRIMARY),
            type_bound: no_fact(VARIABLE_PRIMARY),
        };
        let decision = contract_implication_admission(facts, precondition);
        assert_eq!(decision.outcome, Outcome::Rejected);
        assert_eq!(decision.reason, REASON_LOWER_BOUND_COUNTEREXAMPLE);
        assert_eq!(decision.witness_value, COUNTEREXAMPLE_LOWER);
    }

    #[test]
    fn contract_implication_returns_unknown_for_missing_or_unsupported_facts() {
        let missing = contract_implication_admission(empty_facts(VARIABLE_PRIMARY), range_precondition());
        assert_eq!(missing.outcome, Outcome::Unknown);
        assert!(!outcome_is_proof(missing.outcome));

        let unsupported = contract_implication_admission(
            FiniteFacts {
                caller: fact(VARIABLE_PRIMARY, CALLER_LOWER_STRICT, CALLER_UPPER_STRICT),
                path: no_fact(VARIABLE_PRIMARY),
                type_bound: no_fact(VARIABLE_PRIMARY),
            },
            Precondition {
                kind: PREDICATE_UNSUPPORTED,
                var: VARIABLE_PRIMARY,
                lower: PRECONDITION_LOWER,
                upper: PRECONDITION_UPPER,
            },
        );
        assert_eq!(unsupported.outcome, Outcome::Unknown);
    }

    #[test]
    fn type_bound_facts_distinguish_domains_and_preserve_stricter_bounds() {
        let unsigned = type_bound_fact(DOMAIN_U8, VARIABLE_PRIMARY);
        assert_eq!(unsigned.lower, DOMAIN_U8_MIN);
        assert_eq!(unsigned.upper, DOMAIN_U8_MAX);

        let signed = type_bound_fact(DOMAIN_I8, VARIABLE_PRIMARY);
        assert_eq!(signed.lower, DOMAIN_I8_MIN);
        assert_eq!(signed.upper, DOMAIN_I8_MAX);

        let caller = fact(VARIABLE_PRIMARY, CALLER_LOWER_STRICT, CALLER_UPPER_STRICT);
        assert_eq!(normalize_range_fact(caller, unsigned), caller);
    }

    #[test]
    fn slice_access_accepts_fixed_and_bounded_dynamic_offsets() {
        let fixed = SliceAccessCertificate {
            backing_len: BACKING_LEN,
            start: OFFSET_START,
            end: OFFSET_END,
            offset_kind: OFFSET_FIXED,
            fixed_offset: FIXED_OFFSET_SAFE,
            dynamic_max: 0,
        };
        assert_eq!(slice_access_admission(fixed).outcome, Outcome::Proven);

        let dynamic = SliceAccessCertificate {
            offset_kind: OFFSET_BOUNDED_DYNAMIC,
            fixed_offset: 0,
            dynamic_max: DYNAMIC_MAX_SAFE,
            ..fixed
        };
        assert_eq!(slice_access_admission(dynamic).outcome, Outcome::Proven);
    }

    #[test]
    fn slice_access_rejects_inverted_out_of_backing_and_unbounded_offsets() {
        let valid = SliceAccessCertificate {
            backing_len: BACKING_LEN,
            start: OFFSET_START,
            end: OFFSET_END,
            offset_kind: OFFSET_FIXED,
            fixed_offset: FIXED_OFFSET_SAFE,
            dynamic_max: 0,
        };
        let inverted = SliceAccessCertificate {
            start: OFFSET_END,
            end: OFFSET_START,
            ..valid
        };
        assert_eq!(slice_access_admission(inverted).reason, REASON_INVERTED_RANGE);

        let out_of_backing = SliceAccessCertificate {
            end: BACKING_LEN + PROGRESS_COST_FAST_OP,
            ..valid
        };
        assert_eq!(slice_access_admission(out_of_backing).reason, REASON_OUT_OF_BACKING);

        let bad_fixed = SliceAccessCertificate {
            fixed_offset: FIXED_OFFSET_PAST_END,
            ..valid
        };
        assert_eq!(slice_access_admission(bad_fixed).reason, REASON_OFFSET_OUT_OF_RANGE);

        let bad_dynamic = SliceAccessCertificate {
            offset_kind: OFFSET_BOUNDED_DYNAMIC,
            dynamic_max: DYNAMIC_MAX_PAST_END,
            ..valid
        };
        assert_eq!(slice_access_admission(bad_dynamic).reason, REASON_OFFSET_OUT_OF_RANGE);

        let unbounded = SliceAccessCertificate {
            offset_kind: OFFSET_UNBOUNDED_DYNAMIC,
            ..valid
        };
        assert_eq!(slice_access_admission(unbounded).outcome, Outcome::Unknown);
    }

    #[test]
    fn arena_provenance_tracks_inheritance_and_return_escape() {
        let caller_arena = arena_provenance(SCOPE_DEPTH_CALLER);
        let allocation = allocation_inherits_arena(caller_arena);
        assert_eq!(return_escape_admission(allocation, SCOPE_DEPTH_CALLER).outcome, Outcome::Proven);

        let local_arena = arena_provenance(SCOPE_DEPTH_LOCAL);
        let local_allocation = allocation_inherits_arena(local_arena);
        assert_eq!(return_escape_admission(local_allocation, SCOPE_DEPTH_CALLER).reason, REASON_ARENA_ESCAPE);

        let heap_value = untracked_heap_provenance();
        assert_eq!(return_escape_admission(heap_value, SCOPE_DEPTH_CALLER).outcome, Outcome::Proven);
    }

    #[test]
    fn arena_provenance_covers_wrappers_containers_and_store_escape() {
        let local_container =
            container_inherits_allocator(allocator_wrapper_inherits_arena(arena_provenance(SCOPE_DEPTH_NESTED_LOCAL)));
        assert_eq!(local_container.depth, SCOPE_DEPTH_NESTED_LOCAL);

        let nested_value = allocation_inherits_arena(arena_provenance(SCOPE_DEPTH_NESTED_LOCAL));
        let local_destination = arena_provenance(SCOPE_DEPTH_LOCAL);
        assert_eq!(store_escape_admission(nested_value, local_destination).reason, REASON_STORE_ESCAPE);

        let caller_value = allocation_inherits_arena(arena_provenance(SCOPE_DEPTH_CALLER));
        assert_eq!(store_escape_admission(caller_value, local_destination).outcome, Outcome::Proven);

        let heap_value = untracked_heap_provenance();
        assert_eq!(store_escape_admission(heap_value, local_destination).outcome, Outcome::Proven);
    }

    #[test]
    fn progress_budget_accepts_named_bounded_paths() {
        let main_path_cost = progress_named_path_cost(FAST_OP_COUNT, BRANCH_OP_COUNT);
        let alternate_path_cost = progress_named_path_cost(FAST_OP_COUNT, NO_BRANCH_OPS);
        let summary = ProgressSummary {
            main_path_cost,
            alternate_path_cost,
            has_yield_or_return: true,
            has_no_yield_cycle: false,
        };
        assert_eq!(progress_budget_admission(summary, PROGRESS_BUDGET_INTERACTIVE).outcome, Outcome::Proven);
    }

    #[test]
    fn progress_budget_rejects_over_budget_and_no_yield_cycles() {
        let over_budget = ProgressSummary {
            main_path_cost: progress_named_path_cost(MANY_FAST_OPS, NO_BRANCH_OPS),
            alternate_path_cost: progress_named_path_cost(FAST_OP_COUNT, NO_BRANCH_OPS),
            has_yield_or_return: true,
            has_no_yield_cycle: false,
        };
        assert_eq!(progress_budget_admission(over_budget, PROGRESS_BUDGET_INTERACTIVE).reason, REASON_OVER_BUDGET);

        let no_yield_cycle = ProgressSummary {
            has_no_yield_cycle: true,
            ..over_budget
        };
        assert_eq!(progress_budget_admission(no_yield_cycle, PROGRESS_BUDGET_BACKGROUND).reason, REASON_NO_YIELD_CYCLE);

        let no_terminal = ProgressSummary {
            has_yield_or_return: false,
            has_no_yield_cycle: false,
            ..over_budget
        };
        assert_eq!(
            progress_budget_admission(no_terminal, PROGRESS_BUDGET_BACKGROUND).reason,
            REASON_NO_TERMINAL_PROGRESS
        );

        assert_eq!(progress_budget_admission(over_budget, PROGRESS_COST_FAST_OP).outcome, Outcome::Unknown);
    }

    #[test]
    fn external_reference_boundary_requires_local_evidence_not_external_trust() {
        let accepted = ExternalReferenceClaim {
            labels_reference: true,
            has_local_evidence: true,
            treats_external_as_trusted: false,
            copies_external_benchmark: false,
        };
        assert_eq!(external_reference_boundary_admission(accepted).outcome, Outcome::Proven);

        let external_trust = ExternalReferenceClaim {
            treats_external_as_trusted: true,
            ..accepted
        };
        assert_eq!(external_reference_boundary_admission(external_trust).reason, REASON_EXTERNAL_TRUST);

        let copied_benchmark = ExternalReferenceClaim {
            copies_external_benchmark: true,
            ..accepted
        };
        assert_eq!(external_reference_boundary_admission(copied_benchmark).reason, REASON_EXTERNAL_BENCHMARK);

        let missing_evidence = ExternalReferenceClaim {
            has_local_evidence: false,
            ..accepted
        };
        assert_eq!(external_reference_boundary_admission(missing_evidence).reason, REASON_MISSING_LOCAL_EVIDENCE);

        let unlabeled = ExternalReferenceClaim {
            labels_reference: false,
            ..accepted
        };
        assert_eq!(external_reference_boundary_admission(unlabeled).reason, REASON_REFERENCE_NOT_LABELED);
    }

    // r[verify verification-frontier.outcome-invariant-bundle]
    #[test]
    fn outcome_invariant_bundle_accepts_proven_admission_families() {
        let precondition = range_precondition();
        let facts = FiniteFacts {
            caller: fact(VARIABLE_PRIMARY, CALLER_LOWER_STRICT, CALLER_UPPER_STRICT),
            path: no_fact(VARIABLE_PRIMARY),
            type_bound: type_bound_fact(DOMAIN_U8, VARIABLE_PRIMARY),
        };
        let contract = contract_implication_admission(facts, precondition);
        assert!(outcome_is_proof(contract.outcome));
        assert_eq!(contract.reason, REASON_NONE);

        let slice = slice_access_admission(SliceAccessCertificate {
            backing_len: BACKING_LEN,
            start: OFFSET_START,
            end: OFFSET_END,
            offset_kind: OFFSET_FIXED,
            fixed_offset: FIXED_OFFSET_SAFE,
            dynamic_max: 0,
        });
        assert!(outcome_is_proof(slice.outcome));
        assert_eq!(slice.reason, REASON_NONE);

        let caller_value = arena_provenance(SCOPE_DEPTH_CALLER);
        assert!(outcome_is_proof(return_escape_admission(caller_value, SCOPE_DEPTH_CALLER).outcome));
        let local_destination = arena_provenance(SCOPE_DEPTH_LOCAL);
        assert!(outcome_is_proof(store_escape_admission(caller_value, local_destination).outcome));

        let bounded_progress = ProgressSummary {
            main_path_cost: progress_named_path_cost(FAST_OP_COUNT, BRANCH_OP_COUNT),
            alternate_path_cost: progress_named_path_cost(FAST_OP_COUNT, NO_BRANCH_OPS),
            has_yield_or_return: true,
            has_no_yield_cycle: false,
        };
        assert!(outcome_is_proof(progress_budget_admission(bounded_progress, PROGRESS_BUDGET_INTERACTIVE).outcome));

        let local_reference = ExternalReferenceClaim {
            labels_reference: true,
            has_local_evidence: true,
            treats_external_as_trusted: false,
            copies_external_benchmark: false,
        };
        assert!(outcome_is_proof(external_reference_boundary_admission(local_reference).outcome));
    }

    // r[verify verification-frontier.metadata-stability]
    #[test]
    fn metadata_stability_reports_named_reasons_and_input_evidence() {
        let unsupported = contract_implication_admission(
            FiniteFacts {
                caller: fact(VARIABLE_PRIMARY, CALLER_LOWER_STRICT, CALLER_UPPER_STRICT),
                path: no_fact(VARIABLE_PRIMARY),
                type_bound: no_fact(VARIABLE_PRIMARY),
            },
            Precondition {
                kind: PREDICATE_UNSUPPORTED,
                var: VARIABLE_PRIMARY,
                lower: PRECONDITION_LOWER,
                upper: PRECONDITION_UPPER,
            },
        );
        assert_eq!(unsupported.outcome, Outcome::Unknown);
        assert_eq!(unsupported.reason, REASON_UNSUPPORTED_PREDICATE);
        assert_eq!(unsupported.witness_var, VARIABLE_PRIMARY);
        assert_eq!(unsupported.witness_value, PRECONDITION_LOWER);
        assert!(!outcome_is_proof(unsupported.outcome));

        let inconsistent = contract_implication_admission(
            FiniteFacts {
                caller: fact(VARIABLE_PRIMARY, INCONSISTENT_LOWER, INCONSISTENT_UPPER),
                path: no_fact(VARIABLE_PRIMARY),
                type_bound: no_fact(VARIABLE_PRIMARY),
            },
            range_precondition(),
        );
        assert_eq!(inconsistent.outcome, Outcome::Rejected);
        assert_eq!(inconsistent.reason, REASON_INCONSISTENT_FACTS);
        assert_eq!(inconsistent.witness_value, INCONSISTENT_LOWER);
        assert!(!outcome_is_proof(inconsistent.outcome));

        let base_cert = SliceAccessCertificate {
            backing_len: BACKING_LEN,
            start: OFFSET_START,
            end: OFFSET_END,
            offset_kind: OFFSET_FIXED,
            fixed_offset: FIXED_OFFSET_SAFE,
            dynamic_max: DYNAMIC_MAX_SAFE,
        };
        let unbounded = slice_access_admission(SliceAccessCertificate {
            offset_kind: OFFSET_UNBOUNDED_DYNAMIC,
            ..base_cert
        });
        assert_eq!(unbounded.outcome, Outcome::Unknown);
        assert_eq!(unbounded.reason, REASON_UNBOUNDED_OFFSET);
        assert_eq!(unbounded.evidence, DYNAMIC_MAX_SAFE);

        let unsupported_offset = slice_access_admission(SliceAccessCertificate {
            offset_kind: UNSUPPORTED_OFFSET_KIND,
            ..base_cert
        });
        assert_eq!(unsupported_offset.outcome, Outcome::Unknown);
        assert_eq!(unsupported_offset.reason, REASON_UNSUPPORTED_PREDICATE);
        assert_eq!(unsupported_offset.evidence, u64::from(UNSUPPORTED_OFFSET_KIND));

        let over_budget = ProgressSummary {
            main_path_cost: progress_named_path_cost(MANY_FAST_OPS, NO_BRANCH_OPS),
            alternate_path_cost: progress_named_path_cost(FAST_OP_COUNT, NO_BRANCH_OPS),
            has_yield_or_return: true,
            has_no_yield_cycle: false,
        };
        let unnamed_budget = progress_budget_admission(over_budget, UNNAMED_PROGRESS_BUDGET);
        assert_eq!(unnamed_budget.outcome, Outcome::Unknown);
        assert_eq!(unnamed_budget.reason, REASON_UNNAMED_BUDGET);
        assert_eq!(unnamed_budget.evidence, UNNAMED_PROGRESS_BUDGET);

        let no_yield_cycle = progress_budget_admission(
            ProgressSummary {
                has_no_yield_cycle: true,
                ..over_budget
            },
            PROGRESS_BUDGET_BACKGROUND,
        );
        assert_eq!(no_yield_cycle.outcome, Outcome::Rejected);
        assert_eq!(no_yield_cycle.reason, REASON_NO_YIELD_CYCLE);
        assert_eq!(no_yield_cycle.evidence, over_budget.main_path_cost);

        let trusted_external = external_reference_boundary_admission(ExternalReferenceClaim {
            labels_reference: true,
            has_local_evidence: true,
            treats_external_as_trusted: true,
            copies_external_benchmark: false,
        });
        assert_eq!(trusted_external.outcome, Outcome::Rejected);
        assert_eq!(trusted_external.reason, REASON_EXTERNAL_TRUST);
        assert_eq!(trusted_external.evidence, 0);
    }
}
