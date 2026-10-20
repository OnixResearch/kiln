//! Verified Zircon handle/object model.
//!
//! Handles carry per-handle rights and reference a stable kernel object
//! identity (`koid`). A handle is either bound to a process, in transit,
//! or closed.
#![allow(unknown_lints, excessive_file_length)]

use vstd::prelude::*;

verus! {

pub const ZX_HANDLE_BOUND: u8 = 0u8;
pub const ZX_HANDLE_IN_TRANSIT: u8 = 1u8;
pub const ZX_HANDLE_CLOSED: u8 = 2u8;

#[allow(unknown_lints, path_segment_repetition)]
pub struct ZirconHandle {
    pub koid: u64,
    pub rights: u64,
    pub state: u8,
    pub owner: u64,
}

impl ZirconHandle {
    pub open spec fn wf(&self) -> bool {
        self.state == ZX_HANDLE_BOUND
            || self.state == ZX_HANDLE_IN_TRANSIT
            || self.state == ZX_HANDLE_CLOSED
    }
}

// r[depends zircon-handle.ownership]
pub open spec fn is_live_spec(handle: ZirconHandle) -> bool {
    handle.state == ZX_HANDLE_BOUND || handle.state == ZX_HANDLE_IN_TRANSIT
}

// r[impl zircon-handle.ownership]
pub fn is_live(handle: &ZirconHandle) -> (result: bool)
    requires handle.wf()
    ensures result == is_live_spec(*handle)
{
    handle.state == ZX_HANDLE_BOUND || handle.state == ZX_HANDLE_IN_TRANSIT
}

// r[depends zircon-handle.ownership]
pub open spec fn is_bound_to_spec(handle: ZirconHandle, process: u64) -> bool {
    handle.state == ZX_HANDLE_BOUND && handle.owner == process
}

// r[impl zircon-handle.ownership]
pub fn is_bound_to(handle: &ZirconHandle, process: u64) -> (result: bool)
    requires handle.wf()
    ensures result == is_bound_to_spec(*handle, process)
{
    handle.state == ZX_HANDLE_BOUND && handle.owner == process
}

// r[depends zircon-handle.ownership]
pub open spec fn is_in_transit_to_spec(handle: ZirconHandle, endpoint: u64) -> bool {
    handle.state == ZX_HANDLE_IN_TRANSIT && handle.owner == endpoint
}

// r[impl zircon-handle.ownership]
pub fn is_in_transit_to(handle: &ZirconHandle, endpoint: u64) -> (result: bool)
    requires handle.wf()
    ensures result == is_in_transit_to_spec(*handle, endpoint)
{
    handle.state == ZX_HANDLE_IN_TRANSIT && handle.owner == endpoint
}

// r[depends zircon-handle.identity]
pub open spec fn same_object_spec(a: ZirconHandle, b: ZirconHandle) -> bool {
    a.koid == b.koid
}

// r[impl zircon-handle.identity]
pub fn same_object(a: &ZirconHandle, b: &ZirconHandle) -> (result: bool)
    requires a.wf(), b.wf()
    ensures result == same_object_spec(*a, *b)
{
    a.koid == b.koid
}

// r[impl zircon-handle.ownership]
pub fn bind(koid: u64, rights: u64, process: u64) -> (result: ZirconHandle)
    ensures
        result.wf(),
        result.koid == koid,
        result.rights == rights,
        is_bound_to_spec(result, process),
{
    ZirconHandle { koid, rights, state: ZX_HANDLE_BOUND, owner: process }
}

// r[impl zircon-handle.identity]
// r[impl zircon-handle.ownership]
pub fn duplicate(handle: &ZirconHandle, requested_rights: u64, new_owner: u64) -> (result: Option<ZirconHandle>)
    requires handle.wf()
    ensures
        result.is_some() <==> (handle.state == ZX_HANDLE_BOUND && crate::zircon_rights::can_duplicate_spec(handle.rights, requested_rights)),
        result.is_some() ==> {
            let h = result.unwrap();
            &&& h.wf()
            &&& h.koid == handle.koid
            &&& h.rights == requested_rights
            &&& is_bound_to_spec(h, new_owner)
        },
{
    if handle.state == ZX_HANDLE_BOUND && crate::zircon_rights::can_duplicate(handle.rights, requested_rights) {
        Some(ZirconHandle {
            koid: handle.koid,
            rights: requested_rights,
            state: ZX_HANDLE_BOUND,
            owner: new_owner,
        })
    } else {
        None
    }
}

// r[impl zircon-handle.identity]
// r[impl zircon-handle.ownership]
pub fn replace(handle: &ZirconHandle, requested_rights: u64, new_owner: u64) -> (result: Option<ZirconHandle>)
    requires handle.wf()
    ensures
        result.is_some() <==> (handle.state == ZX_HANDLE_BOUND && crate::zircon_rights::can_replace_spec(handle.rights, requested_rights)),
        result.is_some() ==> {
            let h = result.unwrap();
            &&& h.wf()
            &&& h.koid == handle.koid
            &&& h.rights == requested_rights
            &&& is_bound_to_spec(h, new_owner)
        },
{
    if handle.state == ZX_HANDLE_BOUND && crate::zircon_rights::can_replace(handle.rights, requested_rights) {
        Some(ZirconHandle {
            koid: handle.koid,
            rights: requested_rights,
            state: ZX_HANDLE_BOUND,
            owner: new_owner,
        })
    } else {
        None
    }
}

// r[impl zircon-handle.identity]
// r[impl zircon-handle.ownership]
pub fn transfer(handle: &ZirconHandle, endpoint: u64) -> (result: ZirconHandle)
    requires handle.wf(), handle.state == ZX_HANDLE_BOUND
    ensures
        result.wf(),
        result.koid == handle.koid,
        result.rights == handle.rights,
        is_in_transit_to_spec(result, endpoint),
{
    ZirconHandle {
        koid: handle.koid,
        rights: handle.rights,
        state: ZX_HANDLE_IN_TRANSIT,
        owner: endpoint,
    }
}

// r[impl zircon-handle.identity]
// r[impl zircon-handle.ownership]
pub fn receive(handle: &ZirconHandle, process: u64) -> (result: ZirconHandle)
    requires handle.wf(), handle.state == ZX_HANDLE_IN_TRANSIT
    ensures
        result.wf(),
        result.koid == handle.koid,
        result.rights == handle.rights,
        is_bound_to_spec(result, process),
{
    ZirconHandle {
        koid: handle.koid,
        rights: handle.rights,
        state: ZX_HANDLE_BOUND,
        owner: process,
    }
}

// r[impl zircon-handle.ownership]
pub fn close(handle: &ZirconHandle) -> (result: ZirconHandle)
    requires handle.wf()
    ensures
        result.wf(),
        result.koid == handle.koid,
        result.rights == handle.rights,
        result.state == ZX_HANDLE_CLOSED,
        !is_live_spec(result),
{
    ZirconHandle {
        koid: handle.koid,
        rights: handle.rights,
        state: ZX_HANDLE_CLOSED,
        owner: 0,
    }
}

// r[depends zircon-handle.lifetime]
pub open spec fn object_live_from_spec(handles: Seq<ZirconHandle>, koid: u64, from: int) -> bool
    decreases handles.len() - from
{
    if from >= handles.len() {
        false
    } else {
        (handles[from].koid == koid && is_live_spec(handles[from]))
            || object_live_from_spec(handles, koid, from + 1)
    }
}

// r[depends zircon-handle.lifetime]
pub open spec fn object_live_spec(handles: Seq<ZirconHandle>, koid: u64) -> bool {
    object_live_from_spec(handles, koid, 0)
}

#[allow(unknown_lints, no_recursion)]
fn object_live_from(handles: &[ZirconHandle], koid: u64, from: usize) -> (result: bool)
    requires
        from <= handles@.len(),
        forall|i: int| 0 <= i < handles@.len() ==> #[trigger] handles@[i].wf()
    ensures result == object_live_from_spec(handles@, koid, from as int)
    decreases handles@.len() - from
{
    if from == handles.len() {
        false
    } else {
        (handles[from].koid == koid && is_live(&handles[from])) || object_live_from(handles, koid, from + 1)
    }
}

// r[impl zircon-handle.lifetime]
pub fn object_live(handles: &[ZirconHandle], koid: u64) -> (result: bool)
    requires forall|i: int| 0 <= i < handles@.len() ==> #[trigger] handles@[i].wf()
    ensures result == object_live_spec(handles@, koid)
{
    object_live_from(handles, koid, 0)
}

// r[verify zircon-handle.identity]
proof fn duplicate_preserves_koid(handle: ZirconHandle, requested_rights: u64, new_owner: u64)
    requires handle.wf(), handle.state == ZX_HANDLE_BOUND, crate::zircon_rights::can_duplicate_spec(handle.rights, requested_rights)
    ensures (ZirconHandle { koid: handle.koid, rights: requested_rights, state: ZX_HANDLE_BOUND, owner: new_owner }).koid == handle.koid
{}

// r[verify zircon-handle.identity]
proof fn transfer_receive_preserves_koid(handle: ZirconHandle, endpoint: u64, process: u64)
    requires handle.wf(), handle.state == ZX_HANDLE_BOUND
    ensures (ZirconHandle { koid: handle.koid, rights: handle.rights, state: ZX_HANDLE_BOUND, owner: process }).koid == handle.koid
{}

// r[verify zircon-handle.ownership]
proof fn close_not_live(handle: ZirconHandle)
    requires handle.wf()
    ensures !is_live_spec((ZirconHandle { koid: handle.koid, rights: handle.rights, state: ZX_HANDLE_CLOSED, owner: 0 }))
{}

// r[verify zircon-handle.lifetime]
proof fn in_transit_keeps_object_live(handle: ZirconHandle, endpoint: u64)
    requires handle.wf(), handle.state == ZX_HANDLE_BOUND
    ensures object_live_spec(seq![ZirconHandle { koid: handle.koid, rights: handle.rights, state: ZX_HANDLE_IN_TRANSIT, owner: endpoint }], handle.koid)
{}

// r[verify zircon-handle.identity]
// r[verify zircon-handle.ownership]
proof fn duplicate_never_adds_rights(handle: ZirconHandle, requested_rights: u64, _new_owner: u64)
    requires handle.wf(), handle.state == ZX_HANDLE_BOUND, crate::zircon_rights::can_duplicate_spec(handle.rights, requested_rights)
    ensures crate::zircon_rights::rights_subset_spec(requested_rights, handle.rights)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::zircon_rights::ZX_RIGHT_DUPLICATE;
    use crate::zircon_rights::ZX_RIGHT_TRANSFER;
    use crate::zircon_rights::ZX_RIGHT_WAIT;

    fn base_handle() -> ZirconHandle {
        bind(7, ZX_RIGHT_DUPLICATE | ZX_RIGHT_TRANSFER | ZX_RIGHT_WAIT, 10)
    }

    // r[verify zircon-handle.identity]
    // r[verify zircon-handle.ownership]
    #[test]
    fn test_duplicate_preserves_identity() {
        let source = base_handle();
        let dup = duplicate(&source, ZX_RIGHT_DUPLICATE | ZX_RIGHT_WAIT, 11).unwrap();
        assert_eq!(dup.koid, source.koid);
        assert_eq!(dup.rights, ZX_RIGHT_DUPLICATE | ZX_RIGHT_WAIT);
        assert!(is_bound_to(&dup, 11));
    }

    // r[verify zircon-handle.identity]
    // r[verify zircon-handle.ownership]
    #[test]
    fn test_transfer_receive_preserves_identity() {
        let source = base_handle();
        let transit = transfer(&source, 99);
        assert!(is_in_transit_to(&transit, 99));
        let rebound = receive(&transit, 12);
        assert_eq!(rebound.koid, source.koid);
        assert_eq!(rebound.rights, source.rights);
        assert!(is_bound_to(&rebound, 12));
    }

    // r[verify zircon-handle.lifetime]
    #[test]
    fn test_in_transit_object_live() {
        let source = base_handle();
        let transit = transfer(&source, 77);
        assert!(object_live(&[transit], source.koid));
    }

    // r[verify zircon-handle.ownership]
    #[test]
    fn test_close_ends_liveness() {
        let source = base_handle();
        let closed = close(&source);
        assert!(!is_live(&closed));
        assert_eq!(closed.state, ZX_HANDLE_CLOSED);
    }

    proptest! {
        // r[verify zircon-handle.identity]
        #[test]
        fn prop_transfer_receive_preserves_koid(koid: u64, rights: u64, owner: u64, endpoint: u64, new_owner: u64) {
            let h = bind(koid, rights, owner);
            let got = receive(&transfer(&h, endpoint), new_owner);
            prop_assert_eq!(got.koid, koid);
            prop_assert_eq!(got.rights, rights);
        }

        // r[verify zircon-handle.lifetime]
        #[test]
        fn prop_live_detects_any_live_handle(koid: u64, other: u64) {
            let hs = vec![
                bind(other, 0, 1),
                transfer(&bind(koid, 0, 2), 9),
                close(&bind(other.wrapping_add(1), 0, 3)),
            ];
            prop_assert!(object_live(&hs, koid));
        }
    }
}
