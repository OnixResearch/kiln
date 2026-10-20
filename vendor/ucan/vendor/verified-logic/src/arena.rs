//! Verified arena (region-based) allocation with collective free.
//!
//! The simplest allocator: bump-allocate from a region, free everything
//! at once via reset. Each reset increments a generation counter, so
//! pointers from prior generations are provably invalid. Builds on
//! [`watermark`] (bump arithmetic) and [`alloc_token`] (lifecycle).

use vstd::prelude::*;

verus! {

// ─── Arena Alloc ─────────────────────────────────────────────────────

// r[depends arena.alloc]
/// Spec: bump-allocate `size` bytes. Returns (ptr, new_mark).
/// ptr is the old mark (start of allocated region).
pub open spec fn arena_alloc_spec(mark: u64, size: u64) -> (u64, u64)
    recommends mark as int + size as int <= u64::MAX as int
{
    (mark, (mark + size) as u64)
}

// r[impl arena.alloc]
/// Bump-allocate `size` bytes from the arena. Returns (ptr, new_mark).
pub fn arena_alloc(mark: u64, size: u64) -> (result: (u64, u64))
    requires mark as int + size as int <= u64::MAX as int
    ensures result == arena_alloc_spec(mark, size)
{
    (mark, mark + size)
}

// ─── Arena Reset ─────────────────────────────────────────────────────

// r[depends arena.reset]
/// Spec: reset arena to base, increment generation.
/// Returns (new_mark, new_generation).
pub open spec fn arena_reset_spec(base: u64, generation: u64) -> (u64, u64)
    recommends generation < u64::MAX
{
    (base, (generation + 1) as u64)
}

// r[impl arena.reset]
/// Reset arena: mark returns to base, generation increments.
pub fn arena_reset(base: u64, generation: u64) -> (result: (u64, u64))
    requires generation < u64::MAX
    ensures result == arena_reset_spec(base, generation)
{
    (base, generation + 1)
}

// ─── Arena Remaining ─────────────────────────────────────────────────

// r[depends arena.remaining]
/// Spec: bytes remaining in the arena.
pub open spec fn arena_remaining_spec(mark: u64, base: u64, capacity: u64) -> u64
    recommends base <= mark, mark as int <= base as int + capacity as int
{
    (base as int + capacity as int - mark as int) as u64
}

// r[impl arena.remaining]
/// Returns the number of bytes remaining in the arena.
pub fn arena_remaining(mark: u64, base: u64, capacity: u64) -> (result: u64)
    requires
        base <= mark,
        mark as int <= base as int + capacity as int,
        base as int + capacity as int <= u64::MAX as int,
    ensures result == arena_remaining_spec(mark, base, capacity)
{
    base + capacity - mark
}

// ─── Arena Generation ────────────────────────────────────────────────

// r[depends arena.generation]
/// Spec: current arena generation.
pub open spec fn arena_generation_spec(generation: u64) -> u64 {
    generation
}

// r[impl arena.generation]
/// Returns the current generation.
pub fn arena_generation(generation: u64) -> (result: u64)
    ensures result == arena_generation_spec(generation)
{
    generation
}

// ─── Arena Is Valid ──────────────────────────────────────────────────

// r[depends arena.is-valid]
/// Spec: a pointer is valid iff its generation matches the current generation.
pub open spec fn arena_is_valid_spec(ptr_gen: u64, current_gen: u64) -> bool {
    ptr_gen == current_gen
}

// r[impl arena.is-valid]
/// Returns true iff the pointer's generation matches the arena's current.
pub fn arena_is_valid(ptr_gen: u64, current_gen: u64) -> (result: bool)
    ensures result == arena_is_valid_spec(ptr_gen, current_gen)
{
    ptr_gen == current_gen
}

// ─── Arena Fits ──────────────────────────────────────────────────────

// r[depends arena.fits]
/// Spec: allocation of `size` bytes fits.
pub open spec fn arena_fits_spec(mark: u64, size: u64, base: u64, capacity: u64) -> bool
    recommends base <= mark, mark as int <= base as int + capacity as int
{
    mark as int + size as int <= base as int + capacity as int
}

// r[impl arena.fits]
/// Returns true if `size` bytes fit in remaining space.
pub fn arena_fits(mark: u64, size: u64, base: u64, capacity: u64) -> (result: bool)
    requires
        base <= mark,
        mark as int <= base as int + capacity as int,
        base as int + capacity as int <= u64::MAX as int,
    ensures result == arena_fits_spec(mark, size, base, capacity)
{
    let remaining = base + capacity - mark;
    size <= remaining
}

// ─── Proofs ──────────────────────────────────────────────────────────

// r[verify arena.alloc]
/// Successive allocs produce disjoint regions.
proof fn successive_allocs_disjoint(mark: u64, s1: u64, s2: u64)
    requires
        s1 > 0,
        s2 > 0,
        mark as int + s1 as int + s2 as int <= u64::MAX as int,
    ensures ({
        let (p1, m1) = arena_alloc_spec(mark, s1);
        let (p2, _m2) = arena_alloc_spec(m1, s2);
        // Region 1: [p1, p1 + s1), Region 2: [p2, p2 + s2)
        p1 as int + s1 as int <= p2 as int
    })
{
    let (p1, m1) = arena_alloc_spec(mark, s1);
    let (p2, _m2) = arena_alloc_spec(m1, s2);
    assert(p1 == mark);
    assert(m1 == mark + s1);
    assert(p2 == m1);
    assert(p1 as int + s1 as int == p2 as int);
}

// r[verify arena.alloc]
/// Alloc advances the mark monotonically.
proof fn alloc_advances_mark(mark: u64, size: u64)
    requires mark as int + size as int <= u64::MAX as int
    ensures arena_alloc_spec(mark, size).1 as int >= mark as int
{
    let (_ptr, new_mark) = arena_alloc_spec(mark, size);
    assert(new_mark == mark + size);
    assert(new_mark as int == mark as int + size as int);
}

// r[verify arena.reset]
/// After reset, remaining equals capacity.
proof fn reset_restores_capacity(base: u64, capacity: u64, generation: u64)
    requires
        generation < u64::MAX,
        base as int + capacity as int <= u64::MAX as int,
    ensures ({
        let (new_mark, _new_gen) = arena_reset_spec(base, generation);
        arena_remaining_spec(new_mark, base, capacity) == capacity
    })
{
    let (new_mark, _new_gen) = arena_reset_spec(base, generation);
    assert(new_mark == base);
    assert(arena_remaining_spec(new_mark, base, capacity) as int
        == base as int + capacity as int - base as int);
    assert(base as int + capacity as int - base as int == capacity as int);
}

// r[verify arena.reset]
/// Reset invalidates pointers from prior generation.
proof fn reset_invalidates(base: u64, generation: u64)
    requires generation < u64::MAX
    ensures ({
        let (_new_mark, new_gen) = arena_reset_spec(base, generation);
        !arena_is_valid_spec(generation, new_gen)
    })
{
    let (_new_mark, new_gen) = arena_reset_spec(base, generation);
    assert(new_gen == generation + 1);
    assert(new_gen as int == generation as int + 1);
    assert(new_gen != generation);
}

// r[verify arena.reset]
/// Generation monotonically increases.
proof fn generation_monotonic(base: u64, generation: u64)
    requires generation < u64::MAX
    ensures arena_reset_spec(base, generation).1 as int > generation as int
{
    let (_new_mark, new_generation) = arena_reset_spec(base, generation);
    assert(new_generation == generation + 1);
    assert(new_generation as int == generation as int + 1);
}

// r[verify arena.is-valid]
/// Pointer from generation g is invalid in generation g+1.
proof fn stale_pointer_invalid(g: u64)
    requires g < u64::MAX
    ensures !arena_is_valid_spec(g, (g + 1) as u64)
{
    let next_generation = (g + 1) as u64;
    assert(next_generation as int == g as int + 1);
    assert(next_generation != g);
}

// r[verify arena.reset]
/// Arena reclamation is collective: reset restores the base and advances generation.
proof fn collective_reset_reclaims_all(base: u64, generation: u64)
    requires generation < u64::MAX
    ensures ({
        let (new_mark, new_generation) = arena_reset_spec(base, generation);
        new_mark == base && new_generation as int > generation as int
    })
{
    let (new_mark, new_generation) = arena_reset_spec(base, generation);
    assert(new_mark == base);
    assert(new_generation == generation + 1);
    assert(new_generation as int == generation as int + 1);
}

// r[verify arena.remaining]
/// At base, remaining equals capacity.
proof fn remaining_at_base(base: u64, capacity: u64)
    requires base as int + capacity as int <= u64::MAX as int
    ensures arena_remaining_spec(base, base, capacity) == capacity
{
    assert(arena_remaining_spec(base, base, capacity) as int
        == base as int + capacity as int - base as int);
    assert(base as int + capacity as int - base as int == capacity as int);
}

// r[verify arena.fits]
/// Zero-size allocation always fits.
proof fn zero_always_fits(mark: u64, base: u64, capacity: u64)
    requires base <= mark, mark as int <= base as int + capacity as int
    ensures arena_fits_spec(mark, 0, base, capacity)
{
    assert(mark as int + 0int == mark as int);
    assert(mark as int <= base as int + capacity as int);
}

// r[verify arena.alloc]
/// Alloc reduces remaining: new_remaining == old_remaining - size.
proof fn alloc_decreases_remaining(mark: u64, size: u64, base: u64, capacity: u64)
    requires
        base <= mark,
        mark as int <= base as int + capacity as int,
        mark as int + size as int <= base as int + capacity as int,
        base as int + capacity as int <= u64::MAX as int,
    ensures ({
        let new_mark = (mark + size) as u64;
        arena_remaining_spec(new_mark, base, capacity) as int
            + size as int
            == arena_remaining_spec(mark, base, capacity) as int
    })
{
    let new_mark = (mark + size) as u64;
    assert(new_mark as int == mark as int + size as int);
    assert(arena_remaining_spec(new_mark, base, capacity) as int
        == base as int + capacity as int - new_mark as int);
    assert(arena_remaining_spec(mark, base, capacity) as int
        == base as int + capacity as int - mark as int);
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    const EDGE_BASE: u64 = 200;
    const EDGE_CAPACITY: u64 = 300;
    const EDGE_MARK: u64 = 350;
    const EDGE_ALLOC_SIZE: u64 = 50;
    const EDGE_GENERATION: u64 = 7;

    // r[verify arena.alloc]
    // r[verify arena.generation]
    #[test]
    fn test_arena_alloc() {
        let (ptr, mark) = arena_alloc(100, 32);
        assert_eq!(ptr, 100);
        assert_eq!(mark, 132);
    }

    // r[verify arena.reset]
    #[test]
    fn test_arena_reset() {
        let (mark, generation) = arena_reset(100, 0);
        assert_eq!(mark, 100);
        assert_eq!(generation, 1);
    }

    // r[verify arena.remaining]
    #[test]
    fn test_arena_remaining() {
        assert_eq!(arena_remaining(100, 100, 256), 256);
        assert_eq!(arena_remaining(132, 100, 256), 224);
    }

    // r[verify arena.is-valid]
    #[test]
    fn test_arena_is_valid() {
        assert!(arena_is_valid(0, 0));
        assert!(!arena_is_valid(0, 1));
    }

    // r[verify arena.reset]
    #[test]
    fn test_reset_invalidates() {
        let g = 0u64;
        let (_mark, new_gen) = arena_reset(0, g);
        assert!(!arena_is_valid(g, new_gen));
    }

    // r[verify arena.fits]
    #[test]
    fn test_arena_fits() {
        assert!(arena_fits(100, 100, 100, 256));
        assert!(!arena_fits(100, 257, 100, 256));
        assert!(arena_fits(100, 0, 100, 256));
    }

    // r[verify arena.alloc]
    #[test]
    fn test_alloc_advances_mark_edge_case() {
        let (_ptr, new_mark) = arena_alloc(EDGE_MARK, EDGE_ALLOC_SIZE);
        assert_eq!(new_mark, EDGE_MARK + EDGE_ALLOC_SIZE);
        assert!(new_mark >= EDGE_MARK);
    }

    // r[verify arena.reset]
    #[test]
    fn test_collective_reset_restores_base_and_generation() {
        let (new_mark, new_generation) = arena_reset(EDGE_BASE, EDGE_GENERATION);
        assert_eq!(new_mark, EDGE_BASE);
        assert_eq!(new_generation, EDGE_GENERATION + 1);
    }

    // r[verify arena.remaining]
    #[test]
    fn test_remaining_at_base_edge_case() {
        assert_eq!(arena_remaining(EDGE_BASE, EDGE_BASE, EDGE_CAPACITY), EDGE_CAPACITY);
    }

    // r[verify arena.fits]
    #[test]
    fn test_zero_fits_but_over_capacity_fails() {
        let end_mark = EDGE_BASE + EDGE_CAPACITY;
        assert!(arena_fits(end_mark, 0, EDGE_BASE, EDGE_CAPACITY));
        assert!(!arena_fits(end_mark, EDGE_ALLOC_SIZE, EDGE_BASE, EDGE_CAPACITY));
    }

    // r[verify arena.is-valid]
    #[test]
    fn test_next_generation_rejects_stale_pointer() {
        assert!(!arena_is_valid(EDGE_GENERATION, EDGE_GENERATION + 1));
    }

    proptest! {
        // r[verify arena.alloc]
        #[test]
        fn prop_successive_disjoint(
            base in 0u64..1000,
            s1 in 1u64..100,
            s2 in 1u64..100,
        ) {
            let (p1, m1) = arena_alloc(base, s1);
            let (p2, _m2) = arena_alloc(m1, s2);
            prop_assert!(p1 + s1 <= p2);
        }

        // r[verify arena.reset]
        #[test]
        fn prop_reset_restores_capacity(
            base in 0u64..1000,
            capacity in 1u64..1000,
            generation in 0u64..u64::MAX,
        ) {
            if base as u128 + capacity as u128 <= u64::MAX as u128 {
                let (new_mark, _) = arena_reset(base, generation);
                prop_assert_eq!(arena_remaining(new_mark, base, capacity), capacity);
            }
        }

        // r[verify arena.is-valid]
        #[test]
        fn prop_stale_invalid(generation in 0u64..(u64::MAX - 1)) {
            let (_, new_gen) = arena_reset(0, generation);
            prop_assert!(!arena_is_valid(generation, new_gen));
        }

        // r[verify arena.alloc]
        #[test]
        fn prop_alloc_monotonic(mark in 0u64..10000, size in 0u64..10000) {
            if mark as u128 + size as u128 <= u64::MAX as u128 {
                let (_, new_mark) = arena_alloc(mark, size);
                prop_assert!(new_mark >= mark);
            }
        }
    }
}
