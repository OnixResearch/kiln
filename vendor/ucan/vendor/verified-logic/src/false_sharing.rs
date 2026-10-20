//! Verified false sharing detection.
//!
//! Models cache-line identity, alignment, padding-based separation,
//! object cache-line span, and equivalence with set-associative
//! address decomposition.

use vstd::prelude::*;

use crate::arithmetic::div_ceil;
#[cfg(verus_keep_ghost)]
use crate::arithmetic::div_ceil_spec;
#[cfg(verus_keep_ghost)]
use crate::set_associative_lookup::extract_index_spec;
#[cfg(verus_keep_ghost)]
use crate::set_associative_lookup::extract_tag_spec;
#[cfg(verus_keep_ghost)]
use crate::set_associative_lookup::low_mask_spec;

verus! {

// r[depends false-sharing.same-line]
/// Spec: addresses are in the same cache line when their line numbers agree.
pub open spec fn same_cache_line_spec(a: u64, b: u64, line_size: u64) -> bool
    recommends line_size > 0
{
    a / line_size == b / line_size
}

// r[depends false-sharing.alignment]
/// Spec: address is aligned to a cache-line boundary.
pub open spec fn is_line_aligned_spec(addr: u64, line_size: u64) -> bool
    recommends line_size > 0
{
    addr % line_size == 0u64
}

// r[depends false-sharing.alignment]
/// Spec: first byte of the containing cache line.
pub open spec fn line_start_spec(addr: u64, line_size: u64) -> int
    recommends line_size > 0
{
    (addr as int / line_size as int) * line_size as int
}

// r[depends false-sharing.padding]
/// Spec: object B starts at least one full cache line after the end of object A.
pub open spec fn padding_prevents_sharing_spec(
    addr_a: u64,
    size_a: u64,
    addr_b: u64,
    line_size: u64,
) -> bool
    recommends line_size > 0
{
    (addr_b as int) >= (addr_a as int) + (size_a as int) + (line_size as int)
}

// r[depends false-sharing.object-span]
/// Spec: positive-size object span via ceiling division of offset+size.
pub open spec fn cache_lines_spanned_spec(addr: u64, size: u64, line_size: u64) -> u64
    recommends
        line_size > 0,
        size > 0,
        ((addr as int) % (line_size as int)) + (size as int) <= u64::MAX as int,
{
    div_ceil_spec((((addr as int) % (line_size as int)) + (size as int)) as u64, line_size)
}

// r[impl false-sharing.decomposition-equivalence]
/// Spec: same-line is equivalent to canonical tag+index equality.
pub open spec fn decomposition_equivalent_spec(
    a: u64,
    b: u64,
    offset_bits: u64,
    index_bits: u64,
) -> bool
    recommends offset_bits < 64, offset_bits + index_bits <= 64
{
    same_cache_line_spec(a, b, 1u64 << offset_bits)
        == (
            extract_tag_spec(a, offset_bits, index_bits)
                == extract_tag_spec(b, offset_bits, index_bits)
            && extract_index_spec(a, offset_bits, index_bits)
                == extract_index_spec(b, offset_bits, index_bits)
        )
}

// r[impl false-sharing.distinct-line]
/// Spec: different lines imply a different canonical (tag,index) identity.
pub open spec fn distinct_line_guarantee_spec(
    a: u64,
    b: u64,
    offset_bits: u64,
    index_bits: u64,
) -> bool
    recommends offset_bits < 64, offset_bits + index_bits <= 64
{
    !same_cache_line_spec(a, b, 1u64 << offset_bits)
        ==> (
            extract_tag_spec(a, offset_bits, index_bits)
                != extract_tag_spec(b, offset_bits, index_bits)
            || extract_index_spec(a, offset_bits, index_bits)
                != extract_index_spec(b, offset_bits, index_bits)
        )
}

proof fn line_start_is_floor(addr: u64, line_size: u64)
    requires line_size > 0
    ensures line_start_spec(addr, line_size) == (addr as int / line_size as int) * line_size as int
{
}

proof fn same_line_implies_gap_lt_line(x: u64, y: u64, line_size: u64)
    requires
        line_size > 0,
        x <= y,
        same_cache_line_spec(x, y, line_size),
    ensures (y as int) < (x as int) + (line_size as int)
{
    vstd::arithmetic::div_mod::lemma_fundamental_div_mod(x as int, line_size as int);
    vstd::arithmetic::div_mod::lemma_fundamental_div_mod(y as int, line_size as int);
    assert((y as int) - (x as int) < (line_size as int)) by(nonlinear_arith)
        requires
            (line_size as int) > 0,
            (x as int) <= (y as int),
            (x as int) == ((x as int) / (line_size as int)) * (line_size as int) + ((x as int) % (line_size as int)),
            (y as int) == ((y as int) / (line_size as int)) * (line_size as int) + ((y as int) % (line_size as int)),
            ((x as int) / (line_size as int)) == ((y as int) / (line_size as int)),
            0 <= ((x as int) % (line_size as int)),
            0 <= ((y as int) % (line_size as int)),
            ((y as int) % (line_size as int)) < (line_size as int);
    assert((y as int) < (x as int) + (line_size as int)) by(nonlinear_arith)
        requires (y as int) - (x as int) < (line_size as int);
}

proof fn div_shift_equiv(addr: u64, offset_bits: u64)
    requires offset_bits < 64
    ensures addr / (1u64 << offset_bits) == addr >> offset_bits
{
    if offset_bits == 0 { assert(addr / (1u64 << 0u64) == addr >> 0u64) by(bit_vector); }
    else if offset_bits == 1 { assert(addr / (1u64 << 1u64) == addr >> 1u64) by(bit_vector); }
    else if offset_bits == 2 { assert(addr / (1u64 << 2u64) == addr >> 2u64) by(bit_vector); }
    else if offset_bits == 3 { assert(addr / (1u64 << 3u64) == addr >> 3u64) by(bit_vector); }
    else if offset_bits == 4 { assert(addr / (1u64 << 4u64) == addr >> 4u64) by(bit_vector); }
    else if offset_bits == 5 { assert(addr / (1u64 << 5u64) == addr >> 5u64) by(bit_vector); }
    else if offset_bits == 6 { assert(addr / (1u64 << 6u64) == addr >> 6u64) by(bit_vector); }
    else if offset_bits == 7 { assert(addr / (1u64 << 7u64) == addr >> 7u64) by(bit_vector); }
    else if offset_bits == 8 { assert(addr / (1u64 << 8u64) == addr >> 8u64) by(bit_vector); }
    else if offset_bits == 9 { assert(addr / (1u64 << 9u64) == addr >> 9u64) by(bit_vector); }
    else if offset_bits == 10 { assert(addr / (1u64 << 10u64) == addr >> 10u64) by(bit_vector); }
    else if offset_bits == 11 { assert(addr / (1u64 << 11u64) == addr >> 11u64) by(bit_vector); }
    else if offset_bits == 12 { assert(addr / (1u64 << 12u64) == addr >> 12u64) by(bit_vector); }
    else if offset_bits == 13 { assert(addr / (1u64 << 13u64) == addr >> 13u64) by(bit_vector); }
    else if offset_bits == 14 { assert(addr / (1u64 << 14u64) == addr >> 14u64) by(bit_vector); }
    else if offset_bits == 15 { assert(addr / (1u64 << 15u64) == addr >> 15u64) by(bit_vector); }
    else if offset_bits == 16 { assert(addr / (1u64 << 16u64) == addr >> 16u64) by(bit_vector); }
    else if offset_bits == 17 { assert(addr / (1u64 << 17u64) == addr >> 17u64) by(bit_vector); }
    else if offset_bits == 18 { assert(addr / (1u64 << 18u64) == addr >> 18u64) by(bit_vector); }
    else if offset_bits == 19 { assert(addr / (1u64 << 19u64) == addr >> 19u64) by(bit_vector); }
    else if offset_bits == 20 { assert(addr / (1u64 << 20u64) == addr >> 20u64) by(bit_vector); }
    else if offset_bits == 21 { assert(addr / (1u64 << 21u64) == addr >> 21u64) by(bit_vector); }
    else if offset_bits == 22 { assert(addr / (1u64 << 22u64) == addr >> 22u64) by(bit_vector); }
    else if offset_bits == 23 { assert(addr / (1u64 << 23u64) == addr >> 23u64) by(bit_vector); }
    else if offset_bits == 24 { assert(addr / (1u64 << 24u64) == addr >> 24u64) by(bit_vector); }
    else if offset_bits == 25 { assert(addr / (1u64 << 25u64) == addr >> 25u64) by(bit_vector); }
    else if offset_bits == 26 { assert(addr / (1u64 << 26u64) == addr >> 26u64) by(bit_vector); }
    else if offset_bits == 27 { assert(addr / (1u64 << 27u64) == addr >> 27u64) by(bit_vector); }
    else if offset_bits == 28 { assert(addr / (1u64 << 28u64) == addr >> 28u64) by(bit_vector); }
    else if offset_bits == 29 { assert(addr / (1u64 << 29u64) == addr >> 29u64) by(bit_vector); }
    else if offset_bits == 30 { assert(addr / (1u64 << 30u64) == addr >> 30u64) by(bit_vector); }
    else if offset_bits == 31 { assert(addr / (1u64 << 31u64) == addr >> 31u64) by(bit_vector); }
    else if offset_bits == 32 { assert(addr / (1u64 << 32u64) == addr >> 32u64) by(bit_vector); }
    else if offset_bits == 33 { assert(addr / (1u64 << 33u64) == addr >> 33u64) by(bit_vector); }
    else if offset_bits == 34 { assert(addr / (1u64 << 34u64) == addr >> 34u64) by(bit_vector); }
    else if offset_bits == 35 { assert(addr / (1u64 << 35u64) == addr >> 35u64) by(bit_vector); }
    else if offset_bits == 36 { assert(addr / (1u64 << 36u64) == addr >> 36u64) by(bit_vector); }
    else if offset_bits == 37 { assert(addr / (1u64 << 37u64) == addr >> 37u64) by(bit_vector); }
    else if offset_bits == 38 { assert(addr / (1u64 << 38u64) == addr >> 38u64) by(bit_vector); }
    else if offset_bits == 39 { assert(addr / (1u64 << 39u64) == addr >> 39u64) by(bit_vector); }
    else if offset_bits == 40 { assert(addr / (1u64 << 40u64) == addr >> 40u64) by(bit_vector); }
    else if offset_bits == 41 { assert(addr / (1u64 << 41u64) == addr >> 41u64) by(bit_vector); }
    else if offset_bits == 42 { assert(addr / (1u64 << 42u64) == addr >> 42u64) by(bit_vector); }
    else if offset_bits == 43 { assert(addr / (1u64 << 43u64) == addr >> 43u64) by(bit_vector); }
    else if offset_bits == 44 { assert(addr / (1u64 << 44u64) == addr >> 44u64) by(bit_vector); }
    else if offset_bits == 45 { assert(addr / (1u64 << 45u64) == addr >> 45u64) by(bit_vector); }
    else if offset_bits == 46 { assert(addr / (1u64 << 46u64) == addr >> 46u64) by(bit_vector); }
    else if offset_bits == 47 { assert(addr / (1u64 << 47u64) == addr >> 47u64) by(bit_vector); }
    else if offset_bits == 48 { assert(addr / (1u64 << 48u64) == addr >> 48u64) by(bit_vector); }
    else if offset_bits == 49 { assert(addr / (1u64 << 49u64) == addr >> 49u64) by(bit_vector); }
    else if offset_bits == 50 { assert(addr / (1u64 << 50u64) == addr >> 50u64) by(bit_vector); }
    else if offset_bits == 51 { assert(addr / (1u64 << 51u64) == addr >> 51u64) by(bit_vector); }
    else if offset_bits == 52 { assert(addr / (1u64 << 52u64) == addr >> 52u64) by(bit_vector); }
    else if offset_bits == 53 { assert(addr / (1u64 << 53u64) == addr >> 53u64) by(bit_vector); }
    else if offset_bits == 54 { assert(addr / (1u64 << 54u64) == addr >> 54u64) by(bit_vector); }
    else if offset_bits == 55 { assert(addr / (1u64 << 55u64) == addr >> 55u64) by(bit_vector); }
    else if offset_bits == 56 { assert(addr / (1u64 << 56u64) == addr >> 56u64) by(bit_vector); }
    else if offset_bits == 57 { assert(addr / (1u64 << 57u64) == addr >> 57u64) by(bit_vector); }
    else if offset_bits == 58 { assert(addr / (1u64 << 58u64) == addr >> 58u64) by(bit_vector); }
    else if offset_bits == 59 { assert(addr / (1u64 << 59u64) == addr >> 59u64) by(bit_vector); }
    else if offset_bits == 60 { assert(addr / (1u64 << 60u64) == addr >> 60u64) by(bit_vector); }
    else if offset_bits == 61 { assert(addr / (1u64 << 61u64) == addr >> 61u64) by(bit_vector); }
    else if offset_bits == 62 { assert(addr / (1u64 << 62u64) == addr >> 62u64) by(bit_vector); }
    else {
        assert(offset_bits == 63) by(nonlinear_arith)
            requires offset_bits < 64, offset_bits != 0, offset_bits != 1, offset_bits != 2, offset_bits != 3,
                offset_bits != 4, offset_bits != 5, offset_bits != 6, offset_bits != 7, offset_bits != 8,
                offset_bits != 9, offset_bits != 10, offset_bits != 11, offset_bits != 12, offset_bits != 13,
                offset_bits != 14, offset_bits != 15, offset_bits != 16, offset_bits != 17, offset_bits != 18,
                offset_bits != 19, offset_bits != 20, offset_bits != 21, offset_bits != 22, offset_bits != 23,
                offset_bits != 24, offset_bits != 25, offset_bits != 26, offset_bits != 27, offset_bits != 28,
                offset_bits != 29, offset_bits != 30, offset_bits != 31, offset_bits != 32, offset_bits != 33,
                offset_bits != 34, offset_bits != 35, offset_bits != 36, offset_bits != 37, offset_bits != 38,
                offset_bits != 39, offset_bits != 40, offset_bits != 41, offset_bits != 42, offset_bits != 43,
                offset_bits != 44, offset_bits != 45, offset_bits != 46, offset_bits != 47, offset_bits != 48,
                offset_bits != 49, offset_bits != 50, offset_bits != 51, offset_bits != 52, offset_bits != 53,
                offset_bits != 54, offset_bits != 55, offset_bits != 56, offset_bits != 57, offset_bits != 58,
                offset_bits != 59, offset_bits != 60, offset_bits != 61, offset_bits != 62;
        assert(addr / (1u64 << 63u64) == addr >> 63u64) by(bit_vector);
    }
}

proof fn shifted_parts_equal(x: u64, y: u64, index_bits: u64)
    requires index_bits <= 64
    ensures
        (x == y)
            == (
                (if index_bits == 64 { 0u64 } else { x >> index_bits })
                    == (if index_bits == 64 { 0u64 } else { y >> index_bits })
                && (if index_bits == 0 { 0u64 } else { x & low_mask_spec(index_bits) })
                    == (if index_bits == 0 { 0u64 } else { y & low_mask_spec(index_bits) })
            )
{
    assert(
        (x == y)
            == (
                (if index_bits == 64 { 0u64 } else { x >> index_bits })
                    == (if index_bits == 64 { 0u64 } else { y >> index_bits })
                && (if index_bits == 0 { 0u64 } else { x & low_mask_spec(index_bits) })
                    == (if index_bits == 0 { 0u64 } else { y & low_mask_spec(index_bits) })
            )
    ) by(bit_vector)
        requires index_bits <= 64u64;
}

proof fn shift_assoc(addr: u64, offset_bits: u64, index_bits: u64)
    requires offset_bits < 64, offset_bits + index_bits < 64
    ensures (addr >> (((offset_bits as int) + (index_bits as int)) as u64)) == ((addr >> offset_bits) >> index_bits)
{
    if index_bits == 0u64 { assert((addr >> (offset_bits + 0u64)) == ((addr >> offset_bits) >> 0u64)) by(bit_vector) requires offset_bits + 0u64 < 64u64; }
    else if index_bits == 1u64 { assert((addr >> (offset_bits + 1u64)) == ((addr >> offset_bits) >> 1u64)) by(bit_vector) requires offset_bits + 1u64 < 64u64; }
    else if index_bits == 2u64 { assert((addr >> (offset_bits + 2u64)) == ((addr >> offset_bits) >> 2u64)) by(bit_vector) requires offset_bits + 2u64 < 64u64; }
    else if index_bits == 3u64 { assert((addr >> (offset_bits + 3u64)) == ((addr >> offset_bits) >> 3u64)) by(bit_vector) requires offset_bits + 3u64 < 64u64; }
    else if index_bits == 4u64 { assert((addr >> (offset_bits + 4u64)) == ((addr >> offset_bits) >> 4u64)) by(bit_vector) requires offset_bits + 4u64 < 64u64; }
    else if index_bits == 5u64 { assert((addr >> (offset_bits + 5u64)) == ((addr >> offset_bits) >> 5u64)) by(bit_vector) requires offset_bits + 5u64 < 64u64; }
    else if index_bits == 6u64 { assert((addr >> (offset_bits + 6u64)) == ((addr >> offset_bits) >> 6u64)) by(bit_vector) requires offset_bits + 6u64 < 64u64; }
    else if index_bits == 7u64 { assert((addr >> (offset_bits + 7u64)) == ((addr >> offset_bits) >> 7u64)) by(bit_vector) requires offset_bits + 7u64 < 64u64; }
    else if index_bits == 8u64 { assert((addr >> (offset_bits + 8u64)) == ((addr >> offset_bits) >> 8u64)) by(bit_vector) requires offset_bits + 8u64 < 64u64; }
    else if index_bits == 9u64 { assert((addr >> (offset_bits + 9u64)) == ((addr >> offset_bits) >> 9u64)) by(bit_vector) requires offset_bits + 9u64 < 64u64; }
    else if index_bits == 10u64 { assert((addr >> (offset_bits + 10u64)) == ((addr >> offset_bits) >> 10u64)) by(bit_vector) requires offset_bits + 10u64 < 64u64; }
    else if index_bits == 11u64 { assert((addr >> (offset_bits + 11u64)) == ((addr >> offset_bits) >> 11u64)) by(bit_vector) requires offset_bits + 11u64 < 64u64; }
    else if index_bits == 12u64 { assert((addr >> (offset_bits + 12u64)) == ((addr >> offset_bits) >> 12u64)) by(bit_vector) requires offset_bits + 12u64 < 64u64; }
    else if index_bits == 13u64 { assert((addr >> (offset_bits + 13u64)) == ((addr >> offset_bits) >> 13u64)) by(bit_vector) requires offset_bits + 13u64 < 64u64; }
    else if index_bits == 14u64 { assert((addr >> (offset_bits + 14u64)) == ((addr >> offset_bits) >> 14u64)) by(bit_vector) requires offset_bits + 14u64 < 64u64; }
    else if index_bits == 15u64 { assert((addr >> (offset_bits + 15u64)) == ((addr >> offset_bits) >> 15u64)) by(bit_vector) requires offset_bits + 15u64 < 64u64; }
    else if index_bits == 16u64 { assert((addr >> (offset_bits + 16u64)) == ((addr >> offset_bits) >> 16u64)) by(bit_vector) requires offset_bits + 16u64 < 64u64; }
    else if index_bits == 17u64 { assert((addr >> (offset_bits + 17u64)) == ((addr >> offset_bits) >> 17u64)) by(bit_vector) requires offset_bits + 17u64 < 64u64; }
    else if index_bits == 18u64 { assert((addr >> (offset_bits + 18u64)) == ((addr >> offset_bits) >> 18u64)) by(bit_vector) requires offset_bits + 18u64 < 64u64; }
    else if index_bits == 19u64 { assert((addr >> (offset_bits + 19u64)) == ((addr >> offset_bits) >> 19u64)) by(bit_vector) requires offset_bits + 19u64 < 64u64; }
    else if index_bits == 20u64 { assert((addr >> (offset_bits + 20u64)) == ((addr >> offset_bits) >> 20u64)) by(bit_vector) requires offset_bits + 20u64 < 64u64; }
    else if index_bits == 21u64 { assert((addr >> (offset_bits + 21u64)) == ((addr >> offset_bits) >> 21u64)) by(bit_vector) requires offset_bits + 21u64 < 64u64; }
    else if index_bits == 22u64 { assert((addr >> (offset_bits + 22u64)) == ((addr >> offset_bits) >> 22u64)) by(bit_vector) requires offset_bits + 22u64 < 64u64; }
    else if index_bits == 23u64 { assert((addr >> (offset_bits + 23u64)) == ((addr >> offset_bits) >> 23u64)) by(bit_vector) requires offset_bits + 23u64 < 64u64; }
    else if index_bits == 24u64 { assert((addr >> (offset_bits + 24u64)) == ((addr >> offset_bits) >> 24u64)) by(bit_vector) requires offset_bits + 24u64 < 64u64; }
    else if index_bits == 25u64 { assert((addr >> (offset_bits + 25u64)) == ((addr >> offset_bits) >> 25u64)) by(bit_vector) requires offset_bits + 25u64 < 64u64; }
    else if index_bits == 26u64 { assert((addr >> (offset_bits + 26u64)) == ((addr >> offset_bits) >> 26u64)) by(bit_vector) requires offset_bits + 26u64 < 64u64; }
    else if index_bits == 27u64 { assert((addr >> (offset_bits + 27u64)) == ((addr >> offset_bits) >> 27u64)) by(bit_vector) requires offset_bits + 27u64 < 64u64; }
    else if index_bits == 28u64 { assert((addr >> (offset_bits + 28u64)) == ((addr >> offset_bits) >> 28u64)) by(bit_vector) requires offset_bits + 28u64 < 64u64; }
    else if index_bits == 29u64 { assert((addr >> (offset_bits + 29u64)) == ((addr >> offset_bits) >> 29u64)) by(bit_vector) requires offset_bits + 29u64 < 64u64; }
    else if index_bits == 30u64 { assert((addr >> (offset_bits + 30u64)) == ((addr >> offset_bits) >> 30u64)) by(bit_vector) requires offset_bits + 30u64 < 64u64; }
    else if index_bits == 31u64 { assert((addr >> (offset_bits + 31u64)) == ((addr >> offset_bits) >> 31u64)) by(bit_vector) requires offset_bits + 31u64 < 64u64; }
    else if index_bits == 32u64 { assert((addr >> (offset_bits + 32u64)) == ((addr >> offset_bits) >> 32u64)) by(bit_vector) requires offset_bits + 32u64 < 64u64; }
    else if index_bits == 33u64 { assert((addr >> (offset_bits + 33u64)) == ((addr >> offset_bits) >> 33u64)) by(bit_vector) requires offset_bits + 33u64 < 64u64; }
    else if index_bits == 34u64 { assert((addr >> (offset_bits + 34u64)) == ((addr >> offset_bits) >> 34u64)) by(bit_vector) requires offset_bits + 34u64 < 64u64; }
    else if index_bits == 35u64 { assert((addr >> (offset_bits + 35u64)) == ((addr >> offset_bits) >> 35u64)) by(bit_vector) requires offset_bits + 35u64 < 64u64; }
    else if index_bits == 36u64 { assert((addr >> (offset_bits + 36u64)) == ((addr >> offset_bits) >> 36u64)) by(bit_vector) requires offset_bits + 36u64 < 64u64; }
    else if index_bits == 37u64 { assert((addr >> (offset_bits + 37u64)) == ((addr >> offset_bits) >> 37u64)) by(bit_vector) requires offset_bits + 37u64 < 64u64; }
    else if index_bits == 38u64 { assert((addr >> (offset_bits + 38u64)) == ((addr >> offset_bits) >> 38u64)) by(bit_vector) requires offset_bits + 38u64 < 64u64; }
    else if index_bits == 39u64 { assert((addr >> (offset_bits + 39u64)) == ((addr >> offset_bits) >> 39u64)) by(bit_vector) requires offset_bits + 39u64 < 64u64; }
    else if index_bits == 40u64 { assert((addr >> (offset_bits + 40u64)) == ((addr >> offset_bits) >> 40u64)) by(bit_vector) requires offset_bits + 40u64 < 64u64; }
    else if index_bits == 41u64 { assert((addr >> (offset_bits + 41u64)) == ((addr >> offset_bits) >> 41u64)) by(bit_vector) requires offset_bits + 41u64 < 64u64; }
    else if index_bits == 42u64 { assert((addr >> (offset_bits + 42u64)) == ((addr >> offset_bits) >> 42u64)) by(bit_vector) requires offset_bits + 42u64 < 64u64; }
    else if index_bits == 43u64 { assert((addr >> (offset_bits + 43u64)) == ((addr >> offset_bits) >> 43u64)) by(bit_vector) requires offset_bits + 43u64 < 64u64; }
    else if index_bits == 44u64 { assert((addr >> (offset_bits + 44u64)) == ((addr >> offset_bits) >> 44u64)) by(bit_vector) requires offset_bits + 44u64 < 64u64; }
    else if index_bits == 45u64 { assert((addr >> (offset_bits + 45u64)) == ((addr >> offset_bits) >> 45u64)) by(bit_vector) requires offset_bits + 45u64 < 64u64; }
    else if index_bits == 46u64 { assert((addr >> (offset_bits + 46u64)) == ((addr >> offset_bits) >> 46u64)) by(bit_vector) requires offset_bits + 46u64 < 64u64; }
    else if index_bits == 47u64 { assert((addr >> (offset_bits + 47u64)) == ((addr >> offset_bits) >> 47u64)) by(bit_vector) requires offset_bits + 47u64 < 64u64; }
    else if index_bits == 48u64 { assert((addr >> (offset_bits + 48u64)) == ((addr >> offset_bits) >> 48u64)) by(bit_vector) requires offset_bits + 48u64 < 64u64; }
    else if index_bits == 49u64 { assert((addr >> (offset_bits + 49u64)) == ((addr >> offset_bits) >> 49u64)) by(bit_vector) requires offset_bits + 49u64 < 64u64; }
    else if index_bits == 50u64 { assert((addr >> (offset_bits + 50u64)) == ((addr >> offset_bits) >> 50u64)) by(bit_vector) requires offset_bits + 50u64 < 64u64; }
    else if index_bits == 51u64 { assert((addr >> (offset_bits + 51u64)) == ((addr >> offset_bits) >> 51u64)) by(bit_vector) requires offset_bits + 51u64 < 64u64; }
    else if index_bits == 52u64 { assert((addr >> (offset_bits + 52u64)) == ((addr >> offset_bits) >> 52u64)) by(bit_vector) requires offset_bits + 52u64 < 64u64; }
    else if index_bits == 53u64 { assert((addr >> (offset_bits + 53u64)) == ((addr >> offset_bits) >> 53u64)) by(bit_vector) requires offset_bits + 53u64 < 64u64; }
    else if index_bits == 54u64 { assert((addr >> (offset_bits + 54u64)) == ((addr >> offset_bits) >> 54u64)) by(bit_vector) requires offset_bits + 54u64 < 64u64; }
    else if index_bits == 55u64 { assert((addr >> (offset_bits + 55u64)) == ((addr >> offset_bits) >> 55u64)) by(bit_vector) requires offset_bits + 55u64 < 64u64; }
    else if index_bits == 56u64 { assert((addr >> (offset_bits + 56u64)) == ((addr >> offset_bits) >> 56u64)) by(bit_vector) requires offset_bits + 56u64 < 64u64; }
    else if index_bits == 57u64 { assert((addr >> (offset_bits + 57u64)) == ((addr >> offset_bits) >> 57u64)) by(bit_vector) requires offset_bits + 57u64 < 64u64; }
    else if index_bits == 58u64 { assert((addr >> (offset_bits + 58u64)) == ((addr >> offset_bits) >> 58u64)) by(bit_vector) requires offset_bits + 58u64 < 64u64; }
    else if index_bits == 59u64 { assert((addr >> (offset_bits + 59u64)) == ((addr >> offset_bits) >> 59u64)) by(bit_vector) requires offset_bits + 59u64 < 64u64; }
    else if index_bits == 60u64 { assert((addr >> (offset_bits + 60u64)) == ((addr >> offset_bits) >> 60u64)) by(bit_vector) requires offset_bits + 60u64 < 64u64; }
    else if index_bits == 61u64 { assert((addr >> (offset_bits + 61u64)) == ((addr >> offset_bits) >> 61u64)) by(bit_vector) requires offset_bits + 61u64 < 64u64; }
    else if index_bits == 62u64 { assert((addr >> (offset_bits + 62u64)) == ((addr >> offset_bits) >> 62u64)) by(bit_vector) requires offset_bits + 62u64 < 64u64; }
    else if index_bits == 63u64 { assert((addr >> (offset_bits + 63u64)) == ((addr >> offset_bits) >> 63u64)) by(bit_vector) requires offset_bits + 63u64 < 64u64; }
    else { assert(false); }
}

proof fn tag_from_shift(addr: u64, offset_bits: u64, index_bits: u64)
    requires offset_bits < 64, offset_bits + index_bits <= 64
    ensures extract_tag_spec(addr, offset_bits, index_bits)
        == (if offset_bits + index_bits == 64 { 0u64 } else { (addr >> offset_bits) >> index_bits })
{
    if offset_bits + index_bits == 64 {
    } else {
        assert(offset_bits + index_bits < 64);
        assert(extract_tag_spec(addr, offset_bits, index_bits) == addr >> (offset_bits + index_bits)) by(bit_vector)
            requires offset_bits < 64u64, offset_bits + index_bits < 64u64;
        let total: u64 = ((offset_bits as int) + (index_bits as int)) as u64;
        shift_assoc(addr, offset_bits, index_bits);
        assert((addr >> total) == ((addr >> offset_bits) >> index_bits));
    }
}

proof fn tag_component_matches_parts(addr: u64, offset_bits: u64, index_bits: u64)
    requires offset_bits < 64, offset_bits + index_bits <= 64
    ensures
        (if offset_bits + index_bits == 64 { 0u64 } else { (addr >> offset_bits) >> index_bits })
            == (if index_bits == 64 { 0u64 } else { (addr >> offset_bits) >> index_bits })
{
    if index_bits == 64 {
    } else if offset_bits + index_bits < 64 {
    } else {
        assert(offset_bits + index_bits == 64);
        assert(offset_bits > 0u64) by(nonlinear_arith)
            requires offset_bits < 64, index_bits < 64, offset_bits + index_bits == 64;
        assert(((addr >> offset_bits) >> index_bits) == 0u64) by(bit_vector)
            requires offset_bits < 64u64, index_bits < 64u64, offset_bits + index_bits == 64u64, offset_bits > 0u64;
    }
}

proof fn shift_eq_iff_tag_index_eq(a: u64, b: u64, offset_bits: u64, index_bits: u64)
    requires offset_bits < 64, offset_bits + index_bits <= 64
    ensures
        ((a >> offset_bits) == (b >> offset_bits))
            == (
                extract_tag_spec(a, offset_bits, index_bits)
                    == extract_tag_spec(b, offset_bits, index_bits)
                && extract_index_spec(a, offset_bits, index_bits)
                    == extract_index_spec(b, offset_bits, index_bits)
            )
{
    let x = a >> offset_bits;
    let y = b >> offset_bits;
    shifted_parts_equal(x, y, index_bits);
    assert(extract_index_spec(a, offset_bits, index_bits)
        == (if index_bits == 0 { 0u64 } else { x & low_mask_spec(index_bits) }));
    assert(extract_index_spec(b, offset_bits, index_bits)
        == (if index_bits == 0 { 0u64 } else { y & low_mask_spec(index_bits) }));
    tag_from_shift(a, offset_bits, index_bits);
    tag_from_shift(b, offset_bits, index_bits);
    tag_component_matches_parts(a, offset_bits, index_bits);
    tag_component_matches_parts(b, offset_bits, index_bits);
    let tag_eq = extract_tag_spec(a, offset_bits, index_bits) == extract_tag_spec(b, offset_bits, index_bits);
    let index_eq = extract_index_spec(a, offset_bits, index_bits) == extract_index_spec(b, offset_bits, index_bits);
    let tag_rhs = (if index_bits == 64 { 0u64 } else { x >> index_bits }) == (if index_bits == 64 { 0u64 } else { y >> index_bits });
    let index_rhs = (if index_bits == 0 { 0u64 } else { x & low_mask_spec(index_bits) }) == (if index_bits == 0 { 0u64 } else { y & low_mask_spec(index_bits) });
    assert(tag_eq == tag_rhs);
    assert(index_eq == index_rhs);
    let parts_eq =
        (if index_bits == 64 { 0u64 } else { x >> index_bits })
            == (if index_bits == 64 { 0u64 } else { y >> index_bits })
        && (if index_bits == 0 { 0u64 } else { x & low_mask_spec(index_bits) })
            == (if index_bits == 0 { 0u64 } else { y & low_mask_spec(index_bits) });
    assert((x == y) == parts_eq);
    if (a >> offset_bits) == (b >> offset_bits) {
        assert(x == y);
        assert(parts_eq);
        assert(tag_rhs && index_rhs);
        assert(tag_eq && index_eq);
    } else {
        assert(x != y);
        assert(!(
            (if index_bits == 64 { 0u64 } else { x >> index_bits })
                == (if index_bits == 64 { 0u64 } else { y >> index_bits })
            && (if index_bits == 0 { 0u64 } else { x & low_mask_spec(index_bits) })
                == (if index_bits == 0 { 0u64 } else { y & low_mask_spec(index_bits) })
        ));
        if tag_eq && index_eq {
            assert(extract_tag_spec(a, offset_bits, index_bits)
                == extract_tag_spec(b, offset_bits, index_bits));
            assert(extract_index_spec(a, offset_bits, index_bits)
                == extract_index_spec(b, offset_bits, index_bits));
            assert((if index_bits == 64 { 0u64 } else { x >> index_bits })
                == (if index_bits == 64 { 0u64 } else { y >> index_bits }));
            assert((if index_bits == 0 { 0u64 } else { x & low_mask_spec(index_bits) })
                == (if index_bits == 0 { 0u64 } else { y & low_mask_spec(index_bits) }));
            assert(
                ((if index_bits == 64 { 0u64 } else { x >> index_bits })
                    == (if index_bits == 64 { 0u64 } else { y >> index_bits }))
                ==> (
                    ((if index_bits == 0 { 0u64 } else { x & low_mask_spec(index_bits) })
                        == (if index_bits == 0 { 0u64 } else { y & low_mask_spec(index_bits) }))
                    ==> (
                        ((if index_bits == 64 { 0u64 } else { x >> index_bits })
                            == (if index_bits == 64 { 0u64 } else { y >> index_bits }))
                        && ((if index_bits == 0 { 0u64 } else { x & low_mask_spec(index_bits) })
                            == (if index_bits == 0 { 0u64 } else { y & low_mask_spec(index_bits) }))
                    )
                )
            );
            assert(
                (if index_bits == 64 { 0u64 } else { x >> index_bits })
                    == (if index_bits == 64 { 0u64 } else { y >> index_bits })
                && (if index_bits == 0 { 0u64 } else { x & low_mask_spec(index_bits) })
                    == (if index_bits == 0 { 0u64 } else { y & low_mask_spec(index_bits) })
            );
            assert(false);
        }
    }
}

// r[impl false-sharing.same-line]
/// Runtime same-line predicate.
pub fn same_cache_line(a: u64, b: u64, line_size: u64) -> (result: bool)
    requires line_size > 0
    ensures result == same_cache_line_spec(a, b, line_size)
{
    a / line_size == b / line_size
}

// r[verify false-sharing.same-line]
/// Same-line is reflexive.
pub proof fn same_cache_line_reflexive(addr: u64, line_size: u64)
    requires line_size > 0
    ensures same_cache_line_spec(addr, addr, line_size)
{
}

// r[verify false-sharing.same-line]
/// Same-line is symmetric.
pub proof fn same_cache_line_symmetric(a: u64, b: u64, line_size: u64)
    requires line_size > 0
    ensures same_cache_line_spec(a, b, line_size) == same_cache_line_spec(b, a, line_size)
{
}

// r[impl false-sharing.alignment]
/// Runtime alignment predicate.
pub fn is_line_aligned(addr: u64, line_size: u64) -> (result: bool)
    requires line_size > 0
    ensures result == is_line_aligned_spec(addr, line_size)
{
    addr % line_size == 0
}

// r[impl false-sharing.alignment]
/// Runtime line-start helper.
pub fn line_start(addr: u64, line_size: u64) -> (result: u64)
    requires line_size > 0
    ensures result as int == line_start_spec(addr, line_size)
{
    let q = addr / line_size;
    proof {
        vstd::arithmetic::div_mod::lemma_fundamental_div_mod(addr as int, line_size as int);
        assert((q as int) * (line_size as int) <= addr as int) by(nonlinear_arith)
            requires
                line_size > 0,
                q as int == addr as int / line_size as int,
                addr as int == (addr as int / line_size as int) * line_size as int + addr as int % line_size as int,
                0 <= addr as int % line_size as int;
        assert(q * line_size <= addr);
    }
    q * line_size
}

// r[verify false-sharing.alignment]
/// Same-line is exactly equality of line starts.
pub proof fn same_cache_line_iff_line_start(a: u64, b: u64, line_size: u64)
    requires line_size > 0
    ensures same_cache_line_spec(a, b, line_size) <==> line_start_spec(a, line_size) == line_start_spec(b, line_size)
{
    line_start_is_floor(a, line_size);
    line_start_is_floor(b, line_size);
    if same_cache_line_spec(a, b, line_size) {
        assert(a / line_size == b / line_size);
        assert(line_start_spec(a, line_size) == line_start_spec(b, line_size));
    } else {
        assert(a / line_size != b / line_size);
        assert(line_start_spec(a, line_size) != line_start_spec(b, line_size)) by(nonlinear_arith)
            requires
                line_size > 0,
                a / line_size != b / line_size,
                line_start_spec(a, line_size) == (a as int / line_size as int) * line_size as int,
                line_start_spec(b, line_size) == (b as int / line_size as int) * line_size as int;
    }
}

// r[verify false-sharing.alignment]
/// Alignment means the address is already its own line start.
pub proof fn aligned_addresses_are_boundaries(addr: u64, line_size: u64)
    requires line_size > 0
    ensures is_line_aligned_spec(addr, line_size) <==> line_start_spec(addr, line_size) == addr as int
{
    line_start_is_floor(addr, line_size);
    vstd::arithmetic::div_mod::lemma_fundamental_div_mod(addr as int, line_size as int);
    assert(line_start_spec(addr, line_size) == addr as int <==> addr % line_size == 0u64) by(nonlinear_arith)
        requires line_size > 0;
}

// r[impl false-sharing.padding]
/// Runtime ordered-gap predicate for object A then object B.
pub fn padding_prevents_sharing(
    addr_a: u64,
    size_a: u64,
    addr_b: u64,
    line_size: u64,
) -> (result: bool)
    requires line_size > 0
    ensures result == padding_prevents_sharing_spec(addr_a, size_a, addr_b, line_size)
{
    if addr_b < addr_a {
        false
    } else {
        let gap = addr_b - addr_a;
        if gap < size_a {
            false
        } else {
            gap - size_a >= line_size
        }
    }
}

// r[verify false-sharing.padding]
/// If B starts one full line after the end of A, then every byte of A is on a different line from every byte of B.
pub proof fn padding_implies_no_shared_byte(
    addr_a: u64,
    size_a: u64,
    addr_b: u64,
    line_size: u64,
    offset_a: u64,
    b_addr: u64,
)
    requires
        line_size > 0,
        padding_prevents_sharing_spec(addr_a, size_a, addr_b, line_size),
        offset_a < size_a,
        b_addr >= addr_b,
        (addr_a as int) + (offset_a as int) <= u64::MAX as int,
    ensures !same_cache_line_spec(((addr_a as int) + (offset_a as int)) as u64, b_addr, line_size)
{
    let a_byte: u64 = ((addr_a as int) + (offset_a as int)) as u64;
    assert(a_byte as int == addr_a as int + offset_a as int);
    assert((a_byte as int) < (addr_a as int) + (size_a as int)) by(nonlinear_arith)
        requires a_byte as int == addr_a as int + offset_a as int, offset_a < size_a;
    assert((b_addr as int) >= (addr_a as int) + (size_a as int) + (line_size as int)) by(nonlinear_arith)
        requires
            padding_prevents_sharing_spec(addr_a, size_a, addr_b, line_size),
            b_addr >= addr_b;
    assert((b_addr as int) >= (a_byte as int) + (line_size as int)) by(nonlinear_arith)
        requires
            padding_prevents_sharing_spec(addr_a, size_a, addr_b, line_size),
            a_byte as int == addr_a as int + offset_a as int,
            offset_a < size_a,
            b_addr >= addr_b;
    assert(a_byte <= b_addr) by(nonlinear_arith)
        requires line_size > 0, (b_addr as int) >= (a_byte as int) + (line_size as int);
    if same_cache_line_spec(a_byte, b_addr, line_size) {
        same_line_implies_gap_lt_line(a_byte, b_addr, line_size);
        assert(false);
    }
}

// r[impl false-sharing.object-span]
/// Runtime object span in cache lines.
pub fn cache_lines_spanned(addr: u64, size: u64, line_size: u64) -> (result: u64)
    requires
        line_size > 0,
        size > 0,
        (addr as int % line_size as int) + size as int <= u64::MAX as int,
    ensures result == cache_lines_spanned_spec(addr, size, line_size)
{
    let offset = addr % line_size;
    div_ceil(offset + size, line_size)
}

// r[verify false-sharing.object-span]
/// A line-aligned object that fits in one line spans exactly one line.
pub proof fn aligned_single_line_spans_one(addr: u64, size: u64, line_size: u64)
    requires
        line_size > 0,
        size > 0,
        size <= line_size,
        is_line_aligned_spec(addr, line_size),
    ensures cache_lines_spanned_spec(addr, size, line_size) == 1u64
{
    assert(addr % line_size == 0u64);
    assert(cache_lines_spanned_spec(addr, size, line_size) == div_ceil_spec(size, line_size));
    assert(div_ceil_spec(size, line_size) == 1u64) by(nonlinear_arith)
        requires line_size > 0, size > 0, size <= line_size;
}

// r[verify false-sharing.decomposition-equivalence]
/// Same-line is equivalent to equality of canonical tag and index fields.
pub proof fn prove_decomposition_equivalence(a: u64, b: u64, offset_bits: u64, index_bits: u64)
    requires offset_bits < 64, offset_bits + index_bits <= 64
    ensures decomposition_equivalent_spec(a, b, offset_bits, index_bits)
{
    div_shift_equiv(a, offset_bits);
    div_shift_equiv(b, offset_bits);
    assert(same_cache_line_spec(a, b, 1u64 << offset_bits) == ((a >> offset_bits) == (b >> offset_bits)));
    shift_eq_iff_tag_index_eq(a, b, offset_bits, index_bits);
}

// r[verify false-sharing.distinct-line]
/// Different lines imply a different canonical (tag,index) identity.
pub proof fn prove_distinct_line_guarantee(a: u64, b: u64, offset_bits: u64, index_bits: u64)
    requires offset_bits < 64, offset_bits + index_bits <= 64
    ensures distinct_line_guarantee_spec(a, b, offset_bits, index_bits)
{
    prove_decomposition_equivalence(a, b, offset_bits, index_bits);
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::set_associative_lookup::extract_index;
    use crate::set_associative_lookup::extract_tag;

    // r[verify false-sharing.same-line]
    #[test]
    fn test_same_cache_line_known_pairs() {
        assert!(same_cache_line(0x1000, 0x103f, 64));
        assert!(!same_cache_line(0x103f, 0x1040, 64));
        assert!(same_cache_line(63, 63, 64));
        assert!(!same_cache_line(63, 64, 64));
    }

    // r[verify false-sharing.alignment]
    #[test]
    fn test_alignment_and_line_start() {
        assert!(is_line_aligned(0x1000, 64));
        assert!(!is_line_aligned(0x1004, 64));
        assert_eq!(line_start(0x1000, 64), 0x1000);
        assert_eq!(line_start(0x103f, 64), 0x1000);
        assert_eq!(line_start(0x1040, 64), 0x1040);
    }

    // r[verify false-sharing.object-span]
    #[test]
    fn test_object_span() {
        assert_eq!(cache_lines_spanned(0x1000, 32, 64), 1);
        assert_eq!(cache_lines_spanned(0x1008, 60, 64), 2);
        assert_eq!(cache_lines_spanned(63, 2, 64), 2);
    }

    // r[verify false-sharing.padding]
    #[test]
    fn test_padding_examples() {
        assert!(padding_prevents_sharing(100, 40, 204, 64));
        assert!(!padding_prevents_sharing(100, 40, 150, 64));
    }

    proptest! {
        // r[verify false-sharing.same-line]
        #[test]
        fn prop_same_cache_line_matches_division(a: u64, b: u64, offset_bits in 0u32..64u32) {
            let line_size = 1u64 << offset_bits;
            prop_assert_eq!(same_cache_line(a, b, line_size), a / line_size == b / line_size);
        }

        // r[verify false-sharing.padding]
        #[test]
        fn prop_padding_gap_prevents_sharing(
            addr_a in 0u64..10000,
            size_a in 1u64..64,
            size_b in 1u64..64,
            offset_bits in 0u32..7u32,
            extra_gap in 0u64..32,
        ) {
            let line_size = 1u64 << offset_bits;
            let Some(after_a) = addr_a.checked_add(size_a) else { return Ok(()); };
            let Some(addr_b) = after_a.checked_add(line_size + extra_gap) else { return Ok(()); };
            prop_assert!(padding_prevents_sharing(addr_a, size_a, addr_b, line_size));
            for i in 0..size_a {
                for j in 0..size_b {
                    let a_byte = addr_a + i;
                    let b_byte = addr_b + j;
                    prop_assert!(!same_cache_line(a_byte, b_byte, line_size));
                }
            }
        }

        // r[verify false-sharing.decomposition-equivalence]
        #[test]
        fn prop_decomposition_equivalence(
            a: u64,
            b: u64,
            offset_bits in 0u32..64u32,
            index_bits in 0u32..64u32,
        ) {
            prop_assume!((offset_bits as u64) + (index_bits as u64) <= 64);
            let line_size = 1u64 << offset_bits;
            let tag_eq = extract_tag(a, offset_bits as u64, index_bits as u64)
                == extract_tag(b, offset_bits as u64, index_bits as u64);
            let index_eq = extract_index(a, offset_bits as u64, index_bits as u64)
                == extract_index(b, offset_bits as u64, index_bits as u64);
            prop_assert_eq!(same_cache_line(a, b, line_size), tag_eq && index_eq);
        }
    }
}
