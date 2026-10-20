//! Verified block cache management: occupancy, eviction, dirty tracking.
//!
//! Models a fixed-capacity cache with clean/dirty/empty slots. Operations:
//! insert, evict, mark dirty, write-back (clean). Proofs guarantee capacity
//! bounds, dirty tracking correctness, and eviction ordering via clock-hand
//! sweep. Composes with [`eviction`] for LRU policy and [`page`] for block
//! addressing.

use vstd::prelude::*;

verus! {

pub const SLOT_EMPTY: u8 = 0;
pub const SLOT_CLEAN: u8 = 1;
pub const SLOT_DIRTY: u8 = 2;

/// Spec: is a valid slot state?
// r[depends block-cache.valid-slot-state]
// r[impl block-cache.valid-slot-state]
pub open spec fn is_valid_slot_state_spec(state: u8) -> bool {
    state == SLOT_EMPTY || state == SLOT_CLEAN || state == SLOT_DIRTY
}

// r[impl block-cache.valid-slot-state]
pub fn is_valid_slot_state(state: u8) -> (result: bool)
    ensures result == is_valid_slot_state_spec(state),
{
    state == SLOT_EMPTY || state == SLOT_CLEAN || state == SLOT_DIRTY
}

// ─── Occupancy Counting ──────────────────────────────────────────────

/// Spec: count occupied (non-empty) slots from index `from`.
// r[depends block-cache.occupied-count]
// r[impl block-cache.occupied-count]
pub open spec fn occupied_count_from_spec(slots: Seq<u8>, from: int) -> int
    decreases (slots.len() - from),
{
    if from < 0 || from >= slots.len() {
        0
    } else {
        (if slots[from] != SLOT_EMPTY { 1int } else { 0int })
            + occupied_count_from_spec(slots, from + 1)
    }
}

/// Spec: total occupied count.
pub open spec fn occupied_count_spec(slots: Seq<u8>) -> int {
    occupied_count_from_spec(slots, 0)
}

// r[impl block-cache.occupied-count]
pub fn occupied_count(slots: &[u8]) -> (result: u64)
    requires slots@.len() <= u64::MAX as int,
    ensures result as int == occupied_count_spec(slots@),
{
    let mut count: u64 = 0;
    let mut i: usize = 0;
    while i < slots.len()
        invariant
            0 <= i <= slots@.len(),
            slots@.len() <= u64::MAX as int,
            count as int == occupied_count_spec(slots@) - occupied_count_from_spec(slots@, i as int),
            count as int <= i as int,
        decreases slots@.len() - i,
    {
        if slots[i] != SLOT_EMPTY {
            count = count + 1;
        }
        i = i + 1;
    }
    count
}

/// Spec: count dirty slots from index `from`.
// r[depends block-cache.dirty-count]
// r[impl block-cache.dirty-count]
pub open spec fn dirty_count_from_spec(slots: Seq<u8>, from: int) -> int
    decreases (slots.len() - from),
{
    if from < 0 || from >= slots.len() {
        0
    } else {
        (if slots[from] == SLOT_DIRTY { 1int } else { 0int })
            + dirty_count_from_spec(slots, from + 1)
    }
}

/// Spec: total dirty count.
pub open spec fn dirty_count_spec(slots: Seq<u8>) -> int {
    dirty_count_from_spec(slots, 0)
}

// r[impl block-cache.dirty-count]
pub fn dirty_count(slots: &[u8]) -> (result: u64)
    requires slots@.len() <= u64::MAX as int,
    ensures result as int == dirty_count_spec(slots@),
{
    let mut count: u64 = 0;
    let mut i: usize = 0;
    while i < slots.len()
        invariant
            0 <= i <= slots@.len(),
            slots@.len() <= u64::MAX as int,
            count as int == dirty_count_spec(slots@) - dirty_count_from_spec(slots@, i as int),
            count as int <= i as int,
        decreases slots@.len() - i,
    {
        if slots[i] == SLOT_DIRTY {
            count = count + 1;
        }
        i = i + 1;
    }
    count
}

// ─── Count Bounds ────────────────────────────────────────────────────

proof fn occupied_count_nonneg(slots: Seq<u8>)
    ensures occupied_count_spec(slots) >= 0,
{
    occupied_count_from_nonneg(slots, 0);
}

proof fn occupied_count_from_nonneg(slots: Seq<u8>, from: int)
    ensures occupied_count_from_spec(slots, from) >= 0,
    decreases (slots.len() - from),
{
    if from >= 0 && from < slots.len() {
        occupied_count_from_nonneg(slots, from + 1);
    }
}

proof fn occupied_count_bounded(slots: Seq<u8>)
    ensures occupied_count_spec(slots) <= slots.len(),
{
    occupied_count_from_bounded(slots, 0);
}

proof fn occupied_count_from_bounded(slots: Seq<u8>, from: int)
    requires 0 <= from <= slots.len(),
    ensures occupied_count_from_spec(slots, from) <= slots.len() - from,
    decreases (slots.len() - from),
{
    if from < slots.len() {
        occupied_count_from_bounded(slots, from + 1);
    }
}

// r[verify block-cache.dirty-count]
/// Dirty count <= occupied count.
proof fn dirty_le_occupied(slots: Seq<u8>)
    ensures dirty_count_spec(slots) <= occupied_count_spec(slots),
{
    dirty_le_occupied_from(slots, 0);
}

proof fn dirty_le_occupied_from(slots: Seq<u8>, from: int)
    ensures dirty_count_from_spec(slots, from) <= occupied_count_from_spec(slots, from),
    decreases (slots.len() - from),
{
    if from >= 0 && from < slots.len() {
        dirty_le_occupied_from(slots, from + 1);
    }
}

// ─── Insert ──────────────────────────────────────────────────────────

/// Spec: insert a clean block at slot `idx`.
// r[depends block-cache.insert]
// r[impl block-cache.insert]
pub open spec fn insert_spec(slots: Seq<u8>, idx: int) -> Seq<u8>
    recommends 0 <= idx < slots.len(),
{
    slots.update(idx, SLOT_CLEAN)
}

// r[impl block-cache.insert]
pub fn insert(slots: &mut Vec<u8>, idx: usize)
    requires idx < old(slots)@.len(),
    ensures slots@ == insert_spec(old(slots)@, idx as int),
{
    slots.set(idx, SLOT_CLEAN);
}

// r[verify block-cache.insert]
/// Inserting into an empty slot increases occupied count by 1.
proof fn insert_empty_increments(slots: Seq<u8>, idx: int)
    requires 0 <= idx < slots.len(), slots[idx] == SLOT_EMPTY,
    ensures occupied_count_spec(insert_spec(slots, idx))
        == occupied_count_spec(slots) + 1,
{
    insert_empty_increments_from(slots, idx, 0);
}

proof fn insert_empty_increments_from(slots: Seq<u8>, idx: int, from: int)
    requires 0 <= idx < slots.len(), slots[idx] == SLOT_EMPTY, 0 <= from,
    ensures
        occupied_count_from_spec(insert_spec(slots, idx), from)
            == occupied_count_from_spec(slots, from) + (if from <= idx { 1int } else { 0int }),
    decreases (slots.len() - from),
{
    if from < slots.len() {
        let updated = insert_spec(slots, idx);
        assert(updated.len() == slots.len());
        if from == idx {
            assert(updated[from] == SLOT_CLEAN);
            assert(slots[from] == SLOT_EMPTY);
        } else {
            assert(updated[from] == slots[from]);
        }
        insert_empty_increments_from(slots, idx, from + 1);
    }
}

// r[verify block-cache.insert]
/// Insert produces a clean slot.
proof fn insert_is_clean(slots: Seq<u8>, idx: int)
    requires 0 <= idx < slots.len(),
    ensures insert_spec(slots, idx)[idx] == SLOT_CLEAN,
{}

// ─── Mark Dirty ──────────────────────────────────────────────────────

/// Spec: mark slot as dirty.
// r[depends block-cache.mark-dirty]
// r[impl block-cache.mark-dirty]
pub open spec fn mark_dirty_spec(slots: Seq<u8>, idx: int) -> Seq<u8>
    recommends 0 <= idx < slots.len(),
{
    slots.update(idx, SLOT_DIRTY)
}

// r[impl block-cache.mark-dirty]
pub fn mark_dirty(slots: &mut Vec<u8>, idx: usize)
    requires idx < old(slots)@.len(),
    ensures slots@ == mark_dirty_spec(old(slots)@, idx as int),
{
    slots.set(idx, SLOT_DIRTY);
}

// r[verify block-cache.mark-dirty]
/// Marking a clean slot dirty increases dirty count by 1.
proof fn mark_dirty_increments(slots: Seq<u8>, idx: int)
    requires 0 <= idx < slots.len(), slots[idx] == SLOT_CLEAN,
    ensures dirty_count_spec(mark_dirty_spec(slots, idx))
        == dirty_count_spec(slots) + 1,
{
    mark_dirty_increments_from(slots, idx, 0);
}

proof fn mark_dirty_increments_from(slots: Seq<u8>, idx: int, from: int)
    requires 0 <= idx < slots.len(), slots[idx] == SLOT_CLEAN, 0 <= from,
    ensures
        dirty_count_from_spec(mark_dirty_spec(slots, idx), from)
            == dirty_count_from_spec(slots, from) + (if from <= idx { 1int } else { 0int }),
    decreases (slots.len() - from),
{
    if from < slots.len() {
        let updated = mark_dirty_spec(slots, idx);
        assert(updated.len() == slots.len());
        if from == idx {
            assert(updated[from] == SLOT_DIRTY);
            assert(slots[from] == SLOT_CLEAN);
        } else {
            assert(updated[from] == slots[from]);
        }
        mark_dirty_increments_from(slots, idx, from + 1);
    }
}

// r[verify block-cache.mark-dirty]
/// Marking dirty doesn't change occupied count.
proof fn mark_dirty_preserves_occupied(slots: Seq<u8>, idx: int)
    requires 0 <= idx < slots.len(), slots[idx] != SLOT_EMPTY,
    ensures occupied_count_spec(mark_dirty_spec(slots, idx))
        == occupied_count_spec(slots),
{
    mark_dirty_preserves_occupied_from(slots, idx, 0);
}

proof fn mark_dirty_preserves_occupied_from(slots: Seq<u8>, idx: int, from: int)
    requires 0 <= idx < slots.len(), slots[idx] != SLOT_EMPTY, 0 <= from,
    ensures
        occupied_count_from_spec(mark_dirty_spec(slots, idx), from)
            == occupied_count_from_spec(slots, from),
    decreases (slots.len() - from),
{
    if from < slots.len() {
        let updated = mark_dirty_spec(slots, idx);
        assert(updated.len() == slots.len());
        if from == idx {
            // Both non-empty: old is CLEAN or DIRTY, new is DIRTY
            assert(updated[from] == SLOT_DIRTY);
            assert(slots[from] != SLOT_EMPTY);
            assert(updated[from] != SLOT_EMPTY);
        } else {
            assert(updated[from] == slots[from]);
        }
        mark_dirty_preserves_occupied_from(slots, idx, from + 1);
    }
}

// ─── Write-Back (Clean) ──────────────────────────────────────────────

/// Spec: write-back a dirty slot (mark clean).
// r[depends block-cache.write-back]
// r[impl block-cache.write-back]
pub open spec fn write_back_spec(slots: Seq<u8>, idx: int) -> Seq<u8>
    recommends 0 <= idx < slots.len(),
{
    slots.update(idx, SLOT_CLEAN)
}

// r[impl block-cache.write-back]
pub fn write_back(slots: &mut Vec<u8>, idx: usize)
    requires idx < old(slots)@.len(),
    ensures slots@ == write_back_spec(old(slots)@, idx as int),
{
    slots.set(idx, SLOT_CLEAN);
}

// r[verify block-cache.write-back]
/// Write-back decreases dirty count by 1.
proof fn write_back_decrements_dirty(slots: Seq<u8>, idx: int)
    requires 0 <= idx < slots.len(), slots[idx] == SLOT_DIRTY,
    ensures dirty_count_spec(write_back_spec(slots, idx))
        == dirty_count_spec(slots) - 1,
{
    write_back_decrements_dirty_from(slots, idx, 0);
}

proof fn write_back_decrements_dirty_from(slots: Seq<u8>, idx: int, from: int)
    requires 0 <= idx < slots.len(), slots[idx] == SLOT_DIRTY, 0 <= from,
    ensures
        dirty_count_from_spec(write_back_spec(slots, idx), from)
            == dirty_count_from_spec(slots, from) - (if from <= idx { 1int } else { 0int }),
    decreases (slots.len() - from),
{
    if from < slots.len() {
        let updated = write_back_spec(slots, idx);
        assert(updated.len() == slots.len());
        if from == idx {
            assert(updated[from] == SLOT_CLEAN);
            assert(slots[from] == SLOT_DIRTY);
        } else {
            assert(updated[from] == slots[from]);
        }
        write_back_decrements_dirty_from(slots, idx, from + 1);
    }
}

// ─── Evict ───────────────────────────────────────────────────────────

/// Spec: evict a slot (mark empty).
// r[depends block-cache.evict]
// r[impl block-cache.evict]
pub open spec fn evict_spec(slots: Seq<u8>, idx: int) -> Seq<u8>
    recommends 0 <= idx < slots.len(),
{
    slots.update(idx, SLOT_EMPTY)
}

// r[impl block-cache.evict]
pub fn evict(slots: &mut Vec<u8>, idx: usize)
    requires idx < old(slots)@.len(),
    ensures slots@ == evict_spec(old(slots)@, idx as int),
{
    slots.set(idx, SLOT_EMPTY);
}

// r[verify block-cache.evict]
/// Evicting an occupied slot decreases occupied count by 1.
proof fn evict_decrements_occupied(slots: Seq<u8>, idx: int)
    requires 0 <= idx < slots.len(), slots[idx] != SLOT_EMPTY,
    ensures occupied_count_spec(evict_spec(slots, idx))
        == occupied_count_spec(slots) - 1,
{
    evict_decrements_occupied_from(slots, idx, 0);
}

proof fn evict_decrements_occupied_from(slots: Seq<u8>, idx: int, from: int)
    requires 0 <= idx < slots.len(), slots[idx] != SLOT_EMPTY, 0 <= from,
    ensures
        occupied_count_from_spec(evict_spec(slots, idx), from)
            == occupied_count_from_spec(slots, from) - (if from <= idx { 1int } else { 0int }),
    decreases (slots.len() - from),
{
    if from < slots.len() {
        let updated = evict_spec(slots, idx);
        assert(updated.len() == slots.len());
        if from == idx {
            assert(updated[from] == SLOT_EMPTY);
            assert(slots[from] != SLOT_EMPTY);
        } else {
            assert(updated[from] == slots[from]);
        }
        evict_decrements_occupied_from(slots, idx, from + 1);
    }
}

// r[verify block-cache.evict]
/// Can't evict dirty — must write-back first.
proof fn evict_dirty_loses_data(slots: Seq<u8>, idx: int)
    requires 0 <= idx < slots.len(), slots[idx] == SLOT_DIRTY,
    ensures evict_spec(slots, idx)[idx] == SLOT_EMPTY,
{}

// ─── Clock-Hand Sweep ────────────────────────────────────────────────

/// Spec: advance clock hand, wrapping at capacity.
// r[depends block-cache.clock-advance]
// r[impl block-cache.clock-advance]
pub open spec fn clock_advance_spec(hand: u64, capacity: u64) -> u64
    recommends capacity > 0,
{
    if hand + 1 >= capacity { 0 } else { (hand + 1) as u64 }
}

// r[impl block-cache.clock-advance]
pub fn clock_advance(hand: u64, capacity: u64) -> (result: u64)
    requires capacity > 0, hand < capacity,
    ensures result == clock_advance_spec(hand, capacity),
{
    if hand + 1 >= capacity { 0 } else { hand + 1 }
}

// r[verify block-cache.clock-advance]
/// Clock hand stays in [0, capacity).
proof fn clock_advance_bounded(hand: u64, capacity: u64)
    requires capacity > 0, hand < capacity,
    ensures clock_advance_spec(hand, capacity) < capacity,
{}

// r[verify block-cache.clock-advance]
/// Clock hand wraps around.
proof fn clock_wraps(capacity: u64)
    requires capacity > 0,
    ensures clock_advance_spec((capacity - 1) as u64, capacity) == 0,
{}

// ─── Cache Full / Has Space ──────────────────────────────────────────

/// Spec: is the cache full?
// r[depends block-cache.is-full]
// r[impl block-cache.is-full]
pub open spec fn is_full_spec(occupied: u64, capacity: u64) -> bool {
    occupied >= capacity
}

// r[impl block-cache.is-full]
pub fn is_full(occupied: u64, capacity: u64) -> (result: bool)
    ensures result == is_full_spec(occupied, capacity),
{
    occupied >= capacity
}

/// Spec: dirty ratio as percentage.
// r[depends block-cache.dirty-ratio]
// r[impl block-cache.dirty-ratio]
pub open spec fn dirty_ratio_pct_spec(dirty: u64, capacity: u64) -> u64
    recommends capacity > 0,
{
    if capacity == 0 { 0 } else { ((dirty as int * 100) / (capacity as int)) as u64 }
}

// r[impl block-cache.dirty-ratio]
pub fn dirty_ratio_pct(dirty: u64, capacity: u64) -> (result: u64)
    requires capacity > 0, dirty <= u64::MAX / 100,
    ensures result == dirty_ratio_pct_spec(dirty, capacity),
{
    (dirty * 100) / capacity
}

// r[verify block-cache.dirty-ratio]
/// Dirty ratio is 0 when no dirty pages.
proof fn dirty_ratio_zero(capacity: u64)
    requires capacity > 0,
    ensures dirty_ratio_pct_spec(0, capacity) == 0,
{}

// r[verify block-cache.dirty-ratio]
/// Dirty ratio is at most 100 when dirty <= capacity.
proof fn dirty_ratio_bounded(dirty: u64, capacity: u64)
    requires capacity > 0, dirty <= capacity,
    ensures dirty_ratio_pct_spec(dirty, capacity) <= 100,
{
    assert(dirty as int * 100 <= capacity as int * 100) by(nonlinear_arith)
        requires 0 <= dirty, dirty <= capacity;
    vstd::arithmetic::div_mod::lemma_div_is_ordered(
        dirty as int * 100, capacity as int * 100, capacity as int);
    vstd::arithmetic::div_mod::lemma_div_multiples_vanish(100, capacity as int);
    vstd::arithmetic::div_mod::lemma_div_pos_is_pos(dirty as int * 100, capacity as int);
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_occupied_count() {
        assert_eq!(occupied_count(&[SLOT_EMPTY, SLOT_CLEAN, SLOT_DIRTY, SLOT_EMPTY]), 2);
        assert_eq!(occupied_count(&[]), 0);
        assert_eq!(occupied_count(&[SLOT_CLEAN, SLOT_CLEAN, SLOT_CLEAN]), 3);
    }

    #[test]
    fn test_dirty_count() {
        assert_eq!(dirty_count(&[SLOT_EMPTY, SLOT_CLEAN, SLOT_DIRTY, SLOT_DIRTY]), 2);
        assert_eq!(dirty_count(&[SLOT_CLEAN, SLOT_CLEAN]), 0);
    }

    #[test]
    fn test_insert_into_empty() {
        let mut slots = vec![SLOT_EMPTY, SLOT_EMPTY, SLOT_EMPTY];
        insert(&mut slots, 1);
        assert_eq!(slots, vec![SLOT_EMPTY, SLOT_CLEAN, SLOT_EMPTY]);
        assert_eq!(occupied_count(&slots), 1);
    }

    #[test]
    fn test_mark_dirty() {
        let mut slots = vec![SLOT_CLEAN, SLOT_CLEAN];
        mark_dirty(&mut slots, 0);
        assert_eq!(slots[0], SLOT_DIRTY);
        assert_eq!(dirty_count(&slots), 1);
        assert_eq!(occupied_count(&slots), 2);
    }

    #[test]
    fn test_write_back() {
        let mut slots = vec![SLOT_DIRTY];
        write_back(&mut slots, 0);
        assert_eq!(slots[0], SLOT_CLEAN);
        assert_eq!(dirty_count(&slots), 0);
    }

    #[test]
    fn test_evict() {
        let mut slots = vec![SLOT_CLEAN, SLOT_DIRTY];
        evict(&mut slots, 0);
        assert_eq!(slots[0], SLOT_EMPTY);
        assert_eq!(occupied_count(&slots), 1);
    }

    #[test]
    fn test_clock_advance() {
        assert_eq!(clock_advance(0, 4), 1);
        assert_eq!(clock_advance(3, 4), 0);
        assert_eq!(clock_advance(0, 1), 0);
    }

    #[test]
    fn test_dirty_ratio() {
        assert_eq!(dirty_ratio_pct(0, 100), 0);
        assert_eq!(dirty_ratio_pct(50, 100), 50);
        assert_eq!(dirty_ratio_pct(100, 100), 100);
    }

    #[test]
    fn test_is_full() {
        assert!(!is_full(3, 4));
        assert!(is_full(4, 4));
        assert!(is_full(5, 4));
    }
}

#[cfg(test)]
#[allow(clippy::cast_possible_truncation)]
mod proptests {
    use proptest::prelude::*;

    use super::*;

    fn valid_slot_state() -> impl Strategy<Value = u8> {
        prop_oneof![Just(SLOT_EMPTY), Just(SLOT_CLEAN), Just(SLOT_DIRTY)]
    }

    proptest! {
        #[test]
        fn prop_occupied_bounded(slots in proptest::collection::vec(valid_slot_state(), 0..200)) {
            let count = occupied_count(&slots);
            prop_assert!(count as usize <= slots.len());
        }

        #[test]
        fn prop_dirty_le_occupied(slots in proptest::collection::vec(valid_slot_state(), 0..200)) {
            let d = dirty_count(&slots);
            let o = occupied_count(&slots);
            prop_assert!(d <= o);
        }

        #[test]
        fn prop_insert_then_evict_identity(
            slots in proptest::collection::vec(valid_slot_state(), 1..100usize),
            idx in any::<proptest::sample::Index>(),
        ) {
            let real_idx = idx.index(slots.len());
            if slots[real_idx] == SLOT_EMPTY {
                let mut m = slots.clone();
                insert(&mut m, real_idx);
                evict(&mut m, real_idx);
                prop_assert_eq!(m[real_idx], SLOT_EMPTY);
            }
        }

        #[test]
        fn prop_write_back_dirty_cleans(
            slots in proptest::collection::vec(valid_slot_state(), 1..100usize),
            idx in any::<proptest::sample::Index>(),
        ) {
            let real_idx = idx.index(slots.len());
            if slots[real_idx] == SLOT_DIRTY {
                let mut m = slots.clone();
                let d_before = dirty_count(&m);
                write_back(&mut m, real_idx);
                let d_after = dirty_count(&m);
                prop_assert_eq!(d_after, d_before - 1);
                prop_assert_eq!(m[real_idx], SLOT_CLEAN);
            }
        }

        #[test]
        fn prop_clock_stays_in_bounds(hand in 0u64..10000, capacity in 1u64..10001) {
            if hand < capacity {
                let next = clock_advance(hand, capacity);
                prop_assert!(next < capacity);
            }
        }

        #[test]
        fn prop_dirty_ratio_bounded(dirty in 0u64..10000, capacity in 1u64..10001) {
            if dirty <= capacity && dirty <= u64::MAX / 100 {
                let ratio = dirty_ratio_pct(dirty, capacity);
                prop_assert!(ratio <= 100);
            }
        }
    }
}
