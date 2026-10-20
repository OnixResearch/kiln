//! Exact finite path-segment admission.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FsPathSegment {
    pub len: u8,
    pub kind: u8,
    pub forbidden: bool,
}

pub open spec fn FS_PATH_SEG_EMPTY() -> u8 { 0 }
pub open spec fn FS_PATH_SEG_CURRENT() -> u8 { 1 }
pub open spec fn FS_PATH_SEG_PARENT() -> u8 { 2 }
pub open spec fn FS_PATH_SEG_NORMAL() -> u8 { 3 }

// r[depends domain-primitives.filesystem-path.segment]
pub open spec fn fs_path_segment_admitted_spec(s: FsPathSegment) -> bool {
    s.len > 0 && s.kind != FS_PATH_SEG_EMPTY() && !s.forbidden
}

// r[impl domain-primitives.filesystem-path.segment]
pub fn fs_path_segment_admitted(s: FsPathSegment) -> (result: bool)
    ensures result == fs_path_segment_admitted_spec(s)
{
    s.len > 0 && s.kind != 0 && !s.forbidden
}

// r[verify domain-primitives.filesystem-path.segment]
pub proof fn admitted_segment_not_empty(s: FsPathSegment)
    requires fs_path_segment_admitted_spec(s)
    ensures s.len > 0, s.kind != FS_PATH_SEG_EMPTY(), !s.forbidden
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_rejects_empty_forbidden() {
        assert!(fs_path_segment_admitted(FsPathSegment {
            len: 3,
            kind: 3,
            forbidden: false,
        }));
        assert!(!fs_path_segment_admitted(FsPathSegment {
            len: 0,
            kind: 0,
            forbidden: false,
        }));
        assert!(!fs_path_segment_admitted(FsPathSegment {
            len: 2,
            kind: 3,
            forbidden: true,
        }));
    }
}
