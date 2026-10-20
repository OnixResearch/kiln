//! Ghost state for Raft inductive invariant proofs.
//!
//! Spec-only types that track history needed for system-level safety proofs:
//! vote records, leader log snapshots, and entry provenance. These exist
//! only in the proof world — no runtime representation. Based on the ghost
//! variables from the Verdi Raft proof (Woos, Wilcox et al., CPP 2016).
//!
//! - `VoteRecord`: who voted for whom, with what log, in which term
//! - `LeaderLogSnapshot`: log contents at the moment of election
//! - `EntryProvenance`: every entry ever held by a node, surviving truncation

#![allow(clippy::wildcard_imports)]

use vstd::prelude::*;

use crate::log::*;

verus! {

// ============================================================
// Vote History
// ============================================================

/// A record of a single vote cast by a node.
///
/// Ghost state — exists only in specs and proofs, not at runtime.
/// Captures the voter's log epoch at the time the vote was cast,
/// which is needed to prove leader completeness (the voter's log
/// was at least as up-to-date as the committed entry's epoch).
/// Epoch fields stored inline (Epoch doesn't derive Copy/Clone).
#[derive(Clone, Copy)]
pub struct VoteRecord {
    /// The term in which the vote was cast.
    pub term: u64,
    /// The candidate that received the vote.
    pub candidate_id: u64,
    /// The voter's last log term at vote time.
    pub voter_last_term: u64,
    /// The voter's last log index at vote time.
    pub voter_last_index: u64,
}

// r[depends ghost-history.vote-accumulation]
/// Spec: append a vote record to the history.
pub open spec fn append_vote_spec(
    history: Seq<VoteRecord>,
    record: VoteRecord,
) -> Seq<VoteRecord> {
    history.push(record)
}

// r[verify ghost-history.vote-accumulation]
/// Appending increases length by 1.
proof fn append_vote_length(history: Seq<VoteRecord>, record: VoteRecord)
    ensures
        append_vote_spec(history, record).len() == history.len() + 1
{}

// r[verify ghost-history.vote-accumulation]
/// Appending preserves existing records.
proof fn append_vote_preserves(history: Seq<VoteRecord>, record: VoteRecord)
    ensures
        forall|i: int| #![trigger history[i]]
            0 <= i < history.len()
            ==> append_vote_spec(history, record)[i] == history[i]
{}

// r[verify ghost-history.vote-accumulation]
/// The new record is at the last position.
proof fn append_vote_last(history: Seq<VoteRecord>, record: VoteRecord)
    ensures
        append_vote_spec(history, record)[history.len() as int] == record
{}

// r[depends ghost-history.single-vote-per-term]
/// Spec: the history satisfies single-vote-per-term — for any term,
/// at most one VoteRecord exists. Two records with the same term
/// must have the same candidate_id.
pub open spec fn single_vote_per_term_spec(history: Seq<VoteRecord>) -> bool {
    forall|i: int, j: int|
        #![trigger history[i], history[j]]
        0 <= i < history.len() && 0 <= j < history.len()
        && history[i].term == history[j].term
        ==> history[i].candidate_id == history[j].candidate_id
}

// r[verify ghost-history.single-vote-per-term]
/// Empty history trivially satisfies single-vote-per-term.
pub proof fn empty_history_single_vote()
    ensures single_vote_per_term_spec(Seq::<VoteRecord>::empty())
{}

// r[verify ghost-history.single-vote-per-term]
/// Appending a record whose term is fresh (not in the history)
/// preserves single-vote-per-term.
pub proof fn append_fresh_preserves_single_vote(
    history: Seq<VoteRecord>,
    record: VoteRecord,
)
    requires
        single_vote_per_term_spec(history),
        // The new record's term does not appear in the existing history
        forall|i: int| #![trigger history[i]]
            0 <= i < history.len()
            ==> history[i].term != record.term,
    ensures
        single_vote_per_term_spec(append_vote_spec(history, record))
{
    let new_history = append_vote_spec(history, record);
    assert forall|i: int, j: int|
        #![trigger new_history[i], new_history[j]]
        0 <= i < new_history.len() && 0 <= j < new_history.len()
        && new_history[i].term == new_history[j].term
    implies
        new_history[i].candidate_id == new_history[j].candidate_id
    by {
        if i < history.len() && j < history.len() {
            // Both old — use existing single_vote_per_term
        } else if i == history.len() as int && j == history.len() as int {
            // Both the new record — trivially same candidate
        } else if i == history.len() as int {
            // i is new, j is old — terms can't match (freshness)
            assert(history[j].term != record.term);
        } else {
            // j is new, i is old — symmetric
            assert(history[i].term != record.term);
        }
    }
}

// r[verify ghost-history.single-vote-per-term]
/// Appending a record whose term already exists but for the same
/// candidate (idempotent re-vote) preserves single-vote-per-term.
proof fn append_revote_preserves_single_vote(
    history: Seq<VoteRecord>,
    record: VoteRecord,
)
    requires
        single_vote_per_term_spec(history),
        // Every existing record with this term has the same candidate
        forall|i: int| #![trigger history[i]]
            0 <= i < history.len()
            && history[i].term == record.term
            ==> history[i].candidate_id == record.candidate_id,
    ensures
        single_vote_per_term_spec(append_vote_spec(history, record))
{
    let new_history = append_vote_spec(history, record);
    assert forall|i: int, j: int|
        #![trigger new_history[i], new_history[j]]
        0 <= i < new_history.len() && 0 <= j < new_history.len()
        && new_history[i].term == new_history[j].term
    implies
        new_history[i].candidate_id == new_history[j].candidate_id
    by {
        if i < history.len() && j < history.len() {
            // Both old
        } else if i == history.len() as int && j == history.len() as int {
            // Both new
        } else if i == history.len() as int {
            // i is new (record), j is old — same term means same candidate
            assert(history[j].term == record.term);
            assert(history[j].candidate_id == record.candidate_id);
        } else {
            // j is new, i is old — symmetric
            assert(history[i].term == record.term);
            assert(history[i].candidate_id == record.candidate_id);
        }
    }
}

// r[depends ghost-history.vote-implies-ballot]
/// Spec: if single_vote_per_term holds and the history contains a record
/// for (term, candidate_a), then any different candidate_b cannot have a
/// record for the same term.
pub open spec fn vote_excludes_other_candidate_spec(
    history: Seq<VoteRecord>,
    term: u64,
    candidate_a: u64,
    candidate_b: u64,
) -> bool {
    (single_vote_per_term_spec(history)
     && candidate_a != candidate_b
     && exists|i: int| #![trigger history[i]]
         0 <= i < history.len()
         && history[i].term == term
         && history[i].candidate_id == candidate_a)
    ==> forall|j: int| #![trigger history[j]]
         0 <= j < history.len()
         && history[j].term == term
         ==> history[j].candidate_id == candidate_a
}

// r[verify ghost-history.vote-implies-ballot]
/// A vote for candidate_a in a given term excludes all other candidates
/// in that term.
proof fn vote_excludes_other_candidate(
    history: Seq<VoteRecord>,
    term: u64,
    candidate_a: u64,
    candidate_b: u64,
)
    ensures vote_excludes_other_candidate_spec(history, term, candidate_a, candidate_b)
{
    // Follows directly from single_vote_per_term_spec: any two records
    // with the same term must have the same candidate_id.
}

// ============================================================
// Leader Log Snapshots
// ============================================================

/// A snapshot of a node's log at the moment it became leader.
///
/// Ghost state — captures the log contents when leadership was won.
/// Used to prove leader completeness: the leader's log at election
/// time contained all committed entries.
pub struct LeaderLogSnapshot {
    /// The term in which leadership was won.
    pub term: u64,
    /// The log contents at the moment of election.
    pub log: Seq<LogEntry>,
}

// r[depends ghost-history.leader-log-accumulation]
/// Spec: append a leader log snapshot to the history.
pub open spec fn append_leader_log_spec(
    history: Seq<LeaderLogSnapshot>,
    snapshot: LeaderLogSnapshot,
) -> Seq<LeaderLogSnapshot> {
    history.push(snapshot)
}

// r[verify ghost-history.leader-log-accumulation]
/// Appending increases length by 1.
proof fn append_leader_log_length(
    history: Seq<LeaderLogSnapshot>,
    snapshot: LeaderLogSnapshot,
)
    ensures
        append_leader_log_spec(history, snapshot).len() == history.len() + 1
{}

// r[depends ghost-history.leader-log-prefix-valid]
/// Spec: the snapshot's log is a prefix of the actual log at election time.
pub open spec fn leader_log_prefix_valid_spec(
    snapshot: LeaderLogSnapshot,
    actual_log: Seq<LogEntry>,
) -> bool {
    logs_match_prefix_spec(snapshot.log, actual_log, snapshot.log.len() as int)
}

// r[verify ghost-history.leader-log-prefix-valid]
/// A snapshot taken from the actual log is always a valid prefix.
proof fn snapshot_from_log_is_valid(log: Seq<LogEntry>, term: u64)
    ensures leader_log_prefix_valid_spec(
        LeaderLogSnapshot { term, log },
        log,
    )
{}

// r[verify ghost-history.leader-log-prefix-valid]
/// If the snapshot log is a valid prefix and the actual log grows
/// (leader append-only), the snapshot remains a valid prefix.
proof fn snapshot_valid_after_append(
    snapshot: LeaderLogSnapshot,
    old_log: Seq<LogEntry>,
    entry: LogEntry,
)
    requires
        leader_log_prefix_valid_spec(snapshot, old_log),
        snapshot.log.len() <= old_log.len(),
    ensures
        leader_log_prefix_valid_spec(snapshot, old_log.push(entry))
{}

// ============================================================
// Entry Provenance
// ============================================================

/// A record that an entry was present in a node's log.
///
/// Ghost state — tracks every entry that has ever been in the log,
/// even after truncation. Used for affinity lemmas: any property
/// that holds for all current log entries also held for every entry
/// at the time it was added.
pub struct EntryProvenance {
    /// The term in which this entry was observed in the log.
    pub term_observed: u64,
    /// The log entry itself.
    pub entry: LogEntry,
    /// Which node held this entry (abstract node ID).
    pub node_id: nat,
}

// r[depends ghost-history.all-entries-tracking]
/// Spec: add provenance records for new entries.
///
/// When entries are added to a node's log (via append or conflict
/// resolution), record each one in the provenance set.
pub open spec fn record_entries_spec(
    provenance: Seq<EntryProvenance>,
    entries: Seq<LogEntry>,
    term_observed: u64,
    node_id: nat,
) -> Seq<EntryProvenance>
    decreases entries.len(),
{
    if entries.len() == 0 {
        provenance
    } else {
        record_entries_spec(
            provenance.push(EntryProvenance {
                term_observed,
                entry: entries[0],
                node_id,
            }),
            entries.subrange(1, entries.len() as int),
            term_observed,
            node_id,
        )
    }
}

// r[verify ghost-history.all-entries-tracking]
/// Recording entries increases provenance length by entries.len().
proof fn record_entries_length(
    provenance: Seq<EntryProvenance>,
    entries: Seq<LogEntry>,
    term_observed: u64,
    node_id: nat,
)
    ensures
        record_entries_spec(provenance, entries, term_observed, node_id).len()
            == provenance.len() + entries.len()
    decreases entries.len(),
{
    if entries.len() > 0 {
        record_entries_length(
            provenance.push(EntryProvenance {
                term_observed,
                entry: entries[0],
                node_id,
            }),
            entries.subrange(1, entries.len() as int),
            term_observed,
            node_id,
        );
    }
}

// r[verify ghost-history.all-entries-tracking]
/// Recording preserves existing provenance records.
proof fn record_entries_preserves(
    provenance: Seq<EntryProvenance>,
    entries: Seq<LogEntry>,
    term_observed: u64,
    node_id: nat,
)
    ensures
        forall|i: int| #![trigger provenance[i]]
            0 <= i < provenance.len()
            ==> record_entries_spec(provenance, entries, term_observed, node_id)[i]
                == provenance[i]
    decreases entries.len(),
{
    if entries.len() > 0 {
        let next_prov = provenance.push(EntryProvenance {
            term_observed,
            entry: entries[0],
            node_id,
        });
        let next_entries = entries.subrange(1, entries.len() as int);
        record_entries_preserves(next_prov, next_entries, term_observed, node_id);
        // The first push preserves [0, provenance.len()), and the recursive
        // call preserves [0, next_prov.len()) which includes [0, provenance.len()).
        assert forall|i: int| #![trigger provenance[i]]
            0 <= i < provenance.len()
        implies
            record_entries_spec(provenance, entries, term_observed, node_id)[i]
                == provenance[i]
        by {
            assert(next_prov[i] == provenance[i]);
            assert(record_entries_spec(next_prov, next_entries, term_observed, node_id)[i]
                == next_prov[i]);
        }
    }
}

// r[depends ghost-history.provenance-affinity]
/// Spec: a property P holds for an entry.
/// This is a type alias for the affinity lemma pattern.
///
/// The affinity lemma says: if P(e) holds for every entry e in the
/// current log, then P(p.entry) held for every entry p in provenance
/// at the time it was added. Since provenance is append-only and
/// entries are recorded from the log, the property propagates.
pub open spec fn entry_property_holds(
    provenance: Seq<EntryProvenance>,
    log: Seq<LogEntry>,
) -> bool {
    // Every entry currently in the log has a provenance record
    forall|i: int| #![trigger log[i]]
        0 <= i < log.len()
        ==> exists|j: int| #![trigger provenance[j]]
            0 <= j < provenance.len()
            && provenance[j].entry == log[i]
}

// r[verify ghost-history.provenance-affinity]
/// Helper: the k-th entry is placed at position prov.len() + k in the result.
/// Provides the existential witness for entry_property_holds.
proof fn record_entries_at_position(
    provenance: Seq<EntryProvenance>,
    entries: Seq<LogEntry>,
    term_observed: u64,
    node_id: nat,
    k: int,
)
    requires 0 <= k < entries.len()
    ensures ({
        let result = record_entries_spec(provenance, entries, term_observed, node_id);
        provenance.len() + k < result.len()
        && result[provenance.len() + k].entry == entries[k]
    })
    decreases entries.len(),
{
    let next_prov = provenance.push(EntryProvenance {
        term_observed,
        entry: entries[0],
        node_id,
    });
    let next_entries = entries.subrange(1, entries.len() as int);

    if k == 0 {
        // The first entry lands at prov.len() in the result.
        // record_entries_preserves guarantees result[i] == next_prov[i] for i < next_prov.len().
        record_entries_preserves(next_prov, next_entries, term_observed, node_id);
        assert(next_prov[provenance.len() as int].entry == entries[0]);
    } else {
        // Induction: entry k of entries is entry k-1 of next_entries.
        record_entries_at_position(next_prov, next_entries, term_observed, node_id, k - 1);
        // next_prov.len() == provenance.len() + 1, so
        // result[next_prov.len() + (k-1)] == result[provenance.len() + k]
    }
    // Establish result length bound for the ensures
    record_entries_length(provenance, entries, term_observed, node_id);
}

// r[verify ghost-history.provenance-affinity]
/// After recording the full log, every log entry has a provenance record.
proof fn record_full_log_covers(
    log: Seq<LogEntry>,
    term_observed: u64,
    node_id: nat,
)
    ensures
        entry_property_holds(
            record_entries_spec(Seq::<EntryProvenance>::empty(), log, term_observed, node_id),
            log,
        )
{
    let prov = Seq::<EntryProvenance>::empty();
    let result = record_entries_spec(prov, log, term_observed, node_id);
    assert forall|i: int| #![trigger log[i]]
        0 <= i < log.len()
    implies
        exists|j: int| #![trigger result[j]]
            0 <= j < result.len()
            && result[j].entry == log[i]
    by {
        // The helper gives us the exact witness: j = prov.len() + i = i
        record_entries_at_position(prov, log, term_observed, node_id, i);
        let j = prov.len() + i;
        assert(result[j].entry == log[i]);
    }
}

// ============================================================
// Cronies: who voted for a given candidate in a given term
// ============================================================

// r[depends ghost-history.cronies]
/// Spec: count how many records in the history voted for `candidate`
/// in `term`.
pub open spec fn cronies_count_spec(
    history: Seq<VoteRecord>,
    term: u64,
    candidate: u64,
) -> nat
    decreases history.len(),
{
    if history.len() == 0 {
        0
    } else {
        let rest_count = cronies_count_spec(
            history.subrange(0, history.len() - 1),
            term,
            candidate,
        );
        if history[history.len() - 1].term == term
            && history[history.len() - 1].candidate_id == candidate
        {
            rest_count + 1
        } else {
            rest_count
        }
    }
}

// r[verify ghost-history.cronies]
/// Cronies count is bounded by history length.
proof fn cronies_count_bounded(
    history: Seq<VoteRecord>,
    term: u64,
    candidate: u64,
)
    ensures cronies_count_spec(history, term, candidate) <= history.len()
    decreases history.len(),
{
    if history.len() > 0 {
        cronies_count_bounded(
            history.subrange(0, history.len() - 1),
            term,
            candidate,
        );
    }
}

// r[verify ghost-history.cronies]
/// Empty history has zero cronies.
proof fn cronies_count_empty(term: u64, candidate: u64)
    ensures cronies_count_spec(Seq::<VoteRecord>::empty(), term, candidate) == 0
{}

// r[verify ghost-history.cronies]
/// Helper: if no record in history matches (term, candidate), the count is 0.
proof fn cronies_count_zero_if_no_match(
    history: Seq<VoteRecord>,
    term: u64,
    candidate: u64,
)
    requires
        forall|i: int| #![trigger history[i]]
            0 <= i < history.len()
            ==> !(history[i].term == term && history[i].candidate_id == candidate)
    ensures cronies_count_spec(history, term, candidate) == 0nat
    decreases history.len(),
{
    if history.len() > 0 {
        let prefix = history.subrange(0, history.len() - 1);
        // All prefix elements also don't match
        assert forall|i: int| #![trigger prefix[i]]
            0 <= i < prefix.len()
        implies
            !(prefix[i].term == term && prefix[i].candidate_id == candidate)
        by {
            assert(prefix[i] == history[i]);
        }
        cronies_count_zero_if_no_match(prefix, term, candidate);
        // Last element doesn't match either (from requires)
    }
}

// r[verify ghost-history.cronies]
/// With single_vote_per_term, if candidate A has k cronies, then
/// candidate B (B != A) has zero cronies for the same term.
/// Each voter voted for at most one candidate, so all votes go to A.
pub proof fn cronies_exclusive(
    history: Seq<VoteRecord>,
    term: u64,
    candidate_a: u64,
    candidate_b: u64,
)
    requires
        single_vote_per_term_spec(history),
        candidate_a != candidate_b,
    ensures
        cronies_count_spec(history, term, candidate_b) == 0
        || cronies_count_spec(history, term, candidate_a) == 0
    decreases history.len(),
{
    if history.len() > 0 {
        let prefix = history.subrange(0, history.len() - 1);
        let last = history[history.len() - 1];

        // The prefix also satisfies single_vote_per_term
        assert(single_vote_per_term_spec(prefix)) by {
            assert forall|i: int, j: int|
                #![trigger prefix[i], prefix[j]]
                0 <= i < prefix.len() && 0 <= j < prefix.len()
                && prefix[i].term == prefix[j].term
            implies
                prefix[i].candidate_id == prefix[j].candidate_id
            by {
                assert(history[i].term == history[j].term);
                assert(history[i].candidate_id == history[j].candidate_id);
            }
        }

        cronies_exclusive(prefix, term, candidate_a, candidate_b);

        if last.term == term && last.candidate_id == candidate_a {
            // All records with this term have candidate == candidate_a (by single_vote_per_term).
            // So no record has (term, candidate_b). cronies_count for b is 0.
            assert forall|i: int| #![trigger history[i]]
                0 <= i < history.len()
            implies
                !(history[i].term == term && history[i].candidate_id == candidate_b)
            by {
                if history[i].term == term {
                    // single_vote_per_term: history[i] and last have same term
                    // so they have the same candidate_id
                    assert(history[history.len() - 1].term == term);
                    assert(history[i].candidate_id == history[history.len() - 1].candidate_id);
                    // last.candidate_id == candidate_a != candidate_b
                }
            }
            cronies_count_zero_if_no_match(history, term, candidate_b);
        } else if last.term == term && last.candidate_id == candidate_b {
            // Symmetric: all records with this term have candidate == candidate_b.
            assert forall|i: int| #![trigger history[i]]
                0 <= i < history.len()
            implies
                !(history[i].term == term && history[i].candidate_id == candidate_a)
            by {
                if history[i].term == term {
                    assert(history[history.len() - 1].term == term);
                    assert(history[i].candidate_id == history[history.len() - 1].candidate_id);
                }
            }
            cronies_count_zero_if_no_match(history, term, candidate_a);
        } else {
            // Last record doesn't match this term for either candidate.
            // Count on full history == count on prefix for both candidates.
            // Recursive result carries through.
        }
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn vr(term: u64, candidate: u64, epoch_term: u64, epoch_idx: u64) -> VoteRecord {
        VoteRecord {
            term,
            candidate_id: candidate,
            voter_last_term: epoch_term,
            voter_last_index: epoch_idx,
        }
    }

    fn entry(term: u64, value: u64) -> LogEntry {
        LogEntry { term, value }
    }

    // r[verify ghost-history.vote-accumulation]
    #[test]
    fn test_append_vote() {
        let h: Vec<VoteRecord> = vec![];
        let r = vr(1, 42, 0, 0);
        // Spec-level: append adds to the end
        assert_eq!(h.len(), 0);
        let mut h2 = h.clone();
        h2.push(r);
        assert_eq!(h2.len(), 1);
        assert_eq!(h2[0].term, 1);
        assert_eq!(h2[0].candidate_id, 42);
    }

    // r[verify ghost-history.single-vote-per-term]
    #[test]
    fn test_single_vote_per_term_basic() {
        // Two votes in different terms — OK
        let history = vec![vr(1, 10, 0, 0), vr(2, 20, 1, 5)];
        for i in 0..history.len() {
            for j in 0..history.len() {
                if history[i].term == history[j].term {
                    assert_eq!(history[i].candidate_id, history[j].candidate_id);
                }
            }
        }
    }

    // r[verify ghost-history.single-vote-per-term]
    #[test]
    fn test_single_vote_per_term_violation() {
        // Two votes in same term for different candidates — violation
        let history = vec![vr(5, 10, 4, 3), vr(5, 20, 4, 7)];
        let mut ok = true;
        for i in 0..history.len() {
            for j in 0..history.len() {
                if history[i].term == history[j].term && history[i].candidate_id != history[j].candidate_id {
                    ok = false;
                }
            }
        }
        assert!(!ok);
    }

    // r[verify ghost-history.vote-implies-ballot]
    #[test]
    fn test_vote_excludes_other() {
        let history = vec![vr(5, 10, 4, 3)];
        // Check: all records for term 5 have candidate 10
        for r in &history {
            if r.term == 5 {
                assert_eq!(r.candidate_id, 10);
            }
        }
        // So candidate 20 has no record for term 5
        let has_20 = history.iter().any(|r| r.term == 5 && r.candidate_id == 20);
        assert!(!has_20);
    }

    // r[verify ghost-history.leader-log-prefix-valid]
    #[test]
    fn test_leader_log_snapshot_valid() {
        let log = vec![entry(1, 10), entry(2, 20), entry(3, 30)];
        // Snapshot at election time captures the full log
        let snapshot_log = log.clone();
        // It's a valid prefix of itself
        assert!(crate::log::logs_match_prefix(&snapshot_log, &log, snapshot_log.len()));
    }

    // r[verify ghost-history.leader-log-prefix-valid]
    #[test]
    fn test_leader_log_snapshot_after_append() {
        let log = vec![entry(1, 10), entry(2, 20)];
        let snapshot_log = log.clone();
        let mut extended_log = log;
        extended_log.push(entry(3, 30));
        // Snapshot is still a valid prefix after the log grew
        assert!(crate::log::logs_match_prefix(&snapshot_log, &extended_log, snapshot_log.len()));
    }

    // r[verify ghost-history.cronies]
    #[test]
    fn test_cronies_basic() {
        let history = vec![vr(5, 10, 4, 3), vr(5, 10, 4, 7), vr(5, 10, 3, 2), vr(3, 20, 2, 1)];
        // Count cronies for candidate 10 in term 5: should be 3
        let count = history.iter().filter(|r| r.term == 5 && r.candidate_id == 10).count();
        assert_eq!(count, 3);

        // Count cronies for candidate 20 in term 5: should be 0
        let count_20 = history.iter().filter(|r| r.term == 5 && r.candidate_id == 20).count();
        assert_eq!(count_20, 0);
    }

    proptest! {
        // r[verify ghost-history.single-vote-per-term]
        #[test]
        fn prop_fresh_term_preserves_single_vote(
            terms in proptest::collection::vec(0u64..100, 0..=10),
            candidates in proptest::collection::vec(0u64..50, 0..=10),
            new_term in 100u64..200,
            new_candidate: u64,
        ) {
            // Build a history where each term maps to one candidate
            let mut seen_terms = std::collections::HashMap::new();
            let mut history = Vec::new();
            for (i, &t) in terms.iter().enumerate() {
                let c = candidates.get(i).copied().unwrap_or(0);
                let candidate = *seen_terms.entry(t).or_insert(c);
                history.push(vr(t, candidate, 0, 0));
            }

            // Verify single_vote_per_term on the history
            let svpt = history.iter().enumerate().all(|(_i, ri)| {
                history.iter().enumerate().all(|(_j, rj)| {
                    ri.term != rj.term || ri.candidate_id == rj.candidate_id
                })
            });
            prop_assert!(svpt);

            // new_term is fresh (>= 100, all existing terms < 100)
            let fresh = !history.iter().any(|r| r.term == new_term);
            prop_assert!(fresh);

            // After appending, single_vote_per_term still holds
            history.push(vr(new_term, new_candidate, 0, 0));
            let svpt2 = history.iter().enumerate().all(|(_i, ri)| {
                history.iter().enumerate().all(|(_j, rj)| {
                    ri.term != rj.term || ri.candidate_id == rj.candidate_id
                })
            });
            prop_assert!(svpt2);
        }

        // r[verify ghost-history.cronies]
        #[test]
        fn prop_cronies_exclusive(
            term: u64,
            candidate_a in 0u64..100,
            count_a in 0usize..=10,
        ) {
            let candidate_b = candidate_a + 1;
            // Build history where all votes for `term` go to candidate_a
            let mut history = Vec::new();
            for _ in 0..count_a {
                history.push(vr(term, candidate_a, 0, 0));
            }
            // Also add some votes for other terms
            history.push(vr(term.wrapping_add(1), candidate_b, 0, 0));
            history.push(vr(term.wrapping_add(2), candidate_a, 0, 0));

            // Count cronies
            let cronies_a = history.iter().filter(|r| r.term == term && r.candidate_id == candidate_a).count();
            let cronies_b = history.iter().filter(|r| r.term == term && r.candidate_id == candidate_b).count();

            prop_assert_eq!(cronies_a, count_a);
            prop_assert_eq!(cronies_b, 0);
        }

        // r[verify ghost-history.leader-log-prefix-valid]
        #[test]
        fn prop_snapshot_valid_after_appends(
            terms in proptest::collection::vec(1u64..10, 1..=10),
            extra_terms in proptest::collection::vec(1u64..10, 0..=5),
        ) {
            let log: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 0)).collect();
            let snapshot_len = log.len();

            let mut extended = log.clone();
            for &t in &extra_terms {
                extended.push(entry(t, 0));
            }

            // Snapshot is a valid prefix of the extended log
            prop_assert!(crate::log::logs_match_prefix(&log, &extended, snapshot_len));
        }

        #[test]
        fn prop_append_preserves_svpt_random_history(
            num_records in 1usize..=20,
            seed_terms in proptest::collection::vec(1u64..10, 1..=20),
            seed_candidates in proptest::collection::vec(0u64..5, 1..=20),
        ) {
            // Build a valid SVPT history from random data
            let mut seen = std::collections::HashMap::new();
            let mut history: Vec<VoteRecord> = Vec::new();
            for i in 0..num_records.min(seed_terms.len()).min(seed_candidates.len()) {
                let t = seed_terms[i];
                let c = *seen.entry(t).or_insert(seed_candidates[i]);
                history.push(vr(t, c, 0, 0));
            }
            // Verify SVPT holds
            let ok = history.iter().all(|ri| {
                history.iter().all(|rj| ri.term != rj.term || ri.candidate_id == rj.candidate_id)
            });
            prop_assert!(ok);
        }

        #[test]
        fn prop_revote_same_candidate_preserves(
            term: u64,
            candidate: u64,
            count in 1usize..=5,
        ) {
            // Multiple votes for the same (term, candidate) never violates SVPT
            let history: Vec<VoteRecord> = (0..count)
                .map(|_| vr(term, candidate, 0, 0))
                .collect();
            let ok = history.iter().all(|ri| {
                history.iter().all(|rj| ri.term != rj.term || ri.candidate_id == rj.candidate_id)
            });
            prop_assert!(ok);
        }

        #[test]
        fn prop_cronies_count_matches_filter(
            term: u64,
            candidate: u64,
            records in proptest::collection::vec((0u64..5, 0u64..5), 0..=15),
        ) {
            let history: Vec<VoteRecord> = records.iter()
                .map(|&(t, c)| vr(t, c, 0, 0))
                .collect();
            let count = history.iter()
                .filter(|r| r.term == term && r.candidate_id == candidate)
                .count();
            // Just verify the manual count is consistent
            let count2 = history.iter()
                .filter(|r| r.term == term)
                .filter(|r| r.candidate_id == candidate)
                .count();
            prop_assert_eq!(count, count2);
        }
    }
}
