//! Verified learner catch-up readiness for Raft membership changes.
//!
//! Before promoting a learner (non-voting member) to a full voter,
//! the leader must confirm the learner has replicated enough of the
//! log. Promoting a far-behind learner would temporarily reduce the
//! cluster's effective fault tolerance until the new voter catches up.

use vstd::prelude::*;

verus! {

// --- is_caught_up ---

// r[depends learner-readiness.is-caught-up]
/// Spec: has the learner replicated all entries?
pub open spec fn is_caught_up_spec(
    learner_match: u64,
    leader_log_len: u64,
) -> bool {
    learner_match >= leader_log_len
}

// r[impl learner-readiness.is-caught-up]
/// Check if the learner has caught up to the leader.
pub fn is_caught_up(
    learner_match: u64,
    leader_log_len: u64,
) -> (result: bool)
    ensures result == is_caught_up_spec(learner_match, leader_log_len)
{
    learner_match >= leader_log_len
}

// --- replication_gap ---

// r[depends learner-readiness.replication-gap]
/// Spec: how many entries does the learner still need?
pub open spec fn replication_gap_spec(
    learner_match: u64,
    leader_log_len: u64,
) -> u64 {
    if leader_log_len >= learner_match {
        (leader_log_len - learner_match) as u64
    } else {
        0u64
    }
}

// r[impl learner-readiness.replication-gap]
/// Compute the replication gap.
pub fn replication_gap(
    learner_match: u64,
    leader_log_len: u64,
) -> (result: u64)
    ensures result == replication_gap_spec(learner_match, leader_log_len)
{
    if leader_log_len >= learner_match {
        leader_log_len - learner_match
    } else {
        0
    }
}

// --- is_within_threshold ---

// r[depends learner-readiness.is-within-threshold]
/// Spec: is the learner close enough to promote?
pub open spec fn is_within_threshold_spec(
    learner_match: u64,
    leader_log_len: u64,
    threshold: u64,
) -> bool {
    replication_gap_spec(learner_match, leader_log_len) <= threshold
}

// r[impl learner-readiness.is-within-threshold]
/// Check if the learner is within the promotion threshold.
pub fn is_within_threshold(
    learner_match: u64,
    leader_log_len: u64,
    threshold: u64,
) -> (result: bool)
    ensures result == is_within_threshold_spec(learner_match, leader_log_len, threshold)
{
    replication_gap(learner_match, leader_log_len) <= threshold
}

// --- promotion_safe ---

// r[depends learner-readiness.promotion-safe]
/// Spec: is it safe to promote this learner to voter?
pub open spec fn promotion_safe_spec(
    learner_match: u64,
    leader_log_len: u64,
    cluster_size: u64,
) -> bool {
    is_caught_up_spec(learner_match, leader_log_len) && cluster_size > 0
}

// r[impl learner-readiness.promotion-safe]
/// Check if promoting this learner is safe.
pub fn promotion_safe(
    learner_match: u64,
    leader_log_len: u64,
    cluster_size: u64,
) -> (result: bool)
    ensures result == promotion_safe_spec(learner_match, leader_log_len, cluster_size)
{
    is_caught_up(learner_match, leader_log_len) && cluster_size > 0
}

// --- Safety proofs ---

// r[verify learner-readiness.is-caught-up]
/// Caught up means gap is zero.
proof fn caught_up_zero_gap(learner_match: u64, leader_log_len: u64)
    requires is_caught_up_spec(learner_match, leader_log_len)
    ensures replication_gap_spec(learner_match, leader_log_len) == 0
{}

// r[verify learner-readiness.is-caught-up]
/// Not caught up means positive gap.
proof fn not_caught_up_positive_gap(learner_match: u64, leader_log_len: u64)
    requires !is_caught_up_spec(learner_match, leader_log_len)
    ensures replication_gap_spec(learner_match, leader_log_len) > 0
{}

// r[verify learner-readiness.replication-gap]
/// Gap is zero when learner is at or past the leader.
proof fn gap_zero_when_caught_up(learner_match: u64, leader_log_len: u64)
    requires learner_match >= leader_log_len
    ensures replication_gap_spec(learner_match, leader_log_len) == 0
{}

// r[impl learner-readiness.gap-monotonic]
// r[verify learner-readiness.gap-monotonic]
/// Increasing learner_match decreases gap.
proof fn gap_monotonic(m1: u64, m2: u64, leader_log_len: u64)
    requires m2 >= m1
    ensures replication_gap_spec(m2, leader_log_len)
        <= replication_gap_spec(m1, leader_log_len)
{}

// r[verify learner-readiness.is-within-threshold]
/// Caught up implies within any threshold.
proof fn caught_up_within_threshold(learner_match: u64, leader_log_len: u64, threshold: u64)
    requires is_caught_up_spec(learner_match, leader_log_len)
    ensures is_within_threshold_spec(learner_match, leader_log_len, threshold)
{}

// r[verify learner-readiness.promotion-safe]
/// Promotion requires caught up.
proof fn promotion_requires_caught_up(
    learner_match: u64,
    leader_log_len: u64,
    cluster_size: u64,
)
    requires
        promotion_safe_spec(learner_match, leader_log_len, cluster_size)
    ensures
        is_caught_up_spec(learner_match, leader_log_len)
{}

// r[verify learner-readiness.promotion-safe]
/// Empty cluster never allows promotion.
proof fn empty_cluster_no_promotion(learner_match: u64, leader_log_len: u64)
    ensures !promotion_safe_spec(learner_match, leader_log_len, 0)
{}

// r[verify learner-readiness.is-within-threshold]
/// Within zero threshold means caught up.
proof fn zero_threshold_means_caught_up(learner_match: u64, leader_log_len: u64)
    requires is_within_threshold_spec(learner_match, leader_log_len, 0)
    ensures is_caught_up_spec(learner_match, leader_log_len)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify learner-readiness.is-caught-up]
    #[test]
    fn test_is_caught_up() {
        assert!(is_caught_up(10, 10));
        assert!(is_caught_up(15, 10));
        assert!(!is_caught_up(5, 10));
        assert!(is_caught_up(0, 0));
    }

    // r[verify learner-readiness.replication-gap]
    #[test]
    fn test_replication_gap() {
        assert_eq!(replication_gap(5, 10), 5);
        assert_eq!(replication_gap(10, 10), 0);
        assert_eq!(replication_gap(15, 10), 0);
        assert_eq!(replication_gap(0, 100), 100);
    }

    // r[verify learner-readiness.is-within-threshold]
    #[test]
    fn test_is_within_threshold() {
        assert!(is_within_threshold(8, 10, 5)); // gap 2 <= 5
        assert!(is_within_threshold(10, 10, 0)); // gap 0 <= 0
        assert!(!is_within_threshold(5, 10, 3)); // gap 5 > 3
    }

    // r[verify learner-readiness.promotion-safe]
    #[test]
    fn test_promotion_safe() {
        assert!(promotion_safe(10, 10, 5));
        assert!(!promotion_safe(5, 10, 5)); // not caught up
        assert!(!promotion_safe(10, 10, 0)); // empty cluster
    }

    proptest! {
        // r[verify learner-readiness.is-caught-up]
        #[test]
        fn prop_caught_up_reflexive(index: u64) {
            prop_assert!(is_caught_up(index, index));
        }

        // r[verify learner-readiness.replication-gap]
        #[test]
        fn prop_gap_nonneg(learner: u64, leader: u64) {
            let gap = replication_gap(learner, leader);
            // gap is u64, always >= 0
            prop_assert!(gap <= leader);
        }

        // r[verify learner-readiness.gap-monotonic]
        #[test]
        fn prop_gap_monotonic(m1: u64, delta in 0u64..1000, leader: u64) {
            let m2 = m1.saturating_add(delta);
            prop_assert!(replication_gap(m2, leader) <= replication_gap(m1, leader));
        }

        // r[verify learner-readiness.is-within-threshold]
        #[test]
        fn prop_caught_up_within_any_threshold(index: u64, threshold: u64) {
            prop_assert!(is_within_threshold(index, index, threshold));
        }

        // r[verify learner-readiness.promotion-safe]
        #[test]
        fn prop_promotion_needs_caught_up(
            learner: u64,
            leader: u64,
            cluster in 1u64..100,
        ) {
            if promotion_safe(learner, leader, cluster) {
                prop_assert!(is_caught_up(learner, leader));
            }
        }
    }
}
