//! Exact finite priority-queue primitives.
use vstd::prelude::*;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct DsPriorityItem { pub priority: u8, pub value: u16 }
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct DsPriorityQueue3 { pub len: u8, pub i0: DsPriorityItem, pub i1: DsPriorityItem, pub i2: DsPriorityItem }
// r[depends domain-primitives.ds-priority-queue]
pub open spec fn ds_pq_valid_spec(q: DsPriorityQueue3) -> bool { q.len <= 3 }
// r[depends domain-primitives.ds-priority-queue]
pub open spec fn ds_pq_min_index_spec(q: DsPriorityQueue3) -> Option<u8> { if !ds_pq_valid_spec(q) || q.len == 0 { None } else if q.len == 1 || (q.i0.priority <= q.i1.priority && (q.len == 2 || q.i0.priority <= q.i2.priority)) { Some(0u8) } else if q.len == 2 || q.i1.priority <= q.i2.priority { Some(1u8) } else { Some(2u8) } }
// r[depends domain-primitives.ds-priority-queue]
pub open spec fn ds_pq_min_item_spec(q: DsPriorityQueue3) -> Option<DsPriorityItem> { if ds_pq_min_index_spec(q) == Some(0u8) { Some(q.i0) } else if ds_pq_min_index_spec(q) == Some(1u8) { Some(q.i1) } else if ds_pq_min_index_spec(q) == Some(2u8) { Some(q.i2) } else { None } }
// r[depends domain-primitives.ds-priority-queue]
pub open spec fn ds_pq_push_spec(q: DsPriorityQueue3, item: DsPriorityItem) -> DsPriorityQueue3 { if !ds_pq_valid_spec(q) || q.len >= 3 { q } else if q.len == 0 { DsPriorityQueue3{len:1,i0:item,..q} } else if q.len == 1 { DsPriorityQueue3{len:2,i1:item,..q} } else { DsPriorityQueue3{len:3,i2:item,..q} } }
// r[depends domain-primitives.ds-priority-queue]
pub open spec fn ds_pq_pop_state_spec(q: DsPriorityQueue3) -> DsPriorityQueue3 { if !ds_pq_valid_spec(q) || q.len == 0 { q } else if ds_pq_min_index_spec(q) == Some(0u8) { if q.len == 1 { DsPriorityQueue3{len:0,..q} } else if q.len == 2 { DsPriorityQueue3{len:1,i0:q.i1,..q} } else { DsPriorityQueue3{len:2,i0:q.i1,i1:q.i2,..q} } } else if ds_pq_min_index_spec(q) == Some(1u8) { if q.len == 2 { DsPriorityQueue3{len:1,..q} } else { DsPriorityQueue3{len:2,i1:q.i2,..q} } } else { DsPriorityQueue3{len:2,..q} } }
// r[impl domain-primitives.ds-priority-queue]
pub fn ds_pq_valid(q: DsPriorityQueue3) -> (result: bool) ensures result == ds_pq_valid_spec(q) { q.len <= 3 }
// r[impl domain-primitives.ds-priority-queue]
pub fn ds_pq_min_index(q: DsPriorityQueue3) -> (result: Option<u8>) ensures result == ds_pq_min_index_spec(q) { if !ds_pq_valid(q) || q.len == 0 { None } else if q.len == 1 || (q.i0.priority <= q.i1.priority && (q.len == 2 || q.i0.priority <= q.i2.priority)) { Some(0u8) } else if q.len == 2 || q.i1.priority <= q.i2.priority { Some(1u8) } else { Some(2u8) } }
// r[impl domain-primitives.ds-priority-queue]
pub fn ds_pq_min_item(q: DsPriorityQueue3) -> (result: Option<DsPriorityItem>) ensures result == ds_pq_min_item_spec(q) { let idx=ds_pq_min_index(q); if idx == Some(0u8) { Some(q.i0) } else if idx == Some(1u8) { Some(q.i1) } else if idx == Some(2u8) { Some(q.i2) } else { None } }
// r[impl domain-primitives.ds-priority-queue]
pub fn ds_pq_push(q: DsPriorityQueue3, item: DsPriorityItem) -> (result: DsPriorityQueue3) ensures result == ds_pq_push_spec(q,item) { if !ds_pq_valid(q) || q.len >= 3 { q } else if q.len == 0 { DsPriorityQueue3{len:1,i0:item,..q} } else if q.len == 1 { DsPriorityQueue3{len:2,i1:item,..q} } else { DsPriorityQueue3{len:3,i2:item,..q} } }
// r[impl domain-primitives.ds-priority-queue]
pub fn ds_pq_pop_state(q: DsPriorityQueue3) -> (result: DsPriorityQueue3) ensures result == ds_pq_pop_state_spec(q) { if !ds_pq_valid(q) || q.len == 0 { q } else { let idx=ds_pq_min_index(q); if idx == Some(0u8) { if q.len == 1 { DsPriorityQueue3{len:0,..q} } else if q.len == 2 { DsPriorityQueue3{len:1,i0:q.i1,..q} } else { DsPriorityQueue3{len:2,i0:q.i1,i1:q.i2,..q} } } else if idx == Some(1u8) { if q.len == 2 { DsPriorityQueue3{len:1,..q} } else { DsPriorityQueue3{len:2,i1:q.i2,..q} } } else { DsPriorityQueue3{len:2,..q} } } }
// r[verify domain-primitives.ds-priority-queue]
pub proof fn ds_pq_empty_pop_unchanged(q: DsPriorityQueue3) requires !ds_pq_valid_spec(q) || q.len == 0 ensures ds_pq_pop_state_spec(q) == q {}
// r[verify domain-primitives.ds-priority-queue]
pub proof fn ds_pq_full_push_unchanged(q: DsPriorityQueue3, item: DsPriorityItem) requires !ds_pq_valid_spec(q) || q.len >= 3 ensures ds_pq_push_spec(q,item) == q {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn min_tie_first() {
        let z = DsPriorityItem { priority: 9, value: 0 };
        let q = DsPriorityQueue3 {
            len: 2,
            i0: DsPriorityItem { priority: 5, value: 1 },
            i1: DsPriorityItem { priority: 3, value: 2 },
            i2: z,
        };
        assert_eq!(ds_pq_min_item(q).unwrap().value, 2);
    }
}
