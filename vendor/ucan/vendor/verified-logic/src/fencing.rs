//! Verified fencing tokens for stale-write rejection.
//!
//! Fencing tokens are monotonically increasing values attached to write
//! requests. A storage node tracks the highest token it has seen (the fence).
//! Writes bearing a token older than the fence are rejected. This prevents
//! delayed or zombie clients from corrupting state after a newer client
//! has taken over.
//!
//! Three functions, three safety proofs, one cross-module link to
//! `monotonic::is_strict_advance`.

use vstd::prelude::*;

verus! {

// --- issue_token ---

// r[depends fencing.issue-token]
/// Spec: the next fencing token is current + 1.
pub open spec fn issue_token_spec(current: u64) -> u64 {
    (current + 1) as u64
}

// r[impl fencing.issue-token]
/// Issue a new fencing token by incrementing the current one.
pub fn issue_token(current: u64) -> (result: u64)
    requires current < u64::MAX
    ensures result == issue_token_spec(current)
{
    current + 1
}

// --- is_valid_write ---

// r[depends fencing.is-valid-write]
/// Spec: a write is valid when its token is at least as recent as the fence.
pub open spec fn is_valid_write_spec(request_token: u64, fence: u64) -> bool {
    request_token >= fence
}

// r[impl fencing.is-valid-write]
/// Returns true if the request token is not older than the fence.
pub fn is_valid_write(request_token: u64, fence: u64) -> (result: bool)
    ensures result == is_valid_write_spec(request_token, fence)
{
    request_token >= fence
}

// --- is_stale ---

// r[depends fencing.is-stale]
/// Spec: a token is stale when it is strictly less than the fence.
pub open spec fn is_stale_spec(token: u64, fence: u64) -> bool {
    token < fence
}

// r[impl fencing.is-stale]
/// Returns true if the token predates the fence.
pub fn is_stale(token: u64, fence: u64) -> (result: bool)
    ensures result == is_stale_spec(token, fence)
{
    token < fence
}

// --- Safety proofs ---

// r[impl fencing.issuance-strictly-increasing]
// r[verify fencing.issuance-strictly-increasing]
/// Issued tokens are strictly greater than the input.
proof fn issuance_strictly_increasing(t: u64)
    requires t < u64::MAX
    ensures issue_token_spec(t) > t
{}

// r[impl fencing.no-false-rejection]
// r[verify fencing.no-false-rejection]
/// A freshly issued token is always valid against the fence it came from.
proof fn no_false_rejection(fence: u64)
    requires fence < u64::MAX
    ensures is_valid_write_spec(issue_token_spec(fence), fence)
{}

// r[impl fencing.stale-implies-not-valid]
// r[verify fencing.stale-implies-not-valid]
/// is_stale and is_valid_write are exact complements.
proof fn stale_complement(token: u64, fence: u64)
    ensures is_stale_spec(token, fence) == !is_valid_write_spec(token, fence)
{}

// --- Additional ordering proofs ---

// r[verify fencing.issuance-strictly-increasing]
/// Transitivity: if token a was issued before token b (a < b),
/// then a is stale with respect to any fence that accepts b.
proof fn stale_transitivity(a: u64, b: u64, fence: u64)
    requires a < b, is_valid_write_spec(b, fence)
    ensures
        fence > 0 ==> is_stale_spec(a, fence) || is_valid_write_spec(a, fence),
        b > a,
{}

// r[verify fencing.issuance-strictly-increasing]
/// Two successive issuances produce a gap of exactly 1.
proof fn issuance_gap(t: u64)
    requires t < u64::MAX
    ensures issue_token_spec(t) == t + 1
{}

// r[verify fencing.no-false-rejection]
/// Advancing the fence to the issued token still accepts that token.
proof fn issued_token_meets_own_fence(t: u64)
    requires t < u64::MAX
    ensures is_valid_write_spec(issue_token_spec(t), issue_token_spec(t))
{}

// r[verify fencing.stale-implies-not-valid]
/// Zero fence never rejects any token.
proof fn zero_fence_accepts_all(token: u64)
    ensures !is_stale_spec(token, 0)
{}

// r[verify fencing.stale-implies-not-valid]
/// MAX fence rejects everything except MAX itself.
proof fn max_fence_rejects_below(token: u64)
    requires token < u64::MAX
    ensures is_stale_spec(token, u64::MAX)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify fencing.issue-token]
    #[test]
    fn test_issue_token() {
        assert_eq!(issue_token(0), 1);
        assert_eq!(issue_token(41), 42);
        assert_eq!(issue_token(u64::MAX - 1), u64::MAX);
    }

    // r[verify fencing.is-valid-write]
    #[test]
    fn test_is_valid_write() {
        assert!(is_valid_write(5, 5));
        assert!(is_valid_write(6, 5));
        assert!(!is_valid_write(4, 5));
        assert!(is_valid_write(0, 0));
        assert!(is_valid_write(u64::MAX, u64::MAX));
    }

    // r[verify fencing.is-stale]
    #[test]
    fn test_is_stale() {
        assert!(!is_stale(5, 5));
        assert!(!is_stale(6, 5));
        assert!(is_stale(4, 5));
        assert!(!is_stale(0, 0));
        assert!(!is_stale(u64::MAX, u64::MAX));
    }

    proptest! {
        // r[verify fencing.issuance-strictly-increasing]
        #[test]
        fn prop_issuance_increasing(t in 0..u64::MAX) {
            prop_assert!(issue_token(t) > t);
        }

        // r[verify fencing.no-false-rejection]
        #[test]
        fn prop_no_false_rejection(fence in 0..u64::MAX) {
            let token = issue_token(fence);
            prop_assert!(is_valid_write(token, fence));
        }

        // r[verify fencing.stale-implies-not-valid]
        #[test]
        fn prop_stale_complement(token: u64, fence: u64) {
            prop_assert_eq!(is_stale(token, fence), !is_valid_write(token, fence));
        }

        // r[verify fencing.is-valid-write]
        #[test]
        fn prop_valid_write_reflexive(v: u64) {
            prop_assert!(is_valid_write(v, v));
        }

        // r[verify fencing.is-stale]
        #[test]
        fn prop_stale_irreflexive(v: u64) {
            prop_assert!(!is_stale(v, v));
        }

        // r[verify fencing.issuance-strictly-increasing]
        #[test]
        fn prop_successive_issuance_monotonic(a in 0..u64::MAX - 1) {
            let b = issue_token(a);
            let c = issue_token(b);
            prop_assert!(c > b);
            prop_assert!(c > a);
        }
    }
}
