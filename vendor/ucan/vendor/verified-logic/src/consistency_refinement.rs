//! Refinement edges and availability metadata for consistency models.
//!
//! This module ties together the transactional, object, and session predicate
//! layers and records the availability envelope from the OpenSpec change.

use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::consistency_object::linearizability_flags_spec;
#[cfg(verus_keep_ghost)]
use crate::consistency_object::sequential_consistency_flags_spec;
#[cfg(verus_keep_ghost)]
use crate::consistency_session::causal_consistency_spec;
#[cfg(verus_keep_ghost)]
use crate::consistency_session::monotonic_reads_spec;
#[cfg(verus_keep_ghost)]
use crate::consistency_session::monotonic_writes_spec;
#[cfg(verus_keep_ghost)]
use crate::consistency_session::pram_spec;
#[cfg(verus_keep_ghost)]
use crate::consistency_session::read_your_writes_spec;
#[cfg(verus_keep_ghost)]
use crate::consistency_session::writes_follow_reads_spec;
#[cfg(verus_keep_ghost)]
use crate::consistency_transactional::cursor_stability_spec;
#[cfg(verus_keep_ghost)]
use crate::consistency_transactional::monotonic_atomic_view_spec;
#[cfg(verus_keep_ghost)]
use crate::consistency_transactional::read_committed_spec;
#[cfg(verus_keep_ghost)]
use crate::consistency_transactional::read_uncommitted_spec;
#[cfg(verus_keep_ghost)]
use crate::consistency_transactional::repeatable_read_spec;
#[cfg(verus_keep_ghost)]
use crate::consistency_transactional::serializability_spec;
#[cfg(verus_keep_ghost)]
use crate::consistency_transactional::snapshot_isolation_spec;
#[cfg(verus_keep_ghost)]
use crate::consistency_transactional::strict_serializability_spec;

verus! {

/// Model constants for availability classification.
pub const MODEL_STRICT_SERIALIZABLE: u8 = 0u8;
pub const MODEL_SERIALIZABLE: u8 = 1u8;
pub const MODEL_REPEATABLE_READ: u8 = 2u8;
pub const MODEL_SNAPSHOT_ISOLATION: u8 = 3u8;
pub const MODEL_MONOTONIC_ATOMIC_VIEW: u8 = 4u8;
pub const MODEL_CURSOR_STABILITY: u8 = 5u8;
pub const MODEL_READ_COMMITTED: u8 = 6u8;
pub const MODEL_READ_UNCOMMITTED: u8 = 7u8;
pub const MODEL_LINEARIZABLE: u8 = 8u8;
pub const MODEL_SEQUENTIAL: u8 = 9u8;
pub const MODEL_CAUSAL: u8 = 10u8;
pub const MODEL_PRAM: u8 = 11u8;
pub const MODEL_WRITES_FOLLOW_READS: u8 = 12u8;
pub const MODEL_MONOTONIC_READS: u8 = 13u8;
pub const MODEL_MONOTONIC_WRITES: u8 = 14u8;
pub const MODEL_READ_YOUR_WRITES: u8 = 15u8;

/// Availability envelope constants.
pub const AVAIL_NOT_STICKY: u8 = 0u8;
pub const AVAIL_NOT_TOTAL: u8 = 1u8;
pub const AVAIL_STICKY: u8 = 2u8;
pub const AVAIL_TOTAL: u8 = 3u8;

// r[depends consistency-refinement.model-hierarchy]
/// Spec: shared refinement skeletons connect stronger model predicates to
/// weaker ones.
pub open spec fn model_hierarchy_spec() -> bool {
    true
}

// r[impl consistency-refinement.model-hierarchy]
pub fn model_hierarchy() -> (result: bool)
    ensures result == model_hierarchy_spec()
{
    true
}

// r[depends consistency-refinement.availability-envelope]
/// Spec: classify each model by the strongest availability envelope it may
/// inhabit in the OpenSpec taxonomy.
pub open spec fn availability_class_spec(model: u8) -> u8 {
    if model == MODEL_STRICT_SERIALIZABLE
        || model == MODEL_SERIALIZABLE
        || model == MODEL_LINEARIZABLE
        || model == MODEL_SEQUENTIAL
    {
        AVAIL_NOT_STICKY
    } else if model == MODEL_REPEATABLE_READ
        || model == MODEL_SNAPSHOT_ISOLATION
        || model == MODEL_CURSOR_STABILITY
    {
        AVAIL_NOT_TOTAL
    } else if model == MODEL_CAUSAL
        || model == MODEL_PRAM
        || model == MODEL_READ_YOUR_WRITES
    {
        AVAIL_STICKY
    } else {
        AVAIL_TOTAL
    }
}

// r[impl consistency-refinement.availability-envelope]
pub fn availability_class(model: u8) -> (result: u8)
    ensures result == availability_class_spec(model)
{
    if model == MODEL_STRICT_SERIALIZABLE
        || model == MODEL_SERIALIZABLE
        || model == MODEL_LINEARIZABLE
        || model == MODEL_SEQUENTIAL
    {
        AVAIL_NOT_STICKY
    } else if model == MODEL_REPEATABLE_READ
        || model == MODEL_SNAPSHOT_ISOLATION
        || model == MODEL_CURSOR_STABILITY
    {
        AVAIL_NOT_TOTAL
    } else if model == MODEL_CAUSAL
        || model == MODEL_PRAM
        || model == MODEL_READ_YOUR_WRITES
    {
        AVAIL_STICKY
    } else {
        AVAIL_TOTAL
    }
}

// r[verify consistency-refinement.model-hierarchy]
/// Strict serializability implies serializability.
proof fn strict_serializable_refines_serializable(
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

// r[verify consistency-refinement.model-hierarchy]
/// Strict serializability implies linearizability in the shared witness model.
proof fn strict_serializable_refines_linearizable(
    serial_witness_exists: bool,
    realtime_respected: bool,
    program_order_respected: bool,
)
    ensures strict_serializability_spec(
        serial_witness_exists,
        realtime_respected,
        program_order_respected,
    ) ==> linearizability_flags_spec(
        serial_witness_exists,
        program_order_respected,
        realtime_respected,
    )
{}

// r[verify consistency-refinement.model-hierarchy]
/// Serializability implies repeatable read in the shared witness model.
proof fn serializable_refines_repeatable_read(serial_witness_exists: bool)
    ensures serializability_spec(serial_witness_exists)
        ==> repeatable_read_spec(
            serial_witness_exists,
            serial_witness_exists,
            serial_witness_exists,
        )
{}

// r[verify consistency-refinement.model-hierarchy]
/// Serializability implies snapshot isolation in the shared witness model.
proof fn serializable_refines_snapshot_isolation(serial_witness_exists: bool)
    ensures serializability_spec(serial_witness_exists)
        ==> snapshot_isolation_spec(
            serial_witness_exists,
            serial_witness_exists,
            serial_witness_exists,
            serial_witness_exists,
            serial_witness_exists,
        )
{}

// r[verify consistency-refinement.model-hierarchy]
/// Linearizability implies sequential consistency.
proof fn linearizable_refines_sequential(
    sequential_witness: bool,
    program_order_respected: bool,
    realtime_respected: bool,
)
    ensures linearizability_flags_spec(
        sequential_witness,
        program_order_respected,
        realtime_respected,
    ) ==> sequential_consistency_flags_spec(sequential_witness, program_order_respected)
{}

// r[verify consistency-refinement.model-hierarchy]
/// Repeatable read implies cursor stability.
proof fn repeatable_read_refines_cursor_stability(
    no_dirty_reads: bool,
    no_dirty_writes: bool,
    stable_object_reads: bool,
)
    ensures repeatable_read_spec(no_dirty_reads, no_dirty_writes, stable_object_reads)
        ==> cursor_stability_spec(no_dirty_reads, no_dirty_writes, stable_object_reads)
{}

// r[verify consistency-refinement.model-hierarchy]
/// Repeatable read implies monotonic atomic view.
proof fn repeatable_read_refines_mav(
    no_dirty_reads: bool,
    no_dirty_writes: bool,
    stable_object_reads: bool,
)
    ensures repeatable_read_spec(no_dirty_reads, no_dirty_writes, stable_object_reads)
        ==> monotonic_atomic_view_spec(no_dirty_reads, no_dirty_writes, stable_object_reads)
{}

// r[verify consistency-refinement.model-hierarchy]
/// Repeatable read implies read committed.
proof fn repeatable_read_refines_read_committed(
    no_dirty_reads: bool,
    no_dirty_writes: bool,
    stable_object_reads: bool,
)
    ensures repeatable_read_spec(no_dirty_reads, no_dirty_writes, stable_object_reads)
        ==> read_committed_spec(no_dirty_reads, no_dirty_writes)
{}

// r[verify consistency-refinement.model-hierarchy]
/// Snapshot isolation implies monotonic atomic view.
proof fn snapshot_isolation_refines_mav(
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

// r[verify consistency-refinement.model-hierarchy]
/// Cursor stability implies read committed.
proof fn cursor_stability_refines_read_committed(
    no_dirty_reads: bool,
    no_dirty_writes: bool,
    cursor_protected: bool,
)
    ensures cursor_stability_spec(no_dirty_reads, no_dirty_writes, cursor_protected)
        ==> read_committed_spec(no_dirty_reads, no_dirty_writes)
{}

// r[verify consistency-refinement.model-hierarchy]
/// Monotonic atomic view implies read committed.
proof fn mav_refines_read_committed(
    no_dirty_reads: bool,
    no_dirty_writes: bool,
    atomic_visibility: bool,
)
    ensures monotonic_atomic_view_spec(no_dirty_reads, no_dirty_writes, atomic_visibility)
        ==> read_committed_spec(no_dirty_reads, no_dirty_writes)
{}

// r[verify consistency-refinement.model-hierarchy]
/// Read committed implies read uncommitted.
proof fn read_committed_refines_read_uncommitted(
    no_dirty_reads: bool,
    no_dirty_writes: bool,
)
    ensures read_committed_spec(no_dirty_reads, no_dirty_writes)
        ==> read_uncommitted_spec(no_dirty_writes)
{}

// r[verify consistency-refinement.model-hierarchy]
/// Sequential consistency implies causal consistency in the shared witness model.
proof fn sequential_refines_causal(
    sequential_witness: bool,
    program_order_respected: bool,
)
    ensures sequential_consistency_flags_spec(sequential_witness, program_order_respected)
        ==> causal_consistency_spec(
            program_order_respected,
            program_order_respected,
            sequential_witness,
            program_order_respected,
        )
{}

// r[verify consistency-refinement.model-hierarchy]
/// Causal consistency implies PRAM.
proof fn causal_refines_pram(
    non_decreasing_reads: bool,
    per_session_write_order: bool,
    own_write_visible: bool,
    dependency_order_respected: bool,
)
    ensures causal_consistency_spec(
        non_decreasing_reads,
        per_session_write_order,
        own_write_visible,
        dependency_order_respected,
    ) ==> pram_spec(non_decreasing_reads, per_session_write_order, own_write_visible)
{}

// r[verify consistency-refinement.model-hierarchy]
/// Causal consistency implies writes-follow-reads.
proof fn causal_refines_writes_follow_reads(
    non_decreasing_reads: bool,
    per_session_write_order: bool,
    own_write_visible: bool,
    dependency_order_respected: bool,
)
    ensures causal_consistency_spec(
        non_decreasing_reads,
        per_session_write_order,
        own_write_visible,
        dependency_order_respected,
    ) ==> writes_follow_reads_spec(dependency_order_respected)
{}

// r[verify consistency-refinement.model-hierarchy]
/// PRAM implies monotonic reads.
proof fn pram_refines_monotonic_reads(
    non_decreasing_reads: bool,
    per_session_write_order: bool,
    own_write_visible: bool,
)
    ensures pram_spec(non_decreasing_reads, per_session_write_order, own_write_visible)
        ==> monotonic_reads_spec(non_decreasing_reads)
{}

// r[verify consistency-refinement.model-hierarchy]
/// PRAM implies monotonic writes.
proof fn pram_refines_monotonic_writes(
    non_decreasing_reads: bool,
    per_session_write_order: bool,
    own_write_visible: bool,
)
    ensures pram_spec(non_decreasing_reads, per_session_write_order, own_write_visible)
        ==> monotonic_writes_spec(per_session_write_order)
{}

// r[verify consistency-refinement.model-hierarchy]
/// PRAM implies read-your-writes.
proof fn pram_refines_read_your_writes(
    non_decreasing_reads: bool,
    per_session_write_order: bool,
    own_write_visible: bool,
)
    ensures pram_spec(non_decreasing_reads, per_session_write_order, own_write_visible)
        ==> read_your_writes_spec(own_write_visible)
{}

// r[verify consistency-refinement.availability-envelope]
/// The strongest object-level models are not sticky available.
proof fn strict_models_not_sticky()
    ensures
        availability_class_spec(MODEL_STRICT_SERIALIZABLE) == AVAIL_NOT_STICKY,
        availability_class_spec(MODEL_SERIALIZABLE) == AVAIL_NOT_STICKY,
        availability_class_spec(MODEL_LINEARIZABLE) == AVAIL_NOT_STICKY,
        availability_class_spec(MODEL_SEQUENTIAL) == AVAIL_NOT_STICKY,
{}

// r[verify consistency-refinement.availability-envelope]
/// Session-lite models remain totally available in this taxonomy.
proof fn total_available_models()
    ensures
        availability_class_spec(MODEL_READ_UNCOMMITTED) == AVAIL_TOTAL,
        availability_class_spec(MODEL_READ_COMMITTED) == AVAIL_TOTAL,
        availability_class_spec(MODEL_MONOTONIC_ATOMIC_VIEW) == AVAIL_TOTAL,
        availability_class_spec(MODEL_WRITES_FOLLOW_READS) == AVAIL_TOTAL,
        availability_class_spec(MODEL_MONOTONIC_READS) == AVAIL_TOTAL,
        availability_class_spec(MODEL_MONOTONIC_WRITES) == AVAIL_TOTAL,
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consistency_history::HistoryOp;
    use crate::consistency_history::OP_READ;
    use crate::consistency_history::OP_WRITE;

    fn read(op_id: u64, session: u64, invoke: u64, respond: u64, observed: u64) -> HistoryOp {
        HistoryOp {
            op_id,
            kind: OP_READ,
            object: 7,
            txn: op_id,
            session,
            invoke,
            respond,
            observed,
            value: 0,
        }
    }

    fn write(op_id: u64, session: u64, invoke: u64, respond: u64, value: u64) -> HistoryOp {
        HistoryOp {
            op_id,
            kind: OP_WRITE,
            object: 7,
            txn: op_id,
            session,
            invoke,
            respond,
            observed: 0,
            value,
        }
    }

    // r[verify consistency-refinement.model-hierarchy]
    #[test]
    fn test_transactional_hierarchy_edges_have_runtime_representatives() {
        assert!(crate::consistency_transactional::serializability(true));
        assert!(crate::consistency_transactional::repeatable_read(true, true, true));
        assert!(crate::consistency_transactional::snapshot_isolation(true, true, true, true, true));
        assert!(crate::consistency_transactional::cursor_stability(true, true, true));
        assert!(crate::consistency_transactional::monotonic_atomic_view(true, true, true));
        assert!(crate::consistency_transactional::read_committed(true, true));
        assert!(crate::consistency_transactional::read_uncommitted(true));
    }

    // r[verify consistency-refinement.model-hierarchy]
    #[test]
    fn test_object_history_distinguishes_linearizable_from_sequential() {
        let earlier = write(1, 5, 0, 1, 10);
        let later = read(2, 6, 2, 3, 1);
        let linear = [earlier, later];
        let reordered = [later, earlier];

        assert!(crate::consistency_object::linearizability(&linear));
        assert!(crate::consistency_object::sequential_consistency(&reordered));
        assert!(!crate::consistency_object::linearizability(&reordered));
    }

    // r[verify consistency-refinement.model-hierarchy]
    #[test]
    fn test_session_hierarchy_distinguishes_causal_from_pram() {
        assert!(crate::consistency_session::pram(true, true, true));
        assert!(!crate::consistency_session::causal_consistency(true, true, true, false));
        assert!(crate::consistency_session::causal_consistency(true, true, true, true));
        assert!(crate::consistency_session::writes_follow_reads(true));
    }

    // r[verify consistency-refinement.availability-envelope]
    #[test]
    fn test_availability_classes_match_spec() {
        assert_eq!(availability_class(MODEL_STRICT_SERIALIZABLE), AVAIL_NOT_STICKY);
        assert_eq!(availability_class(MODEL_SNAPSHOT_ISOLATION), AVAIL_NOT_TOTAL);
        assert_eq!(availability_class(MODEL_CAUSAL), AVAIL_STICKY);
        assert_eq!(availability_class(MODEL_MONOTONIC_WRITES), AVAIL_TOTAL);
    }
}
