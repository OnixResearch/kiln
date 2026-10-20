//! Exact finite objective-score metadata.
use vstd::prelude::*;

use crate::opt_feasible_set::OptFeasibleSet;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct OptObjectiveScore { pub candidate: u8, pub score: i64 }

// r[depends domain-primitives.discrete-optimization.objective]
pub open spec fn opt_objective_score_valid_spec(s: OptFeasibleSet, o: OptObjectiveScore) -> bool { crate::opt_feasible_set::opt_feasible_contains_spec(s,o.candidate) }

// r[impl domain-primitives.discrete-optimization.objective]
pub fn opt_objective_score_valid(s: OptFeasibleSet, o: OptObjectiveScore) -> (result: bool) ensures result == (crate::opt_feasible_set::opt_feasible_set_valid_spec(s) && opt_objective_score_valid_spec(s,o)) { crate::opt_feasible_set::opt_feasible_contains(s,o.candidate) }

// r[verify domain-primitives.discrete-optimization.objective]
pub proof fn objective_references_admitted_candidate(s: OptFeasibleSet, o: OptObjectiveScore) requires opt_objective_score_valid_spec(s,o) ensures crate::opt_feasible_set::opt_feasible_contains_spec(s,o.candidate) {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    use crate::opt_feasible_set::OptFeasibleSet;
    #[test]
    fn opt_objective_requires_candidate() {
        let fs = OptFeasibleSet {
            len: 1,
            c0: 4,
            c1: 0,
            c2: 0,
        };
        assert!(opt_objective_score_valid(fs, OptObjectiveScore { candidate: 4, score: 9 }));
        assert!(!opt_objective_score_valid(fs, OptObjectiveScore { candidate: 5, score: 9 }));
    }
}
