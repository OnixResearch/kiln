//! Verified cache-prefetch validity for a single cache line.
//!
//! The model keeps architectural state separate from microarchitectural cache
//! state. Prefetches may change only the cache state and reported coherence bus
//! action; the architectural state is returned unchanged. Dirty replacement is
//! modeled explicitly as a writeback into a finite memory vector.

use vstd::prelude::*;

use crate::mesi;

verus! {

/// Read-oriented prefetch hint.
pub const PREFETCH_HINT_READ: u8 = 0u8;
/// Write-oriented prefetch hint.
pub const PREFETCH_HINT_WRITE: u8 = 1u8;

/// No coherence transaction is issued.
pub const PREFETCH_BUS_NONE: u8 = 0u8;
/// Prefetch behaves like a demand read miss.
pub const PREFETCH_BUS_READ: u8 = 1u8;
/// Prefetch behaves like a demand write miss.
pub const PREFETCH_BUS_READ_X: u8 = 2u8;

#[derive(Debug, PartialEq, Eq)]
pub struct ArchState {
    pub regs: Vec<u64>,
    pub memory: Vec<u64>,
    pub pc: u64,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CacheSlot {
    pub tag: u64,
    pub state: u8,
    pub dirty: bool,
    pub data: u64,
}

pub open spec fn valid_cache_vector_spec(caches: Seq<u8>) -> bool {
    forall|i: int| #![trigger caches[i]] 0 <= i < caches.len() ==> mesi::is_valid_state_spec(caches[i])
}

pub open spec fn cache_slot_valid_spec(slot: CacheSlot) -> bool {
    mesi::is_valid_state_spec(slot.state)
}

pub open spec fn cache_slots_valid_spec(slots: Seq<CacheSlot>) -> bool {
    forall|i: int| #![trigger slots[i]] 0 <= i < slots.len() ==> cache_slot_valid_spec(slots[i])
}

// r[impl cache-prefetch.coherence]
pub open spec fn valid_prefetch_hint_spec(hint: u8) -> bool {
    hint == PREFETCH_HINT_READ || hint == PREFETCH_HINT_WRITE
}

// r[impl cache-prefetch.coherence]
pub fn valid_prefetch_hint(hint: u8) -> (result: bool)
    ensures result == valid_prefetch_hint_spec(hint),
{
    hint == PREFETCH_HINT_READ || hint == PREFETCH_HINT_WRITE
}

pub open spec fn valid_bus_action_spec(action: u8) -> bool {
    action == PREFETCH_BUS_NONE || action == PREFETCH_BUS_READ || action == PREFETCH_BUS_READ_X
}

#[cfg(test)]
fn valid_bus_action(action: u8) -> (result: bool)
    ensures result == valid_bus_action_spec(action),
{
    action == PREFETCH_BUS_NONE || action == PREFETCH_BUS_READ || action == PREFETCH_BUS_READ_X
}

pub open spec fn prefetch_bus_action_spec(hint: u8, addr_valid: bool) -> u8 {
    if !addr_valid {
        PREFETCH_BUS_NONE
    } else if hint == PREFETCH_HINT_READ {
        PREFETCH_BUS_READ
    } else if hint == PREFETCH_HINT_WRITE {
        PREFETCH_BUS_READ_X
    } else {
        PREFETCH_BUS_NONE
    }
}

// r[impl cache-prefetch.coherence]
pub fn prefetch_bus_action(hint: u8, addr_valid: bool) -> (result: u8)
    ensures result == prefetch_bus_action_spec(hint, addr_valid),
{
    if !addr_valid {
        PREFETCH_BUS_NONE
    } else if hint == PREFETCH_HINT_READ {
        PREFETCH_BUS_READ
    } else if hint == PREFETCH_HINT_WRITE {
        PREFETCH_BUS_READ_X
    } else {
        PREFETCH_BUS_NONE
    }
}

// r[impl cache-prefetch.no-fault]
// r[impl cache-prefetch.cache-effect]
pub open spec fn prefetch_read_cache_spec(caches: Seq<u8>, requester: int, addr_valid: bool) -> Seq<u8>
    recommends 0 <= requester < caches.len()
{
    if !addr_valid {
        caches
    } else {
        mesi::full_read_spec(caches, requester)
    }
}

// r[impl cache-prefetch.no-fault]
// r[impl cache-prefetch.cache-effect]
pub open spec fn prefetch_write_cache_spec(caches: Seq<u8>, requester: int, addr_valid: bool) -> Seq<u8>
    recommends 0 <= requester < caches.len()
{
    if !addr_valid {
        caches
    } else {
        mesi::invalidate_others_spec(caches, requester, 0).update(requester, mesi::STATE_EXCLUSIVE)
    }
}

// r[impl cache-prefetch.drop-safe]
pub open spec fn prefetch_drop_cache_spec(caches: Seq<u8>) -> Seq<u8> {
    caches
}

pub open spec fn writeback_memory_spec(memory: Seq<u64>, victim: CacheSlot) -> Seq<u64> {
    if victim.dirty && (victim.tag as int) < memory.len() {
        memory.update(victim.tag as int, victim.data)
    } else {
        memory
    }
}

// r[impl cache-prefetch.eviction-safe]
pub open spec fn replace_prefetch_line_spec(
    set: Seq<CacheSlot>,
    memory: Seq<u64>,
    victim: int,
    incoming: CacheSlot,
) -> (Seq<CacheSlot>, Seq<u64>)
    recommends 0 <= victim < set.len()
{
    (set.update(victim, incoming), writeback_memory_spec(memory, set[victim]))
}

fn clone_u8_vec(values: &[u8]) -> (result: Vec<u8>)
    ensures result@ == values@,
{
    let mut result: Vec<u8> = Vec::new();
    let mut index: usize = 0;
    while index < values.len()
        invariant
            0 <= index <= values@.len(),
            result@ == values@.subrange(0, index as int),
        decreases values@.len() - index,
    {
        result.push(values[index]);
        index = index + 1;
    }
    result
}

fn clone_u64_vec(values: &[u64]) -> (result: Vec<u64>)
    ensures result@ == values@,
{
    let mut result: Vec<u64> = Vec::new();
    let mut index: usize = 0;
    while index < values.len()
        invariant
            0 <= index <= values@.len(),
            result@ == values@.subrange(0, index as int),
        decreases values@.len() - index,
    {
        result.push(values[index]);
        index = index + 1;
    }
    result
}

fn clone_cache_slots(slots: &[CacheSlot]) -> (result: Vec<CacheSlot>)
    ensures result@ == slots@,
{
    let mut result: Vec<CacheSlot> = Vec::new();
    let mut index: usize = 0;
    while index < slots.len()
        invariant
            0 <= index <= slots@.len(),
            result@ == slots@.subrange(0, index as int),
        decreases slots@.len() - index,
    {
        result.push(slots[index]);
        index = index + 1;
    }
    result
}

fn clone_arch_state(arch: &ArchState) -> (result: ArchState)
    ensures
        result.regs@ == arch.regs@,
        result.memory@ == arch.memory@,
        result.pc == arch.pc,
{
    ArchState {
        regs: clone_u64_vec(&arch.regs),
        memory: clone_u64_vec(&arch.memory),
        pc: arch.pc,
    }
}

// r[impl cache-prefetch.no-fault]
// r[impl cache-prefetch.invisible]
// r[impl cache-prefetch.cache-effect]
pub fn prefetch_read(
    arch: &ArchState,
    caches: &[u8],
    requester: usize,
    addr_valid: bool,
) -> (result: (ArchState, Vec<u8>))
    requires
        requester < caches@.len(),
        valid_cache_vector_spec(caches@),
    ensures
        result.0.regs@ == arch.regs@,
        result.0.memory@ == arch.memory@,
        result.0.pc == arch.pc,
        result.1@ == prefetch_read_cache_spec(caches@, requester as int, addr_valid),
{
    let next_arch = clone_arch_state(arch);
    let mut next_caches = clone_u8_vec(caches);
    if addr_valid {
        mesi::full_read(&mut next_caches, requester);
    }
    (next_arch, next_caches)
}

// r[impl cache-prefetch.no-fault]
// r[impl cache-prefetch.invisible]
// r[impl cache-prefetch.cache-effect]
pub fn prefetch_write(
    arch: &ArchState,
    caches: &[u8],
    requester: usize,
    addr_valid: bool,
) -> (result: (ArchState, Vec<u8>))
    requires
        requester < caches@.len(),
        valid_cache_vector_spec(caches@),
    ensures
        result.0.regs@ == arch.regs@,
        result.0.memory@ == arch.memory@,
        result.0.pc == arch.pc,
        result.1@ == prefetch_write_cache_spec(caches@, requester as int, addr_valid),
{
    let next_arch = clone_arch_state(arch);
    let mut next_caches = clone_u8_vec(caches);
    if addr_valid {
        mesi::invalidate_others(&mut next_caches, requester);
        next_caches.set(requester, mesi::STATE_EXCLUSIVE);
        proof {
            assert(next_caches@
                == mesi::invalidate_others_spec(caches@, requester as int, 0).update(requester as int, mesi::STATE_EXCLUSIVE));
        }
    }
    (next_arch, next_caches)
}

// r[impl cache-prefetch.drop-safe]
// r[impl cache-prefetch.invisible]
pub fn prefetch_drop(arch: &ArchState, caches: &[u8]) -> (result: (ArchState, Vec<u8>))
    ensures
        result.0.regs@ == arch.regs@,
        result.0.memory@ == arch.memory@,
        result.0.pc == arch.pc,
        result.1@ == prefetch_drop_cache_spec(caches@),
{
    (clone_arch_state(arch), clone_u8_vec(caches))
}

// r[impl cache-prefetch.eviction-safe]
#[allow(clippy::cast_possible_truncation)]
pub fn replace_prefetch_line(
    set: &[CacheSlot],
    memory: &[u64],
    victim: usize,
    incoming: CacheSlot,
) -> (result: (Vec<CacheSlot>, Vec<u64>))
    requires
        victim < set@.len(),
        cache_slots_valid_spec(set@),
        cache_slot_valid_spec(incoming),
    ensures
        result.0@ == replace_prefetch_line_spec(set@, memory@, victim as int, incoming).0,
        result.1@ == replace_prefetch_line_spec(set@, memory@, victim as int, incoming).1,
{
    let mut next_set = clone_cache_slots(set);
    let mut next_memory = clone_u64_vec(memory);
    let victim_slot = set[victim];
    if victim_slot.dirty && victim_slot.tag < memory.len() as u64 {
        let memory_index = victim_slot.tag as usize;
        next_memory.set(memory_index, victim_slot.data);
    }
    next_set.set(victim, incoming);
    (next_set, next_memory)
}

// r[verify cache-prefetch.no-fault]
proof fn invalid_prefetch_read_is_noop(caches: Seq<u8>, requester: int)
    requires 0 <= requester < caches.len()
    ensures prefetch_read_cache_spec(caches, requester, false) == caches,
{}

// r[verify cache-prefetch.no-fault]
proof fn invalid_prefetch_write_is_noop(caches: Seq<u8>, requester: int)
    requires 0 <= requester < caches.len()
    ensures prefetch_write_cache_spec(caches, requester, false) == caches,
{}

// r[verify cache-prefetch.coherence]
proof fn read_prefetch_uses_bus_read()
    ensures prefetch_bus_action_spec(PREFETCH_HINT_READ, true) == PREFETCH_BUS_READ,
{}

// r[verify cache-prefetch.coherence]
proof fn write_prefetch_uses_bus_read_x()
    ensures prefetch_bus_action_spec(PREFETCH_HINT_WRITE, true) == PREFETCH_BUS_READ_X,
{}

// r[verify cache-prefetch.drop-safe]
proof fn dropped_prefetch_is_identity(caches: Seq<u8>)
    ensures prefetch_drop_cache_spec(caches) == caches,
{}

// r[verify cache-prefetch.eviction-safe]
proof fn dirty_victim_writes_back(
    set: Seq<CacheSlot>,
    memory: Seq<u64>,
    victim: int,
    incoming: CacheSlot,
)
    requires
        0 <= victim < set.len(),
        set[victim].dirty,
        (set[victim].tag as int) < memory.len(),
    ensures
        replace_prefetch_line_spec(set, memory, victim, incoming).1
            == memory.update(set[victim].tag as int, set[victim].data),
{}

// r[verify cache-prefetch.eviction-safe]
proof fn clean_victim_preserves_memory(
    set: Seq<CacheSlot>,
    memory: Seq<u64>,
    victim: int,
    incoming: CacheSlot,
)
    requires
        0 <= victim < set.len(),
        !set[victim].dirty,
    ensures replace_prefetch_line_spec(set, memory, victim, incoming).1 == memory,
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    const TEST_PC: u64 = 64u64;
    const TEST_REG_ZERO: u64 = 0u64;
    const TEST_REG_ONE: u64 = 1u64;
    const TEST_MEMORY_ZERO: u64 = 10u64;
    const TEST_MEMORY_ONE: u64 = 20u64;
    const TEST_VICTIM_TAG: u64 = 1u64;
    const TEST_DIRTY_DATA: u64 = 99u64;
    const TEST_CLEAN_DATA: u64 = 77u64;
    const TEST_NEW_TAG: u64 = 3u64;
    const TEST_NEW_DATA: u64 = 55u64;
    const TEST_CACHE_COUNT: usize = 4usize;
    const TEST_MEMORY_LEN: usize = 4usize;
    const RANDOM_CACHE_MIN: usize = 2usize;
    const RANDOM_CACHE_MAX: usize = 6usize;

    fn sample_arch() -> ArchState {
        ArchState {
            regs: vec![TEST_REG_ZERO, TEST_REG_ONE],
            memory: vec![TEST_MEMORY_ZERO, TEST_MEMORY_ONE],
            pc: TEST_PC,
        }
    }

    fn invalid_vector(count: usize) -> Vec<u8> {
        vec![mesi::STATE_INVALID; count]
    }

    fn valid_states() -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(
            prop_oneof![
                Just(mesi::STATE_INVALID),
                Just(mesi::STATE_SHARED),
                Just(mesi::STATE_EXCLUSIVE),
                Just(mesi::STATE_MODIFIED),
            ],
            RANDOM_CACHE_MIN..=RANDOM_CACHE_MAX,
        )
    }

    fn sample_slot(tag: u64, state: u8, dirty: bool, data: u64) -> CacheSlot {
        CacheSlot {
            tag,
            state,
            dirty,
            data,
        }
    }

    // r[verify cache-prefetch.no-fault]
    // r[verify cache-prefetch.invisible]
    #[test]
    fn test_prefetch_read_invalid_address_is_noop() {
        let arch = sample_arch();
        let caches = invalid_vector(TEST_CACHE_COUNT);
        let result = prefetch_read(&arch, &caches, 0usize, false);
        assert_eq!(result.0, arch);
        assert_eq!(result.1, caches);
        assert_eq!(prefetch_bus_action(PREFETCH_HINT_READ, false), PREFETCH_BUS_NONE);
    }

    // r[verify cache-prefetch.no-fault]
    // r[verify cache-prefetch.invisible]
    #[test]
    fn test_prefetch_write_invalid_address_is_noop() {
        let arch = sample_arch();
        let caches = invalid_vector(TEST_CACHE_COUNT);
        let result = prefetch_write(&arch, &caches, 1usize, false);
        assert_eq!(result.0, arch);
        assert_eq!(result.1, caches);
        assert_eq!(prefetch_bus_action(PREFETCH_HINT_WRITE, false), PREFETCH_BUS_NONE);
    }

    // r[verify cache-prefetch.cache-effect]
    // r[verify cache-prefetch.coherence]
    #[test]
    fn test_prefetch_read_enters_shared_when_other_cache_has_copy() {
        let arch = sample_arch();
        let caches = vec![
            mesi::STATE_SHARED,
            mesi::STATE_INVALID,
            mesi::STATE_INVALID,
            mesi::STATE_INVALID,
        ];
        let result = prefetch_read(&arch, &caches, 1usize, true);
        assert_eq!(result.0, arch);
        assert_eq!(result.1[0], mesi::STATE_SHARED);
        assert_eq!(result.1[1], mesi::STATE_SHARED);
        assert_eq!(prefetch_bus_action(PREFETCH_HINT_READ, true), PREFETCH_BUS_READ);
    }

    // r[verify cache-prefetch.cache-effect]
    #[test]
    fn test_prefetch_read_enters_exclusive_when_line_is_private() {
        let arch = sample_arch();
        let caches = invalid_vector(TEST_CACHE_COUNT);
        let result = prefetch_read(&arch, &caches, 2usize, true);
        assert_eq!(result.0, arch);
        assert_eq!(result.1[2], mesi::STATE_EXCLUSIVE);
    }

    // r[verify cache-prefetch.cache-effect]
    // r[verify cache-prefetch.coherence]
    #[test]
    fn test_prefetch_write_enters_exclusive_and_invalidates_others() {
        let arch = sample_arch();
        let caches = vec![
            mesi::STATE_SHARED,
            mesi::STATE_SHARED,
            mesi::STATE_INVALID,
            mesi::STATE_INVALID,
        ];
        let result = prefetch_write(&arch, &caches, 1usize, true);
        assert_eq!(result.0, arch);
        assert_eq!(result.1[1], mesi::STATE_EXCLUSIVE);
        assert_eq!(result.1[0], mesi::STATE_INVALID);
        assert_eq!(result.1[2], mesi::STATE_INVALID);
        assert_eq!(result.1[3], mesi::STATE_INVALID);
        assert_eq!(prefetch_bus_action(PREFETCH_HINT_WRITE, true), PREFETCH_BUS_READ_X);
    }

    // r[verify cache-prefetch.drop-safe]
    // r[verify cache-prefetch.invisible]
    #[test]
    fn test_prefetch_drop_is_identity() {
        let arch = sample_arch();
        let caches = vec![mesi::STATE_SHARED, mesi::STATE_INVALID];
        let result = prefetch_drop(&arch, &caches);
        assert_eq!(result.0, arch);
        assert_eq!(result.1, caches);
    }

    // r[verify cache-prefetch.eviction-safe]
    #[test]
    #[allow(clippy::cast_possible_truncation)]
    fn test_dirty_victim_writeback_updates_memory() {
        let set = vec![
            sample_slot(TEST_VICTIM_TAG, mesi::STATE_MODIFIED, true, TEST_DIRTY_DATA),
            sample_slot(TEST_NEW_TAG, mesi::STATE_INVALID, false, TEST_NEW_DATA),
        ];
        let memory = vec![0u64, TEST_MEMORY_ONE, 0u64, 0u64];
        let incoming = sample_slot(TEST_NEW_TAG, mesi::STATE_SHARED, false, TEST_NEW_DATA);
        let result = replace_prefetch_line(&set, &memory, 0usize, incoming);
        assert_eq!(result.0[0], incoming);
        assert_eq!(result.1[TEST_VICTIM_TAG as usize], TEST_DIRTY_DATA);
    }

    // r[verify cache-prefetch.eviction-safe]
    #[test]
    fn test_clean_victim_no_writeback() {
        let set = vec![
            sample_slot(TEST_VICTIM_TAG, mesi::STATE_SHARED, false, TEST_CLEAN_DATA),
            sample_slot(TEST_NEW_TAG, mesi::STATE_INVALID, false, TEST_NEW_DATA),
        ];
        let memory = vec![0u64, TEST_MEMORY_ONE, 0u64, 0u64];
        let incoming = sample_slot(TEST_NEW_TAG, mesi::STATE_SHARED, false, TEST_NEW_DATA);
        let result = replace_prefetch_line(&set, &memory, 0usize, incoming);
        assert_eq!(result.0[0], incoming);
        assert_eq!(result.1, memory);
    }

    // r[verify cache-prefetch.invisible]
    proptest! {
        #[test]
        fn prop_prefetch_read_preserves_arch(caches in valid_states()) {
            let arch = sample_arch();
            let requester = 0usize;
            let result = prefetch_read(&arch, &caches, requester % caches.len(), true);
            prop_assert_eq!(result.0, arch);
        }
    }

    // r[verify cache-prefetch.invisible]
    proptest! {
        #[test]
        fn prop_prefetch_write_preserves_arch(caches in valid_states()) {
            let arch = sample_arch();
            let requester = 0usize;
            let result = prefetch_write(&arch, &caches, requester % caches.len(), true);
            prop_assert_eq!(result.0, arch);
        }
    }

    // r[verify cache-prefetch.drop-safe]
    proptest! {
        #[test]
        fn prop_prefetch_drop_preserves_arch(caches in valid_states()) {
            let arch = sample_arch();
            let result = prefetch_drop(&arch, &caches);
            prop_assert_eq!(result.0, arch);
            prop_assert_eq!(result.1, caches);
        }
    }

    // r[verify cache-prefetch.coherence]
    proptest! {
        #[test]
        fn prop_bus_actions_are_valid(addr_valid in any::<bool>()) {
            let read_action = prefetch_bus_action(PREFETCH_HINT_READ, addr_valid);
            let write_action = prefetch_bus_action(PREFETCH_HINT_WRITE, addr_valid);
            prop_assert!(valid_bus_action(read_action));
            prop_assert!(valid_bus_action(write_action));
        }
    }

    // r[verify cache-prefetch.eviction-safe]
    proptest! {
        #[test]
        fn prop_replacement_changes_only_victim_slot(
            dirty in any::<bool>(),
            victim_index in 0usize..2usize,
            incoming_data in any::<u64>(),
        ) {
            let first = sample_slot(0u64, mesi::STATE_SHARED, dirty, TEST_DIRTY_DATA);
            let second = sample_slot(1u64, mesi::STATE_INVALID, false, TEST_NEW_DATA);
            let set = vec![first, second];
            let memory = vec![0u64; TEST_MEMORY_LEN];
            let incoming = sample_slot(TEST_NEW_TAG, mesi::STATE_EXCLUSIVE, false, incoming_data);
            let result = replace_prefetch_line(&set, &memory, victim_index, incoming);
            for index in 0usize..set.len() {
                if index == victim_index {
                    prop_assert_eq!(result.0[index], incoming);
                } else {
                    prop_assert_eq!(result.0[index], set[index]);
                }
            }
        }
    }
}
