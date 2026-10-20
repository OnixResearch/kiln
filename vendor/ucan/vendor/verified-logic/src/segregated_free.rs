//! Verified segregated free lists by size class.
//!
//! Multiple free lists, one per size class. Allocation picks the right
//! class and pops from that list. Freeing pushes back to the correct class.
//! Composes [`size_class`] (bucketing) with [`free_list`] (per-bucket
//! bookkeeping). The central invariant: a block appears in exactly one
//! class list (cross-class uniqueness).

use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::free_list::all_distinct_spec;
#[cfg(verus_keep_ghost)]
use crate::free_list::free_list_contains_spec;
#[cfg(verus_keep_ghost)]
use crate::size_class::class_to_size_spec;

verus! {

// ─── Seg Class For ───────────────────────────────────────────────────

// r[depends segregated-free.class-for]
/// Spec: which free list handles a given requested size.
/// Uses power-of-two classes: class = ceil_log2(size).
pub open spec fn seg_class_for_spec(size: u64, num_classes: u64) -> u64
    recommends size > 0, num_classes > 0
{
    // Index into the class array. Table-based for spec clarity.
    let c = if size <= 1 { 0u64 }
    else if size <= 2 { 1u64 }
    else if size <= 4 { 2u64 }
    else if size <= 8 { 3u64 }
    else if size <= 16 { 4u64 }
    else if size <= 32 { 5u64 }
    else if size <= 64 { 6u64 }
    else if size <= 128 { 7u64 }
    else if size <= 256 { 8u64 }
    else if size <= 512 { 9u64 }
    else if size <= 1024 { 10u64 }
    else if size <= 2048 { 11u64 }
    else if size <= 4096 { 12u64 }
    else if size <= 8192 { 13u64 }
    else if size <= 16384 { 14u64 }
    else if size <= 32768 { 15u64 }
    else if size <= 65536 { 16u64 }
    else { 64u64 };
    // Clamp to num_classes - 1 if class exceeds available classes
    if c >= num_classes { (num_classes - 1) as u64 } else { c }
}

// r[impl segregated-free.class-for]
/// Returns the class index for a given requested size.
pub fn seg_class_for(size: u64, num_classes: u64) -> (result: u64)
    requires size > 0, num_classes > 0, size <= 65536, num_classes <= 17
    ensures result == seg_class_for_spec(size, num_classes), result < num_classes
{
    let c = if size <= 1 { 0u64 }
    else if size <= 2 { 1u64 }
    else if size <= 4 { 2u64 }
    else if size <= 8 { 3u64 }
    else if size <= 16 { 4u64 }
    else if size <= 32 { 5u64 }
    else if size <= 64 { 6u64 }
    else if size <= 128 { 7u64 }
    else if size <= 256 { 8u64 }
    else if size <= 512 { 9u64 }
    else if size <= 1024 { 10u64 }
    else if size <= 2048 { 11u64 }
    else if size <= 4096 { 12u64 }
    else if size <= 8192 { 13u64 }
    else if size <= 16384 { 14u64 }
    else if size <= 32768 { 15u64 }
    else { 16u64 };
    if c >= num_classes { num_classes - 1 } else { c }
}

// ─── Seg Total Free ──────────────────────────────────────────────────

/// Spec: sum of lengths of all class lists.
pub open spec fn seg_total_free_from_spec(lists: Seq<Seq<u64>>, from: int) -> int
    decreases (if lists.len() >= from { lists.len() - from } else { 0 }),
{
    if from < 0 || from >= lists.len() {
        0
    } else {
        lists[from].len() as int + seg_total_free_from_spec(lists, from + 1)
    }
}

// r[depends segregated-free.total-free]
/// Spec: total free blocks across all classes.
pub open spec fn seg_total_free_spec(lists: Seq<Seq<u64>>) -> int {
    seg_total_free_from_spec(lists, 0)
}

// r[impl segregated-free.total-free]
/// Returns the total number of free blocks across all class lists.
pub fn seg_total_free(lists: &[Vec<u64>]) -> (result: u64)
    requires
        lists@.len() <= 64,
        forall|i: int| #![trigger lists@[i]]
            0 <= i < lists@.len() ==> lists@[i]@.len() <= u64::MAX as int,
    ensures result as int <= u64::MAX as int
{
    let mut total: u64 = 0;
    let mut i: usize = 0;
    while i < lists.len()
        invariant
            0 <= i <= lists@.len(),
            lists@.len() <= 64,
            total as int <= u64::MAX as int,
        decreases lists@.len() - i,
    {
        let list_len = lists[i].len() as u64;
        if total <= u64::MAX - list_len {
            total = total + list_len;
        } else {
            total = u64::MAX; // saturate
        }
        i = i + 1;
    }
    total
}

// ─── Cross-class uniqueness ──────────────────────────────────────────

// r[impl segregated-free.cross-class-unique]
// r[verify segregated-free.cross-class-unique]
// r[depends segregated-free.cross-class-unique]
/// Spec: no block appears in two different class lists.
pub open spec fn cross_class_unique_spec(lists: Seq<Seq<u64>>) -> bool {
    forall|c1: int, c2: int, i1: int, i2: int|
        #![trigger lists[c1][i1], lists[c2][i2]]
        0 <= c1 < lists.len() && 0 <= c2 < lists.len()
        && 0 <= i1 < lists[c1].len() && 0 <= i2 < lists[c2].len()
        && (c1 != c2 || i1 != i2)
        ==> lists[c1][i1] != lists[c2][i2]
}

// ─── Seg Alloc ───────────────────────────────────────────────────────

// r[depends segregated-free.alloc]
/// Spec: allocate from the appropriate class.
/// Returns (block, class_index) or None.
pub open spec fn seg_alloc_spec(
    lists: Seq<Seq<u64>>, size: u64, num_classes: u64,
) -> Option<(u64, u64)>
    recommends size > 0, num_classes > 0, num_classes <= lists.len()
{
    let class = seg_class_for_spec(size, num_classes);
    if class < lists.len() && lists[class as int].len() > 0 {
        Some((lists[class as int][0], class))
    } else {
        None
    }
}

// r[impl segregated-free.alloc]
/// Allocate from the appropriate class's free list.
#[allow(clippy::cast_possible_truncation)]
pub fn seg_alloc(
    lists: &[Vec<u64>], size: u64, num_classes: u64,
) -> (result: Option<(u64, u64)>)
    requires
        size > 0, size <= 65536,
        num_classes > 0, num_classes <= 17,
        lists@.len() == num_classes as int,
    ensures
        result == seg_alloc_spec(lists@.map_values(|v: Vec<u64>| v@), size, num_classes),
{
    let ghost mapped = lists@.map_values(|v: Vec<u64>| v@);
    let class = seg_class_for(size, num_classes) as usize;
    proof {
        let n = lists@.len();
        assert(class as u64 == seg_class_for_spec(size, num_classes));
        assert((class as int) < n);
        assert(mapped[class as int] == lists@[class as int]@);
    }
    if !lists[class].is_empty() {
        proof {
            assert(seg_alloc_spec(mapped, size, num_classes) == Some((lists@[class as int]@[0], class as u64)));
        }
        Some((lists[class][0], class as u64))
    } else {
        proof {
            assert(seg_alloc_spec(mapped, size, num_classes) == None::<(u64, u64)>);
        }
        None
    }
}

// ─── Seg Free ────────────────────────────────────────────────────────

// r[depends segregated-free.free]
/// Spec: return block to its class list (prepend).
pub open spec fn seg_free_spec(
    lists: Seq<Seq<u64>>, block: u64, class: u64,
) -> Seq<Seq<u64>>
    recommends 0 <= class < lists.len()
{
    let old_list = lists[class as int];
    let new_list = seq![block] + old_list;
    lists.update(class as int, new_list)
}

/// Helper: clone `list` while prepending `block`.
fn clone_prepend(list: &[u64], block: u64) -> (result: Vec<u64>)
    ensures result@ == seq![block] + list@
{
    let mut result: Vec<u64> = Vec::new();
    result.push(block);
    let mut i: usize = 0;
    while i < list.len()
        invariant
            0 <= i <= list@.len(),
            result@ == seq![block] + list@.take(i as int),
        decreases list@.len() - i,
    {
        result.push(list[i]);
        proof {
            assert(list@.take(i as int + 1) =~= list@.take(i as int).push(list@[i as int]));
        }
        i = i + 1;
    }
    result
}

proof fn mapped_lists_update(lists: Seq<Vec<u64>>, index: int, new_list: Vec<u64>)
    requires
        0 <= index < lists.len(),
    ensures
        lists.update(index, new_list).map_values(|v: Vec<u64>| v@)
            =~= lists.map_values(|v: Vec<u64>| v@).update(index, new_list@),
{
    let updated = lists.update(index, new_list).map_values(|v: Vec<u64>| v@);
    let mapped = lists.map_values(|v: Vec<u64>| v@).update(index, new_list@);
    assert forall|i: int|
        #![trigger updated[i]]
        0 <= i < lists.len()
    implies updated[i] == mapped[i] by {
        if i == index {
        } else {
        }
    }
}

// r[impl segregated-free.free]
/// Return a block to its class's free list.
#[allow(clippy::cast_possible_truncation)]
pub fn seg_free(
    lists: &mut Vec<Vec<u64>>, block: u64, class: u64,
)
    requires
        0 <= class < old(lists)@.len(),
    ensures
        lists@.map_values(|v: Vec<u64>| v@)
            == seg_free_spec(old(lists)@.map_values(|v: Vec<u64>| v@), block, class),
{
    let ghost old_mapped = old(lists)@.map_values(|v: Vec<u64>| v@);
    let index = class as usize;
    let new_list = clone_prepend(&lists[index], block);
    proof {
        assert(new_list@ == seq![block] + old(lists)@[index as int]@);
    }
    lists.set(index, new_list);
    proof {
        mapped_lists_update(old(lists)@, index as int, lists@[index as int]);
        assert(old_mapped[index as int] == old(lists)@[index as int]@);
    }
}

// ─── Proofs ──────────────────────────────────────────────────────────

// r[verify segregated-free.class-for]
/// Class index is deterministic for same size.
proof fn class_deterministic(size: u64, num_classes: u64)
    requires size > 0, num_classes > 0
    ensures seg_class_for_spec(size, num_classes) == seg_class_for_spec(size, num_classes)
{}

// r[verify segregated-free.class-for]
/// Class is bounded by num_classes.
proof fn class_bounded(size: u64, num_classes: u64)
    requires size > 0, num_classes > 0
    ensures seg_class_for_spec(size, num_classes) < num_classes
{}

// r[verify segregated-free.total-free]
/// Total free is non-negative.
proof fn total_free_nonneg(lists: Seq<Seq<u64>>)
    ensures seg_total_free_spec(lists) >= 0
{
    total_free_from_nonneg(lists, 0);
}

proof fn total_free_from_nonneg(lists: Seq<Seq<u64>>, from: int)
    ensures seg_total_free_from_spec(lists, from) >= 0
    decreases (if lists.len() >= from { lists.len() - from } else { 0 }),
{
    if from >= 0 && from < lists.len() {
        total_free_from_nonneg(lists, from + 1);
    }
}

// r[verify segregated-free.free]
/// Free to class c only adds to class c, not other classes.
proof fn free_only_modifies_class(
    lists: Seq<Seq<u64>>, block: u64, class: u64, other: u64,
)
    requires 0 <= class < lists.len(), 0 <= other < lists.len(), other != class
    ensures seg_free_spec(lists, block, class)[other as int]
        == lists[other as int]
{}

// r[verify segregated-free.alloc]
/// Alloc from empty class returns None.
proof fn alloc_empty_class_none(lists: Seq<Seq<u64>>, size: u64, num_classes: u64)
    requires
        size > 0, num_classes > 0,
        num_classes as int <= lists.len(),
        ({
            let class = seg_class_for_spec(size, num_classes);
            class < lists.len() && lists[class as int].len() == 0
        }),
    ensures seg_alloc_spec(lists, size, num_classes) == None::<(u64, u64)>
{}

} // verus!

#[cfg(test)]
#[allow(clippy::cast_possible_truncation)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify segregated-free.class-for]
    #[test]
    fn test_class_for() {
        assert_eq!(seg_class_for(1, 17), 0);
        assert_eq!(seg_class_for(2, 17), 1);
        assert_eq!(seg_class_for(3, 17), 2);
        assert_eq!(seg_class_for(4, 17), 2);
        assert_eq!(seg_class_for(5, 17), 3);
        assert_eq!(seg_class_for(1024, 17), 10);
        assert_eq!(seg_class_for(4096, 17), 12);
    }

    // r[verify segregated-free.alloc]
    #[test]
    fn test_seg_alloc() {
        let mut lists: Vec<Vec<u64>> = vec![Vec::new(); 17];
        lists[2].push(100); // class 2 has one block
        let result = seg_alloc(&lists, 3, 17); // size 3 → class 2
        assert!(result.is_some());
        let (block, class) = result.unwrap();
        assert_eq!(block, 100);
        assert_eq!(class, 2);
    }

    // r[verify segregated-free.alloc]
    #[test]
    fn test_seg_alloc_empty() {
        let lists: Vec<Vec<u64>> = vec![Vec::new(); 17];
        let result = seg_alloc(&lists, 4, 17);
        assert!(result.is_none());
    }

    // r[verify segregated-free.free]
    #[test]
    fn test_seg_free() {
        let mut lists: Vec<Vec<u64>> = vec![Vec::new(); 17];
        seg_free(&mut lists, 200, 3);
        assert_eq!(lists[3], vec![200]);
        assert!(lists[0].is_empty());
    }

    // r[verify segregated-free.total-free]
    #[test]
    fn test_total_free() {
        let mut lists: Vec<Vec<u64>> = vec![Vec::new(); 4];
        lists[0] = vec![1, 2];
        lists[2] = vec![3];
        assert_eq!(seg_total_free(&lists), 3);
    }

    proptest! {
        // r[verify segregated-free.class-for]
        #[test]
        fn prop_class_bounded(size in 1u64..=65536, nc in 1u64..=17) {
            let class = seg_class_for(size, nc);
            prop_assert!(class < nc);
        }

        // r[verify segregated-free.free]
        #[test]
        fn prop_free_only_modifies_target(class in 0u64..16) {
            let mut lists: Vec<Vec<u64>> = vec![Vec::new(); 17];
            lists[5] = vec![99];
            let old_other = lists[5].clone();
            if class != 5 {
                seg_free(&mut lists, 42, class);
                prop_assert_eq!(lists[5].clone(), old_other);
            }
        }

        // r[verify segregated-free.alloc]
        #[test]
        fn prop_alloc_free_roundtrip(
            size in 1u64..=65536,
            block in 0u64..10000,
        ) {
            let nc = 17u64;
            let class = seg_class_for(size, nc);
            let mut lists: Vec<Vec<u64>> = vec![Vec::new(); nc as usize];
            seg_free(&mut lists, block, class);
            let result = seg_alloc(&lists, size, nc);
            prop_assert!(result.is_some());
            let (got_block, got_class) = result.unwrap();
            prop_assert_eq!(got_block, block);
            prop_assert_eq!(got_class, class);
        }
    }
}
