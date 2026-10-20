//! Exact finite satisfiability and contradiction witness metadata.
use vstd::prelude::*;

use crate::constraint_assignment::ConstraintAssignment;
use crate::constraint_consistency::constraint_consistent;
use crate::constraint_relation::ConstraintRelation;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ConstraintWitness { pub kind: u8 }

// r[depends domain-primitives.constraint-solver.witness]
pub open spec fn constraint_witness_admitted_spec(w: ConstraintWitness, r: ConstraintRelation, a: ConstraintAssignment) -> bool { if w.kind == 1 { crate::constraint_consistency::constraint_consistent_spec(r,a) } else if w.kind == 2 { !crate::constraint_consistency::constraint_consistent_spec(r,a) } else { false } }
// r[impl domain-primitives.constraint-solver.witness]
pub fn constraint_witness_admitted(w: ConstraintWitness, r: ConstraintRelation, a: ConstraintAssignment) -> (result: bool) ensures result == constraint_witness_admitted_spec(w,r,a) { if w.kind == 1 { constraint_consistent(r,a) } else if w.kind == 2 { !constraint_consistent(r,a) } else { false } }
// r[verify domain-primitives.constraint-solver.witness]
pub proof fn satisfying_witness_requires_consistency(w: ConstraintWitness, r: ConstraintRelation, a: ConstraintAssignment) requires w.kind == 1, constraint_witness_admitted_spec(w,r,a) ensures crate::constraint_consistency::constraint_consistent_spec(r,a) {}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraint_assignment::ConstraintAssignment;
    use crate::constraint_relation::ConstraintRelation;
    #[test]
    fn witness_matches_consistency() {
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
        assert!(constraint_witness_admitted(ConstraintWitness { kind: 1 }, r, a));
    }
}
