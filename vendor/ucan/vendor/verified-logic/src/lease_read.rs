//! Verified lease-based reads for Raft.
//!
//! An alternative to `ReadIndex` that avoids the per-read network round-trip.
//! The leader serves reads if its lease hasn't expired, accounting for
//! worst-case clock drift. The lease starts when the leader confirms
//! authority via a quorum heartbeat, and expires strictly before any
//! other server's election timeout could fire.

#![allow(clippy::wildcard_imports)]

use vstd::prelude::*;

use crate::election::*;

verus! {

// --- is_lease_valid ---

// r[depends lease-read.is-valid]
/// Spec: is the leader's read lease currently valid?
///
/// The lease window is [start, start + election_timeout - drift).
/// The lease expires before a new election could start, even with
/// worst-case clock drift.
pub open spec fn is_lease_valid_spec(
    lease_start: u64,
    election_timeout: u64,
    clock_drift_bound: u64,
    now: u64,
) -> bool {
    // Effective lease duration: timeout - drift (saturating)
    let duration = if election_timeout > clock_drift_bound {
        (election_timeout - clock_drift_bound) as u64
    } else {
        0u64
    };
    // Lease expiry: start + duration (saturating)
    let expiry = if (lease_start as int + duration as int) <= u64::MAX as int {
        (lease_start + duration) as u64
    } else {
        u64::MAX
    };
    now < expiry
}

// r[impl lease-read.is-valid]
/// Check if the read lease is valid.
pub fn is_lease_valid(
    lease_start: u64,
    election_timeout: u64,
    clock_drift_bound: u64,
    now: u64,
) -> (result: bool)
    ensures result == is_lease_valid_spec(lease_start, election_timeout, clock_drift_bound, now)
{
    let duration = if election_timeout > clock_drift_bound {
        election_timeout - clock_drift_bound
    } else {
        0
    };
    let expiry = if duration <= u64::MAX - lease_start {
        lease_start + duration
    } else {
        u64::MAX
    };
    now < expiry
}

// --- start_lease ---

// r[depends lease-read.start]
/// Spec: record the start of a new lease.
pub open spec fn start_lease_spec(now: u64) -> u64 {
    now
}

// r[impl lease-read.start]
/// Start a new read lease at the current time.
pub fn start_lease(now: u64) -> (result: u64)
    ensures result == start_lease_spec(now)
{
    now
}

// --- lease_read_safe ---

// r[depends lease-read.requires-commit]
/// Spec: is it safe to serve a lease-based read?
///
/// Requires BOTH a valid lease AND the commit index past the read point.
/// A valid lease alone is insufficient.
pub open spec fn lease_read_safe_spec(
    lease_valid: bool,
    commit_index: u64,
    read_index: u64,
) -> bool {
    lease_valid && commit_index >= read_index
}

// r[impl lease-read.requires-commit]
/// Check if a lease read can be served.
pub fn lease_read_safe(
    lease_valid: bool,
    commit_index: u64,
    read_index: u64,
) -> (result: bool)
    ensures result == lease_read_safe_spec(lease_valid, commit_index, read_index)
{
    lease_valid && commit_index >= read_index
}

// --- is_leader_for_lease ---

// r[depends lease-read.leader-only]
/// Spec: only leaders serve lease reads.
pub open spec fn is_leader_for_lease_spec(state: u8) -> bool {
    state == STATE_LEADER
}

// r[impl lease-read.leader-only]
/// Check if this server can serve lease reads.
pub fn is_leader_for_lease(state: u8) -> (result: bool)
    ensures result == is_leader_for_lease_spec(state)
{
    state == STATE_LEADER
}

// --- Safety proofs ---

// r[verify lease-read.is-valid]
/// Lease at current time with positive effective duration is valid.
proof fn fresh_lease_valid(now: u64, timeout: u64, drift: u64)
    requires timeout > drift, now as int + (timeout - drift) as int <= u64::MAX as int
    ensures is_lease_valid_spec(now, timeout, drift, now)
{}

// r[verify lease-read.is-valid]
/// Zero effective duration means lease is never valid.
proof fn zero_duration_invalid(start: u64, drift: u64, now: u64)
    requires drift >= 100, now >= start
    ensures !is_lease_valid_spec(start, 0, drift, now)
{}

// r[verify lease-read.is-valid]
/// Lease expires when now reaches the expiry point.
proof fn lease_expires(start: u64, timeout: u64, drift: u64)
    requires
        timeout > drift,
        start as int + (timeout - drift) as int <= u64::MAX as int,
    ensures !is_lease_valid_spec(start, timeout, drift,
        (start + (timeout - drift)) as u64)
{}

// r[impl lease-read.disjoint]
// r[verify lease-read.disjoint]
/// Two leases with sequential start times don't overlap when the
/// second starts after the first expires.
proof fn sequential_leases_disjoint(
    s1: u64, s2: u64, timeout: u64, drift: u64, now: u64,
)
    requires
        timeout > drift,
        s1 as int + (timeout - drift) as int <= u64::MAX as int,
        s2 >= (s1 + (timeout - drift)) as u64,
        s2 as int + (timeout - drift) as int <= u64::MAX as int,
        now >= s2,
    ensures
        !(is_lease_valid_spec(s1, timeout, drift, now)
          && is_lease_valid_spec(s2, timeout, drift, now))
{}

// r[verify lease-read.requires-commit]
/// Lease alone is insufficient — commit must be reached.
proof fn lease_alone_insufficient(read_index: u64, commit_index: u64)
    requires commit_index < read_index
    ensures !lease_read_safe_spec(true, commit_index, read_index)
{}

// r[verify lease-read.requires-commit]
/// Invalid lease blocks even committed reads.
proof fn invalid_lease_blocks(commit_index: u64, read_index: u64)
    ensures !lease_read_safe_spec(false, commit_index, read_index)
{}

// r[verify lease-read.requires-commit]
/// Both valid lease and committed is safe.
proof fn both_conditions_safe(commit_index: u64, read_index: u64)
    requires commit_index >= read_index
    ensures lease_read_safe_spec(true, commit_index, read_index)
{}

// r[verify lease-read.leader-only]
/// Followers cannot serve lease reads.
proof fn follower_no_lease_read()
    ensures !is_leader_for_lease_spec(STATE_FOLLOWER)
{}

// r[verify lease-read.leader-only]
/// Candidates cannot serve lease reads.
proof fn candidate_no_lease_read()
    ensures !is_leader_for_lease_spec(STATE_TRANSITIONING)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify lease-read.is-valid]
    #[test]
    fn test_is_lease_valid() {
        // timeout=150, drift=50, effective=100, expiry=200
        assert!(is_lease_valid(100, 150, 50, 100)); // at start
        assert!(is_lease_valid(100, 150, 50, 150)); // mid-lease
        assert!(is_lease_valid(100, 150, 50, 199)); // just before
        assert!(!is_lease_valid(100, 150, 50, 200)); // at expiry
        assert!(!is_lease_valid(100, 150, 50, 300)); // past expiry
    }

    // r[verify lease-read.is-valid]
    #[test]
    fn test_drift_exceeds_timeout() {
        // drift >= timeout means zero effective duration
        assert!(!is_lease_valid(100, 50, 100, 100));
        assert!(!is_lease_valid(100, 50, 200, 100));
    }

    // r[verify lease-read.start]
    #[test]
    fn test_start_lease() {
        assert_eq!(start_lease(42), 42);
    }

    // r[verify lease-read.requires-commit]
    #[test]
    fn test_lease_read_safe() {
        assert!(lease_read_safe(true, 10, 5)); // valid + committed
        assert!(!lease_read_safe(true, 3, 5)); // valid but not committed
        assert!(!lease_read_safe(false, 10, 5)); // invalid lease
        assert!(!lease_read_safe(false, 3, 5)); // both bad
    }

    // r[verify lease-read.leader-only]
    #[test]
    fn test_is_leader_for_lease() {
        assert!(is_leader_for_lease(STATE_LEADER));
        assert!(!is_leader_for_lease(STATE_FOLLOWER));
        assert!(!is_leader_for_lease(STATE_TRANSITIONING));
    }

    proptest! {
        // r[verify lease-read.is-valid]
        #[test]
        fn prop_fresh_lease_valid(
            now in 0u64..u64::MAX / 2,
            timeout in 10u64..1000,
            drift in 0u64..10,
        ) {
            if timeout > drift {
                prop_assert!(is_lease_valid(now, timeout, drift, now));
            }
        }

        // r[verify lease-read.requires-commit]
        #[test]
        fn prop_lease_needs_commit(commit: u64, read: u64) {
            if commit < read {
                prop_assert!(!lease_read_safe(true, commit, read));
            }
        }

        // r[verify lease-read.requires-commit]
        #[test]
        fn prop_invalid_lease_blocks(commit: u64, read: u64) {
            prop_assert!(!lease_read_safe(false, commit, read));
        }

        // r[verify lease-read.is-valid]
        #[test]
        fn prop_lease_expires_at_boundary(
            start in 0u64..u64::MAX / 4,
            timeout in 10u64..1000,
            drift in 0u64..10,
        ) {
            if timeout > drift {
                let expiry = start + (timeout - drift);
                prop_assert!(!is_lease_valid(start, timeout, drift, expiry));
            }
        }
    }
}
