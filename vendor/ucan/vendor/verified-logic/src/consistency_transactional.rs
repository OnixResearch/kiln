//! Transactional consistency-model predicates.
//!
//! This module models the transactional family in terms of reusable witness
//! booleans and small helper checks. The goal is to make the refinement edges
//! explicit without pretending this is already a full history checker.

#![allow(clippy::fn_params_excessive_bools, clippy::nonminimal_bool)]

use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::cycle_detect::has_cycle_spec;
#[cfg(verus_keep_ghost)]
use crate::snapshot_isolation::has_ww_conflict_spec;

verus! {

// r[depends consistency-txn.read-uncommitted]
/// Spec: read uncommitted still forbids dirty writes.
pub open spec fn read_uncommitted_spec(no_dirty_writes: bool) -> bool {
    no_dirty_writes
}

// r[impl consistency-txn.read-uncommitted]
pub fn read_uncommitted(no_dirty_writes: bool) -> (result: bool)
    ensures result == read_uncommitted_spec(no_dirty_writes)
{
    no_dirty_writes
}

// r[depends consistency-txn.read-committed]
/// Spec: read committed adds dirty-read protection on top of read
/// uncommitted's dirty-write protection.
pub open spec fn read_committed_spec(no_dirty_reads: bool, no_dirty_writes: bool) -> bool {
    read_uncommitted_spec(no_dirty_writes) && no_dirty_reads
}

// r[impl consistency-txn.read-committed]
pub fn read_committed(no_dirty_reads: bool, no_dirty_writes: bool) -> (result: bool)
    ensures result == read_committed_spec(no_dirty_reads, no_dirty_writes)
{
    read_uncommitted(no_dirty_writes) && no_dirty_reads
}

// r[depends consistency-txn.cursor-stability]
/// Spec: cursor stability is read committed plus cursor-held protection for
/// objects currently under a cursor.
pub open spec fn cursor_stability_spec(
    no_dirty_reads: bool, no_dirty_writes: bool, cursor_protected: bool,
) -> bool {
    read_committed_spec(no_dirty_reads, no_dirty_writes) && cursor_protected
}

// r[impl consistency-txn.cursor-stability]
pub fn cursor_stability(
    no_dirty_reads: bool, no_dirty_writes: bool, cursor_protected: bool,
) -> (result: bool)
    ensures result == cursor_stability_spec(no_dirty_reads, no_dirty_writes, cursor_protected)
{
    read_committed(no_dirty_reads, no_dirty_writes) && cursor_protected
}

// r[depends consistency-txn.monotonic-atomic-view]
/// Spec: monotonic atomic view is read committed plus all-or-none visibility
/// for already-observed committed transactions.
pub open spec fn monotonic_atomic_view_spec(
    no_dirty_reads: bool, no_dirty_writes: bool, atomic_visibility: bool,
) -> bool {
    read_committed_spec(no_dirty_reads, no_dirty_writes) && atomic_visibility
}

// r[impl consistency-txn.monotonic-atomic-view]
pub fn monotonic_atomic_view(
    no_dirty_reads: bool, no_dirty_writes: bool, atomic_visibility: bool,
) -> (result: bool)
    ensures result == monotonic_atomic_view_spec(no_dirty_reads, no_dirty_writes, atomic_visibility)
{
    read_committed(no_dirty_reads, no_dirty_writes) && atomic_visibility
}

// r[depends consistency-txn.repeatable-read]
/// Spec: repeatable read is read committed plus stable per-object rereads.
pub open spec fn repeatable_read_spec(
    no_dirty_reads: bool, no_dirty_writes: bool, stable_object_reads: bool,
) -> bool {
    read_committed_spec(no_dirty_reads, no_dirty_writes) && stable_object_reads
}

// r[impl consistency-txn.repeatable-read]
pub fn repeatable_read(
    no_dirty_reads: bool, no_dirty_writes: bool, stable_object_reads: bool,
) -> (result: bool)
    ensures result == repeatable_read_spec(no_dirty_reads, no_dirty_writes, stable_object_reads)
{
    read_committed(no_dirty_reads, no_dirty_writes) && stable_object_reads
}

// r[depends consistency-txn.predicate-stability]
/// Spec: a predicate read is stable when re-evaluating it yields the same
/// membership result.
pub open spec fn predicate_stability_spec(previous_result: bool, later_result: bool) -> bool {
    previous_result == later_result
}

// r[impl consistency-txn.predicate-stability]
pub fn predicate_stability(previous_result: bool, later_result: bool) -> (result: bool)
    ensures result == predicate_stability_spec(previous_result, later_result)
{
    previous_result == later_result
}

// r[depends consistency-txn.first-committer-wins]
/// Spec: overlapping snapshots with conflicting write sets cannot both commit.
pub open spec fn first_committer_wins_spec(
    ws_a: Seq<u64>, ws_b: Seq<u64>, overlapping_snapshots: bool, both_commit: bool,
) -> bool {
    !(overlapping_snapshots && has_ww_conflict_spec(ws_a, ws_b) && both_commit)
}

// r[impl consistency-txn.first-committer-wins]
pub fn first_committer_wins(
    ws_a: &[u64], ws_b: &[u64], overlapping_snapshots: bool, both_commit: bool,
) -> (result: bool)
    ensures result == first_committer_wins_spec(ws_a@, ws_b@, overlapping_snapshots, both_commit)
{
    !(overlapping_snapshots && crate::snapshot_isolation::has_ww_conflict(ws_a, ws_b) && both_commit)
}

// r[depends consistency-txn.snapshot-isolation]
/// Spec: snapshot isolation is read committed plus a consistent snapshot,
/// atomic visibility, and first-committer-wins on overlapping writes.
pub open spec fn snapshot_isolation_spec(
    no_dirty_reads: bool,
    no_dirty_writes: bool,
    consistent_snapshot: bool,
    atomic_visibility: bool,
    first_committer_wins: bool,
) -> bool {
    read_committed_spec(no_dirty_reads, no_dirty_writes)
    && consistent_snapshot
    && atomic_visibility
    && first_committer_wins
}

// r[impl consistency-txn.snapshot-isolation]
pub fn snapshot_isolation(
    no_dirty_reads: bool,
    no_dirty_writes: bool,
    consistent_snapshot: bool,
    atomic_visibility: bool,
    first_committer_wins: bool,
) -> (result: bool)
    ensures result == snapshot_isolation_spec(
        no_dirty_reads,
        no_dirty_writes,
        consistent_snapshot,
        atomic_visibility,
        first_committer_wins,
    )
{
    read_committed(no_dirty_reads, no_dirty_writes)
        && consistent_snapshot
        && atomic_visibility
        && first_committer_wins
}

// r[depends consistency-txn.serializability]
/// Spec: serializability holds when there exists a serial witness order for
/// the committed transactions.
pub open spec fn serializability_spec(serial_witness_exists: bool) -> bool {
    serial_witness_exists
}

// r[impl consistency-txn.serializability]
pub fn serializability(serial_witness_exists: bool) -> (result: bool)
    ensures result == serializability_spec(serial_witness_exists)
{
    serial_witness_exists
}

// r[depends consistency-txn.dependency-acyclic]
/// Spec: one common serializability witness is an acyclic dependency graph.
pub open spec fn dependency_graph_acyclic_spec(edges: Seq<(int, int)>, n: int) -> bool {
    !has_cycle_spec(edges, n)
}

// r[impl consistency-txn.dependency-acyclic]
/// Checks one witness style for serializability: acyclic dependencies.
pub fn dependency_graph_acyclic(edges: &[(usize, usize)], n: usize) -> (result: bool)
    requires
        n as int + edges@.len() as int <= usize::MAX as int,
        forall|i: int| #![trigger edges@[i]] 0 <= i < edges@.len() ==> {
            let (u, v) = edges@[i];
            (u as int) < n as int && (v as int) < n as int
        },
    ensures
        result ==> dependency_graph_acyclic_spec(
            edges@.map(|_i: int, e: (usize, usize)| (e.0 as int, e.1 as int)),
            n as int,
        ),
{
    !crate::cycle_detect::has_cycle(edges, n)
}

// r[depends consistency-txn.strict-serializability]
/// Spec: strict serializability is serializability plus real-time and
/// per-session program-order respect across transactions.
pub open spec fn strict_serializability_spec(
    serial_witness_exists: bool,
    realtime_respected: bool,
    program_order_respected: bool,
) -> bool {
    serializability_spec(serial_witness_exists)
    && realtime_respected
    && program_order_respected
}

// r[impl consistency-txn.strict-serializability]
pub fn strict_serializability(
    serial_witness_exists: bool,
    realtime_respected: bool,
    program_order_respected: bool,
) -> (result: bool)
    ensures result == strict_serializability_spec(
        serial_witness_exists,
        realtime_respected,
        program_order_respected,
    )
{
    serializability(serial_witness_exists) && realtime_respected && program_order_respected
}

// r[verify consistency-txn.read-committed]
/// Read committed implies read uncommitted's dirty-write rule.
proof fn read_committed_implies_read_uncommitted(no_dirty_reads: bool, no_dirty_writes: bool)
    ensures read_committed_spec(no_dirty_reads, no_dirty_writes)
        ==> read_uncommitted_spec(no_dirty_writes)
{}

// r[verify consistency-txn.cursor-stability]
/// Cursor stability implies read committed.
proof fn cursor_stability_implies_read_committed(
    no_dirty_reads: bool, no_dirty_writes: bool, cursor_protected: bool,
)
    ensures cursor_stability_spec(no_dirty_reads, no_dirty_writes, cursor_protected)
        ==> read_committed_spec(no_dirty_reads, no_dirty_writes)
{}

// r[verify consistency-txn.monotonic-atomic-view]
/// Monotonic atomic view implies read committed.
proof fn mav_implies_read_committed(
    no_dirty_reads: bool, no_dirty_writes: bool, atomic_visibility: bool,
)
    ensures monotonic_atomic_view_spec(no_dirty_reads, no_dirty_writes, atomic_visibility)
        ==> read_committed_spec(no_dirty_reads, no_dirty_writes)
{}

// r[verify consistency-txn.repeatable-read]
/// Repeatable read implies read committed.
proof fn repeatable_read_implies_read_committed(
    no_dirty_reads: bool, no_dirty_writes: bool, stable_object_reads: bool,
)
    ensures repeatable_read_spec(no_dirty_reads, no_dirty_writes, stable_object_reads)
        ==> read_committed_spec(no_dirty_reads, no_dirty_writes)
{}

// r[verify consistency-txn.snapshot-isolation]
/// Snapshot isolation implies monotonic atomic view.
proof fn snapshot_isolation_implies_mav(
    no_dirty_reads: bool,
    no_dirty_writes: bool,
    consistent_snapshot: bool,
    atomic_visibility: bool,
    first_committer_wins: bool,
)
    ensures snapshot_isolation_spec(
        no_dirty_reads,
        no_dirty_writes,
        consistent_snapshot,
        atomic_visibility,
        first_committer_wins,
    ) ==> monotonic_atomic_view_spec(no_dirty_reads, no_dirty_writes, atomic_visibility)
{}

// r[verify consistency-txn.first-committer-wins]
/// Conflicting overlapping writers cannot both commit.
proof fn conflicting_overlap_blocks_double_commit(
    ws_a: Seq<u64>, ws_b: Seq<u64>, both_commit: bool,
)
    requires overlapping_conflict(ws_a, ws_b), both_commit
    ensures !first_committer_wins_spec(ws_a, ws_b, true, both_commit)
{}

// r[verify consistency-txn.dependency-acyclic]
/// A self-looping dependency graph is not acyclic.
proof fn self_loop_not_acyclic(n: int, v: int)
    requires 0 <= v < n
    ensures !dependency_graph_acyclic_spec(seq![(v, v)], n)
{
    let edges = seq![(v, v)];
    assert(crate::cycle_detect::has_cycle_spec(edges, n)) by {
        assert(crate::cycle_detect::has_edge_spec(edges, v, v)) by {
            assert(edges[0] == (v, v));
        }
        assert(crate::cycle_detect::has_path_fuel(edges, v, v, 0));
        assert(crate::cycle_detect::has_nontrivial_cycle_at(edges, n, v));
    }
}

// r[verify consistency-txn.serializability]
/// A provided serial witness is enough to satisfy the abstract
/// serializability predicate.
proof fn serial_witness_is_serializable(serial_witness_exists: bool)
    ensures serial_witness_exists ==> serializability_spec(serial_witness_exists)
{}

// r[verify consistency-txn.strict-serializability]
/// Strict serializability implies serializability.
proof fn strict_serializable_implies_serializable(
    serial_witness_exists: bool,
    realtime_respected: bool,
    program_order_respected: bool,
)
    ensures strict_serializability_spec(
        serial_witness_exists,
        realtime_respected,
        program_order_respected,
    ) ==> serializability_spec(serial_witness_exists)
{}

/// Helper predicate for the conflicting-overlap proof.
pub open spec fn overlapping_conflict(ws_a: Seq<u64>, ws_b: Seq<u64>) -> bool {
    has_ww_conflict_spec(ws_a, ws_b)
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consistency_history::HistoryOp;
    use crate::consistency_history::OP_WRITE;

    fn write(op_id: u64, txn: u64, session: u64, object: u64, invoke: u64, respond: u64) -> HistoryOp {
        HistoryOp {
            op_id,
            kind: OP_WRITE,
            object,
            txn,
            session,
            invoke,
            respond,
            observed: 0,
            value: op_id,
        }
    }

    // r[verify consistency-txn.read-uncommitted]
    #[test]
    fn test_read_uncommitted_tracks_dirty_writes() {
        assert!(read_uncommitted(true));
        assert!(!read_uncommitted(false));
    }

    // r[verify consistency-txn.read-committed]
    #[test]
    fn test_read_committed_adds_dirty_read_protection() {
        assert!(read_committed(true, true));
        assert!(!read_committed(false, true));
    }

    // r[verify consistency-txn.repeatable-read]
    // r[verify consistency-txn.predicate-stability]
    #[test]
    fn test_repeatable_read_allows_phantoms() {
        assert!(repeatable_read(true, true, true));
        assert!(!predicate_stability(true, false));
    }

    // r[verify consistency-txn.first-committer-wins]
    #[test]
    fn test_first_committer_wins_blocks_conflicting_overlap() {
        let t1 = write(1, 10, 1, 7, 0, 10);
        let t2 = write(2, 20, 2, 7, 5, 12);
        let overlapping = t1.respond > t2.invoke && t2.respond > t1.invoke;
        assert!(overlapping);
        assert!(!first_committer_wins(&[t1.object], &[t2.object], overlapping, true));
    }

    // r[verify consistency-txn.snapshot-isolation]
    #[test]
    fn test_snapshot_isolation_allows_write_skew_shape() {
        let t1 = write(1, 10, 1, 7, 0, 10);
        let t2 = write(2, 20, 2, 8, 5, 12);
        let overlapping = t1.respond > t2.invoke && t2.respond > t1.invoke;
        let fcw = first_committer_wins(&[t1.object], &[t2.object], overlapping, true);
        assert!(fcw);
        assert!(snapshot_isolation(true, true, true, true, fcw));
    }

    // r[verify consistency-txn.dependency-acyclic]
    #[test]
    fn test_dependency_graph_acyclic() {
        let dag = [(0usize, 1usize), (1usize, 2usize)];
        let cycle = [(0usize, 1usize), (1usize, 0usize)];
        assert!(dependency_graph_acyclic(&dag, 3));
        assert!(!dependency_graph_acyclic(&cycle, 2));
    }

    // r[verify consistency-txn.strict-serializability]
    #[test]
    fn test_strict_serializability_strengthens_serializability() {
        assert!(strict_serializability(true, true, true));
        assert!(!strict_serializability(true, false, true));
    }
}
