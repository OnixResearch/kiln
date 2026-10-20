//! Exact finite objective-bound metadata.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct OptBound { pub has_lower: bool, pub lower: i64, pub has_upper: bool, pub upper: i64, pub score: i64 }

// r[depends domain-primitives.discrete-optimization.bound]
pub open spec fn opt_bound_valid_spec(b: OptBound) -> bool { (!b.has_lower || b.lower <= b.score) && (!b.has_upper || b.score <= b.upper) && (!(b.has_lower && b.has_upper) || b.lower <= b.upper) }

// r[impl domain-primitives.discrete-optimization.bound]
pub fn opt_bound_valid(b: OptBound) -> (result: bool) ensures result == opt_bound_valid_spec(b) { (!b.has_lower || b.lower <= b.score) && (!b.has_upper || b.score <= b.upper) && (!(b.has_lower && b.has_upper) || b.lower <= b.upper) }

// r[verify domain-primitives.discrete-optimization.bound]
pub proof fn valid_bound_orders_lower_upper(b: OptBound) requires opt_bound_valid_spec(b), b.has_lower, b.has_upper ensures b.lower <= b.upper {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bound_checks_score_and_order() {
        assert!(opt_bound_valid(OptBound {
            has_lower: true,
            lower: 0,
            has_upper: true,
            upper: 10,
            score: 5
        }));
        assert!(!opt_bound_valid(OptBound {
            has_lower: true,
            lower: 9,
            has_upper: true,
            upper: 3,
            score: 5
        }));
    }
}
