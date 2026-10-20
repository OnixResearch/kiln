//! Exact finite filesystem-path admission composition.
use vstd::prelude::*;
verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct FsPathAdmission { pub segment: u8, pub form: u8, pub normalization: u8, pub containment: u8, pub traversal: u8, pub extension_type: u8, pub join_split: u8 }
pub open spec fn ok_flag_spec(v: u8) -> bool { v == 1 }
// r[depends domain-primitives.filesystem-path.admission]
pub open spec fn fs_path_admission_spec(a: FsPathAdmission) -> bool { ok_flag_spec(a.segment) && ok_flag_spec(a.form) && ok_flag_spec(a.normalization) && ok_flag_spec(a.containment) && ok_flag_spec(a.traversal) && ok_flag_spec(a.extension_type) && ok_flag_spec(a.join_split) }
// r[impl domain-primitives.filesystem-path.admission]
pub fn fs_path_admitted(a: FsPathAdmission) -> (result: bool) ensures result == fs_path_admission_spec(a) { a.segment == 1 && a.form == 1 && a.normalization == 1 && a.containment == 1 && a.traversal == 1 && a.extension_type == 1 && a.join_split == 1 }
// r[verify domain-primitives.filesystem-path.admission]
pub proof fn admitted_path_has_segment(a: FsPathAdmission) requires fs_path_admission_spec(a) ensures a.segment == 1 {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn admission_requires_all_checks() {
        let a = FsPathAdmission {
            segment: 1,
            form: 1,
            normalization: 1,
            containment: 1,
            traversal: 1,
            extension_type: 1,
            join_split: 1,
        };
        assert!(fs_path_admitted(a));
        let bad = FsPathAdmission { traversal: 0, ..a };
        assert!(!fs_path_admitted(bad));
    }
}
