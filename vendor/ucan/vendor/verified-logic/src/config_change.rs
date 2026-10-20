//! Verified configuration change protocol state machine for Raft.
//!
//! Joint consensus quorum math lives in `joint_consensus.rs`. This module
//! covers the protocol-level FSM: tracking which configuration(s) are active,
//! enforcing that at most one config change is pending at a time, and the
//! commit ordering between C_old,new and C_new entries. Raft §6 requires
//! that the system never makes a decision without approval from at least
//! one active configuration.

use vstd::prelude::*;

verus! {

/// Configuration change phase.
/// 0 = Stable (single config active)
/// 1 = Joint (both C_old and C_new active)
pub const PHASE_STABLE: u8 = 0;
/// Joint consensus: both C_old and C_new active.
pub const PHASE_JOINT: u8 = 1;

// --- is_valid_phase ---

// r[depends config-change.is-valid-phase]
/// Spec: is this a valid configuration phase?
pub open spec fn is_valid_phase_spec(phase: u8) -> bool {
    phase == PHASE_STABLE || phase == PHASE_JOINT
}

// r[impl config-change.is-valid-phase]
/// Check if a phase value is valid.
pub fn is_valid_phase(phase: u8) -> (result: bool)
    ensures result == is_valid_phase_spec(phase)
{
    phase == PHASE_STABLE || phase == PHASE_JOINT
}

// --- can_start_config_change ---

// r[depends config-change.can-start]
/// Spec: can a new configuration change be started?
///
/// A config change can only start when:
/// 1. The current phase is Stable (no pending change).
/// 2. The server is a leader (state check done at call site).
/// 3. The new config has at least one member.
pub open spec fn can_start_config_change_spec(
    current_phase: u8,
    new_config_size: u64,
) -> bool {
    current_phase == PHASE_STABLE && new_config_size > 0
}

// r[impl config-change.can-start]
/// Check if a config change can be started.
pub fn can_start_config_change(
    current_phase: u8,
    new_config_size: u64,
) -> (result: bool)
    ensures result == can_start_config_change_spec(current_phase, new_config_size)
{
    current_phase == PHASE_STABLE && new_config_size > 0
}

// --- enter_joint ---

// r[depends config-change.enter-joint]
/// Spec: transition from Stable to Joint phase.
///
/// Returns the new phase. Caller must append the C_old,new log entry
/// and replicate it before the joint config takes effect.
pub open spec fn enter_joint_spec(current_phase: u8) -> u8 {
    PHASE_JOINT
}

// r[impl config-change.enter-joint]
/// Enter joint consensus phase.
pub fn enter_joint(_current_phase: u8) -> (result: u8)
    requires _current_phase == PHASE_STABLE
    ensures result == PHASE_JOINT
{
    PHASE_JOINT
}

// --- leave_joint ---

// r[depends config-change.leave-joint]
/// Spec: transition from Joint to Stable phase (C_new only).
///
/// Only valid after the joint config entry is committed.
pub open spec fn leave_joint_spec(current_phase: u8) -> u8 {
    PHASE_STABLE
}

// r[impl config-change.leave-joint]
/// Leave joint consensus, transition to C_new only.
pub fn leave_joint(_current_phase: u8) -> (result: u8)
    requires _current_phase == PHASE_JOINT
    ensures result == PHASE_STABLE
{
    PHASE_STABLE
}

// --- is_joint_committed ---

// r[depends config-change.is-joint-committed]
/// Spec: has the joint configuration entry been committed?
///
/// The joint config entry is at `joint_entry_index` in the log.
/// It is committed when the commit index reaches or passes it.
pub open spec fn is_joint_committed_spec(
    joint_entry_index: u64,
    commit_index: u64,
) -> bool {
    commit_index >= joint_entry_index
}

// r[impl config-change.is-joint-committed]
/// Check if the joint config entry is committed.
pub fn is_joint_committed(
    joint_entry_index: u64,
    commit_index: u64,
) -> (result: bool)
    ensures result == is_joint_committed_spec(joint_entry_index, commit_index)
{
    commit_index >= joint_entry_index
}

// --- can_leave_joint ---

// r[depends config-change.can-leave-joint]
/// Spec: can we transition from Joint to Stable?
///
/// Requires: currently in joint phase AND the joint entry is committed.
pub open spec fn can_leave_joint_spec(
    current_phase: u8,
    joint_entry_index: u64,
    commit_index: u64,
) -> bool {
    current_phase == PHASE_JOINT
    && is_joint_committed_spec(joint_entry_index, commit_index)
}

// r[impl config-change.can-leave-joint]
/// Check if we can leave joint consensus.
pub fn can_leave_joint(
    current_phase: u8,
    joint_entry_index: u64,
    commit_index: u64,
) -> (result: bool)
    ensures result == can_leave_joint_spec(current_phase, joint_entry_index, commit_index)
{
    current_phase == PHASE_JOINT && commit_index >= joint_entry_index
}

// --- config_for_quorum ---

/// Quorum mode: which configuration(s) must approve decisions.
/// 0 = single config only
/// 1 = both old and new configs (joint)
pub const QUORUM_SINGLE: u8 = 0;
/// Both old and new configs must independently form quorums.
pub const QUORUM_JOINT: u8 = 1;

// r[depends config-change.config-for-quorum]
/// Spec: which quorum mode is active for the current phase?
pub open spec fn config_for_quorum_spec(phase: u8) -> u8 {
    if phase == PHASE_JOINT { QUORUM_JOINT } else { QUORUM_SINGLE }
}

// r[impl config-change.config-for-quorum]
/// Get the quorum mode for the current configuration phase.
pub fn config_for_quorum(phase: u8) -> (result: u8)
    ensures result == config_for_quorum_spec(phase)
{
    if phase == PHASE_JOINT { QUORUM_JOINT } else { QUORUM_SINGLE }
}

// --- leader_in_new_config ---

// r[depends config-change.leader-in-new-config]
/// Spec: is the current leader a member of the new config?
///
/// If not, the leader must step down after C_new is committed.
/// This is the "leader not in C_new" case from §6.
pub open spec fn leader_in_new_config_spec(
    leader_id: u64,
    new_config: Seq<u64>,
) -> bool {
    exists|i: int| 0 <= i < new_config.len() && new_config[i] == leader_id
}

// r[impl config-change.leader-in-new-config]
/// Check if the leader is in the new configuration.
pub fn leader_in_new_config(
    leader_id: u64,
    new_config: &[u64],
) -> (result: bool)
    ensures result == leader_in_new_config_spec(leader_id, new_config@)
{
    let mut i: usize = 0;
    while i < new_config.len()
        invariant
            0 <= i <= new_config@.len(),
            forall|j: int| 0 <= j < i as int ==> new_config@[j] != leader_id,
        decreases new_config.len() - i,
    {
        if new_config[i] == leader_id {
            return true;
        }
        i = i + 1;
    }
    false
}

// --- should_leader_step_down ---

// r[depends config-change.should-step-down]
/// Spec: should the leader step down after completing a config change?
///
/// The leader steps down when C_new is committed and the leader is
/// not a member of C_new.
pub open spec fn should_leader_step_down_spec(
    phase: u8,
    leader_in_new: bool,
    c_new_committed: bool,
) -> bool {
    phase == PHASE_STABLE && c_new_committed && !leader_in_new
}

// r[impl config-change.should-step-down]
/// Check if the leader should step down after config change.
pub fn should_leader_step_down(
    phase: u8,
    leader_in_new: bool,
    c_new_committed: bool,
) -> (result: bool)
    ensures result == should_leader_step_down_spec(phase, leader_in_new, c_new_committed)
{
    phase == PHASE_STABLE && c_new_committed && !leader_in_new
}

// --- Safety proofs ---

// r[verify config-change.can-start]
/// Cannot start a change during joint phase.
proof fn no_change_during_joint(new_config_size: u64)
    ensures !can_start_config_change_spec(PHASE_JOINT, new_config_size)
{}

// r[verify config-change.can-start]
/// Cannot start with empty new config.
proof fn no_empty_config()
    ensures !can_start_config_change_spec(PHASE_STABLE, 0)
{}

// r[verify config-change.enter-joint]
/// Entering joint from stable produces joint.
proof fn enter_joint_correct()
    ensures enter_joint_spec(PHASE_STABLE) == PHASE_JOINT
{}

// r[verify config-change.leave-joint]
/// Leaving joint produces stable.
proof fn leave_joint_correct()
    ensures leave_joint_spec(PHASE_JOINT) == PHASE_STABLE
{}

// r[verify config-change.can-leave-joint]
/// Cannot leave joint until committed.
proof fn cannot_leave_uncommitted(joint_entry_index: u64, commit_index: u64)
    requires commit_index < joint_entry_index
    ensures !can_leave_joint_spec(PHASE_JOINT, joint_entry_index, commit_index)
{}

// r[verify config-change.can-leave-joint]
/// Cannot leave joint from stable phase.
proof fn cannot_leave_from_stable(joint_entry_index: u64, commit_index: u64)
    ensures !can_leave_joint_spec(PHASE_STABLE, joint_entry_index, commit_index)
{}

// r[verify config-change.can-start]
// r[verify config-change.enter-joint]
// r[verify config-change.leave-joint]
/// Full lifecycle: stable → joint → stable.
proof fn full_lifecycle(new_config_size: u64, joint_entry_index: u64, commit_index: u64)
    requires
        new_config_size > 0,
        commit_index >= joint_entry_index,
    ensures
        can_start_config_change_spec(PHASE_STABLE, new_config_size),
        enter_joint_spec(PHASE_STABLE) == PHASE_JOINT,
        can_leave_joint_spec(PHASE_JOINT, joint_entry_index, commit_index),
        leave_joint_spec(PHASE_JOINT) == PHASE_STABLE,
{}

// r[verify config-change.config-for-quorum]
/// Stable phase uses single quorum.
proof fn stable_single_quorum()
    ensures config_for_quorum_spec(PHASE_STABLE) == QUORUM_SINGLE
{}

// r[verify config-change.config-for-quorum]
/// Joint phase uses joint quorum.
proof fn joint_joint_quorum()
    ensures config_for_quorum_spec(PHASE_JOINT) == QUORUM_JOINT
{}

// r[verify config-change.should-step-down]
/// Leader in new config does not step down.
proof fn leader_in_config_stays(c_new_committed: bool)
    ensures !should_leader_step_down_spec(PHASE_STABLE, true, c_new_committed)
{}

// r[verify config-change.should-step-down]
/// Leader not in new config steps down after commit.
proof fn leader_not_in_config_steps_down()
    ensures should_leader_step_down_spec(PHASE_STABLE, false, true)
{}

// r[verify config-change.should-step-down]
/// No step-down during joint phase (C_new not yet sole config).
proof fn no_step_down_during_joint(leader_in_new: bool, c_new_committed: bool)
    ensures !should_leader_step_down_spec(PHASE_JOINT, leader_in_new, c_new_committed)
{}

// r[verify config-change.is-valid-phase]
/// Only two valid phases.
proof fn only_two_phases(phase: u8)
    requires is_valid_phase_spec(phase)
    ensures phase == PHASE_STABLE || phase == PHASE_JOINT
{}

// r[verify config-change.enter-joint]
// r[verify config-change.leave-joint]
/// Phase transitions produce valid phases.
proof fn transitions_produce_valid_phases()
    ensures
        is_valid_phase_spec(enter_joint_spec(PHASE_STABLE)),
        is_valid_phase_spec(leave_joint_spec(PHASE_JOINT)),
{}

} // verus!

#[cfg(test)]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::default_numeric_fallback
)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify config-change.can-start]
    // r[verify config-change.is-joint-committed]
    #[test]
    fn test_can_start_config_change() {
        assert!(can_start_config_change(PHASE_STABLE, 3));
        assert!(!can_start_config_change(PHASE_JOINT, 3));
        assert!(!can_start_config_change(PHASE_STABLE, 0));
    }

    // r[verify config-change.enter-joint]
    #[test]
    fn test_enter_leave_joint() {
        let joint = enter_joint(PHASE_STABLE);
        assert_eq!(joint, PHASE_JOINT);
        let stable = leave_joint(joint);
        assert_eq!(stable, PHASE_STABLE);
    }

    // r[verify config-change.can-leave-joint]
    #[test]
    fn test_can_leave_joint() {
        assert!(can_leave_joint(PHASE_JOINT, 5, 5)); // committed
        assert!(can_leave_joint(PHASE_JOINT, 5, 10)); // past committed
        assert!(!can_leave_joint(PHASE_JOINT, 5, 4)); // not committed
        assert!(!can_leave_joint(PHASE_STABLE, 5, 10)); // wrong phase
    }

    // r[verify config-change.config-for-quorum]
    #[test]
    fn test_config_for_quorum() {
        assert_eq!(config_for_quorum(PHASE_STABLE), QUORUM_SINGLE);
        assert_eq!(config_for_quorum(PHASE_JOINT), QUORUM_JOINT);
    }

    // r[verify config-change.leader-in-new-config]
    #[test]
    fn test_leader_in_new_config() {
        assert!(leader_in_new_config(3, &[1, 2, 3, 4, 5]));
        assert!(!leader_in_new_config(6, &[1, 2, 3, 4, 5]));
        assert!(!leader_in_new_config(1, &[]));
    }

    // r[verify config-change.should-step-down]
    #[test]
    fn test_should_leader_step_down() {
        assert!(should_leader_step_down(PHASE_STABLE, false, true));
        assert!(!should_leader_step_down(PHASE_STABLE, true, true));
        assert!(!should_leader_step_down(PHASE_STABLE, false, false));
        assert!(!should_leader_step_down(PHASE_JOINT, false, true));
    }

    // r[verify config-change.can-start]
    // r[verify config-change.enter-joint]
    // r[verify config-change.leave-joint]
    #[test]
    fn test_full_lifecycle() {
        // Start in stable
        assert!(can_start_config_change(PHASE_STABLE, 5));

        // Enter joint
        let phase = enter_joint(PHASE_STABLE);
        assert_eq!(phase, PHASE_JOINT);
        assert_eq!(config_for_quorum(phase), QUORUM_JOINT);

        // Cannot start another change while in joint
        assert!(!can_start_config_change(phase, 3));

        // Joint entry at index 10, not yet committed at 8
        assert!(!can_leave_joint(phase, 10, 8));

        // Committed at 10
        assert!(can_leave_joint(phase, 10, 10));

        // Leave joint
        let phase = leave_joint(phase);
        assert_eq!(phase, PHASE_STABLE);
        assert_eq!(config_for_quorum(phase), QUORUM_SINGLE);
    }

    proptest! {
        // r[verify config-change.can-start]
        #[test]
        fn prop_no_change_during_joint(size: u64) {
            prop_assert!(!can_start_config_change(PHASE_JOINT, size));
        }

        // r[verify config-change.can-leave-joint]
        #[test]
        fn prop_committed_can_leave(entry_idx: u64, commit: u64) {
            if commit >= entry_idx {
                prop_assert!(can_leave_joint(PHASE_JOINT, entry_idx, commit));
            }
        }

        // r[verify config-change.can-leave-joint]
        #[test]
        fn prop_uncommitted_cannot_leave(entry_idx in 1u64..u64::MAX, commit: u64) {
            if commit < entry_idx {
                prop_assert!(!can_leave_joint(PHASE_JOINT, entry_idx, commit));
            }
        }

        // r[verify config-change.leader-in-new-config]
        #[test]
        fn prop_leader_in_config(
            members in proptest::collection::vec(0u64..100, 1..10),
            idx_frac in 0.0f64..1.0,
        ) {
            let idx = (idx_frac * (members.len() - 1) as f64) as usize;
            let leader_id = members[idx];
            prop_assert!(leader_in_new_config(leader_id, &members));
        }

        // r[verify config-change.is-valid-phase]
        #[test]
        fn prop_valid_phase(phase: u8) {
            prop_assert_eq!(is_valid_phase(phase), phase == 0 || phase == 1);
        }
    }
}
