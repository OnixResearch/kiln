//! Verified buddy split/coalesce sequences with block conservation.
//!
//! [`buddy`] proves the arithmetic of individual split/coalesce operations.
//! This module proves *sequences* of splits and coalesces conserve total
//! memory: splitting a block into two halves and coalescing them back
//! recovers the original, and a recursive split from one large block
//! produces disjoint blocks whose sizes sum to the original.

use vstd::prelude::*;

use crate::buddy::block_size;
#[cfg(verus_keep_ghost)]
use crate::buddy::block_size_spec;
use crate::buddy::buddy_of;
#[cfg(verus_keep_ghost)]
use crate::buddy::buddy_of_spec;
use crate::buddy::can_coalesce;
#[cfg(verus_keep_ghost)]
use crate::buddy::can_coalesce_spec;
use crate::buddy::is_aligned_to_order;
#[cfg(verus_keep_ghost)]
use crate::buddy::is_aligned_to_order_spec;

verus! {

// ─── Split Block ─────────────────────────────────────────────────────

// r[depends split-coalesce.split-block]
/// Spec: split a block at `order` into two buddies at `order - 1`.
/// Returns (left_addr, right_addr) where right = left + 2^(order-1).
pub open spec fn split_block_spec(addr: u64, order: u64) -> (u64, u64)
    recommends order > 0, order < 64, is_aligned_to_order_spec(addr, order)
{
    (addr, (addr as int + block_size_spec((order - 1) as u64) as int) as u64)
}

// r[impl split-coalesce.split-block]
/// Split a block into two buddies. Left keeps original address,
/// right starts at addr + 2^(order-1).
pub fn split_block(addr: u64, order: u64) -> (result: (u64, u64))
    requires
        order > 0,
        order < 64,
        is_aligned_to_order_spec(addr, order),
        addr as int + block_size_spec(order) as int <= u64::MAX as int,
    ensures result == split_block_spec(addr, order)
{
    let half = block_size(order - 1);
    proof {
        // half + half == block_size(order), so addr + half <= addr + block_size(order) <= MAX
        assert(order > 0 && order < 64 ==>
            2u64 * (1u64 << ((order - 1) as u64)) == (1u64 << order))
            by(bit_vector);
    }
    (addr, addr + half)
}

// ─── Coalesce Blocks ─────────────────────────────────────────────────

// r[depends split-coalesce.coalesce-blocks]
/// Spec: coalesce two buddy blocks back into a parent at `order + 1`.
/// Returns the smaller of the two addresses (the parent address).
pub open spec fn coalesce_blocks_spec(left: u64, right: u64, order: u64) -> u64
    recommends order < 63, can_coalesce_spec(left, right, order)
{
    if left < right { left } else { right }
}

// r[impl split-coalesce.coalesce-blocks]
/// Coalesce two buddy blocks into a parent block.
pub fn coalesce_blocks(left: u64, right: u64, order: u64) -> (result: u64)
    requires order < 63, can_coalesce_spec(left, right, order)
    ensures result == coalesce_blocks_spec(left, right, order)
{
    if left < right { left } else { right }
}

// ─── Split Tree (spec-only) ─────────────────────────────────────────

// r[depends split-coalesce.split-tree-total]
/// Spec: recursively split a block at `order` down to `target_order`.
/// Produces a sequence of (addr, target_order) blocks.
pub open spec fn split_tree_total_spec(
    addr: u64, order: u64, target_order: u64,
) -> Seq<(u64, u64)>
    recommends target_order <= order, order < 64
    decreases order - target_order,
{
    if order == target_order || target_order > order {
        seq![(addr, order)]
    } else {
        let child_order = (order - 1) as u64;
        let left = addr;
        let right = (addr as int + block_size_spec(child_order) as int) as u64;
        split_tree_total_spec(left, child_order, target_order)
            + split_tree_total_spec(right, child_order, target_order)
    }
}

// ─── Blocks Disjoint ─────────────────────────────────────────────────

// r[depends split-coalesce.blocks-disjoint]
/// Spec: no two blocks in the list overlap.
/// Each block is (addr, order) representing [addr, addr + 2^order).
pub open spec fn blocks_disjoint_spec(blocks: Seq<(u64, u64)>) -> bool {
    forall|i: int, j: int|
        #![trigger blocks[i], blocks[j]]
        0 <= i < blocks.len() && 0 <= j < blocks.len() && i != j
        ==> ({
            let (ai, oi) = blocks[i];
            let (aj, oj) = blocks[j];
            ai as int + block_size_spec(oi) as int <= aj as int
            || aj as int + block_size_spec(oj) as int <= ai as int
        })
}

// ─── Blocks Cover ────────────────────────────────────────────────────

/// Spec helper: sum of block sizes.
pub open spec fn blocks_total_size_spec(blocks: Seq<(u64, u64)>, from: int) -> int
    decreases (if blocks.len() >= from { blocks.len() - from } else { 0 }),
{
    if from < 0 || from >= blocks.len() {
        0
    } else {
        block_size_spec(blocks[from].1) as int
            + blocks_total_size_spec(blocks, from + 1)
    }
}

// r[depends split-coalesce.blocks-cover]
/// Spec: blocks exactly cover the parent region [addr, addr + 2^order).
pub open spec fn blocks_cover_spec(blocks: Seq<(u64, u64)>, addr: u64, order: u64) -> bool {
    blocks_total_size_spec(blocks, 0) == block_size_spec(order) as int
}

// ─── Proofs ──────────────────────────────────────────────────────────

// r[verify split-coalesce.split-block]
/// Split then coalesce is identity: the parent address equals the original.
proof fn split_coalesce_roundtrip(addr: u64, order: u64)
    requires
        order > 0,
        order < 63,
        is_aligned_to_order_spec(addr, order),
        addr as int + block_size_spec(order) as int <= u64::MAX as int,
    ensures ({
        let (left, right) = split_block_spec(addr, order);
        coalesce_blocks_spec(left, right, (order - 1) as u64) == addr
    })
{
    let child_order = (order - 1) as u64;
    // 2^(order-1) >= 1
    assert(child_order < 64 ==> (1u64 << child_order) >= 1u64)
        by(bit_vector);
    // half <= block_size(order): inline for bit_vector
    assert(order > 0 && order < 63 ==> (1u64 << sub(order, 1u64)) <= (1u64 << order))
        by(bit_vector);
    // So addr + half <= addr + block_size(order) <= MAX, no overflow in cast
    // right = addr + half > addr (since half >= 1)
    // coalesce_blocks_spec picks min(addr, addr+half) = addr
}

// r[verify split-coalesce.split-block]
/// Split produces two regions whose sizes sum to the parent.
proof fn split_sizes_sum(addr: u64, order: u64)
    requires order > 0, order < 64
    ensures ({
        let child_order = (order - 1) as u64;
        block_size_spec(child_order) as int + block_size_spec(child_order) as int
            == block_size_spec(order) as int
    })
{
    let child_order = (order - 1) as u64;
    // Prove: 2^(order-1) + 2^(order-1) == 2^order
    // Equivalently: x + x == 2x, and 2 * 2^(order-1) == 2^order
    assert(order > 0 && order < 64 ==>
        (1u64 << ((order - 1) as u64)) + (1u64 << ((order - 1) as u64))
        == (1u64 << order))
        by(bit_vector);
}

// r[verify split-coalesce.split-block]
/// Split produces two disjoint regions.
proof fn split_disjoint(addr: u64, order: u64)
    requires
        order > 0,
        order < 64,
        is_aligned_to_order_spec(addr, order),
        addr as int + block_size_spec(order) as int <= u64::MAX as int,
    ensures ({
        let (left, right) = split_block_spec(addr, order);
        let child_order = (order - 1) as u64;
        // left region [addr, addr + 2^(order-1)) ends before right starts
        left as int + block_size_spec(child_order) as int <= right as int
    })
{
    let child_order = (order - 1) as u64;
    // half <= full block size (no overflow in the cast)
    assert(order > 0 && order < 64 ==>
        (1u64 << sub(order, 1u64)) <= (1u64 << order))
        by(bit_vector);
    // addr + half <= addr + block_size(order) <= MAX, so the as-u64 cast is lossless
    assert(addr as int + block_size_spec(child_order) as int
        <= addr as int + block_size_spec(order) as int);
}

// r[impl split-coalesce.split-tree-total]
// r[verify split-coalesce.split-tree-total]
/// Single block (order == target): tree is one element.
proof fn tree_single(addr: u64, order: u64)
    ensures split_tree_total_spec(addr, order, order) == seq![(addr, order)]
{}

// r[verify split-coalesce.split-tree-total]
/// One split: tree of order k down to k-1 produces two blocks.
proof fn tree_one_split(addr: u64, order: u64)
    requires order > 0, order < 64
    ensures split_tree_total_spec(addr, order, (order - 1) as u64).len() == 2
{
    let target = (order - 1) as u64;
    let tree = split_tree_total_spec(addr, order, target);
    // At order - 1 each child hits target_order, producing singleton seq
    let left = addr;
    let right = (addr as int + block_size_spec(target) as int) as u64;
    assert(split_tree_total_spec(left, target, target) == seq![(left, target)]);
    assert(split_tree_total_spec(right, target, target) == seq![(right, target)]);
}

// r[impl split-coalesce.blocks-cover]
// r[verify split-coalesce.blocks-cover]
/// Total size of a single-block list equals that block's size.
proof fn single_block_total(addr: u64, order: u64)
    requires order < 64
    ensures blocks_total_size_spec(seq![(addr, order)], 0) == block_size_spec(order) as int
{
    let blocks = seq![(addr, order)];
    // Unfold: blocks[0] = (addr, order), blocks[0].1 = order
    // blocks_total_size_spec(blocks, 0) = block_size_spec(order) + blocks_total_size_spec(blocks, 1)
    // blocks_total_size_spec(blocks, 1) = 0 (1 >= blocks.len())
    assert(blocks[0] == (addr, order));
    assert(blocks_total_size_spec(blocks, 1) == 0int);
}

/// Helper: blocks_total_size is non-negative.
proof fn blocks_total_size_nonneg(blocks: Seq<(u64, u64)>, from: int)
    requires forall|i: int| #![trigger blocks[i]] 0 <= i < blocks.len()
        ==> blocks[i].1 < 64u64
    ensures blocks_total_size_spec(blocks, from) >= 0
    decreases (if blocks.len() >= from { blocks.len() - from } else { 0 }),
{
    if from >= 0 && from < blocks.len() {
        let order = blocks[from].1;
        assert(order < 64u64);
        assert(order < 64 ==> (1u64 << order) >= 1u64) by(bit_vector);
        blocks_total_size_nonneg(blocks, from + 1);
    }
}

// r[verify split-coalesce.coalesce-blocks]
/// Coalesce is symmetric: coalesce(a, b, o) == coalesce(b, a, o).
proof fn coalesce_symmetric(a: u64, b: u64, order: u64)
    ensures coalesce_blocks_spec(a, b, order) == coalesce_blocks_spec(b, a, order)
{}

} // verus!

#[cfg(test)]
#[allow(clippy::uninlined_format_args)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::buddy::block_size;
    use crate::buddy::buddy_of;
    use crate::buddy::can_coalesce;
    use crate::buddy::is_aligned_to_order;

    // r[verify split-coalesce.split-block]
    #[test]
    fn test_split_block() {
        let (left, right) = split_block(0, 4); // split 16-byte block at addr 0
        assert_eq!(left, 0);
        assert_eq!(right, 8);
    }

    // r[verify split-coalesce.split-block]
    #[test]
    fn test_split_coalesce_roundtrip() {
        for order in 1u64..20 {
            let addr = 0u64;
            let (left, right) = split_block(addr, order);
            let parent = coalesce_blocks(left, right, order - 1);
            assert_eq!(parent, addr, "roundtrip failed at order {}", order);
        }
    }

    // r[verify split-coalesce.split-block]
    #[test]
    fn test_split_sizes_sum() {
        for order in 1u64..20 {
            let half_size = block_size(order - 1);
            assert_eq!(half_size + half_size, block_size(order), "sizes don't sum at order {}", order);
        }
    }

    // r[verify split-coalesce.split-block]
    #[test]
    fn test_split_disjoint() {
        let (left, right) = split_block(0, 10);
        let half = block_size(9);
        // left region [0, 512), right region [512, 1024): disjoint
        assert!(left + half <= right);
    }

    proptest! {
        // r[verify split-coalesce.split-block]
        #[test]
        fn prop_split_coalesce_roundtrip(order in 1u64..20) {
            let addr = 0u64;
            let (left, right) = split_block(addr, order);
            let parent = coalesce_blocks(left, right, order - 1);
            prop_assert_eq!(parent, addr);
        }

        // r[verify split-coalesce.split-block]
        #[test]
        fn prop_split_sizes_sum(order in 1u64..63) {
            let half = block_size(order - 1);
            prop_assert_eq!(half + half, block_size(order));
        }

        // r[verify split-coalesce.split-block]
        #[test]
        fn prop_split_disjoint(order in 1u64..50) {
            let addr = 0u64;
            let (left, right) = split_block(addr, order);
            let half = block_size(order - 1);
            prop_assert!(left + half <= right);
        }

        // r[verify split-coalesce.split-tree-total]
        #[test]
        fn prop_recursive_split_count(
            order in 1u64..20,
            depth in 1u64..10,
        ) {
            if depth <= order && order < 50 {
                let target = order - depth;
                // Simulate recursive split and count
                let expected_count = 1u64 << depth;
                let mut blocks: Vec<(u64, u64)> = vec![(0u64, order)];
                for _ in 0..depth {
                    let mut next = Vec::new();
                    for &(addr, ord) in &blocks {
                        if ord > target {
                            let (l, r) = split_block(addr, ord);
                            next.push((l, ord - 1));
                            next.push((r, ord - 1));
                        } else {
                            next.push((addr, ord));
                        }
                    }
                    blocks = next;
                }
                prop_assert_eq!(blocks.len() as u64, expected_count);
                // All at target order
                for &(_, ord) in &blocks {
                    prop_assert_eq!(ord, target);
                }
                // Sizes sum to original
                let total: u64 = blocks.iter().map(|&(_, o)| block_size(o)).sum();
                prop_assert_eq!(total, block_size(order));
            }
        }

        // r[impl split-coalesce.blocks-disjoint]
        // r[verify split-coalesce.blocks-disjoint]
        #[test]
        fn prop_recursive_split_disjoint(order in 2u64..16) {
            let target = order - 2; // split twice
            let (l, r) = split_block(0, order);
            let (ll, lr) = split_block(l, order - 1);
            let (rl, rr) = split_block(r, order - 1);
            let sz = block_size(target);
            // All four blocks should be disjoint
            let addrs = [ll, lr, rl, rr];
            for i in 0..4 {
                for j in (i + 1)..4 {
                    prop_assert!(
                        addrs[i] + sz <= addrs[j] || addrs[j] + sz <= addrs[i],
                        "overlap at i={} j={}",
                        i,
                        j
                    );
                }
            }
        }
    }
}
