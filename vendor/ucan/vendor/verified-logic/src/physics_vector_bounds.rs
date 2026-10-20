//! Exact discrete vector envelope primitives.
use vstd::prelude::*;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PhysicsVec3 { pub x: i32, pub y: i32, pub z: i32 }
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PhysicsVec3Bounds { pub min: PhysicsVec3, pub max: PhysicsVec3 }

// r[depends domain-primitives.physics-vector-bounds]
pub open spec fn physics_vec_bounds_valid_spec(b: PhysicsVec3Bounds) -> bool {
    b.min.x <= b.max.x && b.min.y <= b.max.y && b.min.z <= b.max.z
}

// r[depends domain-primitives.physics-vector-bounds]
pub open spec fn physics_vec_in_bounds_spec(v: PhysicsVec3, b: PhysicsVec3Bounds) -> bool {
    physics_vec_bounds_valid_spec(b) && b.min.x <= v.x && v.x <= b.max.x && b.min.y <= v.y && v.y <= b.max.y && b.min.z <= v.z && v.z <= b.max.z
}

// r[impl domain-primitives.physics-vector-bounds]
pub fn physics_vec_in_bounds(v: PhysicsVec3, b: PhysicsVec3Bounds) -> (result: bool)
    ensures result == physics_vec_in_bounds_spec(v, b)
{ b.min.x <= b.max.x && b.min.y <= b.max.y && b.min.z <= b.max.z && b.min.x <= v.x && v.x <= b.max.x && b.min.y <= v.y && v.y <= b.max.y && b.min.z <= v.z && v.z <= b.max.z }

// r[verify domain-primitives.physics-vector-bounds]
pub proof fn physics_vec_accepted_has_valid_bounds(v: PhysicsVec3, b: PhysicsVec3Bounds)
    requires physics_vec_in_bounds_spec(v, b)
    ensures physics_vec_bounds_valid_spec(b)
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vector_bounds_admit_components() {
        let b = PhysicsVec3Bounds {
            min: PhysicsVec3 { x: 0, y: 0, z: 0 },
            max: PhysicsVec3 { x: 2, y: 2, z: 2 },
        };
        assert!(physics_vec_in_bounds(PhysicsVec3 { x: 1, y: 2, z: 0 }, b));
        assert!(!physics_vec_in_bounds(PhysicsVec3 { x: 3, y: 0, z: 0 }, b));
    }
}
