//! Shared order and visibility relations for consistency-model predicates.
//!
//! These relations are small reusable building blocks over `HistoryOp`.
//! Higher-level transactional, object, and session predicates compose them
//! instead of reintroducing private local encodings.

use vstd::prelude::*;

use crate::consistency_history::HistoryOp;
use crate::consistency_history::OP_READ;
use crate::consistency_history::OP_WRITE;
#[cfg(verus_keep_ghost)]
use crate::consistency_history::is_complete_op_spec;
#[cfg(verus_keep_ghost)]
use crate::consistency_history::same_object_spec;
#[cfg(verus_keep_ghost)]
use crate::consistency_history::same_session_spec;
#[cfg(verus_keep_ghost)]
use crate::consistency_history::same_transaction_spec;

verus! {

// r[depends consistency-vocab.realtime-order]
/// Spec: operation `a` is before `b` in real time when `a` completes before
/// `b` is invoked.
pub open spec fn realtime_order_spec(a: HistoryOp, b: HistoryOp) -> bool {
    a.respond < b.invoke
}

// r[impl consistency-vocab.realtime-order]
pub fn realtime_order(a: HistoryOp, b: HistoryOp) -> (result: bool)
    ensures result == realtime_order_spec(a, b)
{
    a.respond < b.invoke
}

// r[depends consistency-vocab.session-order]
/// Spec: session order is the per-client order induced by completion before a
/// later invocation in the same session.
pub open spec fn session_order_spec(a: HistoryOp, b: HistoryOp) -> bool {
    same_session_spec(a, b) && a.op_id != b.op_id && a.respond <= b.invoke
}

// r[impl consistency-vocab.session-order]
pub fn session_order(a: HistoryOp, b: HistoryOp) -> (result: bool)
    ensures result == session_order_spec(a, b)
{
    crate::consistency_history::same_session(a, b) && a.op_id != b.op_id && a.respond <= b.invoke
}

// r[depends consistency-vocab.visibility]
/// Spec: a write is visible to a read when the read names that write as the
/// observed source on the same object.
pub open spec fn visibility_spec(write: HistoryOp, read: HistoryOp) -> bool {
    write.kind == OP_WRITE
    && read.kind == OP_READ
    && same_object_spec(write, read)
    && read.observed == write.op_id
    && write.respond <= read.respond
}

// r[impl consistency-vocab.visibility]
pub fn visibility(write: HistoryOp, read: HistoryOp) -> (result: bool)
    ensures result == visibility_spec(write, read)
{
    write.kind == OP_WRITE
        && read.kind == OP_READ
        && crate::consistency_history::same_object(write, read)
        && read.observed == write.op_id
        && write.respond <= read.respond
}

// r[depends consistency-vocab.arbitration]
/// Spec: arbitration gives a deterministic order between two writes to the
/// same object.
pub open spec fn arbitration_spec(left: HistoryOp, right: HistoryOp) -> bool {
    left.kind == OP_WRITE
    && right.kind == OP_WRITE
    && same_object_spec(left, right)
    && left.op_id != right.op_id
    && (left.respond < right.respond
        || (left.respond == right.respond && left.op_id < right.op_id))
}

// r[impl consistency-vocab.arbitration]
pub fn arbitration(left: HistoryOp, right: HistoryOp) -> (result: bool)
    ensures result == arbitration_spec(left, right)
{
    left.kind == OP_WRITE
        && right.kind == OP_WRITE
        && crate::consistency_history::same_object(left, right)
        && left.op_id != right.op_id
        && (left.respond < right.respond
            || (left.respond == right.respond && left.op_id < right.op_id))
}

// r[depends consistency-vocab.write-conflict]
/// Spec: two writes conflict when they touch the same object in different
/// transactions.
pub open spec fn write_conflict_spec(left: HistoryOp, right: HistoryOp) -> bool {
    left.kind == OP_WRITE
    && right.kind == OP_WRITE
    && same_object_spec(left, right)
    && !same_transaction_spec(left, right)
}

// r[impl consistency-vocab.write-conflict]
pub fn write_conflict(left: HistoryOp, right: HistoryOp) -> (result: bool)
    ensures result == write_conflict_spec(left, right)
{
    left.kind == OP_WRITE
        && right.kind == OP_WRITE
        && crate::consistency_history::same_object(left, right)
        && !crate::consistency_history::same_transaction(left, right)
}

// r[depends consistency-vocab.happens-before]
/// Spec: the shared happens-before edge relation is the union of session
/// order, real-time order, and read visibility.
pub open spec fn happens_before_edge_spec(a: HistoryOp, b: HistoryOp) -> bool {
    realtime_order_spec(a, b) || session_order_spec(a, b) || visibility_spec(a, b)
}

// r[impl consistency-vocab.happens-before]
pub fn happens_before_edge(a: HistoryOp, b: HistoryOp) -> (result: bool)
    ensures result == happens_before_edge_spec(a, b)
{
    realtime_order(a, b) || session_order(a, b) || visibility(a, b)
}

// r[verify consistency-vocab.realtime-order]
/// A complete operation cannot be before itself in real time.
proof fn realtime_irreflexive_for_complete(op: HistoryOp)
    requires is_complete_op_spec(op)
    ensures !realtime_order_spec(op, op)
{}

// r[verify consistency-vocab.session-order]
/// Session order only relates operations in the same session.
proof fn session_order_implies_same_session(a: HistoryOp, b: HistoryOp)
    requires session_order_spec(a, b)
    ensures same_session_spec(a, b)
{}

// r[verify consistency-vocab.visibility]
/// Visibility implies the same object.
proof fn visibility_implies_same_object(write: HistoryOp, read: HistoryOp)
    requires visibility_spec(write, read)
    ensures same_object_spec(write, read)
{}

// r[verify consistency-vocab.write-conflict]
/// Write conflict is symmetric.
proof fn write_conflict_symmetric(a: HistoryOp, b: HistoryOp)
    ensures write_conflict_spec(a, b) == write_conflict_spec(b, a)
{}

// r[verify consistency-vocab.happens-before]
/// The shared edge relation is irreflexive on complete operations.
proof fn happens_before_irreflexive(op: HistoryOp)
    requires is_complete_op_spec(op)
    ensures !happens_before_edge_spec(op, op)
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consistency_history::HistoryOp;
    use crate::consistency_history::OP_READ;
    use crate::consistency_history::OP_WRITE;

    fn op(
        op_id: u64,
        kind: u8,
        object: u64,
        txn: u64,
        session: u64,
        invoke: u64,
        respond: u64,
        observed: u64,
    ) -> HistoryOp {
        HistoryOp {
            op_id,
            kind,
            object,
            txn,
            session,
            invoke,
            respond,
            observed,
            value: 0,
        }
    }

    // r[verify consistency-vocab.realtime-order]
    #[test]
    fn test_realtime_order() {
        let a = op(1, OP_WRITE, 7, 1, 1, 0, 2, 0);
        let b = op(2, OP_READ, 7, 2, 2, 3, 4, 1);
        assert!(realtime_order(a, b));
        assert!(!realtime_order(b, a));
    }

    // r[verify consistency-vocab.session-order]
    #[test]
    fn test_session_order() {
        let a = op(1, OP_WRITE, 7, 1, 9, 0, 2, 0);
        let b = op(2, OP_READ, 8, 2, 9, 2, 4, 0);
        let c = op(3, OP_READ, 8, 2, 10, 2, 4, 0);
        assert!(session_order(a, b));
        assert!(!session_order(a, c));
    }

    // r[verify consistency-vocab.visibility]
    #[test]
    fn test_visibility() {
        let w = op(11, OP_WRITE, 7, 1, 1, 0, 2, 0);
        let r = op(12, OP_READ, 7, 2, 2, 3, 4, 11);
        let r_other = op(13, OP_READ, 8, 2, 2, 3, 4, 11);
        assert!(visibility(w, r));
        assert!(!visibility(w, r_other));
    }

    // r[verify consistency-vocab.arbitration]
    #[test]
    fn test_arbitration() {
        let a = op(1, OP_WRITE, 7, 1, 1, 0, 3, 0);
        let b = op(2, OP_WRITE, 7, 2, 2, 0, 4, 0);
        let c = op(3, OP_WRITE, 7, 2, 2, 0, 4, 0);
        assert!(arbitration(a, b));
        assert!(arbitration(b, c));
        assert!(!arbitration(c, b));
    }

    // r[verify consistency-vocab.write-conflict]
    #[test]
    fn test_write_conflict() {
        let a = op(1, OP_WRITE, 7, 1, 1, 0, 2, 0);
        let b = op(2, OP_WRITE, 7, 2, 2, 2, 4, 0);
        let c = op(3, OP_WRITE, 7, 1, 2, 2, 4, 0);
        assert!(write_conflict(a, b));
        assert!(!write_conflict(a, c));
    }

    // r[verify consistency-vocab.happens-before]
    #[test]
    fn test_happens_before_edge() {
        let w = op(11, OP_WRITE, 7, 1, 9, 0, 2, 0);
        let r = op(12, OP_READ, 7, 2, 10, 3, 4, 11);
        assert!(happens_before_edge(w, r));
        assert!(!happens_before_edge(r, w));
    }
}
