//! Exact non-negative mass and momentum drift budget primitives.
use vstd::prelude::*;

verus! {

// r[depends domain-primitives.physics-momentum-budget]
#[allow(clippy::cast_sign_loss)]
pub open spec fn physics_momentum_budget_ok_spec(mass: u64, pre: i32, post: i32, drift: u64) -> bool { 0 < mass && (if pre >= post { ((pre as i64) - (post as i64)) as u64 <= drift } else { ((post as i64) - (pre as i64)) as u64 <= drift }) }

// r[impl domain-primitives.physics-momentum-budget]
#[allow(clippy::cast_sign_loss)]
pub fn physics_momentum_budget_ok(mass: u64, pre: i32, post: i32, drift: u64) -> (result: bool)
    ensures result == physics_momentum_budget_ok_spec(mass, pre, post, drift)
{ mass > 0 && if pre >= post { ((pre as i64) - (post as i64)) as u64 <= drift } else { ((post as i64) - (pre as i64)) as u64 <= drift } }

// r[verify domain-primitives.physics-momentum-budget]
pub proof fn physics_momentum_budget_requires_positive_mass(mass: u64, pre: i32, post: i32, drift: u64)
    requires physics_momentum_budget_ok_spec(mass, pre, post, drift)
    ensures 0 < mass
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn momentum_budget_checks_mass_and_drift() {
        assert!(physics_momentum_budget_ok(1, 10, 12, 2));
        assert!(!physics_momentum_budget_ok(0, 10, 10, 0));
        assert!(!physics_momentum_budget_ok(1, 10, 13, 2));
    }
}
