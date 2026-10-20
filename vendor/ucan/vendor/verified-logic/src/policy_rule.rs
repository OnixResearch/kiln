//! Exact finite policy-rule envelope primitives.
use vstd::prelude::*;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PolicyRule { pub principal: u16, pub resource: u16, pub action: u8, pub effect: u8, pub priority: u8 }
// r[depends domain-primitives.access-control-policy.rule]
pub open spec fn policy_effect_valid_spec(e: u8) -> bool { e == 0 || e == 1 }
// r[depends domain-primitives.access-control-policy.rule]
pub open spec fn policy_rule_valid_spec(r: PolicyRule) -> bool { r.action < 4 && policy_effect_valid_spec(r.effect) }
// r[depends domain-primitives.access-control-policy.rule]
pub open spec fn policy_rule_matches_spec(r: PolicyRule, principal: u16, resource: u16, action: u8) -> bool { policy_rule_valid_spec(r) && r.principal == principal && r.resource == resource && r.action == action }
// r[impl domain-primitives.access-control-policy.rule]
pub fn policy_effect_valid(e: u8) -> (result: bool) ensures result == policy_effect_valid_spec(e) { e == 0 || e == 1 }
// r[impl domain-primitives.access-control-policy.rule]
pub fn policy_rule_valid(r: PolicyRule) -> (result: bool) ensures result == policy_rule_valid_spec(r) { r.action < 4 && policy_effect_valid(r.effect) }
// r[impl domain-primitives.access-control-policy.rule]
pub fn policy_rule_matches(r: PolicyRule, principal: u16, resource: u16, action: u8) -> (result: bool) ensures result == policy_rule_matches_spec(r,principal,resource,action) { policy_rule_valid(r) && r.principal == principal && r.resource == resource && r.action == action }
// r[verify domain-primitives.access-control-policy.rule]
pub proof fn policy_invalid_rule_never_matches(r: PolicyRule, p: u16, res: u16, a: u8) requires !policy_rule_valid_spec(r) ensures !policy_rule_matches_spec(r,p,res,a) {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matches_exact_scope() {
        let r = PolicyRule {
            principal: 1,
            resource: 2,
            action: 3,
            effect: 1,
            priority: 4,
        };
        assert!(policy_rule_matches(r, 1, 2, 3));
        assert!(!policy_rule_matches(r, 1, 2, 0));
    }
}
