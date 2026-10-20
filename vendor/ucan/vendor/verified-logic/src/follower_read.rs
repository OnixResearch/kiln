//! Verified follower read protocol for Raft.
//!
//! Allows followers to serve linearizable reads without forwarding to the
//! leader. The protocol:
//! 1. Follower receives read request, records its current `commit_index` as `read_index`.
//! 2. Follower asks leader to confirm it is still the leader (leader performs a quorum heartbeat
//!    check).
//! 3. Leader responds with its current `commit_index` (which is ≥ the follower's `read_index` if
//!    the leader is current).
//! 4. Follower waits until `last_applied >= leader_commit_index`.
//! 5. Follower serves the read from local state machine.
//!
//! This differs from `read_index.rs` (leader-side reads) and `lease_read.rs`
//! (lease-based reads). Follower reads trade latency (extra network hop to
//! leader) for read throughput (offload reads from leader).

use vstd::prelude::*;

verus! {

// --- Request phase ---

// r[depends follower-read.register]
/// Spec: register a follower read request.
///
/// Records the follower's current commit_index as the read's minimum
/// freshness guarantee. The read must not return data older than this.
pub open spec fn register_read_spec(commit_index: u64) -> u64 {
    commit_index
}

// r[impl follower-read.register]
/// Register a read request at the follower's current commit index.
pub fn register_read(commit_index: u64) -> (read_index: u64)
    ensures read_index == register_read_spec(commit_index)
{
    commit_index
}

// --- Leader confirmation phase ---

// r[depends follower-read.leader-confirms]
/// Spec: is the leader's response valid for this follower read?
///
/// The leader must confirm:
/// 1. It is still leader (implicit — it responded to the heartbeat).
/// 2. Its commit_index is provided (leader_commit).
///
/// The follower uses max(read_index, leader_commit) as the safe read point
/// to guarantee linearizability even if the leader committed more entries
/// since the follower's request.
pub open spec fn safe_read_point_spec(
    read_index: u64,
    leader_commit: u64,
) -> u64 {
    if leader_commit > read_index {
        leader_commit
    } else {
        read_index
    }
}

// r[impl follower-read.leader-confirms]
/// Compute the safe read point after leader confirmation.
pub fn safe_read_point(
    read_index: u64,
    leader_commit: u64,
) -> (result: u64)
    ensures result == safe_read_point_spec(read_index, leader_commit)
{
    if leader_commit > read_index {
        leader_commit
    } else {
        read_index
    }
}

// --- Apply wait phase ---

// r[depends follower-read.can-serve]
/// Spec: can the follower serve this read from its local state machine?
///
/// The follower must have applied all entries through the safe read point.
/// `last_applied >= safe_point` guarantees the state machine reflects
/// all committed state up to the read's linearization point.
pub open spec fn can_serve_read_spec(
    last_applied: u64,
    safe_point: u64,
) -> bool {
    last_applied >= safe_point
}

// r[impl follower-read.can-serve]
/// Check if the follower's state machine is ready to serve the read.
pub fn can_serve_read(
    last_applied: u64,
    safe_point: u64,
) -> (result: bool)
    ensures result == can_serve_read_spec(last_applied, safe_point)
{
    last_applied >= safe_point
}

// --- Staleness detection ---

// r[depends follower-read.stale-leader]
/// Spec: has the leader term changed since the read was registered?
///
/// If the leader's term in the confirmation response differs from the
/// term when the read was registered, the leader may have changed.
/// The read must be retried.
pub open spec fn is_leader_stale_spec(
    registered_term: u64,
    response_term: u64,
) -> bool {
    response_term != registered_term
}

// r[impl follower-read.stale-leader]
/// Check if the leader changed since the read was registered.
pub fn is_leader_stale(
    registered_term: u64,
    response_term: u64,
) -> (result: bool)
    ensures result == is_leader_stale_spec(registered_term, response_term)
{
    response_term != registered_term
}

// --- Timeout ---

// r[depends follower-read.timeout]
/// Spec: has the read request timed out?
///
/// Follower reads have a deadline. If the leader doesn't respond in time,
/// the read fails. The client can retry (possibly at a different node).
pub open spec fn is_read_timed_out_spec(
    registered_at: u64,
    now: u64,
    timeout: u64,
) -> bool {
    if timeout > u64::MAX as u64 - registered_at {
        false  // overflow: deadline is effectively infinite
    } else {
        now >= registered_at + timeout
    }
}

// r[impl follower-read.timeout]
/// Check if the read request has timed out.
pub fn is_read_timed_out(
    registered_at: u64,
    now: u64,
    timeout: u64,
) -> (result: bool)
    ensures result == is_read_timed_out_spec(registered_at, now, timeout)
{
    if timeout > u64::MAX - registered_at {
        false
    } else {
        now >= registered_at + timeout
    }
}

// --- entries_behind ---

// r[depends follower-read.entries-behind]
/// Spec: how many entries must the follower apply before serving?
///
/// Returns 0 when the follower is already caught up.
pub open spec fn entries_behind_spec(
    last_applied: u64,
    safe_point: u64,
) -> u64 {
    if safe_point > last_applied {
        (safe_point - last_applied) as u64
    } else {
        0u64
    }
}

// r[impl follower-read.entries-behind]
/// Compute how many entries remain to apply.
pub fn entries_behind(
    last_applied: u64,
    safe_point: u64,
) -> (result: u64)
    ensures result == entries_behind_spec(last_applied, safe_point)
{
    if safe_point > last_applied {
        safe_point - last_applied
    } else {
        0
    }
}

// --- Safety proofs ---

// r[verify follower-read.register]
/// Read index is exactly the commit index at registration time.
proof fn register_captures_commit(commit_index: u64)
    ensures register_read_spec(commit_index) == commit_index
{}

// r[verify follower-read.leader-confirms]
/// Safe read point is at least the original read index.
proof fn safe_point_ge_read_index(read_index: u64, leader_commit: u64)
    ensures safe_read_point_spec(read_index, leader_commit) >= read_index
{}

// r[verify follower-read.leader-confirms]
/// Safe read point is at least the leader's commit index.
proof fn safe_point_ge_leader_commit(read_index: u64, leader_commit: u64)
    ensures safe_read_point_spec(read_index, leader_commit) >= leader_commit
{}

// r[verify follower-read.can-serve]
/// Zero entries behind means the read can be served.
proof fn caught_up_can_serve(last_applied: u64, safe_point: u64)
    requires entries_behind_spec(last_applied, safe_point) == 0
    ensures can_serve_read_spec(last_applied, safe_point)
{}

// r[verify follower-read.can-serve]
/// Can serve implies zero entries behind.
proof fn can_serve_implies_caught_up(last_applied: u64, safe_point: u64)
    requires can_serve_read_spec(last_applied, safe_point)
    ensures entries_behind_spec(last_applied, safe_point) == 0
{}

// r[verify follower-read.stale-leader]
/// Same term is not stale.
proof fn same_term_not_stale(term: u64)
    ensures !is_leader_stale_spec(term, term)
{}

// r[verify follower-read.leader-confirms]
/// Safe point with same leader commit is the read index.
proof fn same_commit_preserves(read_index: u64)
    ensures safe_read_point_spec(read_index, read_index) == read_index
{}

// r[verify follower-read.leader-confirms]
/// Safe point is max of read_index and leader_commit.
proof fn safe_point_is_max(read_index: u64, leader_commit: u64)
    ensures safe_read_point_spec(read_index, leader_commit)
        == (if leader_commit > read_index { leader_commit } else { read_index })
{}

// r[verify follower-read.timeout]
/// Read never times out at registration time (timeout > 0).
proof fn no_immediate_timeout(registered_at: u64, timeout: u64)
    requires
        timeout > 0,
        registered_at as int + timeout as int <= u64::MAX as int,
    ensures !is_read_timed_out_spec(registered_at, registered_at, timeout)
{}

// r[verify follower-read.entries-behind]
/// Applying entries reduces the behind count.
proof fn applying_reduces_behind(
    last_applied: u64,
    safe_point: u64,
    applied: u64,
)
    requires
        safe_point > last_applied,
        applied > 0,
        last_applied as int + applied as int <= safe_point as int,
    ensures
        entries_behind_spec((last_applied + applied) as u64, safe_point)
            < entries_behind_spec(last_applied, safe_point)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify follower-read.register]
    #[test]
    fn test_register_read() {
        assert_eq!(register_read(42), 42);
        assert_eq!(register_read(0), 0);
    }

    // r[verify follower-read.leader-confirms]
    #[test]
    fn test_safe_read_point() {
        assert_eq!(safe_read_point(5, 10), 10); // leader ahead
        assert_eq!(safe_read_point(10, 5), 10); // follower ahead
        assert_eq!(safe_read_point(5, 5), 5); // same
    }

    // r[verify follower-read.can-serve]
    #[test]
    fn test_can_serve_read() {
        assert!(can_serve_read(10, 5)); // applied past safe point
        assert!(can_serve_read(5, 5)); // exactly at safe point
        assert!(!can_serve_read(3, 5)); // behind
    }

    // r[verify follower-read.stale-leader]
    #[test]
    fn test_is_leader_stale() {
        assert!(!is_leader_stale(3, 3)); // same term
        assert!(is_leader_stale(3, 4)); // term changed
        assert!(is_leader_stale(4, 3)); // term went backward (shouldn't happen but detected)
    }

    // r[verify follower-read.timeout]
    #[test]
    fn test_is_read_timed_out() {
        assert!(!is_read_timed_out(100, 100, 50)); // at registration
        assert!(!is_read_timed_out(100, 140, 50)); // before deadline
        assert!(is_read_timed_out(100, 150, 50)); // at deadline
        assert!(is_read_timed_out(100, 200, 50)); // past deadline
    }

    // r[verify follower-read.entries-behind]
    #[test]
    fn test_entries_behind() {
        assert_eq!(entries_behind(5, 10), 5);
        assert_eq!(entries_behind(10, 10), 0);
        assert_eq!(entries_behind(15, 10), 0);
    }

    proptest! {
        // r[verify follower-read.leader-confirms]
        #[test]
        fn prop_safe_point_ge_both(read_index: u64, leader_commit: u64) {
            let sp = safe_read_point(read_index, leader_commit);
            prop_assert!(sp >= read_index);
            prop_assert!(sp >= leader_commit);
        }

        // r[verify follower-read.can-serve]
        #[test]
        fn prop_caught_up_iff_can_serve(last_applied: u64, safe_point: u64) {
            let behind = entries_behind(last_applied, safe_point);
            let serve = can_serve_read(last_applied, safe_point);
            prop_assert_eq!(behind == 0, serve);
        }

        // r[verify follower-read.timeout]
        #[test]
        fn prop_no_immediate_timeout(
            registered_at in 0u64..u64::MAX / 2,
            timeout in 1u64..u64::MAX / 2,
        ) {
            prop_assert!(!is_read_timed_out(registered_at, registered_at, timeout));
        }

        // r[verify follower-read.leader-confirms]
        #[test]
        fn prop_safe_point_model(read_index: u64, leader_commit: u64) {
            let sp = safe_read_point(read_index, leader_commit);
            let expected = std::cmp::max(read_index, leader_commit);
            prop_assert_eq!(sp, expected);
        }
    }
}
