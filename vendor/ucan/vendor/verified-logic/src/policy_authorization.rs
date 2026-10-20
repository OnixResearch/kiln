//! Exact finite authorization admission primitives.
use vstd::prelude::*;

use crate::policy_action::PolicyActionSet4;
use crate::policy_action::policy_action_contains;
use crate::policy_grant_denial::PolicyDecision;
use crate::policy_grant_denial::policy_decision_for_rule;
use crate::policy_principal::PolicyPrincipalSet3;
use crate::policy_principal::policy_principal_contains;
use crate::policy_resource::PolicyResourceSet3;
use crate::policy_resource::policy_resource_contains;
use crate::policy_rule::PolicyRule;
use crate::policy_rule::policy_rule_matches;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PolicyRequest { pub principal: u16, pub resource: u16, pub action: u8 }
// r[depends domain-primitives.access-control-policy.authorization]
pub open spec fn policy_request_admitted_spec(p: PolicyPrincipalSet3, r: PolicyResourceSet3, a: PolicyActionSet4, req: PolicyRequest) -> bool { crate::policy_principal::policy_principal_contains_spec(p,req.principal) && crate::policy_resource::policy_resource_contains_spec(r,req.resource) && crate::policy_action::policy_action_contains_spec(a,req.action) }
// r[depends domain-primitives.access-control-policy.authorization]
pub open spec fn policy_authorize_one_spec(p: PolicyPrincipalSet3, r: PolicyResourceSet3, a: PolicyActionSet4, rule: PolicyRule, req: PolicyRequest) -> PolicyDecision { if policy_request_admitted_spec(p,r,a,req) { crate::policy_grant_denial::policy_decision_for_rule_spec(rule,req.principal,req.resource,req.action) } else { PolicyDecision{admitted:false,granted:false,matched:false,priority:0} } }
// r[impl domain-primitives.access-control-policy.authorization]
pub fn policy_request_admitted(p: PolicyPrincipalSet3, r: PolicyResourceSet3, a: PolicyActionSet4, req: PolicyRequest) -> (result: bool) ensures result == policy_request_admitted_spec(p,r,a,req) { policy_principal_contains(p,req.principal) && policy_resource_contains(r,req.resource) && policy_action_contains(a,req.action) }
// r[impl domain-primitives.access-control-policy.authorization]
pub fn policy_authorize_one(p: PolicyPrincipalSet3, r: PolicyResourceSet3, a: PolicyActionSet4, rule: PolicyRule, req: PolicyRequest) -> (result: PolicyDecision) ensures result == policy_authorize_one_spec(p,r,a,rule,req) { if policy_request_admitted(p,r,a,req) { policy_decision_for_rule(rule,req.principal,req.resource,req.action) } else { PolicyDecision{admitted:false,granted:false,matched:false,priority:0} } }
// r[verify domain-primitives.access-control-policy.authorization]
pub proof fn policy_unadmitted_request_denies(p: PolicyPrincipalSet3, r: PolicyResourceSet3, a: PolicyActionSet4, rule: PolicyRule, req: PolicyRequest) requires !policy_request_admitted_spec(p,r,a,req) ensures !policy_authorize_one_spec(p,r,a,rule,req).admitted, !policy_authorize_one_spec(p,r,a,rule,req).granted {}
// r[verify domain-primitives.access-control-policy.authorization]
pub proof fn policy_admitted_matching_rule_records_match(p: PolicyPrincipalSet3, r: PolicyResourceSet3, a: PolicyActionSet4, rule: PolicyRule, req: PolicyRequest) requires policy_request_admitted_spec(p,r,a,req), crate::policy_rule::policy_rule_matches_spec(rule,req.principal,req.resource,req.action) ensures policy_authorize_one_spec(p,r,a,rule,req).matched {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn undeclared_principal_denies() {
        let ps = PolicyPrincipalSet3 {
            len: 1,
            p0: 1,
            p1: 0,
            p2: 0,
        };
        let rs = PolicyResourceSet3 {
            len: 1,
            r0: 2,
            r1: 0,
            r2: 0,
        };
        let acts = PolicyActionSet4 { bits: 1 };
        let rule = PolicyRule {
            principal: 1,
            resource: 2,
            action: 0,
            effect: 1,
            priority: 1,
        };
        let req = PolicyRequest {
            principal: 9,
            resource: 2,
            action: 0,
        };
        let d = policy_authorize_one(ps, rs, acts, rule, req);
        assert!(!d.admitted);
        assert!(!d.granted);
    }
}
