//! Verified Markov chain prediction primitives.
//!
//! Integer-arithmetic building blocks for transition counting and
//! confidence-threshold prediction. Avoids floating point — confidence
//! is compared as `count * 100 >= total * threshold_pct`.
//!
//! Generalized from seaglass's tiered-cache `AccessPredictor`.

use vstd::prelude::*;

verus! {

// --- confidence_meets ---

// r[depends markov.confidence]
/// Spec: whether a transition's confidence meets the threshold.
/// Integer equivalent of (count / total) >= (threshold_pct / 100).
/// Uses cross-multiplication to avoid division.
pub open spec fn confidence_meets_spec(count: u64, total: u64, threshold_pct: u64) -> bool {
    total > 0 && count as int * 100 >= total as int * threshold_pct as int
}

// r[impl markov.confidence]
/// Returns true if the transition confidence meets the threshold.
///
/// Uses cross-multiplication to avoid division: count * 100 >= total * `threshold_pct`.
/// Returns true if the transition confidence meets the threshold.
pub fn confidence_meets(count: u64, total: u64, threshold_pct: u64) -> (result: bool)
    requires total > 0, threshold_pct <= 100, count <= total
    ensures result == confidence_meets_spec(count, total, threshold_pct)
{
    // count <= total, total <= u64::MAX, so count * 100 <= u64::MAX * 100
    // total * threshold_pct <= u64::MAX * 100
    // Both fit in u128 but we need to stay in u64.
    // Since count <= total and threshold_pct <= 100:
    //   count * 100 <= total * 100 (fits if total <= u64::MAX / 100)
    //   total * threshold_pct <= total * 100
    // Guard overflow: if total > u64::MAX / 100, use division instead.
    if total <= u64::MAX / 100 {
        proof {
            assert(count as int * 100 <= total as int * 100) by(nonlinear_arith)
                requires count <= total;
            assert(total as int * 100 <= u64::MAX as int) by(nonlinear_arith)
                requires total <= u64::MAX / 100;
            assert(total as int * threshold_pct as int <= total as int * 100) by(nonlinear_arith)
                requires threshold_pct <= 100;
        }
        count * 100 >= total * threshold_pct
    } else {
        // Fallback: count / total >= threshold_pct / 100
        // Equivalent: count * 100 / total >= threshold_pct (but we avoid division)
        // Since total > u64::MAX / 100, use: count >= total / 100 * threshold_pct
        // This is slightly conservative. For exactness, check:
        // count * 100 >= total * threshold_pct in int domain.
        // We can check: count >= (total * threshold_pct + 99) / 100
        // which is ceil(total * threshold_pct / 100).
        // But total * threshold_pct might overflow. Since threshold_pct <= 100
        // and total <= u64::MAX, total * threshold_pct <= u64::MAX * 100 which
        // overflows u64. Use the division approach:
        // count * 100 / total >= threshold_pct, but count * 100 overflows too.
        // Final approach: count / (total / 100) >= threshold_pct (approximate)
        // or check count == total (always meets) then count >= total * threshold_pct / 100.
        //
        // Since count <= total and total > u64::MAX/100:
        // threshold_pct / 100 * total <= count requires count ~ total.
        // Use: threshold_pct == 0 || count >= total / 100 * threshold_pct
        //
        // For correctness proof: mark external_body and use proptest.
        // Actually, let's restructure: count / threshold_pct >= total / 100
        // when threshold_pct > 0. When threshold_pct == 0, always true.
        if threshold_pct == 0 {
            true
        } else {
            // count * 100 >= total * threshold_pct
            // ≡ count >= total * threshold_pct / 100
            // Since total * threshold_pct might overflow, compute as:
            // total / 100 * threshold_pct + adjustment
            let q = total / 100;
            let r = total % 100;
            // total * threshold_pct = (q * 100 + r) * threshold_pct
            //                       = q * 100 * threshold_pct + r * threshold_pct
            // count * 100 >= q * 100 * threshold_pct + r * threshold_pct
            // count >= q * threshold_pct + (r * threshold_pct + 99) / 100  [ceiling]
            // But q * threshold_pct might overflow too? q <= u64::MAX/100, threshold_pct <= 100
            // so q * threshold_pct <= u64::MAX/100 * 100 = u64::MAX. Safe.
            proof {
                assert(q as int * threshold_pct as int <= u64::MAX as int) by(nonlinear_arith)
                    requires q <= u64::MAX / 100, threshold_pct <= 100;
                assert(r as int * threshold_pct as int <= 99 * 100) by(nonlinear_arith)
                    requires r < 100, threshold_pct <= 100;
            }
            let main = q * threshold_pct;
            let frac = r * threshold_pct;
            proof {
                // Key identity: total * threshold_pct == main * 100 + frac
                // From fundamental_div_mod: total == q * 100 + r
                vstd::arithmetic::div_mod::lemma_fundamental_div_mod(total as int, 100);
                // total * tp = (q*100 + r) * tp = q*tp*100 + r*tp = main*100 + frac
                assert(total as int * threshold_pct as int
                    == main as int * 100 + frac as int) by(nonlinear_arith)
                    requires total as int == q as int * 100 + r as int,
                             main == q * threshold_pct, frac == r * threshold_pct;
            }
            // count * 100 >= total * threshold_pct
            // <==> count * 100 >= main * 100 + frac
            // <==> (count - main) * 100 >= frac  (when count >= main)
            if count < main {
                proof {
                    // count < main ==> count * 100 < main * 100 <= main * 100 + frac
                    assert(count as int * 100 < main as int * 100) by(nonlinear_arith)
                        requires count < main;
                }
                false
            } else {
                let diff = count - main;
                if diff > u64::MAX / 100 {
                    proof {
                        // diff * 100 > (u64::MAX/100) * 100 >= u64::MAX - 99 > frac
                        assert(diff as int * 100 > frac as int) by(nonlinear_arith)
                            requires diff as int > u64::MAX as int / 100,
                                     frac as int <= 99 * 100;
                        // count * 100 = main * 100 + diff * 100 >= main * 100 + frac
                        assert(count as int * 100 == main as int * 100 + diff as int * 100)
                            by(nonlinear_arith) requires count == main + diff;
                    }
                    true
                } else {
                    proof {
                        assert(diff as int * 100 <= u64::MAX as int) by(nonlinear_arith)
                            requires diff <= u64::MAX / 100;
                        // count * 100 = main * 100 + diff * 100
                        assert(count as int * 100 == main as int * 100 + diff as int * 100)
                            by(nonlinear_arith) requires count == main + diff;
                    }
                    diff * 100 >= frac
                }
            }
        }
    }
}

// r[verify markov.confidence]
/// When count == total, any threshold <= 100 is met.
proof fn full_count_meets_any_threshold(total: u64, threshold_pct: u64)
    requires total > 0, threshold_pct <= 100
    ensures confidence_meets_spec(total, total, threshold_pct)
{
    assert(total as int * 100 >= total as int * threshold_pct as int) by(nonlinear_arith)
        requires threshold_pct <= 100;
}

// r[verify markov.confidence]
/// Zero threshold is always met (when total > 0).
proof fn zero_threshold_always_met(count: u64, total: u64)
    requires total > 0, count <= total
    ensures confidence_meets_spec(count, total, 0)
{
    assert(count as int * 100 >= 0) by(nonlinear_arith);
}

// r[verify markov.confidence]
/// Zero count never meets a positive threshold.
proof fn zero_count_fails_positive_threshold(total: u64, threshold_pct: u64)
    requires total > 0, threshold_pct > 0
    ensures !confidence_meets_spec(0, total, threshold_pct)
{
    assert(0 < total as int * threshold_pct as int) by(nonlinear_arith)
        requires total > 0, threshold_pct > 0;
}

// r[verify markov.confidence]
/// Confidence is monotonic in count: more observations => same or higher confidence.
proof fn confidence_monotonic_in_count(count1: u64, count2: u64, total: u64, threshold_pct: u64)
    requires total > 0, count1 <= count2, count2 <= total, threshold_pct <= 100,
        confidence_meets_spec(count1, total, threshold_pct)
    ensures confidence_meets_spec(count2, total, threshold_pct)
{
    assert(count2 as int * 100 >= count1 as int * 100) by(nonlinear_arith)
        requires count1 <= count2;
}

// r[verify markov.confidence]
/// Confidence is anti-monotonic in threshold: lower threshold => easier to meet.
proof fn confidence_antimonotonic_in_threshold(count: u64, total: u64, t1: u64, t2: u64)
    requires total > 0, count <= total, t1 <= t2, t2 <= 100,
        confidence_meets_spec(count, total, t2)
    ensures confidence_meets_spec(count, total, t1)
{
    assert(total as int * t1 as int <= total as int * t2 as int) by(nonlinear_arith)
        requires t1 <= t2;
}

// --- record_bounded ---

// r[depends markov.record-bounded]
/// Spec: record a transition, incrementing count. Returns new count
/// capped at total (saturating).
pub open spec fn record_transition_spec(count: u64, total: u64, max_total: u64) -> (u64, u64) {
    if total >= max_total {
        (count, total) // At capacity, don't record
    } else {
        ((count + 1) as u64, (total + 1) as u64)
    }
}

// r[impl markov.record-bounded]
/// Records a transition. Increments count and total, unless total has
/// reached `max_total`.
pub fn record_transition(count: u64, total: u64, max_total: u64) -> (result: (u64, u64))
    requires count <= total, total <= max_total
    ensures result == record_transition_spec(count, total, max_total)
{
    if total >= max_total {
        (count, total)
    } else {
        (count + 1, total + 1)
    }
}

// r[verify markov.record-bounded]
/// After recording, count <= total always holds.
proof fn record_maintains_count_le_total(count: u64, total: u64, max_total: u64)
    requires count <= total, total <= max_total
    ensures
        record_transition_spec(count, total, max_total).0
        <= record_transition_spec(count, total, max_total).1,
{}

// r[verify markov.record-bounded]
/// After recording, total <= max_total always holds.
proof fn record_maintains_total_bound(count: u64, total: u64, max_total: u64)
    requires count <= total, total <= max_total
    ensures
        record_transition_spec(count, total, max_total).1 <= max_total,
{}

// r[verify markov.record-bounded]
/// Recording increments count by exactly 1 (when not at capacity).
proof fn record_increments_count(count: u64, total: u64, max_total: u64)
    requires count <= total, total < max_total
    ensures
        record_transition_spec(count, total, max_total).0 == count + 1,
{}

// --- prediction_valid ---

// r[depends markov.prediction-valid]
/// Spec: whether a prediction is valid (positive count, total, count <= total).
pub open spec fn prediction_valid_spec(count: u64, total: u64) -> bool {
    count > 0 && total > 0 && count <= total
}

// r[impl markov.prediction-valid]
/// Returns true if a prediction is valid.
pub fn prediction_valid(count: u64, total: u64) -> (result: bool)
    ensures result == prediction_valid_spec(count, total)
{
    count > 0 && total > 0 && count <= total
}

// r[verify markov.prediction-valid]
/// A valid prediction always meets a zero threshold.
proof fn valid_prediction_meets_zero(count: u64, total: u64)
    requires prediction_valid_spec(count, total)
    ensures confidence_meets_spec(count, total, 0)
{
    assert(count as int * 100 >= 0) by(nonlinear_arith);
}

// r[verify markov.prediction-valid]
/// After recording from zero, the prediction is valid.
proof fn record_from_zero_is_valid(max_total: u64)
    requires max_total > 0
    ensures
        prediction_valid_spec(
            record_transition_spec(0, 0, max_total).0,
            record_transition_spec(0, 0, max_total).1,
        ),
{}

// --- best_prediction ---

// r[depends markov.best-prediction]
/// Spec: find the index of the maximum count in a sequence.
/// Returns the index of the first maximum element.
pub open spec fn best_prediction_index_spec(counts: Seq<u64>) -> int
    recommends counts.len() > 0
{
    if counts.len() == 0 { 0 }
    else {
        best_prediction_index_from_spec(counts, 1, 0)
    }
}

/// Helper: find best index starting from position `from`, with current best at `best_idx`.
pub open spec fn best_prediction_index_from_spec(counts: Seq<u64>, from: int, best_idx: int) -> int
    decreases counts.len() - from
{
    if from >= counts.len() {
        best_idx
    } else if counts[from] > counts[best_idx] {
        best_prediction_index_from_spec(counts, from + 1, from)
    } else {
        best_prediction_index_from_spec(counts, from + 1, best_idx)
    }
}

// r[impl markov.best-prediction]
/// Returns the index of the highest count in the slice.
/// If multiple entries tie, returns the first one.
///
/// Returns the index of the highest count in the slice.
/// If multiple entries tie, returns the first one.
pub fn best_prediction_index(counts: &[u64]) -> (result: usize)
    requires counts@.len() > 0
    ensures
        result < counts@.len(),
        forall|j: int| 0 <= j < counts@.len() ==> counts@[j] <= counts@[result as int],
{
    let mut best_idx: usize = 0;
    let mut i: usize = 1;
    while i < counts.len()
        invariant
            1 <= i <= counts@.len(),
            best_idx < counts@.len(),
            best_idx < i,
            forall|j: int| #![trigger counts@[j]]
                0 <= j < i as int ==> counts@[j] <= counts@[best_idx as int],
        decreases counts@.len() - i,
    {
        if counts[i] > counts[best_idx] {
            best_idx = i;
        }
        i += 1;
    }
    best_idx
}

// r[verify markov.best-prediction]
/// The best prediction index is within bounds.
proof fn best_prediction_in_bounds(counts: Seq<u64>)
    requires counts.len() > 0
    ensures 0 <= best_prediction_index_spec(counts) < counts.len()
    decreases counts.len()
{
    best_prediction_from_in_bounds(counts, 1, 0);
}

/// Helper: best_prediction_index_from_spec always returns a valid index.
proof fn best_prediction_from_in_bounds(counts: Seq<u64>, from: int, best_idx: int)
    requires
        counts.len() > 0,
        0 <= best_idx < counts.len(),
        from >= 1,
    ensures
        0 <= best_prediction_index_from_spec(counts, from, best_idx) < counts.len()
    decreases counts.len() - from
{
    if from >= counts.len() {
        // base case: returns best_idx which is in bounds
    } else if counts[from] > counts[best_idx] {
        best_prediction_from_in_bounds(counts, from + 1, from);
    } else {
        best_prediction_from_in_bounds(counts, from + 1, best_idx);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify markov.confidence]
    #[test]
    fn test_confidence_meets() {
        // 7/8 = 87.5% >= 70%
        assert!(confidence_meets(7, 8, 70));
        // 1/4 = 25% < 70%
        assert!(!confidence_meets(1, 4, 70));
        // 4/4 = 100% >= 100%
        assert!(confidence_meets(4, 4, 100));
        // 0/4 = 0% < 1%
        assert!(!confidence_meets(0, 4, 1));
        // 0/4 = 0% >= 0%
        assert!(confidence_meets(0, 4, 0));
        // 1/1 = 100% >= 70%
        assert!(confidence_meets(1, 1, 70));
        // 3/10 = 30% < 50%
        assert!(!confidence_meets(3, 10, 50));
        // 5/10 = 50% >= 50%
        assert!(confidence_meets(5, 10, 50));
    }

    // r[verify markov.record-bounded]
    #[test]
    fn test_record_transition() {
        let (c, t) = record_transition(0, 0, 100);
        assert_eq!(c, 1);
        assert_eq!(t, 1);

        let (c2, t2) = record_transition(5, 10, 100);
        assert_eq!(c2, 6);
        assert_eq!(t2, 11);

        // At capacity
        let (c3, t3) = record_transition(50, 100, 100);
        assert_eq!(c3, 50);
        assert_eq!(t3, 100);
    }

    // r[verify markov.prediction-valid]
    #[test]
    fn test_prediction_valid() {
        assert!(prediction_valid(1, 1));
        assert!(prediction_valid(5, 10));
        assert!(!prediction_valid(0, 10));
        assert!(!prediction_valid(0, 0));
        assert!(!prediction_valid(11, 10)); // count > total
    }

    // r[verify markov.best-prediction]
    #[test]
    fn test_best_prediction_index() {
        assert_eq!(best_prediction_index(&[5, 3, 1]), 0);
        assert_eq!(best_prediction_index(&[1, 5, 3]), 1);
        assert_eq!(best_prediction_index(&[1, 3, 5]), 2);
        assert_eq!(best_prediction_index(&[5]), 0);
        assert_eq!(best_prediction_index(&[3, 3, 3]), 0); // First of ties
    }

    proptest! {
        // r[verify markov.confidence]
        #[test]
        fn prop_full_count_meets_threshold(
            total in 1u64..=1_000_000,
            threshold_pct in 0u64..=100,
        ) {
            prop_assert!(confidence_meets(total, total, threshold_pct));
        }

        // r[verify markov.confidence]
        #[test]
        fn prop_zero_count_fails_positive_threshold(
            total in 1u64..=1_000_000,
            threshold_pct in 1u64..=100,
        ) {
            prop_assert!(!confidence_meets(0, total, threshold_pct));
        }

        // r[verify markov.confidence]
        #[test]
        fn prop_confidence_monotonic_in_count(
            count1 in 0u64..=500,
            count2 in 0u64..=500,
            total in 1u64..=1000,
            threshold_pct in 0u64..=100,
        ) {
            let lo = count1.min(count2).min(total);
            let hi = count1.max(count2).min(total);
            if confidence_meets(lo, total, threshold_pct) {
                prop_assert!(confidence_meets(hi, total, threshold_pct));
            }
        }

        // r[verify markov.confidence]
        #[test]
        fn prop_confidence_antimonotonic_in_threshold(
            count in 0u64..=1000,
            total in 1u64..=1000,
            t1 in 0u64..=100,
            t2 in 0u64..=100,
        ) {
            let count = count.min(total);
            let lo = t1.min(t2);
            let hi = t1.max(t2);
            if confidence_meets(count, total, hi) {
                prop_assert!(confidence_meets(count, total, lo));
            }
        }

        // r[verify markov.record-bounded]
        #[test]
        fn prop_record_maintains_invariants(
            count in 0u64..=1000,
            total in 0u64..=1000,
            max_total in 0u64..=2000,
        ) {
            let total = total.min(max_total);
            let count = count.min(total);
            let (new_count, new_total) = record_transition(count, total, max_total);
            prop_assert!(new_count <= new_total);
            prop_assert!(new_total <= max_total);
        }

        // r[verify markov.best-prediction]
        #[test]
        fn prop_best_prediction_in_bounds(
            counts in proptest::collection::vec(0u64..=1000, 1..20),
        ) {
            let idx = best_prediction_index(&counts);
            prop_assert!(idx < counts.len());
            // Verify it's actually the max
            for &c in &counts {
                prop_assert!(c <= counts[idx]);
            }
        }

        // r[verify markov.confidence]
        #[test]
        fn prop_confidence_matches_float(
            count in 0u64..=1000,
            total in 1u64..=1000,
            threshold_pct in 0u64..=100,
        ) {
            let count = count.min(total);
            #[allow(clippy::cast_precision_loss)]
            let float_confidence = count as f64 / total as f64;
            #[allow(clippy::cast_precision_loss)]
            let float_threshold = threshold_pct as f64 / 100.0_f64;
            let expected = float_confidence >= float_threshold;
            let actual = confidence_meets(count, total, threshold_pct);
            // Should agree (modulo floating point rounding at boundary)
            if (float_confidence - float_threshold).abs() > 0.001 {
                prop_assert_eq!(actual, expected,
                    "count={}, total={}, pct={}, float_conf={}, float_thresh={}",
                    count, total, threshold_pct, float_confidence, float_threshold);
            }
        }
    }
}
