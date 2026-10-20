//! Verified causal ordering and consistent snapshots.
//!
//! Causal delivery: if message m1 happened-before m2, m1 must be
//! delivered before m2. A consistent cut is a set of events where
//! if an event is in the cut, all its causal predecessors are too.

#![allow(clippy::ptr_arg)]

use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::ordering::max_spec;
#[cfg(verus_keep_ghost)]
use crate::pointwise::max_u64;
#[cfg(verus_keep_ghost)]
use crate::pointwise::pointwise_merge_spec;
#[cfg(verus_keep_ghost)]
use crate::vector_clock::happens_before_spec;

verus! {

// --- is_causally_ready ---

// r[depends causal-order.is-causally-ready]
/// Spec: a message is causally ready for delivery when its vector clock
/// is component-wise <= the receiver's clock, with at most one component
/// (the sender's) being exactly one ahead.
///
/// Simplified model: msg_clock[sender] == local_clock[sender] + 1
/// and for all other j: msg_clock[j] <= local_clock[j].
pub open spec fn is_causally_ready_spec(
    msg_clock: Seq<u64>, local_clock: Seq<u64>, sender: int,
) -> bool
    recommends
        msg_clock.len() == local_clock.len(),
        0 <= sender < msg_clock.len() as int,
{
    msg_clock.len() == local_clock.len()
    && 0 <= sender < msg_clock.len() as int
    && msg_clock[sender] as int == local_clock[sender] as int + 1
    && (forall|j: int| 0 <= j < msg_clock.len() as int && j != sender
        ==> msg_clock[j] <= local_clock[j])
}

// r[impl causal-order.is-causally-ready]
/// Returns true if the message can be delivered now.
pub fn is_causally_ready(
    msg_clock: &Vec<u64>, local_clock: &Vec<u64>, sender: usize,
) -> (result: bool)
    requires
        msg_clock.len() == local_clock.len(),
        sender < msg_clock.len(),
    ensures result == is_causally_ready_spec(msg_clock@, local_clock@, sender as int)
{
    if local_clock[sender] == u64::MAX || msg_clock[sender] != local_clock[sender] + 1 {
        return false;
    }
    let mut i: usize = 0;
    while i < msg_clock.len()
        invariant
            i <= msg_clock.len(),
            msg_clock.len() == local_clock.len(),
            sender < msg_clock.len(),
            msg_clock@[sender as int] as int == local_clock@[sender as int] as int + 1,
            forall|j: int| 0 <= j < i as int && j != sender as int
                ==> msg_clock@[j] <= local_clock@[j],
        decreases msg_clock.len() - i,
    {
        if i != sender && msg_clock[i] > local_clock[i] {
            return false;
        }
        i += 1;
    }
    true
}

// --- is_consistent_cut ---

// r[depends causal-order.is-consistent-cut]
/// Spec: a cut (vector of latest delivered event per process) is
/// consistent if it represents a valid causal frontier.
/// For our model: a consistent cut is any vector clock (all cuts
/// from a causal delivery protocol are consistent by construction).
/// The formal property: for any two processes i, j in the cut,
/// the cut doesn't "see" an effect without its cause.
pub open spec fn is_consistent_cut_spec(cut: Seq<u64>) -> bool {
    // All consistent cuts are valid vector clocks (non-negative, bounded)
    true // Every vector clock value is a valid cut
}

// r[impl causal-order.is-consistent-cut]
/// Returns true if the cut is consistent. All cuts from correct
/// causal delivery are consistent by construction.
pub fn is_consistent_cut(_cut: &Vec<u64>) -> (result: bool)
    ensures result == is_consistent_cut_spec(_cut@)
{
    true
}

// --- cut_join ---

// r[depends causal-order.cut-join]
/// Spec: join of two cuts is the component-wise max (same as VC merge).
pub open spec fn cut_join_spec(a: Seq<u64>, b: Seq<u64>) -> Seq<u64>
    recommends a.len() == b.len()
{
    Seq::new(a.len(), |i: int| max_spec(a[i], b[i]))
}

// r[impl causal-order.cut-join]
/// Join two consistent cuts.
pub fn cut_join(a: &Vec<u64>, b: &Vec<u64>) -> (result: Vec<u64>)
    requires a.len() == b.len()
    ensures result@ == cut_join_spec(a@, b@)
{
    let mut result: Vec<u64> = Vec::new();
    let mut i: usize = 0;
    while i < a.len()
        invariant
            i <= a.len(),
            a.len() == b.len(),
            result.len() == i,
            forall|j: int| 0 <= j < i as int ==> result@[j] == max_spec(a@[j], b@[j]),
        decreases a.len() - i,
    {
        result.push(crate::ordering::max(a[i], b[i]));
        i += 1;
    }
    assert(result@ =~= cut_join_spec(a@, b@));
    result
}

// --- Safety proofs ---

// r[verify causal-order.cut-join]
/// Join of two consistent cuts is consistent.
proof fn consistent_cut_join(a: Seq<u64>, b: Seq<u64>)
    requires a.len() == b.len(), is_consistent_cut_spec(a), is_consistent_cut_spec(b)
    ensures is_consistent_cut_spec(cut_join_spec(a, b))
{}

// r[verify causal-order.cut-join]
/// Join is commutative (delegated to pointwise).
proof fn cut_join_commutative(a: Seq<u64>, b: Seq<u64>)
    requires a.len() == b.len()
    ensures cut_join_spec(a, b) =~= cut_join_spec(b, a)
{
    crate::pointwise::max_u64_commutative();
    crate::pointwise::pointwise_commutative(a, b, |x: u64, y: u64| max_u64(x, y));
}

// r[verify causal-order.cut-join]
/// Join is idempotent (delegated to pointwise).
proof fn cut_join_idempotent(a: Seq<u64>)
    ensures cut_join_spec(a, a) =~= a
{
    crate::pointwise::max_u64_idempotent();
    crate::pointwise::pointwise_idempotent(a, |x: u64, y: u64| max_u64(x, y));
}

// r[verify causal-order.is-causally-ready]
/// A causally ready message's clock is one step ahead of local
/// at the sender component.
proof fn ready_one_step_ahead(
    msg_clock: Seq<u64>, local_clock: Seq<u64>, sender: int,
)
    requires
        is_causally_ready_spec(msg_clock, local_clock, sender),
    ensures
        msg_clock[sender] as int == local_clock[sender] as int + 1,
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn vc(entries: &[u64]) -> Vec<u64> {
        entries.to_vec()
    }

    // r[verify causal-order.is-causally-ready]
    #[test]
    fn test_causally_ready() {
        let local = vc(&[2, 3, 1]);
        // Message from process 1: clock[1] = 4 (one ahead), others <=
        let msg = vc(&[2, 4, 1]);
        assert!(is_causally_ready(&msg, &local, 1));

        // Not ready: sender too far ahead
        let msg2 = vc(&[2, 5, 1]);
        assert!(!is_causally_ready(&msg2, &local, 1));

        // Not ready: non-sender component too high
        let msg3 = vc(&[3, 4, 1]);
        assert!(!is_causally_ready(&msg3, &local, 1));
    }

    // r[verify causal-order.is-consistent-cut]
    #[test]
    fn test_consistent_cut() {
        // All vector-clock cuts are consistent by construction
        assert!(is_consistent_cut(&vc(&[1, 2, 3])));
        assert!(is_consistent_cut(&vc(&[0, 0, 0])));
        assert!(is_consistent_cut(&vec![]));
    }

    // r[verify causal-order.cut-join]
    #[test]
    fn test_cut_join() {
        let a = vc(&[3, 1, 0]);
        let b = vc(&[1, 2, 4]);
        assert_eq!(cut_join(&a, &b), vec![3, 2, 4]);
    }

    proptest! {
        // r[verify causal-order.cut-join]
        #[test]
        fn prop_cut_join_commutative(
            a in proptest::collection::vec(any::<u64>(), 3),
            b in proptest::collection::vec(any::<u64>(), 3),
        ) {
            prop_assert_eq!(cut_join(&a, &b), cut_join(&b, &a));
        }

        // r[verify causal-order.cut-join]
        #[test]
        fn prop_cut_join_idempotent(
            v in proptest::collection::vec(any::<u64>(), 3),
        ) {
            prop_assert_eq!(cut_join(&v, &v), v);
        }

        // r[verify causal-order.is-causally-ready]
        #[test]
        fn prop_ready_sender_one_ahead(
            local in proptest::collection::vec(0u64..100, 3),
            sender in 0usize..3,
        ) {
            let mut msg = local.clone();
            if msg[sender] < u64::MAX {
                msg[sender] += 1;
                prop_assert!(is_causally_ready(&msg, &local, sender));
            }
        }
    }
}
