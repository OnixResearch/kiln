//! Verified signal set operations.
//!
//! Models signal masks as u64 bitmaps where each bit represents a signal
//! number [0, 64). Provides set-algebraic operations: add, remove, test
//! membership, union, intersection, complement, and the deliverable
//! computation (pending & !blocked).

use vstd::prelude::*;

verus! {

// --- signal_add ---

// r[depends signal.add]
/// Spec: set with signal added.
pub open spec fn signal_add_spec(set: u64, sig: u64) -> u64
    recommends sig < 64
{
    set | (1u64 << sig)
}

// r[impl signal.add]
/// Returns the signal set with sig added.
pub fn signal_add(set: u64, sig: u64) -> (result: u64)
    requires sig < 64
    ensures result == signal_add_spec(set, sig)
{
    set | (1u64 << sig)
}

// r[verify signal.add]
/// After adding, contains returns true.
proof fn add_then_contains(set: u64, sig: u64)
    requires sig < 64
    ensures signal_contains_spec(signal_add_spec(set, sig), sig)
{
    assert(sig < 64 ==> ((set | (1u64 << sig)) >> sig) & 1u64 == 1u64) by(bit_vector);
}

// r[verify signal.add]
/// Adding is idempotent.
proof fn add_idempotent(set: u64, sig: u64)
    requires sig < 64
    ensures signal_add_spec(signal_add_spec(set, sig), sig)
        == signal_add_spec(set, sig)
{
    assert(sig < 64 ==> (set | (1u64 << sig)) | (1u64 << sig)
        == set | (1u64 << sig)) by(bit_vector);
}

// --- signal_remove ---

// r[depends signal.remove]
/// Spec: set with signal removed.
pub open spec fn signal_remove_spec(set: u64, sig: u64) -> u64
    recommends sig < 64
{
    set & !(1u64 << sig)
}

// r[impl signal.remove]
/// Returns the signal set with sig removed.
pub fn signal_remove(set: u64, sig: u64) -> (result: u64)
    requires sig < 64
    ensures result == signal_remove_spec(set, sig)
{
    set & !(1u64 << sig)
}

// r[verify signal.remove]
/// After removing, contains returns false.
proof fn remove_then_not_contains(set: u64, sig: u64)
    requires sig < 64
    ensures !signal_contains_spec(signal_remove_spec(set, sig), sig)
{
    assert(sig < 64 ==> ((set & !(1u64 << sig)) >> sig) & 1u64 == 0u64) by(bit_vector);
}

// r[verify signal.remove]
/// Removing is idempotent.
proof fn remove_idempotent(set: u64, sig: u64)
    requires sig < 64
    ensures signal_remove_spec(signal_remove_spec(set, sig), sig)
        == signal_remove_spec(set, sig)
{
    assert(sig < 64 ==> (set & !(1u64 << sig)) & !(1u64 << sig)
        == set & !(1u64 << sig)) by(bit_vector);
}

// --- signal_contains ---

// r[depends signal.contains]
/// Spec: true when signal is present in the set.
pub open spec fn signal_contains_spec(set: u64, sig: u64) -> bool
    recommends sig < 64
{
    (set >> sig) & 1u64 == 1u64
}

// r[impl signal.contains]
/// Returns true if sig is present in the set.
pub fn signal_contains(set: u64, sig: u64) -> (result: bool)
    requires sig < 64
    ensures result == signal_contains_spec(set, sig)
{
    (set >> sig) & 1 == 1
}

// r[verify signal.contains]
/// Empty set contains nothing.
proof fn empty_contains_nothing(sig: u64)
    requires sig < 64
    ensures !signal_contains_spec(0, sig)
{
    assert(sig < 64 ==> (0u64 >> sig) & 1u64 == 0u64) by(bit_vector);
}

// --- signal_union ---

// r[depends signal.union]
/// Spec: bitwise union of two signal sets.
pub open spec fn signal_union_spec(a: u64, b: u64) -> u64 {
    a | b
}

// r[impl signal.union]
/// Returns the union of two signal sets.
pub fn signal_union(a: u64, b: u64) -> (result: u64)
    ensures result == signal_union_spec(a, b)
{
    a | b
}

// r[verify signal.union]
/// Union is commutative.
proof fn union_commutative(a: u64, b: u64)
    ensures signal_union_spec(a, b) == signal_union_spec(b, a)
{
    assert(a | b == b | a) by(bit_vector);
}

// r[verify signal.union]
/// Union with zero is identity.
proof fn union_identity(a: u64)
    ensures signal_union_spec(a, 0) == a
{
    assert(a | 0u64 == a) by(bit_vector);
}

// r[verify signal.union]
/// Union is associative.
proof fn union_associative(a: u64, b: u64, c: u64)
    ensures signal_union_spec(signal_union_spec(a, b), c)
        == signal_union_spec(a, signal_union_spec(b, c))
{
    assert((a | b) | c == a | (b | c)) by(bit_vector);
}

// --- signal_intersect ---

// r[depends signal.intersect]
/// Spec: bitwise intersection of two signal sets.
pub open spec fn signal_intersect_spec(a: u64, b: u64) -> u64 {
    a & b
}

// r[impl signal.intersect]
/// Returns the intersection of two signal sets.
pub fn signal_intersect(a: u64, b: u64) -> (result: u64)
    ensures result == signal_intersect_spec(a, b)
{
    a & b
}

// r[verify signal.intersect]
/// Intersect is commutative.
proof fn intersect_commutative(a: u64, b: u64)
    ensures signal_intersect_spec(a, b) == signal_intersect_spec(b, a)
{
    assert(a & b == b & a) by(bit_vector);
}

// r[verify signal.intersect]
/// Intersect with max is identity.
proof fn intersect_identity(a: u64)
    ensures signal_intersect_spec(a, u64::MAX) == a
{
    assert(a & u64::MAX == a) by(bit_vector);
}

// r[verify signal.intersect]
/// Intersect is associative.
proof fn intersect_associative(a: u64, b: u64, c: u64)
    ensures signal_intersect_spec(signal_intersect_spec(a, b), c)
        == signal_intersect_spec(a, signal_intersect_spec(b, c))
{
    assert((a & b) & c == a & (b & c)) by(bit_vector);
}

// --- signal_complement ---

// r[depends signal.complement]
/// Spec: bitwise complement of a signal set.
pub open spec fn signal_complement_spec(set: u64) -> u64 {
    !set
}

// r[impl signal.complement]
/// Returns the complement of a signal set.
pub fn signal_complement(set: u64) -> (result: u64)
    ensures result == signal_complement_spec(set)
{
    !set
}

// r[verify signal.complement]
/// Complement is an involution.
proof fn complement_involution(set: u64)
    ensures signal_complement_spec(signal_complement_spec(set)) == set
{
    assert(!!set == set) by(bit_vector);
}

// r[verify signal.complement]
/// De Morgan: complement of union is intersect of complements.
proof fn de_morgan_union(a: u64, b: u64)
    ensures signal_complement_spec(signal_union_spec(a, b))
        == signal_intersect_spec(signal_complement_spec(a), signal_complement_spec(b))
{
    assert(!(a | b) == !a & !b) by(bit_vector);
}

// r[verify signal.complement]
/// De Morgan: complement of intersect is union of complements.
proof fn de_morgan_intersect(a: u64, b: u64)
    ensures signal_complement_spec(signal_intersect_spec(a, b))
        == signal_union_spec(signal_complement_spec(a), signal_complement_spec(b))
{
    assert(!(a & b) == !a | !b) by(bit_vector);
}

// --- signal_deliverable ---

// r[depends signal.deliverable]
/// Spec: signals that are pending and not blocked.
pub open spec fn signal_deliverable_spec(pending: u64, blocked: u64) -> u64 {
    pending & !blocked
}

// r[impl signal.deliverable]
/// Returns the set of deliverable signals: pending and not blocked.
pub fn signal_deliverable(pending: u64, blocked: u64) -> (result: u64)
    ensures result == signal_deliverable_spec(pending, blocked)
{
    pending & !blocked
}

// r[verify signal.deliverable]
/// Deliverable is a subset of pending.
proof fn deliverable_subset_of_pending(pending: u64, blocked: u64)
    ensures signal_deliverable_spec(pending, blocked) & pending
        == signal_deliverable_spec(pending, blocked)
{
    assert((pending & !blocked) & pending == pending & !blocked) by(bit_vector);
}

// r[verify signal.deliverable]
/// Deliverable has no signals from blocked.
proof fn deliverable_disjoint_from_blocked(pending: u64, blocked: u64)
    ensures signal_deliverable_spec(pending, blocked) & blocked == 0
{
    assert((pending & !blocked) & blocked == 0u64) by(bit_vector);
}

// r[verify signal.deliverable]
/// Nothing blocked means all pending are deliverable.
proof fn nothing_blocked_all_deliverable(pending: u64)
    ensures signal_deliverable_spec(pending, 0) == pending
{
    assert(pending & !0u64 == pending) by(bit_vector);
}

// r[verify signal.deliverable]
/// Everything blocked means nothing deliverable.
proof fn everything_blocked_nothing_deliverable(pending: u64)
    ensures signal_deliverable_spec(pending, u64::MAX) == 0
{
    assert(pending & !u64::MAX == 0u64) by(bit_vector);
}

// --- add/remove roundtrip ---

// r[verify signal.add]
// r[verify signal.remove]
/// Add then remove leaves the signal absent.
proof fn add_remove_roundtrip(set: u64, sig: u64)
    requires sig < 64
    ensures !signal_contains_spec(
        signal_remove_spec(signal_add_spec(set, sig), sig), sig)
{
    assert(sig < 64 ==>
        (((set | (1u64 << sig)) & !(1u64 << sig)) >> sig) & 1u64 == 0u64
    ) by(bit_vector);
}

// r[verify signal.remove]
// r[verify signal.add]
/// Remove then add leaves the signal present.
proof fn remove_add_roundtrip(set: u64, sig: u64)
    requires sig < 64
    ensures signal_contains_spec(
        signal_add_spec(signal_remove_spec(set, sig), sig), sig)
{
    assert(sig < 64 ==>
        (((set & !(1u64 << sig)) | (1u64 << sig)) >> sig) & 1u64 == 1u64
    ) by(bit_vector);
}

// --- Algebraic characterization ---

// r[verify signal.union]
/// signal_union is a semilattice (commutative, associative, idempotent).
proof fn union_is_semilattice()
    ensures crate::algebraic::is_semilattice(|a: u64, b: u64| signal_union_spec(a, b))
{
    assert forall|a: u64, b: u64| #[trigger] signal_union_spec(a, b) == signal_union_spec(b, a) by {
        assert(a | b == b | a) by(bit_vector);
    }
    assert forall|a: u64, b: u64, c: u64|
        #[trigger] signal_union_spec(signal_union_spec(a, b), c)
        == signal_union_spec(a, signal_union_spec(b, c))
    by {
        assert((a | b) | c == a | (b | c)) by(bit_vector);
    }
    assert forall|a: u64| #![trigger signal_union_spec(a, a)]
        signal_union_spec(a, a) == a
    by {
        assert(a | a == a) by(bit_vector);
    }
}

// r[verify signal.intersect]
/// signal_intersect is a semilattice (commutative, associative, idempotent).
proof fn intersect_is_semilattice()
    ensures crate::algebraic::is_semilattice(|a: u64, b: u64| signal_intersect_spec(a, b))
{
    assert forall|a: u64, b: u64| #[trigger] signal_intersect_spec(a, b) == signal_intersect_spec(b, a) by {
        assert(a & b == b & a) by(bit_vector);
    }
    assert forall|a: u64, b: u64, c: u64|
        #[trigger] signal_intersect_spec(signal_intersect_spec(a, b), c)
        == signal_intersect_spec(a, signal_intersect_spec(b, c))
    by {
        assert((a & b) & c == a & (b & c)) by(bit_vector);
    }
    assert forall|a: u64| #![trigger signal_intersect_spec(a, a)]
        signal_intersect_spec(a, a) == a
    by {
        assert(a & a == a) by(bit_vector);
    }
}

// r[verify signal.complement]
/// signal_complement is an involution (per algebraic::is_involution).
proof fn complement_is_involution()
    ensures crate::algebraic::is_involution(|s: u64| signal_complement_spec(s))
{
    assert forall|s: u64| #![trigger signal_complement_spec(signal_complement_spec(s))]
        signal_complement_spec(signal_complement_spec(s)) == s
    by {
        assert(!!s == s) by(bit_vector);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify signal.add]
    // r[verify signal.contains]
    #[test]
    fn test_add_contains() {
        let set = signal_add(0, 5);
        assert!(signal_contains(set, 5));
        assert!(!signal_contains(set, 4));
        assert!(!signal_contains(set, 6));
    }

    // r[verify signal.remove]
    // r[verify signal.contains]
    #[test]
    fn test_remove() {
        let set = signal_add(signal_add(0, 3), 7);
        assert!(signal_contains(set, 3));
        assert!(signal_contains(set, 7));
        let set = signal_remove(set, 3);
        assert!(!signal_contains(set, 3));
        assert!(signal_contains(set, 7));
    }

    // r[verify signal.union]
    #[test]
    fn test_union() {
        let a = signal_add(signal_add(0, 1), 2);
        let b = signal_add(signal_add(0, 2), 3);
        let u = signal_union(a, b);
        assert!(signal_contains(u, 1));
        assert!(signal_contains(u, 2));
        assert!(signal_contains(u, 3));
    }

    // r[verify signal.intersect]
    #[test]
    fn test_intersect() {
        let a = signal_add(signal_add(0, 1), 2);
        let b = signal_add(signal_add(0, 2), 3);
        let i = signal_intersect(a, b);
        assert!(!signal_contains(i, 1));
        assert!(signal_contains(i, 2));
        assert!(!signal_contains(i, 3));
    }

    // r[verify signal.complement]
    #[test]
    fn test_complement() {
        let set = signal_add(0, 10);
        let comp = signal_complement(set);
        assert!(!signal_contains(comp, 10));
        assert!(signal_contains(comp, 0));
        assert!(signal_contains(comp, 63));
    }

    // r[verify signal.deliverable]
    #[test]
    fn test_deliverable() {
        let pending = signal_add(signal_add(0, 1), 2);
        let blocked = signal_add(0, 2);
        let d = signal_deliverable(pending, blocked);
        assert!(signal_contains(d, 1));
        assert!(!signal_contains(d, 2));
    }

    proptest! {
        // r[verify signal.add]
        // r[verify signal.contains]
        #[test]
        fn prop_add_then_contains(set: u64, sig in 0u64..64) {
            prop_assert!(signal_contains(signal_add(set, sig), sig));
        }

        // r[verify signal.remove]
        // r[verify signal.contains]
        #[test]
        fn prop_remove_then_not_contains(set: u64, sig in 0u64..64) {
            prop_assert!(!signal_contains(signal_remove(set, sig), sig));
        }

        // r[verify signal.add]
        #[test]
        fn prop_add_idempotent(set: u64, sig in 0u64..64) {
            let once = signal_add(set, sig);
            let twice = signal_add(once, sig);
            prop_assert_eq!(once, twice);
        }

        // r[verify signal.remove]
        #[test]
        fn prop_remove_idempotent(set: u64, sig in 0u64..64) {
            let once = signal_remove(set, sig);
            let twice = signal_remove(once, sig);
            prop_assert_eq!(once, twice);
        }

        // r[verify signal.union]
        #[test]
        fn prop_union_commutative(a: u64, b: u64) {
            prop_assert_eq!(signal_union(a, b), signal_union(b, a));
        }

        // r[verify signal.intersect]
        #[test]
        fn prop_intersect_commutative(a: u64, b: u64) {
            prop_assert_eq!(signal_intersect(a, b), signal_intersect(b, a));
        }

        // r[verify signal.complement]
        #[test]
        fn prop_complement_involution(set: u64) {
            prop_assert_eq!(signal_complement(signal_complement(set)), set);
        }

        // r[verify signal.deliverable]
        #[test]
        fn prop_deliverable_subset_of_pending(pending: u64, blocked: u64) {
            let d = signal_deliverable(pending, blocked);
            prop_assert_eq!(d & pending, d);
        }

        // r[verify signal.deliverable]
        #[test]
        fn prop_deliverable_disjoint_from_blocked(pending: u64, blocked: u64) {
            let d = signal_deliverable(pending, blocked);
            prop_assert_eq!(d & blocked, 0);
        }

        // r[verify signal.complement]
        #[test]
        fn prop_de_morgan_union(a: u64, b: u64) {
            prop_assert_eq!(
                signal_complement(signal_union(a, b)),
                signal_intersect(signal_complement(a), signal_complement(b))
            );
        }

        // r[verify signal.add]
        // r[verify signal.remove]
        // r[verify signal.contains]
        // r[verify signal.union]
        // r[verify signal.intersect]
        // r[verify signal.complement]
        // r[verify signal.deliverable]
        /// Model oracle: track a BTreeSet alongside signal bitmask.
        #[test]
        fn prop_model_oracle(
            ops in proptest::collection::vec(
                (0u8..4, 0u64..64),
                0..=100
            )
        ) {
            let mut bitmask = 0u64;
            let mut oracle = std::collections::BTreeSet::new();

            for &(op, sig) in &ops {
                match op {
                    0 => {
                        bitmask = signal_add(bitmask, sig);
                        oracle.insert(sig);
                    }
                    1 => {
                        bitmask = signal_remove(bitmask, sig);
                        oracle.remove(&sig);
                    }
                    _ => {
                        // Check membership matches
                        let bm_has = signal_contains(bitmask, sig);
                        let or_has = oracle.contains(&sig);
                        prop_assert_eq!(bm_has, or_has);
                    }
                }
            }

            // Full scan
            for s in 0u64..64 {
                prop_assert_eq!(
                    signal_contains(bitmask, s),
                    oracle.contains(&s)
                );
            }
        }
    }
}
