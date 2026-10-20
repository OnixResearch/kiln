//! Verified job concurrency slot allocation.
//!
//! Models a bounded pool of execution slots. `try_acquire_slot` grants a
//! slot when in_use < capacity; `release_slot` returns one (saturating
//! at zero). Proofs cover the capacity invariant, acquire/release
//! correctness, and the has_available predicate.

use vstd::prelude::*;

verus! {

// ============================================================
// has_available_slot
// ============================================================

// r[depends job-slot.has-available]
/// Spec: at least one slot is free.
pub open spec fn has_available_slot_spec(in_use: u64, capacity: u64) -> bool {
    in_use < capacity
}

// r[impl job-slot.has-available]
/// Check if at least one slot is available.
pub fn has_available_slot(in_use: u64, capacity: u64) -> (result: bool)
    ensures result == has_available_slot_spec(in_use, capacity)
{
    in_use < capacity
}

// ============================================================
// try_acquire_slot
// ============================================================

// r[depends job-slot.try-acquire]
/// Spec: acquire a slot. Returns (success, new_in_use).
/// Succeeds when in_use < capacity; new count is in_use + 1.
/// Fails when in_use >= capacity; count unchanged.
pub open spec fn try_acquire_slot_spec(in_use: u64, capacity: u64) -> (bool, u64) {
    if in_use < capacity {
        (true, (in_use + 1) as u64)
    } else {
        (false, in_use)
    }
}

// r[impl job-slot.try-acquire]
/// Try to acquire a slot. Returns (success, new_in_use).
pub fn try_acquire_slot(in_use: u64, capacity: u64) -> (result: (bool, u64))
    requires in_use <= capacity
    ensures
        result == try_acquire_slot_spec(in_use, capacity),
        // Capacity invariant preserved:
        result.1 <= capacity,
{
    if in_use < capacity {
        (true, in_use + 1)
    } else {
        (false, in_use)
    }
}

// ============================================================
// release_slot
// ============================================================

// r[depends job-slot.release]
/// Spec: release a slot. Saturates at zero.
pub open spec fn release_slot_spec(in_use: u64) -> u64 {
    if in_use > 0 { (in_use - 1) as u64 } else { 0u64 }
}

// r[impl job-slot.release]
/// Release a slot. Saturating — releasing at zero is a no-op.
pub fn release_slot(in_use: u64) -> (result: u64)
    ensures result == release_slot_spec(in_use)
{
    if in_use > 0 { in_use - 1 } else { 0 }
}

// ============================================================
// Safety proofs
// ============================================================

// r[verify job-slot.try-acquire]
/// Acquire succeeds iff there's an available slot.
proof fn acquire_iff_available(in_use: u64, capacity: u64)
    requires in_use <= capacity
    ensures
        try_acquire_slot_spec(in_use, capacity).0
            <==> has_available_slot_spec(in_use, capacity),
{}

// r[verify job-slot.try-acquire]
/// After successful acquire, in_use increases by exactly 1.
proof fn acquire_increments(in_use: u64, capacity: u64)
    requires in_use < capacity
    ensures
        try_acquire_slot_spec(in_use, capacity).0 == true,
        try_acquire_slot_spec(in_use, capacity).1 == (in_use + 1) as u64,
{}

// r[verify job-slot.try-acquire]
/// After failed acquire, in_use is unchanged.
proof fn acquire_fail_unchanged(in_use: u64, capacity: u64)
    requires in_use >= capacity
    ensures
        try_acquire_slot_spec(in_use, capacity).0 == false,
        try_acquire_slot_spec(in_use, capacity).1 == in_use,
{}

// r[verify job-slot.try-acquire]
/// Capacity invariant: after acquire, 0 <= new_in_use <= capacity.
proof fn acquire_preserves_invariant(in_use: u64, capacity: u64)
    requires in_use <= capacity
    ensures
        try_acquire_slot_spec(in_use, capacity).1 <= capacity,
{}

// r[verify job-slot.release]
/// Release never underflows — result is always >= 0 (trivially u64).
proof fn release_nonneg(in_use: u64)
    ensures release_slot_spec(in_use) >= 0u64
{}

// r[verify job-slot.release]
/// After release, in_use decreases by exactly 1 (when non-zero).
proof fn release_decrements(in_use: u64)
    requires in_use > 0
    ensures release_slot_spec(in_use) == (in_use - 1) as u64
{}

// r[verify job-slot.release]
/// Release at zero is a no-op.
proof fn release_at_zero()
    ensures release_slot_spec(0u64) == 0u64
{}

// r[verify job-slot.release]
/// Release preserves capacity invariant.
proof fn release_preserves_invariant(in_use: u64, capacity: u64)
    requires in_use <= capacity
    ensures release_slot_spec(in_use) <= capacity
{}

// r[verify job-slot.try-acquire]
// r[verify job-slot.release]
/// Acquire then release is identity.
proof fn acquire_release_roundtrip(in_use: u64, capacity: u64)
    requires in_use < capacity
    ensures ({
        let after_acquire = try_acquire_slot_spec(in_use, capacity).1;
        release_slot_spec(after_acquire) == in_use
    })
{}

// r[verify job-slot.try-acquire]
// r[verify job-slot.release]
/// Release then acquire is identity (when slots were in use).
proof fn release_acquire_roundtrip(in_use: u64, capacity: u64)
    requires in_use > 0, in_use <= capacity
    ensures ({
        let after_release = release_slot_spec(in_use);
        try_acquire_slot_spec(after_release, capacity).0 == true &&
        try_acquire_slot_spec(after_release, capacity).1 == in_use
    })
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify job-slot.has-available]
    #[test]
    fn test_has_available() {
        assert!(has_available_slot(0, 8));
        assert!(has_available_slot(7, 8));
        assert!(!has_available_slot(8, 8));
        assert!(!has_available_slot(0, 0));
    }

    // r[verify job-slot.try-acquire]
    #[test]
    fn test_acquire() {
        assert_eq!(try_acquire_slot(0, 8), (true, 1));
        assert_eq!(try_acquire_slot(7, 8), (true, 8));
        assert_eq!(try_acquire_slot(8, 8), (false, 8));
        assert_eq!(try_acquire_slot(0, 0), (false, 0));
    }

    // r[verify job-slot.release]
    #[test]
    fn test_release() {
        assert_eq!(release_slot(3), 2);
        assert_eq!(release_slot(1), 0);
        assert_eq!(release_slot(0), 0);
    }

    // r[verify job-slot.try-acquire]
    // r[verify job-slot.release]
    #[test]
    fn test_acquire_release_roundtrip() {
        let (ok, after) = try_acquire_slot(3, 8);
        assert!(ok);
        assert_eq!(after, 4);
        assert_eq!(release_slot(after), 3);
    }

    proptest! {
        // r[verify job-slot.try-acquire]
        #[test]
        fn prop_acquire_bounded(
            in_use in 0u64..1000,
            capacity in 0u64..1000,
        ) {
            if in_use <= capacity {
                let (_, new_in_use) = try_acquire_slot(in_use, capacity);
                prop_assert!(new_in_use <= capacity);
            }
        }

        // r[verify job-slot.try-acquire]
        #[test]
        fn prop_acquire_iff_available(
            in_use in 0u64..1000,
            capacity in 0u64..1000,
        ) {
            if in_use <= capacity {
                let (ok, _) = try_acquire_slot(in_use, capacity);
                prop_assert_eq!(ok, has_available_slot(in_use, capacity));
            }
        }

        // r[verify job-slot.release]
        #[test]
        fn prop_release_never_underflows(in_use in 0u64..1000) {
            let after = release_slot(in_use);
            prop_assert!(after <= in_use);
        }

        // r[verify job-slot.try-acquire]
        // r[verify job-slot.release]
        #[test]
        fn prop_acquire_release_identity(
            in_use in 0u64..999,
            capacity in 1u64..1000,
        ) {
            if in_use < capacity {
                let (ok, after_acquire) = try_acquire_slot(in_use, capacity);
                prop_assert!(ok);
                let after_release = release_slot(after_acquire);
                prop_assert_eq!(after_release, in_use);
            }
        }

        // r[verify job-slot.try-acquire]
        #[test]
        fn prop_capacity_invariant_sequence(
            capacity in 1u64..100,
            ops in proptest::collection::vec(prop::bool::ANY, 1..50),
        ) {
            let mut in_use = 0u64;
            for acquire in ops {
                if acquire {
                    let (_, new) = try_acquire_slot(in_use, capacity);
                    in_use = new;
                } else {
                    in_use = release_slot(in_use);
                }
                prop_assert!(in_use <= capacity);
            }
        }
    }
}
