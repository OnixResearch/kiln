//! Verified seL4-style VSpace mapping primitives.
//!
//! Models one abstract address-space root as a set of disjoint virtual-memory
//! mappings. Map succeeds only for aligned non-overlapping regions. Unmap
//! removes exactly the mapping at one virtual base. Effective rights are the
//! requested rights masked by the capability rights used to install the
//! mapping.

use vstd::prelude::*;

use crate::region::regions_disjoint;
#[cfg(verus_keep_ghost)]
use crate::region::regions_disjoint_spec;
use crate::sel4_cap_rights::mask_rights;
#[cfg(verus_keep_ghost)]
use crate::sel4_cap_rights::mask_rights_spec;
#[cfg(verus_keep_ghost)]
use crate::sel4_cap_rights::masking_never_adds;
#[cfg(verus_keep_ghost)]
use crate::sel4_cap_rights::rights_subset_spec;

verus! {

pub const SEL4_VM_READ: u64 = 1u64 << 0;
pub const SEL4_VM_WRITE: u64 = 1u64 << 1;
pub const SEL4_VM_EXECUTE: u64 = 1u64 << 2;

#[derive(Copy, Clone)]
pub struct Mapping {
    pub vaddr: u64,
    pub size: u64,
    pub frame: usize,
    pub rights: u64,
}

pub struct VSpace {
    pub mappings: Vec<Mapping>,
}

// r[impl sel4-vspace.rights]
// r[depends sel4-vspace.rights]
pub open spec fn effective_rights_spec(cap_rights: u64, requested_rights: u64) -> u64 {
    mask_rights_spec(cap_rights, requested_rights)
}

// r[impl sel4-vspace.map]
// r[depends sel4-vspace.map]
pub open spec fn mapping_wf_spec(mapping: Mapping) -> bool {
    &&& mapping.size > 0
    &&& mapping.vaddr as int + mapping.size as int <= u64::MAX as int
    &&& mapping.vaddr % mapping.size == 0
}

// r[depends sel4-vspace.map]
// r[depends sel4-vspace.unmap]
pub open spec fn mappings_disjoint_spec(left: Mapping, right: Mapping) -> bool {
    regions_disjoint_spec(left.vaddr, left.size, right.vaddr, right.size)
}

// r[depends sel4-vspace.map]
// r[depends sel4-vspace.unmap]
pub open spec fn all_mappings_wf_spec(mappings: Seq<Mapping>) -> bool {
    forall|i: int| #![trigger mappings[i]] 0 <= i < mappings.len() ==> mapping_wf_spec(mappings[i])
}

// r[depends sel4-vspace.map]
// r[depends sel4-vspace.unmap]
pub open spec fn disjoint_from_all_spec(mappings: Seq<Mapping>, mapping: Mapping) -> bool {
    forall|i: int| #![trigger mappings[i]] 0 <= i < mappings.len() ==> mappings_disjoint_spec(mappings[i], mapping)
}

// r[depends sel4-vspace.map]
// r[depends sel4-vspace.unmap]
pub open spec fn pairwise_disjoint_spec(mappings: Seq<Mapping>) -> bool {
    forall|i: int, j: int|
        #![trigger mappings[i], mappings[j]]
        0 <= i && i < j && j < mappings.len() ==> mappings_disjoint_spec(mappings[i], mappings[j])
}

// r[depends sel4-vspace.unmap]
pub open spec fn mapping_present_spec(mappings: Seq<Mapping>, vaddr: u64) -> bool {
    exists|i: int| #![trigger mappings[i]] 0 <= i < mappings.len() && mappings[i].vaddr == vaddr
}

pub open spec fn remove_mapping_prefix_spec(mappings: Seq<Mapping>, vaddr: u64, upto: int) -> Seq<Mapping>
    decreases (if upto >= 0 { upto } else { 0 })
{
    if upto <= 0 {
        seq![]
    } else {
        let prefix = remove_mapping_prefix_spec(mappings, vaddr, upto - 1);
        if mappings[upto - 1].vaddr == vaddr {
            prefix
        } else {
            prefix.push(mappings[upto - 1])
        }
    }
}

// r[impl sel4-vspace.map]
// r[depends sel4-vspace.map]
pub open spec fn map_enabled_spec(
    mappings: Seq<Mapping>,
    vaddr: u64,
    size: u64,
    frame: usize,
    cap_rights: u64,
    requested_rights: u64,
) -> bool {
    let mapping = Mapping {
        vaddr,
        size,
        frame,
        rights: 0u64,
    };
    &&& mapping_wf_spec(mapping)
    &&& disjoint_from_all_spec(mappings, mapping)
}

// r[impl sel4-vspace.map]
// r[depends sel4-vspace.map]
pub open spec fn map_transition_spec(
    mappings: Seq<Mapping>,
    vaddr: u64,
    size: u64,
    frame: usize,
    cap_rights: u64,
    requested_rights: u64,
) -> Seq<Mapping> {
    if map_enabled_spec(mappings, vaddr, size, frame, cap_rights, requested_rights) {
        mappings.push(Mapping {
            vaddr,
            size,
            frame,
            rights: effective_rights_spec(cap_rights, requested_rights),
        })
    } else {
        mappings
    }
}

// r[impl sel4-vspace.unmap]
// r[depends sel4-vspace.unmap]
pub open spec fn unmap_result_spec(mappings: Seq<Mapping>, vaddr: u64) -> bool {
    mapping_present_spec(mappings, vaddr)
}

// r[impl sel4-vspace.unmap]
// r[depends sel4-vspace.unmap]
pub open spec fn unmap_transition_spec(mappings: Seq<Mapping>, vaddr: u64) -> Seq<Mapping> {
    remove_mapping_prefix_spec(mappings, vaddr, mappings.len() as int)
}

fn mapping_wf(mapping: Mapping) -> (result: bool)
    ensures result == mapping_wf_spec(mapping)
{
    mapping.size > 0
        && mapping.vaddr <= u64::MAX - mapping.size
        && mapping.vaddr % mapping.size == 0
}

fn mappings_disjoint(left: Mapping, right: Mapping) -> (result: bool)
    requires
        mapping_wf_spec(left),
        mapping_wf_spec(right),
    ensures result == mappings_disjoint_spec(left, right)
{
    regions_disjoint(left.vaddr, left.size, right.vaddr, right.size)
}

fn clone_without_vaddr(mappings: &[Mapping], vaddr: u64) -> (result: Vec<Mapping>)
    ensures result@ == unmap_transition_spec(mappings@, vaddr)
{
    let mut result: Vec<Mapping> = Vec::new();
    let mut i: usize = 0;
    while i < mappings.len()
        invariant
            0 <= i <= mappings@.len(),
            result@ == remove_mapping_prefix_spec(mappings@, vaddr, i as int),
        decreases mappings@.len() - i,
    {
        let mapping = mappings[i];
        if mapping.vaddr != vaddr {
            result.push(mapping);
        }
        i = i + 1;
    }
    result
}

impl VSpace {
    // r[impl sel4-vspace.map]
    // r[impl sel4-vspace.unmap]
    pub open spec fn mapping_view(&self) -> Seq<Mapping> {
        self.mappings@
    }

    // r[impl sel4-vspace.map]
    // r[impl sel4-vspace.unmap]
    pub open spec fn wf(&self) -> bool {
        &&& all_mappings_wf_spec(self.mapping_view())
        &&& pairwise_disjoint_spec(self.mapping_view())
    }

    // r[impl sel4-vspace.map]
    pub fn new() -> (result: VSpace)
        ensures
            result.mapping_view().len() == 0,
            result.wf(),
    {
        VSpace { mappings: Vec::new() }
    }

    // r[impl sel4-vspace.map]
    pub fn map(
        &mut self,
        vaddr: u64,
        size: u64,
        frame: usize,
        cap_rights: u64,
        requested_rights: u64,
    ) -> (result: bool)
        requires old(self).wf()
        ensures
            self.wf(),
            result ==> map_enabled_spec(old(self).mapping_view(), vaddr, size, frame, cap_rights, requested_rights),
            result ==> self.mapping_view() == map_transition_spec(
                old(self).mapping_view(),
                vaddr,
                size,
                frame,
                cap_rights,
                requested_rights,
            ),
            !result ==> self.mapping_view() == old(self).mapping_view(),
    {
        let candidate = Mapping {
            vaddr,
            size,
            frame,
            rights: 0u64,
        };
        let ghost old_view = old(self).mapping_view();
        if !mapping_wf(candidate) {
            return false;
        }
        let mut i: usize = 0;
        while i < self.mappings.len()
            invariant
                self.mapping_view() =~= old_view,
                old(self).wf(),
                mapping_wf_spec(candidate),
                0 <= i <= old_view.len(),
                forall|j: int| #![trigger old_view[j]] 0 <= j < i ==> mappings_disjoint_spec(old_view[j], candidate),
            decreases old_view.len() - i,
        {
            let current = self.mappings[i];
            proof {
                assert(mapping_wf_spec(current));
            }
            if !mappings_disjoint(current, candidate) {
                proof {
                    assert(current == old_view[i as int]);
                    assert(0 <= i as int && (i as int) < old_view.len());
                    assert(!mappings_disjoint_spec(current, candidate));
                    assert(!disjoint_from_all_spec(old_view, candidate)) by {
                        assert(!mappings_disjoint_spec(old_view[i as int], candidate));
                    }
                    assert(self.mapping_view() == old_view);
                }
                return false;
            }
            i = i + 1;
        }
        proof {
            assert(disjoint_from_all_spec(old_view, candidate));
        }
        let rights = effective_rights(cap_rights, requested_rights);
        proof {
            assert(rights == effective_rights_spec(cap_rights, requested_rights));
        }
        let mapping = Mapping {
            vaddr,
            size,
            frame,
            rights,
        };
        self.mappings.push(mapping);
        proof {
            disjoint_from_all_ignores_frame_and_rights(old_view, candidate, frame, rights);
            assert(disjoint_from_all_spec(old_view, candidate) == disjoint_from_all_spec(old_view, mapping));
            map_transition_preserves_wf(old_view, mapping);
        }
        true
    }

    // r[impl sel4-vspace.unmap]
    pub fn unmap(&mut self, vaddr: u64) -> (result: bool)
        requires old(self).wf()
        ensures
            self.wf(),
            result == unmap_result_spec(old(self).mapping_view(), vaddr),
            self.mapping_view() == unmap_transition_spec(old(self).mapping_view(), vaddr),
    {
        let ghost old_view = old(self).mapping_view();
        let present = exists_mapping(self, vaddr);
        let new_mappings = clone_without_vaddr(&self.mappings, vaddr);
        self.mappings = new_mappings;
        proof {
            remove_transition_preserves_wf(old_view, vaddr);
        }
        present
    }
}

fn exists_mapping(vspace: &VSpace, vaddr: u64) -> (result: bool)
    ensures result == mapping_present_spec(vspace.mapping_view(), vaddr)
{
    let mut i: usize = 0;
    let mut found = false;
    while i < vspace.mappings.len() && !found
        invariant
            0 <= i <= vspace.mapping_view().len(),
            !found ==> forall|j: int| #![trigger vspace.mapping_view()[j]] 0 <= j < i ==> vspace.mapping_view()[j].vaddr != vaddr,
            found ==> mapping_present_spec(vspace.mapping_view(), vaddr),
        decreases vspace.mapping_view().len() - i,
    {
        if vspace.mappings[i].vaddr == vaddr {
            proof {
                assert(vspace.mapping_view()[i as int].vaddr == vaddr);
                assert(mapping_present_spec(vspace.mapping_view(), vaddr));
            }
            found = true;
        }
        i = i + 1;
    }
    found
}

// r[impl sel4-vspace.rights]
pub fn effective_rights(cap_rights: u64, requested_rights: u64) -> (result: u64)
    ensures result == effective_rights_spec(cap_rights, requested_rights)
{
    mask_rights(cap_rights, requested_rights)
}

proof fn mapping_disjoint_ignores_frame_and_rights(left: Mapping, right: Mapping, frame: usize, rights: u64)
    ensures
        mappings_disjoint_spec(left, right)
            == mappings_disjoint_spec(left, Mapping {
                vaddr: right.vaddr,
                size: right.size,
                frame,
                rights,
            }),
{}

proof fn disjoint_from_all_ignores_frame_and_rights(mappings: Seq<Mapping>, right: Mapping, frame: usize, rights: u64)
    ensures
        disjoint_from_all_spec(mappings, right)
            == disjoint_from_all_spec(mappings, Mapping {
                vaddr: right.vaddr,
                size: right.size,
                frame,
                rights,
            }),
{
    assert forall|j: int| #![trigger mappings[j]] 0 <= j < mappings.len() ==>
        mappings_disjoint_spec(mappings[j], right)
            == mappings_disjoint_spec(mappings[j], Mapping {
                vaddr: right.vaddr,
                size: right.size,
                frame,
                rights,
            }) by {
        mapping_disjoint_ignores_frame_and_rights(mappings[j], right, frame, rights);
    };
}

proof fn push_keeps_old_index(seq: Seq<Mapping>, item: Mapping, i: int)
    requires 0 <= i < seq.len()
    ensures seq.push(item)[i] == seq[i]
{}

proof fn push_sets_last_index(seq: Seq<Mapping>, item: Mapping)
    ensures seq.push(item)[seq.len() as int] == item
{}

proof fn map_transition_preserves_wf(mappings: Seq<Mapping>, mapping: Mapping)
    requires
        all_mappings_wf_spec(mappings),
        pairwise_disjoint_spec(mappings),
        mapping_wf_spec(mapping),
        disjoint_from_all_spec(mappings, mapping),
    ensures
        all_mappings_wf_spec(mappings.push(mapping)),
        pairwise_disjoint_spec(mappings.push(mapping)),
{
    let new_seq = mappings.push(mapping);
    assert(new_seq.len() == mappings.len() + 1);
    assert forall|i: int| #![trigger new_seq[i]]
        0 <= i && i < new_seq.len() ==> mapping_wf_spec(new_seq[i]) by {
        if 0 <= i && i < new_seq.len() {
            if i < mappings.len() {
                push_keeps_old_index(mappings, mapping, i);
                assert(mapping_wf_spec(mappings[i]));
            } else {
                assert(i == mappings.len()) by(nonlinear_arith)
                    requires 0 <= i, i < new_seq.len(), !(i < mappings.len()), new_seq.len() == mappings.len() + 1;
                push_sets_last_index(mappings, mapping);
                assert(new_seq[i] == mapping);
                assert(mapping_wf_spec(mapping));
            }
        }
    };
    assert forall|i: int, j: int| #![trigger new_seq[i], new_seq[j]]
        0 <= i && i < j && j < new_seq.len() ==> mappings_disjoint_spec(new_seq[i], new_seq[j]) by {
        if 0 <= i && i < j && j < new_seq.len() {
            if j < mappings.len() {
                assert(mappings_disjoint_spec(mappings[i], mappings[j]));
            } else {
                assert(j == mappings.len()) by(nonlinear_arith)
                    requires 0 <= j, j < new_seq.len(), !(j < mappings.len()), new_seq.len() == mappings.len() + 1;
                assert(i < mappings.len()) by(nonlinear_arith)
                    requires 0 <= i, i < j, j == mappings.len();
                push_keeps_old_index(mappings, mapping, i);
                push_sets_last_index(mappings, mapping);
                assert(new_seq[i] == mappings[i]);
                assert(new_seq[j] == mapping);
                assert(mappings_disjoint_spec(mappings[i], mapping));
            }
        }
    };
}

proof fn remove_prefix_member_origin(mappings: Seq<Mapping>, vaddr: u64, upto: int, k: int)
    requires
        0 <= upto <= mappings.len(),
    ensures
        0 <= k && k < remove_mapping_prefix_spec(mappings, vaddr, upto).len() ==> exists|orig: int|
            0 <= orig < upto
            && mappings[orig].vaddr != vaddr
            && remove_mapping_prefix_spec(mappings, vaddr, upto)[k] == mappings[orig],
    decreases upto,
{
    if upto == 0 {
    } else {
        let prefix = remove_mapping_prefix_spec(mappings, vaddr, upto - 1);
        if 0 <= k && k < remove_mapping_prefix_spec(mappings, vaddr, upto).len() {
            if mappings[upto - 1].vaddr == vaddr {
                remove_prefix_member_origin(mappings, vaddr, upto - 1, k);
            } else if k < prefix.len() {
                remove_prefix_member_origin(mappings, vaddr, upto - 1, k);
                assert(remove_mapping_prefix_spec(mappings, vaddr, upto) == prefix.push(mappings[upto - 1]));
                assert(prefix.push(mappings[upto - 1])[k] == prefix[k]);
            } else {
                assert(k == prefix.len());
                assert(remove_mapping_prefix_spec(mappings, vaddr, upto) == prefix.push(mappings[upto - 1]));
                assert(prefix.push(mappings[upto - 1])[k] == mappings[upto - 1]);
                assert(0 <= upto - 1 && upto - 1 < upto
                    && mappings[upto - 1].vaddr != vaddr
                    && remove_mapping_prefix_spec(mappings, vaddr, upto)[k] == mappings[upto - 1]);
            }
        }
    }
}

proof fn remove_prefix_preserves_wf(mappings: Seq<Mapping>, vaddr: u64, upto: int)
    requires
        all_mappings_wf_spec(mappings),
        pairwise_disjoint_spec(mappings),
        0 <= upto <= mappings.len(),
    ensures
        all_mappings_wf_spec(remove_mapping_prefix_spec(mappings, vaddr, upto)),
        pairwise_disjoint_spec(remove_mapping_prefix_spec(mappings, vaddr, upto)),
    decreases upto,
{
    if upto == 0 {
    } else {
        remove_prefix_preserves_wf(mappings, vaddr, upto - 1);
        if mappings[upto - 1].vaddr != vaddr {
            let prefix = remove_mapping_prefix_spec(mappings, vaddr, upto - 1);
            assert(all_mappings_wf_spec(prefix));
            assert(pairwise_disjoint_spec(prefix));
            assert(disjoint_from_all_spec(prefix, mappings[upto - 1])) by {
                assert forall|idx: int| #![trigger prefix[idx]]
                    0 <= idx && idx < prefix.len() ==> mappings_disjoint_spec(prefix[idx], mappings[upto - 1]) by {
                    if 0 <= idx && idx < prefix.len() {
                        assert(0 <= upto - 1 && upto - 1 <= mappings.len());
                        remove_prefix_member_origin(mappings, vaddr, upto - 1, idx);
                        assert(exists|orig: int|
                            0 <= orig < upto - 1
                            && mappings[orig].vaddr != vaddr
                            && remove_mapping_prefix_spec(mappings, vaddr, upto - 1)[idx] == mappings[orig]);
                        let orig = choose|orig: int|
                            0 <= orig < upto - 1
                            && mappings[orig].vaddr != vaddr
                            && remove_mapping_prefix_spec(mappings, vaddr, upto - 1)[idx] == mappings[orig];
                        assert(0 <= orig && orig < upto - 1);
                        assert(pairwise_disjoint_spec(mappings));
                        assert(mappings_disjoint_spec(mappings[orig], mappings[upto - 1]));
                        assert(prefix[idx] == remove_mapping_prefix_spec(mappings, vaddr, upto - 1)[idx]);
                        assert(mappings_disjoint_spec(prefix[idx], mappings[upto - 1]));
                    }
                }
            };
            map_transition_preserves_wf(prefix, mappings[upto - 1]);
        }
    }
}

// r[verify sel4-vspace.map]
pub proof fn map_success_keeps_wf(mappings: Seq<Mapping>, vaddr: u64, size: u64, frame: usize, cap_rights: u64, requested_rights: u64)
    requires
        all_mappings_wf_spec(mappings),
        pairwise_disjoint_spec(mappings),
        map_enabled_spec(mappings, vaddr, size, frame, cap_rights, requested_rights),
    ensures
        all_mappings_wf_spec(map_transition_spec(mappings, vaddr, size, frame, cap_rights, requested_rights)),
        pairwise_disjoint_spec(map_transition_spec(mappings, vaddr, size, frame, cap_rights, requested_rights)),
{
    let mapping = Mapping {
        vaddr,
        size,
        frame,
        rights: effective_rights_spec(cap_rights, requested_rights),
    };
    map_transition_preserves_wf(mappings, mapping);
}

// r[verify sel4-vspace.unmap]
pub proof fn unmap_keeps_wf(mappings: Seq<Mapping>, vaddr: u64)
    requires
        all_mappings_wf_spec(mappings),
        pairwise_disjoint_spec(mappings),
    ensures
        all_mappings_wf_spec(unmap_transition_spec(mappings, vaddr)),
        pairwise_disjoint_spec(unmap_transition_spec(mappings, vaddr)),
{
    remove_prefix_preserves_wf(mappings, vaddr, mappings.len() as int);
}

proof fn remove_transition_preserves_wf(mappings: Seq<Mapping>, vaddr: u64)
    requires
        all_mappings_wf_spec(mappings),
        pairwise_disjoint_spec(mappings),
    ensures
        all_mappings_wf_spec(unmap_transition_spec(mappings, vaddr)),
        pairwise_disjoint_spec(unmap_transition_spec(mappings, vaddr)),
{
    remove_prefix_preserves_wf(mappings, vaddr, mappings.len() as int);
}

// r[verify sel4-vspace.rights]
pub proof fn effective_rights_are_restricted(cap_rights: u64, requested_rights: u64)
    ensures rights_subset_spec(effective_rights_spec(cap_rights, requested_rights), cap_rights)
{
    masking_never_adds(cap_rights, requested_rights);
}

} // verus!

impl Default for VSpace {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify sel4-vspace.map]
    #[test]
    fn test_aligned_non_overlapping_map_succeeds() {
        let mut vspace = VSpace::new();
        assert!(vspace.map(0x1000, 0x1000, 1, SEL4_VM_READ | SEL4_VM_WRITE, SEL4_VM_READ));
        assert_eq!(vspace.mappings.len(), 1);
        assert_eq!(vspace.mappings[0].vaddr, 0x1000);
        assert_eq!(vspace.mappings[0].size, 0x1000);
        assert_eq!(vspace.mappings[0].rights, SEL4_VM_READ);
    }

    // r[verify sel4-vspace.map]
    #[test]
    fn test_overlap_is_rejected() {
        let mut vspace = VSpace::new();
        assert!(vspace.map(0x2000, 0x1000, 1, SEL4_VM_READ, SEL4_VM_READ));
        assert!(!vspace.map(0x2800, 0x1000, 2, SEL4_VM_READ, SEL4_VM_READ));
        assert_eq!(vspace.mappings.len(), 1);
        assert_eq!(vspace.mappings[0].frame, 1);
    }

    // r[verify sel4-vspace.unmap]
    #[test]
    fn test_unmap_removes_only_target() {
        let mut vspace = VSpace::new();
        assert!(vspace.map(0x3000, 0x1000, 1, SEL4_VM_READ, SEL4_VM_READ));
        assert!(vspace.map(0x5000, 0x1000, 2, SEL4_VM_READ | SEL4_VM_WRITE, SEL4_VM_READ | SEL4_VM_WRITE));
        assert!(vspace.unmap(0x3000));
        assert_eq!(vspace.mappings.len(), 1);
        assert_eq!(vspace.mappings[0].vaddr, 0x5000);
        assert_eq!(vspace.mappings[0].frame, 2);
    }

    // r[verify sel4-vspace.rights]
    #[test]
    fn test_readonly_cap_yields_readonly_mapping() {
        let mut vspace = VSpace::new();
        assert!(vspace.map(0x7000, 0x1000, 7, SEL4_VM_READ, SEL4_VM_READ | SEL4_VM_WRITE,));
        assert_eq!(vspace.mappings[0].rights, SEL4_VM_READ);
    }

    // r[verify sel4-vspace.rights]
    proptest! {
        #[test]
        fn prop_effective_rights_subset(cap in any::<u64>(), requested in any::<u64>()) {
            let effective = effective_rights(cap, requested);
            prop_assert_eq!(effective & !cap, 0);
        }
    }

    // r[verify sel4-vspace.map]
    proptest! {
        #[test]
        fn prop_same_base_overlap_rejected(shift in 0u32..12u32, mult in 1u64..32u64) {
            let size = 1u64 << shift;
            let base = mult * size;
            let mut vspace = VSpace::new();
            prop_assert!(vspace.map(base, size, 1, SEL4_VM_READ, SEL4_VM_READ));
            prop_assert!(!vspace.map(base, size, 2, SEL4_VM_READ, SEL4_VM_READ));
        }
    }
}
