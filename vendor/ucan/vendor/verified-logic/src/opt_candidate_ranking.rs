//! Exact finite deterministic ranking envelopes.
use vstd::prelude::*;

use crate::opt_feasible_set::OptFeasibleSet;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct OptRanking { pub left: u8, pub right: u8, pub left_score: i64, pub right_score: i64 }

// r[depends domain-primitives.discrete-optimization.ranking]
pub open spec fn opt_ranking_valid_spec(s: OptFeasibleSet, r: OptRanking) -> bool { crate::opt_feasible_set::opt_feasible_contains_spec(s,r.left) && crate::opt_feasible_set::opt_feasible_contains_spec(s,r.right) }
pub open spec fn opt_left_preferred_spec(r: OptRanking) -> bool { r.left_score <= r.right_score }

// r[impl domain-primitives.discrete-optimization.ranking]
pub fn opt_ranking_left_preferred(s: OptFeasibleSet, r: OptRanking) -> (result: bool) ensures result == (crate::opt_feasible_set::opt_feasible_set_valid_spec(s) && opt_ranking_valid_spec(s,r) && opt_left_preferred_spec(r)) { crate::opt_feasible_set::opt_feasible_contains(s,r.left) && crate::opt_feasible_set::opt_feasible_contains(s,r.right) && r.left_score <= r.right_score }

// r[verify domain-primitives.discrete-optimization.ranking]
pub proof fn ranking_compares_admitted(s: OptFeasibleSet, r: OptRanking) requires opt_ranking_valid_spec(s,r) ensures crate::opt_feasible_set::opt_feasible_contains_spec(s,r.left), crate::opt_feasible_set::opt_feasible_contains_spec(s,r.right) {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    use crate::opt_feasible_set::OptFeasibleSet;
    #[test]
    fn ranking_requires_feasible_candidates() {
        let fs = OptFeasibleSet {
            len: 2,
            c0: 1,
            c1: 2,
            c2: 0,
        };
        assert!(opt_ranking_left_preferred(fs, OptRanking {
            left: 1,
            right: 2,
            left_score: 3,
            right_score: 4
        }));
        assert!(!opt_ranking_left_preferred(fs, OptRanking {
            left: 1,
            right: 3,
            left_score: 3,
            right_score: 4
        }));
    }
}
