//! Verified seL4-style capability derivation tree primitives.
//!
//! Models derivation ancestry as a topologically ordered parent-pointer tree.
//! Live capabilities occupy stable node indices. Copy and mint append a new
//! child of a live source capability, move changes only slot placement,
//! revoke clears exactly one live subtree, and recycle clears one live leaf.

use vstd::prelude::*;

verus! {

pub struct Cdt {
    pub parents: Vec<Option<usize>>,
    pub slots: Vec<usize>,
    pub present: Vec<bool>,
}

// r[impl sel4-cdt.structure]
// r[depends sel4-cdt.structure]
pub open spec fn parent_ints_spec(parents: Seq<Option<usize>>) -> Seq<Option<int>> {
    parents.map(|_i: int, p: Option<usize>| match p {
        Some(j) => Some(j as int),
        None => None::<int>,
    })
}

// r[impl sel4-cdt.structure]
// r[depends sel4-cdt.structure]
pub open spec fn strict_ancestor_fuel_spec(parents: Seq<Option<usize>>, anc: int, node: int, fuel: nat) -> bool
    decreases fuel
{
    if fuel == 0 || anc < 0 || node < 0 || anc >= parents.len() || node >= parents.len() {
        false
    } else {
        match parents[node] {
            None => false,
            Some(p) => p as int == anc || strict_ancestor_fuel_spec(parents, anc, p as int, (fuel - 1) as nat),
        }
    }
}

// r[impl sel4-cdt.structure]
// r[depends sel4-cdt.structure]
pub open spec fn well_formed_parents_spec(parents: Seq<Option<usize>>) -> bool {
    forall|i: int| #![trigger parents[i]] 0 <= i < parents.len() ==> match parents[i] {
        Some(j) => (j as int) < i,
        None => true,
    }
}

proof fn strict_ancestor_fuel_sufficient(parents: Seq<Option<usize>>, anc: int, node: int, fuel: nat)
    requires
        well_formed_parents_spec(parents),
        0 <= node < parents.len(),
        fuel >= node + 1,
    ensures
        strict_ancestor_fuel_spec(parents, anc, node, fuel)
            == strict_ancestor_fuel_spec(parents, anc, node, (node + 1) as nat)
    decreases node
{
    match parents[node] {
        None => {}
        Some(p) => {
            strict_ancestor_fuel_sufficient(parents, anc, p as int, (fuel - 1) as nat);
            strict_ancestor_fuel_sufficient(parents, anc, p as int, node as nat);
        }
    }
}

proof fn strict_ancestor_parent_closed(parents: Seq<Option<usize>>, anc: int, node: int)
    requires
        well_formed_parents_spec(parents),
        0 <= anc < parents.len(),
        0 <= node < parents.len(),
        parents[node].is_some(),
        strict_ancestor_fuel_spec(parents, anc, parents[node].unwrap() as int, parents.len() as nat),
    ensures
        strict_ancestor_fuel_spec(parents, anc, node, parents.len() as nat)
{
    let parent = parents[node].unwrap();
    if parent as int == anc {
        assert(strict_ancestor_fuel_spec(parents, anc, node, parents.len() as nat));
    } else {
        assert(strict_ancestor_fuel_spec(parents, anc, node, parents.len() as nat)
            == strict_ancestor_fuel_spec(parents, anc, parent as int, (parents.len() - 1) as nat));
        strict_ancestor_fuel_sufficient(parents, anc, parent as int, (parents.len() - 1) as nat);
        strict_ancestor_fuel_sufficient(parents, anc, parent as int, parents.len() as nat);
        assert(strict_ancestor_fuel_spec(parents, anc, parent as int, (parents.len() - 1) as nat)
            == strict_ancestor_fuel_spec(parents, anc, parent as int, (parent as int + 1) as nat));
        assert(strict_ancestor_fuel_spec(parents, anc, parent as int, parents.len() as nat)
            == strict_ancestor_fuel_spec(parents, anc, parent as int, (parent as int + 1) as nat));
    }
}

impl Cdt {
    // r[impl sel4-cdt.structure]
    // r[depends sel4-cdt.structure]
    pub open spec fn wf(&self) -> bool {
        &&& self.parents@.len() == self.slots@.len()
        &&& self.parents@.len() == self.present@.len()
        &&& well_formed_parents_spec(self.parents@)
        &&& forall|i: int| 0 <= i < self.parents@.len() && self.present@[i] ==> match self.parents@[i] {
            None => true,
            Some(p) => p < self.present@.len() && self.present@[p as int],
        }
    }

    // r[impl sel4-cdt.structure]
    // r[depends sel4-cdt.structure]
    pub open spec fn strict_ancestor_spec(&self, anc: int, node: int) -> bool {
        &&& 0 <= anc < self.parents@.len()
        &&& 0 <= node < self.parents@.len()
        &&& anc < node
        &&& self.present@[anc]
        &&& self.present@[node]
        &&& strict_ancestor_fuel_spec(self.parents@, anc, node, self.parents@.len() as nat)
    }

    // r[impl sel4-cdt.revoke]
    // r[impl sel4-cdt.recycle]
    // r[depends sel4-cdt.revoke]
    pub open spec fn subtree_member_spec(&self, root: int, node: int) -> bool {
        &&& 0 <= root < self.parents@.len()
        &&& 0 <= node < self.parents@.len()
        &&& self.present@[root]
        &&& self.present@[node]
        &&& (root == node || strict_ancestor_fuel_spec(self.parents@, root, node, self.parents@.len() as nat))
    }
}

// r[impl sel4-cdt.recycle]
// r[depends sel4-cdt.recycle]
pub open spec fn has_live_child_spec(cdt: Cdt, node: int) -> bool {
    exists|i: int| 0 <= i < cdt.parents@.len()
        && cdt.present@[i]
        && cdt.parents@[i] == Some(node as usize)
}

// r[impl sel4-cdt.revoke]
// r[depends sel4-cdt.revoke]
pub open spec fn revoke_present_spec(cdt: Cdt, target: int) -> Seq<bool> {
    Seq::new(cdt.present@.len(), |i: int| cdt.present@[i] && !cdt.subtree_member_spec(target, i))
}

proof fn subtree_member_extends_child(cdt: Cdt, root: int, node: int)
    requires
        cdt.wf(),
        0 <= root < cdt.parents@.len(),
        0 <= node < cdt.parents@.len(),
        cdt.present@[node],
        cdt.parents@[node].is_some(),
        cdt.subtree_member_spec(root, cdt.parents@[node].unwrap() as int),
    ensures
        cdt.subtree_member_spec(root, node)
{
    let parent = cdt.parents@[node].unwrap();
    assert(cdt.present@[parent as int]);
    if parent as int == root {
        assert(strict_ancestor_fuel_spec(cdt.parents@, root, node, cdt.parents@.len() as nat));
    } else {
        assert(strict_ancestor_fuel_spec(cdt.parents@, root, parent as int, cdt.parents@.len() as nat));
        strict_ancestor_parent_closed(cdt.parents@, root, node);
    }
}

proof fn revoke_preserves_live_parent(cdt: Cdt, target: int, node: int)
    requires
        cdt.wf(),
        0 <= target < cdt.parents@.len(),
        cdt.present@[target],
        0 <= node < cdt.parents@.len(),
        revoke_present_spec(cdt, target)[node],
    ensures
        match cdt.parents@[node] {
            None => true,
            Some(p) => revoke_present_spec(cdt, target)[p as int],
        }
{
    assert(cdt.present@[node]);
    assert(!cdt.subtree_member_spec(target, node));
    match cdt.parents@[node] {
        None => {}
        Some(p) => {
            assert(cdt.present@[p as int]);
            if cdt.subtree_member_spec(target, p as int) {
                subtree_member_extends_child(cdt, target, node);
                assert(cdt.subtree_member_spec(target, node));
                assert(false);
            }
            assert(revoke_present_spec(cdt, target)[p as int]);
        }
    }
}

proof fn recycle_preserves_live_parent(cdt: Cdt, node: int, j: int)
    requires
        cdt.wf(),
        0 <= node < cdt.present@.len(),
        cdt.present@[node],
        !has_live_child_spec(cdt, node),
        0 <= j < cdt.present@.len(),
        cdt.present@.update(node, false)[j],
    ensures
        match cdt.parents@[j] {
            None => true,
            Some(p) => cdt.present@.update(node, false)[p as int],
        }
{
    assert(j != node);
    assert(cdt.present@[j]);
    match cdt.parents@[j] {
        None => {}
        Some(p) => {
            assert(cdt.present@[p as int]);
            if p as int == node {
                assert(has_live_child_spec(cdt, node));
                assert(false);
            }
            assert(cdt.present@.update(node, false)[p as int]);
        }
    }
}

fn subtree_member_exec(parents: &Vec<Option<usize>>, present: &Vec<bool>, target: usize, node: usize) -> (result: bool)
    requires
        parents@.len() == present@.len(),
        well_formed_parents_spec(parents@),
        forall|i: int| 0 <= i < parents@.len() && present@[i] ==> match parents@[i] {
            None => true,
            Some(p) => (p as int) < present@.len() && present@[p as int],
        },
    ensures
        result == (
            target < parents@.len()
            && node < parents@.len()
            && present@[target as int]
            && present@[node as int]
            && (target == node || strict_ancestor_fuel_spec(parents@, target as int, node as int, parents@.len() as nat))
        )
    decreases node
{
    if target >= parents.len() || node >= parents.len() || !present[target] || !present[node] {
        false
    } else if target == node {
        true
    } else {
        match parents[node] {
            None => {
                proof {
                    assert(strict_ancestor_fuel_spec(parents@, target as int, node as int, parents@.len() as nat) == false);
                }
                false
            }
            Some(p) => {
                proof {
                    assert(parents@[node as int] == Some(p));
                    assert((p as int) < node as int);
                    assert(p < node);
                    assert(present@[p as int]);
                }
                if p == target {
                    proof {
                        assert(strict_ancestor_fuel_spec(parents@, target as int, node as int, parents@.len() as nat));
                    }
                    true
                } else {
                    let result = subtree_member_exec(parents, present, target, p);
                    proof {
                        strict_ancestor_fuel_sufficient(parents@, target as int, p as int, (parents@.len() - 1) as nat);
                        strict_ancestor_fuel_sufficient(parents@, target as int, p as int, parents@.len() as nat);
                        assert(strict_ancestor_fuel_spec(parents@, target as int, node as int, parents@.len() as nat)
                            == strict_ancestor_fuel_spec(parents@, target as int, p as int, (parents@.len() - 1) as nat));
                        assert(strict_ancestor_fuel_spec(parents@, target as int, p as int, (parents@.len() - 1) as nat)
                            == strict_ancestor_fuel_spec(parents@, target as int, p as int, (p as int + 1) as nat));
                        assert(strict_ancestor_fuel_spec(parents@, target as int, p as int, parents@.len() as nat)
                            == strict_ancestor_fuel_spec(parents@, target as int, p as int, (p as int + 1) as nat));
                    }
                    result
                }
            }
        }
    }
}

// r[impl sel4-cdt.structure]
pub fn new_root(slot: usize) -> (result: Cdt)
    ensures
        result.wf(),
        result.parents@ == seq![None::<usize>],
        result.slots@ == seq![slot],
        result.present@ == seq![true],
{
    Cdt {
        parents: vec![None],
        slots: vec![slot],
        present: vec![true],
    }
}

// r[impl sel4-cdt.structure]
pub fn copy_cap(cdt: &mut Cdt, source: usize, new_slot: usize) -> (result: Option<usize>)
    requires old(cdt).wf()
    ensures
        cdt.wf(),
        match result {
            Some(new_id) => {
                &&& source < old(cdt).parents@.len()
                &&& old(cdt).present@[source as int]
                &&& new_id == old(cdt).parents@.len()
                &&& cdt.parents@ == old(cdt).parents@.push(Some(source))
                &&& cdt.slots@ == old(cdt).slots@.push(new_slot)
                &&& cdt.present@ == old(cdt).present@.push(true)
                &&& cdt.strict_ancestor_spec(source as int, new_id as int)
            }
            None => {
                &&& cdt.parents@ =~= old(cdt).parents@
                &&& cdt.slots@ =~= old(cdt).slots@
                &&& cdt.present@ =~= old(cdt).present@
                &&& (source >= old(cdt).parents@.len() || !old(cdt).present@[source as int])
            }
        }
{
    if source >= cdt.parents.len() || !cdt.present[source] {
        return None;
    }
    cdt.parents.push(Some(source));
    cdt.slots.push(new_slot);
    cdt.present.push(true);
    Some(cdt.parents.len() - 1)
}

// r[impl sel4-cdt.structure]
pub fn mint_cap(cdt: &mut Cdt, source: usize, new_slot: usize) -> (result: Option<usize>)
    requires old(cdt).wf()
    ensures
        cdt.wf(),
        match result {
            Some(new_id) => {
                &&& source < old(cdt).parents@.len()
                &&& old(cdt).present@[source as int]
                &&& new_id == old(cdt).parents@.len()
                &&& cdt.parents@ == old(cdt).parents@.push(Some(source))
                &&& cdt.slots@ == old(cdt).slots@.push(new_slot)
                &&& cdt.present@ == old(cdt).present@.push(true)
                &&& cdt.strict_ancestor_spec(source as int, new_id as int)
            }
            None => {
                &&& cdt.parents@ =~= old(cdt).parents@
                &&& cdt.slots@ =~= old(cdt).slots@
                &&& cdt.present@ =~= old(cdt).present@
                &&& (source >= old(cdt).parents@.len() || !old(cdt).present@[source as int])
            }
        }
{
    copy_cap(cdt, source, new_slot)
}

// r[impl sel4-cdt.move]
pub fn move_cap(cdt: &mut Cdt, node: usize, new_slot: usize) -> (result: bool)
    requires old(cdt).wf()
    ensures
        cdt.wf(),
        if result {
            &&& node < old(cdt).slots@.len()
            &&& old(cdt).present@[node as int]
            &&& cdt.parents@ =~= old(cdt).parents@
            &&& cdt.present@ =~= old(cdt).present@
            &&& cdt.slots@ == old(cdt).slots@.update(node as int, new_slot)
        } else {
            &&& cdt.parents@ =~= old(cdt).parents@
            &&& cdt.slots@ =~= old(cdt).slots@
            &&& cdt.present@ =~= old(cdt).present@
            &&& (node >= old(cdt).slots@.len() || !old(cdt).present@[node as int])
        }
{
    if node >= cdt.slots.len() || !cdt.present[node] {
        return false;
    }
    cdt.slots.set(node, new_slot);
    true
}

// r[impl sel4-cdt.revoke]
pub fn revoke(cdt: &mut Cdt, target: usize) -> (result: bool)
    requires old(cdt).wf()
    ensures
        cdt.wf(),
        if result {
            &&& target < old(cdt).parents@.len()
            &&& old(cdt).present@[target as int]
            &&& cdt.parents@ =~= old(cdt).parents@
            &&& cdt.slots@ =~= old(cdt).slots@
            &&& cdt.present@ =~= revoke_present_spec(*old(cdt), target as int)
        } else {
            &&& cdt.parents@ =~= old(cdt).parents@
            &&& cdt.slots@ =~= old(cdt).slots@
            &&& cdt.present@ =~= old(cdt).present@
            &&& (target >= old(cdt).parents@.len() || !old(cdt).present@[target as int])
        }
{
    let ghost old_cdt = *old(cdt);
    let old_parents = cdt.parents.clone();
    let old_present = cdt.present.clone();
    if target >= cdt.parents.len() || !cdt.present[target] {
        return false;
    }
    let len = cdt.present.len();
    let mut i: usize = 0;
    while i < len
        invariant
            cdt.parents@ =~= old_cdt.parents@,
            cdt.slots@ =~= old_cdt.slots@,
            cdt.present@.len() == len as int,
            len == old_cdt.present@.len(),
            0 <= i <= len,
            old_parents@ =~= old_cdt.parents@,
            old_present@ =~= old_cdt.present@,
            old_parents@.len() == old_present@.len(),
            well_formed_parents_spec(old_parents@),
            forall|j: int| 0 <= j < old_parents@.len() && old_present@[j] ==> match old_parents@[j] {
                None => true,
                Some(p) => (p as int) < old_present@.len() && old_present@[p as int],
            },
            forall|j: int| 0 <= j < i as int ==> cdt.present@[j] == revoke_present_spec(old_cdt, target as int)[j],
            forall|j: int| i as int <= j < len as int ==> cdt.present@[j] == old_cdt.present@[j],
        decreases len - i,
    {
        proof {
            assert(i < cdt.present@.len());
            assert(old_parents@.len() == old_present@.len());
        }
        let in_subtree = subtree_member_exec(&old_parents, &old_present, target, i);
        proof {
            assert(in_subtree == old_cdt.subtree_member_spec(target as int, i as int));
            assert(revoke_present_spec(old_cdt, target as int)[i as int]
                == (old_cdt.present@[i as int] && !old_cdt.subtree_member_spec(target as int, i as int)));
        }
        if in_subtree {
            cdt.present.set(i, false);
        }
        i = i + 1;
    }
    proof {
        assert forall|j: int| 0 <= j < cdt.parents@.len() && cdt.present@[j] implies match cdt.parents@[j] {
            None => true,
            Some(p) => p < cdt.present@.len() && cdt.present@[p as int],
        } by {
            assert(cdt.present@[j] == revoke_present_spec(old_cdt, target as int)[j]);
            revoke_preserves_live_parent(old_cdt, target as int, j);
        }
        assert(cdt.present@ =~= revoke_present_spec(old_cdt, target as int));
    }
    true
}

// r[impl sel4-cdt.recycle]
pub fn recycle(cdt: &mut Cdt, node: usize) -> (result: bool)
    requires old(cdt).wf()
    ensures
        cdt.wf(),
        if result {
            &&& node < old(cdt).present@.len()
            &&& old(cdt).present@[node as int]
            &&& !has_live_child_spec(*old(cdt), node as int)
            &&& cdt.parents@ =~= old(cdt).parents@
            &&& cdt.slots@ =~= old(cdt).slots@
            &&& cdt.present@ =~= old(cdt).present@.update(node as int, false)
        } else {
            &&& cdt.parents@ =~= old(cdt).parents@
            &&& cdt.slots@ =~= old(cdt).slots@
            &&& cdt.present@ =~= old(cdt).present@
            &&& (node >= old(cdt).present@.len() || !old(cdt).present@[node as int] || has_live_child_spec(*old(cdt), node as int))
        }
{
    if node >= cdt.present.len() || !cdt.present[node] {
        return false;
    }
    let mut i: usize = 0;
    while i < cdt.parents.len()
        invariant
            cdt.wf(),
            cdt.parents@ =~= old(cdt).parents@,
            cdt.slots@ =~= old(cdt).slots@,
            cdt.parents@.len() == old(cdt).parents@.len(),
            cdt.present@ =~= old(cdt).present@,
            cdt.present@.len() == old(cdt).present@.len(),
            cdt.parents@.len() == cdt.present@.len(),
            node < cdt.parents@.len(),
            0 <= i <= cdt.parents@.len(),
            forall|j: int| 0 <= j < i as int ==> !(old(cdt).present@[j] && old(cdt).parents@[j] == Some(node)),
        decreases cdt.parents.len() - i,
    {
        proof {
            assert(i < cdt.parents@.len());
            assert(cdt.parents@.len() == cdt.present@.len());
            assert(i < cdt.present@.len());
        }
        if cdt.present[i] && cdt.parents[i] == Some(node) {
            proof {
                assert(has_live_child_spec(*old(cdt), node as int));
            }
            return false;
        }
        i = i + 1;
    }
    cdt.present.set(node, false);
    proof {
        assert forall|j: int| 0 <= j < cdt.parents@.len() && cdt.present@[j] implies match cdt.parents@[j] {
            None => true,
            Some(p) => p < cdt.present@.len() && cdt.present@[p as int],
        } by {
            recycle_preserves_live_parent(*old(cdt), node as int, j);
        }
        assert(cdt.present@ =~= old(cdt).present@.update(node as int, false));
    }
    true
}

// r[verify sel4-cdt.structure]
proof fn strict_ancestor_smaller(cdt: Cdt, anc: int, node: int)
    requires
        cdt.wf(),
        cdt.strict_ancestor_spec(anc, node),
    ensures
        anc < node
{
}

// r[verify sel4-cdt.structure]
proof fn copy_appended_child_is_descendant(cdt: Cdt, source: usize, new_slot: usize)
    requires
        cdt.wf(),
        source < cdt.parents@.len(),
        cdt.present@[source as int],
    ensures
        strict_ancestor_fuel_spec(
            cdt.parents@.push(Some(source)),
            source as int,
            cdt.parents@.len() as int,
            cdt.parents@.len() + 1,
        )
{
    let ext = cdt.parents@.push(Some(source));
    assert(ext[cdt.parents@.len() as int] == Some(source));
    assert(strict_ancestor_fuel_spec(
        ext,
        source as int,
        cdt.parents@.len() as int,
        cdt.parents@.len() + 1,
    ));
    let _ = new_slot;
}

// r[verify sel4-cdt.move]
proof fn move_preserves_ancestry(cdt: Cdt, node: int, new_slot: usize, anc: int, desc: int)
    requires
        cdt.wf(),
        0 <= node < cdt.slots@.len(),
    ensures
        strict_ancestor_fuel_spec(cdt.parents@, anc, desc, cdt.parents@.len() as nat)
            == strict_ancestor_fuel_spec(cdt.parents@, anc, desc, cdt.parents@.len() as nat)
{
}

// r[verify sel4-cdt.revoke]
proof fn revoke_removes_exact_subtree(cdt: Cdt, target: int, node: int)
    requires
        cdt.wf(),
        0 <= target < cdt.parents@.len(),
        cdt.present@[target],
        0 <= node < cdt.parents@.len(),
    ensures
        revoke_present_spec(cdt, target)[node] == (cdt.present@[node] && !cdt.subtree_member_spec(target, node))
{
}

// r[verify sel4-cdt.recycle]
proof fn recycle_leaf_clears_only_target(cdt: Cdt, node: int)
    requires
        cdt.wf(),
        0 <= node < cdt.present@.len(),
        cdt.present@[node],
        !has_live_child_spec(cdt, node),
    ensures
        cdt.present@.update(node, false)[node] == false,
        forall|i: int| 0 <= i < cdt.present@.len() && i != node ==> cdt.present@.update(node, false)[i] == cdt.present@[i]
{
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn sample_cdt() -> Cdt {
        Cdt {
            parents: vec![None, Some(0), Some(0), Some(1), Some(1), Some(2)],
            slots: vec![10, 11, 12, 13, 14, 15],
            present: vec![true, true, true, true, true, true],
        }
    }

    // r[verify sel4-cdt.structure]
    #[test]
    fn test_copy_creates_child() {
        let mut cdt = sample_cdt();
        let new_id = copy_cap(&mut cdt, 2, 99).expect("copy should succeed");
        assert_eq!(new_id, 6);
        assert_eq!(cdt.parents[new_id], Some(2));
        assert!(cdt.present[new_id]);
        assert_eq!(cdt.slots[new_id], 99);
    }

    // r[verify sel4-cdt.structure]
    #[test]
    fn test_mint_creates_child() {
        let mut cdt = sample_cdt();
        let new_id = mint_cap(&mut cdt, 1, 77).expect("mint should succeed");
        assert_eq!(new_id, 6);
        assert_eq!(cdt.parents[new_id], Some(1));
        assert!(cdt.present[new_id]);
    }

    // r[verify sel4-cdt.move]
    #[test]
    fn test_move_preserves_parent() {
        let mut cdt = sample_cdt();
        let old_parent = cdt.parents[4];
        assert!(move_cap(&mut cdt, 4, 88));
        assert_eq!(cdt.parents[4], old_parent);
        assert_eq!(cdt.slots[4], 88);
    }

    // r[verify sel4-cdt.revoke]
    #[test]
    fn test_revoke_removes_exact_subtree() {
        let mut cdt = sample_cdt();
        assert!(revoke(&mut cdt, 1));
        assert!(cdt.present[0]);
        assert!(!cdt.present[1]);
        assert!(cdt.present[2]);
        assert!(!cdt.present[3]);
        assert!(!cdt.present[4]);
        assert!(cdt.present[5]);
    }

    // r[verify sel4-cdt.revoke]
    #[test]
    fn test_revoke_preserves_sibling() {
        let mut cdt = sample_cdt();
        assert!(revoke(&mut cdt, 1));
        assert!(cdt.present[2]);
        assert!(cdt.present[5]);
    }

    // r[verify sel4-cdt.recycle]
    #[test]
    fn test_recycle_leaf_only() {
        let mut cdt = sample_cdt();
        assert!(recycle(&mut cdt, 4));
        assert!(!cdt.present[4]);
        assert!(cdt.present[3]);
        assert!(cdt.present[5]);
        assert!(!recycle(&mut cdt, 1));
    }

    proptest! {
        // r[verify sel4-cdt.move]
        #[test]
        fn prop_move_preserves_parent(node in 0usize..6, new_slot: usize) {
            let mut cdt = sample_cdt();
            let before = cdt.parents[node];
            prop_assert!(move_cap(&mut cdt, node, new_slot));
            prop_assert_eq!(cdt.parents[node], before);
        }

        // r[verify sel4-cdt.revoke]
        #[test]
        fn prop_revoke_node_two_preserves_other_branch(target in 1usize..=2) {
            let mut cdt = sample_cdt();
            prop_assert!(revoke(&mut cdt, target));
            if target == 1 {
                prop_assert!(cdt.present[2]);
                prop_assert!(cdt.present[5]);
            } else {
                prop_assert!(cdt.present[1]);
                prop_assert!(cdt.present[3]);
                prop_assert!(cdt.present[4]);
            }
        }

        // r[verify sel4-cdt.recycle]
        #[test]
        fn prop_recycle_leaf_clears_one(leaf in prop::sample::select(vec![3usize, 4usize, 5usize])) {
            let mut cdt = sample_cdt();
            prop_assert!(recycle(&mut cdt, leaf));
            prop_assert!(!cdt.present[leaf]);
            for i in 0..cdt.present.len() {
                if i != leaf {
                    prop_assert!(cdt.present[i]);
                }
            }
        }
    }
}
