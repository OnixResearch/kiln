//! Verified consensus read/write path composition.
//!
//! Chains existing Raft consensus module specs into end-to-end proofs:
//!
//! **Write path:** leader append → consistency check → replicate →
//! commit advance → follower commit → apply to state machine.
//!
//! **Read path:** authority confirmation (quorum ack or lease) →
//! commit covers read index → state machine safety (all servers agree).
//!
//! No new exec functions — only proof fns connecting existing specs.

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::commit_advance::*;
#[cfg(verus_keep_ghost)]
use crate::follower_commit::*;
#[cfg(verus_keep_ghost)]
use crate::last_applied::*;
#[cfg(verus_keep_ghost)]
use crate::leader_append_only::*;
#[cfg(verus_keep_ghost)]
use crate::lease_read::*;
#[cfg(verus_keep_ghost)]
use crate::log::*;
#[cfg(verus_keep_ghost)]
use crate::read_index::*;
#[cfg(verus_keep_ghost)]
use crate::state_machine_safety::*;

#[cfg(verus_keep_ghost)]
verus! {

// ============================================================
// Write Path: Stage 1 — Leader Append
// ============================================================

// r[depends consensus-path.append-preserves-invariant]
// r[impl consensus-path.append-preserves-invariant]
// r[verify consensus-path.append-preserves-invariant]
/// Appending an entry satisfies the leader log invariant.
/// The new log is a prefix extension of the old.
pub proof fn append_preserves_invariant(
    log: Seq<LogEntry>,
    current_term: u64,
    value: u64,
)
    ensures ({
        let new_log = leader_append_entry_spec(log, current_term, value);
        leader_log_invariant_spec(log, new_log)
    })
{
    let new_log = leader_append_entry_spec(log, current_term, value);
    // new_log == log.push(entry), so new_log.len() == log.len() + 1 >= log.len()
    // and for all i < log.len(): new_log[i] == log[i] (Seq::push preserves prefix)
    assert(new_log.len() >= log.len());
    assert forall|i: int| #![trigger new_log[i]]
        0 <= i < log.len()
        implies new_log[i].term == log[i].term && new_log[i].value == log[i].value
    by {
        // log.push(e)[i] == log[i] for i < log.len() — vstd Seq::push axiom
    }
}

// ============================================================
// Write Path: Stage 2 — Appended Entry Is Committable
// ============================================================

// r[depends consensus-path.appended-entry-committable]
// r[impl consensus-path.appended-entry-committable]
// r[verify consensus-path.appended-entry-committable]
/// An entry appended at the current term creates a committable entry.
/// The new entry is at index log.len() + 1 (1-based) with the leader's term.
pub proof fn appended_entry_committable(
    log: Seq<LogEntry>,
    current_term: u64,
    value: u64,
)
    requires
        current_term > 0,
        log.len() < u64::MAX as int,
    ensures ({
        let new_log = leader_append_entry_spec(log, current_term, value);
        let new_index = (log.len() + 1) as u64;
        can_commit_at_spec(new_log, new_index, current_term)
    })
{
    let new_log = leader_append_entry_spec(log, current_term, value);
    let new_index = (log.len() + 1) as u64;
    // new_log[new_index - 1] == new_log[log.len()] == LogEntry { term: current_term, value }
    // can_commit_at requires: new_index > 0 ∧ new_log[new_index-1].term == current_term
    assert(new_log[(new_index - 1) as int].term == current_term);
}

// ============================================================
// Write Path: Stage 3 — Follower Commit Is Monotonic and Bounded
// ============================================================

// r[depends consensus-path.follower-commit-monotonic-bounded]
// r[impl consensus-path.follower-commit-monotonic-bounded]
// r[verify consensus-path.follower-commit-monotonic-bounded]
/// Follower commit update is monotonic (>= current) and bounded (<= leader).
pub proof fn follower_commit_monotonic_bounded(
    current_commit: u64,
    leader_commit: u64,
    last_new_entry_index: u64,
)
    requires current_commit <= leader_commit
    ensures ({
        let result = follower_update_commit_spec(current_commit, leader_commit, last_new_entry_index);
        result >= current_commit && result <= leader_commit
    })
{}

// ============================================================
// Write Path: Stage 4 — Applied Entries Are Committed
// ============================================================

// r[depends consensus-path.applied-bounded-by-commit]
// r[impl consensus-path.applied-bounded-by-commit]
// r[verify consensus-path.applied-bounded-by-commit]
/// After advancing last_applied, the result is still <= commit_index.
/// No entry is applied before it's committed.
pub proof fn applied_bounded_by_commit(
    last_applied: u64,
    commit_index: u64,
)
    requires last_applied <= commit_index
    ensures
        advance_last_applied_spec(last_applied, commit_index) <= commit_index,
        advance_last_applied_spec(last_applied, commit_index) >= last_applied,
{}

// ============================================================
// Write Path: Stage 5 — End-to-End Write Composition
// ============================================================

// r[depends consensus-path.write-path-end-to-end]
// r[impl consensus-path.write-path-end-to-end]
// r[verify consensus-path.write-path-end-to-end]
/// End-to-end write path: appending an entry at term T, once committed,
/// creates a committable entry that can be applied. The log invariant
/// holds throughout, and the applied index stays within bounds.
pub proof fn write_path_end_to_end(
    log: Seq<LogEntry>,
    current_term: u64,
    value: u64,
    last_applied: u64,
)
    requires
        current_term > 0,
        last_applied as int <= log.len(),
        log.len() < u64::MAX as int,  // room for one more entry
    ensures ({
        // Stage 1: append preserves invariant
        let new_log = leader_append_entry_spec(log, current_term, value);
        let new_index = (log.len() + 1) as u64;
        leader_log_invariant_spec(log, new_log)
        // Stage 2: the new entry is committable at new_index
        && can_commit_at_spec(new_log, new_index, current_term)
        // Stage 4: applying up to new_index stays bounded
        && advance_last_applied_spec(last_applied, new_index) <= new_index
        && advance_last_applied_spec(last_applied, new_index) >= last_applied
    })
{
    append_preserves_invariant(log, current_term, value);
    appended_entry_committable(log, current_term, value);
    applied_bounded_by_commit(last_applied, (log.len() + 1) as u64);
}

// ============================================================
// Read Path: Stage 1 — Lease Subsumes ReadIndex
// ============================================================

// r[depends consensus-path.lease-subsumes-read-index]
// r[impl consensus-path.lease-subsumes-read-index]
// r[verify consensus-path.lease-subsumes-read-index]
/// lease_read_safe implies read_safe_at. Lease reads are strictly
/// stronger than ReadIndex — they add a lease validity check on top.
pub proof fn lease_subsumes_read_index(
    lease_valid: bool,
    commit_index: u64,
    read_index: u64,
)
    requires lease_read_safe_spec(lease_valid, commit_index, read_index)
    ensures read_safe_at_spec(read_index, commit_index)
{}

// ============================================================
// Read Path: Stage 2 — Committed Read Is Consistent
// ============================================================

// r[depends consensus-path.committed-read-consistent]
// r[impl consensus-path.committed-read-consistent]
// r[verify consensus-path.committed-read-consistent]
/// If two servers have matching log prefixes through the commit point,
/// and a read is safe (commit >= read_index), then both servers agree
/// on every entry below the read index — by state machine safety.
pub proof fn committed_read_consistent(
    log_a: Seq<LogEntry>,
    log_b: Seq<LogEntry>,
    commit_index: u64,
    read_index: u64,
    i: int,
)
    requires
        logs_match_prefix_spec(log_a, log_b, commit_index as int),
        read_safe_at_spec(read_index, commit_index),
        0 <= i,
        (i as int) < read_index as int,
    ensures
        agree_at_spec(log_a, log_b, i)
{
    // read_safe_at: commit_index >= read_index, so i < commit_index
    // logs_match_prefix: k=commit_index, k <= a.len(), k <= b.len()
    // and forall j: 0 <= j < k ==> a[j].term == b[j].term
    // i < read_index <= commit_index = k, so i < a.len() and i < b.len()
    // and a[i].term == b[i].term, which is agree_at_spec
    assert(i < commit_index as int);  // from i < read_index <= commit_index
    assert(i < log_a.len() as int);   // from prefix match: commit_index <= a.len()
    assert(i < log_b.len() as int);   // from prefix match: commit_index <= b.len()
    assert(log_a[i].term == log_b[i].term);  // from prefix quantifier
}

// ============================================================
// Read Path: Stage 3 — End-to-End Read Composition
// ============================================================

// r[depends consensus-path.read-path-end-to-end]
// r[impl consensus-path.read-path-end-to-end]
// r[verify consensus-path.read-path-end-to-end]
/// End-to-end read path: authority confirmation + matching prefix through
/// commit → every entry below read_index agrees on both servers.
pub proof fn read_path_end_to_end(
    log_a: Seq<LogEntry>,
    log_b: Seq<LogEntry>,
    ack_count: u64,
    cluster_size: u64,
    commit_index: u64,
    read_index: u64,
)
    requires
        // Authority confirmed (quorum ack)
        confirm_read_authority_spec(ack_count, cluster_size),
        // Read is safe
        read_safe_at_spec(read_index, commit_index),
        // Logs match through commit
        logs_match_prefix_spec(log_a, log_b, commit_index as int),
    ensures
        // Every entry below read_index agrees
        forall|i: int| 0 <= i < read_index as int
            ==> agree_at_spec(log_a, log_b, i)
{
    assert forall|i: int| 0 <= i < read_index as int
        implies agree_at_spec(log_a, log_b, i)
    by {
        committed_read_consistent(log_a, log_b, commit_index, read_index, i);
    }
}

// ============================================================
// Combined: Write-Then-Read
// ============================================================

// r[depends consensus-path.write-then-read]
// r[impl consensus-path.write-then-read]
// r[verify consensus-path.write-then-read]
/// A write committed at index N is visible to any subsequent read
/// with read_index > N, provided logs match through the commit point.
pub proof fn write_then_read(
    log_a: Seq<LogEntry>,
    log_b: Seq<LogEntry>,
    write_index: u64,
    commit_index: u64,
    read_index: u64,
)
    requires
        // Write is committed
        write_index > 0,
        (write_index as int) <= commit_index as int,
        // Read covers the write
        read_safe_at_spec(read_index, commit_index),
        (write_index as int) <= read_index as int,
        // Logs match through commit
        logs_match_prefix_spec(log_a, log_b, commit_index as int),
    ensures
        // The written entry agrees on both servers
        agree_at_spec(log_a, log_b, (write_index - 1) as int)
{
    // write_index - 1 < write_index <= read_index <= commit_index
    // logs match through commit, so entry at write_index - 1 agrees
    committed_read_consistent(
        log_a, log_b, commit_index, read_index, (write_index - 1) as int,
    );
}

} // verus!

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
#[allow(clippy::cast_possible_truncation)]
mod tests {
    use proptest::prelude::*;

    use crate::commit_advance::*;
    use crate::follower_commit::*;
    use crate::last_applied::*;
    use crate::leader_append_only::*;
    use crate::log::*;
    use crate::read_index::*;

    // r[verify consensus-path.append-preserves-invariant]
    #[test]
    fn test_append_preserves_invariant() {
        let mut log = vec![LogEntry { term: 1, value: 10 }, LogEntry { term: 1, value: 20 }];
        let old_len = log.len();
        leader_append_entry(&mut log, 2, 30);
        assert_eq!(log.len(), old_len + 1);
        // Prefix preserved
        assert_eq!(log[0].term, 1);
        assert_eq!(log[1].term, 1);
        // New entry has current term
        assert_eq!(log[2].term, 2);
    }

    // r[verify consensus-path.appended-entry-committable]
    #[test]
    fn test_appended_entry_committable() {
        let mut log = vec![LogEntry { term: 1, value: 10 }];
        let old_len = log.len();
        leader_append_entry(&mut log, 2, 20);
        let new_index = (old_len + 1) as u64;
        // New entry at index 2 (1-based), term 2
        assert!(can_commit_at(&log, new_index, 2));
        // Old entry at index 1, term 1 — not committable at term 2
        assert!(!can_commit_at(&log, 1, 2));
    }

    // r[verify consensus-path.follower-commit-monotonic-bounded]
    #[test]
    fn test_follower_commit_bounded() {
        // Follower updates to min(leader_commit, last_new_entry)
        let result = follower_update_commit(0, 5, 3);
        assert_eq!(result, 3); // bounded by last_new_entry
        assert!(result <= 5); // bounded by leader

        let result2 = follower_update_commit(2, 5, 10);
        assert_eq!(result2, 5); // bounded by leader_commit
        assert!(result2 >= 2); // monotonic
    }

    // r[verify consensus-path.applied-bounded-by-commit]
    #[test]
    fn test_applied_bounded() {
        assert_eq!(advance_last_applied(0, 5), 1);
        assert!(advance_last_applied(0, 5) <= 5);

        assert_eq!(advance_last_applied(5, 5), 5);
        assert!(advance_last_applied(5, 5) <= 5);
    }

    // r[verify consensus-path.write-path-end-to-end]
    #[test]
    fn test_write_path_scenario() {
        // Start with empty log, term 1
        let mut log: Vec<LogEntry> = vec![];
        let term = 1u64;

        // Stage 1: leader appends
        leader_append_entry(&mut log, term, 42);
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].term, 1);
        assert_eq!(log[0].value, 42);

        // Stage 2: entry at index 1 is committable
        assert!(can_commit_at(&log, 1, term));

        // Stage 4: can apply up to commit index 1
        let last_applied = advance_last_applied(0, 1);
        assert_eq!(last_applied, 1);
        assert!(last_applied <= 1);
    }

    // r[verify consensus-path.read-path-end-to-end]
    #[test]
    fn test_read_path_scenario() {
        // Two servers with matching logs
        let log_a = vec![
            LogEntry { term: 1, value: 10 },
            LogEntry { term: 1, value: 20 },
            LogEntry { term: 2, value: 30 },
        ];
        let log_b = vec![
            LogEntry { term: 1, value: 10 },
            LogEntry { term: 1, value: 20 },
            LogEntry { term: 2, value: 30 },
        ];

        // Authority confirmed (3 acks out of 5)
        assert!(confirm_read_authority(3, 5));

        // Read at index 2, commit at 3 — safe
        assert!(read_safe_at(2, 3));

        // All entries below read_index agree
        assert!(agree_at(&log_a, &log_b, 0));
        assert!(agree_at(&log_a, &log_b, 1));
    }

    // r[verify consensus-path.write-then-read]
    #[test]
    fn test_write_then_read() {
        let log_a = vec![LogEntry { term: 1, value: 100 }, LogEntry { term: 2, value: 200 }];
        let log_b = log_a.clone();

        // Write at index 2 (1-based), committed at index 2
        // Read with read_index=2, commit_index=2
        assert!(read_safe_at(2, 2));

        // Written entry (index 1, 0-based) agrees
        assert!(agree_at(&log_a, &log_b, 1));
    }

    proptest! {
        // r[verify consensus-path.append-preserves-invariant]
        #[test]
        fn prop_append_preserves(
            log_len in 0usize..20,
            term in 1u64..10,
            value in 0u64..1000,
        ) {
            let mut log: Vec<LogEntry> = (0..log_len)
                .map(|i| LogEntry { term: (i as u64 / 3) + 1, value: i as u64 })
                .collect();
            let old_log = log.clone();
            leader_append_entry(&mut log, term, value);

            // Invariant: prefix preserved, length grows
            prop_assert_eq!(log.len(), old_log.len() + 1);
            for i in 0..old_log.len() {
                prop_assert_eq!(log[i].term, old_log[i].term);
                prop_assert_eq!(log[i].value, old_log[i].value);
            }
            prop_assert_eq!(log[old_log.len()].term, term);
        }

        // r[verify consensus-path.appended-entry-committable]
        #[test]
        fn prop_appended_committable(
            log_len in 0usize..20,
            term in 1u64..10,
            value in 0u64..1000,
        ) {
            let mut log: Vec<LogEntry> = (0..log_len)
                .map(|i| LogEntry { term: (i as u64 / 3) + 1, value: i as u64 })
                .collect();
            let old_len = log.len();
            leader_append_entry(&mut log, term, value);
            let new_index = (old_len + 1) as u64;

            prop_assert!(can_commit_at(&log, new_index, term));
        }

        // r[verify consensus-path.follower-commit-monotonic-bounded]
        #[test]
        fn prop_follower_commit_bounded(
            current in 0u64..100,
            leader in 0u64..100,
            last_entry in 0u64..100,
        ) {
            if current <= leader {
                let result = follower_update_commit(current, leader, last_entry);
                prop_assert!(result >= current);
                prop_assert!(result <= leader);
            }
        }

        // r[verify consensus-path.applied-bounded-by-commit]
        #[test]
        fn prop_applied_bounded(
            last_applied in 0u64..100,
            commit in 0u64..100,
        ) {
            if last_applied <= commit {
                let result = advance_last_applied(last_applied, commit);
                prop_assert!(result <= commit);
                prop_assert!(result >= last_applied);
            }
        }

        // r[verify consensus-path.write-then-read]
        #[test]
        fn prop_write_then_read(
            log_len in 1usize..10,
            term in 1u64..5,
        ) {
            // Build matching logs
            let log: Vec<LogEntry> = (0..log_len)
                .map(|i| LogEntry { term, value: i as u64 })
                .collect();
            let log_b = log.clone();

            // Pick a write index and read index
            let write_idx = log_len as u64; // 1-based, last entry
            let commit_idx = log_len as u64;
            let read_idx = log_len as u64;

            prop_assert!(read_safe_at(read_idx, commit_idx));

            // Entry at write_idx - 1 (0-based) agrees
            if write_idx > 0 && (write_idx - 1) < log.len() as u64 {
                prop_assert!(agree_at(&log, &log_b, (write_idx - 1) as usize));
            }
        }
    }
}
