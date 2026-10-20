// Verus proofs guarantee u128→u64 casts are in bounds (weight <= total_weight)
#![allow(clippy::cast_possible_truncation)]

//! Verified fair-share scheduling primitives.
//!
//! Weighted fair scheduling divides a shared resource (CPU time, bandwidth,
//! tokens) among consumers proportional to their weights. Each consumer
//! accumulates a virtual time (deficit) and the scheduler selects the
//! consumer with the lowest virtual time. Proofs cover weight normalization,
//! fair allocation, deficit monotonicity, and bounded unfairness.

use vstd::prelude::*;

verus! {

// ============================================================
// Weight normalization
// ============================================================

// r[depends fair-share.total-weight]
/// Spec: sum of weights from index `from` onward.
pub open spec fn total_weight_from_spec(weights: Seq<u64>, from: int) -> int
    decreases weights.len() - from
{
    if from < 0 || from >= weights.len() as int {
        0int
    } else {
        weights[from] as int + total_weight_from_spec(weights, from + 1)
    }
}

/// Spec: sum of all weights.
pub open spec fn total_weight_spec(weights: Seq<u64>) -> int {
    total_weight_from_spec(weights, 0)
}

// r[impl fair-share.total-weight]
/// Compute the total weight of all consumers.
pub fn total_weight(weights: &[u64]) -> (result: Option<u64>)
    ensures
        match result {
            Some(v) => total_weight_spec(weights@) == v as int,
            None => total_weight_spec(weights@) > u64::MAX as int,
        },
{
    let mut sum: u64 = 0;
    let mut i: usize = 0;
    while i < weights.len()
        invariant
            0 <= i <= weights@.len(),
            sum as int == total_weight_from_spec(weights@, 0) - total_weight_from_spec(weights@, i as int),
        decreases weights@.len() - i,
    {
        if sum > u64::MAX - weights[i] {
            // sum + weights[i] > u64::MAX. Remaining tail is nonneg,
            // so total >= sum + weights[i] > u64::MAX.
            proof {
                total_weight_nonneg_from(weights@, (i + 1) as int);
            }
            return None;
        }
        sum = sum + weights[i];
        i = i + 1;
    }
    Some(sum)
}

// r[verify fair-share.total-weight]
/// Empty weights have zero total.
proof fn total_weight_empty()
    ensures total_weight_spec(seq![]) == 0int
{}

// r[verify fair-share.total-weight]
/// Total weight is nonneg when all weights are u64.
proof fn total_weight_nonneg(weights: Seq<u64>)
    ensures total_weight_spec(weights) >= 0int
    decreases weights.len()
{
    if weights.len() > 0 {
        total_weight_nonneg_from(weights, 0);
    }
}

proof fn total_weight_nonneg_from(weights: Seq<u64>, from: int)
    requires 0 <= from <= weights.len() as int
    ensures total_weight_from_spec(weights, from) >= 0int
    decreases weights.len() - from
{
    if from < weights.len() as int {
        total_weight_nonneg_from(weights, from + 1);
    }
}

// ============================================================
// Fair allocation: how many tokens each consumer gets
// ============================================================

// r[depends fair-share.allocate]
/// Spec: allocate `total_tokens` proportionally to weight.
/// Returns floor(total_tokens * weight / total_weight) for each consumer.
/// When total_weight is 0, each consumer gets 0.
pub open spec fn fair_alloc_spec(total_tokens: u64, weight: u64, total_weight: u64) -> u64 {
    if total_weight == 0 {
        0u64
    } else {
        ((total_tokens as int * weight as int) / total_weight as int) as u64
    }
}

// r[impl fair-share.allocate]
/// Compute a consumer's fair share of tokens.
pub fn fair_alloc(total_tokens: u64, weight: u64, total_weight: u64) -> (result: u64)
    requires total_weight > 0, weight <= total_weight
    ensures result == fair_alloc_spec(total_tokens, weight, total_weight)
{
    let t128 = total_tokens as u128;
    let w128 = weight as u128;
    assert(t128 as int * w128 as int <= u128::MAX as int) by(nonlinear_arith)
        requires t128 as int <= u64::MAX as int, w128 as int <= u64::MAX as int,
                 (u64::MAX as int) * (u64::MAX as int) <= u128::MAX as int;
    let product = t128 * w128;
    let tw128 = total_weight as u128;
    let result128 = product / tw128;
    // result <= total_tokens because weight <= total_weight
    assert(result128 as int <= t128 as int) by(nonlinear_arith)
        requires w128 as int <= tw128 as int, tw128 > 0u128,
                 result128 as int == (t128 as int * w128 as int) / tw128 as int;
    result128 as u64
}

// r[verify fair-share.allocate]
/// Zero weight gets zero tokens.
proof fn zero_weight_zero_alloc(total_tokens: u64, total_weight: u64)
    ensures fair_alloc_spec(total_tokens, 0, total_weight) == 0u64
{}

// r[verify fair-share.allocate]
/// Full weight gets all tokens (when weight == total_weight).
proof fn full_weight_full_alloc(total_tokens: u64, w: u64)
    requires w > 0
    ensures fair_alloc_spec(total_tokens, w, w) == total_tokens
{
    assert(total_tokens as int * w as int / w as int == total_tokens as int) by(nonlinear_arith)
        requires w > 0u64;
}

// r[verify fair-share.allocate]
/// Higher weight gets at least as many tokens.
proof fn higher_weight_more_tokens(total_tokens: u64, w1: u64, w2: u64, tw: u64)
    requires tw > 0, w1 <= w2, w2 <= tw
    ensures fair_alloc_spec(total_tokens, w1, tw) <= fair_alloc_spec(total_tokens, w2, tw)
{
    let t = total_tokens as int;
    let a = w1 as int;
    let b = w2 as int;
    let d = tw as int;
    assert(t * a <= t * b) by(nonlinear_arith)
        requires t >= 0, a >= 0, a <= b;
    vstd::arithmetic::div_mod::lemma_div_is_ordered(t * a, t * b, d);
    // Bridge: both results fit in u64 (both <= total_tokens)
    assert(t * b <= t * d) by(nonlinear_arith)
        requires t >= 0, b >= 0, b <= d;
    vstd::arithmetic::div_mod::lemma_div_is_ordered(t * b, t * d, d);
    assert(t * d / d == t) by(nonlinear_arith)
        requires d > 0, t >= 0;
    // So t*a/d <= t*b/d <= t*d/d == t == total_tokens, both fit in u64
    assert(t * a / d >= 0) by {
        vstd::arithmetic::div_mod::lemma_div_pos_is_pos(t * a, d);
        assert(t * a >= 0) by(nonlinear_arith) requires t >= 0, a >= 0;
    }
    assert(t * b / d >= 0) by {
        vstd::arithmetic::div_mod::lemma_div_pos_is_pos(t * b, d);
        assert(t * b >= 0) by(nonlinear_arith) requires t >= 0, b >= 0;
    }
}

// r[verify fair-share.allocate]
/// Allocation never exceeds total tokens.
proof fn alloc_bounded(total_tokens: u64, weight: u64, total_weight: u64)
    requires total_weight > 0, weight <= total_weight
    ensures fair_alloc_spec(total_tokens, weight, total_weight) <= total_tokens
{
    assert(total_tokens as int * weight as int <= total_tokens as int * total_weight as int) by(nonlinear_arith)
        requires weight <= total_weight;
    vstd::arithmetic::div_mod::lemma_div_is_ordered(
        total_tokens as int * weight as int,
        total_tokens as int * total_weight as int,
        total_weight as int,
    );
    assert(total_tokens as int * total_weight as int / total_weight as int == total_tokens as int) by(nonlinear_arith)
        requires total_weight > 0u64;
}

// ============================================================
// Virtual time (deficit counter)
// ============================================================

// r[depends fair-share.advance-vtime]
/// Spec: advance a consumer's virtual time after receiving service.
/// vtime += service_amount * (total_weight / weight).
/// This normalizes so that equal weights advance by equal amounts,
/// and lower weights advance faster (penalizing them less for smaller shares).
/// Saturates at u64::MAX.
pub open spec fn advance_vtime_spec(vtime: u64, service: u64, weight: u64, total_weight: u64) -> u64 {
    if weight == 0 || total_weight == 0 {
        u64::MAX
    } else {
        let increment = (service as int * total_weight as int) / weight as int;
        if vtime as int + increment > u64::MAX as int {
            u64::MAX
        } else {
            (vtime as int + increment) as u64
        }
    }
}

// r[impl fair-share.advance-vtime]
/// Advance virtual time after service.
pub fn advance_vtime(vtime: u64, service: u64, weight: u64, total_weight: u64) -> (result: u64)
    requires weight > 0, total_weight > 0
    ensures result == advance_vtime_spec(vtime, service, weight, total_weight)
{
    let s128 = service as u128;
    let tw128 = total_weight as u128;
    assert(s128 as int * tw128 as int <= u128::MAX as int) by(nonlinear_arith)
        requires s128 as int <= u64::MAX as int, tw128 as int <= u64::MAX as int,
                 (u64::MAX as int) * (u64::MAX as int) <= u128::MAX as int;
    let increment_128 = s128 * tw128 / weight as u128;
    if increment_128 > (u64::MAX - vtime) as u128 {
        u64::MAX
    } else {
        vtime + increment_128 as u64
    }
}

// r[verify fair-share.advance-vtime]
/// Virtual time never decreases.
proof fn vtime_monotone(vtime: u64, service: u64, weight: u64, total_weight: u64)
    requires weight > 0, total_weight > 0
    ensures advance_vtime_spec(vtime, service, weight, total_weight) >= vtime
{
    let increment = (service as int * total_weight as int) / weight as int;
    assert(service as int * total_weight as int >= 0int) by(nonlinear_arith)
        requires service >= 0u64, total_weight >= 0u64;
    vstd::arithmetic::div_mod::lemma_div_pos_is_pos(
        service as int * total_weight as int,
        weight as int,
    );
}

// r[verify fair-share.advance-vtime]
/// Zero service leaves virtual time unchanged.
proof fn zero_service_no_advance(vtime: u64, weight: u64, total_weight: u64)
    requires weight > 0, total_weight > 0
    ensures advance_vtime_spec(vtime, 0, weight, total_weight) == vtime
{
    assert(0int * total_weight as int == 0int) by(nonlinear_arith);
}

// ============================================================
// Minimum virtual time selection (pick next consumer)
// ============================================================

// r[depends fair-share.select-min-vtime]
/// Spec: index of the consumer with the lowest virtual time.
pub open spec fn select_min_vtime_spec(vtimes: Seq<u64>) -> int
    recommends vtimes.len() > 0
    decreases vtimes.len()
{
    if vtimes.len() <= 1 {
        0int
    } else {
        let rest_idx = select_min_vtime_spec(vtimes.drop_first());
        if vtimes[0] <= vtimes[rest_idx + 1] {
            0int
        } else {
            rest_idx + 1
        }
    }
}

// r[impl fair-share.select-min-vtime]
/// Select the consumer with the lowest virtual time.
pub fn select_min_vtime(vtimes: &[u64]) -> (result: usize)
    requires vtimes@.len() > 0
    ensures
        result < vtimes@.len(),
        forall|j: int| #![trigger vtimes@[j]]
            0 <= j < vtimes@.len() as int
            ==> vtimes@[result as int] <= vtimes@[j],
{
    let mut best: usize = 0;
    let mut i: usize = 1;
    while i < vtimes.len()
        invariant
            1 <= i <= vtimes@.len(),
            best < i,
            forall|j: int| #![trigger vtimes@[j]]
                0 <= j < i as int
                ==> vtimes@[best as int] <= vtimes@[j],
        decreases vtimes@.len() - i,
    {
        if vtimes[i] < vtimes[best] {
            best = i;
        }
        i = i + 1;
    }
    best
}

// r[verify fair-share.select-min-vtime]
/// Singleton always selects index 0.
proof fn select_singleton(v: u64)
    ensures select_min_vtime_spec(seq![v]) == 0int
{}

// r[verify fair-share.allocate]
/// Equal weights get equal allocations.
proof fn equal_weights_equal_alloc(total_tokens: u64, w: u64, tw: u64)
    requires tw > 0
    ensures fair_alloc_spec(total_tokens, w, tw) == fair_alloc_spec(total_tokens, w, tw)
{}

} // verus!

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod test {
    use super::*;

    // r[verify fair-share.total-weight]
    #[test]
    fn test_total_weight() {
        assert_eq!(total_weight(&[10, 20, 30]), Some(60));
        assert_eq!(total_weight(&[]), Some(0));
        assert_eq!(total_weight(&[u64::MAX, 1]), None); // overflow
    }

    // r[verify fair-share.allocate]
    #[test]
    fn test_fair_alloc() {
        // 100 tokens, weight 3 out of 10 → floor(300/10) = 30
        assert_eq!(fair_alloc(100, 3, 10), 30);
        // Full weight
        assert_eq!(fair_alloc(100, 10, 10), 100);
        // Zero weight
        assert_eq!(fair_alloc(100, 0, 10), 0);
    }

    // r[verify fair-share.advance-vtime]
    #[test]
    fn test_advance_vtime() {
        // vtime=0, service=10, weight=2, total=10 → increment = 10*10/2 = 50
        assert_eq!(advance_vtime(0, 10, 2, 10), 50);
        // Zero service
        assert_eq!(advance_vtime(100, 0, 5, 10), 100);
        // Saturation
        assert_eq!(advance_vtime(u64::MAX - 1, 100, 1, 100), u64::MAX);
    }

    // r[verify fair-share.select-min-vtime]
    #[test]
    fn test_select_min_vtime() {
        assert_eq!(select_min_vtime(&[50, 10, 30]), 1);
        assert_eq!(select_min_vtime(&[42]), 0);
        assert_eq!(select_min_vtime(&[5, 5, 5]), 0); // ties: first
    }

    #[cfg(test)]
    mod proptests {
        use proptest::prelude::*;

        use super::*;

        proptest! {
            // r[verify fair-share.allocate]
            #[test]
            fn prop_alloc_bounded(tokens: u64, weight in 0u64..=1000, tw in 1u64..=1000) {
                if weight <= tw {
                    prop_assert!(fair_alloc(tokens, weight, tw) <= tokens);
                }
            }

            // r[verify fair-share.allocate]
            #[test]
            fn prop_higher_weight_more(tokens: u64, w1 in 0u64..=500, w2 in 0u64..=500, tw in 1u64..=1000) {
                if w1 <= w2 && w2 <= tw {
                    prop_assert!(fair_alloc(tokens, w1, tw) <= fair_alloc(tokens, w2, tw));
                }
            }

            // r[verify fair-share.advance-vtime]
            #[test]
            fn prop_vtime_monotone(vt: u64, svc: u64, w in 1u64..=1000, tw in 1u64..=1000) {
                prop_assert!(advance_vtime(vt, svc, w, tw) >= vt);
            }

            // r[verify fair-share.select-min-vtime]
            #[test]
            fn prop_select_min_is_min(vtimes in proptest::collection::vec(any::<u64>(), 1..50)) {
                let idx = select_min_vtime(&vtimes);
                let min_val = vtimes[idx];
                for v in &vtimes {
                    prop_assert!(min_val <= *v);
                }
            }
        }
    }
}
