//! Exact finite transform-shape envelope.
use vstd::prelude::*;
verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GeomTransform { pub input_admitted: bool, pub output_admitted: bool, pub shape: u8, pub overflow: bool }
// r[depends domain-primitives.geometry-foundation.transform]
pub open spec fn geom_transform_valid_spec(t: GeomTransform) -> bool { t.input_admitted && t.output_admitted && t.shape <= 3 && !t.overflow }
// r[impl domain-primitives.geometry-foundation.transform]
pub fn geom_transform_valid(t: GeomTransform) -> (result: bool) ensures result == geom_transform_valid_spec(t) { t.input_admitted && t.output_admitted && t.shape <= 3 && !t.overflow }
// r[verify domain-primitives.geometry-foundation.transform]
pub proof fn valid_transform_has_no_overflow(t: GeomTransform) requires geom_transform_valid_spec(t) ensures !t.overflow {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transform_rejects_overflow() {
        assert!(geom_transform_valid(GeomTransform {
            input_admitted: true,
            output_admitted: true,
            shape: 1,
            overflow: false
        }));
        assert!(!geom_transform_valid(GeomTransform {
            input_admitted: true,
            output_admitted: true,
            shape: 1,
            overflow: true
        }));
    }
}
