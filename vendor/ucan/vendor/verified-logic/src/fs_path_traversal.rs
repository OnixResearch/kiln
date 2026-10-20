//! Exact finite relative traversal rejection.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FsPathTraversal {
    pub has_parent: bool,
    pub has_current: bool,
    pub policy: u8,
}

pub open spec fn FS_PATH_TRAVERSAL_ALLOW_ALL() -> u8 { 0 }
pub open spec fn FS_PATH_TRAVERSAL_REJECT_PARENT() -> u8 { 1 }
pub open spec fn FS_PATH_TRAVERSAL_REJECT_CURRENT() -> u8 { 2 }
pub open spec fn FS_PATH_TRAVERSAL_REJECT_BOTH() -> u8 { 3 }

pub open spec fn rejects_parent_spec(policy: u8) -> bool {
    policy == FS_PATH_TRAVERSAL_REJECT_PARENT() || policy == FS_PATH_TRAVERSAL_REJECT_BOTH()
}

pub open spec fn rejects_current_spec(policy: u8) -> bool {
    policy == FS_PATH_TRAVERSAL_REJECT_CURRENT() || policy == FS_PATH_TRAVERSAL_REJECT_BOTH()
}

// r[depends domain-primitives.filesystem-path.traversal]
pub open spec fn fs_path_traversal_admitted_spec(t: FsPathTraversal) -> bool {
    !(rejects_parent_spec(t.policy) && t.has_parent) && !(rejects_current_spec(t.policy) && t.has_current)
}

// r[impl domain-primitives.filesystem-path.traversal]
pub fn fs_path_traversal_admitted(t: FsPathTraversal) -> (result: bool)
    ensures result == fs_path_traversal_admitted_spec(t)
{
    !((t.policy == 1 || t.policy == 3) && t.has_parent)
        && !((t.policy == 2 || t.policy == 3) && t.has_current)
}

// r[verify domain-primitives.filesystem-path.traversal]
pub proof fn parent_rejected_by_policy(t: FsPathTraversal)
    requires rejects_parent_spec(t.policy), t.has_parent
    ensures !fs_path_traversal_admitted_spec(t)
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traversal_rejects_parent() {
        assert!(!fs_path_traversal_admitted(FsPathTraversal {
            has_parent: true,
            has_current: false,
            policy: 1,
        }));
        assert!(fs_path_traversal_admitted(FsPathTraversal {
            has_parent: true,
            has_current: false,
            policy: 0,
        }));
    }
}
