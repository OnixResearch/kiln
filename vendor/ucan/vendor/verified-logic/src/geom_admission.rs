//! Exact finite geometry admission composition.
use vstd::prelude::*;
verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GeomAdmission { pub point_vector: u8, pub bounds: u8, pub orientation: u8, pub containment: u8, pub intersection: u8, pub transform: u8, pub grid_cell: u8 }
pub open spec fn geom_ok_flag_spec(v: u8) -> bool { v == 1 }
// r[depends domain-primitives.geometry-foundation.admission]
pub open spec fn geom_admission_spec(a: GeomAdmission) -> bool { geom_ok_flag_spec(a.point_vector) && geom_ok_flag_spec(a.bounds) && geom_ok_flag_spec(a.orientation) && geom_ok_flag_spec(a.containment) && geom_ok_flag_spec(a.intersection) && geom_ok_flag_spec(a.transform) && geom_ok_flag_spec(a.grid_cell) }
// r[impl domain-primitives.geometry-foundation.admission]
pub fn geom_admitted(a: GeomAdmission) -> (result: bool) ensures result == geom_admission_spec(a) { a.point_vector == 1 && a.bounds == 1 && a.orientation == 1 && a.containment == 1 && a.intersection == 1 && a.transform == 1 && a.grid_cell == 1 }
// r[verify domain-primitives.geometry-foundation.admission]
pub proof fn admitted_geometry_has_bounds(a: GeomAdmission) requires geom_admission_spec(a) ensures a.bounds == 1 {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn admission_requires_all_checks() {
        let a = GeomAdmission {
            point_vector: 1,
            bounds: 1,
            orientation: 1,
            containment: 1,
            intersection: 1,
            transform: 1,
            grid_cell: 1,
        };
        assert!(geom_admitted(a));
        let bad = GeomAdmission { bounds: 0, ..a };
        assert!(!geom_admitted(bad));
    }
}
