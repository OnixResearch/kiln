//! Exact bounded physics time horizon primitives.
use vstd::prelude::*;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PhysicsHorizon { pub dt: u16, pub steps: u16, pub max_time: u32 }

// r[depends domain-primitives.physics-time-horizon]
pub open spec fn physics_horizon_valid_spec(h: PhysicsHorizon) -> bool {
    0 < h.dt && 0 < h.steps && h.dt as nat <= h.max_time as nat
}

// r[depends domain-primitives.physics-time-horizon]
pub open spec fn physics_step_admitted_spec(h: PhysicsHorizon, step: u16) -> bool {
    physics_horizon_valid_spec(h) && step <= h.steps
}

// r[impl domain-primitives.physics-time-horizon]
pub fn physics_horizon_valid(h: PhysicsHorizon) -> (result: bool)
    ensures result == physics_horizon_valid_spec(h)
{
    h.dt > 0 && h.steps > 0 && h.dt as u32 <= h.max_time
}

// r[impl domain-primitives.physics-time-horizon]
pub fn physics_step_inside_horizon(h: PhysicsHorizon, step: u16) -> (result: bool)
    ensures result == physics_step_admitted_spec(h, step)
{
    physics_horizon_valid(h) && step <= h.steps
}

// r[verify domain-primitives.physics-time-horizon]
pub proof fn physics_valid_step_count_inside(h: PhysicsHorizon, step: u16)
    requires physics_step_admitted_spec(h, step)
    ensures step as nat <= h.steps as nat
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn horizon_bounds_steps() {
        let h = PhysicsHorizon {
            dt: 2,
            steps: 5,
            max_time: 10,
        };
        assert!(physics_horizon_valid(h));
        assert!(physics_step_inside_horizon(h, 5));
        assert!(!physics_horizon_valid(PhysicsHorizon {
            dt: 0,
            steps: 5,
            max_time: 10
        }));
    }
}
