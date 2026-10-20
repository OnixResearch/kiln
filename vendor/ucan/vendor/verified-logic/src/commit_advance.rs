// Verus preconditions guarantee u64→usize casts are in bounds
#![allow(clippy::cast_possible_truncation)]

//! Verified commit index advancement for Raft.
//!
//! The leader advances its commit index to the largest N where:
//! 1. N > current_commit
//! 2. log[N-1].term == current_term (the §5.4.2 "Figure 8" guard)
//! 3. A quorum of servers have matchIndex >= N
//!
//! The current-term guard is the critical safety rule — without it, a
//! leader could "commit" entries from prior terms that another leader
//! might overwrite.

use vstd::prelude::*;

use crate::log::*;
use crate::match_index::*;
use crate::quorum::*;

verus! {

// --- can_commit_at ---

// r[depends commit-advance.term-guard]
/// Spec: can the entry at log index (1-based) be committed?
///
/// True when the entry's term matches the current leader term.
/// Index is 1-based: log[index-1].term == current_term.
pub open spec fn can_commit_at_spec(
    log: Seq<LogEntry>,
    index: u64,
    current_term: u64,
) -> bool {
    index > 0
    && ((index - 1) as int) < log.len()
    && log[(index - 1) as int].term == current_term
}

// r[impl commit-advance.term-guard]
/// Check if the entry at index (1-based) can be committed in the current term.
pub fn can_commit_at(
    log: &[LogEntry],
    index: u64,
    current_term: u64,
) -> (result: bool)
    requires index as int <= usize::MAX as int
    ensures result == can_commit_at_spec(log@, index, current_term)
{
    if index == 0 {
        false
    } else {
        let idx = (index - 1) as usize;
        if idx < log.len() {
            log[idx].term == current_term
        } else {
            false
        }
    }
}

// --- has_quorum_at ---

// r[depends commit-advance.quorum-check]
/// Spec: do a quorum of servers have matchIndex >= index?
pub open spec fn has_quorum_at_spec(
    match_indices: Seq<u64>,
    index: u64,
    cluster_size: u64,
) -> bool
    recommends cluster_size > 0
{
    is_quorum_spec(count_replicated_spec(match_indices, index) as u64, cluster_size)
}

// r[impl commit-advance.quorum-check]
/// Check if a quorum of servers have matchIndex >= index.
pub fn has_quorum_at(
    match_indices: &[u64],
    index: u64,
    cluster_size: u64,
) -> (result: bool)
    requires
        cluster_size > 0,
        match_indices@.len() <= u64::MAX as int,
    ensures result == has_quorum_at_spec(match_indices@, index, cluster_size)
{
    let count = count_replicated(match_indices, index);
    is_quorum(count, cluster_size)
}

// --- advance_commit_index ---

// r[depends commit-advance.advance]
/// Spec: find the largest committable index.
///
/// Scans from the end of the log downward. Returns the first index N
/// where can_commit_at AND has_quorum_at both hold, or current_commit
/// if no such index exists.
pub open spec fn advance_commit_index_spec(
    log: Seq<LogEntry>,
    match_indices: Seq<u64>,
    current_commit: u64,
    current_term: u64,
    cluster_size: u64,
) -> u64
    recommends cluster_size > 0
{
    advance_scan_spec(log, match_indices, current_commit, current_term, cluster_size, log.len() as int)
}

pub open spec fn advance_scan_spec(
    log: Seq<LogEntry>,
    match_indices: Seq<u64>,
    current_commit: u64,
    current_term: u64,
    cluster_size: u64,
    candidate: int,
) -> u64
    decreases candidate,
{
    if candidate <= current_commit as int || candidate <= 0 {
        current_commit
    } else if can_commit_at_spec(log, candidate as u64, current_term)
        && is_quorum_spec(count_replicated_spec(match_indices, candidate as u64) as u64, cluster_size)
    {
        candidate as u64
    } else {
        advance_scan_spec(log, match_indices, current_commit, current_term, cluster_size, candidate - 1)
    }
}

// r[impl commit-advance.advance]
/// Find the largest committable index, scanning backward from log end.
pub fn advance_commit_index(
    log: &[LogEntry],
    match_indices: &[u64],
    current_commit: u64,
    current_term: u64,
    cluster_size: u64,
) -> (result: u64)
    requires
        cluster_size > 0,
        match_indices@.len() <= u64::MAX as int,
        log@.len() <= u64::MAX as int,
        current_commit as int <= usize::MAX as int,
    ensures
        result == advance_commit_index_spec(
            log@, match_indices@, current_commit, current_term, cluster_size,
        ),
        result >= current_commit,
{
    let log_len = log.len();
    if log_len == 0 {
        return current_commit;
    }
    let mut n: usize = log_len;
    while n > current_commit as usize && n > 0
        invariant
            0 <= n <= log@.len(),
            cluster_size > 0,
            match_indices@.len() <= u64::MAX as int,
            log@.len() <= u64::MAX as int,
            current_commit as int <= usize::MAX as int,
            advance_commit_index_spec(
                log@, match_indices@, current_commit, current_term, cluster_size,
            ) == advance_scan_spec(
                log@, match_indices@, current_commit, current_term, cluster_size, n as int,
            ),
        decreases n,
    {
        let n_u64 = n as u64;
        if can_commit_at(log, n_u64, current_term)
            && has_quorum_at(match_indices, n_u64, cluster_size)
        {
            return n_u64;
        }
        n = n - 1;
    }
    current_commit
}

// --- Safety proofs ---

// r[verify commit-advance.term-guard]
/// Current term guard: prior-term entries are not directly committable.
proof fn prior_term_not_committable(
    log: Seq<LogEntry>,
    index: u64,
    current_term: u64,
)
    requires
        index > 0,
        ((index - 1) as int) < log.len(),
        log[(index - 1) as int].term != current_term,
    ensures !can_commit_at_spec(log, index, current_term)
{}

// r[verify commit-advance.term-guard]
/// Current term entry is committable (pending quorum check).
proof fn current_term_committable(
    log: Seq<LogEntry>,
    index: u64,
    current_term: u64,
)
    requires
        index > 0,
        ((index - 1) as int) < log.len(),
        log[(index - 1) as int].term == current_term,
    ensures can_commit_at_spec(log, index, current_term)
{}

// r[verify commit-advance.term-guard]
/// Index 0 is never committable.
proof fn zero_index_not_committable(log: Seq<LogEntry>, current_term: u64)
    ensures !can_commit_at_spec(log, 0, current_term)
{}

// r[verify commit-advance.advance]
/// Commit index never decreases.
proof fn advance_monotonic(
    log: Seq<LogEntry>,
    match_indices: Seq<u64>,
    current_commit: u64,
    current_term: u64,
    cluster_size: u64,
)
    requires
        cluster_size > 0,
        log.len() <= u64::MAX as int,
    ensures
        advance_commit_index_spec(
            log, match_indices, current_commit, current_term, cluster_size,
        ) >= current_commit
{
    advance_monotonic_scan(
        log, match_indices, current_commit, current_term, cluster_size, log.len() as int,
    );
}

proof fn advance_monotonic_scan(
    log: Seq<LogEntry>,
    match_indices: Seq<u64>,
    current_commit: u64,
    current_term: u64,
    cluster_size: u64,
    candidate: int,
)
    requires
        cluster_size > 0,
        candidate <= u64::MAX as int,
    ensures
        advance_scan_spec(
            log, match_indices, current_commit, current_term, cluster_size, candidate,
        ) >= current_commit
    decreases candidate,
{
    if candidate <= current_commit as int || candidate <= 0 {
        // Base: returns current_commit >= current_commit ✓
    } else {
        // candidate > current_commit as int && candidate > 0
        if can_commit_at_spec(log, candidate as u64, current_term)
            && is_quorum_spec(count_replicated_spec(match_indices, candidate as u64) as u64, cluster_size)
        {
            // Returns candidate as u64. candidate > 0 and candidate > current_commit.
            // Since candidate <= u64::MAX (it started as log.len() as int), candidate as u64 >= current_commit.
        } else {
            advance_monotonic_scan(
                log, match_indices, current_commit, current_term, cluster_size, candidate - 1,
            );
        }
    }
}

// r[verify commit-advance.advance]
/// Empty log means no advancement.
proof fn empty_log_no_advance(
    match_indices: Seq<u64>,
    current_commit: u64,
    current_term: u64,
    cluster_size: u64,
)
    requires cluster_size > 0
    ensures
        advance_commit_index_spec(
            Seq::empty(), match_indices, current_commit, current_term, cluster_size,
        ) == current_commit
{}

// r[verify commit-advance.quorum-check]
/// Quorum check delegates to quorum::is_quorum.
proof fn quorum_check_delegates(
    match_indices: Seq<u64>,
    index: u64,
    cluster_size: u64,
)
    requires cluster_size > 0
    ensures
        has_quorum_at_spec(match_indices, index, cluster_size)
        == is_quorum_spec(
            count_replicated_spec(match_indices, index) as u64,
            cluster_size,
        )
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn entry(term: u64, value: u64) -> LogEntry {
        LogEntry { term, value }
    }

    // r[verify commit-advance.term-guard]
    #[test]
    fn test_can_commit_at() {
        let log = vec![entry(1, 0), entry(2, 0), entry(3, 0)];
        // Current term entry
        assert!(can_commit_at(&log, 3, 3)); // log[2].term == 3
        assert!(can_commit_at(&log, 2, 2)); // log[1].term == 2
        // Prior term entry
        assert!(!can_commit_at(&log, 2, 3)); // log[1].term == 2 ≠ 3
        assert!(!can_commit_at(&log, 1, 4)); // log[0].term == 1 ≠ 4
        // Index 0
        assert!(!can_commit_at(&log, 0, 1));
        // Out of bounds
        assert!(!can_commit_at(&log, 4, 3));
    }

    // r[verify commit-advance.quorum-check]
    #[test]
    fn test_has_quorum_at() {
        let matches = vec![5, 3, 5, 2, 5];
        assert!(has_quorum_at(&matches, 5, 5)); // 3 out of 5
        assert!(!has_quorum_at(&matches, 5, 7)); // 3 out of 7 (majority is 4)
        assert!(has_quorum_at(&matches, 3, 5)); // 4 out of 5
        assert!(has_quorum_at(&matches, 1, 5)); // 5 out of 5
        assert!(!has_quorum_at(&matches, 6, 5)); // 0 out of 5
    }

    // r[verify commit-advance.advance]
    #[test]
    fn test_advance_commit_index() {
        let log = vec![entry(1, 0), entry(2, 0), entry(2, 0), entry(2, 0)];
        let matches = vec![4, 2, 3, 1, 4];
        // current_commit=1, current_term=2, cluster=5
        // Index 4: log[3].term=2 ✓, count(matchIndex>=4)=2 (servers 0,4) → not quorum
        // Index 3: log[2].term=2 ✓, count(matchIndex>=3)=3 (servers 0,2,4) → quorum!
        let result = advance_commit_index(&log, &matches, 1, 2, 5);
        assert_eq!(result, 3);
    }

    // r[verify commit-advance.advance]
    #[test]
    fn test_advance_no_current_term_entries() {
        let log = vec![entry(1, 0), entry(1, 0), entry(1, 0)];
        let matches = vec![3, 3, 3, 3, 3];
        // current_term=2 but all entries are term 1 — no advancement
        let result = advance_commit_index(&log, &matches, 0, 2, 5);
        assert_eq!(result, 0);
    }

    // r[verify commit-advance.advance]
    #[test]
    fn test_advance_commit_monotonic() {
        let log = vec![entry(1, 0), entry(2, 0)];
        let matches = vec![2, 2, 2];
        let result = advance_commit_index(&log, &matches, 1, 2, 3);
        assert!(result >= 1); // never goes below current_commit
        assert_eq!(result, 2); // can commit index 2
    }

    // r[verify commit-advance.advance]
    #[test]
    fn test_advance_empty_log() {
        let log: Vec<LogEntry> = vec![];
        let matches = vec![0, 0, 0];
        let result = advance_commit_index(&log, &matches, 0, 1, 3);
        assert_eq!(result, 0);
    }

    // r[verify commit-advance.advance]
    #[test]
    fn test_committed_prefix() {
        // If we commit index 3, indices 1 and 2 are also committed (prefix)
        let log = vec![entry(1, 0), entry(2, 0), entry(3, 0)];
        let matches = vec![3, 3, 3, 1, 1];
        let result = advance_commit_index(&log, &matches, 0, 3, 5);
        assert_eq!(result, 3);
        // log[0] (term 1) and log[1] (term 2) are committed as prefix of committed index 3
    }

    proptest! {
        // r[verify commit-advance.advance]
        #[test]
        fn prop_advance_monotonic(
            terms in proptest::collection::vec(1u64..5, 0..=10),
            match_vals in proptest::collection::vec(0u64..15, 5..=5),
            current_commit in 0u64..10,
            current_term in 1u64..5,
        ) {
            let log: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 0)).collect();
            let result = advance_commit_index(
                &log, &match_vals, current_commit, current_term, 5,
            );
            prop_assert!(result >= current_commit,
                "commit decreased: {} < {}", result, current_commit);
        }

        // r[verify commit-advance.term-guard]
        #[test]
        fn prop_term_guard_blocks_prior(
            terms in proptest::collection::vec(1u64..5, 1..=10),
            current_term in 6u64..10,
        ) {
            let log: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 0)).collect();
            // All entries have term < current_term (6..10 vs 1..5)
            for i in 1..=log.len() {
                prop_assert!(!can_commit_at(&log, i as u64, current_term));
            }
        }

        // r[verify commit-advance.quorum-check]
        // r[verify commit-advance.advance]
        /// Model oracle: compute advance manually and compare.
        #[test]
        fn prop_noop_advance_is_identity(
            terms in proptest::collection::vec(1u64..5, 1..=5),
            current_term in 1u64..5,
        ) {
            let log: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 0)).collect();
            let log_len = log.len() as u64;
            // If commit is already at log_len, advance should return log_len
            let result = advance_commit_index(&log, &[log_len; 5], log_len, current_term, 5);
            prop_assert_eq!(result, log_len);
        }

        #[test]
        fn prop_advance_never_decreases(
            terms in proptest::collection::vec(1u64..5, 1..=10),
            match_vals in proptest::collection::vec(0u64..15, 5..=5),
            current_commit in 0u64..10,
            current_term in 1u64..5,
        ) {
            let log: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 0)).collect();
            let result = advance_commit_index(&log, &match_vals, current_commit, current_term, 5);
            prop_assert!(result >= current_commit);
        }

        #[test]
        fn prop_quorum_at_model(
            match_vals in proptest::collection::vec(0u64..10, 1..=9),
            index in 0u64..10,
        ) {
            let cs = match_vals.len() as u64;
            let has = has_quorum_at(&match_vals, index, cs);
            let manual = match_vals.iter().filter(|&&m| m >= index).count() as u64;
            prop_assert_eq!(has, manual >= cs / 2 + 1);
        }

        #[test]
        fn prop_advance_model_oracle(
            terms in proptest::collection::vec(1u64..4, 1..=8),
            match_vals in proptest::collection::vec(0u64..10, 5..=5),
            current_commit in 0u64..5,
            current_term in 1u64..4,
        ) {
            let log: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 0)).collect();
            let cluster_size = 5u64;

            // Manual computation
            let mut expected = current_commit;
            for n in (1..=log.len()).rev() {
                let n_u64 = n as u64;
                if n_u64 <= current_commit {
                    break;
                }
                if log[n - 1].term == current_term {
                    let count = match_vals.iter().filter(|&&m| m >= n_u64).count() as u64;
                    if count > cluster_size / 2 {
                        expected = n_u64;
                        break;
                    }
                }
            }

            let result = advance_commit_index(
                &log, &match_vals, current_commit, current_term, cluster_size,
            );
            prop_assert_eq!(result, expected,
                "mismatch: got {} expected {}", result, expected);
        }
    }
}
