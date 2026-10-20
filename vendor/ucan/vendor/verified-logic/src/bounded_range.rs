//! Verified bounded range operations with optional upper bounds.
//!
//! Models ranges where the upper bound can be infinite. When `has_upper`
//! is false, the range extends to infinity. When true, it follows half-open
//! interval semantics `[lower, upper)`. This pattern matches key ranges
//! where an empty `end_key` means 'all keys from start onwards'.

use vstd::prelude::*;

verus! {

// --- contains ---

// r[depends bounded-range.contains]
/// Spec: x is in the range [lower, upper) or [lower, ∞).
pub open spec fn contains_spec(lower: u64, upper: u64, has_upper: bool, x: u64) -> bool {
    x >= lower && (!has_upper || x < upper)
}

// r[impl bounded-range.contains]
/// Returns true if `x` is in the range.
/// When `has_upper` is true: `[lower, upper)` (half-open).
/// When `has_upper` is false: `[lower, ∞)` (unbounded above).
pub fn contains(lower: u64, upper: u64, has_upper: bool, x: u64) -> (result: bool)
    ensures result == contains_spec(lower, upper, has_upper, x)
{
    x >= lower && (!has_upper || x < upper)
}

// r[verify bounded-range.contains]
/// Unbounded range contains all x >= lower.
proof fn unbounded_contains_all_ge_lower(lower: u64, upper: u64, x: u64)
    requires x >= lower
    ensures contains_spec(lower, upper, false, x)
{}

// r[verify bounded-range.contains]
/// Bounded range follows interval semantics.
proof fn bounded_follows_interval(lower: u64, upper: u64, x: u64)
    ensures contains_spec(lower, upper, true, x) ==
        (x >= lower && x < upper)
{}

// r[verify bounded-range.contains]
/// Empty bounded range contains nothing.
proof fn empty_bounded_contains_nothing(lower: u64, upper: u64, x: u64)
    requires lower >= upper
    ensures !contains_spec(lower, upper, true, x)
{}

// --- is_empty ---

// r[depends bounded-range.is-empty]
/// Spec: range has no elements.
pub open spec fn is_empty_spec(lower: u64, upper: u64, has_upper: bool) -> bool {
    has_upper && lower >= upper
}

// r[impl bounded-range.is-empty]
/// Returns true if the range is empty.
/// Unbounded ranges are never empty.
/// Bounded ranges are empty when lower >= upper.
pub fn is_empty(lower: u64, upper: u64, has_upper: bool) -> (result: bool)
    ensures result == is_empty_spec(lower, upper, has_upper)
{
    has_upper && lower >= upper
}

// r[verify bounded-range.is-empty]
/// Unbounded range is never empty.
proof fn unbounded_never_empty(lower: u64, upper: u64)
    ensures !is_empty_spec(lower, upper, false)
{}

// r[verify bounded-range.is-empty]
/// Empty range contains nothing.
proof fn empty_contains_nothing(lower: u64, upper: u64, has_upper: bool, x: u64)
    requires is_empty_spec(lower, upper, has_upper)
    ensures !contains_spec(lower, upper, has_upper, x)
{}

// r[verify bounded-range.contains]
// r[verify bounded-range.is-empty]
/// Bounded contains is subset of unbounded contains (same lower).
proof fn bounded_subset_unbounded(lower: u64, upper: u64, x: u64)
    requires contains_spec(lower, upper, true, x)
    ensures contains_spec(lower, upper, false, x)
{}

// --- overlaps ---

// r[depends bounded-range.overlaps]
/// Spec: two ranges share at least one point.
pub open spec fn overlaps_spec(
    a_lo: u64, a_hi: u64, a_has_hi: bool,
    b_lo: u64, b_hi: u64, b_has_hi: bool,
) -> bool {
    !is_empty_spec(a_lo, a_hi, a_has_hi)
    && !is_empty_spec(b_lo, b_hi, b_has_hi)
    && a_lo < effective_hi_spec(b_lo, b_hi, b_has_hi)
    && b_lo < effective_hi_spec(a_lo, a_hi, a_has_hi)
}

// Helper spec for effective upper bound
pub open spec fn effective_hi_spec(lo: u64, hi: u64, has_hi: bool) -> u64 {
    if has_hi { hi } else { u64::MAX }
}

// r[impl bounded-range.overlaps]
/// Returns true if the ranges overlap.
pub fn overlaps(
    a_lo: u64, a_hi: u64, a_has_hi: bool,
    b_lo: u64, b_hi: u64, b_has_hi: bool,
) -> (result: bool)
    ensures result == overlaps_spec(a_lo, a_hi, a_has_hi, b_lo, b_hi, b_has_hi)
{
    !is_empty(a_lo, a_hi, a_has_hi)
    && !is_empty(b_lo, b_hi, b_has_hi)
    && a_lo < effective_hi(b_lo, b_hi, b_has_hi)
    && b_lo < effective_hi(a_lo, a_hi, a_has_hi)
}

fn effective_hi(_lo: u64, hi: u64, has_hi: bool) -> (result: u64)
    ensures result == effective_hi_spec(_lo, hi, has_hi)
{
    if has_hi { hi } else { u64::MAX }
}

// r[verify bounded-range.overlaps]
/// Overlaps is symmetric.
proof fn overlaps_symmetric(
    a_lo: u64, a_hi: u64, a_has_hi: bool,
    b_lo: u64, b_hi: u64, b_has_hi: bool,
)
    ensures overlaps_spec(a_lo, a_hi, a_has_hi, b_lo, b_hi, b_has_hi)
        == overlaps_spec(b_lo, b_hi, b_has_hi, a_lo, a_hi, a_has_hi)
{}

// r[verify bounded-range.overlaps]
/// Empty range never overlaps.
proof fn empty_no_overlap(
    a_lo: u64, a_hi: u64, a_has_hi: bool,
    b_lo: u64, b_hi: u64, b_has_hi: bool,
)
    requires is_empty_spec(a_lo, a_hi, a_has_hi)
    ensures !overlaps_spec(a_lo, a_hi, a_has_hi, b_lo, b_hi, b_has_hi)
{}

// r[verify bounded-range.overlaps]
/// Two unbounded ranges always overlap (unless one starts at u64::MAX).
proof fn unbounded_ranges_overlap(a_lo: u64, b_lo: u64, a_hi: u64, b_hi: u64)
    requires a_lo < u64::MAX, b_lo < u64::MAX
    ensures overlaps_spec(a_lo, a_hi, false, b_lo, b_hi, false)
{}

// --- is_bounded ---

// r[depends bounded-range.is-bounded]
/// Spec: range has a finite upper bound and is non-empty.
pub open spec fn is_bounded_spec(lower: u64, upper: u64, has_upper: bool) -> bool {
    has_upper && lower < upper
}

// r[impl bounded-range.is-bounded]
/// Returns true if the range has a finite upper bound and is non-empty.
pub fn is_bounded(lower: u64, upper: u64, has_upper: bool) -> (result: bool)
    ensures result == is_bounded_spec(lower, upper, has_upper)
{
    has_upper && lower < upper
}

// r[verify bounded-range.is-bounded]
/// Bounded implies non-empty.
proof fn bounded_implies_nonempty(lower: u64, upper: u64, has_upper: bool)
    requires is_bounded_spec(lower, upper, has_upper)
    ensures !is_empty_spec(lower, upper, has_upper)
{}

// --- is_unbounded ---

// r[depends bounded-range.is-bounded]
/// Spec: range extends to infinity.
pub open spec fn is_unbounded_spec(has_upper: bool) -> bool {
    !has_upper
}

// r[impl bounded-range.is-bounded]
/// Returns true if the range extends to infinity.
pub fn is_unbounded(has_upper: bool) -> (result: bool)
    ensures result == is_unbounded_spec(has_upper)
{
    !has_upper
}

// --- restrict ---

// r[depends bounded-range.restrict]
/// Spec: restrict range by adding/tightening upper bound.
pub open spec fn restrict_spec(
    lower: u64, upper: u64, has_upper: bool, new_upper: u64,
) -> (u64, u64, bool)
    recommends !has_upper || new_upper <= upper
{
    if has_upper {
        if new_upper <= upper {
            (lower, new_upper, true)
        } else {
            (lower, upper, true)
        }
    } else {
        (lower, new_upper, true)
    }
}

// r[impl bounded-range.restrict]
/// Restricts a range by adding or tightening the upper bound.
/// For unbounded ranges: adds `new_upper` as the upper bound.
/// For bounded ranges: uses min(upper, `new_upper`).
pub fn restrict(
    lower: u64, upper: u64, has_upper: bool, new_upper: u64,
) -> (result: (u64, u64, bool))
    requires !has_upper || new_upper <= upper
    ensures result == restrict_spec(lower, upper, has_upper, new_upper)
{
    if has_upper {
        if new_upper <= upper {
            (lower, new_upper, true)
        } else {
            (lower, upper, true)
        }
    } else {
        (lower, new_upper, true)
    }
}

// r[verify bounded-range.restrict]
/// Restrict always produces a bounded range.
proof fn restrict_makes_bounded(
    lower: u64, upper: u64, has_upper: bool, new_upper: u64,
)
    requires !has_upper || new_upper <= upper, lower < new_upper
    ensures
        is_bounded_spec(
            restrict_spec(lower, upper, has_upper, new_upper).0,
            restrict_spec(lower, upper, has_upper, new_upper).1,
            restrict_spec(lower, upper, has_upper, new_upper).2,
        )
{}

// r[verify bounded-range.restrict]
/// Restrict preserves lower bound.
proof fn restrict_preserves_lower(
    lower: u64, upper: u64, has_upper: bool, new_upper: u64,
)
    requires !has_upper || new_upper <= upper
    ensures restrict_spec(lower, upper, has_upper, new_upper).0 == lower
{}

// r[verify bounded-range.restrict]
/// Restricted range is subset of original.
proof fn restrict_is_subset(
    lower: u64, upper: u64, has_upper: bool, new_upper: u64, x: u64,
)
    requires
        !has_upper || new_upper <= upper,
        contains_spec(
            restrict_spec(lower, upper, has_upper, new_upper).0,
            restrict_spec(lower, upper, has_upper, new_upper).1,
            restrict_spec(lower, upper, has_upper, new_upper).2,
            x,
        ),
    ensures contains_spec(lower, upper, has_upper, x)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify bounded-range.contains]
    #[test]
    fn test_contains() {
        // Bounded range
        assert!(contains(10, 20, true, 10));
        assert!(contains(10, 20, true, 15));
        assert!(contains(10, 20, true, 19));
        assert!(!contains(10, 20, true, 20)); // half-open
        assert!(!contains(10, 20, true, 9));

        // Unbounded range
        assert!(contains(10, 20, false, 10));
        assert!(contains(10, 20, false, 20)); // no upper limit
        assert!(contains(10, 20, false, u64::MAX));
        assert!(!contains(10, 20, false, 9));

        // Empty bounded range
        assert!(!contains(10, 10, true, 10));
        assert!(!contains(20, 10, true, 15));
    }

    // r[verify bounded-range.is-empty]
    #[test]
    fn test_is_empty() {
        // Bounded ranges
        assert!(is_empty(10, 10, true)); // equal bounds
        assert!(is_empty(20, 10, true)); // inverted bounds
        assert!(!is_empty(10, 20, true)); // normal range

        // Unbounded ranges are never empty
        assert!(!is_empty(10, 10, false));
        assert!(!is_empty(20, 10, false));
        assert!(!is_empty(10, 20, false));
    }

    // r[verify bounded-range.overlaps]
    #[test]
    fn test_overlaps() {
        // Both bounded
        assert!(overlaps(10, 20, true, 15, 25, true)); // partial overlap
        assert!(overlaps(10, 20, true, 10, 20, true)); // identical
        assert!(overlaps(10, 20, true, 12, 18, true)); // contained
        assert!(!overlaps(10, 20, true, 20, 30, true)); // adjacent
        assert!(!overlaps(10, 20, true, 25, 30, true)); // disjoint

        // Both unbounded
        assert!(overlaps(10, 100, false, 20, 200, false)); // different lowers
        assert!(overlaps(10, 100, false, 10, 200, false)); // same lower

        // Mixed bounded/unbounded
        assert!(overlaps(10, 20, true, 15, 100, false)); // bounded overlaps unbounded
        assert!(overlaps(15, 100, false, 10, 20, true)); // unbounded overlaps bounded
        assert!(!overlaps(10, 20, true, 25, 100, false)); // no overlap

        // Empty ranges
        assert!(!overlaps(10, 10, true, 15, 25, true)); // empty first
        assert!(!overlaps(15, 25, true, 10, 10, true)); // empty second
    }

    // r[verify bounded-range.is-bounded]
    #[test]
    fn test_is_bounded() {
        assert!(is_bounded(10, 20, true)); // normal bounded range
        assert!(!is_bounded(10, 10, true)); // empty range
        assert!(!is_bounded(20, 10, true)); // inverted range
        assert!(!is_bounded(10, 20, false)); // unbounded
    }

    // r[verify bounded-range.is-bounded]
    #[test]
    fn test_is_unbounded() {
        assert!(is_unbounded(false));
        assert!(!is_unbounded(true));
    }

    // r[verify bounded-range.restrict]
    #[test]
    fn test_restrict() {
        // Restrict unbounded range
        let (lo, hi, has_hi) = restrict(10, 100, false, 50);
        assert_eq!((lo, hi, has_hi), (10, 50, true));

        // Tighten bounded range
        let (lo, hi, has_hi) = restrict(10, 100, true, 50);
        assert_eq!((lo, hi, has_hi), (10, 50, true));

        // Attempt to widen (should keep original)
        let (lo, hi, has_hi) = restrict(10, 100, true, 100);
        assert_eq!((lo, hi, has_hi), (10, 100, true));
    }

    proptest! {
        // r[verify bounded-range.contains]
        #[test]
        fn prop_unbounded_contains_all_ge(lower: u64, upper: u64, x: u64) {
            if x >= lower {
                prop_assert!(contains(lower, upper, false, x));
            }
        }

        // r[verify bounded-range.contains]
        #[test]
        fn prop_bounded_follows_interval(lower: u64, upper: u64, x: u64) {
            let bounded = contains(lower, upper, true, x);
            let interval = x >= lower && x < upper;
            prop_assert_eq!(bounded, interval);
        }

        // r[verify bounded-range.is-empty]
        #[test]
        fn prop_unbounded_never_empty(lower: u64, upper: u64) {
            prop_assert!(!is_empty(lower, upper, false));
        }

        // r[verify bounded-range.is-empty]
        #[test]
        fn prop_empty_contains_nothing(lower: u64, upper: u64, has_upper: bool, x: u64) {
            if is_empty(lower, upper, has_upper) {
                prop_assert!(!contains(lower, upper, has_upper, x));
            }
        }

        // r[verify bounded-range.overlaps]
        #[test]
        fn prop_overlaps_symmetric(
            a_lo: u64, a_hi: u64, a_has_hi: bool,
            b_lo: u64, b_hi: u64, b_has_hi: bool,
        ) {
            prop_assert_eq!(
                overlaps(a_lo, a_hi, a_has_hi, b_lo, b_hi, b_has_hi),
                overlaps(b_lo, b_hi, b_has_hi, a_lo, a_hi, a_has_hi),
            );
        }

        // r[verify bounded-range.overlaps]
        #[test]
        fn prop_empty_no_overlap(
            a_lo: u64, a_hi: u64,
            b_lo: u64, b_hi: u64, b_has_hi: bool,
        ) {
            if is_empty(a_lo, a_hi, true) {
                prop_assert!(!overlaps(a_lo, a_hi, true, b_lo, b_hi, b_has_hi));
            }
        }

        // r[verify bounded-range.overlaps]
        #[test]
        fn prop_unbounded_overlap(a_lo in 0u64..u64::MAX, b_lo in 0u64..u64::MAX, a_hi: u64, b_hi: u64) {
            prop_assert!(overlaps(a_lo, a_hi, false, b_lo, b_hi, false));
        }

        // r[verify bounded-range.is-bounded]
        #[test]
        fn prop_bounded_implies_nonempty(lower: u64, upper: u64, has_upper: bool) {
            if is_bounded(lower, upper, has_upper) {
                prop_assert!(!is_empty(lower, upper, has_upper));
            }
        }

        // r[verify bounded-range.restrict]
        #[test]
        fn prop_restrict_preserves_lower(
            lower: u64, upper: u64, has_upper: bool, new_upper: u64,
        ) {
            if !has_upper || new_upper <= upper {
                let (result_lower, _, _) = restrict(lower, upper, has_upper, new_upper);
                prop_assert_eq!(result_lower, lower);
            }
        }

        // r[verify bounded-range.restrict]
        #[test]
        fn prop_restrict_makes_bounded(
            lower: u64, upper: u64, has_upper: bool, new_upper: u64,
        ) {
            if (!has_upper || new_upper <= upper) && lower < new_upper {
                let (_, _, result_has_upper) = restrict(lower, upper, has_upper, new_upper);
                prop_assert!(result_has_upper);
            }
        }

        // r[verify bounded-range.contains]
        // r[verify bounded-range.is-empty]
        #[test]
        fn prop_bounded_subset_unbounded(lower: u64, upper: u64, x: u64) {
            if contains(lower, upper, true, x) {
                prop_assert!(contains(lower, upper, false, x));
            }
        }

        // r[verify bounded-range.restrict]
        #[test]
        fn prop_restrict_is_subset(
            lower: u64, upper: u64, has_upper: bool, new_upper: u64, x: u64,
        ) {
            if !has_upper || new_upper <= upper {
                let (r_lo, r_hi, r_has_hi) = restrict(lower, upper, has_upper, new_upper);
                if contains(r_lo, r_hi, r_has_hi, x) {
                    prop_assert!(contains(lower, upper, has_upper, x));
                }
            }
        }
    }
}
