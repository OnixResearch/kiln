//! Exact finite containment envelope.
use vstd::prelude::*;
verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GeomContainment { pub point_admitted: bool, pub bounds_admitted: bool, pub inside: bool }
// r[depends domain-primitives.geometry-foundation.containment]
pub open spec fn geom_containment_valid_spec(c: GeomContainment) -> bool { c.point_admitted && c.bounds_admitted && c.inside }
// r[impl domain-primitives.geometry-foundation.containment]
pub fn geom_containment_valid(c: GeomContainment) -> (result: bool) ensures result == geom_containment_valid_spec(c) { c.point_admitted && c.bounds_admitted && c.inside }
// r[verify domain-primitives.geometry-foundation.containment]
pub proof fn contained_point_has_admitted_inputs(c: GeomContainment) requires geom_containment_valid_spec(c) ensures c.point_admitted, c.bounds_admitted {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn containment_requires_inside() {
        assert!(geom_containment_valid(GeomContainment {
            point_admitted: true,
            bounds_admitted: true,
            inside: true
        }));
        assert!(!geom_containment_valid(GeomContainment {
            point_admitted: true,
            bounds_admitted: true,
            inside: false
        }));
    }
}
