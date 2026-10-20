//! Verified size class bucketing and waste bounds.
//!
//! Real allocators round up requested sizes to size classes (power-of-two
//! buckets). This module defines the mapping from requested size to class,
//! computes internal fragmentation (waste), and proves waste bounds.
//! Delegates to [`buddy::order_for_size`] for the actual classification.

use vstd::prelude::*;

use crate::buddy::order_for_size;

verus! {

// --- class_to_size ---

// r[depends size-class.class-to-size]
/// Spec: actual byte size provided by class `c` (2^c).
pub open spec fn class_to_size_spec(c: u64) -> u64
    recommends c < 64
{
    (1u64 << c)
}

// r[impl size-class.class-to-size]
/// Returns 2^c — the actual allocation size for class c.
pub fn class_to_size(c: u64) -> (result: u64)
    requires c < 64
    ensures result == class_to_size_spec(c)
{
    1u64 << c
}

// --- size_to_class ---

// r[depends size-class.size-to-class]
/// Spec: size class index for a given request size.
/// Uses the same table as [`buddy::order_for_size_spec`].
pub open spec fn size_to_class_spec(size: u64) -> u64
    recommends size > 0
{
    if size <= 1 { 0u64 }
    else if size <= 2 { 1u64 }
    else if size <= 4 { 2u64 }
    else if size <= 8 { 3u64 }
    else if size <= 16 { 4u64 }
    else if size <= 32 { 5u64 }
    else if size <= 64 { 6u64 }
    else if size <= 128 { 7u64 }
    else if size <= 256 { 8u64 }
    else if size <= 512 { 9u64 }
    else if size <= 1024 { 10u64 }
    else if size <= 2048 { 11u64 }
    else if size <= 4096 { 12u64 }
    else if size <= 8192 { 13u64 }
    else if size <= 16384 { 14u64 }
    else if size <= 32768 { 15u64 }
    else if size <= 65536 { 16u64 }
    else { 64u64 }
}

// r[impl size-class.size-to-class]
/// Returns the minimum class index where class_to_size >= size.
/// Delegates to buddy::order_for_size — same shift-loop algorithm.
pub fn size_to_class(size: u64) -> (result: u64)
    requires size > 0, size <= (1u64 << 63u64)
    ensures result < 64, class_to_size_spec(result) >= size
{
    // buddy::order_for_size ensures result < 64 && block_size_spec(result) >= size.
    // block_size_spec(r) = 1u64 << r = class_to_size_spec(r), so the postcondition
    // transfers directly (both open specs unfold to the same expression).
    order_for_size(size)
}

// --- waste ---

// r[depends size-class.waste]
/// Spec: internal fragmentation — bytes wasted by rounding up.
pub open spec fn waste_spec(requested: u64, class: u64) -> int
    recommends class < 64
{
    class_to_size_spec(class) as int - requested as int
}

// r[impl size-class.waste]
/// Returns class_to_size(class) - requested.
pub fn waste(requested: u64, class: u64) -> (result: u64)
    requires class < 64, class_to_size_spec(class) >= requested
    ensures result as int == waste_spec(requested, class)
{
    class_to_size(class) - requested
}

// --- class_count ---

// r[depends size-class.class-count]
/// Spec: number of size classes needed to cover sizes up to max_size.
/// For power-of-two classes, this is size_to_class(max_size) + 1.
pub open spec fn class_count_spec(max_size: u64) -> u64
    recommends max_size > 0
{
    (size_to_class_spec(max_size) + 1) as u64
}

// r[impl size-class.class-count]
/// Returns the number of distinct size classes for sizes in [1, max_size].
pub fn class_count(max_size: u64) -> (result: u64)
    requires max_size > 0, max_size <= (1u64 << 63u64)
    ensures result >= 1, result <= 65
{
    size_to_class(max_size) + 1
}

// --- Proofs ---

// r[verify size-class.class-to-size]
/// Class 0 has size 1.
proof fn class_zero_is_one()
    ensures class_to_size_spec(0) == 1
{
    assert(1u64 << 0u64 == 1u64) by(bit_vector);
}

// r[verify size-class.class-to-size]
/// Class size is always positive.
proof fn class_size_positive(c: u64)
    requires c < 64
    ensures class_to_size_spec(c) >= 1
{
    assert(c < 64 ==> (1u64 << c) >= 1u64) by(bit_vector);
}

// r[verify size-class.class-to-size]
/// Class size doubles with each class.
proof fn class_size_doubles(c: u64)
    requires c < 63
    ensures class_to_size_spec((c + 1) as u64) == 2 * class_to_size_spec(c)
{
    assert(c < 63 ==> (1u64 << ((c + 1) as u64)) == 2u64 * (1u64 << c)) by(bit_vector);
}

// r[verify size-class.size-to-class]
/// Size class for powers of two is exact (small cases).
proof fn class_exact_powers()
    ensures
        size_to_class_spec(1) == 0,
        size_to_class_spec(2) == 1,
        size_to_class_spec(4) == 2,
        size_to_class_spec(8) == 3,
        size_to_class_spec(16) == 4,
        size_to_class_spec(1024) == 10,
        size_to_class_spec(4096) == 12,
{}

// r[verify size-class.size-to-class]
/// Monotonicity: larger request → same or larger class (small cases).
proof fn class_monotonic_small()
    ensures
        size_to_class_spec(1) <= size_to_class_spec(2),
        size_to_class_spec(2) <= size_to_class_spec(3),
        size_to_class_spec(3) <= size_to_class_spec(4),
        size_to_class_spec(4) <= size_to_class_spec(5),
        size_to_class_spec(100) <= size_to_class_spec(200),
{}

// r[verify size-class.size-to-class]
/// Roundtrip: size_to_class(class_to_size(c)) == c for small classes.
proof fn class_roundtrip_small()
    ensures
        size_to_class_spec(class_to_size_spec(0)) == 0,
        size_to_class_spec(class_to_size_spec(1)) == 1,
        size_to_class_spec(class_to_size_spec(2)) == 2,
        size_to_class_spec(class_to_size_spec(3)) == 3,
        size_to_class_spec(class_to_size_spec(4)) == 4,
        size_to_class_spec(class_to_size_spec(10)) == 10,
        size_to_class_spec(class_to_size_spec(12)) == 12,
        size_to_class_spec(class_to_size_spec(16)) == 16,
{
    // Each class_to_size_spec(k) = 2^k, which is a power of two.
    // The table maps 2^k → k for all k in the table range.
    assert(1u64 << 0u64 == 1u64) by(bit_vector);
    assert(1u64 << 1u64 == 2u64) by(bit_vector);
    assert(1u64 << 2u64 == 4u64) by(bit_vector);
    assert(1u64 << 3u64 == 8u64) by(bit_vector);
    assert(1u64 << 4u64 == 16u64) by(bit_vector);
    assert(1u64 << 10u64 == 1024u64) by(bit_vector);
    assert(1u64 << 12u64 == 4096u64) by(bit_vector);
    assert(1u64 << 16u64 == 65536u64) by(bit_vector);
}

// r[verify size-class.waste]
/// Waste is non-negative when class covers the request.
proof fn waste_nonneg(requested: u64, class: u64)
    requires class < 64, class_to_size_spec(class) >= requested
    ensures waste_spec(requested, class) >= 0
{}

// r[verify size-class.waste]
/// Waste is zero for exact power-of-two requests (small cases).
proof fn waste_zero_exact()
    ensures
        waste_spec(1, 0) == 0,
        waste_spec(2, 1) == 0,
        waste_spec(4, 2) == 0,
        waste_spec(8, 3) == 0,
{
    assert(1u64 << 0u64 == 1u64) by(bit_vector);
    assert(1u64 << 1u64 == 2u64) by(bit_vector);
    assert(1u64 << 2u64 == 4u64) by(bit_vector);
    assert(1u64 << 3u64 == 8u64) by(bit_vector);
}

// r[verify size-class.waste]
/// Waste is strictly less than the requested size (for size > 0).
/// For power-of-two classes: waste = 2^c - s where 2^(c-1) < s <= 2^c,
/// so waste < s.
proof fn waste_bounded_small()
    ensures
        // waste(3, class_for_3=2) = 4 - 3 = 1 < 3
        waste_spec(3, 2) < 3,
        // waste(5, class_for_5=3) = 8 - 5 = 3 < 5
        waste_spec(5, 3) < 5,
        // waste(1000, class_for_1000=10) = 1024 - 1000 = 24 < 1000
        waste_spec(1000, 10) < 1000,
{
    assert(1u64 << 2u64 == 4u64) by(bit_vector);
    assert(1u64 << 3u64 == 8u64) by(bit_vector);
    assert(1u64 << 10u64 == 1024u64) by(bit_vector);
}

// r[verify size-class.class-count]
/// At least one class for any positive max_size.
proof fn class_count_positive()
    ensures
        class_count_spec(1) >= 1,
        class_count_spec(100) >= 1,
        class_count_spec(65536) >= 1,
{}

// r[verify size-class.class-count]
/// class_count for small max values.
proof fn class_count_small()
    ensures
        class_count_spec(1) == 1,
        class_count_spec(2) == 2,
        class_count_spec(4) == 3,
        class_count_spec(256) == 9,
        class_count_spec(65536) == 17,
{}

} // verus!

#[cfg(test)]
#[allow(clippy::default_numeric_fallback)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify size-class.class-to-size]
    #[test]
    fn test_class_to_size() {
        assert_eq!(class_to_size(0), 1);
        assert_eq!(class_to_size(1), 2);
        assert_eq!(class_to_size(2), 4);
        assert_eq!(class_to_size(10), 1024);
        assert_eq!(class_to_size(20), 1_048_576);
    }

    // r[verify size-class.size-to-class]
    #[test]
    fn test_size_to_class() {
        assert_eq!(size_to_class(1), 0);
        assert!(class_to_size(size_to_class(1)) >= 1);
        assert!(class_to_size(size_to_class(3)) >= 3);
        assert!(class_to_size(size_to_class(1000)) >= 1000);
    }

    // r[verify size-class.waste]
    #[test]
    fn test_waste() {
        assert_eq!(waste(4, 2), 0); // exact fit
        assert_eq!(waste(3, 2), 1); // 4 - 3
        assert_eq!(waste(5, 3), 3); // 8 - 5
    }

    // r[verify size-class.class-count]
    #[test]
    fn test_class_count() {
        assert!(class_count(1) >= 1);
        assert!(class_count(65536) >= 1);
    }

    proptest! {
        // r[verify size-class.size-to-class]
        #[test]
        fn prop_class_big_enough(size in 1u64..=(1u64 << 32)) {
            let c = size_to_class(size);
            prop_assert!(class_to_size(c) >= size);
        }

        // r[verify size-class.waste]
        #[test]
        fn prop_waste_bounded(size in 1u64..=(1u64 << 32)) {
            let c = size_to_class(size);
            let w = waste(size, c);
            prop_assert!(w < size, "waste {} >= size {}", w, size);
        }

        // r[verify size-class.size-to-class]
        #[test]
        fn prop_monotonic(s1 in 1u64..=(1u64 << 31), s2 in 1u64..=(1u64 << 31)) {
            if s1 <= s2 {
                prop_assert!(size_to_class(s1) <= size_to_class(s2));
            }
        }

        // r[verify size-class.size-to-class]
        #[test]
        fn prop_roundtrip(c in 0u64..32) {
            let size = class_to_size(c);
            if size <= (1u64 << 63) {
                let c2 = size_to_class(size);
                prop_assert_eq!(class_to_size(c2), size);
            }
        }

        // r[verify size-class.class-to-size]
        #[test]
        fn prop_class_size_positive(c in 0u64..64) {
            prop_assert!(class_to_size(c) >= 1);
        }

        // r[verify size-class.class-to-size]
        #[test]
        fn prop_class_size_doubles(c in 0u64..63) {
            prop_assert_eq!(class_to_size(c + 1), 2 * class_to_size(c));
        }
    }
}
