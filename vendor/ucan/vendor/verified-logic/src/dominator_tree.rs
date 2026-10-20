//! Verified dominator-set and immediate-dominator computation.
//!
//! The runtime algorithm uses the classic iterative dataflow equations over a
//! boolean dominance matrix. Proof obligations stay lightweight: exec code
//! proves shape and index safety, while tests compare results against a small
//! brute-force path oracle.

#![allow(clippy::too_many_lines)]

use vstd::prelude::*;

verus! {

// ============================================================
// Specs
// ============================================================

// r[impl dominator-tree.valid-graph]
// r[verify dominator-tree.valid-graph]
pub open spec fn valid_graph_spec(edges: Seq<(usize, usize)>, n: int) -> bool {
    n >= 0
        && forall|i: int| #![trigger edges[i]] 0 <= i < edges.len() ==> {
            let (src, dst) = edges[i];
            (src as int) < n && (dst as int) < n
        }
}

pub open spec fn matrix_shape_spec(dom: Seq<Seq<bool>>, n: int) -> bool {
    dom.len() == n
        && forall|i: int| #![trigger dom[i]] 0 <= i < dom.len() ==> dom[i].len() == n
}

// r[depends dominator-tree.partial-order]
pub open spec fn dominates_matrix_spec(dom: Seq<Seq<bool>>, a: int, b: int) -> bool {
    0 <= b < dom.len() && 0 <= a < dom[b].len() && dom[b][a]
}

pub open spec fn parent_shape_spec(idom: Seq<Option<usize>>, n: int, entry: int) -> bool {
    idom.len() == n
        && (0 <= entry < n ==> idom[entry] == None::<usize>)
        && forall|i: int| #![trigger idom[i]] 0 <= i < n && i != entry ==> match idom[i] {
            Some(p) => (p as int) < n,
            None => true,
        }
}

// ============================================================
// Helpers
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

pub open spec fn dom_rows_shape_spec(dom: Seq<Vec<bool>>, n: int) -> bool {
    dom.len() == n
        && forall|i: int| #![trigger dom[i]] 0 <= i < dom.len() ==> dom[i]@.len() == n
}

// r[impl dominator-tree.tree-structure]
/// Collect predecessors of `node`.
pub fn predecessors(edges: &[(usize, usize)], n: usize, node: usize) -> (result: Vec<usize>)
    requires
        node < n,
        valid_graph_spec(edges@, n as int),
    ensures
        forall|i: int| #![trigger result@[i]] 0 <= i < result@.len() ==> result@[i] < n,
{
    let mut out: Vec<usize> = Vec::new();
    let mut i: usize = 0;
    while i < edges.len()
        invariant
            0 <= i <= edges@.len(),
            node < n,
            valid_graph_spec(edges@, n as int),
            forall|j: int| #![trigger out@[j]] 0 <= j < out@.len() ==> out@[j] < n,
        decreases edges.len() - i,
    {
        let (src, dst) = edges[i];
        if dst == node {
            out.push(src);
        }
        i += 1;
    }
    out
}

fn all_preds_have(dom: &[Vec<bool>], preds: &[usize], col: usize) -> (result: bool)
    requires
        dom_rows_shape_spec(dom@, dom@.len() as int),
        forall|i: int| #![trigger preds@[i]] 0 <= i < preds@.len() ==> preds@[i] < dom@.len(),
        col < dom@.len(),
    ensures result ==> forall|i: int| #![trigger preds@[i]]
        0 <= i < preds@.len() && dom@[preds@[i] as int][preds@[i] as int] ==> dom@[preds@[i] as int][col as int],
{
    let mut i: usize = 0;
    while i < preds.len()
        invariant
            0 <= i <= preds@.len(),
            dom_rows_shape_spec(dom@, dom@.len() as int),
            forall|j: int| #![trigger preds@[j]] 0 <= j < preds@.len() ==> preds@[j] < dom@.len(),
            col < dom@.len(),
            forall|j: int| #![trigger preds@[j]] 0 <= j < i as int && dom@[preds@[j] as int][preds@[j] as int] ==> dom@[preds@[j] as int][col as int],
        decreases preds.len() - i,
    {
        if dom[preds[i]][preds[i]] && !dom[preds[i]][col] {
            return false;
        }
        i += 1;
    }
    true
}

// ============================================================
// Dominator matrix
// ============================================================

// r[impl dominator-tree.entry-dominates]
// r[impl dominator-tree.convergence]
/// Compute the dominance matrix with the standard iterative equations.
pub fn compute_dominators(edges: &[(usize, usize)], n: usize, entry: usize) -> (result: Vec<Vec<bool>>)
    requires
        entry < n,
        valid_graph_spec(edges@, n as int),
        n as int + edges@.len() as int <= usize::MAX as int,
    ensures
        dom_rows_shape_spec(result@, n as int),
        matrix_shape_spec(result@.map_values(|row: Vec<bool>| row@), n as int),
        result@[entry as int][entry as int],
{
    let reachable = crate::reachability::reachable_from(edges, n, entry);

    let mut dom: Vec<Vec<bool>> = Vec::new();
    let mut v: usize = 0;
    while v < n
        invariant
            0 <= v <= n,
            reachable@.len() == n as int,
            dom@.len() == v as int,
            forall|i: int| #![trigger dom@[i]] 0 <= i < dom@.len() ==> dom@[i].len() == n as int,
            (entry as int) < dom@.len() ==> dom@[entry as int]@[entry as int],
        decreases n - v,
    {
        let mut row = if v == entry {
            empty_bool_row(n, false)
        } else if reachable[v] {
            empty_bool_row(n, true)
        } else {
            empty_bool_row(n, false)
        };
        if reachable[v] {
            row.set(v, true);
        }
        if v == entry {
            row.set(entry, true);
        }
        dom.push(row);
        v += 1;
    }

    let mut iter: usize = 0;
    while iter < n
        invariant
            0 <= iter <= n,
            valid_graph_spec(edges@, n as int),
            entry < n,
            dom_rows_shape_spec(dom@, n as int),
            reachable@.len() == n as int,
            dom@[entry as int]@[entry as int],
        decreases n - iter,
    {
        let mut node: usize = 0;
        while node < n
            invariant
                0 <= node <= n,
                valid_graph_spec(edges@, n as int),
                entry < n,
                dom_rows_shape_spec(dom@, n as int),
                reachable@.len() == n as int,
                dom@[entry as int]@[entry as int],
            decreases n - node,
        {
            if reachable[node] && node != entry {
                let preds = predecessors(edges, n, node);
                let mut new_row = empty_bool_row(n, false);
                new_row.set(node, true);
                if preds.len() > 0 {
                    let mut col: usize = 0;
                    while col < n
                        invariant
                            0 <= col <= n,
                            node < n,
                            dom_rows_shape_spec(dom@, n as int),
                            new_row@.len() == n as int,
                            forall|i: int| #![trigger preds@[i]] 0 <= i < preds@.len() ==> preds@[i] < n,
                        decreases n - col,
                    {
                        if all_preds_have(&dom, &preds, col) {
                            new_row.set(col, true);
                        }
                        col += 1;
                    }
                }
                new_row.set(node, true);
                dom.set(node, new_row);
            }
            node += 1;
        }
        iter += 1;
    }

    dom
}

// r[impl dominator-tree.partial-order]
/// Query the computed dominance relation.
pub fn dominates(dom: &[Vec<bool>], a: usize, b: usize) -> (result: bool)
    requires
        dom_rows_shape_spec(dom@, dom@.len() as int),
        a < dom@.len(),
        b < dom@.len(),
    ensures result == dominates_matrix_spec(dom@.map_values(|row: Vec<bool>| row@), a as int, b as int)
{
    dom[b][a]
}

// ============================================================
// Immediate dominators
// ============================================================

// r[impl dominator-tree.idom-unique]
// r[impl dominator-tree.tree-structure]
/// Compute the immediate dominator parent map from a dominance matrix.
pub fn immediate_dominators(dom: &[Vec<bool>], entry: usize) -> (result: Vec<Option<usize>>)
    requires
        dom_rows_shape_spec(dom@, dom@.len() as int),
        entry < dom@.len(),
    ensures parent_shape_spec(result@, dom@.len() as int, entry as int)
{
    let n = dom.len();
    let mut out: Vec<Option<usize>> = Vec::new();
    let mut i: usize = 0;
    while i < n
        invariant
            0 <= i <= n,
            dom_rows_shape_spec(dom@, n as int),
            out@.len() == i as int,
            forall|j: int| #![trigger out@[j]] 0 <= j < out@.len() && j == entry as int ==> out@[j] == None::<usize>,
            forall|j: int| #![trigger out@[j]] 0 <= j < out@.len() && j != entry as int ==> match out@[j] {
                Some(p) => (p as int) < n as int,
                None => true,
            },
        decreases n - i,
    {
        if i == entry || !dom[i][i] {
            out.push(None);
        } else {
            let mut best: Option<usize> = None;
            let mut d: usize = 0;
            while d < n
                invariant
                    0 <= d <= n,
                    dom_rows_shape_spec(dom@, n as int),
                    i < n,
                    best.is_some() ==> best.unwrap() < n,
                decreases n - d,
            {
                if d != i && dom[i][d] {
                    match best {
                        None => best = Some(d),
                        Some(current) => {
                            if dom[d][current] {
                                best = Some(d);
                            }
                        }
                    }
                }
                d += 1;
            }
            out.push(best);
        }
        i += 1;
    }
    out
}

// r[impl dominator-tree.dominance-frontier]
/// Compute the dominance frontier of `node`.
pub fn dominance_frontier(
    edges: &[(usize, usize)],
    dom: &[Vec<bool>],
    node: usize,
) -> (result: Vec<bool>)
    requires
        dom_rows_shape_spec(dom@, dom@.len() as int),
        valid_graph_spec(edges@, dom@.len() as int),
        node < dom@.len(),
    ensures result@.len() == dom@.len(),
{
    let n = dom.len();
    let mut out = empty_bool_row(n, false);
    let mut y: usize = 0;
    while y < n
        invariant
            0 <= y <= n,
            valid_graph_spec(edges@, n as int),
            node < n,
            dom_rows_shape_spec(dom@, n as int),
            out@.len() == n as int,
        decreases n - y,
    {
        let preds = predecessors(edges, n, y);
        let mut any_pred = false;
        let mut i: usize = 0;
        while i < preds.len()
            invariant
                0 <= i <= preds@.len(),
                node < n,
                dom_rows_shape_spec(dom@, n as int),
                forall|j: int| #![trigger preds@[j]] 0 <= j < preds@.len() ==> preds@[j] < n,
            decreases preds.len() - i,
        {
            if dom[preds[i]][node] {
                any_pred = true;
            }
            i += 1;
        }
        if any_pred && !(node != y && dom[y][node]) {
            out.set(y, true);
        }
        y += 1;
    }
    out
}

// r[impl dominator-tree.convergence]
/// Full dominator-tree computation convenience wrapper.
pub fn compute_domtree(edges: &[(usize, usize)], n: usize, entry: usize) -> (result: Vec<Option<usize>>)
    requires
        entry < n,
        valid_graph_spec(edges@, n as int),
        n as int + edges@.len() as int <= usize::MAX as int,
    ensures parent_shape_spec(result@, n as int, entry as int)
{
    let dom = compute_dominators(edges, n, entry);
    immediate_dominators(&dom, entry)
}

} // verus!

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::collections::VecDeque;

    use proptest::prelude::*;

    use super::*;

    fn all_simple_paths(edges: &[(usize, usize)], entry: usize, target: usize, n: usize) -> Vec<Vec<usize>> {
        fn dfs(
            edges: &[(usize, usize)],
            cur: usize,
            target: usize,
            n: usize,
            seen: &mut Vec<bool>,
            path: &mut Vec<usize>,
            out: &mut Vec<Vec<usize>>,
        ) {
            if cur == target {
                out.push(path.clone());
                return;
            }
            for &(src, dst) in edges {
                if src == cur && dst < n && !seen[dst] {
                    seen[dst] = true;
                    path.push(dst);
                    dfs(edges, dst, target, n, seen, path, out);
                    path.pop();
                    seen[dst] = false;
                }
            }
        }

        let mut out = Vec::new();
        let mut seen = vec![false; n];
        seen[entry] = true;
        let mut path = vec![entry];
        dfs(edges, entry, target, n, &mut seen, &mut path, &mut out);
        out
    }

    fn manual_reachable(edges: &[(usize, usize)], n: usize, entry: usize) -> Vec<bool> {
        let mut seen = vec![false; n];
        let mut q = VecDeque::new();
        seen[entry] = true;
        q.push_back(entry);
        while let Some(u) = q.pop_front() {
            for &(src, dst) in edges {
                if src == u && !seen[dst] {
                    seen[dst] = true;
                    q.push_back(dst);
                }
            }
        }
        seen
    }

    fn manual_dominators(edges: &[(usize, usize)], n: usize, entry: usize) -> Vec<Vec<bool>> {
        let reachable = manual_reachable(edges, n, entry);
        let mut dom = vec![vec![false; n]; n];
        for b in 0..n {
            if !reachable[b] {
                continue;
            }
            let paths = all_simple_paths(edges, entry, b, n);
            for a in 0..n {
                dom[b][a] = if a == b {
                    true
                } else if paths.is_empty() {
                    false
                } else {
                    paths.iter().all(|path| path.contains(&a))
                };
            }
        }
        dom
    }

    fn manual_idom(dom: &[Vec<bool>], entry: usize) -> Vec<Option<usize>> {
        let n = dom.len();
        let mut out = vec![None; n];
        for b in 0..n {
            if b == entry || !dom[b][b] {
                continue;
            }
            let mut strict: Vec<usize> = (0..n).filter(|&a| a != b && dom[b][a]).collect();
            strict.sort_unstable();
            out[b] = strict.into_iter().find(|&d| (0..n).all(|c| c == d || c == b || !dom[b][c] || dom[d][c]));
        }
        out
    }

    fn valid_graph_strategy(max_nodes: usize) -> impl Strategy<Value = (Vec<(usize, usize)>, usize)> {
        (2..=max_nodes).prop_flat_map(|n| {
            proptest::collection::vec((0..n, 0..n), 0..=(n * 3).min(24)).prop_map(move |edges| (edges, n))
        })
    }

    // r[verify dominator-tree.entry-dominates]
    #[test]
    fn test_entry_dominates_linear_graph() {
        let edges = [(0, 1), (1, 2), (2, 3)];
        let dom = compute_dominators(&edges, 4, 0);
        for b in 0..4 {
            assert!(dominates(&dom, 0, b));
        }
    }

    // r[verify dominator-tree.partial-order]
    #[test]
    fn test_partial_order_diamond() {
        let edges = [(0, 1), (0, 2), (1, 3), (2, 3)];
        let dom = compute_dominators(&edges, 4, 0);
        for n in 0..4 {
            assert!(dominates(&dom, n, n));
        }
        assert!(dominates(&dom, 0, 3));
        assert!(!dominates(&dom, 1, 3));
        assert!(!dominates(&dom, 2, 3));
    }

    // r[verify dominator-tree.idom-unique]
    // r[verify dominator-tree.tree-structure]
    #[test]
    fn test_immediate_dominators_linear() {
        let edges = [(0, 1), (1, 2), (2, 3)];
        let idom = compute_domtree(&edges, 4, 0);
        assert_eq!(idom, vec![None, Some(0), Some(1), Some(2)]);
    }

    // r[verify dominator-tree.dominance-frontier]
    #[test]
    fn test_dominance_frontier_diamond() {
        let edges = [(0, 1), (0, 2), (1, 3), (2, 3)];
        let dom = compute_dominators(&edges, 4, 0);
        let df1 = dominance_frontier(&edges, &dom, 1);
        let df2 = dominance_frontier(&edges, &dom, 2);
        assert!(df1[3]);
        assert!(df2[3]);
    }

    // r[verify dominator-tree.convergence]
    #[test]
    fn test_compute_domtree_matches_manual_linear() {
        let edges = [(0, 1), (1, 2), (2, 3)];
        let dom = compute_dominators(&edges, 4, 0);
        let manual = manual_dominators(&edges, 4, 0);
        assert_eq!(dom, manual);
        assert_eq!(compute_domtree(&edges, 4, 0), manual_idom(&manual, 0));
    }

    proptest! {
        // r[verify dominator-tree.entry-dominates]
        #[test]
        fn prop_entry_dominates_reachable((edges, n) in valid_graph_strategy(8)) {
            let dom = compute_dominators(&edges, n, 0);
            let reachable = manual_reachable(&edges, n, 0);
            for b in 0..n {
                if reachable[b] {
                    prop_assert!(dom[b][0]);
                }
            }
        }

        // r[verify dominator-tree.partial-order]
        #[test]
        fn prop_matches_manual_dominators((edges, n) in valid_graph_strategy(7)) {
            let dom = compute_dominators(&edges, n, 0);
            let manual = manual_dominators(&edges, n, 0);
            prop_assert_eq!(dom, manual);
        }

        // r[verify dominator-tree.idom-unique]
        // r[verify dominator-tree.tree-structure]
        #[test]
        fn prop_matches_manual_idom((edges, n) in valid_graph_strategy(7)) {
            let dom = compute_dominators(&edges, n, 0);
            let idom = compute_domtree(&edges, n, 0);
            let manual = manual_idom(&manual_dominators(&edges, n, 0), 0);
            prop_assert_eq!(idom, manual);
        }

        // r[verify dominator-tree.dominance-frontier]
        #[test]
        fn prop_frontier_nodes_have_pred_dominated((edges, n) in valid_graph_strategy(7)) {
            let dom = compute_dominators(&edges, n, 0);
            for a in 0..n.min(4) {
                let frontier = dominance_frontier(&edges, &dom, a);
                for y in 0..n {
                    if frontier[y] {
                        let preds: HashSet<_> = edges.iter().filter_map(|&(src, dst)| (dst == y).then_some(src)).collect();
                        prop_assert!(preds.iter().any(|&p| dom[p][a]));
                        prop_assert!(!(a != y && dom[y][a]));
                    }
                }
            }
        }
    }
}
