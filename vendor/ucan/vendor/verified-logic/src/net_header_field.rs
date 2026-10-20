//! Exact finite network-protocol primitive for HeaderFieldMetadata.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct HeaderFieldMetadata { pub field_id: u8, pub field_len: u8, pub required_present: bool, pub duplicate_forbidden: bool }

// r[depends domain-primitives.network-protocol.header-field]
pub open spec fn net_header_field_admitted_spec(x: HeaderFieldMetadata) -> bool { x.field_id > 0 && x.field_len > 0 && x.required_present && !x.duplicate_forbidden }

// r[impl domain-primitives.network-protocol.header-field]
pub fn net_header_field_admitted(x: HeaderFieldMetadata) -> (result: bool)
    ensures result == net_header_field_admitted_spec(x)
{
    x.field_id > 0 && x.field_len > 0 && x.required_present && !x.duplicate_forbidden
}

// r[verify domain-primitives.network-protocol.header-field]
pub proof fn admitted_net_header_field_has_required_metadata(x: HeaderFieldMetadata)
    requires net_header_field_admitted_spec(x)
    ensures x.field_id > 0, x.field_len > 0, x.required_present
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn net_header_field_accepts_and_rejects() {
        assert!(net_header_field_admitted(HeaderFieldMetadata {
            field_id: 1,
            field_len: 1,
            required_present: true,
            duplicate_forbidden: false
        }));
        assert!(!net_header_field_admitted(HeaderFieldMetadata {
            field_id: 0,
            field_len: 1,
            required_present: true,
            duplicate_forbidden: false
        }));
    }
}
