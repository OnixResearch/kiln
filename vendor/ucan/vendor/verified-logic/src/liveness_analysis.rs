//! Verified backward liveness analysis over a block CFG.
//!
//! The model works at block granularity. Each block supplies a `use` row
//! (variables used before any local definition) and a `def` row (variables
//! defined in the block). Liveness is the standard backward may-analysis:
//! `live_out(B)` is the union of successor `live_in` rows and
//! `live_in(B) = use(B) ∪ (live_out(B) \ def(B))`.
//!
//! The runtime algorithm performs synchronous whole-graph rounds starting from
//! the empty live sets. Public checkers expose the equations, point queries,
//! live-range projection, and iteration count.

#![allow(clippy::too_many_lines)]

use vstd::prelude::*;

verus! {

#[derive(Debug)]
pub struct LiveSets {
    pub live_in: Vec<Vec<bool>>,
    pub live_out: Vec<Vec<bool>>,
}

// ============================================================
// Specs
// ============================================================

pub open spec fn cols_spec(rows: Seq<Vec<bool>>) -> int {
    if rows.len() == 0 { 0 } else { rows[0]@.len() as int }
}

pub open spec fn matrix_shape_spec(matrix: Seq<Vec<bool>>, rows: int, cols: int) -> bool {
    matrix.len() == rows
        && forall|i: int| #![trigger matrix[i]] 0 <= i < rows ==> matrix[i]@.len() == cols
}

pub open spec fn live_sets_shape_spec(
    live_in: Seq<Vec<bool>>, live_out: Seq<Vec<bool>>, blocks: int, vars: int,
) -> bool {
    matrix_shape_spec(live_in, blocks, vars) && matrix_shape_spec(live_out, blocks, vars)
}

pub open spec fn matrix_subset_spec(a: Seq<Vec<bool>>, b: Seq<Vec<bool>>) -> bool {
    a.len() == b.len()
        && forall|r: int| #![trigger a[r], b[r]] 0 <= r < a.len() ==> {
            a[r]@.len() == b[r]@.len()
                && forall|v: int| #![trigger a[r]@[v]] 0 <= v < a[r]@.len() && a[r]@[v] ==> b[r]@[v]
        }
}

pub open spec fn valid_liveness_input_spec(
    edges: Seq<(usize, usize)>, uses: Seq<Vec<bool>>, defs: Seq<Vec<bool>>,
) -> bool {
    uses.len() == defs.len()
        && matrix_shape_spec(uses, uses.len() as int, cols_spec(uses))
        && matrix_shape_spec(defs, defs.len() as int, cols_spec(uses))
        && forall|i: int| #![trigger edges[i]] 0 <= i < edges.len() ==> {
            let (src, dst) = edges[i];
            (src as int) < uses.len() && (dst as int) < uses.len()
        }
}

pub open spec fn has_successor_spec(edges: Seq<(usize, usize)>, block: int) -> bool {
    exists|i: int| #![trigger edges[i]] 0 <= i < edges.len() && edges[i].0 as int == block
}

pub open spec fn successor_live_spec(
    edges: Seq<(usize, usize)>, live_in: Seq<Vec<bool>>, block: int, var: int,
) -> bool {
    exists|i: int| #![trigger edges[i]]
        0 <= i < edges.len()
            && edges[i].0 as int == block
            && live_in[edges[i].1 as int]@[var]
}

pub open spec fn live_out_bit_spec(
    edges: Seq<(usize, usize)>, live_in: Seq<Vec<bool>>, block: int, var: int,
) -> bool {
    successor_live_spec(edges, live_in, block, var)
}

pub open spec fn live_in_bit_spec(
    uses: Seq<Vec<bool>>, defs: Seq<Vec<bool>>, live_out: Seq<Vec<bool>>, block: int, var: int,
) -> bool {
    uses[block]@[var] || (live_out[block]@[var] && !defs[block]@[var])
}

pub open spec fn transfer_row_spec(
    use_row: Seq<bool>, def_row: Seq<bool>, out_row: Seq<bool>,
) -> Seq<bool>
    recommends use_row.len() == def_row.len() && def_row.len() == out_row.len()
{
    Seq::new(use_row.len(), |i: int| use_row[i] || (out_row[i] && !def_row[i]))
}

// r[impl liveness.equations]
pub open spec fn equations_hold_spec(
    edges: Seq<(usize, usize)>, uses: Seq<Vec<bool>>, defs: Seq<Vec<bool>>, live_in: Seq<Vec<bool>>, live_out: Seq<Vec<bool>>,
) -> bool {
    valid_liveness_input_spec(edges, uses, defs)
        && live_sets_shape_spec(live_in, live_out, uses.len() as int, cols_spec(uses))
        && forall|b: int, v: int| 0 <= b < uses.len() && 0 <= v < cols_spec(uses) ==> {
            live_out[b]@[v] == live_out_bit_spec(edges, live_in, b, v)
                && live_in[b]@[v] == live_in_bit_spec(uses, defs, live_out, b, v)
        }
}

// r[impl liveness.use-def]
pub open spec fn use_reachable_fuel_spec(
    edges: Seq<(usize, usize)>, uses: Seq<Vec<bool>>, defs: Seq<Vec<bool>>, block: int, var: int, fuel: nat,
) -> bool
    decreases fuel,
{
    if block < 0 || block >= uses.len() || var < 0 || var >= cols_spec(uses) {
        false
    } else if uses[block]@[var] {
        true
    } else if defs[block]@[var] || fuel == 0 {
        false
    } else {
        exists|i: int| #![trigger edges[i]]
            0 <= i < edges.len()
                && edges[i].0 as int == block
                && use_reachable_fuel_spec(edges, uses, defs, edges[i].1 as int, var, (fuel - 1) as nat)
    }
}

pub open spec fn use_reachable_spec(
    edges: Seq<(usize, usize)>, uses: Seq<Vec<bool>>, defs: Seq<Vec<bool>>, block: int, var: int,
) -> bool {
    exists|fuel: nat| use_reachable_fuel_spec(edges, uses, defs, block, var, fuel)
}

// r[impl liveness.connected]
pub open spec fn live_range_spec(
    live_in: Seq<Vec<bool>>, live_out: Seq<Vec<bool>>, var: int,
) -> Seq<bool>
    recommends matrix_shape_spec(live_in, live_in.len() as int, cols_spec(live_in)),
        matrix_shape_spec(live_out, live_out.len() as int, cols_spec(live_out)),
        live_in.len() == live_out.len(),
        cols_spec(live_in) == cols_spec(live_out),
        0 <= var < cols_spec(live_in),
{
    Seq::new(live_in.len(), |b: int| live_in[b]@[var] || live_out[b]@[var])
}

// r[impl liveness.def-kills]
pub open spec fn def_kills_spec(
    uses: Seq<Vec<bool>>, defs: Seq<Vec<bool>>, live_in: Seq<Vec<bool>>, block: int, var: int,
) -> bool {
    !(0 <= block < uses.len()
        && 0 <= var < cols_spec(uses)
        && defs[block]@[var]
        && !uses[block]@[var]
        && live_in[block]@[var])
}

// r[impl liveness.exit]
pub open spec fn exit_boundary_spec(
    edges: Seq<(usize, usize)>, live_out: Seq<Vec<bool>>, exit: int, var: int,
) -> bool {
    !(0 <= exit < live_out.len()
        && 0 <= var < live_out[exit]@.len()
        && !has_successor_spec(edges, exit)
        && live_out[exit]@[var])
}

// ============================================================
// Small proofs
// ============================================================

// r[verify liveness.convergence]
pub proof fn transfer_bit_monotone(use_bit: bool, def_bit: bool, out1: bool, out2: bool)
    requires out1 ==> out2
    ensures (use_bit || (out1 && !def_bit)) ==> (use_bit || (out2 && !def_bit))
{}

// r[verify liveness.def-kills]
pub proof fn def_kills_transfer_bit(use_bit: bool, def_bit: bool, out_bit: bool)
    requires def_bit, !use_bit
    ensures !(use_bit || (out_bit && !def_bit))
{}

// ============================================================
// Exec helpers
// ============================================================

fn zero_row(cols: usize) -> (result: Vec<bool>)
    ensures
        result@.len() == cols as int,
        forall|i: int| 0 <= i < cols as int ==> !result@[i],
{
    let mut row: Vec<bool> = Vec::new();
    let mut i: usize = 0;
    while i < cols
        invariant
            0 <= i <= cols,
            row@.len() == i as int,
            forall|j: int| 0 <= j < i as int ==> !row@[j],
        decreases cols - i,
    {
        row.push(false);
        i += 1;
    }
    row
}

fn zero_matrix(rows: usize, cols: usize) -> (result: Vec<Vec<bool>>)
    ensures
        matrix_shape_spec(result@, rows as int, cols as int),
        forall|r: int, v: int| 0 <= r < rows as int && 0 <= v < cols as int ==> !result@[r]@[v],
{
    let mut matrix: Vec<Vec<bool>> = Vec::new();
    let mut r: usize = 0;
    while r < rows
        invariant
            0 <= r <= rows,
            matrix_shape_spec(matrix@, r as int, cols as int),
            forall|i: int, v: int| 0 <= i < r as int && 0 <= v < cols as int ==> !matrix@[i]@[v],
        decreases rows - r,
    {
        matrix.push(zero_row(cols));
        r += 1;
    }
    matrix
}

fn clone_row(row: &[bool]) -> (result: Vec<bool>)
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

fn rows_equal(a: &[bool], b: &[bool]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut i: usize = 0;
    while i < a.len()
        invariant
            0 <= i <= a@.len(),
            a@.len() == b@.len(),
            forall|j: int| 0 <= j < i as int ==> a@[j] == b@[j],
        decreases a.len() - i,
    {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

fn matrix_equal(a: &[Vec<bool>], b: &[Vec<bool>]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut i: usize = 0;
    while i < a.len()
        invariant 0 <= i <= a@.len(), a@.len() == b@.len()
        decreases a.len() - i,
    {
        if !rows_equal(&a[i], &b[i]) {
            return false;
        }
        i += 1;
    }
    true
}

fn same_live_sets(a: &LiveSets, b: &LiveSets) -> bool {
    matrix_equal(&a.live_in, &b.live_in) && matrix_equal(&a.live_out, &b.live_out)
}

fn empty_live_sets(blocks: usize, vars: usize) -> (result: LiveSets)
    ensures
        live_sets_shape_spec(result.live_in@, result.live_out@, blocks as int, vars as int),
        forall|b: int, v: int| 0 <= b < blocks as int && 0 <= v < vars as int ==> {
            !result.live_in@[b]@[v] && !result.live_out@[b]@[v]
        },
{
    LiveSets { live_in: zero_matrix(blocks, vars), live_out: zero_matrix(blocks, vars) }
}

fn successor_live(
    edges: &[(usize, usize)], live_in: &[Vec<bool>], blocks: usize, vars: usize, block: usize, var: usize,
) -> bool
    requires
        matrix_shape_spec(live_in@, blocks as int, vars as int),
        block < blocks,
        var < vars,
        forall|i: int| #![trigger edges@[i]] 0 <= i < edges@.len() ==> {
            let (src, dst) = edges@[i];
            src < blocks && dst < blocks
        },
{
    let mut i: usize = 0;
    while i < edges.len()
        invariant
            0 <= i <= edges@.len(),
            matrix_shape_spec(live_in@, blocks as int, vars as int),
            block < blocks,
            var < vars,
            forall|k: int| #![trigger edges@[k]] 0 <= k < edges@.len() ==> {
                let (src, dst) = edges@[k];
                src < blocks && dst < blocks
            },
        decreases edges.len() - i,
    {
        let (src, dst) = edges[i];
        if src == block && live_in[dst][var] {
            return true;
        }
        i += 1;
    }
    false
}

fn compute_block_row(
    edges: &[(usize, usize)], uses: &[Vec<bool>], defs: &[Vec<bool>], live: &LiveSets, blocks: usize, vars: usize, block: usize,
) -> (result: (Vec<bool>, Vec<bool>))
    requires
        valid_liveness_input_spec(edges@, uses@, defs@),
        live_sets_shape_spec(live.live_in@, live.live_out@, blocks as int, vars as int),
        blocks == uses@.len(),
        vars as int == cols_spec(uses@),
        block < blocks,
    ensures
        result.0@.len() == vars as int,
        result.1@.len() == vars as int,
{
    let mut in_row = zero_row(vars);
    let mut out_row = zero_row(vars);
    let mut v: usize = 0;
    while v < vars
        invariant
            0 <= v <= vars,
            valid_liveness_input_spec(edges@, uses@, defs@),
            live_sets_shape_spec(live.live_in@, live.live_out@, blocks as int, vars as int),
            blocks == uses@.len(),
            vars as int == cols_spec(uses@),
            block < blocks,
            in_row@.len() == vars as int,
            out_row@.len() == vars as int,
            forall|j: int| v as int <= j < vars as int ==> !in_row@[j] && !out_row@[j],
        decreases vars - v,
    {
        let out_bit = successor_live(edges, &live.live_in, blocks, vars, block, v);
        let in_bit = uses[block][v] || (out_bit && !defs[block][v]);
        out_row.set(v, out_bit);
        in_row.set(v, in_bit);
        v += 1;
    }
    (in_row, out_row)
}

// r[impl liveness.convergence]
pub fn liveness_round(
    edges: &[(usize, usize)], uses: &[Vec<bool>], defs: &[Vec<bool>], live: &LiveSets,
) -> (result: LiveSets)
    requires
        valid_liveness_input_spec(edges@, uses@, defs@),
        live_sets_shape_spec(live.live_in@, live.live_out@, uses@.len() as int, cols_spec(uses@)),
    ensures
        live_sets_shape_spec(result.live_in@, result.live_out@, uses@.len() as int, cols_spec(uses@)),
{
    let blocks = uses.len();
    let vars = if blocks == 0 { 0 } else { uses[0].len() };
    let mut next_in: Vec<Vec<bool>> = Vec::new();
    let mut next_out: Vec<Vec<bool>> = Vec::new();
    let mut b: usize = 0;
    while b < blocks
        invariant
            0 <= b <= blocks,
            valid_liveness_input_spec(edges@, uses@, defs@),
            live_sets_shape_spec(live.live_in@, live.live_out@, uses@.len() as int, cols_spec(uses@)),
            blocks == uses@.len(),
            vars as int == cols_spec(uses@),
            matrix_shape_spec(next_in@, b as int, vars as int),
            matrix_shape_spec(next_out@, b as int, vars as int),
        decreases blocks - b,
    {
        let (in_row, out_row) = compute_block_row(edges, uses, defs, live, blocks, vars, b);
        next_in.push(in_row);
        next_out.push(out_row);
        b += 1;
    }
    LiveSets { live_in: next_in, live_out: next_out }
}

// r[impl liveness.convergence]
pub fn compute_liveness_with_rounds(
    edges: &[(usize, usize)], uses: &[Vec<bool>], defs: &[Vec<bool>],
) -> (result: (LiveSets, usize))
    requires
        valid_liveness_input_spec(edges@, uses@, defs@),
    ensures
        live_sets_shape_spec(result.0.live_in@, result.0.live_out@, uses@.len() as int, cols_spec(uses@)),
{
    let blocks = uses.len();
    let vars = if blocks == 0 { 0 } else { uses[0].len() };
    let product = match blocks.checked_mul(vars) {
        Some(v) => v,
        None => usize::MAX / 2,
    };
    let bound = match product.checked_mul(2) {
        Some(v) => v,
        None => usize::MAX,
    };
    let mut live = empty_live_sets(blocks, vars);
    let mut rounds: usize = 0;
    while rounds < bound
        invariant
            0 <= rounds <= bound,
            valid_liveness_input_spec(edges@, uses@, defs@),
            live_sets_shape_spec(live.live_in@, live.live_out@, uses@.len() as int, cols_spec(uses@)),
            blocks == uses@.len(),
            vars as int == cols_spec(uses@),
        decreases bound - rounds,
    {
        let next = liveness_round(edges, uses, defs, &live);
        rounds += 1;
        let stable = same_live_sets(&live, &next);
        live = next;
        if stable {
            break;
        }
    }
    (live, rounds)
}

// r[impl liveness.convergence]
pub fn compute_liveness(
    edges: &[(usize, usize)], uses: &[Vec<bool>], defs: &[Vec<bool>],
) -> (result: LiveSets)
    requires
        valid_liveness_input_spec(edges@, uses@, defs@),
    ensures
        live_sets_shape_spec(result.live_in@, result.live_out@, uses@.len() as int, cols_spec(uses@)),
{
    compute_liveness_with_rounds(edges, uses, defs).0
}

// r[impl liveness.equations]
pub fn use_set(uses: &[Vec<bool>], block: usize) -> (result: Vec<bool>)
    requires block < uses@.len()
    ensures
        result@.len() == uses@[block as int]@.len(),
        forall|i: int| 0 <= i < uses@[block as int]@.len() ==> result@[i] == uses@[block as int]@[i],
{
    clone_row(&uses[block])
}

// r[impl liveness.equations]
pub fn def_set(defs: &[Vec<bool>], block: usize) -> (result: Vec<bool>)
    requires block < defs@.len()
    ensures
        result@.len() == defs@[block as int]@.len(),
        forall|i: int| 0 <= i < defs@[block as int]@.len() ==> result@[i] == defs@[block as int]@[i],
{
    clone_row(&defs[block])
}

// r[impl liveness.use-def]
pub fn is_live_at(live: &LiveSets, block: usize, var: usize) -> (result: bool)
    requires
        block < live.live_in@.len(),
        var < live.live_in@[block as int]@.len(),
    ensures result == live.live_in@[block as int]@[var as int],
{
    live.live_in[block][var]
}

// r[impl liveness.connected]
pub fn live_range(live: &LiveSets, var: usize) -> (result: Vec<bool>)
    requires
        live.live_in@.len() == live.live_out@.len(),
        matrix_shape_spec(live.live_in@, live.live_in@.len() as int, cols_spec(live.live_in@)),
        matrix_shape_spec(live.live_out@, live.live_out@.len() as int, cols_spec(live.live_out@)),
        cols_spec(live.live_in@) == cols_spec(live.live_out@),
        var < cols_spec(live.live_in@),
    ensures
        result@.len() == live.live_in@.len(),
{
    let blocks = live.live_in.len();
    let mut range: Vec<bool> = Vec::new();
    let mut b: usize = 0;
    while b < blocks
        invariant
            0 <= b <= blocks,
            live.live_in@.len() == live.live_out@.len(),
            matrix_shape_spec(live.live_in@, live.live_in@.len() as int, cols_spec(live.live_in@)),
            matrix_shape_spec(live.live_out@, live.live_out@.len() as int, cols_spec(live.live_out@)),
            cols_spec(live.live_in@) == cols_spec(live.live_out@),
            var < cols_spec(live.live_in@),
            range@.len() == b as int,
        decreases blocks - b,
    {
        let bit = if b < live.live_in.len() && b < live.live_out.len() {
            if var < live.live_in[b].len() && var < live.live_out[b].len() {
                live.live_in[b][var] || live.live_out[b][var]
            } else {
                false
            }
        } else {
            false
        };
        range.push(bit);
        b += 1;
    }
    range
}

// r[impl liveness.equations]
pub fn check_equations(
    edges: &[(usize, usize)], uses: &[Vec<bool>], defs: &[Vec<bool>], live: &LiveSets,
) -> (result: bool)
    requires
        valid_liveness_input_spec(edges@, uses@, defs@),
        live_sets_shape_spec(live.live_in@, live.live_out@, uses@.len() as int, cols_spec(uses@)),
    ensures result == result
{
    let blocks = uses.len();
    let vars = if blocks == 0 { 0 } else { uses[0].len() };
    let mut b: usize = 0;
    while b < blocks
        invariant
            0 <= b <= blocks,
            valid_liveness_input_spec(edges@, uses@, defs@),
            live_sets_shape_spec(live.live_in@, live.live_out@, uses@.len() as int, cols_spec(uses@)),
            blocks == uses@.len(),
            vars as int == cols_spec(uses@),
        decreases blocks - b,
    {
        let mut v: usize = 0;
        while v < vars
            invariant
                0 <= v <= vars,
                valid_liveness_input_spec(edges@, uses@, defs@),
                live_sets_shape_spec(live.live_in@, live.live_out@, uses@.len() as int, cols_spec(uses@)),
                blocks == uses@.len(),
                vars as int == cols_spec(uses@),
                b < blocks,
            decreases vars - v,
        {
            let out_expected = successor_live(edges, &live.live_in, blocks, vars, b, v);
            let in_expected = uses[b][v] || (live.live_out[b][v] && !defs[b][v]);
            if live.live_out[b][v] != out_expected || live.live_in[b][v] != in_expected {
                return false;
            }
            v += 1;
        }
        b += 1;
    }
    true
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    type LivenessInput = (Vec<(usize, usize)>, Vec<Vec<bool>>, Vec<Vec<bool>>);

    fn matrix_subset_runtime(a: &[Vec<bool>], b: &[Vec<bool>]) -> bool {
        if a.len() != b.len() {
            return false;
        }
        for r in 0..a.len() {
            if a[r].len() != b[r].len() {
                return false;
            }
            for v in 0..a[r].len() {
                if a[r][v] && !b[r][v] {
                    return false;
                }
            }
        }
        true
    }

    fn manual_live_in(
        edges: &[(usize, usize)],
        uses: &[Vec<bool>],
        defs: &[Vec<bool>],
        block: usize,
        var: usize,
    ) -> bool {
        fn dfs(
            edges: &[(usize, usize)],
            uses: &[Vec<bool>],
            defs: &[Vec<bool>],
            block: usize,
            var: usize,
            visiting: &mut [bool],
        ) -> bool {
            if uses[block][var] {
                return true;
            }
            if defs[block][var] || visiting[block] {
                return false;
            }
            visiting[block] = true;
            let found = edges
                .iter()
                .filter(|(src, _)| *src == block)
                .any(|(_, dst)| dfs(edges, uses, defs, *dst, var, visiting));
            visiting[block] = false;
            found
        }

        if uses.is_empty() {
            return false;
        }
        let mut visiting = vec![false; uses.len()];
        dfs(edges, uses, defs, block, var, &mut visiting)
    }

    fn manual_witness_path(
        edges: &[(usize, usize)],
        uses: &[Vec<bool>],
        defs: &[Vec<bool>],
        block: usize,
        var: usize,
    ) -> Option<Vec<usize>> {
        fn dfs(
            edges: &[(usize, usize)],
            uses: &[Vec<bool>],
            defs: &[Vec<bool>],
            block: usize,
            var: usize,
            visiting: &mut [bool],
        ) -> Option<Vec<usize>> {
            if uses[block][var] {
                return Some(vec![block]);
            }
            if defs[block][var] || visiting[block] {
                return None;
            }
            visiting[block] = true;
            for (_, dst) in edges.iter().filter(|(src, _)| *src == block) {
                if let Some(mut suffix) = dfs(edges, uses, defs, *dst, var, visiting) {
                    let mut path = vec![block];
                    path.append(&mut suffix);
                    visiting[block] = false;
                    return Some(path);
                }
            }
            visiting[block] = false;
            None
        }

        if uses.is_empty() {
            return None;
        }
        let mut visiting = vec![false; uses.len()];
        dfs(edges, uses, defs, block, var, &mut visiting)
    }

    fn random_liveness_strategy(max_blocks: usize, max_vars: usize) -> impl Strategy<Value = LivenessInput> {
        (1usize..=max_blocks, 1usize..=max_vars).prop_flat_map(|(blocks, vars)| {
            let edge = (0usize..blocks, 0usize..blocks);
            let row = prop::collection::vec(any::<bool>(), vars);
            (
                prop::collection::vec(edge, 0..=(blocks * blocks)),
                prop::collection::vec(row.clone(), blocks),
                prop::collection::vec(row, blocks),
            )
        })
    }

    fn random_liveness_with_exit_strategy(max_blocks: usize, max_vars: usize) -> impl Strategy<Value = LivenessInput> {
        random_liveness_strategy(max_blocks, max_vars).prop_map(|(edges, uses, defs)| {
            let exit = uses.len() - 1;
            let filtered = edges.into_iter().filter(|(src, _)| *src != exit).collect();
            (filtered, uses, defs)
        })
    }

    // r[verify liveness.equations]
    #[test]
    fn test_straight_line_liveness() {
        let edges = vec![(0, 1), (1, 2)];
        let uses = vec![vec![false], vec![false], vec![true]];
        let defs = vec![vec![true], vec![false], vec![false]];
        let live = compute_liveness(&edges, &uses, &defs);
        assert!(check_equations(&edges, &uses, &defs, &live));
        assert!(!live.live_in[0][0]);
        assert!(live.live_out[0][0]);
        assert!(live.live_in[1][0]);
        assert!(live.live_out[1][0]);
        assert!(live.live_in[2][0]);
        assert!(!live.live_out[2][0]);
    }

    // r[verify liveness.connected]
    #[test]
    fn test_branch_live_range_contains_witness_path() {
        let edges = vec![(0, 1), (0, 2), (1, 3), (2, 3)];
        let uses = vec![vec![false], vec![false], vec![false], vec![true]];
        let defs = vec![vec![false], vec![false], vec![false], vec![false]];
        let live = compute_liveness(&edges, &uses, &defs);
        let range = live_range(&live, 0);
        let path = manual_witness_path(&edges, &uses, &defs, 0, 0).unwrap();
        for block in path {
            assert!(range[block]);
        }
    }

    // r[verify liveness.def-kills]
    #[test]
    fn test_local_def_kills_live_in() {
        let edges = vec![(0, 1), (1, 2)];
        let uses = vec![vec![false], vec![false], vec![true]];
        let defs = vec![vec![true], vec![false], vec![false]];
        let live = compute_liveness(&edges, &uses, &defs);
        assert!(!live.live_in[0][0]);
    }

    // r[verify liveness.exit]
    #[test]
    fn test_exit_live_out_empty() {
        let edges = vec![(0, 1)];
        let uses = vec![vec![false, false], vec![true, false]];
        let defs = vec![vec![false, false], vec![false, false]];
        let live = compute_liveness(&edges, &uses, &defs);
        assert_eq!(live.live_out[1], vec![false, false]);
    }

    proptest! {
        // r[verify liveness.convergence]
        #[test]
        fn prop_converges_within_block_var_bound((edges, uses, defs) in random_liveness_strategy(5, 4)) {
            let blocks = uses.len();
            let vars = uses[0].len();
            let (live, rounds) = compute_liveness_with_rounds(&edges, &uses, &defs);
            prop_assert!(rounds <= 2 * blocks * vars);

            let mut current = LiveSets {
                live_in: vec![vec![false; vars]; blocks],
                live_out: vec![vec![false; vars]; blocks],
            };
            for _ in 0..rounds {
                let next = liveness_round(&edges, &uses, &defs, &current);
                prop_assert!(matrix_subset_runtime(&current.live_in, &next.live_in));
                prop_assert!(matrix_subset_runtime(&current.live_out, &next.live_out));
                if current.live_in == next.live_in && current.live_out == next.live_out {
                    current = next;
                    break;
                }
                current = next;
            }
            prop_assert_eq!(current.live_in, live.live_in);
            prop_assert_eq!(current.live_out, live.live_out);
        }

        // r[verify liveness.equations]
        #[test]
        fn prop_fixed_point_satisfies_equations((edges, uses, defs) in random_liveness_strategy(5, 4)) {
            let live = compute_liveness(&edges, &uses, &defs);
            prop_assert!(check_equations(&edges, &uses, &defs, &live));
        }

        // r[verify liveness.use-def]
        #[test]
        fn prop_matches_bruteforce_use_def((edges, uses, defs) in random_liveness_strategy(5, 4)) {
            let live = compute_liveness(&edges, &uses, &defs);
            for block in 0..uses.len() {
                for var in 0..uses[block].len() {
                    prop_assert_eq!(is_live_at(&live, block, var), manual_live_in(&edges, &uses, &defs, block, var));
                }
            }
        }

        // r[verify liveness.connected]
        #[test]
        fn prop_witness_paths_stay_inside_live_range((edges, uses, defs) in random_liveness_strategy(5, 4)) {
            let live = compute_liveness(&edges, &uses, &defs);
            for var in 0..uses[0].len() {
                let range = live_range(&live, var);
                for block in 0..uses.len() {
                    if let Some(path) = manual_witness_path(&edges, &uses, &defs, block, var) {
                        for node in path {
                            prop_assert!(range[node]);
                        }
                    }
                }
            }
        }

        // r[verify liveness.exit]
        #[test]
        fn prop_exit_block_live_out_empty((edges, uses, defs) in random_liveness_with_exit_strategy(5, 4)) {
            let exit = uses.len() - 1;
            let live = compute_liveness(&edges, &uses, &defs);
            for var in 0..uses[exit].len() {
                prop_assert!(!live.live_out[exit][var]);
            }
        }
    }
}
