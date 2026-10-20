//! Exact finite monotone propagation envelopes.
use vstd::prelude::*;

use crate::constraint_domain::ConstraintDomain;
use crate::constraint_domain::constraint_domain_subset;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ConstraintPropagation { pub changed: bool, pub contradiction: bool }

// r[depends domain-primitives.constraint-solver.propagation]
pub open spec fn constraint_propagation_admitted_spec(before: ConstraintDomain, after: ConstraintDomain, p: ConstraintPropagation) -> bool { crate::constraint_domain::constraint_domain_subset_spec(after,before) && (p.contradiction == (after.len == 0)) && (p.changed == !(before.len == after.len && before.v0 == after.v0 && before.v1 == after.v1)) }
// r[impl domain-primitives.constraint-solver.propagation]
pub fn constraint_propagation_admitted(before: ConstraintDomain, after: ConstraintDomain, p: ConstraintPropagation) -> (result: bool) ensures result == constraint_propagation_admitted_spec(before,after,p) { constraint_domain_subset(after,before) && (p.contradiction == (after.len == 0)) && (p.changed == !(before.len == after.len && before.v0 == after.v0 && before.v1 == after.v1)) }
// r[verify domain-primitives.constraint-solver.propagation]
pub proof fn propagation_never_expands(before: ConstraintDomain, after: ConstraintDomain, p: ConstraintPropagation) requires constraint_propagation_admitted_spec(before,after,p) ensures crate::constraint_domain::constraint_domain_subset_spec(after,before) {}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraint_domain::ConstraintDomain;
    #[test]
    fn propagation_requires_subset() {
        let before = ConstraintDomain {
            id: 1,
            len: 2,
            v0: 3,
            v1: 4,
        };
        let after = ConstraintDomain {
            id: 1,
            len: 1,
            v0: 3,
            v1: 0,
        };
        assert!(constraint_propagation_admitted(before, after, ConstraintPropagation {
            changed: true,
            contradiction: false
        }));
    }
}
