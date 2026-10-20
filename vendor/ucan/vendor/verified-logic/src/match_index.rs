//! Verified match index and next index tracking for Raft log replication.
//!
//! A leader maintains two parallel arrays indexed by server ID:
//! - `matchIndex[i]`: the highest log index known to be replicated on server i (monotonically
//!   non-decreasing)
//! - `nextIndex[i]`: the next log index to send to server i (decremented on rejection, floor at 1)
//!
//! These are the bookkeeping that drives commit advancement.

use vstd::prelude::*;

verus! {

// --- init_match_indices ---

// r[depends match-index.init]
/// Spec: initialize matchIndex to all zeros.
pub open spec fn init_match_indices_spec(cluster_size: int) -> Seq<u64> {
    Seq::new(cluster_size as nat, |_i: int| 0u64)
}

// r[impl match-index.init]
/// Create matchIndex array: all zeros.
pub fn init_match_indices(cluster_size: usize) -> (result: Vec<u64>)
    ensures result@ == init_match_indices_spec(cluster_size as int)
{
    let mut v: Vec<u64> = Vec::new();
    let mut i: usize = 0;
    while i < cluster_size
        invariant
            0 <= i <= cluster_size,
            v@.len() == i as int,
            v@ == Seq::new(i as nat, |_j: int| 0u64),
        decreases cluster_size - i,
    {
        v.push(0);
        assert(v@ =~= Seq::new((i + 1) as nat, |_j: int| 0u64));
        i = i + 1;
    }
    v
}

// --- init_next_indices ---

// r[depends match-index.init]
/// Spec: initialize nextIndex to last_log_index + 1.
pub open spec fn init_next_indices_spec(cluster_size: int, last_log_index: u64) -> Seq<u64> {
    Seq::new(cluster_size as nat, |_i: int| (last_log_index + 1) as u64)
}

// r[impl match-index.init]
/// Create nextIndex array: all set to last_log_index + 1.
pub fn init_next_indices(cluster_size: usize, last_log_index: u64) -> (result: Vec<u64>)
    requires last_log_index < u64::MAX
    ensures result@ == init_next_indices_spec(cluster_size as int, last_log_index)
{
    let val = last_log_index + 1;
    let mut v: Vec<u64> = Vec::new();
    let mut i: usize = 0;
    while i < cluster_size
        invariant
            0 <= i <= cluster_size,
            val == (last_log_index + 1) as u64,
            v@.len() == i as int,
            v@ == Seq::new(i as nat, |_j: int| val),
        decreases cluster_size - i,
    {
        v.push(val);
        assert(v@ =~= Seq::new((i + 1) as nat, |_j: int| val));
        i = i + 1;
    }
    v
}

// --- update_match_index ---

// r[depends match-index.update]
/// Spec: set matches[server] to new_match.
pub open spec fn update_match_index_spec(
    matches: Seq<u64>, server: int, new_match: u64,
) -> Seq<u64>
    recommends 0 <= server < matches.len()
{
    matches.update(server, new_match)
}

// r[impl match-index.update]
/// Advance matchIndex for a server. Requires monotonicity: new_match >= current.
pub fn update_match_index(
    matches: &mut Vec<u64>,
    server: usize,
    new_match: u64,
)
    requires
        server < old(matches)@.len(),
        new_match >= old(matches)@[server as int],
    ensures
        matches@ == update_match_index_spec(old(matches)@, server as int, new_match),
        matches@.len() == old(matches)@.len(),
        matches@[server as int] == new_match,
        forall|j: int| #![trigger matches@[j]]
            0 <= j < matches@.len() && j != server as int
            ==> matches@[j] == old(matches)@[j],
{
    matches.set(server, new_match);
}

// --- backtrack_next_index ---

// r[depends match-index.backtrack]
/// Spec: decrement nexts[server] by 1, with floor at 1.
pub open spec fn backtrack_next_index_spec(
    nexts: Seq<u64>, server: int,
) -> Seq<u64>
    recommends 0 <= server < nexts.len()
{
    let old_val = nexts[server];
    let new_val: u64 = if old_val <= 1 { 1 } else { (old_val - 1) as u64 };
    nexts.update(server, new_val)
}

// r[impl match-index.backtrack]
/// Decrement nextIndex for a server, floored at 1.
pub fn backtrack_next_index(
    nexts: &mut Vec<u64>,
    server: usize,
)
    requires server < old(nexts)@.len()
    ensures
        nexts@ == backtrack_next_index_spec(old(nexts)@, server as int),
        nexts@.len() == old(nexts)@.len(),
        nexts@[server as int] >= 1,
        forall|j: int| #![trigger nexts@[j]]
            0 <= j < nexts@.len() && j != server as int
            ==> nexts@[j] == old(nexts)@[j],
{
    let old_val = nexts[server];
    if old_val <= 1 {
        nexts.set(server, 1);
    } else {
        nexts.set(server, old_val - 1);
    }
}

// --- count_replicated ---

// r[depends match-index.count-replicated]
/// Spec: count servers with matchIndex >= index.
pub open spec fn count_replicated_spec(matches: Seq<u64>, index: u64) -> int {
    count_replicated_from_spec(matches, index, 0)
}

pub open spec fn count_replicated_from_spec(
    matches: Seq<u64>, index: u64, from: int,
) -> int
    decreases matches.len() - from,
{
    if from >= matches.len() {
        0int
    } else {
        let inc: int = if matches[from] >= index { 1 } else { 0 };
        inc + count_replicated_from_spec(matches, index, from + 1)
    }
}

// r[impl match-index.count-replicated]
/// Count how many servers have matchIndex >= index.
pub fn count_replicated(matches: &[u64], index: u64) -> (result: u64)
    requires matches@.len() <= u64::MAX as int
    ensures result as int == count_replicated_spec(matches@, index)
{
    let mut count: u64 = 0;
    let mut i: usize = 0;
    while i < matches.len()
        invariant
            0 <= i <= matches@.len(),
            matches@.len() <= u64::MAX as int,
            count as int == count_replicated_spec(matches@, index)
                - count_replicated_from_spec(matches@, index, i as int),
            count as int >= 0,
            count as int <= i as int,
        decreases matches@.len() - i,
    {
        if matches[i] >= index {
            count = count + 1;
        }
        i = i + 1;
    }
    count
}

// --- Safety proofs ---

// r[verify match-index.init]
/// All matchIndex entries initialized to zero.
proof fn init_match_all_zero(cluster_size: int)
    requires cluster_size >= 0
    ensures
        forall|i: int| #![trigger init_match_indices_spec(cluster_size)[i]]
            0 <= i < cluster_size
            ==> init_match_indices_spec(cluster_size)[i] == 0
{}

// r[verify match-index.init]
/// All nextIndex entries initialized to last_log_index + 1.
proof fn init_next_all_correct(cluster_size: int, last_log_index: u64)
    requires cluster_size >= 0, last_log_index < u64::MAX
    ensures
        forall|i: int| #![trigger init_next_indices_spec(cluster_size, last_log_index)[i]]
            0 <= i < cluster_size
            ==> init_next_indices_spec(cluster_size, last_log_index)[i]
                == (last_log_index + 1) as u64
{}

// r[verify match-index.update]
/// Update isolation: only the target server's index changes.
proof fn update_isolates(matches: Seq<u64>, server: int, new_match: u64, j: int)
    requires
        0 <= server < matches.len(),
        0 <= j < matches.len(),
        j != server,
    ensures
        update_match_index_spec(matches, server, new_match)[j] == matches[j]
{}

// r[verify match-index.update]
/// Update sets the correct value.
proof fn update_correct(matches: Seq<u64>, server: int, new_match: u64)
    requires 0 <= server < matches.len()
    ensures update_match_index_spec(matches, server, new_match)[server] == new_match
{}

// r[verify match-index.update]
/// Update preserves length.
proof fn update_preserves_length(matches: Seq<u64>, server: int, new_match: u64)
    requires 0 <= server < matches.len()
    ensures update_match_index_spec(matches, server, new_match).len() == matches.len()
{}

// r[verify match-index.backtrack]
/// Backtrack floor: nextIndex never goes below 1.
proof fn backtrack_floor(nexts: Seq<u64>, server: int)
    requires 0 <= server < nexts.len()
    ensures backtrack_next_index_spec(nexts, server)[server] >= 1
{}

// r[verify match-index.backtrack]
/// Backtrack isolation: other servers unchanged.
proof fn backtrack_isolates(nexts: Seq<u64>, server: int, j: int)
    requires
        0 <= server < nexts.len(),
        0 <= j < nexts.len(),
        j != server,
    ensures
        backtrack_next_index_spec(nexts, server)[j] == nexts[j]
{}

// r[verify match-index.backtrack]
/// Backtrack decrements by at most 1.
proof fn backtrack_decrements_by_one(nexts: Seq<u64>, server: int)
    requires
        0 <= server < nexts.len(),
        nexts[server] >= 1,
    ensures
        (backtrack_next_index_spec(nexts, server)[server] as int)
            >= (nexts[server] as int) - 1,
        (backtrack_next_index_spec(nexts, server)[server] as int)
            <= (nexts[server] as int),
{}

// r[verify match-index.count-replicated]
/// Count is non-negative.
proof fn count_replicated_nonneg(matches: Seq<u64>, index: u64)
    ensures count_replicated_spec(matches, index) >= 0
{
    count_replicated_nonneg_from(matches, index, 0);
}

proof fn count_replicated_nonneg_from(matches: Seq<u64>, index: u64, from: int)
    requires 0 <= from
    ensures count_replicated_from_spec(matches, index, from) >= 0
    decreases matches.len() - from,
{
    if from >= matches.len() {
    } else {
        count_replicated_nonneg_from(matches, index, from + 1);
    }
}

// r[verify match-index.count-replicated]
/// Count is bounded by array length.
proof fn count_replicated_bounded(matches: Seq<u64>, index: u64)
    ensures count_replicated_spec(matches, index) <= matches.len() as int
{
    count_replicated_bounded_from(matches, index, 0);
}

proof fn count_replicated_bounded_from(matches: Seq<u64>, index: u64, from: int)
    requires 0 <= from
    ensures
        from <= matches.len() ==> count_replicated_from_spec(matches, index, from)
            <= (matches.len() as int) - from,
        from > matches.len() ==> count_replicated_from_spec(matches, index, from) == 0,
    decreases matches.len() - from,
{
    if from >= matches.len() {
    } else {
        count_replicated_bounded_from(matches, index, from + 1);
    }
}

} // verus!

#[cfg(test)]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::default_numeric_fallback
)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify match-index.init]
    #[test]
    fn test_init_match_indices() {
        let m = init_match_indices(5);
        assert_eq!(m.len(), 5);
        for &v in &m {
            assert_eq!(v, 0);
        }
    }

    // r[verify match-index.init]
    #[test]
    fn test_init_next_indices() {
        let n = init_next_indices(5, 10);
        assert_eq!(n.len(), 5);
        for &v in &n {
            assert_eq!(v, 11);
        }
    }

    // r[verify match-index.update]
    #[test]
    fn test_update_match_index() {
        let mut m = vec![0, 0, 0, 0, 0];
        update_match_index(&mut m, 2, 5);
        assert_eq!(m, vec![0, 0, 5, 0, 0]);
    }

    // r[verify match-index.backtrack]
    #[test]
    fn test_backtrack_next_index() {
        let mut n = vec![5, 5, 5, 5, 5];
        backtrack_next_index(&mut n, 2);
        assert_eq!(n, vec![5, 5, 4, 5, 5]);
    }

    // r[verify match-index.backtrack]
    #[test]
    fn test_backtrack_floor() {
        let mut n = vec![5, 5, 1, 5, 5];
        backtrack_next_index(&mut n, 2);
        assert_eq!(n, vec![5, 5, 1, 5, 5]); // floor at 1
    }

    // r[verify match-index.count-replicated]
    #[test]
    fn test_count_replicated() {
        let m = vec![5, 3, 5, 2, 5];
        assert_eq!(count_replicated(&m, 5), 3); // indices 0, 2, 4
        assert_eq!(count_replicated(&m, 3), 4); // indices 0, 1, 2, 4
        assert_eq!(count_replicated(&m, 1), 5); // all
        assert_eq!(count_replicated(&m, 6), 0); // none
    }

    proptest! {
        // r[verify match-index.init]
        #[test]
        fn prop_init_match_all_zero(size in 0usize..100) {
            let m = init_match_indices(size);
            prop_assert_eq!(m.len(), size);
            for &v in &m {
                prop_assert_eq!(v, 0);
            }
        }

        // r[verify match-index.update]
        #[test]
        fn prop_update_isolation(
            size in 2usize..20,
            server_frac in 0.0f64..1.0,
            new_match: u64,
        ) {
            let server = (server_frac * (size - 1) as f64) as usize;
            let mut m = init_match_indices(size);
            update_match_index(&mut m, server, new_match);
            prop_assert_eq!(m[server], new_match);
            for (i, &v) in m.iter().enumerate() {
                if i != server {
                    prop_assert_eq!(v, 0, "other index {} changed", i);
                }
            }
        }

        // r[verify match-index.backtrack]
        #[test]
        fn prop_backtrack_floor_at_1(
            vals in proptest::collection::vec(1u64..100, 1..=20),
            server_frac in 0.0f64..1.0,
        ) {
            let server = (server_frac * (vals.len() - 1) as f64) as usize;
            let mut n = vals.clone();
            backtrack_next_index(&mut n, server);
            prop_assert!(n[server] >= 1);
            prop_assert!(n[server] <= vals[server]);
            for (i, &v) in n.iter().enumerate() {
                if i != server {
                    prop_assert_eq!(v, vals[i]);
                }
            }
        }

        // r[verify match-index.count-replicated]
        #[test]
        fn prop_count_bounded(
            matches in proptest::collection::vec(0u64..100, 0..=20),
            index: u64,
        ) {
            let count = count_replicated(&matches, index);
            prop_assert!(count as usize <= matches.len());
        }

        // r[verify match-index.count-replicated]
        #[test]
        fn prop_count_oracle(
            matches in proptest::collection::vec(0u64..100, 0..=20),
            index in 0u64..100,
        ) {
            let count = count_replicated(&matches, index);
            let expected = matches.iter().filter(|&&m| m >= index).count() as u64;
            prop_assert_eq!(count, expected);
        }

        // r[verify match-index.update]
        // r[verify match-index.backtrack]
        /// Simulate ack/reject sequence on a 5-node cluster.
        #[test]
        fn prop_leader_tracking_model(
            ops in proptest::collection::vec(
                (0usize..5, prop_oneof![
                    (1u64..50).prop_map(|m| (true, m)),
                    Just((false, 0u64)),
                ]),
                0..=50,
            ),
        ) {
            let mut match_idx = init_match_indices(5);
            let mut next_idx = init_next_indices(5, 20);

            for &(server, (is_ack, match_val)) in &ops {
                if is_ack {
                    // Only update if monotonic
                    if match_val >= match_idx[server] {
                        update_match_index(&mut match_idx, server, match_val);
                        // After ack, set nextIndex = matchIndex + 1
                        if match_val < u64::MAX {
                            next_idx[server] = match_val + 1;
                        }
                    }
                } else {
                    backtrack_next_index(&mut next_idx, server);
                }
            }

            // Invariants hold after all operations
            for i in 0..5 {
                prop_assert!(next_idx[i] >= 1, "nextIndex[{}] < 1", i);
            }
        }
    }
}
