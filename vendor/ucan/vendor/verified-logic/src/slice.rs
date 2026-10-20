//! Verified slice operations.
//!
//! Chunking arithmetic, prefix/suffix comparison, element-wise
//! equality, and mismatch detection — all with machine-checked
//! contracts.

use vstd::prelude::*;

verus! {

// --- chunk_count ---

// r[depends slice.chunk-count]
/// Spec: number of full chunks of `chunk_size` that fit in `len`.
pub open spec fn chunk_count_spec(len: u64, chunk_size: u64) -> u64
    recommends chunk_size > 0
{
    (len / chunk_size) as u64
}

// r[impl slice.chunk-count]
/// Returns the number of full chunks of `chunk_size` that fit in `len`.
pub fn chunk_count(len: u64, chunk_size: u64) -> (result: u64)
    requires chunk_size > 0
    ensures result == chunk_count_spec(len, chunk_size)
{
    len / chunk_size
}

// r[verify slice.chunk-count]
/// Product of chunk_count and chunk_size never exceeds len.
proof fn chunk_count_bounded(len: u64, chunk_size: u64)
    requires chunk_size > 0
    ensures chunk_count_spec(len, chunk_size) as int * chunk_size as int <= len as int
{
    let n = len as int;
    let d = chunk_size as int;
    vstd::arithmetic::div_mod::lemma_fundamental_div_mod(n, d);
    vstd::arithmetic::div_mod::lemma_mod_pos_bound(n, d);
    // n == d * (n/d) + n%d, 0 <= n%d < d, so d * (n/d) <= n
    assert(d * (n / d) <= n);
    assert((n / d) * d == d * (n / d)) by(nonlinear_arith);
    // (n/d) * d == d * (n/d) <= n
}

// r[verify slice.chunk-count]
/// Remainder after chunking is strictly less than chunk_size.
proof fn chunk_count_remainder_lt(len: u64, chunk_size: u64)
    requires chunk_size > 0
    ensures
        (len as int - chunk_count_spec(len, chunk_size) as int * chunk_size as int)
            < chunk_size as int
{
    let n = len as int;
    let d = chunk_size as int;
    vstd::arithmetic::div_mod::lemma_fundamental_div_mod(n, d);
    vstd::arithmetic::div_mod::lemma_mod_pos_bound(n, d);
    assert((n / d) * d == d * (n / d)) by(nonlinear_arith);
    // n == d * (n/d) + n%d, 0 <= n%d < d
    // (n/d) * d == d * (n/d)
    // n - (n/d) * d == n%d < d
}

// --- prefix_eq ---

// r[depends slice.prefix-eq]
/// Spec: first `n` elements of both sequences are equal.
pub open spec fn prefix_eq_spec(a: Seq<u64>, b: Seq<u64>, n: int) -> bool {
    n >= 0 && n <= a.len() && n <= b.len()
    && forall|i: int| 0 <= i < n ==> a[i] == b[i]
}

// r[impl slice.prefix-eq]
/// Returns true if the first `n` elements of `a` and `b` are equal.
pub fn prefix_eq(a: &[u64], b: &[u64], n: usize) -> (result: bool)
    ensures result == prefix_eq_spec(a@, b@, n as int)
{
    if n > a.len() || n > b.len() {
        return false;
    }
    let mut i: usize = 0;
    while i < n
        invariant
            0 <= i <= n,
            n <= a@.len(),
            n <= b@.len(),
            forall|k: int| 0 <= k < i as int ==> a@[k] == b@[k],
        decreases n - i
    {
        if a[i] != b[i] {
            assert(a@[i as int] != b@[i as int]);
            return false;
        }
        i = i + 1;
    }
    true
}

// r[verify slice.prefix-eq]
/// Prefix of length zero always matches.
proof fn prefix_eq_zero(a: Seq<u64>, b: Seq<u64>)
    ensures prefix_eq_spec(a, b, 0)
{}

// r[verify slice.prefix-eq]
/// Any sequence matches itself for any valid prefix length.
proof fn prefix_eq_reflexive(a: Seq<u64>, n: int)
    requires 0 <= n <= a.len()
    ensures prefix_eq_spec(a, a, n)
{}

// --- suffix_eq ---

// r[depends slice.suffix-eq]
/// Spec: last `n` elements of both sequences are equal.
pub open spec fn suffix_eq_spec(a: Seq<u64>, b: Seq<u64>, n: int) -> bool {
    n >= 0 && n <= a.len() && n <= b.len()
    && forall|i: int| #![trigger a[a.len() - n + i], b[b.len() - n + i]]
        0 <= i < n ==> a[a.len() - n + i] == b[b.len() - n + i]
}

// r[impl slice.suffix-eq]
/// Returns true if the last `n` elements of `a` and `b` are equal.
pub fn suffix_eq(a: &[u64], b: &[u64], n: usize) -> (result: bool)
    ensures result == suffix_eq_spec(a@, b@, n as int)
{
    if n > a.len() || n > b.len() {
        return false;
    }
    let a_start = a.len() - n;
    let b_start = b.len() - n;
    let mut i: usize = 0;
    while i < n
        invariant
            0 <= i <= n,
            n <= a@.len(),
            n <= b@.len(),
            a_start + n == a.len(),
            b_start + n == b.len(),
            forall|k: int|
                #![trigger a@[a@.len() - n as int + k], b@[b@.len() - n as int + k]]
                0 <= k < i as int
                ==> a@[a@.len() - n as int + k] == b@[b@.len() - n as int + k],
        decreases n - i
    {
        if a[a_start + i] != b[b_start + i] {
            assert(
                a@[a@.len() - n as int + i as int]
                    != b@[b@.len() - n as int + i as int]
            );
            return false;
        }
        i = i + 1;
    }
    true
}

// r[verify slice.suffix-eq]
/// Suffix of length zero always matches.
proof fn suffix_eq_zero(a: Seq<u64>, b: Seq<u64>)
    ensures suffix_eq_spec(a, b, 0)
{}

// r[verify slice.suffix-eq]
/// Any sequence matches itself for any valid suffix length.
proof fn suffix_eq_reflexive(a: Seq<u64>, n: int)
    requires 0 <= n <= a.len()
    ensures suffix_eq_spec(a, a, n)
{}

// --- all_equal ---

// r[depends slice.all-equal]
/// Spec: every element equals the given value.
pub open spec fn all_equal_spec(s: Seq<u64>, value: u64) -> bool {
    forall|i: int| 0 <= i < s.len() ==> s[i] == value
}

// r[impl slice.all-equal]
/// Returns true if every element in the slice equals `value`.
pub fn all_equal(slice: &[u64], value: u64) -> (result: bool)
    ensures result == all_equal_spec(slice@, value)
{
    let mut i: usize = 0;
    while i < slice.len()
        invariant
            0 <= i <= slice@.len(),
            forall|k: int| 0 <= k < i as int ==> slice@[k] == value,
        decreases slice@.len() - i
    {
        if slice[i] != value {
            assert(slice@[i as int] != value);
            return false;
        }
        i = i + 1;
    }
    true
}

// r[verify slice.all-equal]
/// Empty slice is trivially all-equal.
proof fn all_equal_empty(value: u64)
    ensures all_equal_spec(Seq::<u64>::empty(), value)
{}

// r[verify slice.all-equal]
/// If all elements equal `value`, any two elements are equal.
proof fn all_equal_pairwise(s: Seq<u64>, value: u64)
    requires all_equal_spec(s, value)
    ensures forall|i: int, j: int|
        0 <= i < s.len() && 0 <= j < s.len() ==> s[i] == s[j]
{}

// --- mismatch_index ---

/// Minimum of two sequence lengths.
pub open spec fn min_len(a: Seq<u64>, b: Seq<u64>) -> int {
    if a.len() <= b.len() { a.len() as int } else { b.len() as int }
}

// r[depends slice.mismatch-index]
/// Spec: index of first position where sequences differ, or -1 if all match.
pub open spec fn mismatch_index_spec(a: Seq<u64>, b: Seq<u64>, from: int) -> int
    decreases min_len(a, b) - from
{
    if from >= min_len(a, b) {
        -1int
    } else if a[from] != b[from] {
        from
    } else {
        mismatch_index_spec(a, b, from + 1)
    }
}

// r[impl slice.mismatch-index]
/// Returns the first index where `a` and `b` differ, comparing up to
/// the shorter length.
pub fn mismatch_index(a: &[u64], b: &[u64]) -> (result: Option<usize>)
    ensures
        result.is_some() ==> {
            let idx = result.unwrap() as int;
            &&& 0 <= idx < a@.len()
            &&& 0 <= idx < b@.len()
            &&& a@[idx] != b@[idx]
            &&& forall|k: int| 0 <= k < idx ==> a@[k] == b@[k]
        },
        result.is_none() ==> forall|k: int|
            0 <= k < min_len(a@, b@) ==> a@[k] == b@[k],
{
    let n = if a.len() <= b.len() { a.len() } else { b.len() };
    let mut i: usize = 0;
    while i < n
        invariant
            0 <= i <= n,
            n as int == min_len(a@, b@),
            forall|k: int| 0 <= k < i as int ==> a@[k] == b@[k],
        decreases n - i
    {
        if a[i] != b[i] {
            return Some(i);
        }
        i = i + 1;
    }
    None
}

// r[verify slice.mismatch-index]
/// Two empty sequences have no mismatch.
proof fn mismatch_empty()
    ensures mismatch_index_spec(
        Seq::<u64>::empty(), Seq::<u64>::empty(), 0
    ) == -1int
{}

// r[verify slice.mismatch-index]
/// No mismatch implies prefix equality up to min length.
proof fn mismatch_none_implies_prefix_eq(a: Seq<u64>, b: Seq<u64>)
    requires mismatch_index_spec(a, b, 0) < 0
    ensures prefix_eq_spec(a, b, min_len(a, b))
{
    mismatch_none_inductive(a, b, 0);
}

/// Helper: inductive proof that spec returning -1 means element-wise equality.
proof fn mismatch_none_inductive(a: Seq<u64>, b: Seq<u64>, from: int)
    requires
        0 <= from,
        mismatch_index_spec(a, b, from) < 0,
    ensures forall|i: int| from <= i < min_len(a, b) ==> a[i] == b[i]
    decreases min_len(a, b) - from
{
    if from < min_len(a, b) {
        mismatch_none_inductive(a, b, from + 1);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify slice.chunk-count]
    #[test]
    fn test_chunk_count() {
        assert_eq!(chunk_count(10, 3), 3);
        assert_eq!(chunk_count(9, 3), 3);
        assert_eq!(chunk_count(0, 5), 0);
        assert_eq!(chunk_count(4, 5), 0);
        assert_eq!(chunk_count(5, 5), 1);
        assert_eq!(chunk_count(100, 10), 10);
        assert_eq!(chunk_count(u64::MAX, 1), u64::MAX);
    }

    // r[verify slice.prefix-eq]
    #[test]
    fn test_prefix_eq() {
        assert!(prefix_eq(&[1, 2, 3], &[1, 2, 4], 2));
        assert!(!prefix_eq(&[1, 2, 3], &[1, 2, 4], 3));
        assert!(prefix_eq(&[1, 2], &[1, 2, 3], 2));
        assert!(prefix_eq(&[], &[], 0));
        assert!(!prefix_eq(&[1], &[2], 1));
        assert!(!prefix_eq(&[1, 2], &[1], 2)); // n > b.len()
        assert!(prefix_eq(&[5, 5, 5], &[5, 5, 5], 3));
    }

    // r[verify slice.suffix-eq]
    #[test]
    fn test_suffix_eq() {
        assert!(suffix_eq(&[1, 2, 3], &[4, 2, 3], 2));
        assert!(!suffix_eq(&[1, 2, 3], &[4, 2, 3], 3));
        assert!(suffix_eq(&[2, 3], &[1, 2, 3], 2));
        assert!(suffix_eq(&[], &[], 0));
        assert!(!suffix_eq(&[1], &[2], 1));
        assert!(!suffix_eq(&[1, 2], &[1], 2)); // n > b.len()
        assert!(suffix_eq(&[5, 5, 5], &[5, 5, 5], 3));
    }

    // r[verify slice.all-equal]
    #[test]
    fn test_all_equal() {
        assert!(all_equal(&[], 5));
        assert!(all_equal(&[5, 5, 5], 5));
        assert!(!all_equal(&[5, 5, 3], 5));
        assert!(all_equal(&[0], 0));
        assert!(!all_equal(&[1], 0));
        assert!(all_equal(&[7], 7));
    }

    // r[verify slice.mismatch-index]
    #[test]
    fn test_mismatch_index() {
        assert_eq!(mismatch_index(&[], &[]), None);
        assert_eq!(mismatch_index(&[1, 2, 3], &[1, 2, 3]), None);
        assert_eq!(mismatch_index(&[1, 2, 3], &[1, 2, 4]), Some(2));
        assert_eq!(mismatch_index(&[1, 2], &[1, 2, 3]), None);
        assert_eq!(mismatch_index(&[1, 3], &[1, 2, 3]), Some(1));
        assert_eq!(mismatch_index(&[2], &[1]), Some(0));
        assert_eq!(mismatch_index(&[], &[1, 2]), None);
    }

    proptest! {
        // r[verify slice.chunk-count]
        #[test]
        fn prop_chunk_count_bounded(len: u64, chunk_size in 1u64..=10000) {
            let count = chunk_count(len, chunk_size);
            let product = u128::from(count) * u128::from(chunk_size);
            prop_assert!(product <= u128::from(len));
        }

        // r[verify slice.chunk-count]
        #[test]
        fn prop_chunk_count_remainder(len: u64, chunk_size in 1u64..=10000) {
            let count = chunk_count(len, chunk_size);
            let remainder = len - count * chunk_size;
            prop_assert!(remainder < chunk_size);
        }

        // r[verify slice.prefix-eq]
        #[test]
        fn prop_prefix_eq_reflexive(
            s in proptest::collection::vec(any::<u64>(), 0..=50),
        ) {
            for n in 0..=s.len() {
                prop_assert!(prefix_eq(&s, &s, n));
            }
        }

        // r[verify slice.prefix-eq]
        #[test]
        fn prop_prefix_eq_zero(
            a in proptest::collection::vec(any::<u64>(), 0..=50),
            b in proptest::collection::vec(any::<u64>(), 0..=50),
        ) {
            prop_assert!(prefix_eq(&a, &b, 0));
        }

        // r[verify slice.suffix-eq]
        #[test]
        fn prop_suffix_eq_reflexive(
            s in proptest::collection::vec(any::<u64>(), 0..=50),
        ) {
            for n in 0..=s.len() {
                prop_assert!(suffix_eq(&s, &s, n));
            }
        }

        // r[verify slice.suffix-eq]
        #[test]
        fn prop_suffix_eq_zero(
            a in proptest::collection::vec(any::<u64>(), 0..=50),
            b in proptest::collection::vec(any::<u64>(), 0..=50),
        ) {
            prop_assert!(suffix_eq(&a, &b, 0));
        }

        // r[verify slice.all-equal]
        #[test]
        fn prop_all_equal_uniform(value: u64, len in 0usize..=50) {
            let v: Vec<u64> = vec![value; len];
            prop_assert!(all_equal(&v, value));
        }

        // r[verify slice.all-equal]
        #[test]
        fn prop_all_equal_single_mismatch(value: u64, other: u64, len in 2usize..=50) {
            if value != other {
                let mut v: Vec<u64> = vec![value; len];
                v[len / 2] = other;
                prop_assert!(!all_equal(&v, value));
            }
        }

        // r[verify slice.mismatch-index]
        #[test]
        fn prop_mismatch_none_means_equal(
            a in proptest::collection::vec(any::<u64>(), 0..=50),
            b in proptest::collection::vec(any::<u64>(), 0..=50),
        ) {
            if mismatch_index(&a, &b).is_none() {
                let n = a.len().min(b.len());
                prop_assert_eq!(&a[..n], &b[..n]);
            }
        }

        // r[verify slice.mismatch-index]
        #[test]
        fn prop_mismatch_some_is_first(
            a in proptest::collection::vec(0u64..10, 0..=50),
            b in proptest::collection::vec(0u64..10, 0..=50),
        ) {
            if let Some(idx) = mismatch_index(&a, &b) {
                prop_assert_ne!(a[idx], b[idx]);
                for i in 0..idx {
                    prop_assert_eq!(a[i], b[i]);
                }
            }
        }
    }
}
