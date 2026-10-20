//! Verified half-open interval operations.
//!
//! Contains, overlap, split, adjacency, and merge for `[lo, hi)` intervals
//! over u64. Matches the semantics of aspen's `KeyRange` (half-open,
//! split produces non-overlapping coverage, merge is inverse of split).

#![allow(clippy::nonminimal_bool)]

use vstd::prelude::*;

verus! {

// --- contains ---

// r[depends interval.contains]
/// Spec: x is in the half-open interval [lo, hi).
pub open spec fn contains_spec(lo: u64, hi: u64, x: u64) -> bool {
    lo <= x && x < hi
}

// r[impl interval.contains]
/// Returns true if `x` is in `[lo, hi)`.
pub fn contains(lo: u64, hi: u64, x: u64) -> (result: bool)
    ensures result == contains_spec(lo, hi, x)
{
    lo <= x && x < hi
}

// r[verify interval.contains]
/// Empty intervals contain nothing.
proof fn empty_contains_nothing(lo: u64, hi: u64, x: u64)
    requires lo >= hi
    ensures !contains_spec(lo, hi, x)
{}

// r[verify interval.contains]
/// lo is always in a non-empty interval.
proof fn lo_in_nonempty(lo: u64, hi: u64)
    requires lo < hi
    ensures contains_spec(lo, hi, lo)
{}

// --- is_empty ---

// r[depends interval.is-empty]
/// Spec: interval has no elements.
pub open spec fn is_empty_spec(lo: u64, hi: u64) -> bool {
    lo >= hi
}

// r[impl interval.is-empty]
/// Returns true if `[lo, hi)` is empty.
pub fn is_empty(lo: u64, hi: u64) -> (result: bool)
    ensures result == is_empty_spec(lo, hi)
{
    lo >= hi
}

// r[verify interval.is-empty]
/// Non-empty iff lo < hi.
proof fn nonempty_iff_lt(lo: u64, hi: u64)
    ensures !is_empty_spec(lo, hi) <==> lo < hi
{}

// --- overlaps ---

// r[depends interval.overlaps]
/// Spec: two intervals share at least one point.
pub open spec fn overlaps_spec(a_lo: u64, a_hi: u64, b_lo: u64, b_hi: u64) -> bool {
    a_lo < a_hi && b_lo < b_hi && a_lo < b_hi && b_lo < a_hi
}

// r[impl interval.overlaps]
/// Returns true if `[a_lo, a_hi)` and `[b_lo, b_hi)` overlap.
pub fn overlaps(a_lo: u64, a_hi: u64, b_lo: u64, b_hi: u64) -> (result: bool)
    ensures result == overlaps_spec(a_lo, a_hi, b_lo, b_hi)
{
    a_lo < a_hi && b_lo < b_hi && a_lo < b_hi && b_lo < a_hi
}

// r[verify interval.overlaps]
/// Overlap is symmetric.
proof fn overlaps_symmetric(a_lo: u64, a_hi: u64, b_lo: u64, b_hi: u64)
    ensures overlaps_spec(a_lo, a_hi, b_lo, b_hi) == overlaps_spec(b_lo, b_hi, a_lo, a_hi)
{}

// r[verify interval.overlaps]
/// Empty intervals never overlap.
proof fn empty_no_overlap(a_lo: u64, a_hi: u64, b_lo: u64, b_hi: u64)
    requires a_lo >= a_hi
    ensures !overlaps_spec(a_lo, a_hi, b_lo, b_hi)
{}

// r[verify interval.overlaps]
/// A non-empty interval overlaps with itself.
proof fn self_overlap(lo: u64, hi: u64)
    requires lo < hi
    ensures overlaps_spec(lo, hi, lo, hi)
{}

// --- split ---

// r[depends interval.split]
/// Spec: left half of a split at mid.
pub open spec fn split_left_spec(lo: u64, hi: u64, mid: u64) -> (u64, u64)
    recommends lo < mid && mid < hi
{
    (lo, mid)
}

/// Spec: right half of a split at mid.
pub open spec fn split_right_spec(lo: u64, hi: u64, mid: u64) -> (u64, u64)
    recommends lo < mid && mid < hi
{
    (mid, hi)
}

// r[impl interval.split]
/// Splits `[lo, hi)` at `mid` into `[lo, mid)` and `[mid, hi)`.
pub fn split(lo: u64, hi: u64, mid: u64) -> (result: ((u64, u64), (u64, u64)))
    requires lo < mid, mid < hi
    ensures
        result.0 == split_left_spec(lo, hi, mid),
        result.1 == split_right_spec(lo, hi, mid),
{
    ((lo, mid), (mid, hi))
}

// r[verify interval.split]
/// Both halves are non-empty.
proof fn split_nonempty(lo: u64, hi: u64, mid: u64)
    requires lo < mid, mid < hi
    ensures
        !is_empty_spec(split_left_spec(lo, hi, mid).0, split_left_spec(lo, hi, mid).1),
        !is_empty_spec(split_right_spec(lo, hi, mid).0, split_right_spec(lo, hi, mid).1),
{}

// r[verify interval.split]
/// Halves don't overlap.
proof fn split_no_overlap(lo: u64, hi: u64, mid: u64)
    requires lo < mid, mid < hi
    ensures !overlaps_spec(
        split_left_spec(lo, hi, mid).0, split_left_spec(lo, hi, mid).1,
        split_right_spec(lo, hi, mid).0, split_right_spec(lo, hi, mid).1,
    )
{}

// r[verify interval.split]
/// Every point in the original is in exactly one half.
proof fn split_covers(lo: u64, hi: u64, mid: u64, x: u64)
    requires lo < mid, mid < hi, contains_spec(lo, hi, x)
    ensures
        contains_spec(lo, mid, x) || contains_spec(mid, hi, x),
        !(contains_spec(lo, mid, x) && contains_spec(mid, hi, x)),
{}

// --- adjacent ---

// r[depends interval.adjacent]
/// Spec: one interval ends exactly where the other begins.
pub open spec fn adjacent_spec(a_lo: u64, a_hi: u64, b_lo: u64, b_hi: u64) -> bool {
    (a_hi == b_lo && a_lo < a_hi && b_lo < b_hi)
    || (b_hi == a_lo && b_lo < b_hi && a_lo < a_hi)
}

// r[impl interval.adjacent]
/// Returns true if one interval ends where the other begins.
pub fn adjacent(a_lo: u64, a_hi: u64, b_lo: u64, b_hi: u64) -> (result: bool)
    ensures result == adjacent_spec(a_lo, a_hi, b_lo, b_hi)
{
    (a_hi == b_lo && a_lo < a_hi && b_lo < b_hi)
    || (b_hi == a_lo && b_lo < b_hi && a_lo < a_hi)
}

// r[verify interval.adjacent]
/// Adjacency is symmetric.
proof fn adjacent_symmetric(a_lo: u64, a_hi: u64, b_lo: u64, b_hi: u64)
    ensures adjacent_spec(a_lo, a_hi, b_lo, b_hi)
        == adjacent_spec(b_lo, b_hi, a_lo, a_hi)
{}

// r[verify interval.adjacent]
/// Adjacent intervals don't overlap.
proof fn adjacent_no_overlap(a_lo: u64, a_hi: u64, b_lo: u64, b_hi: u64)
    requires adjacent_spec(a_lo, a_hi, b_lo, b_hi)
    ensures !overlaps_spec(a_lo, a_hi, b_lo, b_hi)
{}

// r[verify interval.adjacent]
/// Split halves are adjacent.
proof fn split_is_adjacent(lo: u64, hi: u64, mid: u64)
    requires lo < mid, mid < hi
    ensures adjacent_spec(lo, mid, mid, hi)
{}

// --- merge ---

// r[depends interval.merge]
/// Spec: merge two adjacent intervals.
pub open spec fn merge_spec(
    a_lo: u64, a_hi: u64, b_lo: u64, b_hi: u64,
) -> (u64, u64)
    recommends adjacent_spec(a_lo, a_hi, b_lo, b_hi)
{
    if a_hi == b_lo {
        (a_lo, b_hi)
    } else {
        (b_lo, a_hi)
    }
}

// r[impl interval.merge]
/// Merges two adjacent intervals into one.
pub fn merge(a_lo: u64, a_hi: u64, b_lo: u64, b_hi: u64) -> (result: (u64, u64))
    requires adjacent_spec(a_lo, a_hi, b_lo, b_hi)
    ensures result == merge_spec(a_lo, a_hi, b_lo, b_hi)
{
    if a_hi == b_lo {
        (a_lo, b_hi)
    } else {
        (b_lo, a_hi)
    }
}

// r[verify interval.merge]
/// Merge of a split recovers the original.
proof fn split_merge_roundtrip(lo: u64, hi: u64, mid: u64)
    requires lo < mid, mid < hi
    ensures merge_spec(lo, mid, mid, hi) == (lo, hi)
{}

// r[verify interval.merge]
/// Merged interval contains everything from both inputs.
proof fn merge_contains_both(
    a_lo: u64, a_hi: u64, b_lo: u64, b_hi: u64, x: u64,
)
    requires
        adjacent_spec(a_lo, a_hi, b_lo, b_hi),
        a_hi == b_lo,
        contains_spec(a_lo, a_hi, x) || contains_spec(b_lo, b_hi, x),
    ensures contains_spec(
        merge_spec(a_lo, a_hi, b_lo, b_hi).0,
        merge_spec(a_lo, a_hi, b_lo, b_hi).1,
        x,
    )
{}

// r[verify interval.merge]
/// Merged interval is non-empty.
proof fn merge_nonempty(a_lo: u64, a_hi: u64, b_lo: u64, b_hi: u64)
    requires adjacent_spec(a_lo, a_hi, b_lo, b_hi)
    ensures !is_empty_spec(
        merge_spec(a_lo, a_hi, b_lo, b_hi).0,
        merge_spec(a_lo, a_hi, b_lo, b_hi).1,
    )
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify interval.contains]
    #[test]
    fn test_contains() {
        assert!(contains(0, 10, 0));
        assert!(contains(0, 10, 5));
        assert!(contains(0, 10, 9));
        assert!(!contains(0, 10, 10)); // half-open
        assert!(!contains(5, 5, 5)); // empty
        assert!(!contains(5, 3, 4)); // inverted
    }

    // r[verify interval.is-empty]
    #[test]
    fn test_is_empty() {
        assert!(is_empty(5, 5));
        assert!(is_empty(5, 3));
        assert!(!is_empty(0, 1));
        assert!(!is_empty(0, u64::MAX));
    }

    // r[verify interval.overlaps]
    #[test]
    fn test_overlaps() {
        assert!(overlaps(0, 10, 5, 15)); // partial overlap
        assert!(overlaps(0, 10, 0, 10)); // identical
        assert!(overlaps(0, 10, 3, 7)); // contained
        assert!(!overlaps(0, 5, 5, 10)); // adjacent, not overlapping
        assert!(!overlaps(0, 5, 6, 10)); // disjoint
        assert!(!overlaps(5, 5, 0, 10)); // empty a
    }

    // r[verify interval.split]
    #[test]
    fn test_split() {
        let ((l_lo, l_hi), (r_lo, r_hi)) = split(0, 10, 5);
        assert_eq!((l_lo, l_hi), (0, 5));
        assert_eq!((r_lo, r_hi), (5, 10));
        assert!(!overlaps(l_lo, l_hi, r_lo, r_hi));
    }

    // r[verify interval.adjacent]
    #[test]
    fn test_adjacent() {
        assert!(adjacent(0, 5, 5, 10));
        assert!(adjacent(5, 10, 0, 5)); // symmetric
        assert!(!adjacent(0, 5, 6, 10)); // gap
        assert!(!adjacent(0, 5, 4, 10)); // overlap
        assert!(!adjacent(5, 5, 5, 10)); // empty
    }

    // r[verify interval.merge]
    #[test]
    fn test_merge() {
        let (lo, hi) = merge(0, 5, 5, 10);
        assert_eq!((lo, hi), (0, 10));

        // Reverse order
        let (lo, hi) = merge(5, 10, 0, 5);
        assert_eq!((lo, hi), (0, 10));
    }

    // r[verify interval.split]
    // r[verify interval.merge]
    #[test]
    fn test_split_merge_roundtrip() {
        let ((l_lo, l_hi), (r_lo, r_hi)) = split(10, 90, 50);
        let (lo, hi) = merge(l_lo, l_hi, r_lo, r_hi);
        assert_eq!((lo, hi), (10, 90));
    }

    proptest! {
        // r[verify interval.contains]
        #[test]
        fn prop_empty_contains_nothing(lo: u64, hi: u64, x: u64) {
            if lo >= hi {
                prop_assert!(!contains(lo, hi, x));
            }
        }

        // r[verify interval.overlaps]
        #[test]
        fn prop_overlap_symmetric(a_lo: u64, a_hi: u64, b_lo: u64, b_hi: u64) {
            prop_assert_eq!(overlaps(a_lo, a_hi, b_lo, b_hi), overlaps(b_lo, b_hi, a_lo, a_hi));
        }

        // r[verify interval.adjacent]
        #[test]
        fn prop_adjacent_symmetric(a_lo: u64, a_hi: u64, b_lo: u64, b_hi: u64) {
            prop_assert_eq!(adjacent(a_lo, a_hi, b_lo, b_hi), adjacent(b_lo, b_hi, a_lo, a_hi));
        }

        // r[verify interval.adjacent]
        // r[verify interval.overlaps]
        #[test]
        fn prop_adjacent_no_overlap(a_lo: u64, a_hi: u64, b_lo: u64, b_hi: u64) {
            if adjacent(a_lo, a_hi, b_lo, b_hi) {
                prop_assert!(!overlaps(a_lo, a_hi, b_lo, b_hi));
            }
        }

        // r[verify interval.split]
        // r[verify interval.merge]
        #[test]
        fn prop_split_merge_roundtrip(
            lo in 0u64..=u64::MAX / 2,
            spread in 2u64..=1000,
            frac in 1u64..=999,
        ) {
            let hi = lo.saturating_add(spread);
            if lo < hi {
                let mid_offset = ((spread - 1) * frac / 1000).max(1);
                let mid = lo + mid_offset;
                if lo < mid && mid < hi {
                    let ((l_lo, l_hi), (r_lo, r_hi)) = split(lo, hi, mid);
                    // No overlap
                    prop_assert!(!overlaps(l_lo, l_hi, r_lo, r_hi));
                    // Adjacent
                    prop_assert!(adjacent(l_lo, l_hi, r_lo, r_hi));
                    // Merge recovers original
                    let (m_lo, m_hi) = merge(l_lo, l_hi, r_lo, r_hi);
                    prop_assert_eq!((m_lo, m_hi), (lo, hi));
                }
            }
        }

        // r[verify interval.split]
        // r[verify interval.contains]
        #[test]
        fn prop_split_covers(
            lo in 0u64..=u64::MAX / 2,
            spread in 2u64..=1000,
        ) {
            let hi = lo.saturating_add(spread);
            if lo + 1 < hi {
                let mid = lo + 1;
                // Every point in original is in exactly one half
                for x in lo..hi.min(lo + 20) {
                    let in_left = contains(lo, mid, x);
                    let in_right = contains(mid, hi, x);
                    prop_assert!(in_left || in_right);
                    prop_assert!(!(in_left && in_right));
                }
            }
        }
    }
}
