//! Verified quorum arithmetic.
//!
//! Majority threshold, quorum predicates, and the intersection theorem:
//! two quorums of the same set always share at least one member. This is
//! the pigeonhole argument at the heart of Raft's safety proof.

#![allow(clippy::int_plus_one)]

use vstd::prelude::*;

verus! {

// --- majority ---

// r[depends quorum.majority]
/// Spec: smallest count constituting a strict majority of `n`.
pub open spec fn majority_spec(n: u64) -> u64
    recommends n > 0
{
    (n / 2 + 1) as u64
}

// r[impl quorum.majority]
/// Returns the majority threshold for `n` members.
pub fn majority(n: u64) -> (result: u64)
    requires n > 0
    ensures result == majority_spec(n)
{
    n / 2 + 1
}

// r[verify quorum.majority]
/// majority(n) is at least 1 for any n > 0.
proof fn majority_ge_one(n: u64)
    requires n > 0
    ensures majority_spec(n) >= 1
{}

// r[verify quorum.majority]
/// majority(n) is at most n for any n > 0.
proof fn majority_le_n(n: u64)
    requires n > 0
    ensures majority_spec(n) <= n
{
    assert(n / 2 + 1 <= n) by(nonlinear_arith)
        requires n > 0;
}

// r[verify quorum.majority]
/// majority(n) is strictly more than half.
proof fn majority_gt_half(n: u64)
    requires n > 0
    ensures majority_spec(n) as int * 2 > n as int
{
    assert((n as int / 2 + 1) * 2 > n as int) by(nonlinear_arith)
        requires n > 0;
}

// --- is_quorum ---

// r[depends quorum.is-quorum]
/// Spec: count constitutes a quorum of total.
pub open spec fn is_quorum_spec(count: u64, total: u64) -> bool
    recommends total > 0
{
    count >= majority_spec(total)
}

// r[impl quorum.is-quorum]
/// Returns true if `count` forms a quorum of `total` members.
pub fn is_quorum(count: u64, total: u64) -> (result: bool)
    requires total > 0
    ensures result == is_quorum_spec(count, total)
{
    count >= majority(total)
}

// r[verify quorum.is-quorum]
/// A full set always forms a quorum.
pub proof fn full_is_quorum(n: u64)
    requires n > 0
    ensures is_quorum_spec(n, n)
{
    majority_le_n(n);
}

// r[verify quorum.is-quorum]
/// A single member of a single-node cluster is a quorum.
proof fn singleton_quorum()
    ensures is_quorum_spec(1, 1)
{}

// --- intersection theorem ---

// r[depends quorum.intersection]
/// Spec: two quorums overlap — their combined count exceeds total.
pub open spec fn quorum_intersection_prop(a: u64, b: u64, n: u64) -> bool
    recommends n > 0
{
    (is_quorum_spec(a, n) && is_quorum_spec(b, n))
        ==> a as int + b as int > n as int
}

// r[impl quorum.intersection]
// r[verify quorum.intersection]
/// THE safety theorem: two quorums of the same set always share a member.
///
/// Proof: each quorum has at least n/2 + 1 members. Together they account
/// for at least 2*(n/2 + 1) = n + 2 - (n%2) > n slots in an n-member set.
/// By pigeonhole, at least one member belongs to both.
pub proof fn prove_quorum_intersection(a: u64, b: u64, n: u64)
    requires n > 0, is_quorum_spec(a, n), is_quorum_spec(b, n)
    ensures a as int + b as int > n as int
{
    // a >= n/2 + 1, b >= n/2 + 1
    // a + b >= 2*(n/2 + 1) = 2*(n/2) + 2
    // n = 2*(n/2) + n%2, so 2*(n/2) = n - n%2
    // a + b >= n - n%2 + 2 >= n + 1 > n
    assert((n as int / 2 + 1) + (n as int / 2 + 1) > n as int) by(nonlinear_arith)
        requires n > 0;
}

// --- fault_tolerance ---

// r[depends quorum.fault-tolerance]
/// Spec: maximum failures tolerable by a cluster of n.
pub open spec fn fault_tolerance_spec(n: u64) -> u64 {
    if n == 0 { 0 } else { ((n - 1) / 2) as u64 }
}

// r[impl quorum.fault-tolerance]
/// Returns the number of failures a cluster of `n` can tolerate.
///
/// Formula: (n - 1) / 2. Matches aspen's `fault_tolerance_for_size`.
pub fn fault_tolerance(n: u64) -> (result: u64)
    ensures result == fault_tolerance_spec(n)
{
    if n == 0 { 0 } else { (n - 1) / 2 }
}

// r[verify quorum.fault-tolerance]
/// After fault_tolerance failures, quorum still holds:
/// n - fault_tolerance(n) >= majority(n).
proof fn fault_tolerance_leaves_quorum(n: u64)
    requires n > 0
    ensures n as int - fault_tolerance_spec(n) as int >= majority_spec(n) as int
{
    assert(n as int - (n as int - 1) / 2 >= n as int / 2 + 1) by(nonlinear_arith)
        requires n > 0;
}

// r[verify quorum.fault-tolerance]
/// fault_tolerance is bounded: 0 <= fault_tolerance(n) < n for n > 0.
proof fn fault_tolerance_bounded(n: u64)
    requires n > 0
    ensures
        fault_tolerance_spec(n) >= 0,
        (fault_tolerance_spec(n) as int) < n as int,
{}

// --- can_remove_safely ---

// r[depends quorum.can-remove-safely]
/// Spec: removing one voter still leaves a quorum.
pub open spec fn can_remove_safely_spec(n: u64) -> bool {
    n > 0 && (n - 1) as u64 >= majority_spec(n)
}

// r[impl quorum.can-remove-safely]
/// Returns true if removing one voter from a cluster of `n` still
/// leaves enough nodes for quorum.
///
/// Matches aspen's `can_remove_voter_safely`.
pub fn can_remove_safely(n: u64) -> (result: bool)
    ensures result == can_remove_safely_spec(n)
{
    if n == 0 {
        false
    } else {
        n - 1 >= majority(n)
    }
}

// r[verify quorum.can-remove-safely]
/// Clusters of 3 or more can safely remove one voter.
proof fn can_remove_ge_3(n: u64)
    requires n >= 3
    ensures can_remove_safely_spec(n)
{
    assert(n as int - 1 >= n as int / 2 + 1) by(nonlinear_arith)
        requires n >= 3;
}

// r[verify quorum.can-remove-safely]
/// Clusters of 1 or 2 cannot safely remove a voter.
proof fn cannot_remove_small(n: u64)
    requires 0 < n <= 2
    ensures !can_remove_safely_spec(n)
{}

// --- min_cluster_size ---

// r[depends quorum.min-cluster-size]
/// Spec: smallest cluster tolerating f failures.
pub open spec fn min_cluster_size_spec(f: u64) -> u64 {
    (2 * f + 1) as u64
}

// r[impl quorum.min-cluster-size]
/// Returns the minimum cluster size to tolerate `f` failures.
///
/// Formula: 2*f + 1. Matches aspen's `min_cluster_size_for_tolerance`.
pub fn min_cluster_size(f: u64) -> (result: u64)
    requires f as int * 2 + 1 <= u64::MAX as int
    ensures result == min_cluster_size_spec(f)
{
    2 * f + 1
}

// r[verify quorum.min-cluster-size]
/// min_cluster_size and fault_tolerance are inverses:
/// min_cluster_size(fault_tolerance(n)) <= n.
proof fn min_size_fault_tolerance_inverse(n: u64)
    requires n > 0
    ensures min_cluster_size_spec(fault_tolerance_spec(n)) as int <= n as int
{
    assert(2 * ((n as int - 1) / 2) + 1 <= n as int) by(nonlinear_arith)
        requires n > 0;
}

// r[verify quorum.min-cluster-size]
/// The minimum cluster for f failures has exactly fault_tolerance f.
proof fn min_size_has_tolerance(f: u64)
    requires f as int * 2 + 1 <= u64::MAX as int
    ensures fault_tolerance_spec(min_cluster_size_spec(f)) == f
{
    assert(((2 * f as int + 1) - 1) / 2 == f as int) by(nonlinear_arith);
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify quorum.majority]
    #[test]
    fn test_majority() {
        assert_eq!(majority(1), 1);
        assert_eq!(majority(2), 2);
        assert_eq!(majority(3), 2);
        assert_eq!(majority(4), 3);
        assert_eq!(majority(5), 3);
        assert_eq!(majority(6), 4);
        assert_eq!(majority(7), 4);
        assert_eq!(majority(100), 51);
    }

    // r[verify quorum.is-quorum]
    #[test]
    fn test_is_quorum() {
        // 3-node cluster: majority is 2
        assert!(!is_quorum(0, 3));
        assert!(!is_quorum(1, 3));
        assert!(is_quorum(2, 3));
        assert!(is_quorum(3, 3));

        // 5-node cluster: majority is 3
        assert!(!is_quorum(2, 5));
        assert!(is_quorum(3, 5));
        assert!(is_quorum(4, 5));
        assert!(is_quorum(5, 5));

        // 1-node cluster: majority is 1
        assert!(is_quorum(1, 1));
    }

    // r[verify quorum.intersection]
    #[test]
    fn test_quorum_intersection() {
        // Two quorums of 5: majority is 3, so min quorum pair sums to 6 > 5
        let m5 = majority(5);
        assert!(is_quorum(m5, 5));
        assert!(u128::from(m5) + u128::from(m5) > 5);

        // Two quorums of 3: majority is 2, so min quorum pair sums to 4 > 3
        let m3 = majority(3);
        assert!(is_quorum(m3, 3));
        assert!(u128::from(m3) + u128::from(m3) > 3);
    }

    proptest! {
        // r[verify quorum.majority]
        #[test]
        fn prop_majority_bounds(n in 1u64..=10000) {
            let m = majority(n);
            prop_assert!(m >= 1);
            prop_assert!(m <= n);
        }

        // r[verify quorum.majority]
        #[test]
        fn prop_majority_gt_half(n in 1u64..=10000) {
            let m = u128::from(majority(n));
            prop_assert!(m * 2 > u128::from(n));
        }

        // r[verify quorum.is-quorum]
        #[test]
        fn prop_full_is_quorum(n in 1u64..=10000) {
            prop_assert!(is_quorum(n, n));
        }

        // r[verify quorum.intersection]
        #[test]
        fn prop_quorum_intersection(
            n in 1u64..=10000,
            a_extra in 0u64..=100,
            b_extra in 0u64..=100,
        ) {
            let m = majority(n);
            let a = m.saturating_add(a_extra).min(n);
            let b = m.saturating_add(b_extra).min(n);
            // Both a and b are quorums (>= majority, <= n)
            prop_assert!(is_quorum(a, n));
            prop_assert!(is_quorum(b, n));
            // Intersection: a + b > n
            prop_assert!(u128::from(a) + u128::from(b) > u128::from(n));
        }

        // r[verify quorum.is-quorum]
        #[test]
        fn prop_below_majority_not_quorum(n in 2u64..=10000) {
            let m = majority(n);
            prop_assert!(!is_quorum(m - 1, n));
        }

        // r[verify quorum.majority]
        // r[verify quorum.is-quorum]
        // r[verify quorum.intersection]
        /// Simulate two independent voter sets and verify overlap.
        /// Generates a cluster size, then two random subsets each forming
        /// a quorum. The sets must share at least one member.
        #[test]
        #[allow(clippy::cast_possible_truncation)]
        fn prop_quorum_overlap_simulation(
            n in 3u64..=25,
            voters_a in proptest::collection::vec(any::<bool>(), 25),
            voters_b in proptest::collection::vec(any::<bool>(), 25),
        ) {
            let n_usize = n as usize;
            // Build two voter sets from random booleans, capped at cluster size
            let mut set_a: Vec<bool> = vec![false; n_usize];
            let mut set_b: Vec<bool> = vec![false; n_usize];

            // Fill up to a quorum for set_a
            let m = majority(n) as usize;
            let mut count_a = 0usize;
            for (i, slot) in set_a.iter_mut().enumerate() {
                if count_a < m && voters_a.get(i).copied().unwrap_or(true) {
                    *slot = true;
                    count_a += 1;
                }
            }
            for slot in &mut set_a {
                if count_a >= m { break; }
                if !*slot { *slot = true; count_a += 1; }
            }

            let mut count_b = 0usize;
            for (i, slot) in set_b.iter_mut().enumerate() {
                if count_b < m && voters_b.get(i).copied().unwrap_or(true) {
                    *slot = true;
                    count_b += 1;
                }
            }
            for slot in &mut set_b {
                if count_b >= m { break; }
                if !*slot { *slot = true; count_b += 1; }
            }

            // Both are quorums
            prop_assert!(is_quorum(count_a as u64, n));
            prop_assert!(is_quorum(count_b as u64, n));

            // Must have at least one overlapping member
            let overlap = (0..n_usize).filter(|&i| set_a[i] && set_b[i]).count();
            prop_assert!(overlap >= 1);
        }
    }

    // --- fault_tolerance tests ---

    // r[verify quorum.fault-tolerance]
    #[test]
    fn test_fault_tolerance() {
        assert_eq!(fault_tolerance(0), 0);
        assert_eq!(fault_tolerance(1), 0);
        assert_eq!(fault_tolerance(2), 0);
        assert_eq!(fault_tolerance(3), 1);
        assert_eq!(fault_tolerance(4), 1);
        assert_eq!(fault_tolerance(5), 2);
        assert_eq!(fault_tolerance(7), 3);
    }

    // r[verify quorum.can-remove-safely]
    #[test]
    fn test_can_remove_safely() {
        // Matches aspen's test cases exactly
        assert!(!can_remove_safely(0));
        assert!(!can_remove_safely(1));
        assert!(!can_remove_safely(2));
        assert!(can_remove_safely(3));
        assert!(can_remove_safely(4));
        assert!(can_remove_safely(5));
    }

    // r[verify quorum.min-cluster-size]
    #[test]
    fn test_min_cluster_size() {
        // Matches aspen's test cases exactly
        assert_eq!(min_cluster_size(0), 1);
        assert_eq!(min_cluster_size(1), 3);
        assert_eq!(min_cluster_size(2), 5);
        assert_eq!(min_cluster_size(3), 7);
    }

    proptest! {
        // r[verify quorum.fault-tolerance]
        #[test]
        fn prop_fault_tolerance_leaves_quorum(n in 1u64..=10000) {
            let ft = fault_tolerance(n);
            let remaining = n - ft;
            prop_assert!(is_quorum(remaining, n));
        }

        // r[verify quorum.fault-tolerance]
        #[test]
        fn prop_fault_tolerance_bounded(n in 1u64..=10000) {
            prop_assert!(fault_tolerance(n) < n);
        }

        // r[verify quorum.can-remove-safely]
        #[test]
        fn prop_can_remove_matches_quorum(n in 1u64..=10000) {
            let safe = can_remove_safely(n);
            if n > 0 {
                prop_assert_eq!(safe, n - 1 >= majority(n));
            }
        }

        // r[verify quorum.min-cluster-size]
        #[test]
        fn prop_min_size_roundtrip(n in 1u64..=5000) {
            let ft = fault_tolerance(n);
            let min_size = min_cluster_size(ft);
            prop_assert!(min_size <= n);
        }

        // r[verify quorum.min-cluster-size]
        #[test]
        fn prop_min_size_has_tolerance(f in 0u64..=5000) {
            let n = min_cluster_size(f);
            prop_assert_eq!(fault_tolerance(n), f);
        }

        // r[verify quorum.fault-tolerance]
        // r[verify quorum.can-remove-safely]
        /// Fault tolerance matches can_remove_safely threshold.
        #[test]
        fn prop_tolerance_vs_remove(n in 1u64..=10000) {
            // can_remove_safely(n) ⟺ fault_tolerance(n) >= 1
            prop_assert_eq!(can_remove_safely(n), fault_tolerance(n) >= 1);
        }
    }
}
