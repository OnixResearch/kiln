//! Verified lease-based mutual exclusion.
//!
//! A distributed lock modeled as a lease with holder identity and fencing
//! token. Acquisition requires the lock to be available (expired or never
//! held). The fencing token increments on each acquisition, letting
//! storage nodes reject writes from stale holders. Safety: two nodes
//! cannot simultaneously be the owner of the same lock.

use vstd::prelude::*;

verus! {

// --- LockState ---

/// State of a distributed lock: who holds it, when it expires, and the
/// current fencing token.
#[derive(Copy, Clone)]
pub struct LockState {
    /// Node ID of the current holder.
    pub holder: u64,
    /// Lease expiration timestamp (exclusive). Lock is available when
    /// now >= expiry.
    pub expiry: u64,
    /// Monotonically increasing fencing token. Incremented on each
    /// successful acquisition.
    pub fence_token: u64,
}

// --- is_available ---

// r[depends lease-mutex.is-available]
/// Spec: the lock is available when the current time is at or past expiry.
pub open spec fn is_available_spec(state: LockState, now: u64) -> bool {
    now >= state.expiry
}

// r[impl lease-mutex.is-available]
/// Returns true if the lock is available for acquisition.
pub fn is_available(state: &LockState, now: u64) -> (result: bool)
    ensures result == is_available_spec(*state, now)
{
    now >= state.expiry
}

// --- can_acquire ---

// r[depends lease-mutex.can-acquire]
/// Spec: acquisition is possible when the lock is available, the TTL
/// is positive, the new expiry won't overflow, and the fence can be
/// incremented.
pub open spec fn can_acquire_spec(state: LockState, now: u64, ttl: u64) -> bool {
    is_available_spec(state, now)
    && ttl > 0
    && (now as int) + (ttl as int) <= (u64::MAX as int)
    && state.fence_token < u64::MAX
}

// r[impl lease-mutex.can-acquire]
/// Returns true if the lock can be acquired right now.
pub fn can_acquire(state: &LockState, now: u64, ttl: u64) -> (result: bool)
    ensures result == can_acquire_spec(*state, now, ttl)
{
    is_available(state, now)
    && ttl > 0
    && ttl <= u64::MAX - now
    && state.fence_token < u64::MAX
}

// --- try_acquire ---

// r[depends lease-mutex.try-acquire]
/// Spec: new lock state after acquisition.
pub open spec fn acquire_next_spec(state: LockState, node_id: u64, now: u64, ttl: u64) -> LockState {
    LockState {
        holder: node_id,
        expiry: (now + ttl) as u64,
        fence_token: (state.fence_token + 1) as u64,
    }
}

// r[impl lease-mutex.try-acquire]
/// Attempt to acquire the lock. Returns the new state on success.
pub fn try_acquire(state: &LockState, node_id: u64, now: u64, ttl: u64) -> (result: Option<LockState>)
    ensures
        match result {
            Some(new_state) =>
                can_acquire_spec(*state, now, ttl)
                && new_state == acquire_next_spec(*state, node_id, now, ttl),
            None => !can_acquire_spec(*state, now, ttl),
        },
{
    if !is_available(state, now) || ttl == 0 || ttl > u64::MAX - now
        || state.fence_token == u64::MAX
    {
        None
    } else {
        Some(LockState {
            holder: node_id,
            expiry: now + ttl,
            fence_token: state.fence_token + 1,
        })
    }
}

// --- is_owner ---

// r[depends lease-mutex.is-owner]
/// Spec: node_id is the current owner if it matches the holder and the
/// lease has not expired.
pub open spec fn is_owner_spec(state: LockState, node_id: u64, now: u64) -> bool {
    state.holder == node_id && (now as int) < (state.expiry as int)
}

// r[impl lease-mutex.is-owner]
/// Returns true if `node_id` currently holds this lock.
pub fn is_owner(state: &LockState, node_id: u64, now: u64) -> (result: bool)
    ensures result == is_owner_spec(*state, node_id, now)
{
    state.holder == node_id && now < state.expiry
}

// --- write_permitted ---

// r[depends lease-mutex.write-permitted]
/// Spec: a write is permitted when the requester is the current owner
/// and the request carries a fence token at least as recent as the lock's.
pub open spec fn write_permitted_spec(
    state: LockState, node_id: u64, now: u64, request_fence: u64,
) -> bool {
    is_owner_spec(state, node_id, now) && request_fence >= state.fence_token
}

// r[impl lease-mutex.write-permitted]
/// Returns true if this write should be accepted.
pub fn write_permitted(
    state: &LockState, node_id: u64, now: u64, request_fence: u64,
) -> (result: bool)
    ensures result == write_permitted_spec(*state, node_id, now, request_fence)
{
    is_owner(state, node_id, now) && request_fence >= state.fence_token
}

// --- do_release ---

// r[depends lease-mutex.release]
/// Spec: releasing a lock sets the expiry to now, making it immediately
/// available. The fence token is preserved.
pub open spec fn release_spec(state: LockState, now: u64) -> LockState {
    LockState {
        holder: state.holder,
        expiry: now,
        fence_token: state.fence_token,
    }
}

// r[impl lease-mutex.release]
/// Release the lock by expiring the lease immediately.
pub fn do_release(state: &LockState, now: u64) -> (result: LockState)
    ensures result == release_spec(*state, now)
{
    LockState {
        holder: state.holder,
        expiry: now,
        fence_token: state.fence_token,
    }
}

// --- Proofs ---

// r[verify lease-mutex.try-acquire]
/// After acquisition, the acquirer is the owner.
proof fn acquire_sets_owner(state: LockState, node_id: u64, now: u64, ttl: u64)
    requires can_acquire_spec(state, now, ttl)
    ensures is_owner_spec(acquire_next_spec(state, node_id, now, ttl), node_id, now)
{}

// r[verify lease-mutex.try-acquire]
/// Acquisition advances the fence token by exactly one.
proof fn acquire_advances_fence(state: LockState, node_id: u64, now: u64, ttl: u64)
    requires can_acquire_spec(state, now, ttl)
    ensures
        acquire_next_spec(state, node_id, now, ttl).fence_token as int
            == state.fence_token as int + 1,
{}

// r[verify lease-mutex.try-acquire]
/// The new lease expires in the future (at now + ttl > now).
proof fn acquire_extends_validity(state: LockState, node_id: u64, now: u64, ttl: u64)
    requires can_acquire_spec(state, now, ttl)
    ensures (acquire_next_spec(state, node_id, now, ttl).expiry as int) > (now as int)
{}

// r[verify lease-mutex.is-owner]
/// If the lease has expired, nobody is the owner.
proof fn expired_not_owner(state: LockState, node_id: u64, now: u64)
    requires now >= state.expiry
    ensures !is_owner_spec(state, node_id, now)
{}

// r[impl lease-mutex.safety]
// r[verify lease-mutex.safety]
/// If A is the owner, then no different node B is the owner at the same
/// time. (The lock has a single holder field — trivially unique.)
proof fn owner_unique(state: LockState, a: u64, b: u64, now: u64)
    requires is_owner_spec(state, a, now), a != b
    ensures !is_owner_spec(state, b, now)
{}

// r[verify lease-mutex.safety]
/// Successive leases do not overlap. If the new acquisition happened at
/// or after the old lease's expiry, then any time `t` before old_expiry
/// is also before acquire_time (transitivity of <).
proof fn successive_leases_disjoint(old_expiry: u64, acquire_time: u64, t: u64)
    requires
        acquire_time >= old_expiry,
        (t as int) < (old_expiry as int),
    ensures (t as int) < (acquire_time as int)
{}

// r[verify lease-mutex.write-permitted]
/// A stale fence token is rejected even if the holder matches.
proof fn stale_fence_rejected(state: LockState, node_id: u64, now: u64, old_fence: u64)
    requires
        is_owner_spec(state, node_id, now),
        old_fence < state.fence_token,
    ensures !write_permitted_spec(state, node_id, now, old_fence)
{}

// r[verify lease-mutex.release]
/// After release, the lock is available.
proof fn release_makes_available(state: LockState, now: u64)
    ensures is_available_spec(release_spec(state, now), now)
{}

// r[verify lease-mutex.release]
/// After release, the former holder is no longer the owner.
proof fn release_terminates_ownership(state: LockState, now: u64, node_id: u64)
    ensures !is_owner_spec(release_spec(state, now), node_id, now)
{}

// r[verify lease-mutex.release]
/// Release preserves the fence token.
proof fn release_preserves_fence(state: LockState, now: u64)
    ensures release_spec(state, now).fence_token == state.fence_token
{}

// r[verify lease-mutex.try-acquire]
/// After acquire-then-release, the fence still reflects the acquisition.
proof fn acquire_release_fence_persists(state: LockState, node_id: u64, now: u64, ttl: u64)
    requires can_acquire_spec(state, now, ttl)
    ensures
        release_spec(acquire_next_spec(state, node_id, now, ttl), now).fence_token as int
            == state.fence_token as int + 1,
{}

// r[verify lease-mutex.safety]
/// A previous holder's writes are rejected after a new acquisition.
/// The old fence is less than the new fence, so write_permitted fails.
proof fn previous_holder_rejected(
    old_state: LockState, new_holder: u64, old_holder: u64,
    now: u64, ttl: u64,
)
    requires
        can_acquire_spec(old_state, now, ttl),
        old_holder != new_holder,
    ensures !write_permitted_spec(
        acquire_next_spec(old_state, new_holder, now, ttl),
        old_holder,
        now,
        old_state.fence_token,
    )
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn fresh_lock() -> LockState {
        LockState {
            holder: 0,
            expiry: 0,
            fence_token: 0,
        }
    }

    // r[verify lease-mutex.is-available]
    // r[verify lease-mutex.can-acquire]
    #[test]
    fn test_fresh_lock_available() {
        let lock = fresh_lock();
        assert!(is_available(&lock, 0));
        assert!(is_available(&lock, 100));
    }

    // r[verify lease-mutex.try-acquire]
    #[test]
    fn test_acquire_basic() {
        let lock = fresh_lock();
        let result = try_acquire(&lock, 1, 10, 30);
        assert!(result.is_some());
        let new_lock = result.unwrap();
        assert_eq!(new_lock.holder, 1);
        assert_eq!(new_lock.expiry, 40);
        assert_eq!(new_lock.fence_token, 1);
    }

    // r[verify lease-mutex.try-acquire]
    #[test]
    fn test_acquire_while_held() {
        let held = LockState {
            holder: 1,
            expiry: 100,
            fence_token: 1,
        };
        // Lock is held (now=50 < expiry=100), so acquire fails
        assert!(try_acquire(&held, 2, 50, 30).is_none());
        // Lock expired (now=100 >= expiry=100), so acquire succeeds
        assert!(try_acquire(&held, 2, 100, 30).is_some());
    }

    // r[verify lease-mutex.is-owner]
    #[test]
    fn test_owner_check() {
        let lock = LockState {
            holder: 5,
            expiry: 100,
            fence_token: 1,
        };
        assert!(is_owner(&lock, 5, 50)); // correct holder, not expired
        assert!(!is_owner(&lock, 5, 100)); // expired
        assert!(!is_owner(&lock, 6, 50)); // wrong holder
    }

    // r[verify lease-mutex.write-permitted]
    #[test]
    fn test_write_permitted() {
        let lock = LockState {
            holder: 5,
            expiry: 100,
            fence_token: 3,
        };
        assert!(write_permitted(&lock, 5, 50, 3)); // exact fence match
        assert!(write_permitted(&lock, 5, 50, 5)); // higher fence ok
        assert!(!write_permitted(&lock, 5, 50, 2)); // stale fence
        assert!(!write_permitted(&lock, 6, 50, 3)); // wrong holder
        assert!(!write_permitted(&lock, 5, 100, 3)); // expired
    }

    // r[verify lease-mutex.release]
    #[test]
    fn test_release() {
        let lock = LockState {
            holder: 5,
            expiry: 100,
            fence_token: 3,
        };
        let released = do_release(&lock, 60);
        assert_eq!(released.expiry, 60);
        assert_eq!(released.fence_token, 3);
        assert!(is_available(&released, 60));
        assert!(!is_owner(&released, 5, 60));
    }

    proptest! {
        // r[verify lease-mutex.try-acquire]
        #[test]
        fn prop_acquire_sets_owner(
            now in 0u64..u64::MAX / 2,
            ttl in 1u64..u64::MAX / 4,
            node_id in 1u64..1000,
        ) {
            let lock = LockState { holder: 0, expiry: 0, fence_token: 0 };
            if let Some(new_lock) = try_acquire(&lock, node_id, now, ttl) {
                prop_assert!(is_owner(&new_lock, node_id, now));
            }
        }

        // r[verify lease-mutex.safety]
        #[test]
        fn prop_owner_unique(
            holder in 1u64..100,
            expiry in 50u64..200,
            fence in 0u64..100,
            now in 0u64..50,
            other in 100u64..200,
        ) {
            let lock = LockState { holder, expiry, fence_token: fence };
            if is_owner(&lock, holder, now) {
                prop_assert!(!is_owner(&lock, other, now));
            }
        }

        // r[verify lease-mutex.safety]
        #[test]
        fn prop_fence_monotonic(
            now in 0u64..u64::MAX / 4,
            ttl in 1u64..u64::MAX / 4,
            node_a in 1u64..100,
            node_b in 100u64..200,
        ) {
            let lock = LockState { holder: 0, expiry: 0, fence_token: 0 };
            if let Some(after_a) = try_acquire(&lock, node_a, now, ttl) {
                let expire_time = now + ttl;
                if let Some(after_b) = try_acquire(&after_a, node_b, expire_time, ttl) {
                    prop_assert!(after_b.fence_token > after_a.fence_token);
                    // A's old fence token is stale
                    prop_assert!(!write_permitted(
                        &after_b, node_a, expire_time, after_a.fence_token
                    ));
                }
            }
        }

        // r[verify lease-mutex.release]
        #[test]
        fn prop_release_terminates(
            holder in 1u64..100,
            now in 0u64..1000,
            ttl in 1u64..1000,
            fence in 0u64..100,
        ) {
            let lock = LockState { holder, expiry: now + ttl, fence_token: fence };
            let released = do_release(&lock, now);
            prop_assert!(!is_owner(&released, holder, now));
            prop_assert!(is_available(&released, now));
        }

        // r[verify lease-mutex.safety]
        #[test]
        fn prop_successive_leases_no_overlap(
            now_a in 0u64..1000,
            ttl_a in 1u64..1000,
            ttl_b in 1u64..1000,
            node_a in 1u64..100,
            node_b in 100u64..200,
        ) {
            let lock = LockState { holder: 0, expiry: 0, fence_token: 0 };
            if let Some(after_a) = try_acquire(&lock, node_a, now_a, ttl_a) {
                let now_b = now_a + ttl_a; // B acquires right after A expires
                if now_b <= u64::MAX / 2 {
                    if let Some(after_b) = try_acquire(&after_a, node_b, now_b, ttl_b) {
                        // B is owner at now_b
                        prop_assert!(is_owner(&after_b, node_b, now_b));
                        // A is NOT owner at now_b (lease expired, holder changed)
                        prop_assert!(!is_owner(&after_b, node_a, now_b));
                    }
                }
            }
        }
    }
}
