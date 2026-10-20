//! Verified cryptoshedding core.
//!
//! Models a parent-indexed key forest plus a keystore of currently present
//! keys. Data are represented only by their bound leaf key.

#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_possible_wrap)]

use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::set::axiom_set_contains_len;
#[cfg(verus_keep_ghost)]
use vstd::set::axiom_set_empty_finite;
#[cfg(verus_keep_ghost)]
use vstd::set::axiom_set_empty_len;
#[cfg(verus_keep_ghost)]
use vstd::set::axiom_set_insert_finite;
#[cfg(verus_keep_ghost)]
use vstd::set::axiom_set_insert_len;
#[cfg(verus_keep_ghost)]
use vstd::set::axiom_set_remove_finite;
#[cfg(verus_keep_ghost)]
use vstd::set::axiom_set_remove_len;

verus! {

pub struct KeyTree {
    pub parents: Vec<Option<u64>>,
}

pub struct Keystore {
    pub present: Vec<bool>,
}

// r[impl cryptoshedding.shed-definition]
// r[impl cryptoshedding.chain-recoverability]
pub open spec fn keystore_model_from_bits(bits: Seq<bool>) -> Set<u64> {
    Set::new(|k: u64| 0 <= (k as int) < bits.len() && bits[k as int])
}

impl KeyTree {
    // r[impl cryptoshedding.tree-well-formedness]
    // r[impl cryptoshedding.rotation-correctness]
    pub open spec fn view(&self) -> Seq<Option<u64>> {
        self.parents@
    }
}

impl Keystore {
    // r[impl cryptoshedding.shed-definition]
    // r[impl cryptoshedding.chain-recoverability]
    pub open spec fn view(&self) -> Set<u64> {
        keystore_model_from_bits(self.present@)
    }
}

// r[impl cryptoshedding.tree-well-formedness]
pub open spec fn on_chain_fuel_spec(
    tree: Seq<Option<u64>>, node: u64, ancestor: u64, fuel: nat,
) -> bool
    decreases fuel,
{
    if fuel == 0 || !((node as int) < tree.len()) || !((ancestor as int) < tree.len()) {
        false
    } else {
        match tree[node as int] {
            Some(parent) => parent == ancestor
                || on_chain_fuel_spec(tree, parent, ancestor, (fuel - 1) as nat),
            None => false,
        }
    }
}

// r[impl cryptoshedding.tree-well-formedness]
// r[impl cryptoshedding.shed-completeness]
// r[impl cryptoshedding.shed-isolation]
/// Strict ancestor relation: one-or-more parent links from `node` to `ancestor`.
pub open spec fn on_chain_spec(tree: Seq<Option<u64>>, node: u64, ancestor: u64) -> bool {
    on_chain_fuel_spec(tree, node, ancestor, tree.len())
}

// r[impl cryptoshedding.tree-well-formedness]
pub open spec fn parent_walk_reaches_spec(tree: Seq<Option<u64>>, node: u64, ancestor: u64) -> bool {
    &&& 0 <= (node as int) < tree.len()
    &&& 0 <= (ancestor as int) < tree.len()
    &&& exists|fuel: nat| fuel <= tree.len() && on_chain_fuel_spec(tree, node, ancestor, fuel)
}

pub open spec fn reaches_root_fuel_spec(tree: Seq<Option<u64>>, node: u64, fuel: nat) -> bool
    decreases fuel,
{
    if fuel == 0 || !((node as int) < tree.len()) {
        false
    } else {
        match tree[node as int] {
            None => true,
            Some(parent) => reaches_root_fuel_spec(tree, parent, (fuel - 1) as nat),
        }
    }
}

spec fn nodes_upto_spec(n: nat) -> Set<u64> {
    Set::new(|k: u64| (k as int) < n)
}

proof fn nodes_upto_finite_len(n: nat)
    requires
        n <= u64::MAX as nat,
    ensures
        nodes_upto_spec(n).finite(),
        nodes_upto_spec(n).len() == n,
    decreases n,
{
    if n == 0 {
        assert(nodes_upto_spec(n) =~= Set::<u64>::empty()) by {
            assert forall|k: u64| nodes_upto_spec(n).contains(k) == Set::<u64>::empty().contains(k) by {
            }
        }
        assert(Set::<u64>::empty().finite()) by {
            broadcast use axiom_set_empty_finite;
        }
        assert(Set::<u64>::empty().len() == 0) by {
            broadcast use axiom_set_empty_len;
        }
    } else {
        nodes_upto_finite_len((n - 1) as nat);
        let prev = nodes_upto_spec((n - 1) as nat);
        let last = (n - 1) as u64;
        assert(nodes_upto_spec(n) =~= prev.insert(last)) by {
            assert forall|k: u64| nodes_upto_spec(n).contains(k) == prev.insert(last).contains(k) by {
                if k == last {
                } else {
                    assert(((k as int) < n) == ((k as int) < (n - 1) as nat));
                }
            }
        }
        axiom_set_insert_finite(prev, last);
        axiom_set_insert_len(prev, last);
    }
}

proof fn reaches_root_with_budget(tree: Seq<Option<u64>>, node: u64, budget: Set<u64>)
    requires
        tree.len() <= u64::MAX as nat,
        well_formed_spec(tree),
        0 <= (node as int) < tree.len(),
        budget.finite(),
        budget.contains(node),
        budget.len() <= tree.len(),
        forall|v: u64| budget.contains(v) ==> 0 <= (v as int) < tree.len(),
        forall|a: u64| 0 <= (a as int) < tree.len() && !budget.contains(a)
            ==> !on_chain_fuel_spec(tree, node, a, budget.len()),
    ensures
        reaches_root_fuel_spec(tree, node, budget.len()),
    decreases budget.len(),
{
    axiom_set_contains_len(budget, node);
    if budget.len() == 0 {
        assert(false);
    }
    match tree[node as int] {
        None => {
            assert(reaches_root_fuel_spec(tree, node, budget.len()));
        }
        Some(parent) => {
            assert(0 <= (parent as int) < tree.len());
            if parent == node {
                assert(on_chain_fuel_spec(tree, node, node, 1));
                on_chain_true_monotone(tree, node, node, 1, tree.len());
                on_chain_strict_on_well_formed(tree, node);
                assert(false);
            }
            if !budget.contains(parent) {
                assert(!on_chain_fuel_spec(tree, node, parent, budget.len()));
                assert(on_chain_fuel_spec(tree, node, parent, budget.len())
                    == (parent == parent || on_chain_fuel_spec(tree, parent, parent, (budget.len() - 1) as nat)));
                assert(on_chain_fuel_spec(tree, node, parent, budget.len()));
                assert(false) by {
                    assert(!on_chain_fuel_spec(tree, node, parent, budget.len()));
                    assert(on_chain_fuel_spec(tree, node, parent, budget.len()));
                }
            }
            let next_budget = budget.remove(node);
            axiom_set_remove_finite(budget, node);
            axiom_set_remove_len(budget, node);
            assert(next_budget.len() + 1 == budget.len());
            assert(next_budget.contains(parent));
            assert(next_budget.len() < budget.len());
            assert forall|v: u64| next_budget.contains(v) implies 0 <= (v as int) < tree.len() by {
            }
            assert forall|a: u64| 0 <= (a as int) < tree.len() && !next_budget.contains(a)
                implies !on_chain_fuel_spec(tree, parent, a, next_budget.len())
            by {
                if 0 <= (a as int) < tree.len() && !next_budget.contains(a) {
                    if a == node {
                        if on_chain_fuel_spec(tree, parent, a, next_budget.len()) {
                            assert(on_chain_fuel_spec(tree, node, node, budget.len())
                                == (parent == node || on_chain_fuel_spec(tree, parent, node, (budget.len() - 1) as nat)));
                            assert(on_chain_fuel_spec(tree, node, node, budget.len()));
                            on_chain_true_monotone(tree, node, node, budget.len(), tree.len());
                            on_chain_strict_on_well_formed(tree, node);
                            assert(false);
                        }
                    } else {
                        assert(!budget.contains(a));
                        assert(!on_chain_fuel_spec(tree, node, a, budget.len()));
                        assert(on_chain_fuel_spec(tree, node, a, budget.len())
                            == (parent == a || on_chain_fuel_spec(tree, parent, a, (budget.len() - 1) as nat)));
                    }
                }
            }
            reaches_root_with_budget(tree, parent, next_budget);
            assert(reaches_root_fuel_spec(tree, node, budget.len())
                == reaches_root_fuel_spec(tree, parent, (budget.len() - 1) as nat));
        }
    }
}

// r[impl cryptoshedding.chain-recoverability]
// r[impl cryptoshedding.rotation-correctness]
pub open spec fn is_leaf_spec(tree: Seq<Option<u64>>, node: u64) -> bool {
    &&& 0 <= (node as int) < tree.len()
    &&& forall|j: int| #![trigger tree[j]] 0 <= j < tree.len() ==> tree[j] != Some(node)
}

// r[impl cryptoshedding.subtree-shedding]
pub open spec fn in_subtree_spec(tree: Seq<Option<u64>>, node: u64, root: u64) -> bool {
    node == root || on_chain_spec(tree, node, root)
}

// r[impl cryptoshedding.tree-well-formedness]
pub open spec fn well_formed_spec(tree: Seq<Option<u64>>) -> bool {
    &&& forall|i: int| #![trigger tree[i]] 0 <= i < tree.len() ==> match tree[i] {
        Some(parent) => 0 <= (parent as int) < tree.len(),
        None => true,
    }
    &&& forall|i: int| #![trigger on_chain_fuel_spec(tree, i as u64, i as u64, tree.len())]
        0 <= i < tree.len() ==> !on_chain_fuel_spec(tree, i as u64, i as u64, tree.len())
}

// r[impl cryptoshedding.chain-recoverability]
pub open spec fn chain_present_fuel_spec(
    tree: Seq<Option<u64>>, keystore: Set<u64>, node: u64, fuel: nat,
) -> bool
    decreases fuel,
{
    if fuel == 0 || !((node as int) < tree.len()) {
        false
    } else if !keystore.contains(node) {
        false
    } else {
        match tree[node as int] {
            Some(parent) => chain_present_fuel_spec(tree, keystore, parent, (fuel - 1) as nat),
            None => true,
        }
    }
}

// r[impl cryptoshedding.chain-recoverability]
pub open spec fn recoverable_spec(tree: Seq<Option<u64>>, keystore: Set<u64>, leaf: u64) -> bool {
    &&& 0 <= (leaf as int) < tree.len()
    &&& is_leaf_spec(tree, leaf)
    &&& chain_present_fuel_spec(tree, keystore, leaf, tree.len())
}

// r[impl cryptoshedding.shed-definition]
pub open spec fn shed_spec(keystore: Set<u64>, key: u64) -> Set<u64> {
    keystore.remove(key)
}

proof fn shed_non_chain_preserves_chain_present(
    tree: Seq<Option<u64>>, keystore: Set<u64>, node: u64, key: u64, fuel: nat,
)
    requires
        well_formed_spec(tree),
        0 <= (node as int) < tree.len(),
        node != key,
        !on_chain_fuel_spec(tree, node, key, fuel),
    ensures
        chain_present_fuel_spec(tree, shed_spec(keystore, key), node, fuel)
            == chain_present_fuel_spec(tree, keystore, node, fuel)
    decreases fuel,
{
    if fuel == 0 {
    } else if !keystore.contains(node) {
        assert(!shed_spec(keystore, key).contains(node));
    } else {
        assert(shed_spec(keystore, key).contains(node));
        match tree[node as int] {
            Some(parent) => {
                assert(0 <= (parent as int) < tree.len());
                if parent == key {
                    assert(on_chain_fuel_spec(tree, node, key, fuel));
                    assert(false);
                } else {
                    assert(!on_chain_fuel_spec(tree, parent, key, (fuel - 1) as nat));
                    shed_non_chain_preserves_chain_present(tree, keystore, parent, key, (fuel - 1) as nat);
                }
            }
            None => {}
        }
    }
}

proof fn shed_on_chain_breaks_chain_present(
    tree: Seq<Option<u64>>, keystore: Set<u64>, node: u64, key: u64, fuel: nat,
)
    requires
        well_formed_spec(tree),
        0 <= (node as int) < tree.len(),
        key == node || on_chain_fuel_spec(tree, node, key, fuel),
    ensures
        !chain_present_fuel_spec(tree, shed_spec(keystore, key), node, fuel)
    decreases fuel,
{
    if fuel == 0 {
        assert(!chain_present_fuel_spec(tree, shed_spec(keystore, key), node, fuel));
    } else if key == node {
        assert(!shed_spec(keystore, key).contains(node));
        assert(!chain_present_fuel_spec(tree, shed_spec(keystore, key), node, fuel));
    } else {
        match tree[node as int] {
            Some(parent) => {
                assert(0 <= (parent as int) < tree.len());
                if parent == key {
                    assert(!shed_spec(keystore, key).contains(parent));
                    assert(!chain_present_fuel_spec(tree, shed_spec(keystore, key), parent, (fuel - 1) as nat));
                    assert(!chain_present_fuel_spec(tree, shed_spec(keystore, key), node, fuel));
                } else {
                    assert(on_chain_fuel_spec(tree, parent, key, (fuel - 1) as nat));
                    shed_on_chain_breaks_chain_present(tree, keystore, parent, key, (fuel - 1) as nat);
                    assert(!chain_present_fuel_spec(tree, shed_spec(keystore, key), parent, (fuel - 1) as nat));
                    assert(!chain_present_fuel_spec(tree, shed_spec(keystore, key), node, fuel));
                }
            }
            None => {
                assert(on_chain_fuel_spec(tree, node, key, fuel));
                assert(false);
            }
        }
    }
}

proof fn leaf_has_no_child_chain(
    tree: Seq<Option<u64>>, leaf: u64, node: u64, fuel: nat,
)
    requires
        well_formed_spec(tree),
        is_leaf_spec(tree, leaf),
        0 <= (node as int) < tree.len(),
        node != leaf,
    ensures
        !on_chain_fuel_spec(tree, node, leaf, fuel)
    decreases fuel,
{
    if fuel == 0 {
    } else {
        match tree[node as int] {
            Some(parent) => {
                assert(0 <= (parent as int) < tree.len());
                if parent == leaf {
                    assert(tree[node as int] == Some(leaf));
                    assert(false);
                } else {
                    leaf_has_no_child_chain(tree, leaf, parent, (fuel - 1) as nat);
                }
            }
            None => {}
        }
    }
}

// r[impl cryptoshedding.subtree-shedding]
pub open spec fn subtree_members_spec(tree: Seq<Option<u64>>, root: u64) -> Set<u64> {
    Set::new(|node: u64| in_subtree_spec(tree, node, root))
}

// r[impl cryptoshedding.rotation-correctness]
pub open spec fn rotate_spec(tree: Seq<Option<u64>>, leaf: u64, new_parent: u64) -> Seq<Option<u64>>
    recommends 0 <= (leaf as int) < tree.len(),
{
    tree.update(leaf as int, Some(new_parent))
}

// r[impl cryptoshedding.rotation-correctness]
pub open spec fn valid_rotation_spec(tree: Seq<Option<u64>>, leaf: u64, new_parent: u64) -> bool {
    &&& tree.len() <= u64::MAX as nat
    &&& well_formed_spec(tree)
    &&& 0 <= (leaf as int) < tree.len()
    &&& 0 <= (new_parent as int) < tree.len()
    &&& is_leaf_spec(tree, leaf)
    &&& leaf != new_parent
    &&& !on_chain_spec(tree, new_parent, leaf)
    &&& match tree[leaf as int] {
        Some(old_parent) => new_parent != old_parent && !on_chain_spec(tree, new_parent, old_parent),
        None => true,
    }
}

fn is_leaf_exec(parents: &[Option<u64>], node: usize) -> (result: bool)
    requires node < parents@.len()
    ensures result == is_leaf_spec(parents@, node as u64)
{
    let mut i: usize = 0;
    while i < parents.len()
        invariant
            0 <= i <= parents@.len(),
            forall|j: int| #![trigger parents@[j]] 0 <= j < i as int ==> parents@[j] != Some(node as u64),
        decreases parents.len() - i,
    {
        if parents[i] == Some(node as u64) {
            return false;
        }
        i = i + 1;
    }
    true
}

fn on_chain_from_exec(parents: &[Option<u64>], node: usize, ancestor: usize, fuel: usize) -> (result: bool)
    requires
        node < parents@.len(),
        ancestor < parents@.len(),
    ensures
        result == on_chain_fuel_spec(parents@, node as u64, ancestor as u64, fuel as nat)
    decreases fuel,
{
    if fuel == 0 {
        proof {
            assert(on_chain_fuel_spec(parents@, node as u64, ancestor as u64, fuel as nat) == false);
        }
        false
    } else {
        match parents[node] {
            Some(parent) => {
                if parent == ancestor as u64 {
                    proof {
                        assert(on_chain_fuel_spec(parents@, node as u64, ancestor as u64, fuel as nat));
                    }
                    true
                } else if parent < parents.len() as u64 {
                    proof {
                        assert(0 <= (parent as int) < parents@.len());
                        assert(on_chain_fuel_spec(parents@, node as u64, ancestor as u64, fuel as nat)
                            == on_chain_fuel_spec(parents@, parent, ancestor as u64, (fuel - 1) as nat));
                    }
                    on_chain_from_exec(parents, parent as usize, ancestor, fuel - 1)
                } else {
                    proof {
                        assert(!(0 <= (parent as int) < parents@.len()));
                        assert(on_chain_fuel_spec(parents@, parent, ancestor as u64, (fuel - 1) as nat) == false);
                        assert(on_chain_fuel_spec(parents@, node as u64, ancestor as u64, fuel as nat) == false);
                    }
                    false
                }
            }
            None => {
                proof {
                    assert(on_chain_fuel_spec(parents@, node as u64, ancestor as u64, fuel as nat) == false);
                }
                false
            }
        }
    }
}

fn recoverable_from_exec(parents: &[Option<u64>], present: &[bool], node: usize, fuel: usize) -> (result: bool)
    requires
        parents@.len() == present@.len(),
        node < parents@.len(),
    ensures
        result == chain_present_fuel_spec(parents@, keystore_model_from_bits(present@), node as u64, fuel as nat)
    decreases fuel,
{
    let ghost keystore = keystore_model_from_bits(present@);
    if fuel == 0 {
        proof {
            assert(chain_present_fuel_spec(parents@, keystore, node as u64, fuel as nat) == false);
        }
        false
    } else if !present[node] {
        proof {
            assert(!keystore.contains(node as u64));
            assert(chain_present_fuel_spec(parents@, keystore, node as u64, fuel as nat) == false);
        }
        false
    } else {
        proof {
            assert(keystore.contains(node as u64));
        }
        match parents[node] {
            Some(parent) => {
                if parent < parents.len() as u64 {
                    proof {
                        assert(0 <= (parent as int) < parents@.len());
                        assert(chain_present_fuel_spec(parents@, keystore, node as u64, fuel as nat)
                            == chain_present_fuel_spec(parents@, keystore, parent, (fuel - 1) as nat));
                    }
                    recoverable_from_exec(parents, present, parent as usize, fuel - 1)
                } else {
                    proof {
                        assert(!(0 <= (parent as int) < parents@.len()));
                        assert(chain_present_fuel_spec(parents@, keystore, parent, (fuel - 1) as nat) == false);
                        assert(chain_present_fuel_spec(parents@, keystore, node as u64, fuel as nat) == false);
                    }
                    false
                }
            }
            None => {
                proof {
                    assert(chain_present_fuel_spec(parents@, keystore, node as u64, fuel as nat));
                }
                true
            }
        }
    }
}

fn clone_bool_vec(bits: &[bool]) -> (result: Vec<bool>)
    ensures result@ == bits@
{
    let mut out: Vec<bool> = Vec::new();
    let mut i: usize = 0;
    while i < bits.len()
        invariant
            0 <= i <= bits@.len(),
            out@ == bits@.subrange(0, i as int),
        decreases bits.len() - i,
    {
        out.push(bits[i]);
        i = i + 1;
    }
    out
}

fn clone_parent_vec(bits: &[Option<u64>]) -> (result: Vec<Option<u64>>)
    ensures result@ == bits@
{
    let mut out: Vec<Option<u64>> = Vec::new();
    let mut i: usize = 0;
    while i < bits.len()
        invariant
            0 <= i <= bits@.len(),
            out@ == bits@.subrange(0, i as int),
        decreases bits.len() - i,
    {
        out.push(bits[i]);
        i = i + 1;
    }
    out
}

// r[impl cryptoshedding.tree-well-formedness]
pub fn well_formed(tree: &KeyTree) -> (result: bool)
    ensures result == well_formed_spec(tree.view())
{
    let mut i: usize = 0;
    while i < tree.parents.len()
        invariant
            0 <= i <= tree.parents@.len(),
            forall|k: int| #![trigger tree.parents@[k]] 0 <= k < i as int ==> match tree.parents@[k] {
                Some(parent) => 0 <= (parent as int) < tree.parents@.len(),
                None => true,
            },
            forall|k: int| #![trigger on_chain_fuel_spec(tree.view(), k as u64, k as u64, tree.parents@.len())]
                0 <= k < i as int ==> !on_chain_fuel_spec(tree.view(), k as u64, k as u64, tree.parents@.len()),
        decreases tree.parents.len() - i,
    {
        if let Some(parent) = tree.parents[i] {
            if parent >= tree.parents.len() as u64 {
                proof {
                    assert(tree.parents@[i as int] == Some(parent));
                    assert(!(0 <= (parent as int) < tree.parents@.len()));
                }
                return false;
            }
        }
        if on_chain_from_exec(&tree.parents, i, i, tree.parents.len()) {
            return false;
        }
        i = i + 1;
    }
    true
}

// r[impl cryptoshedding.chain-recoverability]
// r[impl cryptoshedding.rotation-correctness]
pub fn is_leaf(tree: &KeyTree, node: usize) -> (result: bool)
    requires node < tree.parents@.len()
    ensures result == is_leaf_spec(tree.view(), node as u64)
{
    is_leaf_exec(&tree.parents, node)
}

// r[impl cryptoshedding.shed-completeness]
// r[impl cryptoshedding.shed-isolation]
// r[impl cryptoshedding.rotation-correctness]
pub fn on_chain(tree: &KeyTree, node: usize, ancestor: usize) -> (result: bool)
    requires
        node < tree.parents@.len(),
        ancestor < tree.parents@.len(),
    ensures result == on_chain_spec(tree.view(), node as u64, ancestor as u64)
{
    on_chain_from_exec(&tree.parents, node, ancestor, tree.parents.len())
}

// r[impl cryptoshedding.chain-recoverability]
pub fn is_recoverable(tree: &KeyTree, keystore: &Keystore, leaf: usize) -> (result: bool)
    requires
        tree.parents@.len() == keystore.present@.len(),
        leaf < tree.parents@.len(),
        well_formed_spec(tree.view()),
        is_leaf_spec(tree.view(), leaf as u64),
    ensures result == recoverable_spec(tree.view(), keystore.view(), leaf as u64)
{
    if !is_leaf_exec(&tree.parents, leaf) {
        false
    } else {
        recoverable_from_exec(&tree.parents, &keystore.present, leaf, tree.parents.len())
    }
}

// r[impl cryptoshedding.shed-definition]
// r[impl cryptoshedding.idempotent-destruction]
// Tree preservation is structural here: `shed` takes only `Keystore` and returns only `Keystore`.
pub fn shed(keystore: &Keystore, key: usize) -> (result: Keystore)
    ensures
        result.present@.len() == keystore.present@.len(),
        result.view() == shed_spec(keystore.view(), key as u64),
{
    let mut out = clone_bool_vec(&keystore.present);
    if key < out.len() {
        out.set(key, false);
    }
    let result = Keystore { present: out };
    proof {
        assert forall|k: u64| result.view().contains(k) == shed_spec(keystore.view(), key as u64).contains(k)
        by {
        }
    }
    result
}

// r[impl cryptoshedding.rotation-correctness]
pub fn rotate(tree: &KeyTree, leaf: usize, new_parent: usize) -> (result: KeyTree)
    requires
        valid_rotation_spec(tree.view(), leaf as u64, new_parent as u64),
    ensures
        result.view() == rotate_spec(tree.view(), leaf as u64, new_parent as u64),
{
    let mut out = clone_parent_vec(&tree.parents);
    out.set(leaf, Some(new_parent as u64));
    KeyTree { parents: out }
}

// r[impl cryptoshedding.idempotent-destruction]
// r[verify cryptoshedding.idempotent-destruction]
pub proof fn shed_idempotent(keystore: Set<u64>, key: u64)
    ensures shed_spec(shed_spec(keystore, key), key) == shed_spec(keystore, key)
{
}

// r[impl cryptoshedding.shed-definition]
// r[verify cryptoshedding.shed-definition]
pub proof fn shed_absent_key_is_noop(keystore: Set<u64>, key: u64)
    requires
        !keystore.contains(key),
    ensures
        shed_spec(keystore, key) == keystore,
{
}

// r[impl cryptoshedding.shed-completeness]
// r[verify cryptoshedding.shed-completeness]
pub proof fn shedding_on_chain_breaks_recoverability(
    tree: Seq<Option<u64>>, keystore: Set<u64>, leaf: u64, key: u64,
)
    requires
        well_formed_spec(tree),
        recoverable_spec(tree, keystore, leaf),
        key == leaf || on_chain_spec(tree, leaf, key),
    ensures !recoverable_spec(tree, shed_spec(keystore, key), leaf)
{
    shed_on_chain_breaks_chain_present(tree, keystore, leaf, key, tree.len());
}

// r[impl cryptoshedding.shed-completeness]
// r[verify cryptoshedding.shed-completeness]
pub proof fn shedding_leaf_breaks_recoverability(tree: Seq<Option<u64>>, keystore: Set<u64>, leaf: u64)
    requires
        well_formed_spec(tree),
        recoverable_spec(tree, keystore, leaf),
    ensures !recoverable_spec(tree, shed_spec(keystore, leaf), leaf)
{
    shedding_on_chain_breaks_recoverability(tree, keystore, leaf, leaf);
}

// r[impl cryptoshedding.shed-isolation]
// r[verify cryptoshedding.shed-isolation]
pub proof fn shedding_nonancestor_preserves_recoverability(
    tree: Seq<Option<u64>>, keystore: Set<u64>, leaf: u64, key: u64,
)
    requires
        well_formed_spec(tree),
        0 <= (leaf as int) < tree.len(),
        is_leaf_spec(tree, leaf),
        key != leaf,
        !on_chain_spec(tree, leaf, key),
    ensures
        recoverable_spec(tree, shed_spec(keystore, key), leaf)
            == recoverable_spec(tree, keystore, leaf)
{
    shed_non_chain_preserves_chain_present(tree, keystore, leaf, key, tree.len());
}

proof fn on_chain_true_monotone(tree: Seq<Option<u64>>, node: u64, ancestor: u64, smaller: nat, larger: nat)
    requires
        0 <= (node as int) < tree.len(),
        0 <= (ancestor as int) < tree.len(),
        smaller <= larger,
        on_chain_fuel_spec(tree, node, ancestor, smaller),
    ensures
        on_chain_fuel_spec(tree, node, ancestor, larger),
    decreases smaller,
{
    if smaller == 0 {
        assert(false);
    } else {
        assert(larger > 0) by {
            assert(smaller > 0);
            assert(smaller <= larger);
        }
        match tree[node as int] {
            Some(parent) => {
                assert(on_chain_fuel_spec(tree, node, ancestor, smaller)
                    == (parent == ancestor || on_chain_fuel_spec(tree, parent, ancestor, (smaller - 1) as nat)));
                assert(on_chain_fuel_spec(tree, node, ancestor, larger)
                    == (parent == ancestor || on_chain_fuel_spec(tree, parent, ancestor, (larger - 1) as nat)));
                if parent == ancestor {
                } else {
                    assert(on_chain_fuel_spec(tree, parent, ancestor, (smaller - 1) as nat));
                    assert(0 <= (parent as int) < tree.len());
                    assert((smaller - 1) as nat <= (larger - 1) as nat);
                    on_chain_true_monotone(tree, parent, ancestor, (smaller - 1) as nat, (larger - 1) as nat);
                }
            }
            None => {
                assert(false);
            }
        }
    }
}

// r[impl cryptoshedding.tree-well-formedness]
// r[verify cryptoshedding.tree-well-formedness]
pub proof fn on_chain_matches_parent_walk_reaches(tree: Seq<Option<u64>>, node: u64, ancestor: u64)
    requires
        well_formed_spec(tree),
        0 <= (node as int) < tree.len(),
        0 <= (ancestor as int) < tree.len(),
    ensures
        on_chain_spec(tree, node, ancestor) == parent_walk_reaches_spec(tree, node, ancestor),
{
    if on_chain_spec(tree, node, ancestor) {
        assert(parent_walk_reaches_spec(tree, node, ancestor));
    }
    if parent_walk_reaches_spec(tree, node, ancestor) {
        let fuel = choose|fuel: nat| fuel <= tree.len() && on_chain_fuel_spec(tree, node, ancestor, fuel);
        on_chain_true_monotone(tree, node, ancestor, fuel, tree.len());
    }
}

// r[impl cryptoshedding.tree-well-formedness]
// r[verify cryptoshedding.tree-well-formedness]
pub proof fn parent_walk_reaches_root_within_len(tree: Seq<Option<u64>>, node: u64)
    requires
        tree.len() <= u64::MAX as nat,
        well_formed_spec(tree),
        0 <= (node as int) < tree.len(),
    ensures
        reaches_root_fuel_spec(tree, node, tree.len()),
{
    nodes_upto_finite_len(tree.len());
    let budget = nodes_upto_spec(tree.len());
    assert(budget.contains(node));
    assert forall|v: u64| budget.contains(v) implies 0 <= (v as int) < tree.len() by {
    }
    assert forall|a: u64| 0 <= (a as int) < tree.len() && !budget.contains(a)
        implies !on_chain_fuel_spec(tree, node, a, budget.len())
    by {
    }
    reaches_root_with_budget(tree, node, budget);
}

// r[impl cryptoshedding.tree-well-formedness]
// r[verify cryptoshedding.tree-well-formedness]
pub proof fn parent_slot_is_unique(tree: Seq<Option<u64>>, node: u64, parent1: u64, parent2: u64)
    requires
        0 <= (node as int) < tree.len(),
        tree[node as int] == Some(parent1),
        tree[node as int] == Some(parent2),
    ensures
        parent1 == parent2,
{
}

// r[impl cryptoshedding.tree-well-formedness]
// r[verify cryptoshedding.tree-well-formedness]
pub proof fn well_formed_rejects_out_of_bounds_parent(tree: Seq<Option<u64>>, node: u64, parent: u64)
    requires
        0 <= (node as int) < tree.len(),
        tree[node as int] == Some(parent),
        !(0 <= (parent as int) < tree.len()),
    ensures
        !well_formed_spec(tree),
{
    assert(!well_formed_spec(tree)) by {
        if well_formed_spec(tree) {
            assert(match tree[node as int] {
                Some(p) => 0 <= (p as int) < tree.len(),
                None => true,
            });
            assert(false);
        }
    }
}

// r[impl cryptoshedding.tree-well-formedness]
// r[verify cryptoshedding.tree-well-formedness]
pub proof fn well_formed_rejects_cycle(tree: Seq<Option<u64>>, node: u64)
    requires
        0 <= (node as int) < tree.len(),
        on_chain_fuel_spec(tree, node, node, tree.len()),
    ensures
        !well_formed_spec(tree),
{
    assert(!well_formed_spec(tree)) by {
        if well_formed_spec(tree) {
            let i = node as int;
            assert(0 <= i < tree.len());
            assert(i as u64 == node);
            assert(!on_chain_fuel_spec(tree, i as u64, i as u64, tree.len())) by {
                assert(well_formed_spec(tree));
            }
            assert(false);
        }
    }
}

// r[impl cryptoshedding.tree-well-formedness]
// r[verify cryptoshedding.tree-well-formedness]
pub proof fn bounded_acyclic_tree_is_well_formed(tree: Seq<Option<u64>>)
    requires
        forall|i: int| #![trigger tree[i]] 0 <= i < tree.len() ==> match tree[i] {
            Some(parent) => 0 <= (parent as int) < tree.len(),
            None => true,
        },
        forall|i: int| #![trigger on_chain_fuel_spec(tree, i as u64, i as u64, tree.len())]
            0 <= i < tree.len() ==> !on_chain_fuel_spec(tree, i as u64, i as u64, tree.len()),
    ensures
        well_formed_spec(tree),
{
}

// r[impl cryptoshedding.tree-well-formedness]
// r[verify cryptoshedding.tree-well-formedness]
pub proof fn on_chain_strict_on_well_formed(tree: Seq<Option<u64>>, node: u64)
    requires
        well_formed_spec(tree),
        0 <= (node as int) < tree.len(),
    ensures
        !on_chain_spec(tree, node, node),
{
    let i = node as int;
    assert(0 <= i < tree.len());
    assert(i as u64 == node);
    assert(!on_chain_fuel_spec(tree, i as u64, i as u64, tree.len())) by {
        assert(well_formed_spec(tree));
    }
}

// r[impl cryptoshedding.chain-recoverability]
// r[verify cryptoshedding.chain-recoverability]
pub proof fn is_leaf_spec_matches_no_children(tree: Seq<Option<u64>>, node: u64)
    ensures
        is_leaf_spec(tree, node)
            == (0 <= (node as int) < tree.len()
                && forall|j: int| #![trigger tree[j]] 0 <= j < tree.len() ==> tree[j] != Some(node)),
{
}

// r[impl cryptoshedding.chain-recoverability]
// r[verify cryptoshedding.chain-recoverability]
pub proof fn recoverable_spec_matches_chain_present(tree: Seq<Option<u64>>, keystore: Set<u64>, leaf: u64)
    ensures
        recoverable_spec(tree, keystore, leaf)
            == (0 <= (leaf as int) < tree.len()
                && is_leaf_spec(tree, leaf)
                && chain_present_fuel_spec(tree, keystore, leaf, tree.len())),
{
}

// r[impl cryptoshedding.subtree-shedding]
// r[verify cryptoshedding.subtree-shedding]
pub proof fn subtree_members_matches_in_subtree(tree: Seq<Option<u64>>, root: u64, node: u64)
    ensures
        subtree_members_spec(tree, root).contains(node)
            == in_subtree_spec(tree, node, root)
{
}

// r[impl cryptoshedding.subtree-shedding]
// r[verify cryptoshedding.subtree-shedding]
pub proof fn shedding_subtree_root_breaks_leaf_recoverability(
    tree: Seq<Option<u64>>, keystore: Set<u64>, leaf: u64, root: u64,
)
    requires
        well_formed_spec(tree),
        recoverable_spec(tree, keystore, leaf),
        0 <= (root as int) < tree.len(),
        !is_leaf_spec(tree, root),
        in_subtree_spec(tree, leaf, root),
    ensures
        !recoverable_spec(tree, shed_spec(keystore, root), leaf)
{
    if leaf == root {
        assert(is_leaf_spec(tree, root));
        assert(false);
    }
    shedding_on_chain_breaks_recoverability(tree, keystore, leaf, root);
}

// r[impl cryptoshedding.subtree-shedding]
// r[verify cryptoshedding.subtree-shedding]
pub proof fn shedding_outside_subtree_preserves_leaf_recoverability(
    tree: Seq<Option<u64>>, keystore: Set<u64>, leaf: u64, root: u64,
)
    requires
        well_formed_spec(tree),
        0 <= (leaf as int) < tree.len(),
        is_leaf_spec(tree, leaf),
        0 <= (root as int) < tree.len(),
        !is_leaf_spec(tree, root),
        !in_subtree_spec(tree, leaf, root),
    ensures
        recoverable_spec(tree, shed_spec(keystore, root), leaf)
            == recoverable_spec(tree, keystore, leaf)
{
    assert(leaf != root);
    shedding_nonancestor_preserves_recoverability(tree, keystore, leaf, root);
}

// r[impl cryptoshedding.idempotent-destruction]
// r[verify cryptoshedding.idempotent-destruction]
pub proof fn double_shed_preserves_recoverability(
    tree: Seq<Option<u64>>, keystore: Set<u64>, leaf: u64, key: u64,
)
    ensures
        recoverable_spec(tree, shed_spec(shed_spec(keystore, key), key), leaf)
            == recoverable_spec(tree, shed_spec(keystore, key), leaf)
{
    shed_idempotent(keystore, key);
}

// r[impl cryptoshedding.rotation-correctness]
// r[verify cryptoshedding.rotation-correctness]
pub proof fn rotate_preserves_leafness(tree: Seq<Option<u64>>, leaf: u64, new_parent: u64)
    requires
        well_formed_spec(tree),
        is_leaf_spec(tree, leaf),
        0 <= (new_parent as int) < tree.len(),
        leaf != new_parent,
    ensures is_leaf_spec(rotate_spec(tree, leaf, new_parent), leaf)
{
    let updated = tree.update(leaf as int, Some(new_parent));
    let rotated = rotate_spec(tree, leaf, new_parent);
    assert(0 <= (leaf as int) < tree.len());
    assert(rotated == updated);
    assert(rotated.len() == tree.len());
    assert forall|j: int| #![trigger rotated[j]]
        0 <= j < rotated.len()
            ==> rotated[j] != Some(leaf)
    by {
        if 0 <= j < rotated.len() {
            assert(j < tree.len()) by {
                assert(rotated.len() == tree.len());
            }
            if j == leaf as int {
                assert(rotated[j] == Some(new_parent));
                assert(Some(new_parent) != Some(leaf));
            } else {
                assert(j != leaf as int);
                assert(updated[j] == tree[j]);
                assert(rotated[j] == updated[j]);
                assert(tree[j] != Some(leaf));
            }
        }
    }
}

// r[impl cryptoshedding.rotation-correctness]
// r[verify cryptoshedding.rotation-correctness]
pub proof fn rotate_updates_only_target(tree: Seq<Option<u64>>, leaf: u64, new_parent: u64, idx: int)
    requires
        0 <= (leaf as int) < tree.len(),
        0 <= idx < tree.len(),
        idx != leaf as int,
    ensures
        rotate_spec(tree, leaf, new_parent)[idx] == tree[idx]
{
}

proof fn rotate_preserves_on_chain_from_other_node(
    tree: Seq<Option<u64>>, leaf: u64, new_parent: u64, node: int, ancestor: int, fuel: nat,
)
    requires
        tree.len() <= u64::MAX as nat,
        well_formed_spec(tree),
        is_leaf_spec(tree, leaf),
        0 <= node < tree.len(),
        0 <= ancestor < tree.len(),
        node != leaf as int,
    ensures
        on_chain_fuel_spec(rotate_spec(tree, leaf, new_parent), node as u64, ancestor as u64, fuel)
            == on_chain_fuel_spec(tree, node as u64, ancestor as u64, fuel)
    decreases fuel,
{
    let rotated = rotate_spec(tree, leaf, new_parent);
    let node_u = node as u64;
    let ancestor_u = ancestor as u64;
    assert(node_u as int == node);
    assert(ancestor_u as int == ancestor);
    if fuel == 0 {
        assert(on_chain_fuel_spec(rotated, node_u, ancestor_u, fuel) == false);
        assert(on_chain_fuel_spec(tree, node_u, ancestor_u, fuel) == false);
    } else {
        match tree[node] {
            Some(parent) => {
                assert(0 <= (parent as int) < tree.len());
                if parent == leaf {
                    assert(tree[node] == Some(leaf));
                    assert(false);
                } else {
                    rotate_updates_only_target(tree, leaf, new_parent, node);
                    assert(rotated[node] == tree[node]);
                    rotate_preserves_on_chain_from_other_node(
                        tree,
                        leaf,
                        new_parent,
                        parent as int,
                        ancestor,
                        (fuel - 1) as nat,
                    );
                    assert(on_chain_fuel_spec(rotated, node_u, ancestor_u, fuel)
                        == (parent == ancestor_u
                            || on_chain_fuel_spec(rotated, parent, ancestor_u, (fuel - 1) as nat)));
                    assert(on_chain_fuel_spec(tree, node_u, ancestor_u, fuel)
                        == (parent == ancestor_u
                            || on_chain_fuel_spec(tree, parent, ancestor_u, (fuel - 1) as nat)));
                }
            }
            None => {
                rotate_updates_only_target(tree, leaf, new_parent, node);
                assert(rotated[node] == tree[node]);
                assert(on_chain_fuel_spec(rotated, node_u, ancestor_u, fuel) == false);
                assert(on_chain_fuel_spec(tree, node_u, ancestor_u, fuel) == false);
            }
        }
    }
}

proof fn on_chain_false_monotone(
    tree: Seq<Option<u64>>, node: u64, ancestor: u64, smaller: nat, larger: nat,
)
    requires
        well_formed_spec(tree),
        0 <= (node as int) < tree.len(),
        0 <= (ancestor as int) < tree.len(),
        smaller <= larger,
        !on_chain_fuel_spec(tree, node, ancestor, larger),
    ensures
        !on_chain_fuel_spec(tree, node, ancestor, smaller)
    decreases smaller,
{
    if smaller == 0 {
        assert(!on_chain_fuel_spec(tree, node, ancestor, smaller));
    } else {
        assert(larger > 0) by {
            assert(smaller > 0);
            assert(smaller <= larger);
        }
        match tree[node as int] {
            Some(parent) => {
                assert(0 <= (parent as int) < tree.len());
                assert(on_chain_fuel_spec(tree, node, ancestor, larger)
                    == (parent == ancestor
                        || on_chain_fuel_spec(tree, parent, ancestor, (larger - 1) as nat)));
                if parent == ancestor {
                    assert(on_chain_fuel_spec(tree, node, ancestor, larger));
                    assert(false);
                } else {
                    assert(!on_chain_fuel_spec(tree, parent, ancestor, (larger - 1) as nat));
                    assert((smaller - 1) as nat <= (larger - 1) as nat);
                    on_chain_false_monotone(
                        tree,
                        parent,
                        ancestor,
                        (smaller - 1) as nat,
                        (larger - 1) as nat,
                    );
                    assert(on_chain_fuel_spec(tree, node, ancestor, smaller)
                        == (parent == ancestor
                            || on_chain_fuel_spec(tree, parent, ancestor, (smaller - 1) as nat)));
                }
            }
            None => {
                assert(!on_chain_fuel_spec(tree, node, ancestor, smaller));
            }
        }
    }
}

// r[impl cryptoshedding.rotation-correctness]
// r[verify cryptoshedding.rotation-correctness]
pub proof fn rotate_preserves_well_formedness(tree: Seq<Option<u64>>, leaf: u64, new_parent: u64)
    requires
        valid_rotation_spec(tree, leaf, new_parent),
    ensures
        well_formed_spec(rotate_spec(tree, leaf, new_parent))
{
    let rotated = rotate_spec(tree, leaf, new_parent);
    assert(rotated.len() == tree.len());
    assert forall|i: int| #![trigger rotated[i]] 0 <= i < rotated.len() ==> match rotated[i] {
        Some(parent) => 0 <= (parent as int) < rotated.len(),
        None => true,
    } by {
        if 0 <= i < rotated.len() {
            assert(0 <= i < tree.len()) by {
                assert(rotated.len() == tree.len());
            }
            if i == leaf as int {
                assert(rotated[i] == Some(new_parent));
                assert(0 <= (new_parent as int) < rotated.len());
            } else {
                rotate_updates_only_target(tree, leaf, new_parent, i);
                assert(rotated[i] == tree[i]);
                assert(match tree[i] {
                    Some(parent) => 0 <= (parent as int) < tree.len(),
                    None => true,
                }) by {
                    assert(well_formed_spec(tree));
                }
            }
        }
    }
    assert forall|i: int| #![trigger on_chain_fuel_spec(rotated, i as u64, i as u64, rotated.len())]
        0 <= i < rotated.len() ==> !on_chain_fuel_spec(rotated, i as u64, i as u64, rotated.len())
    by {
        if 0 <= i < rotated.len() {
            if i == leaf as int {
                assert(rotated[i] == Some(new_parent));
                assert(new_parent != leaf);
                rotate_preserves_on_chain_from_other_node(
                    tree,
                    leaf,
                    new_parent,
                    new_parent as int,
                    leaf as int,
                    (rotated.len() - 1) as nat,
                );
                leaf_has_no_child_chain(tree, leaf, new_parent, (rotated.len() - 1) as nat);
                assert(!on_chain_fuel_spec(rotated, new_parent, leaf, (rotated.len() - 1) as nat));
                assert(on_chain_fuel_spec(rotated, i as u64, i as u64, rotated.len())
                    == (new_parent == i as u64
                        || on_chain_fuel_spec(rotated, new_parent, i as u64, (rotated.len() - 1) as nat)));
            } else {
                assert(i != leaf as int);
                rotate_preserves_on_chain_from_other_node(tree, leaf, new_parent, i, i, rotated.len());
                assert(!on_chain_fuel_spec(tree, i as u64, i as u64, tree.len())) by {
                    assert(well_formed_spec(tree));
                }
            }
        }
    }
}

// r[impl cryptoshedding.rotation-correctness]
// r[verify cryptoshedding.rotation-correctness]
pub proof fn rotate_recoverability_matches_new_chain(
    tree: Seq<Option<u64>>, keystore: Set<u64>, leaf: u64, new_parent: u64,
)
    requires
        valid_rotation_spec(tree, leaf, new_parent),
    ensures
        recoverable_spec(rotate_spec(tree, leaf, new_parent), keystore, leaf)
            == chain_present_fuel_spec(
                rotate_spec(tree, leaf, new_parent),
                keystore,
                leaf,
                rotate_spec(tree, leaf, new_parent).len(),
            )
{
    let rotated = rotate_spec(tree, leaf, new_parent);
    rotate_preserves_well_formedness(tree, leaf, new_parent);
    rotate_preserves_leafness(tree, leaf, new_parent);
    assert(rotated.len() == tree.len());
    assert(0 <= (leaf as int) < rotated.len());
    assert(is_leaf_spec(rotated, leaf));
}

// r[impl cryptoshedding.rotation-correctness]
// r[verify cryptoshedding.rotation-correctness]
pub proof fn rotate_makes_new_parent_immediate_ancestor(tree: Seq<Option<u64>>, leaf: u64, new_parent: u64)
    requires
        0 <= (leaf as int) < tree.len(),
        0 <= (new_parent as int) < tree.len(),
    ensures on_chain_spec(rotate_spec(tree, leaf, new_parent), leaf, new_parent)
{
}

// r[impl cryptoshedding.rotation-correctness]
// r[verify cryptoshedding.rotation-correctness]
pub proof fn valid_rotation_excludes_old_parent_from_new_chain(
    tree: Seq<Option<u64>>, leaf: u64, new_parent: u64, old_parent: u64,
)
    requires
        valid_rotation_spec(tree, leaf, new_parent),
        tree[leaf as int] == Some(old_parent),
    ensures
        !on_chain_spec(rotate_spec(tree, leaf, new_parent), leaf, old_parent)
{
    let rotated = rotate_spec(tree, leaf, new_parent);
    assert(rotated.len() == tree.len());
    assert(0 <= (old_parent as int) < tree.len());
    assert(!on_chain_fuel_spec(tree, new_parent, old_parent, tree.len())) by {
        assert(valid_rotation_spec(tree, leaf, new_parent));
    }
    on_chain_false_monotone(
        tree,
        new_parent,
        old_parent,
        (tree.len() - 1) as nat,
        tree.len(),
    );
    rotate_preserves_on_chain_from_other_node(
        tree,
        leaf,
        new_parent,
        new_parent as int,
        old_parent as int,
        (tree.len() - 1) as nat,
    );
    assert(rotated[leaf as int] == Some(new_parent));
    assert(new_parent != old_parent) by {
        assert(valid_rotation_spec(tree, leaf, new_parent));
    }
    assert(on_chain_fuel_spec(rotated, leaf, old_parent, rotated.len())
        == (new_parent == old_parent
            || on_chain_fuel_spec(rotated, new_parent, old_parent, (rotated.len() - 1) as nat)));
}

// r[impl cryptoshedding.rotation-correctness]
// r[verify cryptoshedding.rotation-correctness]
pub proof fn rotate_old_parent_shed_preserves_recoverability(
    tree: Seq<Option<u64>>, keystore: Set<u64>, leaf: u64, new_parent: u64, old_parent: u64,
)
    requires
        valid_rotation_spec(tree, leaf, new_parent),
        tree[leaf as int] == Some(old_parent),
        recoverable_spec(rotate_spec(tree, leaf, new_parent), keystore, leaf),
    ensures
        recoverable_spec(rotate_spec(tree, leaf, new_parent), shed_spec(keystore, old_parent), leaf)
            == recoverable_spec(rotate_spec(tree, leaf, new_parent), keystore, leaf)
{
    rotate_preserves_well_formedness(tree, leaf, new_parent);
    valid_rotation_excludes_old_parent_from_new_chain(tree, leaf, new_parent, old_parent);
    assert(old_parent != leaf) by {
        if old_parent == leaf {
            assert(on_chain_fuel_spec(tree, leaf, leaf, tree.len()));
            assert(false);
        }
    }
    shedding_nonancestor_preserves_recoverability(
        rotate_spec(tree, leaf, new_parent),
        keystore,
        leaf,
        old_parent,
    );
}

// r[impl cryptoshedding.rotation-correctness]
// r[verify cryptoshedding.rotation-correctness]
pub proof fn rotate_new_parent_shed_breaks_recoverability(
    tree: Seq<Option<u64>>, keystore: Set<u64>, leaf: u64, new_parent: u64,
)
    requires
        valid_rotation_spec(tree, leaf, new_parent),
        recoverable_spec(rotate_spec(tree, leaf, new_parent), keystore, leaf),
    ensures
        !recoverable_spec(rotate_spec(tree, leaf, new_parent), shed_spec(keystore, new_parent), leaf)
{
    rotate_preserves_well_formedness(tree, leaf, new_parent);
    rotate_makes_new_parent_immediate_ancestor(tree, leaf, new_parent);
    shedding_on_chain_breaks_recoverability(rotate_spec(tree, leaf, new_parent), keystore, leaf, new_parent);
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn canonical_tree(raw: &[usize]) -> KeyTree {
        let mut parents: Vec<Option<u64>> = Vec::with_capacity(raw.len());
        for (i, value) in raw.iter().enumerate() {
            if i == 0 {
                parents.push(None);
            } else {
                let slot = value % (i + 1);
                if slot == i {
                    parents.push(None);
                } else {
                    parents.push(Some(slot as u64));
                }
            }
        }
        let tree = KeyTree { parents };
        assert!(well_formed(&tree));
        tree
    }

    fn leaf_indices(tree: &KeyTree) -> Vec<usize> {
        (0..tree.parents.len()).filter(|&i| is_leaf(tree, i)).collect()
    }

    fn full_keystore(len: usize) -> Keystore {
        Keystore {
            present: vec![true; len],
        }
    }

    fn valid_rotation_targets(tree: &KeyTree, leaf: usize) -> Vec<usize> {
        let old_parent = tree.parents[leaf].map(|p| p as usize);
        (0..tree.parents.len())
            .filter(|&new_parent| {
                new_parent != leaf
                    && !on_chain(tree, new_parent, leaf)
                    && match old_parent {
                        Some(old_parent) => new_parent != old_parent && !on_chain(tree, new_parent, old_parent),
                        None => true,
                    }
            })
            .collect()
    }

    // r[verify cryptoshedding.tree-well-formedness]
    #[test]
    fn well_formed_accepts_valid_forest() {
        let tree = KeyTree {
            parents: vec![None, Some(0), None, Some(2), Some(3)],
        };
        assert!(well_formed(&tree));
    }

    // r[verify cryptoshedding.tree-well-formedness]
    #[test]
    fn well_formed_rejects_out_of_bounds_parent() {
        let tree = KeyTree {
            parents: vec![None, Some(7)],
        };
        assert!(!well_formed(&tree));
    }

    // r[verify cryptoshedding.tree-well-formedness]
    #[test]
    fn well_formed_rejects_cycle() {
        let tree = KeyTree {
            parents: vec![Some(1), Some(0)],
        };
        assert!(!well_formed(&tree));
    }

    #[test]
    fn on_chain_linear_chain() {
        let tree = KeyTree {
            parents: vec![None, Some(0), Some(1), Some(2)],
        };
        assert!(on_chain(&tree, 3, 2));
        assert!(on_chain(&tree, 3, 1));
        assert!(on_chain(&tree, 3, 0));
        assert!(!on_chain(&tree, 3, 3));
        assert!(!on_chain(&tree, 1, 2));
    }

    #[test]
    fn on_chain_branching_forest() {
        let tree = KeyTree {
            parents: vec![None, Some(0), Some(0), None, Some(3)],
        };
        assert!(on_chain(&tree, 2, 0));
        assert!(on_chain(&tree, 4, 3));
        assert!(!on_chain(&tree, 4, 0));
        assert!(!on_chain(&tree, 2, 3));
    }

    // r[verify cryptoshedding.chain-recoverability]
    #[test]
    fn recoverable_full_chain_present() {
        let tree = KeyTree {
            parents: vec![None, Some(0), Some(1)],
        };
        let keystore = Keystore {
            present: vec![true, true, true],
        };
        assert!(is_recoverable(&tree, &keystore, 2));
    }

    // r[verify cryptoshedding.chain-recoverability]
    #[test]
    fn recoverable_intermediate_missing() {
        let tree = KeyTree {
            parents: vec![None, Some(0), Some(1)],
        };
        let keystore = Keystore {
            present: vec![true, false, true],
        };
        assert!(!is_recoverable(&tree, &keystore, 2));
    }

    // r[verify cryptoshedding.chain-recoverability]
    #[test]
    fn recoverable_leaf_missing() {
        let tree = KeyTree {
            parents: vec![None, Some(0), Some(1)],
        };
        let keystore = Keystore {
            present: vec![true, true, false],
        };
        assert!(!is_recoverable(&tree, &keystore, 2));
    }

    // r[verify cryptoshedding.shed-definition]
    // r[verify cryptoshedding.idempotent-destruction]
    #[test]
    fn shed_clears_key() {
        let keystore = Keystore {
            present: vec![true, true, true],
        };
        let shed_once = shed(&keystore, 1);
        assert_eq!(shed_once.present, vec![true, false, true]);
        let shed_twice = shed(&shed_once, 1);
        assert_eq!(shed_twice.present, vec![true, false, true]);
    }

    // r[verify cryptoshedding.shed-isolation]
    #[test]
    fn shed_isolation_sibling_unaffected() {
        let tree = KeyTree {
            parents: vec![None, Some(0), Some(1), Some(0)],
        };
        let keystore = Keystore {
            present: vec![true, true, true, true],
        };
        let shed_i = shed(&keystore, 1);
        assert!(!is_recoverable(&tree, &shed_i, 2));
        assert!(is_recoverable(&tree, &shed_i, 3));
    }

    // r[verify cryptoshedding.shed-definition]
    // r[verify cryptoshedding.idempotent-destruction]
    #[test]
    fn shed_multiple_keys_and_repeat_is_stable() {
        let keystore = Keystore {
            present: vec![true, true, true, true],
        };
        let once = shed(&keystore, 1);
        let twice = shed(&once, 3);
        let thrice = shed(&twice, 1);
        assert_eq!(once.present, vec![true, false, true, true]);
        assert_eq!(twice.present, vec![true, false, true, false]);
        assert_eq!(thrice.present, vec![true, false, true, false]);
    }

    // r[verify cryptoshedding.subtree-shedding]
    #[test]
    fn subtree_shedding_internal_key_breaks_descendants() {
        let tree = KeyTree {
            parents: vec![None, Some(0), Some(1), Some(1), Some(0)],
        };
        let keystore = Keystore {
            present: vec![true, true, true, true, true],
        };
        let shed_i = shed(&keystore, 1);
        assert!(!is_recoverable(&tree, &shed_i, 2));
        assert!(!is_recoverable(&tree, &shed_i, 3));
        assert!(is_recoverable(&tree, &shed_i, 4));
    }

    // r[verify cryptoshedding.subtree-shedding]
    #[test]
    fn subtree_shedding_deep_chain_breaks_tail_leaf() {
        let tree = KeyTree {
            parents: vec![None, Some(0), Some(1), Some(2), Some(3)],
        };
        let keystore = Keystore {
            present: vec![true, true, true, true, true],
        };
        let shed_mid = shed(&keystore, 2);
        assert!(!is_recoverable(&tree, &shed_mid, 4));
    }

    #[test]
    fn rotate_reparents_leaf() {
        let tree = KeyTree {
            parents: vec![None, None, Some(0)],
        };
        let rotated = rotate(&tree, 2, 1);
        assert_eq!(rotated.parents, vec![None, None, Some(1)]);
    }

    // r[verify cryptoshedding.rotation-correctness]
    #[test]
    fn rotate_cross_tree_reparents_leaf() {
        let tree = KeyTree {
            parents: vec![None, Some(0), None, Some(2), Some(1)],
        };
        let rotated = rotate(&tree, 4, 3);
        assert_eq!(rotated.parents, vec![None, Some(0), None, Some(2), Some(3)]);
        assert!(is_recoverable(
            &rotated,
            &Keystore {
                present: vec![true, true, true, true, true]
            },
            4
        ));
    }

    // r[verify cryptoshedding.rotation-correctness]
    #[test]
    fn rotate_old_parent_shed_preserves_recoverability() {
        let tree = KeyTree {
            parents: vec![None, Some(0), Some(0), Some(1)],
        };
        let rotated = rotate(&tree, 3, 2);
        let keystore = Keystore {
            present: vec![true, true, true, true],
        };
        let shed_old = shed(&keystore, 1);
        assert!(is_recoverable(&rotated, &shed_old, 3));
    }

    // r[verify cryptoshedding.rotation-correctness]
    #[test]
    fn rotate_new_parent_shed_breaks_recoverability() {
        let tree = KeyTree {
            parents: vec![None, None, Some(0)],
        };
        let rotated = rotate(&tree, 2, 1);
        let keystore = Keystore {
            present: vec![true, true, true],
        };
        assert!(is_recoverable(&rotated, &keystore, 2));
        let shed_new = shed(&keystore, 1);
        assert!(!is_recoverable(&rotated, &shed_new, 2));
    }

    proptest! {
        // r[verify cryptoshedding.shed-isolation]
        #[test]
        fn prop_random_shed_sequences_preserve_isolation(
            raw in proptest::collection::vec(any::<usize>(), 1..20),
            leaf_pick in any::<proptest::sample::Index>(),
            sheds in proptest::collection::vec(any::<usize>(), 0..20),
        ) {
            let tree = canonical_tree(&raw);
            let leaves = leaf_indices(&tree);
            let leaf = leaves[leaf_pick.index(leaves.len())];
            let mut keystore = full_keystore(tree.parents.len());
            prop_assert!(is_recoverable(&tree, &keystore, leaf));
            for raw_key in sheds {
                let key = raw_key % tree.parents.len();
                if key != leaf && !on_chain(&tree, leaf, key) {
                    keystore = shed(&keystore, key);
                    prop_assert!(is_recoverable(&tree, &keystore, leaf));
                }
            }
        }

        // r[verify cryptoshedding.shed-completeness]
        // r[verify cryptoshedding.shed-isolation]
        #[test]
        fn prop_completeness_and_isolation_hold(
            raw in proptest::collection::vec(any::<usize>(), 1..20),
            leaf_pick in any::<proptest::sample::Index>(),
            raw_key in any::<usize>(),
        ) {
            let tree = canonical_tree(&raw);
            let leaves = leaf_indices(&tree);
            let leaf = leaves[leaf_pick.index(leaves.len())];
            let key = raw_key % tree.parents.len();
            let keystore = full_keystore(tree.parents.len());
            let shed_key = shed(&keystore, key);
            if key == leaf || on_chain(&tree, leaf, key) {
                prop_assert!(!is_recoverable(&tree, &shed_key, leaf));
            } else {
                prop_assert_eq!(is_recoverable(&tree, &shed_key, leaf), is_recoverable(&tree, &keystore, leaf));
            }
        }

        // r[verify cryptoshedding.rotation-correctness]
        #[test]
        fn prop_valid_rotation_old_parent_shed_preserves_recoverability(
            raw in proptest::collection::vec(any::<usize>(), 2..20),
            leaf_pick in any::<proptest::sample::Index>(),
            parent_pick in any::<proptest::sample::Index>(),
        ) {
            let tree = canonical_tree(&raw);
            let eligible: Vec<usize> = leaf_indices(&tree).into_iter()
                .filter(|&leaf| tree.parents[leaf].is_some() && !valid_rotation_targets(&tree, leaf).is_empty())
                .collect();
            if eligible.is_empty() {
                return Ok(());
            }
            let leaf = eligible[leaf_pick.index(eligible.len())];
            let targets = valid_rotation_targets(&tree, leaf);
            let new_parent = targets[parent_pick.index(targets.len())];
            let old_parent = tree.parents[leaf].unwrap() as usize;
            let rotated = rotate(&tree, leaf, new_parent);
            let keystore = full_keystore(tree.parents.len());
            let shed_old = shed(&keystore, old_parent);
            prop_assert!(is_recoverable(&rotated, &keystore, leaf));
            prop_assert!(is_recoverable(&rotated, &shed_old, leaf));
        }

        // r[verify cryptoshedding.rotation-correctness]
        #[test]
        fn prop_valid_rotation_new_parent_shed_breaks_recoverability(
            raw in proptest::collection::vec(any::<usize>(), 2..20),
            leaf_pick in any::<proptest::sample::Index>(),
            parent_pick in any::<proptest::sample::Index>(),
        ) {
            let tree = canonical_tree(&raw);
            let eligible: Vec<usize> = leaf_indices(&tree).into_iter()
                .filter(|&leaf| !valid_rotation_targets(&tree, leaf).is_empty())
                .collect();
            if eligible.is_empty() {
                return Ok(());
            }
            let leaf = eligible[leaf_pick.index(eligible.len())];
            let targets = valid_rotation_targets(&tree, leaf);
            let new_parent = targets[parent_pick.index(targets.len())];
            let rotated = rotate(&tree, leaf, new_parent);
            let keystore = full_keystore(tree.parents.len());
            let shed_new = shed(&keystore, new_parent);
            prop_assert!(is_recoverable(&rotated, &keystore, leaf));
            prop_assert!(!is_recoverable(&rotated, &shed_new, leaf));
        }
    }
}
