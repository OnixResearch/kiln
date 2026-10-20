//! Verified UCAN proof-reference collection admission.
//!
//! This module proves only finite collection facts: caller-supplied proof
//! reference count is within a caller-supplied bound and every reference fact is
//! non-empty. It does not hash, decode, resolve, fetch, or verify proof truth.

use vstd::prelude::*;

verus! {

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProofReferenceCollectionFact {
    pub count: usize,
    pub max_count: usize,
    pub all_references_non_empty: bool,
}

pub open spec fn proof_reference_collection_spec(fact: ProofReferenceCollectionFact) -> bool {
    fact.count <= fact.max_count && fact.all_references_non_empty
}

// r[impl ucan-proof-reference-collection.collection-admission]
pub fn proof_reference_collection_is_valid(fact: ProofReferenceCollectionFact) -> (result: bool)
    ensures result == proof_reference_collection_spec(fact),
{
    fact.count <= fact.max_count && fact.all_references_non_empty
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    // r[verify ucan-proof-reference-collection.collection-admission]
    #[test]
    fn accepts_bounded_non_empty_collection_fact() {
        assert!(proof_reference_collection_is_valid(ProofReferenceCollectionFact {
            count: 2,
            max_count: 3,
            all_references_non_empty: true
        }));
    }
    #[test]
    fn rejects_over_limit_or_empty_reference_fact() {
        assert!(!proof_reference_collection_is_valid(ProofReferenceCollectionFact {
            count: 4,
            max_count: 3,
            all_references_non_empty: true
        }));
        assert!(!proof_reference_collection_is_valid(ProofReferenceCollectionFact {
            count: 1,
            max_count: 3,
            all_references_non_empty: false
        }));
    }
}
