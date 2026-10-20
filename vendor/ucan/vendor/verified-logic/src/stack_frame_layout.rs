//! Verified stack-frame slot assignment and frame sizing.
//!
//! The current model focuses on static local and spill slots. Slot placement is
//! append-only: each new slot begins at the first aligned byte after the prior
//! maximum slot extent. This is enough to prove alignment, exact rounded frame
//! size, frame-bound coverage, and non-overlap. Slot reuse is modeled as a
//! separate compatibility check over explicit live ranges.

#[cfg(verus_keep_ghost)]
use vstd::arithmetic::div_mod::lemma_fundamental_div_mod;
#[cfg(verus_keep_ghost)]
use vstd::arithmetic::div_mod::lemma_mod_pos_bound;
use vstd::prelude::*;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SlotKind {
    Local(u32),
    Spill(u32),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct StackSlot {
    pub offset: u32,
    pub size: u32,
    pub alignment: u32,
    pub kind: SlotKind,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct LiveRange {
    pub start: u32,
    pub end: u32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct LiveSlot {
    pub slot: StackSlot,
    pub live: LiveRange,
}

#[derive(Debug, PartialEq, Eq)]
pub struct FrameLayout {
    pub slots: Vec<StackSlot>,
    pub frame_size: u32,
    pub stack_alignment: u32,
}

// r[impl stack-frame.non-overlap]
// r[depends stack-frame.non-overlap]
pub open spec fn slot_end_spec(slot: StackSlot) -> int {
    slot.offset as int + slot.size as int
}

// r[impl stack-frame.alignment]
// r[depends stack-frame.alignment]
pub open spec fn slot_aligned_spec(slot: StackSlot) -> bool {
    &&& slot.alignment > 0
    &&& (slot.offset as int) % (slot.alignment as int) == 0
}

// r[impl stack-frame.non-overlap]
// r[depends stack-frame.non-overlap]
pub open spec fn slot_valid_spec(slot: StackSlot) -> bool {
    &&& slot.size > 0
    &&& slot_aligned_spec(slot)
    &&& slot_end_spec(slot) <= u32::MAX as int
}

// r[impl stack-frame.non-overlap]
// r[depends stack-frame.non-overlap]
pub open spec fn byte_range_spec(slot: StackSlot) -> (int, int) {
    (slot.offset as int, slot_end_spec(slot))
}

// r[impl stack-frame.non-overlap]
// r[depends stack-frame.non-overlap]
pub open spec fn ranges_overlap_spec(left: StackSlot, right: StackSlot) -> bool {
    &&& slot_valid_spec(left)
    &&& slot_valid_spec(right)
    &&& (left.offset as int) < slot_end_spec(right)
    &&& (right.offset as int) < slot_end_spec(left)
}

// r[impl stack-frame.non-overlap]
// r[depends stack-frame.non-overlap]
pub open spec fn ranges_disjoint_spec(left: StackSlot, right: StackSlot) -> bool {
    &&& slot_valid_spec(left)
    &&& slot_valid_spec(right)
    &&& (slot_end_spec(left) <= (right.offset as int) || slot_end_spec(right) <= (left.offset as int))
}

pub open spec fn same_byte_range_spec(left: StackSlot, right: StackSlot) -> bool {
    &&& slot_valid_spec(left)
    &&& slot_valid_spec(right)
    &&& left.offset == right.offset
    &&& left.size == right.size
}

pub open spec fn live_range_valid_spec(live: LiveRange) -> bool {
    live.start <= live.end
}

pub open spec fn live_ranges_overlap_spec(left: LiveRange, right: LiveRange) -> bool {
    &&& live_range_valid_spec(left)
    &&& live_range_valid_spec(right)
    &&& left.start < right.end
    &&& right.start < left.end
}

pub open spec fn live_slot_valid_spec(live_slot: LiveSlot) -> bool {
    &&& slot_valid_spec(live_slot.slot)
    &&& live_range_valid_spec(live_slot.live)
}

// r[impl stack-frame.non-overlap]
// r[depends stack-frame.non-overlap]
pub open spec fn can_reuse_slot_spec(existing: LiveSlot, candidate: LiveSlot) -> bool {
    &&& live_slot_valid_spec(existing)
    &&& live_slot_valid_spec(candidate)
    &&& same_byte_range_spec(existing.slot, candidate.slot)
    &&& !live_ranges_overlap_spec(existing.live, candidate.live)
}

pub open spec fn compatible_with_suffix_spec(assignments: Seq<LiveSlot>, index: int, from: int) -> bool
    decreases if assignments.len() >= from { assignments.len() - from } else { 0 }
{
    if from < 0 || from >= assignments.len() {
        true
    } else {
        let current = assignments[index];
        let other = assignments[from];
        &&& live_slot_valid_spec(current)
        &&& live_slot_valid_spec(other)
        &&& (!live_ranges_overlap_spec(current.live, other.live) || ranges_disjoint_spec(current.slot, other.slot))
        &&& compatible_with_suffix_spec(assignments, index, from + 1)
    }
}

pub open spec fn non_overlap_from_spec(assignments: Seq<LiveSlot>, from: int) -> bool
    decreases if assignments.len() >= from { assignments.len() - from } else { 0 }
{
    if from < 0 || from >= assignments.len() {
        true
    } else {
        &&& compatible_with_suffix_spec(assignments, from, from + 1)
        &&& non_overlap_from_spec(assignments, from + 1)
    }
}

// r[impl stack-frame.non-overlap]
// r[depends stack-frame.non-overlap]
pub open spec fn non_overlap_spec(assignments: Seq<LiveSlot>) -> bool {
    non_overlap_from_spec(assignments, 0)
}

pub open spec fn aligned_slots_from_spec(slots: Seq<StackSlot>, from: int) -> bool
    decreases if slots.len() >= from { slots.len() - from } else { 0 }
{
    if from < 0 || from >= slots.len() {
        true
    } else {
        &&& slot_valid_spec(slots[from])
        &&& aligned_slots_from_spec(slots, from + 1)
    }
}

pub open spec fn max_slot_end_from_spec(slots: Seq<StackSlot>, from: int) -> int
    decreases if slots.len() >= from { slots.len() - from } else { 0 }
{
    if from < 0 || from >= slots.len() {
        0
    } else {
        let tail = max_slot_end_from_spec(slots, from + 1);
        let here = slot_end_spec(slots[from]);
        if here > tail { here } else { tail }
    }
}

// r[impl stack-frame.frame-size]
// r[depends stack-frame.frame-size]
pub open spec fn round_up_spec(value: u32, alignment: u32) -> Option<u32>
    recommends alignment > 0
{
    if value == 0 {
        Some(0u32)
    } else {
        let q = (((value - 1) as int) / alignment as int) + 1;
        let rounded = q * alignment as int;
        if rounded > u32::MAX as int {
            None
        } else {
            Some(rounded as u32)
        }
    }
}

// r[impl stack-frame.frame-size]
// r[depends stack-frame.frame-size]
pub open spec fn frame_size_spec(slots: Seq<StackSlot>, stack_alignment: u32) -> Option<u32> {
    if stack_alignment == 0 || !aligned_slots_from_spec(slots, 0) {
        None
    } else {
        let max_end = max_slot_end_from_spec(slots, 0);
        if max_end > u32::MAX as int {
            None
        } else {
            round_up_spec(max_end as u32, stack_alignment)
        }
    }
}

// r[impl stack-frame.alignment]
// r[depends stack-frame.alignment]
pub open spec fn alignment_invariant_spec(layout: FrameLayout) -> bool {
    &&& layout.stack_alignment > 0
    &&& (layout.frame_size as int) % (layout.stack_alignment as int) == 0
    &&& aligned_slots_from_spec(layout.slots@, 0)
}

// r[impl stack-frame.growth]
// r[depends stack-frame.growth]
pub open spec fn growth_invariant_spec(layout: FrameLayout) -> bool {
    &&& frame_size_spec(layout.slots@, layout.stack_alignment) == Some(layout.frame_size)
    &&& forall|i: int| #![trigger layout.slots@[i]] 0 <= i < layout.slots@.len()
        ==> slot_end_spec(layout.slots@[i]) <= layout.frame_size as int
}

pub open spec fn layout_valid_spec(layout: FrameLayout) -> bool {
    &&& alignment_invariant_spec(layout)
    &&& growth_invariant_spec(layout)
}

fn checked_slot_end(slot: StackSlot) -> (result: Option<u32>)
    ensures
        result.is_some() ==> {
            &&& slot_valid_spec(slot)
            &&& result.unwrap() as int == slot_end_spec(slot)
        },
        result.is_none() ==> !slot_valid_spec(slot),
{
    if slot.size == 0 || slot.alignment == 0 {
        return None;
    }
    if slot.offset % slot.alignment != 0 {
        return None;
    }
    slot.offset.checked_add(slot.size)
}

fn aligned_slots_from(slots: &[StackSlot], from: usize) -> (result: bool)
    ensures result == aligned_slots_from_spec(slots@, from as int),
    decreases slots@.len() - from,
{
    if from >= slots.len() {
        true
    } else {
        match checked_slot_end(slots[from]) {
            Some(_) => aligned_slots_from(slots, from + 1),
            None => false,
        }
    }
}

fn max_slot_end_from(slots: &[StackSlot], from: usize) -> (result: Option<u32>)
    ensures
        result.is_some() ==> {
            &&& aligned_slots_from_spec(slots@, from as int)
            &&& result.unwrap() as int == max_slot_end_from_spec(slots@, from as int)
        },
        result.is_none() ==> !aligned_slots_from_spec(slots@, from as int),
    decreases slots@.len() - from,
{
    if from >= slots.len() {
        Some(0u32)
    } else {
        match checked_slot_end(slots[from]) {
            None => None,
            Some(here) => {
                match max_slot_end_from(slots, from + 1) {
                    None => None,
                    Some(tail) => {
                        if here > tail { Some(here) } else { Some(tail) }
                    },
                }
            },
        }
    }
}

// r[impl stack-frame.frame-size]
pub fn round_up(value: u32, alignment: u32) -> (result: Option<u32>)
    ensures
        alignment > 0 ==> result == round_up_spec(value, alignment),
        alignment == 0 ==> result.is_none(),
{
    if alignment == 0 {
        None
    } else if value == 0 {
        Some(0u32)
    } else {
        let q = (value - 1) / alignment + 1;
        q.checked_mul(alignment)
    }
}

// r[impl stack-frame.non-overlap]
pub fn byte_range(slot: StackSlot) -> (result: Option<(u32, u32)>)
    ensures
        result.is_some() ==> {
            &&& slot_valid_spec(slot)
            &&& result.unwrap().0 == slot.offset
            &&& result.unwrap().1 as int == slot_end_spec(slot)
        },
        result.is_none() ==> !slot_valid_spec(slot),
{
    match checked_slot_end(slot) {
        Some(end) => Some((slot.offset, end)),
        None => None,
    }
}

// r[impl stack-frame.non-overlap]
pub fn ranges_overlap(left: StackSlot, right: StackSlot) -> (result: bool)
    ensures result == ranges_overlap_spec(left, right),
{
    if let Some((left_lo, left_hi)) = byte_range(left) {
        if let Some((right_lo, right_hi)) = byte_range(right) {
            left_lo < right_hi && right_lo < left_hi
        } else {
            false
        }
    } else {
        false
    }
}

fn compatible_with_suffix(assignments: &[LiveSlot], index: usize, from: usize) -> (result: bool)
    requires index < assignments@.len(),
    ensures result == compatible_with_suffix_spec(assignments@, index as int, from as int),
    decreases assignments@.len() - from,
{
    if from >= assignments.len() {
        true
    } else {
        let current = assignments[index];
        let other = assignments[from];
        let current_live_valid = current.live.start <= current.live.end;
        let other_live_valid = other.live.start <= other.live.end;
        let compatible_here = if current_live_valid && other_live_valid {
            if let Some((current_lo, current_hi)) = byte_range(current.slot) {
                if let Some((other_lo, other_hi)) = byte_range(other.slot) {
                    let live_overlap = current.live.start < other.live.end && other.live.start < current.live.end;
                    let slot_disjoint = current_hi <= other_lo || other_hi <= current_lo;
                    !live_overlap || slot_disjoint
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };
        compatible_here && compatible_with_suffix(assignments, index, from + 1)
    }
}

fn non_overlap_from(assignments: &[LiveSlot], from: usize) -> (result: bool)
    ensures result == non_overlap_from_spec(assignments@, from as int),
    decreases assignments@.len() - from,
{
    if from >= assignments.len() {
        true
    } else {
        compatible_with_suffix(assignments, from, from + 1) && non_overlap_from(assignments, from + 1)
    }
}

// r[impl stack-frame.non-overlap]
pub fn check_non_overlap(assignments: &[LiveSlot]) -> (result: bool)
    ensures result == non_overlap_spec(assignments@),
{
    non_overlap_from(assignments, 0)
}

// r[impl stack-frame.non-overlap]
pub fn can_reuse_slot(existing: LiveSlot, candidate: LiveSlot) -> (result: bool)
    ensures result == can_reuse_slot_spec(existing, candidate),
{
    if let Some((existing_lo, existing_hi)) = byte_range(existing.slot) {
        if let Some((candidate_lo, candidate_hi)) = byte_range(candidate.slot) {
            existing_lo == candidate_lo
                && existing_hi == candidate_hi
                && existing.live.start <= existing.live.end
                && candidate.live.start <= candidate.live.end
                && !(existing.live.start < candidate.live.end && candidate.live.start < existing.live.end)
        } else {
            false
        }
    } else {
        false
    }
}

// r[impl stack-frame.alignment]
pub fn check_alignment(layout: &FrameLayout) -> (result: bool)
    ensures result == alignment_invariant_spec(*layout),
{
    layout.stack_alignment > 0
        && layout.frame_size % layout.stack_alignment == 0
        && aligned_slots_from(&layout.slots, 0)
}

// r[impl stack-frame.frame-size]
pub fn compute_frame_size(slots: &[StackSlot], stack_alignment: u32) -> (result: Option<u32>)
    ensures result == frame_size_spec(slots@, stack_alignment),
{
    if stack_alignment == 0 {
        return None;
    }
    match max_slot_end_from(slots, 0) {
        None => None,
        Some(max_end) => round_up(max_end, stack_alignment),
    }
}

fn clone_slots(slots: &[StackSlot]) -> (result: Vec<StackSlot>)
    ensures result@ == slots@,
{
    let mut result: Vec<StackSlot> = Vec::new();
    let mut index: usize = 0;
    while index < slots.len()
        invariant
            0 <= index <= slots@.len(),
            result@ == slots@.subrange(0, index as int),
        decreases slots@.len() - index,
    {
        result.push(slots[index]);
        index = index + 1;
    }
    result
}

fn layout_is_valid(layout: &FrameLayout) -> (result: bool)
{
    if !check_alignment(layout) {
        return false;
    }
    compute_frame_size(&layout.slots, layout.stack_alignment) == Some(layout.frame_size)
}

// r[impl stack-frame.frame-size]
// r[impl stack-frame.alignment]
// r[impl stack-frame.growth]
pub fn empty_frame(stack_alignment: u32) -> (result: Option<FrameLayout>)
    ensures
        result.is_some() ==> layout_valid_spec(result.unwrap()),
        result.is_none() ==> stack_alignment == 0,
{
    if stack_alignment == 0 {
        None
    } else {
        Some(FrameLayout {
            slots: Vec::new(),
            frame_size: 0u32,
            stack_alignment,
        })
    }
}

// r[impl stack-frame.non-overlap]
// r[impl stack-frame.alignment]
// r[impl stack-frame.frame-size]
// r[impl stack-frame.growth]
#[allow(clippy::manual_let_else, clippy::question_mark)]
pub fn assign_slot(layout: &FrameLayout, size: u32, alignment: u32, kind: SlotKind) -> (result: Option<(StackSlot, FrameLayout)>)
    ensures
        result.is_some() ==> {
            let pair = result.unwrap();
            let slot = pair.0;
            let next = pair.1;
            &&& size > 0
            &&& alignment > 0
            &&& slot.size == size
            &&& slot.alignment == alignment
            &&& slot.kind == kind
            &&& next.stack_alignment == layout.stack_alignment
            &&& next.slots@ == layout.slots@.push(slot)
        },
{
    if !layout_is_valid(layout) || size == 0 || alignment == 0 {
        return None;
    }

    let current_end = match max_slot_end_from(&layout.slots, 0) {
        Some(value) => value,
        None => return None,
    };
    let offset = match round_up(current_end, alignment) {
        Some(value) => value,
        None => return None,
    };
    let slot = StackSlot {
        offset,
        size,
        alignment,
        kind,
    };
    let slot_end = match checked_slot_end(slot) {
        Some(value) => value,
        None => return None,
    };
    let frame_size = match round_up(slot_end, layout.stack_alignment) {
        Some(value) => value,
        None => return None,
    };
    let mut next_slots = clone_slots(&layout.slots);
    next_slots.push(slot);
    let next = FrameLayout {
        slots: next_slots,
        frame_size,
        stack_alignment: layout.stack_alignment,
    };
    Some((slot, next))
}

// r[verify stack-frame.non-overlap]
proof fn reusable_same_range_requires_disjoint_liveness(existing: LiveSlot, candidate: LiveSlot)
    requires can_reuse_slot_spec(existing, candidate)
    ensures !live_ranges_overlap_spec(existing.live, candidate.live)
{}

// r[verify stack-frame.frame-size]
proof fn round_up_ge(value: u32, alignment: u32)
    requires alignment > 0, round_up_spec(value, alignment).is_some()
    ensures round_up_spec(value, alignment).unwrap() >= value
{
    if value > 0 {
        let a = alignment as int;
        let xm = (value - 1) as int;
        let q = xm / a;
        lemma_fundamental_div_mod(xm, a);
        lemma_mod_pos_bound(xm, a);
        assert((q + 1) * a >= value as int) by(nonlinear_arith)
            requires
                xm == a * q + xm % a,
                0 <= xm % a,
                xm % a < a,
                xm == value as int - 1,
                a > 0;
    }
}


} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    const STACK_ALIGN_16: u32 = 16u32;
    const SLOT_ALIGN_8: u32 = 8u32;
    const SLOT_ALIGN_4: u32 = 4u32;
    const SLOT_SIZE_8: u32 = 8u32;
    const SLOT_SIZE_4: u32 = 4u32;
    const LIVE_START_ZERO: u32 = 0u32;
    const LIVE_MID: u32 = 4u32;
    const LIVE_END: u32 = 8u32;

    // r[verify stack-frame.frame-size]
    #[test]
    fn test_empty_frame_is_zero_sized() {
        let layout = empty_frame(STACK_ALIGN_16).unwrap();
        assert_eq!(layout.frame_size, 0u32);
        assert!(check_alignment(&layout));
    }

    // r[verify stack-frame.alignment]
    // r[verify stack-frame.frame-size]
    #[test]
    fn test_assign_slot_aligns_and_rounds_frame() {
        let layout = empty_frame(STACK_ALIGN_16).unwrap();
        let (slot, next) = assign_slot(&layout, SLOT_SIZE_8, SLOT_ALIGN_8, SlotKind::Local(0u32)).unwrap();
        assert_eq!(slot.offset, 0u32);
        assert_eq!(next.frame_size, STACK_ALIGN_16);
        assert!(check_alignment(&next));
    }

    // r[verify stack-frame.growth]
    // r[verify stack-frame.non-overlap]
    #[test]
    fn test_assign_slot_uses_max_extent_not_frame_padding() {
        let layout = empty_frame(STACK_ALIGN_16).unwrap();
        let (_, layout) = assign_slot(&layout, SLOT_SIZE_8, SLOT_ALIGN_8, SlotKind::Local(0u32)).unwrap();
        let (slot, next) = assign_slot(&layout, SLOT_SIZE_4, SLOT_ALIGN_4, SlotKind::Spill(1u32)).unwrap();
        assert_eq!(slot.offset, SLOT_SIZE_8);
        assert_eq!(next.frame_size, STACK_ALIGN_16);
    }

    // r[verify stack-frame.non-overlap]
    #[test]
    fn test_reuse_allowed_for_disjoint_liveness() {
        let slot = StackSlot {
            offset: 0u32,
            size: SLOT_SIZE_8,
            alignment: SLOT_ALIGN_8,
            kind: SlotKind::Local(0u32),
        };
        let first = LiveSlot {
            slot,
            live: LiveRange {
                start: LIVE_START_ZERO,
                end: LIVE_MID,
            },
        };
        let second = LiveSlot {
            slot: StackSlot {
                kind: SlotKind::Spill(1u32),
                ..slot
            },
            live: LiveRange {
                start: LIVE_MID,
                end: LIVE_END,
            },
        };
        assert!(can_reuse_slot(first, second));
        assert!(check_non_overlap(&[first, second]));
    }

    // r[verify stack-frame.non-overlap]
    #[test]
    fn test_overlap_rejected_for_live_ranges() {
        let first = LiveSlot {
            slot: StackSlot {
                offset: 0u32,
                size: SLOT_SIZE_8,
                alignment: SLOT_ALIGN_8,
                kind: SlotKind::Local(0u32),
            },
            live: LiveRange {
                start: LIVE_START_ZERO,
                end: LIVE_END,
            },
        };
        let second = LiveSlot {
            slot: StackSlot {
                offset: SLOT_ALIGN_4,
                size: SLOT_SIZE_8,
                alignment: SLOT_ALIGN_4,
                kind: SlotKind::Spill(1u32),
            },
            live: LiveRange {
                start: LIVE_MID,
                end: LIVE_END,
            },
        };
        assert!(!check_non_overlap(&[first, second]));
    }

    // r[verify stack-frame.alignment]
    #[test]
    fn test_zero_alignment_rejected() {
        assert!(empty_frame(0u32).is_none());
    }

    // r[verify stack-frame.frame-size]
    #[test]
    fn test_invalid_slot_rejected_by_frame_size() {
        let slots = [StackSlot {
            offset: 3u32,
            size: SLOT_SIZE_4,
            alignment: SLOT_ALIGN_4,
            kind: SlotKind::Local(0u32),
        }];
        assert_eq!(compute_frame_size(&slots, STACK_ALIGN_16), None);
    }

    // r[verify stack-frame.alignment]
    proptest! {
        #[test]
        fn prop_assign_slot_keeps_alignment(
            size in 1u32..64u32,
            alignment in 1u32..16u32,
        ) {
            let base = empty_frame(STACK_ALIGN_16).unwrap();
            let assigned = assign_slot(&base, size, alignment, SlotKind::Local(0u32));
            if let Some((slot, next)) = assigned {
                prop_assert_eq!(slot.offset % alignment, 0u32);
                prop_assert!(check_alignment(&next));
            } else {
                prop_assert!(alignment == 0u32 || size == 0u32 || STACK_ALIGN_16 == 0u32);
            }
        }

        // r[verify stack-frame.frame-size]
        // r[verify stack-frame.growth]
        #[test]
        fn prop_frame_size_covers_all_slots(
            first_size in 1u32..32u32,
            second_size in 1u32..32u32,
        ) {
            let base = empty_frame(STACK_ALIGN_16).unwrap();
            let (_, first) = assign_slot(&base, first_size, SLOT_ALIGN_4, SlotKind::Local(0u32)).unwrap();
            let (_, second) = assign_slot(&first, second_size, SLOT_ALIGN_4, SlotKind::Spill(1u32)).unwrap();
            for slot in &second.slots {
                let (_, end) = byte_range(*slot).unwrap();
                prop_assert!(end <= second.frame_size);
            }
        }

        // r[verify stack-frame.non-overlap]
        #[test]
        fn prop_disjoint_liveness_allows_shared_slot(
            end_a in 1u32..32u32,
            gap in 0u32..8u32,
            span_b in 1u32..32u32,
        ) {
            let shared = StackSlot { offset: 0u32, size: SLOT_SIZE_8, alignment: SLOT_ALIGN_8, kind: SlotKind::Local(0u32) };
            let first = LiveSlot {
                slot: shared,
                live: LiveRange { start: 0u32, end: end_a },
            };
            let second = LiveSlot {
                slot: StackSlot { kind: SlotKind::Spill(1u32), ..shared },
                live: LiveRange { start: end_a + gap, end: end_a + gap + span_b },
            };
            prop_assert!(can_reuse_slot(first, second));
            prop_assert!(check_non_overlap(&[first, second]));
        }
    }
}
