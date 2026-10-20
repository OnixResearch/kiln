//! Verified two-phase locking discipline.
//!
//! Models the growing/shrinking phase protocol that guarantees conflict
//! serializability. A transaction acquires locks in the growing phase and
//! releases them in the shrinking phase. Once the first release happens,
//! no further acquisitions are permitted. The lock point — the moment
//! between the last acquire and the first release — is when the
//! transaction holds its maximum lock set.

use vstd::prelude::*;

verus! {

// --- Phase constants ---

/// Growing phase: acquiring locks, none released yet.
pub const PHASE_GROWING: u8 = 0;
/// Shrinking phase: releasing locks, no further acquisitions.
pub const PHASE_SHRINKING: u8 = 1;

// --- Operation constants ---

/// Lock acquisition operation.
pub const OP_ACQUIRE: u8 = 0;
/// Lock release operation.
pub const OP_RELEASE: u8 = 1;

// --- is_valid_phase ---

// r[depends two-phase-lock.valid-phase]
/// Spec: phase is one of the two valid values.
pub open spec fn is_valid_phase_spec(phase: u8) -> bool {
    phase == PHASE_GROWING || phase == PHASE_SHRINKING
}

// r[impl two-phase-lock.valid-phase]
/// Returns true if the phase is valid.
pub fn is_valid_phase(phase: u8) -> (result: bool)
    ensures result == is_valid_phase_spec(phase)
{
    phase == PHASE_GROWING || phase == PHASE_SHRINKING
}

// --- can_acquire ---

// r[depends two-phase-lock.can-acquire]
/// Spec: locks can only be acquired in the growing phase.
pub open spec fn can_acquire_spec(phase: u8) -> bool {
    phase == PHASE_GROWING
}

// r[impl two-phase-lock.can-acquire]
/// Returns true if a lock acquisition is permitted.
pub fn can_acquire(phase: u8) -> (result: bool)
    ensures result == can_acquire_spec(phase)
{
    phase == PHASE_GROWING
}

// --- after_acquire ---

// r[depends two-phase-lock.after-acquire]
/// Spec: held count after acquiring one lock.
pub open spec fn after_acquire_held_spec(held: u64) -> u64 {
    (held + 1) as u64
}

// r[depends two-phase-lock.after-acquire]
/// Spec: phase stays GROWING after acquisition.
pub open spec fn after_acquire_phase_spec() -> u8 {
    PHASE_GROWING
}

// r[impl two-phase-lock.after-acquire]
/// Returns the new held count after acquiring one lock.
pub fn after_acquire_held(held: u64) -> (result: u64)
    requires held < u64::MAX
    ensures result == after_acquire_held_spec(held)
{
    held + 1
}

// --- can_release ---

// r[depends two-phase-lock.can-release]
/// Spec: can only release when holding at least one lock.
pub open spec fn can_release_spec(held: u64) -> bool {
    held > 0
}

// r[impl two-phase-lock.can-release]
/// Returns true if a lock release is permitted.
pub fn can_release(held: u64) -> (result: bool)
    ensures result == can_release_spec(held)
{
    held > 0
}

// --- after_release ---

// r[depends two-phase-lock.after-release]
/// Spec: held count after releasing one lock.
pub open spec fn after_release_held_spec(held: u64) -> u64 {
    (held - 1) as u64
}

// r[depends two-phase-lock.after-release]
/// Spec: the phase is always SHRINKING after a release.
pub open spec fn after_release_phase_spec() -> u8 {
    PHASE_SHRINKING
}

// r[impl two-phase-lock.after-release]
/// Returns the new held count after releasing one lock.
pub fn after_release_held(held: u64) -> (result: u64)
    requires held > 0
    ensures result == after_release_held_spec(held)
{
    held - 1
}

// --- is_complete ---

// r[depends two-phase-lock.is-complete]
/// Spec: transaction is complete when all locks are released and we
/// are in the shrinking phase.
pub open spec fn is_complete_spec(held: u64, phase: u8) -> bool {
    held == 0 && phase == PHASE_SHRINKING
}

// r[impl two-phase-lock.is-complete]
/// Returns true if the transaction has completed (all locks released).
pub fn is_complete(held: u64, phase: u8) -> (result: bool)
    ensures result == is_complete_spec(held, phase)
{
    held == 0 && phase == PHASE_SHRINKING
}

// --- would_violate ---

// r[depends two-phase-lock.would-violate]
/// Spec: an operation violates the 2PL protocol if it is an acquire
/// in the shrinking phase.
pub open spec fn would_violate_spec(phase: u8, op: u8) -> bool {
    op == OP_ACQUIRE && phase == PHASE_SHRINKING
}

// r[impl two-phase-lock.would-violate]
/// Returns true if this operation would break the two-phase discipline.
pub fn would_violate(phase: u8, op: u8) -> (result: bool)
    ensures result == would_violate_spec(phase, op)
{
    op == OP_ACQUIRE && phase == PHASE_SHRINKING
}

// --- transition ---

// r[depends two-phase-lock.transition]
/// Spec: apply one lock operation. Returns (new_held, new_phase) or
/// None if the operation is invalid.
pub open spec fn transition_spec(phase: u8, held: u64, op: u8) -> Option<(u64, u8)> {
    if op == OP_ACQUIRE && phase == PHASE_GROWING && held < u64::MAX {
        Some(((held + 1) as u64, PHASE_GROWING))
    } else if op == OP_RELEASE && held > 0 {
        Some(((held - 1) as u64, PHASE_SHRINKING))
    } else {
        None
    }
}

// r[impl two-phase-lock.transition]
/// Apply one lock operation. Returns (new_held, new_phase) or None.
pub fn transition(phase: u8, held: u64, op: u8) -> (result: Option<(u64, u8)>)
    ensures result == transition_spec(phase, held, op)
{
    if op == OP_ACQUIRE && phase == PHASE_GROWING && held < u64::MAX {
        Some((held + 1, PHASE_GROWING))
    } else if op == OP_RELEASE && held > 0 {
        Some((held - 1, PHASE_SHRINKING))
    } else {
        None
    }
}

// --- Proofs ---

// r[verify two-phase-lock.can-acquire]
/// The growing phase allows acquisition.
proof fn growing_allows_acquire()
    ensures can_acquire_spec(PHASE_GROWING)
{}

// r[impl two-phase-lock.phase-monotonic]
// r[verify two-phase-lock.phase-monotonic]
/// The shrinking phase blocks acquisition.
proof fn shrinking_blocks_acquire()
    ensures !can_acquire_spec(PHASE_SHRINKING)
{}

// r[verify two-phase-lock.after-acquire]
/// Acquiring stays in the growing phase.
proof fn acquire_stays_growing()
    ensures after_acquire_phase_spec() == PHASE_GROWING
{}

// r[verify two-phase-lock.after-release]
/// Releasing always enters the shrinking phase.
proof fn release_enters_shrinking()
    ensures after_release_phase_spec() == PHASE_SHRINKING
{}

// r[verify two-phase-lock.phase-monotonic]
/// Once shrinking, all transitions stay shrinking. Acquiring is blocked
/// (would_violate returns true) and releasing stays in shrinking.
proof fn phase_monotonic()
    ensures
        would_violate_spec(PHASE_SHRINKING, OP_ACQUIRE),
        after_release_phase_spec() == PHASE_SHRINKING,
{}

// r[verify two-phase-lock.after-acquire]
/// Acquiring increments the held count by exactly one.
proof fn acquire_increments(held: u64)
    requires held < u64::MAX
    ensures after_acquire_held_spec(held) as int == held as int + 1
{}

// r[verify two-phase-lock.after-release]
/// Releasing decrements the held count by exactly one.
proof fn release_decrements(held: u64)
    requires held > 0
    ensures after_release_held_spec(held) as int == held as int - 1
{}

// r[verify two-phase-lock.is-complete]
/// Completion implies the shrinking phase.
proof fn complete_implies_shrinking(held: u64, phase: u8)
    requires is_complete_spec(held, phase)
    ensures phase == PHASE_SHRINKING
{}

// r[verify two-phase-lock.is-complete]
/// Completion implies zero held locks.
proof fn complete_implies_no_locks(held: u64, phase: u8)
    requires is_complete_spec(held, phase)
    ensures held == 0
{}

// r[verify two-phase-lock.is-complete]
/// After acquiring, the transaction is not complete (held > 0).
proof fn acquire_not_complete(held: u64)
    requires held < u64::MAX
    ensures !is_complete_spec(after_acquire_held_spec(held), after_acquire_phase_spec())
{}

// r[verify two-phase-lock.is-complete]
/// Releasing the last lock makes the transaction complete.
proof fn release_last_completes()
    ensures is_complete_spec(after_release_held_spec(1), after_release_phase_spec())
{}

// r[verify two-phase-lock.transition]
/// Transition preserves 2PL: if phase was valid before, it's valid after.
proof fn transition_preserves_valid_phase(phase: u8, held: u64, op: u8)
    requires
        is_valid_phase_spec(phase),
        transition_spec(phase, held, op).is_some(),
    ensures is_valid_phase_spec(transition_spec(phase, held, op).unwrap().1)
{}

// r[verify two-phase-lock.transition]
/// Valid transitions never produce a violation: the result state is
/// consistent with the 2PL discipline.
proof fn transition_result_not_violating(phase: u8, held: u64, op: u8)
    requires transition_spec(phase, held, op).is_some()
    ensures !would_violate_spec(
        transition_spec(phase, held, op).unwrap().1,
        op,
    )
{}

// r[verify two-phase-lock.would-violate]
/// The only violation is acquiring in the shrinking phase.
proof fn violation_is_shrinking_acquire(phase: u8, op: u8)
    requires would_violate_spec(phase, op)
    ensures phase == PHASE_SHRINKING && op == OP_ACQUIRE
{}

// r[verify two-phase-lock.transition]
/// A complete lifecycle: acquire one, release one, complete.
proof fn lifecycle_one_lock()
    ensures ({
        let s1 = transition_spec(PHASE_GROWING, 0, OP_ACQUIRE);
        let s2 = transition_spec(s1.unwrap().1, s1.unwrap().0, OP_RELEASE);
        s1.is_some()
        && s2.is_some()
        && is_complete_spec(s2.unwrap().0, s2.unwrap().1)
    })
{}

// r[verify two-phase-lock.transition]
/// A two-lock lifecycle: acquire twice, release twice, complete.
proof fn lifecycle_two_locks()
    ensures ({
        let s1 = transition_spec(PHASE_GROWING, 0, OP_ACQUIRE);
        let s2 = transition_spec(s1.unwrap().1, s1.unwrap().0, OP_ACQUIRE);
        let s3 = transition_spec(s2.unwrap().1, s2.unwrap().0, OP_RELEASE);
        let s4 = transition_spec(s3.unwrap().1, s3.unwrap().0, OP_RELEASE);
        s1.is_some() && s2.is_some() && s3.is_some() && s4.is_some()
        && is_complete_spec(s4.unwrap().0, s4.unwrap().1)
    })
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify two-phase-lock.valid-phase]
    #[test]
    fn test_valid_phases() {
        assert!(is_valid_phase(PHASE_GROWING));
        assert!(is_valid_phase(PHASE_SHRINKING));
        assert!(!is_valid_phase(2));
        assert!(!is_valid_phase(255));
    }

    // r[verify two-phase-lock.can-acquire]
    #[test]
    fn test_can_acquire() {
        assert!(can_acquire(PHASE_GROWING));
        assert!(!can_acquire(PHASE_SHRINKING));
    }

    // r[verify two-phase-lock.can-release]
    #[test]
    fn test_can_release() {
        assert!(can_release(1));
        assert!(can_release(100));
        assert!(!can_release(0));
    }

    // r[verify two-phase-lock.is-complete]
    #[test]
    fn test_is_complete() {
        assert!(is_complete(0, PHASE_SHRINKING));
        assert!(!is_complete(1, PHASE_SHRINKING));
        assert!(!is_complete(0, PHASE_GROWING));
    }

    // r[verify two-phase-lock.would-violate]
    #[test]
    fn test_would_violate() {
        assert!(would_violate(PHASE_SHRINKING, OP_ACQUIRE));
        assert!(!would_violate(PHASE_GROWING, OP_ACQUIRE));
        assert!(!would_violate(PHASE_SHRINKING, OP_RELEASE));
        assert!(!would_violate(PHASE_GROWING, OP_RELEASE));
    }

    // r[verify two-phase-lock.transition]
    #[test]
    fn test_transition_acquire() {
        assert_eq!(transition(PHASE_GROWING, 0, OP_ACQUIRE), Some((1, PHASE_GROWING)));
        assert_eq!(transition(PHASE_GROWING, 5, OP_ACQUIRE), Some((6, PHASE_GROWING)));
        assert_eq!(transition(PHASE_SHRINKING, 5, OP_ACQUIRE), None); // blocked
        assert_eq!(transition(PHASE_GROWING, u64::MAX, OP_ACQUIRE), None); // overflow
    }

    // r[verify two-phase-lock.transition]
    #[test]
    fn test_transition_release() {
        assert_eq!(transition(PHASE_GROWING, 3, OP_RELEASE), Some((2, PHASE_SHRINKING)));
        assert_eq!(transition(PHASE_SHRINKING, 3, OP_RELEASE), Some((2, PHASE_SHRINKING)));
        assert_eq!(transition(PHASE_GROWING, 0, OP_RELEASE), None); // nothing to release
        assert_eq!(transition(PHASE_SHRINKING, 0, OP_RELEASE), None);
    }

    // r[verify two-phase-lock.transition]
    #[test]
    fn test_full_lifecycle() {
        let (mut held, mut phase) = (0u64, PHASE_GROWING);

        // Growing: acquire 3 locks
        let (h, p) = transition(phase, held, OP_ACQUIRE).unwrap();
        held = h;
        phase = p;
        assert_eq!((held, phase), (1, PHASE_GROWING));

        let (h, p) = transition(phase, held, OP_ACQUIRE).unwrap();
        held = h;
        phase = p;
        assert_eq!((held, phase), (2, PHASE_GROWING));

        let (h, p) = transition(phase, held, OP_ACQUIRE).unwrap();
        held = h;
        phase = p;
        assert_eq!((held, phase), (3, PHASE_GROWING));

        // Shrinking: release 3 locks
        let (h, p) = transition(phase, held, OP_RELEASE).unwrap();
        held = h;
        phase = p;
        assert_eq!((held, phase), (2, PHASE_SHRINKING));

        // Can't acquire anymore
        assert!(transition(phase, held, OP_ACQUIRE).is_none());

        let (h, p) = transition(phase, held, OP_RELEASE).unwrap();
        held = h;
        phase = p;
        let (h, p) = transition(phase, held, OP_RELEASE).unwrap();
        held = h;
        phase = p;

        assert!(is_complete(held, phase));
    }

    // r[verify two-phase-lock.transition]
    #[test]
    fn test_invalid_ops() {
        // Unknown op code
        assert_eq!(transition(PHASE_GROWING, 0, 5), None);
        assert_eq!(transition(PHASE_GROWING, 0, 255), None);
    }

    proptest! {
        // r[verify two-phase-lock.phase-monotonic]
        #[test]
        fn prop_shrinking_blocks_acquire(held in 0u64..1000) {
            prop_assert!(transition(PHASE_SHRINKING, held, OP_ACQUIRE).is_none());
        }

        // r[verify two-phase-lock.after-acquire]
        #[test]
        fn prop_acquire_increments(held in 0u64..u64::MAX - 1) {
            if let Some((new_held, new_phase)) = transition(PHASE_GROWING, held, OP_ACQUIRE) {
                prop_assert_eq!(new_held, held + 1);
                prop_assert_eq!(new_phase, PHASE_GROWING);
            }
        }

        // r[verify two-phase-lock.after-release]
        #[test]
        fn prop_release_decrements(held in 1u64..1000, phase in 0u8..2) {
            if is_valid_phase(phase) && held > 0 {
                if let Some((new_held, new_phase)) = transition(phase, held, OP_RELEASE) {
                    prop_assert_eq!(new_held, held - 1);
                    prop_assert_eq!(new_phase, PHASE_SHRINKING);
                }
            }
        }

        // r[verify two-phase-lock.transition]
        #[test]
        fn prop_transition_preserves_validity(
            phase in 0u8..2, held in 0u64..1000, op in 0u8..2,
        ) {
            if is_valid_phase(phase) {
                if let Some((_, new_phase)) = transition(phase, held, op) {
                    prop_assert!(is_valid_phase(new_phase));
                }
            }
        }

        // r[verify two-phase-lock.would-violate]
        #[test]
        fn prop_violation_detected(phase in 0u8..2, op in 0u8..2) {
            if would_violate(phase, op) {
                prop_assert!(transition(phase, 0, op).is_none());
                prop_assert!(transition(phase, 100, op).is_none());
            }
        }

        // r[verify two-phase-lock.is-complete]
        #[test]
        fn prop_complete_after_balanced_lifecycle(n in 1u64..20) {
            let mut held = 0u64;
            let mut phase = PHASE_GROWING;

            // Acquire n locks
            for _ in 0..n {
                let (h, p) = transition(phase, held, OP_ACQUIRE).unwrap();
                held = h; phase = p;
            }
            prop_assert_eq!(held, n);
            prop_assert_eq!(phase, PHASE_GROWING);

            // Release n locks
            for _ in 0..n {
                let (h, p) = transition(phase, held, OP_RELEASE).unwrap();
                held = h; phase = p;
            }
            prop_assert!(is_complete(held, phase));
        }
    }
}
