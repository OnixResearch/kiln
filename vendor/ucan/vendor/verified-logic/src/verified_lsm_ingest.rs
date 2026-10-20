//! Verified LSM ingest facade.
//!
//! Validity boundary: the facade admits a flush only when the write buffer is
//! at threshold, the target LSM ratio is valid, the resulting run stays within
//! the target level budget, the compaction fence permits admission, and the
//! run block checksum validates. Rejection preserves buffered write sizes.

use vstd::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerifiedLsmIngestOutcome {
    Accepted {
        active_size: u64,
        immutable_size: u64,
        run_size: u64,
    },
    Rejected {
        active_size: u64,
        immutable_size: u64,
    },
}

verus! {

// r[impl composite-primitives.verified-lsm-ingest]
pub open spec fn verified_lsm_ingest_accepted_spec(
    active_size: u64,
    immutable_size: u64,
    threshold: u64,
    target_level: u64,
    base_size: u64,
    size_ratio: u64,
    current_level_size: u64,
    compact_index: u64,
    min_next_index: u64,
    block_bytes: Seq<u8>,
    expected_checksum: u16,
) -> bool {
    &&& crate::write_buffer::should_flush_spec(active_size, threshold)
    &&& crate::lsm_level::is_valid_size_ratio_spec(size_ratio)
    &&& base_size > 0
    &&& current_level_size as int + active_size as int <= u64::MAX as int
    &&& (current_level_size + active_size) as u64 <= crate::lsm_level::level_target_size_spec(target_level, base_size, size_ratio)
    &&& !crate::lsm_level::needs_compaction_spec(
        (current_level_size + active_size) as u64,
        crate::lsm_level::level_target_size_spec(target_level, base_size, size_ratio),
    )
    &&& crate::compaction_fence::is_safe_to_compact_spec(compact_index, min_next_index)
    &&& crate::page_checksum::is_valid_checksum_spec(block_bytes, expected_checksum)
}

// r[impl composite-primitives.verified-lsm-ingest]
pub open spec fn verified_lsm_ingest_spec(
    active_size: u64,
    immutable_size: u64,
    threshold: u64,
    target_level: u64,
    base_size: u64,
    size_ratio: u64,
    current_level_size: u64,
    compact_index: u64,
    min_next_index: u64,
    block_bytes: Seq<u8>,
    expected_checksum: u16,
) -> bool {
    verified_lsm_ingest_accepted_spec(
        active_size,
        immutable_size,
        threshold,
        target_level,
        base_size,
        size_ratio,
        current_level_size,
        compact_index,
        min_next_index,
        block_bytes,
        expected_checksum,
    )
}

// r[verify composite-primitives.verified-lsm-ingest]
pub proof fn verified_lsm_ingest_refines_sources(
    active_size: u64,
    immutable_size: u64,
    threshold: u64,
    target_level: u64,
    base_size: u64,
    size_ratio: u64,
    current_level_size: u64,
    compact_index: u64,
    min_next_index: u64,
    block_bytes: Seq<u8>,
    expected_checksum: u16,
)
    ensures
        verified_lsm_ingest_accepted_spec(
            active_size,
            immutable_size,
            threshold,
            target_level,
            base_size,
            size_ratio,
            current_level_size,
            compact_index,
            min_next_index,
            block_bytes,
            expected_checksum,
        ) ==> ({
            &&& crate::write_buffer::should_flush_spec(active_size, threshold)
            &&& crate::lsm_level::is_valid_size_ratio_spec(size_ratio)
            &&& crate::compaction_fence::is_safe_to_compact_spec(compact_index, min_next_index)
            &&& crate::page_checksum::is_valid_checksum_spec(block_bytes, expected_checksum)
        }),
{
}

// r[verify composite-primitives.verified-lsm-ingest]
pub proof fn verified_lsm_ingest_reject_preserves_buffer(
    active_size: u64,
    immutable_size: u64,
    threshold: u64,
    target_level: u64,
    base_size: u64,
    size_ratio: u64,
    current_level_size: u64,
    compact_index: u64,
    min_next_index: u64,
    block_bytes: Seq<u8>,
    expected_checksum: u16,
)
    ensures
        !verified_lsm_ingest_accepted_spec(
            active_size,
            immutable_size,
            threshold,
            target_level,
            base_size,
            size_ratio,
            current_level_size,
            compact_index,
            min_next_index,
            block_bytes,
            expected_checksum,
        ) ==> !verified_lsm_ingest_spec(
            active_size,
            immutable_size,
            threshold,
            target_level,
            base_size,
            size_ratio,
            current_level_size,
            compact_index,
            min_next_index,
            block_bytes,
            expected_checksum,
        ),
{
}

} // verus!

// r[impl composite-primitives.verified-lsm-ingest]
pub fn verified_lsm_ingest(
    active_size: u64,
    immutable_size: u64,
    threshold: u64,
    target_level: u64,
    base_size: u64,
    size_ratio: u64,
    current_level_size: u64,
    compact_index: u64,
    min_next_index: u64,
    block_bytes: &[u8],
    expected_checksum: u16,
) -> VerifiedLsmIngestOutcome {
    if !crate::write_buffer::should_flush(active_size, threshold) {
        return VerifiedLsmIngestOutcome::Rejected {
            active_size,
            immutable_size,
        };
    }
    if !crate::lsm_level::is_valid_size_ratio(size_ratio) || base_size == 0 {
        return VerifiedLsmIngestOutcome::Rejected {
            active_size,
            immutable_size,
        };
    }
    if current_level_size > u64::MAX - active_size {
        return VerifiedLsmIngestOutcome::Rejected {
            active_size,
            immutable_size,
        };
    }
    let run_size = current_level_size + active_size;
    let target_size = crate::lsm_level::level_target_size(target_level, base_size, size_ratio);
    if crate::lsm_level::needs_compaction(run_size, target_size) {
        return VerifiedLsmIngestOutcome::Rejected {
            active_size,
            immutable_size,
        };
    }
    if !crate::compaction_fence::is_safe_to_compact(compact_index, min_next_index) {
        return VerifiedLsmIngestOutcome::Rejected {
            active_size,
            immutable_size,
        };
    }
    if !crate::page_checksum::is_valid_checksum(block_bytes, expected_checksum) {
        return VerifiedLsmIngestOutcome::Rejected {
            active_size,
            immutable_size,
        };
    }

    let (swapped_active, swapped_immutable) = crate::write_buffer::swap(active_size, immutable_size);
    let (completed_active, completed_immutable) =
        crate::write_buffer::flush_complete(swapped_active, swapped_immutable);
    VerifiedLsmIngestOutcome::Accepted {
        active_size: completed_active,
        immutable_size: completed_immutable,
        run_size: active_size,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page_checksum::checksum;

    fn checksum_for(data: &[u8]) -> u16 {
        let (a, b) = checksum(data);
        crate::page_checksum::pack(a, b)
    }

    #[test]
    fn safe_flush_produces_level_safe_run() {
        let block = [1u8, 2, 3, 4];
        let checksum = checksum_for(&block);
        let outcome = verified_lsm_ingest(64, 0, 64, 1, 128, 4, 128, 10, 20, &block, checksum);
        assert_eq!(outcome, VerifiedLsmIngestOutcome::Accepted {
            active_size: 0,
            immutable_size: 0,
            run_size: 64
        });
    }

    #[test]
    fn unsafe_target_rejects_without_losing_buffer() {
        let block = [1u8, 2, 3, 4];
        let checksum = checksum_for(&block);
        let outcome = verified_lsm_ingest(64, 7, 64, 0, 64, 4, 64, 10, 20, &block, checksum);
        assert_eq!(outcome, VerifiedLsmIngestOutcome::Rejected {
            active_size: 64,
            immutable_size: 7
        });
    }

    #[test]
    fn unsafe_fence_rejects_without_losing_buffer() {
        let block = [9u8, 9];
        let checksum = checksum_for(&block);
        let outcome = verified_lsm_ingest(64, 11, 64, 1, 128, 4, 128, 20, 20, &block, checksum);
        assert_eq!(outcome, VerifiedLsmIngestOutcome::Rejected {
            active_size: 64,
            immutable_size: 11
        });
    }
}
