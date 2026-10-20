//! Verified read-copy-update (RCU) primitives.
//!
//! Models the RCU synchronization pattern: readers enter/exit read-side
//! critical sections tracked by a per-thread quiescent state counter.
//! Writers publish new versions and wait for a grace period — all threads
//! must pass through a quiescent state before old data can be freed.

use vstd::prelude::*;

verus! {

// --- is_quiescent ---

// r[depends rcu.is-quiescent]
/// A thread is quiescent (not in a read-side critical section) when its
/// local counter matches the global grace period counter.
pub open spec fn is_quiescent_spec(thread_counter: u64, grace_counter: u64) -> bool {
    thread_counter == grace_counter
}

// r[impl rcu.is-quiescent]
pub fn is_quiescent(thread_counter: u64, grace_counter: u64) -> (result: bool)
    ensures result == is_quiescent_spec(thread_counter, grace_counter)
{
    thread_counter == grace_counter
}

// --- enter_read ---

// r[depends rcu.enter-read]
/// Enter a read-side critical section: thread records the current grace
/// counter value. Returns the snapshot value.
pub open spec fn enter_read_spec(grace_counter: u64) -> u64 {
    grace_counter
}

// r[impl rcu.enter-read]
pub fn enter_read(grace_counter: u64) -> (result: u64)
    ensures result == enter_read_spec(grace_counter)
{
    grace_counter
}

// --- exit_read ---

// r[depends rcu.exit-read]
/// Exit a read-side critical section: thread updates its counter to
/// the current grace counter, signaling quiescence.
pub open spec fn exit_read_spec(grace_counter: u64) -> u64 {
    grace_counter
}

// r[impl rcu.exit-read]
pub fn exit_read(grace_counter: u64) -> (result: u64)
    ensures result == exit_read_spec(grace_counter)
{
    grace_counter
}

// --- advance_grace_period ---

// r[depends rcu.advance-grace]
/// Advance the global grace period counter by one.
pub open spec fn advance_grace_period_spec(grace_counter: u64) -> u64 {
    if grace_counter < u64::MAX { (grace_counter + 1) as u64 } else { u64::MAX }
}

// r[impl rcu.advance-grace]
pub fn advance_grace_period(grace_counter: u64) -> (result: u64)
    requires grace_counter < u64::MAX
    ensures result == advance_grace_period_spec(grace_counter)
{
    grace_counter + 1
}

// --- all_threads_quiescent ---

// r[depends rcu.all-quiescent]
/// All threads have passed through a quiescent state relative to
/// `grace_counter` when every thread's counter >= grace_counter.
pub open spec fn all_threads_quiescent_spec(
    thread_counters: Seq<u64>,
    grace_counter: u64,
) -> bool {
    forall|i: int| #![trigger thread_counters[i]]
        0 <= i < thread_counters.len() as int
        ==> thread_counters[i] >= grace_counter
}

// r[impl rcu.all-quiescent]
pub fn all_threads_quiescent(
    thread_counters: &Vec<u64>,
    grace_counter: u64,
) -> (result: bool)
    ensures result == all_threads_quiescent_spec(thread_counters@, grace_counter)
{
    let mut i: usize = 0;
    while i < thread_counters.len()
        invariant
            0 <= i <= thread_counters.len(),
            forall|j: int| #![trigger thread_counters@[j]]
                0 <= j < i as int
                ==> thread_counters@[j] >= grace_counter,
        decreases thread_counters.len() - i,
    {
        if thread_counters[i] < grace_counter {
            return false;
        }
        i += 1;
    }
    true
}

// --- is_grace_period_complete ---

// r[depends rcu.grace-complete]
/// A grace period that started at `start_counter` is complete when the
/// current counter has advanced past it AND all threads are quiescent
/// relative to `start_counter`.
pub open spec fn is_grace_period_complete_spec(
    start_counter: u64,
    current_counter: u64,
    thread_counters: Seq<u64>,
) -> bool {
    current_counter > start_counter
    && all_threads_quiescent_spec(thread_counters, start_counter)
}

// r[impl rcu.grace-complete]
pub fn is_grace_period_complete(
    start_counter: u64,
    current_counter: u64,
    thread_counters: &Vec<u64>,
) -> (result: bool)
    ensures result == is_grace_period_complete_spec(
        start_counter, current_counter, thread_counters@,
    )
{
    current_counter > start_counter
    && all_threads_quiescent(thread_counters, start_counter)
}

// --- can_free_old_version ---

// r[depends rcu.can-free]
/// Old data published before `retire_counter` can be freed once the
/// grace period starting at `retire_counter` is complete.
pub open spec fn can_free_old_version_spec(
    retire_counter: u64,
    current_counter: u64,
    thread_counters: Seq<u64>,
) -> bool {
    is_grace_period_complete_spec(retire_counter, current_counter, thread_counters)
}

// r[impl rcu.can-free]
pub fn can_free_old_version(
    retire_counter: u64,
    current_counter: u64,
    thread_counters: &Vec<u64>,
) -> (result: bool)
    ensures result == can_free_old_version_spec(
        retire_counter, current_counter, thread_counters@,
    )
{
    is_grace_period_complete(retire_counter, current_counter, thread_counters)
}

// --- version_published_at ---

// r[depends rcu.version-published]
/// The version number when data was published. Equals the grace counter
/// at publish time.
pub open spec fn version_published_at_spec(grace_counter: u64) -> u64 {
    grace_counter
}

// r[impl rcu.version-published]
pub fn version_published_at(grace_counter: u64) -> (result: u64)
    ensures result == version_published_at_spec(grace_counter)
{
    grace_counter
}

// === Proofs ===

// r[verify rcu.is-quiescent]
/// Entering then exiting at same grace counter is quiescent.
proof fn enter_exit_quiescent(grace_counter: u64)
    ensures is_quiescent_spec(exit_read_spec(grace_counter), grace_counter)
{}

// r[verify rcu.advance-grace]
/// Advancing strictly increases the counter.
proof fn advance_strict(grace_counter: u64)
    requires grace_counter < u64::MAX
    ensures advance_grace_period_spec(grace_counter) > grace_counter
{}

// r[verify rcu.advance-grace]
/// Advancing is monotonic.
proof fn advance_monotonic(grace_counter: u64)
    ensures advance_grace_period_spec(grace_counter) >= grace_counter
{}

// r[verify rcu.all-quiescent]
/// Empty thread list is trivially quiescent.
proof fn empty_is_quiescent(grace_counter: u64)
    ensures all_threads_quiescent_spec(Seq::<u64>::empty(), grace_counter)
{}

// r[verify rcu.all-quiescent]
// r[verify rcu.exit-read]
/// All threads that have exited at the current grace counter are quiescent.
proof fn all_exited_are_quiescent(grace_counter: u64, n: nat)
    ensures all_threads_quiescent_spec(
        Seq::new(n, |_i| exit_read_spec(grace_counter)),
        grace_counter,
    )
{}

// r[verify rcu.grace-complete]
/// Grace period not complete if counter hasn't advanced.
proof fn no_advance_not_complete(
    start_counter: u64,
    thread_counters: Seq<u64>,
)
    ensures !is_grace_period_complete_spec(start_counter, start_counter, thread_counters)
{}

// r[verify rcu.grace-complete]
// r[verify rcu.advance-grace]
/// Grace period completes after advance + all quiescent.
proof fn advance_plus_quiescent_completes(
    grace_counter: u64,
    thread_counters: Seq<u64>,
)
    requires
        grace_counter < u64::MAX,
        all_threads_quiescent_spec(thread_counters, grace_counter),
    ensures is_grace_period_complete_spec(
        grace_counter,
        advance_grace_period_spec(grace_counter),
        thread_counters,
    )
{}

// r[verify rcu.can-free]
// r[verify rcu.grace-complete]
/// Can't free if grace period isn't complete.
proof fn no_free_without_grace(
    retire: u64,
    current: u64,
    threads: Seq<u64>,
)
    requires !is_grace_period_complete_spec(retire, current, threads)
    ensures !can_free_old_version_spec(retire, current, threads)
{}

// r[verify rcu.can-free]
// r[verify rcu.all-quiescent]
/// Safety: if can_free, then no thread is in a read-side critical section
/// from before the retire point. Specifically, every thread counter >=
/// retire_counter, meaning every thread has observed the new version.
proof fn free_means_all_observed(
    retire_counter: u64,
    current_counter: u64,
    thread_counters: Seq<u64>,
)
    requires can_free_old_version_spec(retire_counter, current_counter, thread_counters)
    ensures forall|i: int| #![trigger thread_counters[i]]
        0 <= i < thread_counters.len() as int
        ==> thread_counters[i] >= retire_counter
{}

// r[verify rcu.is-quiescent]
// r[verify rcu.advance-grace]
/// A thread quiescent at old counter is also quiescent relative to old
/// after grace advances (since thread_counter >= old and old <= new).
proof fn quiescent_at_old_ok(thread_counter: u64, old_grace: u64)
    requires is_quiescent_spec(thread_counter, old_grace)
    ensures thread_counter >= old_grace
{}

// r[verify rcu.enter-read]
/// enter_read returns the grace counter.
proof fn enter_read_identity(grace_counter: u64)
    ensures enter_read_spec(grace_counter) == grace_counter
{}

// r[verify rcu.version-published]
/// Published version equals grace counter at publish time.
proof fn publish_identity(grace_counter: u64)
    ensures version_published_at_spec(grace_counter) == grace_counter
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn test_quiescent() {
        assert!(is_quiescent(5, 5));
        assert!(!is_quiescent(4, 5));
    }

    #[test]
    fn test_enter_exit() {
        let snapshot = enter_read(10);
        assert_eq!(snapshot, 10);
        let after = exit_read(10);
        assert_eq!(after, 10);
        assert!(is_quiescent(after, 10));
    }

    #[test]
    fn test_advance() {
        assert_eq!(advance_grace_period(0), 1);
        assert_eq!(advance_grace_period(99), 100);
    }

    #[test]
    fn test_all_quiescent() {
        let counters = vec![5u64, 5, 5];
        assert!(all_threads_quiescent(&counters, 5));
        assert!(all_threads_quiescent(&counters, 4));
        assert!(!all_threads_quiescent(&counters, 6));

        let empty: Vec<u64> = Vec::new();
        assert!(all_threads_quiescent(&empty, 100));
    }

    #[test]
    fn test_grace_period() {
        let counters = vec![5u64, 6, 5];
        // Started at 5, now at 6, all threads >= 5
        assert!(is_grace_period_complete(5, 6, &counters));
        // Not complete: counter hasn't advanced
        assert!(!is_grace_period_complete(5, 5, &counters));
        // Not complete: thread 0 is behind
        let behind = vec![4u64, 6, 5];
        assert!(!is_grace_period_complete(5, 6, &behind));
    }

    #[test]
    fn test_can_free() {
        let counters = vec![10u64, 10, 11];
        assert!(can_free_old_version(10, 11, &counters));
        assert!(!can_free_old_version(10, 10, &counters));
    }

    #[test]
    fn test_rcu_scenario() {
        // Writer publishes at grace_counter=5
        let retire_at = version_published_at(5);
        // Advance grace period
        let new_grace = advance_grace_period(5);
        // All threads exit their read sections
        let threads = vec![exit_read(new_grace), exit_read(new_grace), exit_read(new_grace)];
        // Grace period complete — safe to free old version
        assert!(can_free_old_version(retire_at, new_grace, &threads));
    }

    proptest! {
        #[test]
        fn prop_enter_exit_quiescent(gc: u64) {
            let snapshot = enter_read(gc);
            let after = exit_read(gc);
            prop_assert!(is_quiescent(after, gc));
            prop_assert_eq!(snapshot, gc);
        }

        #[test]
        fn prop_advance_monotonic(gc in 0u64..u64::MAX) {
            prop_assert!(advance_grace_period(gc) > gc);
        }

        #[test]
        fn prop_empty_quiescent(gc: u64) {
            let empty: Vec<u64> = Vec::new();
            prop_assert!(all_threads_quiescent(&empty, gc));
        }

        #[test]
        fn prop_all_at_gc_quiescent(gc in 0u64..100, n in 0usize..8) {
            let counters = vec![gc; n];
            prop_assert!(all_threads_quiescent(&counters, gc));
        }

        #[test]
        fn prop_no_advance_not_complete(gc: u64) {
            let counters = vec![gc; 3];
            prop_assert!(!is_grace_period_complete(gc, gc, &counters));
        }

        #[test]
        fn prop_advance_plus_quiescent(gc in 0u64..u64::MAX, n in 0usize..8) {
            let counters = vec![gc; n];
            let new_gc = advance_grace_period(gc);
            prop_assert!(is_grace_period_complete(gc, new_gc, &counters));
        }

        #[test]
        fn prop_free_needs_grace(retire: u64, current: u64, n in 0usize..4) {
            let counters = vec![retire; n];
            if can_free_old_version(retire, current, &counters) {
                prop_assert!(current > retire);
                for c in &counters {
                    prop_assert!(*c >= retire);
                }
            }
        }
    }
}
