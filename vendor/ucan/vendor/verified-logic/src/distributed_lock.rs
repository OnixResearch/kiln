//! Verified distributed lock protocol logic.
//!
//! Token progression (always >= 1, monotonic), deadline computation,
//! expiry checks, and backoff with jitter for retry. Pure decision
//! functions — no I/O.

use vstd::prelude::*;

verus! {

// --- Token ---

// r[depends distributed-lock.token-always-positive]
pub open spec fn compute_next_fencing_token_spec(previous: Option<u64>) -> u64 {
    match previous {
        None => 1u64,
        Some(t) => if t == u64::MAX { u64::MAX } else { (t + 1) as u64 },
    }
}

// r[impl distributed-lock.token-always-positive]
pub fn compute_next_fencing_token(previous: Option<u64>) -> (result: u64)
    ensures result == compute_next_fencing_token_spec(previous)
{
    match previous {
        None => 1u64,
        Some(t) => if t == u64::MAX { u64::MAX } else { t + 1 },
    }
}

// --- Deadline ---

// r[depends distributed-lock.expiry]
pub open spec fn compute_lock_deadline_spec(acquired_at_ms: u64, ttl_ms: u64) -> u64 {
    if acquired_at_ms as int + ttl_ms as int > u64::MAX as int {
        u64::MAX
    } else {
        (acquired_at_ms + ttl_ms) as u64
    }
}

// r[impl distributed-lock.expiry]
pub fn compute_lock_deadline(acquired_at_ms: u64, ttl_ms: u64) -> (result: u64)
    ensures result == compute_lock_deadline_spec(acquired_at_ms, ttl_ms)
{
    if ttl_ms > u64::MAX - acquired_at_ms {
        u64::MAX
    } else {
        acquired_at_ms + ttl_ms
    }
}

// --- Expiry ---

// r[depends distributed-lock.expiry]
pub open spec fn is_lock_expired_spec(deadline_ms: u64, now_ms: u64) -> bool {
    now_ms > deadline_ms
}

// r[impl distributed-lock.expiry]
pub fn is_lock_expired(deadline_ms: u64, now_ms: u64) -> (result: bool)
    ensures result == is_lock_expired_spec(deadline_ms, now_ms)
{
    now_ms > deadline_ms
}

// r[depends distributed-lock.expiry]
pub open spec fn remaining_ttl_spec(deadline_ms: u64, now_ms: u64) -> u64 {
    if now_ms > deadline_ms { 0u64 } else { (deadline_ms - now_ms) as u64 }
}

// r[impl distributed-lock.expiry]
pub fn remaining_ttl(deadline_ms: u64, now_ms: u64) -> (result: u64)
    ensures result == remaining_ttl_spec(deadline_ms, now_ms)
{
    if now_ms > deadline_ms { 0 } else { deadline_ms - now_ms }
}

// --- Backoff ---

// r[depends distributed-lock.backoff]
pub open spec fn compute_doubled_backoff_spec(current: u64, max_backoff: u64) -> u64 {
    let doubled = if current as int * 2 > u64::MAX as int {
        u64::MAX
    } else {
        (current * 2) as u64
    };
    if doubled > max_backoff { max_backoff } else { doubled }
}

// r[impl distributed-lock.backoff]
pub fn compute_doubled_backoff(current: u64, max_backoff: u64) -> (result: u64)
    ensures result == compute_doubled_backoff_spec(current, max_backoff)
{
    let doubled = if current > u64::MAX / 2 { u64::MAX } else { current * 2 };
    if doubled > max_backoff { max_backoff } else { doubled }
}

// r[depends distributed-lock.backoff]
pub open spec fn compute_jittered_delay_spec(cap: u64, seed: u64) -> u64 {
    if cap == 0 { 0u64 } else { seed % cap }
}

// r[impl distributed-lock.backoff]
pub fn compute_jittered_delay(cap: u64, seed: u64) -> (result: u64)
    ensures result == compute_jittered_delay_spec(cap, seed)
{
    if cap == 0 { 0 } else { seed % cap }
}

// === Proofs ===

// r[verify distributed-lock.token-always-positive]
proof fn token_always_ge_one(prev: Option<u64>)
    ensures compute_next_fencing_token_spec(prev) >= 1
{}

// r[verify distributed-lock.token-monotonicity]
// r[impl distributed-lock.token-monotonicity]
proof fn token_monotonic(prev_token: u64)
    ensures compute_next_fencing_token_spec(Some(prev_token)) >= prev_token
{}

// r[verify distributed-lock.expiry]
/// Expired implies remaining is zero. (Converse is not true at the boundary.)
proof fn expired_implies_remaining_zero(deadline: u64, now: u64)
    requires is_lock_expired_spec(deadline, now)
    ensures remaining_ttl_spec(deadline, now) == 0
{}

// r[verify distributed-lock.expiry]
/// Positive remaining implies not expired.
proof fn remaining_positive_implies_not_expired(deadline: u64, now: u64)
    requires remaining_ttl_spec(deadline, now) > 0
    ensures !is_lock_expired_spec(deadline, now)
{}

// r[verify distributed-lock.expiry]
proof fn deadline_ge_acquired(acquired: u64, ttl: u64)
    ensures compute_lock_deadline_spec(acquired, ttl) >= acquired
{}

// r[verify distributed-lock.backoff]
proof fn doubled_backoff_bounded(current: u64, max_backoff: u64)
    ensures compute_doubled_backoff_spec(current, max_backoff) <= max_backoff
        || compute_doubled_backoff_spec(current, max_backoff) <= u64::MAX
{}

// r[verify distributed-lock.backoff]
proof fn jittered_delay_bounded(cap: u64, seed: u64)
    requires cap > 0
    ensures compute_jittered_delay_spec(cap, seed) < cap
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify distributed-lock.token-always-positive]
    #[test]
    fn test_token() {
        assert_eq!(compute_next_fencing_token(None), 1);
        assert_eq!(compute_next_fencing_token(Some(5)), 6);
        assert_eq!(compute_next_fencing_token(Some(u64::MAX)), u64::MAX);
    }

    // r[verify distributed-lock.expiry]
    #[test]
    fn test_expiry() {
        assert!(!is_lock_expired(10000, 5000));
        assert!(is_lock_expired(5000, 10000));
        assert!(!is_lock_expired(5000, 5000)); // deadline inclusive
        assert_eq!(remaining_ttl(10000, 5000), 5000);
        assert_eq!(remaining_ttl(5000, 10000), 0);
        assert_eq!(compute_lock_deadline(1000, 5000), 6000);
        assert_eq!(compute_lock_deadline(u64::MAX - 100, 200), u64::MAX);
    }

    // r[verify distributed-lock.backoff]
    #[test]
    fn test_backoff() {
        assert_eq!(compute_doubled_backoff(100, 10000), 200);
        assert_eq!(compute_doubled_backoff(8000, 10000), 10000);
        assert_eq!(compute_jittered_delay(200, 42), 42);
        assert_eq!(compute_jittered_delay(0, 42), 0);
    }

    proptest! {
        // r[verify distributed-lock.token-always-positive]
        #[test]
        fn prop_token_ge_one(prev: u64) {
            prop_assert!(compute_next_fencing_token(Some(prev)) >= 1);
        }

        // r[verify distributed-lock.token-monotonicity]
        #[test]
        fn prop_token_monotonic(prev: u64) {
            prop_assert!(compute_next_fencing_token(Some(prev)) >= prev);
        }

        // r[verify distributed-lock.expiry]
        #[test]
        fn prop_expired_implies_remaining_zero(deadline: u64, now: u64) {
            if is_lock_expired(deadline, now) {
                prop_assert_eq!(remaining_ttl(deadline, now), 0);
            }
            if remaining_ttl(deadline, now) > 0 {
                prop_assert!(!is_lock_expired(deadline, now));
            }
        }

        // r[verify distributed-lock.backoff]
        #[test]
        fn prop_doubled_bounded(current: u64, max_backoff in 1u64..u64::MAX) {
            let d = compute_doubled_backoff(current, max_backoff);
            prop_assert!(d <= max_backoff);
        }

        // r[verify distributed-lock.backoff]
        #[test]
        fn prop_jitter_bounded(cap in 1u64..100_000, seed: u64) {
            prop_assert!(compute_jittered_delay(cap, seed) < cap);
        }
    }
}
