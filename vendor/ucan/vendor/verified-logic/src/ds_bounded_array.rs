//! Exact finite bounded-array primitives.
use vstd::prelude::*;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct DsArray3 { pub len: u8, pub cap: u8, pub v0: u16, pub v1: u16, pub v2: u16 }

// r[depends domain-primitives.ds-bounded-array]
pub open spec fn ds_array_valid_spec(a: DsArray3) -> bool { a.cap <= 3 && a.len <= a.cap }
// r[depends domain-primitives.ds-bounded-array]
pub open spec fn ds_array_get_spec(a: DsArray3, i: u8) -> Option<u16> { if ds_array_valid_spec(a) && i < a.len { if i == 0 { Some(a.v0) } else if i == 1 { Some(a.v1) } else { Some(a.v2) } } else { None } }
// r[depends domain-primitives.ds-bounded-array]
pub open spec fn ds_array_set_spec(a: DsArray3, i: u8, v: u16) -> DsArray3 { if ds_array_valid_spec(a) && i < a.len { if i == 0 { DsArray3 { v0: v, ..a } } else if i == 1 { DsArray3 { v1: v, ..a } } else { DsArray3 { v2: v, ..a } } } else { a } }
// r[impl domain-primitives.ds-bounded-array]
pub fn ds_array_valid(a: DsArray3) -> (result: bool) ensures result == ds_array_valid_spec(a) { a.cap <= 3 && a.len <= a.cap }
// r[impl domain-primitives.ds-bounded-array]
pub fn ds_array_get(a: DsArray3, i: u8) -> (result: Option<u16>) ensures result == ds_array_get_spec(a,i) { if ds_array_valid(a) && i < a.len { if i == 0 { Some(a.v0) } else if i == 1 { Some(a.v1) } else { Some(a.v2) } } else { None } }
// r[impl domain-primitives.ds-bounded-array]
pub fn ds_array_set(a: DsArray3, i: u8, v: u16) -> (result: DsArray3) ensures result == ds_array_set_spec(a,i,v) { if ds_array_valid(a) && i < a.len { if i == 0 { DsArray3 { v0: v, ..a } } else if i == 1 { DsArray3 { v1: v, ..a } } else { DsArray3 { v2: v, ..a } } } else { a } }
// r[verify domain-primitives.ds-bounded-array]
pub proof fn ds_array_reject_unchanged(a: DsArray3, i: u8, v: u16) requires !(ds_array_valid_spec(a) && i < a.len) ensures ds_array_set_spec(a,i,v) == a {}
// r[verify domain-primitives.ds-bounded-array]
pub proof fn ds_array_set_preserves_len(a: DsArray3, i: u8, v: u16) ensures ds_array_set_spec(a,i,v).len == a.len && ds_array_set_spec(a,i,v).cap == a.cap {}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn update_and_reject() {
        let a = DsArray3 {
            len: 2,
            cap: 3,
            v0: 1,
            v1: 2,
            v2: 3,
        };
        assert_eq!(ds_array_get(a, 1), Some(2));
        assert_eq!(ds_array_set(a, 1, 9).v1, 9);
        assert_eq!(ds_array_set(a, 3, 9), a);
    }
}
