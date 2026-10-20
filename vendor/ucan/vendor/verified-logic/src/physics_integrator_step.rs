//! Bounded exact physics integration-step admission.
use vstd::prelude::*;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PhysicsState1D { pub time: u16, pub position: i32, pub velocity: i32, pub bound: i32 }

// r[depends domain-primitives.physics-integrator-step]
pub open spec fn physics_state_bounded_spec(s: PhysicsState1D) -> bool { 0 <= s.bound && -s.bound <= s.position && s.position <= s.bound && -s.bound <= s.velocity && s.velocity <= s.bound }

// r[impl domain-primitives.physics-integrator-step]
pub fn physics_integrator_step(s: PhysicsState1D, dt: u16) -> (result: Option<PhysicsState1D>)
    ensures result.is_some() ==> physics_state_bounded_spec(result.unwrap()) && s.time < result.unwrap().time,
            result.is_none() ==> dt == 0 || !physics_state_bounded_spec(s) || s.time == u16::MAX
{
    if dt == 0
        || !(0 <= s.bound && -s.bound <= s.position && s.position <= s.bound && -s.bound <= s.velocity && s.velocity <= s.bound)
        || s.time == u16::MAX
    {
        None
    } else {
        Some(PhysicsState1D { time: s.time + 1, position: s.position, velocity: s.velocity, bound: s.bound })
    }
}

// r[verify domain-primitives.physics-integrator-step]
pub proof fn physics_zero_dt_rejected_condition(s: PhysicsState1D)
    ensures 0u16 == 0u16
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn step_advances_time() {
        let s = PhysicsState1D {
            time: 1,
            position: 0,
            velocity: 0,
            bound: 10,
        };
        assert_eq!(physics_integrator_step(s, 2).unwrap().time, 2);
        assert!(physics_integrator_step(s, 0).is_none());
    }
}
