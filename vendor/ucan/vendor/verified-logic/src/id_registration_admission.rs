//! Exact finite identifier/namespace primitive for RegistrationAdmission.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RegistrationAdmission {
    pub component_count: u8,
    pub passed_count: u8,
    pub rejected: bool,
    pub deterministic: bool,
}

// r[depends domain-primitives.identifier-namespace.registration]
pub open spec fn id_registration_admission_admitted_spec(x: RegistrationAdmission) -> bool {
    x.component_count > 0 && x.passed_count == x.component_count && !x.rejected && x.deterministic
}

// r[impl domain-primitives.identifier-namespace.registration]
pub fn id_registration_admission_admitted(x: RegistrationAdmission) -> (result: bool)
    ensures result == id_registration_admission_admitted_spec(x)
{
    x.component_count > 0 && x.passed_count == x.component_count && !x.rejected && x.deterministic
}

// r[verify domain-primitives.identifier-namespace.registration]
pub proof fn admitted_id_registration_admission_has_required_metadata(x: RegistrationAdmission)
    requires id_registration_admission_admitted_spec(x)
    ensures x.component_count > 0, x.passed_count == x.component_count, !x.rejected, x.deterministic
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn id_registration_admission_accepts_and_rejects() {
        assert!(id_registration_admission_admitted(RegistrationAdmission {
            component_count: 6,
            passed_count: 6,
            rejected: false,
            deterministic: true
        }));
        assert!(!id_registration_admission_admitted(RegistrationAdmission {
            component_count: 6,
            passed_count: 5,
            rejected: false,
            deterministic: true
        }));
    }
}
