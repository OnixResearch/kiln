//! Exact finite root/scope containment.
use vstd::prelude::*;
verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct FsPathContainment { pub root_len: u8, pub path_len: u8, pub prefix_matches: bool }
// r[depends domain-primitives.filesystem-path.containment]
pub open spec fn fs_path_contained_spec(c: FsPathContainment) -> bool { c.root_len <= c.path_len && c.prefix_matches }
// r[impl domain-primitives.filesystem-path.containment]
pub fn fs_path_contained(c: FsPathContainment) -> (result: bool) ensures result == fs_path_contained_spec(c) { c.root_len <= c.path_len && c.prefix_matches }
// r[verify domain-primitives.filesystem-path.containment]
pub proof fn contained_has_prefix_len(c: FsPathContainment) requires fs_path_contained_spec(c) ensures c.root_len <= c.path_len {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn containment_requires_prefix() {
        assert!(fs_path_contained(FsPathContainment {
            root_len: 1,
            path_len: 2,
            prefix_matches: true
        }));
        assert!(!fs_path_contained(FsPathContainment {
            root_len: 1,
            path_len: 2,
            prefix_matches: false
        }));
    }
}
