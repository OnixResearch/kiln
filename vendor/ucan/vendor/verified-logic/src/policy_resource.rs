//! Exact finite resource admission primitives.
use vstd::prelude::*;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PolicyResourceSet3 { pub len: u8, pub r0: u16, pub r1: u16, pub r2: u16 }
// r[depends domain-primitives.access-control-policy.resource]
pub open spec fn policy_resource_valid_spec(s: PolicyResourceSet3) -> bool { s.len <= 3 && (s.len <= 1 || s.r0 != s.r1) && (s.len <= 2 || (s.r0 != s.r2 && s.r1 != s.r2)) }
// r[depends domain-primitives.access-control-policy.resource]
pub open spec fn policy_resource_contains_spec(s: PolicyResourceSet3, r: u16) -> bool { policy_resource_valid_spec(s) && ((s.len >= 1 && s.r0 == r) || (s.len >= 2 && s.r1 == r) || (s.len >= 3 && s.r2 == r)) }
// r[impl domain-primitives.access-control-policy.resource]
pub fn policy_resource_valid(s: PolicyResourceSet3) -> (result: bool) ensures result == policy_resource_valid_spec(s) { s.len <= 3 && (s.len <= 1 || s.r0 != s.r1) && (s.len <= 2 || (s.r0 != s.r2 && s.r1 != s.r2)) }
// r[impl domain-primitives.access-control-policy.resource]
pub fn policy_resource_contains(s: PolicyResourceSet3, r: u16) -> (result: bool) ensures result == policy_resource_contains_spec(s,r) { policy_resource_valid(s) && ((s.len >= 1 && s.r0 == r) || (s.len >= 2 && s.r1 == r) || (s.len >= 3 && s.r2 == r)) }
// r[verify domain-primitives.access-control-policy.resource]
pub proof fn policy_resource_invalid_contains_false(s: PolicyResourceSet3, r: u16) requires !policy_resource_valid_spec(s) ensures !policy_resource_contains_spec(s,r) {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contains_declared() {
        let s = PolicyResourceSet3 {
            len: 1,
            r0: 5,
            r1: 0,
            r2: 0,
        };
        assert!(policy_resource_contains(s, 5));
        assert!(!policy_resource_contains(s, 6));
    }
}
