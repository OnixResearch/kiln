//! Exact finite network-protocol primitive for EndpointAddressMetadata.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EndpointAddressMetadata { pub family: u8, pub host: u8, pub port: u8, pub unknown_family: bool }

// r[depends domain-primitives.network-protocol.endpoint-address]
pub open spec fn net_endpoint_address_admitted_spec(x: EndpointAddressMetadata) -> bool { x.family > 0 && x.host > 0 && x.port > 0 && !x.unknown_family }

// r[impl domain-primitives.network-protocol.endpoint-address]
pub fn net_endpoint_address_admitted(x: EndpointAddressMetadata) -> (result: bool)
    ensures result == net_endpoint_address_admitted_spec(x)
{
    x.family > 0 && x.host > 0 && x.port > 0 && !x.unknown_family
}

// r[verify domain-primitives.network-protocol.endpoint-address]
pub proof fn admitted_net_endpoint_address_has_required_metadata(x: EndpointAddressMetadata)
    requires net_endpoint_address_admitted_spec(x)
    ensures x.family > 0, x.host > 0, x.port > 0
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn net_endpoint_address_accepts_and_rejects() {
        assert!(net_endpoint_address_admitted(EndpointAddressMetadata {
            family: 1,
            host: 1,
            port: 1,
            unknown_family: false
        }));
        assert!(!net_endpoint_address_admitted(EndpointAddressMetadata {
            family: 0,
            host: 1,
            port: 1,
            unknown_family: false
        }));
    }
}
