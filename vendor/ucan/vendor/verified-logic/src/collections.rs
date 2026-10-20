//! Verified collection operations.
//!
//! `binary_search` and `sorted_insert` are hand-implemented with verified
//! loop invariants. Verus checks that:
//! - `binary_search` returns a valid index when found, and proves absence when not
//! - `sorted_insert` produces a sorted result of length n+1 containing the value

use vstd::prelude::*;

verus! {

/// Spec: a sequence is sorted in ascending order.
pub open spec fn is_sorted_spec(s: Seq<u64>) -> bool {
    forall|i: int, j: int| 0 <= i < j < s.len() ==> s[i] <= s[j]
}

// --- binary_search ---

// r[impl collections.binary-search]
/// Binary search on a sorted slice.
///
/// Returns `Some(index)` where `slice[index] == target`, or `None` if
/// absent. The loop invariant tracks that elements below `lo` and at/above
/// `hi` cannot be the target, narrowing the window each iteration.
pub fn binary_search(slice: &[u64], target: u64) -> (result: Option<usize>)
    requires is_sorted_spec(slice@)
    ensures
        result.is_some() ==> {
            let i = result.unwrap() as int;
            &&& 0 <= i < slice@.len()
            &&& slice@[i] == target
        },
        result.is_none() ==> forall|i: int| 0 <= i < slice@.len() ==> slice@[i] != target,
{
    let mut lo: usize = 0;
    let mut hi: usize = slice.len();

    while lo < hi
        invariant
            0 <= lo <= hi <= slice@.len(),
            is_sorted_spec(slice@),
            forall|i: int| 0 <= i < lo as int ==> slice@[i] != target,
            forall|i: int| hi as int <= i < slice@.len() ==> slice@[i] != target,
        decreases hi - lo,
    {
        let mid = lo + (hi - lo) / 2;

        if slice[mid] == target {
            return Some(mid);
        } else if slice[mid] < target {
            assert forall|i: int| 0 <= i <= mid as int implies slice@[i] != target
            by {
                if i >= lo as int {
                    assert(slice@[i] <= slice@[mid as int]);
                }
            }
            lo = mid + 1;
        } else {
            assert forall|i: int| mid as int <= i < slice@.len() implies slice@[i] != target
            by {
                if i < hi as int {
                    assert(slice@[mid as int] <= slice@[i]);
                }
            }
            hi = mid;
        }
    }

    None
}

// --- sorted_insert ---

/// Finds the insertion point for `value` in a sorted slice.
/// All elements before the returned index are < value;
/// all elements at or after are >= value.
fn find_insert_pos(slice: &[u64], value: u64) -> (pos: usize)
    requires is_sorted_spec(slice@)
    ensures
        0 <= pos <= slice@.len(),
        forall|i: int| 0 <= i < pos as int ==> slice@[i] < value,
        forall|i: int| pos as int <= i < slice@.len() ==> slice@[i] >= value,
{
    let mut lo: usize = 0;
    let mut hi: usize = slice.len();

    while lo < hi
        invariant
            0 <= lo <= hi <= slice@.len(),
            is_sorted_spec(slice@),
            forall|i: int| 0 <= i < lo as int ==> slice@[i] < value,
            forall|i: int| hi as int <= i < slice@.len() ==> slice@[i] >= value,
        decreases hi - lo,
    {
        let mid = lo + (hi - lo) / 2;

        if slice[mid] < value {
            assert forall|i: int| 0 <= i <= mid as int implies slice@[i] < value
            by {
                if i >= lo as int {
                    assert(slice@[i] <= slice@[mid as int]);
                }
            }
            lo = mid + 1;
        } else {
            assert forall|i: int| mid as int <= i < slice@.len() implies slice@[i] >= value
            by {
                if i < hi as int {
                    assert(slice@[mid as int] <= slice@[i]);
                }
            }
            hi = mid;
        }
    }

    lo
}

// r[impl collections.sorted-insert]
/// Inserts `value` into a sorted slice, returning a new sorted `Vec`.
///
/// Uses a verified insertion point search, then builds the result by
/// copying elements before the position, inserting the value, and
/// copying elements after. The postcondition proves:
/// - result length is input length + 1
/// - result is sorted
/// - result contains the inserted value
pub fn sorted_insert(slice: &[u64], value: u64) -> (result: Vec<u64>)
    requires
        is_sorted_spec(slice@),
        slice@.len() < usize::MAX,
    ensures
        result@.len() == slice@.len() + 1,
        is_sorted_spec(result@),
        exists|i: int| 0 <= i < result@.len() && result@[i] == value,
{
    let pos = find_insert_pos(slice, value);
    let mut result = Vec::with_capacity(slice.len() + 1);

    let mut i: usize = 0;
    while i < pos
        invariant
            0 <= i <= pos,
            pos <= slice@.len(),
            result@.len() == i as int,
            forall|j: int| 0 <= j < i as int ==> result@[j] == slice@[j],
        decreases pos - i,
    {
        result.push(slice[i]);
        i = i + 1;
    }

    result.push(value);

    let mut i: usize = pos;
    while i < slice.len()
        invariant
            pos <= i <= slice@.len(),
            pos <= slice@.len(),
            result@.len() == i as int + 1,
            forall|j: int| 0 <= j < pos as int ==> result@[j] == slice@[j],
            result@[pos as int] == value,
            forall|j: int| pos as int <= j < i as int ==> result@[j + 1] == slice@[j],
        decreases slice@.len() - i as int,
    {
        result.push(slice[i]);
        i = i + 1;
    }

    // Prove the result is sorted by case analysis on position relative to
    // the insertion point.
    assert forall|i: int, j: int| 0 <= i < j < result@.len()
        implies result@[i] <= result@[j]
    by {
        let p = pos as int;
        if i < p && j < p {
            assert(result@[i] == slice@[i]);
            assert(result@[j] == slice@[j]);
        } else if i < p && j == p {
            assert(result@[i] == slice@[i]);
            assert(slice@[i] < value);
        } else if i < p && j > p {
            assert(result@[i] == slice@[i]);
            assert(result@[j] == slice@[j - 1]);
        } else if i == p && j > p {
            assert(result@[j] == slice@[j - 1]);
            assert(slice@[j - 1] >= value);
        } else if i > p && j > p {
            assert(result@[i] == slice@[i - 1]);
            assert(result@[j] == slice@[j - 1]);
        }
    }

    assert(result@[pos as int] == value);

    result
}

// --- merge_sorted ---

// r[impl collections.merge-sorted]
/// Merges two sorted slices into a single sorted `Vec`.
///
/// Two-pointer merge: pick the smaller head, advance that pointer.
/// The invariant tracks that:
/// - result is sorted
/// - every element in result <= the next element from both a and b
pub fn merge_sorted(a: &[u64], b: &[u64]) -> (result: Vec<u64>)
    requires
        is_sorted_spec(a@),
        is_sorted_spec(b@),
        a@.len() + b@.len() < usize::MAX,
    ensures
        result@.len() == a@.len() + b@.len(),
        is_sorted_spec(result@),
{
    let mut result = Vec::with_capacity(a.len() + b.len());
    let mut i: usize = 0;
    let mut j: usize = 0;

    while i < a.len() || j < b.len()
        invariant
            0 <= i <= a@.len(),
            0 <= j <= b@.len(),
            result@.len() == i as int + j as int,
            is_sorted_spec(a@),
            is_sorted_spec(b@),
            is_sorted_spec(result@),
            forall|k: int| 0 <= k < result@.len() ==> (
                (i < a@.len() ==> result@[k] <= a@[i as int]) &&
                (j < b@.len() ==> result@[k] <= b@[j as int])
            ),
            a@.len() + b@.len() < usize::MAX,
        decreases a@.len() - i as int + b@.len() - j as int,
    {
        if i < a.len() && (j >= b.len() || a[i] <= b[j]) {
            result.push(a[i]);
            i = i + 1;
        } else {
            result.push(b[j]);
            j = j + 1;
        }
    }

    result
}

// --- is_partitioned ---

// r[depends collections.is-partitioned]
/// Spec: once an element >= threshold appears, all subsequent are >= threshold.
pub open spec fn is_partitioned_spec(s: Seq<u64>, threshold: u64) -> bool {
    forall|i: int, j: int|
        0 <= i < j < s.len() && s[i] >= threshold ==> s[j] >= threshold
}

// r[impl collections.is-partitioned]
/// Returns true if all elements < threshold precede all elements >= threshold.
pub fn is_partitioned(slice: &[u64], threshold: u64) -> (result: bool)
    ensures result == is_partitioned_spec(slice@, threshold)
{
    let mut i: usize = 0;
    // Skip elements < threshold
    while i < slice.len() && slice[i] < threshold
        invariant
            0 <= i <= slice@.len(),
            forall|k: int| 0 <= k < i as int ==> slice@[k] < threshold,
        decreases slice@.len() - i
    {
        i = i + 1;
    }
    // Partition point found at i.
    // First loop exited because i == len OR slice[i] >= threshold.
    #[allow(unused_variables)]
    let partition = i;
    // Check remaining are all >= threshold.
    while i < slice.len()
        invariant
            partition <= i <= slice@.len(),
            partition < slice@.len() ==> slice@[partition as int] >= threshold,
            forall|k: int| 0 <= k < partition as int ==> slice@[k] < threshold,
            forall|k: int| partition as int <= k < i as int ==> slice@[k] >= threshold,
        decreases slice@.len() - i
    {
        if slice[i] < threshold {
            // Violation found
            return false;
        }
        i = i + 1;
    }
    // All elements partitioned: [0, partition) < threshold, [partition, len) >= threshold
    assert forall|i: int, j: int|
        0 <= i < j < slice@.len() && slice@[i] >= threshold
        implies slice@[j] >= threshold
    by {
        assert(i >= partition as int);
        assert(j >= partition as int);
    }
    true
}

// r[verify collections.is-partitioned]
/// Sorted sequences are partitioned by any threshold.
proof fn sorted_implies_partitioned(s: Seq<u64>, threshold: u64)
    requires is_sorted_spec(s)
    ensures is_partitioned_spec(s, threshold)
{
    assert forall|i: int, j: int|
        0 <= i < j < s.len() && s[i] >= threshold
        implies s[j] >= threshold
    by {
        assert(s[i] <= s[j]);
    }
}

// r[verify collections.is-partitioned]
proof fn empty_is_partitioned(threshold: u64)
    ensures is_partitioned_spec(Seq::<u64>::empty(), threshold)
{}

// --- find_first ---

// r[depends collections.find-first]
/// Spec: index of the first occurrence of target, or -1 if absent.
pub open spec fn find_first_spec(s: Seq<u64>, target: u64) -> int
    decreases s.len()
{
    if s.len() == 0 {
        -1int
    } else if s[0] == target {
        0int
    } else {
        let sub = find_first_spec(s.subrange(1, s.len() as int), target);
        if sub < 0 { -1int } else { 1 + sub }
    }
}

// r[impl collections.find-first]
/// Returns the index of the first occurrence of `target`, or `None`.
///
/// Linear scan with loop invariant: no match seen in [0, i).
pub fn find_first(slice: &[u64], target: u64) -> (result: Option<usize>)
    ensures
        result.is_some() ==> {
            let idx = result.unwrap() as int;
            &&& 0 <= idx < slice@.len()
            &&& slice@[idx] == target
            &&& forall|k: int| 0 <= k < idx ==> slice@[k] != target
        },
        result.is_none() ==> forall|k: int| 0 <= k < slice@.len() ==> slice@[k] != target,
{
    let mut i: usize = 0;
    while i < slice.len()
        invariant
            0 <= i <= slice@.len(),
            forall|k: int| 0 <= k < i as int ==> slice@[k] != target,
        decreases slice@.len() - i
    {
        if slice[i] == target {
            return Some(i);
        }
        i = i + 1;
    }
    None
}

// r[verify collections.find-first]
proof fn find_first_empty(target: u64)
    ensures find_first_spec(Seq::<u64>::empty(), target) == -1int
{}

// --- count_matches ---

// r[depends collections.count-matches]
/// Spec: number of elements equal to target, counting from index `from`.
pub open spec fn count_from_spec(s: Seq<u64>, target: u64, from: int) -> nat
    decreases s.len() - from
{
    if from < 0 || from >= s.len() {
        0
    } else if s[from] == target {
        1 + count_from_spec(s, target, from + 1)
    } else {
        count_from_spec(s, target, from + 1)
    }
}

/// Spec: total count of elements equal to target.
pub open spec fn count_matches_spec(s: Seq<u64>, target: u64) -> nat {
    count_from_spec(s, target, 0)
}

// r[impl collections.count-matches]
/// Counts the number of elements equal to `target`.
///
/// Linear scan with a running count. The invariant relates the partial
/// count to the spec via: count + remaining == total.
pub fn count_matches(slice: &[u64], target: u64) -> (result: usize)
    requires slice@.len() < usize::MAX
    ensures result as nat == count_matches_spec(slice@, target)
{
    let mut count: usize = 0;
    let mut i: usize = 0;
    while i < slice.len()
        invariant
            0 <= i <= slice@.len(),
            count as nat + count_from_spec(slice@, target, i as int)
                == count_matches_spec(slice@, target),
            count <= i,
        decreases slice@.len() - i
    {
        if slice[i] == target {
            count = count + 1;
        }
        i = i + 1;
    }
    count
}

// r[verify collections.count-matches]
proof fn count_matches_empty(target: u64)
    ensures count_matches_spec(Seq::<u64>::empty(), target) == 0nat
{}

// r[verify collections.count-matches]
/// Count never exceeds length.
proof fn count_matches_bounded(s: Seq<u64>, target: u64)
    ensures count_matches_spec(s, target) <= s.len()
{
    count_from_bounded(s, target, 0);
}

/// Helper: count_from(s, target, from) <= s.len() - from.
proof fn count_from_bounded(s: Seq<u64>, target: u64, from: int)
    requires 0 <= from <= s.len()
    ensures count_from_spec(s, target, from) as int <= s.len() - from
    decreases s.len() - from
{
    if from < s.len() {
        count_from_bounded(s, target, from + 1);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn is_sorted(s: &[u64]) -> bool {
        s.windows(2).all(|w| w[0] <= w[1])
    }

    fn sorted_vec(max_len: usize) -> impl Strategy<Value = Vec<u64>> {
        proptest::collection::vec(any::<u64>(), 0..=max_len).prop_map(|mut v| {
            v.sort_unstable();
            v
        })
    }

    // r[verify collections.sorted-insert]
    #[test]
    fn test_sorted_insert() {
        let s = vec![1, 3, 5, 7];
        assert_eq!(sorted_insert(&s, 4), vec![1, 3, 4, 5, 7]);
        assert_eq!(sorted_insert(&s, 0), vec![0, 1, 3, 5, 7]);
        assert_eq!(sorted_insert(&s, 10), vec![1, 3, 5, 7, 10]);
        assert_eq!(sorted_insert(&s, 3), vec![1, 3, 3, 5, 7]);
        assert_eq!(sorted_insert(&[], 5), vec![5]);
    }

    // r[verify collections.binary-search]
    #[test]
    fn test_binary_search() {
        let s = vec![1, 3, 5, 7, 9];
        assert_eq!(binary_search(&s, 5), Some(2));
        assert_eq!(binary_search(&s, 4), None);
        assert_eq!(binary_search(&[], 5), None);
    }

    proptest! {
        // r[verify collections.sorted-insert]
        #[test]
        fn prop_sorted_insert_preserves_sorted(s in sorted_vec(50), value: u64) {
            let result = sorted_insert(&s, value);
            prop_assert!(is_sorted(&result));
        }

        // r[verify collections.sorted-insert]
        #[test]
        fn prop_sorted_insert_length(s in sorted_vec(50), value: u64) {
            prop_assert_eq!(sorted_insert(&s, value).len(), s.len() + 1);
        }

        // r[verify collections.sorted-insert]
        #[test]
        fn prop_sorted_insert_contains_value(s in sorted_vec(50), value: u64) {
            prop_assert!(sorted_insert(&s, value).contains(&value));
        }

        // r[verify collections.binary-search]
        #[test]
        fn prop_binary_search_found_valid(s in sorted_vec(50), target: u64) {
            if let Some(idx) = binary_search(&s, target) {
                prop_assert!(idx < s.len());
                prop_assert_eq!(s[idx], target);
            }
        }

        // r[verify collections.binary-search]
        #[test]
        fn prop_binary_search_none_means_absent(s in sorted_vec(50), target: u64) {
            if binary_search(&s, target).is_none() {
                prop_assert!(!s.contains(&target));
            }
        }

        // r[verify collections.merge-sorted]
        #[test]
        fn prop_merge_sorted_length(
            a in sorted_vec(30),
            b in sorted_vec(30),
        ) {
            let result = merge_sorted(&a, &b);
            prop_assert_eq!(result.len(), a.len() + b.len());
        }

        // r[verify collections.merge-sorted]
        #[test]
        fn prop_merge_sorted_is_sorted(
            a in sorted_vec(30),
            b in sorted_vec(30),
        ) {
            let result = merge_sorted(&a, &b);
            prop_assert!(is_sorted(&result));
        }

        // r[verify collections.merge-sorted]
        #[test]
        fn prop_merge_sorted_contains_all(
            a in sorted_vec(30),
            b in sorted_vec(30),
        ) {
            let result = merge_sorted(&a, &b);
            // Every element from a and b appears in result
            for &x in &a {
                prop_assert!(result.contains(&x));
            }
            for &x in &b {
                prop_assert!(result.contains(&x));
            }
        }
    }

    // r[verify collections.is-partitioned]
    #[test]
    fn test_is_partitioned() {
        assert!(is_partitioned(&[], 5));
        assert!(is_partitioned(&[1, 2, 3], 5));
        assert!(is_partitioned(&[5, 6, 7], 5));
        assert!(is_partitioned(&[1, 2, 5, 6], 5));
        assert!(!is_partitioned(&[5, 1, 6], 5));
        assert!(!is_partitioned(&[1, 6, 2, 7], 5));
        assert!(is_partitioned(&[1, 3, 5, 7, 9], 1)); // all >= 1
        assert!(is_partitioned(&[1, 3, 5, 7, 9], 100)); // all < 100
    }

    // r[verify collections.find-first]
    #[test]
    fn test_find_first() {
        assert_eq!(find_first(&[], 5), None);
        assert_eq!(find_first(&[1, 2, 3], 2), Some(1));
        assert_eq!(find_first(&[1, 2, 3], 4), None);
        assert_eq!(find_first(&[5, 5, 5], 5), Some(0));
        assert_eq!(find_first(&[1, 2, 5, 3, 5], 5), Some(2));
    }

    // r[verify collections.count-matches]
    #[test]
    fn test_count_matches() {
        assert_eq!(count_matches(&[], 5), 0);
        assert_eq!(count_matches(&[1, 2, 3], 2), 1);
        assert_eq!(count_matches(&[5, 5, 5], 5), 3);
        assert_eq!(count_matches(&[1, 2, 3], 4), 0);
        assert_eq!(count_matches(&[1, 5, 2, 5, 3, 5], 5), 3);
    }

    proptest! {
        // r[verify collections.is-partitioned]
        #[test]
        fn prop_sorted_is_partitioned(s in sorted_vec(50), threshold: u64) {
            prop_assert!(is_partitioned(&s, threshold));
        }

        // r[verify collections.find-first]
        #[test]
        fn prop_find_first_is_first(s in proptest::collection::vec(0u64..10, 0..=50), target in 0u64..10) {
            if let Some(idx) = find_first(&s, target) {
                prop_assert_eq!(s[idx], target);
                for &val in s.iter().take(idx) {
                    prop_assert_ne!(val, target);
                }
            } else {
                prop_assert!(!s.contains(&target));
            }
        }

        // r[verify collections.count-matches]
        #[test]
        fn prop_count_matches_correct(s in proptest::collection::vec(0u64..10, 0..=50), target in 0u64..10) {
            let expected = s.iter().filter(|&&x| x == target).count();
            prop_assert_eq!(count_matches(&s, target), expected);
        }

        // r[verify collections.count-matches]
        #[test]
        fn prop_count_matches_bounded(s in proptest::collection::vec(any::<u64>(), 0..=50), target: u64) {
            prop_assert!(count_matches(&s, target) <= s.len());
        }
    }

    // r[verify collections.merge-sorted]
    #[test]
    fn test_merge_sorted() {
        assert_eq!(merge_sorted(&[1, 3, 5], &[2, 4, 6]), vec![1, 2, 3, 4, 5, 6]);
        assert_eq!(merge_sorted(&[1, 2, 3], &[]), vec![1, 2, 3]);
        assert_eq!(merge_sorted(&[], &[4, 5, 6]), vec![4, 5, 6]);
        assert_eq!(merge_sorted(&[1, 1], &[1, 1]), vec![1, 1, 1, 1]);
        let empty: &[u64] = &[];
        assert_eq!(merge_sorted(empty, empty), Vec::<u64>::new());
    }
}
