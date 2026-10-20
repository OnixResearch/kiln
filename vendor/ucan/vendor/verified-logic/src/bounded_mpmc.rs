//! Verified bounded MPMC (multi-producer, multi-consumer) queue.
//!
//! Models a fixed-capacity ring buffer queue with head/tail counters.
//! Multiple producers enqueue via CAS on tail; multiple consumers
//! dequeue via CAS on head. Properties: capacity bounds, emptiness,
//! fullness, wrap-around correctness, occupancy monotonicity.

use vstd::prelude::*;

verus! {

// --- occupancy ---

// r[depends bounded_mpmc.occupancy]
/// Number of items in the queue. Tail is the next write position,
/// head is the next read position. Both are monotonically increasing
/// logical counters (not wrapped).
pub open spec fn occupancy_spec(head: u64, tail: u64) -> u64 {
    if tail >= head { (tail - head) as u64 } else { 0 }
}

// r[impl bounded_mpmc.occupancy]
pub fn occupancy(head: u64, tail: u64) -> (result: u64)
    requires tail >= head
    ensures result == occupancy_spec(head, tail)
{
    tail - head
}

// --- is_empty ---

// r[depends bounded_mpmc.is-empty]
/// The queue is empty when head has caught up to tail.
pub open spec fn is_empty_spec(head: u64, tail: u64) -> bool {
    head == tail
}

// r[impl bounded_mpmc.is-empty]
pub fn is_empty(head: u64, tail: u64) -> (result: bool)
    ensures result == is_empty_spec(head, tail)
{
    head == tail
}

// --- is_full ---

// r[depends bounded_mpmc.is-full]
/// The queue is full when occupancy equals capacity.
pub open spec fn is_full_spec(head: u64, tail: u64, capacity: u64) -> bool {
    tail >= head && (tail - head) as u64 >= capacity
}

// r[impl bounded_mpmc.is-full]
pub fn is_full(head: u64, tail: u64, capacity: u64) -> (result: bool)
    requires tail >= head
    ensures result == is_full_spec(head, tail, capacity)
{
    tail - head >= capacity
}

// --- can_enqueue ---

// r[depends bounded_mpmc.can-enqueue]
/// A producer can enqueue when the queue is not full.
pub open spec fn can_enqueue_spec(head: u64, tail: u64, capacity: u64) -> bool {
    tail >= head
    && ((tail - head) as u64) < capacity
    && tail < u64::MAX
}

// r[impl bounded_mpmc.can-enqueue]
pub fn can_enqueue(head: u64, tail: u64, capacity: u64) -> (result: bool)
    requires tail >= head
    ensures result == can_enqueue_spec(head, tail, capacity)
{
    tail - head < capacity && tail < u64::MAX
}

// --- can_dequeue ---

// r[depends bounded_mpmc.can-dequeue]
/// A consumer can dequeue when the queue is not empty.
pub open spec fn can_dequeue_spec(head: u64, tail: u64) -> bool {
    tail > head && head < u64::MAX
}

// r[impl bounded_mpmc.can-dequeue]
pub fn can_dequeue(head: u64, tail: u64) -> (result: bool)
    requires tail >= head
    ensures result == can_dequeue_spec(head, tail)
{
    tail > head && head < u64::MAX
}

// --- buffer_index ---

// r[depends bounded_mpmc.buffer-index]
/// Map a logical counter to a physical buffer slot via modulo.
/// Capacity MUST be a power of two for bitwise masking.
pub open spec fn buffer_index_spec(logical: u64, capacity: u64) -> u64
    recommends capacity > 0
{
    logical % capacity
}

// r[impl bounded_mpmc.buffer-index]
pub fn buffer_index(logical: u64, capacity: u64) -> (result: u64)
    requires capacity > 0
    ensures result == buffer_index_spec(logical, capacity)
{
    logical % capacity
}

// --- after_enqueue ---

// r[depends bounded_mpmc.after-enqueue]
/// Tail after a successful enqueue.
pub open spec fn after_enqueue_spec(tail: u64) -> u64 {
    (tail + 1) as u64
}

// r[impl bounded_mpmc.after-enqueue]
pub fn after_enqueue(tail: u64) -> (result: u64)
    requires tail < u64::MAX
    ensures result == after_enqueue_spec(tail)
{
    tail + 1
}

// --- after_dequeue ---

// r[depends bounded_mpmc.after-dequeue]
/// Head after a successful dequeue.
pub open spec fn after_dequeue_spec(head: u64) -> u64 {
    (head + 1) as u64
}

// r[impl bounded_mpmc.after-dequeue]
pub fn after_dequeue(head: u64) -> (result: u64)
    requires head < u64::MAX
    ensures result == after_dequeue_spec(head)
{
    head + 1
}

// --- slot_generation ---

// r[depends bounded_mpmc.slot-generation]
/// The generation of a slot tells us which "lap" around the ring
/// buffer this write belongs to. Producers write at generation
/// tail/capacity, consumers expect generation head/capacity.
pub open spec fn slot_generation_spec(logical: u64, capacity: u64) -> u64
    recommends capacity > 0
{
    logical / capacity
}

// r[impl bounded_mpmc.slot-generation]
pub fn slot_generation(logical: u64, capacity: u64) -> (result: u64)
    requires capacity > 0
    ensures result == slot_generation_spec(logical, capacity)
{
    logical / capacity
}

// --- remaining_capacity ---

// r[depends bounded_mpmc.remaining-capacity]
/// How many more items can be enqueued.
pub open spec fn remaining_capacity_spec(head: u64, tail: u64, capacity: u64) -> u64 {
    if tail >= head && capacity > (tail - head) as u64 {
        (capacity - (tail - head) as u64) as u64
    } else {
        0
    }
}

// r[impl bounded_mpmc.remaining-capacity]
pub fn remaining_capacity(head: u64, tail: u64, capacity: u64) -> (result: u64)
    requires tail >= head
    ensures result == remaining_capacity_spec(head, tail, capacity)
{
    let used = tail - head;
    if capacity > used { capacity - used } else { 0 }
}

// === Proofs ===

// r[verify bounded_mpmc.occupancy]
/// Empty queue has zero occupancy.
proof fn empty_zero_occupancy(pos: u64)
    ensures occupancy_spec(pos, pos) == 0
{}

// r[verify bounded_mpmc.is-empty]
// r[verify bounded_mpmc.is-full]
/// Empty and full are mutually exclusive (for capacity > 0).
proof fn empty_full_exclusive(head: u64, tail: u64, capacity: u64)
    requires tail >= head, capacity > 0
    ensures !(is_empty_spec(head, tail) && is_full_spec(head, tail, capacity))
{}

// r[verify bounded_mpmc.after-enqueue]
/// Enqueue increases occupancy by one.
proof fn enqueue_increases_occupancy(head: u64, tail: u64)
    requires tail >= head, tail < u64::MAX
    ensures occupancy_spec(head, after_enqueue_spec(tail))
        == occupancy_spec(head, tail) + 1
{}

// r[verify bounded_mpmc.after-dequeue]
/// Dequeue decreases occupancy by one.
proof fn dequeue_decreases_occupancy(head: u64, tail: u64)
    requires tail > head, head < u64::MAX
    ensures occupancy_spec(after_dequeue_spec(head), tail)
        == occupancy_spec(head, tail) - 1
{}

// r[verify bounded_mpmc.after-enqueue]
// r[verify bounded_mpmc.after-dequeue]
/// Enqueue then dequeue is identity on occupancy.
proof fn enqueue_dequeue_identity(head: u64, tail: u64)
    requires tail > head, tail < u64::MAX, head < u64::MAX
    ensures occupancy_spec(after_dequeue_spec(head), after_enqueue_spec(tail))
        == occupancy_spec(head, tail)
{}

// r[verify bounded_mpmc.can-enqueue]
// r[verify bounded_mpmc.is-full]
/// Can't enqueue when full.
proof fn full_blocks_enqueue(head: u64, tail: u64, capacity: u64)
    requires tail >= head, is_full_spec(head, tail, capacity)
    ensures !can_enqueue_spec(head, tail, capacity)
{}

// r[verify bounded_mpmc.can-dequeue]
// r[verify bounded_mpmc.is-empty]
/// Can't dequeue when empty.
proof fn empty_blocks_dequeue(head: u64, tail: u64)
    requires tail >= head, is_empty_spec(head, tail)
    ensures !can_dequeue_spec(head, tail)
{}

// r[verify bounded_mpmc.buffer-index]
/// Buffer index is always within capacity.
proof fn buffer_index_bounded(logical: u64, capacity: u64)
    requires capacity > 0
    ensures buffer_index_spec(logical, capacity) < capacity
{}

// r[verify bounded_mpmc.remaining-capacity]
/// Remaining capacity + occupancy = capacity.
proof fn capacity_conservation(head: u64, tail: u64, capacity: u64)
    requires tail >= head, capacity >= (tail - head) as u64
    ensures remaining_capacity_spec(head, tail, capacity) + occupancy_spec(head, tail)
        == capacity
{}

// r[verify bounded_mpmc.remaining-capacity]
/// Empty queue has full remaining capacity.
proof fn empty_full_remaining(pos: u64, capacity: u64)
    requires capacity > 0
    ensures remaining_capacity_spec(pos, pos, capacity) == capacity
{}

// r[verify bounded_mpmc.remaining-capacity]
/// Full queue has zero remaining capacity.
proof fn full_zero_remaining(head: u64, tail: u64, capacity: u64)
    requires
        tail >= head,
        (tail - head) as u64 >= capacity,
    ensures remaining_capacity_spec(head, tail, capacity) == 0
{}

// r[verify bounded_mpmc.after-enqueue]
// r[verify bounded_mpmc.remaining-capacity]
/// Enqueue decreases remaining capacity by one.
proof fn enqueue_decreases_remaining(head: u64, tail: u64, capacity: u64)
    requires
        tail >= head,
        tail < u64::MAX,
        capacity > (tail - head) as u64,
    ensures remaining_capacity_spec(head, after_enqueue_spec(tail), capacity)
        == remaining_capacity_spec(head, tail, capacity) - 1
{}

// r[verify bounded_mpmc.slot-generation]
/// Consecutive laps have different generations.
proof fn consecutive_laps_different(logical: u64, capacity: u64)
    requires capacity > 0, (logical as int + capacity as int) <= u64::MAX as int
    ensures slot_generation_spec((logical + capacity) as u64, capacity)
        > slot_generation_spec(logical, capacity)
{
    let lc = (logical + capacity) as u64;
    vstd::arithmetic::div_mod::lemma_div_is_ordered(logical as int, lc as int, capacity as int);
    // logical < logical + capacity, so logical / capacity <= (logical + capacity) / capacity
    // Now show strict: (logical + capacity) / capacity >= logical / capacity + 1
    vstd::arithmetic::div_mod::lemma_fundamental_div_mod(logical as int, capacity as int);
    // logical == q * capacity + r where 0 <= r < capacity
    // (logical + capacity) == (q+1) * capacity + r
    // so (logical + capacity) / capacity == q + 1 > q == logical / capacity
    assert(slot_generation_spec(lc, capacity) > slot_generation_spec(logical, capacity))
        by(nonlinear_arith)
        requires
            capacity > 0u64,
            lc == (logical + capacity) as u64,
            (logical as int) + (capacity as int) <= (u64::MAX as int),
            logical as int == (logical / capacity) as int * (capacity as int) + (logical % capacity) as int;
}

// r[verify bounded_mpmc.occupancy]
/// Occupancy is bounded by the distance between head and tail.
proof fn occupancy_bounded(head: u64, tail: u64, capacity: u64)
    requires tail >= head, (tail - head) as u64 <= capacity
    ensures occupancy_spec(head, tail) <= capacity
{}

// r[verify bounded_mpmc.can-enqueue]
/// After dequeue from a full queue, can enqueue again.
proof fn dequeue_enables_enqueue(head: u64, tail: u64, capacity: u64)
    requires
        tail >= head,
        head < u64::MAX,
        (tail - head) as u64 == capacity,
        capacity > 0,
        tail < u64::MAX,
    ensures can_enqueue_spec(after_dequeue_spec(head), tail, capacity)
{}

} // verus!

#[cfg(test)]
#[allow(clippy::default_numeric_fallback)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn test_occupancy() {
        assert_eq!(occupancy(0, 0), 0);
        assert_eq!(occupancy(0, 5), 5);
        assert_eq!(occupancy(3, 10), 7);
    }

    #[test]
    fn test_empty_full() {
        assert!(is_empty(0, 0));
        assert!(!is_empty(0, 1));
        assert!(!is_full(0, 0, 10));
        assert!(is_full(0, 10, 10));
        assert!(is_full(5, 15, 10));
    }

    #[test]
    fn test_can_enqueue_dequeue() {
        assert!(can_enqueue(0, 0, 10));
        assert!(!can_enqueue(0, 10, 10));
        assert!(!can_dequeue(0, 0));
        assert!(can_dequeue(0, 5));
    }

    #[test]
    fn test_buffer_index() {
        assert_eq!(buffer_index(0, 8), 0);
        assert_eq!(buffer_index(5, 8), 5);
        assert_eq!(buffer_index(8, 8), 0);
        assert_eq!(buffer_index(13, 8), 5);
    }

    #[test]
    fn test_after_enqueue_dequeue() {
        assert_eq!(after_enqueue(0), 1);
        assert_eq!(after_dequeue(0), 1);
    }

    #[test]
    fn test_slot_generation() {
        assert_eq!(slot_generation(0, 8), 0);
        assert_eq!(slot_generation(7, 8), 0);
        assert_eq!(slot_generation(8, 8), 1);
        assert_eq!(slot_generation(16, 8), 2);
    }

    #[test]
    fn test_remaining_capacity() {
        assert_eq!(remaining_capacity(0, 0, 10), 10);
        assert_eq!(remaining_capacity(0, 5, 10), 5);
        assert_eq!(remaining_capacity(0, 10, 10), 0);
    }

    #[test]
    fn test_scenario_producer_consumer() {
        let cap = 4u64;
        let (mut head, mut tail) = (0u64, 0u64);

        // Enqueue 3 items
        for _ in 0..3 {
            assert!(can_enqueue(head, tail, cap));
            tail = after_enqueue(tail);
        }
        assert_eq!(occupancy(head, tail), 3);
        assert!(!is_full(head, tail, cap));

        // Enqueue 1 more → full
        tail = after_enqueue(tail);
        assert!(is_full(head, tail, cap));
        assert!(!can_enqueue(head, tail, cap));

        // Dequeue 2 items
        for _ in 0..2 {
            assert!(can_dequeue(head, tail));
            head = after_dequeue(head);
        }
        assert_eq!(occupancy(head, tail), 2);
        assert!(can_enqueue(head, tail, cap));
    }

    proptest! {
        #[test]
        fn prop_empty_zero_occupancy(pos: u64) {
            prop_assert_eq!(occupancy(pos, pos), 0);
        }

        #[test]
        fn prop_empty_full_exclusive(
            head in 0u64..u64::MAX / 2,
            occ in 0u64..1000,
            cap in 1u64..1000,
        ) {
            let tail = head + occ;
            prop_assert!(!(is_empty(head, tail) && is_full(head, tail, cap)));
        }

        #[test]
        fn prop_enqueue_increases(
            head in 0u64..u64::MAX / 2,
            occ in 0u64..1000,
        ) {
            let tail = head + occ;
            if tail < u64::MAX {
                prop_assert_eq!(
                    occupancy(head, after_enqueue(tail)),
                    occupancy(head, tail) + 1
                );
            }
        }

        #[test]
        fn prop_dequeue_decreases(
            head in 0u64..u64::MAX / 2,
            occ in 1u64..1000,
        ) {
            let tail = head + occ;
            if head < u64::MAX {
                prop_assert_eq!(
                    occupancy(after_dequeue(head), tail),
                    occupancy(head, tail) - 1
                );
            }
        }

        #[test]
        fn prop_capacity_conservation(
            head in 0u64..u64::MAX / 2,
            occ in 0u64..1000,
            extra in 0u64..1000,
        ) {
            let tail = head + occ;
            let cap = occ + extra;
            prop_assert_eq!(
                remaining_capacity(head, tail, cap) + occupancy(head, tail),
                cap
            );
        }

        #[test]
        fn prop_buffer_index_bounded(logical: u64, capacity in 1u64..10000) {
            prop_assert!(buffer_index(logical, capacity) < capacity);
        }

        #[test]
        fn prop_full_blocks_enqueue(
            head in 0u64..u64::MAX / 2,
            cap in 1u64..1000,
        ) {
            let tail = head + cap;
            prop_assert!(!can_enqueue(head, tail, cap));
        }

        #[test]
        fn prop_empty_blocks_dequeue(pos: u64) {
            prop_assert!(!can_dequeue(pos, pos));
        }
    }
}
