//! Verified resource quota enforcement.
//!
//! Functions for tracking usage against a fixed limit — remaining capacity,
//! admission checks, bidirectional consume/release, utilization percentage,
//! and exhaustion detection. Conservation proofs guarantee roundtrip
//! correctness: consuming then releasing the same amount recovers the
//! original usage.
//!
//! Distinct from `watermark` (forward-only bump allocation), `balance`
//! (no limit concept), `rate_limit` (time-based refill), and `semaphore`
//! (concurrency permits with acquire/release semantics).

use vstd::prelude::*;

verus! {

// --- quota_remaining ---

// r[depends quota.remaining]
/// Spec: unused capacity, saturating at 0.
pub open spec fn quota_remaining_spec(limit: u64, used: u64) -> u64 {
    if used >= limit { 0 } else { (limit - used) as u64 }
}

// r[impl quota.remaining]
/// Returns the remaining quota capacity.
pub fn quota_remaining(limit: u64, used: u64) -> (result: u64)
    ensures result == quota_remaining_spec(limit, used)
{
    if used >= limit { 0 } else { limit - used }
}

// r[verify quota.remaining]
/// Full usage means zero remaining.
proof fn remaining_zero_when_full(limit: u64)
    ensures quota_remaining_spec(limit, limit) == 0
{}

// r[verify quota.remaining]
/// Zero usage means full remaining.
proof fn remaining_full_when_empty(limit: u64)
    ensures quota_remaining_spec(limit, 0) == limit
{}

// --- quota_allows ---

// r[depends quota.allows]
/// Spec: request fits within remaining capacity.
pub open spec fn quota_allows_spec(limit: u64, used: u64, request: u64) -> bool {
    used as int + request as int <= limit as int
}

// r[impl quota.allows]
/// Returns true if the request can be served within the quota.
pub fn quota_allows(limit: u64, used: u64, request: u64) -> (result: bool)
    ensures result == quota_allows_spec(limit, used, request)
{
    // Overflow-safe: rearrange to avoid used + request overflow
    if used > limit {
        false
    } else {
        request <= limit - used
    }
}

// r[verify quota.allows]
/// Zero request is always allowed.
proof fn allows_zero_always(limit: u64, used: u64)
    requires used <= limit
    ensures quota_allows_spec(limit, used, 0)
{}

// r[verify quota.allows]
/// Request larger than limit is never allowed.
proof fn rejects_over_limit(limit: u64, used: u64, request: u64)
    requires request as int > limit as int
    ensures !quota_allows_spec(limit, used, request)
{}

// --- quota_consume ---

// r[depends quota.consume]
/// Spec: increase usage by amount.
pub open spec fn quota_consume_spec(used: u64, amount: u64) -> u64 {
    (used as int + amount as int) as u64
}

// r[impl quota.consume]
/// Returns usage after consuming amount.
pub fn quota_consume(used: u64, amount: u64) -> (result: u64)
    requires used as int + amount as int <= u64::MAX as int
    ensures result == quota_consume_spec(used, amount)
{
    used + amount
}

// r[verify quota.consume]
/// Consuming zero is identity.
proof fn consume_zero_identity(used: u64)
    ensures quota_consume_spec(used, 0) == used
{}

// r[verify quota.consume]
/// Consuming a positive amount strictly increases usage.
proof fn consume_increases(used: u64, amount: u64)
    requires amount > 0, used as int + amount as int <= u64::MAX as int
    ensures quota_consume_spec(used, amount) > used
{}

// --- quota_release ---

// r[depends quota.release]
/// Spec: decrease usage by amount, saturating at 0.
pub open spec fn quota_release_spec(used: u64, amount: u64) -> u64 {
    if amount >= used { 0 } else { (used - amount) as u64 }
}

// r[impl quota.release]
/// Returns usage after releasing amount (saturating subtraction).
pub fn quota_release(used: u64, amount: u64) -> (result: u64)
    ensures result == quota_release_spec(used, amount)
{
    if amount >= used { 0 } else { used - amount }
}

// r[verify quota.release]
/// Releasing zero is identity.
proof fn release_zero_identity(used: u64)
    ensures quota_release_spec(used, 0) == used
{}

// r[verify quota.release]
/// Release never increases usage.
proof fn release_bounded(used: u64, amount: u64)
    ensures (quota_release_spec(used, amount) as int) <= (used as int)
{}

// --- quota_utilization ---

// r[depends quota.utilization]
/// Spec: integer percentage of quota consumed, clamped to [0, 100].
pub open spec fn quota_utilization_spec(limit: u64, used: u64) -> u64 {
    if limit == 0 {
        if used > 0 { 100 } else { 0 }
    } else if used >= limit {
        100
    } else {
        (used as int * 100 / limit as int) as u64
    }
}

// r[impl quota.utilization]
/// Returns the utilization percentage (0–100).
#[allow(clippy::cast_possible_truncation)]
pub fn quota_utilization(limit: u64, used: u64) -> (result: u64)
    ensures result == quota_utilization_spec(limit, used)
{
    if limit == 0 {
        if used > 0 { 100 } else { 0 }
    } else if used >= limit {
        100
    } else {
        // used < limit, so used * 100 may overflow u64 — use u128 intermediate
        (used as u128 * 100 / limit as u128) as u64
    }
}

// r[verify quota.utilization]
/// Utilization is always at most 100.
proof fn utilization_bounded(limit: u64, used: u64)
    ensures quota_utilization_spec(limit, used) <= 100
{
    if limit > 0 && used < limit {
        assert((used as int) * 100 / (limit as int) <= 99) by(nonlinear_arith)
            requires (used as int) >= 0, (limit as int) > 0, (used as int) < (limit as int);
    }
}

// r[verify quota.utilization]
/// Zero usage means zero utilization (when limit > 0).
proof fn utilization_zero_when_unused(limit: u64)
    requires limit > 0
    ensures quota_utilization_spec(limit, 0) == 0
{}

// r[verify quota.utilization]
/// Exact full usage means 100% utilization.
proof fn utilization_full(limit: u64)
    requires limit > 0
    ensures quota_utilization_spec(limit, limit) == 100
{}

// --- quota_is_exhausted ---

// r[depends quota.is-exhausted]
/// Spec: no remaining capacity.
pub open spec fn quota_is_exhausted_spec(limit: u64, used: u64) -> bool {
    used >= limit
}

// r[impl quota.is-exhausted]
/// Returns true if the quota is fully consumed.
pub fn quota_is_exhausted(limit: u64, used: u64) -> (result: bool)
    ensures result == quota_is_exhausted_spec(limit, used)
{
    used >= limit
}

// r[verify quota.is-exhausted]
/// Exhaustion implies zero remaining.
proof fn exhausted_means_zero_remaining(limit: u64, used: u64)
    requires quota_is_exhausted_spec(limit, used)
    ensures quota_remaining_spec(limit, used) == 0
{}

// r[verify quota.is-exhausted]
/// Exhaustion blocks all positive requests.
proof fn exhausted_no_allows(limit: u64, used: u64, request: u64)
    requires quota_is_exhausted_spec(limit, used), request > 0
    ensures !quota_allows_spec(limit, used, request)
{}

// --- Roundtrip proofs ---

// r[verify quota.consume]
// r[verify quota.release]
/// Consuming then releasing the same amount recovers original usage.
proof fn consume_release_roundtrip(used: u64, amount: u64)
    requires used as int + amount as int <= u64::MAX as int
    ensures quota_release_spec(quota_consume_spec(used, amount), amount) == used
{}

// r[verify quota.release]
// r[verify quota.consume]
/// Releasing then consuming the same amount recovers original usage
/// (when used >= amount so release doesn't saturate).
proof fn release_consume_roundtrip(used: u64, amount: u64)
    requires amount <= used
    ensures quota_consume_spec(quota_release_spec(used, amount), amount) == used
{}

// --- Cross-function proofs ---

// r[verify quota.remaining]
// r[verify quota.allows]
/// Remaining capacity is at least as large as any allowed request.
proof fn allows_implies_fits_in_remaining(limit: u64, used: u64, request: u64)
    requires used <= limit, quota_allows_spec(limit, used, request)
    ensures request <= quota_remaining_spec(limit, used)
{}

// r[verify quota.remaining]
// r[verify quota.is-exhausted]
/// Zero remaining with positive limit means exhausted.
proof fn zero_remaining_means_exhausted(limit: u64, used: u64)
    requires limit > 0, quota_remaining_spec(limit, used) == 0
    ensures quota_is_exhausted_spec(limit, used)
{}

// r[verify quota.remaining]
/// Remaining plus used equals the limit (when used <= limit).
proof fn remaining_plus_used(limit: u64, used: u64)
    requires used <= limit
    ensures (quota_remaining_spec(limit, used) as int) + (used as int) == (limit as int)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify quota.remaining]
    #[test]
    fn test_quota_remaining() {
        assert_eq!(quota_remaining(100, 0), 100);
        assert_eq!(quota_remaining(100, 30), 70);
        assert_eq!(quota_remaining(100, 100), 0);
        assert_eq!(quota_remaining(100, 150), 0);
        assert_eq!(quota_remaining(0, 0), 0);
    }

    // r[verify quota.allows]
    #[test]
    fn test_quota_allows() {
        assert!(quota_allows(100, 0, 50));
        assert!(quota_allows(100, 50, 50));
        assert!(!quota_allows(100, 50, 51));
        assert!(quota_allows(100, 100, 0));
        assert!(!quota_allows(100, 100, 1));
        assert!(!quota_allows(0, 0, 1));
        assert!(quota_allows(0, 0, 0));
    }

    // r[verify quota.consume]
    #[test]
    fn test_quota_consume() {
        assert_eq!(quota_consume(0, 10), 10);
        assert_eq!(quota_consume(50, 30), 80);
        assert_eq!(quota_consume(100, 0), 100);
    }

    // r[verify quota.release]
    #[test]
    fn test_quota_release() {
        assert_eq!(quota_release(50, 10), 40);
        assert_eq!(quota_release(50, 50), 0);
        assert_eq!(quota_release(50, 60), 0);
        assert_eq!(quota_release(0, 0), 0);
        assert_eq!(quota_release(0, 5), 0);
    }

    // r[verify quota.utilization]
    #[test]
    fn test_quota_utilization() {
        assert_eq!(quota_utilization(100, 0), 0);
        assert_eq!(quota_utilization(100, 50), 50);
        assert_eq!(quota_utilization(100, 100), 100);
        assert_eq!(quota_utilization(100, 200), 100);
        assert_eq!(quota_utilization(0, 0), 0);
        assert_eq!(quota_utilization(0, 5), 100);
        assert_eq!(quota_utilization(3, 1), 33);
    }

    // r[verify quota.is-exhausted]
    #[test]
    fn test_quota_is_exhausted() {
        assert!(!quota_is_exhausted(100, 0));
        assert!(!quota_is_exhausted(100, 99));
        assert!(quota_is_exhausted(100, 100));
        assert!(quota_is_exhausted(100, 200));
        assert!(quota_is_exhausted(0, 0));
    }

    proptest! {
        // r[verify quota.remaining]
        #[test]
        fn prop_remaining_plus_used(limit: u64, used in 0u64..=u64::MAX) {
            if used <= limit {
                let rem = quota_remaining(limit, used);
                prop_assert_eq!(rem + used, limit);
            }
        }

        // r[verify quota.consume]
        // r[verify quota.release]
        #[test]
        fn prop_consume_release_roundtrip(used in 0u64..=u64::MAX / 2, amount in 0u64..=u64::MAX / 2) {
            let after_consume = quota_consume(used, amount);
            let after_release = quota_release(after_consume, amount);
            prop_assert_eq!(after_release, used);
        }

        // r[verify quota.release]
        // r[verify quota.consume]
        #[test]
        fn prop_release_consume_roundtrip(used: u64, amount: u64) {
            if amount <= used {
                let after_release = quota_release(used, amount);
                if after_release as u128 + amount as u128 <= u64::MAX as u128 {
                    let after_consume = quota_consume(after_release, amount);
                    prop_assert_eq!(after_consume, used);
                }
            }
        }

        // r[verify quota.is-exhausted]
        #[test]
        fn prop_exhausted_no_allows(limit: u64, used: u64, request in 1u64..=u64::MAX) {
            if quota_is_exhausted(limit, used) {
                prop_assert!(!quota_allows(limit, used, request));
            }
        }

        // r[verify quota.release]
        #[test]
        fn prop_release_bounded(used: u64, amount: u64) {
            prop_assert!(quota_release(used, amount) <= used);
        }

        // r[verify quota.utilization]
        #[test]
        fn prop_utilization_bounded(limit: u64, used: u64) {
            prop_assert!(quota_utilization(limit, used) <= 100);
        }

        // r[verify quota.allows]
        #[test]
        fn prop_allows_zero(limit: u64, used: u64) {
            if used <= limit {
                prop_assert!(quota_allows(limit, used, 0));
            }
        }

        // r[verify quota.remaining]
        // r[verify quota.allows]
        #[test]
        fn prop_allows_fits_remaining(limit: u64, used: u64, request: u64) {
            if used <= limit && quota_allows(limit, used, request) {
                prop_assert!(request <= quota_remaining(limit, used));
            }
        }

        // r[verify quota.consume]
        #[test]
        fn prop_consume_monotonic(used in 0u64..=u64::MAX / 2, amount in 1u64..=u64::MAX / 2) {
            prop_assert!(quota_consume(used, amount) > used);
        }

        // r[verify quota.is-exhausted]
        // r[verify quota.remaining]
        #[test]
        fn prop_exhausted_iff_zero_remaining(limit: u64, used: u64) {
            if limit > 0 {
                prop_assert_eq!(
                    quota_is_exhausted(limit, used),
                    quota_remaining(limit, used) == 0
                );
            }
        }
    }
}
