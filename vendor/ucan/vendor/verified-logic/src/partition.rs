//! Verified interval partition operations.
//!
//! Models a partition as a sorted sequence of (lo, hi) pairs representing
//! half-open intervals [lo, hi) that cover a contiguous range with no gaps
//! or overlaps. Includes validation, adjacency checks, and coverage proofs.

#![allow(clippy::len_zero)]

use vstd::prelude::*;

verus! {

// --- is_non_empty_interval ---

// r[depends partition.is-non-empty-interval]
/// Spec: interval [lo, hi) contains at least one point.
pub open spec fn is_non_empty_interval_spec(lo: u64, hi: u64) -> bool {
    lo < hi
}

// r[impl partition.is-non-empty-interval]
/// Returns true if interval [lo, hi) is non-empty.
pub fn is_non_empty_interval(lo: u64, hi: u64) -> (result: bool)
    ensures result == is_non_empty_interval_spec(lo, hi)
{
    lo < hi
}

// --- is_adjacent_pair ---

// r[depends partition.is-adjacent-pair]
/// Spec: one interval ends exactly where the next begins.
pub open spec fn is_adjacent_pair_spec(a_hi: u64, b_lo: u64) -> bool {
    a_hi == b_lo
}

// r[impl partition.is-adjacent-pair]
/// Returns true if the first interval's end equals the second's start.
pub fn is_adjacent_pair(a_hi: u64, b_lo: u64) -> (result: bool)
    ensures result == is_adjacent_pair_spec(a_hi, b_lo)
{
    a_hi == b_lo
}

// --- is_valid_partition ---

/// Spec: all intervals up to index `i` form a valid partition prefix.
pub open spec fn is_valid_prefix_spec(intervals: Seq<(u64, u64)>, i: int) -> bool {
    forall|j: int| 0 <= j < i ==> {
        &&& is_non_empty_interval_spec(intervals[j].0, intervals[j].1)
        &&& (j + 1 < i ==> is_adjacent_pair_spec(intervals[j].1, intervals[j + 1].0))
        &&& (j > 0 ==> intervals[j - 1].0 <= intervals[j].0)
    }
}

// r[depends partition.is-valid]
/// Spec: a sequence of intervals forms a valid partition.
pub open spec fn is_valid_partition_spec(intervals: Seq<(u64, u64)>) -> bool {
    is_valid_prefix_spec(intervals, intervals.len() as int)
}

proof fn valid_prefix_extend(intervals: Seq<(u64, u64)>, i: int)
    requires
        0 <= i < intervals.len(),
        is_valid_prefix_spec(intervals, i),
        is_non_empty_interval_spec(intervals[i].0, intervals[i].1),
        i > 0 ==> intervals[i - 1].0 <= intervals[i].0,
        i > 0 ==> is_adjacent_pair_spec(intervals[i - 1].1, intervals[i].0),
    ensures
        is_valid_prefix_spec(intervals, i + 1),
{
    assert forall|j: int| 0 <= j < i + 1 implies {
        &&& is_non_empty_interval_spec(intervals[j].0, intervals[j].1)
        &&& (j + 1 < i + 1 ==> is_adjacent_pair_spec(intervals[j].1, intervals[j + 1].0))
        &&& (j > 0 ==> intervals[j - 1].0 <= intervals[j].0)
    } by {
        if j < i {
            if j + 1 == i {
                assert(i > 0);
            }
        } else {
            assert(j == i);
        }
    }
}

// r[impl partition.is-valid]
/// Validates that intervals form a valid partition.
/// Checks: (1) non-empty intervals, (2) sorted by start, (3) adjacent pairs.
pub fn is_valid_partition(intervals: &[(u64, u64)]) -> (result: bool)
    ensures result == is_valid_partition_spec(intervals@)
{
    if intervals.len() == 0 {
        return true;
    }

    let mut i: usize = 0;
    while i < intervals.len()
        invariant
            0 <= i <= intervals@.len(),
            is_valid_prefix_spec(intervals@, i as int),
        decreases intervals@.len() - i as int,
    {
        let (lo, hi) = intervals[i];

        // Check interval is non-empty
        if !is_non_empty_interval(lo, hi) {
            return false;
        }

        if i > 0 {
            let (prev_lo, prev_hi) = intervals[i - 1];

            // Check sorted order
            if prev_lo > lo {
                return false;
            }

            // Check adjacent (no gap, no overlap)
            if !is_adjacent_pair(prev_hi, lo) {
                return false;
            }
        }

        proof {
            valid_prefix_extend(intervals@, i as int);
        }
        i = i + 1;
    }

    true
}

// --- partition_span_lo ---

// r[depends partition.span]
/// Spec: start of the first interval in a partition.
pub open spec fn partition_span_lo_spec(intervals: Seq<(u64, u64)>) -> u64
    recommends intervals.len() > 0
{
    intervals[0].0
}

// r[impl partition.span]
/// Returns the start of the first interval.
pub fn partition_span_lo(intervals: &[(u64, u64)]) -> (result: u64)
    requires intervals@.len() > 0
    ensures result == partition_span_lo_spec(intervals@)
{
    intervals[0].0
}

// --- partition_span_hi ---

// r[depends partition.span]
/// Spec: end of the last interval in a partition.
pub open spec fn partition_span_hi_spec(intervals: Seq<(u64, u64)>) -> u64
    recommends intervals.len() > 0
{
    intervals[intervals.len() - 1].1
}

// r[impl partition.span]
/// Returns the end of the last interval.
pub fn partition_span_hi(intervals: &[(u64, u64)]) -> (result: u64)
    requires intervals@.len() > 0
    ensures result == partition_span_hi_spec(intervals@)
{
    intervals[intervals.len() - 1].1
}

// --- partition_covers ---

// r[depends partition.covers]
/// Spec: partition exactly covers the range [lo, hi).
pub open spec fn partition_covers_spec(
    intervals: Seq<(u64, u64)>,
    lo: u64,
    hi: u64
) -> bool {
    &&& intervals.len() > 0
    &&& partition_span_lo_spec(intervals) == lo
    &&& partition_span_hi_spec(intervals) == hi
}

// r[impl partition.covers]
/// Returns true if the partition spans exactly [lo, hi).
pub fn partition_covers(intervals: &[(u64, u64)], lo: u64, hi: u64) -> (result: bool)
    ensures result == partition_covers_spec(intervals@, lo, hi)
{
    if intervals.len() == 0 {
        return false;
    }

    let start = partition_span_lo(intervals);
    let end = partition_span_hi(intervals);

    start == lo && end == hi
}

// --- Proofs ---

// r[verify partition.is-valid]
/// Single interval forms a valid partition.
proof fn single_interval_valid(lo: u64, hi: u64)
    requires lo < hi
    ensures is_valid_partition_spec(seq![(lo, hi)])
{
    let intervals = seq![(lo, hi)];
    assert(is_valid_prefix_spec(intervals, 1));
}

// r[verify partition.is-valid]
/// Empty partition is valid (vacuously).
proof fn empty_partition_valid()
    ensures is_valid_partition_spec(Seq::<(u64, u64)>::empty())
{
    let intervals = Seq::<(u64, u64)>::empty();
    assert(is_valid_prefix_spec(intervals, 0));
}

// r[verify partition.is-valid]
/// Valid partition has all non-empty intervals.
proof fn valid_partition_has_non_empty_intervals(intervals: Seq<(u64, u64)>)
    requires is_valid_partition_spec(intervals)
    ensures forall|i: int| 0 <= i < intervals.len() ==>
        is_non_empty_interval_spec(intervals[i].0, intervals[i].1)
{}



// r[verify partition.covers]
/// Single interval partition covers the expected range.
proof fn single_interval_covers(lo: u64, hi: u64)
    requires lo < hi
    ensures partition_covers_spec(seq![(lo, hi)], lo, hi)
{
    let intervals = seq![(lo, hi)];
    assert(partition_span_lo_spec(intervals) == lo);
    assert(partition_span_hi_spec(intervals) == hi);
}



} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify partition.is-non-empty-interval]
    #[test]
    fn test_is_non_empty_interval() {
        assert!(is_non_empty_interval(0, 10));
        assert!(is_non_empty_interval(5, 6));
        assert!(!is_non_empty_interval(5, 5));
        assert!(!is_non_empty_interval(10, 5));
    }

    // r[verify partition.is-adjacent-pair]
    #[test]
    fn test_is_adjacent_pair() {
        assert!(is_adjacent_pair(5, 5));
        assert!(is_adjacent_pair(0, 0));
        assert!(is_adjacent_pair(100, 100));
        assert!(!is_adjacent_pair(5, 6));
        assert!(!is_adjacent_pair(10, 5));
    }

    // r[verify partition.is-valid]
    #[test]
    fn test_is_valid_partition() {
        // Empty partition
        assert!(is_valid_partition(&[]));

        // Single interval
        assert!(is_valid_partition(&[(0, 10)]));
        assert!(!is_valid_partition(&[(5, 5)])); // empty interval

        // Adjacent intervals
        assert!(is_valid_partition(&[(0, 5), (5, 10)]));
        assert!(is_valid_partition(&[(0, 5), (5, 10), (10, 15)]));

        // Gap between intervals
        assert!(!is_valid_partition(&[(0, 5), (6, 10)]));

        // Overlapping intervals
        assert!(!is_valid_partition(&[(0, 6), (5, 10)]));

        // Unsorted intervals
        assert!(!is_valid_partition(&[(5, 10), (0, 5)]));

        // Empty interval in sequence
        assert!(!is_valid_partition(&[(0, 5), (5, 5), (5, 10)]));
    }

    // r[verify partition.span]
    #[test]
    fn test_partition_span() {
        let intervals = vec![(0, 5), (5, 10), (10, 15)];
        assert_eq!(partition_span_lo(&intervals), 0);
        assert_eq!(partition_span_hi(&intervals), 15);

        let single = vec![(100, 200)];
        assert_eq!(partition_span_lo(&single), 100);
        assert_eq!(partition_span_hi(&single), 200);
    }

    // r[verify partition.covers]
    #[test]
    fn test_partition_covers() {
        // Empty never covers
        assert!(!partition_covers(&[], 0, 10));

        // Single interval exact match
        assert!(partition_covers(&[(0, 10)], 0, 10));
        assert!(!partition_covers(&[(0, 10)], 0, 5));
        assert!(!partition_covers(&[(0, 10)], 5, 10));

        // Multi-interval exact match
        assert!(partition_covers(&[(0, 5), (5, 10)], 0, 10));
        assert!(!partition_covers(&[(0, 5), (5, 10)], 1, 10));
        assert!(!partition_covers(&[(0, 5), (5, 10)], 0, 9));
    }

    proptest! {
        // r[verify partition.is-non-empty-interval]
        #[test]
        fn prop_non_empty_interval_iff_lt(lo: u64, hi: u64) {
            prop_assert_eq!(is_non_empty_interval(lo, hi), lo < hi);
        }
    }

    // r[verify partition.is-valid]
    /// Construct valid 3-interval partition and verify properties.
    #[test]
    fn test_three_interval_partition() {
        let intervals = vec![(0, 10), (10, 20), (20, 30)];

        // Valid partition
        assert!(is_valid_partition(&intervals));

        // Covers full range
        assert!(partition_covers(&intervals, 0, 30));

        // Span properties
        assert_eq!(partition_span_lo(&intervals), 0);
        assert_eq!(partition_span_hi(&intervals), 30);

        // Doesn't cover partial ranges
        assert!(!partition_covers(&intervals, 0, 25));
        assert!(!partition_covers(&intervals, 5, 30));
    }

    // r[verify partition.is-valid]
    /// Test edge cases with single-point intervals.
    #[test]
    fn test_single_point_coverage() {
        // Adjacent single-point coverage: [0,1), [1,2), [2,3)
        let intervals = vec![(0, 1), (1, 2), (2, 3)];
        assert!(is_valid_partition(&intervals));
        assert!(partition_covers(&intervals, 0, 3));

        // Large span with single points
        let intervals = vec![(100, 101), (101, 102)];
        assert!(is_valid_partition(&intervals));
        assert!(partition_covers(&intervals, 100, 102));
    }
}
