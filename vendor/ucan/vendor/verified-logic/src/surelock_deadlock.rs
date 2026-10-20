//! Verified surelock deadlock-freedom core.
//!
//! Wait edges are admitted only when they move forward in lexicographic rank
//! `(level, id)`. Any path in such a graph strictly increases rank, so no
//! nontrivial cycle can exist.

use vstd::prelude::*;

use crate::surelock_id::SureLock;

verus! {

#[derive(Copy, Clone, Debug)]
pub struct WaitEdge {
    pub from: SureLock,
    pub to: SureLock,
}

// r[depends surelock-deadlock-freedom.forward-wait-rank]
pub open spec fn rank_lt_spec(a: SureLock, b: SureLock) -> bool {
    a.level < b.level || (a.level == b.level && a.id < b.id)
}

// r[impl surelock-deadlock-freedom.forward-wait-rank]
pub fn rank_lt(a: &SureLock, b: &SureLock) -> (result: bool)
    ensures result == rank_lt_spec(*a, *b)
{
    a.level < b.level || (a.level == b.level && a.id < b.id)
}

// r[depends surelock-deadlock-freedom.forward-wait-rank]
pub open spec fn wait_edge_forward_spec(edge: WaitEdge) -> bool {
    rank_lt_spec(edge.from, edge.to)
}

// r[impl surelock-deadlock-freedom.forward-wait-rank]
pub fn wait_edge_forward(edge: &WaitEdge) -> (result: bool)
    ensures result == wait_edge_forward_spec(*edge)
{
    rank_lt(&edge.from, &edge.to)
}

// r[depends surelock-deadlock-freedom.forward-wait-rank]
pub open spec fn all_wait_edges_forward_spec(edges: Seq<WaitEdge>) -> bool {
    forall|i: int| #![trigger edges[i]] 0 <= i < edges.len() ==> wait_edge_forward_spec(edges[i])
}

// r[depends surelock-deadlock-freedom.admitted-trace]
pub open spec fn admitted_trace_spec(edges: Seq<WaitEdge>) -> bool {
    all_wait_edges_forward_spec(edges)
}

// r[impl surelock-deadlock-freedom.admitted-trace]
pub fn admitted_trace(edges: &[WaitEdge]) -> (result: bool)
    ensures result == admitted_trace_spec(edges@)
{
    all_wait_edges_forward(edges)
}

// r[impl surelock-deadlock-freedom.forward-wait-rank]
pub fn all_wait_edges_forward(edges: &[WaitEdge]) -> (result: bool)
    ensures result == all_wait_edges_forward_spec(edges@)
{
    let mut i: usize = 0;
    while i < edges.len()
        invariant
            0 <= i <= edges@.len(),
            forall|j: int| #![trigger edges@[j]] 0 <= j < i as int ==> wait_edge_forward_spec(edges@[j]),
        decreases edges.len() - i,
    {
        if !wait_edge_forward(&edges[i]) {
            assert(!all_wait_edges_forward_spec(edges@));
            return false;
        }
        i += 1;
    }
    true
}

pub open spec fn has_wait_edge_spec(edges: Seq<WaitEdge>, from: SureLock, to: SureLock) -> bool {
    exists|i: int| #![trigger edges[i]] 0 <= i < edges.len() && edges[i].from == from && edges[i].to == to
}

pub open spec fn has_path_fuel(edges: Seq<WaitEdge>, src: SureLock, dst: SureLock, fuel: nat) -> bool
    decreases fuel,
{
    if src == dst {
        true
    } else if fuel == 0 {
        false
    } else {
        exists|mid: SureLock| has_wait_edge_spec(edges, src, mid)
            && has_path_fuel(edges, mid, dst, (fuel - 1) as nat)
    }
}

pub open spec fn has_nontrivial_cycle_at(edges: Seq<WaitEdge>, lock: SureLock) -> bool {
    exists|mid: SureLock| has_wait_edge_spec(edges, lock, mid)
        && has_path_fuel(edges, mid, lock, edges.len() as nat)
}

// r[impl surelock-deadlock-freedom.no-circular-wait]
pub open spec fn wait_graph_acyclic_spec(edges: Seq<WaitEdge>) -> bool {
    !exists|lock: SureLock| has_nontrivial_cycle_at(edges, lock)
}

// r[impl surelock-deadlock-freedom.admitted-trace]
pub open spec fn deadlock_free_admitted_trace_spec(edges: Seq<WaitEdge>) -> bool {
    admitted_trace_spec(edges) ==> wait_graph_acyclic_spec(edges)
}

// r[verify surelock-deadlock-freedom.forward-wait-rank]
pub proof fn rank_irreflexive(lock: SureLock)
    ensures !rank_lt_spec(lock, lock)
{}

// r[verify surelock-deadlock-freedom.forward-wait-rank]
pub proof fn rank_transitive(a: SureLock, b: SureLock, c: SureLock)
    requires rank_lt_spec(a, b), rank_lt_spec(b, c)
    ensures rank_lt_spec(a, c)
{}

// r[verify surelock-deadlock-freedom.forward-wait-rank]
pub proof fn path_strictly_increases_rank(edges: Seq<WaitEdge>, src: SureLock, dst: SureLock, fuel: nat)
    requires
        all_wait_edges_forward_spec(edges),
        has_path_fuel(edges, src, dst, fuel),
        src != dst,
    ensures rank_lt_spec(src, dst)
    decreases fuel,
{
    let mid = choose|mid: SureLock| has_wait_edge_spec(edges, src, mid)
        && has_path_fuel(edges, mid, dst, (fuel - 1) as nat);
    let eidx = choose|i: int| 0 <= i < edges.len() && edges[i].from == src && edges[i].to == mid;
    assert(wait_edge_forward_spec(edges[eidx]));
    if mid != dst {
        path_strictly_increases_rank(edges, mid, dst, (fuel - 1) as nat);
        rank_transitive(src, mid, dst);
    }
}

// r[verify surelock-deadlock-freedom.no-circular-wait]
pub proof fn forward_wait_graph_is_acyclic(edges: Seq<WaitEdge>)
    requires all_wait_edges_forward_spec(edges)
    ensures wait_graph_acyclic_spec(edges)
{
    if exists|lock: SureLock| has_nontrivial_cycle_at(edges, lock) {
        let lock = choose|lock: SureLock| has_nontrivial_cycle_at(edges, lock);
        let mid = choose|mid: SureLock| has_wait_edge_spec(edges, lock, mid)
            && has_path_fuel(edges, mid, lock, edges.len() as nat);
        let eidx = choose|i: int| 0 <= i < edges.len() && edges[i].from == lock && edges[i].to == mid;
        assert(wait_edge_forward_spec(edges[eidx]));
        if mid == lock {
            rank_irreflexive(lock);
            assert(false);
        } else {
            path_strictly_increases_rank(edges, mid, lock, edges.len() as nat);
            rank_transitive(lock, mid, lock);
            rank_irreflexive(lock);
            assert(false);
        }
    }
}

// r[verify surelock-deadlock-freedom.admitted-trace]
pub proof fn admitted_traces_are_deadlock_free(edges: Seq<WaitEdge>)
    ensures deadlock_free_admitted_trace_spec(edges)
{
    if admitted_trace_spec(edges) {
        forward_wait_graph_is_acyclic(edges);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::surelock_id::SureLock;

    fn lock(id: u64, level: u64) -> SureLock {
        SureLock { id, level }
    }

    fn edge(from: SureLock, to: SureLock) -> WaitEdge {
        WaitEdge { from, to }
    }

    // r[verify surelock-deadlock-freedom.forward-wait-rank]
    #[test]
    fn test_same_level_wait_moves_by_id() {
        let a = lock(1, 2);
        let b = lock(3, 2);
        assert!(rank_lt(&a, &b));
        assert!(!rank_lt(&b, &a));
    }

    // r[verify surelock-deadlock-freedom.forward-wait-rank]
    #[test]
    fn test_cross_level_wait_moves_by_level() {
        let a = lock(7, 1);
        let b = lock(1, 4);
        assert!(rank_lt(&a, &b));
    }

    // r[verify surelock-deadlock-freedom.no-circular-wait]
    #[test]
    fn test_same_level_sibling_locking_is_acyclic() {
        let a = lock(1, 2);
        let b = lock(2, 2);
        let c = lock(3, 2);
        let edges = [edge(a, b), edge(a, c), edge(b, c)];
        assert!(all_wait_edges_forward(&edges));
    }

    // r[verify surelock-deadlock-freedom.no-circular-wait]
    #[test]
    fn test_upward_only_cross_level_locking_is_acyclic() {
        let a = lock(1, 0);
        let b = lock(2, 1);
        let c = lock(3, 2);
        let edges = [edge(a, b), edge(b, c)];
        assert!(all_wait_edges_forward(&edges));
    }

    // r[verify surelock-deadlock-freedom.admitted-trace]
    #[test]
    fn test_wrong_order_rejected() {
        let high = lock(9, 4);
        let low = lock(1, 1);
        let edges = [edge(high, low)];
        assert!(!admitted_trace(&edges));
    }

    proptest! {
        // r[verify surelock-deadlock-freedom.forward-wait-rank]
        #[test]
        fn prop_same_level_rank_matches_id(level: u64, a: u64, b: u64) {
            let left = lock(a, level);
            let right = lock(b, level);
            prop_assert_eq!(rank_lt(&left, &right), a < b);
        }

        // r[verify surelock-deadlock-freedom.forward-wait-rank]
        #[test]
        fn prop_higher_level_always_higher_rank(id_a: u64, level_a: u64, id_b: u64, delta in 1u64..1000) {
            let left = lock(id_a, level_a);
            let right = lock(id_b, level_a.saturating_add(delta));
            if right.level > left.level {
                prop_assert!(rank_lt(&left, &right));
            }
        }

        // r[verify surelock-deadlock-freedom.admitted-trace]
        #[test]
        fn prop_reverse_edge_rejected(level: u64, lo: u64, hi: u64) {
            let a = lock(lo.min(hi), level);
            let b = lock(lo.max(hi), level);
            let edges = [edge(b, a)];
            prop_assert!(!admitted_trace(&edges));
        }
    }
}
