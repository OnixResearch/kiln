//! Verified gossip infection bounds.
//!
//! Models the lower bound on gossip infection: after k rounds with
//! fanout f, at least min(n, f^k) nodes are infected (assuming ideal
//! conditions — no message loss, no duplicates in early rounds).

#![allow(clippy::single_match_else)]

#[cfg(verus_keep_ghost)]
use vstd::arithmetic::mul::lemma_mul_inequality;
use vstd::prelude::*;

verus! {

// --- infection_lower_bound ---

// r[depends gossip.infection-lower-bound]
/// Spec: lower bound on infected nodes after `rounds` gossip rounds.
/// Uses integer power with saturation at `cluster_size`.
pub open spec fn infection_lower_bound_spec(
    cluster_size: u64, fanout: u64, rounds: u64,
) -> u64 {
    let bound = saturating_pow_spec(fanout, rounds);
    if bound > cluster_size as int { cluster_size } else { bound as u64 }
}

/// Spec: saturating integer power (returns u64::MAX+1 sentinel on overflow).
pub open spec fn saturating_pow_spec(base: u64, exp: u64) -> int
    decreases exp as int
{
    if exp == 0 {
        1int
    } else {
        let prev = saturating_pow_spec(base, (exp - 1) as u64);
        if prev > u64::MAX as int {
            u64::MAX as int + 1 // already overflowed
        } else {
            prev * base as int
        }
    }
}

/// Helper: once saturating_pow_spec exceeds u64::MAX, it stays as sentinel.
proof fn saturating_pow_sentinel_propagates(base: u64, exp: u64)
    requires
        saturating_pow_spec(base, exp) > u64::MAX as int,
        exp < u64::MAX,
    ensures saturating_pow_spec(base, (exp + 1) as u64) > u64::MAX as int
{
    let prev = saturating_pow_spec(base, exp);
    // Unfold one step: since prev > u64::MAX, spec returns sentinel
    assert(prev > u64::MAX as int);
    assert(saturating_pow_spec(base, (exp + 1) as u64) == (u64::MAX as int + 1));
}

// r[impl gossip.infection-lower-bound]
/// Returns a lower bound on the number of infected nodes.
pub fn infection_lower_bound(cluster_size: u64, fanout: u64, rounds: u64) -> (result: u64)
    ensures result == infection_lower_bound_spec(cluster_size, fanout, rounds)
{
    // Compute saturating_pow_spec(fanout, rounds) iteratively.
    // Track the raw power, applying clamp to cluster_size only at the end.
    let mut val: u64 = 1;
    let mut overflowed: bool = false;
    let mut i: u64 = 0;

    proof {
        assert(saturating_pow_spec(fanout, 0u64) == 1int);
    }

    while i < rounds
        invariant
            0 <= i <= rounds,
            !overflowed ==> val as int == saturating_pow_spec(fanout, i),
            overflowed ==> saturating_pow_spec(fanout, i) > u64::MAX as int,
        decreases rounds - i,
    {
        if overflowed {
            proof {
                saturating_pow_sentinel_propagates(fanout, i);
            }
            i = i + 1;
        } else {
            proof {
                // Unfold spec one step: since val == saturating_pow_spec(fanout, i) <= u64::MAX,
                // saturating_pow_spec(fanout, i+1) = val * fanout.
                let prev = saturating_pow_spec(fanout, i);
                assert(prev == val as int);
                assert(prev <= u64::MAX as int);
                assert(saturating_pow_spec(fanout, (i + 1) as u64) == prev * fanout as int);
            }
            match val.checked_mul(fanout) {
                Some(v) => {
                    proof {
                        assert(v as int == val as int * fanout as int);
                        assert(v as int == saturating_pow_spec(fanout, (i + 1) as u64));
                    }
                    val = v;
                },
                None => {
                    proof {
                        assert(val as int * fanout as int > u64::MAX as int);
                        assert(saturating_pow_spec(fanout, (i + 1) as u64) > u64::MAX as int);
                    }
                    overflowed = true;
                },
            }
            i = i + 1;
        }
    }

    // Now apply the clamping to cluster_size.
    proof {
        let bound = saturating_pow_spec(fanout, rounds);
        saturating_pow_nonneg(fanout, rounds);
        if overflowed {
            assert(bound > u64::MAX as int);
            assert(bound > cluster_size as int);
            assert(infection_lower_bound_spec(cluster_size, fanout, rounds) == cluster_size);
        } else {
            assert(val as int == bound);
            if bound > cluster_size as int {
                assert(infection_lower_bound_spec(cluster_size, fanout, rounds) == cluster_size);
            } else {
                assert(0 <= bound <= u64::MAX as int);
                assert(infection_lower_bound_spec(cluster_size, fanout, rounds) == bound as u64);
            }
        }
    }
    if overflowed || val > cluster_size {
        cluster_size
    } else {
        val
    }
}

// --- convergence_rounds ---

// r[depends gossip.convergence-rounds]
/// Spec: number of rounds for full infection (upper bound).
/// ceil(log_f(n)) rounds suffice when fanout >= 2.
pub open spec fn convergence_rounds_spec(cluster_size: u64, fanout: u64) -> u64
    recommends cluster_size > 0, fanout >= 2
{
    if cluster_size <= 1 {
        0u64
    } else {
        ceil_log_spec(cluster_size, fanout)
    }
}

/// Spec: ceiling of log base `base` of `n`.
/// Uses fuel-bounded recursion for guaranteed termination.
pub open spec fn ceil_log_spec(n: u64, base: u64) -> u64 {
    ceil_log_helper(n, base, n as nat)
}

/// Fuel-bounded recursive helper. Fuel starts at n (sufficient since
/// ceil(n/base) < n for n > 1, base >= 2).
pub open spec fn ceil_log_helper(n: u64, base: u64, fuel: nat) -> u64
    decreases fuel
{
    if fuel == 0 || n <= 1 {
        0u64
    } else {
        let next = ((n as int - 1) / base as int + 1) as u64;
        (1 + ceil_log_helper(next, base, (fuel - 1) as nat)) as u64
    }
}

/// Helper: ceil_div(n, base) < n when n > 1, base >= 2.
proof fn ceil_div_decreases(n: u64, base: u64)
    requires n > 1, base >= 2
    ensures ({
        let next_int = (n as int - 1) / base as int + 1;
        &&& 1 <= next_int < n as int
        &&& next_int <= u64::MAX as int
        &&& ((n as int - 1) / base as int + 1) as u64 == next_int as u64
    })
{
    let ni = n as int;
    let bi = base as int;
    assert((ni - 1) / bi >= 0) by(nonlinear_arith)
        requires ni > 1, bi >= 2;
    assert((ni - 1) / bi < ni - 1) by(nonlinear_arith)
        requires ni > 1, bi >= 2;
    assert((ni - 1) / bi + 1 < ni) by(nonlinear_arith)
        requires ni > 1, bi >= 2, (ni - 1) / bi < ni - 1;
    assert((ni - 1) / bi + 1 >= 1) by(nonlinear_arith)
        requires ni > 1, bi >= 2, (ni - 1) / bi >= 0;
}

/// Helper: extra fuel beyond what's needed doesn't change the result.
proof fn ceil_log_helper_fuel_sufficient(n: u64, base: u64, fuel1: nat, fuel2: nat)
    requires
        base >= 2,
        fuel1 >= n as nat,
        fuel2 >= n as nat,
    ensures ceil_log_helper(n, base, fuel1) == ceil_log_helper(n, base, fuel2)
    decreases fuel1
{
    if n > 1 && fuel1 > 0 && fuel2 > 0 {
        ceil_div_decreases(n, base);
        let next = ((n as int - 1) / base as int + 1) as u64;
        ceil_log_helper_fuel_sufficient(
            next, base,
            (fuel1 - 1) as nat,
            (fuel2 - 1) as nat,
        );
    }
}

/// Helper: ceil_log result is bounded by n (so it fits in u64 and rounds don't overflow).
proof fn ceil_log_bounded(n: u64, base: u64, fuel: nat)
    requires base >= 2, fuel >= n as nat
    ensures ceil_log_helper(n, base, fuel) <= n
    decreases fuel
{
    if n > 1 && fuel > 0 {
        ceil_div_decreases(n, base);
        let next = ((n as int - 1) / base as int + 1) as u64;
        ceil_log_bounded(next, base, (fuel - 1) as nat);
    }
}

/// One-step unfolding: ceil_log(n, base) == 1 + ceil_log(ceil(n/base), base) for n > 1.
proof fn ceil_log_unfold(n: u64, base: u64)
    requires n > 1, base >= 2
    ensures ({
        let next = ((n as int - 1) / base as int + 1) as u64;
        ceil_log_spec(n, base) as int == 1 + ceil_log_spec(next, base) as int
    })
{
    ceil_div_decreases(n, base);
    let next = ((n as int - 1) / base as int + 1) as u64;
    // ceil_log_spec(n, base) = ceil_log_helper(n, base, n as nat)
    // By definition: helper(n, base, n) = 1 + helper(next, base, n-1) (since n > 1, fuel > 0)
    // Need: helper(next, base, n-1) == helper(next, base, next) = ceil_log_spec(next, base)
    assert(next as nat <= (n as nat - 1) as nat);
    ceil_log_helper_fuel_sufficient(next, base, (n as nat - 1) as nat, next as nat);
    // Now need to show (1 + helper(next, base, n-1)) as u64 == 1 + ceil_log_spec(next, base)
    // i.e., the as-u64 cast is lossless
    ceil_log_bounded(next, base, next as nat);
    ceil_log_bounded(n, base, n as nat);
}

// r[impl gossip.convergence-rounds]
/// Returns the number of gossip rounds needed for full infection.
pub fn convergence_rounds(cluster_size: u64, fanout: u64) -> (result: u64)
    requires cluster_size > 0, fanout >= 2
    ensures result == convergence_rounds_spec(cluster_size, fanout)
{
    if cluster_size <= 1 {
        return 0;
    }
    let mut n: u64 = cluster_size;
    let mut rounds: u64 = 0;
    while n > 1
        invariant
            1 <= n <= cluster_size,
            fanout >= 2,
            rounds as int + ceil_log_spec(n, fanout) as int
                == ceil_log_spec(cluster_size, fanout) as int,
            rounds <= cluster_size,
        decreases n,
    {
        proof {
            ceil_log_unfold(n, fanout);
            ceil_log_bounded(cluster_size, fanout, cluster_size as nat);
            ceil_div_decreases(n, fanout);
        }
        // ceil(n / fanout) = (n - 1) / fanout + 1
        // Use subtraction first to avoid u64 overflow on n + fanout - 1
        let next = (n - 1) / fanout + 1;
        n = next;
        rounds = rounds + 1;
    }
    rounds
}

// --- Safety proofs ---

// r[verify gossip.infection-lower-bound]
/// Helper: saturating_pow_spec is always >= 0.
pub proof fn saturating_pow_nonneg(base: u64, exp: u64)
    ensures saturating_pow_spec(base, exp) >= 0
    decreases exp as int
{
    if exp != 0 {
        saturating_pow_nonneg(base, (exp - 1) as u64);
    }
}

/// Infection is bounded by cluster size.
proof fn infection_bounded(cluster_size: u64, fanout: u64, rounds: u64)
    ensures infection_lower_bound_spec(cluster_size, fanout, rounds) <= cluster_size
{
    saturating_pow_nonneg(fanout, rounds);
    let bound = saturating_pow_spec(fanout, rounds);
    assert(bound >= 0);
    if bound > cluster_size as int {
        assert(infection_lower_bound_spec(cluster_size, fanout, rounds) == cluster_size);
    } else {
        assert(0 <= bound <= u64::MAX as int);
        assert(infection_lower_bound_spec(cluster_size, fanout, rounds) == bound as u64);
    }
}

// r[verify gossip.infection-lower-bound]
/// Zero rounds means 1 infected (the initial node), capped at cluster size.
proof fn infection_zero_rounds(cluster_size: u64, fanout: u64)
    ensures infection_lower_bound_spec(cluster_size, fanout, 0) ==
        if cluster_size == 0 { 0u64 } else { 1u64 }
{
    assert(saturating_pow_spec(fanout, 0u64) == 1int);
}

// r[verify gossip.convergence-rounds]
/// Single node needs zero rounds.
proof fn convergence_single_node(fanout: u64)
    requires fanout >= 2
    ensures convergence_rounds_spec(1, fanout) == 0
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify gossip.infection-lower-bound]
    #[test]
    fn test_infection_lower_bound() {
        // fanout=2, 3 rounds: 2^3 = 8, capped at cluster
        assert_eq!(infection_lower_bound(100, 2, 3), 8);
        assert_eq!(infection_lower_bound(5, 2, 3), 5); // capped at 5
        assert_eq!(infection_lower_bound(100, 2, 0), 1); // zero rounds = 1
        assert_eq!(infection_lower_bound(0, 2, 5), 0); // empty cluster
    }

    // r[verify gossip.convergence-rounds]
    #[test]
    fn test_convergence_rounds() {
        assert_eq!(convergence_rounds(1, 2), 0);
        assert_eq!(convergence_rounds(2, 2), 1);
        assert_eq!(convergence_rounds(8, 2), 3);
    }

    proptest! {
        // r[verify gossip.infection-lower-bound]
        #[test]
        fn prop_infection_bounded(
            cluster_size: u64,
            fanout in 1u64..=10,
            rounds in 0u64..=20,
        ) {
            prop_assert!(infection_lower_bound(cluster_size, fanout, rounds) <= cluster_size);
        }

        // r[verify gossip.infection-lower-bound]
        #[test]
        fn prop_zero_rounds_one_infected(
            cluster_size in 1u64..=1000,
            fanout in 1u64..=10,
        ) {
            let result = infection_lower_bound(cluster_size, fanout, 0);
            prop_assert!(result <= 1);
        }

        #[test]
        fn prop_convergence_rounds_positive(
            cluster_size in 2u64..=100,
            fanout in 2u64..=10,
        ) {
            let rounds = convergence_rounds(cluster_size, fanout);
            prop_assert!(rounds >= 1);
        }

        #[test]
        fn prop_converged_means_all_infected(
            cluster_size in 2u64..=50,
            fanout in 2u64..=5,
        ) {
            let rounds = convergence_rounds(cluster_size, fanout);
            let infected = infection_lower_bound(cluster_size, fanout, rounds);
            prop_assert_eq!(infected, cluster_size);
        }

        #[test]
        fn prop_infection_monotonic_rounds(
            cluster_size in 1u64..=1000,
            fanout in 2u64..=10,
            r1 in 0u64..=10,
            r2 in 0u64..=10,
        ) {
            if r1 <= r2 {
                prop_assert!(
                    infection_lower_bound(cluster_size, fanout, r1)
                    <= infection_lower_bound(cluster_size, fanout, r2)
                );
            }
        }
    }
}
