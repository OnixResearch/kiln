//! Write-ahead log segment management.
//!
//! Pure functions for WAL entry sequencing, segment space tracking, truncation
//! safety, and crash-recovery replay range computation. These compose with
//! [`monotonic`], [`watermark`], and [`arithmetic`] for cross-module properties.

use vstd::prelude::*;

verus! {

// ─── Sequence Contiguity ─────────────────────────────────────────────

/// Spec: consecutive WAL entries have adjacent sequence numbers.
// r[depends wal-segment.is-contiguous]
// r[impl wal-segment.is-contiguous]
pub open spec fn is_contiguous_spec(prev_seq: u64, current_seq: u64) -> bool {
    current_seq == prev_seq + 1
}

/// WAL entries MUST have consecutive sequence numbers.
// r[impl wal-segment.is-contiguous]
pub fn is_contiguous(prev_seq: u64, current_seq: u64) -> (result: bool)
    requires prev_seq < u64::MAX,
    ensures result == is_contiguous_spec(prev_seq, current_seq),
{
    current_seq == prev_seq + 1
}

// ─── Segment Space ───────────────────────────────────────────────────

/// Spec: remaining bytes in the current WAL segment.
// r[depends wal-segment.space-remaining]
// r[impl wal-segment.space-remaining]
pub open spec fn space_remaining_spec(written: u64, capacity: u64) -> u64 {
    if written <= capacity { (capacity - written) as u64 } else { 0 }
}

/// Remaining space in the WAL segment.
// r[impl wal-segment.space-remaining]
pub fn space_remaining(written: u64, capacity: u64) -> (result: u64)
    ensures result == space_remaining_spec(written, capacity),
{
    if written <= capacity { capacity - written } else { 0 }
}

/// Spec: can this entry fit in the remaining space?
// r[depends wal-segment.entry-fits]
// r[impl wal-segment.entry-fits]
pub open spec fn entry_fits_spec(written: u64, entry_size: u64, capacity: u64) -> bool {
    written as int + entry_size as int <= capacity as int
}

/// An entry fits when written + entry_size does not exceed capacity.
// r[impl wal-segment.entry-fits]
pub fn entry_fits(written: u64, entry_size: u64, capacity: u64) -> (result: bool)
    ensures result == entry_fits_spec(written, entry_size, capacity),
{
    // Use saturating add to avoid overflow, then compare.
    let sum = if entry_size > u64::MAX - written { u64::MAX } else { written + entry_size };
    if entry_size > u64::MAX - written {
        // Overflow case: sum exceeds u64::MAX, which is > any capacity.
        false
    } else {
        written + entry_size <= capacity
    }
}

/// Spec: is the segment full (time to rotate)?
// r[depends wal-segment.should-rotate]
// r[impl wal-segment.should-rotate]
pub open spec fn should_rotate_spec(written: u64, capacity: u64) -> bool {
    written >= capacity
}

/// A segment should be rotated when the write position reaches capacity.
// r[impl wal-segment.should-rotate]
pub fn should_rotate(written: u64, capacity: u64) -> (result: bool)
    ensures result == should_rotate_spec(written, capacity),
{
    written >= capacity
}

/// Spec: advance the write position by entry_size bytes.
// r[depends wal-segment.write-advance]
// r[impl wal-segment.write-advance]
pub open spec fn write_advance_spec(written: u64, entry_size: u64) -> Option<u64> {
    if written as int + entry_size as int <= u64::MAX as int {
        Some((written + entry_size) as u64)
    } else {
        None
    }
}

/// Advance the WAL write position. Returns None on overflow.
// r[impl wal-segment.write-advance]
pub fn write_advance(written: u64, entry_size: u64) -> (result: Option<u64>)
    ensures result == write_advance_spec(written, entry_size),
{
    if entry_size > u64::MAX - written {
        None
    } else {
        Some(written + entry_size)
    }
}

// ─── Truncation Safety ───────────────────────────────────────────────

/// Spec: is it safe to truncate the WAL at this sequence number?
// r[depends wal-segment.truncate-safe]
// r[impl wal-segment.truncate-safe]
pub open spec fn is_truncate_safe_spec(
    truncate_to: u64,
    committed_seq: u64,
    applied_seq: u64,
) -> bool {
    truncate_to <= committed_seq && truncate_to <= applied_seq
}

/// Truncation is safe only below both committed and applied points.
// r[impl wal-segment.truncate-safe]
pub fn is_truncate_safe(
    truncate_to: u64,
    committed_seq: u64,
    applied_seq: u64,
) -> (result: bool)
    ensures result == is_truncate_safe_spec(truncate_to, committed_seq, applied_seq),
{
    truncate_to <= committed_seq && truncate_to <= applied_seq
}

// ─── Replay Range ────────────────────────────────────────────────────

/// Spec: number of entries to replay during crash recovery.
// r[depends wal-segment.replay-count]
// r[impl wal-segment.replay-count]
pub open spec fn replay_count_spec(checkpoint_seq: u64, end_seq: u64) -> u64 {
    if end_seq >= checkpoint_seq { (end_seq - checkpoint_seq) as u64 } else { 0 }
}

/// The replay count is the gap between checkpoint and WAL end.
// r[impl wal-segment.replay-count]
pub fn replay_count(checkpoint_seq: u64, end_seq: u64) -> (result: u64)
    ensures result == replay_count_spec(checkpoint_seq, end_seq),
{
    if end_seq >= checkpoint_seq { end_seq - checkpoint_seq } else { 0 }
}

/// Spec: is the checkpoint position valid?
// r[depends wal-segment.checkpoint-valid]
// r[impl wal-segment.checkpoint-valid]
pub open spec fn is_checkpoint_valid_spec(
    checkpoint_seq: u64,
    committed_seq: u64,
) -> bool {
    checkpoint_seq <= committed_seq
}

/// A checkpoint is valid when it does not advance past committed.
// r[impl wal-segment.checkpoint-valid]
pub fn is_checkpoint_valid(
    checkpoint_seq: u64,
    committed_seq: u64,
) -> (result: bool)
    ensures result == is_checkpoint_valid_spec(checkpoint_seq, committed_seq),
{
    checkpoint_seq <= committed_seq
}

// ─── Proofs ──────────────────────────────────────────────────────────

// --- is_contiguous ---

/// Contiguity implies the current seq is exactly one past prev.
// r[verify wal-segment.is-contiguous]
proof fn contiguous_increments(prev_seq: u64, current_seq: u64)
    requires
        prev_seq < u64::MAX,
        is_contiguous_spec(prev_seq, current_seq),
    ensures current_seq == prev_seq + 1,
{}

/// Contiguous entries are distinct.
// r[verify wal-segment.is-contiguous]
proof fn contiguous_distinct(prev_seq: u64, current_seq: u64)
    requires
        prev_seq < u64::MAX,
        is_contiguous_spec(prev_seq, current_seq),
    ensures current_seq != prev_seq,
{}

/// Contiguity chains: if a→b and b→c, then c == a + 2.
// r[verify wal-segment.is-contiguous]
proof fn contiguous_chain(a: u64, b: u64, c: u64)
    requires
        a < u64::MAX - 1,
        is_contiguous_spec(a, b),
        is_contiguous_spec(b, c),
    ensures c == a + 2,
{}

// --- space_remaining ---

/// Empty segment has full capacity remaining.
// r[verify wal-segment.space-remaining]
proof fn space_remaining_empty(capacity: u64)
    ensures space_remaining_spec(0, capacity) == capacity,
{}

/// Full segment has zero remaining.
// r[verify wal-segment.space-remaining]
proof fn space_remaining_full(capacity: u64)
    ensures space_remaining_spec(capacity, capacity) == 0,
{}

/// Remaining is monotonically decreasing as written increases.
// r[verify wal-segment.space-remaining]
proof fn space_remaining_monotone(w1: u64, w2: u64, capacity: u64)
    requires w1 <= w2, w2 <= capacity,
    ensures space_remaining_spec(w1, capacity) >= space_remaining_spec(w2, capacity),
{}

// --- entry_fits ---

/// Zero-size entry always fits when segment isn't overfull.
// r[verify wal-segment.entry-fits]
proof fn zero_entry_fits(written: u64, capacity: u64)
    requires written <= capacity,
    ensures entry_fits_spec(written, 0, capacity),
{}

/// Entry larger than capacity never fits.
// r[verify wal-segment.entry-fits]
proof fn oversized_entry_never_fits(entry_size: u64, capacity: u64)
    requires entry_size as int > capacity as int,
    ensures !entry_fits_spec(0, entry_size, capacity),
{}

/// Fit implies remaining space is sufficient.
// r[verify wal-segment.entry-fits]
proof fn fit_implies_remaining(written: u64, entry_size: u64, capacity: u64)
    requires entry_fits_spec(written, entry_size, capacity),
    ensures space_remaining_spec(written, capacity) >= entry_size,
{}

// --- should_rotate ---

/// Empty segment with nonzero capacity does not rotate.
// r[verify wal-segment.should-rotate]
proof fn empty_no_rotate(capacity: u64)
    requires capacity > 0,
    ensures !should_rotate_spec(0, capacity),
{}

/// Full segment triggers rotation.
// r[verify wal-segment.should-rotate]
proof fn full_rotates(capacity: u64)
    ensures should_rotate_spec(capacity, capacity),
{}

/// Rotation implies no space remaining.
// r[verify wal-segment.should-rotate]
proof fn rotate_implies_zero_remaining(written: u64, capacity: u64)
    requires should_rotate_spec(written, capacity),
    ensures space_remaining_spec(written, capacity) == 0,
{}

// --- write_advance ---

/// Advancing by zero is the identity.
// r[verify wal-segment.write-advance]
proof fn advance_zero_identity(written: u64)
    ensures write_advance_spec(written, 0) == Some(written),
{}

/// Advancing by a positive amount increases the position.
// r[verify wal-segment.write-advance]
proof fn advance_increases(written: u64, entry_size: u64)
    requires
        entry_size > 0,
        write_advance_spec(written, entry_size).is_some(),
    ensures write_advance_spec(written, entry_size).unwrap() > written,
{}

// --- truncate_safe ---

/// Truncating to 0 is always safe.
// r[verify wal-segment.truncate-safe]
proof fn truncate_zero_safe(committed: u64, applied: u64)
    ensures is_truncate_safe_spec(0, committed, applied),
{}

/// Truncation beyond committed is unsafe.
// r[verify wal-segment.truncate-safe]
proof fn truncate_past_committed_unsafe(t: u64, committed: u64, applied: u64)
    requires t > committed,
    ensures !is_truncate_safe_spec(t, committed, applied),
{}

/// Truncation beyond applied is unsafe.
// r[verify wal-segment.truncate-safe]
proof fn truncate_past_applied_unsafe(t: u64, committed: u64, applied: u64)
    requires t > applied,
    ensures !is_truncate_safe_spec(t, committed, applied),
{}

/// Safe truncation is monotonic: if safe at t, safe at any t' <= t.
// r[verify wal-segment.truncate-safe]
proof fn truncate_safe_monotone(t1: u64, t2: u64, committed: u64, applied: u64)
    requires
        t1 <= t2,
        is_truncate_safe_spec(t2, committed, applied),
    ensures is_truncate_safe_spec(t1, committed, applied),
{}

// --- replay_count ---

/// Replay from the same point yields zero.
// r[verify wal-segment.replay-count]
proof fn replay_same_point(seq: u64)
    ensures replay_count_spec(seq, seq) == 0,
{}

/// Replay count equals distance when end >= checkpoint.
// r[verify wal-segment.replay-count]
proof fn replay_is_distance(checkpoint: u64, end: u64)
    requires end >= checkpoint,
    ensures replay_count_spec(checkpoint, end) == end - checkpoint,
{}

// --- checkpoint_valid ---

/// Zero checkpoint is always valid.
// r[verify wal-segment.checkpoint-valid]
proof fn zero_checkpoint_valid(committed: u64)
    ensures is_checkpoint_valid_spec(0, committed),
{}

/// Checkpoint at committed is valid.
// r[verify wal-segment.checkpoint-valid]
proof fn checkpoint_at_committed_valid(committed: u64)
    ensures is_checkpoint_valid_spec(committed, committed),
{}

/// Past committed is invalid.
// r[verify wal-segment.checkpoint-valid]
proof fn checkpoint_past_committed_invalid(checkpoint: u64, committed: u64)
    requires checkpoint > committed,
    ensures !is_checkpoint_valid_spec(checkpoint, committed),
{}

// --- Lifecycle: write → check → rotate ---

/// After writing, fit and rotate are consistent.
// r[verify wal-segment.write-advance]
// r[verify wal-segment.should-rotate]
proof fn write_then_rotate_check(written: u64, entry_size: u64, capacity: u64)
    requires
        write_advance_spec(written, entry_size).is_some(),
    ensures ({
        let new_written = write_advance_spec(written, entry_size).unwrap();
        should_rotate_spec(new_written, capacity) ==>
            space_remaining_spec(new_written, capacity) == 0
    }),
{}

/// Valid checkpoint implies truncation to checkpoint is safe
/// when applied >= committed.
// r[verify wal-segment.checkpoint-valid]
// r[verify wal-segment.truncate-safe]
proof fn valid_checkpoint_safe_truncation(
    checkpoint: u64,
    committed: u64,
    applied: u64,
)
    requires
        is_checkpoint_valid_spec(checkpoint, committed),
        applied >= committed,
    ensures is_truncate_safe_spec(checkpoint, committed, applied),
{}

} // verus!

// ─── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // --- Unit tests ---

    #[test]
    // r[verify wal-segment.is-contiguous]
    fn test_contiguous_basic() {
        assert!(is_contiguous(0, 1));
        assert!(is_contiguous(5, 6));
        assert!(!is_contiguous(5, 5));
        assert!(!is_contiguous(5, 7));
        assert!(!is_contiguous(5, 4));
    }

    #[test]
    // r[verify wal-segment.space-remaining]
    fn test_space_remaining_basic() {
        assert_eq!(space_remaining(0, 1024), 1024);
        assert_eq!(space_remaining(512, 1024), 512);
        assert_eq!(space_remaining(1024, 1024), 0);
        assert_eq!(space_remaining(2000, 1024), 0);
    }

    #[test]
    // r[verify wal-segment.entry-fits]
    fn test_entry_fits_basic() {
        assert!(entry_fits(0, 100, 1024));
        assert!(entry_fits(924, 100, 1024));
        assert!(!entry_fits(925, 100, 1024));
        assert!(entry_fits(100, 0, 100));
        assert!(!entry_fits(0, 2000, 1024));
    }

    #[test]
    // r[verify wal-segment.should-rotate]
    fn test_should_rotate_basic() {
        assert!(!should_rotate(0, 1024));
        assert!(!should_rotate(1023, 1024));
        assert!(should_rotate(1024, 1024));
        assert!(should_rotate(2000, 1024));
    }

    #[test]
    // r[verify wal-segment.write-advance]
    fn test_write_advance_basic() {
        assert_eq!(write_advance(0, 100), Some(100));
        assert_eq!(write_advance(100, 0), Some(100));
        assert_eq!(write_advance(u64::MAX - 10, 10), Some(u64::MAX));
        assert_eq!(write_advance(u64::MAX, 1), None);
        assert_eq!(write_advance(u64::MAX - 5, 10), None);
    }

    #[test]
    // r[verify wal-segment.truncate-safe]
    fn test_truncate_safe_basic() {
        assert!(is_truncate_safe(0, 10, 10));
        assert!(is_truncate_safe(5, 10, 10));
        assert!(is_truncate_safe(10, 10, 10));
        assert!(!is_truncate_safe(11, 10, 10));
        // committed=10 but applied=5: can't truncate past applied
        assert!(is_truncate_safe(5, 10, 5));
        assert!(!is_truncate_safe(6, 10, 5));
    }

    #[test]
    // r[verify wal-segment.replay-count]
    fn test_replay_count_basic() {
        assert_eq!(replay_count(0, 0), 0);
        assert_eq!(replay_count(0, 10), 10);
        assert_eq!(replay_count(5, 10), 5);
        assert_eq!(replay_count(10, 10), 0);
        assert_eq!(replay_count(10, 5), 0);
    }

    #[test]
    // r[verify wal-segment.checkpoint-valid]
    fn test_checkpoint_valid_basic() {
        assert!(is_checkpoint_valid(0, 10));
        assert!(is_checkpoint_valid(10, 10));
        assert!(!is_checkpoint_valid(11, 10));
    }

    // --- Property tests ---

    use proptest::prelude::*;

    proptest! {
        #[test]
        // r[verify wal-segment.is-contiguous]
        fn prop_contiguous_iff_successor(prev in 0..u64::MAX - 1, delta: u64) {
            let current = prev.wrapping_add(delta);
            let expected = delta == 1;
            // Can only call is_contiguous when prev < MAX
            prop_assert_eq!(is_contiguous(prev, current), expected);
        }

        #[test]
        // r[verify wal-segment.space-remaining]
        fn prop_remaining_plus_written(written: u64, capacity: u64) {
            let rem = space_remaining(written, capacity);
            if written <= capacity {
                prop_assert_eq!(rem + written, capacity);
            } else {
                prop_assert_eq!(rem, 0);
            }
        }

        #[test]
        // r[verify wal-segment.entry-fits]
        fn prop_fits_iff_sufficient_remaining(written: u64, entry_size: u64, capacity: u64) {
            let fits = entry_fits(written, entry_size, capacity);
            let remaining = space_remaining(written, capacity);
            if fits {
                prop_assert!(remaining >= entry_size);
            }
        }

        #[test]
        // r[verify wal-segment.should-rotate]
        fn prop_rotate_implies_no_remaining(written: u64, capacity: u64) {
            if should_rotate(written, capacity) {
                prop_assert_eq!(space_remaining(written, capacity), 0);
            }
        }

        #[test]
        // r[verify wal-segment.write-advance]
        fn prop_advance_monotonic(written: u64, entry_size: u64) {
            if let Some(new_written) = write_advance(written, entry_size) {
                prop_assert!(new_written >= written);
                if entry_size > 0 {
                    prop_assert!(new_written > written);
                }
            }
        }

        #[test]
        // r[verify wal-segment.truncate-safe]
        fn prop_truncate_zero_always_safe(committed: u64, applied: u64) {
            prop_assert!(is_truncate_safe(0, committed, applied));
        }

        #[test]
        // r[verify wal-segment.truncate-safe]
        fn prop_truncate_safe_monotone(
            t1 in 0..u64::MAX / 2,
            committed in 0..u64::MAX / 2,
            applied in 0..u64::MAX / 2,
        ) {
            // if safe at t1, safe at any t0 <= t1
            if is_truncate_safe(t1, committed, applied) && t1 > 0 {
                prop_assert!(is_truncate_safe(t1 - 1, committed, applied));
            }
        }

        #[test]
        // r[verify wal-segment.replay-count]
        fn prop_replay_is_saturating_sub(checkpoint: u64, end: u64) {
            let expected = if end >= checkpoint { end - checkpoint } else { 0 };
            prop_assert_eq!(replay_count(checkpoint, end), expected);
        }

        #[test]
        // r[verify wal-segment.checkpoint-valid]
        fn prop_checkpoint_valid_iff_le(checkpoint: u64, committed: u64) {
            prop_assert_eq!(is_checkpoint_valid(checkpoint, committed), checkpoint <= committed);
        }

        #[test]
        // r[verify wal-segment.write-advance]
        // r[verify wal-segment.should-rotate]
        fn prop_write_lifecycle(
            written in 0u64..1_000_000,
            entry_size in 1u64..1000,
            capacity in 1u64..1_000_000,
        ) {
            // Write, then check rotation
            if let Some(new_written) = write_advance(written, entry_size) {
                if should_rotate(new_written, capacity) {
                    prop_assert_eq!(space_remaining(new_written, capacity), 0);
                }
            }
        }
    }
}
