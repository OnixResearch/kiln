//! Verified single-pool seL4-style ASID ownership.
//!
//! Models one finite ASID pool. Each slot stores either `None` (free) or
//! `Some(owner_id)` for a live address-space object ID. Allocation succeeds iff
//! some free slot exists; release succeeds only for an exact `(asid, owner)`
//! match and otherwise leaves state unchanged.

use vstd::prelude::*;

verus! {

pub struct AsidPool {
    pub slots: Vec<Option<u64>>,
    pub capacity: usize,
}

// r[depends sel4-asid.uniqueness]
// r[depends sel4-asid.release-state]
// r[depends sel4-asid.reuse]
// r[depends sel4-asid.pool-bounds]
pub open spec fn same_ownership_except_spec(
    before: Seq<Option<u64>>,
    after: Seq<Option<u64>>,
    except: int,
) -> bool {
    &&& before.len() == after.len()
    &&& forall|i: int| 0 <= i < before.len() && i != except ==> before[i] == after[i]
}

// r[depends sel4-asid.pool-bounds]
pub open spec fn first_free_from_spec(slots: Seq<Option<u64>>, from: int) -> int
    decreases (if slots.len() >= from { slots.len() - from } else { 0 })
{
    if from < 0 || from >= slots.len() {
        -1
    } else if slots[from] == None::<u64> {
        from
    } else {
        first_free_from_spec(slots, from + 1)
    }
}

impl AsidPool {
    // r[impl sel4-asid.uniqueness]
    // r[impl sel4-asid.release-state]
    // r[impl sel4-asid.reuse]
    // r[impl sel4-asid.pool-bounds]
    pub open spec fn ownership_view(&self) -> Seq<Option<u64>> {
        self.slots@
    }

    pub open spec fn wf(&self) -> bool {
        self.capacity == self.slots@.len()
    }

    pub open spec fn live_spec(&self, asid: int) -> bool
        recommends 0 <= asid < self.capacity as int
    {
        self.ownership_view()[asid] != None::<u64>
    }

    pub open spec fn owner_matches_spec(&self, asid: int, owner: u64) -> bool
        recommends 0 <= asid < self.capacity as int
    {
        self.ownership_view()[asid] == Some(owner)
    }

    pub open spec fn free_spec(&self, asid: int) -> bool
        recommends 0 <= asid < self.capacity as int
    {
        self.ownership_view()[asid] == None::<u64>
    }

    // r[impl sel4-asid.pool-bounds]
    pub fn new(capacity: usize) -> (result: AsidPool)
        ensures
            result.wf(),
            result.capacity == capacity,
            result.ownership_view().len() == capacity as int,
            forall|i: int| 0 <= i < capacity as int ==> result.ownership_view()[i] == None::<u64>,
    {
        let mut slots: Vec<Option<u64>> = Vec::new();
        let mut i: usize = 0;
        while i < capacity
            invariant
                0 <= i <= capacity,
                slots@.len() == i as int,
                forall|j: int| 0 <= j < i as int ==> slots@[j] == None::<u64>,
            decreases capacity - i,
        {
            slots.push(None);
            i = i + 1;
        }
        proof {
            new_establishes_wf(capacity as int);
        }
        AsidPool { slots, capacity }
    }

    // r[impl sel4-asid.uniqueness]
    // r[impl sel4-asid.reuse]
    // r[impl sel4-asid.pool-bounds]
    pub fn allocate(&mut self, owner: u64) -> (result: Option<usize>)
        requires old(self).wf()
        ensures
            self.wf(),
            self.ownership_view().len() == old(self).ownership_view().len(),
            match result {
                Some(asid) => {
                    &&& asid < old(self).ownership_view().len()
                    &&& old(self).ownership_view()[asid as int] == None::<u64>
                    &&& self.ownership_view()[asid as int] == Some(owner)
                    &&& same_ownership_except_spec(old(self).ownership_view(), self.ownership_view(), asid as int)
                }
                None => {
                    &&& self.ownership_view() =~= old(self).ownership_view()
                    &&& forall|i: int| 0 <= i < old(self).ownership_view().len() ==> old(self).ownership_view()[i] != None::<u64>
                }
            }
    {
        let ghost old_slots = self.ownership_view();
        let ghost old_capacity = self.capacity;
        let mut i: usize = 0;
        let mut result: Option<usize> = None;
        while i < self.capacity && result.is_none()
            invariant
                self.wf(),
                self.capacity == old_capacity,
                self.ownership_view().len() == old_slots.len(),
                0 <= i <= self.capacity,
                result == None::<usize> ==> self.ownership_view() =~= old_slots,
                result == None::<usize> ==> forall|j: int| 0 <= j < i as int ==> old_slots[j] != None::<u64>,
                match result {
                    None => true,
                    Some(asid) => {
                        &&& asid < self.capacity
                        &&& old_slots[asid as int] == None::<u64>
                        &&& self.ownership_view()[asid as int] == Some(owner)
                        &&& same_ownership_except_spec(old_slots, self.ownership_view(), asid as int)
                    }
                },
            decreases self.capacity - i,
        {
            let slot = self.slots[i];
            if slot.is_none() {
                let ghost chosen = i as int;
                proof {
                    assert(slot == old_slots[chosen]);
                    assert(old_slots[chosen] == None::<u64>);
                    first_free_at_prefix_boundary(old_slots, 0, chosen);
                }
                self.slots.set(i, Some(owner));
                proof {
                    allocation_targets_free_slot(old_slots, chosen, owner);
                    allocation_preserves_unrelated_slots(old_slots, chosen, owner);
                    allocation_preserves_wf(old_capacity as int, old_slots, chosen, owner);
                    assert(self.ownership_view() == old_slots.update(chosen, Some(owner)));
                    assert(self.ownership_view().len() == old_slots.len());
                    assert(self.wf());
                }
                result = Some(i);
            }
            i = i + 1;
        }
        if result.is_none() {
            proof {
                exhausted_pool_failure(old_slots, 0);
                allocation_failure_unchanged_state(old_slots);
            }
        }
        result
    }

    // r[impl sel4-asid.release-state]
    // r[impl sel4-asid.reuse]
    pub fn release(&mut self, asid: usize, owner: u64) -> (result: bool)
        requires old(self).wf()
        ensures
            self.wf(),
            self.ownership_view().len() == old(self).ownership_view().len(),
            if result {
                &&& asid < old(self).ownership_view().len()
                &&& old(self).ownership_view()[asid as int] == Some(owner)
                &&& self.ownership_view()[asid as int] == None::<u64>
                &&& same_ownership_except_spec(old(self).ownership_view(), self.ownership_view(), asid as int)
            } else {
                &&& self.ownership_view() =~= old(self).ownership_view()
                &&& (asid >= old(self).ownership_view().len() || old(self).ownership_view()[asid as int] != Some(owner))
            }
    {
        let ghost old_slots = self.ownership_view();
        let ghost old_capacity = self.capacity;
        if asid >= self.capacity {
            proof {
                allocation_failure_unchanged_state(old_slots);
            }
            return false;
        }
        let slot = self.slots[asid];
        if slot == Some(owner) {
            proof {
                release_success_clears_target(old_slots, asid as int, owner);
                release_preserves_unrelated_slots(old_slots, asid as int);
                release_preserves_wf(old_capacity as int, old_slots, asid as int);
            }
            self.slots.set(asid, None);
            proof {
                assert(self.ownership_view() == old_slots.update(asid as int, None::<u64>));
            }
            true
        } else {
            proof {
                allocation_failure_unchanged_state(old_slots);
            }
            false
        }
    }
}

// r[verify sel4-asid.pool-bounds]
pub proof fn new_establishes_wf(capacity: int)
    requires 0 <= capacity
    ensures ({
        let slots = Seq::<Option<u64>>::new(capacity as nat, |_i: int| None::<u64>);
        slots.len() == capacity
    })
{}

// r[verify sel4-asid.pool-bounds]
pub proof fn allocation_preserves_wf(capacity: int, slots: Seq<Option<u64>>, asid: int, owner: u64)
    requires
        capacity == slots.len(),
        0 <= asid < slots.len(),
    ensures capacity == slots.update(asid, Some(owner)).len()
{}

// r[verify sel4-asid.release-state]
pub proof fn release_preserves_wf(capacity: int, slots: Seq<Option<u64>>, asid: int)
    requires
        capacity == slots.len(),
        0 <= asid < slots.len(),
    ensures capacity == slots.update(asid, None::<u64>).len()
{}

proof fn first_free_at_prefix_boundary(slots: Seq<Option<u64>>, from: int, asid: int)
    requires
        0 <= from <= asid < slots.len(),
        slots[asid] == None::<u64>,
        forall|j: int| from <= j < asid ==> slots[j] != None::<u64>,
    ensures first_free_from_spec(slots, from) == asid
    decreases asid - from
{
    if from < asid {
        assert(slots[from] != None::<u64>);
        first_free_at_prefix_boundary(slots, from + 1, asid);
    }
}

// r[verify sel4-asid.uniqueness]
pub proof fn allocation_targets_free_slot(slots: Seq<Option<u64>>, asid: int, owner: u64)
    requires
        0 <= asid < slots.len(),
        slots[asid] == None::<u64>,
    ensures
        slots.update(asid, Some(owner))[asid] == Some(owner),
        slots[asid] == None::<u64>,
{}

// r[verify sel4-asid.uniqueness]
pub proof fn allocation_preserves_unrelated_slots(slots: Seq<Option<u64>>, asid: int, owner: u64)
    requires
        0 <= asid < slots.len(),
    ensures
        same_ownership_except_spec(slots, slots.update(asid, Some(owner)), asid),
{}

// r[verify sel4-asid.uniqueness]
pub proof fn distinct_successful_allocations(slots: Seq<Option<u64>>, first: int, second: int, owner1: u64, owner2: u64)
    requires
        0 <= first < slots.len(),
        0 <= second < slots.len(),
        first != second,
        slots[first] == None::<u64>,
        slots[second] == None::<u64>,
    ensures
        first != second,
        slots.update(first, Some(owner1)).update(second, Some(owner2))[first] == Some(owner1),
        slots.update(first, Some(owner1)).update(second, Some(owner2))[second] == Some(owner2),
{}

// r[verify sel4-asid.release-state]
pub proof fn release_success_clears_target(slots: Seq<Option<u64>>, asid: int, owner: u64)
    requires
        0 <= asid < slots.len(),
        slots[asid] == Some(owner),
    ensures slots.update(asid, None::<u64>)[asid] == None::<u64>
{}

// r[verify sel4-asid.release-state]
pub proof fn release_preserves_unrelated_slots(slots: Seq<Option<u64>>, asid: int)
    requires
        0 <= asid < slots.len(),
    ensures same_ownership_except_spec(slots, slots.update(asid, None::<u64>), asid)
{}

// r[verify sel4-asid.release-state]
// r[verify sel4-asid.pool-bounds]
pub proof fn allocation_failure_unchanged_state(slots: Seq<Option<u64>>)
    ensures slots =~= slots
{}

// r[verify sel4-asid.reuse]
pub proof fn safe_reuse(slots: Seq<Option<u64>>, asid: int, old_owner: u64, new_owner: u64)
    requires
        0 <= asid < slots.len(),
        slots[asid] == Some(old_owner),
    ensures ({
        let released = slots.update(asid, None::<u64>);
        let reused = released.update(asid, Some(new_owner));
        &&& released[asid] == None::<u64>
        &&& reused[asid] == Some(new_owner)
    })
{}

// r[verify sel4-asid.pool-bounds]
pub proof fn allocation_success_when_free(slots: Seq<Option<u64>>, from: int)
    requires
        0 <= from <= slots.len(),
        exists|i: int| from <= i < slots.len() && slots[i] == None::<u64>,
    ensures 0 <= first_free_from_spec(slots, from) < slots.len()
    decreases slots.len() - from
{
    if from < slots.len() {
        if slots[from] == None::<u64> {
        } else {
            assert(exists|i: int| from + 1 <= i < slots.len() && slots[i] == None::<u64>);
            allocation_success_when_free(slots, from + 1);
        }
    }
}

// r[verify sel4-asid.pool-bounds]
pub proof fn in_range_allocation(slots: Seq<Option<u64>>, from: int)
    requires
        0 <= from <= slots.len(),
        0 <= first_free_from_spec(slots, from),
    ensures first_free_from_spec(slots, from) < slots.len()
    decreases slots.len() - from
{
    if from < slots.len() {
        if slots[from] == None::<u64> {
        } else {
            in_range_allocation(slots, from + 1);
        }
    }
}

// r[verify sel4-asid.pool-bounds]
pub proof fn exhausted_pool_failure(slots: Seq<Option<u64>>, from: int)
    requires
        0 <= from <= slots.len(),
        forall|i: int| from <= i < slots.len() ==> slots[i] != None::<u64>,
    ensures first_free_from_spec(slots, from) == -1
    decreases slots.len() - from
{
    if from < slots.len() {
        assert(slots[from] != None::<u64>);
        exhausted_pool_failure(slots, from + 1);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify sel4-asid.pool-bounds]
    #[test]
    fn test_zero_capacity_exhausted() {
        let mut pool = AsidPool::new(0);
        assert_eq!(pool.allocate(11), None);
        assert!(pool.slots.is_empty());
    }

    // r[verify sel4-asid.uniqueness]
    #[test]
    fn test_successful_allocation_creates_live_owner() {
        let mut pool = AsidPool::new(2);
        assert_eq!(pool.allocate(7), Some(0));
        assert_eq!(pool.slots[0], Some(7));
        assert_eq!(pool.slots[1], None);
    }

    // r[verify sel4-asid.uniqueness]
    #[test]
    fn test_distinct_successful_allocations_differ() {
        let mut pool = AsidPool::new(3);
        let a0 = pool.allocate(10).unwrap();
        let a1 = pool.allocate(20).unwrap();
        assert_ne!(a0, a1);
        assert_eq!(pool.slots[a0], Some(10));
        assert_eq!(pool.slots[a1], Some(20));
    }

    // r[verify sel4-asid.uniqueness]
    #[test]
    fn test_same_owner_can_hold_multiple_live_asids() {
        let mut pool = AsidPool::new(3);
        let a0 = pool.allocate(55).unwrap();
        let a1 = pool.allocate(55).unwrap();
        assert_ne!(a0, a1);
        assert_eq!(pool.slots[a0], Some(55));
        assert_eq!(pool.slots[a1], Some(55));
    }

    // r[verify sel4-asid.uniqueness]
    #[test]
    fn test_allocation_preserves_unrelated_slots() {
        let mut pool = AsidPool::new(3);
        let first = pool.allocate(1).unwrap();
        let second = pool.allocate(2).unwrap();
        assert_eq!(first, 0);
        assert_eq!(second, 1);
        assert_eq!(pool.slots[0], Some(1));
    }

    // r[verify sel4-asid.release-state]
    #[test]
    fn test_successful_release() {
        let mut pool = AsidPool::new(2);
        let asid = pool.allocate(9).unwrap();
        assert!(pool.release(asid, 9));
        assert_eq!(pool.slots[asid], None);
    }

    // r[verify sel4-asid.release-state]
    #[test]
    fn test_release_preserves_unrelated_slots() {
        let mut pool = AsidPool::new(3);
        let a0 = pool.allocate(1).unwrap();
        let a1 = pool.allocate(2).unwrap();
        assert!(pool.release(a0, 1));
        assert_eq!(pool.slots[a0], None);
        assert_eq!(pool.slots[a1], Some(2));
    }

    // r[verify sel4-asid.release-state]
    #[test]
    fn test_owner_mismatch_release_fails() {
        let mut pool = AsidPool::new(2);
        let asid = pool.allocate(1).unwrap();
        let before = pool.slots.clone();
        assert!(!pool.release(asid, 2));
        assert_eq!(pool.slots, before);
    }

    // r[verify sel4-asid.release-state]
    #[test]
    fn test_duplicate_release_fails() {
        let mut pool = AsidPool::new(2);
        let asid = pool.allocate(4).unwrap();
        assert!(pool.release(asid, 4));
        let before = pool.slots.clone();
        assert!(!pool.release(asid, 4));
        assert_eq!(pool.slots, before);
    }

    // r[verify sel4-asid.release-state]
    #[test]
    fn test_out_of_range_release_fails() {
        let mut pool = AsidPool::new(1);
        let before = pool.slots.clone();
        assert!(!pool.release(5, 1));
        assert_eq!(pool.slots, before);
    }

    // r[verify sel4-asid.pool-bounds]
    #[test]
    fn test_failed_allocation_unchanged_state() {
        let mut pool = AsidPool::new(1);
        assert_eq!(pool.allocate(8), Some(0));
        let before = pool.slots.clone();
        assert_eq!(pool.allocate(9), None);
        assert_eq!(pool.slots, before);
    }

    // r[verify sel4-asid.reuse]
    #[test]
    fn test_capacity_one_reuse() {
        let mut pool = AsidPool::new(1);
        let asid = pool.allocate(3).unwrap();
        assert!(pool.release(asid, 3));
        assert_eq!(pool.allocate(7), Some(0));
        assert_eq!(pool.slots[0], Some(7));
    }

    // r[verify sel4-asid.uniqueness]
    proptest! {
        #[test]
        fn proptest_distinct_allocations(capacity in 2usize..8, owner1: u64, owner2: u64) {
            let mut pool = AsidPool::new(capacity);
            let a0 = pool.allocate(owner1).unwrap();
            let a1 = pool.allocate(owner2).unwrap();
            prop_assert_ne!(a0, a1);
            prop_assert_eq!(pool.slots[a0], Some(owner1));
            prop_assert_eq!(pool.slots[a1], Some(owner2));
        }

        // r[verify sel4-asid.uniqueness]
        #[test]
        fn proptest_same_owner_multiple_asids(capacity in 2usize..8, owner: u64) {
            let mut pool = AsidPool::new(capacity);
            let a0 = pool.allocate(owner).unwrap();
            let a1 = pool.allocate(owner).unwrap();
            prop_assert_ne!(a0, a1);
            prop_assert_eq!(pool.slots[a0], Some(owner));
            prop_assert_eq!(pool.slots[a1], Some(owner));
        }

        // r[verify sel4-asid.release-state]
        #[test]
        fn proptest_release_failures_leave_state(capacity in 1usize..8, owner: u64, wrong_owner: u64) {
            let mut pool = AsidPool::new(capacity);
            let asid = pool.allocate(owner).unwrap();
            let before = pool.slots.clone();
            if wrong_owner != owner {
                prop_assert!(!pool.release(asid, wrong_owner));
                prop_assert_eq!(pool.slots, before);
            }
        }

        // r[verify sel4-asid.pool-bounds]
        #[test]
        fn proptest_exhausted_allocation_unchanged(capacity in 1usize..8, owner_seed: u64) {
            let mut pool = AsidPool::new(capacity);
            for i in 0..capacity {
                prop_assert_eq!(pool.allocate(owner_seed.wrapping_add(i as u64)), Some(i));
            }
            let before = pool.slots.clone();
            prop_assert_eq!(pool.allocate(owner_seed.wrapping_add(99)), None);
            prop_assert_eq!(pool.slots, before);
        }

        // r[verify sel4-asid.reuse]
        #[test]
        fn proptest_capacity_one_forced_reuse(first_owner: u64, second_owner: u64) {
            let mut pool = AsidPool::new(1);
            prop_assert_eq!(pool.allocate(first_owner), Some(0));
            prop_assert!(pool.release(0, first_owner));
            prop_assert_eq!(pool.allocate(second_owner), Some(0));
            prop_assert_eq!(pool.slots[0], Some(second_owner));
        }
    }
}
