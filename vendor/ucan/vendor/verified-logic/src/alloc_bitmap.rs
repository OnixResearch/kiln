//! Verified allocation bitmap with double-free detection.
//!
//! A typed allocation bitmap where each slot has a boolean state
//! (free/allocated) and a generation counter. Handles encode both
//! slot index and generation, so freeing a stale handle is detected.
//! Composes [`free_space_map`] (bitmap tracking) with [`slab`]
//! (generation counters) into a single structure.

// Verified bit-packing uses u32→u64 casts.
#![allow(clippy::cast_lossless)]
#![allow(clippy::result_unit_err)]

use vstd::prelude::*;

verus! {

// ─── Handle packing (slot + generation) ──────────────────────────────

// r[depends alloc-bitmap.handle-pack]
/// Spec: pack slot index (low 32 bits) and generation (high 32 bits).
pub open spec fn handle_pack_spec(slot: u32, gen: u32) -> u64 {
    (slot as u64) | ((gen as u64) << 32u64)
}

// r[impl alloc-bitmap.handle-pack]
/// Pack a slot index and generation into a single handle.
pub fn handle_pack(slot: u32, generation: u32) -> (result: u64)
    ensures result == handle_pack_spec(slot, generation)
{
    (slot as u64) | ((generation as u64) << 32u64)
}

// r[depends alloc-bitmap.handle-unpack-slot]
/// Spec: extract slot index from handle (low 32 bits).
pub open spec fn handle_unpack_slot_spec(handle: u64) -> u32 {
    (handle & 0xFFFF_FFFFu64) as u32
}

// r[impl alloc-bitmap.handle-unpack-slot]
/// Extract slot index from handle.
pub fn handle_unpack_slot(handle: u64) -> (result: u32)
    ensures result == handle_unpack_slot_spec(handle)
{
    (handle & 0xFFFF_FFFF) as u32
}

// r[depends alloc-bitmap.handle-unpack-gen]
/// Spec: extract generation from handle (high 32 bits).
pub open spec fn handle_unpack_gen_spec(handle: u64) -> u32 {
    ((handle >> 32u64) & 0xFFFF_FFFFu64) as u32
}

// r[impl alloc-bitmap.handle-unpack-gen]
/// Extract generation from handle.
pub fn handle_unpack_gen(handle: u64) -> (result: u32)
    ensures result == handle_unpack_gen_spec(handle)
{
    ((handle >> 32) & 0xFFFF_FFFF) as u32
}

// ─── Bitmap State ────────────────────────────────────────────────────

// r[depends alloc-bitmap.is-allocated]
/// Spec: slot is currently allocated.
pub open spec fn bitmap_is_allocated_spec(map: Seq<bool>, slot: int) -> bool
    recommends 0 <= slot < map.len()
{
    map[slot]
}

// r[impl alloc-bitmap.is-allocated]
/// Returns true if the slot is currently allocated.
pub fn bitmap_is_allocated(map: &[bool], slot: usize) -> (result: bool)
    requires 0 <= slot < map@.len()
    ensures result == bitmap_is_allocated_spec(map@, slot as int)
{
    map[slot]
}

// r[depends alloc-bitmap.generation]
/// Spec: current generation for a slot.
pub open spec fn bitmap_generation_spec(gens: Seq<u32>, slot: int) -> u32
    recommends 0 <= slot < gens.len()
{
    gens[slot]
}

// r[impl alloc-bitmap.generation]
/// Returns the current generation counter for a slot.
pub fn bitmap_generation(gens: &[u32], slot: usize) -> (result: u32)
    requires 0 <= slot < gens@.len()
    ensures result == bitmap_generation_spec(gens@, slot as int)
{
    gens[slot]
}

// r[impl alloc-bitmap.clone-bool-slice]
// r[verify alloc-bitmap.clone-bool-slice]
/// Helper: clone a bool slice into a Vec.
pub fn clone_bool_slice(src: &[bool]) -> (result: Vec<bool>)
    ensures result@ == src@
{
    let mut out: Vec<bool> = Vec::new();
    let mut i: usize = 0;
    while i < src.len()
        invariant
            0 <= i <= src@.len(),
            out@ == src@.subrange(0, i as int),
        decreases src@.len() - i,
    {
        out.push(src[i]);
        i = i + 1;
    }
    out
}

// r[impl alloc-bitmap.clone-u32-slice]
// r[verify alloc-bitmap.clone-u32-slice]
/// Helper: clone a u32 slice into a Vec.
pub fn clone_u32_slice(src: &[u32]) -> (result: Vec<u32>)
    ensures result@ == src@
{
    let mut out: Vec<u32> = Vec::new();
    let mut i: usize = 0;
    while i < src.len()
        invariant
            0 <= i <= src@.len(),
            out@ == src@.subrange(0, i as int),
        decreases src@.len() - i,
    {
        out.push(src[i]);
        i = i + 1;
    }
    out
}

// ─── Utilization ─────────────────────────────────────────────────────

/// Spec helper: count allocated (true) entries from position `from`.
pub open spec fn count_allocated_from_spec(map: Seq<bool>, from: int) -> int
    decreases (if map.len() >= from { map.len() - from } else { 0 }),
{
    if from < 0 || from >= map.len() {
        0
    } else {
        (if map[from] { 1int } else { 0int }) + count_allocated_from_spec(map, from + 1)
    }
}

// r[depends alloc-bitmap.utilization]
/// Spec: (used_count, total_count).
pub open spec fn bitmap_utilization_spec(map: Seq<bool>) -> (int, int) {
    (count_allocated_from_spec(map, 0), map.len() as int)
}

// r[impl alloc-bitmap.utilization]
/// Returns (allocated count, total slots).
pub fn bitmap_utilization(map: &[bool]) -> (result: (u64, u64))
    requires map@.len() <= u64::MAX as int
    ensures
        result.0 as int == bitmap_utilization_spec(map@).0,
        result.1 as int == bitmap_utilization_spec(map@).1,
{
    let mut used: u64 = 0;
    let mut i: usize = 0;
    while i < map.len()
        invariant
            0 <= i <= map@.len(),
            map@.len() <= u64::MAX as int,
            used as int == count_allocated_from_spec(map@, 0)
                - count_allocated_from_spec(map@, i as int),
            used as int <= i as int,
        decreases map@.len() - i,
    {
        if map[i] {
            used = used + 1;
        }
        i = i + 1;
    }
    (used, map.len() as u64)
}

// ─── Allocate ────────────────────────────────────────────────────────

/// Spec helper: index of first free (false) slot from position `from`.
pub open spec fn first_free_from_spec(map: Seq<bool>, from: int) -> int
    decreases (if map.len() >= from { map.len() - from } else { 0 }),
{
    if from < 0 || from >= map.len() {
        -1
    } else if !map[from] {
        from
    } else {
        first_free_from_spec(map, from + 1)
    }
}

// r[depends alloc-bitmap.alloc]
/// Spec: allocate first free slot. Returns (slot, handle, new_map, new_gens)
/// or (-1, 0, map, gens) if full.
pub open spec fn bitmap_alloc_spec(
    map: Seq<bool>, gens: Seq<u32>,
) -> (int, u64, Seq<bool>, Seq<u32>)
    recommends map.len() == gens.len()
{
    let slot = first_free_from_spec(map, 0);
    if slot < 0 {
        (-1, 0, map, gens)
    } else {
        let new_gen = if gens[slot] < u32::MAX as u32 { (gens[slot] + 1) as u32 } else { gens[slot] };
        let handle = handle_pack_spec(slot as u32, new_gen);
        let new_map = map.update(slot, true);
        let new_gens = gens.update(slot, new_gen);
        (slot, handle, new_map, new_gens)
    }
}

// r[impl alloc-bitmap.alloc]
/// Allocate the first free slot. Returns None if full.
#[allow(clippy::cast_possible_truncation)]
pub fn bitmap_alloc(
    map: &[bool], gens: &[u32],
) -> (result: Option<(usize, u64, Vec<bool>, Vec<u32>)>)
    requires map@.len() == gens@.len(), map@.len() <= u32::MAX as int
    ensures ({
        let spec_result = bitmap_alloc_spec(map@, gens@);
        match result {
            None => spec_result.0 < 0,
            Some((slot, handle, new_map, new_gens)) => {
                &&& spec_result.0 == slot as int
                &&& spec_result.1 == handle
                &&& new_map@ == spec_result.2
                &&& new_gens@ == spec_result.3
            }
        }
    })
{
    let mut i: usize = 0;
    while i < map.len()
        invariant
            0 <= i <= map@.len(),
            map@.len() == gens@.len(),
            map@.len() <= u32::MAX as int,
            forall|j: int| 0 <= j < i as int ==> map@[j],
        decreases map@.len() - i,
    {
        if !map[i] {
            let new_gen = if gens[i] < u32::MAX { gens[i] + 1 } else { gens[i] };
            let slot = i as u32;
            let handle = handle_pack(slot, new_gen);
            let mut new_map = clone_bool_slice(map);
            let mut new_gens = clone_u32_slice(gens);
            new_map.set(i, true);
            new_gens.set(i, new_gen);
            proof {
                first_free_at_prefix_boundary(map@, i as int);
                assert(bitmap_alloc_spec(map@, gens@).0 == i as int);
                assert(bitmap_alloc_spec(map@, gens@).1 == handle);
                assert(new_map@ == bitmap_alloc_spec(map@, gens@).2);
                assert(new_gens@ == bitmap_alloc_spec(map@, gens@).3);
            }
            return Some((i, handle, new_map, new_gens));
        }
        i = i + 1;
    }
    proof {
        alloc_full_returns_none(map@, gens@);
    }
    None
}

// ─── Free ────────────────────────────────────────────────────────────

// r[depends alloc-bitmap.free]
/// Spec: free a slot by handle. Returns new state or error flag.
/// Error (false) if: slot out of range, not allocated, or generation mismatch.
pub open spec fn bitmap_free_spec(
    map: Seq<bool>, gens: Seq<u32>, handle: u64,
) -> (bool, Seq<bool>, Seq<u32>)
    recommends map.len() == gens.len()
{
    let slot = handle_unpack_slot_spec(handle) as int;
    let gen = handle_unpack_gen_spec(handle);
    if slot < 0 || slot >= map.len() || !map[slot] || gens[slot] != gen {
        (false, map, gens)
    } else {
        (true, map.update(slot, false), gens)
    }
}

// r[impl alloc-bitmap.free]
/// Free a slot by handle. Returns Ok(new_map, new_gens) or Err(()) for
/// double-free, stale handle, or out-of-range.
pub fn bitmap_free(
    map: &[bool], gens: &[u32], handle: u64,
) -> (result: Result<(Vec<bool>, Vec<u32>), ()>)
    requires map@.len() == gens@.len(), map@.len() <= u32::MAX as int
    ensures ({
        let spec_result = bitmap_free_spec(map@, gens@, handle);
        match result {
            Ok((new_map, new_gens)) => {
                &&& spec_result.0
                &&& new_map@ == spec_result.1
                &&& new_gens@ == spec_result.2
            }
            Err(()) => !spec_result.0,
        }
    })
{
    let slot = handle_unpack_slot(handle) as usize;
    let generation = handle_unpack_gen(handle);
    if slot >= map.len() || !map[slot] || gens[slot] != generation {
        return Err(());
    }
    let mut new_map = clone_bool_slice(map);
    let new_gens = clone_u32_slice(gens);
    new_map.set(slot, false);
    Ok((new_map, new_gens))
}

// ─── Proofs ──────────────────────────────────────────────────────────

// r[verify alloc-bitmap.handle-pack]
/// Handle pack/unpack roundtrip for slot.
proof fn handle_roundtrip_slot(slot: u32, gen: u32)
    ensures handle_unpack_slot_spec(handle_pack_spec(slot, gen)) == slot
{
    assert(((slot as u64 | ((gen as u64) << 32u64)) & 0xFFFF_FFFFu64) as u32 == slot)
        by(bit_vector);
}

// r[verify alloc-bitmap.handle-pack]
/// Handle pack/unpack roundtrip for generation.
proof fn handle_roundtrip_gen(slot: u32, gen: u32)
    ensures handle_unpack_gen_spec(handle_pack_spec(slot, gen)) == gen
{
    assert((((slot as u64 | ((gen as u64) << 32u64)) >> 32u64) & 0xFFFF_FFFFu64) as u32 == gen)
        by(bit_vector);
}

// r[verify alloc-bitmap.handle-pack]
/// Different (slot, gen) pairs produce different handles.
proof fn handle_injective(s1: u32, g1: u32, s2: u32, g2: u32)
    requires s1 != s2 || g1 != g2
    ensures handle_pack_spec(s1, g1) != handle_pack_spec(s2, g2)
{
    if s1 != s2 {
        assert(s1 != s2 ==>
            ((s1 as u64 | ((g1 as u64) << 32u64)) & 0xFFFF_FFFFu64)
            != ((s2 as u64 | ((g2 as u64) << 32u64)) & 0xFFFF_FFFFu64))
            by(bit_vector);
    } else {
        // s1 == s2, g1 != g2
        assert(g1 != g2 ==>
            (((s1 as u64 | ((g1 as u64) << 32u64)) >> 32u64) & 0xFFFF_FFFFu64)
            != (((s2 as u64 | ((g2 as u64) << 32u64)) >> 32u64) & 0xFFFF_FFFFu64))
            by(bit_vector);
    }
}

/// Helper: count_allocated_from is non-negative.
pub proof fn count_allocated_nonneg(map: Seq<bool>, from: int)
    ensures count_allocated_from_spec(map, from) >= 0
    decreases (if map.len() >= from { map.len() - from } else { 0 }),
{
    if from >= 0 && from < map.len() {
        count_allocated_nonneg(map, from + 1);
    }
}

/// Helper: count_allocated_from is bounded by remaining length.
pub proof fn count_allocated_bounded(map: Seq<bool>, from: int)
    requires 0 <= from <= map.len()
    ensures count_allocated_from_spec(map, from) <= map.len() - from
    decreases map.len() - from,
{
    if from < map.len() {
        count_allocated_bounded(map, from + 1);
    }
}

// r[verify alloc-bitmap.utilization]
/// used + free == total.
proof fn utilization_conservation(map: Seq<bool>)
    ensures ({
        let (used, total) = bitmap_utilization_spec(map);
        used >= 0 && used <= total
    })
{
    count_allocated_nonneg(map, 0);
    if map.len() > 0 {
        count_allocated_bounded(map, 0);
    }
}

// r[verify alloc-bitmap.alloc]
/// Alloc when full returns slot < 0 (None).
proof fn alloc_full_returns_none(map: Seq<bool>, gens: Seq<u32>)
    requires
        map.len() == gens.len(),
        forall|i: int| #![trigger map[i]] 0 <= i < map.len() ==> map[i],
    ensures bitmap_alloc_spec(map, gens).0 < 0
{
    // first_free_from_spec returns -1 when all slots are allocated
    alloc_full_helper(map, 0);
}

proof fn alloc_full_helper(map: Seq<bool>, from: int)
    requires
        0 <= from,
        forall|i: int| #![trigger map[i]] 0 <= i < map.len() ==> map[i],
    ensures first_free_from_spec(map, from) < 0
    decreases (if map.len() >= from { map.len() - from } else { 0 }),
{
    if from >= 0 && from < map.len() {
        assert(map[from]); // all allocated
        alloc_full_helper(map, from + 1);
    }
}

/// Helper: if every slot in `[from, target)` is allocated and `target` is free,
/// then `target` is the first free slot from `from`.
pub proof fn first_free_from_is_target(map: Seq<bool>, from: int, target: int)
    requires
        0 <= from <= target < map.len(),
        !map[target],
        forall|j: int| from <= j < target ==> map[j],
    ensures first_free_from_spec(map, from) == target
    decreases target - from,
{
    if from == target {
        assert(first_free_from_spec(map, from) == from);
    } else {
        assert(map[from]);
        first_free_from_is_target(map, from + 1, target);
    }
}

/// Helper: if every earlier slot is allocated and slot `i` is free, then it is the first free slot.
pub proof fn first_free_at_prefix_boundary(map: Seq<bool>, i: int)
    requires
        0 <= i < map.len(),
        !map[i],
        forall|j: int| 0 <= j < i ==> map[j],
    ensures first_free_from_spec(map, 0) == i
{
    first_free_from_is_target(map, 0, i);
}

/// Helper: a found free slot is in bounds.
pub proof fn first_free_in_bounds(map: Seq<bool>, from: int)
    requires
        0 <= from <= map.len(),
        0 <= first_free_from_spec(map, from),
    ensures
        from <= first_free_from_spec(map, from),
        first_free_from_spec(map, from) < map.len(),
    decreases map.len() - from,
{
    if from < map.len() {
        if !map[from] {
            assert(first_free_from_spec(map, from) == from);
        } else {
            assert(first_free_from_spec(map, from) == first_free_from_spec(map, from + 1));
            first_free_in_bounds(map, from + 1);
        }
    }
}

// r[verify alloc-bitmap.free]
/// Double-free: free(alloc_result) twice fails on second attempt.
proof fn double_free_detected(map: Seq<bool>, gens: Seq<u32>)
    requires
        map.len() == gens.len(),
        map.len() > 0,
        !map[0],
        gens[0] < u32::MAX as u32,
    ensures ({
        let alloc_result = bitmap_alloc_spec(map, gens);
        let handle = alloc_result.1;
        let free1 = bitmap_free_spec(alloc_result.2, alloc_result.3, handle);
        // First free succeeds
        &&& free1.0
        // Second free with same handle fails (slot is now free)
        &&& !bitmap_free_spec(free1.1, free1.2, handle).0
    })
{
    // Help Z3 unfold first_free_from_spec
    assert(first_free_from_spec(map, 0) == 0);

    let alloc_result = bitmap_alloc_spec(map, gens);
    // alloc_result: slot=0, handle=pack(0, gens[0]+1), new_map=map.update(0,true), new_gens=gens.update(0,gens[0]+1)
    assert(alloc_result.0 == 0);

    let new_gen: u32 = (gens[0] + 1) as u32;
    let handle = alloc_result.1;

    // Unpack the handle to help Z3
    handle_roundtrip_slot(0u32, new_gen);
    handle_roundtrip_gen(0u32, new_gen);

    let new_map = alloc_result.2;
    let new_gens = alloc_result.3;
    assert(new_map[0]);

    // First free succeeds
    let free1 = bitmap_free_spec(new_map, new_gens, handle);
    assert(free1.0);

    // Second free: map is now updated to false at slot 0
    let map2 = free1.1;
    assert(!map2[0]);
    // So bitmap_free_spec checks !map2[0] → slot not allocated → fails
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify alloc-bitmap.handle-pack]
    // r[verify alloc-bitmap.handle-unpack-slot]
    // r[verify alloc-bitmap.handle-unpack-gen]
    // r[verify alloc-bitmap.generation]
    #[test]
    fn test_handle_roundtrip() {
        for slot in [0u32, 1, 100, u32::MAX] {
            for g in [0u32, 1, 42, u32::MAX] {
                let h = handle_pack(slot, g);
                assert_eq!(handle_unpack_slot(h), slot);
                assert_eq!(handle_unpack_gen(h), g);
            }
        }
    }

    // r[verify alloc-bitmap.is-allocated]
    #[test]
    fn test_is_allocated() {
        assert!(!bitmap_is_allocated(&[false, false, false], 0));
        assert!(bitmap_is_allocated(&[true, false, false], 0));
    }

    // r[verify alloc-bitmap.alloc]
    #[test]
    fn test_alloc_basic() {
        let map = vec![false, false, false];
        let gens = vec![0u32, 0, 0];
        let result = bitmap_alloc(&map, &gens);
        assert!(result.is_some());
        let (slot, handle, new_map, new_gens) = result.unwrap();
        assert_eq!(slot, 0);
        assert!(new_map[0]);
        assert_eq!(new_gens[0], 1);
        assert_eq!(handle_unpack_slot(handle), 0);
        assert_eq!(handle_unpack_gen(handle), 1);
    }

    // r[verify alloc-bitmap.alloc]
    #[test]
    fn test_alloc_full() {
        let map = vec![true, true, true];
        let gens = vec![1u32, 1, 1];
        let result = bitmap_alloc(&map, &gens);
        assert!(result.is_none());
    }

    // r[verify alloc-bitmap.free]
    #[test]
    fn test_free_basic() {
        let map = vec![false, false];
        let gens = vec![0u32, 0];
        let (slot, handle, map2, gens2) = bitmap_alloc(&map, &gens).unwrap();
        assert_eq!(slot, 0);
        let result = bitmap_free(&map2, &gens2, handle);
        assert!(result.is_ok());
        let (map3, _gens3) = result.unwrap();
        assert!(!map3[0]); // freed
    }

    // r[verify alloc-bitmap.free]
    #[test]
    fn test_double_free() {
        let map = vec![false, false];
        let gens = vec![0u32, 0];
        let (_slot, handle, map2, gens2) = bitmap_alloc(&map, &gens).unwrap();
        let (map3, gens3) = bitmap_free(&map2, &gens2, handle).unwrap();
        // Second free with same handle should fail
        let result = bitmap_free(&map3, &gens3, handle);
        assert!(result.is_err());
    }

    // r[verify alloc-bitmap.free]
    #[test]
    fn test_stale_handle() {
        let map = vec![false];
        let gens = vec![0u32];
        // Alloc → free → alloc again → try to free with first handle
        let (_s, h1, m1, g1) = bitmap_alloc(&map, &gens).unwrap();
        let (m2, g2) = bitmap_free(&m1, &g1, h1).unwrap();
        let (_s2, _h2, m3, g3) = bitmap_alloc(&m2, &g2).unwrap();
        // h1 has old generation
        let result = bitmap_free(&m3, &g3, h1);
        assert!(result.is_err());
    }

    // r[verify alloc-bitmap.utilization]
    #[test]
    fn test_utilization() {
        let (used, total) = bitmap_utilization(&[true, false, true, false]);
        assert_eq!(used, 2);
        assert_eq!(total, 4);
    }

    proptest! {
        // r[verify alloc-bitmap.handle-pack]
        #[test]
        fn prop_handle_roundtrip(slot: u32, generation: u32) {
            let h = handle_pack(slot, generation);
            prop_assert_eq!(handle_unpack_slot(h), slot);
            prop_assert_eq!(handle_unpack_gen(h), generation);
        }

        // r[verify alloc-bitmap.utilization]
        #[test]
        fn prop_utilization_conservation(
            map in proptest::collection::vec(proptest::bool::ANY, 1..100),
        ) {
            let (used, total) = bitmap_utilization(&map);
            let free = map.iter().filter(|&&b| !b).count() as u64;
            prop_assert_eq!(used + free, total);
        }

        // r[verify alloc-bitmap.alloc]
        #[test]
        fn prop_alloc_free_cycle(size in 1usize..50) {
            let map = vec![false; size];
            let gens = vec![0u32; size];
            if let Some((slot, handle, map2, gens2)) = bitmap_alloc(&map, &gens) {
                prop_assert!(map2[slot]);
                let result = bitmap_free(&map2, &gens2, handle);
                prop_assert!(result.is_ok());
                let (map3, _) = result.unwrap();
                prop_assert!(!map3[slot]);
            }
        }
    }
}
