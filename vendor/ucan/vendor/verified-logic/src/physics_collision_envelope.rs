//! Conservative exact swept-envelope collision candidates.
use vstd::prelude::*;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PhysicsInterval { pub lo: i32, pub hi: i32 }

// r[depends domain-primitives.physics-collision-envelope]
pub open spec fn physics_interval_valid_spec(i: PhysicsInterval) -> bool { i.lo <= i.hi }

// r[depends domain-primitives.physics-collision-envelope]
pub open spec fn physics_interval_overlap_spec(a: PhysicsInterval, b: PhysicsInterval) -> bool { physics_interval_valid_spec(a) && physics_interval_valid_spec(b) && a.lo <= b.hi && b.lo <= a.hi }

// r[impl domain-primitives.physics-collision-envelope]
pub fn physics_collision_candidate(a: PhysicsInterval, b: PhysicsInterval) -> (result: bool)
    ensures result == physics_interval_overlap_spec(a, b)
{ a.lo <= a.hi && b.lo <= b.hi && a.lo <= b.hi && b.lo <= a.hi }

// r[verify domain-primitives.physics-collision-envelope]
pub proof fn physics_overlap_symmetric(a: PhysicsInterval, b: PhysicsInterval)
    ensures physics_interval_overlap_spec(a,b) == physics_interval_overlap_spec(b,a)
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn collision_candidates_are_interval_overlaps() {
        assert!(physics_collision_candidate(PhysicsInterval { lo: 0, hi: 2 }, PhysicsInterval { lo: 2, hi: 4 }));
        assert!(!physics_collision_candidate(PhysicsInterval { lo: 0, hi: 1 }, PhysicsInterval { lo: 2, hi: 4 }));
    }
}
