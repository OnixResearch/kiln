//! Verified lastApplied tracking for Raft state machine application.
//!
//! After entries are committed, they must be applied to the state machine
//! in order, from `lastApplied + 1` through `commitIndex`. This module
//! provides the primitives for tracking the application cursor, computing
//! the next entry to apply, and batch application bounds.

use vstd::prelude::*;

verus! {

// --- can_apply ---

// r[depends last-applied.can-apply]
/// Spec: are there committed entries waiting to be applied?
pub open spec fn can_apply_spec(last_applied: u64, commit_index: u64) -> bool {
    last_applied < commit_index
}

// r[impl last-applied.can-apply]
/// Returns true when there are committed entries waiting to be applied.
pub fn can_apply(last_applied: u64, commit_index: u64) -> (result: bool)
    ensures result == can_apply_spec(last_applied, commit_index)
{
    last_applied < commit_index
}

// --- advance_last_applied ---

// r[depends last-applied.advance]
/// Spec: advance the apply cursor by one entry.
///
/// Returns min(last_applied + 1, commit_index) when there are entries
/// to apply, otherwise last_applied (no change).
pub open spec fn advance_last_applied_spec(
    last_applied: u64,
    commit_index: u64,
) -> u64 {
    if last_applied < commit_index {
        (last_applied + 1) as u64
    } else {
        last_applied
    }
}

// r[impl last-applied.advance]
/// Advance the apply cursor by one entry.
pub fn advance_last_applied(
    last_applied: u64,
    commit_index: u64,
) -> (result: u64)
    requires last_applied < u64::MAX || last_applied >= commit_index
    ensures result == advance_last_applied_spec(last_applied, commit_index)
{
    if last_applied < commit_index {
        last_applied + 1
    } else {
        last_applied
    }
}

// --- pending_apply_count ---

// r[depends last-applied.apply-count]
/// Spec: number of entries waiting to be applied.
pub open spec fn pending_apply_count_spec(
    last_applied: u64,
    commit_index: u64,
) -> u64 {
    if commit_index >= last_applied {
        (commit_index - last_applied) as u64
    } else {
        0u64
    }
}

// r[impl last-applied.apply-count]
/// Returns the number of committed entries waiting to be applied.
pub fn pending_apply_count(
    last_applied: u64,
    commit_index: u64,
) -> (result: u64)
    ensures result == pending_apply_count_spec(last_applied, commit_index)
{
    if commit_index >= last_applied {
        commit_index - last_applied
    } else {
        0
    }
}

// --- next_apply_index ---

// r[depends last-applied.apply-entry-index]
/// Spec: the 1-based log index of the next entry to apply.
pub open spec fn next_apply_index_spec(last_applied: u64) -> u64 {
    (last_applied + 1) as u64
}

// r[impl last-applied.apply-entry-index]
/// Returns the 1-based log index of the next entry to apply.
pub fn next_apply_index(last_applied: u64) -> (result: u64)
    requires last_applied < u64::MAX
    ensures result == next_apply_index_spec(last_applied)
{
    last_applied + 1
}

// --- batch_apply_end ---

// r[depends last-applied.batch-apply]
/// Spec: the last_applied value after applying up to max_batch entries.
///
/// Returns min(last_applied + max_batch, commit_index).
pub open spec fn batch_apply_end_spec(
    last_applied: u64,
    commit_index: u64,
    max_batch: u64,
) -> u64 {
    let target = if (last_applied as int + max_batch as int) > u64::MAX as int {
        u64::MAX
    } else {
        (last_applied as int + max_batch as int) as u64
    };
    if target <= commit_index {
        target
    } else {
        commit_index
    }
}

// r[impl last-applied.batch-apply]
/// Compute the end of a batch apply.
pub fn batch_apply_end(
    last_applied: u64,
    commit_index: u64,
    max_batch: u64,
) -> (result: u64)
    ensures result == batch_apply_end_spec(last_applied, commit_index, max_batch)
{
    // Saturating add to avoid overflow
    let target = if max_batch > u64::MAX - last_applied {
        u64::MAX
    } else {
        last_applied + max_batch
    };
    if target <= commit_index {
        target
    } else {
        commit_index
    }
}

// --- invariant ---

// r[depends last-applied.invariant]
/// Spec: the last-applied invariant.
pub open spec fn last_applied_invariant_spec(
    last_applied: u64,
    commit_index: u64,
) -> bool {
    last_applied <= commit_index
}

// r[impl last-applied.invariant]
/// Check the last-applied invariant.
pub fn last_applied_invariant(
    last_applied: u64,
    commit_index: u64,
) -> (result: bool)
    ensures result == last_applied_invariant_spec(last_applied, commit_index)
{
    last_applied <= commit_index
}

// --- Safety proofs ---

// r[verify last-applied.advance]
/// Advancing preserves the invariant.
proof fn advance_preserves_invariant(last_applied: u64, commit_index: u64)
    requires last_applied_invariant_spec(last_applied, commit_index)
    ensures last_applied_invariant_spec(
        advance_last_applied_spec(last_applied, commit_index),
        commit_index,
    )
{}

// r[verify last-applied.advance]
/// Advance is monotonic: result >= last_applied.
proof fn advance_monotonic(last_applied: u64, commit_index: u64)
    ensures advance_last_applied_spec(last_applied, commit_index) >= last_applied
{}

// r[verify last-applied.advance]
/// Advance never exceeds commit_index.
proof fn advance_bounded(last_applied: u64, commit_index: u64)
    requires last_applied_invariant_spec(last_applied, commit_index)
    ensures advance_last_applied_spec(last_applied, commit_index) <= commit_index
{}

// r[verify last-applied.can-apply]
/// can_apply is false when last_applied == commit_index.
proof fn caught_up_nothing_to_apply(index: u64)
    ensures !can_apply_spec(index, index)
{}

// r[verify last-applied.apply-count]
/// Pending count is zero when last_applied >= commit_index.
proof fn no_pending_when_caught_up(last_applied: u64, commit_index: u64)
    requires last_applied >= commit_index
    ensures pending_apply_count_spec(last_applied, commit_index) == 0
{}

// r[verify last-applied.apply-count]
/// Pending count equals commit_index - last_applied when invariant holds.
proof fn pending_count_correct(last_applied: u64, commit_index: u64)
    requires last_applied_invariant_spec(last_applied, commit_index)
    ensures pending_apply_count_spec(last_applied, commit_index) == (commit_index - last_applied) as u64
{}

// r[verify last-applied.batch-apply]
/// Batch end is >= last_applied.
proof fn batch_apply_ge_last_applied(
    last_applied: u64,
    commit_index: u64,
    max_batch: u64,
)
    requires last_applied <= commit_index
    ensures batch_apply_end_spec(last_applied, commit_index, max_batch) >= last_applied
{}

// r[verify last-applied.batch-apply]
/// Batch end is <= commit_index.
proof fn batch_apply_le_commit(
    last_applied: u64,
    commit_index: u64,
    max_batch: u64,
)
    ensures batch_apply_end_spec(last_applied, commit_index, max_batch) <= commit_index
{}

// r[verify last-applied.invariant]
/// Increasing commit_index preserves the invariant.
proof fn commit_advance_preserves_invariant(
    last_applied: u64,
    old_commit: u64,
    new_commit: u64,
)
    requires
        last_applied_invariant_spec(last_applied, old_commit),
        new_commit >= old_commit,
    ensures last_applied_invariant_spec(last_applied, new_commit)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify last-applied.can-apply]
    #[test]
    fn test_can_apply() {
        assert!(can_apply(0, 5));
        assert!(can_apply(4, 5));
        assert!(!can_apply(5, 5));
        assert!(!can_apply(6, 5));
    }

    // r[verify last-applied.advance]
    #[test]
    fn test_advance_last_applied() {
        assert_eq!(advance_last_applied(0, 5), 1);
        assert_eq!(advance_last_applied(4, 5), 5);
        assert_eq!(advance_last_applied(5, 5), 5); // caught up
        assert_eq!(advance_last_applied(5, 3), 5); // invariant violated but safe
    }

    // r[verify last-applied.apply-count]
    #[test]
    fn test_pending_apply_count() {
        assert_eq!(pending_apply_count(0, 5), 5);
        assert_eq!(pending_apply_count(3, 5), 2);
        assert_eq!(pending_apply_count(5, 5), 0);
        assert_eq!(pending_apply_count(7, 5), 0);
    }

    // r[verify last-applied.apply-entry-index]
    #[test]
    fn test_next_apply_index() {
        assert_eq!(next_apply_index(0), 1);
        assert_eq!(next_apply_index(4), 5);
        assert_eq!(next_apply_index(99), 100);
    }

    // r[verify last-applied.batch-apply]
    #[test]
    fn test_batch_apply_end() {
        // Normal batch
        assert_eq!(batch_apply_end(0, 10, 3), 3);
        // Batch capped by commit
        assert_eq!(batch_apply_end(0, 2, 5), 2);
        // Already caught up
        assert_eq!(batch_apply_end(5, 5, 3), 5);
        // Overflow protection
        assert_eq!(batch_apply_end(u64::MAX - 1, u64::MAX, u64::MAX), u64::MAX);
    }

    // r[verify last-applied.invariant]
    #[test]
    fn test_invariant() {
        assert!(last_applied_invariant(0, 0));
        assert!(last_applied_invariant(0, 5));
        assert!(last_applied_invariant(5, 5));
        assert!(!last_applied_invariant(6, 5));
    }

    proptest! {
        // r[verify last-applied.advance]
        #[test]
        fn prop_advance_monotonic(
            last_applied in 0u64..1000,
            commit_index in 0u64..1000,
        ) {
            if last_applied < u64::MAX || last_applied >= commit_index {
                let result = advance_last_applied(last_applied, commit_index);
                prop_assert!(result >= last_applied);
            }
        }

        // r[verify last-applied.invariant]
        #[test]
        fn prop_advance_preserves_invariant(
            last_applied in 0u64..1000,
            commit_index in 0u64..1000,
        ) {
            if last_applied <= commit_index && (last_applied < u64::MAX || last_applied >= commit_index) {
                let result = advance_last_applied(last_applied, commit_index);
                prop_assert!(result <= commit_index);
            }
        }

        // r[verify last-applied.batch-apply]
        #[test]
        fn prop_batch_apply_bounded(
            last_applied in 0u64..1000,
            commit_index in 0u64..1000,
            max_batch in 1u64..100,
        ) {
            let end = batch_apply_end(last_applied, commit_index, max_batch);
            // End is always <= commit_index
            prop_assert!(end <= commit_index);
            if last_applied <= commit_index {
                // When invariant holds, end >= last_applied
                prop_assert!(end >= last_applied);
                // Batch size check
                prop_assert!(end - last_applied <= max_batch);
            }
        }

        // r[verify last-applied.apply-count]
        #[test]
        fn prop_pending_count_model(
            last_applied in 0u64..1000,
            commit_index in 0u64..1000,
        ) {
            let count = pending_apply_count(last_applied, commit_index);
            if commit_index >= last_applied {
                prop_assert_eq!(count, commit_index - last_applied);
            } else {
                prop_assert_eq!(count, 0);
            }
        }
    }
}
