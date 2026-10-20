//! Verified free-space bitmap tracking for block storage.
//!
//! A free-space map is a sequence of booleans: `true` = free, `false` = allocated.
//! Operations: allocate (mark used), free (mark available), count free blocks,
//! find first free, and scan for contiguous free runs. Proofs guarantee
//! conservation (alloc+free = identity), count bounds, and first-free correctness.
//! Composes with [`bitmap`] for bitwise ops and [`buddy`] for power-of-two blocks.

use vstd::prelude::*;

verus! {

// ─── Free Count ──────────────────────────────────────────────────────

/// Spec: count free (true) entries in map[from..].
// r[depends free-space-map.free-count]
// r[impl free-space-map.free-count]
pub open spec fn free_count_from_spec(map: Seq<bool>, from: int) -> int
    decreases (map.len() - from),
{
    if from < 0 || from >= map.len() {
        0
    } else {
        (if map[from] { 1int } else { 0int }) + free_count_from_spec(map, from + 1)
    }
}

/// Spec: total free count.
// r[depends free-space-map.free-count]
pub open spec fn free_count_spec(map: Seq<bool>) -> int {
    free_count_from_spec(map, 0)
}

// r[impl free-space-map.free-count]
/// Count free blocks in the map.
pub fn free_count(map: &[bool]) -> (result: u64)
    requires map@.len() <= u64::MAX as int,
    ensures result as int == free_count_spec(map@),
{
    let mut count: u64 = 0;
    let mut i: usize = 0;

    while i < map.len()
        invariant
            0 <= i <= map@.len(),
            map@.len() <= u64::MAX as int,
            count as int == free_count_spec(map@) - free_count_from_spec(map@, i as int),
            count as int <= i as int,
        decreases map@.len() - i,
    {
        if map[i] {
            count = count + 1;
        }
        i = i + 1;
    }
    count
}

// r[verify free-space-map.free-count]
/// Free count is non-negative.
proof fn free_count_nonneg(map: Seq<bool>)
    ensures free_count_spec(map) >= 0,
{
    free_count_from_nonneg(map, 0);
}

proof fn free_count_from_nonneg(map: Seq<bool>, from: int)
    ensures free_count_from_spec(map, from) >= 0,
    decreases (map.len() - from),
{
    if from >= 0 && from < map.len() {
        free_count_from_nonneg(map, from + 1);
    }
}

// r[verify free-space-map.free-count]
/// Free count is at most the map length.
proof fn free_count_bounded(map: Seq<bool>)
    ensures free_count_spec(map) <= map.len(),
{
    free_count_from_bounded(map, 0);
}

proof fn free_count_from_bounded(map: Seq<bool>, from: int)
    requires 0 <= from <= map.len(),
    ensures free_count_from_spec(map, from) <= map.len() - from,
    decreases (map.len() - from),
{
    if from < map.len() {
        free_count_from_bounded(map, from + 1);
    }
}

// ─── Allocate (Mark Used) ────────────────────────────────────────────

/// Spec: mark block at `idx` as allocated (false).
// r[depends free-space-map.allocate]
// r[impl free-space-map.allocate]
pub open spec fn allocate_spec(map: Seq<bool>, idx: int) -> Seq<bool>
    recommends 0 <= idx < map.len(),
{
    map.update(idx, false)
}

// r[impl free-space-map.allocate]
pub fn allocate(map: &mut Vec<bool>, idx: usize)
    requires
        idx < old(map)@.len(),
    ensures
        map@ == allocate_spec(old(map)@, idx as int),
{
    map.set(idx, false);
}

// r[verify free-space-map.allocate]
/// Allocating a free block decreases free count by 1.
proof fn allocate_free_decrements(map: Seq<bool>, idx: int)
    requires 0 <= idx < map.len(), map[idx] == true,
    ensures free_count_spec(allocate_spec(map, idx)) == free_count_spec(map) - 1,
{
    allocate_free_decrements_from(map, idx, 0);
}

proof fn allocate_free_decrements_from(map: Seq<bool>, idx: int, from: int)
    requires 0 <= idx < map.len(), map[idx] == true, 0 <= from,
    ensures
        free_count_from_spec(allocate_spec(map, idx), from)
            == free_count_from_spec(map, from) - (if from <= idx { 1int } else { 0int }),
    decreases (map.len() - from),
{
    if from < map.len() {
        let updated = allocate_spec(map, idx);
        assert(updated.len() == map.len());
        if from == idx {
            assert(updated[from] == false);
            assert(map[from] == true);
        } else {
            assert(updated[from] == map[from]);
        }
        allocate_free_decrements_from(map, idx, from + 1);
    }
}

// r[verify free-space-map.allocate]
/// Allocating an already-used block doesn't change free count.
proof fn allocate_used_no_change(map: Seq<bool>, idx: int)
    requires 0 <= idx < map.len(), map[idx] == false,
    ensures free_count_spec(allocate_spec(map, idx)) == free_count_spec(map),
{
    allocate_used_no_change_from(map, idx, 0);
}

proof fn allocate_used_no_change_from(map: Seq<bool>, idx: int, from: int)
    requires 0 <= idx < map.len(), map[idx] == false, 0 <= from,
    ensures
        free_count_from_spec(allocate_spec(map, idx), from)
            == free_count_from_spec(map, from),
    decreases (map.len() - from),
{
    if from < map.len() {
        let updated = allocate_spec(map, idx);
        assert(updated.len() == map.len());
        if from == idx {
            assert(updated[from] == false);
            assert(map[from] == false);
        } else {
            assert(updated[from] == map[from]);
        }
        allocate_used_no_change_from(map, idx, from + 1);
    }
}

// ─── Free (Mark Available) ───────────────────────────────────────────

/// Spec: mark block at `idx` as free (true).
// r[depends free-space-map.free-block]
// r[impl free-space-map.free-block]
pub open spec fn free_block_spec(map: Seq<bool>, idx: int) -> Seq<bool>
    recommends 0 <= idx < map.len(),
{
    map.update(idx, true)
}

// r[impl free-space-map.free-block]
pub fn free_block(map: &mut Vec<bool>, idx: usize)
    requires idx < old(map)@.len(),
    ensures map@ == free_block_spec(old(map)@, idx as int),
{
    map.set(idx, true);
}

// r[verify free-space-map.free-block]
/// Freeing an allocated block increases free count by 1.
proof fn free_allocated_increments(map: Seq<bool>, idx: int)
    requires 0 <= idx < map.len(), map[idx] == false,
    ensures free_count_spec(free_block_spec(map, idx)) == free_count_spec(map) + 1,
{
    free_allocated_increments_from(map, idx, 0);
}

proof fn free_allocated_increments_from(map: Seq<bool>, idx: int, from: int)
    requires 0 <= idx < map.len(), map[idx] == false, 0 <= from,
    ensures
        free_count_from_spec(free_block_spec(map, idx), from)
            == free_count_from_spec(map, from) + (if from <= idx { 1int } else { 0int }),
    decreases (map.len() - from),
{
    if from < map.len() {
        let updated = free_block_spec(map, idx);
        assert(updated.len() == map.len());
        if from == idx {
            assert(updated[from] == true);
            assert(map[from] == false);
        } else {
            assert(updated[from] == map[from]);
        }
        free_allocated_increments_from(map, idx, from + 1);
    }
}

// ─── Allocate-Free Roundtrip ─────────────────────────────────────────

// r[verify free-space-map.alloc-free-roundtrip]
/// Allocating then freeing the same block restores the original map.
proof fn alloc_free_roundtrip(map: Seq<bool>, idx: int)
    requires 0 <= idx < map.len(),
    ensures free_block_spec(allocate_spec(map, idx), idx) =~= map.update(idx, true),
{}

// r[verify free-space-map.alloc-free-identity]
/// Allocating then freeing a free block restores the original map exactly.
proof fn alloc_free_identity(map: Seq<bool>, idx: int)
    requires 0 <= idx < map.len(), map[idx] == true,
    ensures free_block_spec(allocate_spec(map, idx), idx) =~= map,
{}

// ─── First Free ──────────────────────────────────────────────────────

/// Spec: index of first free block at or after `from`, or -1 if none.
// r[depends free-space-map.first-free]
// r[impl free-space-map.first-free]
pub open spec fn first_free_from_spec(map: Seq<bool>, from: int) -> int
    decreases (map.len() - from),
{
    if from < 0 || from >= map.len() {
        -1int
    } else if map[from] {
        from
    } else {
        first_free_from_spec(map, from + 1)
    }
}

/// Spec: index of first free block, or -1.
pub open spec fn first_free_spec(map: Seq<bool>) -> int {
    first_free_from_spec(map, 0)
}

// r[impl free-space-map.first-free]
/// Find the first free block, returning -1 (as i64) if none.
pub fn first_free(map: &[bool]) -> (result: i64)
    requires map@.len() <= i64::MAX as int,
    ensures result as int == first_free_spec(map@),
{
    let mut i: usize = 0;
    while i < map.len()
        invariant
            0 <= i <= map@.len(),
            map@.len() <= i64::MAX as int,
            first_free_from_spec(map@, i as int) == first_free_spec(map@),
            forall|j: int| 0 <= j < i as int ==> !map@[j],
        decreases map@.len() - i,
    {
        if map[i] {
            return match i64::try_from(i) {
                Ok(value) => value,
                Err(_) => -1,
            };
        }
        i = i + 1;
    }
    -1i64
}

// r[verify free-space-map.first-free]
/// If first_free returns a valid index, that block is free.
proof fn first_free_is_free(map: Seq<bool>)
    ensures ({
        let ff = first_free_spec(map);
        ff >= 0 ==> (ff < map.len() && map[ff])
    }),
{
    first_free_from_is_free(map, 0);
}

proof fn first_free_from_is_free(map: Seq<bool>, from: int)
    ensures ({
        let ff = first_free_from_spec(map, from);
        ff >= 0 ==> (ff < map.len() && map[ff])
    }),
    decreases (map.len() - from),
{
    if from >= 0 && from < map.len() && !map[from] {
        first_free_from_is_free(map, from + 1);
    }
}

// r[verify free-space-map.first-free]
/// If first_free returns -1, no block is free.
proof fn first_free_none_means_full(map: Seq<bool>)
    ensures
        first_free_spec(map) == -1 ==>
            forall|j: int| 0 <= j < map.len() ==> !map[j],
{
    first_free_from_none_means_full(map, 0);
}

proof fn first_free_from_none_means_full(map: Seq<bool>, from: int)
    requires 0 <= from,
    ensures
        first_free_from_spec(map, from) == -1 ==>
            forall|j: int| #![trigger map[j]] from <= j < map.len() ==> !map[j],
    decreases (map.len() - from),
{
    if from < map.len() {
        if map[from] {
            // first_free_from returns `from`, not -1 — vacuously true
        } else {
            first_free_from_none_means_full(map, from + 1);
        }
    }
}

// ─── Contiguous Free Run ─────────────────────────────────────────────

/// Spec: length of contiguous free run starting at `from`.
// r[depends free-space-map.contiguous-run]
// r[impl free-space-map.contiguous-run]
pub open spec fn contiguous_free_run_spec(map: Seq<bool>, from: int) -> int
    decreases (map.len() - from),
{
    if from < 0 || from >= map.len() || !map[from] {
        0
    } else {
        1 + contiguous_free_run_spec(map, from + 1)
    }
}

// r[impl free-space-map.contiguous-run]
pub fn contiguous_free_run(map: &[bool], from: usize) -> (result: u64)
    requires from <= map@.len(), map@.len() <= u64::MAX as int,
    ensures result as int == contiguous_free_run_spec(map@, from as int),
{
    let mut len: u64 = 0;
    let mut i: usize = from;
    while i < map.len() && map[i]
        invariant
            from <= i,
            i <= map@.len(),
            map@.len() <= u64::MAX as int,
            len as int == (i - from) as int,
            contiguous_free_run_spec(map@, i as int) + len as int
                == contiguous_free_run_spec(map@, from as int),
        decreases map@.len() - i,
    {
        len = len + 1;
        i = i + 1;
    }
    len
}

// r[verify free-space-map.contiguous-run]
/// Run length never exceeds remaining map length.
proof fn contiguous_run_bounded(map: Seq<bool>, from: int)
    requires 0 <= from <= map.len(),
    ensures contiguous_free_run_spec(map, from) <= map.len() - from,
    decreases (map.len() - from),
{
    if from < map.len() && map[from] {
        contiguous_run_bounded(map, from + 1);
    }
}

// r[verify free-space-map.contiguous-run]
/// All blocks in the run are free.
proof fn contiguous_run_all_free(map: Seq<bool>, from: int)
    requires 0 <= from,
    ensures
        forall|j: int| #![trigger map[j]]
            from <= j < from + contiguous_free_run_spec(map, from) ==> map[j],
    decreases (map.len() - from),
{
    if from >= 0 && from < map.len() && map[from] {
        contiguous_run_all_free(map, from + 1);
    }
}

// ─── Fragmentation ───────────────────────────────────────────────────

/// Spec: fragmentation ratio as percentage: 100 * (1 - largest_run / free_count).
/// Returns 0 when no free blocks. Returns 0..=100.
// r[depends free-space-map.fragmentation]
// r[impl free-space-map.fragmentation]
pub open spec fn fragmentation_pct_spec(free: u64, largest_run: u64) -> u64
    recommends largest_run <= free,
{
    if free == 0 {
        0
    } else {
        (((free - largest_run) as int * 100) / (free as int)) as u64
    }
}

// r[impl free-space-map.fragmentation]
#[allow(clippy::manual_checked_ops)]
pub fn fragmentation_pct(free: u64, largest_run: u64) -> (result: u64)
    requires largest_run <= free, free <= u64::MAX / 100,
    ensures result == fragmentation_pct_spec(free, largest_run),
{
    if free == 0 {
        0
    } else {
        ((free - largest_run) * 100) / free
    }
}

// r[verify free-space-map.fragmentation]
/// No fragmentation when all free blocks are contiguous.
proof fn no_fragmentation_when_contiguous(free: u64)
    requires free > 0,
    ensures fragmentation_pct_spec(free, free) == 0,
{}

// r[verify free-space-map.fragmentation]
/// Maximum fragmentation when largest run is 0.
proof fn max_fragmentation_bound(free: u64)
    requires free > 0,
    ensures fragmentation_pct_spec(free, 0) == 100,
{
    let f = free as int;
    vstd::arithmetic::div_mod::lemma_fundamental_div_mod(f * 100, f);
    assert(f * 100 / f == 100) by(nonlinear_arith)
        requires f > 0;
}

} // verus!

#[cfg(test)]
#[allow(clippy::bool_assert_comparison)]
mod tests {
    use super::*;

    #[test]
    fn test_free_count_empty() {
        assert_eq!(free_count(&[]), 0);
    }

    #[test]
    fn test_free_count_all_free() {
        assert_eq!(free_count(&[true, true, true]), 3);
    }

    #[test]
    fn test_free_count_none_free() {
        assert_eq!(free_count(&[false, false, false]), 0);
    }

    #[test]
    fn test_free_count_mixed() {
        assert_eq!(free_count(&[true, false, true, false, true]), 3);
    }

    #[test]
    fn test_allocate_marks_used() {
        let mut map = vec![true, true, true];
        allocate(&mut map, 1);
        assert_eq!(map, vec![true, false, true]);
    }

    #[test]
    fn test_free_block_marks_free() {
        let mut map = vec![false, false, false];
        free_block(&mut map, 1);
        assert_eq!(map, vec![false, true, false]);
    }

    #[test]
    fn test_alloc_free_roundtrip() {
        let mut map = vec![true, true, true];
        allocate(&mut map, 1);
        assert_eq!(map[1], false);
        free_block(&mut map, 1);
        assert_eq!(map, vec![true, true, true]);
    }

    #[test]
    fn test_first_free_found() {
        assert_eq!(first_free(&[false, false, true, false]), 2);
    }

    #[test]
    fn test_first_free_first() {
        assert_eq!(first_free(&[true, false, false]), 0);
    }

    #[test]
    fn test_first_free_none() {
        assert_eq!(first_free(&[false, false, false]), -1);
    }

    #[test]
    fn test_first_free_empty() {
        assert_eq!(first_free(&[]), -1);
    }

    #[test]
    fn test_contiguous_free_run() {
        let map = [false, true, true, true, false, true];
        assert_eq!(contiguous_free_run(&map, 1), 3);
        assert_eq!(contiguous_free_run(&map, 0), 0);
        assert_eq!(contiguous_free_run(&map, 5), 1);
    }

    #[test]
    fn test_fragmentation_pct() {
        assert_eq!(fragmentation_pct(10, 10), 0);
        assert_eq!(fragmentation_pct(10, 0), 100);
        assert_eq!(fragmentation_pct(0, 0), 0);
        assert_eq!(fragmentation_pct(10, 5), 50);
    }
}

#[cfg(test)]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
mod proptests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        #[test]
        fn prop_free_count_bounded(map in proptest::collection::vec(any::<bool>(), 0..200)) {
            let count = free_count(&map);
            prop_assert!(count as usize <= map.len());
        }

        #[test]
        fn prop_allocate_decrements_or_same(
            map in proptest::collection::vec(any::<bool>(), 1..100usize),
            idx in any::<proptest::sample::Index>(),
        ) {
            let real_idx = idx.index(map.len());
            let before = free_count(&map);
            let mut m = map.clone();
            allocate(&mut m, real_idx);
            let after = free_count(&m);
            if map[real_idx] {
                prop_assert_eq!(after, before - 1);
            } else {
                prop_assert_eq!(after, before);
            }
        }

        #[test]
        fn prop_alloc_free_roundtrip(
            map in proptest::collection::vec(proptest::bool::ANY, 1..100usize),
            idx in any::<proptest::sample::Index>(),
        ) {
            let real_idx = idx.index(map.len());
            if map[real_idx] {
                let mut m = map.clone();
                allocate(&mut m, real_idx);
                free_block(&mut m, real_idx);
                prop_assert_eq!(m, map);
            }
        }

        #[test]
        fn prop_first_free_correct(map in proptest::collection::vec(any::<bool>(), 0..200)) {
            let ff = first_free(&map);
            if ff >= 0 {
                let idx = ff as usize;
                prop_assert!(idx < map.len());
                prop_assert!(map[idx]);
                for j in 0..idx {
                    prop_assert!(!map[j]);
                }
            } else {
                for j in 0..map.len() {
                    prop_assert!(!map[j]);
                }
            }
        }

        #[test]
        fn prop_contiguous_run_correct(
            map in proptest::collection::vec(any::<bool>(), 0..200),
            from_idx in any::<proptest::sample::Index>(),
        ) {
            if !map.is_empty() {
                let from = from_idx.index(map.len());
                let run = contiguous_free_run(&map, from) as usize;
                for j in from..from + run {
                    prop_assert!(map[j]);
                }
                if from + run < map.len() {
                    prop_assert!(!map[from + run]);
                }
            }
        }
    }
}
