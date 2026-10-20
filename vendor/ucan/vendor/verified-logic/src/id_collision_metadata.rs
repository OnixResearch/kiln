//! Exact finite identifier/namespace primitive for CollisionMetadata.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CollisionMetadata {
    pub registry_valid: bool,
    pub scope_valid: bool,
    pub collision_code: u8,
    pub deterministic: bool,
}

// r[depends domain-primitives.identifier-namespace.collision]
pub open spec fn id_collision_metadata_admitted_spec(x: CollisionMetadata) -> bool {
    x.registry_valid && x.scope_valid && x.collision_code <= 1 && x.deterministic
}

// r[impl domain-primitives.identifier-namespace.collision]
pub fn id_collision_metadata_admitted(x: CollisionMetadata) -> (result: bool)
    ensures result == id_collision_metadata_admitted_spec(x)
{
    x.registry_valid && x.scope_valid && x.collision_code <= 1 && x.deterministic
}

// r[verify domain-primitives.identifier-namespace.collision]
pub proof fn admitted_id_collision_metadata_has_required_metadata(x: CollisionMetadata)
    requires id_collision_metadata_admitted_spec(x)
    ensures x.deterministic, x.scope_valid, x.collision_code <= 1
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn id_collision_metadata_accepts_and_rejects() {
        assert!(id_collision_metadata_admitted(CollisionMetadata {
            registry_valid: true,
            scope_valid: true,
            collision_code: 0,
            deterministic: true
        }));
        assert!(!id_collision_metadata_admitted(CollisionMetadata {
            registry_valid: false,
            scope_valid: true,
            collision_code: 0,
            deterministic: true
        }));
    }
}
