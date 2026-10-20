//! Exact finite two-state hysteresis transition.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ControlMode { Low, High }

// r[depends domain-primitives.control-hysteresis]
pub open spec fn control_hysteresis_next_spec(low: i64, high: i64, input: i64, mode: ControlMode) -> Option<ControlMode> { if low < high { if input <= low { Some(ControlMode::Low) } else if input >= high { Some(ControlMode::High) } else { Some(mode) } } else { None } }

// r[impl domain-primitives.control-hysteresis]
pub fn control_hysteresis_next(low: i64, high: i64, input: i64, mode: ControlMode) -> (result: Option<ControlMode>) ensures result == control_hysteresis_next_spec(low,high,input,mode) { if low < high { if input <= low { Some(ControlMode::Low) } else if input >= high { Some(ControlMode::High) } else { Some(mode) } } else { None } }

// r[verify domain-primitives.control-hysteresis]
pub proof fn hold_band_preserves_mode(low: i64, high: i64, input: i64, mode: ControlMode)
    requires low < input, input < high
    ensures control_hysteresis_next_spec(low,high,input,mode) == Some(mode)
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn control_hysteresis_transitions() {
        assert_eq!(control_hysteresis_next(0, 10, 5, ControlMode::Low), Some(ControlMode::Low));
        assert_eq!(control_hysteresis_next(0, 10, 11, ControlMode::Low), Some(ControlMode::High));
        assert_eq!(control_hysteresis_next(0, 10, -1, ControlMode::High), Some(ControlMode::Low));
    }
}
