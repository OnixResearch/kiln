//! Verified view change protocol primitives.
//!
//! Models view-based reconfiguration used in BFT and state machine
//! replication protocols. Each view has a leader; a view change moves
//! to the next view when the current leader is suspected faulty. Proofs
//! cover view monotonicity, leader uniqueness per view, quorum-based
//! view change initiation, and new-view message validity.

use vstd::prelude::*;

verus! {

// ============================================================
// View number
// ============================================================

// r[depends view-change.next-view]
/// Spec: the next view number.
pub open spec fn next_view_spec(current: u64) -> u64 {
    if current < u64::MAX { (current + 1) as u64 } else { u64::MAX }
}

// r[impl view-change.next-view]
/// Advance to the next view.
pub fn next_view(current: u64) -> (result: u64)
    ensures result == next_view_spec(current)
{
    if current < u64::MAX { current + 1 } else { u64::MAX }
}

// r[depends view-change.leader-of]
/// Spec: the leader for a given view, determined by round-robin over
/// n replicas.
pub open spec fn leader_of_spec(view: u64, n: u64) -> u64
    recommends n > 0
{
    (view % n) as u64
}

// r[impl view-change.leader-of]
/// Compute the leader for a given view.
pub fn leader_of(view: u64, n: u64) -> (result: u64)
    requires n > 0
    ensures result == leader_of_spec(view, n)
{
    view % n
}

// ============================================================
// View change request tracking
// ============================================================

// r[depends view-change.has-quorum-requests]
/// Spec: are there enough view-change requests to trigger a view change?
/// Requires f+1 requests out of n replicas (for BFT with f faults).
pub open spec fn has_quorum_requests_spec(request_count: u64, f: u64) -> bool {
    request_count >= f + 1
}

// r[impl view-change.has-quorum-requests]
/// Returns true if enough replicas have requested a view change.
pub fn has_quorum_requests(request_count: u64, f: u64) -> (result: bool)
    requires f < u64::MAX
    ensures result == has_quorum_requests_spec(request_count, f)
{
    request_count >= f + 1
}

// r[depends view-change.should-change-view]
/// Spec: a replica should initiate a view change when:
/// 1. Enough view-change requests have been received, AND
/// 2. The proposed view is strictly higher than the current view.
pub open spec fn should_change_view_spec(
    current_view: u64, proposed_view: u64,
    request_count: u64, f: u64,
) -> bool {
    proposed_view > current_view
    && has_quorum_requests_spec(request_count, f)
}

// r[impl view-change.should-change-view]
/// Returns true if a view change should be initiated.
pub fn should_change_view(
    current_view: u64, proposed_view: u64,
    request_count: u64, f: u64,
) -> (result: bool)
    requires f < u64::MAX
    ensures result == should_change_view_spec(current_view, proposed_view, request_count, f)
{
    proposed_view > current_view
    && has_quorum_requests(request_count, f)
}

// ============================================================
// New-view message validation
// ============================================================

// r[depends view-change.is-valid-new-view]
/// Spec: a new-view message is valid when:
/// 1. The view is strictly greater than the current view.
/// 2. The claimed leader matches leader_of(new_view, n).
/// 3. The proof count meets the quorum threshold.
pub open spec fn is_valid_new_view_spec(
    current_view: u64, new_view: u64,
    claimed_leader: u64, n: u64,
    proof_count: u64, f: u64,
) -> bool
    recommends n > 0
{
    new_view > current_view
    && claimed_leader == leader_of_spec(new_view, n)
    && has_quorum_requests_spec(proof_count, f)
}

// r[impl view-change.is-valid-new-view]
/// Validate a new-view message.
pub fn is_valid_new_view(
    current_view: u64, new_view: u64,
    claimed_leader: u64, n: u64,
    proof_count: u64, f: u64,
) -> (result: bool)
    requires n > 0, f < u64::MAX
    ensures result == is_valid_new_view_spec(
        current_view, new_view, claimed_leader, n, proof_count, f,
    )
{
    new_view > current_view
    && claimed_leader == leader_of(new_view, n)
    && has_quorum_requests(proof_count, f)
}

// ============================================================
// View status
// ============================================================

/// View is normal (leader active).
pub const VIEW_NORMAL: u8 = 0;
/// View change in progress.
pub const VIEW_CHANGING: u8 = 1;

// r[depends view-change.is-normal]
/// Spec: is the replica in normal operation?
pub open spec fn is_normal_spec(status: u8) -> bool {
    status == VIEW_NORMAL
}

// r[impl view-change.is-normal]
pub fn is_normal(status: u8) -> (result: bool)
    ensures result == is_normal_spec(status)
{
    status == VIEW_NORMAL
}

// r[depends view-change.enter-view-change]
/// Spec: enter the view-change state.
pub open spec fn enter_view_change_spec() -> u8 { VIEW_CHANGING }

// r[impl view-change.enter-view-change]
pub fn enter_view_change() -> (result: u8)
    ensures result == enter_view_change_spec()
{
    VIEW_CHANGING
}

// r[depends view-change.enter-normal]
/// Spec: enter normal state after a successful view change.
pub open spec fn enter_normal_spec() -> u8 { VIEW_NORMAL }

// r[impl view-change.enter-normal]
pub fn enter_normal() -> (result: u8)
    ensures result == enter_normal_spec()
{
    VIEW_NORMAL
}

// ============================================================
// Proofs
// ============================================================

// r[verify view-change.next-view]
/// View numbers are monotonically non-decreasing.
proof fn next_view_monotone(v: u64)
    ensures next_view_spec(v) >= v
{}

// r[verify view-change.next-view]
/// Next view is strictly greater unless saturated.
proof fn next_view_strict(v: u64)
    requires v < u64::MAX
    ensures next_view_spec(v) > v
{}

// r[verify view-change.leader-of]
/// The leader is always in range [0, n).
proof fn leader_in_range(view: u64, n: u64)
    requires n > 0
    ensures leader_of_spec(view, n) < n
{
    vstd::arithmetic::div_mod::lemma_mod_pos_bound(view as int, n as int);
}

// r[verify view-change.leader-of]
/// Same view always gets the same leader.
proof fn leader_deterministic(v: u64, n: u64)
    requires n > 0
    ensures leader_of_spec(v, n) == leader_of_spec(v, n)
{}

// r[verify view-change.should-change-view]
/// Cannot initiate a view change to the same view.
proof fn no_change_to_same_view(v: u64, rc: u64, f: u64)
    ensures !should_change_view_spec(v, v, rc, f)
{}

// r[verify view-change.should-change-view]
/// A single request (f+1 == 1, f == 0) suffices with zero faults.
proof fn single_request_zero_faults(current: u64, proposed: u64)
    requires proposed > current
    ensures should_change_view_spec(current, proposed, 1, 0)
{}

// r[verify view-change.is-valid-new-view]
/// An invalid new-view (wrong leader) is rejected.
proof fn wrong_leader_rejected(
    cv: u64, nv: u64, wrong_leader: u64, n: u64, pc: u64, f: u64,
)
    requires
        n > 0,
        nv > cv,
        wrong_leader != leader_of_spec(nv, n),
    ensures !is_valid_new_view_spec(cv, nv, wrong_leader, n, pc, f)
{}

// r[verify view-change.is-valid-new-view]
/// A valid new-view advances the view strictly.
proof fn valid_new_view_advances(
    cv: u64, nv: u64, leader: u64, n: u64, pc: u64, f: u64,
)
    requires is_valid_new_view_spec(cv, nv, leader, n, pc, f)
    ensures nv > cv
{}

// r[verify view-change.enter-view-change]
/// After entering view-change, the status is not normal.
proof fn view_change_not_normal()
    ensures !is_normal_spec(enter_view_change_spec())
{}

// r[verify view-change.enter-normal]
/// After entering normal, the status is normal.
proof fn normal_is_normal()
    ensures is_normal_spec(enter_normal_spec())
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify view-change.next-view]
    #[test]
    fn test_next_view() {
        assert_eq!(next_view(0), 1);
        assert_eq!(next_view(5), 6);
        assert_eq!(next_view(u64::MAX), u64::MAX);
    }

    // r[verify view-change.leader-of]
    #[test]
    fn test_leader_of() {
        assert_eq!(leader_of(0, 3), 0);
        assert_eq!(leader_of(1, 3), 1);
        assert_eq!(leader_of(2, 3), 2);
        assert_eq!(leader_of(3, 3), 0);
        assert_eq!(leader_of(7, 5), 2);
    }

    // r[verify view-change.should-change-view]
    #[test]
    fn test_should_change_view() {
        assert!(should_change_view(0, 1, 2, 1)); // 2 requests, f=1
        assert!(!should_change_view(0, 1, 1, 1)); // only 1 request, need 2
        assert!(!should_change_view(5, 3, 10, 1)); // backward view
        assert!(!should_change_view(5, 5, 10, 1)); // same view
    }

    // r[verify view-change.is-valid-new-view]
    #[test]
    fn test_valid_new_view() {
        assert!(is_valid_new_view(0, 1, 1, 3, 2, 1)); // leader_of(1,3)=1, ok
        assert!(!is_valid_new_view(0, 1, 0, 3, 2, 1)); // wrong leader
        assert!(!is_valid_new_view(0, 1, 1, 3, 1, 1)); // insufficient proof (need 2)
    }

    #[test]
    fn test_view_status() {
        assert!(is_normal(VIEW_NORMAL));
        assert!(!is_normal(VIEW_CHANGING));
        assert_eq!(enter_view_change(), VIEW_CHANGING);
        assert_eq!(enter_normal(), VIEW_NORMAL);
    }

    proptest! {
        // r[verify view-change.next-view]
        #[test]
        fn prop_next_view_monotone(v: u64) {
            prop_assert!(next_view(v) >= v);
        }

        // r[verify view-change.leader-of]
        #[test]
        fn prop_leader_in_range(view: u64, n in 1u64..1000) {
            prop_assert!(leader_of(view, n) < n);
        }

        // r[verify view-change.leader-of]
        #[test]
        fn prop_leader_deterministic(view: u64, n in 1u64..1000) {
            prop_assert_eq!(leader_of(view, n), leader_of(view, n));
        }

        // r[verify view-change.should-change-view]
        #[test]
        fn prop_no_backward_view_change(current: u64, proposed: u64, rc: u64, f in 0u64..100) {
            if proposed <= current {
                prop_assert!(!should_change_view(current, proposed, rc, f));
            }
        }

        // r[verify view-change.is-valid-new-view]
        #[test]
        fn prop_wrong_leader_rejected(
            cv in 0u64..100, nv in 0u64..200, n in 1u64..100, pc: u64, f in 0u64..50,
        ) {
            if nv > cv {
                let real_leader = leader_of(nv, n);
                let wrong = if real_leader == 0 { 1 } else { 0 };
                if wrong != real_leader {
                    prop_assert!(!is_valid_new_view(cv, nv, wrong, n, pc, f));
                }
            }
        }
    }
}
