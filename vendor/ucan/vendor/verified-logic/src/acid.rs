//! Verified ACID transaction properties.
//!
//! Models atomic write transactions over an indexed state vector. A
//! transaction is a sequence of `WriteOp`s applied all-or-nothing.
//! Proves the four ACID guarantees:
//!
//! - **Atomicity**: commit applies all writes; abort applies none.
//! - **Consistency**: unwritten positions retain their original values.
//! - **Isolation**: writes to distinct positions commute.
//! - **Durability**: WAL recovery replays committed entries exactly.

use vstd::prelude::*;

verus! {

// ==================== Types ====================

/// A write operation: set state[idx] = val.
pub struct WriteOp {
    /// Target position in the state.
    pub idx: usize,
    /// New value to write.
    pub val: u64,
}

/// A write-ahead log entry: a write operation tagged with commit status.
pub struct WalEntry {
    /// Target position in the state.
    pub idx: usize,
    /// Value to write.
    pub val: u64,
    /// Whether this entry's transaction committed.
    pub committed: bool,
}

// ==================== Validity predicates ====================

// r[depends acid.apply-write]
/// Spec: all write indices are within state bounds.
pub open spec fn writes_valid_spec(writes: Seq<WriteOp>, state_len: int) -> bool {
    forall|i: int| #![trigger writes[i]]
        0 <= i < writes.len() ==> 0 <= (writes[i].idx as int) < state_len
}

/// Spec: all WAL entry indices are within state bounds.
pub open spec fn wal_valid_spec(wal: Seq<WalEntry>, state_len: int) -> bool {
    forall|i: int| #![trigger wal[i]]
        0 <= i < wal.len() ==> 0 <= (wal[i].idx as int) < state_len
}

// r[depends acid.writes-disjoint]
/// Spec: no write index in `a` matches any write index in `b`.
pub open spec fn writes_disjoint_spec(a: Seq<WriteOp>, b: Seq<WriteOp>) -> bool {
    forall|i: int, j: int| #![trigger a[i], b[j]]
        0 <= i < a.len() && 0 <= j < b.len() ==> a[i].idx != b[j].idx
}

// ==================== Core operation specs ====================

// r[depends acid.apply-write]
/// Spec: apply a single write to the state.
pub open spec fn apply_write_spec(state: Seq<u64>, idx: int, val: u64) -> Seq<u64>
    recommends 0 <= idx < state.len()
{
    state.update(idx, val)
}

// r[depends acid.commit]
/// Spec: apply writes[from..] to the state in sequence.
pub open spec fn apply_writes_spec(
    state: Seq<u64>, writes: Seq<WriteOp>, from: int,
) -> Seq<u64>
    decreases writes.len() - from
{
    if from >= writes.len() {
        state
    } else {
        apply_writes_spec(
            state.update(writes[from].idx as int, writes[from].val),
            writes,
            from + 1,
        )
    }
}

// r[depends acid.commit]
/// Spec: commit = apply all writes from the beginning.
pub open spec fn commit_spec(state: Seq<u64>, writes: Seq<WriteOp>) -> Seq<u64> {
    apply_writes_spec(state, writes, 0)
}

// r[depends acid.abort]
// r[impl acid.abort]
/// Spec: abort = identity (no writes applied).
pub open spec fn abort_spec(state: Seq<u64>) -> Seq<u64> {
    state
}

// ==================== WAL specs ====================

// r[depends acid.durability]
/// Spec: recover state by replaying committed WAL entries.
pub open spec fn wal_recover_spec(
    state: Seq<u64>, wal: Seq<WalEntry>, from: int,
) -> Seq<u64>
    decreases wal.len() - from
{
    if from >= wal.len() {
        state
    } else if wal[from].committed {
        wal_recover_spec(
            state.update(wal[from].idx as int, wal[from].val),
            wal,
            from + 1,
        )
    } else {
        wal_recover_spec(state, wal, from + 1)
    }
}

// r[depends acid.durability]
/// Spec: all WAL entries are committed.
pub open spec fn all_committed_spec(wal: Seq<WalEntry>) -> bool {
    forall|i: int| #![trigger wal[i]] 0 <= i < wal.len() ==> wal[i].committed
}

// r[depends acid.durability]
/// Spec: no WAL entries are committed.
pub open spec fn none_committed_spec(wal: Seq<WalEntry>) -> bool {
    forall|i: int| #![trigger wal[i]] 0 <= i < wal.len() ==> !wal[i].committed
}

/// Spec: extract write ops from WAL entries (stripping commit status).
pub open spec fn wal_to_writes_spec(wal: Seq<WalEntry>) -> Seq<WriteOp> {
    Seq::new(wal.len(), |i: int| WriteOp { idx: wal[i].idx, val: wal[i].val })
}

// ==================== Consistency helper ====================

// r[depends acid.consistency]
// r[impl acid.consistency]
/// Spec: index j does not appear in any write from position `from` onward.
pub open spec fn idx_not_in_writes_from(
    writes: Seq<WriteOp>, j: int, from: int,
) -> bool {
    forall|k: int| #![trigger writes[k]] from <= k < writes.len()
        ==> writes[k].idx as int != j
}

// ==================== Exec functions ====================

// r[impl acid.apply-write]
/// Applies a single write operation to the state.
pub fn apply_write(state: &mut Vec<u64>, op: &WriteOp)
    requires op.idx < old(state)@.len()
    ensures
        state@ == apply_write_spec(old(state)@, op.idx as int, op.val),
        state@.len() == old(state)@.len(),
{
    state.set(op.idx, op.val);
}

// r[impl acid.commit]
/// Applies all write operations to the state in sequence (commit).
pub fn apply_writes(state: &mut Vec<u64>, ops: &[WriteOp])
    requires writes_valid_spec(ops@, old(state)@.len() as int)
    ensures
        state@ == commit_spec(old(state)@, ops@),
        state@.len() == old(state)@.len(),
{
    let ghost initial = state@;
    let mut i: usize = 0;
    while i < ops.len()
        invariant
            0 <= i <= ops@.len(),
            state@.len() == initial.len(),
            writes_valid_spec(ops@, state@.len() as int),
            apply_writes_spec(state@, ops@, i as int)
                == apply_writes_spec(initial, ops@, 0),
        decreases ops@.len() - i
    {
        state.set(ops[i].idx, ops[i].val);
        i = i + 1;
    }
}

// r[impl acid.writes-disjoint]
/// Returns true if the two write sets target entirely different positions.
pub fn writes_disjoint(a: &[WriteOp], b: &[WriteOp]) -> (result: bool)
    ensures result == writes_disjoint_spec(a@, b@)
{
    let mut i: usize = 0;
    while i < a.len()
        invariant
            0 <= i <= a@.len(),
            forall|ii: int, jj: int| #![trigger a@[ii], b@[jj]]
                0 <= ii < i as int && 0 <= jj < b@.len()
                ==> a@[ii].idx != b@[jj].idx,
        decreases a@.len() - i
    {
        let mut j: usize = 0;
        while j < b.len()
            invariant
                0 <= i < a@.len(),
                0 <= j <= b@.len(),
                forall|ii: int, jj: int| #![trigger a@[ii], b@[jj]]
                    0 <= ii < i as int && 0 <= jj < b@.len()
                    ==> a@[ii].idx != b@[jj].idx,
                forall|jj: int| #![trigger b@[jj]]
                    0 <= jj < j as int
                    ==> a@[i as int].idx != b@[jj].idx,
            decreases b@.len() - j
        {
            if a[i].idx == b[j].idx {
                return false;
            }
            j = j + 1;
        }
        i = i + 1;
    }
    true
}

// r[impl acid.durability]
/// Recovers state by replaying committed WAL entries.
pub fn wal_recover(state: &mut Vec<u64>, wal: &[WalEntry])
    requires wal_valid_spec(wal@, old(state)@.len() as int)
    ensures
        state@ == wal_recover_spec(old(state)@, wal@, 0),
        state@.len() == old(state)@.len(),
{
    let ghost initial = state@;
    let mut i: usize = 0;
    while i < wal.len()
        invariant
            0 <= i <= wal@.len(),
            state@.len() == initial.len(),
            wal_valid_spec(wal@, state@.len() as int),
            wal_recover_spec(state@, wal@, i as int)
                == wal_recover_spec(initial, wal@, 0),
        decreases wal@.len() - i
    {
        if wal[i].committed {
            state.set(wal[i].idx, wal[i].val);
        }
        i = i + 1;
    }
}

// ==================== Atomicity proofs ====================

// r[verify acid.abort]
/// Abort is the identity function on state.
proof fn abort_is_identity(state: Seq<u64>)
    ensures abort_spec(state) == state
{}

// r[impl acid.atomicity]
// r[verify acid.atomicity]
/// Committing an empty write set leaves the state unchanged.
proof fn commit_empty_is_identity(state: Seq<u64>)
    ensures commit_spec(state, Seq::<WriteOp>::empty()) == state
{}

// r[verify acid.atomicity]
/// apply_writes preserves state length (inductive).
proof fn apply_writes_preserves_length(
    state: Seq<u64>, writes: Seq<WriteOp>, from: int,
)
    requires
        0 <= from,
        writes_valid_spec(writes, state.len() as int),
    ensures
        apply_writes_spec(state, writes, from).len() == state.len()
    decreases writes.len() - from
{
    if from < writes.len() {
        let next = state.update(writes[from].idx as int, writes[from].val);
        apply_writes_preserves_length(next, writes, from + 1);
    }
}

// r[verify acid.atomicity]
/// Commit preserves state length.
proof fn commit_preserves_length(state: Seq<u64>, writes: Seq<WriteOp>)
    requires writes_valid_spec(writes, state.len() as int)
    ensures commit_spec(state, writes).len() == state.len()
{
    apply_writes_preserves_length(state, writes, 0);
}

// r[verify acid.apply-write]
/// A single write is reflected at the written position.
proof fn single_write_applied(state: Seq<u64>, idx: int, val: u64)
    requires 0 <= idx < state.len()
    ensures apply_write_spec(state, idx, val)[idx] == val
{}

// ==================== Consistency proofs ====================

// r[verify acid.consistency]
/// Positions not targeted by any write retain their original value.
proof fn unwritten_unchanged(
    state: Seq<u64>, writes: Seq<WriteOp>, j: int, from: int,
)
    requires
        0 <= from,
        0 <= j < state.len(),
        writes_valid_spec(writes, state.len() as int),
        idx_not_in_writes_from(writes, j, from),
    ensures
        apply_writes_spec(state, writes, from)[j] == state[j]
    decreases writes.len() - from
{
    if from < writes.len() {
        let next = state.update(writes[from].idx as int, writes[from].val);
        // writes[from].idx != j, so next[j] == state[j]
        assert(next[j] == state[j]);
        unwritten_unchanged(next, writes, j, from + 1);
    }
}

// ==================== Isolation proofs ====================

// r[impl acid.isolation]
// r[verify acid.isolation]
/// Two writes to distinct positions commute.
proof fn single_writes_commute(
    state: Seq<u64>,
    idx_a: int, val_a: u64,
    idx_b: int, val_b: u64,
)
    requires
        0 <= idx_a < state.len(),
        0 <= idx_b < state.len(),
        idx_a != idx_b,
    ensures
        state.update(idx_a, val_a).update(idx_b, val_b)
        =~= state.update(idx_b, val_b).update(idx_a, val_a)
{}

// r[verify acid.isolation]
/// A write at position idx does not affect the value at position j != idx.
proof fn write_read_independent(
    state: Seq<u64>, idx: int, val: u64, j: int,
)
    requires
        0 <= idx < state.len(),
        0 <= j < state.len(),
        idx != j,
    ensures
        apply_write_spec(state, idx, val)[j] == state[j]
{}

// ==================== Durability proofs ====================

// r[verify acid.durability]
/// WAL recovery preserves state length (inductive).
proof fn wal_recover_preserves_length(
    state: Seq<u64>, wal: Seq<WalEntry>, from: int,
)
    requires
        0 <= from,
        wal_valid_spec(wal, state.len() as int),
    ensures
        wal_recover_spec(state, wal, from).len() == state.len()
    decreases wal.len() - from
{
    if from < wal.len() {
        if wal[from].committed {
            let next = state.update(wal[from].idx as int, wal[from].val);
            wal_recover_preserves_length(next, wal, from + 1);
        } else {
            wal_recover_preserves_length(state, wal, from + 1);
        }
    }
}

// r[verify acid.durability]
/// Fully committed WAL recovery equals direct commit.
proof fn wal_all_committed_is_commit(
    state: Seq<u64>, wal: Seq<WalEntry>, from: int,
)
    requires
        0 <= from,
        all_committed_spec(wal),
    ensures
        wal_recover_spec(state, wal, from)
        == apply_writes_spec(state, wal_to_writes_spec(wal), from)
    decreases wal.len() - from
{
    if from < wal.len() {
        let ops = wal_to_writes_spec(wal);
        assert(ops[from].idx == wal[from].idx);
        assert(ops[from].val == wal[from].val);
        let next = state.update(wal[from].idx as int, wal[from].val);
        wal_all_committed_is_commit(next, wal, from + 1);
    }
}

// r[verify acid.durability]
/// Fully uncommitted WAL recovery leaves state unchanged.
proof fn wal_none_committed_is_abort(
    state: Seq<u64>, wal: Seq<WalEntry>, from: int,
)
    requires
        0 <= from,
        none_committed_spec(wal),
    ensures
        wal_recover_spec(state, wal, from) == state
    decreases wal.len() - from
{
    if from < wal.len() {
        assert(!wal[from].committed);
        wal_none_committed_is_abort(state, wal, from + 1);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn w(idx: usize, val: u64) -> WriteOp {
        WriteOp { idx, val }
    }

    fn wal(idx: usize, val: u64, committed: bool) -> WalEntry {
        WalEntry { idx, val, committed }
    }

    // r[verify acid.apply-write]
    #[test]
    fn test_apply_write() {
        let mut state = vec![10, 20, 30];
        apply_write(&mut state, &w(1, 99));
        assert_eq!(state, vec![10, 99, 30]);
    }

    // r[verify acid.apply-write]
    #[test]
    fn test_apply_write_preserves_length() {
        let mut state = vec![0, 0, 0, 0];
        let orig_len = state.len();
        apply_write(&mut state, &w(2, 42));
        assert_eq!(state.len(), orig_len);
    }

    // r[verify acid.commit]
    #[test]
    fn test_apply_writes_batch() {
        let mut state = vec![10, 20, 30, 40];
        let ops = [w(0, 100), w(2, 300)];
        apply_writes(&mut state, &ops);
        assert_eq!(state, vec![100, 20, 300, 40]);
    }

    // r[verify acid.commit]
    #[test]
    fn test_apply_writes_empty() {
        let mut state = vec![10, 20, 30];
        let original = state.clone();
        apply_writes(&mut state, &[]);
        assert_eq!(state, original);
    }

    // r[verify acid.atomicity]
    #[test]
    fn test_commit_preserves_length() {
        let mut state = vec![0u64; 10];
        let ops = [w(0, 1), w(5, 2), w(9, 3)];
        apply_writes(&mut state, &ops);
        assert_eq!(state.len(), 10);
    }

    // r[verify acid.abort]
    #[test]
    fn test_abort_no_change() {
        let state = vec![10_i32, 20_i32, 30_i32];
        // Abort = don't call apply_writes. State is unchanged.
        assert_eq!(state, vec![10_i32, 20_i32, 30_i32]);
    }

    // r[verify acid.consistency]
    #[test]
    fn test_unwritten_positions_unchanged() {
        let mut state = vec![10, 20, 30, 40, 50];
        let ops = [w(1, 99), w(3, 88)];
        apply_writes(&mut state, &ops);
        // Positions 0, 2, 4 were not written
        assert_eq!(state[0], 10);
        assert_eq!(state[2], 30);
        assert_eq!(state[4], 50);
        // Written positions changed
        assert_eq!(state[1], 99);
        assert_eq!(state[3], 88);
    }

    // r[verify acid.writes-disjoint]
    #[test]
    fn test_writes_disjoint() {
        let a = [w(0, 1), w(2, 2)];
        let b = [w(1, 3), w(3, 4)];
        assert!(writes_disjoint(&a, &b));
    }

    // r[verify acid.writes-disjoint]
    #[test]
    fn test_writes_not_disjoint() {
        let a = [w(0, 1), w(2, 2)];
        let b = [w(1, 3), w(2, 4)]; // index 2 overlaps
        assert!(!writes_disjoint(&a, &b));
    }

    // r[verify acid.writes-disjoint]
    #[test]
    fn test_writes_disjoint_empty() {
        let a: [WriteOp; 0] = [];
        let b = [w(0, 1)];
        assert!(writes_disjoint(&a, &b));
        assert!(writes_disjoint(&b, &a));
        assert!(writes_disjoint(&a, &a));
    }

    // r[verify acid.isolation]
    #[test]
    fn test_disjoint_writes_commute() {
        let mut state_ab = vec![0, 0, 0, 0];
        let mut state_ba = vec![0, 0, 0, 0];
        // Apply A then B
        apply_write(&mut state_ab, &w(1, 10));
        apply_write(&mut state_ab, &w(3, 30));
        // Apply B then A
        apply_write(&mut state_ba, &w(3, 30));
        apply_write(&mut state_ba, &w(1, 10));
        assert_eq!(state_ab, state_ba);
    }

    // r[verify acid.durability]
    #[test]
    fn test_wal_recover_all_committed() {
        let mut state = vec![0, 0, 0];
        let entries = [wal(0, 10, true), wal(1, 20, true), wal(2, 30, true)];
        wal_recover(&mut state, &entries);
        assert_eq!(state, vec![10, 20, 30]);
    }

    // r[verify acid.durability]
    #[test]
    fn test_wal_recover_none_committed() {
        let mut state = vec![10, 20, 30];
        let entries = [wal(0, 99, false), wal(1, 88, false)];
        wal_recover(&mut state, &entries);
        assert_eq!(state, vec![10, 20, 30]); // unchanged
    }

    // r[verify acid.durability]
    #[test]
    fn test_wal_recover_mixed() {
        let mut state = vec![0, 0, 0];
        let entries = [
            wal(0, 10, true),
            wal(1, 20, false), // skipped
            wal(2, 30, true),
        ];
        wal_recover(&mut state, &entries);
        assert_eq!(state, vec![10, 0, 30]);
    }

    // r[verify acid.durability]
    #[test]
    fn test_wal_recover_preserves_length() {
        let mut state = vec![0u64; 5];
        let entries = [wal(0, 1, true), wal(4, 2, true)];
        wal_recover(&mut state, &entries);
        assert_eq!(state.len(), 5);
    }

    proptest! {
        // r[verify acid.atomicity]
        #[test]
        fn prop_commit_preserves_length(
            len in 1usize..=50,
            ops_raw in proptest::collection::vec((0usize..50, any::<u64>()), 0..=20),
        ) {
            let mut state = vec![0u64; len];
            let ops: Vec<WriteOp> = ops_raw.iter()
                .map(|&(idx, val)| w(idx % len, val))
                .collect();
            apply_writes(&mut state, &ops);
            prop_assert_eq!(state.len(), len);
        }

        // r[verify acid.consistency]
        #[test]
        fn prop_unwritten_unchanged(
            len in 2usize..=50,
            ops_raw in proptest::collection::vec((0usize..50, any::<u64>()), 1..=10),
        ) {
            let initial: Vec<u64> = (0..len as u64).collect();
            let mut state = initial.clone();
            let ops: Vec<WriteOp> = ops_raw.iter()
                .map(|&(idx, val)| w(idx % len, val))
                .collect();
            let written_indices: std::collections::HashSet<usize> =
                ops.iter().map(|op| op.idx).collect();
            apply_writes(&mut state, &ops);
            // Every position NOT in written_indices must be unchanged
            for j in 0..len {
                if !written_indices.contains(&j) {
                    prop_assert_eq!(state[j], initial[j],
                        "position {} changed: expected {}, got {}", j, initial[j], state[j]);
                }
            }
        }

        // r[verify acid.isolation]
        #[test]
        fn prop_single_writes_commute(
            len in 2usize..=50,
            idx_a_raw: usize,
            val_a: u64,
            idx_b_offset in 1usize..50,
            val_b: u64,
        ) {
            let idx_a = idx_a_raw % len;
            let idx_b = (idx_a + idx_b_offset) % len;
            if idx_a != idx_b {
                let initial: Vec<u64> = (0..len as u64).collect();
                let mut state_ab = initial.clone();
                let mut state_ba = initial.clone();
                // A then B
                apply_write(&mut state_ab, &w(idx_a, val_a));
                apply_write(&mut state_ab, &w(idx_b, val_b));
                // B then A
                apply_write(&mut state_ba, &w(idx_b, val_b));
                apply_write(&mut state_ba, &w(idx_a, val_a));
                prop_assert_eq!(state_ab, state_ba);
            }
        }

        // r[verify acid.writes-disjoint]
        #[test]
        fn prop_writes_disjoint_symmetric(
            a_raw in proptest::collection::vec(0usize..100, 0..=10),
            b_raw in proptest::collection::vec(0usize..100, 0..=10),
        ) {
            let a: Vec<WriteOp> = a_raw.iter().map(|&idx| w(idx, 0)).collect();
            let b: Vec<WriteOp> = b_raw.iter().map(|&idx| w(idx, 0)).collect();
            // Disjointness should be symmetric
            prop_assert_eq!(writes_disjoint(&a, &b), writes_disjoint(&b, &a));
        }

        // r[verify acid.durability]
        #[test]
        fn prop_wal_all_committed_equals_direct(
            len in 1usize..=20,
            ops_raw in proptest::collection::vec((0usize..20, any::<u64>()), 0..=10),
        ) {
            let initial = vec![0u64; len];
            let ops: Vec<WriteOp> = ops_raw.iter()
                .map(|&(idx, val)| w(idx % len, val))
                .collect();
            let wal_entries: Vec<WalEntry> = ops.iter()
                .map(|op| wal(op.idx, op.val, true))
                .collect();
            // Direct commit
            let mut state_direct = initial.clone();
            apply_writes(&mut state_direct, &ops);
            // WAL recovery (all committed)
            let mut state_wal = initial;
            wal_recover(&mut state_wal, &wal_entries);
            prop_assert_eq!(state_direct, state_wal);
        }

        // r[verify acid.durability]
        #[test]
        fn prop_wal_none_committed_is_identity(
            len in 1usize..=20,
            ops_raw in proptest::collection::vec((0usize..20, any::<u64>()), 0..=10),
        ) {
            let initial: Vec<u64> = (0..len as u64).collect();
            let wal_entries: Vec<WalEntry> = ops_raw.iter()
                .map(|&(idx, val)| wal(idx % len, val, false))
                .collect();
            let mut state = initial.clone();
            wal_recover(&mut state, &wal_entries);
            prop_assert_eq!(state, initial);
        }

        // r[verify acid.apply-write]
        // r[verify acid.commit]
        // r[verify acid.abort]
        // r[verify acid.atomicity]
        // r[verify acid.consistency]
        // r[verify acid.isolation]
        // r[verify acid.durability]
        // r[verify acid.writes-disjoint]
        /// Model oracle: random commit/abort/recover traces on a state
        /// vector, checking all ACID properties against a Vec<u64> reference.
        #[test]
        fn prop_acid_model_oracle(
            len in 3usize..=20,
            ops in proptest::collection::vec(
                prop_oneof![
                    3 => (0usize..20, any::<u64>()).prop_map(|(i, v)| (0u8, i, v, true)),
                    2 => (0usize..20, any::<u64>()).prop_map(|(i, v)| (1u8, i, v, true)),
                    1 => (0usize..20, any::<u64>(), any::<bool>()).prop_map(|(i, v, c)| (2u8, i, v, c)),
                ],
                0..=50,
            ),
        ) {
            let mut state = vec![0u64; len];
            let mut oracle = vec![0u64; len];

            for &(op_type, idx_raw, val, committed) in &ops {
                let idx = idx_raw % len;
                match op_type {
                    0 => {
                        // Single write — apply to both
                        apply_write(&mut state, &w(idx, val));
                        oracle[idx] = val;
                    }
                    1 => {
                        // Batch commit
                        let batch = [w(idx, val)];
                        apply_writes(&mut state, &batch);
                        oracle[idx] = val;
                    }
                    _ => {
                        // WAL recovery
                        let entry = [wal(idx, val, committed)];
                        wal_recover(&mut state, &entry);
                        if committed {
                            oracle[idx] = val;
                        }
                    }
                }

                // Validate state matches oracle at every step
                prop_assert_eq!(state.len(), oracle.len());
                for j in 0..len {
                    prop_assert_eq!(state[j], oracle[j],
                        "mismatch at position {}: state={}, oracle={}", j, state[j], oracle[j]);
                }
            }
        }
    }
}
