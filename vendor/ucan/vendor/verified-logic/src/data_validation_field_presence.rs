//! Exact finite required/optional field presence decisions.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct FieldPresence { pub required: bool, pub present_count: u8 }
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum FieldPresenceDecision { Missing, Present, Duplicate, OptionalAbsent }

// r[depends domain-primitives.data-validation.field-presence]
pub open spec fn field_presence_decision_spec(f: FieldPresence) -> FieldPresenceDecision { if f.present_count == 0 { if f.required { FieldPresenceDecision::Missing } else { FieldPresenceDecision::OptionalAbsent } } else if f.present_count == 1 { FieldPresenceDecision::Present } else { FieldPresenceDecision::Duplicate } }

// r[impl domain-primitives.data-validation.field-presence]
pub fn field_presence_decision(f: FieldPresence) -> (result: FieldPresenceDecision) ensures result == field_presence_decision_spec(f) { if f.present_count == 0 { if f.required { FieldPresenceDecision::Missing } else { FieldPresenceDecision::OptionalAbsent } } else if f.present_count == 1 { FieldPresenceDecision::Present } else { FieldPresenceDecision::Duplicate } }

// r[verify domain-primitives.data-validation.field-presence]
pub proof fn missing_required_is_missing(f: FieldPresence) requires f.required, f.present_count == 0 ensures field_presence_decision_spec(f) == FieldPresenceDecision::Missing {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn field_presence_reports_missing_and_duplicate() {
        assert_eq!(
            field_presence_decision(FieldPresence {
                required: true,
                present_count: 0
            }),
            FieldPresenceDecision::Missing
        );
        assert_eq!(
            field_presence_decision(FieldPresence {
                required: true,
                present_count: 2
            }),
            FieldPresenceDecision::Duplicate
        );
    }
}
