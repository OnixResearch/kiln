//! Verified multi-version concurrency control (MVCC) visibility.
//!
//! Models version visibility for snapshot reads. Each version has a
//! creation timestamp and an optional deletion timestamp. A snapshot
//! at time T sees a version iff it was created at or before T and
//! not yet deleted at T.
//!
//! Timestamps are u64. Active transactions are modeled as a Seq<u64>
//! of transaction IDs whose writes should be invisible (they haven't
//! committed yet from the reader's perspective).

#![allow(clippy::nonminimal_bool)]

use vstd::prelude::*;

verus! {

/// No deletion timestamp — version is still live.
pub const TS_INFINITY: u64 = u64::MAX;

// --- is_visible ---

// r[depends mvcc.is-visible]
/// Spec: a version created at `begin_ts` and deleted at `end_ts`
/// is visible to a snapshot at time `snapshot_ts` iff:
///   begin_ts <= snapshot_ts < end_ts
pub open spec fn is_visible_spec(begin_ts: u64, end_ts: u64, snapshot_ts: u64) -> bool {
    begin_ts <= snapshot_ts && (snapshot_ts as int) < (end_ts as int)
}

// r[impl mvcc.is-visible]
/// Returns true if the version is visible at the given snapshot time.
pub fn is_visible(begin_ts: u64, end_ts: u64, snapshot_ts: u64) -> (result: bool)
    ensures result == is_visible_spec(begin_ts, end_ts, snapshot_ts)
{
    begin_ts <= snapshot_ts && snapshot_ts < end_ts
}

// --- is_committed ---

// r[depends mvcc.is-committed]
/// Spec: a version is committed if its creator is not in the active set.
pub open spec fn is_committed_spec(creator_txn: u64, active: Seq<u64>) -> bool {
    !active.contains(creator_txn)
}

// --- is_active_from ---

/// Spec helper: check if txn_id appears in active[from..].
pub open spec fn is_active_from_spec(txn_id: u64, active: Seq<u64>, from: int) -> bool
    decreases active.len() - from
{
    if from >= active.len() {
        false
    } else {
        active[from] == txn_id || is_active_from_spec(txn_id, active, from + 1)
    }
}

/// Spec: is_active_from matches Seq::contains when from == 0.
proof fn is_active_from_matches_contains(txn_id: u64, active: Seq<u64>, from: int)
    requires 0 <= from
    ensures is_active_from_spec(txn_id, active, from) <==>
        exists|k: int| from <= k < active.len() && active[k] == txn_id
    decreases active.len() - from
{
    if from < active.len() {
        is_active_from_matches_contains(txn_id, active, from + 1);
    }
}

// r[impl mvcc.is-committed]
/// Returns true if the creator transaction has committed (not in active set).
pub fn is_committed(creator_txn: u64, active: &[u64]) -> (result: bool)
    ensures result == is_committed_spec(creator_txn, active@)
{
    let mut i: usize = 0;
    while i < active.len()
        invariant
            0 <= i <= active@.len(),
            forall|k: int| #![trigger active@[k]] 0 <= k < i as int ==> active@[k] != creator_txn,
        decreases active@.len() - i
    {
        if active[i] == creator_txn {
            proof {
                assert(active@.contains(creator_txn));
            }
            return false;
        }
        i = i + 1;
    }
    proof {
        if active@.contains(creator_txn) {
            let witness = choose|k: int| 0 <= k < active@.len() && active@[k] == creator_txn;
            assert(active@[witness] != creator_txn);
        }
    }
    true
}

// --- snapshot_visible ---

// r[depends mvcc.snapshot-visible]
/// Spec: version is visible to a snapshot, considering both timestamp
/// range and active transaction set.
pub open spec fn snapshot_visible_spec(
    begin_ts: u64, end_ts: u64, creator_txn: u64,
    snapshot_ts: u64, active: Seq<u64>,
) -> bool {
    is_visible_spec(begin_ts, end_ts, snapshot_ts)
    && is_committed_spec(creator_txn, active)
}

// r[impl mvcc.snapshot-visible]
pub fn snapshot_visible(
    begin_ts: u64, end_ts: u64, creator_txn: u64,
    snapshot_ts: u64, active: &[u64],
) -> (result: bool)
    ensures result == snapshot_visible_spec(begin_ts, end_ts, creator_txn, snapshot_ts, active@)
{
    is_visible(begin_ts, end_ts, snapshot_ts) && is_committed(creator_txn, active)
}

// --- version_superseded ---

// r[depends mvcc.version-superseded]
/// Spec: a version is superseded (deleted) if end_ts != TS_INFINITY.
pub open spec fn version_superseded_spec(end_ts: u64) -> bool {
    end_ts != TS_INFINITY
}

// r[impl mvcc.version-superseded]
pub fn version_superseded(end_ts: u64) -> (result: bool)
    ensures result == version_superseded_spec(end_ts)
{
    end_ts != TS_INFINITY
}

// --- can_garbage_collect ---

// r[depends mvcc.can-gc]
/// Spec: a version can be garbage collected if it's been superseded
/// and its end_ts is before the oldest active snapshot.
pub open spec fn can_gc_spec(end_ts: u64, oldest_active: u64) -> bool {
    version_superseded_spec(end_ts) && (end_ts as int) <= (oldest_active as int)
}

// r[impl mvcc.can-gc]
pub fn can_gc(end_ts: u64, oldest_active: u64) -> (result: bool)
    ensures result == can_gc_spec(end_ts, oldest_active)
{
    end_ts != TS_INFINITY && end_ts <= oldest_active
}

// --- version_chain_latest ---

// r[depends mvcc.chain-latest]
/// Spec: find the latest visible version in a chain.
/// Chain is ordered newest-first: Seq<(begin_ts, end_ts, creator_txn)>.
/// Returns the index of the first visible version, or -1 if none.
pub open spec fn chain_latest_spec(
    chain: Seq<(u64, u64, u64)>, snapshot_ts: u64, active: Seq<u64>, from: int,
) -> int
    decreases chain.len() - from
{
    if from >= chain.len() {
        -1int
    } else if snapshot_visible_spec(
        chain[from].0, chain[from].1, chain[from].2,
        snapshot_ts, active,
    ) {
        from
    } else {
        chain_latest_spec(chain, snapshot_ts, active, from + 1)
    }
}

// r[impl mvcc.chain-latest]
/// Scans a version chain and returns the index of the first visible version,
/// or chain.len() if none found.
pub fn chain_latest(
    chain: &[(u64, u64, u64)], snapshot_ts: u64, active: &[u64],
) -> (result: usize)
    ensures
        result <= chain@.len(),
        result < chain@.len() ==> snapshot_visible_spec(
            chain@[result as int].0, chain@[result as int].1, chain@[result as int].2,
            snapshot_ts, active@,
        ),
        result == chain@.len() ==> forall|k: int| #![trigger chain@[k]]
            0 <= k < chain@.len() ==>
            !snapshot_visible_spec(chain@[k].0, chain@[k].1, chain@[k].2, snapshot_ts, active@),
{
    let mut i: usize = 0;
    while i < chain.len()
        invariant
            0 <= i <= chain@.len(),
            forall|k: int| #![trigger chain@[k]]
                0 <= k < i as int ==>
                !snapshot_visible_spec(chain@[k].0, chain@[k].1, chain@[k].2, snapshot_ts, active@),
        decreases chain@.len() - i
    {
        let (begin_ts, end_ts, creator_txn) = chain[i];
        if snapshot_visible(begin_ts, end_ts, creator_txn, snapshot_ts, active) {
            return i;
        }
        i = i + 1;
    }
    chain.len()
}

// ==================== Proofs ====================

// r[verify mvcc.is-visible]
/// A live version (end_ts = TS_INFINITY) is visible to any snapshot
/// at or after its creation (except at u64::MAX itself, where the
/// strict < check fails).
proof fn live_version_visible(begin_ts: u64, snapshot_ts: u64)
    requires begin_ts <= snapshot_ts, snapshot_ts < TS_INFINITY
    ensures is_visible_spec(begin_ts, TS_INFINITY, snapshot_ts)
{}

// r[verify mvcc.is-visible]
/// A version is not visible before its creation time.
proof fn not_visible_before_creation(begin_ts: u64, end_ts: u64, snapshot_ts: u64)
    requires snapshot_ts < begin_ts
    ensures !is_visible_spec(begin_ts, end_ts, snapshot_ts)
{}

// r[verify mvcc.is-visible]
/// A version is not visible at or after its deletion time.
proof fn not_visible_after_deletion(begin_ts: u64, end_ts: u64, snapshot_ts: u64)
    requires (snapshot_ts as int) >= (end_ts as int)
    ensures !is_visible_spec(begin_ts, end_ts, snapshot_ts)
{}

// r[verify mvcc.is-visible]
/// Visibility requires begin_ts < end_ts (well-formed version).
proof fn visibility_implies_wellformed(begin_ts: u64, end_ts: u64, snapshot_ts: u64)
    requires is_visible_spec(begin_ts, end_ts, snapshot_ts)
    ensures (begin_ts as int) < (end_ts as int)
{}

// r[verify mvcc.is-visible]
/// Two snapshots at the same time see the same version.
proof fn same_snapshot_same_visibility(
    begin_ts: u64, end_ts: u64, ts_a: u64, ts_b: u64,
)
    requires ts_a == ts_b
    ensures is_visible_spec(begin_ts, end_ts, ts_a)
        == is_visible_spec(begin_ts, end_ts, ts_b)
{}

// r[verify mvcc.is-visible]
/// Once a version becomes invisible (snapshot moves past end_ts),
/// it stays invisible for all later snapshots.
proof fn invisible_stays_invisible(
    begin_ts: u64, end_ts: u64, ts_a: u64, ts_b: u64,
)
    requires
        !is_visible_spec(begin_ts, end_ts, ts_a),
        ts_b >= ts_a,
        begin_ts <= ts_a,
    ensures !is_visible_spec(begin_ts, end_ts, ts_b)
{}

// r[verify mvcc.snapshot-visible]
/// An uncommitted version is never snapshot-visible regardless of timestamps.
proof fn uncommitted_not_snapshot_visible(
    begin_ts: u64, end_ts: u64, creator_txn: u64,
    snapshot_ts: u64, active: Seq<u64>,
)
    requires !is_committed_spec(creator_txn, active)
    ensures !snapshot_visible_spec(begin_ts, end_ts, creator_txn, snapshot_ts, active)
{}

// r[verify mvcc.snapshot-visible]
/// Committing a transaction (removing from active set) can make
/// its versions visible.
proof fn commit_makes_visible(
    begin_ts: u64, end_ts: u64, creator_txn: u64,
    snapshot_ts: u64,
)
    requires
        is_visible_spec(begin_ts, end_ts, snapshot_ts),
    ensures
        snapshot_visible_spec(
            begin_ts, end_ts, creator_txn, snapshot_ts, Seq::<u64>::empty(),
        )
{
    assert(!Seq::<u64>::empty().contains(creator_txn));
}

// r[verify mvcc.version-superseded]
/// TS_INFINITY means the version is live (not superseded).
proof fn infinity_not_superseded()
    ensures !version_superseded_spec(TS_INFINITY)
{}

// r[verify mvcc.version-superseded]
/// Any finite end_ts means the version is superseded.
proof fn finite_is_superseded(end_ts: u64)
    requires end_ts != TS_INFINITY
    ensures version_superseded_spec(end_ts)
{}

// r[verify mvcc.can-gc]
/// A live version (end_ts = TS_INFINITY) can never be GC'd.
proof fn live_version_not_gc(oldest_active: u64)
    ensures !can_gc_spec(TS_INFINITY, oldest_active)
{}

// r[verify mvcc.can-gc]
/// GC-eligible versions are not visible to any active snapshot.
proof fn gc_not_visible(end_ts: u64, oldest_active: u64, snapshot_ts: u64)
    requires
        can_gc_spec(end_ts, oldest_active),
        snapshot_ts >= oldest_active,
    ensures !is_visible_spec(0, end_ts, snapshot_ts)
{}

// Chain latest proofs are embedded in the exec ensures above.

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn test_live_version_visible() {
        assert!(is_visible(10, TS_INFINITY, 15));
        assert!(is_visible(10, TS_INFINITY, 10));
    }

    #[test]
    fn test_deleted_version_visible_in_range() {
        assert!(is_visible(10, 20, 15));
        assert!(is_visible(10, 20, 10));
        assert!(!is_visible(10, 20, 20));
        assert!(!is_visible(10, 20, 9));
    }

    // r[verify mvcc.is-committed]
    #[test]
    fn test_is_committed() {
        assert!(is_committed(5, &[1, 2, 3]));
        assert!(!is_committed(2, &[1, 2, 3]));
        assert!(is_committed(1, &[]));
    }

    #[test]
    fn test_snapshot_visible() {
        // Committed + in range
        assert!(snapshot_visible(10, 20, 99, 15, &[]));
        // Uncommitted
        assert!(!snapshot_visible(10, 20, 99, 15, &[99]));
        // Out of range
        assert!(!snapshot_visible(10, 20, 99, 25, &[]));
    }

    // r[verify mvcc.chain-latest]
    #[test]
    fn test_chain_latest() {
        let chain = vec![
            (20, TS_INFINITY, 3), // newest, created at 20
            (10, 20, 2),          // superseded at 20
            (5, 10, 1),           // superseded at 10
        ];
        // Snapshot at 15 sees version created at 10 (index 1)
        assert_eq!(chain_latest(&chain, 15, &[]), 1);
        // Snapshot at 25 sees latest (index 0)
        assert_eq!(chain_latest(&chain, 25, &[]), 0);
        // Snapshot at 7 sees oldest (index 2)
        assert_eq!(chain_latest(&chain, 7, &[]), 2);
        // Snapshot at 3 sees nothing (returns chain.len())
        assert_eq!(chain_latest(&chain, 3, &[]), 3);
    }

    // r[verify mvcc.chain-latest]
    #[test]
    fn test_chain_latest_with_active() {
        let chain = vec![(20, TS_INFINITY, 3), (10, 20, 2)];
        // txn 3 is active, entry 1 out of range at snapshot 25 (end=20 <= 25)
        // So nothing visible
        assert_eq!(chain_latest(&chain, 25, &[3]), 2);
        // At snapshot 15: entry 0 active, entry 1 visible (10 <= 15 < 20, creator 2 committed)
        assert_eq!(chain_latest(&chain, 15, &[3]), 1);
        // Both active - nothing visible
        assert_eq!(chain_latest(&chain, 15, &[2, 3]), 2);
    }

    #[test]
    fn test_can_gc() {
        assert!(can_gc(10, 15)); // deleted at 10, oldest active at 15
        assert!(can_gc(10, 10)); // deleted at 10, oldest active at 10
        assert!(!can_gc(10, 5)); // oldest active still needs it
        assert!(!can_gc(TS_INFINITY, 100)); // live version
    }

    proptest! {
        #[test]
        fn prop_live_version_visible_after_creation(begin_ts: u64, snapshot_ts: u64) {
            if begin_ts <= snapshot_ts {
                prop_assert!(is_visible(begin_ts, TS_INFINITY, snapshot_ts));
            }
        }

        #[test]
        fn prop_not_visible_before_creation(begin_ts in 1u64..u64::MAX, offset in 1u64..1000) {
            let snapshot_ts = begin_ts.saturating_sub(offset);
            if snapshot_ts < begin_ts {
                prop_assert!(!is_visible(begin_ts, TS_INFINITY, snapshot_ts));
            }
        }

        // r[verify mvcc.is-committed]
        #[test]
        fn prop_committed_on_empty_active(creator_txn: u64) {
            prop_assert!(is_committed(creator_txn, &[]));
        }

        // r[verify mvcc.is-committed]
        #[test]
        fn prop_not_committed_when_in_active(creator_txn: u64, prefix_len in 0usize..5) {
            let mut active: Vec<u64> = (0..prefix_len as u64).collect();
            active.push(creator_txn);
            prop_assert!(!is_committed(creator_txn, &active));
        }

        #[test]
        fn prop_gc_implies_not_visible_to_newer(
            end_ts in 0u64..u64::MAX,
            oldest in 0u64..=u64::MAX,
            snapshot_ts in 0u64..=u64::MAX,
        ) {
            if can_gc(end_ts, oldest) && snapshot_ts >= oldest {
                prop_assert!(!is_visible(0, end_ts, snapshot_ts));
            }
        }

        #[test]
        fn prop_visibility_window(begin_ts in 0u64..1000, span in 1u64..1000) {
            let end_ts = begin_ts + span;
            // Every ts in [begin_ts, end_ts) is visible
            for ts in begin_ts..end_ts {
                prop_assert!(is_visible(begin_ts, end_ts, ts));
            }
            // end_ts itself is not visible
            prop_assert!(!is_visible(begin_ts, end_ts, end_ts));
        }
    }
}
