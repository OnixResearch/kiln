//! Verified lease management with disjointness, renewal, and revocation.
//!
//! A lease grants exclusive access to a resource for a time interval
//! [start, expiry). Revocation cuts the interval short. The key safety
//! property: if lease B starts at or after lease A's effective expiry,
//! no point in time can satisfy both leases simultaneously.

use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::ordering::min_spec;

verus! {

/// A time-bounded lease on a resource.
pub struct Lease {
    /// Resource identifier.
    pub resource_id: u64,
    /// Start of validity (inclusive).
    pub start: u64,
    /// End of validity (exclusive).
    pub expiry: u64,
    /// If revoked, the revocation timestamp. Revocation takes effect at this time.
    pub revoked_at: u64,
    /// Whether this lease has been revoked.
    pub is_revoked: bool,
}

// --- effective_expiry ---

// r[depends lease.effective-expiry]
/// Spec: the actual end of validity, accounting for revocation.
pub open spec fn effective_expiry_spec(lease: Lease) -> u64 {
    if lease.is_revoked {
        min_spec(lease.expiry, lease.revoked_at)
    } else {
        lease.expiry
    }
}

// r[impl lease.effective-expiry]
/// Returns the effective end of this lease, accounting for revocation.
pub fn effective_expiry(lease: &Lease) -> (result: u64)
    ensures result == effective_expiry_spec(*lease)
{
    if lease.is_revoked {
        crate::ordering::min(lease.expiry, lease.revoked_at)
    } else {
        lease.expiry
    }
}

// --- is_valid ---

// r[depends lease.is-valid]
/// Spec: lease is valid at time `now` iff now is in [start, effective_expiry).
pub open spec fn is_valid_spec(lease: Lease, now: u64) -> bool {
    now >= lease.start && now < effective_expiry_spec(lease)
}

// r[impl lease.is-valid]
/// Returns true if this lease is valid at the given time.
pub fn is_valid(lease: &Lease, now: u64) -> (result: bool)
    ensures result == is_valid_spec(*lease, now)
{
    now >= lease.start && now < effective_expiry(lease)
}

// --- grant ---

// r[depends lease.grant]
/// Spec: create a new lease.
pub open spec fn grant_spec(resource_id: u64, start: u64, expiry: u64) -> Lease
    recommends start < expiry
{
    Lease {
        resource_id,
        start,
        expiry,
        revoked_at: 0,
        is_revoked: false,
    }
}

// r[impl lease.grant]
/// Grant a new lease on a resource for the interval [start, expiry).
pub fn grant(resource_id: u64, start: u64, expiry: u64) -> (result: Lease)
    requires start < expiry
    ensures result == grant_spec(resource_id, start, expiry)
{
    Lease {
        resource_id,
        start,
        expiry,
        revoked_at: 0,
        is_revoked: false,
    }
}

// --- renew ---

// r[depends lease.renew]
/// Spec: extend a lease to a new expiry.
pub open spec fn renew_spec(lease: Lease, new_expiry: u64) -> Lease
    recommends new_expiry > lease.expiry, !lease.is_revoked
{
    Lease {
        expiry: new_expiry,
        ..lease
    }
}

// r[impl lease.renew]
/// Extend the lease to a new (later) expiry. Cannot renew a revoked lease.
pub fn renew(lease: &Lease, new_expiry: u64) -> (result: Lease)
    requires new_expiry > lease.expiry, !lease.is_revoked
    ensures result == renew_spec(*lease, new_expiry)
{
    Lease {
        resource_id: lease.resource_id,
        start: lease.start,
        expiry: new_expiry,
        revoked_at: lease.revoked_at,
        is_revoked: lease.is_revoked,
    }
}

// --- revoke ---

// r[depends lease.revoke]
/// Spec: revoke a lease at a given time.
pub open spec fn revoke_spec(lease: Lease, at: u64) -> Lease {
    Lease {
        revoked_at: at,
        is_revoked: true,
        ..lease
    }
}

// r[impl lease.revoke]
/// Revoke this lease. Takes effect at `at`.
pub fn revoke(lease: &Lease, at: u64) -> (result: Lease)
    ensures result == revoke_spec(*lease, at)
{
    Lease {
        resource_id: lease.resource_id,
        start: lease.start,
        expiry: lease.expiry,
        revoked_at: at,
        is_revoked: true,
    }
}

// --- Safety proofs ---

// r[verify lease.is-valid]
/// Disjointness: if lease B starts at or after lease A's effective expiry,
/// no time `t` can make both valid.
proof fn lease_disjointness(a: Lease, b: Lease, t: u64)
    requires
        b.start >= effective_expiry_spec(a),
    ensures
        !(is_valid_spec(a, t) && is_valid_spec(b, t))
{
    // If t is valid for A: t < effective_expiry(A)
    // If t is valid for B: t >= b.start >= effective_expiry(A)
    // Contradiction: t < effective_expiry(A) AND t >= effective_expiry(A)
}

// r[verify lease.renew]
/// Renewal is continuous: the old lease is valid up to old_expiry,
/// the renewed lease is valid from start up to new_expiry, with no gap.
proof fn renewal_continuous(lease: Lease, new_expiry: u64, t: u64)
    requires
        !lease.is_revoked,
        new_expiry > lease.expiry,
        is_valid_spec(lease, t),
    ensures
        is_valid_spec(renew_spec(lease, new_expiry), t)
{
    // Valid for old: start <= t < expiry
    // Valid for renewed: start <= t < new_expiry
    // Since new_expiry > expiry and t < expiry < new_expiry: valid.
}

// r[verify lease.revoke]
/// Revocation is immediate: at time `at` and after, the lease is invalid.
proof fn revocation_immediate(lease: Lease, at: u64, t: u64)
    requires
        t >= at,
        lease.start <= at, // revocation during validity window
    ensures
        !is_valid_spec(revoke_spec(lease, at), t)
{
    // effective_expiry(revoked) = min(expiry, at)
    // t >= at >= min(expiry, at) = effective_expiry
    // so t >= effective_expiry, meaning not valid
}

// r[verify lease.grant]
/// A freshly granted lease is valid at its start time.
proof fn grant_valid_at_start(resource_id: u64, start: u64, expiry: u64)
    requires start < expiry
    ensures is_valid_spec(grant_spec(resource_id, start, expiry), start)
{}

// r[verify lease.grant]
/// A freshly granted lease is invalid before its start.
proof fn grant_invalid_before_start(resource_id: u64, start: u64, expiry: u64, t: u64)
    requires start < expiry, t < start
    ensures !is_valid_spec(grant_spec(resource_id, start, expiry), t)
{}

// r[verify lease.is-valid]
/// A lease with start >= expiry is never valid.
proof fn empty_lease_never_valid(lease: Lease, t: u64)
    requires lease.start >= effective_expiry_spec(lease)
    ensures !is_valid_spec(lease, t)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify lease.is-valid]
    // r[verify lease.effective-expiry]
    #[test]
    fn test_lease_validity() {
        let l = grant(1, 10, 20);
        assert!(!is_valid(&l, 9));
        assert!(is_valid(&l, 10));
        assert!(is_valid(&l, 15));
        assert!(is_valid(&l, 19));
        assert!(!is_valid(&l, 20));
    }

    // r[verify lease.revoke]
    #[test]
    fn test_revocation() {
        let l = grant(1, 10, 20);
        let r = revoke(&l, 15);
        assert!(is_valid(&r, 14));
        assert!(!is_valid(&r, 15));
        assert!(!is_valid(&r, 19));
    }

    // r[verify lease.renew]
    #[test]
    fn test_renewal() {
        let l = grant(1, 10, 20);
        let r = renew(&l, 30);
        assert!(is_valid(&r, 25));
        assert!(!is_valid(&r, 30));
    }

    // r[verify lease.is-valid]
    #[test]
    fn test_disjointness() {
        let a = grant(1, 10, 20);
        let b = grant(1, 20, 30);
        // No time t can be valid for both
        for t in 0..35 {
            assert!(!(is_valid(&a, t) && is_valid(&b, t)));
        }
    }

    proptest! {
        // r[verify lease.is-valid]
        #[test]
        fn prop_lease_disjointness(
            start_a in 0u64..1000,
            width_a in 1u64..1000,
            width_b in 1u64..1000,
            t: u64,
        ) {
            let expiry_a = start_a + width_a;
            let start_b = expiry_a; // B starts where A ends
            let expiry_b = start_b.saturating_add(width_b);
            if expiry_b > start_b {
                let a = grant(1, start_a, expiry_a);
                let b = grant(1, start_b, expiry_b);
                prop_assert!(!(is_valid(&a, t) && is_valid(&b, t)));
            }
        }

        // r[verify lease.revoke]
        #[test]
        fn prop_revocation_immediate(
            start in 0u64..1000,
            width in 1u64..1000,
            revoke_offset in 0u64..1000,
        ) {
            let expiry = start + width;
            let at = start + revoke_offset.min(width);
            let l = grant(1, start, expiry);
            let r = revoke(&l, at);
            // At revocation time and after, lease is invalid
            prop_assert!(!is_valid(&r, at));
            prop_assert!(!is_valid(&r, at.saturating_add(1)));
        }

        // r[verify lease.renew]
        #[test]
        fn prop_renewal_extends(
            start in 0u64..1000,
            width in 1u64..500,
            extra in 1u64..500,
            t_offset in 0u64..1000,
        ) {
            let expiry = start + width;
            let new_expiry = expiry + extra;
            let l = grant(1, start, expiry);
            let r = renew(&l, new_expiry);
            let t = start + t_offset;
            // If t was valid for old, it's valid for renewed
            if is_valid(&l, t) {
                prop_assert!(is_valid(&r, t));
            }
        }

        // r[verify lease.grant]
        #[test]
        fn prop_grant_valid_at_start(
            start in 0u64..u64::MAX - 1,
            width in 1u64..1000,
        ) {
            if start + width > start {
                let l = grant(1, start, start + width);
                prop_assert!(is_valid(&l, start));
            }
        }
    }
}
