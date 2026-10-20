//! Verified trust level resolution and access control.
//!
//! Models the trust manager's decision logic: blocked status takes
//! precedence over trusted, and access control depends on trust level
//! and resource visibility (public vs private).
//!
//! Three levels: Blocked (deny all), Trusted (allow all), Public
//! (allow only public resources).

use vstd::prelude::*;

verus! {

/// Trust level constants (u8 tags — Verus can't match on enum in specs).
pub const BLOCKED: u8 = 0u8;
pub const TRUSTED: u8 = 1u8;
pub const PUBLIC: u8 = 2u8;

// --- resolve_trust ---

// r[depends trust-level.resolve]
/// Spec: resolve trust level from membership in blocked/trusted sets.
/// Blocked takes precedence over trusted.
pub open spec fn resolve_trust_spec(in_blocked: bool, in_trusted: bool) -> u8 {
    if in_blocked {
        BLOCKED
    } else if in_trusted {
        TRUSTED
    } else {
        PUBLIC
    }
}

// r[impl trust-level.resolve]
/// Resolve trust level from set membership.
pub fn resolve_trust(in_blocked: bool, in_trusted: bool) -> (result: u8)
    ensures result == resolve_trust_spec(in_blocked, in_trusted)
{
    if in_blocked {
        BLOCKED
    } else if in_trusted {
        TRUSTED
    } else {
        PUBLIC
    }
}

// --- can_access ---

// r[depends trust-level.can-access]
/// Spec: access control based on trust level and resource visibility.
pub open spec fn can_access_spec(level: u8, is_public: bool) -> bool {
    if level == BLOCKED {
        false
    } else if level == TRUSTED {
        true
    } else {
        // PUBLIC — only if resource is public
        is_public
    }
}

// r[impl trust-level.can-access]
/// Check whether a node at the given trust level can access a resource.
pub fn can_access(level: u8, is_public: bool) -> (result: bool)
    ensures result == can_access_spec(level, is_public)
{
    if level == BLOCKED {
        false
    } else if level == TRUSTED {
        true
    } else {
        is_public
    }
}

// --- add/block operations (spec-level mutual exclusivity) ---

// r[depends trust-level.mutual-exclusivity]
/// Spec: after adding to trusted, the node is trusted (not blocked).
pub open spec fn after_add_trusted_spec(in_blocked: bool) -> (bool, bool) {
    // Remove from blocked, add to trusted
    (false, true)
}

// r[impl trust-level.mutual-exclusivity]
/// Adding a node to trusted removes it from blocked.
pub fn after_add_trusted(_in_blocked: bool) -> (result: (bool, bool))
    ensures
        result.0 == after_add_trusted_spec(_in_blocked).0,
        result.1 == after_add_trusted_spec(_in_blocked).1,
{
    (false, true)
}

// r[depends trust-level.mutual-exclusivity]
/// Spec: after blocking, the node is blocked (not trusted).
pub open spec fn after_block_spec(in_trusted: bool) -> (bool, bool) {
    // Add to blocked, remove from trusted
    (true, false)
}

// r[impl trust-level.mutual-exclusivity]
/// Blocking a node removes it from trusted.
pub fn after_block(_in_trusted: bool) -> (result: (bool, bool))
    ensures
        result.0 == after_block_spec(_in_trusted).0,
        result.1 == after_block_spec(_in_trusted).1,
{
    (true, false)
}

// --- Capacity predicates ---

/// Spec: a set count is within capacity.
pub open spec fn is_bounded_spec(count: u64, capacity: u64) -> bool {
    count <= capacity
}

// r[impl trust-level.capacity-bounded]
/// Check if a set count is within capacity.
pub fn is_bounded(count: u64, capacity: u64) -> (result: bool)
    ensures result == is_bounded_spec(count, capacity)
{
    count <= capacity
}

/// Spec: adding one element stays within capacity when count < capacity.
pub open spec fn add_stays_bounded_spec(count: u64, capacity: u64) -> bool {
    count < capacity
}

// --- Safety proofs ---

// r[verify trust-level.mutual-exclusivity]
/// Adding to trusted while under capacity maintains the bound.
proof fn add_trusted_maintains_bound(blocked_count: u64, trusted_count: u64, capacity: u64)
    requires
        is_bounded_spec(trusted_count, capacity),
        add_stays_bounded_spec(trusted_count, capacity),
    ensures is_bounded_spec((trusted_count + 1) as u64, capacity)
{}

// r[verify trust-level.mutual-exclusivity]
/// Blocking while under capacity maintains the bound (trusted decrements, blocked increments).
proof fn block_maintains_bound(
    blocked_count: u64, trusted_count: u64, blocked_cap: u64,
)
    requires
        is_bounded_spec(blocked_count, blocked_cap),
        add_stays_bounded_spec(blocked_count, blocked_cap),
        trusted_count > 0,
    ensures
        is_bounded_spec((blocked_count + 1) as u64, blocked_cap),
{}

// r[impl trust-level.block-precedence]
// r[verify trust-level.block-precedence]
/// Blocked always wins regardless of trusted status.
proof fn block_precedence(in_trusted: bool)
    ensures resolve_trust_spec(true, in_trusted) == BLOCKED
{}

// r[verify trust-level.resolve]
/// Not blocked and trusted → TRUSTED.
proof fn trusted_when_not_blocked()
    ensures resolve_trust_spec(false, true) == TRUSTED
{}

// r[verify trust-level.resolve]
/// Neither blocked nor trusted → PUBLIC.
proof fn public_when_neither()
    ensures resolve_trust_spec(false, false) == PUBLIC
{}

// r[verify trust-level.can-access]
/// Blocked never gets access.
proof fn blocked_denies_all(is_public: bool)
    ensures !can_access_spec(BLOCKED, is_public)
{}

// r[verify trust-level.can-access]
/// Trusted always gets access.
proof fn trusted_allows_all(is_public: bool)
    ensures can_access_spec(TRUSTED, is_public)
{}

// r[verify trust-level.can-access]
/// Public access depends on resource visibility.
proof fn public_conditional(is_public: bool)
    ensures can_access_spec(PUBLIC, is_public) == is_public
{}

// r[verify trust-level.mutual-exclusivity]
/// After add_trusted, resolve gives TRUSTED.
proof fn add_trusted_resolves_trusted(was_blocked: bool)
    ensures
        resolve_trust_spec(
            after_add_trusted_spec(was_blocked).0,
            after_add_trusted_spec(was_blocked).1,
        ) == TRUSTED,
{}

// r[verify trust-level.mutual-exclusivity]
/// After block, resolve gives BLOCKED.
proof fn block_resolves_blocked(was_trusted: bool)
    ensures
        resolve_trust_spec(
            after_block_spec(was_trusted).0,
            after_block_spec(was_trusted).1,
        ) == BLOCKED,
{}

// r[impl trust-level.can-access]
// r[verify trust-level.can-access]
/// Access control is exhaustive: every (level, is_public) pair is classified.
proof fn access_exhaustive(level: u8, is_public: bool)
    ensures
        level == BLOCKED ==> !can_access_spec(level, is_public),
        level == TRUSTED ==> can_access_spec(level, is_public),
        level == PUBLIC ==> can_access_spec(level, is_public) == is_public,
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify trust-level.resolve]
    // r[verify trust-level.capacity-bounded]
    #[test]
    fn test_resolve() {
        assert_eq!(resolve_trust(true, true), BLOCKED);
        assert_eq!(resolve_trust(true, false), BLOCKED);
        assert_eq!(resolve_trust(false, true), TRUSTED);
        assert_eq!(resolve_trust(false, false), PUBLIC);
    }

    // r[verify trust-level.can-access]
    #[test]
    fn test_access() {
        assert!(!can_access(BLOCKED, true));
        assert!(!can_access(BLOCKED, false));
        assert!(can_access(TRUSTED, true));
        assert!(can_access(TRUSTED, false));
        assert!(can_access(PUBLIC, true));
        assert!(!can_access(PUBLIC, false));
    }

    // r[verify trust-level.mutual-exclusivity]
    #[test]
    fn test_mutual_exclusivity() {
        let (b, t) = after_add_trusted(true);
        assert_eq!(resolve_trust(b, t), TRUSTED);
        let (b, t) = after_block(true);
        assert_eq!(resolve_trust(b, t), BLOCKED);
    }

    proptest! {
        // r[verify trust-level.block-precedence]
        #[test]
        fn prop_block_precedence(in_trusted: bool) {
            prop_assert_eq!(resolve_trust(true, in_trusted), BLOCKED);
        }

        // r[verify trust-level.can-access]
        #[test]
        fn prop_blocked_denies(is_public: bool) {
            prop_assert!(!can_access(BLOCKED, is_public));
        }

        // r[verify trust-level.can-access]
        #[test]
        fn prop_trusted_allows(is_public: bool) {
            prop_assert!(can_access(TRUSTED, is_public));
        }

        // r[verify trust-level.can-access]
        #[test]
        fn prop_access_matches_resolve(in_blocked: bool, in_trusted: bool, is_public: bool) {
            let level = resolve_trust(in_blocked, in_trusted);
            let access = can_access(level, is_public);
            if in_blocked {
                prop_assert!(!access);
            } else if in_trusted {
                prop_assert!(access);
            } else {
                prop_assert_eq!(access, is_public);
            }
        }
    }
}
