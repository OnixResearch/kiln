//! Verified thread barrier primitives.
//!
//! A barrier synchronizes N threads: each thread calls arrive, and all
//! block until the Nth arrival. A generation counter distinguishes
//! successive barrier uses (so a thread from a prior round doesn't
//! accidentally "arrive" for the current round).

use vstd::prelude::*;

verus! {

// --- arrive ---

// r[depends barrier.arrive]
/// Increment the arrival count. Saturates at u64::MAX.
pub open spec fn arrive_spec(arrived: u64) -> u64 {
    if arrived < u64::MAX { (arrived + 1) as u64 } else { u64::MAX }
}

// r[impl barrier.arrive]
pub fn arrive(arrived: u64) -> (result: u64)
    requires arrived < u64::MAX
    ensures result == arrive_spec(arrived)
{
    arrived + 1
}

// --- all_arrived ---

// r[depends barrier.all-arrived]
/// All threads have arrived when the count reaches expected.
pub open spec fn all_arrived_spec(arrived: u64, expected: u64) -> bool {
    arrived >= expected
}

// r[impl barrier.all-arrived]
pub fn all_arrived(arrived: u64, expected: u64) -> (result: bool)
    ensures result == all_arrived_spec(arrived, expected)
{
    arrived >= expected
}

// --- should_release ---

// r[depends barrier.should-release]
/// The barrier should release (wake all threads) when arrived == expected.
/// This is the exact trigger point.
pub open spec fn should_release_spec(arrived: u64, expected: u64) -> bool {
    arrived == expected
}

// r[impl barrier.should-release]
pub fn should_release(arrived: u64, expected: u64) -> (result: bool)
    ensures result == should_release_spec(arrived, expected)
{
    arrived == expected
}

// --- remaining ---

// r[depends barrier.remaining]
/// How many more arrivals are needed. Returns 0 when all have arrived.
pub open spec fn remaining_spec(arrived: u64, expected: u64) -> u64 {
    if arrived >= expected { 0 } else { (expected - arrived) as u64 }
}

// r[impl barrier.remaining]
pub fn remaining(arrived: u64, expected: u64) -> (result: u64)
    ensures result == remaining_spec(arrived, expected)
{
    if arrived >= expected { 0 } else { expected - arrived }
}

// --- reset ---

// r[depends barrier.reset]
/// Reset the barrier for the next round: zero arrivals, bump generation.
/// Returns (new_arrived, new_generation).
pub open spec fn reset_arrived_spec() -> u64 { 0 }

// r[depends barrier.reset]
pub open spec fn reset_generation_spec(generation: u64) -> u64 {
    if generation < u64::MAX { (generation + 1) as u64 } else { 0 }
}

// r[impl barrier.reset]
pub fn reset_arrived() -> (result: u64)
    ensures result == reset_arrived_spec()
{
    0
}

// r[impl barrier.reset]
pub fn reset_generation(generation: u64) -> (result: u64)
    ensures result == reset_generation_spec(generation)
{
    if generation < u64::MAX { generation + 1 } else { 0 }
}

// --- is_same_generation ---

// r[depends barrier.same-generation]
/// Two operations are in the same barrier round when their generations match.
pub open spec fn is_same_generation_spec(g1: u64, g2: u64) -> bool {
    g1 == g2
}

// r[impl barrier.same-generation]
pub fn is_same_generation(g1: u64, g2: u64) -> (result: bool)
    ensures result == is_same_generation_spec(g1, g2)
{
    g1 == g2
}

// --- is_leader ---

// r[depends barrier.is-leader]
/// The thread that makes arrived == expected is the "leader" that
/// triggers the release. Only one thread can be the leader per round.
pub open spec fn is_leader_spec(my_arrival: u64, expected: u64) -> bool {
    my_arrival == expected
}

// r[impl barrier.is-leader]
pub fn is_leader(my_arrival: u64, expected: u64) -> (result: bool)
    ensures result == is_leader_spec(my_arrival, expected)
{
    my_arrival == expected
}

// === Proofs ===

// r[verify barrier.arrive]
/// arrive is monotonic.
proof fn arrive_monotonic(arrived: u64)
    ensures arrive_spec(arrived) >= arrived
{}

// r[verify barrier.arrive]
/// arrive from non-max strictly increases.
proof fn arrive_strict(arrived: u64)
    requires arrived < u64::MAX
    ensures arrive_spec(arrived) > arrived
{}

// r[verify barrier.arrive]
// r[verify barrier.all-arrived]
/// Arriving from expected-1 triggers all_arrived.
proof fn last_arrival_triggers(expected: u64)
    requires expected > 0
    ensures all_arrived_spec(arrive_spec((expected - 1) as u64), expected)
{}

// r[verify barrier.arrive]
// r[verify barrier.should-release]
/// Arriving from expected-1 triggers should_release.
proof fn last_arrival_releases(expected: u64)
    requires expected > 0
    ensures should_release_spec(arrive_spec((expected - 1) as u64), expected)
{}

// r[verify barrier.arrive]
// r[verify barrier.should-release]
/// Arriving before expected-1 does not trigger release.
proof fn early_arrival_no_release(arrived: u64, expected: u64)
    requires arrived + 1 < expected
    ensures !should_release_spec(arrive_spec(arrived), expected)
{}

// r[verify barrier.remaining]
/// Zero remaining iff all arrived.
proof fn remaining_zero_iff_done(arrived: u64, expected: u64)
    ensures (remaining_spec(arrived, expected) == 0) == all_arrived_spec(arrived, expected)
{}

// r[verify barrier.remaining]
// r[verify barrier.arrive]
/// Each arrival decreases remaining by 1 (when remaining > 0).
proof fn arrive_decreases_remaining(arrived: u64, expected: u64)
    requires arrived < expected, arrived < u64::MAX
    ensures remaining_spec(arrive_spec(arrived), expected)
        == (remaining_spec(arrived, expected) - 1) as u64
{}

// r[verify barrier.reset]
/// Reset produces zero arrivals.
proof fn reset_zero_arrivals()
    ensures reset_arrived_spec() == 0
{}

// r[verify barrier.reset]
/// Reset produces a different generation (when not wrapping).
proof fn reset_changes_generation(generation: u64)
    requires generation < u64::MAX
    ensures reset_generation_spec(generation) != generation
{}

// r[verify barrier.reset]
// r[verify barrier.all-arrived]
/// After reset, not all arrived (when expected > 0).
proof fn reset_not_arrived(expected: u64)
    requires expected > 0
    ensures !all_arrived_spec(reset_arrived_spec(), expected)
{}

// r[verify barrier.reset]
// r[verify barrier.remaining]
/// After reset, remaining == expected.
proof fn reset_full_remaining(expected: u64)
    ensures remaining_spec(reset_arrived_spec(), expected) == expected
{}

// r[verify barrier.same-generation]
/// Same generation is reflexive.
proof fn same_gen_reflexive(g: u64)
    ensures is_same_generation_spec(g, g)
{}

// r[verify barrier.same-generation]
/// Same generation is symmetric.
proof fn same_gen_symmetric(g1: u64, g2: u64)
    ensures is_same_generation_spec(g1, g2) == is_same_generation_spec(g2, g1)
{}

// r[verify barrier.reset]
// r[verify barrier.same-generation]
/// After reset, new generation differs from old (non-wrapping).
proof fn reset_new_generation(generation: u64)
    requires generation < u64::MAX
    ensures !is_same_generation_spec(generation, reset_generation_spec(generation))
{}

// r[verify barrier.is-leader]
// r[verify barrier.should-release]
/// Leader arrival triggers release.
proof fn leader_triggers_release(my_arrival: u64, expected: u64)
    requires is_leader_spec(my_arrival, expected)
    ensures should_release_spec(my_arrival, expected)
{}

// r[verify barrier.is-leader]
// r[verify barrier.arrive]
/// The thread arriving at position expected is the leader.
proof fn last_arriver_is_leader(arrived: u64, expected: u64)
    requires arrived + 1 == expected, arrived < u64::MAX
    ensures is_leader_spec(arrive_spec(arrived), expected)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify barrier.arrive]
    #[test]
    fn test_arrive() {
        assert_eq!(arrive(0), 1);
        assert_eq!(arrive(4), 5);
        assert_eq!(arrive(u64::MAX - 1), u64::MAX);
    }

    // r[verify barrier.all-arrived]
    #[test]
    fn test_all_arrived() {
        assert!(!all_arrived(0, 4));
        assert!(!all_arrived(3, 4));
        assert!(all_arrived(4, 4));
        assert!(all_arrived(5, 4));
    }

    // r[verify barrier.should-release]
    #[test]
    fn test_should_release() {
        assert!(!should_release(3, 4));
        assert!(should_release(4, 4));
        assert!(!should_release(5, 4));
    }

    // r[verify barrier.remaining]
    #[test]
    fn test_remaining() {
        assert_eq!(remaining(0, 4), 4);
        assert_eq!(remaining(2, 4), 2);
        assert_eq!(remaining(4, 4), 0);
        assert_eq!(remaining(5, 4), 0);
    }

    // r[verify barrier.reset]
    #[test]
    fn test_reset() {
        assert_eq!(reset_arrived(), 0);
        assert_eq!(reset_generation(0), 1);
        assert_eq!(reset_generation(99), 100);
        assert_eq!(reset_generation(u64::MAX), 0); // wraps
    }

    // r[verify barrier.same-generation]
    #[test]
    fn test_same_generation() {
        assert!(is_same_generation(5, 5));
        assert!(!is_same_generation(5, 6));
    }

    // r[verify barrier.is-leader]
    #[test]
    fn test_is_leader() {
        assert!(!is_leader(3, 4));
        assert!(is_leader(4, 4));
    }

    #[test]
    fn test_barrier_scenario() {
        let expected = 3u64;
        let generation = 0u64;

        // Three threads arrive
        let a1 = arrive(0); // thread 1
        assert!(!all_arrived(a1, expected));
        let a2 = arrive(a1); // thread 2
        assert!(!all_arrived(a2, expected));
        let a3 = arrive(a2); // thread 3 — leader
        assert!(all_arrived(a3, expected));
        assert!(should_release(a3, expected));
        assert!(is_leader(a3, expected));

        // Reset for next round
        let new_arrived = reset_arrived();
        let new_gen = reset_generation(generation);
        assert_eq!(new_arrived, 0);
        assert!(!is_same_generation(generation, new_gen));
        assert!(!all_arrived(new_arrived, expected));
    }

    proptest! {
        // r[verify barrier.arrive]
        #[test]
        fn prop_arrive_monotonic(arrived in 0u64..u64::MAX) {
            prop_assert!(arrive(arrived) >= arrived);
        }

        // r[verify barrier.arrive]
        // r[verify barrier.should-release]
        #[test]
        fn prop_last_arrival_releases(expected in 1u64..1000) {
            let arrived = expected - 1;
            prop_assert!(should_release(arrive(arrived), expected));
        }

        // r[verify barrier.arrive]
        // r[verify barrier.should-release]
        #[test]
        fn prop_early_no_release(arrived in 0u64..998, expected in 2u64..1000) {
            if arrived + 1 < expected {
                prop_assert!(!should_release(arrive(arrived), expected));
            }
        }

        // r[verify barrier.remaining]
        #[test]
        fn prop_remaining_consistent(arrived in 0u64..1000, expected in 0u64..1000) {
            prop_assert_eq!(remaining(arrived, expected) == 0, all_arrived(arrived, expected));
        }

        // r[verify barrier.remaining]
        // r[verify barrier.arrive]
        #[test]
        fn prop_arrive_decreases_remaining(arrived in 0u64..999, expected in 1u64..1000) {
            if arrived < expected {
                let rem_before = remaining(arrived, expected);
                let rem_after = remaining(arrive(arrived), expected);
                prop_assert_eq!(rem_after, rem_before - 1);
            }
        }

        // r[verify barrier.reset]
        // r[verify barrier.all-arrived]
        #[test]
        fn prop_reset_not_arrived(expected in 1u64..1000) {
            prop_assert!(!all_arrived(reset_arrived(), expected));
        }

        // r[verify barrier.reset]
        // r[verify barrier.remaining]
        #[test]
        fn prop_reset_full_remaining(expected in 0u64..1000) {
            prop_assert_eq!(remaining(reset_arrived(), expected), expected);
        }

        // r[verify barrier.reset]
        // r[verify barrier.same-generation]
        #[test]
        fn prop_reset_changes_generation(generation in 0u64..u64::MAX) {
            prop_assert!(!is_same_generation(generation, reset_generation(generation)));
        }

        // r[verify barrier.same-generation]
        #[test]
        fn prop_same_gen_reflexive(g: u64) {
            prop_assert!(is_same_generation(g, g));
        }

        // r[verify barrier.is-leader]
        // r[verify barrier.arrive]
        #[test]
        fn prop_exactly_one_leader(expected in 1u64..100) {
            // Walk from 0 to expected: exactly one arrival is the leader
            let mut leaders = 0u64;
            let mut arrived = 0u64;
            for _ in 0..expected {
                arrived = arrive(arrived);
                if is_leader(arrived, expected) {
                    leaders += 1;
                }
            }
            prop_assert_eq!(leaders, 1);
        }
    }
}
