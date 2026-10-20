//! Verified allocation lifecycle state machine.
//!
//! An allocated block has a lifecycle: free → allocated → freed.
//! Double-free and use-after-free are modeled as invalid state transitions.
//! The token tracks state independently of the backing allocator, so
//! the proofs compose with any allocation strategy.

use vstd::prelude::*;

verus! {

/// State: block is available for allocation.
pub const STATE_FREE: u8 = 0;
/// State: block is currently allocated.
pub const STATE_ALLOCATED: u8 = 1;
/// State: block has been freed (terminal).
pub const STATE_FREED: u8 = 2;

// --- alloc_token_new ---

// r[depends alloc-token.new]
/// Spec: create a new token in the Free state.
pub open spec fn alloc_token_new_spec(id: u64) -> (u64, u8) {
    (id, STATE_FREE)
}

// r[impl alloc-token.new]
/// Create a new allocation token for the given block id.
pub fn alloc_token_new(id: u64) -> (result: (u64, u8))
    ensures result == alloc_token_new_spec(id)
{
    (id, STATE_FREE)
}

// --- alloc_token_allocate ---

// r[depends alloc-token.allocate]
/// Spec: transition from Free to Allocated. Returns new (id, state).
pub open spec fn alloc_token_allocate_spec(id: u64, state: u8) -> (u64, u8)
    recommends state == STATE_FREE
{
    (id, STATE_ALLOCATED)
}

// r[impl alloc-token.allocate]
/// Transition token from Free to Allocated.
pub fn alloc_token_allocate(id: u64, state: u8) -> (result: (u64, u8))
    requires state == STATE_FREE
    ensures result == alloc_token_allocate_spec(id, state)
{
    (id, STATE_ALLOCATED)
}

// --- alloc_token_release ---

// r[depends alloc-token.release]
/// Spec: transition from Allocated to Freed. Returns new (id, state).
pub open spec fn alloc_token_release_spec(id: u64, state: u8) -> (u64, u8)
    recommends state == STATE_ALLOCATED
{
    (id, STATE_FREED)
}

// r[impl alloc-token.release]
/// Transition token from Allocated to Freed.
pub fn alloc_token_release(id: u64, state: u8) -> (result: (u64, u8))
    requires state == STATE_ALLOCATED
    ensures result == alloc_token_release_spec(id, state)
{
    (id, STATE_FREED)
}

// --- is_live ---

// r[depends alloc-token.is-live]
/// Spec: token is in the Allocated state.
pub open spec fn is_live_spec(state: u8) -> bool {
    state == STATE_ALLOCATED
}

// r[impl alloc-token.is-live]
/// Returns true iff the token is currently allocated (live).
pub fn is_live(state: u8) -> (result: bool)
    ensures result == is_live_spec(state)
{
    state == STATE_ALLOCATED
}

// --- is_valid_state ---

// r[depends alloc-token.valid-state]
/// Spec: state is one of the three valid lifecycle states.
pub open spec fn is_valid_state_spec(state: u8) -> bool {
    state == STATE_FREE || state == STATE_ALLOCATED || state == STATE_FREED
}

// r[impl alloc-token.valid-state]
/// Returns true iff state is Free, Allocated, or Freed.
pub fn is_valid_state(state: u8) -> (result: bool)
    ensures result == is_valid_state_spec(state)
{
    state == STATE_FREE || state == STATE_ALLOCATED || state == STATE_FREED
}

// --- Proofs ---

// r[verify alloc-token.allocate]
/// Allocate requires Free state — cannot allocate from Allocated or Freed.
proof fn allocate_requires_free()
    ensures
        STATE_ALLOCATED != STATE_FREE,
        STATE_FREED != STATE_FREE,
{}

// r[verify alloc-token.release]
/// Release requires Allocated state — cannot release from Free or Freed.
proof fn release_requires_allocated()
    ensures
        STATE_FREE != STATE_ALLOCATED,
        STATE_FREED != STATE_ALLOCATED,
{}

// r[verify alloc-token.release]
/// Freed is a terminal state — no valid transition out.
proof fn freed_is_terminal()
    ensures
        STATE_FREED != STATE_FREE,
        STATE_FREED != STATE_ALLOCATED,
{}

// r[verify alloc-token.is-live]
/// is_live is true only in the Allocated state.
proof fn is_live_only_allocated(state: u8)
    ensures is_live_spec(state) <==> state == STATE_ALLOCATED
{}

// r[verify alloc-token.new]
/// Token id is preserved through allocation.
proof fn id_preserved_allocate(id: u64)
    ensures alloc_token_allocate_spec(id, STATE_FREE).0 == id
{}

// r[verify alloc-token.new]
/// Token id is preserved through release.
proof fn id_preserved_release(id: u64)
    ensures alloc_token_release_spec(id, STATE_ALLOCATED).0 == id
{}

// r[verify alloc-token.allocate]
/// Full lifecycle: new → allocate → release preserves id and reaches Freed.
proof fn full_lifecycle(id: u64)
    ensures ({
        let t0 = alloc_token_new_spec(id);
        let t1 = alloc_token_allocate_spec(t0.0, t0.1);
        let t2 = alloc_token_release_spec(t1.0, t1.1);
        t0.1 == STATE_FREE
        && t1.1 == STATE_ALLOCATED
        && t2.1 == STATE_FREED
        && t0.0 == id
        && t1.0 == id
        && t2.0 == id
    })
{}

// r[verify alloc-token.allocate]
/// Double-allocate is invalid: Allocated ≠ Free.
proof fn double_allocate_invalid()
    ensures STATE_ALLOCATED != STATE_FREE
{}

// r[verify alloc-token.release]
/// Double-free is invalid: Freed ≠ Allocated.
proof fn double_free_invalid()
    ensures STATE_FREED != STATE_ALLOCATED
{}

// r[verify alloc-token.is-live]
/// New token is not live.
proof fn new_not_live(id: u64)
    ensures !is_live_spec(alloc_token_new_spec(id).1)
{}

// r[verify alloc-token.is-live]
/// Allocated token is live.
proof fn allocated_is_live(id: u64)
    ensures is_live_spec(alloc_token_allocate_spec(id, STATE_FREE).1)
{}

// r[verify alloc-token.is-live]
/// Freed token is not live.
proof fn freed_not_live(id: u64)
    ensures !is_live_spec(alloc_token_release_spec(id, STATE_ALLOCATED).1)
{}

// r[verify alloc-token.valid-state]
/// All three states are valid.
proof fn all_states_valid()
    ensures
        is_valid_state_spec(STATE_FREE),
        is_valid_state_spec(STATE_ALLOCATED),
        is_valid_state_spec(STATE_FREED),
{}

// r[verify alloc-token.valid-state]
/// Each lifecycle step produces a valid state.
proof fn transitions_preserve_validity(id: u64)
    ensures
        is_valid_state_spec(alloc_token_new_spec(id).1),
        is_valid_state_spec(alloc_token_allocate_spec(id, STATE_FREE).1),
        is_valid_state_spec(alloc_token_release_spec(id, STATE_ALLOCATED).1),
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify alloc-token.new]
    #[test]
    fn test_new_token() {
        let (id, state) = alloc_token_new(42);
        assert_eq!(id, 42);
        assert_eq!(state, STATE_FREE);
        assert!(!is_live(state));
    }

    // r[verify alloc-token.allocate]
    #[test]
    fn test_allocate() {
        let (id, s0) = alloc_token_new(7);
        let (id2, s1) = alloc_token_allocate(id, s0);
        assert_eq!(id2, 7);
        assert_eq!(s1, STATE_ALLOCATED);
        assert!(is_live(s1));
    }

    // r[verify alloc-token.release]
    #[test]
    fn test_release() {
        let (id, s0) = alloc_token_new(7);
        let (_, s1) = alloc_token_allocate(id, s0);
        let (id3, s2) = alloc_token_release(id, s1);
        assert_eq!(id3, 7);
        assert_eq!(s2, STATE_FREED);
        assert!(!is_live(s2));
    }

    // r[verify alloc-token.allocate]
    #[test]
    fn test_full_lifecycle() {
        let (id, s0) = alloc_token_new(99);
        assert_eq!(s0, STATE_FREE);
        let (id2, s1) = alloc_token_allocate(id, s0);
        assert_eq!(s1, STATE_ALLOCATED);
        assert_eq!(id, id2);
        let (id3, s2) = alloc_token_release(id2, s1);
        assert_eq!(s2, STATE_FREED);
        assert_eq!(id2, id3);
    }

    // r[verify alloc-token.valid-state]
    #[test]
    fn test_valid_states() {
        assert!(is_valid_state(STATE_FREE));
        assert!(is_valid_state(STATE_ALLOCATED));
        assert!(is_valid_state(STATE_FREED));
        assert!(!is_valid_state(3));
        assert!(!is_valid_state(255));
    }

    // r[verify alloc-token.is-live]
    #[test]
    fn test_is_live() {
        assert!(!is_live(STATE_FREE));
        assert!(is_live(STATE_ALLOCATED));
        assert!(!is_live(STATE_FREED));
    }
}
