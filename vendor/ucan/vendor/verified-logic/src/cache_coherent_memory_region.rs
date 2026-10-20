//! Cache-coherent memory-region facade.
//!
//! Validity boundary: cache vectors, directory state, store-buffer entries,
//! prefetch hints, and write-policy lines are admitted only after the source
//! predicates agree that coherence, forwarding, prefetch, and dirty-line policy
//! facts are valid.

use vstd::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheCoherentMemoryRegionOutcome {
    Accepted,
    Rejected,
    Invalid,
}

verus! {

// r[impl composite-primitives.cache-coherent-memory-region]
pub open spec fn cache_coherent_memory_region_spec(
    mesi_caches: Seq<u8>,
    moesi_caches: Seq<u8>,
    directory: crate::directory_coherence::DirectoryEntry,
    store_buffer: Seq<crate::store_buffer::StoreEntry>,
    forward_entry: crate::store_buffer::StoreEntry,
    load_addr: u64,
    load_size: u8,
    requester: int,
    memory_up_to_date: bool,
    prefetch_hint: u8,
    prefetch_addr_valid: bool,
    lines: Seq<crate::write_policy::CacheLine>,
    memory: Seq<u64>,
    policy_line: crate::write_policy::CacheLine,
    new_data: u64,
    mem_addr: int,
) -> bool {
    &&& 0 <= requester < mesi_caches.len()
    &&& mesi_caches.len() == moesi_caches.len()
    &&& crate::mesi::system_invariant_spec(mesi_caches)
    &&& crate::mesi::system_invariant_spec(crate::mesi::processor_write_spec(mesi_caches, requester))
    &&& crate::moesi::single_writer_spec(moesi_caches)
    &&& crate::moesi::memory_consistent_spec(moesi_caches, memory_up_to_date)
    &&& crate::moesi::read_fresh_spec(moesi_caches, requester, memory_up_to_date)
    &&& crate::directory_coherence::valid_directory_entry_spec(directory, mesi_caches.len() as usize)
    &&& crate::directory_coherence::directory_cache_consistent_spec(directory, mesi_caches)
    &&& crate::store_buffer::buffer_valid_spec(store_buffer)
    &&& crate::store_buffer::can_forward_spec(forward_entry, load_addr, load_size)
    &&& crate::store_buffer::is_fence_satisfied_spec(Seq::<crate::store_buffer::StoreEntry>::empty())
    &&& crate::cache_prefetch::valid_cache_vector_spec(mesi_caches)
    &&& crate::cache_prefetch::valid_prefetch_hint_spec(prefetch_hint)
    &&& crate::cache_prefetch::valid_cache_vector_spec(crate::cache_prefetch::prefetch_read_cache_spec(mesi_caches, requester, prefetch_addr_valid))
    &&& crate::cache_prefetch::valid_cache_vector_spec(crate::cache_prefetch::prefetch_write_cache_spec(mesi_caches, requester, prefetch_addr_valid))
    &&& crate::write_policy::flushable_cache_spec(lines, memory)
    &&& (policy_line.policy == crate::write_policy::WritePolicy::WriteBack
        ==> crate::write_policy::write_back_store_spec(policy_line, new_data).dirty)
    &&& (policy_line.policy == crate::write_policy::WritePolicy::WriteThrough && 0 <= mem_addr < memory.len()
        ==> crate::write_policy::write_through_store_mem_spec(policy_line, new_data, mem_addr, memory)[mem_addr] == new_data)
}

// r[verify composite-primitives.cache-coherent-memory-region]
pub proof fn cache_coherent_memory_region_refines_sources(
    mesi_caches: Seq<u8>,
    moesi_caches: Seq<u8>,
    directory: crate::directory_coherence::DirectoryEntry,
    store_buffer: Seq<crate::store_buffer::StoreEntry>,
    forward_entry: crate::store_buffer::StoreEntry,
    load_addr: u64,
    load_size: u8,
    requester: int,
    memory_up_to_date: bool,
    prefetch_hint: u8,
    prefetch_addr_valid: bool,
    lines: Seq<crate::write_policy::CacheLine>,
    memory: Seq<u64>,
    policy_line: crate::write_policy::CacheLine,
    new_data: u64,
    mem_addr: int,
)
    ensures
        cache_coherent_memory_region_spec(
            mesi_caches, moesi_caches, directory, store_buffer, forward_entry,
            load_addr, load_size, requester, memory_up_to_date, prefetch_hint,
            prefetch_addr_valid, lines, memory, policy_line, new_data, mem_addr,
        ) ==> crate::moesi::single_writer_spec(moesi_caches),
        cache_coherent_memory_region_spec(
            mesi_caches, moesi_caches, directory, store_buffer, forward_entry,
            load_addr, load_size, requester, memory_up_to_date, prefetch_hint,
            prefetch_addr_valid, lines, memory, policy_line, new_data, mem_addr,
        ) ==> crate::directory_coherence::directory_cache_consistent_spec(directory, mesi_caches),
        cache_coherent_memory_region_spec(
            mesi_caches, moesi_caches, directory, store_buffer, forward_entry,
            load_addr, load_size, requester, memory_up_to_date, prefetch_hint,
            prefetch_addr_valid, lines, memory, policy_line, new_data, mem_addr,
        ) ==> crate::store_buffer::can_forward_spec(forward_entry, load_addr, load_size),
        cache_coherent_memory_region_spec(
            mesi_caches, moesi_caches, directory, store_buffer, forward_entry,
            load_addr, load_size, requester, memory_up_to_date, prefetch_hint,
            prefetch_addr_valid, lines, memory, policy_line, new_data, mem_addr,
        ) ==> crate::write_policy::flushable_cache_spec(lines, memory),
{
}

// r[verify composite-primitives.cache-coherent-memory-region]
pub proof fn cache_coherent_memory_region_rejects_invalid_inputs(
    mesi_caches: Seq<u8>,
    moesi_caches: Seq<u8>,
    directory: crate::directory_coherence::DirectoryEntry,
    store_buffer: Seq<crate::store_buffer::StoreEntry>,
    forward_entry: crate::store_buffer::StoreEntry,
    load_addr: u64,
    load_size: u8,
    requester: int,
    memory_up_to_date: bool,
    prefetch_hint: u8,
    prefetch_addr_valid: bool,
    lines: Seq<crate::write_policy::CacheLine>,
    memory: Seq<u64>,
    policy_line: crate::write_policy::CacheLine,
    new_data: u64,
    mem_addr: int,
)
    ensures
        (!crate::moesi::single_writer_spec(moesi_caches)
            || !crate::directory_coherence::directory_cache_consistent_spec(directory, mesi_caches)
            || !crate::store_buffer::can_forward_spec(forward_entry, load_addr, load_size)
            || !crate::cache_prefetch::valid_prefetch_hint_spec(prefetch_hint)
            || !crate::write_policy::flushable_cache_spec(lines, memory)) ==>
        !cache_coherent_memory_region_spec(
            mesi_caches, moesi_caches, directory, store_buffer, forward_entry,
            load_addr, load_size, requester, memory_up_to_date, prefetch_hint,
            prefetch_addr_valid, lines, memory, policy_line, new_data, mem_addr,
        ),
{
}

} // verus!

fn valid_mesi_state(state: u8) -> bool {
    state <= 3
}

fn valid_moesi_state(state: u8) -> bool {
    state <= 4
}

fn single_dirty_owner(caches: &[u8]) -> bool {
    let mut seen = false;
    for state in caches {
        if !valid_moesi_state(*state) {
            return false;
        }
        if *state == 3 || *state == 4 {
            if seen {
                return false;
            }
            seen = true;
        }
    }
    true
}

fn memory_consistent_runtime(caches: &[u8], memory_up_to_date: bool) -> bool {
    let dirty_owner = caches.iter().any(|state| *state == 3 || *state == 4);
    memory_up_to_date == !dirty_owner
}

fn flushable_runtime(lines: &[crate::write_policy::CacheLine], memory: &[u64]) -> bool {
    lines.len() == memory.len() && lines.iter().zip(memory.iter()).all(|(line, mem)| line.dirty || line.data == *mem)
}

// r[impl composite-primitives.cache-coherent-memory-region]
#[allow(clippy::too_many_arguments)]
pub fn cache_coherent_memory_region(
    mesi_caches: &[u8],
    moesi_caches: &[u8],
    directory: &crate::directory_coherence::DirectoryEntry,
    store_buffer: &[crate::store_buffer::StoreEntry],
    forward_entry: crate::store_buffer::StoreEntry,
    load_addr: u64,
    load_size: u8,
    requester: usize,
    memory_up_to_date: bool,
    prefetch_hint: u8,
    prefetch_addr_valid: bool,
    lines: &[crate::write_policy::CacheLine],
    memory: &[u64],
    policy_line: crate::write_policy::CacheLine,
    new_data: u64,
    mem_addr: usize,
) -> CacheCoherentMemoryRegionOutcome {
    if requester >= mesi_caches.len()
        || mesi_caches.len() != moesi_caches.len()
        || !mesi_caches.iter().all(|state| valid_mesi_state(*state))
        || !moesi_caches.iter().all(|state| valid_moesi_state(*state))
        || !(prefetch_hint == 0 || prefetch_hint == 1)
        || !(load_size == 1 || load_size == 2 || load_size == 4 || load_size == 8)
        || store_buffer.len() > 8
    {
        return CacheCoherentMemoryRegionOutcome::Invalid;
    }

    if !crate::mesi::check_system_invariant(&mesi_caches.to_vec())
        || !single_dirty_owner(moesi_caches)
        || !memory_consistent_runtime(moesi_caches, memory_up_to_date)
        || !crate::directory_coherence::valid_directory_entry(directory, mesi_caches.len())
        || !crate::directory_coherence::check_consistency(directory, mesi_caches)
        || !crate::cache_prefetch::valid_prefetch_hint(prefetch_hint)
        || !flushable_runtime(lines, memory)
    {
        return CacheCoherentMemoryRegionOutcome::Rejected;
    }

    for window in store_buffer.windows(2) {
        if window[0].seq >= window[1].seq {
            return CacheCoherentMemoryRegionOutcome::Invalid;
        }
    }
    for entry in store_buffer {
        if !(entry.size == 1 || entry.size == 2 || entry.size == 4 || entry.size == 8) {
            return CacheCoherentMemoryRegionOutcome::Invalid;
        }
    }

    let store_end = forward_entry.addr.saturating_add(forward_entry.size as u64);
    let load_end = load_addr.saturating_add(load_size as u64);
    if !(forward_entry.size == 1 || forward_entry.size == 2 || forward_entry.size == 4 || forward_entry.size == 8)
        || forward_entry.addr > load_addr
        || load_end > store_end
    {
        return CacheCoherentMemoryRegionOutcome::Rejected;
    }

    let _prefetch_bus = crate::cache_prefetch::prefetch_bus_action(prefetch_hint, prefetch_addr_valid);

    if policy_line.policy == crate::write_policy::WritePolicy::WriteThrough && mem_addr >= memory.len() {
        return CacheCoherentMemoryRegionOutcome::Invalid;
    }
    let _ = new_data;

    CacheCoherentMemoryRegionOutcome::Accepted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::directory_coherence::DIR_EXCLUSIVE;
    use crate::directory_coherence::DirectoryEntry;
    use crate::store_buffer::StoreEntry;
    use crate::write_policy::CacheLine;
    use crate::write_policy::WritePolicy;

    #[test]
    fn exclusive_writer_remains_unique() {
        let directory = DirectoryEntry {
            owner: Some(0),
            sharers: vec![false, false],
            state: DIR_EXCLUSIVE,
        };
        let line = CacheLine {
            data: 7,
            dirty: false,
            policy: WritePolicy::WriteBack,
        };
        assert_eq!(
            cache_coherent_memory_region(
                &[2, 0],
                &[3, 0],
                &directory,
                &[StoreEntry {
                    addr: 16,
                    size: 4,
                    data: 9,
                    seq: 1
                }],
                StoreEntry {
                    addr: 16,
                    size: 4,
                    data: 9,
                    seq: 2
                },
                18,
                2,
                0,
                false,
                1,
                true,
                &[line],
                &[7],
                line,
                11,
                0,
            ),
            CacheCoherentMemoryRegionOutcome::Accepted,
        );
    }

    #[test]
    fn second_dirty_owner_rejected() {
        let directory = DirectoryEntry {
            owner: Some(0),
            sharers: vec![false, false],
            state: DIR_EXCLUSIVE,
        };
        let line = CacheLine {
            data: 7,
            dirty: false,
            policy: WritePolicy::WriteBack,
        };
        assert_eq!(
            cache_coherent_memory_region(
                &[2, 0],
                &[3, 4],
                &directory,
                &[],
                StoreEntry {
                    addr: 16,
                    size: 4,
                    data: 9,
                    seq: 2
                },
                16,
                4,
                0,
                false,
                0,
                true,
                &[line],
                &[7],
                line,
                11,
                0,
            ),
            CacheCoherentMemoryRegionOutcome::Rejected,
        );
    }

    #[test]
    fn prefetch_or_forwarding_shape_violation_rejected() {
        let directory = DirectoryEntry {
            owner: Some(0),
            sharers: vec![false, false],
            state: DIR_EXCLUSIVE,
        };
        let line = CacheLine {
            data: 7,
            dirty: false,
            policy: WritePolicy::WriteBack,
        };
        assert_eq!(
            cache_coherent_memory_region(
                &[2, 0],
                &[3, 0],
                &directory,
                &[],
                StoreEntry {
                    addr: 20,
                    size: 1,
                    data: 9,
                    seq: 2
                },
                16,
                4,
                0,
                false,
                1,
                true,
                &[line],
                &[7],
                line,
                11,
                0,
            ),
            CacheCoherentMemoryRegionOutcome::Rejected,
        );
    }

    #[test]
    fn invalid_shapes_reject_before_coherence_claim() {
        let directory = DirectoryEntry {
            owner: Some(0),
            sharers: vec![false],
            state: DIR_EXCLUSIVE,
        };
        let line = CacheLine {
            data: 7,
            dirty: false,
            policy: WritePolicy::WriteThrough,
        };
        assert_eq!(
            cache_coherent_memory_region(
                &[2, 0],
                &[3, 0],
                &directory,
                &[],
                StoreEntry {
                    addr: 16,
                    size: 4,
                    data: 9,
                    seq: 2
                },
                16,
                4,
                0,
                false,
                7,
                true,
                &[line],
                &[7],
                line,
                11,
                0,
            ),
            CacheCoherentMemoryRegionOutcome::Invalid,
        );
    }
}
