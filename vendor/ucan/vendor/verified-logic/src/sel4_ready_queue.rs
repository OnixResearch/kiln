//! Verified seL4-style ready queues.
//!
//! Models per-priority ready queues as a vector of FIFO queues of TCB indices.
//! Higher numeric priority wins. Queue operations are total: invalid inputs
//! return `false` / `None` and leave the queue state unchanged.

use vstd::prelude::*;

use crate::sel4_tcb::Tcb;
use crate::sel4_tcb::TcbState;
#[cfg(verus_keep_ghost)]
use crate::sel4_tcb::runnable_state_spec;

verus! {

#[derive(Clone)]
pub struct ReadyQueues {
    pub queues: Vec<Vec<usize>>,
}

// r[impl sel4-ready-queue.consistency]
// r[depends sel4-ready-queue.consistency]
pub open spec fn queue_view_spec(queues: Seq<Vec<usize>>) -> Seq<Seq<usize>> {
    queues.map_values(|v: Vec<usize>| v@)
}

// r[impl sel4-ready-queue.selection]
// r[depends sel4-ready-queue.selection]
pub open spec fn valid_priority_spec(queues: Seq<Seq<usize>>, priority: int) -> bool {
    0 <= priority < queues.len()
}

// r[impl sel4-ready-queue.consistency]
// r[depends sel4-ready-queue.consistency]
pub open spec fn queue_contains_spec(queue: Seq<usize>, tid: usize) -> bool {
    exists|i: int| #![trigger queue[i]] 0 <= i < queue.len() && queue[i] == tid
}

// r[impl sel4-ready-queue.consistency]
// r[depends sel4-ready-queue.consistency]
pub open spec fn queues_contain_spec(queues: Seq<Seq<usize>>, tid: usize) -> bool {
    exists|p: int, i: int|
        #![trigger queues[p][i]]
        0 <= p < queues.len() && 0 <= i < queues[p].len() && queues[p][i] == tid
}

pub open spec fn count_tid_in_queue_from_spec(queue: Seq<usize>, tid: usize, from: int) -> nat
    decreases (if queue.len() >= from { queue.len() - from } else { 0 }),
{
    if from < 0 || from >= queue.len() {
        0nat
    } else {
        let tail = count_tid_in_queue_from_spec(queue, tid, from + 1);
        if queue[from] == tid { tail + 1nat } else { tail }
    }
}

// r[impl sel4-ready-queue.consistency]
// r[depends sel4-ready-queue.consistency]
pub open spec fn count_tid_in_queue_spec(queue: Seq<usize>, tid: usize) -> nat {
    count_tid_in_queue_from_spec(queue, tid, 0)
}

pub open spec fn count_tid_in_queues_from_spec(queues: Seq<Seq<usize>>, tid: usize, from: int) -> nat
    decreases (if queues.len() >= from { queues.len() - from } else { 0 }),
{
    if from < 0 || from >= queues.len() {
        0nat
    } else {
        count_tid_in_queue_spec(queues[from], tid) + count_tid_in_queues_from_spec(queues, tid, from + 1)
    }
}

// r[impl sel4-ready-queue.consistency]
// r[depends sel4-ready-queue.consistency]
pub open spec fn count_tid_in_queues_spec(queues: Seq<Seq<usize>>, tid: usize) -> nat {
    count_tid_in_queues_from_spec(queues, tid, 0)
}

// r[impl sel4-ready-queue.consistency]
// r[depends sel4-ready-queue.consistency]
pub open spec fn consistent_ready_queues_spec(queues: Seq<Seq<usize>>, tcbs: Seq<Tcb>) -> bool {
    &&& forall|tid: int|
        #![trigger tcbs[tid]]
        #![trigger count_tid_in_queues_spec(queues, tid as usize)]
        0 <= tid < tcbs.len()
        ==> {
            let priority = tcbs[tid].priority as int;
            if runnable_state_spec(tcbs[tid].state) {
                &&& valid_priority_spec(queues, priority)
                &&& count_tid_in_queues_spec(queues, tid as usize) == 1nat
                &&& queue_contains_spec(queues[priority], tid as usize)
            } else {
                count_tid_in_queues_spec(queues, tid as usize) == 0nat
            }
        }
    &&& forall|priority: int, pos: int|
        #![trigger queues[priority][pos]]
        valid_priority_spec(queues, priority) && 0 <= pos < queues[priority].len()
        ==> {
            let tid = queues[priority][pos];
            &&& (tid as int) < tcbs.len()
            &&& runnable_state_spec(tcbs[tid as int].state)
            &&& tcbs[tid as int].priority as int == priority
        }
}

// r[impl sel4-ready-queue.order]
// r[depends sel4-ready-queue.order]
pub open spec fn enqueue_queue_spec(queue: Seq<usize>, tid: usize) -> Seq<usize> {
    queue.push(tid)
}

// r[impl sel4-ready-queue.order]
// r[depends sel4-ready-queue.order]
pub open spec fn dequeue_queue_spec(queue: Seq<usize>) -> Option<(usize, Seq<usize>)>
    recommends queue.len() > 0
{
    Some((queue[0], queue.subrange(1, queue.len() as int)))
}

pub open spec fn remove_queue_prefix_spec(queue: Seq<usize>, tid: usize, upto: int) -> Seq<usize>
    decreases (if upto >= 0 { upto } else { 0 }),
{
    if upto <= 0 {
        seq![]
    } else {
        let prefix = remove_queue_prefix_spec(queue, tid, upto - 1);
        if queue[upto - 1] == tid { prefix } else { prefix.push(queue[upto - 1]) }
    }
}

// r[impl sel4-ready-queue.order]
// r[depends sel4-ready-queue.order]
pub open spec fn remove_queue_spec(queue: Seq<usize>, tid: usize) -> Seq<usize> {
    remove_queue_prefix_spec(queue, tid, queue.len() as int)
}

// r[impl sel4-ready-queue.consistency]
// r[depends sel4-ready-queue.consistency]
pub open spec fn enqueue_enabled_spec(queues: Seq<Seq<usize>>, tcbs: Seq<Tcb>, tid: usize) -> bool {
    &&& (tid as int) < tcbs.len()
    &&& runnable_state_spec(tcbs[tid as int].state)
    &&& valid_priority_spec(queues, tcbs[tid as int].priority as int)
    &&& !queues_contain_spec(queues, tid)
}

// r[impl sel4-ready-queue.order]
// r[depends sel4-ready-queue.order]
pub open spec fn enqueue_transition_spec(queues: Seq<Seq<usize>>, tcbs: Seq<Tcb>, tid: usize) -> Seq<Seq<usize>> {
    if enqueue_enabled_spec(queues, tcbs, tid) {
        let priority = tcbs[tid as int].priority as int;
        queues.update(priority, enqueue_queue_spec(queues[priority], tid))
    } else {
        queues
    }
}

// r[impl sel4-ready-queue.order]
// r[depends sel4-ready-queue.order]
pub open spec fn dequeue_result_spec(queues: Seq<Seq<usize>>, priority: int) -> Option<usize> {
    if valid_priority_spec(queues, priority) && queues[priority].len() > 0 {
        Some(queues[priority][0])
    } else {
        None
    }
}

// r[impl sel4-ready-queue.order]
// r[depends sel4-ready-queue.order]
pub open spec fn dequeue_transition_spec(queues: Seq<Seq<usize>>, priority: int) -> Seq<Seq<usize>> {
    if valid_priority_spec(queues, priority) && queues[priority].len() > 0 {
        queues.update(priority, queues[priority].subrange(1, queues[priority].len() as int))
    } else {
        queues
    }
}

// r[impl sel4-ready-queue.order]
// r[depends sel4-ready-queue.order]
pub open spec fn remove_result_spec(queues: Seq<Seq<usize>>, priority: int, tid: usize) -> bool {
    valid_priority_spec(queues, priority) && queue_contains_spec(queues[priority], tid)
}

// r[impl sel4-ready-queue.order]
// r[depends sel4-ready-queue.order]
pub open spec fn remove_transition_spec(queues: Seq<Seq<usize>>, priority: int, tid: usize) -> Seq<Seq<usize>> {
    if remove_result_spec(queues, priority, tid) {
        queues.update(priority, remove_queue_spec(queues[priority], tid))
    } else {
        queues
    }
}

pub open spec fn highest_nonempty_priority_from_spec(queues: Seq<Seq<usize>>, priority: int) -> Option<int>
    decreases (if priority >= 0 { priority + 1 } else { 0 }),
{
    if priority < 0 {
        None
    } else if queues[priority].len() > 0 {
        Some(priority)
    } else {
        highest_nonempty_priority_from_spec(queues, priority - 1)
    }
}

// r[impl sel4-ready-queue.selection]
// r[depends sel4-ready-queue.selection]
pub open spec fn highest_nonempty_priority_spec(queues: Seq<Seq<usize>>) -> Option<int> {
    highest_nonempty_priority_from_spec(queues, queues.len() - 1)
}

pub open spec fn select_highest_prefix_spec(queues: Seq<Seq<usize>>, upto: int) -> Option<usize>
    decreases (if upto >= 0 { upto } else { 0 }),
{
    if upto <= 0 {
        None
    } else {
        let prev = select_highest_prefix_spec(queues, upto - 1);
        if queues[upto - 1].len() > 0 {
            Some(queues[upto - 1][0])
        } else {
            prev
        }
    }
}

// r[impl sel4-ready-queue.selection]
// r[depends sel4-ready-queue.selection]
pub open spec fn select_highest_spec(queues: Seq<Seq<usize>>) -> Option<usize> {
    select_highest_prefix_spec(queues, queues.len() as int)
}

fn clone_from(queue: &[usize], from: usize) -> (result: Vec<usize>)
    requires from <= queue@.len()
    ensures result@ == queue@.subrange(from as int, queue@.len() as int)
{
    let mut result: Vec<usize> = Vec::new();
    let mut i: usize = from;
    while i < queue.len()
        invariant
            from <= i <= queue@.len(),
            result@ == queue@.subrange(from as int, i as int),
        decreases queue@.len() - i,
    {
        result.push(queue[i]);
        i = i + 1;
    }
    result
}

fn clone_remove(queue: &[usize], tid: usize) -> (result: Vec<usize>)
    ensures result@ == remove_queue_spec(queue@, tid)
{
    let mut result: Vec<usize> = Vec::new();
    let mut i: usize = 0;
    while i < queue.len()
        invariant
            0 <= i <= queue@.len(),
            result@ == remove_queue_prefix_spec(queue@, tid, i as int),
        decreases queue@.len() - i,
    {
        if queue[i] != tid {
            result.push(queue[i]);
            proof {
                assert(remove_queue_prefix_spec(queue@, tid, i as int + 1)
                    == remove_queue_prefix_spec(queue@, tid, i as int).push(queue[i as int]));
            }
        } else {
            proof {
                assert(remove_queue_prefix_spec(queue@, tid, i as int + 1)
                    == remove_queue_prefix_spec(queue@, tid, i as int));
            }
        }
        i = i + 1;
    }
    result
}

proof fn mapped_queues_update(queues: Seq<Vec<usize>>, index: int, new_queue: Vec<usize>)
    requires
        0 <= index < queues.len(),
    ensures
        queue_view_spec(queues.update(index, new_queue))
            =~= queue_view_spec(queues).update(index, new_queue@),
{
    let updated = queue_view_spec(queues.update(index, new_queue));
    let mapped = queue_view_spec(queues).update(index, new_queue@);
    assert forall|i: int|
        #![trigger updated[i]]
        0 <= i < queues.len()
    implies updated[i] == mapped[i] by {
        if i == index {
        } else {
        }
    }
}

// r[impl sel4-ready-queue.consistency]
pub fn new_ready_queues(num_priorities: usize) -> (result: ReadyQueues)
    ensures
        result.queues@.len() == num_priorities,
        forall|priority: int| 0 <= priority < num_priorities ==> result.queues@[priority]@.len() == 0,
{
    let mut queues: Vec<Vec<usize>> = Vec::new();
    let mut i: usize = 0;
    while i < num_priorities
        invariant
            0 <= i <= num_priorities,
            queues@.len() == i,
            forall|priority: int| 0 <= priority < i ==> queues@[priority]@.len() == 0,
        decreases num_priorities - i,
    {
        let empty: Vec<usize> = Vec::new();
        queues.push(empty);
        i = i + 1;
    }
    ReadyQueues { queues }
}

// r[impl sel4-ready-queue.consistency]
pub fn queue_contains(queue: &[usize], tid: usize) -> (result: bool)
    ensures result == queue_contains_spec(queue@, tid)
{
    let mut i: usize = 0;
    while i < queue.len()
        invariant
            0 <= i <= queue@.len(),
            forall|j: int| #![trigger queue@[j]] 0 <= j < i as int ==> queue@[j] != tid,
        decreases queue@.len() - i,
    {
        if queue[i] == tid {
            return true;
        }
        i = i + 1;
    }
    false
}

fn queues_contain(queues: &[Vec<usize>], tid: usize) -> (result: bool)
    ensures result == queues_contain_spec(queue_view_spec(queues@), tid)
{
    let mut q: usize = 0;
    while q < queues.len()
        invariant
            0 <= q <= queues@.len(),
            forall|priority: int, pos: int|
                #![trigger queues@[priority]@[pos]]
                0 <= priority < q as int && 0 <= pos < queues@[priority]@.len()
                ==> queues@[priority]@[pos] != tid,
        decreases queues@.len() - q,
    {
        if queue_contains(&queues[q], tid) {
            proof {
                let pos = choose|pos: int| #![trigger queues@[q as int]@[pos]]
                    0 <= pos < queues@[q as int]@.len() && queues@[q as int]@[pos] == tid;
                assert(queue_view_spec(queues@)[q as int] == queues@[q as int]@);
                assert(queue_view_spec(queues@)[q as int][pos] == tid);
            }
            return true;
        }
        q = q + 1;
    }
    false
}

// r[impl sel4-ready-queue.order]
pub fn enqueue(ready: &mut ReadyQueues, tcbs: &[Tcb], tid: usize) -> (result: bool)
    ensures
        queue_view_spec(ready.queues@)
            == enqueue_transition_spec(queue_view_spec(old(ready).queues@), tcbs@, tid),
        result == enqueue_enabled_spec(queue_view_spec(old(ready).queues@), tcbs@, tid),
{
    let ghost old_view = queue_view_spec(old(ready).queues@);
    if tid < tcbs.len() {
        let tcb = tcbs[tid];
        let priority = tcb.priority as usize;
        if matches!(tcb.state, TcbState::Runnable)
            && priority < ready.queues.len()
            && !queues_contain(&ready.queues, tid)
        {
            let mut new_queue = clone_from(&ready.queues[priority], 0);
            new_queue.push(tid);
            proof {
                assert(new_queue@ == enqueue_queue_spec(old(ready).queues@[priority as int]@, tid));
            }
            ready.queues.set(priority, new_queue);
            proof {
                mapped_queues_update(old(ready).queues@, priority as int, ready.queues@[priority as int]);
                assert(old_view[priority as int] == old(ready).queues@[priority as int]@);
            }
            true
        } else {
            false
        }
    } else {
        false
    }
}

// r[impl sel4-ready-queue.order]
pub fn dequeue(ready: &mut ReadyQueues, priority: usize) -> (result: Option<usize>)
    ensures
        queue_view_spec(ready.queues@)
            == dequeue_transition_spec(queue_view_spec(old(ready).queues@), priority as int),
        result == dequeue_result_spec(queue_view_spec(old(ready).queues@), priority as int),
{
    let ghost old_view = queue_view_spec(old(ready).queues@);
    if priority < ready.queues.len() && ready.queues[priority].len() > 0 {
        let tid = ready.queues[priority][0];
        let rest = clone_from(&ready.queues[priority], 1);
        ready.queues.set(priority, rest);
        proof {
            mapped_queues_update(old(ready).queues@, priority as int, ready.queues@[priority as int]);
            assert(old_view[priority as int] == old(ready).queues@[priority as int]@);
        }
        Some(tid)
    } else {
        None
    }
}

// r[impl sel4-ready-queue.order]
pub fn remove(ready: &mut ReadyQueues, priority: usize, tid: usize) -> (result: bool)
    ensures
        queue_view_spec(ready.queues@)
            == remove_transition_spec(queue_view_spec(old(ready).queues@), priority as int, tid),
        result == remove_result_spec(queue_view_spec(old(ready).queues@), priority as int, tid),
{
    let ghost old_view = queue_view_spec(old(ready).queues@);
    if priority < ready.queues.len() && queue_contains(&ready.queues[priority], tid) {
        let new_queue = clone_remove(&ready.queues[priority], tid);
        ready.queues.set(priority, new_queue);
        proof {
            mapped_queues_update(old(ready).queues@, priority as int, ready.queues@[priority as int]);
            assert(old_view[priority as int] == old(ready).queues@[priority as int]@);
        }
        true
    } else {
        false
    }
}

// r[impl sel4-ready-queue.selection]
pub fn select_highest(ready: &ReadyQueues) -> (result: Option<usize>)
    ensures result == select_highest_spec(queue_view_spec(ready.queues@))
{
    let mut i: usize = 0;
    let mut best: Option<usize> = None;
    while i < ready.queues.len()
        invariant
            0 <= i <= ready.queues@.len(),
            best == select_highest_prefix_spec(queue_view_spec(ready.queues@), i as int),
        decreases ready.queues@.len() - i,
    {
        if ready.queues[i].len() > 0 {
            best = Some(ready.queues[i][0]);
            proof {
                assert(select_highest_prefix_spec(queue_view_spec(ready.queues@), i as int + 1)
                    == Some(ready.queues@[i as int]@[0]));
            }
        } else {
            proof {
                assert(select_highest_prefix_spec(queue_view_spec(ready.queues@), i as int + 1)
                    == select_highest_prefix_spec(queue_view_spec(ready.queues@), i as int));
            }
        }
        i = i + 1;
    }
    best
}

// r[verify sel4-ready-queue.consistency]
proof fn consistency_runnable_queued_once(queues: Seq<Seq<usize>>, tcbs: Seq<Tcb>, tid: usize)
    requires
        consistent_ready_queues_spec(queues, tcbs),
        (tid as int) < tcbs.len(),
        runnable_state_spec(tcbs[tid as int].state),
    ensures
        count_tid_in_queues_spec(queues, tid) == 1nat,
        queue_contains_spec(queues[tcbs[tid as int].priority as int], tid),
{
}

// r[verify sel4-ready-queue.consistency]
proof fn consistency_queued_tcb_is_runnable(queues: Seq<Seq<usize>>, tcbs: Seq<Tcb>, priority: int, pos: int)
    requires
        consistent_ready_queues_spec(queues, tcbs),
        valid_priority_spec(queues, priority),
        0 <= pos < queues[priority].len(),
    ensures
        (queues[priority][pos] as int) < tcbs.len(),
        runnable_state_spec(tcbs[queues[priority][pos] as int].state),
        tcbs[queues[priority][pos] as int].priority as int == priority,
{
}

// r[verify sel4-ready-queue.consistency]
proof fn enqueue_preserves_consistency(queues: Seq<Seq<usize>>, tcbs: Seq<Tcb>, tid: usize)
    requires
        consistent_ready_queues_spec(queues, tcbs),
        enqueue_enabled_spec(queues, tcbs, tid),
    ensures
        consistent_ready_queues_spec(enqueue_transition_spec(queues, tcbs, tid), tcbs),
{
}

proof fn highest_nonempty_matches_selected_prefix(queues: Seq<Seq<usize>>, upto: int)
    requires
        0 <= upto <= queues.len(),
    ensures
        match highest_nonempty_priority_from_spec(queues, upto - 1) {
            Some(priority) => select_highest_prefix_spec(queues, upto) == Some(queues[priority][0]),
            None => select_highest_prefix_spec(queues, upto) == None::<usize>,
        },
    decreases upto,
{
    if upto > 0 {
        highest_nonempty_matches_selected_prefix(queues, upto - 1);
        if queues[upto - 1].len() > 0 {
        } else {
        }
    }
}

// r[verify sel4-ready-queue.selection]
proof fn selected_from_highest_nonempty_queue(queues: Seq<Seq<usize>>)
    requires
        select_highest_spec(queues).is_some(),
    ensures
        highest_nonempty_priority_spec(queues).is_some(),
        select_highest_spec(queues) == Some(queues[highest_nonempty_priority_spec(queues).unwrap()][0]),
{
    highest_nonempty_matches_selected_prefix(queues, queues.len() as int);
}

// r[verify sel4-ready-queue.order]
proof fn enqueue_preserves_prefix_order(queue: Seq<usize>, tid: usize)
    ensures
        enqueue_queue_spec(queue, tid).subrange(0, queue.len() as int) == queue,
{
}

// r[verify sel4-ready-queue.order]
proof fn dequeue_preserves_tail_order(queue: Seq<usize>)
    requires
        queue.len() > 0,
    ensures
        dequeue_queue_spec(queue).unwrap().1 == queue.subrange(1, queue.len() as int),
{
}

// r[verify sel4-ready-queue.order]
proof fn remove_preserves_head_when_unaffected(queue: Seq<usize>, tid: usize)
    requires
        queue.len() > 0,
        queue[0] != tid,
    ensures
        remove_queue_prefix_spec(queue, tid, 1).len() > 0,
        remove_queue_prefix_spec(queue, tid, 1)[0] == queue[0],
{
    assert(remove_queue_prefix_spec(queue, tid, 0) == Seq::<usize>::empty());
    assert(remove_queue_prefix_spec(queue, tid, 1)
        == remove_queue_prefix_spec(queue, tid, 0).push(queue[0]));
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::sel4_tcb::Tcb;
    use crate::sel4_tcb::new_tcb;

    fn sample_tcbs() -> Vec<Tcb> {
        vec![new_tcb(10, 1), new_tcb(11, 3), new_tcb(12, 2), new_tcb(13, 3)]
    }

    // r[verify sel4-ready-queue.consistency]
    #[test]
    fn test_enqueue_runnable_once_at_priority() {
        let tcbs = sample_tcbs();
        let mut ready = new_ready_queues(4);
        assert!(enqueue(&mut ready, &tcbs, 0));
        assert_eq!(ready.queues[1], vec![0]);
        assert!(!enqueue(&mut ready, &tcbs, 0));
    }

    // r[verify sel4-ready-queue.selection]
    #[test]
    fn test_select_highest_priority_head() {
        let tcbs = sample_tcbs();
        let mut ready = new_ready_queues(4);
        assert!(enqueue(&mut ready, &tcbs, 0));
        assert!(enqueue(&mut ready, &tcbs, 1));
        assert!(enqueue(&mut ready, &tcbs, 2));
        assert_eq!(select_highest(&ready), Some(1));
    }

    // r[verify sel4-ready-queue.order]
    #[test]
    fn test_dequeue_preserves_fifo_order() {
        let tcbs = sample_tcbs();
        let mut ready = new_ready_queues(4);
        assert!(enqueue(&mut ready, &tcbs, 1));
        assert!(enqueue(&mut ready, &tcbs, 3));
        assert_eq!(dequeue(&mut ready, 3), Some(1));
        assert_eq!(ready.queues[3], vec![3]);
    }

    // r[verify sel4-ready-queue.order]
    #[test]
    fn test_remove_preserves_remaining_order() {
        let tcbs = sample_tcbs();
        let mut ready = new_ready_queues(4);
        assert!(enqueue(&mut ready, &tcbs, 1));
        assert!(enqueue(&mut ready, &tcbs, 3));
        assert!(remove(&mut ready, 3, 1));
        assert_eq!(ready.queues[3], vec![3]);
    }

    // r[verify sel4-ready-queue.consistency]
    #[test]
    fn test_invalid_tid_does_not_change_state() {
        let tcbs = sample_tcbs();
        let mut ready = new_ready_queues(4);
        assert!(!enqueue(&mut ready, &tcbs, 99));
        assert_eq!(select_highest(&ready), None);
    }

    proptest! {
        // r[verify sel4-ready-queue.order]
        #[test]
        fn prop_higher_priority_wins(_low in 0usize..4, _high in 0usize..4) {
            let tcbs = vec![new_tcb(0, 1), new_tcb(1, 3)];
            let mut ready = new_ready_queues(4);
            prop_assert!(enqueue(&mut ready, &tcbs, 0));
            prop_assert!(enqueue(&mut ready, &tcbs, 1));
            prop_assert_eq!(select_highest(&ready), Some(1));
        }

        // r[verify sel4-ready-queue.order]
        #[test]
        fn prop_remove_keeps_order_of_unaffected_elements(a: usize, b: usize, c: usize) {
            prop_assume!(a != b && c != b);
            let mut ready = new_ready_queues(1);
            ready.queues[0] = vec![a, b, c];
            prop_assert!(remove(&mut ready, 0, b));
            prop_assert_eq!(&ready.queues[0], &vec![a, c]);
        }

        // r[verify sel4-ready-queue.order]
        #[test]
        fn prop_dequeue_returns_head_and_tail(priority in 0usize..4) {
            let mut ready = new_ready_queues(4);
            ready.queues[priority] = vec![7, 9, 11];
            prop_assert_eq!(dequeue(&mut ready, priority), Some(7));
            prop_assert_eq!(&ready.queues[priority], &vec![9, 11]);
        }

        // r[verify sel4-ready-queue.consistency]
        #[test]
        fn prop_duplicate_enqueue_rejected(priority in 0u8..4) {
            let tcbs = vec![new_tcb(0, priority)];
            let mut ready = new_ready_queues(4);
            prop_assert!(enqueue(&mut ready, &tcbs, 0));
            prop_assert!(!enqueue(&mut ready, &tcbs, 0));
        }
    }
}
