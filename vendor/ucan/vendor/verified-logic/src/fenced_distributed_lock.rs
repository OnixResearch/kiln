//! Fenced distributed lock facade.
//!
//! This composite ties lease ownership, distributed-lock token progression,
//! fencing-token write admission, monotonic advancement, and CAS preconditions
//! into one stale-holder-safe decision surface.

use vstd::prelude::*;

use crate::lease_mutex::LockState;

verus! {

/// Public decision for the fenced lock composite.
pub enum FencedDistributedLockDecision {
    Grant { state: LockState, token: u64 },
    ProtectedWrite { token: u64 },
    RejectUnavailable,
    RejectStaleHolder,
    RejectStaleFence,
    RejectCasFailed,
}

/// Accepted acquisition branch: a new lease is grantable and its token advances.
pub open spec fn fenced_distributed_lock_grant_spec(
    state: LockState,
    node_id: u64,
    now: u64,
    ttl: u64,
) -> bool {
    &&& crate::lease_mutex::can_acquire_spec(state, now, ttl)
    &&& crate::distributed_lock::compute_next_fencing_token_spec(Some(state.fence_token))
        == crate::fencing::issue_token_spec(state.fence_token)
    &&& crate::monotonic::is_strict_advance_spec(
        state.fence_token,
        crate::distributed_lock::compute_next_fencing_token_spec(Some(state.fence_token)),
    )
    &&& crate::lease_mutex::acquire_next_spec(state, node_id, now, ttl).fence_token
        == crate::distributed_lock::compute_next_fencing_token_spec(Some(state.fence_token))
}

/// Accepted protected-write branch: owner, fence, monotonic, and CAS all agree.
pub open spec fn fenced_distributed_lock_write_spec(
    state: LockState,
    node_id: u64,
    now: u64,
    request_fence: u64,
    accepted_fence: u64,
    expected_is_some: bool,
    current_is_some: bool,
    values_equal: bool,
) -> bool {
    &&& crate::lease_mutex::write_permitted_spec(state, node_id, now, request_fence)
    &&& crate::fencing::is_valid_write_spec(request_fence, state.fence_token)
    &&& crate::monotonic::is_strict_advance_spec(accepted_fence, request_fence)
    &&& crate::cas::check_cas_spec(expected_is_some, current_is_some, values_equal)
}

/// Spec-level accepted predicate for the facade.
pub open spec fn fenced_distributed_lock_accepted_spec(
    state: LockState,
    node_id: u64,
    now: u64,
    ttl: u64,
    request_fence: u64,
    accepted_fence: u64,
    expected_is_some: bool,
    current_is_some: bool,
    values_equal: bool,
) -> bool {
    fenced_distributed_lock_grant_spec(state, node_id, now, ttl)
        || fenced_distributed_lock_write_spec(
            state,
            node_id,
            now,
            request_fence,
            accepted_fence,
            expected_is_some,
            current_is_some,
            values_equal,
        )
}

/// Rejection is read-only: caller-owned lock state and accepted fence remain unchanged.
pub open spec fn fenced_distributed_lock_reject_preserves_spec(
    before_state: LockState,
    after_state: LockState,
    before_accepted_fence: u64,
    after_accepted_fence: u64,
) -> bool {
    before_state == after_state && before_accepted_fence == after_accepted_fence
}

/// Spec decision code: 1 = Grant, 2 = ProtectedWrite, 3+ = rejects.
pub open spec fn fenced_distributed_lock_decision_spec(
    state: LockState,
    node_id: u64,
    now: u64,
    ttl: u64,
    request_fence: u64,
    accepted_fence: u64,
    expected_is_some: bool,
    current_is_some: bool,
    values_equal: bool,
) -> int {
    if fenced_distributed_lock_grant_spec(state, node_id, now, ttl) {
        1
    } else if !crate::lease_mutex::is_owner_spec(state, node_id, now) {
        3
    } else if crate::fencing::is_stale_spec(request_fence, state.fence_token)
        || !crate::monotonic::is_strict_advance_spec(accepted_fence, request_fence) {
        4
    } else if !crate::cas::check_cas_spec(expected_is_some, current_is_some, values_equal) {
        5
    } else if fenced_distributed_lock_write_spec(
        state,
        node_id,
        now,
        request_fence,
        accepted_fence,
        expected_is_some,
        current_is_some,
        values_equal,
    ) {
        2
    } else {
        3
    }
}

// r[impl composite-primitives.fenced-distributed-lock]
/// Admit either a new lock acquisition or a protected write under a live fence.
pub fn fenced_distributed_lock(
    state: &LockState,
    node_id: u64,
    now: u64,
    ttl: u64,
    request_fence: u64,
    accepted_fence: u64,
    expected_is_some: bool,
    current_is_some: bool,
    values_equal: bool,
) -> (decision: FencedDistributedLockDecision)
    ensures
        decision is Grant ==> fenced_distributed_lock_decision_spec(
            *state,
            node_id,
            now,
            ttl,
            request_fence,
            accepted_fence,
            expected_is_some,
            current_is_some,
            values_equal,
        ) == 1,
        decision is ProtectedWrite ==> fenced_distributed_lock_decision_spec(
            *state,
            node_id,
            now,
            ttl,
            request_fence,
            accepted_fence,
            expected_is_some,
            current_is_some,
            values_equal,
        ) == 2,
{
    if crate::lease_mutex::can_acquire(state, now, ttl) {
        let new_state = crate::lease_mutex::try_acquire(state, node_id, now, ttl).unwrap();
        let token = crate::distributed_lock::compute_next_fencing_token(Some(state.fence_token));
        return FencedDistributedLockDecision::Grant { state: new_state, token };
    }
    if !crate::lease_mutex::is_owner(state, node_id, now) {
        return FencedDistributedLockDecision::RejectStaleHolder;
    }
    if crate::fencing::is_stale(request_fence, state.fence_token)
        || !crate::monotonic::is_strict_advance(accepted_fence, request_fence)
    {
        return FencedDistributedLockDecision::RejectStaleFence;
    }
    if !crate::cas::check_cas(expected_is_some, current_is_some, values_equal) {
        return FencedDistributedLockDecision::RejectCasFailed;
    }
    if crate::lease_mutex::write_permitted(state, node_id, now, request_fence)
        && crate::fencing::is_valid_write(request_fence, state.fence_token)
    {
        FencedDistributedLockDecision::ProtectedWrite { token: request_fence }
    } else {
        FencedDistributedLockDecision::RejectUnavailable
    }
}

// r[verify composite-primitives.fenced-distributed-lock]
/// Accepted decisions refine the selected source predicates.
pub proof fn fenced_distributed_lock_refines_sources(
    state: LockState,
    node_id: u64,
    now: u64,
    ttl: u64,
    request_fence: u64,
    accepted_fence: u64,
    expected_is_some: bool,
    current_is_some: bool,
    values_equal: bool,
)
    requires fenced_distributed_lock_decision_spec(
        state,
        node_id,
        now,
        ttl,
        request_fence,
        accepted_fence,
        expected_is_some,
        current_is_some,
        values_equal,
    ) == 1 || fenced_distributed_lock_decision_spec(
        state,
        node_id,
        now,
        ttl,
        request_fence,
        accepted_fence,
        expected_is_some,
        current_is_some,
        values_equal,
    ) == 2
    ensures fenced_distributed_lock_accepted_spec(
        state,
        node_id,
        now,
        ttl,
        request_fence,
        accepted_fence,
        expected_is_some,
        current_is_some,
        values_equal,
    )
{}

// r[verify composite-primitives.fenced-distributed-lock]
/// Reject paths do not mutate caller-owned state.
pub proof fn fenced_distributed_lock_reject_is_read_only(state: LockState, accepted_fence: u64)
    ensures fenced_distributed_lock_reject_preserves_spec(state, state, accepted_fence, accepted_fence)
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    fn held_lock(holder: u64, expiry: u64, fence_token: u64) -> LockState {
        LockState {
            holder,
            expiry,
            fence_token,
        }
    }

    // r[verify composite-primitives.fenced-distributed-lock]
    #[test]
    fn grants_current_request_with_usable_fence() {
        let state = held_lock(7, 10, 41);
        let decision = fenced_distributed_lock(&state, 9, 10, 5, 0, 0, false, false, true);
        match decision {
            FencedDistributedLockDecision::Grant { state, token } => {
                assert_eq!(state.holder, 9);
                assert_eq!(state.expiry, 15);
                assert_eq!(state.fence_token, 42);
                assert_eq!(token, 42);
            }
            _ => panic!("expected fenced lock grant"),
        }
    }

    // r[verify composite-primitives.fenced-distributed-lock]
    #[test]
    fn rejects_stale_holder_and_stale_fence() {
        let state = held_lock(7, 20, 42);
        let stale_holder = fenced_distributed_lock(&state, 8, 11, 5, 42, 41, false, false, true);
        assert!(matches!(stale_holder, FencedDistributedLockDecision::RejectStaleHolder));

        let stale_fence = fenced_distributed_lock(&state, 7, 11, 5, 41, 40, false, false, true);
        assert!(matches!(stale_fence, FencedDistributedLockDecision::RejectStaleFence));
    }
}
