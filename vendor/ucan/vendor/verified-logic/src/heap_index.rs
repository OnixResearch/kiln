//! Verified binary heap index arithmetic.
//!
//! Functions for navigating a binary heap stored as a flat array.
//! Parent-child index relationships, leaf detection, and roundtrip
//! proofs. All indices are zero-based — root is at index 0, left child
//! of node i is at 2i+1, right child at 2i+2, parent at (i-1)/2.

use vstd::prelude::*;

verus! {

// --- parent_index ---

// r[depends heap-index.parent]
/// Spec: parent of node at index i is at (i-1)/2.
pub open spec fn parent_index_spec(i: u64) -> u64 {
    ((i as int - 1) / 2) as u64
}

// r[impl heap-index.parent]
/// Returns the index of the parent node.
pub fn parent_index(i: u64) -> (result: u64)
    requires i > 0
    ensures result == parent_index_spec(i)
{
    (i - 1) / 2
}

// r[verify heap-index.parent]
/// Root's children have root as parent.
proof fn parent_of_one_is_zero()
    ensures parent_index_spec(1) == 0
{}

// r[verify heap-index.parent]
/// Parent is always strictly before the node.
proof fn parent_before_self(i: u64)
    requires i > 0
    ensures (parent_index_spec(i) as int) < (i as int)
{}

// --- left_child ---

// r[depends heap-index.left-child]
/// Spec: left child of node at index i is at 2*i+1.
pub open spec fn left_child_spec(i: u64) -> u64 {
    (2 * i as int + 1) as u64
}

// r[impl heap-index.left-child]
/// Returns the index of the left child.
pub fn left_child(i: u64) -> (result: u64)
    requires 2 * i as int + 1 <= u64::MAX as int
    ensures result == left_child_spec(i)
{
    2 * i + 1
}

// r[verify heap-index.left-child]
/// Left child of root is at index 1.
proof fn left_child_of_root()
    ensures left_child_spec(0) == 1
{}

// r[verify heap-index.left-child]
/// Left child is always strictly after the parent.
proof fn left_child_after_parent(i: u64)
    requires 2 * i as int + 1 <= u64::MAX as int
    ensures (left_child_spec(i) as int) > (i as int)
{}

// --- right_child ---

// r[depends heap-index.right-child]
/// Spec: right child of node at index i is at 2*i+2.
pub open spec fn right_child_spec(i: u64) -> u64 {
    (2 * i as int + 2) as u64
}

// r[impl heap-index.right-child]
/// Returns the index of the right child.
pub fn right_child(i: u64) -> (result: u64)
    requires 2 * i as int + 2 <= u64::MAX as int
    ensures result == right_child_spec(i)
{
    2 * i + 2
}

// r[verify heap-index.right-child]
/// Right child of root is at index 2.
proof fn right_child_of_root()
    ensures right_child_spec(0) == 2
{}

// r[verify heap-index.right-child]
/// Right child is always strictly after the parent.
proof fn right_child_after_parent(i: u64)
    requires 2 * i as int + 2 <= u64::MAX as int
    ensures (right_child_spec(i) as int) > (i as int)
{}

// --- is_leaf ---

// r[depends heap-index.is-leaf]
/// Spec: node is a leaf when its left child index is beyond the heap.
pub open spec fn is_leaf_spec(i: u64, size: u64) -> bool {
    2 * i as int + 1 >= size as int
}

// r[impl heap-index.is-leaf]
/// Returns true if node at index i has no children in a heap of given size.
pub fn is_leaf(i: u64, size: u64) -> (result: bool)
    ensures result == is_leaf_spec(i, size)
{
    i >= size / 2
}

// r[verify heap-index.is-leaf]
/// A leaf has no left child within the heap.
proof fn leaf_no_left_child(i: u64, size: u64)
    requires is_leaf_spec(i, size)
    ensures 2 * i as int + 1 >= size as int
{}

// r[verify heap-index.is-leaf]
/// A non-leaf in a valid heap has a left child within bounds.
proof fn non_leaf_has_left_child(i: u64, size: u64)
    requires !is_leaf_spec(i, size), i < size
    ensures 2 * i as int + 1 < size as int
{}

// r[verify heap-index.is-leaf]
/// Every node in an empty heap is a leaf.
proof fn empty_heap_all_leaves(i: u64)
    ensures is_leaf_spec(i, 0)
{}

// --- has_parent ---

// r[depends heap-index.has-parent]
/// Spec: every node except root has a parent.
pub open spec fn has_parent_spec(i: u64) -> bool {
    i > 0
}

// r[impl heap-index.has-parent]
/// Returns true if the node has a parent (is not root).
pub fn has_parent(i: u64) -> (result: bool)
    ensures result == has_parent_spec(i)
{
    i > 0
}

// r[verify heap-index.has-parent]
/// Root has no parent.
proof fn root_has_no_parent()
    ensures !has_parent_spec(0)
{}

// r[verify heap-index.has-parent]
/// All non-root nodes have a parent.
proof fn non_root_has_parent(i: u64)
    requires i > 0
    ensures has_parent_spec(i)
{}

// --- is_left_child ---

// r[depends heap-index.is-left-child]
/// Spec: left children occupy odd indices.
pub open spec fn is_left_child_spec(i: u64) -> bool {
    i > 0 && i % 2 == 1
}

// r[impl heap-index.is-left-child]
/// Returns true if index i is a left child (odd, non-root).
pub fn is_left_child(i: u64) -> (result: bool)
    ensures result == is_left_child_spec(i)
{
    i > 0 && i % 2 == 1
}

// r[verify heap-index.is-left-child]
/// Index 1 (left child of root) is a left child.
proof fn index_one_is_left()
    ensures is_left_child_spec(1)
{}

// --- is_right_child ---

// r[depends heap-index.is-right-child]
/// Spec: right children occupy even non-root indices.
pub open spec fn is_right_child_spec(i: u64) -> bool {
    i > 0 && i % 2 == 0
}

// r[impl heap-index.is-right-child]
/// Returns true if index i is a right child (even, non-root).
pub fn is_right_child(i: u64) -> (result: bool)
    ensures result == is_right_child_spec(i)
{
    i > 0 && i % 2 == 0
}

// r[verify heap-index.is-right-child]
/// Index 2 (right child of root) is a right child.
proof fn index_two_is_right()
    ensures is_right_child_spec(2)
{}

// --- Roundtrip proofs ---

// r[verify heap-index.parent]
// r[verify heap-index.left-child]
/// Parent of left child recovers the original index.
proof fn parent_left_roundtrip(i: u64)
    requires 2 * i as int + 1 <= u64::MAX as int
    ensures parent_index_spec(left_child_spec(i)) == i
{
    assert(((2 * i as int + 1 - 1) / 2) == i as int) by(nonlinear_arith);
}

// r[verify heap-index.parent]
// r[verify heap-index.right-child]
/// Parent of right child recovers the original index.
proof fn parent_right_roundtrip(i: u64)
    requires 2 * i as int + 2 <= u64::MAX as int
    ensures parent_index_spec(right_child_spec(i)) == i
{
    assert(((2 * i as int + 2 - 1) / 2) == i as int) by(nonlinear_arith);
}

// --- Children ordering ---

// r[verify heap-index.left-child]
// r[verify heap-index.right-child]
/// Left child index is strictly less than right child index.
proof fn left_before_right(i: u64)
    requires 2 * i as int + 2 <= u64::MAX as int
    ensures (left_child_spec(i) as int) < (right_child_spec(i) as int)
{}

// r[verify heap-index.left-child]
// r[verify heap-index.right-child]
/// Left and right children are distinct indices.
proof fn children_distinct(i: u64)
    requires 2 * i as int + 2 <= u64::MAX as int
    ensures left_child_spec(i) != right_child_spec(i)
{}

// --- Classification proofs ---

// r[verify heap-index.is-left-child]
// r[verify heap-index.left-child]
/// The left child of any node is classified as a left child.
proof fn left_child_is_left(i: u64)
    requires 2 * i as int + 1 <= u64::MAX as int
    ensures is_left_child_spec(left_child_spec(i))
{
    assert((2 * i as int + 1) % 2 == 1) by(nonlinear_arith);
}

// r[verify heap-index.is-right-child]
// r[verify heap-index.right-child]
/// The right child of any node is classified as a right child.
proof fn right_child_is_right(i: u64)
    requires 2 * i as int + 2 <= u64::MAX as int
    ensures is_right_child_spec(right_child_spec(i))
{
    assert((2 * i as int + 2) % 2 == 0) by(nonlinear_arith);
}

// r[verify heap-index.is-left-child]
// r[verify heap-index.is-right-child]
/// Every non-root node is exactly one of left child or right child.
proof fn non_root_classification(i: u64)
    requires i > 0
    ensures
        is_left_child_spec(i) != is_right_child_spec(i),
        is_left_child_spec(i) || is_right_child_spec(i),
{}

// r[verify heap-index.is-left-child]
// r[verify heap-index.is-right-child]
/// Root is neither a left child nor a right child.
proof fn root_is_neither()
    ensures !is_left_child_spec(0), !is_right_child_spec(0)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify heap-index.parent]
    #[test]
    fn test_parent_index() {
        assert_eq!(parent_index(1), 0);
        assert_eq!(parent_index(2), 0);
        assert_eq!(parent_index(3), 1);
        assert_eq!(parent_index(4), 1);
        assert_eq!(parent_index(5), 2);
        assert_eq!(parent_index(6), 2);
    }

    // r[verify heap-index.left-child]
    #[test]
    fn test_left_child() {
        assert_eq!(left_child(0), 1);
        assert_eq!(left_child(1), 3);
        assert_eq!(left_child(2), 5);
        assert_eq!(left_child(3), 7);
    }

    // r[verify heap-index.right-child]
    #[test]
    fn test_right_child() {
        assert_eq!(right_child(0), 2);
        assert_eq!(right_child(1), 4);
        assert_eq!(right_child(2), 6);
        assert_eq!(right_child(3), 8);
    }

    // r[verify heap-index.is-leaf]
    #[test]
    fn test_is_leaf() {
        // Heap of size 7 (complete 3-level tree):
        //       0
        //      / \
        //     1   2
        //    / \ / \
        //   3  4 5  6
        assert!(!is_leaf(0, 7));
        assert!(!is_leaf(1, 7));
        assert!(!is_leaf(2, 7));
        assert!(is_leaf(3, 7));
        assert!(is_leaf(4, 7));
        assert!(is_leaf(5, 7));
        assert!(is_leaf(6, 7));
        // Empty heap: everything is a leaf
        assert!(is_leaf(0, 0));
        // Single-element heap: root is a leaf
        assert!(is_leaf(0, 1));
    }

    // r[verify heap-index.has-parent]
    #[test]
    fn test_has_parent() {
        assert!(!has_parent(0));
        assert!(has_parent(1));
        assert!(has_parent(2));
        assert!(has_parent(u64::MAX));
    }

    // r[verify heap-index.is-left-child]
    #[test]
    fn test_is_left_child() {
        assert!(!is_left_child(0));
        assert!(is_left_child(1));
        assert!(!is_left_child(2));
        assert!(is_left_child(3));
        assert!(!is_left_child(4));
        assert!(is_left_child(5));
    }

    // r[verify heap-index.is-right-child]
    #[test]
    fn test_is_right_child() {
        assert!(!is_right_child(0));
        assert!(!is_right_child(1));
        assert!(is_right_child(2));
        assert!(!is_right_child(3));
        assert!(is_right_child(4));
        assert!(!is_right_child(5));
    }

    proptest! {
        // r[verify heap-index.parent]
        #[test]
        fn prop_parent_before_self(i in 1u64..=u64::MAX) {
            prop_assert!(parent_index(i) < i);
        }

        // r[verify heap-index.parent]
        // r[verify heap-index.left-child]
        #[test]
        fn prop_parent_left_roundtrip(i in 0u64..=(u64::MAX / 2 - 1)) {
            let lc = left_child(i);
            prop_assert_eq!(parent_index(lc), i);
        }

        // r[verify heap-index.parent]
        // r[verify heap-index.right-child]
        #[test]
        fn prop_parent_right_roundtrip(i in 0u64..=(u64::MAX / 2 - 1)) {
            let rc = right_child(i);
            prop_assert_eq!(parent_index(rc), i);
        }

        // r[verify heap-index.left-child]
        // r[verify heap-index.right-child]
        #[test]
        fn prop_left_before_right(i in 0u64..=(u64::MAX / 2 - 1)) {
            prop_assert!(left_child(i) < right_child(i));
        }

        // r[verify heap-index.left-child]
        #[test]
        fn prop_left_child_after_parent(i in 0u64..=(u64::MAX / 2 - 1)) {
            prop_assert!(left_child(i) > i);
        }

        // r[verify heap-index.right-child]
        #[test]
        fn prop_right_child_after_parent(i in 0u64..=(u64::MAX / 2 - 1)) {
            prop_assert!(right_child(i) > i);
        }

        // r[verify heap-index.is-leaf]
        #[test]
        fn prop_leaf_no_children(i: u64, size: u64) {
            if is_leaf(i, size) {
                // 2*i + 1 >= size means no left child in bounds
                let lc = 2u128 * i as u128 + 1;
                prop_assert!(lc >= size as u128);
            }
        }

        // r[verify heap-index.is-left-child]
        // r[verify heap-index.is-right-child]
        #[test]
        fn prop_non_root_is_left_xor_right(i in 1u64..=u64::MAX) {
            prop_assert!(is_left_child(i) != is_right_child(i));
            prop_assert!(is_left_child(i) || is_right_child(i));
        }

        // r[verify heap-index.is-left-child]
        // r[verify heap-index.left-child]
        #[test]
        fn prop_left_child_classified_left(i in 0u64..=(u64::MAX / 2 - 1)) {
            prop_assert!(is_left_child(left_child(i)));
        }

        // r[verify heap-index.is-right-child]
        // r[verify heap-index.right-child]
        #[test]
        fn prop_right_child_classified_right(i in 0u64..=(u64::MAX / 2 - 1)) {
            prop_assert!(is_right_child(right_child(i)));
        }

        // r[verify heap-index.is-leaf]
        #[test]
        fn prop_non_leaf_has_left_in_bounds(
            size in 2u64..10000u64,
            i: u64,
        ) {
            if i < size && !is_leaf(i, size) {
                let lc = 2u128 * i as u128 + 1;
                prop_assert!(lc < size as u128);
            }
        }
    }
}
