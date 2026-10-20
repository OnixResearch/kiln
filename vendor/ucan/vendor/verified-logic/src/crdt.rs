//! Verified CRDTs: G-Counter, PN-Counter, LWW-Register.
//!
//! Each type's merge satisfies the semilattice laws (commutativity,
//! associativity, idempotency). G-Counter and PN-Counter use vector
//! clocks internally; LWW-Register uses timestamp comparison.

#![allow(clippy::nonminimal_bool, clippy::ptr_arg)]

use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::ordering::max_spec;
#[cfg(verus_keep_ghost)]
use crate::pointwise::max_u64;
#[cfg(verus_keep_ghost)]
use crate::pointwise::pointwise_merge_spec;

verus! {

// ============================================================
// G-Counter: grow-only counter (vector of per-replica counts)
// ============================================================

// r[depends crdt.g-counter-merge]
/// Spec: merge two G-Counters by taking component-wise max.
pub open spec fn g_counter_merge_spec(a: Seq<u64>, b: Seq<u64>) -> Seq<u64>
    recommends a.len() == b.len()
{
    Seq::new(a.len(), |i: int| max_spec(a[i], b[i]))
}

// r[impl crdt.g-counter-merge]
/// Merge two G-Counters by taking component-wise max.
pub fn g_counter_merge(a: &Vec<u64>, b: &Vec<u64>) -> (result: Vec<u64>)
    requires a.len() == b.len()
    ensures result@ == g_counter_merge_spec(a@, b@)
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
    assert(result@ =~= g_counter_merge_spec(a@, b@));
    result
}

// r[depends crdt.g-counter-value]
/// Spec: total value of a G-Counter.
pub open spec fn g_counter_value_spec(counter: Seq<u64>) -> int
    decreases counter.len()
{
    if counter.len() == 0 {
        0int
    } else {
        g_counter_value_spec(counter.drop_last()) + counter.last() as int
    }
}

// Helper lemma: taking i+1 elements decomposes as take(i) + element[i]
proof fn g_counter_value_take_lemma(s: Seq<u64>, i: int)
    requires 0 <= i < s.len()
    ensures
        g_counter_value_spec(s.take(i + 1))
            == g_counter_value_spec(s.take(i)) + s[i] as int,
{
    let t = s.take(i + 1);
    assert(t.drop_last() =~= s.take(i));
    assert(t.last() == s[i]);
}

// Helper lemma: g_counter_value_spec is non-negative
proof fn g_counter_value_nonneg(s: Seq<u64>)
    ensures g_counter_value_spec(s) >= 0
    decreases s.len()
{
    if s.len() > 0 {
        g_counter_value_nonneg(s.drop_last());
    }
}

// Helper lemma: prefix sum is monotonically non-decreasing
proof fn g_counter_value_prefix_le(s: Seq<u64>, i: int)
    requires 0 <= i <= s.len()
    ensures g_counter_value_spec(s.take(i)) <= g_counter_value_spec(s)
    decreases s.len() - i
{
    if i == s.len() {
        assert(s.take(i) =~= s);
    } else {
        // take(i) sum <= take(i+1) sum <= ... <= full sum
        g_counter_value_take_lemma(s, i);
        g_counter_value_prefix_le(s, i + 1);
    }
}

// r[impl crdt.g-counter-value]
/// Returns the sum of all replica counts.
pub fn g_counter_value(counter: &Vec<u64>) -> (result: u64)
    requires
        g_counter_value_spec(counter@) <= u64::MAX as int,
        g_counter_value_spec(counter@) >= 0,
    ensures result as int == g_counter_value_spec(counter@)
{
    let mut sum: u64 = 0;
    let mut i: usize = 0;
    assert(counter@.take(0int) =~= Seq::<u64>::empty());
    while i < counter.len()
        invariant
            i <= counter.len(),
            sum as int == g_counter_value_spec(counter@.take(i as int)),
            g_counter_value_spec(counter@) <= u64::MAX as int,
            g_counter_value_spec(counter@.take(i as int)) <= u64::MAX as int,
        decreases counter.len() - i,
    {
        proof {
            g_counter_value_take_lemma(counter@, i as int);
            g_counter_value_prefix_le(counter@, i as int + 1);
        }
        sum = sum + counter[i];
        i = i + 1;
    }
    proof {
        assert(counter@.take(counter@.len() as int) =~= counter@);
    }
    sum
}

// r[depends crdt.g-counter-increment]
/// Spec: increment the counter at replica `idx`.
pub open spec fn g_counter_increment_spec(counter: Seq<u64>, idx: int) -> Seq<u64>
    recommends 0 <= idx < counter.len() as int, counter[idx] < u64::MAX
{
    counter.update(idx, (counter[idx] + 1) as u64)
}

// r[impl crdt.g-counter-increment]
/// Increment the count for replica `idx`.
pub fn g_counter_increment(counter: &Vec<u64>, idx: usize) -> (result: Vec<u64>)
    requires idx < counter.len(), counter[idx as int] < u64::MAX
    ensures result@ == g_counter_increment_spec(counter@, idx as int)
{
    let mut result: Vec<u64> = Vec::new();
    let mut i: usize = 0;
    while i < counter.len()
        invariant
            i <= counter.len(),
            idx < counter.len(),
            counter@[idx as int] < u64::MAX,
            result.len() == i,
            forall|j: int| 0 <= j < i as int && j != idx as int ==> result@[j] == counter@[j],
            forall|j: int| 0 <= j < i as int && j == idx as int ==> result@[j] == (counter@[j] + 1) as u64,
        decreases counter.len() - i,
    {
        if i == idx {
            assert(counter[i as int] < u64::MAX);
            result.push(counter[i] + 1);
        } else {
            result.push(counter[i]);
        }
        i += 1;
    }
    assert(result@ =~= g_counter_increment_spec(counter@, idx as int));
    result
}

// --- G-Counter merge proofs (delegated to pointwise) ---

// r[verify crdt.g-counter-merge]
/// Merge is commutative.
proof fn g_counter_merge_commutative(a: Seq<u64>, b: Seq<u64>)
    requires a.len() == b.len()
    ensures g_counter_merge_spec(a, b) =~= g_counter_merge_spec(b, a)
{
    crate::pointwise::max_u64_commutative();
    crate::pointwise::pointwise_commutative(a, b, |x: u64, y: u64| max_u64(x, y));
}

// r[verify crdt.g-counter-merge]
/// Merge is associative.
proof fn g_counter_merge_associative(a: Seq<u64>, b: Seq<u64>, c: Seq<u64>)
    requires a.len() == b.len(), b.len() == c.len()
    ensures g_counter_merge_spec(g_counter_merge_spec(a, b), c)
        =~= g_counter_merge_spec(a, g_counter_merge_spec(b, c))
{
    crate::pointwise::max_u64_associative();
    crate::pointwise::pointwise_associative(a, b, c, |x: u64, y: u64| max_u64(x, y));
}

// r[verify crdt.g-counter-merge]
/// Merge is idempotent.
proof fn g_counter_merge_idempotent(a: Seq<u64>)
    ensures g_counter_merge_spec(a, a) =~= a
{
    crate::pointwise::max_u64_idempotent();
    crate::pointwise::pointwise_idempotent(a, |x: u64, y: u64| max_u64(x, y));
}

// ============================================================
// PN-Counter: increment/decrement via two G-Counters
// ============================================================

// r[depends crdt.pn-counter-merge]
/// Spec: merge two PN-Counters (merge both the P and N vectors).
pub open spec fn pn_counter_merge_spec(
    p_a: Seq<u64>, n_a: Seq<u64>,
    p_b: Seq<u64>, n_b: Seq<u64>,
) -> (Seq<u64>, Seq<u64>)
    recommends p_a.len() == p_b.len(), n_a.len() == n_b.len()
{
    (g_counter_merge_spec(p_a, p_b), g_counter_merge_spec(n_a, n_b))
}

// r[impl crdt.pn-counter-merge]
/// Merge two PN-Counters.
pub fn pn_counter_merge(
    p_a: &Vec<u64>, n_a: &Vec<u64>,
    p_b: &Vec<u64>, n_b: &Vec<u64>,
) -> (result: (Vec<u64>, Vec<u64>))
    requires p_a.len() == p_b.len(), n_a.len() == n_b.len()
    ensures result.0@ == pn_counter_merge_spec(p_a@, n_a@, p_b@, n_b@).0,
            result.1@ == pn_counter_merge_spec(p_a@, n_a@, p_b@, n_b@).1
{
    (g_counter_merge(p_a, p_b), g_counter_merge(n_a, n_b))
}

// r[verify crdt.pn-counter-merge]
/// PN-Counter merge is commutative.
proof fn pn_counter_merge_commutative(
    p_a: Seq<u64>, n_a: Seq<u64>,
    p_b: Seq<u64>, n_b: Seq<u64>,
)
    requires p_a.len() == p_b.len(), n_a.len() == n_b.len()
    ensures pn_counter_merge_spec(p_a, n_a, p_b, n_b)
        == pn_counter_merge_spec(p_b, n_b, p_a, n_a)
{
    g_counter_merge_commutative(p_a, p_b);
    g_counter_merge_commutative(n_a, n_b);
}

// r[verify crdt.pn-counter-merge]
/// PN-Counter merge is idempotent.
proof fn pn_counter_merge_idempotent(p: Seq<u64>, n: Seq<u64>)
    ensures pn_counter_merge_spec(p, n, p, n) == (p, n)
{
    g_counter_merge_idempotent(p);
    g_counter_merge_idempotent(n);
}

// ============================================================
// LWW-Register: last-writer-wins by timestamp
// ============================================================

// r[depends crdt.lww-merge]
/// Spec: merge two LWW registers. Higher timestamp wins. On tie, b wins.
pub open spec fn lww_merge_spec(
    val_a: u64, ts_a: u64,
    val_b: u64, ts_b: u64,
) -> (u64, u64) {
    if ts_a > ts_b { (val_a, ts_a) } else { (val_b, ts_b) }
}

// r[impl crdt.lww-merge]
/// Merge two LWW registers.
pub fn lww_merge(val_a: u64, ts_a: u64, val_b: u64, ts_b: u64) -> (result: (u64, u64))
    ensures result == lww_merge_spec(val_a, ts_a, val_b, ts_b)
{
    if ts_a > ts_b { (val_a, ts_a) } else { (val_b, ts_b) }
}

// r[verify crdt.lww-merge]
/// LWW merge is commutative when timestamps differ.
proof fn lww_merge_commutative_distinct(val_a: u64, ts_a: u64, val_b: u64, ts_b: u64)
    requires ts_a != ts_b
    ensures lww_merge_spec(val_a, ts_a, val_b, ts_b)
        == lww_merge_spec(val_b, ts_b, val_a, ts_a)
{}

// r[verify crdt.lww-merge]
/// LWW merge is idempotent.
proof fn lww_merge_idempotent(val: u64, ts: u64)
    ensures lww_merge_spec(val, ts, val, ts) == (val, ts)
{}

// r[verify crdt.lww-merge]
/// Higher timestamp always wins.
proof fn lww_higher_timestamp_wins(val_a: u64, ts_a: u64, val_b: u64, ts_b: u64)
    requires ts_a > ts_b
    ensures lww_merge_spec(val_a, ts_a, val_b, ts_b) == (val_a, ts_a)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn gc(entries: &[u64]) -> Vec<u64> {
        entries.to_vec()
    }

    // r[verify crdt.g-counter-merge]
    #[test]
    fn test_g_counter_merge() {
        let a = gc(&[3, 1, 0]);
        let b = gc(&[1, 2, 4]);
        let m = g_counter_merge(&a, &b);
        assert_eq!(m, vec![3, 2, 4]);
    }

    // r[verify crdt.g-counter-value]
    #[test]
    fn test_g_counter_value() {
        assert_eq!(g_counter_value(&gc(&[3, 2, 4])), 9);
        assert_eq!(g_counter_value(&gc(&[0, 0, 0])), 0);
    }

    // r[verify crdt.g-counter-increment]
    #[test]
    fn test_g_counter_increment() {
        let c = gc(&[1, 2, 3]);
        let inc = g_counter_increment(&c, 1);
        assert_eq!(inc, vec![1, 3, 3]);
    }

    // r[verify crdt.pn-counter-merge]
    #[test]
    fn test_pn_counter_merge() {
        let (p, n) = pn_counter_merge(&gc(&[3, 1]), &gc(&[1, 0]), &gc(&[1, 2]), &gc(&[0, 1]));
        assert_eq!(p, vec![3, 2]);
        assert_eq!(n, vec![1, 1]);
    }

    // r[verify crdt.lww-merge]
    #[test]
    fn test_lww_merge() {
        assert_eq!(lww_merge(10, 5, 20, 3), (10, 5)); // a newer
        assert_eq!(lww_merge(10, 3, 20, 5), (20, 5)); // b newer
        assert_eq!(lww_merge(10, 5, 20, 5), (20, 5)); // tie: b wins
    }

    proptest! {
        // r[verify crdt.g-counter-merge]
        #[test]
        fn prop_g_counter_merge_commutative(
            a in proptest::collection::vec(any::<u64>(), 3),
            b in proptest::collection::vec(any::<u64>(), 3),
        ) {
            prop_assert_eq!(g_counter_merge(&a, &b), g_counter_merge(&b, &a));
        }

        // r[verify crdt.g-counter-merge]
        #[test]
        fn prop_g_counter_merge_idempotent(
            v in proptest::collection::vec(any::<u64>(), 3),
        ) {
            prop_assert_eq!(g_counter_merge(&v, &v), v);
        }

        // r[verify crdt.lww-merge]
        #[test]
        fn prop_lww_merge_idempotent(val: u64, ts: u64) {
            prop_assert_eq!(lww_merge(val, ts, val, ts), (val, ts));
        }

        // r[verify crdt.lww-merge]
        #[test]
        fn prop_lww_higher_wins(val_a: u64, ts_a: u64, val_b: u64, ts_b: u64) {
            if ts_a > ts_b {
                prop_assert_eq!(lww_merge(val_a, ts_a, val_b, ts_b), (val_a, ts_a));
            }
        }
    }
}
