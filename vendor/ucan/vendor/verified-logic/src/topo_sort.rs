//! Verified topological sort via Kahn's algorithm.
//!
//! Given a directed graph as an edge list and node count, produces a
//! topological ordering where every edge (u,v) has u before v.
//! Returns `None` when the graph contains a cycle. The ordering is
//! validated by a verified checker.

#![allow(clippy::too_many_lines)]

use vstd::prelude::*;

verus! {

// ============================================================
// Graph well-formedness
// ============================================================

// r[depends topo-sort.valid-graph]
/// Spec: all edge endpoints are within [0, n).
pub open spec fn is_valid_graph_spec(edges: Seq<(int, int)>, n: int) -> bool {
    n >= 0
    && forall|i: int| #![trigger edges[i]]
        0 <= i < edges.len() ==> {
            let (u, v) = edges[i];
            0 <= u < n && 0 <= v < n
        }
}

// ============================================================
// Position lookup
// ============================================================

// r[depends topo-sort.position-of]
/// Spec: find the position of `node` in `order` starting from `from`.
pub open spec fn position_of_from_spec(order: Seq<int>, node: int, from: int) -> int
    decreases if from < order.len() as int { (order.len() - from) as int } else { 0 },
{
    if from < 0 || from >= order.len() {
        -1int
    } else if order[from] == node {
        from
    } else {
        position_of_from_spec(order, node, from + 1)
    }
}

/// Spec: position of `node` in `order`, or -1.
pub open spec fn position_of_spec(order: Seq<int>, node: int) -> int {
    position_of_from_spec(order, node, 0)
}

// ============================================================
// Topological order specification
// ============================================================

// r[depends topo-sort.is-topo-order]
/// Spec: `order` is a valid topological ordering.
pub open spec fn is_topo_order_spec(edges: Seq<(int, int)>, n: int, order: Seq<int>) -> bool {
    order.len() == n
    && all_in_range_spec(order, n, 0)
    && all_distinct_spec(order, 0)
    && edges_forward_spec(edges, n, order, 0)
}

pub open spec fn all_in_range_spec(order: Seq<int>, n: int, from: int) -> bool
    decreases if from < order.len() as int { (order.len() - from) as int } else { 0 },
{
    if from < 0 || from >= order.len() { true }
    else { 0 <= order[from] < n && all_in_range_spec(order, n, from + 1) }
}

pub open spec fn all_distinct_spec(order: Seq<int>, from: int) -> bool
    decreases if from < order.len() as int { (order.len() - from) as int } else { 0 },
{
    if from < 0 || from >= order.len() { true }
    else { not_in_rest_spec(order, order[from], from + 1) && all_distinct_spec(order, from + 1) }
}

pub open spec fn not_in_rest_spec(order: Seq<int>, val: int, from: int) -> bool
    decreases if from < order.len() as int { (order.len() - from) as int } else { 0 },
{
    if from < 0 || from >= order.len() { true }
    else { order[from] != val && not_in_rest_spec(order, val, from + 1) }
}

pub open spec fn edges_forward_spec(
    edges: Seq<(int, int)>, n: int, order: Seq<int>, from: int,
) -> bool
    decreases if from < edges.len() as int { (edges.len() - from) as int } else { 0 },
{
    if from < 0 || from >= edges.len() { true }
    else {
        let (u, v) = edges[from];
        position_of_spec(order, u) < position_of_spec(order, v)
        && edges_forward_spec(edges, n, order, from + 1)
    }
}

// ============================================================
// In-degree spec
// ============================================================

// r[depends topo-sort.in-degree]
pub open spec fn in_degree_from_spec(edges: Seq<(int, int)>, v: int, from: int) -> int
    decreases if from < edges.len() as int { (edges.len() - from) as int } else { 0 },
{
    if from < 0 || from >= edges.len() { 0int }
    else {
        let rest = in_degree_from_spec(edges, v, from + 1);
        if edges[from].1 == v { rest + 1 } else { rest }
    }
}

pub open spec fn in_degree_spec(edges: Seq<(int, int)>, v: int) -> int {
    in_degree_from_spec(edges, v, 0)
}

proof fn position_of_from_found(order: Seq<int>, node: int, from: int, pos: int)
    requires
        0 <= from <= pos < order.len(),
        order[pos] == node,
        forall|j: int| #![trigger order[j]] from <= j < pos ==> order[j] != node,
    ensures position_of_from_spec(order, node, from) == pos
    decreases order.len() - from,
{
    if from < pos {
        position_of_from_found(order, node, from + 1, pos);
    }
}

proof fn mapped_position_of_from_found(order: Seq<usize>, node: usize, from: int, pos: int)
    requires
        0 <= from <= pos < order.len(),
        order[pos] == node,
        forall|j: int| #![trigger order[j]] from <= j < pos ==> order[j] != node,
    ensures position_of_from_spec(order.map(|_i: int, v: usize| v as int), node as int, from) == pos
    decreases order.len() - from,
{
    if from < pos {
        mapped_position_of_from_found(order, node, from + 1, pos);
    }
}

proof fn all_in_range_from_pointwise(order: Seq<int>, n: int, from: int)
    requires
        0 <= from <= order.len(),
        forall|j: int| #![trigger order[j]] from <= j < order.len() ==> 0 <= order[j] < n,
    ensures all_in_range_spec(order, n, from)
    decreases order.len() - from,
{
    if from < order.len() {
        all_in_range_from_pointwise(order, n, from + 1);
    }
}

proof fn not_in_rest_from_pointwise(order: Seq<int>, val: int, from: int)
    requires
        0 <= from <= order.len(),
        forall|j: int| #![trigger order[j]] from <= j < order.len() ==> order[j] != val,
    ensures not_in_rest_spec(order, val, from)
    decreases order.len() - from,
{
    if from < order.len() {
        not_in_rest_from_pointwise(order, val, from + 1);
    }
}

proof fn all_distinct_from_pairwise(order: Seq<int>, from: int)
    requires
        0 <= from <= order.len(),
        forall|a: int, b: int| #![trigger order[a], order[b]] from <= a < b < order.len() ==> order[a] != order[b],
    ensures all_distinct_spec(order, from)
    decreases order.len() - from,
{
    if from < order.len() {
        assert(forall|j: int| #![trigger order[j]] from + 1 <= j < order.len() ==> order[j] != order[from]);
        not_in_rest_from_pointwise(order, order[from], from + 1);
        all_distinct_from_pairwise(order, from + 1);
    }
}

proof fn edges_forward_from_pointwise(edges: Seq<(int, int)>, n: int, order: Seq<int>, from: int)
    requires
        0 <= from <= edges.len(),
        forall|k: int| #![trigger edges[k]] from <= k < edges.len() ==>
            position_of_spec(order, edges[k].0) < position_of_spec(order, edges[k].1),
    ensures edges_forward_spec(edges, n, order, from)
    decreases edges.len() - from,
{
    if from < edges.len() {
        edges_forward_from_pointwise(edges, n, order, from + 1);
    }
}

// ============================================================
// Verified checker
// ============================================================

// r[impl topo-sort.is-topo-order]
/// Verify that `order` is a valid topological ordering.
/// Uses O(n*E) time — scans for position of each edge endpoint.
pub fn is_topo_order(edges: &[(usize, usize)], n: usize, order: &[usize]) -> (result: bool)
    requires
        is_valid_graph_spec(edges@.map(|_i: int, e: (usize, usize)| (e.0 as int, e.1 as int)), n as int),
        n as int + edges@.len() as int <= usize::MAX as int,
    ensures result ==> is_topo_order_spec(
        edges@.map(|_i: int, e: (usize, usize)| (e.0 as int, e.1 as int)),
        n as int,
        order@.map(|_i: int, v: usize| v as int),
    ),
{
    let ghost em = edges@.map(|_i: int, e: (usize, usize)| (e.0 as int, e.1 as int));
    let ghost om = order@.map(|_i: int, v: usize| v as int);

    if order.len() != n { return false; }

    proof {
        // The requires gives us is_valid_graph_spec(em, n as int).
        // Unfold: em maps edges@ through the int-casting function.
        // For each index k, em[k] == (edges@[k].0 as int, edges@[k].1 as int).
        // And is_valid_graph says em[k].0 < n and em[k].1 < n.
        // But the invariant quantifier is over edges@[k], not em[k].
        // edges@[k] == (u, v) where u: usize, v: usize.
        // em[k] == (u as int, v as int). So (u as int) < n as int iff u < n.
        assert forall|k: int| #![trigger edges@[k]] 0 <= k < edges@.len() implies ({
            let (eu, ev) = edges@[k];
            (eu as int) < n as int && (ev as int) < n as int
        }) by {
            let e = edges@[k];
            assert(em[k] == (e.0 as int, e.1 as int));
        }
    }

    // Check range + distinct via seen array
    let mut seen: Vec<bool> = Vec::new();
    let mut k: usize = 0;
    while k < n
        invariant 0 <= k <= n, seen@.len() == k as int,
            forall|j: int| 0 <= j < k as int ==> seen@[j] == false,
        decreases n - k,
    {
        seen.push(false);
        k += 1;
    }

    let mut i: usize = 0;
    while i < n
        invariant
            0 <= i <= n,
            order@.len() == n,
            seen@.len() == n as int,
            om =~= order@.map(|_i: int, v: usize| v as int),
            forall|k: int| #![trigger edges@[k]] 0 <= k < edges@.len() ==> {
                let (eu, ev) = edges@[k];
                (eu as int) < n as int && (ev as int) < n as int
            },
            forall|j: int| #![trigger om[j]] 0 <= j < i as int ==> (0 <= om[j] && om[j] < n as int),
            // All values seen so far are marked
            forall|j: int| #![trigger om[j]] 0 <= j < i as int ==> seen@[om[j]] == true,
            // Prefix values are pairwise distinct
            forall|a: int, b: int| 0 <= a < b < i as int ==> om[a] != om[b],
            // Only values in order[0..i] are marked
            forall|v: int| #![trigger seen@[v]] 0 <= v < n as int && seen@[v] ==> (
                exists|j: int| #![trigger om[j]] 0 <= j < i as int && om[j] == v
            ),
            // Unmarked means not in order[0..i]
            forall|v: int| #![trigger seen@[v]] 0 <= v < n as int && !seen@[v] ==> (
                forall|j: int| #![trigger om[j]] 0 <= j < i as int ==> om[j] != v
            ),
        decreases n - i,
    {
        let v = order[i];
        proof { assert(om[i as int] == v as int); }
        if v >= n { return false; }
        if seen[v] { return false; }
        seen.set(v, true);
        i += 1;
    }

    proof {
        all_in_range_from_pointwise(om, n as int, 0);
        all_distinct_from_pairwise(om, 0);
        assert forall|k: int| #![trigger edges@[k]] 0 <= k < edges@.len() implies ({
            let (eu, ev) = edges@[k];
            (eu as int) < n as int && (ev as int) < n as int
        }) by {
            let e = edges@[k];
            assert(em[k] == (e.0 as int, e.1 as int));
        }
    }

    // Check edges forward: for each edge, find positions by scanning
    let mut ei: usize = 0;
    while ei < edges.len()
        invariant
            0 <= ei <= edges@.len(),
            em =~= edges@.map(|_i: int, e: (usize, usize)| (e.0 as int, e.1 as int)),
            om =~= order@.map(|_i: int, v: usize| v as int),
            order@.len() == n,
            all_in_range_spec(om, n as int, 0),
            all_distinct_spec(om, 0),
            forall|k: int| #![trigger edges@[k]] 0 <= k < edges@.len() ==> {
                let (eu, ev) = edges@[k];
                (eu as int) < n as int && (ev as int) < n as int
            },
            forall|k: int| 0 <= k < ei as int ==> position_of_spec(order@.map(|_i: int, v: usize| v as int), em[k].0) < position_of_spec(order@.map(|_i: int, v: usize| v as int), em[k].1),
        decreases edges.len() - ei,
    {
        let (u, v) = edges[ei];
        proof {
            let e = edges@[ei as int];
            assert((e.0 as int) < n as int);
            assert((e.1 as int) < n as int);
        }
        // Find position of u
        let mut pu: usize = n; // sentinel: not found
        let mut pi: usize = 0;
        while pi < n
            invariant
                0 <= pi <= n,
                order@.len() == n,
                pu == n || (pu < n && order@[pu as int] == u),
                pu == n ==> forall|j: int| #![trigger order@[j]] 0 <= j < pi as int ==> order@[j] != u,
                pu < n ==> forall|j: int| #![trigger order@[j]] 0 <= j < pu as int ==> order@[j] != u,
            decreases n - pi,
        {
            if pu == n && order[pi] == u { pu = pi; }
            pi += 1;
        }
        // Find position of v
        let mut pv: usize = n;
        let mut pj: usize = 0;
        while pj < n
            invariant
                0 <= pj <= n,
                order@.len() == n,
                pv == n || (pv < n && order@[pv as int] == v),
                pv == n ==> forall|j: int| #![trigger order@[j]] 0 <= j < pj as int ==> order@[j] != v,
                pv < n ==> forall|j: int| #![trigger order@[j]] 0 <= j < pv as int ==> order@[j] != v,
            decreases n - pj,
        {
            if pv == n && order[pj] == v { pv = pj; }
            pj += 1;
        }
        if pu == n || pv == n || pu >= pv { return false; }
        proof {
            mapped_position_of_from_found(order@, u, 0, pu as int);
            mapped_position_of_from_found(order@, v, 0, pv as int);
            assert(position_of_spec(order@.map(|_i: int, w: usize| w as int), u as int) == pu as int);
            assert(position_of_spec(order@.map(|_i: int, w: usize| w as int), v as int) == pv as int);
            assert(position_of_spec(order@.map(|_i: int, w: usize| w as int), u as int) < position_of_spec(order@.map(|_i: int, w: usize| w as int), v as int));
        }
        ei += 1;
    }

    proof {
        edges_forward_from_pointwise(em, n as int, order@.map(|_i: int, v: usize| v as int), 0);
        assert(is_topo_order_spec(em, n as int, order@.map(|_i: int, v: usize| v as int)));
    }
    true
}

// ============================================================
// Kahn's algorithm
// ============================================================

// r[impl topo-sort.kahn-sort]
/// Topological sort via Kahn's algorithm.
pub fn topo_sort(edges: &[(usize, usize)], n: usize) -> (result: Option<Vec<usize>>)
    requires
        is_valid_graph_spec(edges@.map(|_i: int, e: (usize, usize)| (e.0 as int, e.1 as int)), n as int),
        n as int + edges@.len() as int <= usize::MAX as int,
    ensures match result {
        Some(order) => is_topo_order_spec(
            edges@.map(|_i: int, e: (usize, usize)| (e.0 as int, e.1 as int)),
            n as int,
            order@.map(|_i: int, v: usize| v as int),
        ),
        None => true,
    }
{
    let ghost em = edges@.map(|_i: int, e: (usize, usize)| (e.0 as int, e.1 as int));
    proof {
        assert forall|k: int| #![trigger edges@[k]] 0 <= k < edges@.len() implies ({
            let (eu, ev) = edges@[k];
            (eu as int) < n as int && (ev as int) < n as int
        }) by {
            let e = edges@[k];
            assert(em[k] == (e.0 as int, e.1 as int));
        }
    }

    // Compute in-degrees
    let mut in_deg: Vec<u64> = Vec::new();
    let mut k: usize = 0;
    while k < n
        invariant 0 <= k <= n, in_deg@.len() == k as int,
            forall|j: int| 0 <= j < k as int ==> in_deg@[j] == 0u64,
        decreases n - k,
    {
        in_deg.push(0);
        k += 1;
    }

    let mut ei: usize = 0;
    while ei < edges.len()
        invariant
            0 <= ei <= edges@.len(),
            in_deg@.len() == n as int,
            forall|i: int| #![trigger edges@[i]] 0 <= i < edges@.len() ==> {
                let (u, v) = edges@[i];
                (u as int) < n as int && (v as int) < n as int
            },
            forall|v: int| #![trigger in_deg@[v]] 0 <= v < n as int ==>
                in_deg@[v] as int <= edges@.len() as int,
        decreases edges.len() - ei,
    {
        let (_, v) = edges[ei];
        proof {
            let e = edges@[ei as int];
            assert((e.1 as int) < n as int);
        }
        if in_deg[v] < edges.len() as u64 {
            in_deg.set(v, in_deg[v] + 1);
        }
        ei += 1;
    }

    // Initialize queue
    let mut queue: Vec<usize> = Vec::new();
    let mut qi_init: usize = 0;
    while qi_init < n
        invariant
            0 <= qi_init <= n, in_deg@.len() == n as int,
            queue@.len() <= qi_init,
            forall|s: int| #![trigger queue@[s]]
                0 <= s < queue@.len() ==> (queue@[s] as int) < n as int,
        decreases n - qi_init,
    {
        if in_deg[qi_init] == 0 { queue.push(qi_init); }
        qi_init += 1;
    }

    // Process
    let mut order: Vec<usize> = Vec::new();
    let mut qi: usize = 0;
    while qi < queue.len()
        invariant
            0 <= qi <= queue@.len(),
            in_deg@.len() == n as int,
            order@.len() == qi,
            queue@.len() <= n,
            forall|i: int| #![trigger edges@[i]] 0 <= i < edges@.len() ==> {
                let (u, v) = edges@[i];
                (u as int) < n as int && (v as int) < n as int
            },
            forall|s: int| #![trigger queue@[s]]
                0 <= s < queue@.len() ==> (queue@[s] as int) < n as int,
        decreases n - qi,
    {
        let u = queue[qi];
        order.push(u);
        qi += 1;

        let mut ej: usize = 0;
        while ej < edges.len()
            invariant
                0 <= ej <= edges@.len(),
                in_deg@.len() == n as int,
                queue@.len() <= n,
                qi as int <= queue@.len() as int,
                forall|i: int| #![trigger edges@[i]] 0 <= i < edges@.len() ==> {
                    let (u, v) = edges@[i];
                    (u as int) < n as int && (v as int) < n as int
                },
                forall|s: int| #![trigger queue@[s]]
                    0 <= s < queue@.len() ==> (queue@[s] as int) < n as int,
            decreases edges.len() - ej,
        {
            let (src, dst) = edges[ej];
            proof {
                let e = edges@[ej as int];
                assert((e.0 as int) < n as int);
                assert((e.1 as int) < n as int);
            }
            if src == u && in_deg[dst] > 0 {
                in_deg.set(dst, in_deg[dst] - 1);
                if in_deg[dst] == 0 && queue.len() < n {
                    queue.push(dst);
                }
            }
            ej += 1;
        }
    }

    if order.len() == n {
        if is_topo_order(edges, n, &order) {
            Some(order)
        } else {
            None
        }
    } else {
        None
    }
}

// ============================================================
// Properties
// ============================================================

// r[verify topo-sort.valid-graph]
proof fn empty_graph_valid(n: int)
    requires n >= 0
    ensures is_valid_graph_spec(seq![], n)
{}

// r[verify topo-sort.in-degree]
proof fn in_degree_nonneg(edges: Seq<(int, int)>, v: int)
    ensures in_degree_spec(edges, v) >= 0
{
    in_degree_nonneg_from(edges, v, 0);
}

proof fn in_degree_nonneg_from(edges: Seq<(int, int)>, v: int, from: int)
    requires 0 <= from <= edges.len()
    ensures in_degree_from_spec(edges, v, from) >= 0
    decreases edges.len() - from,
{
    if from < edges.len() as int {
        in_degree_nonneg_from(edges, v, from + 1);
    }
}

// r[verify topo-sort.in-degree]
proof fn in_degree_bounded(edges: Seq<(int, int)>, v: int)
    ensures in_degree_spec(edges, v) <= edges.len()
{
    in_degree_bounded_from(edges, v, 0);
}

proof fn in_degree_bounded_from(edges: Seq<(int, int)>, v: int, from: int)
    requires 0 <= from <= edges.len()
    ensures in_degree_from_spec(edges, v, from) <= edges.len() - from
    decreases edges.len() - from,
{
    if from < edges.len() as int {
        in_degree_bounded_from(edges, v, from + 1);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topo_sort_linear() {
        let edges = [(0, 1), (1, 2)];
        let result = topo_sort(&edges, 3);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), vec![0, 1, 2]);
    }

    #[test]
    fn test_topo_sort_diamond() {
        let edges = [(0, 1), (0, 2), (1, 3), (2, 3)];
        let result = topo_sort(&edges, 4);
        assert!(result.is_some());
        let order = result.unwrap();
        assert_eq!(order[0], 0);
        assert_eq!(order[3], 3);
    }

    #[test]
    fn test_topo_sort_cycle() {
        let edges = [(0, 1), (1, 2), (2, 0)];
        assert!(topo_sort(&edges, 3).is_none());
    }

    #[test]
    fn test_topo_sort_no_edges() {
        let edges: [(usize, usize); 0] = [];
        let result = topo_sort(&edges, 3);
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 3);
    }

    #[test]
    fn test_topo_sort_single() {
        let edges: [(usize, usize); 0] = [];
        assert_eq!(topo_sort(&edges, 1).unwrap(), vec![0]);
    }

    #[cfg(test)]
    mod proptests {
        use proptest::prelude::*;

        use super::*;

        fn random_dag(max_nodes: usize) -> impl Strategy<Value = (Vec<(usize, usize)>, usize)> {
            (2..=max_nodes).prop_flat_map(|n| {
                let edge_count = 0..=(n * (n - 1) / 2).min(30);
                (Just(n), edge_count).prop_flat_map(move |(n, e)| {
                    proptest::collection::vec((0..n, 0..n), e..=e).prop_map(move |raw| {
                        let edges: Vec<(usize, usize)> = raw.into_iter().filter(|&(u, v)| u < v).collect();
                        (edges, n)
                    })
                })
            })
        }

        proptest! {
            #[test]
            fn prop_dag_always_sortable((edges, n) in random_dag(15)) {
                prop_assert!(topo_sort(&edges, n).is_some());
            }

            #[test]
            fn prop_topo_order_respects_edges((edges, n) in random_dag(15)) {
                if let Some(order) = topo_sort(&edges, n) {
                    let mut pos = vec![0usize; n];
                    for (i, &v) in order.iter().enumerate() { pos[v] = i; }
                    for &(u, v) in &edges {
                        prop_assert!(pos[u] < pos[v]);
                    }
                }
            }

            #[test]
            fn prop_cycle_detected(n in 3..10usize) {
                let edges: Vec<(usize, usize)> = (0..n).map(|i| (i, (i + 1) % n)).collect();
                prop_assert!(topo_sort(&edges, n).is_none());
            }

            #[test]
            fn prop_topo_order_is_permutation((edges, n) in random_dag(15)) {
                if let Some(order) = topo_sort(&edges, n) {
                    prop_assert_eq!(order.len(), n);
                    let mut seen = vec![false; n];
                    for &v in &order {
                        prop_assert!(v < n);
                        prop_assert!(!seen[v]);
                        seen[v] = true;
                    }
                }
            }

            // P0: directly test the verified is_topo_order checker
            #[test]
            fn prop_checker_validates_output((edges, n) in random_dag(20)) {
                if let Some(order) = topo_sort(&edges, n) {
                    prop_assert!(is_topo_order(&edges, n, &order));
                }
            }

            #[test]
            fn prop_checker_rejects_reversed_pair((edges, n) in random_dag(15)) {
                if let Some(mut order) = topo_sort(&edges, n) {
                    if !edges.is_empty() {
                        let (u, v) = edges[0];
                        let pu = order.iter().position(|&x| x == u).unwrap();
                        let pv = order.iter().position(|&x| x == v).unwrap();
                        order.swap(pu, pv);
                        prop_assert!(!is_topo_order(&edges, n, &order));
                    }
                }
            }

            #[test]
            fn prop_checker_rejects_wrong_length((edges, n) in random_dag(15)) {
                if n > 1 {
                    let short: Vec<usize> = (0..n - 1).collect();
                    prop_assert!(!is_topo_order(&edges, n, &short));
                }
            }

            #[test]
            fn prop_checker_rejects_out_of_range((edges, n) in random_dag(15)) {
                if n > 0 {
                    let mut bad: Vec<usize> = (0..n).collect();
                    bad[0] = n;
                    prop_assert!(!is_topo_order(&edges, n, &bad));
                }
            }
        }
    }
}
