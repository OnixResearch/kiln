//! Session tree operations as compositions of forest and `seq_set` primitives.
//!
//! Maps clankers session-tree operations to generic verified building blocks:
//! `find_unique` = `set_difference(ancestor_path)`, `find_divergence` = lca,
//! `selective_merge` = `set_intersection`, `cherry_pick_subtree` = subtree.

use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::forest::*;
#[cfg(verus_keep_ghost)]
use crate::seq_set::*;

verus! {

// ============================================================
// find_unique — set difference of two ancestor paths
// ============================================================

// r[depends session-tree.unique-messages]
/// Spec: nodes on source branch not on target branch.
pub open spec fn find_unique_spec(
    parents: Seq<Option<int>>, src: int, tgt: int,
) -> Seq<int> {
    set_difference_spec(
        ancestor_path_spec(parents, src),
        ancestor_path_spec(parents, tgt),
    )
}

// ============================================================
// find_divergence — LCA of two branches
// ============================================================

// r[depends session-tree.divergence-point]
/// Spec: last common ancestor of two branches.
pub open spec fn find_divergence_spec(
    parents: Seq<Option<int>>, a: int, b: int,
) -> int {
    lca_spec(parents, a, b)
}

// ============================================================
// selective_merge — intersection of selection with unique set
// ============================================================

// r[depends session-tree.selective-merge]
/// Spec: nodes selected by user that are actually unique to source.
pub open spec fn selective_merge_spec(
    selected: Seq<int>, unique: Seq<int>,
) -> Seq<int> {
    set_intersection_spec(selected, unique)
}

// ============================================================
// cherry_pick_subtree — subtree reachability
// ============================================================

// r[depends session-tree.cherry-pick-subtree]
/// Spec: all nodes reachable from root via child pointers.
pub open spec fn cherry_pick_subtree_spec(
    parents: Seq<Option<int>>, root: int,
) -> Seq<int> {
    subtree_spec(parents, root)
}

// ============================================================
// merge_branch_count — length of unique set
// ============================================================

// r[depends session-tree.merge-branch-count]
/// Spec: count of unique messages between two branches.
pub open spec fn merge_branch_count_spec(
    parents: Seq<Option<int>>, src: int, tgt: int,
) -> nat {
    find_unique_spec(parents, src, tgt).len()
}

// ============================================================
// find_latest_leaf — max-index leaf
// ============================================================

// r[depends session-tree.latest-leaf]
/// Spec: highest-indexed leaf node.
pub open spec fn find_latest_leaf_spec(parents: Seq<Option<int>>) -> int
{
    latest_leaf_from(parents, parents.len() as int - 1)
}

/// Helper: scan backward for first leaf.
pub open spec fn latest_leaf_from(parents: Seq<Option<int>>, from: int) -> int
    decreases if from >= 0 { from + 1 } else { 0 },
{
    if from < 0 {
        -1 // no nodes
    } else if is_leaf_spec(parents, from) {
        from
    } else {
        latest_leaf_from(parents, from - 1)
    }
}

// r[impl session-tree.latest-leaf]
/// Find the highest-indexed leaf node (most recently added leaf).
pub fn find_latest_leaf(parents: &[Option<usize>]) -> (result: Option<usize>)
    requires parents@.len() > 0
    ensures ({
        let pm = parents@.map(|_i: int, p: Option<usize>| match p {
            Some(j) => Some(j as int),
            None => None::<int>,
        });
        match result {
            Some(v) => v as int == find_latest_leaf_spec(pm) && is_leaf_spec(pm, v as int),
            None => find_latest_leaf_spec(pm) < 0,
        }
    })
{
    let ghost pm = parents@.map(|_i: int, p: Option<usize>| match p {
        Some(j) => Some(j as int),
        None => None::<int>,
    });
    let mut i = parents.len();
    while i > 0
        invariant
            0 <= i <= parents@.len(),
            pm =~= parents@.map(|_i: int, p: Option<usize>| match p {
                Some(j) => Some(j as int),
                None => None::<int>,
            }),
            // No leaf in [i, parents.len())
            forall|k: int| i as int <= k < parents@.len() as int ==>
                !is_leaf_spec(pm, k),
            // Connects to spec: latest_leaf_from scanned past [i, len)
            latest_leaf_from(pm, parents@.len() as int - 1) ==
                latest_leaf_from(pm, i as int - 1),
        decreases i,
    {
        i -= 1;
        if crate::forest::is_leaf(parents, i) {
            proof {
                assert(is_leaf_spec(pm, i as int));
                // latest_leaf_from(pm, i) == i since is_leaf
            }
            return Some(i);
        }
    }
    None
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify session-tree.unique-messages]
    #[test]
    fn test_find_unique() {
        // Fork: root 0, branch A: 0→1→3, branch B: 0→2→4
        let p = [None, Some(0), Some(0), Some(1), Some(2)];
        let path_a = crate::forest::ancestor_path(&p, 3); // [0, 1, 3]
        let path_b = crate::forest::ancestor_path(&p, 4); // [0, 2, 4]
        // Convert to i64 for seq_set
        let a_i64: Vec<i64> = path_a.iter().map(|&x| i64::try_from(x).unwrap()).collect();
        let b_i64: Vec<i64> = path_b.iter().map(|&x| i64::try_from(x).unwrap()).collect();
        let unique = crate::seq_set::set_difference(&a_i64, &b_i64, 10);
        assert_eq!(unique, vec![1, 3]);
    }

    // r[verify session-tree.divergence-point]
    #[test]
    fn test_find_divergence() {
        // Fork at root
        let p = [None, Some(0), Some(0)];
        assert_eq!(crate::forest::lca(&p, 1, 2), Some(0));

        // Fork mid-chain
        let p = [None, Some(0), Some(1), Some(1)];
        assert_eq!(crate::forest::lca(&p, 2, 3), Some(1));
    }

    // r[verify session-tree.selective-merge]
    #[test]
    fn test_selective_merge() {
        // unique = [1, 3, 5], selected = [1, 5]
        let unique: Vec<i64> = vec![1, 3, 5];
        let selected: Vec<i64> = vec![1, 5];
        let merged = crate::seq_set::set_intersection(&selected, &unique, 10);
        assert_eq!(merged, vec![1, 5]);

        // Selection includes non-unique
        let unique2: Vec<i64> = vec![1, 3];
        let selected2: Vec<i64> = vec![1, 2, 3];
        let merged2 = crate::seq_set::set_intersection(&selected2, &unique2, 10);
        assert_eq!(merged2, vec![1, 3]);
    }

    // r[verify session-tree.cherry-pick-subtree]
    #[test]
    fn test_cherry_pick_subtree() {
        let p = [None, Some(0), Some(1), Some(1)];
        let sub = crate::forest::subtree(&p, 1);
        assert_eq!(sub, vec![1, 2, 3]);

        // Sibling not included
        let p2 = [None, Some(0), Some(0)];
        let sub2 = crate::forest::subtree(&p2, 1);
        assert_eq!(sub2, vec![1]);
    }

    // r[verify session-tree.merge-branch-count]
    #[test]
    fn test_merge_branch_count() {
        let p = [None, Some(0), Some(0), Some(1), Some(2)];
        let path_a = crate::forest::ancestor_path(&p, 3);
        let path_b = crate::forest::ancestor_path(&p, 4);
        let a_i64: Vec<i64> = path_a.iter().map(|&x| i64::try_from(x).unwrap()).collect();
        let b_i64: Vec<i64> = path_b.iter().map(|&x| i64::try_from(x).unwrap()).collect();
        let unique = crate::seq_set::set_difference(&a_i64, &b_i64, 10);
        assert_eq!(unique.len(), 2); // nodes 1 and 3
    }

    // r[verify session-tree.latest-leaf]
    #[test]
    fn test_find_latest_leaf() {
        let p = [None, Some(0), Some(0), Some(1)];
        assert_eq!(find_latest_leaf(&p), Some(3));

        let p2: [Option<usize>; 1] = [None];
        assert_eq!(find_latest_leaf(&p2), Some(0));
    }

    // Generate random well-formed parent maps
    fn well_formed_parents(max_size: usize) -> impl Strategy<Value = Vec<Option<usize>>> {
        (1..=max_size).prop_flat_map(|n| {
            proptest::collection::vec(proptest::option::of(0usize..100), n..=n).prop_map(move |raw| {
                raw.into_iter()
                    .enumerate()
                    .map(|(i, opt)| {
                        if i == 0 {
                            None
                        } else {
                            match opt {
                                Some(v) => Some(v % i),
                                None => None,
                            }
                        }
                    })
                    .collect()
            })
        })
    }

    proptest! {
        // r[verify session-tree.unique-messages]
        #[test]
        fn prop_unique_subset_of_source(parents in well_formed_parents(15)) {
            if parents.len() >= 2 {
                let src = parents.len() - 1;
                let tgt = 0;
                let path_src = crate::forest::ancestor_path(&parents, src);
                let path_tgt = crate::forest::ancestor_path(&parents, tgt);
                let src_i64: Vec<i64> = path_src.iter().map(|&x| i64::try_from(x).unwrap()).collect();
                let tgt_i64: Vec<i64> = path_tgt.iter().map(|&x| i64::try_from(x).unwrap()).collect();
                let unique = crate::seq_set::set_difference(&src_i64, &tgt_i64, parents.len() + 1);
                // Every unique node is on source path
                for &u in &unique {
                    prop_assert!(src_i64.contains(&u), "unique {} not on source path", u);
                    prop_assert!(!tgt_i64.contains(&u), "unique {} on target path", u);
                }
            }
        }

        // r[verify session-tree.selective-merge]
        #[test]
        fn prop_selective_subset_of_unique(parents in well_formed_parents(15)) {
            if parents.len() >= 3 {
                let src = parents.len() - 1;
                let tgt = 0;
                let path_src = crate::forest::ancestor_path(&parents, src);
                let path_tgt = crate::forest::ancestor_path(&parents, tgt);
                let src_i64: Vec<i64> = path_src.iter().map(|&x| i64::try_from(x).unwrap()).collect();
                let tgt_i64: Vec<i64> = path_tgt.iter().map(|&x| i64::try_from(x).unwrap()).collect();
                let unique = crate::seq_set::set_difference(&src_i64, &tgt_i64, parents.len() + 1);
                // Select first half
                let selected: Vec<i64> = unique.iter().take(unique.len() / 2 + 1).copied().collect();
                let cap = parents.len() + 1;
                let merged = crate::seq_set::set_intersection(&selected, &unique, cap);
                // merged ⊆ unique
                for &m in &merged {
                    prop_assert!(unique.contains(&m));
                }
                // merged ⊆ selected
                for &m in &merged {
                    prop_assert!(selected.contains(&m));
                }
            }
        }

        // r[verify session-tree.latest-leaf]
        #[test]
        fn prop_latest_leaf_is_leaf(parents in well_formed_parents(15)) {
            if let Some(leaf) = find_latest_leaf(&parents) {
                prop_assert!(crate::forest::is_leaf(&parents, leaf),
                    "latest leaf {} is not a leaf", leaf);
                // No leaf has higher index
                for n in (leaf + 1)..parents.len() {
                    prop_assert!(!crate::forest::is_leaf(&parents, n),
                        "node {} > latest leaf {} is also a leaf", n, leaf);
                }
            }
        }

        #[test]
        fn prop_single_node_is_leaf(dummy in 0u8..1) {
            let _ = dummy;
            let parents = vec![None];
            let leaf = find_latest_leaf(&parents);
            prop_assert_eq!(leaf, Some(0));
        }

        #[test]
        fn prop_linear_chain_leaf_is_last(len in 2usize..=10) {
            let mut parents: Vec<Option<usize>> = vec![None];
            for i in 1..len {
                parents.push(Some(i - 1));
            }
            let leaf = find_latest_leaf(&parents);
            prop_assert_eq!(leaf, Some(len - 1));
        }
    }
}
