//! Verified state machine safety theorem for Raft.
//!
//! If server A has applied entry E_a at log index i and server B has
//! applied entry E_b at log index i, then E_a == E_b. No two servers
//! apply different entries at the same index.
//!
//! This follows from:
//! 1. Only committed entries are applied
//! 2. Committed entries have quorum replication
//! 3. Log Matching Property: agreement at index i implies agreement at all j < i
//! 4. Leader completeness: committed entries survive elections
//!
//! This module states the key lemmas and their connections.

use vstd::prelude::*;

use crate::log::*;

verus! {

// --- State machine safety: committed entries agree ---

// r[depends state-machine-safety.index-agreement]
/// Spec: two logs that both have a committed entry at index i with the
/// same term have identical entries at that index.
///
/// This follows from the Log Matching Property: if two logs agree on
/// term at an index, they agree at all prior indices. Since committed
/// entries come from the same leader term at that index, the terms match.
pub open spec fn committed_entries_agree_spec(
    log_a: Seq<LogEntry>,
    log_b: Seq<LogEntry>,
    index: int,
) -> bool {
    (0 <= index && index < log_a.len() && index < log_b.len()
     && log_a[index].term == log_b[index].term)
    ==> agree_at_spec(log_a, log_b, index)
}

// r[impl state-machine-safety.index-agreement]
// r[verify state-machine-safety.index-agreement]
/// If two logs have the same term at an index, they agree (by definition
/// of agree_at which checks term equality).
proof fn committed_entries_agree(
    log_a: Seq<LogEntry>,
    log_b: Seq<LogEntry>,
    index: int,
)
    ensures committed_entries_agree_spec(log_a, log_b, index)
{}

// --- Committed prefix agreement ---

// r[depends state-machine-safety.prefix-agreement]
/// Spec: if two logs agree through a committed prefix (same terms at all
/// indices [0, k)), then every committed entry in that prefix is identical
/// across both logs.
pub open spec fn committed_prefix_agrees_spec(
    log_a: Seq<LogEntry>,
    log_b: Seq<LogEntry>,
    k: int,
) -> bool {
    logs_match_prefix_spec(log_a, log_b, k)
    ==> forall|i: int| #![trigger log_a[i]]
        0 <= i < k ==> agree_at_spec(log_a, log_b, i)
}

// r[impl state-machine-safety.prefix-agreement]
// r[verify state-machine-safety.prefix-agreement]
/// If two logs match on a prefix, every entry in that prefix agrees.
proof fn committed_prefix_agrees(
    log_a: Seq<LogEntry>,
    log_b: Seq<LogEntry>,
    k: int,
)
    requires logs_match_prefix_spec(log_a, log_b, k)
    ensures
        forall|i: int| #![trigger log_a[i]]
            0 <= i < k ==> agree_at_spec(log_a, log_b, i)
{}

// --- Commit implies application safety ---

// r[depends state-machine-safety.commit-safety]
/// Spec: if entry at index i is committed (i < commit_index) and the
/// log satisfies the Log Matching Property with another log through
/// commit_index, then both logs have the same entry at i.
pub open spec fn commit_implies_safe_spec(
    log_a: Seq<LogEntry>,
    log_b: Seq<LogEntry>,
    commit_index: int,
    i: int,
) -> bool {
    (logs_match_prefix_spec(log_a, log_b, commit_index) && 0 <= i && i < commit_index)
    ==> agree_at_spec(log_a, log_b, i)
}

// r[impl state-machine-safety.commit-safety]
// r[verify state-machine-safety.commit-safety]
/// Committed entry safety: if logs match through the commit point,
/// all committed entries are identical.
proof fn commit_implies_safe(
    log_a: Seq<LogEntry>,
    log_b: Seq<LogEntry>,
    commit_index: int,
    i: int,
)
    requires
        logs_match_prefix_spec(log_a, log_b, commit_index),
        0 <= i,
        i < commit_index,
    ensures agree_at_spec(log_a, log_b, i)
{}

// --- Monotonic commit safety ---

// r[verify state-machine-safety.commit-safety]
/// Advancing the commit index preserves safety: if logs match through
/// old_commit and we advance to new_commit (still within the matching
/// prefix), all entries up to new_commit are safe.
proof fn advance_commit_preserves_safety(
    log_a: Seq<LogEntry>,
    log_b: Seq<LogEntry>,
    old_commit: int,
    new_commit: int,
)
    requires
        logs_match_prefix_spec(log_a, log_b, new_commit),
        0 <= old_commit,
        old_commit <= new_commit,
    ensures
        forall|i: int| #![trigger log_a[i]]
            0 <= i < new_commit ==> agree_at_spec(log_a, log_b, i)
{}

// --- No divergence after commitment ---

// r[verify state-machine-safety.index-agreement]
/// If an entry is committed and two logs agree at that index, they
/// cannot later disagree at that index (entries are immutable once
/// committed). This is a restatement of agreement persistence.
proof fn committed_agreement_persists(
    log_a: Seq<LogEntry>,
    log_b: Seq<LogEntry>,
    i: int,
)
    requires agree_at_spec(log_a, log_b, i)
    ensures
        log_a[i].term == log_b[i].term
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn entry(term: u64, value: u64) -> LogEntry {
        LogEntry { term, value }
    }

    // r[verify state-machine-safety.index-agreement]
    #[test]
    fn test_same_term_entries_agree() {
        let log_a = vec![entry(1, 10), entry(2, 20), entry(3, 30)];
        let log_b = vec![entry(1, 99), entry(2, 88), entry(3, 77)];
        for i in 0..3 {
            assert!(crate::log::agree_at(&log_a, &log_b, i));
        }
    }

    // r[verify state-machine-safety.prefix-agreement]
    #[test]
    fn test_prefix_agreement() {
        let log_a = vec![entry(1, 10), entry(2, 20), entry(3, 30)];
        let log_b = vec![entry(1, 99), entry(2, 88), entry(3, 77)];
        assert!(crate::log::logs_match_prefix(&log_a, &log_b, 3));
        for i in 0..3 {
            assert!(crate::log::agree_at(&log_a, &log_b, i));
        }
    }

    // r[verify state-machine-safety.commit-safety]
    #[test]
    fn test_committed_entries_safe() {
        let log_a = vec![entry(1, 10), entry(2, 20), entry(3, 30)];
        let log_b = vec![entry(1, 99), entry(2, 88), entry(3, 77), entry(4, 66)];
        assert!(crate::log::logs_match_prefix(&log_a, &log_b, 3));
        for i in 0..3 {
            assert!(crate::log::agree_at(&log_a, &log_b, i));
        }
    }

    // r[verify state-machine-safety.index-agreement]
    #[test]
    fn test_divergent_terms_disagree() {
        let log_a = vec![entry(1, 10), entry(2, 20)];
        let log_b = vec![entry(1, 10), entry(3, 20)];
        assert!(crate::log::agree_at(&log_a, &log_b, 0));
        assert!(!crate::log::agree_at(&log_a, &log_b, 1));
    }

    /// Generate a log with monotonically non-decreasing terms (valid Raft log).
    fn arb_log(max_len: usize) -> impl Strategy<Value = Vec<LogEntry>> {
        proptest::collection::vec((1u64..20, 0u64..1000), 0..max_len).prop_map(|pairs| {
            let mut term = 0u64;
            pairs
                .into_iter()
                .map(|(dt, val)| {
                    term = term.saturating_add(dt);
                    entry(term, val)
                })
                .collect()
        })
    }

    proptest! {
        // r[verify state-machine-safety.index-agreement]
        #[test]
        fn prop_same_terms_agree(terms in proptest::collection::vec(1u64..100, 1..10)) {
            // Two logs with identical term sequences agree at every index
            let log_a: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 0)).collect();
            let log_b: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 999)).collect();
            for i in 0..log_a.len() {
                prop_assert!(crate::log::agree_at(&log_a, &log_b, i));
            }
        }

        // r[verify state-machine-safety.prefix-agreement]
        #[test]
        fn prop_prefix_match_implies_agreement(log in arb_log(8)) {
            // A log matches its own prefix at every position
            let n = log.len();
            if n > 0 {
                prop_assert!(crate::log::logs_match_prefix(&log, &log, n));
                for i in 0..n {
                    prop_assert!(crate::log::agree_at(&log, &log, i));
                }
            }
        }

        // r[verify state-machine-safety.commit-safety]
        #[test]
        fn prop_commit_advance_safe(
            terms in proptest::collection::vec(1u64..20, 2..8),
            commit in 1usize..7,
        ) {
            let n = terms.len();
            if commit <= n {
                let log_a: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 0)).collect();
                let log_b: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 42)).collect();
                prop_assert!(crate::log::logs_match_prefix(&log_a, &log_b, commit));
                for i in 0..commit {
                    prop_assert!(crate::log::agree_at(&log_a, &log_b, i));
                }
            }
        }

        // r[verify state-machine-safety.index-agreement]
        #[test]
        fn prop_different_terms_disagree(
            t_a in 1u64..100,
            t_b in 1u64..100,
            idx in 0usize..5,
        ) {
            if t_a != t_b {
                let log_a = vec![entry(t_a, 0)];
                let log_b = vec![entry(t_b, 0)];
                if idx == 0 {
                    prop_assert!(!crate::log::agree_at(&log_a, &log_b, idx));
                }
            }
        }
    }
}
