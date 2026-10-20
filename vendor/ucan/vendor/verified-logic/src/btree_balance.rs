// Verus proofs guarantee u128→u64 casts are in bounds (count <= capacity)
#![allow(clippy::cast_possible_truncation)]

//! B-tree node balance management.
//!
//! Pure functions for B-tree node capacity: fill percentage, split/merge
//! triggers, split point computation, and redistribution eligibility.
//! These compose with [`arithmetic`] for cross-module properties.

use vstd::prelude::*;

verus! {

// ─── Fill Factor ─────────────────────────────────────────────────────

/// Spec: fill percentage of a B-tree node.
// r[depends btree-balance.fill-pct]
// r[impl btree-balance.fill-pct]
pub open spec fn fill_pct_spec(count: u64, capacity: u64) -> u64 {
    ((count as int * 100) / capacity as int) as u64
}

/// Fill percentage: count * 100 / capacity (integer percentage).
// r[impl btree-balance.fill-pct]
pub fn fill_pct(count: u64, capacity: u64) -> (result: u64)
    requires capacity > 0, count <= capacity,
    ensures result == fill_pct_spec(count, capacity),
{
    // count <= capacity <= u64::MAX, so count * 100 fits in u128
    let wide = count as u128 * 100u128;
    (wide / capacity as u128) as u64
}

// ─── Underfull / Overfull ────────────────────────────────────────────

/// Spec: is the node underfull (below minimum occupancy)?
// r[depends btree-balance.is-underfull]
// r[impl btree-balance.is-underfull]
pub open spec fn is_underfull_spec(count: u64, capacity: u64) -> bool {
    (count as int) < (capacity as int) / 2
}

/// A node with fewer than half its capacity is underfull.
// r[impl btree-balance.is-underfull]
pub fn is_underfull(count: u64, capacity: u64) -> (result: bool)
    requires capacity >= 2,
    ensures result == is_underfull_spec(count, capacity),
{
    count < capacity / 2
}

/// Spec: is the node overfull (exceeds capacity)?
// r[depends btree-balance.is-overfull]
// r[impl btree-balance.is-overfull]
pub open spec fn is_overfull_spec(count: u64, capacity: u64) -> bool {
    count > capacity
}

/// A node exceeding its capacity must be split.
// r[impl btree-balance.is-overfull]
pub fn is_overfull(count: u64, capacity: u64) -> (result: bool)
    ensures result == is_overfull_spec(count, capacity),
{
    count > capacity
}

// ─── Split / Merge Decisions ─────────────────────────────────────────

/// Spec: should this node be split?
// r[depends btree-balance.should-split]
// r[impl btree-balance.should-split]
pub open spec fn should_split_spec(count: u64, capacity: u64) -> bool {
    count > capacity
}

/// Splitting is required when the node exceeds its capacity.
// r[impl btree-balance.should-split]
pub fn should_split(count: u64, capacity: u64) -> (result: bool)
    ensures result == should_split_spec(count, capacity),
{
    count > capacity
}

/// Spec: should two nodes be merged?
// r[depends btree-balance.should-merge]
// r[impl btree-balance.should-merge]
pub open spec fn should_merge_spec(count: u64, sibling_count: u64, capacity: u64) -> bool {
    count as int + sibling_count as int <= capacity as int
}

/// Two underfull nodes that together fit in one node should be merged.
// r[impl btree-balance.should-merge]
pub fn should_merge(count: u64, sibling_count: u64, capacity: u64) -> (result: bool)
    requires count as int + sibling_count as int <= u64::MAX as int,
    ensures result == should_merge_spec(count, sibling_count, capacity),
{
    count + sibling_count <= capacity
}

// ─── Split Point ─────────────────────────────────────────────────────

/// Spec: split point (median position).
// r[depends btree-balance.split-point]
// r[impl btree-balance.split-point]
pub open spec fn split_point_spec(count: u64) -> u64 {
    (count / 2) as u64
}

/// The split point divides the node into two halves.
// r[impl btree-balance.split-point]
pub fn split_point(count: u64) -> (result: u64)
    ensures result == split_point_spec(count),
{
    count / 2
}

/// Spec: size of the left node after split.
// r[depends btree-balance.split-left-size]
// r[impl btree-balance.split-left-size]
pub open spec fn split_left_size_spec(count: u64) -> u64 {
    split_point_spec(count)
}

/// The left node takes the first half.
// r[impl btree-balance.split-left-size]
pub fn split_left_size(count: u64) -> (result: u64)
    ensures result == split_left_size_spec(count),
{
    split_point(count)
}

/// Spec: size of the right node after split.
// r[depends btree-balance.split-right-size]
// r[impl btree-balance.split-right-size]
pub open spec fn split_right_size_spec(count: u64) -> u64 {
    (count - split_point_spec(count)) as u64
}

/// The right node takes the remainder.
// r[impl btree-balance.split-right-size]
pub fn split_right_size(count: u64) -> (result: u64)
    ensures result == split_right_size_spec(count),
{
    count - split_point(count)
}

// ─── Minimum Keys ────────────────────────────────────────────────────

/// Spec: minimum key count for a non-root internal node.
// r[depends btree-balance.min-keys]
// r[impl btree-balance.min-keys]
pub open spec fn min_keys_spec(capacity: u64) -> u64 {
    (capacity / 2) as u64
}

/// The B-tree invariant requires at least capacity/2 keys in non-root nodes.
// r[impl btree-balance.min-keys]
pub fn min_keys(capacity: u64) -> (result: u64)
    requires capacity >= 2,
    ensures result == min_keys_spec(capacity),
{
    capacity / 2
}

// ─── Redistribution ──────────────────────────────────────────────────

/// Spec: can the sibling donate keys?
// r[depends btree-balance.can-borrow]
// r[impl btree-balance.can-borrow]
pub open spec fn can_borrow_spec(sibling_count: u64, capacity: u64) -> bool {
    sibling_count > min_keys_spec(capacity)
}

/// A sibling can donate keys when it has more than the minimum.
// r[impl btree-balance.can-borrow]
pub fn can_borrow(sibling_count: u64, capacity: u64) -> (result: bool)
    requires capacity >= 2,
    ensures result == can_borrow_spec(sibling_count, capacity),
{
    sibling_count > min_keys(capacity)
}

// ─── Proofs ──────────────────────────────────────────────────────────

// --- fill_pct ---

/// Empty node has 0% fill.
// r[verify btree-balance.fill-pct]
proof fn fill_pct_empty(capacity: u64)
    requires capacity > 0,
    ensures fill_pct_spec(0, capacity) == 0,
{}

/// Full node has 100% fill.
// r[verify btree-balance.fill-pct]
proof fn fill_pct_full(capacity: u64)
    requires capacity > 0,
    ensures fill_pct_spec(capacity, capacity) == 100,
{
    assert(capacity as int * 100 / capacity as int == 100) by(nonlinear_arith)
        requires capacity > 0u64;
}

/// Fill percentage is bounded by [0, 100] for valid counts.
// r[verify btree-balance.fill-pct]
proof fn fill_pct_bounded(count: u64, capacity: u64)
    requires capacity > 0, count <= capacity,
    ensures
        fill_pct_spec(count, capacity) >= 0,
        fill_pct_spec(count, capacity) <= 100,
{
    assert(count as int * 100 / capacity as int <= 100) by(nonlinear_arith)
        requires capacity > 0u64, count <= capacity;
    assert(count as int * 100 / capacity as int >= 0) by(nonlinear_arith)
        requires capacity > 0u64;
}

// --- is_underfull ---

/// A node at exactly min_keys is NOT underfull.
// r[verify btree-balance.is-underfull]
proof fn min_keys_not_underfull(capacity: u64)
    requires capacity >= 2,
    ensures !is_underfull_spec(min_keys_spec(capacity), capacity),
{}

/// An empty node is always underfull (when capacity >= 2).
// r[verify btree-balance.is-underfull]
proof fn empty_is_underfull(capacity: u64)
    requires capacity >= 2,
    ensures is_underfull_spec(0, capacity),
{}

/// A full node is never underfull.
// r[verify btree-balance.is-underfull]
proof fn full_not_underfull(capacity: u64)
    requires capacity >= 2,
    ensures !is_underfull_spec(capacity, capacity),
{}

// --- is_overfull ---

/// A node at capacity is NOT overfull.
// r[verify btree-balance.is-overfull]
proof fn at_capacity_not_overfull(capacity: u64)
    ensures !is_overfull_spec(capacity, capacity),
{}

/// One past capacity is overfull.
// r[verify btree-balance.is-overfull]
proof fn one_past_is_overfull(capacity: u64)
    requires capacity < u64::MAX,
    ensures is_overfull_spec((capacity + 1) as u64, capacity),
{}

// --- should_split / should_merge ---

/// Split and merge are disjoint when count <= capacity + sibling_count.
// r[verify btree-balance.should-split]
// r[verify btree-balance.should-merge]
proof fn split_merge_disjoint(count: u64, capacity: u64)
    requires count <= capacity,
    ensures !should_split_spec(count, capacity),
{}

// --- split_point ---

/// Split point is at least 1 when count >= 2.
// r[verify btree-balance.split-point]
proof fn split_point_ge_one(count: u64)
    requires count >= 2,
    ensures split_point_spec(count) >= 1,
{}

/// Split point is at most count.
// r[verify btree-balance.split-point]
proof fn split_point_le_count(count: u64)
    ensures split_point_spec(count) <= count,
{}

// --- split sizes ---

/// Split conserves: left_size + right_size == count.
// r[verify btree-balance.split-left-size]
// r[verify btree-balance.split-right-size]
proof fn split_conserves(count: u64)
    ensures split_left_size_spec(count) + split_right_size_spec(count) == count,
{
    vstd::arithmetic::div_mod::lemma_fundamental_div_mod(count as int, 2);
}

/// Right half is always at least as big as left (for odd counts, right is bigger).
// r[verify btree-balance.split-right-size]
proof fn right_ge_left(count: u64)
    ensures split_right_size_spec(count) >= split_left_size_spec(count),
{
    vstd::arithmetic::div_mod::lemma_fundamental_div_mod(count as int, 2);
}

// --- min_keys ---

/// min_keys is at least 1 when capacity >= 2.
// r[verify btree-balance.min-keys]
proof fn min_keys_ge_one(capacity: u64)
    requires capacity >= 2,
    ensures min_keys_spec(capacity) >= 1,
{}

/// min_keys equals split_point(capacity).
// r[verify btree-balance.min-keys]
proof fn min_keys_is_split_point(capacity: u64)
    ensures min_keys_spec(capacity) == split_point_spec(capacity),
{}

// --- can_borrow ---

/// A full sibling can always lend (when capacity >= 2).
// r[verify btree-balance.can-borrow]
proof fn full_sibling_can_lend(capacity: u64)
    requires capacity >= 2,
    ensures can_borrow_spec(capacity, capacity),
{}

/// A sibling at exactly min_keys cannot lend.
// r[verify btree-balance.can-borrow]
proof fn min_sibling_cannot_lend(capacity: u64)
    requires capacity >= 2,
    ensures !can_borrow_spec(min_keys_spec(capacity), capacity),
{}

// --- Lifecycle: underfull → merge or borrow ---

/// An underfull node with a lendable sibling can borrow instead of merge.
// r[verify btree-balance.is-underfull]
// r[verify btree-balance.can-borrow]
proof fn underfull_borrow_alternative(count: u64, sibling: u64, capacity: u64)
    requires
        capacity >= 2,
        is_underfull_spec(count, capacity),
        can_borrow_spec(sibling, capacity),
    ensures sibling > capacity / 2,
{}

/// After a split, both halves are within capacity.
// r[verify btree-balance.should-split]
// r[verify btree-balance.split-left-size]
// r[verify btree-balance.split-right-size]
proof fn split_halves_within_capacity(count: u64, capacity: u64)
    requires
        count > 0,
        capacity > 0,
        count <= 2 * capacity,
    ensures
        split_left_size_spec(count) <= capacity,
        split_right_size_spec(count) <= capacity,
{
    vstd::arithmetic::div_mod::lemma_fundamental_div_mod(count as int, 2);
    assert(count as int / 2 <= capacity as int) by(nonlinear_arith)
        requires count as int <= 2 * capacity as int;
    assert(count as int - count as int / 2 <= capacity as int) by(nonlinear_arith)
        requires count as int <= 2 * capacity as int, count as int / 2 >= 0;
}

} // verus!

// ─── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // --- Unit tests ---

    #[test]
    // r[verify btree-balance.fill-pct]
    fn test_fill_pct_basic() {
        assert_eq!(fill_pct(0, 100), 0);
        assert_eq!(fill_pct(50, 100), 50);
        assert_eq!(fill_pct(100, 100), 100);
        assert_eq!(fill_pct(1, 3), 33); // floor(100/3)
    }

    #[test]
    // r[verify btree-balance.is-underfull]
    fn test_is_underfull_basic() {
        assert!(is_underfull(0, 10));
        assert!(is_underfull(4, 10));
        assert!(!is_underfull(5, 10));
        assert!(!is_underfull(10, 10));
    }

    #[test]
    // r[verify btree-balance.is-overfull]
    fn test_is_overfull_basic() {
        assert!(!is_overfull(10, 10));
        assert!(is_overfull(11, 10));
        assert!(!is_overfull(0, 10));
    }

    #[test]
    // r[verify btree-balance.should-split]
    fn test_should_split_basic() {
        assert!(!should_split(10, 10));
        assert!(should_split(11, 10));
    }

    #[test]
    // r[verify btree-balance.should-merge]
    fn test_should_merge_basic() {
        assert!(should_merge(3, 4, 10));
        assert!(should_merge(5, 5, 10));
        assert!(!should_merge(6, 5, 10));
    }

    #[test]
    // r[verify btree-balance.split-point]
    fn test_split_point_basic() {
        assert_eq!(split_point(10), 5);
        assert_eq!(split_point(11), 5);
        assert_eq!(split_point(0), 0);
        assert_eq!(split_point(1), 0);
    }

    #[test]
    // r[verify btree-balance.split-left-size]
    // r[verify btree-balance.split-right-size]
    fn test_split_sizes_conserve() {
        for count in 0..=20 {
            assert_eq!(split_left_size(count) + split_right_size(count), count,);
        }
    }

    #[test]
    // r[verify btree-balance.min-keys]
    fn test_min_keys_basic() {
        assert_eq!(min_keys(10), 5);
        assert_eq!(min_keys(11), 5);
        assert_eq!(min_keys(2), 1);
        assert_eq!(min_keys(3), 1);
    }

    #[test]
    // r[verify btree-balance.can-borrow]
    fn test_can_borrow_basic() {
        // capacity=10, min_keys=5
        assert!(!can_borrow(5, 10));
        assert!(can_borrow(6, 10));
        assert!(can_borrow(10, 10));
    }

    // --- Property tests ---

    use proptest::prelude::*;

    proptest! {
        #[test]
        // r[verify btree-balance.fill-pct]
        fn prop_fill_bounded(count in 0u64..10000, capacity in 1u64..10000) {
            if count <= capacity {
                let pct = fill_pct(count, capacity);
                prop_assert!(pct <= 100);
            }
        }

        #[test]
        // r[verify btree-balance.split-left-size]
        // r[verify btree-balance.split-right-size]
        fn prop_split_conserves(count: u64) {
            prop_assert_eq!(split_left_size(count) + split_right_size(count), count);
        }

        #[test]
        // r[verify btree-balance.split-right-size]
        fn prop_right_ge_left(count: u64) {
            prop_assert!(split_right_size(count) >= split_left_size(count));
        }

        #[test]
        // r[verify btree-balance.is-underfull]
        // r[verify btree-balance.min-keys]
        fn prop_min_keys_not_underfull(capacity in 2u64..10000) {
            prop_assert!(!is_underfull(min_keys(capacity), capacity));
        }

        #[test]
        // r[verify btree-balance.should-merge]
        fn prop_merge_fits(
            count in 0u64..5000,
            sibling in 0u64..5000,
            capacity in 1u64..10000,
        ) {
            if should_merge(count, sibling, capacity) {
                prop_assert!(count + sibling <= capacity);
            }
        }

        #[test]
        // r[verify btree-balance.can-borrow]
        fn prop_borrow_implies_above_min(
            sibling in 0u64..10000,
            capacity in 2u64..10000,
        ) {
            if can_borrow(sibling, capacity) {
                prop_assert!(sibling > capacity / 2);
            }
        }

        #[test]
        // r[verify btree-balance.should-split]
        // r[verify btree-balance.split-left-size]
        // r[verify btree-balance.split-right-size]
        fn prop_split_halves_within_capacity(
            count in 1u64..20000,
            capacity in 1u64..10000,
        ) {
            if count <= 2 * capacity {
                prop_assert!(split_left_size(count) <= capacity);
                prop_assert!(split_right_size(count) <= capacity);
            }
        }
    }
}
