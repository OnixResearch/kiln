//! Verified multi-factor eviction scoring.
//!
//! Computes a retention score from four weighted factors (recency, frequency,
//! cost, size) multiplied by priority and tier modifiers. Lower scores are
//! evicted first. Weights are integers summing to 100 and factor scores are
//! in [0, 100], keeping all arithmetic in u64 without floating point.
//!
//! Generalized from seaglass's tiered-cache eviction scoring.

use vstd::prelude::*;

verus! {

// --- weights_valid ---

// r[depends eviction-score.weights-valid]
/// Spec: whether four weights are valid (each in [0, 100], sum == 100).
pub open spec fn weights_valid_spec(w1: u64, w2: u64, w3: u64, w4: u64) -> bool {
    w1 <= 100 && w2 <= 100 && w3 <= 100 && w4 <= 100
    && (w1 + w2 + w3 + w4) == 100
}

// r[impl eviction-score.weights-valid]
/// Returns true if the four weights are each in [0, 100] and sum to 100.
pub fn weights_valid(w1: u64, w2: u64, w3: u64, w4: u64) -> (result: bool)
    ensures result == weights_valid_spec(w1, w2, w3, w4)
{
    w1 <= 100 && w2 <= 100 && w3 <= 100 && w4 <= 100
    && (w1 + w2 + w3 + w4) == 100
}

// r[verify eviction-score.weights-valid]
/// Default weights (30/30/30/10) are valid.
proof fn default_weights_valid()
    ensures weights_valid_spec(30, 30, 30, 10)
{}

// r[verify eviction-score.weights-valid]
/// Equal weights (25/25/25/25) are valid.
proof fn equal_weights_valid()
    ensures weights_valid_spec(25, 25, 25, 25)
{}

// r[verify eviction-score.weights-valid]
/// Single-factor weights are valid (one weight is 100, rest are 0).
proof fn single_factor_valid()
    ensures
        weights_valid_spec(100, 0, 0, 0),
        weights_valid_spec(0, 100, 0, 0),
        weights_valid_spec(0, 0, 100, 0),
        weights_valid_spec(0, 0, 0, 100),
{}

// --- weighted_score ---

// r[depends eviction-score.weighted-score]
/// Spec: compute weighted sum of four factor scores.
/// Each factor is in [0, 100] and weights sum to 100.
/// Result is in [0, 10000].
pub open spec fn weighted_score_spec(
    w1: u64, w2: u64, w3: u64, w4: u64,
    s1: u64, s2: u64, s3: u64, s4: u64,
) -> u64 {
    (w1 * s1 + w2 * s2 + w3 * s3 + w4 * s4) as u64
}

// r[impl eviction-score.weighted-score]
/// Computes the weighted sum of four factor scores.
///
/// Weights must be valid (sum to 100, each in [0, 100]).
/// Factor scores must each be in [0, 100].
/// Result is in [0, 10000].
pub fn weighted_score(
    w1: u64, w2: u64, w3: u64, w4: u64,
    s1: u64, s2: u64, s3: u64, s4: u64,
) -> (result: u64)
    requires
        weights_valid_spec(w1, w2, w3, w4),
        s1 <= 100, s2 <= 100, s3 <= 100, s4 <= 100,
    ensures
        result == weighted_score_spec(w1, w2, w3, w4, s1, s2, s3, s4),
{
    proof {
        assert(w1 as int * s1 as int <= 10000) by(nonlinear_arith)
            requires w1 <= 100, s1 <= 100;
        assert(w2 as int * s2 as int <= 10000) by(nonlinear_arith)
            requires w2 <= 100, s2 <= 100;
        assert(w3 as int * s3 as int <= 10000) by(nonlinear_arith)
            requires w3 <= 100, s3 <= 100;
        assert(w4 as int * s4 as int <= 10000) by(nonlinear_arith)
            requires w4 <= 100, s4 <= 100;
    }
    w1 * s1 + w2 * s2 + w3 * s3 + w4 * s4
}

// r[verify eviction-score.weighted-score]
/// Weighted score is bounded by 10000.
proof fn weighted_score_bounded(
    w1: u64, w2: u64, w3: u64, w4: u64,
    s1: u64, s2: u64, s3: u64, s4: u64,
)
    requires
        weights_valid_spec(w1, w2, w3, w4),
        s1 <= 100, s2 <= 100, s3 <= 100, s4 <= 100,
    ensures
        weighted_score_spec(w1, w2, w3, w4, s1, s2, s3, s4) <= 10000,
{
    assert(w1 as int * s1 as int <= w1 as int * 100) by(nonlinear_arith)
        requires s1 <= 100;
    assert(w2 as int * s2 as int <= w2 as int * 100) by(nonlinear_arith)
        requires s2 <= 100;
    assert(w3 as int * s3 as int <= w3 as int * 100) by(nonlinear_arith)
        requires s3 <= 100;
    assert(w4 as int * s4 as int <= w4 as int * 100) by(nonlinear_arith)
        requires s4 <= 100;
    assert(w1 as int * 100 + w2 as int * 100 + w3 as int * 100 + w4 as int * 100
        == (w1 as int + w2 as int + w3 as int + w4 as int) * 100) by(nonlinear_arith);
}

// r[verify eviction-score.weighted-score]
/// All factors at 100 yields maximum score of 10000.
proof fn weighted_score_max(w1: u64, w2: u64, w3: u64, w4: u64)
    requires weights_valid_spec(w1, w2, w3, w4)
    ensures weighted_score_spec(w1, w2, w3, w4, 100, 100, 100, 100) == 10000
{
    assert(w1 * 100 + w2 * 100 + w3 * 100 + w4 * 100
        == (w1 + w2 + w3 + w4) * 100) by(nonlinear_arith);
}

// r[verify eviction-score.weighted-score]
/// All factors at 0 yields minimum score of 0.
proof fn weighted_score_min(w1: u64, w2: u64, w3: u64, w4: u64)
    requires weights_valid_spec(w1, w2, w3, w4)
    ensures weighted_score_spec(w1, w2, w3, w4, 0, 0, 0, 0) == 0
{}

// r[verify eviction-score.weighted-score]
/// Increasing any factor score does not decrease the weighted score.
proof fn weighted_score_monotonic_s1(
    w1: u64, w2: u64, w3: u64, w4: u64,
    s1a: u64, s1b: u64, s2: u64, s3: u64, s4: u64,
)
    requires
        weights_valid_spec(w1, w2, w3, w4),
        s1a <= 100, s1b <= 100, s2 <= 100, s3 <= 100, s4 <= 100,
        s1a <= s1b,
    ensures
        weighted_score_spec(w1, w2, w3, w4, s1a, s2, s3, s4)
        <= weighted_score_spec(w1, w2, w3, w4, s1b, s2, s3, s4),
{
    // Individual product bounds (nonlinear: a*b <= 100*100 when a,b <= 100)
    assert(w1 as int * s1a as int <= 10000) by(nonlinear_arith) requires w1 <= 100, s1a <= 100;
    assert(w1 as int * s1b as int <= 10000) by(nonlinear_arith) requires w1 <= 100, s1b <= 100;
    assert(w2 as int * s2 as int <= 10000) by(nonlinear_arith) requires w2 <= 100, s2 <= 100;
    assert(w3 as int * s3 as int <= 10000) by(nonlinear_arith) requires w3 <= 100, s3 <= 100;
    assert(w4 as int * s4 as int <= 10000) by(nonlinear_arith) requires w4 <= 100, s4 <= 100;
    // w1 * s1a <= w1 * s1b since s1a <= s1b
    assert(w1 as int * s1a as int <= w1 as int * s1b as int) by(nonlinear_arith)
        requires s1a <= s1b;
    // Z3 can add up product bounds (linear) and derive int-level sum ordering.
    // Then the as u64 casts are identity since sums are in [0, 40000] < u64::MAX.
}

// --- priority_multiplier ---

// r[depends eviction-score.priority-multiplier]
/// Spec: priority multiplier. Low=1, Normal=2, High=4, Critical=20.
/// Higher priority entries get higher scores (harder to evict).
pub open spec fn priority_multiplier_spec(priority: u8) -> u64 {
    if priority == 0 { 1 }       // Low
    else if priority == 1 { 2 }  // Normal
    else if priority == 2 { 4 }  // High
    else if priority == 3 { 20 } // Critical
    else { 1 }                   // Unknown defaults to Low
}

// r[impl eviction-score.priority-multiplier]
/// Returns the priority multiplier for the given priority level.
/// Low (0) = 1, Normal (1) = 2, High (2) = 4, Critical (3) = 20.
pub fn priority_multiplier(priority: u8) -> (result: u64)
    ensures result == priority_multiplier_spec(priority)
{
    if priority == 0 { 1 }
    else if priority == 1 { 2 }
    else if priority == 2 { 4 }
    else if priority == 3 { 20 }
    else { 1 }
}

// r[verify eviction-score.priority-multiplier]
/// Priority multiplier is always >= 1.
proof fn priority_multiplier_ge_one(priority: u8)
    ensures priority_multiplier_spec(priority) >= 1
{}

// r[verify eviction-score.priority-multiplier]
/// Higher priority yields higher or equal multiplier.
proof fn priority_multiplier_monotonic(a: u8, b: u8)
    requires a <= b, a <= 3, b <= 3
    ensures priority_multiplier_spec(a) <= priority_multiplier_spec(b)
{}

// r[verify eviction-score.priority-multiplier]
/// Critical priority dominates all others.
proof fn critical_dominates(priority: u8)
    requires priority <= 3
    ensures priority_multiplier_spec(priority) <= priority_multiplier_spec(3)
{}

// --- tier_multiplier ---

// r[depends eviction-score.tier-multiplier]
/// Spec: tier multiplier. Cold=1, Warm=2, Hot=4.
/// Hotter entries get higher scores (harder to evict).
pub open spec fn tier_multiplier_spec(tier: u8) -> u64 {
    if tier == 0 { 1 }      // Cold
    else if tier == 1 { 2 }  // Warm
    else if tier == 2 { 4 }  // Hot
    else { 1 }               // Unknown defaults to Cold
}

// r[impl eviction-score.tier-multiplier]
/// Returns the tier multiplier for the given tier.
/// Cold (0) = 1, Warm (1) = 2, Hot (2) = 4.
pub fn tier_multiplier(tier: u8) -> (result: u64)
    ensures result == tier_multiplier_spec(tier)
{
    if tier == 0 { 1 }
    else if tier == 1 { 2 }
    else if tier == 2 { 4 }
    else { 1 }
}

// r[verify eviction-score.tier-multiplier]
/// Tier multiplier is always >= 1.
proof fn tier_multiplier_ge_one(tier: u8)
    ensures tier_multiplier_spec(tier) >= 1
{}

// r[verify eviction-score.tier-multiplier]
/// Hotter tier yields higher or equal multiplier.
proof fn tier_multiplier_monotonic(a: u8, b: u8)
    requires a <= b, a <= 2, b <= 2
    ensures tier_multiplier_spec(a) <= tier_multiplier_spec(b)
{}

// --- full_score ---

// r[depends eviction-score.full-score]
/// Spec: full eviction score = weighted_score * priority_mult * tier_mult.
/// Lower scores are evicted first.
pub open spec fn full_score_spec(
    w1: u64, w2: u64, w3: u64, w4: u64,
    s1: u64, s2: u64, s3: u64, s4: u64,
    priority: u8, tier: u8,
) -> u64 {
    (weighted_score_spec(w1, w2, w3, w4, s1, s2, s3, s4)
     * priority_multiplier_spec(priority)
     * tier_multiplier_spec(tier)) as u64
}

// r[impl eviction-score.full-score]
/// Computes the full eviction score: base * priority * tier.
///
/// Lower scores are evicted first. Critical+Hot entries get the
/// highest scores and are hardest to evict.
pub fn full_score(
    w1: u64, w2: u64, w3: u64, w4: u64,
    s1: u64, s2: u64, s3: u64, s4: u64,
    priority: u8, tier: u8,
) -> (result: u64)
    requires
        weights_valid_spec(w1, w2, w3, w4),
        s1 <= 100, s2 <= 100, s3 <= 100, s4 <= 100,
        priority <= 3, tier <= 2,
    ensures
        result == full_score_spec(w1, w2, w3, w4, s1, s2, s3, s4, priority, tier),
{
    let base = weighted_score(w1, w2, w3, w4, s1, s2, s3, s4);
    let pm = priority_multiplier(priority);
    let tm = tier_multiplier(tier);
    proof {
        weighted_score_bounded(w1, w2, w3, w4, s1, s2, s3, s4);
        // base <= 10000, pm <= 20, tm <= 4
        // 10000 * 20 * 4 = 800000, fits in u64
        assert(base as int * pm as int <= 10000 * 20) by(nonlinear_arith)
            requires base <= 10000, pm <= 20;
        assert(base as int * pm as int * tm as int <= 10000 * 20 * 4) by(nonlinear_arith)
            requires base as int * pm as int <= 10000 * 20, tm <= 4;
    }
    base * pm * tm
}

// r[verify eviction-score.full-score]
/// Full score is bounded by 800000 (10000 * 20 * 4).
proof fn full_score_bounded(
    w1: u64, w2: u64, w3: u64, w4: u64,
    s1: u64, s2: u64, s3: u64, s4: u64,
    priority: u8, tier: u8,
)
    requires
        weights_valid_spec(w1, w2, w3, w4),
        s1 <= 100, s2 <= 100, s3 <= 100, s4 <= 100,
        priority <= 3, tier <= 2,
    ensures
        full_score_spec(w1, w2, w3, w4, s1, s2, s3, s4, priority, tier) <= 800000,
{
    weighted_score_bounded(w1, w2, w3, w4, s1, s2, s3, s4);
    let base = weighted_score_spec(w1, w2, w3, w4, s1, s2, s3, s4);
    let pm = priority_multiplier_spec(priority);
    let tm = tier_multiplier_spec(tier);
    assert(base * pm <= 10000 * 20) by(nonlinear_arith)
        requires base <= 10000, pm <= 20;
    assert(base * pm * tm <= 10000 * 20 * 4) by(nonlinear_arith)
        requires base * pm <= 10000 * 20, tm <= 4;
}

// r[verify eviction-score.full-score]
/// Higher priority yields higher or equal full score (all else equal).
proof fn full_score_priority_monotonic(
    w1: u64, w2: u64, w3: u64, w4: u64,
    s1: u64, s2: u64, s3: u64, s4: u64,
    p1: u8, p2: u8, tier: u8,
)
    requires
        weights_valid_spec(w1, w2, w3, w4),
        s1 <= 100, s2 <= 100, s3 <= 100, s4 <= 100,
        p1 <= p2, p1 <= 3, p2 <= 3, tier <= 2,
    ensures
        full_score_spec(w1, w2, w3, w4, s1, s2, s3, s4, p1, tier)
        <= full_score_spec(w1, w2, w3, w4, s1, s2, s3, s4, p2, tier),
{
    weighted_score_bounded(w1, w2, w3, w4, s1, s2, s3, s4);
    let base = weighted_score_spec(w1, w2, w3, w4, s1, s2, s3, s4);
    let pm1 = priority_multiplier_spec(p1);
    let pm2 = priority_multiplier_spec(p2);
    let tm = tier_multiplier_spec(tier);
    priority_multiplier_monotonic(p1, p2);
    assert(base * pm1 <= base * pm2) by(nonlinear_arith)
        requires pm1 <= pm2, base <= 10000;
    assert(base * pm1 * tm <= base * pm2 * tm) by(nonlinear_arith)
        requires base * pm1 <= base * pm2, tm <= 4;
}

// r[verify eviction-score.full-score]
/// Hotter tier yields higher or equal full score (all else equal).
proof fn full_score_tier_monotonic(
    w1: u64, w2: u64, w3: u64, w4: u64,
    s1: u64, s2: u64, s3: u64, s4: u64,
    priority: u8, t1: u8, t2: u8,
)
    requires
        weights_valid_spec(w1, w2, w3, w4),
        s1 <= 100, s2 <= 100, s3 <= 100, s4 <= 100,
        priority <= 3, t1 <= t2, t1 <= 2, t2 <= 2,
    ensures
        full_score_spec(w1, w2, w3, w4, s1, s2, s3, s4, priority, t1)
        <= full_score_spec(w1, w2, w3, w4, s1, s2, s3, s4, priority, t2),
{
    weighted_score_bounded(w1, w2, w3, w4, s1, s2, s3, s4);
    let base = weighted_score_spec(w1, w2, w3, w4, s1, s2, s3, s4);
    let pm = priority_multiplier_spec(priority);
    let tm1 = tier_multiplier_spec(t1);
    let tm2 = tier_multiplier_spec(t2);
    tier_multiplier_monotonic(t1, t2);
    assert(base * pm <= 10000 * 20) by(nonlinear_arith)
        requires base <= 10000, pm <= 20;
    assert(base * pm * tm1 <= base * pm * tm2) by(nonlinear_arith)
        requires tm1 <= tm2, base * pm <= 200000;
}

// r[verify eviction-score.full-score]
/// Zero base score means zero full score regardless of multipliers.
proof fn zero_base_zero_full(priority: u8, tier: u8)
    requires priority <= 3, tier <= 2
    ensures full_score_spec(25, 25, 25, 25, 0, 0, 0, 0, priority, tier) == 0
{
    assert(weighted_score_spec(25, 25, 25, 25, 0, 0, 0, 0) == 0);
    let pm = priority_multiplier_spec(priority);
    let tm = tier_multiplier_spec(tier);
    assert(0int * pm == 0) by(nonlinear_arith);
    assert(0int * pm * tm == 0) by(nonlinear_arith);
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify eviction-score.weights-valid]
    #[test]
    fn test_weights_valid() {
        assert!(weights_valid(30, 30, 30, 10));
        assert!(weights_valid(25, 25, 25, 25));
        assert!(weights_valid(100, 0, 0, 0));
        assert!(!weights_valid(30, 30, 30, 20)); // sum = 110
        assert!(!weights_valid(0, 0, 0, 0)); // sum = 0
        assert!(!weights_valid(101, 0, 0, 0)); // > 100
    }

    // r[verify eviction-score.weighted-score]
    #[test]
    fn test_weighted_score() {
        // Default weights (30/30/30/10), all factors at 50
        assert_eq!(weighted_score(30, 30, 30, 10, 50, 50, 50, 50), 5000);
        // All factors at max
        assert_eq!(weighted_score(25, 25, 25, 25, 100, 100, 100, 100), 10000);
        // All factors at min
        assert_eq!(weighted_score(25, 25, 25, 25, 0, 0, 0, 0), 0);
        // Single factor
        assert_eq!(weighted_score(100, 0, 0, 0, 80, 50, 50, 50), 8000);
    }

    // r[verify eviction-score.priority-multiplier]
    #[test]
    fn test_priority_multiplier() {
        assert_eq!(priority_multiplier(0), 1); // Low
        assert_eq!(priority_multiplier(1), 2); // Normal
        assert_eq!(priority_multiplier(2), 4); // High
        assert_eq!(priority_multiplier(3), 20); // Critical
        assert_eq!(priority_multiplier(4), 1); // Unknown
    }

    // r[verify eviction-score.tier-multiplier]
    #[test]
    fn test_tier_multiplier() {
        assert_eq!(tier_multiplier(0), 1); // Cold
        assert_eq!(tier_multiplier(1), 2); // Warm
        assert_eq!(tier_multiplier(2), 4); // Hot
        assert_eq!(tier_multiplier(3), 1); // Unknown
    }

    // r[verify eviction-score.full-score]
    #[test]
    fn test_full_score() {
        // Low priority, cold tier, low factors
        let low = full_score(25, 25, 25, 25, 10, 10, 10, 10, 0, 0);
        // Critical priority, hot tier, high factors
        let high = full_score(25, 25, 25, 25, 90, 90, 90, 90, 3, 2);
        assert!(high > low);

        // Max possible: 10000 * 20 * 4 = 800000
        let max = full_score(25, 25, 25, 25, 100, 100, 100, 100, 3, 2);
        assert_eq!(max, 800_000);

        // Min possible: 0
        let min = full_score(25, 25, 25, 25, 0, 0, 0, 0, 0, 0);
        assert_eq!(min, 0);
    }

    // r[verify eviction-score.full-score]
    #[test]
    fn test_critical_hot_dominates() {
        // Same base score, different priority/tier
        let cold_low = full_score(25, 25, 25, 25, 50, 50, 50, 50, 0, 0);
        let hot_critical = full_score(25, 25, 25, 25, 50, 50, 50, 50, 3, 2);
        // hot_critical should be 80x cold_low
        assert_eq!(hot_critical, cold_low * 80);
    }

    proptest! {
        // r[verify eviction-score.weighted-score]
        #[test]
        fn prop_weighted_score_bounded(
            s1 in 0u64..=100,
            s2 in 0u64..=100,
            s3 in 0u64..=100,
            s4 in 0u64..=100,
        ) {
            let score = weighted_score(25, 25, 25, 25, s1, s2, s3, s4);
            prop_assert!(score <= 10000);
        }

        // r[verify eviction-score.full-score]
        #[test]
        fn prop_full_score_bounded(
            s1 in 0u64..=100,
            s2 in 0u64..=100,
            s3 in 0u64..=100,
            s4 in 0u64..=100,
            priority in 0u8..=3,
            tier in 0u8..=2,
        ) {
            let score = full_score(25, 25, 25, 25, s1, s2, s3, s4, priority, tier);
            prop_assert!(score <= 800_000);
        }

        // r[verify eviction-score.full-score]
        #[test]
        fn prop_priority_monotonic(
            s1 in 0u64..=100,
            s2 in 0u64..=100,
            s3 in 0u64..=100,
            s4 in 0u64..=100,
            p1 in 0u8..=3,
            p2 in 0u8..=3,
            tier in 0u8..=2,
        ) {
            let lo = p1.min(p2);
            let hi = p1.max(p2);
            let score_lo = full_score(25, 25, 25, 25, s1, s2, s3, s4, lo, tier);
            let score_hi = full_score(25, 25, 25, 25, s1, s2, s3, s4, hi, tier);
            prop_assert!(score_lo <= score_hi);
        }

        // r[verify eviction-score.full-score]
        #[test]
        fn prop_tier_monotonic(
            s1 in 0u64..=100,
            s2 in 0u64..=100,
            s3 in 0u64..=100,
            s4 in 0u64..=100,
            priority in 0u8..=3,
            t1 in 0u8..=2,
            t2 in 0u8..=2,
        ) {
            let lo = t1.min(t2);
            let hi = t1.max(t2);
            let score_lo = full_score(25, 25, 25, 25, s1, s2, s3, s4, priority, lo);
            let score_hi = full_score(25, 25, 25, 25, s1, s2, s3, s4, priority, hi);
            prop_assert!(score_lo <= score_hi);
        }

        // r[verify eviction-score.weighted-score]
        #[test]
        fn prop_factor_monotonic(
            s1a in 0u64..=100,
            s1b in 0u64..=100,
            s2 in 0u64..=100,
            s3 in 0u64..=100,
            s4 in 0u64..=100,
        ) {
            let lo = s1a.min(s1b);
            let hi = s1a.max(s1b);
            let score_lo = weighted_score(25, 25, 25, 25, lo, s2, s3, s4);
            let score_hi = weighted_score(25, 25, 25, 25, hi, s2, s3, s4);
            prop_assert!(score_lo <= score_hi);
        }
    }
}
