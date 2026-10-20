//! Exact finite validation-error accumulation metadata.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct ValidationErrors { pub len: u8, pub e0: u8, pub e1: u8, pub e2: u8 }

// r[depends domain-primitives.data-validation.error-accumulation]
pub open spec fn validation_errors_valid_spec(e: ValidationErrors) -> bool { e.len <= 3 }
pub open spec fn validation_errors_add_spec(e: ValidationErrors, code: u8) -> ValidationErrors { if e.len == 0 { ValidationErrors { len: 1, e0: code, e1: e.e1, e2: e.e2 } } else if e.len == 1 { ValidationErrors { len: 2, e0: e.e0, e1: code, e2: e.e2 } } else if e.len == 2 { ValidationErrors { len: 3, e0: e.e0, e1: e.e1, e2: code } } else { e } }

// r[impl domain-primitives.data-validation.error-accumulation]
pub fn validation_errors_add(e: ValidationErrors, code: u8) -> (result: ValidationErrors) ensures result == validation_errors_add_spec(e,code) { if e.len == 0 { ValidationErrors { len: 1, e0: code, e1: e.e1, e2: e.e2 } } else if e.len == 1 { ValidationErrors { len: 2, e0: e.e0, e1: code, e2: e.e2 } } else if e.len == 2 { ValidationErrors { len: 3, e0: e.e0, e1: e.e1, e2: code } } else { e } }

// r[verify domain-primitives.data-validation.error-accumulation]
pub proof fn add_never_exceeds_capacity(e: ValidationErrors, code: u8) requires validation_errors_valid_spec(e) ensures validation_errors_valid_spec(validation_errors_add_spec(e,code)) {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validation_errors_accumulate_deterministically() {
        let e = ValidationErrors {
            len: 0,
            e0: 0,
            e1: 0,
            e2: 0,
        };
        let e = validation_errors_add(e, 7);
        assert_eq!(e.len, 1);
        assert_eq!(e.e0, 7);
    }
}
