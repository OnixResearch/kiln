//! Verified UCAN delegation depth budget.
//!
//! Models UCAN's implicit depth limit as chain length, proves that
//! chain extension increments depth, and that a depth-limited chain
//! is bounded.

#![allow(clippy::ptr_arg)]

use vstd::prelude::*;

verus! {

/// Delegation depth is the number of hops in a proof chain.
/// A single token (no delegation) has depth 0.
// r[impl ucan-delegation-depth.depth-definition]
pub open spec fn delegation_depth_spec(chain_len: nat) -> nat
    recommends chain_len > 0,
{
    (chain_len - 1) as nat
}

/// A chain respects a depth budget if its depth is at most the budget.
// r[impl ucan-delegation-depth.budget-check]
pub open spec fn within_depth_budget_spec(chain_len: nat, max_depth: nat) -> bool {
    chain_len > 0 && delegation_depth_spec(chain_len) <= max_depth
}

/// Check at runtime whether a chain length is within budget.
// r[impl ucan-delegation-depth.budget-check]
pub fn check_depth_budget(chain_len: usize, max_depth: usize) -> (result: bool)
    requires chain_len > 0,
    ensures result == within_depth_budget_spec(chain_len as nat, max_depth as nat),
{
    chain_len - 1 <= max_depth
}

/// A shell proof-hop depth is admissible when it can be represented as a
/// positive finite chain length and that chain length is within budget.
// r[impl ucan-delegation-depth.budget-check]
pub open spec fn proof_hop_depth_within_budget_spec(depth: usize, max_depth: usize) -> bool {
    depth < usize::MAX && within_depth_budget_spec((depth as nat) + 1, max_depth as nat)
}

/// Check shell proof-hop depth against a maximum delegation-depth budget.
///
/// The shell counts proof hops from the leaf. The verified chain model counts
/// tokens in the finite chain, so this helper rejects `depth + 1` overflow and
/// then delegates to the chain-length budget predicate.
// r[impl ucan-delegation-depth.budget-check]
pub fn check_proof_hop_depth_budget(depth: usize, max_depth: usize) -> (result: bool)
    ensures result == proof_hop_depth_within_budget_spec(depth, max_depth),
{
    if depth == usize::MAX {
        false
    } else {
        assert(depth < usize::MAX);
        let chain_len = depth + 1;
        assert(chain_len > 0);
        assert(chain_len as nat == (depth as nat) + 1);
        check_depth_budget(chain_len, max_depth)
    }
}

// --- Proofs ---

/// A single-token chain has depth 0.
// r[verify ucan-delegation-depth.depth-definition]
pub proof fn single_token_depth_zero()
    ensures delegation_depth_spec(1) == 0,
{
}

/// Extending a chain by one increases depth by one.
// r[verify ucan-delegation-depth.depth-definition]
pub proof fn extend_increments_depth(chain_len: nat)
    requires chain_len > 0,
    ensures delegation_depth_spec(chain_len + 1) == delegation_depth_spec(chain_len) + 1,
{
}

/// A chain within budget stays within budget if the budget grows.
// r[verify ucan-delegation-depth.budget-check]
pub proof fn budget_monotone(chain_len: nat, small_budget: nat, big_budget: nat)
    requires
        within_depth_budget_spec(chain_len, small_budget),
        small_budget <= big_budget,
    ensures
        within_depth_budget_spec(chain_len, big_budget),
{
}

/// Extending a chain within budget requires at least one more depth unit.
// r[verify ucan-delegation-depth.budget-check]
pub proof fn extend_needs_budget(chain_len: nat, max_depth: nat)
    requires
        chain_len > 0,
        within_depth_budget_spec(chain_len, max_depth),
    ensures
        within_depth_budget_spec(chain_len + 1, max_depth) <==>
            delegation_depth_spec(chain_len) < max_depth,
{
}

/// A chain that exceeds the budget is rejected.
pub proof fn over_budget_rejects(chain_len: nat, max_depth: nat)
    requires
        chain_len > 0,
        delegation_depth_spec(chain_len) > max_depth,
    ensures
        !within_depth_budget_spec(chain_len, max_depth),
{
}

} // verus!

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_token_within_any_budget() {
        assert!(check_depth_budget(1, 0));
        assert!(check_depth_budget(1, 10));
    }

    #[test]
    fn test_two_hop_needs_depth_one() {
        assert!(!check_depth_budget(2, 0));
        assert!(check_depth_budget(2, 1));
    }

    #[test]
    fn test_deep_chain() {
        assert!(check_depth_budget(11, 10));
        assert!(!check_depth_budget(12, 10));
    }

    #[test]
    fn test_proof_hop_depth_admission() {
        assert!(check_proof_hop_depth_budget(0, 0));
        assert!(check_proof_hop_depth_budget(5, 5));
        assert!(!check_proof_hop_depth_budget(6, 5));
        assert!(!check_proof_hop_depth_budget(usize::MAX, usize::MAX));
    }

    #[test]
    fn test_exact_budget() {
        assert!(check_depth_budget(6, 5));
        assert!(!check_depth_budget(7, 5));
    }
}
