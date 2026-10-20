//! Exact finite identifier/namespace primitive for IdentifierEnvelope.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct IdentifierEnvelope { pub len: u8, pub kind: u8, pub malformed: bool }

// r[depends domain-primitives.identifier-namespace.identifier]
pub open spec fn id_identifier_envelope_admitted_spec(x: IdentifierEnvelope) -> bool { x.len > 0 && x.kind > 0 && !x.malformed }

// r[impl domain-primitives.identifier-namespace.identifier]
pub fn id_identifier_envelope_admitted(x: IdentifierEnvelope) -> (result: bool)
    ensures result == id_identifier_envelope_admitted_spec(x)
{
    x.len > 0 && x.kind > 0 && !x.malformed
}

// r[verify domain-primitives.identifier-namespace.identifier]
pub proof fn admitted_id_identifier_envelope_has_required_metadata(x: IdentifierEnvelope)
    requires id_identifier_envelope_admitted_spec(x)
    ensures x.len > 0, x.kind > 0
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn id_identifier_envelope_accepts_and_rejects() {
        assert!(id_identifier_envelope_admitted(IdentifierEnvelope {
            len: 1,
            kind: 1,
            malformed: false
        }));
        assert!(!id_identifier_envelope_admitted(IdentifierEnvelope {
            len: 0,
            kind: 1,
            malformed: false
        }));
    }
}
