//! Exact finite rate-limited output step.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ControlRateLimited { pub previous: i64, pub requested: i64, pub output: i64, pub max_step: i64 }

pub open spec fn control_rate_limit_valid_spec(previous: i64, max_step: i64) -> bool { 0 <= max_step && i64::MIN + max_step <= previous && previous <= i64::MAX - max_step }
pub open spec fn rate_limited_output_spec(previous: i64, requested: i64, max_step: i64) -> i64 { if requested > (previous + max_step) as i64 { (previous + max_step) as i64 } else if requested < (previous - max_step) as i64 { (previous - max_step) as i64 } else { requested } }
pub open spec fn control_rate_limit_apply_spec(previous: i64, requested: i64, max_step: i64) -> Option<ControlRateLimited> { if control_rate_limit_valid_spec(previous,max_step) { Some(ControlRateLimited { previous, requested, output: rate_limited_output_spec(previous,requested,max_step), max_step }) } else { None } }

// r[impl domain-primitives.control-rate-limit]
pub fn control_rate_limit_apply(previous: i64, requested: i64, max_step: i64) -> (result: Option<ControlRateLimited>) ensures result == control_rate_limit_apply_spec(previous,requested,max_step) { if 0 <= max_step && i64::MIN + max_step <= previous && previous <= i64::MAX - max_step { let up = previous + max_step; let down = previous - max_step; let output = if requested > up { up } else if requested < down { down } else { requested }; Some(ControlRateLimited { previous, requested, output, max_step }) } else { None } }

// r[verify domain-primitives.control-rate-limit]
pub proof fn rate_limited_output_within_step(previous: i64, requested: i64, max_step: i64, r: ControlRateLimited)
    requires control_rate_limit_apply_spec(previous,requested,max_step) == Some(r)
    ensures (previous - max_step) as i64 <= r.output, r.output <= (previous + max_step) as i64
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn control_rate_limits_steps() {
        assert_eq!(control_rate_limit_apply(10, 20, 3).unwrap().output, 13);
        assert_eq!(control_rate_limit_apply(10, 8, 3).unwrap().output, 8);
        assert!(control_rate_limit_apply(0, 1, -1).is_none());
    }
}
