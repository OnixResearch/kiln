//! Verified constant-time conditional selection.
//!
//! Branchless selection: returns one of two values based on a boolean
//! flag using bitwise masking. Pairs with `constant_time_eq` for
//! building constant-time cryptographic primitives.
//!
//! The exec functions use bitmask arithmetic `(a & mask) | (b & !mask)`
//! where mask is all-ones or all-zeros. Verus proves the mask algebra
//! equivalent to the branching spec via `by(bit_vector)`.

use vstd::prelude::*;

verus! {

// ============================================================
// ct_select_u64
// ============================================================

// r[depends ct-select.u64]
/// Spec: select a when flag is true, b when false.
pub open spec fn ct_select_u64_spec(flag: bool, a: u64, b: u64) -> u64 {
    if flag { a } else { b }
}

// r[impl ct-select.u64]
// r[impl ct-select.flag-true]
// r[impl ct-select.flag-false]
/// Constant-time select for u64. Uses bitmask arithmetic:
/// mask = all-ones (flag=true) or all-zeros (flag=false).
/// result = (a & mask) | (b & !mask).
pub fn ct_select_u64(flag: bool, a: u64, b: u64) -> (result: u64)
    ensures result == ct_select_u64_spec(flag, a, b),
{
    let mask: u64 = if flag { 0xFFFF_FFFF_FFFF_FFFFu64 } else { 0u64 };
    proof {
        if flag {
            assert((a & 0xFFFF_FFFF_FFFF_FFFFu64) | (b & !0xFFFF_FFFF_FFFF_FFFFu64) == a) by(bit_vector);
        } else {
            assert((a & 0u64) | (b & !0u64) == b) by(bit_vector);
        }
    }
    (a & mask) | (b & !mask)
}

// ============================================================
// ct_select_u8
// ============================================================

// r[depends ct-select.u8]
/// Spec: select a when flag is true, b when false.
pub open spec fn ct_select_u8_spec(flag: bool, a: u8, b: u8) -> u8 {
    if flag { a } else { b }
}

// r[impl ct-select.u8]
// r[impl ct-select.flag-true]
// r[impl ct-select.flag-false]
/// Constant-time select for u8. Same masking approach as u64.
pub fn ct_select_u8(flag: bool, a: u8, b: u8) -> (result: u8)
    ensures result == ct_select_u8_spec(flag, a, b),
{
    let mask: u8 = if flag { 0xFFu8 } else { 0u8 };
    proof {
        if flag {
            assert((a & 0xFFu8) | (b & !0xFFu8) == a) by(bit_vector);
        } else {
            assert((a & 0u8) | (b & !0u8) == b) by(bit_vector);
        }
    }
    (a & mask) | (b & !mask)
}

// ============================================================
// Proofs
// ============================================================

// r[verify ct-select.flag-true]
/// When flag is true, result is a.
proof fn select_u64_true(a: u64, b: u64)
    ensures ct_select_u64_spec(true, a, b) == a,
{}

// r[verify ct-select.flag-true]
/// When flag is true, result is a (u8 variant).
proof fn select_u8_true(a: u8, b: u8)
    ensures ct_select_u8_spec(true, a, b) == a,
{}

// r[verify ct-select.flag-false]
/// When flag is false, result is b.
proof fn select_u64_false(a: u64, b: u64)
    ensures ct_select_u64_spec(false, a, b) == b,
{}

// r[verify ct-select.flag-false]
/// When flag is false, result is b (u8 variant).
proof fn select_u8_false(a: u8, b: u8)
    ensures ct_select_u8_spec(false, a, b) == b,
{}

// r[verify ct-select.u64]
/// Both branches produce valid results (no undefined behavior).
proof fn select_u64_total(flag: bool, a: u64, b: u64)
    ensures ct_select_u64_spec(flag, a, b) == a || ct_select_u64_spec(flag, a, b) == b,
{}

// r[verify ct-select.u8]
/// Both branches produce valid results (u8 variant).
proof fn select_u8_total(flag: bool, a: u8, b: u8)
    ensures ct_select_u8_spec(flag, a, b) == a || ct_select_u8_spec(flag, a, b) == b,
{}

// r[verify ct-select.u64]
/// Selecting with the same value for both arms returns that value.
proof fn select_u64_same(flag: bool, v: u64)
    ensures ct_select_u64_spec(flag, v, v) == v,
{}

// r[verify ct-select.u8]
/// Selecting with the same value for both arms returns that value.
proof fn select_u8_same(flag: bool, v: u8)
    ensures ct_select_u8_spec(flag, v, v) == v,
{}

// r[verify ct-select.u64]
/// Flipping the flag swaps the result.
proof fn select_u64_flip(flag: bool, a: u64, b: u64)
    ensures ct_select_u64_spec(flag, a, b) == ct_select_u64_spec(!flag, b, a),
{}

// r[verify ct-select.u8]
/// Flipping the flag swaps the result (u8 variant).
proof fn select_u8_flip(flag: bool, a: u8, b: u8)
    ensures ct_select_u8_spec(flag, a, b) == ct_select_u8_spec(!flag, b, a),
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify ct-select.flag-true]
    #[test]
    fn test_select_u64_true() {
        assert_eq!(ct_select_u64(true, 42, 99), 42);
    }

    // r[verify ct-select.flag-false]
    #[test]
    fn test_select_u64_false() {
        assert_eq!(ct_select_u64(false, 42, 99), 99);
    }

    // r[verify ct-select.flag-true]
    #[test]
    fn test_select_u8_true() {
        assert_eq!(ct_select_u8(true, 0xAA, 0x55), 0xAA);
    }

    // r[verify ct-select.flag-false]
    #[test]
    fn test_select_u8_false() {
        assert_eq!(ct_select_u8(false, 0xAA, 0x55), 0x55);
    }

    // r[verify ct-select.u64]
    #[test]
    fn test_select_u64_extremes() {
        assert_eq!(ct_select_u64(true, u64::MAX, 0), u64::MAX);
        assert_eq!(ct_select_u64(false, u64::MAX, 0), 0);
        assert_eq!(ct_select_u64(true, 0, u64::MAX), 0);
        assert_eq!(ct_select_u64(false, 0, u64::MAX), u64::MAX);
    }

    // r[verify ct-select.u8]
    #[test]
    fn test_select_u8_extremes() {
        assert_eq!(ct_select_u8(true, 0xFF, 0x00), 0xFF);
        assert_eq!(ct_select_u8(false, 0xFF, 0x00), 0x00);
    }

    proptest! {
        // r[verify ct-select.u64]
        #[test]
        fn prop_select_u64_matches_branch(flag: bool, a: u64, b: u64) {
            let expected = if flag { a } else { b };
            prop_assert_eq!(ct_select_u64(flag, a, b), expected);
        }

        // r[verify ct-select.u8]
        #[test]
        fn prop_select_u8_matches_branch(flag: bool, a: u8, b: u8) {
            let expected = if flag { a } else { b };
            prop_assert_eq!(ct_select_u8(flag, a, b), expected);
        }

        // r[verify ct-select.u64]
        #[test]
        fn prop_select_u64_same_value(flag: bool, v: u64) {
            prop_assert_eq!(ct_select_u64(flag, v, v), v);
        }

        // r[verify ct-select.u8]
        #[test]
        fn prop_select_u8_same_value(flag: bool, v: u8) {
            prop_assert_eq!(ct_select_u8(flag, v, v), v);
        }

        // r[verify ct-select.u64]
        #[test]
        fn prop_select_u64_flip(flag: bool, a: u64, b: u64) {
            prop_assert_eq!(
                ct_select_u64(flag, a, b),
                ct_select_u64(!flag, b, a),
            );
        }

        // r[verify ct-select.u8]
        #[test]
        fn prop_select_u8_flip(flag: bool, a: u8, b: u8) {
            prop_assert_eq!(
                ct_select_u8(flag, a, b),
                ct_select_u8(!flag, b, a),
            );
        }
    }
}
