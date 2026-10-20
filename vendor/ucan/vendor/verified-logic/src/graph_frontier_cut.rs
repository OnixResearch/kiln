//! Exact finite traversal frontier/cut envelope.
use vstd::prelude::*;
verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct GraphFrontierCut { pub node_count: u8, pub visited_count: u8, pub frontier_count: u8, pub disjoint: bool, pub expansion_valid: bool }
// r[depends domain-primitives.graph-frontier-cut]
pub open spec fn graph_frontier_cut_valid_spec(f: GraphFrontierCut) -> bool { f.visited_count <= f.node_count && f.frontier_count <= f.node_count && f.disjoint && f.expansion_valid }
// r[impl domain-primitives.graph-frontier-cut]
pub fn graph_frontier_cut_valid(f: GraphFrontierCut) -> (result: bool) ensures result == graph_frontier_cut_valid_spec(f) { f.visited_count <= f.node_count && f.frontier_count <= f.node_count && f.disjoint && f.expansion_valid }
// r[verify domain-primitives.graph-frontier-cut]
pub proof fn valid_frontier_counts_in_range(f: GraphFrontierCut) requires graph_frontier_cut_valid_spec(f) ensures f.visited_count <= f.node_count, f.frontier_count <= f.node_count {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frontier_cut_requires_disjointness() {
        assert!(graph_frontier_cut_valid(GraphFrontierCut {
            node_count: 3,
            visited_count: 1,
            frontier_count: 1,
            disjoint: true,
            expansion_valid: true
        }));
        assert!(!graph_frontier_cut_valid(GraphFrontierCut {
            node_count: 3,
            visited_count: 1,
            frontier_count: 1,
            disjoint: false,
            expansion_valid: true
        }));
    }
}
