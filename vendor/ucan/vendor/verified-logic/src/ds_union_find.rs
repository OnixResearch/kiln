//! Exact finite union-find admission primitives.
use vstd::prelude::*;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct DsUnionFind3 { pub len: u8, pub p0: u8, pub p1: u8, pub p2: u8 }
// r[depends domain-primitives.ds-union-find]
pub open spec fn ds_uf_valid_spec(u: DsUnionFind3) -> bool { u.len <= 3 && (u.len == 0 || u.p0 < u.len) && (u.len <= 1 || u.p1 < u.len) && (u.len <= 2 || u.p2 < u.len) }
// r[depends domain-primitives.ds-union-find]
pub open spec fn ds_uf_parent_spec(u: DsUnionFind3, i: u8) -> Option<u8> { if !ds_uf_valid_spec(u) || i >= u.len { None } else if i == 0 { Some(u.p0) } else if i == 1 { Some(u.p1) } else { Some(u.p2) } }
// r[depends domain-primitives.ds-union-find]
pub open spec fn ds_uf_is_root_spec(u: DsUnionFind3, i: u8) -> bool { ds_uf_parent_spec(u,i) == Some(i) }
// r[depends domain-primitives.ds-union-find]
pub open spec fn ds_uf_find_one_spec(u: DsUnionFind3, i: u8) -> Option<u8> { if !ds_uf_valid_spec(u) || i >= u.len { None } else if ds_uf_is_root_spec(u,i) { Some(i) } else { ds_uf_parent_spec(u,i) } }
// r[depends domain-primitives.ds-union-find]
pub open spec fn ds_uf_set_parent_spec(u: DsUnionFind3, i: u8, p: u8) -> DsUnionFind3 { if !ds_uf_valid_spec(u) || i >= u.len || p >= u.len { u } else if i == 0 { DsUnionFind3{p0:p,..u} } else if i == 1 { DsUnionFind3{p1:p,..u} } else { DsUnionFind3{p2:p,..u} } }
// r[depends domain-primitives.ds-union-find]
pub open spec fn ds_uf_union_spec(u: DsUnionFind3, a: u8, b: u8) -> DsUnionFind3 { if ds_uf_find_one_spec(u,a).is_some() && ds_uf_find_one_spec(u,b).is_some() { ds_uf_set_parent_spec(u, ds_uf_find_one_spec(u,a).unwrap(), ds_uf_find_one_spec(u,b).unwrap()) } else { u } }
// r[impl domain-primitives.ds-union-find]
pub fn ds_uf_valid(u: DsUnionFind3) -> (result: bool) ensures result == ds_uf_valid_spec(u) { u.len <= 3 && (u.len == 0 || u.p0 < u.len) && (u.len <= 1 || u.p1 < u.len) && (u.len <= 2 || u.p2 < u.len) }
// r[impl domain-primitives.ds-union-find]
pub fn ds_uf_parent(u: DsUnionFind3, i: u8) -> (result: Option<u8>) ensures result == ds_uf_parent_spec(u,i) { if !ds_uf_valid(u) || i >= u.len { None } else if i == 0 { Some(u.p0) } else if i == 1 { Some(u.p1) } else { Some(u.p2) } }
// r[impl domain-primitives.ds-union-find]
pub fn ds_uf_find_one(u: DsUnionFind3, i: u8) -> (result: Option<u8>) ensures result == ds_uf_find_one_spec(u,i) { if !ds_uf_valid(u) || i >= u.len { None } else { let p=ds_uf_parent(u,i); if p == Some(i) { Some(i) } else { p } } }
// r[impl domain-primitives.ds-union-find]
pub fn ds_uf_set_parent(u: DsUnionFind3, i: u8, p: u8) -> (result: DsUnionFind3) ensures result == ds_uf_set_parent_spec(u,i,p) { if !ds_uf_valid(u) || i >= u.len || p >= u.len { u } else if i == 0 { DsUnionFind3{p0:p,..u} } else if i == 1 { DsUnionFind3{p1:p,..u} } else { DsUnionFind3{p2:p,..u} } }
// r[impl domain-primitives.ds-union-find]
pub fn ds_uf_union(u: DsUnionFind3, a: u8, b: u8) -> (result: DsUnionFind3) ensures result == ds_uf_union_spec(u,a,b) { let ra=ds_uf_find_one(u,a); let rb=ds_uf_find_one(u,b); match (ra, rb) { (Some(rav), Some(rbv)) => ds_uf_set_parent(u, rav, rbv), _ => u } }
// r[verify domain-primitives.ds-union-find]
pub proof fn ds_uf_invalid_union_unchanged(u: DsUnionFind3, a: u8, b: u8) requires ds_uf_find_one_spec(u,a).is_none() || ds_uf_find_one_spec(u,b).is_none() ensures ds_uf_union_spec(u,a,b) == u {}
// r[verify domain-primitives.ds-union-find]
pub proof fn ds_uf_parent_result_bounded(u: DsUnionFind3, i: u8, p: u8) requires ds_uf_parent_spec(u,i) == Some(p) ensures p < u.len {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn union_links_roots() {
        let u = DsUnionFind3 {
            len: 3,
            p0: 0,
            p1: 1,
            p2: 2,
        };
        let u = ds_uf_union(u, 0, 1);
        assert_eq!(ds_uf_parent(u, 0), Some(1));
        assert_eq!(ds_uf_union(u, 5, 1), u);
    }
}
