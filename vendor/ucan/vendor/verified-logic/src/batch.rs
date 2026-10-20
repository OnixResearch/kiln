//! Verified write batch limit checking and flush decisions.
//!
//! Determines when a batch of write operations should be flushed based
//! on entry count, byte size, and scheduling state. Matches aspen's
//! `check_batch_limits` and `determine_flush_action`.

#![allow(clippy::if_same_then_else)]

use vstd::prelude::*;

verus! {

// --- check_limits ---

// r[depends batch.check-limits]
/// Spec: whether adding an operation would exceed limits.
/// Returns (exceeds_entries, exceeds_bytes).
pub open spec fn check_limits_spec(
    current_entries: u64,
    current_bytes: u64,
    op_bytes: u64,
    max_entries: u64,
    max_bytes: u64,
) -> (bool, bool) {
    let has_pending = current_entries > 0;
    let exceeds_entries = has_pending && current_entries >= max_entries;
    let exceeds_bytes = has_pending && {
        let new_bytes = if current_bytes as int + op_bytes as int > u64::MAX as int {
            u64::MAX
        } else {
            (current_bytes + op_bytes) as u64
        };
        new_bytes > max_bytes
    };
    (exceeds_entries, exceeds_bytes)
}

/// Spec: either limit exceeded.
pub open spec fn would_exceed_spec(
    current_entries: u64,
    current_bytes: u64,
    op_bytes: u64,
    max_entries: u64,
    max_bytes: u64,
) -> bool {
    let (ee, eb) = check_limits_spec(current_entries, current_bytes, op_bytes, max_entries, max_bytes);
    ee || eb
}

// r[impl batch.check-limits]
/// Checks whether adding an operation would exceed batch limits.
///
/// Returns `(exceeds_entries, exceeds_bytes)`. An empty batch (zero
/// entries) never reports exceeded — the first operation always fits.
/// Matches aspen's `check_batch_limits`.
pub fn check_limits(
    current_entries: u64,
    current_bytes: u64,
    op_bytes: u64,
    max_entries: u64,
    max_bytes: u64,
) -> (result: (bool, bool))
    ensures result == check_limits_spec(current_entries, current_bytes, op_bytes, max_entries, max_bytes)
{
    let has_pending = current_entries > 0;
    let exceeds_entries = has_pending && current_entries >= max_entries;
    // Manual saturating_add (Verus doesn't support the method)
    let new_bytes = if current_bytes > u64::MAX - op_bytes {
        u64::MAX
    } else {
        current_bytes + op_bytes
    };
    let exceeds_bytes = has_pending && new_bytes > max_bytes;
    (exceeds_entries, exceeds_bytes)
}

// r[verify batch.check-limits]
/// Empty batch never exceeds.
proof fn empty_never_exceeds(op_bytes: u64, max_entries: u64, max_bytes: u64)
    ensures check_limits_spec(0, 0, op_bytes, max_entries, max_bytes) == (false, false)
{}

// r[verify batch.check-limits]
/// Full entries always exceeds (when non-empty).
proof fn full_entries_exceeds(
    current_entries: u64, current_bytes: u64, op_bytes: u64,
    max_entries: u64, max_bytes: u64,
)
    requires current_entries > 0, current_entries >= max_entries
    ensures check_limits_spec(current_entries, current_bytes, op_bytes, max_entries, max_bytes).0
{}

// r[verify batch.check-limits]
/// Under both limits means no exceed.
proof fn under_limits_ok(
    current_entries: u64, current_bytes: u64, op_bytes: u64,
    max_entries: u64, max_bytes: u64,
)
    requires
        current_entries > 0,
        current_entries < max_entries,
        current_bytes as int + op_bytes as int <= max_bytes as int,
    ensures !would_exceed_spec(current_entries, current_bytes, op_bytes, max_entries, max_bytes)
{}

// --- flush_decision ---

/// Flush decision result.
pub enum FlushDecision {
    /// Flush the batch immediately (limits reached or batching disabled).
    Immediate,
    /// Schedule a delayed flush.
    Delayed,
    /// No action needed.
    None,
}

// r[depends batch.flush-decision]
/// Spec: flush decision. Returns 1 for Immediate, 2 for Delayed, 0 for None.
pub open spec fn flush_decision_spec(
    pending_count: u64,
    current_bytes: u64,
    max_entries: u64,
    max_bytes: u64,
    immediate_mode: bool,
    flush_scheduled: bool,
) -> int {
    if pending_count >= max_entries {
        1 // Immediate: entries full
    } else if current_bytes >= max_bytes {
        1 // Immediate: bytes full
    } else if pending_count == 0 {
        0 // None: nothing pending
    } else if immediate_mode {
        1 // Immediate: batching disabled
    } else if !flush_scheduled {
        2 // Delayed: schedule a flush
    } else {
        0 // None: already scheduled
    }
}

// r[impl batch.flush-decision]
/// Determines what flush action to take after adding to a batch.
///
/// Matches aspen's `determine_flush_action`.
pub fn flush_decision(
    pending_count: u64,
    current_bytes: u64,
    max_entries: u64,
    max_bytes: u64,
    immediate_mode: bool,
    flush_scheduled: bool,
) -> (result: FlushDecision)
    ensures
        result is Immediate <==> flush_decision_spec(
            pending_count, current_bytes, max_entries, max_bytes,
            immediate_mode, flush_scheduled) == 1,
        result is Delayed <==> flush_decision_spec(
            pending_count, current_bytes, max_entries, max_bytes,
            immediate_mode, flush_scheduled) == 2,
        result is None <==> flush_decision_spec(
            pending_count, current_bytes, max_entries, max_bytes,
            immediate_mode, flush_scheduled) == 0,
{
    if pending_count >= max_entries {
        FlushDecision::Immediate
    } else if current_bytes >= max_bytes {
        FlushDecision::Immediate
    } else if pending_count == 0 {
        FlushDecision::None
    } else if immediate_mode {
        FlushDecision::Immediate
    } else if !flush_scheduled {
        FlushDecision::Delayed
    } else {
        FlushDecision::None
    }
}

// r[verify batch.flush-decision]
/// Full batch always flushes immediately.
proof fn full_flushes(
    pending_count: u64, current_bytes: u64, max_entries: u64, max_bytes: u64,
    immediate_mode: bool, flush_scheduled: bool,
)
    requires pending_count >= max_entries
    ensures flush_decision_spec(
        pending_count, current_bytes, max_entries, max_bytes,
        immediate_mode, flush_scheduled) == 1
{}

// r[verify batch.flush-decision]
/// Empty batch never flushes.
proof fn empty_no_flush(
    current_bytes: u64, max_entries: u64, max_bytes: u64,
    immediate_mode: bool, flush_scheduled: bool,
)
    requires max_entries > 0, current_bytes < max_bytes
    ensures flush_decision_spec(
        0, current_bytes, max_entries, max_bytes,
        immediate_mode, flush_scheduled) == 0
{}

// r[verify batch.flush-decision]
/// Immediate mode always flushes non-empty batches.
proof fn immediate_mode_flushes(
    pending_count: u64, current_bytes: u64, max_entries: u64, max_bytes: u64,
    flush_scheduled: bool,
)
    requires pending_count > 0, pending_count < max_entries, current_bytes < max_bytes
    ensures flush_decision_spec(
        pending_count, current_bytes, max_entries, max_bytes,
        true, flush_scheduled) == 1
{}

// r[verify batch.flush-decision]
/// With a flush already scheduled, no double-scheduling.
proof fn no_double_schedule(
    pending_count: u64, current_bytes: u64, max_entries: u64, max_bytes: u64,
)
    requires pending_count > 0, pending_count < max_entries, current_bytes < max_bytes
    ensures flush_decision_spec(
        pending_count, current_bytes, max_entries, max_bytes,
        false, true) == 0
{}

// --- op_size ---

// r[depends batch.op-size]
/// Spec: total size of a set operation.
pub open spec fn op_size_spec(key_len: u64, value_len: u64) -> u64 {
    if key_len as int + value_len as int > u64::MAX as int {
        u64::MAX
    } else {
        (key_len + value_len) as u64
    }
}

/// Spec: size of a delete operation.
pub open spec fn delete_op_size_spec(key_len: u64) -> u64 {
    key_len
}

// r[impl batch.op-size]
/// Returns the total size of a set operation (key + value), saturating.
pub fn op_size(key_len: u64, value_len: u64) -> (result: u64)
    ensures result == op_size_spec(key_len, value_len)
{
    if key_len > u64::MAX - value_len {
        u64::MAX
    } else {
        key_len + value_len
    }
}

// r[impl batch.op-size]
/// Returns the size of a delete operation (key only).
pub fn delete_op_size(key_len: u64) -> (result: u64)
    ensures result == delete_op_size_spec(key_len)
{
    key_len
}

// r[verify batch.op-size]
/// op_size >= key_len always.
proof fn op_size_ge_key(key_len: u64, value_len: u64)
    ensures op_size_spec(key_len, value_len) >= key_len
{}

// r[verify batch.op-size]
/// op_size >= value_len always.
proof fn op_size_ge_value(key_len: u64, value_len: u64)
    ensures op_size_spec(key_len, value_len) >= value_len
{}

// r[verify batch.op-size]
/// delete is always <= set for same key.
proof fn delete_le_set(key_len: u64, value_len: u64)
    ensures delete_op_size_spec(key_len) <= op_size_spec(key_len, value_len)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify batch.check-limits]
    #[test]
    fn test_check_limits() {
        // Empty batch — always accepts
        let (ee, eb) = check_limits(0, 0, 1_000_000, 1, 1);
        assert!(!ee);
        assert!(!eb);

        // Entries full
        let (ee, eb) = check_limits(100, 0, 0, 100, u64::MAX);
        assert!(ee);
        assert!(!eb);

        // Bytes exceeded
        let (ee, eb) = check_limits(1, 900_000, 200_000, 100, 1_000_000);
        assert!(!ee);
        assert!(eb);

        // Under both limits
        let (ee, eb) = check_limits(99, 500_000, 499_999, 100, 1_000_000);
        assert!(!ee);
        assert!(!eb);
    }

    // r[verify batch.flush-decision]
    #[test]
    fn test_flush_decision() {
        // Full entries → Immediate
        assert!(matches!(flush_decision(100, 0, 100, 1_000_000, false, false), FlushDecision::Immediate));

        // Full bytes → Immediate
        assert!(matches!(flush_decision(1, 1_000_000, 100, 1_000_000, false, false), FlushDecision::Immediate));

        // Empty → None
        assert!(matches!(flush_decision(0, 0, 100, 1_000_000, false, false), FlushDecision::None));

        // Immediate mode → Immediate
        assert!(matches!(flush_decision(1, 100, 100, 1_000_000, true, false), FlushDecision::Immediate));

        // Not scheduled → Delayed
        assert!(matches!(flush_decision(1, 100, 100, 1_000_000, false, false), FlushDecision::Delayed));

        // Already scheduled → None
        assert!(matches!(flush_decision(1, 100, 100, 1_000_000, false, true), FlushDecision::None));
    }

    // r[verify batch.op-size]
    #[test]
    fn test_op_size() {
        assert_eq!(op_size(8, 10), 18);
        assert_eq!(op_size(0, 0), 0);
        assert_eq!(op_size(u64::MAX, 1), u64::MAX); // saturates
        assert_eq!(delete_op_size(8), 8);
    }

    proptest! {
        // r[verify batch.check-limits]
        #[test]
        fn prop_empty_never_exceeds(op_bytes: u64, max_entries in 1u64..=1000, max_bytes in 1u64..=u64::MAX) {
            let (ee, eb) = check_limits(0, 0, op_bytes, max_entries, max_bytes);
            prop_assert!(!ee);
            prop_assert!(!eb);
        }

        // r[verify batch.check-limits]
        #[test]
        fn prop_full_entries_exceeds(
            entries in 1u64..=1000,
            current_bytes: u64,
            op_bytes: u64,
            max_bytes: u64,
        ) {
            let (ee, _) = check_limits(entries, current_bytes, op_bytes, entries, max_bytes);
            prop_assert!(ee);
        }

        // r[verify batch.flush-decision]
        #[test]
        fn prop_full_always_immediate(
            count in 100u64..=200,
            bytes: u64,
            max_bytes: u64,
            im: bool,
            sched: bool,
        ) {
            let max_entries = 100u64;
            assert!(matches!(
                flush_decision(count, bytes, max_entries, max_bytes, im, sched),
                FlushDecision::Immediate
            ));
        }

        // r[verify batch.flush-decision]
        #[test]
        fn prop_immediate_mode_non_empty(
            count in 1u64..=99,
            bytes in 0u64..=999_999,
            sched: bool,
        ) {
            assert!(matches!(
                flush_decision(count, bytes, 100, 1_000_000, true, sched),
                FlushDecision::Immediate
            ));
        }

        // r[verify batch.op-size]
        #[test]
        fn prop_op_size_ge_parts(key_len: u64, value_len: u64) {
            let s = op_size(key_len, value_len);
            prop_assert!(s >= key_len);
            prop_assert!(s >= value_len);
        }

        // r[verify batch.op-size]
        #[test]
        fn prop_delete_le_set(key_len: u64, value_len: u64) {
            prop_assert!(delete_op_size(key_len) <= op_size(key_len, value_len));
        }

        // r[verify batch.check-limits]
        // r[verify batch.flush-decision]
        // r[verify batch.op-size]
        /// Model oracle: simulate a sequence of adds, check limits and flush
        /// decisions match manual calculation at every step.
        #[test]
        fn prop_batch_lifecycle(
            max_entries in 2u64..=20,
            max_bytes in 100u64..=10_000,
            ops in proptest::collection::vec((1u64..=50, 1u64..=50), 0..=30),
        ) {
            let mut entries = 0u64;
            let mut bytes = 0u64;

            for &(key_len, val_len) in &ops {
                let size = op_size(key_len, val_len);
                let (ee, eb) = check_limits(entries, bytes, size, max_entries, max_bytes);

                // Manual check
                if entries == 0 {
                    prop_assert!(!ee);
                    prop_assert!(!eb);
                } else {
                    prop_assert_eq!(ee, entries >= max_entries);
                }

                // Flush decision consistency
                let fd = flush_decision(entries, bytes, max_entries, max_bytes, false, false);
                if entries >= max_entries || bytes >= max_bytes {
                    prop_assert!(matches!(fd, FlushDecision::Immediate));
                } else if entries == 0 {
                    prop_assert!(matches!(fd, FlushDecision::None));
                } else {
                    prop_assert!(matches!(fd, FlushDecision::Delayed));
                }

                // Simulate add (reset if would exceed)
                if ee || eb {
                    entries = 1;
                    bytes = size;
                } else {
                    entries += 1;
                    bytes = bytes.saturating_add(size);
                }
            }
        }
    }
}
