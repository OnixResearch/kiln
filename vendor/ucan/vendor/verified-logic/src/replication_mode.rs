//! Verified replication mode FSM for Raft per-peer progress tracking.
//!
//! Each follower's replication state follows a three-state machine:
//! - **Probe**: match point unknown, send one entry at a time, wait for response
//! - **Replicate**: match point known, pipeline entries freely
//! - **Snapshot**: follower behind compaction point, send snapshot chunks
//!
//! Transitions:
//! - Probe → Replicate (successful AppendEntries response)
//! - Replicate → Probe (AppendEntries rejection)
//! - Replicate → Snapshot (follower behind compaction point)
//! - Snapshot → Probe (snapshot installed, resume probing)
//! - Probe → Snapshot (follower behind compaction point)

use vstd::prelude::*;

verus! {

/// Probe mode: match point unknown, send one entry at a time.
pub const MODE_PROBE: u8 = 0;
/// Replicate mode: match point known, pipeline entries.
pub const MODE_REPLICATE: u8 = 1;
/// Snapshot mode: sending snapshot chunks.
pub const MODE_SNAPSHOT: u8 = 2;

// --- is_valid_mode ---

// r[depends replication-mode.valid-mode]
/// Spec: is this a valid replication mode?
pub open spec fn is_valid_mode_spec(mode: u8) -> bool {
    mode == MODE_PROBE || mode == MODE_REPLICATE || mode == MODE_SNAPSHOT
}

// r[impl replication-mode.valid-mode]
/// Check if a mode value is valid.
pub fn is_valid_mode(mode: u8) -> (result: bool)
    ensures result == is_valid_mode_spec(mode)
{
    mode == MODE_PROBE || mode == MODE_REPLICATE || mode == MODE_SNAPSHOT
}

// --- can_pipeline ---

// r[depends replication-mode.can-pipeline]
/// Spec: can entries be pipelined in this mode?
///
/// Only Replicate mode allows pipelining. Probe sends one at a time,
/// Snapshot doesn't send log entries at all.
pub open spec fn can_pipeline_spec(mode: u8) -> bool {
    mode == MODE_REPLICATE
}

// r[impl replication-mode.can-pipeline]
/// Check if pipelining is allowed.
pub fn can_pipeline(mode: u8) -> (result: bool)
    ensures result == can_pipeline_spec(mode)
{
    mode == MODE_REPLICATE
}

// --- can_send_entries ---

// r[depends replication-mode.can-send-entries]
/// Spec: can log entries be sent in this mode?
///
/// Both Probe and Replicate send entries. Snapshot does not.
pub open spec fn can_send_entries_spec(mode: u8) -> bool {
    mode == MODE_PROBE || mode == MODE_REPLICATE
}

// r[impl replication-mode.can-send-entries]
/// Check if log entries can be sent.
pub fn can_send_entries(mode: u8) -> (result: bool)
    ensures result == can_send_entries_spec(mode)
{
    mode == MODE_PROBE || mode == MODE_REPLICATE
}

// --- transition_on_success ---

// r[depends replication-mode.transition-on-success]
/// Spec: mode after a successful AppendEntries response.
///
/// Probe → Replicate (match point discovered).
/// Replicate → Replicate (stay, pipelining continues).
/// Snapshot → unchanged (snapshot responses handled separately).
pub open spec fn transition_on_success_spec(mode: u8) -> u8 {
    if mode == MODE_PROBE {
        MODE_REPLICATE
    } else {
        mode
    }
}

// r[impl replication-mode.transition-on-success]
/// Compute the new mode after a successful AppendEntries.
pub fn transition_on_success(mode: u8) -> (result: u8)
    ensures
        result == transition_on_success_spec(mode),
        is_valid_mode_spec(mode) ==> is_valid_mode_spec(result),
{
    if mode == MODE_PROBE {
        MODE_REPLICATE
    } else {
        mode
    }
}

// --- transition_on_reject ---

// r[depends replication-mode.transition-on-reject]
/// Spec: mode after a rejected AppendEntries response.
///
/// Replicate → Probe (match point wrong, must re-probe).
/// Probe → Probe (stay, continue probing).
/// Snapshot → unchanged (snapshot has its own failure path).
pub open spec fn transition_on_reject_spec(mode: u8) -> u8 {
    if mode == MODE_REPLICATE {
        MODE_PROBE
    } else {
        mode
    }
}

// r[impl replication-mode.transition-on-reject]
/// Compute the new mode after a rejected AppendEntries.
pub fn transition_on_reject(mode: u8) -> (result: u8)
    ensures
        result == transition_on_reject_spec(mode),
        is_valid_mode_spec(mode) ==> is_valid_mode_spec(result),
{
    if mode == MODE_REPLICATE {
        MODE_PROBE
    } else {
        mode
    }
}

// --- transition_to_snapshot ---

// r[depends replication-mode.transition-to-snapshot]
/// Spec: should the leader switch to snapshot mode?
///
/// When the follower's next_index is at or before the leader's first
/// available log entry, the needed entries have been compacted. The
/// leader must send a snapshot instead.
pub open spec fn should_send_snapshot_spec(
    next_index: u64,
    first_log_index: u64,
) -> bool {
    next_index <= first_log_index
}

// r[impl replication-mode.transition-to-snapshot]
/// Check if the leader should switch to snapshot mode.
pub fn should_send_snapshot(
    next_index: u64,
    first_log_index: u64,
) -> (result: bool)
    ensures result == should_send_snapshot_spec(next_index, first_log_index)
{
    next_index <= first_log_index
}

// --- enter_snapshot ---

// r[depends replication-mode.enter-snapshot]
/// Spec: enter snapshot mode from any entry-sending mode.
pub open spec fn enter_snapshot_spec(mode: u8) -> u8 {
    if mode == MODE_PROBE || mode == MODE_REPLICATE {
        MODE_SNAPSHOT
    } else {
        mode
    }
}

// r[impl replication-mode.enter-snapshot]
/// Switch to snapshot mode.
pub fn enter_snapshot(mode: u8) -> (result: u8)
    ensures
        result == enter_snapshot_spec(mode),
        is_valid_mode_spec(mode) ==> is_valid_mode_spec(result),
{
    if mode == MODE_PROBE || mode == MODE_REPLICATE {
        MODE_SNAPSHOT
    } else {
        mode
    }
}

// --- snapshot_complete ---

// r[depends replication-mode.snapshot-complete]
/// Spec: mode after snapshot installation completes.
///
/// Snapshot → Probe (resume probing from the snapshot's last index).
/// Other modes unchanged (shouldn't happen, but safe).
pub open spec fn snapshot_complete_spec(mode: u8) -> u8 {
    if mode == MODE_SNAPSHOT {
        MODE_PROBE
    } else {
        mode
    }
}

// r[impl replication-mode.snapshot-complete]
/// Compute mode after snapshot completes.
pub fn snapshot_complete(mode: u8) -> (result: u8)
    ensures
        result == snapshot_complete_spec(mode),
        is_valid_mode_spec(mode) ==> is_valid_mode_spec(result),
{
    if mode == MODE_SNAPSHOT {
        MODE_PROBE
    } else {
        mode
    }
}

// --- entries_per_message ---

// r[depends replication-mode.entries-per-message]
/// Spec: max entries allowed per message in this mode.
///
/// Probe: 1 (conservative, probing for match point).
/// Replicate: max_batch (pipeline freely).
/// Snapshot: 0 (no log entries during snapshot).
pub open spec fn entries_per_message_spec(mode: u8, max_batch: u64) -> u64 {
    if mode == MODE_PROBE {
        1u64
    } else if mode == MODE_REPLICATE {
        max_batch
    } else {
        0u64
    }
}

// r[impl replication-mode.entries-per-message]
/// Get the max entries per message for this mode.
pub fn entries_per_message(mode: u8, max_batch: u64) -> (result: u64)
    ensures result == entries_per_message_spec(mode, max_batch)
{
    if mode == MODE_PROBE {
        1
    } else if mode == MODE_REPLICATE {
        max_batch
    } else {
        0
    }
}

// --- Safety proofs ---

// r[verify replication-mode.valid-mode]
/// All mode constants are valid.
proof fn all_constants_valid()
    ensures
        is_valid_mode_spec(MODE_PROBE),
        is_valid_mode_spec(MODE_REPLICATE),
        is_valid_mode_spec(MODE_SNAPSHOT),
{}

// r[verify replication-mode.transition-on-success]
/// Probe always transitions to Replicate on success.
proof fn probe_to_replicate_on_success()
    ensures transition_on_success_spec(MODE_PROBE) == MODE_REPLICATE
{}

// r[verify replication-mode.transition-on-success]
/// Replicate stays in Replicate on success.
proof fn replicate_stays_on_success()
    ensures transition_on_success_spec(MODE_REPLICATE) == MODE_REPLICATE
{}

// r[verify replication-mode.transition-on-reject]
/// Replicate falls back to Probe on reject.
proof fn replicate_to_probe_on_reject()
    ensures transition_on_reject_spec(MODE_REPLICATE) == MODE_PROBE
{}

// r[verify replication-mode.transition-on-reject]
/// Probe stays in Probe on reject.
proof fn probe_stays_on_reject()
    ensures transition_on_reject_spec(MODE_PROBE) == MODE_PROBE
{}

// r[verify replication-mode.snapshot-complete]
/// Snapshot completes back to Probe.
proof fn snapshot_to_probe()
    ensures snapshot_complete_spec(MODE_SNAPSHOT) == MODE_PROBE
{}

// r[verify replication-mode.enter-snapshot]
/// Probe can enter Snapshot.
proof fn probe_to_snapshot()
    ensures enter_snapshot_spec(MODE_PROBE) == MODE_SNAPSHOT
{}

// r[verify replication-mode.enter-snapshot]
/// Replicate can enter Snapshot.
proof fn replicate_to_snapshot()
    ensures enter_snapshot_spec(MODE_REPLICATE) == MODE_SNAPSHOT
{}

// r[verify replication-mode.valid-mode]
/// All transitions preserve mode validity.
proof fn transitions_preserve_validity(mode: u8)
    requires is_valid_mode_spec(mode)
    ensures
        is_valid_mode_spec(transition_on_success_spec(mode)),
        is_valid_mode_spec(transition_on_reject_spec(mode)),
        is_valid_mode_spec(enter_snapshot_spec(mode)),
        is_valid_mode_spec(snapshot_complete_spec(mode)),
{}

// r[verify replication-mode.can-pipeline]
/// Only Replicate allows pipelining.
proof fn pipeline_only_in_replicate()
    ensures
        !can_pipeline_spec(MODE_PROBE),
        can_pipeline_spec(MODE_REPLICATE),
        !can_pipeline_spec(MODE_SNAPSHOT),
{}

// r[verify replication-mode.can-send-entries]
/// Snapshot blocks entry sending.
proof fn snapshot_blocks_entries()
    ensures !can_send_entries_spec(MODE_SNAPSHOT)
{}

// r[verify replication-mode.entries-per-message]
/// Probe sends exactly one entry.
proof fn probe_sends_one(max_batch: u64)
    ensures entries_per_message_spec(MODE_PROBE, max_batch) == 1
{}

// r[verify replication-mode.entries-per-message]
/// Snapshot sends zero entries.
proof fn snapshot_sends_zero(max_batch: u64)
    ensures entries_per_message_spec(MODE_SNAPSHOT, max_batch) == 0
{}

// r[verify replication-mode.transition-on-success]
/// Success-then-reject returns to Probe from Replicate.
proof fn success_reject_cycle()
    ensures
        transition_on_reject_spec(
            transition_on_success_spec(MODE_PROBE)
        ) == MODE_PROBE
{}

// r[verify replication-mode.snapshot-complete]
/// Full snapshot lifecycle: enter → complete → Probe.
proof fn snapshot_lifecycle(mode: u8)
    requires mode == MODE_PROBE || mode == MODE_REPLICATE
    ensures snapshot_complete_spec(enter_snapshot_spec(mode)) == MODE_PROBE
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify replication-mode.valid-mode]
    #[test]
    fn test_is_valid_mode() {
        assert!(is_valid_mode(MODE_PROBE));
        assert!(is_valid_mode(MODE_REPLICATE));
        assert!(is_valid_mode(MODE_SNAPSHOT));
        assert!(!is_valid_mode(3));
        assert!(!is_valid_mode(255));
    }

    // r[verify replication-mode.can-pipeline]
    #[test]
    fn test_can_pipeline() {
        assert!(!can_pipeline(MODE_PROBE));
        assert!(can_pipeline(MODE_REPLICATE));
        assert!(!can_pipeline(MODE_SNAPSHOT));
    }

    // r[verify replication-mode.can-send-entries]
    #[test]
    fn test_can_send_entries() {
        assert!(can_send_entries(MODE_PROBE));
        assert!(can_send_entries(MODE_REPLICATE));
        assert!(!can_send_entries(MODE_SNAPSHOT));
    }

    // r[verify replication-mode.transition-on-success]
    #[test]
    fn test_transition_on_success() {
        assert_eq!(transition_on_success(MODE_PROBE), MODE_REPLICATE);
        assert_eq!(transition_on_success(MODE_REPLICATE), MODE_REPLICATE);
        assert_eq!(transition_on_success(MODE_SNAPSHOT), MODE_SNAPSHOT);
    }

    // r[verify replication-mode.transition-on-reject]
    #[test]
    fn test_transition_on_reject() {
        assert_eq!(transition_on_reject(MODE_REPLICATE), MODE_PROBE);
        assert_eq!(transition_on_reject(MODE_PROBE), MODE_PROBE);
        assert_eq!(transition_on_reject(MODE_SNAPSHOT), MODE_SNAPSHOT);
    }

    // r[verify replication-mode.enter-snapshot]
    #[test]
    fn test_enter_snapshot() {
        assert_eq!(enter_snapshot(MODE_PROBE), MODE_SNAPSHOT);
        assert_eq!(enter_snapshot(MODE_REPLICATE), MODE_SNAPSHOT);
        assert_eq!(enter_snapshot(MODE_SNAPSHOT), MODE_SNAPSHOT);
    }

    // r[verify replication-mode.snapshot-complete]
    #[test]
    fn test_snapshot_complete() {
        assert_eq!(snapshot_complete(MODE_SNAPSHOT), MODE_PROBE);
        assert_eq!(snapshot_complete(MODE_PROBE), MODE_PROBE);
        assert_eq!(snapshot_complete(MODE_REPLICATE), MODE_REPLICATE);
    }

    // r[verify replication-mode.entries-per-message]
    #[test]
    fn test_entries_per_message() {
        assert_eq!(entries_per_message(MODE_PROBE, 100), 1);
        assert_eq!(entries_per_message(MODE_REPLICATE, 100), 100);
        assert_eq!(entries_per_message(MODE_SNAPSHOT, 100), 0);
    }

    // r[verify replication-mode.transition-to-snapshot]
    #[test]
    fn test_should_send_snapshot() {
        assert!(should_send_snapshot(5, 10)); // behind compaction
        assert!(should_send_snapshot(10, 10)); // at compaction point
        assert!(!should_send_snapshot(15, 10)); // within log
    }

    proptest! {
        // r[verify replication-mode.valid-mode]
        #[test]
        fn prop_transitions_preserve_validity(mode in 0u8..3) {
            prop_assert!(is_valid_mode(mode));
            prop_assert!(is_valid_mode(transition_on_success(mode)));
            prop_assert!(is_valid_mode(transition_on_reject(mode)));
            prop_assert!(is_valid_mode(enter_snapshot(mode)));
            prop_assert!(is_valid_mode(snapshot_complete(mode)));
        }

        // r[verify replication-mode.snapshot-complete]
        #[test]
        fn prop_snapshot_lifecycle(mode in 0u8..2) {
            // Probe or Replicate → Snapshot → Probe
            let snap = enter_snapshot(mode);
            prop_assert_eq!(snap, MODE_SNAPSHOT);
            let after = snapshot_complete(snap);
            prop_assert_eq!(after, MODE_PROBE);
        }

        // r[verify replication-mode.transition-on-success]
        #[test]
        fn prop_success_reject_cycle(_attempt in 0u32..100) {
            // Probe → success → Replicate → reject → Probe
            let after_success = transition_on_success(MODE_PROBE);
            prop_assert_eq!(after_success, MODE_REPLICATE);
            let after_reject = transition_on_reject(after_success);
            prop_assert_eq!(after_reject, MODE_PROBE);
        }

        // r[verify replication-mode.entries-per-message]
        #[test]
        fn prop_probe_always_one(max_batch in 1u64..10000) {
            prop_assert_eq!(entries_per_message(MODE_PROBE, max_batch), 1);
        }

        // r[verify replication-mode.entries-per-message]
        #[test]
        fn prop_replicate_uses_max(max_batch: u64) {
            prop_assert_eq!(entries_per_message(MODE_REPLICATE, max_batch), max_batch);
        }
    }
}
