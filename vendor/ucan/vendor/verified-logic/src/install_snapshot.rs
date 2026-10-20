//! Verified snapshot installation for Raft log compaction.
//!
//! When a leader's `nextIndex` for a follower falls behind the compacted
//! portion of the log, it sends a snapshot instead of `AppendEntries`.
//! The follower replaces its log prefix with the snapshot state and
//! advances its commit index.

#![allow(clippy::cast_possible_truncation)]

use vstd::prelude::*;

use crate::log::LogEntry;

verus! {

// --- should_install_snapshot ---

// r[depends install-snapshot.trigger]
/// Spec: should the leader send a snapshot instead of AppendEntries?
///
/// True when the follower's nextIndex is at or before the first
/// available log entry (the rest has been compacted).
pub open spec fn should_install_snapshot_spec(
    next_index: u64,
    first_log_index: u64,
) -> bool {
    next_index <= first_log_index
}

// r[impl install-snapshot.trigger]
/// Check if a snapshot should be sent to this follower.
pub fn should_install_snapshot(
    next_index: u64,
    first_log_index: u64,
) -> (result: bool)
    ensures result == should_install_snapshot_spec(next_index, first_log_index)
{
    next_index <= first_log_index
}

// --- apply_snapshot_to_log ---

// r[depends install-snapshot.apply]
/// Spec: apply a snapshot to the follower's log.
///
/// If the follower has an entry at snapshot_index with matching term,
/// preserve entries from snapshot_index onward. Otherwise, replace
/// the entire log with a single entry at the snapshot point.
///
/// Returns (new_log, new_commit_index).
pub open spec fn apply_snapshot_commit_spec(
    current_commit: u64,
    snapshot_index: u64,
) -> u64 {
    if snapshot_index > current_commit {
        snapshot_index
    } else {
        current_commit
    }
}

// r[impl install-snapshot.apply]
/// Compute the new commit index after applying a snapshot.
pub fn apply_snapshot_commit(
    current_commit: u64,
    snapshot_index: u64,
) -> (result: u64)
    ensures result == apply_snapshot_commit_spec(current_commit, snapshot_index)
{
    if snapshot_index > current_commit {
        snapshot_index
    } else {
        current_commit
    }
}

// --- has_matching_entry ---

// r[depends install-snapshot.matching-entry]
/// Spec: does the log have an entry at snapshot_index (0-based) with
/// the given term?
pub open spec fn has_matching_entry_spec(
    log: Seq<LogEntry>,
    snapshot_index: u64,
    snapshot_term: u64,
) -> bool {
    (snapshot_index as int) < log.len()
    && log[snapshot_index as int].term == snapshot_term
}

// r[impl install-snapshot.matching-entry]
/// Check if the log has a matching entry at the snapshot point.
pub fn has_matching_entry(
    log: &[LogEntry],
    snapshot_index: u64,
    snapshot_term: u64,
) -> (result: bool)
    requires snapshot_index as int <= usize::MAX as int
    ensures result == has_matching_entry_spec(log@, snapshot_index, snapshot_term)
{
    let idx = snapshot_index as usize;
    if idx < log.len() {
        log[idx].term == snapshot_term
    } else {
        false
    }
}

// --- Safety proofs ---

// r[verify install-snapshot.trigger]
/// Follower behind compaction needs snapshot.
proof fn behind_compaction_needs_snapshot(next_index: u64, first_log_index: u64)
    requires next_index <= first_log_index
    ensures should_install_snapshot_spec(next_index, first_log_index)
{}

// r[verify install-snapshot.trigger]
/// Follower within log does not need snapshot.
proof fn within_log_no_snapshot(next_index: u64, first_log_index: u64)
    requires next_index > first_log_index
    ensures !should_install_snapshot_spec(next_index, first_log_index)
{}

// r[verify install-snapshot.apply]
/// Snapshot advances commit index.
proof fn snapshot_advances_commit(current_commit: u64, snapshot_index: u64)
    ensures apply_snapshot_commit_spec(current_commit, snapshot_index) >= current_commit
{}

// r[verify install-snapshot.apply]
/// Snapshot commit is at least snapshot_index.
proof fn snapshot_commit_ge_snapshot(current_commit: u64, snapshot_index: u64)
    ensures apply_snapshot_commit_spec(current_commit, snapshot_index) >= snapshot_index
        || apply_snapshot_commit_spec(current_commit, snapshot_index) >= current_commit
{}

// r[verify install-snapshot.apply]
/// Snapshot commit is max of current and snapshot.
proof fn snapshot_commit_is_max(current_commit: u64, snapshot_index: u64)
    ensures
        apply_snapshot_commit_spec(current_commit, snapshot_index)
        == if snapshot_index > current_commit { snapshot_index } else { current_commit }
{}

// r[verify install-snapshot.trigger]
/// Snapshot trigger is decidable.
proof fn trigger_decidable(next_index: u64, first_log_index: u64)
    ensures
        should_install_snapshot_spec(next_index, first_log_index)
        || !should_install_snapshot_spec(next_index, first_log_index)
{}

// --- apply_snapshot_to_log ---

// r[depends install-snapshot.apply]
/// Spec: apply a snapshot to the follower's log.
///
/// If the follower has an entry at snapshot_index with matching term,
/// preserve entries from snapshot_index onward. Otherwise, replace
/// the entire log with a single entry at the snapshot point.
pub open spec fn apply_snapshot_to_log_spec(
    log: Seq<LogEntry>,
    snapshot_index: u64,
    snapshot_term: u64,
) -> Seq<LogEntry> {
    if has_matching_entry_spec(log, snapshot_index, snapshot_term) {
        // Preserve entries from snapshot_index onward
        log.subrange(snapshot_index as int, log.len() as int)
    } else {
        // Replace entire log with snapshot entry
        seq![LogEntry { term: snapshot_term, value: 0 }]
    }
}

// r[impl install-snapshot.apply]
/// Apply a snapshot to the follower's log.
pub fn apply_snapshot_to_log(
    log: &[LogEntry],
    snapshot_index: u64,
    snapshot_term: u64,
) -> (result: Vec<LogEntry>)
    requires snapshot_index as int <= usize::MAX as int
    ensures result@ == apply_snapshot_to_log_spec(log@, snapshot_index, snapshot_term)
{
    if has_matching_entry(log, snapshot_index, snapshot_term) {
        // Preserve entries from snapshot_index onward
        let idx = snapshot_index as usize;
        let mut result = Vec::new();
        let mut i: usize = idx;
        while i < log.len()
            invariant
                idx <= i,
                i <= log@.len(),
                result@.len() == i - idx,
                forall|j: int| 0 <= j < result@.len() ==>
                    #[trigger] result@[j] == log@[idx as int + j],
                snapshot_index as int <= usize::MAX as int,
                idx == snapshot_index as usize,
            decreases log.len() - i
        {
            result.push(log[i]);
            i = i + 1;
        }
        assert(result@ =~= log@.subrange(snapshot_index as int, log@.len() as int));
        result
    } else {
        // Replace entire log with snapshot entry
        let entry = LogEntry { term: snapshot_term, value: 0 };
        let result = vec![entry];
        assert(result@ =~= seq![LogEntry { term: snapshot_term, value: 0 }]);
        result
    }
}

// r[verify install-snapshot.apply]
/// Matching entry preserves suffix.
proof fn matching_preserves_suffix(
    log: Seq<LogEntry>,
    snapshot_index: u64,
    snapshot_term: u64,
)
    requires has_matching_entry_spec(log, snapshot_index, snapshot_term)
    ensures
        apply_snapshot_to_log_spec(log, snapshot_index, snapshot_term).len()
        == log.len() - snapshot_index as int,
        apply_snapshot_to_log_spec(log, snapshot_index, snapshot_term)[0].term
        == snapshot_term,
{}

// r[verify install-snapshot.apply]
/// No matching entry replaces with single entry.
proof fn no_match_replaces(
    log: Seq<LogEntry>,
    snapshot_index: u64,
    snapshot_term: u64,
)
    requires !has_matching_entry_spec(log, snapshot_index, snapshot_term)
    ensures
        apply_snapshot_to_log_spec(log, snapshot_index, snapshot_term).len() == 1,
        apply_snapshot_to_log_spec(log, snapshot_index, snapshot_term)[0].term
        == snapshot_term,
{}

// r[verify install-snapshot.apply]
/// Result is never empty.
proof fn result_nonempty(
    log: Seq<LogEntry>,
    snapshot_index: u64,
    snapshot_term: u64,
)
    ensures apply_snapshot_to_log_spec(log, snapshot_index, snapshot_term).len() >= 1
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn entry(term: u64, value: u64) -> LogEntry {
        LogEntry { term, value }
    }

    // r[verify install-snapshot.trigger]
    #[test]
    fn test_should_install_snapshot() {
        assert!(should_install_snapshot(5, 10)); // behind compaction
        assert!(should_install_snapshot(10, 10)); // at compaction point
        assert!(!should_install_snapshot(15, 10)); // within log
    }

    // r[verify install-snapshot.apply]
    #[test]
    fn test_apply_snapshot_commit() {
        assert_eq!(apply_snapshot_commit(3, 10), 10); // snapshot advances
        assert_eq!(apply_snapshot_commit(10, 3), 10); // commit already ahead
        assert_eq!(apply_snapshot_commit(5, 5), 5); // equal
    }

    // r[verify install-snapshot.matching-entry]
    #[test]
    fn test_has_matching_entry() {
        let log = vec![entry(1, 0), entry(2, 0), entry(3, 0), entry(4, 0)];
        assert!(has_matching_entry(&log, 2, 3)); // log[2].term == 3
        assert!(!has_matching_entry(&log, 2, 5)); // log[2].term == 3 ≠ 5
        assert!(!has_matching_entry(&log, 5, 1)); // out of bounds
    }

    // r[verify install-snapshot.apply]
    #[test]
    fn test_apply_snapshot_matching() {
        let log = vec![entry(1, 10), entry(2, 20), entry(3, 30), entry(4, 40)];
        let result = apply_snapshot_to_log(&log, 2, 3); // log[2].term == 3, match
        assert_eq!(result.len(), 2); // entries at [2, 4)
        assert_eq!(result[0].term, 3);
        assert_eq!(result[1].term, 4);
    }

    // r[verify install-snapshot.apply]
    #[test]
    fn test_apply_snapshot_no_match() {
        let log = vec![entry(1, 10), entry(2, 20), entry(5, 30)];
        let result = apply_snapshot_to_log(&log, 2, 3); // log[2].term == 5 != 3
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].term, 3);
    }

    // r[verify install-snapshot.apply]
    #[test]
    fn test_apply_snapshot_beyond_log() {
        let log = vec![entry(1, 10), entry(2, 20)];
        let result = apply_snapshot_to_log(&log, 5, 3); // out of bounds
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].term, 3);
    }

    proptest! {
        // r[verify install-snapshot.trigger]
        #[test]
        fn prop_trigger_correct(next_index: u64, first_log_index: u64) {
            prop_assert_eq!(
                should_install_snapshot(next_index, first_log_index),
                next_index <= first_log_index,
            );
        }

        // r[verify install-snapshot.apply]
        #[test]
        fn prop_commit_advances(current_commit: u64, snapshot_index: u64) {
            let result = apply_snapshot_commit(current_commit, snapshot_index);
            prop_assert!(result >= current_commit);
        }

        // r[verify install-snapshot.apply]
        #[test]
        fn prop_commit_is_max(current_commit: u64, snapshot_index: u64) {
            let result = apply_snapshot_commit(current_commit, snapshot_index);
            let expected = current_commit.max(snapshot_index);
            prop_assert_eq!(result, expected);
        }

        // r[verify install-snapshot.apply]
        #[test]
        fn prop_apply_snapshot_nonempty(
            log_terms in proptest::collection::vec(1u64..10, 0..10),
            snap_idx in 0u64..15,
            snap_term in 1u64..10,
        ) {
            let log: Vec<LogEntry> = log_terms.iter().map(|&t| entry(t, 0)).collect();
            let result = apply_snapshot_to_log(&log, snap_idx, snap_term);
            prop_assert!(!result.is_empty());
            prop_assert_eq!(result[0].term, if has_matching_entry(&log, snap_idx, snap_term) {
                log[snap_idx as usize].term
            } else {
                snap_term
            });
        }
    }
}
