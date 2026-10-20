//! Exact finite shortest-path cost envelope.
use vstd::prelude::*;
verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct GraphShortestPathEnvelope { pub path_valid: bool, pub nonnegative_weights: bool, pub cost: u16, pub bound: u16, pub overflow: bool }
// r[depends domain-primitives.graph-shortest-path-envelope]
pub open spec fn graph_shortest_path_envelope_valid_spec(s: GraphShortestPathEnvelope) -> bool { s.path_valid && s.nonnegative_weights && s.cost <= s.bound && !s.overflow }
// r[impl domain-primitives.graph-shortest-path-envelope]
pub fn graph_shortest_path_envelope_valid(s: GraphShortestPathEnvelope) -> (result: bool) ensures result == graph_shortest_path_envelope_valid_spec(s) { s.path_valid && s.nonnegative_weights && s.cost <= s.bound && !s.overflow }
// r[verify domain-primitives.graph-shortest-path-envelope]
pub proof fn valid_shortest_path_cost_bounded(s: GraphShortestPathEnvelope) requires graph_shortest_path_envelope_valid_spec(s) ensures s.cost <= s.bound, !s.overflow {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shortest_path_rejects_over_bound() {
        assert!(graph_shortest_path_envelope_valid(GraphShortestPathEnvelope {
            path_valid: true,
            nonnegative_weights: true,
            cost: 4,
            bound: 5,
            overflow: false
        }));
        assert!(!graph_shortest_path_envelope_valid(GraphShortestPathEnvelope {
            path_valid: true,
            nonnegative_weights: true,
            cost: 6,
            bound: 5,
            overflow: false
        }));
    }
}
