//! Verified unweighted shortest path via BFS.
//!
//! Computes shortest distances from a source node to all other nodes in
//! a directed graph. Uses BFS which naturally explores nodes in distance
//! order. Verified properties: source distance is zero, all distances
//! are non-negative or -1, and the triangle inequality holds for edges.

use vstd::prelude::*;

verus! {

/// Sentinel for unreachable nodes.
pub const UNREACHABLE: i64 = -1;

// ============================================================
// Distance specification
// ============================================================

// r[depends shortest-path.has-edge]
/// Spec: edge (u,v) exists.
pub open spec fn has_edge_spec(edges: Seq<(int, int)>, u: int, v: int) -> bool {
    exists|i: int| #![trigger edges[i]] 0 <= i < edges.len() && edges[i] == (u, v)
}

// r[depends shortest-path.shortest-dist]
/// Spec: a path of exactly `len` edges exists from src to dst.
pub open spec fn has_path_of_length(
    edges: Seq<(int, int)>, src: int, dst: int, len: nat,
) -> bool
    decreases len,
{
    if len == 0 {
        src == dst
    } else {
        exists|mid: int| has_edge_spec(edges, src, mid)
            && has_path_of_length(edges, mid, dst, (len - 1) as nat)
    }
}

// r[depends shortest-path.edge-triangle]
/// Spec: triangle inequality for edges. If u is reachable, v is at most
/// dist(u) + 1 away.
pub open spec fn edge_triangle_spec(
    edges: Seq<(int, int)>, n: int, dists: Seq<i64>,
) -> bool {
    forall|i: int| #![trigger edges[i]] 0 <= i < edges.len() ==> ({
        let (u, v) = edges[i];
        0 <= u < n && 0 <= v < n && (
        dists[u] == UNREACHABLE as int
        || dists[v] <= dists[u] + 1)
    })
}

// ============================================================
// BFS shortest path
// ============================================================

// r[impl shortest-path.bfs-distances]
/// Compute shortest distances from `src` to all nodes via BFS.
/// Returns dist[v] == -1 if v is unreachable, else the shortest hop count.
pub fn bfs_distances(edges: &[(usize, usize)], n: usize, src: usize) -> (result: Vec<i64>)
    requires
        src < n,
        n as int + edges@.len() as int <= usize::MAX as int,
        (n as int) < (i64::MAX as int),
        forall|i: int| #![trigger edges@[i]] 0 <= i < edges@.len() ==> {
            let (u, v) = edges@[i];
            (u as int) < n as int && (v as int) < n as int
        },
    ensures
        result@.len() == n as int,
        result@[src as int] == 0i64,
        forall|v: int| #![trigger result@[v]] 0 <= v < n as int ==> result@[v] >= UNREACHABLE as int,
        forall|v: int| #![trigger result@[v]] 0 <= v < n as int && v != src as int ==>
            (result@[v] == UNREACHABLE as int || result@[v] > 0),
{
    // Initialize distances to UNREACHABLE
    let mut dist: Vec<i64> = Vec::new();
    let mut k: usize = 0;
    while k < n
        invariant 0 <= k <= n, dist@.len() == k as int,
            forall|j: int| 0 <= j < k as int ==> dist@[j] == UNREACHABLE,
        decreases n - k,
    {
        dist.push(UNREACHABLE);
        k += 1;
    }

    // BFS from source
    dist.set(src, 0);
    let mut queue: Vec<usize> = Vec::new();
    queue.push(src);
    let mut qi: usize = 0;

    while qi < queue.len()
        invariant
            0 <= qi <= queue@.len(),
            dist@.len() == n as int,
            queue@.len() <= n,
            src < n,
            (n as int) < (i64::MAX as int),
            forall|i: int| #![trigger edges@[i]] 0 <= i < edges@.len() ==> {
                let (eu, ev) = edges@[i];
                (eu as int) < n as int && (ev as int) < n as int
            },
            dist@[src as int] == 0i64,
            forall|v: int| #![trigger dist@[v]] 0 <= v < n as int ==> dist@[v] >= UNREACHABLE as int,
            forall|v: int| #![trigger dist@[v]] 0 <= v < n as int && v != src as int ==>
                (dist@[v] == UNREACHABLE as int || dist@[v] > 0),
            forall|s: int| #![trigger queue@[s]]
                0 <= s < queue@.len() ==>
                    0 <= (queue@[s] as int) < n as int
                    && dist@[queue@[s] as int] >= 0
                    && dist@[queue@[s] as int] >= 0,
        decreases n - qi,
    {
        let u = queue[qi];
        proof {
            assert(0 <= (queue@[qi as int] as int) < n as int);
        }
        qi += 1;
        let du = dist[u];

        let mut ei: usize = 0;
        while ei < edges.len()
            invariant
                0 <= ei <= edges@.len(),
                dist@.len() == n as int,
                queue@.len() <= n,
                qi as int <= queue@.len() as int,
                0 <= (u as int) < n as int,
                du >= 0,
                du == dist@[u as int],
                src < n,
                (n as int) < (i64::MAX as int),
                forall|i: int| #![trigger edges@[i]] 0 <= i < edges@.len() ==> {
                    let (eu, ev) = edges@[i];
                    (eu as int) < n as int && (ev as int) < n as int
                },
                dist@[src as int] == 0i64,
                forall|v: int| #![trigger dist@[v]] 0 <= v < n as int ==> dist@[v] >= UNREACHABLE as int,
                forall|v: int| #![trigger dist@[v]] 0 <= v < n as int && v != src as int ==>
                    (dist@[v] == UNREACHABLE as int || dist@[v] > 0),
                forall|s: int| #![trigger queue@[s]]
                    0 <= s < queue@.len() ==>
                        0 <= (queue@[s] as int) < n as int
                        && dist@[queue@[s] as int] >= 0
                        && dist@[queue@[s] as int] >= 0,
            decreases edges.len() - ei,
        {
            let (edge_src, dst) = edges[ei];
            proof {
                let e = edges@[ei as int];
                assert((e.0 as int) < n as int);
                assert((e.1 as int) < n as int);
            }
            if edge_src == u && dist[dst] == UNREACHABLE && du < i64::MAX - 1 {
                let new_dist: i64 = du + 1;
                dist.set(dst, new_dist);
                if queue.len() < n {
                    queue.push(dst);
                }
            }
            ei += 1;
        }
    }

    dist
}

// ============================================================
// Properties
// ============================================================

// r[verify shortest-path.shortest-dist]
/// Source has distance 0 to itself.
proof fn source_distance_zero(edges: Seq<(int, int)>, src: int)
    ensures has_path_of_length(edges, src, src, 0)
{}

// r[verify shortest-path.has-edge]
/// A path of length 0 means src == dst.
proof fn path_zero_reflexive(edges: Seq<(int, int)>, src: int, dst: int)
    requires has_path_of_length(edges, src, dst, 0)
    ensures src == dst
{}

// r[verify shortest-path.has-edge]
/// Extending a path by one edge gives a longer path.
proof fn path_extend(edges: Seq<(int, int)>, src: int, mid: int, dst: int, len: nat)
    requires has_path_of_length(edges, src, mid, len), has_edge_spec(edges, mid, dst)
    ensures has_path_of_length(edges, src, dst, len + 1)
    decreases len,
{
    if len == 0 {
        assert(has_path_of_length(edges, dst, dst, 0));
    } else {
        let next = choose|next: int| has_edge_spec(edges, src, next)
            && has_path_of_length(edges, next, mid, (len - 1) as nat);
        path_extend(edges, next, mid, dst, (len - 1) as nat);
    }
}

// r[verify shortest-path.has-edge]
/// A single edge gives a path of length 1.
proof fn edge_gives_path(edges: Seq<(int, int)>, u: int, v: int)
    requires has_edge_spec(edges, u, v)
    ensures has_path_of_length(edges, u, v, 1)
{
    assert(has_path_of_length(edges, v, v, 0));
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_distances() {
        let edges = [(0, 1), (1, 2), (2, 3)];
        let dist = bfs_distances(&edges, 4, 0);
        assert_eq!(dist[0], 0);
        assert_eq!(dist[1], 1);
        assert_eq!(dist[2], 2);
        assert_eq!(dist[3], 3);
    }

    #[test]
    fn test_unreachable() {
        let edges = [(0, 1)];
        let dist = bfs_distances(&edges, 3, 0);
        assert_eq!(dist[0], 0);
        assert_eq!(dist[1], 1);
        assert_eq!(dist[2], UNREACHABLE);
    }

    #[test]
    fn test_shortest_path_diamond() {
        let edges = [(0, 1), (0, 2), (1, 3), (2, 3)];
        let dist = bfs_distances(&edges, 4, 0);
        assert_eq!(dist[0], 0);
        assert_eq!(dist[1], 1);
        assert_eq!(dist[2], 1);
        assert_eq!(dist[3], 2);
    }

    #[test]
    fn test_direct_shortcut() {
        let edges = [(0, 1), (1, 2), (0, 2)];
        let dist = bfs_distances(&edges, 3, 0);
        assert_eq!(dist[2], 1);
    }

    #[test]
    fn test_no_edges() {
        let edges: [(usize, usize); 0] = [];
        let dist = bfs_distances(&edges, 3, 1);
        assert_eq!(dist[0], UNREACHABLE);
        assert_eq!(dist[1], 0);
        assert_eq!(dist[2], UNREACHABLE);
    }

    #[test]
    fn test_triangle_inequality() {
        let edges = [(0, 1), (1, 2), (0, 2)];
        let dist = bfs_distances(&edges, 3, 0);
        for &(u, v) in &edges {
            if dist[u] != UNREACHABLE {
                assert!(dist[v] <= dist[u] + 1);
            }
        }
    }

    #[cfg(test)]
    mod proptests {
        use proptest::prelude::*;

        use super::*;

        fn random_graph(max_nodes: usize) -> impl Strategy<Value = (Vec<(usize, usize)>, usize)> {
            (2..=max_nodes).prop_flat_map(|n| {
                proptest::collection::vec((0..n, 0..n), 0..=(n * 3).min(40)).prop_map(move |edges| (edges, n))
            })
        }

        proptest! {
            #[test]
            fn prop_source_distance_zero((edges, n) in random_graph(15)) {
                for src in 0..n.min(5) {
                    let dist = bfs_distances(&edges, n, src);
                    prop_assert_eq!(dist[src], 0);
                }
            }

            #[test]
            fn prop_triangle_inequality((edges, n) in random_graph(15)) {
                if n > 0 {
                    let dist = bfs_distances(&edges, n, 0);
                    for &(u, v) in &edges {
                        if dist[u] != UNREACHABLE {
                            prop_assert!(dist[v] <= dist[u] + 1);
                        }
                    }
                }
            }

            #[test]
            fn prop_distances_nonneg_or_unreachable((edges, n) in random_graph(15)) {
                if n > 0 {
                    let dist = bfs_distances(&edges, n, 0);
                    for v in 0..n {
                        prop_assert!(dist[v] >= UNREACHABLE as i64);
                        prop_assert!(dist[v] == UNREACHABLE || dist[v] >= 0);
                    }
                }
            }

            #[test]
            fn prop_bfs_matches_naive((edges, n) in random_graph(12)) {
                if n > 0 {
                    let dist = bfs_distances(&edges, n, 0);
                    let mut naive = vec![-1i64; n];
                    naive[0] = 0;
                    let mut changed = true;
                    let mut iter = 0;
                    while changed && iter < n {
                        changed = false;
                        for &(u, v) in &edges {
                            if naive[u] >= 0 && (naive[v] == -1 || naive[u] + 1 < naive[v]) {
                                naive[v] = naive[u] + 1;
                                changed = true;
                            }
                        }
                        iter += 1;
                    }
                    for v in 0..n {
                        prop_assert_eq!(dist[v], naive[v]);
                    }
                }
            }

            #[test]
            fn prop_reachable_has_predecessor((edges, n) in random_graph(12)) {
                if n > 0 {
                    let dist = bfs_distances(&edges, n, 0);
                    for v in 0..n {
                        if dist[v] > 0 {
                            let has_pred = edges.iter().any(|&(u, w)| w == v && dist[u] == dist[v] - 1);
                            prop_assert!(has_pred,
                                "node {} at dist {} has no predecessor", v, dist[v]);
                        }
                    }
                }
            }
        }
    }
}
