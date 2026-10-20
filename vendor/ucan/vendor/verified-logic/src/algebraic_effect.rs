//! Finite algebraic-effect operation, handler, and continuation primitives.
//!
//! This module models algebraic effects as finite `(effect, operation)` call
//! identifiers. Signatures declare admitted operations, handlers discharge calls
//! for one effect only when a matching clause exists, and continuation tokens are
//! consumed at most once. No runtime stack capture, typechecker correctness, or
//! effect execution semantics are claimed by these predicates.

use vstd::prelude::*;

verus! {

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AlgebraicEffectOperation {
    pub effect: u64,
    pub operation: u64,
    pub input_arity: u64,
    pub output_arity: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EffectCall {
    pub effect: u64,
    pub operation: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EffectInvocation {
    pub effect: u64,
    pub operation: u64,
    pub input_count: u64,
    pub output_count: u64,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AlgebraicEffectSignature {
    pub operations: Vec<AlgebraicEffectOperation>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EffectRow {
    pub effects: Vec<u64>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EffectSummary {
    pub required: EffectRow,
    pub residual: EffectRow,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AlgebraicHandler {
    pub handled_effect: u64,
    pub clauses: Vec<u64>,
    pub available: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ContinuationToken {
    pub token: u64,
    pub effect: u64,
    pub used: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OperationResumeMode {
    pub effect: u64,
    pub operation: u64,
    pub resumable: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct HandlerClauseResume {
    pub handled_effect: u64,
    pub operation: u64,
    pub resumes: bool,
}

// r[depends domain-primitives.algebraic-effect.signature]
pub open spec fn operation_matches_call_spec(operation: AlgebraicEffectOperation, call: EffectCall) -> bool {
    operation.effect == call.effect && operation.operation == call.operation
}

// r[depends domain-primitives.algebraic-effect.arity]
pub open spec fn operation_matches_invocation_spec(operation: AlgebraicEffectOperation, invocation: EffectInvocation) -> bool {
    operation.effect == invocation.effect
        && operation.operation == invocation.operation
        && operation.input_arity == invocation.input_count
        && operation.output_arity == invocation.output_count
}

// r[depends domain-primitives.algebraic-effect.arity]
pub open spec fn invocation_call_spec(invocation: EffectInvocation) -> EffectCall {
    EffectCall { effect: invocation.effect, operation: invocation.operation }
}

// r[depends domain-primitives.algebraic-effect.signature]
pub open spec fn signature_declares_call_spec(operations: Seq<AlgebraicEffectOperation>, call: EffectCall) -> bool {
    exists|i: int| 0 <= i < operations.len() && operation_matches_call_spec(#[trigger] operations[i], call)
}

// r[depends domain-primitives.algebraic-effect.signature]
pub open spec fn call_admitted_by_signature_spec(signature: AlgebraicEffectSignature, call: EffectCall) -> bool {
    signature_declares_call_spec(signature.operations@, call)
}

// r[depends domain-primitives.algebraic-effect.arity]
pub open spec fn signature_declares_invocation_spec(operations: Seq<AlgebraicEffectOperation>, invocation: EffectInvocation) -> bool {
    exists|i: int| 0 <= i < operations.len() && operation_matches_invocation_spec(#[trigger] operations[i], invocation)
}

// r[depends domain-primitives.algebraic-effect.arity]
pub open spec fn invocation_admitted_by_signature_spec(signature: AlgebraicEffectSignature, invocation: EffectInvocation) -> bool {
    signature_declares_invocation_spec(signature.operations@, invocation)
}

// r[depends domain-primitives.algebraic-effect.effect-row]
pub open spec fn effect_row_contains_spec(row: EffectRow, effect: u64) -> bool {
    exists|i: int| 0 <= i < row.effects@.len() && row.effects@[i] == effect
}

// r[depends domain-primitives.algebraic-effect.effect-row]
pub open spec fn call_admitted_by_row_spec(row: EffectRow, call: EffectCall) -> bool {
    effect_row_contains_spec(row, call.effect)
}

// r[depends domain-primitives.algebraic-effect.effect-row]
pub open spec fn handler_targets_row_effect_spec(row: EffectRow, handler: AlgebraicHandler) -> bool {
    handler.available && effect_row_contains_spec(row, handler.handled_effect)
}

// r[depends domain-primitives.algebraic-effect.effect-row]
pub open spec fn remove_effect_prefix_spec(effects: Seq<u64>, effect: u64, upto: int) -> Seq<u64>
    recommends 0 <= upto <= effects.len(),
    decreases upto,
{
    if upto <= 0 {
        seq![]
    } else {
        let prev = remove_effect_prefix_spec(effects, effect, upto - 1);
        let current = effects[upto - 1];
        if current == effect {
            prev
        } else {
            prev.push(current)
        }
    }
}

// r[depends domain-primitives.algebraic-effect.effect-row]
pub open spec fn remove_effect_from_row_spec(row: EffectRow, effect: u64) -> Seq<u64> {
    remove_effect_prefix_spec(row.effects@, effect, row.effects@.len() as int)
}

// r[depends domain-primitives.algebraic-effect.effect-summary]
pub open spec fn row_contains_all_effects_spec(row: EffectRow, required: EffectRow) -> bool {
    forall|i: int| 0 <= i < required.effects@.len() ==> effect_row_contains_spec(row, #[trigger] required.effects@[i])
}

// r[depends domain-primitives.algebraic-effect.effect-summary]
pub open spec fn summary_admitted_by_row_spec(row: EffectRow, summary: EffectSummary) -> bool {
    row_contains_all_effects_spec(row, summary.required)
}

// r[depends domain-primitives.algebraic-effect.effect-summary]
pub open spec fn summary_composition_admitted_spec(left: EffectSummary, right: EffectSummary) -> bool {
    summary_admitted_by_row_spec(left.residual, right)
}

// r[depends domain-primitives.algebraic-effect.effect-summary]
pub open spec fn compose_effect_summaries_required_spec(left: EffectSummary, right: EffectSummary) -> Seq<u64> {
    left.required.effects@
}

// r[depends domain-primitives.algebraic-effect.effect-summary]
pub open spec fn compose_effect_summaries_residual_spec(left: EffectSummary, right: EffectSummary) -> Seq<u64> {
    right.residual.effects@
}

// r[depends domain-primitives.algebraic-effect.summary-handler-application]
pub open spec fn handler_admitted_for_summary_spec(summary: EffectSummary, handler: AlgebraicHandler) -> bool {
    handler.available && effect_row_contains_spec(summary.required, handler.handled_effect)
}

// r[depends domain-primitives.algebraic-effect.summary-handler-application]
pub open spec fn apply_handler_to_summary_required_spec(summary: EffectSummary, handler: AlgebraicHandler) -> Seq<u64> {
    summary.required.effects@
}

// r[depends domain-primitives.algebraic-effect.summary-handler-application]
pub open spec fn apply_handler_to_summary_residual_spec(summary: EffectSummary, handler: AlgebraicHandler) -> Seq<u64> {
    remove_effect_from_row_spec(summary.residual, handler.handled_effect)
}

// r[depends domain-primitives.algebraic-effect.handler-coverage]
pub open spec fn handler_has_clause_spec(handler: AlgebraicHandler, operation: u64) -> bool {
    exists|i: int| 0 <= i < handler.clauses@.len() && handler.clauses@[i] == operation
}

// r[depends domain-primitives.algebraic-effect.handler-coverage]
pub open spec fn handler_covers_signature_effect_spec(signature: AlgebraicEffectSignature, handler: AlgebraicHandler) -> bool {
    handler.available
        && forall|i: int| 0 <= i < signature.operations@.len()
            && signature.operations@[i].effect == handler.handled_effect
            ==> handler_has_clause_spec(handler, #[trigger] signature.operations@[i].operation)
}

// r[depends domain-primitives.algebraic-effect.handler-coverage]
pub open spec fn call_handled_by_handler_spec(call: EffectCall, handler: AlgebraicHandler) -> bool {
    handler.available && call.effect == handler.handled_effect && handler_has_clause_spec(handler, call.operation)
}

// r[depends domain-primitives.algebraic-effect.handler-composition]
pub open spec fn handler_clauses_disjoint_spec(left: AlgebraicHandler, right: AlgebraicHandler) -> bool {
    forall|i: int, j: int| 0 <= i < left.clauses@.len() && 0 <= j < right.clauses@.len()
        ==> left.clauses@[i] != right.clauses@[j]
}

// r[depends domain-primitives.algebraic-effect.handler-composition]
pub open spec fn handler_composition_admitted_spec(left: AlgebraicHandler, right: AlgebraicHandler) -> bool {
    left.available
        && right.available
        && left.handled_effect == right.handled_effect
        && handler_clauses_disjoint_spec(left, right)
}

// r[depends domain-primitives.algebraic-effect.handler-composition]
pub open spec fn composed_handler_clauses_spec(left: AlgebraicHandler, right: AlgebraicHandler) -> Seq<u64> {
    left.clauses@ + right.clauses@
}

// r[depends domain-primitives.algebraic-effect.handler-discharge]
pub open spec fn discharge_call_prefix_spec(calls: Seq<EffectCall>, handler: AlgebraicHandler, upto: int) -> Seq<EffectCall>
    recommends 0 <= upto <= calls.len(),
    decreases upto,
{
    if upto <= 0 {
        seq![]
    } else {
        let prev = discharge_call_prefix_spec(calls, handler, upto - 1);
        let call = calls[upto - 1];
        if call_handled_by_handler_spec(call, handler) {
            prev
        } else {
            prev.push(call)
        }
    }
}

// r[depends domain-primitives.algebraic-effect.handler-discharge]
pub open spec fn discharge_effect_calls_spec(calls: Seq<EffectCall>, handler: AlgebraicHandler) -> Seq<EffectCall> {
    discharge_call_prefix_spec(calls, handler, calls.len() as int)
}

// r[depends domain-primitives.algebraic-effect.continuation-token]
pub open spec fn continuation_resume_admitted_spec(token: ContinuationToken, handler: AlgebraicHandler) -> bool {
    !token.used && handler.available && token.effect == handler.handled_effect
}

// r[depends domain-primitives.algebraic-effect.continuation-resume-mode]
pub open spec fn operation_resume_mode_matches_call_spec(mode: OperationResumeMode, call: EffectCall) -> bool {
    mode.effect == call.effect && mode.operation == call.operation
}

// r[depends domain-primitives.algebraic-effect.continuation-resume-mode]
pub open spec fn continuation_mode_admitted_spec(token: ContinuationToken, handler: AlgebraicHandler, mode: OperationResumeMode) -> bool {
    continuation_resume_admitted_spec(token, handler)
        && token.effect == mode.effect
        && mode.resumable
}

// r[depends domain-primitives.algebraic-effect.continuation-resume-mode]
pub open spec fn handler_clause_resume_compatible_spec(mode: OperationResumeMode, clause: HandlerClauseResume) -> bool {
    mode.effect == clause.handled_effect
        && mode.operation == clause.operation
        && (!clause.resumes || mode.resumable)
}

// r[depends domain-primitives.algebraic-effect.continuation-token]
pub open spec fn consume_continuation_spec(token: ContinuationToken) -> ContinuationToken {
    ContinuationToken { used: true, ..token }
}

// r[impl domain-primitives.algebraic-effect.signature]
pub fn operation_matches_call(operation: AlgebraicEffectOperation, call: EffectCall) -> (result: bool)
    ensures result == operation_matches_call_spec(operation, call)
{
    operation.effect == call.effect && operation.operation == call.operation
}

// r[impl domain-primitives.algebraic-effect.arity]
pub fn operation_matches_invocation(operation: AlgebraicEffectOperation, invocation: EffectInvocation) -> (result: bool)
    ensures result == operation_matches_invocation_spec(operation, invocation)
{
    operation.effect == invocation.effect
        && operation.operation == invocation.operation
        && operation.input_arity == invocation.input_count
        && operation.output_arity == invocation.output_count
}

// r[impl domain-primitives.algebraic-effect.arity]
pub fn invocation_call(invocation: EffectInvocation) -> (result: EffectCall)
    ensures result == invocation_call_spec(invocation)
{
    EffectCall { effect: invocation.effect, operation: invocation.operation }
}

// r[impl domain-primitives.algebraic-effect.signature]
pub fn call_admitted_by_signature(signature: &AlgebraicEffectSignature, call: EffectCall) -> (result: bool)
    ensures result == call_admitted_by_signature_spec(*signature, call)
{
    let mut i: usize = 0;
    while i < signature.operations.len()
        invariant
            0 <= i <= signature.operations.len(),
            forall|k: int| 0 <= k < i ==> !operation_matches_call_spec(signature.operations@[k], call),
        decreases signature.operations.len() - i,
    {
        if operation_matches_call(signature.operations[i], call) {
            assert(signature_declares_call_spec(signature.operations@, call));
            return true;
        }
        i = i + 1;
    }
    assert forall|k: int| 0 <= k < signature.operations@.len() implies !operation_matches_call_spec(signature.operations@[k], call) by {
        assert(0 <= k < i);
    }
    assert(!signature_declares_call_spec(signature.operations@, call));
    false
}

// r[impl domain-primitives.algebraic-effect.arity]
pub fn invocation_admitted_by_signature(signature: &AlgebraicEffectSignature, invocation: EffectInvocation) -> (result: bool)
    ensures result == invocation_admitted_by_signature_spec(*signature, invocation)
{
    let mut i: usize = 0;
    while i < signature.operations.len()
        invariant
            0 <= i <= signature.operations.len(),
            forall|k: int| 0 <= k < i ==> !operation_matches_invocation_spec(signature.operations@[k], invocation),
        decreases signature.operations.len() - i,
    {
        if operation_matches_invocation(signature.operations[i], invocation) {
            assert(signature_declares_invocation_spec(signature.operations@, invocation));
            return true;
        }
        i = i + 1;
    }
    assert forall|k: int| 0 <= k < signature.operations@.len() implies !operation_matches_invocation_spec(signature.operations@[k], invocation) by {
        assert(0 <= k < i);
    }
    assert(!signature_declares_invocation_spec(signature.operations@, invocation));
    false
}

// r[impl domain-primitives.algebraic-effect.effect-row]
pub fn effect_row_contains(row: &EffectRow, effect: u64) -> (result: bool)
    ensures result == effect_row_contains_spec(*row, effect)
{
    let mut i: usize = 0;
    while i < row.effects.len()
        invariant
            0 <= i <= row.effects.len(),
            forall|k: int| 0 <= k < i ==> row.effects@[k] != effect,
        decreases row.effects.len() - i,
    {
        if row.effects[i] == effect {
            assert(effect_row_contains_spec(*row, effect));
            return true;
        }
        i = i + 1;
    }
    assert forall|k: int| 0 <= k < row.effects@.len() implies row.effects@[k] != effect by {
        assert(0 <= k < i);
    }
    assert(!effect_row_contains_spec(*row, effect));
    false
}

// r[impl domain-primitives.algebraic-effect.effect-row]
pub fn call_admitted_by_row(row: &EffectRow, call: EffectCall) -> (result: bool)
    ensures result == call_admitted_by_row_spec(*row, call)
{
    effect_row_contains(row, call.effect)
}

// r[impl domain-primitives.algebraic-effect.effect-row]
pub fn handler_targets_row_effect(row: &EffectRow, handler: &AlgebraicHandler) -> (result: bool)
    ensures result == handler_targets_row_effect_spec(*row, *handler)
{
    handler.available && effect_row_contains(row, handler.handled_effect)
}

// r[impl domain-primitives.algebraic-effect.effect-row]
pub fn remove_effect_from_row(row: &EffectRow, effect: u64) -> (result: EffectRow)
    ensures result.effects@ == remove_effect_from_row_spec(*row, effect)
{
    let mut effects: Vec<u64> = Vec::new();
    let mut i: usize = 0;
    while i < row.effects.len()
        invariant
            0 <= i <= row.effects.len(),
            effects@ == remove_effect_prefix_spec(row.effects@, effect, i as int),
        decreases row.effects.len() - i,
    {
        let current = row.effects[i];
        if current != effect {
            effects.push(current);
            assert(effects@ == remove_effect_prefix_spec(row.effects@, effect, i as int + 1));
        } else {
            assert(effects@ == remove_effect_prefix_spec(row.effects@, effect, i as int + 1));
        }
        i = i + 1;
    }
    assert(effects@ == remove_effect_from_row_spec(*row, effect));
    EffectRow { effects }
}

// r[impl domain-primitives.algebraic-effect.effect-summary]
pub fn row_contains_all_effects(row: &EffectRow, required: &EffectRow) -> (result: bool)
    ensures result == row_contains_all_effects_spec(*row, *required)
{
    let mut i: usize = 0;
    while i < required.effects.len()
        invariant
            0 <= i <= required.effects.len(),
            forall|k: int| 0 <= k < i ==> effect_row_contains_spec(*row, required.effects@[k]),
        decreases required.effects.len() - i,
    {
        if !effect_row_contains(row, required.effects[i]) {
            assert(!row_contains_all_effects_spec(*row, *required));
            return false;
        }
        i = i + 1;
    }
    assert forall|k: int| 0 <= k < required.effects@.len() implies effect_row_contains_spec(*row, required.effects@[k]) by {
        assert(0 <= k < i);
    }
    assert(row_contains_all_effects_spec(*row, *required));
    true
}

// r[impl domain-primitives.algebraic-effect.effect-summary]
pub fn summary_admitted_by_row(row: &EffectRow, summary: &EffectSummary) -> (result: bool)
    ensures result == summary_admitted_by_row_spec(*row, *summary)
{
    row_contains_all_effects(row, &summary.required)
}

// r[impl domain-primitives.algebraic-effect.effect-summary]
pub fn summary_composition_admitted(left: &EffectSummary, right: &EffectSummary) -> (result: bool)
    ensures result == summary_composition_admitted_spec(*left, *right)
{
    summary_admitted_by_row(&left.residual, right)
}

// r[impl domain-primitives.algebraic-effect.effect-summary]
pub fn copy_effect_row(row: &EffectRow) -> (result: EffectRow)
    ensures result.effects@ == row.effects@
{
    let mut effects: Vec<u64> = Vec::new();
    let mut i: usize = 0;
    while i < row.effects.len()
        invariant
            0 <= i <= row.effects.len(),
            effects@ == row.effects@.subrange(0, i as int),
        decreases row.effects.len() - i,
    {
        effects.push(row.effects[i]);
        i = i + 1;
    }
    EffectRow { effects }
}

// r[impl domain-primitives.algebraic-effect.effect-summary]
pub fn compose_effect_summaries(left: &EffectSummary, right: &EffectSummary) -> (result: EffectSummary)
    requires summary_composition_admitted_spec(*left, *right)
    ensures
        result.required.effects@ == compose_effect_summaries_required_spec(*left, *right),
        result.residual.effects@ == compose_effect_summaries_residual_spec(*left, *right),
{
    EffectSummary {
        required: copy_effect_row(&left.required),
        residual: copy_effect_row(&right.residual),
    }
}

// r[impl domain-primitives.algebraic-effect.summary-handler-application]
pub fn handler_admitted_for_summary(summary: &EffectSummary, handler: &AlgebraicHandler) -> (result: bool)
    ensures result == handler_admitted_for_summary_spec(*summary, *handler)
{
    handler.available && effect_row_contains(&summary.required, handler.handled_effect)
}

// r[impl domain-primitives.algebraic-effect.summary-handler-application]
pub fn apply_handler_to_summary(summary: &EffectSummary, handler: &AlgebraicHandler) -> (result: EffectSummary)
    requires handler_admitted_for_summary_spec(*summary, *handler)
    ensures
        result.required.effects@ == apply_handler_to_summary_required_spec(*summary, *handler),
        result.residual.effects@ == apply_handler_to_summary_residual_spec(*summary, *handler),
{
    EffectSummary {
        required: copy_effect_row(&summary.required),
        residual: remove_effect_from_row(&summary.residual, handler.handled_effect),
    }
}

// r[impl domain-primitives.algebraic-effect.handler-coverage]
pub fn handler_has_clause(handler: &AlgebraicHandler, operation: u64) -> (result: bool)
    ensures result == handler_has_clause_spec(*handler, operation)
{
    let mut i: usize = 0;
    while i < handler.clauses.len()
        invariant
            0 <= i <= handler.clauses.len(),
            forall|k: int| 0 <= k < i ==> handler.clauses@[k] != operation,
        decreases handler.clauses.len() - i,
    {
        if handler.clauses[i] == operation {
            assert(handler_has_clause_spec(*handler, operation));
            return true;
        }
        i = i + 1;
    }
    assert forall|k: int| 0 <= k < handler.clauses@.len() implies handler.clauses@[k] != operation by {
        assert(0 <= k < i);
    }
    assert(!handler_has_clause_spec(*handler, operation));
    false
}

// r[impl domain-primitives.algebraic-effect.handler-coverage]
pub fn call_handled_by_handler(call: EffectCall, handler: &AlgebraicHandler) -> (result: bool)
    ensures result == call_handled_by_handler_spec(call, *handler)
{
    handler.available && call.effect == handler.handled_effect && handler_has_clause(handler, call.operation)
}

// r[impl domain-primitives.algebraic-effect.handler-composition]
pub fn handler_clauses_disjoint(left: &AlgebraicHandler, right: &AlgebraicHandler) -> (result: bool)
    ensures result == handler_clauses_disjoint_spec(*left, *right)
{
    let mut i: usize = 0;
    while i < left.clauses.len()
        invariant
            0 <= i <= left.clauses.len(),
            forall|li: int, rj: int| 0 <= li < i && 0 <= rj < right.clauses@.len()
                ==> left.clauses@[li] != right.clauses@[rj],
        decreases left.clauses.len() - i,
    {
        let mut j: usize = 0;
        while j < right.clauses.len()
            invariant
                0 <= j <= right.clauses.len(),
                0 <= i < left.clauses.len(),
                forall|li: int, rj: int| 0 <= li < i && 0 <= rj < right.clauses@.len()
                    ==> left.clauses@[li] != right.clauses@[rj],
                forall|rj: int| 0 <= rj < j ==> left.clauses@[i as int] != right.clauses@[rj],
            decreases right.clauses.len() - j,
        {
            if left.clauses[i] == right.clauses[j] {
                assert(!handler_clauses_disjoint_spec(*left, *right));
                return false;
            }
            j = j + 1;
        }
        assert forall|rj: int| 0 <= rj < right.clauses@.len() implies left.clauses@[i as int] != right.clauses@[rj] by {
            assert(0 <= rj < j);
        }
        i = i + 1;
    }
    assert forall|li: int, rj: int| 0 <= li < left.clauses@.len() && 0 <= rj < right.clauses@.len()
        implies left.clauses@[li] != right.clauses@[rj] by {
        assert(0 <= li < i);
    }
    assert(handler_clauses_disjoint_spec(*left, *right));
    true
}

// r[impl domain-primitives.algebraic-effect.handler-composition]
pub fn handler_composition_admitted(left: &AlgebraicHandler, right: &AlgebraicHandler) -> (result: bool)
    ensures result == handler_composition_admitted_spec(*left, *right)
{
    left.available && right.available && left.handled_effect == right.handled_effect && handler_clauses_disjoint(left, right)
}

// r[impl domain-primitives.algebraic-effect.handler-composition]
pub fn compose_handlers(left: &AlgebraicHandler, right: &AlgebraicHandler) -> (result: AlgebraicHandler)
    requires handler_composition_admitted_spec(*left, *right)
    ensures
        result.handled_effect == left.handled_effect,
        result.available,
        result.clauses@ == composed_handler_clauses_spec(*left, *right),
{
    let mut clauses: Vec<u64> = Vec::new();
    let mut i: usize = 0;
    while i < left.clauses.len()
        invariant
            0 <= i <= left.clauses.len(),
            clauses@ == left.clauses@.subrange(0, i as int),
        decreases left.clauses.len() - i,
    {
        clauses.push(left.clauses[i]);
        i = i + 1;
    }
    let mut j: usize = 0;
    while j < right.clauses.len()
        invariant
            0 <= j <= right.clauses.len(),
            clauses@ == left.clauses@ + right.clauses@.subrange(0, j as int),
        decreases right.clauses.len() - j,
    {
        clauses.push(right.clauses[j]);
        j = j + 1;
    }
    AlgebraicHandler { handled_effect: left.handled_effect, clauses, available: true }
}

// r[impl domain-primitives.algebraic-effect.handler-coverage]
pub fn handler_covers_signature_effect(signature: &AlgebraicEffectSignature, handler: &AlgebraicHandler) -> (result: bool)
    ensures result == handler_covers_signature_effect_spec(*signature, *handler)
{
    if !handler.available {
        assert(!handler_covers_signature_effect_spec(*signature, *handler));
        return false;
    }
    let mut i: usize = 0;
    while i < signature.operations.len()
        invariant
            0 <= i <= signature.operations.len(),
            handler.available,
            forall|k: int| 0 <= k < i && signature.operations@[k].effect == handler.handled_effect
                ==> handler_has_clause_spec(*handler, #[trigger] signature.operations@[k].operation),
        decreases signature.operations.len() - i,
    {
        let operation = signature.operations[i];
        if operation.effect == handler.handled_effect && !handler_has_clause(handler, operation.operation) {
            assert(!handler_covers_signature_effect_spec(*signature, *handler));
            return false;
        }
        i = i + 1;
    }
    assert forall|k: int| 0 <= k < signature.operations@.len() && signature.operations@[k].effect == handler.handled_effect
        implies handler_has_clause_spec(*handler, #[trigger] signature.operations@[k].operation) by {
        assert(0 <= k < i);
    }
    assert(handler_covers_signature_effect_spec(*signature, *handler));
    true
}

// r[impl domain-primitives.algebraic-effect.handler-discharge]
pub fn discharge_effect_calls(calls: &Vec<EffectCall>, handler: &AlgebraicHandler) -> (result: Vec<EffectCall>)
    ensures result@ == discharge_effect_calls_spec(calls@, *handler)
{
    let mut out: Vec<EffectCall> = Vec::new();
    let mut i: usize = 0;
    while i < calls.len()
        invariant
            0 <= i <= calls.len(),
            out@ == discharge_call_prefix_spec(calls@, *handler, i as int),
        decreases calls.len() - i,
    {
        let call = calls[i];
        if !call_handled_by_handler(call, handler) {
            out.push(call);
            assert(out@ == discharge_call_prefix_spec(calls@, *handler, i as int + 1));
        } else {
            assert(out@ == discharge_call_prefix_spec(calls@, *handler, i as int + 1));
        }
        i = i + 1;
    }
    assert(out@ == discharge_effect_calls_spec(calls@, *handler));
    out
}

// r[impl domain-primitives.algebraic-effect.continuation-token]
pub fn continuation_resume_admitted(token: ContinuationToken, handler: &AlgebraicHandler) -> (result: bool)
    ensures result == continuation_resume_admitted_spec(token, *handler)
{
    !token.used && handler.available && token.effect == handler.handled_effect
}

// r[impl domain-primitives.algebraic-effect.continuation-resume-mode]
pub fn operation_resume_mode_matches_call(mode: OperationResumeMode, call: EffectCall) -> (result: bool)
    ensures result == operation_resume_mode_matches_call_spec(mode, call)
{
    mode.effect == call.effect && mode.operation == call.operation
}

// r[impl domain-primitives.algebraic-effect.continuation-resume-mode]
pub fn continuation_mode_admitted(token: ContinuationToken, handler: &AlgebraicHandler, mode: OperationResumeMode) -> (result: bool)
    ensures result == continuation_mode_admitted_spec(token, *handler, mode)
{
    continuation_resume_admitted(token, handler) && token.effect == mode.effect && mode.resumable
}

// r[impl domain-primitives.algebraic-effect.continuation-resume-mode]
pub fn handler_clause_resume_compatible(mode: OperationResumeMode, clause: HandlerClauseResume) -> (result: bool)
    ensures result == handler_clause_resume_compatible_spec(mode, clause)
{
    mode.effect == clause.handled_effect
        && mode.operation == clause.operation
        && (!clause.resumes || mode.resumable)
}

// r[impl domain-primitives.algebraic-effect.continuation-token]
pub fn consume_continuation(token: ContinuationToken) -> (result: ContinuationToken)
    ensures result == consume_continuation_spec(token), result.used
{
    ContinuationToken { used: true, ..token }
}

// r[verify domain-primitives.algebraic-effect.continuation-token]
pub proof fn consumed_continuation_cannot_resume(token: ContinuationToken, handler: AlgebraicHandler)
    ensures !continuation_resume_admitted_spec(consume_continuation_spec(token), handler)
{
}

// r[verify domain-primitives.algebraic-effect.continuation-resume-mode]
pub proof fn non_resumable_operation_cannot_admit_mode_resume(token: ContinuationToken, handler: AlgebraicHandler, mode: OperationResumeMode)
    requires !mode.resumable
    ensures !continuation_mode_admitted_spec(token, handler, mode)
{
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // r[verify domain-primitives.algebraic-effect.signature]
    fn signature_admits_declared_call_only() {
        let signature = AlgebraicEffectSignature {
            operations: vec![
                AlgebraicEffectOperation {
                    effect: 1,
                    operation: 10,
                    input_arity: 1,
                    output_arity: 1,
                },
                AlgebraicEffectOperation {
                    effect: 2,
                    operation: 20,
                    input_arity: 0,
                    output_arity: 1,
                },
            ],
        };

        assert!(call_admitted_by_signature(&signature, EffectCall {
            effect: 1,
            operation: 10
        }));
        assert!(!call_admitted_by_signature(&signature, EffectCall {
            effect: 1,
            operation: 20
        }));
    }

    #[test]
    // r[verify domain-primitives.algebraic-effect.arity]
    fn invocation_admission_checks_declared_arities() {
        let signature = AlgebraicEffectSignature {
            operations: vec![AlgebraicEffectOperation {
                effect: 3,
                operation: 30,
                input_arity: 2,
                output_arity: 1,
            }],
        };
        let matching = EffectInvocation {
            effect: 3,
            operation: 30,
            input_count: 2,
            output_count: 1,
        };
        let wrong_input_count = EffectInvocation {
            input_count: 1,
            ..matching
        };
        let wrong_output_count = EffectInvocation {
            output_count: 2,
            ..matching
        };

        assert_eq!(invocation_call(matching), EffectCall {
            effect: 3,
            operation: 30
        });
        assert!(invocation_admitted_by_signature(&signature, matching));
        assert!(!invocation_admitted_by_signature(&signature, wrong_input_count));
        assert!(!invocation_admitted_by_signature(&signature, wrong_output_count));
    }

    #[test]
    // r[verify domain-primitives.algebraic-effect.effect-row]
    fn effect_row_admits_calls_by_effect_and_removes_handled_effects() {
        let row = EffectRow {
            effects: vec![1, 2, 1, 3],
        };
        let state_handler = AlgebraicHandler {
            handled_effect: 1,
            clauses: vec![10],
            available: true,
        };
        let unavailable_handler = AlgebraicHandler {
            handled_effect: 2,
            clauses: vec![20],
            available: false,
        };

        assert!(effect_row_contains(&row, 1));
        assert!(call_admitted_by_row(&row, EffectCall {
            effect: 2,
            operation: 20
        }));
        assert!(!call_admitted_by_row(&row, EffectCall {
            effect: 4,
            operation: 40
        }));
        assert!(handler_targets_row_effect(&row, &state_handler));
        assert!(!handler_targets_row_effect(&row, &unavailable_handler));

        let residual = remove_effect_from_row(&row, 1);
        assert_eq!(residual.effects, vec![2, 3]);
        assert!(!effect_row_contains(&residual, 1));
        assert!(effect_row_contains(&residual, 2));
    }

    #[test]
    // r[verify domain-primitives.algebraic-effect.effect-summary]
    fn effect_summary_admission_requires_all_effects_in_row() {
        let ambient = EffectRow { effects: vec![1, 2, 3] };
        let admitted = EffectSummary {
            required: EffectRow { effects: vec![1, 3] },
            residual: EffectRow { effects: vec![2] },
        };
        let rejected = EffectSummary {
            required: EffectRow { effects: vec![1, 4] },
            residual: EffectRow { effects: vec![2] },
        };

        assert!(row_contains_all_effects(&ambient, &admitted.required));
        assert!(summary_admitted_by_row(&ambient, &admitted));
        assert!(!summary_admitted_by_row(&ambient, &rejected));
    }

    #[test]
    // r[verify domain-primitives.algebraic-effect.effect-summary]
    fn effect_summary_composition_chains_residual_rows() {
        let left = EffectSummary {
            required: EffectRow { effects: vec![1, 2] },
            residual: EffectRow { effects: vec![2, 3] },
        };
        let right = EffectSummary {
            required: EffectRow { effects: vec![3] },
            residual: EffectRow { effects: vec![2] },
        };
        let missing = EffectSummary {
            required: EffectRow { effects: vec![4] },
            residual: EffectRow { effects: vec![2] },
        };

        assert!(summary_composition_admitted(&left, &right));
        assert!(!summary_composition_admitted(&left, &missing));

        let composed = compose_effect_summaries(&left, &right);
        assert_eq!(composed.required.effects, vec![1, 2]);
        assert_eq!(composed.residual.effects, vec![2]);
    }

    #[test]
    // r[verify domain-primitives.algebraic-effect.summary-handler-application]
    fn handler_application_removes_handled_effect_from_summary_residual() {
        let summary = EffectSummary {
            required: EffectRow { effects: vec![1, 2] },
            residual: EffectRow {
                effects: vec![2, 1, 3, 1],
            },
        };
        let handler = AlgebraicHandler {
            handled_effect: 1,
            clauses: vec![10],
            available: true,
        };
        let unavailable = AlgebraicHandler {
            available: false,
            ..handler.clone()
        };
        let missing_required_effect = AlgebraicHandler {
            handled_effect: 4,
            clauses: vec![40],
            available: true,
        };

        assert!(handler_admitted_for_summary(&summary, &handler));
        assert!(!handler_admitted_for_summary(&summary, &unavailable));
        assert!(!handler_admitted_for_summary(&summary, &missing_required_effect));

        let applied = apply_handler_to_summary(&summary, &handler);
        assert_eq!(applied.required.effects, vec![1, 2]);
        assert_eq!(applied.residual.effects, vec![2, 3]);
    }

    #[test]
    // r[verify domain-primitives.algebraic-effect.handler-coverage]
    fn handler_covers_only_its_declared_effect_clauses() {
        let signature = AlgebraicEffectSignature {
            operations: vec![
                AlgebraicEffectOperation {
                    effect: 7,
                    operation: 1,
                    input_arity: 0,
                    output_arity: 1,
                },
                AlgebraicEffectOperation {
                    effect: 7,
                    operation: 2,
                    input_arity: 1,
                    output_arity: 1,
                },
                AlgebraicEffectOperation {
                    effect: 8,
                    operation: 3,
                    input_arity: 1,
                    output_arity: 0,
                },
            ],
        };
        let complete = AlgebraicHandler {
            handled_effect: 7,
            clauses: vec![2, 1],
            available: true,
        };
        let incomplete = AlgebraicHandler {
            handled_effect: 7,
            clauses: vec![1],
            available: true,
        };
        let unavailable = AlgebraicHandler {
            handled_effect: 7,
            clauses: vec![1, 2],
            available: false,
        };

        assert!(handler_covers_signature_effect(&signature, &complete));
        assert!(!handler_covers_signature_effect(&signature, &incomplete));
        assert!(!handler_covers_signature_effect(&signature, &unavailable));
    }

    #[test]
    // r[verify domain-primitives.algebraic-effect.handler-discharge]
    fn handler_discharges_matching_calls_and_keeps_residuals() {
        let calls = vec![
            EffectCall {
                effect: 1,
                operation: 10,
            },
            EffectCall {
                effect: 2,
                operation: 20,
            },
            EffectCall {
                effect: 1,
                operation: 11,
            },
            EffectCall {
                effect: 1,
                operation: 10,
            },
        ];
        let handler = AlgebraicHandler {
            handled_effect: 1,
            clauses: vec![10],
            available: true,
        };

        assert_eq!(discharge_effect_calls(&calls, &handler), vec![
            EffectCall {
                effect: 2,
                operation: 20
            },
            EffectCall {
                effect: 1,
                operation: 11
            },
        ]);
    }

    #[test]
    // r[verify domain-primitives.algebraic-effect.handler-composition]
    fn handler_composition_unions_disjoint_same_effect_clauses() {
        let state_read = AlgebraicHandler {
            handled_effect: 4,
            clauses: vec![1],
            available: true,
        };
        let state_write = AlgebraicHandler {
            handled_effect: 4,
            clauses: vec![2, 3],
            available: true,
        };
        let composed = compose_handlers(&state_read, &state_write);

        assert!(handler_composition_admitted(&state_read, &state_write));
        assert_eq!(composed.handled_effect, 4);
        assert!(composed.available);
        assert_eq!(composed.clauses, vec![1, 2, 3]);
        assert!(call_handled_by_handler(
            EffectCall {
                effect: 4,
                operation: 1
            },
            &composed
        ));
        assert!(call_handled_by_handler(
            EffectCall {
                effect: 4,
                operation: 3
            },
            &composed
        ));
        assert!(!call_handled_by_handler(
            EffectCall {
                effect: 5,
                operation: 1
            },
            &composed
        ));
    }

    #[test]
    // r[verify domain-primitives.algebraic-effect.handler-composition]
    fn handler_composition_rejects_ambiguous_or_mismatched_handlers() {
        let base = AlgebraicHandler {
            handled_effect: 6,
            clauses: vec![1, 2],
            available: true,
        };
        let duplicate = AlgebraicHandler {
            handled_effect: 6,
            clauses: vec![2, 3],
            available: true,
        };
        let other_effect = AlgebraicHandler {
            handled_effect: 7,
            clauses: vec![3],
            available: true,
        };
        let unavailable = AlgebraicHandler {
            handled_effect: 6,
            clauses: vec![3],
            available: false,
        };

        assert!(!handler_composition_admitted(&base, &duplicate));
        assert!(!handler_composition_admitted(&base, &other_effect));
        assert!(!handler_composition_admitted(&base, &unavailable));
    }

    #[test]
    // r[verify domain-primitives.algebraic-effect.continuation-token]
    fn continuation_token_is_single_use_for_matching_handler() {
        let handler = AlgebraicHandler {
            handled_effect: 9,
            clauses: vec![1],
            available: true,
        };
        let token = ContinuationToken {
            token: 42,
            effect: 9,
            used: false,
        };
        let other = AlgebraicHandler {
            handled_effect: 10,
            clauses: vec![1],
            available: true,
        };

        assert!(continuation_resume_admitted(token, &handler));
        assert!(!continuation_resume_admitted(token, &other));
        assert!(!continuation_resume_admitted(consume_continuation(token), &handler));
    }

    #[test]
    // r[verify domain-primitives.algebraic-effect.continuation-resume-mode]
    fn continuation_resume_mode_rejects_non_resumable_operations() {
        let handler = AlgebraicHandler {
            handled_effect: 9,
            clauses: vec![1, 2],
            available: true,
        };
        let token = ContinuationToken {
            token: 42,
            effect: 9,
            used: false,
        };
        let resumable = OperationResumeMode {
            effect: 9,
            operation: 1,
            resumable: true,
        };
        let non_resumable = OperationResumeMode {
            effect: 9,
            operation: 2,
            resumable: false,
        };
        let other_effect = OperationResumeMode {
            effect: 10,
            operation: 1,
            resumable: true,
        };

        assert!(operation_resume_mode_matches_call(resumable, EffectCall {
            effect: 9,
            operation: 1
        }));
        assert!(continuation_mode_admitted(token, &handler, resumable));
        assert!(!continuation_mode_admitted(token, &handler, non_resumable));
        assert!(!continuation_mode_admitted(token, &handler, other_effect));
    }

    #[test]
    // r[verify domain-primitives.algebraic-effect.continuation-resume-mode]
    fn handler_clause_resume_must_match_operation_resume_mode() {
        let resumable = OperationResumeMode {
            effect: 4,
            operation: 7,
            resumable: true,
        };
        let non_resumable = OperationResumeMode {
            resumable: false,
            ..resumable
        };
        let resuming_clause = HandlerClauseResume {
            handled_effect: 4,
            operation: 7,
            resumes: true,
        };
        let non_resuming_clause = HandlerClauseResume {
            resumes: false,
            ..resuming_clause
        };
        let wrong_operation = HandlerClauseResume {
            operation: 8,
            ..resuming_clause
        };

        assert!(handler_clause_resume_compatible(resumable, resuming_clause));
        assert!(handler_clause_resume_compatible(non_resumable, non_resuming_clause));
        assert!(!handler_clause_resume_compatible(non_resumable, resuming_clause));
        assert!(!handler_clause_resume_compatible(resumable, wrong_operation));
    }
}
