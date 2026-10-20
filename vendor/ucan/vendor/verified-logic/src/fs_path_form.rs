//! Exact finite path-form metadata.
use vstd::prelude::*;
verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct FsPathForm { pub segment_count: u8, pub absolute: bool, pub rooted: bool, pub allow_empty: bool }
// r[depends domain-primitives.filesystem-path.form]
pub open spec fn fs_path_form_admitted_spec(f: FsPathForm) -> bool { f.segment_count > 0 || f.allow_empty }
// r[impl domain-primitives.filesystem-path.form]
pub fn fs_path_form_admitted(f: FsPathForm) -> (result: bool) ensures result == fs_path_form_admitted_spec(f) { f.segment_count > 0 || f.allow_empty }
// r[verify domain-primitives.filesystem-path.form]
pub proof fn nonempty_form_admitted(f: FsPathForm) requires f.segment_count > 0 ensures fs_path_form_admitted_spec(f) {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn form_checks_empty_policy() {
        assert!(fs_path_form_admitted(FsPathForm {
            segment_count: 1,
            absolute: false,
            rooted: false,
            allow_empty: false
        }));
        assert!(!fs_path_form_admitted(FsPathForm {
            segment_count: 0,
            absolute: false,
            rooted: false,
            allow_empty: false
        }));
    }
}
