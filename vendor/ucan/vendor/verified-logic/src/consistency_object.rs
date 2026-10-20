//! Object-level consistency predicates over the shared history vocabulary.
//!
//! Uses `HistoryOp` plus the shared real-time and session-order relations,
//! so the object-model layer matches the same operation metadata used by the
//! transactional and session-model modules.

use vstd::prelude::*;

use crate::consistency_history::HistoryOp;
use crate::consistency_history::OP_READ;
use crate::consistency_history::OP_WRITE;
#[cfg(verus_keep_ghost)]
use crate::consistency_history::is_complete_op_spec;
#[cfg(verus_keep_ghost)]
use crate::consistency_relations::realtime_order_spec;
#[cfg(verus_keep_ghost)]
use crate::consistency_relations::session_order_spec;

verus! {

spec fn is_object_op_spec(op: HistoryOp) -> bool {
    op.kind == OP_READ || op.kind == OP_WRITE
}

fn is_object_op(op: HistoryOp) -> (result: bool)
    ensures result == is_object_op_spec(op)
{
    op.kind == OP_READ || op.kind == OP_WRITE
}

// r[depends consistency-object.sequential-witness]
/// Spec: a sequential witness is a well-formed total order of complete
/// read/write operations expressed in the shared history vocabulary.
pub open spec fn sequential_witness_from_spec(history: Seq<HistoryOp>, from: int) -> bool
    decreases history.len() - from
{
    if from >= history.len() {
        true
    } else {
        is_complete_op_spec(history[from])
        && (history[from].kind == OP_READ || history[from].kind == OP_WRITE)
        && sequential_witness_from_spec(history, from + 1)
    }
}

// r[depends consistency-object.sequential-witness]
pub open spec fn sequential_witness_spec(history: Seq<HistoryOp>) -> bool {
    sequential_witness_from_spec(history, 0)
}

fn sequential_witness_from(history: &[HistoryOp], from: usize) -> (result: bool)
    requires from <= history.len()
    ensures result == sequential_witness_from_spec(history@, from as int)
    decreases history@.len() - from as int
{
    if from >= history.len() {
        true
    } else {
        crate::consistency_history::is_complete_op(history[from])
            && is_object_op(history[from])
            && sequential_witness_from(history, from + 1)
    }
}

// r[impl consistency-object.sequential-witness]
/// Runtime checker for the shared object-operation witness shape.
pub fn sequential_witness(history: &[HistoryOp]) -> (result: bool)
    ensures result == sequential_witness_spec(history@)
{
    sequential_witness_from(history, 0)
}

pub open spec fn program_order_respected_from_spec(history: Seq<HistoryOp>, i: int, j: int) -> bool
    decreases history.len() - i, history.len() - j
{
    if i + 1 >= history.len() {
        true
    } else if j >= history.len() {
        program_order_respected_from_spec(history, i + 1, i + 2)
    } else {
        !session_order_spec(history[j], history[i])
        && program_order_respected_from_spec(history, i, j + 1)
    }
}

pub open spec fn program_order_respected_spec(history: Seq<HistoryOp>) -> bool {
    if history.len() <= 1 {
        true
    } else {
        program_order_respected_from_spec(history, 0, 1)
    }
}

fn program_order_respected_from(history: &[HistoryOp], i: usize, j: usize) -> (result: bool)
    requires
        history.len() <= 1 || (i < history.len() && i < j && j <= history.len()),
    ensures result == program_order_respected_from_spec(history@, i as int, j as int)
    decreases history@.len() - i as int, history@.len() - j as int
{
    if history.len() <= 1 || i >= history.len() - 1 {
        true
    } else if j >= history.len() {
        program_order_respected_from(history, i + 1, i + 2)
    } else {
        !crate::consistency_relations::session_order(history[j], history[i])
            && program_order_respected_from(history, i, j + 1)
    }
}

fn program_order_respected(history: &[HistoryOp]) -> (result: bool)
    ensures result == program_order_respected_spec(history@)
{
    if history.len() <= 1 {
        true
    } else {
        program_order_respected_from(history, 0, 1)
    }
}

// r[depends consistency-object.sequential-consistency]
/// Spec: sequential consistency requires a shared sequential witness and
/// preserved session/program order in that witness order.
pub open spec fn sequential_consistency_spec(history: Seq<HistoryOp>) -> bool {
    sequential_witness_spec(history) && program_order_respected_spec(history)
}

// r[impl consistency-object.sequential-consistency]
pub fn sequential_consistency(history: &[HistoryOp]) -> (result: bool)
    ensures result == sequential_consistency_spec(history@)
{
    sequential_witness(history) && program_order_respected(history)
}

pub open spec fn realtime_order_respected_from_spec(history: Seq<HistoryOp>, i: int, j: int) -> bool
    decreases history.len() - i, history.len() - j
{
    if i + 1 >= history.len() {
        true
    } else if j >= history.len() {
        realtime_order_respected_from_spec(history, i + 1, i + 2)
    } else {
        !realtime_order_spec(history[j], history[i])
        && realtime_order_respected_from_spec(history, i, j + 1)
    }
}

pub open spec fn realtime_order_respected_spec(history: Seq<HistoryOp>) -> bool {
    if history.len() <= 1 {
        true
    } else {
        realtime_order_respected_from_spec(history, 0, 1)
    }
}

fn realtime_order_respected_from(history: &[HistoryOp], i: usize, j: usize) -> (result: bool)
    requires
        history.len() <= 1 || (i < history.len() && i < j && j <= history.len()),
    ensures result == realtime_order_respected_from_spec(history@, i as int, j as int)
    decreases history@.len() - i as int, history@.len() - j as int
{
    if history.len() <= 1 || i >= history.len() - 1 {
        true
    } else if j >= history.len() {
        realtime_order_respected_from(history, i + 1, i + 2)
    } else {
        !crate::consistency_relations::realtime_order(history[j], history[i])
            && realtime_order_respected_from(history, i, j + 1)
    }
}

fn realtime_order_respected(history: &[HistoryOp]) -> (result: bool)
    ensures result == realtime_order_respected_spec(history@)
{
    if history.len() <= 1 {
        true
    } else {
        realtime_order_respected_from(history, 0, 1)
    }
}

// r[depends consistency-object.linearizability]
/// Spec: linearizability is sequential consistency plus preserved real-time
/// order in the shared witness order.
pub open spec fn linearizability_spec(history: Seq<HistoryOp>) -> bool {
    sequential_consistency_spec(history) && realtime_order_respected_spec(history)
}

// r[impl consistency-object.linearizability]
pub fn linearizability(history: &[HistoryOp]) -> (result: bool)
    ensures result == linearizability_spec(history@)
{
    sequential_consistency(history) && realtime_order_respected(history)
}

/// Shared refinement adapter: abstract sequential-consistency flag bundle.
pub open spec fn sequential_consistency_flags_spec(
    sequential_witness: bool,
    program_order_respected: bool,
) -> bool {
    sequential_witness && program_order_respected
}

/// Shared refinement adapter: abstract linearizability flag bundle.
pub open spec fn linearizability_flags_spec(
    sequential_witness: bool,
    program_order_respected: bool,
    realtime_respected: bool,
) -> bool {
    sequential_consistency_flags_spec(sequential_witness, program_order_respected)
    && realtime_respected
}

// r[verify consistency-object.linearizability]
/// Linearizability implies sequential consistency.
proof fn linearizability_implies_sequential_consistency(history: Seq<HistoryOp>)
    ensures linearizability_spec(history) ==> sequential_consistency_spec(history)
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

    // r[verify consistency-object.sequential-witness]
    #[test]
    fn test_sequential_witness_accepts_shared_history() {
        let history = [write(1, 10, 0, 1, 11), read(2, 11, 2, 3, 1)];
        assert!(sequential_witness(&history));
    }

    // r[verify consistency-object.sequential-witness]
    #[test]
    fn test_sequential_witness_rejects_non_object_op() {
        let mut bad = read(1, 10, 0, 1, 0);
        bad.kind = crate::consistency_history::OP_COMMIT;
        assert!(!sequential_witness(&[bad]));
    }

    // r[verify consistency-object.sequential-consistency]
    #[test]
    fn test_sequential_consistency_accepts_program_ordered_history() {
        let history = [write(1, 5, 0, 1, 10), read(2, 5, 2, 3, 1)];
        assert!(sequential_consistency(&history));
    }

    // r[verify consistency-object.sequential-consistency]
    #[test]
    fn test_sequential_consistency_rejects_program_order_inversion() {
        let earlier = write(1, 5, 0, 1, 10);
        let later = read(2, 5, 2, 3, 1);
        let inverted = [later, earlier];
        assert!(!sequential_consistency(&inverted));
    }

    // r[verify consistency-object.linearizability]
    #[test]
    fn test_linearizability_accepts_realtime_ordered_history() {
        let history = [write(1, 5, 0, 1, 10), read(2, 6, 2, 3, 1)];
        assert!(linearizability(&history));
    }

    // r[verify consistency-object.linearizability]
    #[test]
    fn test_linearizability_rejects_realtime_inversion_but_sc_accepts_it() {
        let earlier = write(1, 5, 0, 1, 10);
        let later = read(2, 6, 2, 3, 1);
        let reordered = [later, earlier];
        assert!(sequential_consistency(&reordered));
        assert!(!linearizability(&reordered));
    }
}
