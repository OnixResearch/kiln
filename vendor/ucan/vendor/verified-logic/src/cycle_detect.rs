//! Verified cycle detection for directed graphs.
//!
//! Detects cycles via Kahn's algorithm: repeatedly remove zero-in-degree
//! nodes. If all nodes are removed, the graph is acyclic. If some remain,
//! those nodes form cycles. Specs define cycles via fuel-bounded path
//! existence.

#![allow(clippy::too_many_lines)]

use vstd::prelude::*;

verus! {

// ============================================================
// Path and cycle specs
// ============================================================

// r[depends cycle-detect.has-edge]
/// Spec: edge (u,v) exists in the edge list.
pub open spec fn has_edge_spec(edges: Seq<(int, int)>, u: int, v: int) -> bool {
    exists|i: int| #![trigger edges[i]] 0 <= i < edges.len() && edges[i] == (u, v)
}

// r[depends cycle-detect.has-path]
/// Spec: path exists from `src` to `dst` using at most `fuel` edges.
pub open spec fn has_path_fuel(edges: Seq<(int, int)>, src: int, dst: int, fuel: nat) -> bool
    decreases fuel,
{
    if src == dst {
        true
    } else if fuel == 0 {
        false
    } else {
        exists|mid: int| has_edge_spec(edges, src, mid)
            && has_path_fuel(edges, mid, dst, (fuel - 1) as nat)
    }
}

/// Spec: path exists with sufficient fuel.
pub open spec fn has_path_spec(edges: Seq<(int, int)>, n: int, src: int, dst: int) -> bool {
    has_path_fuel(edges, src, dst, n as nat)
}

// r[depends cycle-detect.has-cycle]
/// Spec: the graph has a cycle (some node can reach itself via >= 1 edge).
pub open spec fn has_cycle_spec(edges: Seq<(int, int)>, n: int) -> bool {
    exists|v: int| #![trigger has_nontrivial_cycle_at(edges, n, v)]
        0 <= v < n && has_nontrivial_cycle_at(edges, n, v)
}

/// Spec: node `v` is on a cycle (reachable from itself via >= 1 edge).
pub open spec fn has_nontrivial_cycle_at(edges: Seq<(int, int)>, n: int, v: int) -> bool {
    exists|mid: int| has_edge_spec(edges, v, mid)
        && has_path_fuel(edges, mid, v, n as nat)
}

// ============================================================
// Cycle detection (Kahn's method)
// ============================================================

// r[impl cycle-detect.has-cycle]
/// Detect whether a directed graph has a cycle.
///
/// Reuses the verified Kahn topological sort: an acyclic graph returns a
/// topological order, while a cycle leaves no valid full ordering.
pub fn has_cycle(edges: &[(usize, usize)], n: usize) -> (result: bool)
    requires
        n as int + edges@.len() as int <= usize::MAX as int,
        forall|i: int| #![trigger edges@[i]] 0 <= i < edges@.len() ==> {
            let (u, v) = edges@[i];
            (u as int) < n as int && (v as int) < n as int
        },
    ensures
        !result ==> !has_cycle_spec(
            edges@.map(|_i: int, e: (usize, usize)| (e.0 as int, e.1 as int)),
            n as int,
        ),
{
    let ghost em = edges@.map(|_i: int, e: (usize, usize)| (e.0 as int, e.1 as int));
    let order = crate::topo_sort::topo_sort(edges, n);
    match order {
        Some(order) => {
            proof {
                let om = order@.map(|_i: int, v: usize| v as int);
                assert(crate::topo_sort::is_topo_order_spec(em, n as int, om));
                topo_order_implies_acyclic(em, n as int, om);
            }
            false
        }
        None => true,
    }
}

// ============================================================
// Properties
// ============================================================

// r[verify cycle-detect.has-path]
/// Every node has a trivial path to itself.
proof fn path_reflexive(edges: Seq<(int, int)>, src: int)
    ensures has_path_fuel(edges, src, src, 0)
{}

// r[verify cycle-detect.has-path]
/// A single edge gives a path of length 1.
proof fn edge_gives_path(edges: Seq<(int, int)>, u: int, v: int)
    requires has_edge_spec(edges, u, v)
    ensures has_path_fuel(edges, u, v, 1)
{
    assert(has_path_fuel(edges, v, v, 0));
}

// r[verify cycle-detect.has-path]
/// More fuel doesn't lose paths.
proof fn more_fuel_keeps_path(edges: Seq<(int, int)>, src: int, dst: int, fuel: nat)
    requires has_path_fuel(edges, src, dst, fuel)
    ensures has_path_fuel(edges, src, dst, fuel + 1)
    decreases fuel,
{
    if src == dst {
    } else {
        let mid = choose|mid: int| has_edge_spec(edges, src, mid)
            && has_path_fuel(edges, mid, dst, (fuel - 1) as nat);
        more_fuel_keeps_path(edges, mid, dst, (fuel - 1) as nat);
    }
}

// r[verify cycle-detect.has-cycle]
/// Self-loop is a cycle.
proof fn self_loop_is_cycle(edges: Seq<(int, int)>, n: int, v: int)
    requires 0 <= v < n, has_edge_spec(edges, v, v)
    ensures has_cycle_spec(edges, n)
{
    assert(has_path_fuel(edges, v, v, 0));
    assert(has_nontrivial_cycle_at(edges, n, v));
}

// r[verify cycle-detect.has-path]
/// Path concatenation: if src→mid and mid→dst, then src→dst.
proof fn path_concat(edges: Seq<(int, int)>, src: int, mid: int, dst: int, f1: nat, f2: nat)
    requires
        has_path_fuel(edges, src, mid, f1),
        has_path_fuel(edges, mid, dst, f2),
    ensures has_path_fuel(edges, src, dst, (f1 + f2) as nat)
    decreases f1,
{
    if src == mid {
        fuel_monotone(edges, src, dst, f2, (f1 + f2) as nat);
    } else {
        let next = choose|next: int| has_edge_spec(edges, src, next)
            && has_path_fuel(edges, next, mid, (f1 - 1) as nat);
        path_concat(edges, next, mid, dst, (f1 - 1) as nat, f2);
    }
}

proof fn fuel_monotone(edges: Seq<(int, int)>, src: int, dst: int, f1: nat, f2: nat)
    requires f2 >= f1, has_path_fuel(edges, src, dst, f1)
    ensures has_path_fuel(edges, src, dst, f2)
    decreases f1,
{
    if src == dst {
    } else {
        let next = choose|next: int| has_edge_spec(edges, src, next)
            && has_path_fuel(edges, next, dst, (f1 - 1) as nat);
        fuel_monotone(edges, next, dst, (f1 - 1) as nat, (f2 - 1) as nat);
    }
}

proof fn topo_edge_forward_at_index(edges: Seq<(int, int)>, n: int, order: Seq<int>, from: int, target: int)
    requires
        0 <= from <= target < edges.len(),
        crate::topo_sort::edges_forward_spec(edges, n, order, from),
    ensures crate::topo_sort::position_of_spec(order, edges[target].0)
        < crate::topo_sort::position_of_spec(order, edges[target].1)
    decreases target - from,
{
    if from < target {
        topo_edge_forward_at_index(edges, n, order, from + 1, target);
    }
}

proof fn topo_edge_forward_member(edges: Seq<(int, int)>, n: int, order: Seq<int>, src: int, dst: int)
    requires
        crate::topo_sort::is_topo_order_spec(edges, n, order),
        has_edge_spec(edges, src, dst),
    ensures crate::topo_sort::position_of_spec(order, src)
        < crate::topo_sort::position_of_spec(order, dst)
{
    let i = choose|i: int| 0 <= i < edges.len() && edges[i] == (src, dst);
    topo_edge_forward_at_index(edges, n, order, 0, i);
}

proof fn topo_path_strictly_forward(edges: Seq<(int, int)>, n: int, order: Seq<int>, src: int, dst: int, fuel: nat)
    requires
        crate::topo_sort::is_topo_order_spec(edges, n, order),
        has_path_fuel(edges, src, dst, fuel),
        src != dst,
    ensures crate::topo_sort::position_of_spec(order, src)
        < crate::topo_sort::position_of_spec(order, dst)
    decreases fuel,
{
    let next = choose|next: int| has_edge_spec(edges, src, next)
        && has_path_fuel(edges, next, dst, (fuel - 1) as nat);
    topo_edge_forward_member(edges, n, order, src, next);
    if next != dst {
        topo_path_strictly_forward(edges, n, order, next, dst, (fuel - 1) as nat);
    }
}

proof fn topo_order_implies_acyclic(edges: Seq<(int, int)>, n: int, order: Seq<int>)
    requires crate::topo_sort::is_topo_order_spec(edges, n, order)
    ensures !has_cycle_spec(edges, n)
{
    if has_cycle_spec(edges, n) {
        let v = choose|v: int| 0 <= v < n && has_nontrivial_cycle_at(edges, n, v);
        let mid = choose|mid: int| has_edge_spec(edges, v, mid)
            && has_path_fuel(edges, mid, v, n as nat);
        topo_edge_forward_member(edges, n, order, v, mid);
        if mid == v {
            assert(crate::topo_sort::position_of_spec(order, v)
                < crate::topo_sort::position_of_spec(order, v));
        } else {
            topo_path_strictly_forward(edges, n, order, mid, v, n as nat);
            assert(crate::topo_sort::position_of_spec(order, v)
                < crate::topo_sort::position_of_spec(order, mid));
            assert(crate::topo_sort::position_of_spec(order, mid)
                < crate::topo_sort::position_of_spec(order, v));
        }
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_cycle_linear() {
        let edges = [(0, 1), (1, 2)];
        assert!(!has_cycle(&edges, 3));
    }

    #[test]
    fn test_cycle_triangle() {
        let edges = [(0, 1), (1, 2), (2, 0)];
        assert!(has_cycle(&edges, 3));
    }

    #[test]
    fn test_self_loop() {
        let edges = [(0, 0)];
        assert!(has_cycle(&edges, 1));
    }

    #[test]
    fn test_no_edges() {
        let edges: [(usize, usize); 0] = [];
        assert!(!has_cycle(&edges, 5));
    }

    #[test]
    fn test_diamond_no_cycle() {
        let edges = [(0, 1), (0, 2), (1, 3), (2, 3)];
        assert!(!has_cycle(&edges, 4));
    }

    #[test]
    fn test_disconnected_with_cycle() {
        let edges = [(2, 3), (3, 2)];
        assert!(has_cycle(&edges, 4));
    }

    #[cfg(test)]
    mod proptests {
        use proptest::prelude::*;

        use super::*;

        fn random_dag(max_nodes: usize) -> impl Strategy<Value = (Vec<(usize, usize)>, usize)> {
            (2..=max_nodes).prop_flat_map(|n| {
                proptest::collection::vec((0..n, 0..n), 0..=(n * 2).min(30)).prop_map(move |raw| {
                    let edges: Vec<(usize, usize)> = raw.into_iter().filter(|&(u, v)| u < v).collect();
                    (edges, n)
                })
            })
        }

        fn random_graph_with_cycle(max_nodes: usize) -> impl Strategy<Value = (Vec<(usize, usize)>, usize)> {
            (3..=max_nodes).prop_flat_map(|n| {
                proptest::collection::vec((0..n, 0..n), 0..=(n * 2).min(20)).prop_map(move |mut edges| {
                    for i in 0..n {
                        edges.push((i, (i + 1) % n));
                    }
                    (edges, n)
                })
            })
        }

        proptest! {
            #[test]
            fn prop_dag_no_cycle((edges, n) in random_dag(12)) {
                prop_assert!(!has_cycle(&edges, n));
            }

            #[test]
            fn prop_forced_cycle_detected((edges, n) in random_graph_with_cycle(10)) {
                prop_assert!(has_cycle(&edges, n));
            }

            #[test]
            fn prop_no_edges_no_cycle(n in 1..20usize) {
                let edges: Vec<(usize, usize)> = vec![];
                prop_assert!(!has_cycle(&edges, n));
            }

            #[test]
            fn prop_self_loop_detected(v in 0..15usize) {
                let edges = vec![(v, v)];
                prop_assert!(has_cycle(&edges, v + 1));
            }
        }
    }
}
