//! Exact finite ring-buffer admission primitives.
use vstd::prelude::*;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct DsRing3 { pub head: u8, pub len: u8, pub v0: u16, pub v1: u16, pub v2: u16 }
// r[depends domain-primitives.ds-ring-buffer]
pub open spec fn ds_ring_valid_spec(r: DsRing3) -> bool { r.head < 3 && r.len <= 3 }
// r[depends domain-primitives.ds-ring-buffer]
pub open spec fn ds_ring_phys_spec(head: u8, logical: u8) -> int { if (head as int) + (logical as int) < 3 { (head as int) + (logical as int) } else { (head as int) + (logical as int) - 3 } }
// r[depends domain-primitives.ds-ring-buffer]
pub open spec fn ds_ring_next_head_spec(head: u8) -> u8 { if head == 0 { 1 } else if head == 1 { 2 } else { 0 } }
// r[depends domain-primitives.ds-ring-buffer]
pub open spec fn ds_ring_get_spec(r: DsRing3, logical: u8) -> Option<u16> { if !ds_ring_valid_spec(r) || logical >= r.len { None } else if ds_ring_phys_spec(r.head,logical) == 0 { Some(r.v0) } else if ds_ring_phys_spec(r.head,logical) == 1 { Some(r.v1) } else { Some(r.v2) } }
// r[depends domain-primitives.ds-ring-buffer]
pub open spec fn ds_ring_push_spec(r: DsRing3, v: u16) -> DsRing3 { if !ds_ring_valid_spec(r) || r.len >= 3 { r } else if ds_ring_phys_spec(r.head,r.len) == 0 { if r.len == 0 { DsRing3{len:1,v0:v,..r} } else if r.len == 1 { DsRing3{len:2,v0:v,..r} } else { DsRing3{len:3,v0:v,..r} } } else if ds_ring_phys_spec(r.head,r.len) == 1 { if r.len == 0 { DsRing3{len:1,v1:v,..r} } else if r.len == 1 { DsRing3{len:2,v1:v,..r} } else { DsRing3{len:3,v1:v,..r} } } else { if r.len == 0 { DsRing3{len:1,v2:v,..r} } else if r.len == 1 { DsRing3{len:2,v2:v,..r} } else { DsRing3{len:3,v2:v,..r} } } }
// r[depends domain-primitives.ds-ring-buffer]
pub open spec fn ds_ring_pop_spec(r: DsRing3) -> DsRing3 { if !ds_ring_valid_spec(r) || r.len == 0 { r } else { if r.len == 1 { DsRing3{head:ds_ring_next_head_spec(r.head),len:0,..r} } else if r.len == 2 { DsRing3{head:ds_ring_next_head_spec(r.head),len:1,..r} } else { DsRing3{head:ds_ring_next_head_spec(r.head),len:2,..r} } } }
// r[impl domain-primitives.ds-ring-buffer]
pub fn ds_ring_valid(r: DsRing3) -> (result: bool) ensures result == ds_ring_valid_spec(r) { r.head < 3 && r.len <= 3 }
// r[impl domain-primitives.ds-ring-buffer]
pub fn ds_ring_phys(head: u8, logical: u8) -> (result: u8) requires head < 3, logical <= 3 ensures result == ds_ring_phys_spec(head,logical), result < 3 { if head + logical < 3 { head + logical } else { head + logical - 3 } }
// r[impl domain-primitives.ds-ring-buffer]
pub fn ds_ring_get(r: DsRing3, logical: u8) -> (result: Option<u16>) ensures result == ds_ring_get_spec(r,logical) { if !ds_ring_valid(r) || logical >= r.len { None } else { let p=ds_ring_phys(r.head,logical); if p == 0 { Some(r.v0) } else if p == 1 { Some(r.v1) } else { Some(r.v2) } } }
// r[impl domain-primitives.ds-ring-buffer]
pub fn ds_ring_push(r: DsRing3, v: u16) -> (result: DsRing3) ensures result == ds_ring_push_spec(r,v) { if !ds_ring_valid(r) || r.len >= 3 { r } else { let p=ds_ring_phys(r.head,r.len); if p == 0 { if r.len == 0 { DsRing3{len:1,v0:v,..r} } else if r.len == 1 { DsRing3{len:2,v0:v,..r} } else { DsRing3{len:3,v0:v,..r} } } else if p == 1 { if r.len == 0 { DsRing3{len:1,v1:v,..r} } else if r.len == 1 { DsRing3{len:2,v1:v,..r} } else { DsRing3{len:3,v1:v,..r} } } else { if r.len == 0 { DsRing3{len:1,v2:v,..r} } else if r.len == 1 { DsRing3{len:2,v2:v,..r} } else { DsRing3{len:3,v2:v,..r} } } } }
// r[impl domain-primitives.ds-ring-buffer]
pub fn ds_ring_pop(r: DsRing3) -> (result: DsRing3) ensures result == ds_ring_pop_spec(r) { if !ds_ring_valid(r) || r.len == 0 { r } else { let h=ds_ring_phys(r.head,1); if r.len == 1 { DsRing3{head:h,len:0,..r} } else if r.len == 2 { DsRing3{head:h,len:1,..r} } else { DsRing3{head:h,len:2,..r} } } }
// r[verify domain-primitives.ds-ring-buffer]
pub proof fn ds_ring_physical_slot_bounded(head: u8, logical: u8) requires head < 3, logical <= 3 ensures ds_ring_phys_spec(head,logical) < 3 {}
// r[verify domain-primitives.ds-ring-buffer]
pub proof fn ds_ring_invalid_ops_unchanged(r: DsRing3, v: u16) requires !ds_ring_valid_spec(r) || r.len >= 3 ensures ds_ring_push_spec(r,v) == r {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wraps() {
        let r = DsRing3 {
            head: 2,
            len: 1,
            v0: 0,
            v1: 0,
            v2: 7,
        };
        let r = ds_ring_push(r, 8);
        assert_eq!(ds_ring_get(r, 0), Some(7));
        assert_eq!(ds_ring_get(r, 1), Some(8));
    }
}
