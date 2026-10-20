//! Verified consistent snapshot (cut) primitives.
//!
//! A consistent cut of a distributed execution is a set of local states
//! (one per process) such that no message crosses the cut backward: if a
//! receive event is in the cut, the corresponding send must also be in the
//! cut. The local states are encoded as vector clock entries (how many
//! events each process has executed). Proofs cover cut consistency,
//! lattice operations (join/meet), and the relationship between cuts and
//! happened-before.

use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::ordering::max_spec;
#[cfg(verus_keep_ghost)]
use crate::ordering::min_spec;

verus! {

// ============================================================
// Cut validity
// ============================================================

// r[depends consistent-snapshot.is-valid-cut]
/// Spec: a cut is valid when it has at least one process and all
/// entries are non-negative (u64 guarantees this).
pub open spec fn is_valid_cut_spec(cut: Seq<u64>) -> bool {
    cut.len() > 0
}

// r[impl consistent-snapshot.is-valid-cut]
/// Returns true if the cut vector is valid (non-empty).
pub fn is_valid_cut(cut: &[u64]) -> (result: bool)
    ensures result == is_valid_cut_spec(cut@)
{
    cut.len() > 0
}

// ============================================================
// Cut consistency: no backward messages
// ============================================================

// r[depends consistent-snapshot.is-consistent]
/// Spec: a cut is consistent with a message (from sender at send_clock
/// to receiver at recv_clock) when:
/// - If the receive is in the cut (cut[receiver] >= recv_time),
///   then the send must be in the cut (cut[sender] >= send_time).
///
/// This checks one message. A globally consistent cut must satisfy
/// this for ALL messages.
pub open spec fn message_consistent_spec(
    cut: Seq<u64>, sender: int, send_time: u64,
    receiver: int, recv_time: u64,
) -> bool
    recommends
        0 <= sender < cut.len(),
        0 <= receiver < cut.len(),
{
    // If the receive is in the cut, the send must be too.
    (cut[receiver] >= recv_time) ==> (cut[sender] >= send_time)
}

// r[impl consistent-snapshot.is-consistent]
/// Check if a cut is consistent with respect to one message.
pub fn message_consistent(
    cut: &[u64], sender: usize, send_time: u64,
    receiver: usize, recv_time: u64,
) -> (result: bool)
    requires
        sender < cut@.len(),
        receiver < cut@.len(),
    ensures result == message_consistent_spec(
        cut@, sender as int, send_time, receiver as int, recv_time,
    )
{
    if cut[receiver] >= recv_time {
        cut[sender] >= send_time
    } else {
        true
    }
}

// ============================================================
// Cut comparison: happened-before on cuts
// ============================================================

// r[depends consistent-snapshot.cut-le]
/// Spec: cut A <= cut B (A happened before or equal to B) when every
/// component of A is <= the corresponding component of B.
pub open spec fn cut_le_spec(a: Seq<u64>, b: Seq<u64>) -> bool
    recommends a.len() == b.len()
{
    a.len() == b.len()
    && forall|i: int| #![trigger a[i]] 0 <= i < a.len() as int ==> a[i] <= b[i]
}

// r[impl consistent-snapshot.cut-le]
/// Returns true if cut A is component-wise <= cut B.
pub fn cut_le(a: &[u64], b: &[u64]) -> (result: bool)
    requires a@.len() == b@.len()
    ensures result == cut_le_spec(a@, b@)
{
    let mut i: usize = 0;
    while i < a.len()
        invariant
            0 <= i <= a@.len(),
            a@.len() == b@.len(),
            forall|j: int| #![trigger a@[j]] 0 <= j < i as int ==> a@[j] <= b@[j],
        decreases a@.len() - i,
    {
        if a[i] > b[i] {
            return false;
        }
        i = i + 1;
    }
    true
}

// ============================================================
// Cut join (component-wise max) and meet (component-wise min)
// ============================================================

// r[depends consistent-snapshot.cut-join]
/// Spec: join of two cuts — component-wise maximum.
pub open spec fn cut_join_spec(a: Seq<u64>, b: Seq<u64>) -> Seq<u64>
    recommends a.len() == b.len()
{
    Seq::new(a.len(), |i: int| max_spec(a[i], b[i]))
}

// r[impl consistent-snapshot.cut-join]
/// Compute the join (component-wise max) of two cuts.
pub fn cut_join(a: &[u64], b: &[u64]) -> (result: Vec<u64>)
    requires a@.len() == b@.len(), a@.len() <= usize::MAX as int
    ensures
        result@.len() == a@.len(),
        result@ =~= cut_join_spec(a@, b@),
{
    let mut result: Vec<u64> = Vec::new();
    let mut i: usize = 0;
    while i < a.len()
        invariant
            0 <= i <= a@.len(),
            a@.len() == b@.len(),
            result@.len() == i as int,
            forall|j: int| #![trigger result@[j]]
                0 <= j < i as int ==> result@[j] == max_spec(a@[j], b@[j]),
        decreases a@.len() - i,
    {
        let v = if a[i] >= b[i] { a[i] } else { b[i] };
        result.push(v);
        i = i + 1;
    }
    result
}

// r[depends consistent-snapshot.cut-meet]
/// Spec: meet of two cuts — component-wise minimum.
pub open spec fn cut_meet_spec(a: Seq<u64>, b: Seq<u64>) -> Seq<u64>
    recommends a.len() == b.len()
{
    Seq::new(a.len(), |i: int| min_spec(a[i], b[i]))
}

// r[impl consistent-snapshot.cut-meet]
/// Compute the meet (component-wise min) of two cuts.
pub fn cut_meet(a: &[u64], b: &[u64]) -> (result: Vec<u64>)
    requires a@.len() == b@.len(), a@.len() <= usize::MAX as int
    ensures
        result@.len() == a@.len(),
        result@ =~= cut_meet_spec(a@, b@),
{
    let mut result: Vec<u64> = Vec::new();
    let mut i: usize = 0;
    while i < a.len()
        invariant
            0 <= i <= a@.len(),
            a@.len() == b@.len(),
            result@.len() == i as int,
            forall|j: int| #![trigger result@[j]]
                0 <= j < i as int ==> result@[j] == min_spec(a@[j], b@[j]),
        decreases a@.len() - i,
    {
        let v = if a[i] <= b[i] { a[i] } else { b[i] };
        result.push(v);
        i = i + 1;
    }
    result
}

// ============================================================
// Proofs
// ============================================================

// r[verify consistent-snapshot.cut-le]
/// cut_le is reflexive.
proof fn cut_le_reflexive(a: Seq<u64>)
    ensures cut_le_spec(a, a)
{}

// r[verify consistent-snapshot.cut-le]
/// cut_le is transitive.
proof fn cut_le_transitive(a: Seq<u64>, b: Seq<u64>, c: Seq<u64>)
    requires cut_le_spec(a, b), cut_le_spec(b, c)
    ensures cut_le_spec(a, c)
{}

// r[verify consistent-snapshot.cut-le]
/// cut_le is antisymmetric: a <= b and b <= a implies a == b.
proof fn cut_le_antisymmetric(a: Seq<u64>, b: Seq<u64>)
    requires cut_le_spec(a, b), cut_le_spec(b, a)
    ensures a =~= b
{}

// r[verify consistent-snapshot.cut-join]
/// Join dominates both inputs.
proof fn cut_join_dominates(a: Seq<u64>, b: Seq<u64>)
    requires a.len() == b.len()
    ensures
        cut_le_spec(a, cut_join_spec(a, b)),
        cut_le_spec(b, cut_join_spec(a, b)),
{}

// r[verify consistent-snapshot.cut-join]
/// Join is commutative.
proof fn cut_join_commutative(a: Seq<u64>, b: Seq<u64>)
    requires a.len() == b.len()
    ensures cut_join_spec(a, b) =~= cut_join_spec(b, a)
{}

// r[verify consistent-snapshot.cut-join]
/// Join is idempotent.
proof fn cut_join_idempotent(a: Seq<u64>)
    ensures cut_join_spec(a, a) =~= a
{}

// r[verify consistent-snapshot.cut-meet]
/// Meet is below both inputs.
proof fn cut_meet_below(a: Seq<u64>, b: Seq<u64>)
    requires a.len() == b.len()
    ensures
        cut_le_spec(cut_meet_spec(a, b), a),
        cut_le_spec(cut_meet_spec(a, b), b),
{}

// r[verify consistent-snapshot.cut-meet]
/// Meet is commutative.
proof fn cut_meet_commutative(a: Seq<u64>, b: Seq<u64>)
    requires a.len() == b.len()
    ensures cut_meet_spec(a, b) =~= cut_meet_spec(b, a)
{}

// r[verify consistent-snapshot.cut-meet]
/// Meet is idempotent.
proof fn cut_meet_idempotent(a: Seq<u64>)
    ensures cut_meet_spec(a, a) =~= a
{}

// r[verify consistent-snapshot.is-consistent]
/// A trivial cut (all zeros) is consistent with any message whose
/// receive time is > 0 (the receive is not in the cut).
proof fn zero_cut_consistent(n: int, sender: int, st: u64, receiver: int, rt: u64)
    requires
        n > 0, 0 <= sender < n, 0 <= receiver < n, rt > 0,
    ensures message_consistent_spec(
        Seq::new(n as nat, |_i: int| 0u64),
        sender, st, receiver, rt,
    )
{}

// r[verify consistent-snapshot.is-consistent]
/// A maximal cut (all u64::MAX) is consistent with any message.
proof fn max_cut_consistent(n: int, sender: int, st: u64, receiver: int, rt: u64)
    requires
        n > 0, 0 <= sender < n, 0 <= receiver < n,
    ensures message_consistent_spec(
        Seq::new(n as nat, |_i: int| u64::MAX),
        sender, st, receiver, rt,
    )
{}

// r[verify consistent-snapshot.cut-join]
/// Join preserves message consistency: if both cuts are consistent
/// with a message, their join is too. The join has higher entries,
/// so if send_time <= cut[sender], it's also <= max(cut_a, cut_b)[sender].
proof fn join_preserves_consistency(
    a: Seq<u64>, b: Seq<u64>,
    sender: int, st: u64, receiver: int, rt: u64,
)
    requires
        a.len() == b.len(),
        0 <= sender < a.len(),
        0 <= receiver < a.len(),
        message_consistent_spec(a, sender, st, receiver, rt),
        message_consistent_spec(b, sender, st, receiver, rt),
    ensures message_consistent_spec(
        cut_join_spec(a, b), sender, st, receiver, rt,
    )
{}

// r[verify consistent-snapshot.cut-le]
/// A larger cut includes strictly more events: if a < b (not equal),
/// then at least one process has progressed.
proof fn cut_lt_means_progress(a: Seq<u64>, b: Seq<u64>)
    requires
        cut_le_spec(a, b),
        !(a =~= b),
    ensures exists|i: int| #![trigger a[i]]
        0 <= i < a.len() as int && a[i] < b[i]
{
    // If a <= b but a != b, some component must be strictly less.
    // By contrapositive: if all a[i] == b[i], then a =~= b.
}

} // verus!

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify consistent-snapshot.is-valid-cut]
    #[test]
    fn test_valid_cut() {
        assert!(is_valid_cut(&[0, 0, 0]));
        assert!(!is_valid_cut(&[]));
    }

    // r[verify consistent-snapshot.is-consistent]
    #[test]
    fn test_message_consistent() {
        // Receive in cut, send in cut -> consistent
        assert!(message_consistent(&[5, 3], 0, 2, 1, 3));
        // Receive in cut, send NOT in cut -> inconsistent
        assert!(!message_consistent(&[1, 3], 0, 2, 1, 3));
        // Receive NOT in cut -> always consistent
        assert!(message_consistent(&[1, 2], 0, 100, 1, 3));
    }

    // r[verify consistent-snapshot.cut-le]
    #[test]
    fn test_cut_le() {
        assert!(cut_le(&[1, 2, 3], &[1, 2, 3])); // equal
        assert!(cut_le(&[1, 2, 3], &[2, 3, 4])); // strictly less
        assert!(!cut_le(&[1, 5, 3], &[2, 3, 4])); // 5 > 3
    }

    // r[verify consistent-snapshot.cut-join]
    #[test]
    fn test_cut_join() {
        let j = cut_join(&[1, 5, 3], &[4, 2, 6]);
        assert_eq!(j, vec![4, 5, 6]);
    }

    // r[verify consistent-snapshot.cut-meet]
    #[test]
    fn test_cut_meet() {
        let m = cut_meet(&[1, 5, 3], &[4, 2, 6]);
        assert_eq!(m, vec![1, 2, 3]);
    }

    proptest! {
        // r[verify consistent-snapshot.cut-le]
        #[test]
        fn prop_cut_le_reflexive(cut in proptest::collection::vec(any::<u64>(), 1..10)) {
            prop_assert!(cut_le(&cut, &cut));
        }

        // r[verify consistent-snapshot.cut-join]
        #[test]
        fn prop_join_dominates(
            a in proptest::collection::vec(any::<u64>(), 1..10),
        ) {
            let n = a.len();
            let b: Vec<u64> = (0..n).map(|i| a[i].wrapping_add(1)).collect();
            let j = cut_join(&a, &b);
            prop_assert!(cut_le(&a, &j));
            prop_assert!(cut_le(&b, &j));
        }

        // r[verify consistent-snapshot.cut-join]
        #[test]
        fn prop_join_commutative(
            a in proptest::collection::vec(any::<u64>(), 1..10),
            b_raw in proptest::collection::vec(any::<u64>(), 1..20),
        ) {
            let n = a.len();
            if b_raw.len() >= n {
                let b: Vec<u64> = b_raw[..n].to_vec();
                let j1 = cut_join(&a, &b);
                let j2 = cut_join(&b, &a);
                prop_assert_eq!(j1, j2);
            }
        }

        // r[verify consistent-snapshot.cut-meet]
        #[test]
        fn prop_meet_commutative(
            a in proptest::collection::vec(any::<u64>(), 1..10),
            b_raw in proptest::collection::vec(any::<u64>(), 1..20),
        ) {
            let n = a.len();
            if b_raw.len() >= n {
                let b: Vec<u64> = b_raw[..n].to_vec();
                let m1 = cut_meet(&a, &b);
                let m2 = cut_meet(&b, &a);
                prop_assert_eq!(m1, m2);
            }
        }

        // r[verify consistent-snapshot.is-consistent]
        #[test]
        fn prop_max_cut_always_consistent(
            n in 2usize..10,
            sender in 0usize..10,
            receiver in 0usize..10,
            st: u64, rt: u64,
        ) {
            if sender < n && receiver < n {
                let cut: Vec<u64> = vec![u64::MAX; n];
                prop_assert!(message_consistent(&cut, sender, st, receiver, rt));
            }
        }
    }
}
