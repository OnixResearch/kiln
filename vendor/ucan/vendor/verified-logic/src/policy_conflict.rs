//! Exact finite policy conflict-resolution primitives.
use vstd::prelude::*;

use crate::policy_rule::PolicyRule;

verus! {
// r[depends domain-primitives.access-control-policy.conflict]
pub open spec fn policy_rules_overlap_spec(a: PolicyRule, b: PolicyRule) -> bool { a.principal == b.principal && a.resource == b.resource && a.action == b.action }
// r[depends domain-primitives.access-control-policy.conflict]
pub open spec fn policy_choose_rule_spec(a: PolicyRule, b: PolicyRule) -> PolicyRule { if a.priority >= b.priority { a } else { b } }
// r[impl domain-primitives.access-control-policy.conflict]
pub fn policy_rules_overlap(a: PolicyRule, b: PolicyRule) -> (result: bool) ensures result == policy_rules_overlap_spec(a,b) { a.principal == b.principal && a.resource == b.resource && a.action == b.action }
// r[impl domain-primitives.access-control-policy.conflict]
pub fn policy_choose_rule(a: PolicyRule, b: PolicyRule) -> (result: PolicyRule) ensures result == policy_choose_rule_spec(a,b) { if a.priority >= b.priority { a } else { b } }
// r[verify domain-primitives.access-control-policy.conflict]
pub proof fn policy_choose_priority_at_least_left_or_right(a: PolicyRule, b: PolicyRule) ensures policy_choose_rule_spec(a,b).priority == a.priority || policy_choose_rule_spec(a,b).priority == b.priority {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn higher_priority_wins() {
        let a = PolicyRule {
            principal: 1,
            resource: 1,
            action: 1,
            effect: 0,
            priority: 1,
        };
        let b = PolicyRule { priority: 2, ..a };
        assert_eq!(policy_choose_rule(a, b), b);
    }
}
