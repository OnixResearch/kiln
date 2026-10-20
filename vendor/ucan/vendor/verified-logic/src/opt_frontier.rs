//! Exact finite frontier membership envelope.
use vstd::prelude::*;

use crate::opt_feasible_set::OptFeasibleSet;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct OptFrontier { pub candidate: u8, pub dominated_by_any: bool }

// r[depends domain-primitives.discrete-optimization.frontier]
pub open spec fn opt_frontier_member_spec(s: OptFeasibleSet, f: OptFrontier) -> bool { crate::opt_feasible_set::opt_feasible_contains_spec(s,f.candidate) && !f.dominated_by_any }

// r[impl domain-primitives.discrete-optimization.frontier]
pub fn opt_frontier_member(s: OptFeasibleSet, f: OptFrontier) -> (result: bool) ensures result == (crate::opt_feasible_set::opt_feasible_set_valid_spec(s) && opt_frontier_member_spec(s,f)) { crate::opt_feasible_set::opt_feasible_contains(s,f.candidate) && !f.dominated_by_any }

// r[verify domain-primitives.discrete-optimization.frontier]
pub proof fn frontier_member_is_feasible(s: OptFeasibleSet, f: OptFrontier) requires opt_frontier_member_spec(s,f) ensures crate::opt_feasible_set::opt_feasible_contains_spec(s,f.candidate) {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    use crate::opt_feasible_set::OptFeasibleSet;
    #[test]
    fn frontier_requires_nondominated_feasible() {
        let fs = OptFeasibleSet {
            len: 1,
            c0: 8,
            c1: 0,
            c2: 0,
        };
        assert!(opt_frontier_member(fs, OptFrontier {
            candidate: 8,
            dominated_by_any: false
        }));
        assert!(!opt_frontier_member(fs, OptFrontier {
            candidate: 8,
            dominated_by_any: true
        }));
    }
}
