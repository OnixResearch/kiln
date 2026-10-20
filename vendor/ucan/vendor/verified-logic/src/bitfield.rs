//! Verified multi-bit field extraction and insertion.
//!
//! Operates on u64 words. Each field is defined by an `offset` (bit position
//! of the least-significant bit) and a `width` (number of bits). All functions
//! require `offset + width <= 64`.
//!
//! Four core operations:
//! - `field_mask` — bitmask for bits [offset, offset+width)
//! - `extract_field` — read a field, right-shifted to bit 0
//! - `insert_field` — write a value into a field, preserving other bits
//! - `clear_field` — zero a field, preserving other bits
//!
//! Five verified properties: round-trip, non-interference, clear-then-extract,
//! extract-bounded, insert-idempotent.

use vstd::prelude::*;

verus! {

// --- field_mask ---

// r[depends bitfield.field-mask]
/// Spec: bitmask with `width` consecutive 1-bits starting at `offset`.
pub open spec fn field_mask_spec(offset: u64, width: u64) -> u64
    recommends offset + width <= 64
{
    if width == 0 {
        0u64
    } else {
        (sub(1u64 << width, 1u64)) << offset
    }
}

// r[impl bitfield.field-mask]
/// Returns a bitmask covering bits [`offset`, `offset`+`width`).
pub fn field_mask(offset: u64, width: u64) -> (result: u64)
    requires offset + width <= 64, width > 0, width <= 63
    ensures result == field_mask_spec(offset, width)
{
    assert(width <= 63 ==> (1u64 << width) >= 1u64) by(bit_vector);
    ((1u64 << width) - 1) << offset
}

// --- extract_field ---

// r[depends bitfield.extract-field]
/// Spec: extract bits [`offset`, `offset`+`width`) right-shifted to bit 0.
pub open spec fn extract_field_spec(value: u64, offset: u64, width: u64) -> u64
    recommends offset + width <= 64
{
    if width == 0 {
        0u64
    } else {
        (value >> offset) & sub(1u64 << width, 1u64)
    }
}

// r[impl bitfield.extract-field]
/// Extracts the field at [`offset`, `offset`+`width`) from `value`.
pub fn extract_field(value: u64, offset: u64, width: u64) -> (result: u64)
    requires offset + width <= 64, width > 0, width <= 63
    ensures result == extract_field_spec(value, offset, width)
{
    assert(width <= 63 ==> (1u64 << width) >= 1u64) by(bit_vector);
    (value >> offset) & ((1u64 << width) - 1)
}

// --- insert_field ---

// r[depends bitfield.insert-field]
/// Spec: replace bits [`offset`, `offset`+`width`) with `field`, preserve the rest.
pub open spec fn insert_field_spec(value: u64, field: u64, offset: u64, width: u64) -> u64
    recommends offset + width <= 64
{
    if width == 0 {
        value
    } else {
        (value & !field_mask_spec(offset, width)) | ((field & sub(1u64 << width, 1u64)) << offset)
    }
}

// r[impl bitfield.insert-field]
/// Inserts `field` into bits [`offset`, `offset`+`width`) of `value`.
pub fn insert_field(value: u64, field: u64, offset: u64, width: u64) -> (result: u64)
    requires offset + width <= 64, width > 0, width <= 63
    ensures result == insert_field_spec(value, field, offset, width)
{
    assert(width <= 63 ==> (1u64 << width) >= 1u64) by(bit_vector);
    let mask = field_mask(offset, width);
    (value & !mask) | ((field & ((1u64 << width) - 1)) << offset)
}

// --- clear_field ---

// r[depends bitfield.clear-field]
/// Spec: zero bits [`offset`, `offset`+`width`), preserve the rest.
pub open spec fn clear_field_spec(value: u64, offset: u64, width: u64) -> u64
    recommends offset + width <= 64
{
    if width == 0 {
        value
    } else {
        value & !field_mask_spec(offset, width)
    }
}

// r[impl bitfield.clear-field]
/// Zeros bits [`offset`, `offset`+`width`) of `value`.
pub fn clear_field(value: u64, offset: u64, width: u64) -> (result: u64)
    requires offset + width <= 64, width > 0, width <= 63
    ensures result == clear_field_spec(value, offset, width)
{
    let mask = field_mask(offset, width);
    value & !mask
}

// --- Proofs ---

// r[impl bitfield.round-trip]
// r[verify bitfield.round-trip]
/// Helper: cleared region extracts to zero.
proof fn clear_extracts_zero(value: u64, offset: u64, width: u64)
    requires offset + width <= 64, width <= 63, width > 0
    ensures ((value & !(sub(1u64 << width, 1u64) << offset)) >> offset) & sub(1u64 << width, 1u64) == 0u64
{
    assert(
        ((value & !(sub(1u64 << width, 1u64) << offset)) >> offset) & sub(1u64 << width, 1u64) == 0u64
    ) by(bit_vector)
        requires offset + width <= 64, width <= 63, width > 0;
}

/// Helper: shift-left then shift-right recovers field within mask.
proof fn shift_roundtrip(field: u64, offset: u64, width: u64)
    requires
        offset + width <= 64, width <= 63, width > 0,
        field & sub(1u64 << width, 1u64) == field,
    ensures (((field & sub(1u64 << width, 1u64)) << offset) >> offset) & sub(1u64 << width, 1u64) == field
{
    assert(
        (((field & sub(1u64 << width, 1u64)) << offset) >> offset) & sub(1u64 << width, 1u64) == field
    ) by(bit_vector)
        requires offset + width <= 64, width <= 63, width > 0,
                 field & sub(1u64 << width, 1u64) == field;
}

/// Helper: OR distributes through extract when regions don't overlap.
proof fn or_extract_distribute(a: u64, b: u64, offset: u64, width: u64)
    requires offset + width <= 64, width <= 63, width > 0
    ensures
        ((a | b) >> offset) & sub(1u64 << width, 1u64)
            == (((a >> offset) & sub(1u64 << width, 1u64)) | ((b >> offset) & sub(1u64 << width, 1u64)))
{
    assert(
        ((a | b) >> offset) & sub(1u64 << width, 1u64)
            == (((a >> offset) & sub(1u64 << width, 1u64)) | ((b >> offset) & sub(1u64 << width, 1u64)))
    ) by(bit_vector)
        requires offset + width <= 64, width <= 63, width > 0;
}

/// Round-trip: extract(insert(v, f, o, w), o, w) == f when f fits in width bits.
proof fn round_trip(value: u64, field: u64, offset: u64, width: u64)
    requires
        offset + width <= 64,
        width <= 63,
        width > 0,
        field & sub(1u64 << width, 1u64) == field,
    ensures extract_field_spec(insert_field_spec(value, field, offset, width), offset, width) == field
{
    let wmask = sub(1u64 << width, 1u64);
    let mask = wmask << offset;
    let a = value & !mask;
    let b = (field & wmask) << offset;
    // Step 1: OR distributes through extract
    or_extract_distribute(a, b, offset, width);
    // Now: ((a | b) >> offset) & wmask == ((a >> offset) & wmask) | ((b >> offset) & wmask)
    // Step 2: cleared part extracts to 0
    clear_extracts_zero(value, offset, width);
    // Now: (a >> offset) & wmask == 0
    // Step 3: shifted field recovers original
    shift_roundtrip(field, offset, width);
    // Now: (b >> offset) & wmask == field
    // Combine: 0 | field == field
    assert(0u64 | field == field) by(bit_vector);
}

// r[impl bitfield.non-interference]
// r[verify bitfield.non-interference]
/// Helper: bit p of (v & !mask) equals bit p of v when p is outside [offset, offset+width).
proof fn clear_preserves_outside(value: u64, offset: u64, width: u64, p: u64)
    requires offset + width <= 64, width <= 63, width > 0, p < 64, p < offset || p >= offset + width
    ensures ((value & !(sub(1u64 << width, 1u64) << offset)) >> p) & 1u64 == (value >> p) & 1u64
{
    assert(
        ((value & !(sub(1u64 << width, 1u64) << offset)) >> p) & 1u64 == (value >> p) & 1u64
    ) by(bit_vector)
        requires offset + width <= 64, width <= 63, width > 0, p < 64, p < offset || p >= offset + width;
}

/// Helper: bit p of ((f & wmask) << offset) is 0 when p is outside [offset, offset+width).
proof fn shifted_field_zero_outside(field: u64, offset: u64, width: u64, p: u64)
    requires offset + width <= 64, width <= 63, width > 0, p < 64, p < offset || p >= offset + width
    ensures ((((field & sub(1u64 << width, 1u64)) << offset)) >> p) & 1u64 == 0u64
{
    assert(
        (((field & sub(1u64 << width, 1u64)) << offset) >> p) & 1u64 == 0u64
    ) by(bit_vector)
        requires offset + width <= 64, width <= 63, width > 0, p < 64, p < offset || p >= offset + width;
}

/// Non-interference: insert preserves bits outside the field.
proof fn non_interference(value: u64, field: u64, offset: u64, width: u64, p: u64)
    requires
        offset + width <= 64,
        width <= 63,
        width > 0,
        p < 64,
        p < offset || p >= offset + width,
    ensures
        (insert_field_spec(value, field, offset, width) >> p) & 1u64
            == (value >> p) & 1u64
{
    let wmask = sub(1u64 << width, 1u64);
    let a = value & !(wmask << offset);
    let b = (field & wmask) << offset;
    // OR distributes through single-bit extract
    assert(((a | b) >> p) & 1u64 == ((a >> p) & 1u64) | ((b >> p) & 1u64)) by(bit_vector);
    // Cleared value preserves bit p outside field
    clear_preserves_outside(value, offset, width, p);
    // Shifted field has bit p == 0 outside field
    shifted_field_zero_outside(field, offset, width, p);
    // Combine: original_bit | 0 == original_bit
    assert(((value >> p) & 1u64) | 0u64 == (value >> p) & 1u64) by(bit_vector);
}

// r[impl bitfield.clear-then-extract]
// r[verify bitfield.clear-then-extract]
/// Clear-then-extract: extracting from a cleared field gives 0.
proof fn clear_then_extract(value: u64, offset: u64, width: u64)
    requires
        offset + width <= 64,
        width <= 63,
        width > 0,
    ensures extract_field_spec(clear_field_spec(value, offset, width), offset, width) == 0
{
    // Reuses the clear_extracts_zero helper (which verifies by bit_vector)
    clear_extracts_zero(value, offset, width);
}

// r[impl bitfield.extract-bounded]
// r[verify bitfield.extract-bounded]
/// Extract bounded: result fits in width bits.
proof fn extract_bounded(value: u64, offset: u64, width: u64)
    requires
        offset + width <= 64,
        width <= 63,
        width > 0,
    ensures extract_field_spec(value, offset, width) & sub(1u64 << width, 1u64) == extract_field_spec(value, offset, width)
{
    assert(
        ((value >> offset) & sub(1u64 << width, 1u64)) & sub(1u64 << width, 1u64)
            == (value >> offset) & sub(1u64 << width, 1u64)
    ) by(bit_vector);
}

// r[verify bitfield.insert-field]
/// Helper: shifted field has no bits outside the mask region.
proof fn shifted_field_outside_mask_zero(field: u64, offset: u64, width: u64)
    requires offset + width <= 64, width <= 63, width > 0
    ensures ((field & sub(1u64 << width, 1u64)) << offset) & !(sub(1u64 << width, 1u64) << offset) == 0u64
{
    assert(
        ((field & sub(1u64 << width, 1u64)) << offset) & !(sub(1u64 << width, 1u64) << offset) == 0u64
    ) by(bit_vector)
        requires offset + width <= 64, width <= 63, width > 0;
}

/// Insert idempotent: inserting the same field twice is the same as once.
/// Key decomposition: insert(x,f,o,w) = (x & !mask) | f_shifted.
/// After first insert: only bits outside mask come from x.
/// Second insert: clears same mask region, writes same field → identical result.
proof fn insert_idempotent(value: u64, field: u64, offset: u64, width: u64)
    requires
        offset + width <= 64,
        width <= 63,
        width > 0,
    ensures
        insert_field_spec(insert_field_spec(value, field, offset, width), field, offset, width)
            == insert_field_spec(value, field, offset, width)
{
    // Step 1: f_shifted has no bits outside mask
    shifted_field_outside_mask_zero(field, offset, width);

    // Step 2: AND distributes over OR
    assert(
        ((value & !(sub(1u64 << width, 1u64) << offset))
         | ((field & sub(1u64 << width, 1u64)) << offset))
        & !(sub(1u64 << width, 1u64) << offset)
        == (value & !(sub(1u64 << width, 1u64) << offset)) & !(sub(1u64 << width, 1u64) << offset)
         | (((field & sub(1u64 << width, 1u64)) << offset) & !(sub(1u64 << width, 1u64) << offset))
    ) by(bit_vector);

    // Step 3: & !mask is idempotent
    assert(
        (value & !(sub(1u64 << width, 1u64) << offset)) & !(sub(1u64 << width, 1u64) << offset)
        == value & !(sub(1u64 << width, 1u64) << offset)
    ) by(bit_vector);

    // Step 4: Connect to spec fns.
    // Z3 needs: once & !mask == value & !mask (the only thing insert reads from x is x & !mask)
    // Since steps 1+2+3 prove ((v&!M)|f_s)&!M == (v&!M), assert it directly:
    let wmask = sub(1u64 << width, 1u64);
    let mask = wmask << offset;
    let f_s = (field & wmask) << offset;
    let cleared = value & !mask;
    // Z3 knows from step 1: f_s & !mask == 0
    // Z3 knows from step 2: (cleared | f_s) & !mask == (cleared & !mask) | (f_s & !mask)
    // Z3 knows from step 3: cleared & !mask == cleared
    // Chain: (cleared | f_s) & !mask == cleared | 0 == cleared
    // Spell it out for Z3:
    assert((cleared | f_s) & !mask == cleared) by {
        // distribute
        assert((cleared | f_s) & !mask == (cleared & !mask) | (f_s & !mask));
        // idempotent: cleared & !mask == cleared
        assert(cleared & !mask == cleared) by {
            assert((value & !(sub(1u64 << width, 1u64) << offset)) & !(sub(1u64 << width, 1u64) << offset)
                == value & !(sub(1u64 << width, 1u64) << offset)) by(bit_vector);
        };
        // zero: f_s & !mask == 0
        assert(f_s & !mask == 0u64);
        // combine
        assert(cleared | 0u64 == cleared) by(bit_vector);
    };

    // Now Z3 can see: once == cleared | f_s, and once & !mask == cleared
    // So insert(once, f) == (once & !mask) | f_s == cleared | f_s == once
    assert(field_mask_spec(offset, width) == mask);
}

// r[verify bitfield.clear-field]
/// Clear idempotent: clearing twice is the same as clearing once.
proof fn clear_idempotent(value: u64, offset: u64, width: u64)
    requires
        offset + width <= 64,
        width <= 63,
        width > 0,
    ensures
        clear_field_spec(clear_field_spec(value, offset, width), offset, width)
            == clear_field_spec(value, offset, width)
{
    assert(
        (value & !(sub(1u64 << width, 1u64) << offset)) & !(sub(1u64 << width, 1u64) << offset)
            == value & !(sub(1u64 << width, 1u64) << offset)
    ) by(bit_vector);
}

// r[verify bitfield.field-mask]
/// Zero-width mask is zero.
proof fn zero_width_mask(offset: u64)
    requires offset <= 64
    ensures field_mask_spec(offset, 0) == 0
{}

// r[verify bitfield.extract-field]
/// Zero-width extract is zero.
proof fn zero_width_extract(value: u64, offset: u64)
    requires offset <= 64
    ensures extract_field_spec(value, offset, 0) == 0
{}

// r[verify bitfield.insert-field]
/// Zero-width insert is identity.
proof fn zero_width_insert(value: u64, field: u64, offset: u64)
    requires offset <= 64
    ensures insert_field_spec(value, field, offset, 0) == value
{}

// r[verify bitfield.clear-field]
/// Zero-width clear is identity.
proof fn zero_width_clear(value: u64, offset: u64)
    requires offset <= 64
    ensures clear_field_spec(value, offset, 0) == value
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify bitfield.field-mask]
    #[test]
    fn test_field_mask() {
        assert_eq!(field_mask(0, 1), 0x1);
        assert_eq!(field_mask(0, 4), 0xF);
        assert_eq!(field_mask(4, 4), 0xF0);
        assert_eq!(field_mask(0, 8), 0xFF);
        assert_eq!(field_mask(8, 8), 0xFF00);
        assert_eq!(field_mask(0, 63), u64::MAX >> 1_u32);
    }

    // r[verify bitfield.extract-field]
    #[test]
    fn test_extract_field() {
        assert_eq!(extract_field(0xFF, 4, 4), 0xF);
        assert_eq!(extract_field(0xABCD, 8, 8), 0xAB);
        assert_eq!(extract_field(0xABCD, 0, 8), 0xCD);
        assert_eq!(extract_field(0xABCD, 4, 8), 0xBC);
    }

    // r[verify bitfield.insert-field]
    #[test]
    fn test_insert_field() {
        assert_eq!(insert_field(0, 0xA, 4, 4), 0xA0);
        assert_eq!(insert_field(0xFF, 0x5, 4, 4), 0x5F);
        assert_eq!(insert_field(0, 0xFF, 0, 8), 0xFF);
        assert_eq!(insert_field(0xFFFF, 0, 8, 8), 0x00FF);
    }

    // r[verify bitfield.clear-field]
    #[test]
    fn test_clear_field() {
        assert_eq!(clear_field(0xFF, 4, 4), 0x0F);
        assert_eq!(clear_field(0xFFFF, 8, 8), 0x00FF);
        assert_eq!(clear_field(0xABCD, 0, 8), 0xAB00);
    }

    proptest! {
        // r[verify bitfield.round-trip]
        #[test]
        fn prop_round_trip(
            value: u64,
            field: u64,
            width in 1u64..63,
            offset in 0u64..63,
        ) {
            prop_assume!(offset + width <= 64);
            let masked_field = field & ((1u64 << width) - 1);
            let inserted = insert_field(value, masked_field, offset, width);
            let extracted = extract_field(inserted, offset, width);
            prop_assert_eq!(extracted, masked_field);
        }

        // r[verify bitfield.non-interference]
        #[test]
        fn prop_non_interference(
            value: u64,
            field: u64,
            width in 1u64..63,
            offset in 0u64..63,
        ) {
            prop_assume!(offset + width <= 64);
            let inserted = insert_field(value, field, offset, width);
            // Check bits outside the field are unchanged
            for p in 0..64u64 {
                if p < offset || p >= offset + width {
                    prop_assert_eq!(
                        (inserted >> p) & 1,
                        (value >> p) & 1,
                        "bit {} changed by insert at offset={} width={}", p, offset, width
                    );
                }
            }
        }

        // r[verify bitfield.clear-then-extract]
        #[test]
        fn prop_clear_then_extract(
            value: u64,
            width in 1u64..63,
            offset in 0u64..63,
        ) {
            prop_assume!(offset + width <= 64);
            let cleared = clear_field(value, offset, width);
            let extracted = extract_field(cleared, offset, width);
            prop_assert_eq!(extracted, 0);
        }

        // r[verify bitfield.extract-bounded]
        #[test]
        fn prop_extract_bounded(
            value: u64,
            width in 1u64..63,
            offset in 0u64..63,
        ) {
            prop_assume!(offset + width <= 64);
            let extracted = extract_field(value, offset, width);
            let mask = (1u64 << width) - 1;
            prop_assert_eq!(extracted & mask, extracted, "extract not bounded");
        }

        // r[verify bitfield.insert-field]
        #[test]
        fn prop_insert_idempotent(
            value: u64,
            field: u64,
            width in 1u64..63,
            offset in 0u64..63,
        ) {
            prop_assume!(offset + width <= 64);
            let once = insert_field(value, field, offset, width);
            let twice = insert_field(once, field, offset, width);
            prop_assert_eq!(once, twice);
        }

        // r[verify bitfield.clear-field]
        #[test]
        fn prop_clear_idempotent(
            value: u64,
            width in 1u64..63,
            offset in 0u64..63,
        ) {
            prop_assume!(offset + width <= 64);
            let once = clear_field(value, offset, width);
            let twice = clear_field(once, offset, width);
            prop_assert_eq!(once, twice);
        }
    }
}
