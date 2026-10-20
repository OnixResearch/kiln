//! Exact finite checked error-signal envelope.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ControlErrorSignal { pub min_error: i64, pub max_error: i64, pub error: i64 }

pub open spec fn subtraction_safe_spec(target: i64, measurement: i64) -> bool { if measurement >= 0 { i64::MIN + measurement <= target } else { target <= i64::MAX + measurement } }
pub open spec fn control_error_signal_from_delta_spec(min_error: i64, max_error: i64, target: i64, measurement: i64) -> Option<ControlErrorSignal> { if min_error <= max_error && subtraction_safe_spec(target,measurement) && min_error <= target - measurement && target - measurement <= max_error { Some(ControlErrorSignal { min_error, max_error, error: (target - measurement) as i64 }) } else { None } }

// r[depends domain-primitives.control-error-signal]
pub open spec fn control_error_signal_valid_spec(e: ControlErrorSignal) -> bool { e.min_error <= e.max_error && e.min_error <= e.error && e.error <= e.max_error }

// r[impl domain-primitives.control-error-signal]
pub fn control_error_signal_valid(e: ControlErrorSignal) -> (result: bool) ensures result == control_error_signal_valid_spec(e) { e.min_error <= e.max_error && e.min_error <= e.error && e.error <= e.max_error }

// r[impl domain-primitives.control-error-signal]
pub fn control_error_signal_from_delta(min_error: i64, max_error: i64, target: i64, measurement: i64) -> (result: Option<ControlErrorSignal>) ensures result == control_error_signal_from_delta_spec(min_error,max_error,target,measurement) { if min_error <= max_error && ((measurement >= 0 && i64::MIN + measurement <= target) || (measurement < 0 && target <= i64::MAX + measurement)) { let d = target - measurement; if min_error <= d && d <= max_error { Some(ControlErrorSignal { min_error, max_error, error: d }) } else { None } } else { None } }

// r[verify domain-primitives.control-error-signal]
pub proof fn admitted_error_is_in_envelope(min_error: i64, max_error: i64, target: i64, measurement: i64, e: ControlErrorSignal)
    requires control_error_signal_from_delta_spec(min_error,max_error,target,measurement) == Some(e)
    ensures control_error_signal_valid_spec(e)
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn control_error_signal_bounds_delta() {
        assert_eq!(control_error_signal_from_delta(-5, 5, 10, 7).unwrap().error, 3);
        assert!(control_error_signal_from_delta(-1, 1, 10, 7).is_none());
    }
}
