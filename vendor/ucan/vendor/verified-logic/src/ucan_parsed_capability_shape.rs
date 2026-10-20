//! Verified UCAN parsed capability-list shape admission.
//!
//! This module proves only finite serde/shell-supplied presence facts for parsed
//! capability lists. Resource/ability string semantics, caveats, JSON parsing,
//! and authorization remain caller/runtime boundaries.

use vstd::prelude::*;

verus! {

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParsedCapabilityShapeFact {
    pub resource_present: bool,
    pub ability_present: bool,
}

pub open spec fn capability_shape_spec(fact: ParsedCapabilityShapeFact) -> bool {
    fact.resource_present && fact.ability_present
}

pub open spec fn capability_list_shape_spec(facts: Seq<ParsedCapabilityShapeFact>) -> bool {
    facts.len() > 0 && forall|i: int| #![trigger facts[i]] 0 <= i < facts.len() ==> capability_shape_spec(facts[i])
}

// r[impl ucan-parsed-capability-list-shape.required-entry-fields]
pub fn capability_shape_is_valid(fact: ParsedCapabilityShapeFact) -> (result: bool)
    ensures result == capability_shape_spec(fact),
{
    fact.resource_present && fact.ability_present
}

// r[impl ucan-parsed-capability-list-shape.required-entry-fields]
pub fn capability_list_shape_is_valid(facts: &Vec<ParsedCapabilityShapeFact>) -> (result: bool)
    ensures result == capability_list_shape_spec(facts@),
{
    if facts.len() == 0 {
        assert(!capability_list_shape_spec(facts@));
        return false;
    }
    let mut i: usize = 0;
    while i < facts.len()
        invariant
            0 <= i <= facts.len(),
            facts.len() > 0,
            forall|j: int| #![trigger facts@[j]] 0 <= j < i ==> capability_shape_spec(facts@[j]),
        decreases facts.len() - i,
    {
        if !capability_shape_is_valid(facts[i]) {
            assert(!capability_shape_spec(facts@[i as int]));
            assert(!capability_list_shape_spec(facts@));
            return false;
        }
        i = i + 1;
    }
    true
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    // r[verify ucan-parsed-capability-list-shape.required-entry-fields]
    #[test]
    fn accepts_non_empty_capability_list() {
        let facts = vec![ParsedCapabilityShapeFact {
            resource_present: true,
            ability_present: true,
        }];
        assert!(capability_list_shape_is_valid(&facts));
    }
    #[test]
    fn rejects_empty_or_missing_field() {
        assert!(!capability_list_shape_is_valid(&Vec::new()));
        let facts = vec![ParsedCapabilityShapeFact {
            resource_present: false,
            ability_present: true,
        }];
        assert!(!capability_list_shape_is_valid(&facts));
    }
}
