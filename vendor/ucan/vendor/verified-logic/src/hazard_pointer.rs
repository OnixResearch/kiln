//! Verified hazard pointer protocol for safe memory reclamation.
//!
//! Hazard pointers protect addresses from premature freeing. A thread
//! publishes the address it is accessing into a global hazard array.
//! Before reclaiming a retired address, the reclaimer scans all hazard
//! slots — any address found in the hazard array must not be freed.
//!
//! The core safety property: if an address is protected by any thread,
//! `can_reclaim` returns false, preventing use-after-free.

use vstd::prelude::*;

verus! {

/// Sentinel for "no address protected in this slot".
pub const HP_EMPTY: u64 = u64::MAX;

// --- is_protected ---

// r[depends hazard_pointer.is-protected]
/// An address is protected if it appears in any hazard slot.
/// HP_EMPTY slots are ignored.
pub open spec fn is_protected_spec(addr: u64, hazards: Seq<u64>) -> bool {
    exists|i: int| #![trigger hazards[i]]
        0 <= i < hazards.len() as int && hazards[i] == addr && addr != HP_EMPTY
}

// r[impl hazard_pointer.is-protected]
pub fn is_protected(addr: u64, hazards: &Vec<u64>) -> (result: bool)
    ensures result == is_protected_spec(addr, hazards@)
{
    if addr == HP_EMPTY {
        return false;
    }
    let mut i: usize = 0;
    while i < hazards.len()
        invariant
            0 <= i <= hazards.len(),
            addr != HP_EMPTY,
            forall|j: int| #![trigger hazards@[j]]
                0 <= j < i as int ==> hazards@[j] != addr,
        decreases hazards.len() - i,
    {
        if hazards[i] == addr {
            return true;
        }
        i += 1;
    }
    false
}

// --- can_reclaim ---

// r[depends hazard_pointer.can-reclaim]
/// An address can be reclaimed if it is NOT protected by any thread.
pub open spec fn can_reclaim_spec(addr: u64, hazards: Seq<u64>) -> bool {
    !is_protected_spec(addr, hazards)
}

// r[impl hazard_pointer.can-reclaim]
pub fn can_reclaim(addr: u64, hazards: &Vec<u64>) -> (result: bool)
    ensures result == can_reclaim_spec(addr, hazards@)
{
    !is_protected(addr, hazards)
}

// --- protect ---

// r[depends hazard_pointer.protect]
/// Publish an address into hazard slot `slot`. Returns updated hazards.
pub open spec fn protect_spec(hazards: Seq<u64>, slot: int, addr: u64) -> Seq<u64>
    recommends 0 <= slot < hazards.len() as int
{
    hazards.update(slot, addr)
}

// r[impl hazard_pointer.protect]
pub fn protect(hazards: &mut Vec<u64>, slot: usize, addr: u64)
    requires
        slot < old(hazards).len(),
    ensures
        hazards@ == protect_spec(old(hazards)@, slot as int, addr),
        hazards.len() == old(hazards).len(),
{
    hazards.set(slot, addr);
}

// --- unprotect ---

// r[depends hazard_pointer.unprotect]
/// Clear a hazard slot (set to HP_EMPTY). Returns updated hazards.
pub open spec fn unprotect_spec(hazards: Seq<u64>, slot: int) -> Seq<u64>
    recommends 0 <= slot < hazards.len() as int
{
    hazards.update(slot, HP_EMPTY)
}

// r[impl hazard_pointer.unprotect]
pub fn unprotect(hazards: &mut Vec<u64>, slot: usize)
    requires slot < old(hazards).len()
    ensures
        hazards@ == unprotect_spec(old(hazards)@, slot as int),
        hazards.len() == old(hazards).len(),
{
    hazards.set(slot, HP_EMPTY);
}

// --- count_reclaimable ---

// r[depends hazard_pointer.count-reclaimable]
/// Count how many addresses in `retired` can be reclaimed.
pub open spec fn count_reclaimable_from_spec(
    retired: Seq<u64>, hazards: Seq<u64>, from: int,
) -> nat
    decreases retired.len() - from
{
    if from >= retired.len() as int {
        0
    } else if can_reclaim_spec(retired[from], hazards) {
        1 + count_reclaimable_from_spec(retired, hazards, from + 1)
    } else {
        count_reclaimable_from_spec(retired, hazards, from + 1)
    }
}

// r[depends hazard_pointer.count-reclaimable]
pub open spec fn count_reclaimable_spec(retired: Seq<u64>, hazards: Seq<u64>) -> nat {
    count_reclaimable_from_spec(retired, hazards, 0)
}

// r[impl hazard_pointer.count-reclaimable]
pub fn count_reclaimable(retired: &Vec<u64>, hazards: &Vec<u64>) -> (result: u64)
    requires retired.len() <= u64::MAX as usize
    ensures result as nat == count_reclaimable_spec(retired@, hazards@)
{
    let mut count: u64 = 0;
    let mut i: usize = 0;
    while i < retired.len()
        invariant
            0 <= i <= retired.len(),
            retired.len() <= u64::MAX as usize,
            count as int <= i as int,
            count as nat == count_reclaimable_from_spec(retired@, hazards@, 0)
                - count_reclaimable_from_spec(retired@, hazards@, i as int),
        decreases retired.len() - i,
    {
        if can_reclaim(retired[i], hazards) {
            count = count + 1;
        }
        i += 1;
    }
    count
}

// --- is_slot_active ---

// r[depends hazard_pointer.slot-active]
/// A hazard slot is active (protecting something) when it is not HP_EMPTY.
pub open spec fn is_slot_active_spec(hazards: Seq<u64>, slot: int) -> bool
    recommends 0 <= slot < hazards.len() as int
{
    hazards[slot] != HP_EMPTY
}

// r[impl hazard_pointer.slot-active]
pub fn is_slot_active(hazards: &Vec<u64>, slot: usize) -> (result: bool)
    requires slot < hazards.len()
    ensures result == is_slot_active_spec(hazards@, slot as int)
{
    hazards[slot] != HP_EMPTY
}

// --- all_slots_empty ---

// r[depends hazard_pointer.all-empty]
/// All hazard slots are empty (no address is protected).
pub open spec fn all_slots_empty_spec(hazards: Seq<u64>) -> bool {
    forall|i: int| #![trigger hazards[i]]
        0 <= i < hazards.len() as int ==> hazards[i] == HP_EMPTY
}

// r[impl hazard_pointer.all-empty]
pub fn all_slots_empty(hazards: &Vec<u64>) -> (result: bool)
    ensures result == all_slots_empty_spec(hazards@)
{
    let mut i: usize = 0;
    while i < hazards.len()
        invariant
            0 <= i <= hazards.len(),
            forall|j: int| #![trigger hazards@[j]]
                0 <= j < i as int ==> hazards@[j] == HP_EMPTY,
        decreases hazards.len() - i,
    {
        if hazards[i] != HP_EMPTY {
            return false;
        }
        i += 1;
    }
    true
}

// === Proofs ===

// r[verify hazard_pointer.is-protected]
/// HP_EMPTY is never protected.
proof fn empty_sentinel_not_protected(hazards: Seq<u64>)
    ensures !is_protected_spec(HP_EMPTY, hazards)
{}

// r[verify hazard_pointer.is-protected]
/// Empty hazard list protects nothing.
proof fn no_hazards_no_protection(addr: u64)
    ensures !is_protected_spec(addr, Seq::<u64>::empty())
{}

// r[verify hazard_pointer.can-reclaim]
/// Empty hazard list means everything is reclaimable.
proof fn empty_hazards_all_reclaimable(addr: u64)
    ensures can_reclaim_spec(addr, Seq::<u64>::empty())
{}

// r[verify hazard_pointer.can-reclaim]
/// HP_EMPTY is always reclaimable.
proof fn sentinel_always_reclaimable(hazards: Seq<u64>)
    ensures can_reclaim_spec(HP_EMPTY, hazards)
{}

// r[verify hazard_pointer.protect]
/// After protecting an address, it is protected.
proof fn protect_makes_protected(hazards: Seq<u64>, slot: int, addr: u64)
    requires
        0 <= slot < hazards.len() as int,
        addr != HP_EMPTY,
    ensures is_protected_spec(addr, protect_spec(hazards, slot, addr))
{
    let new_hazards = protect_spec(hazards, slot, addr);
    assert(new_hazards[slot] == addr);
}

// r[verify hazard_pointer.protect]
/// Protecting preserves protection of other addresses in other slots.
proof fn protect_preserves_others(
    hazards: Seq<u64>, slot: int, addr: u64, other_slot: int,
)
    requires
        0 <= slot < hazards.len() as int,
        0 <= other_slot < hazards.len() as int,
        slot != other_slot,
    ensures protect_spec(hazards, slot, addr)[other_slot] == hazards[other_slot]
{}

// r[verify hazard_pointer.unprotect]
/// After unprotecting, the slot is inactive.
proof fn unprotect_clears_slot(hazards: Seq<u64>, slot: int)
    requires 0 <= slot < hazards.len() as int
    ensures !is_slot_active_spec(unprotect_spec(hazards, slot), slot)
{}

// r[verify hazard_pointer.protect]
// r[verify hazard_pointer.unprotect]
/// Protect then unprotect same slot: other slots unchanged.
proof fn protect_unprotect_other_slots(
    hazards: Seq<u64>, slot: int, addr: u64, other_slot: int,
)
    requires
        0 <= slot < hazards.len() as int,
        0 <= other_slot < hazards.len() as int,
        slot != other_slot,
    ensures
        unprotect_spec(protect_spec(hazards, slot, addr), slot)[other_slot]
            == hazards[other_slot]
{}

// r[verify hazard_pointer.protect]
// r[verify hazard_pointer.can-reclaim]
/// Protected address cannot be reclaimed (the core safety property).
proof fn protected_cannot_be_reclaimed(addr: u64, hazards: Seq<u64>)
    requires is_protected_spec(addr, hazards)
    ensures !can_reclaim_spec(addr, hazards)
{}

// r[verify hazard_pointer.can-reclaim]
// r[verify hazard_pointer.is-protected]
/// can_reclaim and is_protected are complementary (for non-sentinel).
proof fn reclaim_protected_complement(addr: u64, hazards: Seq<u64>)
    requires addr != HP_EMPTY
    ensures can_reclaim_spec(addr, hazards) == !is_protected_spec(addr, hazards)
{}

// r[verify hazard_pointer.all-empty]
/// All empty means nothing is protected.
proof fn all_empty_nothing_protected(hazards: Seq<u64>, addr: u64)
    requires all_slots_empty_spec(hazards)
    ensures !is_protected_spec(addr, hazards)
{}

// r[verify hazard_pointer.all-empty]
/// All empty means everything is reclaimable.
proof fn all_empty_all_reclaimable(hazards: Seq<u64>, addr: u64)
    requires all_slots_empty_spec(hazards)
    ensures can_reclaim_spec(addr, hazards)
{
    all_empty_nothing_protected(hazards, addr);
}

// r[verify hazard_pointer.protect]
// r[verify hazard_pointer.unprotect]
/// Protect-unprotect roundtrip on same slot restores HP_EMPTY.
proof fn protect_unprotect_roundtrip(hazards: Seq<u64>, slot: int, addr: u64)
    requires 0 <= slot < hazards.len() as int
    ensures unprotect_spec(protect_spec(hazards, slot, addr), slot)[slot] == HP_EMPTY
{}

// r[verify hazard_pointer.slot-active]
/// Freshly created slot (HP_EMPTY) is inactive.
proof fn empty_slot_inactive(hazards: Seq<u64>, slot: int)
    requires
        0 <= slot < hazards.len() as int,
        hazards[slot] == HP_EMPTY,
    ensures !is_slot_active_spec(hazards, slot)
{}

// r[verify hazard_pointer.all-empty]
/// Empty hazard array has all slots empty.
proof fn empty_array_all_empty()
    ensures all_slots_empty_spec(Seq::<u64>::empty())
{}

// r[verify hazard_pointer.all-empty]
/// Single HP_EMPTY has all slots empty.
proof fn singleton_empty_all_empty()
    ensures all_slots_empty_spec(seq![HP_EMPTY])
{}

} // verus!

#[cfg(test)]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::default_numeric_fallback
)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify hazard_pointer.is-protected]
    #[test]
    fn test_is_protected() {
        let hazards = vec![10u64, HP_EMPTY, 20, HP_EMPTY];
        assert!(is_protected(10, &hazards));
        assert!(is_protected(20, &hazards));
        assert!(!is_protected(30, &hazards));
        assert!(!is_protected(HP_EMPTY, &hazards));
    }

    // r[verify hazard_pointer.can-reclaim]
    #[test]
    fn test_can_reclaim() {
        let hazards = vec![10u64, HP_EMPTY, 20];
        assert!(!can_reclaim(10, &hazards));
        assert!(!can_reclaim(20, &hazards));
        assert!(can_reclaim(30, &hazards));
        assert!(can_reclaim(HP_EMPTY, &hazards));
    }

    // r[verify hazard_pointer.protect]
    #[test]
    fn test_protect() {
        let mut hazards = vec![HP_EMPTY, HP_EMPTY, HP_EMPTY];
        protect(&mut hazards, 1, 42);
        assert_eq!(hazards[0], HP_EMPTY);
        assert_eq!(hazards[1], 42);
        assert_eq!(hazards[2], HP_EMPTY);
        assert!(is_protected(42, &hazards));
    }

    // r[verify hazard_pointer.unprotect]
    #[test]
    fn test_unprotect() {
        let mut hazards = vec![10u64, 20, 30];
        unprotect(&mut hazards, 1);
        assert_eq!(hazards[1], HP_EMPTY);
        assert!(is_protected(10, &hazards));
        assert!(!is_protected(20, &hazards));
        assert!(is_protected(30, &hazards));
    }

    // r[verify hazard_pointer.protect]
    // r[verify hazard_pointer.unprotect]
    #[test]
    fn test_protect_unprotect_roundtrip() {
        let mut hazards = vec![HP_EMPTY, HP_EMPTY];
        protect(&mut hazards, 0, 99);
        assert!(is_protected(99, &hazards));
        unprotect(&mut hazards, 0);
        assert!(!is_protected(99, &hazards));
    }

    // r[verify hazard_pointer.count-reclaimable]
    #[test]
    fn test_count_reclaimable() {
        let hazards = vec![10u64, 20];
        let retired = vec![10u64, 30, 40, 20];
        assert_eq!(count_reclaimable(&retired, &hazards), 2); // 30 and 40
    }

    // r[verify hazard_pointer.slot-active]
    #[test]
    fn test_is_slot_active() {
        let hazards = vec![42u64, HP_EMPTY, 99];
        assert!(is_slot_active(&hazards, 0));
        assert!(!is_slot_active(&hazards, 1));
        assert!(is_slot_active(&hazards, 2));
    }

    // r[verify hazard_pointer.all-empty]
    #[test]
    fn test_all_slots_empty() {
        let empty: Vec<u64> = Vec::new();
        assert!(all_slots_empty(&empty));

        let all_empty = vec![HP_EMPTY, HP_EMPTY, HP_EMPTY];
        assert!(all_slots_empty(&all_empty));

        let one_active = vec![HP_EMPTY, 42, HP_EMPTY];
        assert!(!all_slots_empty(&one_active));
    }

    proptest! {
        // r[verify hazard_pointer.is-protected]
        #[test]
        fn prop_sentinel_not_protected(
            hazards in proptest::collection::vec(any::<u64>(), 0..8)
        ) {
            prop_assert!(!is_protected(HP_EMPTY, &hazards));
        }

        // r[verify hazard_pointer.can-reclaim]
        #[test]
        fn prop_sentinel_always_reclaimable(
            hazards in proptest::collection::vec(any::<u64>(), 0..8)
        ) {
            prop_assert!(can_reclaim(HP_EMPTY, &hazards));
        }

        // r[verify hazard_pointer.can-reclaim]
        #[test]
        fn prop_empty_hazards_all_reclaimable(addr: u64) {
            let empty: Vec<u64> = Vec::new();
            prop_assert!(can_reclaim(addr, &empty));
        }

        // r[verify hazard_pointer.protect]
        // r[verify hazard_pointer.is-protected]
        #[test]
        fn prop_protect_makes_protected(
            len in 1usize..8,
            slot_frac in 0.0f64..1.0,
            addr in 0u64..u64::MAX // exclude HP_EMPTY
        ) {
            let slot = (slot_frac * (len - 1) as f64) as usize;
            let mut hazards = vec![HP_EMPTY; len];
            protect(&mut hazards, slot, addr);
            prop_assert!(is_protected(addr, &hazards));
        }

        // r[verify hazard_pointer.protect]
        // r[verify hazard_pointer.can-reclaim]
        #[test]
        fn prop_protected_not_reclaimable(
            len in 1usize..8,
            slot_frac in 0.0f64..1.0,
            addr in 0u64..u64::MAX
        ) {
            let slot = (slot_frac * (len - 1) as f64) as usize;
            let mut hazards = vec![HP_EMPTY; len];
            protect(&mut hazards, slot, addr);
            prop_assert!(!can_reclaim(addr, &hazards));
        }

        // r[verify hazard_pointer.protect]
        // r[verify hazard_pointer.unprotect]
        #[test]
        fn prop_protect_unprotect_roundtrip(
            len in 1usize..8,
            slot_frac in 0.0f64..1.0,
            addr in 0u64..u64::MAX
        ) {
            let slot = (slot_frac * (len - 1) as f64) as usize;
            let mut hazards = vec![HP_EMPTY; len];
            protect(&mut hazards, slot, addr);
            prop_assert!(is_protected(addr, &hazards));
            unprotect(&mut hazards, slot);
            // After unprotecting the only slot with addr, it should be reclaimable
            prop_assert!(can_reclaim(addr, &hazards));
        }

        // r[verify hazard_pointer.all-empty]
        #[test]
        fn prop_all_empty_nothing_protected(
            len in 0usize..8,
            addr: u64
        ) {
            let hazards = vec![HP_EMPTY; len];
            prop_assert!(all_slots_empty(&hazards));
            prop_assert!(!is_protected(addr, &hazards));
            prop_assert!(can_reclaim(addr, &hazards));
        }

        // r[verify hazard_pointer.count-reclaimable]
        #[test]
        fn prop_count_reclaimable_bounded(
            retired in proptest::collection::vec(0u64..100, 0..8),
            hazards in proptest::collection::vec(0u64..100, 0..8)
        ) {
            let count = count_reclaimable(&retired, &hazards);
            prop_assert!(count as usize <= retired.len());
        }

        // r[verify hazard_pointer.count-reclaimable]
        #[test]
        fn prop_empty_hazards_all_retired_reclaimable(
            retired in proptest::collection::vec(0u64..u64::MAX, 0..8)
        ) {
            let empty: Vec<u64> = Vec::new();
            let count = count_reclaimable(&retired, &empty);
            prop_assert_eq!(count as usize, retired.len());
        }
    }
}
