//! Exact finite constraint domain predicates.
use vstd::prelude::*;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ConstraintDomain { pub id: u8, pub len: u8, pub v0: i16, pub v1: i16 }

// r[depends domain-primitives.constraint-solver.domain]
pub open spec fn constraint_domain_valid_spec(d: ConstraintDomain) -> bool { d.len <= 2 && (d.len < 2 || d.v0 != d.v1) }
// r[depends domain-primitives.constraint-solver.domain]
pub open spec fn constraint_domain_contains_spec(d: ConstraintDomain, value: i16) -> bool { constraint_domain_valid_spec(d) && ((d.len >= 1 && value == d.v0) || (d.len >= 2 && value == d.v1)) }
// r[depends domain-primitives.constraint-solver.domain]
pub open spec fn constraint_domain_subset_spec(a: ConstraintDomain, b: ConstraintDomain) -> bool { constraint_domain_valid_spec(a) && constraint_domain_valid_spec(b) && (a.len < 1 || constraint_domain_contains_spec(b,a.v0)) && (a.len < 2 || constraint_domain_contains_spec(b,a.v1)) }
// r[impl domain-primitives.constraint-solver.domain]
pub fn constraint_domain_valid(d: ConstraintDomain) -> (result: bool) ensures result == constraint_domain_valid_spec(d) { d.len <= 2 && (d.len < 2 || d.v0 != d.v1) }
// r[impl domain-primitives.constraint-solver.domain]
pub fn constraint_domain_contains(d: ConstraintDomain, value: i16) -> (result: bool) ensures result == constraint_domain_contains_spec(d,value) { constraint_domain_valid(d) && ((d.len >= 1 && value == d.v0) || (d.len >= 2 && value == d.v1)) }
// r[impl domain-primitives.constraint-solver.domain]
pub fn constraint_domain_subset(a: ConstraintDomain, b: ConstraintDomain) -> (result: bool) ensures result == constraint_domain_subset_spec(a,b) { constraint_domain_valid(a) && constraint_domain_valid(b) && (a.len < 1 || constraint_domain_contains(b,a.v0)) && (a.len < 2 || constraint_domain_contains(b,a.v1)) }
// r[verify domain-primitives.constraint-solver.domain]
pub proof fn absent_value_rejected(d: ConstraintDomain, value: i16) requires constraint_domain_valid_spec(d), !(d.len >= 1 && value == d.v0), !(d.len >= 2 && value == d.v1) ensures !constraint_domain_contains_spec(d,value) {}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finite_domain_membership() {
        let d = ConstraintDomain {
            id: 1,
            len: 2,
            v0: 3,
            v1: 5,
        };
        assert!(constraint_domain_contains(d, 3));
        assert!(!constraint_domain_contains(d, 4));
        let sub = ConstraintDomain {
            id: 2,
            len: 1,
            v0: 5,
            v1: 0,
        };
        assert!(constraint_domain_subset(sub, d));
    }
}
