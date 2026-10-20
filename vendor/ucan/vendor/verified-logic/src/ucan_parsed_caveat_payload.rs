//! Verified UCAN parsed caveat payload admission.
//!
//! This module proves only finite serde/shell-supplied caveat payload facts:
//! payload field presence, encoded payload non-emptiness, encoded base64url
//! no-padding admission, and decoded payload non-emptiness. JSON parsing,
//! base64url decoder correctness, decoded-byte provenance, typed payload parsing,
//! policy semantics, clocks, counters, and token provenance remain runtime/caller
//! boundaries.
#![allow(clippy::struct_excessive_bools)]

use vstd::prelude::*;

verus! {

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParsedCaveatPayloadFact {
    pub payload_present: bool,
    pub encoded_non_empty: bool,
    pub encoded_base64url_no_pad: bool,
    pub decoded_non_empty: bool,
}

pub open spec fn caveat_payload_spec(fact: ParsedCaveatPayloadFact) -> bool {
    fact.payload_present
        && fact.encoded_non_empty
        && fact.encoded_base64url_no_pad
        && fact.decoded_non_empty
}

pub open spec fn caveat_payload_list_spec(facts: Seq<ParsedCaveatPayloadFact>) -> bool {
    forall|i: int| #![trigger facts[i]] 0 <= i < facts.len() ==> caveat_payload_spec(facts[i])
}

// r[impl ucan-parsed-caveat-payload-admission.payload-facts]
pub fn caveat_payload_is_valid(fact: ParsedCaveatPayloadFact) -> (result: bool)
    ensures result == caveat_payload_spec(fact),
{
    fact.payload_present
        && fact.encoded_non_empty
        && fact.encoded_base64url_no_pad
        && fact.decoded_non_empty
}

// r[impl ucan-parsed-caveat-payload-admission.payload-facts]
pub fn caveat_payload_list_is_valid(facts: &Vec<ParsedCaveatPayloadFact>) -> (result: bool)
    ensures result == caveat_payload_list_spec(facts@),
{
    let mut i: usize = 0;
    while i < facts.len()
        invariant
            0 <= i <= facts.len(),
            forall|j: int| #![trigger facts@[j]] 0 <= j < i ==> caveat_payload_spec(facts@[j]),
        decreases facts.len() - i,
    {
        if !caveat_payload_is_valid(facts[i]) {
            assert(!caveat_payload_spec(facts@[i as int]));
            assert(!caveat_payload_list_spec(facts@));
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

    fn valid_fact() -> ParsedCaveatPayloadFact {
        ParsedCaveatPayloadFact {
            payload_present: true,
            encoded_non_empty: true,
            encoded_base64url_no_pad: true,
            decoded_non_empty: true,
        }
    }

    // r[verify ucan-parsed-caveat-payload-admission.payload-facts]
    #[test]
    fn accepts_empty_list_and_valid_payloads() {
        assert!(caveat_payload_list_is_valid(&Vec::new()));
        assert!(caveat_payload_list_is_valid(&vec![valid_fact()]));
    }

    #[test]
    fn rejects_missing_payload() {
        let mut fact = valid_fact();
        fact.payload_present = false;
        assert!(!caveat_payload_is_valid(fact));
    }

    #[test]
    fn rejects_invalid_encoding_or_empty_decoded_payload() {
        let mut invalid_encoding = valid_fact();
        invalid_encoding.encoded_base64url_no_pad = false;
        assert!(!caveat_payload_is_valid(invalid_encoding));

        let mut empty_decoded = valid_fact();
        empty_decoded.decoded_non_empty = false;
        assert!(!caveat_payload_list_is_valid(&vec![empty_decoded]));
    }
}
