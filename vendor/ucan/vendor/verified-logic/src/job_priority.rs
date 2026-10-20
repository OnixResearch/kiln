//! Verified job priority ordering.
//!
//! Total order on (priority_class, sequence_number) pairs. Higher
//! priority class wins; within the same class, lower sequence number
//! (older job, submitted earlier) wins. Proofs cover transitivity,
//! antisymmetry, totality, and correct selection from a set.

use vstd::prelude::*;

verus! {

// ============================================================
// is_higher_priority
// ============================================================

// r[depends job-priority.is-higher]
/// Spec: strict "higher priority" comparison.
/// Higher class wins. Same class: lower sequence (older) wins.
pub open spec fn is_higher_priority_spec(
    class_a: u64, seq_a: u64,
    class_b: u64, seq_b: u64,
) -> bool {
    class_a > class_b || (class_a == class_b && seq_a < seq_b)
}

// r[impl job-priority.is-higher]
/// Returns true if job A has strictly higher priority than job B.
pub fn is_higher_priority(
    class_a: u64, seq_a: u64,
    class_b: u64, seq_b: u64,
) -> (result: bool)
    ensures result == is_higher_priority_spec(class_a, seq_a, class_b, seq_b)
{
    class_a > class_b || (class_a == class_b && seq_a < seq_b)
}

// ============================================================
// is_equal_priority
// ============================================================

// r[depends job-priority.is-equal]
/// Spec: two jobs have identical priority.
pub open spec fn is_equal_priority_spec(
    class_a: u64, seq_a: u64,
    class_b: u64, seq_b: u64,
) -> bool {
    class_a == class_b && seq_a == seq_b
}

// r[impl job-priority.is-equal]
/// Check if two jobs have identical priority.
pub fn is_equal_priority(
    class_a: u64, seq_a: u64,
    class_b: u64, seq_b: u64,
) -> (result: bool)
    ensures result == is_equal_priority_spec(class_a, seq_a, class_b, seq_b)
{
    class_a == class_b && seq_a == seq_b
}

// ============================================================
// select_highest_priority
// ============================================================

// r[depends job-priority.select-highest]
/// Spec: index of the highest-priority element from index `from` onward.
pub open spec fn select_highest_from_spec(
    classes: Seq<u64>, seqs: Seq<u64>, from: int,
) -> int
    recommends classes.len() == seqs.len(), 0 <= from < classes.len() as int
    decreases classes.len() - from
{
    if from < 0 || from >= classes.len() as int {
        from
    } else if from == classes.len() as int - 1 {
        from
    } else {
        let rest_best = select_highest_from_spec(classes, seqs, from + 1);
        if is_higher_priority_spec(classes[from], seqs[from], classes[rest_best], seqs[rest_best]) {
            from
        } else {
            rest_best
        }
    }
}

/// Spec: index of the highest-priority element.
pub open spec fn select_highest_priority_spec(
    classes: Seq<u64>, seqs: Seq<u64>,
) -> int
    recommends classes.len() > 0, classes.len() == seqs.len()
{
    select_highest_from_spec(classes, seqs, 0)
}

// r[impl job-priority.select-highest]
/// Select the index of the highest-priority job from parallel arrays.
pub fn select_highest_priority(
    classes: &[u64], seqs: &[u64],
) -> (result: usize)
    requires classes@.len() > 0, classes@.len() == seqs@.len()
    ensures result == select_highest_priority_spec(classes@, seqs@) as usize
{
    // Right-to-left scan matching the recursive spec.
    let mut best: usize = classes.len() - 1;
    let mut i: usize = classes.len() - 1;
    while i > 0
        invariant
            0 <= i < classes@.len(),
            classes@.len() == seqs@.len(),
            i <= best,
            best < classes@.len(),
            best as int == select_highest_from_spec(classes@, seqs@, i as int),
        decreases i,
    {
        i = i - 1;
        if is_higher_priority(classes[i], seqs[i], classes[best], seqs[best]) {
            best = i;
        }
    }
    best
}

// ============================================================
// Ordering proofs
// ============================================================

// r[verify job-priority.is-higher]
/// Transitivity: A > B ∧ B > C → A > C.
proof fn transitivity(
    ca: u64, sa: u64,
    cb: u64, sb: u64,
    cc: u64, sc: u64,
)
    requires
        is_higher_priority_spec(ca, sa, cb, sb),
        is_higher_priority_spec(cb, sb, cc, sc),
    ensures
        is_higher_priority_spec(ca, sa, cc, sc)
{}

// r[verify job-priority.is-higher]
/// Antisymmetry: A > B → ¬(B > A).
proof fn antisymmetry(
    ca: u64, sa: u64,
    cb: u64, sb: u64,
)
    requires is_higher_priority_spec(ca, sa, cb, sb)
    ensures !is_higher_priority_spec(cb, sb, ca, sa)
{}

// r[verify job-priority.is-higher]
/// Totality: for any two distinct pairs, one is higher.
proof fn totality(
    ca: u64, sa: u64,
    cb: u64, sb: u64,
)
    requires !is_equal_priority_spec(ca, sa, cb, sb)
    ensures
        is_higher_priority_spec(ca, sa, cb, sb)
        || is_higher_priority_spec(cb, sb, ca, sa)
{}

// r[verify job-priority.is-higher]
/// Equal pairs: neither is higher than the other.
proof fn equal_neither_higher(
    ca: u64, sa: u64,
    cb: u64, sb: u64,
)
    requires is_equal_priority_spec(ca, sa, cb, sb)
    ensures
        !is_higher_priority_spec(ca, sa, cb, sb),
        !is_higher_priority_spec(cb, sb, ca, sa),
{}

// r[verify job-priority.select-highest]
/// Selected index is valid.
proof fn select_in_bounds(classes: Seq<u64>, seqs: Seq<u64>)
    requires classes.len() > 0, classes.len() == seqs.len()
    ensures ({
        let idx = select_highest_priority_spec(classes, seqs);
        0 <= idx < classes.len() as int
    })
{
    select_in_bounds_from(classes, seqs, 0);
}

proof fn select_in_bounds_from(classes: Seq<u64>, seqs: Seq<u64>, from: int)
    requires
        0 <= from < classes.len() as int,
        classes.len() == seqs.len(),
    ensures ({
        let idx = select_highest_from_spec(classes, seqs, from);
        from <= idx < classes.len() as int
    })
    decreases classes.len() - from
{
    if from < classes.len() as int - 1 {
        select_in_bounds_from(classes, seqs, from + 1);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify job-priority.is-higher]
    #[test]
    fn test_higher_priority() {
        // Higher class wins
        assert!(is_higher_priority(3, 10, 1, 5));
        // Same class, lower seq wins
        assert!(is_higher_priority(1, 5, 1, 10));
        // Equal — neither higher
        assert!(!is_higher_priority(1, 5, 1, 5));
        // Lower class loses
        assert!(!is_higher_priority(1, 5, 3, 10));
    }

    // r[verify job-priority.select-highest]
    #[test]
    fn test_select_highest() {
        let classes = [1, 3, 2];
        let seqs = [10, 5, 1];
        assert_eq!(select_highest_priority(&classes, &seqs), 1); // class 3 wins

        let classes2 = [2, 2, 2];
        let seqs2 = [10, 3, 7];
        assert_eq!(select_highest_priority(&classes2, &seqs2), 1); // seq 3 is lowest
    }

    // r[verify job-priority.select-highest]
    #[test]
    fn test_select_single() {
        assert_eq!(select_highest_priority(&[5], &[1]), 0);
    }

    proptest! {
        // r[verify job-priority.is-higher]
        #[test]
        fn prop_antisymmetry(ca in 0u64..10, sa in 0u64..100, cb in 0u64..10, sb in 0u64..100) {
            if is_higher_priority(ca, sa, cb, sb) {
                prop_assert!(!is_higher_priority(cb, sb, ca, sa));
            }
        }

        // r[verify job-priority.is-higher]
        #[test]
        fn prop_totality(ca in 0u64..10, sa in 0u64..100, cb in 0u64..10, sb in 0u64..100) {
            if !is_equal_priority(ca, sa, cb, sb) {
                prop_assert!(
                    is_higher_priority(ca, sa, cb, sb)
                    || is_higher_priority(cb, sb, ca, sa)
                );
            }
        }

        // r[verify job-priority.is-higher]
        #[test]
        fn prop_transitivity(
            ca in 0u64..10, sa in 0u64..100,
            cb in 0u64..10, sb in 0u64..100,
            cc in 0u64..10, sc in 0u64..100,
        ) {
            if is_higher_priority(ca, sa, cb, sb) && is_higher_priority(cb, sb, cc, sc) {
                prop_assert!(is_higher_priority(ca, sa, cc, sc));
            }
        }

        // r[verify job-priority.select-highest]
        #[test]
        fn prop_select_in_bounds(
            classes in proptest::collection::vec(0u64..10, 1..20),
        ) {
            let seqs: Vec<u64> = (0..classes.len() as u64).collect();
            let idx = select_highest_priority(&classes, &seqs);
            prop_assert!(idx < classes.len());
        }

        // r[verify job-priority.select-highest]
        #[test]
        fn prop_select_is_best(
            classes in proptest::collection::vec(0u64..10, 1..20),
        ) {
            let seqs: Vec<u64> = (0..classes.len() as u64).collect();
            let idx = select_highest_priority(&classes, &seqs);
            for i in 0..classes.len() {
                if i != idx {
                    // selected is >= every other element
                    prop_assert!(
                        is_higher_priority(classes[idx], seqs[idx], classes[i], seqs[i])
                        || is_equal_priority(classes[idx], seqs[idx], classes[i], seqs[i])
                    );
                }
            }
        }
    }
}
