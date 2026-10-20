//! Verified bounded peer collection for bootstrap peer lists.
//!
//! Builds a peer list from candidates with a hard capacity bound and
//! self-exclusion. Mirrors `aspen-cluster-handler`'s ticket peer
//! collection: scan candidates in order, skip self, stop at capacity.
//!
//! Properties: size bounded by max_peers, self never in result,
//! candidates added in order, non-empty when valid candidates exist,
//! no duplicates when input is distinct.

use vstd::prelude::*;

verus! {

// ---------------------------------------------------------------------------
// Helper: distinct
// ---------------------------------------------------------------------------

/// Spec: all elements in `s` are pairwise distinct.
pub open spec fn all_distinct_spec(s: Seq<u64>) -> bool {
    forall|i: int, j: int|
        #![trigger s[i], s[j]]
        0 <= i < s.len() && 0 <= j < s.len() && i != j ==> s[i] != s[j]
}

/// Spec: `s` contains element `x`.
pub open spec fn seq_contains_spec(s: Seq<u64>, x: u64) -> bool {
    exists|i: int| #![trigger s[i]] 0 <= i < s.len() && s[i] == x
}

// ---------------------------------------------------------------------------
// collect_peers_spec — recursive construction
// ---------------------------------------------------------------------------

/// Spec: build peer list from index `from`, collecting at most `remaining` peers.
///
/// Scans candidates left to right:
/// - Skip elements equal to `self_id`.
/// - Stop when `remaining` reaches 0.
/// - Keep everything else (decrement remaining).
pub open spec fn collect_build_spec(
    candidates: Seq<u64>,
    self_id: u64,
    from: int,
    remaining: int,
) -> Seq<u64>
    decreases candidates.len() - from,
{
    if from >= candidates.len() || remaining <= 0 {
        seq![]
    } else {
        let c = candidates[from];
        if c == self_id {
            collect_build_spec(candidates, self_id, from + 1, remaining)
        } else {
            seq![c] + collect_build_spec(candidates, self_id, from + 1, remaining - 1)
        }
    }
}

// r[depends bounded-peers.collect]
/// Top-level spec: collect up to max_peers from candidates, excluding self_id.
pub open spec fn collect_peers_spec(
    candidates: Seq<u64>,
    self_id: u64,
    max_peers: u64,
) -> Seq<u64> {
    collect_build_spec(candidates, self_id, 0, max_peers as int)
}

// ---------------------------------------------------------------------------
// Exec: collect_peers
// ---------------------------------------------------------------------------

// r[impl bounded-peers.collect]
// r[impl bounded-peers.bounded]
// r[impl bounded-peers.self-excluded]
// r[impl bounded-peers.non-empty]
// r[impl bounded-peers.no-duplicates]
/// Collect up to `max_peers` from `candidates`, excluding `self_id`.
pub fn collect_peers(
    candidates: &[u64],
    self_id: u64,
    max_peers: u64,
) -> (result: Vec<u64>)
    ensures result@ == collect_peers_spec(candidates@, self_id, max_peers),
{
    let mut result: Vec<u64> = Vec::new();
    let mut count: u64 = 0;
    let mut i: usize = 0;
    let ghost full = collect_peers_spec(candidates@, self_id, max_peers);
    while i < candidates.len() && count < max_peers
        invariant
            0 <= i <= candidates@.len(),
            0 <= count <= max_peers,
            full == collect_peers_spec(candidates@, self_id, max_peers),
            result@ + collect_build_spec(candidates@, self_id, i as int, (max_peers - count) as int)
                =~= full,
        decreases candidates@.len() - i,
    {
        let c = candidates[i];
        if c != self_id {
            result.push(c);
            count = count + 1;
        }
        i = i + 1;
    }
    // Loop exited: either i >= len or count >= max_peers
    // In both cases, collect_build_spec(...) == seq![]
    // So result@ + seq![] =~= full, hence result@ =~= full
    result
}

// ---------------------------------------------------------------------------
// Property: bounded
// ---------------------------------------------------------------------------

// r[verify bounded-peers.bounded]
/// Result length never exceeds max_peers.
pub proof fn collect_bounded(
    candidates: Seq<u64>,
    self_id: u64,
    max_peers: u64,
)
    ensures
        collect_peers_spec(candidates, self_id, max_peers).len()
        <= max_peers as int,
{
    collect_bounded_build(candidates, self_id, 0, max_peers as int);
}

proof fn collect_bounded_build(
    candidates: Seq<u64>,
    self_id: u64,
    from: int,
    remaining: int,
)
    requires 0 <= from, remaining >= 0,
    ensures
        collect_build_spec(candidates, self_id, from, remaining).len()
        <= remaining,
    decreases candidates.len() - from,
{
    if from >= candidates.len() || remaining <= 0 {
        // result == seq![], len = 0 <= remaining
    } else {
        let c = candidates[from];
        if c == self_id {
            collect_bounded_build(candidates, self_id, from + 1, remaining);
        } else {
            collect_bounded_build(candidates, self_id, from + 1, remaining - 1);
            let rest = collect_build_spec(candidates, self_id, from + 1, remaining - 1);
            let result = collect_build_spec(candidates, self_id, from, remaining);
            assert(result =~= seq![c] + rest);
        }
    }
}

// ---------------------------------------------------------------------------
// Property: self excluded
// ---------------------------------------------------------------------------

// r[verify bounded-peers.self-excluded]
/// self_id never appears in the result.
pub proof fn collect_self_excluded(
    candidates: Seq<u64>,
    self_id: u64,
    max_peers: u64,
)
    ensures
        !seq_contains_spec(
            collect_peers_spec(candidates, self_id, max_peers),
            self_id,
        ),
{
    self_excluded_build(candidates, self_id, 0, max_peers as int);
}

proof fn self_excluded_build(
    candidates: Seq<u64>,
    self_id: u64,
    from: int,
    remaining: int,
)
    requires 0 <= from, remaining >= 0,
    ensures
        !seq_contains_spec(
            collect_build_spec(candidates, self_id, from, remaining),
            self_id,
        ),
    decreases candidates.len() - from,
{
    let result = collect_build_spec(candidates, self_id, from, remaining);
    if from >= candidates.len() || remaining <= 0 {
        assert(result =~= seq![]);
    } else {
        let c = candidates[from];
        if c == self_id {
            self_excluded_build(candidates, self_id, from + 1, remaining);
        } else {
            self_excluded_build(candidates, self_id, from + 1, remaining - 1);
            let rest = collect_build_spec(candidates, self_id, from + 1, remaining - 1);
            assert(result =~= seq![c] + rest);
            // c != self_id, rest doesn't contain self_id
            assert forall|i: int| #![trigger result[i]]
                0 <= i < result.len() implies result[i] != self_id
            by {
                if i == 0 {
                    assert(result[0] == c);
                } else {
                    assert(result[i] == rest[i - 1]);
                }
            };
        }
    }
}

// ---------------------------------------------------------------------------
// Property: non-empty guarantee
// ---------------------------------------------------------------------------

// r[verify bounded-peers.non-empty]
/// If at least one non-self candidate exists and max_peers > 0,
/// the result has at least one element.
pub proof fn collect_non_empty(
    candidates: Seq<u64>,
    self_id: u64,
    max_peers: u64,
)
    requires
        max_peers > 0,
        exists|k: int| #![trigger candidates[k]]
            0 <= k < candidates.len() && candidates[k] != self_id,
    ensures
        collect_peers_spec(candidates, self_id, max_peers).len() >= 1,
{
    non_empty_build(candidates, self_id, 0, max_peers as int);
}

proof fn non_empty_build(
    candidates: Seq<u64>,
    self_id: u64,
    from: int,
    remaining: int,
)
    requires
        0 <= from,
        remaining > 0,
        exists|k: int| #![trigger candidates[k]]
            from <= k < candidates.len() && candidates[k] != self_id,
    ensures
        collect_build_spec(candidates, self_id, from, remaining).len() >= 1,
    decreases candidates.len() - from,
{
    let result = collect_build_spec(candidates, self_id, from, remaining);
    let c = candidates[from];
    if c == self_id {
        // Witness still exists in from+1..
        let witness = choose|k: int| #![trigger candidates[k]]
            from <= k < candidates.len() && candidates[k] != self_id;
        if witness == from {
            // contradiction: c == self_id but candidates[from] != self_id
        } else {
            assert(from + 1 <= witness);
            non_empty_build(candidates, self_id, from + 1, remaining);
        }
    } else {
        // c != self_id, so c is added to result
        let rest = collect_build_spec(candidates, self_id, from + 1, remaining - 1);
        assert(result =~= seq![c] + rest);
    }
}

// ---------------------------------------------------------------------------
// Property: no duplicates
// ---------------------------------------------------------------------------

// r[verify bounded-peers.no-duplicates]
/// Result has no duplicates when candidates have no duplicates.
pub proof fn collect_no_duplicates(
    candidates: Seq<u64>,
    self_id: u64,
    max_peers: u64,
)
    requires all_distinct_spec(candidates),
    ensures all_distinct_spec(collect_peers_spec(candidates, self_id, max_peers)),
{
    no_dups_build(candidates, self_id, 0, max_peers as int);
}

proof fn no_dups_build(
    candidates: Seq<u64>,
    self_id: u64,
    from: int,
    remaining: int,
)
    requires
        0 <= from,
        remaining >= 0,
        all_distinct_spec(candidates),
    ensures
        all_distinct_spec(collect_build_spec(candidates, self_id, from, remaining)),
    decreases candidates.len() - from,
{
    let result = collect_build_spec(candidates, self_id, from, remaining);
    if from >= candidates.len() || remaining <= 0 {
        // empty seq is distinct
    } else {
        let c = candidates[from];
        if c == self_id {
            no_dups_build(candidates, self_id, from + 1, remaining);
        } else {
            no_dups_build(candidates, self_id, from + 1, remaining - 1);
            let rest = collect_build_spec(candidates, self_id, from + 1, remaining - 1);
            assert(result =~= seq![c] + rest);
            // c not in rest: rest only contains candidates from from+1..,
            // candidates distinct, so c != any of them
            assert(!seq_contains_spec(rest, c)) by {
                if seq_contains_spec(rest, c) {
                    let j = choose|j: int| #![trigger rest[j]]
                        0 <= j < rest.len() && rest[j] == c;
                    build_element_source(candidates, self_id, from + 1, remaining - 1, j);
                }
            };
            assert forall|i: int, j: int|
                #![trigger result[i], result[j]]
                0 <= i < result.len() && 0 <= j < result.len() && i != j
            implies result[i] != result[j]
            by {
                if i == 0 && j > 0 {
                    assert(result[0] == c);
                    assert(result[j] == rest[j - 1]);
                } else if j == 0 && i > 0 {
                    assert(result[0] == c);
                    assert(result[i] == rest[i - 1]);
                } else if i > 0 && j > 0 {
                    assert(result[i] == rest[i - 1]);
                    assert(result[j] == rest[j - 1]);
                }
            };
        }
    }
}

/// Each element in the build result comes from candidates[from..].
proof fn build_element_source(
    candidates: Seq<u64>,
    self_id: u64,
    from: int,
    remaining: int,
    j: int,
)
    requires
        0 <= from,
        remaining >= 0,
        0 <= j < collect_build_spec(candidates, self_id, from, remaining).len(),
    ensures
        ({
            let elt = collect_build_spec(candidates, self_id, from, remaining)[j];
            exists|m: int| #![trigger candidates[m]]
                from <= m < candidates.len() && candidates[m] == elt
        }),
    decreases candidates.len() - from,
{
    let result = collect_build_spec(candidates, self_id, from, remaining);
    if from >= candidates.len() || remaining <= 0 {
        // result is empty, j < 0 impossible
    } else {
        let c = candidates[from];
        if c == self_id {
            build_element_source(candidates, self_id, from + 1, remaining, j);
        } else {
            let rest = collect_build_spec(candidates, self_id, from + 1, remaining - 1);
            assert(result =~= seq![c] + rest);
            if j == 0 {
                assert(result[0] == c);
                assert(candidates[from] == c);
            } else {
                assert(result[j] == rest[j - 1]);
                build_element_source(candidates, self_id, from + 1, remaining - 1, j - 1);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Property: fill monotonic (in-order)
// ---------------------------------------------------------------------------

// r[verify bounded-peers.collect]
/// Elements appear in the same relative order as in candidates.
/// Specifically, result is a subsequence of candidates (minus self_id).
pub proof fn collect_preserves_order(
    candidates: Seq<u64>,
    self_id: u64,
    max_peers: u64,
)
    ensures
        // Every element in result came from candidates in left-to-right order
        forall|j: int|
            #![trigger collect_peers_spec(candidates, self_id, max_peers)[j]]
            0 <= j < collect_peers_spec(candidates, self_id, max_peers).len()
            ==> exists|m: int| #![trigger candidates[m]]
                0 <= m < candidates.len()
                && candidates[m] == collect_peers_spec(candidates, self_id, max_peers)[j],
{
    let result = collect_peers_spec(candidates, self_id, max_peers);
    assert forall|j: int|
        #![trigger result[j]]
        0 <= j < result.len()
    implies
        exists|m: int| #![trigger candidates[m]]
            0 <= m < candidates.len() && candidates[m] == result[j]
    by {
        build_element_source(candidates, self_id, 0, max_peers as int, j);
    };
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify bounded-peers.collect]
    #[test]
    fn test_collect_basic() {
        let candidates = vec![1, 2, 3, 4, 5];
        let result = collect_peers(&candidates, 99, 16);
        assert_eq!(result, vec![1, 2, 3, 4, 5]);
    }

    // r[verify bounded-peers.bounded]
    #[test]
    fn test_collect_bounded() {
        let candidates = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let result = collect_peers(&candidates, 99, 3);
        assert_eq!(result, vec![1, 2, 3]);
    }

    // r[verify bounded-peers.self-excluded]
    #[test]
    fn test_collect_self_excluded() {
        let candidates = vec![1, 2, 3, 4, 5];
        let result = collect_peers(&candidates, 3, 16);
        assert_eq!(result, vec![1, 2, 4, 5]);
        assert!(!result.contains(&3));
    }

    // r[verify bounded-peers.self-excluded]
    #[test]
    fn test_collect_self_only() {
        let candidates = vec![42];
        let result = collect_peers(&candidates, 42, 16);
        assert!(result.is_empty());
    }

    // r[verify bounded-peers.non-empty]
    #[test]
    fn test_collect_non_empty() {
        let candidates = vec![42, 1];
        let result = collect_peers(&candidates, 42, 16);
        assert_eq!(result, vec![1]);
    }

    // r[verify bounded-peers.bounded]
    #[test]
    fn test_collect_zero_max() {
        let candidates = vec![1, 2, 3];
        let result = collect_peers(&candidates, 99, 0);
        assert!(result.is_empty());
    }

    // r[verify bounded-peers.collect]
    #[test]
    fn test_collect_empty_candidates() {
        let candidates: Vec<u64> = vec![];
        let result = collect_peers(&candidates, 99, 16);
        assert!(result.is_empty());
    }

    // r[verify bounded-peers.bounded]
    // r[verify bounded-peers.self-excluded]
    #[test]
    fn test_collect_self_affects_count() {
        // Self is at position 2, so we collect [1, 2, 4] (3 peers), not [1, 2] (skip self + cap)
        let candidates = vec![1, 2, 3, 4, 5];
        let result = collect_peers(&candidates, 3, 3);
        assert_eq!(result, vec![1, 2, 4]);
    }

    proptest! {
        // r[verify bounded-peers.bounded]
        #[test]
        fn prop_bounded(
            candidates in proptest::collection::vec(0u64..100, 0..30),
            self_id in 0u64..100,
            max_peers in 0u64..20,
        ) {
            let result = collect_peers(&candidates, self_id, max_peers);
            prop_assert!(result.len() as u64 <= max_peers,
                "len {} > max {}", result.len(), max_peers);
        }

        // r[verify bounded-peers.self-excluded]
        #[test]
        fn prop_self_excluded(
            candidates in proptest::collection::vec(0u64..100, 0..30),
            self_id in 0u64..100,
            max_peers in 0u64..20,
        ) {
            let result = collect_peers(&candidates, self_id, max_peers);
            prop_assert!(!result.contains(&self_id),
                "self {} in {:?}", self_id, result);
        }

        // r[verify bounded-peers.non-empty]
        #[test]
        fn prop_non_empty_guarantee(
            candidates in proptest::collection::vec(0u64..100, 1..30),
            self_id in 0u64..100,
            max_peers in 1u64..20,
        ) {
            let has_non_self = candidates.iter().any(|&c| c != self_id);
            let result = collect_peers(&candidates, self_id, max_peers);
            if has_non_self {
                prop_assert!(!result.is_empty(),
                    "empty despite non-self candidates");
            }
        }

        // r[verify bounded-peers.no-duplicates]
        #[test]
        fn prop_no_duplicates(
            candidates in proptest::collection::vec(0u64..100, 0..30),
            self_id in 0u64..100,
            max_peers in 0u64..20,
        ) {
            let mut deduped: Vec<u64> = Vec::new();
            for &c in &candidates {
                if !deduped.contains(&c) {
                    deduped.push(c);
                }
            }
            let result = collect_peers(&deduped, self_id, max_peers);
            for i in 0..result.len() {
                for j in (i + 1)..result.len() {
                    prop_assert!(result[i] != result[j],
                        "dup at {},{} in {:?}", i, j, result);
                }
            }
        }

        // r[verify bounded-peers.collect]
        #[test]
        fn prop_order_preserved(
            candidates in proptest::collection::vec(0u64..1000, 0..30),
            self_id in 0u64..1000,
            max_peers in 0u64..20,
        ) {
            let mut deduped: Vec<u64> = Vec::new();
            for &c in &candidates {
                if !deduped.contains(&c) {
                    deduped.push(c);
                }
            }
            let result = collect_peers(&deduped, self_id, max_peers);
            // Result must be a subsequence of deduped (minus self_id)
            let filtered: Vec<u64> = deduped.iter()
                .filter(|&&c| c != self_id)
                .copied()
                .collect();
            let mut fi = 0;
            for &r in &result {
                while fi < filtered.len() && filtered[fi] != r {
                    fi += 1;
                }
                prop_assert!(fi < filtered.len(),
                    "order violated: {:?} not subsequence of {:?}", result, filtered);
                fi += 1;
            }
        }
    }
}
