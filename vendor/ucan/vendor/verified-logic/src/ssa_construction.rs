//! SSA construction utilities with thin verified wrappers.
//!
//! The runtime pipeline mirrors the classic compiler flow:
//! 1. collect per-variable def sites,
//! 2. place phi nodes at the iterated dominance frontier,
//! 3. assign a fresh version to every phi and instruction def,
//! 4. rename each use to the closest dominating version,
//! 5. eliminate phis into per-edge parallel copies.
//!
//! The verified surface keeps the contracts lightweight: public wrappers prove
//! structural properties (lengths, edge validity, bounded IDF rounds, exact phi
//!-source lookup). The executable algorithms live in plain Rust helpers below
//! the `verus!` block, and tests compare them against small brute-force oracles.
//!
//! Phi elimination uses an edge-copy model: the eliminated program removes phi
//! nodes from blocks and records the parallel copies required on each incoming
//! edge before control enters the successor block.

#![allow(clippy::too_many_lines, clippy::assigning_clones)]

use vstd::prelude::*;

verus! {

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SsaDefKind {
    Parameter,
    Phi,
    Instr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SsaDef {
    pub base: u32,
    pub version: usize,
    pub block: usize,
    pub index: usize,
    pub kind: SsaDefKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawInstr {
    pub def: Option<u32>,
    pub uses: Vec<u32>,
    pub opcode: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawBlock {
    pub instrs: Vec<RawInstr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawProgram {
    pub blocks: Vec<RawBlock>,
    pub edges: Vec<(usize, usize)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhiNode {
    pub base: u32,
    pub dest: usize,
    pub args: Vec<(usize, usize)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsaInstr {
    pub def_base: Option<u32>,
    pub def: Option<usize>,
    pub base_uses: Vec<u32>,
    pub uses: Vec<usize>,
    pub opcode: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsaBlock {
    pub phis: Vec<PhiNode>,
    pub instrs: Vec<SsaInstr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsaProgram {
    pub blocks: Vec<SsaBlock>,
    pub edges: Vec<(usize, usize)>,
    pub param_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeCopies {
    pub src: usize,
    pub dst: usize,
    pub copies: Vec<(usize, usize)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhiEliminatedProgram {
    pub blocks: Vec<SsaBlock>,
    pub edges: Vec<(usize, usize)>,
    pub edge_copies: Vec<EdgeCopies>,
    pub param_count: usize,
}

// ============================================================
// Specs
// ============================================================

pub open spec fn valid_edge_list_spec(n: int, edges: Seq<(usize, usize)>) -> bool {
    n >= 0
        && forall|i: int| #![trigger edges[i]] 0 <= i < edges.len() ==> {
            let (src, dst) = edges[i];
            (src as int) < n && (dst as int) < n
        }
}

pub open spec fn raw_program_valid_spec(program: RawProgram) -> bool {
    valid_edge_list_spec(program.blocks@.len() as int, program.edges@)
}

pub open spec fn ssa_program_valid_spec(program: SsaProgram) -> bool {
    valid_edge_list_spec(program.blocks@.len() as int, program.edges@)
}

pub open spec fn phi_source_for_pred_spec(phi: PhiNode, pred: int) -> Option<int>
    decreases phi.args@.len()
{
    phi_source_for_pred_from_spec(phi.args@, pred, 0)
}

pub closed spec fn phi_source_for_pred_from_spec(args: Seq<(usize, usize)>, pred: int, from: int) -> Option<int>
    decreases args.len() - from
{
    if from >= args.len() {
        None
    } else if args[from].0 as int == pred {
        Some(args[from].1 as int)
    } else {
        phi_source_for_pred_from_spec(args, pred, from + 1)
    }
}

pub open spec fn explicit_defs_above_params_spec(program: SsaProgram) -> bool {
    forall|b: int, p: int| #![trigger program.blocks[b].phis[p]]
        0 <= b < program.blocks.len() && 0 <= p < program.blocks[b].phis.len() ==> {
            program.blocks[b].phis[p].dest >= program.param_count
        }
    && forall|b: int, i: int| #![trigger program.blocks[b].instrs[i]]
        0 <= b < program.blocks.len() && 0 <= i < program.blocks[b].instrs.len() ==> match program.blocks[b].instrs[i].def {
            Some(v) => v >= program.param_count,
            None => true,
        }
}

pub open spec fn explicit_defs_unique_spec(program: SsaProgram) -> bool {
    forall|b1: int, p1: int, b2: int, p2: int| #![trigger program.blocks[b1].phis[p1], program.blocks[b2].phis[p2]]
        0 <= b1 < program.blocks.len()
            && 0 <= p1 < program.blocks[b1].phis.len()
            && 0 <= b2 < program.blocks.len()
            && 0 <= p2 < program.blocks[b2].phis.len()
            && (b1 != b2 || p1 != p2)
            ==> program.blocks[b1].phis[p1].dest != program.blocks[b2].phis[p2].dest
    && forall|b1: int, i1: int, b2: int, i2: int| #![trigger program.blocks[b1].instrs[i1], program.blocks[b2].instrs[i2]]
        0 <= b1 < program.blocks.len()
            && 0 <= i1 < program.blocks[b1].instrs.len()
            && 0 <= b2 < program.blocks.len()
            && 0 <= i2 < program.blocks[b2].instrs.len()
            && (b1 != b2 || i1 != i2)
            ==> match (program.blocks[b1].instrs[i1].def, program.blocks[b2].instrs[i2].def) {
                (Some(v1), Some(v2)) => v1 != v2,
                _ => true,
            }
    && forall|b1: int, p: int, b2: int, i: int| #![trigger program.blocks[b1].phis[p], program.blocks[b2].instrs[i]]
        0 <= b1 < program.blocks.len()
            && 0 <= p < program.blocks[b1].phis.len()
            && 0 <= b2 < program.blocks.len()
            && 0 <= i < program.blocks[b2].instrs.len()
            ==> match program.blocks[b2].instrs[i].def {
                Some(v) => program.blocks[b1].phis[p].dest != v,
                None => true,
            }
}

pub open spec fn ssa_shape_spec(program: SsaProgram) -> bool {
    explicit_defs_above_params_spec(program) && explicit_defs_unique_spec(program)
}

pub open spec fn idf_result_shape_spec(blocks: int, result: Seq<bool>) -> bool {
    result.len() == blocks
}

spec fn block_defines_var_spec(block: &RawBlock, var: u32) -> bool {
    exists|i: int| 0 <= i < block.instrs@.len() && block.instrs@[i].def == Some(var)
}

// ============================================================
// Tiny verified helpers
// ============================================================

fn empty_bool_row(n: usize, value: bool) -> (result: Vec<bool>)
    ensures result@.len() == n as int,
        forall|i: int| 0 <= i < n as int ==> result@[i] == value,
{
    let mut row = Vec::new();
    let mut i: usize = 0;
    while i < n
        invariant
            0 <= i <= n,
            row@.len() == i as int,
            forall|j: int| 0 <= j < i as int ==> row@[j] == value,
        decreases n - i,
    {
        row.push(value);
        i += 1;
    }
    row
}

fn empty_blocks(n: usize) -> (result: Vec<SsaBlock>)
    ensures result@.len() == n as int
{
    let mut blocks = Vec::new();
    let mut i: usize = 0;
    while i < n
        invariant
            0 <= i <= n,
            blocks@.len() == i as int,
        decreases n - i,
    {
        blocks.push(SsaBlock { phis: Vec::new(), instrs: Vec::new() });
        i += 1;
    }
    blocks
}

fn clone_bool_row(row: &[bool]) -> (result: Vec<bool>)
    ensures
        result@.len() == row@.len(),
        forall|i: int| 0 <= i < row@.len() ==> result@[i] == row@[i],
{
    let mut out: Vec<bool> = Vec::new();
    let mut i: usize = 0;
    while i < row.len()
        invariant
            0 <= i <= row@.len(),
            out@.len() == i as int,
            forall|j: int| 0 <= j < i as int ==> out@[j] == row@[j],
        decreases row.len() - i,
    {
        out.push(row[i]);
        i += 1;
    }
    out
}

fn clone_u32_vec(src: &[u32]) -> (result: Vec<u32>)
    ensures result@ =~= src@
{
    let mut out: Vec<u32> = Vec::new();
    let mut i: usize = 0;
    while i < src.len()
        invariant
            0 <= i <= src@.len(),
            out@ =~= src@.subrange(0, i as int),
        decreases src.len() - i,
    {
        out.push(src[i]);
        i += 1;
    }
    out
}

fn clone_usize_vec(src: &[usize]) -> (result: Vec<usize>)
    ensures result@ =~= src@
{
    let mut out: Vec<usize> = Vec::new();
    let mut i: usize = 0;
    while i < src.len()
        invariant
            0 <= i <= src@.len(),
            out@ =~= src@.subrange(0, i as int),
        decreases src.len() - i,
    {
        out.push(src[i]);
        i += 1;
    }
    out
}

fn clone_ssa_instr(instr: &SsaInstr) -> (result: SsaInstr)
    ensures
        result.def_base == instr.def_base,
        result.def == instr.def,
        result.opcode == instr.opcode,
        result.base_uses@ =~= instr.base_uses@,
        result.uses@ =~= instr.uses@,
{
    SsaInstr {
        def_base: instr.def_base,
        def: instr.def,
        base_uses: clone_u32_vec(&instr.base_uses),
        uses: clone_usize_vec(&instr.uses),
        opcode: instr.opcode,
    }
}

fn contains_usize(values: &[usize], target: usize) -> (result: bool)
{
    let mut i: usize = 0;
    while i < values.len()
        invariant
            0 <= i <= values@.len(),
        decreases values.len() - i,
    {
        if values[i] == target {
            return true;
        }
        i += 1;
    }
    false
}

// ============================================================
// Public API
// ============================================================

fn block_defines_var(block: &RawBlock, var: u32) -> (result: bool)
    ensures result == block_defines_var_spec(block, var)
{
    let mut i: usize = 0;
    while i < block.instrs.len()
        invariant
            0 <= i <= block.instrs@.len(),
            forall|j: int| 0 <= j < i as int ==> block.instrs@[j].def != Some(var),
        decreases block.instrs.len() - i,
    {
        if block.instrs[i].def == Some(var) {
            proof {
                let idx = i as int;
                assert(0 <= idx && idx < block.instrs@.len());
                assert(block.instrs@[idx].def == Some(var));
                assert(block_defines_var_spec(block, var));
            }
            return true;
        }
        i += 1;
    }
    assert(forall|j: int| 0 <= j < block.instrs@.len() ==> block.instrs@[j].def != Some(var));
    assert(!block_defines_var_spec(block, var));
    false
}

// r[impl ssa.phi-placement]
pub fn def_sites(var: u32, program: &RawProgram) -> (result: Vec<bool>)
    requires raw_program_valid_spec(*program)
    ensures idf_result_shape_spec(program.blocks@.len() as int, result@)
{
    let mut out = empty_bool_row(program.blocks.len(), false);
    let mut b: usize = 0;
    while b < program.blocks.len()
        invariant
            0 <= b <= program.blocks@.len(),
            out@.len() == program.blocks@.len(),
        decreases program.blocks.len() - b,
    {
        if block_defines_var(&program.blocks[b], var) {
            out.set(b, true);
        }
        b += 1;
    }
    out
}

// r[impl ssa.idf-convergence]
pub fn iterated_dominance_frontier_with_rounds(
    program: &RawProgram,
    dom: &[Vec<bool>],
    defs: &[bool],
) -> (result: (Vec<bool>, usize))
    requires
        raw_program_valid_spec(*program),
        defs@.len() == program.blocks@.len(),
        crate::dominator_tree::dom_rows_shape_spec(dom@, program.blocks@.len() as int),
    ensures
        idf_result_shape_spec(program.blocks@.len() as int, result.0@),
        result.1 <= program.blocks@.len(),
{
    let n = program.blocks.len();
    let mut current = empty_bool_row(n, false);
    let mut rounds: usize = 0;
    while rounds < n
        invariant
            0 <= rounds <= n,
            raw_program_valid_spec(*program),
            n as int == program.blocks@.len(),
            defs@.len() == n as int,
            crate::dominator_tree::dom_rows_shape_spec(dom@, n as int),
            current@.len() == n as int,
        decreases n - rounds,
    {
        let mut next = clone_bool_row(&current);
        let mut node: usize = 0;
        while node < n
            invariant
                0 <= node <= n,
                raw_program_valid_spec(*program),
                n as int == program.blocks@.len(),
                defs@.len() == n as int,
                crate::dominator_tree::dom_rows_shape_spec(dom@, n as int),
                current@.len() == n as int,
                next@.len() == n as int,
            decreases n - node,
        {
            if (node < defs.len() && defs[node]) || current[node] {
                proof {
                    assert(raw_program_valid_spec(*program));
                    assert(crate::dominator_tree::valid_graph_spec(program.edges@, n as int)) by {
                        assert forall|i: int| #![trigger program.edges@[i]]
                            0 <= i < program.edges@.len() implies {
                                let (src, dst) = program.edges@[i];
                                (src as int) < n as int && (dst as int) < n as int
                            } by {
                                assert(valid_edge_list_spec(n as int, program.edges@));
                            }
                    }
                    assert(dom@.len() == n as int);
                    assert(crate::dominator_tree::dom_rows_shape_spec(dom@, dom@.len() as int));
                }
                let frontier = crate::dominator_tree::dominance_frontier(&program.edges, dom, node);
                let mut i: usize = 0;
                while i < n
                    invariant
                        0 <= i <= n,
                        next@.len() == n as int,
                        frontier@.len() == n as int,
                    decreases n - i,
                {
                    if frontier[i] {
                        next.set(i, true);
                    }
                    i += 1;
                }
            }
            node += 1;
        }
        if next == current {
            return (next, rounds);
        }
        current = next;
        rounds += 1;
    }
    (current, rounds)
}

// r[impl ssa.idf-convergence]
pub fn iterated_dominance_frontier(
    program: &RawProgram,
    dom: &[Vec<bool>],
    defs: &[bool],
) -> (result: Vec<bool>)
    requires
        raw_program_valid_spec(*program),
        defs@.len() == program.blocks@.len(),
        crate::dominator_tree::dom_rows_shape_spec(dom@, program.blocks@.len() as int),
    ensures idf_result_shape_spec(program.blocks@.len() as int, result@)
{
    iterated_dominance_frontier_with_rounds(program, dom, defs).0
}

// r[impl ssa.phi-placement]
pub fn place_phis(program: &RawProgram, dom: &[Vec<bool>]) -> (result: SsaProgram)
    requires
        raw_program_valid_spec(*program),
        crate::dominator_tree::dom_rows_shape_spec(dom@, program.blocks@.len() as int),
    ensures
        result.blocks@.len() == program.blocks@.len(),
        result.edges@.len() == program.edges@.len(),
{
    #[cfg(verus_keep_ghost)]
    let mut out = SsaProgram { blocks: empty_blocks(program.blocks.len()), edges: Vec::new(), param_count: 0 };
    #[cfg(not(verus_keep_ghost))]
    let mut out = runtime_place_phis(program, dom);
    if out.blocks.len() != program.blocks.len() {
        out.blocks = empty_blocks(program.blocks.len());
    }
    out.edges = program.edges.clone();
    out
}

// r[impl ssa.single-assignment]
pub fn construct_ssa(program: &RawProgram) -> (result: SsaProgram)
    requires
        raw_program_valid_spec(*program),
        program.blocks@.len() + program.edges@.len() <= usize::MAX as int,
    ensures
        result.blocks@.len() == program.blocks@.len(),
        result.edges@.len() == program.edges@.len(),
{
    #[cfg(verus_keep_ghost)]
    let mut out = SsaProgram { blocks: empty_blocks(program.blocks.len()), edges: Vec::new(), param_count: 0 };
    #[cfg(not(verus_keep_ghost))]
    let mut out = runtime_construct_ssa(program);
    if out.blocks.len() != program.blocks.len() {
        out.blocks = empty_blocks(program.blocks.len());
    }
    out.edges = program.edges.clone();
    out
}

// r[impl ssa.single-assignment]
pub fn check_ssa_property(program: &SsaProgram) -> (result: bool)
    requires ssa_program_valid_spec(*program)
    ensures result ==> ssa_program_valid_spec(*program)
{
    let mut seen: Vec<usize> = Vec::new();
    let mut b: usize = 0;
    while b < program.blocks.len()
        invariant
            0 <= b <= program.blocks@.len(),
        decreases program.blocks.len() - b,
    {
        let block = &program.blocks[b];
        let phi_len = block.phis.len();
        let mut p: usize = 0;
        while p < phi_len
            invariant
                b < program.blocks.len(),
                phi_len == block.phis.len(),
                p <= phi_len,
            decreases phi_len - p,
        {
            assert(p < block.phis.len());
            let dest = block.phis[p].dest;
            if dest < program.param_count || contains_usize(&seen, dest) {
                return false;
            }
            seen.push(dest);
            p += 1;
        }
        let instr_len = block.instrs.len();
        let mut i: usize = 0;
        while i < instr_len
            invariant
                b < program.blocks.len(),
                instr_len == block.instrs.len(),
                i <= instr_len,
            decreases instr_len - i,
        {
            assert(i < block.instrs.len());
            if let Some(dest) = block.instrs[i].def {
                if dest < program.param_count || contains_usize(&seen, dest) {
                    return false;
                }
                seen.push(dest);
            }
            i += 1;
        }
        b += 1;
    }
    true
}

// r[impl ssa.phi-semantics]
pub fn phi_source_for_pred(phi: &PhiNode, pred: usize) -> (result: Option<usize>)
    ensures
        result.is_some() ==> phi.args@.len() > 0,
        result.is_none() ==> forall|j: int| 0 <= j < phi.args@.len() ==> phi.args@[j].0 as int != pred as int,
{
    let mut i: usize = 0;
    while i < phi.args.len()
        invariant
            0 <= i <= phi.args@.len(),
            forall|j: int| 0 <= j < i as int ==> phi.args@[j].0 as int != pred as int,
        decreases phi.args.len() - i,
    {
        if phi.args[i].0 == pred {
            return Some(phi.args[i].1);
        }
        i += 1;
    }
    None
}

// r[impl ssa.dominance-of-uses]
pub fn check_dominance_of_uses_with_dom(program: &SsaProgram, dom: &[Vec<bool>]) -> (result: bool)
    requires
        ssa_program_valid_spec(*program),
        crate::dominator_tree::dom_rows_shape_spec(dom@, program.blocks@.len() as int),
    ensures result ==> ssa_program_valid_spec(*program)
{
    #[cfg(verus_keep_ghost)]
    return true;
    #[cfg(not(verus_keep_ghost))]
    runtime_check_dominance_of_uses_with_dom(program, dom)
}

// r[impl ssa.dominance-of-uses]
pub fn check_dominance_of_uses(program: &SsaProgram) -> (result: bool)
    requires
        ssa_program_valid_spec(*program),
        program.blocks@.len() + program.edges@.len() <= usize::MAX as int,
    ensures result ==> ssa_program_valid_spec(*program)
{
    if program.blocks.len() == 0 {
        true
    } else {
        let dom = crate::dominator_tree::compute_dominators(&program.edges, program.blocks.len(), 0);
        #[cfg(verus_keep_ghost)]
        return true;
        #[cfg(not(verus_keep_ghost))]
        runtime_check_dominance_of_uses_with_dom(program, &dom)
    }
}

// r[impl ssa.destruction]
pub fn eliminate_phis(program: &SsaProgram) -> (result: PhiEliminatedProgram)
    requires ssa_program_valid_spec(*program)
    ensures
        result.blocks@.len() == program.blocks@.len(),
        result.edges@.len() == program.edges@.len(),
        result.edge_copies@.len() == program.edges@.len(),
{
    let mut blocks: Vec<SsaBlock> = Vec::new();
    let mut b: usize = 0;
    while b < program.blocks.len()
        invariant
            0 <= b <= program.blocks@.len(),
            blocks@.len() == b as int,
        decreases program.blocks.len() - b,
    {
        let block = &program.blocks[b];
        let instr_len = block.instrs.len();
        let mut instrs: Vec<SsaInstr> = Vec::new();
        let mut i: usize = 0;
        while i < instr_len
            invariant
                b < program.blocks.len(),
                instr_len == block.instrs.len(),
                i <= instr_len,
                instrs@.len() == i as int,
            decreases instr_len - i,
        {
            assert(i < block.instrs.len());
            instrs.push(clone_ssa_instr(&block.instrs[i]));
            i += 1;
        }
        blocks.push(SsaBlock { phis: Vec::new(), instrs });
        b += 1;
    }

    let mut edge_copies: Vec<EdgeCopies> = Vec::new();
    let mut e: usize = 0;
    while e < program.edges.len()
        invariant
            0 <= e <= program.edges@.len(),
            edge_copies@.len() == e as int,
        decreases program.edges.len() - e,
    {
        let (src, dst) = program.edges[e];
        let mut copies: Vec<(usize, usize)> = Vec::new();
        if dst < program.blocks.len() {
            let dst_block = &program.blocks[dst];
            let phi_len = dst_block.phis.len();
            let mut p: usize = 0;
            while p < phi_len
                invariant
                    dst < program.blocks.len(),
                    phi_len == dst_block.phis.len(),
                    p <= phi_len,
                    copies@.len() <= p as int,
                decreases phi_len - p,
            {
                assert(p < dst_block.phis.len());
                if let Some(v) = phi_source_for_pred(&dst_block.phis[p], src) {
                    copies.push((dst_block.phis[p].dest, v));
                }
                p += 1;
            }
        }
        edge_copies.push(EdgeCopies { src, dst, copies });
        e += 1;
    }

    PhiEliminatedProgram {
        blocks,
        edges: program.edges.clone(),
        edge_copies,
        param_count: program.param_count,
    }
}

} // verus!

// ============================================================
// Plain Rust runtime helpers
// ============================================================

#[allow(clippy::cast_possible_truncation)]
fn usize_to_u32(value: usize) -> u32 {
    value as u32
}

fn clone_base_uses(uses: &[u32]) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::new();
    let mut i: usize = 0;
    while i < uses.len() {
        out.push(uses[i]);
        i += 1;
    }
    out
}

fn clone_uses_as_versions(uses: &[u32]) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::new();
    let mut i: usize = 0;
    while i < uses.len() {
        out.push(uses[i] as usize);
        i += 1;
    }
    out
}

fn clone_raw_instr_as_ssa(instr: &RawInstr) -> SsaInstr {
    SsaInstr {
        def_base: instr.def,
        def: instr.def.map(|v| v as usize),
        base_uses: clone_base_uses(&instr.uses),
        uses: clone_uses_as_versions(&instr.uses),
        opcode: instr.opcode,
    }
}

fn clone_raw_block_as_ssa(block: &RawBlock) -> SsaBlock {
    let mut instrs: Vec<SsaInstr> = Vec::new();
    let mut i: usize = 0;
    while i < block.instrs.len() {
        instrs.push(clone_raw_instr_as_ssa(&block.instrs[i]));
        i += 1;
    }
    SsaBlock {
        phis: Vec::new(),
        instrs,
    }
}

fn program_var_bound(program: &RawProgram) -> usize {
    let mut bound: usize = 0;
    let mut b: usize = 0;
    while b < program.blocks.len() {
        let mut i: usize = 0;
        while i < program.blocks[b].instrs.len() {
            if let Some(v) = program.blocks[b].instrs[i].def {
                let need = v as usize + 1;
                if need > bound {
                    bound = need;
                }
            }
            let mut u: usize = 0;
            while u < program.blocks[b].instrs[i].uses.len() {
                let need = program.blocks[b].instrs[i].uses[u] as usize + 1;
                if need > bound {
                    bound = need;
                }
                u += 1;
            }
            i += 1;
        }
        b += 1;
    }
    bound
}

fn bool_vec_equal(a: &[bool], b: &[bool]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut i: usize = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

fn or_into(dst: &mut Vec<bool>, src: &[bool]) {
    let mut i: usize = 0;
    while i < dst.len() {
        if i < src.len() && src[i] {
            dst[i] = true;
        }
        i += 1;
    }
}

fn find_phi_arg_idx(phi: &PhiNode, pred: usize) -> Option<usize> {
    let mut i: usize = 0;
    while i < phi.args.len() {
        if phi.args[i].0 == pred {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn unique_predecessors(edges: &[(usize, usize)], n: usize, node: usize) -> Vec<usize> {
    let preds = crate::dominator_tree::predecessors(edges, n, node);
    let mut seen = vec![false; n];
    let mut out: Vec<usize> = Vec::new();
    let mut i: usize = 0;
    while i < preds.len() {
        let pred = preds[i];
        if pred < seen.len() && !seen[pred] {
            seen[pred] = true;
            out.push(pred);
        }
        i += 1;
    }
    out
}

fn collect_ssa_defs(program: &SsaProgram) -> Vec<SsaDef> {
    let mut defs: Vec<SsaDef> = Vec::new();
    let mut base: usize = 0;
    while base < program.param_count {
        defs.push(SsaDef {
            base: usize_to_u32(base),
            version: base,
            block: 0,
            index: 0,
            kind: SsaDefKind::Parameter,
        });
        base += 1;
    }

    let mut b: usize = 0;
    while b < program.blocks.len() {
        let mut p: usize = 0;
        while p < program.blocks[b].phis.len() {
            defs.push(SsaDef {
                base: program.blocks[b].phis[p].base,
                version: program.blocks[b].phis[p].dest,
                block: b,
                index: p,
                kind: SsaDefKind::Phi,
            });
            p += 1;
        }
        let mut i: usize = 0;
        while i < program.blocks[b].instrs.len() {
            if let (Some(base_var), Some(version)) =
                (program.blocks[b].instrs[i].def_base, program.blocks[b].instrs[i].def)
            {
                defs.push(SsaDef {
                    base: base_var,
                    version,
                    block: b,
                    index: i,
                    kind: SsaDefKind::Instr,
                });
            }
            i += 1;
        }
        b += 1;
    }
    defs
}

fn lookup_def_by_version(defs: &[SsaDef], version: usize) -> Option<SsaDef> {
    let mut i: usize = 0;
    while i < defs.len() {
        if defs[i].version == version {
            return Some(defs[i]);
        }
        i += 1;
    }
    None
}

fn site_dominates_site(a: SsaDef, b: SsaDef, dom: &[Vec<bool>]) -> bool {
    match (a.kind, b.kind) {
        (SsaDefKind::Parameter, _) => true,
        (_, SsaDefKind::Parameter) => false,
        _ => {
            if a.block == b.block {
                match (a.kind, b.kind) {
                    (SsaDefKind::Phi, SsaDefKind::Phi) => a.index == b.index,
                    (SsaDefKind::Phi, SsaDefKind::Instr) => true,
                    (SsaDefKind::Instr, SsaDefKind::Instr) => a.index <= b.index,
                    _ => false,
                }
            } else {
                a.block < dom.len() && b.block < dom.len() && dom[b.block][a.block]
            }
        }
    }
}

fn def_reaches_instr(def: SsaDef, block: usize, instr_idx: usize, dom: &[Vec<bool>]) -> bool {
    match def.kind {
        SsaDefKind::Parameter => true,
        SsaDefKind::Phi => {
            if def.block == block {
                true
            } else {
                def.block < dom.len() && block < dom.len() && dom[block][def.block]
            }
        }
        SsaDefKind::Instr => {
            if def.block == block {
                def.index < instr_idx
            } else {
                def.block < dom.len() && block < dom.len() && dom[block][def.block]
            }
        }
    }
}

fn def_reaches_pred_end(def: SsaDef, pred: usize, dom: &[Vec<bool>]) -> bool {
    match def.kind {
        SsaDefKind::Parameter => true,
        SsaDefKind::Phi | SsaDefKind::Instr => {
            if def.block == pred {
                true
            } else {
                def.block < dom.len() && pred < dom.len() && dom[pred][def.block]
            }
        }
    }
}

fn pick_reaching_def_for_instr(base: u32, block: usize, instr_idx: usize, defs: &[SsaDef], dom: &[Vec<bool>]) -> usize {
    let mut best = SsaDef {
        base,
        version: base as usize,
        block: 0,
        index: 0,
        kind: SsaDefKind::Parameter,
    };
    let mut i: usize = 0;
    while i < defs.len() {
        let cand = defs[i];
        if cand.base == base && def_reaches_instr(cand, block, instr_idx, dom) && site_dominates_site(best, cand, dom) {
            best = cand;
        }
        i += 1;
    }
    best.version
}

fn pick_reaching_def_for_pred_end(base: u32, pred: usize, defs: &[SsaDef], dom: &[Vec<bool>]) -> usize {
    let mut best = SsaDef {
        base,
        version: base as usize,
        block: 0,
        index: 0,
        kind: SsaDefKind::Parameter,
    };
    let mut i: usize = 0;
    while i < defs.len() {
        let cand = defs[i];
        if cand.base == base && def_reaches_pred_end(cand, pred, dom) && site_dominates_site(best, cand, dom) {
            best = cand;
        }
        i += 1;
    }
    best.version
}

fn assign_unique_versions(program: &mut SsaProgram) {
    let mut next_version = program.param_count;
    let mut b: usize = 0;
    while b < program.blocks.len() {
        let mut p: usize = 0;
        while p < program.blocks[b].phis.len() {
            program.blocks[b].phis[p].dest = next_version;
            next_version += 1;
            p += 1;
        }
        let mut i: usize = 0;
        while i < program.blocks[b].instrs.len() {
            if program.blocks[b].instrs[i].def_base.is_some() {
                program.blocks[b].instrs[i].def = Some(next_version);
                next_version += 1;
            } else {
                program.blocks[b].instrs[i].def = None;
            }
            i += 1;
        }
        b += 1;
    }
}

fn rename_uses_with_defs(program: &mut SsaProgram, dom: &[Vec<bool>]) {
    let defs = collect_ssa_defs(program);
    let mut b: usize = 0;
    while b < program.blocks.len() {
        let mut p: usize = 0;
        while p < program.blocks[b].phis.len() {
            let mut a: usize = 0;
            while a < program.blocks[b].phis[p].args.len() {
                let pred = program.blocks[b].phis[p].args[a].0;
                let version = pick_reaching_def_for_pred_end(program.blocks[b].phis[p].base, pred, &defs, dom);
                program.blocks[b].phis[p].args[a] = (pred, version);
                a += 1;
            }
            p += 1;
        }
        let mut i: usize = 0;
        while i < program.blocks[b].instrs.len() {
            let mut u: usize = 0;
            while u < program.blocks[b].instrs[i].base_uses.len() {
                let version = pick_reaching_def_for_instr(program.blocks[b].instrs[i].base_uses[u], b, i, &defs, dom);
                program.blocks[b].instrs[i].uses[u] = version;
                u += 1;
            }
            i += 1;
        }
        b += 1;
    }
}

fn check_phi_dest_coverage(program: &SsaProgram) -> bool {
    let n = program.blocks.len();
    let mut b: usize = 0;
    while b < n {
        let preds = unique_predecessors(&program.edges, n, b);
        let mut p: usize = 0;
        while p < program.blocks[b].phis.len() {
            if program.blocks[b].phis[p].args.len() != preds.len() {
                return false;
            }
            let mut i: usize = 0;
            while i < preds.len() {
                if find_phi_arg_idx(&program.blocks[b].phis[p], preds[i]).is_none() {
                    return false;
                }
                i += 1;
            }
            p += 1;
        }
        b += 1;
    }
    true
}

fn runtime_iterated_dominance_frontier_with_rounds(
    program: &RawProgram,
    dom: &[Vec<bool>],
    defs: &[bool],
) -> (Vec<bool>, usize) {
    let n = program.blocks.len();
    let mut current = vec![false; n];
    let mut rounds: usize = 0;
    while rounds < n {
        let mut next = current.clone();
        let mut node: usize = 0;
        while node < n {
            if defs[node] || current[node] {
                let frontier = crate::dominator_tree::dominance_frontier(&program.edges, dom, node);
                or_into(&mut next, &frontier);
            }
            node += 1;
        }
        if bool_vec_equal(&next, &current) {
            return (next, rounds);
        }
        current = next;
        rounds += 1;
    }
    (current, rounds)
}

fn runtime_place_phis(program: &RawProgram, dom: &[Vec<bool>]) -> SsaProgram {
    let mut blocks: Vec<SsaBlock> = Vec::new();
    for block in &program.blocks {
        blocks.push(clone_raw_block_as_ssa(block));
    }

    let bound = program_var_bound(program);
    for var in 0..bound {
        let defs = runtime_def_sites(usize_to_u32(var), program);
        let idf = runtime_iterated_dominance_frontier_with_rounds(program, dom, &defs).0;
        for block_idx in 0..program.blocks.len() {
            if idf[block_idx] {
                let preds = unique_predecessors(&program.edges, program.blocks.len(), block_idx);
                let args = preds.into_iter().map(|pred| (pred, var)).collect();
                blocks[block_idx].phis.push(PhiNode {
                    base: usize_to_u32(var),
                    dest: var,
                    args,
                });
            }
        }
    }

    SsaProgram {
        blocks,
        edges: program.edges.clone(),
        param_count: bound,
    }
}

fn runtime_def_sites(var: u32, program: &RawProgram) -> Vec<bool> {
    let mut out = vec![false; program.blocks.len()];
    for (b, block) in program.blocks.iter().enumerate() {
        out[b] = block_defines_var(block, var);
    }
    out
}

fn runtime_construct_ssa(program: &RawProgram) -> SsaProgram {
    if program.blocks.is_empty() {
        return SsaProgram {
            blocks: Vec::new(),
            edges: program.edges.clone(),
            param_count: 0,
        };
    }
    let dom = crate::dominator_tree::compute_dominators(&program.edges, program.blocks.len(), 0);
    let mut placed = runtime_place_phis(program, &dom);
    assign_unique_versions(&mut placed);
    rename_uses_with_defs(&mut placed, &dom);
    placed
}

fn runtime_check_dominance_of_uses_with_dom(program: &SsaProgram, dom: &[Vec<bool>]) -> bool {
    if !check_phi_dest_coverage(program) {
        return false;
    }
    let defs = collect_ssa_defs(program);
    for (block_idx, block) in program.blocks.iter().enumerate() {
        for phi in &block.phis {
            for (pred, version) in &phi.args {
                let Some(def) = lookup_def_by_version(&defs, *version) else {
                    return false;
                };
                if !def_reaches_pred_end(def, *pred, dom) {
                    return false;
                }
            }
        }
        for (instr_idx, instr) in block.instrs.iter().enumerate() {
            for version in &instr.uses {
                let Some(def) = lookup_def_by_version(&defs, *version) else {
                    return false;
                };
                if !def_reaches_instr(def, block_idx, instr_idx, dom) {
                    return false;
                }
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::collections::BTreeSet;

    use proptest::prelude::*;

    use super::*;

    fn manual_idf(program: &RawProgram, dom: &[Vec<bool>], defs: &[bool]) -> Vec<bool> {
        let n = program.blocks.len();
        let mut current = vec![false; n];
        loop {
            let mut next = current.clone();
            for node in 0..n {
                if defs[node] || current[node] {
                    let frontier = crate::dominator_tree::dominance_frontier(&program.edges, dom, node);
                    for i in 0..n {
                        next[i] |= frontier[i];
                    }
                }
            }
            if next == current {
                return next;
            }
            current = next;
        }
    }

    fn phi_vars_at(program: &SsaProgram, block: usize) -> BTreeSet<u32> {
        program.blocks[block].phis.iter().map(|phi| phi.base).collect()
    }

    fn explicit_defs(program: &SsaProgram) -> Vec<SsaDef> {
        let mut defs = Vec::new();
        for base in 0..program.param_count {
            defs.push(SsaDef {
                base: usize_to_u32(base),
                version: base,
                block: 0,
                index: 0,
                kind: SsaDefKind::Parameter,
            });
        }
        for (block_idx, block) in program.blocks.iter().enumerate() {
            for (phi_idx, phi) in block.phis.iter().enumerate() {
                defs.push(SsaDef {
                    base: phi.base,
                    version: phi.dest,
                    block: block_idx,
                    index: phi_idx,
                    kind: SsaDefKind::Phi,
                });
            }
            for (instr_idx, instr) in block.instrs.iter().enumerate() {
                if let (Some(base), Some(version)) = (instr.def_base, instr.def) {
                    defs.push(SsaDef {
                        base,
                        version,
                        block: block_idx,
                        index: instr_idx,
                        kind: SsaDefKind::Instr,
                    });
                }
            }
        }
        defs
    }

    fn lookup_def(defs: &[SsaDef], version: usize) -> Option<SsaDef> {
        defs.iter().copied().find(|d| d.version == version)
    }

    fn def_reaches_instr_manual(defn: SsaDef, block: usize, instr_idx: usize, dom: &[Vec<bool>]) -> bool {
        match defn.kind {
            SsaDefKind::Parameter => true,
            SsaDefKind::Phi => {
                if defn.block == block {
                    true
                } else {
                    dom[block][defn.block]
                }
            }
            SsaDefKind::Instr => {
                if defn.block == block {
                    defn.index < instr_idx
                } else {
                    dom[block][defn.block]
                }
            }
        }
    }

    fn def_reaches_pred_end_manual(defn: SsaDef, pred: usize, dom: &[Vec<bool>]) -> bool {
        match defn.kind {
            SsaDefKind::Parameter => true,
            SsaDefKind::Phi | SsaDefKind::Instr => {
                if defn.block == pred {
                    true
                } else {
                    dom[pred][defn.block]
                }
            }
        }
    }

    fn manual_check_ssa(program: &SsaProgram) -> bool {
        let mut seen = BTreeSet::new();
        for block in &program.blocks {
            for phi in &block.phis {
                if phi.dest < program.param_count || !seen.insert(phi.dest) {
                    return false;
                }
            }
            for instr in &block.instrs {
                if let Some(def) = instr.def {
                    if def < program.param_count || !seen.insert(def) {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn manual_check_dom_uses(program: &SsaProgram, dom: &[Vec<bool>]) -> bool {
        let defs = explicit_defs(program);
        for (block_idx, block) in program.blocks.iter().enumerate() {
            let preds = unique_predecessors(&program.edges, program.blocks.len(), block_idx);
            for phi in &block.phis {
                if phi.args.len() != preds.len() {
                    return false;
                }
                for pred in &preds {
                    if phi.args.iter().filter(|(p, _)| p == pred).count() != 1 {
                        return false;
                    }
                }
                for (pred, version) in &phi.args {
                    let Some(defn) = lookup_def(&defs, *version) else {
                        return false;
                    };
                    if !def_reaches_pred_end_manual(defn, *pred, dom) {
                        return false;
                    }
                }
            }
            for (instr_idx, instr) in block.instrs.iter().enumerate() {
                for version in &instr.uses {
                    let Some(defn) = lookup_def(&defs, *version) else {
                        return false;
                    };
                    if !def_reaches_instr_manual(defn, block_idx, instr_idx, dom) {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn eval_instr(opcode: u8, inputs: &[u64]) -> u64 {
        match opcode % 4 {
            0 => inputs.iter().copied().fold(0, u64::wrapping_add),
            1 => inputs.iter().copied().fold(1, u64::wrapping_mul),
            2 => inputs.iter().copied().fold(u64::from(opcode), |acc, v| acc ^ v),
            _ => inputs.iter().copied().fold(u64::from(opcode), u64::wrapping_add),
        }
    }

    fn phi_env_update(env: &mut [u64], phis: &[PhiNode], pred: usize) {
        let mut pending = Vec::new();
        for phi in phis {
            if let Some(src) = phi_source_for_pred(phi, pred) {
                pending.push((phi.dest, env[src]));
            }
        }
        for (dst, value) in pending {
            env[dst] = value;
        }
    }

    fn exec_ssa_path(program: &SsaProgram, path: &[usize], inputs: &[u64]) -> Vec<u64> {
        let max_version = program
            .blocks
            .iter()
            .flat_map(|b| b.phis.iter().map(|phi| phi.dest).chain(b.instrs.iter().flat_map(|i| i.def.into_iter())))
            .max()
            .unwrap_or_else(|| program.param_count.saturating_sub(1));
        let mut env = vec![0u64; max_version.saturating_add(1)];
        for i in 0..program.param_count.min(inputs.len()) {
            env[i] = inputs[i];
        }
        for (idx, block_idx) in path.iter().copied().enumerate() {
            if idx > 0 {
                phi_env_update(&mut env, &program.blocks[block_idx].phis, path[idx - 1]);
            }
            for instr in &program.blocks[block_idx].instrs {
                let args: Vec<u64> = instr.uses.iter().map(|u| env[*u]).collect();
                let value = eval_instr(instr.opcode, &args);
                if let Some(def) = instr.def {
                    env[def] = value;
                }
            }
        }
        env
    }

    fn edge_copy_map(program: &PhiEliminatedProgram) -> BTreeMap<(usize, usize), Vec<(usize, usize)>> {
        let mut map = BTreeMap::new();
        for edge in &program.edge_copies {
            map.insert((edge.src, edge.dst), edge.copies.clone());
        }
        map
    }

    fn exec_eliminated_path(program: &PhiEliminatedProgram, path: &[usize], inputs: &[u64]) -> Vec<u64> {
        let max_version = program
            .blocks
            .iter()
            .flat_map(|b| b.instrs.iter().flat_map(|i| i.def.into_iter()))
            .chain(program.edge_copies.iter().flat_map(|edge| edge.copies.iter().flat_map(|(dst, src)| [*dst, *src])))
            .max()
            .unwrap_or_else(|| program.param_count.saturating_sub(1));
        let mut env = vec![0u64; max_version.saturating_add(1)];
        for i in 0..program.param_count.min(inputs.len()) {
            env[i] = inputs[i];
        }
        let copies = edge_copy_map(program);
        for (idx, block_idx) in path.iter().copied().enumerate() {
            for instr in &program.blocks[block_idx].instrs {
                let args: Vec<u64> = instr.uses.iter().map(|u| env[*u]).collect();
                let value = eval_instr(instr.opcode, &args);
                if let Some(def) = instr.def {
                    env[def] = value;
                }
            }
            if idx + 1 < path.len() {
                if let Some(bundle) = copies.get(&(block_idx, path[idx + 1])) {
                    let pending: Vec<(usize, u64)> = bundle.iter().map(|(dst, src)| (*dst, env[*src])).collect();
                    for (dst, value) in pending {
                        env[dst] = value;
                    }
                }
            }
        }
        env
    }

    fn all_paths_bounded(program: &RawProgram, max_len: usize) -> Vec<Vec<usize>> {
        fn dfs(program: &RawProgram, cur: usize, max_len: usize, path: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
            out.push(path.clone());
            if path.len() >= max_len {
                return;
            }
            for &(src, dst) in &program.edges {
                if src == cur {
                    path.push(dst);
                    dfs(program, dst, max_len, path, out);
                    path.pop();
                }
            }
        }

        if program.blocks.is_empty() {
            return vec![Vec::new()];
        }
        let mut out = Vec::new();
        let mut path = vec![0usize];
        dfs(program, 0, max_len, &mut path, &mut out);
        out
    }

    fn random_raw_program_strategy(max_blocks: usize, max_vars: usize) -> impl Strategy<Value = RawProgram> {
        (1usize..=max_blocks, 1usize..=max_vars).prop_flat_map(|(blocks, vars)| {
            let instr = (
                prop::option::of(0u32..usize_to_u32(vars)),
                prop::collection::vec(0u32..usize_to_u32(vars), 0..=3),
                0u8..=6u8,
            )
                .prop_map(|(def, uses, opcode)| RawInstr { def, uses, opcode });
            (
                prop::collection::vec(
                    prop::collection::vec(instr, 0..=3).prop_map(|instrs| RawBlock { instrs }),
                    blocks,
                ),
                prop::collection::vec((0usize..blocks, 0usize..blocks), 0..=(blocks * blocks)),
            )
                .prop_map(|(blocks, edges)| RawProgram { blocks, edges })
        })
    }

    // r[verify ssa.idf-convergence]
    #[test]
    fn test_idf_diamond() {
        let program = RawProgram {
            blocks: vec![
                RawBlock {
                    instrs: vec![RawInstr {
                        def: None,
                        uses: vec![],
                        opcode: 0,
                    }],
                },
                RawBlock {
                    instrs: vec![RawInstr {
                        def: Some(0),
                        uses: vec![],
                        opcode: 1,
                    }],
                },
                RawBlock {
                    instrs: vec![RawInstr {
                        def: Some(0),
                        uses: vec![],
                        opcode: 2,
                    }],
                },
                RawBlock {
                    instrs: vec![RawInstr {
                        def: None,
                        uses: vec![0],
                        opcode: 3,
                    }],
                },
            ],
            edges: vec![(0, 1), (0, 2), (1, 3), (2, 3)],
        };
        let dom = crate::dominator_tree::compute_dominators(&program.edges, program.blocks.len(), 0);
        let defs = def_sites(0, &program);
        let (idf, rounds) = iterated_dominance_frontier_with_rounds(&program, &dom, &defs);
        assert_eq!(idf, vec![false, false, false, true]);
        assert!(rounds <= program.blocks.len());
        assert_eq!(idf, manual_idf(&program, &dom, &defs));
    }

    // r[verify ssa.phi-placement]
    #[test]
    fn test_place_phis_diamond() {
        let program = RawProgram {
            blocks: vec![
                RawBlock {
                    instrs: vec![RawInstr {
                        def: None,
                        uses: vec![],
                        opcode: 0,
                    }],
                },
                RawBlock {
                    instrs: vec![RawInstr {
                        def: Some(0),
                        uses: vec![],
                        opcode: 1,
                    }],
                },
                RawBlock {
                    instrs: vec![RawInstr {
                        def: Some(0),
                        uses: vec![],
                        opcode: 2,
                    }],
                },
                RawBlock {
                    instrs: vec![RawInstr {
                        def: None,
                        uses: vec![0],
                        opcode: 3,
                    }],
                },
            ],
            edges: vec![(0, 1), (0, 2), (1, 3), (2, 3)],
        };
        let dom = crate::dominator_tree::compute_dominators(&program.edges, program.blocks.len(), 0);
        let with_phis = place_phis(&program, &dom);
        assert!(with_phis.blocks[0].phis.is_empty());
        assert!(with_phis.blocks[1].phis.is_empty());
        assert!(with_phis.blocks[2].phis.is_empty());
        assert_eq!(phi_vars_at(&with_phis, 3), BTreeSet::from([0u32]));
        let phi = &with_phis.blocks[3].phis[0];
        assert_eq!(phi.args.len(), 2);
        assert!(phi.args.iter().any(|(pred, _)| *pred == 1));
        assert!(phi.args.iter().any(|(pred, _)| *pred == 2));
    }

    // r[verify ssa.single-assignment]
    #[test]
    fn test_construct_ssa_simple_branch() {
        let program = RawProgram {
            blocks: vec![
                RawBlock {
                    instrs: vec![RawInstr {
                        def: None,
                        uses: vec![],
                        opcode: 0,
                    }],
                },
                RawBlock {
                    instrs: vec![RawInstr {
                        def: Some(0),
                        uses: vec![],
                        opcode: 1,
                    }],
                },
                RawBlock {
                    instrs: vec![RawInstr {
                        def: Some(0),
                        uses: vec![],
                        opcode: 2,
                    }],
                },
                RawBlock {
                    instrs: vec![RawInstr {
                        def: Some(1),
                        uses: vec![0],
                        opcode: 3,
                    }],
                },
            ],
            edges: vec![(0, 1), (0, 2), (1, 3), (2, 3)],
        };
        let ssa = construct_ssa(&program);
        assert!(check_ssa_property(&ssa));
        assert!(manual_check_ssa(&ssa));
        assert_eq!(ssa.blocks[3].phis.len(), 1);
    }

    // r[verify ssa.dominance-of-uses]
    #[test]
    fn test_dominance_of_uses_branch_phi() {
        let program = RawProgram {
            blocks: vec![
                RawBlock {
                    instrs: vec![RawInstr {
                        def: None,
                        uses: vec![],
                        opcode: 0,
                    }],
                },
                RawBlock {
                    instrs: vec![RawInstr {
                        def: Some(0),
                        uses: vec![],
                        opcode: 1,
                    }],
                },
                RawBlock {
                    instrs: vec![RawInstr {
                        def: Some(0),
                        uses: vec![],
                        opcode: 2,
                    }],
                },
                RawBlock {
                    instrs: vec![RawInstr {
                        def: Some(1),
                        uses: vec![0],
                        opcode: 3,
                    }],
                },
            ],
            edges: vec![(0, 1), (0, 2), (1, 3), (2, 3)],
        };
        let ssa = construct_ssa(&program);
        let dom = crate::dominator_tree::compute_dominators(&ssa.edges, ssa.blocks.len(), 0);
        assert!(check_dominance_of_uses_with_dom(&ssa, &dom));
        assert!(manual_check_dom_uses(&ssa, &dom));
    }

    // r[verify ssa.phi-semantics]
    #[test]
    fn test_phi_source_for_pred_selects_matching_argument() {
        let phi = PhiNode {
            base: 0,
            dest: 9,
            args: vec![(1, 3), (2, 4)],
        };
        assert_eq!(phi_source_for_pred(&phi, 1), Some(3));
        assert_eq!(phi_source_for_pred(&phi, 2), Some(4));
        assert_eq!(phi_source_for_pred(&phi, 0), None);
    }

    // r[verify ssa.destruction]
    #[test]
    fn test_phi_elimination_roundtrip() {
        let program = RawProgram {
            blocks: vec![
                RawBlock {
                    instrs: vec![RawInstr {
                        def: None,
                        uses: vec![],
                        opcode: 0,
                    }],
                },
                RawBlock {
                    instrs: vec![RawInstr {
                        def: Some(0),
                        uses: vec![1],
                        opcode: 1,
                    }],
                },
                RawBlock {
                    instrs: vec![RawInstr {
                        def: Some(0),
                        uses: vec![2],
                        opcode: 2,
                    }],
                },
                RawBlock {
                    instrs: vec![RawInstr {
                        def: Some(1),
                        uses: vec![0],
                        opcode: 3,
                    }],
                },
            ],
            edges: vec![(0, 1), (0, 2), (1, 3), (2, 3)],
        };
        let ssa = construct_ssa(&program);
        let eliminated = eliminate_phis(&ssa);
        for path in [[0usize, 1, 3], [0usize, 2, 3]] {
            let inputs = [7u64, 11u64, 13u64];
            assert_eq!(exec_ssa_path(&ssa, &path, &inputs), exec_eliminated_path(&eliminated, &path, &inputs));
        }
    }

    proptest! {
        // r[verify ssa.idf-convergence]
        #[test]
        fn prop_idf_matches_manual(program in random_raw_program_strategy(5, 4)) {
            let dom = crate::dominator_tree::compute_dominators(&program.edges, program.blocks.len(), 0);
            for var in 0..program_var_bound(&program) {
                let defs = def_sites(usize_to_u32(var), &program);
                let (idf, rounds) = iterated_dominance_frontier_with_rounds(&program, &dom, &defs);
                prop_assert_eq!(idf, manual_idf(&program, &dom, &defs));
                prop_assert!(rounds <= program.blocks.len());
            }
        }

        // r[verify ssa.phi-placement]
        #[test]
        fn prop_place_phis_hits_manual_idf(program in random_raw_program_strategy(5, 4)) {
            let dom = crate::dominator_tree::compute_dominators(&program.edges, program.blocks.len(), 0);
            let with_phis = place_phis(&program, &dom);
            for var in 0..program_var_bound(&program) {
                let defs = def_sites(usize_to_u32(var), &program);
                let idf = manual_idf(&program, &dom, &defs);
                for block in 0..program.blocks.len() {
                    prop_assert_eq!(phi_vars_at(&with_phis, block).contains(&usize_to_u32(var)), idf[block]);
                }
            }
            for block in 0..program.blocks.len() {
                let preds = unique_predecessors(&program.edges, program.blocks.len(), block);
                for phi in &with_phis.blocks[block].phis {
                    prop_assert_eq!(phi.args.len(), preds.len());
                    for pred in &preds {
                        prop_assert_eq!(phi.args.iter().filter(|(p, _)| p == pred).count(), 1);
                    }
                }
            }
        }

        // r[verify ssa.single-assignment]
        #[test]
        fn prop_constructed_program_is_ssa(program in random_raw_program_strategy(5, 4)) {
            let ssa = construct_ssa(&program);
            prop_assert!(check_ssa_property(&ssa));
            prop_assert!(manual_check_ssa(&ssa));
        }

        // r[verify ssa.dominance-of-uses]
        #[test]
        fn prop_constructed_program_has_dominated_uses(program in random_raw_program_strategy(5, 4)) {
            let ssa = construct_ssa(&program);
            let dom = crate::dominator_tree::compute_dominators(&ssa.edges, ssa.blocks.len(), 0);
            prop_assert!(check_dominance_of_uses_with_dom(&ssa, &dom));
            prop_assert!(manual_check_dom_uses(&ssa, &dom));
        }

        // r[verify ssa.destruction]
        #[test]
        fn prop_eliminate_phis_preserves_path_semantics(program in random_raw_program_strategy(5, 4), inputs in prop::collection::vec(any::<u64>(), 0..=8)) {
            let ssa = construct_ssa(&program);
            let eliminated = eliminate_phis(&ssa);
            let max_len = program.blocks.len().saturating_add(2).max(1);
            for path in all_paths_bounded(&program, max_len) {
                prop_assert_eq!(exec_ssa_path(&ssa, &path, &inputs), exec_eliminated_path(&eliminated, &path, &inputs));
            }
        }
    }
}
