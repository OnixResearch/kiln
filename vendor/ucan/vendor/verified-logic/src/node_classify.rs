// MAX_NODES is a small constant (1024), safe to cast u64→usize
#![allow(clippy::cast_possible_truncation)]

//! Verified node classification for cluster state reporting.
//!
//! Given a set of member IDs and an optional leader ID, classify each
//! node as voter/learner and leader/follower. Mirrors the classification
//! logic from `aspen-cluster-handler`'s state reporting.
//!
//! Properties: voter iff member, learner iff not voter, exclusive roles,
//! leader implies voter, at most one leader, bounded output, no duplicates.

use vstd::prelude::*;

verus! {

// ---------------------------------------------------------------------------
// Single-node classification
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

// r[depends node-classify.voter-equivalence]
// r[depends node-classify.leader-is-voter]
/// Spec: classify a single node.
/// Returns (is_voter, is_learner, is_leader).
///
/// - is_voter: node_id ∈ member_ids
/// - is_learner: ¬is_voter
/// - is_leader: is_voter AND node_id == leader_id (when Some)
pub open spec fn classify_node_spec(
    node_id: u64,
    member_ids: Seq<u64>,
    leader_id: Option<u64>,
) -> (bool, bool, bool) {
    let is_voter = seq_contains_spec(member_ids, node_id);
    let is_learner = !is_voter;
    let is_leader = is_voter && match leader_id {
        Some(lid) => node_id == lid,
        None => false,
    };
    (is_voter, is_learner, is_leader)
}

// r[impl node-classify.voter-equivalence]
// r[impl node-classify.leader-is-voter]
/// Classify a single node given member set and leader.
pub fn classify_node(
    node_id: u64,
    member_ids: &[u64],
    leader_id: Option<u64>,
) -> (result: (bool, bool, bool))
    ensures result == classify_node_spec(node_id, member_ids@, leader_id),
{
    let mut is_voter = false;
    let mut i: usize = 0;
    while i < member_ids.len()
        invariant
            0 <= i <= member_ids@.len(),
            is_voter == exists|j: int| #![trigger member_ids@[j]] 0 <= j < i as int && member_ids@[j] == node_id,
        decreases member_ids@.len() - i,
    {
        if member_ids[i] == node_id {
            is_voter = true;
        }
        i = i + 1;
    }
    let is_learner = !is_voter;
    let is_leader = is_voter && match leader_id {
        Some(lid) => node_id == lid,
        None => false,
    };
    (is_voter, is_learner, is_leader)
}

// ---------------------------------------------------------------------------
// Batch classification
// ---------------------------------------------------------------------------

/// Max nodes for bounded output.
pub const MAX_NODES: u64 = 100;

// r[depends node-classify.bounded-output]
// r[depends node-classify.no-duplicates]
/// Spec: classify a batch of nodes. Truncates at MAX_NODES.
/// Returns Seq of (node_id, is_voter, is_learner, is_leader).
pub open spec fn classify_nodes_spec(
    node_ids: Seq<u64>,
    member_ids: Seq<u64>,
    leader_id: Option<u64>,
) -> Seq<(u64, bool, bool, bool)> {
    let len: int = if node_ids.len() <= MAX_NODES as int {
        node_ids.len() as int
    } else {
        MAX_NODES as int
    };
    Seq::new(len as nat, |i: int| {
        let nid = node_ids[i];
        let (v, l, ld) = classify_node_spec(nid, member_ids, leader_id);
        (nid, v, l, ld)
    })
}

// r[impl node-classify.bounded-output]
// r[impl node-classify.no-duplicates]
/// Classify a batch of nodes. Truncates at MAX_NODES.
pub fn classify_nodes(
    node_ids: &[u64],
    member_ids: &[u64],
    leader_id: Option<u64>,
) -> (result: Vec<(u64, bool, bool, bool)>)
    ensures result@ == classify_nodes_spec(node_ids@, member_ids@, leader_id),
{
    let limit: usize = if node_ids.len() <= MAX_NODES as usize {
        node_ids.len()
    } else {
        MAX_NODES as usize
    };
    let mut result: Vec<(u64, bool, bool, bool)> = Vec::new();
    let mut i: usize = 0;
    while i < limit
        invariant
            0 <= i <= limit,
            limit as int == if node_ids@.len() <= MAX_NODES as int { node_ids@.len() as int } else { MAX_NODES as int },
            result@.len() == i as int,
            forall|j: int| #![trigger result@[j]] 0 <= j < i as int ==> ({
                let nid = node_ids@[j];
                let (v, l, ld) = classify_node_spec(nid, member_ids@, leader_id);
                result@[j] == (nid, v, l, ld)
            }),
        decreases limit - i,
    {
        let nid = node_ids[i];
        let (v, l, ld) = classify_node(nid, member_ids, leader_id);
        result.push((nid, v, l, ld));
        i = i + 1;
    }
    assert(result@ =~= classify_nodes_spec(node_ids@, member_ids@, leader_id));
    result
}

// ---------------------------------------------------------------------------
// Property proofs
// ---------------------------------------------------------------------------

// r[verify node-classify.voter-equivalence]
/// is_voter ⟺ node_id ∈ member_ids.
pub proof fn voter_iff_member(
    node_id: u64,
    member_ids: Seq<u64>,
    leader_id: Option<u64>,
)
    ensures
        ({
            let (is_voter, _, _) = classify_node_spec(node_id, member_ids, leader_id);
            is_voter == seq_contains_spec(member_ids, node_id)
        }),
{}

// r[verify node-classify.voter-equivalence]
/// is_learner ⟺ ¬is_voter.
pub proof fn learner_iff_not_voter(
    node_id: u64,
    member_ids: Seq<u64>,
    leader_id: Option<u64>,
)
    ensures
        ({
            let (is_voter, is_learner, _) = classify_node_spec(node_id, member_ids, leader_id);
            is_learner == !is_voter
        }),
{}

// r[verify node-classify.voter-equivalence]
/// is_voter and is_learner are mutually exclusive and exhaustive.
pub proof fn voter_learner_exclusive(
    node_id: u64,
    member_ids: Seq<u64>,
    leader_id: Option<u64>,
)
    ensures
        ({
            let (is_voter, is_learner, _) = classify_node_spec(node_id, member_ids, leader_id);
            (is_voter || is_learner) && !(is_voter && is_learner)
        }),
{}

// r[verify node-classify.leader-is-voter]
/// is_leader implies is_voter.
pub proof fn leader_implies_voter(
    node_id: u64,
    member_ids: Seq<u64>,
    leader_id: Option<u64>,
)
    ensures
        ({
            let (is_voter, _, is_leader) = classify_node_spec(node_id, member_ids, leader_id);
            is_leader ==> is_voter
        }),
{}

// r[verify node-classify.leader-is-voter]
/// A learner cannot be leader.
pub proof fn learner_not_leader(
    node_id: u64,
    member_ids: Seq<u64>,
    leader_id: Option<u64>,
)
    ensures
        ({
            let (_, is_learner, is_leader) = classify_node_spec(node_id, member_ids, leader_id);
            is_learner ==> !is_leader
        }),
{}

// r[impl node-classify.at-most-one-leader]
// r[verify node-classify.at-most-one-leader]
/// At most one node is classified as leader.
pub proof fn at_most_one_leader(
    node_ids: Seq<u64>,
    member_ids: Seq<u64>,
    leader_id: Option<u64>,
)
    requires all_distinct_spec(node_ids),
    ensures
        ({
            let result = classify_nodes_spec(node_ids, member_ids, leader_id);
            forall|i: int, j: int|
                #![trigger result[i], result[j]]
                0 <= i < result.len() && 0 <= j < result.len() && i != j
                ==> !(result[i].3 && result[j].3)
        }),
{
    let result = classify_nodes_spec(node_ids, member_ids, leader_id);
    assert forall|i: int, j: int|
        #![trigger result[i], result[j]]
        0 <= i < result.len() && 0 <= j < result.len() && i != j
    implies !(result[i].3 && result[j].3)
    by {
        // result[i].3 = is_leader for node_ids[i]
        // is_leader requires node_id == leader_id
        // node_ids[i] != node_ids[j] (distinct)
        // So at most one can equal leader_id
        let len: int = if node_ids.len() <= MAX_NODES as int {
            node_ids.len() as int
        } else {
            MAX_NODES as int
        };
        assert(i < len && j < len);
        assert(node_ids[i] != node_ids[j]);
    };
}

// r[verify node-classify.at-most-one-leader]
/// When leader_id is None, no node is classified as leader.
pub proof fn no_leader_when_none(
    node_ids: Seq<u64>,
    member_ids: Seq<u64>,
)
    ensures
        ({
            let result = classify_nodes_spec(node_ids, member_ids, None);
            forall|i: int|
                #![trigger result[i]]
                0 <= i < result.len() ==> !result[i].3
        }),
{}

// r[verify node-classify.bounded-output]
/// Output length is bounded by MAX_NODES.
pub proof fn output_bounded(
    node_ids: Seq<u64>,
    member_ids: Seq<u64>,
    leader_id: Option<u64>,
)
    ensures
        classify_nodes_spec(node_ids, member_ids, leader_id).len()
        <= MAX_NODES as int,
{}

// r[verify node-classify.no-duplicates]
/// Output has no duplicate node_ids when input has no duplicates.
pub proof fn output_no_duplicate_ids(
    node_ids: Seq<u64>,
    member_ids: Seq<u64>,
    leader_id: Option<u64>,
)
    requires all_distinct_spec(node_ids),
    ensures
        ({
            let result = classify_nodes_spec(node_ids, member_ids, leader_id);
            forall|i: int, j: int|
                #![trigger result[i], result[j]]
                0 <= i < result.len() && 0 <= j < result.len() && i != j
                ==> result[i].0 != result[j].0
        }),
{
    let result = classify_nodes_spec(node_ids, member_ids, leader_id);
    assert forall|i: int, j: int|
        #![trigger result[i], result[j]]
        0 <= i < result.len() && 0 <= j < result.len() && i != j
    implies result[i].0 != result[j].0
    by {
        // result[i].0 == node_ids[i], result[j].0 == node_ids[j]
        // node_ids distinct → node_ids[i] != node_ids[j]
    };
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify node-classify.voter-equivalence]
    #[test]
    fn test_classify_voter() {
        let members = vec![1, 2, 3];
        let (v, l, ld) = classify_node(2, &members, Some(2));
        assert!(v);
        assert!(!l);
        assert!(ld);
    }

    // r[verify node-classify.voter-equivalence]
    #[test]
    fn test_classify_learner() {
        let members = vec![1, 2, 3];
        let (v, l, ld) = classify_node(5, &members, Some(2));
        assert!(!v);
        assert!(l);
        assert!(!ld);
    }

    // r[verify node-classify.leader-is-voter]
    #[test]
    fn test_classify_leader_is_voter() {
        let members = vec![1, 2, 3];
        let (v, l, ld) = classify_node(2, &members, Some(2));
        assert!(v && ld && !l);
    }

    // r[verify node-classify.at-most-one-leader]
    #[test]
    fn test_classify_no_leader() {
        let members = vec![1, 2, 3];
        let (_, _, ld) = classify_node(2, &members, None);
        assert!(!ld);
    }

    // r[verify node-classify.voter-equivalence]
    #[test]
    fn test_classify_follower() {
        // Voter but not leader
        let members = vec![1, 2, 3];
        let (v, l, ld) = classify_node(1, &members, Some(2));
        assert!(v && !l && !ld);
    }

    // r[verify node-classify.bounded-output]
    #[test]
    fn test_batch_truncation() {
        let nodes: Vec<u64> = (0..150).collect();
        let members = vec![1, 2, 3];
        let result = classify_nodes(&nodes, &members, Some(2));
        assert_eq!(result.len(), 100);
    }

    // r[verify node-classify.no-duplicates]
    #[test]
    fn test_batch_ids_match_input() {
        let nodes = vec![10, 20, 30];
        let members = vec![10, 30];
        let result = classify_nodes(&nodes, &members, Some(10));
        assert_eq!(result[0], (10, true, false, true));
        assert_eq!(result[1], (20, false, true, false));
        assert_eq!(result[2], (30, true, false, false));
    }

    proptest! {
        // r[verify node-classify.voter-equivalence]
        #[test]
        fn prop_voter_iff_member(
            node_id in 0u64..100,
            members in proptest::collection::vec(0u64..100, 0..20),
        ) {
            let (v, l, _) = classify_node(node_id, &members, None);
            let in_members = members.contains(&node_id);
            prop_assert_eq!(v, in_members);
            prop_assert_eq!(l, !in_members);
        }

        // r[verify node-classify.voter-equivalence]
        #[test]
        fn prop_voter_learner_exclusive(
            node_id in 0u64..100,
            members in proptest::collection::vec(0u64..100, 0..20),
            leader_id in proptest::option::of(0u64..100),
        ) {
            let (v, l, _) = classify_node(node_id, &members, leader_id);
            prop_assert!(v ^ l, "not exclusive: v={v}, l={l}");
        }

        // r[verify node-classify.leader-is-voter]
        #[test]
        fn prop_leader_implies_voter(
            node_id in 0u64..100,
            members in proptest::collection::vec(0u64..100, 0..20),
            leader_id in proptest::option::of(0u64..100),
        ) {
            let (v, _, ld) = classify_node(node_id, &members, leader_id);
            if ld {
                prop_assert!(v, "leader but not voter");
            }
        }

        // r[verify node-classify.at-most-one-leader]
        #[test]
        fn prop_at_most_one_leader(
            nodes in proptest::collection::vec(0u64..200, 0..50),
            members in proptest::collection::vec(0u64..200, 0..20),
            leader_id in proptest::option::of(0u64..200),
        ) {
            let mut deduped: Vec<u64> = Vec::new();
            for &n in &nodes {
                if !deduped.contains(&n) { deduped.push(n); }
            }
            let result = classify_nodes(&deduped, &members, leader_id);
            let leader_count = result.iter().filter(|r| r.3).count();
            prop_assert!(leader_count <= 1,
                "multiple leaders: {leader_count}");
        }

        // r[verify node-classify.bounded-output]
        #[test]
        fn prop_bounded_output(
            nodes in proptest::collection::vec(0u64..500, 0..200),
            members in proptest::collection::vec(0u64..500, 0..20),
            leader_id in proptest::option::of(0u64..500),
        ) {
            let result = classify_nodes(&nodes, &members, leader_id);
            prop_assert!(result.len() <= 100);
        }

        // r[verify node-classify.no-duplicates]
        #[test]
        fn prop_no_duplicate_ids(
            nodes in proptest::collection::vec(0u64..200, 0..50),
            members in proptest::collection::vec(0u64..200, 0..20),
            leader_id in proptest::option::of(0u64..200),
        ) {
            let mut deduped: Vec<u64> = Vec::new();
            for &n in &nodes {
                if !deduped.contains(&n) { deduped.push(n); }
            }
            let result = classify_nodes(&deduped, &members, leader_id);
            for i in 0..result.len() {
                for j in (i + 1)..result.len() {
                    prop_assert!(result[i].0 != result[j].0,
                        "dup id at {},{}", i, j);
                }
            }
        }
    }
}
