//! Exact finite network-protocol primitive for ChecksumShapeMetadata.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ChecksumShapeMetadata { pub offset: u8, pub width: u8, pub covered_len: u8, pub integrity_claim: bool }

// r[depends domain-primitives.network-protocol.checksum-shape]
pub open spec fn net_checksum_shape_admitted_spec(x: ChecksumShapeMetadata) -> bool { x.width > 0 && x.covered_len > 0 && !x.integrity_claim }

// r[impl domain-primitives.network-protocol.checksum-shape]
pub fn net_checksum_shape_admitted(x: ChecksumShapeMetadata) -> (result: bool)
    ensures result == net_checksum_shape_admitted_spec(x)
{
    x.width > 0 && x.covered_len > 0 && !x.integrity_claim
}

// r[verify domain-primitives.network-protocol.checksum-shape]
pub proof fn admitted_net_checksum_shape_has_required_metadata(x: ChecksumShapeMetadata)
    requires net_checksum_shape_admitted_spec(x)
    ensures x.width > 0, x.covered_len > 0, !x.integrity_claim
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn net_checksum_shape_accepts_and_rejects() {
        assert!(net_checksum_shape_admitted(ChecksumShapeMetadata {
            offset: 1,
            width: 1,
            covered_len: 1,
            integrity_claim: false
        }));
        assert!(!net_checksum_shape_admitted(ChecksumShapeMetadata {
            offset: 1,
            width: 0,
            covered_len: 1,
            integrity_claim: false
        }));
    }
}
