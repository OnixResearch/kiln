//! Finite ability/effect admission primitives.
//!
//! This module models Unison-style abilities as finite structural identifiers.
//! Requirement sets are admitted by subset checks, handlers syntactically
//! discharge one declared ability, and sandboxes may only attenuate a parent
//! context. No runtime effect, I/O, continuation, or typechecker correctness is
//! claimed by these predicates.
use vstd::prelude::*;

verus! {

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AbilityRequirementSet {
    pub required: Vec<u64>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AbilityContext {
    pub provided: Vec<u64>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AbilityHandler {
    pub handled: u64,
    pub available: bool,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SandboxEnvelope {
    pub parent_provided: Vec<u64>,
    pub sandbox_provided: Vec<u64>,
}

// r[depends domain-primitives.ability-effect.requirement-set]
pub open spec fn ability_present_spec(abilities: Seq<u64>, ability: u64) -> bool {
    exists|i: int| 0 <= i < abilities.len() && abilities[i] == ability
}

// r[depends domain-primitives.ability-effect.requirement-set]
pub open spec fn requirements_admitted_spec(required: Seq<u64>, provided: Seq<u64>) -> bool {
    forall|i: int| 0 <= i < required.len() ==> ability_present_spec(provided, #[trigger] required[i])
}

// r[depends domain-primitives.ability-effect.requirement-set]
pub open spec fn requirement_set_admitted_spec(requirements: AbilityRequirementSet, context: AbilityContext) -> bool {
    requirements_admitted_spec(requirements.required@, context.provided@)
}

// r[depends domain-primitives.ability-effect.requirement-set]
pub open spec fn pure_requirement_set_spec(requirements: AbilityRequirementSet) -> bool {
    requirements.required@.len() == 0
}

// r[depends domain-primitives.ability-effect.requirement-set]
pub open spec fn first_missing_ability_from_spec(required: Seq<u64>, provided: Seq<u64>, from: int) -> Option<u64>
    recommends 0 <= from <= required.len(),
    decreases required.len() - from,
{
    if from >= required.len() {
        Option::None
    } else if !ability_present_spec(provided, required[from]) {
        Option::Some(required[from])
    } else {
        first_missing_ability_from_spec(required, provided, from + 1)
    }
}

// r[depends domain-primitives.ability-effect.requirement-set]
pub open spec fn first_missing_ability_spec(requirements: AbilityRequirementSet, context: AbilityContext) -> Option<u64> {
    first_missing_ability_from_spec(requirements.required@, context.provided@, 0)
}

// r[depends domain-primitives.ability-effect.requirement-set]
pub open spec fn first_missing_ability_result_spec(required: Seq<u64>, provided: Seq<u64>, result: Option<u64>) -> bool {
    match result {
        Option::Some(ability) => exists|i: int|
            0 <= i < required.len()
                && required[i] == ability
                && !ability_present_spec(provided, ability)
                && forall|k: int| 0 <= k < i ==> ability_present_spec(provided, #[trigger] required[k]),
        Option::None => requirements_admitted_spec(required, provided),
    }
}

// r[depends domain-primitives.ability-effect.handler-discharge]
pub open spec fn discharge_prefix_spec(required: Seq<u64>, handler: AbilityHandler, upto: int) -> Seq<u64>
    recommends 0 <= upto <= required.len(),
    decreases upto,
{
    if upto <= 0 {
        seq![]
    } else {
        let prev = discharge_prefix_spec(required, handler, upto - 1);
        let ability = required[upto - 1];
        if handler.available && ability == handler.handled {
            prev
        } else {
            prev.push(ability)
        }
    }
}

// r[depends domain-primitives.ability-effect.handler-discharge]
pub open spec fn discharge_requirements_spec(requirements: AbilityRequirementSet, handler: AbilityHandler) -> Seq<u64> {
    discharge_prefix_spec(requirements.required@, handler, requirements.required@.len() as int)
}

// r[depends domain-primitives.ability-effect.handler-discharge]
pub open spec fn ability_unhandled_by_handler_spec(ability: u64, handler: AbilityHandler) -> bool {
    !handler.available || ability != handler.handled
}

// r[depends domain-primitives.ability-effect.handler-discharge]
pub open spec fn ability_present_before_spec(required: Seq<u64>, ability: u64, upto: int) -> bool {
    exists|i: int| 0 <= i < upto && i < required.len() && required[i] == ability
}

// r[depends domain-primitives.ability-effect.handler-discharge]
pub open spec fn discharged_requirement_preserves_spec(
    requirements: AbilityRequirementSet,
    handler: AbilityHandler,
    ability: u64,
) -> bool {
    ability_unhandled_by_handler_spec(ability, handler)
        && ability_present_spec(requirements.required@, ability)
        ==> ability_present_spec(discharge_requirements_spec(requirements, handler), ability)
}

// r[depends domain-primitives.ability-effect.sandbox-attenuation]
pub open spec fn sandbox_attenuation_admitted_spec(sandbox: SandboxEnvelope) -> bool {
    requirements_admitted_spec(sandbox.sandbox_provided@, sandbox.parent_provided@)
}

// r[depends domain-primitives.ability-effect.sandbox-attenuation]
pub open spec fn requirements_admitted_in_sandbox_spec(requirements: AbilityRequirementSet, sandbox: SandboxEnvelope) -> bool {
    sandbox_attenuation_admitted_spec(sandbox)
        && requirements_admitted_spec(requirements.required@, sandbox.sandbox_provided@)
}

// r[impl domain-primitives.ability-effect.requirement-set]
pub fn ability_present(abilities: &Vec<u64>, ability: u64) -> (result: bool)
    ensures result == ability_present_spec(abilities@, ability)
{
    let mut i: usize = 0;
    while i < abilities.len()
        invariant
            0 <= i <= abilities.len(),
            forall|k: int| 0 <= k < i ==> abilities@[k] != ability,
        decreases abilities.len() - i,
    {
        if abilities[i] == ability {
            assert(ability_present_spec(abilities@, ability));
            return true;
        }
        i = i + 1;
    }
    assert forall|k: int| 0 <= k < abilities@.len() implies abilities@[k] != ability by {
        assert(0 <= k < i);
    }
    assert(!ability_present_spec(abilities@, ability));
    false
}

// r[impl domain-primitives.ability-effect.requirement-set]
pub fn requirements_admitted(required: &Vec<u64>, provided: &Vec<u64>) -> (result: bool)
    ensures result == requirements_admitted_spec(required@, provided@)
{
    let mut i: usize = 0;
    while i < required.len()
        invariant
            0 <= i <= required.len(),
            forall|k: int| 0 <= k < i ==> ability_present_spec(provided@, #[trigger] required@[k]),
        decreases required.len() - i,
    {
        if !ability_present(provided, required[i]) {
            assert(0 <= i < required@.len());
            assert(!requirements_admitted_spec(required@, provided@));
            return false;
        }
        i = i + 1;
    }
    assert forall|k: int| 0 <= k < required@.len() implies ability_present_spec(provided@, #[trigger] required@[k]) by {
        assert(0 <= k < i);
    }
    assert(requirements_admitted_spec(required@, provided@));
    true
}

// r[impl domain-primitives.ability-effect.requirement-set]
pub fn requirement_set_admitted(requirements: &AbilityRequirementSet, context: &AbilityContext) -> (result: bool)
    ensures result == requirement_set_admitted_spec(*requirements, *context)
{
    requirements_admitted(&requirements.required, &context.provided)
}

// r[impl domain-primitives.ability-effect.requirement-set]
pub fn pure_requirement_set(requirements: &AbilityRequirementSet) -> (result: bool)
    ensures result == pure_requirement_set_spec(*requirements)
{
    requirements.required.len() == 0
}

// r[impl domain-primitives.ability-effect.requirement-set]
pub fn first_missing_ability(requirements: &AbilityRequirementSet, context: &AbilityContext) -> (result: Option<u64>)
    ensures
        result == first_missing_ability_spec(*requirements, *context),
        first_missing_ability_result_spec(requirements.required@, context.provided@, result),
{
    let mut i: usize = 0;
    while i < requirements.required.len()
        invariant
            0 <= i <= requirements.required.len(),
            forall|k: int| 0 <= k < i ==> ability_present_spec(context.provided@, #[trigger] requirements.required@[k]),
            first_missing_ability_from_spec(requirements.required@, context.provided@, 0)
                == first_missing_ability_from_spec(requirements.required@, context.provided@, i as int),
        decreases requirements.required.len() - i,
    {
        if !ability_present(&context.provided, requirements.required[i]) {
            let missing = requirements.required[i];
            assert(first_missing_ability_from_spec(requirements.required@, context.provided@, i as int) == Option::Some(missing));
            assert(first_missing_ability_result_spec(requirements.required@, context.provided@, Option::Some(missing)));
            return Some(missing);
        }
        assert(ability_present_spec(context.provided@, requirements.required@[i as int]));
        assert(first_missing_ability_from_spec(requirements.required@, context.provided@, i as int)
            == first_missing_ability_from_spec(requirements.required@, context.provided@, i as int + 1));
        i = i + 1;
    }
    assert(first_missing_ability_from_spec(requirements.required@, context.provided@, i as int) == Option::<u64>::None);
    assert(requirements_admitted_spec(requirements.required@, context.provided@));
    assert(first_missing_ability_result_spec(requirements.required@, context.provided@, Option::None));
    None
}

// r[impl domain-primitives.ability-effect.handler-discharge]
pub fn discharge_requirements(requirements: &AbilityRequirementSet, handler: AbilityHandler) -> (result: Vec<u64>)
    ensures result@ == discharge_requirements_spec(*requirements, handler)
{
    let mut out: Vec<u64> = Vec::new();
    let mut i: usize = 0;
    while i < requirements.required.len()
        invariant
            0 <= i <= requirements.required.len(),
            out@ == discharge_prefix_spec(requirements.required@, handler, i as int),
        decreases requirements.required.len() - i,
    {
        let ability = requirements.required[i];
        if !(handler.available && ability == handler.handled) {
            out.push(ability);
            assert(out@ == discharge_prefix_spec(requirements.required@, handler, i as int + 1));
        } else {
            assert(out@ == discharge_prefix_spec(requirements.required@, handler, i as int + 1));
        }
        i = i + 1;
    }
    assert(out@ == discharge_requirements_spec(*requirements, handler));
    out
}

// r[verify domain-primitives.ability-effect.handler-discharge]
pub proof fn discharge_prefix_preserves_unhandled(
    required: Seq<u64>,
    handler: AbilityHandler,
    upto: int,
    ability: u64,
)
    requires
        0 <= upto <= required.len(),
        ability_unhandled_by_handler_spec(ability, handler),
        ability_present_before_spec(required, ability, upto),
    ensures ability_present_spec(discharge_prefix_spec(required, handler, upto), ability),
    decreases upto,
{
    if upto <= 0 {
        assert(!ability_present_before_spec(required, ability, upto));
    } else {
        let last_index = upto - 1;
        let last = required[last_index];
        if last == ability {
            if handler.available {
                assert(ability != handler.handled);
            }
            assert(!(handler.available && last == handler.handled));
            assert(discharge_prefix_spec(required, handler, upto)
                == discharge_prefix_spec(required, handler, last_index).push(ability));
            let out = discharge_prefix_spec(required, handler, upto);
            assert(out[out.len() - 1] == ability);
            assert(ability_present_spec(out, ability));
        } else {
            let witness = choose|i: int| 0 <= i < upto && i < required.len() && required[i] == ability;
            assert(witness != last_index);
            assert(witness < last_index);
            assert(ability_present_before_spec(required, ability, last_index));
            discharge_prefix_preserves_unhandled(required, handler, last_index, ability);
            let prev = discharge_prefix_spec(required, handler, last_index);
            assert(ability_present_spec(prev, ability));
            if handler.available && last == handler.handled {
                assert(discharge_prefix_spec(required, handler, upto) == prev);
            } else {
                assert(discharge_prefix_spec(required, handler, upto) == prev.push(last));
                let old_witness = choose|i: int| 0 <= i < prev.len() && prev[i] == ability;
                assert(prev.push(last)[old_witness] == ability);
                assert(ability_present_spec(prev.push(last), ability));
            }
            assert(ability_present_spec(discharge_prefix_spec(required, handler, upto), ability));
        }
    }
}

// r[verify domain-primitives.ability-effect.handler-discharge]
pub proof fn discharge_preserves_unhandled(requirements: AbilityRequirementSet, handler: AbilityHandler, ability: u64)
    requires
        ability_unhandled_by_handler_spec(ability, handler),
        ability_present_spec(requirements.required@, ability),
    ensures discharged_requirement_preserves_spec(requirements, handler, ability)
{
    let witness = choose|i: int| 0 <= i < requirements.required@.len() && requirements.required@[i] == ability;
    assert(ability_present_before_spec(requirements.required@, ability, requirements.required@.len() as int));
    discharge_prefix_preserves_unhandled(requirements.required@, handler, requirements.required@.len() as int, ability);
    assert(discharged_requirement_preserves_spec(requirements, handler, ability));
}

// r[impl domain-primitives.ability-effect.sandbox-attenuation]
pub fn sandbox_attenuation_admitted(sandbox: &SandboxEnvelope) -> (result: bool)
    ensures result == sandbox_attenuation_admitted_spec(*sandbox)
{
    requirements_admitted(&sandbox.sandbox_provided, &sandbox.parent_provided)
}

// r[impl domain-primitives.ability-effect.sandbox-attenuation]
pub fn requirements_admitted_in_sandbox(requirements: &AbilityRequirementSet, sandbox: &SandboxEnvelope) -> (result: bool)
    ensures result == requirements_admitted_in_sandbox_spec(*requirements, *sandbox)
{
    if !sandbox_attenuation_admitted(sandbox) {
        return false;
    }
    requirements_admitted(&requirements.required, &sandbox.sandbox_provided)
}

// r[verify domain-primitives.ability-effect.requirement-set]
pub proof fn empty_requirement_set_is_admitted(requirements: AbilityRequirementSet, context: AbilityContext)
    requires pure_requirement_set_spec(requirements)
    ensures requirement_set_admitted_spec(requirements, context)
{
    assert forall|i: int| 0 <= i < requirements.required@.len() implies ability_present_spec(context.provided@, #[trigger] requirements.required@[i]) by {
        assert(false);
    }
}

// r[verify domain-primitives.ability-effect.sandbox-attenuation]
pub proof fn sandbox_admission_requires_attenuation(requirements: AbilityRequirementSet, sandbox: SandboxEnvelope)
    requires requirements_admitted_in_sandbox_spec(requirements, sandbox)
    ensures sandbox_attenuation_admitted_spec(sandbox)
{
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // r[verify domain-primitives.ability-effect.requirement-set]
    fn admits_when_all_abilities_are_provided() {
        let requirements = AbilityRequirementSet { required: vec![1, 2] };
        let context = AbilityContext {
            provided: vec![3, 1, 2],
        };

        assert!(requirement_set_admitted(&requirements, &context));
        assert_eq!(first_missing_ability(&requirements, &context), None);
    }

    #[test]
    // r[verify domain-primitives.ability-effect.requirement-set]
    fn rejects_with_first_missing_ability() {
        let requirements = AbilityRequirementSet {
            required: vec![1, 2, 4],
        };
        let context = AbilityContext { provided: vec![1, 4] };

        assert!(!requirement_set_admitted(&requirements, &context));
        assert_eq!(first_missing_ability(&requirements, &context), Some(2));
    }

    #[test]
    // r[verify domain-primitives.ability-effect.handler-discharge]
    fn handler_removes_only_declared_available_ability() {
        let requirements = AbilityRequirementSet {
            required: vec![10, 20, 10, 30],
        };
        let handler = AbilityHandler {
            handled: 10,
            available: true,
        };
        let unavailable = AbilityHandler {
            handled: 20,
            available: false,
        };

        assert_eq!(discharge_requirements(&requirements, handler), vec![20, 30]);
        assert_eq!(discharge_requirements(&requirements, unavailable), vec![10, 20, 10, 30]);
    }

    #[test]
    // r[verify domain-primitives.ability-effect.sandbox-attenuation]
    fn sandbox_attenuates_parent_and_bounds_computation() {
        let sandbox = SandboxEnvelope {
            parent_provided: vec![1, 2, 3],
            sandbox_provided: vec![3],
        };
        let admitted = AbilityRequirementSet { required: vec![3] };
        let rejected = AbilityRequirementSet { required: vec![2, 3] };
        let invalid_sandbox = SandboxEnvelope {
            parent_provided: vec![3],
            sandbox_provided: vec![2, 3],
        };

        assert!(sandbox_attenuation_admitted(&sandbox));
        assert!(requirements_admitted_in_sandbox(&admitted, &sandbox));
        assert!(!requirements_admitted_in_sandbox(&rejected, &sandbox));
        assert!(!sandbox_attenuation_admitted(&invalid_sandbox));
        assert!(!requirements_admitted_in_sandbox(&admitted, &invalid_sandbox));
    }
}
