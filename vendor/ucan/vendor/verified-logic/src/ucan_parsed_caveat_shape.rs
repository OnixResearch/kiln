//! Verified UCAN parsed caveat-list shape admission.
//!
//! This module proves only finite serde/shell-supplied caveat field-presence
//! facts. Payload decoding, typed payload parsing, policy semantics, clocks,
//! counters, and token provenance remain caller/runtime boundaries.
#![allow(clippy::struct_excessive_bools)]

use vstd::prelude::*;

verus! {

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParsedCaveatShapeFact {
    pub domain_present: bool,
    pub caveat_type_present: bool,
    pub key_present: bool,
    pub payload_present: bool,
}

pub open spec fn caveat_shape_spec(fact: ParsedCaveatShapeFact) -> bool {
    fact.domain_present && fact.caveat_type_present && fact.key_present && fact.payload_present
}

pub open spec fn caveat_list_shape_spec(facts: Seq<ParsedCaveatShapeFact>) -> bool {
    forall|i: int| #![trigger facts[i]] 0 <= i < facts.len() ==> caveat_shape_spec(facts[i])
}

// r[impl ucan-parsed-caveat-list-shape.required-entry-fields]
pub fn caveat_shape_is_valid(fact: ParsedCaveatShapeFact) -> (result: bool)
    ensures result == caveat_shape_spec(fact),
{
    fact.domain_present && fact.caveat_type_present && fact.key_present && fact.payload_present
}

// r[impl ucan-parsed-caveat-list-shape.required-entry-fields]
pub fn caveat_list_shape_is_valid(facts: &Vec<ParsedCaveatShapeFact>) -> (result: bool)
    ensures result == caveat_list_shape_spec(facts@),
{
    let mut i: usize = 0;
    while i < facts.len()
        invariant
            0 <= i <= facts.len(),
            forall|j: int| #![trigger facts@[j]] 0 <= j < i ==> caveat_shape_spec(facts@[j]),
        decreases facts.len() - i,
    {
        if !caveat_shape_is_valid(facts[i]) {
            assert(!caveat_shape_spec(facts@[i as int]));
            assert(!caveat_list_shape_spec(facts@));
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
    // r[verify ucan-parsed-caveat-list-shape.required-entry-fields]
    #[test]
    fn accepts_empty_and_valid_caveat_list() {
        assert!(caveat_list_shape_is_valid(&Vec::new()));
        let facts = vec![ParsedCaveatShapeFact {
            domain_present: true,
            caveat_type_present: true,
            key_present: true,
            payload_present: true,
        }];
        assert!(caveat_list_shape_is_valid(&facts));
    }
    #[test]
    fn rejects_missing_payload() {
        let facts = vec![ParsedCaveatShapeFact {
            domain_present: true,
            caveat_type_present: true,
            key_present: true,
            payload_present: false,
        }];
        assert!(!caveat_list_shape_is_valid(&facts));
    }
}
