//! Exact finite point/vector envelope admission.
use vstd::prelude::*;
verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GeomPointVector { pub coord_count: u8, pub declared_dim: u8, pub within_bounds: bool, pub scale_tag: u8 }
// r[depends domain-primitives.geometry-foundation.point-vector]
pub open spec fn geom_point_vector_admitted_spec(p: GeomPointVector) -> bool { p.coord_count == p.declared_dim && p.declared_dim > 0 && p.within_bounds }
// r[impl domain-primitives.geometry-foundation.point-vector]
pub fn geom_point_vector_admitted(p: GeomPointVector) -> (result: bool) ensures result == geom_point_vector_admitted_spec(p) { p.coord_count == p.declared_dim && p.declared_dim > 0 && p.within_bounds }
// r[verify domain-primitives.geometry-foundation.point-vector]
pub proof fn admitted_point_vector_has_dimension(p: GeomPointVector) requires geom_point_vector_admitted_spec(p) ensures p.coord_count == p.declared_dim, p.declared_dim > 0 {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn point_vector_rejects_dimension_mismatch() {
        assert!(geom_point_vector_admitted(GeomPointVector {
            coord_count: 2,
            declared_dim: 2,
            within_bounds: true,
            scale_tag: 1
        }));
        assert!(!geom_point_vector_admitted(GeomPointVector {
            coord_count: 3,
            declared_dim: 2,
            within_bounds: true,
            scale_tag: 1
        }));
    }
}
