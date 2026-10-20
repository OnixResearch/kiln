//! Exact finite FIFO queue primitives.
use vstd::prelude::*;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct DsFifo3 { pub len: u8, pub v0: u16, pub v1: u16, pub v2: u16 }
// r[depends domain-primitives.ds-fifo-queue]
pub open spec fn ds_fifo_valid_spec(q: DsFifo3) -> bool { q.len <= 3 }
// r[depends domain-primitives.ds-fifo-queue]
pub open spec fn ds_fifo_enqueue_spec(q: DsFifo3, v: u16) -> DsFifo3 { if !ds_fifo_valid_spec(q) || q.len >= 3 { q } else if q.len == 0 { DsFifo3{len:1,v0:v,..q} } else if q.len == 1 { DsFifo3{len:2,v1:v,..q} } else { DsFifo3{len:3,v2:v,..q} } }
// r[depends domain-primitives.ds-fifo-queue]
pub open spec fn ds_fifo_dequeue_value_spec(q: DsFifo3) -> Option<u16> { if ds_fifo_valid_spec(q) && q.len > 0 { Some(q.v0) } else { None } }
// r[depends domain-primitives.ds-fifo-queue]
pub open spec fn ds_fifo_dequeue_state_spec(q: DsFifo3) -> DsFifo3 { if !ds_fifo_valid_spec(q) || q.len == 0 { q } else if q.len == 1 { DsFifo3{len:0,..q} } else if q.len == 2 { DsFifo3{len:1,v0:q.v1,..q} } else { DsFifo3{len:2,v0:q.v1,v1:q.v2,..q} } }
// r[impl domain-primitives.ds-fifo-queue]
pub fn ds_fifo_valid(q: DsFifo3) -> (result: bool) ensures result == ds_fifo_valid_spec(q) { q.len <= 3 }
// r[impl domain-primitives.ds-fifo-queue]
pub fn ds_fifo_enqueue(q: DsFifo3, v: u16) -> (result: DsFifo3) ensures result == ds_fifo_enqueue_spec(q,v) { if !ds_fifo_valid(q) || q.len >= 3 { q } else if q.len == 0 { DsFifo3{len:1,v0:v,..q} } else if q.len == 1 { DsFifo3{len:2,v1:v,..q} } else { DsFifo3{len:3,v2:v,..q} } }
// r[impl domain-primitives.ds-fifo-queue]
pub fn ds_fifo_dequeue_value(q: DsFifo3) -> (result: Option<u16>) ensures result == ds_fifo_dequeue_value_spec(q) { if ds_fifo_valid(q) && q.len > 0 { Some(q.v0) } else { None } }
// r[impl domain-primitives.ds-fifo-queue]
pub fn ds_fifo_dequeue_state(q: DsFifo3) -> (result: DsFifo3) ensures result == ds_fifo_dequeue_state_spec(q) { if !ds_fifo_valid(q) || q.len == 0 { q } else if q.len == 1 { DsFifo3{len:0,..q} } else if q.len == 2 { DsFifo3{len:1,v0:q.v1,..q} } else { DsFifo3{len:2,v0:q.v1,v1:q.v2,..q} } }
// r[verify domain-primitives.ds-fifo-queue]
pub proof fn ds_fifo_reject_full_unchanged(q: DsFifo3, v: u16) requires !ds_fifo_valid_spec(q) || q.len >= 3 ensures ds_fifo_enqueue_spec(q,v) == q {}
// r[verify domain-primitives.ds-fifo-queue]
pub proof fn ds_fifo_empty_dequeue_unchanged(q: DsFifo3) requires !ds_fifo_valid_spec(q) || q.len == 0 ensures ds_fifo_dequeue_state_spec(q) == q {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fifo_order() {
        let q = DsFifo3 {
            len: 0,
            v0: 0,
            v1: 0,
            v2: 0,
        };
        let q = ds_fifo_enqueue(ds_fifo_enqueue(q, 7), 8);
        assert_eq!(ds_fifo_dequeue_value(q), Some(7));
        assert_eq!(ds_fifo_dequeue_state(q).v0, 8);
    }
}
