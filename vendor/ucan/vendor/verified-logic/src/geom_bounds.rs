//! Exact finite axis-aligned bounds envelope.
use vstd::prelude::*;
verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GeomBounds { pub low_x: i16, pub high_x: i16, pub low_y: i16, pub high_y: i16 }
// r[depends domain-primitives.geometry-foundation.bounds]
pub open spec fn geom_bounds_valid_spec(b: GeomBounds) -> bool { b.low_x <= b.high_x && b.low_y <= b.high_y }
// r[impl domain-primitives.geometry-foundation.bounds]
pub fn geom_bounds_valid(b: GeomBounds) -> (result: bool) ensures result == geom_bounds_valid_spec(b) { b.low_x <= b.high_x && b.low_y <= b.high_y }
// r[verify domain-primitives.geometry-foundation.bounds]
pub proof fn valid_bounds_are_not_inverted(b: GeomBounds) requires geom_bounds_valid_spec(b) ensures b.low_x <= b.high_x, b.low_y <= b.high_y {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounds_reject_inverted() {
        assert!(geom_bounds_valid(GeomBounds {
            low_x: 0,
            high_x: 2,
            low_y: -1,
            high_y: 1
        }));
        assert!(!geom_bounds_valid(GeomBounds {
            low_x: 3,
            high_x: 2,
            low_y: 0,
            high_y: 1
        }));
    }
}
