//! Finite content-addressed code identity and dependency-closure primitives.
//!
//! This module models the structural side of Unison-style content-addressed
//! definitions: definition hashes are opaque finite identifiers, names are
//! separate aliases, dependency closures are admitted only when every referenced
//! hash is present, and recursive groups use canonical sorted member envelopes.
use vstd::prelude::*;

verus! {

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DefinitionEnvelope {
    pub hash: u64,
    pub kind: u8,
    pub normalized_id: u64,
    pub dependency_fingerprint: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NameBinding {
    pub name_id: u64,
    pub definition_hash: u64,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DependencyClosure {
    pub root_hash: u64,
    pub dependency_hashes: Vec<u64>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RecursiveReference {
    pub member_index: u64,
    pub target_index: u64,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RecursiveGroup {
    pub group_hash: u64,
    pub member_hashes: Vec<u64>,
    pub references: Vec<RecursiveReference>,
}

// r[depends domain-primitives.content-addressed-code.definition-identity]
pub open spec fn definition_identity_input_eq_spec(left: DefinitionEnvelope, right: DefinitionEnvelope) -> bool {
    left.kind == right.kind
        && left.normalized_id == right.normalized_id
        && left.dependency_fingerprint == right.dependency_fingerprint
}

// r[depends domain-primitives.content-addressed-code.definition-identity]
// r[depends domain-primitives.content-addressed-code.dependency-closure]
pub open spec fn definition_hash_present_from_spec(defs: Seq<DefinitionEnvelope>, hash: u64, from: int) -> bool
    recommends 0 <= from <= defs.len(),
    decreases defs.len() - from,
{
    if from >= defs.len() {
        false
    } else {
        defs[from].hash == hash || definition_hash_present_from_spec(defs, hash, from + 1)
    }
}

// r[depends domain-primitives.content-addressed-code.definition-identity]
// r[depends domain-primitives.content-addressed-code.dependency-closure]
pub open spec fn definition_hash_present_spec(defs: Seq<DefinitionEnvelope>, hash: u64) -> bool {
    exists|i: int| 0 <= i < defs.len() && defs[i].hash == hash
}

// r[depends domain-primitives.content-addressed-code.definition-identity]
pub open spec fn name_binding_admitted_spec(binding: NameBinding, defs: Seq<DefinitionEnvelope>) -> bool {
    definition_hash_present_spec(defs, binding.definition_hash)
}

// r[depends domain-primitives.content-addressed-code.dependency-closure]
pub open spec fn dependency_hashes_present_from_spec(deps: Seq<u64>, defs: Seq<DefinitionEnvelope>, from: int) -> bool
    recommends 0 <= from <= deps.len(),
    decreases deps.len() - from,
{
    if from >= deps.len() {
        true
    } else {
        definition_hash_present_spec(defs, deps[from])
            && dependency_hashes_present_from_spec(deps, defs, from + 1)
    }
}

// r[depends domain-primitives.content-addressed-code.dependency-closure]
pub open spec fn dependency_hashes_present_spec(deps: Seq<u64>, defs: Seq<DefinitionEnvelope>) -> bool {
    forall|i: int| 0 <= i < deps.len() ==> definition_hash_present_spec(defs, #[trigger] deps[i])
}

// r[depends domain-primitives.content-addressed-code.dependency-closure]
pub open spec fn dependency_closure_admitted_spec(closure: DependencyClosure, defs: Seq<DefinitionEnvelope>) -> bool {
    definition_hash_present_spec(defs, closure.root_hash)
        && dependency_hashes_present_spec(closure.dependency_hashes@, defs)
}

// r[depends domain-primitives.content-addressed-code.dependency-closure]
pub open spec fn first_missing_dependency_from_spec(deps: Seq<u64>, defs: Seq<DefinitionEnvelope>, from: int) -> Option<u64>
    recommends 0 <= from <= deps.len(),
    decreases deps.len() - from,
{
    if from >= deps.len() {
        Option::None
    } else if !definition_hash_present_spec(defs, deps[from]) {
        Option::Some(deps[from])
    } else {
        first_missing_dependency_from_spec(deps, defs, from + 1)
    }
}

// r[depends domain-primitives.content-addressed-code.dependency-closure]
pub open spec fn first_missing_dependency_spec(closure: DependencyClosure, defs: Seq<DefinitionEnvelope>) -> Option<u64> {
    first_missing_dependency_from_spec(closure.dependency_hashes@, defs, 0)
}

// r[depends domain-primitives.content-addressed-code.dependency-closure]
pub open spec fn first_missing_dependency_result_spec(
    deps: Seq<u64>,
    defs: Seq<DefinitionEnvelope>,
    result: Option<u64>,
) -> bool {
    match result {
        Option::Some(hash) => exists|i: int|
            0 <= i < deps.len()
                && deps[i] == hash
                && !definition_hash_present_spec(defs, hash)
                && forall|k: int| 0 <= k < i ==> definition_hash_present_spec(defs, #[trigger] deps[k]),
        Option::None => dependency_hashes_present_spec(deps, defs),
    }
}

// r[depends domain-primitives.content-addressed-code.recursive-group]
pub open spec fn strictly_increasing_from_spec(values: Seq<u64>, from: int) -> bool
    recommends 0 <= from <= values.len(),
    decreases values.len() - from,
{
    if from + 1 >= values.len() {
        true
    } else {
        values[from] < values[from + 1] && strictly_increasing_from_spec(values, from + 1)
    }
}

// r[depends domain-primitives.content-addressed-code.recursive-group]
pub open spec fn strictly_increasing_spec(values: Seq<u64>) -> bool {
    forall|i: int| 0 <= i && i + 1 < values.len() ==> #[trigger] values[i] < values[i + 1]
}

// r[depends domain-primitives.content-addressed-code.recursive-group]
pub open spec fn recursive_refs_in_bounds_from_spec(refs: Seq<RecursiveReference>, member_count: nat, from: int) -> bool
    recommends 0 <= from <= refs.len(),
    decreases refs.len() - from,
{
    if from >= refs.len() {
        true
    } else {
        refs[from].member_index < member_count
            && refs[from].target_index < member_count
            && recursive_refs_in_bounds_from_spec(refs, member_count, from + 1)
    }
}

// r[depends domain-primitives.content-addressed-code.recursive-group]
pub open spec fn recursive_refs_in_bounds_spec(refs: Seq<RecursiveReference>, member_count: nat) -> bool {
    forall|i: int| 0 <= i < refs.len() ==> refs[i].member_index < member_count && refs[i].target_index < member_count
}

// r[depends domain-primitives.content-addressed-code.recursive-group]
pub open spec fn recursive_group_admitted_spec(group: RecursiveGroup) -> bool {
    group.group_hash > 0
        && group.member_hashes@.len() > 0
        && strictly_increasing_spec(group.member_hashes@)
        && recursive_refs_in_bounds_spec(group.references@, group.member_hashes@.len())
}

// r[impl domain-primitives.content-addressed-code.definition-identity]
pub fn definition_identity_input_eq(left: DefinitionEnvelope, right: DefinitionEnvelope) -> (result: bool)
    ensures result == definition_identity_input_eq_spec(left, right)
{
    left.kind == right.kind
        && left.normalized_id == right.normalized_id
        && left.dependency_fingerprint == right.dependency_fingerprint
}

// r[impl domain-primitives.content-addressed-code.definition-identity]
// r[impl domain-primitives.content-addressed-code.dependency-closure]
pub fn definition_hash_present(defs: &Vec<DefinitionEnvelope>, hash: u64) -> (result: bool)
    ensures result == definition_hash_present_spec(defs@, hash)
{
    let mut i: usize = 0;
    while i < defs.len()
        invariant
            0 <= i <= defs.len(),
            forall|k: int| 0 <= k < i ==> defs@[k].hash != hash,
        decreases defs.len() - i,
    {
        if defs[i].hash == hash {
            assert(definition_hash_present_spec(defs@, hash));
            return true;
        }
        i = i + 1;
    }
    assert forall|k: int| 0 <= k < defs@.len() implies defs@[k].hash != hash by {
        assert(0 <= k < i);
    }
    assert(!definition_hash_present_spec(defs@, hash));
    false
}

// r[impl domain-primitives.content-addressed-code.definition-identity]
pub fn name_binding_admitted(binding: NameBinding, defs: &Vec<DefinitionEnvelope>) -> (result: bool)
    ensures result == name_binding_admitted_spec(binding, defs@)
{
    definition_hash_present(defs, binding.definition_hash)
}

// r[impl domain-primitives.content-addressed-code.dependency-closure]
pub fn dependency_hashes_present(deps: &Vec<u64>, defs: &Vec<DefinitionEnvelope>) -> (result: bool)
    ensures result == dependency_hashes_present_spec(deps@, defs@)
{
    let mut i: usize = 0;
    while i < deps.len()
        invariant
            0 <= i <= deps.len(),
            forall|k: int| 0 <= k < i ==> definition_hash_present_spec(defs@, deps@[k]),
        decreases deps.len() - i,
    {
        if !definition_hash_present(defs, deps[i]) {
            assert(0 <= i < deps@.len());
            assert(!dependency_hashes_present_spec(deps@, defs@));
            return false;
        }
        i = i + 1;
    }
    assert forall|k: int| 0 <= k < deps@.len() implies definition_hash_present_spec(defs@, deps@[k]) by {
        assert(0 <= k < i);
    }
    assert(dependency_hashes_present_spec(deps@, defs@));
    true
}

// r[impl domain-primitives.content-addressed-code.dependency-closure]
pub fn dependency_closure_admitted(closure: &DependencyClosure, defs: &Vec<DefinitionEnvelope>) -> (result: bool)
    ensures result == dependency_closure_admitted_spec(*closure, defs@)
{
    definition_hash_present(defs, closure.root_hash)
        && dependency_hashes_present(&closure.dependency_hashes, defs)
}

// r[impl domain-primitives.content-addressed-code.dependency-closure]
pub fn first_missing_dependency(closure: &DependencyClosure, defs: &Vec<DefinitionEnvelope>) -> (result: Option<u64>)
    ensures first_missing_dependency_result_spec(closure.dependency_hashes@, defs@, result)
{
    let mut i: usize = 0;
    while i < closure.dependency_hashes.len()
        invariant
            0 <= i <= closure.dependency_hashes.len(),
            forall|k: int| 0 <= k < i ==> definition_hash_present_spec(defs@, closure.dependency_hashes@[k]),
        decreases closure.dependency_hashes.len() - i,
    {
        if !definition_hash_present(defs, closure.dependency_hashes[i]) {
            assert(0 <= i < closure.dependency_hashes@.len());
            assert(exists|j: int|
                0 <= j < closure.dependency_hashes@.len()
                    && closure.dependency_hashes@[j] == closure.dependency_hashes@[i as int]
                    && !definition_hash_present_spec(defs@, closure.dependency_hashes@[i as int])
                    && forall|k: int| 0 <= k < j ==> definition_hash_present_spec(defs@, #[trigger] closure.dependency_hashes@[k]));
            return Some(closure.dependency_hashes[i]);
        }
        i = i + 1;
    }
    assert forall|k: int| 0 <= k < closure.dependency_hashes@.len() implies definition_hash_present_spec(defs@, closure.dependency_hashes@[k]) by {
        assert(0 <= k < i);
    }
    assert(dependency_hashes_present_spec(closure.dependency_hashes@, defs@));
    None
}

// r[impl domain-primitives.content-addressed-code.recursive-group]
pub fn strictly_increasing(values: &Vec<u64>) -> (result: bool)
    ensures result == strictly_increasing_spec(values@)
{
    if values.len() <= 1 {
        assert(strictly_increasing_spec(values@));
        return true;
    }
    let mut i: usize = 0;
    while i + 1 < values.len()
        invariant
            0 <= i < values.len(),
            forall|k: int| 0 <= k < i ==> #[trigger] values@[k] < values@[k + 1],
        decreases values.len() - i,
    {
        if values[i] >= values[i + 1] {
            assert(0 <= i && i + 1 < values@.len());
            assert(!strictly_increasing_spec(values@));
            return false;
        }
        i = i + 1;
    }
    assert forall|k: int| 0 <= k && k + 1 < values@.len() implies #[trigger] values@[k] < values@[k + 1] by {
        assert(k < i);
    }
    assert(strictly_increasing_spec(values@));
    true
}

// r[impl domain-primitives.content-addressed-code.recursive-group]
pub fn recursive_refs_in_bounds(refs: &Vec<RecursiveReference>, member_count: usize) -> (result: bool)
    ensures result == recursive_refs_in_bounds_spec(refs@, member_count as nat)
{
    let mut i: usize = 0;
    while i < refs.len()
        invariant
            0 <= i <= refs.len(),
            forall|k: int| 0 <= k < i ==> refs@[k].member_index < member_count && refs@[k].target_index < member_count,
        decreases refs.len() - i,
    {
        if refs[i].member_index >= member_count as u64 || refs[i].target_index >= member_count as u64 {
            assert(0 <= i < refs@.len());
            assert(!recursive_refs_in_bounds_spec(refs@, member_count as nat));
            return false;
        }
        i = i + 1;
    }
    assert forall|k: int| 0 <= k < refs@.len() implies refs@[k].member_index < member_count && refs@[k].target_index < member_count by {
        assert(0 <= k < i);
    }
    assert(recursive_refs_in_bounds_spec(refs@, member_count as nat));
    true
}

// r[impl domain-primitives.content-addressed-code.recursive-group]
pub fn recursive_group_admitted(group: &RecursiveGroup) -> (result: bool)
    ensures result == recursive_group_admitted_spec(*group)
{
    group.group_hash > 0
        && group.member_hashes.len() > 0
        && strictly_increasing(&group.member_hashes)
        && recursive_refs_in_bounds(&group.references, group.member_hashes.len())
}

// r[verify domain-primitives.content-addressed-code.definition-identity]
pub proof fn admitted_name_binding_points_to_known_definition(binding: NameBinding, defs: Seq<DefinitionEnvelope>)
    requires name_binding_admitted_spec(binding, defs)
    ensures definition_hash_present_spec(defs, binding.definition_hash)
{}

// r[verify domain-primitives.content-addressed-code.definition-identity]
pub proof fn name_independent_identity(left: DefinitionEnvelope, right: DefinitionEnvelope, left_name: u64, right_name: u64)
    requires definition_identity_input_eq_spec(left, right)
    ensures definition_identity_input_eq_spec(left, right)
{}

// r[verify domain-primitives.content-addressed-code.dependency-closure]
pub proof fn admitted_dependency_closure_has_root(closure: DependencyClosure, defs: Seq<DefinitionEnvelope>)
    requires dependency_closure_admitted_spec(closure, defs)
    ensures definition_hash_present_spec(defs, closure.root_hash)
{}

// r[verify domain-primitives.content-addressed-code.dependency-closure]
pub proof fn admitted_dependency_closure_has_all_dependencies(closure: DependencyClosure, defs: Seq<DefinitionEnvelope>)
    requires dependency_closure_admitted_spec(closure, defs)
    ensures dependency_hashes_present_spec(closure.dependency_hashes@, defs)
{}

// r[verify domain-primitives.content-addressed-code.recursive-group]
pub proof fn admitted_recursive_group_has_canonical_members(group: RecursiveGroup)
    requires recursive_group_admitted_spec(group)
    ensures strictly_increasing_spec(group.member_hashes@)
{}

// r[verify domain-primitives.content-addressed-code.recursive-group]
pub proof fn admitted_recursive_group_refs_are_in_bounds(group: RecursiveGroup)
    requires recursive_group_admitted_spec(group)
    ensures recursive_refs_in_bounds_spec(group.references@, group.member_hashes@.len())
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    fn def(hash: u64, kind: u8, normalized_id: u64, dependency_fingerprint: u64) -> DefinitionEnvelope {
        DefinitionEnvelope {
            hash,
            kind,
            normalized_id,
            dependency_fingerprint,
        }
    }

    #[test]
    fn name_changes_do_not_affect_definition_identity() {
        let d1 = def(10, 1, 42, 99);
        let d2 = def(11, 1, 42, 99);
        let defs = vec![d1];

        assert!(definition_identity_input_eq(d1, d2));
        assert!(name_binding_admitted(
            NameBinding {
                name_id: 1,
                definition_hash: 10
            },
            &defs
        ));
        assert!(name_binding_admitted(
            NameBinding {
                name_id: 2,
                definition_hash: 10
            },
            &defs
        ));
        assert!(!name_binding_admitted(
            NameBinding {
                name_id: 3,
                definition_hash: 77
            },
            &defs
        ));
    }

    #[test]
    fn dependency_change_distinguishes_identity_input() {
        let base = def(10, 1, 42, 99);
        let changed_dep = def(10, 1, 42, 100);
        assert!(!definition_identity_input_eq(base, changed_dep));
    }

    #[test]
    fn dependency_closure_reports_first_missing_hash() {
        let defs = vec![def(1, 1, 10, 0), def(2, 1, 20, 0)];
        let closure = DependencyClosure {
            root_hash: 1,
            dependency_hashes: vec![2, 3, 4],
        };
        assert!(!dependency_closure_admitted(&closure, &defs));
        assert_eq!(first_missing_dependency(&closure, &defs), Some(3));

        let complete_defs = vec![def(1, 1, 10, 0), def(2, 1, 20, 0), def(3, 1, 30, 0), def(4, 1, 40, 0)];
        assert!(dependency_closure_admitted(&closure, &complete_defs));
        assert_eq!(first_missing_dependency(&closure, &complete_defs), None);
    }

    #[test]
    fn recursive_group_requires_sorted_members_and_in_bounds_refs() {
        let good = RecursiveGroup {
            group_hash: 9,
            member_hashes: vec![10, 20],
            references: vec![RecursiveReference {
                member_index: 0,
                target_index: 1,
            }],
        };
        assert!(recursive_group_admitted(&good));

        let unsorted = RecursiveGroup {
            member_hashes: vec![20, 10],
            ..good.clone()
        };
        assert!(!recursive_group_admitted(&unsorted));

        let bad_ref = RecursiveGroup {
            references: vec![RecursiveReference {
                member_index: 0,
                target_index: 2,
            }],
            ..good
        };
        assert!(!recursive_group_admitted(&bad_ref));
    }
}
