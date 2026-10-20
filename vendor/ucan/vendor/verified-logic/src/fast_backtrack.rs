// Verus preconditions guarantee u64→usize casts are in bounds
#![allow(clippy::cast_possible_truncation)]

//! Verified accelerated log backtracking for Raft.
//!
//! When AppendEntries fails, the basic protocol decrements nextIndex by 1.
//! The accelerated variant (§5.3 optimization) has the follower return the
//! term of the conflicting entry and the first index it stores for that
//! term. The leader can then skip an entire conflicting term in one step,
//! reducing the number of rejected RPCs from O(entries) to O(terms).

use vstd::prelude::*;

use crate::log::LogEntry;

verus! {

// --- conflict_hint ---

/// Conflict hint returned by a follower on AppendEntries rejection.
pub struct ConflictHint {
    /// Term of the conflicting entry (0 if log too short).
    pub conflict_term: u64,
    /// First index the follower stores for that term (0 if log too short).
    pub first_index_for_term: u64,
    /// Length of the follower's log.
    pub log_length: u64,
}

// r[depends fast-backtrack.conflict-hint]
/// Spec: compute the conflict hint from the follower's log.
///
/// If the log is too short (no entry at prev_log_index - 1), return
/// conflict_term=0 and first_index_for_term=0 with the log length.
/// Otherwise, return the term at the rejected index and scan backward
/// to find the first index with that term.
pub open spec fn conflict_hint_spec(
    log: Seq<LogEntry>,
    prev_log_index: u64,
) -> ConflictHint {
    let idx = (prev_log_index - 1) as int;
    if prev_log_index == 0 || idx >= log.len() {
        // Log too short — no entry at that index.
        ConflictHint {
            conflict_term: 0,
            first_index_for_term: 0,
            log_length: log.len() as u64,
        }
    } else {
        let cterm = log[idx].term;
        ConflictHint {
            conflict_term: cterm,
            first_index_for_term: first_index_of_term_spec(log, cterm, idx),
            log_length: log.len() as u64,
        }
    }
}

// r[impl fast-backtrack.first-index-of-term]
// r[verify fast-backtrack.first-index-of-term]
// r[depends fast-backtrack.first-index-of-term]
/// Spec: find the first log index with the given term, scanning backward
/// from `from` (inclusive).
pub open spec fn first_index_of_term_spec(
    log: Seq<LogEntry>,
    term: u64,
    from: int,
) -> u64
    decreases from,
{
    if from <= 0 {
        if log.len() > 0 && log[0].term == term { 0 } else { 0 }
    } else if log[from - 1].term == term {
        first_index_of_term_spec(log, term, from - 1)
    } else {
        from as u64
    }
}

// r[impl fast-backtrack.conflict-hint]
/// Compute the conflict hint for a rejected AppendEntries.
pub fn conflict_hint(
    log: &[LogEntry],
    prev_log_index: u64,
) -> (result: ConflictHint)
    requires
        prev_log_index as int <= usize::MAX as int,
        log@.len() <= u64::MAX as int,
    ensures
        result.conflict_term == conflict_hint_spec(log@, prev_log_index).conflict_term,
        result.first_index_for_term == conflict_hint_spec(log@, prev_log_index).first_index_for_term,
        result.log_length == log@.len() as u64,
{
    if prev_log_index == 0 {
        return ConflictHint {
            conflict_term: 0,
            first_index_for_term: 0,
            log_length: log.len() as u64,
        };
    }
    let idx = (prev_log_index - 1) as usize;
    if idx >= log.len() {
        ConflictHint {
            conflict_term: 0,
            first_index_for_term: 0,
            log_length: log.len() as u64,
        }
    } else {
        let cterm = log[idx].term;
        // Scan backward to find first index with this term.
        let mut scan: usize = idx;
        while scan > 0 && log[scan - 1].term == cterm
            invariant
                0 <= scan <= idx,
                scan <= log@.len(),
                forall|j: int| scan <= j <= idx as int ==> log@[j].term == cterm,
            decreases scan,
        {
            scan = scan - 1;
        }
        assert(scan as u64 == first_index_of_term_spec(log@, cterm, idx as int)) by {
            first_index_of_term_loop_equiv(log@, cterm, idx as int, scan as int);
        }
        ConflictHint {
            conflict_term: cterm,
            first_index_for_term: scan as u64,
            log_length: log.len() as u64,
        }
    }
}

/// Helper lemma connecting the loop invariant to the recursive spec.
proof fn first_index_of_term_loop_equiv(
    log: Seq<LogEntry>,
    term: u64,
    from: int,
    result: int,
)
    requires
        0 <= result <= from,
        from < log.len(),
        forall|j: int| result <= j <= from ==> log[j].term == term,
        result == 0 || log[result - 1].term != term,
    ensures result as u64 == first_index_of_term_spec(log, term, from)
    decreases from - result,
{
    if from <= 0 {
        // Base case
    } else if log[from - 1].term == term {
        first_index_of_term_loop_equiv(log, term, from - 1, result);
    } else {
        // from == result
    }
}

// --- fast_backtrack ---

// r[depends fast-backtrack.fast-backtrack]
/// Spec: compute the new nextIndex using the conflict hint.
///
/// If the leader has no entries with conflict_term, set nextIndex to
/// first_index_for_term (skip the entire conflicting term at once).
/// If the leader does have entries with conflict_term, set nextIndex
/// to one past the leader's last entry with that term.
/// If conflict_term is 0 (log too short), set nextIndex to log_length + 1.
pub open spec fn fast_backtrack_spec(
    hint: ConflictHint,
    leader_has_conflict_term: bool,
    leader_last_index_for_term: u64,
) -> u64 {
    if hint.conflict_term == 0 {
        // Follower's log is too short.
        if hint.log_length == 0 { 1 } else { (hint.log_length + 1) as u64 }
    } else if leader_has_conflict_term {
        // Leader has the term — skip to one past the leader's last entry.
        if leader_last_index_for_term == 0 { 1u64 }
        else { (leader_last_index_for_term + 1) as u64 }
    } else {
        // Leader doesn't have the term — skip to the first index.
        if hint.first_index_for_term == 0 { 1 }
        else { hint.first_index_for_term }
    }
}

// r[impl fast-backtrack.fast-backtrack]
/// Compute the new nextIndex using the conflict hint.
pub fn fast_backtrack(
    hint: &ConflictHint,
    leader_has_conflict_term: bool,
    leader_last_index_for_term: u64,
) -> (result: u64)
    requires
        leader_has_conflict_term ==> leader_last_index_for_term < u64::MAX,
        hint.log_length < u64::MAX,
    ensures
        result == fast_backtrack_spec(
            *hint, leader_has_conflict_term, leader_last_index_for_term,
        ),
        result >= 1,
{
    if hint.conflict_term == 0 {
        if hint.log_length == 0 { 1 } else { hint.log_length + 1 }
    } else if leader_has_conflict_term {
        if leader_last_index_for_term == 0 { 1 }
        else { leader_last_index_for_term + 1 }
    } else {
        if hint.first_index_for_term == 0 { 1 }
        else { hint.first_index_for_term }
    }
}

// --- leader_find_last_index_for_term ---

// r[depends fast-backtrack.leader-find-last]
/// Spec: find the last index in the leader's log with the given term.
/// Returns 0 if the term is not found.
pub open spec fn leader_find_last_index_for_term_spec(
    log: Seq<LogEntry>,
    term: u64,
) -> u64
{
    leader_find_last_from_spec(log, term, (log.len() - 1) as int)
}

pub open spec fn leader_find_last_from_spec(
    log: Seq<LogEntry>,
    term: u64,
    from: int,
) -> u64
    decreases from + 1,
{
    if from < 0 {
        0
    } else if log[from].term == term {
        // 1-indexed result (matching Raft convention for nextIndex math)
        (from + 1) as u64
    } else {
        leader_find_last_from_spec(log, term, from - 1)
    }
}

// r[impl fast-backtrack.leader-find-last]
/// Find the last index in the leader's log with the given term.
/// Returns (true, last_index_1based) or (false, 0).
pub fn leader_find_last_index_for_term(
    log: &[LogEntry],
    term: u64,
) -> (result: (bool, u64))
    ensures
        result.0 ==> result.1 > 0,
        !result.0 ==> result.1 == 0,
{
    if log.len() == 0 {
        return (false, 0);
    }
    let mut i: usize = log.len() - 1;
    loop
        invariant
            i < log@.len(),
            i < usize::MAX,
        decreases i,
    {
        if log[i].term == term {
            return (true, (i + 1) as u64);
        }
        if i == 0 {
            return (false, 0);
        }
        i = i - 1;
    }
}

// --- Safety proofs ---

// r[verify fast-backtrack.fast-backtrack]
/// Fast backtrack always produces nextIndex >= 1.
proof fn fast_backtrack_ge_one(
    hint: ConflictHint,
    leader_has: bool,
    leader_last: u64,
)
    requires
        leader_has ==> leader_last < u64::MAX,
        hint.log_length < u64::MAX,
    ensures fast_backtrack_spec(hint, leader_has, leader_last) >= 1
{}

// r[verify fast-backtrack.conflict-hint]
/// Short log produces conflict_term == 0.
proof fn short_log_hint(log: Seq<LogEntry>, prev_log_index: u64)
    requires
        prev_log_index > 0,
        ((prev_log_index - 1) as int) >= log.len(),
    ensures conflict_hint_spec(log, prev_log_index).conflict_term == 0
{}

// r[verify fast-backtrack.conflict-hint]
/// Hint log_length always matches the actual log length.
proof fn hint_log_length_correct(log: Seq<LogEntry>, prev_log_index: u64)
    requires log.len() <= u64::MAX as int
    ensures conflict_hint_spec(log, prev_log_index).log_length == log.len() as u64
{}

// r[verify fast-backtrack.conflict-hint]
/// When the log has the entry, conflict_term matches its term.
proof fn hint_conflict_term_matches(log: Seq<LogEntry>, prev_log_index: u64)
    requires
        prev_log_index > 0,
        ((prev_log_index - 1) as int) < log.len(),
    ensures
        conflict_hint_spec(log, prev_log_index).conflict_term
        == log[(prev_log_index - 1) as int].term
{}

// r[verify fast-backtrack.conflict-hint]
/// first_index_for_term is within log bounds.
proof fn hint_first_index_bounded(log: Seq<LogEntry>, prev_log_index: u64)
    requires
        prev_log_index > 0,
        ((prev_log_index - 1) as int) < log.len(),
        log.len() <= u64::MAX as int,
    ensures
        (conflict_hint_spec(log, prev_log_index).first_index_for_term as int)
        <= (prev_log_index - 1) as int
{
    let idx = (prev_log_index - 1) as int;
    let cterm = log[idx].term;
    first_index_bounded(log, cterm, idx);
}

proof fn first_index_bounded(log: Seq<LogEntry>, term: u64, from: int)
    requires
        0 <= from,
        from < log.len(),
        log[from].term == term,
    ensures (first_index_of_term_spec(log, term, from) as int) <= from
    decreases from,
{
    if from <= 0 {
    } else if log[from - 1].term == term {
        first_index_bounded(log, term, from - 1);
    } else {
    }
}

// r[verify fast-backtrack.fast-backtrack]
/// When conflict_term is 0 and log_length > 0, nextIndex = log_length + 1.
proof fn short_log_backtrack(hint: ConflictHint)
    requires
        hint.conflict_term == 0,
        hint.log_length > 0,
        hint.log_length < u64::MAX,
    ensures fast_backtrack_spec(hint, false, 0) == hint.log_length + 1
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn entry(term: u64, value: u64) -> LogEntry {
        LogEntry { term, value }
    }

    // r[verify fast-backtrack.conflict-hint]
    #[test]
    fn test_conflict_hint_short_log() {
        let log = vec![entry(1, 0), entry(2, 0)];
        let hint = conflict_hint(&log, 5); // log only has 2 entries
        assert_eq!(hint.conflict_term, 0);
        assert_eq!(hint.log_length, 2);
    }

    // r[verify fast-backtrack.conflict-hint]
    #[test]
    fn test_conflict_hint_with_entry() {
        let log = vec![entry(1, 0), entry(2, 0), entry(2, 0), entry(3, 0)];
        let hint = conflict_hint(&log, 3); // log[2].term=2, check at index 2
        assert_eq!(hint.conflict_term, 2);
        assert_eq!(hint.first_index_for_term, 1); // entries 1,2 have term 2
        assert_eq!(hint.log_length, 4);
    }

    // r[verify fast-backtrack.conflict-hint]
    #[test]
    fn test_conflict_hint_single_term_run() {
        let log = vec![entry(3, 0), entry(3, 0), entry(3, 0)];
        let hint = conflict_hint(&log, 3); // log[2].term=3
        assert_eq!(hint.conflict_term, 3);
        assert_eq!(hint.first_index_for_term, 0); // all entries are term 3
    }

    // r[verify fast-backtrack.fast-backtrack]
    #[test]
    fn test_fast_backtrack_short_log() {
        let hint = ConflictHint {
            conflict_term: 0,
            first_index_for_term: 0,
            log_length: 5,
        };
        assert_eq!(fast_backtrack(&hint, false, 0), 6);
    }

    // r[verify fast-backtrack.fast-backtrack]
    #[test]
    fn test_fast_backtrack_leader_has_term() {
        let hint = ConflictHint {
            conflict_term: 3,
            first_index_for_term: 5,
            log_length: 10,
        };
        // Leader has term 3, last at index 7 (1-based)
        assert_eq!(fast_backtrack(&hint, true, 7), 8);
    }

    // r[verify fast-backtrack.fast-backtrack]
    #[test]
    fn test_fast_backtrack_leader_no_term() {
        let hint = ConflictHint {
            conflict_term: 3,
            first_index_for_term: 5,
            log_length: 10,
        };
        // Leader doesn't have term 3
        assert_eq!(fast_backtrack(&hint, false, 0), 5);
    }

    // r[verify fast-backtrack.leader-find-last]
    #[test]
    fn test_leader_find_last_index() {
        let log = vec![entry(1, 0), entry(2, 0), entry(2, 0), entry(3, 0)];
        let (found, idx) = leader_find_last_index_for_term(&log, 2);
        assert!(found);
        assert_eq!(idx, 3); // 1-based: entries at 0-based indices 1,2 have term 2

        let (found, idx) = leader_find_last_index_for_term(&log, 5);
        assert!(!found);
        assert_eq!(idx, 0);
    }

    // r[verify fast-backtrack.fast-backtrack]
    #[test]
    fn test_end_to_end_backtrack() {
        // Follower log: [t1, t2, t2, t2]
        // Leader log:   [t1, t3, t3, t3, t3]
        // Leader sends prevLogIndex=4, prevLogTerm=3 — follower rejects.
        let follower_log = vec![entry(1, 0), entry(2, 0), entry(2, 0), entry(2, 0)];
        let hint = conflict_hint(&follower_log, 4); // log[3].term=2
        assert_eq!(hint.conflict_term, 2);
        assert_eq!(hint.first_index_for_term, 1);

        let leader_log = vec![entry(1, 0), entry(3, 0), entry(3, 0), entry(3, 0), entry(3, 0)];
        let (has, _) = leader_find_last_index_for_term(&leader_log, 2);
        assert!(!has); // leader has no term-2 entries

        let next = fast_backtrack(&hint, false, 0);
        assert_eq!(next, 1); // skip to first_index_for_term=1
        // One RPC skipped 3 entries instead of decrementing one at a time.
    }

    proptest! {
        // r[verify fast-backtrack.fast-backtrack]
        #[test]
        fn prop_fast_backtrack_ge_one(
            conflict_term: u64,
            first_idx in 0u64..1000,
            log_len in 0u64..1000,
            leader_has: bool,
            leader_last in 0u64..1000,
        ) {
            let hint = ConflictHint {
                conflict_term,
                first_index_for_term: first_idx,
                log_length: log_len,
            };
            let result = fast_backtrack(
                &hint,
                leader_has,
                leader_last,
            );
            prop_assert!(result >= 1);
        }

        // r[verify fast-backtrack.conflict-hint]
        #[test]
        fn prop_hint_log_length(
            terms in proptest::collection::vec(1u64..10, 0..20),
            prev_log_index in 0u64..25,
        ) {
            let log: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 0)).collect();
            let hint = conflict_hint(&log, prev_log_index);
            prop_assert_eq!(hint.log_length, log.len() as u64);
        }

        // r[verify fast-backtrack.conflict-hint]
        #[test]
        fn prop_hint_first_index_bounded(
            terms in proptest::collection::vec(1u64..10, 1..20),
        ) {
            let log: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 0)).collect();
            for prev_log_index in 1..=(log.len() as u64) {
                let hint = conflict_hint(&log, prev_log_index);
                if hint.conflict_term > 0 {
                    prop_assert!(hint.first_index_for_term < prev_log_index);
                }
            }
        }

        #[test]
        fn prop_backtrack_model_oracle(
            conflict_term in 1u64..10,
            first_idx in 1u64..100,
            log_len in 1u64..100,
            leader_has: bool,
            leader_last in 0u64..100,
        ) {
            let hint = ConflictHint {
                conflict_term,
                first_index_for_term: first_idx,
                log_length: log_len,
            };
            let result = fast_backtrack(&hint, leader_has, leader_last);
            if leader_has {
                prop_assert_eq!(result, leader_last + 1);
            } else {
                prop_assert_eq!(result, first_idx);
            }
        }

        #[test]
        fn prop_leader_has_term_skips_to_after(
            conflict_term in 1u64..10,
            first_idx in 1u64..50,
            log_len in 50u64..100,
            leader_last in 1u64..50,
        ) {
            let hint = ConflictHint {
                conflict_term,
                first_index_for_term: first_idx,
                log_length: log_len,
            };
            let result = fast_backtrack(&hint, true, leader_last);
            // When leader has the conflict term, result should be leader_last + 1
            prop_assert_eq!(result, leader_last + 1);
        }

        #[test]
        fn prop_leader_missing_term_uses_first_index(
            conflict_term in 1u64..10,
            first_idx in 1u64..100,
            log_len in 1u64..100,
        ) {
            let hint = ConflictHint {
                conflict_term,
                first_index_for_term: first_idx,
                log_length: log_len,
            };
            let result = fast_backtrack(&hint, false, 0);
            prop_assert_eq!(result, first_idx);
        }
    }
}
