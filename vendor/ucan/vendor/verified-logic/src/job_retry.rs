//! Verified job retry decision logic.
//!
//! Determines whether a failed job should be retried based on three
//! criteria: error classification (transient vs permanent), attempt
//! count vs maximum, and cooldown elapsed since last attempt.
//!
//! Error code ranges: 0-99 permanent (non-retryable), 100-199 transient
//! (retryable), 200+ unknown (non-retryable).

use vstd::prelude::*;

verus! {

// ============================================================
// is_retryable_error: error code classification
// ============================================================

// r[depends job-retry.is-retryable]
/// Spec: error code is in the transient (retryable) range [100, 200).
pub open spec fn is_retryable_error_spec(error_code: u64) -> bool {
    error_code >= 100 && error_code < 200
}

// r[impl job-retry.is-retryable]
/// Classify an error code as retryable (transient) or not.
pub fn is_retryable_error(error_code: u64) -> (result: bool)
    ensures result == is_retryable_error_spec(error_code)
{
    error_code >= 100 && error_code < 200
}

// ============================================================
// attempts_remaining
// ============================================================

// r[depends job-retry.attempts-remaining]
/// Spec: how many retry attempts remain.
pub open spec fn attempts_remaining_spec(attempts: u64, max_attempts: u64) -> u64 {
    if attempts < max_attempts {
        (max_attempts - attempts) as u64
    } else {
        0u64
    }
}

// r[impl job-retry.attempts-remaining]
/// Compute how many retry attempts remain.
pub fn attempts_remaining(attempts: u64, max_attempts: u64) -> (result: u64)
    ensures result == attempts_remaining_spec(attempts, max_attempts)
{
    if attempts < max_attempts {
        max_attempts - attempts
    } else {
        0
    }
}

// ============================================================
// is_cooldown_elapsed
// ============================================================

// r[depends job-retry.cooldown-elapsed]
/// Spec: cooldown period has elapsed since last attempt.
/// Uses int arithmetic to avoid overflow in spec.
pub open spec fn is_cooldown_elapsed_spec(
    now: u64,
    last_attempt_time: u64,
    cooldown_ms: u64,
) -> bool {
    now as int >= last_attempt_time as int + cooldown_ms as int
}

// r[impl job-retry.cooldown-elapsed]
/// Check if the cooldown period has elapsed since the last attempt.
/// When last_attempt_time + cooldown_ms would overflow u64, the deadline
/// is unreachable (no u64 value is large enough), so return false.
pub fn is_cooldown_elapsed(
    now: u64,
    last_attempt_time: u64,
    cooldown_ms: u64,
) -> (result: bool)
    ensures result == is_cooldown_elapsed_spec(now, last_attempt_time, cooldown_ms)
{
    if cooldown_ms > u64::MAX - last_attempt_time {
        // Overflow: the int-level deadline exceeds u64::MAX, so now can never reach it.
        false
    } else {
        now >= last_attempt_time + cooldown_ms
    }
}

// ============================================================
// should_retry: conjunction of three criteria
// ============================================================

// r[depends job-retry.should-retry]
/// Spec: should a failed job be retried?
/// True only when: error is retryable AND attempts < max AND cooldown elapsed.
pub open spec fn should_retry_spec(
    error_code: u64,
    attempts: u64,
    max_attempts: u64,
    now: u64,
    last_attempt_time: u64,
    cooldown_ms: u64,
) -> bool {
    is_retryable_error_spec(error_code)
    && attempts < max_attempts
    && is_cooldown_elapsed_spec(now, last_attempt_time, cooldown_ms)
}

// r[impl job-retry.should-retry]
/// Decide whether a failed job should be retried.
pub fn should_retry(
    error_code: u64,
    attempts: u64,
    max_attempts: u64,
    now: u64,
    last_attempt_time: u64,
    cooldown_ms: u64,
) -> (result: bool)
    ensures result == should_retry_spec(error_code, attempts, max_attempts, now, last_attempt_time, cooldown_ms)
{
    is_retryable_error(error_code)
        && attempts < max_attempts
        && is_cooldown_elapsed(now, last_attempt_time, cooldown_ms)
}

// ============================================================
// next_attempt: capped increment
// ============================================================

// r[depends job-retry.next-attempt]
/// Spec: next attempt number, capped at max_attempts.
pub open spec fn next_attempt_spec(attempts: u64, max_attempts: u64) -> u64 {
    if attempts < max_attempts {
        (attempts + 1) as u64
    } else {
        max_attempts
    }
}

// r[impl job-retry.next-attempt]
/// Compute the next attempt number, capped at max_attempts.
pub fn next_attempt(attempts: u64, max_attempts: u64) -> (result: u64)
    ensures result == next_attempt_spec(attempts, max_attempts)
{
    if attempts < max_attempts {
        attempts + 1
    } else {
        max_attempts
    }
}

// ============================================================
// Safety proofs
// ============================================================

// r[verify job-retry.is-retryable]
/// Permanent errors (0-99) are not retryable.
proof fn permanent_not_retryable(error_code: u64)
    requires error_code < 100
    ensures !is_retryable_error_spec(error_code)
{}

// r[verify job-retry.is-retryable]
/// Unknown errors (200+) are not retryable.
proof fn unknown_not_retryable(error_code: u64)
    requires error_code >= 200
    ensures !is_retryable_error_spec(error_code)
{}

// r[verify job-retry.is-retryable]
/// Transient errors (100-199) are retryable.
proof fn transient_is_retryable(error_code: u64)
    requires error_code >= 100, error_code < 200
    ensures is_retryable_error_spec(error_code)
{}

// r[verify job-retry.should-retry]
/// Non-retryable error → should_retry is false.
proof fn non_retryable_never_retries(
    error_code: u64,
    attempts: u64,
    max_attempts: u64,
    now: u64,
    last_attempt_time: u64,
    cooldown_ms: u64,
)
    requires !is_retryable_error_spec(error_code)
    ensures !should_retry_spec(error_code, attempts, max_attempts, now, last_attempt_time, cooldown_ms)
{}

// r[verify job-retry.should-retry]
/// Max attempts reached → should_retry is false.
proof fn max_attempts_never_retries(
    error_code: u64,
    attempts: u64,
    max_attempts: u64,
    now: u64,
    last_attempt_time: u64,
    cooldown_ms: u64,
)
    requires attempts >= max_attempts
    ensures !should_retry_spec(error_code, attempts, max_attempts, now, last_attempt_time, cooldown_ms)
{}

// r[verify job-retry.should-retry]
/// Cooldown not elapsed → should_retry is false.
proof fn cooldown_blocks_retry(
    error_code: u64,
    attempts: u64,
    max_attempts: u64,
    now: u64,
    last_attempt_time: u64,
    cooldown_ms: u64,
)
    requires !is_cooldown_elapsed_spec(now, last_attempt_time, cooldown_ms)
    ensures !should_retry_spec(error_code, attempts, max_attempts, now, last_attempt_time, cooldown_ms)
{}

// r[verify job-retry.attempts-remaining]
/// Attempts remaining is bounded by max_attempts.
proof fn remaining_bounded(attempts: u64, max_attempts: u64)
    ensures attempts_remaining_spec(attempts, max_attempts) <= max_attempts
{}

// r[verify job-retry.attempts-remaining]
/// Attempts remaining is zero iff attempts >= max_attempts.
proof fn remaining_zero_iff_exhausted(attempts: u64, max_attempts: u64)
    ensures
        (attempts_remaining_spec(attempts, max_attempts) == 0)
            <==> (attempts >= max_attempts)
{}

// r[verify job-retry.next-attempt]
/// Next attempt is always <= max_attempts.
proof fn next_attempt_bounded(attempts: u64, max_attempts: u64)
    ensures next_attempt_spec(attempts, max_attempts) <= max_attempts
{}

// r[verify job-retry.next-attempt]
/// Next attempt is >= attempts when attempts <= max_attempts.
proof fn next_attempt_monotonic(attempts: u64, max_attempts: u64)
    requires attempts <= max_attempts
    ensures next_attempt_spec(attempts, max_attempts) >= attempts
{}

// r[verify job-retry.cooldown-elapsed]
/// Cooldown monotonicity: if elapsed at time t, still elapsed at t' > t.
proof fn cooldown_monotonic(
    now: u64,
    later: u64,
    last_attempt_time: u64,
    cooldown_ms: u64,
)
    requires
        now <= later,
        is_cooldown_elapsed_spec(now, last_attempt_time, cooldown_ms),
    ensures
        is_cooldown_elapsed_spec(later, last_attempt_time, cooldown_ms)
{}

// r[verify job-retry.attempts-remaining]
/// After next_attempt, remaining decreases by 1 (when not exhausted).
proof fn next_attempt_decreases_remaining(attempts: u64, max_attempts: u64)
    requires attempts < max_attempts
    ensures
        attempts_remaining_spec(next_attempt_spec(attempts, max_attempts), max_attempts)
            == attempts_remaining_spec(attempts, max_attempts) - 1
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify job-retry.is-retryable]
    #[test]
    fn test_error_classification() {
        // Permanent (0-99)
        assert!(!is_retryable_error(0));
        assert!(!is_retryable_error(42));
        assert!(!is_retryable_error(99));
        // Transient (100-199)
        assert!(is_retryable_error(100));
        assert!(is_retryable_error(150));
        assert!(is_retryable_error(199));
        // Unknown (200+)
        assert!(!is_retryable_error(200));
        assert!(!is_retryable_error(500));
        assert!(!is_retryable_error(u64::MAX));
    }

    // r[verify job-retry.attempts-remaining]
    #[test]
    fn test_attempts_remaining() {
        assert_eq!(attempts_remaining(0, 5), 5);
        assert_eq!(attempts_remaining(2, 5), 3);
        assert_eq!(attempts_remaining(5, 5), 0);
        assert_eq!(attempts_remaining(7, 5), 0);
        assert_eq!(attempts_remaining(0, 0), 0);
    }

    // r[verify job-retry.cooldown-elapsed]
    #[test]
    fn test_cooldown_elapsed() {
        assert!(is_cooldown_elapsed(1600, 1000, 500)); // 1600 >= 1500
        assert!(!is_cooldown_elapsed(1200, 1000, 500)); // 1200 < 1500
        assert!(is_cooldown_elapsed(1500, 1000, 500)); // 1500 >= 1500 (boundary)
        assert!(is_cooldown_elapsed(0, 0, 0)); // 0 >= 0
        // Overflow: last=u64::MAX-50, cooldown=100 → int deadline > u64::MAX, unreachable
        assert!(!is_cooldown_elapsed(u64::MAX - 10, u64::MAX - 50, 100));
        assert!(!is_cooldown_elapsed(u64::MAX, u64::MAX - 50, 100)); // even u64::MAX can't reach it
    }

    // r[verify job-retry.should-retry]
    #[test]
    fn test_should_retry() {
        // All conditions met
        assert!(should_retry(150, 2, 5, 2000, 1000, 500));
        // Non-retryable error
        assert!(!should_retry(42, 2, 5, 2000, 1000, 500));
        // Max attempts reached
        assert!(!should_retry(150, 5, 5, 2000, 1000, 500));
        // Cooldown not elapsed
        assert!(!should_retry(150, 2, 5, 1200, 1000, 500));
    }

    // r[verify job-retry.next-attempt]
    #[test]
    fn test_next_attempt() {
        assert_eq!(next_attempt(0, 5), 1);
        assert_eq!(next_attempt(2, 5), 3);
        assert_eq!(next_attempt(4, 5), 5);
        assert_eq!(next_attempt(5, 5), 5); // capped
        assert_eq!(next_attempt(10, 5), 5); // capped
    }

    proptest! {
        // r[verify job-retry.is-retryable]
        #[test]
        fn prop_classification_exhaustive(error_code in 0u64..300) {
            let permanent = error_code < 100;
            let transient = error_code >= 100 && error_code < 200;
            let unknown = error_code >= 200;
            // Exactly one category
            prop_assert_eq!(
                (permanent as u8) + (transient as u8) + (unknown as u8),
                1
            );
            prop_assert_eq!(is_retryable_error(error_code), transient);
        }

        // r[verify job-retry.attempts-remaining]
        #[test]
        fn prop_remaining_bounded(attempts in 0u64..100, max_attempts in 0u64..100) {
            let rem = attempts_remaining(attempts, max_attempts);
            prop_assert!(rem <= max_attempts);
        }

        // r[verify job-retry.attempts-remaining]
        #[test]
        fn prop_remaining_zero_iff_exhausted(attempts in 0u64..100, max_attempts in 0u64..100) {
            let rem = attempts_remaining(attempts, max_attempts);
            prop_assert_eq!(rem == 0, attempts >= max_attempts);
        }

        // r[verify job-retry.next-attempt]
        #[test]
        fn prop_next_attempt_bounded(attempts in 0u64..100, max_attempts in 0u64..100) {
            let next = next_attempt(attempts, max_attempts);
            prop_assert!(next <= max_attempts);
            if attempts <= max_attempts {
                prop_assert!(next >= attempts);
            }
        }

        // r[verify job-retry.should-retry]
        #[test]
        fn prop_non_retryable_never_retries(
            error_code in (0u64..100).prop_union(200u64..500),
            attempts in 0u64..10,
            max_attempts in 0u64..10,
            now in 0u64..10000,
            last_attempt in 0u64..10000,
            cooldown in 0u64..5000,
        ) {
            prop_assert!(!should_retry(error_code, attempts, max_attempts, now, last_attempt, cooldown));
        }

        // r[verify job-retry.should-retry]
        #[test]
        fn prop_max_attempts_blocks(
            error_code in 100u64..200,
            max_attempts in 0u64..10,
            now in 5000u64..10000,
            last_attempt in 0u64..1000,
            cooldown in 0u64..100,
        ) {
            // attempts == max_attempts → never retries
            prop_assert!(!should_retry(error_code, max_attempts, max_attempts, now, last_attempt, cooldown));
        }

        // r[verify job-retry.cooldown-elapsed]
        #[test]
        fn prop_cooldown_monotonic(
            now in 0u64..10000,
            delta in 0u64..5000,
            last_attempt in 0u64..5000,
            cooldown in 0u64..5000,
        ) {
            let later = now.saturating_add(delta);
            if is_cooldown_elapsed(now, last_attempt, cooldown) {
                prop_assert!(is_cooldown_elapsed(later, last_attempt, cooldown));
            }
        }

        // r[verify job-retry.next-attempt]
        #[test]
        fn prop_next_decreases_remaining(
            attempts in 0u64..99,
            max_attempts in 1u64..100,
        ) {
            if attempts < max_attempts {
                let next = next_attempt(attempts, max_attempts);
                let rem_before = attempts_remaining(attempts, max_attempts);
                let rem_after = attempts_remaining(next, max_attempts);
                prop_assert_eq!(rem_after, rem_before - 1);
            }
        }
    }
}
