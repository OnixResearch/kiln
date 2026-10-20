//! Verified no-op entry for Raft leader commitment.
//!
//! After winning an election, a new leader appends a no-op entry for its
//! current term. Once the no-op is committed (replicated to a quorum),
//! all preceding entries are also committed. Without this, entries from
//! prior terms may remain uncommitted indefinitely.

use vstd::prelude::*;

use crate::log::*;

verus! {

/// Distinguished value marking a no-op entry.
pub const NOOP_VALUE: u64 = 0u64;

// --- make_noop_entry ---

// r[depends noop-commit.make-noop]
/// Spec: create a no-op entry for the given term.
pub open spec fn make_noop_entry_spec(current_term: u64) -> LogEntry {
    LogEntry { term: current_term, value: NOOP_VALUE }
}

// r[impl noop-commit.make-noop]
/// Create a no-op log entry for the current term.
pub fn make_noop_entry(current_term: u64) -> (result: LogEntry)
    ensures
        result.term == current_term,
        result.value == NOOP_VALUE,
        result == make_noop_entry_spec(current_term),
{
    LogEntry { term: current_term, value: NOOP_VALUE }
}

// --- is_noop ---

// r[depends noop-commit.is-noop]
/// Spec: is this entry a no-op?
pub open spec fn is_noop_spec(entry: LogEntry) -> bool {
    entry.value == NOOP_VALUE
}

// r[impl noop-commit.is-noop]
/// Check if an entry is a no-op.
pub fn is_noop(entry: &LogEntry) -> (result: bool)
    ensures result == is_noop_spec(*entry)
{
    entry.value == NOOP_VALUE
}

// --- noop_commit_index ---

// r[depends noop-commit.commit-index]
/// Spec: the commit index after appending and committing a no-op.
///
/// If the no-op at log.len() is committed, commit_index advances to
/// log.len() (the no-op's 1-based index), which means all entries
/// at [0, log.len()) — including all prior-term entries — are committed.
pub open spec fn noop_commit_index_spec(log_len_before_noop: int) -> int {
    log_len_before_noop + 1
}

// r[impl noop-commit.commit-index]
/// Compute the commit index after the no-op is committed.
pub fn noop_commit_index(log_len_before_noop: u64) -> (result: u64)
    requires log_len_before_noop < u64::MAX
    ensures result as int == noop_commit_index_spec(log_len_before_noop as int)
{
    log_len_before_noop + 1
}

// --- Safety proofs ---

// r[verify noop-commit.make-noop]
/// No-op entry has the correct term.
proof fn noop_has_correct_term(current_term: u64)
    ensures make_noop_entry_spec(current_term).term == current_term
{}

// r[verify noop-commit.make-noop]
/// No-op entry is recognized as a no-op.
proof fn noop_is_noop(current_term: u64)
    ensures is_noop_spec(make_noop_entry_spec(current_term))
{}

// r[verify noop-commit.commit-index]
/// Committing the no-op implies all prior entries are committed.
/// If commit_index == noop_commit_index(old_len), then for all
/// i in [0, old_len), entry i is committed (i < commit_index).
proof fn noop_commits_prior(log_len_before_noop: int, i: int)
    requires
        log_len_before_noop >= 0,
        0 <= i,
        i < log_len_before_noop,
    ensures
        i < noop_commit_index_spec(log_len_before_noop)
{}

// r[verify noop-commit.commit-index]
/// The no-op itself is at the commit boundary.
proof fn noop_at_commit_boundary(log_len_before_noop: int)
    requires log_len_before_noop >= 0
    ensures
        log_len_before_noop < noop_commit_index_spec(log_len_before_noop)
{}

// r[verify noop-commit.make-noop]
/// Appending a no-op preserves prior log entries (delegates to log module).
proof fn append_noop_preserves_prefix(log: Seq<LogEntry>, current_term: u64)
    ensures
        forall|i: int| #![trigger log.push(make_noop_entry_spec(current_term))[i]]
            0 <= i < log.len()
            ==> log.push(make_noop_entry_spec(current_term))[i] == log[i]
{}

} // verus!

#[cfg(test)]
#[allow(clippy::cast_possible_truncation)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn entry(term: u64, value: u64) -> LogEntry {
        LogEntry { term, value }
    }

    // r[verify noop-commit.make-noop]
    // r[verify noop-commit.is-noop]
    #[test]
    fn test_make_noop() {
        let noop = make_noop_entry(5);
        assert_eq!(noop.term, 5);
        assert_eq!(noop.value, NOOP_VALUE);
        assert!(is_noop(&noop));
    }

    // r[verify noop-commit.make-noop]
    #[test]
    fn test_is_noop() {
        assert!(is_noop(&entry(5, NOOP_VALUE)));
        assert!(!is_noop(&entry(5, 42)));
    }

    // r[verify noop-commit.commit-index]
    #[test]
    fn test_noop_commit_index() {
        assert_eq!(noop_commit_index(0), 1);
        assert_eq!(noop_commit_index(5), 6);
        assert_eq!(noop_commit_index(10), 11);
    }

    // r[verify noop-commit.commit-index]
    #[test]
    fn test_noop_commits_prior_entries() {
        // Log has 3 entries, then we append a no-op
        let mut log = vec![entry(1, 10), entry(2, 20), entry(3, 30)];
        let noop = make_noop_entry(4);
        log.push(noop);
        let commit_idx = noop_commit_index(3);
        assert_eq!(commit_idx, 4);
        // All prior entries (indices 0, 1, 2) are below the commit index
        for i in 0..3 {
            assert!(i < commit_idx as usize);
        }
    }

    proptest! {
        // r[verify noop-commit.make-noop]
        #[test]
        fn prop_noop_has_correct_term(term: u64) {
            let noop = make_noop_entry(term);
            prop_assert_eq!(noop.term, term);
            prop_assert!(is_noop(&noop));
        }

        // r[verify noop-commit.commit-index]
        #[test]
        fn prop_noop_commits_all_prior(log_len in 0u64..10000) {
            let commit_idx = noop_commit_index(log_len);
            for i in 0..log_len {
                prop_assert!(i < commit_idx);
            }
        }

        #[test]
        fn prop_noop_commit_index_is_len_plus_one(log_len in 0u64..10000) {
            prop_assert_eq!(noop_commit_index(log_len), log_len + 1);
        }

        #[test]
        fn prop_noop_roundtrip(term: u64) {
            let entry = make_noop_entry(term);
            prop_assert!(is_noop(&entry));
            prop_assert_eq!(entry.term, term);
            prop_assert_eq!(entry.value, NOOP_VALUE);
        }

        #[test]
        fn prop_non_noop_rejected(term: u64, value in 1u64..u64::MAX) {
            let entry = LogEntry { term, value };
            prop_assert!(!is_noop(&entry));
        }
    }
}
