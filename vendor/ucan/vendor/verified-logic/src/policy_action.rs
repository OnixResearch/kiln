//! Exact finite action admission primitives.
use vstd::prelude::*;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PolicyActionSet4 { pub bits: u8 }
// r[depends domain-primitives.access-control-policy.action]
pub open spec fn policy_action_valid_spec(a: u8) -> bool { a < 4 }
// r[depends domain-primitives.access-control-policy.action]
pub open spec fn policy_action_set_valid_spec(s: PolicyActionSet4) -> bool { s.bits < 16 }
// r[depends domain-primitives.access-control-policy.action]
pub open spec fn policy_action_contains_spec(s: PolicyActionSet4, a: u8) -> bool { policy_action_set_valid_spec(s) && policy_action_valid_spec(a) && (s.bits & (1u8 << a)) != 0 }
// r[impl domain-primitives.access-control-policy.action]
pub fn policy_action_valid(a: u8) -> (result: bool) ensures result == policy_action_valid_spec(a) { a < 4 }
// r[impl domain-primitives.access-control-policy.action]
pub fn policy_action_set_valid(s: PolicyActionSet4) -> (result: bool) ensures result == policy_action_set_valid_spec(s) { s.bits < 16 }
// r[impl domain-primitives.access-control-policy.action]
pub fn policy_action_contains(s: PolicyActionSet4, a: u8) -> (result: bool) ensures result == policy_action_contains_spec(s,a) { policy_action_set_valid(s) && policy_action_valid(a) && (s.bits & (1u8 << a)) != 0 }
// r[verify domain-primitives.access-control-policy.action]
pub proof fn policy_action_invalid_not_contains(s: PolicyActionSet4, a: u8) requires !policy_action_valid_spec(a) ensures !policy_action_contains_spec(s,a) {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bitset_contains() {
        let s = PolicyActionSet4 { bits: 0b1010 };
        assert!(policy_action_contains(s, 1));
        assert!(!policy_action_contains(s, 2));
    }
}
