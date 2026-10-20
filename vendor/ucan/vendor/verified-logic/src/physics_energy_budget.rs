//! Exact non-negative energy drift budget primitives.
use vstd::prelude::*;

verus! {

// r[depends domain-primitives.physics-energy-budget]
pub open spec fn physics_abs_diff_spec(a: u64, b: u64) -> nat { if a >= b { (a - b) as nat } else { (b - a) as nat } }

// r[depends domain-primitives.physics-energy-budget]
pub open spec fn physics_energy_budget_ok_spec(pre: u64, post: u64, drift: u64) -> bool { physics_abs_diff_spec(pre, post) <= drift as nat }

// r[impl domain-primitives.physics-energy-budget]
pub fn physics_energy_budget_ok(pre: u64, post: u64, drift: u64) -> (result: bool)
    ensures result == physics_energy_budget_ok_spec(pre, post, drift)
{ if pre >= post { pre - post <= drift } else { post - pre <= drift } }

// r[verify domain-primitives.physics-energy-budget]
pub proof fn physics_energy_zero_drift_exact(pre: u64, post: u64)
    requires physics_energy_budget_ok_spec(pre, post, 0)
    ensures pre == post
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn energy_budget_bounds_drift() {
        assert!(physics_energy_budget_ok(10, 12, 2));
        assert!(!physics_energy_budget_ok(10, 13, 2));
    }
}
