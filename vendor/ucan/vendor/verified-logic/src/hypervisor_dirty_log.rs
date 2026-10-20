//! Verified dirty-page bitmap for KVM-style memslots.
//!
//! Each memslot carries a per-slot dirty bitmap indexed by the guest frame's
//! relative offset `gfn - base_gfn`.  Mark is idempotent, merge is bitwise
//! union, and clear resets all bits.

use vstd::prelude::*;

verus! {

// ────────────────────────────────────────────────────────────────────
//  dirty_index — relative GFN offset
// ────────────────────────────────────────────────────────────────────

// r[depends hypervisor-dirty-log.index]
/// Spec: dirty-bit index for `gfn` in a slot with `base_gfn`.
pub open spec fn dirty_index_spec(base_gfn: u64, gfn: u64) -> int {
    gfn as int - base_gfn as int
}

// r[impl hypervisor-dirty-log.index]
/// Returns the dirty-bit index: `gfn - base_gfn`.
pub fn dirty_index(base_gfn: u64, gfn: u64) -> (result: u64)
    requires base_gfn <= gfn
    ensures  result as int == dirty_index_spec(base_gfn, gfn)
{
    gfn - base_gfn
}

// r[verify hypervisor-dirty-log.index]
/// First page of slot maps to bit zero.
proof fn first_page_index_zero(base_gfn: u64)
    ensures dirty_index_spec(base_gfn, base_gfn) == 0
{}

// r[verify hypervisor-dirty-log.index]
/// Index is bounded by npages when gfn is in the slot.
proof fn dirty_index_bounded(base_gfn: u64, npages: u64, gfn: u64)
    requires
        base_gfn as int <= gfn as int,
        (gfn as int) < base_gfn as int + npages as int,
    ensures
        0 <= dirty_index_spec(base_gfn, gfn) < npages as int,
{}

// ────────────────────────────────────────────────────────────────────
//  is_dirty / mark_dirty — single bit operations
// ────────────────────────────────────────────────────────────────────

// r[depends hypervisor-dirty-log.mark]
/// Spec: is the bit at `idx` set?
pub open spec fn is_dirty_spec(bitmap: Seq<bool>, idx: int) -> bool
    recommends 0 <= idx < bitmap.len()
{
    bitmap[idx]
}

// r[depends hypervisor-dirty-log.mark]
/// Spec: set bit at `idx`, leaving others unchanged.
pub open spec fn mark_dirty_spec(bitmap: Seq<bool>, idx: int) -> Seq<bool>
    recommends 0 <= idx < bitmap.len()
{
    bitmap.update(idx, true)
}

// r[verify hypervisor-dirty-log.mark]
/// Marking is idempotent.
proof fn mark_idempotent(bitmap: Seq<bool>, idx: int)
    requires 0 <= idx < bitmap.len()
    ensures
        mark_dirty_spec(mark_dirty_spec(bitmap, idx), idx)
            =~= mark_dirty_spec(bitmap, idx)
{}

// r[verify hypervisor-dirty-log.mark]
/// Marking preserves other bits.
proof fn mark_preserves_others(bitmap: Seq<bool>, idx: int, other: int)
    requires
        0 <= idx < bitmap.len(),
        0 <= other < bitmap.len(),
        other != idx,
    ensures
        mark_dirty_spec(bitmap, idx)[other] == bitmap[other]
{}

// r[impl hypervisor-dirty-log.mark]
/// Sets the dirty bit at `idx`.
pub fn mark_dirty(bitmap: &mut Vec<bool>, idx: usize)
    requires
        0 <= idx < old(bitmap)@.len(),
    ensures
        bitmap@ =~= mark_dirty_spec(old(bitmap)@, idx as int),
{
    bitmap.set(idx, true);
}

// r[impl hypervisor-dirty-log.mark]
/// Returns whether the bit at `idx` is dirty.
pub fn is_dirty(bitmap: &Vec<bool>, idx: usize) -> (result: bool)
    requires 0 <= idx < bitmap@.len()
    ensures  result == is_dirty_spec(bitmap@, idx as int)
{
    bitmap[idx]
}

// ────────────────────────────────────────────────────────────────────
//  merge_dirty — bitwise union
// ────────────────────────────────────────────────────────────────────

// r[depends hypervisor-dirty-log.union]
/// Spec: merge two bitmaps by logical OR.
pub open spec fn merge_dirty_spec(a: Seq<bool>, b: Seq<bool>) -> Seq<bool>
    recommends a.len() == b.len()
{
    Seq::new(a.len(), |i: int| a[i] || b[i])
}

// r[verify hypervisor-dirty-log.union]
/// Merge accumulates: if a bit was dirty in either input it is dirty
/// in the result.
proof fn merge_accumulates(a: Seq<bool>, b: Seq<bool>, idx: int)
    requires
        a.len() == b.len(),
        0 <= idx < a.len(),
    ensures
        merge_dirty_spec(a, b)[idx] == (a[idx] || b[idx])
{}

// r[impl hypervisor-dirty-log.union]
/// Merges `src` into `dst` by OR.
pub fn merge_dirty(dst: &mut Vec<bool>, src: &Vec<bool>)
    requires
        old(dst)@.len() == src@.len(),
    ensures
        dst@ =~= merge_dirty_spec(old(dst)@, src@),
{
    let ghost initial = dst@;
    let len = dst.len();
    let mut i: usize = 0;
    while i < len
        invariant
            0 <= i <= len,
            len == initial.len(),
            len == src@.len(),
            dst@.len() == len,
            // Already merged bits.
            forall|k: int| 0 <= k < i as int ==>
                dst@[k] == (initial[k] || src@[k]),
            // Untouched bits.
            forall|k: int| i as int <= k < len as int ==>
                dst@[k] == initial[k],
        decreases len - i,
    {
        let ghost before = dst@;
        proof {
            assert forall|k: int| 0 <= k < i as int implies
                before[k] == (initial[k] || src@[k]) by {}
            assert forall|k: int| i as int <= k < len as int implies
                before[k] == initial[k] by {}
        }
        if src[i] {
            dst.set(i, true);
            assert(dst@[i as int]);
        } else {
            assert(dst@[i as int] == initial[i as int]);
        }
        assert(dst@[i as int] == (initial[i as int] || src@[i as int]));
        assert forall|k: int| 0 <= k < i as int + 1 implies
            dst@[k] == (initial[k] || src@[k]) by {
            if k < i as int {
                assert(before[k] == (initial[k] || src@[k]));
                assert(dst@[k] == before[k]);
            } else {
                assert(k == i as int);
                assert(dst@[k] == (initial[k] || src@[k]));
            }
        }
        assert forall|k: int| i as int + 1 <= k < len as int implies
            dst@[k] == initial[k] by {
            assert(before[k] == initial[k]);
            assert(dst@[k] == before[k]);
        }
        i += 1;
    }
    assert forall|k: int| 0 <= k < dst@.len() implies
        dst@[k] == merge_dirty_spec(initial, src@)[k] by {
        assert(dst@[k] == (initial[k] || src@[k]));
        merge_accumulates(initial, src@, k);
        assert(merge_dirty_spec(initial, src@)[k] == (initial[k] || src@[k]));
    }
    assert(dst@ =~= merge_dirty_spec(initial, src@));
}

// ────────────────────────────────────────────────────────────────────
//  clear_dirty
// ────────────────────────────────────────────────────────────────────

// r[depends hypervisor-dirty-log.union]
/// Spec: all-zero bitmap of given length.
pub open spec fn cleared_bitmap_spec(len: nat) -> Seq<bool> {
    Seq::new(len, |_i: int| false)
}

// r[verify hypervisor-dirty-log.union]
/// Cleared bitmap has no dirty bits.
proof fn cleared_all_clean(len: nat, idx: int)
    requires 0 <= idx < len as int
    ensures  !cleared_bitmap_spec(len)[idx]
{}

// r[impl hypervisor-dirty-log.union]
/// Resets all dirty bits to false.
pub fn clear_dirty(bitmap: &mut Vec<bool>)
    ensures
        bitmap@.len() == old(bitmap)@.len(),
        bitmap@ =~= cleared_bitmap_spec(old(bitmap)@.len()),
{
    let len = bitmap.len();
    let mut i: usize = 0;
    while i < len
        invariant
            0 <= i <= len,
            len == bitmap@.len(),
            bitmap@.len() == old(bitmap)@.len(),
            forall|k: int| 0 <= k < i as int ==> !bitmap@[k],
            forall|k: int| i as int <= k < len as int ==>
                bitmap@[k] == old(bitmap)@[k],
        decreases len - i,
    {
        bitmap.set(i, false);
        i += 1;
    }
}

} // verus!

// ────────────────────────────────────────────────────────────────────
//  Tests
// ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // r[test hypervisor-dirty-log.index]
    #[test]
    fn test_dirty_index_first_page() {
        assert_eq!(dirty_index(100, 100), 0);
    }

    // r[test hypervisor-dirty-log.index]
    #[test]
    fn test_dirty_index_interior() {
        assert_eq!(dirty_index(100, 103), 3);
    }

    // r[test hypervisor-dirty-log.mark]
    #[test]
    fn test_mark_idempotent() {
        let mut bm = vec![false; 8];
        mark_dirty(&mut bm, 3);
        mark_dirty(&mut bm, 3);
        assert!(is_dirty(&bm, 3));
        assert!(!is_dirty(&bm, 0));
    }

    // r[test hypervisor-dirty-log.mark]
    #[test]
    fn test_mark_preserves_others() {
        let mut bm = vec![false, true, false, false];
        mark_dirty(&mut bm, 2);
        assert!(is_dirty(&bm, 1)); // was already dirty
        assert!(is_dirty(&bm, 2)); // newly dirty
        assert!(!is_dirty(&bm, 0));
        assert!(!is_dirty(&bm, 3));
    }

    // r[test hypervisor-dirty-log.union]
    #[test]
    fn test_merge_accumulates() {
        let mut dst = vec![true, false, false, true];
        let src = vec![false, false, true, true];
        merge_dirty(&mut dst, &src);
        assert!(is_dirty(&dst, 0));
        assert!(!is_dirty(&dst, 1));
        assert!(is_dirty(&dst, 2));
        assert!(is_dirty(&dst, 3));
    }

    // r[test hypervisor-dirty-log.union]
    #[test]
    fn test_clear() {
        let mut bm = vec![true, true, true];
        clear_dirty(&mut bm);
        for i in 0..3 {
            assert!(!is_dirty(&bm, i));
        }
    }
}
