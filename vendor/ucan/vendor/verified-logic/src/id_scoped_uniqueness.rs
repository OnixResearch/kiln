//! Exact finite identifier/namespace primitive for ScopedUniqueness.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ScopedUniqueness { pub scope_valid: bool, pub duplicate_in_scope: bool, pub registry_valid: bool }

// r[depends domain-primitives.identifier-namespace.scoped-uniqueness]
pub open spec fn id_scoped_uniqueness_admitted_spec(x: ScopedUniqueness) -> bool { x.scope_valid && x.registry_valid && !x.duplicate_in_scope }

// r[impl domain-primitives.identifier-namespace.scoped-uniqueness]
pub fn id_scoped_uniqueness_admitted(x: ScopedUniqueness) -> (result: bool)
    ensures result == id_scoped_uniqueness_admitted_spec(x)
{
    x.scope_valid && x.registry_valid && !x.duplicate_in_scope
}

// r[verify domain-primitives.identifier-namespace.scoped-uniqueness]
pub proof fn admitted_id_scoped_uniqueness_has_required_metadata(x: ScopedUniqueness)
    requires id_scoped_uniqueness_admitted_spec(x)
    ensures !x.duplicate_in_scope, x.registry_valid
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn id_scoped_uniqueness_accepts_and_rejects() {
        assert!(id_scoped_uniqueness_admitted(ScopedUniqueness {
            scope_valid: true,
            duplicate_in_scope: false,
            registry_valid: true
        }));
        assert!(!id_scoped_uniqueness_admitted(ScopedUniqueness {
            scope_valid: false,
            duplicate_in_scope: false,
            registry_valid: true
        }));
    }
}
