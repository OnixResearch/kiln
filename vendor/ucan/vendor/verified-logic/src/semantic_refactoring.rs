//! Finite semantic refactoring session primitives.
//!
//! This module models the structural admission side of Unison-style semantic
//! refactoring: names move between immutable definition identities, declared
//! compatibility admits dependent rewrites, and incompatible dependents remain
//! explicit in a finite unresolved set. It does not claim real parser, compiler,
//! type-inference, source-rewrite, IDE, storage, or merge semantics.
use vstd::prelude::*;

verus! {

pub const REFACTOR_ADMITTED: u8 = 0;
pub const REFACTOR_MISSING_OLD_DEFINITION: u8 = 1;
pub const REFACTOR_MISSING_NEW_DEFINITION: u8 = 2;
pub const REFACTOR_MISSING_OLD_BINDING: u8 = 3;
pub const REFACTOR_MISSING_NEW_BINDING: u8 = 4;
pub const REFACTOR_DANGLING_NAME: u8 = 5;
pub const REFACTOR_MISSING_UNRESOLVED_DEPENDENT: u8 = 6;

pub const DEPENDENT_UNAFFECTED: u8 = 0;
pub const DEPENDENT_REWRITTEN: u8 = 1;
pub const DEPENDENT_UNRESOLVED: u8 = 2;
pub const DEPENDENT_MISSING_UNRESOLVED: u8 = 3;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DefinitionMeta {
    pub hash: u64,
    pub type_hash: u64,
    pub interface_hash: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NameBinding {
    pub name_hash: u64,
    pub definition_hash: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ReplacementCompatibility {
    pub old_hash: u64,
    pub new_hash: u64,
    pub type_compatible: bool,
    pub interface_compatible: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DependentEdge {
    pub dependent_hash: u64,
    pub referenced_hash: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RefactorStep {
    pub name_hash: u64,
    pub old_hash: u64,
    pub new_hash: u64,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RefactorSession {
    pub definitions: Vec<DefinitionMeta>,
    pub before_bindings: Vec<NameBinding>,
    pub after_bindings: Vec<NameBinding>,
    pub compatibilities: Vec<ReplacementCompatibility>,
    pub dependents: Vec<DependentEdge>,
    pub unresolved_dependents: Vec<u64>,
    pub step: RefactorStep,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RefactorAdmission {
    pub state: u8,
    pub detail: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DependentPropagation {
    pub state: u8,
    pub dependent_hash: u64,
}

// r[depends domain-primitives.semantic-refactoring.immutable-binding]
pub open spec fn definition_present_spec(definitions: Seq<DefinitionMeta>, hash: u64) -> bool {
    exists|i: int| 0 <= i < definitions.len() && definitions[i].hash == hash
}

// r[depends domain-primitives.semantic-refactoring.immutable-binding]
pub open spec fn binding_present_spec(bindings: Seq<NameBinding>, name_hash: u64, definition_hash: u64) -> bool {
    exists|i: int| 0 <= i < bindings.len() && bindings[i].name_hash == name_hash && bindings[i].definition_hash == definition_hash
}

// r[depends domain-primitives.semantic-refactoring.session-validity]
pub open spec fn binding_valid_spec(binding: NameBinding, definitions: Seq<DefinitionMeta>) -> bool {
    binding.name_hash > 0 && definition_present_spec(definitions, binding.definition_hash)
}

// r[depends domain-primitives.semantic-refactoring.session-validity]
pub open spec fn bindings_valid_spec(bindings: Seq<NameBinding>, definitions: Seq<DefinitionMeta>) -> bool {
    forall|i: int| 0 <= i < bindings.len() ==> binding_valid_spec(#[trigger] bindings[i], definitions)
}

// r[depends domain-primitives.semantic-refactoring.compatible-propagation]
pub open spec fn compatibility_admits_spec(
    compatibilities: Seq<ReplacementCompatibility>,
    old_hash: u64,
    new_hash: u64,
) -> bool {
    exists|i: int| 0 <= i < compatibilities.len()
        && compatibilities[i].old_hash == old_hash
        && compatibilities[i].new_hash == new_hash
        && compatibilities[i].type_compatible
        && compatibilities[i].interface_compatible
}

// r[depends domain-primitives.semantic-refactoring.compatible-propagation]
pub open spec fn value_present_spec(values: Seq<u64>, value: u64) -> bool {
    exists|i: int| 0 <= i < values.len() && values[i] == value
}

// r[depends domain-primitives.semantic-refactoring.compatible-propagation]
pub open spec fn dependent_propagation_spec(
    dependent: DependentEdge,
    step: RefactorStep,
    compatibilities: Seq<ReplacementCompatibility>,
    unresolved: Seq<u64>,
) -> DependentPropagation {
    if dependent.referenced_hash != step.old_hash {
        DependentPropagation { state: DEPENDENT_UNAFFECTED, dependent_hash: dependent.dependent_hash }
    } else if compatibility_admits_spec(compatibilities, step.old_hash, step.new_hash) {
        DependentPropagation { state: DEPENDENT_REWRITTEN, dependent_hash: dependent.dependent_hash }
    } else if value_present_spec(unresolved, dependent.dependent_hash) {
        DependentPropagation { state: DEPENDENT_UNRESOLVED, dependent_hash: dependent.dependent_hash }
    } else {
        DependentPropagation { state: DEPENDENT_MISSING_UNRESOLVED, dependent_hash: dependent.dependent_hash }
    }
}

// r[depends domain-primitives.semantic-refactoring.session-validity]
pub open spec fn first_missing_unresolved_from_spec(
    dependents: Seq<DependentEdge>,
    step: RefactorStep,
    compatibilities: Seq<ReplacementCompatibility>,
    unresolved: Seq<u64>,
    from: int,
) -> Option<u64>
    recommends 0 <= from <= dependents.len(),
    decreases dependents.len() - from,
{
    if from >= dependents.len() {
        Option::None
    } else {
        let propagation = dependent_propagation_spec(dependents[from], step, compatibilities, unresolved);
        if propagation.state == DEPENDENT_MISSING_UNRESOLVED {
            Option::Some(propagation.dependent_hash)
        } else {
            first_missing_unresolved_from_spec(dependents, step, compatibilities, unresolved, from + 1)
        }
    }
}

// r[depends domain-primitives.semantic-refactoring.session-validity]
pub open spec fn first_missing_unresolved_spec(session: RefactorSession) -> Option<u64> {
    first_missing_unresolved_from_spec(session.dependents@, session.step, session.compatibilities@, session.unresolved_dependents@, 0)
}

// r[depends domain-primitives.semantic-refactoring.immutable-binding]
// r[depends domain-primitives.semantic-refactoring.compatible-propagation]
// r[depends domain-primitives.semantic-refactoring.session-validity]
pub open spec fn refactor_admission_spec(session: RefactorSession) -> RefactorAdmission {
    if !definition_present_spec(session.definitions@, session.step.old_hash) {
        RefactorAdmission { state: REFACTOR_MISSING_OLD_DEFINITION, detail: session.step.old_hash }
    } else if !definition_present_spec(session.definitions@, session.step.new_hash) {
        RefactorAdmission { state: REFACTOR_MISSING_NEW_DEFINITION, detail: session.step.new_hash }
    } else if !binding_present_spec(session.before_bindings@, session.step.name_hash, session.step.old_hash) {
        RefactorAdmission { state: REFACTOR_MISSING_OLD_BINDING, detail: session.step.name_hash }
    } else if !binding_present_spec(session.after_bindings@, session.step.name_hash, session.step.new_hash) {
        RefactorAdmission { state: REFACTOR_MISSING_NEW_BINDING, detail: session.step.name_hash }
    } else if !bindings_valid_spec(session.after_bindings@, session.definitions@) {
        RefactorAdmission { state: REFACTOR_DANGLING_NAME, detail: session.step.name_hash }
    } else {
        match first_missing_unresolved_spec(session) {
            Option::Some(dependent) => RefactorAdmission { state: REFACTOR_MISSING_UNRESOLVED_DEPENDENT, detail: dependent },
            Option::None => RefactorAdmission { state: REFACTOR_ADMITTED, detail: session.step.new_hash },
        }
    }
}

// r[depends domain-primitives.semantic-refactoring.session-validity]
pub open spec fn refactor_admitted_spec(admission: RefactorAdmission) -> bool {
    admission.state == REFACTOR_ADMITTED
}

// r[impl domain-primitives.semantic-refactoring.immutable-binding]
pub fn definition_present(definitions: &Vec<DefinitionMeta>, hash: u64) -> (result: bool)
    ensures result == definition_present_spec(definitions@, hash)
{
    let mut i: usize = 0;
    while i < definitions.len()
        invariant
            0 <= i <= definitions.len(),
            forall|k: int| 0 <= k < i ==> definitions@[k].hash != hash,
        decreases definitions.len() - i,
    {
        if definitions[i].hash == hash {
            assert(definition_present_spec(definitions@, hash));
            return true;
        }
        i = i + 1;
    }
    assert forall|k: int| 0 <= k < definitions@.len() implies definitions@[k].hash != hash by {
        assert(0 <= k < i);
    }
    assert(!definition_present_spec(definitions@, hash));
    false
}

// r[impl domain-primitives.semantic-refactoring.immutable-binding]
pub fn binding_present(bindings: &Vec<NameBinding>, name_hash: u64, definition_hash: u64) -> (result: bool)
    ensures result == binding_present_spec(bindings@, name_hash, definition_hash)
{
    let mut i: usize = 0;
    while i < bindings.len()
        invariant
            0 <= i <= bindings.len(),
            forall|k: int| 0 <= k < i ==> !(bindings@[k].name_hash == name_hash && bindings@[k].definition_hash == definition_hash),
        decreases bindings.len() - i,
    {
        if bindings[i].name_hash == name_hash && bindings[i].definition_hash == definition_hash {
            assert(binding_present_spec(bindings@, name_hash, definition_hash));
            return true;
        }
        i = i + 1;
    }
    assert forall|k: int| 0 <= k < bindings@.len() implies !(bindings@[k].name_hash == name_hash && bindings@[k].definition_hash == definition_hash) by {
        assert(0 <= k < i);
    }
    assert(!binding_present_spec(bindings@, name_hash, definition_hash));
    false
}

// r[impl domain-primitives.semantic-refactoring.session-validity]
pub fn binding_valid(binding: &NameBinding, definitions: &Vec<DefinitionMeta>) -> (result: bool)
    ensures result == binding_valid_spec(*binding, definitions@)
{
    binding.name_hash > 0 && definition_present(definitions, binding.definition_hash)
}

// r[impl domain-primitives.semantic-refactoring.session-validity]
pub fn bindings_valid(bindings: &Vec<NameBinding>, definitions: &Vec<DefinitionMeta>) -> (result: bool)
    ensures result == bindings_valid_spec(bindings@, definitions@)
{
    let mut i: usize = 0;
    while i < bindings.len()
        invariant
            0 <= i <= bindings.len(),
            forall|k: int| 0 <= k < i ==> binding_valid_spec(#[trigger] bindings@[k], definitions@),
        decreases bindings.len() - i,
    {
        if !binding_valid(&bindings[i], definitions) {
            assert(0 <= i < bindings@.len());
            assert(!bindings_valid_spec(bindings@, definitions@));
            return false;
        }
        i = i + 1;
    }
    assert forall|k: int| 0 <= k < bindings@.len() implies binding_valid_spec(#[trigger] bindings@[k], definitions@) by {
        assert(0 <= k < i);
    }
    assert(bindings_valid_spec(bindings@, definitions@));
    true
}

// r[impl domain-primitives.semantic-refactoring.compatible-propagation]
pub fn compatibility_admits(
    compatibilities: &Vec<ReplacementCompatibility>,
    old_hash: u64,
    new_hash: u64,
) -> (result: bool)
    ensures result == compatibility_admits_spec(compatibilities@, old_hash, new_hash)
{
    let mut i: usize = 0;
    while i < compatibilities.len()
        invariant
            0 <= i <= compatibilities.len(),
            forall|k: int| 0 <= k < i ==> !(compatibilities@[k].old_hash == old_hash
                && compatibilities@[k].new_hash == new_hash
                && compatibilities@[k].type_compatible
                && compatibilities@[k].interface_compatible),
        decreases compatibilities.len() - i,
    {
        let compatibility = compatibilities[i];
        if compatibility.old_hash == old_hash && compatibility.new_hash == new_hash
            && compatibility.type_compatible && compatibility.interface_compatible {
            assert(compatibility_admits_spec(compatibilities@, old_hash, new_hash));
            return true;
        }
        i = i + 1;
    }
    assert forall|k: int| 0 <= k < compatibilities@.len() implies !(compatibilities@[k].old_hash == old_hash
        && compatibilities@[k].new_hash == new_hash
        && compatibilities@[k].type_compatible
        && compatibilities@[k].interface_compatible) by {
        assert(0 <= k < i);
    }
    assert(!compatibility_admits_spec(compatibilities@, old_hash, new_hash));
    false
}

// r[impl domain-primitives.semantic-refactoring.compatible-propagation]
pub fn value_present(values: &Vec<u64>, value: u64) -> (result: bool)
    ensures result == value_present_spec(values@, value)
{
    let mut i: usize = 0;
    while i < values.len()
        invariant
            0 <= i <= values.len(),
            forall|k: int| 0 <= k < i ==> values@[k] != value,
        decreases values.len() - i,
    {
        if values[i] == value {
            assert(value_present_spec(values@, value));
            return true;
        }
        i = i + 1;
    }
    assert forall|k: int| 0 <= k < values@.len() implies values@[k] != value by {
        assert(0 <= k < i);
    }
    assert(!value_present_spec(values@, value));
    false
}

// r[impl domain-primitives.semantic-refactoring.compatible-propagation]
pub fn dependent_propagation(
    dependent: DependentEdge,
    step: RefactorStep,
    compatibilities: &Vec<ReplacementCompatibility>,
    unresolved: &Vec<u64>,
) -> (result: DependentPropagation)
    ensures result == dependent_propagation_spec(dependent, step, compatibilities@, unresolved@)
{
    if dependent.referenced_hash != step.old_hash {
        DependentPropagation { state: DEPENDENT_UNAFFECTED, dependent_hash: dependent.dependent_hash }
    } else if compatibility_admits(compatibilities, step.old_hash, step.new_hash) {
        DependentPropagation { state: DEPENDENT_REWRITTEN, dependent_hash: dependent.dependent_hash }
    } else if value_present(unresolved, dependent.dependent_hash) {
        DependentPropagation { state: DEPENDENT_UNRESOLVED, dependent_hash: dependent.dependent_hash }
    } else {
        DependentPropagation { state: DEPENDENT_MISSING_UNRESOLVED, dependent_hash: dependent.dependent_hash }
    }
}

// r[impl domain-primitives.semantic-refactoring.session-validity]
pub fn first_missing_unresolved(session: &RefactorSession) -> (result: Option<u64>)
    ensures result == first_missing_unresolved_spec(*session)
{
    let mut i: usize = 0;
    while i < session.dependents.len()
        invariant
            0 <= i <= session.dependents.len(),
            first_missing_unresolved_from_spec(session.dependents@, session.step, session.compatibilities@, session.unresolved_dependents@, 0)
                == first_missing_unresolved_from_spec(session.dependents@, session.step, session.compatibilities@, session.unresolved_dependents@, i as int),
        decreases session.dependents.len() - i,
    {
        let propagation = dependent_propagation(session.dependents[i], session.step, &session.compatibilities, &session.unresolved_dependents);
        if propagation.state == DEPENDENT_MISSING_UNRESOLVED {
            assert(first_missing_unresolved_from_spec(session.dependents@, session.step, session.compatibilities@, session.unresolved_dependents@, i as int)
                == Option::Some(propagation.dependent_hash));
            return Some(propagation.dependent_hash);
        }
        assert(first_missing_unresolved_from_spec(session.dependents@, session.step, session.compatibilities@, session.unresolved_dependents@, i as int)
            == first_missing_unresolved_from_spec(session.dependents@, session.step, session.compatibilities@, session.unresolved_dependents@, i as int + 1));
        i = i + 1;
    }
    assert(first_missing_unresolved_from_spec(session.dependents@, session.step, session.compatibilities@, session.unresolved_dependents@, i as int) == Option::<u64>::None);
    None
}

// r[impl domain-primitives.semantic-refactoring.immutable-binding]
// r[impl domain-primitives.semantic-refactoring.compatible-propagation]
// r[impl domain-primitives.semantic-refactoring.session-validity]
pub fn refactor_admission(session: &RefactorSession) -> (result: RefactorAdmission)
    ensures result == refactor_admission_spec(*session)
{
    if !definition_present(&session.definitions, session.step.old_hash) {
        return RefactorAdmission { state: REFACTOR_MISSING_OLD_DEFINITION, detail: session.step.old_hash };
    }
    if !definition_present(&session.definitions, session.step.new_hash) {
        return RefactorAdmission { state: REFACTOR_MISSING_NEW_DEFINITION, detail: session.step.new_hash };
    }
    if !binding_present(&session.before_bindings, session.step.name_hash, session.step.old_hash) {
        return RefactorAdmission { state: REFACTOR_MISSING_OLD_BINDING, detail: session.step.name_hash };
    }
    if !binding_present(&session.after_bindings, session.step.name_hash, session.step.new_hash) {
        return RefactorAdmission { state: REFACTOR_MISSING_NEW_BINDING, detail: session.step.name_hash };
    }
    if !bindings_valid(&session.after_bindings, &session.definitions) {
        return RefactorAdmission { state: REFACTOR_DANGLING_NAME, detail: session.step.name_hash };
    }
    match first_missing_unresolved(session) {
        Some(dependent) => RefactorAdmission { state: REFACTOR_MISSING_UNRESOLVED_DEPENDENT, detail: dependent },
        None => RefactorAdmission { state: REFACTOR_ADMITTED, detail: session.step.new_hash },
    }
}

// r[verify domain-primitives.semantic-refactoring.immutable-binding]
pub proof fn admitted_session_has_old_and_new_definitions(session: RefactorSession)
    requires refactor_admission_spec(session).state == REFACTOR_ADMITTED
    ensures
        definition_present_spec(session.definitions@, session.step.old_hash),
        definition_present_spec(session.definitions@, session.step.new_hash),
{
}

// r[verify domain-primitives.semantic-refactoring.compatible-propagation]
pub proof fn compatible_dependent_is_rewritten(
    dependent: DependentEdge,
    step: RefactorStep,
    compatibilities: Seq<ReplacementCompatibility>,
    unresolved: Seq<u64>,
)
    requires
        dependent.referenced_hash == step.old_hash,
        compatibility_admits_spec(compatibilities, step.old_hash, step.new_hash),
    ensures dependent_propagation_spec(dependent, step, compatibilities, unresolved).state == DEPENDENT_REWRITTEN
{
}

// r[verify domain-primitives.semantic-refactoring.session-validity]
pub proof fn admitted_session_is_admitted(admission: RefactorAdmission)
    requires admission.state == REFACTOR_ADMITTED
    ensures refactor_admitted_spec(admission)
{
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> RefactorSession {
        RefactorSession {
            definitions: vec![
                DefinitionMeta {
                    hash: 10,
                    type_hash: 1,
                    interface_hash: 1,
                },
                DefinitionMeta {
                    hash: 20,
                    type_hash: 1,
                    interface_hash: 1,
                },
                DefinitionMeta {
                    hash: 30,
                    type_hash: 2,
                    interface_hash: 2,
                },
            ],
            before_bindings: vec![NameBinding {
                name_hash: 7,
                definition_hash: 10,
            }],
            after_bindings: vec![NameBinding {
                name_hash: 7,
                definition_hash: 20,
            }],
            compatibilities: vec![ReplacementCompatibility {
                old_hash: 10,
                new_hash: 20,
                type_compatible: true,
                interface_compatible: true,
            }],
            dependents: vec![DependentEdge {
                dependent_hash: 99,
                referenced_hash: 10,
            }],
            unresolved_dependents: vec![],
            step: RefactorStep {
                name_hash: 7,
                old_hash: 10,
                new_hash: 20,
            },
        }
    }

    #[test]
    fn admits_compatible_refactor_without_mutating_old_definition() {
        let s = session();
        let admission = refactor_admission(&s);
        assert_eq!(admission, RefactorAdmission {
            state: REFACTOR_ADMITTED,
            detail: 20,
        });
        assert!(definition_present(&s.definitions, 10));
        assert!(definition_present(&s.definitions, 20));
    }

    #[test]
    fn rejects_unknown_new_definition() {
        let mut s = session();
        s.step.new_hash = 404;
        s.after_bindings = vec![NameBinding {
            name_hash: 7,
            definition_hash: 404,
        }];
        let admission = refactor_admission(&s);
        assert_eq!(admission, RefactorAdmission {
            state: REFACTOR_MISSING_NEW_DEFINITION,
            detail: 404,
        });
    }

    #[test]
    fn rejects_dangling_resulting_name() {
        let mut s = session();
        s.after_bindings.push(NameBinding {
            name_hash: 8,
            definition_hash: 404,
        });
        let admission = refactor_admission(&s);
        assert_eq!(admission, RefactorAdmission {
            state: REFACTOR_DANGLING_NAME,
            detail: 7,
        });
    }

    #[test]
    fn incompatible_dependent_must_be_recorded_unresolved() {
        let mut s = session();
        s.compatibilities = vec![ReplacementCompatibility {
            old_hash: 10,
            new_hash: 20,
            type_compatible: false,
            interface_compatible: true,
        }];
        let admission = refactor_admission(&s);
        assert_eq!(admission, RefactorAdmission {
            state: REFACTOR_MISSING_UNRESOLVED_DEPENDENT,
            detail: 99,
        });

        s.unresolved_dependents = vec![99];
        let admission = refactor_admission(&s);
        assert_eq!(admission, RefactorAdmission {
            state: REFACTOR_ADMITTED,
            detail: 20,
        });
    }

    #[test]
    fn rejects_missing_name_movement_bindings() {
        let mut s = session();
        s.before_bindings.clear();
        let admission = refactor_admission(&s);
        assert_eq!(admission, RefactorAdmission {
            state: REFACTOR_MISSING_OLD_BINDING,
            detail: 7,
        });

        let mut s = session();
        s.after_bindings.clear();
        let admission = refactor_admission(&s);
        assert_eq!(admission, RefactorAdmission {
            state: REFACTOR_MISSING_NEW_BINDING,
            detail: 7,
        });
    }
}
