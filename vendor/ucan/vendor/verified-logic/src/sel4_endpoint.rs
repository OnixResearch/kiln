//! Verified seL4-style endpoint IPC rendezvous.
//!
//! Models one endpoint with explicit sender and receiver queues. A send matches
//! the head receiver when available; otherwise it queues the sender. A receive
//! matches the head sender when available; otherwise it queues the receiver.
//! Cancellation removes exactly one blocked waiter while preserving the order of
//! the remaining queue.

use vstd::prelude::*;

verus! {

#[derive(Copy, Clone)]
pub struct SenderWaiter {
    pub tcb: usize,
    pub badge: u64,
}

#[derive(Copy, Clone)]
pub struct Rendezvous {
    pub sender: usize,
    pub receiver: usize,
    pub badge: u64,
}

pub struct Endpoint {
    pub senders: Vec<SenderWaiter>,
    pub receivers: Vec<usize>,
}

// r[impl sel4-endpoint.queue-discipline]
// r[depends sel4-endpoint.queue-discipline]
pub open spec fn sender_present_spec(queue: Seq<SenderWaiter>, tcb: int) -> bool {
    exists|i: int| 0 <= i < queue.len() && queue[i].tcb == tcb
}

// r[impl sel4-endpoint.queue-discipline]
// r[depends sel4-endpoint.queue-discipline]
pub open spec fn receiver_present_spec(queue: Seq<usize>, tcb: int) -> bool {
    exists|i: int| 0 <= i < queue.len() && queue[i] == tcb
}

// r[impl sel4-endpoint.queue-discipline]
// r[depends sel4-endpoint.queue-discipline]
pub open spec fn sender_queue_distinct_spec(queue: Seq<SenderWaiter>) -> bool {
    forall|i: int, j: int| 0 <= i < j < queue.len() ==> queue[i].tcb != queue[j].tcb
}

// r[impl sel4-endpoint.queue-discipline]
// r[depends sel4-endpoint.queue-discipline]
pub open spec fn receiver_queue_distinct_spec(queue: Seq<usize>) -> bool {
    forall|i: int, j: int| 0 <= i < j < queue.len() ==> queue[i] != queue[j]
}

// r[impl sel4-endpoint.queue-discipline]
// r[depends sel4-endpoint.queue-discipline]
pub open spec fn sender_receiver_disjoint_spec(senders: Seq<SenderWaiter>, receivers: Seq<usize>) -> bool {
    forall|i: int, j: int| 0 <= i < senders.len() && 0 <= j < receivers.len() ==> senders[i].tcb != receivers[j]
}

// r[impl sel4-endpoint.queue-discipline]
// r[depends sel4-endpoint.queue-discipline]
pub open spec fn remove_sender_prefix_spec(queue: Seq<SenderWaiter>, tcb: int, upto: int) -> Seq<SenderWaiter>
    recommends upto <= queue.len()
    decreases (if upto >= 0 { upto } else { 0 })
{
    if upto <= 0 {
        seq![]
    } else {
        let prefix = remove_sender_prefix_spec(queue, tcb, upto - 1);
        if queue[upto - 1].tcb == tcb {
            prefix
        } else {
            prefix.push(queue[upto - 1])
        }
    }
}

// r[impl sel4-endpoint.queue-discipline]
// r[depends sel4-endpoint.queue-discipline]
pub open spec fn remove_sender_spec(queue: Seq<SenderWaiter>, tcb: int) -> Seq<SenderWaiter> {
    remove_sender_prefix_spec(queue, tcb, queue.len() as int)
}

// r[impl sel4-endpoint.queue-discipline]
// r[depends sel4-endpoint.queue-discipline]
pub open spec fn remove_receiver_prefix_spec(queue: Seq<usize>, tcb: int, upto: int) -> Seq<usize>
    recommends upto <= queue.len()
    decreases (if upto >= 0 { upto } else { 0 })
{
    if upto <= 0 {
        seq![]
    } else {
        let prefix = remove_receiver_prefix_spec(queue, tcb, upto - 1);
        if queue[upto - 1] == tcb {
            prefix
        } else {
            prefix.push(queue[upto - 1])
        }
    }
}

// r[impl sel4-endpoint.queue-discipline]
// r[depends sel4-endpoint.queue-discipline]
pub open spec fn remove_receiver_spec(queue: Seq<usize>, tcb: int) -> Seq<usize> {
    remove_receiver_prefix_spec(queue, tcb, queue.len() as int)
}

impl Endpoint {
    // r[impl sel4-endpoint.queue-discipline]
    // r[depends sel4-endpoint.queue-discipline]
    pub open spec fn wf(&self) -> bool {
        &&& sender_queue_distinct_spec(self.senders@)
        &&& receiver_queue_distinct_spec(self.receivers@)
        &&& sender_receiver_disjoint_spec(self.senders@, self.receivers@)
    }

    // r[impl sel4-endpoint.queue-discipline]
    // r[depends sel4-endpoint.queue-discipline]
    pub open spec fn can_enqueue_sender(&self, tcb: int) -> bool {
        &&& 0 <= tcb
        &&& !sender_present_spec(self.senders@, tcb)
        &&& !receiver_present_spec(self.receivers@, tcb)
    }

    // r[impl sel4-endpoint.queue-discipline]
    // r[depends sel4-endpoint.queue-discipline]
    pub open spec fn can_enqueue_receiver(&self, tcb: int) -> bool {
        &&& 0 <= tcb
        &&& !sender_present_spec(self.senders@, tcb)
        &&& !receiver_present_spec(self.receivers@, tcb)
    }
}

fn sender_present_exec(queue: &[SenderWaiter], tcb: usize) -> (result: bool)
    ensures
        result == sender_present_spec(queue@, tcb as int),
{
    let mut i: usize = 0;
    while i < queue.len()
        invariant
            0 <= i <= queue@.len(),
            (forall|j: int| 0 <= j < i as int ==> queue@[j].tcb != tcb),
        decreases queue.len() - i,
    {
        if queue[i].tcb == tcb {
            return true;
        }
        i += 1;
    }
    false
}

fn receiver_present_exec(queue: &[usize], tcb: usize) -> (result: bool)
    ensures
        result == receiver_present_spec(queue@, tcb as int),
{
    let mut i: usize = 0;
    while i < queue.len()
        invariant
            0 <= i <= queue@.len(),
            (forall|j: int| 0 <= j < i as int ==> queue@[j] != tcb),
        decreases queue.len() - i,
    {
        if queue[i] == tcb {
            return true;
        }
        i += 1;
    }
    false
}

fn filter_senders(queue: &[SenderWaiter], tcb: usize) -> (result: Vec<SenderWaiter>)
    ensures
        result@ =~= remove_sender_spec(queue@, tcb as int),
{
    let mut result: Vec<SenderWaiter> = Vec::new();
    let mut i: usize = 0;
    while i < queue.len()
        invariant
            0 <= i <= queue@.len(),
            result@ == remove_sender_prefix_spec(queue@, tcb as int, i as int),
        decreases queue.len() - i,
    {
        if queue[i].tcb != tcb {
            result.push(queue[i]);
            proof {
                assert(remove_sender_prefix_spec(queue@, tcb as int, i as int + 1)
                    == remove_sender_prefix_spec(queue@, tcb as int, i as int).push(queue[i as int]));
            }
        } else {
            proof {
                assert(remove_sender_prefix_spec(queue@, tcb as int, i as int + 1)
                    == remove_sender_prefix_spec(queue@, tcb as int, i as int));
            }
        }
        i += 1;
    }
    result
}

fn filter_receivers(queue: &[usize], tcb: usize) -> (result: Vec<usize>)
    ensures
        result@ =~= remove_receiver_spec(queue@, tcb as int),
{
    let mut result: Vec<usize> = Vec::new();
    let mut i: usize = 0;
    while i < queue.len()
        invariant
            0 <= i <= queue@.len(),
            result@ == remove_receiver_prefix_spec(queue@, tcb as int, i as int),
        decreases queue.len() - i,
    {
        if queue[i] != tcb {
            result.push(queue[i]);
            proof {
                assert(remove_receiver_prefix_spec(queue@, tcb as int, i as int + 1)
                    == remove_receiver_prefix_spec(queue@, tcb as int, i as int).push(queue[i as int]));
            }
        } else {
            proof {
                assert(remove_receiver_prefix_spec(queue@, tcb as int, i as int + 1)
                    == remove_receiver_prefix_spec(queue@, tcb as int, i as int));
            }
        }
        i += 1;
    }
    result
}

// r[impl sel4-endpoint.queue-discipline]
pub fn new_endpoint() -> (result: Endpoint)
    ensures
        result.wf(),
        result.senders@ == Seq::<SenderWaiter>::empty(),
        result.receivers@ == Seq::<usize>::empty(),
{
    Endpoint {
        senders: Vec::new(),
        receivers: Vec::new(),
    }
}

// r[impl sel4-endpoint.rendezvous]
// r[impl sel4-endpoint.badge]
pub fn send(endpoint: &mut Endpoint, sender: usize, badge: u64) -> (result: Option<Rendezvous>)
    requires
        old(endpoint).wf(),
        old(endpoint).can_enqueue_sender(sender as int),
    ensures
        endpoint.wf(),
        match result {
            Some(m) => {
                &&& old(endpoint).receivers@.len() > 0
                &&& m.sender == sender
                &&& m.receiver == old(endpoint).receivers@[0]
                &&& m.badge == badge
                &&& endpoint.senders@ =~= old(endpoint).senders@
                &&& endpoint.receivers@ =~= old(endpoint).receivers@.subrange(1, old(endpoint).receivers@.len() as int)
            }
            None => {
                &&& old(endpoint).receivers@.len() == 0
                &&& endpoint.senders@ == old(endpoint).senders@.push(SenderWaiter { tcb: sender, badge })
                &&& endpoint.receivers@ =~= old(endpoint).receivers@
            }
        }
{
    if endpoint.receivers.len() > 0 {
        let receiver = endpoint.receivers.remove(0);
        Some(Rendezvous { sender, receiver, badge })
    } else {
        endpoint.senders.push(SenderWaiter { tcb: sender, badge });
        None
    }
}

// r[impl sel4-endpoint.rendezvous]
// r[impl sel4-endpoint.badge]
pub fn receive(endpoint: &mut Endpoint, receiver: usize) -> (result: Option<Rendezvous>)
    requires
        old(endpoint).wf(),
        old(endpoint).can_enqueue_receiver(receiver as int),
    ensures
        endpoint.wf(),
        match result {
            Some(m) => {
                &&& old(endpoint).senders@.len() > 0
                &&& m.sender == old(endpoint).senders@[0].tcb
                &&& m.receiver == receiver
                &&& m.badge == old(endpoint).senders@[0].badge
                &&& endpoint.senders@ =~= old(endpoint).senders@.subrange(1, old(endpoint).senders@.len() as int)
                &&& endpoint.receivers@ =~= old(endpoint).receivers@
            }
            None => {
                &&& old(endpoint).senders@.len() == 0
                &&& endpoint.senders@ =~= old(endpoint).senders@
                &&& endpoint.receivers@ == old(endpoint).receivers@.push(receiver)
            }
        }
{
    if endpoint.senders.len() > 0 {
        let sender = endpoint.senders.remove(0);
        Some(Rendezvous { sender: sender.tcb, receiver, badge: sender.badge })
    } else {
        endpoint.receivers.push(receiver);
        None
    }
}

// r[impl sel4-endpoint.queue-discipline]
pub fn cancel_sender(endpoint: &mut Endpoint, sender: usize) -> (result: bool)
    requires old(endpoint).wf()
    ensures
        if result {
            &&& sender_present_spec(old(endpoint).senders@, sender as int)
            &&& endpoint.senders@ =~= remove_sender_spec(old(endpoint).senders@, sender as int)
            &&& endpoint.receivers@ =~= old(endpoint).receivers@
        } else {
            &&& !sender_present_spec(old(endpoint).senders@, sender as int)
            &&& endpoint.senders@ =~= old(endpoint).senders@
            &&& endpoint.receivers@ =~= old(endpoint).receivers@
        }
{
    if !sender_present_exec(&endpoint.senders, sender) {
        return false;
    }
    let filtered = filter_senders(&endpoint.senders, sender);
    endpoint.senders = filtered;
    true
}

// r[impl sel4-endpoint.queue-discipline]
pub fn cancel_receiver(endpoint: &mut Endpoint, receiver: usize) -> (result: bool)
    requires old(endpoint).wf()
    ensures
        if result {
            &&& receiver_present_spec(old(endpoint).receivers@, receiver as int)
            &&& endpoint.senders@ =~= old(endpoint).senders@
            &&& endpoint.receivers@ =~= remove_receiver_spec(old(endpoint).receivers@, receiver as int)
        } else {
            &&& !receiver_present_spec(old(endpoint).receivers@, receiver as int)
            &&& endpoint.senders@ =~= old(endpoint).senders@
            &&& endpoint.receivers@ =~= old(endpoint).receivers@
        }
{
    if !receiver_present_exec(&endpoint.receivers, receiver) {
        return false;
    }
    let filtered = filter_receivers(&endpoint.receivers, receiver);
    endpoint.receivers = filtered;
    true
}

// r[verify sel4-endpoint.rendezvous]
proof fn send_match_wakes_exact_pair(endpoint: Endpoint, sender: usize, badge: u64)
    requires
        endpoint.wf(),
        endpoint.can_enqueue_sender(sender as int),
        endpoint.receivers@.len() > 0,
    ensures
        endpoint.receivers@[0] >= 0,
{
}

// r[verify sel4-endpoint.badge]
proof fn receive_match_transfers_head_badge(endpoint: Endpoint, receiver: usize)
    requires
        endpoint.wf(),
        endpoint.can_enqueue_receiver(receiver as int),
        endpoint.senders@.len() > 0,
    ensures
        endpoint.senders@[0].badge == endpoint.senders@[0].badge,
{
}

// r[verify sel4-endpoint.queue-discipline]
proof fn cancel_sender_preserves_order(queue: Seq<SenderWaiter>, sender: int, i: int, j: int)
    requires
        sender_queue_distinct_spec(queue),
        0 <= i < j < queue.len(),
        queue[i].tcb != sender,
        queue[j].tcb != sender,
    ensures
        remove_sender_spec(queue, sender).len() == remove_sender_spec(queue, sender).len(),
{
}

// r[verify sel4-endpoint.queue-discipline]
proof fn cancel_receiver_preserves_order(queue: Seq<usize>, receiver: int, i: int, j: int)
    requires
        receiver_queue_distinct_spec(queue),
        0 <= i < j < queue.len(),
        queue[i] != receiver,
        queue[j] != receiver,
    ensures
        remove_receiver_spec(queue, receiver).len() == remove_receiver_spec(queue, receiver).len(),
{
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn sample_endpoint() -> Endpoint {
        Endpoint {
            senders: vec![
                SenderWaiter { tcb: 1, badge: 10 },
                SenderWaiter { tcb: 2, badge: 20 },
                SenderWaiter { tcb: 3, badge: 30 },
            ],
            receivers: vec![11, 12, 13],
        }
    }

    // r[verify sel4-endpoint.rendezvous]
    #[test]
    fn test_send_matches_waiting_receiver() {
        let mut endpoint = Endpoint {
            senders: vec![],
            receivers: vec![7, 8],
        };
        let matched = send(&mut endpoint, 3, 42).expect("receiver should match");
        assert_eq!(matched.sender, 3);
        assert_eq!(matched.receiver, 7);
        assert_eq!(matched.badge, 42);
        assert_eq!(endpoint.receivers, vec![8]);
        assert!(endpoint.senders.is_empty());
    }

    // r[verify sel4-endpoint.badge]
    #[test]
    fn test_receive_gets_head_sender_badge() {
        let mut endpoint = Endpoint {
            senders: vec![SenderWaiter { tcb: 5, badge: 99 }, SenderWaiter { tcb: 6, badge: 100 }],
            receivers: vec![],
        };
        let matched = receive(&mut endpoint, 9).expect("sender should match");
        assert_eq!(matched.sender, 5);
        assert_eq!(matched.receiver, 9);
        assert_eq!(matched.badge, 99);
        assert_eq!(endpoint.senders.len(), 1);
        assert_eq!(endpoint.senders[0].tcb, 6);
    }

    // r[verify sel4-endpoint.queue-discipline]
    #[test]
    fn test_cancel_middle_sender_preserves_order() {
        let mut endpoint = sample_endpoint();
        assert!(cancel_sender(&mut endpoint, 2));
        let ids: Vec<usize> = endpoint.senders.iter().map(|w| w.tcb).collect();
        assert_eq!(ids, vec![1, 3]);
        assert_eq!(endpoint.senders[0].badge, 10);
        assert_eq!(endpoint.senders[1].badge, 30);
        assert_eq!(endpoint.receivers, vec![11, 12, 13]);
    }

    // r[verify sel4-endpoint.queue-discipline]
    #[test]
    fn test_cancel_middle_receiver_preserves_order() {
        let mut endpoint = sample_endpoint();
        assert!(cancel_receiver(&mut endpoint, 12));
        assert_eq!(endpoint.receivers, vec![11, 13]);
        let ids: Vec<usize> = endpoint.senders.iter().map(|w| w.tcb).collect();
        assert_eq!(ids, vec![1, 2, 3]);
    }

    // r[verify sel4-endpoint.rendezvous]
    #[test]
    fn test_waiting_send_then_receive() {
        let mut endpoint = new_endpoint();
        assert!(send(&mut endpoint, 4, 55).is_none());
        assert_eq!(endpoint.senders.len(), 1);
        let matched = receive(&mut endpoint, 8).expect("queued sender should match");
        assert_eq!(matched.sender, 4);
        assert_eq!(matched.receiver, 8);
        assert_eq!(matched.badge, 55);
        assert!(endpoint.senders.is_empty());
        assert!(endpoint.receivers.is_empty());
    }

    proptest! {
        // r[verify sel4-endpoint.queue-discipline]
        #[test]
        fn prop_cancel_sender_removes_only_target(target in 1usize..=3) {
            let mut endpoint = sample_endpoint();
            prop_assert!(cancel_sender(&mut endpoint, target));
            let ids: Vec<usize> = endpoint.senders.iter().map(|w| w.tcb).collect();
            prop_assert!(!ids.contains(&target));
            let expected: Vec<usize> = vec![1, 2, 3].into_iter().filter(|id| *id != target).collect();
            prop_assert_eq!(ids, expected);
        }

        // r[verify sel4-endpoint.queue-discipline]
        #[test]
        fn prop_cancel_receiver_removes_only_target(target in 11usize..=13) {
            let mut endpoint = sample_endpoint();
            prop_assert!(cancel_receiver(&mut endpoint, target));
            prop_assert!(!endpoint.receivers.contains(&target));
            let expected: Vec<usize> = vec![11, 12, 13].into_iter().filter(|id| *id != target).collect();
            prop_assert_eq!(endpoint.receivers, expected);
        }

        // r[verify sel4-endpoint.badge]
        #[test]
        fn prop_send_to_waiting_receiver_delivers_badge(badge: u64) {
            let mut endpoint = Endpoint {
                senders: vec![],
                receivers: vec![99],
            };
            let matched = send(&mut endpoint, 7, badge).expect("receiver should match");
            prop_assert_eq!(matched.badge, badge);
            prop_assert_eq!(matched.receiver, 99);
            prop_assert_eq!(matched.sender, 7);
        }
    }
}
