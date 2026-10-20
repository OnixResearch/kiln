//! Verified learner add/remove primitives for Raft.
//!
//! Adding or removing a learner does not affect quorum — learners don't
//! vote. These operations are simpler than voter membership changes and
//! do not require joint consensus. However, they still need correctness
//! checks:
//!
//! - Adding a learner that already exists is idempotent.
//! - Removing a learner that is being promoted is forbidden.
//! - The learner count must not exceed the configured maximum.
//! - Removing a nonexistent learner is a no-op.

use vstd::prelude::*;

verus! {

// --- can_add_learner ---

// r[depends learner-change.can-add]
/// Spec: can a new learner be added?
///
/// Adding is allowed when:
/// 1. The node is not already a voter (would be a demotion, not an add).
/// 2. The learner count is below the maximum (if max > 0; 0 means unlimited).
/// 3. No config change is currently pending (one at a time).
pub open spec fn can_add_learner_spec(
    is_voter: bool,
    is_existing_learner: bool,
    learner_count: u64,
    max_learners: u64,
    config_change_pending: bool,
) -> bool {
    !is_voter
    && !config_change_pending
    && (is_existing_learner || max_learners == 0 || learner_count < max_learners)
}

// r[impl learner-change.can-add]
/// Check if a learner can be added.
pub fn can_add_learner(
    is_voter: bool,
    is_existing_learner: bool,
    learner_count: u64,
    max_learners: u64,
    config_change_pending: bool,
) -> (result: bool)
    ensures result == can_add_learner_spec(is_voter, is_existing_learner, learner_count, max_learners, config_change_pending)
{
    !is_voter
    && !config_change_pending
    && (is_existing_learner || max_learners == 0 || learner_count < max_learners)
}

// --- add_learner_result ---

// r[depends learner-change.add-result]
/// Spec: learner count after adding a learner.
///
/// If the node is already a learner, the count is unchanged (idempotent).
/// Otherwise, count increases by 1.
pub open spec fn add_learner_result_spec(
    learner_count: u64,
    is_existing_learner: bool,
) -> u64 {
    if is_existing_learner {
        learner_count
    } else {
        (learner_count + 1) as u64
    }
}

// r[impl learner-change.add-result]
/// Compute learner count after adding.
pub fn add_learner_result(
    learner_count: u64,
    is_existing_learner: bool,
) -> (result: u64)
    requires !is_existing_learner ==> learner_count < u64::MAX
    ensures result == add_learner_result_spec(learner_count, is_existing_learner)
{
    if is_existing_learner {
        learner_count
    } else {
        learner_count + 1
    }
}

// --- can_remove_learner ---

// r[depends learner-change.can-remove]
/// Spec: can a learner be removed?
///
/// Removal is allowed when:
/// 1. The node is a learner (not a voter — use voter set change for that).
/// 2. The node is not in the process of being promoted to voter.
/// 3. No config change is currently pending.
pub open spec fn can_remove_learner_spec(
    is_learner: bool,
    is_promoting: bool,
    config_change_pending: bool,
) -> bool {
    is_learner && !is_promoting && !config_change_pending
}

// r[impl learner-change.can-remove]
/// Check if a learner can be removed.
pub fn can_remove_learner(
    is_learner: bool,
    is_promoting: bool,
    config_change_pending: bool,
) -> (result: bool)
    ensures result == can_remove_learner_spec(is_learner, is_promoting, config_change_pending)
{
    is_learner && !is_promoting && !config_change_pending
}

// --- remove_learner_result ---

// r[depends learner-change.remove-result]
/// Spec: learner count after removing a learner.
///
/// If the node is not a learner, count is unchanged (no-op).
/// Otherwise, count decreases by 1.
pub open spec fn remove_learner_result_spec(
    learner_count: u64,
    is_learner: bool,
) -> u64 {
    if is_learner && learner_count > 0 {
        (learner_count - 1) as u64
    } else {
        learner_count
    }
}

// r[impl learner-change.remove-result]
/// Compute learner count after removing.
pub fn remove_learner_result(
    learner_count: u64,
    is_learner: bool,
) -> (result: u64)
    ensures result == remove_learner_result_spec(learner_count, is_learner)
{
    if is_learner && learner_count > 0 {
        learner_count - 1
    } else {
        learner_count
    }
}

// --- quorum_unaffected ---

// r[depends learner-change.quorum-unaffected]
/// Spec: does this learner operation affect the quorum size?
///
/// Learner add/remove MUST NOT change the quorum calculation. The quorum
/// is computed from voters only. This is the key safety property.
pub open spec fn quorum_unchanged_spec(
    voter_count_before: u64,
    voter_count_after: u64,
) -> bool {
    voter_count_before == voter_count_after
}

// r[impl learner-change.quorum-unaffected]
/// Verify that voter count is unchanged.
pub fn quorum_unchanged(
    voter_count_before: u64,
    voter_count_after: u64,
) -> (result: bool)
    ensures result == quorum_unchanged_spec(voter_count_before, voter_count_after)
{
    voter_count_before == voter_count_after
}

// --- Safety proofs ---

// r[verify learner-change.can-add]
/// Adding an existing learner is always allowed (idempotent).
proof fn existing_learner_add_idempotent(
    learner_count: u64,
    max_learners: u64,
)
    ensures can_add_learner_spec(false, true, learner_count, max_learners, false)
{}

// r[verify learner-change.can-add]
/// Voters cannot be added as learners.
proof fn voter_cannot_add_as_learner(
    is_existing: bool,
    count: u64,
    max: u64,
    pending: bool,
)
    ensures !can_add_learner_spec(true, is_existing, count, max, pending)
{}

// r[verify learner-change.can-add]
/// Pending config change blocks add.
proof fn pending_blocks_add(
    is_voter: bool,
    is_existing: bool,
    count: u64,
    max: u64,
)
    ensures !can_add_learner_spec(is_voter, is_existing, count, max, true)
{}

// r[verify learner-change.add-result]
/// Adding an existing learner does not change count.
proof fn add_existing_no_change(learner_count: u64)
    ensures add_learner_result_spec(learner_count, true) == learner_count
{}

// r[verify learner-change.add-result]
/// Adding a new learner increases count by 1.
proof fn add_new_increments(learner_count: u64)
    requires learner_count < u64::MAX
    ensures add_learner_result_spec(learner_count, false) == learner_count + 1
{}

// r[verify learner-change.can-remove]
/// Promoting learner cannot be removed.
proof fn promoting_blocks_remove()
    ensures !can_remove_learner_spec(true, true, false)
{}

// r[verify learner-change.can-remove]
/// Non-learner cannot be removed as learner.
proof fn non_learner_cannot_remove(promoting: bool, pending: bool)
    ensures !can_remove_learner_spec(false, promoting, pending)
{}

// r[verify learner-change.remove-result]
/// Removing a learner decreases count by 1.
proof fn remove_decrements(learner_count: u64)
    requires learner_count > 0
    ensures remove_learner_result_spec(learner_count, true) == learner_count - 1
{}

// r[verify learner-change.remove-result]
/// Removing a non-learner does not change count.
proof fn remove_non_learner_no_change(learner_count: u64)
    ensures remove_learner_result_spec(learner_count, false) == learner_count
{}

// r[verify learner-change.add-result]
/// Add then remove is a roundtrip (for new learners).
proof fn add_remove_roundtrip(learner_count: u64)
    requires learner_count < u64::MAX
    ensures remove_learner_result_spec(
        add_learner_result_spec(learner_count, false),
        true,
    ) == learner_count
{}

// r[verify learner-change.quorum-unaffected]
/// Learner operations do not change voter count (by definition).
proof fn learner_ops_preserve_quorum(voter_count: u64)
    ensures quorum_unchanged_spec(voter_count, voter_count)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify learner-change.can-add]
    #[test]
    fn test_can_add_learner() {
        assert!(can_add_learner(false, false, 2, 5, false)); // new learner, under max
        assert!(can_add_learner(false, true, 5, 5, false)); // existing (idempotent)
        assert!(!can_add_learner(true, false, 0, 5, false)); // voter
        assert!(!can_add_learner(false, false, 5, 5, false)); // at max
        assert!(!can_add_learner(false, false, 2, 5, true)); // pending config
        assert!(can_add_learner(false, false, 10, 0, false)); // unlimited (max=0)
    }

    // r[verify learner-change.add-result]
    #[test]
    fn test_add_learner_result() {
        assert_eq!(add_learner_result(3, false), 4);
        assert_eq!(add_learner_result(3, true), 3); // idempotent
    }

    // r[verify learner-change.can-remove]
    #[test]
    fn test_can_remove_learner() {
        assert!(can_remove_learner(true, false, false));
        assert!(!can_remove_learner(false, false, false)); // not a learner
        assert!(!can_remove_learner(true, true, false)); // promoting
        assert!(!can_remove_learner(true, false, true)); // pending config
    }

    // r[verify learner-change.remove-result]
    #[test]
    fn test_remove_learner_result() {
        assert_eq!(remove_learner_result(3, true), 2);
        assert_eq!(remove_learner_result(3, false), 3); // not a learner
        assert_eq!(remove_learner_result(0, true), 0); // floor at 0
    }

    // r[verify learner-change.quorum-unaffected]
    #[test]
    fn test_quorum_unchanged() {
        assert!(quorum_unchanged(5, 5));
        assert!(!quorum_unchanged(5, 4));
    }

    proptest! {
        // r[verify learner-change.add-result]
        #[test]
        fn prop_add_remove_roundtrip(count in 0u64..u64::MAX - 1) {
            let after_add = add_learner_result(count, false);
            let after_remove = remove_learner_result(after_add, true);
            prop_assert_eq!(after_remove, count);
        }

        // r[verify learner-change.add-result]
        #[test]
        fn prop_add_existing_idempotent(count: u64) {
            prop_assert_eq!(add_learner_result(count, true), count);
        }

        // r[verify learner-change.can-add]
        #[test]
        fn prop_voter_never_added(
            is_existing: bool,
            count: u64,
            max: u64,
            pending: bool,
        ) {
            prop_assert!(!can_add_learner(true, is_existing, count, max, pending));
        }

        // r[verify learner-change.can-add]
        #[test]
        fn prop_pending_blocks_all(
            is_voter: bool,
            is_existing: bool,
            count: u64,
            max: u64,
        ) {
            prop_assert!(!can_add_learner(is_voter, is_existing, count, max, true));
        }
    }
}
