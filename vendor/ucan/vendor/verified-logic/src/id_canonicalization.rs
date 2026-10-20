//! Exact finite identifier/namespace primitive for CanonicalizationDecision.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CanonicalizationDecision { pub original_recorded: bool, pub decision_code: u8, pub target_valid: bool }

// r[depends domain-primitives.identifier-namespace.canonicalization]
pub open spec fn id_canonicalization_admitted_spec(x: CanonicalizationDecision) -> bool { x.original_recorded && x.decision_code <= 3 && x.target_valid }

// r[impl domain-primitives.identifier-namespace.canonicalization]
pub fn id_canonicalization_admitted(x: CanonicalizationDecision) -> (result: bool)
    ensures result == id_canonicalization_admitted_spec(x)
{
    x.original_recorded && x.decision_code <= 3 && x.target_valid
}

// r[verify domain-primitives.identifier-namespace.canonicalization]
pub proof fn admitted_id_canonicalization_has_required_metadata(x: CanonicalizationDecision)
    requires id_canonicalization_admitted_spec(x)
    ensures x.original_recorded, x.decision_code <= 3
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn id_canonicalization_accepts_and_rejects() {
        assert!(id_canonicalization_admitted(CanonicalizationDecision {
            original_recorded: true,
            decision_code: 1,
            target_valid: true
        }));
        assert!(!id_canonicalization_admitted(CanonicalizationDecision {
            original_recorded: false,
            decision_code: 1,
            target_valid: true
        }));
    }
}
