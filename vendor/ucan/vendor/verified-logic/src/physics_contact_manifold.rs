//! Bounded exact contact manifold admission.
use vstd::prelude::*;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PhysicsContact { pub normal_axis: u8, pub normal_sign: i8, pub penetration: u32, pub max_penetration: u32 }

// r[depends domain-primitives.physics-contact-manifold]
pub open spec fn physics_contact_valid_spec(c: PhysicsContact) -> bool { c.normal_axis < 3 && (c.normal_sign == -1 || c.normal_sign == 1) && c.penetration <= c.max_penetration }

// r[impl domain-primitives.physics-contact-manifold]
pub fn physics_contact_valid(c: PhysicsContact) -> (result: bool)
    ensures result == physics_contact_valid_spec(c)
{ c.normal_axis < 3 && (c.normal_sign == -1 || c.normal_sign == 1) && c.penetration <= c.max_penetration }

// r[verify domain-primitives.physics-contact-manifold]
pub proof fn physics_valid_contact_has_axis(c: PhysicsContact)
    requires physics_contact_valid_spec(c)
    ensures c.normal_axis < 3
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contact_admission_checks_axis_sign_penetration() {
        assert!(physics_contact_valid(PhysicsContact {
            normal_axis: 1,
            normal_sign: -1,
            penetration: 2,
            max_penetration: 3
        }));
        assert!(!physics_contact_valid(PhysicsContact {
            normal_axis: 3,
            normal_sign: 1,
            penetration: 0,
            max_penetration: 3
        }));
    }
}
