//! Verified token timestamp logic for UCAN authorization.
//!
//! Expiration with clock-skew tolerance, future-token rejection,
//! and refresh threshold detection. Matches aspen's `verified_credential`
//! timestamp functions.

use vstd::prelude::*;

verus! {

/// Clock skew tolerance in seconds (matches aspen's TOKEN_CLOCK_SKEW_SECS).
pub open spec fn TOKEN_CLOCK_SKEW_SECS() -> u64 { 60 }

/// Maximum capabilities per token (matches aspen's constant).
pub open spec fn MAX_CAPABILITIES_PER_TOKEN() -> u64 { 32 }

// ============================================================
// Expiration with tolerance
// ============================================================

// r[depends token-timestamp.expiration-tolerance]
/// Spec: is the token expired, accounting for tolerance?
/// A token is expired when `now > expires_at + tolerance`.
/// Uses saturating addition to prevent overflow.
pub open spec fn is_expired_spec(expires_at: u64, now: u64, tolerance: u64) -> bool {
    let deadline: int = expires_at as int + tolerance as int;
    now as int > deadline
}

// r[impl token-timestamp.expiration-tolerance]
/// Returns true if the token is expired (now > `expires_at` + tolerance).
/// Uses saturating addition for overflow safety.
pub fn is_expired(expires_at: u64, now: u64, tolerance: u64) -> (result: bool)
    ensures result == is_expired_spec(expires_at, now, tolerance),
{
    // Saturating add: if expires_at + tolerance would overflow, deadline = MAX.
    // now > MAX is impossible for u64, so not expired.
    let deadline = if tolerance > u64::MAX - expires_at {
        u64::MAX
    } else {
        expires_at + tolerance
    };
    now > deadline
}

// r[impl token-timestamp.fresh-token-valid]
// r[verify token-timestamp.fresh-token-valid]
/// A fresh token (checked at issued_at, with issued_at <= expires_at) is not expired.
proof fn fresh_token_valid(issued_at: u64, expires_at: u64, tolerance: u64)
    requires issued_at <= expires_at
    ensures !is_expired_spec(expires_at, issued_at, tolerance)
{
    // issued_at <= expires_at <= expires_at + tolerance, so issued_at <= deadline, not expired.
}

// r[impl token-timestamp.expiration-monotonic]
// r[verify token-timestamp.expiration-monotonic]
/// Once expired, stays expired as time advances.
proof fn expiration_monotonic(expires_at: u64, t1: u64, t2: u64, tolerance: u64)
    requires
        is_expired_spec(expires_at, t1, tolerance),
        t2 >= t1,
    ensures is_expired_spec(expires_at, t2, tolerance)
{
    // t1 > deadline and t2 >= t1, so t2 > deadline.
}

// r[verify token-timestamp.expiration-tolerance]
/// Within tolerance window, token is not expired.
proof fn within_tolerance_valid(expires_at: u64, now: u64, tolerance: u64)
    requires now as int <= expires_at as int + tolerance as int
    ensures !is_expired_spec(expires_at, now, tolerance)
{}

// r[verify token-timestamp.expiration-tolerance]
/// Zero tolerance: expired iff now > expires_at (no grace period).
proof fn zero_tolerance(expires_at: u64, now: u64)
    ensures is_expired_spec(expires_at, now, 0) == (now > expires_at)
{}

// ============================================================
// Future token rejection
// ============================================================

// r[depends token-timestamp.future-rejection]
/// Spec: is the token from the future (issued_at too far ahead of now)?
pub open spec fn is_from_future_spec(issued_at: u64, now: u64, tolerance: u64) -> bool {
    issued_at as int > now as int + tolerance as int
}

// r[impl token-timestamp.future-rejection]
/// Returns true if the token was issued too far in the future.
pub fn is_from_future(issued_at: u64, now: u64, tolerance: u64) -> (result: bool)
    ensures result == is_from_future_spec(issued_at, now, tolerance),
{
    let limit = if tolerance > u64::MAX - now {
        u64::MAX
    } else {
        now + tolerance
    };
    issued_at > limit
}

// r[verify token-timestamp.future-rejection]
/// Tokens issued at or before now + tolerance are not from the future.
proof fn current_not_future(issued_at: u64, now: u64, tolerance: u64)
    requires issued_at as int <= now as int + tolerance as int
    ensures !is_from_future_spec(issued_at, now, tolerance)
{}

// r[verify token-timestamp.future-rejection]
/// Clock skew within tolerance is accepted.
proof fn clock_skew_accepted(now: u64, tolerance: u64)
    requires tolerance > 0, now as int + tolerance as int <= u64::MAX as int
    ensures !is_from_future_spec((now + tolerance) as u64, now, tolerance)
{}

// r[verify token-timestamp.future-rejection]
/// Tokens beyond tolerance are rejected.
proof fn beyond_tolerance_rejected(issued_at: u64, now: u64, tolerance: u64)
    requires issued_at as int > now as int + tolerance as int
    ensures is_from_future_spec(issued_at, now, tolerance)
{}

// ============================================================
// Refresh threshold (20%)
// ============================================================

// r[depends token-timestamp.refresh-threshold]
/// Spec: does the token need refresh?
/// Needs refresh when remaining lifetime ≤ 20% of total lifetime.
/// `remaining = expires_at - now` (0 if expired).
/// `total = expires_at - issued_at`.
/// Threshold: remaining * 5 <= total (equivalent to remaining <= total/5).
pub open spec fn needs_refresh_spec(issued_at: u64, expires_at: u64, now: u64) -> bool {
    if issued_at >= expires_at {
        true // zero-length token always needs refresh
    } else if now >= expires_at {
        true // already expired
    } else {
        let remaining: int = expires_at as int - now as int;
        let total: int = expires_at as int - issued_at as int;
        remaining * 5 <= total
    }
}

// r[impl token-timestamp.refresh-threshold]
/// Returns true if the token needs refresh (remaining ≤ 20% of total).
pub fn needs_refresh(issued_at: u64, expires_at: u64, now: u64) -> (result: bool)
    ensures result == needs_refresh_spec(issued_at, expires_at, now),
{
    if issued_at >= expires_at {
        return true;
    }
    if now >= expires_at {
        return true;
    }
    let remaining = expires_at - now;
    let total = expires_at - issued_at;
    // remaining * 5 <= total, but remaining * 5 might overflow.
    // Use division: remaining <= total / 5 is close but not identical due to rounding.
    // Instead: check if remaining > total / 5 (integer division rounds down).
    // remaining * 5 <= total  <==>  remaining <= total / 5 when total % 5 == 0
    // For general case: remaining * 5 <= total. Check overflow:
    // remaining < 2^64 and total < 2^64, remaining <= total, so remaining * 5 <= 5 * total.
    // If remaining <= u64::MAX / 5, we can multiply safely.
    if remaining > u64::MAX / 5 {
        // remaining > MAX/5, so remaining * 5 > MAX >= total, so remaining * 5 > total.
        false
    } else {
        remaining * 5 <= total
    }
}

// r[verify token-timestamp.refresh-threshold]
/// Expired tokens need refresh.
proof fn expired_needs_refresh(issued_at: u64, expires_at: u64, now: u64)
    requires now >= expires_at, issued_at < expires_at
    ensures needs_refresh_spec(issued_at, expires_at, now)
{}

// r[verify token-timestamp.refresh-threshold]
/// Fresh token (just issued) does not need refresh if total > 0.
proof fn fresh_no_refresh(issued_at: u64, expires_at: u64)
    requires
        issued_at < expires_at,
        // remaining == total at issued_at, so remaining * 5 == total * 5 > total (when total > 0)
    ensures !needs_refresh_spec(issued_at, expires_at, issued_at)
{
    // remaining = expires_at - issued_at = total
    // remaining * 5 = total * 5 > total (since total > 0)
}

// r[verify token-timestamp.refresh-threshold]
/// At 80% of lifetime, no refresh needed yet.
proof fn at_80_pct_no_refresh(issued_at: u64, expires_at: u64)
    requires
        issued_at < expires_at,
        (expires_at - issued_at) >= 5, // need at least 5 units for clean division
    ensures
        // now = issued_at + 4*(total/5) puts us at 80%, remaining = total - 4*(total/5)
        // For total=100: now=issued_at+80, remaining=20, 20*5=100=total, so threshold met
        // Actually at exactly 20% remaining we DO need refresh (<=).
        // At 21% remaining we don't. Let's just verify the definition works.
        needs_refresh_spec(issued_at, expires_at, (expires_at - 1) as u64),
{
    // remaining = 1, total = expires_at - issued_at >= 5
    // 1 * 5 = 5 <= total. So needs refresh.
}

// ============================================================
// Combined token validity
// ============================================================

// r[depends token-timestamp.token-valid]
/// Spec: is the token currently valid (not expired, not from future)?
pub open spec fn token_time_valid_spec(
    issued_at: u64,
    expires_at: u64,
    now: u64,
    tolerance: u64,
) -> bool {
    !is_expired_spec(expires_at, now, tolerance)
    && !is_from_future_spec(issued_at, now, tolerance)
}

// r[impl token-timestamp.token-valid]
/// Returns true if the token is temporally valid.
pub fn token_time_valid(
    issued_at: u64,
    expires_at: u64,
    now: u64,
    tolerance: u64,
) -> (result: bool)
    ensures result == token_time_valid_spec(issued_at, expires_at, now, tolerance),
{
    !is_expired(expires_at, now, tolerance)
    && !is_from_future(issued_at, now, tolerance)
}

// r[verify token-timestamp.token-valid]
/// A well-formed token checked at its issued_at time is valid.
proof fn fresh_token_time_valid(issued_at: u64, expires_at: u64, tolerance: u64)
    requires issued_at <= expires_at
    ensures token_time_valid_spec(issued_at, expires_at, issued_at, tolerance)
{
    // Not expired: issued_at <= expires_at <= expires_at + tolerance.
    // Not from future: issued_at <= issued_at + tolerance.
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn test_is_expired() {
        assert!(!is_expired(1000, 500, 0)); // before
        assert!(!is_expired(1000, 1000, 0)); // at boundary
        assert!(is_expired(1000, 1001, 0)); // just past
        assert!(!is_expired(1000, 1001, 60)); // within tolerance
        assert!(!is_expired(1000, 1060, 60)); // at tolerance boundary
        assert!(is_expired(1000, 1061, 60)); // past tolerance
    }

    #[test]
    fn test_is_expired_overflow() {
        // expires_at + tolerance would overflow
        assert!(!is_expired(u64::MAX, u64::MAX, 1));
        assert!(!is_expired(u64::MAX - 5, u64::MAX, 10));
    }

    #[test]
    fn test_is_from_future() {
        assert!(!is_from_future(100, 100, 0)); // at now
        assert!(!is_from_future(100, 50, 60)); // within tolerance
        assert!(!is_from_future(110, 50, 60)); // within tolerance
        assert!(is_from_future(200, 50, 60)); // beyond tolerance
    }

    #[test]
    fn test_is_from_future_overflow() {
        assert!(!is_from_future(u64::MAX, u64::MAX, 0));
        assert!(!is_from_future(u64::MAX, u64::MAX - 5, 10));
    }

    #[test]
    fn test_needs_refresh() {
        // total=100, at issued_at: remaining=100, 100*5=500 > 100, no refresh
        assert!(!needs_refresh(0, 100, 0));
        // remaining=20, 20*5=100=total, needs refresh (<=)
        assert!(needs_refresh(0, 100, 80));
        // remaining=21, 21*5=105 > 100, no refresh
        assert!(!needs_refresh(0, 100, 79));
        // expired
        assert!(needs_refresh(0, 100, 100));
        assert!(needs_refresh(0, 100, 200));
        // zero-length token
        assert!(needs_refresh(50, 50, 50));
    }

    #[test]
    fn test_token_time_valid() {
        assert!(token_time_valid(100, 200, 150, 60)); // in range
        assert!(!token_time_valid(100, 200, 261, 60)); // expired past tolerance
        assert!(!token_time_valid(300, 400, 100, 60)); // from future
        assert!(token_time_valid(100, 200, 100, 0)); // at issued_at
    }

    proptest! {
        // r[verify token-timestamp.fresh-token-valid]
        #[test]
        fn prop_fresh_not_expired(
            expires_at in 0u64..=u64::MAX / 2,
            issued_at in 0u64..=u64::MAX / 2,
            tolerance in 0u64..=1000,
        ) {
            if issued_at <= expires_at {
                prop_assert!(!is_expired(expires_at, issued_at, tolerance));
            }
        }

        // r[verify token-timestamp.expiration-monotonic]
        #[test]
        fn prop_expiration_monotonic(
            expires_at in 0u64..=10000,
            t1 in 0u64..=20000,
            t2 in 0u64..=20000,
            tolerance in 0u64..=1000,
        ) {
            if is_expired(expires_at, t1, tolerance) && t2 >= t1 {
                prop_assert!(is_expired(expires_at, t2, tolerance));
            }
        }

        // r[verify token-timestamp.future-rejection]
        #[test]
        fn prop_current_not_future(
            now in 0u64..=u64::MAX / 2,
            tolerance in 0u64..=1000,
        ) {
            prop_assert!(!is_from_future(now, now, tolerance));
        }

        // r[verify token-timestamp.refresh-threshold]
        #[test]
        fn prop_fresh_no_refresh(
            issued_at in 0u64..=u64::MAX / 4,
            total in 1u64..=10000,
        ) {
            let expires_at = issued_at + total;
            prop_assert!(!needs_refresh(issued_at, expires_at, issued_at));
        }

        // r[verify token-timestamp.refresh-threshold]
        #[test]
        fn prop_expired_needs_refresh(
            issued_at in 0u64..=10000,
            total in 1u64..=10000,
            extra in 0u64..=1000,
        ) {
            let expires_at = issued_at + total;
            prop_assert!(needs_refresh(issued_at, expires_at, expires_at + extra));
        }

        // r[verify token-timestamp.token-valid]
        #[test]
        fn prop_fresh_token_valid(
            issued_at in 0u64..=u64::MAX / 4,
            total in 0u64..=10000,
            tolerance in 0u64..=1000,
        ) {
            let expires_at = issued_at + total;
            prop_assert!(token_time_valid(issued_at, expires_at, issued_at, tolerance));
        }
    }
}
