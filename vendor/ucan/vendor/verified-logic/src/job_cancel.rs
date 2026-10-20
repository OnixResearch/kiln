//! Verified job cancellation propagation.
//!
//! When a job is cancelled, dependents with no surviving path to
//! completion should also be cancelled. This module provides the
//! decision logic: given job states and dependency lists, determine
//! which jobs should be cancelled via iterative depth-bounded
//! propagation.
//!
//! A job is cancelled if ALL its dependencies are either cancelled,
//! failed, or already marked for cancellation (no completed dependency
//! to keep it alive). Already-terminal jobs (Completed, Failed,
//! Cancelled) are never re-cancelled.

use vstd::prelude::*;

verus! {

/// Job state constants (mirrors job_state module).
pub const COMPLETED: u8 = 3u8;
pub const FAILED: u8 = 4u8;
pub const CANCELLED: u8 = 5u8;

// ============================================================
// is_terminal (local copy to avoid cross-module import in verus)
// ============================================================

/// Spec: job is in a terminal state.
pub open spec fn is_terminal_spec(state: u8) -> bool {
    state == COMPLETED || state == FAILED || state == CANCELLED
}

// ============================================================
// has_surviving_dep: does a job have at least one completed dep?
// ============================================================

// r[depends job-cancel.has-surviving-dep]
/// Spec: at least one dependency is Completed (surviving path).
pub open spec fn has_surviving_dep_from_spec(
    dep_indices: Seq<usize>,
    job_states: Seq<u8>,
    cancel_marks: Seq<bool>,
    from: int,
) -> bool
    decreases dep_indices.len() - from
{
    if from < 0 || from >= dep_indices.len() as int {
        false
    } else {
        let idx = dep_indices[from] as int;
        let surviving = idx < job_states.len() as int
            && job_states[idx] == COMPLETED
            && !cancel_marks[idx];
        surviving || has_surviving_dep_from_spec(dep_indices, job_states, cancel_marks, from + 1)
    }
}

/// Spec: does this job have any surviving (completed, not-cancelled) dependency?
pub open spec fn has_surviving_dep_spec(
    dep_indices: Seq<usize>,
    job_states: Seq<u8>,
    cancel_marks: Seq<bool>,
) -> bool {
    has_surviving_dep_from_spec(dep_indices, job_states, cancel_marks, 0)
}

// r[impl job-cancel.has-surviving-dep]
/// Check if a job has at least one completed dependency not marked for cancel.
pub fn has_surviving_dep(
    dep_indices: &[usize],
    job_states: &[u8],
    cancel_marks: &[bool],
) -> (result: bool)
    requires
        job_states@.len() == cancel_marks@.len(),
    ensures result == has_surviving_dep_spec(dep_indices@, job_states@, cancel_marks@)
{
    let mut i: usize = 0;
    while i < dep_indices.len()
        invariant
            0 <= i <= dep_indices@.len(),
            job_states@.len() == cancel_marks@.len(),
            has_surviving_dep_spec(dep_indices@, job_states@, cancel_marks@)
                <==> has_surviving_dep_from_spec(dep_indices@, job_states@, cancel_marks@, i as int),
        decreases dep_indices@.len() - i,
    {
        let idx = dep_indices[i];
        if idx < job_states.len()
            && job_states[idx] == COMPLETED
            && !cancel_marks[idx]
        {
            return true;
        }
        i = i + 1;
    }
    false
}

// ============================================================
// should_cancel_job: single job decision
// ============================================================

// r[depends job-cancel.should-cancel]
/// Spec: a job should be cancelled if:
/// 1. It is not already terminal
/// 2. It has at least one dependency (not a root job)
/// 3. It has no surviving (completed) dependency
pub open spec fn should_cancel_job_spec(
    state: u8,
    dep_indices: Seq<usize>,
    job_states: Seq<u8>,
    cancel_marks: Seq<bool>,
) -> bool {
    !is_terminal_spec(state)
    && dep_indices.len() > 0
    && !has_surviving_dep_spec(dep_indices, job_states, cancel_marks)
}

// r[impl job-cancel.should-cancel]
/// Decide if a single job should be cancelled.
pub fn should_cancel_job(
    state: u8,
    dep_indices: &[usize],
    job_states: &[u8],
    cancel_marks: &[bool],
) -> (result: bool)
    requires job_states@.len() == cancel_marks@.len()
    ensures result == should_cancel_job_spec(state, dep_indices@, job_states@, cancel_marks@)
{
    !is_terminal(state)
        && dep_indices.len() > 0
        && !has_surviving_dep(dep_indices, job_states, cancel_marks)
}

fn is_terminal(state: u8) -> (result: bool)
    ensures result == is_terminal_spec(state)
{
    state == COMPLETED || state == FAILED || state == CANCELLED
}

// ============================================================
// count_new_cancellations: count how many new marks a level adds
// ============================================================

// r[depends job-cancel.count-new]
/// Spec: count new cancellations in one pass from index `from`.
pub open spec fn count_new_from_spec(
    all_deps: Seq<Seq<usize>>,
    job_states: Seq<u8>,
    cancel_marks: Seq<bool>,
    from: int,
) -> int
    decreases all_deps.len() - from
{
    if from < 0 || from >= all_deps.len() as int {
        0int
    } else {
        let would_cancel = should_cancel_job_spec(
            job_states[from],
            all_deps[from],
            job_states,
            cancel_marks,
        ) && !cancel_marks[from];
        let rest = count_new_from_spec(all_deps, job_states, cancel_marks, from + 1);
        if would_cancel { rest + 1 } else { rest }
    }
}

/// Spec: total new cancellations in one propagation pass.
pub open spec fn count_new_cancellations_spec(
    all_deps: Seq<Seq<usize>>,
    job_states: Seq<u8>,
    cancel_marks: Seq<bool>,
) -> int {
    count_new_from_spec(all_deps, job_states, cancel_marks, 0)
}

// ============================================================
// Safety proofs
// ============================================================

// r[verify job-cancel.should-cancel]
/// Completed jobs are never cancelled.
proof fn completed_never_cancelled(
    dep_indices: Seq<usize>,
    job_states: Seq<u8>,
    cancel_marks: Seq<bool>,
)
    ensures !should_cancel_job_spec(COMPLETED, dep_indices, job_states, cancel_marks)
{}

// r[verify job-cancel.should-cancel]
/// Failed jobs are never cancelled.
proof fn failed_never_cancelled(
    dep_indices: Seq<usize>,
    job_states: Seq<u8>,
    cancel_marks: Seq<bool>,
)
    ensures !should_cancel_job_spec(FAILED, dep_indices, job_states, cancel_marks)
{}

// r[verify job-cancel.should-cancel]
/// Already-cancelled jobs are not re-cancelled.
proof fn already_cancelled_unchanged(
    dep_indices: Seq<usize>,
    job_states: Seq<u8>,
    cancel_marks: Seq<bool>,
)
    ensures !should_cancel_job_spec(CANCELLED, dep_indices, job_states, cancel_marks)
{}

// r[verify job-cancel.should-cancel]
/// Root jobs (no deps) are never cancelled by propagation.
proof fn root_never_propagated(
    state: u8,
    job_states: Seq<u8>,
    cancel_marks: Seq<bool>,
)
    ensures !should_cancel_job_spec(state, seq![], job_states, cancel_marks)
{}

// r[verify job-cancel.has-surviving-dep]
/// No dependencies means no surviving dep.
proof fn no_deps_no_survivor(
    job_states: Seq<u8>,
    cancel_marks: Seq<bool>,
)
    ensures !has_surviving_dep_spec(seq![], job_states, cancel_marks)
{}

// r[verify job-cancel.count-new]
/// New cancellations count is bounded by total job count.
proof fn count_bounded(
    all_deps: Seq<Seq<usize>>,
    job_states: Seq<u8>,
    cancel_marks: Seq<bool>,
)
    ensures count_new_cancellations_spec(all_deps, job_states, cancel_marks) <= all_deps.len() as int
{
    count_bounded_from(all_deps, job_states, cancel_marks, 0);
}

proof fn count_bounded_from(
    all_deps: Seq<Seq<usize>>,
    job_states: Seq<u8>,
    cancel_marks: Seq<bool>,
    from: int,
)
    requires 0 <= from <= all_deps.len() as int
    ensures count_new_from_spec(all_deps, job_states, cancel_marks, from)
        <= all_deps.len() as int - from
    decreases all_deps.len() - from
{
    if from < all_deps.len() as int {
        count_bounded_from(all_deps, job_states, cancel_marks, from + 1);
    }
}

// r[verify job-cancel.count-new]
/// New cancellations count is non-negative.
proof fn count_nonneg(
    all_deps: Seq<Seq<usize>>,
    job_states: Seq<u8>,
    cancel_marks: Seq<bool>,
)
    ensures count_new_cancellations_spec(all_deps, job_states, cancel_marks) >= 0
{
    count_nonneg_from(all_deps, job_states, cancel_marks, 0);
}

proof fn count_nonneg_from(
    all_deps: Seq<Seq<usize>>,
    job_states: Seq<u8>,
    cancel_marks: Seq<bool>,
    from: int,
)
    requires 0 <= from <= all_deps.len() as int
    ensures count_new_from_spec(all_deps, job_states, cancel_marks, from) >= 0
    decreases all_deps.len() - from
{
    if from < all_deps.len() as int {
        count_nonneg_from(all_deps, job_states, cancel_marks, from + 1);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify job-cancel.should-cancel]
    #[test]
    fn test_should_cancel() {
        // Pending job with one cancelled dep and no surviving → cancel
        let states = [0u8, 5]; // job 0 = Pending, job 1 = Cancelled
        let marks = [false, true];
        assert!(should_cancel_job(0, &[1usize], &states, &marks));

        // Pending job with one completed dep → don't cancel (surviving path)
        let states2 = [0u8, 3]; // job 1 = Completed
        let marks2 = [false, false];
        assert!(!should_cancel_job(0, &[1usize], &states2, &marks2));
    }

    // r[verify job-cancel.should-cancel]
    #[test]
    fn test_terminal_not_cancelled() {
        let states = [3u8, 5]; // Completed, Cancelled
        let marks = [false, true];
        assert!(!should_cancel_job(3, &[1usize], &states, &marks)); // Completed
        assert!(!should_cancel_job(4, &[1usize], &states, &marks)); // Failed
        assert!(!should_cancel_job(5, &[1usize], &states, &marks)); // Cancelled
    }

    // r[verify job-cancel.should-cancel]
    #[test]
    fn test_root_not_propagated() {
        let states = [0u8];
        let marks = [false];
        assert!(!should_cancel_job(0, &[], &states, &marks));
    }

    // r[verify job-cancel.has-surviving-dep]
    #[test]
    fn test_surviving_dep() {
        // One completed dep among cancelled ones → surviving
        let states = [5u8, 3, 5]; // Cancelled, Completed, Cancelled
        let marks = [true, false, true];
        assert!(has_surviving_dep(&[0usize, 1, 2], &states, &marks));

        // All deps cancelled → no survivor
        let states2 = [5u8, 5, 5];
        let marks2 = [true, true, true];
        assert!(!has_surviving_dep(&[0usize, 1, 2], &states2, &marks2));
    }

    // r[verify job-cancel.should-cancel]
    #[test]
    fn test_mixed_deps() {
        // Dep 0: Failed, Dep 1: Cancelled → no surviving, should cancel
        let states = [0u8, 4, 5]; // self=Pending, dep1=Failed, dep2=Cancelled
        let marks = [false, false, true];
        assert!(should_cancel_job(0, &[1usize, 2], &states, &marks));

        // Dep 0: Failed, Dep 1: Completed → surviving, don't cancel
        let states2 = [0u8, 4, 3];
        let marks2 = [false, false, false];
        assert!(!should_cancel_job(0, &[1usize, 2], &states2, &marks2));
    }

    proptest! {
        // r[verify job-cancel.should-cancel]
        #[test]
        fn prop_completed_never_cancelled(
            num_deps in 0usize..10,
            num_jobs in 1usize..20,
        ) {
            let states: Vec<u8> = vec![3; num_jobs]; // All completed
            let marks: Vec<bool> = vec![false; num_jobs];
            let deps: Vec<usize> = (0..num_deps).map(|i| i % num_jobs).collect();
            prop_assert!(!should_cancel_job(3, &deps, &states, &marks));
        }

        // r[verify job-cancel.should-cancel]
        #[test]
        fn prop_root_never_propagated(state in 0u8..6) {
            let states = vec![state];
            let marks = vec![false];
            prop_assert!(!should_cancel_job(state, &[], &states, &marks));
        }

        // r[verify job-cancel.has-surviving-dep]
        #[test]
        fn prop_one_completed_survives(
            num_deps in 1usize..10,
            completed_pos in 0usize..10,
        ) {
            if completed_pos < num_deps {
                let num_jobs = num_deps;
                let mut states: Vec<u8> = vec![5; num_jobs]; // All cancelled
                states[completed_pos] = 3; // One completed
                let marks: Vec<bool> = states.iter().map(|&s| s == 5).collect();
                let deps: Vec<usize> = (0..num_deps).collect();
                prop_assert!(has_surviving_dep(&deps, &states, &marks));
            }
        }

        #[test]
        fn prop_cancel_idempotent(
            state in 0u8..6,
            num_deps in 1usize..5,
        ) {
            // CANCELLED=5, COMPLETED=3, FAILED=4 are terminal
            let states: Vec<u8> = vec![5; num_deps]; // all deps cancelled
            let marks: Vec<bool> = vec![true; num_deps];
            let deps: Vec<usize> = (0..num_deps).collect();
            let first = should_cancel_job(state, &deps, &states, &marks);
            if first {
                // Already-cancelled job (state=5) should not cancel again
                prop_assert!(!should_cancel_job(5, &deps, &states, &marks));
            }
        }

        #[test]
        fn prop_terminal_never_cancels(
            terminal in proptest::sample::select(vec![3u8, 4, 5]),
            num_deps in 1usize..5,
        ) {
            let states: Vec<u8> = vec![5; num_deps];
            let marks: Vec<bool> = vec![true; num_deps];
            let deps: Vec<usize> = (0..num_deps).collect();
            prop_assert!(!should_cancel_job(terminal, &deps, &states, &marks));
        }

        #[test]
        fn prop_no_deps_never_cancels(
            state in 0u8..3,
        ) {
            let states: Vec<u8> = vec![state];
            let marks: Vec<bool> = vec![false];
            prop_assert!(!should_cancel_job(state, &[], &states, &marks));
        }
    }
}
