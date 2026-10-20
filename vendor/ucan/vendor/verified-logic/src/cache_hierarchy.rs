//! Verified two-level cache hierarchy policies.
//!
//! Models inclusive, exclusive, and non-inclusive L1/L2 relationships over
//! finite sets of cache-line tags. Inclusive hierarchies maintain `L1 ⊆ L2`;
//! exclusive hierarchies maintain `L1 ∩ L2 = ∅`; non-inclusive hierarchies
//! impose no structural constraint.

use vstd::prelude::*;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum HierarchyPolicy {
    Inclusive,
    Exclusive,
    NonInclusive,
}

// r[depends hierarchy.inclusive-invariant]
// r[depends hierarchy.exclusive-invariant]
// r[impl hierarchy.inclusive-invariant]
// r[impl hierarchy.exclusive-invariant]
pub open spec fn contains_line_spec(level: Seq<u64>, line: u64) -> bool {
    exists|i: int| 0 <= i < level.len() && level[i] == line
}

// r[impl hierarchy.capacity]
pub open spec fn no_duplicates_spec(level: Seq<u64>) -> bool {
    forall|i: int, j: int| 0 <= i < j < level.len() ==> level[i] != level[j]
}

// r[impl hierarchy.back-invalidation]
// r[impl hierarchy.exclusive-invariant]
pub open spec fn remove_prefix_spec(level: Seq<u64>, line: u64, upto: int) -> Seq<u64>
    recommends 0 <= upto <= level.len()
    decreases (if upto >= 0 { upto } else { 0 })
{
    if upto <= 0 {
        seq![]
    } else {
        let prefix = remove_prefix_spec(level, line, upto - 1);
        if level[upto - 1] == line {
            prefix
        } else {
            prefix.push(level[upto - 1])
        }
    }
}

// r[impl hierarchy.back-invalidation]
// r[impl hierarchy.exclusive-invariant]
pub open spec fn remove_line_spec(level: Seq<u64>, line: u64) -> Seq<u64> {
    remove_prefix_spec(level, line, level.len() as int)
}

// r[impl hierarchy.inclusive-invariant]
// r[impl hierarchy.exclusive-invariant]
// r[impl hierarchy.non-inclusive]
pub open spec fn append_unique_spec(level: Seq<u64>, line: u64) -> Seq<u64> {
    if contains_line_spec(level, line) {
        level
    } else {
        level.push(line)
    }
}

// r[impl hierarchy.inclusive-invariant]
// r[impl hierarchy.inclusive-invariant]
pub open spec fn inclusive_invariant_spec(l1: Seq<u64>, l2: Seq<u64>) -> bool {
    forall|i: int| 0 <= i < l1.len() ==> contains_line_spec(l2, l1[i])
}

// r[impl hierarchy.exclusive-invariant]
// r[impl hierarchy.exclusive-invariant]
pub open spec fn exclusive_invariant_spec(l1: Seq<u64>, l2: Seq<u64>) -> bool {
    forall|i: int| 0 <= i < l1.len() ==> !contains_line_spec(l2, l1[i])
}

// r[impl hierarchy.non-inclusive]
// r[impl hierarchy.non-inclusive]
pub open spec fn non_inclusive_invariant_spec(_l1: Seq<u64>, _l2: Seq<u64>) -> bool {
    true
}

// r[impl hierarchy.inclusive-invariant]
// r[impl hierarchy.exclusive-invariant]
// r[impl hierarchy.non-inclusive]
pub open spec fn hierarchy_invariant_spec(policy: HierarchyPolicy, l1: Seq<u64>, l2: Seq<u64>) -> bool {
    match policy {
        HierarchyPolicy::Inclusive => inclusive_invariant_spec(l1, l2),
        HierarchyPolicy::Exclusive => exclusive_invariant_spec(l1, l2),
        HierarchyPolicy::NonInclusive => non_inclusive_invariant_spec(l1, l2),
    }
}

// r[impl hierarchy.inclusive-invariant]
// r[impl hierarchy.exclusive-invariant]
// r[impl hierarchy.non-inclusive]
pub open spec fn l1_fill_spec(
    line: u64,
    l1: Seq<u64>,
    l2: Seq<u64>,
    policy: HierarchyPolicy,
) -> (Seq<u64>, Seq<u64>) {
    match policy {
        HierarchyPolicy::Inclusive => (append_unique_spec(l1, line), append_unique_spec(l2, line)),
        HierarchyPolicy::Exclusive => (append_unique_spec(l1, line), remove_line_spec(l2, line)),
        HierarchyPolicy::NonInclusive => (append_unique_spec(l1, line), l2),
    }
}

// r[impl hierarchy.inclusive-invariant]
// r[impl hierarchy.exclusive-invariant]
// r[impl hierarchy.exclusive-swap]
pub open spec fn l1_evict_spec(
    line: u64,
    l1: Seq<u64>,
    l2: Seq<u64>,
    policy: HierarchyPolicy,
) -> (Seq<u64>, Seq<u64>) {
    let next_l1 = remove_line_spec(l1, line);
    match policy {
        HierarchyPolicy::Inclusive => (next_l1, l2),
        HierarchyPolicy::Exclusive => {
            let next_l2 = if contains_line_spec(l1, line) { append_unique_spec(l2, line) } else { l2 };
            (next_l1, next_l2)
        }
        HierarchyPolicy::NonInclusive => (next_l1, l2),
    }
}

// r[impl hierarchy.inclusive-invariant]
// r[impl hierarchy.exclusive-invariant]
// r[impl hierarchy.non-inclusive]
pub open spec fn l2_fill_spec(
    line: u64,
    l1: Seq<u64>,
    l2: Seq<u64>,
    policy: HierarchyPolicy,
) -> (Seq<u64>, Seq<u64>) {
    match policy {
        HierarchyPolicy::Inclusive => (l1, append_unique_spec(l2, line)),
        HierarchyPolicy::Exclusive => (remove_line_spec(l1, line), append_unique_spec(l2, line)),
        HierarchyPolicy::NonInclusive => (l1, append_unique_spec(l2, line)),
    }
}

// r[impl hierarchy.back-invalidation]
// r[impl hierarchy.exclusive-invariant]
// r[impl hierarchy.non-inclusive]
pub open spec fn l2_evict_spec(
    line: u64,
    l1: Seq<u64>,
    l2: Seq<u64>,
    policy: HierarchyPolicy,
) -> (Seq<u64>, Seq<u64>) {
    let next_l2 = remove_line_spec(l2, line);
    match policy {
        HierarchyPolicy::Inclusive => (remove_line_spec(l1, line), next_l2),
        HierarchyPolicy::Exclusive => (l1, next_l2),
        HierarchyPolicy::NonInclusive => (l1, next_l2),
    }
}

// r[impl hierarchy.exclusive-swap]
// r[impl hierarchy.exclusive-invariant]
pub open spec fn l2_to_l1_promote_spec(
    line: u64,
    l1: Seq<u64>,
    l2: Seq<u64>,
    policy: HierarchyPolicy,
) -> (Seq<u64>, Seq<u64>) {
    match policy {
        HierarchyPolicy::Inclusive => (append_unique_spec(l1, line), append_unique_spec(l2, line)),
        HierarchyPolicy::Exclusive => (append_unique_spec(l1, line), remove_line_spec(l2, line)),
        HierarchyPolicy::NonInclusive => (append_unique_spec(l1, line), l2),
    }
}

// r[impl hierarchy.capacity]
pub open spec fn count_missing_from_spec(source: Seq<u64>, other: Seq<u64>, from: int) -> int
    recommends 0 <= from <= source.len()
    decreases source.len() - from
{
    if from >= source.len() {
        0int
    } else {
        (if contains_line_spec(other, source[from]) { 0int } else { 1int })
            + count_missing_from_spec(source, other, from + 1)
    }
}

// r[impl hierarchy.capacity]
// r[impl hierarchy.capacity]
pub open spec fn effective_capacity_spec(policy: HierarchyPolicy, l1: Seq<u64>, l2: Seq<u64>) -> int {
    match policy {
        HierarchyPolicy::Inclusive => l2.len() as int,
        HierarchyPolicy::Exclusive => l1.len() as int + l2.len() as int,
        HierarchyPolicy::NonInclusive => l2.len() as int + count_missing_from_spec(l1, l2, 0),
    }
}

// r[impl hierarchy.snoop-scope]
// r[impl hierarchy.snoop-scope]
pub open spec fn snoop_scope_spec(policy: HierarchyPolicy, line: u64, l1: Seq<u64>, l2: Seq<u64>) -> bool {
    match policy {
        HierarchyPolicy::Inclusive => contains_line_spec(l2, line),
        HierarchyPolicy::Exclusive => contains_line_spec(l1, line) || contains_line_spec(l2, line),
        HierarchyPolicy::NonInclusive => contains_line_spec(l1, line) || contains_line_spec(l2, line),
    }
}

proof fn append_unique_contains_line(level: Seq<u64>, line: u64)
    ensures contains_line_spec(append_unique_spec(level, line), line)
{
    if !contains_line_spec(level, line) {
        let appended = level.push(line);
        assert(appended[level.len() as int] == line);
    }
}

proof fn append_unique_preserves_other_members(level: Seq<u64>, line: u64, target: u64)
    requires target != line
    ensures contains_line_spec(append_unique_spec(level, line), target) == contains_line_spec(level, target)
{
    if !contains_line_spec(level, line) {
        let appended = level.push(line);
        if contains_line_spec(level, target) {
            let i = choose|i: int| 0 <= i < level.len() && level[i] == target;
            assert(appended[i] == target);
        } else {
            assert(!contains_line_spec(appended, target));
        }
    }
}

proof fn remove_prefix_excludes_line(level: Seq<u64>, line: u64, upto: int)
    requires 0 <= upto <= level.len()
    ensures !contains_line_spec(remove_prefix_spec(level, line, upto), line)
    decreases (if upto >= 0 { upto } else { 0 })
{
    if upto > 0 {
        remove_prefix_excludes_line(level, line, upto - 1);
    }
}

proof fn remove_prefix_preserves_other(level: Seq<u64>, line: u64, target: u64, upto: int)
    requires
        0 <= upto <= level.len(),
        target != line,
    ensures
        contains_line_spec(remove_prefix_spec(level, line, upto), target)
            == (exists|i: int| 0 <= i < upto && level[i] == target)
    decreases (if upto >= 0 { upto } else { 0 })
{
    if upto > 0 {
        remove_prefix_preserves_other(level, line, target, upto - 1);
        let prefix = remove_prefix_spec(level, line, upto - 1);
        if level[upto - 1] != line {
            if level[upto - 1] == target {
                assert(prefix.push(level[upto - 1])[prefix.len() as int] == target);
            } else if contains_line_spec(prefix, target) {
                let i = choose|i: int| 0 <= i < prefix.len() && prefix[i] == target;
                assert(prefix.push(level[upto - 1])[i] == target);
            }
        }
    }
}

proof fn remove_line_removes_target(level: Seq<u64>, line: u64)
    ensures !contains_line_spec(remove_line_spec(level, line), line)
{
    remove_prefix_excludes_line(level, line, level.len() as int);
}

proof fn remove_line_preserves_other_members(level: Seq<u64>, line: u64, target: u64)
    requires target != line
    ensures contains_line_spec(remove_line_spec(level, line), target) == contains_line_spec(level, target)
{
    remove_prefix_preserves_other(level, line, target, level.len() as int);
}

// r[verify hierarchy.inclusive-invariant]
proof fn empty_levels_satisfy_invariants()
    ensures
        inclusive_invariant_spec(seq![], seq![]),
        exclusive_invariant_spec(seq![], seq![]),
        non_inclusive_invariant_spec(seq![], seq![]),
{
}

proof fn l1_fill_preserves_invariant(policy: HierarchyPolicy, line: u64, l1: Seq<u64>, l2: Seq<u64>)
    requires hierarchy_invariant_spec(policy, l1, l2)
    ensures hierarchy_invariant_spec(policy, l1_fill_spec(line, l1, l2, policy).0, l1_fill_spec(line, l1, l2, policy).1)
{
    match policy {
        HierarchyPolicy::Inclusive => {
            assert forall|i: int| 0 <= i < append_unique_spec(l1, line).len()
                implies contains_line_spec(append_unique_spec(l2, line), append_unique_spec(l1, line)[i]) by {
                let target = append_unique_spec(l1, line)[i];
                if target == line {
                    append_unique_contains_line(l2, line);
                } else {
                    append_unique_preserves_other_members(l1, line, target);
                    append_unique_preserves_other_members(l2, line, target);
                }
            };
        }
        HierarchyPolicy::Exclusive => {
            assert forall|i: int| 0 <= i < append_unique_spec(l1, line).len()
                implies !contains_line_spec(remove_line_spec(l2, line), append_unique_spec(l1, line)[i]) by {
                let target = append_unique_spec(l1, line)[i];
                if target == line {
                    remove_line_removes_target(l2, line);
                } else {
                    append_unique_preserves_other_members(l1, line, target);
                    remove_line_preserves_other_members(l2, line, target);
                }
            };
        }
        HierarchyPolicy::NonInclusive => {}
    }
}

proof fn l1_evict_preserves_invariant(policy: HierarchyPolicy, line: u64, l1: Seq<u64>, l2: Seq<u64>)
    requires hierarchy_invariant_spec(policy, l1, l2)
    ensures hierarchy_invariant_spec(policy, l1_evict_spec(line, l1, l2, policy).0, l1_evict_spec(line, l1, l2, policy).1)
{
    match policy {
        HierarchyPolicy::Inclusive => {
            assert forall|i: int| 0 <= i < remove_line_spec(l1, line).len()
                implies contains_line_spec(l2, remove_line_spec(l1, line)[i]) by {
                let target = remove_line_spec(l1, line)[i];
                remove_line_removes_target(l1, line);
                if target != line {
                    remove_line_preserves_other_members(l1, line, target);
                }
            };
        }
        HierarchyPolicy::Exclusive => {
            if contains_line_spec(l1, line) {
                assert forall|i: int| 0 <= i < remove_line_spec(l1, line).len()
                    implies !contains_line_spec(append_unique_spec(l2, line), remove_line_spec(l1, line)[i]) by {
                    let target = remove_line_spec(l1, line)[i];
                    remove_line_removes_target(l1, line);
                    if target != line {
                        remove_line_preserves_other_members(l1, line, target);
                        append_unique_preserves_other_members(l2, line, target);
                    }
                };
            } else {
                assert forall|i: int| 0 <= i < remove_line_spec(l1, line).len()
                    implies !contains_line_spec(l2, remove_line_spec(l1, line)[i]) by {
                    let target = remove_line_spec(l1, line)[i];
                    remove_line_removes_target(l1, line);
                    if target != line {
                        remove_line_preserves_other_members(l1, line, target);
                    }
                };
            }
        }
        HierarchyPolicy::NonInclusive => {}
    }
}

proof fn l2_fill_preserves_invariant(policy: HierarchyPolicy, line: u64, l1: Seq<u64>, l2: Seq<u64>)
    requires hierarchy_invariant_spec(policy, l1, l2)
    ensures hierarchy_invariant_spec(policy, l2_fill_spec(line, l1, l2, policy).0, l2_fill_spec(line, l1, l2, policy).1)
{
    match policy {
        HierarchyPolicy::Inclusive => {
            assert forall|i: int| 0 <= i < l1.len()
                implies contains_line_spec(append_unique_spec(l2, line), l1[i]) by {
                if l1[i] == line {
                    append_unique_contains_line(l2, line);
                } else {
                    append_unique_preserves_other_members(l2, line, l1[i]);
                }
            };
        }
        HierarchyPolicy::Exclusive => {
            assert forall|i: int| 0 <= i < remove_line_spec(l1, line).len()
                implies !contains_line_spec(append_unique_spec(l2, line), remove_line_spec(l1, line)[i]) by {
                let target = remove_line_spec(l1, line)[i];
                remove_line_removes_target(l1, line);
                if target != line {
                    remove_line_preserves_other_members(l1, line, target);
                    append_unique_preserves_other_members(l2, line, target);
                }
            };
        }
        HierarchyPolicy::NonInclusive => {}
    }
}

// r[verify hierarchy.back-invalidation]
proof fn l2_evict_preserves_invariant(policy: HierarchyPolicy, line: u64, l1: Seq<u64>, l2: Seq<u64>)
    requires hierarchy_invariant_spec(policy, l1, l2)
    ensures hierarchy_invariant_spec(policy, l2_evict_spec(line, l1, l2, policy).0, l2_evict_spec(line, l1, l2, policy).1)
{
    match policy {
        HierarchyPolicy::Inclusive => {
            assert forall|i: int| 0 <= i < remove_line_spec(l1, line).len()
                implies contains_line_spec(remove_line_spec(l2, line), remove_line_spec(l1, line)[i]) by {
                let target = remove_line_spec(l1, line)[i];
                remove_line_removes_target(l1, line);
                if target != line {
                    remove_line_preserves_other_members(l1, line, target);
                    remove_line_preserves_other_members(l2, line, target);
                }
            };
        }
        HierarchyPolicy::Exclusive => {
            assert forall|i: int| 0 <= i < l1.len()
                implies !contains_line_spec(remove_line_spec(l2, line), l1[i]) by {
                let target = l1[i];
                if target == line {
                    remove_line_removes_target(l2, line);
                } else {
                    remove_line_preserves_other_members(l2, line, target);
                }
            };
        }
        HierarchyPolicy::NonInclusive => {
            assert(non_inclusive_invariant_spec(l1, remove_line_spec(l2, line)));
        }
    }
}

// r[verify hierarchy.exclusive-swap]
proof fn l2_to_l1_promote_preserves_invariant(policy: HierarchyPolicy, line: u64, l1: Seq<u64>, l2: Seq<u64>)
    requires hierarchy_invariant_spec(policy, l1, l2)
    ensures hierarchy_invariant_spec(policy, l2_to_l1_promote_spec(line, l1, l2, policy).0, l2_to_l1_promote_spec(line, l1, l2, policy).1)
{
    l1_fill_preserves_invariant(policy, line, l1, l2);
}

// r[verify hierarchy.capacity]
proof fn inclusive_capacity_equals_l2(l1: Seq<u64>, l2: Seq<u64>)
    requires inclusive_invariant_spec(l1, l2)
    ensures effective_capacity_spec(HierarchyPolicy::Inclusive, l1, l2) == l2.len()
{
}

// r[verify hierarchy.capacity]
proof fn exclusive_capacity_is_sum(l1: Seq<u64>, l2: Seq<u64>)
    requires exclusive_invariant_spec(l1, l2)
    ensures effective_capacity_spec(HierarchyPolicy::Exclusive, l1, l2) == l1.len() + l2.len()
{
}

// r[verify hierarchy.snoop-scope]
proof fn inclusive_l2_miss_implies_l1_miss(line: u64, l1: Seq<u64>, l2: Seq<u64>)
    requires
        inclusive_invariant_spec(l1, l2),
        !contains_line_spec(l2, line),
    ensures
        !contains_line_spec(l1, line),
        !snoop_scope_spec(HierarchyPolicy::Inclusive, line, l1, l2),
{
}

fn clone_level(level: &[u64]) -> (result: Vec<u64>)
    ensures result@ =~= level@,
{
    let mut result: Vec<u64> = Vec::new();
    let mut index: usize = 0;
    while index < level.len()
        invariant
            0 <= index <= level@.len(),
            result@ =~= level@.subrange(0, index as int),
        decreases level@.len() - index
    {
        result.push(level[index]);
        index = index + 1;
    }
    result
}

fn remove_line(level: &[u64], line: u64) -> (result: Vec<u64>)
    ensures result@ == remove_line_spec(level@, line),
{
    let mut result: Vec<u64> = Vec::new();
    let mut index: usize = 0;
    while index < level.len()
        invariant
            0 <= index <= level@.len(),
            result@ == remove_prefix_spec(level@, line, index as int),
        decreases level@.len() - index
    {
        if level[index] != line {
            result.push(level[index]);
            proof {
                assert(remove_prefix_spec(level@, line, index as int + 1)
                    == remove_prefix_spec(level@, line, index as int).push(level[index as int]));
            }
        } else {
            proof {
                assert(remove_prefix_spec(level@, line, index as int + 1)
                    == remove_prefix_spec(level@, line, index as int));
            }
        }
        index = index + 1;
    }
    result
}

fn append_unique(level: &[u64], line: u64) -> (result: Vec<u64>)
    ensures result@ == append_unique_spec(level@, line),
{
    if contains_line(level, line) {
        clone_level(level)
    } else {
        let mut result = clone_level(level);
        result.push(line);
        result
    }
}

fn contains_line(level: &[u64], line: u64) -> (result: bool)
    ensures result == contains_line_spec(level@, line),
{
    let mut index: usize = 0;
    while index < level.len()
        invariant
            0 <= index <= level@.len(),
            forall|j: int| 0 <= j < index as int ==> level@[j] != line,
        decreases level@.len() - index
    {
        if level[index] == line {
            proof {
                assert(contains_line_spec(level@, line));
            }
            return true;
        }
        index = index + 1;
    }
    proof {
        assert(forall|j: int| 0 <= j < level@.len() ==> level@[j] != line);
        assert(!contains_line_spec(level@, line));
    }
    false
}

// r[impl hierarchy.inclusive-invariant]
pub fn check_inclusive_invariant(l1: &[u64], l2: &[u64]) -> (result: bool)
    ensures result == inclusive_invariant_spec(l1@, l2@),
{
    let mut index: usize = 0;
    while index < l1.len()
        invariant
            0 <= index <= l1@.len(),
            forall|i: int| 0 <= i < index as int ==> contains_line_spec(l2@, l1@[i]),
        decreases l1@.len() - index
    {
        if !contains_line(l2, l1[index]) {
            proof {
                assert(!contains_line_spec(l2@, l1@[index as int]));
            }
            return false;
        }
        index = index + 1;
    }
    true
}

// r[impl hierarchy.exclusive-invariant]
pub fn check_exclusive_invariant(l1: &[u64], l2: &[u64]) -> (result: bool)
    ensures result == exclusive_invariant_spec(l1@, l2@),
{
    let mut index: usize = 0;
    while index < l1.len()
        invariant
            0 <= index <= l1@.len(),
            forall|i: int| 0 <= i < index as int ==> !contains_line_spec(l2@, l1@[i]),
        decreases l1@.len() - index
    {
        if contains_line(l2, l1[index]) {
            proof {
                assert(contains_line_spec(l2@, l1@[index as int]));
            }
            return false;
        }
        index = index + 1;
    }
    true
}

// r[impl hierarchy.inclusive-invariant]
// r[impl hierarchy.exclusive-invariant]
// r[impl hierarchy.non-inclusive]
pub fn l1_fill(
    line: u64,
    l1: &[u64],
    l2: &[u64],
    policy: HierarchyPolicy,
) -> (result: (Vec<u64>, Vec<u64>))
    requires hierarchy_invariant_spec(policy, l1@, l2@)
    ensures
        result.0@ == l1_fill_spec(line, l1@, l2@, policy).0,
        result.1@ == l1_fill_spec(line, l1@, l2@, policy).1,
        hierarchy_invariant_spec(policy, result.0@, result.1@),
{
    match policy {
        HierarchyPolicy::Inclusive => {
            let next_l1 = append_unique(l1, line);
            let next_l2 = append_unique(l2, line);
            proof {
                l1_fill_preserves_invariant(policy, line, l1@, l2@);
            }
            (next_l1, next_l2)
        }
        HierarchyPolicy::Exclusive => {
            let next_l1 = append_unique(l1, line);
            let next_l2 = remove_line(l2, line);
            proof {
                l1_fill_preserves_invariant(policy, line, l1@, l2@);
            }
            (next_l1, next_l2)
        }
        HierarchyPolicy::NonInclusive => {
            let next_l1 = append_unique(l1, line);
            let next_l2 = clone_level(l2);
            proof {
                l1_fill_preserves_invariant(policy, line, l1@, l2@);
            }
            (next_l1, next_l2)
        }
    }
}

// r[impl hierarchy.inclusive-invariant]
// r[impl hierarchy.exclusive-invariant]
// r[impl hierarchy.exclusive-swap]
pub fn l1_evict(
    line: u64,
    l1: &[u64],
    l2: &[u64],
    policy: HierarchyPolicy,
) -> (result: (Vec<u64>, Vec<u64>))
    requires hierarchy_invariant_spec(policy, l1@, l2@)
    ensures
        result.0@ == l1_evict_spec(line, l1@, l2@, policy).0,
        result.1@ == l1_evict_spec(line, l1@, l2@, policy).1,
        hierarchy_invariant_spec(policy, result.0@, result.1@),
{
    let next_l1 = remove_line(l1, line);
    match policy {
        HierarchyPolicy::Inclusive | HierarchyPolicy::NonInclusive => {
            let next_l2 = clone_level(l2);
            proof {
                l1_evict_preserves_invariant(policy, line, l1@, l2@);
            }
            (next_l1, next_l2)
        }
        HierarchyPolicy::Exclusive => {
            let next_l2 = if contains_line(l1, line) {
                append_unique(l2, line)
            } else {
                clone_level(l2)
            };
            proof {
                l1_evict_preserves_invariant(policy, line, l1@, l2@);
            }
            (next_l1, next_l2)
        }
    }
}

// r[impl hierarchy.inclusive-invariant]
// r[impl hierarchy.exclusive-invariant]
// r[impl hierarchy.non-inclusive]
pub fn l2_fill(
    line: u64,
    l1: &[u64],
    l2: &[u64],
    policy: HierarchyPolicy,
) -> (result: (Vec<u64>, Vec<u64>))
    requires hierarchy_invariant_spec(policy, l1@, l2@)
    ensures
        result.0@ == l2_fill_spec(line, l1@, l2@, policy).0,
        result.1@ == l2_fill_spec(line, l1@, l2@, policy).1,
        hierarchy_invariant_spec(policy, result.0@, result.1@),
{
    match policy {
        HierarchyPolicy::Inclusive | HierarchyPolicy::NonInclusive => {
            let next_l1 = clone_level(l1);
            let next_l2 = append_unique(l2, line);
            proof {
                l2_fill_preserves_invariant(policy, line, l1@, l2@);
            }
            (next_l1, next_l2)
        }
        HierarchyPolicy::Exclusive => {
            let next_l1 = remove_line(l1, line);
            let next_l2 = append_unique(l2, line);
            proof {
                l2_fill_preserves_invariant(policy, line, l1@, l2@);
            }
            (next_l1, next_l2)
        }
    }
}

// r[impl hierarchy.back-invalidation]
// r[impl hierarchy.exclusive-invariant]
// r[impl hierarchy.non-inclusive]
pub fn l2_evict(
    line: u64,
    l1: &[u64],
    l2: &[u64],
    policy: HierarchyPolicy,
) -> (result: (Vec<u64>, Vec<u64>))
    requires hierarchy_invariant_spec(policy, l1@, l2@)
    ensures
        result.0@ == l2_evict_spec(line, l1@, l2@, policy).0,
        result.1@ == l2_evict_spec(line, l1@, l2@, policy).1,
        hierarchy_invariant_spec(policy, result.0@, result.1@),
{
    let next_l2 = remove_line(l2, line);
    match policy {
        HierarchyPolicy::Inclusive => {
            let next_l1 = remove_line(l1, line);
            proof {
                l2_evict_preserves_invariant(policy, line, l1@, l2@);
            }
            (next_l1, next_l2)
        }
        HierarchyPolicy::Exclusive | HierarchyPolicy::NonInclusive => {
            let next_l1 = clone_level(l1);
            proof {
                l2_evict_preserves_invariant(policy, line, l1@, l2@);
            }
            (next_l1, next_l2)
        }
    }
}

// r[impl hierarchy.exclusive-swap]
// r[impl hierarchy.exclusive-invariant]
pub fn l2_to_l1_promote(
    line: u64,
    l1: &[u64],
    l2: &[u64],
    policy: HierarchyPolicy,
) -> (result: (Vec<u64>, Vec<u64>))
    requires hierarchy_invariant_spec(policy, l1@, l2@)
    ensures
        result.0@ == l2_to_l1_promote_spec(line, l1@, l2@, policy).0,
        result.1@ == l2_to_l1_promote_spec(line, l1@, l2@, policy).1,
        hierarchy_invariant_spec(policy, result.0@, result.1@),
{
    match policy {
        HierarchyPolicy::Inclusive => {
            let next_l1 = append_unique(l1, line);
            let next_l2 = append_unique(l2, line);
            proof {
                l2_to_l1_promote_preserves_invariant(policy, line, l1@, l2@);
            }
            (next_l1, next_l2)
        }
        HierarchyPolicy::Exclusive => {
            let next_l1 = append_unique(l1, line);
            let next_l2 = remove_line(l2, line);
            proof {
                l2_to_l1_promote_preserves_invariant(policy, line, l1@, l2@);
            }
            (next_l1, next_l2)
        }
        HierarchyPolicy::NonInclusive => {
            let next_l1 = append_unique(l1, line);
            let next_l2 = clone_level(l2);
            proof {
                l2_to_l1_promote_preserves_invariant(policy, line, l1@, l2@);
            }
            (next_l1, next_l2)
        }
    }
}

fn count_missing_from(level: &[u64], other: &[u64]) -> (result: usize)
    requires
        no_duplicates_spec(level@),
        no_duplicates_spec(other@),
        level@.len() <= usize::MAX as int,
    ensures result as int == count_missing_from_spec(level@, other@, 0),
{
    let mut count: usize = 0;
    let mut index: usize = 0;
    while index < level.len()
        invariant
            0 <= index <= level@.len(),
            no_duplicates_spec(level@),
            no_duplicates_spec(other@),
            level@.len() <= usize::MAX as int,
            count as int == count_missing_from_spec(level@, other@, 0) - count_missing_from_spec(level@, other@, index as int),
            0 <= count as int <= index as int,
        decreases level@.len() - index
    {
        if !contains_line(other, level[index]) {
            count = count + 1;
        }
        index = index + 1;
    }
    count
}

// r[impl hierarchy.capacity]
pub fn effective_capacity(policy: HierarchyPolicy, l1: &[u64], l2: &[u64]) -> (result: usize)
    requires
        hierarchy_invariant_spec(policy, l1@, l2@),
        no_duplicates_spec(l1@),
        no_duplicates_spec(l2@),
        l1@.len() <= usize::MAX as int,
        l2@.len() <= usize::MAX as int,
        effective_capacity_spec(policy, l1@, l2@) <= usize::MAX as int,
    ensures result as int == effective_capacity_spec(policy, l1@, l2@),
{
    match policy {
        HierarchyPolicy::Inclusive => {
            proof {
                inclusive_capacity_equals_l2(l1@, l2@);
            }
            l2.len()
        }
        HierarchyPolicy::Exclusive => {
            proof {
                exclusive_capacity_is_sum(l1@, l2@);
            }
            l1.len() + l2.len()
        }
        HierarchyPolicy::NonInclusive => l2.len() + count_missing_from(l1, l2),
    }
}

// r[impl hierarchy.snoop-scope]
pub fn snoop_scope(policy: HierarchyPolicy, line: u64, l1: &[u64], l2: &[u64]) -> (result: bool)
    requires hierarchy_invariant_spec(policy, l1@, l2@)
    ensures result == snoop_scope_spec(policy, line, l1@, l2@),
{
    match policy {
        HierarchyPolicy::Inclusive => {
            let l2_hit = contains_line(l2, line);
            if !l2_hit {
                proof {
                    inclusive_l2_miss_implies_l1_miss(line, l1@, l2@);
                }
            }
            l2_hit
        }
        HierarchyPolicy::Exclusive | HierarchyPolicy::NonInclusive => {
            contains_line(l1, line) || contains_line(l2, line)
        }
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    const TEST_LINE_A: u64 = 10u64;
    const TEST_LINE_B: u64 = 20u64;
    const TEST_LINE_C: u64 = 30u64;
    const TEST_LINE_D: u64 = 40u64;
    const TEST_TAG_VARIANTS: u8 = 5u8;
    const TEST_MAX_OPS: usize = 16usize;

    #[derive(Copy, Clone, Debug)]
    enum OpTag {
        L1Fill,
        L1Evict,
        L2Fill,
        L2Evict,
        Promote,
    }

    fn line_from_tag(tag: u8) -> u64 {
        match tag % TEST_TAG_VARIANTS {
            0 => TEST_LINE_A,
            1 => TEST_LINE_B,
            2 => TEST_LINE_C,
            3 => TEST_LINE_D,
            _ => TEST_LINE_A + TEST_LINE_B,
        }
    }

    fn op_from_tag(tag: u8) -> OpTag {
        match tag % TEST_TAG_VARIANTS {
            0 => OpTag::L1Fill,
            1 => OpTag::L1Evict,
            2 => OpTag::L2Fill,
            3 => OpTag::L2Evict,
            _ => OpTag::Promote,
        }
    }

    fn apply_op(policy: HierarchyPolicy, l1: &[u64], l2: &[u64], op_tag: u8, line_tag: u8) -> (Vec<u64>, Vec<u64>) {
        let line = line_from_tag(line_tag);
        match op_from_tag(op_tag) {
            OpTag::L1Fill => l1_fill(line, l1, l2, policy),
            OpTag::L1Evict => l1_evict(line, l1, l2, policy),
            OpTag::L2Fill => l2_fill(line, l1, l2, policy),
            OpTag::L2Evict => l2_evict(line, l1, l2, policy),
            OpTag::Promote => l2_to_l1_promote(line, l1, l2, policy),
        }
    }

    // r[verify hierarchy.inclusive-invariant]
    #[test]
    fn test_inclusive_fill_adds_line_to_both_levels() {
        let (l1, l2) = l1_fill(TEST_LINE_A, &Vec::new(), &Vec::new(), HierarchyPolicy::Inclusive);
        assert_eq!(l1, vec![TEST_LINE_A]);
        assert_eq!(l2, vec![TEST_LINE_A]);
        assert!(check_inclusive_invariant(&l1, &l2));
    }

    // r[verify hierarchy.back-invalidation]
    #[test]
    fn test_inclusive_l2_evict_back_invalidates_l1() {
        let l1 = vec![TEST_LINE_A];
        let l2 = vec![TEST_LINE_A, TEST_LINE_B];
        let (next_l1, next_l2) = l2_evict(TEST_LINE_A, &l1, &l2, HierarchyPolicy::Inclusive);
        assert_eq!(next_l1, Vec::<u64>::new());
        assert_eq!(next_l2, vec![TEST_LINE_B]);
        assert!(check_inclusive_invariant(&next_l1, &next_l2));
    }

    // r[verify hierarchy.inclusive-invariant]
    #[test]
    fn test_inclusive_without_back_invalidation_breaks_invariant() {
        let l1 = vec![TEST_LINE_A];
        let l2 = vec![TEST_LINE_A];
        let broken_l2 = Vec::<u64>::new();
        assert!(!check_inclusive_invariant(&l1, &broken_l2));
        assert!(check_inclusive_invariant(&l1, &l2));
    }

    // r[verify hierarchy.exclusive-invariant]
    // r[verify hierarchy.exclusive-swap]
    #[test]
    fn test_exclusive_promote_removes_line_from_l2() {
        let l1 = Vec::<u64>::new();
        let l2 = vec![TEST_LINE_A];
        let (next_l1, next_l2) = l2_to_l1_promote(TEST_LINE_A, &l1, &l2, HierarchyPolicy::Exclusive);
        assert_eq!(next_l1, vec![TEST_LINE_A]);
        assert_eq!(next_l2, Vec::<u64>::new());
        assert!(check_exclusive_invariant(&next_l1, &next_l2));
    }

    // r[verify hierarchy.non-inclusive]
    #[test]
    fn test_non_inclusive_allows_same_line_in_both_levels() {
        let l1 = vec![TEST_LINE_A];
        let l2 = vec![TEST_LINE_A];
        let (next_l1, next_l2) = l1_fill(TEST_LINE_A, &l1, &l2, HierarchyPolicy::NonInclusive);
        assert_eq!(next_l1, vec![TEST_LINE_A]);
        assert_eq!(next_l2, vec![TEST_LINE_A]);
    }

    // r[verify hierarchy.capacity]
    #[test]
    fn test_inclusive_capacity_equals_l2_size() {
        let l1 = vec![TEST_LINE_A];
        let l2 = vec![TEST_LINE_A, TEST_LINE_B];
        assert_eq!(effective_capacity(HierarchyPolicy::Inclusive, &l1, &l2), l2.len());
    }

    // r[verify hierarchy.capacity]
    #[test]
    fn test_exclusive_capacity_equals_sum_of_levels() {
        let l1 = vec![TEST_LINE_A];
        let l2 = vec![TEST_LINE_B, TEST_LINE_C];
        assert_eq!(effective_capacity(HierarchyPolicy::Exclusive, &l1, &l2), l1.len() + l2.len());
    }

    // r[verify hierarchy.snoop-scope]
    #[test]
    fn test_inclusive_snoop_only_needs_l2() {
        let l1 = vec![TEST_LINE_A];
        let l2 = vec![TEST_LINE_A];
        assert!(snoop_scope(HierarchyPolicy::Inclusive, TEST_LINE_A, &l1, &l2));
        assert!(!snoop_scope(HierarchyPolicy::Inclusive, TEST_LINE_B, &l1, &l2));
    }

    // r[verify hierarchy.snoop-scope]
    #[test]
    fn test_exclusive_snoop_must_check_both_levels() {
        let l1 = vec![TEST_LINE_A];
        let l2 = vec![TEST_LINE_B];
        assert!(snoop_scope(HierarchyPolicy::Exclusive, TEST_LINE_A, &l1, &l2));
        assert!(snoop_scope(HierarchyPolicy::Exclusive, TEST_LINE_B, &l1, &l2));
        assert!(!snoop_scope(HierarchyPolicy::Exclusive, TEST_LINE_C, &l1, &l2));
    }

    // r[verify hierarchy.inclusive-invariant]
    proptest! {
        #[test]
        fn proptest_inclusive_invariant_preserved(
            ops in prop::collection::vec((0u8..TEST_TAG_VARIANTS, 0u8..TEST_TAG_VARIANTS), 0..TEST_MAX_OPS)
        ) {
            let mut l1 = Vec::<u64>::new();
            let mut l2 = Vec::<u64>::new();
            for (op_tag, line_tag) in ops {
                let next = apply_op(HierarchyPolicy::Inclusive, &l1, &l2, op_tag, line_tag);
                l1 = next.0;
                l2 = next.1;
                prop_assert!(check_inclusive_invariant(&l1, &l2));
            }
        }
    }

    // r[verify hierarchy.exclusive-invariant]
    proptest! {
        #[test]
        fn proptest_exclusive_invariant_preserved(
            ops in prop::collection::vec((0u8..TEST_TAG_VARIANTS, 0u8..TEST_TAG_VARIANTS), 0..TEST_MAX_OPS)
        ) {
            let mut l1 = Vec::<u64>::new();
            let mut l2 = Vec::<u64>::new();
            for (op_tag, line_tag) in ops {
                let next = apply_op(HierarchyPolicy::Exclusive, &l1, &l2, op_tag, line_tag);
                l1 = next.0;
                l2 = next.1;
                prop_assert!(check_exclusive_invariant(&l1, &l2));
            }
        }
    }

    // r[verify hierarchy.capacity]
    proptest! {
        #[test]
        fn proptest_effective_capacity_matches_policy(
            ops in prop::collection::vec((0u8..TEST_TAG_VARIANTS, 0u8..TEST_TAG_VARIANTS), 0..TEST_MAX_OPS)
        ) {
            let mut inclusive_l1 = Vec::<u64>::new();
            let mut inclusive_l2 = Vec::<u64>::new();
            let mut exclusive_l1 = Vec::<u64>::new();
            let mut exclusive_l2 = Vec::<u64>::new();
            for (op_tag, line_tag) in ops {
                let next_inclusive = apply_op(HierarchyPolicy::Inclusive, &inclusive_l1, &inclusive_l2, op_tag, line_tag);
                inclusive_l1 = next_inclusive.0;
                inclusive_l2 = next_inclusive.1;
                prop_assert_eq!(effective_capacity(HierarchyPolicy::Inclusive, &inclusive_l1, &inclusive_l2), inclusive_l2.len());

                let next_exclusive = apply_op(HierarchyPolicy::Exclusive, &exclusive_l1, &exclusive_l2, op_tag, line_tag);
                exclusive_l1 = next_exclusive.0;
                exclusive_l2 = next_exclusive.1;
                prop_assert_eq!(
                    effective_capacity(HierarchyPolicy::Exclusive, &exclusive_l1, &exclusive_l2),
                    exclusive_l1.len() + exclusive_l2.len(),
                );
            }
        }
    }
}
