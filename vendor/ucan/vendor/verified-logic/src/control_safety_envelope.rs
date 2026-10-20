//! Exact finite state/input safety-envelope guard.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ControlSafetyEnvelope { pub state_min: i64, pub state_max: i64, pub input_min: i64, pub input_max: i64 }

// r[depends domain-primitives.control-safety-envelope]
pub open spec fn control_safety_envelope_valid_spec(e: ControlSafetyEnvelope) -> bool { e.state_min <= e.state_max && e.input_min <= e.input_max }

pub open spec fn control_safety_guard_spec(e: ControlSafetyEnvelope, state: i64, input: i64) -> bool { control_safety_envelope_valid_spec(e) && e.state_min <= state && state <= e.state_max && e.input_min <= input && input <= e.input_max }

// r[impl domain-primitives.control-safety-envelope]
pub fn control_safety_guard(e: ControlSafetyEnvelope, state: i64, input: i64) -> (result: bool) ensures result == control_safety_guard_spec(e,state,input) { e.state_min <= e.state_max && e.input_min <= e.input_max && e.state_min <= state && state <= e.state_max && e.input_min <= input && input <= e.input_max }

// r[verify domain-primitives.control-safety-envelope]
pub proof fn accepted_guard_has_valid_envelope(e: ControlSafetyEnvelope, state: i64, input: i64)
    requires control_safety_guard_spec(e,state,input)
    ensures control_safety_envelope_valid_spec(e)
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn control_safety_guard_checks_bounds() {
        let e = ControlSafetyEnvelope {
            state_min: 0,
            state_max: 5,
            input_min: -1,
            input_max: 1,
        };
        assert!(control_safety_guard(e, 3, 0));
        assert!(!control_safety_guard(e, 6, 0));
    }
}
