//! Verified monotonic advancement predicates.
//!
//! Predicates for values that only increase — terms, commit indices,
//! sequence numbers. Proofs of reflexivity, transitivity, antisymmetry,
//! and mixed strict/non-strict composition.

use vstd::prelude::*;

verus! {

// --- is_advance ---

// r[depends monotonic.is-advance]
/// Spec: new value is at least as large as old.
pub open spec fn is_advance_spec(old: u64, new: u64) -> bool {
    new >= old
}

// r[impl monotonic.is-advance]
/// Returns true if `new >= old`.
pub fn is_advance(old: u64, new: u64) -> (result: bool)
    ensures result == is_advance_spec(old, new)
{
    new >= old
}

// r[verify monotonic.is-advance]
/// Reflexive: any value advances to itself.
proof fn advance_reflexive(x: u64)
    ensures is_advance_spec(x, x)
{}

// r[verify monotonic.is-advance]
/// Transitive: a ≤ b ≤ c implies a ≤ c.
proof fn advance_transitive(a: u64, b: u64, c: u64)
    requires is_advance_spec(a, b), is_advance_spec(b, c)
    ensures is_advance_spec(a, c)
{}

// r[verify monotonic.is-advance]
/// Antisymmetric: a ≤ b and b ≤ a implies a == b.
proof fn advance_antisymmetric(a: u64, b: u64)
    requires is_advance_spec(a, b), is_advance_spec(b, a)
    ensures a == b
{}

// --- is_strict_advance ---

// r[depends monotonic.is-strict-advance]
/// Spec: new value is strictly greater than old.
pub open spec fn is_strict_advance_spec(old: u64, new: u64) -> bool {
    new > old
}

// r[impl monotonic.is-strict-advance]
/// Returns true if `new > old`.
pub fn is_strict_advance(old: u64, new: u64) -> (result: bool)
    ensures result == is_strict_advance_spec(old, new)
{
    new > old
}

// r[verify monotonic.is-strict-advance]
/// Irreflexive: no value strictly advances to itself.
proof fn strict_advance_irreflexive(x: u64)
    ensures !is_strict_advance_spec(x, x)
{}

// r[verify monotonic.is-strict-advance]
/// Transitive: a < b < c implies a < c.
proof fn strict_advance_transitive(a: u64, b: u64, c: u64)
    requires is_strict_advance_spec(a, b), is_strict_advance_spec(b, c)
    ensures is_strict_advance_spec(a, c)
{}

// r[verify monotonic.is-strict-advance]
/// Mixed composition: a < b and b ≤ c implies a < c.
proof fn strict_then_advance(a: u64, b: u64, c: u64)
    requires is_strict_advance_spec(a, b), is_advance_spec(b, c)
    ensures is_strict_advance_spec(a, c)
{}

// r[verify monotonic.is-strict-advance]
/// Mixed composition: a ≤ b and b < c implies a < c.
proof fn advance_then_strict(a: u64, b: u64, c: u64)
    requires is_advance_spec(a, b), is_strict_advance_spec(b, c)
    ensures is_strict_advance_spec(a, c)
{}

// r[verify monotonic.is-advance]
// r[verify monotonic.is-strict-advance]
/// Strict advance implies non-strict advance.
proof fn strict_implies_advance(a: u64, b: u64)
    requires is_strict_advance_spec(a, b)
    ensures is_advance_spec(a, b)
{}

// r[verify monotonic.is-advance]
// r[verify monotonic.is-strict-advance]
/// Non-strict advance with inequality implies strict advance.
proof fn advance_neq_implies_strict(a: u64, b: u64)
    requires is_advance_spec(a, b), a != b
    ensures is_strict_advance_spec(a, b)
{}

// --- Algebraic characterization ---

// r[verify monotonic.is-advance]
/// is_advance is a partial order on u64.
proof fn advance_is_partial_order()
    ensures crate::algebraic::is_partial_order(|a: u64, b: u64| is_advance_spec(a, b))
{}

// r[verify monotonic.is-strict-advance]
/// is_strict_advance is a strict partial order on u64.
proof fn strict_advance_is_strict_partial_order()
    ensures crate::algebraic::is_strict_partial_order(|a: u64, b: u64| is_strict_advance_spec(a, b))
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify monotonic.is-advance]
    #[test]
    fn test_is_advance() {
        assert!(is_advance(0, 0));
        assert!(is_advance(0, 1));
        assert!(is_advance(5, 10));
        assert!(!is_advance(10, 5));
        assert!(is_advance(u64::MAX, u64::MAX));
    }

    // r[verify monotonic.is-strict-advance]
    #[test]
    fn test_is_strict_advance() {
        assert!(!is_strict_advance(0, 0));
        assert!(is_strict_advance(0, 1));
        assert!(is_strict_advance(5, 10));
        assert!(!is_strict_advance(10, 5));
        assert!(!is_strict_advance(u64::MAX, u64::MAX));
    }

    proptest! {
        // r[verify monotonic.is-advance]
        #[test]
        fn prop_advance_reflexive(x: u64) {
            prop_assert!(is_advance(x, x));
        }

        // r[verify monotonic.is-advance]
        #[test]
        fn prop_advance_transitive(a: u64, b: u64, c: u64) {
            if is_advance(a, b) && is_advance(b, c) {
                prop_assert!(is_advance(a, c));
            }
        }

        // r[verify monotonic.is-advance]
        #[test]
        fn prop_advance_antisymmetric(a: u64, b: u64) {
            if is_advance(a, b) && is_advance(b, a) {
                prop_assert_eq!(a, b);
            }
        }

        // r[verify monotonic.is-strict-advance]
        #[test]
        fn prop_strict_advance_irreflexive(x: u64) {
            prop_assert!(!is_strict_advance(x, x));
        }

        // r[verify monotonic.is-strict-advance]
        #[test]
        fn prop_strict_implies_advance(a: u64, b: u64) {
            if is_strict_advance(a, b) {
                prop_assert!(is_advance(a, b));
            }
        }

        // r[verify monotonic.is-strict-advance]
        #[test]
        fn prop_strict_then_advance(a: u64, b: u64, c: u64) {
            if is_strict_advance(a, b) && is_advance(b, c) {
                prop_assert!(is_strict_advance(a, c));
            }
        }
    }
}
