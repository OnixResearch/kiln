//! Exact finite component-partition envelope.
use vstd::prelude::*;
verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct GraphComponentPartition { pub node_count: u8, pub covered_count: u8, pub disjoint: bool, pub connected_inside: bool, pub edges_valid: bool }
// r[depends domain-primitives.graph-component-partition]
pub open spec fn graph_component_partition_valid_spec(p: GraphComponentPartition) -> bool { p.node_count > 0 && p.covered_count == p.node_count && p.disjoint && p.connected_inside && p.edges_valid }
// r[impl domain-primitives.graph-component-partition]
pub fn graph_component_partition_valid(p: GraphComponentPartition) -> (result: bool) ensures result == graph_component_partition_valid_spec(p) { p.node_count > 0 && p.covered_count == p.node_count && p.disjoint && p.connected_inside && p.edges_valid }
// r[verify domain-primitives.graph-component-partition]
pub proof fn valid_partition_covers_nodes(p: GraphComponentPartition) requires graph_component_partition_valid_spec(p) ensures p.covered_count == p.node_count, p.disjoint {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn partition_requires_coverage() {
        assert!(graph_component_partition_valid(GraphComponentPartition {
            node_count: 2,
            covered_count: 2,
            disjoint: true,
            connected_inside: true,
            edges_valid: true
        }));
        assert!(!graph_component_partition_valid(GraphComponentPartition {
            node_count: 2,
            covered_count: 1,
            disjoint: true,
            connected_inside: true,
            edges_valid: true
        }));
    }
}
