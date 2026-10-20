//! Verified checkpoint and resume decisions for durable execution.
//!
//! Checkpoints advance only within the contiguous durable prefix. Recovery
//! resumes at the first not-yet-integrated durable step, unless durable
//! history already fixes a terminal outcome.

use vstd::prelude::*;

use crate::durable_history::DurableEvent;
use crate::durable_history::durable_prefix_len;

verus! {

pub open spec fn checkpoint_valid_spec(old_checkpoint: usize, events: Seq<DurableEvent>) -> bool {
    old_checkpoint <= crate::durable_history::durable_prefix_len_spec(events)
}

// r[depends durable-execution.checkpoint-advance]
// r[impl durable-execution.checkpoint-prefix]
/// Spec: checkpoint advances to the durable prefix when the old checkpoint is valid.
pub open spec fn advance_checkpoint_spec(old_checkpoint: usize, events: Seq<DurableEvent>) -> usize {
    let prefix = crate::durable_history::durable_prefix_len_spec(events);
    if old_checkpoint <= prefix {
        prefix as usize
    } else {
        old_checkpoint
    }
}

// r[impl durable-execution.checkpoint-advance]
// r[impl durable-execution.checkpoint-prefix]
/// Advance the checkpoint as far as the contiguous durable prefix allows.
pub fn advance_checkpoint(old_checkpoint: usize, events: &[DurableEvent]) -> (result: usize)
    ensures result == advance_checkpoint_spec(old_checkpoint, events@)
{
    let prefix = durable_prefix_len(events);
    if old_checkpoint <= prefix {
        prefix
    } else {
        old_checkpoint
    }
}

// r[verify durable-execution.checkpoint-advance]
/// Checkpoint advancement never rewinds.
pub proof fn advance_checkpoint_never_rewinds(old_checkpoint: usize, events: Seq<DurableEvent>)
    requires events.len() <= usize::MAX as int
    ensures old_checkpoint <= advance_checkpoint_spec(old_checkpoint, events)
{
    let prefix = crate::durable_history::durable_prefix_len_spec(events);
    crate::durable_history::durable_prefix_bounded(events);
    assert(prefix <= events.len());
    if old_checkpoint <= prefix {
        assert(advance_checkpoint_spec(old_checkpoint, events) == prefix as usize);
    } else {
        assert(advance_checkpoint_spec(old_checkpoint, events) == old_checkpoint);
    }
}

// r[verify durable-execution.checkpoint-prefix]
/// Valid checkpoint advancement stays within the durable prefix.
pub proof fn valid_checkpoint_never_exceeds_prefix(old_checkpoint: usize, events: Seq<DurableEvent>)
    requires
        events.len() <= usize::MAX as int,
        checkpoint_valid_spec(old_checkpoint, events),
    ensures advance_checkpoint_spec(old_checkpoint, events) as int <= crate::durable_history::durable_prefix_len_spec(events)
{
    let prefix = crate::durable_history::durable_prefix_len_spec(events);
    crate::durable_history::durable_prefix_bounded(events);
    assert(prefix <= events.len());
    assert(old_checkpoint <= prefix);
    assert(advance_checkpoint_spec(old_checkpoint, events) == prefix as usize);
}

// r[verify durable-execution.checkpoint-prefix]
/// Valid checkpoint advancement stops exactly at the first non-durable step.
pub proof fn valid_checkpoint_stops_at_gap(old_checkpoint: usize, events: Seq<DurableEvent>)
    requires
        events.len() <= usize::MAX as int,
        checkpoint_valid_spec(old_checkpoint, events),
    ensures advance_checkpoint_spec(old_checkpoint, events) as int == crate::durable_history::durable_prefix_len_spec(events)
{
    let prefix = crate::durable_history::durable_prefix_len_spec(events);
    crate::durable_history::durable_prefix_bounded(events);
    assert(prefix <= events.len());
    assert(old_checkpoint <= prefix);
    assert(advance_checkpoint_spec(old_checkpoint, events) == prefix as usize);
}

// r[depends durable-execution.resume-step]
// r[impl durable-execution.resume-bounds]
/// Spec: recovery resumes at the durable prefix, unless the run is terminal.
pub open spec fn resume_step_spec(old_checkpoint: usize, events: Seq<DurableEvent>) -> Option<usize> {
    match crate::durable_terminal::recovered_status_spec(events) {
        Some(_) => None,
        None => Some(advance_checkpoint_spec(old_checkpoint, events)),
    }
}

// r[impl durable-execution.resume-step]
// r[impl durable-execution.resume-bounds]
/// Return the next resumable step, or `None` if durable history is terminal.
pub fn resume_step(old_checkpoint: usize, events: &[DurableEvent]) -> (result: Option<usize>)
    ensures result == resume_step_spec(old_checkpoint, events@)
{
    if crate::durable_terminal::is_terminal_execution(events) {
        None
    } else {
        Some(advance_checkpoint(old_checkpoint, events))
    }
}

// r[verify durable-execution.resume-step]
/// Terminal durable histories never produce a resumable step.
pub proof fn terminal_history_has_no_resume(old_checkpoint: usize, events: Seq<DurableEvent>)
    requires crate::durable_terminal::is_terminal_execution_spec(events)
    ensures resume_step_spec(old_checkpoint, events) == None::<usize>
{
}

// r[verify durable-execution.resume-bounds]
/// Resume from a valid checkpoint never rewinds below that checkpoint.
pub proof fn valid_resume_never_rewinds(old_checkpoint: usize, events: Seq<DurableEvent>)
    requires
        events.len() <= usize::MAX as int,
        checkpoint_valid_spec(old_checkpoint, events),
        !crate::durable_terminal::is_terminal_execution_spec(events),
    ensures match resume_step_spec(old_checkpoint, events) {
        Some(step) => old_checkpoint <= step,
        None => false,
    }
{
    advance_checkpoint_never_rewinds(old_checkpoint, events);
    assert(resume_step_spec(old_checkpoint, events) == Some(advance_checkpoint_spec(old_checkpoint, events)));
}

// r[verify durable-execution.resume-bounds]
/// Resume from a valid checkpoint never skips beyond the durable prefix.
pub proof fn valid_resume_stays_within_prefix(old_checkpoint: usize, events: Seq<DurableEvent>)
    requires
        events.len() <= usize::MAX as int,
        checkpoint_valid_spec(old_checkpoint, events),
        !crate::durable_terminal::is_terminal_execution_spec(events),
    ensures match resume_step_spec(old_checkpoint, events) {
        Some(step) => step as int <= crate::durable_history::durable_prefix_len_spec(events),
        None => false,
    }
{
    valid_checkpoint_never_exceeds_prefix(old_checkpoint, events);
    assert(resume_step_spec(old_checkpoint, events) == Some(advance_checkpoint_spec(old_checkpoint, events)));
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::durable_history::DurableEvent;
    use crate::durable_history::EVENT_CANCELLED;
    use crate::durable_history::EVENT_COMPLETED;
    use crate::durable_history::EVENT_EFFECT_DONE;
    use crate::durable_history::EVENT_FAILED;
    use crate::durable_history::EVENT_WORK;
    use crate::durable_history::durable_prefix_len;
    use crate::durable_terminal::is_terminal_execution;

    fn work(seq_num: usize) -> DurableEvent {
        DurableEvent {
            seq_num,
            kind: EVENT_WORK,
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

    // r[verify durable-execution.checkpoint-advance]
    #[test]
    fn test_advance_checkpoint() {
        let events = vec![work(0), work(1), work(2)];
        assert_eq!(advance_checkpoint(0, &events), 3);
        assert_eq!(advance_checkpoint(2, &events), 3);
        assert_eq!(advance_checkpoint(3, &events), 3);
        assert_eq!(advance_checkpoint(5, &events), 5);
    }

    // r[verify durable-execution.resume-step]
    #[test]
    fn test_resume_step() {
        let events = vec![work(0), work(1), work(2)];
        assert_eq!(resume_step(0, &events), Some(3));
        assert_eq!(resume_step(2, &events), Some(3));
        assert_eq!(resume_step(0, &[]), Some(0));
        assert_eq!(resume_step(5, &[]), Some(5));

        let terminal = vec![work(0), DurableEvent {
            seq_num: 1,
            kind: EVENT_CANCELLED,
            effect_id: 0,
        }];
        assert_eq!(resume_step(0, &terminal), None);
    }

    proptest! {
        // r[verify durable-execution.checkpoint-prefix]
        #[test]
        fn prop_valid_checkpoint_advances_to_prefix(len in 0usize..40, old in 0usize..40) {
            let events: Vec<DurableEvent> = (0..len).map(work).collect();
            let prefix = durable_prefix_len(&events);
            let checkpoint = old.min(prefix);
            prop_assert_eq!(advance_checkpoint(checkpoint, &events), prefix);
        }

        // r[verify durable-execution.resume-bounds]
        #[test]
        fn prop_nonterminal_resume_is_prefix(len in 0usize..40) {
            let events: Vec<DurableEvent> = (0..len).map(work).collect();
            prop_assert_eq!(resume_step(0, &events), Some(len));
        }

        // r[verify durable-execution.checkpoint-prefix]
        #[test]
        fn prop_random_histories_checkpoint_rules(
            events in proptest::collection::vec((0usize..20, 0u8..8, 0u64..8), 0..20),
            old in 0usize..25,
        ) {
            let events: Vec<DurableEvent> = events.into_iter().map(random_event).collect();
            let prefix = durable_prefix_len(&events);
            let advanced = advance_checkpoint(old, &events);
            if old <= prefix {
                prop_assert_eq!(advanced, prefix);
                prop_assert!(advanced <= prefix);
            } else {
                prop_assert_eq!(advanced, old);
            }
        }

        // r[verify durable-execution.resume-bounds]
        #[test]
        fn prop_random_histories_resume_rules(
            events in proptest::collection::vec((0usize..20, 0u8..8, 0u64..8), 0..20),
            old in 0usize..25,
        ) {
            let events: Vec<DurableEvent> = events.into_iter().map(random_event).collect();
            let prefix = durable_prefix_len(&events);
            let resumed = resume_step(old, &events);
            if is_terminal_execution(&events) {
                prop_assert_eq!(resumed, None);
            } else {
                prop_assert_eq!(resumed, Some(advance_checkpoint(old, &events)));
                if old <= prefix {
                    let step = resumed.unwrap();
                    prop_assert!(step >= old);
                    prop_assert!(step <= prefix);
                }
            }
        }

        // r[verify durable-execution.resume-step]
        #[test]
        fn prop_terminal_history_never_resumes(
            prefix in 0usize..12,
            terminal_kind in prop_oneof![Just(EVENT_COMPLETED), Just(EVENT_FAILED), Just(EVENT_CANCELLED)],
            tail in proptest::collection::vec((0usize..12, 0u8..8, 0u64..8), 0..8),
            old in 0usize..20,
        ) {
            let mut events: Vec<DurableEvent> = (0..prefix).map(work).collect();
            events.push(DurableEvent { seq_num: prefix, kind: terminal_kind, effect_id: 0 });
            events.extend(tail.into_iter().map(random_event));
            prop_assert!(is_terminal_execution(&events));
            prop_assert_eq!(resume_step(old, &events), None);
        }

        // r[verify durable-execution.resume-bounds]
        #[test]
        fn prop_random_histories_invalid_checkpoint_can_survive_nonterminal_resume(
            events in proptest::collection::vec((0usize..20, 0u8..8, 0u64..8), 0..20),
            old_extra in 1usize..10,
        ) {
            let events: Vec<DurableEvent> = events.into_iter().map(random_event).collect();
            if !is_terminal_execution(&events) {
                let prefix = durable_prefix_len(&events);
                let old = prefix + old_extra;
                prop_assert_eq!(advance_checkpoint(old, &events), old);
                prop_assert_eq!(resume_step(old, &events), Some(old));
            }
        }
    }
}
