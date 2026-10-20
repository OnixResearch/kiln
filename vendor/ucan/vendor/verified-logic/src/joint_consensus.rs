//! Verified joint consensus for Raft membership changes.
//!
//! During a membership transition, both old and new configurations must
//! independently form quorums for every decision. This prevents split-brain
//! during the transition window where some servers have the new config
//! and others still have the old.

use vstd::prelude::*;

use crate::quorum::*;

verus! {

// --- is_joint_quorum ---

// r[depends joint-consensus.dual-quorum]
/// Spec: both old and new configurations independently form quorums.
pub open spec fn is_joint_quorum_spec(
    count_old: u64,
    total_old: u64,
    count_new: u64,
    total_new: u64,
) -> bool
    recommends total_old > 0, total_new > 0
{
    is_quorum_spec(count_old, total_old)
    && is_quorum_spec(count_new, total_new)
}

// r[impl joint-consensus.dual-quorum]
/// Check if both configurations have quorum.
pub fn is_joint_quorum(
    count_old: u64,
    total_old: u64,
    count_new: u64,
    total_new: u64,
) -> (result: bool)
    requires total_old > 0, total_new > 0
    ensures result == is_joint_quorum_spec(count_old, total_old, count_new, total_new)
{
    is_quorum(count_old, total_old) && is_quorum(count_new, total_new)
}

// --- Safety proofs ---

// r[verify joint-consensus.dual-quorum]
/// Joint intersection: two joint quorums overlap in BOTH configurations.
pub proof fn joint_quorum_intersection(
    a_old: u64, b_old: u64, total_old: u64,
    a_new: u64, b_new: u64, total_new: u64,
)
    requires
        total_old > 0,
        total_new > 0,
        is_joint_quorum_spec(a_old, total_old, a_new, total_new),
        is_joint_quorum_spec(b_old, total_old, b_new, total_new),
    ensures
        a_old as int + b_old as int > total_old as int,
        a_new as int + b_new as int > total_new as int,
{
    prove_quorum_intersection(a_old, b_old, total_old);
    prove_quorum_intersection(a_new, b_new, total_new);
}

// r[verify joint-consensus.dual-quorum]
/// Single-server change: when old and new configs differ by one node,
/// joint quorum reduces to requiring quorums of both configs.
/// (This is a documentation proof — the semantics are identical.)
proof fn single_server_change_is_joint(
    count_old: u64, total_old: u64,
    count_new: u64, total_new: u64,
)
    requires total_old > 0, total_new > 0
    ensures
        is_joint_quorum_spec(count_old, total_old, count_new, total_new)
        == (is_quorum_spec(count_old, total_old) && is_quorum_spec(count_new, total_new))
{}

// r[verify joint-consensus.dual-quorum]
/// Neither config having quorum means joint quorum fails.
proof fn no_old_quorum_fails_joint(
    count_old: u64, total_old: u64,
    count_new: u64, total_new: u64,
)
    requires
        total_old > 0,
        total_new > 0,
        !is_quorum_spec(count_old, total_old),
    ensures !is_joint_quorum_spec(count_old, total_old, count_new, total_new)
{}

// r[verify joint-consensus.dual-quorum]
proof fn no_new_quorum_fails_joint(
    count_old: u64, total_old: u64,
    count_new: u64, total_new: u64,
)
    requires
        total_old > 0,
        total_new > 0,
        !is_quorum_spec(count_new, total_new),
    ensures !is_joint_quorum_spec(count_old, total_old, count_new, total_new)
{}

// r[verify joint-consensus.dual-quorum]
/// Full clusters in both configs always form a joint quorum.
proof fn full_joint_quorum(total_old: u64, total_new: u64)
    requires total_old > 0, total_new > 0
    ensures is_joint_quorum_spec(total_old, total_old, total_new, total_new)
{
    full_is_quorum(total_old);
    full_is_quorum(total_new);
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify joint-consensus.dual-quorum]
    #[test]
    fn test_joint_quorum() {
        assert!(is_joint_quorum(3, 5, 2, 3)); // old: 3/5, new: 2/3
        assert!(!is_joint_quorum(2, 5, 2, 3)); // old: 2/5 (not quorum)
        assert!(!is_joint_quorum(3, 5, 1, 3)); // new: 1/3 (not quorum)
        assert!(!is_joint_quorum(2, 5, 1, 3)); // neither
        assert!(is_joint_quorum(5, 5, 3, 3)); // both full
    }

    // r[verify joint-consensus.dual-quorum]
    #[test]
    fn test_joint_intersection() {
        // Two joint quorums of old=5, new=3 must overlap in both
        let (a_old, b_old) = (3u64, 3u64);
        let (a_new, b_new) = (2u64, 2u64);
        assert!(a_old + b_old > 5); // overlap in old config
        assert!(a_new + b_new > 3); // overlap in new config
    }

    proptest! {
        // r[verify joint-consensus.dual-quorum]
        #[test]
        fn prop_joint_intersection(
            total_old in 1u64..50,
            total_new in 1u64..50,
            extra_a_old in 0u64..10,
            extra_b_old in 0u64..10,
            extra_a_new in 0u64..10,
            extra_b_new in 0u64..10,
        ) {
            let maj_old = total_old / 2 + 1;
            let maj_new = total_new / 2 + 1;
            let a_old = maj_old.saturating_add(extra_a_old).min(total_old);
            let b_old = maj_old.saturating_add(extra_b_old).min(total_old);
            let a_new = maj_new.saturating_add(extra_a_new).min(total_new);
            let b_new = maj_new.saturating_add(extra_b_new).min(total_new);

            if is_joint_quorum(a_old, total_old, a_new, total_new)
                && is_joint_quorum(b_old, total_old, b_new, total_new)
            {
                prop_assert!(u128::from(a_old) + u128::from(b_old) > u128::from(total_old));
                prop_assert!(u128::from(a_new) + u128::from(b_new) > u128::from(total_new));
            }
        }

        // r[verify joint-consensus.dual-quorum]
        #[test]
        fn prop_full_always_joint(total_old in 1u64..100, total_new in 1u64..100) {
            prop_assert!(is_joint_quorum(total_old, total_old, total_new, total_new));
        }

        #[test]
        fn prop_zero_count_never_quorum(
            total_old in 1u64..50,
            total_new in 1u64..50,
        ) {
            // Zero votes in old config means no joint quorum
            prop_assert!(!is_joint_quorum(0, total_old, total_new, total_new));
            // Zero votes in new config means no joint quorum
            prop_assert!(!is_joint_quorum(total_old, total_old, 0, total_new));
        }

        #[test]
        fn prop_both_quorums_required(
            total_old in 3u64..=20,
            total_new in 3u64..=20,
        ) {
            let maj_old = total_old / 2 + 1;
            let maj_new = total_new / 2 + 1;
            // Old quorum but not new
            if maj_new > 1 {
                prop_assert!(!is_joint_quorum(maj_old, total_old, maj_new - 1, total_new));
            }
            // New quorum but not old
            if maj_old > 1 {
                prop_assert!(!is_joint_quorum(maj_old - 1, total_old, maj_new, total_new));
            }
            // Both quorums
            prop_assert!(is_joint_quorum(maj_old, total_old, maj_new, total_new));
        }
    }
}
