//! Exact finite local consistency predicates.
use vstd::prelude::*;

use crate::constraint_assignment::ConstraintAssignment;
use crate::constraint_assignment::constraint_assignment_value_for;
use crate::constraint_relation::ConstraintRelation;
use crate::constraint_relation::ConstraintTuple;
use crate::constraint_relation::constraint_relation_admits_tuple;

verus! {
// r[depends domain-primitives.constraint-solver.consistency]
pub open spec fn constraint_relation_assigned_tuple_spec(r: ConstraintRelation, a: ConstraintAssignment) -> Option<ConstraintTuple> { match (crate::constraint_assignment::constraint_assignment_value_for_spec(a,r.var0), crate::constraint_assignment::constraint_assignment_value_for_spec(a,r.var1)) { (Some(x), Some(y)) => Some(ConstraintTuple{arity:r.arity,v0:x,v1:y}), (Some(x), None) => if r.arity == 1 { Some(ConstraintTuple{arity:1,v0:x,v1:0}) } else { None }, _ => None } }
// r[depends domain-primitives.constraint-solver.consistency]
pub open spec fn constraint_consistent_spec(r: ConstraintRelation, a: ConstraintAssignment) -> bool { match constraint_relation_assigned_tuple_spec(r,a) { Some(t) => crate::constraint_relation::constraint_relation_admits_tuple_spec(r,t), None => false } }
// r[impl domain-primitives.constraint-solver.consistency]
pub fn constraint_relation_assigned_tuple(r: ConstraintRelation, a: ConstraintAssignment) -> (result: Option<ConstraintTuple>) ensures result == constraint_relation_assigned_tuple_spec(r,a) { match (constraint_assignment_value_for(a,r.var0), constraint_assignment_value_for(a,r.var1)) { (Some(x), Some(y)) => Some(ConstraintTuple{arity:r.arity,v0:x,v1:y}), (Some(x), None) => if r.arity == 1 { Some(ConstraintTuple{arity:1,v0:x,v1:0}) } else { None }, _ => None } }
// r[impl domain-primitives.constraint-solver.consistency]
pub fn constraint_consistent(r: ConstraintRelation, a: ConstraintAssignment) -> (result: bool) ensures result == constraint_consistent_spec(r,a) { match constraint_relation_assigned_tuple(r,a) { Some(t) => constraint_relation_admits_tuple(r,t), None => false } }
// r[verify domain-primitives.constraint-solver.consistency]
pub proof fn uncovered_scope_inconsistent(r: ConstraintRelation, a: ConstraintAssignment) requires constraint_relation_assigned_tuple_spec(r,a).is_None() ensures !constraint_consistent_spec(r,a) {}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraint_assignment::ConstraintAssignment;
    use crate::constraint_relation::ConstraintRelation;
    #[test]
    fn local_consistency_checks_relation_tuple() {
        let r = ConstraintRelation {
            id: 1,
            arity: 1,
            var0: 1,
            var1: 0,
            t0v0: 3,
            t0v1: 0,
            t1v0: 0,
            t1v1: 0,
            tuple_len: 1,
        };
        let a = ConstraintAssignment {
            var0: 1,
            has0: true,
            value0: 3,
            var1: 0,
            has1: false,
            value1: 0,
        };
        assert!(constraint_consistent(r, a));
    }
}
