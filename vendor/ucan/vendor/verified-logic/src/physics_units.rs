//! Exact discrete physics unit/dimension primitives.
// r[impl domain-primitives.physics-non-claims]
// r[verify domain-primitives.physics-non-claims]
// Exact finite integer checks only; no continuous dynamics, floating-point, or simulator-fidelity
// claim.
use vstd::prelude::*;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PhysicsDimension { pub mass: i8, pub length: i8, pub time: i8 }

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PhysicsQuantity { pub value: i32, pub dim: PhysicsDimension }

// r[depends domain-primitives.physics-units]
pub open spec fn physics_same_dimension_spec(a: PhysicsDimension, b: PhysicsDimension) -> bool {
    a.mass == b.mass && a.length == b.length && a.time == b.time
}

// r[impl domain-primitives.physics-units]
pub fn physics_same_dimension(a: PhysicsDimension, b: PhysicsDimension) -> (result: bool)
    ensures result == physics_same_dimension_spec(a, b)
{ a.mass == b.mass && a.length == b.length && a.time == b.time }

// r[impl domain-primitives.physics-units]
pub fn physics_quantity_add(a: PhysicsQuantity, b: PhysicsQuantity) -> (result: Option<PhysicsQuantity>)
    ensures result.is_some() ==> physics_same_dimension_spec(a.dim, b.dim) && result.unwrap().dim == a.dim,
            result.is_none() ==> !physics_same_dimension_spec(a.dim, b.dim)
{
    if physics_same_dimension(a.dim, b.dim) { Some(PhysicsQuantity { value: a.value, dim: a.dim }) } else { None }
}

// r[verify domain-primitives.physics-units]
pub proof fn physics_add_preserves_dimension(a: PhysicsQuantity, b: PhysicsQuantity, r: PhysicsQuantity)
    requires physics_same_dimension_spec(a.dim, b.dim), r.dim == a.dim
    ensures physics_same_dimension_spec(r.dim, a.dim)
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compatible_units_add() {
        let d = PhysicsDimension {
            mass: 1,
            length: 0,
            time: -2,
        };
        let q = PhysicsQuantity { value: 2, dim: d };
        assert_eq!(physics_quantity_add(q, q).unwrap().dim, d);
        assert!(
            physics_quantity_add(q, PhysicsQuantity {
                value: 1,
                dim: PhysicsDimension {
                    mass: 0,
                    length: 1,
                    time: 0
                }
            })
            .is_none()
        );
    }
}
