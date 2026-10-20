//! Verified address region arithmetic.
//!
//! A region is `(base, size)` representing the half-open address range
//! `[base, base + size)`. Operations: containment, disjointness, adjacency,
//! merge, and split — with overflow-safe arithmetic for 64-bit address spaces.
//!
//! Region operations agree with `interval` module operations where the
//! interval endpoints don't overflow (see cross-module properties in
//! `properties.rs`).

#![allow(clippy::nonminimal_bool)]

use vstd::prelude::*;

verus! {

// --- end_addr ---

// r[depends region.end-addr]
/// Spec: exclusive end address of a region.
pub open spec fn end_addr_spec(base: u64, size: u64) -> int {
    base as int + size as int
}

// r[impl region.end-addr]
/// Returns the exclusive end address `base + size`.
pub fn end_addr(base: u64, size: u64) -> (result: u64)
    requires base as int + size as int <= u64::MAX as int
    ensures result == end_addr_spec(base, size)
{
    base + size
}

// --- is_empty ---

// r[depends region.is-empty]
/// Spec: region has no addresses.
pub open spec fn is_empty_spec(size: u64) -> bool {
    size == 0
}

// r[impl region.is-empty]
/// Returns true if the region is empty.
pub fn is_empty(size: u64) -> (result: bool)
    ensures result == is_empty_spec(size)
{
    size == 0
}

// --- contains_addr ---

// r[depends region.contains-addr]
/// Spec: address x is in the region [base, base + size).
pub open spec fn contains_addr_spec(base: u64, size: u64, x: u64) -> bool {
    (base as int) <= (x as int) && (x as int) < (base as int + size as int)
}

// r[impl region.contains-addr]
/// Returns true if `x` is in `[base, base + size)`.
pub fn contains_addr(base: u64, size: u64, x: u64) -> (result: bool)
    requires base as int + size as int <= u64::MAX as int
    ensures result == contains_addr_spec(base, size, x)
{
    x >= base && x < base + size
}

// r[verify region.contains-addr]
/// Empty regions contain nothing.
proof fn empty_contains_nothing(base: u64, x: u64)
    ensures !contains_addr_spec(base, 0, x)
{}

// r[verify region.contains-addr]
/// Base is always in a non-empty region.
proof fn base_in_nonempty(base: u64, size: u64)
    requires size > 0
    ensures contains_addr_spec(base, size, base)
{}

// r[verify region.contains-addr]
/// The end address is not in the region (half-open).
proof fn end_not_contained(base: u64, size: u64)
    requires base as int + size as int <= u64::MAX as int
    ensures !contains_addr_spec(base, size, (base as int + size as int) as u64)
{}

// --- regions_disjoint ---

// r[depends region.disjoint]
/// Spec: two regions share no address.
pub open spec fn regions_disjoint_spec(
    a_base: u64, a_size: u64, b_base: u64, b_size: u64,
) -> bool {
    a_size == 0 || b_size == 0
    || a_base as int + a_size as int <= b_base as int
    || b_base as int + b_size as int <= a_base as int
}

// r[impl region.disjoint]
/// Returns true if the two regions share no address.
pub fn regions_disjoint(
    a_base: u64, a_size: u64, b_base: u64, b_size: u64,
) -> (result: bool)
    requires
        a_base as int + a_size as int <= u64::MAX as int,
        b_base as int + b_size as int <= u64::MAX as int,
    ensures result == regions_disjoint_spec(a_base, a_size, b_base, b_size)
{
    a_size == 0 || b_size == 0
    || a_base + a_size <= b_base
    || b_base + b_size <= a_base
}

// r[verify region.disjoint]
/// Disjointness is symmetric.
proof fn disjoint_symmetric(
    a_base: u64, a_size: u64, b_base: u64, b_size: u64,
)
    ensures regions_disjoint_spec(a_base, a_size, b_base, b_size)
        == regions_disjoint_spec(b_base, b_size, a_base, a_size)
{}

// r[verify region.disjoint]
/// Empty regions are disjoint from everything.
proof fn empty_disjoint(a_base: u64, b_base: u64, b_size: u64)
    ensures regions_disjoint_spec(a_base, 0, b_base, b_size)
{}

// r[verify region.disjoint]
/// A non-empty region is not disjoint from itself.
proof fn self_not_disjoint(base: u64, size: u64)
    requires size > 0
    ensures !regions_disjoint_spec(base, size, base, size)
{}

// --- contains_region ---

// r[depends region.contains-region]
/// Spec: every address in region B is also in region A.
pub open spec fn contains_region_spec(
    a_base: u64, a_size: u64, b_base: u64, b_size: u64,
) -> bool {
    b_size == 0
    || (a_base as int <= b_base as int
        && b_base as int + b_size as int <= a_base as int + a_size as int)
}

// r[impl region.contains-region]
/// Returns true if region A fully contains region B.
pub fn contains_region(
    a_base: u64, a_size: u64, b_base: u64, b_size: u64,
) -> (result: bool)
    requires
        a_base as int + a_size as int <= u64::MAX as int,
        b_base as int + b_size as int <= u64::MAX as int,
    ensures result == contains_region_spec(a_base, a_size, b_base, b_size)
{
    b_size == 0
    || (a_base <= b_base && b_base + b_size <= a_base + a_size)
}

// r[verify region.contains-region]
/// Containment is reflexive.
proof fn contains_region_reflexive(base: u64, size: u64)
    ensures contains_region_spec(base, size, base, size)
{}

// r[verify region.contains-region]
/// Containment is transitive.
proof fn contains_region_transitive(
    a_base: u64, a_size: u64,
    b_base: u64, b_size: u64,
    c_base: u64, c_size: u64,
)
    requires
        contains_region_spec(a_base, a_size, b_base, b_size),
        contains_region_spec(b_base, b_size, c_base, c_size),
    ensures contains_region_spec(a_base, a_size, c_base, c_size)
{}

// r[verify region.contains-region]
/// A region always contains the empty region.
proof fn contains_empty(a_base: u64, a_size: u64, b_base: u64)
    ensures contains_region_spec(a_base, a_size, b_base, 0)
{}

// r[verify region.contains-region]
/// If A contains B and x is in B, then x is in A.
proof fn contains_region_implies_addr(
    a_base: u64, a_size: u64,
    b_base: u64, b_size: u64,
    x: u64,
)
    requires
        contains_region_spec(a_base, a_size, b_base, b_size),
        contains_addr_spec(b_base, b_size, x),
    ensures contains_addr_spec(a_base, a_size, x)
{}

// --- region_adjacent ---

// r[depends region.adjacent]
/// Spec: one region ends exactly where the other begins.
pub open spec fn region_adjacent_spec(
    a_base: u64, a_size: u64, b_base: u64, b_size: u64,
) -> bool {
    (a_size > 0 && b_size > 0 && a_base as int + a_size as int == b_base as int)
    || (a_size > 0 && b_size > 0 && b_base as int + b_size as int == a_base as int)
}

// r[impl region.adjacent]
/// Returns true if one region ends where the other begins.
pub fn region_adjacent(
    a_base: u64, a_size: u64, b_base: u64, b_size: u64,
) -> (result: bool)
    requires
        a_base as int + a_size as int <= u64::MAX as int,
        b_base as int + b_size as int <= u64::MAX as int,
    ensures result == region_adjacent_spec(a_base, a_size, b_base, b_size)
{
    (a_size > 0 && b_size > 0 && a_base + a_size == b_base)
    || (a_size > 0 && b_size > 0 && b_base + b_size == a_base)
}

// r[verify region.adjacent]
/// Adjacency is symmetric.
proof fn adjacent_symmetric(
    a_base: u64, a_size: u64, b_base: u64, b_size: u64,
)
    ensures region_adjacent_spec(a_base, a_size, b_base, b_size)
        == region_adjacent_spec(b_base, b_size, a_base, a_size)
{}

// r[verify region.adjacent]
/// Adjacent regions are disjoint.
proof fn adjacent_implies_disjoint(
    a_base: u64, a_size: u64, b_base: u64, b_size: u64,
)
    requires region_adjacent_spec(a_base, a_size, b_base, b_size)
    ensures regions_disjoint_spec(a_base, a_size, b_base, b_size)
{}

// r[verify region.adjacent]
/// Empty regions are never adjacent.
proof fn empty_not_adjacent(a_base: u64, b_base: u64, b_size: u64)
    ensures !region_adjacent_spec(a_base, 0, b_base, b_size)
{}

// --- region_merge ---

// r[depends region.merge]
/// Spec: merge two adjacent regions into one.
pub open spec fn region_merge_spec(
    a_base: u64, a_size: u64, b_base: u64, b_size: u64,
) -> (u64, u64)
    recommends region_adjacent_spec(a_base, a_size, b_base, b_size)
{
    if a_base as int + a_size as int == b_base as int {
        (a_base, (a_size as int + b_size as int) as u64)
    } else {
        (b_base, (b_size as int + a_size as int) as u64)
    }
}

// r[impl region.merge]
/// Merges two adjacent regions into one.
pub fn region_merge(
    a_base: u64, a_size: u64, b_base: u64, b_size: u64,
) -> (result: (u64, u64))
    requires
        region_adjacent_spec(a_base, a_size, b_base, b_size),
        a_base as int + a_size as int + b_size as int <= u64::MAX as int
            || b_base as int + b_size as int + a_size as int <= u64::MAX as int,
    ensures result == region_merge_spec(a_base, a_size, b_base, b_size)
{
    if a_base + a_size == b_base {
        (a_base, a_size + b_size)
    } else {
        (b_base, b_size + a_size)
    }
}

// r[verify region.merge]
/// Merged region is non-empty.
proof fn merge_nonempty(
    a_base: u64, a_size: u64, b_base: u64, b_size: u64,
)
    requires
        region_adjacent_spec(a_base, a_size, b_base, b_size),
        a_size as int + b_size as int <= u64::MAX as int,
    ensures !is_empty_spec(region_merge_spec(a_base, a_size, b_base, b_size).1)
{}

// r[verify region.merge]
/// Merged region size equals sum of input sizes.
proof fn merge_size_sum(
    a_base: u64, a_size: u64, b_base: u64, b_size: u64,
)
    requires
        region_adjacent_spec(a_base, a_size, b_base, b_size),
        a_size as int + b_size as int <= u64::MAX as int,
    ensures region_merge_spec(a_base, a_size, b_base, b_size).1 as int
        == a_size as int + b_size as int
{}

// --- region_split ---

// r[depends region.split]
/// Spec: left half of splitting a region at an internal offset.
pub open spec fn region_split_left_spec(base: u64, size: u64, offset: u64) -> (u64, u64)
    recommends 0 < offset && offset < size
{
    (base, offset)
}

/// Spec: right half of splitting a region at an internal offset.
pub open spec fn region_split_right_spec(base: u64, size: u64, offset: u64) -> (u64, u64)
    recommends 0 < offset && offset < size
{
    ((base as int + offset as int) as u64, (size - offset) as u64)
}

// r[impl region.split]
/// Splits a region at `offset` bytes from base.
pub fn region_split(base: u64, size: u64, offset: u64) -> (result: ((u64, u64), (u64, u64)))
    requires
        0 < offset, offset < size,
        base as int + size as int <= u64::MAX as int,
    ensures
        result.0 == region_split_left_spec(base, size, offset),
        result.1 == region_split_right_spec(base, size, offset),
{
    ((base, offset), (base + offset, size - offset))
}

// r[verify region.split]
/// Both halves are non-empty.
proof fn split_nonempty(base: u64, size: u64, offset: u64)
    requires 0 < offset, offset < size
    ensures
        !is_empty_spec(region_split_left_spec(base, size, offset).1),
        !is_empty_spec(region_split_right_spec(base, size, offset).1),
{}

// r[verify region.split]
/// Halves are disjoint.
proof fn split_disjoint(base: u64, size: u64, offset: u64)
    requires
        0 < offset, offset < size,
        base as int + size as int <= u64::MAX as int,
    ensures regions_disjoint_spec(
        region_split_left_spec(base, size, offset).0,
        region_split_left_spec(base, size, offset).1,
        region_split_right_spec(base, size, offset).0,
        region_split_right_spec(base, size, offset).1,
    )
{}

// r[verify region.split]
/// Halves are adjacent.
proof fn split_adjacent(base: u64, size: u64, offset: u64)
    requires
        0 < offset, offset < size,
        base as int + size as int <= u64::MAX as int,
    ensures region_adjacent_spec(
        region_split_left_spec(base, size, offset).0,
        region_split_left_spec(base, size, offset).1,
        region_split_right_spec(base, size, offset).0,
        region_split_right_spec(base, size, offset).1,
    )
{}

// r[verify region.split]
/// Sizes sum to original.
proof fn split_size_sum(base: u64, size: u64, offset: u64)
    requires 0 < offset, offset < size
    ensures region_split_left_spec(base, size, offset).1 as int
        + region_split_right_spec(base, size, offset).1 as int
        == size as int
{}

// r[verify region.split]
// r[verify region.merge]
/// Split-then-merge is a roundtrip.
proof fn split_merge_roundtrip(base: u64, size: u64, offset: u64)
    requires
        0 < offset, offset < size,
        base as int + size as int <= u64::MAX as int,
    ensures region_merge_spec(
        region_split_left_spec(base, size, offset).0,
        region_split_left_spec(base, size, offset).1,
        region_split_right_spec(base, size, offset).0,
        region_split_right_spec(base, size, offset).1,
    ) == (base, size)
{}

// r[verify region.split]
// r[verify region.contains-addr]
/// Every address in the original is in exactly one half.
proof fn split_covers(base: u64, size: u64, offset: u64, x: u64)
    requires
        0 < offset, offset < size,
        base as int + size as int <= u64::MAX as int,
        contains_addr_spec(base, size, x),
    ensures
        contains_addr_spec(base, offset, x)
            || contains_addr_spec(
                region_split_right_spec(base, size, offset).0,
                region_split_right_spec(base, size, offset).1,
                x,
            ),
        !(contains_addr_spec(base, offset, x)
            && contains_addr_spec(
                region_split_right_spec(base, size, offset).0,
                region_split_right_spec(base, size, offset).1,
                x,
            )),
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify region.end-addr]
    #[test]
    fn test_end_addr() {
        assert_eq!(end_addr(0, 10), 10);
        assert_eq!(end_addr(100, 0), 100);
        assert_eq!(end_addr(50, 50), 100);
    }

    // r[verify region.is-empty]
    #[test]
    fn test_is_empty() {
        assert!(is_empty(0));
        assert!(!is_empty(1));
        assert!(!is_empty(u64::MAX));
    }

    // r[verify region.contains-addr]
    #[test]
    fn test_contains_addr() {
        assert!(contains_addr(0, 10, 0));
        assert!(contains_addr(0, 10, 5));
        assert!(contains_addr(0, 10, 9));
        assert!(!contains_addr(0, 10, 10)); // half-open
        assert!(!contains_addr(5, 0, 5)); // empty
        assert!(!contains_addr(10, 5, 3)); // below base
    }

    // r[verify region.disjoint]
    #[test]
    fn test_regions_disjoint() {
        assert!(regions_disjoint(0, 5, 5, 5)); // adjacent
        assert!(regions_disjoint(0, 5, 10, 5)); // gap
        assert!(regions_disjoint(0, 0, 0, 10)); // empty a
        assert!(regions_disjoint(0, 10, 0, 0)); // empty b
        assert!(!regions_disjoint(0, 10, 5, 10)); // overlap
        assert!(!regions_disjoint(0, 10, 0, 10)); // identical
    }

    // r[verify region.contains-region]
    #[test]
    fn test_contains_region() {
        assert!(contains_region(0, 10, 2, 5)); // inner
        assert!(contains_region(0, 10, 0, 10)); // self
        assert!(contains_region(0, 10, 0, 0)); // empty
        assert!(!contains_region(2, 5, 0, 10)); // larger
        assert!(!contains_region(0, 5, 3, 5)); // overlaps end
    }

    // r[verify region.adjacent]
    #[test]
    fn test_region_adjacent() {
        assert!(region_adjacent(0, 5, 5, 5));
        assert!(region_adjacent(5, 5, 0, 5)); // symmetric
        assert!(!region_adjacent(0, 5, 6, 5)); // gap
        assert!(!region_adjacent(0, 0, 0, 5)); // empty
        assert!(!region_adjacent(0, 5, 0, 0)); // empty
    }

    // r[verify region.merge]
    #[test]
    fn test_region_merge() {
        let (b, s) = region_merge(0, 5, 5, 5);
        assert_eq!((b, s), (0, 10));

        let (b, s) = region_merge(5, 5, 0, 5);
        assert_eq!((b, s), (0, 10));
    }

    // r[verify region.split]
    #[test]
    fn test_region_split() {
        let ((lb, ls), (rb, rs)) = region_split(10, 20, 8);
        assert_eq!((lb, ls), (10, 8));
        assert_eq!((rb, rs), (18, 12));
        assert!(regions_disjoint(lb, ls, rb, rs));
        assert!(region_adjacent(lb, ls, rb, rs));
        assert_eq!(ls + rs, 20);
    }

    // r[verify region.split]
    // r[verify region.merge]
    #[test]
    fn test_split_merge_roundtrip() {
        let ((lb, ls), (rb, rs)) = region_split(100, 50, 20);
        let (b, s) = region_merge(lb, ls, rb, rs);
        assert_eq!((b, s), (100, 50));
    }

    proptest! {
        // r[verify region.disjoint]
        #[test]
        fn prop_disjoint_symmetric(
            a_base in 0u64..=u64::MAX / 2,
            a_size in 0u64..=1000,
            b_base in 0u64..=u64::MAX / 2,
            b_size in 0u64..=1000,
        ) {
            prop_assert_eq!(
                regions_disjoint(a_base, a_size, b_base, b_size),
                regions_disjoint(b_base, b_size, a_base, a_size)
            );
        }

        // r[verify region.adjacent]
        #[test]
        fn prop_adjacent_symmetric(
            a_base in 0u64..=u64::MAX / 2,
            a_size in 0u64..=1000,
            b_base in 0u64..=u64::MAX / 2,
            b_size in 0u64..=1000,
        ) {
            prop_assert_eq!(
                region_adjacent(a_base, a_size, b_base, b_size),
                region_adjacent(b_base, b_size, a_base, a_size)
            );
        }

        // r[verify region.adjacent]
        // r[verify region.disjoint]
        #[test]
        fn prop_adjacent_implies_disjoint(
            a_base in 0u64..=u64::MAX / 2,
            a_size in 1u64..=1000,
            b_size in 1u64..=1000,
        ) {
            let b_base = a_base + a_size;
            if u128::from(b_base) + u128::from(b_size) <= u128::from(u64::MAX) {
                prop_assert!(region_adjacent(a_base, a_size, b_base, b_size));
                prop_assert!(regions_disjoint(a_base, a_size, b_base, b_size));
            }
        }

        // r[verify region.split]
        // r[verify region.merge]
        #[test]
        fn prop_split_merge_roundtrip(
            base in 0u64..=u64::MAX / 2,
            size in 2u64..=1000,
            frac in 1u64..=999,
        ) {
            let offset = ((size - 1) * frac / 1000).max(1).min(size - 1);
            if offset > 0 && offset < size && u128::from(base) + u128::from(size) <= u128::from(u64::MAX) {
                let ((lb, ls), (rb, rs)) = region_split(base, size, offset);
                // Disjoint
                prop_assert!(regions_disjoint(lb, ls, rb, rs));
                // Adjacent
                prop_assert!(region_adjacent(lb, ls, rb, rs));
                // Sizes sum
                prop_assert_eq!(ls + rs, size);
                // Merge recovers original
                let (b, s) = region_merge(lb, ls, rb, rs);
                prop_assert_eq!((b, s), (base, size));
            }
        }

        // r[verify region.contains-addr]
        // r[verify region.split]
        #[test]
        fn prop_split_covers(
            base in 0u64..=u64::MAX / 2,
            size in 2u64..=100,
        ) {
            let offset = 1u64;
            if u128::from(base) + u128::from(size) <= u128::from(u64::MAX) {
                let ((lb, ls), (rb, rs)) = region_split(base, size, offset);
                for x in base..base + size.min(50) {
                    let in_left = contains_addr(lb, ls, x);
                    let in_right = contains_addr(rb, rs, x);
                    prop_assert!(in_left || in_right);
                    prop_assert!(!(in_left && in_right));
                }
            }
        }

        // r[verify region.contains-region]
        #[test]
        fn prop_contains_region_reflexive(
            base in 0u64..=u64::MAX / 2,
            size in 0u64..=1000,
        ) {
            if u128::from(base) + u128::from(size) <= u128::from(u64::MAX) {
                prop_assert!(contains_region(base, size, base, size));
            }
        }

        // r[verify region.contains-addr]
        #[test]
        fn prop_empty_contains_nothing(base: u64, x: u64) {
            // Empty region contains nothing
            prop_assert!(!contains_addr(base, 0, x));
        }

        // r[verify region.contains-addr]
        // r[verify region.disjoint]
        /// Model oracle: region contains iff interval contains.
        #[test]
        fn prop_model_oracle(
            base in 0u64..=u64::MAX / 2,
            size in 0u64..=1000,
            x: u64,
        ) {
            if u128::from(base) + u128::from(size) <= u128::from(u64::MAX) {
                // Region contains matches manual check
                let expected = x >= base && x < base + size;
                prop_assert_eq!(contains_addr(base, size, x), expected);
            }
        }
    }
}
