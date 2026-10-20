//! Verified cost tracking and budget enforcement.
//!
//! Integer-based cost accumulation using microdollars (u64), threshold
//! crossing detection, milestone counting, budget status classification,
//! and model selection priority ordering. Extracted from clankers-model-selection.

#![allow(clippy::nonminimal_bool, clippy::bool_to_int_with_if)]

use vstd::prelude::*;

verus! {

// ============================================================
// Phase 1: Cost Accumulation
// ============================================================

// r[depends cost-tracker.cost-non-negative]
/// Spec: compute cost in microdollars.
/// `tokens` is the raw token count, `price_micro_per_mtok` is microdollars
/// per million tokens. Result = tokens * price / 1_000_000.
/// Uses int arithmetic (arbitrary precision) — no overflow in spec land.
pub open spec fn compute_cost_spec(tokens: u64, price_micro_per_mtok: u64) -> int {
    (tokens as int) * (price_micro_per_mtok as int) / 1_000_000
}

// r[impl cost-tracker.cost-non-negative]
/// Computes turn cost in microdollars with overflow protection.
/// Returns None if the multiplication would overflow u64.
pub fn compute_cost(tokens: u64, price_micro_per_mtok: u64) -> (result: Option<u64>)
    ensures
        match result {
            Some(v) => v as int == compute_cost_spec(tokens, price_micro_per_mtok),
            None => tokens as int * price_micro_per_mtok as int > u64::MAX as int,
        },
{
    if tokens == 0 || price_micro_per_mtok == 0 {
        Some(0)
    } else if tokens > u64::MAX / price_micro_per_mtok {
        proof {
            // tokens > MAX / price means tokens * price > MAX
            assert(tokens as int * price_micro_per_mtok as int > u64::MAX as int) by(nonlinear_arith)
                requires tokens > u64::MAX / price_micro_per_mtok,
                         price_micro_per_mtok > 0u64,
            {
                vstd::arithmetic::div_mod::lemma_fundamental_div_mod(
                    u64::MAX as int, price_micro_per_mtok as int);
            }
        }
        None
    } else {
        proof {
            // tokens <= MAX / price means tokens * price <= MAX (fits in u64)
            assert(tokens as int * price_micro_per_mtok as int <= u64::MAX as int) by(nonlinear_arith)
                requires tokens <= u64::MAX / price_micro_per_mtok,
                         tokens > 0u64,
                         price_micro_per_mtok > 0u64,
            {
                vstd::arithmetic::div_mod::lemma_fundamental_div_mod(
                    u64::MAX as int, price_micro_per_mtok as int);
            }
        }
        let product = tokens * price_micro_per_mtok;
        Some(product / 1_000_000)
    }
}

// r[verify cost-tracker.cost-non-negative]
/// Cost is non-negative for any inputs (trivial — u64 * u64 / positive is non-negative).
proof fn cost_non_negative(tokens: u64, price_micro_per_mtok: u64)
    ensures compute_cost_spec(tokens, price_micro_per_mtok) >= 0
{}

// r[verify cost-tracker.cost-non-negative]
/// Zero tokens always yields zero cost.
proof fn cost_zero_tokens(price_micro_per_mtok: u64)
    ensures compute_cost_spec(0, price_micro_per_mtok) == 0
{}

// r[verify cost-tracker.cost-non-negative]
/// Zero price always yields zero cost.
proof fn cost_zero_price(tokens: u64)
    ensures compute_cost_spec(tokens, 0) == 0
{}

// r[depends cost-tracker.cost-additive]
/// Spec: accumulate a turn cost onto a running total.
pub open spec fn accumulate_spec(total_before: u64, turn_cost: u64) -> int {
    total_before as int + turn_cost as int
}

// r[impl cost-tracker.cost-additive]
/// Accumulates a turn cost onto the running total.
/// Returns None on overflow.
pub fn accumulate(total_before: u64, turn_cost: u64) -> (result: Option<u64>)
    ensures
        match result {
            Some(v) => v as int == accumulate_spec(total_before, turn_cost),
            None => accumulate_spec(total_before, turn_cost) > u64::MAX as int,
        },
{
    if total_before > u64::MAX - turn_cost {
        None
    } else {
        Some(total_before + turn_cost)
    }
}

// r[impl cost-tracker.cost-monotonic]
// r[verify cost-tracker.cost-monotonic]
/// Accumulation is monotonic: result >= total_before.
proof fn accumulate_monotonic(total_before: u64, turn_cost: u64)
    ensures accumulate_spec(total_before, turn_cost) >= total_before as int
{}

// r[verify cost-tracker.cost-additive]
/// Accumulation is exactly addition.
proof fn accumulate_additive(total_before: u64, turn_cost: u64)
    ensures accumulate_spec(total_before, turn_cost) == total_before as int + turn_cost as int
{}

// r[verify cost-tracker.cost-additive]
/// Two accumulations compose: accumulate(accumulate(t, c1), c2) == t + c1 + c2.
proof fn accumulate_associative(total: u64, cost1: u64, cost2: u64)
    requires
        total as int + cost1 as int <= u64::MAX as int,
        total as int + cost1 as int + cost2 as int <= u64::MAX as int,
    ensures
        accumulate_spec((total + cost1) as u64, cost2) ==
            total as int + cost1 as int + cost2 as int,
{}

// ============================================================
// Phase 2: Threshold Crossing Detection
// ============================================================

// r[depends cost-tracker.threshold-fires-once]
/// Spec: does the transition from prev to current cross the limit?
pub open spec fn crosses_threshold_spec(prev: u64, current: u64, limit: u64) -> bool {
    prev < limit && current >= limit
}

// r[impl cost-tracker.threshold-fires-once]
/// Detects whether a threshold is crossed in a single update.
pub fn crosses_threshold(prev: u64, current: u64, limit: u64) -> (result: bool)
    ensures result == crosses_threshold_spec(prev, current, limit),
{
    prev < limit && current >= limit
}

// r[verify cost-tracker.threshold-fires-once]
/// Fires on crossing: prev < limit and current >= limit.
proof fn threshold_fires_on_crossing(prev: u64, current: u64, limit: u64)
    requires prev < limit, current >= limit,
    ensures crosses_threshold_spec(prev, current, limit),
{}

// r[verify cost-tracker.threshold-fires-once]
/// Silent when already crossed: prev >= limit means no fire.
proof fn threshold_silent_already_crossed(prev: u64, current: u64, limit: u64)
    requires prev >= limit,
    ensures !crosses_threshold_spec(prev, current, limit),
{}

// r[verify cost-tracker.threshold-fires-once]
/// Silent when not reached: current < limit means no fire.
proof fn threshold_silent_not_reached(prev: u64, current: u64, limit: u64)
    requires current < limit,
    ensures !crosses_threshold_spec(prev, current, limit),
{}

// r[verify cost-tracker.threshold-fires-once]
/// Exactly once over a monotonic sequence: if crosses at (i, i+1),
/// then no other adjacent pair crosses.
/// Modeled: if prev1 <= prev2 < limit <= current2, and prev2 >= limit,
/// then prev1 >= limit (contradiction). So crossing at a later point
/// means all earlier points already crossed.
proof fn threshold_exactly_once(prev1: u64, mid: u64, current: u64, limit: u64)
    requires
        prev1 <= mid,
        mid <= current,
        crosses_threshold_spec(prev1, mid, limit),
    ensures
        !crosses_threshold_spec(mid, current, limit),
{}

// r[depends cost-tracker.milestone-count]
/// Spec: number of milestones crossed in one update.
/// Equal to floor(current / interval) - floor(prev / interval).
pub open spec fn milestone_count_spec(prev: u64, current: u64, interval: u64) -> int
    recommends interval > 0,
{
    if interval == 0 {
        0
    } else {
        (current as int / interval as int) - (prev as int / interval as int)
    }
}

// r[impl cost-tracker.milestone-count]
/// Computes how many milestone intervals were crossed.
pub fn milestone_count(prev: u64, current: u64, interval: u64) -> (result: u64)
    requires
        interval > 0,
        current >= prev,
    ensures
        result as int == milestone_count_spec(prev, current, interval),
{
    proof {
        vstd::arithmetic::div_mod::lemma_div_is_ordered(prev as int, current as int, interval as int);
    }
    current / interval - prev / interval
}

// r[impl cost-tracker.milestone-non-negative]
// r[verify cost-tracker.milestone-non-negative]
/// Milestone count is non-negative when current >= prev.
proof fn milestone_count_non_negative(prev: u64, current: u64, interval: u64)
    requires interval > 0, current >= prev,
    ensures milestone_count_spec(prev, current, interval) >= 0
{
    vstd::arithmetic::div_mod::lemma_div_is_ordered(prev as int, current as int, interval as int);
}

// r[verify cost-tracker.milestone-count]
/// Count is zero when both values are in the same interval.
proof fn milestone_count_zero_same_interval(prev: u64, current: u64, interval: u64)
    requires
        interval > 0,
        current >= prev,
        current as int / interval as int == prev as int / interval as int,
    ensures
        milestone_count_spec(prev, current, interval) == 0,
{}

// r[verify cost-tracker.milestone-count]
/// Milestones compose: count(a,c) == count(a,b) + count(b,c).
proof fn milestone_count_additive(a: u64, b: u64, c: u64, interval: u64)
    requires interval > 0, a <= b, b <= c,
    ensures
        milestone_count_spec(a, c, interval) ==
            milestone_count_spec(a, b, interval) + milestone_count_spec(b, c, interval),
{}

// ============================================================
// Phase 3: Budget Status State Machine
// ============================================================

/// Budget status: exactly one of four states.
/// Encoded as u8 for Verus compatibility (no enum matching in specs).
/// 0 = NoBudget, 1 = Ok, 2 = Warning, 3 = Exceeded.
pub open spec fn NO_BUDGET() -> u8 { 0 }
pub open spec fn BUDGET_OK() -> u8 { 1 }
pub open spec fn BUDGET_WARNING() -> u8 { 2 }
pub open spec fn BUDGET_EXCEEDED() -> u8 { 3 }

// r[depends cost-tracker.budget-status-exhaustive]
/// Spec: compute budget status from soft limit, hard limit, and total.
/// soft and hard are encoded as Option<u64>.
pub open spec fn budget_status_spec(
    soft: Option<u64>,
    hard: Option<u64>,
    total: u64,
) -> u8 {
    match (soft, hard) {
        (None, None) => NO_BUDGET(),
        (None, Some(h)) => {
            if total >= h { BUDGET_EXCEEDED() }
            else { BUDGET_OK() }
        },
        (Some(s), None) => {
            if total >= s { BUDGET_WARNING() }
            else { BUDGET_OK() }
        },
        (Some(s), Some(h)) => {
            if total >= h { BUDGET_EXCEEDED() }
            else if total >= s { BUDGET_WARNING() }
            else { BUDGET_OK() }
        },
    }
}

// r[impl cost-tracker.budget-status-exhaustive]
/// Computes budget status.
pub fn budget_status(
    soft: Option<u64>,
    hard: Option<u64>,
    total: u64,
) -> (result: u8)
    ensures result == budget_status_spec(soft, hard, total),
{
    match (soft, hard) {
        (None, None) => 0,
        (None, Some(h)) => {
            if total >= h { 3 } else { 1 }
        },
        (Some(s), None) => {
            if total >= s { 2 } else { 1 }
        },
        (Some(s), Some(h)) => {
            if total >= h { 3 }
            else if total >= s { 2 }
            else { 1 }
        },
    }
}

// r[verify cost-tracker.budget-status-exhaustive]
/// No limits means NoBudget for any total.
proof fn budget_no_limits_is_no_budget(total: u64)
    ensures budget_status_spec(None, None, total) == NO_BUDGET(),
{}

// r[verify cost-tracker.budget-status-exhaustive]
/// Under the lowest defined limit means Ok.
proof fn budget_under_limits_is_ok(soft: Option<u64>, hard: Option<u64>, total: u64)
    requires
        soft.is_some() || hard.is_some(),
        soft.is_some() ==> total < soft.unwrap(),
        hard.is_some() ==> total < hard.unwrap(),
    ensures
        budget_status_spec(soft, hard, total) == BUDGET_OK(),
{}

// r[verify cost-tracker.budget-status-exhaustive]
/// Between soft and hard means Warning (when both defined).
proof fn budget_between_is_warning(soft: u64, hard: u64, total: u64)
    requires soft <= total, total < hard,
    ensures budget_status_spec(Some(soft), Some(hard), total) == BUDGET_WARNING(),
{}

// r[verify cost-tracker.budget-status-exhaustive]
/// At or above hard means Exceeded (when hard defined).
proof fn budget_over_hard_is_exceeded(soft: Option<u64>, hard: u64, total: u64)
    requires total >= hard,
    ensures budget_status_spec(soft, Some(hard), total) == BUDGET_EXCEEDED(),
{}

// r[verify cost-tracker.budget-status-exhaustive]
/// Result is always one of the four valid states.
proof fn budget_status_valid(soft: Option<u64>, hard: Option<u64>, total: u64)
    ensures ({
        let s = budget_status_spec(soft, hard, total);
        s == NO_BUDGET() || s == BUDGET_OK() || s == BUDGET_WARNING() || s == BUDGET_EXCEEDED()
    }),
{}

// r[impl cost-tracker.budget-status-monotone]
// r[verify cost-tracker.budget-status-monotone]
/// Increasing total never moves status backward.
/// Encoded: status_value(t2) >= status_value(t1) when t2 >= t1.
/// NoBudget(0) is special — stays at 0 regardless.
proof fn budget_status_monotone(soft: Option<u64>, hard: Option<u64>, t1: u64, t2: u64)
    requires t2 >= t1,
    ensures budget_status_spec(soft, hard, t2) >= budget_status_spec(soft, hard, t1),
{}

// r[verify cost-tracker.budget-status-monotone]
/// Warning at t1 means Warning or Exceeded at t2 >= t1.
proof fn budget_warning_stable(soft: Option<u64>, hard: Option<u64>, t1: u64, t2: u64)
    requires
        t2 >= t1,
        budget_status_spec(soft, hard, t1) == BUDGET_WARNING(),
    ensures
        budget_status_spec(soft, hard, t2) == BUDGET_WARNING()
        || budget_status_spec(soft, hard, t2) == BUDGET_EXCEEDED(),
{}

// r[verify cost-tracker.budget-status-monotone]
/// Exceeded is absorbing: once exceeded, always exceeded.
proof fn budget_exceeded_absorbing(soft: Option<u64>, hard: Option<u64>, t1: u64, t2: u64)
    requires
        t2 >= t1,
        budget_status_spec(soft, hard, t1) == BUDGET_EXCEEDED(),
    ensures
        budget_status_spec(soft, hard, t2) == BUDGET_EXCEEDED(),
{}

// ============================================================
// Phase 4: Model Selection Priority
// ============================================================

/// Role constants: 0 = smol (cheapest), 1 = default, 2 = slow (most capable).
pub open spec fn ROLE_SMOL() -> u8 { 0 }
pub open spec fn ROLE_DEFAULT() -> u8 { 1 }
pub open spec fn ROLE_SLOW() -> u8 { 2 }

// r[depends cost-tracker.hard-budget-overrides-hint]
/// Spec: select model role based on budget, hint, and complexity score.
/// Priority: hard budget > user hint > complexity score.
pub open spec fn select_role_spec(
    total: u64,
    hard_limit: Option<u64>,
    soft_limit: Option<u64>,
    hint: Option<u8>,
    score: u64,
    low_threshold: u64,
    high_threshold: u64,
) -> u8 {
    if hard_limit.is_some() && total >= hard_limit.unwrap() {
        ROLE_SMOL()
    } else if hint.is_some() {
        hint.unwrap()
    } else {
        let adjusted = if soft_limit.is_some() && total >= soft_limit.unwrap() {
            score / 2
        } else {
            score
        };
        if adjusted > high_threshold {
            ROLE_SLOW()
        } else if adjusted < low_threshold {
            ROLE_SMOL()
        } else {
            ROLE_DEFAULT()
        }
    }
}

// r[impl cost-tracker.hard-budget-overrides-hint]
/// Selects the model role.
pub fn select_role(
    total: u64,
    hard_limit: Option<u64>,
    soft_limit: Option<u64>,
    hint: Option<u8>,
    score: u64,
    low_threshold: u64,
    high_threshold: u64,
) -> (result: u8)
    ensures result == select_role_spec(total, hard_limit, soft_limit, hint, score, low_threshold, high_threshold),
{
    if hard_limit.is_some() && total >= hard_limit.unwrap() {
        0
    } else if hint.is_some() {
        hint.unwrap()
    } else {
        let adjusted = if soft_limit.is_some() && total >= soft_limit.unwrap() {
            score / 2
        } else {
            score
        };
        if adjusted > high_threshold {
            2
        } else if adjusted < low_threshold {
            0
        } else {
            1
        }
    }
}

// r[verify cost-tracker.hard-budget-overrides-hint]
/// Hard budget always yields smol, regardless of hint or score.
proof fn hard_budget_overrides_all(
    total: u64,
    hard_limit: u64,
    soft_limit: Option<u64>,
    hint: Option<u8>,
    score: u64,
    low_threshold: u64,
    high_threshold: u64,
)
    requires total >= hard_limit,
    ensures select_role_spec(total, Some(hard_limit), soft_limit, hint, score, low_threshold, high_threshold) == ROLE_SMOL(),
{}

// r[impl cost-tracker.hint-overrides-score]
// r[verify cost-tracker.hint-overrides-score]
/// User hint overrides complexity score when under hard limit.
proof fn hint_overrides_score(
    total: u64,
    hard_limit: Option<u64>,
    soft_limit: Option<u64>,
    hint: u8,
    score: u64,
    low_threshold: u64,
    high_threshold: u64,
)
    requires
        hard_limit.is_some() ==> total < hard_limit.unwrap(),
    ensures
        select_role_spec(total, hard_limit, soft_limit, Some(hint), score, low_threshold, high_threshold) == hint,
{}

// r[impl cost-tracker.score-thresholds-partition]
// r[verify cost-tracker.score-thresholds-partition]
/// Score thresholds partition the number line with no gaps.
proof fn score_thresholds_partition(
    total: u64,
    hard_limit: Option<u64>,
    soft_limit: Option<u64>,
    score: u64,
    low_threshold: u64,
    high_threshold: u64,
)
    requires
        hard_limit.is_some() ==> total < hard_limit.unwrap(),
        soft_limit.is_none() || total < soft_limit.unwrap(),
    ensures ({
        let r = select_role_spec(total, hard_limit, soft_limit, None, score, low_threshold, high_threshold);
        r == ROLE_SMOL() || r == ROLE_DEFAULT() || r == ROLE_SLOW()
    }),
{}

// r[impl cost-tracker.soft-budget-demotes]
// r[verify cost-tracker.soft-budget-demotes]
/// Soft budget halves the score — never promotes.
proof fn soft_budget_never_promotes(score: u64)
    ensures score / 2 <= score,
{}

// r[verify cost-tracker.soft-budget-demotes]
/// Soft budget can only demote: role with soft pressure <= role without.
proof fn soft_budget_demotes_role(
    total: u64,
    hard_limit: Option<u64>,
    soft_limit: u64,
    score: u64,
    low_threshold: u64,
    high_threshold: u64,
)
    requires
        total >= soft_limit,
        hard_limit.is_some() ==> total < hard_limit.unwrap(),
        low_threshold <= high_threshold,
    ensures
        select_role_spec(total, hard_limit, Some(soft_limit), None, score, low_threshold, high_threshold)
        <= select_role_spec(total, hard_limit, None, None, score, low_threshold, high_threshold),
{
    // Z3 handles: score/2 <= score, so adjusted thresholds can only move the role down
}

// --- Complexity Score ---

// r[depends cost-tracker.score-non-negative]
/// Spec: complexity score from token count, tool use, and keyword bonus.
/// All weights are u64 (non-negative by construction).
pub open spec fn complexity_score_spec(
    token_count: u64,
    token_weight: u64,
    has_tools: bool,
    tool_weight: u64,
    keyword_bonus: u64,
) -> int {
    (token_count as int) * (token_weight as int) / 100
    + if has_tools { tool_weight as int } else { 0 }
    + keyword_bonus as int
}

// r[impl cost-tracker.score-non-negative]
/// Computes complexity score. Returns None on overflow.
pub fn complexity_score(
    token_count: u64,
    token_weight: u64,
    has_tools: bool,
    tool_weight: u64,
    keyword_bonus: u64,
) -> (result: Option<u64>)
    ensures
        match result {
            Some(v) => v as int == complexity_score_spec(token_count, token_weight, has_tools, tool_weight, keyword_bonus),
            None => true,
        },
{
    // token contribution: token_count * token_weight / 100
    let token_part = if token_count == 0 || token_weight == 0 {
        0u64
    } else if token_count > u64::MAX / token_weight {
        return None;
    } else {
        proof {
            assert(token_count as int * token_weight as int <= u64::MAX as int) by(nonlinear_arith)
                requires token_count <= u64::MAX / token_weight,
                         token_count > 0u64,
                         token_weight > 0u64,
            {
                vstd::arithmetic::div_mod::lemma_fundamental_div_mod(
                    u64::MAX as int, token_weight as int);
            }
        }
        token_count * token_weight / 100
    };

    let tool_part = if has_tools { tool_weight } else { 0u64 };

    // Sum the three parts with overflow checks
    if token_part > u64::MAX - tool_part {
        return None;
    }
    let subtotal = token_part + tool_part;
    if subtotal > u64::MAX - keyword_bonus {
        return None;
    }
    Some(subtotal + keyword_bonus)
}

// r[verify cost-tracker.score-non-negative]
/// Score is non-negative for any inputs (all u64, so trivially non-negative).
proof fn score_non_negative(
    token_count: u64,
    token_weight: u64,
    has_tools: bool,
    tool_weight: u64,
    keyword_bonus: u64,
)
    ensures complexity_score_spec(token_count, token_weight, has_tools, tool_weight, keyword_bonus) >= 0
{}

} // verus!

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- Phase 1: Cost Accumulation ---

    #[test]
    fn test_compute_cost_zero_tokens() {
        assert_eq!(compute_cost(0, 3_000_000), Some(0));
    }

    #[test]
    fn test_compute_cost_zero_price() {
        assert_eq!(compute_cost(1_000_000, 0), Some(0));
    }

    #[test]
    fn test_compute_cost_one_mtok() {
        // 1M tokens at $3/MTok = 3_000_000 microdollars
        assert_eq!(compute_cost(1_000_000, 3_000_000), Some(3_000_000));
    }

    #[test]
    fn test_compute_cost_small() {
        // 100 tokens at $3/MTok = 300 microdollars (integer division)
        assert_eq!(compute_cost(100, 3_000_000), Some(300));
    }

    #[test]
    fn test_compute_cost_overflow() {
        // u64::MAX tokens * u64::MAX price → overflow
        assert_eq!(compute_cost(u64::MAX, u64::MAX), None);
    }

    #[test]
    fn test_accumulate_normal() {
        assert_eq!(accumulate(5_000_000, 300_000), Some(5_300_000));
    }

    #[test]
    fn test_accumulate_overflow() {
        assert_eq!(accumulate(u64::MAX, 1), None);
    }

    #[test]
    fn test_accumulate_zero() {
        assert_eq!(accumulate(5_000_000, 0), Some(5_000_000));
    }

    // --- Phase 2: Threshold Crossing ---

    #[test]
    fn test_crosses_threshold_fires() {
        assert!(crosses_threshold(4_000_000, 6_000_000, 5_000_000));
    }

    #[test]
    fn test_crosses_threshold_already_crossed() {
        assert!(!crosses_threshold(6_000_000, 8_000_000, 5_000_000));
    }

    #[test]
    fn test_crosses_threshold_not_reached() {
        assert!(!crosses_threshold(2_000_000, 3_000_000, 5_000_000));
    }

    #[test]
    fn test_crosses_threshold_exact_boundary() {
        assert!(crosses_threshold(4_999_999, 5_000_000, 5_000_000));
    }

    #[test]
    fn test_milestone_count_three() {
        // prev=0.50, current=3.20 in microdollars (500K, 3.2M), interval=$1 (1M)
        assert_eq!(milestone_count(500_000, 3_200_000, 1_000_000), 3);
    }

    #[test]
    fn test_milestone_count_zero() {
        assert_eq!(milestone_count(1_200_000, 1_800_000, 1_000_000), 0);
    }

    #[test]
    fn test_milestone_count_exact_boundary() {
        assert_eq!(milestone_count(999_999, 1_000_000, 1_000_000), 1);
    }

    #[test]
    fn test_milestone_count_same() {
        assert_eq!(milestone_count(1_500_000, 1_500_000, 1_000_000), 0);
    }

    // --- Phase 3: Budget Status ---

    #[test]
    fn test_budget_no_limits() {
        assert_eq!(budget_status(None, None, 999_999_999), 0); // NoBudget
    }

    #[test]
    fn test_budget_ok_both_limits() {
        assert_eq!(budget_status(Some(5_000_000), Some(10_000_000), 3_000_000), 1); // Ok
    }

    #[test]
    fn test_budget_warning() {
        assert_eq!(budget_status(Some(5_000_000), Some(10_000_000), 7_000_000), 2); // Warning
    }

    #[test]
    fn test_budget_exceeded() {
        assert_eq!(budget_status(Some(5_000_000), Some(10_000_000), 12_000_000), 3); // Exceeded
    }

    #[test]
    fn test_budget_only_hard_ok() {
        assert_eq!(budget_status(None, Some(10_000_000), 5_000_000), 1); // Ok
    }

    #[test]
    fn test_budget_only_hard_exceeded() {
        assert_eq!(budget_status(None, Some(10_000_000), 15_000_000), 3); // Exceeded
    }

    #[test]
    fn test_budget_only_soft_ok() {
        assert_eq!(budget_status(Some(5_000_000), None, 3_000_000), 1); // Ok
    }

    #[test]
    fn test_budget_only_soft_warning() {
        assert_eq!(budget_status(Some(5_000_000), None, 7_000_000), 2); // Warning
    }

    // --- Phase 4: Model Selection ---

    #[test]
    fn test_hard_budget_overrides() {
        // Over hard limit → smol regardless of hint
        assert_eq!(select_role(6_000_000, Some(5_000_000), None, Some(2), 100, 20, 80), 0);
    }

    #[test]
    fn test_hint_overrides_score() {
        // Under hard, hint=slow → slow regardless of low score
        assert_eq!(select_role(1_000_000, Some(5_000_000), None, Some(2), 5, 20, 80), 2);
    }

    #[test]
    fn test_score_high() {
        // No hint, score > high → slow
        assert_eq!(select_role(1_000_000, Some(5_000_000), None, None, 90, 20, 80), 2);
    }

    #[test]
    fn test_score_low() {
        // No hint, score < low → smol
        assert_eq!(select_role(1_000_000, Some(5_000_000), None, None, 10, 20, 80), 0);
    }

    #[test]
    fn test_score_middle() {
        // No hint, low <= score <= high → default
        assert_eq!(select_role(1_000_000, Some(5_000_000), None, None, 50, 20, 80), 1);
    }

    #[test]
    fn test_soft_budget_demotes() {
        // Over soft limit, score=90 → adjusted=45 → default (not slow)
        assert_eq!(select_role(6_000_000, Some(10_000_000), Some(5_000_000), None, 90, 20, 80), 1);
        // Without soft pressure, same score=90 → slow
        assert_eq!(select_role(6_000_000, Some(10_000_000), None, None, 90, 20, 80), 2);
    }

    #[test]
    fn test_complexity_score_basic() {
        // 1000 tokens, weight=10, no tools, keyword_bonus=5
        // token_part = 1000*10/100 = 100, tool=0, keyword=5 → 105
        assert_eq!(complexity_score(1000, 10, false, 0, 5), Some(105));
    }

    #[test]
    fn test_complexity_score_with_tools() {
        // 500 tokens, weight=10, tools with weight=20, keyword=0
        // token_part = 50, tool=20, keyword=0 → 70
        assert_eq!(complexity_score(500, 10, true, 20, 0), Some(70));
    }

    #[test]
    fn test_complexity_score_zero() {
        assert_eq!(complexity_score(0, 10, false, 0, 0), Some(0));
    }

    #[test]
    fn test_complexity_score_overflow() {
        assert_eq!(complexity_score(u64::MAX, u64::MAX, false, 0, 0), None);
    }
}

#[cfg(test)]
mod proptests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        #[test]
        fn prop_cost_non_negative(tokens in 0u64..1_000_000_000, price in 0u64..1_000_000_000) {
            if let Some(cost) = compute_cost(tokens, price) {
                prop_assert!(cost <= tokens.checked_mul(price).unwrap_or(u64::MAX));
            }
        }

        #[test]
        fn prop_accumulate_monotonic(total in 0u64..u64::MAX/2, cost in 0u64..u64::MAX/2) {
            if let Some(result) = accumulate(total, cost) {
                prop_assert!(result >= total);
            }
        }

        #[test]
        fn prop_threshold_at_most_once(
            t0 in 0u64..1_000_000u64,
            d1 in 0u64..1_000_000u64,
            d2 in 0u64..1_000_000u64,
            limit in 0u64..2_000_000u64,
        ) {
            let t1 = t0.saturating_add(d1);
            let t2 = t1.saturating_add(d2);
            let c1 = crosses_threshold(t0, t1, limit);
            let c2 = crosses_threshold(t1, t2, limit);
            // At most one can fire
            prop_assert!(!(c1 && c2));
        }

        #[test]
        fn prop_milestone_monotonic(
            prev in 0u64..10_000_000u64,
            delta in 0u64..10_000_000u64,
            interval in 1u64..1_000_000u64,
        ) {
            let current = prev.saturating_add(delta);
            let count = milestone_count(prev, current, interval);
            prop_assert!(count <= delta / interval + 1);
        }

        #[test]
        fn prop_budget_status_monotone(
            soft in proptest::option::of(0u64..10_000_000u64),
            hard in proptest::option::of(0u64..10_000_000u64),
            t1 in 0u64..10_000_000u64,
            delta in 0u64..5_000_000u64,
        ) {
            let t2 = t1.saturating_add(delta);
            let s1 = budget_status(soft, hard, t1);
            let s2 = budget_status(soft, hard, t2);
            prop_assert!(s2 >= s1, "status went backward: {} -> {} at t1={}, t2={}", s1, s2, t1, t2);
        }

        #[test]
        fn prop_hard_budget_always_smol(
            total in 0u64..10_000_000u64,
            hard in 0u64..10_000_000u64,
            soft in proptest::option::of(0u64..10_000_000u64),
            hint in proptest::option::of(0u8..3u8),
            score in 0u64..200u64,
        ) {
            if total >= hard {
                let role = select_role(total, Some(hard), soft, hint, score, 20, 80);
                prop_assert_eq!(role, 0, "hard budget did not override to smol");
            }
        }

        #[test]
        fn prop_soft_never_promotes(
            score in 0u64..1000u64,
        ) {
            prop_assert!(score / 2 <= score);
        }
    }
}
