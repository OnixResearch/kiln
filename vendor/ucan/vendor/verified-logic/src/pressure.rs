//! Verified multi-threshold pressure/severity classification.
//!
//! Functions for classifying values into severity levels (Normal/Warning/Critical)
//! based on thresholds. Provides predicates for decision-making and aggregation
//! functions for combining multiple pressure readings. Every function has a
//! machine-checked spec and proven properties.

use vstd::prelude::*;

verus! {

// --- classify_level ---

// r[depends pressure.classify]
/// Spec: classify value into pressure levels based on thresholds.
/// Returns 0 (Normal) if value < warn, 1 (Warning) if warn <= value < crit,
/// 2 (Critical) if value >= crit.
pub open spec fn classify_level_spec(value: u64, warn_threshold: u64, crit_threshold: u64) -> u8 {
    if value < warn_threshold {
        0
    } else if value < crit_threshold {
        1
    } else {
        2
    }
}

// r[impl pressure.classify]
/// Returns pressure level (0=Normal, 1=Warning, 2=Critical) for the given value.
///
/// Thresholds must be ordered: `warn_threshold` <= `crit_threshold`.
/// Classification is exclusive: exactly one level applies to any value.
pub fn classify_level(value: u64, warn_threshold: u64, crit_threshold: u64) -> (result: u8)
    requires warn_threshold <= crit_threshold
    ensures result == classify_level_spec(value, warn_threshold, crit_threshold)
{
    if value < warn_threshold {
        0
    } else if value < crit_threshold {
        1
    } else {
        2
    }
}

// r[verify pressure.classify]
/// Classification result is always valid (0, 1, or 2).
proof fn classify_result_bounded(value: u64, warn_threshold: u64, crit_threshold: u64)
    requires warn_threshold <= crit_threshold
    ensures classify_level_spec(value, warn_threshold, crit_threshold) <= 2
{}

// r[verify pressure.classify]
/// Classification is monotonic: higher value => same or higher level.
proof fn classify_monotonic(
    value1: u64,
    value2: u64,
    warn_threshold: u64,
    crit_threshold: u64,
)
    requires
        warn_threshold <= crit_threshold,
        value1 <= value2,
    ensures
        classify_level_spec(value1, warn_threshold, crit_threshold) <=
        classify_level_spec(value2, warn_threshold, crit_threshold),
{}

// r[verify pressure.classify]
/// Values below warning threshold are always Normal.
proof fn below_warning_is_normal(value: u64, warn_threshold: u64, crit_threshold: u64)
    requires warn_threshold <= crit_threshold, value < warn_threshold
    ensures classify_level_spec(value, warn_threshold, crit_threshold) == 0
{}

// r[verify pressure.classify]
/// Values at or above critical threshold are always Critical.
proof fn above_critical_is_critical(value: u64, warn_threshold: u64, crit_threshold: u64)
    requires warn_threshold <= crit_threshold, value >= crit_threshold
    ensures classify_level_spec(value, warn_threshold, crit_threshold) == 2
{}

// --- is_critical ---

// r[depends pressure.is-critical]
/// Spec: whether the pressure level is Critical (level 2).
pub open spec fn is_critical_spec(level: u8) -> bool {
    level == 2
}

// r[impl pressure.is-critical]
/// Returns true if the pressure level is Critical (level 2).
pub fn is_critical(level: u8) -> (result: bool)
    ensures result == is_critical_spec(level)
{
    level == 2
}

// --- is_warning_or_above ---

// r[depends pressure.is-critical]
/// Spec: whether the pressure level is Warning (1) or Critical (2).
pub open spec fn is_warning_or_above_spec(level: u8) -> bool {
    level >= 1
}

// r[impl pressure.is-critical]
/// Returns true if the pressure level is Warning or Critical (level >= 1).
pub fn is_warning_or_above(level: u8) -> (result: bool)
    ensures result == is_warning_or_above_spec(level)
{
    level >= 1
}

// --- should_pause ---

// r[depends pressure.should-pause]
/// Spec: whether operations should pause due to critical pressure.
pub open spec fn should_pause_spec(level: u8) -> bool {
    level >= 2
}

// r[impl pressure.should-pause]
/// Returns true if operations should pause (level >= 2, i.e., Critical).
pub fn should_pause(level: u8) -> (result: bool)
    ensures result == should_pause_spec(level)
{
    level >= 2
}

// r[verify pressure.is-critical]
// r[verify pressure.should-pause]
/// Critical level implies should pause.
proof fn critical_implies_pause(level: u8)
    requires is_critical_spec(level)
    ensures should_pause_spec(level)
{}

// r[verify pressure.should-pause]
/// Only Critical level (2) triggers pause, not Warning (1).
proof fn warning_no_pause()
    ensures !should_pause_spec(1), should_pause_spec(2)
{}

// --- most_severe ---

// r[depends pressure.most-severe]
/// Spec: maximum of two pressure levels, capped at 2.
pub open spec fn most_severe_spec(a: u8, b: u8) -> u8 {
    if a > b { if a > 2 { 2 } else { a } } else { if b > 2 { 2 } else { b } }
}

// r[impl pressure.most-severe]
/// Returns the more severe of two pressure levels, capped at 2.
///
/// Combines pressure readings to determine overall system pressure.
/// Result is always bounded to valid pressure levels (≤ 2).
pub fn most_severe(a: u8, b: u8) -> (result: u8)
    ensures result == most_severe_spec(a, b)
{
    let max_val = if a > b { a } else { b };
    if max_val > 2 { 2 } else { max_val }
}

// r[verify pressure.most-severe]
/// most_severe is commutative.
proof fn most_severe_commutative(a: u8, b: u8)
    ensures most_severe_spec(a, b) == most_severe_spec(b, a)
{}

// r[verify pressure.most-severe]
/// most_severe is associative.
proof fn most_severe_associative(a: u8, b: u8, c: u8)
    ensures most_severe_spec(most_severe_spec(a, b), c) == most_severe_spec(a, most_severe_spec(b, c))
{}

// r[verify pressure.most-severe]
/// most_severe is idempotent.
proof fn most_severe_idempotent(a: u8)
    ensures most_severe_spec(a, a) == a || most_severe_spec(a, a) == 2
{}

// r[verify pressure.most-severe]
/// most_severe result is at least as severe as both inputs.
proof fn most_severe_ge_both(a: u8, b: u8)
    ensures
        most_severe_spec(a, b) >= a || most_severe_spec(a, b) == 2,
        most_severe_spec(a, b) >= b || most_severe_spec(a, b) == 2,
{}

// r[verify pressure.most-severe]
/// most_severe result is bounded.
proof fn most_severe_bounded(a: u8, b: u8)
    ensures most_severe_spec(a, b) <= 2
{}

// --- is_all_normal ---

// --- Algebraic characterization ---

// r[verify pressure.most-severe]
/// most_severe is a semilattice over valid pressure levels (0, 1, 2).
/// For arbitrary u8 inputs, it still satisfies comm+assoc but idempotence
/// holds up to capping: most_severe(a, a) == a when a <= 2.
proof fn most_severe_is_semilattice_on_valid()
    ensures
        // Commutativity (unconditional)
        forall|a: u8, b: u8| #[trigger] most_severe_spec(a, b) == most_severe_spec(b, a),
        // Associativity (unconditional)
        forall|a: u8, b: u8, c: u8|
            #[trigger] most_severe_spec(most_severe_spec(a, b), c)
            == most_severe_spec(a, most_severe_spec(b, c)),
        // Idempotence for valid levels
        forall|a: u8| a <= 2 ==> (#[trigger] most_severe_spec(a, a) == a),
{}

// r[depends pressure.is-all-normal]
/// Spec: whether all pressure levels in a slice are Normal (0).
pub open spec fn is_all_normal_spec(levels: Seq<u8>) -> bool {
    forall|i: int| 0 <= i < levels.len() ==> levels[i] == 0
}

// r[impl pressure.is-all-normal]
/// Returns true if all pressure levels are Normal (level 0).
///
/// Empty slice is considered all normal by vacuous truth.
/// Useful for system-wide health verification.
pub fn is_all_normal(levels: &[u8]) -> (result: bool)
    ensures result == is_all_normal_spec(levels@)
{
    let mut i = 0;
    while i < levels.len()
        invariant
            0 <= i <= levels.len(),
            forall|j: int| 0 <= j < i ==> levels[j] == 0,
        decreases levels.len() - i,
    {
        if levels[i] != 0 {
            return false;
        }
        i += 1;
    }
    true
}

// r[verify pressure.is-all-normal]
/// Empty slice is all normal.
proof fn empty_all_normal()
    ensures is_all_normal_spec(Seq::empty())
{}

// r[verify pressure.is-all-normal]
// r[verify pressure.should-pause]
/// If all levels are normal, none should cause a pause.
proof fn all_normal_no_pause(levels: Seq<u8>)
    requires is_all_normal_spec(levels)
    ensures forall|i: int| #![trigger levels[i]] 0 <= i < levels.len() ==> !should_pause_spec(levels[i])
{}

// r[verify pressure.is-all-normal]
/// Single normal level is all normal.
proof fn single_normal_all_normal()
    ensures is_all_normal_spec(seq![0u8])
{}

// r[verify pressure.is-all-normal]
/// Any non-zero level breaks all normal.
proof fn non_zero_not_all_normal(level: u8)
    requires level != 0
    ensures !is_all_normal_spec(seq![level])
{
    let seq_with_level = seq![level];
    // The sequence has length 1, and seq_with_level[0] == level != 0
    assert(seq_with_level.len() == 1);
    assert(seq_with_level[0] == level);
    assert(seq_with_level[0] != 0);
    // Therefore, the forall condition is false for i = 0
    assert(!(forall|i: int| 0 <= i < seq_with_level.len() ==> seq_with_level[i] == 0));
}

// --- combined properties ---

// r[verify pressure.classify]
// r[verify pressure.is-critical]
/// Classifying at critical threshold yields critical level.
proof fn classify_at_critical_is_critical(warn_threshold: u64, crit_threshold: u64)
    requires warn_threshold <= crit_threshold
    ensures is_critical_spec(classify_level_spec(crit_threshold, warn_threshold, crit_threshold))
{}

// r[verify pressure.classify]
// r[verify pressure.should-pause]
/// Values at or above critical threshold should pause.
proof fn classify_critical_should_pause(value: u64, warn_threshold: u64, crit_threshold: u64)
    requires warn_threshold <= crit_threshold, value >= crit_threshold
    ensures should_pause_spec(classify_level_spec(value, warn_threshold, crit_threshold))
{}

// r[verify pressure.classify]
// r[verify pressure.most-severe]
/// Classification and most_severe are consistent for threshold comparison.
proof fn classify_most_severe_consistent(
    value1: u64,
    value2: u64,
    warn_threshold: u64,
    crit_threshold: u64,
)
    requires warn_threshold <= crit_threshold
    ensures
        most_severe_spec(
            classify_level_spec(value1, warn_threshold, crit_threshold),
            classify_level_spec(value2, warn_threshold, crit_threshold),
        ) == classify_level_spec(
            if value1 > value2 { value1 } else { value2 },
            warn_threshold,
            crit_threshold,
        ),
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify pressure.classify]
    #[test]
    fn test_classify_level() {
        assert_eq!(classify_level(5, 10, 20), 0); // Normal: below warning
        assert_eq!(classify_level(15, 10, 20), 1); // Warning: between thresholds
        assert_eq!(classify_level(25, 10, 20), 2); // Critical: above critical
        assert_eq!(classify_level(10, 10, 20), 1); // Boundary: at warning
        assert_eq!(classify_level(20, 10, 20), 2); // Boundary: at critical
        // Equal thresholds
        assert_eq!(classify_level(5, 10, 10), 0); // Below
        assert_eq!(classify_level(10, 10, 10), 2); // At threshold
        assert_eq!(classify_level(15, 10, 10), 2); // Above
    }

    // r[verify pressure.is-critical]
    #[test]
    fn test_is_critical() {
        assert!(!is_critical(0));
        assert!(!is_critical(1));
        assert!(is_critical(2));
        assert!(!is_critical(3)); // Invalid but handled
    }

    // r[verify pressure.is-critical]
    #[test]
    fn test_is_warning_or_above() {
        assert!(!is_warning_or_above(0));
        assert!(is_warning_or_above(1));
        assert!(is_warning_or_above(2));
        assert!(is_warning_or_above(3)); // Invalid but handled
    }

    // r[verify pressure.should-pause]
    #[test]
    fn test_should_pause() {
        assert!(!should_pause(0));
        assert!(!should_pause(1));
        assert!(should_pause(2));
        assert!(should_pause(3)); // Invalid but handled
    }

    // r[verify pressure.most-severe]
    #[test]
    fn test_most_severe() {
        assert_eq!(most_severe(0, 1), 1);
        assert_eq!(most_severe(1, 0), 1);
        assert_eq!(most_severe(1, 2), 2);
        assert_eq!(most_severe(2, 1), 2);
        assert_eq!(most_severe(0, 0), 0);
        assert_eq!(most_severe(2, 2), 2);
        // Capping at 2
        assert_eq!(most_severe(3, 1), 2);
        assert_eq!(most_severe(1, 5), 2);
        assert_eq!(most_severe(10, 20), 2);
    }

    // r[verify pressure.is-all-normal]
    #[test]
    fn test_is_all_normal() {
        assert!(is_all_normal(&[])); // Empty
        assert!(is_all_normal(&[0])); // Single normal
        assert!(is_all_normal(&[0, 0, 0])); // Multiple normal
        assert!(!is_all_normal(&[1])); // Single warning
        assert!(!is_all_normal(&[0, 1])); // Mixed with warning
        assert!(!is_all_normal(&[0, 0, 2])); // Mixed with critical
        assert!(!is_all_normal(&[1, 1, 1])); // All warning
        assert!(!is_all_normal(&[2, 2])); // All critical
    }

    proptest! {
        // r[verify pressure.classify]
        #[test]
        fn prop_classify_bounded(
            value: u64,
            warn_threshold: u64,
            crit_threshold in 0u64..=u64::MAX,
        ) {
            let warn = warn_threshold.min(crit_threshold);
            let crit = warn_threshold.max(crit_threshold);
            let level = classify_level(value, warn, crit);
            prop_assert!(level <= 2);
        }

        // r[verify pressure.classify]
        #[test]
        fn prop_classify_monotonic(
            value1: u64,
            value2 in 0u64..=u64::MAX,
            warn_threshold: u64,
            crit_threshold in 0u64..=u64::MAX,
        ) {
            let warn = warn_threshold.min(crit_threshold);
            let crit = warn_threshold.max(crit_threshold);
            let v1 = value1.min(value2);
            let v2 = value1.max(value2);
            let level1 = classify_level(v1, warn, crit);
            let level2 = classify_level(v2, warn, crit);
            prop_assert!(level1 <= level2);
        }

        // r[verify pressure.classify]
        // r[verify pressure.is-critical]
        #[test]
        fn prop_critical_at_threshold(warn_threshold: u64, crit_threshold in 0u64..=u64::MAX) {
            let warn = warn_threshold.min(crit_threshold);
            let crit = warn_threshold.max(crit_threshold);
            let level = classify_level(crit, warn, crit);
            prop_assert!(is_critical(level));
        }

        // r[verify pressure.classify]
        // r[verify pressure.should-pause]
        #[test]
        fn prop_above_critical_should_pause(
            value in 0u64..=u64::MAX,
            warn_threshold: u64,
            crit_threshold in 0u64..=u64::MAX,
        ) {
            let warn = warn_threshold.min(crit_threshold);
            let crit = warn_threshold.max(crit_threshold);
            if value >= crit {
                let level = classify_level(value, warn, crit);
                prop_assert!(should_pause(level));
            }
        }

        // r[verify pressure.most-severe]
        #[test]
        fn prop_most_severe_commutative(a: u8, b: u8) {
            prop_assert_eq!(most_severe(a, b), most_severe(b, a));
        }

        // r[verify pressure.most-severe]
        #[test]
        fn prop_most_severe_associative(a: u8, b: u8, c: u8) {
            prop_assert_eq!(
                most_severe(most_severe(a, b), c),
                most_severe(a, most_severe(b, c))
            );
        }

        // r[verify pressure.most-severe]
        #[test]
        fn prop_most_severe_idempotent(a: u8) {
            let result = most_severe(a, a);
            prop_assert!(result == a || result == 2); // Capped at 2
        }

        // r[verify pressure.most-severe]
        #[test]
        fn prop_most_severe_ge_inputs(a: u8, b: u8) {
            let result = most_severe(a, b);
            prop_assert!(result >= a || result == 2);
            prop_assert!(result >= b || result == 2);
            prop_assert!(result <= 2);
        }

        // r[verify pressure.is-all-normal]
        #[test]
        fn prop_all_normal_no_pause(levels: Vec<u8>) {
            if is_all_normal(&levels) {
                for &level in &levels {
                    prop_assert!(!should_pause(level));
                }
            }
        }

        // r[verify pressure.is-all-normal]
        #[test]
        fn prop_any_non_zero_breaks_all_normal(mut levels: Vec<u8>, idx: usize, level in 1u8..=255) {
            if !levels.is_empty() {
                let idx = idx % levels.len();
                levels[idx] = level;
                prop_assert!(!is_all_normal(&levels));
            }
        }

        // r[verify pressure.classify]
        // r[verify pressure.most-severe]
        #[test]
        fn prop_classify_most_severe_max(
            value1: u64,
            value2: u64,
            warn_threshold: u64,
            crit_threshold in 0u64..=u64::MAX,
        ) {
            let warn = warn_threshold.min(crit_threshold);
            let crit = warn_threshold.max(crit_threshold);
            let level1 = classify_level(value1, warn, crit);
            let level2 = classify_level(value2, warn, crit);
            let combined = most_severe(level1, level2);
            let max_value = value1.max(value2);
            let max_level = classify_level(max_value, warn, crit);
            prop_assert_eq!(combined, max_level);
        }

        // r[verify pressure.is-critical]
        // r[verify pressure.should-pause]
        #[test]
        fn prop_critical_implies_pause(level: u8) {
            if is_critical(level) {
                prop_assert!(should_pause(level));
            }
        }

        // r[verify pressure.classify]
        #[test]
        fn prop_threshold_order_consistency(value: u64, threshold: u64) {
            // When warn == crit, only Normal (0) and Critical (2) exist
            let level = classify_level(value, threshold, threshold);
            prop_assert!(level == 0 || level == 2);
            if value < threshold {
                prop_assert_eq!(level, 0);
            } else {
                prop_assert_eq!(level, 2);
            }
        }
    }
}
