//! Verified Raft timeout configuration validators.
//!
//! Validates the three ordering constraints for Raft liveness:
//! 1. Heartbeat interval must be positive
//! 2. Election timeout range must have min > 0 and max > min
//! 3. Heartbeat must be strictly less than election min
//!
//! `validate_all` composes them with fail-fast semantics. If it passes,
//! every individual constraint holds.

use vstd::prelude::*;

verus! {

// --- validate_heartbeat ---

// r[depends raft-timing.validate-heartbeat]
/// Spec: heartbeat must be positive.
pub open spec fn validate_heartbeat_spec(heartbeat_ms: u64) -> bool {
    heartbeat_ms > 0
}

// r[impl raft-timing.validate-heartbeat]
/// Validate that the heartbeat interval is positive.
pub fn validate_heartbeat(heartbeat_ms: u64) -> (result: bool)
    ensures result == validate_heartbeat_spec(heartbeat_ms)
{
    heartbeat_ms > 0
}

// --- validate_election_range ---

// r[depends raft-timing.validate-election-range]
/// Spec: election min > 0 and max > min (range must allow randomization).
pub open spec fn validate_election_range_spec(election_min: u64, election_max: u64) -> bool {
    election_min > 0 && election_max > election_min
}

// r[impl raft-timing.validate-election-range]
/// Validate the election timeout range.
pub fn validate_election_range(election_min: u64, election_max: u64) -> (result: bool)
    ensures result == validate_election_range_spec(election_min, election_max)
{
    election_min > 0 && election_max > election_min
}

// --- validate_timing_order ---

// r[depends raft-timing.validate-order]
/// Spec: heartbeat must be strictly less than election min.
pub open spec fn validate_timing_order_spec(heartbeat_ms: u64, election_min: u64) -> bool {
    heartbeat_ms < election_min
}

// r[impl raft-timing.validate-order]
/// Validate heartbeat < election_min ordering.
pub fn validate_timing_order(heartbeat_ms: u64, election_min: u64) -> (result: bool)
    ensures result == validate_timing_order_spec(heartbeat_ms, election_min)
{
    heartbeat_ms < election_min
}

// --- validate_all ---

// r[depends raft-timing.validate-all]
/// Spec: all three constraints hold.
pub open spec fn validate_all_spec(
    heartbeat_ms: u64,
    election_min: u64,
    election_max: u64,
) -> bool {
    validate_heartbeat_spec(heartbeat_ms)
        && validate_election_range_spec(election_min, election_max)
        && validate_timing_order_spec(heartbeat_ms, election_min)
}

// r[impl raft-timing.validate-all]
/// Validate all Raft timing constraints (fail-fast).
pub fn validate_all(
    heartbeat_ms: u64,
    election_min: u64,
    election_max: u64,
) -> (result: bool)
    ensures result == validate_all_spec(heartbeat_ms, election_min, election_max)
{
    validate_heartbeat(heartbeat_ms)
        && validate_election_range(election_min, election_max)
        && validate_timing_order(heartbeat_ms, election_min)
}

// --- Safety proofs ---

// r[impl raft-timing.validate-all]
// r[verify raft-timing.validate-all]
/// If validate_all passes, each individual constraint holds.
proof fn all_implies_each(heartbeat_ms: u64, election_min: u64, election_max: u64)
    requires validate_all_spec(heartbeat_ms, election_min, election_max)
    ensures
        validate_heartbeat_spec(heartbeat_ms),
        validate_election_range_spec(election_min, election_max),
        validate_timing_order_spec(heartbeat_ms, election_min),
{}

// r[verify raft-timing.validate-election-range]
/// Ordering failure: max <= min means range invalid.
proof fn ordering_failure(election_min: u64, election_max: u64)
    requires election_max <= election_min
    ensures !validate_election_range_spec(election_min, election_max)
{}

// r[verify raft-timing.validate-order]
/// When heartbeat >= election_min, timing order fails.
proof fn order_violation(heartbeat_ms: u64, election_min: u64)
    requires heartbeat_ms >= election_min
    ensures !validate_timing_order_spec(heartbeat_ms, election_min)
{}

// r[verify raft-timing.validate-all]
/// Concrete valid configuration: heartbeat=50, min=150, max=300.
proof fn typical_config_valid()
    ensures validate_all_spec(50, 150, 300)
{}

// r[verify raft-timing.validate-all]
/// The constraints enforce strict ordering: heartbeat < election_min < election_max.
proof fn strict_ordering(heartbeat_ms: u64, election_min: u64, election_max: u64)
    requires validate_all_spec(heartbeat_ms, election_min, election_max)
    ensures heartbeat_ms < election_min, election_min < election_max
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify raft-timing.validate-all]
    #[test]
    fn test_valid_config() {
        assert!(validate_all(50, 150, 300));
        assert!(validate_all(1, 2, 3));
    }

    // r[verify raft-timing.validate-heartbeat]
    #[test]
    fn test_zero_heartbeat() {
        assert!(!validate_heartbeat(0));
        assert!(validate_heartbeat(1));
    }

    // r[verify raft-timing.validate-election-range]
    #[test]
    fn test_bad_range() {
        assert!(!validate_election_range(0, 100));
        assert!(!validate_election_range(100, 100));
        assert!(!validate_election_range(100, 50));
    }

    // r[verify raft-timing.validate-order]
    #[test]
    fn test_bad_order() {
        assert!(!validate_timing_order(150, 150));
        assert!(!validate_timing_order(200, 150));
        assert!(validate_timing_order(50, 150));
    }

    proptest! {
        // r[verify raft-timing.validate-all]
        #[test]
        fn prop_all_implies_each(
            hb in 1u64..100,
            emin in 101u64..500,
            emax in 501u64..1000,
        ) {
            prop_assert!(validate_all(hb, emin, emax));
            prop_assert!(validate_heartbeat(hb));
            prop_assert!(validate_election_range(emin, emax));
            prop_assert!(validate_timing_order(hb, emin));
        }

        // r[verify raft-timing.validate-all]
        #[test]
        fn prop_strict_ordering(
            hb in 1u64..100,
            emin in 101u64..500,
            emax in 501u64..1000,
        ) {
            if validate_all(hb, emin, emax) {
                prop_assert!(hb < emin);
                prop_assert!(emin < emax);
            }
        }

        // r[verify raft-timing.validate-heartbeat]
        #[test]
        fn prop_zero_heartbeat_fails(emin in 1u64..1000, emax in 1001u64..2000) {
            prop_assert!(!validate_all(0, emin, emax));
        }
    }
}
