//! Exact finite constraint variable envelopes.
use vstd::prelude::*;

use crate::constraint_domain::ConstraintDomain;
use crate::constraint_domain::constraint_domain_valid;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ConstraintVariable { pub id: u8, pub domain_id: u8 }

// r[depends domain-primitives.constraint-solver.variable]
pub open spec fn constraint_variable_admitted_spec(v: ConstraintVariable, d0: ConstraintDomain, d1: ConstraintDomain) -> bool { v.id <= 15 && ((v.domain_id == d0.id && crate::constraint_domain::constraint_domain_valid_spec(d0)) || (v.domain_id == d1.id && crate::constraint_domain::constraint_domain_valid_spec(d1))) }
// r[impl domain-primitives.constraint-solver.variable]
pub fn constraint_variable_admitted(v: ConstraintVariable, d0: ConstraintDomain, d1: ConstraintDomain) -> (result: bool) ensures result == constraint_variable_admitted_spec(v,d0,d1) { v.id <= 15 && ((v.domain_id == d0.id && constraint_domain_valid(d0)) || (v.domain_id == d1.id && constraint_domain_valid(d1))) }
// r[verify domain-primitives.constraint-solver.variable]
pub proof fn undeclared_domain_rejected(v: ConstraintVariable, d0: ConstraintDomain, d1: ConstraintDomain) requires v.domain_id != d0.id, v.domain_id != d1.id ensures !constraint_variable_admitted_spec(v,d0,d1) {}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraint_domain::ConstraintDomain;
    #[test]
    fn variable_references_declared_domain() {
        let d = ConstraintDomain {
            id: 4,
            len: 1,
            v0: 9,
            v1: 0,
        };
        let e = ConstraintDomain {
            id: 5,
            len: 0,
            v0: 0,
            v1: 0,
        };
        assert!(constraint_variable_admitted(ConstraintVariable { id: 1, domain_id: 4 }, d, e));
        assert!(!constraint_variable_admitted(ConstraintVariable { id: 1, domain_id: 6 }, d, e));
    }
}
