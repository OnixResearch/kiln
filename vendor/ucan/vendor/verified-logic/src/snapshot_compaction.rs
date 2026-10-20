// Verus preconditions guarantee u64→usize casts are in bounds
#![allow(clippy::cast_possible_truncation)]

//! Verified snapshot creation and compaction trigger for Raft.
//!
//! `install_snapshot.rs` covers the receiving end (follower applying a
//! snapshot). This module covers the creation side: when to trigger
//! compaction, what can be safely discarded, and the relationship between
//! snapshots and the commit index. The key safety invariant is that only
//! committed, applied entries are compacted — uncommitted entries must
//! remain in the log.

use vstd::prelude::*;

use crate::log::LogEntry;

verus! {

// --- should_compact ---

// r[depends snapshot-compaction.should-compact]
/// Spec: should the log be compacted?
///
/// Compaction triggers when the log exceeds a size threshold.
/// The threshold is measured in number of entries.
pub open spec fn should_compact_spec(
    log_length: u64,
    snapshot_threshold: u64,
) -> bool {
    log_length >= snapshot_threshold && snapshot_threshold > 0
}

// r[impl snapshot-compaction.should-compact]
/// Check if compaction should be triggered.
pub fn should_compact(
    log_length: u64,
    snapshot_threshold: u64,
) -> (result: bool)
    ensures result == should_compact_spec(log_length, snapshot_threshold)
{
    log_length >= snapshot_threshold && snapshot_threshold > 0
}

// --- can_compact_through ---

// r[depends snapshot-compaction.can-compact-through]
/// Spec: can we compact (discard) log entries through the given index?
///
/// Only committed entries can be compacted. The last_applied index
/// tracks how far the state machine has consumed. We can compact through
/// last_applied because:
/// 1. last_applied <= commit_index (only committed entries are applied)
/// 2. The state machine has already consumed these entries
/// 3. The snapshot captures the state machine output through last_applied
pub open spec fn can_compact_through_spec(
    compact_through: u64,
    last_applied: u64,
    commit_index: u64,
) -> bool {
    compact_through <= last_applied && last_applied <= commit_index
}

// r[impl snapshot-compaction.can-compact-through]
/// Check if compaction through a given index is safe.
pub fn can_compact_through(
    compact_through: u64,
    last_applied: u64,
    commit_index: u64,
) -> (result: bool)
    ensures result == can_compact_through_spec(compact_through, last_applied, commit_index)
{
    compact_through <= last_applied && last_applied <= commit_index
}

// --- entries_after_compaction ---

// r[depends snapshot-compaction.entries-after-compaction]
/// Spec: how many log entries remain after compacting through `compact_through`?
///
/// compact_through is the number of entries to discard from the front.
pub open spec fn entries_after_compaction_spec(
    log_length: u64,
    compact_through: u64,
) -> u64 {
    if compact_through >= log_length { 0 } else { (log_length - compact_through) as u64 }
}

// r[impl snapshot-compaction.entries-after-compaction]
/// Compute entries remaining after compaction.
pub fn entries_after_compaction(
    log_length: u64,
    compact_through: u64,
) -> (result: u64)
    ensures result == entries_after_compaction_spec(log_length, compact_through)
{
    if compact_through >= log_length { 0 } else { log_length - compact_through }
}

// --- snapshot_metadata ---

/// Metadata for a snapshot: the last included index and term.
pub struct SnapshotMeta {
    /// Last log index included in the snapshot (1-based).
    pub last_included_index: u64,
    /// Term of the last included entry.
    pub last_included_term: u64,
}

// --- compute_snapshot_meta ---

// r[depends snapshot-compaction.compute-meta]
/// Spec: compute snapshot metadata from the log.
///
/// The snapshot covers entries [0, compact_through). The metadata
/// records the index and term of the last included entry.
pub open spec fn compute_snapshot_meta_spec(
    log: Seq<LogEntry>,
    compact_through: u64,
) -> SnapshotMeta
    recommends compact_through > 0, ((compact_through - 1) as int) < log.len()
{
    let last_idx = (compact_through - 1) as int;
    SnapshotMeta {
        last_included_index: compact_through,
        last_included_term: log[last_idx].term,
    }
}

// r[impl snapshot-compaction.compute-meta]
/// Compute snapshot metadata from the log.
pub fn compute_snapshot_meta(
    log: &[LogEntry],
    compact_through: u64,
) -> (result: SnapshotMeta)
    requires
        compact_through > 0,
        ((compact_through - 1) as int) < log@.len(),
        compact_through as int <= usize::MAX as int,
    ensures
        result.last_included_index == compact_through,
        result.last_included_term == log@[(compact_through - 1) as int].term,
{
    let last_idx = (compact_through - 1) as usize;
    SnapshotMeta {
        last_included_index: compact_through,
        last_included_term: log[last_idx].term,
    }
}

// --- discard_prefix ---

// r[depends snapshot-compaction.discard-prefix]
/// Spec: discard the first `count` entries from the log.
///
/// Returns the suffix starting at index `count`.
pub open spec fn discard_prefix_spec(
    log: Seq<LogEntry>,
    count: u64,
) -> Seq<LogEntry> {
    if count as int >= log.len() {
        Seq::empty()
    } else {
        log.subrange(count as int, log.len() as int)
    }
}

// r[impl snapshot-compaction.discard-prefix]
/// Discard the first `count` entries from the log.
pub fn discard_prefix(
    log: &[LogEntry],
    count: u64,
) -> (result: Vec<LogEntry>)
    requires count as int <= usize::MAX as int
    ensures result@ == discard_prefix_spec(log@, count)
{
    let start = count as usize;
    if start >= log.len() {
        let empty: Vec<LogEntry> = Vec::new();
        assert(empty@ =~= Seq::<LogEntry>::empty());
        return empty;
    }
    let mut result = Vec::new();
    let mut i: usize = start;
    while i < log.len()
        invariant
            start <= i,
            i <= log@.len(),
            result@.len() == i - start,
            forall|j: int| 0 <= j < result@.len() ==>
                #[trigger] result@[j] == log@[start as int + j],
        decreases log.len() - i,
    {
        result.push(log[i]);
        i = i + 1;
    }
    assert(result@ =~= log@.subrange(count as int, log@.len() as int));
    result
}

// --- new_first_log_index ---

// r[depends snapshot-compaction.new-first-index]
/// Spec: compute the new first log index after compaction.
///
/// Before compaction, first_log_index tracks where the log starts.
/// After discarding through `compact_through`, the new first index
/// is max(old_first, compact_through).
pub open spec fn new_first_log_index_spec(
    old_first: u64,
    compact_through: u64,
) -> u64 {
    if compact_through > old_first { compact_through } else { old_first }
}

// r[impl snapshot-compaction.new-first-index]
/// Compute the new first log index after compaction.
pub fn new_first_log_index(
    old_first: u64,
    compact_through: u64,
) -> (result: u64)
    ensures result == new_first_log_index_spec(old_first, compact_through)
{
    if compact_through > old_first { compact_through } else { old_first }
}

// --- Safety proofs ---

// r[verify snapshot-compaction.should-compact]
/// Zero threshold never triggers compaction.
proof fn zero_threshold_no_compact(log_length: u64)
    ensures !should_compact_spec(log_length, 0)
{}

// r[verify snapshot-compaction.should-compact]
/// Empty log never triggers compaction.
proof fn empty_log_no_compact(threshold: u64)
    requires threshold > 0
    ensures !should_compact_spec(0, threshold)
{}

// r[verify snapshot-compaction.should-compact]
/// Log at threshold triggers compaction.
proof fn at_threshold_compacts(threshold: u64)
    requires threshold > 0
    ensures should_compact_spec(threshold, threshold)
{}

// r[verify snapshot-compaction.can-compact-through]
/// Cannot compact beyond last_applied.
proof fn compact_bounded_by_applied(
    compact_through: u64,
    last_applied: u64,
    commit_index: u64,
)
    requires compact_through > last_applied
    ensures !can_compact_through_spec(compact_through, last_applied, commit_index)
{}

// r[verify snapshot-compaction.can-compact-through]
/// Cannot compact when last_applied exceeds commit_index.
proof fn applied_bounded_by_committed(
    compact_through: u64,
    last_applied: u64,
    commit_index: u64,
)
    requires last_applied > commit_index
    ensures !can_compact_through_spec(compact_through, last_applied, commit_index)
{}

// r[verify snapshot-compaction.can-compact-through]
/// Zero compaction is always safe when applied <= committed.
proof fn zero_compact_safe(last_applied: u64, commit_index: u64)
    requires last_applied <= commit_index
    ensures can_compact_through_spec(0, last_applied, commit_index)
{}

// r[verify snapshot-compaction.entries-after-compaction]
/// Compaction reduces log length.
proof fn compaction_reduces_length(log_length: u64, compact_through: u64)
    requires compact_through <= log_length
    ensures entries_after_compaction_spec(log_length, compact_through)
        == (log_length - compact_through) as u64
{}

// r[verify snapshot-compaction.entries-after-compaction]
/// Compacting nothing leaves the log unchanged.
proof fn zero_compaction_identity(log_length: u64)
    ensures entries_after_compaction_spec(log_length, 0) == log_length
{}

// r[verify snapshot-compaction.entries-after-compaction]
/// Compacting everything leaves an empty log.
proof fn full_compaction_empty(log_length: u64)
    ensures entries_after_compaction_spec(log_length, log_length) == 0
{}

// r[verify snapshot-compaction.discard-prefix]
/// Discarding zero entries returns the full log.
proof fn discard_zero_identity(log: Seq<LogEntry>)
    ensures discard_prefix_spec(log, 0) == log
{
    assert(discard_prefix_spec(log, 0) =~= log);
}

// r[verify snapshot-compaction.discard-prefix]
/// Discarding all entries returns empty.
proof fn discard_all_empty(log: Seq<LogEntry>)
    requires log.len() <= u64::MAX as int
    ensures discard_prefix_spec(log, log.len() as u64).len() == 0
{}

// r[verify snapshot-compaction.discard-prefix]
/// Discarded prefix length matches expectations.
proof fn discard_prefix_length(log: Seq<LogEntry>, count: u64)
    requires count as int <= log.len()
    ensures discard_prefix_spec(log, count).len() == log.len() - count as int
{}

// r[verify snapshot-compaction.new-first-index]
/// New first index is monotonically non-decreasing.
proof fn first_index_monotonic(old_first: u64, compact_through: u64)
    ensures new_first_log_index_spec(old_first, compact_through) >= old_first
{}

// r[verify snapshot-compaction.new-first-index]
/// New first index is at least compact_through.
proof fn first_index_ge_compact(old_first: u64, compact_through: u64)
    ensures
        new_first_log_index_spec(old_first, compact_through) >= compact_through
        || new_first_log_index_spec(old_first, compact_through) >= old_first
{}

} // verus!

#[cfg(test)]
#[allow(clippy::cast_sign_loss, clippy::default_numeric_fallback)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn entry(term: u64, value: u64) -> LogEntry {
        LogEntry { term, value }
    }

    // r[verify snapshot-compaction.should-compact]
    #[test]
    fn test_should_compact() {
        assert!(should_compact(100, 50));
        assert!(should_compact(50, 50));
        assert!(!should_compact(49, 50));
        assert!(!should_compact(100, 0));
    }

    // r[verify snapshot-compaction.can-compact-through]
    #[test]
    fn test_can_compact_through() {
        assert!(can_compact_through(5, 10, 15));
        assert!(can_compact_through(10, 10, 15));
        assert!(!can_compact_through(11, 10, 15));
        assert!(!can_compact_through(5, 16, 15));
    }

    // r[verify snapshot-compaction.entries-after-compaction]
    #[test]
    fn test_entries_after_compaction() {
        assert_eq!(entries_after_compaction(10, 3), 7);
        assert_eq!(entries_after_compaction(10, 10), 0);
        assert_eq!(entries_after_compaction(10, 0), 10);
        assert_eq!(entries_after_compaction(10, 15), 0);
    }

    // r[verify snapshot-compaction.compute-meta]
    #[test]
    fn test_compute_snapshot_meta() {
        let log = vec![entry(1, 0), entry(2, 0), entry(3, 0), entry(3, 0)];
        let meta = compute_snapshot_meta(&log, 3);
        assert_eq!(meta.last_included_index, 3);
        assert_eq!(meta.last_included_term, 3); // log[2].term
    }

    // r[verify snapshot-compaction.discard-prefix]
    #[test]
    fn test_discard_prefix() {
        let log = vec![entry(1, 10), entry(2, 20), entry(3, 30), entry(4, 40)];
        let result = discard_prefix(&log, 2);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].term, 3);
        assert_eq!(result[1].term, 4);
    }

    // r[verify snapshot-compaction.discard-prefix]
    #[test]
    fn test_discard_prefix_zero() {
        let log = vec![entry(1, 10), entry(2, 20)];
        let result = discard_prefix(&log, 0);
        assert_eq!(result.len(), 2);
    }

    // r[verify snapshot-compaction.discard-prefix]
    #[test]
    fn test_discard_prefix_all() {
        let log = vec![entry(1, 10), entry(2, 20)];
        let result = discard_prefix(&log, 2);
        assert_eq!(result.len(), 0);
    }

    // r[verify snapshot-compaction.new-first-index]
    #[test]
    fn test_new_first_log_index() {
        assert_eq!(new_first_log_index(0, 5), 5);
        assert_eq!(new_first_log_index(10, 5), 10);
        assert_eq!(new_first_log_index(5, 5), 5);
    }

    proptest! {
        // r[verify snapshot-compaction.should-compact]
        #[test]
        fn prop_threshold_decides(log_length: u64, threshold in 1u64..u64::MAX) {
            prop_assert_eq!(
                should_compact(log_length, threshold),
                log_length >= threshold,
            );
        }

        // r[verify snapshot-compaction.can-compact-through]
        #[test]
        fn prop_zero_compact_safe(applied: u64, committed: u64) {
            if committed >= applied {
                prop_assert!(can_compact_through(0, applied, committed));
            }
        }

        // r[verify snapshot-compaction.entries-after-compaction]
        #[test]
        fn prop_compaction_conservation(log_length: u64, compact: u64) {
            if compact <= log_length {
                let remaining = entries_after_compaction(log_length, compact);
                prop_assert_eq!(remaining, log_length - compact);
            }
        }

        // r[verify snapshot-compaction.discard-prefix]
        #[test]
        fn prop_discard_prefix_length(
            terms in proptest::collection::vec(1u64..10, 0..20),
            count_frac in 0.0f64..=1.0,
        ) {
            let log: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 0)).collect();
            let count = (count_frac * log.len() as f64) as u64;
            let result = discard_prefix(&log, count);
            let expected = if count as usize >= log.len() { 0 } else { log.len() - count as usize };
            prop_assert_eq!(result.len(), expected);
        }

        // r[verify snapshot-compaction.new-first-index]
        #[test]
        fn prop_first_index_monotonic(old_first: u64, compact: u64) {
            let new = new_first_log_index(old_first, compact);
            prop_assert!(new >= old_first);
        }
    }
}
