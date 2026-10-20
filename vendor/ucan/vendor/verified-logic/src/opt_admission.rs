//! Exact finite optimization admission composition.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OptAdmission {
    pub objective: u8,
    pub feasible: u8,
    pub ranking: u8,
    pub dominance: u8,
    pub bound: u8,
    pub witness: u8,
    pub frontier: u8,
}

pub open spec fn ok_flag_spec(v: u8) -> bool { v == 1 }

// r[depends domain-primitives.discrete-optimization.admission]
pub open spec fn opt_admission_spec(a: OptAdmission) -> bool {
    ok_flag_spec(a.objective) && ok_flag_spec(a.feasible) && ok_flag_spec(a.ranking)
        && ok_flag_spec(a.dominance) && ok_flag_spec(a.bound) && ok_flag_spec(a.witness)
        && ok_flag_spec(a.frontier)
}

// r[impl domain-primitives.discrete-optimization.admission]
pub fn opt_admitted(a: OptAdmission) -> (result: bool)
    ensures result == opt_admission_spec(a)
{
    a.objective == 1 && a.feasible == 1 && a.ranking == 1 && a.dominance == 1
        && a.bound == 1 && a.witness == 1 && a.frontier == 1
}

// r[verify domain-primitives.discrete-optimization.admission]
pub proof fn admitted_problem_has_witness(a: OptAdmission)
    requires opt_admission_spec(a)
    ensures a.witness == 1
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn admission_requires_all_parts() {
        let a = OptAdmission {
            objective: 1,
            feasible: 1,
            ranking: 1,
            dominance: 1,
            bound: 1,
            witness: 1,
            frontier: 1,
        };
        assert!(opt_admitted(a));
        let bad = OptAdmission { witness: 0, ..a };
        assert!(!opt_admitted(bad));
    }
}
