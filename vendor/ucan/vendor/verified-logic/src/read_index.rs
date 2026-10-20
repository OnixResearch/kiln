//! Verified read index protocol for Raft linearizable reads.
//!
//! A leader must confirm it still holds authority before serving a read.
//! It does this by collecting heartbeat acknowledgments from a quorum.
//! Without this check, a deposed leader (partitioned from the new leader)
//! could serve stale data.
//!
//! The read is safe when the commit index at read registration time has
//! been reached (all entries up to that point applied to the state machine).

use vstd::prelude::*;

use crate::quorum::*;

verus! {

// --- confirm_read_authority ---

// r[depends read-index.confirm-authority]
/// Spec: does the leader have quorum confirmation?
pub open spec fn confirm_read_authority_spec(
    ack_count: u64,
    cluster_size: u64,
) -> bool
    recommends cluster_size > 0
{
    is_quorum_spec(ack_count, cluster_size)
}

// r[impl read-index.confirm-authority]
/// Check if the leader has quorum authority to serve reads.
pub fn confirm_read_authority(
    ack_count: u64,
    cluster_size: u64,
) -> (result: bool)
    requires cluster_size > 0
    ensures result == confirm_read_authority_spec(ack_count, cluster_size)
{
    is_quorum(ack_count, cluster_size)
}

// --- read_safe_at ---

// r[depends read-index.safe-at]
/// Spec: is the read safe to serve?
///
/// A read registered when commit_index was `read_index` is safe when
/// the current commit_index >= read_index (all entries up to the read
/// point have been applied).
pub open spec fn read_safe_at_spec(
    read_index: u64,
    commit_index: u64,
) -> bool {
    commit_index >= read_index
}

// r[impl read-index.safe-at]
/// Check if a read is safe to serve.
pub fn read_safe_at(
    read_index: u64,
    commit_index: u64,
) -> (result: bool)
    ensures result == read_safe_at_spec(read_index, commit_index)
{
    commit_index >= read_index
}

// --- Safety proofs ---

// r[verify read-index.confirm-authority]
/// A deposed leader with only self-ack fails the quorum check.
proof fn deposed_leader_rejected(cluster_size: u64)
    requires cluster_size > 1
    ensures !confirm_read_authority_spec(1, cluster_size)
{
    // majority(n) >= 2 for n >= 2, so 1 < majority
}

// r[verify read-index.confirm-authority]
/// Full cluster always confirms.
proof fn full_cluster_confirms(cluster_size: u64)
    requires cluster_size > 0
    ensures confirm_read_authority_spec(cluster_size, cluster_size)
{
    full_is_quorum(cluster_size);
}

// r[verify read-index.safe-at]
/// Read at commit index 0 with any commit is safe.
proof fn zero_read_always_safe(commit_index: u64)
    ensures read_safe_at_spec(0, commit_index)
{}

// r[verify read-index.safe-at]
/// Read is safe at the exact commit point.
proof fn read_safe_at_exact(index: u64)
    ensures read_safe_at_spec(index, index)
{}

// r[verify read-index.safe-at]
/// Read safety is monotonic in commit_index.
proof fn read_safe_monotonic(read_index: u64, ci_old: u64, ci_new: u64)
    requires
        read_safe_at_spec(read_index, ci_old),
        ci_new >= ci_old,
    ensures read_safe_at_spec(read_index, ci_new)
{}

// r[verify read-index.safe-at]
/// Not-yet-committed read is unsafe.
proof fn uncommitted_read_unsafe(read_index: u64, commit_index: u64)
    requires commit_index < read_index
    ensures !read_safe_at_spec(read_index, commit_index)
{}

// r[verify read-index.confirm-authority]
/// Quorum check delegates to quorum module.
proof fn authority_is_quorum(ack_count: u64, cluster_size: u64)
    requires cluster_size > 0
    ensures confirm_read_authority_spec(ack_count, cluster_size)
        == is_quorum_spec(ack_count, cluster_size)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify read-index.confirm-authority]
    #[test]
    fn test_confirm_read_authority() {
        assert!(confirm_read_authority(3, 5)); // quorum
        assert!(!confirm_read_authority(2, 5)); // not quorum
        assert!(!confirm_read_authority(1, 5)); // deposed leader
        assert!(confirm_read_authority(1, 1)); // singleton
    }

    // r[verify read-index.safe-at]
    #[test]
    fn test_read_safe_at() {
        assert!(read_safe_at(5, 7)); // committed past read
        assert!(read_safe_at(5, 5)); // at exact point
        assert!(!read_safe_at(5, 3)); // not yet committed
        assert!(read_safe_at(0, 0)); // zero
    }

    proptest! {
        // r[verify read-index.confirm-authority]
        #[test]
        fn prop_deposed_leader_rejected(cluster_size in 2u64..100) {
            prop_assert!(!confirm_read_authority(1, cluster_size));
        }

        // r[verify read-index.safe-at]
        #[test]
        fn prop_read_safe_monotonic(
            read_index: u64,
            ci_old: u64,
            delta in 0u64..1000,
        ) {
            if read_safe_at(read_index, ci_old) {
                let ci_new = ci_old.saturating_add(delta);
                prop_assert!(read_safe_at(read_index, ci_new));
            }
        }

        // r[verify read-index.safe-at]
        #[test]
        fn prop_exact_commit_safe(index: u64) {
            prop_assert!(read_safe_at(index, index));
        }
    }
}
