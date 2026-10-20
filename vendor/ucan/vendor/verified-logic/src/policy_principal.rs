//! Exact finite principal admission primitives.
use vstd::prelude::*;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PolicyPrincipalSet3 { pub len: u8, pub p0: u16, pub p1: u16, pub p2: u16 }
// r[depends domain-primitives.access-control-policy.principal]
pub open spec fn policy_principal_valid_spec(s: PolicyPrincipalSet3) -> bool { s.len <= 3 && (s.len <= 1 || s.p0 != s.p1) && (s.len <= 2 || (s.p0 != s.p2 && s.p1 != s.p2)) }
// r[depends domain-primitives.access-control-policy.principal]
pub open spec fn policy_principal_contains_spec(s: PolicyPrincipalSet3, p: u16) -> bool { policy_principal_valid_spec(s) && ((s.len >= 1 && s.p0 == p) || (s.len >= 2 && s.p1 == p) || (s.len >= 3 && s.p2 == p)) }
// r[impl domain-primitives.access-control-policy.principal]
pub fn policy_principal_valid(s: PolicyPrincipalSet3) -> (result: bool) ensures result == policy_principal_valid_spec(s) { s.len <= 3 && (s.len <= 1 || s.p0 != s.p1) && (s.len <= 2 || (s.p0 != s.p2 && s.p1 != s.p2)) }
// r[impl domain-primitives.access-control-policy.principal]
pub fn policy_principal_contains(s: PolicyPrincipalSet3, p: u16) -> (result: bool) ensures result == policy_principal_contains_spec(s,p) { policy_principal_valid(s) && ((s.len >= 1 && s.p0 == p) || (s.len >= 2 && s.p1 == p) || (s.len >= 3 && s.p2 == p)) }
// r[verify domain-primitives.access-control-policy.principal]
pub proof fn policy_principal_invalid_contains_false(s: PolicyPrincipalSet3, p: u16) requires !policy_principal_valid_spec(s) ensures !policy_principal_contains_spec(s,p) {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contains_declared() {
        let s = PolicyPrincipalSet3 {
            len: 2,
            p0: 7,
            p1: 9,
            p2: 0,
        };
        assert!(policy_principal_contains(s, 9));
        assert!(!policy_principal_contains(s, 8));
    }
}
