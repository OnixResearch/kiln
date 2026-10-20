//! Exact finite network-protocol primitive for PacketFrameEnvelope.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PacketFrameEnvelope { pub frame_len: u8, pub header_len: u8, pub payload_len: u8, pub range_invalid: bool }

// r[depends domain-primitives.network-protocol.packet-frame]
pub open spec fn net_packet_frame_admitted_spec(x: PacketFrameEnvelope) -> bool { x.frame_len > 0 && x.header_len <= x.frame_len && x.payload_len <= x.frame_len && x.payload_len <= x.frame_len - x.header_len && !x.range_invalid }

// r[impl domain-primitives.network-protocol.packet-frame]
pub fn net_packet_frame_admitted(x: PacketFrameEnvelope) -> (result: bool)
    ensures result == net_packet_frame_admitted_spec(x)
{
    x.frame_len > 0 && x.header_len <= x.frame_len && x.payload_len <= x.frame_len && x.payload_len <= x.frame_len - x.header_len && !x.range_invalid
}

// r[verify domain-primitives.network-protocol.packet-frame]
pub proof fn admitted_net_packet_frame_has_required_metadata(x: PacketFrameEnvelope)
    requires net_packet_frame_admitted_spec(x)
    ensures x.header_len <= x.frame_len, x.payload_len <= x.frame_len, x.payload_len <= x.frame_len - x.header_len
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn net_packet_frame_accepts_and_rejects() {
        assert!(net_packet_frame_admitted(PacketFrameEnvelope {
            frame_len: 2,
            header_len: 1,
            payload_len: 1,
            range_invalid: false
        }));
        assert!(!net_packet_frame_admitted(PacketFrameEnvelope {
            frame_len: 0,
            header_len: 1,
            payload_len: 1,
            range_invalid: false
        }));
    }
}
