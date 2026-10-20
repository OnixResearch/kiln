//! Exact finite network-protocol primitive for MessageSequenceMetadata.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MessageSequenceMetadata { pub seq: u8, pub window: u8, pub duplicate: bool, pub delivery_claim: bool }

// r[depends domain-primitives.network-protocol.sequence]
pub open spec fn net_message_sequence_admitted_spec(x: MessageSequenceMetadata) -> bool { x.window > 0 && !x.duplicate && !x.delivery_claim }

// r[impl domain-primitives.network-protocol.sequence]
pub fn net_message_sequence_admitted(x: MessageSequenceMetadata) -> (result: bool)
    ensures result == net_message_sequence_admitted_spec(x)
{
    x.window > 0 && !x.duplicate && !x.delivery_claim
}

// r[verify domain-primitives.network-protocol.sequence]
pub proof fn admitted_net_message_sequence_has_required_metadata(x: MessageSequenceMetadata)
    requires net_message_sequence_admitted_spec(x)
    ensures x.window > 0, !x.duplicate, !x.delivery_claim
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn net_message_sequence_accepts_and_rejects() {
        assert!(net_message_sequence_admitted(MessageSequenceMetadata {
            seq: 1,
            window: 1,
            duplicate: false,
            delivery_claim: false
        }));
        assert!(!net_message_sequence_admitted(MessageSequenceMetadata {
            seq: 1,
            window: 1,
            duplicate: true,
            delivery_claim: false
        }));
    }
}
