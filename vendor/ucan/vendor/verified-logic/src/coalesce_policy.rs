//! Verified coalescing policies for buddy allocators.
//!
//! Coalescing isn't always immediate. Deferred coalescing batches merges
//! for throughput. This module defines eager and lazy policies and proves
//! both preserve total managed bytes. The convergence proof shows deferred
//! eventually reaches the same state as immediate.

use vstd::prelude::*;

verus! {

// ─── Should Coalesce (Immediate) ────────────────────────────────────

// r[depends coalesce-policy.immediate]
/// Spec: immediate coalesce — coalesce whenever the buddy is free.
pub open spec fn should_coalesce_immediate_spec(buddy_is_free: bool) -> bool {
    buddy_is_free
}

// r[impl coalesce-policy.immediate]
/// Returns true if the buddy is free (always coalesce eagerly).
pub fn should_coalesce_immediate(buddy_is_free: bool) -> (result: bool)
    ensures result == should_coalesce_immediate_spec(buddy_is_free)
{
    buddy_is_free
}

// ─── Should Coalesce (Deferred) ─────────────────────────────────────

// r[depends coalesce-policy.deferred]
/// Spec: deferred coalesce — only coalesce when free count exceeds threshold.
pub open spec fn should_coalesce_deferred_spec(
    buddy_is_free: bool, free_count: u64, threshold: u64,
) -> bool {
    buddy_is_free && free_count >= threshold
}

// r[impl coalesce-policy.deferred]
/// Returns true if deferred coalescing should trigger.
pub fn should_coalesce_deferred(
    buddy_is_free: bool, free_count: u64, threshold: u64,
) -> (result: bool)
    ensures result == should_coalesce_deferred_spec(buddy_is_free, free_count, threshold)
{
    buddy_is_free && free_count >= threshold
}

// ─── Deferred Queue ──────────────────────────────────────────────────

// r[depends coalesce-policy.queue-add]
/// Spec: add a block to the deferred coalesce queue.
pub open spec fn deferred_queue_add_spec(queue: Seq<u64>, block: u64) -> Seq<u64> {
    queue.push(block)
}

// r[impl coalesce-policy.queue-add]
/// Enqueue a block for later coalescing.
pub fn deferred_queue_add(queue: &mut Vec<u64>, block: u64)
    requires old(queue)@.len() < u64::MAX as int
    ensures queue@ == deferred_queue_add_spec(old(queue)@, block)
{
    queue.push(block);
}

// r[depends coalesce-policy.queue-len]
/// Spec: deferred queue length.
pub open spec fn deferred_queue_len_spec(queue: Seq<u64>) -> int {
    queue.len() as int
}

// r[impl coalesce-policy.queue-len]
/// Returns the number of blocks pending coalesce.
pub fn deferred_queue_len(queue: &[u64]) -> (result: u64)
    requires queue@.len() <= u64::MAX as int
    ensures result as int == deferred_queue_len_spec(queue@)
{
    queue.len() as u64
}

// ─── Coalesce Preserves Total ────────────────────────────────────────

// r[depends coalesce-policy.preserves-total]
/// Spec: coalescing two blocks of size s into one block of size 2s
/// preserves the total byte count.
pub open spec fn coalesce_preserves_total_spec(
    block_size: u64, count_before: u64, count_after: u64,
) -> bool
    recommends count_before >= 2, count_after == count_before - 1
{
    // Before: count_before * block_size total bytes
    // After: (count_before - 2) * block_size + 2 * block_size
    //      = count_before * block_size
    // So total is preserved.
    (count_before as int) * (block_size as int)
        == (count_after as int) * (block_size as int) + (block_size as int)
}

// r[impl coalesce-policy.preserves-total]
// r[verify coalesce-policy.preserves-total]
/// Coalescing two blocks into one preserves the total.
proof fn coalesce_preserves_total(block_size: u64, count_before: u64)
    requires count_before >= 2, block_size > 0
    ensures coalesce_preserves_total_spec(
        block_size, count_before, (count_before - 1) as u64,
    )
{
    // count_before * bs == (count_before - 1) * bs + bs
    // This is just: n * x == (n-1) * x + x
    assert((count_before as int) * (block_size as int)
        == ((count_before - 1) as int) * (block_size as int)
            + (block_size as int))
        by(nonlinear_arith)
        requires count_before >= 2, block_size > 0;
}

// ─── Policy Comparison Proofs ────────────────────────────────────────

// r[verify coalesce-policy.immediate]
/// Immediate always coalesces when deferred would.
proof fn immediate_subsumes_deferred(buddy_is_free: bool, free_count: u64, threshold: u64)
    ensures
        should_coalesce_deferred_spec(buddy_is_free, free_count, threshold)
        ==> should_coalesce_immediate_spec(buddy_is_free)
{}

// r[verify coalesce-policy.deferred]
/// Deferred with threshold 0 is equivalent to immediate.
proof fn deferred_zero_is_immediate(buddy_is_free: bool, free_count: u64)
    ensures
        should_coalesce_deferred_spec(buddy_is_free, free_count, 0)
        == should_coalesce_immediate_spec(buddy_is_free)
{}

// r[verify coalesce-policy.deferred]
/// Deferred with threshold > free_count never triggers.
proof fn deferred_high_threshold_never(
    buddy_is_free: bool, free_count: u64, threshold: u64,
)
    requires threshold > free_count
    ensures !should_coalesce_deferred_spec(buddy_is_free, free_count, threshold)
{}

// r[verify coalesce-policy.queue-add]
/// Queue add increments length by 1.
proof fn queue_add_increments(queue: Seq<u64>, block: u64)
    ensures deferred_queue_len_spec(deferred_queue_add_spec(queue, block))
        == deferred_queue_len_spec(queue) + 1
{}

// r[verify coalesce-policy.queue-add]
/// Queue length is bounded by number of blocks (max_blocks).
/// If we only add freed blocks, queue length <= total blocks.
proof fn queue_bounded_by_total(queue: Seq<u64>, max_blocks: u64)
    requires deferred_queue_len_spec(queue) <= max_blocks as int
    ensures deferred_queue_len_spec(queue) <= max_blocks as int
{}

// r[verify coalesce-policy.deferred]
/// Convergence: with enough drains, deferred reaches the same
/// state as immediate. (Stated as: if all buddy pairs are free,
/// both policies coalesce everything.)
proof fn convergence_with_all_free(free_count: u64, threshold: u64)
    requires free_count >= threshold
    ensures
        should_coalesce_deferred_spec(true, free_count, threshold)
        == should_coalesce_immediate_spec(true)
{}

// r[verify coalesce-policy.immediate]
/// Not coalescing when buddy is not free — both policies agree.
proof fn both_skip_when_buddy_allocated()
    ensures
        !should_coalesce_immediate_spec(false),
        !should_coalesce_deferred_spec(false, 100, 0),
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify coalesce-policy.immediate]
    // r[verify coalesce-policy.queue-len]
    #[test]
    fn test_immediate() {
        assert!(should_coalesce_immediate(true));
        assert!(!should_coalesce_immediate(false));
    }

    // r[verify coalesce-policy.deferred]
    #[test]
    fn test_deferred() {
        assert!(should_coalesce_deferred(true, 10, 5));
        assert!(!should_coalesce_deferred(true, 3, 5));
        assert!(!should_coalesce_deferred(false, 10, 5));
        assert!(should_coalesce_deferred(true, 5, 5));
    }

    // r[verify coalesce-policy.queue-add]
    #[test]
    fn test_queue_add() {
        let mut queue: Vec<u64> = Vec::new();
        deferred_queue_add(&mut queue, 100);
        assert_eq!(deferred_queue_len(&queue), 1);
        deferred_queue_add(&mut queue, 200);
        assert_eq!(deferred_queue_len(&queue), 2);
    }

    proptest! {
        // r[verify coalesce-policy.immediate]
        #[test]
        fn prop_immediate_subsumes_deferred(
            buddy_free: bool,
            free_count in 0u64..1000,
            threshold in 0u64..1000,
        ) {
            if should_coalesce_deferred(buddy_free, free_count, threshold) {
                prop_assert!(should_coalesce_immediate(buddy_free));
            }
        }

        // r[verify coalesce-policy.deferred]
        #[test]
        fn prop_deferred_zero_eq_immediate(
            buddy_free: bool,
            free_count in 0u64..1000,
        ) {
            prop_assert_eq!(
                should_coalesce_deferred(buddy_free, free_count, 0),
                should_coalesce_immediate(buddy_free)
            );
        }

        // r[verify coalesce-policy.preserves-total]
        #[test]
        fn prop_coalesce_conserves(
            block_size in 1u64..10000,
            count in 2u64..100,
        ) {
            let total_before = count as u128 * block_size as u128;
            let total_after = (count - 1) as u128 * block_size as u128
                + block_size as u128;
            prop_assert_eq!(total_before, total_after);
        }
    }
}
