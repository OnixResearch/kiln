//! Verified lexicographic epoch comparison.
//!
//! An epoch is a (term, index) pair used in Raft to determine which log
//! is more up-to-date. Comparison is lexicographic: higher term wins;
//! within the same term, higher index wins. This decides vote granting
//! during elections.

#![allow(clippy::bool_to_int_with_if)]

use vstd::prelude::*;

verus! {

/// A (term, index) epoch pair.
pub struct Epoch {
    /// Election term.
    pub term: u64,
    /// Log index within the term.
    pub index: u64,
}

// --- epoch_cmp ---

// r[depends epoch.cmp]
/// Spec: lexicographic comparison of two epochs.
/// Returns -1 (a < b), 0 (a == b), or 1 (a > b).
pub open spec fn epoch_cmp_spec(a: Epoch, b: Epoch) -> int {
    if a.term < b.term {
        -1int
    } else if a.term > b.term {
        1int
    } else if a.index < b.index {
        -1int
    } else if a.index > b.index {
        1int
    } else {
        0int
    }
}

// r[impl epoch.cmp]
/// Compares two epochs lexicographically.
/// Returns -1, 0, or 1.
pub fn epoch_cmp(a: &Epoch, b: &Epoch) -> (result: i8)
    ensures result as int == epoch_cmp_spec(*a, *b)
{
    if a.term < b.term {
        -1
    } else if a.term > b.term {
        1
    } else if a.index < b.index {
        -1
    } else if a.index > b.index {
        1
    } else {
        0
    }
}

// r[verify epoch.cmp]
/// The ordering is total: exactly one of <, ==, > holds.
proof fn epoch_cmp_total(a: Epoch, b: Epoch)
    ensures
        epoch_cmp_spec(a, b) == -1
        || epoch_cmp_spec(a, b) == 0
        || epoch_cmp_spec(a, b) == 1
{}

// r[verify epoch.cmp]
/// Antisymmetric: cmp(a, b) == -cmp(b, a).
proof fn epoch_cmp_antisymmetric(a: Epoch, b: Epoch)
    ensures epoch_cmp_spec(a, b) == -epoch_cmp_spec(b, a)
{}

// r[verify epoch.cmp]
/// Equality: cmp(a, b) == 0 iff a.term == b.term && a.index == b.index.
proof fn epoch_cmp_eq(a: Epoch, b: Epoch)
    ensures
        epoch_cmp_spec(a, b) == 0 <==> (a.term == b.term && a.index == b.index)
{}

// r[verify epoch.cmp]
/// Transitive: if a < b and b < c then a < c.
proof fn epoch_cmp_transitive_lt(a: Epoch, b: Epoch, c: Epoch)
    requires epoch_cmp_spec(a, b) == -1, epoch_cmp_spec(b, c) == -1
    ensures epoch_cmp_spec(a, c) == -1
{}

// --- epoch_ge ---

// r[depends epoch.ge]
/// Spec: a is at least as up-to-date as b.
pub open spec fn epoch_ge_spec(a: Epoch, b: Epoch) -> bool {
    epoch_cmp_spec(a, b) >= 0
}

// r[impl epoch.ge]
/// Returns true if epoch `a` is >= epoch `b` (vote-granting predicate).
pub fn epoch_ge(a: &Epoch, b: &Epoch) -> (result: bool)
    ensures result == epoch_ge_spec(*a, *b)
{
    epoch_cmp(a, b) >= 0
}

// r[verify epoch.ge]
/// Reflexive: any epoch is at least as up-to-date as itself.
proof fn epoch_ge_reflexive(a: Epoch)
    ensures epoch_ge_spec(a, a)
{}

// r[verify epoch.ge]
/// Antisymmetric: a >= b and b >= a implies equality.
proof fn epoch_ge_antisymmetric(a: Epoch, b: Epoch)
    requires epoch_ge_spec(a, b), epoch_ge_spec(b, a)
    ensures a.term == b.term && a.index == b.index
{
    epoch_cmp_eq(a, b);
    epoch_cmp_antisymmetric(a, b);
}

// r[verify epoch.ge]
/// Transitive: a >= b and b >= c implies a >= c.
proof fn epoch_ge_transitive(a: Epoch, b: Epoch, c: Epoch)
    requires epoch_ge_spec(a, b), epoch_ge_spec(b, c)
    ensures epoch_ge_spec(a, c)
{}

// r[verify epoch.ge]
/// Higher term always wins regardless of index.
proof fn epoch_higher_term_wins(a: Epoch, b: Epoch)
    requires a.term > b.term
    ensures epoch_ge_spec(a, b), epoch_cmp_spec(a, b) == 1
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn e(term: u64, index: u64) -> Epoch {
        Epoch { term, index }
    }

    // r[verify epoch.cmp]
    #[test]
    fn test_epoch_cmp() {
        assert_eq!(epoch_cmp(&e(3, 5), &e(4, 0)), -1); // lower term
        assert_eq!(epoch_cmp(&e(4, 0), &e(3, 5)), 1); // higher term
        assert_eq!(epoch_cmp(&e(3, 5), &e(3, 8)), -1); // same term, lower index
        assert_eq!(epoch_cmp(&e(3, 8), &e(3, 5)), 1); // same term, higher index
        assert_eq!(epoch_cmp(&e(3, 5), &e(3, 5)), 0); // equal
        assert_eq!(epoch_cmp(&e(0, 0), &e(0, 0)), 0);
    }

    // r[verify epoch.ge]
    #[test]
    fn test_epoch_ge() {
        assert!(epoch_ge(&e(4, 0), &e(3, 100))); // higher term wins
        assert!(epoch_ge(&e(3, 5), &e(3, 5))); // equal
        assert!(epoch_ge(&e(3, 8), &e(3, 5))); // same term, higher index
        assert!(!epoch_ge(&e(3, 5), &e(3, 8))); // same term, lower index
        assert!(!epoch_ge(&e(2, 100), &e(3, 0))); // lower term loses
    }

    proptest! {
        // r[verify epoch.cmp]
        #[test]
        fn prop_epoch_cmp_antisymmetric(
            t1: u64, i1: u64, t2: u64, i2: u64,
        ) {
            let a = e(t1, i1);
            let b = e(t2, i2);
            prop_assert_eq!(
                i16::from(epoch_cmp(&a, &b)),
                -i16::from(epoch_cmp(&b, &a))
            );
        }

        // r[verify epoch.ge]
        #[test]
        fn prop_epoch_ge_reflexive(term: u64, index: u64) {
            prop_assert!(epoch_ge(&e(term, index), &e(term, index)));
        }

        // r[verify epoch.ge]
        #[test]
        fn prop_epoch_ge_antisymmetric(
            t1: u64, i1: u64, t2: u64, i2: u64,
        ) {
            let a = e(t1, i1);
            let b = e(t2, i2);
            if epoch_ge(&a, &b) && epoch_ge(&b, &a) {
                prop_assert_eq!(t1, t2);
                prop_assert_eq!(i1, i2);
            }
        }

        // r[verify epoch.ge]
        #[test]
        fn prop_higher_term_wins(
            t_hi in 1u64..=u64::MAX,
            t_lo_delta in 1u64..=1000,
            i_hi: u64,
            i_lo: u64,
        ) {
            let t_lo = t_hi.saturating_sub(t_lo_delta);
            if t_lo < t_hi {
                prop_assert!(epoch_ge(&e(t_hi, i_hi), &e(t_lo, i_lo)));
            }
        }

        // r[verify epoch.cmp]
        // r[verify epoch.ge]
        #[test]
        fn prop_epoch_total_order(
            t1: u64, i1: u64, t2: u64, i2: u64,
        ) {
            let a = e(t1, i1);
            let b = e(t2, i2);
            // exactly one of: a < b, a == b, a > b
            let cmp = epoch_cmp(&a, &b);
            prop_assert!(cmp == -1 || cmp == 0 || cmp == 1);
            prop_assert_eq!(epoch_ge(&a, &b), cmp >= 0);
        }

        // r[verify epoch.cmp]
        // r[verify epoch.ge]
        /// Transitivity oracle: generate three random epochs and verify
        /// that the ordering relation is transitive.
        #[test]
        fn prop_epoch_transitivity(
            t1: u64, i1: u64,
            t2: u64, i2: u64,
            t3: u64, i3: u64,
        ) {
            let a = e(t1, i1);
            let b = e(t2, i2);
            let c = e(t3, i3);
            // If a >= b and b >= c, then a >= c
            if epoch_ge(&a, &b) && epoch_ge(&b, &c) {
                prop_assert!(epoch_ge(&a, &c));
            }
            // If a < b and b < c, then a < c
            if epoch_cmp(&a, &b) == -1 && epoch_cmp(&b, &c) == -1 {
                prop_assert_eq!(epoch_cmp(&a, &c), -1);
            }
        }
    }
}
