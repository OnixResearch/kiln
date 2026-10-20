//! Exact finite monotone tuple primitives.
use vstd::prelude::*;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)] pub struct NumericSeq3 { pub len:u8, pub x0:i32, pub x1:i32, pub x2:i32 }
// r[depends domain-primitives.numeric-monotone-sequence]
pub open spec fn numeric_seq3_valid_spec(s: NumericSeq3)->bool { s.len<=3 && (s.len<2 || s.x0<=s.x1) && (s.len<3 || s.x1<=s.x2) }
// r[depends domain-primitives.numeric-monotone-sequence]
pub open spec fn numeric_seq3_lower_bound_spec(s: NumericSeq3, q:i32)->u8 { if s.len==0 || q<=s.x0 {0} else if s.len==1 || q<=s.x1 {1} else if s.len==2 || q<=s.x2 {2} else {3} }
// r[impl domain-primitives.numeric-monotone-sequence]
pub fn numeric_seq3_valid(s: NumericSeq3)->(result:bool) ensures result==numeric_seq3_valid_spec(s){ s.len<=3 && (s.len<2 || s.x0<=s.x1) && (s.len<3 || s.x1<=s.x2) }
// r[impl domain-primitives.numeric-monotone-sequence]
pub fn numeric_seq3_lower_bound(s: NumericSeq3, q:i32)->(result:u8) requires numeric_seq3_valid_spec(s) ensures result==numeric_seq3_lower_bound_spec(s,q), result <= s.len { if s.len==0 || q<=s.x0 {0} else if s.len==1 || q<=s.x1 {1} else if s.len==2 || q<=s.x2 {2} else {3} }
// r[verify domain-primitives.numeric-monotone-sequence]
pub proof fn numeric_seq3_lower_bound_in_range(s: NumericSeq3, q:i32) requires numeric_seq3_valid_spec(s) ensures numeric_seq3_lower_bound_spec(s,q) <= s.len {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seq3_lower_bound_is_bounded() {
        let s = NumericSeq3 {
            len: 3,
            x0: 1,
            x1: 3,
            x2: 5,
        };
        assert!(numeric_seq3_valid(s));
        assert_eq!(numeric_seq3_lower_bound(s, 4), 2);
        assert!(!numeric_seq3_valid(NumericSeq3 { x1: 0, ..s }));
    }
}
