//! Verified register-allocation core over caller-supplied live ranges.
//!
//! This module lands the allocation core only: exact interference-graph
//! construction from live-range overlap, conflict-free register assignment,
//! and deterministic spill fallback. It does not rewrite programs with
//! spill/reload instructions or model coalescing.

#![allow(clippy::too_many_lines)]

use vstd::prelude::*;

verus! {

#[derive(Debug, PartialEq, Eq)]
pub struct InterferenceGraph {
    pub edges: Vec<Vec<bool>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Allocation {
    pub register: Option<usize>,
    pub spill_slot: Option<usize>,
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

pub open spec fn valid_live_ranges_spec(live_ranges: Seq<Vec<bool>>) -> bool {
    matrix_shape_spec(live_ranges, live_ranges.len() as int, cols_spec(live_ranges))
}

pub open spec fn graph_size_spec(graph: InterferenceGraph) -> int {
    graph.edges@.len() as int
}

pub open spec fn overlap_spec(left: Seq<bool>, right: Seq<bool>) -> bool {
    left.len() == right.len()
        && exists|point: int| 0 <= point < left.len() && left[point] && right[point]
}

pub open spec fn interference_edge_spec(live_ranges: Seq<Vec<bool>>, left: int, right: int) -> bool {
    valid_live_ranges_spec(live_ranges)
        && 0 <= left < live_ranges.len()
        && 0 <= right < live_ranges.len()
        && left != right
        && overlap_spec(live_ranges[left]@, live_ranges[right]@)
}

// r[impl regalloc.interference]
pub open spec fn valid_graph_spec(graph: InterferenceGraph) -> bool {
    &&& matrix_shape_spec(graph.edges@, graph_size_spec(graph), graph_size_spec(graph))
    &&& forall|i: int| #![trigger graph.edges[i]] 0 <= i < graph_size_spec(graph) ==> !graph.edges[i]@[i]
    &&& forall|i: int, j: int| #![trigger graph.edges[i]@[j]]
        0 <= i < graph_size_spec(graph) && 0 <= j < graph_size_spec(graph)
            ==> graph.edges[i]@[j] == graph.edges[j]@[i]
}

pub open spec fn is_register_allocation_spec(allocation: Allocation) -> bool {
    allocation.register.is_some() && allocation.spill_slot.is_none()
}

pub open spec fn is_spill_allocation_spec(allocation: Allocation) -> bool {
    allocation.register.is_none() && allocation.spill_slot.is_some()
}

pub open spec fn register_value_spec(allocation: Allocation) -> int
    recommends is_register_allocation_spec(allocation)
{
    allocation.register.unwrap() as int
}

// r[impl regalloc.completeness]
pub open spec fn allocation_well_formed_spec(allocation: Allocation, register_count: int) -> bool {
    &&& 0 <= register_count
    &&& (
        (is_register_allocation_spec(allocation) && register_value_spec(allocation) < register_count)
        || is_spill_allocation_spec(allocation)
    )
}

pub open spec fn complete_prefix_spec(assignments: Seq<Allocation>, prefix: int, register_count: int) -> bool {
    assignments.len() == prefix
        && forall|i: int| #![trigger assignments[i]]
            0 <= i < prefix ==> allocation_well_formed_spec(assignments[i], register_count)
}

pub open spec fn register_available_prefix_spec(
    graph: InterferenceGraph,
    assignments: Seq<Allocation>,
    node: int,
    register: int,
    prefix: int,
) -> bool {
    &&& valid_graph_spec(graph)
    &&& 0 <= node < graph_size_spec(graph)
    &&& assignments.len() == prefix
    &&& 0 <= prefix <= graph_size_spec(graph)
    &&& 0 <= register
    &&& forall|prev: int| #![trigger graph.edges[node]@[prev]]
        0 <= prev < prefix
            && graph.edges[node]@[prev]
            && is_register_allocation_spec(assignments[prev])
            ==> register_value_spec(assignments[prev]) != register
}

pub open spec fn valid_prefix_coloring_spec(
    graph: InterferenceGraph,
    assignments: Seq<Allocation>,
    register_count: int,
    prefix: int,
) -> bool {
    &&& 0 <= prefix <= graph_size_spec(graph)
    &&& valid_graph_spec(graph)
    &&& complete_prefix_spec(assignments, prefix, register_count)
    &&& forall|left: int, right: int| #![trigger graph.edges[left]@[right]]
        0 <= left < prefix
            && 0 <= right < prefix
            && graph.edges[left]@[right]
            && is_register_allocation_spec(assignments[left])
            && is_register_allocation_spec(assignments[right])
            ==> register_value_spec(assignments[left]) != register_value_spec(assignments[right])
}

// r[impl regalloc.coloring]
pub open spec fn valid_coloring_spec(
    graph: InterferenceGraph,
    assignments: Seq<Allocation>,
    register_count: int,
) -> bool {
    valid_prefix_coloring_spec(graph, assignments, register_count, graph_size_spec(graph))
}

// ============================================================
// Proof helpers
// ============================================================

proof fn extend_prefix_with_register(graph: InterferenceGraph, assignments: Seq<Allocation>, register_count: int, node: usize, register: usize)
    requires
        0 <= register_count,
        valid_prefix_coloring_spec(graph, assignments, register_count, node as int),
        node < graph.edges@.len(),
        register < register_count,
        register_available_prefix_spec(graph, assignments, node as int, register as int, node as int),
    ensures
        valid_prefix_coloring_spec(
            graph,
            assignments.push(Allocation { register: Some(register), spill_slot: None }),
            register_count,
            node as int + 1,
        )
{
    let added = Allocation { register: Some(register), spill_slot: None };
    let next = assignments.push(added);
    assert(allocation_well_formed_spec(added, register_count));
    assert(complete_prefix_spec(next, node as int + 1, register_count));
    assert(valid_prefix_coloring_spec(graph, next, register_count, node as int + 1));
}

proof fn extend_prefix_with_spill(graph: InterferenceGraph, assignments: Seq<Allocation>, register_count: int, node: usize)
    requires
        0 <= register_count,
        valid_prefix_coloring_spec(graph, assignments, register_count, node as int),
        node < graph.edges@.len(),
    ensures
        valid_prefix_coloring_spec(
            graph,
            assignments.push(Allocation { register: None, spill_slot: Some(node) }),
            register_count,
            node as int + 1,
        )
{
    let added = Allocation { register: None, spill_slot: Some(node) };
    let next = assignments.push(added);
    assert(allocation_well_formed_spec(added, register_count));
    assert(complete_prefix_spec(next, node as int + 1, register_count));
    assert(valid_prefix_coloring_spec(graph, next, register_count, node as int + 1));
}

// ============================================================
// Exec helpers
// ============================================================

fn zero_bool_row(cols: usize) -> (result: Vec<bool>)
    ensures
        result@.len() == cols as int,
        forall|i: int| 0 <= i < cols as int ==> !result@[i],
{
    let mut row: Vec<bool> = Vec::new();
    let mut index: usize = 0;
    while index < cols
        invariant
            0 <= index <= cols,
            row@.len() == index as int,
            forall|i: int| 0 <= i < index as int ==> !row@[i],
        decreases cols - index,
    {
        row.push(false);
        index += 1;
    }
    row
}

fn ranges_overlap(left: &[bool], right: &[bool]) -> (result: bool)
    requires left@.len() == right@.len()
    ensures result == overlap_spec(left@, right@)
{
    let mut point: usize = 0;
    while point < left.len()
        invariant
            0 <= point <= left@.len(),
            left@.len() == right@.len(),
            forall|i: int| 0 <= i < point as int ==> !(left@[i] && right@[i]),
        decreases left.len() - point,
    {
        if left[point] && right[point] {
            return true;
        }
        point += 1;
    }
    false
}

fn allocation_well_formed(allocation: &Allocation, register_count: usize) -> (result: bool)
    ensures result == allocation_well_formed_spec(*allocation, register_count as int)
{
    match (allocation.register, allocation.spill_slot) {
        (Some(register), None) => register < register_count,
        (None, Some(_slot)) => true,
        (None, None) | (Some(_), Some(_)) => false,
    }
}

fn register_used_by_neighbor(graph: &InterferenceGraph, assignments: &[Allocation], node: usize, register: usize) -> (result: bool)
    requires
        valid_graph_spec(*graph),
        assignments@.len() <= node as int,
        node < graph.edges@.len(),
    ensures result == !register_available_prefix_spec(*graph, assignments@, node as int, register as int, assignments@.len() as int)
{
    let mut prev: usize = 0;
    while prev < assignments.len()
        invariant
            0 <= prev <= assignments@.len(),
            valid_graph_spec(*graph),
            assignments@.len() <= node as int,
            node < graph.edges@.len(),
            forall|i: int| #![trigger graph.edges@[node as int]@[i]]
                0 <= i < prev as int && graph.edges@[node as int]@[i] && is_register_allocation_spec(assignments@[i])
                    ==> register_value_spec(assignments@[i]) != register as int,
        decreases assignments.len() - prev,
    {
        if graph.edges[node][prev] {
            if assignments[prev].register == Some(register) && assignments[prev].spill_slot.is_none() {
                proof {
                    assert(is_register_allocation_spec(assignments@[prev as int]));
                    assert(register_value_spec(assignments@[prev as int]) == register as int);
                }
                return true;
            }
        }
        prev += 1;
    }
    false
}

fn choose_register(graph: &InterferenceGraph, assignments: &[Allocation], node: usize, register_count: usize) -> (result: Option<usize>)
    requires
        valid_graph_spec(*graph),
        assignments@.len() == node as int,
        valid_prefix_coloring_spec(*graph, assignments@, register_count as int, node as int),
        node < graph.edges@.len(),
    ensures
        match result {
            Some(register) => {
                &&& register < register_count
                &&& register_available_prefix_spec(*graph, assignments@, node as int, register as int, node as int)
                &&& forall|smaller: int| 0 <= smaller < register as int ==>
                    !register_available_prefix_spec(*graph, assignments@, node as int, smaller, node as int)
            }
            None => forall|candidate: int| 0 <= candidate < register_count as int ==>
                !register_available_prefix_spec(*graph, assignments@, node as int, candidate, node as int)
        }
{
    let mut register: usize = 0;
    while register < register_count
        invariant
            0 <= register <= register_count,
            valid_graph_spec(*graph),
            assignments@.len() == node as int,
            valid_prefix_coloring_spec(*graph, assignments@, register_count as int, node as int),
            node < graph.edges@.len(),
            forall|candidate: int| 0 <= candidate < register as int ==>
                !register_available_prefix_spec(*graph, assignments@, node as int, candidate, node as int),
        decreases register_count - register,
    {
        let used = register_used_by_neighbor(graph, assignments, node, register);
        if !used {
            return Some(register);
        }
        register += 1;
    }
    None
}

fn build_interference_row(live_ranges: &[Vec<bool>], node: usize) -> (result: Vec<bool>)
    requires
        valid_live_ranges_spec(live_ranges@),
        node < live_ranges@.len(),
    ensures
        result@.len() == live_ranges@.len(),
        forall|other: int| #![trigger result@[other]]
            0 <= other < live_ranges@.len() ==> result@[other] == interference_edge_spec(live_ranges@, node as int, other),
{
    let mut row = zero_bool_row(live_ranges.len());
    let mut other: usize = 0;
    while other < live_ranges.len()
        invariant
            0 <= other <= live_ranges@.len(),
            row@.len() == live_ranges@.len(),
            valid_live_ranges_spec(live_ranges@),
            node < live_ranges@.len(),
            forall|i: int| #![trigger row@[i]]
                0 <= i < other as int ==> row@[i] == interference_edge_spec(live_ranges@, node as int, i),
            forall|i: int| #![trigger row@[i]]
                other as int <= i < live_ranges@.len() ==> !row@[i],
        decreases live_ranges.len() - other,
    {
        if other != node {
            let overlaps = ranges_overlap(&live_ranges[node], &live_ranges[other]);
            row.set(other, overlaps);
        }
        other += 1;
    }
    row
}

// ============================================================
// Public exec surface
// ============================================================

// r[impl regalloc.interference]
pub fn valid_live_ranges(live_ranges: &[Vec<bool>]) -> (result: bool)
    ensures result == valid_live_ranges_spec(live_ranges@)
{
    if live_ranges.len() == 0 {
        return true;
    }
    let cols = live_ranges[0].len();
    let mut row: usize = 0;
    while row < live_ranges.len()
        invariant
            0 <= row <= live_ranges@.len(),
            cols == live_ranges@[0]@.len(),
            forall|i: int| #![trigger live_ranges@[i]]
                0 <= i < row as int ==> live_ranges@[i]@.len() == cols as int,
        decreases live_ranges.len() - row,
    {
        if live_ranges[row].len() != cols {
            return false;
        }
        row += 1;
    }
    true
}

// r[impl regalloc.interference]
pub fn valid_graph(graph: &InterferenceGraph) -> (result: bool)
    ensures result ==> valid_graph_spec(*graph)
{
    let size = graph.edges.len();

    let mut row: usize = 0;
    while row < size
        invariant
            size == graph.edges.len(),
            size as int == graph.edges@.len(),
            0 <= row <= size,
            forall|i: int| #![trigger graph.edges@[i]] 0 <= i < row as int ==> graph.edges@[i]@.len() == size as int,
        decreases size - row,
    {
        assert(row < graph.edges.len());
        if graph.edges[row].len() != size {
            return false;
        }
        row += 1;
    }

    let mut check_row: usize = 0;
    while check_row < size
        invariant
            size == graph.edges.len(),
            size as int == graph.edges@.len(),
            0 <= check_row <= size,
            forall|i: int| #![trigger graph.edges@[i]] 0 <= i < size as int ==> graph.edges@[i]@.len() == size as int,
            forall|i: int| #![trigger graph.edges@[i]] 0 <= i < check_row as int ==> !graph.edges@[i]@[i],
            forall|i: int, j: int| #![trigger graph.edges@[i]@[j]]
                0 <= i < check_row as int && 0 <= j < size as int ==> graph.edges@[i]@[j] == graph.edges@[j]@[i],
        decreases size - check_row,
    {
        assert(check_row < graph.edges.len());
        if graph.edges[check_row][check_row] {
            return false;
        }
        let mut col: usize = 0;
        while col < size
            invariant
                size == graph.edges.len(),
                size as int == graph.edges@.len(),
                0 <= col <= size,
                0 <= check_row < size,
                forall|i: int| #![trigger graph.edges@[i]] 0 <= i < size as int ==> graph.edges@[i]@.len() == size as int,
                forall|j: int| #![trigger graph.edges@[check_row as int]@[j]]
                    0 <= j < col as int ==> graph.edges@[check_row as int]@[j] == graph.edges@[j]@[check_row as int],
            decreases size - col,
        {
            assert(check_row < graph.edges.len());
            assert(col < graph.edges.len());
            if graph.edges[check_row][col] != graph.edges[col][check_row] {
                return false;
            }
            col += 1;
        }
        check_row += 1;
    }
    true
}

// r[impl regalloc.interference]
// r[impl regalloc.interference.scenario-overlapping-live-ranges-create-an-edge]
// r[impl regalloc.interference.scenario-disjoint-live-ranges-do-not-interfere]
// r[impl regalloc.interference.scenario-variable-does-not-interfere-with-itself]
pub fn build_interference(live_ranges: &[Vec<bool>]) -> (result: Option<InterferenceGraph>)
    ensures
        match result {
            Some(graph) => {
                &&& valid_live_ranges_spec(live_ranges@)
                &&& graph.edges@.len() == live_ranges@.len()
                &&& forall|left: int, right: int| #![trigger graph.edges[left]@[right]]
                    0 <= left < live_ranges@.len() && 0 <= right < live_ranges@.len()
                        ==> graph.edges[left]@[right] == interference_edge_spec(live_ranges@, left, right)
            }
            None => !valid_live_ranges_spec(live_ranges@)
        }
{
    if !valid_live_ranges(live_ranges) {
        return None;
    }
    let mut edges: Vec<Vec<bool>> = Vec::new();
    let mut node: usize = 0;
    while node < live_ranges.len()
        invariant
            0 <= node <= live_ranges@.len(),
            valid_live_ranges_spec(live_ranges@),
            edges@.len() == node as int,
            forall|row: int, col: int| #![trigger edges@[row]@[col]]
                0 <= row < node as int && 0 <= col < live_ranges@.len()
                    ==> edges@[row]@[col] == interference_edge_spec(live_ranges@, row, col),
        decreases live_ranges.len() - node,
    {
        edges.push(build_interference_row(live_ranges, node));
        node += 1;
    }
    Some(InterferenceGraph { edges })
}

// r[impl regalloc.completeness]
// r[impl regalloc.coloring]
// r[impl regalloc.coloring.scenario-interfering-variables-use-different-registers]
// r[impl regalloc.coloring.scenario-register-ids-stay-in-range]
pub fn check_coloring(graph: &InterferenceGraph, assignments: &[Allocation], register_count: usize) -> (result: bool)
    ensures result ==> valid_coloring_spec(*graph, assignments@, register_count as int)
{
    if !valid_graph(graph) {
        return false;
    }
    if assignments.len() != graph.edges.len() {
        return false;
    }
    let mut index: usize = 0;
    while index < assignments.len()
        invariant
            0 <= index <= assignments@.len(),
            valid_graph_spec(*graph),
            assignments@.len() == graph.edges@.len(),
            forall|i: int| #![trigger assignments@[i]]
                0 <= i < index as int ==> allocation_well_formed_spec(assignments@[i], register_count as int),
        decreases assignments.len() - index,
    {
        if !allocation_well_formed(&assignments[index], register_count) {
            return false;
        }
        index += 1;
    }

    let mut row: usize = 0;
    while row < graph.edges.len()
        invariant
            0 <= row <= graph.edges@.len(),
            valid_graph_spec(*graph),
            complete_prefix_spec(assignments@, assignments@.len() as int, register_count as int),
            assignments@.len() == graph.edges@.len(),
            forall|left: int, right: int| #![trigger graph.edges[left]@[right]]
                0 <= left < row as int && 0 <= right < graph.edges@.len()
                    && graph.edges[left]@[right]
                    && is_register_allocation_spec(assignments@[left])
                    && is_register_allocation_spec(assignments@[right])
                    ==> register_value_spec(assignments@[left]) != register_value_spec(assignments@[right]),
        decreases graph.edges.len() - row,
    {
        let mut col: usize = 0;
        while col < graph.edges.len()
            invariant
                0 <= col <= graph.edges@.len(),
                0 <= row < graph.edges@.len(),
                valid_graph_spec(*graph),
                complete_prefix_spec(assignments@, assignments@.len() as int, register_count as int),
                assignments@.len() == graph.edges@.len(),
                forall|right: int| #![trigger graph.edges[row as int]@[right]]
                    0 <= right < col as int
                        && graph.edges[row as int]@[right]
                        && is_register_allocation_spec(assignments@[row as int])
                        && is_register_allocation_spec(assignments@[right])
                        ==> register_value_spec(assignments@[row as int]) != register_value_spec(assignments@[right]),
            decreases graph.edges.len() - col,
        {
            if graph.edges[row][col] {
                if assignments[row].register.is_some()
                    && assignments[col].register.is_some()
                    && assignments[row].register == assignments[col].register
                {
                    return false;
                }
            }
            col += 1;
        }
        row += 1;
    }
    true
}

// r[impl regalloc.completeness]
// r[impl regalloc.spill-fallback]
// r[impl regalloc.spill-fallback.scenario-lowest-available-register-chosen]
// r[impl regalloc.spill-fallback.scenario-spill-only-when-all-registers-conflict]
// r[impl regalloc.completeness.scenario-register-outcome]
// r[impl regalloc.completeness.scenario-spill-outcome]
pub fn allocate_greedy(graph: &InterferenceGraph, register_count: usize) -> (result: Option<Vec<Allocation>>)
    ensures
        match result {
            Some(assignments) => valid_coloring_spec(*graph, assignments@, register_count as int),
            None => true,
        }
{
    if !valid_graph(graph) {
        return None;
    }
    let mut assignments: Vec<Allocation> = Vec::new();
    let mut node: usize = 0;
    while node < graph.edges.len()
        invariant
            0 <= node <= graph.edges@.len(),
            assignments@.len() == node as int,
            valid_prefix_coloring_spec(*graph, assignments@, register_count as int, node as int),
        decreases graph.edges.len() - node,
    {
        let choice = choose_register(graph, &assignments, node, register_count);
        match choice {
            Some(register) => {
                proof {
                    extend_prefix_with_register(*graph, assignments@, register_count as int, node, register);
                }
                assignments.push(Allocation { register: Some(register), spill_slot: None });
            }
            None => {
                proof {
                    extend_prefix_with_spill(*graph, assignments@, register_count as int, node);
                }
                assignments.push(Allocation { register: None, spill_slot: Some(node) });
            }
        }
        node += 1;
    }
    Some(assignments)
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    const REGISTER_COUNT_ZERO: usize = 0usize;
    const REGISTER_COUNT_TWO: usize = 2usize;
    const REGISTER_COUNT_THREE: usize = 3usize;
    const VARIABLE_COUNT_THREE: usize = 3usize;
    const MAX_PROP_VARIABLES: usize = 6usize;
    const MAX_PROP_POINTS: usize = 8usize;

    fn triangle_graph() -> InterferenceGraph {
        InterferenceGraph {
            edges: vec![vec![false, true, true], vec![true, false, true], vec![
                true, true, false,
            ]],
        }
    }

    fn path_graph() -> InterferenceGraph {
        InterferenceGraph {
            edges: vec![
                vec![false, true, false, false],
                vec![true, false, true, false],
                vec![false, true, false, true],
                vec![false, false, true, false],
            ],
        }
    }

    fn overlap_oracle(live_ranges: &[Vec<bool>], left: usize, right: usize) -> bool {
        left != right && live_ranges[left].iter().zip(live_ranges[right].iter()).any(|(l, r)| *l && *r)
    }

    fn symmetric_graph_from_upper(variable_count: usize, upper_bits: &[bool]) -> InterferenceGraph {
        let mut edges = vec![vec![false; variable_count]; variable_count];
        let mut cursor: usize = 0;
        for left in 0usize..variable_count {
            for right in (left + 1usize)..variable_count {
                if upper_bits[cursor] {
                    edges[left][right] = true;
                    edges[right][left] = true;
                }
                cursor += 1usize;
            }
        }
        InterferenceGraph { edges }
    }

    fn rectangular_live_ranges() -> impl Strategy<Value = Vec<Vec<bool>>> {
        (1usize..=MAX_PROP_VARIABLES, 1usize..=MAX_PROP_POINTS).prop_flat_map(|(variables, points)| {
            prop::collection::vec(prop::collection::vec(any::<bool>(), points), variables)
        })
    }

    fn valid_graphs() -> impl Strategy<Value = InterferenceGraph> {
        (1usize..=MAX_PROP_VARIABLES).prop_flat_map(|variables| {
            let upper_entries = variables * (variables - 1usize) / REGISTER_COUNT_TWO;
            prop::collection::vec(any::<bool>(), upper_entries)
                .prop_map(move |bits| symmetric_graph_from_upper(variables, &bits))
        })
    }

    // r[verify regalloc.interference]
    // r[verify regalloc.interference.scenario-overlapping-live-ranges-create-an-edge]
    // r[verify regalloc.interference.scenario-disjoint-live-ranges-do-not-interfere]
    #[test]
    fn test_build_interference_matches_overlap_oracle() {
        let live_ranges = vec![vec![true, true, false, false], vec![false, true, true, false], vec![
            false, false, true, true,
        ]];
        let graph = build_interference(&live_ranges).expect("rectangular live ranges should succeed");
        assert!(valid_graph(&graph));
        assert!(graph.edges[0][1]);
        assert!(!graph.edges[0][2]);
        assert!(graph.edges[1][2]);
    }

    // r[verify regalloc.interference]
    #[test]
    fn test_ragged_live_ranges_are_rejected() {
        let live_ranges = vec![vec![true, false, true], vec![false, true]];
        assert!(build_interference(&live_ranges).is_none());
    }

    // r[verify regalloc.coloring]
    // r[verify regalloc.coloring.scenario-interfering-variables-use-different-registers]
    // r[verify regalloc.coloring.scenario-register-ids-stay-in-range]
    #[test]
    fn test_check_coloring_accepts_valid_register_assignment() {
        let graph = path_graph();
        let assignments = vec![
            Allocation {
                register: Some(0usize),
                spill_slot: None,
            },
            Allocation {
                register: Some(1usize),
                spill_slot: None,
            },
            Allocation {
                register: Some(0usize),
                spill_slot: None,
            },
            Allocation {
                register: Some(1usize),
                spill_slot: None,
            },
        ];
        assert!(check_coloring(&graph, &assignments, REGISTER_COUNT_TWO));
    }

    // r[verify regalloc.coloring]
    #[test]
    fn test_check_coloring_rejects_conflict_and_out_of_range_register() {
        let graph = triangle_graph();
        let conflict = vec![
            Allocation {
                register: Some(0usize),
                spill_slot: None,
            },
            Allocation {
                register: Some(0usize),
                spill_slot: None,
            },
            Allocation {
                register: Some(1usize),
                spill_slot: None,
            },
        ];
        assert!(!check_coloring(&graph, &conflict, REGISTER_COUNT_TWO));

        let out_of_range = vec![
            Allocation {
                register: Some(REGISTER_COUNT_TWO),
                spill_slot: None,
            },
            Allocation {
                register: Some(1usize),
                spill_slot: None,
            },
            Allocation {
                register: None,
                spill_slot: Some(2usize),
            },
        ];
        assert!(!check_coloring(&graph, &out_of_range, REGISTER_COUNT_TWO));
    }

    // r[verify regalloc.completeness]
    // r[verify regalloc.spill-fallback]
    // r[verify regalloc.spill-fallback.scenario-lowest-available-register-chosen]
    #[test]
    fn test_allocate_greedy_uses_lowest_available_register() {
        let graph = path_graph();
        let assignments = allocate_greedy(&graph, REGISTER_COUNT_TWO).expect("valid graph should allocate");
        assert_eq!(assignments[0].register, Some(0usize));
        assert_eq!(assignments[1].register, Some(1usize));
        assert_eq!(assignments[2].register, Some(0usize));
        assert_eq!(assignments[3].register, Some(1usize));
        assert!(check_coloring(&graph, &assignments, REGISTER_COUNT_TWO));
    }

    // r[verify regalloc.completeness]
    // r[verify regalloc.spill-fallback]
    // r[verify regalloc.spill-fallback.scenario-spill-only-when-all-registers-conflict]
    // r[verify regalloc.completeness.scenario-spill-outcome]
    #[test]
    fn test_allocate_greedy_spills_when_all_registers_conflict() {
        let graph = triangle_graph();
        let assignments = allocate_greedy(&graph, REGISTER_COUNT_TWO).expect("valid graph should allocate");
        assert_eq!(assignments.len(), VARIABLE_COUNT_THREE);
        assert_eq!(assignments[0].register, Some(0usize));
        assert_eq!(assignments[1].register, Some(1usize));
        assert_eq!(assignments[2].register, None);
        assert_eq!(assignments[2].spill_slot, Some(2usize));
        assert!(check_coloring(&graph, &assignments, REGISTER_COUNT_TWO));
    }

    // r[verify regalloc.completeness]
    // r[verify regalloc.completeness.scenario-register-outcome]
    #[test]
    fn test_zero_registers_force_all_spills() {
        let graph = path_graph();
        let assignments = allocate_greedy(&graph, REGISTER_COUNT_ZERO).expect("valid graph should allocate");
        for (index, allocation) in assignments.iter().enumerate() {
            assert_eq!(allocation.register, None);
            assert_eq!(allocation.spill_slot, Some(index));
        }
        assert!(check_coloring(&graph, &assignments, REGISTER_COUNT_ZERO));
    }

    // r[verify regalloc.interference]
    #[test]
    fn test_invalid_graph_is_rejected() {
        let invalid_graph = InterferenceGraph {
            edges: vec![vec![false, true], vec![false, false]],
        };
        let assignments = vec![
            Allocation {
                register: Some(0usize),
                spill_slot: None,
            },
            Allocation {
                register: Some(1usize),
                spill_slot: None,
            },
        ];
        assert!(!valid_graph(&invalid_graph));
        assert!(!check_coloring(&invalid_graph, &assignments, REGISTER_COUNT_TWO));
        assert!(allocate_greedy(&invalid_graph, REGISTER_COUNT_TWO).is_none());
    }

    proptest! {
        // r[verify regalloc.interference]
        #[test]
        fn prop_interference_graph_matches_overlap_oracle(live_ranges in rectangular_live_ranges()) {
            let graph = build_interference(&live_ranges).expect("rectangular generator should stay valid");
            prop_assert!(valid_graph(&graph));
            for left in 0usize..live_ranges.len() {
                prop_assert!(!graph.edges[left][left]);
                for right in 0usize..live_ranges.len() {
                    prop_assert_eq!(graph.edges[left][right], overlap_oracle(&live_ranges, left, right));
                    prop_assert_eq!(graph.edges[left][right], graph.edges[right][left]);
                }
            }
        }

        // r[verify regalloc.coloring]
        // r[verify regalloc.completeness]
        // r[verify regalloc.spill-fallback]
        #[test]
        fn prop_greedy_allocations_pass_verified_checker(graph in valid_graphs(), register_count in 0usize..=REGISTER_COUNT_THREE) {
            let assignments = allocate_greedy(&graph, register_count).expect("generated graphs are valid");
            prop_assert!(check_coloring(&graph, &assignments, register_count));
            prop_assert_eq!(assignments.len(), graph.edges.len());
            for (index, allocation) in assignments.iter().enumerate() {
                let well_formed = allocation.register.map(|register| register < register_count).unwrap_or(false)
                    || (allocation.register.is_none() && allocation.spill_slot == Some(index));
                prop_assert!(well_formed);
            }
        }
    }
}
