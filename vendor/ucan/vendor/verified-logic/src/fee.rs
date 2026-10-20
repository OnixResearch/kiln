// Verus proofs guarantee u128→u64 casts are in bounds (fee <= amount)
#![allow(clippy::cast_possible_truncation)]

//! Verified fee computation with basis-point precision.
//!
//! Computes proportional fees using basis points (1 bp = 0.01%).
//! A rate of 250 basis points means 2.5%. All arithmetic uses
//! checked/saturating operations — no overflow, no negative results.
//! Conservation proofs guarantee fee + net = original amount.

use vstd::prelude::*;

verus! {

/// Maximum fee rate: 10000 basis points = 100%.
pub const MAX_RATE_BPS: u64 = 10000;

// --- compute_fee ---

// r[depends fee.compute]
/// Spec: fee = floor(amount * rate_bps / 10000).
pub open spec fn compute_fee_spec(amount: u64, rate_bps: u64) -> int {
    (amount as int * rate_bps as int) / 10000
}

// r[impl fee.compute]
/// Compute fee in basis points. rate_bps must be <= 10000.
pub fn compute_fee(amount: u64, rate_bps: u64) -> (result: u64)
    requires rate_bps <= MAX_RATE_BPS
    ensures result as int == compute_fee_spec(amount, rate_bps)
{
    proof {
        // Prove product fits in u128: amount * rate_bps <= u64::MAX * 10000 < u128::MAX
        assert((amount as int) * (rate_bps as int) <= (u64::MAX as int) * 10000) by(nonlinear_arith)
            requires amount <= u64::MAX, rate_bps <= 10000u64;
        assert((u64::MAX as int) * 10000 <= u128::MAX as int) by(nonlinear_arith);
        // Prove result fits in u64: fee = product / 10000 <= amount <= u64::MAX
        assert((amount as int) * (rate_bps as int) / 10000 <= (amount as int)) by(nonlinear_arith)
            requires rate_bps <= 10000u64, amount >= 0u64;
    }
    let wide = amount as u128 * rate_bps as u128;
    (wide / 10000) as u64
}

// --- compute_net ---

// r[depends fee.net]
/// Spec: net = amount - fee.
pub open spec fn compute_net_spec(amount: u64, rate_bps: u64) -> int {
    amount as int - compute_fee_spec(amount, rate_bps)
}

// r[impl fee.net]
/// Compute net amount after fee deduction. rate_bps must be <= 10000.
pub fn compute_net(amount: u64, rate_bps: u64) -> (result: u64)
    requires rate_bps <= MAX_RATE_BPS
    ensures result as int == compute_net_spec(amount, rate_bps)
{
    let fee = compute_fee(amount, rate_bps);
    proof {
        fee_bounded(amount, rate_bps);
    }
    amount - fee
}

// --- is_valid_rate ---

// r[depends fee.valid-rate]
/// Spec: rate is within [0, 10000] basis points.
pub open spec fn is_valid_rate_spec(rate_bps: u64) -> bool {
    rate_bps <= MAX_RATE_BPS as u64
}

// r[impl fee.valid-rate]
/// Returns true if the rate is valid (0-10000 basis points).
pub fn is_valid_rate(rate_bps: u64) -> (result: bool)
    ensures result == is_valid_rate_spec(rate_bps)
{
    rate_bps <= MAX_RATE_BPS
}

// --- split_with_fee ---

// r[depends fee.split]
/// Spec: split amount into (fee, net) pair.
pub open spec fn split_with_fee_spec(amount: u64, rate_bps: u64) -> (int, int) {
    (compute_fee_spec(amount, rate_bps), compute_net_spec(amount, rate_bps))
}

// r[impl fee.split]
/// Split amount into (fee, net) pair. fee + net = amount.
pub fn split_with_fee(amount: u64, rate_bps: u64) -> (result: (u64, u64))
    requires rate_bps <= MAX_RATE_BPS
    ensures
        result.0 as int == compute_fee_spec(amount, rate_bps),
        result.1 as int == compute_net_spec(amount, rate_bps),
{
    let fee = compute_fee(amount, rate_bps);
    proof {
        fee_bounded(amount, rate_bps);
    }
    (fee, amount - fee)
}

// --- tiered_fee ---

// r[depends fee.tiered]
/// Spec: fee with a minimum floor.
pub open spec fn tiered_fee_spec(amount: u64, rate_bps: u64, min_fee: u64) -> int {
    let proportional = compute_fee_spec(amount, rate_bps);
    if proportional >= min_fee as int {
        proportional
    } else {
        if min_fee as int <= amount as int {
            min_fee as int
        } else {
            amount as int
        }
    }
}

// r[impl fee.tiered]
/// Compute fee with a minimum floor, capped at amount.
pub fn tiered_fee(amount: u64, rate_bps: u64, min_fee: u64) -> (result: u64)
    requires rate_bps <= MAX_RATE_BPS
    ensures result as int == tiered_fee_spec(amount, rate_bps, min_fee)
{
    let proportional = compute_fee(amount, rate_bps);
    if proportional >= min_fee {
        proportional
    } else if min_fee <= amount {
        min_fee
    } else {
        amount
    }
}

// --- Proofs ---

// r[impl fee.conservation]
// r[verify fee.conservation]
/// Fee + net = amount. The fundamental conservation property.
pub proof fn fee_net_conservation(amount: u64, rate_bps: u64)
    requires rate_bps <= MAX_RATE_BPS
    ensures
        compute_fee_spec(amount, rate_bps)
        + compute_net_spec(amount, rate_bps)
        == amount as int,
{}

// r[verify fee.compute]
/// Fee is bounded by amount: fee <= amount.
pub proof fn fee_bounded(amount: u64, rate_bps: u64)
    requires rate_bps <= MAX_RATE_BPS
    ensures 0 <= compute_fee_spec(amount, rate_bps) <= amount as int
{
    assert(amount as int * rate_bps as int >= 0) by(nonlinear_arith)
        requires amount >= 0u64, rate_bps >= 0u64;
    assert(amount as int * rate_bps as int <= amount as int * 10000) by(nonlinear_arith)
        requires rate_bps <= 10000u64;
    // fee = (amount * rate_bps) / 10000 <= (amount * 10000) / 10000 = amount
    vstd::arithmetic::div_mod::lemma_div_is_ordered(
        amount as int * rate_bps as int,
        amount as int * 10000,
        10000,
    );
}

// r[verify fee.compute]
/// Zero amount produces zero fee.
proof fn zero_amount_zero_fee(rate_bps: u64)
    requires rate_bps <= MAX_RATE_BPS
    ensures compute_fee_spec(0, rate_bps) == 0
{}

// r[verify fee.compute]
/// Zero rate produces zero fee.
proof fn zero_rate_zero_fee(amount: u64)
    ensures compute_fee_spec(amount, 0) == 0
{}

// r[verify fee.compute]
/// Full rate (10000 bps = 100%) takes the entire amount.
proof fn full_rate_takes_all(amount: u64)
    ensures compute_fee_spec(amount, MAX_RATE_BPS) == amount as int
{
    // amount * 10000 / 10000 = amount
    vstd::arithmetic::div_mod::lemma_fundamental_div_mod(
        amount as int * 10000,
        10000,
    );
    assert(amount as int * 10000 == 10000 * (amount as int * 10000 / 10000)
        + (amount as int * 10000) % 10000);
    assert((amount as int * 10000) % 10000 == 0) by(nonlinear_arith)
        requires amount >= 0u64;
}

// r[verify fee.net]
/// Zero rate means net = amount (no fee taken).
proof fn zero_rate_full_net(amount: u64)
    ensures compute_net_spec(amount, 0) == amount as int
{}

// r[verify fee.net]
/// Full rate means net = 0 (entire amount is fee).
proof fn full_rate_zero_net(amount: u64)
    ensures compute_net_spec(amount, MAX_RATE_BPS) == 0
{
    full_rate_takes_all(amount);
}

// r[verify fee.compute]
/// Fee is monotonic in amount: larger amount → larger or equal fee.
pub proof fn fee_monotonic_amount(a1: u64, a2: u64, rate_bps: u64)
    requires rate_bps <= MAX_RATE_BPS, a1 <= a2
    ensures compute_fee_spec(a1, rate_bps) <= compute_fee_spec(a2, rate_bps)
{
    assert(a1 as int * rate_bps as int <= a2 as int * rate_bps as int) by(nonlinear_arith)
        requires a1 <= a2, rate_bps >= 0u64;
    vstd::arithmetic::div_mod::lemma_div_is_ordered(
        a1 as int * rate_bps as int,
        a2 as int * rate_bps as int,
        10000,
    );
}

// r[verify fee.compute]
/// Fee is monotonic in rate: higher rate → larger or equal fee.
pub proof fn fee_monotonic_rate(amount: u64, r1: u64, r2: u64)
    requires r1 <= r2, r2 <= MAX_RATE_BPS
    ensures compute_fee_spec(amount, r1) <= compute_fee_spec(amount, r2)
{
    assert(amount as int * r1 as int <= amount as int * r2 as int) by(nonlinear_arith)
        requires r1 <= r2, amount >= 0u64;
    vstd::arithmetic::div_mod::lemma_div_is_ordered(
        amount as int * r1 as int,
        amount as int * r2 as int,
        10000,
    );
}

// r[verify fee.net]
/// Net is non-negative.
proof fn net_nonneg(amount: u64, rate_bps: u64)
    requires rate_bps <= MAX_RATE_BPS
    ensures compute_net_spec(amount, rate_bps) >= 0
{
    fee_bounded(amount, rate_bps);
}

// r[verify fee.tiered]
/// Tiered fee is at least min_fee when amount >= min_fee.
proof fn tiered_at_least_min(amount: u64, rate_bps: u64, min_fee: u64)
    requires rate_bps <= MAX_RATE_BPS, min_fee as int <= amount as int
    ensures tiered_fee_spec(amount, rate_bps, min_fee) >= min_fee as int
{}

// r[verify fee.tiered]
/// Tiered fee never exceeds amount.
proof fn tiered_bounded(amount: u64, rate_bps: u64, min_fee: u64)
    requires rate_bps <= MAX_RATE_BPS
    ensures tiered_fee_spec(amount, rate_bps, min_fee) <= amount as int
{
    fee_bounded(amount, rate_bps);
}

// r[verify fee.split]
/// Split conservation: fee + net = amount.
proof fn split_conservation(amount: u64, rate_bps: u64)
    requires rate_bps <= MAX_RATE_BPS
    ensures ({
        let (f, n) = split_with_fee_spec(amount, rate_bps);
        f + n == amount as int
    })
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify fee.compute]
    #[test]
    fn test_compute_fee_basic() {
        // 2.5% of 10000 = 250
        assert_eq!(compute_fee(10000, 250), 250);
        // 10% of 1000 = 100
        assert_eq!(compute_fee(1000, 1000), 100);
        // 100% = full amount
        assert_eq!(compute_fee(500, 10000), 500);
        // 0% = nothing
        assert_eq!(compute_fee(1000, 0), 0);
        // 0 amount
        assert_eq!(compute_fee(0, 5000), 0);
        // Rounding: 1 bp of 99 = floor(99 * 1 / 10000) = 0
        assert_eq!(compute_fee(99, 1), 0);
        // Rounding: 1 bp of 10000 = 1
        assert_eq!(compute_fee(10000, 1), 1);
    }

    // r[verify fee.net]
    #[test]
    fn test_compute_net_basic() {
        assert_eq!(compute_net(10000, 250), 9750);
        assert_eq!(compute_net(1000, 1000), 900);
        assert_eq!(compute_net(500, 10000), 0);
        assert_eq!(compute_net(1000, 0), 1000);
    }

    // r[verify fee.valid-rate]
    #[test]
    fn test_valid_rate() {
        assert!(is_valid_rate(0));
        assert!(is_valid_rate(5000));
        assert!(is_valid_rate(10000));
        assert!(!is_valid_rate(10001));
        assert!(!is_valid_rate(u64::MAX));
    }

    // r[verify fee.split]
    #[test]
    fn test_split_with_fee() {
        let (fee, net) = split_with_fee(10000, 250);
        assert_eq!(fee, 250);
        assert_eq!(net, 9750);
        assert_eq!(fee + net, 10000); // conservation
    }

    // r[verify fee.tiered]
    #[test]
    fn test_tiered_fee() {
        // Proportional exceeds min: use proportional
        assert_eq!(tiered_fee(10000, 500, 100), 500); // 5% of 10000 = 500 > 100
        // Proportional below min: use min
        assert_eq!(tiered_fee(100, 100, 50), 50); // 1% of 100 = 1 < 50
        // Min exceeds amount: cap at amount
        assert_eq!(tiered_fee(10, 100, 50), 10); // min=50 > amount=10
    }

    proptest! {
        // r[verify fee.conservation]
        #[test]
        fn prop_fee_net_conservation(amount: u64, rate_bps in 0..=10000u64) {
            let fee = compute_fee(amount, rate_bps);
            let net = compute_net(amount, rate_bps);
            prop_assert_eq!(fee as u128 + net as u128, amount as u128);
        }

        // r[verify fee.compute]
        #[test]
        fn prop_fee_bounded(amount: u64, rate_bps in 0..=10000u64) {
            let fee = compute_fee(amount, rate_bps);
            prop_assert!(fee <= amount);
        }

        // r[verify fee.compute]
        #[test]
        fn prop_fee_monotonic_amount(
            a1 in 0..u64::MAX / 2,
            rate_bps in 0..=10000u64,
        ) {
            let a2 = a1 + 1;
            let f1 = compute_fee(a1, rate_bps);
            let f2 = compute_fee(a2, rate_bps);
            prop_assert!(f1 <= f2);
        }

        // r[verify fee.compute]
        #[test]
        fn prop_fee_monotonic_rate(
            amount: u64,
            r1 in 0..10000u64,
        ) {
            let r2 = r1 + 1;
            let f1 = compute_fee(amount, r1);
            let f2 = compute_fee(amount, r2);
            prop_assert!(f1 <= f2);
        }

        // r[verify fee.compute]
        #[test]
        fn prop_fee_oracle(amount: u64, rate_bps in 0..=10000u64) {
            let fee = compute_fee(amount, rate_bps);
            let oracle = (amount as u128 * rate_bps as u128) / 10000;
            prop_assert_eq!(fee as u128, oracle);
        }

        // r[verify fee.split]
        #[test]
        fn prop_split_conservation(amount: u64, rate_bps in 0..=10000u64) {
            let (fee, net) = split_with_fee(amount, rate_bps);
            prop_assert_eq!(fee as u128 + net as u128, amount as u128);
        }

        // r[verify fee.tiered]
        #[test]
        fn prop_tiered_bounded(amount: u64, rate_bps in 0..=10000u64, min_fee: u64) {
            let fee = tiered_fee(amount, rate_bps, min_fee);
            prop_assert!(fee <= amount);
        }

        // r[verify fee.tiered]
        #[test]
        fn prop_tiered_at_least_min(
            amount in 1..u64::MAX / 2,
            rate_bps in 0..=10000u64,
            min_fee in 0..u64::MAX / 2,
        ) {
            if min_fee <= amount {
                let fee = tiered_fee(amount, rate_bps, min_fee);
                prop_assert!(fee >= min_fee);
            }
        }
    }
}
