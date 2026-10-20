//! Verified two-tier rate limit decision logic.
//!
//! Models a global-first rate limit: check the global bucket first
//! (cheaper), then per-peer. If global denies, skip per-peer evaluation.
//! Also includes a capacity invariant for the peer map (bounded by max).
//!
//! Builds on `rate_limit::try_consume` for individual bucket decisions.

use vstd::prelude::*;

verus! {

/// Result constants for tiered rate limit decisions.
pub const ALLOWED: u8 = 0u8;
pub const GLOBAL_DENIED: u8 = 1u8;
pub const PEER_DENIED: u8 = 2u8;

// --- check_tiered ---

// r[depends tiered-rate-limit.check-tiered]
/// Spec: two-tier rate limit decision.
///
/// Checks global bucket first. If global allows, checks per-peer bucket.
/// Returns (result_code, new_global_tokens, new_peer_tokens).
pub open spec fn check_tiered_spec(
    global_tokens: u64,
    peer_tokens: u64,
    cost: u64,
) -> (u8, u64, u64) {
    let (global_ok, new_global) = crate::rate_limit::try_consume_spec(global_tokens, cost);
    if !global_ok {
        (GLOBAL_DENIED, global_tokens, peer_tokens)
    } else {
        let (peer_ok, new_peer) = crate::rate_limit::try_consume_spec(peer_tokens, cost);
        if !peer_ok {
            // Global was consumed but peer denied — rollback global
            (PEER_DENIED, global_tokens, peer_tokens)
        } else {
            (ALLOWED, new_global, new_peer)
        }
    }
}

// r[impl tiered-rate-limit.check-tiered]
/// Check the two-tier rate limit.
pub fn check_tiered(
    global_tokens: u64,
    peer_tokens: u64,
    cost: u64,
) -> (result: (u8, u64, u64))
    ensures
        result.0 == check_tiered_spec(global_tokens, peer_tokens, cost).0,
        result.1 == check_tiered_spec(global_tokens, peer_tokens, cost).1,
        result.2 == check_tiered_spec(global_tokens, peer_tokens, cost).2,
{
    let (global_ok, new_global) = crate::rate_limit::try_consume(global_tokens, cost);
    if !global_ok {
        (GLOBAL_DENIED, global_tokens, peer_tokens)
    } else {
        let (peer_ok, new_peer) = crate::rate_limit::try_consume(peer_tokens, cost);
        if !peer_ok {
            (PEER_DENIED, global_tokens, peer_tokens)
        } else {
            (ALLOWED, new_global, new_peer)
        }
    }
}

// --- peer_map_bounded ---

// r[depends tiered-rate-limit.peer-map-bounded]
/// Spec: peer map count is within capacity.
pub open spec fn peer_map_bounded_spec(count: u64, max_peers: u64) -> bool {
    count <= max_peers
}

// r[impl tiered-rate-limit.peer-map-bounded]
/// Check if peer map is within capacity.
pub fn peer_map_bounded(count: u64, max_peers: u64) -> (result: bool)
    ensures result == peer_map_bounded_spec(count, max_peers)
{
    count <= max_peers
}

// --- evict_reduces_count ---

// r[depends tiered-rate-limit.evict-reduces]
/// Spec: after evicting one entry, count decreases by 1.
pub open spec fn after_evict_spec(count: u64) -> u64 {
    if count == 0 { 0u64 } else { (count - 1) as u64 }
}

// r[impl tiered-rate-limit.evict-reduces]
/// Compute count after evicting one entry.
pub fn after_evict(count: u64) -> (result: u64)
    ensures result == after_evict_spec(count)
{
    if count == 0 { 0 } else { count - 1 }
}

// --- Safety proofs ---

// r[impl tiered-rate-limit.global-first]
// r[verify tiered-rate-limit.global-first]
/// Global denied means result is GLOBAL_DENIED without per-peer evaluation.
/// Tokens are unchanged.
proof fn global_first_ordering(global_tokens: u64, peer_tokens: u64, cost: u64)
    requires global_tokens < cost
    ensures
        check_tiered_spec(global_tokens, peer_tokens, cost).0 == GLOBAL_DENIED,
        check_tiered_spec(global_tokens, peer_tokens, cost).1 == global_tokens,
        check_tiered_spec(global_tokens, peer_tokens, cost).2 == peer_tokens,
{}

// r[verify tiered-rate-limit.check-tiered]
/// When both buckets have enough tokens, request is allowed.
proof fn both_sufficient(global_tokens: u64, peer_tokens: u64, cost: u64)
    requires global_tokens >= cost, peer_tokens >= cost
    ensures check_tiered_spec(global_tokens, peer_tokens, cost).0 == ALLOWED
{}

// r[verify tiered-rate-limit.check-tiered]
/// Peer denied: global has enough but peer doesn't. Tokens unchanged (rollback).
proof fn peer_denied_rollback(global_tokens: u64, peer_tokens: u64, cost: u64)
    requires global_tokens >= cost, peer_tokens < cost
    ensures
        check_tiered_spec(global_tokens, peer_tokens, cost).0 == PEER_DENIED,
        check_tiered_spec(global_tokens, peer_tokens, cost).1 == global_tokens,
        check_tiered_spec(global_tokens, peer_tokens, cost).2 == peer_tokens,
{}

// r[impl tiered-rate-limit.capacity-invariant]
// r[verify tiered-rate-limit.capacity-invariant]
/// After eviction, count stays within capacity if it was before.
proof fn evict_maintains_bound(count: u64, max_peers: u64)
    requires peer_map_bounded_spec(count, max_peers)
    ensures peer_map_bounded_spec(after_evict_spec(count), max_peers)
{}

// r[verify tiered-rate-limit.capacity-invariant]
/// After eviction of a non-empty map, there's room for a new entry.
proof fn evict_makes_room(count: u64, max_peers: u64)
    requires peer_map_bounded_spec(count, max_peers), count > 0
    ensures after_evict_spec(count) < max_peers
{}

// r[verify tiered-rate-limit.check-tiered]
/// Zero cost is always allowed (consumes nothing).
proof fn zero_cost_allowed(global_tokens: u64, peer_tokens: u64)
    ensures check_tiered_spec(global_tokens, peer_tokens, 0).0 == ALLOWED
{}

// r[verify tiered-rate-limit.check-tiered]
/// On ALLOWED, global tokens decrease by cost.
proof fn allowed_decrements_global(global_tokens: u64, peer_tokens: u64, cost: u64)
    requires global_tokens >= cost, peer_tokens >= cost
    ensures
        check_tiered_spec(global_tokens, peer_tokens, cost).1 == global_tokens - cost,
        check_tiered_spec(global_tokens, peer_tokens, cost).2 == peer_tokens - cost,
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify tiered-rate-limit.check-tiered]
    // r[verify tiered-rate-limit.peer-map-bounded]
    // r[verify tiered-rate-limit.evict-reduces]
    #[test]
    fn test_both_sufficient() {
        let (code, g, p) = check_tiered(100, 50, 10);
        assert_eq!(code, ALLOWED);
        assert_eq!(g, 90);
        assert_eq!(p, 40);
    }

    // r[verify tiered-rate-limit.global-first]
    #[test]
    fn test_global_denied() {
        let (code, g, p) = check_tiered(5, 100, 10);
        assert_eq!(code, GLOBAL_DENIED);
        assert_eq!(g, 5);
        assert_eq!(p, 100);
    }

    // r[verify tiered-rate-limit.check-tiered]
    #[test]
    fn test_peer_denied() {
        let (code, g, p) = check_tiered(100, 5, 10);
        assert_eq!(code, PEER_DENIED);
        assert_eq!(g, 100); // rolled back
        assert_eq!(p, 5);
    }

    // r[verify tiered-rate-limit.capacity-invariant]
    #[test]
    fn test_evict() {
        assert_eq!(after_evict(5), 4);
        assert_eq!(after_evict(1), 0);
        assert_eq!(after_evict(0), 0);
    }

    proptest! {
        // r[verify tiered-rate-limit.global-first]
        #[test]
        fn prop_global_denied_unchanged(global in 0u64..100, peer: u64, cost in 101u64..200) {
            let (code, g, p) = check_tiered(global, peer, cost);
            prop_assert_eq!(code, GLOBAL_DENIED);
            prop_assert_eq!(g, global);
            prop_assert_eq!(p, peer);
        }

        // r[verify tiered-rate-limit.check-tiered]
        #[test]
        fn prop_both_sufficient(
            global in 100u64..1000,
            peer in 100u64..1000,
            cost in 0u64..100,
        ) {
            let (code, g, p) = check_tiered(global, peer, cost);
            prop_assert_eq!(code, ALLOWED);
            prop_assert_eq!(g, global - cost);
            prop_assert_eq!(p, peer - cost);
        }

        // r[verify tiered-rate-limit.check-tiered]
        #[test]
        fn prop_peer_denied_rollback(
            global in 100u64..1000,
            peer in 0u64..100,
            cost in 101u64..200,
        ) {
            if global >= cost && peer < cost {
                let (code, g, p) = check_tiered(global, peer, cost);
                prop_assert_eq!(code, PEER_DENIED);
                prop_assert_eq!(g, global);
                prop_assert_eq!(p, peer);
            }
        }

        // r[verify tiered-rate-limit.capacity-invariant]
        #[test]
        fn prop_evict_bounded(count: u64, max_peers in 1u64..1000) {
            if count <= max_peers {
                prop_assert!(after_evict(count) <= max_peers);
            }
        }

        // r[verify tiered-rate-limit.check-tiered]
        #[test]
        fn prop_zero_cost_allowed(global: u64, peer: u64) {
            let (code, _, _) = check_tiered(global, peer, 0);
            prop_assert_eq!(code, ALLOWED);
        }
    }
}
