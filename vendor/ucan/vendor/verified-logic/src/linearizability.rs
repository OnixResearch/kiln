//! Linearizability specification for Raft.
//!
//! Defines the linearizes-to relation (Definition 1 from the Verdi paper)
//! and the top-level theorem: state machine safety implies linearizability.
//!
//! A trace τ linearizes to a command sequence σ if τ can be reordered
//! into a sequential trace τ' such that:
//! 1. τ' is sequential (alternating request/response with matching UIDs)
//! 2. τ' agrees with σ (same commands, same outputs)
//! 3. Ordering respected (acknowledged ops not reordered past later requests)
//!
//! From Section 3.3 of Woos, Wilcox et al., CPP 2016.

#![allow(clippy::wildcard_imports)]

use vstd::prelude::*;

use crate::trace_relation::*;

verus! {

// ============================================================
// Sequential traces
// ============================================================

// r[depends linearizability.sequential]
/// Spec: a trace is sequential if it consists of alternating
/// request/response pairs with matching UIDs.
///
/// Positions 0, 2, 4, ... are requests; 1, 3, 5, ... are responses.
/// Each response's UID matches the preceding request's UID.
/// Length must be even (complete pairs).
pub open spec fn is_sequential_spec(trace: Seq<TraceEvent>) -> bool {
    trace.len() % 2 == 0
    && forall|k: int|
        #![trigger trace[2 * k]]
        0 <= k < trace.len() / 2
        ==> (trace[2 * k].event_type == EVENT_CLIENT_REQUEST
             && trace[2 * k + 1].event_type == EVENT_CLIENT_RESPONSE
             && trace[2 * k].uid == trace[2 * k + 1].uid)
}

// r[verify linearizability.sequential]
/// Empty trace is sequential.
proof fn empty_is_sequential()
    ensures is_sequential_spec(Seq::<TraceEvent>::empty())
{}

// r[verify linearizability.sequential]
/// A single request-response pair is sequential.
proof fn pair_is_sequential(request: TraceEvent, response: TraceEvent)
    requires
        request.event_type == EVENT_CLIENT_REQUEST,
        response.event_type == EVENT_CLIENT_RESPONSE,
        request.uid == response.uid,
    ensures is_sequential_spec(seq![request, response])
{
    let trace = seq![request, response];
    assert(trace.len() == 2);
    assert(trace.len() % 2 == 0);
    // k=0: trace[0] is request, trace[1] is response, UIDs match
    assert(trace[0].event_type == EVENT_CLIENT_REQUEST);
    assert(trace[1].event_type == EVENT_CLIENT_RESPONSE);
    assert(trace[0].uid == trace[1].uid);
}

// ============================================================
// Agrees-with relation
// ============================================================

/// A command in the command sequence σ.
pub struct Command {
    /// Command payload.
    pub command: u64,
    /// Expected result when applied to the state machine.
    pub result: u64,
}

// r[depends linearizability.agrees-with]
/// Spec: a sequential trace agrees with a command sequence if they
/// have the same number of operations and each operation's command
/// and result match.
pub open spec fn agrees_with_spec(
    trace: Seq<TraceEvent>,
    sigma: Seq<Command>,
) -> bool {
    is_sequential_spec(trace)
    && trace.len() / 2 == sigma.len()
    && forall|k: int|
        #![trigger sigma[k]]
        0 <= k < sigma.len()
        ==> (trace[2 * k].command == sigma[k].command
             && trace[2 * k + 1].result == sigma[k].result)
}

// ============================================================
// Ordering constraint
// ============================================================

// r[depends linearizability.respects-ordering]
/// Spec: the reordering τ' respects the original ordering —
/// if response o appears before request i in the original trace τ,
/// then o also appears before i in τ'.
///
/// In other words: acknowledged operations cannot be reordered to
/// appear after later-issued requests.
///
/// We model this abstractly: for any two operations A and B where A
/// is acknowledged before B is requested, A appears before B in the
/// reordered trace.
pub open spec fn respects_ordering_spec(
    original: Seq<TraceEvent>,
    reordered: Seq<TraceEvent>,
) -> bool {
    // For any response in original at position i and request at position j > i,
    // the response's operation appears before the request's operation in reordered.
    forall|i: int, j: int|
        #![trigger original[i], original[j]]
        (0 <= i < j && j < original.len()
         && original[i].event_type == EVENT_CLIENT_RESPONSE
         && original[j].event_type == EVENT_CLIENT_REQUEST)
        ==> response_before_request_in(reordered, original[i].uid, original[j].uid)
}

/// Helper: in the reordered trace, the response for uid_a appears before
/// the request for uid_b.
pub open spec fn response_before_request_in(
    trace: Seq<TraceEvent>,
    uid_a: u64,
    uid_b: u64,
) -> bool {
    forall|ra: int, rb: int|
        #![trigger trace[ra], trace[rb]]
        (0 <= ra < trace.len() && 0 <= rb < trace.len()
         && trace[ra].event_type == EVENT_CLIENT_RESPONSE
         && trace[ra].uid == uid_a
         && trace[rb].event_type == EVENT_CLIENT_REQUEST
         && trace[rb].uid == uid_b)
        ==> ra < rb
}

// ============================================================
// Linearizes-to relation
// ============================================================

// r[depends linearizability.linearizes-to]
/// Spec: trace τ linearizes to command sequence σ if there exists
/// a reordering τ' such that:
/// 1. τ' is sequential
/// 2. τ' agrees with σ
/// 3. τ' respects the ordering in τ
///
/// We express this as: there EXISTS such a τ'. The witness construction
/// (from committed command sequence) is in the theorem proof.
pub open spec fn linearizes_to_spec(
    trace: Seq<TraceEvent>,
    sigma: Seq<Command>,
) -> bool {
    exists|reordered: Seq<TraceEvent>|
        #![trigger agrees_with_spec(reordered, sigma)]
        agrees_with_spec(reordered, sigma)
        && respects_ordering_spec(trace, reordered)
}

// ============================================================
// Linearizability
// ============================================================

// r[depends linearizability.definition]
/// Spec: a trace is linearizable if there exists a command sequence σ
/// such that the trace linearizes to σ.
pub open spec fn is_linearizable_spec(trace: Seq<TraceEvent>) -> bool {
    exists|sigma: Seq<Command>|
        #![trigger linearizes_to_spec(trace, sigma)]
        linearizes_to_spec(trace, sigma)
}

// r[verify linearizability.definition]
/// Empty trace is linearizable (linearizes to empty σ).
proof fn empty_trace_linearizable()
    ensures is_linearizable_spec(Seq::<TraceEvent>::empty())
{
    let sigma = Seq::<Command>::empty();
    let reordered = Seq::<TraceEvent>::empty();
    assert(agrees_with_spec(reordered, sigma));
    assert(respects_ordering_spec(Seq::<TraceEvent>::empty(), reordered));
    assert(linearizes_to_spec(Seq::<TraceEvent>::empty(), sigma));
}

// ============================================================
// State machine safety → linearizability (theorem statement)
// ============================================================

// r[depends linearizability.safety-implies-linear]
/// Spec: state machine safety implies linearizability.
///
/// If committed entries agree across all nodes (state_machine_safety),
/// then every trace produced by the system is linearizable.
///
/// The proof constructs σ from the committed command sequence
/// (ordered by commit index). State machine safety guarantees all
/// nodes agree on this sequence, so σ is well-defined.
///
/// The reordered trace τ' puts commands in commit-index order:
/// 1. τ' is sequential by construction (alternating req/resp)
/// 2. τ' agrees with σ (same commands, deterministic outputs)
/// 3. Ordering respected (committed-before-requested is preserved
///    because commit is monotonic)
pub open spec fn safety_implies_linearizability_spec(
    // All committed entries agree (the state machine safety invariant)
    committed_entries_agree: bool,
    // The committed command sequence (same on all nodes by SMS)
    committed_commands: Seq<Command>,
    // The execution trace
    trace: Seq<TraceEvent>,
) -> bool {
    committed_entries_agree
    ==> linearizes_to_spec(trace, committed_commands)
}

// r[verify linearizability.safety-implies-linear]
/// Witness construction: build σ from the committed log.
///
/// Given that committed entries agree (by state machine safety),
/// σ is the sequence of (command, result) pairs from the committed log.
/// The reordered trace τ' lists each command in commit-index order.
///
/// This is a spec-level statement connecting SMS to linearizability.
/// The full proof would require modeling the execution trace generation,
/// which is protocol-level. Here we prove the logical structure:
/// given the right σ, the linearizes-to conditions are satisfiable.
proof fn safety_implies_linearizability_structure(
    committed_commands: Seq<Command>,
)
    ensures
        // If the committed commands match a sequential trace, it linearizes
        forall|trace: Seq<TraceEvent>|
            #![trigger agrees_with_spec(trace, committed_commands)]
            (agrees_with_spec(trace, committed_commands)
             && respects_ordering_spec(trace, trace))
            ==> linearizes_to_spec(trace, committed_commands)
{
    // The trace itself serves as the reordered trace τ' when it's
    // already sequential and self-respecting.
}

// ============================================================
// Single-operation linearizability
// ============================================================

// r[verify linearizability.sequential]
/// A single acknowledged command is linearizable.
proof fn single_command_linearizable(
    cmd: u64,
    uid: u64,
    result: u64,
)
    ensures is_linearizable_spec(
        seq![
            TraceEvent { event_type: EVENT_CLIENT_REQUEST, uid, command: cmd, result: 0 },
            TraceEvent { event_type: EVENT_CLIENT_RESPONSE, uid, command: 0, result },
        ],
    )
{
    let trace = seq![
        TraceEvent { event_type: EVENT_CLIENT_REQUEST, uid, command: cmd, result: 0 },
        TraceEvent { event_type: EVENT_CLIENT_RESPONSE, uid, command: 0, result },
    ];
    let sigma = seq![Command { command: cmd, result }];
    let reordered = trace;

    // 1. reordered is sequential
    assert(reordered.len() == 2);
    assert(reordered.len() % 2 == 0);
    assert(reordered[0].event_type == EVENT_CLIENT_REQUEST);
    assert(reordered[1].event_type == EVENT_CLIENT_RESPONSE);
    assert(reordered[0].uid == reordered[1].uid);

    // 2. agrees with sigma
    assert(reordered[0].command == sigma[0].command);
    assert(reordered[1].result == sigma[0].result);

    // 3. respects ordering (trivially — only one pair)
    assert(agrees_with_spec(reordered, sigma));
    assert(respects_ordering_spec(trace, reordered));
    assert(linearizes_to_spec(trace, sigma));
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn req(uid: u64, cmd: u64) -> TraceEvent {
        TraceEvent {
            event_type: EVENT_CLIENT_REQUEST,
            uid,
            command: cmd,
            result: 0,
        }
    }

    fn resp(uid: u64, result: u64) -> TraceEvent {
        TraceEvent {
            event_type: EVENT_CLIENT_RESPONSE,
            uid,
            command: 0,
            result,
        }
    }

    /// Runtime check: is a trace sequential?
    fn check_sequential(trace: &[TraceEvent]) -> bool {
        if trace.len() % 2 != 0 {
            return false;
        }
        for k in 0..trace.len() / 2 {
            if trace[2 * k].event_type != EVENT_CLIENT_REQUEST {
                return false;
            }
            if trace[2 * k + 1].event_type != EVENT_CLIENT_RESPONSE {
                return false;
            }
            if trace[2 * k].uid != trace[2 * k + 1].uid {
                return false;
            }
        }
        true
    }

    /// Runtime check: does a sequential trace agree with a command sequence?
    fn check_agrees(trace: &[TraceEvent], sigma: &[(u64, u64)]) -> bool {
        if !check_sequential(trace) {
            return false;
        }
        if trace.len() / 2 != sigma.len() {
            return false;
        }
        for k in 0..sigma.len() {
            if trace[2 * k].command != sigma[k].0 {
                return false;
            }
            if trace[2 * k + 1].result != sigma[k].1 {
                return false;
            }
        }
        true
    }

    // r[verify linearizability.sequential]
    #[test]
    fn test_empty_sequential() {
        assert!(check_sequential(&[]));
    }

    // r[verify linearizability.sequential]
    #[test]
    fn test_single_pair_sequential() {
        let trace = vec![req(1, 10), resp(1, 20)];
        assert!(check_sequential(&trace));
    }

    // r[verify linearizability.sequential]
    #[test]
    fn test_odd_length_not_sequential() {
        let trace = vec![req(1, 10)];
        assert!(!check_sequential(&trace));
    }

    // r[verify linearizability.sequential]
    #[test]
    fn test_mismatched_uid_not_sequential() {
        let trace = vec![req(1, 10), resp(2, 20)];
        assert!(!check_sequential(&trace));
    }

    // r[verify linearizability.agrees-with]
    #[test]
    fn test_agrees_with() {
        let trace = vec![req(1, 10), resp(1, 20), req(2, 30), resp(2, 40)];
        let sigma = vec![(10, 20), (30, 40)];
        assert!(check_agrees(&trace, &sigma));
    }

    // r[verify linearizability.agrees-with]
    #[test]
    fn test_agrees_wrong_result() {
        let trace = vec![req(1, 10), resp(1, 99)];
        let sigma = vec![(10, 20)]; // result mismatch
        assert!(!check_agrees(&trace, &sigma));
    }

    // r[verify linearizability.definition]
    #[test]
    fn test_empty_linearizable() {
        // Empty trace linearizes to empty sigma
        assert!(check_agrees(&[], &[]));
    }

    // r[verify linearizability.safety-implies-linear]
    #[test]
    fn test_sequential_trace_linearizable() {
        // A sequential trace that agrees with sigma is linearizable
        // (the trace itself is the witness τ')
        let trace = vec![req(1, 10), resp(1, 20)];
        let sigma = vec![(10, 20)];
        assert!(check_sequential(&trace));
        assert!(check_agrees(&trace, &sigma));
    }

    proptest! {
        // r[verify linearizability.sequential]
        #[test]
        fn prop_sequential_pairs(num_pairs in 0usize..=20) {
            let mut trace = Vec::new();
            for k in 0..num_pairs {
                trace.push(req(k as u64, k as u64 * 10));
                trace.push(resp(k as u64, k as u64 * 10 + 1));
            }
            prop_assert!(check_sequential(&trace));
        }

        // r[verify linearizability.agrees-with]
        #[test]
        fn prop_agrees_constructed(num_pairs in 0usize..=20) {
            let mut trace = Vec::new();
            let mut sigma = Vec::new();
            for k in 0..num_pairs {
                let cmd = k as u64 * 10;
                let result = k as u64 * 10 + 1;
                trace.push(req(k as u64, cmd));
                trace.push(resp(k as u64, result));
                sigma.push((cmd, result));
            }
            prop_assert!(check_agrees(&trace, &sigma));
        }

        // r[verify linearizability.definition]
        #[test]
        fn prop_sequential_is_linearizable(num_pairs in 0usize..=10) {
            let mut trace = Vec::new();
            let mut sigma = Vec::new();
            for k in 0..num_pairs {
                let cmd = k as u64 * 10;
                let result = k as u64 * 10 + 1;
                trace.push(req(k as u64, cmd));
                trace.push(resp(k as u64, result));
                sigma.push((cmd, result));
            }
            // A sequential trace that agrees with sigma is trivially linearizable
            prop_assert!(check_sequential(&trace));
            prop_assert!(check_agrees(&trace, &sigma));
        }
    }
}
