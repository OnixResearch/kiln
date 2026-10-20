//! Exact finite inclusive range-constraint admission.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct RangeConstraint { pub min: i64, pub max: i64, pub value: i64 }

// r[depends domain-primitives.data-validation.range-constraint]
pub open spec fn range_constraint_admitted_spec(r: RangeConstraint) -> bool { r.min <= r.max && r.min <= r.value && r.value <= r.max }

// r[impl domain-primitives.data-validation.range-constraint]
pub fn range_constraint_admitted(r: RangeConstraint) -> (result: bool) ensures result == range_constraint_admitted_spec(r) { r.min <= r.max && r.min <= r.value && r.value <= r.max }

// r[verify domain-primitives.data-validation.range-constraint]
pub proof fn admitted_range_value_inside(r: RangeConstraint) requires range_constraint_admitted_spec(r) ensures r.min <= r.value, r.value <= r.max {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn range_constraint_checks_bounds() {
        assert!(range_constraint_admitted(RangeConstraint {
            min: 0,
            max: 5,
            value: 3
        }));
        assert!(!range_constraint_admitted(RangeConstraint {
            min: 0,
            max: 5,
            value: 9
        }));
    }
}
