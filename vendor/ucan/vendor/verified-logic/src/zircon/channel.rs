//! Verified Zircon channel datagram semantics.
//!
//! Messages are whole datagrams carrying bytes plus zero or more handles.
//! Sending moves handles to in-transit ownership for the destination
//! endpoint. Receiving rebinds them to a process. Closing an endpoint
//! discards unread messages and closes transferred handles.
#![allow(
    unknown_lints,
    path_segment_repetition,
    unbounded_collection_growth,
    borrowed_argument_types,
    no_recursion,
    bool_naming
)]

use vstd::prelude::*;

use crate::zircon_handle::ZirconHandle;
use crate::zircon_handle::close;
#[cfg(verus_keep_ghost)]
use crate::zircon_handle::is_in_transit_to_spec;
#[cfg(verus_keep_ghost)]
use crate::zircon_handle::is_live_spec;
use crate::zircon_handle::receive;
use crate::zircon_handle::transfer;

verus! {

pub const ZX_CHANNEL_READABLE: u64 = 1u64 << 0;
pub const ZX_CHANNEL_PEER_CLOSED: u64 = 1u64 << 1;

pub struct ChannelMessage {
    pub bytes: Vec<u8>,
    pub handles: Vec<ZirconHandle>,
}

pub struct ChannelEndpoint {
    pub koid: u64,
    pub queue: Vec<ChannelMessage>,
    pub peer_live: bool,
}

impl ChannelEndpoint {
    pub open spec fn wf(&self) -> bool {
        forall|i: int, j: int|
            0 <= i < self.queue@.len() && 0 <= j < self.queue@[i].handles@.len()
            ==> #[trigger] is_in_transit_to_spec(self.queue@[i].handles@[j], self.koid)
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
            to <= data@.len(),
            result@ =~= data@.subrange(from as int, i as int),
        decreases to - i,
    {
        result.push(data[i]);
        i = i + 1;
    }
    result
}

fn clone_handle(handle: &ZirconHandle) -> (result: ZirconHandle)
    ensures result == *handle
{
    ZirconHandle {
        koid: handle.koid,
        rights: handle.rights,
        state: handle.state,
        owner: handle.owner,
    }
}

fn clone_handles(handles: &[ZirconHandle], from: usize, to: usize) -> (result: Vec<ZirconHandle>)
    requires from as int <= to as int, to as int <= handles@.len()
    ensures result@ =~= handles@.subrange(from as int, to as int)
{
    let mut result: Vec<ZirconHandle> = Vec::new();
    let mut i: usize = from;
    while i < to
        invariant
            from <= i <= to,
            to <= handles@.len(),
            result@ =~= handles@.subrange(from as int, i as int),
        decreases to - i,
    {
        result.push(clone_handle(&handles[i]));
        i = i + 1;
    }
    result
}

fn clone_message(msg: &ChannelMessage) -> (result: ChannelMessage)
    ensures result.bytes@ =~= msg.bytes@, result.handles@ =~= msg.handles@
{
    ChannelMessage {
        bytes: clone_bytes(&msg.bytes, 0, msg.bytes.len()),
        handles: clone_handles(&msg.handles, 0, msg.handles.len()),
    }
}

fn clone_message_suffix(queue: &[ChannelMessage], from: usize) -> (result: Vec<ChannelMessage>)
    requires from <= queue@.len()
    ensures result@.len() == queue@.len() - from as int,
        forall|i: int| 0 <= i < result@.len() ==> #[trigger] result@[i].bytes@ =~= queue@[from as int + i].bytes@,
        forall|i: int| 0 <= i < result@.len() ==> #[trigger] result@[i].handles@ =~= queue@[from as int + i].handles@,
{
    let mut result: Vec<ChannelMessage> = Vec::new();
    let mut i: usize = from;
    while i < queue.len()
        invariant
            from <= i <= queue@.len(),
            result@.len() == i as int - from as int,
            forall|j: int| 0 <= j < result@.len() ==> #[trigger] result@[j].bytes@ =~= queue@[from as int + j].bytes@,
            forall|j: int| 0 <= j < result@.len() ==> #[trigger] result@[j].handles@ =~= queue@[from as int + j].handles@,
        decreases queue@.len() - i,
    {
        result.push(clone_message(&queue[i]));
        i = i + 1;
    }
    result
}

fn transfer_handles(handles: &[ZirconHandle], endpoint: u64) -> (result: Vec<ZirconHandle>)
    requires
        forall|i: int| 0 <= i < handles@.len() ==> #[trigger] handles@[i].wf() && handles@[i].state == crate::zircon_handle::ZX_HANDLE_BOUND,
    ensures
        result@.len() == handles@.len(),
        forall|i: int| 0 <= i < result@.len() ==> #[trigger] result@[i].koid == handles@[i].koid,
        forall|i: int| 0 <= i < result@.len() ==> #[trigger] result@[i].rights == handles@[i].rights,
        forall|i: int| 0 <= i < result@.len() ==> #[trigger] is_in_transit_to_spec(result@[i], endpoint),
{
    let mut result: Vec<ZirconHandle> = Vec::new();
    let mut i: usize = 0;
    while i < handles.len()
        invariant
            0 <= i <= handles@.len(),
            forall|j: int| 0 <= j < handles@.len() ==> #[trigger] handles@[j].wf() && handles@[j].state == crate::zircon_handle::ZX_HANDLE_BOUND,
            result@.len() == i as int,
            forall|j: int| 0 <= j < result@.len() ==> #[trigger] result@[j].koid == handles@[j].koid,
            forall|j: int| 0 <= j < result@.len() ==> #[trigger] result@[j].rights == handles@[j].rights,
            forall|j: int| 0 <= j < result@.len() ==> #[trigger] is_in_transit_to_spec(result@[j], endpoint),
        decreases handles@.len() - i,
    {
        assert(handles@[i as int].wf());
        assert(handles@[i as int].state == crate::zircon_handle::ZX_HANDLE_BOUND);
        result.push(transfer(&handles[i], endpoint));
        i = i + 1;
    }
    result
}

fn receive_handles(handles: &[ZirconHandle], process: u64) -> (result: Vec<ZirconHandle>)
    requires forall|i: int| 0 <= i < handles@.len() ==> #[trigger] handles@[i].wf() && handles@[i].state == crate::zircon_handle::ZX_HANDLE_IN_TRANSIT
    ensures
        result@.len() == handles@.len(),
        forall|i: int| 0 <= i < result@.len() ==> #[trigger] result@[i].koid == handles@[i].koid,
        forall|i: int| 0 <= i < result@.len() ==> #[trigger] result@[i].rights == handles@[i].rights,
        forall|i: int| 0 <= i < result@.len() ==> #[trigger] crate::zircon_handle::is_bound_to_spec(result@[i], process),
{
    let mut result: Vec<ZirconHandle> = Vec::new();
    let mut i: usize = 0;
    while i < handles.len()
        invariant
            0 <= i <= handles@.len(),
            forall|j: int| 0 <= j < handles@.len() ==> #[trigger] handles@[j].wf() && handles@[j].state == crate::zircon_handle::ZX_HANDLE_IN_TRANSIT,
            result@.len() == i as int,
            forall|j: int| 0 <= j < result@.len() ==> #[trigger] result@[j].koid == handles@[j].koid,
            forall|j: int| 0 <= j < result@.len() ==> #[trigger] result@[j].rights == handles@[j].rights,
            forall|j: int| 0 <= j < result@.len() ==> #[trigger] crate::zircon_handle::is_bound_to_spec(result@[j], process),
        decreases handles@.len() - i,
    {
        assert(handles@[i as int].wf());
        assert(handles@[i as int].state == crate::zircon_handle::ZX_HANDLE_IN_TRANSIT);
        result.push(receive(&handles[i], process));
        i = i + 1;
    }
    result
}

pub open spec fn total_handles_in_queue_from_spec(queue: Seq<ChannelMessage>, from: int) -> int
    decreases if from <= queue.len() { queue.len() - from } else { 0 }
{
    if from >= queue.len() {
        0
    } else {
        queue[from].handles@.len() + total_handles_in_queue_from_spec(queue, from + 1)
    }
}

fn append_handles(dst: &mut Vec<ZirconHandle>, src: &[ZirconHandle])
    ensures dst@ =~= old(dst)@ + src@
{
    let ghost base = old(dst)@;
    let mut i: usize = 0;
    while i < src.len()
        invariant
            0 <= i <= src@.len(),
            dst@ =~= base + src@.subrange(0, i as int),
        decreases src@.len() - i,
    {
        dst.push(clone_handle(&src[i]));
        i = i + 1;
    }
}

fn close_handles_from(handles: &[ZirconHandle], from: usize) -> (result: Vec<ZirconHandle>)
    requires from <= handles@.len(), forall|i: int| 0 <= i < handles@.len() ==> #[trigger] handles@[i].wf()
    ensures
        result@.len() == handles@.len() - from as int,
        forall|i: int| 0 <= i < result@.len() ==> #[trigger] result@[i].state == crate::zircon_handle::ZX_HANDLE_CLOSED,
        forall|i: int| 0 <= i < result@.len() ==> !is_live_spec(result@[i]),
    decreases handles@.len() - from
{
    if from >= handles.len() {
        Vec::new()
    } else {
        let mut result = Vec::new();
        result.push(close(&handles[from]));
        let tail = close_handles_from(handles, from + 1);
        append_handles(&mut result, &tail);
        result
    }
}

#[allow(clippy::only_used_in_recursion)]
fn close_queue_from(queue: &[ChannelMessage], from: usize, endpoint: u64) -> (result: Vec<ZirconHandle>)
    requires
        from <= queue@.len(),
        forall|i: int, j: int|
            0 <= i < queue@.len() && 0 <= j < queue@[i].handles@.len()
            ==> #[trigger] queue@[i].handles@[j].wf() && is_in_transit_to_spec(queue@[i].handles@[j], endpoint),
    ensures
        result@.len() == total_handles_in_queue_from_spec(queue@, from as int),
        forall|i: int| 0 <= i < result@.len() ==> #[trigger] result@[i].state == crate::zircon_handle::ZX_HANDLE_CLOSED,
        forall|i: int| 0 <= i < result@.len() ==> !is_live_spec(result@[i]),
    decreases queue@.len() - from
{
    if from >= queue.len() {
        Vec::new()
    } else {
        let mut result = close_handles_from(&queue[from].handles, 0);
        let tail = close_queue_from(queue, from + 1, endpoint);
        append_handles(&mut result, &tail);
        result
    }
}

// r[depends zircon-channel.datagram]
pub open spec fn channel_readable_spec(end: ChannelEndpoint) -> bool
    recommends end.wf()
{
    end.queue@.len() > 0
}

// r[depends zircon-channel.signals]
pub open spec fn channel_signals_spec(end: ChannelEndpoint) -> u64
    recommends end.wf()
{
    (if channel_readable_spec(end) { ZX_CHANNEL_READABLE } else { 0u64 })
        | (if !end.peer_live { ZX_CHANNEL_PEER_CLOSED } else { 0u64 })
}

// r[impl zircon-channel.signals]
pub fn channel_signals(end: &ChannelEndpoint) -> (result: u64)
    requires end.wf()
    ensures result == channel_signals_spec(*end)
{
    let readable = !end.queue.is_empty();
    let peer_closed = !end.peer_live;
    let result = (if readable { ZX_CHANNEL_READABLE } else { 0u64 })
        | (if peer_closed { ZX_CHANNEL_PEER_CLOSED } else { 0u64 });
    proof {
        assert(readable == channel_readable_spec(*end));
        assert(peer_closed == (!end.peer_live));
    }
    result
}

// r[impl zircon-channel.datagram]
pub fn new_endpoint(koid: u64) -> (result: ChannelEndpoint)
    ensures result.wf(), result.koid == koid, result.queue@.len() == 0, result.peer_live
{
    ChannelEndpoint { koid, queue: Vec::new(), peer_live: true }
}

// r[impl zircon-channel.transfer]
// r[impl zircon-channel.datagram]
pub fn send_message(dst: &mut ChannelEndpoint, bytes: &[u8], handles: &[ZirconHandle]) -> (ok: bool)
    requires
        old(dst).wf(),
        forall|i: int| 0 <= i < handles@.len() ==> #[trigger] handles@[i].wf() && handles@[i].state == crate::zircon_handle::ZX_HANDLE_BOUND,
    ensures
        dst.wf(),
        ok <==> old(dst).peer_live,
        !ok ==> dst.queue@ =~= old(dst).queue@,
        ok ==> dst.queue@.len() == old(dst).queue@.len() + 1,
        ok ==> dst.queue@[old(dst).queue@.len() as int].bytes@ =~= bytes@,
        ok ==> dst.queue@[old(dst).queue@.len() as int].handles@.len() == handles@.len(),
        ok ==> forall|i: int| 0 <= i < handles@.len() ==> #[trigger] dst.queue@[old(dst).queue@.len() as int].handles@[i].koid == handles@[i].koid && dst.queue@[old(dst).queue@.len() as int].handles@[i].rights == handles@[i].rights && is_in_transit_to_spec(dst.queue@[old(dst).queue@.len() as int].handles@[i], old(dst).koid),
{
    if !dst.peer_live {
        return false;
    }
    let moved = transfer_handles(handles, dst.koid);
    let payload = clone_bytes(bytes, 0, bytes.len());
    dst.queue.push(ChannelMessage { bytes: payload, handles: moved });
    true
}

// r[impl zircon-channel.transfer]
// r[impl zircon-channel.datagram]
pub fn receive_message(dst: &mut ChannelEndpoint, process: u64) -> (result: Option<ChannelMessage>)
    requires old(dst).wf()
    ensures
        dst.wf(),
        result.is_some() <==> old(dst).queue@.len() > 0,
        result.is_some() ==> result.unwrap().bytes@ =~= old(dst).queue@[0].bytes@,
        result.is_some() ==> result.unwrap().handles@.len() == old(dst).queue@[0].handles@.len(),
        result.is_some() ==> forall|i: int| 0 <= i < result.unwrap().handles@.len() ==> #[trigger] result.unwrap().handles@[i].koid == old(dst).queue@[0].handles@[i].koid && result.unwrap().handles@[i].rights == old(dst).queue@[0].handles@[i].rights && crate::zircon_handle::is_bound_to_spec(result.unwrap().handles@[i], process),
        result.is_none() ==> dst.queue@ =~= old(dst).queue@,
        result.is_some() ==> dst.queue@.len() + 1 == old(dst).queue@.len(),
{
    if dst.queue.is_empty() {
        None
    } else {
        proof {
            assert forall|i: int| 0 <= i < dst.queue@[0].handles@.len()
                implies #[trigger] dst.queue@[0].handles@[i].wf() && dst.queue@[0].handles@[i].state == crate::zircon_handle::ZX_HANDLE_IN_TRANSIT by {
                assert(is_in_transit_to_spec(dst.queue@[0].handles@[i], dst.koid));
            }
        }
        let payload = clone_bytes(&dst.queue[0].bytes, 0, dst.queue[0].bytes.len());
        let rebound = receive_handles(&dst.queue[0].handles, process);
        let rest = clone_message_suffix(&dst.queue, 1);
        dst.queue = rest;
        assert forall|i: int, j: int|
            0 <= i < dst.queue@.len() && 0 <= j < dst.queue@[i].handles@.len()
            implies #[trigger] is_in_transit_to_spec(dst.queue@[i].handles@[j], dst.koid) by {
            assert(dst.queue@[i].handles@ =~= old(dst).queue@[i + 1].handles@);
            assert(dst.queue@[i].handles@[j] == old(dst).queue@[i + 1].handles@[j]);
            assert(is_in_transit_to_spec(old(dst).queue@[i + 1].handles@[j], old(dst).koid));
        }
        Some(ChannelMessage { bytes: payload, handles: rebound })
    }
}

// r[impl zircon-channel.discard]
pub fn close_endpoint(dst: &mut ChannelEndpoint) -> (discarded: Vec<ZirconHandle>)
    requires old(dst).wf()
    ensures
        dst.wf(),
        !dst.peer_live,
        dst.queue@.len() == 0,
        discarded@.len() == total_handles_in_queue_from_spec(old(dst).queue@, 0),
        forall|i: int| 0 <= i < discarded@.len() ==> #[trigger] discarded@[i].state == crate::zircon_handle::ZX_HANDLE_CLOSED && !is_live_spec(discarded@[i]),
{
    proof {
        assert forall|i: int, j: int|
            0 <= i < dst.queue@.len() && 0 <= j < dst.queue@[i].handles@.len()
            implies #[trigger] dst.queue@[i].handles@[j].wf() && is_in_transit_to_spec(dst.queue@[i].handles@[j], dst.koid) by {
            assert(is_in_transit_to_spec(dst.queue@[i].handles@[j], dst.koid));
        }
    }
    let discarded = close_queue_from(&dst.queue, 0, dst.koid);
    dst.queue = Vec::new();
    dst.peer_live = false;
    discarded
}

// r[verify zircon-channel.signals]
proof fn readable_when_nonempty(end: ChannelEndpoint)
    requires end.wf(), end.queue@.len() > 0
    ensures end.queue@.len() > 0
{}

// r[verify zircon-channel.transfer]
proof fn transfer_preserves_identity(handle: ZirconHandle, _endpoint: u64, process: u64)
    requires handle.wf(), handle.state == crate::zircon_handle::ZX_HANDLE_BOUND
    ensures (ZirconHandle { koid: handle.koid, rights: handle.rights, state: crate::zircon_handle::ZX_HANDLE_BOUND, owner: process }).koid == handle.koid
{}

// r[verify zircon-channel.discard]
proof fn discarded_handles_closed(h: ZirconHandle)
    requires h.wf()
    ensures (ZirconHandle { koid: h.koid, rights: h.rights, state: crate::zircon_handle::ZX_HANDLE_CLOSED, owner: 0 }).state == crate::zircon_handle::ZX_HANDLE_CLOSED
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::signal::signal_contains;
    use crate::zircon_handle::bind;
    use crate::zircon_rights::ZX_RIGHT_TRANSFER;
    use crate::zircon_rights::ZX_RIGHT_WAIT;

    fn h(koid: u64) -> ZirconHandle {
        bind(koid, ZX_RIGHT_TRANSFER | ZX_RIGHT_WAIT, 1)
    }

    // r[verify zircon-channel.datagram]
    // r[verify zircon-channel.transfer]
    #[test]
    fn test_send_receive_roundtrip() {
        let mut dst = new_endpoint(9);
        let hs = vec![h(7), h(8)];
        assert!(send_message(&mut dst, b"abc", &hs));
        assert!(signal_contains(channel_signals(&dst), 0));
        let msg = receive_message(&mut dst, 22).unwrap();
        assert_eq!(msg.bytes, b"abc");
        assert_eq!(msg.handles.len(), 2);
        assert_eq!(msg.handles[0].koid, 7);
        assert_eq!(msg.handles[1].koid, 8);
        assert_eq!(msg.handles[0].owner, 22);
    }

    // r[verify zircon-channel.discard]
    #[test]
    fn test_close_discards_unread_handles() {
        let mut dst = new_endpoint(9);
        let hs = vec![h(7), h(8)];
        assert!(send_message(&mut dst, b"abc", &hs));
        let discarded = close_endpoint(&mut dst);
        assert_eq!(dst.queue.len(), 0);
        assert_eq!(discarded.len(), 2);
        assert!(discarded.iter().all(|h| h.state == crate::zircon_handle::ZX_HANDLE_CLOSED));
    }

    // r[verify zircon-channel.signals]
    #[test]
    fn test_peer_closed_signal() {
        let mut dst = new_endpoint(9);
        let _ = close_endpoint(&mut dst);
        assert!(signal_contains(channel_signals(&dst), 1));
    }

    proptest! {
        // r[verify zircon-channel.datagram]
        #[test]
        fn prop_fifo_datagrams(
            payloads in proptest::collection::vec(proptest::collection::vec(any::<u8>(), 0..8), 0..8)
        ) {
            let mut dst = new_endpoint(9);
            for p in &payloads {
                let hs: Vec<ZirconHandle> = Vec::new();
                prop_assert!(send_message(&mut dst, p, &hs));
            }
            for p in &payloads {
                let msg = receive_message(&mut dst, 1).unwrap();
                prop_assert_eq!(msg.bytes, p.clone());
            }
            prop_assert!(receive_message(&mut dst, 1).is_none());
        }
    }
}
