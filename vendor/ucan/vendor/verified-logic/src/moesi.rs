//! Verified MOESI cache coherence protocol.
//!
//! Extends the existing MESI line-state model with an Owned state.
//! The landed proof surface focuses on one-line coherence facts:
//! - single dirty-owner exclusivity across caches (`M` or `O`)
//! - the defining `M -> O` transition on a remote read
//! - dirty-owner data sourcing (cache, not memory)
//! - writeback on evicting an owned line
//! - pointwise refinement back to MESI on transitions that stay in `M/E/S/I`

use vstd::prelude::*;

verus! {

// --- States ---

/// Invalid: line is absent or stale.
pub const STATE_INVALID: u8 = 0u8;
/// Shared: clean copy, potentially replicated.
pub const STATE_SHARED: u8 = 1u8;
/// Exclusive: clean sole copy.
pub const STATE_EXCLUSIVE: u8 = 2u8;
/// Modified: dirty sole copy.
pub const STATE_MODIFIED: u8 = 3u8;
/// Owned: dirty shared supplier.
pub const STATE_OWNED: u8 = 4u8;

/// Processor read event.
pub const EVENT_READ: u8 = 0u8;
/// Processor write event.
pub const EVENT_WRITE: u8 = 1u8;

/// No bus traffic required.
pub const BUS_NONE: u8 = 0u8;
/// Shared read miss.
pub const BUS_READ: u8 = 1u8;
/// Read-for-ownership.
pub const BUS_READ_X: u8 = 2u8;
/// Upgrade a shared/owned copy to modified.
pub const BUS_UPGRADE: u8 = 3u8;

/// Read is satisfied from memory.
pub const SOURCE_MEMORY: u8 = 0u8;
/// Read is satisfied from some cache.
pub const SOURCE_CACHE: u8 = 1u8;

// r[depends moesi.transitions]
pub open spec fn is_valid_state_spec(state: u8) -> bool {
    state == STATE_INVALID
        || state == STATE_SHARED
        || state == STATE_EXCLUSIVE
        || state == STATE_MODIFIED
        || state == STATE_OWNED
}

// r[impl moesi.transitions]
pub fn is_valid_state(state: u8) -> (result: bool)
    ensures result == is_valid_state_spec(state),
{
    state == STATE_INVALID
        || state == STATE_SHARED
        || state == STATE_EXCLUSIVE
        || state == STATE_MODIFIED
        || state == STATE_OWNED
}

// r[depends moesi.transitions]
pub open spec fn is_valid_event_spec(event: u8) -> bool {
    event == EVENT_READ || event == EVENT_WRITE
}

// r[depends moesi.transitions]
pub open spec fn is_valid_bus_action_spec(action: u8) -> bool {
    action == BUS_NONE || action == BUS_READ || action == BUS_READ_X || action == BUS_UPGRADE
}

// r[depends moesi.single-writer]
pub open spec fn is_dirty_owner_state_spec(state: u8) -> bool {
    state == STATE_MODIFIED || state == STATE_OWNED
}

// r[impl moesi.single-writer]
pub open spec fn single_writer_spec(caches: Seq<u8>) -> bool {
    &&& forall|i: int| 0 <= i < caches.len() ==> is_valid_state_spec(caches[i])
    &&& forall|i: int, j: int|
        0 <= i < j < caches.len()
            ==> !(is_dirty_owner_state_spec(caches[i]) && is_dirty_owner_state_spec(caches[j]))
}

// r[depends moesi.no-stale-reads]
pub open spec fn requester_has_copy_spec(caches: Seq<u8>, requester: int) -> bool
    recommends 0 <= requester < caches.len()
{
    caches[requester] != STATE_INVALID
}

// r[depends moesi.no-stale-reads]
pub open spec fn dirty_owner_exists_spec(caches: Seq<u8>) -> bool {
    exists|i: int| 0 <= i < caches.len() && is_dirty_owner_state_spec(caches[i])
}

// r[impl moesi.no-stale-reads]
pub open spec fn memory_consistent_spec(caches: Seq<u8>, memory_up_to_date: bool) -> bool {
    memory_up_to_date == !dirty_owner_exists_spec(caches)
}

// r[impl moesi.no-stale-reads]
pub open spec fn read_source_spec(caches: Seq<u8>, requester: int, memory_up_to_date: bool) -> u8
    recommends 0 <= requester < caches.len()
{
    if requester_has_copy_spec(caches, requester) || dirty_owner_exists_spec(caches) {
        SOURCE_CACHE
    } else if memory_up_to_date {
        SOURCE_MEMORY
    } else {
        SOURCE_CACHE
    }
}

// r[impl moesi.no-stale-reads]
pub open spec fn read_fresh_spec(caches: Seq<u8>, requester: int, memory_up_to_date: bool) -> bool
    recommends 0 <= requester < caches.len()
{
    if read_source_spec(caches, requester, memory_up_to_date) == SOURCE_MEMORY {
        memory_up_to_date && !dirty_owner_exists_spec(caches)
    } else {
        requester_has_copy_spec(caches, requester) || dirty_owner_exists_spec(caches)
    }
}

// r[depends moesi.transitions]
pub open spec fn processor_transition_state_spec(state: u8, is_write: bool, others_have_copy: bool) -> u8 {
    if is_write {
        match state {
            STATE_INVALID => STATE_MODIFIED,
            STATE_SHARED => STATE_MODIFIED,
            STATE_EXCLUSIVE => STATE_MODIFIED,
            STATE_MODIFIED => STATE_MODIFIED,
            STATE_OWNED => STATE_MODIFIED,
            _ => STATE_INVALID,
        }
    } else {
        match state {
            STATE_INVALID => if others_have_copy { STATE_SHARED } else { STATE_EXCLUSIVE },
            STATE_SHARED => STATE_SHARED,
            STATE_EXCLUSIVE => STATE_EXCLUSIVE,
            STATE_MODIFIED => STATE_MODIFIED,
            STATE_OWNED => STATE_OWNED,
            _ => STATE_INVALID,
        }
    }
}

// r[impl moesi.transitions]
pub open spec fn processor_bus_action_spec(state: u8, is_write: bool, others_have_copy: bool) -> u8 {
    if is_write {
        match state {
            STATE_INVALID => BUS_READ_X,
            STATE_SHARED => BUS_UPGRADE,
            STATE_EXCLUSIVE => BUS_NONE,
            STATE_MODIFIED => BUS_NONE,
            STATE_OWNED => BUS_UPGRADE,
            _ => BUS_NONE,
        }
    } else {
        match state {
            STATE_INVALID => BUS_READ,
            STATE_SHARED => BUS_NONE,
            STATE_EXCLUSIVE => BUS_NONE,
            STATE_MODIFIED => BUS_NONE,
            STATE_OWNED => BUS_NONE,
            _ => BUS_NONE,
        }
    }
}

// r[impl moesi.transitions]
pub fn processor_transition(state: u8, is_write: bool, others_have_copy: bool) -> (result: (u8, u8))
    ensures
        result.0 == processor_transition_state_spec(state, is_write, others_have_copy),
        result.1 == processor_bus_action_spec(state, is_write, others_have_copy),
{
    if is_write {
        match state {
            STATE_INVALID => (STATE_MODIFIED, BUS_READ_X),
            STATE_SHARED | STATE_OWNED => (STATE_MODIFIED, BUS_UPGRADE),
            STATE_EXCLUSIVE | STATE_MODIFIED => (STATE_MODIFIED, BUS_NONE),
            _ => (STATE_INVALID, BUS_NONE),
        }
    } else {
        match state {
            STATE_INVALID => {
                if others_have_copy {
                    (STATE_SHARED, BUS_READ)
                } else {
                    (STATE_EXCLUSIVE, BUS_READ)
                }
            }
            STATE_SHARED => (STATE_SHARED, BUS_NONE),
            STATE_EXCLUSIVE => (STATE_EXCLUSIVE, BUS_NONE),
            STATE_MODIFIED => (STATE_MODIFIED, BUS_NONE),
            STATE_OWNED => (STATE_OWNED, BUS_NONE),
            _ => (STATE_INVALID, BUS_NONE),
        }
    }
}

// r[depends moesi.transitions]
pub open spec fn snoop_transition_state_spec(state: u8, bus_action: u8) -> u8 {
    match bus_action {
        BUS_READ =>
            if state == STATE_MODIFIED {
                STATE_OWNED
            } else if state == STATE_EXCLUSIVE {
                STATE_SHARED
            } else {
                state
            },
        BUS_READ_X => if state == STATE_INVALID { STATE_INVALID } else { STATE_INVALID },
        BUS_UPGRADE => if state == STATE_SHARED || state == STATE_OWNED {
            STATE_INVALID
        } else {
            state
        },
        _ => state,
    }
}

// r[impl moesi.owned-supply]
pub open spec fn dirty_data_supplied_spec(state: u8, bus_action: u8) -> bool {
    bus_action == BUS_READ && (state == STATE_MODIFIED || state == STATE_OWNED)
}

// r[impl moesi.transitions]
pub fn snoop_transition(state: u8, bus_action: u8) -> (result: u8)
    ensures result == snoop_transition_state_spec(state, bus_action),
{
    match bus_action {
        BUS_READ => {
            if state == STATE_MODIFIED {
                STATE_OWNED
            } else if state == STATE_EXCLUSIVE {
                STATE_SHARED
            } else {
                state
            }
        }
        BUS_READ_X => {
            if state == STATE_INVALID {
                STATE_INVALID
            } else {
                STATE_INVALID
            }
        }
        BUS_UPGRADE => {
            if state == STATE_SHARED || state == STATE_OWNED {
                STATE_INVALID
            } else {
                state
            }
        }
        _ => state,
    }
}

// r[impl moesi.owned-supply]
pub fn dirty_data_supplied(state: u8, bus_action: u8) -> (result: bool)
    ensures result == dirty_data_supplied_spec(state, bus_action),
{
    bus_action == BUS_READ && (state == STATE_MODIFIED || state == STATE_OWNED)
}

// r[depends moesi.writeback]
pub open spec fn evict_state_spec(state: u8) -> u8 {
    let _ = state;
    STATE_INVALID
}

// r[impl moesi.writeback]
pub open spec fn evict_writes_back_spec(state: u8) -> bool {
    state == STATE_MODIFIED || state == STATE_OWNED
}

// r[impl moesi.writeback]
pub fn evict_state(state: u8) -> (result: (u8, bool))
    ensures
        result.0 == evict_state_spec(state),
        result.1 == evict_writes_back_spec(state),
{
    (STATE_INVALID, state == STATE_MODIFIED || state == STATE_OWNED)
}

// r[depends moesi.writeback]
pub open spec fn evict_cache_spec(caches: Seq<u8>, idx: int) -> Seq<u8>
    recommends 0 <= idx < caches.len()
{
    caches.update(idx, STATE_INVALID)
}

// r[impl moesi.writeback]
pub open spec fn evict_memory_flag_spec(caches: Seq<u8>, idx: int, memory_up_to_date: bool) -> bool {
    if 0 <= idx < caches.len() && evict_writes_back_spec(caches[idx]) {
        true
    } else {
        memory_up_to_date
    }
}

// r[depends moesi.single-writer]
pub open spec fn any_other_has_copy_spec(caches: Seq<u8>, requester: int) -> bool {
    exists|i: int| 0 <= i < caches.len() && i != requester && caches[i] != STATE_INVALID
}

// r[impl moesi.single-writer]
pub open spec fn snoop_read_all_spec(caches: Seq<u8>, requester: int) -> Seq<u8> {
    Seq::new(caches.len(), |i: int|
        if i == requester {
            caches[i]
        } else {
            snoop_transition_state_spec(caches[i], BUS_READ)
        }
    )
}

// r[impl moesi.single-writer]
pub open spec fn invalidate_others_spec(caches: Seq<u8>, writer: int) -> Seq<u8> {
    Seq::new(caches.len(), |i: int|
        if i == writer {
            caches[i]
        } else {
            STATE_INVALID
        }
    )
}

// r[impl moesi.single-writer]
// r[impl moesi.owned-entry]
pub open spec fn full_read_spec(caches: Seq<u8>, requester: int) -> Seq<u8>
    recommends 0 <= requester < caches.len()
{
    let after_snoop = snoop_read_all_spec(caches, requester);
    let new_state = processor_transition_state_spec(after_snoop[requester], false, any_other_has_copy_spec(caches, requester));
    after_snoop.update(requester, new_state)
}

// r[impl moesi.single-writer]
pub open spec fn full_write_spec(caches: Seq<u8>, writer: int) -> Seq<u8>
    recommends 0 <= writer < caches.len()
{
    let after_inv = invalidate_others_spec(caches, writer);
    let new_state = processor_transition_state_spec(after_inv[writer], true, false);
    after_inv.update(writer, new_state)
}

// r[depends moesi.mesi-refinement]
// r[impl moesi.mesi-refinement]
pub open spec fn moesi_to_mesi_spec(state: u8) -> u8 {
    if state == STATE_OWNED { crate::mesi::STATE_SHARED } else { state }
}

proof fn update_dirty_owner_same_preserves_single_writer(caches: Seq<u8>, idx: int, new_state: u8)
    requires
        0 <= idx < caches.len(),
        single_writer_spec(caches),
        is_valid_state_spec(new_state),
        is_dirty_owner_state_spec(new_state) == is_dirty_owner_state_spec(caches[idx]),
    ensures
        single_writer_spec(caches.update(idx, new_state)),
{
    let updated = caches.update(idx, new_state);
    assert forall|i: int| 0 <= i < updated.len() implies is_valid_state_spec(updated[i]) by {
        if i == idx {
            assert(updated[i] == new_state);
        } else {
            assert(updated[i] == caches[i]);
        }
    };
    assert forall|i: int, j: int|
        0 <= i < j < updated.len()
            implies !(is_dirty_owner_state_spec(updated[i]) && is_dirty_owner_state_spec(updated[j])) by {
        if i == idx {
            assert(updated[i] == new_state);
            assert(is_dirty_owner_state_spec(updated[i]) == is_dirty_owner_state_spec(caches[i]));
            if j != idx {
                assert(updated[j] == caches[j]);
            }
        } else if j == idx {
            assert(updated[j] == new_state);
            assert(is_dirty_owner_state_spec(updated[j]) == is_dirty_owner_state_spec(caches[j]));
            assert(updated[i] == caches[i]);
        } else {
            assert(updated[i] == caches[i]);
            assert(updated[j] == caches[j]);
        }
    };
}

proof fn update_to_nondirty_preserves_single_writer(caches: Seq<u8>, idx: int, new_state: u8)
    requires
        0 <= idx < caches.len(),
        single_writer_spec(caches),
        is_valid_state_spec(new_state),
        !is_dirty_owner_state_spec(new_state),
    ensures
        single_writer_spec(caches.update(idx, new_state)),
{
    let updated = caches.update(idx, new_state);
    assert forall|i: int| 0 <= i < updated.len() implies is_valid_state_spec(updated[i]) by {
        if i == idx {
            assert(updated[i] == new_state);
        } else {
            assert(updated[i] == caches[i]);
        }
    };
    assert forall|i: int, j: int|
        0 <= i < j < updated.len()
            implies !(is_dirty_owner_state_spec(updated[i]) && is_dirty_owner_state_spec(updated[j])) by {
        if i == idx {
            assert(updated[i] == new_state);
            assert(!is_dirty_owner_state_spec(updated[i]));
        } else if j == idx {
            assert(updated[j] == new_state);
            assert(!is_dirty_owner_state_spec(updated[j]));
        } else {
            assert(updated[i] == caches[i]);
            assert(updated[j] == caches[j]);
        }
    };
}

// r[verify moesi.transitions]
pub proof fn transition_completeness(state: u8, is_write: bool, others_have_copy: bool, bus_action: u8)
    requires
        is_valid_state_spec(state),
        is_valid_bus_action_spec(bus_action),
    ensures
        is_valid_state_spec(processor_transition_state_spec(state, is_write, others_have_copy)),
        is_valid_bus_action_spec(processor_bus_action_spec(state, is_write, others_have_copy)),
        is_valid_state_spec(snoop_transition_state_spec(state, bus_action)),
{
}

proof fn bus_read_preserves_dirty_owner_kind(state: u8)
    requires is_valid_state_spec(state)
    ensures
        is_dirty_owner_state_spec(snoop_transition_state_spec(state, BUS_READ))
            == is_dirty_owner_state_spec(state),
{
}

proof fn processor_read_preserves_dirty_owner_kind(state: u8, others_have_copy: bool)
    requires is_valid_state_spec(state)
    ensures
        is_dirty_owner_state_spec(processor_transition_state_spec(state, false, others_have_copy))
            == is_dirty_owner_state_spec(state),
{
}

proof fn processor_write_produces_modified(state: u8)
    requires is_valid_state_spec(state)
    ensures processor_transition_state_spec(state, true, false) == STATE_MODIFIED,
{
}

// r[verify moesi.single-writer]
pub proof fn initial_all_invalid_is_single_writer(cache_count: nat)
    ensures single_writer_spec(Seq::new(cache_count, |i: int| STATE_INVALID)),
{
}

// r[verify moesi.single-writer]
pub proof fn full_read_preserves_single_writer(caches: Seq<u8>, requester: int)
    requires
        single_writer_spec(caches),
        0 <= requester < caches.len(),
    ensures single_writer_spec(full_read_spec(caches, requester)),
{
    let after_snoop = snoop_read_all_spec(caches, requester);
    assert(single_writer_spec(after_snoop)) by {
        assert forall|i: int| 0 <= i < after_snoop.len() implies is_valid_state_spec(after_snoop[i]) by {
            if i == requester {
                assert(after_snoop[i] == caches[i]);
            } else {
                transition_completeness(caches[i], false, false, BUS_READ);
                assert(after_snoop[i] == snoop_transition_state_spec(caches[i], BUS_READ));
            }
        };
        assert forall|i: int, j: int| 0 <= i < j < after_snoop.len()
            implies !(is_dirty_owner_state_spec(after_snoop[i]) && is_dirty_owner_state_spec(after_snoop[j])) by {
            if i == requester {
                assert(after_snoop[i] == caches[i]);
                if j != requester {
                    bus_read_preserves_dirty_owner_kind(caches[j]);
                    assert(is_dirty_owner_state_spec(after_snoop[j]) == is_dirty_owner_state_spec(caches[j]));
                }
            } else if j == requester {
                assert(after_snoop[j] == caches[j]);
                bus_read_preserves_dirty_owner_kind(caches[i]);
                assert(is_dirty_owner_state_spec(after_snoop[i]) == is_dirty_owner_state_spec(caches[i]));
            } else {
                bus_read_preserves_dirty_owner_kind(caches[i]);
                bus_read_preserves_dirty_owner_kind(caches[j]);
                assert(is_dirty_owner_state_spec(after_snoop[i]) == is_dirty_owner_state_spec(caches[i]));
                assert(is_dirty_owner_state_spec(after_snoop[j]) == is_dirty_owner_state_spec(caches[j]));
            }
        };
    };
    let next_state = processor_transition_state_spec(after_snoop[requester], false, any_other_has_copy_spec(caches, requester));
    transition_completeness(after_snoop[requester], false, any_other_has_copy_spec(caches, requester), BUS_READ);
    processor_read_preserves_dirty_owner_kind(after_snoop[requester], any_other_has_copy_spec(caches, requester));
    update_dirty_owner_same_preserves_single_writer(after_snoop, requester, next_state);
}

// r[verify moesi.single-writer]
pub proof fn full_write_preserves_single_writer(caches: Seq<u8>, writer: int)
    requires
        single_writer_spec(caches),
        0 <= writer < caches.len(),
    ensures single_writer_spec(full_write_spec(caches, writer)),
{
    let after_inv = invalidate_others_spec(caches, writer);
    assert forall|i: int| 0 <= i < after_inv.len() implies is_valid_state_spec(after_inv[i]) by {
        if i == writer {
            assert(after_inv[i] == caches[i]);
        } else {
            assert(after_inv[i] == STATE_INVALID);
        }
    };
    let next_state = processor_transition_state_spec(after_inv[writer], true, false);
    transition_completeness(after_inv[writer], true, false, BUS_READ_X);
    processor_write_produces_modified(after_inv[writer]);
    assert(next_state == STATE_MODIFIED);
    assert(full_write_spec(caches, writer) == after_inv.update(writer, STATE_MODIFIED));
    assert forall|i: int| 0 <= i < full_write_spec(caches, writer).len()
        implies is_valid_state_spec(full_write_spec(caches, writer)[i]) by {
        if i == writer {
            assert(full_write_spec(caches, writer)[i] == STATE_MODIFIED);
        } else {
            assert(full_write_spec(caches, writer)[i] == STATE_INVALID);
        }
    };
    assert forall|i: int, j: int| 0 <= i < j < full_write_spec(caches, writer).len()
        implies !(is_dirty_owner_state_spec(full_write_spec(caches, writer)[i])
            && is_dirty_owner_state_spec(full_write_spec(caches, writer)[j])) by {
        if i == writer {
            assert(full_write_spec(caches, writer)[i] == STATE_MODIFIED);
            assert(full_write_spec(caches, writer)[j] == STATE_INVALID);
        } else if j == writer {
            assert(full_write_spec(caches, writer)[j] == STATE_MODIFIED);
            assert(full_write_spec(caches, writer)[i] == STATE_INVALID);
        } else {
            assert(full_write_spec(caches, writer)[i] == STATE_INVALID);
            assert(full_write_spec(caches, writer)[j] == STATE_INVALID);
        }
    };
}

// r[verify moesi.single-writer]
pub proof fn evict_preserves_single_writer(caches: Seq<u8>, idx: int)
    requires
        single_writer_spec(caches),
        0 <= idx < caches.len(),
    ensures single_writer_spec(evict_cache_spec(caches, idx)),
{
    update_to_nondirty_preserves_single_writer(caches, idx, STATE_INVALID);
}

// r[verify moesi.owned-entry]
pub proof fn modified_remote_read_promotes_owned(caches: Seq<u8>, owner: int, requester: int)
    requires
        single_writer_spec(caches),
        0 <= owner < caches.len(),
        0 <= requester < caches.len(),
        owner != requester,
        caches[owner] == STATE_MODIFIED,
        caches[requester] == STATE_INVALID,
    ensures
        full_read_spec(caches, requester)[owner] == STATE_OWNED,
        full_read_spec(caches, requester)[requester] == STATE_SHARED,
{
    let after_snoop = snoop_read_all_spec(caches, requester);
    assert(after_snoop[owner] == STATE_OWNED);
    assert(after_snoop[requester] == STATE_INVALID);
    assert(any_other_has_copy_spec(caches, requester));
    assert(full_read_spec(caches, requester)[requester]
        == processor_transition_state_spec(after_snoop[requester], false, any_other_has_copy_spec(caches, requester)));
}

// r[verify moesi.owned-supply]
pub proof fn owned_holder_supplies_dirty_data(caches: Seq<u8>, requester: int, memory_up_to_date: bool)
    requires
        0 <= requester < caches.len(),
        memory_consistent_spec(caches, memory_up_to_date),
        exists|i: int| 0 <= i < caches.len() && i != requester && caches[i] == STATE_OWNED,
    ensures read_source_spec(caches, requester, memory_up_to_date) == SOURCE_CACHE,
{
}

// r[verify moesi.writeback]
pub proof fn owned_evict_writes_back(caches: Seq<u8>, idx: int, memory_up_to_date: bool)
    requires
        single_writer_spec(caches),
        memory_consistent_spec(caches, memory_up_to_date),
        0 <= idx < caches.len(),
        caches[idx] == STATE_OWNED,
    ensures
        evict_cache_spec(caches, idx)[idx] == STATE_INVALID,
        evict_memory_flag_spec(caches, idx, memory_up_to_date),
        memory_consistent_spec(evict_cache_spec(caches, idx), evict_memory_flag_spec(caches, idx, memory_up_to_date)),
{
    update_to_nondirty_preserves_single_writer(caches, idx, STATE_INVALID);
    assert(!dirty_owner_exists_spec(evict_cache_spec(caches, idx))) by {
        if dirty_owner_exists_spec(evict_cache_spec(caches, idx)) {
            let witness = choose|w: int| 0 <= w < evict_cache_spec(caches, idx).len()
                && is_dirty_owner_state_spec(evict_cache_spec(caches, idx)[w]);
            assert(witness != idx);
            assert(evict_cache_spec(caches, idx)[witness] == caches[witness]);
            assert(is_dirty_owner_state_spec(caches[witness]));
            assert(is_dirty_owner_state_spec(caches[idx]));
            assert(false);
        }
    };
}

// r[verify moesi.writeback]
pub proof fn shared_evict_skips_writeback(caches: Seq<u8>, idx: int, memory_up_to_date: bool)
    requires
        0 <= idx < caches.len(),
        caches[idx] == STATE_SHARED,
    ensures
        evict_cache_spec(caches, idx)[idx] == STATE_INVALID,
        evict_memory_flag_spec(caches, idx, memory_up_to_date) == memory_up_to_date,
{
}

// r[verify moesi.no-stale-reads]
pub proof fn memory_consistency_implies_read_fresh(caches: Seq<u8>, requester: int, memory_up_to_date: bool)
    requires
        0 <= requester < caches.len(),
        single_writer_spec(caches),
        memory_consistent_spec(caches, memory_up_to_date),
    ensures read_fresh_spec(caches, requester, memory_up_to_date),
{
}

// r[verify moesi.mesi-refinement]
pub proof fn processor_read_refines_mesi(state: u8, others_have_copy: bool)
    requires
        is_valid_state_spec(state),
        state != STATE_OWNED,
    ensures
        moesi_to_mesi_spec(processor_transition_state_spec(state, false, others_have_copy))
            == crate::mesi::processor_read_spec(seq![state], 0, others_have_copy)[0],
{
}

// r[verify moesi.mesi-refinement]
pub proof fn processor_write_refines_mesi(state: u8)
    requires
        is_valid_state_spec(state),
        state != STATE_OWNED,
    ensures
        moesi_to_mesi_spec(processor_transition_state_spec(state, true, false))
            == crate::mesi::processor_write_spec(seq![state], 0)[0],
{
}

// r[verify moesi.mesi-refinement]
pub proof fn snoop_read_refines_mesi(state: u8)
    requires
        is_valid_state_spec(state),
        state != STATE_OWNED,
        state != STATE_MODIFIED,
    ensures
        moesi_to_mesi_spec(snoop_transition_state_spec(state, BUS_READ))
            == crate::mesi::bus_read_spec(seq![state], 0)[0],
{
}

// r[verify moesi.mesi-refinement]
pub proof fn snoop_invalidate_refines_mesi(state: u8, action: u8)
    requires
        is_valid_state_spec(state),
        state != STATE_OWNED,
        (action == BUS_READ_X)
            || (action == BUS_UPGRADE && (state == STATE_SHARED || state == STATE_INVALID)),
    ensures
        moesi_to_mesi_spec(snoop_transition_state_spec(state, action))
            == crate::mesi::bus_invalidate_spec(seq![state], 0)[0],
{
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_state_is_rejected() {
        // r[verify moesi.transitions]
        let invalid_state = STATE_OWNED + 1;
        assert!(!is_valid_state(invalid_state));
    }

    #[test]
    fn modified_remote_read_becomes_owned() {
        // r[verify moesi.owned-entry]
        let owner = snoop_transition(STATE_MODIFIED, BUS_READ);
        let requester = processor_transition(STATE_INVALID, false, true).0;
        assert_eq!(owner, STATE_OWNED);
        assert_eq!(requester, STATE_SHARED);
        assert!(dirty_data_supplied(STATE_MODIFIED, BUS_READ));
    }

    #[test]
    fn owned_remote_read_stays_owned() {
        // r[verify moesi.owned-supply]
        let owner = snoop_transition(STATE_OWNED, BUS_READ);
        let requester = processor_transition(STATE_INVALID, false, true).0;
        assert_eq!(owner, STATE_OWNED);
        assert_eq!(requester, STATE_SHARED);
        assert!(dirty_data_supplied(STATE_OWNED, BUS_READ));
    }

    #[test]
    fn modified_evict_writes_back() {
        // r[verify moesi.writeback]
        let (next_state, writeback) = evict_state(STATE_MODIFIED);
        assert_eq!(next_state, STATE_INVALID);
        assert!(writeback);
    }

    #[test]
    fn owned_evict_writes_back() {
        // r[verify moesi.writeback]
        let (next_state, writeback) = evict_state(STATE_OWNED);
        assert_eq!(next_state, STATE_INVALID);
        assert!(writeback);
    }

    #[test]
    fn shared_evict_is_clean() {
        // r[verify moesi.writeback]
        let (next_state, writeback) = evict_state(STATE_SHARED);
        assert_eq!(next_state, STATE_INVALID);
        assert!(!writeback);
    }

    #[test]
    fn processor_write_from_owned_requests_upgrade() {
        // r[verify moesi.transitions]
        let transition = processor_transition(STATE_OWNED, true, false);
        assert_eq!(transition.0, STATE_MODIFIED);
        assert_eq!(transition.1, BUS_UPGRADE);
    }
}
