//! Verified snapshot isolation conflict detection.
//!
//! Models write-write conflict detection for snapshot isolation (SI).
//! Two concurrent transactions conflict if their write sets intersect.
//! SI allows commit only when no write-write conflict exists with any
//! concurrently committed transaction.
//!
//! Write sets and read sets are Seq<u64> of key identifiers.

#![allow(clippy::nonminimal_bool)]

use vstd::prelude::*;

verus! {

// --- has_ww_conflict ---

// r[depends snapshot-iso.has-ww-conflict]
/// Spec: two write sets have a write-write conflict if they share any key.
pub open spec fn has_ww_conflict_spec(ws_a: Seq<u64>, ws_b: Seq<u64>) -> bool {
    exists|i: int, j: int|
        #![trigger ws_a[i], ws_b[j]]
        0 <= i < ws_a.len() && 0 <= j < ws_b.len() && ws_a[i] == ws_b[j]
}

/// Spec helper: check conflict starting from indices (from_a, from_b).
pub open spec fn has_conflict_from_spec(
    ws_a: Seq<u64>, ws_b: Seq<u64>, from_a: int,
) -> bool
    decreases ws_a.len() - from_a
{
    if from_a >= ws_a.len() {
        false
    } else {
        ws_b.contains(ws_a[from_a])
        || has_conflict_from_spec(ws_a, ws_b, from_a + 1)
    }
}

/// has_conflict_from matches has_ww_conflict when from_a == 0.
proof fn has_conflict_from_equiv(ws_a: Seq<u64>, ws_b: Seq<u64>, from_a: int)
    requires 0 <= from_a
    ensures has_conflict_from_spec(ws_a, ws_b, from_a) <==>
        exists|i: int, j: int|
            #![trigger ws_a[i], ws_b[j]]
            from_a <= i < ws_a.len() && 0 <= j < ws_b.len() && ws_a[i] == ws_b[j]
    decreases ws_a.len() - from_a
{
    if from_a < ws_a.len() {
        has_conflict_from_equiv(ws_a, ws_b, from_a + 1);
    }
}

// r[impl snapshot-iso.has-ww-conflict]
/// Returns true if the two write sets share any key.
pub fn has_ww_conflict(ws_a: &[u64], ws_b: &[u64]) -> (result: bool)
    ensures result == has_ww_conflict_spec(ws_a@, ws_b@)
{
    let mut i: usize = 0;
    while i < ws_a.len()
        invariant
            0 <= i <= ws_a@.len(),
            !has_conflict_from_spec(ws_a@, ws_b@, 0)
                || has_conflict_from_spec(ws_a@, ws_b@, i as int),
            forall|ii: int, jj: int|
                #![trigger ws_a@[ii], ws_b@[jj]]
                0 <= ii < i as int && 0 <= jj < ws_b@.len()
                ==> ws_a@[ii] != ws_b@[jj],
        decreases ws_a@.len() - i
    {
        let mut j: usize = 0;
        while j < ws_b.len()
            invariant
                0 <= i < ws_a@.len(),
                0 <= j <= ws_b@.len(),
                forall|ii: int, jj: int|
                    #![trigger ws_a@[ii], ws_b@[jj]]
                    0 <= ii < i as int && 0 <= jj < ws_b@.len()
                    ==> ws_a@[ii] != ws_b@[jj],
                forall|jj: int|
                    #![trigger ws_b@[jj]]
                    0 <= jj < j as int ==> ws_a@[i as int] != ws_b@[jj],
            decreases ws_b@.len() - j
        {
            if ws_a[i] == ws_b[j] {
                proof {
                    has_conflict_from_equiv(ws_a@, ws_b@, 0);
                    assert(ws_a@[i as int] == ws_b@[j as int]);
                }
                return true;
            }
            j = j + 1;
        }
        i = i + 1;
    }
    proof {
        has_conflict_from_equiv(ws_a@, ws_b@, 0);
    }
    false
}

// --- si_allows_commit ---

// r[depends snapshot-iso.si-allows-commit]
// r[impl snapshot-iso.si-allows-commit]
/// Spec: SI allows commit of a transaction with write set `ws` when
/// none of the concurrently committed transactions' write sets conflict.
/// `concurrent_ws` is a Seq of write sets (Seq<Seq<u64>>).
pub open spec fn si_allows_commit_spec(
    ws: Seq<u64>, concurrent_ws: Seq<Seq<u64>>, from: int,
) -> bool
    decreases concurrent_ws.len() - from
{
    if from >= concurrent_ws.len() {
        true
    } else {
        !has_ww_conflict_spec(ws, concurrent_ws[from])
        && si_allows_commit_spec(ws, concurrent_ws, from + 1)
    }
}

// --- write_set_size ---

// r[depends snapshot-iso.write-set-size]
/// Spec: number of keys in the write set.
pub open spec fn write_set_size_spec(ws: Seq<u64>) -> int {
    ws.len() as int
}

// r[impl snapshot-iso.write-set-size]
pub fn write_set_size(ws: &[u64]) -> (result: usize)
    ensures result as int == write_set_size_spec(ws@)
{
    ws.len()
}

// --- read_write_overlap ---

// r[depends snapshot-iso.rw-overlap]
/// Spec: the read set of one transaction overlaps the write set of another.
/// This models a read-write dependency (relevant for serializability but
/// allowed under snapshot isolation).
pub open spec fn rw_overlap_spec(read_set: Seq<u64>, write_set: Seq<u64>) -> bool {
    exists|i: int, j: int|
        #![trigger read_set[i], write_set[j]]
        0 <= i < read_set.len() && 0 <= j < write_set.len()
        && read_set[i] == write_set[j]
}

// r[impl snapshot-iso.rw-overlap]
pub fn rw_overlap(read_set: &[u64], write_set: &[u64]) -> (result: bool)
    ensures result == rw_overlap_spec(read_set@, write_set@)
{
    let mut i: usize = 0;
    while i < read_set.len()
        invariant
            0 <= i <= read_set@.len(),
            forall|ii: int, jj: int|
                #![trigger read_set@[ii], write_set@[jj]]
                0 <= ii < i as int && 0 <= jj < write_set@.len()
                ==> read_set@[ii] != write_set@[jj],
        decreases read_set@.len() - i
    {
        let mut j: usize = 0;
        while j < write_set.len()
            invariant
                0 <= i < read_set@.len(),
                0 <= j <= write_set@.len(),
                forall|ii: int, jj: int|
                    #![trigger read_set@[ii], write_set@[jj]]
                    0 <= ii < i as int && 0 <= jj < write_set@.len()
                    ==> read_set@[ii] != write_set@[jj],
                forall|jj: int|
                    #![trigger write_set@[jj]]
                    0 <= jj < j as int ==> read_set@[i as int] != write_set@[jj],
            decreases write_set@.len() - j
        {
            if read_set[i] == write_set[j] {
                return true;
            }
            j = j + 1;
        }
        i = i + 1;
    }
    false
}

// ==================== Proofs ====================

// r[verify snapshot-iso.has-ww-conflict]
/// Empty write set never conflicts.
proof fn empty_ws_no_conflict(ws: Seq<u64>)
    ensures !has_ww_conflict_spec(Seq::<u64>::empty(), ws)
{}

// r[verify snapshot-iso.has-ww-conflict]
/// Conflict detection is symmetric.
proof fn ww_conflict_symmetric(ws_a: Seq<u64>, ws_b: Seq<u64>)
    ensures has_ww_conflict_spec(ws_a, ws_b) == has_ww_conflict_spec(ws_b, ws_a)
{
    if has_ww_conflict_spec(ws_a, ws_b) {
        let wit = choose|i: int| #![trigger ws_a[i]]
            0 <= i < ws_a.len() && ws_b.contains(ws_a[i]);
        assert(ws_b.contains(ws_a[wit]));
        let j = choose|j: int| #![trigger ws_b[j]]
            0 <= j < ws_b.len() && ws_b[j] == ws_a[wit];
        assert(ws_a.contains(ws_b[j]));
        assert(ws_b[j] == ws_a[wit]);
    }
    if has_ww_conflict_spec(ws_b, ws_a) {
        let wit = choose|i: int| #![trigger ws_b[i]]
            0 <= i < ws_b.len() && ws_a.contains(ws_b[i]);
        assert(ws_a.contains(ws_b[wit]));
        let j = choose|j: int| #![trigger ws_a[j]]
            0 <= j < ws_a.len() && ws_a[j] == ws_b[wit];
        assert(ws_b.contains(ws_a[j]));
        assert(ws_a[j] == ws_b[wit]);
    }
}

// r[verify snapshot-iso.has-ww-conflict]
/// A write set always conflicts with itself if non-empty.
proof fn nonempty_self_conflict(ws: Seq<u64>)
    requires ws.len() > 0
    ensures has_ww_conflict_spec(ws, ws)
{
    assert(0 < ws.len());
    assert(ws[0int] == ws[0int]);
    assert(ws.contains(ws[0int]));
}

// r[verify snapshot-iso.si-allows-commit]
/// No concurrent transactions means commit is always allowed.
proof fn no_concurrent_always_commits(ws: Seq<u64>)
    ensures si_allows_commit_spec(ws, Seq::<Seq<u64>>::empty(), 0)
{}

// r[verify snapshot-iso.si-allows-commit]
/// Empty write set can always commit (no conflicts possible).
proof fn empty_ws_always_commits(concurrent_ws: Seq<Seq<u64>>, from: int)
    requires 0 <= from
    ensures si_allows_commit_spec(Seq::<u64>::empty(), concurrent_ws, from)
    decreases concurrent_ws.len() - from
{
    if from < concurrent_ws.len() {
        empty_ws_always_commits(concurrent_ws, from + 1);
    }
}

// r[verify snapshot-iso.rw-overlap]
/// Empty read set has no overlap.
proof fn empty_read_no_overlap(ws: Seq<u64>)
    ensures !rw_overlap_spec(Seq::<u64>::empty(), ws)
{}

// r[verify snapshot-iso.rw-overlap]
/// Empty write set has no overlap.
proof fn empty_write_no_overlap(rs: Seq<u64>)
    ensures !rw_overlap_spec(rs, Seq::<u64>::empty())
{}

// r[verify snapshot-iso.has-ww-conflict]
// r[verify snapshot-iso.rw-overlap]
/// Write-write conflict implies read-write overlap when the
/// read set is the same as the write set of transaction A.
proof fn ww_conflict_implies_rw_when_read_is_write(
    ws_a: Seq<u64>, ws_b: Seq<u64>,
)
    requires
        has_ww_conflict_spec(ws_a, ws_b),
    ensures
        rw_overlap_spec(ws_a, ws_b)
{
    // Both specs are: exists i,j: ws_a[i] == ws_b[j].
    // The trigger sets differ but the bodies are identical.
    assert(has_ww_conflict_spec(ws_a, ws_b));
    let i = choose|i: int| #![trigger ws_a[i]]
        0 <= i < ws_a.len() && ws_b.contains(ws_a[i]);
    let j = choose|j: int| #![trigger ws_b[j]]
        0 <= j < ws_b.len() && ws_b[j] == ws_a[i];
    assert(ws_a[i] == ws_b[j]);
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn test_no_conflict_disjoint() {
        assert!(!has_ww_conflict(&[1, 2, 3], &[4, 5, 6]));
    }

    #[test]
    fn test_conflict_overlap() {
        assert!(has_ww_conflict(&[1, 2, 3], &[3, 4, 5]));
    }

    #[test]
    fn test_no_conflict_empty() {
        assert!(!has_ww_conflict(&[], &[1, 2, 3]));
        assert!(!has_ww_conflict(&[1, 2, 3], &[]));
        assert!(!has_ww_conflict(&[], &[]));
    }

    #[test]
    fn test_rw_overlap() {
        assert!(rw_overlap(&[1, 2, 3], &[3, 4]));
        assert!(!rw_overlap(&[1, 2], &[3, 4]));
    }

    // r[verify snapshot-iso.write-set-size]
    #[test]
    fn test_write_set_size() {
        assert_eq!(write_set_size(&[1, 2, 3]), 3);
        assert_eq!(write_set_size(&[]), 0);
    }

    proptest! {
        #[test]
        fn prop_ww_conflict_symmetric(
            a in proptest::collection::vec(0u64..100, 0..=10),
            b in proptest::collection::vec(0u64..100, 0..=10),
        ) {
            prop_assert_eq!(has_ww_conflict(&a, &b), has_ww_conflict(&b, &a));
        }

        #[test]
        fn prop_empty_no_conflict(ws in proptest::collection::vec(any::<u64>(), 0..=10)) {
            prop_assert!(!has_ww_conflict(&[], &ws));
            prop_assert!(!has_ww_conflict(&ws, &[]));
        }

        #[test]
        fn prop_self_conflict_iff_nonempty(ws in proptest::collection::vec(any::<u64>(), 0..=10)) {
            if ws.is_empty() {
                prop_assert!(!has_ww_conflict(&ws, &ws));
            } else {
                prop_assert!(has_ww_conflict(&ws, &ws));
            }
        }

        #[test]
        fn prop_conflict_implies_shared_key(
            a in proptest::collection::vec(0u64..50, 1..=10),
            b in proptest::collection::vec(0u64..50, 1..=10),
        ) {
            if has_ww_conflict(&a, &b) {
                // There must be a shared key
                let a_set: std::collections::HashSet<u64> = a.iter().copied().collect();
                let b_set: std::collections::HashSet<u64> = b.iter().copied().collect();
                let intersection: Vec<u64> = a_set.intersection(&b_set).copied().collect();
                prop_assert!(!intersection.is_empty());
            }
        }

        #[test]
        fn prop_rw_overlap_empty_read(ws in proptest::collection::vec(any::<u64>(), 0..=10)) {
            prop_assert!(!rw_overlap(&[], &ws));
        }

        #[test]
        fn prop_rw_overlap_empty_write(rs in proptest::collection::vec(any::<u64>(), 0..=10)) {
            prop_assert!(!rw_overlap(&rs, &[]));
        }

        #[test]
        fn prop_ww_conflict_implies_rw_overlap(
            a in proptest::collection::vec(0u64..50, 1..=10),
            b in proptest::collection::vec(0u64..50, 1..=10),
        ) {
            // If write sets conflict and we treat a as both read and write set,
            // then there's a read-write overlap
            if has_ww_conflict(&a, &b) {
                prop_assert!(rw_overlap(&a, &b));
            }
        }
    }
}
