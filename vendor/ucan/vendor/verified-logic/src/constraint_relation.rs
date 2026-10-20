//! Exact finite constraint relation envelopes.
use vstd::prelude::*;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ConstraintRelation { pub id: u8, pub arity: u8, pub var0: u8, pub var1: u8, pub t0v0: i16, pub t0v1: i16, pub t1v0: i16, pub t1v1: i16, pub tuple_len: u8 }
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ConstraintTuple { pub arity: u8, pub v0: i16, pub v1: i16 }

// r[depends domain-primitives.constraint-solver.relation]
pub open spec fn constraint_relation_valid_spec(r: ConstraintRelation) -> bool { (r.arity == 1 || r.arity == 2) && r.tuple_len <= 2 && (r.arity == 1 || r.var0 != r.var1) }
// r[depends domain-primitives.constraint-solver.relation]
pub open spec fn constraint_tuple_matches_at_spec(r: ConstraintRelation, t: ConstraintTuple, slot: u8) -> bool { if slot == 0 { t.v0 == r.t0v0 && (r.arity == 1 || t.v1 == r.t0v1) } else { t.v0 == r.t1v0 && (r.arity == 1 || t.v1 == r.t1v1) } }
// r[depends domain-primitives.constraint-solver.relation]
pub open spec fn constraint_relation_admits_tuple_spec(r: ConstraintRelation, t: ConstraintTuple) -> bool { constraint_relation_valid_spec(r) && t.arity == r.arity && ((r.tuple_len >= 1 && constraint_tuple_matches_at_spec(r,t,0)) || (r.tuple_len >= 2 && constraint_tuple_matches_at_spec(r,t,1))) }
// r[impl domain-primitives.constraint-solver.relation]
pub fn constraint_relation_valid(r: ConstraintRelation) -> (result: bool) ensures result == constraint_relation_valid_spec(r) { (r.arity == 1 || r.arity == 2) && r.tuple_len <= 2 && (r.arity == 1 || r.var0 != r.var1) }
// r[impl domain-primitives.constraint-solver.relation]
pub fn constraint_tuple_matches_at(r: ConstraintRelation, t: ConstraintTuple, slot: u8) -> (result: bool) ensures result == constraint_tuple_matches_at_spec(r,t,slot) { if slot == 0 { t.v0 == r.t0v0 && (r.arity == 1 || t.v1 == r.t0v1) } else { t.v0 == r.t1v0 && (r.arity == 1 || t.v1 == r.t1v1) } }
// r[impl domain-primitives.constraint-solver.relation]
pub fn constraint_relation_admits_tuple(r: ConstraintRelation, t: ConstraintTuple) -> (result: bool) ensures result == constraint_relation_admits_tuple_spec(r,t) { constraint_relation_valid(r) && t.arity == r.arity && ((r.tuple_len >= 1 && constraint_tuple_matches_at(r,t,0)) || (r.tuple_len >= 2 && constraint_tuple_matches_at(r,t,1))) }
// r[verify domain-primitives.constraint-solver.relation]
pub proof fn tuple_arity_mismatch_rejected(r: ConstraintRelation, t: ConstraintTuple) requires t.arity != r.arity ensures !constraint_relation_admits_tuple_spec(r,t) {}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn relation_checks_tuple_arity_and_table() {
        let r = ConstraintRelation {
            id: 1,
            arity: 2,
            var0: 1,
            var1: 2,
            t0v0: 3,
            t0v1: 4,
            t1v0: 5,
            t1v1: 6,
            tuple_len: 2,
        };
        assert!(constraint_relation_admits_tuple(r, ConstraintTuple { arity: 2, v0: 3, v1: 4 }));
        assert!(!constraint_relation_admits_tuple(r, ConstraintTuple { arity: 1, v0: 3, v1: 4 }));
    }
}
