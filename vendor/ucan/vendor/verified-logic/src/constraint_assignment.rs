//! Exact finite partial/total assignment shape and domain admission.
use vstd::prelude::*;

use crate::constraint_domain::ConstraintDomain;
use crate::constraint_domain::constraint_domain_contains;
use crate::constraint_variable::ConstraintVariable;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ConstraintAssignment { pub var0: u8, pub has0: bool, pub value0: i16, pub var1: u8, pub has1: bool, pub value1: i16 }

// r[depends domain-primitives.constraint-solver.assignment]
pub open spec fn constraint_assignment_unique_spec(a: ConstraintAssignment) -> bool { !(a.has0 && a.has1 && a.var0 == a.var1) }
// r[depends domain-primitives.constraint-solver.assignment]
pub open spec fn constraint_assignment_value_for_spec(a: ConstraintAssignment, var: u8) -> Option<i16> { if a.has0 && a.var0 == var { Some(a.value0) } else if a.has1 && a.var1 == var { Some(a.value1) } else { None } }
// r[depends domain-primitives.constraint-solver.assignment]
pub open spec fn constraint_assignment_var_admitted_spec(a: ConstraintAssignment, v: ConstraintVariable, d: ConstraintDomain) -> bool { match constraint_assignment_value_for_spec(a,v.id) { Some(x) => crate::constraint_domain::constraint_domain_contains_spec(d,x), None => false } }
// r[impl domain-primitives.constraint-solver.assignment]
pub fn constraint_assignment_unique(a: ConstraintAssignment) -> (result: bool) ensures result == constraint_assignment_unique_spec(a) { !(a.has0 && a.has1 && a.var0 == a.var1) }
// r[impl domain-primitives.constraint-solver.assignment]
pub fn constraint_assignment_value_for(a: ConstraintAssignment, var: u8) -> (result: Option<i16>) ensures result == constraint_assignment_value_for_spec(a,var) { if a.has0 && a.var0 == var { Some(a.value0) } else if a.has1 && a.var1 == var { Some(a.value1) } else { None } }
// r[impl domain-primitives.constraint-solver.assignment]
pub fn constraint_assignment_var_admitted(a: ConstraintAssignment, v: ConstraintVariable, d: ConstraintDomain) -> (result: bool) ensures result == constraint_assignment_var_admitted_spec(a,v,d) { match constraint_assignment_value_for(a,v.id) { Some(x) => constraint_domain_contains(d,x), None => false } }
// r[verify domain-primitives.constraint-solver.assignment]
pub proof fn duplicate_assignment_rejected(a: ConstraintAssignment) requires a.has0, a.has1, a.var0 == a.var1 ensures !constraint_assignment_unique_spec(a) {}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraint_domain::ConstraintDomain;
    use crate::constraint_variable::ConstraintVariable;
    #[test]
    fn assignment_value_must_be_in_domain() {
        let a = ConstraintAssignment {
            var0: 1,
            has0: true,
            value0: 3,
            var1: 2,
            has1: false,
            value1: 0,
        };
        let v = ConstraintVariable { id: 1, domain_id: 7 };
        let d = ConstraintDomain {
            id: 7,
            len: 1,
            v0: 3,
            v1: 0,
        };
        assert!(constraint_assignment_var_admitted(a, v, d));
        let e = ConstraintDomain {
            id: 7,
            len: 1,
            v0: 4,
            v1: 0,
        };
        assert!(!constraint_assignment_var_admitted(a, v, e));
    }
}
