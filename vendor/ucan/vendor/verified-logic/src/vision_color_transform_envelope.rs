//! Exact bounded integer color-transform envelope primitives.
use vstd::prelude::*;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct VisionTransform1 { pub coeff:u8, pub bias:u16, pub input_min:u8, pub input_max:u8, pub output_min:u32, pub output_max:u32 }

// r[depends domain-primitives.vision-color-transform-envelope]
pub open spec fn vision_transform_value_spec(t: VisionTransform1, input: u8) -> nat { (t.coeff as nat) * (input as nat) + t.bias as nat }

// r[depends domain-primitives.vision-color-transform-envelope]
pub open spec fn vision_transform_envelope_spec(t:VisionTransform1)->bool{ t.input_min<=t.input_max && t.output_min<=t.output_max && t.output_min as nat <= vision_transform_value_spec(t, t.input_min) && vision_transform_value_spec(t, t.input_min) <= t.output_max as nat && t.output_min as nat <= vision_transform_value_spec(t, t.input_max) && vision_transform_value_spec(t, t.input_max) <= t.output_max as nat }

// r[impl domain-primitives.vision-color-transform-envelope]
pub fn vision_transform_envelope(t:VisionTransform1)->(result:bool) ensures result==vision_transform_envelope_spec(t){
    assert((t.coeff as int) * (t.input_min as int) <= 255int * 255int) by(nonlinear_arith);
    assert((t.coeff as int) * (t.input_max as int) <= 255int * 255int) by(nonlinear_arith);
    let lo = (t.coeff as u32) * (t.input_min as u32) + (t.bias as u32);
    let hi = (t.coeff as u32) * (t.input_max as u32) + (t.bias as u32);
    t.input_min<=t.input_max && t.output_min<=t.output_max && t.output_min <= lo && lo <= t.output_max && t.output_min <= hi && hi <= t.output_max
}

// r[verify domain-primitives.vision-color-transform-envelope]
pub proof fn vision_transform_envelope_has_order(t:VisionTransform1) requires vision_transform_envelope_spec(t) ensures t.input_min<=t.input_max && t.output_min<=t.output_max {}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transform_checks_endpoints() {
        let t = VisionTransform1 {
            coeff: 2,
            bias: 1,
            input_min: 0,
            input_max: 10,
            output_min: 0,
            output_max: 21,
        };
        assert!(vision_transform_envelope(t));
        assert!(!vision_transform_envelope(VisionTransform1 { output_max: 20, ..t }));
    }
}
