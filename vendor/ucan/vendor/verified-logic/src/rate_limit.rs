//! Verified integer-based token bucket rate limiting.
//!
//! Token bucket rate limiter using integer arithmetic (millitokens)
//! to avoid floating point precision issues. Functions for refill,
//! consumption, burst checking, and wait time calculation with
//! overflow safety and machine-checked correctness proofs.

// Verus proofs guarantee u128→u64 casts are in bounds
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::manual_div_ceil)]

use vstd::prelude::*;

verus! {

// --- tokens_after_refill ---

// r[depends rate-limit.refill]
/// Spec: tokens available after time-based refill.
/// Formula: min(tokens + elapsed_ms * rate_per_sec / 1000, burst).
/// Uses pure int arithmetic — no overflow.
pub open spec fn tokens_after_refill_spec(
    tokens: u64,
    elapsed_ms: u64,
    rate_per_sec: u64,
    burst: u64,
) -> u64 {
    let refill_amount_int = elapsed_ms as int * rate_per_sec as int / 1000;
    let new_tokens_int = tokens as int + refill_amount_int;
    if new_tokens_int > burst as int { burst } else { new_tokens_int as u64 }
}

// r[impl rate-limit.refill]
/// Returns tokens available after refill: tokens + `elapsed_ms` * `rate_per_sec` / 1000,
/// capped at burst. Uses u128 intermediate to avoid overflow splitting.
pub fn tokens_after_refill(
    tokens: u64,
    elapsed_ms: u64,
    rate_per_sec: u64,
    burst: u64,
) -> (result: u64)
    ensures result == tokens_after_refill_spec(tokens, elapsed_ms, rate_per_sec, burst)
{
    // u64 * u64 fits in u128: (2^64-1)^2 = 2^128 - 2^65 + 1 < 2^128 - 1
    assert((elapsed_ms as int) * (rate_per_sec as int) <= (u128::MAX as int)) by(nonlinear_arith)
        requires
            0 <= elapsed_ms as int <= u64::MAX as int,
            0 <= rate_per_sec as int <= u64::MAX as int,
            (u64::MAX as int) * (u64::MAX as int) <= (u128::MAX as int),
    ;
    let product: u128 = elapsed_ms as u128 * rate_per_sec as u128;
    let refill_128: u128 = product / 1000;

    // If refill exceeds burst, the spec returns burst (new_tokens > burst since tokens >= 0).
    if refill_128 > burst as u128 {
        return burst;
    }
    // refill_128 <= burst <= u64::MAX, so the cast is safe.
    let refill: u64 = refill_128 as u64;

    // Check addition overflow.
    if tokens > u64::MAX - refill {
        // tokens + refill > u64::MAX >= burst, so spec returns burst.
        return burst;
    }

    let new_tokens: u64 = tokens + refill;
    if new_tokens > burst { burst } else { new_tokens }
}

// r[verify rate-limit.refill]
/// Refill result is always bounded by burst capacity.
proof fn refill_bounded_by_burst(tokens: u64, elapsed_ms: u64, rate_per_sec: u64, burst: u64)
    ensures tokens_after_refill_spec(tokens, elapsed_ms, rate_per_sec, burst) <= burst
{}

// r[verify rate-limit.refill]
/// Refill is monotonic in elapsed time: more time yields more tokens (up to burst).
proof fn refill_monotonic_time(tokens: u64, t1: u64, t2: u64, rate_per_sec: u64, burst: u64)
    requires t1 <= t2
    ensures tokens_after_refill_spec(tokens, t1, rate_per_sec, burst)
            <= tokens_after_refill_spec(tokens, t2, rate_per_sec, burst)
{
    assert(t1 as int * rate_per_sec as int <= t2 as int * rate_per_sec as int) by(nonlinear_arith)
        requires t1 as int <= t2 as int, rate_per_sec as int >= 0int
    ;
    assert(t1 as int * rate_per_sec as int / 1000 <= t2 as int * rate_per_sec as int / 1000) by(nonlinear_arith)
        requires t1 as int * rate_per_sec as int <= t2 as int * rate_per_sec as int
    ;
}

// r[verify rate-limit.refill]
/// Zero elapsed time returns tokens unchanged.
proof fn refill_identity(tokens: u64, rate_per_sec: u64, burst: u64)
    requires tokens <= burst
    ensures tokens_after_refill_spec(tokens, 0, rate_per_sec, burst) == tokens
{}

// --- try_consume ---

// r[depends rate-limit.try-consume]
/// Spec: attempt to consume tokens for a request.
/// Returns (success, remaining_tokens).
pub open spec fn try_consume_spec(tokens: u64, cost: u64) -> (bool, u64) {
    if tokens >= cost {
        (true, (tokens - cost) as u64)
    } else {
        (false, tokens)
    }
}

// r[impl rate-limit.try-consume]
/// Attempts to consume tokens. Returns (success, `remaining_tokens`).
/// If tokens >= cost, returns (true, tokens - cost).
/// Otherwise returns (false, tokens) unchanged.
pub fn try_consume(tokens: u64, cost: u64) -> (result: (bool, u64))
    ensures result == try_consume_spec(tokens, cost)
{
    if tokens >= cost {
        (true, tokens - cost)
    } else {
        (false, tokens)
    }
}

// r[verify rate-limit.try-consume]
/// When consumption succeeds, remaining tokens equal original minus cost.
proof fn consume_success_correctness(tokens: u64, cost: u64)
    requires tokens >= cost
    ensures try_consume_spec(tokens, cost).1 == tokens - cost
{}

// r[verify rate-limit.try-consume]
/// When consumption fails, tokens remain unchanged.
proof fn consume_failure_preserves(tokens: u64, cost: u64)
    requires tokens < cost
    ensures try_consume_spec(tokens, cost).1 == tokens
{}

// r[verify rate-limit.try-consume]
/// try_consume is deterministic: same inputs yield same outputs.
proof fn consume_deterministic(tokens: u64, cost: u64)
    ensures try_consume_spec(tokens, cost) == try_consume_spec(tokens, cost)
{}

// --- is_within_burst ---

// r[depends rate-limit.within-burst]
/// Spec: tokens are within burst limit.
pub open spec fn is_within_burst_spec(tokens: u64, burst: u64) -> bool {
    tokens <= burst
}

// r[impl rate-limit.within-burst]
/// Returns true if tokens are within burst limit.
pub fn is_within_burst(tokens: u64, burst: u64) -> (result: bool)
    ensures result == is_within_burst_spec(tokens, burst)
{
    tokens <= burst
}

// r[verify rate-limit.within-burst]
/// After refill, tokens are always within burst.
proof fn refill_within_burst(tokens: u64, elapsed_ms: u64, rate_per_sec: u64, burst: u64)
    ensures is_within_burst_spec(tokens_after_refill_spec(tokens, elapsed_ms, rate_per_sec, burst), burst)
{}

// r[verify rate-limit.within-burst]
/// Boundary case: exactly burst tokens is within limits.
proof fn burst_boundary_within(burst: u64)
    ensures is_within_burst_spec(burst, burst)
{}

// --- tokens_to_wait_ms ---

// r[depends rate-limit.wait-time]
/// Spec: milliseconds to wait for sufficient tokens.
/// Uses ceiling division for conservative estimates.
pub open spec fn tokens_to_wait_ms_spec(
    tokens: u64,
    cost: u64,
    rate_per_sec: u64,
) -> u64
    recommends rate_per_sec > 0
{
    if tokens >= cost {
        0u64
    } else {
        let shortage_int = cost as int - tokens as int;
        if rate_per_sec == 0 {
            0u64
        } else {
            let result_int = (shortage_int * 1000 + rate_per_sec as int - 1) / rate_per_sec as int;
            if result_int > u64::MAX as int { u64::MAX } else { result_int as u64 }
        }
    }
}

// r[impl rate-limit.wait-time]
/// Returns milliseconds to wait until cost can be afforded.
/// If tokens >= cost, returns 0. Otherwise calculates ceiling division.
/// Requires `rate_per_sec` > 0 to avoid division by zero.
/// Uses u128 intermediate to avoid overflow splitting.
pub fn tokens_to_wait_ms(
    tokens: u64,
    cost: u64,
    rate_per_sec: u64,
) -> (result: u64)
    requires rate_per_sec > 0
    ensures result == tokens_to_wait_ms_spec(tokens, cost, rate_per_sec)
{
    if tokens >= cost {
        0
    } else {
        let shortage: u64 = cost - tokens;
        // Ceiling division: (shortage * 1000 + rate - 1) / rate.
        // Use u128 to avoid overflow: shortage * 1000 <= u64::MAX * 1000 << u128::MAX.
        assert((u64::MAX as int) * 1000 <= (u128::MAX as int)) by(nonlinear_arith);
        let numerator: u128 = shortage as u128 * 1000 + rate_per_sec as u128 - 1;
        let result_128: u128 = numerator / rate_per_sec as u128;
        if result_128 > u64::MAX as u128 {
            u64::MAX
        } else {
            result_128 as u64
        }
    }
}

// r[verify rate-limit.wait-time]
/// When tokens are sufficient, no wait is needed.
proof fn wait_time_sufficient(tokens: u64, cost: u64, rate_per_sec: u64)
    requires rate_per_sec > 0, tokens >= cost
    ensures tokens_to_wait_ms_spec(tokens, cost, rate_per_sec) == 0
{}

// r[verify rate-limit.wait-time]
/// When tokens are insufficient, wait time is positive.
proof fn wait_time_insufficient(tokens: u64, cost: u64, rate_per_sec: u64)
    requires rate_per_sec > 0, tokens < cost
    ensures tokens_to_wait_ms_spec(tokens, cost, rate_per_sec) > 0
{
    let s = cost as int - tokens as int;
    let r = rate_per_sec as int;
    let n = s * 1000 + r - 1;
    // n = s*1000 + r - 1 >= 1*1000 + 1 - 1 = 1000 >= r (when r <= 1000)
    // n = s*1000 + r - 1 >= s*1000 >= 1000 and n >= r - 1 + 1000 >= r
    // In general: n >= 1000 and n >= r - 1, and since s >= 1: n = s*1000+r-1 >= 1000+r-1 >= r
    assert(n >= r) by(nonlinear_arith)
        requires s >= 1int, r >= 1int, n == s * 1000 + r - 1
    ;
    vstd::arithmetic::div_mod::lemma_div_by_self(r);
    vstd::arithmetic::div_mod::lemma_div_is_ordered(r, n, r);
}

// --- cross-function properties ---

// r[verify rate-limit.refill]
// r[verify rate-limit.try-consume]
/// After refill, tokens are bounded by burst capacity.
proof fn refill_bounded_by_capacity(
    tokens: u64,
    elapsed_ms: u64,
    rate_per_sec: u64,
    burst: u64,
)
    requires rate_per_sec > 0, elapsed_ms > 0
    ensures ({
        let refilled = tokens_after_refill_spec(tokens, elapsed_ms, rate_per_sec, burst);
        refilled <= burst && (tokens <= burst ==> refilled >= tokens)
    })
{}

// r[verify rate-limit.wait-time]
// r[verify rate-limit.refill]
/// Wait time calculation is conservative: after waiting and refilling, cost can be consumed.
///
/// Proof strategy: show that the refill amount >= shortage (= cost - tokens).
/// The ceiling division property ensures wait_ms * rate >= shortage * 1000,
/// so wait_ms * rate / 1000 >= shortage. If the refill exceeds burst, the
/// capped result (burst) >= cost. Otherwise tokens + refill >= cost.
///
/// Requires the wait time to not saturate at u64::MAX. When wait_ms
/// saturates, u64::MAX milliseconds may be insufficient to earn the
/// shortage tokens (e.g., tokens=0, cost=u64::MAX, rate=1).
proof fn wait_enables_consumption(
    tokens: u64,
    cost: u64,
    rate_per_sec: u64,
    burst: u64,
)
    requires
        rate_per_sec > 0,
        tokens < cost,
        cost <= burst,
        tokens_to_wait_ms_spec(tokens, cost, rate_per_sec) < u64::MAX,
    ensures ({
        let wait_ms = tokens_to_wait_ms_spec(tokens, cost, rate_per_sec);
        let refilled = tokens_after_refill_spec(tokens, wait_ms, rate_per_sec, burst);
        try_consume_spec(refilled, cost).0 == true
    })
{
    // Proof outline:
    //   1. wait_int = ceil(shortage * 1000 / rate) — ceiling division
    //   2. wait_int * rate >= shortage * 1000 — ceiling property
    //   3. wait_int * rate / 1000 >= shortage — division monotonicity
    //   4. refill >= shortage, so tokens + refill >= cost
    //   5. If tokens + refill > burst: returns burst >= cost
    //      Otherwise: returns tokens + refill >= cost

    let s: int = cost as int - tokens as int;   // shortage >= 1
    let r: int = rate_per_sec as int;            // rate >= 1
    let n: int = s * 1000 + r - 1;              // ceiling div numerator
    let wait_int: int = n / r;                  // = ceil(s * 1000 / r)

    // Step 1: ceiling property — wait_int * r >= s * 1000
    // From fundamental_div_mod: n == wait_int * r + (n % r), 0 <= n % r < r
    // So wait_int * r = n - n%r >= n - (r-1) = s*1000 + r - 1 - r + 1 = s*1000
    vstd::arithmetic::div_mod::lemma_fundamental_div_mod(n, r);
    assert(wait_int * r >= s * 1000) by(nonlinear_arith)
        requires
            n == wait_int * r + (n % r),
            0 <= n % r < r,
            n == s * 1000 + r - 1,
            s >= 1int,
            r >= 1int,
    ;

    // Step 2: exact division — s * 1000 / 1000 == s
    assert(s * 1000 / 1000 == s) by(nonlinear_arith)
        requires s >= 1int
    ;

    // Step 3: division monotonicity — wait_int * r / 1000 >= s
    vstd::arithmetic::div_mod::lemma_div_is_ordered(s * 1000, wait_int * r, 1000);

    // Z3 connects: refill = wait_ms * rate / 1000 >= s = cost - tokens
    // tokens_after_refill returns min(tokens + refill, burst), both >= cost
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify rate-limit.refill]
    #[test]
    fn test_tokens_after_refill() {
        assert_eq!(tokens_after_refill(100, 2000, 10, 1000), 120);
        assert_eq!(tokens_after_refill(950, 10000, 10, 1000), 1000);
        assert_eq!(tokens_after_refill(500, 0, 10, 1000), 500);
        assert_eq!(tokens_after_refill(100, u64::MAX, 1000, 500), 500);
    }

    // r[verify rate-limit.try-consume]
    #[test]
    fn test_try_consume() {
        assert_eq!(try_consume(100, 30), (true, 70));
        assert_eq!(try_consume(20, 30), (false, 20));
        assert_eq!(try_consume(50, 50), (true, 0));
    }

    // r[verify rate-limit.within-burst]
    #[test]
    fn test_is_within_burst() {
        assert!(is_within_burst(500, 1000));
        assert!(is_within_burst(1000, 1000));
        assert!(!is_within_burst(1001, 1000));
    }

    // r[verify rate-limit.wait-time]
    #[test]
    fn test_tokens_to_wait_ms() {
        assert_eq!(tokens_to_wait_ms(100, 50, 10), 0);
        assert_eq!(tokens_to_wait_ms(5, 10, 10), 500);
        assert_eq!(tokens_to_wait_ms(0, 1, 3), 334);
    }

    proptest! {
        // r[verify rate-limit.refill]
        #[test]
        fn prop_refill_bounded(tokens: u64, elapsed_ms: u64, rate_per_sec: u64, burst: u64) {
            let result = tokens_after_refill(tokens, elapsed_ms, rate_per_sec, burst);
            prop_assert!(result <= burst);
        }

        // r[verify rate-limit.refill]
        #[test]
        fn prop_refill_monotonic(
            tokens: u64, t1: u64, t2: u64, rate_per_sec: u64, burst: u64
        ) {
            if t1 <= t2 {
                let r1 = tokens_after_refill(tokens, t1, rate_per_sec, burst);
                let r2 = tokens_after_refill(tokens, t2, rate_per_sec, burst);
                prop_assert!(r1 <= r2);
            }
        }

        // r[verify rate-limit.try-consume]
        #[test]
        fn prop_consume_conservation(tokens: u64, cost: u64) {
            let (success, remaining) = try_consume(tokens, cost);
            if success {
                prop_assert_eq!(remaining, tokens - cost);
            } else {
                prop_assert_eq!(remaining, tokens);
            }
        }

        // r[verify rate-limit.within-burst]
        #[test]
        fn prop_refill_within_burst(
            tokens: u64, elapsed_ms: u64, rate_per_sec: u64, burst: u64
        ) {
            let refilled = tokens_after_refill(tokens, elapsed_ms, rate_per_sec, burst);
            prop_assert!(is_within_burst(refilled, burst));
        }

        // r[verify rate-limit.wait-time]
        #[test]
        fn prop_wait_time_sufficient(tokens: u64, cost: u64, rate_per_sec in 1u64..=1000) {
            if tokens >= cost {
                prop_assert_eq!(tokens_to_wait_ms(tokens, cost, rate_per_sec), 0);
            }
        }

        // r[verify rate-limit.wait-time]
        #[test]
        fn prop_wait_time_insufficient(tokens: u64, cost: u64, rate_per_sec in 1u64..=1000) {
            if tokens < cost {
                prop_assert!(tokens_to_wait_ms(tokens, cost, rate_per_sec) > 0);
            }
        }

        // r[verify rate-limit.refill]
        // r[verify rate-limit.try-consume]
        #[test]
        fn prop_lifecycle(
            initial_tokens: u64,
            costs in proptest::collection::vec(1u64..=100, 1..=10),
            rate_per_sec in 1u64..=100,
            burst in 100u64..=1000,
            time_steps in proptest::collection::vec(0u64..=5000, 1..=10),
        ) {
            let mut tokens = initial_tokens.min(burst);
            for (i, &cost) in costs.iter().enumerate() {
                if let Some(&time_step) = time_steps.get(i) {
                    tokens = tokens_after_refill(tokens, time_step, rate_per_sec, burst);
                    prop_assert!(is_within_burst(tokens, burst));
                }
                let (success, new_tokens) = try_consume(tokens, cost);
                if success {
                    prop_assert_eq!(new_tokens, tokens - cost);
                } else {
                    prop_assert_eq!(new_tokens, tokens);
                }
                tokens = new_tokens;
            }
        }
    }
}
