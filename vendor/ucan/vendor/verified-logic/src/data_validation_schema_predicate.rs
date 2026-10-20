//! Exact finite schema-predicate envelope admission.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SchemaPredicateEnvelope { pub len: u8, pub p0: u8, pub p1: u8, pub p2: u8 }

// r[depends domain-primitives.data-validation.schema-predicate]
pub open spec fn schema_predicate_valid_spec(s: SchemaPredicateEnvelope) -> bool { s.len <= 3 && (s.len == 0 || (s.len == 1 || s.p0 != s.p1) && (s.len <= 2 || (s.p0 != s.p2 && s.p1 != s.p2))) }

pub open spec fn schema_predicate_contains_spec(s: SchemaPredicateEnvelope, id: u8) -> bool { (s.len >= 1 && s.p0 == id) || (s.len >= 2 && s.p1 == id) || (s.len >= 3 && s.p2 == id) }

// r[impl domain-primitives.data-validation.schema-predicate]
pub fn schema_predicate_admitted(s: SchemaPredicateEnvelope, id: u8) -> (result: bool) ensures result == (schema_predicate_valid_spec(s) && schema_predicate_contains_spec(s,id)) { s.len <= 3 && (s.len == 0 || (s.len == 1 || s.p0 != s.p1) && (s.len <= 2 || (s.p0 != s.p2 && s.p1 != s.p2))) && ((s.len >= 1 && s.p0 == id) || (s.len >= 2 && s.p1 == id) || (s.len >= 3 && s.p2 == id)) }

// r[verify domain-primitives.data-validation.schema-predicate]
pub proof fn admitted_predicate_is_declared(s: SchemaPredicateEnvelope, id: u8) requires schema_predicate_valid_spec(s) && schema_predicate_contains_spec(s,id) ensures schema_predicate_contains_spec(s,id) {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn schema_predicates_reject_unknown() {
        let s = SchemaPredicateEnvelope {
            len: 2,
            p0: 1,
            p1: 2,
            p2: 0,
        };
        assert!(schema_predicate_admitted(s, 2));
        assert!(!schema_predicate_admitted(s, 3));
    }
}
