//! Exact finite key/value map primitives.
use vstd::prelude::*;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct DsMap3 { pub len: u8, pub k0: u8, pub v0: u16, pub k1: u8, pub v1: u16, pub k2: u8, pub v2: u16 }
// r[depends domain-primitives.ds-key-value-map]
pub open spec fn ds_map_keys_unique_spec(m: DsMap3) -> bool { m.len <= 3 && (m.len <= 1 || m.k0 != m.k1) && (m.len <= 2 || (m.k0 != m.k2 && m.k1 != m.k2)) }
// r[depends domain-primitives.ds-key-value-map]
pub open spec fn ds_map_lookup_spec(m: DsMap3, k: u8) -> Option<u16> { if !ds_map_keys_unique_spec(m) { None } else if m.len > 0 && k == m.k0 { Some(m.v0) } else if m.len > 1 && k == m.k1 { Some(m.v1) } else if m.len > 2 && k == m.k2 { Some(m.v2) } else { None } }
// r[depends domain-primitives.ds-key-value-map]
pub open spec fn ds_map_insert_spec(m: DsMap3, k: u8, v: u16) -> DsMap3 { if !ds_map_keys_unique_spec(m) { m } else if m.len > 0 && k == m.k0 { DsMap3{v0:v,..m} } else if m.len > 1 && k == m.k1 { DsMap3{v1:v,..m} } else if m.len > 2 && k == m.k2 { DsMap3{v2:v,..m} } else if m.len == 0 { DsMap3{len:1,k0:k,v0:v,..m} } else if m.len == 1 { DsMap3{len:2,k1:k,v1:v,..m} } else if m.len == 2 { DsMap3{len:3,k2:k,v2:v,..m} } else { m } }
// r[depends domain-primitives.ds-key-value-map]
pub open spec fn ds_map_remove_spec(m: DsMap3, k: u8) -> DsMap3 { if !ds_map_keys_unique_spec(m) { m } else if m.len > 0 && k == m.k0 { if m.len == 1 { DsMap3{len:0,..m} } else if m.len == 2 { DsMap3{len:1,k0:m.k1,v0:m.v1,..m} } else { DsMap3{len:2,k0:m.k1,v0:m.v1,k1:m.k2,v1:m.v2,..m} } } else if m.len > 1 && k == m.k1 { if m.len == 2 { DsMap3{len:1,..m} } else { DsMap3{len:2,k1:m.k2,v1:m.v2,..m} } } else if m.len > 2 && k == m.k2 { DsMap3{len:2,..m} } else { m } }
// r[impl domain-primitives.ds-key-value-map]
pub fn ds_map_valid(m: DsMap3) -> (result: bool) ensures result == ds_map_keys_unique_spec(m) { m.len <= 3 && (m.len <= 1 || m.k0 != m.k1) && (m.len <= 2 || (m.k0 != m.k2 && m.k1 != m.k2)) }
// r[impl domain-primitives.ds-key-value-map]
pub fn ds_map_lookup(m: DsMap3, k: u8) -> (result: Option<u16>) ensures result == ds_map_lookup_spec(m,k) { if !ds_map_valid(m) { None } else if m.len > 0 && k == m.k0 { Some(m.v0) } else if m.len > 1 && k == m.k1 { Some(m.v1) } else if m.len > 2 && k == m.k2 { Some(m.v2) } else { None } }
// r[impl domain-primitives.ds-key-value-map]
pub fn ds_map_insert(m: DsMap3, k: u8, v: u16) -> (result: DsMap3) ensures result == ds_map_insert_spec(m,k,v) { if !ds_map_valid(m) { m } else if m.len > 0 && k == m.k0 { DsMap3{v0:v,..m} } else if m.len > 1 && k == m.k1 { DsMap3{v1:v,..m} } else if m.len > 2 && k == m.k2 { DsMap3{v2:v,..m} } else if m.len == 0 { DsMap3{len:1,k0:k,v0:v,..m} } else if m.len == 1 { DsMap3{len:2,k1:k,v1:v,..m} } else if m.len == 2 { DsMap3{len:3,k2:k,v2:v,..m} } else { m } }
// r[impl domain-primitives.ds-key-value-map]
pub fn ds_map_remove(m: DsMap3, k: u8) -> (result: DsMap3) ensures result == ds_map_remove_spec(m,k) { if !ds_map_valid(m) { m } else if m.len > 0 && k == m.k0 { if m.len == 1 { DsMap3{len:0,..m} } else if m.len == 2 { DsMap3{len:1,k0:m.k1,v0:m.v1,..m} } else { DsMap3{len:2,k0:m.k1,v0:m.v1,k1:m.k2,v1:m.v2,..m} } } else if m.len > 1 && k == m.k1 { if m.len == 2 { DsMap3{len:1,..m} } else { DsMap3{len:2,k1:m.k2,v1:m.v2,..m} } } else if m.len > 2 && k == m.k2 { DsMap3{len:2,..m} } else { m } }
// r[verify domain-primitives.ds-key-value-map]
pub proof fn ds_map_insert_target_lookup(m: DsMap3, k: u8, v: u16) requires ds_map_keys_unique_spec(m) && (ds_map_lookup_spec(m,k).is_some() || m.len < 3) ensures ds_map_lookup_spec(ds_map_insert_spec(m,k,v),k) == Some(v) {}
// r[verify domain-primitives.ds-key-value-map]
pub proof fn ds_map_remove_clears(m: DsMap3, k: u8) requires ds_map_keys_unique_spec(m) ensures ds_map_lookup_spec(ds_map_remove_spec(m,k),k).is_none() {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn insert_remove_lookup() {
        let m = DsMap3 {
            len: 1,
            k0: 4,
            v0: 10,
            k1: 0,
            v1: 0,
            k2: 0,
            v2: 0,
        };
        let m2 = ds_map_insert(m, 5, 11);
        assert_eq!(ds_map_lookup(m2, 5), Some(11));
        assert_eq!(ds_map_lookup(ds_map_remove(m2, 4), 4), None);
    }
}
