//! Exact finite cross-field predicate envelope.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct CrossFieldPredicate { pub left_present: bool, pub right_present: bool, pub relation_holds: bool }
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum CrossFieldDecision { Satisfied, Unsatisfied, Inapplicable }

// r[depends domain-primitives.data-validation.cross-field]
pub open spec fn cross_field_decision_spec(c: CrossFieldPredicate) -> CrossFieldDecision { if !(c.left_present && c.right_present) { CrossFieldDecision::Inapplicable } else if c.relation_holds { CrossFieldDecision::Satisfied } else { CrossFieldDecision::Unsatisfied } }

// r[impl domain-primitives.data-validation.cross-field]
pub fn cross_field_decision(c: CrossFieldPredicate) -> (result: CrossFieldDecision) ensures result == cross_field_decision_spec(c) { if !(c.left_present && c.right_present) { CrossFieldDecision::Inapplicable } else if c.relation_holds { CrossFieldDecision::Satisfied } else { CrossFieldDecision::Unsatisfied } }

// r[verify domain-primitives.data-validation.cross-field]
pub proof fn missing_dependency_is_inapplicable(c: CrossFieldPredicate) requires !c.left_present || !c.right_present ensures cross_field_decision_spec(c) == CrossFieldDecision::Inapplicable {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cross_field_requires_both_dependencies() {
        assert_eq!(
            cross_field_decision(CrossFieldPredicate {
                left_present: true,
                right_present: false,
                relation_holds: true
            }),
            CrossFieldDecision::Inapplicable
        );
        assert_eq!(
            cross_field_decision(CrossFieldPredicate {
                left_present: true,
                right_present: true,
                relation_holds: false
            }),
            CrossFieldDecision::Unsatisfied
        );
    }
}
