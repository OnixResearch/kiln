//! Verified merge gating approval counting.
//!
//! Models the decision logic for whether a code merge is allowed based on
//! approval counts and CI status. Functions cover counting valid approvals,
//! checking merge conditions, calculating remaining approvals needed, and
//! detecting stale approvals. Every function has a machine-checked spec
//! and proven properties.

use vstd::prelude::*;

verus! {

// --- count_valid_approvals ---

// r[depends approval.count-valid]
/// Spec: count of valid approvals (both approved and matches head).
pub open spec fn count_valid_approvals_spec(approvals: Seq<(bool, bool)>, from: int) -> int
    decreases approvals.len() - from
{
    if from >= approvals.len() {
        0
    } else {
        let (approved, matches_head) = approvals[from];
        let current_count: int = if approved && matches_head { 1 } else { 0 };
        current_count + count_valid_approvals_spec(approvals, from + 1)
    }
}

// r[impl approval.count-valid]
/// Count entries where approved is true AND `matches_head_commit` is true.
pub fn count_valid_approvals(approvals: &[(bool, bool)]) -> (result: u64)
    ensures result as int == count_valid_approvals_spec(approvals@, 0)
{
    let mut i: usize = 0;
    let mut count: u64 = 0;

    while i < approvals.len()
        invariant
            0 <= i <= approvals@.len(),
            count as int == count_valid_approvals_spec(approvals@, 0) - count_valid_approvals_spec(approvals@, i as int),
            count <= i as u64
        decreases approvals@.len() - i
    {
        let (approved, matches_head) = approvals[i];
        if approved && matches_head {
            count = count + 1;
        }
        i = i + 1;
    }

    count
}

// r[verify approval.count-valid]
/// Count is bounded by total approvals length.
proof fn count_valid_le_len(approvals: Seq<(bool, bool)>)
    ensures count_valid_approvals_spec(approvals, 0) <= approvals.len()
{
    count_valid_le_len_helper(approvals, 0);
}

/// Helper proof for count_valid_le_len.
proof fn count_valid_le_len_helper(approvals: Seq<(bool, bool)>, from: int)
    requires 0 <= from <= approvals.len()
    ensures count_valid_approvals_spec(approvals, from) <= approvals.len() - from
    decreases approvals.len() - from
{
    if from < approvals.len() {
        count_valid_le_len_helper(approvals, from + 1);
    }
}

// r[verify approval.count-valid]
/// Empty approvals sequence has count 0.
proof fn count_valid_empty()
    ensures count_valid_approvals_spec(Seq::<(bool, bool)>::empty(), 0) == 0
{}

// --- is_merge_allowed ---

// r[depends approval.is-merge-allowed]
/// Spec: merge is allowed when CI passed AND valid count >= required.
pub open spec fn is_merge_allowed_spec(valid_count: u64, required: u64, ci_passed: bool) -> bool {
    ci_passed && valid_count >= required
}

// r[impl approval.is-merge-allowed]
/// Returns true if CI passed and valid approvals >= required count.
pub fn is_merge_allowed(valid_count: u64, required: u64, ci_passed: bool) -> (result: bool)
    ensures result == is_merge_allowed_spec(valid_count, required, ci_passed)
{
    ci_passed && valid_count >= required
}

// r[verify approval.is-merge-allowed]
/// If CI failed, merge is not allowed regardless of approvals.
proof fn ci_failed_no_merge(valid_count: u64, required: u64)
    ensures !is_merge_allowed_spec(valid_count, required, false)
{}

// r[verify approval.is-merge-allowed]
/// Merge requires BOTH CI passed AND enough approvals.
proof fn merge_requires_both(valid_count: u64, required: u64, ci_passed: bool)
    ensures is_merge_allowed_spec(valid_count, required, ci_passed) ==> (ci_passed && valid_count >= required)
{}

// --- needs_more_approvals ---

// r[depends approval.needs-more]
/// Spec: remaining approvals needed.
pub open spec fn needs_more_approvals_spec(valid_count: u64, required: u64) -> u64 {
    if valid_count >= required { 0 } else { (required - valid_count) as u64 }
}

// r[impl approval.needs-more]
/// Returns number of additional approvals needed, or 0 if sufficient.
pub fn needs_more_approvals(valid_count: u64, required: u64) -> (result: u64)
    ensures result == needs_more_approvals_spec(valid_count, required)
{
    if valid_count >= required {
        0
    } else {
        required - valid_count
    }
}

// r[verify approval.needs-more]
/// Returns 0 if and only if valid count >= required.
proof fn needs_more_zero_iff_sufficient(valid_count: u64, required: u64)
    ensures needs_more_approvals_spec(valid_count, required) == 0 <==> valid_count >= required
{}

// --- has_stale_approvals ---

// r[depends approval.has-stale]
/// Spec: check if any approvals are stale (approved but don't match head).
pub open spec fn has_stale_approvals_spec(approvals: Seq<(bool, bool)>, from: int) -> bool
    decreases approvals.len() - from
{
    if from >= approvals.len() {
        false
    } else {
        let (approved, matches_head) = approvals[from];
        (approved && !matches_head) || has_stale_approvals_spec(approvals, from + 1)
    }
}

// r[impl approval.has-stale]
/// Returns true if any entry has approved=true but `matches_head=false`.
pub fn has_stale_approvals(approvals: &[(bool, bool)]) -> (result: bool)
    ensures result == has_stale_approvals_spec(approvals@, 0)
{
    has_stale_from(approvals, 0)
}

/// Helper function that implements the recursive spec.
fn has_stale_from(approvals: &[(bool, bool)], from: usize) -> (result: bool)
    requires from <= approvals@.len()
    ensures result == has_stale_approvals_spec(approvals@, from as int)
    decreases approvals@.len() - from
{
    if from >= approvals.len() {
        false
    } else {
        let (approved, matches_head) = approvals[from];
        if approved && !matches_head {
            true
        } else {
            has_stale_from(approvals, from + 1)
        }
    }
}

// --- Combined properties ---

// r[verify approval.count-valid]
// r[verify approval.is-merge-allowed]
/// Merge allowed with required=0 and CI passed is always true.
proof fn required_zero_ci_passed_always_allowed(valid_count: u64)
    ensures is_merge_allowed_spec(valid_count, 0, true)
{}

// r[verify approval.count-valid]
// r[verify approval.has-stale]
/// Stale approvals property: if stale approvals exist, there's at least one.
proof fn stale_approvals_exist_property(approvals: Seq<(bool, bool)>)
    requires has_stale_approvals_spec(approvals, 0)
    ensures exists|i: int| #![trigger approvals[i]] 0 <= i < approvals.len() && {
        let (approved, matches_head) = approvals[i];
        approved && !matches_head
    }
{
    stale_exists_helper(approvals, 0);
}

/// Helper to show stale approvals exist.
proof fn stale_exists_helper(approvals: Seq<(bool, bool)>, from: int)
    requires
        0 <= from <= approvals.len(),
        has_stale_approvals_spec(approvals, from)
    ensures exists|i: int| #![trigger approvals[i]] from <= i < approvals.len() && {
        let (approved, matches_head) = approvals[i];
        approved && !matches_head
    }
    decreases approvals.len() - from
{
    if from < approvals.len() {
        let (approved, matches_head) = approvals[from];
        if approved && !matches_head {
            // Found the stale approval at index from
        } else {
            stale_exists_helper(approvals, from + 1);
        }
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify approval.count-valid]
    #[test]
    fn test_count_valid_approvals() {
        // Empty
        assert_eq!(count_valid_approvals(&[]), 0);

        // All valid
        assert_eq!(count_valid_approvals(&[(true, true), (true, true)]), 2);

        // Mixed
        assert_eq!(
            count_valid_approvals(&[
                (true, true),   // valid
                (true, false),  // stale
                (false, true),  // not approved
                (false, false), // not approved
                (true, true),   // valid
            ]),
            2
        );
    }

    // r[verify approval.is-merge-allowed]
    #[test]
    fn test_is_merge_allowed() {
        // CI failed
        assert!(!is_merge_allowed(5, 3, false));

        // CI passed, enough approvals
        assert!(is_merge_allowed(5, 3, true));
        assert!(is_merge_allowed(3, 3, true));

        // CI passed, not enough approvals
        assert!(!is_merge_allowed(2, 3, true));

        // Required 0 with CI passed
        assert!(is_merge_allowed(0, 0, true));
        assert!(is_merge_allowed(10, 0, true));
    }

    // r[verify approval.needs-more]
    #[test]
    fn test_needs_more_approvals() {
        assert_eq!(needs_more_approvals(5, 3), 0);
        assert_eq!(needs_more_approvals(3, 3), 0);
        assert_eq!(needs_more_approvals(2, 5), 3);
        assert_eq!(needs_more_approvals(0, 3), 3);
    }

    // r[verify approval.has-stale]
    #[test]
    fn test_has_stale_approvals() {
        // No stale
        assert!(!has_stale_approvals(&[]));
        assert!(!has_stale_approvals(&[(true, true), (false, false)]));

        // Has stale
        assert!(has_stale_approvals(&[(true, false)]));
        assert!(has_stale_approvals(&[
            (true, true),  // valid
            (true, false), // stale
        ]));
    }

    proptest! {
        // r[verify approval.count-valid]
        #[test]
        fn prop_count_valid_bounded(approvals: Vec<(bool, bool)>) {
            let count = count_valid_approvals(&approvals);
            prop_assert!(count <= approvals.len() as u64);
        }

        // r[verify approval.is-merge-allowed]
        #[test]
        fn prop_ci_failed_no_merge(valid_count: u64, required: u64) {
            prop_assert!(!is_merge_allowed(valid_count, required, false));
        }

        // r[verify approval.is-merge-allowed]
        #[test]
        fn prop_required_zero_ci_passed(valid_count: u64) {
            prop_assert!(is_merge_allowed(valid_count, 0, true));
        }

        // r[verify approval.needs-more]
        #[test]
        fn prop_needs_more_zero_iff_sufficient(valid_count: u64, required: u64) {
            let needs = needs_more_approvals(valid_count, required);
            prop_assert_eq!(needs == 0, valid_count >= required);
        }

        // r[verify approval.count-valid]
        // r[verify approval.has-stale]
        #[test]
        fn prop_no_stale_valid_eq_approved(approvals: Vec<(bool, bool)>) {
            let has_stale = has_stale_approvals(&approvals);
            if !has_stale {
                let valid_count = count_valid_approvals(&approvals);
                let approved_count = approvals.iter().filter(|(approved, _)| *approved).count();
                prop_assert_eq!(valid_count, approved_count as u64);
            }
        }

        // r[verify approval.count-valid]
        // r[verify approval.is-merge-allowed]
        // r[verify approval.needs-more]
        // r[verify approval.has-stale]
        /// Model oracle: generate random approvals, check all functions agree.
        #[test]
        fn prop_model_oracle(
            approvals in proptest::collection::vec((any::<bool>(), any::<bool>()), 0..=20),
            required in 0u64..10,
            ci_passed: bool,
        ) {
            let valid_count = count_valid_approvals(&approvals);
            let merge_allowed = is_merge_allowed(valid_count, required, ci_passed);
            let needs_more = needs_more_approvals(valid_count, required);
            let has_stale = has_stale_approvals(&approvals);

            // Oracle calculations
            let oracle_valid = approvals.iter()
                .filter(|(approved, matches_head)| *approved && *matches_head)
                .count() as u64;
            let oracle_merge = ci_passed && oracle_valid >= required;
            let oracle_needs = if oracle_valid >= required { 0 } else { required - oracle_valid };
            let oracle_stale = approvals.iter()
                .any(|(approved, matches_head)| *approved && !*matches_head);

            prop_assert_eq!(valid_count, oracle_valid);
            prop_assert_eq!(merge_allowed, oracle_merge);
            prop_assert_eq!(needs_more, oracle_needs);
            prop_assert_eq!(has_stale, oracle_stale);
        }
    }
}
