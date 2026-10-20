//! Exact finite prefix-sum primitives.
use vstd::prelude::*;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)] pub struct NumericPrefix3 { pub len:u8, pub p0:u16, pub p1:u16, pub p2:u16, pub p3:u16 }
// r[depends domain-primitives.numeric-prefix-sum]
pub open spec fn numeric_prefix3_valid_spec(p:NumericPrefix3)->bool { p.len<=3 && p.p0==0 && (p.len<1 || p.p0<=p.p1) && (p.len<2 || p.p1<=p.p2) && (p.len<3 || p.p2<=p.p3) }
// r[depends domain-primitives.numeric-prefix-sum]
pub open spec fn numeric_prefix3_range_spec(p:NumericPrefix3, start:u8, end:u8)->Option<nat> { if numeric_prefix3_valid_spec(p) && start<=end && end<=p.len { if start==0 && end==0 {Some(0)} else if start==0 && end==1 {Some((p.p1-p.p0) as nat)} else if start==1 && end==2 {Some((p.p2-p.p1) as nat)} else if start==2 && end==3 {Some((p.p3-p.p2) as nat)} else {Some(0)} } else { None } }
// r[impl domain-primitives.numeric-prefix-sum]
pub fn numeric_prefix3_valid(p:NumericPrefix3)->(result:bool) ensures result==numeric_prefix3_valid_spec(p){ p.len<=3 && p.p0==0 && (p.len<1 || p.p0<=p.p1) && (p.len<2 || p.p1<=p.p2) && (p.len<3 || p.p2<=p.p3) }
// r[impl domain-primitives.numeric-prefix-sum]
pub fn numeric_prefix3_range_some(p:NumericPrefix3, start:u8, end:u8)->(result:bool) ensures result == numeric_prefix3_range_spec(p,start,end).is_some(){ numeric_prefix3_valid(p) && start<=end && end<=p.len }
// r[verify domain-primitives.numeric-prefix-sum]
pub proof fn numeric_prefix3_range_requires_valid(p:NumericPrefix3,start:u8,end:u8) requires numeric_prefix3_range_spec(p,start,end).is_some() ensures numeric_prefix3_valid_spec(p) {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prefix3_admits_ranges() {
        let p = NumericPrefix3 {
            len: 3,
            p0: 0,
            p1: 2,
            p2: 5,
            p3: 9,
        };
        assert!(numeric_prefix3_valid(p));
        assert!(numeric_prefix3_range_some(p, 1, 2));
        assert!(!numeric_prefix3_valid(NumericPrefix3 { p2: 1, ..p }));
    }
}
