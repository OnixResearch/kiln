//! Exact finite identifier/namespace primitive for NamespaceScope.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NamespaceScope { pub scope_id: u8, pub parent_valid: bool, pub admitted: bool }

// r[depends domain-primitives.identifier-namespace.namespace]
pub open spec fn id_namespace_scope_admitted_spec(x: NamespaceScope) -> bool { x.scope_id > 0 && x.parent_valid && x.admitted }

// r[impl domain-primitives.identifier-namespace.namespace]
pub fn id_namespace_scope_admitted(x: NamespaceScope) -> (result: bool)
    ensures result == id_namespace_scope_admitted_spec(x)
{
    x.scope_id > 0 && x.parent_valid && x.admitted
}

// r[verify domain-primitives.identifier-namespace.namespace]
pub proof fn admitted_id_namespace_scope_has_required_metadata(x: NamespaceScope)
    requires id_namespace_scope_admitted_spec(x)
    ensures x.admitted, x.parent_valid
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn id_namespace_scope_accepts_and_rejects() {
        assert!(id_namespace_scope_admitted(NamespaceScope {
            scope_id: 1,
            parent_valid: true,
            admitted: true
        }));
        assert!(!id_namespace_scope_admitted(NamespaceScope {
            scope_id: 1,
            parent_valid: false,
            admitted: true
        }));
    }
}
