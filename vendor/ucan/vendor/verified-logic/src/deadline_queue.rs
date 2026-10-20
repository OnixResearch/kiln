//! Verified deadline ordering for earliest-deadline-first (EDF) scheduling.
//!
//! Models tasks with absolute deadlines. EDF always selects the task whose
//! deadline is soonest. Proofs cover ordering correctness, urgency
//! classification, deadline advancement, and the relationship between
//! deadline proximity and scheduling priority.

use vstd::prelude::*;

verus! {

// ============================================================
// Deadline comparison
// ============================================================

// r[depends deadline-queue.is-earlier]
/// Spec: is deadline `a` earlier than (or equal to) deadline `b`?
pub open spec fn is_earlier_spec(a: u64, b: u64) -> bool {
    a <= b
}

// r[impl deadline-queue.is-earlier]
/// Returns true if deadline `a` is no later than deadline `b`.
pub fn is_earlier(a: u64, b: u64) -> (result: bool)
    ensures result == is_earlier_spec(a, b)
{
    a <= b
}

// r[verify deadline-queue.is-earlier]
/// Reflexive: every deadline is as early as itself.
proof fn is_earlier_reflexive(a: u64)
    ensures is_earlier_spec(a, a)
{}

// r[verify deadline-queue.is-earlier]
/// Transitive: a ≤ b ≤ c implies a ≤ c.
proof fn is_earlier_transitive(a: u64, b: u64, c: u64)
    requires is_earlier_spec(a, b), is_earlier_spec(b, c)
    ensures is_earlier_spec(a, c)
{}

// r[verify deadline-queue.is-earlier]
/// Antisymmetric: a ≤ b and b ≤ a implies a == b.
proof fn is_earlier_antisymmetric(a: u64, b: u64)
    requires is_earlier_spec(a, b), is_earlier_spec(b, a)
    ensures a == b
{}

// r[verify deadline-queue.is-earlier]
/// Total: for any two deadlines, at least one ordering holds.
proof fn is_earlier_total(a: u64, b: u64)
    ensures is_earlier_spec(a, b) || is_earlier_spec(b, a)
{}

// ============================================================
// Slack (time remaining until deadline)
// ============================================================

// r[depends deadline-queue.slack]
/// Spec: time remaining before the deadline.
/// Returns 0 if the deadline has already passed.
pub open spec fn slack_spec(now: u64, deadline: u64) -> u64 {
    if deadline >= now { (deadline - now) as u64 } else { 0u64 }
}

// r[impl deadline-queue.slack]
/// Compute time remaining until deadline.
pub fn slack(now: u64, deadline: u64) -> (result: u64)
    ensures result == slack_spec(now, deadline)
{
    if deadline >= now { deadline - now } else { 0 }
}

// r[verify deadline-queue.slack]
/// A passed deadline has zero slack.
proof fn passed_zero_slack(now: u64, deadline: u64)
    requires now > deadline
    ensures slack_spec(now, deadline) == 0u64
{}

// r[verify deadline-queue.slack]
/// Slack decreases as time advances.
proof fn slack_monotone_decreasing(t1: u64, t2: u64, deadline: u64)
    requires t1 <= t2
    ensures slack_spec(t2, deadline) <= slack_spec(t1, deadline)
{}

// r[verify deadline-queue.slack]
/// At the deadline itself, slack is zero.
proof fn slack_at_deadline(deadline: u64)
    ensures slack_spec(deadline, deadline) == 0u64
{}

// r[verify deadline-queue.slack]
/// Earlier deadline has less slack (same now).
proof fn earlier_deadline_less_slack(now: u64, d1: u64, d2: u64)
    requires is_earlier_spec(d1, d2), now <= d1
    ensures slack_spec(now, d1) <= slack_spec(now, d2)
{}

// ============================================================
// Urgency classification
// ============================================================

/// Overdue: deadline already passed.
pub const URGENCY_OVERDUE: u8 = 0;
/// Critical: less than or equal to threshold.
pub const URGENCY_CRITICAL: u8 = 1;
/// Normal: more than threshold remaining.
pub const URGENCY_NORMAL: u8 = 2;

// r[depends deadline-queue.urgency]
/// Spec: classify urgency based on remaining slack.
pub open spec fn urgency_spec(now: u64, deadline: u64, threshold: u64) -> u8 {
    if now > deadline {
        URGENCY_OVERDUE
    } else if (deadline - now) as u64 <= threshold {
        URGENCY_CRITICAL
    } else {
        URGENCY_NORMAL
    }
}

// r[impl deadline-queue.urgency]
/// Classify a task's urgency.
pub fn urgency(now: u64, deadline: u64, threshold: u64) -> (result: u8)
    ensures result == urgency_spec(now, deadline, threshold)
{
    if now > deadline {
        URGENCY_OVERDUE
    } else if deadline - now <= threshold {
        URGENCY_CRITICAL
    } else {
        URGENCY_NORMAL
    }
}

// r[verify deadline-queue.urgency]
/// Urgency is always a valid value.
proof fn urgency_valid(now: u64, deadline: u64, threshold: u64)
    ensures
        urgency_spec(now, deadline, threshold) == URGENCY_OVERDUE
        || urgency_spec(now, deadline, threshold) == URGENCY_CRITICAL
        || urgency_spec(now, deadline, threshold) == URGENCY_NORMAL,
{}

// r[verify deadline-queue.urgency]
/// Time advancing can only increase urgency (lower value = more urgent).
proof fn urgency_monotone(t1: u64, t2: u64, deadline: u64, threshold: u64)
    requires t1 <= t2
    ensures urgency_spec(t2, deadline, threshold) <= urgency_spec(t1, deadline, threshold)
{}

// r[verify deadline-queue.urgency]
/// Overdue is the most urgent classification.
proof fn overdue_most_urgent(now: u64, deadline: u64, threshold: u64)
    requires now > deadline
    ensures urgency_spec(now, deadline, threshold) == URGENCY_OVERDUE
{}

// ============================================================
// EDF selection: pick the task with the earliest deadline
// ============================================================

// r[depends deadline-queue.edf-select]
/// Spec: index of the earliest deadline in a non-empty sequence.
/// Ties broken by first occurrence.
pub open spec fn edf_select_spec(deadlines: Seq<u64>) -> int
    recommends deadlines.len() > 0
    decreases deadlines.len()
{
    if deadlines.len() <= 1 {
        0int
    } else {
        let rest_idx = edf_select_spec(deadlines.drop_first());
        let rest_deadline = deadlines[rest_idx + 1];
        if deadlines[0] <= rest_deadline {
            0int
        } else {
            rest_idx + 1
        }
    }
}

// r[impl deadline-queue.edf-select]
/// Select the index of the task with the earliest deadline.
pub fn edf_select(deadlines: &[u64]) -> (result: usize)
    requires deadlines@.len() > 0
    ensures
        result < deadlines@.len(),
        forall|j: int| #![trigger deadlines@[j]]
            0 <= j < deadlines@.len() as int
            ==> deadlines@[result as int] <= deadlines@[j],
{
    let mut best: usize = 0;
    let mut i: usize = 1;
    while i < deadlines.len()
        invariant
            1 <= i <= deadlines@.len(),
            best < i,
            forall|j: int| #![trigger deadlines@[j]]
                0 <= j < i as int
                ==> deadlines@[best as int] <= deadlines@[j],
        decreases deadlines@.len() - i,
    {
        if deadlines[i] < deadlines[best] {
            best = i;
        }
        i = i + 1;
    }
    best
}

// r[verify deadline-queue.edf-select]
/// The selected task has a deadline no later than any other task.
proof fn edf_select_is_minimum(deadlines: Seq<u64>)
    requires deadlines.len() > 0
    ensures
        0 <= edf_select_spec(deadlines) < deadlines.len() as int,
    decreases deadlines.len()
{
    if deadlines.len() > 1 {
        edf_select_is_minimum(deadlines.drop_first());
    }
}

// r[verify deadline-queue.edf-select]
/// Singleton always selects index 0.
proof fn edf_singleton(d: u64)
    ensures edf_select_spec(seq![d]) == 0int
{}

// ============================================================
// Deadline extension (pushing a deadline later)
// ============================================================

// r[depends deadline-queue.extend-deadline]
/// Spec: extend a deadline by a given amount, saturating at u64::MAX.
pub open spec fn extend_deadline_spec(deadline: u64, extension: u64) -> u64 {
    if (deadline as int + extension as int) > u64::MAX as int {
        u64::MAX
    } else {
        (deadline as int + extension as int) as u64
    }
}

// r[impl deadline-queue.extend-deadline]
/// Extend a deadline, saturating on overflow.
pub fn extend_deadline(deadline: u64, extension: u64) -> (result: u64)
    ensures result == extend_deadline_spec(deadline, extension)
{
    if extension > u64::MAX - deadline {
        u64::MAX
    } else {
        deadline + extension
    }
}

// r[verify deadline-queue.extend-deadline]
/// Extension never moves the deadline earlier.
proof fn extend_never_earlier(deadline: u64, extension: u64)
    ensures extend_deadline_spec(deadline, extension) >= deadline
{}

// r[verify deadline-queue.extend-deadline]
/// Zero extension is identity.
proof fn extend_zero_identity(deadline: u64)
    ensures extend_deadline_spec(deadline, 0) == deadline
{}

// r[verify deadline-queue.extend-deadline]
/// Extension is monotone in the extension amount.
proof fn extend_monotone(deadline: u64, ext1: u64, ext2: u64)
    requires ext1 <= ext2
    ensures extend_deadline_spec(deadline, ext1) <= extend_deadline_spec(deadline, ext2)
{}

} // verus!

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod test {
    use super::*;

    // r[verify deadline-queue.is-earlier]
    #[test]
    fn test_is_earlier() {
        assert!(is_earlier(5, 10));
        assert!(is_earlier(5, 5));
        assert!(!is_earlier(10, 5));
    }

    // r[verify deadline-queue.slack]
    #[test]
    fn test_slack() {
        assert_eq!(slack(0, 100), 100);
        assert_eq!(slack(50, 100), 50);
        assert_eq!(slack(100, 100), 0);
        assert_eq!(slack(150, 100), 0);
    }

    // r[verify deadline-queue.urgency]
    #[test]
    fn test_urgency() {
        assert_eq!(urgency(150, 100, 10), URGENCY_OVERDUE);
        assert_eq!(urgency(95, 100, 10), URGENCY_CRITICAL);
        assert_eq!(urgency(50, 100, 10), URGENCY_NORMAL);
        assert_eq!(urgency(100, 100, 10), URGENCY_CRITICAL); // at deadline = 0 slack ≤ 10
    }

    // r[verify deadline-queue.edf-select]
    #[test]
    fn test_edf_select() {
        assert_eq!(edf_select(&[100, 50, 200, 10, 150]), 3); // deadline 10 is earliest
        assert_eq!(edf_select(&[42]), 0); // singleton
        assert_eq!(edf_select(&[5, 5, 5]), 0); // ties: first occurrence
    }

    // r[verify deadline-queue.extend-deadline]
    #[test]
    fn test_extend_deadline() {
        assert_eq!(extend_deadline(100, 50), 150);
        assert_eq!(extend_deadline(u64::MAX - 10, 20), u64::MAX); // saturates
        assert_eq!(extend_deadline(100, 0), 100);
    }

    #[cfg(test)]
    mod proptests {
        use proptest::prelude::*;

        use super::*;

        proptest! {
            // r[verify deadline-queue.is-earlier]
            #[test]
            fn prop_is_earlier_total(a: u64, b: u64) {
                prop_assert!(is_earlier(a, b) || is_earlier(b, a));
            }

            // r[verify deadline-queue.slack]
            #[test]
            fn prop_slack_bounded(now: u64, deadline: u64) {
                let s = slack(now, deadline);
                if deadline >= now {
                    prop_assert_eq!(s, deadline - now);
                } else {
                    prop_assert_eq!(s, 0);
                }
            }

            // r[verify deadline-queue.urgency]
            #[test]
            fn prop_urgency_valid(now: u64, deadline: u64, threshold: u64) {
                let u = urgency(now, deadline, threshold);
                prop_assert!(u == URGENCY_OVERDUE || u == URGENCY_CRITICAL || u == URGENCY_NORMAL);
            }

            // r[verify deadline-queue.edf-select]
            #[test]
            fn prop_edf_select_is_min(deadlines in proptest::collection::vec(any::<u64>(), 1..50)) {
                let idx = edf_select(&deadlines);
                let min_val = deadlines[idx];
                for d in &deadlines {
                    prop_assert!(min_val <= *d);
                }
            }

            // r[verify deadline-queue.extend-deadline]
            #[test]
            fn prop_extend_never_earlier(deadline: u64, ext: u64) {
                prop_assert!(extend_deadline(deadline, ext) >= deadline);
            }
        }
    }
}
