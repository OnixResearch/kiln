//! Verified leadership transfer for Raft.
//!
//! A leader voluntarily transfers leadership to a target follower by
//! catching it up, sending `TimeoutNow`, and blocking new proposals.
//! The transfer has a deadline — if the target doesn't win the election
//! in time, the leader resumes normal operation.

#![allow(clippy::wildcard_imports)]

use vstd::prelude::*;

use crate::election::*;

verus! {

// --- can_start_transfer ---

// r[depends leadership-transfer.can-start]
/// Spec: can this server initiate a leadership transfer?
///
/// Only a leader can transfer, and not to itself.
pub open spec fn can_start_transfer_spec(
    state: u8,
    target_id: u64,
    self_id: u64,
) -> bool {
    state == STATE_LEADER && target_id != self_id
}

// r[impl leadership-transfer.can-start]
/// Check if a leadership transfer can be started.
pub fn can_start_transfer(
    state: u8,
    target_id: u64,
    self_id: u64,
) -> (result: bool)
    ensures result == can_start_transfer_spec(state, target_id, self_id)
{
    state == STATE_LEADER && target_id != self_id
}

// --- is_target_caught_up ---

// r[depends leadership-transfer.target-caught-up]
/// Spec: has the target replicated all entries?
pub open spec fn is_target_caught_up_spec(
    target_match_index: u64,
    last_log_index: u64,
) -> bool {
    target_match_index >= last_log_index
}

// r[impl leadership-transfer.target-caught-up]
/// Check if the transfer target has caught up.
pub fn is_target_caught_up(
    target_match_index: u64,
    last_log_index: u64,
) -> (result: bool)
    ensures result == is_target_caught_up_spec(target_match_index, last_log_index)
{
    target_match_index >= last_log_index
}

// --- should_send_timeout_now ---

// r[depends leadership-transfer.should-send-timeout-now]
/// Spec: should the leader send `TimeoutNow` to the target?
///
/// All conditions: leader state, target caught up, transfer not expired.
pub open spec fn should_send_timeout_now_spec(
    is_leader: bool,
    target_caught_up: bool,
    transfer_deadline: u64,
    now: u64,
) -> bool {
    is_leader && target_caught_up && now <= transfer_deadline
}

// r[impl leadership-transfer.should-send-timeout-now]
/// Check if `TimeoutNow` should be sent.
pub fn should_send_timeout_now(
    is_leader: bool,
    target_caught_up: bool,
    transfer_deadline: u64,
    now: u64,
) -> (result: bool)
    ensures result == should_send_timeout_now_spec(
        is_leader, target_caught_up, transfer_deadline, now,
    )
{
    is_leader && target_caught_up && now <= transfer_deadline
}

// --- is_transfer_expired ---

// r[depends leadership-transfer.transfer-expired]
/// Spec: has the transfer deadline passed?
pub open spec fn is_transfer_expired_spec(
    transfer_deadline: u64,
    now: u64,
) -> bool {
    now > transfer_deadline
}

// r[impl leadership-transfer.transfer-expired]
/// Check if the transfer has expired.
pub fn is_transfer_expired(
    transfer_deadline: u64,
    now: u64,
) -> (result: bool)
    ensures result == is_transfer_expired_spec(transfer_deadline, now)
{
    now > transfer_deadline
}

// --- should_reject_proposal ---

// r[depends leadership-transfer.blocks-new-proposals]
/// Spec: should new client proposals be rejected?
pub open spec fn should_reject_proposal_spec(
    transfer_in_progress: bool,
) -> bool {
    transfer_in_progress
}

// r[impl leadership-transfer.blocks-new-proposals]
/// Check if proposals should be rejected during transfer.
pub fn should_reject_proposal(
    transfer_in_progress: bool,
) -> (result: bool)
    ensures result == should_reject_proposal_spec(transfer_in_progress)
{
    transfer_in_progress
}

// --- can_abort_transfer ---

// r[depends leadership-transfer.abort]
/// Spec: can the transfer be aborted?
pub open spec fn can_abort_transfer_spec(
    is_leader: bool,
    transfer_in_progress: bool,
) -> bool {
    is_leader && transfer_in_progress
}

// r[impl leadership-transfer.abort]
/// Check if a transfer can be aborted.
pub fn can_abort_transfer(
    is_leader: bool,
    transfer_in_progress: bool,
) -> (result: bool)
    ensures result == can_abort_transfer_spec(is_leader, transfer_in_progress)
{
    is_leader && transfer_in_progress
}

// --- Safety proofs ---

// r[verify leadership-transfer.can-start]
/// Non-leaders cannot start transfer.
proof fn non_leader_cannot_transfer(state: u8, target_id: u64, self_id: u64)
    requires state != STATE_LEADER
    ensures !can_start_transfer_spec(state, target_id, self_id)
{}

// r[verify leadership-transfer.can-start]
/// Self-transfer is rejected.
proof fn self_transfer_rejected(state: u8, self_id: u64)
    ensures !can_start_transfer_spec(state, self_id, self_id)
{}

// r[verify leadership-transfer.target-caught-up]
/// Target at log end is caught up.
proof fn target_at_end_caught_up(last_log_index: u64)
    ensures is_target_caught_up_spec(last_log_index, last_log_index)
{}

// r[verify leadership-transfer.target-caught-up]
/// Target ahead of log end is caught up.
proof fn target_ahead_caught_up(target_match: u64, last_log: u64)
    requires target_match >= last_log
    ensures is_target_caught_up_spec(target_match, last_log)
{}

// r[verify leadership-transfer.target-caught-up]
/// Target behind log end is not caught up.
proof fn target_behind_not_caught_up(target_match: u64, last_log: u64)
    requires target_match < last_log
    ensures !is_target_caught_up_spec(target_match, last_log)
{}

// r[verify leadership-transfer.should-send-timeout-now]
/// Expired transfer blocks `TimeoutNow`.
proof fn expired_blocks_timeout_now(
    is_leader: bool,
    target_caught_up: bool,
    deadline: u64,
    now: u64,
)
    requires now > deadline
    ensures !should_send_timeout_now_spec(is_leader, target_caught_up, deadline, now)
{}

// r[verify leadership-transfer.should-send-timeout-now]
/// Non-leader cannot send `TimeoutNow`.
proof fn non_leader_no_timeout_now(
    target_caught_up: bool,
    deadline: u64,
    now: u64,
)
    ensures !should_send_timeout_now_spec(false, target_caught_up, deadline, now)
{}

// r[verify leadership-transfer.transfer-expired]
/// Expired and not-expired are complements.
proof fn expired_complement(deadline: u64, now: u64)
    ensures
        is_transfer_expired_spec(deadline, now)
        == !(now <= deadline)
{}

// r[verify leadership-transfer.blocks-new-proposals]
/// No transfer means proposals accepted.
proof fn no_transfer_accepts_proposals()
    ensures !should_reject_proposal_spec(false)
{}

// r[verify leadership-transfer.abort]
/// Non-leader cannot abort.
proof fn non_leader_cannot_abort(transfer_in_progress: bool)
    ensures !can_abort_transfer_spec(false, transfer_in_progress)
{}

// r[verify leadership-transfer.abort]
/// No transfer means nothing to abort.
proof fn no_transfer_nothing_to_abort(is_leader: bool)
    ensures !can_abort_transfer_spec(is_leader, false)
{}

// r[verify leadership-transfer.should-send-timeout-now]
/// All conditions met allows `TimeoutNow`.
proof fn all_conditions_allow_timeout_now(deadline: u64, now: u64)
    requires now <= deadline
    ensures should_send_timeout_now_spec(true, true, deadline, now)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify leadership-transfer.can-start]
    #[test]
    fn test_can_start_transfer() {
        assert!(can_start_transfer(STATE_LEADER, 2, 1));
        assert!(!can_start_transfer(STATE_LEADER, 1, 1)); // self
        assert!(!can_start_transfer(STATE_FOLLOWER, 2, 1)); // not leader
        assert!(!can_start_transfer(STATE_TRANSITIONING, 2, 1));
    }

    // r[verify leadership-transfer.target-caught-up]
    #[test]
    fn test_is_target_caught_up() {
        assert!(is_target_caught_up(10, 10)); // exact
        assert!(is_target_caught_up(15, 10)); // ahead
        assert!(!is_target_caught_up(5, 10)); // behind
    }

    // r[verify leadership-transfer.should-send-timeout-now]
    #[test]
    fn test_should_send_timeout_now() {
        assert!(should_send_timeout_now(true, true, 100, 50));
        assert!(!should_send_timeout_now(true, true, 100, 101)); // expired
        assert!(!should_send_timeout_now(false, true, 100, 50)); // not leader
        assert!(!should_send_timeout_now(true, false, 100, 50)); // not caught up
    }

    // r[verify leadership-transfer.transfer-expired]
    #[test]
    fn test_is_transfer_expired() {
        assert!(is_transfer_expired(100, 101));
        assert!(!is_transfer_expired(100, 100)); // at deadline
        assert!(!is_transfer_expired(100, 50));
    }

    // r[verify leadership-transfer.blocks-new-proposals]
    #[test]
    fn test_should_reject_proposal() {
        assert!(should_reject_proposal(true));
        assert!(!should_reject_proposal(false));
    }

    // r[verify leadership-transfer.abort]
    #[test]
    fn test_can_abort_transfer() {
        assert!(can_abort_transfer(true, true));
        assert!(!can_abort_transfer(false, true));
        assert!(!can_abort_transfer(true, false));
    }

    proptest! {
        // r[verify leadership-transfer.can-start]
        #[test]
        fn prop_self_transfer_rejected(state: u8, id: u64) {
            prop_assert!(!can_start_transfer(state, id, id));
        }

        // r[verify leadership-transfer.target-caught-up]
        #[test]
        fn prop_caught_up_reflexive(index: u64) {
            prop_assert!(is_target_caught_up(index, index));
        }

        // r[verify leadership-transfer.transfer-expired]
        #[test]
        fn prop_expired_complement(deadline: u64, now: u64) {
            prop_assert_eq!(
                is_transfer_expired(deadline, now),
                now > deadline,
            );
        }

        // r[verify leadership-transfer.should-send-timeout-now]
        #[test]
        fn prop_expired_blocks_send(deadline: u64, now in 1u64..u64::MAX) {
            if now > deadline {
                prop_assert!(!should_send_timeout_now(true, true, deadline, now));
            }
        }

        // r[verify leadership-transfer.blocks-new-proposals]
        #[test]
        fn prop_transfer_blocks_proposals(in_progress: bool) {
            prop_assert_eq!(should_reject_proposal(in_progress), in_progress);
        }
    }
}
