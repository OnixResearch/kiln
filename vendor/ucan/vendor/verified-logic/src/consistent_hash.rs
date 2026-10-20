//! Verified consistent hash ring.
//!
//! A ring is a sorted array of (hash, `node_id`) pairs. Lookup finds the
//! first node whose hash is >= the key hash, wrapping to index 0 if none.
//! The key safety property: a non-empty ring always assigns every key
//! to exactly one node (complete coverage).

#![allow(clippy::ptr_arg)]

use vstd::prelude::*;

verus! {

// --- lookup ---

// r[depends consistent-hash.lookup]
/// Spec: find the node responsible for `key_hash` on the ring.
/// Returns the node_id of the first entry with hash >= key_hash,
/// wrapping to the first entry if no such entry exists.
pub open spec fn lookup_spec(ring: Seq<(u64, u64)>, key_hash: u64) -> u64
    recommends ring.len() > 0
{
    let idx = first_ge_index_spec(ring, key_hash);
    if idx < ring.len() as int {
        ring[idx].1
    } else {
        ring[0int].1 // wrap around
    }
}

/// Spec: index of first entry with hash >= key_hash, or ring.len() if none.
pub open spec fn first_ge_index_spec(ring: Seq<(u64, u64)>, key_hash: u64) -> int {
    if ring.len() == 0 {
        0int
    } else {
        first_ge_index_helper(ring, key_hash, 0)
    }
}

/// Helper: linear scan from `from`.
pub open spec fn first_ge_index_helper(ring: Seq<(u64, u64)>, key_hash: u64, from: int) -> int
    decreases ring.len() - from
{
    if from >= ring.len() as int {
        ring.len() as int
    } else if ring[from].0 >= key_hash {
        from
    } else {
        first_ge_index_helper(ring, key_hash, from + 1)
    }
}

// Helper: if ring[from].0 < key_hash, the helper skips to from+1.
proof fn first_ge_skip(ring: Seq<(u64, u64)>, key_hash: u64, from: int)
    requires 0 <= from < ring.len() as int, ring[from].0 < key_hash
    ensures first_ge_index_helper(ring, key_hash, from)
        == first_ge_index_helper(ring, key_hash, from + 1)
{}

// r[impl consistent-hash.lookup]
/// Find the node responsible for a key hash on the ring.
pub fn lookup(ring: &Vec<(u64, u64)>, key_hash: u64) -> (result: u64)
    requires ring.len() > 0
    ensures result == lookup_spec(ring@, key_hash)
{
    let mut i: usize = 0;
    while i < ring.len()
        invariant
            i <= ring.len(),
            ring.len() > 0,
            // The recursive helper from 0 equals the helper from i
            // (all elements before i had hash < key_hash)
            first_ge_index_helper(ring@, key_hash, 0)
                == first_ge_index_helper(ring@, key_hash, i as int),
        decreases ring.len() - i,
    {
        if ring[i].0 >= key_hash {
            // first_ge_index_helper(ring@, key_hash, i) == i (by definition)
            assert(first_ge_index_helper(ring@, key_hash, i as int) == i as int);
            return ring[i].1;
        }
        // ring[i].0 < key_hash, so helper skips from i to i+1
        proof { first_ge_skip(ring@, key_hash, i as int); }
        i = i + 1;
    }
    // i == ring.len(), helper(ring, key_hash, ring.len()) == ring.len()
    assert(first_ge_index_helper(ring@, key_hash, ring@.len() as int) == ring@.len() as int);
    ring[0].1
}

// --- Safety proofs ---

// Helper: first_ge_index_helper returns a value in [from, ring.len()].
proof fn first_ge_index_helper_bounds(ring: Seq<(u64, u64)>, key_hash: u64, from: int)
    requires 0 <= from, from <= ring.len()
    ensures from <= first_ge_index_helper(ring, key_hash, from) <= ring.len() as int
    decreases ring.len() - from
{
    if from < ring.len() as int && !(ring[from].0 >= key_hash) {
        first_ge_index_helper_bounds(ring, key_hash, from + 1);
    }
}

// r[verify consistent-hash.lookup]
/// Complete coverage: for any key_hash, lookup on a non-empty ring
/// returns a node_id that exists in the ring.
proof fn complete_coverage(ring: Seq<(u64, u64)>, key_hash: u64)
    requires ring.len() > 0
    ensures
        exists|i: int| 0 <= i < ring.len() as int
            && ring[i].1 == lookup_spec(ring, key_hash)
{
    let idx = first_ge_index_spec(ring, key_hash);
    first_ge_index_helper_bounds(ring, key_hash, 0);
    if idx < ring.len() as int {
        // Found an element: witness is idx itself
        assert(ring[idx].1 == lookup_spec(ring, key_hash));
    } else {
        // Wrap around: witness is 0
        assert(ring[0int].1 == lookup_spec(ring, key_hash));
    }
}

// r[verify consistent-hash.lookup]
/// Deterministic: same ring and key always produce the same result.
proof fn lookup_deterministic(ring: Seq<(u64, u64)>, key_hash: u64)
    requires ring.len() > 0
    ensures lookup_spec(ring, key_hash) == lookup_spec(ring, key_hash)
{}

// r[verify consistent-hash.lookup]
/// Single-node ring: all keys map to that node.
proof fn single_node_all_keys(node_hash: u64, node_id: u64, key_hash: u64)
    ensures lookup_spec(seq![(node_hash, node_id)], key_hash) == node_id
{
    let ring = seq![(node_hash, node_id)];
    // Unfold first_ge_index_helper(ring, key_hash, 0):
    // Case 1: node_hash >= key_hash → helper returns 0 → lookup returns ring[0].1 = node_id
    // Case 2: node_hash < key_hash → helper recurses with from=1 → from >= len → returns 1
    //         → idx=1 >= len=1 → lookup wraps to ring[0].1 = node_id
    assert(first_ge_index_helper(ring, key_hash, 0) ==
        if ring[0int].0 >= key_hash { 0int } else { first_ge_index_helper(ring, key_hash, 1) });
    assert(first_ge_index_helper(ring, key_hash, 1) == ring.len() as int);
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify consistent-hash.lookup]
    #[test]
    fn test_lookup_basic() {
        // Ring: nodes at hash positions 10, 30, 50
        let ring = vec![(10, 1), (30, 2), (50, 3)];
        assert_eq!(lookup(&ring, 5), 1); // before first → first node
        assert_eq!(lookup(&ring, 10), 1); // exact match
        assert_eq!(lookup(&ring, 15), 2); // between 10 and 30 → 30's node
        assert_eq!(lookup(&ring, 35), 3); // between 30 and 50 → 50's node
        assert_eq!(lookup(&ring, 55), 1); // past last → wrap to first
    }

    // r[verify consistent-hash.lookup]
    #[test]
    fn test_single_node() {
        let ring = vec![(100, 42)];
        assert_eq!(lookup(&ring, 0), 42);
        assert_eq!(lookup(&ring, 100), 42);
        assert_eq!(lookup(&ring, 200), 42);
    }

    proptest! {
        // r[verify consistent-hash.lookup]
        #[test]
        fn prop_complete_coverage(
            ring in proptest::collection::vec((any::<u64>(), any::<u64>()), 1..=10),
            key_hash: u64,
        ) {
            let result = lookup(&ring, key_hash);
            // Result must be a node_id that appears in the ring
            prop_assert!(ring.iter().any(|(_, id)| *id == result));
        }

        // r[verify consistent-hash.lookup]
        #[test]
        fn prop_deterministic(
            ring in proptest::collection::vec((any::<u64>(), any::<u64>()), 1..=10),
            key_hash: u64,
        ) {
            prop_assert_eq!(lookup(&ring, key_hash), lookup(&ring, key_hash));
        }

        #[test]
        fn prop_result_is_ring_member(
            ring in proptest::collection::vec((any::<u64>(), any::<u64>()), 1..=10),
            key_hash: u64,
        ) {
            let result = lookup(&ring, key_hash);
            let node_ids: Vec<u64> = ring.iter().map(|&(_, n)| n).collect();
            prop_assert!(node_ids.contains(&result));
        }

        #[test]
        fn prop_single_node_always_returns_it(
            hash: u64,
            node_id: u64,
            key_hash: u64,
        ) {
            let ring = vec![(hash, node_id)];
            prop_assert_eq!(lookup(&ring, key_hash), node_id);
        }
    }
}
