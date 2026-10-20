//! Verified retry decision logic.
//!
//! Decides *whether* to retry a failed operation based on attempt count,
//! error retryability, and an optional budget ratio. Composes with
//! `backoff` (delay timing) and `circuit_breaker` (circuit state).

use vstd::prelude::*;

verus! {

// --- Outcome constants ---

/// Operation succeeded.
pub const SUCCESS: u8 = 0;
/// Operation failed but should be retried.
pub const RETRY: u8 = 1;
/// Operation failed permanently (non-retryable or exhausted).
pub const FAIL: u8 = 2;

// --- should_retry ---

// r[depends retry.should-retry]
/// Spec: retry iff the error is retryable and attempts remain.
pub open spec fn should_retry_spec(attempt: u64, max_attempts: u64, is_retryable: bool) -> bool {
    is_retryable && attempt < max_attempts
}

// r[impl retry.should-retry]
/// Returns true if the operation should be retried.
pub fn should_retry(attempt: u64, max_attempts: u64, is_retryable: bool) -> (result: bool)
    ensures result == should_retry_spec(attempt, max_attempts, is_retryable)
{
    is_retryable && attempt < max_attempts
}

// r[verify retry.should-retry]
/// Non-retryable errors are never retried.
proof fn never_retries_non_retryable(attempt: u64, max_attempts: u64)
    ensures !should_retry_spec(attempt, max_attempts, false)
{}

// r[verify retry.should-retry]
/// Exhausted attempts are never retried.
proof fn never_retries_exhausted(attempt: u64, max_attempts: u64, is_retryable: bool)
    requires attempt >= max_attempts
    ensures !should_retry_spec(attempt, max_attempts, is_retryable)
{}

// r[verify retry.should-retry]
/// First attempt of a retryable error always retries (if max > 0).
proof fn first_attempt_retries(max_attempts: u64)
    requires max_attempts > 0
    ensures should_retry_spec(0, max_attempts, true)
{}

// --- attempts_remaining ---

// r[depends retry.attempts-remaining]
/// Spec: saturating subtraction of attempt from max_attempts.
pub open spec fn attempts_remaining_spec(attempt: u64, max_attempts: u64) -> u64 {
    if attempt < max_attempts {
        (max_attempts - attempt) as u64
    } else {
        0u64
    }
}

// r[impl retry.attempts-remaining]
/// Returns the number of retry attempts remaining.
pub fn attempts_remaining(attempt: u64, max_attempts: u64) -> (result: u64)
    ensures result == attempts_remaining_spec(attempt, max_attempts)
{
    if attempt < max_attempts {
        max_attempts - attempt
    } else {
        0
    }
}

// --- is_exhausted ---

// r[depends retry.is-exhausted]
/// Spec: attempt count has reached or exceeded max.
pub open spec fn is_exhausted_spec(attempt: u64, max_attempts: u64) -> bool {
    attempt >= max_attempts
}

// r[impl retry.is-exhausted]
/// Returns true if all retry attempts have been used.
pub fn is_exhausted(attempt: u64, max_attempts: u64) -> (result: bool)
    ensures result == is_exhausted_spec(attempt, max_attempts)
{
    attempt >= max_attempts
}

// r[verify retry.is-exhausted]
/// Exhausted iff zero remaining.
proof fn exhausted_iff_zero_remaining(attempt: u64, max_attempts: u64)
    ensures is_exhausted_spec(attempt, max_attempts) == (attempts_remaining_spec(attempt, max_attempts) == 0)
{}

// r[verify retry.attempts-remaining]
/// Incrementing the attempt decreases remaining (or stays at 0).
proof fn remaining_monotonic(attempt: u64, max_attempts: u64)
    requires attempt as int + 1 <= u64::MAX as int
    ensures attempts_remaining_spec((attempt + 1) as u64, max_attempts) as int
        <= attempts_remaining_spec(attempt, max_attempts) as int
{}

// r[verify retry.should-retry]
/// should_retry with retryable == true is the complement of is_exhausted.
proof fn should_retry_iff_not_exhausted(attempt: u64, max_attempts: u64)
    ensures should_retry_spec(attempt, max_attempts, true) == !is_exhausted_spec(attempt, max_attempts)
{}

// --- retry_budget_ok ---

// r[depends retry.retry-budget]
/// Spec: failure ratio is within budget. Uses multiplication to avoid
/// division by zero: failures * 100 <= total * budget_pct.
/// Empty window (total == 0) is always within budget.
pub open spec fn retry_budget_ok_spec(successes: u64, failures: u64, budget_pct: u64) -> bool
    recommends
        successes as int + failures as int <= u64::MAX as int,
        (successes as int + failures as int) * budget_pct as int <= u64::MAX as int,
{
    let total = successes as int + failures as int;
    if total == 0 {
        true
    } else {
        failures as int * 100 <= total * budget_pct as int
    }
}

// r[impl retry.retry-budget]
/// Returns true if failures are within the budget percentage of total operations.
pub fn retry_budget_ok(successes: u64, failures: u64, budget_pct: u64) -> (result: bool)
    requires
        successes as int + failures as int <= u64::MAX as int,
        (successes as int + failures as int) as int * budget_pct as int <= u64::MAX as int,
        failures as int * 100 <= u64::MAX as int,
    ensures result == retry_budget_ok_spec(successes, failures, budget_pct)
{
    let total = successes + failures;
    if total == 0 {
        true
    } else {
        failures * 100 <= total * budget_pct
    }
}

// r[verify retry.retry-budget]
/// Empty window is always within budget.
proof fn empty_window_always_ok(budget_pct: u64)
    ensures retry_budget_ok_spec(0, 0, budget_pct)
{}

// r[verify retry.retry-budget]
/// Zero failures is always within budget.
proof fn zero_failures_always_ok(successes: u64, budget_pct: u64)
    requires successes as int * budget_pct as int <= u64::MAX as int
    ensures retry_budget_ok_spec(successes, 0, budget_pct)
{}

// r[verify retry.retry-budget]
/// Budget >= 100% always passes (failures can't exceed total).
proof fn full_budget_always_ok(successes: u64, failures: u64)
    requires
        successes as int + failures as int <= u64::MAX as int,
        (successes as int + failures as int) * 100 <= u64::MAX as int,
    ensures retry_budget_ok_spec(successes, failures, 100)
{}

// --- classify_outcome ---

// r[depends retry.classify-outcome]
/// Spec: map every outcome to SUCCESS (0), RETRY (1), or FAIL (2).
pub open spec fn classify_outcome_spec(
    succeeded: bool,
    is_retryable: bool,
    attempt: u64,
    max_attempts: u64,
) -> u8 {
    if succeeded {
        SUCCESS
    } else if should_retry_spec(attempt, max_attempts, is_retryable) {
        RETRY
    } else {
        FAIL
    }
}

// r[impl retry.classify-outcome]
/// Returns SUCCESS (0), RETRY (1), or FAIL (2).
pub fn classify_outcome(
    succeeded: bool,
    is_retryable: bool,
    attempt: u64,
    max_attempts: u64,
) -> (result: u8)
    ensures result == classify_outcome_spec(succeeded, is_retryable, attempt, max_attempts)
{
    if succeeded {
        SUCCESS
    } else if should_retry(attempt, max_attempts, is_retryable) {
        RETRY
    } else {
        FAIL
    }
}

// r[verify retry.classify-outcome]
/// classify always returns a valid outcome.
proof fn classify_is_valid(
    succeeded: bool, is_retryable: bool, attempt: u64, max_attempts: u64,
)
    ensures ({
        let c = classify_outcome_spec(succeeded, is_retryable, attempt, max_attempts);
        c == SUCCESS || c == RETRY || c == FAIL
    })
{}

// r[verify retry.classify-outcome]
/// Success always classifies as SUCCESS.
proof fn success_always_success(
    is_retryable: bool, attempt: u64, max_attempts: u64,
)
    ensures classify_outcome_spec(true, is_retryable, attempt, max_attempts) == SUCCESS
{}

// r[verify retry.classify-outcome]
/// Non-retryable errors never classify as RETRY.
proof fn non_retryable_never_retry(
    succeeded: bool, attempt: u64, max_attempts: u64,
)
    ensures classify_outcome_spec(succeeded, false, attempt, max_attempts) != RETRY
{}

// r[verify retry.classify-outcome]
/// classify == RETRY iff should_retry (when not succeeded).
proof fn classify_agrees_with_should_retry(
    is_retryable: bool, attempt: u64, max_attempts: u64,
)
    ensures
        (classify_outcome_spec(false, is_retryable, attempt, max_attempts) == RETRY)
            == should_retry_spec(attempt, max_attempts, is_retryable)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // --- Unit tests ---

    // r[verify retry.should-retry]
    #[test]
    fn test_should_retry() {
        assert!(should_retry(0, 5, true));
        assert!(should_retry(2, 5, true));
        assert!(should_retry(4, 5, true));
        assert!(!should_retry(5, 5, true));
        assert!(!should_retry(6, 5, true));
        assert!(!should_retry(0, 5, false));
        assert!(!should_retry(0, 0, true));
    }

    // r[verify retry.attempts-remaining]
    #[test]
    fn test_attempts_remaining() {
        assert_eq!(attempts_remaining(0, 5), 5);
        assert_eq!(attempts_remaining(3, 10), 7);
        assert_eq!(attempts_remaining(5, 5), 0);
        assert_eq!(attempts_remaining(10, 5), 0);
        assert_eq!(attempts_remaining(0, 0), 0);
    }

    // r[verify retry.is-exhausted]
    #[test]
    fn test_is_exhausted() {
        assert!(!is_exhausted(0, 5));
        assert!(!is_exhausted(4, 5));
        assert!(is_exhausted(5, 5));
        assert!(is_exhausted(6, 5));
        assert!(is_exhausted(0, 0));
    }

    // r[verify retry.retry-budget]
    #[test]
    fn test_retry_budget_ok() {
        assert!(retry_budget_ok(0, 0, 20));
        assert!(retry_budget_ok(90, 10, 20));
        assert!(retry_budget_ok(80, 20, 20));
        assert!(!retry_budget_ok(50, 50, 20));
        assert!(!retry_budget_ok(0, 1, 0));
        assert!(retry_budget_ok(0, 100, 100));
        assert!(retry_budget_ok(99, 1, 1));
    }

    // r[verify retry.classify-outcome]
    #[test]
    fn test_classify_outcome() {
        assert_eq!(classify_outcome(true, true, 0, 5), SUCCESS);
        assert_eq!(classify_outcome(true, false, 0, 5), SUCCESS);
        assert_eq!(classify_outcome(false, true, 0, 5), RETRY);
        assert_eq!(classify_outcome(false, true, 2, 5), RETRY);
        assert_eq!(classify_outcome(false, true, 5, 5), FAIL);
        assert_eq!(classify_outcome(false, false, 0, 5), FAIL);
        assert_eq!(classify_outcome(false, false, 5, 5), FAIL);
    }

    // --- Property tests ---

    proptest! {
        // r[verify retry.should-retry]
        #[test]
        fn prop_should_retry_agrees_with_exhausted(
            attempt in 0u64..=1000,
            max_attempts in 0u64..=1000,
        ) {
            prop_assert_eq!(
                should_retry(attempt, max_attempts, true),
                !is_exhausted(attempt, max_attempts)
            );
        }

        // r[verify retry.classify-outcome]
        #[test]
        fn prop_classify_exhaustive(
            succeeded in proptest::bool::ANY,
            is_retryable in proptest::bool::ANY,
            attempt in 0u64..=1000,
            max_attempts in 0u64..=1000,
        ) {
            let c = classify_outcome(succeeded, is_retryable, attempt, max_attempts);
            prop_assert!(c == SUCCESS || c == RETRY || c == FAIL);
        }

        // r[verify retry.retry-budget]
        #[test]
        fn prop_budget_monotonic_fewer_failures(
            successes in 0u64..=10000,
            failures in 1u64..=10000,
            budget_pct in 1u64..=100,
        ) {
            // Guard against overflow
            let total = u128::from(successes) + u128::from(failures);
            if total * u128::from(budget_pct) <= u128::from(u64::MAX)
                && u128::from(failures) * 100 <= u128::from(u64::MAX)
            {
                if retry_budget_ok(successes, failures, budget_pct) {
                    // Fewer failures should also be within budget
                    let fewer = failures - 1;
                    if u128::from(fewer) * 100 <= u128::from(u64::MAX) {
                        let new_total = u128::from(successes) + u128::from(fewer);
                        if new_total * u128::from(budget_pct) <= u128::from(u64::MAX) {
                            prop_assert!(
                                retry_budget_ok(successes, fewer, budget_pct),
                                "fewer failures should still be within budget"
                            );
                        }
                    }
                }
            }
        }

        // r[verify retry.classify-outcome]
        #[test]
        fn prop_classify_non_retryable_never_retry(
            succeeded in proptest::bool::ANY,
            attempt in 0u64..=1000,
            max_attempts in 0u64..=1000,
        ) {
            let c = classify_outcome(succeeded, false, attempt, max_attempts);
            prop_assert_ne!(c, RETRY);
        }
    }
}
