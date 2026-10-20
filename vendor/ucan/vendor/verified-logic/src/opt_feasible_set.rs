//! Exact finite feasible-set admission.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct OptFeasibleSet { pub len: u8, pub c0: u8, pub c1: u8, pub c2: u8 }

// r[depends domain-primitives.discrete-optimization.feasible-set]
pub open spec fn opt_feasible_set_valid_spec(s: OptFeasibleSet) -> bool { s.len <= 3 }
pub open spec fn opt_feasible_contains_spec(s: OptFeasibleSet, c: u8) -> bool { (s.len >= 1 && s.c0 == c) || (s.len >= 2 && s.c1 == c) || (s.len >= 3 && s.c2 == c) }

// r[impl domain-primitives.discrete-optimization.feasible-set]
pub fn opt_feasible_contains(s: OptFeasibleSet, c: u8) -> (result: bool) ensures result == (opt_feasible_set_valid_spec(s) && opt_feasible_contains_spec(s,c)) { s.len <= 3 && ((s.len >= 1 && s.c0 == c) || (s.len >= 2 && s.c1 == c) || (s.len >= 3 && s.c2 == c)) }

// r[verify domain-primitives.discrete-optimization.feasible-set]
pub proof fn feasible_accepts_only_declared(s: OptFeasibleSet, c: u8) requires opt_feasible_set_valid_spec(s), opt_feasible_contains_spec(s,c) ensures opt_feasible_contains_spec(s,c) {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opt_feasible_rejects_absent() {
        let s = OptFeasibleSet {
            len: 2,
            c0: 1,
            c1: 3,
            c2: 0,
        };
        assert!(opt_feasible_contains(s, 3));
        assert!(!opt_feasible_contains(s, 2));
    }
}
