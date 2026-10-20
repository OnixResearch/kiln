//! Verified bitwise operations.
//!
//! Bit manipulation with machine-checked correctness. Proofs use Verus's
//! `by(bit_vector)` mode which switches Z3 to bit-vector theory for
//! precise reasoning about AND, OR, XOR, and shifts.

use vstd::prelude::*;

verus! {

// --- is_power_of_two ---

// r[depends bitwise.is-power-of-two]
/// Spec: x is a power of two (exactly one bit set).
pub open spec fn is_power_of_two_spec(x: u64) -> bool {
    x != 0 && (x & sub(x, 1)) == 0
}

// r[impl bitwise.is-power-of-two]
/// Returns true if `x` is a power of two.
///
/// Uses the classic bit trick: a power of two has exactly one bit set,
/// so `x & (x - 1)` clears that bit, giving zero.
pub fn is_power_of_two(x: u64) -> (result: bool)
    ensures result == is_power_of_two_spec(x)
{
    x != 0 && (x & (x - 1)) == 0
}

// r[verify bitwise.is-power-of-two]
proof fn power_of_two_examples()
    ensures
        is_power_of_two_spec(1),
        is_power_of_two_spec(2),
        is_power_of_two_spec(4),
        !is_power_of_two_spec(0),
        !is_power_of_two_spec(3),
{
    assert(1u64 & sub(1u64, 1) == 0u64) by(bit_vector);
    assert(2u64 & sub(2u64, 1) == 0u64) by(bit_vector);
    assert(4u64 & sub(4u64, 1) == 0u64) by(bit_vector);
    assert(3u64 & sub(3u64, 1) != 0u64) by(bit_vector);
}

// --- bit_at ---

// r[depends bitwise.bit-at]
/// Spec: the bit at position `pos` is set.
pub open spec fn bit_at_spec(x: u64, pos: u64) -> bool
    recommends pos < 64
{
    (x >> pos) & 1u64 == 1u64
}

// r[impl bitwise.bit-at]
/// Returns true if the bit at `pos` is set in `x`.
pub fn bit_at(x: u64, pos: u64) -> (result: bool)
    requires pos < 64
    ensures result == bit_at_spec(x, pos)
{
    (x >> pos) & 1 == 1
}

// --- set_bit ---

// r[depends bitwise.set-bit]
/// Spec: set the bit at position `pos`.
pub open spec fn set_bit_spec(x: u64, pos: u64) -> u64
    recommends pos < 64
{
    x | (1u64 << pos)
}

// r[impl bitwise.set-bit]
/// Returns `x` with the bit at `pos` set to 1.
pub fn set_bit(x: u64, pos: u64) -> (result: u64)
    requires pos < 64
    ensures result == set_bit_spec(x, pos)
{
    x | (1u64 << pos)
}

// r[verify bitwise.set-bit]
/// Proof: setting a bit makes it read as 1.
proof fn set_then_check(x: u64, pos: u64)
    requires pos < 64
    ensures bit_at_spec(set_bit_spec(x, pos), pos)
{
    assert(pos < 64 ==> ((x | (1u64 << pos)) >> pos) & 1u64 == 1u64) by(bit_vector);
}

// r[verify bitwise.set-bit]
/// Proof: setting a bit twice is the same as setting it once.
proof fn set_bit_idempotent(x: u64, pos: u64)
    requires pos < 64
    ensures set_bit_spec(set_bit_spec(x, pos), pos) == set_bit_spec(x, pos)
{
    assert(pos < 64 ==> (x | (1u64 << pos)) | (1u64 << pos) == x | (1u64 << pos)) by(bit_vector);
}

// --- clear_bit ---

// r[depends bitwise.clear-bit]
/// Spec: clear the bit at position `pos`.
pub open spec fn clear_bit_spec(x: u64, pos: u64) -> u64
    recommends pos < 64
{
    x & !(1u64 << pos)
}

// r[impl bitwise.clear-bit]
/// Returns `x` with the bit at `pos` set to 0.
pub fn clear_bit(x: u64, pos: u64) -> (result: u64)
    requires pos < 64
    ensures result == clear_bit_spec(x, pos)
{
    x & !(1u64 << pos)
}

// r[verify bitwise.clear-bit]
/// Proof: clearing a bit makes it read as 0.
proof fn clear_then_check(x: u64, pos: u64)
    requires pos < 64
    ensures !bit_at_spec(clear_bit_spec(x, pos), pos)
{
    assert(pos < 64 ==> ((x & !(1u64 << pos)) >> pos) & 1u64 == 0u64) by(bit_vector);
}

// r[verify bitwise.clear-bit]
/// Proof: clearing a bit twice is the same as clearing it once.
proof fn clear_bit_idempotent(x: u64, pos: u64)
    requires pos < 64
    ensures clear_bit_spec(clear_bit_spec(x, pos), pos) == clear_bit_spec(x, pos)
{
    assert(pos < 64 ==> (x & !(1u64 << pos)) & !(1u64 << pos) == x & !(1u64 << pos)) by(bit_vector);
}

// --- toggle_bit ---

// r[depends bitwise.toggle-bit]
/// Spec: toggle (flip) the bit at position `pos`.
pub open spec fn toggle_bit_spec(x: u64, pos: u64) -> u64
    recommends pos < 64
{
    x ^ (1u64 << pos)
}

// r[impl bitwise.toggle-bit]
/// Returns `x` with the bit at `pos` flipped.
pub fn toggle_bit(x: u64, pos: u64) -> (result: u64)
    requires pos < 64
    ensures result == toggle_bit_spec(x, pos)
{
    x ^ (1u64 << pos)
}

// r[verify bitwise.toggle-bit]
/// Proof: toggling twice returns the original value.
proof fn toggle_bit_involution(x: u64, pos: u64)
    requires pos < 64
    ensures toggle_bit_spec(toggle_bit_spec(x, pos), pos) == x
{
    assert(pos < 64 ==> (x ^ (1u64 << pos)) ^ (1u64 << pos) == x) by(bit_vector);
}

// --- is_sign_extended ---

// r[depends bitwise.is-sign-extended]
/// Spec: bits [63:width] are all copies of bit [width-1].
/// At full width (64), this is vacuously true.
pub open spec fn is_sign_extended_spec(value: u64, width: u64) -> bool
    recommends 1 <= width <= 64
{
    if width >= 64 {
        true
    } else {
        // The sign bit (bit width-1) determines what the upper bits should be.
        // If sign bit is 0, upper bits must all be 0: value >> width == 0.
        // If sign bit is 1, upper bits must all be 1: value >> width == (u64::MAX >> width).
        // Equivalently: arithmetic shift right by (width-1) gives all-0 or all-1,
        // which means the top (64-width) bits equal the sign bit repeated.
        let sign_bit = (value >> sub(width, 1u64)) & 1u64;
        if sign_bit == 0 {
            (value >> width) == 0u64
        } else {
            // All upper bits must be 1. There are (64 - width) upper bits.
            // value >> width should be (1 << (64 - width)) - 1 = all-ones in those bits.
            (value >> width) == sub(1u64 << sub(64u64, width), 1u64)
        }
    }
}

// r[impl bitwise.is-sign-extended]
/// Returns true if bits [63:width] are all copies of bit [width-1].
pub fn is_sign_extended(value: u64, width: u64) -> (result: bool)
    requires 1 <= width <= 64
    ensures result == is_sign_extended_spec(value, width)
{
    if width >= 64 {
        true
    } else {
        let shift: u64 = width - 1;
        let sign_bit = (value >> shift) & 1;
        proof {
            assert((value >> shift) & 1u64 == (value >> sub(width, 1u64)) & 1u64) by(bit_vector)
                requires 1 <= width < 64, shift == width - 1;
        }
        if sign_bit == 0 {
            (value >> width) == 0
        } else {
            let upper: u64 = 64 - width;
            proof {
                assert((1u64 << upper) - 1u64 == sub(1u64 << sub(64u64, width), 1u64)) by(bit_vector)
                    requires 1 <= width < 64, upper == 64 - width;
            }
            (value >> width) == (1u64 << upper) - 1
        }
    }
}

// r[verify bitwise.is-sign-extended]
/// Full width is always sign-extended.
proof fn sign_extended_full(value: u64)
    ensures is_sign_extended_spec(value, 64)
{}

// r[verify bitwise.is-sign-extended]
/// Zero is sign-extended at any width.
proof fn sign_extended_zero(width: u64)
    requires 1 <= width <= 64
    ensures is_sign_extended_spec(0, width)
{
    if width < 64 {
        assert(((0u64 >> sub(width, 1u64)) & 1u64) == 0u64) by(bit_vector)
            requires 1 <= width < 64;
        assert((0u64 >> width) == 0u64) by(bit_vector)
            requires width < 64;
    }
}

// r[verify bitwise.is-sign-extended]
/// All-ones is sign-extended at any width.
proof fn sign_extended_all_ones(width: u64)
    requires 1 <= width <= 64
    ensures is_sign_extended_spec(u64::MAX, width)
{
    if width < 64 {
        assert(((u64::MAX >> sub(width, 1u64)) & 1u64) == 1u64) by(bit_vector)
            requires 1 <= width < 64;
        assert((u64::MAX >> width) == sub(1u64 << sub(64u64, width), 1u64)) by(bit_vector)
            requires 1 <= width < 64;
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify bitwise.is-power-of-two]
    #[test]
    fn test_is_power_of_two() {
        assert!(is_power_of_two(1));
        assert!(is_power_of_two(2));
        assert!(is_power_of_two(4));
        assert!(is_power_of_two(1 << 63));
        assert!(!is_power_of_two(0));
        assert!(!is_power_of_two(3));
        assert!(!is_power_of_two(6));
        assert!(!is_power_of_two(u64::MAX));
    }

    // r[verify bitwise.bit-at]
    #[test]
    fn test_bit_at() {
        assert!(bit_at(0b1010, 1));
        assert!(bit_at(0b1010, 3));
        assert!(!bit_at(0b1010, 0));
        assert!(!bit_at(0b1010, 2));
        assert!(bit_at(1 << 63, 63));
    }

    // r[verify bitwise.set-bit]
    #[test]
    fn test_set_bit() {
        assert_eq!(set_bit(0, 0), 1);
        assert_eq!(set_bit(0, 3), 8);
        assert_eq!(set_bit(0b1010, 0), 0b1011);
        assert_eq!(set_bit(0b1010, 1), 0b1010); // already set
    }

    // r[verify bitwise.clear-bit]
    #[test]
    fn test_clear_bit() {
        assert_eq!(clear_bit(0b1010, 1), 0b1000);
        assert_eq!(clear_bit(0b1010, 0), 0b1010); // already clear
        assert_eq!(clear_bit(u64::MAX, 63), u64::MAX >> 1_u32);
    }

    // r[verify bitwise.toggle-bit]
    #[test]
    fn test_toggle_bit() {
        assert_eq!(toggle_bit(0, 0), 1);
        assert_eq!(toggle_bit(1, 0), 0);
        assert_eq!(toggle_bit(0b1010, 0), 0b1011);
        assert_eq!(toggle_bit(0b1010, 1), 0b1000);
    }

    // r[verify bitwise.is-sign-extended]
    #[test]
    fn test_is_sign_extended() {
        // Full width always true
        assert!(is_sign_extended(0, 64));
        assert!(is_sign_extended(u64::MAX, 64));
        assert!(is_sign_extended(42, 64));

        // Zero: sign-extended at any width
        assert!(is_sign_extended(0, 1));
        assert!(is_sign_extended(0, 39));

        // All-ones: sign-extended at any width
        assert!(is_sign_extended(u64::MAX, 1));
        assert!(is_sign_extended(u64::MAX, 39));

        // Sv39 canonical: bit 38 = 0, bits 63-39 = 0
        assert!(is_sign_extended(0x0000_003F_FFFF_FFFF, 39));
        // Sv39 canonical: bit 38 = 1, bits 63-39 = 1
        assert!(is_sign_extended(0xFFFF_FFC0_0000_0000, 39));
        // Sv39 non-canonical: bit 38 = 1, bits 63-39 = 0
        assert!(!is_sign_extended(0x0000_0040_0000_0000, 39));
        // Sv39 non-canonical: bit 38 = 0, bits 63-39 = 1
        assert!(!is_sign_extended(0xFFFF_FF80_0000_0000, 39));

        // Width 1: only 0 and u64::MAX (all-ones) are sign-extended
        assert!(is_sign_extended(0, 1));
        assert!(is_sign_extended(u64::MAX, 1));
        assert!(!is_sign_extended(1, 1)); // bit 0 = 1, but upper bits = 0
    }

    proptest! {
        // r[verify bitwise.is-power-of-two]
        #[test]
        fn prop_power_of_two_matches_stdlib(x: u64) {
            prop_assert_eq!(is_power_of_two(x), x.is_power_of_two());
        }

        // r[verify bitwise.set-bit]
        #[test]
        fn prop_set_then_read(x: u64, pos in 0u64..64) {
            prop_assert!(bit_at(set_bit(x, pos), pos));
        }

        // r[verify bitwise.clear-bit]
        #[test]
        fn prop_clear_then_read(x: u64, pos in 0u64..64) {
            prop_assert!(!bit_at(clear_bit(x, pos), pos));
        }

        // r[verify bitwise.set-bit]
        #[test]
        fn prop_set_idempotent(x: u64, pos in 0u64..64) {
            prop_assert_eq!(set_bit(set_bit(x, pos), pos), set_bit(x, pos));
        }

        // r[verify bitwise.clear-bit]
        #[test]
        fn prop_clear_idempotent(x: u64, pos in 0u64..64) {
            prop_assert_eq!(clear_bit(clear_bit(x, pos), pos), clear_bit(x, pos));
        }

        // r[verify bitwise.toggle-bit]
        #[test]
        fn prop_toggle_involution(x: u64, pos in 0u64..64) {
            prop_assert_eq!(toggle_bit(toggle_bit(x, pos), pos), x);
        }

        // r[verify bitwise.is-sign-extended]
        #[test]
        fn prop_sign_extended_full(value: u64) {
            prop_assert!(is_sign_extended(value, 64));
        }

        // r[verify bitwise.is-sign-extended]
        #[test]
        fn prop_sign_extended_zero(width in 1u64..=64) {
            prop_assert!(is_sign_extended(0, width));
        }

        // r[verify bitwise.is-sign-extended]
        #[test]
        fn prop_sign_extended_all_ones(width in 1u64..=64) {
            prop_assert!(is_sign_extended(u64::MAX, width));
        }

        // r[verify bitwise.is-sign-extended]
        #[test]
        fn prop_sign_extended_monotonic(value: u64, w1 in 1u64..=64, w2 in 1u64..=64) {
            if w2 >= w1 && is_sign_extended(value, w1) {
                prop_assert!(is_sign_extended(value, w2));
            }
        }

        // r[verify bitwise.is-sign-extended]
        #[test]
        fn prop_sign_extended_model_oracle(value: u64, width in 1u64..=63) {
            // Model: extract sign bit, check all upper bits match
            let sign_bit = (value >> (width - 1)) & 1;
            let expected = if sign_bit == 0 {
                // Upper bits should all be 0
                value >> width == 0
            } else {
                // Upper bits should all be 1
                let mask = u64::MAX << width;
                value & mask == mask
            };
            prop_assert_eq!(is_sign_extended(value, width), expected);
        }
    }
}
