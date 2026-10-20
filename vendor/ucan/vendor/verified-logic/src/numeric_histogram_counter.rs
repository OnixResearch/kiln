//! Exact bounded histogram-counter primitives.
use vstd::prelude::*;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)] pub struct NumericHistogram3 { pub bucket0_max:i32, pub bucket1_max:i32, pub c0:u16, pub c1:u16, pub c2:u16 }
// r[depends domain-primitives.numeric-histogram-counter]
pub open spec fn numeric_histogram3_valid_spec(h:NumericHistogram3)->bool { h.bucket0_max < h.bucket1_max }
// r[depends domain-primitives.numeric-histogram-counter]
pub open spec fn numeric_histogram3_bucket_spec(h:NumericHistogram3, v:i32)->u8 { if v<=h.bucket0_max {0} else if v<=h.bucket1_max {1} else {2} }

// r[impl domain-primitives.numeric-histogram-counter]
pub fn numeric_histogram3_bucket(h:NumericHistogram3, v:i32)->(result:u8) ensures result==numeric_histogram3_bucket_spec(h,v), result < 3 { if v<=h.bucket0_max {0} else if v<=h.bucket1_max {1} else {2} }

// r[impl domain-primitives.numeric-histogram-counter]
pub fn numeric_histogram3_valid(h:NumericHistogram3)->(result:bool) ensures result==numeric_histogram3_valid_spec(h){ h.bucket0_max < h.bucket1_max }
// r[impl domain-primitives.numeric-histogram-counter]
pub fn numeric_histogram3_can_increment(h:NumericHistogram3, bucket:u8)->(result:bool) ensures result == (bucket<3 && (bucket==0 ==> h.c0<u16::MAX) && (bucket==1 ==> h.c1<u16::MAX) && (bucket==2 ==> h.c2<u16::MAX)) { bucket<3 && (bucket!=0 || h.c0<u16::MAX) && (bucket!=1 || h.c1<u16::MAX) && (bucket!=2 || h.c2<u16::MAX) }
// r[verify domain-primitives.numeric-histogram-counter]
pub proof fn numeric_histogram_bucket_bounded(h:NumericHistogram3, v:i32) ensures numeric_histogram3_bucket_spec(h,v) < 3 {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn histogram_bucket_and_overflow() {
        let h = NumericHistogram3 {
            bucket0_max: 0,
            bucket1_max: 10,
            c0: 0,
            c1: 5,
            c2: u16::MAX,
        };
        assert!(numeric_histogram3_valid(h));
        assert!(!numeric_histogram3_can_increment(h, 2));
        assert_eq!(numeric_histogram3_bucket(h, 11), 2);
    }
}
