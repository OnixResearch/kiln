//! Verified fixed-size object pool with bounded capacity.
//!
//! A slab allocator for one object size. Combines [`slab`] (slot math)
//! with [`alloc_bitmap`] (lifecycle tracking) and proves the composition
//! maintains both modules' invariants. The key cross-module property:
//! allocated slot addresses fall within the pool region.

#![allow(clippy::result_unit_err, clippy::type_complexity)]

use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::alloc_bitmap::bitmap_alloc_spec;
#[cfg(verus_keep_ghost)]
use crate::alloc_bitmap::bitmap_free_spec;
#[cfg(verus_keep_ghost)]
use crate::alloc_bitmap::bitmap_is_allocated_spec;
#[cfg(verus_keep_ghost)]
use crate::alloc_bitmap::bitmap_utilization_spec;
#[cfg(verus_keep_ghost)]
use crate::alloc_bitmap::count_allocated_from_spec;
#[cfg(verus_keep_ghost)]
use crate::slab::slot_address_spec;

verus! {

// ─── Pool Alloc ──────────────────────────────────────────────────────

// r[depends pool.alloc]
/// Spec: allocate one slot from the pool.
/// Returns (slot, address, handle) or None if full.
/// address = base + slot * obj_size.
pub open spec fn pool_alloc_spec(
    map: Seq<bool>, gens: Seq<u32>, base: u64, obj_size: u64,
) -> Option<(int, u64, u64)>
    recommends
        map.len() == gens.len(),
        obj_size > 0,
{
    let alloc_result = bitmap_alloc_spec(map, gens);
    if alloc_result.0 < 0 {
        None
    } else {
        let slot = alloc_result.0;
        let handle = alloc_result.1;
        let addr = slot_address_spec(slot as u64, base, obj_size);
        Some((slot, addr, handle))
    }
}

// r[impl pool.alloc]
/// Allocate one slot. Returns (slot, address, handle, new_map, new_gens)
/// or None if the pool is full.
pub fn pool_alloc(
    map: &[bool], gens: &[u32], base: u64, obj_size: u64,
) -> (result: Option<(usize, u64, u64, Vec<bool>, Vec<u32>)>)
    requires
        map@.len() == gens@.len(),
        map@.len() <= u32::MAX as int,
        obj_size > 0,
        base as int + map@.len() * obj_size as int <= u64::MAX as int,
    ensures ({
        match result {
            None => pool_alloc_spec(map@, gens@, base, obj_size) == None::<(int, u64, u64)>,
            Some((slot, addr, handle, new_map, new_gens)) => {
                let spec_result = pool_alloc_spec(map@, gens@, base, obj_size);
                match spec_result {
                    Some((ss, sa, sh)) => {
                        &&& ss == slot as int
                        &&& sa == addr
                        &&& sh == handle
                    }
                    None => false,
                }
            }
        }
    })
{
    use crate::alloc_bitmap::bitmap_alloc;
    use crate::slab::slot_address;

    let alloc_result = bitmap_alloc(map, gens);
    match alloc_result {
        None => None,
        Some((slot, handle, new_map, new_gens)) => {
            proof {
                assert(slot as int == bitmap_alloc_spec(map@, gens@).0);
                assert(0 <= bitmap_alloc_spec(map@, gens@).0);
                assert(bitmap_alloc_spec(map@, gens@).0 == crate::alloc_bitmap::first_free_from_spec(map@, 0));
                crate::alloc_bitmap::first_free_in_bounds(map@, 0);
                assert(0 <= (slot as int));
                assert((slot as int) < map@.len());
                assert(base as int + (slot as int) * obj_size as int <= u64::MAX as int) by (nonlinear_arith)
                    requires
                        0 <= (slot as int),
                        (slot as int) < map.len() as int,
                        obj_size > 0,
                        base as int + map.len() as int * obj_size as int <= u64::MAX as int,
                ;
                assert(pool_alloc_spec(map@, gens@, base, obj_size)
                    == Some((slot as int, slot_address_spec(slot as u64, base, obj_size), handle)));
            }
            let addr = slot_address(slot as u64, base, obj_size);
            Some((slot, addr, handle, new_map, new_gens))
        }
    }
}

// ─── Pool Free ───────────────────────────────────────────────────────

// r[depends pool.free]
/// Spec: free a slot by handle. Returns success flag.
pub open spec fn pool_free_spec(
    map: Seq<bool>, gens: Seq<u32>, handle: u64,
) -> bool
    recommends map.len() == gens.len()
{
    bitmap_free_spec(map, gens, handle).0
}

// r[impl pool.free]
/// Free a pool slot by handle. Returns Ok(new_map, new_gens) or Err.
pub fn pool_free(
    map: &[bool], gens: &[u32], handle: u64,
) -> (result: Result<(Vec<bool>, Vec<u32>), ()>)
    requires map@.len() == gens@.len(), map@.len() <= u32::MAX as int
    ensures ({
        match result {
            Ok(_) => pool_free_spec(map@, gens@, handle),
            Err(()) => !pool_free_spec(map@, gens@, handle),
        }
    })
{
    let free_result = crate::alloc_bitmap::bitmap_free(map, gens, handle);
    proof {
        assert(pool_free_spec(map@, gens@, handle) == bitmap_free_spec(map@, gens@, handle).0);
    }
    free_result
}

// ─── Pool Is Full ────────────────────────────────────────────────────

// r[depends pool.is-full]
/// Spec: pool is at capacity (all slots allocated).
pub open spec fn pool_is_full_spec(map: Seq<bool>) -> bool {
    forall|i: int| #![trigger map[i]] 0 <= i < map.len() ==> map[i]
}

// r[impl pool.is-full]
/// Returns true if every slot is allocated.
pub fn pool_is_full(map: &[bool]) -> (result: bool)
    ensures result == pool_is_full_spec(map@)
{
    let mut i: usize = 0;
    while i < map.len()
        invariant
            0 <= i <= map@.len(),
            forall|j: int| #![trigger map@[j]] 0 <= j < i as int ==> map@[j],
        decreases map@.len() - i,
    {
        if !map[i] {
            return false;
        }
        i = i + 1;
    }
    true
}

// ─── Pool Count ──────────────────────────────────────────────────────

// r[depends pool.count]
/// Spec: (allocated_count, free_count).
pub open spec fn pool_count_spec(map: Seq<bool>) -> (int, int) {
    let (used, total) = bitmap_utilization_spec(map);
    (used, total - used)
}

// r[impl pool.count]
/// Returns (allocated, free) counts.
pub fn pool_count(map: &[bool]) -> (result: (u64, u64))
    requires map@.len() <= u64::MAX as int
    ensures
        result.0 as int == pool_count_spec(map@).0,
        result.0 + result.1 == map.len() as u64,
{
    use crate::alloc_bitmap::bitmap_utilization;
    let (used, total) = bitmap_utilization(map);
    proof {
        crate::alloc_bitmap::count_allocated_nonneg(map@, 0);
        crate::alloc_bitmap::count_allocated_bounded(map@, 0);
    }
    (used, total - used)
}

// ─── Proofs ──────────────────────────────────────────────────────────

// r[verify pool.is-full]
/// Empty pool (all free) is not full.
proof fn empty_not_full(n: int)
    requires n > 0
    ensures ({
        let map = Seq::new(n as nat, |_i| false);
        !pool_is_full_spec(map)
    })
{
    let map = Seq::new(n as nat, |_i| false);
    assert(!map[0]);
}

// r[verify pool.count]
/// Alloc/free cycle conserves total count.
proof fn alloc_free_conserves(map: Seq<bool>)
    ensures ({
        let (allocated, free) = pool_count_spec(map);
        allocated + free == map.len()
    })
{}

// r[verify pool.is-full]
/// Full pool means zero free slots.
proof fn full_means_zero_free(map: Seq<bool>)
    requires pool_is_full_spec(map)
    ensures pool_count_spec(map).1 == 0
{
    // When all slots are allocated, count_allocated = map.len, so free = 0.
    // Need to show count_allocated_from_spec(map, 0) == map.len.
    full_count_helper(map, 0);
}

proof fn full_count_helper(map: Seq<bool>, from: int)
    requires
        0 <= from,
        forall|i: int| #![trigger map[i]] 0 <= i < map.len() ==> map[i],
    ensures count_allocated_from_spec(map, from)
        == (if from >= map.len() { 0int } else { map.len() - from })
    decreases (if map.len() >= from { map.len() - from } else { 0 }),
{
    if from >= 0 && from < map.len() {
        assert(map[from]); // all allocated
        full_count_helper(map, from + 1);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify pool.alloc]
    #[test]
    fn test_pool_alloc() {
        let map = vec![false, false, false];
        let gens = vec![0u32; 3];
        let result = pool_alloc(&map, &gens, 1000, 64);
        assert!(result.is_some());
        let (slot, addr, _handle, new_map, _new_gens) = result.unwrap();
        assert_eq!(slot, 0);
        assert_eq!(addr, 1000); // base + 0 * 64
        assert!(new_map[0]);
    }

    // r[verify pool.free]
    #[test]
    fn test_pool_free() {
        let map = vec![false, false];
        let gens = vec![0u32; 2];
        let (_slot, _addr, handle, map2, gens2) = pool_alloc(&map, &gens, 0, 32).unwrap();
        let result = pool_free(&map2, &gens2, handle);
        assert!(result.is_ok());
    }

    // r[verify pool.is-full]
    #[test]
    fn test_pool_is_full() {
        assert!(!pool_is_full(&[false, false]));
        assert!(!pool_is_full(&[true, false]));
        assert!(pool_is_full(&[true, true]));
    }

    // r[verify pool.count]
    #[test]
    fn test_pool_count() {
        let (alloc, free) = pool_count(&[true, false, true]);
        assert_eq!(alloc, 2);
        assert_eq!(free, 1);
    }

    // r[verify pool.free]
    #[test]
    fn test_pool_double_free() {
        let map = vec![false];
        let gens = vec![0u32];
        let (_, _, handle, m2, g2) = pool_alloc(&map, &gens, 0, 16).unwrap();
        let (m3, g3) = pool_free(&m2, &g2, handle).unwrap();
        // Double free should fail
        assert!(pool_free(&m3, &g3, handle).is_err());
    }

    proptest! {
        // r[verify pool.count]
        #[test]
        fn prop_count_conservation(
            map in proptest::collection::vec(proptest::bool::ANY, 1..50),
        ) {
            let (alloc, free) = pool_count(&map);
            prop_assert_eq!(alloc + free, map.len() as u64);
        }

        // r[verify pool.alloc]
        #[test]
        fn prop_alloc_address_aligned(
            cap in 1usize..20,
            obj_size in 1u64..256,
        ) {
            let map = vec![false; cap];
            let gens = vec![0u32; cap];
            let base = 0u64;
            if (cap as u128) * (obj_size as u128) <= u64::MAX as u128 {
                if let Some((slot, addr, _, _, _)) = pool_alloc(&map, &gens, base, obj_size) {
                    prop_assert_eq!(addr, base + slot as u64 * obj_size);
                    prop_assert_eq!(addr % obj_size, 0);
                }
            }
        }
    }
}
