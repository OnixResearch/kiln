//! Verified follower commit index update for Raft.
//!
//! When a follower receives a valid `AppendEntries` RPC, it updates its
//! commit index to `min(leaderCommit, index of last new entry)`. This
//! is the follower-side counterpart to `commit_advance.rs` (leader-side).
//! The leader advances via matchIndex + quorum counting; followers
//! advance via the `leaderCommit` field piggybacked on `AppendEntries`.

use vstd::prelude::*;

verus! {

// --- follower_update_commit ---

// r[depends follower-commit.update]
/// Spec: compute the follower's new commit index after AppendEntries.
///
/// Returns min(leader_commit, last_new_entry_index) when that exceeds
/// current_commit, otherwise current_commit (no regression).
pub open spec fn follower_update_commit_spec(
    current_commit: u64,
    leader_commit: u64,
    last_new_entry_index: u64,
) -> u64 {
    let candidate = if leader_commit <= last_new_entry_index {
        leader_commit
    } else {
        last_new_entry_index
    };
    if candidate > current_commit {
        candidate
    } else {
        current_commit
    }
}

// r[impl follower-commit.update]
/// Compute the follower's new commit index.
pub fn follower_update_commit(
    current_commit: u64,
    leader_commit: u64,
    last_new_entry_index: u64,
) -> (result: u64)
    ensures result == follower_update_commit_spec(current_commit, leader_commit, last_new_entry_index)
{
    let candidate = if leader_commit <= last_new_entry_index {
        leader_commit
    } else {
        last_new_entry_index
    };
    if candidate > current_commit {
        candidate
    } else {
        current_commit
    }
}

// --- Safety proofs ---

// r[impl follower-commit.monotonic]
// r[verify follower-commit.monotonic]
/// Follower commit index never decreases.
proof fn follower_commit_monotonic(
    current_commit: u64,
    leader_commit: u64,
    last_new_entry_index: u64,
)
    ensures
        follower_update_commit_spec(current_commit, leader_commit, last_new_entry_index)
            >= current_commit
{}

// r[impl follower-commit.bounded-by-leader]
// r[verify follower-commit.bounded-by-leader]
/// Follower commit index never exceeds leader's commit index.
proof fn follower_commit_bounded_by_leader(
    current_commit: u64,
    leader_commit: u64,
    last_new_entry_index: u64,
)
    ensures
        follower_update_commit_spec(current_commit, leader_commit, last_new_entry_index)
            <= leader_commit || follower_update_commit_spec(current_commit, leader_commit, last_new_entry_index) == current_commit
{}

// r[impl follower-commit.bounded-by-log]
// r[verify follower-commit.bounded-by-log]
/// Follower commit index never exceeds the last log entry.
proof fn follower_commit_bounded_by_log(
    current_commit: u64,
    leader_commit: u64,
    last_new_entry_index: u64,
)
    ensures
        follower_update_commit_spec(current_commit, leader_commit, last_new_entry_index)
            <= last_new_entry_index || follower_update_commit_spec(current_commit, leader_commit, last_new_entry_index) == current_commit
{}

// r[verify follower-commit.update]
/// When leader_commit <= current_commit, no change.
proof fn follower_commit_no_change_when_behind(
    current_commit: u64,
    leader_commit: u64,
    last_new_entry_index: u64,
)
    requires leader_commit <= current_commit
    ensures
        follower_update_commit_spec(current_commit, leader_commit, last_new_entry_index)
            == current_commit
{}

// r[verify follower-commit.update]
/// When both leader_commit and last_new_entry_index exceed current_commit,
/// the result is exactly min(leader_commit, last_new_entry_index).
proof fn follower_commit_advances(
    current_commit: u64,
    leader_commit: u64,
    last_new_entry_index: u64,
)
    requires
        leader_commit > current_commit,
        last_new_entry_index > current_commit,
    ensures
        follower_update_commit_spec(current_commit, leader_commit, last_new_entry_index)
            == if leader_commit <= last_new_entry_index { leader_commit } else { last_new_entry_index }
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify follower-commit.update]
    #[test]
    fn test_basic_follower_commit_update() {
        // Leader ahead, log covers it
        assert_eq!(follower_update_commit(0, 5, 10), 5);
        // Leader ahead, log shorter
        assert_eq!(follower_update_commit(0, 10, 5), 5);
        // Leader behind current
        assert_eq!(follower_update_commit(5, 3, 10), 5);
        // All equal
        assert_eq!(follower_update_commit(5, 5, 5), 5);
        // Current is zero
        assert_eq!(follower_update_commit(0, 0, 0), 0);
    }

    // r[verify follower-commit.monotonic]
    #[test]
    fn test_follower_commit_never_decreases() {
        assert!(follower_update_commit(5, 3, 2) >= 5);
        assert!(follower_update_commit(5, 10, 1) >= 5);
        assert!(follower_update_commit(5, 1, 10) >= 5);
    }

    // r[verify follower-commit.bounded-by-leader]
    #[test]
    fn test_follower_commit_bounded_by_leader() {
        let result = follower_update_commit(0, 5, 10);
        assert!(result <= 5);
    }

    // r[verify follower-commit.bounded-by-log]
    #[test]
    fn test_follower_commit_bounded_by_log() {
        let result = follower_update_commit(0, 10, 5);
        assert!(result <= 5);
    }

    proptest! {
        // r[verify follower-commit.monotonic]
        #[test]
        fn prop_follower_commit_monotonic(
            current in 0u64..100,
            leader in 0u64..100,
            last_entry in 0u64..100,
        ) {
            let result = follower_update_commit(current, leader, last_entry);
            prop_assert!(result >= current, "commit decreased: {} < {}", result, current);
        }

        // r[verify follower-commit.update]
        #[test]
        fn prop_follower_commit_model_oracle(
            current in 0u64..100,
            leader in 0u64..100,
            last_entry in 0u64..100,
        ) {
            let result = follower_update_commit(current, leader, last_entry);
            let expected_candidate = std::cmp::min(leader, last_entry);
            let expected = std::cmp::max(current, expected_candidate);
            // When candidate > current, we advance. When not, we stay.
            // This matches: max(current, min(leader, last_entry))
            prop_assert_eq!(result, expected);
        }

        // r[verify follower-commit.bounded-by-leader]
        // r[verify follower-commit.bounded-by-log]
        #[test]
        fn prop_follower_commit_bounded(
            current in 0u64..100,
            leader in 0u64..100,
            last_entry in 0u64..100,
        ) {
            let result = follower_update_commit(current, leader, last_entry);
            // Result is either current_commit (no change) or min(leader, last_entry)
            prop_assert!(result == current || (result <= leader && result <= last_entry));
        }

        #[test]
        fn prop_reorder_independence(
            current in 0u64..50,
            l1 in 0u64..50,
            e1 in 0u64..50,
            l2 in 0u64..50,
            e2 in 0u64..50,
        ) {
            // Apply two messages in both orders, result is the same
            let r1 = follower_update_commit(follower_update_commit(current, l1, e1), l2, e2);
            let r2 = follower_update_commit(follower_update_commit(current, l2, e2), l1, e1);
            prop_assert_eq!(r1, r2);
        }

        #[test]
        fn prop_idempotent(
            current in 0u64..100,
            leader in 0u64..100,
            last_entry in 0u64..100,
        ) {
            let once = follower_update_commit(current, leader, last_entry);
            let twice = follower_update_commit(once, leader, last_entry);
            prop_assert_eq!(once, twice);
        }

        #[test]
        fn prop_higher_leader_commit_advances(
            current in 0u64..50,
            leader1 in 0u64..50,
            leader2 in 0u64..50,
            last_entry in 50u64..100,
        ) {
            // With large log, higher leader commit means higher result
            let r1 = follower_update_commit(current, leader1, last_entry);
            let r2 = follower_update_commit(current, leader2, last_entry);
            if leader1 <= leader2 {
                prop_assert!(r1 <= r2);
            }
        }
    }
}
