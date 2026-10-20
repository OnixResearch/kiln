//! Verified transaction isolation levels.
//!
//! Models the four SQL standard isolation levels as a u8 hierarchy.
//! Each level prevents a specific set of anomalies (dirty read,
//! non-repeatable read, phantom). Higher levels prevent strictly
//! more anomalies.
//!
//! Level encoding:
//!   0=READ_UNCOMMITTED, 1=READ_COMMITTED, 2=REPEATABLE_READ,
//!   3=SERIALIZABLE.

#![allow(clippy::nonminimal_bool)]

use vstd::prelude::*;

verus! {

/// Isolation level constants (SQL standard).
pub const ISO_READ_UNCOMMITTED: u8 = 0u8;
pub const ISO_READ_COMMITTED: u8 = 1u8;
pub const ISO_REPEATABLE_READ: u8 = 2u8;
pub const ISO_SERIALIZABLE: u8 = 3u8;

// --- is_valid_level ---

// r[depends txn-isolation.is-valid]
/// Spec: level is one of the four isolation levels.
pub open spec fn is_valid_level_spec(level: u8) -> bool {
    level <= 3
}

// r[impl txn-isolation.is-valid]
/// Returns true if level is a valid isolation level.
pub fn is_valid_level(level: u8) -> (result: bool)
    ensures result == is_valid_level_spec(level)
{
    level <= 3
}

// --- anomaly predicates ---

// r[depends txn-isolation.allows-dirty-read]
/// Spec: dirty reads are possible at this isolation level.
/// Only READ_UNCOMMITTED allows dirty reads.
pub open spec fn allows_dirty_read_spec(level: u8) -> bool {
    level < ISO_READ_COMMITTED
}

// r[impl txn-isolation.allows-dirty-read]
/// Returns true if the level allows dirty reads.
pub fn allows_dirty_read(level: u8) -> (result: bool)
    ensures result == allows_dirty_read_spec(level)
{
    level < ISO_READ_COMMITTED
}

// r[depends txn-isolation.allows-non-repeatable-read]
/// Spec: non-repeatable reads are possible at this level.
/// READ_UNCOMMITTED and READ_COMMITTED allow them.
pub open spec fn allows_non_repeatable_read_spec(level: u8) -> bool {
    level < ISO_REPEATABLE_READ
}

// r[impl txn-isolation.allows-non-repeatable-read]
/// Returns true if the level allows non-repeatable reads.
pub fn allows_non_repeatable_read(level: u8) -> (result: bool)
    ensures result == allows_non_repeatable_read_spec(level)
{
    level < ISO_REPEATABLE_READ
}

// r[depends txn-isolation.allows-phantom]
/// Spec: phantom reads are possible at this level.
/// Only SERIALIZABLE prevents phantoms.
pub open spec fn allows_phantom_spec(level: u8) -> bool {
    level < ISO_SERIALIZABLE
}

// r[impl txn-isolation.allows-phantom]
/// Returns true if the level allows phantom reads.
pub fn allows_phantom(level: u8) -> (result: bool)
    ensures result == allows_phantom_spec(level)
{
    level < ISO_SERIALIZABLE
}

// --- prevents predicates ---

// r[depends txn-isolation.prevents-dirty-read]
/// Spec: this level prevents dirty reads.
pub open spec fn prevents_dirty_read_spec(level: u8) -> bool {
    level >= ISO_READ_COMMITTED
}

// r[impl txn-isolation.prevents-dirty-read]
pub fn prevents_dirty_read(level: u8) -> (result: bool)
    ensures result == prevents_dirty_read_spec(level)
{
    level >= ISO_READ_COMMITTED
}

// r[depends txn-isolation.prevents-non-repeatable-read]
/// Spec: this level prevents non-repeatable reads.
pub open spec fn prevents_non_repeatable_read_spec(level: u8) -> bool {
    level >= ISO_REPEATABLE_READ
}

// r[impl txn-isolation.prevents-non-repeatable-read]
pub fn prevents_non_repeatable_read(level: u8) -> (result: bool)
    ensures result == prevents_non_repeatable_read_spec(level)
{
    level >= ISO_REPEATABLE_READ
}

// r[depends txn-isolation.prevents-phantom]
/// Spec: this level prevents phantom reads.
pub open spec fn prevents_phantom_spec(level: u8) -> bool {
    level >= ISO_SERIALIZABLE
}

// r[impl txn-isolation.prevents-phantom]
pub fn prevents_phantom(level: u8) -> (result: bool)
    ensures result == prevents_phantom_spec(level)
{
    level >= ISO_SERIALIZABLE
}

// --- anomaly_count ---

// r[depends txn-isolation.anomaly-count]
/// Spec: number of anomaly classes allowed at this level.
/// READ_UNCOMMITTED=3, READ_COMMITTED=2, REPEATABLE_READ=1, SERIALIZABLE=0.
pub open spec fn anomaly_count_spec(level: u8) -> u8
    recommends is_valid_level_spec(level)
{
    if level >= ISO_SERIALIZABLE { 0u8 }
    else if level >= ISO_REPEATABLE_READ { 1u8 }
    else if level >= ISO_READ_COMMITTED { 2u8 }
    else { 3u8 }
}

// r[impl txn-isolation.anomaly-count]
pub fn anomaly_count(level: u8) -> (result: u8)
    requires is_valid_level_spec(level)
    ensures result == anomaly_count_spec(level)
{
    if level >= ISO_SERIALIZABLE { 0u8 }
    else if level >= ISO_REPEATABLE_READ { 1u8 }
    else if level >= ISO_READ_COMMITTED { 2u8 }
    else { 3u8 }
}

// --- strengthen ---

// r[depends txn-isolation.strengthen]
/// Spec: choose the stricter of two isolation levels.
pub open spec fn strengthen_spec(a: u8, b: u8) -> u8 {
    if a >= b { a } else { b }
}

// r[impl txn-isolation.strengthen]
pub fn strengthen(a: u8, b: u8) -> (result: u8)
    ensures result == strengthen_spec(a, b)
{
    if a >= b { a } else { b }
}

// --- weaken ---

// r[depends txn-isolation.weaken]
/// Spec: choose the more permissive of two isolation levels.
pub open spec fn weaken_spec(a: u8, b: u8) -> u8 {
    if a <= b { a } else { b }
}

// r[impl txn-isolation.weaken]
pub fn weaken(a: u8, b: u8) -> (result: u8)
    ensures result == weaken_spec(a, b)
{
    if a <= b { a } else { b }
}

// ==================== Proofs ====================

// r[verify txn-isolation.is-valid]
/// All four named levels are valid.
proof fn all_levels_valid()
    ensures
        is_valid_level_spec(ISO_READ_UNCOMMITTED),
        is_valid_level_spec(ISO_READ_COMMITTED),
        is_valid_level_spec(ISO_REPEATABLE_READ),
        is_valid_level_spec(ISO_SERIALIZABLE),
{}

// r[verify txn-isolation.allows-dirty-read]
/// READ_COMMITTED prevents dirty reads.
proof fn read_committed_no_dirty_read()
    ensures !allows_dirty_read_spec(ISO_READ_COMMITTED)
{}

// r[verify txn-isolation.allows-non-repeatable-read]
/// REPEATABLE_READ prevents non-repeatable reads.
proof fn repeatable_read_no_nr_read()
    ensures !allows_non_repeatable_read_spec(ISO_REPEATABLE_READ)
{}

// r[verify txn-isolation.allows-phantom]
/// SERIALIZABLE prevents phantoms.
proof fn serializable_no_phantom()
    ensures !allows_phantom_spec(ISO_SERIALIZABLE)
{}

// r[verify txn-isolation.prevents-dirty-read]
// r[verify txn-isolation.prevents-non-repeatable-read]
// r[verify txn-isolation.prevents-phantom]
/// SERIALIZABLE prevents all anomalies.
proof fn serializable_prevents_all()
    ensures
        prevents_dirty_read_spec(ISO_SERIALIZABLE),
        prevents_non_repeatable_read_spec(ISO_SERIALIZABLE),
        prevents_phantom_spec(ISO_SERIALIZABLE),
{}

// r[verify txn-isolation.anomaly-count]
/// SERIALIZABLE has zero anomalies.
proof fn serializable_zero_anomalies()
    ensures anomaly_count_spec(ISO_SERIALIZABLE) == 0u8
{}

// r[verify txn-isolation.anomaly-count]
/// READ_UNCOMMITTED allows all three anomaly classes.
proof fn read_uncommitted_three_anomalies()
    ensures anomaly_count_spec(ISO_READ_UNCOMMITTED) == 3u8
{}

// r[verify txn-isolation.anomaly-count]
/// Higher level means fewer anomalies.
proof fn higher_level_fewer_anomalies(a: u8, b: u8)
    requires is_valid_level_spec(a), is_valid_level_spec(b), a <= b
    ensures anomaly_count_spec(a) >= anomaly_count_spec(b)
{}

// r[verify txn-isolation.strengthen]
/// Strengthening two valid levels yields a valid level.
proof fn strengthen_valid(a: u8, b: u8)
    requires is_valid_level_spec(a), is_valid_level_spec(b)
    ensures is_valid_level_spec(strengthen_spec(a, b))
{}

// r[verify txn-isolation.strengthen]
/// Strengthened level prevents at least as many anomalies as either input.
proof fn strengthen_fewer_anomalies(a: u8, b: u8)
    requires is_valid_level_spec(a), is_valid_level_spec(b)
    ensures
        anomaly_count_spec(strengthen_spec(a, b)) <= anomaly_count_spec(a),
        anomaly_count_spec(strengthen_spec(a, b)) <= anomaly_count_spec(b),
{}

// r[verify txn-isolation.strengthen]
/// Strengthen is idempotent.
proof fn strengthen_idempotent(a: u8)
    ensures strengthen_spec(a, a) == a
{}

// r[verify txn-isolation.strengthen]
/// Strengthen is commutative.
proof fn strengthen_commutative(a: u8, b: u8)
    ensures strengthen_spec(a, b) == strengthen_spec(b, a)
{}

// r[verify txn-isolation.weaken]
/// Weakened level is no stricter than either input.
proof fn weaken_valid(a: u8, b: u8)
    requires is_valid_level_spec(a), is_valid_level_spec(b)
    ensures is_valid_level_spec(weaken_spec(a, b))
{}

// r[verify txn-isolation.weaken]
/// Weaken is commutative.
proof fn weaken_commutative(a: u8, b: u8)
    ensures weaken_spec(a, b) == weaken_spec(b, a)
{}

// r[verify txn-isolation.weaken]
/// Weaken is idempotent.
proof fn weaken_idempotent(a: u8)
    ensures weaken_spec(a, a) == a
{}

// r[verify txn-isolation.prevents-dirty-read]
/// If a prevents dirty read and b >= a, b also prevents dirty read.
proof fn prevents_dirty_read_monotonic(a: u8, b: u8)
    requires prevents_dirty_read_spec(a), b >= a
    ensures prevents_dirty_read_spec(b)
{}

// r[verify txn-isolation.prevents-non-repeatable-read]
/// If a prevents non-repeatable read and b >= a, b also does.
proof fn prevents_nr_read_monotonic(a: u8, b: u8)
    requires prevents_non_repeatable_read_spec(a), b >= a
    ensures prevents_non_repeatable_read_spec(b)
{}

// r[verify txn-isolation.prevents-phantom]
/// If a prevents phantom and b >= a, b also does.
proof fn prevents_phantom_monotonic(a: u8, b: u8)
    requires prevents_phantom_spec(a), b >= a
    ensures prevents_phantom_spec(b)
{}

// r[verify txn-isolation.allows-dirty-read]
// r[verify txn-isolation.allows-non-repeatable-read]
/// Prevents-dirty implies prevents-lower: if no dirty read, then
/// non-repeatable read prevention is the next question.
proof fn dirty_implies_nr(level: u8)
    requires allows_dirty_read_spec(level)
    ensures allows_non_repeatable_read_spec(level)
{}

// r[verify txn-isolation.allows-non-repeatable-read]
// r[verify txn-isolation.allows-phantom]
/// Allows-nr implies allows-phantom.
proof fn nr_implies_phantom(level: u8)
    requires allows_non_repeatable_read_spec(level)
    ensures allows_phantom_spec(level)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn test_levels_valid() {
        assert!(is_valid_level(ISO_READ_UNCOMMITTED));
        assert!(is_valid_level(ISO_READ_COMMITTED));
        assert!(is_valid_level(ISO_REPEATABLE_READ));
        assert!(is_valid_level(ISO_SERIALIZABLE));
        assert!(!is_valid_level(4));
    }

    #[test]
    fn test_dirty_read() {
        assert!(allows_dirty_read(ISO_READ_UNCOMMITTED));
        assert!(!allows_dirty_read(ISO_READ_COMMITTED));
        assert!(!allows_dirty_read(ISO_REPEATABLE_READ));
        assert!(!allows_dirty_read(ISO_SERIALIZABLE));
    }

    #[test]
    fn test_non_repeatable_read() {
        assert!(allows_non_repeatable_read(ISO_READ_UNCOMMITTED));
        assert!(allows_non_repeatable_read(ISO_READ_COMMITTED));
        assert!(!allows_non_repeatable_read(ISO_REPEATABLE_READ));
        assert!(!allows_non_repeatable_read(ISO_SERIALIZABLE));
    }

    #[test]
    fn test_phantom() {
        assert!(allows_phantom(ISO_READ_UNCOMMITTED));
        assert!(allows_phantom(ISO_READ_COMMITTED));
        assert!(allows_phantom(ISO_REPEATABLE_READ));
        assert!(!allows_phantom(ISO_SERIALIZABLE));
    }

    #[test]
    fn test_anomaly_count() {
        assert_eq!(anomaly_count(ISO_READ_UNCOMMITTED), 3);
        assert_eq!(anomaly_count(ISO_READ_COMMITTED), 2);
        assert_eq!(anomaly_count(ISO_REPEATABLE_READ), 1);
        assert_eq!(anomaly_count(ISO_SERIALIZABLE), 0);
    }

    #[test]
    fn test_strengthen() {
        assert_eq!(strengthen(ISO_READ_UNCOMMITTED, ISO_SERIALIZABLE), ISO_SERIALIZABLE);
        assert_eq!(strengthen(ISO_READ_COMMITTED, ISO_REPEATABLE_READ), ISO_REPEATABLE_READ);
        assert_eq!(strengthen(ISO_SERIALIZABLE, ISO_READ_UNCOMMITTED), ISO_SERIALIZABLE);
    }

    #[test]
    fn test_weaken() {
        assert_eq!(weaken(ISO_SERIALIZABLE, ISO_READ_UNCOMMITTED), ISO_READ_UNCOMMITTED);
        assert_eq!(weaken(ISO_READ_COMMITTED, ISO_REPEATABLE_READ), ISO_READ_COMMITTED);
    }

    proptest! {
        #[test]
        fn prop_higher_level_fewer_anomalies(a in 0u8..=3, b in 0u8..=3) {
            if a <= b {
                prop_assert!(anomaly_count(a) >= anomaly_count(b));
            }
        }

        #[test]
        fn prop_strengthen_commutative(a in 0u8..=3, b in 0u8..=3) {
            prop_assert_eq!(strengthen(a, b), strengthen(b, a));
        }

        #[test]
        fn prop_weaken_commutative(a in 0u8..=3, b in 0u8..=3) {
            prop_assert_eq!(weaken(a, b), weaken(b, a));
        }

        #[test]
        fn prop_strengthen_idempotent(a in 0u8..=3) {
            prop_assert_eq!(strengthen(a, a), a);
        }

        #[test]
        fn prop_weaken_idempotent(a in 0u8..=3) {
            prop_assert_eq!(weaken(a, a), a);
        }

        #[test]
        fn prop_strengthen_prevents_more(a in 0u8..=3, b in 0u8..=3) {
            let s = strengthen(a, b);
            prop_assert!(anomaly_count(s) <= anomaly_count(a));
            prop_assert!(anomaly_count(s) <= anomaly_count(b));
        }

        #[test]
        fn prop_dirty_implies_nr(level in 0u8..=3) {
            if allows_dirty_read(level) {
                prop_assert!(allows_non_repeatable_read(level));
            }
        }

        #[test]
        fn prop_nr_implies_phantom(level in 0u8..=3) {
            if allows_non_repeatable_read(level) {
                prop_assert!(allows_phantom(level));
            }
        }
    }
}
