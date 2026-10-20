//! Verified WAL crash recovery: replay range, idempotent replay, checkpoint logic.
//!
//! After a crash, the WAL must be replayed from the last checkpoint to the end.
//! Proofs guarantee that the replay range is valid, replaying twice is harmless
//! (idempotent), and checkpoints only advance forward. Composes with
//! [`wal_segment`] for segment sequencing and [`monotonic`] for advancement.

use vstd::prelude::*;

verus! {

// ─── Replay Range ────────────────────────────────────────────────────

/// Spec: compute the replay range [checkpoint_lsn, wal_end).
/// Returns (start, end) where start <= end.
// r[depends wal-recovery.replay-range]
// r[impl wal-recovery.replay-range]
pub open spec fn replay_range_spec(checkpoint_lsn: u64, wal_end: u64) -> (u64, u64) {
    if checkpoint_lsn <= wal_end {
        (checkpoint_lsn, wal_end)
    } else {
        (wal_end, wal_end)
    }
}

// r[impl wal-recovery.replay-range]
pub fn replay_range(checkpoint_lsn: u64, wal_end: u64) -> (result: (u64, u64))
    ensures result == replay_range_spec(checkpoint_lsn, wal_end),
{
    if checkpoint_lsn <= wal_end {
        (checkpoint_lsn, wal_end)
    } else {
        (wal_end, wal_end)
    }
}

/// Spec: number of entries to replay.
// r[depends wal-recovery.replay-count]
// r[impl wal-recovery.replay-count]
pub open spec fn replay_count_spec(checkpoint_lsn: u64, wal_end: u64) -> u64 {
    let (start, end) = replay_range_spec(checkpoint_lsn, wal_end);
    (end - start) as u64
}

// r[impl wal-recovery.replay-count]
pub fn replay_count(checkpoint_lsn: u64, wal_end: u64) -> (result: u64)
    ensures result == replay_count_spec(checkpoint_lsn, wal_end),
{
    let (start, end) = replay_range(checkpoint_lsn, wal_end);
    end - start
}

// r[verify wal-recovery.replay-range]
/// Replay range start <= end.
proof fn replay_range_ordered(checkpoint_lsn: u64, wal_end: u64)
    ensures ({
        let (s, e) = replay_range_spec(checkpoint_lsn, wal_end);
        s <= e
    }),
{}

// r[verify wal-recovery.replay-range]
/// Empty WAL means zero replay.
proof fn empty_wal_no_replay(checkpoint_lsn: u64)
    ensures replay_count_spec(checkpoint_lsn, checkpoint_lsn) == 0,
{}

// ─── Checkpoint Advancement ──────────────────────────────────────────

/// Spec: is the new checkpoint a valid advancement?
// r[depends wal-recovery.checkpoint-advance]
// r[impl wal-recovery.checkpoint-advance]
pub open spec fn is_valid_checkpoint_advance_spec(old_cp: u64, new_cp: u64, wal_end: u64) -> bool {
    new_cp >= old_cp && new_cp <= wal_end
}

// r[impl wal-recovery.checkpoint-advance]
pub fn is_valid_checkpoint_advance(old_cp: u64, new_cp: u64, wal_end: u64) -> (result: bool)
    ensures result == is_valid_checkpoint_advance_spec(old_cp, new_cp, wal_end),
{
    new_cp >= old_cp && new_cp <= wal_end
}

// r[verify wal-recovery.checkpoint-advance]
/// Advancing the checkpoint reduces replay count.
proof fn checkpoint_advance_reduces_replay(old_cp: u64, new_cp: u64, wal_end: u64)
    requires is_valid_checkpoint_advance_spec(old_cp, new_cp, wal_end),
    ensures replay_count_spec(new_cp, wal_end) <= replay_count_spec(old_cp, wal_end),
{}

// r[verify wal-recovery.checkpoint-advance]
/// Checkpoint at WAL end means zero replay.
proof fn checkpoint_at_end_no_replay(wal_end: u64)
    ensures replay_count_spec(wal_end, wal_end) == 0,
{}

// r[verify wal-recovery.checkpoint-advance]
/// Checkpoint advancement is transitive.
proof fn checkpoint_advance_transitive(cp1: u64, cp2: u64, cp3: u64, wal_end: u64)
    requires
        is_valid_checkpoint_advance_spec(cp1, cp2, wal_end),
        is_valid_checkpoint_advance_spec(cp2, cp3, wal_end),
    ensures
        is_valid_checkpoint_advance_spec(cp1, cp3, wal_end),
{}

// ─── Idempotent Replay ──────────────────────────────────────────────

/// Spec: apply a single WAL entry. Each entry sets state[entry.key] = entry.value.
/// Modeled as Seq<u64> state, entry is (key, value).
// r[depends wal-recovery.apply-entry]
// r[impl wal-recovery.apply-entry]
pub open spec fn apply_entry_spec(state: Seq<u64>, key: int, value: u64) -> Seq<u64>
    recommends 0 <= key < state.len(),
{
    state.update(key, value)
}

/// Spec: apply entries[from..] to state.
// r[depends wal-recovery.apply-entries]
// r[impl wal-recovery.apply-entries]
pub open spec fn apply_entries_from_spec(
    state: Seq<u64>,
    entries: Seq<(u64, u64)>,
    from: int,
) -> Seq<u64>
    decreases (entries.len() - from),
{
    if from < 0 || from >= entries.len() {
        state
    } else {
        let (k, v) = entries[from];
        apply_entries_from_spec(apply_entry_spec(state, k as int, v), entries, from + 1)
    }
}

/// Spec: apply all entries to state.
pub open spec fn apply_all_spec(state: Seq<u64>, entries: Seq<(u64, u64)>) -> Seq<u64> {
    apply_entries_from_spec(state, entries, 0)
}

// r[impl wal-recovery.apply-entries]
/// Apply WAL entries to state.
pub fn apply_entries(state: &mut Vec<u64>, entries: &[(u64, u64)])
    requires
        forall|i: int| 0 <= i < entries@.len() ==>
            (entries@[i].0 as int) < old(state)@.len(),
        old(state)@.len() <= usize::MAX as int,
    ensures
        state@ == apply_all_spec(old(state)@, entries@),
{
    let ghost original = state@;
    let mut i: usize = 0;
    while i < entries.len()
        invariant
            0 <= i <= entries@.len(),
            state@.len() == original.len(),
            forall|j: int| 0 <= j < entries@.len() ==>
                (entries@[j].0 as int) < original.len(),
            state@ == apply_entries_from_spec(original, entries@, 0)
                || apply_entries_from_spec(state@, entries@, i as int)
                    == apply_entries_from_spec(original, entries@, 0),
            apply_entries_from_spec(state@, entries@, i as int)
                == apply_entries_from_spec(original, entries@, 0),
            original.len() <= usize::MAX as int,
        decreases entries@.len() - i,
    {
        let (k, v) = entries[i];
        proof {
            assert((entries@[i as int].0 as int) < original.len());
            assert((k as int) < state@.len());
        }
        let index = match usize::try_from(k) {
            Ok(value) => value,
            Err(_) => 0,
        };
        state.set(index, v);
        i = i + 1;
    }
}

// r[verify wal-recovery.idempotent]
/// Applying the same single entry twice is the same as once.
proof fn apply_entry_idempotent(state: Seq<u64>, key: int, value: u64)
    requires 0 <= key < state.len(),
    ensures
        apply_entry_spec(apply_entry_spec(state, key, value), key, value)
            =~= apply_entry_spec(state, key, value),
{}

// r[verify wal-recovery.apply-entries]
/// Applying empty entries is identity.
proof fn apply_empty_identity(state: Seq<u64>)
    ensures apply_all_spec(state, Seq::<(u64, u64)>::empty()) =~= state,
{}

// ─── Recovery Correctness ────────────────────────────────────────────

/// Spec: entries in the replay range.
// r[depends wal-recovery.replay-entries]
// r[impl wal-recovery.replay-entries]
pub open spec fn replay_entries_spec(
    wal: Seq<(u64, u64)>,
    checkpoint_lsn: u64,
    wal_end: u64,
) -> Seq<(u64, u64)>
    recommends checkpoint_lsn <= wal_end, (wal_end as int) <= wal.len(),
{
    let (start, end) = replay_range_spec(checkpoint_lsn, wal_end);
    wal.subrange(start as int, end as int)
}

// r[verify wal-recovery.replay-entries]
/// Replay entries length matches replay count.
proof fn replay_entries_length(wal: Seq<(u64, u64)>, checkpoint_lsn: u64, wal_end: u64)
    requires checkpoint_lsn <= wal_end, (wal_end as int) <= wal.len(),
    ensures
        replay_entries_spec(wal, checkpoint_lsn, wal_end).len()
            == replay_count_spec(checkpoint_lsn, wal_end) as int,
{}

// ─── WAL Append Safety ───────────────────────────────────────────────

/// Spec: is it safe to append to the WAL (won't wrap around)?
// r[depends wal-recovery.append-safe]
// r[impl wal-recovery.append-safe]
pub open spec fn can_append_spec(wal_end: u64, count: u64) -> bool {
    (wal_end as int) + (count as int) <= u64::MAX as int
}

// r[impl wal-recovery.append-safe]
pub fn can_append(wal_end: u64, count: u64) -> (result: bool)
    ensures result == can_append_spec(wal_end, count),
{
    wal_end <= u64::MAX - count
}

// r[verify wal-recovery.append-safe]
/// After a safe append, the new end is valid.
proof fn safe_append_valid(wal_end: u64, count: u64)
    requires can_append_spec(wal_end, count),
    ensures (wal_end + count) as u64 >= wal_end,
{}

// r[verify wal-recovery.append-safe]
/// Safe append preserves checkpoint validity.
proof fn safe_append_preserves_checkpoint(checkpoint_lsn: u64, wal_end: u64, count: u64)
    requires
        checkpoint_lsn <= wal_end,
        can_append_spec(wal_end, count),
    ensures
        is_valid_checkpoint_advance_spec(
            checkpoint_lsn,
            checkpoint_lsn,
            (wal_end + count) as u64,
        ),
{}

// ─── Needs Recovery ──────────────────────────────────────────────────

/// Spec: does the system need recovery (checkpoint behind WAL end)?
// r[depends wal-recovery.needs-recovery]
// r[impl wal-recovery.needs-recovery]
pub open spec fn needs_recovery_spec(checkpoint_lsn: u64, wal_end: u64) -> bool {
    checkpoint_lsn < wal_end
}

// r[impl wal-recovery.needs-recovery]
pub fn needs_recovery(checkpoint_lsn: u64, wal_end: u64) -> (result: bool)
    ensures result == needs_recovery_spec(checkpoint_lsn, wal_end),
{
    checkpoint_lsn < wal_end
}

// r[verify wal-recovery.needs-recovery]
/// Needs recovery iff replay count > 0.
proof fn needs_recovery_iff_replay(checkpoint_lsn: u64, wal_end: u64)
    requires checkpoint_lsn <= wal_end,
    ensures needs_recovery_spec(checkpoint_lsn, wal_end) <==> replay_count_spec(checkpoint_lsn, wal_end) > 0,
{}

// r[verify wal-recovery.needs-recovery]
/// After checkpointing to WAL end, no recovery needed.
proof fn checkpoint_to_end_clears_recovery(wal_end: u64)
    ensures !needs_recovery_spec(wal_end, wal_end),
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replay_range_normal() {
        assert_eq!(replay_range(10, 20), (10, 20));
    }

    #[test]
    fn test_replay_range_empty() {
        assert_eq!(replay_range(10, 10), (10, 10));
    }

    #[test]
    fn test_replay_range_checkpoint_ahead() {
        assert_eq!(replay_range(30, 20), (20, 20));
    }

    #[test]
    fn test_replay_count() {
        assert_eq!(replay_count(10, 20), 10);
        assert_eq!(replay_count(10, 10), 0);
        assert_eq!(replay_count(30, 20), 0);
    }

    #[test]
    fn test_checkpoint_advance_valid() {
        assert!(is_valid_checkpoint_advance(10, 15, 20));
        assert!(is_valid_checkpoint_advance(10, 10, 20));
        assert!(is_valid_checkpoint_advance(10, 20, 20));
    }

    #[test]
    fn test_checkpoint_advance_invalid() {
        assert!(!is_valid_checkpoint_advance(10, 5, 20));
        assert!(!is_valid_checkpoint_advance(10, 25, 20));
    }

    #[test]
    fn test_apply_entries_basic() {
        let mut state = vec![0u64, 0, 0];
        let entries = vec![(0u64, 42), (2, 99)];
        apply_entries(&mut state, &entries);
        assert_eq!(state, vec![42, 0, 99]);
    }

    #[test]
    fn test_apply_entries_overwrite() {
        let mut state = vec![0u64, 0];
        let entries = vec![(0u64, 1), (0, 2), (0, 3)];
        apply_entries(&mut state, &entries);
        assert_eq!(state, vec![3, 0]);
    }

    #[test]
    fn test_apply_entries_empty() {
        let mut state = vec![1u64, 2, 3];
        let entries: Vec<(u64, u64)> = vec![];
        apply_entries(&mut state, &entries);
        assert_eq!(state, vec![1, 2, 3]);
    }

    #[test]
    fn test_apply_entry_idempotent() {
        let mut state = vec![0u64, 0, 0];
        let entries = vec![(1u64, 42)];
        apply_entries(&mut state, &entries);
        let after_once = state.clone();
        apply_entries(&mut state, &entries);
        assert_eq!(state, after_once);
    }

    #[test]
    fn test_needs_recovery() {
        assert!(needs_recovery(10, 20));
        assert!(!needs_recovery(20, 20));
        assert!(!needs_recovery(30, 20));
    }

    #[test]
    fn test_can_append() {
        assert!(can_append(0, 100));
        assert!(can_append(u64::MAX - 1, 1));
        assert!(!can_append(u64::MAX, 1));
        assert!(can_append(u64::MAX, 0));
    }
}

#[cfg(test)]
mod proptests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        #[test]
        fn prop_replay_range_ordered(cp in any::<u64>(), end in any::<u64>()) {
            let (s, e) = replay_range(cp, end);
            prop_assert!(s <= e);
        }

        #[test]
        fn prop_checkpoint_advance_reduces_replay(
            cp1 in 0u64..10000,
            delta in 0u64..10000,
            end_delta in 0u64..10000,
        ) {
            let cp2 = cp1.saturating_add(delta);
            let wal_end = cp2.saturating_add(end_delta);
            if is_valid_checkpoint_advance(cp1, cp2, wal_end) {
                let r1 = replay_count(cp1, wal_end);
                let r2 = replay_count(cp2, wal_end);
                prop_assert!(r2 <= r1);
            }
        }

        #[test]
        fn prop_apply_entry_idempotent(
            state_len in 1usize..50,
            key_idx in any::<proptest::sample::Index>(),
            value in any::<u64>(),
        ) {
            let key = key_idx.index(state_len) as u64;
            let mut state = vec![0u64; state_len];
            let entries = vec![(key, value)];
            apply_entries(&mut state, &entries);
            let after_once = state.clone();
            apply_entries(&mut state, &entries);
            prop_assert_eq!(state, after_once);
        }

        #[test]
        fn prop_needs_recovery_iff_replay(cp in 0u64..100000, end in 0u64..100000) {
            if cp <= end {
                let nr = needs_recovery(cp, end);
                let rc = replay_count(cp, end);
                prop_assert_eq!(nr, rc > 0);
            }
        }
    }
}
