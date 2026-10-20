//! Exact finite validation admission/rejection composition.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ValidationAdmission {
    pub schema_ok: bool,
    pub field_code: u8,
    pub enum_ok: bool,
    pub range_ok: bool,
    pub cross_code: u8,
    pub errors_len: u8,
    pub normalization_code: u8,
}

pub open spec fn field_present_code_spec(code: u8) -> bool { code == 1 }
pub open spec fn cross_satisfied_code_spec(code: u8) -> bool { code == 1 }
pub open spec fn normalization_admitted_code_spec(code: u8) -> bool { code == 0 || code == 1 }

// r[depends domain-primitives.data-validation.admission]
pub open spec fn validation_admission_spec(v: ValidationAdmission) -> bool {
    v.schema_ok && field_present_code_spec(v.field_code) && v.enum_ok && v.range_ok
        && cross_satisfied_code_spec(v.cross_code) && v.errors_len == 0
        && normalization_admitted_code_spec(v.normalization_code)
}

// r[impl domain-primitives.data-validation.admission]
pub fn validation_admitted(v: ValidationAdmission) -> (result: bool)
    ensures result == validation_admission_spec(v)
{
    v.schema_ok && v.field_code == 1 && v.enum_ok && v.range_ok && v.cross_code == 1
        && v.errors_len == 0 && (v.normalization_code == 0 || v.normalization_code == 1)
}

// r[verify domain-primitives.data-validation.admission]
pub proof fn admitted_validation_has_no_errors(v: ValidationAdmission)
    requires validation_admission_spec(v)
    ensures v.errors_len == 0
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validation_admission_requires_all_checks() {
        let v = ValidationAdmission {
            schema_ok: true,
            field_code: 1,
            enum_ok: true,
            range_ok: true,
            cross_code: 1,
            errors_len: 0,
            normalization_code: 0,
        };
        assert!(validation_admitted(v));
        let bad = ValidationAdmission { errors_len: 1, ..v };
        assert!(!validation_admitted(bad));
    }
}
