//! Verified vector clocks for causality tracking.
//!
//! A vector clock is a sequence of counters, one per replica. The
//! happens-before relation is the pointwise partial order: a happened
//! before b iff every component of a is <= the corresponding component
//! of b, and at least one is strictly less. Merge is the component-wise
//! max (join in the lattice).

// Verus requires &Vec for @ view, not &[T]
#![allow(clippy::ptr_arg)]

use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::ordering::max_spec;
#[cfg(verus_keep_ghost)]
use crate::pointwise::max_u64;
#[cfg(verus_keep_ghost)]
use crate::pointwise::pointwise_merge_spec;

verus! {

// --- happens_before ---

// r[depends vector-clock.happens-before]
/// Spec: a happened before b (pointwise ≤ with at least one strict <).
pub open spec fn happens_before_spec(a: Seq<u64>, b: Seq<u64>) -> bool
    recommends a.len() == b.len()
{
    a.len() == b.len()
    && (forall|i: int| 0 <= i < a.len() as int ==> a[i] <= b[i])
    && (exists|i: int| 0 <= i < a.len() as int && a[i] < b[i])
}

// r[impl vector-clock.happens-before]
/// Returns true if clock `a` happened before clock `b`.
pub fn happens_before(a: &Vec<u64>, b: &Vec<u64>) -> (result: bool)
    requires a.len() == b.len()
    ensures result == happens_before_spec(a@, b@)
{
    let mut all_leq: bool = true;
    let mut some_lt: bool = false;
    let mut i: usize = 0;
    while i < a.len()
        invariant
            i <= a.len(),
            a.len() == b.len(),
            all_leq == (forall|j: int| 0 <= j < i as int ==> a@[j] <= b@[j]),
            some_lt == (exists|j: int| 0 <= j < i as int && a@[j] < b@[j]),
        decreases a.len() - i,
    {
        if a[i] > b[i] {
            all_leq = false;
        }
        if a[i] < b[i] {
            some_lt = true;
        }
        i += 1;
    }
    all_leq && some_lt
}

// --- concurrent ---

// r[depends vector-clock.concurrent]
/// Spec: neither a nor b happened before the other, AND they differ.
/// Equal clocks represent the same logical time, not concurrent events.
pub open spec fn concurrent_spec(a: Seq<u64>, b: Seq<u64>) -> bool
    recommends a.len() == b.len()
{
    a.len() == b.len()
    && !happens_before_spec(a, b)
    && !happens_before_spec(b, a)
    && (exists|i: int| 0 <= i < a.len() as int && a[i] != b[i])
}

// r[impl vector-clock.concurrent]
/// Returns true if clocks a and b are concurrent (incomparable and not equal).
pub fn concurrent(a: &Vec<u64>, b: &Vec<u64>) -> (result: bool)
    requires a.len() == b.len()
    ensures result == concurrent_spec(a@, b@)
{
    if happens_before(a, b) || happens_before(b, a) {
        return false;
    }
    // Check they differ somewhere
    let mut differ: bool = false;
    let mut i: usize = 0;
    while i < a.len()
        invariant
            i <= a.len(),
            a.len() == b.len(),
            differ == (exists|j: int| 0 <= j < i as int && a@[j] != b@[j]),
        decreases a.len() - i,
    {
        if a[i] != b[i] {
            differ = true;
        }
        i += 1;
    }
    differ
}

// --- merge ---

// r[depends vector-clock.merge]
/// Spec: component-wise max (lattice join).
pub open spec fn merge_spec(a: Seq<u64>, b: Seq<u64>) -> Seq<u64>
    recommends a.len() == b.len()
{
    Seq::new(a.len(), |i: int| max_spec(a[i], b[i]))
}

// r[impl vector-clock.merge]
/// Merges two vector clocks by taking the component-wise max.
pub fn merge(a: &Vec<u64>, b: &Vec<u64>) -> (result: Vec<u64>)
    requires a.len() == b.len()
    ensures result@ == merge_spec(a@, b@)
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
    assert(result@ =~= merge_spec(a@, b@));
    result
}

// --- increment ---

// r[depends vector-clock.increment]
/// Spec: increment the counter at position `idx`.
pub open spec fn increment_spec(v: Seq<u64>, idx: int) -> Seq<u64>
    recommends 0 <= idx < v.len() as int, v[idx] < u64::MAX
{
    v.update(idx, (v[idx] + 1) as u64)
}

// r[impl vector-clock.increment]
/// Returns a new clock with position `idx` incremented by 1.
pub fn increment(v: &Vec<u64>, idx: usize) -> (result: Vec<u64>)
    requires idx < v.len(), v[idx as int] < u64::MAX
    ensures result@ == increment_spec(v@, idx as int)
{
    let mut result: Vec<u64> = Vec::new();
    let mut i: usize = 0;
    while i < v.len()
        invariant
            i <= v.len(),
            idx < v.len(),
            v@[idx as int] < u64::MAX,
            result.len() == i,
            forall|j: int| 0 <= j < i as int && j != idx as int ==> result@[j] == v@[j],
            forall|j: int| 0 <= j < i as int && j == idx as int ==> result@[j] == (v@[j] + 1) as u64,
        decreases v.len() - i,
    {
        if i == idx {
            assert(v[i as int] < u64::MAX);
            result.push(v[i] + 1);
        } else {
            result.push(v[i]);
        }
        i += 1;
    }
    assert(result@ =~= increment_spec(v@, idx as int));
    result
}

// --- Partial order proofs ---

// r[verify vector-clock.happens-before]
/// Irreflexive: no clock happens before itself.
proof fn happens_before_irreflexive(v: Seq<u64>)
    ensures !happens_before_spec(v, v)
{}

// r[verify vector-clock.happens-before]
/// Asymmetric: if a happened before b, then b did not happen before a.
proof fn happens_before_asymmetric(a: Seq<u64>, b: Seq<u64>)
    requires happens_before_spec(a, b)
    ensures !happens_before_spec(b, a)
{
    // a hb b means: forall i, a[i] <= b[i] AND exists i, a[i] < b[i]
    // If also b hb a: forall i, b[i] <= a[i]
    // Combined: forall i, a[i] == b[i] — contradicts the strict < witness.
}

// r[verify vector-clock.happens-before]
/// Transitive: if a hb b and b hb c then a hb c.
proof fn happens_before_transitive(a: Seq<u64>, b: Seq<u64>, c: Seq<u64>)
    requires happens_before_spec(a, b), happens_before_spec(b, c)
    ensures happens_before_spec(a, c)
{
    // forall i: a[i] <= b[i] <= c[i], so a[i] <= c[i]
    // exists i: a[i] < b[i], and b[i] <= c[i], so a[i] < c[i]
}

// --- Merge lattice proofs (delegated to pointwise) ---

// r[verify vector-clock.merge]
/// merge_spec is pointwise max — connect to the generic pointwise module.
proof fn merge_is_pointwise(a: Seq<u64>, b: Seq<u64>)
    requires a.len() == b.len()
    ensures merge_spec(a, b) =~= pointwise_merge_spec(a, b, |x: u64, y: u64| max_u64(x, y))
{}

// r[verify vector-clock.merge]
/// Merge is commutative: merge(a, b) == merge(b, a).
proof fn merge_commutative(a: Seq<u64>, b: Seq<u64>)
    requires a.len() == b.len()
    ensures merge_spec(a, b) =~= merge_spec(b, a)
{
    crate::pointwise::max_u64_commutative();
    crate::pointwise::pointwise_commutative(a, b, |x: u64, y: u64| max_u64(x, y));
}

// r[verify vector-clock.merge]
/// Merge is associative: merge(merge(a, b), c) == merge(a, merge(b, c)).
proof fn merge_associative(a: Seq<u64>, b: Seq<u64>, c: Seq<u64>)
    requires a.len() == b.len(), b.len() == c.len()
    ensures merge_spec(merge_spec(a, b), c) =~= merge_spec(a, merge_spec(b, c))
{
    crate::pointwise::max_u64_associative();
    crate::pointwise::pointwise_associative(a, b, c, |x: u64, y: u64| max_u64(x, y));
}

// r[verify vector-clock.merge]
/// Merge is idempotent: merge(v, v) == v.
proof fn merge_idempotent(v: Seq<u64>)
    ensures merge_spec(v, v) =~= v
{
    crate::pointwise::max_u64_idempotent();
    crate::pointwise::pointwise_idempotent(v, |x: u64, y: u64| max_u64(x, y));
}

// r[verify vector-clock.merge]
/// Merge dominates both inputs: merge(a,b)[i] >= a[i] for all i.
proof fn merge_dominates_left(a: Seq<u64>, b: Seq<u64>)
    requires a.len() == b.len()
    ensures forall|i: int| 0 <= i < a.len() as int ==> merge_spec(a, b)[i] >= a[i]
{
    crate::pointwise::max_u64_dominates_left();
    crate::pointwise::pointwise_dominates_left(a, b, |x: u64, y: u64| max_u64(x, y));
}

// r[verify vector-clock.merge]
/// Merge dominates right input too.
proof fn merge_dominates_right(a: Seq<u64>, b: Seq<u64>)
    requires a.len() == b.len()
    ensures forall|i: int| 0 <= i < a.len() as int ==> merge_spec(a, b)[i] >= b[i]
{
    crate::pointwise::max_u64_dominates_right();
    crate::pointwise::pointwise_dominates_right(a, b, |x: u64, y: u64| max_u64(x, y));
}

// --- Increment proofs ---

// r[verify vector-clock.increment]
/// Increment advances: v happens-before increment(v, idx).
proof fn increment_advances(v: Seq<u64>, idx: int)
    requires 0 <= idx < v.len() as int, v[idx] < u64::MAX
    ensures happens_before_spec(v, increment_spec(v, idx))
{
    // All components: v[i] <= increment(v, idx)[i] — true because
    // increment only changes idx (increasing by 1), rest unchanged.
    // Strict at idx: v[idx] < v[idx] + 1.
    assert(v[idx] < increment_spec(v, idx)[idx]);
}

// r[verify vector-clock.increment]
/// Increment preserves other components.
proof fn increment_preserves_others(v: Seq<u64>, idx: int)
    requires 0 <= idx < v.len() as int, v[idx] < u64::MAX
    ensures forall|j: int| 0 <= j < v.len() as int && j != idx ==>
        increment_spec(v, idx)[j] == v[j]
{}

// r[verify vector-clock.increment]
/// Increment preserves length.
proof fn increment_preserves_length(v: Seq<u64>, idx: int)
    requires 0 <= idx < v.len() as int, v[idx] < u64::MAX
    ensures increment_spec(v, idx).len() == v.len()
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn vc(entries: &[u64]) -> Vec<u64> {
        entries.to_vec()
    }

    // r[verify vector-clock.happens-before]
    #[test]
    fn test_happens_before() {
        assert!(happens_before(&vc(&[1, 0, 0]), &vc(&[1, 1, 0])));
        assert!(happens_before(&vc(&[0, 0, 0]), &vc(&[1, 0, 0])));
        assert!(!happens_before(&vc(&[1, 0, 0]), &vc(&[1, 0, 0]))); // equal
        assert!(!happens_before(&vc(&[1, 1, 0]), &vc(&[0, 0, 1]))); // concurrent
    }

    // r[verify vector-clock.concurrent]
    #[test]
    fn test_concurrent() {
        assert!(concurrent(&vc(&[1, 0, 0]), &vc(&[0, 1, 0])));
        assert!(concurrent(&vc(&[2, 1, 0]), &vc(&[1, 2, 0])));
        assert!(!concurrent(&vc(&[1, 0, 0]), &vc(&[1, 1, 0]))); // a hb b
        assert!(!concurrent(&vc(&[1, 0, 0]), &vc(&[1, 0, 0]))); // equal (not concurrent)
    }

    // r[verify vector-clock.merge]
    #[test]
    fn test_merge() {
        let a = vc(&[3, 1, 0]);
        let b = vc(&[1, 2, 4]);
        let m = merge(&a, &b);
        assert_eq!(m, vec![3, 2, 4]);
    }

    // r[verify vector-clock.increment]
    #[test]
    fn test_increment() {
        let v = vc(&[1, 2, 3]);
        let inc = increment(&v, 1);
        assert_eq!(inc, vec![1, 3, 3]);
    }

    proptest! {
        // r[verify vector-clock.happens-before]
        #[test]
        fn prop_happens_before_irreflexive(
            entries in proptest::collection::vec(any::<u64>(), 1..=8),
        ) {
            prop_assert!(!happens_before(&entries, &entries));
        }

        // r[verify vector-clock.happens-before]
        #[test]
        fn prop_happens_before_asymmetric(
            a in proptest::collection::vec(0u64..100, 3),
            b in proptest::collection::vec(0u64..100, 3),
        ) {
            if happens_before(&a, &b) {
                prop_assert!(!happens_before(&b, &a));
            }
        }

        // r[verify vector-clock.merge]
        #[test]
        fn prop_merge_commutative(
            a in proptest::collection::vec(any::<u64>(), 3),
            b in proptest::collection::vec(any::<u64>(), 3),
        ) {
            prop_assert_eq!(merge(&a, &b), merge(&b, &a));
        }

        // r[verify vector-clock.merge]
        #[test]
        fn prop_merge_idempotent(
            v in proptest::collection::vec(any::<u64>(), 1..=8),
        ) {
            prop_assert_eq!(merge(&v, &v), v);
        }

        // r[verify vector-clock.merge]
        #[test]
        fn prop_merge_associative(
            a in proptest::collection::vec(any::<u64>(), 3),
            b in proptest::collection::vec(any::<u64>(), 3),
            c in proptest::collection::vec(any::<u64>(), 3),
        ) {
            let ab = merge(&a, &b);
            let bc = merge(&b, &c);
            prop_assert_eq!(merge(&ab, &c), merge(&a, &bc));
        }

        // r[verify vector-clock.merge]
        #[test]
        fn prop_merge_dominates(
            a in proptest::collection::vec(any::<u64>(), 3),
            b in proptest::collection::vec(any::<u64>(), 3),
        ) {
            let m = merge(&a, &b);
            for i in 0..3 {
                prop_assert!(m[i] >= a[i]);
                prop_assert!(m[i] >= b[i]);
            }
        }

        // r[verify vector-clock.increment]
        #[test]
        fn prop_increment_advances(
            v in proptest::collection::vec(0u64..u64::MAX, 3),
            idx in 0usize..3,
        ) {
            let inc = increment(&v, idx);
            prop_assert!(happens_before(&v, &inc));
        }

        // r[verify vector-clock.increment]
        #[test]
        fn prop_increment_preserves_others(
            v in proptest::collection::vec(0u64..u64::MAX, 3),
            idx in 0usize..3,
        ) {
            let inc = increment(&v, idx);
            for i in 0..3 {
                if i != idx {
                    prop_assert_eq!(inc[i], v[i]);
                }
            }
            prop_assert_eq!(inc[idx], v[idx] + 1);
        }

        // r[verify vector-clock.concurrent]
        #[test]
        fn prop_concurrent_symmetric(
            a in proptest::collection::vec(any::<u64>(), 3),
            b in proptest::collection::vec(any::<u64>(), 3),
        ) {
            prop_assert_eq!(concurrent(&a, &b), concurrent(&b, &a));
        }
    }
}
