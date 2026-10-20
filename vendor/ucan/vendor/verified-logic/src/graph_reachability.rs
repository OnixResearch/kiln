//! Exact finite reachability/path-witness envelope.
use vstd::prelude::*;
verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct GraphReachability { pub node_count: u8, pub src: u8, pub dst: u8, pub witness_valid: bool, pub path_len: u8 }
// r[depends domain-primitives.graph-reachability]
pub open spec fn graph_reachable_spec(r: GraphReachability) -> bool { r.node_count > 0 && r.src < r.node_count && r.dst < r.node_count && r.witness_valid && r.path_len > 0 }
// r[impl domain-primitives.graph-reachability]
pub fn graph_reachable(r: GraphReachability) -> (result: bool) ensures result == graph_reachable_spec(r) { r.node_count > 0 && r.src < r.node_count && r.dst < r.node_count && r.witness_valid && r.path_len > 0 }
// r[verify domain-primitives.graph-reachability]
pub proof fn reachable_has_valid_endpoints(r: GraphReachability) requires graph_reachable_spec(r) ensures r.src < r.node_count, r.dst < r.node_count {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reachability_requires_witness() {
        assert!(graph_reachable(GraphReachability {
            node_count: 2,
            src: 0,
            dst: 1,
            witness_valid: true,
            path_len: 2
        }));
        assert!(!graph_reachable(GraphReachability {
            node_count: 2,
            src: 0,
            dst: 1,
            witness_valid: false,
            path_len: 0
        }));
    }
}
