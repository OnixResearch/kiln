//! Verified UCAN opaque caveat identifier preservation.
//!
//! This module proves only finite byte-identifier preservation over caller- or
//! shell-supplied caveat identifier bytes. Caveat-document validation, payload
//! parsing, length-prefix encoding provenance/injectivity, policy semantics,
//! clocks, counters, backend truth, and cryptography remain runtime boundaries.

#![allow(clippy::ptr_arg)]

use vstd::prelude::*;

verus! {

pub open spec fn bytes_equal_spec(a: Seq<u8>, b: Seq<u8>) -> bool {
    a =~= b
}

pub open spec fn child_has_parent_caveat_id_spec(
    child_ids: Seq<Vec<u8>>,
    parent_id: Vec<u8>,
) -> bool {
    exists|k: int| 0 <= k < child_ids.len()
        && #[trigger] bytes_equal_spec(child_ids[k]@, parent_id@)
}

pub open spec fn parent_caveat_ids_preserved_spec(
    parent_ids: Seq<Vec<u8>>,
    child_ids: Seq<Vec<u8>>,
) -> bool {
    forall|i: int| #![trigger parent_ids[i]] 0 <= i < parent_ids.len() ==>
        child_has_parent_caveat_id_spec(child_ids, parent_ids[i])
}

// r[impl ucan-caveat-preservation.byte-equality]
pub fn bytes_equal(a: &Vec<u8>, b: &Vec<u8>) -> (result: bool)
    ensures result == bytes_equal_spec(a@, b@),
{
    if a.len() != b.len() {
        return false;
    }
    let mut i: usize = 0;
    while i < a.len()
        invariant
            0 <= i <= a.len(),
            a.len() == b.len(),
            forall|k: int| 0 <= k < i ==> a@[k] == b@[k],
        decreases a.len() - i,
    {
        if a[i] != b[i] {
            assert(a@[i as int] != b@[i as int]);
            assert(!(a@ =~= b@));
            return false;
        }
        i = i + 1;
    }
    assert(a@ =~= b@);
    true
}

// r[impl ucan-caveat-preservation.child-contains-parent-id]
pub fn child_has_parent_caveat_id(
    child_ids: &Vec<Vec<u8>>,
    parent_id: &Vec<u8>,
) -> (result: bool)
    ensures result == child_has_parent_caveat_id_spec(child_ids@, *parent_id),
{
    let mut k: usize = 0;
    while k < child_ids.len()
        invariant
            0 <= k <= child_ids.len(),
            forall|j: int| 0 <= j < k ==>
                !bytes_equal_spec(child_ids@[j]@, parent_id@),
        decreases child_ids.len() - k,
    {
        if bytes_equal(&child_ids[k], parent_id) {
            assert(bytes_equal_spec(child_ids@[k as int]@, parent_id@));
            return true;
        }
        assert(!bytes_equal_spec(child_ids@[k as int]@, parent_id@));
        k = k + 1;
    }
    assert forall|j: int| 0 <= j < child_ids@.len() implies
        !bytes_equal_spec(child_ids@[j]@, parent_id@) by {
    }
    false
}

// r[impl ucan-caveat-preservation.parent-ids-preserved]
pub fn parent_caveat_ids_preserved(
    parent_ids: &Vec<Vec<u8>>,
    child_ids: &Vec<Vec<u8>>,
) -> (result: bool)
    ensures result == parent_caveat_ids_preserved_spec(parent_ids@, child_ids@),
{
    let mut i: usize = 0;
    while i < parent_ids.len()
        invariant
            0 <= i <= parent_ids.len(),
            forall|j: int| #![trigger parent_ids@[j]] 0 <= j < i ==>
                child_has_parent_caveat_id_spec(child_ids@, parent_ids@[j]),
        decreases parent_ids.len() - i,
    {
        if !child_has_parent_caveat_id(child_ids, &parent_ids[i]) {
            assert(!child_has_parent_caveat_id_spec(child_ids@, parent_ids@[i as int]));
            assert(!parent_caveat_ids_preserved_spec(parent_ids@, child_ids@));
            return false;
        }
        assert(child_has_parent_caveat_id_spec(child_ids@, parent_ids@[i as int]));
        assert forall|j: int| #![trigger parent_ids@[j]] 0 <= j < i + 1 implies
            child_has_parent_caveat_id_spec(child_ids@, parent_ids@[j]) by {
            if j < i {
                assert(parent_ids@[j] == parent_ids@[j]);
            } else {
                assert(j == i);
            }
        }
        i = i + 1;
        assert forall|j: int| #![trigger parent_ids@[j]] 0 <= j < i implies
            child_has_parent_caveat_id_spec(child_ids@, parent_ids@[j]) by {
            assert(parent_ids@[j] == parent_ids@[j]);
        };
    }
    true
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    fn id(bytes: &[u8]) -> Vec<u8> {
        bytes.to_vec()
    }

    // r[verify ucan-caveat-preservation.parent-ids-preserved]
    #[test]
    fn accepts_empty_parent_ids() {
        assert!(parent_caveat_ids_preserved(&Vec::new(), &Vec::new()));
    }

    // r[verify ucan-caveat-preservation.parent-ids-preserved]
    // r[verify ucan-caveat-preservation.child-contains-parent-id]
    // r[verify ucan-caveat-preservation.byte-equality]
    #[test]
    fn accepts_child_added_caveat_ids() {
        let parent = vec![id(b"a"), id(b"b")];
        let child = vec![id(b"extra"), id(b"b"), id(b"a")];
        assert!(parent_caveat_ids_preserved(&parent, &child));
    }

    // r[verify ucan-caveat-preservation.parent-ids-preserved]
    // r[verify ucan-caveat-preservation.child-contains-parent-id]
    #[test]
    fn rejects_missing_parent_caveat_id() {
        let parent = vec![id(b"a"), id(b"b")];
        let child = vec![id(b"a")];
        assert!(!parent_caveat_ids_preserved(&parent, &child));
    }

    // r[verify ucan-caveat-preservation.byte-equality]
    // r[verify ucan-caveat-preservation.parent-ids-preserved]
    #[test]
    fn rejects_changed_parent_caveat_id_bytes() {
        let parent = vec![id(b"a")];
        let child = vec![id(b"A")];
        assert!(!parent_caveat_ids_preserved(&parent, &child));
    }
}
