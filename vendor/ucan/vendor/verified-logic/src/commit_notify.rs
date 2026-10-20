//! Verified commit notification dispatch for Raft.
//!
//! When the leader advances the commit index, it must notify clients
//! whose proposals have been committed. This module provides the
//! primitives for computing which indices are newly committed and
//! matching them back to pending proposals.
//!
//! The commit notification window is [old_commit + 1, new_commit].
//! Proposals registered at indices within this window are ready for
//! client notification.

use vstd::prelude::*;

verus! {

// --- newly_committed_count ---

// r[depends commit-notify.count]
/// Spec: how many entries were newly committed?
///
/// When the commit index advances from old_commit to new_commit,
/// the entries in (old_commit, new_commit] are newly committed.
pub open spec fn newly_committed_count_spec(
    old_commit: u64,
    new_commit: u64,
) -> u64 {
    if new_commit > old_commit {
        (new_commit - old_commit) as u64
    } else {
        0u64
    }
}

// r[impl commit-notify.count]
/// Count newly committed entries.
pub fn newly_committed_count(
    old_commit: u64,
    new_commit: u64,
) -> (result: u64)
    ensures result == newly_committed_count_spec(old_commit, new_commit)
{
    if new_commit > old_commit {
        new_commit - old_commit
    } else {
        0
    }
}

// --- is_newly_committed ---

// r[depends commit-notify.is-newly-committed]
/// Spec: is this index newly committed?
///
/// An index is newly committed if it falls in (old_commit, new_commit].
pub open spec fn is_newly_committed_spec(
    index: u64,
    old_commit: u64,
    new_commit: u64,
) -> bool {
    index > old_commit && index <= new_commit
}

// r[impl commit-notify.is-newly-committed]
/// Check if a specific index is in the newly committed range.
pub fn is_newly_committed(
    index: u64,
    old_commit: u64,
    new_commit: u64,
) -> (result: bool)
    ensures result == is_newly_committed_spec(index, old_commit, new_commit)
{
    index > old_commit && index <= new_commit
}

// --- first_newly_committed ---

// r[depends commit-notify.first-committed]
/// Spec: first newly committed index (old_commit + 1).
pub open spec fn first_newly_committed_spec(old_commit: u64) -> u64 {
    (old_commit + 1) as u64
}

// r[impl commit-notify.first-committed]
/// Get the first newly committed index.
pub fn first_newly_committed(old_commit: u64) -> (result: u64)
    requires old_commit < u64::MAX
    ensures result == first_newly_committed_spec(old_commit)
{
    old_commit + 1
}

// --- should_notify ---

// r[depends commit-notify.should-notify]
/// Spec: should clients be notified?
///
/// Notification is needed when the commit index actually advanced.
pub open spec fn should_notify_spec(
    old_commit: u64,
    new_commit: u64,
) -> bool {
    new_commit > old_commit
}

// r[impl commit-notify.should-notify]
/// Check if commit advancement warrants notification.
pub fn should_notify(
    old_commit: u64,
    new_commit: u64,
) -> (result: bool)
    ensures result == should_notify_spec(old_commit, new_commit)
{
    new_commit > old_commit
}

// --- proposal_status ---

// r[depends commit-notify.proposal-status]
/// Spec: what is the status of a pending proposal at the given index?
///
/// 0 = Pending (not yet committed, not expired)
/// 1 = Committed (index <= commit_index)
/// 2 = Expired (proposal timed out before commit)
pub open spec fn proposal_status_spec(
    proposal_index: u64,
    commit_index: u64,
    is_expired: bool,
) -> u8 {
    if proposal_index <= commit_index {
        1u8  // Committed
    } else if is_expired {
        2u8  // Expired
    } else {
        0u8  // Pending
    }
}

/// Proposal is pending (not yet committed).
pub const STATUS_PENDING: u8 = 0;
/// Proposal is committed.
pub const STATUS_COMMITTED: u8 = 1;
/// Proposal has expired.
pub const STATUS_EXPIRED: u8 = 2;

// r[impl commit-notify.proposal-status]
/// Get the status of a proposal.
pub fn proposal_status(
    proposal_index: u64,
    commit_index: u64,
    is_expired: bool,
) -> (result: u8)
    ensures result == proposal_status_spec(proposal_index, commit_index, is_expired)
{
    if proposal_index <= commit_index {
        STATUS_COMMITTED
    } else if is_expired {
        STATUS_EXPIRED
    } else {
        STATUS_PENDING
    }
}

// --- count_ready_proposals ---

// r[depends commit-notify.count-ready]
/// Spec: how many proposals in [start, start+len) are ready (committed or expired)?
pub open spec fn count_ready_from_spec(
    indices: Seq<u64>,
    expired: Seq<bool>,
    commit_index: u64,
    from: int,
) -> u64
    decreases indices.len() - from,
{
    if from >= indices.len() {
        0u64
    } else {
        let status = proposal_status_spec(indices[from], commit_index, expired[from]);
        let rest = count_ready_from_spec(indices, expired, commit_index, from + 1);
        if status != STATUS_PENDING {
            (rest + 1) as u64
        } else {
            rest
        }
    }
}

pub open spec fn count_ready_spec(
    indices: Seq<u64>,
    expired: Seq<bool>,
    commit_index: u64,
) -> u64 {
    count_ready_from_spec(indices, expired, commit_index, 0)
}

// Helper: count_ready_from is bounded by remaining length.
proof fn count_ready_from_bounded(
    indices: Seq<u64>, expired: Seq<bool>, commit_index: u64, from: int,
)
    requires 0 <= from, indices.len() == expired.len(), indices.len() <= u64::MAX as int
    ensures count_ready_from_spec(indices, expired, commit_index, from) <= (indices.len() - from) as u64
    decreases indices.len() - from,
{
    if from < indices.len() {
        count_ready_from_bounded(indices, expired, commit_index, from + 1);
    }
}

// r[impl commit-notify.count-ready]
/// Count proposals that are ready for notification.
pub fn count_ready_proposals(
    indices: &[u64],
    expired: &[bool],
    commit_index: u64,
) -> (result: u64)
    requires
        indices@.len() == expired@.len(),
        indices@.len() <= u64::MAX as int,
    ensures result == count_ready_spec(indices@, expired@, commit_index)
{
    let mut count: u64 = 0;
    let mut i: usize = 0;
    while i < indices.len()
        invariant
            0 <= i <= indices@.len(),
            indices@.len() == expired@.len(),
            indices@.len() <= u64::MAX as int,
            count_ready_spec(indices@, expired@, commit_index) ==
                count + count_ready_from_spec(indices@, expired@, commit_index, i as int),
            count <= i as u64,
        decreases indices.len() - i,
    {
        // Prove the remaining count fits in u64 (needed for `(rest + 1) as u64` cast)
        proof { count_ready_from_bounded(indices@, expired@, commit_index, i as int + 1); }
        let status = proposal_status(indices[i], commit_index, expired[i]);
        if status != STATUS_PENDING {
            count = count + 1;
        }
        i = i + 1;
    }
    count
}

// --- Safety proofs ---

// r[verify commit-notify.count]
/// No advancement means zero newly committed.
proof fn no_advance_zero(old_commit: u64)
    ensures newly_committed_count_spec(old_commit, old_commit) == 0
{}

// r[verify commit-notify.count]
/// Single step commits one entry.
proof fn single_step(old_commit: u64)
    requires old_commit < u64::MAX
    ensures newly_committed_count_spec(old_commit, (old_commit + 1) as u64) == 1
{}

// r[verify commit-notify.is-newly-committed]
/// First newly committed index is in the range.
proof fn first_is_committed(old_commit: u64, new_commit: u64)
    requires
        new_commit > old_commit,
        old_commit < u64::MAX,
    ensures is_newly_committed_spec((old_commit + 1) as u64, old_commit, new_commit)
{}

// r[verify commit-notify.is-newly-committed]
/// Old commit index is not newly committed.
proof fn old_commit_not_new(old_commit: u64, new_commit: u64)
    ensures !is_newly_committed_spec(old_commit, old_commit, new_commit)
{}

// r[verify commit-notify.is-newly-committed]
/// Index beyond new_commit is not newly committed.
proof fn beyond_not_committed(index: u64, old_commit: u64, new_commit: u64)
    requires index > new_commit
    ensures !is_newly_committed_spec(index, old_commit, new_commit)
{}

// r[verify commit-notify.should-notify]
/// Notification iff newly committed count > 0.
proof fn notify_iff_count_positive(old_commit: u64, new_commit: u64)
    ensures should_notify_spec(old_commit, new_commit) <==>
        newly_committed_count_spec(old_commit, new_commit) > 0
{}

// r[verify commit-notify.proposal-status]
/// Status is always valid.
proof fn status_valid(proposal_index: u64, commit_index: u64, is_expired: bool)
    ensures ({
        let s = proposal_status_spec(proposal_index, commit_index, is_expired);
        s == STATUS_PENDING || s == STATUS_COMMITTED || s == STATUS_EXPIRED
    })
{}

// r[verify commit-notify.proposal-status]
/// Committed proposals stay committed (monotonic commit index).
proof fn committed_stays_committed(proposal_index: u64, ci1: u64, ci2: u64, expired: bool)
    requires
        ci2 >= ci1,
        proposal_status_spec(proposal_index, ci1, expired) == STATUS_COMMITTED,
    ensures proposal_status_spec(proposal_index, ci2, expired) == STATUS_COMMITTED
{}

// r[verify commit-notify.proposal-status]
/// Committed wins over expired.
proof fn committed_wins_over_expired(proposal_index: u64, commit_index: u64)
    requires proposal_index <= commit_index
    ensures
        proposal_status_spec(proposal_index, commit_index, true) == STATUS_COMMITTED,
        proposal_status_spec(proposal_index, commit_index, false) == STATUS_COMMITTED,
{}

// r[verify commit-notify.count-ready]
/// Count is bounded by length.
proof fn count_ready_bounded(
    indices: Seq<u64>,
    expired: Seq<bool>,
    commit_index: u64,
)
    requires
        indices.len() == expired.len(),
        indices.len() <= u64::MAX as int,
    ensures count_ready_spec(indices, expired, commit_index) <= indices.len() as u64
{
    count_ready_bounded_from(indices, expired, commit_index, 0);
}

proof fn count_ready_bounded_from(
    indices: Seq<u64>,
    expired: Seq<bool>,
    commit_index: u64,
    from: int,
)
    requires
        from >= 0,
        indices.len() == expired.len(),
        indices.len() <= u64::MAX as int,
    ensures count_ready_from_spec(indices, expired, commit_index, from)
        <= (indices.len() - from) as u64
    decreases indices.len() - from,
{
    if from >= indices.len() {
    } else {
        count_ready_bounded_from(indices, expired, commit_index, from + 1);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify commit-notify.count]
    // r[verify commit-notify.first-committed]
    #[test]
    fn test_newly_committed_count() {
        assert_eq!(newly_committed_count(5, 10), 5);
        assert_eq!(newly_committed_count(5, 5), 0);
        assert_eq!(newly_committed_count(10, 5), 0);
        assert_eq!(newly_committed_count(0, 1), 1);
    }

    // r[verify commit-notify.is-newly-committed]
    #[test]
    fn test_is_newly_committed() {
        assert!(is_newly_committed(6, 5, 10));
        assert!(is_newly_committed(10, 5, 10));
        assert!(!is_newly_committed(5, 5, 10)); // old commit itself
        assert!(!is_newly_committed(11, 5, 10)); // beyond new
        assert!(!is_newly_committed(3, 5, 10)); // before old
    }

    // r[verify commit-notify.proposal-status]
    #[test]
    fn test_proposal_status() {
        assert_eq!(proposal_status(5, 10, false), STATUS_COMMITTED);
        assert_eq!(proposal_status(5, 10, true), STATUS_COMMITTED); // committed wins
        assert_eq!(proposal_status(15, 10, false), STATUS_PENDING);
        assert_eq!(proposal_status(15, 10, true), STATUS_EXPIRED);
    }

    // r[verify commit-notify.should-notify]
    #[test]
    fn test_should_notify() {
        assert!(should_notify(5, 10));
        assert!(!should_notify(10, 10));
        assert!(!should_notify(10, 5));
    }

    // r[verify commit-notify.count-ready]
    #[test]
    fn test_count_ready_proposals() {
        let indices = [3, 5, 8, 12];
        let expired = [false, false, false, true];
        // commit_index = 7: index 3 committed, index 5 committed, index 8 pending, index 12 expired
        assert_eq!(count_ready_proposals(&indices, &expired, 7), 3);
    }

    proptest! {
        // r[verify commit-notify.count]
        #[test]
        fn prop_count_nonneg(old_commit: u64, new_commit: u64) {
            // count is unsigned, always >= 0
            let _ = newly_committed_count(old_commit, new_commit);
        }

        // r[verify commit-notify.should-notify]
        #[test]
        fn prop_notify_iff_count(old_commit: u64, new_commit: u64) {
            let notify = should_notify(old_commit, new_commit);
            let count = newly_committed_count(old_commit, new_commit);
            prop_assert_eq!(notify, count > 0);
        }

        // r[verify commit-notify.proposal-status]
        #[test]
        fn prop_committed_wins(
            proposal_index in 0u64..100,
            commit_index in 0u64..100,
        ) {
            if proposal_index <= commit_index {
                prop_assert_eq!(proposal_status(proposal_index, commit_index, true), STATUS_COMMITTED);
                prop_assert_eq!(proposal_status(proposal_index, commit_index, false), STATUS_COMMITTED);
            }
        }

        // r[verify commit-notify.proposal-status]
        #[test]
        fn prop_status_valid(
            proposal_index: u64,
            commit_index: u64,
            is_expired: bool,
        ) {
            let s = proposal_status(proposal_index, commit_index, is_expired);
            prop_assert!(s <= 2);
        }
    }
}
