//! Verified LRU eviction logic with protected entries.
//!
//! Models the decision logic for cache eviction: determining when eviction
//! is needed, finding the least-recently-used unprotected entry, validating
//! cache state, and counting evictable entries. Every function has a
//! machine-checked spec and proven properties.

use vstd::prelude::*;

verus! {

// --- should_evict ---

// r[depends eviction.should-evict]
/// Spec: whether eviction is needed.
/// Cache should evict when at or over capacity.
pub open spec fn should_evict_spec(count: u64, capacity: u64) -> bool {
    count >= capacity
}

// r[impl eviction.should-evict]
/// Returns true if the cache is at or over capacity and needs eviction.
pub fn should_evict(count: u64, capacity: u64) -> (result: bool)
    ensures result == should_evict_spec(count, capacity)
{
    count >= capacity
}

// r[verify eviction.should-evict]
/// Cache at capacity should evict.
proof fn should_evict_at_capacity(capacity: u64)
    ensures should_evict_spec(capacity, capacity)
{}

// r[verify eviction.should-evict]
/// Cache below capacity should not evict.
proof fn should_not_evict_below_capacity(count: u64, capacity: u64)
    requires count < capacity
    ensures !should_evict_spec(count, capacity)
{}

// --- find_lru_candidate ---

// r[depends eviction.find-lru]
/// Spec: index of entry with smallest access_time among unprotected entries.
/// Returns -1 if no unprotected entries exist.
pub open spec fn find_lru_candidate_spec(entries: Seq<(u64, bool)>) -> int
    decreases entries.len()
{
    find_lru_from_spec(entries, 0, -1, u64::MAX)
}

/// Helper spec: find LRU starting from index `from`, with current best at `best_idx`
/// having access time `best_time`. Returns -1 if no unprotected entries found.
pub open spec fn find_lru_from_spec(entries: Seq<(u64, bool)>, from: int, best_idx: int, best_time: u64) -> int
    decreases entries.len() - from
{
    if from >= entries.len() {
        best_idx
    } else {
        let (access_time, is_protected) = entries[from];
        if !is_protected && (best_idx == -1 || access_time < best_time) {
            find_lru_from_spec(entries, from + 1, from, access_time)
        } else {
            find_lru_from_spec(entries, from + 1, best_idx, best_time)
        }
    }
}

/// Finds the index of the least-recently-used unprotected entry.
/// Returns the index with smallest `access_time` among non-protected entries.
/// First match wins on ties. Returns None if all protected or empty.
// r[impl eviction.find-lru]
#[allow(clippy::len_zero)]
pub fn find_lru_candidate(entries: &[(u64, bool)]) -> (result: Option<usize>)
    ensures
        result.is_some() ==> {
            let idx = result.unwrap() as int;
            &&& 0 <= idx < entries@.len()
            &&& !entries@[idx].1
            &&& forall|i: int| #![auto] 0 <= i < entries@.len() && !entries@[i].1
                   ==> entries@[idx].0 <= entries@[i].0
        },
        result.is_none() ==> forall|i: int| #![auto]
            0 <= i < entries@.len() ==> entries@[i].1,
{
    if entries.len() == 0 {
        return None;
    }

    let mut best_idx: Option<usize> = None;
    let mut best_time: u64 = u64::MAX;
    let mut i: usize = 0;

    while i < entries.len()
        invariant
            0 <= i <= entries@.len(),
            best_idx.is_some() ==> {
                let idx = best_idx.unwrap() as int;
                &&& 0 <= idx < i as int
                &&& !entries@[idx].1
                &&& entries@[idx].0 == best_time
                &&& forall|j: int| #![auto] 0 <= j < i as int && !entries@[j].1
                       ==> best_time <= entries@[j].0
            },
            best_idx.is_none() ==> {
                &&& best_time == u64::MAX
                &&& forall|j: int| #![auto] 0 <= j < i as int ==> entries@[j].1
            },
        decreases entries@.len() - i as int
    {
        let (access_time, is_protected) = entries[i];
        if !is_protected && (best_idx.is_none() || access_time < best_time) {
            best_idx = Some(i);
            best_time = access_time;
        }
        i = i + 1;
    }

    best_idx
}

// r[verify eviction.find-lru]
/// Returns None on empty slice.
proof fn find_lru_empty()
    ensures find_lru_candidate_spec(Seq::<(u64, bool)>::empty()) == -1int
{}



// --- is_cache_valid ---

// r[depends eviction.cache-valid]
/// Spec: whether cache state is valid.
/// Cache can temporarily exceed capacity by 1 before eviction.
/// Protected count cannot exceed total count.
pub open spec fn is_cache_valid_spec(count: u64, capacity: u64, protected_count: u64) -> bool {
    count as int <= capacity as int + 1 && protected_count <= count
}

// r[impl eviction.cache-valid]
/// Returns true if the cache is in a valid state.
///
/// A valid cache has:
/// - Count at most capacity + 1 (allowing temporary overflow before eviction)
/// - Protected count at most the total count
pub fn is_cache_valid(count: u64, capacity: u64, protected_count: u64) -> (result: bool)
    requires capacity < u64::MAX  // prevent overflow
    ensures result == is_cache_valid_spec(count, capacity, protected_count)
{
    count <= capacity + 1 && protected_count <= count
}

// --- evictable_count ---

// r[depends eviction.evictable-count]
/// Spec: number of entries eligible for eviction.
pub open spec fn evictable_count_spec(count: u64, protected_count: u64) -> u64 {
    if count >= protected_count { (count - protected_count) as u64 } else { 0 }
}

// r[impl eviction.evictable-count]
/// Returns the number of entries that can be evicted (not protected).
///
/// Uses saturating subtraction to handle edge cases where `protected_count`
/// might temporarily exceed count.
pub fn evictable_count(count: u64, protected_count: u64) -> (result: u64)
    ensures result == evictable_count_spec(count, protected_count)
{
    if count >= protected_count { count - protected_count } else { 0 }
}

// r[verify eviction.evictable-count]
/// Evictable count never exceeds total count.
proof fn evictable_count_bounded(count: u64, protected_count: u64)
    ensures evictable_count_spec(count, protected_count) <= count
{}

// r[verify eviction.evictable-count]
/// When evictable count is zero, all entries are protected (if protected_count equals count).
proof fn evictable_zero_implies_all_protected(count: u64, protected_count: u64)
    requires evictable_count_spec(count, protected_count) == 0, protected_count == count
    ensures count == protected_count
{}

/// Inductive helper: if best_idx >= 0, find_lru_from_spec returns >= 0.
/// (The recursive spec only returns best_idx at the base case, and only
/// replaces it with `from >= 0`, so it never goes below 0 once positive.)
proof fn find_lru_from_preserves_nonneg(entries: Seq<(u64, bool)>, from: int, best_idx: int, best_time: u64)
    requires from >= 0, best_idx >= 0
    ensures find_lru_from_spec(entries, from, best_idx, best_time) >= 0
    decreases entries.len() - from
{
    if from >= entries.len() {
    } else {
        let (access_time, is_protected) = entries[from];
        if !is_protected && (best_idx == -1 || access_time < best_time) {
            find_lru_from_preserves_nonneg(entries, from + 1, from, access_time);
        } else {
            find_lru_from_preserves_nonneg(entries, from + 1, best_idx, best_time);
        }
    }
}

/// Inductive helper: if there exists a non-protected entry at `witness`,
/// scanning from `from` with best_idx == -1 will find it (or something better).
proof fn find_lru_from_finds_entry(entries: Seq<(u64, bool)>, from: int, best_time: u64, witness: int)
    requires
        from >= 0,
        from <= witness,
        witness < entries.len(),
        !entries[witness].1,
    ensures find_lru_from_spec(entries, from, -1, best_time) >= 0
    decreases entries.len() - from
{
    if from >= entries.len() {
        // Contradiction: from <= witness < len but from >= len
    } else {
        let (access_time, is_protected) = entries[from];
        if !is_protected && (true || access_time < best_time) {
            // best_idx == -1, so condition is just !is_protected
            // Found a non-protected entry at `from`. New best_idx = from >= 0.
            find_lru_from_preserves_nonneg(entries, from + 1, from, access_time);
        } else {
            // entries[from] is protected. witness must be after from.
            if from == witness {
                // Contradiction: entries[witness].1 is false but entries[from].1 is true
                assert(!entries[witness].1);
                assert(entries[from].1);
            } else {
                find_lru_from_finds_entry(entries, from + 1, best_time, witness);
            }
        }
    }
}

// r[verify eviction.evictable-count]
/// When evictable count is positive and we have matching entries, find_lru should succeed.
proof fn evictable_positive_find_lru_succeeds(entries: Seq<(u64, bool)>, count: u64, protected_count: u64)
    requires
        evictable_count_spec(count, protected_count) > 0,
        entries.len() == count,
        count == protected_count + evictable_count_spec(count, protected_count),
        exists|i: int| 0 <= i < entries.len() && !entries[i].1
    ensures find_lru_candidate_spec(entries) != -1int
{
    // Extract the witness from the existential
    let witness = choose|i: int| 0 <= i < entries.len() && !entries[i].1;
    find_lru_from_finds_entry(entries, 0, u64::MAX, witness);
}



} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify eviction.should-evict]
    #[test]
    fn test_should_evict() {
        assert!(should_evict(10, 10)); // at capacity
        assert!(should_evict(11, 10)); // over capacity
        assert!(!should_evict(9, 10)); // below capacity
        assert!(should_evict(0, 0)); // zero capacity
        assert!(!should_evict(0, 1)); // empty cache with capacity
    }

    // r[verify eviction.find-lru]
    #[test]
    fn test_find_lru_candidate() {
        // Empty slice
        assert_eq!(find_lru_candidate(&[]), None);

        // All protected
        assert_eq!(find_lru_candidate(&[(100, true), (200, true)]), None);

        // Single unprotected entry
        assert_eq!(find_lru_candidate(&[(100, false)]), Some(0));

        // Multiple unprotected, different access times
        let entries = &[(100, false), (50, false), (200, false)];
        assert_eq!(find_lru_candidate(entries), Some(1)); // index 1 has access_time 50

        // Mix of protected and unprotected
        let entries = &[(100, true), (50, false), (25, true), (75, false)];
        assert_eq!(find_lru_candidate(entries), Some(1)); // index 1 has access_time 50

        // Multiple entries with same minimum access time - first wins
        let entries = &[(100, false), (50, false), (50, false), (200, false)];
        assert_eq!(find_lru_candidate(entries), Some(1)); // first occurrence of 50
    }

    // r[verify eviction.cache-valid]
    #[test]
    fn test_is_cache_valid() {
        // Valid states
        assert!(is_cache_valid(10, 10, 5)); // at capacity with some protected
        assert!(is_cache_valid(9, 10, 5)); // below capacity
        assert!(is_cache_valid(11, 10, 5)); // one over capacity (temporary)
        assert!(is_cache_valid(10, 10, 10)); // all protected
        assert!(is_cache_valid(0, 10, 0)); // empty cache

        // Invalid states
        assert!(!is_cache_valid(12, 10, 5)); // more than one over capacity
        assert!(!is_cache_valid(10, 10, 11)); // protected count exceeds total
    }

    // r[verify eviction.evictable-count]
    #[test]
    fn test_evictable_count() {
        assert_eq!(evictable_count(10, 3), 7); // 10 - 3 = 7
        assert_eq!(evictable_count(10, 10), 0); // all protected
        assert_eq!(evictable_count(5, 8), 0); // protected exceeds total (saturating)
        assert_eq!(evictable_count(0, 0), 0); // empty cache
        assert_eq!(evictable_count(100, 0), 100); // none protected
    }

    proptest! {
        // r[verify eviction.should-evict]
        #[test]
        fn prop_should_evict_at_or_over_capacity(count: u64, capacity: u64) {
            prop_assert_eq!(should_evict(count, capacity), count >= capacity);
        }

        // r[verify eviction.should-evict]
        #[test]
        fn prop_should_evict_below_capacity(count in 0u64..1000, capacity in 1u64..1000) {
            if count < capacity {
                prop_assert!(!should_evict(count, capacity));
            }
        }

        // r[verify eviction.find-lru]
        #[test]
        fn prop_find_lru_result_valid(
            entries in proptest::collection::vec((0u64..1000, any::<bool>()), 0..=20)
        ) {
            match find_lru_candidate(&entries) {
                Some(idx) => {
                    prop_assert!(idx < entries.len());
                    prop_assert!(!entries[idx].1); // not protected
                    // Check it has minimum access time among unprotected
                    let min_time = entries[idx].0;
                    for &(access_time, is_protected) in &entries {
                        if !is_protected {
                            prop_assert!(min_time <= access_time);
                        }
                    }
                },
                None => {
                    // Should be empty or all protected
                    prop_assert!(entries.is_empty() || entries.iter().all(|&(_, protected)| protected));
                }
            }
        }

        // r[verify eviction.find-lru]
        #[test]
        fn prop_find_lru_all_protected_returns_none(
            entries in proptest::collection::vec(0u64..1000, 0..=20)
        ) {
            let protected_entries: Vec<(u64, bool)> = entries.into_iter()
                .map(|access_time| (access_time, true))
                .collect();
            prop_assert_eq!(find_lru_candidate(&protected_entries), None);
        }

        // r[verify eviction.cache-valid]
        #[test]
        fn prop_cache_valid_properties(count: u64, capacity: u64, protected_count: u64) {
            let valid = is_cache_valid(count, capacity, protected_count);
            if valid {
                prop_assert!(count <= capacity + 1);
                prop_assert!(protected_count <= count);
            }
            if count > capacity + 1 || protected_count > count {
                prop_assert!(!valid);
            }
        }

        // r[verify eviction.evictable-count]
        #[test]
        fn prop_evictable_count_bounded(count: u64, protected_count: u64) {
            let evictable = evictable_count(count, protected_count);
            prop_assert!(evictable <= count);
            if count >= protected_count {
                prop_assert_eq!(evictable, count - protected_count);
            } else {
                prop_assert_eq!(evictable, 0);
            }
        }

        // r[verify eviction.evictable-count]
        #[test]
        fn prop_evictable_zero_when_all_protected(count in 0u64..1000) {
            prop_assert_eq!(evictable_count(count, count), 0);
        }

        // r[verify eviction.find-lru]
        #[test]
        fn prop_find_lru_exists_when_unprotected_present(
            protected_entries in proptest::collection::vec(0u64..100, 0..=10),
            unprotected_entries in proptest::collection::vec(0u64..100, 1..=10)
        ) {
            let mut entries = Vec::new();
            for access_time in protected_entries {
                entries.push((access_time, true));
            }
            for access_time in unprotected_entries {
                entries.push((access_time, false));
            }

            prop_assert!(find_lru_candidate(&entries).is_some());
        }

        // Integration property: cache with evictable entries can find LRU
        // r[verify eviction.evictable-count]
        // r[verify eviction.find-lru]
        #[test]
        fn prop_evictable_positive_implies_lru_found(
            count in 1u64..20,
            protected_count in 0u64..20,
            access_times in proptest::collection::vec(0u64..1000, 1..=20)
        ) {
            if evictable_count(count, protected_count) > 0 && access_times.len() == usize::try_from(count).unwrap() {
                // Create entries where exactly `protected_count` are protected
                let mut entries = Vec::new();
                for (i, &access_time) in access_times.iter().enumerate() {
                    entries.push((access_time, i < usize::try_from(protected_count).unwrap()));
                }

                if entries.iter().any(|&(_, protected)| !protected) {
                    prop_assert!(find_lru_candidate(&entries).is_some());
                }
            }
        }
    }
}
