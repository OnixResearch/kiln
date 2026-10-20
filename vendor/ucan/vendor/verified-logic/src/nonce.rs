//! Verified nonce issuance with uniqueness and freshness guarantees.
//!
//! A nonce counter produces strictly increasing values. Two nonces
//! from different counter states are provably distinct. Builds on
//! `monotonic::is_strict_advance` for the ordering guarantee.

use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::monotonic::is_advance_spec;
#[cfg(verus_keep_ghost)]
use crate::monotonic::is_strict_advance_spec;

verus! {

// ============================================================
// issue_nonce: produce a nonce and advance the counter
// ============================================================

// r[depends nonce.issue]
/// Spec: the nonce equals the current counter value.
pub open spec fn nonce_value_spec(counter: u64) -> u64 {
    counter
}

// r[depends nonce.issue]
/// Spec: the new counter after issuing a nonce.
pub open spec fn next_counter_spec(counter: u64) -> u64 {
    (counter + 1) as u64
}

// r[impl nonce.issue]
// r[impl nonce.monotonic]
// r[impl nonce.uniqueness]
/// Issue a nonce from the counter. Returns (new_counter, nonce).
/// The nonce is the current counter value; the counter advances by one.
pub fn issue_nonce(counter: u64) -> (result: (u64, u64))
    requires counter < u64::MAX,
    ensures
        result.0 == next_counter_spec(counter),
        result.1 == nonce_value_spec(counter),
{
    (counter + 1, counter)
}

// ============================================================
// is_fresh: check if a nonce is newer than the last seen
// ============================================================

// r[depends nonce.freshness]
/// Spec: a nonce is fresh if it exceeds the last-seen value.
pub open spec fn is_fresh_spec(nonce: u64, last_seen: u64) -> bool {
    nonce > last_seen
}

// r[impl nonce.freshness]
/// Returns true if the nonce is strictly greater than last_seen.
pub fn is_fresh(nonce: u64, last_seen: u64) -> (result: bool)
    ensures result == is_fresh_spec(nonce, last_seen),
{
    nonce > last_seen
}

// ============================================================
// Proofs
// ============================================================

// r[verify nonce.issue]
/// The new counter is greater than the old counter.
proof fn issue_advances_counter(counter: u64)
    requires counter < u64::MAX,
    ensures next_counter_spec(counter) > counter,
{}

// r[verify nonce.monotonic]
/// The new counter is a strict advance over the old.
proof fn issue_is_strict_advance(counter: u64)
    requires counter < u64::MAX,
    ensures is_strict_advance_spec(counter, next_counter_spec(counter)),
{}

// r[verify nonce.monotonic]
/// The new counter is also a (non-strict) advance.
proof fn issue_is_advance(counter: u64)
    requires counter < u64::MAX,
    ensures is_advance_spec(counter, next_counter_spec(counter)),
{}

// r[verify nonce.uniqueness]
/// Two nonces from different counter states are distinct.
proof fn nonces_distinct(c1: u64, c2: u64)
    requires c1 != c2,
    ensures nonce_value_spec(c1) != nonce_value_spec(c2),
{
    // nonce_value_spec(c) == c, so c1 != c2 implies nonces differ.
}

// r[verify nonce.uniqueness]
/// Stronger: a sequence of N consecutive nonces produces N distinct values.
/// If c1 < c2, then nonce(c1) < nonce(c2).
proof fn nonces_ordered(c1: u64, c2: u64)
    requires c1 < c2,
    ensures nonce_value_spec(c1) < nonce_value_spec(c2),
{}

// r[verify nonce.freshness]
/// A freshly issued nonce is fresh against the issuing counter minus one.
/// That is, if we track last_seen = counter - 1 before issuing, the
/// new nonce (== counter) is fresh.
proof fn fresh_after_issue(counter: u64)
    requires counter > 0,
    ensures is_fresh_spec(nonce_value_spec(counter), (counter - 1) as u64),
{}

// r[verify nonce.freshness]
/// A nonce that is not fresh is stale: nonce <= last_seen.
proof fn not_fresh_is_stale(nonce: u64, last_seen: u64)
    requires !is_fresh_spec(nonce, last_seen),
    ensures nonce <= last_seen,
{}

// r[verify nonce.freshness]
/// Freshness is anti-reflexive: a nonce is never fresh against itself.
proof fn freshness_antireflexive(n: u64)
    ensures !is_fresh_spec(n, n),
{}

// r[verify nonce.monotonic]
/// Issuing twice produces strictly ordered nonces.
proof fn double_issue_ordered(c: u64)
    requires c + 1 < u64::MAX,
    ensures nonce_value_spec(c) < nonce_value_spec(next_counter_spec(c)),
{
    // nonce(c) == c, nonce(c+1) == c+1, and c < c+1.
}

// r[verify nonce.freshness]
/// After issuing, the nonce is fresh against any prior counter value.
proof fn issued_nonce_fresh_against_prior(counter: u64, prior: u64)
    requires prior < counter,
    ensures is_fresh_spec(nonce_value_spec(counter), prior),
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify nonce.issue]
    #[test]
    fn test_issue_basic() {
        let (new_counter, nonce) = issue_nonce(0);
        assert_eq!(nonce, 0);
        assert_eq!(new_counter, 1);
    }

    // r[verify nonce.issue]
    #[test]
    fn test_issue_sequence() {
        let (c1, n0) = issue_nonce(0);
        let (c2, n1) = issue_nonce(c1);
        let (c3, n2) = issue_nonce(c2);
        assert_eq!((n0, n1, n2), (0, 1, 2));
        assert_eq!(c3, 3);
    }

    // r[verify nonce.uniqueness]
    #[test]
    fn test_all_distinct() {
        let mut nonces = Vec::new();
        let mut c = 0u64;
        for _ in 0_i32..100_i32 {
            let (new_c, n) = issue_nonce(c);
            nonces.push(n);
            c = new_c;
        }
        // All nonces must be unique
        let mut sorted = nonces.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), nonces.len());
    }

    // r[verify nonce.freshness]
    #[test]
    fn test_freshness() {
        assert!(is_fresh(5, 4));
        assert!(!is_fresh(4, 4));
        assert!(!is_fresh(3, 4));
    }

    // r[verify nonce.freshness]
    #[test]
    fn test_fresh_after_issue() {
        let last_seen = 41u64;
        let (_, nonce) = issue_nonce(last_seen + 1);
        assert!(is_fresh(nonce, last_seen));
    }

    // r[verify nonce.monotonic]
    #[test]
    fn test_monotonic_sequence() {
        let mut c = 0u64;
        let mut prev_nonce = None;
        for _ in 0_i32..100_i32 {
            let (new_c, n) = issue_nonce(c);
            if let Some(p) = prev_nonce {
                assert!(n > p, "nonce {n} should be > previous {p}");
            }
            prev_nonce = Some(n);
            c = new_c;
        }
    }

    proptest! {
        // r[verify nonce.issue]
        #[test]
        fn prop_issue_returns_counter(counter in 0u64..u64::MAX) {
            let (new_c, nonce) = issue_nonce(counter);
            prop_assert_eq!(nonce, counter);
            prop_assert_eq!(new_c, counter + 1);
        }

        // r[verify nonce.uniqueness]
        #[test]
        fn prop_distinct_counters_distinct_nonces(c1 in 0u64..u64::MAX, c2 in 0u64..u64::MAX) {
            if c1 != c2 {
                let (_, n1) = issue_nonce(c1);
                let (_, n2) = issue_nonce(c2);
                prop_assert_ne!(n1, n2);
            }
        }

        // r[verify nonce.monotonic]
        #[test]
        fn prop_sequence_monotonic(start in 0u64..u64::MAX - 100) {
            let mut c = start;
            let mut prev = None;
            for _ in 0_i32..100_i32 {
                let (new_c, n) = issue_nonce(c);
                if let Some(p) = prev {
                    prop_assert!(n > p);
                }
                prev = Some(n);
                c = new_c;
            }
        }

        // r[verify nonce.freshness]
        #[test]
        fn prop_issued_is_fresh_against_prior(
            counter in 1u64..u64::MAX,
            delta in 1u64..1000,
        ) {
            let prior = counter.saturating_sub(delta);
            if prior < counter {
                let (_, nonce) = issue_nonce(counter);
                prop_assert!(is_fresh(nonce, prior));
            }
        }

        // r[verify nonce.freshness]
        #[test]
        fn prop_freshness_antireflexive(n: u64) {
            prop_assert!(!is_fresh(n, n));
        }
    }
}
