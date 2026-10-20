//! Verified UCAN parsed proof-reference list-shape admission.
//!
//! This module proves only finite shell-supplied entry-shape facts for parsed
//! proof references. It does not decode base64url, compute hashes, resolve
//! proofs, verify signatures, or prove proof-store truth/provenance.

use vstd::prelude::*;

verus! {

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParsedProofReferenceEntryShape {
    pub encoded_non_empty: bool,
    pub encoded_base64url_no_pad: bool,
    pub decoded_non_empty: bool,
}

pub open spec fn proof_reference_entry_shape_spec(fact: ParsedProofReferenceEntryShape) -> bool {
    fact.encoded_non_empty && fact.encoded_base64url_no_pad && fact.decoded_non_empty
}

pub open spec fn proof_reference_list_shape_spec(facts: Seq<ParsedProofReferenceEntryShape>) -> bool {
    forall|i: int| #![trigger facts[i]] 0 <= i < facts.len() ==> proof_reference_entry_shape_spec(facts[i])
}

// r[impl ucan-proof-reference-list-shape.entry-shape]
pub fn proof_reference_entry_shape_is_valid(fact: ParsedProofReferenceEntryShape) -> (result: bool)
    ensures result == proof_reference_entry_shape_spec(fact),
{
    fact.encoded_non_empty && fact.encoded_base64url_no_pad && fact.decoded_non_empty
}

// r[impl ucan-proof-reference-list-shape.list-shape]
pub fn proof_reference_list_shape_is_valid(facts: &Vec<ParsedProofReferenceEntryShape>) -> (result: bool)
    ensures result == proof_reference_list_shape_spec(facts@),
{
    let mut i: usize = 0;
    while i < facts.len()
        invariant
            0 <= i <= facts.len(),
            forall|j: int| #![trigger facts@[j]] 0 <= j < i ==> proof_reference_entry_shape_spec(facts@[j]),
        decreases facts.len() - i,
    {
        if !proof_reference_entry_shape_is_valid(facts[i]) {
            assert(!proof_reference_entry_shape_spec(facts@[i as int]));
            assert(!proof_reference_list_shape_spec(facts@));
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

    // r[verify ucan-proof-reference-list-shape.entry-shape]
    // r[verify ucan-proof-reference-list-shape.list-shape]
    #[test]
    fn accepts_empty_and_valid_entries() {
        assert!(proof_reference_list_shape_is_valid(&Vec::new()));
        let facts = vec![ParsedProofReferenceEntryShape {
            encoded_non_empty: true,
            encoded_base64url_no_pad: true,
            decoded_non_empty: true,
        }];
        assert!(proof_reference_list_shape_is_valid(&facts));
    }

    #[test]
    fn rejects_invalid_entry() {
        let facts = vec![ParsedProofReferenceEntryShape {
            encoded_non_empty: true,
            encoded_base64url_no_pad: false,
            decoded_non_empty: true,
        }];
        assert!(!proof_reference_list_shape_is_valid(&facts));
    }
}
