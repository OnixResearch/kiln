//! Exact finite intersection/touch metadata.
use vstd::prelude::*;
verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GeomIntersection { pub left_admitted: bool, pub right_admitted: bool, pub relation: u8 }
pub open spec fn GEOM_REL_DISJOINT() -> u8 { 0 }
pub open spec fn GEOM_REL_TOUCHES() -> u8 { 1 }
pub open spec fn GEOM_REL_OVERLAPS() -> u8 { 2 }
// r[depends domain-primitives.geometry-foundation.intersection]
pub open spec fn geom_intersection_valid_spec(i: GeomIntersection) -> bool { i.left_admitted && i.right_admitted && i.relation <= GEOM_REL_OVERLAPS() }
// r[impl domain-primitives.geometry-foundation.intersection]
pub fn geom_intersection_valid(i: GeomIntersection) -> (result: bool) ensures result == geom_intersection_valid_spec(i) { i.left_admitted && i.right_admitted && i.relation <= 2 }
// r[verify domain-primitives.geometry-foundation.intersection]
pub proof fn valid_intersection_relation_bounded(i: GeomIntersection) requires geom_intersection_valid_spec(i) ensures i.relation <= GEOM_REL_OVERLAPS() {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn intersection_classifies_disjoint() {
        assert!(geom_intersection_valid(GeomIntersection {
            left_admitted: true,
            right_admitted: true,
            relation: 0
        }));
        assert!(!geom_intersection_valid(GeomIntersection {
            left_admitted: true,
            right_admitted: true,
            relation: 9
        }));
    }
}
