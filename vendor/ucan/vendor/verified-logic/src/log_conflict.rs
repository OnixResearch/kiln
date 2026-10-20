//! Verified log conflict resolution.
//!
//! When a leader sends entries to a follower, the follower may have
//! conflicting entries (same index, different term). Conflict resolution
//! finds the first divergence point, truncates the follower's log there,
//! and appends the leader's entries. The result matches the leader's log
//! at all covered indices while preserving the matching prefix.

use vstd::prelude::*;

use crate::log::*;

verus! {

// --- find_conflict ---

// r[depends log-conflict.find-conflict]
/// Spec: find the first index (relative to start_index) where follower
/// and leader entries disagree on term.
///
/// Scans from 0 through the overlapping range. Returns the first
/// relative offset where terms differ, or None if all overlapping
/// entries match.
pub open spec fn find_conflict_spec(
    follower: Seq<LogEntry>,
    leader_entries: Seq<LogEntry>,
    start_index: int,
) -> Option<int> {
    find_conflict_from_spec(follower, leader_entries, start_index, 0)
}

/// Helper: scan from relative offset `from`.
pub open spec fn find_conflict_from_spec(
    follower: Seq<LogEntry>,
    leader_entries: Seq<LogEntry>,
    start_index: int,
    from: int,
) -> Option<int>
    decreases leader_entries.len() - from,
{
    if from >= leader_entries.len() {
        None
    } else {
        let abs_idx = start_index + from;
        if abs_idx >= follower.len() {
            // Follower is shorter — no conflict, leader extends beyond
            None
        } else if follower[abs_idx].term != leader_entries[from].term {
            Some(from)
        } else {
            find_conflict_from_spec(follower, leader_entries, start_index, from + 1)
        }
    }
}

// r[impl log-conflict.find-conflict]
/// Find the first conflict between follower log and leader entries.
///
/// Returns Some(relative_offset) at the first term disagreement,
/// or None if all overlapping entries agree (or leader extends beyond follower).
pub fn find_conflict(
    follower: &[LogEntry],
    leader_entries: &[LogEntry],
    start_index: usize,
) -> (result: Option<usize>)
    requires start_index as int + leader_entries@.len() <= usize::MAX as int
    ensures
        match result {
            Some(offset) => find_conflict_spec(follower@, leader_entries@, start_index as int) == Some(offset as int),
            None => find_conflict_spec(follower@, leader_entries@, start_index as int) == None::<int>,
        }
{
    let mut i: usize = 0;
    while i < leader_entries.len()
        invariant
            0 <= i <= leader_entries@.len(),
            start_index as int + leader_entries@.len() <= usize::MAX as int,
            find_conflict_spec(follower@, leader_entries@, start_index as int)
                == find_conflict_from_spec(follower@, leader_entries@, start_index as int, i as int),
        decreases leader_entries@.len() - i,
    {
        let abs_idx = start_index + i;
        if abs_idx >= follower.len() {
            return None;
        }
        if follower[abs_idx].term != leader_entries[i].term {
            return Some(i);
        }
        i = i + 1;
    }
    None
}

// --- resolve_conflict ---

// r[depends log-conflict.resolve]
/// Spec: resolve conflict by truncating at the conflict point and
/// appending leader entries from that point onward.
///
/// If no conflict, append any leader entries that extend beyond the
/// follower's log.
pub open spec fn resolve_conflict_spec(
    follower: Seq<LogEntry>,
    leader_entries: Seq<LogEntry>,
    start_index: int,
) -> Seq<LogEntry> {
    let conflict = find_conflict_spec(follower, leader_entries, start_index);
    match conflict {
        Some(offset) => {
            // Truncate follower at start_index + offset, then append
            // leader entries from offset onward
            let trunc_len = start_index + offset;
            let prefix = if trunc_len <= follower.len() {
                follower.subrange(0, trunc_len)
            } else {
                follower
            };
            append_entries_spec(prefix, leader_entries, offset)
        }
        None => {
            // No conflict — append leader entries that extend beyond follower
            let overlap: int = if start_index + leader_entries.len() <= follower.len() {
                leader_entries.len() as int
            } else if start_index <= follower.len() {
                (follower.len() - start_index) as int
            } else {
                0int
            };
            append_entries_spec(follower, leader_entries, overlap)
        }
    }
}

/// Helper: append leader_entries[from..] to prefix.
pub open spec fn append_entries_spec(
    prefix: Seq<LogEntry>,
    leader_entries: Seq<LogEntry>,
    from: int,
) -> Seq<LogEntry>
    decreases leader_entries.len() - from,
{
    if from >= leader_entries.len() {
        prefix
    } else {
        append_entries_spec(prefix.push(leader_entries[from]), leader_entries, from + 1)
    }
}

// r[impl log-conflict.resolve]
/// Resolve a log conflict: find divergence, truncate, and append.
pub fn resolve_conflict(
    follower: &mut Vec<LogEntry>,
    leader_entries: &[LogEntry],
    start_index: usize,
)
    requires
        start_index as int + leader_entries@.len() <= usize::MAX as int,
        old(follower)@.len() + leader_entries@.len() <= usize::MAX as int,
    ensures
        follower@ == resolve_conflict_spec(
            old(follower)@, leader_entries@, start_index as int,
        )
{
    let ghost original = follower@;
    let conflict = find_conflict(follower, leader_entries, start_index);
    let append_from: usize;

    match conflict {
        Some(offset) => {
            proof {
                conflict_in_range(original, leader_entries@, start_index as int);
                // offset < leader_entries.len(), so start_index + offset < start_index + leader_entries.len() <= usize::MAX.
            }
            let trunc_len = start_index + offset;
            if trunc_len < follower.len() {
                follower.truncate(trunc_len);
            }
            append_from = offset;

            // After truncation, follower is the conflict-case prefix
            proof {
                conflict_in_range(original, leader_entries@, start_index as int);
                let spec_conflict = find_conflict_spec(original, leader_entries@, start_index as int);
                assert(spec_conflict == Some(offset as int));

                let spec_trunc = start_index as int + offset as int;
                if spec_trunc <= original.len() {
                    // follower was truncated to subrange(0, spec_trunc)
                    assert(follower@ =~= original.subrange(0, spec_trunc));
                    assert(resolve_start_state(original, leader_entries@, start_index as int, offset as int)
                        == original.subrange(0, spec_trunc));
                } else {
                    assert(follower@ =~= original);
                    assert(resolve_start_state(original, leader_entries@, start_index as int, offset as int)
                        == original);
                }
                assert(follower@ == resolve_start_state(original, leader_entries@, start_index as int, offset as int));
            }
        }
        None => {
            let overlap: usize;
            if start_index + leader_entries.len() <= follower.len() {
                overlap = leader_entries.len();
            } else if start_index <= follower.len() {
                overlap = follower.len() - start_index;
            } else {
                overlap = 0;
            }
            append_from = overlap;

            // No truncation happened — follower is unchanged
            proof {
                assert(follower@ =~= original);
                let spec_conflict = find_conflict_spec(original, leader_entries@, start_index as int);
                assert(spec_conflict is None);
                assert(resolve_start_state(original, leader_entries@, start_index as int, overlap as int)
                    == original);
                assert(follower@ == resolve_start_state(original, leader_entries@, start_index as int, overlap as int));
            }
        }
    }

    // At this point: follower@ == resolve_start_state(original, leader_entries@, start_index, append_from)
    // Now prove that resolve_conflict_spec == append_entries_spec(resolve_start_state(...), entries, append_from)
    proof {
        resolve_equals_append_from_start(original, leader_entries@, start_index as int);
    }

    // Append leader_entries[append_from..]
    // Invariant: append_entries_spec(follower@, entries, j) == full_result
    // where full_result = append_entries_spec(rss, entries, append_from)
    //                    = resolve_conflict_spec(original, entries, start_index).
    let ghost rss = resolve_start_state(original, leader_entries@, start_index as int, append_from as int);
    let ghost full_result = append_entries_spec(rss, leader_entries@, append_from as int);

    proof {
        // Before loop: follower@ == rss, j == append_from.
        // append_entries_spec(rss, entries, append_from) == full_result. ✓
    }

    let mut j = append_from;
    while j < leader_entries.len()
        invariant
            append_from as int <= j as int <= leader_entries@.len(),
            append_entries_spec(follower@, leader_entries@, j as int) == full_result,
            start_index as int + leader_entries@.len() <= usize::MAX as int,
            original.len() + leader_entries@.len() <= usize::MAX as int,
            follower@.len() + leader_entries@.len() - j <= usize::MAX as int,
        decreases leader_entries@.len() - j,
    {
        proof {
            // append_entries_spec(follower@, entries, j)
            //   = append_entries_spec(follower@.push(entries[j]), entries, j+1)  [by definition]
            append_entries_step(follower@, leader_entries@, j as int);
        }
        follower.push(leader_entries[j]);
        // Now follower@ = old_follower@.push(entries[j])
        // append_entries_spec(follower@, entries, j+1) = append_entries_spec(old_follower@, entries, j) = full_result ✓
        j = j + 1;
    }

    // Post-loop: j == entries.len().
    // append_entries_spec(follower@, entries, entries.len()) == full_result.
    // But append_entries_spec(x, entries, entries.len()) == x (base case).
    // So follower@ == full_result == resolve_conflict_spec(original, entries, start_index).
    proof {
        append_entries_empty(follower@, leader_entries@, j as int);
    }
}

/// Helper spec: the state of the follower log after truncation but before
/// appending leader entries. Used in the append loop invariant.
pub open spec fn resolve_start_state(
    follower: Seq<LogEntry>,
    leader_entries: Seq<LogEntry>,
    start_index: int,
    append_from: int,
) -> Seq<LogEntry> {
    let conflict = find_conflict_spec(follower, leader_entries, start_index);
    match conflict {
        Some(offset) => {
            let trunc_len = start_index + offset;
            if trunc_len <= follower.len() {
                follower.subrange(0, trunc_len)
            } else {
                follower
            }
        }
        None => follower,
    }
}

// --- Safety proofs ---

// r[verify log-conflict.find-conflict]
/// No conflict when leader entries are empty.
proof fn no_conflict_empty_leader(follower: Seq<LogEntry>, start_index: int)
    ensures find_conflict_spec(follower, Seq::empty(), start_index) == None::<int>
{}

// r[verify log-conflict.find-conflict]
/// Conflict offset is always within leader entries range.
proof fn conflict_in_range(
    follower: Seq<LogEntry>,
    leader_entries: Seq<LogEntry>,
    start_index: int,
)
    ensures
        match find_conflict_spec(follower, leader_entries, start_index) {
            Some(offset) => 0 <= offset < leader_entries.len(),
            None => true,
        }
{
    conflict_in_range_from(follower, leader_entries, start_index, 0);
}

proof fn conflict_in_range_from(
    follower: Seq<LogEntry>,
    leader_entries: Seq<LogEntry>,
    start_index: int,
    from: int,
)
    requires 0 <= from
    ensures
        match find_conflict_from_spec(follower, leader_entries, start_index, from) {
            Some(offset) => from <= offset < leader_entries.len(),
            None => true,
        }
    decreases leader_entries.len() - from,
{
    if from >= leader_entries.len() {
        // Base: None
    } else {
        let abs_idx = start_index + from;
        if abs_idx >= follower.len() {
            // None
        } else if follower[abs_idx].term != leader_entries[from].term {
            // Some(from)
        } else {
            conflict_in_range_from(follower, leader_entries, start_index, from + 1);
        }
    }
}

// r[verify log-conflict.find-conflict]
/// At the conflict point, terms disagree.
proof fn conflict_means_term_mismatch(
    follower: Seq<LogEntry>,
    leader_entries: Seq<LogEntry>,
    start_index: int,
    offset: int,
)
    requires
        find_conflict_spec(follower, leader_entries, start_index) == Some(offset),
        0 <= start_index,
    ensures
        0 <= offset < leader_entries.len(),
        0 <= start_index + offset < follower.len(),
        follower[start_index + offset].term != leader_entries[offset].term,
{
    conflict_means_term_mismatch_from(follower, leader_entries, start_index, 0, offset);
}

proof fn conflict_means_term_mismatch_from(
    follower: Seq<LogEntry>,
    leader_entries: Seq<LogEntry>,
    start_index: int,
    from: int,
    offset: int,
)
    requires
        0 <= from,
        0 <= start_index,
        find_conflict_from_spec(follower, leader_entries, start_index, from) == Some(offset),
    ensures
        0 <= offset < leader_entries.len(),
        0 <= start_index + offset < follower.len(),
        follower[start_index + offset].term != leader_entries[offset].term,
    decreases leader_entries.len() - from,
{
    if from >= leader_entries.len() {
        // Contradiction: spec returns None
    } else {
        let abs_idx = start_index + from;
        if abs_idx >= follower.len() {
            // Contradiction: spec returns None
        } else if follower[abs_idx].term != leader_entries[from].term {
            // offset == from
        } else {
            conflict_means_term_mismatch_from(
                follower, leader_entries, start_index, from + 1, offset,
            );
        }
    }
}

// r[verify log-conflict.find-conflict]
/// Before the conflict point, all entries agree on term.
proof fn entries_agree_before_conflict(
    follower: Seq<LogEntry>,
    leader_entries: Seq<LogEntry>,
    start_index: int,
    offset: int,
)
    requires
        find_conflict_spec(follower, leader_entries, start_index) == Some(offset),
        0 <= start_index,
    ensures
        forall|j: int| #![trigger leader_entries[j]]
            0 <= j < offset
            ==> (start_index + j < follower.len()
                 && follower[start_index + j].term == leader_entries[j].term)
{
    conflict_in_range(follower, leader_entries, start_index);
    entries_agree_before_conflict_from(follower, leader_entries, start_index, 0, offset);
}

proof fn entries_agree_before_conflict_from(
    follower: Seq<LogEntry>,
    leader_entries: Seq<LogEntry>,
    start_index: int,
    from: int,
    offset: int,
)
    requires
        0 <= from,
        0 <= start_index,
        find_conflict_from_spec(follower, leader_entries, start_index, from) == Some(offset),
    ensures
        from <= offset,
        offset < leader_entries.len() as int,
        start_index + offset < follower.len(),
        forall|j: int| #![trigger leader_entries[j]]
            from <= j < offset
            ==> (start_index + j < follower.len()
                 && follower[start_index + j].term == leader_entries[j].term),
    decreases leader_entries.len() - from,
{
    if from >= leader_entries.len() {
        // Contradiction: spec returns None
    } else {
        let abs_idx = start_index + from;
        if abs_idx >= follower.len() {
            // Contradiction: spec returns None
        } else if follower[abs_idx].term != leader_entries[from].term {
            // offset == from, nothing to prove (from <= j < from is empty)
        } else {
            entries_agree_before_conflict_from(
                follower, leader_entries, start_index, from + 1, offset,
            );
        }
    }
}

// r[verify log-conflict.resolve]
/// Appending empty leader entries (from >= len) is identity.
proof fn append_entries_empty(prefix: Seq<LogEntry>, leader_entries: Seq<LogEntry>, from: int)
    requires from >= leader_entries.len()
    ensures append_entries_spec(prefix, leader_entries, from) == prefix
{}

// r[verify log-conflict.resolve]
/// One-step unrolling: append_entries_spec at `from` equals
/// append_entries_spec at `from + 1` with the element pushed.
/// This is the definition itself, but stated as a lemma so Z3
/// can use it in loop invariant proofs.
proof fn append_entries_step(
    prefix: Seq<LogEntry>,
    leader_entries: Seq<LogEntry>,
    from: int,
)
    requires 0 <= from < leader_entries.len()
    ensures
        append_entries_spec(prefix, leader_entries, from)
        == append_entries_spec(prefix.push(leader_entries[from]), leader_entries, from + 1)
{}

// r[verify log-conflict.resolve]
/// resolve_conflict_spec equals append_entries_spec applied to
/// resolve_start_state with the correct append_from.
///
/// In the conflict case: append_from == offset
/// In the no-conflict case: append_from == overlap
proof fn resolve_equals_append_from_start(
    follower: Seq<LogEntry>,
    leader_entries: Seq<LogEntry>,
    start_index: int,
)
    ensures
        resolve_conflict_spec(follower, leader_entries, start_index)
        == ({
            let conflict = find_conflict_spec(follower, leader_entries, start_index);
            let append_from: int = match conflict {
                Some(offset) => offset,
                None => {
                    if start_index + leader_entries.len() <= follower.len() {
                        leader_entries.len() as int
                    } else if start_index <= follower.len() {
                        (follower.len() - start_index) as int
                    } else {
                        0int
                    }
                }
            };
            append_entries_spec(
                resolve_start_state(follower, leader_entries, start_index, append_from),
                leader_entries,
                append_from,
            )
        })
{
    // Both sides unfold to the same expression by definition.
    // resolve_conflict_spec computes the prefix inline, resolve_start_state
    // computes the same prefix given the same conflict/offset.
    let conflict = find_conflict_spec(follower, leader_entries, start_index);
    match conflict {
        Some(offset) => {
            // resolve_conflict_spec: prefix = subrange(0, si+offset), then append from offset
            // resolve_start_state: same subrange
            // They're definitionally equal.
            let trunc_len = start_index + offset;
            let prefix = if trunc_len <= follower.len() {
                follower.subrange(0, trunc_len)
            } else {
                follower
            };
            assert(resolve_start_state(follower, leader_entries, start_index, offset) == prefix);
        }
        None => {
            // resolve_conflict_spec: prefix = follower, append from overlap
            // resolve_start_state with None conflict: follower
            assert(resolve_start_state(follower, leader_entries, start_index, 0) == follower);
        }
    }
}

// r[verify log-conflict.resolve]
/// Append increases length by (leader_entries.len() - from).
proof fn append_entries_length(
    prefix: Seq<LogEntry>,
    leader_entries: Seq<LogEntry>,
    from: int,
)
    requires 0 <= from <= leader_entries.len()
    ensures
        append_entries_spec(prefix, leader_entries, from).len()
            == prefix.len() + leader_entries.len() - from
    decreases leader_entries.len() - from,
{
    if from >= leader_entries.len() {
        // Base
    } else {
        append_entries_length(
            prefix.push(leader_entries[from]),
            leader_entries,
            from + 1,
        );
    }
}

} // verus!

#[cfg(test)]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::default_numeric_fallback
)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn entry(term: u64, value: u64) -> LogEntry {
        LogEntry { term, value }
    }

    // r[verify log-conflict.find-conflict]
    #[test]
    fn test_find_conflict_at_start() {
        let follower = vec![entry(1, 0), entry(2, 0), entry(3, 0)];
        let leader = vec![entry(1, 0), entry(4, 0)];
        let result = find_conflict(&follower, &leader, 0);
        assert_eq!(result, Some(1)); // terms 2 vs 4 at offset 1
    }

    // r[verify log-conflict.find-conflict]
    #[test]
    fn test_find_conflict_no_conflict() {
        let follower = vec![entry(1, 0), entry(2, 0)];
        let leader = vec![entry(1, 0), entry(2, 0)];
        let result = find_conflict(&follower, &leader, 0);
        assert_eq!(result, None);
    }

    // r[verify log-conflict.find-conflict]
    #[test]
    fn test_find_conflict_leader_extends() {
        let follower = vec![entry(1, 0)];
        let leader = vec![entry(1, 0), entry(2, 0), entry(3, 0)];
        let result = find_conflict(&follower, &leader, 0);
        assert_eq!(result, None); // no conflict in overlapping portion
    }

    // r[verify log-conflict.find-conflict]
    #[test]
    fn test_find_conflict_empty_leader() {
        let follower = vec![entry(1, 0), entry(2, 0)];
        let leader: Vec<LogEntry> = vec![];
        let result = find_conflict(&follower, &leader, 0);
        assert_eq!(result, None);
    }

    // r[verify log-conflict.find-conflict]
    #[test]
    fn test_find_conflict_with_start_index() {
        let follower = vec![entry(1, 0), entry(2, 0), entry(3, 0), entry(4, 0)];
        let leader = vec![entry(3, 0), entry(5, 0)]; // starts at index 2
        let result = find_conflict(&follower, &leader, 2);
        assert_eq!(result, Some(1)); // follower[3].term=4 vs leader[1].term=5
    }

    // r[verify log-conflict.resolve]
    #[test]
    fn test_resolve_truncate_and_append() {
        let mut follower = vec![entry(1, 10), entry(2, 20), entry(3, 30)];
        let leader = vec![entry(1, 0), entry(4, 0), entry(4, 0)];
        resolve_conflict(&mut follower, &leader, 0);
        assert_eq!(follower.len(), 3);
        assert_eq!(follower[0].term, 1);
        assert_eq!(follower[1].term, 4);
        assert_eq!(follower[2].term, 4);
    }

    // r[verify log-conflict.resolve]
    #[test]
    fn test_resolve_append_only() {
        let mut follower = vec![entry(1, 10), entry(2, 20)];
        let leader = vec![entry(1, 0), entry(2, 0), entry(3, 0)];
        resolve_conflict(&mut follower, &leader, 0);
        assert_eq!(follower.len(), 3);
        assert_eq!(follower[0].term, 1);
        assert_eq!(follower[1].term, 2);
        assert_eq!(follower[2].term, 3);
    }

    // r[verify log-conflict.resolve]
    #[test]
    fn test_resolve_no_change() {
        let mut follower = vec![entry(1, 10), entry(2, 20)];
        let leader = vec![entry(1, 0), entry(2, 0)];
        resolve_conflict(&mut follower, &leader, 0);
        assert_eq!(follower.len(), 2);
        assert_eq!(follower[0].term, 1);
        assert_eq!(follower[1].term, 2);
    }

    // r[verify log-conflict.resolve]
    #[test]
    fn test_resolve_empty_leader() {
        let mut follower = vec![entry(1, 10), entry(2, 20)];
        let leader: Vec<LogEntry> = vec![];
        resolve_conflict(&mut follower, &leader, 0);
        assert_eq!(follower.len(), 2);
        assert_eq!(follower[0].term, 1);
        assert_eq!(follower[1].term, 2);
    }

    // r[verify log-conflict.resolve]
    #[test]
    fn test_resolve_with_start_index() {
        let mut follower = vec![entry(1, 10), entry(2, 20), entry(3, 30), entry(4, 40)];
        let leader = vec![entry(3, 0), entry(5, 0)]; // start at index 2
        resolve_conflict(&mut follower, &leader, 2);
        // Conflict at offset 1 (follower[3].term=4 vs leader[1].term=5)
        // Truncate to length 3, append leader[1..] = [(5,0)]
        assert_eq!(follower.len(), 4);
        assert_eq!(follower[0].term, 1);
        assert_eq!(follower[1].term, 2);
        assert_eq!(follower[2].term, 3);
        assert_eq!(follower[3].term, 5);
    }

    proptest! {
        // r[verify log-conflict.find-conflict]
        #[test]
        fn prop_no_conflict_same_terms(
            terms in proptest::collection::vec(1u64..10, 1..=20),
        ) {
            let follower: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 0)).collect();
            let leader: Vec<LogEntry> = terms.iter().map(|&t| entry(t, 1)).collect();
            prop_assert_eq!(find_conflict(&follower, &leader, 0), None);
        }

        // r[verify log-conflict.find-conflict]
        #[test]
        fn prop_conflict_in_range(
            f_terms in proptest::collection::vec(1u64..10, 1..=20),
            l_terms in proptest::collection::vec(1u64..10, 1..=20),
        ) {
            let follower: Vec<LogEntry> = f_terms.iter().map(|&t| entry(t, 0)).collect();
            let leader: Vec<LogEntry> = l_terms.iter().map(|&t| entry(t, 0)).collect();
            if let Some(offset) = find_conflict(&follower, &leader, 0) {
                prop_assert!(offset < leader.len());
            }
        }

        // r[verify log-conflict.resolve]
        #[test]
        fn prop_resolve_prefix_preserved(
            f_terms in proptest::collection::vec(1u64..5, 1..=10),
            l_terms in proptest::collection::vec(1u64..5, 1..=10),
        ) {
            let original: Vec<LogEntry> = f_terms.iter().map(|&t| entry(t, 0)).collect();
            let mut follower = original.clone();
            let leader: Vec<LogEntry> = l_terms.iter().map(|&t| entry(t, 0)).collect();

            // Find conflict manually
            let conflict_at = (0..leader.len().min(original.len()))
                .find(|&i| original[i].term != leader[i].term);

            resolve_conflict(&mut follower, &leader, 0);

            // Entries before conflict unchanged
            let preserved = conflict_at.unwrap_or(leader.len().min(original.len()));
            for i in 0..preserved {
                prop_assert_eq!(follower[i].term, original[i].term,
                    "prefix changed at {}", i);
            }
        }

        // r[verify log-conflict.resolve]
        #[test]
        fn prop_resolve_matches_leader(
            f_terms in proptest::collection::vec(1u64..5, 1..=10),
            l_terms in proptest::collection::vec(1u64..5, 1..=10),
        ) {
            let mut follower: Vec<LogEntry> = f_terms.iter().map(|&t| entry(t, 0)).collect();
            let leader: Vec<LogEntry> = l_terms.iter().map(|&t| entry(t, 0)).collect();
            resolve_conflict(&mut follower, &leader, 0);

            // Result matches leader at all covered indices
            for (i, le) in leader.iter().enumerate() {
                if i < follower.len() {
                    prop_assert_eq!(follower[i].term, le.term,
                        "mismatch at index {} (start_index=0)", i);
                }
            }
        }

        // r[verify log-conflict.resolve]
        // r[verify log-conflict.find-conflict]
        /// Model oracle: apply random leader entries at random start indices,
        /// verify the result by checking term agreement at each position.
        #[test]
        fn prop_resolve_model_oracle(
            f_terms in proptest::collection::vec(1u64..5, 0..=15),
            l_terms in proptest::collection::vec(1u64..5, 0..=10),
            start_frac in 0.0f64..=1.0,
        ) {
            let mut follower: Vec<LogEntry> = f_terms.iter().map(|&t| entry(t, 0)).collect();
            let leader: Vec<LogEntry> = l_terms.iter().map(|&t| entry(t, 0)).collect();
            let start_index = if follower.is_empty() {
                0
            } else {
                (start_frac * follower.len() as f64) as usize % follower.len()
            };

            // Build expected result manually
            let mut expected = follower.clone();
            let mut conflict_idx: Option<usize> = None;
            for (i, le) in leader.iter().enumerate() {
                let abs = start_index + i;
                if abs < expected.len() {
                    if expected[abs].term != le.term && conflict_idx.is_none() {
                        conflict_idx = Some(abs);
                    }
                }
            }
            if let Some(ci) = conflict_idx {
                expected.truncate(ci);
            }
            // Append leader entries from the conflict point or overlap end
            let append_from = if let Some(ci) = conflict_idx {
                ci - start_index
            } else if start_index + leader.len() <= expected.len() {
                leader.len()
            } else if start_index <= expected.len() {
                expected.len() - start_index
            } else {
                0
            };
            for i in append_from..leader.len() {
                expected.push(leader[i]);
            }

            resolve_conflict(&mut follower, &leader, start_index);

            prop_assert_eq!(follower.len(), expected.len(),
                "length mismatch: got {} expected {}", follower.len(), expected.len());
            for i in 0..follower.len() {
                prop_assert_eq!(follower[i].term, expected[i].term,
                    "term mismatch at index {}", i);
            }
        }
    }
}
