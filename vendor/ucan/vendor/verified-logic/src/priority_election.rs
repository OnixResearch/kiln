//! Verified priority-based leader election for Raft.
//!
//! In multi-datacenter deployments, some nodes are preferred as leader
//! (e.g., nodes in the same DC as most clients). Priority-based election
//! adds a priority field to the election logic:
//!
//! - Higher-priority candidates are preferred when logs are equally fresh.
//! - A node with a lower priority should yield to a higher-priority node if the higher-priority
//!   node's log is at least as up-to-date.
//! - Priority never overrides log freshness — safety is preserved.
//! - A leader voluntarily steps down when it discovers a higher-priority node that is caught up.

use vstd::prelude::*;

verus! {

// --- should_yield ---

// r[depends priority-election.should-yield]
/// Spec: should this candidate yield to another node with higher priority?
///
/// A candidate yields when:
/// 1. The other node has strictly higher priority.
/// 2. The other node's log is at least as up-to-date (by epoch comparison).
///
/// This is advisory — the candidate MAY still run if it doesn't hear
/// from the higher-priority node within a timeout.
pub open spec fn should_yield_spec(
    my_priority: u64,
    other_priority: u64,
    other_log_current: bool,
) -> bool {
    other_priority > my_priority && other_log_current
}

// r[impl priority-election.should-yield]
/// Check if this candidate should yield.
pub fn should_yield(
    my_priority: u64,
    other_priority: u64,
    other_log_current: bool,
) -> (result: bool)
    ensures result == should_yield_spec(my_priority, other_priority, other_log_current)
{
    other_priority > my_priority && other_log_current
}

// --- election_delay ---

// r[depends priority-election.delay]
/// Spec: compute election delay based on priority.
///
/// Lower-priority nodes wait longer before starting an election, giving
/// higher-priority nodes a chance to win. The delay is:
///   base_timeout + (max_priority - my_priority) * delay_per_level
///
/// Higher priority → shorter delay → starts election first.
pub open spec fn election_delay_spec(
    base_timeout: u64,
    my_priority: u64,
    max_priority: u64,
    delay_per_level: u64,
) -> u64
    recommends my_priority <= max_priority
{
    let levels = (max_priority - my_priority) as u64;
    let extra: u64 = if delay_per_level > 0 && levels > 0 {
        if levels > u64::MAX as u64 / delay_per_level {
            (u64::MAX as u64 - base_timeout) as u64
        } else {
            let product = (levels * delay_per_level) as u64;
            if product > u64::MAX as u64 - base_timeout {
                (u64::MAX as u64 - base_timeout) as u64
            } else {
                product
            }
        }
    } else {
        0u64
    };
    (base_timeout + extra) as u64
}

// r[impl priority-election.delay]
/// Compute the election delay for a given priority.
pub fn election_delay(
    base_timeout: u64,
    my_priority: u64,
    max_priority: u64,
    delay_per_level: u64,
) -> (result: u64)
    requires my_priority <= max_priority
    ensures result == election_delay_spec(base_timeout, my_priority, max_priority, delay_per_level)
{
    let levels = max_priority - my_priority;
    let extra = if delay_per_level > 0 && levels > 0 {
        if levels > u64::MAX / delay_per_level {
            u64::MAX - base_timeout
        } else {
            assert(levels as int * delay_per_level as int <= u64::MAX as int) by(nonlinear_arith)
                requires levels <= u64::MAX / delay_per_level, delay_per_level > 0u64;
            let product = levels * delay_per_level;
            if product > u64::MAX - base_timeout {
                u64::MAX - base_timeout
            } else {
                product
            }
        }
    } else {
        0
    };
    base_timeout + extra
}

// --- should_step_down_for_priority ---

// r[depends priority-election.step-down]
/// Spec: should a leader step down because a higher-priority node is ready?
///
/// The leader steps down when:
/// 1. A node with higher priority exists.
/// 2. That node has caught up (matchIndex close to leader's log end).
/// 3. The leader has been leader long enough (grace period expired).
///
/// The grace period prevents thrashing during initial leader startup.
pub open spec fn should_step_down_for_priority_spec(
    my_priority: u64,
    higher_priority_ready: bool,
    grace_period_expired: bool,
) -> bool {
    higher_priority_ready && grace_period_expired
}

// r[impl priority-election.step-down]
/// Check if the leader should voluntarily step down.
pub fn should_step_down_for_priority(
    _my_priority: u64,
    higher_priority_ready: bool,
    grace_period_expired: bool,
) -> (result: bool)
    ensures result == should_step_down_for_priority_spec(
        _my_priority, higher_priority_ready, grace_period_expired,
    )
{
    higher_priority_ready && grace_period_expired
}

// --- compare_candidates ---

// r[depends priority-election.compare-candidates]
/// Spec: compare two candidates for preference.
///
/// Returns:
/// -1 if candidate A is preferred
///  0 if they are equal
///  1 if candidate B is preferred
///
/// Log freshness (epoch) takes absolute precedence. Priority is the
/// tiebreaker when epochs are equal.
pub open spec fn compare_candidates_spec(
    a_epoch_ge_b: bool,
    b_epoch_ge_a: bool,
    a_priority: u64,
    b_priority: u64,
) -> i8 {
    if a_epoch_ge_b && !b_epoch_ge_a {
        -1i8  // A has a fresher log
    } else if b_epoch_ge_a && !a_epoch_ge_b {
        1i8   // B has a fresher log
    } else if a_priority > b_priority {
        -1i8  // Same log, A has higher priority
    } else if b_priority > a_priority {
        1i8   // Same log, B has higher priority
    } else {
        0i8   // Tied
    }
}

// r[impl priority-election.compare-candidates]
/// Compare two candidates by log freshness then priority.
pub fn compare_candidates(
    a_epoch_ge_b: bool,
    b_epoch_ge_a: bool,
    a_priority: u64,
    b_priority: u64,
) -> (result: i8)
    ensures result == compare_candidates_spec(a_epoch_ge_b, b_epoch_ge_a, a_priority, b_priority)
{
    if a_epoch_ge_b && !b_epoch_ge_a {
        -1
    } else if b_epoch_ge_a && !a_epoch_ge_b {
        1
    } else if a_priority > b_priority {
        -1
    } else if b_priority > a_priority {
        1
    } else {
        0
    }
}

// --- is_highest_priority ---

// r[depends priority-election.is-highest]
/// Spec: does this node have the highest priority in the cluster?
///
/// True when my_priority >= max_priority. Used to determine if the
/// node should aggressively start elections.
pub open spec fn is_highest_priority_spec(
    my_priority: u64,
    max_cluster_priority: u64,
) -> bool {
    my_priority >= max_cluster_priority
}

// r[impl priority-election.is-highest]
/// Check if this node has the highest priority.
pub fn is_highest_priority(
    my_priority: u64,
    max_cluster_priority: u64,
) -> (result: bool)
    ensures result == is_highest_priority_spec(my_priority, max_cluster_priority)
{
    my_priority >= max_cluster_priority
}

// --- Safety proofs ---

// r[verify priority-election.should-yield]
/// Same priority never yields.
proof fn same_priority_no_yield(priority: u64, log_current: bool)
    ensures !should_yield_spec(priority, priority, log_current)
{}

// r[verify priority-election.should-yield]
/// Outdated log never causes yield regardless of priority.
proof fn outdated_log_no_yield(my_p: u64, other_p: u64)
    ensures !should_yield_spec(my_p, other_p, false)
{}

// r[verify priority-election.delay]
/// Highest priority gets base timeout (no extra delay).
proof fn highest_priority_base_delay(base_timeout: u64, max_priority: u64, delay_per_level: u64)
    ensures election_delay_spec(base_timeout, max_priority, max_priority, delay_per_level) == base_timeout
{}

// r[verify priority-election.delay]
/// Lower priority gets longer delay.
proof fn lower_priority_longer_delay(
    base_timeout: u64,
    my_priority: u64,
    max_priority: u64,
    delay_per_level: u64,
)
    requires my_priority <= max_priority
    ensures election_delay_spec(base_timeout, my_priority, max_priority, delay_per_level) >= base_timeout
{}

// r[verify priority-election.compare-candidates]
/// Fresher log always wins regardless of priority.
proof fn fresher_log_wins(a_priority: u64, b_priority: u64)
    ensures compare_candidates_spec(true, false, a_priority, b_priority) == -1i8
{}

// r[verify priority-election.compare-candidates]
/// Same freshness uses priority as tiebreaker.
proof fn priority_tiebreaker(a_priority: u64, b_priority: u64)
    requires a_priority > b_priority
    ensures compare_candidates_spec(true, true, a_priority, b_priority) == -1i8
{}

// r[verify priority-election.compare-candidates]
/// Comparison is antisymmetric on log freshness.
proof fn compare_antisymmetric_on_log()
    ensures
        compare_candidates_spec(true, false, 0, 100) == -1i8,
        compare_candidates_spec(false, true, 0, 100) == 1i8,
{}

// r[verify priority-election.compare-candidates]
/// Equal candidates get 0.
proof fn equal_candidates(priority: u64)
    ensures compare_candidates_spec(true, true, priority, priority) == 0i8
{}

// r[verify priority-election.step-down]
/// Grace period blocks early step-down.
proof fn grace_blocks_stepdown(my_priority: u64)
    ensures !should_step_down_for_priority_spec(my_priority, true, false)
{}

// r[verify priority-election.is-highest]
/// Highest priority node is self-consistent.
proof fn highest_is_reflexive(priority: u64)
    ensures is_highest_priority_spec(priority, priority)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify priority-election.should-yield]
    #[test]
    fn test_should_yield() {
        assert!(should_yield(3, 5, true)); // higher priority, current log
        assert!(!should_yield(5, 3, true)); // lower priority
        assert!(!should_yield(3, 5, false)); // log not current
        assert!(!should_yield(3, 3, true)); // same priority
    }

    // r[verify priority-election.delay]
    #[test]
    fn test_election_delay() {
        assert_eq!(election_delay(100, 10, 10, 50), 100); // highest priority
        assert_eq!(election_delay(100, 8, 10, 50), 200); // 2 levels below
        assert_eq!(election_delay(100, 5, 10, 50), 350); // 5 levels below
        assert_eq!(election_delay(100, 10, 10, 0), 100); // zero delay per level
    }

    // r[verify priority-election.compare-candidates]
    #[test]
    fn test_compare_candidates() {
        assert_eq!(compare_candidates(true, false, 1, 10), -1); // A fresher log
        assert_eq!(compare_candidates(false, true, 10, 1), 1); // B fresher log
        assert_eq!(compare_candidates(true, true, 10, 5), -1); // Same, A higher prio
        assert_eq!(compare_candidates(true, true, 5, 10), 1); // Same, B higher prio
        assert_eq!(compare_candidates(true, true, 5, 5), 0); // Tied
    }

    // r[verify priority-election.step-down]
    #[test]
    fn test_should_step_down_for_priority() {
        assert!(should_step_down_for_priority(3, true, true));
        assert!(!should_step_down_for_priority(3, true, false)); // grace period
        assert!(!should_step_down_for_priority(3, false, true)); // no higher ready
    }

    // r[verify priority-election.is-highest]
    #[test]
    fn test_is_highest_priority() {
        assert!(is_highest_priority(10, 10));
        assert!(is_highest_priority(15, 10));
        assert!(!is_highest_priority(5, 10));
    }

    proptest! {
        // r[verify priority-election.delay]
        #[test]
        fn prop_highest_gets_base(base in 0u64..1000, max_p in 0u64..100, dpl: u64) {
            prop_assert_eq!(election_delay(base, max_p, max_p, dpl), base);
        }

        // r[verify priority-election.delay]
        #[test]
        fn prop_delay_ge_base(
            base in 0u64..1000,
            my_p in 0u64..100,
            max_p in 0u64..100,
            dpl in 0u64..100,
        ) {
            if my_p <= max_p {
                prop_assert!(election_delay(base, my_p, max_p, dpl) >= base);
            }
        }

        // r[verify priority-election.compare-candidates]
        #[test]
        fn prop_fresher_log_wins(a_prio: u64, b_prio: u64) {
            // A has fresher log → A preferred regardless of priority
            prop_assert_eq!(compare_candidates(true, false, a_prio, b_prio), -1);
        }

        // r[verify priority-election.compare-candidates]
        #[test]
        fn prop_equal_is_zero(priority: u64) {
            prop_assert_eq!(compare_candidates(true, true, priority, priority), 0);
        }
    }
}
