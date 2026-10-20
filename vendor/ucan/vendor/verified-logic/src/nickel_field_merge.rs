//! Verified Nickel field-level merge resolution.
//!
//! Models the priority-based conflict resolution in `core/src/eval/merge.rs`:
//! higher priority wins, equal priority requires recursion or equality,
//! Bottom (default) is overridden by anything, Top (force) overrides anything.

use vstd::prelude::*;

use crate::nickel_priority::*;

verus! {

/// Result of merging two fields.
#[derive(Debug, PartialEq, Eq)]
pub enum FieldMergeResult {
    /// Left field wins (higher priority).
    Left(u64, MergePriority),
    /// Right field wins (higher priority).
    Right(u64, MergePriority),
    /// Equal priority, values are equal — either value works.
    Equal(u64, MergePriority),
    /// Equal priority, values differ — conflict.
    Conflict,
}

/// Spec: resolve two fields by priority.
// r[depends nickel-field-merge.higher-wins]
// r[depends nickel-field-merge.equal-recurse]
// r[depends nickel-field-merge.default-override]
pub open spec fn field_merge_spec(
    v1: u64, p1: MergePriority,
    v2: u64, p2: MergePriority,
) -> FieldMergeResult {
    if priority_lt(p2, p1) {
        FieldMergeResult::Left(v1, p1)
    } else if priority_lt(p1, p2) {
        FieldMergeResult::Right(v2, p2)
    } else {
        // Equal priority
        if v1 == v2 {
            FieldMergeResult::Equal(v1, p1)
        } else {
            FieldMergeResult::Conflict
        }
    }
}

/// Exec: resolve two fields by priority.
// r[impl nickel-field-merge.higher-wins]
// r[impl nickel-field-merge.equal-recurse]
// r[impl nickel-field-merge.default-override]
pub fn field_merge(
    v1: u64, p1: &MergePriority,
    v2: u64, p2: &MergePriority,
) -> (result: FieldMergeResult)
    ensures result == field_merge_spec(v1, *p1, v2, *p2)
{
    let cmp = priority_cmp(p1, p2);
    if cmp > 0 {
        FieldMergeResult::Left(v1, match p1 {
            MergePriority::Bottom => MergePriority::Bottom,
            MergePriority::Neutral => MergePriority::Neutral,
            MergePriority::Numeral(n) => MergePriority::Numeral(*n),
            MergePriority::Top => MergePriority::Top,
        })
    } else if cmp < 0 {
        FieldMergeResult::Right(v2, match p2 {
            MergePriority::Bottom => MergePriority::Bottom,
            MergePriority::Neutral => MergePriority::Neutral,
            MergePriority::Numeral(n) => MergePriority::Numeral(*n),
            MergePriority::Top => MergePriority::Top,
        })
    } else {
        if v1 == v2 {
            FieldMergeResult::Equal(v1, match p1 {
                MergePriority::Bottom => MergePriority::Bottom,
                MergePriority::Neutral => MergePriority::Neutral,
                MergePriority::Numeral(n) => MergePriority::Numeral(*n),
                MergePriority::Top => MergePriority::Top,
            })
        } else {
            FieldMergeResult::Conflict
        }
    }
}

// --- Proofs ---

// r[verify nickel-field-merge.higher-wins]
/// Higher priority always wins.
proof fn higher_priority_wins(v1: u64, p1: MergePriority, v2: u64, p2: MergePriority)
    requires priority_lt(p1, p2)
    ensures field_merge_spec(v1, p1, v2, p2) == FieldMergeResult::Right(v2, p2)
{}

// r[verify nickel-field-merge.equal-recurse]
/// Equal priority with equal values succeeds.
proof fn equal_priority_equal_values(v: u64, p1: MergePriority, p2: MergePriority)
    requires priority_eq(p1, p2)
    ensures field_merge_spec(v, p1, v, p2) == FieldMergeResult::Equal(v, p1)
{}

// r[verify nickel-field-merge.equal-recurse]
/// Equal priority with different values is a conflict.
proof fn equal_priority_conflict(v1: u64, p1: MergePriority, v2: u64, p2: MergePriority)
    requires priority_eq(p1, p2), v1 != v2
    ensures field_merge_spec(v1, p1, v2, p2) == FieldMergeResult::Conflict
{}

// r[verify nickel-field-merge.default-override]
/// Bottom (default) is overridden by any non-Bottom priority.
proof fn default_overridden(v1: u64, v2: u64, p2: MergePriority)
    requires priority_lt(MergePriority::Bottom, p2)
    ensures field_merge_spec(v1, MergePriority::Bottom, v2, p2) == FieldMergeResult::Right(v2, p2)
{}

// r[verify nickel-field-merge.default-override]
/// Top (force) overrides any non-Top priority.
proof fn force_overrides(v1: u64, p1: MergePriority, v2: u64)
    requires priority_lt(p1, MergePriority::Top)
    ensures field_merge_spec(v1, p1, v2, MergePriority::Top) == FieldMergeResult::Right(v2, MergePriority::Top)
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify nickel-field-merge.higher-wins]
    #[test]
    fn test_higher_priority_wins() {
        let r = field_merge(1, &MergePriority::Neutral, 2, &MergePriority::Top);
        assert_eq!(r, FieldMergeResult::Right(2, MergePriority::Top));
    }

    // r[verify nickel-field-merge.default-override]
    #[test]
    fn test_force_overrides_default() {
        let r = field_merge(1, &MergePriority::Bottom, 2, &MergePriority::Top);
        assert_eq!(r, FieldMergeResult::Right(2, MergePriority::Top));
    }

    // r[verify nickel-field-merge.equal-recurse]
    #[test]
    fn test_equal_priority_conflict() {
        let r = field_merge(1, &MergePriority::Neutral, 2, &MergePriority::Neutral);
        assert_eq!(r, FieldMergeResult::Conflict);
    }

    // r[verify nickel-field-merge.equal-recurse]
    #[test]
    fn test_equal_priority_same_value() {
        let r = field_merge(5, &MergePriority::Neutral, 5, &MergePriority::Neutral);
        assert_eq!(r, FieldMergeResult::Equal(5, MergePriority::Neutral));
    }
}
