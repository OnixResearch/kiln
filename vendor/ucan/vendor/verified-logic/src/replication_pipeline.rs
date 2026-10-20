//! Verified replication pipeline / flow control for Raft.
//!
//! A Raft leader can pipeline `AppendEntries` without waiting for each
//! response. The inflight window tracks how many entries have been sent
//! but not yet acknowledged. Flow control caps the window to prevent
//! overwhelming slow followers.

use vstd::prelude::*;

verus! {

// --- inflight_count ---

// r[depends replication-pipeline.inflight-count]
/// Spec: how many entries are in flight (sent but unacknowledged)?
///
/// next_index points to the next entry to send.
/// match_index is the last acknowledged entry.
/// inflight = next_index - match_index - 1 (the entries between them).
pub open spec fn inflight_count_spec(
    next_index: u64,
    match_index: u64,
) -> u64 {
    if next_index > match_index + 1 {
        (next_index - match_index - 1) as u64
    } else {
        0u64
    }
}

// r[impl replication-pipeline.inflight-count]
/// Compute the number of inflight entries.
pub fn inflight_count(
    next_index: u64,
    match_index: u64,
) -> (result: u64)
    ensures result == inflight_count_spec(next_index, match_index)
{
    if next_index > 0 && next_index - 1 > match_index {
        next_index - match_index - 1
    } else {
        0
    }
}

// --- can_send_more ---

// r[depends replication-pipeline.can-send-more]
/// Spec: can the leader send more entries to this follower?
pub open spec fn can_send_more_spec(
    next_index: u64,
    match_index: u64,
    max_inflight: u64,
) -> bool {
    inflight_count_spec(next_index, match_index) < max_inflight
}

// r[impl replication-pipeline.can-send-more]
/// Check if the pipeline has capacity.
pub fn can_send_more(
    next_index: u64,
    match_index: u64,
    max_inflight: u64,
) -> (result: bool)
    ensures result == can_send_more_spec(next_index, match_index, max_inflight)
{
    inflight_count(next_index, match_index) < max_inflight
}

// --- window_after_send ---

// r[depends replication-pipeline.window-after-send]
/// Spec: new `next_index` after sending entries.
pub open spec fn window_after_send_spec(
    next_index: u64,
    entries_sent: u64,
) -> u64
{
    (next_index + entries_sent) as u64
}

// r[impl replication-pipeline.window-after-send]
/// Compute the new `next_index` after sending.
pub fn window_after_send(
    next_index: u64,
    entries_sent: u64,
) -> (result: u64)
    requires next_index as int + entries_sent as int <= u64::MAX as int
    ensures result == window_after_send_spec(next_index, entries_sent)
{
    next_index + entries_sent
}

// --- window_after_ack ---

// r[depends replication-pipeline.window-after-ack]
/// Spec: new `match_index` after receiving acknowledgment.
pub open spec fn window_after_ack_spec(
    match_index: u64,
    new_match: u64,
) -> u64 {
    if new_match > match_index {
        new_match
    } else {
        match_index
    }
}

// r[impl replication-pipeline.window-after-ack]
/// Compute the new `match_index` after an ack.
pub fn window_after_ack(
    match_index: u64,
    new_match: u64,
) -> (result: u64)
    ensures result == window_after_ack_spec(match_index, new_match)
{
    if new_match > match_index {
        new_match
    } else {
        match_index
    }
}

// --- Safety proofs ---

// r[impl replication-pipeline.empty-window]
// r[verify replication-pipeline.empty-window]
/// Fully acknowledged pipeline has zero inflight.
proof fn empty_pipeline(match_index: u64)
    ensures inflight_count_spec((match_index + 1) as u64, match_index) == 0
{}

// r[verify replication-pipeline.empty-window]
/// Empty pipeline can always send (if max > 0).
proof fn empty_can_send(match_index: u64, max_inflight: u64)
    requires max_inflight > 0
    ensures can_send_more_spec((match_index + 1) as u64, match_index, max_inflight)
{}

// r[impl replication-pipeline.full-window-blocks]
// r[verify replication-pipeline.full-window-blocks]
/// Full pipeline blocks further sends.
proof fn full_blocks(next_index: u64, match_index: u64, max_inflight: u64)
    requires inflight_count_spec(next_index, match_index) >= max_inflight
    ensures !can_send_more_spec(next_index, match_index, max_inflight)
{}

// r[verify replication-pipeline.window-after-send]
/// Sending increases inflight count.
proof fn send_increases_inflight(
    next_index: u64,
    match_index: u64,
    entries_sent: u64,
)
    requires
        entries_sent > 0,
        next_index as int + entries_sent as int <= u64::MAX as int,
        next_index > match_index,
    ensures
        inflight_count_spec(
            window_after_send_spec(next_index, entries_sent),
            match_index
        ) as int
        == inflight_count_spec(next_index, match_index) as int
           + entries_sent as int
{}

// r[verify replication-pipeline.window-after-ack]
/// Ack decreases or maintains inflight count.
proof fn ack_decreases_inflight(
    next_index: u64,
    old_match: u64,
    new_match: u64,
)
    requires
        new_match >= old_match,
        next_index > old_match + 1,
        next_index > new_match + 1,
    ensures
        inflight_count_spec(next_index, new_match)
        <= inflight_count_spec(next_index, old_match)
{}

// r[verify replication-pipeline.window-after-ack]
/// Full ack empties the pipeline.
proof fn full_ack_empties(next_index: u64, match_index: u64)
    requires next_index > 0
    ensures inflight_count_spec(next_index, (next_index - 1) as u64) == 0
{}

// r[verify replication-pipeline.inflight-count]
/// Inflight is zero when next == match + 1.
proof fn adjacent_is_zero(match_index: u64)
    ensures inflight_count_spec((match_index + 1) as u64, match_index) == 0
{}

// r[verify replication-pipeline.can-send-more]
/// Zero max inflight always blocks.
proof fn zero_max_blocks(next_index: u64, match_index: u64)
    ensures !can_send_more_spec(next_index, match_index, 0)
{}

// r[verify replication-pipeline.window-after-ack]
/// Ack is monotonic.
proof fn ack_monotonic(match_index: u64, new_match: u64)
    ensures window_after_ack_spec(match_index, new_match) >= match_index
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify replication-pipeline.inflight-count]
    #[test]
    fn test_inflight_count() {
        assert_eq!(inflight_count(11, 5), 5); // 6,7,8,9,10 inflight
        assert_eq!(inflight_count(6, 5), 0); // nothing inflight
        assert_eq!(inflight_count(5, 5), 0); // nothing (next <= match+1)
        assert_eq!(inflight_count(1, 0), 0); // initialized state
    }

    // r[verify replication-pipeline.can-send-more]
    #[test]
    fn test_can_send_more() {
        assert!(can_send_more(6, 5, 10)); // 0 inflight < 10
        assert!(can_send_more(10, 5, 10)); // 4 inflight < 10
        assert!(!can_send_more(16, 5, 10)); // 10 inflight = max
        assert!(!can_send_more(20, 5, 10)); // 14 inflight > max
    }

    // r[verify replication-pipeline.window-after-send]
    #[test]
    fn test_window_after_send() {
        assert_eq!(window_after_send(6, 5), 11);
        assert_eq!(window_after_send(1, 0), 1);
    }

    // r[verify replication-pipeline.window-after-ack]
    #[test]
    fn test_window_after_ack() {
        assert_eq!(window_after_ack(5, 8), 8); // advance
        assert_eq!(window_after_ack(5, 3), 5); // stale ack
        assert_eq!(window_after_ack(5, 5), 5); // same
    }

    proptest! {
        // r[verify replication-pipeline.empty-window]
        #[test]
        fn prop_adjacent_is_zero(match_index in 0u64..u64::MAX - 1) {
            prop_assert_eq!(inflight_count(match_index + 1, match_index), 0);
        }

        // r[verify replication-pipeline.full-window-blocks]
        #[test]
        fn prop_full_blocks(
            match_index in 0u64..1000,
            max in 1u64..100,
        ) {
            let next = match_index + 1 + max; // exactly full
            prop_assert!(!can_send_more(next, match_index, max));
        }

        // r[verify replication-pipeline.window-after-ack]
        #[test]
        fn prop_ack_monotonic(old_match: u64, new_match: u64) {
            prop_assert!(window_after_ack(old_match, new_match) >= old_match);
        }

        // r[verify replication-pipeline.can-send-more]
        #[test]
        fn prop_zero_max_blocks(next: u64, match_idx: u64) {
            prop_assert!(!can_send_more(next, match_idx, 0));
        }

        // r[verify replication-pipeline.inflight-count]
        #[test]
        fn prop_send_then_ack_roundtrip(
            match_index in 0u64..1000,
            entries in 1u64..100,
        ) {
            let next = match_index + 1;
            let after_send = window_after_send(next, entries);
            let new_match = match_index + entries;
            prop_assert_eq!(inflight_count(after_send, new_match), 0);
        }
    }
}
