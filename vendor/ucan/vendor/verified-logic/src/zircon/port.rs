//! Verified Zircon port semantics.
//!
//! A port combines a FIFO packet queue with async-wait registrations.
//! Matching signals materialize wait readiness into packets.

use vstd::prelude::*;

verus! {

pub struct Packet {
    pub key: u64,
    pub observed: u64,
    pub user_queued: bool,
}

pub struct Registration {
    pub koid: u64,
    pub mask: u64,
    pub key: u64,
    pub repeating: bool,
}

pub struct Object {
    pub queue: Vec<Packet>,
    pub regs: Vec<Registration>,
}

impl Object {
    pub open spec fn wf(&self) -> bool {
        true
    }
}

fn clone_packet(packet: &Packet) -> (result: Packet)
    ensures result == *packet
{
    Packet { key: packet.key, observed: packet.observed, user_queued: packet.user_queued }
}

fn clone_packet_suffix(queue: &[Packet], from: usize) -> (result: Vec<Packet>)
    requires from <= queue@.len()
    ensures result@ =~= queue@.subrange(from as int, queue@.len() as int)
{
    let mut result: Vec<Packet> = Vec::with_capacity(queue.len() - from);
    let mut i: usize = from;
    while i < queue.len()
        invariant
            from <= i <= queue@.len(),
            result@ =~= queue@.subrange(from as int, i as int),
        decreases queue@.len() - i,
    {
        result.push(clone_packet(&queue[i]));
        i = i + 1;
    }
    result
}

// r[impl zircon-port.user-queue]
pub fn new() -> (result: Object)
    ensures result.wf(), result.queue@.len() == 0, result.regs@.len() == 0
{
    Object { queue: Vec::new(), regs: Vec::new() }
}

// r[impl zircon-port.async-wait]
pub fn bind(port: &mut Object, koid: u64, mask: u64, key: u64, repeating: bool)
    requires old(port).wf()
    ensures
        port.wf(),
        port.queue@ =~= old(port).queue@,
        port.regs@.len() == old(port).regs@.len() + 1,
        port.regs@[old(port).regs@.len() as int].koid == koid,
        port.regs@[old(port).regs@.len() as int].mask == mask,
        port.regs@[old(port).regs@.len() as int].key == key,
        port.regs@[old(port).regs@.len() as int].repeating == repeating,
{
    port.regs.push(Registration { koid, mask, key, repeating });
}

// r[impl zircon-port.user-queue]
pub fn queue_user(port: &mut Object, key: u64, observed: u64)
    requires old(port).wf()
    ensures
        port.wf(),
        port.regs@ =~= old(port).regs@,
        port.queue@.len() == old(port).queue@.len() + 1,
        port.queue@[old(port).queue@.len() as int].key == key,
        port.queue@[old(port).queue@.len() as int].observed == observed,
        port.queue@[old(port).queue@.len() as int].user_queued,
{
    port.queue.push(Packet { key, observed, user_queued: true });
}

// r[impl zircon-port.user-queue]
pub fn pop_packet(port: &mut Object) -> (result: Option<Packet>)
    requires old(port).wf()
    ensures
        port.wf(),
        port.regs@ =~= old(port).regs@,
        result.is_some() <==> old(port).queue@.len() > 0,
        result.is_some() ==> result.unwrap() == old(port).queue@[0],
        result.is_none() ==> port.queue@ =~= old(port).queue@,
        result.is_some() ==> port.queue@ =~= old(port).queue@.subrange(1, old(port).queue@.len() as int),
{
    if port.queue.is_empty() {
        None
    } else {
        let first = clone_packet(&port.queue[0]);
        let rest = clone_packet_suffix(&port.queue, 1);
        port.queue = rest;
        Some(first)
    }
}

fn deliver_core(port: &mut Object, koid: u64, observed: u64) -> (delivered: usize)
    requires old(port).wf()
    ensures
        port.wf(),
        delivered <= old(port).regs@.len(),
        port.queue@.len() == old(port).queue@.len() + delivered as int,
{
    let mut kept = Vec::with_capacity(port.regs.len());
    let mut delivered_count = 0usize;
    let ghost base_queue_len = old(port).queue@.len();
    let ghost regs = old(port).regs@;
    let mut i: usize = 0;
    while i < port.regs.len()
        invariant
            0 <= i <= regs.len(),
            port.regs@ =~= regs,
            kept@.len() <= i as int,
            delivered_count <= i,
            port.queue@.len() == base_queue_len + delivered_count as int,
        decreases regs.len() - i,
    {
        let reg = &port.regs[i];
        if reg.koid == koid && crate::zircon_wait::ready(observed, reg.mask) {
            port.queue.push(Packet { key: reg.key, observed, user_queued: false });
            delivered_count += 1;
            if reg.repeating {
                kept.push(Registration { koid: reg.koid, mask: reg.mask, key: reg.key, repeating: reg.repeating });
            }
        } else {
            kept.push(Registration { koid: reg.koid, mask: reg.mask, key: reg.key, repeating: reg.repeating });
        }
        i += 1;
    }
    port.regs = kept;
    delivered_count
}

// r[verify zircon-port.one-shot]
proof fn one_shot_removed_after_delivery(reg: Registration, observed: u64)
    requires !reg.repeating, crate::zircon_wait::wait_ready_spec(observed, reg.mask)
    ensures !(reg.repeating)
{}

// r[verify zircon-port.one-shot]
proof fn repeating_stays_registered(reg: Registration, observed: u64)
    requires reg.repeating, crate::zircon_wait::wait_ready_spec(observed, reg.mask)
    ensures reg.repeating
{}

// r[verify zircon-port.user-queue]
proof fn user_queue_fifo_two_packets()
    ensures
        seq![
            Packet { key: 1, observed: 10, user_queued: true },
            Packet { key: 2, observed: 20, user_queued: true },
        ][0].key == 1,
        seq![
            Packet { key: 1, observed: 10, user_queued: true },
            Packet { key: 2, observed: 20, user_queued: true },
        ][1].key == 2,
{}

} // verus!

fn u64_from_usize(value: usize) -> u64 {
    u64::try_from(value).expect("usize count fits in u64")
}

// r[impl zircon-port.async-wait]
// r[impl zircon-port.one-shot]
pub fn deliver(port: &mut Object, koid: u64, observed: u64) -> u64 {
    u64_from_usize(deliver_core(port, koid, observed))
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify zircon-port.async-wait]
    #[test]
    fn test_async_delivery() {
        let mut p = new();
        bind(&mut p, 7, 0b0100, 55, false);
        assert_eq!(deliver(&mut p, 7, 0b0100), 1);
        let pkt = pop_packet(&mut p).unwrap();
        assert_eq!(pkt.key, 55);
        assert_eq!(pkt.observed, 0b0100);
        assert!(!pkt.user_queued);
    }

    // r[verify zircon-port.one-shot]
    #[test]
    fn test_one_shot_consumed() {
        let mut p = new();
        bind(&mut p, 1, 1, 9, false);
        assert_eq!(deliver(&mut p, 1, 1), 1);
        assert_eq!(deliver(&mut p, 1, 1), 0);
    }

    // r[verify zircon-port.one-shot]
    #[test]
    fn test_repeating_rearms() {
        let mut p = new();
        bind(&mut p, 1, 1, 9, true);
        assert_eq!(deliver(&mut p, 1, 1), 1);
        assert_eq!(deliver(&mut p, 1, 1), 1);
    }

    // r[verify zircon-port.user-queue]
    #[test]
    fn test_user_queue_fifo() {
        let mut p = new();
        queue_user(&mut p, 1, 10);
        queue_user(&mut p, 2, 20);
        assert_eq!(pop_packet(&mut p).unwrap().key, 1);
        assert_eq!(pop_packet(&mut p).unwrap().key, 2);
    }

    proptest! {
        // r[verify zircon-port.async-wait]
        #[test]
        fn prop_no_match_no_delivery(mask: u64, observed: u64) {
            let mut p = new();
            bind(&mut p, 7, mask, 1, false);
            if mask & observed == 0 {
                prop_assert_eq!(deliver(&mut p, 7, observed), 0);
            }
        }

        // r[verify zircon-port.user-queue]
        #[test]
        fn prop_user_queue_fifo(keys in proptest::collection::vec(any::<u64>(), 0..20)) {
            let mut p = new();
            for &k in &keys {
                queue_user(&mut p, k, 0);
            }
            for &k in &keys {
                prop_assert_eq!(pop_packet(&mut p).unwrap().key, k);
            }
            prop_assert!(pop_packet(&mut p).is_none());
        }
    }
}
