//! Verified terminal-state recovery for durable execution.
//!
//! A durable terminal event fixes the recovered outcome. Recovery reads the
//! last event in the valid durable prefix and never reopens a terminal run.

use vstd::prelude::*;

use crate::durable_history::DurableEvent;
use crate::durable_history::EVENT_CANCELLED;
use crate::durable_history::EVENT_COMPLETED;
use crate::durable_history::EVENT_FAILED;
use crate::durable_history::durable_prefix_len;

verus! {

pub open spec fn terminal_status_spec(events: Seq<DurableEvent>) -> Option<u8> {
    let prefix = crate::durable_history::durable_prefix_len_spec(events);
    if prefix == 0 {
        None
    } else {
        let last = events[prefix - 1];
        if crate::durable_history::is_terminal_kind_spec(last.kind) {
            Some(last.kind)
        } else {
            None
        }
    }
}

// r[depends durable-execution.terminal]
/// Spec: execution is terminal when the durable prefix ends in a terminal event.
pub open spec fn is_terminal_execution_spec(events: Seq<DurableEvent>) -> bool {
    match terminal_status_spec(events) {
        Some(_) => true,
        None => false,
    }
}

// r[impl durable-execution.terminal]
/// Check whether durable history already fixes a terminal outcome.
pub fn is_terminal_execution(events: &[DurableEvent]) -> (result: bool)
    ensures result == is_terminal_execution_spec(events@)
{
    recovered_status(events).is_some()
}

// r[verify durable-execution.terminal]
/// Once durable history is terminal, appending any later event is rejected.
pub proof fn post_terminal_append_rejected(events: Seq<DurableEvent>, new_event: DurableEvent)
    requires is_terminal_execution_spec(events)
    ensures !crate::durable_history::append_allowed_spec(events, new_event)
{
}

// r[depends durable-execution.terminal-recovery]
/// Spec: recovered terminal status, if one exists.
pub open spec fn recovered_status_spec(events: Seq<DurableEvent>) -> Option<u8> {
    terminal_status_spec(events)
}

// r[impl durable-execution.terminal-recovery]
/// Recover the durable terminal status, if one exists.
pub fn recovered_status(events: &[DurableEvent]) -> (result: Option<u8>)
    ensures result == recovered_status_spec(events@)
{
    let prefix = durable_prefix_len(events);
    proof {
        crate::durable_history::durable_prefix_bounded(events@);
        assert(prefix <= events@.len());
    }
    if prefix == 0 {
        None
    } else {
        let last = events[prefix - 1];
        if last.kind == EVENT_COMPLETED || last.kind == EVENT_FAILED || last.kind == EVENT_CANCELLED {
            Some(last.kind)
        } else {
            None
        }
    }
}

// r[verify durable-execution.terminal-recovery]
/// Recovery reports the same durable terminal outcome fixed in the prefix.
pub proof fn terminal_recovery_reproduces_status(events: Seq<DurableEvent>)
    ensures recovered_status_spec(events) == terminal_status_spec(events)
{
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::durable_history::DurableEvent;
    use crate::durable_history::EVENT_CANCELLED;
    use crate::durable_history::EVENT_COMPLETED;
    use crate::durable_history::EVENT_FAILED;
    use crate::durable_history::EVENT_WORK;

    fn work(seq_num: usize) -> DurableEvent {
        DurableEvent {
            seq_num,
            kind: EVENT_WORK,
            effect_id: 0,
        }
    }

    fn completed(seq_num: usize) -> DurableEvent {
        DurableEvent {
            seq_num,
            kind: EVENT_COMPLETED,
            effect_id: 0,
        }
    }

    fn cancelled(seq_num: usize) -> DurableEvent {
        DurableEvent {
            seq_num,
            kind: EVENT_CANCELLED,
            effect_id: 0,
        }
    }

    fn failed(seq_num: usize) -> DurableEvent {
        DurableEvent {
            seq_num,
            kind: EVENT_FAILED,
            effect_id: 0,
        }
    }

    fn random_event((seq_num, kind, effect_id): (usize, u8, u64)) -> DurableEvent {
        DurableEvent {
            seq_num,
            kind,
            effect_id,
        }
    }

    // r[verify durable-execution.terminal]
    #[test]
    fn test_is_terminal_execution() {
        assert!(!is_terminal_execution(&[]));
        assert!(!is_terminal_execution(&[work(0), work(1)]));
        assert!(is_terminal_execution(&[work(0), completed(1)]));
        assert!(is_terminal_execution(&[cancelled(0)]));
        assert!(is_terminal_execution(&[failed(0)]));
    }

    // r[verify durable-execution.terminal-recovery]
    #[test]
    fn test_recovered_status() {
        assert_eq!(recovered_status(&[]), None);
        assert_eq!(recovered_status(&[work(0)]), None);
        assert_eq!(recovered_status(&[work(0), completed(1)]), Some(EVENT_COMPLETED));
        assert_eq!(recovered_status(&[cancelled(0)]), Some(EVENT_CANCELLED));
        assert_eq!(recovered_status(&[failed(0)]), Some(EVENT_FAILED));
    }

    proptest! {
        // r[verify durable-execution.terminal]
        #[test]
        fn prop_completed_prefix_is_terminal(prefix in 0usize..20) {
            let mut events: Vec<DurableEvent> = (0..prefix).map(work).collect();
            events.push(completed(prefix));
            prop_assert!(is_terminal_execution(&events));
        }

        // r[verify durable-execution.terminal-recovery]
        #[test]
        fn prop_terminal_status_matches_last_terminal(prefix in 0usize..20) {
            let mut events: Vec<DurableEvent> = (0..prefix).map(work).collect();
            events.push(completed(prefix));
            prop_assert_eq!(recovered_status(&events), Some(EVENT_COMPLETED));
        }

        // r[verify durable-execution.terminal]
        #[test]
        fn prop_random_histories_post_terminal_append_rejected(
            prefix in 0usize..12,
            terminal_kind in prop_oneof![Just(EVENT_COMPLETED), Just(EVENT_FAILED), Just(EVENT_CANCELLED)],
            tail in proptest::collection::vec((0usize..12, 0u8..8, 0u64..8), 0..8),
            new_seq in 0usize..20,
            new_kind in 0u8..8,
            effect_id in 0u64..8,
        ) {
            let mut events: Vec<DurableEvent> = (0..prefix).map(work).collect();
            events.push(DurableEvent { seq_num: prefix, kind: terminal_kind, effect_id: 0 });
            events.extend(tail.into_iter().map(random_event));
            let new_event = DurableEvent { seq_num: new_seq, kind: new_kind, effect_id };
            prop_assert!(is_terminal_execution(&events));
            prop_assert!(!crate::durable_history::append_allowed(&events, new_event));
        }

        // r[verify durable-execution.terminal-recovery]
        #[test]
        fn prop_random_histories_recover_terminal_prefix(
            prefix in 0usize..12,
            terminal_kind in prop_oneof![Just(EVENT_COMPLETED), Just(EVENT_FAILED), Just(EVENT_CANCELLED)],
            tail in proptest::collection::vec((0usize..12, 0u8..8, 0u64..8), 0..8),
        ) {
            let mut events: Vec<DurableEvent> = (0..prefix).map(work).collect();
            events.push(DurableEvent { seq_num: prefix, kind: terminal_kind, effect_id: 0 });
            events.extend(tail.into_iter().map(random_event));
            prop_assert_eq!(recovered_status(&events), Some(terminal_kind));
        }
    }
}
