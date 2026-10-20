//! Verified UCAN parsed claims presence admission.
//!
//! This module proves only a finite predicate over caller- or shell-supplied
//! parsed-field presence facts. It does not verify JSON/serde parsing,
//! duplicate-key behavior, field extraction correctness, string validation,
//! capability/proof parsing, proof-reference decoding, temporal ordering, signing input
//! construction, cryptography, or token provenance.

use vstd::prelude::*;

verus! {

/// Shell-supplied presence facts for parsed UCAN payload claims.
#[allow(clippy::struct_excessive_bools)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ParsedClaimsPresence {
    pub issuer_present: bool,
    pub audience_present: bool,
    pub attenuations_present: bool,
    pub proofs_present: bool,
    pub expiration_present: bool,
    pub not_before_present: bool,
}

/// Parsed required-claims presence is valid exactly when every required field
/// was reported present by the shell/parser boundary.
// r[impl ucan-claims-presence.required-fields]
pub open spec fn parsed_claims_presence_spec(facts: ParsedClaimsPresence) -> bool {
    facts.issuer_present
        && facts.audience_present
        && facts.attenuations_present
        && facts.proofs_present
        && facts.expiration_present
        && facts.not_before_present
}

/// Check parsed required-claims presence.
// r[impl ucan-claims-presence.required-fields]
pub fn parsed_claims_presence_is_valid(facts: ParsedClaimsPresence) -> (result: bool)
    ensures result == parsed_claims_presence_spec(facts),
{
    facts.issuer_present
        && facts.audience_present
        && facts.attenuations_present
        && facts.proofs_present
        && facts.expiration_present
        && facts.not_before_present
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(
        issuer_present: bool,
        audience_present: bool,
        attenuations_present: bool,
        proofs_present: bool,
        expiration_present: bool,
        not_before_present: bool,
    ) -> ParsedClaimsPresence {
        ParsedClaimsPresence {
            issuer_present,
            audience_present,
            attenuations_present,
            proofs_present,
            expiration_present,
            not_before_present,
        }
    }

    #[test]
    // r[verify ucan-claims-presence.required-fields]
    fn accepts_all_required_fields_present() {
        assert!(parsed_claims_presence_is_valid(facts(true, true, true, true, true, true)));
    }

    #[test]
    // r[verify ucan-claims-presence.required-fields]
    fn rejects_missing_required_field_presence() {
        assert!(!parsed_claims_presence_is_valid(facts(false, true, true, true, true, true)));
        assert!(!parsed_claims_presence_is_valid(facts(true, false, true, true, true, true)));
        assert!(!parsed_claims_presence_is_valid(facts(true, true, false, true, true, true)));
        assert!(!parsed_claims_presence_is_valid(facts(true, true, true, false, true, true)));
        assert!(!parsed_claims_presence_is_valid(facts(true, true, true, true, false, true)));
        assert!(!parsed_claims_presence_is_valid(facts(true, true, true, true, true, false)));
    }
}
