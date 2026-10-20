// Verus preconditions guarantee u64→usize casts are in bounds
#![allow(clippy::cast_possible_truncation)]

//! Verified AppendEntries consistency check.
//!
//! Follower-side validation for Raft's AppendEntries RPC: before accepting
//! entries from a leader, the follower checks that its log at prevLogIndex
//! has the expected term. This is the runtime enforcement of the Log
//! Matching Property — if the check passes, the follower's log prefix
//! matches the leader's.

use vstd::prelude::*;

use crate::log::*;

verus! {

// --- check_append_entries ---

// r[depends append-entries.check]
/// Spec: does the follower's log satisfy the AppendEntries consistency check?
///
/// Returns true when:
/// 1. prev_log_index == 0 (empty prefix — first AppendEntries), OR
/// 2. The log has an entry at prev_log_index - 1 whose term equals prev_log_term.
pub open spec fn check_append_entries_spec(
    log: Seq<LogEntry>,
    prev_log_index: u64,
    prev_log_term: u64,
) -> bool {
    if prev_log_index == 0 {
        true
    } else {
        let idx = (prev_log_index - 1) as int;
        idx < log.len() && log[idx].term == prev_log_term
    }
}

// r[impl append-entries.check]
/// Check AppendEntries consistency: log[prevLogIndex-1].term == prevLogTerm.
pub fn check_append_entries(
    log: &[LogEntry],
    prev_log_index: u64,
    prev_log_term: u64,
) -> (result: bool)
    requires prev_log_index as int <= usize::MAX as int
    ensures result == check_append_entries_spec(log@, prev_log_index, prev_log_term)
{
    if prev_log_index == 0 {
        true
    } else {
        let idx: usize = (prev_log_index - 1) as usize;
        if idx < log.len() {
            log[idx].term == prev_log_term
        } else {
            false
        }
    }
}

// --- Safety proofs ---

// r[verify append-entries.check]
/// Empty prefix always passes.
proof fn empty_prefix_passes(log: Seq<LogEntry>)
    ensures check_append_entries_spec(log, 0, 0)
{}

// r[verify append-entries.check]
/// Empty prefix passes regardless of prev_log_term.
proof fn empty_prefix_passes_any_term(log: Seq<LogEntry>, term: u64)
    ensures check_append_entries_spec(log, 0, term)
{}

// r[verify append-entries.check]
/// Log too short implies rejection.
proof fn short_log_rejects(log: Seq<LogEntry>, prev_log_index: u64, prev_log_term: u64)
    requires
        prev_log_index > 0,
        ((prev_log_index - 1) as int) >= log.len(),
    ensures !check_append_entries_spec(log, prev_log_index, prev_log_term)
{}

// r[verify append-entries.check]
/// Term mismatch implies rejection.
proof fn term_mismatch_rejects(
    log: Seq<LogEntry>,
    prev_log_index: u64,
    prev_log_term: u64,
)
    requires
        prev_log_index > 0,
        ((prev_log_index - 1) as int) < log.len(),
        log[(prev_log_index - 1) as int].term != prev_log_term,
    ensures !check_append_entries_spec(log, prev_log_index, prev_log_term)
{}

// r[verify append-entries.check]
/// Matching entry implies acceptance.
proof fn matching_entry_passes(
    log: Seq<LogEntry>,
    prev_log_index: u64,
    prev_log_term: u64,
)
    requires
        prev_log_index > 0,
        ((prev_log_index - 1) as int) < log.len(),
        log[(prev_log_index - 1) as int].term == prev_log_term,
    ensures check_append_entries_spec(log, prev_log_index, prev_log_term)
{}

// r[verify append-entries.check]
/// Consistency check links to log agreement: if both logs pass the check
/// for the same (prevLogIndex, prevLogTerm), they agree at prevLogIndex-1.
proof fn consistency_implies_agreement(
    log_a: Seq<LogEntry>,
    log_b: Seq<LogEntry>,
    prev_log_index: u64,
    prev_log_term: u64,
)
    requires
        prev_log_index > 0,
        check_append_entries_spec(log_a, prev_log_index, prev_log_term),
        check_append_entries_spec(log_b, prev_log_index, prev_log_term),
    ensures
        agree_at_spec(log_a, log_b, (prev_log_index - 1) as int)
{}

// r[verify append-entries.check]
/// The check is decidable: for any inputs, it returns true or false.
proof fn check_decidable(log: Seq<LogEntry>, prev_log_index: u64, prev_log_term: u64)
    ensures
        check_append_entries_spec(log, prev_log_index, prev_log_term)
        || !check_append_entries_spec(log, prev_log_index, prev_log_term)
{}

// r[verify append-entries.check]
/// Rejection means divergence: if the check fails with prev_log_index > 0,
/// either the log is too short or the entry has a different term.
proof fn rejection_means_divergence(
    log: Seq<LogEntry>,
    prev_log_index: u64,
    prev_log_term: u64,
)
    requires
        prev_log_index > 0,
        !check_append_entries_spec(log, prev_log_index, prev_log_term),
    ensures
        ((prev_log_index - 1) as int) >= log.len()
        || log[(prev_log_index - 1) as int].term != prev_log_term
{}

// r[verify append-entries.check]
/// If the check passes at index k, the log has at least k entries.
proof fn check_implies_length(
    log: Seq<LogEntry>,
    prev_log_index: u64,
    prev_log_term: u64,
)
    requires
        prev_log_index > 0,
        check_append_entries_spec(log, prev_log_index, prev_log_term),
    ensures
        log.len() >= prev_log_index as int
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn entry(term: u64, value: u64) -> LogEntry {
        LogEntry { term, value }
    }

    // r[verify append-entries.check]
    #[test]
    fn test_empty_prefix() {
        let log: Vec<LogEntry> = vec![];
        assert!(check_append_entries(&log, 0, 0));

        let log = vec![entry(1, 10)];
        assert!(check_append_entries(&log, 0, 0));
        assert!(check_append_entries(&log, 0, 99));
    }

    // r[verify append-entries.check]
    #[test]
    fn test_matching_entry() {
        let log = vec![entry(1, 10), entry(2, 20), entry(3, 30)];
        assert!(check_append_entries(&log, 1, 1)); // log[0].term == 1
        assert!(check_append_entries(&log, 2, 2)); // log[1].term == 2
        assert!(check_append_entries(&log, 3, 3)); // log[2].term == 3
    }

    // r[verify append-entries.check]
    #[test]
    fn test_term_mismatch() {
        let log = vec![entry(1, 10), entry(2, 20), entry(3, 30)];
        assert!(!check_append_entries(&log, 2, 5)); // log[1].term == 2 ≠ 5
        assert!(!check_append_entries(&log, 1, 9)); // log[0].term == 1 ≠ 9
    }

    // r[verify append-entries.check]
    #[test]
    fn test_log_too_short() {
        let log = vec![entry(1, 10), entry(2, 20)];
        assert!(!check_append_entries(&log, 5, 1)); // log has 2 entries, need index 4
        assert!(!check_append_entries(&log, 3, 1)); // log has 2 entries, need index 2
    }

    // r[verify append-entries.check]
    #[test]
    fn test_consistency_agreement() {
        let log_a = vec![entry(1, 10), entry(2, 20), entry(3, 30)];
        let log_b = vec![entry(1, 99), entry(2, 88), entry(3, 77)];
        // Both pass check at prev_log_index=2, prev_log_term=2
        assert!(check_append_entries(&log_a, 2, 2));
        assert!(check_append_entries(&log_b, 2, 2));
        // They agree at index 1 (both have term 2)
        assert!(agree_at(&log_a, &log_b, 1));
    }

    proptest! {
        // r[verify append-entries.check]
        #[test]
        fn prop_empty_prefix_always_passes(
            terms in proptest::collection::vec(1u64..10, 0..=20),
            term: u64,
        ) {
            let log: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 0)).collect();
            prop_assert!(check_append_entries(&log, 0, term));
        }

        // r[verify append-entries.check]
        #[test]
        fn prop_matching_entry_passes(
            terms in proptest::collection::vec(1u64..10, 1..=20),
        ) {
            let log: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 0)).collect();
            for (i, &t) in terms.iter().enumerate() {
                let prev_log_index = (i + 1) as u64;
                prop_assert!(check_append_entries(&log, prev_log_index, t));
            }
        }

        // r[verify append-entries.check]
        #[test]
        fn prop_short_log_rejects(
            terms in proptest::collection::vec(1u64..10, 0..=10),
            extra in 1u64..100,
        ) {
            let log: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 0)).collect();
            let prev_log_index = log.len() as u64 + extra;
            prop_assert!(!check_append_entries(&log, prev_log_index, 1));
        }

        // r[verify append-entries.check]
        #[test]
        fn prop_check_implies_length(
            terms in proptest::collection::vec(1u64..10, 1..=20),
            idx_offset in 0usize..20,
        ) {
            let log: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 0)).collect();
            let idx = idx_offset % log.len();
            let prev_log_index = (idx + 1) as u64;
            let prev_log_term = terms[idx];
            if check_append_entries(&log, prev_log_index, prev_log_term) {
                prop_assert!(log.len() >= prev_log_index as usize);
            }
        }

        // r[verify append-entries.check]
        #[test]
        fn prop_consistency_implies_agreement(
            terms in proptest::collection::vec(1u64..10, 1..=20),
        ) {
            // Build two logs with same terms but different values
            let log_a: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 0)).collect();
            let log_b: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 1)).collect();
            for (i, &t) in terms.iter().enumerate() {
                let prev_log_index = (i + 1) as u64;
                prop_assert!(check_append_entries(&log_a, prev_log_index, t));
                prop_assert!(check_append_entries(&log_b, prev_log_index, t));
                prop_assert!(agree_at(&log_a, &log_b, i));
            }
        }
    }
}
