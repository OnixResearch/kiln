//! Blocking bounded channel facade.
//!
//! Validity boundary: send admission checks MPMC/MPSC capacity, closed/drained
//! state, futex validity, backoff phase, and atomic memory-order requirements.
//! A full nonblocking send rejects without changing queue state; a full blocking
//! send enters a valid futex wait/backoff state.

use vstd::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockingBoundedChannelOutcome {
    Enqueued {
        new_tail: u64,
        new_occupancy: u64,
    },
    WouldBlock {
        futex_state: u32,
        backoff_step: u64,
        should_park: bool,
    },
    Rejected,
}

verus! {

// r[impl composite-primitives.blocking-bounded-channel]
pub open spec fn blocking_bounded_channel_wait_valid_spec(
    futex_state: u32,
    backoff_step: u64,
    spin_limit: u64,
    yield_limit: u64,
) -> bool {
    crate::futex_state::is_valid_state_spec(futex_state)
    && spin_limit <= yield_limit
    && (crate::spin_backoff::is_spinning_spec(backoff_step, spin_limit)
        || crate::spin_backoff::is_yielding_spec(backoff_step, spin_limit, yield_limit)
        || crate::spin_backoff::is_completed_spec(backoff_step, yield_limit))
}

// r[impl composite-primitives.blocking-bounded-channel]
pub open spec fn blocking_bounded_channel_enqueue_spec(
    head: u64,
    tail: u64,
    mpsc_occupancy: u64,
    capacity: u64,
    is_closed: bool,
    futex_state: u32,
    backoff_step: u64,
    spin_limit: u64,
    yield_limit: u64,
    load_order: u8,
    store_order: u8,
    required_load_order: u8,
    required_store_order: u8,
) -> bool {
    tail >= head
    && crate::bounded_mpmc::can_enqueue_spec(head, tail, capacity)
    && crate::mpsc_bounds::can_send_spec(mpsc_occupancy, capacity, is_closed)
    && !crate::mpsc_bounds::is_drained_spec(is_closed, mpsc_occupancy)
    && crate::futex_state::is_valid_state_spec(futex_state)
    && blocking_bounded_channel_wait_valid_spec(futex_state, backoff_step, spin_limit, yield_limit)
    && crate::memory_order::valid_for_load_spec(load_order)
    && crate::memory_order::valid_for_store_spec(store_order)
    && crate::memory_order::at_least_as_strong_spec(load_order, required_load_order)
    && crate::memory_order::at_least_as_strong_spec(store_order, required_store_order)
}

// r[impl composite-primitives.blocking-bounded-channel]
pub open spec fn blocking_bounded_channel_blocking_wait_spec(
    head: u64,
    tail: u64,
    capacity: u64,
    is_closed: bool,
    mpsc_occupancy: u64,
    futex_state: u32,
    backoff_step: u64,
    spin_limit: u64,
    yield_limit: u64,
    load_order: u8,
    store_order: u8,
    required_load_order: u8,
    required_store_order: u8,
) -> bool {
    tail >= head
    && !is_closed
    && !crate::bounded_mpmc::can_enqueue_spec(head, tail, capacity)
    && !crate::mpsc_bounds::can_send_spec(mpsc_occupancy, capacity, is_closed)
    && crate::futex_state::is_valid_state_spec(futex_state)
    && blocking_bounded_channel_wait_valid_spec(
        crate::futex_state::try_wait_spec(futex_state),
        crate::spin_backoff::advance_spec(backoff_step),
        spin_limit,
        yield_limit,
    )
    && crate::memory_order::valid_for_load_spec(load_order)
    && crate::memory_order::valid_for_store_spec(store_order)
    && crate::memory_order::at_least_as_strong_spec(load_order, required_load_order)
    && crate::memory_order::at_least_as_strong_spec(store_order, required_store_order)
}

// r[verify composite-primitives.blocking-bounded-channel]
pub proof fn blocking_bounded_channel_refines_sources(
    head: u64,
    tail: u64,
    mpsc_occupancy: u64,
    capacity: u64,
    is_closed: bool,
    futex_state: u32,
    backoff_step: u64,
    spin_limit: u64,
    yield_limit: u64,
    load_order: u8,
    store_order: u8,
    required_load_order: u8,
    required_store_order: u8,
)
    ensures
        blocking_bounded_channel_enqueue_spec(
            head, tail, mpsc_occupancy, capacity, is_closed,
            futex_state, backoff_step, spin_limit, yield_limit,
            load_order, store_order, required_load_order, required_store_order,
        ) ==> crate::bounded_mpmc::can_enqueue_spec(head, tail, capacity),
        blocking_bounded_channel_enqueue_spec(
            head, tail, mpsc_occupancy, capacity, is_closed,
            futex_state, backoff_step, spin_limit, yield_limit,
            load_order, store_order, required_load_order, required_store_order,
        ) ==> crate::mpsc_bounds::can_send_spec(mpsc_occupancy, capacity, is_closed),
        blocking_bounded_channel_enqueue_spec(
            head, tail, mpsc_occupancy, capacity, is_closed,
            futex_state, backoff_step, spin_limit, yield_limit,
            load_order, store_order, required_load_order, required_store_order,
        ) ==> crate::futex_state::is_valid_state_spec(futex_state),
        blocking_bounded_channel_enqueue_spec(
            head, tail, mpsc_occupancy, capacity, is_closed,
            futex_state, backoff_step, spin_limit, yield_limit,
            load_order, store_order, required_load_order, required_store_order,
        ) ==> crate::memory_order::valid_for_load_spec(load_order),
        blocking_bounded_channel_enqueue_spec(
            head, tail, mpsc_occupancy, capacity, is_closed,
            futex_state, backoff_step, spin_limit, yield_limit,
            load_order, store_order, required_load_order, required_store_order,
        ) ==> crate::memory_order::valid_for_store_spec(store_order),
{
}

// r[verify composite-primitives.blocking-bounded-channel]
pub proof fn blocking_bounded_channel_wait_refines_sources(
    head: u64,
    tail: u64,
    capacity: u64,
    is_closed: bool,
    mpsc_occupancy: u64,
    futex_state: u32,
    backoff_step: u64,
    spin_limit: u64,
    yield_limit: u64,
    load_order: u8,
    store_order: u8,
    required_load_order: u8,
    required_store_order: u8,
)
    ensures
        blocking_bounded_channel_blocking_wait_spec(
            head, tail, capacity, is_closed, mpsc_occupancy,
            futex_state, backoff_step, spin_limit, yield_limit,
            load_order, store_order, required_load_order, required_store_order,
        ) ==> !crate::bounded_mpmc::can_enqueue_spec(head, tail, capacity),
        blocking_bounded_channel_blocking_wait_spec(
            head, tail, capacity, is_closed, mpsc_occupancy,
            futex_state, backoff_step, spin_limit, yield_limit,
            load_order, store_order, required_load_order, required_store_order,
        ) ==> blocking_bounded_channel_wait_valid_spec(
            crate::futex_state::try_wait_spec(futex_state),
            crate::spin_backoff::advance_spec(backoff_step),
            spin_limit,
            yield_limit,
        ),
{
}

// r[verify composite-primitives.blocking-bounded-channel]
pub proof fn blocking_bounded_channel_rejects_nonblocking_full(
    head: u64,
    tail: u64,
    mpsc_occupancy: u64,
    capacity: u64,
    is_closed: bool,
    futex_state: u32,
    backoff_step: u64,
    spin_limit: u64,
    yield_limit: u64,
    load_order: u8,
    store_order: u8,
    required_load_order: u8,
    required_store_order: u8,
)
    ensures
        (tail < head || !crate::bounded_mpmc::can_enqueue_spec(head, tail, capacity)
            || !crate::mpsc_bounds::can_send_spec(mpsc_occupancy, capacity, is_closed)) ==>
        !blocking_bounded_channel_enqueue_spec(
            head, tail, mpsc_occupancy, capacity, is_closed,
            futex_state, backoff_step, spin_limit, yield_limit,
            load_order, store_order, required_load_order, required_store_order,
        ),
{
}

} // verus!

// r[impl composite-primitives.blocking-bounded-channel]
#[allow(clippy::too_many_arguments)]
pub fn blocking_bounded_channel_send(
    head: u64,
    tail: u64,
    mpsc_occupancy: u64,
    capacity: u64,
    is_closed: bool,
    blocking: bool,
    futex_state: u32,
    backoff_step: u64,
    spin_limit: u64,
    yield_limit: u64,
    load_order: u8,
    store_order: u8,
    required_load_order: u8,
    required_store_order: u8,
) -> BlockingBoundedChannelOutcome {
    if tail < head || !crate::futex_state::is_valid_state(futex_state) || spin_limit > yield_limit {
        return BlockingBoundedChannelOutcome::Rejected;
    }
    if !crate::memory_order::valid_for_load(load_order)
        || !crate::memory_order::valid_for_store(store_order)
        || !crate::memory_order::at_least_as_strong(load_order, required_load_order)
        || !crate::memory_order::at_least_as_strong(store_order, required_store_order)
    {
        return BlockingBoundedChannelOutcome::Rejected;
    }
    if crate::bounded_mpmc::can_enqueue(head, tail, capacity)
        && crate::mpsc_bounds::can_send(mpsc_occupancy, capacity, is_closed)
        && !crate::mpsc_bounds::is_drained(is_closed, mpsc_occupancy)
    {
        return BlockingBoundedChannelOutcome::Enqueued {
            new_tail: crate::bounded_mpmc::after_enqueue(tail),
            new_occupancy: crate::mpsc_bounds::after_send(mpsc_occupancy),
        };
    }
    if blocking && !is_closed {
        let wait_state = crate::futex_state::try_wait(futex_state);
        let next_backoff = crate::spin_backoff::advance(backoff_step);
        return BlockingBoundedChannelOutcome::WouldBlock {
            futex_state: wait_state,
            backoff_step: next_backoff,
            should_park: crate::futex_state::should_park(wait_state)
                || crate::spin_backoff::is_completed(next_backoff, yield_limit),
        };
    }
    BlockingBoundedChannelOutcome::Rejected
}

#[cfg(test)]
mod tests {
    use super::*;

    const RELAXED: u8 = 0;
    const ACQUIRE: u8 = 1;
    const RELEASE: u8 = 2;
    const WAITING: u32 = 1;

    #[test]
    fn available_slot_sends_without_parking() {
        assert_eq!(
            blocking_bounded_channel_send(
                0, 1, 1, 4, false, true, WAITING, 0, 6, 10, ACQUIRE, RELEASE, RELAXED, RELAXED,
            ),
            BlockingBoundedChannelOutcome::Enqueued {
                new_tail: 2,
                new_occupancy: 2
            },
        );
    }

    #[test]
    fn full_nonblocking_rejects_without_mutation() {
        assert_eq!(
            blocking_bounded_channel_send(
                0, 4, 4, 4, false, false, WAITING, 3, 6, 10, ACQUIRE, RELEASE, RELAXED, RELAXED,
            ),
            BlockingBoundedChannelOutcome::Rejected,
        );
    }

    #[test]
    fn full_blocking_enters_valid_wait_backoff() {
        assert_eq!(
            blocking_bounded_channel_send(0, 4, 4, 4, false, true, 0, 9, 6, 10, ACQUIRE, RELEASE, RELAXED, RELAXED,),
            BlockingBoundedChannelOutcome::WouldBlock {
                futex_state: WAITING,
                backoff_step: 10,
                should_park: true,
            },
        );
    }

    #[test]
    fn invalid_memory_order_rejects() {
        assert_eq!(
            blocking_bounded_channel_send(
                0, 1, 1, 4, false, true, WAITING, 0, 6, 10, RELEASE, RELEASE, ACQUIRE, RELAXED,
            ),
            BlockingBoundedChannelOutcome::Rejected,
        );
    }
}
