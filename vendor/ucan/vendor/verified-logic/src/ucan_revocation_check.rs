//! Verified UCAN revocation-check transcript admission.
//!
//! This module proves only finite caller/shell-supplied revocation result facts.
//! It does not prove registry honesty, freshness, storage/network correctness,
//! token provenance, or revocation authority semantics.

use vstd::prelude::*;

verus! {

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RevocationCheckFact {
    pub reference_non_empty: bool,
    pub backend_succeeded: bool,
    pub not_revoked: bool,
}

pub open spec fn revocation_check_spec(fact: RevocationCheckFact) -> bool {
    fact.reference_non_empty && fact.backend_succeeded && fact.not_revoked
}

// r[impl ucan-revocation-check-transcript.result-admission]
pub fn revocation_check_is_valid(fact: RevocationCheckFact) -> (result: bool)
    ensures result == revocation_check_spec(fact),
{
    fact.reference_non_empty && fact.backend_succeeded && fact.not_revoked
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    // r[verify ucan-revocation-check-transcript.result-admission]
    #[test]
    fn accepts_clear_success() {
        assert!(revocation_check_is_valid(RevocationCheckFact {
            reference_non_empty: true,
            backend_succeeded: true,
            not_revoked: true
        }));
    }
    #[test]
    fn rejects_revoked_or_failed_result() {
        assert!(!revocation_check_is_valid(RevocationCheckFact {
            reference_non_empty: true,
            backend_succeeded: true,
            not_revoked: false
        }));
        assert!(!revocation_check_is_valid(RevocationCheckFact {
            reference_non_empty: true,
            backend_succeeded: false,
            not_revoked: true
        }));
    }
}
