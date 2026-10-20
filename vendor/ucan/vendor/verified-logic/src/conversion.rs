//! Verified integer type conversions.
//!
//! Safe narrowing, widening, and sign conversions with machine-checked
//! bounds. Every conversion that can fail returns Option — no silent
//! truncation, no undefined behavior. Verus proves every cast is within
//! the target type's range.

// Cast lints are allow'd here because verified casts are the module's
// entire purpose. Verus mechanically checks every one is safe.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_lossless,
    clippy::checked_conversions
)]

use vstd::prelude::*;

verus! {

// --- narrow_u32 ---

// r[depends conversion.narrow-u32]
/// Spec: checked narrowing from u64 to u32.
pub open spec fn narrow_u32_spec(x: u64) -> Option<u32> {
    if x <= u32::MAX as u64 {
        Some(x as u32)
    } else {
        None
    }
}

// r[impl conversion.narrow-u32]
/// Narrows a u64 to u32, returning None on overflow.
pub fn narrow_u32(x: u64) -> (result: Option<u32>)
    ensures result == narrow_u32_spec(x)
{
    if x <= u32::MAX as u64 {
        Some(x as u32)
    } else {
        None
    }
}

// r[verify conversion.narrow-u32]
/// Widening then narrowing a u32 is identity.
proof fn widen_narrow_roundtrip_u32(x: u32)
    ensures narrow_u32_spec(x as u64) == Some(x)
{}

// r[verify conversion.narrow-u32]
/// Values above u32::MAX are rejected.
proof fn narrow_u32_rejects(x: u64)
    requires x > u32::MAX as u64
    ensures narrow_u32_spec(x).is_none()
{}

// --- narrow_u16 ---

// r[depends conversion.narrow-u16]
/// Spec: checked narrowing from u64 to u16.
pub open spec fn narrow_u16_spec(x: u64) -> Option<u16> {
    if x <= u16::MAX as u64 {
        Some(x as u16)
    } else {
        None
    }
}

// r[impl conversion.narrow-u16]
/// Narrows a u64 to u16, returning None on overflow.
pub fn narrow_u16(x: u64) -> (result: Option<u16>)
    ensures result == narrow_u16_spec(x)
{
    if x <= u16::MAX as u64 {
        Some(x as u16)
    } else {
        None
    }
}

// r[verify conversion.narrow-u16]
/// Widening then narrowing a u16 is identity.
proof fn widen_narrow_roundtrip_u16(x: u16)
    ensures narrow_u16_spec(x as u64) == Some(x)
{}

// r[verify conversion.narrow-u16]
/// Values above u16::MAX are rejected.
proof fn narrow_u16_rejects(x: u64)
    requires x > u16::MAX as u64
    ensures narrow_u16_spec(x).is_none()
{}

// --- narrow_u8 ---

// r[depends conversion.narrow-u8]
/// Spec: checked narrowing from u64 to u8.
pub open spec fn narrow_u8_spec(x: u64) -> Option<u8> {
    if x <= u8::MAX as u64 {
        Some(x as u8)
    } else {
        None
    }
}

// r[impl conversion.narrow-u8]
/// Narrows a u64 to u8, returning None on overflow.
pub fn narrow_u8(x: u64) -> (result: Option<u8>)
    ensures result == narrow_u8_spec(x)
{
    if x <= u8::MAX as u64 {
        Some(x as u8)
    } else {
        None
    }
}

// r[verify conversion.narrow-u8]
/// Widening then narrowing a u8 is identity.
proof fn widen_narrow_roundtrip_u8(x: u8)
    ensures narrow_u8_spec(x as u64) == Some(x)
{}

// r[verify conversion.narrow-u8]
/// Values above u8::MAX are rejected.
proof fn narrow_u8_rejects(x: u64)
    requires x > u8::MAX as u64
    ensures narrow_u8_spec(x).is_none()
{}

// --- signed_to_unsigned ---

// r[depends conversion.signed-to-unsigned]
/// Spec: non-negative i64 to u64.
pub open spec fn signed_to_unsigned_spec(x: i64) -> Option<u64> {
    if x >= 0i64 {
        Some(x as u64)
    } else {
        None
    }
}

// r[impl conversion.signed-to-unsigned]
/// Converts a non-negative i64 to u64, returning None for negative values.
pub fn signed_to_unsigned(x: i64) -> (result: Option<u64>)
    ensures result == signed_to_unsigned_spec(x)
{
    if x >= 0 {
        Some(x as u64)
    } else {
        None
    }
}

// r[verify conversion.signed-to-unsigned]
/// Zero converts to zero.
proof fn signed_to_unsigned_zero()
    ensures signed_to_unsigned_spec(0i64) == Some(0u64)
{}

// r[verify conversion.signed-to-unsigned]
/// Negative values are rejected.
proof fn signed_to_unsigned_negative(x: i64)
    requires x < 0
    ensures signed_to_unsigned_spec(x).is_none()
{}

// --- unsigned_to_signed ---

// r[depends conversion.unsigned-to-signed]
/// Spec: u64 to i64 when the value fits.
pub open spec fn unsigned_to_signed_spec(x: u64) -> Option<i64> {
    if x <= i64::MAX as u64 {
        Some(x as i64)
    } else {
        None
    }
}

// r[impl conversion.unsigned-to-signed]
/// Converts a u64 to i64, returning None if the value exceeds `i64::MAX`.
pub fn unsigned_to_signed(x: u64) -> (result: Option<i64>)
    ensures result == unsigned_to_signed_spec(x)
{
    if x <= i64::MAX as u64 {
        Some(x as i64)
    } else {
        None
    }
}

// r[verify conversion.unsigned-to-signed]
/// Zero converts to zero.
proof fn unsigned_to_signed_zero()
    ensures unsigned_to_signed_spec(0u64) == Some(0i64)
{}

// r[verify conversion.unsigned-to-signed]
/// Values above i64::MAX are rejected.
proof fn unsigned_to_signed_rejects(x: u64)
    requires x > i64::MAX as u64
    ensures unsigned_to_signed_spec(x).is_none()
{}

// --- roundtrip proofs ---

// r[verify conversion.signed-to-unsigned]
// r[verify conversion.unsigned-to-signed]
/// Non-negative i64 survives i64 → u64 → i64 roundtrip.
proof fn signed_unsigned_roundtrip(x: i64)
    requires x >= 0
    ensures unsigned_to_signed_spec(
        signed_to_unsigned_spec(x).unwrap()
    ) == Some(x)
{}

// r[verify conversion.unsigned-to-signed]
// r[verify conversion.signed-to-unsigned]
/// Small u64 survives u64 → i64 → u64 roundtrip.
proof fn unsigned_signed_roundtrip(x: u64)
    requires x <= i64::MAX as u64
    ensures signed_to_unsigned_spec(
        unsigned_to_signed_spec(x).unwrap()
    ) == Some(x)
{}

// --- narrowing chain ---

// r[verify conversion.narrow-u8]
// r[verify conversion.narrow-u16]
// r[verify conversion.narrow-u32]
/// If a value fits in a smaller type, it fits in all larger types.
proof fn narrow_chain(x: u64)
    ensures
        narrow_u8_spec(x).is_some() ==> narrow_u16_spec(x).is_some(),
        narrow_u16_spec(x).is_some() ==> narrow_u32_spec(x).is_some(),
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify conversion.narrow-u32]
    #[test]
    fn test_narrow_u32() {
        assert_eq!(narrow_u32(0), Some(0u32));
        assert_eq!(narrow_u32(42), Some(42u32));
        assert_eq!(narrow_u32(u64::from(u32::MAX)), Some(u32::MAX));
        assert_eq!(narrow_u32(u64::from(u32::MAX) + 1), None);
        assert_eq!(narrow_u32(u64::MAX), None);
    }

    // r[verify conversion.narrow-u16]
    #[test]
    fn test_narrow_u16() {
        assert_eq!(narrow_u16(0), Some(0u16));
        assert_eq!(narrow_u16(u64::from(u16::MAX)), Some(u16::MAX));
        assert_eq!(narrow_u16(u64::from(u16::MAX) + 1), None);
        assert_eq!(narrow_u16(u64::MAX), None);
    }

    // r[verify conversion.narrow-u8]
    #[test]
    fn test_narrow_u8() {
        assert_eq!(narrow_u8(0), Some(0u8));
        assert_eq!(narrow_u8(255), Some(255u8));
        assert_eq!(narrow_u8(256), None);
        assert_eq!(narrow_u8(u64::MAX), None);
    }

    // r[verify conversion.signed-to-unsigned]
    #[test]
    fn test_signed_to_unsigned() {
        assert_eq!(signed_to_unsigned(0), Some(0u64));
        assert_eq!(signed_to_unsigned(42), Some(42u64));
        assert_eq!(signed_to_unsigned(i64::MAX), Some(9_223_372_036_854_775_807_u64));
        assert_eq!(signed_to_unsigned(-1), None);
        assert_eq!(signed_to_unsigned(i64::MIN), None);
    }

    // r[verify conversion.unsigned-to-signed]
    #[test]
    fn test_unsigned_to_signed() {
        assert_eq!(unsigned_to_signed(0), Some(0i64));
        assert_eq!(unsigned_to_signed(42), Some(42i64));
        assert_eq!(unsigned_to_signed(9_223_372_036_854_775_807), Some(i64::MAX));
        assert_eq!(unsigned_to_signed(9_223_372_036_854_775_808), None);
        assert_eq!(unsigned_to_signed(u64::MAX), None);
    }

    proptest! {
        // r[verify conversion.narrow-u32]
        #[test]
        fn prop_narrow_u32_roundtrip(x: u32) {
            prop_assert_eq!(narrow_u32(u64::from(x)), Some(x));
        }

        // r[verify conversion.narrow-u32]
        #[test]
        fn prop_narrow_u32_preserves(x: u64) {
            if let Some(v) = narrow_u32(x) {
                prop_assert_eq!(u64::from(v), x);
            }
        }

        // r[verify conversion.narrow-u16]
        #[test]
        fn prop_narrow_u16_roundtrip(x: u16) {
            prop_assert_eq!(narrow_u16(u64::from(x)), Some(x));
        }

        // r[verify conversion.narrow-u16]
        #[test]
        fn prop_narrow_u16_preserves(x: u64) {
            if let Some(v) = narrow_u16(x) {
                prop_assert_eq!(u64::from(v), x);
            }
        }

        // r[verify conversion.narrow-u8]
        #[test]
        fn prop_narrow_u8_roundtrip(x: u8) {
            prop_assert_eq!(narrow_u8(u64::from(x)), Some(x));
        }

        // r[verify conversion.narrow-u8]
        #[test]
        fn prop_narrow_u8_preserves(x: u64) {
            if let Some(v) = narrow_u8(x) {
                prop_assert_eq!(u64::from(v), x);
            }
        }

        // r[verify conversion.signed-to-unsigned]
        // r[verify conversion.unsigned-to-signed]
        #[test]
        fn prop_signed_unsigned_roundtrip(x in 0i64..=i64::MAX) {
            let u = signed_to_unsigned(x).unwrap();
            prop_assert_eq!(unsigned_to_signed(u), Some(x));
        }

        // r[verify conversion.unsigned-to-signed]
        // r[verify conversion.signed-to-unsigned]
        #[test]
        fn prop_unsigned_signed_roundtrip(x in 0u64..=9_223_372_036_854_775_807u64) {
            let s = unsigned_to_signed(x).unwrap();
            prop_assert_eq!(signed_to_unsigned(s), Some(x));
        }

        // r[verify conversion.narrow-u8]
        // r[verify conversion.narrow-u16]
        // r[verify conversion.narrow-u32]
        #[test]
        fn prop_narrow_chain(x: u64) {
            if narrow_u8(x).is_some() {
                prop_assert!(narrow_u16(x).is_some());
                prop_assert!(narrow_u32(x).is_some());
            }
            if narrow_u16(x).is_some() {
                prop_assert!(narrow_u32(x).is_some());
            }
        }
    }
}
