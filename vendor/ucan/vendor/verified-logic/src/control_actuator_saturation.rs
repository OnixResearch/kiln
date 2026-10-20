//! Exact finite actuator saturation and clamp semantics.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ControlActuatorCommand { pub min: i64, pub max: i64, pub output: i64, pub saturated: bool }

pub open spec fn clamp_spec(min: i64, max: i64, cmd: i64) -> i64 { if cmd < min { min } else if cmd > max { max } else { cmd } }
pub open spec fn saturation_flag_spec(min: i64, max: i64, cmd: i64) -> bool { cmd < min || cmd > max }
pub open spec fn control_actuator_saturate_spec(min: i64, max: i64, cmd: i64) -> Option<ControlActuatorCommand> { if min <= max { Some(ControlActuatorCommand { min, max, output: clamp_spec(min,max,cmd), saturated: saturation_flag_spec(min,max,cmd) }) } else { None } }

// r[depends domain-primitives.control-actuator-saturation]
pub open spec fn control_actuator_command_valid_spec(c: ControlActuatorCommand) -> bool { c.min <= c.max && c.min <= c.output && c.output <= c.max }

// r[impl domain-primitives.control-actuator-saturation]
pub fn control_actuator_saturate(min: i64, max: i64, cmd: i64) -> (result: Option<ControlActuatorCommand>) ensures result == control_actuator_saturate_spec(min,max,cmd) { if min <= max { let output = if cmd < min { min } else if cmd > max { max } else { cmd }; Some(ControlActuatorCommand { min, max, output, saturated: cmd < min || cmd > max }) } else { None } }

// r[verify domain-primitives.control-actuator-saturation]
pub proof fn saturated_output_within_bounds(min: i64, max: i64, cmd: i64, c: ControlActuatorCommand)
    requires control_actuator_saturate_spec(min,max,cmd) == Some(c)
    ensures min <= c.output, c.output <= max
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn control_actuator_clamps() {
        let c = control_actuator_saturate(0, 10, 20).unwrap();
        assert_eq!(c.output, 10);
        assert!(c.saturated);
        assert!(control_actuator_saturate(10, 0, 4).is_none());
    }
}
