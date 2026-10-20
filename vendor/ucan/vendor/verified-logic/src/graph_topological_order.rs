//! Exact finite topological-order envelope.
use vstd::prelude::*;
verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct GraphTopologicalOrder { pub node_count: u8, pub order_len: u8, pub all_nodes_once: bool, pub edges_forward: bool, pub acyclic: bool }
// r[depends domain-primitives.graph-topological-order]
pub open spec fn graph_topological_order_valid_spec(t: GraphTopologicalOrder) -> bool { t.node_count > 0 && t.order_len == t.node_count && t.all_nodes_once && t.edges_forward && t.acyclic }
// r[impl domain-primitives.graph-topological-order]
pub fn graph_topological_order_valid(t: GraphTopologicalOrder) -> (result: bool) ensures result == graph_topological_order_valid_spec(t) { t.node_count > 0 && t.order_len == t.node_count && t.all_nodes_once && t.edges_forward && t.acyclic }
// r[verify domain-primitives.graph-topological-order]
pub proof fn valid_topological_order_has_edges_forward(t: GraphTopologicalOrder) requires graph_topological_order_valid_spec(t) ensures t.edges_forward, t.order_len == t.node_count {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn topological_order_rejects_cycle_marker() {
        assert!(graph_topological_order_valid(GraphTopologicalOrder {
            node_count: 3,
            order_len: 3,
            all_nodes_once: true,
            edges_forward: true,
            acyclic: true
        }));
        assert!(!graph_topological_order_valid(GraphTopologicalOrder {
            node_count: 3,
            order_len: 3,
            all_nodes_once: true,
            edges_forward: false,
            acyclic: false
        }));
    }
}
