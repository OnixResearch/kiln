//! Exact quantized threshold primitives.
use vstd::prelude::*;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)] pub struct NumericThreshold3 { pub min:i32, pub t0:i32, pub t1:i32, pub max:i32 }
// r[depends domain-primitives.numeric-quantized-threshold]
pub open spec fn numeric_threshold3_valid_spec(t:NumericThreshold3)->bool { t.min<=t.t0 && t.t0<t.t1 && t.t1<=t.max }
// r[depends domain-primitives.numeric-quantized-threshold]
pub open spec fn numeric_threshold3_bucket_spec(t:NumericThreshold3, v:i32)->Option<u8> { if numeric_threshold3_valid_spec(t) && t.min<=v && v<=t.max { if v<=t.t0 {Some(0)} else if v<=t.t1 {Some(1)} else {Some(2)} } else {None} }
// r[impl domain-primitives.numeric-quantized-threshold]
pub fn numeric_threshold3_valid(t:NumericThreshold3)->(result:bool) ensures result==numeric_threshold3_valid_spec(t){ t.min<=t.t0 && t.t0<t.t1 && t.t1<=t.max }
// r[impl domain-primitives.numeric-quantized-threshold]
pub fn numeric_threshold3_bucket(t:NumericThreshold3, v:i32)->(result:Option<u8>) ensures result==numeric_threshold3_bucket_spec(t,v){ if numeric_threshold3_valid(t) && t.min<=v && v<=t.max { if v<=t.t0 {Some(0)} else if v<=t.t1 {Some(1)} else {Some(2)} } else {None} }
// r[verify domain-primitives.numeric-quantized-threshold]
pub proof fn numeric_threshold3_bucket_bounded(t:NumericThreshold3, v:i32) requires numeric_threshold3_bucket_spec(t,v).is_some() ensures numeric_threshold3_bucket_spec(t,v).unwrap()<3 {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thresholds_quantize_deterministically() {
        let t = NumericThreshold3 {
            min: 0,
            t0: 10,
            t1: 20,
            max: 30,
        };
        assert_eq!(numeric_threshold3_bucket(t, 11), Some(1));
        assert_eq!(numeric_threshold3_bucket(t, 31), None);
        assert!(!numeric_threshold3_valid(NumericThreshold3 { t1: 10, ..t }));
    }
}
