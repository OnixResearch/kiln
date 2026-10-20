//! Exact finite network-protocol primitive for ParserAdmissionEnvelope.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ParserAdmissionEnvelope { pub parser_id: u8, pub frame_kind: u8, pub one_message: bool, pub stateful_claim: bool }

// r[depends domain-primitives.network-protocol.parser-admission]
pub open spec fn net_parser_admission_admitted_spec(x: ParserAdmissionEnvelope) -> bool { x.parser_id > 0 && x.frame_kind > 0 && x.one_message && !x.stateful_claim }

// r[impl domain-primitives.network-protocol.parser-admission]
pub fn net_parser_admission_admitted(x: ParserAdmissionEnvelope) -> (result: bool)
    ensures result == net_parser_admission_admitted_spec(x)
{
    x.parser_id > 0 && x.frame_kind > 0 && x.one_message && !x.stateful_claim
}

// r[verify domain-primitives.network-protocol.parser-admission]
pub proof fn admitted_net_parser_admission_has_required_metadata(x: ParserAdmissionEnvelope)
    requires net_parser_admission_admitted_spec(x)
    ensures x.parser_id > 0, x.frame_kind > 0, x.one_message
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn net_parser_admission_accepts_and_rejects() {
        assert!(net_parser_admission_admitted(ParserAdmissionEnvelope {
            parser_id: 1,
            frame_kind: 1,
            one_message: true,
            stateful_claim: false
        }));
        assert!(!net_parser_admission_admitted(ParserAdmissionEnvelope {
            parser_id: 0,
            frame_kind: 1,
            one_message: true,
            stateful_claim: false
        }));
    }
}
