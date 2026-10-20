//! Verified heartbeat primitives for Raft.
//!
//! Heartbeats are empty AppendEntries RPCs that the leader sends
//! periodically to maintain authority and prevent followers from
//! starting elections. This module covers the timing decision
//! (`should_send`), follower-side timer reset, leader liveness
//! detection, and heartbeat acknowledgment validation.

use vstd::prelude::*;

verus! {

// --- is_heartbeat ---

// r[depends heartbeat.is-heartbeat]
/// Spec: is this AppendEntries a heartbeat?
///
/// A heartbeat carries zero log entries. It serves only to assert
/// leader authority and deliver the leader's commit index.
pub open spec fn is_heartbeat_spec(entry_count: u64) -> bool {
    entry_count == 0
}

// r[impl heartbeat.is-heartbeat]
/// Check if an AppendEntries message is a heartbeat.
pub fn is_heartbeat(entry_count: u64) -> (result: bool)
    ensures result == is_heartbeat_spec(entry_count)
{
    entry_count == 0
}

// --- should_send_heartbeat ---

// r[depends heartbeat.should-send]
/// Spec: should the leader send a heartbeat now?
///
/// The leader sends a heartbeat when elapsed_ms >= heartbeat_interval_ms.
/// The interval must be much shorter than the election timeout so
/// followers don't start spurious elections.
pub open spec fn should_send_heartbeat_spec(
    elapsed_ms: u64,
    heartbeat_interval_ms: u64,
) -> bool {
    elapsed_ms >= heartbeat_interval_ms
}

// r[impl heartbeat.should-send]
/// Check if a heartbeat should be sent.
pub fn should_send_heartbeat(
    elapsed_ms: u64,
    heartbeat_interval_ms: u64,
) -> (result: bool)
    ensures result == should_send_heartbeat_spec(elapsed_ms, heartbeat_interval_ms)
{
    elapsed_ms >= heartbeat_interval_ms
}

// --- should_reset_election_timer ---

// r[depends heartbeat.reset-election-timer]
/// Spec: should the follower reset its election timer on this heartbeat?
///
/// A heartbeat resets the timer only when the message comes from
/// the current leader (msg_term >= current_term) and from_leader is
/// true. Stale-term messages and messages from non-leaders do NOT
/// reset the timer.
pub open spec fn should_reset_election_timer_spec(
    msg_term: u64,
    current_term: u64,
    from_leader: bool,
) -> bool {
    msg_term >= current_term && from_leader
}

// r[impl heartbeat.reset-election-timer]
/// Check if this heartbeat should reset the follower's election timer.
pub fn should_reset_election_timer(
    msg_term: u64,
    current_term: u64,
    from_leader: bool,
) -> (result: bool)
    ensures result == should_reset_election_timer_spec(msg_term, current_term, from_leader)
{
    msg_term >= current_term && from_leader
}

// --- is_leader_alive ---

// r[depends heartbeat.leader-alive]
/// Spec: does the follower consider the leader alive?
///
/// The leader is alive if the time since the last heartbeat is
/// less than the election timeout. Uses saturating subtraction
/// to avoid underflow.
pub open spec fn is_leader_alive_spec(
    last_heartbeat_time: u64,
    now: u64,
    election_timeout: u64,
) -> bool {
    let elapsed = if now >= last_heartbeat_time {
        (now - last_heartbeat_time) as u64
    } else {
        0u64
    };
    elapsed < election_timeout
}

// r[impl heartbeat.leader-alive]
/// Check if the leader is considered alive.
pub fn is_leader_alive(
    last_heartbeat_time: u64,
    now: u64,
    election_timeout: u64,
) -> (result: bool)
    ensures result == is_leader_alive_spec(last_heartbeat_time, now, election_timeout)
{
    let elapsed = if now >= last_heartbeat_time {
        now - last_heartbeat_time
    } else {
        0
    };
    elapsed < election_timeout
}

// --- heartbeat_ack_valid ---

// r[depends heartbeat.ack]
/// Spec: is this heartbeat acknowledgment valid?
///
/// Valid when ack_term == leader_term. Stale acks (ack_term < leader_term)
/// are ignored. Higher-term acks signal the leader is stale and trigger
/// step-down via term_authority.
pub open spec fn heartbeat_ack_valid_spec(
    ack_term: u64,
    leader_term: u64,
) -> bool {
    ack_term == leader_term
}

// r[impl heartbeat.ack]
/// Check if a heartbeat ack is valid.
pub fn heartbeat_ack_valid(
    ack_term: u64,
    leader_term: u64,
) -> (result: bool)
    ensures result == heartbeat_ack_valid_spec(ack_term, leader_term)
{
    ack_term == leader_term
}

// --- Safety proofs ---

// r[verify heartbeat.is-heartbeat]
/// Non-empty AppendEntries is not a heartbeat.
proof fn non_empty_not_heartbeat(count: u64)
    requires count > 0
    ensures !is_heartbeat_spec(count)
{}

// r[verify heartbeat.should-send]
/// Heartbeat not due at time zero with positive interval.
proof fn no_heartbeat_at_zero(interval: u64)
    requires interval > 0
    ensures !should_send_heartbeat_spec(0, interval)
{}

// r[verify heartbeat.should-send]
/// Heartbeat due at exactly the interval.
proof fn heartbeat_at_boundary(interval: u64)
    ensures should_send_heartbeat_spec(interval, interval)
{}

// r[verify heartbeat.should-send]
/// Heartbeat is monotonic: once due, stays due as time advances.
proof fn heartbeat_monotonic(elapsed1: u64, elapsed2: u64, interval: u64)
    requires
        elapsed1 <= elapsed2,
        should_send_heartbeat_spec(elapsed1, interval),
    ensures should_send_heartbeat_spec(elapsed2, interval)
{}

// r[verify heartbeat.reset-election-timer]
/// Stale-term messages do not reset the timer.
proof fn stale_term_no_reset(msg_term: u64, current_term: u64, from_leader: bool)
    requires msg_term < current_term
    ensures !should_reset_election_timer_spec(msg_term, current_term, from_leader)
{}

// r[verify heartbeat.reset-election-timer]
/// Messages from non-leaders do not reset the timer.
proof fn non_leader_no_reset(msg_term: u64, current_term: u64)
    ensures !should_reset_election_timer_spec(msg_term, current_term, false)
{}

// r[verify heartbeat.leader-alive]
/// Leader alive immediately after a heartbeat (elapsed = 0).
proof fn alive_at_heartbeat(election_timeout: u64)
    requires election_timeout > 0
    ensures is_leader_alive_spec(0, 0, election_timeout)
{}

// r[verify heartbeat.leader-alive]
/// Leader dead after election timeout elapses.
proof fn dead_after_timeout(last: u64, timeout: u64)
    requires timeout <= u64::MAX - last
    ensures !is_leader_alive_spec(last, (last + timeout) as u64, timeout)
{}

// r[verify heartbeat.ack]
/// Stale ack is invalid.
proof fn stale_ack_invalid(ack_term: u64, leader_term: u64)
    requires ack_term < leader_term
    ensures !heartbeat_ack_valid_spec(ack_term, leader_term)
{}

// r[verify heartbeat.ack]
/// Same-term ack is valid.
proof fn same_term_ack_valid(term: u64)
    ensures heartbeat_ack_valid_spec(term, term)
{}

// r[verify heartbeat.ack]
/// Higher-term ack signals stale leader.
proof fn higher_term_ack_invalid(ack_term: u64, leader_term: u64)
    requires ack_term > leader_term
    ensures !heartbeat_ack_valid_spec(ack_term, leader_term)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify heartbeat.is-heartbeat]
    #[test]
    fn test_is_heartbeat() {
        assert!(is_heartbeat(0));
        assert!(!is_heartbeat(1));
        assert!(!is_heartbeat(100));
    }

    // r[verify heartbeat.should-send]
    #[test]
    fn test_should_send_heartbeat() {
        assert!(!should_send_heartbeat(0, 100));
        assert!(!should_send_heartbeat(99, 100));
        assert!(should_send_heartbeat(100, 100));
        assert!(should_send_heartbeat(200, 100));
        // Zero interval: always send
        assert!(should_send_heartbeat(0, 0));
    }

    // r[verify heartbeat.reset-election-timer]
    #[test]
    fn test_should_reset_election_timer() {
        // Valid heartbeat from current leader
        assert!(should_reset_election_timer(3, 3, true));
        // Higher-term leader
        assert!(should_reset_election_timer(4, 3, true));
        // Stale term
        assert!(!should_reset_election_timer(2, 3, true));
        // Not from leader
        assert!(!should_reset_election_timer(3, 3, false));
    }

    // r[verify heartbeat.leader-alive]
    #[test]
    fn test_is_leader_alive() {
        // Just received heartbeat
        assert!(is_leader_alive(100, 100, 150));
        // Within timeout
        assert!(is_leader_alive(100, 200, 150));
        // At timeout boundary: dead
        assert!(!is_leader_alive(100, 250, 150));
        // Past timeout
        assert!(!is_leader_alive(100, 300, 150));
        // Zero timeout: never alive
        assert!(!is_leader_alive(0, 0, 0));
    }

    // r[verify heartbeat.ack]
    #[test]
    fn test_heartbeat_ack_valid() {
        assert!(heartbeat_ack_valid(3, 3));
        assert!(!heartbeat_ack_valid(2, 3));
        assert!(!heartbeat_ack_valid(4, 3));
    }

    proptest! {
        // r[verify heartbeat.should-send]
        #[test]
        fn prop_heartbeat_monotonic(
            elapsed1 in 0u64..1000,
            elapsed2 in 0u64..1000,
            interval in 1u64..500,
        ) {
            if elapsed1 <= elapsed2 && should_send_heartbeat(elapsed1, interval) {
                prop_assert!(should_send_heartbeat(elapsed2, interval));
            }
        }

        // r[verify heartbeat.reset-election-timer]
        #[test]
        fn prop_stale_never_resets(
            msg_term in 0u64..50,
            current_term in 51u64..100,
            from_leader: bool,
        ) {
            prop_assert!(!should_reset_election_timer(msg_term, current_term, from_leader));
        }

        // r[verify heartbeat.leader-alive]
        #[test]
        fn prop_leader_alive_monotonic_death(
            last in 0u64..500,
            now1 in 0u64..1000,
            now2 in 0u64..1000,
            timeout in 1u64..500,
        ) {
            // If leader dead at now1, still dead at now2 >= now1
            if now1 <= now2 && !is_leader_alive(last, now1, timeout) {
                prop_assert!(!is_leader_alive(last, now2, timeout));
            }
        }

        // r[verify heartbeat.ack]
        #[test]
        fn prop_only_same_term_valid(ack: u64, leader: u64) {
            prop_assert_eq!(heartbeat_ack_valid(ack, leader), ack == leader);
        }
    }
}
