//! Exact finite path-normalization decisions.
use vstd::prelude::*;
verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct FsPathNormalization { pub admitted: bool, pub rewritten: bool, pub original_len: u8, pub output_len: u8 }
// r[depends domain-primitives.filesystem-path.normalization]
pub open spec fn fs_path_normalization_valid_spec(n: FsPathNormalization) -> bool { if n.admitted { n.output_len > 0 || n.original_len == 0 } else { n.output_len == n.original_len } }
// r[impl domain-primitives.filesystem-path.normalization]
pub fn fs_path_normalization_valid(n: FsPathNormalization) -> (result: bool) ensures result == fs_path_normalization_valid_spec(n) { if n.admitted { n.output_len > 0 || n.original_len == 0 } else { n.output_len == n.original_len } }
// r[verify domain-primitives.filesystem-path.normalization]
pub proof fn rejected_normalization_preserves_len(n: FsPathNormalization) requires fs_path_normalization_valid_spec(n), !n.admitted ensures n.output_len == n.original_len {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalization_rejection_preserves_len() {
        assert!(fs_path_normalization_valid(FsPathNormalization {
            admitted: false,
            rewritten: false,
            original_len: 2,
            output_len: 2
        }));
        assert!(!fs_path_normalization_valid(FsPathNormalization {
            admitted: false,
            rewritten: true,
            original_len: 2,
            output_len: 1
        }));
    }
}
