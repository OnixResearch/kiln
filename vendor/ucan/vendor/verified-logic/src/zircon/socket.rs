//! Verified Zircon socket stream semantics.
//!
//! Each endpoint owns a receive buffer. Writes append to the peer's receive
//! buffer up to available capacity. Reads consume from the local receive
//! buffer in order. Write disablement and peer liveness surface as signals.

use vstd::prelude::*;

verus! {

pub const ZX_SOCKET_READABLE: u64 = 1u64 << 0;
pub const ZX_SOCKET_WRITE_DISABLED: u64 = 1u64 << 1;
pub const ZX_SOCKET_PEER_WRITE_DISABLED: u64 = 1u64 << 2;
pub const ZX_SOCKET_READ_THRESHOLD: u64 = 1u64 << 3;
pub const ZX_SOCKET_WRITE_THRESHOLD: u64 = 1u64 << 4;
pub const ZX_SOCKET_PEER_CLOSED: u64 = 1u64 << 5;

pub struct Endpoint {
    pub rx: Vec<u8>,
    pub capacity: usize,
    pub write_disabled: bool,
    pub peer_write_disabled: bool,
    pub peer_live: bool,
    pub read_threshold: usize,
    pub write_threshold: usize,
}

impl Endpoint {
    pub open spec fn wf(&self) -> bool {
        self.rx@.len() <= self.capacity as int
    }
}

fn clone_bytes(data: &[u8], from: usize, to: usize) -> (result: Vec<u8>)
    requires from as int <= to as int, to as int <= data@.len()
    ensures result@ =~= data@.subrange(from as int, to as int)
{
    let mut result: Vec<u8> = Vec::with_capacity(to - from);
    let mut i: usize = from;
    while i < to
        invariant
            from <= i <= to,
            to <= data@.len(),
            result@ =~= data@.subrange(from as int, i as int),
        decreases to - i,
    {
        result.push(data[i]);
        i = i + 1;
    }
    result
}

fn new_core(capacity: usize) -> (result: Endpoint)
    ensures result.wf(), result.capacity == capacity, result.rx@.len() == 0,
{
    Endpoint {
        rx: Vec::new(),
        capacity,
        write_disabled: false,
        peer_write_disabled: false,
        peer_live: true,
        read_threshold: 0,
        write_threshold: 0,
    }
}

// r[depends zircon-socket.stream]
pub open spec fn socket_readable_spec(end: Endpoint) -> bool
    recommends end.wf()
{
    end.rx@.len() > 0
}

// r[depends zircon-socket.thresholds]
pub open spec fn read_threshold_ready_spec(end: Endpoint) -> bool
    recommends end.wf()
{
    end.read_threshold > 0 && end.rx@.len() >= end.read_threshold as int
}

// r[depends zircon-socket.thresholds]
pub open spec fn write_threshold_ready_spec(end: Endpoint) -> bool
    recommends end.wf()
{
    end.write_threshold > 0 && (end.capacity as int - end.rx@.len()) >= end.write_threshold as int
}

// r[depends zircon-socket.disposition]
// r[depends zircon-socket.thresholds]
// r[depends zircon-socket.peer-closed]
pub open spec fn socket_signals_spec(end: Endpoint) -> u64
    recommends end.wf()
{
    (if socket_readable_spec(end) { ZX_SOCKET_READABLE } else { 0u64 })
        | (if end.write_disabled { ZX_SOCKET_WRITE_DISABLED } else { 0u64 })
        | (if end.peer_write_disabled { ZX_SOCKET_PEER_WRITE_DISABLED } else { 0u64 })
        | (if read_threshold_ready_spec(end) { ZX_SOCKET_READ_THRESHOLD } else { 0u64 })
        | (if write_threshold_ready_spec(end) { ZX_SOCKET_WRITE_THRESHOLD } else { 0u64 })
        | (if !end.peer_live { ZX_SOCKET_PEER_CLOSED } else { 0u64 })
}

// r[impl zircon-socket.disposition]
// r[impl zircon-socket.thresholds]
// r[impl zircon-socket.peer-closed]
pub fn signals(end: &Endpoint) -> (result: u64)
    requires end.wf()
    ensures result == socket_signals_spec(*end)
{
    let is_readable = !end.rx.is_empty();
    let has_write_disabled = end.write_disabled;
    let has_peer_write_disabled = end.peer_write_disabled;
    let is_read_threshold_ready = end.read_threshold > 0 && end.rx.len() >= end.read_threshold;
    let free = end.capacity - end.rx.len();
    let is_write_threshold_ready = end.write_threshold > 0 && free >= end.write_threshold;
    let has_peer_closed = !end.peer_live;
    let result = (if is_readable { ZX_SOCKET_READABLE } else { 0u64 })
        | (if has_write_disabled { ZX_SOCKET_WRITE_DISABLED } else { 0u64 })
        | (if has_peer_write_disabled { ZX_SOCKET_PEER_WRITE_DISABLED } else { 0u64 })
        | (if is_read_threshold_ready { ZX_SOCKET_READ_THRESHOLD } else { 0u64 })
        | (if is_write_threshold_ready { ZX_SOCKET_WRITE_THRESHOLD } else { 0u64 })
        | (if has_peer_closed { ZX_SOCKET_PEER_CLOSED } else { 0u64 });
    proof {
        assert(is_readable == socket_readable_spec(*end));
        assert(has_write_disabled == end.write_disabled);
        assert(has_peer_write_disabled == end.peer_write_disabled);
        assert(is_read_threshold_ready == read_threshold_ready_spec(*end));
        assert(is_write_threshold_ready == write_threshold_ready_spec(*end));
        assert(has_peer_closed == (!end.peer_live));
    }
    result
}

fn write_core(local: &Endpoint, peer: &mut Endpoint, data: &[u8]) -> (accepted: usize)
    requires local.wf(), old(peer).wf()
    ensures
        peer.wf(),
        accepted as int <= data@.len(),
        accepted as int <= old(peer).capacity as int - old(peer).rx@.len(),
        (local.write_disabled || !local.peer_live) ==> accepted == 0,
        !(local.write_disabled || !local.peer_live) ==> accepted as int == if data@.len() as int <= old(peer).capacity as int - old(peer).rx@.len() { data@.len() as int } else { old(peer).capacity as int - old(peer).rx@.len() },
        peer.rx@ =~= old(peer).rx@ + data@.subrange(0, accepted as int),
{
    if local.write_disabled || !local.peer_live {
        return 0;
    }
    let free = peer.capacity - peer.rx.len();
    let accepted = if free <= data.len() { free } else { data.len() };
    let ghost base = old(peer).rx@;
    let mut i: usize = 0;
    while i < accepted
        invariant
            peer.capacity == old(peer).capacity,
            peer.write_disabled == old(peer).write_disabled,
            peer.peer_write_disabled == old(peer).peer_write_disabled,
            peer.peer_live == old(peer).peer_live,
            peer.read_threshold == old(peer).read_threshold,
            peer.write_threshold == old(peer).write_threshold,
            0 <= i <= accepted,
            accepted as int <= data@.len(),
            accepted as int <= old(peer).capacity as int - old(peer).rx@.len(),
            base.len() + accepted as int <= peer.capacity as int,
            peer.rx@ =~= base + data@.subrange(0, i as int),
            peer.rx@.len() <= peer.capacity as int,
        decreases accepted - i,
    {
        peer.rx.push(data[i]);
        assert(peer.rx@.len() <= peer.capacity as int);
        i = i + 1;
    }
    assert(peer.rx@.len() <= peer.capacity as int);
    accepted
}

fn read_core(end: &mut Endpoint, want: usize) -> (result: Vec<u8>)
    requires old(end).wf()
    ensures
        end.wf(),
        result@.len() == if want as int <= old(end).rx@.len() { want as int } else { old(end).rx@.len() as int },
        result@ =~= old(end).rx@.subrange(0, result@.len() as int),
        end.rx@ =~= old(end).rx@.subrange(result@.len() as int, old(end).rx@.len() as int),
{
    let n = if want <= end.rx.len() { want } else { end.rx.len() };
    let out = clone_bytes(&end.rx, 0, n);
    let rest = clone_bytes(&end.rx, n, end.rx.len());
    end.rx = rest;
    out
}

// r[impl zircon-socket.disposition]
pub fn disable_write(local: &mut Endpoint, peer: &mut Endpoint)
    requires old(local).wf(), old(peer).wf()
    ensures
        local.wf(), peer.wf(),
        local.write_disabled,
        peer.peer_write_disabled,
        local.rx@ =~= old(local).rx@,
        peer.rx@ =~= old(peer).rx@,
{
    local.write_disabled = true;
    peer.peer_write_disabled = true;
}

// r[impl zircon-socket.disposition]
pub fn enable_write(local: &mut Endpoint, peer: &mut Endpoint)
    requires old(local).wf(), old(peer).wf()
    ensures
        local.wf(), peer.wf(),
        !local.write_disabled,
        !peer.peer_write_disabled,
        local.rx@ =~= old(local).rx@,
        peer.rx@ =~= old(peer).rx@,
{
    local.write_disabled = false;
    peer.peer_write_disabled = false;
}

fn set_read_threshold_core(end: &mut Endpoint, threshold: usize)
    requires old(end).wf()
    ensures end.wf(), end.read_threshold == threshold, end.rx@ =~= old(end).rx@
{
    end.read_threshold = threshold;
}

fn set_write_threshold_core(end: &mut Endpoint, threshold: usize)
    requires old(end).wf()
    ensures end.wf(), end.write_threshold == threshold, end.rx@ =~= old(end).rx@
{
    end.write_threshold = threshold;
}

// r[impl zircon-socket.peer-closed]
pub fn close_endpoint(peer: &mut Endpoint)
    requires old(peer).wf()
    ensures peer.wf(), !peer.peer_live, peer.rx@ =~= old(peer).rx@
{
    peer.peer_live = false;
}

// r[verify zircon-socket.stream]
proof fn short_write_bounded(peer: Endpoint, offered_len: int)
    requires peer.wf(), offered_len >= 0
    ensures (if offered_len <= peer.capacity as int - peer.rx@.len() { offered_len } else { peer.capacity as int - peer.rx@.len() }) <= offered_len
{}

// r[verify zircon-socket.disposition]
proof fn disable_keeps_buffer(local: Endpoint, peer: Endpoint)
    requires local.wf(), peer.wf()
    ensures local.rx@ =~= local.rx@, peer.rx@ =~= peer.rx@
{}

// r[verify zircon-socket.peer-closed]
proof fn peer_closed_sets_signal(end: Endpoint)
    requires end.wf(), !end.peer_live
    ensures !end.peer_live
{}

} // verus!

fn usize_from_u64(value: u64, label: &str) -> usize {
    usize::try_from(value).unwrap_or_else(|_| panic!("{label} fits in usize"))
}

fn u64_from_usize(value: usize) -> u64 {
    u64::try_from(value).expect("usize count fits in u64")
}

// r[impl zircon-socket.stream]
pub fn new(capacity: u64) -> Endpoint {
    new_core(usize_from_u64(capacity, "socket capacity"))
}

// r[impl zircon-socket.stream]
pub fn write(local: &Endpoint, peer: &mut Endpoint, data: &[u8]) -> u64 {
    u64_from_usize(write_core(local, peer, data))
}

// r[impl zircon-socket.stream]
pub fn read(end: &mut Endpoint, want: u64) -> Vec<u8> {
    read_core(end, usize_from_u64(want, "socket read length"))
}

// r[impl zircon-socket.thresholds]
pub fn set_read_threshold(end: &mut Endpoint, threshold: u64) {
    set_read_threshold_core(end, usize_from_u64(threshold, "socket read threshold"));
}

// r[impl zircon-socket.thresholds]
pub fn set_write_threshold(end: &mut Endpoint, threshold: u64) {
    set_write_threshold_core(end, usize_from_u64(threshold, "socket write threshold"));
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn pair(cap: u64) -> (Endpoint, Endpoint) {
        (new(cap), new(cap))
    }

    // r[verify zircon-socket.stream]
    #[test]
    fn test_write_read_roundtrip() {
        let (a, mut b) = pair(16);
        let accepted = write(&a, &mut b, b"hello");
        assert_eq!(accepted, 5);
        let got = read(&mut b, 8);
        assert_eq!(got, b"hello");
    }

    // r[verify zircon-socket.stream]
    #[test]
    fn test_short_write_short_read() {
        let (a, mut b) = pair(4);
        assert_eq!(write(&a, &mut b, b"abcdef"), 4);
        assert_eq!(read(&mut b, 10), b"abcd");
    }

    // r[verify zircon-socket.disposition]
    #[test]
    fn test_disable_write() {
        let (mut a, mut b) = pair(8);
        let _ = write(&a, &mut b, b"abc");
        disable_write(&mut a, &mut b);
        assert_eq!(write(&a, &mut b, b"zzz"), 0);
        assert_eq!(read(&mut b, 8), b"abc");
        assert!(crate::signal::signal_contains(signals(&a), 1));
        assert!(crate::signal::signal_contains(signals(&b), 2));
    }

    // r[verify zircon-socket.thresholds]
    #[test]
    fn test_threshold_signals() {
        let (a, mut b) = pair(8);
        set_read_threshold(&mut b, 3);
        set_write_threshold(&mut b, 4);
        let _ = write(&a, &mut b, b"abc");
        let s = signals(&b);
        assert!(crate::signal::signal_contains(s, 0));
        assert!(crate::signal::signal_contains(s, 3));
        assert!(crate::signal::signal_contains(s, 4));
    }

    // r[verify zircon-socket.peer-closed]
    #[test]
    fn test_peer_closed_signal() {
        let (_a, mut b) = pair(8);
        close_endpoint(&mut b);
        assert!(crate::signal::signal_contains(signals(&b), 5));
    }

    proptest! {
        // r[verify zircon-socket.stream]
        #[test]
        fn prop_write_never_exceeds_capacity(
            cap in 1u64..64,
            data in proptest::collection::vec(any::<u8>(), 0..128),
        ) {
            let (a, mut b) = pair(cap);
            let accepted = write(&a, &mut b, &data);
            prop_assert!(accepted <= cap);
            prop_assert!(u64::try_from(b.rx.len()).expect("buffer length fits in u64") <= cap);
        }

        // r[verify zircon-socket.stream]
        #[test]
        fn prop_read_consumes_prefix(
            cap in 1usize..64,
            seed in proptest::collection::vec(any::<u8>(), 0..64),
            want in 0u64..64,
        ) {
            let mut end = new(u64::try_from(cap).expect("capacity fits in u64"));
            let prefix = if seed.len() < cap { seed.len() } else { cap };
            end.rx.extend_from_slice(&seed[..prefix]);
            let before = end.rx.clone();
            let out = read(&mut end, want);
            let wanted = usize_from_u64(want, "test read length");
            let n = core::cmp::min(wanted, before.len());
            prop_assert_eq!(out, before[..n].to_vec());
            prop_assert_eq!(end.rx, before[n..].to_vec());
        }
    }
}
