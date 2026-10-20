//! Verified seL4-style untyped-memory primitives.
//!
//! Models one untyped source region plus a set of typed child objects created
//! by retype. Retype succeeds only for aligned, contained, non-overlapping
//! children. Revoke/reset reclaim the full parent by clearing all live
//! children.

use vstd::prelude::*;

use crate::region::contains_region;
#[cfg(verus_keep_ghost)]
use crate::region::contains_region_spec;
use crate::region::regions_disjoint;
#[cfg(verus_keep_ghost)]
use crate::region::regions_disjoint_spec;

verus! {

pub const SEL4_OBJ_ENDPOINT: u8 = 1u8;
pub const SEL4_OBJ_FRAME: u8 = 2u8;

#[derive(Copy, Clone)]
pub struct ChildObject {
    pub base: u64,
    pub size: u64,
    pub align: u64,
    pub kind: u8,
}

#[derive(Clone)]
pub struct UntypedMemory {
    pub base: u64,
    pub size: u64,
    pub children: Vec<ChildObject>,
}

// r[depends sel4-untyped.retype]
// r[depends sel4-untyped.child-layout]
pub open spec fn parent_wf_spec(base: u64, size: u64) -> bool {
    &&& size > 0
    &&& base as int + size as int <= u64::MAX as int
}

// r[depends sel4-untyped.retype]
// r[depends sel4-untyped.child-layout]
pub open spec fn child_wf_spec(child: ChildObject) -> bool {
    &&& child.size > 0
    &&& child.align > 0
    &&& child.base as int + child.size as int <= u64::MAX as int
    &&& child.base % child.align == 0
}

// r[depends sel4-untyped.retype]
// r[depends sel4-untyped.child-layout]
pub open spec fn child_contained_spec(parent_base: u64, parent_size: u64, child: ChildObject) -> bool {
    contains_region_spec(parent_base, parent_size, child.base, child.size)
}

// r[depends sel4-untyped.child-layout]
pub open spec fn child_disjoint_spec(left: ChildObject, right: ChildObject) -> bool {
    regions_disjoint_spec(left.base, left.size, right.base, right.size)
}

// r[depends sel4-untyped.retype]
// r[depends sel4-untyped.child-layout]
pub open spec fn all_children_wf_spec(children: Seq<ChildObject>) -> bool {
    forall|i: int| #![trigger children[i]] 0 <= i < children.len() ==> child_wf_spec(children[i])
}

// r[depends sel4-untyped.retype]
pub open spec fn all_children_contained_spec(parent_base: u64, parent_size: u64, children: Seq<ChildObject>) -> bool {
    forall|i: int| #![trigger children[i]] 0 <= i < children.len() ==> child_contained_spec(parent_base, parent_size, children[i])
}

// r[depends sel4-untyped.child-layout]
pub open spec fn pairwise_disjoint_spec(children: Seq<ChildObject>) -> bool {
    forall|i: int, j: int|
        #![trigger children[i], children[j]]
        0 <= i && i < j && j < children.len() ==> child_disjoint_spec(children[i], children[j])
}

// r[depends sel4-untyped.retype]
// r[depends sel4-untyped.child-layout]
pub open spec fn disjoint_from_all_spec(children: Seq<ChildObject>, child: ChildObject) -> bool {
    forall|i: int| #![trigger children[i]] 0 <= i < children.len() ==> child_disjoint_spec(children[i], child)
}

// r[impl sel4-untyped.retype]
// r[depends sel4-untyped.retype]
// r[depends sel4-untyped.child-layout]
pub open spec fn retype_enabled_spec(
    parent_base: u64,
    parent_size: u64,
    children: Seq<ChildObject>,
    child: ChildObject,
) -> bool {
    &&& parent_wf_spec(parent_base, parent_size)
    &&& child_wf_spec(child)
    &&& child_contained_spec(parent_base, parent_size, child)
    &&& disjoint_from_all_spec(children, child)
}

// r[impl sel4-untyped.retype]
// r[depends sel4-untyped.retype]
pub open spec fn retype_transition_spec(
    parent_base: u64,
    parent_size: u64,
    children: Seq<ChildObject>,
    child: ChildObject,
) -> Seq<ChildObject> {
    if retype_enabled_spec(parent_base, parent_size, children, child) {
        children.push(child)
    } else {
        children
    }
}

// r[impl sel4-untyped.reclaim]
// r[depends sel4-untyped.reclaim]
pub open spec fn reclaim_transition_spec(children: Seq<ChildObject>) -> Seq<ChildObject> {
    seq![]
}

fn child_wf(child: ChildObject) -> (result: bool)
    ensures result == child_wf_spec(child)
{
    child.size > 0
        && child.align > 0
        && child.base <= u64::MAX - child.size
        && child.base % child.align == 0
}

fn child_contained(parent_base: u64, parent_size: u64, child: ChildObject) -> (result: bool)
    requires
        parent_wf_spec(parent_base, parent_size),
        child_wf_spec(child),
    ensures result == child_contained_spec(parent_base, parent_size, child)
{
    contains_region(parent_base, parent_size, child.base, child.size)
}

fn children_disjoint(left: ChildObject, right: ChildObject) -> (result: bool)
    requires
        child_wf_spec(left),
        child_wf_spec(right),
    ensures result == child_disjoint_spec(left, right)
{
    regions_disjoint(left.base, left.size, right.base, right.size)
}

impl UntypedMemory {
    // r[impl sel4-untyped.retype]
    // r[impl sel4-untyped.child-layout]
    // r[impl sel4-untyped.reclaim]
    pub open spec fn child_view(&self) -> Seq<ChildObject> {
        self.children@
    }

    // r[impl sel4-untyped.retype]
    // r[impl sel4-untyped.child-layout]
    // r[impl sel4-untyped.reclaim]
    pub open spec fn wf(&self) -> bool {
        &&& parent_wf_spec(self.base, self.size)
        &&& all_children_wf_spec(self.child_view())
        &&& all_children_contained_spec(self.base, self.size, self.child_view())
        &&& pairwise_disjoint_spec(self.child_view())
    }

    // r[impl sel4-untyped.retype]
    pub fn new(base: u64, size: u64) -> (result: UntypedMemory)
        requires parent_wf_spec(base, size)
        ensures
            result.base == base,
            result.size == size,
            result.child_view().len() == 0,
            result.wf(),
    {
        UntypedMemory { base, size, children: Vec::new() }
    }

    // r[impl sel4-untyped.retype]
    pub fn retype(&mut self, base: u64, size: u64, align: u64, kind: u8) -> (result: bool)
        requires old(self).wf()
        ensures
            self.base == old(self).base,
            self.size == old(self).size,
            self.wf(),
            result ==> retype_enabled_spec(
                old(self).base,
                old(self).size,
                old(self).child_view(),
                ChildObject { base, size, align, kind },
            ),
            result ==> self.child_view() == retype_transition_spec(
                old(self).base,
                old(self).size,
                old(self).child_view(),
                ChildObject { base, size, align, kind },
            ),
            !result ==> self.child_view() == old(self).child_view(),
    {
        let candidate = ChildObject { base, size, align, kind };
        let ghost old_children = old(self).child_view();
        if !child_wf(candidate) {
            return false;
        }
        if !child_contained(self.base, self.size, candidate) {
            proof {
                assert(!child_contained_spec(self.base, self.size, candidate));
            }
            return false;
        }
        let mut i: usize = 0;
        while i < self.children.len()
            invariant
                self.base == old(self).base,
                self.size == old(self).size,
                self.child_view() =~= old_children,
                old(self).wf(),
                child_wf_spec(candidate),
                child_contained_spec(self.base, self.size, candidate),
                0 <= i <= old_children.len(),
                forall|j: int| #![trigger old_children[j]] 0 <= j < i ==> child_disjoint_spec(old_children[j], candidate),
            decreases old_children.len() - i,
        {
            let current = self.children[i];
            proof {
                assert(child_wf_spec(current));
            }
            if !children_disjoint(current, candidate) {
                proof {
                    assert(current == old_children[i as int]);
                    assert(!child_disjoint_spec(old_children[i as int], candidate));
                    assert(!disjoint_from_all_spec(old_children, candidate)) by {
                        assert(!child_disjoint_spec(old_children[i as int], candidate));
                    }
                    assert(self.child_view() == old_children);
                }
                return false;
            }
            i = i + 1;
        }
        proof {
            assert(disjoint_from_all_spec(old_children, candidate));
        }
        self.children.push(candidate);
        proof {
            retype_push_preserves_wf(self.base, self.size, old_children, candidate);
        }
        true
    }

    // r[impl sel4-untyped.reclaim]
    pub fn revoke(&mut self)
        requires old(self).wf()
        ensures
            self.base == old(self).base,
            self.size == old(self).size,
            self.wf(),
            self.child_view() == reclaim_transition_spec(old(self).child_view()),
    {
        self.children = Vec::new();
    }

    // r[impl sel4-untyped.reclaim]
    pub fn reset(&mut self)
        requires old(self).wf()
        ensures
            self.base == old(self).base,
            self.size == old(self).size,
            self.wf(),
            self.child_view() == reclaim_transition_spec(old(self).child_view()),
    {
        self.children = Vec::new();
    }
}

proof fn push_keeps_old_index(seq: Seq<ChildObject>, item: ChildObject, i: int)
    requires 0 <= i < seq.len()
    ensures seq.push(item)[i] == seq[i]
{}

proof fn push_sets_last_index(seq: Seq<ChildObject>, item: ChildObject)
    ensures seq.push(item)[seq.len() as int] == item
{}

proof fn child_wf_from_all(children: Seq<ChildObject>, i: int)
    requires
        all_children_wf_spec(children),
        0 <= i < children.len(),
    ensures child_wf_spec(children[i])
{}

proof fn child_contained_from_all(parent_base: u64, parent_size: u64, children: Seq<ChildObject>, i: int)
    requires
        all_children_contained_spec(parent_base, parent_size, children),
        0 <= i < children.len(),
    ensures child_contained_spec(parent_base, parent_size, children[i])
{}

proof fn child_disjoint_from_pairwise(children: Seq<ChildObject>, i: int, j: int)
    requires
        pairwise_disjoint_spec(children),
        0 <= i < j < children.len(),
    ensures child_disjoint_spec(children[i], children[j])
{}

proof fn child_disjoint_ignores_align_kind(left: ChildObject, right: ChildObject, align: u64, kind: u8)
    ensures
        child_disjoint_spec(left, right)
            == child_disjoint_spec(left, ChildObject {
                base: right.base,
                size: right.size,
                align,
                kind,
            }),
{}

proof fn retype_push_preserves_wf(
    parent_base: u64,
    parent_size: u64,
    children: Seq<ChildObject>,
    child: ChildObject,
)
    requires
        parent_wf_spec(parent_base, parent_size),
        all_children_wf_spec(children),
        all_children_contained_spec(parent_base, parent_size, children),
        pairwise_disjoint_spec(children),
        child_wf_spec(child),
        child_contained_spec(parent_base, parent_size, child),
        disjoint_from_all_spec(children, child),
    ensures
        all_children_wf_spec(children.push(child)),
        all_children_contained_spec(parent_base, parent_size, children.push(child)),
        pairwise_disjoint_spec(children.push(child)),
{
    let new_seq = children.push(child);
    assert(new_seq.len() == children.len() + 1);
    assert forall|i: int| #![trigger new_seq[i]] 0 <= i < new_seq.len() ==> child_wf_spec(new_seq[i]) by {
        if 0 <= i && i < new_seq.len() {
            if i < children.len() {
                push_keeps_old_index(children, child, i);
                child_wf_from_all(children, i);
            } else {
                assert(i == children.len()) by(nonlinear_arith)
                    requires 0 <= i, i < new_seq.len(), !(i < children.len()), new_seq.len() == children.len() + 1;
                push_sets_last_index(children, child);
                assert(new_seq[i] == child);
            }
        }
    };
    assert forall|i: int| #![trigger new_seq[i]] 0 <= i < new_seq.len() ==> child_contained_spec(parent_base, parent_size, new_seq[i]) by {
        if 0 <= i && i < new_seq.len() {
            if i < children.len() {
                push_keeps_old_index(children, child, i);
                child_contained_from_all(parent_base, parent_size, children, i);
            } else {
                assert(i == children.len()) by(nonlinear_arith)
                    requires 0 <= i, i < new_seq.len(), !(i < children.len()), new_seq.len() == children.len() + 1;
                push_sets_last_index(children, child);
                assert(new_seq[i] == child);
            }
        }
    };
    assert forall|i: int, j: int| #![trigger new_seq[i], new_seq[j]]
        0 <= i && i < j && j < new_seq.len() ==> child_disjoint_spec(new_seq[i], new_seq[j]) by {
        if 0 <= i && i < j && j < new_seq.len() {
            if j < children.len() {
                child_disjoint_from_pairwise(children, i, j);
            } else {
                assert(j == children.len()) by(nonlinear_arith)
                    requires 0 <= j, j < new_seq.len(), !(j < children.len()), new_seq.len() == children.len() + 1;
                assert(i < children.len()) by(nonlinear_arith)
                    requires 0 <= i, i < j, j == children.len();
                push_keeps_old_index(children, child, i);
                push_sets_last_index(children, child);
                assert(new_seq[i] == children[i]);
                assert(new_seq[j] == child);
                assert(child_disjoint_spec(children[i], child));
            }
        }
    };
}

// r[verify sel4-untyped.retype]
// r[verify sel4-untyped.child-layout]
pub proof fn retype_success_keeps_wf(
    parent_base: u64,
    parent_size: u64,
    children: Seq<ChildObject>,
    child: ChildObject,
)
    requires
        parent_wf_spec(parent_base, parent_size),
        all_children_wf_spec(children),
        all_children_contained_spec(parent_base, parent_size, children),
        pairwise_disjoint_spec(children),
        retype_enabled_spec(parent_base, parent_size, children, child),
    ensures
        all_children_wf_spec(retype_transition_spec(parent_base, parent_size, children, child)),
        all_children_contained_spec(parent_base, parent_size, retype_transition_spec(parent_base, parent_size, children, child)),
        pairwise_disjoint_spec(retype_transition_spec(parent_base, parent_size, children, child)),
{
    retype_push_preserves_wf(parent_base, parent_size, children, child);
}

// r[impl sel4-untyped.child-layout]
// r[verify sel4-untyped.child-layout]
pub proof fn retype_rejects_overlap(
    parent_base: u64,
    parent_size: u64,
    children: Seq<ChildObject>,
    child: ChildObject,
)
    requires
        parent_wf_spec(parent_base, parent_size),
        all_children_wf_spec(children),
        exists|i: int| #![trigger children[i]] 0 <= i < children.len() && !child_disjoint_spec(children[i], child),
    ensures
        !retype_enabled_spec(parent_base, parent_size, children, child),
        retype_transition_spec(parent_base, parent_size, children, child) == children,
{
    let i = choose|i: int| #![trigger children[i]] 0 <= i < children.len() && !child_disjoint_spec(children[i], child);
    assert(0 <= i < children.len());
    assert(!child_disjoint_spec(children[i], child));
    assert(!disjoint_from_all_spec(children, child)) by {
        assert(!child_disjoint_spec(children[i], child));
    }
}

// r[impl sel4-untyped.reclaim]
// r[verify sel4-untyped.reclaim]
pub proof fn reclaim_clears_children(parent_base: u64, parent_size: u64, children: Seq<ChildObject>)
    requires
        parent_wf_spec(parent_base, parent_size),
        all_children_wf_spec(children),
        all_children_contained_spec(parent_base, parent_size, children),
        pairwise_disjoint_spec(children),
    ensures
        reclaim_transition_spec(children).len() == 0,
        all_children_wf_spec(reclaim_transition_spec(children)),
        all_children_contained_spec(parent_base, parent_size, reclaim_transition_spec(children)),
        pairwise_disjoint_spec(reclaim_transition_spec(children)),
{}

} // verus!

impl Default for UntypedMemory {
    fn default() -> Self {
        Self::new(0x1000, 0x1000)
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify sel4-untyped.retype]
    #[test]
    fn test_retype_creates_contained_children() {
        let mut ut = UntypedMemory::new(0x1000, 0x4000);
        assert!(ut.retype(0x1000, 0x1000, 0x1000, SEL4_OBJ_ENDPOINT));
        assert!(ut.retype(0x2000, 0x1000, 0x1000, SEL4_OBJ_FRAME));
        assert_eq!(ut.children.len(), 2);
        assert_eq!(ut.children[0].base, 0x1000);
        assert_eq!(ut.children[1].base, 0x2000);
    }

    // r[verify sel4-untyped.child-layout]
    #[test]
    fn test_overlap_rejected() {
        let mut ut = UntypedMemory::new(0x1000, 0x4000);
        assert!(ut.retype(0x1000, 0x1000, 0x1000, SEL4_OBJ_ENDPOINT));
        assert!(!ut.retype(0x1800, 0x1000, 0x1000, SEL4_OBJ_FRAME));
        assert_eq!(ut.children.len(), 1);
    }

    // r[verify sel4-untyped.child-layout]
    #[test]
    fn test_misaligned_child_rejected() {
        let mut ut = UntypedMemory::new(0x1000, 0x4000);
        assert!(!ut.retype(0x1800, 0x1000, 0x1000, SEL4_OBJ_ENDPOINT));
        assert_eq!(ut.children.len(), 0);
    }

    // r[verify sel4-untyped.reclaim]
    #[test]
    fn test_revoke_and_reset_clear_children() {
        let mut ut = UntypedMemory::new(0x1000, 0x4000);
        assert!(ut.retype(0x1000, 0x1000, 0x1000, SEL4_OBJ_ENDPOINT));
        assert!(ut.retype(0x2000, 0x1000, 0x1000, SEL4_OBJ_FRAME));
        ut.revoke();
        assert!(ut.children.is_empty());
        assert!(ut.retype(0x3000, 0x1000, 0x1000, SEL4_OBJ_FRAME));
        ut.reset();
        assert!(ut.children.is_empty());
    }

    proptest! {
        // r[verify sel4-untyped.child-layout]
        #[test]
        fn prop_overlapping_second_child_rejected(mult in 1u64..16u64) {
            let base = mult * 0x1000;
            let mut ut = UntypedMemory::new(base, 0x4000);
            prop_assert!(ut.retype(base, 0x1000, 0x1000, SEL4_OBJ_ENDPOINT));
            prop_assert!(!ut.retype(base + 0x800, 0x1000, 0x1000, SEL4_OBJ_FRAME));
        }

        // r[verify sel4-untyped.reclaim]
        #[test]
        fn prop_reclaim_clears_random_valid_children(mult in 1u64..16u64) {
            let base = mult * 0x1000;
            let mut ut = UntypedMemory::new(base, 0x4000);
            let _ = ut.retype(base, 0x1000, 0x1000, SEL4_OBJ_ENDPOINT);
            let _ = ut.retype(base + 0x1000, 0x1000, 0x1000, SEL4_OBJ_FRAME);
            ut.revoke();
            prop_assert!(ut.children.is_empty());
        }
    }
}
