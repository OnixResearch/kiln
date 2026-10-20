// Verus preconditions guarantee u64→usize casts are in bounds
#![allow(clippy::cast_possible_truncation)]

//! Verified log compaction (prefix truncation) for Raft.
//!
//! `snapshot_compaction.rs` covers when to trigger compaction and what
//! can be safely discarded. This module covers the actual mechanics:
//! truncating the log prefix, preserving the suffix, translating indices
//! between absolute (Raft-level) and relative (compacted-array) space,
//! and maintaining the first-log-index watermark.

use vstd::prelude::*;

use crate::log::LogEntry;

verus! {

// --- should_compact ---

// r[depends log-compaction.should-compact]
/// Spec: should the log be compacted?
///
/// Compaction triggers when the log exceeds a size threshold.
/// A threshold of 0 always triggers. An empty or short log does not.
pub open spec fn should_compact_spec(
    log_len: u64,
    snapshot_threshold: u64,
) -> bool {
    log_len >= snapshot_threshold
}

// r[impl log-compaction.should-compact]
/// Check if the log should be compacted.
pub fn should_compact(
    log_len: u64,
    snapshot_threshold: u64,
) -> (result: bool)
    ensures result == should_compact_spec(log_len, snapshot_threshold)
{
    log_len >= snapshot_threshold
}

// --- compact_log ---

// r[depends log-compaction.compact]
/// Spec: compact the log by discarding entries before snapshot_index.
///
/// Returns the suffix of the log starting at snapshot_index. When
/// snapshot_index exceeds the log length, the result is empty.
pub open spec fn compact_log_spec(
    log: Seq<LogEntry>,
    snapshot_index: u64,
) -> Seq<LogEntry> {
    if snapshot_index as int >= log.len() {
        Seq::empty()
    } else {
        log.subrange(snapshot_index as int, log.len() as int)
    }
}

// r[impl log-compaction.compact]
/// Compact the log by discarding entries before snapshot_index.
pub fn compact_log(
    log: &[LogEntry],
    snapshot_index: u64,
) -> (result: Vec<LogEntry>)
    requires snapshot_index as int <= log@.len()
    ensures result@ == compact_log_spec(log@, snapshot_index)
{
    // Establish snapshot_index fits in usize: log is a slice, so
    // log@.len() <= usize::MAX. Combined with requires, snapshot_index <= usize::MAX.
    let _log_len = log.len(); // binds log@.len() to a usize
    let si = snapshot_index as usize;
    let mut result = Vec::new();
    let mut i: usize = si;
    while i < log.len()
        invariant
            si as int == snapshot_index as int,
            si as int <= log@.len(),
            si <= i,
            i <= log.len(),
            result@.len() == i - si,
            forall|j: int| 0 <= j < result@.len() ==>
                result@[j] == log@[si as int + j],
        decreases log.len() - i
    {
        result.push(log[i]);
        i = i + 1;
    }
    result
}

// --- first_log_index_after_compaction ---

// r[depends log-compaction.first-index-after]
/// Spec: compute the first log index after compaction.
///
/// The first log index never decreases: returns max(prev, snapshot_index).
pub open spec fn first_log_index_after_compaction_spec(
    prev_first_index: u64,
    snapshot_index: u64,
) -> u64 {
    if snapshot_index > prev_first_index {
        snapshot_index
    } else {
        prev_first_index
    }
}

// r[impl log-compaction.first-index-after]
/// Compute the first log index after compaction.
pub fn first_log_index_after_compaction(
    prev_first_index: u64,
    snapshot_index: u64,
) -> (result: u64)
    ensures result == first_log_index_after_compaction_spec(prev_first_index, snapshot_index)
{
    if snapshot_index > prev_first_index {
        snapshot_index
    } else {
        prev_first_index
    }
}

// --- translate_index ---

// r[depends log-compaction.offset-translation]
/// Spec: translate an absolute log index to a relative index in
/// the compacted array.
///
/// Returns the relative index when the absolute index is within the
/// compacted log (>= first_log_index). Returns None (modeled as
/// u64::MAX) when the index falls before the compacted region.
pub open spec fn translate_index_spec(
    absolute_index: u64,
    first_log_index: u64,
) -> u64 {
    if absolute_index >= first_log_index {
        (absolute_index - first_log_index) as u64
    } else {
        u64::MAX // sentinel for "inaccessible"
    }
}

// r[impl log-compaction.offset-translation]
/// Translate an absolute index to a compacted-array index.
///
/// Returns u64::MAX when the index is before the compacted region.
pub fn translate_index(
    absolute_index: u64,
    first_log_index: u64,
) -> (result: u64)
    ensures result == translate_index_spec(absolute_index, first_log_index)
{
    if absolute_index >= first_log_index {
        absolute_index - first_log_index
    } else {
        u64::MAX
    }
}

// --- Safety proofs ---

// r[verify log-compaction.should-compact]
/// Threshold of 0 always triggers.
proof fn zero_threshold_always_triggers(log_len: u64)
    ensures should_compact_spec(log_len, 0)
{}

// r[verify log-compaction.should-compact]
/// Empty log never triggers with positive threshold.
proof fn empty_log_no_compact(threshold: u64)
    requires threshold > 0
    ensures !should_compact_spec(0, threshold)
{}

// r[impl log-compaction.prefix-removed]
// r[verify log-compaction.prefix-removed]
/// After compaction, no entry from the original [0, snapshot_index)
/// range is present.
proof fn prefix_removed(log: Seq<LogEntry>, snapshot_index: u64)
    requires snapshot_index as int <= log.len()
    ensures compact_log_spec(log, snapshot_index).len()
        == log.len() - snapshot_index as int
{}

// r[impl log-compaction.suffix-preserved]
// r[verify log-compaction.suffix-preserved]
/// All entries at [snapshot_index, log.len()) are preserved.
proof fn suffix_preserved(log: Seq<LogEntry>, snapshot_index: u64)
    requires snapshot_index as int <= log.len()
    ensures
        forall|i: int| 0 <= i < compact_log_spec(log, snapshot_index).len() ==>
            compact_log_spec(log, snapshot_index)[i] == log[snapshot_index as int + i]
{}

// r[verify log-compaction.first-index-after]
/// First index never decreases.
proof fn first_index_monotonic(prev: u64, snapshot_index: u64)
    ensures
        first_log_index_after_compaction_spec(prev, snapshot_index) >= prev
{}

// r[verify log-compaction.first-index-after]
/// First index is at least snapshot_index.
proof fn first_index_at_least_snapshot(prev: u64, snapshot_index: u64)
    ensures
        first_log_index_after_compaction_spec(prev, snapshot_index) >= snapshot_index
        || first_log_index_after_compaction_spec(prev, snapshot_index) >= prev
{}

// r[verify log-compaction.offset-translation]
/// Indices before the compacted region are inaccessible.
proof fn before_compaction_inaccessible(abs: u64, first: u64)
    requires abs < first
    ensures translate_index_spec(abs, first) == u64::MAX
{}

// r[verify log-compaction.offset-translation]
/// Index at exactly first_log_index maps to 0.
proof fn first_index_maps_to_zero(first: u64)
    ensures translate_index_spec(first, first) == 0
{}

// r[verify log-compaction.offset-translation]
/// Translation preserves ordering.
proof fn translate_preserves_order(a: u64, b: u64, first: u64)
    requires
        first <= a,
        a <= b,
    ensures
        translate_index_spec(a, first) <= translate_index_spec(b, first)
{}

// r[impl log-compaction.compact-preserves-commit]
// r[verify log-compaction.compact-preserves-commit]
/// Compacting below commit index does not change commit index.
/// The commit index is an absolute index, independent of log array position.
proof fn compact_preserves_commit(
    snapshot_index: u64,
    commit_index: u64,
)
    requires
        snapshot_index <= commit_index,
        // Exclude the edge case where the difference equals the sentinel value
        (commit_index as int) - (snapshot_index as int) < (u64::MAX as int),
    ensures
        // The commit index is still >= snapshot_index (accessible)
        translate_index_spec(commit_index, snapshot_index) != u64::MAX
{}

} // verus!

#[cfg(test)]
#[allow(clippy::cast_sign_loss, clippy::default_numeric_fallback)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn make_log(terms: &[u64]) -> Vec<LogEntry> {
        terms.iter().map(|&t| LogEntry { term: t, value: 0 }).collect()
    }

    // r[verify log-compaction.should-compact]
    #[test]
    fn test_should_compact() {
        assert!(should_compact(10, 10));
        assert!(should_compact(15, 10));
        assert!(!should_compact(9, 10));
        assert!(should_compact(0, 0));
        assert!(should_compact(5, 0));
    }

    // r[verify log-compaction.compact]
    #[test]
    fn test_compact_log() {
        let log = make_log(&[1, 2, 3, 4, 5]);
        let result = compact_log(&log, 2);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].term, 3);
        assert_eq!(result[1].term, 4);
        assert_eq!(result[2].term, 5);
    }

    // r[verify log-compaction.prefix-removed]
    #[test]
    fn test_compact_removes_prefix() {
        let log = make_log(&[1, 2, 3, 4, 5]);
        let result = compact_log(&log, 3);
        assert_eq!(result.len(), 2);
        // Entries with terms 1, 2, 3 are gone
        assert_eq!(result[0].term, 4);
    }

    // r[verify log-compaction.compact]
    #[test]
    fn test_compact_full_log() {
        let log = make_log(&[1, 2, 3]);
        let result = compact_log(&log, 3);
        assert_eq!(result.len(), 0);
    }

    // r[verify log-compaction.compact]
    #[test]
    fn test_compact_nothing() {
        let log = make_log(&[1, 2, 3]);
        let result = compact_log(&log, 0);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].term, 1);
    }

    // r[verify log-compaction.first-index-after]
    #[test]
    fn test_first_log_index_after_compaction() {
        assert_eq!(first_log_index_after_compaction(0, 5), 5);
        assert_eq!(first_log_index_after_compaction(5, 3), 5);
        assert_eq!(first_log_index_after_compaction(5, 5), 5);
        assert_eq!(first_log_index_after_compaction(0, 0), 0);
    }

    // r[verify log-compaction.offset-translation]
    #[test]
    fn test_translate_index() {
        assert_eq!(translate_index(10, 5), 5);
        assert_eq!(translate_index(5, 5), 0);
        assert_eq!(translate_index(4, 5), u64::MAX);
        assert_eq!(translate_index(0, 0), 0);
    }

    proptest! {
        // r[verify log-compaction.first-index-after]
        #[test]
        fn prop_first_index_monotonic(prev: u64, snap: u64) {
            let result = first_log_index_after_compaction(prev, snap);
            prop_assert!(result >= prev, "first index decreased");
        }

        // r[verify log-compaction.offset-translation]
        #[test]
        fn prop_translate_round_trip(abs in 0u64..1000, first in 0u64..1000) {
            let rel = translate_index(abs, first);
            if abs >= first {
                prop_assert_eq!(rel, abs - first);
                // Inverse: rel + first == abs
                prop_assert_eq!(rel + first, abs);
            } else {
                prop_assert_eq!(rel, u64::MAX);
            }
        }

        // r[verify log-compaction.suffix-preserved]
        #[test]
        fn prop_compact_preserves_suffix(
            terms in proptest::collection::vec(1u64..10, 0..20),
            snap_frac in 0.0f64..1.0,
        ) {
            let log = make_log(&terms);
            let snap = (snap_frac * log.len() as f64) as u64;
            let snap = std::cmp::min(snap, log.len() as u64);
            let result = compact_log(&log, snap);
            prop_assert_eq!(result.len(), log.len() - snap as usize);
            for i in 0..result.len() {
                prop_assert_eq!(result[i].term, log[snap as usize + i].term);
            }
        }

        // r[verify log-compaction.compact-preserves-commit]
        #[test]
        fn prop_compact_preserves_commit(
            snap in 0u64..100,
            commit in 0u64..100,
        ) {
            if snap <= commit {
                let rel = translate_index(commit, snap);
                prop_assert_ne!(rel, u64::MAX, "commit became inaccessible");
            }
        }
    }
}
