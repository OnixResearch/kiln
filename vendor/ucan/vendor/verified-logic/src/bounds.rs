//! Verified bounds checking and clamping.

use vstd::prelude::*;

verus! {

// r[depends bounds.in-range]
/// Spec: value is within inclusive range [lo, hi].
pub open spec fn in_range_spec(value: u64, lo: u64, hi: u64) -> bool {
    lo <= value && value <= hi
}

// r[impl bounds.in-range]
/// Returns `true` if `value` is within the inclusive range `[lo, hi]`.
pub fn in_range(value: u64, lo: u64, hi: u64) -> (result: bool)
    ensures result == in_range_spec(value, lo, hi)
{
    lo <= value && value <= hi
}

// r[depends bounds.clamp]
/// Spec: clamp value to [lo, hi].
pub open spec fn clamp_spec(value: u64, lo: u64, hi: u64) -> u64
    recommends lo <= hi
{
    if value < lo { lo }
    else if value > hi { hi }
    else { value }
}

// r[impl bounds.clamp]
/// Clamps `value` to the inclusive range `[lo, hi]`.
pub fn clamp(value: u64, lo: u64, hi: u64) -> (result: u64)
    requires lo <= hi
    ensures result == clamp_spec(value, lo, hi)
{
    if value < lo { lo }
    else if value > hi { hi }
    else { value }
}

// r[verify bounds.in-range]
proof fn in_range_boundaries(lo: u64, hi: u64)
    requires lo <= hi
    ensures in_range_spec(lo, lo, hi), in_range_spec(hi, lo, hi)
{}

// r[verify bounds.in-range]
proof fn in_range_empty(value: u64, lo: u64, hi: u64)
    requires lo > hi
    ensures !in_range_spec(value, lo, hi)
{}

// r[verify bounds.clamp]
proof fn clamp_in_range(value: u64, lo: u64, hi: u64)
    requires lo <= hi
    ensures in_range_spec(clamp_spec(value, lo, hi), lo, hi)
{}

// r[verify bounds.clamp]
proof fn clamp_idempotent(value: u64, lo: u64, hi: u64)
    requires lo <= hi
    ensures clamp_spec(clamp_spec(value, lo, hi), lo, hi) == clamp_spec(value, lo, hi)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify bounds.in-range]
    #[test]
    fn test_in_range() {
        assert!(in_range(5, 0, 10));
        assert!(in_range(0, 0, 10));
        assert!(in_range(10, 0, 10));
        assert!(!in_range(11, 0, 10));
        assert!(in_range(5, 5, 5));
        assert!(!in_range(0, 5, 3));
    }

    // r[verify bounds.clamp]
    #[test]
    fn test_clamp() {
        assert_eq!(clamp(5, 0, 10), 5);
        assert_eq!(clamp(0, 3, 10), 3);
        assert_eq!(clamp(15, 3, 10), 10);
        for &v in &[0, 5, 10, 15, u64::MAX] {
            let c = clamp(v, 3, 10);
            assert!(in_range(c, 3, 10));
        }
    }

    proptest! {
        // r[verify bounds.in-range]
        #[test]
        fn prop_in_range_boundaries(lo in 0..=u64::MAX / 2, spread in 0..=u64::MAX / 2) {
            let hi = lo + spread;
            prop_assert!(in_range(lo, lo, hi));
            prop_assert!(in_range(hi, lo, hi));
        }

        // r[verify bounds.clamp]
        #[test]
        fn prop_clamp_result_in_range(
            value: u64,
            lo in 0..=u64::MAX / 2,
            spread in 0..=u64::MAX / 2,
        ) {
            let hi = lo + spread;
            prop_assert!(in_range(clamp(value, lo, hi), lo, hi));
        }

        // r[verify bounds.clamp]
        #[test]
        fn prop_clamp_idempotent(
            value: u64,
            lo in 0..=u64::MAX / 2,
            spread in 0..=u64::MAX / 2,
        ) {
            let hi = lo + spread;
            prop_assert_eq!(clamp(clamp(value, lo, hi), lo, hi), clamp(value, lo, hi));
        }
    }
}
