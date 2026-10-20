//! Exact finite extension/type metadata.
use vstd::prelude::*;
verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct FsPathExtensionType { pub final_segment_admitted: bool, pub extension_declared: bool, pub type_tag: u8 }
// r[depends domain-primitives.filesystem-path.extension-type]
pub open spec fn fs_path_extension_type_valid_spec(e: FsPathExtensionType) -> bool { e.final_segment_admitted && (!e.extension_declared || e.type_tag > 0) }
// r[impl domain-primitives.filesystem-path.extension-type]
pub fn fs_path_extension_type_valid(e: FsPathExtensionType) -> (result: bool) ensures result == fs_path_extension_type_valid_spec(e) { e.final_segment_admitted && (!e.extension_declared || e.type_tag > 0) }
// r[verify domain-primitives.filesystem-path.extension-type]
pub proof fn valid_extension_has_segment(e: FsPathExtensionType) requires fs_path_extension_type_valid_spec(e) ensures e.final_segment_admitted {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extension_requires_declared_type() {
        assert!(fs_path_extension_type_valid(FsPathExtensionType {
            final_segment_admitted: true,
            extension_declared: true,
            type_tag: 2
        }));
        assert!(!fs_path_extension_type_valid(FsPathExtensionType {
            final_segment_admitted: true,
            extension_declared: true,
            type_tag: 0
        }));
    }
}
