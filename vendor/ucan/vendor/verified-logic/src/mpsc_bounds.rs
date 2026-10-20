//! Verified MPSC (multi-producer single-consumer) queue bounds.
//!
//! Models the logical state of a bounded MPSC channel: producer count,
//! queue occupancy, capacity checks, and close/drain protocol.
//! Producers send, a single consumer receives. The queue has a fixed
//! capacity — sends block or fail when full.

use vstd::prelude::*;

verus! {

// --- queue_occupancy ---

// r[depends mpsc_bounds.occupancy]
/// Number of items in the queue. Defined as sends - receives.
pub open spec fn queue_occupancy_spec(total_sent: u64, total_received: u64) -> u64 {
    if total_sent >= total_received { (total_sent - total_received) as u64 } else { 0 }
}

// r[impl mpsc_bounds.occupancy]
pub fn queue_occupancy(total_sent: u64, total_received: u64) -> (result: u64)
    requires total_sent >= total_received
    ensures result == queue_occupancy_spec(total_sent, total_received)
{
    total_sent - total_received
}

// --- is_full ---

// r[depends mpsc_bounds.is-full]
/// The queue is full when occupancy reaches capacity.
pub open spec fn is_full_spec(occupancy: u64, capacity: u64) -> bool {
    occupancy >= capacity
}

// r[impl mpsc_bounds.is-full]
pub fn is_full(occupancy: u64, capacity: u64) -> (result: bool)
    ensures result == is_full_spec(occupancy, capacity)
{
    occupancy >= capacity
}

// --- is_empty ---

// r[depends mpsc_bounds.is-empty]
/// The queue is empty when occupancy is zero.
pub open spec fn is_empty_spec(occupancy: u64) -> bool {
    occupancy == 0
}

// r[impl mpsc_bounds.is-empty]
pub fn is_empty(occupancy: u64) -> (result: bool)
    ensures result == is_empty_spec(occupancy)
{
    occupancy == 0
}

// --- can_send ---

// r[depends mpsc_bounds.can-send]
/// A producer can send when the queue is not full and not closed.
pub open spec fn can_send_spec(occupancy: u64, capacity: u64, is_closed: bool) -> bool {
    !is_closed && !is_full_spec(occupancy, capacity)
}

// r[impl mpsc_bounds.can-send]
pub fn can_send(occupancy: u64, capacity: u64, is_closed: bool) -> (result: bool)
    ensures result == can_send_spec(occupancy, capacity, is_closed)
{
    !is_closed && !is_full(occupancy, capacity)
}

// --- can_receive ---

// r[depends mpsc_bounds.can-receive]
/// The consumer can receive when the queue is not empty.
pub open spec fn can_receive_spec(occupancy: u64) -> bool {
    !is_empty_spec(occupancy)
}

// r[impl mpsc_bounds.can-receive]
pub fn can_receive(occupancy: u64) -> (result: bool)
    ensures result == can_receive_spec(occupancy)
{
    !is_empty(occupancy)
}

// --- after_send ---

// r[depends mpsc_bounds.after-send]
/// Occupancy after a successful send.
pub open spec fn after_send_spec(occupancy: u64) -> u64 {
    (occupancy + 1) as u64
}

// r[impl mpsc_bounds.after-send]
pub fn after_send(occupancy: u64) -> (result: u64)
    requires occupancy < u64::MAX
    ensures result == after_send_spec(occupancy)
{
    occupancy + 1
}

// --- after_receive ---

// r[depends mpsc_bounds.after-receive]
/// Occupancy after a successful receive.
pub open spec fn after_receive_spec(occupancy: u64) -> u64 {
    if occupancy > 0 { (occupancy - 1) as u64 } else { 0 }
}

// r[impl mpsc_bounds.after-receive]
pub fn after_receive(occupancy: u64) -> (result: u64)
    requires occupancy > 0
    ensures result == after_receive_spec(occupancy)
{
    occupancy - 1
}

// --- remaining_capacity ---

// r[depends mpsc_bounds.remaining-capacity]
/// How many more items can be sent before the queue is full.
pub open spec fn remaining_capacity_spec(occupancy: u64, capacity: u64) -> u64 {
    if occupancy >= capacity { 0 } else { (capacity - occupancy) as u64 }
}

// r[impl mpsc_bounds.remaining-capacity]
pub fn remaining_capacity(occupancy: u64, capacity: u64) -> (result: u64)
    ensures result == remaining_capacity_spec(occupancy, capacity)
{
    if occupancy >= capacity { 0 } else { capacity - occupancy }
}

// --- is_drained ---

// r[depends mpsc_bounds.is-drained]
/// The queue is fully drained when closed and empty.
pub open spec fn is_drained_spec(is_closed: bool, occupancy: u64) -> bool {
    is_closed && is_empty_spec(occupancy)
}

// r[impl mpsc_bounds.is-drained]
pub fn is_drained(is_closed: bool, occupancy: u64) -> (result: bool)
    ensures result == is_drained_spec(is_closed, occupancy)
{
    is_closed && is_empty(occupancy)
}

// === Proofs ===

// r[verify mpsc_bounds.occupancy]
/// Occupancy is zero when sent == received.
proof fn equal_means_empty(n: u64)
    ensures queue_occupancy_spec(n, n) == 0
{}

// r[verify mpsc_bounds.after-send]
// r[verify mpsc_bounds.after-receive]
/// Send then receive is identity on occupancy.
proof fn send_receive_roundtrip(occupancy: u64)
    requires occupancy < u64::MAX
    ensures after_receive_spec(after_send_spec(occupancy)) == occupancy
{}

// r[verify mpsc_bounds.after-receive]
// r[verify mpsc_bounds.after-send]
/// Receive then send is identity on occupancy (when positive).
proof fn receive_send_roundtrip(occupancy: u64)
    requires occupancy > 0, occupancy < u64::MAX
    ensures after_send_spec(after_receive_spec(occupancy)) == occupancy
{}

// r[verify mpsc_bounds.after-send]
// r[verify mpsc_bounds.is-empty]
/// After send, queue is not empty.
proof fn send_makes_non_empty(occupancy: u64)
    requires occupancy < u64::MAX
    ensures !is_empty_spec(after_send_spec(occupancy))
{}

// r[verify mpsc_bounds.after-receive]
// r[verify mpsc_bounds.is-full]
/// After receive, queue is not full (when capacity > 0).
proof fn receive_makes_not_full(occupancy: u64, capacity: u64)
    requires occupancy > 0, capacity > 0, occupancy <= capacity
    ensures !is_full_spec(after_receive_spec(occupancy), capacity)
{}

// r[verify mpsc_bounds.can-send]
/// Closed queue blocks all sends.
proof fn closed_blocks_send(occupancy: u64, capacity: u64)
    ensures !can_send_spec(occupancy, capacity, true)
{}

// r[verify mpsc_bounds.can-send]
// r[verify mpsc_bounds.is-full]
/// Full queue blocks sends (even if open).
proof fn full_blocks_send(occupancy: u64, capacity: u64)
    requires is_full_spec(occupancy, capacity)
    ensures !can_send_spec(occupancy, capacity, false)
{}

// r[verify mpsc_bounds.remaining-capacity]
/// Remaining capacity + occupancy = capacity (when not overfull).
proof fn remaining_plus_occupancy(occupancy: u64, capacity: u64)
    requires occupancy <= capacity
    ensures (remaining_capacity_spec(occupancy, capacity) + occupancy) as u64 == capacity
{}

// r[verify mpsc_bounds.remaining-capacity]
/// Zero remaining iff full.
proof fn zero_remaining_iff_full(occupancy: u64, capacity: u64)
    requires occupancy <= capacity
    ensures (remaining_capacity_spec(occupancy, capacity) == 0) == is_full_spec(occupancy, capacity)
{}

// r[verify mpsc_bounds.is-drained]
/// Drained implies closed.
proof fn drained_implies_closed(is_closed: bool, occupancy: u64)
    requires is_drained_spec(is_closed, occupancy)
    ensures is_closed
{}

// r[verify mpsc_bounds.is-drained]
/// Drained implies empty.
proof fn drained_implies_empty(is_closed: bool, occupancy: u64)
    requires is_drained_spec(is_closed, occupancy)
    ensures is_empty_spec(occupancy)
{}

// r[verify mpsc_bounds.is-drained]
// r[verify mpsc_bounds.can-send]
/// Drained queue blocks sends.
proof fn drained_blocks_send(is_closed: bool, occupancy: u64, capacity: u64)
    requires is_drained_spec(is_closed, occupancy)
    ensures !can_send_spec(occupancy, capacity, is_closed)
{}

// r[verify mpsc_bounds.is-drained]
// r[verify mpsc_bounds.can-receive]
/// Drained queue has nothing to receive.
proof fn drained_blocks_receive(is_closed: bool, occupancy: u64)
    requires is_drained_spec(is_closed, occupancy)
    ensures !can_receive_spec(occupancy)
{}

// r[verify mpsc_bounds.after-send]
/// Send increases occupancy by 1.
proof fn send_increases(occupancy: u64)
    requires occupancy < u64::MAX
    ensures after_send_spec(occupancy) == occupancy + 1
{}

// r[verify mpsc_bounds.after-receive]
/// Receive decreases occupancy by 1.
proof fn receive_decreases(occupancy: u64)
    requires occupancy > 0
    ensures after_receive_spec(occupancy) == occupancy - 1
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn test_occupancy() {
        assert_eq!(queue_occupancy(10, 7), 3);
        assert_eq!(queue_occupancy(5, 5), 0);
    }

    #[test]
    fn test_full_empty() {
        assert!(is_full(8, 8));
        assert!(!is_full(7, 8));
        assert!(is_empty(0));
        assert!(!is_empty(1));
    }

    #[test]
    fn test_can_send_receive() {
        assert!(can_send(3, 8, false));
        assert!(!can_send(8, 8, false)); // full
        assert!(!can_send(3, 8, true)); // closed
        assert!(can_receive(1));
        assert!(!can_receive(0));
    }

    #[test]
    fn test_after_send_receive() {
        assert_eq!(after_send(3), 4);
        assert_eq!(after_receive(4), 3);
    }

    #[test]
    fn test_remaining() {
        assert_eq!(remaining_capacity(3, 8), 5);
        assert_eq!(remaining_capacity(8, 8), 0);
        assert_eq!(remaining_capacity(0, 8), 8);
    }

    #[test]
    fn test_drained() {
        assert!(is_drained(true, 0));
        assert!(!is_drained(false, 0));
        assert!(!is_drained(true, 1));
    }

    proptest! {
        #[test]
        fn prop_send_receive_roundtrip(occ in 0u64..u64::MAX) {
            prop_assert_eq!(after_receive(after_send(occ)), occ);
        }

        #[test]
        fn prop_receive_send_roundtrip(occ in 1u64..u64::MAX) {
            prop_assert_eq!(after_send(after_receive(occ)), occ);
        }

        #[test]
        fn prop_send_not_empty(occ in 0u64..u64::MAX) {
            prop_assert!(!is_empty(after_send(occ)));
        }

        #[test]
        fn prop_receive_not_full(cap in 1u64..1000, occ in 1u64..1000) {
            if occ <= cap {
                prop_assert!(!is_full(after_receive(occ), cap));
            }
        }

        #[test]
        fn prop_closed_blocks_send(occ: u64, cap: u64) {
            prop_assert!(!can_send(occ, cap, true));
        }

        #[test]
        fn prop_remaining_plus_occ(occ in 0u64..1000, cap in 0u64..1000) {
            if occ <= cap {
                prop_assert_eq!(remaining_capacity(occ, cap) + occ, cap);
            }
        }

        #[test]
        fn prop_drained_blocks_all(occ: u64, cap: u64) {
            if is_drained(true, occ) {
                prop_assert!(!can_send(occ, cap, true));
                prop_assert!(!can_receive(occ));
            }
        }

        #[test]
        fn prop_occupancy_correct(sent in 0u64..10000, delta in 0u64..10000) {
            let received = if delta <= sent { delta } else { sent };
            prop_assert_eq!(queue_occupancy(sent, received), sent - received);
        }
    }
}
