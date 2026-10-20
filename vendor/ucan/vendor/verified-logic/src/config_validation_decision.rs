//! Exact finite validation-decision metadata.
use vstd::prelude::*;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ConfigValidationDecision { pub accepted: bool, pub reason: u8 }

// r[depends domain-primitives.configuration-feature-flag.validation-decision]
pub open spec fn config_validation_accept_spec() -> ConfigValidationDecision { ConfigValidationDecision{accepted:true,reason:0} }
// r[depends domain-primitives.configuration-feature-flag.validation-decision]
pub open spec fn config_validation_reject_spec(reason: u8) -> ConfigValidationDecision { if reason == 0 { ConfigValidationDecision{accepted:false,reason:255} } else { ConfigValidationDecision{accepted:false,reason} } }
// r[impl domain-primitives.configuration-feature-flag.validation-decision]
pub fn config_validation_accept() -> (result: ConfigValidationDecision) ensures result == config_validation_accept_spec() { ConfigValidationDecision{accepted:true,reason:0} }
// r[impl domain-primitives.configuration-feature-flag.validation-decision]
pub fn config_validation_reject(reason: u8) -> (result: ConfigValidationDecision) ensures result == config_validation_reject_spec(reason) { if reason == 0 { ConfigValidationDecision{accepted:false,reason:255} } else { ConfigValidationDecision{accepted:false,reason} } }
// r[verify domain-primitives.configuration-feature-flag.validation-decision]
pub proof fn rejection_preserves_nonzero_reason(reason: u8) requires reason != 0 ensures !config_validation_reject_spec(reason).accepted, config_validation_reject_spec(reason).reason == reason {}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejection_reason_is_explicit() {
        let d = config_validation_reject(3);
        assert!(!d.accepted);
        assert_eq!(d.reason, 3);
        assert_eq!(config_validation_reject(0).reason, 255);
    }
}
