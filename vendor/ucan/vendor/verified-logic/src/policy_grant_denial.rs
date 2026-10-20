//! Exact finite grant/denial decision metadata.
use vstd::prelude::*;

use crate::policy_rule::PolicyRule;
use crate::policy_rule::policy_rule_matches;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PolicyDecision { pub admitted: bool, pub granted: bool, pub matched: bool, pub priority: u8 }
// r[depends domain-primitives.access-control-policy.grant-denial]
pub open spec fn policy_no_match_decision_spec() -> PolicyDecision { PolicyDecision{admitted:true,granted:false,matched:false,priority:0} }
// r[depends domain-primitives.access-control-policy.grant-denial]
pub open spec fn policy_decision_for_rule_spec(r: PolicyRule, p: u16, res: u16, a: u8) -> PolicyDecision { if crate::policy_rule::policy_rule_matches_spec(r,p,res,a) { PolicyDecision{admitted:true,granted:r.effect == 1,matched:true,priority:r.priority} } else { policy_no_match_decision_spec() } }
// r[impl domain-primitives.access-control-policy.grant-denial]
pub fn policy_no_match_decision() -> (result: PolicyDecision) ensures result == policy_no_match_decision_spec() { PolicyDecision{admitted:true,granted:false,matched:false,priority:0} }
// r[impl domain-primitives.access-control-policy.grant-denial]
pub fn policy_decision_for_rule(r: PolicyRule, p: u16, res: u16, a: u8) -> (result: PolicyDecision) ensures result == policy_decision_for_rule_spec(r,p,res,a) { if policy_rule_matches(r,p,res,a) { PolicyDecision{admitted:true,granted:r.effect == 1,matched:true,priority:r.priority} } else { policy_no_match_decision() } }
// r[verify domain-primitives.access-control-policy.grant-denial]
pub proof fn policy_no_match_denies(r: PolicyRule, p: u16, res: u16, a: u8) requires !crate::policy_rule::policy_rule_matches_spec(r,p,res,a) ensures !policy_decision_for_rule_spec(r,p,res,a).granted {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decision_records_rule() {
        let r = PolicyRule {
            principal: 1,
            resource: 2,
            action: 0,
            effect: 1,
            priority: 9,
        };
        let d = policy_decision_for_rule(r, 1, 2, 0);
        assert!(d.granted);
        assert!(d.matched);
        assert_eq!(d.priority, 9);
    }
}
