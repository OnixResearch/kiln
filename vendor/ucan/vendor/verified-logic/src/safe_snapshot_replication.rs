//! Safe snapshot replication facade.
//!
//! This composite admits a snapshot install only when the follower is behind
//! the compacted prefix, the final snapshot chunk is present and in bounds,
//! integrity verification succeeds, and source-side compaction is safe through
//! the snapshot index.

use vstd::prelude::*;

use crate::snapshot_integrity::SnapshotIntegrity;

verus! {

/// Public decision for one snapshot replication step.
pub enum SafeSnapshotReplicationDecision {
    Install { snapshot_index: u64, commit_index: u64 },
    RejectNotNeeded,
    RejectMissingChunk,
    RejectCorrupt,
    RejectUnsafeCompaction,
}

/// Final chunk validity for complete snapshot installation.
pub open spec fn safe_snapshot_replication_complete_chunk_spec(
    offset: u64,
    chunk_size: u64,
    total_size: u64,
) -> bool {
    &&& crate::snapshot_transfer::is_chunk_valid_spec(offset, chunk_size, total_size)
    &&& offset as int + chunk_size as int == total_size as int
}

/// Spec-level accepted predicate.
pub open spec fn safe_snapshot_replication_accepted_spec(
    next_index: u64,
    first_log_index: u64,
    offset: u64,
    chunk_size: u64,
    total_size: u64,
    integrity: SnapshotIntegrity,
    compact_through: u64,
    last_applied: u64,
    commit_index: u64,
    current_commit: u64,
) -> bool {
    &&& crate::install_snapshot::should_install_snapshot_spec(next_index, first_log_index)
    &&& safe_snapshot_replication_complete_chunk_spec(offset, chunk_size, total_size)
    &&& crate::snapshot_integrity::verify_integrity_spec(integrity)
    &&& crate::snapshot_compaction::can_compact_through_spec(
        compact_through,
        last_applied,
        commit_index,
    )
    &&& crate::install_snapshot::apply_snapshot_commit_spec(current_commit, compact_through)
        >= current_commit
}

/// Rejection is read-only over caller-owned commit and compacted-prefix state.
pub open spec fn safe_snapshot_replication_reject_preserves_spec(
    before_commit_index: u64,
    after_commit_index: u64,
    before_first_log_index: u64,
    after_first_log_index: u64,
) -> bool {
    before_commit_index == after_commit_index && before_first_log_index == after_first_log_index
}

/// Decision code: 1 = install, 2+ = rejects.
pub open spec fn safe_snapshot_replication_decision_spec(
    next_index: u64,
    first_log_index: u64,
    offset: u64,
    chunk_size: u64,
    total_size: u64,
    integrity: SnapshotIntegrity,
    compact_through: u64,
    last_applied: u64,
    commit_index: u64,
    current_commit: u64,
) -> int {
    if !crate::install_snapshot::should_install_snapshot_spec(next_index, first_log_index) {
        2
    } else if !safe_snapshot_replication_complete_chunk_spec(offset, chunk_size, total_size) {
        3
    } else if !crate::snapshot_integrity::verify_integrity_spec(integrity) {
        4
    } else if !crate::snapshot_compaction::can_compact_through_spec(
        compact_through,
        last_applied,
        commit_index,
    ) {
        5
    } else {
        1
    }
}

// r[impl composite-primitives.safe-snapshot-replication]
/// Admit a complete, integrity-checked snapshot install with safe compaction.
pub fn safe_snapshot_replication(
    next_index: u64,
    first_log_index: u64,
    offset: u64,
    chunk_size: u64,
    total_size: u64,
    integrity: &SnapshotIntegrity,
    compact_through: u64,
    last_applied: u64,
    commit_index: u64,
    current_commit: u64,
) -> (decision: SafeSnapshotReplicationDecision)
    ensures
        decision is Install ==> safe_snapshot_replication_decision_spec(
            next_index,
            first_log_index,
            offset,
            chunk_size,
            total_size,
            *integrity,
            compact_through,
            last_applied,
            commit_index,
            current_commit,
        ) == 1,
{
    if !crate::install_snapshot::should_install_snapshot(next_index, first_log_index) {
        return SafeSnapshotReplicationDecision::RejectNotNeeded;
    }
    if !crate::snapshot_transfer::is_chunk_valid(offset, chunk_size, total_size)
        || offset + chunk_size != total_size
    {
        return SafeSnapshotReplicationDecision::RejectMissingChunk;
    }
    if !crate::snapshot_integrity::verify_integrity(integrity) {
        return SafeSnapshotReplicationDecision::RejectCorrupt;
    }
    if !crate::snapshot_compaction::can_compact_through(
        compact_through,
        last_applied,
        commit_index,
    ) {
        return SafeSnapshotReplicationDecision::RejectUnsafeCompaction;
    }
    let new_commit = crate::install_snapshot::apply_snapshot_commit(current_commit, compact_through);
    SafeSnapshotReplicationDecision::Install { snapshot_index: compact_through, commit_index: new_commit }
}

// r[verify composite-primitives.safe-snapshot-replication]
/// Accepted installs refine chunk, integrity, install-trigger, and compaction facts.
pub proof fn safe_snapshot_replication_refines_sources(
    next_index: u64,
    first_log_index: u64,
    offset: u64,
    chunk_size: u64,
    total_size: u64,
    integrity: SnapshotIntegrity,
    compact_through: u64,
    last_applied: u64,
    commit_index: u64,
    current_commit: u64,
)
    requires safe_snapshot_replication_decision_spec(
        next_index,
        first_log_index,
        offset,
        chunk_size,
        total_size,
        integrity,
        compact_through,
        last_applied,
        commit_index,
        current_commit,
    ) == 1
    ensures safe_snapshot_replication_accepted_spec(
        next_index,
        first_log_index,
        offset,
        chunk_size,
        total_size,
        integrity,
        compact_through,
        last_applied,
        commit_index,
        current_commit,
    )
{}

// r[verify composite-primitives.safe-snapshot-replication]
/// Reject paths do not mutate caller-owned snapshot position state.
pub proof fn safe_snapshot_replication_reject_is_read_only(commit_index: u64, first_log_index: u64)
    ensures safe_snapshot_replication_reject_preserves_spec(
        commit_index,
        commit_index,
        first_log_index,
        first_log_index,
    )
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    fn good_integrity() -> SnapshotIntegrity {
        crate::snapshot_integrity::compute_integrity(11, 22, 33)
    }

    // r[verify composite-primitives.safe-snapshot-replication]
    #[test]
    fn installs_complete_verified_snapshot() {
        let integrity = good_integrity();
        let decision = safe_snapshot_replication(5, 10, 80, 20, 100, &integrity, 10, 10, 12, 7);
        assert!(matches!(decision, SafeSnapshotReplicationDecision::Install {
            snapshot_index: 10,
            commit_index: 10
        }));
    }

    // r[verify composite-primitives.safe-snapshot-replication]
    #[test]
    fn rejects_corrupt_and_unsafe_snapshot() {
        let mut corrupt = good_integrity();
        corrupt.combined_hash = corrupt.combined_hash.wrapping_add(1);
        let corrupt_decision = safe_snapshot_replication(5, 10, 80, 20, 100, &corrupt, 10, 10, 12, 7);
        assert!(matches!(corrupt_decision, SafeSnapshotReplicationDecision::RejectCorrupt));

        let unsafe_compaction = safe_snapshot_replication(5, 10, 80, 20, 100, &good_integrity(), 13, 10, 12, 7);
        assert!(matches!(unsafe_compaction, SafeSnapshotReplicationDecision::RejectUnsafeCompaction));
    }
}
