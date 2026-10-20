//! Verified TTL and expiration arithmetic.
//!
//! Saturating time calculations for key expiration, lease management,
//! and TTL validation. Every function is overflow-safe — no silent
//! wraparound, no panics. Matches aspen's `calculate_expires_at_ms`,
//! `is_lease_expired`, `is_key_expired`, `time_remaining_exec`.

use vstd::prelude::*;

verus! {

// --- expires_at ---

// r[depends expiration.expires-at]
/// Spec: absolute expiration time from current time + TTL.
/// Saturates at u64::MAX on overflow.
pub open spec fn expires_at_spec(now_ms: u64, ttl_seconds: u64) -> u64 {
    let ttl_ms = if ttl_seconds as int * 1000 > u64::MAX as int {
        u64::MAX
    } else {
        (ttl_seconds * 1000) as u64
    };
    if now_ms as int + ttl_ms as int > u64::MAX as int {
        u64::MAX
    } else {
        (now_ms + ttl_ms) as u64
    }
}

// r[impl expiration.expires-at]
/// Computes the absolute expiration timestamp from `now_ms` + `ttl_seconds`.
///
/// Uses saturating arithmetic to prevent overflow. Matches aspen's
/// `calculate_expires_at_ms`.
pub fn expires_at(now_ms: u64, ttl_seconds: u64) -> (result: u64)
    ensures
        result == expires_at_spec(now_ms, ttl_seconds),
        result >= now_ms,
{
    // Saturating mul: ttl_seconds * 1000
    let ttl_ms = if ttl_seconds > u64::MAX / 1000 {
        u64::MAX
    } else {
        ttl_seconds * 1000
    };
    // Saturating add: now_ms + ttl_ms
    if now_ms > u64::MAX - ttl_ms {
        u64::MAX
    } else {
        now_ms + ttl_ms
    }
}

// r[verify expiration.expires-at]
/// Expiration is always >= now (monotonic forward).
proof fn expires_at_ge_now(now_ms: u64, ttl_seconds: u64)
    ensures expires_at_spec(now_ms, ttl_seconds) >= now_ms
{}

// r[verify expiration.expires-at]
/// Zero TTL means expiration equals now.
proof fn expires_at_zero_ttl(now_ms: u64)
    ensures expires_at_spec(now_ms, 0) == now_ms
{}

// r[verify expiration.expires-at]
/// Larger TTL means later (or equal) expiration.
proof fn expires_at_monotonic_ttl(now_ms: u64, ttl_a: u64, ttl_b: u64)
    requires ttl_a <= ttl_b
    ensures expires_at_spec(now_ms, ttl_a) <= expires_at_spec(now_ms, ttl_b)
{}

// --- is_expired ---

// r[depends expiration.is-expired]
/// Spec: entry has expired (now is strictly past the expiration time).
pub open spec fn is_expired_spec(expires_at_ms: u64, now_ms: u64) -> bool {
    now_ms > expires_at_ms
}

// r[impl expiration.is-expired]
/// Returns true if `now_ms` is strictly past `expires_at_ms`.
///
/// At the exact expiration time, the entry is NOT expired.
/// Matches aspen's `is_lease_expired`.
pub fn is_expired(expires_at_ms: u64, now_ms: u64) -> (result: bool)
    ensures result == is_expired_spec(expires_at_ms, now_ms)
{
    now_ms > expires_at_ms
}

// r[verify expiration.is-expired]
/// A freshly created expiration is never expired.
proof fn fresh_not_expired(now_ms: u64, ttl_seconds: u64)
    ensures !is_expired_spec(expires_at_spec(now_ms, ttl_seconds), now_ms)
{
    // expires_at >= now_ms, so now_ms <= expires_at, so !(now_ms > expires_at)
}

// r[verify expiration.is-expired]
/// Expiration is monotonic in time: if not expired at t1, not expired at
/// any t0 <= t1.
proof fn expired_monotonic(expires_at_ms: u64, t0: u64, t1: u64)
    requires t0 <= t1, !is_expired_spec(expires_at_ms, t1)
    ensures !is_expired_spec(expires_at_ms, t0)
{}

// --- is_expired_opt ---

// r[depends expiration.is-expired-opt]
/// Spec: optional expiration check. None means never expires.
pub open spec fn is_expired_opt_spec(
    expires_at_ms: Option<u64>, now_ms: u64,
) -> bool {
    match expires_at_ms {
        Some(ea) => now_ms >= ea,
        None => false,
    }
}

// r[impl expiration.is-expired-opt]
/// Returns true if the key has expired. None means no TTL (never expires).
///
/// Note: uses `>=` at boundary (matching aspen's `is_key_expired`), while
/// `is_expired` uses `>` (matching aspen's `is_lease_expired`). The
/// difference: keys expire AT the deadline, leases expire AFTER it.
pub fn is_expired_opt(
    expires_at_ms: Option<u64>, now_ms: u64,
) -> (result: bool)
    ensures result == is_expired_opt_spec(expires_at_ms, now_ms)
{
    match expires_at_ms {
        Some(ea) => now_ms >= ea,
        None => false,
    }
}

// r[verify expiration.is-expired-opt]
/// No TTL means never expired.
proof fn no_ttl_never_expires(now_ms: u64)
    ensures !is_expired_opt_spec(None, now_ms)
{}

// r[verify expiration.is-expired-opt]
/// is_expired_opt(None, t) is always false regardless of time.
proof fn none_stable(t1: u64, t2: u64)
    ensures
        is_expired_opt_spec(None, t1) == is_expired_opt_spec(None, t2),
        !is_expired_opt_spec(None, t1),
{}

// --- time_remaining ---

// r[depends expiration.time-remaining]
/// Spec: time remaining until expiration.
pub open spec fn time_remaining_spec(
    expires_at_ms: Option<u64>, now_ms: u64,
) -> Option<u64> {
    match expires_at_ms {
        None => None,
        Some(ea) => {
            if now_ms >= ea {
                Some(0u64)
            } else {
                Some((ea - now_ms) as u64)
            }
        }
    }
}

// r[impl expiration.time-remaining]
/// Returns the time remaining until expiration.
///
/// - None: no TTL set
/// - Some(0): already expired
/// - Some(ms): milliseconds until expiration
///
/// Matches aspen's `time_remaining_exec`.
pub fn time_remaining(
    expires_at_ms: Option<u64>, now_ms: u64,
) -> (result: Option<u64>)
    ensures result == time_remaining_spec(expires_at_ms, now_ms)
{
    match expires_at_ms {
        None => None,
        Some(ea) => {
            if now_ms >= ea {
                Some(0)
            } else {
                Some(ea - now_ms)
            }
        }
    }
}

// r[verify expiration.time-remaining]
/// No TTL means no remaining time (None, not Some(0)).
proof fn time_remaining_no_ttl(now_ms: u64)
    ensures time_remaining_spec(None, now_ms).is_none()
{}

// r[verify expiration.time-remaining]
/// Remaining time is zero when expired.
proof fn time_remaining_expired(ea: u64, now_ms: u64)
    requires now_ms >= ea
    ensures time_remaining_spec(Some(ea), now_ms) == Some(0u64)
{}

// r[verify expiration.time-remaining]
/// Remaining time decreases as now advances.
proof fn time_remaining_decreases(ea: u64, t0: u64, t1: u64)
    requires t0 <= t1, t1 < ea
    ensures
        time_remaining_spec(Some(ea), t1).unwrap()
            <= time_remaining_spec(Some(ea), t0).unwrap()
{}

// --- is_ttl_valid ---

// r[depends expiration.is-ttl-valid]
/// Spec: TTL is within allowed bounds.
pub open spec fn is_ttl_valid_spec(ttl_seconds: u64, max_ttl: u64) -> bool {
    ttl_seconds <= max_ttl
}

// r[impl expiration.is-ttl-valid]
/// Returns true if `ttl_seconds` is within `max_ttl`.
///
/// Matches aspen's `is_ttl_valid` (aspen hardcodes 30 days).
pub fn is_ttl_valid(ttl_seconds: u64, max_ttl: u64) -> (result: bool)
    ensures result == is_ttl_valid_spec(ttl_seconds, max_ttl)
{
    ttl_seconds <= max_ttl
}

// r[verify expiration.is-ttl-valid]
/// Zero TTL is always valid.
proof fn zero_ttl_valid(max_ttl: u64)
    ensures is_ttl_valid_spec(0, max_ttl)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify expiration.expires-at]
    #[test]
    fn test_expires_at() {
        assert_eq!(expires_at(1_000_000, 3600), 1_000_000 + 3_600_000);
        assert_eq!(expires_at(0, 0), 0);
        assert_eq!(expires_at(u64::MAX, 1), u64::MAX); // saturates
        assert_eq!(expires_at(u64::MAX - 999, 1), u64::MAX); // saturates
        assert_eq!(expires_at(u64::MAX - 1000, 1), u64::MAX); // exact edge
    }

    // r[verify expiration.is-expired]
    #[test]
    fn test_is_expired() {
        assert!(is_expired(1000, 2000)); // past
        assert!(!is_expired(2000, 1000)); // before
        assert!(!is_expired(1000, 1000)); // at boundary — NOT expired
    }

    // r[verify expiration.is-expired-opt]
    #[test]
    fn test_is_expired_opt() {
        assert!(is_expired_opt(Some(1000), 2000)); // expired
        assert!(is_expired_opt(Some(1000), 1000)); // at boundary — expired (>=)
        assert!(!is_expired_opt(Some(2000), 1000)); // not expired
        assert!(!is_expired_opt(None, u64::MAX)); // no TTL — never
    }

    // r[verify expiration.time-remaining]
    #[test]
    fn test_time_remaining() {
        assert_eq!(time_remaining(Some(2000), 1000), Some(1000));
        assert_eq!(time_remaining(Some(1000), 2000), Some(0));
        assert_eq!(time_remaining(Some(1000), 1000), Some(0));
        assert_eq!(time_remaining(None, 1000), None);
    }

    // r[verify expiration.is-ttl-valid]
    #[test]
    fn test_is_ttl_valid() {
        let max_30d = 30 * 24 * 60 * 60;
        assert!(is_ttl_valid(3600, max_30d));
        assert!(is_ttl_valid(max_30d, max_30d));
        assert!(!is_ttl_valid(max_30d + 1, max_30d));
        assert!(is_ttl_valid(0, max_30d));
    }

    proptest! {
        // r[verify expiration.expires-at]
        #[test]
        fn prop_expires_at_ge_now(now_ms: u64, ttl: u64) {
            prop_assert!(expires_at(now_ms, ttl) >= now_ms);
        }

        // r[verify expiration.expires-at]
        #[test]
        fn prop_expires_at_monotonic_ttl(now_ms: u64, a: u64, b: u64) {
            if a <= b {
                prop_assert!(expires_at(now_ms, a) <= expires_at(now_ms, b));
            }
        }

        // r[verify expiration.is-expired]
        #[test]
        fn prop_fresh_not_expired(now_ms: u64, ttl: u64) {
            let ea = expires_at(now_ms, ttl);
            prop_assert!(!is_expired(ea, now_ms));
        }

        // r[verify expiration.is-expired]
        // r[verify expiration.is-expired-opt]
        #[test]
        fn prop_none_never_expires(now_ms: u64) {
            prop_assert!(!is_expired_opt(None, now_ms));
        }

        // r[verify expiration.time-remaining]
        #[test]
        fn prop_time_remaining_consistency(ea: u64, now_ms: u64) {
            match time_remaining(Some(ea), now_ms) {
                Some(0) => prop_assert!(now_ms >= ea),
                Some(r) => {
                    prop_assert!(now_ms < ea);
                    prop_assert_eq!(r, ea - now_ms);
                }
                None => prop_assert!(false), // Some input always gives Some output
            }
        }

        // r[verify expiration.time-remaining]
        #[test]
        fn prop_time_remaining_decreases(ea in 100u64..=u64::MAX, delta in 0u64..=99) {
            let t0 = 0u64;
            let t1 = delta;
            if t1 < ea {
                let r0 = time_remaining(Some(ea), t0).unwrap();
                let r1 = time_remaining(Some(ea), t1).unwrap();
                prop_assert!(r1 <= r0);
            }
        }

        // r[verify expiration.expires-at]
        // r[verify expiration.is-expired]
        // r[verify expiration.time-remaining]
        /// Model oracle: create an expiration, advance time, check consistency
        /// of is_expired and time_remaining at each step.
        #[test]
        fn prop_expiration_lifecycle(
            now_ms in 0u64..=u64::MAX / 2,
            ttl in 0u64..=1_000_000,
            advances in proptest::collection::vec(0u64..=2_000_000, 0..=20),
        ) {
            let ea = expires_at(now_ms, ttl);
            prop_assert!(ea >= now_ms);

            let mut t = now_ms;
            for &delta in &advances {
                t = t.saturating_add(delta);

                // is_expired and time_remaining must agree
                let expired = is_expired(ea, t);
                let remaining = time_remaining(Some(ea), t);

                if expired {
                    // t > ea, so remaining is 0
                    prop_assert_eq!(remaining, Some(0));
                } else {
                    // t <= ea, remaining = ea - t
                    prop_assert_eq!(remaining, Some(ea - t));
                }
            }
        }
    }
}
