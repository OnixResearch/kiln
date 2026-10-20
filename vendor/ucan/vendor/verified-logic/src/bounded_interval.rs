//! Bounded closed integer intervals for exact domain primitives.

use vstd::prelude::*;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BoundedInterval {
    pub lo: i64,
    pub hi: i64,
}

// r[depends domain-primitives.bounded-interval]
pub open spec fn bounded_interval_valid_spec(lo: i64, hi: i64) -> bool {
    lo <= hi
}

// r[depends domain-primitives.bounded-interval]
pub open spec fn bounded_interval_accepted_spec(interval: BoundedInterval) -> bool {
    interval.lo <= interval.hi
}

// r[depends domain-primitives.bounded-interval]
pub open spec fn bounded_interval_contains_spec(interval: BoundedInterval, value: i64) -> bool {
    bounded_interval_accepted_spec(interval) && interval.lo <= value && value <= interval.hi
}

// r[depends domain-primitives.bounded-interval]
pub open spec fn bounded_interval_overlaps_spec(a: BoundedInterval, b: BoundedInterval) -> bool {
    bounded_interval_accepted_spec(a) && bounded_interval_accepted_spec(b) && a.lo <= b.hi && b.lo <= a.hi
}

// r[impl domain-primitives.bounded-interval]
pub fn make_bounded_interval(lo: i64, hi: i64) -> (result: Option<BoundedInterval>)
    ensures
        result.is_some() <==> bounded_interval_valid_spec(lo, hi),
        result.is_some() ==> bounded_interval_accepted_spec(result.unwrap()),
{
    if lo <= hi {
        Some(BoundedInterval { lo, hi })
    } else {
        None
    }
}

// r[impl domain-primitives.bounded-interval]
pub fn bounded_interval_contains(interval: BoundedInterval, value: i64) -> (result: bool)
    requires bounded_interval_accepted_spec(interval)
    ensures result == bounded_interval_contains_spec(interval, value)
{
    interval.lo <= value && value <= interval.hi
}

// r[impl domain-primitives.bounded-interval]
pub fn bounded_interval_overlaps(a: BoundedInterval, b: BoundedInterval) -> (result: bool)
    requires bounded_interval_accepted_spec(a), bounded_interval_accepted_spec(b)
    ensures result == bounded_interval_overlaps_spec(a, b)
{
    a.lo <= b.hi && b.lo <= a.hi
}

// r[impl domain-primitives.bounded-interval]
pub fn bounded_interval_clamp(interval: BoundedInterval, value: i64) -> (result: i64)
    requires bounded_interval_accepted_spec(interval)
    ensures bounded_interval_contains_spec(interval, result)
{
    if value < interval.lo {
        interval.lo
    } else if value > interval.hi {
        interval.hi
    } else {
        value
    }
}

// r[verify domain-primitives.bounded-interval]
pub proof fn bounded_interval_overlap_symmetric(a: BoundedInterval, b: BoundedInterval)
    ensures bounded_interval_overlaps_spec(a, b) == bounded_interval_overlaps_spec(b, a)
{}

// r[verify domain-primitives.bounded-interval]
pub proof fn bounded_interval_disjoint_rejected(a: BoundedInterval, b: BoundedInterval)
    requires bounded_interval_accepted_spec(a), bounded_interval_accepted_spec(b), a.hi < b.lo
    ensures !bounded_interval_overlaps_spec(a, b)
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_accepts_contains_and_clamps() {
        let i = make_bounded_interval(10, 20).unwrap();
        assert!(bounded_interval_contains(i, 15));
        assert_eq!(bounded_interval_clamp(i, 25), 20);
    }

    #[test]
    fn interval_rejects_inverted_and_disjoint_overlap() {
        assert!(make_bounded_interval(5, 4).is_none());
        let a = make_bounded_interval(0, 2).unwrap();
        let b = make_bounded_interval(3, 5).unwrap();
        assert!(!bounded_interval_overlaps(a, b));
    }
}
