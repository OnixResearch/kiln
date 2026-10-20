//! Verified B-tree node insert operations.
//!
//! Single-node operations on sorted `Seq<u64>` keys: find the insert
//! position, insert at that position preserving sortedness, and split
//! an overfull node into two sorted halves. Composes with
//! [`btree_balance`] for capacity decisions.

use vstd::prelude::*;

verus! {

// ============================================================
// is_sorted: strict sortedness predicate
// ============================================================

// r[depends btree-insert.is-sorted]
/// Spec: keys are strictly increasing.
pub open spec fn is_sorted_spec(keys: Seq<u64>) -> bool {
    forall|i: int, j: int|
        #![trigger keys[i], keys[j]]
        0 <= i < j < keys.len() ==> keys[i] < keys[j]
}

// r[impl btree-insert.is-sorted]
/// Check if a key sequence is strictly sorted.
pub fn is_sorted(keys: &[u64]) -> (result: bool)
    ensures result == is_sorted_spec(keys@)
{
    if keys.len() <= 1 {
        return true;
    }
    let mut i: usize = 1;
    while i < keys.len()
        invariant
            1 <= i <= keys@.len(),
            forall|a: int, b: int|
                #![trigger keys@[a], keys@[b]]
                0 <= a < b < i ==> keys@[a] < keys@[b],
        decreases keys@.len() - i,
    {
        if keys[i - 1] >= keys[i] {
            // Found violation: keys[i-1] >= keys[i]
            // Need to show !is_sorted_spec: witness i-1, i
            assert(keys@[(i - 1) as int] >= keys@[i as int]);
            return false;
        }
        // Extend the sorted prefix: for any a < b < i+1,
        // if b < i, covered by invariant; if b == i, then
        // keys[a] < keys[i-1] < keys[i] by transitivity (a < i-1)
        // or a == i-1 and we just checked keys[i-1] < keys[i]
        assert forall|a: int, b: int|
            #![trigger keys@[a], keys@[b]]
            0 <= a < b < (i + 1) as int
            implies keys@[a] < keys@[b]
        by {
            if b == i as int {
                if a == (i - 1) as int {
                    // Direct: keys[i-1] < keys[i]
                } else {
                    // a < i-1 < i: keys[a] < keys[i-1] < keys[i]
                    assert(keys@[a] < keys@[(i - 1) as int]);
                }
            }
        }
        i = i + 1;
    }
    true
}

// ============================================================
// find_insert_position
// ============================================================

// r[depends btree-insert.find-position]
/// Spec: position where key should be inserted.
/// Returns the first index where keys[index] > key (or keys.len()).
pub open spec fn find_insert_position_spec(keys: Seq<u64>, key: u64) -> int {
    find_position_from_spec(keys, key, 0)
}

/// Helper: find position from index `from`.
pub open spec fn find_position_from_spec(keys: Seq<u64>, key: u64, from: int) -> int
    decreases keys.len() - from
{
    if from < 0 || from >= keys.len() {
        keys.len() as int
    } else if keys[from] > key {
        from
    } else {
        find_position_from_spec(keys, key, from + 1)
    }
}

// r[impl btree-insert.find-position]
/// Find the position where a key should be inserted to maintain sortedness.
pub fn find_insert_position(keys: &[u64], key: u64) -> (result: usize)
    ensures result == find_insert_position_spec(keys@, key) as usize
{
    let mut i: usize = 0;
    while i < keys.len()
        invariant
            0 <= i <= keys@.len(),
            find_insert_position_spec(keys@, key) as int
                == find_position_from_spec(keys@, key, i as int),
        decreases keys@.len() - i,
    {
        if keys[i] > key {
            return i;
        }
        i = i + 1;
    }
    i
}

// ============================================================
// insert_at: insert a key at a given position
// ============================================================

// r[depends btree-insert.insert-at]
/// Spec: insert key at position pos, shifting elements right.
pub open spec fn insert_at_spec(keys: Seq<u64>, pos: int, key: u64) -> Seq<u64> {
    keys.subrange(0, pos) + seq![key] + keys.subrange(pos, keys.len() as int)
}

// r[impl btree-insert.insert-at]
/// Insert a key at the given position in a sorted node.
pub fn insert_at(keys: &[u64], pos: usize, key: u64) -> (result: Vec<u64>)
    requires pos as int <= keys@.len()
    ensures result@ == insert_at_spec(keys@, pos as int, key)
{
    let mut out = Vec::new();
    let mut i: usize = 0;
    // Copy elements before pos
    while i < pos
        invariant
            0 <= i <= pos,
            pos as int <= keys@.len(),
            out@ =~= keys@.subrange(0, i as int),
        decreases pos - i,
    {
        out.push(keys[i]);
        i = i + 1;
    }
    // Insert the key
    out.push(key);
    // Copy elements from pos onward
    let mut j: usize = pos;
    while j < keys.len()
        invariant
            pos <= j <= keys@.len(),
            pos as int <= keys@.len(),
            out@ =~= keys@.subrange(0, pos as int) + seq![key] + keys@.subrange(pos as int, j as int),
        decreases keys@.len() - j,
    {
        out.push(keys[j]);
        j = j + 1;
    }
    assert(out@ =~= insert_at_spec(keys@, pos as int, key));
    out
}

// ============================================================
// split_node: split at midpoint into two sorted halves
// ============================================================

// r[depends btree-insert.split-node]
/// Spec: left half of split.
pub open spec fn split_left_spec(keys: Seq<u64>, mid: int) -> Seq<u64> {
    keys.subrange(0, mid)
}

/// Spec: right half of split.
pub open spec fn split_right_spec(keys: Seq<u64>, mid: int) -> Seq<u64> {
    keys.subrange(mid, keys.len() as int)
}

// r[impl btree-insert.split-node]
/// Split a node's keys at the midpoint into (left, right).
pub fn split_node(keys: &[u64], mid: usize) -> (result: (Vec<u64>, Vec<u64>))
    requires mid as int <= keys@.len()
    ensures
        result.0@ == split_left_spec(keys@, mid as int),
        result.1@ == split_right_spec(keys@, mid as int),
{
    let mut left = Vec::new();
    let mut right = Vec::new();

    let mut i: usize = 0;
    while i < mid
        invariant
            0 <= i <= mid,
            mid as int <= keys@.len(),
            left@ =~= keys@.subrange(0, i as int),
        decreases mid - i,
    {
        left.push(keys[i]);
        i = i + 1;
    }

    let mut j: usize = mid;
    while j < keys.len()
        invariant
            mid <= j <= keys@.len(),
            mid as int <= keys@.len(),
            right@ =~= keys@.subrange(mid as int, j as int),
        decreases keys@.len() - j,
    {
        right.push(keys[j]);
        j = j + 1;
    }

    assert(left@ =~= split_left_spec(keys@, mid as int));
    assert(right@ =~= split_right_spec(keys@, mid as int));
    (left, right)
}

// ============================================================
// Safety proofs
// ============================================================

// r[verify btree-insert.is-sorted]
/// Empty and singleton sequences are sorted.
proof fn empty_is_sorted()
    ensures is_sorted_spec(seq![]), is_sorted_spec(seq![42u64])
{}

// r[verify btree-insert.find-position]
/// Position is in range [0, keys.len()].
proof fn find_position_in_range(keys: Seq<u64>, key: u64)
    ensures
        0 <= find_insert_position_spec(keys, key),
        find_insert_position_spec(keys, key) <= keys.len() as int,
{
    find_position_in_range_from(keys, key, 0);
}

proof fn find_position_in_range_from(keys: Seq<u64>, key: u64, from: int)
    requires 0 <= from <= keys.len()
    ensures
        from <= find_position_from_spec(keys, key, from),
        find_position_from_spec(keys, key, from) <= keys.len() as int,
    decreases keys.len() - from
{
    if from < keys.len() as int && !(keys[from] > key) {
        find_position_in_range_from(keys, key, from + 1);
    }
}

// r[verify btree-insert.find-position]
/// All keys before the position are <= key.
proof fn find_position_left_bound(keys: Seq<u64>, key: u64)
    ensures forall|i: int|
        #![trigger keys[i]]
        0 <= i < find_insert_position_spec(keys, key)
        ==> keys[i] <= key
{
    find_position_left_bound_from(keys, key, 0);
}

proof fn find_position_left_bound_from(keys: Seq<u64>, key: u64, from: int)
    requires 0 <= from <= keys.len()
    ensures forall|i: int|
        #![trigger keys[i]]
        from <= i < find_position_from_spec(keys, key, from)
        ==> keys[i] <= key
    decreases keys.len() - from
{
    if from < keys.len() as int && !(keys[from] > key) {
        find_position_left_bound_from(keys, key, from + 1);
    }
}

// Helper: in a sorted sequence, all keys at or after the found position are > key.
proof fn find_position_right_bound(keys: Seq<u64>, key: u64)
    requires is_sorted_spec(keys)
    ensures forall|i: int|
        #![trigger keys[i]]
        find_insert_position_spec(keys, key) <= i < keys.len()
        ==> keys[i] > key
{
    find_position_right_bound_from(keys, key, 0);
}

proof fn find_position_right_bound_from(keys: Seq<u64>, key: u64, from: int)
    requires
        0 <= from <= keys.len(),
        is_sorted_spec(keys),
    ensures forall|i: int|
        #![trigger keys[i]]
        find_position_from_spec(keys, key, from) <= i < keys.len()
        ==> keys[i] > key
    decreases keys.len() - from
{
    if from < keys.len() as int {
        if keys[from] > key {
            // pos == from; for sorted keys, all i >= from have keys[i] >= keys[from] > key
        } else {
            find_position_right_bound_from(keys, key, from + 1);
        }
    }
}

// r[verify btree-insert.insert-at]
/// Insert increases length by exactly 1.
proof fn insert_at_length(keys: Seq<u64>, pos: int, key: u64)
    requires 0 <= pos <= keys.len()
    ensures insert_at_spec(keys, pos, key).len() == keys.len() + 1
{}

// r[verify btree-insert.split-node]
/// Split conserves: left.len + right.len == keys.len.
proof fn split_conserves(keys: Seq<u64>, mid: int)
    requires 0 <= mid <= keys.len()
    ensures
        split_left_spec(keys, mid).len() + split_right_spec(keys, mid).len()
            == keys.len()
{}

// r[verify btree-insert.split-node]
/// Left half of a sorted sequence is sorted.
proof fn split_left_sorted(keys: Seq<u64>, mid: int)
    requires is_sorted_spec(keys), 0 <= mid <= keys.len()
    ensures is_sorted_spec(split_left_spec(keys, mid))
{
    let left = split_left_spec(keys, mid);
    assert forall|i: int, j: int|
        #![trigger left[i], left[j]]
        0 <= i < j < left.len()
        implies left[i] < left[j]
    by {
        assert(keys[i] < keys[j]);
    }
}

// r[verify btree-insert.split-node]
/// Right half of a sorted sequence is sorted.
proof fn split_right_sorted(keys: Seq<u64>, mid: int)
    requires is_sorted_spec(keys), 0 <= mid <= keys.len()
    ensures is_sorted_spec(split_right_spec(keys, mid))
{
    let right = split_right_spec(keys, mid);
    assert forall|i: int, j: int|
        #![trigger right[i], right[j]]
        0 <= i < j < right.len()
        implies right[i] < right[j]
    by {
        assert(keys[mid + i] < keys[mid + j]);
    }
}

// r[verify btree-insert.split-node]
/// All keys in left < all keys in right.
proof fn split_left_lt_right(keys: Seq<u64>, mid: int)
    requires is_sorted_spec(keys), 0 < mid, mid < keys.len()
    ensures
        forall|i: int, j: int|
            #![trigger split_left_spec(keys, mid)[i], split_right_spec(keys, mid)[j]]
            0 <= i < split_left_spec(keys, mid).len()
            && 0 <= j < split_right_spec(keys, mid).len()
            ==> split_left_spec(keys, mid)[i] < split_right_spec(keys, mid)[j]
{
    let left = split_left_spec(keys, mid);
    let right = split_right_spec(keys, mid);
    assert forall|i: int, j: int|
        #![trigger left[i], right[j]]
        0 <= i < left.len() && 0 <= j < right.len()
        implies left[i] < right[j]
    by {
        // left[i] == keys[i], right[j] == keys[mid + j]
        // i < mid <= mid + j, so keys[i] < keys[mid + j]
        assert(keys[i] < keys[mid + j]);
    }
}

// r[verify btree-insert.insert-at]
/// Inserting into a sorted sequence at the correct position preserves sortedness,
/// when the key is not already present.
proof fn insert_preserves_sorted(keys: Seq<u64>, key: u64)
    requires
        is_sorted_spec(keys),
        // key not already in keys
        forall|i: int| #![trigger keys[i]] 0 <= i < keys.len() ==> keys[i] != key,
    ensures ({
        let pos = find_insert_position_spec(keys, key);
        is_sorted_spec(insert_at_spec(keys, pos, key))
    })
{
    let pos = find_insert_position_spec(keys, key);
    find_position_in_range(keys, key);
    find_position_left_bound(keys, key);
    find_position_right_bound(keys, key);
    let result = insert_at_spec(keys, pos, key);

    // result = keys[0..pos] + [key] + keys[pos..len]
    // left_bound: keys[i] <= key for i < pos
    // right_bound: keys[i] > key for i >= pos (uses sortedness)
    // precondition: keys[i] != key for all i, so keys[i] < key for i < pos
    assert forall|a: int, b: int|
        #![trigger result[a], result[b]]
        0 <= a < b < result.len()
        implies result[a] < result[b]
    by {
        if a < pos && b < pos {
            assert(keys[a] < keys[b]);
        } else if a < pos && b == pos {
            // keys[a] <= key and keys[a] != key, so keys[a] < key
            assert(keys[a] <= key);
            assert(keys[a] != key);
        } else if a == pos && b > pos {
            // key < keys[b-1] from right_bound (b-1 >= pos)
            assert(keys[(b - 1) as int] > key);
        } else if a < pos && b > pos {
            assert(keys[a] < keys[(b - 1) as int]);
        } else if a > pos && b > pos {
            assert(keys[(a - 1) as int] < keys[(b - 1) as int]);
        }
    }
}

// r[verify btree-insert.split-node]
/// Split sizes match btree_balance split_left/right_size.
proof fn split_sizes_match_balance(keys: Seq<u64>)
    requires keys.len() > 0, keys.len() <= u64::MAX as int
    ensures ({
        let mid = crate::btree_balance::split_point_spec(keys.len() as u64);
        split_left_spec(keys, mid as int).len() as u64
            == crate::btree_balance::split_left_size_spec(keys.len() as u64)
        && split_right_spec(keys, mid as int).len() as u64
            == crate::btree_balance::split_right_size_spec(keys.len() as u64)
    })
{
    // split_point_spec(n) = n/2, left.len = mid, right.len = n - mid
    // split_left_size_spec(n) = split_point_spec(n) = n/2
    // split_right_size_spec(n) = n - split_point_spec(n) = n - n/2
    let n = keys.len() as u64;
    let mid = crate::btree_balance::split_point_spec(n);
    assert(split_left_spec(keys, mid as int).len() == mid as int);
    assert(split_right_spec(keys, mid as int).len() == keys.len() - mid as int);
}

} // verus!

#[cfg(test)]
#[allow(clippy::cast_possible_truncation)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify btree-insert.is-sorted]
    #[test]
    fn test_is_sorted() {
        assert!(is_sorted(&[]));
        assert!(is_sorted(&[5]));
        assert!(is_sorted(&[1, 3, 5, 7]));
        assert!(!is_sorted(&[1, 5, 3, 7]));
        assert!(!is_sorted(&[1, 1])); // duplicates not sorted
    }

    // r[verify btree-insert.find-position]
    #[test]
    fn test_find_position() {
        assert_eq!(find_insert_position(&[10, 20, 30], 5), 0);
        assert_eq!(find_insert_position(&[10, 20, 30], 15), 1);
        assert_eq!(find_insert_position(&[10, 20, 30], 25), 2);
        assert_eq!(find_insert_position(&[10, 20, 30], 35), 3);
        assert_eq!(find_insert_position(&[], 5), 0);
    }

    // r[verify btree-insert.insert-at]
    #[test]
    fn test_insert_at() {
        let result = insert_at(&[10, 30, 50], 1, 20);
        assert_eq!(result, vec![10, 20, 30, 50]);

        let result2 = insert_at(&[10, 30], 0, 5);
        assert_eq!(result2, vec![5, 10, 30]);

        let result3 = insert_at(&[10, 30], 2, 50);
        assert_eq!(result3, vec![10, 30, 50]);
    }

    // r[verify btree-insert.insert-at]
    #[test]
    fn test_insert_preserves_sorted() {
        let keys = vec![10u64, 20, 30, 40, 50];
        for key in [5, 15, 25, 35, 45, 55] {
            let pos = find_insert_position(&keys, key);
            let result = insert_at(&keys, pos, key);
            assert!(is_sorted(&result));
            assert_eq!(result.len(), keys.len() + 1);
        }
    }

    // r[verify btree-insert.split-node]
    #[test]
    fn test_split_node() {
        let keys = vec![10u64, 20, 30, 40, 50];
        let mid = crate::btree_balance::split_point(5) as usize;
        let (left, right) = split_node(&keys, mid);
        assert_eq!(left, vec![10, 20]);
        assert_eq!(right, vec![30, 40, 50]);
        assert_eq!(left.len() + right.len(), keys.len());
        assert!(is_sorted(&left));
        assert!(is_sorted(&right));
    }

    // r[verify btree-insert.split-node]
    #[test]
    fn test_split_left_lt_right() {
        let keys = vec![10u64, 20, 30, 40, 50];
        let (left, right) = split_node(&keys, 2);
        for l in &left {
            for r in &right {
                assert!(l < r);
            }
        }
    }

    proptest! {
        // r[verify btree-insert.is-sorted]
        #[test]
        fn prop_sorted_after_sort(mut keys in proptest::collection::vec(0u64..1000, 0..20)) {
            keys.sort();
            keys.dedup();
            prop_assert!(is_sorted(&keys));
        }

        // r[verify btree-insert.find-position]
        #[test]
        fn prop_position_in_range(
            keys in proptest::collection::vec(0u64..1000, 0..20),
            key in 0u64..1000,
        ) {
            let mut sorted = keys;
            sorted.sort();
            sorted.dedup();
            let pos = find_insert_position(&sorted, key);
            prop_assert!(pos <= sorted.len());
        }

        // r[verify btree-insert.insert-at]
        #[test]
        fn prop_insert_preserves_sorted(
            keys in proptest::collection::vec(0u64..1000, 0..20),
            key in 0u64..1000,
        ) {
            let mut sorted = keys;
            sorted.sort();
            sorted.dedup();
            if !sorted.contains(&key) {
                let pos = find_insert_position(&sorted, key);
                let result = insert_at(&sorted, pos, key);
                prop_assert!(is_sorted(&result));
                prop_assert_eq!(result.len(), sorted.len() + 1);
            }
        }

        // r[verify btree-insert.split-node]
        #[test]
        fn prop_split_conserves(
            keys in proptest::collection::vec(0u64..1000, 1..20),
        ) {
            let mut sorted = keys;
            sorted.sort();
            sorted.dedup();
            if !sorted.is_empty() {
                let mid = crate::btree_balance::split_point(sorted.len() as u64) as usize;
                let (left, right) = split_node(&sorted, mid);
                prop_assert_eq!(left.len() + right.len(), sorted.len());
                prop_assert!(is_sorted(&left));
                prop_assert!(is_sorted(&right));
            }
        }

        // r[verify btree-insert.split-node]
        #[test]
        fn prop_split_left_lt_right(
            keys in proptest::collection::vec(0u64..1000, 2..20),
        ) {
            let mut sorted = keys;
            sorted.sort();
            sorted.dedup();
            if sorted.len() >= 2 {
                let mid = crate::btree_balance::split_point(sorted.len() as u64) as usize;
                if mid > 0 && mid < sorted.len() {
                    let (left, right) = split_node(&sorted, mid);
                    if let (Some(&max_l), Some(&min_r)) = (left.last(), right.first()) {
                        prop_assert!(max_l < min_r);
                    }
                }
            }
        }
    }
}
