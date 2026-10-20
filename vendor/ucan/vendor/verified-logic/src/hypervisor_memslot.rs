//! Verified KVM-style memslot tables.
//!
//! A memslot maps a contiguous range of guest frame numbers (GFNs) to a
//! host-backed region.  A memslot table is valid when its entries are sorted
//! by `base_gfn` and pairwise disjoint.  `lookup_memslot` returns the unique
//! slot containing a given GFN, or `None`.

use vstd::prelude::*;

verus! {

// ────────────────────────────────────────────────────────────────────
//  Memslot representation
// ────────────────────────────────────────────────────────────────────

/// A single memslot: guest frame range `[base_gfn, base_gfn + npages)`
/// backed by host memory starting at `host_base`, with a readonly flag.
pub struct Memslot {
    pub base_gfn: u64,
    pub npages:   u64,
    pub host_base: u64,
    pub readonly: bool,
}

// ────────────────────────────────────────────────────────────────────
//  slot_contains_gfn
// ────────────────────────────────────────────────────────────────────

// r[depends hypervisor-memslot.lookup]
/// Spec: GFN is inside this slot's range.
pub open spec fn slot_contains_gfn_spec(slot: Memslot, gfn: u64) -> bool {
    slot.base_gfn as int <= gfn as int
        && (gfn as int) < slot.base_gfn as int + slot.npages as int
}

// r[impl hypervisor-memslot.lookup]
/// Returns true when `gfn` is in `[base_gfn, base_gfn + npages)`.
pub fn slot_contains_gfn(slot: &Memslot, gfn: u64) -> (result: bool)
    requires slot.base_gfn as int + slot.npages as int <= u64::MAX as int
    ensures  result == slot_contains_gfn_spec(*slot, gfn)
{
    gfn >= slot.base_gfn && gfn < slot.base_gfn + slot.npages
}

// ────────────────────────────────────────────────────────────────────
//  slot_end_gfn
// ────────────────────────────────────────────────────────────────────

/// Spec: exclusive end GFN.
pub open spec fn slot_end_gfn_spec(slot: Memslot) -> int {
    slot.base_gfn as int + slot.npages as int
}

// r[impl hypervisor-memslot.lookup]
/// Returns the exclusive end GFN.
pub fn slot_end_gfn(slot: &Memslot) -> (result: u64)
    requires slot.base_gfn as int + slot.npages as int <= u64::MAX as int
    ensures  result as int == slot_end_gfn_spec(*slot)
{
    slot.base_gfn + slot.npages
}

// ────────────────────────────────────────────────────────────────────
//  slots_disjoint  (pairwise predicate)
// ────────────────────────────────────────────────────────────────────

// r[depends hypervisor-memslot.disjoint]
/// Spec: two memslots share no GFN.
pub open spec fn slots_disjoint_spec(a: Memslot, b: Memslot) -> bool {
    a.npages == 0 || b.npages == 0
    || slot_end_gfn_spec(a) <= b.base_gfn as int
    || slot_end_gfn_spec(b) <= a.base_gfn as int
}

// r[verify hypervisor-memslot.disjoint]
/// Disjointness is symmetric.
proof fn slots_disjoint_symmetric(a: Memslot, b: Memslot)
    ensures slots_disjoint_spec(a, b) == slots_disjoint_spec(b, a)
{}

// ────────────────────────────────────────────────────────────────────
//  slots_sorted  (adjacent-pair ordering predicate)
// ────────────────────────────────────────────────────────────────────

// r[depends hypervisor-memslot.disjoint]
/// Spec: slot at index `i` ends at or before the next slot starts.
pub open spec fn pair_sorted_spec(slots: Seq<Memslot>, i: int) -> bool
    recommends 0 <= i < slots.len() - 1
{
    slot_end_gfn_spec(slots[i]) <= slots[i + 1].base_gfn as int
}

/// Spec: the whole table is sorted (each pair in order).
pub open spec fn slots_sorted_spec(slots: Seq<Memslot>) -> bool {
    forall|i: int| 0 <= i < slots.len() - 1 ==> pair_sorted_spec(slots, i)
}

// ────────────────────────────────────────────────────────────────────
//  table_valid — sorted + disjoint + bounded
// ────────────────────────────────────────────────────────────────────

/// Spec: each slot's GFN range fits in u64.
pub open spec fn slots_bounded_spec(slots: Seq<Memslot>) -> bool {
    forall|i: int| 0 <= i < slots.len() as int ==>
        slots[i].base_gfn as int + slots[i].npages as int <= u64::MAX as int
}

// r[depends hypervisor-memslot.disjoint]
/// Spec: a valid memslot table is sorted, bounded, and therefore pairwise
/// disjoint.
pub open spec fn table_valid_spec(slots: Seq<Memslot>) -> bool {
    slots_sorted_spec(slots) && slots_bounded_spec(slots)
}

// r[verify hypervisor-memslot.disjoint]
/// In a valid table, slot_end_gfn(slots[i]) <= slots[j].base_gfn for i < j.
/// This directly implies disjointness.
pub proof fn sorted_end_le_base(slots: Seq<Memslot>, i: int, j: int)
    requires
        table_valid_spec(slots),
        0 <= i < j,
        j < slots.len() as int,
    ensures
        slot_end_gfn_spec(slots[i]) <= slots[j].base_gfn as int
    decreases j - i
{
    // Base: adjacent pair.
    assert(pair_sorted_spec(slots, i));
    // end(i) <= base(i+1)
    if j == i + 1 {
        // Done: pair_sorted gives us exactly this.
    } else {
        // Induction: end(i+1) <= base(j)
        sorted_end_le_base(slots, i + 1, j);
        // Chain: end(i) <= base(i+1) <= end(i+1) <= base(j)
        // base(i+1) <= end(i+1) because npages >= 0.
    }
}

// r[verify hypervisor-memslot.disjoint]
/// In a valid table, any two distinct slots are disjoint.
pub proof fn sorted_implies_pairwise_disjoint(slots: Seq<Memslot>, i: int, j: int)
    requires
        table_valid_spec(slots),
        0 <= i < j,
        j < slots.len() as int,
    ensures
        slots_disjoint_spec(slots[i], slots[j])
{
    sorted_end_le_base(slots, i, j);
}

// ────────────────────────────────────────────────────────────────────
//  table_valid exec checker
// ────────────────────────────────────────────────────────────────────

// r[impl hypervisor-memslot.disjoint]
/// Validates that a memslot table is sorted and bounded.
pub fn table_valid(slots: &Vec<Memslot>) -> (result: bool)
    ensures result ==> table_valid_spec(slots@)
{
    let len = slots.len();
    if len == 0 {
        return true;
    }
    // Check the first slot is bounded.
    if slots[0].base_gfn as u64 > u64::MAX - slots[0].npages {
        return false;
    }
    let mut i: usize = 1;
    while i < len
        invariant
            1 <= i <= len,
            len == slots@.len(),
            forall|k: int| 0 <= k < i as int ==>
                slots@[k].base_gfn as int + slots@[k].npages as int <= u64::MAX as int,
            forall|k: int| 0 <= k < i as int - 1 ==>
                pair_sorted_spec(slots@, k),
        decreases len - i,
    {
        if slots[i].base_gfn as u64 > u64::MAX - slots[i].npages {
            return false;
        }
        let prev_end = slots[i - 1].base_gfn + slots[i - 1].npages;
        if prev_end > slots[i].base_gfn {
            return false;
        }
        i += 1;
    }
    true
}

// ────────────────────────────────────────────────────────────────────
//  lookup_memslot — spec
// ────────────────────────────────────────────────────────────────────

/// Recursive spec: scan from index `from` for a slot containing `gfn`.
pub open spec fn lookup_from_spec(slots: Seq<Memslot>, gfn: u64, from: int) -> Option<usize>
    decreases slots.len() - from
{
    if from < 0 || from >= slots.len() {
        None
    } else if slot_contains_gfn_spec(slots[from], gfn) {
        Some(from as usize)
    } else {
        lookup_from_spec(slots, gfn, from + 1)
    }
}

// r[depends hypervisor-memslot.lookup]
/// Spec: lookup starting from index 0.
pub open spec fn lookup_memslot_spec(slots: Seq<Memslot>, gfn: u64) -> Option<usize> {
    lookup_from_spec(slots, gfn, 0)
}

// r[verify hypervisor-memslot.lookup]
/// If lookup returns Some(idx), the slot at idx contains the gfn.
/// Helper: lookup_from returning Some(idx) means idx is valid and the slot
/// contains gfn. Proved by returning the idx explicitly as usize.
pub proof fn lookup_from_hit_contains(slots: Seq<Memslot>, gfn: u64, from: int)
    -> (idx: usize)
    requires
        0 <= from,
        from < slots.len(),
        slots.len() <= usize::MAX as int,
        lookup_from_spec(slots, gfn, from) is Some,
    ensures
        lookup_from_spec(slots, gfn, from) == Some(idx),
        (idx as int) < slots.len(),
        slot_contains_gfn_spec(slots[idx as int], gfn),
    decreases slots.len() - from
{
    if slot_contains_gfn_spec(slots[from], gfn) {
        from as usize
    } else if from + 1 >= slots.len() {
        assert(lookup_from_spec(slots, gfn, from + 1) is None);
        from as usize // unreachable
    } else {
        lookup_from_hit_contains(slots, gfn, from + 1)
    }
}

// r[verify hypervisor-memslot.lookup]
/// In a valid table, lookup is unique: if gfn is in slot j, lookup returns j.
proof fn lookup_unique_in_valid_table(slots: Seq<Memslot>, gfn: u64, j: int)
    requires
        table_valid_spec(slots),
        0 <= j < slots.len() as int,
        slot_contains_gfn_spec(slots[j], gfn),
    ensures
        lookup_memslot_spec(slots, gfn) == Some(j as usize)
    decreases j
{
    // We prove by strong induction that lookup_from_spec(slots, gfn, 0)
    // reaches j without hitting any earlier slot.
    // In a valid table, no earlier slot can contain gfn (pairwise disjoint).
    lookup_from_skips_to(slots, gfn, 0, j);
}

/// Disjoint slots cannot both contain the same gfn.
proof fn disjoint_no_shared_gfn(a: Memslot, b: Memslot, gfn: u64)
    requires
        slot_contains_gfn_spec(b, gfn),
        slot_end_gfn_spec(a) <= b.base_gfn as int,
    ensures
        !slot_contains_gfn_spec(a, gfn)
{
    // b contains gfn: base_b <= gfn < end_b.
    // end_a <= base_b, so end_a <= gfn. But slot_contains requires gfn < end_a.
}

/// Helper: in a valid table, lookup_from starting at `from` where `from <= j`
/// and slot[j] contains gfn, will skip all slots before j and return j.
pub proof fn lookup_from_skips_to(slots: Seq<Memslot>, gfn: u64, from: int, j: int)
    requires
        table_valid_spec(slots),
        0 <= from <= j,
        j < slots.len() as int,
        slot_contains_gfn_spec(slots[j], gfn),
    ensures
        lookup_from_spec(slots, gfn, from) == Some(j as usize)
    decreases j - from
{
    if from == j {
    } else {
        // end(from) <= base(j) from sorted order.
        sorted_end_le_base(slots, from, j);
        // Therefore slots[from] cannot contain gfn.
        disjoint_no_shared_gfn(slots[from], slots[j], gfn);
        lookup_from_skips_to(slots, gfn, from + 1, j);
    }
}

/// If lookup_memslot_spec returns Some(idx), slot idx contains gfn.
pub proof fn lookup_hit_implies_contains(slots: Seq<Memslot>, gfn: u64, idx: usize)
    requires
        table_valid_spec(slots),
        slots.len() <= usize::MAX as int,
        lookup_memslot_spec(slots, gfn) == Some(idx),
    ensures
        0 <= idx as int,
        (idx as int) < slots.len(),
        slot_contains_gfn_spec(slots[idx as int], gfn),
{
    assert(lookup_from_spec(slots, gfn, 0) is Some);
    assert(slots.len() > 0);
    let pi = lookup_from_hit_contains(slots, gfn, 0);
    // pi: usize.  lookup_from_spec(..., 0) == Some(pi).
    // Precondition: lookup_memslot_spec(...) == Some(idx).
    // lookup_memslot_spec == lookup_from_spec(..., 0).
    // So Some(pi) == Some(idx), hence pi == idx.
}

// r[verify hypervisor-memslot.lookup]
/// Lookup miss: if no slot contains gfn, lookup returns None.
pub proof fn lookup_miss_when_no_slot(slots: Seq<Memslot>, gfn: u64, from: int)
    requires
        0 <= from,
        forall|k: int| from <= k < slots.len() as int ==>
            !slot_contains_gfn_spec(slots[k], gfn),
    ensures
        lookup_from_spec(slots, gfn, from) == None::<usize>
    decreases slots.len() - from
{
    if from >= slots.len() {
    } else {
        lookup_miss_when_no_slot(slots, gfn, from + 1);
    }
}

// ────────────────────────────────────────────────────────────────────
//  lookup_memslot — exec (linear scan)
// ────────────────────────────────────────────────────────────────────

// r[impl hypervisor-memslot.lookup]
/// Finds the unique memslot containing `gfn`, or returns `None`.
pub fn lookup_memslot(slots: &Vec<Memslot>, gfn: u64) -> (result: Option<usize>)
    requires
        table_valid_spec(slots@),
    ensures
        result == lookup_memslot_spec(slots@, gfn),
{
    let len = slots.len();
    let mut i: usize = 0;
    while i < len
        invariant
            0 <= i <= len,
            len == slots@.len(),
            table_valid_spec(slots@),
            // Haven't found gfn in any slot before i.
            forall|k: int| 0 <= k < i as int ==>
                !slot_contains_gfn_spec(slots@[k], gfn),
        decreases len - i,
    {
        if slot_contains_gfn(&slots[i], gfn) {
            proof {
                lookup_from_skips_to(slots@, gfn, 0, i as int);
            }
            return Some(i);
        }
        // Early exit: if this slot starts after gfn, all later slots do too.
        if slots[i].base_gfn > gfn {
            proof {
                assert forall|k: int| i as int <= k < slots@.len() as int implies
                    !slot_contains_gfn_spec(slots@[k], gfn)
                by {
                    if k == i as int {
                        // base_gfn > gfn, so slot i doesn't contain gfn.
                    } else {
                        // end(i) <= base(k) from sorted_end_le_base.
                        // base(i) <= end(i) since npages >= 0.
                        // So base(i) <= base(k), and base(i) > gfn,
                        // meaning base(k) > gfn, so slot k doesn't contain gfn.
                        sorted_end_le_base(slots@, i as int, k);
                    }
                }
                lookup_miss_when_no_slot(slots@, gfn, 0);
            }
            return None;
        }
        i += 1;
    }
    proof {
        lookup_miss_when_no_slot(slots@, gfn, 0);
    }
    None
}

// ────────────────────────────────────────────────────────────────────
//  slot_flags_preserved — registration check
// ────────────────────────────────────────────────────────────────────

// r[depends hypervisor-memslot.disjoint]
/// Spec: two memslots have the same configuration (ignoring host_base).
pub open spec fn slot_config_eq_spec(a: Memslot, b: Memslot) -> bool {
    a.base_gfn == b.base_gfn
    && a.npages == b.npages
    && a.readonly == b.readonly
}

// r[verify hypervisor-memslot.disjoint]
/// Config equality is reflexive.
proof fn slot_config_eq_reflexive(s: Memslot)
    ensures slot_config_eq_spec(s, s)
{}

} // verus!

// ────────────────────────────────────────────────────────────────────
//  Tests
// ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(base_gfn: u64, npages: u64, host_base: u64, readonly: bool) -> Memslot {
        Memslot {
            base_gfn,
            npages,
            host_base,
            readonly,
        }
    }

    // r[test hypervisor-memslot.disjoint]
    #[test]
    fn test_adjacent_slots_valid() {
        let slots = vec![mk(0, 16, 0x1000, false), mk(16, 16, 0x2000, false)];
        assert!(table_valid(&slots));
    }

    // r[test hypervisor-memslot.disjoint]
    #[test]
    fn test_overlapping_slots_invalid() {
        let slots = vec![mk(0, 16, 0x1000, false), mk(15, 5, 0x2000, false)];
        assert!(!table_valid(&slots));
    }

    // r[test hypervisor-memslot.disjoint]
    #[test]
    fn test_empty_table_valid() {
        let slots: Vec<Memslot> = vec![];
        assert!(table_valid(&slots));
    }

    // r[test hypervisor-memslot.disjoint]
    #[test]
    fn test_single_slot_valid() {
        let slots = vec![mk(100, 50, 0x5000, false)];
        assert!(table_valid(&slots));
    }

    // r[test hypervisor-memslot.lookup]
    #[test]
    fn test_lookup_hit() {
        let slots = vec![mk(0, 64, 0x1000, false), mk(64, 64, 0x2000, false)];
        assert!(table_valid(&slots));
        assert_eq!(lookup_memslot(&slots, 96), Some(1));
    }

    // r[test hypervisor-memslot.lookup]
    #[test]
    fn test_lookup_miss() {
        let slots = vec![mk(0, 64, 0x1000, false), mk(128, 64, 0x2000, false)];
        assert!(table_valid(&slots));
        assert_eq!(lookup_memslot(&slots, 100), None);
    }

    // r[test hypervisor-memslot.lookup]
    #[test]
    fn test_lookup_first_gfn() {
        let slots = vec![mk(10, 5, 0x1000, false)];
        assert!(table_valid(&slots));
        assert_eq!(lookup_memslot(&slots, 10), Some(0));
    }

    // r[test hypervisor-memslot.lookup]
    #[test]
    fn test_lookup_last_gfn() {
        let slots = vec![mk(10, 5, 0x1000, false)];
        assert!(table_valid(&slots));
        assert_eq!(lookup_memslot(&slots, 14), Some(0));
        // 15 is past the end
        assert_eq!(lookup_memslot(&slots, 15), None);
    }

    // r[test hypervisor-memslot.disjoint]
    #[test]
    fn test_unsorted_table_invalid() {
        let slots = vec![mk(64, 16, 0x2000, false), mk(0, 16, 0x1000, false)];
        assert!(!table_valid(&slots));
    }

    // r[test hypervisor-memslot.lookup]
    #[test]
    fn test_lookup_empty_table() {
        let slots: Vec<Memslot> = vec![];
        assert_eq!(lookup_memslot(&slots, 42), None);
    }
}
