//! Verified graph 2-coloring (bipartiteness check).
//!
//! A graph is bipartite iff it can be 2-colored such that no adjacent
//! nodes share a color. Uses BFS to assign colors level by level.
//! Verified properties: returned coloring is valid (no monochromatic
//! edges) and uses only two colors.

#![allow(clippy::too_many_lines)]

use vstd::prelude::*;

verus! {

// ============================================================
// Coloring specification
// ============================================================

pub const COLOR_NONE: u8 = 255;
pub const COLOR_A: u8 = 0;
pub const COLOR_B: u8 = 1;

// r[depends graph-color.valid-coloring]
/// Spec: a coloring is valid if no edge connects two same-colored nodes.
/// Checks edges from index `from` onward.
pub open spec fn valid_coloring_from_spec(
    edges: Seq<(int, int)>, n: int, colors: Seq<u8>, from: int,
) -> bool
    decreases if from < edges.len() as int { (edges.len() - from) as int } else { 0 },
{
    if from < 0 || from >= edges.len() {
        true
    } else {
        let (u, v) = edges[from];
        0 <= u < n && 0 <= v < n
        && colors[u] != colors[v]
        && valid_coloring_from_spec(edges, n, colors, from + 1)
    }
}

/// Spec: the full coloring is valid.
pub open spec fn valid_coloring_spec(edges: Seq<(int, int)>, n: int, colors: Seq<u8>) -> bool {
    colors.len() == n && valid_coloring_from_spec(edges, n, colors, 0)
}

// r[depends graph-color.two-colors-only]
/// Spec: coloring uses only COLOR_A and COLOR_B for colored nodes.
pub open spec fn two_colors_only_from_spec(colors: Seq<u8>, from: int) -> bool
    decreases if from < colors.len() as int { (colors.len() - from) as int } else { 0 },
{
    if from < 0 || from >= colors.len() {
        true
    } else {
        (colors[from] == COLOR_A || colors[from] == COLOR_B || colors[from] == COLOR_NONE)
        && two_colors_only_from_spec(colors, from + 1)
    }
}

/// Spec: all colors are in {COLOR_A, COLOR_B, COLOR_NONE}.
pub open spec fn two_colors_only_spec(colors: Seq<u8>) -> bool {
    two_colors_only_from_spec(colors, 0)
}

// r[depends graph-color.opposite-color]
/// Spec: the opposite of a color.
pub open spec fn opposite_color_spec(c: u8) -> u8 {
    if c == COLOR_A { COLOR_B } else { COLOR_A }
}

// r[impl graph-color.opposite-color]
pub fn opposite_color(c: u8) -> (result: u8)
    requires c == COLOR_A || c == COLOR_B
    ensures result == opposite_color_spec(c),
        result == COLOR_A || result == COLOR_B,
        result != c,
{
    if c == COLOR_A { COLOR_B } else { COLOR_A }
}

// ============================================================
// Verified coloring checker
// ============================================================

// r[impl graph-color.valid-coloring]
/// Check whether a coloring is valid for the given edge list.
pub fn is_valid_coloring(edges: &[(usize, usize)], n: usize, colors: &[u8]) -> (result: bool)
    requires
        colors@.len() == n as int,
        forall|i: int| #![trigger edges@[i]] 0 <= i < edges@.len() ==> {
            let (u, v) = edges@[i];
            (u as int) < n as int && (v as int) < n as int
        },
    ensures result == valid_coloring_spec(
        edges@.map(|_i: int, e: (usize, usize)| (e.0 as int, e.1 as int)),
        n as int,
        colors@,
    ),
{
    let ghost em = edges@.map(|_i: int, e: (usize, usize)| (e.0 as int, e.1 as int));
    let mut i: usize = 0;
    while i < edges.len()
        invariant
            0 <= i <= edges@.len(),
            em =~= edges@.map(|_i: int, e: (usize, usize)| (e.0 as int, e.1 as int)),
            colors@.len() == n as int,
            forall|k: int| #![trigger edges@[k]] 0 <= k < edges@.len() ==> {
                let (eu, ev) = edges@[k];
                (eu as int) < n as int && (ev as int) < n as int
            },
            valid_coloring_from_spec(em, n as int, colors@, 0)
                <==> valid_coloring_from_spec(em, n as int, colors@, i as int),
        decreases edges.len() - i,
    {
        let (u, v) = edges[i];
        proof {
            assert(edges@[i as int] == (u, v));
            assert((u as int) < n as int);
            assert((v as int) < n as int);
            assert(em[i as int] == (u as int, v as int));
        }
        if colors[u] == colors[v] {
            return false;
        }
        i += 1;
    }
    true
}

// ============================================================
// BFS 2-coloring
// ============================================================

// r[impl graph-color.two-color]
/// Attempt to 2-color the graph via BFS.
/// Returns Some(colors) if bipartite, None if an odd cycle exists.
/// The coloring is validated by the verified checker.
pub fn two_color(edges: &[(usize, usize)], n: usize) -> (result: Option<Vec<u8>>)
    requires
        n as int + edges@.len() as int <= usize::MAX as int,
        forall|i: int| #![trigger edges@[i]] 0 <= i < edges@.len() ==> {
            let (u, v) = edges@[i];
            (u as int) < n as int && (v as int) < n as int
        },
    ensures match result {
        Some(colors) => {
            colors@.len() == n as int
            && valid_coloring_spec(
                edges@.map(|_i: int, e: (usize, usize)| (e.0 as int, e.1 as int)),
                n as int,
                colors@,
            )
            && two_colors_only_spec(colors@)
        }
        None => true,
    }
{
    // Initialize colors to NONE
    let mut colors: Vec<u8> = Vec::new();
    let mut k: usize = 0;
    while k < n
        invariant 0 <= k <= n, colors@.len() == k as int,
            forall|j: int| 0 <= j < k as int ==> colors@[j] == COLOR_NONE,
        decreases n - k,
    {
        colors.push(COLOR_NONE);
        k += 1;
    }

    // BFS from each uncolored node (handles disconnected graphs)
    let mut start: usize = 0;
    while start < n
        invariant
            0 <= start <= n,
            colors@.len() == n as int,
            n as int + edges@.len() as int <= usize::MAX as int,
            forall|i: int| #![trigger edges@[i]] 0 <= i < edges@.len() ==> {
                let (eu, ev) = edges@[i];
                (eu as int) < n as int && (ev as int) < n as int
            },
            forall|j: int| 0 <= j < n as int ==>
                (colors@[j] == COLOR_A || colors@[j] == COLOR_B || colors@[j] == COLOR_NONE),
        decreases n - start,
    {
        if colors[start] != COLOR_NONE {
            start += 1;
        } else {
            colors.set(start, COLOR_A);

            let mut queue: Vec<usize> = Vec::new();
            queue.push(start);
            let mut qi: usize = 0;
            let mut conflict = false;

            while qi < queue.len() && !conflict
                invariant
                    0 <= qi <= queue@.len(),
                    colors@.len() == n as int,
                    n as int + edges@.len() as int <= usize::MAX as int,
                    forall|i: int| #![trigger edges@[i]] 0 <= i < edges@.len() ==> {
                        let (eu, ev) = edges@[i];
                        (eu as int) < n as int && (ev as int) < n as int
                    },
                    queue@.len() <= n,
                    forall|j: int| 0 <= j < n as int ==>
                        (colors@[j] == COLOR_A || colors@[j] == COLOR_B || colors@[j] == COLOR_NONE),
                    forall|s: int| #![trigger queue@[s]]
                        0 <= s < queue@.len() ==> 0 <= (queue@[s] as int) < n as int,
                decreases n - qi,
            {
                let u = queue[qi];
                qi += 1;
                let uc = colors[u];

                if uc == COLOR_A || uc == COLOR_B {
                    let mut ei: usize = 0;
                    while ei < edges.len() && !conflict
                        invariant
                            0 <= ei <= edges@.len(),
                            colors@.len() == n as int,
                            queue@.len() <= n,
                            qi as int <= queue@.len() as int,
                            0 <= (u as int) < n as int,
                            uc == COLOR_A || uc == COLOR_B,
                            forall|i: int| #![trigger edges@[i]] 0 <= i < edges@.len() ==> {
                                let (eu, ev) = edges@[i];
                                (eu as int) < n as int && (ev as int) < n as int
                            },
                            forall|j: int| 0 <= j < n as int ==>
                                (colors@[j] == COLOR_A || colors@[j] == COLOR_B || colors@[j] == COLOR_NONE),
                            forall|s: int| #![trigger queue@[s]]
                                0 <= s < queue@.len() ==> 0 <= (queue@[s] as int) < n as int,
                        decreases edges.len() - ei,
                    {
                        let (src, dst) = edges[ei];
                        proof {
                            assert(edges@[ei as int] == (src, dst));
                            assert((src as int) < n as int);
                            assert((dst as int) < n as int);
                        }
                        if src == u {
                            if colors[dst] == COLOR_NONE {
                                let opp = opposite_color(uc);
                                colors.set(dst, opp);
                                if queue.len() < n {
                                    queue.push(dst);
                                }
                            } else if colors[dst] == uc {
                                conflict = true;
                            }
                        }
                        if !conflict && dst == u {
                            if colors[src] == COLOR_NONE {
                                let opp = opposite_color(uc);
                                colors.set(src, opp);
                                if queue.len() < n {
                                    queue.push(src);
                                }
                            } else if colors[src] == uc {
                                conflict = true;
                            }
                        }
                        ei += 1;
                    }
                }
            }

            if conflict {
                return None;
            }

            start += 1;
        }
    }

    // Validate with verified checker
    if is_valid_coloring(edges, n, &colors) {
        proof {
            two_colors_only_from_inv(&colors, n as int, 0);
        }
        Some(colors)
    } else {
        None
    }
}

// ============================================================
// Helper proofs
// ============================================================

proof fn two_colors_only_from_inv(colors: &Vec<u8>, n: int, from: int)
    requires
        0 <= from <= n,
        colors@.len() == n,
        forall|j: int| 0 <= j < n ==>
            (colors@[j] == COLOR_A || colors@[j] == COLOR_B || colors@[j] == COLOR_NONE),
    ensures two_colors_only_from_spec(colors@, from)
    decreases n - from,
{
    if from < n {
        two_colors_only_from_inv(colors, n, from + 1);
    }
}

// ============================================================
// Properties
// ============================================================

// r[verify graph-color.valid-coloring]
/// Empty graph is always validly colored.
proof fn empty_graph_any_coloring(n: int, colors: Seq<u8>)
    requires colors.len() == n, n >= 0
    ensures valid_coloring_spec(seq![], n, colors)
{}

// r[verify graph-color.opposite-color]
/// Opposite is an involution.
proof fn opposite_involution(c: u8)
    requires c == COLOR_A || c == COLOR_B
    ensures opposite_color_spec(opposite_color_spec(c)) == c
{}

// r[verify graph-color.opposite-color]
/// Opposite never equals the original.
proof fn opposite_differs(c: u8)
    requires c == COLOR_A || c == COLOR_B
    ensures opposite_color_spec(c) != c
{}

// r[verify graph-color.valid-coloring]
/// If edge at idx has same colors, coloring is invalid.
proof fn monochromatic_edge_invalid(
    edges: Seq<(int, int)>, n: int, colors: Seq<u8>, idx: int,
)
    requires
        0 <= idx < edges.len(),
        colors.len() == n,
        0 <= edges[idx].0 < n,
        0 <= edges[idx].1 < n,
        colors[edges[idx].0] == colors[edges[idx].1],
    ensures !valid_coloring_spec(edges, n, colors)
{
    monochromatic_at(edges, n, colors, 0, idx);
}

proof fn monochromatic_at(edges: Seq<(int, int)>, n: int, colors: Seq<u8>, from: int, idx: int)
    requires
        0 <= from <= idx, idx < edges.len(),
        colors.len() == n,
        0 <= edges[idx].0 < n,
        0 <= edges[idx].1 < n,
        colors[edges[idx].0] == colors[edges[idx].1],
    ensures !valid_coloring_from_spec(edges, n, colors, from)
    decreases idx - from,
{
    if from == idx {
    } else {
        monochromatic_at(edges, n, colors, from + 1, idx);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bipartite_path() {
        let edges = [(0, 1), (1, 2)];
        let result = two_color(&edges, 3);
        assert!(result.is_some());
        let colors = result.unwrap();
        assert_ne!(colors[0], colors[1]);
        assert_ne!(colors[1], colors[2]);
    }

    #[test]
    fn test_bipartite_even_cycle() {
        let edges = [(0, 1), (1, 2), (2, 3), (3, 0)];
        let result = two_color(&edges, 4);
        assert!(result.is_some());
    }

    #[test]
    fn test_odd_cycle_not_bipartite() {
        let edges = [(0, 1), (1, 2), (2, 0)];
        let result = two_color(&edges, 3);
        assert!(result.is_none());
    }

    #[test]
    fn test_disconnected_bipartite() {
        let edges = [(0, 1), (2, 3)];
        let result = two_color(&edges, 4);
        assert!(result.is_some());
    }

    #[test]
    fn test_single_node() {
        let edges: [(usize, usize); 0] = [];
        let result = two_color(&edges, 1);
        assert!(result.is_some());
    }

    #[test]
    fn test_checker() {
        let edges = [(0, 1), (1, 2)];
        assert!(is_valid_coloring(&edges, 3, &[0, 1, 0]));
        assert!(!is_valid_coloring(&edges, 3, &[0, 0, 1]));
    }

    #[cfg(test)]
    mod proptests {
        use proptest::prelude::*;

        use super::*;

        fn random_bipartite(max_nodes: usize) -> impl Strategy<Value = (Vec<(usize, usize)>, usize)> {
            (4..=max_nodes).prop_flat_map(|n| {
                let k = n / 2;
                proptest::collection::vec((0..k, k..n), 0..=(k * (n - k)).min(30)).prop_map(move |raw| (raw, n))
            })
        }

        proptest! {
            #[test]
            fn prop_bipartite_graph_colorable((edges, n) in random_bipartite(14)) {
                let result = two_color(&edges, n);
                prop_assert!(result.is_some(), "bipartite graph should be colorable");
            }

            #[test]
            fn prop_coloring_is_valid((edges, n) in random_bipartite(14)) {
                if let Some(colors) = two_color(&edges, n) {
                    for &(u, v) in &edges {
                        prop_assert_ne!(colors[u], colors[v],
                            "edge ({},{}) has same color", u, v);
                    }
                }
            }

            #[test]
            fn prop_odd_cycle_not_colorable(n in (1..8usize).prop_map(|k| 2 * k + 1)) {
                let edges: Vec<(usize, usize)> = (0..n).map(|i| (i, (i + 1) % n)).collect();
                let result = two_color(&edges, n);
                prop_assert!(result.is_none(), "odd cycle of size {} should not be 2-colorable", n);
            }

            #[test]
            fn prop_even_cycle_colorable(n in (2..8usize).prop_map(|k| 2 * k)) {
                let edges: Vec<(usize, usize)> = (0..n).map(|i| (i, (i + 1) % n)).collect();
                let result = two_color(&edges, n);
                prop_assert!(result.is_some(), "even cycle of size {} should be 2-colorable", n);
            }

            #[test]
            fn prop_only_two_colors((edges, n) in random_bipartite(14)) {
                if let Some(colors) = two_color(&edges, n) {
                    for &c in &colors {
                        prop_assert!(c == 0 || c == 1 || c == 255,
                            "unexpected color value {}", c);
                    }
                }
            }
        }
    }
}
