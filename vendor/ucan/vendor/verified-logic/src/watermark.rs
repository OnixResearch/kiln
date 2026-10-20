//! Verified watermark (bump) allocation arithmetic.
//!
//! A watermark tracks the current allocation frontier within a fixed
//! region. Allocation advances the watermark by the requested size.
//! Remaining space decreases monotonically. Reset reclaims everything.

use vstd::prelude::*;

verus! {

// --- watermark_remaining ---

// r[depends watermark.remaining]
/// Spec: bytes remaining in the region.
pub open spec fn watermark_remaining_spec(mark: u64, base: u64, capacity: u64) -> u64
    recommends base <= mark, mark as int <= base as int + capacity as int
{
    (base as int + capacity as int - mark as int) as u64
}

// r[impl watermark.remaining]
/// Returns the number of bytes remaining.
pub fn watermark_remaining(mark: u64, base: u64, capacity: u64) -> (result: u64)
    requires
        base <= mark,
        mark as int <= base as int + capacity as int,
        base as int + capacity as int <= u64::MAX as int,
    ensures result == watermark_remaining_spec(mark, base, capacity)
{
    base + capacity - mark
}

// r[verify watermark.remaining]
/// At the base, remaining equals capacity.
proof fn remaining_at_base(base: u64, capacity: u64)
    requires base as int + capacity as int <= u64::MAX as int
    ensures watermark_remaining_spec(base, base, capacity) == capacity
{}

// r[impl watermark.capacity-conservation]
// r[verify watermark.capacity-conservation]
/// remaining + used == capacity.
proof fn capacity_conservation(mark: u64, base: u64, capacity: u64)
    requires base <= mark, mark as int <= base as int + capacity as int
    ensures watermark_remaining_spec(mark, base, capacity) as int
        + (mark as int - base as int) == capacity as int
{}

// --- watermark_fits ---

// r[depends watermark.fits]
/// Spec: allocation of `size` bytes fits in remaining space.
pub open spec fn watermark_fits_spec(mark: u64, size: u64, base: u64, capacity: u64) -> bool
    recommends base <= mark, mark as int <= base as int + capacity as int
{
    mark as int + size as int <= base as int + capacity as int
}

// r[impl watermark.fits]
/// Returns true if `size` bytes fit in remaining space.
pub fn watermark_fits(mark: u64, size: u64, base: u64, capacity: u64) -> (result: bool)
    requires
        base <= mark,
        mark as int <= base as int + capacity as int,
        base as int + capacity as int <= u64::MAX as int,
    ensures result == watermark_fits_spec(mark, size, base, capacity)
{
    // Avoid overflow: check size <= remaining instead
    let remaining = base + capacity - mark;
    size <= remaining
}

// r[verify watermark.fits]
/// Zero-size allocation always fits.
proof fn zero_always_fits(mark: u64, base: u64, capacity: u64)
    requires base <= mark, mark as int <= base as int + capacity as int
    ensures watermark_fits_spec(mark, 0, base, capacity)
{}

// r[verify watermark.fits]
/// Fits iff size <= remaining.
proof fn fits_iff_size_le_remaining(mark: u64, size: u64, base: u64, capacity: u64)
    requires base <= mark, mark as int <= base as int + capacity as int
    ensures watermark_fits_spec(mark, size, base, capacity)
        == (size as int <= watermark_remaining_spec(mark, base, capacity) as int)
{}

// --- watermark_alloc ---

// r[depends watermark.alloc]
/// Spec: new mark after allocating `size` bytes.
pub open spec fn watermark_alloc_spec(mark: u64, size: u64) -> u64
    recommends mark as int + size as int <= u64::MAX as int
{
    (mark as int + size as int) as u64
}

// r[impl watermark.alloc]
/// Returns `mark + size` — the new watermark position.
pub fn watermark_alloc(mark: u64, size: u64) -> (result: u64)
    requires mark as int + size as int <= u64::MAX as int
    ensures result == watermark_alloc_spec(mark, size)
{
    mark + size
}

// r[impl watermark.monotonic]
// r[verify watermark.monotonic]
/// Allocation moves the mark forward (or keeps it same for size==0).
proof fn alloc_monotonic(mark: u64, size: u64)
    requires mark as int + size as int <= u64::MAX as int
    ensures watermark_alloc_spec(mark, size) as int >= mark as int
{}

// r[verify watermark.monotonic]
/// Non-zero allocation strictly advances the mark.
proof fn alloc_strict_advance(mark: u64, size: u64)
    requires mark as int + size as int <= u64::MAX as int, size > 0
    ensures watermark_alloc_spec(mark, size) as int > mark as int
{}

// r[verify watermark.alloc]
/// Allocation reduces remaining by exactly `size`.
proof fn alloc_reduces_remaining(mark: u64, size: u64, base: u64, capacity: u64)
    requires
        base <= mark,
        mark as int + size as int <= base as int + capacity as int,
        base as int + capacity as int <= u64::MAX as int,
    ensures watermark_remaining_spec(
        watermark_alloc_spec(mark, size), base, capacity,
    ) as int == watermark_remaining_spec(mark, base, capacity) as int - size as int
{}

// --- watermark_reset ---

// r[depends watermark.reset]
/// Spec: reset mark to base.
pub open spec fn watermark_reset_spec(base: u64) -> u64 {
    base
}

// r[impl watermark.reset]
/// Returns the base address (resetting the watermark).
pub fn watermark_reset(base: u64) -> (result: u64)
    ensures result == watermark_reset_spec(base)
{
    base
}

// r[verify watermark.reset]
/// After reset, remaining equals capacity.
proof fn reset_full_remaining(base: u64, capacity: u64)
    requires base as int + capacity as int <= u64::MAX as int
    ensures watermark_remaining_spec(
        watermark_reset_spec(base), base, capacity,
    ) == capacity
{}

// r[verify watermark.reset]
/// After reset, the watermark is empty.
proof fn reset_is_empty(base: u64)
    ensures watermark_is_empty_spec(watermark_reset_spec(base), base)
{}

// --- watermark_is_empty ---

// r[depends watermark.is-empty]
/// Spec: no bytes have been allocated.
pub open spec fn watermark_is_empty_spec(mark: u64, base: u64) -> bool {
    mark == base
}

// r[impl watermark.is-empty]
/// Returns true if no allocations have been made.
pub fn watermark_is_empty(mark: u64, base: u64) -> (result: bool)
    ensures result == watermark_is_empty_spec(mark, base)
{
    mark == base
}

// r[verify watermark.is-empty]
/// Empty watermark has full remaining capacity.
proof fn empty_full_remaining(base: u64, capacity: u64)
    requires base as int + capacity as int <= u64::MAX as int
    ensures watermark_remaining_spec(base, base, capacity) == capacity
{}

// --- non-overlap ---

// r[impl watermark.non-overlap]
// r[verify watermark.non-overlap]
/// Two successive allocations produce disjoint regions.
proof fn successive_allocs_disjoint(
    mark: u64, size1: u64, size2: u64,
)
    requires
        size1 > 0, size2 > 0,
        mark as int + size1 as int + size2 as int <= u64::MAX as int,
    ensures ({
        let mark2 = watermark_alloc_spec(mark, size1);
        // Region 1: [mark, mark + size1), Region 2: [mark2, mark2 + size2)
        // mark + size1 == mark2, so region 1 ends where region 2 begins
        mark as int + size1 as int <= mark2 as int
        && mark2 as int + size2 as int > mark as int + size1 as int
        // Actually: they are adjacent, thus disjoint
        && mark as int + size1 as int == mark2 as int
    })
{}

// r[verify watermark.non-overlap]
/// The first allocation occupies [mark, mark+size1) and the second
/// occupies [mark+size1, mark+size1+size2). These don't overlap.
proof fn successive_allocs_no_shared_point(
    mark: u64, size1: u64, size2: u64, x: u64,
)
    requires
        size1 > 0, size2 > 0,
        mark as int + size1 as int + size2 as int <= u64::MAX as int,
    ensures ({
        let mark2 = watermark_alloc_spec(mark, size1);
        // x can't be in both [mark, mark+size1) and [mark2, mark2+size2)
        !((mark as int <= x as int && (x as int) < (mark as int + size1 as int))
          && (mark2 as int <= x as int && (x as int) < (mark2 as int + size2 as int)))
    })
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify watermark.remaining]
    #[test]
    fn test_watermark_remaining() {
        assert_eq!(watermark_remaining(0, 0, 100), 100);
        assert_eq!(watermark_remaining(50, 0, 100), 50);
        assert_eq!(watermark_remaining(100, 0, 100), 0);
        assert_eq!(watermark_remaining(200, 100, 200), 100);
    }

    // r[verify watermark.fits]
    #[test]
    fn test_watermark_fits() {
        assert!(watermark_fits(0, 50, 0, 100));
        assert!(watermark_fits(0, 100, 0, 100));
        assert!(!watermark_fits(0, 101, 0, 100));
        assert!(watermark_fits(50, 50, 0, 100));
        assert!(!watermark_fits(50, 51, 0, 100));
        assert!(watermark_fits(0, 0, 0, 100)); // zero always fits
    }

    // r[verify watermark.alloc]
    #[test]
    fn test_watermark_alloc() {
        assert_eq!(watermark_alloc(0, 32), 32);
        assert_eq!(watermark_alloc(32, 64), 96);
        assert_eq!(watermark_alloc(100, 0), 100); // zero-size
    }

    // r[verify watermark.reset]
    #[test]
    fn test_watermark_reset() {
        assert_eq!(watermark_reset(0), 0);
        assert_eq!(watermark_reset(100), 100);
        assert_eq!(watermark_remaining(watermark_reset(0), 0, 100), 100);
    }

    // r[verify watermark.is-empty]
    #[test]
    fn test_watermark_is_empty() {
        assert!(watermark_is_empty(0, 0));
        assert!(watermark_is_empty(100, 100));
        assert!(!watermark_is_empty(50, 0));
    }

    // r[verify watermark.non-overlap]
    #[test]
    fn test_successive_allocs() {
        let mark = 0u64;
        let m1 = watermark_alloc(mark, 32);
        let m2 = watermark_alloc(m1, 64);
        assert_eq!(m1, 32);
        assert_eq!(m2, 96);
        // Region 1: [0, 32), Region 2: [32, 96) — no overlap
        for x in 0..32 {
            assert!(x < m1); // in region 1
            assert!(x < 32); // not in region 2 (which starts at 32)
        }
        for x in 32..96 {
            assert!(x >= m1); // not in region 1
            assert!(x < m2); // in region 2
        }
    }

    proptest! {
        // r[verify watermark.capacity-conservation]
        #[test]
        fn prop_capacity_conservation(
            base in 0u64..=u64::MAX / 4,
            capacity in 0u64..=10000,
            used_frac in 0u64..=100,
        ) {
            if u128::from(base) + u128::from(capacity) <= u128::from(u64::MAX) {
                let used = capacity * used_frac / 100;
                let mark = base + used;
                let remaining = watermark_remaining(mark, base, capacity);
                prop_assert_eq!(u128::from(remaining) + u128::from(used), u128::from(capacity));
            }
        }

        // r[verify watermark.monotonic]
        #[test]
        fn prop_alloc_monotonic(
            mark in 0u64..=u64::MAX / 2,
            size in 0u64..=10000,
        ) {
            if u128::from(mark) + u128::from(size) <= u128::from(u64::MAX) {
                let new_mark = watermark_alloc(mark, size);
                prop_assert!(new_mark >= mark);
            }
        }

        // r[verify watermark.fits]
        #[test]
        fn prop_fits_iff_le_remaining(
            base in 0u64..=u64::MAX / 4,
            capacity in 1u64..=10000,
            used_pct in 0u64..=100,
            size in 0u64..=20000,
        ) {
            if u128::from(base) + u128::from(capacity) <= u128::from(u64::MAX) {
                let used = capacity * used_pct / 100;
                let mark = base + used;
                let remaining = watermark_remaining(mark, base, capacity);
                prop_assert_eq!(
                    watermark_fits(mark, size, base, capacity),
                    size <= remaining
                );
            }
        }

        // r[verify watermark.reset]
        #[test]
        fn prop_reset_full(
            base in 0u64..=u64::MAX / 4,
            capacity in 0u64..=10000,
        ) {
            if u128::from(base) + u128::from(capacity) <= u128::from(u64::MAX) {
                let reset_mark = watermark_reset(base);
                prop_assert!(watermark_is_empty(reset_mark, base));
                prop_assert_eq!(watermark_remaining(reset_mark, base, capacity), capacity);
            }
        }

        // r[verify watermark.non-overlap]
        #[test]
        fn prop_successive_disjoint(
            mark in 0u64..=u64::MAX / 4,
            size1 in 1u64..=1000,
            size2 in 1u64..=1000,
        ) {
            if u128::from(mark) + u128::from(size1) + u128::from(size2) <= u128::from(u64::MAX) {
                let m1 = watermark_alloc(mark, size1);
                let m2 = watermark_alloc(m1, size2);
                // Region 1: [mark, m1), Region 2: [m1, m2)
                prop_assert_eq!(m1, mark + size1);
                prop_assert_eq!(m2, m1 + size2);
                // Disjoint: region 1 ends at m1, region 2 starts at m1
                prop_assert!(m1 > mark); // region 1 non-empty
                prop_assert!(m2 > m1); // region 2 non-empty
                // No shared point possible
                for x in mark..mark + size1.min(10) {
                    prop_assert!(x >= mark && x < m1); // in region 1
                    prop_assert!(!(x >= m1 && x < m2)); // not in region 2
                }
            }
        }

        // r[verify watermark.alloc]
        #[test]
        fn prop_alloc_reduces_remaining(
            base in 0u64..=u64::MAX / 4,
            capacity in 1u64..=10000,
            used in 0u64..=10000,
            size in 0u64..=10000,
        ) {
            let used = used.min(capacity);
            let size = size.min(capacity - used);
            if u128::from(base) + u128::from(capacity) <= u128::from(u64::MAX) {
                let mark = base + used;
                let rem_before = watermark_remaining(mark, base, capacity);
                let new_mark = watermark_alloc(mark, size);
                let rem_after = watermark_remaining(new_mark, base, capacity);
                prop_assert_eq!(rem_before - size, rem_after);
            }
        }
    }
}
