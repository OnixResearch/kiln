//! Verified way-based cache partitioning.
//!
//! Models partition assignment of cache ways and proves isolation,
//! eviction containment, capacity accounting, resize correctness,
//! and single-partition equivalence to an unpartitioned cache.

use vstd::prelude::*;

verus! {

// ── Assignment validity ──────────────────────────────────────────────

// r[depends cache-partition.assignment]
/// Every way maps to a partition in `[0, num_partitions)`.
pub open spec fn valid_assignment_spec(map: Seq<u32>, num_partitions: u32) -> bool {
    forall|w: int| #![trigger map[w]] 0 <= w < map.len() ==> 0 <= map[w] < num_partitions
}

// r[impl cache-partition.assignment]
pub fn valid_assignment(map: &[u32], num_partitions: u32) -> (result: bool)
    ensures result == valid_assignment_spec(map@, num_partitions)
{
    let mut i: usize = 0;
    while i < map.len()
        invariant
            0 <= i <= map@.len(),
            forall|w: int| #![trigger map@[w]] 0 <= w < i as int ==> 0 <= map@[w] < num_partitions,
        decreases map@.len() - i as int
    {
        if map[i] >= num_partitions {
            assert(!(0 <= map@[i as int] < num_partitions));
            return false;
        }
        i = i + 1;
    }
    true
}

// ── Partition size ───────────────────────────────────────────────────

// r[depends cache-partition.capacity]
/// Count of ways assigned to `partition` starting from index `from`.
pub open spec fn partition_size_from_spec(partition: u32, map: Seq<u32>, from: int) -> int
    decreases map.len() - from
{
    if from >= map.len() || from < 0 {
        0
    } else if map[from] == partition {
        1 + partition_size_from_spec(partition, map, from + 1)
    } else {
        partition_size_from_spec(partition, map, from + 1)
    }
}

// r[impl cache-partition.capacity]
/// Total ways assigned to `partition`.
pub open spec fn partition_size_spec(partition: u32, map: Seq<u32>) -> int {
    partition_size_from_spec(partition, map, 0)
}

// r[impl cache-partition.capacity]
pub fn partition_size(partition: u32, map: &[u32]) -> (result: usize)
    requires map@.len() <= usize::MAX as int
    ensures result as int == partition_size_spec(partition, map@)
{
    let mut count: usize = 0;
    let mut i: usize = 0;
    proof {
        partition_size_from_nonneg(partition, map@, 0);
        partition_size_from_bounded(partition, map@, 0);
    }
    while i < map.len()
        invariant
            0 <= i <= map@.len(),
            map@.len() <= usize::MAX as int,
            count as int == partition_size_spec(partition, map@) - partition_size_from_spec(partition, map@, i as int),
            count as int >= 0,
            count as int <= i as int,
            partition_size_from_spec(partition, map@, i as int) >= 0,
        decreases map@.len() - i as int
    {
        proof {
            partition_size_from_nonneg(partition, map@, i as int + 1);
            partition_size_from_bounded(partition, map@, i as int + 1);
            // Unfold: partition_size_from_spec at i steps to i+1
            if map@[i as int] == partition {
                assert(partition_size_from_spec(partition, map@, i as int)
                    == 1 + partition_size_from_spec(partition, map@, i as int + 1));
            } else {
                assert(partition_size_from_spec(partition, map@, i as int)
                    == partition_size_from_spec(partition, map@, i as int + 1));
            }
        }
        if map[i] == partition {
            count = count + 1;
        }
        i = i + 1;
    }
    count
}

proof fn partition_size_from_nonneg(partition: u32, map: Seq<u32>, from: int)
    ensures partition_size_from_spec(partition, map, from) >= 0
    decreases map.len() - from
{
    if from >= map.len() || from < 0 {
    } else {
        partition_size_from_nonneg(partition, map, from + 1);
    }
}

proof fn partition_size_from_bounded(partition: u32, map: Seq<u32>, from: int)
    requires 0 <= from <= map.len()
    ensures
        partition_size_from_spec(partition, map, from) >= 0,
        partition_size_from_spec(partition, map, from) <= map.len() - from,
    decreases map.len() - from
{
    if from >= map.len() {
    } else {
        partition_size_from_bounded(partition, map, from + 1);
    }
}

// ── Partitioned lookup ───────────────────────────────────────────────

// r[depends cache-partition.isolation]
/// First matching way for `tag` within `partition`, starting from `from`.
pub open spec fn lookup_partitioned_from_spec(
    set: Seq<Option<u64>>,
    tag: u64,
    partition: u32,
    map: Seq<u32>,
    from: int,
) -> Option<int>
    recommends set.len() == map.len()
    decreases set.len() - from
{
    if from >= set.len() || from < 0 {
        None
    } else if map[from] == partition && set[from] == Some(tag) {
        Some(from)
    } else {
        lookup_partitioned_from_spec(set, tag, partition, map, from + 1)
    }
}

// r[impl cache-partition.isolation]
/// Partitioned lookup: first matching way in the requesting partition.
pub open spec fn lookup_partitioned_spec(
    set: Seq<Option<u64>>,
    tag: u64,
    partition: u32,
    map: Seq<u32>,
) -> Option<int> {
    lookup_partitioned_from_spec(set, tag, partition, map, 0)
}

// r[impl cache-partition.isolation]
pub fn lookup_partitioned(
    set: &[Option<u64>],
    tag: u64,
    partition: u32,
    map: &[u32],
) -> (result: Option<usize>)
    requires set@.len() == map@.len()
    ensures
        match result {
            Some(idx) => {
                let i = idx as int;
                &&& lookup_partitioned_spec(set@, tag, partition, map@) == Some(i)
                &&& 0 <= i < set@.len()
                &&& map@[i] == partition
                &&& set@[i] == Some(tag)
            }
            None => lookup_partitioned_spec(set@, tag, partition, map@).is_none()
        }
{
    let mut i: usize = 0;
    while i < set.len()
        invariant
            0 <= i <= set@.len(),
            set@.len() == map@.len(),
            lookup_partitioned_spec(set@, tag, partition, map@)
                == lookup_partitioned_from_spec(set@, tag, partition, map@, i as int),
        decreases set@.len() - i as int
    {
        if map[i] == partition && set[i] == Some(tag) {
            return Some(i);
        }
        proof {
            // Unfold: current position is not a match, so spec advances
            assert(lookup_partitioned_from_spec(set@, tag, partition, map@, i as int)
                == lookup_partitioned_from_spec(set@, tag, partition, map@, i as int + 1));
        }
        i = i + 1;
    }
    None
}

// ── Partitioned eviction ─────────────────────────────────────────────

// r[depends cache-partition.eviction-containment]
/// First occupied way in `partition` starting from `from`.
pub open spec fn evict_partitioned_from_spec(
    set: Seq<Option<u64>>,
    partition: u32,
    map: Seq<u32>,
    from: int,
) -> Option<int>
    recommends set.len() == map.len()
    decreases set.len() - from
{
    if from >= set.len() || from < 0 {
        None
    } else if map[from] == partition && set[from].is_some() {
        Some(from)
    } else {
        evict_partitioned_from_spec(set, partition, map, from + 1)
    }
}

// r[impl cache-partition.eviction-containment]
/// Partitioned eviction: first occupied way in the requesting partition.
pub open spec fn evict_partitioned_spec(
    set: Seq<Option<u64>>,
    partition: u32,
    map: Seq<u32>,
) -> Option<int> {
    evict_partitioned_from_spec(set, partition, map, 0)
}

// r[impl cache-partition.eviction-containment]
pub fn evict_partitioned(
    set: &[Option<u64>],
    partition: u32,
    map: &[u32],
) -> (result: Option<usize>)
    requires set@.len() == map@.len()
    ensures
        match result {
            Some(idx) => {
                let i = idx as int;
                &&& evict_partitioned_spec(set@, partition, map@) == Some(i)
                &&& 0 <= i < set@.len()
                &&& map@[i] == partition
                &&& set@[i].is_some()
            }
            None => evict_partitioned_spec(set@, partition, map@).is_none()
        }
{
    let mut i: usize = 0;
    while i < set.len()
        invariant
            0 <= i <= set@.len(),
            set@.len() == map@.len(),
            evict_partitioned_spec(set@, partition, map@)
                == evict_partitioned_from_spec(set@, partition, map@, i as int),
        decreases set@.len() - i as int
    {
        if map[i] == partition && set[i].is_some() {
            return Some(i);
        }
        proof {
            assert(evict_partitioned_from_spec(set@, partition, map@, i as int)
                == evict_partitioned_from_spec(set@, partition, map@, i as int + 1));
        }
        i = i + 1;
    }
    None
}

// ── Isolation proofs ─────────────────────────────────────────────────

// r[verify cache-partition.isolation]
/// Same-partition hit: if a way holds the tag and belongs to the partition,
/// lookup_partitioned finds it (or an earlier match).
proof fn lookup_partitioned_same_partition_hit(
    set: Seq<Option<u64>>,
    tag: u64,
    partition: u32,
    map: Seq<u32>,
    way: int,
)
    requires
        set.len() == map.len(),
        0 <= way < set.len(),
        map[way] == partition,
        set[way] == Some(tag),
    ensures lookup_partitioned_spec(set, tag, partition, map).is_some()
{
    lookup_partitioned_from_finds_witness(set, tag, partition, map, 0, way);
}

proof fn lookup_partitioned_from_finds_witness(
    set: Seq<Option<u64>>,
    tag: u64,
    partition: u32,
    map: Seq<u32>,
    from: int,
    witness: int,
)
    requires
        set.len() == map.len(),
        0 <= from,
        from <= witness,
        witness < set.len(),
        map[witness] == partition,
        set[witness] == Some(tag),
    ensures lookup_partitioned_from_spec(set, tag, partition, map, from).is_some()
    decreases witness - from
{
    if from == witness {
        // At the witness: it matches, so spec returns Some(from)
    } else {
        // from < witness: whether or not from matches, the result is Some
        if map[from] == partition && set[from] == Some(tag) {
            // Found an earlier match
        } else {
            lookup_partitioned_from_finds_witness(set, tag, partition, map, from + 1, witness);
        }
    }
}

// r[verify cache-partition.isolation]
/// Hit result belongs to the requesting partition.
proof fn lookup_partitioned_hit_in_partition(
    set: Seq<Option<u64>>,
    tag: u64,
    partition: u32,
    map: Seq<u32>,
    idx: int,
)
    requires
        set.len() == map.len(),
        lookup_partitioned_spec(set, tag, partition, map) == Some(idx),
    ensures
        0 <= idx < set.len(),
        map[idx] == partition,
        set[idx] == Some(tag),
{
    lookup_partitioned_from_result_valid(set, tag, partition, map, 0, idx);
}

proof fn lookup_partitioned_from_result_valid(
    set: Seq<Option<u64>>,
    tag: u64,
    partition: u32,
    map: Seq<u32>,
    from: int,
    idx: int,
)
    requires
        set.len() == map.len(),
        lookup_partitioned_from_spec(set, tag, partition, map, from) == Some(idx),
    ensures
        0 <= idx < set.len(),
        map[idx] == partition,
        set[idx] == Some(tag),
    decreases set.len() - from
{
    if from >= set.len() || from < 0 {
        // None case — contradiction
    } else if map[from] == partition && set[from] == Some(tag) {
        assert(idx == from);
    } else {
        lookup_partitioned_from_result_valid(set, tag, partition, map, from + 1, idx);
    }
}

// r[verify cache-partition.isolation]
/// Cross-partition miss: a tag match in a way belonging to another
/// partition is invisible to the requesting partition's lookup.
proof fn lookup_partitioned_cross_partition_miss(
    set: Seq<Option<u64>>,
    tag: u64,
    partition_p: u32,
    partition_q: u32,
    map: Seq<u32>,
    way: int,
)
    requires
        set.len() == map.len(),
        partition_p != partition_q,
        0 <= way < set.len(),
        map[way] == partition_q,
        set[way] == Some(tag),
        lookup_partitioned_spec(set, tag, partition_p, map).is_some(),
    ensures
        lookup_partitioned_spec(set, tag, partition_p, map).unwrap() != way
{
    let idx = lookup_partitioned_spec(set, tag, partition_p, map).unwrap();
    lookup_partitioned_hit_in_partition(set, tag, partition_p, map, idx);
    // idx belongs to partition_p, way belongs to partition_q, so idx != way
}

// r[verify cache-partition.isolation]
/// No data leakage: lookup only examines ways in the requesting partition.
/// If a way is not assigned to the partition, the lookup result is
/// independent of that way's contents.
proof fn lookup_partitioned_no_leakage(
    set1: Seq<Option<u64>>,
    set2: Seq<Option<u64>>,
    tag: u64,
    partition: u32,
    map: Seq<u32>,
)
    requires
        set1.len() == map.len(),
        set2.len() == map.len(),
        // All ways assigned to `partition` are identical in both sets
        forall|w: int| 0 <= w < map.len() && map[w] == partition
            ==> set1[w] == set2[w],
    ensures
        lookup_partitioned_spec(set1, tag, partition, map)
            == lookup_partitioned_spec(set2, tag, partition, map)
{
    lookup_partitioned_from_no_leakage(set1, set2, tag, partition, map, 0);
}

proof fn lookup_partitioned_from_no_leakage(
    set1: Seq<Option<u64>>,
    set2: Seq<Option<u64>>,
    tag: u64,
    partition: u32,
    map: Seq<u32>,
    from: int,
)
    requires
        set1.len() == map.len(),
        set2.len() == map.len(),
        from >= 0,
        forall|w: int| 0 <= w < map.len() && map[w] == partition
            ==> set1[w] == set2[w],
    ensures
        lookup_partitioned_from_spec(set1, tag, partition, map, from)
            == lookup_partitioned_from_spec(set2, tag, partition, map, from)
    decreases set1.len() - from
{
    if from >= set1.len() {
    } else {
        if map[from] == partition {
            // set1[from] == set2[from] by precondition
            if set1[from] == Some(tag) {
                // Both return Some(from)
            } else {
                lookup_partitioned_from_no_leakage(set1, set2, tag, partition, map, from + 1);
            }
        } else {
            // Not our partition — both skip
            lookup_partitioned_from_no_leakage(set1, set2, tag, partition, map, from + 1);
        }
    }
}

// ── Eviction containment proofs ──────────────────────────────────────

// r[verify cache-partition.eviction-containment]
/// Eviction victim belongs to the requesting partition.
proof fn evict_partitioned_in_partition(
    set: Seq<Option<u64>>,
    partition: u32,
    map: Seq<u32>,
    idx: int,
)
    requires
        set.len() == map.len(),
        evict_partitioned_spec(set, partition, map) == Some(idx),
    ensures
        0 <= idx < set.len(),
        map[idx] == partition,
        set[idx].is_some(),
{
    evict_partitioned_from_result_valid(set, partition, map, 0, idx);
}

proof fn evict_partitioned_from_result_valid(
    set: Seq<Option<u64>>,
    partition: u32,
    map: Seq<u32>,
    from: int,
    idx: int,
)
    requires
        set.len() == map.len(),
        evict_partitioned_from_spec(set, partition, map, from) == Some(idx),
    ensures
        0 <= idx < set.len(),
        map[idx] == partition,
        set[idx].is_some(),
    decreases set.len() - from
{
    if from >= set.len() || from < 0 {
    } else if map[from] == partition && set[from].is_some() {
        assert(idx == from);
    } else {
        evict_partitioned_from_result_valid(set, partition, map, from + 1, idx);
    }
}

// r[verify cache-partition.eviction-containment]
/// Other partition's ways unchanged: eviction only affects the victim
/// way, which belongs to the requesting partition.
proof fn evict_other_partition_unchanged(
    set: Seq<Option<u64>>,
    partition: u32,
    map: Seq<u32>,
    idx: int,
    other_way: int,
)
    requires
        set.len() == map.len(),
        evict_partitioned_spec(set, partition, map) == Some(idx),
        0 <= other_way < set.len(),
        map[other_way] != partition,
    ensures
        // After eviction (clearing set[idx]), other_way's data is unchanged
        set.update(idx, None)[other_way] == set[other_way]
{
    evict_partitioned_in_partition(set, partition, map, idx);
    // idx belongs to partition, other_way doesn't, so idx != other_way
    assert(idx != other_way);
}

// ── Capacity accounting ──────────────────────────────────────────────

// r[depends cache-partition.capacity]
/// Sum of partition sizes for partitions `[0, p)`.
pub open spec fn partition_size_sum_spec(map: Seq<u32>, num_partitions: u32, p: u32) -> int
    decreases p as int
{
    if p == 0 {
        0
    } else {
        partition_size_sum_spec(map, num_partitions, (p - 1) as u32)
            + partition_size_spec((p - 1) as u32, map)
    }
}

/// Count how many entries in `map[from..]` have value `< p`.
spec fn count_in_range_from_spec(map: Seq<u32>, p: u32, from: int) -> int
    decreases map.len() - from
{
    if from >= map.len() || from < 0 {
        0
    } else if map[from] < p {
        1 + count_in_range_from_spec(map, p, from + 1)
    } else {
        count_in_range_from_spec(map, p, from + 1)
    }
}

spec fn count_in_range_spec(map: Seq<u32>, p: u32) -> int {
    count_in_range_from_spec(map, p, 0)
}

/// Key lemma: partition_size_sum equals count_in_range.
/// The sum of per-partition sizes for [0, p) equals the count of ways
/// with partition ID < p.
proof fn size_sum_equals_count_in_range(map: Seq<u32>, num_partitions: u32, p: u32)
    requires
        valid_assignment_spec(map, num_partitions),
        p <= num_partitions,
    ensures partition_size_sum_spec(map, num_partitions, p) == count_in_range_spec(map, p)
    decreases p as int
{
    if p == 0 {
        // Both are 0
        count_in_range_from_zero(map, 0);
    } else {
        size_sum_equals_count_in_range(map, num_partitions, (p - 1) as u32);
        // partition_size_sum(p) = partition_size_sum(p-1) + partition_size(p-1)
        //                       = count_in_range(p-1) + partition_size(p-1)
        // Need: count_in_range(p-1) + partition_size(p-1) == count_in_range(p)
        count_split_last_partition(map, (p - 1) as u32);
    }
}

proof fn count_in_range_from_zero(map: Seq<u32>, from: int)
    ensures count_in_range_from_spec(map, 0, from) == 0
    decreases map.len() - from
{
    if from >= map.len() || from < 0 {
    } else {
        count_in_range_from_zero(map, from + 1);
    }
}

/// count_in_range(p+1) == count_in_range(p) + partition_size(p)
proof fn count_split_last_partition(map: Seq<u32>, p: u32)
    requires p < u32::MAX
    ensures
        count_in_range_spec(map, (p + 1) as u32)
            == count_in_range_spec(map, p) + partition_size_spec(p, map)
{
    count_split_last_partition_from(map, p, 0);
}

proof fn count_split_last_partition_from(map: Seq<u32>, p: u32, from: int)
    requires from >= 0, p < u32::MAX
    ensures
        count_in_range_from_spec(map, (p + 1) as u32, from)
            == count_in_range_from_spec(map, p, from)
                + partition_size_from_spec(p, map, from)
    decreases map.len() - from
{
    if from >= map.len() {
    } else {
        count_split_last_partition_from(map, p, from + 1);
        // At position `from`:
        // If map[from] == p: counted in (p+1) but not in p, and counted in partition_size(p)
        // If map[from] < p: counted in both (p+1) and p, not in partition_size(p)
        // If map[from] > p: counted in neither, not in partition_size(p)
        // If map[from] == p+1 but > p and < p+1: impossible since u32
    }
}

/// When valid, count_in_range(num_partitions) == map.len()
proof fn count_in_range_full(map: Seq<u32>, num_partitions: u32)
    requires valid_assignment_spec(map, num_partitions)
    ensures count_in_range_spec(map, num_partitions) == map.len()
{
    count_in_range_full_from(map, num_partitions, 0);
}

proof fn count_in_range_full_from(map: Seq<u32>, num_partitions: u32, from: int)
    requires
        valid_assignment_spec(map, num_partitions),
        0 <= from <= map.len(),
    ensures count_in_range_from_spec(map, num_partitions, from) == map.len() - from
    decreases map.len() - from
{
    if from >= map.len() {
    } else {
        count_in_range_full_from(map, num_partitions, from + 1);
        // map[from] < num_partitions by valid_assignment_spec
        assert(map[from] < num_partitions);
    }
}

// r[verify cache-partition.capacity]
/// Capacity sum: for a valid assignment, the sum of all partition sizes
/// equals the total number of ways.
pub proof fn capacity_sum(map: Seq<u32>, num_partitions: u32)
    requires valid_assignment_spec(map, num_partitions)
    ensures partition_size_sum_spec(map, num_partitions, num_partitions) == map.len()
{
    size_sum_equals_count_in_range(map, num_partitions, num_partitions);
    count_in_range_full(map, num_partitions);
}

// r[verify cache-partition.assignment]
/// Valid assignment ensures every way has a partition in range.
pub proof fn valid_assignment_in_range(map: Seq<u32>, num_partitions: u32, way: int)
    requires
        valid_assignment_spec(map, num_partitions),
        0 <= way < map.len(),
    ensures 0 <= map[way] < num_partitions
{}

// ── Resize ───────────────────────────────────────────────────────────

// r[depends cache-partition.resize]
/// Spec for resize: reassign transferred ways and invalidate their data.
pub open spec fn resize_spec(
    map: Seq<u32>,
    set: Seq<Option<u64>>,
    from_partition: u32,
    to_partition: u32,
    ways_to_transfer: Set<u32>,
) -> (Seq<u32>, Seq<Option<u64>>)
    recommends
        map.len() == set.len(),
{
    (
        resize_map_spec(map, from_partition, to_partition, ways_to_transfer),
        resize_set_spec(set, map, from_partition, ways_to_transfer),
    )
}

/// New partition map after resize.
pub open spec fn resize_map_spec(
    map: Seq<u32>,
    from_partition: u32,
    to_partition: u32,
    ways_to_transfer: Set<u32>,
) -> Seq<u32> {
    Seq::new(map.len(), |w: int|
        if ways_to_transfer.contains(w as u32) && map[w] == from_partition {
            to_partition
        } else {
            map[w]
        }
    )
}

/// New set after resize: transferred ways are invalidated.
pub open spec fn resize_set_spec(
    set: Seq<Option<u64>>,
    map: Seq<u32>,
    from_partition: u32,
    ways_to_transfer: Set<u32>,
) -> Seq<Option<u64>> {
    Seq::new(set.len(), |w: int|
        if ways_to_transfer.contains(w as u32) && map[w] == from_partition {
            None
        } else {
            set[w]
        }
    )
}

// r[impl cache-partition.resize]
pub fn resize(
    map: &mut Vec<u32>,
    set: &mut Vec<Option<u64>>,
    from_partition: u32,
    to_partition: u32,
    ways_to_transfer: &[u32],
)
    requires
        old(map)@.len() == old(set)@.len(),
        old(map)@.len() <= u32::MAX as int,
    ensures
        map@.len() == old(map)@.len(),
        set@.len() == old(set)@.len(),
        // Each way: if in transfer list and was from_partition, reassigned+invalidated;
        // otherwise unchanged.
        forall|w: int| #![auto] 0 <= w < map@.len() ==> (
            if ways_to_transfer@.contains(w as u32) && old(map)@[w] == from_partition {
                map@[w] == to_partition && set@[w].is_none()
            } else {
                map@[w] == old(map)@[w] && set@[w] == old(set)@[w]
            }
        ),
{
    let ghost old_map = map@;
    let ghost old_set = set@;
    let ghost mut processed: Set<u32> = Set::empty();
    let mut t: usize = 0;
    while t < ways_to_transfer.len()
        invariant
            0 <= t <= ways_to_transfer@.len(),
            map@.len() == old_map.len(),
            set@.len() == old_set.len(),
            old_map.len() == old_set.len(),
            map@.len() <= u32::MAX as int,
            // processed tracks indices [0..t)
            forall|k: int| #![auto] 0 <= k < t as int ==> processed.contains(ways_to_transfer@[k]),
            forall|v: u32| processed.contains(v) ==> exists|k: int| 0 <= k < t as int && ways_to_transfer@[k] == v,
            // Processed ways: applied
            forall|w: int| #![auto] 0 <= w < map@.len() && processed.contains(w as u32) && old_map[w] == from_partition
                ==> map@[w] == to_partition && set@[w].is_none(),
            // Unprocessed ways: unchanged
            forall|w: int| #![auto] 0 <= w < map@.len() && !processed.contains(w as u32)
                ==> map@[w] == old_map[w] && set@[w] == old_set[w],
            // Processed ways where old_map didn't match: still unchanged
            forall|w: int| #![auto] 0 <= w < map@.len() && processed.contains(w as u32) && old_map[w] != from_partition
                ==> map@[w] == old_map[w] && set@[w] == old_set[w],
        decreases ways_to_transfer@.len() - t as int
    {
        let way_val = ways_to_transfer[t];
        let way = way_val as usize;
        if way < map.len() && map[way] == from_partition {
            map.set(way, to_partition);
            set.set(way, None);
            proof {
                // After mutation: map@[way] == to_partition, set@[way] == None
                // Now add to processed. The "processed + from_partition" invariant
                // holds for way since map@[way] == to_partition and set@[way].is_none().
                // For other w != way, map@[w] and set@[w] are unchanged.
                processed = processed.insert(way_val);
                // Show all previous invariants still hold:
                assert forall|w: int| #![auto]
                    0 <= w < map@.len() && !processed.contains(w as u32)
                    implies map@[w] == old_map[w] && set@[w] == old_set[w] by {
                    // w != way since processed contains way_val
                    // and !processed.contains(w as u32), so w as u32 != way_val
                    // so map@[w] unchanged
                    if w == way as int {
                        assert(processed.contains(way_val));
                        assert(way_val == w as u32);
                        assert(false);
                    }
                }
            }
        } else {
            proof {
                // No mutation in this branch.
                // Save pre-insert processed for reasoning.
                let pre_processed = processed;
                processed = processed.insert(way_val);
                assert forall|w: int| #![auto]
                    0 <= w < map@.len() && processed.contains(w as u32) && old_map[w] == from_partition
                    implies map@[w] == to_partition && set@[w].is_none() by {
                    if w as u32 == way_val {
                        if way >= map@.len() as usize {
                            // way >= map.len(), w < map.len(), w as u32 == way_val
                            // way_val = ways_to_transfer[t], way = way_val as usize
                            // Since map.len() <= u32::MAX, way_val as usize >= map.len()
                            // means way_val as int >= map.len(). But w < map.len() and
                            // w as u32 == way_val means w == way_val as int >= map.len().
                            // Contradiction with w < map.len().
                            assert(w == way_val as int);
                            assert(w >= map@.len());
                            assert(false);
                        } else {
                            // way < map.len() and map[way] != from_partition
                            // If way was not previously processed:
                            if !pre_processed.contains(w as u32) {
                                // unprocessed invariant: map@[w] == old_map[w]
                                // old_map[w] == from_partition (given)
                                // so map@[w] == from_partition
                                // but map[way] != from_partition in this branch
                                // w == way (since w as u32 == way_val and way = way_val as usize
                                // and map.len() <= u32::MAX)
                                assert(w == way as int);
                                assert(map@[w] == old_map[w]);
                                assert(map@[w] == from_partition);
                                // But the else branch says map[way] != from_partition
                                assert(false);
                            }
                            // way was previously processed:
                            // pre_processed.contains(w as u32) && old_map[w] == from_partition
                            // => by invariant, map@[w] == to_partition && set@[w].is_none()
                        }
                    }
                    // w as u32 != way_val: pre_processed.contains(w as u32) == processed.contains(w as u32)
                    // so the previous invariant applies
                }
            }
        }
        t = t + 1;
    }
    proof {
        // Show processed >= {values in ways_to_transfer}
        // For any w: if ways_to_transfer@.contains(w as u32), then there exists k
        // such that ways_to_transfer@[k] == w as u32. Since k < len == t, processed
        // contains w as u32.
        assert forall|w: int| #![auto]
            0 <= w < map@.len()
        implies (
            if ways_to_transfer@.contains(w as u32) && old_map[w] == from_partition {
                map@[w] == to_partition && set@[w].is_none()
            } else {
                map@[w] == old_map[w] && set@[w] == old_set[w]
            }
        ) by {
            if ways_to_transfer@.contains(w as u32) {
                let k = choose|k: int| 0 <= k < ways_to_transfer@.len() && ways_to_transfer@[k] == w as u32;
                assert(processed.contains(w as u32));
            }
        }
    }
}

// r[verify cache-partition.resize]
/// Transferred ways are reassigned.
proof fn resize_transferred_reassigned(
    map: Seq<u32>,
    set: Seq<Option<u64>>,
    from_partition: u32,
    to_partition: u32,
    ways_to_transfer: Set<u32>,
    way: int,
)
    requires
        map.len() == set.len(),
        0 <= way < map.len(),
        ways_to_transfer.contains(way as u32),
        map[way] == from_partition,
    ensures
        resize_map_spec(map, from_partition, to_partition, ways_to_transfer)[way] == to_partition,
        resize_set_spec(set, map, from_partition, ways_to_transfer)[way].is_none(),
{}

// r[verify cache-partition.resize]
/// Non-transferred ways unchanged.
proof fn resize_nontransferred_unchanged(
    map: Seq<u32>,
    set: Seq<Option<u64>>,
    from_partition: u32,
    to_partition: u32,
    ways_to_transfer: Set<u32>,
    way: int,
)
    requires
        map.len() == set.len(),
        0 <= way < map.len(),
        !(ways_to_transfer.contains(way as u32) && map[way] == from_partition),
    ensures
        resize_map_spec(map, from_partition, to_partition, ways_to_transfer)[way] == map[way],
        resize_set_spec(set, map, from_partition, ways_to_transfer)[way] == set[way],
{}

// r[verify cache-partition.resize]
/// Capacity preserved: resize doesn't change the number of ways.
proof fn resize_capacity_preserved(
    map: Seq<u32>,
    set: Seq<Option<u64>>,
    from_partition: u32,
    to_partition: u32,
    ways_to_transfer: Set<u32>,
)
    requires map.len() == set.len()
    ensures
        resize_map_spec(map, from_partition, to_partition, ways_to_transfer).len() == map.len(),
        resize_set_spec(set, map, from_partition, ways_to_transfer).len() == set.len(),
{}

// ── Full-cache equivalence ───────────────────────────────────────────

// r[impl cache-partition.full-equivalence]
/// All ways assigned to partition 0.
pub open spec fn single_partition_map_spec(num_ways: nat) -> Seq<u32> {
    Seq::new(num_ways, |_w: int| 0u32)
}

/// Unpartitioned find_way (same as set_associative_lookup::find_way_from_spec).
pub open spec fn find_way_from_spec(set: Seq<Option<u64>>, tag: u64, from: int) -> Option<int>
    decreases set.len() - from
{
    if from >= set.len() || from < 0 {
        None
    } else if set[from] == Some(tag) {
        Some(from)
    } else {
        find_way_from_spec(set, tag, from + 1)
    }
}

pub open spec fn find_way_spec(set: Seq<Option<u64>>, tag: u64) -> Option<int> {
    find_way_from_spec(set, tag, 0)
}

// r[verify cache-partition.full-equivalence]
/// Single-partition lookup equivalence: with all ways in partition 0,
/// partitioned lookup behaves identically to unpartitioned find_way.
pub proof fn single_partition_lookup_equivalence(
    set: Seq<Option<u64>>,
    tag: u64,
)
    requires set.len() > 0
    ensures
        lookup_partitioned_spec(set, tag, 0, single_partition_map_spec(set.len()))
            == find_way_spec(set, tag)
{
    single_partition_lookup_from_equivalence(set, tag, 0);
}

proof fn single_partition_lookup_from_equivalence(
    set: Seq<Option<u64>>,
    tag: u64,
    from: int,
)
    requires from >= 0
    ensures
        lookup_partitioned_from_spec(set, tag, 0, single_partition_map_spec(set.len()), from)
            == find_way_from_spec(set, tag, from)
    decreases set.len() - from
{
    if from >= set.len() {
    } else {
        let map = single_partition_map_spec(set.len());
        assert(map[from] == 0u32);
        if set[from] == Some(tag) {
            // Both return Some(from)
        } else {
            single_partition_lookup_from_equivalence(set, tag, from + 1);
        }
    }
}

// r[verify cache-partition.full-equivalence]
/// Single-partition eviction equivalence: with all ways in partition 0,
/// partitioned eviction considers all ways.
pub proof fn single_partition_evict_equivalence(
    set: Seq<Option<u64>>,
)
    requires set.len() > 0
    ensures
        evict_partitioned_spec(set, 0, single_partition_map_spec(set.len()))
            == find_first_occupied_spec(set)
{
    single_partition_evict_from_equivalence(set, 0);
}

/// First occupied way (unpartitioned).
pub open spec fn find_first_occupied_spec(set: Seq<Option<u64>>) -> Option<int> {
    find_first_occupied_from_spec(set, 0)
}

pub open spec fn find_first_occupied_from_spec(set: Seq<Option<u64>>, from: int) -> Option<int>
    decreases set.len() - from
{
    if from >= set.len() || from < 0 {
        None
    } else if set[from].is_some() {
        Some(from)
    } else {
        find_first_occupied_from_spec(set, from + 1)
    }
}

proof fn single_partition_evict_from_equivalence(
    set: Seq<Option<u64>>,
    from: int,
)
    requires from >= 0
    ensures
        evict_partitioned_from_spec(set, 0, single_partition_map_spec(set.len()), from)
            == find_first_occupied_from_spec(set, from)
    decreases set.len() - from
{
    if from >= set.len() {
    } else {
        let map = single_partition_map_spec(set.len());
        assert(map[from] == 0u32);
        if set[from].is_some() {
            // Both return Some(from)
        } else {
            single_partition_evict_from_equivalence(set, from + 1);
        }
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify cache-partition.isolation]
    #[test]
    fn test_lookup_hit_within_partition() {
        // Way 1 assigned to partition 0, holds tag 0xAA
        let set = vec![None, Some(0xAA), None, Some(0xBB)];
        let map = vec![1, 0, 1, 0];
        assert_eq!(lookup_partitioned(&set, 0xAA, 0, &map), Some(1));
    }

    // r[verify cache-partition.isolation]
    #[test]
    fn test_lookup_miss_across_partition() {
        // Way 0 holds tag 0xAA but belongs to partition 1
        let set = vec![Some(0xAA), None, None, None];
        let map = vec![1, 0, 0, 0];
        assert_eq!(lookup_partitioned(&set, 0xAA, 0, &map), None);
    }

    // r[verify cache-partition.isolation]
    #[test]
    fn test_lookup_same_partition_hit() {
        let set = vec![Some(0x11), Some(0x22), Some(0x33), Some(0x44)];
        let map = vec![0, 1, 0, 1];
        // Partition 0 owns ways 0 and 2
        assert_eq!(lookup_partitioned(&set, 0x33, 0, &map), Some(2));
        // Partition 1 owns ways 1 and 3
        assert_eq!(lookup_partitioned(&set, 0x22, 1, &map), Some(1));
    }

    // r[verify cache-partition.eviction-containment]
    #[test]
    fn test_evict_from_correct_partition() {
        let set = vec![Some(0xAA), Some(0xBB), Some(0xCC), Some(0xDD)];
        let map = vec![0, 1, 0, 1];
        // Evict from partition 1: should get way 1 (first occupied in partition 1)
        assert_eq!(evict_partitioned(&set, 1, &map), Some(1));
    }

    // r[verify cache-partition.eviction-containment]
    #[test]
    fn test_evict_no_victim_when_partition_empty() {
        let set = vec![None, None, Some(0xCC), Some(0xDD)];
        let map = vec![0, 0, 1, 1];
        // Partition 0 has no occupied ways
        assert_eq!(evict_partitioned(&set, 0, &map), None);
    }

    // r[verify cache-partition.capacity]
    #[test]
    fn test_partition_size() {
        let map = vec![0, 1, 0, 1, 0];
        assert_eq!(partition_size(0, &map), 3);
        assert_eq!(partition_size(1, &map), 2);
        assert_eq!(partition_size(2, &map), 0);
    }

    // r[verify cache-partition.assignment]
    #[test]
    fn test_valid_assignment() {
        assert!(valid_assignment(&[0, 1, 0, 1], 2));
        assert!(!valid_assignment(&[0, 1, 2, 1], 2)); // 2 is out of range
        assert!(valid_assignment(&[0, 0, 0, 0], 1));
        assert!(!valid_assignment(&[0, 1], 1)); // 1 is out of range for num_partitions=1
    }

    // r[verify cache-partition.resize]
    #[test]
    fn test_resize_transfer() {
        let mut map = vec![0, 0, 0, 1, 1];
        let mut set: Vec<Option<u64>> = vec![Some(0xAA), Some(0xBB), Some(0xCC), Some(0xDD), Some(0xEE)];
        // Transfer way 1 from partition 0 to partition 1
        resize(&mut map, &mut set, 0, 1, &[1]);
        assert_eq!(map, vec![0, 1, 0, 1, 1]);
        assert_eq!(set[1], None); // Invalidated
        assert_eq!(set[0], Some(0xAA)); // Unchanged
        assert_eq!(set[3], Some(0xDD)); // Unchanged
    }

    // r[verify cache-partition.resize]
    #[test]
    fn test_resize_nontransferred_unchanged() {
        let mut map = vec![0, 0, 1, 1];
        let mut set: Vec<Option<u64>> = vec![Some(0xAA), Some(0xBB), Some(0xCC), Some(0xDD)];
        resize(&mut map, &mut set, 0, 1, &[0]);
        // Way 1 is in partition 0 but not in ways_to_transfer
        assert_eq!(map[1], 0);
        assert_eq!(set[1], Some(0xBB));
        // Way 2 is in partition 1, not from_partition
        assert_eq!(map[2], 1);
        assert_eq!(set[2], Some(0xCC));
    }

    // r[verify cache-partition.resize]
    #[test]
    fn test_resize_capacity_preserved() {
        let mut map = vec![0, 0, 1, 1];
        let mut set: Vec<Option<u64>> = vec![Some(1), Some(2), Some(3), Some(4)];
        resize(&mut map, &mut set, 0, 1, &[0, 1]);
        assert_eq!(map.len(), 4);
        assert_eq!(set.len(), 4);
    }

    // r[verify cache-partition.full-equivalence]
    #[test]
    fn test_single_partition_lookup() {
        let set = vec![None, Some(0x42), None, Some(0x99)];
        let map = vec![0, 0, 0, 0];
        // Should behave like unpartitioned: find first match
        assert_eq!(lookup_partitioned(&set, 0x42, 0, &map), Some(1));
        assert_eq!(lookup_partitioned(&set, 0x99, 0, &map), Some(3));
        assert_eq!(lookup_partitioned(&set, 0xFF, 0, &map), None);
    }

    proptest! {
        // r[verify cache-partition.isolation]
        #[test]
        fn prop_isolation_random(
            num_ways in 1usize..=16,
            num_partitions in 1u32..=4,
            seed: u64,
        ) {
            let mut map = Vec::new();
            let mut set = Vec::new();
            let mut rng = seed;
            for _ in 0..num_ways {
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                map.push(u32::try_from(rng % (num_partitions as u64)).expect("partition index fits in u32"));
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                if rng % 3 == 0 {
                    set.push(None);
                } else {
                    set.push(Some(rng));
                }
            }

            for tag_probe in [0u64, 1, u64::MAX, seed] {
                for p in 0..num_partitions {
                    if let Some(idx) = lookup_partitioned(&set, tag_probe, p, &map) {
                        // Hit must be in the right partition
                        prop_assert_eq!(map[idx], p);
                        prop_assert_eq!(set[idx], Some(tag_probe));
                    }
                }
            }
        }

        // r[verify cache-partition.capacity]
        #[test]
        fn prop_capacity_sum_random(
            num_ways in 0usize..=20,
            num_partitions in 1u32..=5,
            seed: u64,
        ) {
            let mut map = Vec::new();
            let mut rng = seed;
            for _ in 0..num_ways {
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                map.push(u32::try_from(rng % (num_partitions as u64)).expect("partition index fits in u32"));
            }

            let mut total = 0usize;
            for p in 0..num_partitions {
                total += partition_size(p, &map);
            }
            prop_assert_eq!(total, num_ways);
        }

        // r[verify cache-partition.resize]
        #[test]
        fn prop_resize_preserves_capacity_and_isolation(
            num_ways in 2usize..=12,
            num_partitions in 2u32..=4,
            seed: u64,
        ) {
            let mut map = Vec::new();
            let mut set = Vec::new();
            let mut rng = seed;
            for _ in 0..num_ways {
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                map.push(u32::try_from(rng % (num_partitions as u64)).expect("partition index fits in u32"));
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                set.push(if rng % 2 == 0 { Some(rng) } else { None });
            }

            // Transfer some ways from partition 0 to partition 1
            let ways_to_transfer: Vec<u32> = (0..u32::try_from(num_ways).expect("num_ways fits in u32"))
                .filter(|&w| map[w as usize] == 0)
                .take(2)
                .collect();

            let old_map = map.clone();
            resize(&mut map, &mut set, 0, 1, &ways_to_transfer);

            // Length preserved
            prop_assert_eq!(map.len(), num_ways);
            prop_assert_eq!(set.len(), num_ways);

            // Transferred ways: reassigned and invalidated
            for &w in &ways_to_transfer {
                if old_map[w as usize] == 0 {
                    prop_assert_eq!(map[w as usize], 1);
                    prop_assert_eq!(set[w as usize], None);
                }
            }

            // Still a valid assignment if original was valid
            if valid_assignment(&old_map, num_partitions) {
                // New assignment only changes 0->1, still in range
                for &m in &map {
                    prop_assert!(m < num_partitions);
                }
            }
        }
    }
}
