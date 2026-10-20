//! Verified UCAN capability merge from multiple delegation paths.
//!
//! When a principal holds capabilities from multiple independent delegation
//! chains, the effective capability set is the union. Proves that the merged
//! set is a superset of each individual path's capabilities, and that
//! merging preserves the attenuation invariant.

#![allow(clippy::ptr_arg)]

use vstd::prelude::*;

verus! {

/// A capability entry: (resource, ability) pair as byte sequences.
pub struct CapEntry {
    pub resource: Vec<u8>,
    pub ability: Vec<u8>,
}

/// Spec-level capability: (resource, ability).
pub open spec fn cap_view(c: &CapEntry) -> (Seq<u8>, Seq<u8>) {
    (c.resource@, c.ability@)
}

/// Capability containment: child resource starts with parent resource
/// and abilities match.
// r[impl ucan-capability-merge.containment]
pub open spec fn cap_contained_spec(
    parent: (Seq<u8>, Seq<u8>),
    child: (Seq<u8>, Seq<u8>),
) -> bool {
    child.0.len() >= parent.0.len()
    && child.0.subrange(0, parent.0.len() as int) =~= parent.0
    && child.1 =~= parent.1
}

/// A capability set is the view of a Vec<CapEntry>.
pub open spec fn capset_view(caps: &Vec<CapEntry>) -> Seq<(Seq<u8>, Seq<u8>)> {
    caps@.map_values(|c: CapEntry| cap_view(&c))
}

/// Union of two capability sets (concatenation).
// r[impl ucan-capability-merge.union]
pub open spec fn capset_union_spec(
    a: Seq<(Seq<u8>, Seq<u8>)>,
    b: Seq<(Seq<u8>, Seq<u8>)>,
) -> Seq<(Seq<u8>, Seq<u8>)> {
    a + b
}

/// A capability is authorized by a set if some entry in the set contains it.
// r[impl ucan-capability-merge.authorization]
pub open spec fn authorized_by_spec(
    capset: Seq<(Seq<u8>, Seq<u8>)>,
    cap: (Seq<u8>, Seq<u8>),
) -> bool {
    exists|i: int| 0 <= i < capset.len()
        && #[trigger] cap_contained_spec(capset[i], cap)
}

/// Merge two capability sets (union/concatenation at runtime).
// r[impl ucan-capability-merge.union]
pub fn merge_capsets(a: &Vec<CapEntry>, b: &Vec<CapEntry>) -> (result: Vec<CapEntry>)
    ensures result@.len() == a@.len() + b@.len(),
{
    let mut result = Vec::new();
    let mut i: usize = 0;
    while i < a.len()
        invariant
            0 <= i <= a.len(),
            result@.len() == i as int,
        decreases a.len() - i,
    {
        result.push(CapEntry {
            resource: a[i].resource.clone(),
            ability: a[i].ability.clone(),
        });
        i = i + 1;
    }
    let mut j: usize = 0;
    while j < b.len()
        invariant
            0 <= j <= b.len(),
            result@.len() == a@.len() + j as int,
        decreases b.len() - j,
    {
        result.push(CapEntry {
            resource: b[j].resource.clone(),
            ability: b[j].ability.clone(),
        });
        j = j + 1;
    }
    result
}


/// Check byte-sequence equality.
// r[impl ucan-capability-merge.authorization]
pub fn bytes_equal(a: &Vec<u8>, b: &Vec<u8>) -> (result: bool)
    ensures result == (a@ =~= b@),
{
    if a.len() != b.len() {
        return false;
    }
    let mut i: usize = 0;
    while i < a.len()
        invariant
            0 <= i <= a.len(),
            a.len() == b.len(),
            forall|k: int| 0 <= k < i ==> a@[k] == b@[k],
        decreases a.len() - i,
    {
        if a[i] != b[i] {
            assert(a@[i as int] != b@[i as int]);
            assert(!(a@ =~= b@));
            return false;
        }
        i = i + 1;
    }
    assert(a@ =~= b@);
    true
}

/// Check whether `resource` has `prefix` as a byte prefix.
// r[impl ucan-capability-merge.containment]
pub fn resource_has_prefix(resource: &Vec<u8>, prefix: &Vec<u8>) -> (result: bool)
    ensures result == (resource@.len() >= prefix@.len()
        && resource@.subrange(0, prefix@.len() as int) =~= prefix@),
{
    if resource.len() < prefix.len() {
        return false;
    }
    let mut i: usize = 0;
    while i < prefix.len()
        invariant
            0 <= i <= prefix.len(),
            resource.len() >= prefix.len(),
            forall|k: int| 0 <= k < i ==> resource@[k] == prefix@[k],
        decreases prefix.len() - i,
    {
        if resource[i] != prefix[i] {
            assert(resource@[i as int] != prefix@[i as int]);
            assert(!(resource@.subrange(0, prefix@.len() as int) =~= prefix@));
            return false;
        }
        i = i + 1;
    }
    assert(resource@.subrange(0, prefix@.len() as int) =~= prefix@);
    true
}

/// Check whether a parent capability contains a requested child capability.
// r[impl ucan-capability-merge.containment]
pub fn check_cap_contains(parent: &CapEntry, resource: &Vec<u8>, ability: &Vec<u8>) -> (result: bool)
    ensures result == cap_contained_spec(cap_view(parent), (resource@, ability@)),
{
    let resource_ok = resource_has_prefix(resource, &parent.resource);
    if !resource_ok {
        return false;
    }
    let ability_ok = bytes_equal(ability, &parent.ability);
    if !ability_ok {
        return false;
    }
    true
}

/// Check whether any capability in a set authorizes the requested capability.
// r[impl ucan-capability-merge.authorization]
pub fn check_authorized_by(capset: &Vec<CapEntry>, resource: &Vec<u8>, ability: &Vec<u8>) -> (result: bool)
    ensures result == authorized_by_spec(capset_view(capset), (resource@, ability@)),
{
    let mut i: usize = 0;
    while i < capset.len()
        invariant
            0 <= i <= capset.len(),
            forall|k: int| 0 <= k < i ==> !cap_contained_spec(
                (capset@[k].resource@, capset@[k].ability@),
                (resource@, ability@),
            ),
        decreases capset.len() - i,
    {
        if check_cap_contains(&capset[i], resource, ability) {
            assert(cap_contained_spec(
                (capset@[i as int].resource@, capset@[i as int].ability@),
                (resource@, ability@),
            ));
            assert(capset_view(capset)[i as int] === (capset@[i as int].resource@, capset@[i as int].ability@));
            return true;
        }
        assert(!cap_contained_spec(
            (capset@[i as int].resource@, capset@[i as int].ability@),
            (resource@, ability@),
        ));
        i = i + 1;
    }
    assert forall|k: int| 0 <= k < capset_view(capset).len() implies
        !cap_contained_spec(capset_view(capset)[k], (resource@, ability@)) by {
        assert(capset_view(capset)[k] === (capset@[k].resource@, capset@[k].ability@));
    }
    assert(!authorized_by_spec(capset_view(capset), (resource@, ability@)));
    false
}

// --- Proofs ---

/// The union contains everything from the first set.
// r[verify ucan-capability-merge.union]
pub proof fn union_contains_left(
    a: Seq<(Seq<u8>, Seq<u8>)>,
    b: Seq<(Seq<u8>, Seq<u8>)>,
    cap: (Seq<u8>, Seq<u8>),
)
    requires authorized_by_spec(a, cap),
    ensures authorized_by_spec(capset_union_spec(a, b), cap),
{
    let i = choose|i: int| 0 <= i < a.len() && cap_contained_spec(a[i], cap);
    let u = capset_union_spec(a, b);
    assert(u[i] === a[i]);
    assert(cap_contained_spec(u[i], cap));
}

/// The union contains everything from the second set.
// r[verify ucan-capability-merge.union]
pub proof fn union_contains_right(
    a: Seq<(Seq<u8>, Seq<u8>)>,
    b: Seq<(Seq<u8>, Seq<u8>)>,
    cap: (Seq<u8>, Seq<u8>),
)
    requires authorized_by_spec(b, cap),
    ensures authorized_by_spec(capset_union_spec(a, b), cap),
{
    let i = choose|i: int| 0 <= i < b.len() && cap_contained_spec(b[i], cap);
    let u = capset_union_spec(a, b);
    assert(u[a.len() + i] === b[i]);
    assert(cap_contained_spec(u[a.len() + i], cap));
}

/// Merging with an empty set is identity.
// r[verify ucan-capability-merge.authorization]
pub proof fn merge_empty_right(
    a: Seq<(Seq<u8>, Seq<u8>)>,
    cap: (Seq<u8>, Seq<u8>),
)
    requires authorized_by_spec(a, cap),
    ensures authorized_by_spec(capset_union_spec(a, Seq::<(Seq<u8>, Seq<u8>)>::empty()), cap),
{
    let i = choose|i: int| 0 <= i < a.len() && cap_contained_spec(a[i], cap);
    let u = capset_union_spec(a, Seq::<(Seq<u8>, Seq<u8>)>::empty());
    assert(u =~= a);
}

/// Containment is reflexive: a capability contains itself.
// r[verify ucan-capability-merge.containment]
pub proof fn containment_reflexive(cap: (Seq<u8>, Seq<u8>))
    ensures cap_contained_spec(cap, cap),
{
    assert(cap.0.subrange(0, cap.0.len() as int) =~= cap.0);
}

/// Containment is transitive.
pub proof fn containment_transitive(
    parent: (Seq<u8>, Seq<u8>),
    middle: (Seq<u8>, Seq<u8>),
    child: (Seq<u8>, Seq<u8>),
)
    requires
        cap_contained_spec(parent, middle),
        cap_contained_spec(middle, child),
    ensures
        cap_contained_spec(parent, child),
{
    assert(child.0.len() >= middle.0.len());
    assert(middle.0.len() >= parent.0.len());
    assert(child.0.len() >= parent.0.len());
    assert(child.0.subrange(0, parent.0.len() as int) =~=
           middle.0.subrange(0, parent.0.len() as int));
    assert(child.1 =~= middle.1);
    assert(middle.1 =~= parent.1);
}

/// Union is associative for authorization.
pub proof fn union_associative_auth(
    a: Seq<(Seq<u8>, Seq<u8>)>,
    b: Seq<(Seq<u8>, Seq<u8>)>,
    c: Seq<(Seq<u8>, Seq<u8>)>,
    cap: (Seq<u8>, Seq<u8>),
)
    requires authorized_by_spec(capset_union_spec(capset_union_spec(a, b), c), cap),
    ensures authorized_by_spec(capset_union_spec(a, capset_union_spec(b, c)), cap),
{
    let u_ab = capset_union_spec(a, b);
    let u_abc = capset_union_spec(u_ab, c);
    let i = choose|i: int| 0 <= i < u_abc.len() && cap_contained_spec(u_abc[i], cap);

    let u_bc = capset_union_spec(b, c);
    let u_a_bc = capset_union_spec(a, u_bc);

    if i < a.len() {
        assert(u_a_bc[i] === a[i]);
        assert(cap_contained_spec(u_a_bc[i], cap));
    } else if i < a.len() + b.len() {
        let bi = i - a.len();
        assert(u_a_bc[a.len() + bi] === b[bi]);
        assert(u_abc[i] === b[bi]);
        assert(cap_contained_spec(u_a_bc[a.len() + bi], cap));
    } else {
        let ci = i - a.len() - b.len();
        assert(u_abc[i] === c[ci]);
        assert(u_bc[b.len() + ci] === c[ci]);
        assert(u_a_bc[a.len() + b.len() + ci] === c[ci]);
        assert(cap_contained_spec(u_a_bc[a.len() + b.len() + ci], cap));
    }
}

} // verus!

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    fn cap(resource: &[u8], ability: &[u8]) -> CapEntry {
        CapEntry {
            resource: resource.to_vec(),
            ability: ability.to_vec(),
        }
    }

    #[test]
    fn test_merge_empty() {
        let a: Vec<CapEntry> = vec![];
        let b = vec![cap(b"/photos", b"read")];
        let merged = merge_capsets(&a, &b);
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn test_merge_two_sets() {
        let a = vec![cap(b"/photos", b"read"), cap(b"/docs", b"write")];
        let b = vec![cap(b"/mail", b"send")];
        let merged = merge_capsets(&a, &b);
        assert_eq!(merged.len(), 3);
    }

    #[test]
    fn test_merge_preserves_content() {
        let a = vec![cap(b"/photos", b"read")];
        let b = vec![cap(b"/docs", b"write")];
        let merged = merge_capsets(&a, &b);
        assert_eq!(merged[0].resource, b"/photos");
        assert_eq!(merged[0].ability, b"read");
        assert_eq!(merged[1].resource, b"/docs");
        assert_eq!(merged[1].ability, b"write");
    }

    #[test]
    fn test_merge_both_empty() {
        let a: Vec<CapEntry> = vec![];
        let b: Vec<CapEntry> = vec![];
        let merged = merge_capsets(&a, &b);
        assert_eq!(merged.len(), 0);
    }

    #[test]
    fn test_authorized_by_accepts_left_and_right_caps() {
        let a = vec![cap(b"/photos", b"read")];
        let b = vec![cap(b"/docs", b"write")];
        let merged = merge_capsets(&a, &b);
        assert!(check_authorized_by(&merged, &b"/photos/vacation".to_vec(), &b"read".to_vec()));
        assert!(check_authorized_by(&merged, &b"/docs/spec".to_vec(), &b"write".to_vec()));
    }

    #[test]
    fn test_authorized_by_rejects_resource_and_ability_escalation() {
        let caps = vec![cap(b"/photos", b"read"), cap(b"/docs", b"write")];
        assert!(!check_authorized_by(&caps, &b"/videos".to_vec(), &b"read".to_vec()));
        assert!(!check_authorized_by(&caps, &b"/photos/vacation".to_vec(), &b"write".to_vec()));
    }

    #[test]
    fn test_large_merge() {
        let a = vec![cap(b"/a", b"r"), cap(b"/b", b"r"), cap(b"/c", b"r")];
        let b = vec![cap(b"/d", b"w"), cap(b"/e", b"w")];
        let merged = merge_capsets(&a, &b);
        assert_eq!(merged.len(), 5);
    }
}
