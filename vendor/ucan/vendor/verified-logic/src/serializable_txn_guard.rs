//! Serializable transaction guard facade.
//!
//! Validity boundary: commit admission is accepted only when MVCC visibility,
//! snapshot-isolation conflict checks, dependency-cycle checks, timestamp
//! ordering, and two-phase-lock phase facts all agree. Any write-write
//! conflict, modeled self/two-cycle, timestamp disorder, or 2PL violation
//! rejects the commit without changing transaction state.

use vstd::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SerializableTxnGuardOutcome {
    Accepted,
    Rejected,
}

verus! {

// r[impl composite-primitives.serializable-txn-guard]
pub open spec fn serializable_txn_guard_spec(
    begin_ts: u64,
    end_ts: u64,
    creator_txn: u64,
    snapshot_ts: u64,
    active: Seq<u64>,
    write_set: Seq<u64>,
    concurrent_write_sets: Seq<Seq<u64>>,
    dep_edges: Seq<(u64, u64)>,
    ts_edges: Seq<(usize, usize)>,
    timestamps: Seq<u64>,
    lock_phase: u8,
    held_locks: u64,
    next_lock_op: u8,
) -> bool {
    crate::mvcc::snapshot_visible_spec(begin_ts, end_ts, creator_txn, snapshot_ts, active)
    && crate::snapshot_isolation::si_allows_commit_spec(write_set, concurrent_write_sets, 0)
    && !crate::txn_dependency::has_self_dep_spec(dep_edges)
    && !crate::txn_dependency::has_two_cycle_spec(dep_edges)
    && crate::txn_dependency::ts_ordered_spec(ts_edges, timestamps)
    && !crate::two_phase_lock::would_violate_spec(lock_phase, next_lock_op)
    && (crate::two_phase_lock::can_acquire_spec(lock_phase)
        || crate::two_phase_lock::is_complete_spec(held_locks, lock_phase))
}

// r[verify composite-primitives.serializable-txn-guard]
pub proof fn serializable_txn_guard_refines_sources(
    begin_ts: u64,
    end_ts: u64,
    creator_txn: u64,
    snapshot_ts: u64,
    active: Seq<u64>,
    write_set: Seq<u64>,
    concurrent_write_sets: Seq<Seq<u64>>,
    dep_edges: Seq<(u64, u64)>,
    ts_edges: Seq<(usize, usize)>,
    timestamps: Seq<u64>,
    lock_phase: u8,
    held_locks: u64,
    next_lock_op: u8,
)
    ensures
        serializable_txn_guard_spec(
            begin_ts,
            end_ts,
            creator_txn,
            snapshot_ts,
            active,
            write_set,
            concurrent_write_sets,
            dep_edges,
            ts_edges,
            timestamps,
            lock_phase,
            held_locks,
            next_lock_op,
        ) ==> crate::mvcc::snapshot_visible_spec(begin_ts, end_ts, creator_txn, snapshot_ts, active),
        serializable_txn_guard_spec(
            begin_ts,
            end_ts,
            creator_txn,
            snapshot_ts,
            active,
            write_set,
            concurrent_write_sets,
            dep_edges,
            ts_edges,
            timestamps,
            lock_phase,
            held_locks,
            next_lock_op,
        ) ==> crate::snapshot_isolation::si_allows_commit_spec(write_set, concurrent_write_sets, 0),
        serializable_txn_guard_spec(
            begin_ts,
            end_ts,
            creator_txn,
            snapshot_ts,
            active,
            write_set,
            concurrent_write_sets,
            dep_edges,
            ts_edges,
            timestamps,
            lock_phase,
            held_locks,
            next_lock_op,
        ) ==> !crate::txn_dependency::has_self_dep_spec(dep_edges),
        serializable_txn_guard_spec(
            begin_ts,
            end_ts,
            creator_txn,
            snapshot_ts,
            active,
            write_set,
            concurrent_write_sets,
            dep_edges,
            ts_edges,
            timestamps,
            lock_phase,
            held_locks,
            next_lock_op,
        ) ==> !crate::txn_dependency::has_two_cycle_spec(dep_edges),
        serializable_txn_guard_spec(
            begin_ts,
            end_ts,
            creator_txn,
            snapshot_ts,
            active,
            write_set,
            concurrent_write_sets,
            dep_edges,
            ts_edges,
            timestamps,
            lock_phase,
            held_locks,
            next_lock_op,
        ) ==> crate::txn_dependency::ts_ordered_spec(ts_edges, timestamps),
        serializable_txn_guard_spec(
            begin_ts,
            end_ts,
            creator_txn,
            snapshot_ts,
            active,
            write_set,
            concurrent_write_sets,
            dep_edges,
            ts_edges,
            timestamps,
            lock_phase,
            held_locks,
            next_lock_op,
        ) ==> !crate::two_phase_lock::would_violate_spec(lock_phase, next_lock_op),
{
}

// r[verify composite-primitives.serializable-txn-guard]
pub proof fn serializable_txn_guard_rejects_risks(
    begin_ts: u64,
    end_ts: u64,
    creator_txn: u64,
    snapshot_ts: u64,
    active: Seq<u64>,
    write_set: Seq<u64>,
    concurrent_write_sets: Seq<Seq<u64>>,
    dep_edges: Seq<(u64, u64)>,
    ts_edges: Seq<(usize, usize)>,
    timestamps: Seq<u64>,
    lock_phase: u8,
    held_locks: u64,
    next_lock_op: u8,
)
    ensures
        (!crate::snapshot_isolation::si_allows_commit_spec(write_set, concurrent_write_sets, 0)
            || crate::txn_dependency::has_self_dep_spec(dep_edges)
            || crate::txn_dependency::has_two_cycle_spec(dep_edges)
            || !crate::txn_dependency::ts_ordered_spec(ts_edges, timestamps)
            || crate::two_phase_lock::would_violate_spec(lock_phase, next_lock_op)) ==>
        !serializable_txn_guard_spec(
            begin_ts,
            end_ts,
            creator_txn,
            snapshot_ts,
            active,
            write_set,
            concurrent_write_sets,
            dep_edges,
            ts_edges,
            timestamps,
            lock_phase,
            held_locks,
            next_lock_op,
        ),
{
}

} // verus!

// r[impl composite-primitives.serializable-txn-guard]
pub fn serializable_txn_guard(
    begin_ts: u64,
    end_ts: u64,
    creator_txn: u64,
    snapshot_ts: u64,
    active: &[u64],
    write_set: &[u64],
    concurrent_write_sets: &[&[u64]],
    dep_edges: &[(u64, u64)],
    ts_edges: &[(usize, usize)],
    timestamps: &[u64],
    lock_phase: u8,
    held_locks: u64,
    next_lock_op: u8,
) -> SerializableTxnGuardOutcome {
    if !crate::mvcc::snapshot_visible(begin_ts, end_ts, creator_txn, snapshot_ts, active) {
        return SerializableTxnGuardOutcome::Rejected;
    }

    for other in concurrent_write_sets {
        if crate::snapshot_isolation::has_ww_conflict(write_set, other) {
            return SerializableTxnGuardOutcome::Rejected;
        }
    }

    if crate::txn_dependency::has_self_dep(dep_edges) || crate::txn_dependency::has_two_cycle(dep_edges) {
        return SerializableTxnGuardOutcome::Rejected;
    }

    for (from, to) in ts_edges {
        if *from >= timestamps.len() || *to >= timestamps.len() || timestamps[*from] >= timestamps[*to] {
            return SerializableTxnGuardOutcome::Rejected;
        }
    }

    if crate::two_phase_lock::would_violate(lock_phase, next_lock_op) {
        return SerializableTxnGuardOutcome::Rejected;
    }
    if !(crate::two_phase_lock::can_acquire(lock_phase) || crate::two_phase_lock::is_complete(held_locks, lock_phase)) {
        return SerializableTxnGuardOutcome::Rejected;
    }

    SerializableTxnGuardOutcome::Accepted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflict_free_transaction_commits() {
        let active = [];
        let writes = [10, 11];
        let other_a = [20, 21];
        let other_b = [30, 31];
        let concurrent: [&[u64]; 2] = [&other_a, &other_b];
        let deps = [(0, 1), (1, 2)];
        let ts_edges = [(0usize, 1usize), (1usize, 2usize)];
        let timestamps = [1, 2, 3];

        let outcome = serializable_txn_guard(
            1,
            crate::mvcc::TS_INFINITY,
            7,
            10,
            &active,
            &writes,
            &concurrent,
            &deps,
            &ts_edges,
            &timestamps,
            crate::two_phase_lock::PHASE_GROWING,
            2,
            crate::two_phase_lock::OP_ACQUIRE,
        );
        assert_eq!(outcome, SerializableTxnGuardOutcome::Accepted);
    }

    #[test]
    fn conflict_cycle_or_2pl_risk_rejects() {
        let active = [];
        let writes = [10, 11];
        let conflicting = [9, 10];
        let concurrent: [&[u64]; 1] = [&conflicting];
        let deps = [(0, 1), (1, 0)];
        let ts_edges = [(0usize, 1usize)];
        let timestamps = [1, 2];

        let conflict = serializable_txn_guard(
            1,
            crate::mvcc::TS_INFINITY,
            7,
            10,
            &active,
            &writes,
            &concurrent,
            &[],
            &ts_edges,
            &timestamps,
            crate::two_phase_lock::PHASE_GROWING,
            1,
            crate::two_phase_lock::OP_ACQUIRE,
        );
        assert_eq!(conflict, SerializableTxnGuardOutcome::Rejected);

        let cycle = serializable_txn_guard(
            1,
            crate::mvcc::TS_INFINITY,
            7,
            10,
            &active,
            &writes,
            &[],
            &deps,
            &ts_edges,
            &timestamps,
            crate::two_phase_lock::PHASE_GROWING,
            1,
            crate::two_phase_lock::OP_ACQUIRE,
        );
        assert_eq!(cycle, SerializableTxnGuardOutcome::Rejected);

        let phase_violation = serializable_txn_guard(
            1,
            crate::mvcc::TS_INFINITY,
            7,
            10,
            &active,
            &writes,
            &[],
            &[],
            &ts_edges,
            &timestamps,
            crate::two_phase_lock::PHASE_SHRINKING,
            0,
            crate::two_phase_lock::OP_ACQUIRE,
        );
        assert_eq!(phase_violation, SerializableTxnGuardOutcome::Rejected);
    }
}
