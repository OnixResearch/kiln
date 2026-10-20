//! Exact finite deadband decisions with inclusive thresholds.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ControlDeadband { pub low: i64, pub high: i64, pub no_action: bool }

// r[depends domain-primitives.control-deadband]
pub open spec fn control_deadband_decision_spec(low: i64, high: i64, error: i64) -> Option<ControlDeadband> { if low <= high { Some(ControlDeadband { low, high, no_action: low <= error && error <= high }) } else { None } }

// r[impl domain-primitives.control-deadband]
pub fn control_deadband_decision(low: i64, high: i64, error: i64) -> (result: Option<ControlDeadband>) ensures result == control_deadband_decision_spec(low,high,error) { if low <= high { Some(ControlDeadband { low, high, no_action: low <= error && error <= high }) } else { None } }

// r[verify domain-primitives.control-deadband]
pub proof fn in_band_is_no_action(low: i64, high: i64, error: i64)
    requires low <= error, error <= high
    ensures control_deadband_decision_spec(low,high,error) == Some(ControlDeadband { low, high, no_action: true })
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn control_deadband_inclusive() {
        assert!(control_deadband_decision(-2, 2, 2).unwrap().no_action);
        assert!(!control_deadband_decision(-2, 2, 3).unwrap().no_action);
        assert!(control_deadband_decision(2, -2, 0).is_none());
    }
}
