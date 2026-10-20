//! Exact finite network-protocol primitive for PayloadBoundMetadata.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PayloadBoundMetadata { pub payload_len: u8, pub declared_len: u8, pub kind: u8, pub empty_forbidden: bool }

// r[depends domain-primitives.network-protocol.payload-bound]
pub open spec fn net_payload_bound_admitted_spec(x: PayloadBoundMetadata) -> bool { x.payload_len == x.declared_len && x.kind > 0 && (x.payload_len > 0 || !x.empty_forbidden) }

// r[impl domain-primitives.network-protocol.payload-bound]
pub fn net_payload_bound_admitted(x: PayloadBoundMetadata) -> (result: bool)
    ensures result == net_payload_bound_admitted_spec(x)
{
    x.payload_len == x.declared_len && x.kind > 0 && (x.payload_len > 0 || !x.empty_forbidden)
}

// r[verify domain-primitives.network-protocol.payload-bound]
pub proof fn admitted_net_payload_bound_has_required_metadata(x: PayloadBoundMetadata)
    requires net_payload_bound_admitted_spec(x)
    ensures x.payload_len == x.declared_len, x.kind > 0
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn net_payload_bound_accepts_and_rejects() {
        assert!(net_payload_bound_admitted(PayloadBoundMetadata {
            payload_len: 1,
            declared_len: 1,
            kind: 1,
            empty_forbidden: false
        }));
        assert!(!net_payload_bound_admitted(PayloadBoundMetadata {
            payload_len: 0,
            declared_len: 1,
            kind: 1,
            empty_forbidden: false
        }));
    }
}
