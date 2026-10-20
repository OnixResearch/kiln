//! Verified dead-instruction elimination from side effects and liveness.
//!
//! Phase 1 classifies a single instruction as dead when it is pure, defines a
//! variable, and that definition is not live-out. Phase 2 lifts that local
//! criterion to whole-block elimination over the shared raw compiler IR from
//! `ssa_construction`: each block is scanned right-to-left, dead instructions
//! are dropped, and surviving instructions update the live set seen by earlier
//! instructions. Phase 3 adds an observable-trace semantics for stores, calls,
//! and returns and proves that elimination preserves that trace under the
//! current straight-line block-order execution model for `RawProgram`.

use vstd::prelude::*;

use crate::liveness_analysis::LiveSets;
use crate::ssa_construction::RawBlock;
use crate::ssa_construction::RawInstr;
use crate::ssa_construction::RawProgram;

verus! {

/// First effectful opcode in the raw instruction model.
///
/// Opcodes below this value are treated as pure. Opcodes at or above this
/// value are conservatively treated as observable side effects.
pub const OPCODE_STORE: u8 = 128u8;

/// External call opcode.
pub const OPCODE_CALL: u8 = 129u8;

/// Function return opcode.
pub const OPCODE_RETURN: u8 = 130u8;

// r[depends dce.side-effect]
/// Spec: any opcode in the effectful range has an observable side effect.
pub open spec fn has_side_effect_spec(opcode: u8) -> bool {
    opcode >= OPCODE_STORE
}

// r[impl dce.side-effect]
/// Returns true when the instruction opcode is conservatively effectful.
pub fn has_side_effect(opcode: u8) -> (result: bool)
    ensures result == has_side_effect_spec(opcode)
{
    opcode >= OPCODE_STORE
}

// r[depends dce.criterion]
/// Spec: an instruction is dead iff it is pure, defines a variable, and that
/// definition is not live-out.
pub open spec fn is_dead_spec(opcode: u8, def: Option<u32>, live_out: Seq<bool>) -> bool {
    !has_side_effect_spec(opcode) && match def {
        Some(v) => (v as int) < live_out.len() && !live_out[v as int],
        None => false,
    }
}

// r[impl dce.criterion]
/// Returns true when the raw instruction is removable by the local DCE
/// criterion.
pub fn is_dead(instr: &RawInstr, live_out: &[bool]) -> (result: bool)
    ensures result == is_dead_spec(instr.opcode, instr.def, live_out@)
{
    if has_side_effect(instr.opcode) {
        proof {
            side_effect_never_dead(instr.opcode, instr.def, live_out@);
        }
        return false;
    }

    match instr.def {
        Some(v) => {
            let idx = v as usize;
            if idx < live_out.len() {
                proof {
                    assert(idx as int == v as int);
                    assert((idx as int) < (live_out@).len());
                }
                !live_out[idx]
            } else {
                false
            }
        }
        None => false,
    }
}

// r[impl dce.side-effect-never-dead]
// r[verify dce.side-effect-never-dead]
/// Side-effecting instructions are never dead.
pub proof fn side_effect_never_dead(opcode: u8, def: Option<u32>, live_out: Seq<bool>)
    requires has_side_effect_spec(opcode)
    ensures !is_dead_spec(opcode, def, live_out)
{}

// r[impl dce.live-def-never-dead]
// r[verify dce.live-def-never-dead]
/// A live definition is never dead.
pub proof fn live_def_never_dead(opcode: u8, v: u32, live_out: Seq<bool>)
    requires
        (v as int) < live_out.len(),
        live_out[v as int],
    ensures !is_dead_spec(opcode, Some(v), live_out)
{}

// ============================================================
// Phase 2 specs
// ============================================================

/// Spec: set one variable live if it is in range.
pub open spec fn set_live_if_in_range_spec(live: Seq<bool>, var: u32) -> Seq<bool> {
    if (var as int) < live.len() {
        live.update(var as int, true)
    } else {
        live
    }
}

/// Spec: kill a definition if it is in range.
pub open spec fn kill_def_spec(def: Option<u32>, live: Seq<bool>) -> Seq<bool> {
    match def {
        Some(v) => {
            if (v as int) < live.len() {
                live.update(v as int, false)
            } else {
                live
            }
        }
        None => live,
    }
}

/// Spec: apply the first `upto` uses, left to right.
pub open spec fn apply_uses_prefix_spec(uses: Seq<u32>, live: Seq<bool>, upto: int) -> Seq<bool>
    decreases if upto >= 0 { upto } else { 0 },
{
    if upto <= 0 {
        live
    } else {
        let prefix = apply_uses_prefix_spec(uses, live, upto - 1);
        set_live_if_in_range_spec(prefix, uses[upto - 1])
    }
}

/// Spec: standard per-instruction backward liveness transfer.
pub open spec fn transfer_instr_spec(instr: RawInstr, live_after: Seq<bool>) -> Seq<bool> {
    apply_uses_prefix_spec(instr.uses@, kill_def_spec(instr.def, live_after), instr.uses@.len() as int)
}

/// Live set before processing the kept suffix starting at `from`.
pub open spec fn live_before_spec(instrs: Seq<RawInstr>, block_live_out: Seq<bool>, from: int) -> Seq<bool>
    decreases if from <= instrs.len() { instrs.len() - from } else { 0 },
{
    if from >= instrs.len() {
        block_live_out
    } else {
        let live_after = live_before_spec(instrs, block_live_out, from + 1);
        if is_dead_spec(instrs[from].opcode, instrs[from].def, live_after) {
            live_after
        } else {
            transfer_instr_spec(instrs[from], live_after)
        }
    }
}

/// Kept instructions from `instrs[from..]`, preserving original order.
pub open spec fn eliminate_from_spec(instrs: Seq<RawInstr>, block_live_out: Seq<bool>, from: int) -> Seq<RawInstr>
    decreases if from <= instrs.len() { instrs.len() - from } else { 0 },
{
    if from >= instrs.len() {
        seq![]
    } else {
        let live_after = live_before_spec(instrs, block_live_out, from + 1);
        if is_dead_spec(instrs[from].opcode, instrs[from].def, live_after) {
            eliminate_from_spec(instrs, block_live_out, from + 1)
        } else {
            seq![instrs[from]] + eliminate_from_spec(instrs, block_live_out, from + 1)
        }
    }
}

pub open spec fn eliminate_block_spec(instrs: Seq<RawInstr>, block_live_out: Seq<bool>) -> Seq<RawInstr> {
    eliminate_from_spec(instrs, block_live_out, 0)
}

pub open spec fn live_rows_compatible_spec(program: RawProgram, live: LiveSets) -> bool {
    live.live_out@.len() == program.blocks@.len()
}

pub open spec fn raw_instr_view_spec(instr: RawInstr) -> (Option<u32>, u8, Seq<u32>) {
    (instr.def, instr.opcode, instr.uses@)
}

pub open spec fn raw_instr_views_spec(instrs: Seq<RawInstr>) -> Seq<(Option<u32>, u8, Seq<u32>)> {
    instrs.map_values(|instr: RawInstr| raw_instr_view_spec(instr))
}

proof fn raw_instr_views_cons(instr: RawInstr, tail: Seq<RawInstr>)
    ensures raw_instr_views_spec(seq![instr] + tail) =~= seq![raw_instr_view_spec(instr)] + raw_instr_views_spec(tail)
{}

proof fn raw_instr_views_push(instrs: Seq<RawInstr>, instr: RawInstr)
    ensures raw_instr_views_spec(instrs.push(instr)) =~= raw_instr_views_spec(instrs).push(raw_instr_view_spec(instr))
{}

proof fn live_before_unfold(instrs: Seq<RawInstr>, block_live_out: Seq<bool>, from: int)
    requires 0 <= from < instrs.len()
    ensures
        live_before_spec(instrs, block_live_out, from)
            == if is_dead_spec(
                instrs[from].opcode,
                instrs[from].def,
                live_before_spec(instrs, block_live_out, from + 1),
            ) {
                live_before_spec(instrs, block_live_out, from + 1)
            } else {
                transfer_instr_spec(instrs[from], live_before_spec(instrs, block_live_out, from + 1))
            },
{}

/// Helper: unfold one elimination step.
proof fn eliminate_from_unfold(instrs: Seq<RawInstr>, block_live_out: Seq<bool>, from: int)
    requires 0 <= from < instrs.len()
    ensures
        eliminate_from_spec(instrs, block_live_out, from)
            == if is_dead_spec(
                instrs[from].opcode,
                instrs[from].def,
                live_before_spec(instrs, block_live_out, from + 1),
            ) {
                eliminate_from_spec(instrs, block_live_out, from + 1)
            } else {
                seq![instrs[from]] + eliminate_from_spec(instrs, block_live_out, from + 1)
            },
{}

/// Reversing `s.push(x)` moves `x` to the front.
proof fn reverse_push_lemma<T>(s: Seq<T>, x: T)
    ensures s.push(x).reverse() =~= seq![x] + s.reverse()
{
    assert forall|i: int| 0 <= i < s.push(x).len()
        implies s.push(x).reverse()[i] == (seq![x] + s.reverse())[i]
    by {
        if i == 0 {
            assert(s.push(x).reverse()[0] == s.push(x)[s.len() as int - 0]);
            assert(s.push(x)[s.len() as int] == x);
        } else {
            let j = i - 1;
            assert((seq![x] + s.reverse())[i] == s.reverse()[j]);
            assert(s.reverse()[j] == s[s.len() as int - 1 - j]);
            assert(s.len() as int - 1 - j == s.len() as int - i);
            assert(s.push(x).reverse()[i] == s.push(x)[s.push(x).len() as int - 1 - i]);
            assert(s.push(x).len() == s.len() + 1);
            assert(s.push(x).len() as int - 1 - i == s.len() as int - i);
            assert(s.push(x)[s.len() as int - i] == s[s.len() as int - i]);
        }
    }
}

// r[verify dce.no-live-removed]
/// Any instruction that is not dead at its program point appears in the
/// elimination result for that suffix.
pub proof fn non_dead_preserved_from(instrs: Seq<RawInstr>, block_live_out: Seq<bool>, from: int, target: int)
    requires
        0 <= from <= target < instrs.len(),
        !is_dead_spec(
            instrs[target].opcode,
            instrs[target].def,
            live_before_spec(instrs, block_live_out, target + 1),
        ),
    ensures
        exists|j: int| 0 <= j < eliminate_from_spec(instrs, block_live_out, from).len()
            && eliminate_from_spec(instrs, block_live_out, from)[j] == instrs[target],
    decreases target - from,
{
    eliminate_from_unfold(instrs, block_live_out, from);
    if from == target {
        let live_after = live_before_spec(instrs, block_live_out, from + 1);
        assert(!is_dead_spec(instrs[from].opcode, instrs[from].def, live_after));
        assert(eliminate_from_spec(instrs, block_live_out, from)[0] == instrs[target]);
    } else {
        non_dead_preserved_from(instrs, block_live_out, from + 1, target);
        let suffix = eliminate_from_spec(instrs, block_live_out, from + 1);
        let witness = choose|j: int|
            #![trigger suffix[j]]
            0 <= j < suffix.len() && suffix[j] == instrs[target];
        let live_after = live_before_spec(instrs, block_live_out, from + 1);
        if is_dead_spec(instrs[from].opcode, instrs[from].def, live_after) {
            assert(eliminate_from_spec(instrs, block_live_out, from)
                == eliminate_from_spec(instrs, block_live_out, from + 1));
        } else {
            assert(eliminate_from_spec(instrs, block_live_out, from)
                == seq![instrs[from]] + eliminate_from_spec(instrs, block_live_out, from + 1));
            assert(eliminate_from_spec(instrs, block_live_out, from)[witness + 1] == instrs[target]);
        }
    }
}

// ============================================================
// Phase 3 specs and proofs
// ============================================================

pub uninterp spec fn compute_value_spec(opcode: u8, args: Seq<u64>) -> u64;

pub open spec fn read_var_spec(env: Seq<u64>, var: u32) -> u64 {
    if (var as int) < env.len() { env[var as int] } else { 0u64 }
}

pub open spec fn read_uses_spec(uses: Seq<u32>, env: Seq<u64>) -> Seq<u64> {
    Seq::new(uses.len(), |i: int| read_var_spec(env, uses[i]))
}

pub open spec fn env_agrees_spec(e1: Seq<u64>, e2: Seq<u64>, live: Seq<bool>) -> bool {
    e1.len() == live.len()
        && e2.len() == live.len()
        && forall|v: int|
            #![trigger live[v]]
            #![trigger e1[v]]
            #![trigger e2[v]]
            0 <= v < live.len() && live[v] ==> e1[v] == e2[v]
}

pub open spec fn instr_value_spec(instr: RawInstr, env: Seq<u64>) -> u64 {
    compute_value_spec(instr.opcode, read_uses_spec(instr.uses@, env))
}

pub open spec fn observable_event_spec(instr: RawInstr, env: Seq<u64>) -> Option<(u8, u64)> {
    if has_side_effect_spec(instr.opcode) {
        Some((instr.opcode, instr_value_spec(instr, env)))
    } else {
        None
    }
}

pub open spec fn env_step_spec(instr: RawInstr, env: Seq<u64>) -> Seq<u64> {
    match instr.def {
        Some(v) => {
            if (v as int) < env.len() {
                env.update(v as int, instr_value_spec(instr, env))
            } else {
                env
            }
        }
        None => env,
    }
}

pub open spec fn observable_trace_from_spec(instrs: Seq<RawInstr>, env: Seq<u64>, from: int) -> Seq<(u8, u64)>
    decreases if from <= instrs.len() { instrs.len() - from } else { 0 },
{
    if from >= instrs.len() {
        seq![]
    } else {
        let event = observable_event_spec(instrs[from], env);
        let next_env = env_step_spec(instrs[from], env);
        match event {
            Some(e) => seq![e] + observable_trace_from_spec(instrs, next_env, from + 1),
            None => observable_trace_from_spec(instrs, next_env, from + 1),
        }
    }
}

pub open spec fn env_after_from_spec(instrs: Seq<RawInstr>, env: Seq<u64>, from: int) -> Seq<u64>
    decreases if from <= instrs.len() { instrs.len() - from } else { 0 },
{
    if from >= instrs.len() {
        env
    } else {
        env_after_from_spec(instrs, env_step_spec(instrs[from], env), from + 1)
    }
}

pub open spec fn block_entry_live_spec(block: RawBlock, block_live_out: Seq<bool>) -> Seq<bool> {
    live_before_spec(block.instrs@, block_live_out, 0)
}

pub open spec fn sequential_live_summary_spec(program: RawProgram, live: LiveSets) -> bool {
    live_rows_compatible_spec(program, live)
        && forall|b: int|
            #![trigger live.live_out@[b]]
            #![trigger program.blocks@[b + 1]]
            0 <= b && b + 1 < program.blocks@.len()
                ==> live.live_out@[b]@ == block_entry_live_spec(program.blocks@[b + 1], live.live_out@[b + 1]@)
}

// r[depends dce.semantics]
pub open spec fn observable_trace_program_from_spec(program: RawProgram, env: Seq<u64>, from: int) -> Seq<(u8, u64)>
    decreases if from <= program.blocks@.len() { program.blocks@.len() - from } else { 0 },
{
    if from >= program.blocks@.len() {
        seq![]
    } else {
        let block = program.blocks@[from];
        let block_trace = observable_trace_from_spec(block.instrs@, env, 0);
        let next_env = env_after_from_spec(block.instrs@, env, 0);
        block_trace + observable_trace_program_from_spec(program, next_env, from + 1)
    }
}

pub open spec fn observable_trace_after_dce_program_from_spec(program: RawProgram, live: LiveSets, env: Seq<u64>, from: int) -> Seq<(u8, u64)>
    decreases if from <= program.blocks@.len() { program.blocks@.len() - from } else { 0 },
{
    if from >= program.blocks@.len() {
        seq![]
    } else {
        let block_instrs = eliminate_block_spec(program.blocks@[from].instrs@, live.live_out@[from]@);
        let block_trace = observable_trace_from_spec(block_instrs, env, 0);
        let next_env = env_after_from_spec(block_instrs, env, 0);
        block_trace + observable_trace_after_dce_program_from_spec(program, live, next_env, from + 1)
    }
}

// r[depends dce.semantics]
pub open spec fn observable_trace_spec(program: RawProgram, env: Seq<u64>) -> Seq<(u8, u64)> {
    observable_trace_program_from_spec(program, env, 0)
}

pub open spec fn observable_trace_after_dce_spec(program: RawProgram, live: LiveSets, env: Seq<u64>) -> Seq<(u8, u64)> {
    observable_trace_after_dce_program_from_spec(program, live, env, 0)
}

proof fn observable_trace_from_unfold(instrs: Seq<RawInstr>, env: Seq<u64>, from: int)
    requires 0 <= from < instrs.len()
    ensures
        observable_trace_from_spec(instrs, env, from)
            =~= match observable_event_spec(instrs[from], env) {
                Some(e) => seq![e] + observable_trace_from_spec(instrs, env_step_spec(instrs[from], env), from + 1),
                None => observable_trace_from_spec(instrs, env_step_spec(instrs[from], env), from + 1),
            },
{}

proof fn env_after_from_unfold(instrs: Seq<RawInstr>, env: Seq<u64>, from: int)
    requires 0 <= from < instrs.len()
    ensures env_after_from_spec(instrs, env, from) == env_after_from_spec(instrs, env_step_spec(instrs[from], env), from + 1)
{}

proof fn trace_from_subrange(instrs: Seq<RawInstr>, env: Seq<u64>, from: int)
    requires 0 <= from <= instrs.len()
    ensures
        observable_trace_from_spec(instrs, env, from)
            =~= observable_trace_from_spec(instrs.subrange(from, instrs.len() as int), env, 0),
        env_after_from_spec(instrs, env, from)
            == env_after_from_spec(instrs.subrange(from, instrs.len() as int), env, 0),
    decreases if from <= instrs.len() { instrs.len() - from } else { 0 },
{
    if from < instrs.len() {
        let sub = instrs.subrange(from, instrs.len() as int);
        let next_env = env_step_spec(instrs[from], env);
        observable_trace_from_unfold(instrs, env, from);
        env_after_from_unfold(instrs, env, from);
        observable_trace_from_unfold(sub, env, 0);
        env_after_from_unfold(sub, env, 0);
        assert(sub[0] == instrs[from]);
        assert(observable_event_spec(sub[0], env) == observable_event_spec(instrs[from], env));
        assert(env_step_spec(sub[0], env) == next_env);
        trace_from_subrange(instrs, next_env, from + 1);
        trace_from_subrange(sub, next_env, 1);
        assert(sub.subrange(1, sub.len() as int) =~= instrs.subrange(from + 1, instrs.len() as int));
        match observable_event_spec(instrs[from], env) {
            Some(e) => {
                assert(observable_trace_from_spec(instrs, env, from)
                    =~= seq![e] + observable_trace_from_spec(instrs, next_env, from + 1));
                assert(observable_trace_from_spec(sub, env, 0)
                    =~= seq![e] + observable_trace_from_spec(sub, next_env, 1));
            }
            None => {
                assert(observable_trace_from_spec(instrs, env, from)
                    =~= observable_trace_from_spec(instrs, next_env, from + 1));
                assert(observable_trace_from_spec(sub, env, 0)
                    =~= observable_trace_from_spec(sub, next_env, 1));
            }
        }
        assert(env_after_from_spec(instrs, env, from) == env_after_from_spec(instrs, next_env, from + 1));
        assert(env_after_from_spec(sub, env, 0) == env_after_from_spec(sub, next_env, 1));
    } else {
        assert(instrs.subrange(from, instrs.len() as int) =~= seq![]);
        assert(observable_trace_from_spec(instrs, env, from) =~= seq![]);
        assert(observable_trace_from_spec(instrs.subrange(from, instrs.len() as int), env, 0) =~= seq![]);
        assert(env_after_from_spec(instrs, env, from) == env);
        assert(env_after_from_spec(instrs.subrange(from, instrs.len() as int), env, 0) == env);
    }
}

proof fn env_agrees_refl(env: Seq<u64>, live: Seq<bool>)
    requires env.len() == live.len()
    ensures env_agrees_spec(env, env, live)
{}

proof fn live_before_len(instrs: Seq<RawInstr>, block_live_out: Seq<bool>, from: int)
    requires 0 <= from <= instrs.len()
    ensures live_before_spec(instrs, block_live_out, from).len() == block_live_out.len()
    decreases if from <= instrs.len() { instrs.len() - from } else { 0 },
{
    if from < instrs.len() {
        live_before_unfold(instrs, block_live_out, from);
        live_before_len(instrs, block_live_out, from + 1);
        let live_after = live_before_spec(instrs, block_live_out, from + 1);
        if !is_dead_spec(instrs[from].opcode, instrs[from].def, live_after) {
            apply_uses_prefix_len(instrs[from].uses@, kill_def_spec(instrs[from].def, live_after), instrs[from].uses@.len() as int);
        }
    }
}

proof fn sequential_live_summary_step(program: RawProgram, live: LiveSets, from: int)
    requires
        sequential_live_summary_spec(program, live),
        0 <= from,
        from + 1 < program.blocks@.len(),
    ensures live.live_out@[from]@ == block_entry_live_spec(program.blocks@[from + 1], live.live_out@[from + 1]@)
{}

proof fn read_uses_agree(uses: Seq<u32>, e1: Seq<u64>, e2: Seq<u64>, live: Seq<bool>)
    requires
        env_agrees_spec(e1, e2, live),
        forall|j: int| #![trigger uses[j]]
            0 <= j < uses.len() && (uses[j] as int) < live.len() ==> live[uses[j] as int],
    ensures read_uses_spec(uses, e1) =~= read_uses_spec(uses, e2)
{
    assert forall|i: int| 0 <= i < uses.len() implies read_uses_spec(uses, e1)[i] == read_uses_spec(uses, e2)[i]
    by {
        let var = uses[i];
        if (var as int) < live.len() {
            assert(live[var as int]);
            assert(e1[var as int] == e2[var as int]);
        } else {
            assert((var as int) >= e1.len());
            assert((var as int) >= e2.len());
        }
    }
}

proof fn apply_uses_prefix_len(uses: Seq<u32>, live: Seq<bool>, upto: int)
    requires 0 <= upto <= uses.len()
    ensures apply_uses_prefix_spec(uses, live, upto).len() == live.len()
    decreases upto,
{
    if upto > 0 {
        apply_uses_prefix_len(uses, live, upto - 1);
    }
}

proof fn env_step_len(instr: RawInstr, env: Seq<u64>)
    ensures env_step_spec(instr, env).len() == env.len()
{}

proof fn env_after_from_len(instrs: Seq<RawInstr>, env: Seq<u64>, from: int)
    requires 0 <= from <= instrs.len()
    ensures env_after_from_spec(instrs, env, from).len() == env.len()
    decreases if from <= instrs.len() { instrs.len() - from } else { 0 },
{
    if from < instrs.len() {
        env_step_len(instrs[from], env);
        env_after_from_len(instrs, env_step_spec(instrs[from], env), from + 1);
    }
}

proof fn apply_uses_prefix_preserves_true(uses: Seq<u32>, live: Seq<bool>, upto: int, v: int)
    requires
        0 <= upto <= uses.len(),
        0 <= v < live.len(),
        live[v],
    ensures apply_uses_prefix_spec(uses, live, upto)[v]
    decreases upto,
{
    if upto > 0 {
        apply_uses_prefix_preserves_true(uses, live, upto - 1, v);
        let prefix = apply_uses_prefix_spec(uses, live, upto - 1);
        apply_uses_prefix_len(uses, live, upto - 1);
        assert(prefix[v]);
        assert(apply_uses_prefix_spec(uses, live, upto)
            == set_live_if_in_range_spec(prefix, uses[upto - 1]));
        if (uses[upto - 1] as int) == v {
            assert((uses[upto - 1] as int) < prefix.len());
            assert(set_live_if_in_range_spec(prefix, uses[upto - 1]) == prefix.update(uses[upto - 1] as int, true));
            assert(set_live_if_in_range_spec(prefix, uses[upto - 1])[v]);
        } else if (uses[upto - 1] as int) < prefix.len() {
            assert(set_live_if_in_range_spec(prefix, uses[upto - 1]) == prefix.update(uses[upto - 1] as int, true));
            assert(set_live_if_in_range_spec(prefix, uses[upto - 1])[v] == prefix[v]);
        } else {
            assert(set_live_if_in_range_spec(prefix, uses[upto - 1]) == prefix);
        }
    }
}

proof fn apply_uses_prefix_sets_seen_use(uses: Seq<u32>, live: Seq<bool>, upto: int, target: int)
    requires
        0 <= target < upto <= uses.len(),
        (uses[target] as int) < live.len(),
    ensures apply_uses_prefix_spec(uses, live, upto)[uses[target] as int]
    decreases upto - target,
{
    if upto == target + 1 {
        let prefix = apply_uses_prefix_spec(uses, live, target);
        apply_uses_prefix_len(uses, live, target);
        assert(apply_uses_prefix_spec(uses, live, target + 1)
            == set_live_if_in_range_spec(prefix, uses[target]));
        assert((uses[target] as int) < prefix.len());
        assert(set_live_if_in_range_spec(prefix, uses[target]) == prefix.update(uses[target] as int, true));
        assert(set_live_if_in_range_spec(prefix, uses[target])[uses[target] as int]);
    } else {
        apply_uses_prefix_sets_seen_use(uses, live, upto - 1, target);
        let prefix = apply_uses_prefix_spec(uses, live, upto - 1);
        apply_uses_prefix_len(uses, live, upto - 1);
        let var = uses[target] as int;
        assert(prefix[var]);
        assert(apply_uses_prefix_spec(uses, live, upto)
            == set_live_if_in_range_spec(prefix, uses[upto - 1]));
        if (uses[upto - 1] as int) == var {
            assert((uses[upto - 1] as int) < prefix.len());
            assert(set_live_if_in_range_spec(prefix, uses[upto - 1]) == prefix.update(uses[upto - 1] as int, true));
            assert(set_live_if_in_range_spec(prefix, uses[upto - 1])[var]);
        } else if (uses[upto - 1] as int) < prefix.len() {
            assert(set_live_if_in_range_spec(prefix, uses[upto - 1]) == prefix.update(uses[upto - 1] as int, true));
            assert(set_live_if_in_range_spec(prefix, uses[upto - 1])[var] == prefix[var]);
        } else {
            assert(set_live_if_in_range_spec(prefix, uses[upto - 1]) == prefix);
        }
    }
}

proof fn transfer_marks_use_live(instr: RawInstr, live_after: Seq<bool>, use_idx: int)
    requires
        0 <= use_idx < instr.uses@.len(),
        (instr.uses@[use_idx] as int) < live_after.len(),
    ensures transfer_instr_spec(instr, live_after)[instr.uses@[use_idx] as int]
{
    let base = kill_def_spec(instr.def, live_after);
    apply_uses_prefix_sets_seen_use(instr.uses@, base, instr.uses@.len() as int, use_idx);
}

// r[impl dce.removed-no-effect]
// r[verify dce.removed-no-effect]
/// Dead instructions emit no observable event and preserve agreement on the
/// future-live variables.
pub proof fn dead_no_observable_effect(instr: RawInstr, orig_env: Seq<u64>, elim_env: Seq<u64>, live_after: Seq<bool>)
    requires
        is_dead_spec(instr.opcode, instr.def, live_after),
        env_agrees_spec(orig_env, elim_env, live_after),
    ensures
        observable_event_spec(instr, orig_env) == None::<(u8, u64)>,
        env_agrees_spec(env_step_spec(instr, orig_env), elim_env, live_after)
{
    assert(!has_side_effect_spec(instr.opcode));
    assert(observable_event_spec(instr, orig_env) == None::<(u8, u64)>);
    assert(env_step_spec(instr, orig_env).len() == live_after.len());
    assert forall|v: int|
        #![trigger live_after[v]]
        #![trigger env_step_spec(instr, orig_env)[v]]
        #![trigger elim_env[v]]
        0 <= v < live_after.len() && live_after[v]
            implies env_step_spec(instr, orig_env)[v] == elim_env[v]
    by {
        match instr.def {
            Some(def_var) => {
                if (def_var as int) < live_after.len() && def_var as int == v {
                    assert(!live_after[def_var as int]);
                } else {
                    assert(env_step_spec(instr, orig_env)[v] == orig_env[v]);
                    assert(orig_env[v] == elim_env[v]);
                }
            }
            None => {
                assert(env_step_spec(instr, orig_env)[v] == orig_env[v]);
                assert(orig_env[v] == elim_env[v]);
            }
        }
    }
}

// r[impl dce.retained-identical]
// r[verify dce.retained-identical]
/// If two environments agree on the kept instruction's entry-live set, the
/// instruction emits the same observable event in both and preserves
/// agreement on the live-after set.
pub proof fn retained_instruction_identical(instr: RawInstr, e1: Seq<u64>, e2: Seq<u64>, live_after: Seq<bool>)
    requires env_agrees_spec(e1, e2, transfer_instr_spec(instr, live_after))
    ensures
        observable_event_spec(instr, e1) == observable_event_spec(instr, e2),
        env_agrees_spec(env_step_spec(instr, e1), env_step_spec(instr, e2), live_after)
{
    let entry_live = transfer_instr_spec(instr, live_after);
    apply_uses_prefix_len(instr.uses@, kill_def_spec(instr.def, live_after), instr.uses@.len() as int);
    assert(entry_live.len() == live_after.len());
    assert forall|j: int| #![trigger instr.uses@[j]]
        0 <= j < instr.uses@.len() && (instr.uses@[j] as int) < live_after.len()
            implies entry_live[instr.uses@[j] as int]
    by {
        transfer_marks_use_live(instr, live_after, j);
    }
    read_uses_agree(instr.uses@, e1, e2, entry_live);
    assert(instr_value_spec(instr, e1) == instr_value_spec(instr, e2));
    assert(observable_event_spec(instr, e1) == observable_event_spec(instr, e2));
    env_step_len(instr, e1);
    env_step_len(instr, e2);
    assert(env_step_spec(instr, e1).len() == live_after.len());
    assert(env_step_spec(instr, e2).len() == live_after.len());
    assert forall|v: int|
        #![trigger live_after[v]]
        #![trigger env_step_spec(instr, e1)[v]]
        #![trigger env_step_spec(instr, e2)[v]]
        0 <= v < live_after.len() && live_after[v]
            implies env_step_spec(instr, e1)[v] == env_step_spec(instr, e2)[v]
    by {
        match instr.def {
            Some(def_var) => {
                if (def_var as int) < live_after.len() && def_var as int == v {
                    assert(instr_value_spec(instr, e1) == instr_value_spec(instr, e2));
                } else {
                    assert(kill_def_spec(instr.def, live_after)[v]);
                    apply_uses_prefix_preserves_true(instr.uses@, kill_def_spec(instr.def, live_after), instr.uses@.len() as int, v);
                    assert(entry_live[v]);
                    assert(e1[v] == e2[v]);
                    assert(env_step_spec(instr, e1)[v] == e1[v]);
                    assert(env_step_spec(instr, e2)[v] == e2[v]);
                }
            }
            None => {
                assert(kill_def_spec(instr.def, live_after)[v]);
                apply_uses_prefix_preserves_true(instr.uses@, kill_def_spec(instr.def, live_after), instr.uses@.len() as int, v);
                assert(entry_live[v]);
                assert(e1[v] == e2[v]);
                assert(env_step_spec(instr, e1)[v] == e1[v]);
                assert(env_step_spec(instr, e2)[v] == e2[v]);
            }
        }
    }
}

pub proof fn dce_block_semantics_from(instrs: Seq<RawInstr>, orig_env: Seq<u64>, elim_env: Seq<u64>, block_live_out: Seq<bool>, from: int)
    requires
        0 <= from <= instrs.len(),
        env_agrees_spec(orig_env, elim_env, live_before_spec(instrs, block_live_out, from)),
    ensures
        observable_trace_from_spec(instrs, orig_env, from)
            =~= observable_trace_from_spec(eliminate_from_spec(instrs, block_live_out, from), elim_env, 0),
        env_agrees_spec(
            env_after_from_spec(instrs, orig_env, from),
            env_after_from_spec(eliminate_from_spec(instrs, block_live_out, from), elim_env, 0),
            block_live_out,
        )
    decreases if from <= instrs.len() { instrs.len() - from } else { 0 },
{
    if from < instrs.len() {
        live_before_unfold(instrs, block_live_out, from);
        eliminate_from_unfold(instrs, block_live_out, from);
        observable_trace_from_unfold(instrs, orig_env, from);
        env_after_from_unfold(instrs, orig_env, from);
        let live_after = live_before_spec(instrs, block_live_out, from + 1);
        if is_dead_spec(instrs[from].opcode, instrs[from].def, live_after) {
            dead_no_observable_effect(instrs[from], orig_env, elim_env, live_after);
            let orig_next = env_step_spec(instrs[from], orig_env);
            dce_block_semantics_from(instrs, orig_next, elim_env, block_live_out, from + 1);
            assert(eliminate_from_spec(instrs, block_live_out, from)
                == eliminate_from_spec(instrs, block_live_out, from + 1));
            assert(observable_trace_from_spec(instrs, orig_env, from)
                =~= observable_trace_from_spec(instrs, orig_next, from + 1));
            assert(observable_trace_from_spec(eliminate_from_spec(instrs, block_live_out, from), elim_env, 0)
                =~= observable_trace_from_spec(eliminate_from_spec(instrs, block_live_out, from + 1), elim_env, 0));
            assert(env_after_from_spec(instrs, orig_env, from) == env_after_from_spec(instrs, orig_next, from + 1));
            assert(env_after_from_spec(eliminate_from_spec(instrs, block_live_out, from), elim_env, 0)
                == env_after_from_spec(eliminate_from_spec(instrs, block_live_out, from + 1), elim_env, 0));
        } else {
            retained_instruction_identical(instrs[from], orig_env, elim_env, live_after);
            let orig_next = env_step_spec(instrs[from], orig_env);
            let elim_next = env_step_spec(instrs[from], elim_env);
            dce_block_semantics_from(instrs, orig_next, elim_next, block_live_out, from + 1);
            let kept = eliminate_from_spec(instrs, block_live_out, from);
            let kept_tail = eliminate_from_spec(instrs, block_live_out, from + 1);
            let kept_seq = seq![instrs[from]] + kept_tail;
            assert(kept == kept_seq);
            assert(kept.subrange(1, kept.len() as int) =~= kept_tail);
            observable_trace_from_unfold(kept, elim_env, 0);
            env_after_from_unfold(kept, elim_env, 0);
            trace_from_subrange(kept, elim_next, 1);
            assert(observable_event_spec(instrs[from], orig_env) == observable_event_spec(instrs[from], elim_env));
            match observable_event_spec(instrs[from], orig_env) {
                Some(e) => {
                    assert(kept[0] == instrs[from]);
                    assert(observable_event_spec(kept[0], elim_env) == Some(e));
                    assert(env_step_spec(kept[0], elim_env) == elim_next);
                    assert(observable_trace_from_spec(instrs, orig_env, from)
                        =~= seq![e] + observable_trace_from_spec(instrs, orig_next, from + 1));
                    assert(observable_trace_from_spec(kept, elim_env, 0)
                        =~= seq![e] + observable_trace_from_spec(kept, elim_next, 1));
                    assert(observable_trace_from_spec(kept, elim_next, 1)
                        =~= observable_trace_from_spec(kept_tail, elim_next, 0));
                    assert(observable_trace_from_spec(kept, elim_env, 0)
                        =~= seq![e] + observable_trace_from_spec(kept_tail, elim_next, 0));
                }
                None => {
                    assert(kept[0] == instrs[from]);
                    assert(observable_event_spec(kept[0], elim_env) == None::<(u8, u64)>);
                    assert(env_step_spec(kept[0], elim_env) == elim_next);
                    assert(observable_trace_from_spec(instrs, orig_env, from)
                        =~= observable_trace_from_spec(instrs, orig_next, from + 1));
                    assert(observable_trace_from_spec(kept, elim_env, 0)
                        =~= observable_trace_from_spec(kept, elim_next, 1));
                    assert(observable_trace_from_spec(kept, elim_next, 1)
                        =~= observable_trace_from_spec(kept_tail, elim_next, 0));
                    assert(observable_trace_from_spec(kept, elim_env, 0)
                        =~= observable_trace_from_spec(kept_tail, elim_next, 0));
                }
            }
            assert(env_after_from_spec(instrs, orig_env, from) == env_after_from_spec(instrs, orig_next, from + 1));
            assert(env_after_from_spec(kept, elim_env, 0) == env_after_from_spec(kept, elim_next, 1));
            assert(env_after_from_spec(kept, elim_next, 1) == env_after_from_spec(kept_tail, elim_next, 0));
            assert(env_after_from_spec(kept, elim_env, 0) == env_after_from_spec(kept_tail, elim_next, 0));
        }
    } else {
        assert(live_before_spec(instrs, block_live_out, from) == block_live_out);
        assert(observable_trace_from_spec(instrs, orig_env, from) =~= seq![]);
        assert(eliminate_from_spec(instrs, block_live_out, from) =~= seq![]);
        assert(observable_trace_from_spec(eliminate_from_spec(instrs, block_live_out, from), elim_env, 0) =~= seq![]);
        assert(env_after_from_spec(instrs, orig_env, from) == orig_env);
        assert(env_after_from_spec(eliminate_from_spec(instrs, block_live_out, from), elim_env, 0) == elim_env);
    }
}

// r[impl dce.semantics]
// r[verify dce.semantics]
/// Under the raw IR's current straight-line block-order model, DCE preserves
/// the observable trace of stores, calls, and returns.
pub proof fn dce_preserves_observable_trace(program: RawProgram, live: LiveSets, env: Seq<u64>)
    requires
        sequential_live_summary_spec(program, live),
        forall|b: int| #![trigger live.live_out@[b]] 0 <= b < live.live_out@.len() ==> live.live_out@[b]@.len() == env.len(),
    ensures observable_trace_spec(program, env) =~= observable_trace_after_dce_spec(program, live, env)
{
    if 0 < program.blocks@.len() {
        live_before_len(program.blocks@[0].instrs@, live.live_out@[0]@, 0);
        env_agrees_refl(env, block_entry_live_spec(program.blocks@[0], live.live_out@[0]@));
    }
    dce_preserves_observable_trace_from(program, live, env, env, 0);
}

proof fn dce_preserves_observable_trace_from(program: RawProgram, live: LiveSets, orig_env: Seq<u64>, elim_env: Seq<u64>, from: int)
    requires
        sequential_live_summary_spec(program, live),
        forall|b: int| #![trigger live.live_out@[b]] 0 <= b < live.live_out@.len() ==> live.live_out@[b]@.len() == orig_env.len(),
        orig_env.len() == elim_env.len(),
        0 <= from <= program.blocks@.len(),
        from < program.blocks@.len() ==> env_agrees_spec(orig_env, elim_env, block_entry_live_spec(program.blocks@[from], live.live_out@[from]@)),
    ensures observable_trace_program_from_spec(program, orig_env, from)
        =~= observable_trace_after_dce_program_from_spec(program, live, elim_env, from)
    decreases if from <= program.blocks@.len() { program.blocks@.len() - from } else { 0 },
{
    if from < program.blocks@.len() {
        let block = program.blocks@[from];
        dce_block_semantics_from(block.instrs@, orig_env, elim_env, live.live_out@[from]@, 0);
        let orig_next = env_after_from_spec(block.instrs@, orig_env, 0);
        let elim_next = env_after_from_spec(eliminate_block_spec(block.instrs@, live.live_out@[from]@), elim_env, 0);
        env_after_from_len(block.instrs@, orig_env, 0);
        env_after_from_len(eliminate_block_spec(block.instrs@, live.live_out@[from]@), elim_env, 0);
        if from + 1 < program.blocks@.len() {
            sequential_live_summary_step(program, live, from);
            assert(live.live_out@[from]@ == block_entry_live_spec(program.blocks@[from + 1], live.live_out@[from + 1]@));
            assert(orig_next.len() == orig_env.len());
            assert(elim_next.len() == elim_env.len());
            dce_preserves_observable_trace_from(program, live, orig_next, elim_next, from + 1);
        }
    }
}

// ============================================================
// Phase 4: Idempotence
// ============================================================

/// In a sequence with no dead instructions, `live_before` always applies
/// `transfer_instr_spec` (never skips).
proof fn live_before_all_kept(instrs: Seq<RawInstr>, block_live_out: Seq<bool>, from: int)
    requires
        0 <= from <= instrs.len(),
        forall|i: int| #![trigger instrs[i]]
            from <= i < instrs.len() ==>
                !is_dead_spec(instrs[i].opcode, instrs[i].def,
                    live_before_spec(instrs, block_live_out, i + 1)),
    ensures
        live_before_spec(instrs, block_live_out, from)
            == if from < instrs.len() {
                transfer_instr_spec(instrs[from], live_before_spec(instrs, block_live_out, from + 1))
            } else {
                block_live_out
            },
    decreases if from <= instrs.len() { instrs.len() - from } else { 0 },
{
    if from < instrs.len() {
        live_before_unfold(instrs, block_live_out, from);
    }
}

/// If no instruction in `instrs[from..]` is dead, elimination is the identity.
proof fn eliminate_all_kept_from(instrs: Seq<RawInstr>, block_live_out: Seq<bool>, from: int)
    requires
        0 <= from <= instrs.len(),
        forall|i: int| #![trigger instrs[i]]
            from <= i < instrs.len() ==>
                !is_dead_spec(instrs[i].opcode, instrs[i].def,
                    live_before_spec(instrs, block_live_out, i + 1)),
    ensures eliminate_from_spec(instrs, block_live_out, from) =~= instrs.subrange(from, instrs.len() as int)
    decreases if from <= instrs.len() { instrs.len() - from } else { 0 },
{
    if from < instrs.len() {
        eliminate_from_unfold(instrs, block_live_out, from);
        eliminate_all_kept_from(instrs, block_live_out, from + 1);
        assert(instrs.subrange(from, instrs.len() as int)
            =~= seq![instrs[from]] + instrs.subrange(from + 1, instrs.len() as int));
    }
}

/// Core idempotence lemma: after one elimination pass, `live_before` over the
/// eliminated sequence at each kept position matches `live_before` over the
/// original at the corresponding original position.
///
/// This means every kept instruction was non-dead in the original and remains
/// non-dead in the eliminated sequence, so a second pass keeps everything.
///
/// We prove the simpler form: the eliminated sequence has no dead instructions,
/// by showing `live_before_spec(kept, lo, i+1) == live_before_spec(orig, lo, orig_pos+1)`
/// at each corresponding position.
proof fn eliminated_has_no_dead_from(instrs: Seq<RawInstr>, block_live_out: Seq<bool>, from: int)
    requires 0 <= from <= instrs.len()
    ensures
        ({
            let kept = eliminate_from_spec(instrs, block_live_out, from);
            forall|i: int| #![trigger kept[i]]
                0 <= i < kept.len() ==>
                    !is_dead_spec(kept[i].opcode, kept[i].def,
                        live_before_spec(kept, block_live_out, i + 1))
        }),
        live_before_spec(
            eliminate_from_spec(instrs, block_live_out, from),
            block_live_out, 0
        ) == live_before_spec(instrs, block_live_out, from),
    decreases if from <= instrs.len() { instrs.len() - from } else { 0 },
{
    if from < instrs.len() {
        eliminate_from_unfold(instrs, block_live_out, from);
        live_before_unfold(instrs, block_live_out, from);
        let live_after = live_before_spec(instrs, block_live_out, from + 1);
        eliminated_has_no_dead_from(instrs, block_live_out, from + 1);
        let kept_tail = eliminate_from_spec(instrs, block_live_out, from + 1);
        // IH: live_before(kept_tail, lo, 0) == live_before(orig, lo, from+1)
        // IH: forall i. kept_tail[i] is not dead in kept_tail
        if is_dead_spec(instrs[from].opcode, instrs[from].def, live_after) {
            // Dead: kept == kept_tail, live_before skips this instr
            assert(eliminate_from_spec(instrs, block_live_out, from) == kept_tail);
            assert(live_before_spec(instrs, block_live_out, from) == live_after);
        } else {
            // Kept: kept == seq![instrs[from]] + kept_tail
            let kept = seq![instrs[from]] + kept_tail;
            assert(eliminate_from_spec(instrs, block_live_out, from) =~= kept);
            // live_before(kept, lo, 1) == live_before(kept_tail, lo, 0)
            //                          == live_before(orig, lo, from+1)
            //                          == live_after
            // Need: kept.subrange(1, kept.len()) =~= kept_tail
            assert(kept.subrange(1, kept.len() as int) =~= kept_tail);

            // Prove live_before(kept, lo, 1) == live_before(kept_tail, lo, 0)
            // by showing the suffix starting at index 1 is exactly kept_tail.
            // Use the subrange relationship and induction on kept.
            live_before_suffix_shift(kept, block_live_out, 1);

            // Now live_before(kept, lo, 1) == live_before(kept_tail, lo, 0)
            //                               == live_after (by IH)
            assert(live_before_spec(kept, block_live_out, 1)
                == live_before_spec(kept_tail, block_live_out, 0));
            assert(live_before_spec(kept_tail, block_live_out, 0) == live_after);

            // So instrs[from] is not dead w.r.t. live_before(kept, lo, 1)
            assert(kept[0] == instrs[from]);
            assert(!is_dead_spec(kept[0].opcode, kept[0].def, live_before_spec(kept, block_live_out, 1)));

            // live_before(kept, lo, 0) uses transfer since kept[0] is not dead
            live_before_unfold(kept, block_live_out, 0);
            assert(live_before_spec(kept, block_live_out, 0)
                == transfer_instr_spec(instrs[from], live_after));
            assert(live_before_spec(instrs, block_live_out, from)
                == transfer_instr_spec(instrs[from], live_after));

            // For the forall: kept[0] is not dead (shown above).
            // For i > 0: kept[i] == kept_tail[i-1], and by IH
            // kept_tail[i-1] is not dead w.r.t. live_before(kept_tail, lo, i).
            // Need live_before(kept, lo, i+1) == live_before(kept_tail, lo, i)
            // which follows from the suffix shift.
            assert forall|i: int| #![trigger kept[i]]
                0 <= i < kept.len() implies
                    !is_dead_spec(kept[i].opcode, kept[i].def,
                        live_before_spec(kept, block_live_out, i + 1))
            by {
                if i == 0 {
                    assert(!is_dead_spec(kept[0].opcode, kept[0].def,
                        live_before_spec(kept, block_live_out, 1)));
                } else {
                    assert(kept[i] == kept_tail[i - 1]);
                    live_before_suffix_shift(kept, block_live_out, i + 1);
                    assert(live_before_spec(kept, block_live_out, i + 1)
                        == live_before_spec(kept_tail, block_live_out, i));
                    // By IH, kept_tail[i-1] is not dead w.r.t. live_before(kept_tail, lo, i)
                    assert(!is_dead_spec(kept_tail[i - 1].opcode, kept_tail[i - 1].def,
                        live_before_spec(kept_tail, block_live_out, i)));
                }
            }
        }
    }
}

/// For `instrs = seq![x] + tail`, `live_before_spec(instrs, lo, k)` for
/// `k >= 1` equals `live_before_spec(tail, lo, k - 1)`.
proof fn live_before_suffix_shift(instrs: Seq<RawInstr>, block_live_out: Seq<bool>, k: int)
    requires
        instrs.len() > 0,
        1 <= k <= instrs.len(),
    ensures
        live_before_spec(instrs, block_live_out, k)
            == live_before_spec(
                instrs.subrange(1, instrs.len() as int), block_live_out, k - 1),
    decreases if k <= instrs.len() { instrs.len() - k } else { 0 },
{
    let tail = instrs.subrange(1, instrs.len() as int);
    if k == instrs.len() {
        // Both are block_live_out
        assert(k - 1 == tail.len() as int);
    } else {
        // k < instrs.len(), so k-1 < tail.len()
        live_before_suffix_shift(instrs, block_live_out, k + 1);
        live_before_unfold(instrs, block_live_out, k);
        live_before_unfold(tail, block_live_out, k - 1);
        assert(instrs[k] == tail[k - 1]);
        assert(live_before_spec(instrs, block_live_out, k + 1)
            == live_before_spec(tail, block_live_out, k));
    }
}

// r[impl dce.idempotent]
// r[verify dce.idempotent]
/// Idempotence: a second elimination pass on the result of DCE keeps every
/// instruction, because the first pass already removed all dead ones.
pub proof fn eliminate_block_idempotent(instrs: Seq<RawInstr>, block_live_out: Seq<bool>)
    ensures
        eliminate_block_spec(
            eliminate_block_spec(instrs, block_live_out),
            block_live_out,
        ) =~= eliminate_block_spec(instrs, block_live_out)
{
    let kept = eliminate_block_spec(instrs, block_live_out);
    eliminated_has_no_dead_from(instrs, block_live_out, 0);
    // Now: forall i. kept[i] is not dead in kept
    eliminate_all_kept_from(kept, block_live_out, 0);
    // eliminate_from(kept, lo, 0) =~= kept.subrange(0, kept.len()) =~= kept
    assert(kept.subrange(0, kept.len() as int) =~= kept);
}

// ============================================================
// Exec helpers
// ============================================================

fn clone_bool_row(row: &[bool]) -> (result: Vec<bool>)
    ensures result@ == row@
{
    let mut result: Vec<bool> = Vec::new();
    let mut i: usize = 0;
    while i < row.len()
        invariant
            0 <= i <= row@.len(),
            result@ == row@.subrange(0, i as int),
        decreases row.len() - i,
    {
        result.push(row[i]);
        i = i + 1;
    }
    result
}

fn clone_u32_row(row: &[u32]) -> (result: Vec<u32>)
    ensures result@ == row@
{
    let mut result: Vec<u32> = Vec::new();
    let mut i: usize = 0;
    while i < row.len()
        invariant
            0 <= i <= row@.len(),
            result@ == row@.subrange(0, i as int),
        decreases row.len() - i,
    {
        result.push(row[i]);
        i = i + 1;
    }
    result
}

fn clone_instr(instr: &RawInstr) -> (result: RawInstr)
    ensures raw_instr_view_spec(result) == raw_instr_view_spec(*instr)
{
    RawInstr { def: instr.def, uses: clone_u32_row(&instr.uses), opcode: instr.opcode }
}

fn clone_edges(edges: &[(usize, usize)]) -> (result: Vec<(usize, usize)>)
    ensures result@ == edges@
{
    let mut result: Vec<(usize, usize)> = Vec::new();
    let mut i: usize = 0;
    while i < edges.len()
        invariant
            0 <= i <= edges@.len(),
            result@ == edges@.subrange(0, i as int),
        decreases edges.len() - i,
    {
        result.push(edges[i]);
        i = i + 1;
    }
    result
}

fn transfer_instr(instr: &RawInstr, live_after: &[bool]) -> (result: Vec<bool>)
    ensures result@ == transfer_instr_spec(*instr, live_after@)
{
    let mut live = clone_bool_row(live_after);
    proof {
        assert(apply_uses_prefix_spec(instr.uses@, kill_def_spec(instr.def, live_after@), 0) =~= kill_def_spec(instr.def, live_after@));
    }
    if let Some(v) = instr.def {
        let idx = v as usize;
        if idx < live.len() {
            proof {
                assert(idx as int == v as int);
                assert((idx as int) < live_after@.len());
                assert(kill_def_spec(instr.def, live_after@) == live_after@.update(v as int, false));
            }
            live.set(idx, false);
        }
    }
    let ghost start_live = live@;
    let mut j: usize = 0;
    while j < instr.uses.len()
        invariant
            0 <= j <= instr.uses@.len(),
            start_live == kill_def_spec(instr.def, live_after@),
            live@ == apply_uses_prefix_spec(instr.uses@, start_live, j as int),
        decreases instr.uses.len() - j,
    {
        let use_var = instr.uses[j];
        let idx = use_var as usize;
        if idx < live.len() {
            proof {
                assert(idx as int == use_var as int);
                assert((idx as int) < live@.len());
                assert(apply_uses_prefix_spec(instr.uses@, start_live, j as int + 1)
                    == set_live_if_in_range_spec(apply_uses_prefix_spec(instr.uses@, start_live, j as int), use_var));
                assert(set_live_if_in_range_spec(apply_uses_prefix_spec(instr.uses@, start_live, j as int), use_var)
                    == live@.update(use_var as int, true));
            }
            live.set(idx, true);
        } else {
            proof {
                assert(apply_uses_prefix_spec(instr.uses@, start_live, j as int + 1)
                    == set_live_if_in_range_spec(apply_uses_prefix_spec(instr.uses@, start_live, j as int), use_var));
                assert(set_live_if_in_range_spec(apply_uses_prefix_spec(instr.uses@, start_live, j as int), use_var)
                    == live@);
            }
        }
        j = j + 1;
    }
    live
}

fn eliminate_block(instrs: &[RawInstr], block_live_out: &[bool]) -> (result: Vec<RawInstr>)
    ensures raw_instr_views_spec(result@) =~= raw_instr_views_spec(eliminate_block_spec(instrs@, block_live_out@))
{
    let mut kept_rev: Vec<RawInstr> = Vec::new();
    let mut current_live = clone_bool_row(block_live_out);
    let mut i: usize = instrs.len();
    while i > 0
        invariant
            0 <= i <= instrs@.len(),
            current_live@ =~= live_before_spec(instrs@, block_live_out@, i as int),
            raw_instr_views_spec(kept_rev@).reverse() =~= raw_instr_views_spec(eliminate_from_spec(instrs@, block_live_out@, i as int)),
        decreases i,
    {
        i = i - 1;
        let instr = &instrs[i];
        let ghost live_after = current_live@;
        if is_dead(instr, &current_live) {
            proof {
                eliminate_from_unfold(instrs@, block_live_out@, i as int);
                assert(is_dead_spec(instrs@[i as int].opcode, instrs@[i as int].def, live_after));
                assert(live_before_spec(instrs@, block_live_out@, i as int) =~= current_live@);
                assert(eliminate_from_spec(instrs@, block_live_out@, i as int)
                    == eliminate_from_spec(instrs@, block_live_out@, i as int + 1));
                assert(raw_instr_views_spec(eliminate_from_spec(instrs@, block_live_out@, i as int))
                    =~= raw_instr_views_spec(eliminate_from_spec(instrs@, block_live_out@, i as int + 1)));
            }
        } else {
            let cloned = clone_instr(instr);
            let next_live = transfer_instr(instr, &current_live);
            proof {
                eliminate_from_unfold(instrs@, block_live_out@, i as int);
                assert(!is_dead_spec(instrs@[i as int].opcode, instrs@[i as int].def, live_after));
                assert(next_live@ =~= transfer_instr_spec(instrs@[i as int], live_after));
                assert(raw_instr_view_spec(cloned) == raw_instr_view_spec(instrs@[i as int]));
                raw_instr_views_push(kept_rev@, cloned);
                reverse_push_lemma(raw_instr_views_spec(kept_rev@), raw_instr_view_spec(cloned));
                raw_instr_views_cons(instrs@[i as int], eliminate_from_spec(instrs@, block_live_out@, i as int + 1));
                assert(raw_instr_views_spec(kept_rev@.push(cloned)).reverse()
                    =~= seq![raw_instr_view_spec(instrs@[i as int])] + raw_instr_views_spec(kept_rev@).reverse());
                assert(raw_instr_views_spec(eliminate_from_spec(instrs@, block_live_out@, i as int))
                    =~= seq![raw_instr_view_spec(instrs@[i as int])] + raw_instr_views_spec(eliminate_from_spec(instrs@, block_live_out@, i as int + 1)));
            }
            kept_rev.push(cloned);
            current_live = next_live;
        }
    }

    let rlen = kept_rev.len();
    let mut result: Vec<RawInstr> = Vec::new();
    let mut j: usize = rlen;
    while j > 0
        invariant
            0 <= j <= rlen,
            rlen == kept_rev@.len(),
            result@.len() == rlen - j,
            forall|k: int| 0 <= k < result@.len()
                ==> raw_instr_view_spec(result@[k]) == raw_instr_view_spec(kept_rev@[rlen as int - 1 - k]),
        decreases j,
    {
        j = j - 1;
        let cloned = clone_instr(&kept_rev[j]);
        proof {
            assert(raw_instr_view_spec(cloned) == raw_instr_view_spec(kept_rev@[j as int]));
        }
        result.push(cloned);
    }

    proof {
        assert forall|k: int| 0 <= k < result@.len()
            implies raw_instr_view_spec(result@[k]) == raw_instr_view_spec(kept_rev@.reverse()[k]) by {
            assert(kept_rev@.reverse()[k] == kept_rev@[rlen as int - 1 - k]);
        }
        assert(raw_instr_views_spec(result@) =~= raw_instr_views_spec(kept_rev@.reverse()));
        assert(raw_instr_views_spec(kept_rev@.reverse()) =~= raw_instr_views_spec(kept_rev@).reverse());
    }
    result
}

// r[impl dce.no-live-removed]
/// Eliminate dead instructions from every block using block-exit live-out rows.
pub fn eliminate_dead(program: &RawProgram, live: &LiveSets) -> (result: RawProgram)
    requires
        live_rows_compatible_spec(*program, *live),
    ensures
        result.edges@ == program.edges@,
        result.blocks@.len() == program.blocks@.len(),
        forall|b: int| 0 <= b < program.blocks@.len()
            ==> raw_instr_views_spec(result.blocks@[b].instrs@) =~= raw_instr_views_spec(eliminate_block_spec(program.blocks@[b].instrs@, live.live_out@[b]@)),
{
    let mut blocks: Vec<RawBlock> = Vec::new();
    let mut b: usize = 0;
    while b < program.blocks.len()
        invariant
            0 <= b <= program.blocks@.len(),
            live_rows_compatible_spec(*program, *live),
            blocks@.len() == b as int,
            forall|i: int| 0 <= i < b as int
                ==> raw_instr_views_spec(blocks@[i].instrs@) =~= raw_instr_views_spec(eliminate_block_spec(program.blocks@[i].instrs@, live.live_out@[i]@)),
        decreases program.blocks.len() - b,
    {
        let instrs = eliminate_block(&program.blocks[b].instrs, &live.live_out[b]);
        blocks.push(RawBlock { instrs });
        b = b + 1;
    }
    RawProgram { blocks, edges: clone_edges(&program.edges) }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::liveness_analysis::LiveSets;
    use crate::ssa_construction::RawBlock;
    use crate::ssa_construction::RawInstr;
    use crate::ssa_construction::RawProgram;

    fn raw_instr(def: Option<u32>, uses: &[u32], opcode: u8) -> RawInstr {
        RawInstr {
            def,
            uses: uses.to_vec(),
            opcode,
        }
    }

    fn single_block_program(instrs: Vec<RawInstr>) -> RawProgram {
        RawProgram {
            blocks: vec![RawBlock { instrs }],
            edges: vec![],
        }
    }

    fn live_sets(rows: Vec<Vec<bool>>) -> LiveSets {
        LiveSets {
            live_in: rows.clone(),
            live_out: rows,
        }
    }

    fn clone_instr_runtime(instr: &RawInstr) -> RawInstr {
        RawInstr {
            def: instr.def,
            uses: instr.uses.clone(),
            opcode: instr.opcode,
        }
    }

    fn transfer_instr_runtime(instr: &RawInstr, live_after: &[bool]) -> Vec<bool> {
        let mut live = live_after.to_vec();
        if let Some(v) = instr.def {
            let idx = v as usize;
            if idx < live.len() {
                live[idx] = false;
            }
        }
        for &u in &instr.uses {
            let idx = u as usize;
            if idx < live.len() {
                live[idx] = true;
            }
        }
        live
    }

    fn eliminate_block_oracle(instrs: &[RawInstr], block_live_out: &[bool]) -> Vec<RawInstr> {
        let mut kept_rev = Vec::new();
        let mut live = block_live_out.to_vec();
        for instr in instrs.iter().rev() {
            if !opcode_is_dead_oracle(instr, &live) {
                kept_rev.push(clone_instr_runtime(instr));
                live = transfer_instr_runtime(instr, &live);
            }
        }
        kept_rev.into_iter().rev().collect()
    }

    fn eval_instr_runtime(opcode: u8, inputs: &[u64]) -> u64 {
        match opcode % 4 {
            0 => inputs.iter().copied().fold(0, u64::wrapping_add),
            1 => inputs.iter().copied().fold(1, u64::wrapping_mul),
            2 => inputs.iter().copied().fold(u64::from(opcode), |acc, v| acc ^ v),
            _ => inputs.iter().copied().fold(u64::from(opcode), u64::wrapping_add),
        }
    }

    fn observable_trace_program_runtime(program: &RawProgram, input_env: &[u64]) -> Vec<(u8, u64)> {
        let mut env = input_env.to_vec();
        let mut trace = Vec::new();
        for block in &program.blocks {
            for instr in &block.instrs {
                let args: Vec<u64> = instr.uses.iter().map(|u| env.get(*u as usize).copied().unwrap_or(0)).collect();
                let value = eval_instr_runtime(instr.opcode, &args);
                if instr.opcode >= OPCODE_STORE {
                    trace.push((instr.opcode, value));
                }
                if let Some(def) = instr.def {
                    let idx = def as usize;
                    if idx < env.len() {
                        env[idx] = value;
                    }
                }
            }
        }
        trace
    }

    fn sequential_live_rows_runtime(program: &RawProgram, vars: usize) -> LiveSets {
        let blocks = program.blocks.len();
        let mut live_out = vec![vec![false; vars]; blocks];
        let mut next_entry = vec![false; vars];
        for b in (0..blocks).rev() {
            live_out[b] = next_entry.clone();
            let mut entry_live = next_entry.clone();
            for instr in program.blocks[b].instrs.iter().rev() {
                if !opcode_is_dead_oracle(instr, &entry_live) {
                    entry_live = transfer_instr_runtime(instr, &entry_live);
                }
            }
            next_entry = entry_live;
        }
        LiveSets {
            live_in: live_out.clone(),
            live_out,
        }
    }

    fn opcode_is_dead_oracle(instr: &RawInstr, live_out: &[bool]) -> bool {
        if instr.opcode >= OPCODE_STORE {
            return false;
        }
        match instr.def {
            Some(v) => {
                let idx = v as usize;
                idx < live_out.len() && !live_out[idx]
            }
            None => false,
        }
    }

    // r[verify dce.side-effect]
    #[test]
    fn test_has_side_effect_boundaries() {
        assert!(!has_side_effect(0));
        assert!(!has_side_effect(OPCODE_STORE - 1));
        assert!(has_side_effect(OPCODE_STORE));
        assert!(has_side_effect(OPCODE_CALL));
        assert!(has_side_effect(OPCODE_RETURN));
    }

    // r[verify dce.criterion]
    #[test]
    fn test_pure_unused_definition_is_dead() {
        let instr = raw_instr(Some(1u32), &[7, 11], 3);
        let live_out = vec![true, false, true];
        assert!(is_dead(&instr, &live_out));
    }

    // r[verify dce.live-def-never-dead]
    #[test]
    fn test_live_definition_is_not_dead() {
        let instr = raw_instr(Some(2u32), &[7, 11], 9);
        let live_out = vec![false, false, true];
        assert!(!is_dead(&instr, &live_out));
    }

    // r[verify dce.side-effect-never-dead]
    #[test]
    fn test_effectful_instruction_is_not_dead() {
        let instr = raw_instr(Some(0u32), &[7, 11], OPCODE_STORE);
        let live_out = vec![false, false];
        assert!(!is_dead(&instr, &live_out));
    }

    // r[verify dce.criterion]
    #[test]
    fn test_missing_definition_is_conservatively_live() {
        let instr = raw_instr(None, &[7, 11], 5);
        let live_out = vec![false, false];
        assert!(!is_dead(&instr, &live_out));
    }

    // r[verify dce.criterion]
    #[test]
    fn test_out_of_range_definition_is_conservatively_live() {
        let instr = raw_instr(Some(4u32), &[7, 11], 6);
        let live_out = vec![false, false, false, false];
        assert!(!is_dead(&instr, &live_out));
    }

    // r[verify dce.no-live-removed]
    #[test]
    fn test_eliminate_dead_preserves_live_use_chain() {
        let program = single_block_program(vec![raw_instr(Some(0u32), &[], 1), raw_instr(None, &[0u32], OPCODE_CALL)]);
        let live = live_sets(vec![vec![false]]);
        let optimized = eliminate_dead(&program, &live);
        assert_eq!(optimized.blocks.len(), 1);
        assert_eq!(optimized.blocks[0].instrs, program.blocks[0].instrs);
    }

    #[test]
    fn test_eliminate_dead_removes_cascading_dead_code() {
        let program = single_block_program(vec![raw_instr(Some(0u32), &[], 1), raw_instr(Some(1u32), &[0u32], 2)]);
        let live = live_sets(vec![vec![false, false]]);
        let optimized = eliminate_dead(&program, &live);
        assert!(optimized.blocks[0].instrs.is_empty());
    }

    #[test]
    fn test_eliminate_dead_preserves_effectful_instruction() {
        let program = single_block_program(vec![raw_instr(Some(0u32), &[], 1), raw_instr(None, &[0u32], OPCODE_CALL)]);
        let live = live_sets(vec![vec![false]]);
        let optimized = eliminate_dead(&program, &live);
        assert_eq!(optimized.blocks[0].instrs, program.blocks[0].instrs);
    }

    #[test]
    fn test_eliminate_dead_preserves_blocks_and_edges() {
        let program = RawProgram {
            blocks: vec![
                RawBlock {
                    instrs: vec![raw_instr(Some(0u32), &[], 1)],
                },
                RawBlock {
                    instrs: vec![raw_instr(None, &[0u32], OPCODE_RETURN)],
                },
            ],
            edges: vec![(0, 1)],
        };
        let live = live_sets(vec![vec![false], vec![false]]);
        let optimized = eliminate_dead(&program, &live);
        assert_eq!(optimized.blocks.len(), program.blocks.len());
        assert_eq!(optimized.edges, program.edges);
    }

    // r[verify dce.idempotent]
    #[test]
    fn test_dce_idempotent() {
        let program = single_block_program(vec![
            raw_instr(Some(0u32), &[], 1),
            raw_instr(Some(1u32), &[0u32], 2),
            raw_instr(None, &[0u32], OPCODE_CALL),
        ]);
        let live = live_sets(vec![vec![false, false]]);
        let first = eliminate_dead(&program, &live);
        let second = eliminate_dead(&first, &live);
        assert_eq!(first.blocks[0].instrs, second.blocks[0].instrs);
    }

    // r[verify dce.semantics]
    #[test]
    fn test_observable_trace_preserved_for_sequential_program() {
        let program = RawProgram {
            blocks: vec![
                RawBlock {
                    instrs: vec![raw_instr(Some(0u32), &[], 1), raw_instr(Some(1u32), &[0u32], 2)],
                },
                RawBlock {
                    instrs: vec![
                        raw_instr(None, &[0u32], OPCODE_CALL),
                        raw_instr(None, &[1u32], OPCODE_RETURN),
                    ],
                },
            ],
            edges: vec![(0, 1)],
        };
        let env = vec![3u64, 5u64];
        let live = sequential_live_rows_runtime(&program, env.len());
        let optimized = eliminate_dead(&program, &live);
        assert_eq!(
            observable_trace_program_runtime(&program, &env),
            observable_trace_program_runtime(&optimized, &env)
        );
    }

    proptest! {
        // r[verify dce.side-effect-never-dead]
        #[test]
        fn prop_effectful_instruction_never_dead(
            opcode in OPCODE_STORE..=u8::MAX,
            def in prop::option::of(0u8..32),
            live_out in prop::collection::vec(any::<bool>(), 0..32usize),
            uses in prop::collection::vec(0u8..32, 0..4usize),
        ) {
            let instr = RawInstr {
                def: def.map(u32::from),
                uses: uses.into_iter().map(u32::from).collect(),
                opcode,
            };
            prop_assert!(!is_dead(&instr, &live_out));
        }

        // r[verify dce.live-def-never-dead]
        #[test]
        fn prop_live_definition_never_dead(
            opcode in any::<u8>(),
            v in 0u8..32,
            uses in prop::collection::vec(0u8..32, 0..4usize),
        ) {
            let idx = usize::from(v);
            let live_out = {
                let mut row = vec![false; idx + 1];
                row[idx] = true;
                row
            };
            let instr = RawInstr {
                def: Some(u32::from(v)),
                uses: uses.into_iter().map(u32::from).collect(),
                opcode,
            };
            prop_assert!(!is_dead(&instr, &live_out));
        }

        // r[verify dce.criterion]
        #[test]
        fn prop_is_dead_matches_local_formula(
            opcode in any::<u8>(),
            def in prop::option::of(0u8..32),
            live_out in prop::collection::vec(any::<bool>(), 0..32usize),
            uses in prop::collection::vec(0u8..32, 0..4usize),
        ) {
            let instr = RawInstr {
                def: def.map(u32::from),
                uses: uses.into_iter().map(u32::from).collect(),
                opcode,
            };
            let expected = opcode < OPCODE_STORE && match def {
                Some(v) => {
                    let idx = usize::from(v);
                    idx < live_out.len() && !live_out[idx]
                }
                None => false,
            };
            prop_assert_eq!(is_dead(&instr, &live_out), expected);
        }

        // r[verify dce.no-live-removed]
        #[test]
        fn prop_eliminate_dead_matches_oracle(
            blocks in 1usize..5,
            vars in 0usize..6,
            block_instrs in prop::collection::vec(
                prop::collection::vec(
                    (
                        prop::option::of(0u8..6),
                        prop::collection::vec(0u8..6, 0..4usize),
                        0u8..=130u8,
                    ).prop_map(|(def, uses, opcode)| RawInstr {
                        def: def.map(u32::from),
                        uses: uses.into_iter().map(u32::from).collect(),
                        opcode,
                    }),
                    0..5usize,
                ),
                1..5usize,
            ),
            live_rows in prop::collection::vec(prop::collection::vec(any::<bool>(), 0..6usize), 1..5usize),
            edges in prop::collection::vec((0usize..5usize, 0usize..5usize), 0..8usize),
        ) {
            let actual_blocks = blocks.min(block_instrs.len()).min(live_rows.len()).max(1);
            let program = RawProgram {
                blocks: block_instrs.into_iter().take(actual_blocks).map(|instrs| RawBlock { instrs }).collect(),
                edges: edges.into_iter().filter(|(src, dst)| *src < actual_blocks && *dst < actual_blocks).collect(),
            };
            let live_out: Vec<Vec<bool>> = live_rows.into_iter().take(actual_blocks).collect();
            let live = LiveSets { live_in: live_out.clone(), live_out: live_out.clone() };
            let optimized = eliminate_dead(&program, &live);

            let expected_blocks: Vec<Vec<RawInstr>> = program.blocks.iter().zip(live_out.iter())
                .map(|(block, row)| eliminate_block_oracle(&block.instrs, row))
                .collect();

            prop_assert_eq!(optimized.blocks.len(), actual_blocks);
            prop_assert_eq!(optimized.edges, program.edges);
            for (block, expected) in optimized.blocks.iter().zip(expected_blocks.iter()) {
                prop_assert_eq!(&block.instrs, expected);
            }
            let _ = vars;
        }

        // r[verify dce.idempotent]
        #[test]
        fn prop_dce_idempotent(
            block_instrs in prop::collection::vec(
                prop::collection::vec(
                    (
                        prop::option::of(0u8..6),
                        prop::collection::vec(0u8..6, 0..4usize),
                        0u8..=130u8,
                    ).prop_map(|(def, uses, opcode)| RawInstr {
                        def: def.map(u32::from),
                        uses: uses.into_iter().map(u32::from).collect(),
                        opcode,
                    }),
                    0..5usize,
                ),
                1..5usize,
            ),
            live_rows in prop::collection::vec(prop::collection::vec(any::<bool>(), 0..6usize), 1..5usize),
        ) {
            let actual_blocks = block_instrs.len().min(live_rows.len()).max(1);
            let program = RawProgram {
                blocks: block_instrs.into_iter().take(actual_blocks).map(|instrs| RawBlock { instrs }).collect(),
                edges: vec![],
            };
            let live_out: Vec<Vec<bool>> = live_rows.into_iter().take(actual_blocks).collect();
            let live = LiveSets { live_in: live_out.clone(), live_out: live_out.clone() };
            let first = eliminate_dead(&program, &live);
            let second = eliminate_dead(&first, &live);
            prop_assert_eq!(first.blocks.len(), second.blocks.len());
            for (b1, b2) in first.blocks.iter().zip(second.blocks.iter()) {
                prop_assert_eq!(&b1.instrs, &b2.instrs);
            }
        }

        // r[verify dce.semantics]
        #[test]
        fn prop_observable_trace_preserved(
            block_instrs in prop::collection::vec(
                prop::collection::vec(
                    (
                        prop::option::of(0u8..6),
                        prop::collection::vec(0u8..6, 0..4usize),
                        0u8..=130u8,
                    ).prop_map(|(def, uses, opcode)| RawInstr {
                        def: def.map(u32::from),
                        uses: uses.into_iter().map(u32::from).collect(),
                        opcode,
                    }),
                    0..5usize,
                ),
                1..5usize,
            ),
            env in prop::collection::vec(any::<u64>(), 0..6usize),
        ) {
            let program = RawProgram {
                blocks: block_instrs.into_iter().map(|instrs| RawBlock { instrs }).collect(),
                edges: vec![],
            };
            let live = sequential_live_rows_runtime(&program, env.len());
            let optimized = eliminate_dead(&program, &live);
            prop_assert_eq!(observable_trace_program_runtime(&program, &env), observable_trace_program_runtime(&optimized, &env));
        }
    }
}
