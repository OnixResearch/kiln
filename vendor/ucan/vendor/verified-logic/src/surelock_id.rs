//! Verified surelock lock identities.
//!
//! Models stable lock records with monotonic allocation. Fresh locks take the
//! current allocator ID, then advance allocator state by one. IDs stay attached
//! to the lock record regardless of where caller stores that record.

use vstd::prelude::*;

verus! {

#[derive(Copy, Clone, Debug)]
pub struct SureLock {
    pub id: u64,
    pub level: u64,
}

#[derive(Copy, Clone, Debug)]
pub struct LockAlloc {
    pub next_id: u64,
}

// r[depends surelock-lock-id.stable-identity]
pub open spec fn lock_id_spec(lock: SureLock) -> u64 {
    lock.id
}

// r[impl surelock-lock-id.stable-identity]
pub fn lock_id(lock: &SureLock) -> (result: u64)
    ensures result == lock_id_spec(*lock)
{
    lock.id
}

// r[depends surelock-level-order.lock-after]
pub open spec fn lock_level_spec(lock: SureLock) -> u64 {
    lock.level
}

// r[impl surelock-level-order.lock-after]
pub fn lock_level(lock: &SureLock) -> (result: u64)
    ensures result == lock_level_spec(*lock)
{
    lock.level
}

// r[depends surelock-lock-id.unique]
// r[depends surelock-lock-id.monotonic]
pub open spec fn initial_alloc_spec(start: u64) -> LockAlloc {
    LockAlloc { next_id: start }
}

// r[impl surelock-lock-id.unique]
// r[impl surelock-lock-id.monotonic]
pub fn initial_alloc(start: u64) -> (result: LockAlloc)
    ensures result == initial_alloc_spec(start)
{
    LockAlloc { next_id: start }
}

// r[depends surelock-lock-id.unique]
// r[depends surelock-lock-id.monotonic]
pub open spec fn allocate_lock_spec(alloc: LockAlloc, level: u64) -> Option<(SureLock, LockAlloc)> {
    if alloc.next_id == u64::MAX {
        None
    } else {
        Some((
            SureLock { id: alloc.next_id, level },
            LockAlloc { next_id: (alloc.next_id + 1) as u64 },
        ))
    }
}

// r[impl surelock-lock-id.unique]
// r[impl surelock-lock-id.monotonic]
pub fn allocate_lock(alloc: &LockAlloc, level: u64) -> (result: Option<(SureLock, LockAlloc)>)
    ensures result == allocate_lock_spec(*alloc, level)
{
    if alloc.next_id == u64::MAX {
        None
    } else {
        Some((
            SureLock {
                id: alloc.next_id,
                level,
            },
            LockAlloc {
                next_id: alloc.next_id + 1,
            },
        ))
    }
}

// r[depends surelock-lock-id.unique]
pub open spec fn contains_lock_id_spec(locks: Seq<SureLock>, id: u64) -> bool {
    exists|i: int| #![trigger locks[i]] 0 <= i < locks.len() && locks[i].id == id
}

// r[depends surelock-lock-id.monotonic]
pub open spec fn ids_strict_increasing_spec(locks: Seq<SureLock>) -> bool {
    forall|i: int, j: int| #![trigger locks[i], locks[j]]
        0 <= i < j < locks.len() ==> locks[i].id < locks[j].id
}

// r[verify surelock-lock-id.unique]
proof fn successive_allocations_distinct(alloc: LockAlloc, level_a: u64, level_b: u64)
    requires alloc.next_id < u64::MAX - 1,
    ensures allocate_lock_spec(alloc, level_a).unwrap().0.id
        != allocate_lock_spec(allocate_lock_spec(alloc, level_a).unwrap().1, level_b).unwrap().0.id
{}

// r[verify surelock-lock-id.monotonic]
proof fn successive_allocations_increase(alloc: LockAlloc, level_a: u64, level_b: u64)
    requires alloc.next_id < u64::MAX - 1,
    ensures allocate_lock_spec(alloc, level_a).unwrap().0.id
        < allocate_lock_spec(allocate_lock_spec(alloc, level_a).unwrap().1, level_b).unwrap().0.id
{}

// r[verify surelock-lock-id.monotonic]
proof fn triple_creation_is_strict(alloc: LockAlloc, a_level: u64, b_level: u64, c_level: u64)
    requires alloc.next_id < u64::MAX - 2,
    ensures
        allocate_lock_spec(alloc, a_level).unwrap().0.id
            < allocate_lock_spec(allocate_lock_spec(alloc, a_level).unwrap().1, b_level).unwrap().0.id,
        allocate_lock_spec(allocate_lock_spec(alloc, a_level).unwrap().1, b_level).unwrap().0.id
            < allocate_lock_spec(
                allocate_lock_spec(allocate_lock_spec(alloc, a_level).unwrap().1, b_level).unwrap().1,
                c_level,
            ).unwrap().0.id,
        allocate_lock_spec(alloc, a_level).unwrap().0.id
            < allocate_lock_spec(
                allocate_lock_spec(allocate_lock_spec(alloc, a_level).unwrap().1, b_level).unwrap().1,
                c_level,
            ).unwrap().0.id,
{}

// r[verify surelock-lock-id.stable-identity]
proof fn lock_identity_survives_context(lock: SureLock, before: Seq<SureLock>, after: Seq<SureLock>)
    ensures (before + seq![lock] + after)[before.len() as int].id == lock.id
{}

// r[verify surelock-lock-id.unique]
proof fn increasing_ids_imply_unique(locks: Seq<SureLock>)
    requires ids_strict_increasing_spec(locks),
    ensures forall|i: int, j: int| #![trigger locks[i], locks[j]]
        0 <= i < j < locks.len() ==> locks[i].id != locks[j].id
{
    assert forall|i: int, j: int| #![trigger locks[i], locks[j]]
        0 <= i < j < locks.len()
    implies locks[i].id != locks[j].id by {
        assert(locks[i].id < locks[j].id);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify surelock-lock-id.unique]
    #[test]
    fn test_successive_allocations_distinct() {
        let alloc = initial_alloc(10);
        let (a, alloc2) = allocate_lock(&alloc, 1).unwrap();
        let (b, _) = allocate_lock(&alloc2, 1).unwrap();
        assert_ne!(a.id, b.id);
    }

    // r[verify surelock-lock-id.monotonic]
    #[test]
    fn test_successive_allocations_increase() {
        let alloc = initial_alloc(7);
        let (a, alloc2) = allocate_lock(&alloc, 2).unwrap();
        let (b, alloc3) = allocate_lock(&alloc2, 2).unwrap();
        let (c, _) = allocate_lock(&alloc3, 2).unwrap();
        assert!(a.id < b.id);
        assert!(b.id < c.id);
    }

    // r[verify surelock-lock-id.stable-identity]
    #[test]
    fn test_reordering_keeps_ids() {
        let alloc = initial_alloc(3);
        let (a, alloc2) = allocate_lock(&alloc, 0).unwrap();
        let (b, _) = allocate_lock(&alloc2, 0).unwrap();
        let forward = [a, b];
        let reverse = [b, a];
        assert_eq!(lock_id(&forward[0]), a.id);
        assert_eq!(lock_id(&reverse[1]), a.id);
        assert_eq!(lock_id(&forward[1]), b.id);
        assert_eq!(lock_id(&reverse[0]), b.id);
    }

    proptest! {
        // r[verify surelock-lock-id.unique]
        #[test]
        fn prop_successive_allocations_distinct(start in 0u64..(u64::MAX - 1)) {
            let alloc = initial_alloc(start);
            let (a, alloc2) = allocate_lock(&alloc, 1).unwrap();
            let (b, _) = allocate_lock(&alloc2, 2).unwrap();
            prop_assert_ne!(a.id, b.id);
        }

        // r[verify surelock-lock-id.monotonic]
        #[test]
        fn prop_successive_allocations_increase(start in 0u64..(u64::MAX - 1)) {
            let alloc = initial_alloc(start);
            let (a, alloc2) = allocate_lock(&alloc, 1).unwrap();
            let (b, _) = allocate_lock(&alloc2, 2).unwrap();
            prop_assert!(a.id < b.id);
        }

        // r[verify surelock-lock-id.stable-identity]
        #[test]
        fn prop_lock_id_stable_under_swap(start in 0u64..(u64::MAX - 1), level_a: u64, level_b: u64) {
            let alloc = initial_alloc(start);
            let (a, alloc2) = allocate_lock(&alloc, level_a).unwrap();
            let (b, _) = allocate_lock(&alloc2, level_b).unwrap();
            let forward = [a, b];
            let reverse = [b, a];
            prop_assert_eq!(lock_id(&forward[0]), lock_id(&reverse[1]));
            prop_assert_eq!(lock_id(&forward[1]), lock_id(&reverse[0]));
        }
    }
}
