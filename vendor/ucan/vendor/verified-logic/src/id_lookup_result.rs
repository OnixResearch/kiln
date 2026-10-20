//! Exact finite identifier/namespace primitive for LookupResult.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LookupResult { pub identifier_admitted: bool, pub namespace_admitted: bool, pub result_code: u8 }

// r[depends domain-primitives.identifier-namespace.lookup]
pub open spec fn id_lookup_result_admitted_spec(x: LookupResult) -> bool { x.identifier_admitted && x.namespace_admitted && x.result_code <= 3 }

// r[impl domain-primitives.identifier-namespace.lookup]
pub fn id_lookup_result_admitted(x: LookupResult) -> (result: bool)
    ensures result == id_lookup_result_admitted_spec(x)
{
    x.identifier_admitted && x.namespace_admitted && x.result_code <= 3
}

// r[verify domain-primitives.identifier-namespace.lookup]
pub proof fn admitted_id_lookup_result_has_required_metadata(x: LookupResult)
    requires id_lookup_result_admitted_spec(x)
    ensures x.identifier_admitted, x.namespace_admitted
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn id_lookup_result_accepts_and_rejects() {
        assert!(id_lookup_result_admitted(LookupResult {
            identifier_admitted: true,
            namespace_admitted: true,
            result_code: 1
        }));
        assert!(!id_lookup_result_admitted(LookupResult {
            identifier_admitted: false,
            namespace_admitted: true,
            result_code: 1
        }));
    }
}
