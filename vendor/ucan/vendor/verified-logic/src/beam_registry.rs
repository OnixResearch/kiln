//! Verified BEAM process registry.
//!
//! The registry is a bijective partial map between names and pids, modeled
//! as a sequence of `(name, pid)` pairs with uniqueness invariants. Process
//! death cleanup removes by pid so names can be re-used safely.

use vstd::prelude::*;

verus! {

/// Spec: registry contains a name.
pub open spec fn registry_has_name_spec(registry: Seq<(u64, u64)>, name: u64) -> bool {
    exists|i: int| #![trigger registry[i]] 0 <= i < registry.len() && registry[i].0 == name
}

/// Spec: registry contains a pid.
pub open spec fn registry_has_pid_spec(registry: Seq<(u64, u64)>, pid: u64) -> bool {
    exists|i: int| #![trigger registry[i]] 0 <= i < registry.len() && registry[i].1 == pid
}

// r[depends beam-registry.uniqueness]
/// Spec: names and pids are both unique.
pub open spec fn registry_unique_spec(registry: Seq<(u64, u64)>) -> bool {
    forall|i: int, j: int|
        #![trigger registry[i], registry[j]]
        0 <= i < registry.len() && 0 <= j < registry.len() && i != j
            ==> registry[i].0 != registry[j].0 && registry[i].1 != registry[j].1
}

/// Spec helper: lookup by name starting at `from`.
pub open spec fn whereis_from_spec(registry: Seq<(u64, u64)>, name: u64, from: int) -> Option<u64>
    decreases if from <= registry.len() { registry.len() - from } else { 0 }
{
    if from < 0 || from >= registry.len() {
        None
    } else if registry[from].0 == name {
        Some(registry[from].1)
    } else {
        whereis_from_spec(registry, name, from + 1)
    }
}

/// Spec: lookup by name.
pub open spec fn whereis_spec(registry: Seq<(u64, u64)>, name: u64) -> Option<u64> {
    whereis_from_spec(registry, name, 0)
}

/// Spec helper: remove one name from prefix `[0, upto)`.
pub open spec fn unregister_name_prefix_spec(registry: Seq<(u64, u64)>, name: u64, upto: int) -> Seq<(u64, u64)>
    decreases if upto >= 0 { upto } else { 0 }
{
    if upto <= 0 {
        seq![]
    } else {
        let prefix = unregister_name_prefix_spec(registry, name, upto - 1);
        if registry[upto - 1].0 == name { prefix } else { prefix.push(registry[upto - 1]) }
    }
}

/// Spec: unregister by name.
pub open spec fn unregister_name_spec(registry: Seq<(u64, u64)>, name: u64) -> Seq<(u64, u64)> {
    unregister_name_prefix_spec(registry, name, registry.len() as int)
}

/// Spec helper: remove one pid from prefix `[0, upto)`.
pub open spec fn unregister_pid_prefix_spec(registry: Seq<(u64, u64)>, pid: u64, upto: int) -> Seq<(u64, u64)>
    decreases if upto >= 0 { upto } else { 0 }
{
    if upto <= 0 {
        seq![]
    } else {
        let prefix = unregister_pid_prefix_spec(registry, pid, upto - 1);
        if registry[upto - 1].1 == pid { prefix } else { prefix.push(registry[upto - 1]) }
    }
}

// r[depends beam-registry.cleanup]
/// Spec: cleanup dead process by pid.
pub open spec fn unregister_pid_spec(registry: Seq<(u64, u64)>, pid: u64) -> Seq<(u64, u64)> {
    unregister_pid_prefix_spec(registry, pid, registry.len() as int)
}

// r[depends beam-registry.uniqueness]
/// Spec: register a fresh name/pid pair.
pub open spec fn register_spec(registry: Seq<(u64, u64)>, name: u64, pid: u64) -> Option<Seq<(u64, u64)>> {
    if registry_has_name_spec(registry, name) || registry_has_pid_spec(registry, pid) {
        None
    } else {
        Some(registry.push((name, pid)))
    }
}

// r[impl beam-registry.uniqueness]
/// Lookup a pid by registered name.
pub fn whereis(registry: &[(u64, u64)], name: u64) -> (result: Option<u64>)
    ensures
        result.is_some() ==> exists|i: int| #![trigger registry@[i]] 0 <= i < registry@.len() && registry@[i].0 == name && registry@[i].1 == result.unwrap(),
        result.is_none() ==> !registry_has_name_spec(registry@, name)
{
    let mut i: usize = 0;
    while i < registry.len()
        invariant
            0 <= i <= registry@.len(),
            forall|j: int| #![trigger registry@[j]] 0 <= j < i as int ==> registry@[j].0 != name,
        decreases registry@.len() - i,
    {
        if registry[i].0 == name {
            return Some(registry[i].1);
        }
        i = i + 1;
    }
    None
}

fn clone_registry(registry: &[(u64, u64)]) -> (result: Vec<(u64, u64)>)
    ensures result@ =~= registry@
{
    let mut result: Vec<(u64, u64)> = Vec::new();
    let mut i: usize = 0;
    while i < registry.len()
        invariant
            0 <= i <= registry@.len(),
            result@ =~= registry@.subrange(0, i as int),
        decreases registry@.len() - i,
    {
        result.push(registry[i]);
        i = i + 1;
    }
    result
}

// r[impl beam-registry.uniqueness]
/// Register a fresh name/pid pair.
pub fn register_name(registry: &[(u64, u64)], name: u64, pid: u64) -> (result: Option<Vec<(u64, u64)>>)
    ensures
        result.is_none() <==> register_spec(registry@, name, pid).is_none(),
        result.is_some() ==> result.unwrap()@ == register_spec(registry@, name, pid).unwrap(),
{
    let mut i: usize = 0;
    while i < registry.len()
        invariant
            0 <= i <= registry@.len(),
            forall|j: int| #![trigger registry@[j]] 0 <= j < i as int ==> registry@[j].0 != name && registry@[j].1 != pid,
        decreases registry@.len() - i,
    {
        if registry[i].0 == name || registry[i].1 == pid {
            return None;
        }
        i = i + 1;
    }
    let mut next = clone_registry(registry);
    next.push((name, pid));
    Some(next)
}

// r[impl beam-registry.cleanup]
/// Remove a registry entry by dead pid.
pub fn cleanup_dead_process(registry: &[(u64, u64)], pid: u64) -> (result: Vec<(u64, u64)>)
    ensures result@ == unregister_pid_spec(registry@, pid)
{
    let mut result: Vec<(u64, u64)> = Vec::new();
    let mut i: usize = 0;
    while i < registry.len()
        invariant
            0 <= i <= registry@.len(),
            result@ == unregister_pid_prefix_spec(registry@, pid, i as int),
        decreases registry@.len() - i,
    {
        if registry[i].1 != pid {
            result.push(registry[i]);
            proof {
                assert(unregister_pid_prefix_spec(registry@, pid, i as int + 1)
                    == unregister_pid_prefix_spec(registry@, pid, i as int).push(registry[i as int]));
            }
        } else {
            proof {
                assert(unregister_pid_prefix_spec(registry@, pid, i as int + 1)
                    == unregister_pid_prefix_spec(registry@, pid, i as int));
            }
        }
        i = i + 1;
    }
    result
}

// r[impl beam-registry.uniqueness]
/// Remove a registry entry by name.
pub fn unregister_name(registry: &[(u64, u64)], name: u64) -> (result: Vec<(u64, u64)>)
    ensures result@ == unregister_name_spec(registry@, name)
{
    let mut result: Vec<(u64, u64)> = Vec::new();
    let mut i: usize = 0;
    while i < registry.len()
        invariant
            0 <= i <= registry@.len(),
            result@ == unregister_name_prefix_spec(registry@, name, i as int),
        decreases registry@.len() - i,
    {
        if registry[i].0 != name {
            result.push(registry[i]);
            proof {
                assert(unregister_name_prefix_spec(registry@, name, i as int + 1)
                    == unregister_name_prefix_spec(registry@, name, i as int).push(registry[i as int]));
            }
        } else {
            proof {
                assert(unregister_name_prefix_spec(registry@, name, i as int + 1)
                    == unregister_name_prefix_spec(registry@, name, i as int));
            }
        }
        i = i + 1;
    }
    result
}

// r[verify beam-registry.uniqueness]
proof fn register_preserves_uniqueness(registry: Seq<(u64, u64)>, name: u64, pid: u64)
    requires registry_unique_spec(registry), !registry_has_name_spec(registry, name), !registry_has_pid_spec(registry, pid)
    ensures registry_unique_spec(register_spec(registry, name, pid).unwrap())
{}

// r[verify beam-registry.uniqueness]
proof fn duplicate_name_rejected(registry: Seq<(u64, u64)>, name: u64, pid: u64)
    requires registry_has_name_spec(registry, name)
    ensures register_spec(registry, name, pid).is_none()
{}

// r[verify beam-registry.uniqueness]
proof fn duplicate_pid_rejected(registry: Seq<(u64, u64)>, name: u64, pid: u64)
    requires registry_has_pid_spec(registry, pid)
    ensures register_spec(registry, name, pid).is_none()
{}

// r[verify beam-registry.uniqueness]
pub proof fn register_fresh_pair_is_some(registry: Seq<(u64, u64)>, name: u64, pid: u64)
    requires !registry_has_name_spec(registry, name), !registry_has_pid_spec(registry, pid)
    ensures register_spec(registry, name, pid).is_some()
{}

// r[verify beam-registry.cleanup]
proof fn cleanup_prefix_keeps_absent_pid(registry: Seq<(u64, u64)>, pid: u64, upto: int)
    requires
        0 <= upto <= registry.len(),
        forall|j: int| 0 <= j < upto ==> registry[j].1 != pid,
    ensures
        unregister_pid_prefix_spec(registry, pid, upto) == registry.subrange(0, upto)
    decreases upto
{
    if upto > 0 {
        cleanup_prefix_keeps_absent_pid(registry, pid, upto - 1);
        assert(registry[upto - 1].1 != pid);
        assert(unregister_pid_prefix_spec(registry, pid, upto)
            == unregister_pid_prefix_spec(registry, pid, upto - 1).push(registry[upto - 1]));
        assert(registry.subrange(0, upto)
            == registry.subrange(0, upto - 1).push(registry[upto - 1]));
    }
}

// r[verify beam-registry.cleanup]
pub proof fn cleanup_fresh_register_restores_registry(registry: Seq<(u64, u64)>, name: u64, pid: u64)
    requires
        !registry_has_name_spec(registry, name),
        !registry_has_pid_spec(registry, pid),
    ensures
        unregister_pid_spec(register_spec(registry, name, pid).unwrap(), pid) =~= registry
{
    register_fresh_pair_is_some(registry, name, pid);
    let registered = register_spec(registry, name, pid).unwrap();
    assert(registered == registry.push((name, pid)));
    assert forall|j: int| 0 <= j < registry.len() implies registered[j].1 != pid by {
        assert(registered[j] == registry[j]);
    }
    cleanup_prefix_keeps_absent_pid(registered, pid, registry.len() as int);
    assert(unregister_pid_spec(registered, pid) == unregister_pid_prefix_spec(registered, pid, registered.len() as int));
    assert(unregister_pid_prefix_spec(registered, pid, registered.len() as int)
        == unregister_pid_prefix_spec(registered, pid, registry.len() as int));
    assert(registry.len() + 1 == registered.len());
    assert(registry.subrange(0, registry.len() as int) =~= registry);
}

// r[verify beam-registry.cleanup]
proof fn cleanup_result_is_sequence(registry: Seq<(u64, u64)>, pid: u64)
    ensures unregister_pid_spec(registry, pid).len() >= 0
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify beam-registry.uniqueness]
    #[test]
    fn test_register_and_whereis() {
        let registry = register_name(&vec![], 10, 20).unwrap();
        assert_eq!(whereis(&registry, 10), Some(20));
        assert!(register_name(&registry, 10, 99).is_none());
        assert!(register_name(&registry, 11, 20).is_none());
    }

    // r[verify beam-registry.cleanup]
    #[test]
    fn test_cleanup_dead_process() {
        let registry = vec![(10, 20), (11, 21)];
        let next = cleanup_dead_process(&registry, 20);
        assert_eq!(next, vec![(11, 21)]);
        assert_eq!(whereis(&next, 10), None);
    }

    // r[verify beam-registry.uniqueness]
    #[test]
    fn test_unregister_name() {
        let registry = vec![(10, 20), (11, 21)];
        let next = unregister_name(&registry, 10);
        assert_eq!(next, vec![(11, 21)]);
        assert_eq!(whereis(&next, 10), None);
    }

    // r[verify beam-registry.cleanup]
    // r[verify beam-registry.uniqueness]
    #[test]
    fn test_cleanup_allows_reregister_same_name() {
        let registry = register_name(&vec![], 10, 20).unwrap();
        let cleaned = cleanup_dead_process(&registry, 20);
        let reregistered = register_name(&cleaned, 10, 20).unwrap();
        assert_eq!(whereis(&reregistered, 10), Some(20));
    }

    proptest! {
        // r[verify beam-registry.uniqueness]
        #[test]
        fn prop_register_fresh_name_pid(name: u64, pid: u64) {
            let registry: Vec<(u64, u64)> = Vec::new();
            let next = register_name(&registry, name, pid).unwrap();
            prop_assert_eq!(whereis(&next, name), Some(pid));
        }
    }
}
