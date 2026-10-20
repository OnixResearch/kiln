//! Exact finite pairwise dominance predicate.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct OptDominance { pub left: u8, pub right: u8, pub left_a: i64, pub left_b: i64, pub right_a: i64, pub right_b: i64 }

// r[depends domain-primitives.discrete-optimization.dominance]
pub open spec fn opt_dominates_spec(d: OptDominance) -> bool { d.left != d.right && d.left_a <= d.right_a && d.left_b <= d.right_b && (d.left_a < d.right_a || d.left_b < d.right_b) }

// r[impl domain-primitives.discrete-optimization.dominance]
pub fn opt_dominates(d: OptDominance) -> (result: bool) ensures result == opt_dominates_spec(d) { d.left != d.right && d.left_a <= d.right_a && d.left_b <= d.right_b && (d.left_a < d.right_a || d.left_b < d.right_b) }

// r[verify domain-primitives.discrete-optimization.dominance]
pub proof fn dominance_names_distinct_candidates(d: OptDominance) requires opt_dominates_spec(d) ensures d.left != d.right {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dominance_requires_no_worse_and_better() {
        assert!(opt_dominates(OptDominance {
            left: 1,
            right: 2,
            left_a: 1,
            left_b: 2,
            right_a: 1,
            right_b: 3
        }));
        assert!(!opt_dominates(OptDominance {
            left: 1,
            right: 2,
            left_a: 2,
            left_b: 2,
            right_a: 1,
            right_b: 3
        }));
    }
}
