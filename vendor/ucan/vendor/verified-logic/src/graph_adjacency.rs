//! Exact finite graph adjacency admission.
use vstd::prelude::*;
verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct GraphEdge { pub node_count: u8, pub src: u8, pub dst: u8 }
// r[depends domain-primitives.graph-adjacency]
pub open spec fn graph_edge_admitted_spec(e: GraphEdge) -> bool { e.node_count > 0 && e.src < e.node_count && e.dst < e.node_count }
// r[impl domain-primitives.graph-adjacency]
pub fn graph_edge_admitted(e: GraphEdge) -> (result: bool) ensures result == graph_edge_admitted_spec(e) { e.node_count > 0 && e.src < e.node_count && e.dst < e.node_count }
// r[verify domain-primitives.graph-adjacency]
pub proof fn admitted_edge_endpoints_in_range(e: GraphEdge) requires graph_edge_admitted_spec(e) ensures e.src < e.node_count, e.dst < e.node_count {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adjacency_rejects_out_of_range_edge() {
        assert!(graph_edge_admitted(GraphEdge {
            node_count: 3,
            src: 0,
            dst: 2
        }));
        assert!(!graph_edge_admitted(GraphEdge {
            node_count: 3,
            src: 3,
            dst: 0
        }));
    }
}
