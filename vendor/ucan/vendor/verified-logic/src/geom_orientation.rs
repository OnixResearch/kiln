//! Exact finite orientation metadata.
use vstd::prelude::*;
verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GeomOrientation { pub admitted: bool, pub orientation: u8 }
pub open spec fn GEOM_ORIENT_DEGENERATE() -> u8 { 0 }
pub open spec fn GEOM_ORIENT_CLOCKWISE() -> u8 { 1 }
pub open spec fn GEOM_ORIENT_COUNTERCLOCKWISE() -> u8 { 2 }
// r[depends domain-primitives.geometry-foundation.orientation]
pub open spec fn geom_orientation_valid_spec(o: GeomOrientation) -> bool { o.admitted && o.orientation <= GEOM_ORIENT_COUNTERCLOCKWISE() }
// r[impl domain-primitives.geometry-foundation.orientation]
pub fn geom_orientation_valid(o: GeomOrientation) -> (result: bool) ensures result == geom_orientation_valid_spec(o) { o.admitted && o.orientation <= 2 }
// r[verify domain-primitives.geometry-foundation.orientation]
pub proof fn valid_orientation_code_bounded(o: GeomOrientation) requires geom_orientation_valid_spec(o) ensures o.orientation <= GEOM_ORIENT_COUNTERCLOCKWISE() {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn orientation_bounds_code() {
        assert!(geom_orientation_valid(GeomOrientation {
            admitted: true,
            orientation: 0
        }));
        assert!(!geom_orientation_valid(GeomOrientation {
            admitted: true,
            orientation: 4
        }));
    }
}
