//! Verified seL4-style kernel invariant composition.
//!
//! This module starts the cross-module kernel-state story by aggregating the
//! existing seL4 primitives into one `KernelState` and naming the first layer
//! of global sub-invariants.

use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::region::regions_disjoint_spec;
use crate::sel4_asid::AsidPool;
use crate::sel4_cdt::Cdt;
use crate::sel4_cspace::CSpace;
use crate::sel4_endpoint::Endpoint;
use crate::sel4_endpoint::Rendezvous;
use crate::sel4_endpoint::SenderWaiter;
use crate::sel4_endpoint::send;
use crate::sel4_irq::IrqLine;
use crate::sel4_ready_queue::ReadyQueues;
#[cfg(verus_keep_ghost)]
use crate::sel4_ready_queue::consistent_ready_queues_spec;
#[cfg(verus_keep_ghost)]
use crate::sel4_ready_queue::count_tid_in_queue_from_spec;
#[cfg(verus_keep_ghost)]
use crate::sel4_ready_queue::count_tid_in_queue_spec;
#[cfg(verus_keep_ghost)]
use crate::sel4_ready_queue::count_tid_in_queues_from_spec;
#[cfg(verus_keep_ghost)]
use crate::sel4_ready_queue::count_tid_in_queues_spec;
use crate::sel4_ready_queue::enqueue;
#[cfg(verus_keep_ghost)]
use crate::sel4_ready_queue::queue_contains_spec;
#[cfg(verus_keep_ghost)]
use crate::sel4_ready_queue::queue_view_spec;
#[cfg(verus_keep_ghost)]
use crate::sel4_ready_queue::queues_contain_spec;
use crate::sel4_tcb::Tcb;
use crate::sel4_tcb::TcbState;
use crate::sel4_tcb::block_on_send;
#[cfg(verus_keep_ghost)]
use crate::sel4_tcb::block_on_send_transition_spec;
#[cfg(verus_keep_ghost)]
use crate::sel4_tcb::legal_state_spec;
#[cfg(verus_keep_ghost)]
use crate::sel4_tcb::runnable_state_spec;
use crate::sel4_tcb::wake;
#[cfg(verus_keep_ghost)]
use crate::sel4_tcb::wake_transition_spec;
use crate::sel4_untyped::ChildObject;
use crate::sel4_untyped::UntypedMemory;
use crate::sel4_vspace::VSpace;

verus! {

pub struct KernelState {
    pub tcbs: Vec<Tcb>,
    pub ready_queues: ReadyQueues,
    pub endpoints: Vec<Endpoint>,
    pub notification_count: usize,
    pub reply_count: usize,
    pub cdt: Cdt,
    pub cspace: CSpace,
    pub untypeds: Vec<UntypedMemory>,
    pub asid_pool: AsidPool,
    pub vspaces: Vec<VSpace>,
    pub irqs: Vec<IrqLine>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct KernelStepDecision {
    pub applied: bool,
    pub reason: u8,
}

pub const PRESERVATION_STEP_ENDPOINT_SEND_BLOCK: u8 = 1;
pub const PRESERVATION_STEP_ENDPOINT_SEND_RENDEZVOUS: u8 = 2;
pub const PRESERVATION_STEP_UNTYPED_REVOKE: u8 = 3;

pub const KERNEL_STEP_REASON_NONE: u8 = 0;
pub const KERNEL_STEP_REJECT_INVALID_REFERENCE: u8 = 1;
pub const KERNEL_STEP_REJECT_MISMATCHED_ENDPOINT_TCB: u8 = 2;
pub const KERNEL_STEP_REJECT_OUT_OF_SCOPE: u8 = 3;

// r[impl sel4-invariants.preservation-matrix]
// r[depends sel4-invariants.preservation-matrix]
pub open spec fn preservation_matrix_step_known_spec(step: u8) -> bool {
    step == PRESERVATION_STEP_ENDPOINT_SEND_BLOCK
        || step == PRESERVATION_STEP_ENDPOINT_SEND_RENDEZVOUS
        || step == PRESERVATION_STEP_UNTYPED_REVOKE
}

// r[impl sel4-invariants.step-preservation-expansion]
// r[depends sel4-invariants.step-preservation-expansion]
pub open spec fn rejected_kernel_step_decision_spec(reason: u8) -> KernelStepDecision {
    KernelStepDecision { applied: false, reason }
}

// r[impl sel4-invariants.preservation-matrix]
// r[depends sel4-invariants.preservation-matrix]
pub open spec fn kernel_state_unchanged_spec(before: KernelState, after: KernelState) -> bool {
    &&& after.tcbs@ =~= before.tcbs@
    &&& after.ready_queues.queues@ =~= before.ready_queues.queues@
    &&& after.endpoints@ =~= before.endpoints@
    &&& after.notification_count == before.notification_count
    &&& after.reply_count == before.reply_count
    &&& after.cdt.parents@ =~= before.cdt.parents@
    &&& after.cdt.slots@ =~= before.cdt.slots@
    &&& after.cdt.present@ =~= before.cdt.present@
    &&& after.cspace.nodes@ =~= before.cspace.nodes@
    &&& after.cspace.root == before.cspace.root
    &&& after.untypeds@ =~= before.untypeds@
    &&& after.asid_pool.ownership_view() =~= before.asid_pool.ownership_view()
    &&& after.asid_pool.capacity == before.asid_pool.capacity
    &&& after.vspaces@ =~= before.vspaces@
    &&& after.irqs@ =~= before.irqs@
}

// r[impl sel4-invariants.decomposition]
// r[depends sel4-invariants.decomposition]
pub open spec fn object_refs_valid_spec(state: KernelState) -> bool {
    &&& forall|endpoint_idx: int| #![trigger state.endpoints@[endpoint_idx]]
        0 <= endpoint_idx < state.endpoints@.len() ==> state.endpoints@[endpoint_idx].wf()
    &&& forall|tid: int|
        #![trigger state.tcbs@[tid]]
        0 <= tid < state.tcbs@.len() ==> match state.tcbs@[tid].state {
            TcbState::BlockedOnSend { endpoint } | TcbState::BlockedOnReceive { endpoint } => {
                endpoint < state.endpoints@.len()
            }
            TcbState::BlockedOnNotification { notification } => {
                notification < state.notification_count
            }
            TcbState::BlockedOnReply { reply } => {
                reply < state.reply_count
            }
            TcbState::Runnable | TcbState::Running | TcbState::Restart | TcbState::Inactive => true,
        }
    &&& forall|endpoint_idx: int, pos: int|
        #![trigger state.endpoints@[endpoint_idx].senders@[pos]]
        0 <= endpoint_idx < state.endpoints@.len()
            && 0 <= pos < state.endpoints@[endpoint_idx].senders@.len()
            ==> state.endpoints@[endpoint_idx].senders@[pos].tcb < state.tcbs@.len()
    &&& forall|endpoint_idx: int, pos: int|
        #![trigger state.endpoints@[endpoint_idx].receivers@[pos]]
        0 <= endpoint_idx < state.endpoints@.len()
            && 0 <= pos < state.endpoints@[endpoint_idx].receivers@.len()
            ==> state.endpoints@[endpoint_idx].receivers@[pos] < state.tcbs@.len()
    &&& forall|irq_idx: int|
        #![trigger state.irqs@[irq_idx]]
        0 <= irq_idx < state.irqs@.len() ==> match state.irqs@[irq_idx].handler {
            Some(handler) => handler < state.notification_count,
            None => true,
        }
}

// r[impl sel4-invariants.decomposition]
// r[depends sel4-invariants.decomposition]
pub open spec fn cdt_wf_spec(state: KernelState) -> bool {
    state.cdt.wf()
}

// r[impl sel4-invariants.decomposition]
// r[depends sel4-invariants.decomposition]
pub open spec fn cspace_valid_spec(state: KernelState) -> bool {
    &&& state.cspace.root < state.cspace.nodes@.len()
    &&& forall|node_idx: int, slot_idx: int|
        #![trigger state.cspace.nodes@[node_idx].slots@[slot_idx]]
        0 <= node_idx < state.cspace.nodes@.len()
            && 0 <= slot_idx < state.cspace.nodes@[node_idx].slots@.len()
            ==> match state.cspace.nodes@[node_idx].slots@[slot_idx] {
                Some(child) => child < state.cspace.nodes@.len(),
                None => true,
            }
}

pub open spec fn is_running_tcb_state_spec(state: TcbState) -> bool {
    match state {
        TcbState::Running => true,
        _ => false,
    }
}

pub open spec fn is_blocked_on_send_tcb_state_spec(state: TcbState, endpoint: int) -> bool {
    match state {
        TcbState::BlockedOnSend { endpoint: actual } => actual as int == endpoint,
        _ => false,
    }
}

pub open spec fn is_blocked_on_receive_tcb_state_spec(state: TcbState, endpoint: int) -> bool {
    match state {
        TcbState::BlockedOnReceive { endpoint: actual } => actual as int == endpoint,
        _ => false,
    }
}

pub open spec fn is_runnable_tcb_state_spec(state: TcbState) -> bool {
    match state {
        TcbState::Runnable => true,
        _ => false,
    }
}

// r[impl sel4-invariants.decomposition]
// r[depends sel4-invariants.decomposition]
pub open spec fn tcb_legal_spec(state: KernelState) -> bool {
    forall|tid: int| #![trigger state.tcbs@[tid]] 0 <= tid < state.tcbs@.len() ==> legal_state_spec(state.tcbs@[tid].state)
}

// r[impl sel4-invariants.decomposition]
// r[depends sel4-invariants.decomposition]
pub open spec fn scheduler_consistent_spec(state: KernelState) -> bool {
    consistent_ready_queues_spec(queue_view_spec(state.ready_queues.queues@), state.tcbs@)
}

pub open spec fn top_level_untypeds_disjoint_spec(untypeds: Seq<UntypedMemory>) -> bool {
    forall|i: int, j: int|
        #![trigger untypeds[i], untypeds[j]]
        0 <= i < j < untypeds.len() ==> regions_disjoint_spec(untypeds[i].base, untypeds[i].size, untypeds[j].base, untypeds[j].size)
}

// r[impl sel4-invariants.decomposition]
// r[depends sel4-invariants.decomposition]
pub open spec fn untyped_ownership_spec(state: KernelState) -> bool {
    &&& forall|idx: int| #![trigger state.untypeds@[idx]] 0 <= idx < state.untypeds@.len() ==> state.untypeds@[idx].wf()
    &&& top_level_untypeds_disjoint_spec(state.untypeds@)
}

// r[impl sel4-invariants.decomposition]
// r[depends sel4-invariants.decomposition]
pub open spec fn asid_pool_wf_spec(state: KernelState) -> bool {
    state.asid_pool.wf()
}

// r[impl sel4-invariants.decomposition]
// r[depends sel4-invariants.decomposition]
pub open spec fn vspace_consistent_spec(state: KernelState) -> bool {
    forall|idx: int| #![trigger state.vspaces@[idx]] 0 <= idx < state.vspaces@.len() ==> state.vspaces@[idx].wf()
}

pub open spec fn irq_numbers_distinct_spec(irqs: Seq<IrqLine>) -> bool {
    forall|i: int, j: int| #![trigger irqs[i], irqs[j]] 0 <= i < j < irqs.len() ==> irqs[i].irq != irqs[j].irq
}

// r[impl sel4-invariants.decomposition]
// r[depends sel4-invariants.decomposition]
pub open spec fn irq_unique_spec(state: KernelState) -> bool {
    &&& irq_numbers_distinct_spec(state.irqs@)
    &&& forall|idx: int| #![trigger state.irqs@[idx]] 0 <= idx < state.irqs@.len() ==> match state.irqs@[idx].handler {
        Some(handler) => handler < state.notification_count,
        None => true,
    }
}

// r[impl sel4-invariants.compose]
// r[depends sel4-invariants.compose]
pub open spec fn kernel_invariant_spec(state: KernelState) -> bool {
    &&& object_refs_valid_spec(state)
    &&& cdt_wf_spec(state)
    &&& cspace_valid_spec(state)
    &&& tcb_legal_spec(state)
    &&& scheduler_consistent_spec(state)
    &&& untyped_ownership_spec(state)
    &&& asid_pool_wf_spec(state)
    &&& vspace_consistent_spec(state)
    &&& irq_unique_spec(state)
}

// r[verify sel4-invariants.preservation-matrix]
pub proof fn endpoint_send_block_is_matrix_step()
    ensures preservation_matrix_step_known_spec(PRESERVATION_STEP_ENDPOINT_SEND_BLOCK)
{}

// r[verify sel4-invariants.preservation-matrix]
pub proof fn endpoint_send_rendezvous_is_matrix_step()
    ensures preservation_matrix_step_known_spec(PRESERVATION_STEP_ENDPOINT_SEND_RENDEZVOUS)
{}

// r[verify sel4-invariants.preservation-matrix]
pub proof fn untyped_revoke_is_matrix_step()
    ensures preservation_matrix_step_known_spec(PRESERVATION_STEP_UNTYPED_REVOKE)
{}

// r[verify sel4-invariants.preservation-matrix]
pub proof fn unchanged_state_frames_kernel_invariant(before: KernelState, after: KernelState)
    requires
        kernel_invariant_spec(before),
        kernel_state_unchanged_spec(before, after),
    ensures kernel_invariant_spec(after)
{
    assert(object_refs_valid_spec(after)) by {
        assert(forall|endpoint_idx: int| #![trigger after.endpoints@[endpoint_idx]]
            0 <= endpoint_idx < after.endpoints@.len() ==> after.endpoints@[endpoint_idx].wf());
        assert(forall|tid: int| #![trigger after.tcbs@[tid]] 0 <= tid < after.tcbs@.len() ==> match after.tcbs@[tid].state {
            TcbState::BlockedOnSend { endpoint } | TcbState::BlockedOnReceive { endpoint } => endpoint < after.endpoints@.len(),
            TcbState::BlockedOnNotification { notification } => notification < after.notification_count,
            TcbState::BlockedOnReply { reply } => reply < after.reply_count,
            TcbState::Runnable | TcbState::Running | TcbState::Restart | TcbState::Inactive => true,
        });
        assert(forall|endpoint_idx: int, pos: int| #![trigger after.endpoints@[endpoint_idx].senders@[pos]]
            0 <= endpoint_idx < after.endpoints@.len()
                && 0 <= pos < after.endpoints@[endpoint_idx].senders@.len()
                ==> after.endpoints@[endpoint_idx].senders@[pos].tcb < after.tcbs@.len());
        assert(forall|endpoint_idx: int, pos: int| #![trigger after.endpoints@[endpoint_idx].receivers@[pos]]
            0 <= endpoint_idx < after.endpoints@.len()
                && 0 <= pos < after.endpoints@[endpoint_idx].receivers@.len()
                ==> after.endpoints@[endpoint_idx].receivers@[pos] < after.tcbs@.len());
        assert(forall|irq_idx: int| #![trigger after.irqs@[irq_idx]]
            0 <= irq_idx < after.irqs@.len() ==> match after.irqs@[irq_idx].handler {
                Some(handler) => handler < after.notification_count,
                None => true,
            });
    };
    assert(cdt_wf_spec(after));
    assert(cspace_valid_spec(after));
    assert(tcb_legal_spec(after));
    assert(scheduler_consistent_spec(after));
    assert(untyped_ownership_spec(after));
    assert(asid_pool_wf_spec(after));
    assert(vspace_consistent_spec(after));
    assert(irq_unique_spec(after));
}

// r[impl sel4-invariants.step-preservation-expansion]
pub fn reject_kernel_step_noop(state: &mut KernelState, reason: u8) -> (result: KernelStepDecision)
    ensures
        result == rejected_kernel_step_decision_spec(reason),
        kernel_state_unchanged_spec(*old(state), *state),
        kernel_invariant_spec(*old(state)) ==> kernel_invariant_spec(*state),
{
    proof {
        assert(kernel_state_unchanged_spec(*old(state), *state));
        if kernel_invariant_spec(*old(state)) {
            unchanged_state_frames_kernel_invariant(*old(state), *state);
        }
    }
    KernelStepDecision { applied: false, reason }
}

// Keep initial step-preservation proof factoring local to this module; the
// first kernel-state preservation set does not need a separate properties layer.
proof fn blocked_thread_not_queued(queues: Seq<Seq<usize>>, tcbs: Seq<Tcb>, tid: int)
    requires
        consistent_ready_queues_spec(queues, tcbs),
        0 <= tid < tcbs.len(),
        !runnable_state_spec(tcbs[tid].state),
    ensures
        count_tid_in_queues_spec(queues, tid as usize) == 0nat,
        !queues_contain_spec(queues, tid as usize),
{
    assert(count_tid_in_queues_spec(queues, tid as usize) == 0nat);
    assert(!queues_contain_spec(queues, tid as usize)) by {
        if queues_contain_spec(queues, tid as usize) {
            let pair = choose|pair: (int, int)|
                0 <= pair.0 < queues.len()
                    && 0 <= pair.1 < queues[pair.0].len()
                    && queues[pair.0][pair.1] == tid as usize;
            let priority = pair.0;
            let pos = pair.1;
            assert(runnable_state_spec(tcbs[tid].state));
            assert(false);
        }
    }
}

proof fn count_tid_in_queue_from_push_same(queue: Seq<usize>, tid: usize, from: int)
    requires
        0 <= from <= queue.len(),
    ensures
        count_tid_in_queue_from_spec(queue.push(tid), tid, from)
            == count_tid_in_queue_from_spec(queue, tid, from) + 1nat,
    decreases queue.len() - from,
{
    if from < queue.len() {
        count_tid_in_queue_from_push_same(queue, tid, from + 1);
        assert(queue.push(tid)[from] == queue[from]);
        if queue[from] == tid {
            assert(count_tid_in_queue_from_spec(queue.push(tid), tid, from)
                == count_tid_in_queue_from_spec(queue.push(tid), tid, from + 1) + 1nat);
            assert(count_tid_in_queue_from_spec(queue, tid, from)
                == count_tid_in_queue_from_spec(queue, tid, from + 1) + 1nat);
        } else {
            assert(count_tid_in_queue_from_spec(queue.push(tid), tid, from)
                == count_tid_in_queue_from_spec(queue.push(tid), tid, from + 1));
            assert(count_tid_in_queue_from_spec(queue, tid, from)
                == count_tid_in_queue_from_spec(queue, tid, from + 1));
        }
    } else {
        assert(from == queue.len());
        assert(queue.push(tid)[from] == tid);
        assert(count_tid_in_queue_from_spec(queue.push(tid), tid, from + 1) == 0nat);
        assert(count_tid_in_queue_from_spec(queue.push(tid), tid, from) == 1nat);
        assert(count_tid_in_queue_from_spec(queue, tid, from) == 0nat);
    }
}

proof fn count_tid_in_queue_push_same(queue: Seq<usize>, tid: usize)
    ensures
        count_tid_in_queue_spec(queue.push(tid), tid) == count_tid_in_queue_spec(queue, tid) + 1nat,
{
    count_tid_in_queue_from_push_same(queue, tid, 0);
}

proof fn count_tid_in_queue_from_push_other(queue: Seq<usize>, tid: usize, added: usize, from: int)
    requires
        tid != added,
        0 <= from <= queue.len(),
    ensures
        count_tid_in_queue_from_spec(queue.push(added), tid, from)
            == count_tid_in_queue_from_spec(queue, tid, from),
    decreases queue.len() - from,
{
    if from < queue.len() {
        count_tid_in_queue_from_push_other(queue, tid, added, from + 1);
        assert(queue.push(added)[from] == queue[from]);
        if queue[from] == tid {
            assert(count_tid_in_queue_from_spec(queue.push(added), tid, from)
                == count_tid_in_queue_from_spec(queue.push(added), tid, from + 1) + 1nat);
            assert(count_tid_in_queue_from_spec(queue, tid, from)
                == count_tid_in_queue_from_spec(queue, tid, from + 1) + 1nat);
        } else {
            assert(count_tid_in_queue_from_spec(queue.push(added), tid, from)
                == count_tid_in_queue_from_spec(queue.push(added), tid, from + 1));
            assert(count_tid_in_queue_from_spec(queue, tid, from)
                == count_tid_in_queue_from_spec(queue, tid, from + 1));
        }
    } else {
        assert(from == queue.len());
        assert(queue.push(added)[from] == added);
        assert(count_tid_in_queue_from_spec(queue.push(added), tid, from + 1) == 0nat);
        assert(count_tid_in_queue_from_spec(queue.push(added), tid, from) == 0nat);
        assert(count_tid_in_queue_from_spec(queue, tid, from) == 0nat);
    }
}

proof fn count_tid_in_queue_push_other(queue: Seq<usize>, tid: usize, added: usize)
    requires
        tid != added,
    ensures
        count_tid_in_queue_spec(queue.push(added), tid) == count_tid_in_queue_spec(queue, tid),
{
    count_tid_in_queue_from_push_other(queue, tid, added, 0);
}

proof fn count_tid_in_queues_from_update_push_same(queues: Seq<Seq<usize>>, priority: int, tid: usize, from: int)
    requires
        0 <= priority < queues.len(),
        0 <= from <= queues.len(),
    ensures
        count_tid_in_queues_from_spec(queues.update(priority, queues[priority].push(tid)), tid, from)
            == count_tid_in_queues_from_spec(queues, tid, from)
                + if from <= priority { 1nat } else { 0nat },
    decreases queues.len() - from,
{
    if from < queues.len() {
        count_tid_in_queues_from_update_push_same(queues, priority, tid, from + 1);
        if from == priority {
            count_tid_in_queue_push_same(queues[priority], tid);
            assert(queues.update(priority, queues[priority].push(tid))[from] == queues[priority].push(tid));
        } else {
            assert(queues.update(priority, queues[priority].push(tid))[from] == queues[from]);
        }
    }
}

proof fn count_tid_in_queues_update_push_same(queues: Seq<Seq<usize>>, priority: int, tid: usize)
    requires
        0 <= priority < queues.len(),
    ensures
        count_tid_in_queues_spec(queues.update(priority, queues[priority].push(tid)), tid)
            == count_tid_in_queues_spec(queues, tid) + 1nat,
{
    count_tid_in_queues_from_update_push_same(queues, priority, tid, 0);
}

proof fn count_tid_in_queues_from_update_push_other(queues: Seq<Seq<usize>>, priority: int, tid: usize, added: usize, from: int)
    requires
        tid != added,
        0 <= priority < queues.len(),
        0 <= from <= queues.len(),
    ensures
        count_tid_in_queues_from_spec(queues.update(priority, queues[priority].push(added)), tid, from)
            == count_tid_in_queues_from_spec(queues, tid, from),
    decreases queues.len() - from,
{
    if from < queues.len() {
        count_tid_in_queues_from_update_push_other(queues, priority, tid, added, from + 1);
        if from == priority {
            count_tid_in_queue_push_other(queues[priority], tid, added);
            assert(queues.update(priority, queues[priority].push(added))[from] == queues[priority].push(added));
        } else {
            assert(queues.update(priority, queues[priority].push(added))[from] == queues[from]);
        }
    }
}

proof fn count_tid_in_queues_update_push_other(queues: Seq<Seq<usize>>, priority: int, tid: usize, added: usize)
    requires
        tid != added,
        0 <= priority < queues.len(),
    ensures
        count_tid_in_queues_spec(queues.update(priority, queues[priority].push(added)), tid)
            == count_tid_in_queues_spec(queues, tid),
{
    count_tid_in_queues_from_update_push_other(queues, priority, tid, added, 0);
}

proof fn tcb_update_keeps_other(tcbs: Seq<Tcb>, index: int, updated: Tcb, other: int)
    requires
        0 <= index < tcbs.len(),
        0 <= other < tcbs.len(),
        other != index,
    ensures
        tcbs.update(index, updated)[other] == tcbs[other],
{}

proof fn running_transition_blocks_on_send(tcb: Tcb, endpoint: usize)
    requires
        is_running_tcb_state_spec(tcb.state),
    ensures
        is_blocked_on_send_tcb_state_spec(block_on_send_transition_spec(tcb, endpoint).state, endpoint as int),
{
    assert(is_blocked_on_send_tcb_state_spec(block_on_send_transition_spec(tcb, endpoint).state, endpoint as int));
}

proof fn ints_below_upper_cast_to_distinct_usizes(a: int, b: int, upper: usize)
    requires
        0 <= a < upper as int,
        0 <= b < upper as int,
        a != b,
    ensures
        a as usize != b as usize,
{}

// r[verify sel4-invariants.step-preservation]
// r[verify sel4-invariants.step-preservation-expansion]
pub proof fn step_endpoint_send_block_preserves_kernel_invariant(
    before: KernelState,
    after: KernelState,
    endpoint_idx: int,
    sender: int,
    badge: u64,
)
    requires
        kernel_invariant_spec(before),
        0 <= endpoint_idx < before.endpoints@.len(),
        0 <= sender < before.tcbs@.len(),
        is_running_tcb_state_spec(before.tcbs@[sender].state),
        before.endpoints@[endpoint_idx].receivers@.len() == 0,
        before.endpoints@[endpoint_idx].can_enqueue_sender(sender),
        after.tcbs@ =~= before.tcbs@.update(sender, after.tcbs@[sender]),
        is_blocked_on_send_tcb_state_spec(after.tcbs@[sender].state, endpoint_idx),
        after.ready_queues.queues@ =~= before.ready_queues.queues@,
        after.endpoints@ =~= before.endpoints@.update(endpoint_idx, after.endpoints@[endpoint_idx]),
        after.endpoints@[endpoint_idx].wf(),
        after.endpoints@[endpoint_idx].receivers@ =~= before.endpoints@[endpoint_idx].receivers@,
        after.endpoints@[endpoint_idx].senders@
            == before.endpoints@[endpoint_idx].senders@.push(SenderWaiter { tcb: sender as usize, badge }),
        after.notification_count == before.notification_count,
        after.reply_count == before.reply_count,
        after.cdt.parents@ =~= before.cdt.parents@,
        after.cdt.slots@ =~= before.cdt.slots@,
        after.cdt.present@ =~= before.cdt.present@,
        after.cspace.nodes@ =~= before.cspace.nodes@,
        after.cspace.root == before.cspace.root,
        after.untypeds@ =~= before.untypeds@,
        after.asid_pool.ownership_view() =~= before.asid_pool.ownership_view(),
        after.asid_pool.capacity == before.asid_pool.capacity,
        after.vspaces@ =~= before.vspaces@,
        after.irqs@ =~= before.irqs@,
    ensures
        kernel_invariant_spec(after),
{
    let before_queues = queue_view_spec(before.ready_queues.queues@);
    blocked_thread_not_queued(before_queues, before.tcbs@, sender);

    assert(object_refs_valid_spec(after)) by {
        assert forall|idx: int| #![trigger after.endpoints@[idx]]
            0 <= idx < after.endpoints@.len() implies after.endpoints@[idx].wf() by {
            if idx == endpoint_idx {
            } else {
                assert(after.endpoints@[idx] == before.endpoints@[idx]);
            }
        };
        assert forall|tid: int| #![trigger after.tcbs@[tid]]
            0 <= tid < after.tcbs@.len() implies match after.tcbs@[tid].state {
                TcbState::BlockedOnSend { endpoint } | TcbState::BlockedOnReceive { endpoint } => endpoint < after.endpoints@.len(),
                TcbState::BlockedOnNotification { notification } => notification < after.notification_count,
                TcbState::BlockedOnReply { reply } => reply < after.reply_count,
                TcbState::Runnable | TcbState::Running | TcbState::Restart | TcbState::Inactive => true,
            } by {
            if 0 <= tid < after.tcbs@.len() {
                if tid == sender {
                    assert(is_blocked_on_send_tcb_state_spec(after.tcbs@[tid].state, endpoint_idx));
                } else {
                    assert(tid != sender);
                    assert(after.tcbs@.len() == before.tcbs@.len());
                    assert(0 <= tid < before.tcbs@.len()) by(nonlinear_arith)
                        requires 0 <= tid, tid < after.tcbs@.len(), after.tcbs@.len() == before.tcbs@.len();
                    tcb_update_keeps_other(before.tcbs@, sender, after.tcbs@[sender], tid);
                    assert(after.tcbs@[tid] == before.tcbs@[tid]);
                }
            }
        };
        assert forall|idx: int, pos: int| #![trigger after.endpoints@[idx].senders@[pos]]
            0 <= idx < after.endpoints@.len() && 0 <= pos < after.endpoints@[idx].senders@.len()
                implies after.endpoints@[idx].senders@[pos].tcb < after.tcbs@.len() by {
            if idx == endpoint_idx {
                if pos < before.endpoints@[endpoint_idx].senders@.len() {
                    assert(after.endpoints@[idx].senders@[pos] == before.endpoints@[idx].senders@[pos]);
                } else {
                    assert(pos == before.endpoints@[endpoint_idx].senders@.len());
                    assert(after.endpoints@[idx].senders@[pos] == SenderWaiter { tcb: sender as usize, badge });
                }
            } else {
                assert(after.endpoints@[idx].senders@[pos] == before.endpoints@[idx].senders@[pos]);
            }
        };
        assert forall|idx: int, pos: int| #![trigger after.endpoints@[idx].receivers@[pos]]
            0 <= idx < after.endpoints@.len() && 0 <= pos < after.endpoints@[idx].receivers@.len()
                implies after.endpoints@[idx].receivers@[pos] < after.tcbs@.len() by {
            assert(after.endpoints@[idx].receivers@[pos] == before.endpoints@[idx].receivers@[pos]);
        };
        assert forall|irq_idx: int| #![trigger after.irqs@[irq_idx]]
            0 <= irq_idx < after.irqs@.len() implies match after.irqs@[irq_idx].handler {
                Some(handler) => handler < after.notification_count,
                None => true,
            } by {
            assert(after.irqs@[irq_idx] == before.irqs@[irq_idx]);
        };
    };
    assert(cdt_wf_spec(after));
    assert(cspace_valid_spec(after));
    assert(tcb_legal_spec(after));
    assert(scheduler_consistent_spec(after)) by {
        assert forall|tid: int|
            #![trigger after.tcbs@[tid]]
            #![trigger count_tid_in_queues_spec(queue_view_spec(after.ready_queues.queues@), tid as usize)]
            0 <= tid < after.tcbs@.len()
                ==> {
                    let priority = after.tcbs@[tid].priority as int;
                    if runnable_state_spec(after.tcbs@[tid].state) {
                        &&& 0 <= priority < queue_view_spec(after.ready_queues.queues@).len()
                        &&& count_tid_in_queues_spec(queue_view_spec(after.ready_queues.queues@), tid as usize) == 1nat
                        &&& queue_contains_spec(queue_view_spec(after.ready_queues.queues@)[priority], tid as usize)
                    } else {
                        count_tid_in_queues_spec(queue_view_spec(after.ready_queues.queues@), tid as usize) == 0nat
                    }
                } by {
            if 0 <= tid < after.tcbs@.len() {
                if tid == sender {
                    assert(!runnable_state_spec(after.tcbs@[tid].state));
                    assert(count_tid_in_queues_spec(queue_view_spec(after.ready_queues.queues@), tid as usize) == 0nat);
                } else {
                    assert(tid != sender);
                    assert(after.tcbs@.len() == before.tcbs@.len());
                    assert(0 <= tid < before.tcbs@.len()) by(nonlinear_arith)
                        requires 0 <= tid, tid < after.tcbs@.len(), after.tcbs@.len() == before.tcbs@.len();
                    tcb_update_keeps_other(before.tcbs@, sender, after.tcbs@[sender], tid);
                    assert(after.tcbs@[tid] == before.tcbs@[tid]);
                    assert(queue_view_spec(after.ready_queues.queues@) =~= before_queues);
                }
            }
        };
        assert forall|priority: int, pos: int|
            #![trigger queue_view_spec(after.ready_queues.queues@)[priority][pos]]
            0 <= priority < queue_view_spec(after.ready_queues.queues@).len()
                && 0 <= pos < queue_view_spec(after.ready_queues.queues@)[priority].len()
                ==> {
                    let tid = queue_view_spec(after.ready_queues.queues@)[priority][pos];
                    &&& (tid as int) < after.tcbs@.len()
                    &&& runnable_state_spec(after.tcbs@[tid as int].state)
                    &&& after.tcbs@[tid as int].priority as int == priority
                } by {
            if 0 <= priority < queue_view_spec(after.ready_queues.queues@).len()
                && 0 <= pos < queue_view_spec(after.ready_queues.queues@)[priority].len() {
                let tid = queue_view_spec(after.ready_queues.queues@)[priority][pos];
                assert(queue_view_spec(after.ready_queues.queues@)[priority][pos] == before_queues[priority][pos]);
                if tid == sender as usize {
                    assert(queues_contain_spec(before_queues, sender as usize)) by {
                        assert(before_queues[priority][pos] == sender as usize);
                    }
                    assert(false);
                }
                assert(tid as int != sender);
                assert(0 <= (tid as int));
                assert((tid as int) < before.tcbs@.len());
                tcb_update_keeps_other(before.tcbs@, sender, after.tcbs@[sender], tid as int);
                assert(after.tcbs@[tid as int] == before.tcbs@[tid as int]);
            }
        };
    };
    assert(untyped_ownership_spec(after));
    assert(asid_pool_wf_spec(after));
    assert(vspace_consistent_spec(after));
    assert(irq_unique_spec(after));
}

proof fn all_untypeds_wf_preserved_by_update(untypeds: Seq<UntypedMemory>, index: int, updated: UntypedMemory)
    requires
        forall|idx: int| #![trigger untypeds[idx]] 0 <= idx < untypeds.len() ==> untypeds[idx].wf(),
        0 <= index < untypeds.len(),
        updated.wf(),
    ensures
        forall|idx: int| #![trigger untypeds.update(index, updated)[idx]]
            0 <= idx < untypeds.update(index, updated).len()
                ==> untypeds.update(index, updated)[idx].wf(),
{
    assert forall|idx: int| #![trigger untypeds.update(index, updated)[idx]]
        0 <= idx < untypeds.update(index, updated).len()
            implies untypeds.update(index, updated)[idx].wf() by {
        if idx == index {
            assert(untypeds.update(index, updated)[idx] == updated);
        } else {
            assert(untypeds.update(index, updated)[idx] == untypeds[idx]);
        }
    }
}

proof fn top_level_untypeds_disjoint_preserved_by_update(untypeds: Seq<UntypedMemory>, index: int, updated: UntypedMemory)
    requires
        top_level_untypeds_disjoint_spec(untypeds),
        0 <= index < untypeds.len(),
        updated.base == untypeds[index].base,
        updated.size == untypeds[index].size,
    ensures
        top_level_untypeds_disjoint_spec(untypeds.update(index, updated)),
{
    let new_untypeds = untypeds.update(index, updated);
    assert forall|i: int, j: int| #![trigger new_untypeds[i], new_untypeds[j]]
        0 <= i < j < new_untypeds.len()
            implies regions_disjoint_spec(new_untypeds[i].base, new_untypeds[i].size, new_untypeds[j].base, new_untypeds[j].size) by {
        if i == index {
            assert(new_untypeds[i] == updated);
            if j == index {
                assert(false);
            } else {
                assert(new_untypeds[j] == untypeds[j]);
                assert(updated.base == untypeds[index].base);
                assert(updated.size == untypeds[index].size);
            }
        } else if j == index {
            assert(new_untypeds[j] == updated);
            assert(new_untypeds[i] == untypeds[i]);
            assert(updated.base == untypeds[index].base);
            assert(updated.size == untypeds[index].size);
        } else {
            assert(new_untypeds[i] == untypeds[i]);
            assert(new_untypeds[j] == untypeds[j]);
        }
    }
}

// r[verify sel4-invariants.step-preservation]
// r[verify sel4-invariants.step-preservation-expansion]
pub proof fn step_untyped_revoke_preserves_kernel_invariant(before: KernelState, after: KernelState, index: int)
    requires
        kernel_invariant_spec(before),
        0 <= index < before.untypeds@.len(),
        after.tcbs@ =~= before.tcbs@,
        after.ready_queues.queues@ =~= before.ready_queues.queues@,
        after.endpoints@ =~= before.endpoints@,
        after.notification_count == before.notification_count,
        after.reply_count == before.reply_count,
        after.cdt.parents@ =~= before.cdt.parents@,
        after.cdt.slots@ =~= before.cdt.slots@,
        after.cdt.present@ =~= before.cdt.present@,
        after.cspace.nodes@ =~= before.cspace.nodes@,
        after.cspace.root == before.cspace.root,
        after.untypeds@ =~= before.untypeds@.update(index, after.untypeds@[index]),
        after.untypeds@[index].base == before.untypeds@[index].base,
        after.untypeds@[index].size == before.untypeds@[index].size,
        after.untypeds@[index].wf(),
        after.asid_pool.ownership_view() =~= before.asid_pool.ownership_view(),
        after.asid_pool.capacity == before.asid_pool.capacity,
        after.vspaces@ =~= before.vspaces@,
        after.irqs@ =~= before.irqs@,
    ensures
        kernel_invariant_spec(after),
{
    assert(object_refs_valid_spec(after));
    assert(cdt_wf_spec(after));
    assert(cspace_valid_spec(after));
    assert(tcb_legal_spec(after));
    assert(scheduler_consistent_spec(after));
    all_untypeds_wf_preserved_by_update(before.untypeds@, index, after.untypeds@[index]);
    top_level_untypeds_disjoint_preserved_by_update(before.untypeds@, index, after.untypeds@[index]);
    assert(untyped_ownership_spec(after));
    assert(asid_pool_wf_spec(after));
    assert(vspace_consistent_spec(after));
    assert(irq_unique_spec(after));
}

fn clone_senders(senders: &Vec<SenderWaiter>) -> (result: Vec<SenderWaiter>)
    ensures
        result@ =~= senders@,
{
    let mut out: Vec<SenderWaiter> = Vec::new();
    let mut i: usize = 0;
    while i < senders.len()
        invariant
            0 <= i <= senders@.len(),
            out@ =~= senders@.subrange(0, i as int),
        decreases senders@.len() - i,
    {
        out.push(senders[i]);
        proof {
            assert(senders@.subrange(0, i as int + 1) == senders@.subrange(0, i as int).push(senders@[i as int]));
        }
        i += 1;
    }
    out
}

fn clone_receivers(receivers: &Vec<usize>) -> (result: Vec<usize>)
    ensures
        result@ =~= receivers@,
{
    let mut out: Vec<usize> = Vec::new();
    let mut i: usize = 0;
    while i < receivers.len()
        invariant
            0 <= i <= receivers@.len(),
            out@ =~= receivers@.subrange(0, i as int),
        decreases receivers@.len() - i,
    {
        out.push(receivers[i]);
        proof {
            assert(receivers@.subrange(0, i as int + 1) == receivers@.subrange(0, i as int).push(receivers@[i as int]));
        }
        i += 1;
    }
    out
}

fn clone_endpoint(endpoint: &Endpoint) -> (result: Endpoint)
    ensures
        result.senders@ =~= endpoint.senders@,
        result.receivers@ =~= endpoint.receivers@,
        result.wf() == endpoint.wf(),
{
    let senders = clone_senders(&endpoint.senders);
    let receivers = clone_receivers(&endpoint.receivers);
    proof {
        assert(senders@ =~= endpoint.senders@);
        assert(receivers@ =~= endpoint.receivers@);
    }
    Endpoint { senders, receivers }
}

fn clone_children(children: &Vec<ChildObject>) -> (result: Vec<ChildObject>)
    ensures
        result@ =~= children@,
{
    let mut out: Vec<ChildObject> = Vec::new();
    let mut i: usize = 0;
    while i < children.len()
        invariant
            0 <= i <= children@.len(),
            out@ =~= children@.subrange(0, i as int),
        decreases children@.len() - i,
    {
        out.push(children[i]);
        proof {
            assert(children@.subrange(0, i as int + 1) == children@.subrange(0, i as int).push(children@[i as int]));
        }
        i += 1;
    }
    out
}

fn clone_untyped(untyped: &UntypedMemory) -> (result: UntypedMemory)
    ensures
        result.base == untyped.base,
        result.size == untyped.size,
        result.child_view() =~= untyped.child_view(),
        result.wf() == untyped.wf(),
{
    let children = clone_children(&untyped.children);
    proof {
        assert(children@ =~= untyped.child_view());
    }
    UntypedMemory {
        base: untyped.base,
        size: untyped.size,
        children,
    }
}

// r[verify sel4-invariants.step-preservation]
// r[verify sel4-invariants.step-preservation-expansion]
pub proof fn step_endpoint_send_rendezvous_preserves_kernel_invariant(
    before: KernelState,
    after: KernelState,
    endpoint_idx: int,
    sender: int,
    receiver: int,
    badge: u64,
)
    requires
        kernel_invariant_spec(before),
        0 <= endpoint_idx < before.endpoints@.len(),
        0 <= sender < before.tcbs@.len(),
        0 <= receiver < before.tcbs@.len(),
        is_running_tcb_state_spec(before.tcbs@[sender].state),
        before.endpoints@[endpoint_idx].receivers@.len() > 0,
        receiver == before.endpoints@[endpoint_idx].receivers@[0],
        before.endpoints@[endpoint_idx].can_enqueue_sender(sender),
        is_blocked_on_receive_tcb_state_spec(before.tcbs@[receiver].state, endpoint_idx),
        (before.tcbs@[receiver].priority as int) < before.ready_queues.queues@.len(),
        after.tcbs@ =~= before.tcbs@.update(receiver, after.tcbs@[receiver]),
        after.tcbs@[receiver] == wake_transition_spec(before.tcbs@[receiver]),
        queue_view_spec(after.ready_queues.queues@)
            =~= queue_view_spec(before.ready_queues.queues@).update(
                before.tcbs@[receiver].priority as int,
                queue_view_spec(before.ready_queues.queues@)[before.tcbs@[receiver].priority as int].push(receiver as usize),
            ),
        after.endpoints@ =~= before.endpoints@.update(endpoint_idx, after.endpoints@[endpoint_idx]),
        after.endpoints@[endpoint_idx].wf(),
        after.endpoints@[endpoint_idx].senders@ =~= before.endpoints@[endpoint_idx].senders@,
        after.endpoints@[endpoint_idx].receivers@
            =~= before.endpoints@[endpoint_idx].receivers@.subrange(1, before.endpoints@[endpoint_idx].receivers@.len() as int),
        after.notification_count == before.notification_count,
        after.reply_count == before.reply_count,
        after.cdt.parents@ =~= before.cdt.parents@,
        after.cdt.slots@ =~= before.cdt.slots@,
        after.cdt.present@ =~= before.cdt.present@,
        after.cspace.nodes@ =~= before.cspace.nodes@,
        after.cspace.root == before.cspace.root,
        after.untypeds@ =~= before.untypeds@,
        after.asid_pool.ownership_view() =~= before.asid_pool.ownership_view(),
        after.asid_pool.capacity == before.asid_pool.capacity,
        after.vspaces@ =~= before.vspaces@,
        after.irqs@ =~= before.irqs@,
    ensures
        kernel_invariant_spec(after),
{
    let before_queues = queue_view_spec(before.ready_queues.queues@);
    let after_queues = queue_view_spec(after.ready_queues.queues@);
    let priority = before.tcbs@[receiver].priority as int;
    blocked_thread_not_queued(before_queues, before.tcbs@, receiver);
    count_tid_in_queues_update_push_same(before_queues, priority, receiver as usize);

    assert(object_refs_valid_spec(after)) by {
        assert forall|idx: int| #![trigger after.endpoints@[idx]]
            0 <= idx < after.endpoints@.len() implies after.endpoints@[idx].wf() by {
            if idx == endpoint_idx {
            } else {
                assert(after.endpoints@[idx] == before.endpoints@[idx]);
            }
        };
        assert forall|tid: int| #![trigger after.tcbs@[tid]]
            0 <= tid < after.tcbs@.len() implies match after.tcbs@[tid].state {
                TcbState::BlockedOnSend { endpoint } | TcbState::BlockedOnReceive { endpoint } => endpoint < after.endpoints@.len(),
                TcbState::BlockedOnNotification { notification } => notification < after.notification_count,
                TcbState::BlockedOnReply { reply } => reply < after.reply_count,
                TcbState::Runnable | TcbState::Running | TcbState::Restart | TcbState::Inactive => true,
            } by {
            if tid == receiver {
                assert(is_runnable_tcb_state_spec(after.tcbs@[tid].state));
            } else {
                assert(tid != receiver);
                assert(after.tcbs@.len() == before.tcbs@.len());
                assert(0 <= tid < before.tcbs@.len()) by(nonlinear_arith)
                    requires 0 <= tid, tid < after.tcbs@.len(), after.tcbs@.len() == before.tcbs@.len();
                tcb_update_keeps_other(before.tcbs@, receiver, after.tcbs@[receiver], tid);
                assert(after.tcbs@[tid] == before.tcbs@[tid]);
            }
        };
        assert forall|idx: int, pos: int| #![trigger after.endpoints@[idx].senders@[pos]]
            0 <= idx < after.endpoints@.len() && 0 <= pos < after.endpoints@[idx].senders@.len()
                implies after.endpoints@[idx].senders@[pos].tcb < after.tcbs@.len() by {
            assert(after.endpoints@[idx].senders@[pos] == before.endpoints@[idx].senders@[pos]);
        };
        assert forall|idx: int, pos: int| #![trigger after.endpoints@[idx].receivers@[pos]]
            0 <= idx < after.endpoints@.len() && 0 <= pos < after.endpoints@[idx].receivers@.len()
                implies after.endpoints@[idx].receivers@[pos] < after.tcbs@.len() by {
            if idx == endpoint_idx {
                assert(after.endpoints@[idx].receivers@[pos] == before.endpoints@[idx].receivers@[pos + 1]);
            } else {
                assert(after.endpoints@[idx].receivers@[pos] == before.endpoints@[idx].receivers@[pos]);
            }
        };
        assert forall|irq_idx: int| #![trigger after.irqs@[irq_idx]]
            0 <= irq_idx < after.irqs@.len() implies match after.irqs@[irq_idx].handler {
                Some(handler) => handler < after.notification_count,
                None => true,
            } by {
            assert(after.irqs@[irq_idx] == before.irqs@[irq_idx]);
        };
    };
    assert(cdt_wf_spec(after));
    assert(cspace_valid_spec(after));
    assert(tcb_legal_spec(after));
    assert(scheduler_consistent_spec(after)) by {
        assert forall|tid: int|
            #![trigger after.tcbs@[tid]]
            #![trigger count_tid_in_queues_spec(after_queues, tid as usize)]
            0 <= tid < after.tcbs@.len()
                ==> {
                    let tid_priority = after.tcbs@[tid].priority as int;
                    if runnable_state_spec(after.tcbs@[tid].state) {
                        &&& 0 <= tid_priority < after_queues.len()
                        &&& count_tid_in_queues_spec(after_queues, tid as usize) == 1nat
                        &&& queue_contains_spec(after_queues[tid_priority], tid as usize)
                    } else {
                        count_tid_in_queues_spec(after_queues, tid as usize) == 0nat
                    }
                } by {
            if 0 <= tid < after.tcbs@.len() {
                let tid_priority = after.tcbs@[tid].priority as int;
                if tid == receiver {
                    assert(is_runnable_tcb_state_spec(after.tcbs@[tid].state));
                    assert(0 <= tid_priority < after_queues.len());
                    assert(count_tid_in_queues_spec(after_queues, tid as usize) == 1nat);
                    assert(queue_contains_spec(after_queues[tid_priority], tid as usize)) by {
                        assert(after_queues[tid_priority] == before_queues[priority].push(receiver as usize));
                        assert(tid_priority == priority);
                        assert(after_queues[tid_priority][before_queues[priority].len() as int] == tid as usize);
                    };
                } else {
                    assert(tid != receiver);
                    assert(0 <= receiver);
                    assert(after.tcbs@.len() == before.tcbs@.len());
                    assert(before.tcbs@.len() == before.tcbs.len() as int);
                    assert(tid < before.tcbs.len() as int) by(nonlinear_arith)
                        requires tid < after.tcbs@.len(), after.tcbs@.len() == before.tcbs@.len(), before.tcbs@.len() == before.tcbs.len() as int;
                    assert(receiver < before.tcbs.len() as int) by(nonlinear_arith)
                        requires receiver < before.tcbs@.len(), before.tcbs@.len() == before.tcbs.len() as int;
                    ints_below_upper_cast_to_distinct_usizes(tid, receiver, before.tcbs.len());
                    count_tid_in_queues_update_push_other(before_queues, priority, tid as usize, receiver as usize);
                    assert(after.tcbs@.len() == before.tcbs@.len());
                    assert(0 <= tid < before.tcbs@.len()) by(nonlinear_arith)
                        requires 0 <= tid, tid < after.tcbs@.len(), after.tcbs@.len() == before.tcbs@.len();
                    tcb_update_keeps_other(before.tcbs@, receiver, after.tcbs@[receiver], tid);
                    assert(after.tcbs@[tid] == before.tcbs@[tid]);
                    if runnable_state_spec(after.tcbs@[tid].state) {
                        assert(0 <= tid_priority < after_queues.len());
                        assert(count_tid_in_queues_spec(after_queues, tid as usize) == 1nat);
                        assert(queue_contains_spec(after_queues[tid_priority], tid as usize)) by {
                            let pos = choose|pos: int| 0 <= pos < before_queues[tid_priority].len() && before_queues[tid_priority][pos] == tid as usize;
                            if tid_priority == priority {
                                assert(after_queues[tid_priority] == before_queues[priority].push(receiver as usize));
                                assert(after_queues[tid_priority][pos] == tid as usize);
                            } else {
                                assert(after_queues[tid_priority] == before_queues[tid_priority]);
                                assert(after_queues[tid_priority][pos] == tid as usize);
                            }
                        };
                    } else {
                        assert(count_tid_in_queues_spec(after_queues, tid as usize) == 0nat);
                    }
                }
            }
        };
        assert forall|q: int, pos: int|
            #![trigger after_queues[q][pos]]
            0 <= q < after_queues.len() && 0 <= pos < after_queues[q].len()
                ==> {
                    let tid = after_queues[q][pos];
                    &&& (tid as int) < after.tcbs@.len()
                    &&& runnable_state_spec(after.tcbs@[tid as int].state)
                    &&& after.tcbs@[tid as int].priority as int == q
                } by {
            if 0 <= q < after_queues.len() && 0 <= pos < after_queues[q].len() {
                let tid = after_queues[q][pos];
                if q == priority && pos == before_queues[priority].len() {
                    assert(tid == receiver as usize);
                    assert((tid as int) < after.tcbs@.len());
                    assert(runnable_state_spec(after.tcbs@[tid as int].state));
                    assert(after.tcbs@[tid as int].priority as int == q);
                } else if q == priority {
                    assert(after_queues[q] == before_queues[q].push(receiver as usize));
                    assert(tid == before_queues[q][pos]);
                    if tid as int == receiver {
                        assert((tid as int) < after.tcbs@.len());
                        assert(runnable_state_spec(after.tcbs@[tid as int].state));
                        assert(after.tcbs@[tid as int].priority as int == q);
                    } else {
                        assert(tid as int != receiver);
                        assert(after.tcbs@.len() == before.tcbs@.len());
                        assert(0 <= (tid as int) < before.tcbs@.len());
                        tcb_update_keeps_other(before.tcbs@, receiver, after.tcbs@[receiver], tid as int);
                        assert(after.tcbs@[tid as int] == before.tcbs@[tid as int]);
                    }
                } else {
                    assert(after_queues[q] == before_queues[q]);
                    assert(tid == before_queues[q][pos]);
                    if tid as int == receiver {
                        assert(queues_contain_spec(before_queues, receiver as usize));
                        assert(false);
                    }
                    assert(tid as int != receiver);
                    assert(after.tcbs@.len() == before.tcbs@.len());
                    assert(0 <= (tid as int) < before.tcbs@.len());
                    tcb_update_keeps_other(before.tcbs@, receiver, after.tcbs@[receiver], tid as int);
                    assert(after.tcbs@[tid as int] == before.tcbs@[tid as int]);
                }
            }
        };
    };
    assert(untyped_ownership_spec(after));
    assert(asid_pool_wf_spec(after));
    assert(vspace_consistent_spec(after));
    assert(irq_unique_spec(after));
}

// r[impl sel4-invariants.step-preservation]
pub fn step_endpoint_send_block(state: &mut KernelState, endpoint_idx: usize, sender: usize, badge: u64)
    requires
        kernel_invariant_spec(*old(state)),
        endpoint_idx < old(state).endpoints@.len(),
        sender < old(state).tcbs@.len(),
        is_running_tcb_state_spec(old(state).tcbs@[sender as int].state),
        old(state).endpoints@[endpoint_idx as int].receivers@.len() == 0,
        old(state).endpoints@[endpoint_idx as int].can_enqueue_sender(sender as int),
    ensures
        kernel_invariant_spec(*state),
        is_blocked_on_send_tcb_state_spec(state.tcbs@[sender as int].state, endpoint_idx as int),
        state.endpoints@[endpoint_idx as int].senders@[state.endpoints@[endpoint_idx as int].senders@.len() - 1].tcb == sender,
        state.endpoints@[endpoint_idx as int].senders@[state.endpoints@[endpoint_idx as int].senders@.len() - 1].badge == badge,
{
    let mut endpoint = clone_endpoint(&state.endpoints[endpoint_idx]);
    let send_result = send(&mut endpoint, sender, badge);
    let mut sender_tcb = state.tcbs[sender];
    let blocked = block_on_send(&mut sender_tcb, endpoint_idx);
    state.tcbs.set(sender, sender_tcb);
    state.endpoints.set(endpoint_idx, endpoint);
    proof {
        assert(send_result.is_none());
        assert(blocked);
        assert(state.tcbs@[sender as int] == block_on_send_transition_spec(old(state).tcbs@[sender as int], endpoint_idx));
        assert(state.endpoints@[endpoint_idx as int].wf());
        assert(state.endpoints@[endpoint_idx as int].receivers@ =~= old(state).endpoints@[endpoint_idx as int].receivers@);
        assert(state.endpoints@[endpoint_idx as int].senders@
            == old(state).endpoints@[endpoint_idx as int].senders@.push(SenderWaiter { tcb: sender, badge }));
        assert(state.tcbs@ =~= old(state).tcbs@.update(sender as int, state.tcbs@[sender as int]));
        assert(state.endpoints@ =~= old(state).endpoints@.update(endpoint_idx as int, state.endpoints@[endpoint_idx as int]));
        step_endpoint_send_block_preserves_kernel_invariant(*old(state), *state, endpoint_idx as int, sender as int, badge);
    }
}

// r[impl sel4-invariants.step-preservation]
pub fn step_endpoint_send_rendezvous(
    state: &mut KernelState,
    endpoint_idx: usize,
    sender: usize,
    badge: u64,
) -> (result: Rendezvous)
    requires
        kernel_invariant_spec(*old(state)),
        endpoint_idx < old(state).endpoints@.len(),
        sender < old(state).tcbs@.len(),
        is_running_tcb_state_spec(old(state).tcbs@[sender as int].state),
        old(state).endpoints@[endpoint_idx as int].receivers@.len() > 0,
        old(state).endpoints@[endpoint_idx as int].can_enqueue_sender(sender as int),
        old(state).endpoints@[endpoint_idx as int].receivers@[0] < old(state).tcbs@.len(),
        is_blocked_on_receive_tcb_state_spec(
            old(state).tcbs@[old(state).endpoints@[endpoint_idx as int].receivers@[0] as int].state,
            endpoint_idx as int,
        ),
        (old(state).tcbs@[old(state).endpoints@[endpoint_idx as int].receivers@[0] as int].priority as int)
            < old(state).ready_queues.queues@.len(),
    ensures
        kernel_invariant_spec(*state),
        result.sender == sender,
        result.receiver == old(state).endpoints@[endpoint_idx as int].receivers@[0],
        result.badge == badge,
        is_runnable_tcb_state_spec(state.tcbs@[result.receiver as int].state),
{
    let mut endpoint = clone_endpoint(&state.endpoints[endpoint_idx]);
    let matched = send(&mut endpoint, sender, badge);
    proof {
        assert(matched.is_some());
    }
    let result = matched.unwrap();
    let mut receiver_tcb = state.tcbs[result.receiver];
    let woke = wake(&mut receiver_tcb);
    state.tcbs.set(result.receiver, receiver_tcb);
    let enqueued = enqueue(&mut state.ready_queues, &state.tcbs, result.receiver);
    state.endpoints.set(endpoint_idx, endpoint);
    proof {
        assert(woke);
        assert(enqueued);
        assert(result.receiver == old(state).endpoints@[endpoint_idx as int].receivers@[0]);
        assert(result.sender == sender);
        assert(result.badge == badge);
        assert(state.tcbs@[result.receiver as int] == wake_transition_spec(old(state).tcbs@[result.receiver as int]));
        assert(queue_view_spec(state.ready_queues.queues@)
            =~= queue_view_spec(old(state).ready_queues.queues@).update(
                old(state).tcbs@[result.receiver as int].priority as int,
                queue_view_spec(old(state).ready_queues.queues@)[old(state).tcbs@[result.receiver as int].priority as int]
                    .push(result.receiver),
            ));
        assert(state.endpoints@[endpoint_idx as int].wf());
        assert(state.endpoints@[endpoint_idx as int].senders@ =~= old(state).endpoints@[endpoint_idx as int].senders@);
        assert(state.endpoints@[endpoint_idx as int].receivers@
            =~= old(state).endpoints@[endpoint_idx as int].receivers@.subrange(1, old(state).endpoints@[endpoint_idx as int].receivers@.len() as int));
        assert(state.tcbs@ =~= old(state).tcbs@.update(result.receiver as int, state.tcbs@[result.receiver as int]));
        assert(state.endpoints@ =~= old(state).endpoints@.update(endpoint_idx as int, state.endpoints@[endpoint_idx as int]));
        step_endpoint_send_rendezvous_preserves_kernel_invariant(
            *old(state),
            *state,
            endpoint_idx as int,
            sender as int,
            result.receiver as int,
            badge,
        );
    }
    result
}

// r[impl sel4-invariants.step-preservation]
pub fn step_untyped_revoke(state: &mut KernelState, index: usize)
    requires
        kernel_invariant_spec(*old(state)),
        index < old(state).untypeds@.len(),
    ensures
        kernel_invariant_spec(*state),
        state.untypeds@[index as int].child_view().len() == 0,
{
    let mut updated = clone_untyped(&state.untypeds[index]);
    proof {
        assert(old(state).untypeds@[index as int].wf());
        assert(updated.base == old(state).untypeds@[index as int].base);
        assert(updated.size == old(state).untypeds@[index as int].size);
        assert(updated.child_view() =~= old(state).untypeds@[index as int].child_view());
        assert(updated.wf());
    }
    updated.revoke();
    proof {
        assert(updated.base == old(state).untypeds@[index as int].base);
        assert(updated.size == old(state).untypeds@[index as int].size);
        assert(updated.wf());
    }
    state.untypeds.set(index, updated);
    proof {
        assert(state.untypeds@[index as int] == updated);
        assert(state.untypeds@ =~= old(state).untypeds@.update(index as int, state.untypeds@[index as int]));
        step_untyped_revoke_preserves_kernel_invariant(*old(state), *state, index as int);
    }
}

// r[verify sel4-invariants.decomposition]
pub proof fn kernel_invariant_implies_object_refs_valid(state: KernelState)
    ensures kernel_invariant_spec(state) ==> object_refs_valid_spec(state)
{
}

// r[verify sel4-invariants.decomposition]
pub proof fn kernel_invariant_implies_cdt_wf(state: KernelState)
    ensures kernel_invariant_spec(state) ==> cdt_wf_spec(state)
{
}

// r[verify sel4-invariants.decomposition]
pub proof fn kernel_invariant_implies_cspace_valid(state: KernelState)
    ensures kernel_invariant_spec(state) ==> cspace_valid_spec(state)
{
}

// r[verify sel4-invariants.decomposition]
pub proof fn kernel_invariant_implies_tcb_legal(state: KernelState)
    ensures kernel_invariant_spec(state) ==> tcb_legal_spec(state)
{
}

// r[verify sel4-invariants.decomposition]
pub proof fn kernel_invariant_implies_scheduler_consistent(state: KernelState)
    ensures kernel_invariant_spec(state) ==> scheduler_consistent_spec(state)
{
}

// r[verify sel4-invariants.decomposition]
pub proof fn kernel_invariant_implies_untyped_ownership(state: KernelState)
    ensures kernel_invariant_spec(state) ==> untyped_ownership_spec(state)
{
}

// r[verify sel4-invariants.decomposition]
pub proof fn kernel_invariant_implies_asid_pool_wf(state: KernelState)
    ensures kernel_invariant_spec(state) ==> asid_pool_wf_spec(state)
{
}

// r[verify sel4-invariants.decomposition]
pub proof fn kernel_invariant_implies_vspace_consistent(state: KernelState)
    ensures kernel_invariant_spec(state) ==> vspace_consistent_spec(state)
{
}

// r[verify sel4-invariants.decomposition]
pub proof fn kernel_invariant_implies_irq_unique(state: KernelState)
    ensures kernel_invariant_spec(state) ==> irq_unique_spec(state)
{
}

// r[verify sel4-invariants.compose]
pub proof fn named_subinvariants_imply_kernel_invariant(state: KernelState)
    ensures
        object_refs_valid_spec(state)
            && cdt_wf_spec(state)
            && cspace_valid_spec(state)
            && tcb_legal_spec(state)
            && scheduler_consistent_spec(state)
            && untyped_ownership_spec(state)
            && asid_pool_wf_spec(state)
            && vspace_consistent_spec(state)
            && irq_unique_spec(state)
            ==> kernel_invariant_spec(state)
{
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::sel4_asid::AsidPool;
    use crate::sel4_cdt::Cdt;
    use crate::sel4_cspace::CNode;
    use crate::sel4_cspace::CSpace;
    use crate::sel4_ready_queue::new_ready_queues;
    use crate::sel4_tcb::new_tcb;
    use crate::sel4_untyped::SEL4_OBJ_ENDPOINT;
    use crate::sel4_untyped::SEL4_OBJ_FRAME;
    use crate::sel4_untyped::UntypedMemory;

    const NOOP_TEST_UNTYPED_BASE: u64 = 0x3000;
    const NOOP_TEST_UNTYPED_SIZE: u64 = 0x1000;

    #[derive(Clone, Copy)]
    struct NoopSnapshot {
        tcb_count: usize,
        ready_queue_count: usize,
        endpoint_count: usize,
        endpoint_sender_count: usize,
        endpoint_receiver_count: usize,
        untyped_count: usize,
        untyped_child_count: usize,
        asid_capacity: usize,
        vspace_count: usize,
        irq_count: usize,
    }

    fn noop_snapshot(state: &KernelState) -> NoopSnapshot {
        NoopSnapshot {
            tcb_count: state.tcbs.len(),
            ready_queue_count: state.ready_queues.queues.len(),
            endpoint_count: state.endpoints.len(),
            endpoint_sender_count: state.endpoints[0].senders.len(),
            endpoint_receiver_count: state.endpoints[0].receivers.len(),
            untyped_count: state.untypeds.len(),
            untyped_child_count: state.untypeds[0].children.len(),
            asid_capacity: state.asid_pool.capacity,
            vspace_count: state.vspaces.len(),
            irq_count: state.irqs.len(),
        }
    }

    fn assert_noop_snapshot(state: &KernelState, before: NoopSnapshot) {
        assert_eq!(state.tcbs.len(), before.tcb_count);
        assert_eq!(state.ready_queues.queues.len(), before.ready_queue_count);
        assert_eq!(state.endpoints.len(), before.endpoint_count);
        assert_eq!(state.endpoints[0].senders.len(), before.endpoint_sender_count);
        assert_eq!(state.endpoints[0].receivers.len(), before.endpoint_receiver_count);
        assert_eq!(state.untypeds.len(), before.untyped_count);
        assert_eq!(state.untypeds[0].children.len(), before.untyped_child_count);
        assert_eq!(state.asid_pool.capacity, before.asid_capacity);
        assert_eq!(state.vspaces.len(), before.vspace_count);
        assert_eq!(state.irqs.len(), before.irq_count);
    }

    fn noop_boundary_state() -> KernelState {
        let mut sender = new_tcb(0, 0);
        sender.state = TcbState::Running;
        base_kernel_state(
            vec![sender],
            new_ready_queues(1),
            vec![Endpoint {
                senders: vec![],
                receivers: vec![],
            }],
            vec![UntypedMemory::new(NOOP_TEST_UNTYPED_BASE, NOOP_TEST_UNTYPED_SIZE)],
        )
    }

    fn assert_rejected_noop(reason: u8) {
        let mut state = noop_boundary_state();
        let before = noop_snapshot(&state);
        let decision = reject_kernel_step_noop(&mut state, reason);
        assert!(!decision.applied);
        assert_eq!(decision.reason, reason);
        assert_noop_snapshot(&state, before);
        assert!(matches!(state.tcbs[0].state, TcbState::Running));
    }

    fn base_kernel_state(
        tcbs: Vec<Tcb>,
        ready_queues: ReadyQueues,
        endpoints: Vec<Endpoint>,
        untypeds: Vec<UntypedMemory>,
    ) -> KernelState {
        KernelState {
            tcbs,
            ready_queues,
            endpoints,
            notification_count: 0,
            reply_count: 0,
            cdt: Cdt {
                parents: vec![],
                slots: vec![],
                present: vec![],
            },
            cspace: CSpace {
                nodes: vec![CNode {
                    guard: vec![],
                    slots: vec![],
                }],
                root: 0,
            },
            untypeds,
            asid_pool: AsidPool::new(1),
            vspaces: vec![VSpace::new()],
            irqs: vec![],
        }
    }

    // r[verify sel4-invariants.step-preservation]
    #[test]
    fn test_step_endpoint_send_block_queues_sender() {
        let mut sender = new_tcb(0, 0);
        sender.state = TcbState::Running;

        let mut state = base_kernel_state(
            vec![sender],
            new_ready_queues(1),
            vec![Endpoint {
                senders: vec![],
                receivers: vec![],
            }],
            vec![UntypedMemory::new(0x1000, 0x1000)],
        );

        step_endpoint_send_block(&mut state, 0, 0, 99);

        assert!(matches!(state.tcbs[0].state, TcbState::BlockedOnSend { endpoint: 0 }));
        assert_eq!(state.endpoints[0].senders.len(), 1);
        assert_eq!(state.endpoints[0].senders[0].tcb, 0);
        assert_eq!(state.endpoints[0].senders[0].badge, 99);
        assert!(state.endpoints[0].receivers.is_empty());
        assert!(state.ready_queues.queues[0].is_empty());
    }

    // r[verify sel4-invariants.step-preservation]
    #[test]
    fn test_step_endpoint_send_rendezvous_wakes_head_receiver() {
        let mut sender = new_tcb(0, 0);
        sender.state = TcbState::Running;
        let mut receiver = new_tcb(1, 1);
        receiver.state = TcbState::BlockedOnReceive { endpoint: 0 };

        let mut state = base_kernel_state(
            vec![sender, receiver],
            new_ready_queues(2),
            vec![Endpoint {
                senders: vec![],
                receivers: vec![1],
            }],
            vec![UntypedMemory::new(0x1000, 0x1000)],
        );

        let matched = step_endpoint_send_rendezvous(&mut state, 0, 0, 77);

        assert_eq!(matched.sender, 0);
        assert_eq!(matched.receiver, 1);
        assert_eq!(matched.badge, 77);
        assert!(matches!(state.tcbs[1].state, TcbState::Runnable));
        assert!(state.endpoints[0].receivers.is_empty());
        assert_eq!(state.ready_queues.queues[1], vec![1]);
    }

    // r[verify sel4-invariants.step-preservation]
    #[test]
    fn test_step_untyped_revoke_clears_children() {
        let mut ut = UntypedMemory::new(0x1000, 0x4000);
        assert!(ut.retype(0x1000, 0x1000, 0x1000, SEL4_OBJ_ENDPOINT));
        assert!(ut.retype(0x2000, 0x1000, 0x1000, SEL4_OBJ_FRAME));

        let mut sender = new_tcb(0, 0);
        sender.state = TcbState::Running;
        let mut state = base_kernel_state(
            vec![sender],
            new_ready_queues(1),
            vec![Endpoint {
                senders: vec![],
                receivers: vec![],
            }],
            vec![ut],
        );

        step_untyped_revoke(&mut state, 0);

        assert!(state.untypeds[0].children.is_empty());
    }

    // r[verify sel4-invariants.step-preservation-expansion]
    #[test]
    fn rejected_invalid_reference_request_is_noop() {
        assert_rejected_noop(KERNEL_STEP_REJECT_INVALID_REFERENCE);
    }

    // r[verify sel4-invariants.step-preservation-expansion]
    #[test]
    fn rejected_mismatched_endpoint_tcb_request_is_noop() {
        assert_rejected_noop(KERNEL_STEP_REJECT_MISMATCHED_ENDPOINT_TCB);
    }

    // r[verify sel4-invariants.step-preservation-expansion]
    #[test]
    fn rejected_out_of_scope_request_is_noop() {
        assert_rejected_noop(KERNEL_STEP_REJECT_OUT_OF_SCOPE);
    }

    proptest! {
        // r[verify sel4-invariants.step-preservation]
        #[test]
        fn prop_rendezvous_enqueues_receiver_at_its_priority(priority in 0u8..4u8, badge: u64) {
            let mut sender = new_tcb(0, 0);
            sender.state = TcbState::Running;
            let mut receiver = new_tcb(1, priority);
            receiver.state = TcbState::BlockedOnReceive { endpoint: 0 };

            let mut state = base_kernel_state(
                vec![sender, receiver],
                new_ready_queues(priority as usize + 1),
                vec![Endpoint {
                    senders: vec![],
                    receivers: vec![1],
                }],
                vec![UntypedMemory::new(0x1000, 0x1000)],
            );

            let matched = step_endpoint_send_rendezvous(&mut state, 0, 0, badge);

            prop_assert_eq!(matched.receiver, 1);
            prop_assert_eq!(matched.badge, badge);
            prop_assert_eq!(&state.ready_queues.queues[priority as usize], &vec![1]);
        }
    }
}
