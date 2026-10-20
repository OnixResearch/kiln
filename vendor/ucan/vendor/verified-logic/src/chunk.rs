//! Verified ceiling-division chunk arithmetic.
//!
//! Functions for dividing data into fixed-size chunks with ceiling
//! semantics — the last chunk may be partial. Covers chunk counting,
//! size calculation, offset mapping, and decomposition proofs. All
//! functions require positive chunk size and avoid integer overflow.

// Verus spec needs separate branches for Z3 case analysis even when exec values match.
#![allow(clippy::if_same_then_else)]

use vstd::prelude::*;

verus! {

// --- chunk_count_ceil ---

// r[depends chunk.count-ceil]
/// Spec: number of chunks including partial last chunk.
/// Uses ceiling division to avoid overflow.
pub open spec fn chunk_count_ceil_spec(total_size: u64, chunk_size: u64) -> u64 {
    if total_size == 0 {
        0
    } else {
        ((total_size - 1) as int / chunk_size as int + 1) as u64
    }
}

// r[impl chunk.count-ceil]
/// Returns the number of chunks needed to store `total_size` bytes.
/// Includes a partial last chunk if needed.
pub fn chunk_count_ceil(total_size: u64, chunk_size: u64) -> (result: u64)
    requires chunk_size > 0
    ensures result == chunk_count_ceil_spec(total_size, chunk_size)
{
    if total_size == 0 {
        0
    } else {
        (total_size - 1) / chunk_size + 1
    }
}

// r[verify chunk.count-ceil]
/// Zero total size gives zero chunks.
proof fn chunk_count_zero()
    ensures chunk_count_ceil_spec(0, 10) == 0
{}

// r[verify chunk.count-ceil]
// r[verify chunk.ceiling]
/// Ceiling division is at least floor division.
proof fn chunk_count_ge_floor(total_size: u64, chunk_size: u64)
    requires chunk_size > 0
    ensures chunk_count_ceil_spec(total_size, chunk_size) as int >= total_size as int / chunk_size as int
{
    assert(((total_size - 1) as int / chunk_size as int + 1) >= total_size as int / chunk_size as int) by(nonlinear_arith)
        requires chunk_size > 0;
}

// --- chunk_size_at ---

// r[depends chunk.size-at]
/// Spec: size of chunk at given index.
/// Last chunk may be smaller if `total_size` is not evenly divisible.
pub open spec fn chunk_size_at_spec(total_size: u64, chunk_size: u64, index: u64) -> u64 {
    if (index + 1) < chunk_count_ceil_spec(total_size, chunk_size) {
        chunk_size
    } else if index < chunk_count_ceil_spec(total_size, chunk_size) {
        if total_size % chunk_size == 0 {
            chunk_size
        } else {
            (total_size as int % chunk_size as int) as u64
        }
    } else {
        0
    }
}

// r[impl chunk.size-at]
/// Returns the size of the chunk at the given index.
/// All chunks except possibly the last have size `chunk_size`.
pub fn chunk_size_at(total_size: u64, chunk_size: u64, index: u64) -> (result: u64)
    requires
        chunk_size > 0,
        index < chunk_count_ceil_spec(total_size, chunk_size),
    ensures
        result == chunk_size_at_spec(total_size, chunk_size, index),
        result > 0,
        result <= chunk_size,
{
    if index + 1 < chunk_count_ceil(total_size, chunk_size) {
        chunk_size
    } else if total_size % chunk_size == 0 {
        chunk_size
    } else {
        total_size % chunk_size
    }
}

// r[verify chunk.size-at]
/// Non-last chunks have full size.
proof fn non_last_chunk_full_size(total_size: u64, chunk_size: u64, index: u64)
    requires
        chunk_size > 0,
        (index + 1) < chunk_count_ceil_spec(total_size, chunk_size),
    ensures chunk_size_at_spec(total_size, chunk_size, index) == chunk_size
{}

// r[verify chunk.size-at]
/// Chunk size is always positive and bounded.
proof fn chunk_size_bounds(total_size: u64, chunk_size: u64, index: u64)
    requires
        chunk_size > 0,
        index < chunk_count_ceil_spec(total_size, chunk_size),
        total_size > 0,
    ensures
        chunk_size_at_spec(total_size, chunk_size, index) > 0,
        chunk_size_at_spec(total_size, chunk_size, index) <= chunk_size,
{}

// --- chunk_offset ---

// r[depends chunk.offset]
/// Spec: byte offset where chunk starts.
pub open spec fn chunk_offset_spec(chunk_size: u64, index: u64) -> u64 {
    (index as int * chunk_size as int) as u64
}

// r[impl chunk.offset]
/// Returns the byte offset where the given chunk starts.
pub fn chunk_offset(chunk_size: u64, index: u64) -> (result: u64)
    requires
        chunk_size > 0,
        index as int * chunk_size as int <= u64::MAX as int,
    ensures result == chunk_offset_spec(chunk_size, index)
{
    index * chunk_size
}

// r[verify chunk.offset]
/// Chunk 0 starts at offset 0.
proof fn chunk_zero_offset_zero(chunk_size: u64)
    requires chunk_size > 0
    ensures chunk_offset_spec(chunk_size, 0) == 0
{}

// --- chunk_index_for_offset ---

// r[depends chunk.index-for-offset]
/// Spec: which chunk contains this byte offset.
pub open spec fn chunk_index_for_offset_spec(chunk_size: u64, offset: u64) -> u64 {
    (offset as int / chunk_size as int) as u64
}

// r[impl chunk.index-for-offset]
/// Returns the index of the chunk that contains the given offset.
pub fn chunk_index_for_offset(chunk_size: u64, offset: u64) -> (result: u64)
    requires chunk_size > 0
    ensures result == chunk_index_for_offset_spec(chunk_size, offset)
{
    offset / chunk_size
}

// r[verify chunk.index-for-offset]
/// Offset 0 is in chunk 0.
proof fn offset_zero_in_chunk_zero(chunk_size: u64)
    requires chunk_size > 0
    ensures chunk_index_for_offset_spec(chunk_size, 0) == 0
{}

// r[verify chunk.bounds]
/// Valid offset gives in-bounds chunk index.
proof fn valid_offset_in_bounds(total_size: u64, chunk_size: u64, offset: u64)
    requires
        chunk_size > 0,
        offset < total_size,
        total_size > 0,
    ensures
        chunk_index_for_offset_spec(chunk_size, offset) < chunk_count_ceil_spec(total_size, chunk_size)
{
    assert((offset as int / chunk_size as int) < ((total_size - 1) as int / chunk_size as int + 1)) by(nonlinear_arith)
        requires
            chunk_size > 0,
            offset < total_size,
            total_size > 0;
}

// --- offset_within_chunk ---

// r[depends chunk.offset-within]
/// Spec: position within the chunk.
pub open spec fn offset_within_chunk_spec(chunk_size: u64, offset: u64) -> u64 {
    (offset as int % chunk_size as int) as u64
}

// r[impl chunk.offset-within]
/// Returns the position of the offset within its containing chunk.
pub fn offset_within_chunk(chunk_size: u64, offset: u64) -> (result: u64)
    requires chunk_size > 0
    ensures
        result == offset_within_chunk_spec(chunk_size, offset),
        result < chunk_size,
{
    offset % chunk_size
}

// r[verify chunk.offset-within]
/// Within-chunk offset is always less than chunk size.
proof fn offset_within_chunk_bounded(chunk_size: u64, offset: u64)
    requires chunk_size > 0
    ensures offset_within_chunk_spec(chunk_size, offset) < chunk_size
{}

// --- decomposition proofs ---

// r[verify chunk.decomposition]
/// Offset decomposition: `chunk_offset` + `offset_within` = original offset.
proof fn offset_decomposition(chunk_size: u64, offset: u64)
    requires chunk_size > 0
    ensures
        (chunk_offset_spec(chunk_size, chunk_index_for_offset_spec(chunk_size, offset)) as int)
            + (offset_within_chunk_spec(chunk_size, offset) as int)
            == (offset as int)
{
    assert((offset as int / chunk_size as int) * chunk_size as int + (offset as int % chunk_size as int) == offset as int) by(nonlinear_arith)
        requires chunk_size > 0;
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify chunk.count-ceil]
    #[test]
    fn test_chunk_count_ceil() {
        assert_eq!(chunk_count_ceil(0, 10), 0);
        assert_eq!(chunk_count_ceil(1, 10), 1);
        assert_eq!(chunk_count_ceil(10, 10), 1);
        assert_eq!(chunk_count_ceil(11, 10), 2);
        assert_eq!(chunk_count_ceil(20, 10), 2);
        assert_eq!(chunk_count_ceil(21, 10), 3);
    }

    // r[verify chunk.size-at]
    #[test]
    fn test_chunk_size_at() {
        assert_eq!(chunk_size_at(20, 10, 0), 10);
        assert_eq!(chunk_size_at(20, 10, 1), 10);
        assert_eq!(chunk_size_at(25, 10, 0), 10);
        assert_eq!(chunk_size_at(25, 10, 1), 10);
        assert_eq!(chunk_size_at(25, 10, 2), 5);
        assert_eq!(chunk_size_at(21, 10, 2), 1);
    }

    // r[verify chunk.offset]
    #[test]
    fn test_chunk_offset() {
        assert_eq!(chunk_offset(10, 0), 0);
        assert_eq!(chunk_offset(10, 1), 10);
        assert_eq!(chunk_offset(10, 2), 20);
    }

    // r[verify chunk.index-for-offset]
    #[test]
    fn test_chunk_index_for_offset() {
        assert_eq!(chunk_index_for_offset(10, 0), 0);
        assert_eq!(chunk_index_for_offset(10, 9), 0);
        assert_eq!(chunk_index_for_offset(10, 10), 1);
        assert_eq!(chunk_index_for_offset(10, 19), 1);
        assert_eq!(chunk_index_for_offset(10, 20), 2);
    }

    // r[verify chunk.offset-within]
    #[test]
    fn test_offset_within_chunk() {
        assert_eq!(offset_within_chunk(10, 0), 0);
        assert_eq!(offset_within_chunk(10, 9), 9);
        assert_eq!(offset_within_chunk(10, 10), 0);
        assert_eq!(offset_within_chunk(10, 25), 5);
    }

    // r[verify chunk.decomposition]
    #[test]
    fn test_offset_decomposition() {
        let chunk_size = 10u64;
        for offset in [0, 9, 10, 19, 20, 25, 100, 999] {
            let index = chunk_index_for_offset(chunk_size, offset);
            let within = offset_within_chunk(chunk_size, offset);
            let reconstructed = chunk_offset(chunk_size, index) + within;
            assert_eq!(reconstructed, offset);
        }
    }

    proptest! {
        // r[verify chunk.count-ceil]
        #[test]
        fn prop_chunk_count_ceil_vs_floor(total_size: u64, chunk_size in 1u64..=1000u64) {
            let ceil_count = chunk_count_ceil(total_size, chunk_size);
            let floor_count = total_size / chunk_size;
            prop_assert!(ceil_count >= floor_count);
            if total_size % chunk_size == 0 {
                prop_assert_eq!(ceil_count, floor_count);
            } else if total_size > 0 {
                prop_assert_eq!(ceil_count, floor_count + 1);
            }
        }

        // r[verify chunk.size-at]
        #[test]
        fn prop_chunk_size_at_bounds(
            total_size in 1u64..10000u64,
            chunk_size in 1u64..100u64,
            index: u64
        ) {
            let count = chunk_count_ceil(total_size, chunk_size);
            if index < count {
                let size = chunk_size_at(total_size, chunk_size, index);
                prop_assert!(size > 0);
                prop_assert!(size <= chunk_size);
            }
        }

        // r[verify chunk.decomposition]
        #[test]
        fn prop_offset_decomposition_roundtrip(offset: u64, chunk_size in 1u64..1000u64) {
            let index = chunk_index_for_offset(chunk_size, offset);
            let within = offset_within_chunk(chunk_size, offset);
            prop_assert!(within < chunk_size);
            if index <= u64::MAX / chunk_size {
                let reconstructed = chunk_offset(chunk_size, index) + within;
                prop_assert_eq!(reconstructed, offset);
            }
        }

        // r[verify chunk.bounds]
        #[test]
        fn prop_valid_offset_in_bounds(
            total_size in 1u64..10000u64,
            chunk_size in 1u64..100u64,
            offset: u64
        ) {
            if offset < total_size {
                let index = chunk_index_for_offset(chunk_size, offset);
                let count = chunk_count_ceil(total_size, chunk_size);
                prop_assert!(index < count);
            }
        }

        // r[verify chunk.count-ceil]
        #[test]
        fn prop_chunk_count_zero_total(chunk_size in 1u64..1000u64) {
            prop_assert_eq!(chunk_count_ceil(0, chunk_size), 0);
        }
    }
}
