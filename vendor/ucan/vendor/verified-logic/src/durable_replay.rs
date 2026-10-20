//! Verified replay decisions for durable execution.
//!
//! Replay reads only the contiguous durable prefix. Durable effect markers
//! suppress fresh execution for the same `(seq_num, effect_id)` pair.

use vstd::prelude::*;

use crate::durable_history::DurableEvent;
use crate::durable_history::EVENT_EFFECT_DONE;
use crate::durable_history::durable_prefix_len;

verus! {

// r[depends durable-execution.replayable-event]
/// Spec: replay returns an event only inside the durable prefix.
pub open spec fn replayable_event_at_spec(events: Seq<DurableEvent>, index: int) -> Option<DurableEvent> {
    if 0 <= index < crate::durable_history::durable_prefix_len_spec(events) {
        Some(events[index])
    } else {
        None
    }
}

// r[impl durable-execution.replayable-event]
/// Return the replayable durable event at `index`, if any.
pub fn replayable_event_at(events: &[DurableEvent], index: usize) -> (result: Option<DurableEvent>)
    ensures result == replayable_event_at_spec(events@, index as int)
{
    let prefix = durable_prefix_len(events);
    proof {
        crate::durable_history::durable_prefix_bounded(events@);
        assert(prefix <= events@.len());
    }
    if index < prefix {
        Some(events[index])
    } else {
        None
    }
}

// r[verify durable-execution.replayable-event]
/// Replay within the durable prefix returns the recorded event.
pub proof fn replay_inside_prefix_returns_event(events: Seq<DurableEvent>, index: int)
    requires 0 <= index < crate::durable_history::durable_prefix_len_spec(events)
    ensures replayable_event_at_spec(events, index) == Some(events[index])
{
}

// r[verify durable-execution.replayable-event]
/// Replay at or beyond the durable prefix is rejected.
pub proof fn replay_outside_prefix_rejected(events: Seq<DurableEvent>, index: int)
    requires index < 0 || crate::durable_history::durable_prefix_len_spec(events) <= index
    ensures replayable_event_at_spec(events, index) == None::<DurableEvent>
{
}

pub open spec fn effect_completed_spec(events: Seq<DurableEvent>, seq_num: usize, effect_id: u64) -> bool {
    exists|i: int| {
        &&& 0 <= i < crate::durable_history::durable_prefix_len_spec(events)
        &&& events[i].seq_num == seq_num
        &&& events[i].kind == EVENT_EFFECT_DONE
        &&& events[i].effect_id == effect_id
    }
}

// r[depends durable-execution.effect-dedup]
/// Spec: an effect should execute only if no matching durable completion exists.
pub open spec fn should_execute_effect_spec(events: Seq<DurableEvent>, seq_num: usize, effect_id: u64) -> bool {
    !effect_completed_spec(events, seq_num, effect_id)
}

// r[impl durable-execution.effect-dedup]
/// Decide whether a fresh effect execution is still allowed.
pub fn should_execute_effect(events: &[DurableEvent], seq_num: usize, effect_id: u64) -> (result: bool)
    ensures result == should_execute_effect_spec(events@, seq_num, effect_id)
{
    let prefix = durable_prefix_len(events);
    let mut i: usize = 0;
    proof {
        crate::durable_history::durable_prefix_bounded(events@);
        assert(prefix <= events@.len());
    }
    while i < prefix
        invariant
            0 <= i <= prefix,
            prefix == crate::durable_history::durable_prefix_len_spec(events@),
            prefix <= events@.len(),
            forall|k: int| 0 <= k < i as int ==> !(events@[k].seq_num == seq_num
                && events@[k].kind == EVENT_EFFECT_DONE
                && events@[k].effect_id == effect_id),
        decreases prefix - i,
    {
        if events[i].seq_num == seq_num
            && events[i].kind == EVENT_EFFECT_DONE
            && events[i].effect_id == effect_id
        {
            return false;
        }
        i = i + 1;
    }
    true
}

// r[verify durable-execution.effect-dedup]
/// A matching durable effect marker suppresses fresh execution.
pub proof fn durable_effect_marker_blocks_reexecution(events: Seq<DurableEvent>, seq_num: usize, effect_id: u64)
    requires effect_completed_spec(events, seq_num, effect_id)
    ensures !should_execute_effect_spec(events, seq_num, effect_id)
{
}

// r[verify durable-execution.effect-dedup]
/// Appending a valid durable marker for one effect id does not change decisions for another id.
pub proof fn unrelated_effect_ids_do_not_block(events: Seq<DurableEvent>, seq_num: usize, completed_id: u64, other_id: u64)
    requires
        events.len() <= usize::MAX as int,
        completed_id != other_id,
        crate::durable_history::history_valid_spec(events),
        crate::durable_history::history_valid_spec(events.push(DurableEvent {
            seq_num: events.len() as usize,
            kind: EVENT_EFFECT_DONE,
            effect_id: completed_id,
        })),
    ensures should_execute_effect_spec(events.push(DurableEvent {
        seq_num: events.len() as usize,
        kind: EVENT_EFFECT_DONE,
        effect_id: completed_id,
    }), seq_num, other_id) == should_execute_effect_spec(events, seq_num, other_id)
{
    let marker = DurableEvent {
        seq_num: events.len() as usize,
        kind: EVENT_EFFECT_DONE,
        effect_id: completed_id,
    };
    let extended = events.push(marker);
    assert(crate::durable_history::durable_prefix_len_spec(events) == events.len());
    assert(crate::durable_history::durable_prefix_len_spec(extended) == extended.len());

    assert(effect_completed_spec(extended, seq_num, other_id) ==> effect_completed_spec(events, seq_num, other_id)) by {
        if effect_completed_spec(extended, seq_num, other_id) {
            let i = choose|i: int| {
                &&& 0 <= i < crate::durable_history::durable_prefix_len_spec(extended)
                &&& extended[i].seq_num == seq_num
                &&& extended[i].kind == EVENT_EFFECT_DONE
                &&& extended[i].effect_id == other_id
            };
            if i < events.len() {
                assert(0 <= i < crate::durable_history::durable_prefix_len_spec(events));
                assert(extended[i] == events[i]);
                assert(effect_completed_spec(events, seq_num, other_id));
            } else {
                assert(i < extended.len());
                assert(i == events.len());
                assert(extended[i] == marker);
                assert(marker.effect_id == completed_id);
                assert(marker.effect_id != other_id);
                assert(false);
            }
        }
    };

    assert(effect_completed_spec(events, seq_num, other_id) ==> effect_completed_spec(extended, seq_num, other_id)) by {
        if effect_completed_spec(events, seq_num, other_id) {
            let i = choose|i: int| {
                &&& 0 <= i < crate::durable_history::durable_prefix_len_spec(events)
                &&& events[i].seq_num == seq_num
                &&& events[i].kind == EVENT_EFFECT_DONE
                &&& events[i].effect_id == other_id
            };
            assert(0 <= i < crate::durable_history::durable_prefix_len_spec(extended));
            assert(extended[i] == events[i]);
            assert(effect_completed_spec(extended, seq_num, other_id));
        }
    };
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::durable_history::DurableEvent;
    use crate::durable_history::EVENT_CANCELLED;
    use crate::durable_history::EVENT_EFFECT_DONE;
    use crate::durable_history::EVENT_WORK;

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

    fn random_event((seq_num, kind, effect_id): (usize, u8, u64)) -> DurableEvent {
        DurableEvent {
            seq_num,
            kind,
            effect_id,
        }
    }

    fn effect_completed_exec(events: &[DurableEvent], seq_num: usize, effect_id: u64) -> bool {
        let prefix = durable_prefix_len(events);
        events
            .iter()
            .take(prefix)
            .any(|event| event.seq_num == seq_num && event.kind == EVENT_EFFECT_DONE && event.effect_id == effect_id)
    }

    // r[verify durable-execution.replayable-event]
    #[test]
    fn test_replayable_event_at() {
        let events = vec![work(0), effect(1, 9), DurableEvent {
            seq_num: 2,
            kind: EVENT_CANCELLED,
            effect_id: 0,
        }];
        assert_eq!(replayable_event_at(&events, 0), Some(work(0)));
        assert_eq!(replayable_event_at(&events, 1), Some(effect(1, 9)));
        assert_eq!(
            replayable_event_at(&events, 2),
            Some(DurableEvent {
                seq_num: 2,
                kind: EVENT_CANCELLED,
                effect_id: 0
            })
        );
        assert_eq!(replayable_event_at(&events, 3), None);
    }

    // r[verify durable-execution.effect-dedup]
    #[test]
    fn test_should_execute_effect() {
        let events = vec![work(0), effect(1, 42)];
        assert!(should_execute_effect(&events, 0, 42));
        assert!(!should_execute_effect(&events, 1, 42));
        assert!(should_execute_effect(&events, 1, 99));
    }

    proptest! {
        // r[verify durable-execution.effect-dedup]
        #[test]
        fn prop_matching_marker_blocks(effect_id in 0u64..1000) {
            let events = vec![work(0), effect(1, effect_id)];
            prop_assert!(!should_execute_effect(&events, 1, effect_id));
        }

        // r[verify durable-execution.replayable-event]
        #[test]
        fn prop_replay_outside_prefix_rejected(len in 0usize..30) {
            let mut events: Vec<DurableEvent> = (0..len).map(work).collect();
            events.push(DurableEvent { seq_num: len + 1, kind: EVENT_WORK, effect_id: 0 });
            let prefix = durable_prefix_len(&events);
            prop_assert_eq!(replayable_event_at(&events, prefix), None);
        }

        // r[verify durable-execution.effect-dedup]
        #[test]
        fn prop_random_histories_match_effect_oracle(
            events in proptest::collection::vec((0usize..20, 0u8..8, 0u64..8), 0..20),
            seq_num in 0usize..20,
            effect_id in 0u64..8,
        ) {
            let events: Vec<DurableEvent> = events.into_iter().map(random_event).collect();
            let oracle = effect_completed_exec(&events, seq_num, effect_id);
            prop_assert_eq!(should_execute_effect(&events, seq_num, effect_id), !oracle);
        }

        // r[verify durable-execution.effect-dedup]
        #[test]
        fn prop_random_valid_histories_unrelated_effect_ids_remain_independent(
            base in proptest::collection::vec((any::<bool>(), 0u64..8), 0..20),
            query_seq in 0usize..21,
            completed_id in 0u64..8,
            other_id in 8u64..16,
        ) {
            let events: Vec<DurableEvent> = base.into_iter().enumerate().map(|(seq_num, (is_effect, effect_id))| {
                DurableEvent {
                    seq_num,
                    kind: if is_effect { EVENT_EFFECT_DONE } else { EVENT_WORK },
                    effect_id,
                }
            }).collect();
            let before = should_execute_effect(&events, query_seq, other_id);
            let mut extended = events.clone();
            extended.push(DurableEvent {
                seq_num: events.len(),
                kind: EVENT_EFFECT_DONE,
                effect_id: completed_id,
            });
            prop_assert!(completed_id != other_id);
            prop_assert_eq!(should_execute_effect(&extended, query_seq, other_id), before);
        }

        // r[verify durable-execution.replayable-event]
        #[test]
        fn prop_random_histories_replay_obeys_prefix(
            events in proptest::collection::vec((0usize..20, 0u8..8, 0u64..8), 0..20),
            raw_index in 0usize..25,
        ) {
            let events: Vec<DurableEvent> = events.into_iter().map(random_event).collect();
            let prefix = durable_prefix_len(&events);
            if raw_index < prefix {
                prop_assert_eq!(replayable_event_at(&events, raw_index), Some(events[raw_index]));
            } else {
                prop_assert_eq!(replayable_event_at(&events, raw_index), None);
            }
        }
    }
}
