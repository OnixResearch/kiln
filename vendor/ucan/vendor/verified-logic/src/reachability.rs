//! Verified graph reachability via BFS.
//!
//! Computes the set of nodes reachable from a source in a directed graph.
//! The spec uses fuel-bounded path existence. The exec implements BFS
//! with a visited array and queue. Soundness (every marked node has a
//! path from source) is verified; completeness is tested via proptest.

use vstd::prelude::*;

verus! {

// ============================================================
// Reachability spec (fuel-bounded paths)
// ============================================================

// r[depends reachability.has-edge]
/// Spec: edge (u,v) exists in the edge list.
pub open spec fn has_edge_spec(edges: Seq<(int, int)>, u: int, v: int) -> bool {
    exists|i: int| #![trigger edges[i]] 0 <= i < edges.len() && edges[i] == (u, v)
}

// r[depends reachability.reachable]
/// Spec: node `dst` is reachable from `src` in at most `fuel` steps.
pub open spec fn reachable_fuel(edges: Seq<(int, int)>, src: int, dst: int, fuel: nat) -> bool
    decreases fuel,
{
    if src == dst {
        true
    } else if fuel == 0 {
        false
    } else {
        exists|mid: int| has_edge_spec(edges, src, mid)
            && reachable_fuel(edges, mid, dst, (fuel - 1) as nat)
    }
}

/// Spec: a path exists from `src` to `dst` with some finite fuel witness.
pub open spec fn reachable_spec(edges: Seq<(int, int)>, n: int, src: int, dst: int) -> bool {
    exists|fuel: nat| reachable_fuel(edges, src, dst, fuel)
}

// r[depends reachability.reachable-set]
/// Spec: the visited array is sound — every marked node is reachable.
pub open spec fn visited_sound_spec(
    edges: Seq<(int, int)>, n: int, src: int, visited: Seq<bool>,
) -> bool {
    visited.len() == n
    && forall|v: int| #![trigger visited[v]]
        0 <= v < n && visited[v] ==> reachable_spec(edges, n, src, v)
}

/// Spec: the visited array is complete — every reachable node is marked.
pub open spec fn visited_complete_spec(
    edges: Seq<(int, int)>, n: int, src: int, visited: Seq<bool>,
) -> bool {
    visited.len() == n
    && forall|v: int| #![trigger visited[v]]
        0 <= v < n && reachable_spec(edges, n, src, v) ==> visited[v]
}

// ============================================================
// BFS reachability
// ============================================================

// r[impl reachability.reachable-set]
/// Compute reachable set from `src` via BFS.
/// Returns a boolean array where `result[v] == true` iff `v` is reachable from `src`.
pub fn reachable_from(edges: &[(usize, usize)], n: usize, src: usize) -> (result: Vec<bool>)
    requires
        src < n,
        n as int + edges@.len() as int <= usize::MAX as int,
        forall|i: int| #![trigger edges@[i]] 0 <= i < edges@.len() ==> {
            let (u, v) = edges@[i];
            (u as int) < n as int && (v as int) < n as int
        },
    ensures
        result@.len() == n as int,
        result@[src as int] == true,
        visited_sound_spec(
            edges@.map(|_i: int, e: (usize, usize)| (e.0 as int, e.1 as int)),
            n as int,
            src as int,
            result@,
        ),
{
    let ghost em = edges@.map(|_i: int, e: (usize, usize)| (e.0 as int, e.1 as int));

    // Initialize visited array
    let mut visited: Vec<bool> = Vec::new();
    let mut k: usize = 0;
    while k < n
        invariant 0 <= k <= n, visited@.len() == k as int,
            forall|j: int| 0 <= j < k as int ==> visited@[j] == false,
        decreases n - k,
    {
        visited.push(false);
        k += 1;
    }

    // BFS queue (index-based)
    visited.set(src, true);
    let mut queue: Vec<usize> = Vec::new();
    queue.push(src);

    proof {
        assert(reachable_fuel(em, src as int, src as int, n as nat));
    }

    let mut qi: usize = 0;
    while qi < queue.len()
        invariant
            0 <= qi <= queue@.len(),
            visited@.len() == n as int,
            queue@.len() <= n,
            src < n,
            em =~= edges@.map(|_i: int, e: (usize, usize)| (e.0 as int, e.1 as int)),
            forall|i: int| #![trigger edges@[i]] 0 <= i < edges@.len() ==> {
                let (eu, ev) = edges@[i];
                (eu as int) < n as int && (ev as int) < n as int
            },
            // Source marked
            visited@[src as int] == true,
            // All queue elements are valid and visited
            forall|s: int| #![trigger queue@[s]]
                0 <= s < queue@.len() ==>
                    0 <= (queue@[s] as int) < n as int
                    && visited@[queue@[s] as int] == true,
            // Soundness: all visited nodes are reachable from src
            forall|v: int| #![trigger visited@[v]]
                0 <= v < n as int && visited@[v] ==>
                    reachable_spec(em, n as int, src as int, v),
        decreases n - qi,
    {
        let u = queue[qi];
        proof {
            assert(0 <= (queue@[qi as int] as int) < n as int);
        }
        qi += 1;

        // Scan edges from u
        let mut ei: usize = 0;
        while ei < edges.len()
            invariant
                0 <= ei <= edges@.len(),
                visited@.len() == n as int,
                queue@.len() <= n,
                qi as int <= queue@.len() as int,
                0 <= (u as int) < n as int,
                src < n,
                em =~= edges@.map(|_i: int, e: (usize, usize)| (e.0 as int, e.1 as int)),
                forall|i: int| #![trigger edges@[i]] 0 <= i < edges@.len() ==> {
                    let (eu, ev) = edges@[i];
                    (eu as int) < n as int && (ev as int) < n as int
                },
                visited@[src as int] == true,
                // u is reachable
                reachable_spec(em, n as int, src as int, u as int),
                // All queue elements valid
                forall|s: int| #![trigger queue@[s]]
                    0 <= s < queue@.len() ==>
                        0 <= (queue@[s] as int) < n as int
                        && visited@[queue@[s] as int] == true,
                // Soundness maintained
                forall|v: int| #![trigger visited@[v]]
                    0 <= v < n as int && visited@[v] ==>
                        reachable_spec(em, n as int, src as int, v),
            decreases edges.len() - ei,
        {
            let (edge_src, dst) = edges[ei];
            proof {
                let e = edges@[ei as int];
                assert((e.0 as int) < n as int);
                assert((e.1 as int) < n as int);
            }
            if edge_src == u && !visited[dst] {
                proof {
                    assert(em[ei as int] == (u as int, dst as int));
                    assert(has_edge_spec(em, u as int, dst as int));
                    reachable_via_edge(em, n as int, src as int, u as int, dst as int);
                }
                visited.set(dst, true);
                if queue.len() < n {
                    queue.push(dst);
                }
            }
            ei += 1;
        }
    }

    visited
}

// ============================================================
// Reachable count
// ============================================================

// r[depends reachability.count]
/// Spec: count of reachable nodes from index `from`.
pub open spec fn reachable_count_from_spec(visited: Seq<bool>, from: int) -> int
    decreases if from < visited.len() as int { (visited.len() - from) as int } else { 0 },
{
    if from < 0 || from >= visited.len() {
        0int
    } else {
        let rest = reachable_count_from_spec(visited, from + 1);
        if visited[from] { rest + 1 } else { rest }
    }
}

/// Spec: total count of reachable nodes.
pub open spec fn reachable_count_spec(visited: Seq<bool>) -> int {
    reachable_count_from_spec(visited, 0)
}

// r[impl reachability.count]
/// Count the number of reachable (true) entries.
pub fn reachable_count(visited: &[bool]) -> (result: u64)
    requires visited@.len() <= u64::MAX as int
    ensures result as int == reachable_count_spec(visited@)
{
    let mut count: u64 = 0;
    let mut i: usize = visited.len();
    while i > 0
        invariant
            0 <= i <= visited@.len(),
            visited@.len() <= u64::MAX as int,
            count as int == reachable_count_from_spec(visited@, i as int),
            count as int <= (visited@.len() - i) as int,
        decreases i,
    {
        i -= 1;
        if visited[i] {
            count += 1;
        }
    }
    count
}

// ============================================================
// Helper proof lemmas
// ============================================================

/// If src reaches u and edge (u, dst) exists, then src reaches dst.
proof fn reachable_via_edge(edges: Seq<(int, int)>, n: int, src: int, u: int, dst: int)
    requires
        0 <= src < n,
        0 <= u < n,
        0 <= dst < n,
        reachable_spec(edges, n, src, u),
        has_edge_spec(edges, u, dst),
    ensures reachable_spec(edges, n, src, dst)
{
    let fuel = choose|fuel: nat| reachable_fuel(edges, src, u, fuel);
    extend_path(edges, src, u, dst, fuel);
    assert(exists|witness: nat| reachable_fuel(edges, src, dst, witness));
}

/// If reachable_fuel(src, u, fuel) and edge(u, dst) exists, then src reaches dst with one extra edge.
proof fn extend_path(edges: Seq<(int, int)>, src: int, u: int, dst: int, fuel: nat)
    requires
        reachable_fuel(edges, src, u, fuel),
        has_edge_spec(edges, u, dst),
    ensures reachable_fuel(edges, src, dst, (fuel + 1) as nat)
    decreases fuel,
{
    if src == dst {
        assert(reachable_fuel(edges, src, dst, (fuel + 1) as nat));
    } else if src == u {
        assert(reachable_fuel(edges, dst, dst, fuel));
    } else {
        let mid = choose|mid: int| has_edge_spec(edges, src, mid)
            && reachable_fuel(edges, mid, u, (fuel - 1) as nat);
        extend_path(edges, mid, u, dst, (fuel - 1) as nat);
    }
}

// ============================================================
// Properties
// ============================================================

// r[verify reachability.reachable]
/// Source is always reachable from itself.
proof fn source_reachable(edges: Seq<(int, int)>, n: int, src: int)
    requires n > 0, 0 <= src < n
    ensures reachable_spec(edges, n, src, src)
{
    assert(reachable_fuel(edges, src, src, 0));
}

// r[verify reachability.reachable]
/// If there's an edge from src to dst, dst is reachable.
proof fn neighbor_reachable(edges: Seq<(int, int)>, n: int, src: int, dst: int)
    requires n > 0, has_edge_spec(edges, src, dst)
    ensures reachable_spec(edges, n, src, dst)
{
    assert(reachable_fuel(edges, dst, dst, 0));
    assert(reachable_fuel(edges, src, dst, 1));
}

// r[verify reachability.count]
/// Reachable count is bounded by the number of nodes.
proof fn reachable_count_bounded(visited: Seq<bool>)
    ensures reachable_count_spec(visited) <= visited.len() as int
{
    reachable_count_bounded_from(visited, 0);
}

proof fn reachable_count_bounded_from(visited: Seq<bool>, from: int)
    requires 0 <= from <= visited.len()
    ensures reachable_count_from_spec(visited, from) <= (visited.len() - from) as int
    decreases visited.len() - from,
{
    if from < visited.len() as int {
        reachable_count_bounded_from(visited, from + 1);
    }
}

// r[verify reachability.count]
/// Reachable count is non-negative.
proof fn reachable_count_nonneg(visited: Seq<bool>)
    ensures reachable_count_spec(visited) >= 0
{
    reachable_count_nonneg_from(visited, 0);
}

proof fn reachable_count_nonneg_from(visited: Seq<bool>, from: int)
    requires 0 <= from <= visited.len()
    ensures reachable_count_from_spec(visited, from) >= 0
    decreases visited.len() - from,
{
    if from < visited.len() as int {
        reachable_count_nonneg_from(visited, from + 1);
    }
}

// r[verify reachability.count]
/// If source is the only reachable node, count is at least 1.
proof fn count_at_least_one(visited: Seq<bool>, src: int)
    requires 0 <= src < visited.len(), visited[src] == true
    ensures reachable_count_spec(visited) >= 1
{
    count_true_implies_ge_one(visited, 0, src);
}

proof fn count_true_implies_ge_one(visited: Seq<bool>, from: int, idx: int)
    requires 0 <= from <= idx, idx < visited.len(), visited[idx] == true
    ensures reachable_count_from_spec(visited, from) >= 1
    decreases idx - from,
{
    if from == idx {
        reachable_count_nonneg_from(visited, from + 1);
    } else {
        count_true_implies_ge_one(visited, from + 1, idx);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify reachability.reachable-set]
    #[test]
    fn test_linear_reachability() {
        let edges = [(0, 1), (1, 2)];
        let visited = reachable_from(&edges, 3, 0);
        assert!(visited[0]);
        assert!(visited[1]);
        assert!(visited[2]);
    }

    // r[verify reachability.reachable-set]
    #[test]
    fn test_unreachable_node() {
        let edges = [(0, 1)];
        let visited = reachable_from(&edges, 3, 0);
        assert!(visited[0]);
        assert!(visited[1]);
        assert!(!visited[2]);
    }

    // r[verify reachability.reachable-set]
    #[test]
    fn test_no_edges() {
        let edges: [(usize, usize); 0] = [];
        let visited = reachable_from(&edges, 5, 2);
        assert!(visited[2]);
        assert!(!visited[0]);
        assert!(!visited[4]);
    }

    // r[verify reachability.reachable-set]
    #[test]
    fn test_cycle_reachability() {
        let edges = [(0, 1), (1, 2), (2, 0)];
        let visited = reachable_from(&edges, 3, 0);
        assert!(visited[0]);
        assert!(visited[1]);
        assert!(visited[2]);
    }

    // r[verify reachability.count]
    #[test]
    fn test_reachable_count() {
        let edges = [(0, 1), (1, 2)];
        let visited = reachable_from(&edges, 4, 0);
        assert_eq!(reachable_count(&visited), 3);
    }

    #[cfg(test)]
    #[allow(clippy::cast_possible_truncation)]
    mod proptests {
        use proptest::prelude::*;

        use super::*;

        fn random_graph(max_nodes: usize) -> impl Strategy<Value = (Vec<(usize, usize)>, usize)> {
            (2..=max_nodes).prop_flat_map(|n| {
                proptest::collection::vec((0..n, 0..n), 0..=(n * 3).min(40)).prop_map(move |edges| (edges, n))
            })
        }

        proptest! {
            // r[verify reachability.reachable-set]
            #[test]
            fn prop_source_always_reachable((edges, n) in random_graph(15)) {
                for src in 0..n.min(5) {
                    let visited = reachable_from(&edges, n, src);
                    prop_assert!(visited[src], "source {} should be reachable", src);
                }
            }

            // r[verify reachability.reachable-set]
            #[test]
            fn prop_neighbor_reachable((edges, n) in random_graph(15)) {
                if n > 0 {
                    let visited = reachable_from(&edges, n, 0);
                    for &(u, v) in &edges {
                        if visited[u] {
                            prop_assert!(visited[v],
                                "node {} reachable but neighbor {} not (edge ({},{}))", u, v, u, v);
                        }
                    }
                }
            }

            // r[verify reachability.reachable-set]
            #[test]
            fn prop_reachable_count_sound((edges, n) in random_graph(15)) {
                if n > 0 {
                    let visited = reachable_from(&edges, n, 0);
                    let count = reachable_count(&visited);
                    let manual_count = visited.iter().filter(|&&b| b).count();
                    prop_assert_eq!(count as usize, manual_count);
                }
            }

            // r[verify reachability.reachable-set]
            #[test]
            fn prop_completeness_check((edges, n) in random_graph(12)) {
                if n > 0 {
                    let visited = reachable_from(&edges, n, 0);
                    let mut manual = vec![false; n];
                    manual[0] = true;
                    let mut changed = true;
                    while changed {
                        changed = false;
                        for &(u, v) in &edges {
                            if manual[u] && !manual[v] {
                                manual[v] = true;
                                changed = true;
                            }
                        }
                    }
                    for v in 0..n {
                        prop_assert_eq!(visited[v], manual[v],
                            "mismatch at node {}: bfs={}, manual={}", v, visited[v], manual[v]);
                    }
                }
            }

            // P0: for each reachable node, reconstruct and verify a concrete path
            #[test]
            fn prop_path_exists_for_reachable((edges, n) in random_graph(12)) {
                if n > 0 {
                    let visited = reachable_from(&edges, n, 0);
                    let mut parent = vec![usize::MAX; n];
                    let mut seen = vec![false; n];
                    let mut queue = std::collections::VecDeque::new();
                    seen[0] = true;
                    queue.push_back(0usize);
                    while let Some(u) = queue.pop_front() {
                        for &(src, dst) in &edges {
                            if src == u && !seen[dst] {
                                seen[dst] = true;
                                parent[dst] = u;
                                queue.push_back(dst);
                            }
                        }
                    }
                    for target in 0..n {
                        if visited[target] && target != 0 {
                            prop_assert!(seen[target]);
                            let mut cur = target;
                            while cur != 0 {
                                let p = parent[cur];
                                prop_assert!(edges.contains(&(p, cur)));
                                cur = p;
                            }
                        }
                    }
                }
            }

            // P0: no edges means only source is reachable
            #[test]
            fn prop_isolated_source_only_self(n in 2..10usize) {
                let edges: Vec<(usize, usize)> = vec![];
                let visited = reachable_from(&edges, n, 0);
                prop_assert!(visited[0]);
                for v in 1..n {
                    prop_assert!(!visited[v]);
                }
            }

            // P0: reachable count is in [1, n]
            #[test]
            fn prop_reachable_count_in_bounds((edges, n) in random_graph(15)) {
                if n > 0 {
                    let visited = reachable_from(&edges, n, 0);
                    let count = reachable_count(&visited);
                    prop_assert!(count >= 1);
                    prop_assert!(count <= n as u64);
                }
            }
        }
    }
}
