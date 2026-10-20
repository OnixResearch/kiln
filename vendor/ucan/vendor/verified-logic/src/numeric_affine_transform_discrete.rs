//! Exact discrete affine transform envelope primitives.
use vstd::prelude::*;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)] pub struct NumericAffine1 { pub coeff:i16, pub bias:i16, pub input_min:i16, pub input_max:i16, pub output_min:i32, pub output_max:i32 }
// r[depends domain-primitives.numeric-affine-transform-discrete]
pub open spec fn numeric_affine_value_spec(a:NumericAffine1, x:i16)->int { (a.coeff as int)*(x as int) + a.bias as int }
// r[depends domain-primitives.numeric-affine-transform-discrete]
pub open spec fn numeric_affine_valid_spec(a:NumericAffine1)->bool { a.input_min<=a.input_max && a.output_min<=a.output_max && a.output_min as int <= numeric_affine_value_spec(a,a.input_min) && numeric_affine_value_spec(a,a.input_min)<=a.output_max as int && a.output_min as int <= numeric_affine_value_spec(a,a.input_max) && numeric_affine_value_spec(a,a.input_max)<=a.output_max as int }
// r[impl domain-primitives.numeric-affine-transform-discrete]
pub fn numeric_affine_valid(a:NumericAffine1)->(result:bool) ensures result==numeric_affine_valid_spec(a){ assert((a.coeff as int)*(a.input_min as int) <= 32768int*32768int) by(nonlinear_arith); assert(-(32768int*32768int) <= (a.coeff as int)*(a.input_min as int)) by(nonlinear_arith); assert((a.coeff as int)*(a.input_max as int) <= 32768int*32768int) by(nonlinear_arith); assert(-(32768int*32768int) <= (a.coeff as int)*(a.input_max as int)) by(nonlinear_arith); assert(-(32768int*32768int) + -32768int >= i32::MIN as int) by(nonlinear_arith); assert(32768int*32768int + 32767int <= i32::MAX as int) by(nonlinear_arith); let lo=(a.coeff as i32)*(a.input_min as i32)+(a.bias as i32); let hi=(a.coeff as i32)*(a.input_max as i32)+(a.bias as i32); a.input_min<=a.input_max && a.output_min<=a.output_max && a.output_min<=lo && lo<=a.output_max && a.output_min<=hi && hi<=a.output_max }
// r[verify domain-primitives.numeric-affine-transform-discrete]
pub proof fn numeric_affine_valid_has_order(a:NumericAffine1) requires numeric_affine_valid_spec(a) ensures a.input_min<=a.input_max && a.output_min<=a.output_max {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn affine_checks_endpoint_envelope() {
        let a = NumericAffine1 {
            coeff: 2,
            bias: 1,
            input_min: 0,
            input_max: 5,
            output_min: 0,
            output_max: 11,
        };
        assert!(numeric_affine_valid(a));
        assert!(!numeric_affine_valid(NumericAffine1 { output_max: 10, ..a }));
    }
}
