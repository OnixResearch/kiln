//! Exact finite path join/split shape envelope.
use vstd::prelude::*;
verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct FsPathJoinSplit { pub left_len: u8, pub right_len: u8, pub result_len: u8, pub order_preserved: bool }
// r[depends domain-primitives.filesystem-path.join-split]
pub open spec fn fs_path_join_split_valid_spec(j: FsPathJoinSplit) -> bool { j.order_preserved && j.result_len == j.left_len + j.right_len && j.left_len + j.right_len <= u8::MAX }
// r[impl domain-primitives.filesystem-path.join-split]
pub fn fs_path_join_split_valid(j: FsPathJoinSplit) -> (result: bool) ensures result == fs_path_join_split_valid_spec(j) { j.order_preserved && j.left_len <= u8::MAX - j.right_len && j.result_len == j.left_len + j.right_len }
// r[verify domain-primitives.filesystem-path.join-split]
pub proof fn valid_join_preserves_order(j: FsPathJoinSplit) requires fs_path_join_split_valid_spec(j) ensures j.order_preserved {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn join_split_checks_shape() {
        assert!(fs_path_join_split_valid(FsPathJoinSplit {
            left_len: 1,
            right_len: 2,
            result_len: 3,
            order_preserved: true
        }));
        assert!(!fs_path_join_split_valid(FsPathJoinSplit {
            left_len: 1,
            right_len: 2,
            result_len: 4,
            order_preserved: true
        }));
    }
}
