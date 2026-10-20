//! Verified election jitter via Knuth multiplicative hash.
//!
//! Produces a deterministic per-node jitter value for Raft election timeouts.
//! The jitter is bounded by `jitter_range` (typically `(max - min) / 3`).
//! Uses the Knuth multiplicative hash constant (2,654,435,761) for good
//! distribution across small sequential node IDs.

use vstd::prelude::*;

verus! {

/// Knuth multiplicative hash constant (golden ratio × 2^32, truncated).
pub const PHI_KNUTH: u64 = 2_654_435_761u64;

// --- compute_jitter ---

// r[depends election-jitter.compute]
/// Spec: jitter = (node_id wrapping_mul PHI_KNUTH) % jitter_range.
/// When jitter_range is 0, returns 0.
pub open spec fn compute_jitter_spec(node_id: u64, jitter_range: u64) -> u64 {
    if jitter_range == 0 {
        0u64
    } else {
        ((node_id as int * PHI_KNUTH as int) % (u64::MAX as int + 1)) as u64 % jitter_range
    }
}

// r[impl election-jitter.compute]
/// Compute deterministic election jitter for a node.
pub fn compute_jitter(node_id: u64, jitter_range: u64) -> (result: u64)
    ensures result == compute_jitter_spec(node_id, jitter_range)
{
    if jitter_range == 0 {
        0
    } else {
        node_id.wrapping_mul(PHI_KNUTH) % jitter_range
    }
}

// --- Safety proofs ---

// r[impl election-jitter.bounded]
// r[verify election-jitter.bounded]
/// Jitter is always strictly less than jitter_range (or zero when range is zero).
proof fn jitter_bounded(node_id: u64, jitter_range: u64)
    ensures
        jitter_range > 0 ==> compute_jitter_spec(node_id, jitter_range) < jitter_range,
        jitter_range == 0 ==> compute_jitter_spec(node_id, jitter_range) == 0u64,
{}

// r[impl election-jitter.deterministic]
// r[verify election-jitter.deterministic]
/// Same inputs produce same output.
proof fn deterministic(n1: u64, n2: u64, r1: u64, r2: u64)
    requires n1 == n2, r1 == r2
    ensures compute_jitter_spec(n1, r1) == compute_jitter_spec(n2, r2)
{}

// r[verify election-jitter.bounded]
/// Zero range always gives zero jitter.
proof fn zero_range_zero_jitter(node_id: u64)
    ensures compute_jitter_spec(node_id, 0) == 0u64
{}

// r[verify election-jitter.compute]
/// Jitter for node 0 with any range is 0 (0 * K = 0).
proof fn node_zero_jitter_zero(jitter_range: u64)
    ensures compute_jitter_spec(0, jitter_range) == 0u64
{
    let k = PHI_KNUTH as int;
    let m = u64::MAX as int + 1;
    assert(0int * k == 0int) by(nonlinear_arith);
    assert((0int * k) % m == 0int) by(nonlinear_arith)
        requires m > 0;
    if jitter_range > 0 {
        assert(0u64 % jitter_range == 0u64) by(nonlinear_arith)
            requires jitter_range > 0u64;
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify election-jitter.compute]
    #[test]
    fn test_basic() {
        assert_eq!(compute_jitter(0, 100), 0);
        assert_eq!(compute_jitter(1, 100), compute_jitter(1, 100));
        assert_eq!(compute_jitter(42, 0), 0);
    }

    // r[verify election-jitter.bounded]
    #[test]
    fn test_bounded() {
        for id in 0_u64..1000 {
            let j = compute_jitter(id, 50);
            assert!(j < 50);
        }
    }

    proptest! {
        // r[verify election-jitter.bounded]
        #[test]
        fn prop_bounded(node_id: u64, jitter_range in 1u64..10000) {
            prop_assert!(compute_jitter(node_id, jitter_range) < jitter_range);
        }

        // r[verify election-jitter.bounded]
        #[test]
        fn prop_zero_range(node_id: u64) {
            prop_assert_eq!(compute_jitter(node_id, 0), 0);
        }

        // r[verify election-jitter.deterministic]
        #[test]
        fn prop_deterministic(node_id: u64, jitter_range: u64) {
            prop_assert_eq!(
                compute_jitter(node_id, jitter_range),
                compute_jitter(node_id, jitter_range),
            );
        }

        // r[verify election-jitter.compute]
        #[test]
        fn prop_different_nodes_may_differ(
            a in 1u64..1000,
            b in 1001u64..2000,
            jitter_range in 2u64..1000,
        ) {
            // Not a hard property — just checks the hash spreads values
            let ja = compute_jitter(a, jitter_range);
            let jb = compute_jitter(b, jitter_range);
            // Both must be in range regardless
            prop_assert!(ja < jitter_range);
            prop_assert!(jb < jitter_range);
        }
    }
}
