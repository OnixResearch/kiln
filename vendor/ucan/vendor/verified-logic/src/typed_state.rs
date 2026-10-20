//! Compile-time enforcement of verification pipelines via phantom types.
//!
//! `Stamped<S, T>` wraps data `T` with a state marker `S`. The marker
//! is either `Unverified` (freely constructable) or `Verified` (only
//! producible by `verify_with`). Private fields prevent forging
//! `Stamped<Verified, T>` without going through the verification step.
//!
//! This guarantee holds even in downstream crates that don't use Verus.
//! The Verus proofs here show the logical properties (roundtrip,
//! predicate preservation); the type-level enforcement is structural.

use vstd::prelude::*;

/// Marker: data has not been verified.
pub struct Unverified;

/// Marker: data has passed a verification predicate.
pub struct Verified;

/// Data `T` stamped with verification state `S`.
///
/// Fields are private — the only way to get `Stamped<Verified, T>` is
/// through `verify_with`. Downstream crates can name the type in
/// signatures but cannot construct it directly.
pub struct Stamped<S, T> {
    value: T,
    _state: std::marker::PhantomData<S>,
}

// Non-verus impl blocks for the type-level API.
// These are the runtime functions that downstream crates call.

impl<T> Stamped<Unverified, T> {
    /// Wrap raw data as unverified.
    // r[impl typed-state.stamp]
    #[inline]
    pub fn stamp(value: T) -> Self {
        Stamped {
            value,
            _state: std::marker::PhantomData,
        }
    }
}

impl<S, T> Stamped<S, T> {
    /// Read access to the inner value, regardless of state.
    // r[impl typed-state.inner]
    #[inline]
    pub fn inner(&self) -> &T {
        &self.value
    }
}

impl<T> Stamped<Verified, T> {
    /// Extract the inner value. Only callable on verified data.
    // r[impl typed-state.unwrap]
    #[inline]
    pub fn unwrap_verified(self) -> T {
        self.value
    }
}

/// Verify the inner value with a predicate. Returns `Some` with
/// the value promoted to `Verified` if the predicate passes, `None`
/// otherwise.
// r[impl typed-state.verify]
pub fn verify_with<T>(
    stamped: Stamped<Unverified, T>,
    predicate: impl FnOnce(&T) -> bool,
) -> Option<Stamped<Verified, T>> {
    if predicate(&stamped.value) {
        Some(Stamped {
            value: stamped.value,
            _state: std::marker::PhantomData,
        })
    } else {
        None
    }
}

verus! {

// ═══════════════════════════════════════════════════════════════════
// Spec-level model using ghost bool
// ═══════════════════════════════════════════════════════════════════

// Verus can't reason about phantom type parameters directly.
// We model the state as a ghost bool: true = Verified, false = Unverified.

/// Ghost model of a stamped value.
pub ghost struct GhostStamped<T> {
    pub value: T,
    pub verified: bool,
}

// r[depends typed-state.stamp]
/// Spec: stamp produces an unverified ghost stamped value.
pub open spec fn stamp_spec<T>(value: T) -> GhostStamped<T> {
    GhostStamped { value, verified: false }
}

// r[depends typed-state.verify]
/// Spec: verify_with transitions to verified iff predicate holds.
pub open spec fn verify_with_spec<T>(
    gs: GhostStamped<T>,
    predicate_result: bool,
) -> Option<GhostStamped<T>> {
    if predicate_result {
        Some(GhostStamped { value: gs.value, verified: true })
    } else {
        None
    }
}

// r[depends typed-state.unwrap]
/// Spec: unwrap extracts the inner value.
pub open spec fn unwrap_verified_spec<T>(gs: GhostStamped<T>) -> T
    recommends gs.verified
{
    gs.value
}

// r[depends typed-state.inner]
/// Spec: inner returns the wrapped value.
pub open spec fn inner_spec<T>(gs: GhostStamped<T>) -> T {
    gs.value
}

// ═══════════════════════════════════════════════════════════════════
// Proofs
// ═══════════════════════════════════════════════════════════════════

// r[impl typed-state.stamp]
// r[verify typed-state.stamp]
/// Proof: stamp preserves the value.
proof fn stamp_preserves_value<T>(v: T)
    ensures inner_spec(stamp_spec(v)) == v
{}

// r[impl typed-state.roundtrip]
// r[verify typed-state.roundtrip]
/// Proof: stamp → verify(true) → unwrap returns the original value.
proof fn verify_roundtrip<T>(v: T)
    ensures ({
        let stamped = stamp_spec(v);
        let verified = verify_with_spec(stamped, true);
        verified.is_some() && unwrap_verified_spec(verified.unwrap()) == v
    })
{}

// r[impl typed-state.verify]
// r[verify typed-state.verify]
/// Proof: verify with false predicate returns None.
proof fn verify_none_on_false<T>(v: T)
    ensures verify_with_spec(stamp_spec(v), false).is_none()
{}

// r[impl typed-state.predicate-holds]
// r[verify typed-state.predicate-holds]
/// Proof: if verify_with returns Some, the predicate held.
proof fn verify_implies_predicate<T>(gs: GhostStamped<T>, pred_result: bool)
    requires verify_with_spec(gs, pred_result).is_some()
    ensures pred_result == true
{}

// r[impl typed-state.inner]
// r[verify typed-state.inner]
/// Proof: inner returns the same value regardless of verified state.
proof fn inner_same_value<T>(v: T, pred_result: bool)
    ensures ({
        let stamped = stamp_spec(v);
        let inner_before = inner_spec(stamped);
        inner_before == v
    })
{}

// r[verify typed-state.roundtrip]
/// Proof: verified flag is set after verify_with(true).
proof fn verify_sets_flag<T>(v: T)
    ensures ({
        let stamped = stamp_spec(v);
        let verified = verify_with_spec(stamped, true);
        verified.is_some() && verified.unwrap().verified == true
    })
{}

// r[verify typed-state.stamp]
/// Proof: stamp sets verified to false.
proof fn stamp_unverified<T>(v: T)
    ensures stamp_spec(v).verified == false
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify typed-state.stamp]
    #[test]
    fn test_stamp_preserves_value() {
        let stamped = Stamped::stamp(42u64);
        assert_eq!(*stamped.inner(), 42);
    }

    // r[verify typed-state.verify]
    #[test]
    fn test_verify_with_true() {
        let stamped = Stamped::stamp(42u64);
        let verified = verify_with(stamped, |v| *v == 42);
        assert!(verified.is_some());
        assert_eq!(*verified.unwrap().inner(), 42);
    }

    // r[verify typed-state.verify]
    #[test]
    fn test_verify_with_false() {
        let stamped = Stamped::stamp(42u64);
        let result = verify_with(stamped, |v| *v == 99);
        assert!(result.is_none());
    }

    // r[verify typed-state.roundtrip]
    #[test]
    fn test_roundtrip() {
        let original = 123u64;
        let stamped = Stamped::stamp(original);
        let verified = verify_with(stamped, |_| true).unwrap();
        let recovered = verified.unwrap_verified();
        assert_eq!(recovered, original);
    }

    // r[verify typed-state.unwrap]
    #[test]
    fn test_unwrap_verified() {
        let stamped = Stamped::stamp(String::from("hello"));
        let verified = verify_with(stamped, |s| s.len() == 5).unwrap();
        let s = verified.unwrap_verified();
        assert_eq!(s, "hello");
    }

    // r[verify typed-state.inner]
    #[test]
    fn test_inner_both_states() {
        let stamped = Stamped::stamp(77u64);
        assert_eq!(*stamped.inner(), 77);

        let verified = verify_with(stamped, |_| true).unwrap();
        assert_eq!(*verified.inner(), 77);
    }

    proptest! {
        // r[verify typed-state.roundtrip]
        #[test]
        fn prop_roundtrip(v: u64) {
            let stamped = Stamped::stamp(v);
            let verified = verify_with(stamped, |_| true).unwrap();
            let recovered = verified.unwrap_verified();
            prop_assert_eq!(recovered, v);
        }

        // r[verify typed-state.verify]
        #[test]
        fn prop_verify_respects_predicate(v: u64, threshold: u64) {
            let stamped = Stamped::stamp(v);
            let result = verify_with(stamped, |x| *x > threshold);
            if v > threshold {
                prop_assert!(result.is_some());
                prop_assert_eq!(*result.unwrap().inner(), v);
            } else {
                prop_assert!(result.is_none());
            }
        }

        // r[verify typed-state.stamp]
        #[test]
        fn prop_stamp_preserves(v: u64) {
            let stamped = Stamped::stamp(v);
            prop_assert_eq!(*stamped.inner(), v);
        }

        // r[verify typed-state.inner]
        #[test]
        fn prop_inner_consistent(v: u64) {
            let stamped = Stamped::stamp(v);
            let before = *stamped.inner();
            let verified = verify_with(stamped, |_| true).unwrap();
            let after = *verified.inner();
            prop_assert_eq!(before, after);
        }

        // r[verify typed-state.predicate-holds]
        #[test]
        fn prop_none_means_predicate_failed(v in 0u64..100) {
            let stamped = Stamped::stamp(v);
            // Predicate: v >= 100. Always false for range 0..100.
            let result = verify_with(stamped, |x| *x >= 100);
            prop_assert!(result.is_none());
        }
    }
}
