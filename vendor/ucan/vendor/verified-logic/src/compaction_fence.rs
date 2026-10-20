//! Verified log compaction coordination with replication for Raft.
//!
//! When the leader compacts its log, entries before the compaction point
//! are discarded. If a follower's next_index points to a discarded entry,
//! the leader must switch that follower to snapshot mode. This module
//! provides the coordination primitives that prevent compaction from
//! racing with inflight replication.
//!
//! Key invariant: the leader MUST NOT compact entries that any follower
//! in Replicate mode still needs. Either the follower must be switched
//! to Snapshot mode first, or compaction must wait.

use vstd::prelude::*;

verus! {

// --- is_safe_to_compact ---

// r[depends compaction-fence.safe-to-compact]
/// Spec: can the leader safely compact up to `compact_index`?
///
/// Compaction is safe when `compact_index` does not exceed the minimum
/// next_index across all followers. If any follower needs entries at
/// or before compact_index, those entries must not be discarded until
/// the follower has been switched to snapshot mode or caught up.
///
/// The `min_next_index` parameter is the minimum of all followers'
/// next_index values (excluding followers already in snapshot mode).
pub open spec fn is_safe_to_compact_spec(
    compact_index: u64,
    min_next_index: u64,
) -> bool {
    compact_index < min_next_index
}

// r[impl compaction-fence.safe-to-compact]
/// Check if compaction up to `compact_index` is safe.
pub fn is_safe_to_compact(
    compact_index: u64,
    min_next_index: u64,
) -> (result: bool)
    ensures result == is_safe_to_compact_spec(compact_index, min_next_index)
{
    compact_index < min_next_index
}

// --- min_next_index ---

// r[depends compaction-fence.min-next-index]
/// Spec: compute the minimum next_index across active (non-snapshot) followers.
pub open spec fn min_active_next_spec(
    next_indices: Seq<u64>,
    modes: Seq<u8>,
    self_id: u64,
) -> u64
    recommends
        next_indices.len() == modes.len(),
        next_indices.len() > 0,
{
    min_active_next_from_spec(next_indices, modes, self_id, 0, u64::MAX)
}

pub open spec fn min_active_next_from_spec(
    next_indices: Seq<u64>,
    modes: Seq<u8>,
    self_id: u64,
    from: int,
    current_min: u64,
) -> u64
    decreases next_indices.len() - from,
{
    if from >= next_indices.len() {
        current_min
    } else if from == self_id as int {
        // Skip self — leader doesn't replicate to itself
        min_active_next_from_spec(next_indices, modes, self_id, from + 1, current_min)
    } else if modes[from] == 2u8 {
        // Skip snapshot-mode followers — they don't need log entries
        min_active_next_from_spec(next_indices, modes, self_id, from + 1, current_min)
    } else {
        let new_min = if next_indices[from] < current_min {
            next_indices[from]
        } else {
            current_min
        };
        min_active_next_from_spec(next_indices, modes, self_id, from + 1, new_min)
    }
}

// r[impl compaction-fence.min-next-index]
/// Compute the minimum next_index among active followers.
pub fn min_active_next(
    next_indices: &[u64],
    modes: &[u8],
    self_id: u64,
) -> (result: u64)
    requires
        next_indices@.len() == modes@.len(),
        next_indices@.len() <= u64::MAX as int,
    ensures result == min_active_next_spec(next_indices@, modes@, self_id)
{
    let mut min_val: u64 = u64::MAX;
    let mut i: usize = 0;
    // Unfold spec definition once: min_active_next_spec == min_active_next_from_spec(... 0, u64::MAX)
    assert(min_active_next_spec(next_indices@, modes@, self_id) ==
        min_active_next_from_spec(next_indices@, modes@, self_id, 0, u64::MAX));
    while i < next_indices.len()
        invariant
            0 <= i <= next_indices@.len(),
            next_indices@.len() == modes@.len(),
            next_indices@.len() <= u64::MAX as int,
            min_active_next_spec(next_indices@, modes@, self_id) ==
                min_active_next_from_spec(next_indices@, modes@, self_id, i as int, min_val),
        decreases next_indices.len() - i,
    {
        if i as u64 != self_id && modes[i] != 2 {
            if next_indices[i] < min_val {
                min_val = next_indices[i];
            }
        }
        i = i + 1;
    }
    min_val
}

// --- needs_snapshot_switch ---

// r[depends compaction-fence.needs-snapshot]
/// Spec: does this follower need to be switched to snapshot mode?
///
/// After compaction, if a follower's next_index is at or before the
/// new first log index, it can no longer receive entries from the log.
pub open spec fn needs_snapshot_switch_spec(
    follower_next: u64,
    first_log_index: u64,
) -> bool {
    follower_next <= first_log_index
}

// r[impl compaction-fence.needs-snapshot]
/// Check if a follower needs to switch to snapshot mode.
pub fn needs_snapshot_switch(
    follower_next: u64,
    first_log_index: u64,
) -> (result: bool)
    ensures result == needs_snapshot_switch_spec(follower_next, first_log_index)
{
    follower_next <= first_log_index
}

// --- safe_compact_index ---

// r[depends compaction-fence.safe-compact-index]
/// Spec: the maximum index that can be safely compacted.
///
/// This is min(desired_compact, min_active_next - 1). We can never
/// compact past what the slowest active follower still needs.
/// If min_active_next is 0 (shouldn't happen in practice), returns 0.
pub open spec fn safe_compact_index_spec(
    desired_compact: u64,
    min_next: u64,
) -> u64 {
    if min_next == 0 {
        0u64
    } else {
        let limit = (min_next - 1) as u64;
        if desired_compact < limit {
            desired_compact
        } else {
            limit
        }
    }
}

// r[impl compaction-fence.safe-compact-index]
/// Compute the safe compaction index.
pub fn safe_compact_index(
    desired_compact: u64,
    min_next: u64,
) -> (result: u64)
    ensures result == safe_compact_index_spec(desired_compact, min_next)
{
    if min_next == 0 {
        0
    } else {
        let limit = min_next - 1;
        if desired_compact < limit {
            desired_compact
        } else {
            limit
        }
    }
}

// --- Safety proofs ---

// r[verify compaction-fence.safe-to-compact]
/// Safe compaction index is always safe to compact.
proof fn safe_index_is_safe(desired_compact: u64, min_next: u64)
    requires min_next > 0
    ensures is_safe_to_compact_spec(
        safe_compact_index_spec(desired_compact, min_next),
        min_next,
    )
{}

// r[verify compaction-fence.safe-compact-index]
/// Safe compact index never exceeds desired.
proof fn safe_index_le_desired(desired_compact: u64, min_next: u64)
    ensures safe_compact_index_spec(desired_compact, min_next) <= desired_compact
{}

// r[verify compaction-fence.safe-compact-index]
/// Safe compact index never exceeds the limit.
proof fn safe_index_le_limit(desired_compact: u64, min_next: u64)
    requires min_next > 0
    ensures safe_compact_index_spec(desired_compact, min_next) < min_next
{}

// r[verify compaction-fence.needs-snapshot]
/// Follower within the log does not need snapshot.
proof fn within_log_no_snapshot(follower_next: u64, first_log_index: u64)
    requires follower_next > first_log_index
    ensures !needs_snapshot_switch_spec(follower_next, first_log_index)
{}

// r[verify compaction-fence.needs-snapshot]
/// Follower behind compaction needs snapshot.
proof fn behind_compaction_needs_snapshot(follower_next: u64, first_log_index: u64)
    requires follower_next <= first_log_index
    ensures needs_snapshot_switch_spec(follower_next, first_log_index)
{}

// r[verify compaction-fence.safe-to-compact]
/// Compaction at 0 is always safe (nothing to compact).
proof fn zero_always_safe(min_next: u64)
    requires min_next > 0
    ensures is_safe_to_compact_spec(0, min_next)
{}

// r[verify compaction-fence.needs-snapshot]
/// needs_snapshot_switch agrees with install_snapshot trigger.
proof fn agrees_with_install_snapshot(follower_next: u64, first_log_index: u64)
    ensures needs_snapshot_switch_spec(follower_next, first_log_index)
        == (follower_next <= first_log_index)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify compaction-fence.safe-to-compact]
    #[test]
    fn test_is_safe_to_compact() {
        assert!(is_safe_to_compact(5, 10)); // well within
        assert!(!is_safe_to_compact(10, 10)); // at boundary
        assert!(!is_safe_to_compact(15, 10)); // past boundary
    }

    // r[verify compaction-fence.min-next-index]
    #[test]
    fn test_min_active_next() {
        // 5-node cluster, self=0, node 3 in snapshot mode
        let nexts = [11, 8, 10, 5, 9];
        let modes = [0, 0, 0, 2, 0]; // node 3 = snapshot
        let result = min_active_next(&nexts, &modes, 0);
        assert_eq!(result, 8); // min of {8, 10, 9} (skip self=0 and snapshot=3)
    }

    // r[verify compaction-fence.min-next-index]
    #[test]
    fn test_min_active_next_all_snapshot() {
        let nexts = [11, 5, 5];
        let modes = [0, 2, 2]; // all followers in snapshot mode
        let result = min_active_next(&nexts, &modes, 0);
        assert_eq!(result, u64::MAX); // no active followers
    }

    // r[verify compaction-fence.needs-snapshot]
    #[test]
    fn test_needs_snapshot_switch() {
        assert!(needs_snapshot_switch(5, 10)); // behind
        assert!(needs_snapshot_switch(10, 10)); // at boundary
        assert!(!needs_snapshot_switch(15, 10)); // within log
    }

    // r[verify compaction-fence.safe-compact-index]
    #[test]
    fn test_safe_compact_index() {
        assert_eq!(safe_compact_index(20, 10), 9); // limited by min_next
        assert_eq!(safe_compact_index(5, 10), 5); // desired is less
        assert_eq!(safe_compact_index(9, 10), 9); // at limit
        assert_eq!(safe_compact_index(100, 0), 0); // edge: zero min
    }

    proptest! {
        // r[verify compaction-fence.safe-compact-index]
        #[test]
        fn prop_safe_index_is_safe(
            desired in 0u64..1000,
            min_next in 1u64..1000,
        ) {
            let safe = safe_compact_index(desired, min_next);
            prop_assert!(safe < min_next);
        }

        // r[verify compaction-fence.safe-compact-index]
        #[test]
        fn prop_safe_index_le_desired(desired: u64, min_next: u64) {
            let safe = safe_compact_index(desired, min_next);
            prop_assert!(safe <= desired);
        }

        // r[verify compaction-fence.needs-snapshot]
        #[test]
        fn prop_needs_snapshot_agrees(follower_next: u64, first_log_index: u64) {
            let needs = needs_snapshot_switch(follower_next, first_log_index);
            prop_assert_eq!(needs, follower_next <= first_log_index);
        }

        #[test]
        fn prop_safe_index_never_exceeds_applied(
            desired in 0u64..1000,
            applied in 0u64..1000,
        ) {
            // min_active_next acts like a floor derived from applied state
            // safe_compact_index never exceeds desired
            let safe = safe_compact_index(desired, applied.max(1));
            prop_assert!(safe <= desired);
        }

        #[test]
        fn prop_safe_compact_monotonic(
            desired1 in 0u64..500,
            desired2 in 0u64..500,
            min_next in 1u64..100,
        ) {
            let s1 = safe_compact_index(desired1, min_next);
            let s2 = safe_compact_index(desired2, min_next);
            if desired1 <= desired2 {
                prop_assert!(s1 <= s2);
            }
        }

        #[test]
        fn prop_is_safe_compact_iff_strict_less(
            compact in 0u64..100,
            min_next in 0u64..100,
        ) {
            let safe = is_safe_to_compact(compact, min_next);
            prop_assert_eq!(safe, compact < min_next);
        }
    }
}
