//! Exact finite bounded-delegation primitives.
use vstd::prelude::*;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PolicyDelegation { pub from: u16, pub to: u16, pub resource: u16, pub action: u8, pub max_depth: u8 }
// r[depends domain-primitives.access-control-policy.delegation]
pub open spec fn policy_delegation_valid_spec(d: PolicyDelegation) -> bool { d.from != d.to && d.action < 4 && d.max_depth > 0 && d.max_depth <= 8 }
// r[depends domain-primitives.access-control-policy.delegation]
pub open spec fn policy_delegation_allows_step_spec(d: PolicyDelegation, from: u16, to: u16, resource: u16, action: u8, depth: u8) -> bool { policy_delegation_valid_spec(d) && d.from == from && d.to == to && d.resource == resource && d.action == action && depth < d.max_depth }
// r[impl domain-primitives.access-control-policy.delegation]
pub fn policy_delegation_valid(d: PolicyDelegation) -> (result: bool) ensures result == policy_delegation_valid_spec(d) { d.from != d.to && d.action < 4 && d.max_depth > 0 && d.max_depth <= 8 }
// r[impl domain-primitives.access-control-policy.delegation]
pub fn policy_delegation_allows_step(d: PolicyDelegation, from: u16, to: u16, resource: u16, action: u8, depth: u8) -> (result: bool) ensures result == policy_delegation_allows_step_spec(d,from,to,resource,action,depth) { policy_delegation_valid(d) && d.from == from && d.to == to && d.resource == resource && d.action == action && depth < d.max_depth }
// r[verify domain-primitives.access-control-policy.delegation]
pub proof fn policy_invalid_delegation_allows_no_step(d: PolicyDelegation, from: u16, to: u16, resource: u16, action: u8, depth: u8) requires !policy_delegation_valid_spec(d) ensures !policy_delegation_allows_step_spec(d,from,to,resource,action,depth) {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn delegation_bounds_depth() {
        let d = PolicyDelegation {
            from: 1,
            to: 2,
            resource: 3,
            action: 1,
            max_depth: 2,
        };
        assert!(policy_delegation_allows_step(d, 1, 2, 3, 1, 1));
        assert!(!policy_delegation_allows_step(d, 1, 2, 3, 1, 2));
    }
}
