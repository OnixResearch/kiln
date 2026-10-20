//! Exact finite bounded setpoint admission.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ControlSetpoint { pub min: i64, pub max: i64, pub value: i64 }

// r[depends domain-primitives.control-setpoint]
pub open spec fn control_setpoint_valid_spec(s: ControlSetpoint) -> bool { s.min <= s.max && s.min <= s.value && s.value <= s.max }

pub open spec fn control_setpoint_update_spec(prior: ControlSetpoint, target: i64) -> ControlSetpoint { if prior.min <= prior.max && prior.min <= target && target <= prior.max { ControlSetpoint { min: prior.min, max: prior.max, value: target } } else { prior } }

// r[impl domain-primitives.control-setpoint]
pub fn control_setpoint_valid(s: ControlSetpoint) -> (result: bool) ensures result == control_setpoint_valid_spec(s) { s.min <= s.max && s.min <= s.value && s.value <= s.max }

// r[impl domain-primitives.control-setpoint]
pub fn control_setpoint_update(prior: ControlSetpoint, target: i64) -> (result: ControlSetpoint) ensures result == control_setpoint_update_spec(prior,target) { if prior.min <= prior.max && prior.min <= target && target <= prior.max { ControlSetpoint { min: prior.min, max: prior.max, value: target } } else { prior } }

// r[verify domain-primitives.control-setpoint]
pub proof fn valid_update_preserves_bounds(prior: ControlSetpoint, target: i64)
    requires prior.min <= prior.max, prior.min <= target, target <= prior.max
    ensures control_setpoint_valid_spec(control_setpoint_update_spec(prior, target))
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn control_setpoint_accepts_and_rejects() {
        let s = ControlSetpoint {
            min: 0,
            max: 10,
            value: 3,
        };
        assert_eq!(control_setpoint_update(s, 7).value, 7);
        assert_eq!(control_setpoint_update(s, 11), s);
    }
}
