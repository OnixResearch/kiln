//! Exact finite constraint admission result envelopes.
use vstd::prelude::*;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ConstraintAdmissionInputs { pub variables: u8, pub domains: u8, pub relations: u8, pub assignment: u8, pub propagation: u8, pub witness: u8 }
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ConstraintAdmission { pub admitted: bool, pub status: u8 }

// r[depends domain-primitives.constraint-solver.admission]
pub open spec fn constraint_status_ok_spec(status: u8) -> bool { status == 1 }
// r[depends domain-primitives.constraint-solver.admission]
pub open spec fn constraint_admission_spec(i: ConstraintAdmissionInputs) -> ConstraintAdmission { if constraint_status_ok_spec(i.variables) && constraint_status_ok_spec(i.domains) && constraint_status_ok_spec(i.relations) && constraint_status_ok_spec(i.assignment) && constraint_status_ok_spec(i.propagation) && constraint_status_ok_spec(i.witness) { ConstraintAdmission{admitted:true,status:1} } else { ConstraintAdmission{admitted:false,status:2} } }
// r[impl domain-primitives.constraint-solver.admission]
pub fn constraint_status_ok(status: u8) -> (result: bool) ensures result == constraint_status_ok_spec(status) { status == 1 }
// r[impl domain-primitives.constraint-solver.admission]
pub fn constraint_admission(i: ConstraintAdmissionInputs) -> (result: ConstraintAdmission) ensures result == constraint_admission_spec(i) { if constraint_status_ok(i.variables) && constraint_status_ok(i.domains) && constraint_status_ok(i.relations) && constraint_status_ok(i.assignment) && constraint_status_ok(i.propagation) && constraint_status_ok(i.witness) { ConstraintAdmission{admitted:true,status:1} } else { ConstraintAdmission{admitted:false,status:2} } }
// r[verify domain-primitives.constraint-solver.admission]
pub proof fn admission_rejects_bad_status(i: ConstraintAdmissionInputs) requires !(constraint_status_ok_spec(i.variables) && constraint_status_ok_spec(i.domains) && constraint_status_ok_spec(i.relations) && constraint_status_ok_spec(i.assignment) && constraint_status_ok_spec(i.propagation) && constraint_status_ok_spec(i.witness)) ensures !constraint_admission_spec(i).admitted {}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn admission_rejects_any_bad_component() {
        let ok = ConstraintAdmissionInputs {
            variables: 1,
            domains: 1,
            relations: 1,
            assignment: 1,
            propagation: 1,
            witness: 1,
        };
        assert!(constraint_admission(ok).admitted);
        let bad = ConstraintAdmissionInputs {
            variables: 1,
            domains: 0,
            relations: 1,
            assignment: 1,
            propagation: 1,
            witness: 1,
        };
        assert!(!constraint_admission(bad).admitted);
    }
}
