//! Verified B-tree node delete operations.
//!
//! Single-node operations on sorted `Seq<u64>` keys: find a key's
//! position, delete at that position preserving sortedness, merge two
//! sorted halves, and borrow a key from a sibling. Composes with
//! [`btree_balance`] for underflow decisions and [`btree_insert`] for
//! the sortedness predicate.

use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::btree_insert::is_sorted_spec;

verus! {

// ============================================================
// contains_key
// ============================================================

// r[depends btree-delete.contains-key]
/// Spec: key exists in the sorted sequence.
pub open spec fn contains_key_spec(keys: Seq<u64>, key: u64) -> bool {
    exists|i: int| #![trigger keys[i]] 0 <= i < keys.len() && keys[i] == key
}

// r[impl btree-delete.contains-key]
/// Check if a key exists in the sorted node.
pub fn contains_key(keys: &[u64], key: u64) -> (result: bool)
    ensures result == contains_key_spec(keys@, key)
{
    let mut i: usize = 0;
    while i < keys.len()
        invariant
            0 <= i <= keys@.len(),
            !exists|k: int| #![trigger keys@[k]] 0 <= k < i as int && keys@[k] == key,
        decreases keys@.len() - i,
    {
        if keys[i] == key {
            assert(keys@[i as int] == key);
            return true;
        }
        i = i + 1;
    }
    false
}

// ============================================================
// find_key_position
// ============================================================

// r[depends btree-delete.find-key-position]
/// Spec: index of the key, or keys.len() if not found.
pub open spec fn find_key_position_spec(keys: Seq<u64>, key: u64) -> int {
    find_key_from_spec(keys, key, 0)
}

/// Helper: find key from index `from`.
pub open spec fn find_key_from_spec(keys: Seq<u64>, key: u64, from: int) -> int
    decreases keys.len() - from
{
    if from < 0 || from >= keys.len() {
        keys.len() as int
    } else if keys[from] == key {
        from
    } else {
        find_key_from_spec(keys, key, from + 1)
    }
}

// r[impl btree-delete.find-key-position]
/// Find the position of a key in a sorted node.
pub fn find_key_position(keys: &[u64], key: u64) -> (result: usize)
    ensures result == find_key_position_spec(keys@, key) as usize
{
    let mut i: usize = 0;
    while i < keys.len()
        invariant
            0 <= i <= keys@.len(),
            find_key_position_spec(keys@, key) as int
                == find_key_from_spec(keys@, key, i as int),
        decreases keys@.len() - i,
    {
        if keys[i] == key {
            return i;
        }
        i = i + 1;
    }
    i
}

// ============================================================
// delete_at: remove element at position
// ============================================================

// r[depends btree-delete.delete-at]
/// Spec: remove element at position, shifting left.
pub open spec fn delete_at_spec(keys: Seq<u64>, pos: int) -> Seq<u64> {
    keys.subrange(0, pos) + keys.subrange(pos + 1, keys.len() as int)
}

// r[impl btree-delete.delete-at]
/// Delete the element at the given position.
pub fn delete_at(keys: &[u64], pos: usize) -> (result: Vec<u64>)
    requires pos < keys@.len(), keys@.len() <= usize::MAX as int
    ensures result@ == delete_at_spec(keys@, pos as int)
{
    let mut out = Vec::new();
    let mut i: usize = 0;
    while i < pos
        invariant
            0 <= i <= pos,
            pos < keys@.len(),
            out@ =~= keys@.subrange(0, i as int),
        decreases pos - i,
    {
        out.push(keys[i]);
        i = i + 1;
    }
    // Skip keys[pos]
    let j_start: usize = pos + 1;
    let mut j: usize = j_start;
    while j < keys.len()
        invariant
            pos < keys@.len(),
            pos + 1 <= j <= keys@.len(),
            out@ =~= keys@.subrange(0, pos as int) + keys@.subrange((pos + 1) as int, j as int),
        decreases keys@.len() - j,
    {
        out.push(keys[j]);
        j = j + 1;
    }
    assert(out@ =~= delete_at_spec(keys@, pos as int));
    out
}

// ============================================================
// merge_nodes: concatenate two sorted halves
// ============================================================

// r[depends btree-delete.merge-nodes]
/// Spec: merge two sorted sequences into one.
pub open spec fn merge_nodes_spec(left: Seq<u64>, right: Seq<u64>) -> Seq<u64> {
    left + right
}

// r[impl btree-delete.merge-nodes]
/// Merge two sorted node halves where all keys in left < all keys in right.
pub fn merge_nodes(left: &[u64], right: &[u64]) -> (result: Vec<u64>)
    ensures result@ == merge_nodes_spec(left@, right@)
{
    let mut out = Vec::new();
    let mut i: usize = 0;
    while i < left.len()
        invariant
            0 <= i <= left@.len(),
            out@ =~= left@.subrange(0, i as int),
        decreases left@.len() - i,
    {
        out.push(left[i]);
        i = i + 1;
    }
    assert(out@ =~= left@);
    let mut j: usize = 0;
    while j < right.len()
        invariant
            0 <= j <= right@.len(),
            out@ =~= left@ + right@.subrange(0, j as int),
        decreases right@.len() - j,
    {
        out.push(right[j]);
        j = j + 1;
    }
    assert(out@ =~= merge_nodes_spec(left@, right@));
    out
}

// ============================================================
// borrow_left / borrow_right
// ============================================================

// r[depends btree-delete.borrow-left]
/// Spec: borrow the last key from left sibling, prepend to node.
pub open spec fn borrow_left_node_spec(node: Seq<u64>, left: Seq<u64>) -> Seq<u64> {
    seq![left[left.len() - 1]] + node
}

/// Spec: left sibling after donating its last key.
pub open spec fn borrow_left_sibling_spec(left: Seq<u64>) -> Seq<u64> {
    left.subrange(0, left.len() - 1)
}

// r[impl btree-delete.borrow-left]
/// Borrow the last key from the left sibling.
/// Returns (updated_node, updated_left_sibling).
pub fn borrow_left(node: &[u64], left: &[u64]) -> (result: (Vec<u64>, Vec<u64>))
    requires left@.len() > 0
    ensures
        result.0@ == borrow_left_node_spec(node@, left@),
        result.1@ == borrow_left_sibling_spec(left@),
{
    let donated = left[left.len() - 1];
    let mut new_node = Vec::new();
    new_node.push(donated);
    let mut i: usize = 0;
    while i < node.len()
        invariant
            0 <= i <= node@.len(),
            new_node@ =~= seq![donated] + node@.subrange(0, i as int),
        decreases node@.len() - i,
    {
        new_node.push(node[i]);
        i = i + 1;
    }
    assert(new_node@ =~= borrow_left_node_spec(node@, left@));

    let mut new_left = Vec::new();
    let mut j: usize = 0;
    while j < left.len() - 1
        invariant
            0 <= j <= left@.len() - 1,
            left@.len() > 0,
            new_left@ =~= left@.subrange(0, j as int),
        decreases (left@.len() - 1) - j,
    {
        new_left.push(left[j]);
        j = j + 1;
    }
    assert(new_left@ =~= borrow_left_sibling_spec(left@));
    (new_node, new_left)
}

// r[depends btree-delete.borrow-right]
/// Spec: borrow the first key from right sibling, append to node.
pub open spec fn borrow_right_node_spec(node: Seq<u64>, right: Seq<u64>) -> Seq<u64> {
    node + seq![right[0]]
}

/// Spec: right sibling after donating its first key.
pub open spec fn borrow_right_sibling_spec(right: Seq<u64>) -> Seq<u64> {
    right.subrange(1, right.len() as int)
}

// r[impl btree-delete.borrow-right]
/// Borrow the first key from the right sibling.
/// Returns (updated_node, updated_right_sibling).
pub fn borrow_right(node: &[u64], right: &[u64]) -> (result: (Vec<u64>, Vec<u64>))
    requires right@.len() > 0
    ensures
        result.0@ == borrow_right_node_spec(node@, right@),
        result.1@ == borrow_right_sibling_spec(right@),
{
    let donated = right[0];
    let mut new_node = Vec::new();
    let mut i: usize = 0;
    while i < node.len()
        invariant
            0 <= i <= node@.len(),
            new_node@ =~= node@.subrange(0, i as int),
        decreases node@.len() - i,
    {
        new_node.push(node[i]);
        i = i + 1;
    }
    new_node.push(donated);
    assert(new_node@ =~= borrow_right_node_spec(node@, right@));

    let mut new_right = Vec::new();
    let mut j: usize = 1;
    while j < right.len()
        invariant
            1 <= j <= right@.len(),
            right@.len() > 0,
            new_right@ =~= right@.subrange(1, j as int),
        decreases right@.len() - j,
    {
        new_right.push(right[j]);
        j = j + 1;
    }
    assert(new_right@ =~= borrow_right_sibling_spec(right@));
    (new_node, new_right)
}

// ============================================================
// Safety proofs
// ============================================================

// r[verify btree-delete.delete-at]
/// Delete decreases length by exactly 1.
proof fn delete_at_length(keys: Seq<u64>, pos: int)
    requires 0 <= pos < keys.len()
    ensures delete_at_spec(keys, pos).len() == keys.len() - 1
{}

// r[verify btree-delete.delete-at]
/// Deleting from a sorted sequence preserves sortedness.
proof fn delete_preserves_sorted(keys: Seq<u64>, pos: int)
    requires is_sorted_spec(keys), 0 <= pos < keys.len()
    ensures is_sorted_spec(delete_at_spec(keys, pos))
{
    let result = delete_at_spec(keys, pos);
    assert forall|i: int, j: int|
        #![trigger result[i], result[j]]
        0 <= i < j < result.len()
        implies result[i] < result[j]
    by {
        // Map result indices back to original indices
        let orig_i = if i < pos { i } else { i + 1 };
        let orig_j = if j < pos { j } else { j + 1 };
        assert(keys[orig_i] < keys[orig_j]);
    }
}

// r[verify btree-delete.merge-nodes]
/// Merge conserves: result.len == left.len + right.len.
proof fn merge_conserves(left: Seq<u64>, right: Seq<u64>)
    ensures merge_nodes_spec(left, right).len() == left.len() + right.len()
{}

// r[verify btree-delete.merge-nodes]
/// Merging two sorted halves where left < right produces a sorted result.
proof fn merge_preserves_sorted(left: Seq<u64>, right: Seq<u64>)
    requires
        is_sorted_spec(left),
        is_sorted_spec(right),
        // All keys in left < all keys in right
        forall|i: int, j: int|
            #![trigger left[i], right[j]]
            0 <= i < left.len() && 0 <= j < right.len()
            ==> left[i] < right[j],
    ensures is_sorted_spec(merge_nodes_spec(left, right))
{
    let result = merge_nodes_spec(left, right);
    assert forall|a: int, b: int|
        #![trigger result[a], result[b]]
        0 <= a < b < result.len()
        implies result[a] < result[b]
    by {
        if a < left.len() as int && b < left.len() as int {
            assert(left[a] < left[b]);
        } else if a < left.len() as int && b >= left.len() as int {
            assert(left[a] < right[b - left.len() as int]);
        } else {
            // Both in right
            assert(right[a - left.len() as int] < right[b - left.len() as int]);
        }
    }
}

// r[verify btree-delete.borrow-left]
/// After borrowing from left, combined count is unchanged.
proof fn borrow_left_conserves(node: Seq<u64>, left: Seq<u64>)
    requires left.len() > 0
    ensures
        borrow_left_node_spec(node, left).len() == node.len() + 1,
        borrow_left_sibling_spec(left).len() == left.len() - 1,
{}

// r[verify btree-delete.borrow-right]
/// After borrowing from right, combined count is unchanged.
proof fn borrow_right_conserves(node: Seq<u64>, right: Seq<u64>)
    requires right.len() > 0
    ensures
        borrow_right_node_spec(node, right).len() == node.len() + 1,
        borrow_right_sibling_spec(right).len() == right.len() - 1,
{}

// r[verify btree-delete.contains-key]
/// A key at a known position is contained.
proof fn key_at_position_is_contained(keys: Seq<u64>, pos: int)
    requires 0 <= pos < keys.len()
    ensures contains_key_spec(keys, keys[pos])
{
    assert(keys[pos] == keys[pos]);
}

// r[verify btree-delete.find-key-position]
/// Position is in range [0, keys.len()].
proof fn find_key_in_range(keys: Seq<u64>, key: u64)
    ensures
        0 <= find_key_position_spec(keys, key),
        find_key_position_spec(keys, key) <= keys.len() as int,
{
    find_key_in_range_from(keys, key, 0);
}

proof fn find_key_in_range_from(keys: Seq<u64>, key: u64, from: int)
    requires 0 <= from <= keys.len()
    ensures
        from <= find_key_from_spec(keys, key, from),
        find_key_from_spec(keys, key, from) <= keys.len() as int,
    decreases keys.len() - from
{
    if from < keys.len() as int && keys[from] != key {
        find_key_in_range_from(keys, key, from + 1);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::btree_insert::is_sorted;

    // r[verify btree-delete.contains-key]
    #[test]
    fn test_contains_key() {
        assert!(contains_key(&[10, 20, 30], 20));
        assert!(!contains_key(&[10, 20, 30], 15));
        assert!(!contains_key(&[], 5));
    }

    // r[verify btree-delete.find-key-position]
    #[test]
    fn test_find_key_position() {
        assert_eq!(find_key_position(&[10, 20, 30], 20), 1);
        assert_eq!(find_key_position(&[10, 20, 30], 15), 3); // not found
        assert_eq!(find_key_position(&[], 5), 0);
    }

    // r[verify btree-delete.delete-at]
    #[test]
    fn test_delete_at() {
        let result = delete_at(&[10, 20, 30, 40], 1);
        assert_eq!(result, vec![10, 30, 40]);

        let result2 = delete_at(&[10, 20, 30], 0);
        assert_eq!(result2, vec![20, 30]);

        let result3 = delete_at(&[10, 20, 30], 2);
        assert_eq!(result3, vec![10, 20]);
    }

    // r[verify btree-delete.delete-at]
    #[test]
    fn test_delete_preserves_sorted() {
        let keys = vec![10u64, 20, 30, 40, 50];
        for pos in 0..keys.len() {
            let result = delete_at(&keys, pos);
            assert!(is_sorted(&result));
            assert_eq!(result.len(), keys.len() - 1);
        }
    }

    // r[verify btree-delete.merge-nodes]
    #[test]
    fn test_merge_nodes() {
        let result = merge_nodes(&[10, 20], &[30, 40, 50]);
        assert_eq!(result, vec![10, 20, 30, 40, 50]);
        assert!(is_sorted(&result));
    }

    // r[verify btree-delete.borrow-left]
    #[test]
    fn test_borrow_left() {
        let (new_node, new_left) = borrow_left(&[30, 40], &[10, 20]);
        assert_eq!(new_node, vec![20, 30, 40]);
        assert_eq!(new_left, vec![10]);
    }

    // r[verify btree-delete.borrow-right]
    #[test]
    fn test_borrow_right() {
        let (new_node, new_right) = borrow_right(&[10, 20], &[30, 40, 50]);
        assert_eq!(new_node, vec![10, 20, 30]);
        assert_eq!(new_right, vec![40, 50]);
    }

    proptest! {
        // r[verify btree-delete.delete-at]
        #[test]
        fn prop_delete_preserves_sorted(
            keys in proptest::collection::vec(0u64..1000, 1..20),
        ) {
            let mut sorted = keys;
            sorted.sort();
            sorted.dedup();
            if !sorted.is_empty() {
                for pos in 0..sorted.len() {
                    let result = delete_at(&sorted, pos);
                    prop_assert!(is_sorted(&result));
                    prop_assert_eq!(result.len(), sorted.len() - 1);
                }
            }
        }

        // r[verify btree-delete.merge-nodes]
        #[test]
        fn prop_merge_sorted(
            keys in proptest::collection::vec(0u64..1000, 2..20),
        ) {
            let mut sorted = keys;
            sorted.sort();
            sorted.dedup();
            if sorted.len() >= 2 {
                let mid = sorted.len() / 2;
                let left = &sorted[..mid];
                let right = &sorted[mid..];
                let result = merge_nodes(left, right);
                prop_assert!(is_sorted(&result));
                prop_assert_eq!(result.len(), sorted.len());
            }
        }

        // r[verify btree-delete.borrow-left]
        // r[verify btree-delete.borrow-right]
        #[test]
        fn prop_borrow_conserves(
            keys in proptest::collection::vec(0u64..1000, 3..20),
        ) {
            let mut sorted = keys;
            sorted.sort();
            sorted.dedup();
            if sorted.len() >= 3 {
                let mid = sorted.len() / 2;
                let node = &sorted[..mid];
                let right = &sorted[mid..];

                let (new_node, new_right) = borrow_right(node, right);
                prop_assert_eq!(new_node.len() + new_right.len(), node.len() + right.len());

                let left = &sorted[..mid];
                let node2 = &sorted[mid..];
                let (new_node2, new_left2) = borrow_left(node2, left);
                prop_assert_eq!(new_node2.len() + new_left2.len(), node2.len() + left.len());
            }
        }

        // r[verify btree-delete.contains-key]
        #[test]
        fn prop_contains_present(
            keys in proptest::collection::vec(0u64..100, 1..20),
            idx in 0usize..20,
        ) {
            let mut sorted = keys;
            sorted.sort();
            sorted.dedup();
            if !sorted.is_empty() && idx < sorted.len() {
                prop_assert!(contains_key(&sorted, sorted[idx]));
            }
        }

        // r[verify btree-delete.contains-key]
        #[test]
        fn prop_not_contains_absent(
            keys in proptest::collection::vec(0u64..100, 0..20),
            key in 200u64..300,
        ) {
            let mut sorted = keys;
            sorted.sort();
            sorted.dedup();
            prop_assert!(!contains_key(&sorted, key));
        }
    }
}
