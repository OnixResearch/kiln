//! Verified access mode partial order for capability authorization.
//!
//! Models permission levels as a partial order over integer-tagged
//! modes. Covers mode comparison, mode permits checks, and the
//! no-escalation guarantee for delegation containment.

use vstd::prelude::*;

verus! {

// ============================================================
// Mode constants (integer-tagged for Verus compatibility)
// ============================================================

// Four modes forming a diamond lattice:
//
//       FULL (3)
//      / |  \
//   READ WRITE DELETE
//    (0)  (1)   (2)
//
// READ, WRITE, DELETE are pairwise incomparable.
// FULL subsumes all three.

/// Read-only access.
pub open spec fn MODE_READ() -> u8 { 0 }
/// Write access.
pub open spec fn MODE_WRITE() -> u8 { 1 }
/// Delete access.
pub open spec fn MODE_DELETE() -> u8 { 2 }
/// Full access (subsumes read, write, delete).
pub open spec fn MODE_FULL() -> u8 { 3 }

// r[impl access-mode.valid-mode]
// r[depends access-mode.valid-mode]
/// Spec: is this a valid mode value?
pub open spec fn is_valid_mode_spec(m: u8) -> bool {
    m == MODE_READ() || m == MODE_WRITE() || m == MODE_DELETE() || m == MODE_FULL()
}

// ============================================================
// Partial order: mode_le
// ============================================================

// r[depends access-mode.order-transitive]
/// Spec: partial order on modes. `mode_le(a, b)` means mode `a`
/// is equal to or weaker than mode `b`.
///
/// - Every mode is ≤ itself (reflexive).
/// - READ, WRITE, DELETE are each ≤ FULL.
/// - READ, WRITE, DELETE are pairwise incomparable.
pub open spec fn mode_le_spec(a: u8, b: u8) -> bool {
    a == b || b == MODE_FULL()
}

// r[impl access-mode.order-transitive]
/// Returns true if mode `a` is equal to or weaker than mode `b`.
pub fn mode_le(a: u8, b: u8) -> (result: bool)
    ensures result == mode_le_spec(a, b),
{
    a == b || b == 3
}

// r[impl access-mode.order-reflexive]
// r[verify access-mode.order-reflexive]
/// Every mode is ≤ itself.
proof fn mode_le_reflexive(a: u8)
    ensures mode_le_spec(a, a)
{}

// r[verify access-mode.order-transitive]
/// mode_le is transitive: a ≤ b ∧ b ≤ c ⟹ a ≤ c.
pub proof fn mode_le_transitive(a: u8, b: u8, c: u8)
    requires
        mode_le_spec(a, b),
        mode_le_spec(b, c),
    ensures mode_le_spec(a, c)
{
    // Case analysis: mode_le(a,b) means a==b or b==FULL.
    // If b==FULL, then mode_le(b,c) means FULL==c or c==FULL, so c==FULL, and a ≤ FULL.
    // If a==b, then mode_le(b,c) gives mode_le(a,c) directly.
}

// r[impl access-mode.order-antisymmetric]
// r[verify access-mode.order-antisymmetric]
/// mode_le is antisymmetric: a ≤ b ∧ b ≤ a ⟹ a == b.
proof fn mode_le_antisymmetric(a: u8, b: u8)
    requires
        mode_le_spec(a, b),
        mode_le_spec(b, a),
    ensures a == b
{
    // mode_le(a,b): a==b or b==FULL
    // mode_le(b,a): b==a or a==FULL
    // If a==b, done. If b==FULL and a==FULL, done. If b==FULL and b==a, done.
}

// r[verify access-mode.order-transitive]
/// Read ≤ Full, but Full is not ≤ Read.
proof fn read_lt_full()
    ensures
        mode_le_spec(MODE_READ(), MODE_FULL()),
        !mode_le_spec(MODE_FULL(), MODE_READ()),
{}

// r[verify access-mode.order-transitive]
/// Write ≤ Full, but Full is not ≤ Write.
proof fn write_lt_full()
    ensures
        mode_le_spec(MODE_WRITE(), MODE_FULL()),
        !mode_le_spec(MODE_FULL(), MODE_WRITE()),
{}

// r[verify access-mode.order-transitive]
/// Delete ≤ Full, but Full is not ≤ Delete.
proof fn delete_lt_full()
    ensures
        mode_le_spec(MODE_DELETE(), MODE_FULL()),
        !mode_le_spec(MODE_FULL(), MODE_DELETE()),
{}

// r[verify access-mode.order-antisymmetric]
/// Read, Write, Delete are pairwise incomparable.
proof fn leaf_modes_incomparable()
    ensures
        !mode_le_spec(MODE_READ(), MODE_WRITE()),
        !mode_le_spec(MODE_WRITE(), MODE_READ()),
        !mode_le_spec(MODE_READ(), MODE_DELETE()),
        !mode_le_spec(MODE_DELETE(), MODE_READ()),
        !mode_le_spec(MODE_WRITE(), MODE_DELETE()),
        !mode_le_spec(MODE_DELETE(), MODE_WRITE()),
{}

// ============================================================
// Mode permits: authorization check
// ============================================================

// r[depends access-mode.permits-ordering]
/// Spec: does the capability's mode permit the operation's mode?
/// A capability permits an operation if the operation's mode is
/// equal to or weaker than the capability's mode.
pub open spec fn mode_permits_spec(cap_mode: u8, op_mode: u8) -> bool {
    mode_le_spec(op_mode, cap_mode)
}

// r[impl access-mode.permits-ordering]
/// Returns true if the capability's mode permits the operation's mode.
pub fn mode_permits(cap_mode: u8, op_mode: u8) -> (result: bool)
    ensures result == mode_permits_spec(cap_mode, op_mode),
{
    mode_le(op_mode, cap_mode)
}

// r[verify access-mode.permits-ordering]
/// Read-only capability denies write operations.
proof fn read_denies_write()
    ensures !mode_permits_spec(MODE_READ(), MODE_WRITE())
{}

// r[verify access-mode.permits-ordering]
/// Read-only capability denies delete operations.
proof fn read_denies_delete()
    ensures !mode_permits_spec(MODE_READ(), MODE_DELETE())
{}

// r[verify access-mode.permits-ordering]
/// Full capability allows read operations.
proof fn full_allows_read()
    ensures mode_permits_spec(MODE_FULL(), MODE_READ())
{}

// r[verify access-mode.permits-ordering]
/// Full capability allows write operations.
proof fn full_allows_write()
    ensures mode_permits_spec(MODE_FULL(), MODE_WRITE())
{}

// r[verify access-mode.permits-ordering]
/// Full capability allows delete operations.
proof fn full_allows_delete()
    ensures mode_permits_spec(MODE_FULL(), MODE_DELETE())
{}

// r[verify access-mode.permits-ordering]
/// Every mode permits itself.
proof fn mode_permits_self(m: u8)
    ensures mode_permits_spec(m, m)
{}

// ============================================================
// Delegation no-escalation
// ============================================================

// r[depends access-mode.no-escalation]
/// Spec: is delegation from parent mode to child mode allowed?
/// Delegation requires the child mode to be ≤ the parent mode.
pub open spec fn delegation_allowed_spec(parent_mode: u8, child_mode: u8) -> bool {
    mode_le_spec(child_mode, parent_mode)
}

// r[impl access-mode.no-escalation]
/// Returns true if delegating from `parent_mode` to `child_mode` is allowed.
pub fn delegation_allowed(parent_mode: u8, child_mode: u8) -> (result: bool)
    ensures result == delegation_allowed_spec(parent_mode, child_mode),
{
    mode_le(child_mode, parent_mode)
}

// r[verify access-mode.no-escalation]
/// Read parent cannot delegate write to child.
proof fn no_escalation_read_write()
    ensures !delegation_allowed_spec(MODE_READ(), MODE_WRITE())
{}

// r[verify access-mode.no-escalation]
/// Full parent can delegate read to child (attenuation).
proof fn attenuation_full_read()
    ensures delegation_allowed_spec(MODE_FULL(), MODE_READ())
{}

// r[verify access-mode.no-escalation]
/// Full parent can delegate write to child.
proof fn attenuation_full_write()
    ensures delegation_allowed_spec(MODE_FULL(), MODE_WRITE())
{}

// r[verify access-mode.no-escalation]
/// Delegation allowed is transitive: if a delegates to b and b delegates to c, a can delegate to c.
proof fn delegation_transitive(a: u8, b: u8, c: u8)
    requires
        delegation_allowed_spec(a, b),
        delegation_allowed_spec(b, c),
    ensures delegation_allowed_spec(a, c)
{
    mode_le_transitive(c, b, a);
}

// r[verify access-mode.no-escalation]
/// If delegation is not allowed, the child's mode exceeds the parent's.
/// Any authorization using that child mode against the parent mode will also fail.
proof fn escalation_denies_permits(parent_mode: u8, child_mode: u8)
    requires !delegation_allowed_spec(parent_mode, child_mode)
    ensures !mode_permits_spec(parent_mode, child_mode)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify access-mode.valid-mode]
    #[test]
    fn test_valid_modes() {
        // Exactly 4 valid modes: READ(0), WRITE(1), DELETE(2), FULL(3)
        // Valid modes are self-reflexive under mode_le
        for m in [0u8, 1, 2, 3] {
            assert!(mode_le(m, m));
            assert!(mode_permits(m, m));
        }
        // FULL permits all valid modes
        for m in [0u8, 1, 2, 3] {
            assert!(mode_permits(3, m));
        }
    }

    #[test]
    fn test_mode_le_reflexive() {
        for m in [0u8, 1, 2, 3] {
            assert!(mode_le(m, m));
        }
    }

    #[test]
    fn test_mode_le_to_full() {
        for m in [0u8, 1, 2, 3] {
            assert!(mode_le(m, 3));
        }
    }

    #[test]
    fn test_leaf_incomparable() {
        assert!(!mode_le(0, 1));
        assert!(!mode_le(1, 0));
        assert!(!mode_le(0, 2));
        assert!(!mode_le(2, 0));
        assert!(!mode_le(1, 2));
        assert!(!mode_le(2, 1));
    }

    #[test]
    fn test_full_not_le_leaf() {
        assert!(!mode_le(3, 0));
        assert!(!mode_le(3, 1));
        assert!(!mode_le(3, 2));
    }

    #[test]
    fn test_mode_permits() {
        // Full permits everything
        assert!(mode_permits(3, 0));
        assert!(mode_permits(3, 1));
        assert!(mode_permits(3, 2));
        assert!(mode_permits(3, 3));
        // Self-permits
        assert!(mode_permits(0, 0));
        assert!(mode_permits(1, 1));
        assert!(mode_permits(2, 2));
        // Cross-leaf denied
        assert!(!mode_permits(0, 1));
        assert!(!mode_permits(1, 0));
        assert!(!mode_permits(0, 2));
    }

    #[test]
    fn test_delegation_allowed() {
        // Full can delegate to anything
        assert!(delegation_allowed(3, 0));
        assert!(delegation_allowed(3, 1));
        assert!(delegation_allowed(3, 2));
        assert!(delegation_allowed(3, 3));
        // Self-delegation
        assert!(delegation_allowed(0, 0));
        // No escalation
        assert!(!delegation_allowed(0, 1));
        assert!(!delegation_allowed(0, 3));
        assert!(!delegation_allowed(1, 0));
        assert!(!delegation_allowed(1, 3));
    }

    proptest! {
        // r[verify access-mode.order-reflexive]
        #[test]
        fn prop_mode_le_reflexive(m in 0u8..=3) {
            prop_assert!(mode_le(m, m));
        }

        // r[verify access-mode.order-transitive]
        #[test]
        fn prop_mode_le_transitive(a in 0u8..=3, b in 0u8..=3, c in 0u8..=3) {
            if mode_le(a, b) && mode_le(b, c) {
                prop_assert!(mode_le(a, c));
            }
        }

        // r[verify access-mode.order-antisymmetric]
        #[test]
        fn prop_mode_le_antisymmetric(a in 0u8..=3, b in 0u8..=3) {
            if mode_le(a, b) && mode_le(b, a) {
                prop_assert_eq!(a, b);
            }
        }

        // r[verify access-mode.permits-ordering]
        #[test]
        fn prop_permits_self(m in 0u8..=3) {
            prop_assert!(mode_permits(m, m));
        }

        // r[verify access-mode.permits-ordering]
        #[test]
        fn prop_permits_implies_le(cap in 0u8..=3, op in 0u8..=3) {
            // mode_permits(cap, op) iff mode_le(op, cap)
            prop_assert_eq!(mode_permits(cap, op), mode_le(op, cap));
        }

        // r[verify access-mode.no-escalation]
        #[test]
        fn prop_delegation_no_escalation(parent in 0u8..=3, child in 0u8..=3) {
            // delegation_allowed(parent, child) iff mode_le(child, parent)
            prop_assert_eq!(delegation_allowed(parent, child), mode_le(child, parent));
        }

        // r[verify access-mode.no-escalation]
        #[test]
        fn prop_escalation_denies(parent in 0u8..=3, child in 0u8..=3) {
            if !delegation_allowed(parent, child) {
                prop_assert!(!mode_permits(parent, child));
            }
        }
    }
}
