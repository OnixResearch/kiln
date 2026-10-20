//! Verified UCAN proof-store result transcript admission.
//!
//! This module proves only finite caller/shell-supplied proof-store result facts.
//! It does not prove proof-store honesty, storage/network correctness,
//! freshness, compact-token parsing, signatures, or proof truth.

use vstd::prelude::*;

verus! {

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProofStoreResultFact {
    pub reference_non_empty: bool,
    pub backend_succeeded: bool,
    pub compact_token_present: bool,
}

pub open spec fn proof_store_result_spec(fact: ProofStoreResultFact) -> bool {
    fact.reference_non_empty && fact.backend_succeeded && fact.compact_token_present
}

// r[impl ucan-proof-store-result-transcript.result-admission]
pub fn proof_store_result_is_valid(fact: ProofStoreResultFact) -> (result: bool)
    ensures result == proof_store_result_spec(fact),
{
    fact.reference_non_empty && fact.backend_succeeded && fact.compact_token_present
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    // r[verify ucan-proof-store-result-transcript.result-admission]
    #[test]
    fn accepts_successful_result() {
        assert!(proof_store_result_is_valid(ProofStoreResultFact {
            reference_non_empty: true,
            backend_succeeded: true,
            compact_token_present: true
        }));
    }
    #[test]
    fn rejects_failed_or_empty_result() {
        assert!(!proof_store_result_is_valid(ProofStoreResultFact {
            reference_non_empty: true,
            backend_succeeded: false,
            compact_token_present: false
        }));
    }
}
