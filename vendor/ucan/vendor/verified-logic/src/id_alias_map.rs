//! Exact finite identifier/namespace primitive for AliasMap.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AliasMap { pub alias_valid: bool, pub target_admitted: bool, pub cycle: bool }

// r[depends domain-primitives.identifier-namespace.alias-map]
pub open spec fn id_alias_map_admitted_spec(x: AliasMap) -> bool { x.alias_valid && x.target_admitted && !x.cycle }

// r[impl domain-primitives.identifier-namespace.alias-map]
pub fn id_alias_map_admitted(x: AliasMap) -> (result: bool)
    ensures result == id_alias_map_admitted_spec(x)
{
    x.alias_valid && x.target_admitted && !x.cycle
}

// r[verify domain-primitives.identifier-namespace.alias-map]
pub proof fn admitted_id_alias_map_has_required_metadata(x: AliasMap)
    requires id_alias_map_admitted_spec(x)
    ensures x.target_admitted, !x.cycle
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn id_alias_map_accepts_and_rejects() {
        assert!(id_alias_map_admitted(AliasMap {
            alias_valid: true,
            target_admitted: true,
            cycle: false
        }));
        assert!(!id_alias_map_admitted(AliasMap {
            alias_valid: false,
            target_admitted: true,
            cycle: false
        }));
    }
}
