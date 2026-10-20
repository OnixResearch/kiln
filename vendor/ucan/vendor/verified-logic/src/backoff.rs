//! Verified exponential backoff with cap and jitter bounds.
//!
//! Computes retry delays as `min(base * 2^attempt, cap)` using saturating
//! left-shift to prevent overflow. Jitter range is `[0, backoff]`.

#![allow(
    unused_assignments,
    clippy::single_match,
    clippy::collapsible_match,
    clippy::single_match_else
)]

#[cfg(verus_keep_ghost)]
use vstd::arithmetic::mul::lemma_mul_inequality;
#[cfg(verus_keep_ghost)]
use vstd::arithmetic::mul::lemma_mul_is_associative;
#[cfg(verus_keep_ghost)]
use vstd::arithmetic::mul::lemma_mul_is_commutative;
use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::ordering::min_spec;

verus! {

// --- compute_backoff ---

// r[depends backoff.compute-backoff]
/// Spec: exponential backoff capped at `cap`.
/// Uses saturating shift: if attempt >= 64, the shift overflows to
/// u64::MAX before capping.
pub open spec fn compute_backoff_spec(base: u64, attempt: u64, cap: u64) -> u64 {
    if base == 0 {
        0u64
    } else {
        let uncapped: int =
            if attempt >= 64 {
                u64::MAX as int
            } else if base as int * pow2(attempt as nat) > u64::MAX as int {
                u64::MAX as int
            } else {
                base as int * pow2(attempt as nat)
            };
        min_spec(uncapped as u64, cap)
    }
}

/// Power of two for spec use.
pub open spec fn pow2(n: nat) -> int
    decreases n
{
    if n == 0 { 1int } else { 2 * pow2((n - 1) as nat) }
}

/// Helper: pow2(n+1) == 2 * pow2(n).
proof fn pow2_step(n: nat)
    ensures pow2(n + 1) == 2 * pow2(n)
{
    // Directly from the definition: pow2(n+1) = 2 * pow2((n+1-1)) = 2 * pow2(n)
}

/// Helper: if base * pow2(i) <= u64::MAX, then base * pow2(i) >= 0.
proof fn pow2_mul_nonneg(base: u64, n: nat)
    ensures base as int * pow2(n) >= 0
{
    backoff_pow2_nonneg(n);
}

// r[impl backoff.compute-backoff]
/// Computes the exponential backoff delay for the given attempt.
///
/// Returns `min(base * 2^attempt, cap)`, with overflow detection.
pub fn compute_backoff(base: u64, attempt: u64, cap: u64) -> (result: u64)
    ensures
        result <= cap,
        result == compute_backoff_spec(base, attempt, cap),
{
    if base == 0 {
        return 0;
    }
    // Compute base * 2^attempt with overflow detection.
    // Cap iterations at 63 since 2^64 always overflows u64.
    let capped_attempt: u64 = if attempt < 63 { attempt } else { 63 };
    let mut shifted: u64 = base;
    let mut overflowed: bool = false;
    let mut i: u64 = 0;
    proof {
        assert(pow2(0nat) == 1int);
        assert(shifted as int == base as int * pow2(0nat));
    }
    while i < capped_attempt
        invariant
            0 <= i <= capped_attempt,
            capped_attempt <= 63,
            base > 0,
            attempt >= capped_attempt,
            capped_attempt == (if attempt < 63 { attempt } else { 63u64 }),
            !overflowed ==> shifted as int == base as int * pow2(i as nat),
            overflowed ==> shifted == u64::MAX,
            overflowed ==> base as int * pow2(i as nat) > u64::MAX as int,
        decreases capped_attempt - i,
    {
        proof {
            pow2_step(i as nat);
            backoff_pow2_nonneg(i as nat);
        }
        if overflowed {
            proof {
                assert(pow2((i + 1) as nat) == 2 * pow2(i as nat));
                backoff_pow2_nonneg(i as nat);
                assert(pow2((i + 1) as nat) >= pow2(i as nat));
                // Use vstd: pow2(i) <= pow2(i+1) && base >= 0 ==> pow2(i)*base <= pow2(i+1)*base
                lemma_mul_inequality(pow2(i as nat), pow2((i + 1) as nat), base as int);
                // Now we have pow2(i)*base <= pow2(i+1)*base
                // By commutativity: base*pow2(i) <= base*pow2(i+1)
                lemma_mul_is_commutative(pow2(i as nat), base as int);
                lemma_mul_is_commutative(pow2((i + 1) as nat), base as int);
            }
            i = i + 1;
        } else {
            // shifted == base * pow2(i), try to double
            let doubled = shifted.checked_mul(2);
            match doubled {
                Some(v) => {
                    proof {
                        assert(v as int == shifted as int * 2);
                        assert(v as int == base as int * pow2(i as nat) * 2);
                        // base * pow2(i+1) = base * (2 * pow2(i)) = (base * 2) * pow2(i)
                        assert(pow2((i + 1) as nat) == 2 * pow2(i as nat));
                        lemma_mul_is_associative(base as int, 2int, pow2(i as nat));
                        // now: base * (2 * pow2(i)) == (base * 2) * pow2(i)
                        lemma_mul_is_commutative(base as int, 2int);
                        // now: base * 2 == 2 * base
                        // so: base * pow2(i+1) == (2 * base) * pow2(i) == 2 * (base * pow2(i))
                        lemma_mul_is_associative(2int, base as int, pow2(i as nat));
                    }
                    shifted = v;
                },
                None => {
                    proof {
                        assert(shifted as int * 2 > u64::MAX as int);
                        assert(base as int * pow2(i as nat) * 2 > u64::MAX as int);
                        assert(pow2((i + 1) as nat) == 2 * pow2(i as nat));
                        lemma_mul_is_associative(base as int, 2int, pow2(i as nat));
                        lemma_mul_is_commutative(base as int, 2int);
                        lemma_mul_is_associative(2int, base as int, pow2(i as nat));
                    }
                    overflowed = true;
                    shifted = u64::MAX;
                },
            }
            i = i + 1;
        }
    }

    // If attempt >= 64 and base > 0, the shift certainly overflows
    if attempt >= 64 {
        shifted = u64::MAX;
        overflowed = true;
    }

    proof {
        if attempt >= 64 {
            assert(compute_backoff_spec(base, attempt, cap) == min_spec(u64::MAX, cap));
        } else if overflowed {
            assert(base as int * pow2(attempt as nat) > u64::MAX as int);
            assert(compute_backoff_spec(base, attempt, cap) == min_spec(u64::MAX, cap));
        } else {
            assert(i == capped_attempt);
            assert(capped_attempt == attempt);
            assert(shifted as int == base as int * pow2(attempt as nat));
            backoff_pow2_nonneg(attempt as nat);
            assert(base as int * pow2(attempt as nat) >= 0);
            assert(base as int * pow2(attempt as nat) <= u64::MAX as int);
            assert(compute_backoff_spec(base, attempt, cap) == min_spec(shifted, cap));
        }
    }
    shifted.min(cap)
}

// --- jitter_range ---

// r[depends backoff.jitter-range]
/// Spec: the jitter window is [0, backoff_value].
pub open spec fn jitter_range_spec(backoff_value: u64) -> (u64, u64) {
    (0u64, backoff_value)
}

// r[impl backoff.jitter-range]
/// Returns the jitter bounds for a given backoff value.
pub fn jitter_range(backoff_value: u64) -> (result: (u64, u64))
    ensures result == jitter_range_spec(backoff_value)
{
    (0, backoff_value)
}

// --- Safety proofs ---

// r[verify backoff.compute-backoff]
/// Backoff is always bounded by cap.
proof fn backoff_bounded(base: u64, attempt: u64, cap: u64)
    ensures compute_backoff_spec(base, attempt, cap) <= cap
{}

// r[verify backoff.compute-backoff]
/// Backoff with attempt 0 returns min(base, cap).
proof fn backoff_attempt_zero(base: u64, cap: u64)
    ensures compute_backoff_spec(base, 0, cap) == min_spec(base, cap)
{
    assert(pow2(0nat) == 1int);
}

// r[verify backoff.compute-backoff]
/// Backoff with base 0 is always 0.
proof fn backoff_base_zero(attempt: u64, cap: u64)
    ensures compute_backoff_spec(0, attempt, cap) == 0
{}

/// Helper: pow2 is always positive.
proof fn backoff_pow2_nonneg(n: nat)
    ensures pow2(n) >= 1
    decreases n
{
    if n > 0 {
        backoff_pow2_nonneg((n - 1) as nat);
    }
}

// r[verify backoff.jitter-range]
/// Jitter lower bound is 0.
proof fn jitter_lower_bound(backoff_value: u64)
    ensures jitter_range_spec(backoff_value).0 == 0
{}

// r[verify backoff.jitter-range]
/// Jitter upper bound equals the backoff value.
proof fn jitter_upper_bound(backoff_value: u64)
    ensures jitter_range_spec(backoff_value).1 == backoff_value
{}

// r[verify backoff.jitter-range]
/// Jitter range is well-formed: lower <= upper.
proof fn jitter_range_valid(backoff_value: u64)
    ensures jitter_range_spec(backoff_value).0 <= jitter_range_spec(backoff_value).1
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify backoff.compute-backoff]
    #[test]
    fn test_compute_backoff() {
        assert_eq!(compute_backoff(100, 0, 10000), 100);
        assert_eq!(compute_backoff(100, 1, 10000), 200);
        assert_eq!(compute_backoff(100, 2, 10000), 400);
        assert_eq!(compute_backoff(100, 3, 10000), 800);
        assert_eq!(compute_backoff(100, 7, 10000), 10000); // capped
        assert_eq!(compute_backoff(100, 63, 10000), 10000); // capped (overflow)
        assert_eq!(compute_backoff(100, 64, 10000), 10000); // attempt >= 64
        assert_eq!(compute_backoff(0, 10, 1000), 0);
    }

    // r[verify backoff.jitter-range]
    #[test]
    fn test_jitter_range() {
        assert_eq!(jitter_range(500), (0, 500));
        assert_eq!(jitter_range(0), (0, 0));
    }

    proptest! {
        // r[verify backoff.compute-backoff]
        #[test]
        fn prop_backoff_bounded(base: u64, attempt: u64, cap: u64) {
            prop_assert!(compute_backoff(base, attempt, cap) <= cap);
        }

        // r[verify backoff.compute-backoff]
        #[test]
        fn prop_backoff_base_zero(attempt: u64, cap: u64) {
            prop_assert_eq!(compute_backoff(0, attempt, cap), 0);
        }

        // r[verify backoff.compute-backoff]
        #[test]
        fn prop_backoff_monotonic_attempt(
            base in 1u64..=1000,
            a1 in 0u64..32,
            a2 in 0u64..32,
            cap: u64,
        ) {
            if a1 <= a2 {
                prop_assert!(compute_backoff(base, a1, cap) <= compute_backoff(base, a2, cap));
            }
        }

        // r[verify backoff.jitter-range]
        #[test]
        fn prop_jitter_range_valid(backoff_value: u64) {
            let (lo, hi) = jitter_range(backoff_value);
            prop_assert!(lo <= hi);
            prop_assert_eq!(lo, 0);
            prop_assert_eq!(hi, backoff_value);
        }
    }
}
