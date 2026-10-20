//! Exact finite enum-constraint membership.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct EnumConstraint { pub len: u8, pub v0: u8, pub v1: u8, pub v2: u8 }

// r[depends domain-primitives.data-validation.enum-constraint]
pub open spec fn enum_constraint_valid_spec(e: EnumConstraint) -> bool { e.len <= 3 }
pub open spec fn enum_constraint_contains_spec(e: EnumConstraint, v: u8) -> bool { (e.len >= 1 && e.v0 == v) || (e.len >= 2 && e.v1 == v) || (e.len >= 3 && e.v2 == v) }

// r[impl domain-primitives.data-validation.enum-constraint]
pub fn enum_constraint_admitted(e: EnumConstraint, v: u8) -> (result: bool) ensures result == (enum_constraint_valid_spec(e) && enum_constraint_contains_spec(e,v)) { e.len <= 3 && ((e.len >= 1 && e.v0 == v) || (e.len >= 2 && e.v1 == v) || (e.len >= 3 && e.v2 == v)) }

// r[verify domain-primitives.data-validation.enum-constraint]
pub proof fn admitted_enum_is_member(e: EnumConstraint, v: u8) requires enum_constraint_valid_spec(e) && enum_constraint_contains_spec(e,v) ensures enum_constraint_contains_spec(e,v) {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn enum_constraint_checks_membership() {
        let e = EnumConstraint {
            len: 2,
            v0: 4,
            v1: 7,
            v2: 0,
        };
        assert!(enum_constraint_admitted(e, 7));
        assert!(!enum_constraint_admitted(e, 8));
    }
}
