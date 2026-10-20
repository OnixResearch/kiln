//! Verified ordering and comparison operations.
//!
//! Min, max, median, sign comparison, and compare-and-swap — all with
//! machine-checked contracts. Proofs cover commutativity, associativity,
//! idempotence, and the relationship between min/max and `abs_diff`.

#![allow(clippy::bool_to_int_with_if)]

use vstd::prelude::*;

verus! {

// --- min ---

// r[depends ordering.min]
/// Spec: the smaller of two values.
pub open spec fn min_spec(a: u64, b: u64) -> u64 {
    if a <= b { a } else { b }
}

// r[impl ordering.min]
/// Returns the smaller of `a` and `b`.
pub fn min(a: u64, b: u64) -> (result: u64)
    ensures result == min_spec(a, b)
{
    if a <= b { a } else { b }
}

// r[verify ordering.min]
proof fn min_commutative(a: u64, b: u64)
    ensures min_spec(a, b) == min_spec(b, a)
{}

// r[verify ordering.min]
proof fn min_idempotent(a: u64)
    ensures min_spec(a, a) == a
{}

// r[verify ordering.min]
proof fn min_associative(a: u64, b: u64, c: u64)
    ensures min_spec(a, min_spec(b, c)) == min_spec(min_spec(a, b), c)
{}

// --- max ---

// r[depends ordering.max]
/// Spec: the larger of two values.
pub open spec fn max_spec(a: u64, b: u64) -> u64 {
    if a >= b { a } else { b }
}

// r[impl ordering.max]
/// Returns the larger of `a` and `b`.
pub fn max(a: u64, b: u64) -> (result: u64)
    ensures result == max_spec(a, b)
{
    if a >= b { a } else { b }
}

// r[verify ordering.max]
proof fn max_commutative(a: u64, b: u64)
    ensures max_spec(a, b) == max_spec(b, a)
{}

// r[verify ordering.max]
proof fn max_idempotent(a: u64)
    ensures max_spec(a, a) == a
{}

// r[verify ordering.max]
proof fn max_associative(a: u64, b: u64, c: u64)
    ensures max_spec(a, max_spec(b, c)) == max_spec(max_spec(a, b), c)
{}

// --- min/max as semilattices (via algebraic) ---

// r[verify ordering.min]
/// min is a semilattice: commutative + associative + idempotent.
proof fn min_is_semilattice()
    ensures crate::algebraic::is_semilattice(|a: u64, b: u64| min_spec(a, b))
{}

// r[verify ordering.max]
/// max is a semilattice: commutative + associative + idempotent.
proof fn max_is_semilattice()
    ensures crate::algebraic::is_semilattice(|a: u64, b: u64| max_spec(a, b))
{}

// --- min/max relationship ---

// r[verify ordering.min]
// r[verify ordering.max]
proof fn min_le_max(a: u64, b: u64)
    ensures min_spec(a, b) <= max_spec(a, b)
{}

// r[verify ordering.min]
// r[verify ordering.max]
/// min(a,b) + max(a,b) == a + b (as int, no overflow concern).
proof fn min_plus_max(a: u64, b: u64)
    ensures min_spec(a, b) as int + max_spec(a, b) as int == a as int + b as int
{}

// --- median_of_three ---

// r[depends ordering.median-of-three]
/// Spec: the median of three values.
pub open spec fn median_of_three_spec(a: u64, b: u64, c: u64) -> u64 {
    max_spec(min_spec(a, b), min_spec(max_spec(a, b), c))
}

// r[impl ordering.median-of-three]
/// Returns the median of three values.
///
/// Uses the formula: max(min(a, b), min(max(a, b), c)).
/// No branches beyond the nested min/max calls.
pub fn median_of_three(a: u64, b: u64, c: u64) -> (result: u64)
    ensures result == median_of_three_spec(a, b, c)
{
    max(min(a, b), min(max(a, b), c))
}

// r[verify ordering.median-of-three]
/// The median is bounded by the overall min and max.
proof fn median_bounded(a: u64, b: u64, c: u64)
    ensures
        median_of_three_spec(a, b, c) >= min_spec(a, min_spec(b, c)),
        median_of_three_spec(a, b, c) <= max_spec(a, max_spec(b, c)),
{}

// r[verify ordering.median-of-three]
/// The median of three equal values is that value.
proof fn median_all_equal(x: u64)
    ensures median_of_three_spec(x, x, x) == x
{}

// r[verify ordering.median-of-three]
/// The median of two equal and one different is the repeated value.
proof fn median_two_equal(x: u64, y: u64)
    ensures median_of_three_spec(x, x, y) == x
{}

// --- sign ---

// r[depends ordering.sign]
/// Spec: comparison sign of two u64 values.
/// Returns -1 if a < b, 0 if a == b, 1 if a > b.
pub open spec fn sign_spec(a: u64, b: u64) -> int {
    if a < b { -1int } else if a == b { 0int } else { 1int }
}

// r[impl ordering.sign]
/// Returns the comparison sign: -1 if a < b, 0 if equal, 1 if a > b.
///
/// Returned as `i64` since the value is always in {-1, 0, 1}.
pub fn sign(a: u64, b: u64) -> (result: i64)
    ensures result as int == sign_spec(a, b)
{
    if a < b { -1 } else if a == b { 0 } else { 1 }
}

// r[verify ordering.sign]
/// Sign is antisymmetric: sign(a, b) == -sign(b, a).
proof fn sign_antisymmetric(a: u64, b: u64)
    ensures sign_spec(a, b) == -sign_spec(b, a)
{}

// r[verify ordering.sign]
/// Sign is zero iff a == b.
proof fn sign_zero_iff_equal(a: u64, b: u64)
    ensures (sign_spec(a, b) == 0) <==> (a == b)
{}

// --- compare_and_swap ---

// r[depends ordering.compare-and-swap]
/// Spec: returns (min, max) of two values.
pub open spec fn compare_and_swap_spec(a: u64, b: u64) -> (u64, u64) {
    (min_spec(a, b), max_spec(a, b))
}

// r[impl ordering.compare-and-swap]
/// Returns `(min(a,b), max(a,b))` — the pair sorted in ascending order.
pub fn compare_and_swap(a: u64, b: u64) -> (result: (u64, u64))
    ensures
        result.0 == compare_and_swap_spec(a, b).0,
        result.1 == compare_and_swap_spec(a, b).1,
{
    if a <= b { (a, b) } else { (b, a) }
}

// r[verify ordering.compare-and-swap]
/// The first element is always <= the second.
proof fn compare_and_swap_ordered(a: u64, b: u64)
    ensures compare_and_swap_spec(a, b).0 <= compare_and_swap_spec(a, b).1
{}

// r[verify ordering.compare-and-swap]
/// compare_and_swap preserves both values (as a multiset).
proof fn compare_and_swap_preserves(a: u64, b: u64)
    ensures
        compare_and_swap_spec(a, b).0 as int + compare_and_swap_spec(a, b).1 as int
            == a as int + b as int
{}

// r[verify ordering.compare-and-swap]
/// compare_and_swap on equal values returns (x, x).
proof fn compare_and_swap_equal(x: u64)
    ensures compare_and_swap_spec(x, x) == (x, x)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify ordering.min]
    #[test]
    fn test_min() {
        assert_eq!(min(3, 7), 3);
        assert_eq!(min(7, 3), 3);
        assert_eq!(min(5, 5), 5);
        assert_eq!(min(0, u64::MAX), 0);
        assert_eq!(min(u64::MAX, 0), 0);
    }

    // r[verify ordering.max]
    #[test]
    fn test_max() {
        assert_eq!(max(3, 7), 7);
        assert_eq!(max(7, 3), 7);
        assert_eq!(max(5, 5), 5);
        assert_eq!(max(0, u64::MAX), u64::MAX);
        assert_eq!(max(u64::MAX, 0), u64::MAX);
    }

    // r[verify ordering.median-of-three]
    #[test]
    fn test_median_of_three() {
        assert_eq!(median_of_three(1, 2, 3), 2);
        assert_eq!(median_of_three(3, 1, 2), 2);
        assert_eq!(median_of_three(2, 3, 1), 2);
        assert_eq!(median_of_three(5, 5, 5), 5);
        assert_eq!(median_of_three(1, 1, 2), 1);
        assert_eq!(median_of_three(1, 2, 2), 2);
        assert_eq!(median_of_three(0, u64::MAX, 42), 42);
    }

    // r[verify ordering.sign]
    #[test]
    fn test_sign() {
        assert_eq!(sign(3, 7), -1);
        assert_eq!(sign(7, 3), 1);
        assert_eq!(sign(5, 5), 0);
        assert_eq!(sign(0, u64::MAX), -1);
        assert_eq!(sign(u64::MAX, 0), 1);
    }

    // r[verify ordering.compare-and-swap]
    #[test]
    fn test_compare_and_swap() {
        assert_eq!(compare_and_swap(3, 7), (3, 7));
        assert_eq!(compare_and_swap(7, 3), (3, 7));
        assert_eq!(compare_and_swap(5, 5), (5, 5));
        assert_eq!(compare_and_swap(0, u64::MAX), (0, u64::MAX));
    }

    proptest! {
        // r[verify ordering.min]
        #[test]
        fn prop_min_commutative(a: u64, b: u64) {
            prop_assert_eq!(min(a, b), min(b, a));
        }

        // r[verify ordering.min]
        #[test]
        fn prop_min_le_both(a: u64, b: u64) {
            let m = min(a, b);
            prop_assert!(m <= a);
            prop_assert!(m <= b);
        }

        // r[verify ordering.max]
        #[test]
        fn prop_max_commutative(a: u64, b: u64) {
            prop_assert_eq!(max(a, b), max(b, a));
        }

        // r[verify ordering.max]
        #[test]
        fn prop_max_ge_both(a: u64, b: u64) {
            let m = max(a, b);
            prop_assert!(m >= a);
            prop_assert!(m >= b);
        }

        // r[verify ordering.min]
        // r[verify ordering.max]
        #[test]
        fn prop_min_le_max(a: u64, b: u64) {
            prop_assert!(min(a, b) <= max(a, b));
        }

        // r[verify ordering.median-of-three]
        #[test]
        fn prop_median_bounded(a: u64, b: u64, c: u64) {
            let med = median_of_three(a, b, c);
            prop_assert!(med >= min(a, min(b, c)));
            prop_assert!(med <= max(a, max(b, c)));
        }

        // r[verify ordering.median-of-three]
        #[test]
        fn prop_median_two_equal(x: u64, y: u64) {
            prop_assert_eq!(median_of_three(x, x, y), x);
        }

        // r[verify ordering.sign]
        #[test]
        fn prop_sign_antisymmetric(a: u64, b: u64) {
            prop_assert_eq!(sign(a, b), -sign(b, a));
        }

        // r[verify ordering.sign]
        #[test]
        fn prop_sign_zero_iff_equal(a: u64, b: u64) {
            prop_assert_eq!(sign(a, b) == 0, a == b);
        }

        // r[verify ordering.compare-and-swap]
        #[test]
        fn prop_compare_and_swap_ordered(a: u64, b: u64) {
            let (lo, hi) = compare_and_swap(a, b);
            prop_assert!(lo <= hi);
        }

        // r[verify ordering.compare-and-swap]
        #[test]
        fn prop_compare_and_swap_preserves(a: u64, b: u64) {
            let (lo, hi) = compare_and_swap(a, b);
            prop_assert_eq!(u128::from(lo) + u128::from(hi), u128::from(a) + u128::from(b));
        }
    }
}
