//! Shared history vocabulary for consistency-model predicates.
//!
//! Defines a common operation shape used by transactional, object, and
//! session-level consistency modules. The intent is not to execute a full
//! history checker here, but to give later model predicates one shared event
//! vocabulary so they do not invent incompatible local encodings.

use vstd::prelude::*;

verus! {

/// Operation kind constants shared across the consistency-model modules.
pub const OP_READ: u8 = 0u8;
pub const OP_WRITE: u8 = 1u8;
pub const OP_COMMIT: u8 = 2u8;
pub const OP_ABORT: u8 = 3u8;

/// Common history event shape.
#[derive(Copy, Clone, Debug)]
pub struct HistoryOp {
    /// Stable operation identifier.
    pub op_id: u64,
    /// Event kind.
    pub kind: u8,
    /// Object touched by the event.
    pub object: u64,
    /// Transaction identifier.
    pub txn: u64,
    /// Session / client identifier.
    pub session: u64,
    /// Invocation time.
    pub invoke: u64,
    /// Response / completion time.
    pub respond: u64,
    /// For reads, the write operation observed; 0 for non-reads.
    pub observed: u64,
    /// Value carried by the operation.
    pub value: u64,
}

// r[depends consistency-vocab.valid-op-kind]
/// Spec: shared history vocabulary uses four operation kinds.
pub open spec fn is_valid_op_kind_spec(kind: u8) -> bool {
    kind <= OP_ABORT
}

// r[impl consistency-vocab.valid-op-kind]
/// Returns true when `kind` is one of the shared operation tags.
pub fn is_valid_op_kind(kind: u8) -> (result: bool)
    ensures result == is_valid_op_kind_spec(kind)
{
    kind <= OP_ABORT
}

// r[depends consistency-vocab.op-complete]
/// Spec: a history operation is complete when its tag is valid and its
/// invocation does not happen after its response.
pub open spec fn is_complete_op_spec(op: HistoryOp) -> bool {
    is_valid_op_kind_spec(op.kind) && op.invoke <= op.respond
}

// r[impl consistency-vocab.op-complete]
/// Returns true when the operation has a valid tag and non-decreasing time.
pub fn is_complete_op(op: HistoryOp) -> (result: bool)
    ensures result == is_complete_op_spec(op)
{
    is_valid_op_kind(op.kind) && op.invoke <= op.respond
}

// r[depends consistency-vocab.same-object]
/// Spec: two operations touch the same object.
pub open spec fn same_object_spec(a: HistoryOp, b: HistoryOp) -> bool {
    a.object == b.object
}

// r[impl consistency-vocab.same-object]
pub fn same_object(a: HistoryOp, b: HistoryOp) -> (result: bool)
    ensures result == same_object_spec(a, b)
{
    a.object == b.object
}

// r[depends consistency-vocab.same-transaction]
/// Spec: two operations belong to the same transaction.
pub open spec fn same_transaction_spec(a: HistoryOp, b: HistoryOp) -> bool {
    a.txn == b.txn
}

// r[impl consistency-vocab.same-transaction]
pub fn same_transaction(a: HistoryOp, b: HistoryOp) -> (result: bool)
    ensures result == same_transaction_spec(a, b)
{
    a.txn == b.txn
}

// r[depends consistency-vocab.same-session]
/// Spec: two operations belong to the same client session.
pub open spec fn same_session_spec(a: HistoryOp, b: HistoryOp) -> bool {
    a.session == b.session
}

// r[impl consistency-vocab.same-session]
pub fn same_session(a: HistoryOp, b: HistoryOp) -> (result: bool)
    ensures result == same_session_spec(a, b)
{
    a.session == b.session
}

// r[depends consistency-vocab.history-well-formed]
/// Spec: every event in the history is complete.
pub open spec fn history_well_formed_spec(history: Seq<HistoryOp>, from: int) -> bool
    decreases history.len() - from
{
    if from >= history.len() {
        true
    } else {
        is_complete_op_spec(history[from])
        && history_well_formed_spec(history, from + 1)
    }
}

proof fn history_tail_to_start(history: Seq<HistoryOp>, upto: int)
    requires
        0 <= upto <= history.len(),
        forall|j: int| #![trigger history[j]] 0 <= j < upto ==> is_complete_op_spec(history[j]),
        history_well_formed_spec(history, upto),
    ensures history_well_formed_spec(history, 0)
    decreases upto,
{
    if upto > 0 {
        history_tail_to_start(history, upto - 1);
    }
}

proof fn history_tail_implies_complete_at(history: Seq<HistoryOp>, from: int, idx: int)
    requires
        0 <= from <= idx < history.len(),
        history_well_formed_spec(history, from),
    ensures is_complete_op_spec(history[idx])
    decreases idx - from,
{
    if from < idx {
        history_tail_implies_complete_at(history, from + 1, idx);
    }
}

// r[impl consistency-vocab.history-well-formed]
/// Checks that each event in the history uses the shared vocabulary correctly.
pub fn history_well_formed(history: &[HistoryOp]) -> (result: bool)
    ensures result == history_well_formed_spec(history@, 0)
{
    let mut i: usize = 0;
    while i < history.len()
        invariant
            0 <= i <= history@.len(),
            forall|j: int| #![trigger history@[j]] 0 <= j < i as int ==> is_complete_op_spec(history@[j]),
        decreases history@.len() - i,
    {
        if !is_complete_op(history[i]) {
            proof {
                if history_well_formed_spec(history@, 0) {
                    history_tail_implies_complete_at(history@, 0, i as int);
                    assert(is_complete_op_spec(history@[i as int]));
                }
            }
            return false;
        }
        i += 1;
    }
    proof {
        assert(history_well_formed_spec(history@, i as int));
        history_tail_to_start(history@, i as int);
    }
    true
}

// r[verify consistency-vocab.valid-op-kind]
/// All shared operation constants are valid.
proof fn op_kinds_are_valid()
    ensures
        is_valid_op_kind_spec(OP_READ),
        is_valid_op_kind_spec(OP_WRITE),
        is_valid_op_kind_spec(OP_COMMIT),
        is_valid_op_kind_spec(OP_ABORT),
{}

// r[verify consistency-vocab.op-complete]
/// Complete operations always have valid tags.
proof fn complete_op_has_valid_kind(op: HistoryOp)
    requires is_complete_op_spec(op)
    ensures is_valid_op_kind_spec(op.kind)
{}

// r[verify consistency-vocab.same-object]
/// same-object is symmetric.
proof fn same_object_symmetric(a: HistoryOp, b: HistoryOp)
    ensures same_object_spec(a, b) == same_object_spec(b, a)
{}

// r[verify consistency-vocab.same-transaction]
/// same-transaction is reflexive.
proof fn same_transaction_reflexive(op: HistoryOp)
    ensures same_transaction_spec(op, op)
{}

// r[verify consistency-vocab.same-session]
/// same-session is reflexive.
proof fn same_session_reflexive(op: HistoryOp)
    ensures same_session_spec(op, op)
{}

// r[verify consistency-vocab.history-well-formed]
/// The empty history is well formed.
proof fn empty_history_well_formed()
    ensures history_well_formed_spec(Seq::<HistoryOp>::empty(), 0)
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    fn read_op(op_id: u64, object: u64, txn: u64, session: u64, invoke: u64, respond: u64, observed: u64) -> HistoryOp {
        HistoryOp {
            op_id,
            kind: OP_READ,
            object,
            txn,
            session,
            invoke,
            respond,
            observed,
            value: 0,
        }
    }

    fn write_op(op_id: u64, object: u64, txn: u64, session: u64, invoke: u64, respond: u64, value: u64) -> HistoryOp {
        HistoryOp {
            op_id,
            kind: OP_WRITE,
            object,
            txn,
            session,
            invoke,
            respond,
            observed: 0,
            value,
        }
    }

    // r[verify consistency-vocab.valid-op-kind]
    #[test]
    fn test_valid_kinds() {
        assert!(is_valid_op_kind(OP_READ));
        assert!(is_valid_op_kind(OP_WRITE));
        assert!(is_valid_op_kind(OP_COMMIT));
        assert!(is_valid_op_kind(OP_ABORT));
        assert!(!is_valid_op_kind(4));
    }

    // r[verify consistency-vocab.op-complete]
    #[test]
    fn test_complete_op() {
        assert!(is_complete_op(read_op(1, 10, 20, 30, 4, 9, 0)));
        assert!(!is_complete_op(write_op(2, 10, 20, 30, 9, 4, 7)));
    }

    // r[verify consistency-vocab.same-object]
    #[test]
    fn test_same_object() {
        let a = read_op(1, 7, 1, 1, 0, 1, 0);
        let b = write_op(2, 7, 2, 3, 2, 3, 9);
        let c = write_op(3, 9, 2, 3, 2, 3, 9);
        assert!(same_object(a, b));
        assert!(!same_object(a, c));
    }

    // r[verify consistency-vocab.same-transaction]
    #[test]
    fn test_same_transaction() {
        let a = read_op(1, 7, 11, 1, 0, 1, 0);
        let b = write_op(2, 8, 11, 2, 2, 3, 9);
        let c = write_op(3, 8, 12, 2, 2, 3, 9);
        assert!(same_transaction(a, b));
        assert!(!same_transaction(a, c));
    }

    // r[verify consistency-vocab.same-session]
    #[test]
    fn test_same_session() {
        let a = read_op(1, 7, 11, 5, 0, 1, 0);
        let b = write_op(2, 8, 12, 5, 2, 3, 9);
        let c = write_op(3, 8, 12, 6, 2, 3, 9);
        assert!(same_session(a, b));
        assert!(!same_session(a, c));
    }

    // r[verify consistency-vocab.history-well-formed]
    #[test]
    fn test_history_well_formed() {
        let good = [read_op(1, 7, 11, 5, 0, 1, 3), write_op(2, 7, 11, 5, 2, 3, 9)];
        let bad = [read_op(1, 7, 11, 5, 0, 1, 3), write_op(2, 7, 11, 5, 4, 3, 9)];
        assert!(history_well_formed(&good));
        assert!(!history_well_formed(&bad));
    }
}
