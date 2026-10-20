//! Verified UCAN parsed DID claim admission.
//!
//! This module proves only finite serde/shell-supplied shape facts for parsed
//! issuer/audience DID claims: present, non-empty, and already trimmed. JSON
//! parsing, UTF-8/string extraction, DID method semantics, key resolution, and
//! identity truth remain caller/runtime boundaries.

use vstd::prelude::*;

verus! {

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParsedDidClaimFact {
    pub present: bool,
    pub non_empty: bool,
    pub trimmed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParsedDidClaimsFact {
    pub issuer: ParsedDidClaimFact,
    pub audience: ParsedDidClaimFact,
}

pub open spec fn did_claim_spec(fact: ParsedDidClaimFact) -> bool {
    fact.present && fact.non_empty && fact.trimmed
}

pub open spec fn did_claims_spec(facts: ParsedDidClaimsFact) -> bool {
    did_claim_spec(facts.issuer) && did_claim_spec(facts.audience)
}

// r[impl ucan-parsed-did-claim.claim-admission]
pub fn did_claim_is_valid(fact: ParsedDidClaimFact) -> (result: bool)
    ensures result == did_claim_spec(fact),
{
    fact.present && fact.non_empty && fact.trimmed
}

// r[impl ucan-parsed-did-claim.claims-admission]
pub fn did_claims_are_valid(facts: ParsedDidClaimsFact) -> (result: bool)
    ensures result == did_claims_spec(facts),
{
    did_claim_is_valid(facts.issuer) && did_claim_is_valid(facts.audience)
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    fn claim(present: bool, non_empty: bool, trimmed: bool) -> ParsedDidClaimFact {
        ParsedDidClaimFact {
            present,
            non_empty,
            trimmed,
        }
    }

    #[test]
    // r[verify ucan-parsed-did-claim.claims-admission]
    fn accepts_present_non_empty_trimmed_issuer_and_audience() {
        assert!(did_claims_are_valid(ParsedDidClaimsFact {
            issuer: claim(true, true, true),
            audience: claim(true, true, true),
        }));
    }

    #[test]
    // r[verify ucan-parsed-did-claim.claim-admission]
    fn rejects_missing_empty_or_untrimmed_claim() {
        assert!(!did_claim_is_valid(claim(false, true, true)));
        assert!(!did_claim_is_valid(claim(true, false, true)));
        assert!(!did_claim_is_valid(claim(true, true, false)));
    }

    #[test]
    fn rejects_invalid_issuer_or_audience() {
        assert!(!did_claims_are_valid(ParsedDidClaimsFact {
            issuer: claim(true, true, true),
            audience: claim(true, false, true),
        }));
        assert!(!did_claims_are_valid(ParsedDidClaimsFact {
            issuer: claim(true, true, false),
            audience: claim(true, true, true),
        }));
    }
}
