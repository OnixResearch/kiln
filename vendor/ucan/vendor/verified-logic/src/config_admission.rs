//! Exact finite configuration/feature-flag admission composition.
use vstd::prelude::*;

use crate::config_validation_decision::ConfigValidationDecision;
use crate::config_validation_decision::config_validation_accept;
use crate::config_validation_decision::config_validation_reject;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ConfigAdmissionChecks { pub key: u8, pub value: u8, pub default_value: u8, pub override_value: u8, pub gate: u8, pub rollout: u8 }

// r[depends domain-primitives.configuration-feature-flag.admission]
pub open spec fn config_check_passed_spec(status: u8) -> bool { status == 1 }
// r[depends domain-primitives.configuration-feature-flag.admission]
pub open spec fn config_admission_all_passed_spec(c: ConfigAdmissionChecks) -> bool { config_check_passed_spec(c.key) && config_check_passed_spec(c.value) && config_check_passed_spec(c.default_value) && config_check_passed_spec(c.override_value) && config_check_passed_spec(c.gate) && config_check_passed_spec(c.rollout) }
// r[depends domain-primitives.configuration-feature-flag.admission]
pub open spec fn config_admission_spec(c: ConfigAdmissionChecks) -> ConfigValidationDecision { if config_admission_all_passed_spec(c) { crate::config_validation_decision::config_validation_accept_spec() } else { crate::config_validation_decision::config_validation_reject_spec(1) } }
// r[impl domain-primitives.configuration-feature-flag.admission]
pub fn config_check_passed(status: u8) -> (result: bool) ensures result == config_check_passed_spec(status) { status == 1 }
// r[impl domain-primitives.configuration-feature-flag.admission]
pub fn config_admission_all_passed(c: ConfigAdmissionChecks) -> (result: bool) ensures result == config_admission_all_passed_spec(c) { config_check_passed(c.key) && config_check_passed(c.value) && config_check_passed(c.default_value) && config_check_passed(c.override_value) && config_check_passed(c.gate) && config_check_passed(c.rollout) }
// r[impl domain-primitives.configuration-feature-flag.admission]
pub fn config_admission(c: ConfigAdmissionChecks) -> (result: ConfigValidationDecision) ensures result == config_admission_spec(c) { if config_admission_all_passed(c) { config_validation_accept() } else { config_validation_reject(1) } }
// r[verify domain-primitives.configuration-feature-flag.admission]
pub proof fn admission_requires_all_checks(c: ConfigAdmissionChecks) requires !config_admission_all_passed_spec(c) ensures !config_admission_spec(c).accepted {}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn admission_requires_all_checks_runtime() {
        let ok = ConfigAdmissionChecks {
            key: 1,
            value: 1,
            default_value: 1,
            override_value: 1,
            gate: 1,
            rollout: 1,
        };
        assert!(config_admission(ok).accepted);
        let bad = ConfigAdmissionChecks {
            key: 1,
            value: 1,
            default_value: 0,
            override_value: 1,
            gate: 1,
            rollout: 1,
        };
        assert!(!config_admission(bad).accepted);
    }
}
