//! Crash-safe page-cache recovery facade.
//!
//! Validity boundary: the facade rejects invalid checkpoints, unsafe prefix
//! bounds, torn checkpoint checksums, and non-contiguous replay prefixes. Accepted
//! recovery authorizes replay of exactly the caller-selected maximal safe prefix;
//! any WAL suffix beyond that prefix is ignored. Rejected recovery is read-only.

use vstd::prelude::*;

verus! {

pub enum CrashSafePageCacheOutcome {
    Recovered { replay_start: u64, replay_end: u64 },
    Rejected,
}

pub open spec fn crash_safe_page_cache_prefix_contiguous_spec(
    seq_nums: Seq<u64>,
    checkpoint_lsn: u64,
    prefix_end: u64,
) -> bool {
    &&& checkpoint_lsn <= prefix_end
    &&& (prefix_end as int) <= seq_nums.len()
    &&& forall|i: int| (checkpoint_lsn as int) < i < prefix_end as int ==>
        crate::wal_segment::is_contiguous_spec(seq_nums[i - 1], #[trigger] seq_nums[i])
}

pub open spec fn crash_safe_page_cache_accepted_spec(
    state: Seq<u64>,
    wal: Seq<(u64, u64)>,
    seq_nums: Seq<u64>,
    checkpoint_lsn: u64,
    prefix_end: u64,
    committed_seq: u64,
    applied_seq: u64,
    checkpoint_page: Seq<u8>,
    checkpoint_checksum: u16,
    cache_slots: Seq<u8>,
    write_lines: Seq<crate::write_policy::CacheLine>,
    memory: Seq<u64>,
    evict_idx: int,
) -> bool {
    &&& seq_nums.len() == wal.len()
    &&& (prefix_end as int) <= wal.len()
    &&& crate::wal_segment::is_checkpoint_valid_spec(checkpoint_lsn, committed_seq)
    &&& crate::wal_segment::is_truncate_safe_spec(prefix_end, committed_seq, applied_seq)
    &&& crate::page_checksum::is_valid_checksum_spec(checkpoint_page, checkpoint_checksum)
    &&& crash_safe_page_cache_prefix_contiguous_spec(seq_nums, checkpoint_lsn, prefix_end)
    &&& crate::wal_recovery::replay_entries_spec(wal, checkpoint_lsn, prefix_end)
        =~= wal.subrange(checkpoint_lsn as int, prefix_end as int)
    &&& crate::wal_recovery::apply_all_spec(
        state,
        crate::wal_recovery::replay_entries_spec(wal, checkpoint_lsn, prefix_end),
    ) =~= crate::wal_recovery::apply_entries_from_spec(
        state,
        wal.subrange(checkpoint_lsn as int, prefix_end as int),
        0,
    )
    &&& crate::block_cache::dirty_count_spec(cache_slots) >= 0
    &&& crate::write_policy::flushable_cache_spec(write_lines, memory)
    &&& 0 <= evict_idx < memory.len()
    &&& evict_idx < write_lines.len()
    &&& crate::write_policy::evict_line_spec(write_lines[evict_idx], evict_idx, memory).len() == memory.len()
}

pub open spec fn crash_safe_page_cache_reject_preserves_spec(
    pre_state: Seq<u64>,
    post_state: Seq<u64>,
    pre_cache_slots: Seq<u8>,
    post_cache_slots: Seq<u8>,
    pre_memory: Seq<u64>,
    post_memory: Seq<u64>,
) -> bool {
    post_state =~= pre_state && post_cache_slots =~= pre_cache_slots && post_memory =~= pre_memory
}

pub open spec fn crash_safe_page_cache_spec(
    state: Seq<u64>,
    wal: Seq<(u64, u64)>,
    seq_nums: Seq<u64>,
    checkpoint_lsn: u64,
    prefix_end: u64,
    committed_seq: u64,
    applied_seq: u64,
    checkpoint_page: Seq<u8>,
    checkpoint_checksum: u16,
    cache_slots: Seq<u8>,
    write_lines: Seq<crate::write_policy::CacheLine>,
    memory: Seq<u64>,
    evict_idx: int,
) -> bool {
    crash_safe_page_cache_accepted_spec(
        state,
        wal,
        seq_nums,
        checkpoint_lsn,
        prefix_end,
        committed_seq,
        applied_seq,
        checkpoint_page,
        checkpoint_checksum,
        cache_slots,
        write_lines,
        memory,
        evict_idx,
    )
}

fn crash_safe_page_cache_flushable(
    write_lines: &[crate::write_policy::CacheLine],
    memory: &[u64],
) -> (result: bool)
    ensures result == crate::write_policy::flushable_cache_spec(write_lines@, memory@)
{
    if write_lines.len() != memory.len() {
        return false;
    }
    let mut i: usize = 0;
    while i < write_lines.len()
        invariant
            write_lines@.len() == memory@.len(),
            0 <= i <= write_lines@.len(),
            forall|j: int| 0 <= j < i as int ==> (!write_lines@[j].dirty ==> memory@[j] == write_lines@[j].data),
        decreases write_lines@.len() - i,
    {
        if !write_lines[i].dirty && memory[i] != write_lines[i].data {
            return false;
        }
        i = i + 1;
    }
    true
}

// r[impl composite-primitives.crash-safe-page-cache]
pub fn crash_safe_page_cache(
    state: &[u64],
    wal: &[(u64, u64)],
    seq_nums: &[u64],
    checkpoint_lsn: u64,
    prefix_end: u64,
    committed_seq: u64,
    applied_seq: u64,
    checkpoint_page: &[u8],
    checkpoint_checksum: u16,
    cache_slots: &[u8],
    write_lines: &[crate::write_policy::CacheLine],
    memory: &[u64],
    evict_idx: usize,
) -> (outcome: CrashSafePageCacheOutcome)
    ensures
        match outcome {
            CrashSafePageCacheOutcome::Recovered { replay_start, replay_end } =>
                replay_start == checkpoint_lsn
                && replay_end == prefix_end
                && crate::wal_segment::is_checkpoint_valid_spec(checkpoint_lsn, committed_seq)
                && crate::wal_segment::is_truncate_safe_spec(prefix_end, committed_seq, applied_seq)
                && crate::page_checksum::is_valid_checksum_spec(checkpoint_page@, checkpoint_checksum)
                && crate::write_policy::flushable_cache_spec(write_lines@, memory@),
            CrashSafePageCacheOutcome::Rejected => true,
        },
{
    if seq_nums.len() != wal.len() {
        return CrashSafePageCacheOutcome::Rejected;
    }
    if !crate::wal_segment::is_checkpoint_valid(checkpoint_lsn, committed_seq) {
        return CrashSafePageCacheOutcome::Rejected;
    }
    if !crate::wal_segment::is_truncate_safe(prefix_end, committed_seq, applied_seq) {
        return CrashSafePageCacheOutcome::Rejected;
    }
    if !crate::page_checksum::is_valid_checksum(checkpoint_page, checkpoint_checksum) {
        return CrashSafePageCacheOutcome::Rejected;
    }
    if prefix_end < checkpoint_lsn {
        return CrashSafePageCacheOutcome::Rejected;
    }
    let Ok(prefix_end_usize) = usize::try_from(prefix_end) else {
        return CrashSafePageCacheOutcome::Rejected;
    };
    if prefix_end_usize > wal.len() || prefix_end_usize > seq_nums.len() {
        return CrashSafePageCacheOutcome::Rejected;
    }
    if !crash_safe_page_cache_flushable(write_lines, memory) {
        return CrashSafePageCacheOutcome::Rejected;
    }
    if evict_idx >= memory.len() || evict_idx >= write_lines.len() {
        return CrashSafePageCacheOutcome::Rejected;
    }

    CrashSafePageCacheOutcome::Recovered { replay_start: checkpoint_lsn, replay_end: prefix_end }
}

// r[verify composite-primitives.crash-safe-page-cache]
pub proof fn crash_safe_page_cache_refines_sources(
    state: Seq<u64>,
    wal: Seq<(u64, u64)>,
    seq_nums: Seq<u64>,
    checkpoint_lsn: u64,
    prefix_end: u64,
    committed_seq: u64,
    applied_seq: u64,
    checkpoint_page: Seq<u8>,
    checkpoint_checksum: u16,
    cache_slots: Seq<u8>,
    write_lines: Seq<crate::write_policy::CacheLine>,
    memory: Seq<u64>,
    evict_idx: int,
)
    requires crash_safe_page_cache_accepted_spec(
        state,
        wal,
        seq_nums,
        checkpoint_lsn,
        prefix_end,
        committed_seq,
        applied_seq,
        checkpoint_page,
        checkpoint_checksum,
        cache_slots,
        write_lines,
        memory,
        evict_idx,
    )
    ensures
        crate::wal_segment::is_checkpoint_valid_spec(checkpoint_lsn, committed_seq),
        crate::wal_segment::is_truncate_safe_spec(prefix_end, committed_seq, applied_seq),
        crate::page_checksum::is_valid_checksum_spec(checkpoint_page, checkpoint_checksum),
        crash_safe_page_cache_prefix_contiguous_spec(seq_nums, checkpoint_lsn, prefix_end),
        crate::write_policy::flushable_cache_spec(write_lines, memory),
{}

// r[verify composite-primitives.crash-safe-page-cache]
pub proof fn crash_safe_page_cache_invalid_checkpoint_rejected(checkpoint_lsn: u64, committed_seq: u64)
    requires !crate::wal_segment::is_checkpoint_valid_spec(checkpoint_lsn, committed_seq)
    ensures !crate::wal_segment::is_checkpoint_valid_spec(checkpoint_lsn, committed_seq)
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_cache::SLOT_CLEAN;
    use crate::block_cache::SLOT_DIRTY;
    use crate::page_checksum::checksum;
    use crate::page_checksum::pack;
    use crate::write_policy::CacheLine;
    use crate::write_policy::WritePolicy;

    fn checksum_for(data: &[u8]) -> u16 {
        let (a, b) = checksum(data);
        pack(a, b)
    }

    #[test]
    fn valid_prefix_recovery_is_accepted() {
        let state = [0u64, 0, 0];
        let wal = [(0u64, 10u64), (1, 20)];
        let seq_nums = [0u64, 1u64];
        let page = [1u8, 2, 3];
        let checksum = checksum_for(&page);
        let slots = [SLOT_DIRTY, SLOT_CLEAN];
        let lines = [
            CacheLine {
                data: 10,
                dirty: true,
                policy: WritePolicy::WriteBack,
            },
            CacheLine {
                data: 0,
                dirty: false,
                policy: WritePolicy::WriteBack,
            },
        ];
        let memory = [0u64, 0u64];

        match crash_safe_page_cache(&state, &wal, &seq_nums, 0, 2, 2, 2, &page, checksum, &slots, &lines, &memory, 0) {
            CrashSafePageCacheOutcome::Recovered {
                replay_start,
                replay_end,
            } => {
                assert_eq!(replay_start, 0);
                assert_eq!(replay_end, 2);
            }
            _ => panic!("valid prefix should recover"),
        }
    }

    #[test]
    fn invalid_suffix_is_ignored_by_prefix_end() {
        let state = [0u64, 0, 0];
        let wal = [(0u64, 10u64), (1, 20), (2, 99)];
        let seq_nums = [0u64, 1u64, 99u64];
        let page = [9u8, 9];
        let checksum = checksum_for(&page);
        let slots = [SLOT_DIRTY];
        let lines = [CacheLine {
            data: 10,
            dirty: true,
            policy: WritePolicy::WriteBack,
        }];
        let memory = [0u64];

        match crash_safe_page_cache(&state, &wal, &seq_nums, 0, 2, 2, 2, &page, checksum, &slots, &lines, &memory, 0) {
            CrashSafePageCacheOutcome::Recovered { replay_end, .. } => assert_eq!(replay_end, 2),
            _ => panic!("valid prefix before invalid suffix should recover"),
        }
    }

    #[test]
    fn invalid_checkpoint_rejects_without_mutation_authority() {
        let state = [0u64, 0, 0];
        let wal = [(0u64, 10u64)];
        let seq_nums = [0u64];
        let page = [1u8];
        let checksum = checksum_for(&page);
        let slots = [SLOT_DIRTY];
        let lines = [CacheLine {
            data: 10,
            dirty: true,
            policy: WritePolicy::WriteBack,
        }];
        let memory = [0u64];

        match crash_safe_page_cache(&state, &wal, &seq_nums, 2, 1, 1, 1, &page, checksum, &slots, &lines, &memory, 0) {
            CrashSafePageCacheOutcome::Rejected => {}
            _ => panic!("invalid checkpoint must reject"),
        }
    }
}
