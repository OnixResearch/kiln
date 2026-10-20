//! Verified Zircon FIFO semantics.
//!
//! FIFOs move whole fixed-size elements only. Reads and writes are bounded
//! by free/available element slots.
#![allow(
    unknown_lints,
    underscore_in_module_filename,
    non_trait_imports,
    path_segment_repetition,
    unbounded_collection_growth,
    usize_in_public_api
)]

use vstd::prelude::*;

use crate::signal::signal_contains;

verus! {

pub const ZX_FIFO_READABLE: u64 = 1u64 << 0;
pub const ZX_FIFO_WRITABLE: u64 = 1u64 << 1;
pub const ZX_FIFO_PEER_CLOSED: u64 = 1u64 << 2;

pub struct ZirconFifoEndpoint {
    pub rx: Vec<Vec<u8>>,
    pub element_size: usize,
    pub capacity: usize,
    pub peer_live: bool,
}

impl ZirconFifoEndpoint {
    pub open spec fn wf(&self) -> bool {
        &&& self.rx@.len() <= self.capacity as int
        &&& forall|i: int| 0 <= i < self.rx@.len() ==> #[trigger] self.rx@[i]@.len() == self.element_size as int
    }
}

fn clone_bytes(data: &[u8], from: usize, to: usize) -> (result: Vec<u8>)
    requires from as int <= to as int, to as int <= data@.len()
    ensures result@ =~= data@.subrange(from as int, to as int)
{
    let mut result: Vec<u8> = Vec::new();
    let mut i: usize = from;
    while i < to
        invariant
            from <= i <= to,
            to <= data.len(),
            result@ =~= data@.subrange(from as int, i as int),
        decreases to - i,
    {
        result.push(data[i]);
        i = i + 1;
    }
    result
}

fn clone_elements(elements: &[Vec<u8>], from: usize, to: usize) -> (result: Vec<Vec<u8>>)
    requires from as int <= to as int, to as int <= elements@.len()
    ensures result@.len() == to as int - from as int,
        forall|i: int| 0 <= i < result@.len() ==> #[trigger] result@[i]@ =~= elements@[from as int + i]@
{
    let mut result: Vec<Vec<u8>> = Vec::new();
    let mut i: usize = from;
    while i < to
        invariant
            from <= i <= to,
            to <= elements@.len(),
            result@.len() == i as int - from as int,
            forall|j: int| 0 <= j < result@.len() ==> #[trigger] result@[j]@ =~= elements@[from as int + j]@,
        decreases to - i,
    {
        result.push(clone_bytes(&elements[i], 0, elements[i].len()));
        i = i + 1;
    }
    result
}

// r[impl zircon-fifo.element-aligned]
pub fn new_endpoint(element_size: usize, capacity: usize) -> (result: ZirconFifoEndpoint)
    ensures result.wf(), result.element_size == element_size, result.capacity == capacity, result.rx@.len() == 0
{
    ZirconFifoEndpoint { rx: Vec::new(), element_size, capacity, peer_live: true }
}

// r[impl zircon-fifo.element-aligned]
pub fn fifo_write(peer: &mut ZirconFifoEndpoint, elements: &[Vec<u8>]) -> (accepted: usize)
    requires old(peer).wf(), forall|i: int| 0 <= i < elements@.len() ==> #[trigger] elements@[i]@.len() == old(peer).element_size as int
    ensures
        peer.wf(),
        accepted as int <= elements@.len(),
        accepted <= old(peer).capacity - old(peer).rx.len(),
        peer.rx@.len() == old(peer).rx@.len() + accepted as int,
{
    let free = peer.capacity - peer.rx.len();
    let accepted = if free <= elements.len() { free } else { elements.len() };
    let ghost old_len = old(peer).rx@.len();
    let mut i: usize = 0;
    while i < accepted
        invariant
            peer.element_size == old(peer).element_size,
            peer.capacity == old(peer).capacity,
            peer.peer_live == old(peer).peer_live,
            0 <= i <= accepted,
            accepted <= elements@.len(),
            accepted <= old(peer).capacity - old(peer).rx.len(),
            old_len + accepted as int <= peer.capacity as int,
            forall|j: int| 0 <= j < elements@.len() ==> #[trigger] elements@[j]@.len() == old(peer).element_size as int,
            peer.rx@.len() == old_len + i as int,
            peer.rx@.len() <= peer.capacity as int,
            forall|j: int| 0 <= j < old_len ==> #[trigger] peer.rx@[j]@.len() == peer.element_size as int,
            forall|j: int| 0 <= j < i as int ==> #[trigger] peer.rx@[old_len + j]@ =~= elements@[j]@,
        decreases accepted - i,
    {
        assert(elements@[i as int]@.len() == old(peer).element_size as int);
        let elt = clone_bytes(&elements[i], 0, elements[i].len());
        assert(elt@ =~= elements@[i as int]@);
        assert(elt@.len() == peer.element_size as int);
        peer.rx.push(elt);
        assert(peer.rx@.len() <= peer.capacity as int);
        i = i + 1;
    }
    assert(peer.rx@.len() <= peer.capacity as int);
    assert forall|j: int| 0 <= j < peer.rx@.len() implies #[trigger] peer.rx@[j]@.len() == peer.element_size as int by {
        if j < old_len {
        } else {
            assert(0 <= j - old_len < accepted as int);
            assert(old_len + (j - old_len) == j);
            assert(peer.rx@[old_len + (j - old_len)]@ =~= elements@[j - old_len]@);
        }
    }
    accepted
}

// r[impl zircon-fifo.element-aligned]
pub fn fifo_read(end: &mut ZirconFifoEndpoint, want: usize) -> (result: Vec<Vec<u8>>)
    requires old(end).wf()
    ensures
        end.wf(),
        result@.len() == if want as int <= old(end).rx@.len() { want as int } else { old(end).rx@.len() as int },
        forall|i: int| 0 <= i < result@.len() ==> #[trigger] result@[i]@ =~= old(end).rx@[i]@,
        end.rx@.len() + result@.len() == old(end).rx@.len(),
{
    let n = if want <= end.rx.len() { want } else { end.rx.len() };
    let out = clone_elements(&end.rx, 0, n);
    let rest = clone_elements(&end.rx, n, end.rx.len());
    end.rx = rest;
    out
}

// r[depends zircon-fifo.signals]
pub open spec fn fifo_signals_spec(end: ZirconFifoEndpoint)
    -> u64
    recommends end.wf()
{
    (if end.rx@.len() > 0 { ZX_FIFO_READABLE } else { 0u64 })
        | (if end.rx@.len() < end.capacity as int { ZX_FIFO_WRITABLE } else { 0u64 })
        | (if !end.peer_live { ZX_FIFO_PEER_CLOSED } else { 0u64 })
}

// r[impl zircon-fifo.signals]
pub fn fifo_signals(end: &ZirconFifoEndpoint) -> (result: u64)
    requires end.wf()
    ensures result == fifo_signals_spec(*end)
{
    let is_readable = !end.rx.is_empty();
    let is_writable = end.rx.len() < end.capacity;
    let has_peer_closed = !end.peer_live;
    let result = (if is_readable { ZX_FIFO_READABLE } else { 0u64 })
        | (if is_writable { ZX_FIFO_WRITABLE } else { 0u64 })
        | (if has_peer_closed { ZX_FIFO_PEER_CLOSED } else { 0u64 });
    proof {
        assert(is_readable == (end.rx@.len() > 0));
        assert(is_writable == (end.rx@.len() < end.capacity as int));
        assert(has_peer_closed == (!end.peer_live));
    }
    result
}

// r[impl zircon-fifo.peer-closed]
pub fn close_endpoint(peer: &mut ZirconFifoEndpoint)
    requires old(peer).wf()
    ensures peer.wf(), !peer.peer_live, peer.rx@ =~= old(peer).rx@
{
    peer.peer_live = false;
}

// r[verify zircon-fifo.element-aligned]
proof fn write_accepts_only_whole_elements(end: ZirconFifoEndpoint, elements: Seq<Seq<u8>>)
    requires end.wf(), forall|i: int| 0 <= i < elements.len() ==> #[trigger] elements[i].len() == end.element_size as int
    ensures forall|i: int| 0 <= i < elements.len() ==> #[trigger] elements[i].len() == end.element_size as int
{}

// r[verify zircon-fifo.peer-closed]
proof fn peer_close_sets_signal(end: ZirconFifoEndpoint)
    requires end.wf(), !end.peer_live
    ensures !end.peer_live
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn elem(bytes: &[u8]) -> Vec<u8> {
        bytes.to_vec()
    }

    // r[verify zircon-fifo.element-aligned]
    #[test]
    fn test_fifo_whole_elements() {
        let mut end = new_endpoint(2, 4);
        let els = vec![elem(b"aa"), elem(b"bb"), elem(b"cc")];
        assert_eq!(fifo_write(&mut end, &els), 3);
        let got = fifo_read(&mut end, 2);
        assert_eq!(got, vec![elem(b"aa"), elem(b"bb")]);
        let rest = fifo_read(&mut end, 2);
        assert_eq!(rest, vec![elem(b"cc")]);
    }

    // r[verify zircon-fifo.element-aligned]
    #[test]
    fn test_fifo_capacity_bound() {
        let mut end = new_endpoint(2, 2);
        let els = vec![elem(b"aa"), elem(b"bb"), elem(b"cc")];
        assert_eq!(fifo_write(&mut end, &els), 2);
    }

    // r[verify zircon-fifo.signals]
    #[test]
    fn test_fifo_signals() {
        let mut end = new_endpoint(2, 2);
        assert!(signal_contains(fifo_signals(&end), 1));
        let _ = fifo_write(&mut end, &[elem(b"aa")]);
        let s = fifo_signals(&end);
        assert!(signal_contains(s, 0));
        assert!(signal_contains(s, 1));
    }

    // r[verify zircon-fifo.peer-closed]
    #[test]
    fn test_peer_closed() {
        let mut end = new_endpoint(2, 2);
        close_endpoint(&mut end);
        assert!(signal_contains(fifo_signals(&end), 2));
    }

    proptest! {
        // r[verify zircon-fifo.element-aligned]
        #[test]
        fn prop_read_write_len_bounded(n in 0usize..16, want in 0usize..16) {
            let mut end = new_endpoint(1, 8);
            let mut els = Vec::new();
            for i in 0..n {
                els.push(vec![u8::try_from(i & 0xff).expect("masked to u8")]);
            }
            let written = fifo_write(&mut end, &els);
            let got = fifo_read(&mut end, want);
            prop_assert!(written <= 8);
            prop_assert!(got.len() <= written);
            for e in got {
                prop_assert_eq!(e.len(), 1);
            }
        }
    }
}
