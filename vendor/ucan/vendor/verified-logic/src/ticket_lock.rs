//! Verified ticket lock: FIFO-fair spinlock using ticket/serving counters.
//!
//! A ticket lock assigns monotonically increasing tickets to arriving
//! threads. The lock holder is the thread whose ticket matches the
//! serving counter. This guarantees FIFO fairness — no starvation.

use vstd::prelude::*;

verus! {

// --- take_ticket ---

// r[depends ticket_lock.take-ticket]
/// Atomically increment the ticket counter, returning the old value
/// as this thread's ticket. The ticket counter wraps at u64::MAX.
pub open spec fn take_ticket_spec(ticket_counter: u64) -> (u64, u64) {
    // Returns (my_ticket, new_counter)
    if ticket_counter == u64::MAX {
        (ticket_counter, 0)
    } else {
        (ticket_counter, (ticket_counter + 1) as u64)
    }
}

// r[impl ticket_lock.take-ticket]
pub fn take_ticket(ticket_counter: u64) -> (result: (u64, u64))
    ensures result == take_ticket_spec(ticket_counter)
{
    if ticket_counter == u64::MAX {
        (ticket_counter, 0)
    } else {
        (ticket_counter, ticket_counter + 1)
    }
}

// --- is_my_turn ---

// r[depends ticket_lock.is-my-turn]
/// A thread may enter the critical section when its ticket matches
/// the current serving counter.
pub open spec fn is_my_turn_spec(my_ticket: u64, serving: u64) -> bool {
    my_ticket == serving
}

// r[impl ticket_lock.is-my-turn]
pub fn is_my_turn(my_ticket: u64, serving: u64) -> (result: bool)
    ensures result == is_my_turn_spec(my_ticket, serving)
{
    my_ticket == serving
}

// --- release ---

// r[depends ticket_lock.release]
/// Release the lock by advancing the serving counter. The next thread
/// in FIFO order (serving + 1) becomes the lock holder.
pub open spec fn release_spec(serving: u64) -> u64 {
    if serving == u64::MAX { 0 } else { (serving + 1) as u64 }
}

// r[impl ticket_lock.release]
pub fn release(serving: u64) -> (result: u64)
    ensures result == release_spec(serving)
{
    if serving == u64::MAX { 0 } else { serving + 1 }
}

// --- waiters_count ---

// r[depends ticket_lock.waiters-count]
/// Number of threads waiting (holding a ticket but not yet serving).
/// When ticket >= serving, it's ticket - serving. Wrapping is handled
/// by treating the counters as a circular space.
pub open spec fn waiters_count_spec(ticket_counter: u64, serving: u64) -> u64 {
    if ticket_counter >= serving {
        (ticket_counter - serving) as u64
    } else {
        // Wrapped: distance through u64::MAX
        ((u64::MAX as int - serving as int + ticket_counter as int + 1) as u64)
    }
}

// r[impl ticket_lock.waiters-count]
pub fn waiters_count(ticket_counter: u64, serving: u64) -> (result: u64)
    ensures result == waiters_count_spec(ticket_counter, serving)
{
    if ticket_counter >= serving {
        ticket_counter - serving
    } else {
        // u64::MAX - serving + ticket_counter + 1
        // = (u64::MAX - serving) + (ticket_counter + 1)
        let gap = u64::MAX - serving;
        // ticket_counter < serving, so ticket_counter + 1 <= serving <= u64::MAX
        // gap = u64::MAX - serving >= 0
        // gap + ticket_counter + 1 = u64::MAX - serving + ticket_counter + 1
        // since ticket_counter < serving: result <= u64::MAX
        gap + ticket_counter + 1
    }
}

// --- is_locked ---

// r[depends ticket_lock.is-locked]
/// The lock is held when the ticket counter has advanced past the
/// serving counter (at least one thread holds or waits for the lock).
pub open spec fn is_locked_spec(ticket_counter: u64, serving: u64) -> bool {
    ticket_counter != serving
}

// r[impl ticket_lock.is-locked]
pub fn is_locked(ticket_counter: u64, serving: u64) -> (result: bool)
    ensures result == is_locked_spec(ticket_counter, serving)
{
    ticket_counter != serving
}

// --- position_in_queue ---

// r[depends ticket_lock.position]
/// A thread's position in the wait queue: 0 means it holds the lock,
/// >0 means it's waiting behind that many threads.
pub open spec fn position_in_queue_spec(my_ticket: u64, serving: u64) -> u64 {
    if my_ticket >= serving {
        (my_ticket - serving) as u64
    } else {
        ((u64::MAX as int - serving as int + my_ticket as int + 1) as u64)
    }
}

// r[impl ticket_lock.position]
pub fn position_in_queue(my_ticket: u64, serving: u64) -> (result: u64)
    ensures result == position_in_queue_spec(my_ticket, serving)
{
    if my_ticket >= serving {
        my_ticket - serving
    } else {
        let gap = u64::MAX - serving;
        gap + my_ticket + 1
    }
}

// === Proofs ===

// r[verify ticket_lock.take-ticket]
/// Taking a ticket advances the counter.
proof fn take_ticket_advances(ticket_counter: u64)
    ensures ({
        let (_, new_counter) = take_ticket_spec(ticket_counter);
        ticket_counter == u64::MAX || new_counter == (ticket_counter + 1) as u64
    })
{}

// r[verify ticket_lock.take-ticket]
/// The returned ticket is the old counter value.
proof fn take_ticket_returns_old(ticket_counter: u64)
    ensures ({
        let (my_ticket, _) = take_ticket_spec(ticket_counter);
        my_ticket == ticket_counter
    })
{}

// r[verify ticket_lock.is-my-turn]
/// A freshly-taken ticket on an unlocked lock gets immediate entry.
proof fn fresh_ticket_on_unlocked(serving: u64)
    ensures ({
        let (my_ticket, _) = take_ticket_spec(serving);
        is_my_turn_spec(my_ticket, serving)
    })
{}

// r[verify ticket_lock.release]
/// Release advances past the current holder's ticket.
proof fn release_advances(serving: u64)
    ensures !is_my_turn_spec(serving, release_spec(serving))
{}

// r[verify ticket_lock.release]
/// Release gives the next ticket holder their turn.
proof fn release_gives_next_turn(serving: u64)
    ensures is_my_turn_spec(release_spec(serving), release_spec(serving))
{}

// r[verify ticket_lock.waiters-count]
/// An unlocked lock has zero waiters.
proof fn unlocked_zero_waiters(counter: u64)
    ensures waiters_count_spec(counter, counter) == 0
{}

// r[verify ticket_lock.waiters-count]
/// After taking one ticket on an unlocked lock, one waiter.
proof fn one_ticket_one_waiter(serving: u64)
    requires serving < u64::MAX
    ensures ({
        let (_, new_counter) = take_ticket_spec(serving);
        waiters_count_spec(new_counter, serving) == 1
    })
{}

// r[verify ticket_lock.release]
// r[verify ticket_lock.waiters-count]
/// Release decrements the waiter count.
proof fn release_decrements_waiters(ticket_counter: u64, serving: u64)
    requires
        ticket_counter != serving,
        ticket_counter >= serving, // no wrap case
    ensures waiters_count_spec(ticket_counter, release_spec(serving))
        == waiters_count_spec(ticket_counter, serving) - 1
{}

// r[verify ticket_lock.is-locked]
/// Taking a ticket on an unlocked lock locks it.
proof fn take_ticket_locks(serving: u64)
    requires serving < u64::MAX
    ensures ({
        let (_, new_counter) = take_ticket_spec(serving);
        is_locked_spec(new_counter, serving)
    })
{}

// r[verify ticket_lock.position]
/// Position 0 means it's your turn.
proof fn position_zero_is_turn(my_ticket: u64, serving: u64)
    ensures (position_in_queue_spec(my_ticket, serving) == 0) == is_my_turn_spec(my_ticket, serving)
{}

// r[verify ticket_lock.position]
/// Position equals waiters count when ticket == counter.
proof fn position_matches_waiters(ticket_counter: u64, serving: u64)
    ensures position_in_queue_spec(ticket_counter, serving) == waiters_count_spec(ticket_counter, serving)
{}

// r[verify ticket_lock.take-ticket]
// r[verify ticket_lock.is-my-turn]
/// Two different tickets can't both be serving (mutual exclusion).
proof fn mutual_exclusion(ticket_a: u64, ticket_b: u64, serving: u64)
    requires
        ticket_a != ticket_b,
        is_my_turn_spec(ticket_a, serving),
    ensures !is_my_turn_spec(ticket_b, serving)
{}

// r[verify ticket_lock.take-ticket]
/// Consecutive take_ticket calls get distinct tickets.
proof fn consecutive_tickets_distinct(counter: u64)
    requires counter < u64::MAX
    ensures ({
        let (t1, c1) = take_ticket_spec(counter);
        let (t2, _) = take_ticket_spec(c1);
        t1 != t2
    })
{}

// r[verify ticket_lock.position]
/// FIFO ordering: earlier ticket has smaller position.
proof fn fifo_ordering(ticket_a: u64, ticket_b: u64, serving: u64)
    requires
        serving <= ticket_a,
        ticket_a < ticket_b,
    ensures position_in_queue_spec(ticket_a, serving) < position_in_queue_spec(ticket_b, serving)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn test_take_ticket_basic() {
        assert_eq!(take_ticket(0), (0, 1));
        assert_eq!(take_ticket(5), (5, 6));
        assert_eq!(take_ticket(u64::MAX), (u64::MAX, 0));
    }

    #[test]
    fn test_is_my_turn() {
        assert!(is_my_turn(0, 0));
        assert!(is_my_turn(5, 5));
        assert!(!is_my_turn(0, 1));
        assert!(!is_my_turn(3, 5));
    }

    #[test]
    fn test_release() {
        assert_eq!(release(0), 1);
        assert_eq!(release(5), 6);
        assert_eq!(release(u64::MAX), 0);
    }

    #[test]
    fn test_waiters_count() {
        assert_eq!(waiters_count(0, 0), 0);
        assert_eq!(waiters_count(5, 0), 5);
        assert_eq!(waiters_count(5, 5), 0);
        assert_eq!(waiters_count(5, 3), 2);
        // Wrap case: counter=2, serving=u64::MAX-1 means 4 waiters
        assert_eq!(waiters_count(2, u64::MAX - 1), 4);
    }

    #[test]
    fn test_is_locked() {
        assert!(!is_locked(0, 0));
        assert!(is_locked(1, 0));
        assert!(is_locked(0, 1));
    }

    #[test]
    fn test_position_in_queue() {
        assert_eq!(position_in_queue(5, 5), 0);
        assert_eq!(position_in_queue(7, 5), 2);
        assert_eq!(position_in_queue(1, u64::MAX), 2);
    }

    #[test]
    fn test_scenario_fifo() {
        // Three threads acquire in order
        let (t1, c1) = take_ticket(0);
        let (t2, c2) = take_ticket(c1);
        let (t3, _c3) = take_ticket(c2);

        assert!(is_my_turn(t1, 0)); // t1 goes first
        assert!(!is_my_turn(t2, 0));
        assert!(!is_my_turn(t3, 0));

        let s1 = release(0); // t1 releases
        assert!(is_my_turn(t2, s1)); // t2 goes next
        assert!(!is_my_turn(t3, s1));

        let s2 = release(s1); // t2 releases
        assert!(is_my_turn(t3, s2)); // t3 goes last
    }

    proptest! {
        #[test]
        fn prop_take_returns_old(counter: u64) {
            let (ticket, _) = take_ticket(counter);
            prop_assert_eq!(ticket, counter);
        }

        #[test]
        fn prop_fresh_ticket_unlocked_gets_turn(serving: u64) {
            let (ticket, _) = take_ticket(serving);
            prop_assert!(is_my_turn(ticket, serving));
        }

        #[test]
        fn prop_release_advances(serving: u64) {
            let new_serving = release(serving);
            prop_assert!(!is_my_turn(serving, new_serving));
        }

        #[test]
        fn prop_unlocked_zero_waiters(counter: u64) {
            prop_assert_eq!(waiters_count(counter, counter), 0);
        }

        #[test]
        fn prop_position_zero_iff_turn(ticket: u64, serving: u64) {
            prop_assert_eq!(
                position_in_queue(ticket, serving) == 0,
                is_my_turn(ticket, serving)
            );
        }

        #[test]
        fn prop_mutual_exclusion(a: u64, b: u64, serving: u64) {
            prop_assume!(a != b);
            if is_my_turn(a, serving) {
                prop_assert!(!is_my_turn(b, serving));
            }
        }

        #[test]
        fn prop_consecutive_distinct(counter in 0..u64::MAX) {
            let (t1, c1) = take_ticket(counter);
            let (t2, _) = take_ticket(c1);
            prop_assert_ne!(t1, t2);
        }

        #[test]
        fn prop_fifo_order(serving in 0..u64::MAX / 2, offset_a in 0u64..1000, gap in 1u64..1000) {
            let ticket_a = serving + offset_a;
            let ticket_b = ticket_a + gap;
            if ticket_b < u64::MAX / 2 {
                prop_assert!(
                    position_in_queue(ticket_a, serving)
                        < position_in_queue(ticket_b, serving)
                );
            }
        }
    }
}
