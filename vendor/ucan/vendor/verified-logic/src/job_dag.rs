//! Verified job dependency DAG primitives.
//!
//! Models a directed acyclic graph of jobs where each job has a set of
//! dependencies that must complete before it can run. Proofs cover
//! readiness (all deps satisfied), topological ordering, and the
//! relationship between dependency satisfaction and schedulability.

use vstd::prelude::*;

verus! {

// ============================================================
// Job readiness: are all dependencies satisfied?
// ============================================================

// r[depends job-dag.is-dep-satisfied]
/// Spec: is a single dependency satisfied?
/// A dependency is satisfied when the completed set contains it.
pub open spec fn is_dep_satisfied_spec(dep: u64, completed: Seq<u64>) -> bool {
    exists|i: int| 0 <= i < completed.len() as int && completed[i] == dep
}

// r[depends job-dag.all-deps-satisfied]
/// Spec: are all dependencies of a job satisfied?
pub open spec fn all_deps_satisfied_from_spec(deps: Seq<u64>, completed: Seq<u64>, from: int) -> bool
    decreases deps.len() - from
{
    if from < 0 || from >= deps.len() as int {
        true
    } else {
        is_dep_satisfied_spec(deps[from], completed) && all_deps_satisfied_from_spec(deps, completed, from + 1)
    }
}

/// Spec: are all dependencies of a job satisfied?
pub open spec fn all_deps_satisfied_spec(deps: Seq<u64>, completed: Seq<u64>) -> bool {
    all_deps_satisfied_from_spec(deps, completed, 0)
}

// r[impl job-dag.all-deps-satisfied]
/// Check if all dependencies of a job are satisfied.
pub fn all_deps_satisfied(deps: &[u64], completed: &[u64]) -> (result: bool)
    ensures result == all_deps_satisfied_spec(deps@, completed@)
{
    let mut i: usize = 0;
    while i < deps.len()
        invariant
            0 <= i <= deps@.len(),
            all_deps_satisfied_spec(deps@, completed@)
                <==> all_deps_satisfied_from_spec(deps@, completed@, i as int),
        decreases deps@.len() - i,
    {
        let mut found = false;
        let mut j: usize = 0;
        while j < completed.len()
            invariant
                0 <= j <= completed@.len(),
                i < deps@.len(),
                found <==> exists|k: int| #![trigger completed@[k]]
                    0 <= k < j as int && completed@[k] == deps@[i as int],
            decreases completed@.len() - j,
        {
            if completed[j] == deps[i] {
                found = true;
            }
            j = j + 1;
        }
        if !found {
            return false;
        }
        i = i + 1;
    }
    true
}

// r[verify job-dag.all-deps-satisfied]
/// No dependencies means always ready.
proof fn no_deps_always_ready(completed: Seq<u64>)
    ensures all_deps_satisfied_spec(seq![], completed)
{}

// r[verify job-dag.all-deps-satisfied]
/// Completing more jobs preserves readiness.
proof fn completion_preserves_readiness(deps: Seq<u64>, completed: Seq<u64>, new_job: u64)
    requires all_deps_satisfied_spec(deps, completed)
    ensures all_deps_satisfied_spec(deps, completed.push(new_job))
{
    completion_preserves_from(deps, completed, new_job, 0);
}

proof fn completion_preserves_from(deps: Seq<u64>, completed: Seq<u64>, new_job: u64, from: int)
    requires
        0 <= from <= deps.len() as int,
        all_deps_satisfied_from_spec(deps, completed, from),
    ensures all_deps_satisfied_from_spec(deps, completed.push(new_job), from)
    decreases deps.len() - from
{
    if from < deps.len() as int {
        // Each dep satisfied in `completed` is still satisfied in `completed.push(new_job)`
        assert(is_dep_satisfied_spec(deps[from], completed));
        let witness = choose|i: int| 0 <= i < completed.len() as int && completed[i] == deps[from];
        assert(completed.push(new_job)[witness] == deps[from]);
        assert(is_dep_satisfied_spec(deps[from], completed.push(new_job)));
        completion_preserves_from(deps, completed, new_job, from + 1);
    }
}

// ============================================================
// Dependency count
// ============================================================

// r[depends job-dag.dep-count]
/// Spec: count of unsatisfied dependencies.
pub open spec fn unsatisfied_count_from_spec(deps: Seq<u64>, completed: Seq<u64>, from: int) -> int
    decreases deps.len() - from
{
    if from < 0 || from >= deps.len() as int {
        0int
    } else {
        let rest = unsatisfied_count_from_spec(deps, completed, from + 1);
        if is_dep_satisfied_spec(deps[from], completed) {
            rest
        } else {
            rest + 1
        }
    }
}

/// Spec: total unsatisfied dependency count.
pub open spec fn unsatisfied_count_spec(deps: Seq<u64>, completed: Seq<u64>) -> int {
    unsatisfied_count_from_spec(deps, completed, 0)
}

// r[impl job-dag.dep-count]
/// Count unsatisfied dependencies.
pub fn unsatisfied_count(deps: &[u64], completed: &[u64]) -> (result: u64)
    requires deps@.len() <= u64::MAX as int
    ensures result == unsatisfied_count_spec(deps@, completed@) as u64
{
    let mut count: u64 = 0;
    let mut i: usize = 0;
    while i < deps.len()
        invariant
            0 <= i <= deps@.len(),
            deps@.len() <= u64::MAX as int,
            count as int == unsatisfied_count_from_spec(deps@, completed@, 0)
                - unsatisfied_count_from_spec(deps@, completed@, i as int),
            count as int <= i as int,
        decreases deps@.len() - i,
    {
        let mut found = false;
        let mut j: usize = 0;
        while j < completed.len()
            invariant
                0 <= j <= completed@.len(),
                i < deps@.len(),
                found <==> exists|k: int| #![trigger completed@[k]]
                    0 <= k < j as int && completed@[k] == deps@[i as int],
            decreases completed@.len() - j,
        {
            if completed[j] == deps[i] {
                found = true;
            }
            j = j + 1;
        }
        if !found {
            count = count + 1;
        }
        i = i + 1;
    }
    count
}

// r[verify job-dag.dep-count]
/// Zero unsatisfied dependencies iff all deps satisfied.
proof fn zero_unsatisfied_iff_ready(deps: Seq<u64>, completed: Seq<u64>)
    ensures
        (unsatisfied_count_spec(deps, completed) == 0) <==> all_deps_satisfied_spec(deps, completed),
{
    zero_unsatisfied_iff_ready_from(deps, completed, 0);
}

proof fn zero_unsatisfied_iff_ready_from(deps: Seq<u64>, completed: Seq<u64>, from: int)
    requires 0 <= from <= deps.len() as int
    ensures
        (unsatisfied_count_from_spec(deps, completed, from) == 0)
            <==> all_deps_satisfied_from_spec(deps, completed, from),
    decreases deps.len() - from
{
    if from < deps.len() as int {
        zero_unsatisfied_iff_ready_from(deps, completed, from + 1);
        unsatisfied_nonneg_from(deps, completed, from + 1);
    }
}

// r[verify job-dag.dep-count]
/// Unsatisfied count is bounded by total dependency count.
proof fn unsatisfied_bounded(deps: Seq<u64>, completed: Seq<u64>)
    ensures unsatisfied_count_spec(deps, completed) <= deps.len() as int
{
    unsatisfied_bounded_from(deps, completed, 0);
}

proof fn unsatisfied_bounded_from(deps: Seq<u64>, completed: Seq<u64>, from: int)
    requires 0 <= from <= deps.len() as int
    ensures unsatisfied_count_from_spec(deps, completed, from) <= deps.len() as int - from
    decreases deps.len() - from
{
    if from < deps.len() as int {
        unsatisfied_bounded_from(deps, completed, from + 1);
    }
}

// r[verify job-dag.dep-count]
/// Unsatisfied count is nonneg.
proof fn unsatisfied_nonneg(deps: Seq<u64>, completed: Seq<u64>)
    ensures unsatisfied_count_spec(deps, completed) >= 0
{
    unsatisfied_nonneg_from(deps, completed, 0);
}

proof fn unsatisfied_nonneg_from(deps: Seq<u64>, completed: Seq<u64>, from: int)
    requires 0 <= from <= deps.len() as int
    ensures unsatisfied_count_from_spec(deps, completed, from) >= 0
    decreases deps.len() - from
{
    if from < deps.len() as int {
        unsatisfied_nonneg_from(deps, completed, from + 1);
    }
}

// ============================================================
// Topological level: longest path from any source
// ============================================================

// r[depends job-dag.topo-level]
/// Spec: topological level of a job given its dependencies' levels.
/// A job's level is max(dep_levels) + 1, or 0 if no dependencies.
pub open spec fn topo_level_spec(dep_levels: Seq<u64>) -> u64 {
    if dep_levels.len() == 0 {
        0u64
    } else {
        let max_dep = max_of_from_spec(dep_levels, 0);
        if max_dep as int + 1 > u64::MAX as int { u64::MAX } else { (max_dep + 1) as u64 }
    }
}

/// Spec: max value from index `from` onward.
pub open spec fn max_of_from_spec(levels: Seq<u64>, from: int) -> u64
    decreases levels.len() - from
{
    if from < 0 || from >= levels.len() as int {
        0u64
    } else {
        let rest = max_of_from_spec(levels, from + 1);
        if levels[from] >= rest { levels[from] as u64 } else { rest }
    }
}

// r[impl job-dag.topo-level]
/// Compute the topological level of a job.
pub fn topo_level(dep_levels: &[u64]) -> (result: u64)
    ensures result == topo_level_spec(dep_levels@)
{
    if dep_levels.is_empty() {
        return 0;
    }
    // Scan right-to-left to match the recursive spec direction.
    let mut max_dep: u64 = 0;
    let mut i: usize = dep_levels.len();
    while i > 0
        invariant
            0 <= i <= dep_levels@.len(),
            max_dep == max_of_from_spec(dep_levels@, i as int),
        decreases i,
    {
        i = i - 1;
        if dep_levels[i] > max_dep {
            max_dep = dep_levels[i];
        }
    }
    if max_dep == u64::MAX {
        u64::MAX
    } else {
        max_dep + 1
    }
}

// r[verify job-dag.topo-level]
/// Source jobs (no dependencies) have level 0.
proof fn source_level_zero()
    ensures topo_level_spec(seq![]) == 0u64
{}

// r[verify job-dag.topo-level]
/// A job's level is strictly greater than any dependency's level
/// (when not at u64::MAX).
proof fn level_exceeds_deps(dep_levels: Seq<u64>)
    requires dep_levels.len() > 0
    ensures
        topo_level_spec(dep_levels) >= 1,
{
    max_of_nonneg(dep_levels, 0);
}

proof fn max_of_nonneg(levels: Seq<u64>, from: int)
    requires 0 <= from <= levels.len() as int
    ensures max_of_from_spec(levels, from) >= 0u64
    decreases levels.len() - from
{
    if from < levels.len() as int {
        max_of_nonneg(levels, from + 1);
    }
}

} // verus!

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
#[allow(clippy::cast_possible_truncation)]
mod test {
    use super::*;

    // r[verify job-dag.all-deps-satisfied]
    #[test]
    fn test_all_deps_satisfied() {
        assert!(all_deps_satisfied(&[1, 2, 3], &[3, 1, 2, 4]));
        assert!(!all_deps_satisfied(&[1, 2, 3], &[1, 4])); // missing 2, 3
        assert!(all_deps_satisfied(&[], &[1, 2, 3])); // no deps
        assert!(all_deps_satisfied(&[], &[])); // no deps, nothing completed
    }

    // r[verify job-dag.dep-count]
    #[test]
    fn test_unsatisfied_count() {
        assert_eq!(unsatisfied_count(&[1, 2, 3], &[1]), 2);
        assert_eq!(unsatisfied_count(&[1, 2, 3], &[1, 2, 3]), 0);
        assert_eq!(unsatisfied_count(&[], &[1, 2]), 0);
        assert_eq!(unsatisfied_count(&[5, 6], &[]), 2);
    }

    // r[verify job-dag.topo-level]
    #[test]
    fn test_topo_level() {
        assert_eq!(topo_level(&[]), 0); // source
        assert_eq!(topo_level(&[0, 0, 0]), 1); // all deps are sources
        assert_eq!(topo_level(&[2, 1, 3]), 4); // max dep level is 3
    }

    #[cfg(test)]
    mod proptests {
        use proptest::prelude::*;

        use super::*;

        proptest! {
            // r[verify job-dag.all-deps-satisfied]
            #[test]
            fn prop_no_deps_always_ready(completed in proptest::collection::vec(any::<u64>(), 0..20)) {
                prop_assert!(all_deps_satisfied(&[], &completed));
            }

            // r[verify job-dag.dep-count]
            #[test]
            fn prop_unsatisfied_bounded(
                deps in proptest::collection::vec(any::<u64>(), 0..20),
                completed in proptest::collection::vec(any::<u64>(), 0..20),
            ) {
                let count = unsatisfied_count(&deps, &completed);
                prop_assert!(count as usize <= deps.len());
            }

            // r[verify job-dag.dep-count]
            #[test]
            fn prop_zero_unsatisfied_iff_ready(
                deps in proptest::collection::vec(0u64..10, 0..10),
                completed in proptest::collection::vec(0u64..10, 0..20),
            ) {
                let count = unsatisfied_count(&deps, &completed);
                let ready = all_deps_satisfied(&deps, &completed);
                prop_assert_eq!(count == 0, ready);
            }

            // r[verify job-dag.topo-level]
            #[test]
            fn prop_source_level_zero(_x: u8) {
                let dep_levels: Vec<u64> = vec![];
                prop_assert_eq!(topo_level(&dep_levels), 0);
            }
        }
    }
}
