//! Exact finite optimum witness metadata.
use vstd::prelude::*;

use crate::opt_feasible_set::OptFeasibleSet;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct OptimumWitness { pub candidate: u8, pub candidate_score: i64, pub other0_score: i64, pub other1_score: i64 }

// r[depends domain-primitives.discrete-optimization.witness]
pub open spec fn optimum_witness_valid_spec(s: OptFeasibleSet, w: OptimumWitness) -> bool { crate::opt_feasible_set::opt_feasible_contains_spec(s,w.candidate) && w.candidate_score <= w.other0_score && w.candidate_score <= w.other1_score }

// r[impl domain-primitives.discrete-optimization.witness]
pub fn optimum_witness_valid(s: OptFeasibleSet, w: OptimumWitness) -> (result: bool) ensures result == (crate::opt_feasible_set::opt_feasible_set_valid_spec(s) && optimum_witness_valid_spec(s,w)) { crate::opt_feasible_set::opt_feasible_contains(s,w.candidate) && w.candidate_score <= w.other0_score && w.candidate_score <= w.other1_score }

// r[verify domain-primitives.discrete-optimization.witness]
pub proof fn witness_candidate_is_feasible(s: OptFeasibleSet, w: OptimumWitness) requires optimum_witness_valid_spec(s,w) ensures crate::opt_feasible_set::opt_feasible_contains_spec(s,w.candidate) {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    use crate::opt_feasible_set::OptFeasibleSet;
    #[test]
    fn witness_requires_feasible_best() {
        let fs = OptFeasibleSet {
            len: 1,
            c0: 1,
            c1: 0,
            c2: 0,
        };
        assert!(optimum_witness_valid(fs, OptimumWitness {
            candidate: 1,
            candidate_score: 1,
            other0_score: 2,
            other1_score: 3
        }));
        assert!(!optimum_witness_valid(fs, OptimumWitness {
            candidate: 2,
            candidate_score: 1,
            other0_score: 2,
            other1_score: 3
        }));
    }
}
