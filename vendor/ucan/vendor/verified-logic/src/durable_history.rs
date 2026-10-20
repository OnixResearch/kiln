//! Verified durable execution history rules.
//!
//! A durable execution history is an append-only sequence of events with
//! contiguous sequence numbers starting at 0. Once a terminal event is
//! durably recorded, later events are outside the valid durable prefix.

use vstd::prelude::*;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct DurableEvent {
    pub seq_num: usize,
    pub kind: u8,
    pub effect_id: u64,
}

pub const EVENT_WORK: u8 = 0u8;
pub const EVENT_EFFECT_DONE: u8 = 1u8;
pub const EVENT_COMPLETED: u8 = crate::job_state::COMPLETED;
pub const EVENT_FAILED: u8 = crate::job_state::FAILED;
pub const EVENT_CANCELLED: u8 = crate::job_state::CANCELLED;

pub open spec fn valid_event_kind_spec(kind: u8) -> bool {
    kind == EVENT_WORK
        || kind == EVENT_EFFECT_DONE
        || kind == EVENT_COMPLETED
        || kind == EVENT_FAILED
        || kind == EVENT_CANCELLED
}

pub open spec fn is_terminal_kind_spec(kind: u8) -> bool {
    kind == EVENT_COMPLETED || kind == EVENT_FAILED || kind == EVENT_CANCELLED
}

fn valid_event_kind(kind: u8) -> (result: bool)
    ensures result == valid_event_kind_spec(kind)
{
    kind == EVENT_WORK
        || kind == EVENT_EFFECT_DONE
        || kind == EVENT_COMPLETED
        || kind == EVENT_FAILED
        || kind == EVENT_CANCELLED
}

fn is_terminal_kind(kind: u8) -> (result: bool)
    ensures result == is_terminal_kind_spec(kind)
{
    kind == EVENT_COMPLETED || kind == EVENT_FAILED || kind == EVENT_CANCELLED
}

pub open spec fn durable_prefix_from_spec(events: Seq<DurableEvent>, from: int) -> nat
    decreases events.len() - from
{
    if from < 0 || from >= events.len() {
        from as nat
    } else if !valid_event_kind_spec(events[from].kind) {
        from as nat
    } else if events[from].seq_num != from as usize {
        from as nat
    } else if from > 0 && is_terminal_kind_spec(events[from - 1].kind) {
        from as nat
    } else {
        durable_prefix_from_spec(events, from + 1)
    }
}

// r[depends durable-execution.prefix]
/// Spec: length of the contiguous durable prefix.
pub open spec fn durable_prefix_len_spec(events: Seq<DurableEvent>) -> nat {
    durable_prefix_from_spec(events, 0)
}

// r[impl durable-execution.prefix]
/// Return the largest contiguous durable prefix length.
pub fn durable_prefix_len(events: &[DurableEvent]) -> (result: usize)
    ensures result == durable_prefix_len_spec(events@)
{
    let mut i: usize = 0;
    while i < events.len()
        invariant
            0 <= i <= events@.len(),
            durable_prefix_from_spec(events@, i as int) == durable_prefix_len_spec(events@),
        decreases events@.len() - i,
    {
        if !valid_event_kind(events[i].kind) {
            return i;
        }
        if events[i].seq_num != i {
            return i;
        }
        if i > 0 && is_terminal_kind(events[i - 1].kind) {
            return i;
        }
        proof {
            assert(valid_event_kind_spec(events@[i as int].kind));
            assert(events@[i as int].seq_num == i);
            if i > 0 {
                assert(!is_terminal_kind_spec(events@[(i - 1) as int].kind));
            }
        }
        i = i + 1;
    }
    events.len()
}

// r[depends durable-execution.history-valid]
/// Spec: entire history is a valid durable prefix.
pub open spec fn history_valid_spec(events: Seq<DurableEvent>) -> bool {
    durable_prefix_len_spec(events) == events.len()
}

// r[impl durable-execution.history-valid]
/// Check whether the whole history is valid.
pub fn history_valid(events: &[DurableEvent]) -> (result: bool)
    ensures result == history_valid_spec(events@)
{
    durable_prefix_len(events) == events.len()
}

// r[impl durable-execution.history-valid]
/// Check whether `new_event` is a valid append to `events`.
pub fn append_allowed(events: &[DurableEvent], new_event: DurableEvent) -> (result: bool)
    ensures result == append_allowed_spec(events@, new_event)
{
    let prefix = durable_prefix_len(events);
    prefix == events.len()
        && valid_event_kind(new_event.kind)
        && new_event.seq_num == events.len()
        && (events.len() == 0 || !is_terminal_kind(events[events.len() - 1].kind))
}

/// Spec: append is allowed exactly for ordered growth of a valid history.
pub open spec fn append_allowed_spec(events: Seq<DurableEvent>, new_event: DurableEvent) -> bool {
    history_valid_spec(events)
        && valid_event_kind_spec(new_event.kind)
        && new_event.seq_num == events.len()
        && (events.len() == 0 || !is_terminal_kind_spec(events[events.len() - 1].kind))
}

/// Prefix length from any valid starting point stays within the history bounds.
pub proof fn durable_prefix_from_bounded(events: Seq<DurableEvent>, from: int)
    requires 0 <= from <= events.len()
    ensures from <= durable_prefix_from_spec(events, from), durable_prefix_from_spec(events, from) <= events.len()
    decreases events.len() - from
{
    if from < events.len() {
        if valid_event_kind_spec(events[from].kind)
            && events[from].seq_num == from as usize
            && !(from > 0 && is_terminal_kind_spec(events[from - 1].kind))
        {
            durable_prefix_from_bounded(events, from + 1);
        }
    }
}

// r[verify durable-execution.prefix]
/// Prefix length never exceeds the history length.
pub proof fn durable_prefix_bounded(events: Seq<DurableEvent>)
    ensures durable_prefix_len_spec(events) <= events.len()
{
    durable_prefix_from_bounded(events, 0);
}

// r[verify durable-execution.history-valid]
/// Empty history is valid.
proof fn empty_history_valid()
    ensures history_valid_spec(seq![])
{}

proof fn history_valid_pointwise_from(events: Seq<DurableEvent>, idx: int, from: int)
    requires
        0 <= from <= idx < events.len(),
        durable_prefix_from_spec(events, from) == events.len(),
    ensures
        valid_event_kind_spec(events[idx].kind),
        events[idx].seq_num == idx as usize,
        idx == 0 || !is_terminal_kind_spec(events[idx - 1].kind),
    decreases idx - from
{
    if from < idx {
        assert(valid_event_kind_spec(events[from].kind));
        assert(events[from].seq_num == from as usize);
        if from > 0 {
            assert(!is_terminal_kind_spec(events[from - 1].kind));
        }
        assert(durable_prefix_from_spec(events, from + 1) == events.len());
        history_valid_pointwise_from(events, idx, from + 1);
    } else {
        assert(valid_event_kind_spec(events[idx].kind));
        assert(events[idx].seq_num == idx as usize);
        if idx > 0 {
            assert(!is_terminal_kind_spec(events[idx - 1].kind));
        }
    }
}

/// Pointwise facts available from a valid history.
pub proof fn history_valid_pointwise(events: Seq<DurableEvent>, idx: int)
    requires history_valid_spec(events), 0 <= idx < events.len()
    ensures
        valid_event_kind_spec(events[idx].kind),
        events[idx].seq_num == idx as usize,
        idx == 0 || !is_terminal_kind_spec(events[idx - 1].kind),
{
    history_valid_pointwise_from(events, idx, 0);
}


} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn work(seq_num: usize) -> DurableEvent {
        DurableEvent {
            seq_num,
            kind: EVENT_WORK,
            effect_id: 0,
        }
    }

    fn effect(seq_num: usize, effect_id: u64) -> DurableEvent {
        DurableEvent {
            seq_num,
            kind: EVENT_EFFECT_DONE,
            effect_id,
        }
    }

    fn completed(seq_num: usize) -> DurableEvent {
        DurableEvent {
            seq_num,
            kind: EVENT_COMPLETED,
            effect_id: 0,
        }
    }

    // r[verify durable-execution.prefix]
    #[test]
    fn test_durable_prefix_len() {
        assert_eq!(durable_prefix_len(&[]), 0);
        assert_eq!(durable_prefix_len(&[work(0), work(1), work(2)]), 3);
        assert_eq!(durable_prefix_len(&[work(0), work(1), work(3)]), 2);
        assert_eq!(durable_prefix_len(&[completed(0), work(1)]), 1);
        assert_eq!(durable_prefix_len(&[effect(0, 10), effect(1, 11)]), 2);
    }

    // r[verify durable-execution.history-valid]
    #[test]
    fn test_history_valid_and_append_allowed() {
        let history = vec![work(0), effect(1, 7)];
        assert!(history_valid(&history));
        assert!(append_allowed(&history, completed(2)));
        assert!(!append_allowed(&history, work(3)));
        assert!(!append_allowed(&[completed(0)], work(1)));
    }

    proptest! {
        // r[verify durable-execution.prefix]
        #[test]
        fn prop_prefix_of_contiguous_work_history(len in 0usize..40) {
            let events: Vec<DurableEvent> = (0..len).map(work).collect();
            prop_assert_eq!(durable_prefix_len(&events), len);
        }

        // r[verify durable-execution.history-valid]
        #[test]
        fn prop_duplicate_or_gap_breaks_validity(len in 2usize..40, bad in 1usize..39) {
            let mut events: Vec<DurableEvent> = (0..len).map(work).collect();
            let idx = bad.min(len - 1);
            events[idx].seq_num = events[idx - 1].seq_num;
            prop_assert!(!history_valid(&events));
            prop_assert!(durable_prefix_len(&events) <= idx);
        }

        // r[verify durable-execution.history-valid]
        #[test]
        fn prop_terminal_blocks_append(tail in 0u64..1000) {
            let history = vec![work(0), completed(1)];
            let new_event = DurableEvent { seq_num: 2, kind: EVENT_WORK, effect_id: tail };
            prop_assert!(!append_allowed(&history, new_event));
        }
    }
}
