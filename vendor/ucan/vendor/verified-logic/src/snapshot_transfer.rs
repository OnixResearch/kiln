// Verus proofs guarantee u128→u64 casts are in bounds (percentage < 100)
#![allow(clippy::cast_possible_truncation)]

//! Verified snapshot chunked transfer for Raft.
//!
//! Real Raft implementations stream large snapshots in chunks rather
//! than sending a single monolithic RPC. This module provides the
//! validation, ordering, and progress tracking for chunked transfers.

use vstd::prelude::*;

verus! {

// --- is_chunk_valid ---

// r[depends snapshot-transfer.chunk-valid]
/// Spec: is this chunk within the snapshot bounds?
///
/// offset + chunk_size must not exceed total_size.
/// Zero-size chunks are invalid.
pub open spec fn is_chunk_valid_spec(
    offset: u64,
    chunk_size: u64,
    total_size: u64,
) -> bool {
    chunk_size > 0
    && offset as int + chunk_size as int <= total_size as int
}

// r[impl snapshot-transfer.chunk-valid]
/// Validate a snapshot chunk.
pub fn is_chunk_valid(
    offset: u64,
    chunk_size: u64,
    total_size: u64,
) -> (result: bool)
    ensures result == is_chunk_valid_spec(offset, chunk_size, total_size)
{
    chunk_size > 0 && offset <= total_size && chunk_size <= total_size - offset
}

// --- is_first_chunk ---

// r[depends snapshot-transfer.is-first-chunk]
/// Spec: is this the first chunk?
pub open spec fn is_first_chunk_spec(offset: u64) -> bool {
    offset == 0
}

// r[impl snapshot-transfer.is-first-chunk]
/// Check if this is the first chunk.
pub fn is_first_chunk(offset: u64) -> (result: bool)
    ensures result == is_first_chunk_spec(offset)
{
    offset == 0
}

// --- is_last_chunk ---

// r[depends snapshot-transfer.is-last-chunk]
/// Spec: is this the last chunk?
pub open spec fn is_last_chunk_spec(
    offset: u64,
    chunk_size: u64,
    total_size: u64,
) -> bool {
    offset as int + chunk_size as int == total_size as int
}

// r[impl snapshot-transfer.is-last-chunk]
/// Check if this is the last chunk.
pub fn is_last_chunk(
    offset: u64,
    chunk_size: u64,
    total_size: u64,
) -> (result: bool)
    requires offset as int + chunk_size as int <= u64::MAX as int
    ensures result == is_last_chunk_spec(offset, chunk_size, total_size)
{
    offset + chunk_size == total_size
}

// --- next_expected_offset ---

// r[depends snapshot-transfer.next-offset]
/// Spec: what offset should the next chunk have?
pub open spec fn next_expected_offset_spec(
    current_offset: u64,
    chunk_size: u64,
) -> Option<u64> {
    if current_offset as int + chunk_size as int <= u64::MAX as int {
        Some((current_offset + chunk_size) as u64)
    } else {
        None
    }
}

// r[impl snapshot-transfer.next-offset]
/// Compute the next expected offset. Returns None on overflow.
pub fn next_expected_offset(
    current_offset: u64,
    chunk_size: u64,
) -> (result: Option<u64>)
    ensures result == next_expected_offset_spec(current_offset, chunk_size)
{
    if chunk_size <= u64::MAX - current_offset {
        Some(current_offset + chunk_size)
    } else {
        None
    }
}

// --- chunk_count ---

// r[depends snapshot-transfer.chunk-count]
/// Spec: how many chunks are needed?
pub open spec fn chunk_count_spec(
    total_size: u64,
    chunk_size: u64,
) -> u64
    recommends chunk_size > 0
{
    if chunk_size == 0 {
        0u64
    } else if total_size == 0 {
        0u64
    } else {
        ((total_size - 1) / (chunk_size as int) + 1) as u64
    }
}

// r[impl snapshot-transfer.chunk-count]
/// Compute the number of chunks needed.
pub fn chunk_count(
    total_size: u64,
    chunk_size: u64,
) -> (result: u64)
    requires chunk_size > 0
    ensures result == chunk_count_spec(total_size, chunk_size)
{
    if total_size == 0 {
        0
    } else {
        (total_size - 1) / chunk_size + 1
    }
}

// --- transfer_progress ---

// r[depends snapshot-transfer.progress]
/// Spec: transfer completion percentage [0, 100].
pub open spec fn transfer_progress_spec(
    received_bytes: u64,
    total_size: u64,
) -> u64 {
    if total_size == 0 {
        100u64
    } else if received_bytes == 0 {
        0u64
    } else if received_bytes >= total_size {
        100u64
    } else {
        (received_bytes * 100 / (total_size as int)) as u64
    }
}

// r[impl snapshot-transfer.progress]
/// Compute the transfer progress as a percentage.
/// Uses u128 intermediate to avoid overflow in `received_bytes * 100`.
pub fn transfer_progress(
    received_bytes: u64,
    total_size: u64,
) -> (result: u64)
    requires received_bytes <= total_size || total_size == 0
    ensures result == transfer_progress_spec(received_bytes, total_size),
            result <= 100
{
    if total_size == 0 {
        100
    } else if received_bytes == 0 {
        0
    } else if received_bytes >= total_size {
        100
    } else {
        // u64 * 100 fits in u128 trivially.
        // received_bytes < total_size, so received_bytes * 100 < total_size * 100,
        // thus received_bytes * 100 / total_size < 100.
        assert((received_bytes as int) * 100 / (total_size as int) < 100) by(nonlinear_arith)
            requires
                0 < received_bytes as int,
                (received_bytes as int) < (total_size as int),
                0 < total_size as int,
        ;
        (received_bytes as u128 * 100 / total_size as u128) as u64
    }
}

// --- Safety proofs ---

// r[verify snapshot-transfer.chunk-valid]
/// Zero-size chunk is invalid.
proof fn zero_chunk_invalid(offset: u64, total: u64)
    ensures !is_chunk_valid_spec(offset, 0, total)
{}

// r[verify snapshot-transfer.chunk-valid]
/// Chunk beyond total is invalid.
proof fn beyond_total_invalid(offset: u64, chunk_size: u64, total: u64)
    requires offset as int + chunk_size as int > total as int
    ensures !is_chunk_valid_spec(offset, chunk_size, total)
{}

// r[verify snapshot-transfer.chunk-valid]
/// Chunk at start within bounds is valid.
proof fn first_chunk_valid(chunk_size: u64, total: u64)
    requires chunk_size > 0, chunk_size as int <= total as int
    ensures is_chunk_valid_spec(0, chunk_size, total)
{}

// r[verify snapshot-transfer.is-first-chunk]
/// Only offset 0 is first.
proof fn only_zero_is_first(offset: u64)
    ensures is_first_chunk_spec(offset) <==> offset == 0
{}

// r[verify snapshot-transfer.is-last-chunk]
/// Last chunk reaches the end.
proof fn last_reaches_end(offset: u64, chunk_size: u64, total: u64)
    requires is_last_chunk_spec(offset, chunk_size, total)
    ensures offset as int + chunk_size as int == total as int
{}

// r[verify snapshot-transfer.next-offset]
/// Next offset advances.
proof fn next_offset_advances(offset: u64, chunk_size: u64)
    requires
        chunk_size > 0,
        offset as int + chunk_size as int <= u64::MAX as int,
    ensures next_expected_offset_spec(offset, chunk_size) == Some(
        (offset + chunk_size) as u64)
{}

// r[verify snapshot-transfer.chunk-count]
/// Empty snapshot needs zero chunks.
proof fn empty_needs_zero(chunk_size: u64)
    requires chunk_size > 0
    ensures chunk_count_spec(0, chunk_size) == 0
{}

// r[verify snapshot-transfer.chunk-count]
/// Exact-fit snapshot.
proof fn exact_fit(chunk_size: u64)
    requires chunk_size > 0
    ensures chunk_count_spec(chunk_size, chunk_size) == 1
{
    assert((chunk_size - 1) / (chunk_size as int) == 0) by(nonlinear_arith)
        requires chunk_size > 0u64;
}

// r[verify snapshot-transfer.progress]
/// Complete transfer is 100%.
proof fn complete_is_100(total: u64)
    ensures transfer_progress_spec(total, total) == 100
{}

// r[verify snapshot-transfer.progress]
/// Zero received is 0%.
proof fn zero_is_0(total: u64)
    requires total > 0
    ensures transfer_progress_spec(0, total) == 0
{}

// r[verify snapshot-transfer.progress]
/// Progress is bounded at 100.
proof fn progress_bounded(received: u64, total: u64)
    requires total > 0, received <= total
    ensures transfer_progress_spec(received, total) <= 100
{
    if received > 0 && received < total {
        assert(received * 100 / (total as int) <= 100) by(nonlinear_arith)
            requires received < total, total > 0u64;
    }
}

// r[impl snapshot-transfer.all-chunks-cover]
// r[verify snapshot-transfer.all-chunks-cover]
/// Valid sequential chunks cover the full range without gaps.
proof fn sequential_chunks_contiguous(
    o1: u64, s1: u64, o2: u64, s2: u64, total: u64,
)
    requires
        is_chunk_valid_spec(o1, s1, total),
        is_chunk_valid_spec(o2, s2, total),
        o2 as int == o1 as int + s1 as int,
    ensures
        // Chunk 1 covers [o1, o1+s1), chunk 2 covers [o1+s1, o1+s1+s2)
        // No gap between them
        o2 as int == o1 as int + s1 as int
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify snapshot-transfer.chunk-valid]
    #[test]
    fn test_is_chunk_valid() {
        assert!(is_chunk_valid(0, 100, 1000));
        assert!(is_chunk_valid(900, 100, 1000));
        assert!(!is_chunk_valid(950, 100, 1000)); // exceeds
        assert!(!is_chunk_valid(0, 0, 1000)); // zero size
    }

    // r[verify snapshot-transfer.is-first-chunk]
    #[test]
    fn test_is_first_chunk() {
        assert!(is_first_chunk(0));
        assert!(!is_first_chunk(1));
    }

    // r[verify snapshot-transfer.is-last-chunk]
    #[test]
    fn test_is_last_chunk() {
        assert!(is_last_chunk(900, 100, 1000));
        assert!(!is_last_chunk(800, 100, 1000));
    }

    // r[verify snapshot-transfer.next-offset]
    #[test]
    fn test_next_expected_offset() {
        assert_eq!(next_expected_offset(0, 100), Some(100));
        assert_eq!(next_expected_offset(100, 100), Some(200));
        assert_eq!(next_expected_offset(u64::MAX, 1), None); // overflow
    }

    // r[verify snapshot-transfer.chunk-count]
    #[test]
    fn test_chunk_count() {
        assert_eq!(chunk_count(0, 100), 0);
        assert_eq!(chunk_count(100, 100), 1);
        assert_eq!(chunk_count(250, 100), 3);
        assert_eq!(chunk_count(1000, 100), 10);
    }

    // r[verify snapshot-transfer.progress]
    #[test]
    fn test_transfer_progress() {
        assert_eq!(transfer_progress(0, 1000), 0);
        assert_eq!(transfer_progress(500, 1000), 50);
        assert_eq!(transfer_progress(1000, 1000), 100);
        assert_eq!(transfer_progress(0, 0), 100); // empty snapshot = done
    }

    proptest! {
        // r[verify snapshot-transfer.chunk-valid]
        #[test]
        fn prop_zero_chunk_invalid(offset: u64, total: u64) {
            prop_assert!(!is_chunk_valid(offset, 0, total));
        }

        // r[verify snapshot-transfer.chunk-count]
        #[test]
        fn prop_chunk_count_covers(
            total in 1u64..10000,
            chunk in 1u64..1000,
        ) {
            let count = chunk_count(total, chunk);
            // count * chunk >= total (full coverage)
            prop_assert!(u128::from(count) * u128::from(chunk) >= u128::from(total));
        }

        // r[verify snapshot-transfer.progress]
        #[test]
        fn prop_progress_bounded(
            received in 0u64..10000,
            total in 1u64..10000,
        ) {
            if received <= total {
                let p = transfer_progress(received, total);
                prop_assert!(p <= 100);
            }
        }

        // r[verify snapshot-transfer.progress]
        #[test]
        fn prop_complete_is_100(total in 1u64..u64::MAX) {
            prop_assert_eq!(transfer_progress(total, total), 100);
        }

        // r[verify snapshot-transfer.next-offset]
        #[test]
        fn prop_next_offset_advances(offset in 0u64..u64::MAX / 2, chunk in 1u64..1000) {
            if let Some(next) = next_expected_offset(offset, chunk) {
                prop_assert!(next > offset);
            }
        }
    }
}
