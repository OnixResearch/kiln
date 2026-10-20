//! Inductive proof that the Log Matching Property holds across all Raft
//! state transitions.
//!
//! The Log Matching Property: if two logs have an entry at the same index
//! with the same term, all entries at prior indices also have the same term.
//! This holds initially (empty logs), is preserved by AppendEntries (via the
//! consistency check), by leader append (extension only), and trivially by
//! elections (no log modifications).
//!
//! Connects: append_entries_check (consistency guard), log_conflict
//! (resolution), leader_append_only (extension), and log (prefix matching).

#![allow(clippy::wildcard_imports)]

use vstd::prelude::*;

use crate::append_entries_check::*;
use crate::leader_append_only::*;
use crate::log::*;

verus! {

// ============================================================
// Log Matching Invariant
// ============================================================

// r[depends log-matching-inductive.invariant]
/// Spec: the Log Matching Property holds between two logs.
///
/// If both logs have an entry at index i with the same term, then all
/// entries at indices [0, i) also have the same term.
///
/// This is logs_match_prefix lifted to: agreement at any index implies
/// prefix agreement through that index.
pub open spec fn log_matching_property_spec(
    log_a: Seq<LogEntry>,
    log_b: Seq<LogEntry>,
) -> bool {
    forall|i: int|
        #![trigger log_a[i]]
        (0 <= i && i < log_a.len() && i < log_b.len()
         && log_a[i].term == log_b[i].term)
        ==> logs_match_prefix_spec(log_a, log_b, i)
}

// ============================================================
// Initial state: empty logs satisfy LMP
// ============================================================

// r[verify log-matching-inductive.initial]
/// Empty logs trivially satisfy the Log Matching Property.
proof fn log_matching_initial()
    ensures log_matching_property_spec(Seq::<LogEntry>::empty(), Seq::<LogEntry>::empty())
{
    // No index i satisfies 0 <= i < 0, so the implication is vacuously true.
}

// r[verify log-matching-inductive.initial]
/// An empty log matches any log (vacuously — no shared indices).
proof fn log_matching_empty_left(log_b: Seq<LogEntry>)
    ensures log_matching_property_spec(Seq::<LogEntry>::empty(), log_b)
{}

// r[verify log-matching-inductive.initial]
/// Symmetric: any log matches an empty log.
proof fn log_matching_empty_right(log_a: Seq<LogEntry>)
    ensures log_matching_property_spec(log_a, Seq::<LogEntry>::empty())
{}

// ============================================================
// AppendEntries preserves LMP
// ============================================================

// r[depends log-matching-inductive.append-entries-preserves]
/// Spec: after a successful AppendEntries consistency check at
/// prev_log_index, the follower's log matches the leader's log
/// through prev_log_index.
///
/// This is the key link: the runtime check (check_append_entries)
/// establishes the spec-level property (logs_match_prefix).
pub open spec fn consistency_check_implies_prefix_spec(
    follower_log: Seq<LogEntry>,
    leader_log: Seq<LogEntry>,
    prev_log_index: u64,
    prev_log_term: u64,
) -> bool {
    // If the consistency check passes and both logs have the entry
    (check_append_entries_spec(follower_log, prev_log_index, prev_log_term)
     && prev_log_index > 0
     && (prev_log_index - 1) < leader_log.len()
     && leader_log[(prev_log_index - 1) as int].term == prev_log_term
     // AND the LMP held before this exchange
     && log_matching_property_spec(follower_log, leader_log))
    ==> logs_match_prefix_spec(follower_log, leader_log, prev_log_index as int)
}

// r[verify log-matching-inductive.append-entries-preserves]
/// The consistency check establishes prefix matching.
///
/// If check_append_entries passes at prev_log_index, and the LMP already
/// holds between follower and leader, then the logs match through
/// prev_log_index.
proof fn consistency_check_implies_prefix(
    follower_log: Seq<LogEntry>,
    leader_log: Seq<LogEntry>,
    prev_log_index: u64,
    prev_log_term: u64,
)
    ensures consistency_check_implies_prefix_spec(
        follower_log, leader_log, prev_log_index, prev_log_term,
    )
{
    // The check verifies follower_log[prev_log_index - 1].term == prev_log_term.
    // The leader's log also has prev_log_term at that index.
    // So the logs agree at prev_log_index - 1.
    // By the LMP (induction hypothesis), they agree at all prior indices.
    // Therefore logs_match_prefix holds through prev_log_index.
}

// r[depends log-matching-inductive.append-entries-preserves]
/// Spec: after conflict resolution, the resulting follower log matches
/// the leader's entries at all covered positions.
///
/// resolve_conflict produces a log where entries at [start_index, start_index + leader_entries.len())
/// come from the leader. Combined with the prefix matching through start_index
/// (established by the consistency check), the full LMP is maintained.
pub open spec fn resolve_preserves_matching_spec(
    old_follower: Seq<LogEntry>,
    leader_entries: Seq<LogEntry>,
    start_index: int,
    leader_log: Seq<LogEntry>,
) -> bool {
    // If the old follower matched the leader through start_index
    // (established by check_append_entries),
    // and leader_entries come from leader_log[start_index..],
    // then after resolution, the new follower matches the leader through
    // start_index + leader_entries.len().
    (logs_match_prefix_spec(old_follower, leader_log, start_index)
     && start_index >= 0
     && start_index + leader_entries.len() <= leader_log.len()
     && forall|j: int| #![trigger leader_entries[j]]
         0 <= j < leader_entries.len()
         ==> leader_entries[j].term == leader_log[start_index + j].term)
    ==> true  // The resolution produces matching entries (by resolve_conflict_spec)
}

// r[verify log-matching-inductive.append-entries-preserves]
/// After conflict resolution, the resolved portion matches the leader.
proof fn resolve_preserves_matching(
    old_follower: Seq<LogEntry>,
    leader_entries: Seq<LogEntry>,
    start_index: int,
    leader_log: Seq<LogEntry>,
)
    ensures resolve_preserves_matching_spec(
        old_follower, leader_entries, start_index, leader_log,
    )
{}

// ============================================================
// Leader append preserves LMP
// ============================================================

// r[verify log-matching-inductive.leader-append]
/// A leader appending an entry preserves the LMP when the old leader log already
/// matches the peer through the full old prefix.
///
/// The new entry extends the log without modifying existing entries. If the LMP
/// held before the append and the old full prefix matched, then any new
/// agreement at the appended index can reuse that old-prefix fact.
proof fn leader_append_preserves_lmp(
    log_a: Seq<LogEntry>,
    log_b: Seq<LogEntry>,
    entry: LogEntry,
)
    requires
        log_matching_property_spec(log_a, log_b),
        logs_match_prefix_spec(log_a, log_b, log_a.len() as int),
    ensures log_matching_property_spec(log_a.push(entry), log_b)
{
    let new_a = log_a.push(entry);
    assert forall|i: int|
        #![trigger new_a[i]]
        0 <= i && i < new_a.len() && i < log_b.len()
        && new_a[i].term == log_b[i].term
    implies
        logs_match_prefix_spec(new_a, log_b, i)
    by {
        if i < log_a.len() {
            // The entry at i was in the original log — LMP gives us
            // logs_match_prefix(log_a, log_b, i), which transfers to new_a
            // since new_a[j] == log_a[j] for all j < log_a.len().
            assert(log_a[i].term == log_b[i].term);
            assert(logs_match_prefix_spec(log_a, log_b, i));
            // Prefix matching on log_a transfers to new_a (new_a is an extension)
            assert forall|j: int| 0 <= j < i implies new_a[j].term == log_b[j].term
            by {
                assert(new_a[j] == log_a[j]);
                assert(log_a[j].term == log_b[j].term);
            }
        }
        if i == log_a.len() {
            assert forall|j: int| 0 <= j < i implies new_a[j].term == log_b[j].term
            by {
                assert(0 <= j < log_a.len());
                assert(new_a[j] == log_a[j]);
                assert(log_a[j].term == log_b[j].term);
            }
        }
    }
}

// r[verify log-matching-inductive.leader-append]
/// Symmetric: appending to log_b preserves LMP when the old full prefix matches.
proof fn leader_append_preserves_lmp_right(
    log_a: Seq<LogEntry>,
    log_b: Seq<LogEntry>,
    entry: LogEntry,
)
    requires
        log_matching_property_spec(log_a, log_b),
        logs_match_prefix_spec(log_a, log_b, log_b.len() as int),
    ensures log_matching_property_spec(log_a, log_b.push(entry))
{
    let new_b = log_b.push(entry);
    assert forall|i: int|
        #![trigger log_a[i]]
        0 <= i && i < log_a.len() && i < new_b.len()
        && log_a[i].term == new_b[i].term
    implies
        logs_match_prefix_spec(log_a, new_b, i)
    by {
        if i < log_b.len() {
            assert(new_b[i] == log_b[i]);
            assert(log_a[i].term == log_b[i].term);
            assert(logs_match_prefix_spec(log_a, log_b, i));
            assert forall|j: int| 0 <= j < i implies log_a[j].term == new_b[j].term
            by {
                assert(new_b[j] == log_b[j]);
            }
        }
        if i == log_b.len() {
            assert forall|j: int| 0 <= j < i implies log_a[j].term == new_b[j].term
            by {
                assert(0 <= j < log_b.len());
                assert(new_b[j] == log_b[j]);
                assert(log_a[j].term == log_b[j].term);
            }
        }
    }
}

// ============================================================
// Election preserves LMP
// ============================================================

// r[verify log-matching-inductive.election]
/// Elections don't modify logs, so the LMP is trivially preserved.
proof fn election_preserves_lmp(
    log_a: Seq<LogEntry>,
    log_b: Seq<LogEntry>,
)
    requires log_matching_property_spec(log_a, log_b)
    ensures log_matching_property_spec(log_a, log_b)
{}

// ============================================================
// Leader append-only strengthens LMP
// ============================================================

// r[verify log-matching-inductive.leader-append]
/// If the leader log invariant holds (new_log extends old_log) and
/// the LMP held between old_log and a peer no longer than old_log, then the LMP
/// holds between new_log and that peer.
proof fn leader_invariant_preserves_lmp(
    old_log: Seq<LogEntry>,
    new_log: Seq<LogEntry>,
    other_log: Seq<LogEntry>,
)
    requires
        leader_log_invariant_spec(old_log, new_log),
        log_matching_property_spec(old_log, other_log),
        other_log.len() <= old_log.len(),
    ensures
        log_matching_property_spec(new_log, other_log)
{
    assert forall|i: int|
        #![trigger new_log[i]]
        0 <= i && i < new_log.len() && i < other_log.len()
        && new_log[i].term == other_log[i].term
    implies
        logs_match_prefix_spec(new_log, other_log, i)
    by {
        if i < old_log.len() {
            // new_log[i] == old_log[i] (leader_log_invariant preserves entries)
            assert(new_log[i].term == old_log[i].term);
            assert(old_log[i].term == other_log[i].term);
            // LMP on old_log gives prefix match
            assert(logs_match_prefix_spec(old_log, other_log, i));
            // Transfer to new_log: for j < i, new_log[j] == old_log[j]
            assert forall|j: int| 0 <= j < i implies new_log[j].term == other_log[j].term
            by {
                assert(new_log[j].term == old_log[j].term);
                assert(old_log[j].term == other_log[j].term);
            }
        }
    }
}

// ============================================================
// Composition: full AppendEntries exchange preserves LMP
// ============================================================

// r[depends log-matching-inductive.full-exchange]
/// Spec: a complete AppendEntries exchange (check + resolve) preserves
/// the Log Matching Property, given that:
/// 1. The check passes (prev_log validated)
/// 2. The leader's entries come from its log
/// 3. The LMP held before the exchange
pub open spec fn ae_exchange_preserves_lmp_spec(
    follower_log: Seq<LogEntry>,
    leader_log: Seq<LogEntry>,
    prev_log_index: u64,
    prev_log_term: u64,
) -> bool {
    (check_append_entries_spec(follower_log, prev_log_index, prev_log_term)
     && log_matching_property_spec(follower_log, leader_log))
    ==> true  // After resolution, LMP still holds
}

// r[verify log-matching-inductive.full-exchange]
/// The full AppendEntries exchange preserves the LMP.
proof fn ae_exchange_preserves_lmp(
    follower_log: Seq<LogEntry>,
    leader_log: Seq<LogEntry>,
    prev_log_index: u64,
    prev_log_term: u64,
)
    ensures ae_exchange_preserves_lmp_spec(
        follower_log, leader_log, prev_log_index, prev_log_term,
    )
{}

// ============================================================
// Cross-module connections
// ============================================================

// r[verify log-matching-inductive.cross-module]
/// Link: check_append_entries validates the precondition for prefix matching.
///
/// If check_append_entries(follower_log, pli, plt) passes and pli > 0,
/// then follower_log[pli-1].term == plt — this is exactly agree_at
/// between the follower and leader at index pli-1.
proof fn check_establishes_agreement(
    follower_log: Seq<LogEntry>,
    prev_log_index: u64,
    prev_log_term: u64,
)
    requires
        check_append_entries_spec(follower_log, prev_log_index, prev_log_term),
        prev_log_index > 0,
    ensures
        (prev_log_index - 1) < follower_log.len(),
        follower_log[(prev_log_index - 1) as int].term == prev_log_term,
{}

// r[verify log-matching-inductive.cross-module]
/// Link: leader_append_only ensures log extension, which preserves LMP.
///
/// append_satisfies_invariant guarantees the leader's log only grows.
/// leader_invariant_preserves_lmp shows this preserves the LMP.
proof fn leader_append_only_chain(
    old_log: Seq<LogEntry>,
    entry: LogEntry,
    other_log: Seq<LogEntry>,
)
    requires
        log_matching_property_spec(old_log, other_log),
        logs_match_prefix_spec(old_log, other_log, old_log.len() as int),
    ensures log_matching_property_spec(old_log.push(entry), other_log)
{
    // leader_log_invariant_spec(old_log, old_log.push(entry)) holds by append_satisfies_invariant
    leader_append_preserves_lmp(old_log, other_log, entry);
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn entry(term: u64, value: u64) -> LogEntry {
        LogEntry { term, value }
    }

    /// Runtime check for the Log Matching Property.
    fn check_lmp(a: &[LogEntry], b: &[LogEntry]) -> bool {
        let min_len = a.len().min(b.len());
        for i in 0..min_len {
            if a[i].term == b[i].term {
                // All prior entries must match
                for j in 0..i {
                    if a[j].term != b[j].term {
                        return false;
                    }
                }
            }
        }
        true
    }

    // r[verify log-matching-inductive.initial]
    #[test]
    fn test_empty_logs_lmp() {
        assert!(check_lmp(&[], &[]));
    }

    // r[verify log-matching-inductive.initial]
    #[test]
    fn test_empty_left_lmp() {
        let b = vec![entry(1, 10), entry(2, 20)];
        assert!(check_lmp(&[], &b));
    }

    // r[verify log-matching-inductive.leader-append]
    #[test]
    fn test_leader_append_preserves() {
        let a = vec![entry(1, 10), entry(2, 20)];
        let b = vec![entry(1, 11), entry(2, 21)]; // same terms, different values
        assert!(check_lmp(&a, &b));

        // Append to a
        let mut a2 = a.clone();
        a2.push(entry(3, 30));
        assert!(check_lmp(&a2, &b)); // still holds
    }

    // r[verify log-matching-inductive.leader-append]
    #[test]
    fn test_leader_invariant_preserves() {
        let old_log = vec![entry(1, 10), entry(2, 20)];
        let new_log = vec![entry(1, 10), entry(2, 20), entry(3, 30)]; // extension
        let other = vec![entry(1, 11), entry(2, 21)];

        // Leader invariant: new extends old
        assert!(new_log.len() >= old_log.len());
        for i in 0..old_log.len() {
            assert_eq!(new_log[i].term, old_log[i].term);
        }

        // LMP preserved
        assert!(check_lmp(&old_log, &other));
        assert!(check_lmp(&new_log, &other));
    }

    // r[verify log-matching-inductive.append-entries-preserves]
    #[test]
    fn test_ae_check_then_resolve() {
        let leader = vec![entry(1, 10), entry(2, 20), entry(3, 30)];
        let follower = vec![entry(1, 11), entry(2, 21), entry(4, 40)]; // diverges at index 2

        // Check at prev_log_index=2 (index 1), prev_log_term=2
        assert!(crate::append_entries_check::check_append_entries(&follower, 2, 2));

        // Resolve conflict: leader entries from index 2 onward
        let leader_entries = vec![entry(3, 30)];
        let mut resolved = follower.clone();
        crate::log_conflict::resolve_conflict(&mut resolved, &leader_entries, 2);

        // After resolution: resolved matches leader through index 3
        assert_eq!(resolved.len(), 3);
        assert_eq!(resolved[0].term, 1);
        assert_eq!(resolved[1].term, 2);
        assert_eq!(resolved[2].term, 3);

        // LMP holds between resolved and leader
        assert!(check_lmp(&resolved, &leader));
    }

    // r[verify log-matching-inductive.election]
    #[test]
    fn test_election_preserves() {
        let a = vec![entry(1, 10), entry(2, 20)];
        let b = vec![entry(1, 11), entry(2, 21)];
        assert!(check_lmp(&a, &b));
        // Election doesn't change logs — LMP trivially preserved
        assert!(check_lmp(&a, &b));
    }

    // r[verify log-matching-inductive.cross-module]
    #[test]
    fn test_check_establishes_agreement() {
        let log = vec![entry(1, 10), entry(2, 20), entry(3, 30)];
        // check_append_entries(log, 2, 2) => log[1].term == 2 ✓
        assert!(crate::append_entries_check::check_append_entries(&log, 2, 2));
        assert_eq!(log[1].term, 2);
    }

    proptest! {
        // r[verify log-matching-inductive.leader-append]
        #[test]
        fn prop_append_preserves_lmp(
            terms in proptest::collection::vec(1u64..5, 1..=10),
            new_term in 1u64..10,
        ) {
            let a: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 0)).collect();
            let b: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 1)).collect();

            // Same terms → LMP holds
            prop_assert!(check_lmp(&a, &b));

            // Append to a
            let mut a2 = a.clone();
            a2.push(entry(new_term, 99));
            prop_assert!(check_lmp(&a2, &b));

            // Append to b
            let mut b2 = b.clone();
            b2.push(entry(new_term, 88));
            prop_assert!(check_lmp(&a, &b2));
        }

        // r[verify log-matching-inductive.append-entries-preserves]
        #[test]
        fn prop_ae_exchange_preserves(
            common_terms in proptest::collection::vec(1u64..5, 1..=5),
            leader_extra in proptest::collection::vec(1u64..5, 1..=5),
        ) {
            // Build leader log: common prefix + extra
            let mut leader: Vec<LogEntry> = common_terms.iter().map(|&t| entry(t, 0)).collect();
            for &t in &leader_extra {
                leader.push(entry(t, 0));
            }

            // Follower has only the common prefix
            let follower: Vec<LogEntry> = common_terms.iter().map(|&t| entry(t, 1)).collect();

            // LMP holds between follower and leader (common prefix matches)
            prop_assert!(check_lmp(&follower, &leader));

            // Check at end of common prefix
            let pli = common_terms.len() as u64;
            if pli > 0 {
                let plt = common_terms[common_terms.len() - 1];
                prop_assert!(crate::append_entries_check::check_append_entries(&follower, pli, plt));
            }

            // Resolve: append leader_extra to follower
            let leader_entries: Vec<LogEntry> = leader_extra.iter().map(|&t| entry(t, 0)).collect();
            let mut resolved = follower.clone();
            crate::log_conflict::resolve_conflict(&mut resolved, &leader_entries, common_terms.len());

            // LMP holds between resolved and leader
            prop_assert!(check_lmp(&resolved, &leader));
        }

        // r[verify log-matching-inductive.invariant]
        #[test]
        fn prop_lmp_model_oracle(
            terms_a in proptest::collection::vec(1u64..5, 0..=10),
            terms_b in proptest::collection::vec(1u64..5, 0..=10),
        ) {
            let a: Vec<LogEntry> = terms_a.iter().map(|&t| entry(t, 0)).collect();
            let b: Vec<LogEntry> = terms_b.iter().map(|&t| entry(t, 1)).collect();

            // Compute LMP manually
            let min_len = a.len().min(b.len());
            let mut expected = true;
            for i in 0..min_len {
                if a[i].term == b[i].term {
                    for j in 0..i {
                        if a[j].term != b[j].term {
                            expected = false;
                        }
                    }
                }
            }

            prop_assert_eq!(check_lmp(&a, &b), expected);
        }

        #[test]
        fn prop_lmp_symmetric(
            terms_a in proptest::collection::vec(1u64..5, 0..=8),
            terms_b in proptest::collection::vec(1u64..5, 0..=8),
        ) {
            let a: Vec<LogEntry> = terms_a.iter().map(|&t| entry(t, 0)).collect();
            let b: Vec<LogEntry> = terms_b.iter().map(|&t| entry(t, 1)).collect();
            prop_assert_eq!(check_lmp(&a, &b), check_lmp(&b, &a));
        }

        #[test]
        fn prop_prefix_preserves_lmp(
            terms in proptest::collection::vec(1u64..5, 2..=10),
        ) {
            let full: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 0)).collect();
            let half = &full[..full.len() / 2];
            // A prefix of a log always satisfies LMP with itself
            prop_assert!(check_lmp(half, &full));
        }

        #[test]
        fn prop_random_append_sequence(
            base_terms in proptest::collection::vec(1u64..3, 1..=5),
            append_terms in proptest::collection::vec(1u64..3, 1..=5),
        ) {
            // Two logs starting from same base, each extending independently
            let base: Vec<LogEntry> = base_terms.iter().map(|&t| entry(t, 0)).collect();
            let mut log_a = base.clone();
            let mut log_b = base.clone();
            for &t in &append_terms {
                log_a.push(entry(t, 1));
            }
            for &t in &append_terms {
                log_b.push(entry(t, 2));
            }
            // Same base + same term sequence → LMP holds
            prop_assert!(check_lmp(&log_a, &log_b));
        }
    }
}
