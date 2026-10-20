//! Finite remote computation admission primitives.
//!
//! This module models the structural admission side of Unison-style remote
//! computation: a finite envelope names a computation, dependencies, required
//! abilities, a target node, and declared resource bounds. Admission reports
//! only metadata-level readiness, fetch-pending state, or rejection; it does not
//! claim real networking, authentication, sandbox execution, scheduling, or
//! code safety.
use vstd::prelude::*;

verus! {

pub const REMOTE_ADMITTED: u8 = 0;
pub const REMOTE_FETCH_PENDING: u8 = 1;
pub const REMOTE_UNKNOWN_TARGET: u8 = 2;
pub const REMOTE_MALFORMED_BOUNDS: u8 = 3;
pub const REMOTE_UNFETCHABLE_DEPENDENCY: u8 = 4;
pub const REMOTE_MISSING_ABILITY: u8 = 5;
pub const REMOTE_RESOURCE_EXCEEDED: u8 = 6;

pub const DEP_READY: u8 = 0;
pub const DEP_FETCH_PENDING: u8 = 1;
pub const DEP_UNFETCHABLE: u8 = 2;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RemoteComputationEnvelope {
    pub computation_hash: u64,
    pub dependency_hashes: Vec<u64>,
    pub required_abilities: Vec<u64>,
    pub target_node: u64,
    pub max_memory: u64,
    pub max_steps: u64,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RemoteTargetNode {
    pub node_id: u64,
    pub cached_hashes: Vec<u64>,
    pub fetchable_hashes: Vec<u64>,
    pub sandbox_abilities: Vec<u64>,
    pub max_memory: u64,
    pub max_steps: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DependencyAvailability {
    pub state: u8,
    pub dependency: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RemoteAdmission {
    pub state: u8,
    pub detail: u64,
}

// r[depends domain-primitives.remote-computation.envelope]
pub open spec fn node_present_spec(nodes: Seq<RemoteTargetNode>, node_id: u64) -> bool {
    exists|i: int| 0 <= i < nodes.len() && nodes[i].node_id == node_id
}

// r[depends domain-primitives.remote-computation.envelope]
pub open spec fn remote_envelope_well_formed_spec(envelope: RemoteComputationEnvelope) -> bool {
    envelope.computation_hash > 0
        && envelope.target_node > 0
        && envelope.max_memory > 0
        && envelope.max_steps > 0
}

// r[depends domain-primitives.remote-computation.dependency-availability]
pub open spec fn value_present_spec(values: Seq<u64>, value: u64) -> bool {
    exists|i: int| 0 <= i < values.len() && values[i] == value
}

// r[depends domain-primitives.remote-computation.dependency-availability]
pub open spec fn dependency_availability_from_spec(
    deps: Seq<u64>,
    cached: Seq<u64>,
    fetchable: Seq<u64>,
    from: int,
) -> DependencyAvailability
    recommends 0 <= from <= deps.len(),
    decreases deps.len() - from,
{
    if from >= deps.len() {
        DependencyAvailability { state: DEP_READY, dependency: 0 }
    } else if value_present_spec(cached, deps[from]) {
        dependency_availability_from_spec(deps, cached, fetchable, from + 1)
    } else if value_present_spec(fetchable, deps[from]) {
        DependencyAvailability { state: DEP_FETCH_PENDING, dependency: deps[from] }
    } else {
        DependencyAvailability { state: DEP_UNFETCHABLE, dependency: deps[from] }
    }
}

// r[depends domain-primitives.remote-computation.dependency-availability]
pub open spec fn dependency_availability_spec(
    envelope: RemoteComputationEnvelope,
    node: RemoteTargetNode,
) -> DependencyAvailability {
    dependency_availability_from_spec(envelope.dependency_hashes@, node.cached_hashes@, node.fetchable_hashes@, 0)
}

// r[depends domain-primitives.remote-computation.dependency-availability]
pub open spec fn dependency_ready_spec(envelope: RemoteComputationEnvelope, node: RemoteTargetNode) -> bool {
    dependency_availability_spec(envelope, node).state == DEP_READY
}

// r[depends domain-primitives.remote-computation.ability-resource]
pub open spec fn abilities_admitted_spec(required: Seq<u64>, provided: Seq<u64>) -> bool {
    forall|i: int| 0 <= i < required.len() ==> value_present_spec(provided, #[trigger] required[i])
}

// r[depends domain-primitives.remote-computation.ability-resource]
pub open spec fn first_missing_ability_from_spec(required: Seq<u64>, provided: Seq<u64>, from: int) -> Option<u64>
    recommends 0 <= from <= required.len(),
    decreases required.len() - from,
{
    if from >= required.len() {
        Option::None
    } else if !value_present_spec(provided, required[from]) {
        Option::Some(required[from])
    } else {
        first_missing_ability_from_spec(required, provided, from + 1)
    }
}

// r[depends domain-primitives.remote-computation.ability-resource]
pub open spec fn first_missing_ability_spec(envelope: RemoteComputationEnvelope, node: RemoteTargetNode) -> Option<u64> {
    first_missing_ability_from_spec(envelope.required_abilities@, node.sandbox_abilities@, 0)
}

// r[depends domain-primitives.remote-computation.ability-resource]
pub open spec fn resource_bounds_fit_spec(envelope: RemoteComputationEnvelope, node: RemoteTargetNode) -> bool {
    envelope.max_memory <= node.max_memory && envelope.max_steps <= node.max_steps
}

// r[depends domain-primitives.remote-computation.envelope]
// r[depends domain-primitives.remote-computation.dependency-availability]
// r[depends domain-primitives.remote-computation.ability-resource]
pub open spec fn remote_admission_for_node_spec(
    envelope: RemoteComputationEnvelope,
    node: RemoteTargetNode,
) -> RemoteAdmission {
    if !remote_envelope_well_formed_spec(envelope) {
        RemoteAdmission { state: REMOTE_MALFORMED_BOUNDS, detail: envelope.computation_hash }
    } else {
        let deps = dependency_availability_spec(envelope, node);
        if deps.state == DEP_UNFETCHABLE {
            RemoteAdmission { state: REMOTE_UNFETCHABLE_DEPENDENCY, detail: deps.dependency }
        } else if deps.state == DEP_FETCH_PENDING {
            RemoteAdmission { state: REMOTE_FETCH_PENDING, detail: deps.dependency }
        } else {
            match first_missing_ability_spec(envelope, node) {
                Option::Some(ability) => RemoteAdmission { state: REMOTE_MISSING_ABILITY, detail: ability },
                Option::None => if !resource_bounds_fit_spec(envelope, node) {
                    RemoteAdmission { state: REMOTE_RESOURCE_EXCEEDED, detail: envelope.target_node }
                } else {
                    RemoteAdmission { state: REMOTE_ADMITTED, detail: envelope.computation_hash }
                },
            }
        }
    }
}

// r[depends domain-primitives.remote-computation.envelope]
pub open spec fn remote_admitted_spec(admission: RemoteAdmission) -> bool {
    admission.state == REMOTE_ADMITTED
}

// r[impl domain-primitives.remote-computation.envelope]
pub fn node_present(nodes: &Vec<RemoteTargetNode>, node_id: u64) -> (result: bool)
    ensures result == node_present_spec(nodes@, node_id)
{
    let mut i: usize = 0;
    while i < nodes.len()
        invariant
            0 <= i <= nodes.len(),
            forall|k: int| 0 <= k < i ==> nodes@[k].node_id != node_id,
        decreases nodes.len() - i,
    {
        if nodes[i].node_id == node_id {
            assert(node_present_spec(nodes@, node_id));
            return true;
        }
        i = i + 1;
    }
    assert forall|k: int| 0 <= k < nodes@.len() implies nodes@[k].node_id != node_id by {
        assert(0 <= k < i);
    }
    assert(!node_present_spec(nodes@, node_id));
    false
}

// r[impl domain-primitives.remote-computation.envelope]
pub fn remote_envelope_well_formed(envelope: &RemoteComputationEnvelope) -> (result: bool)
    ensures result == remote_envelope_well_formed_spec(*envelope)
{
    envelope.computation_hash > 0 && envelope.target_node > 0 && envelope.max_memory > 0 && envelope.max_steps > 0
}

// r[impl domain-primitives.remote-computation.dependency-availability]
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

// r[impl domain-primitives.remote-computation.dependency-availability]
pub fn dependency_availability(envelope: &RemoteComputationEnvelope, node: &RemoteTargetNode) -> (result: DependencyAvailability)
    ensures result == dependency_availability_spec(*envelope, *node)
{
    let mut i: usize = 0;
    while i < envelope.dependency_hashes.len()
        invariant
            0 <= i <= envelope.dependency_hashes.len(),
            dependency_availability_from_spec(envelope.dependency_hashes@, node.cached_hashes@, node.fetchable_hashes@, 0)
                == dependency_availability_from_spec(envelope.dependency_hashes@, node.cached_hashes@, node.fetchable_hashes@, i as int),
        decreases envelope.dependency_hashes.len() - i,
    {
        let dep = envelope.dependency_hashes[i];
        if value_present(&node.cached_hashes, dep) {
            assert(dependency_availability_from_spec(envelope.dependency_hashes@, node.cached_hashes@, node.fetchable_hashes@, i as int)
                == dependency_availability_from_spec(envelope.dependency_hashes@, node.cached_hashes@, node.fetchable_hashes@, i as int + 1));
            i = i + 1;
        } else if value_present(&node.fetchable_hashes, dep) {
            let result = DependencyAvailability { state: DEP_FETCH_PENDING, dependency: dep };
            assert(dependency_availability_from_spec(envelope.dependency_hashes@, node.cached_hashes@, node.fetchable_hashes@, i as int) == result);
            return result;
        } else {
            let result = DependencyAvailability { state: DEP_UNFETCHABLE, dependency: dep };
            assert(dependency_availability_from_spec(envelope.dependency_hashes@, node.cached_hashes@, node.fetchable_hashes@, i as int) == result);
            return result;
        }
    }
    let result = DependencyAvailability { state: DEP_READY, dependency: 0 };
    assert(dependency_availability_from_spec(envelope.dependency_hashes@, node.cached_hashes@, node.fetchable_hashes@, i as int) == result);
    result
}

// r[impl domain-primitives.remote-computation.dependency-availability]
pub fn dependency_ready(envelope: &RemoteComputationEnvelope, node: &RemoteTargetNode) -> (result: bool)
    ensures result == dependency_ready_spec(*envelope, *node)
{
    dependency_availability(envelope, node).state == DEP_READY
}

// r[impl domain-primitives.remote-computation.ability-resource]
pub fn abilities_admitted(required: &Vec<u64>, provided: &Vec<u64>) -> (result: bool)
    ensures result == abilities_admitted_spec(required@, provided@)
{
    let mut i: usize = 0;
    while i < required.len()
        invariant
            0 <= i <= required.len(),
            forall|k: int| 0 <= k < i ==> value_present_spec(provided@, #[trigger] required@[k]),
        decreases required.len() - i,
    {
        if !value_present(provided, required[i]) {
            assert(0 <= i < required@.len());
            assert(!abilities_admitted_spec(required@, provided@));
            return false;
        }
        i = i + 1;
    }
    assert forall|k: int| 0 <= k < required@.len() implies value_present_spec(provided@, #[trigger] required@[k]) by {
        assert(0 <= k < i);
    }
    assert(abilities_admitted_spec(required@, provided@));
    true
}

// r[impl domain-primitives.remote-computation.ability-resource]
pub fn first_missing_ability(envelope: &RemoteComputationEnvelope, node: &RemoteTargetNode) -> (result: Option<u64>)
    ensures result == first_missing_ability_spec(*envelope, *node)
{
    let mut i: usize = 0;
    while i < envelope.required_abilities.len()
        invariant
            0 <= i <= envelope.required_abilities.len(),
            forall|k: int| 0 <= k < i ==> value_present_spec(node.sandbox_abilities@, #[trigger] envelope.required_abilities@[k]),
            first_missing_ability_from_spec(envelope.required_abilities@, node.sandbox_abilities@, 0)
                == first_missing_ability_from_spec(envelope.required_abilities@, node.sandbox_abilities@, i as int),
        decreases envelope.required_abilities.len() - i,
    {
        let ability = envelope.required_abilities[i];
        if !value_present(&node.sandbox_abilities, ability) {
            assert(first_missing_ability_from_spec(envelope.required_abilities@, node.sandbox_abilities@, i as int) == Option::Some(ability));
            return Some(ability);
        }
        assert(first_missing_ability_from_spec(envelope.required_abilities@, node.sandbox_abilities@, i as int)
            == first_missing_ability_from_spec(envelope.required_abilities@, node.sandbox_abilities@, i as int + 1));
        i = i + 1;
    }
    assert(first_missing_ability_from_spec(envelope.required_abilities@, node.sandbox_abilities@, i as int) == Option::<u64>::None);
    None
}

// r[impl domain-primitives.remote-computation.ability-resource]
pub fn resource_bounds_fit(envelope: &RemoteComputationEnvelope, node: &RemoteTargetNode) -> (result: bool)
    ensures result == resource_bounds_fit_spec(*envelope, *node)
{
    envelope.max_memory <= node.max_memory && envelope.max_steps <= node.max_steps
}

// r[impl domain-primitives.remote-computation.envelope]
// r[impl domain-primitives.remote-computation.dependency-availability]
// r[impl domain-primitives.remote-computation.ability-resource]
pub fn remote_admission_for_node(envelope: &RemoteComputationEnvelope, node: &RemoteTargetNode) -> (result: RemoteAdmission)
    ensures result == remote_admission_for_node_spec(*envelope, *node)
{
    if !remote_envelope_well_formed(envelope) {
        return RemoteAdmission { state: REMOTE_MALFORMED_BOUNDS, detail: envelope.computation_hash };
    }
    let deps = dependency_availability(envelope, node);
    if deps.state == DEP_UNFETCHABLE {
        return RemoteAdmission { state: REMOTE_UNFETCHABLE_DEPENDENCY, detail: deps.dependency };
    }
    if deps.state == DEP_FETCH_PENDING {
        return RemoteAdmission { state: REMOTE_FETCH_PENDING, detail: deps.dependency };
    }
    match first_missing_ability(envelope, node) {
        Some(ability) => RemoteAdmission { state: REMOTE_MISSING_ABILITY, detail: ability },
        None => {
            if !resource_bounds_fit(envelope, node) {
                RemoteAdmission { state: REMOTE_RESOURCE_EXCEEDED, detail: envelope.target_node }
            } else {
                RemoteAdmission { state: REMOTE_ADMITTED, detail: envelope.computation_hash }
            }
        }
    }
}

// r[impl domain-primitives.remote-computation.envelope]
// r[impl domain-primitives.remote-computation.dependency-availability]
// r[impl domain-primitives.remote-computation.ability-resource]
pub fn remote_admission(envelope: &RemoteComputationEnvelope, nodes: &Vec<RemoteTargetNode>) -> (result: RemoteAdmission)
    ensures
        result.state == REMOTE_UNKNOWN_TARGET ==> !node_present_spec(nodes@, envelope.target_node),
        result.state != REMOTE_UNKNOWN_TARGET ==> node_present_spec(nodes@, envelope.target_node),
{
    let mut i: usize = 0;
    while i < nodes.len()
        invariant
            0 <= i <= nodes.len(),
            forall|k: int| 0 <= k < i ==> nodes@[k].node_id != envelope.target_node,
        decreases nodes.len() - i,
    {
        if nodes[i].node_id == envelope.target_node {
            let result = remote_admission_for_node(envelope, &nodes[i]);
            assert(node_present_spec(nodes@, envelope.target_node));
            return result;
        }
        i = i + 1;
    }
    assert forall|k: int| 0 <= k < nodes@.len() implies nodes@[k].node_id != envelope.target_node by {
        assert(0 <= k < i);
    }
    assert(!node_present_spec(nodes@, envelope.target_node));
    RemoteAdmission { state: REMOTE_UNKNOWN_TARGET, detail: envelope.target_node }
}

// r[verify domain-primitives.remote-computation.dependency-availability]
pub proof fn dependency_ready_state_is_ready(envelope: RemoteComputationEnvelope, node: RemoteTargetNode)
    requires dependency_ready_spec(envelope, node)
    ensures dependency_availability_spec(envelope, node).state == DEP_READY
{
}

// r[verify domain-primitives.remote-computation.ability-resource]
pub proof fn admitted_state_means_remote_admitted(admission: RemoteAdmission)
    requires admission.state == REMOTE_ADMITTED
    ensures remote_admitted_spec(admission)
{
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope() -> RemoteComputationEnvelope {
        RemoteComputationEnvelope {
            computation_hash: 10,
            dependency_hashes: vec![1, 2],
            required_abilities: vec![7, 8],
            target_node: 42,
            max_memory: 128,
            max_steps: 1_000,
        }
    }

    fn node() -> RemoteTargetNode {
        RemoteTargetNode {
            node_id: 42,
            cached_hashes: vec![1, 2],
            fetchable_hashes: vec![3],
            sandbox_abilities: vec![7, 8],
            max_memory: 256,
            max_steps: 2_000,
        }
    }

    #[test]
    fn admits_ready_remote_envelope() {
        let admission = remote_admission(&envelope(), &vec![node()]);
        assert_eq!(admission, RemoteAdmission {
            state: REMOTE_ADMITTED,
            detail: 10
        });
    }

    #[test]
    fn reports_fetch_pending_dependency() {
        let mut env = envelope();
        env.dependency_hashes = vec![1, 3];
        let admission = remote_admission(&env, &vec![node()]);
        assert_eq!(admission, RemoteAdmission {
            state: REMOTE_FETCH_PENDING,
            detail: 3
        });
    }

    #[test]
    fn rejects_unfetchable_dependency() {
        let mut env = envelope();
        env.dependency_hashes = vec![1, 99];
        let admission = remote_admission(&env, &vec![node()]);
        assert_eq!(admission, RemoteAdmission {
            state: REMOTE_UNFETCHABLE_DEPENDENCY,
            detail: 99
        });
    }

    #[test]
    fn rejects_missing_ability_and_excess_resource() {
        let mut n = node();
        n.sandbox_abilities = vec![7];
        let admission = remote_admission(&envelope(), &vec![n]);
        assert_eq!(admission, RemoteAdmission {
            state: REMOTE_MISSING_ABILITY,
            detail: 8
        });

        let mut env = envelope();
        env.max_steps = 9_999;
        let admission = remote_admission(&env, &vec![node()]);
        assert_eq!(admission, RemoteAdmission {
            state: REMOTE_RESOURCE_EXCEEDED,
            detail: 42
        });
    }

    #[test]
    fn rejects_unknown_target_and_malformed_bounds() {
        let mut env = envelope();
        env.target_node = 100;
        let admission = remote_admission(&env, &vec![node()]);
        assert_eq!(admission, RemoteAdmission {
            state: REMOTE_UNKNOWN_TARGET,
            detail: 100
        });

        let mut malformed = envelope();
        malformed.max_memory = 0;
        let admission = remote_admission(&malformed, &vec![node()]);
        assert_eq!(admission, RemoteAdmission {
            state: REMOTE_MALFORMED_BOUNDS,
            detail: 10
        });
    }
}
