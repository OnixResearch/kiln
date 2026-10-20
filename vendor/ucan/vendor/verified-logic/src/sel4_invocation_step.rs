//! Composite seL4-style invocation-step facade.
//!
//! This is intentionally a thin facade over the seL4 starter modules that exist
//! in this repository.  It covers exactly four operations: capability copy,
//! page map, endpoint send rendezvous, and explicit sender blocking on an
//! endpoint.  Requests outside that set are rejected without mutating the kernel
//! state.

use vstd::prelude::*;

use crate::sel4_cdt::copy_cap;
use crate::sel4_invariants::KernelState;
#[cfg(verus_keep_ghost)]
use crate::sel4_invariants::kernel_invariant_spec;
use crate::sel4_invariants::step_endpoint_send_block;
use crate::sel4_invariants::step_endpoint_send_rendezvous;
use crate::sel4_tcb::TcbState;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Sel4InvocationStep {
    CopyCap { source: usize, new_slot: usize },
    MapPage {
        vspace: usize,
        frame: usize,
        vaddr: u64,
        size: u64,
        cap_rights: u64,
        requested_rights: u64,
    },
    EndpointSend { endpoint: usize, sender: usize, badge: u64 },
    BlockSenderOnEndpoint { endpoint: usize, sender: usize, badge: u64 },
    OutOfScope,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Sel4InvocationAccepted {
    CopyCap { new_cap: usize },
    MapPage,
    EndpointSend { receiver: usize },
    BlockSenderOnEndpoint,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Sel4InvocationReject {
    Invalid,
    OutOfScope,
}

pub type Sel4InvocationStepOutcome = Result<Sel4InvocationAccepted, Sel4InvocationReject>;

// r[impl sel4-invocation-step.accepted]
// r[depends sel4-invocation-step.accepted]
pub open spec fn sel4_invocation_in_scope_spec(inv: Sel4InvocationStep) -> bool {
    match inv {
        Sel4InvocationStep::CopyCap { .. }
        | Sel4InvocationStep::MapPage { .. }
        | Sel4InvocationStep::EndpointSend { .. }
        | Sel4InvocationStep::BlockSenderOnEndpoint { .. } => true,
        Sel4InvocationStep::OutOfScope => false,
    }
}

// r[impl sel4-invocation-step.accepted]
// r[depends sel4-invocation-step.accepted]
pub open spec fn sel4_invocation_step_accepted_spec(
    before: KernelState,
    after: KernelState,
    inv: Sel4InvocationStep,
    outcome: Sel4InvocationStepOutcome,
) -> bool {
    match outcome {
        Ok(Sel4InvocationAccepted::CopyCap { new_cap }) => match inv {
            Sel4InvocationStep::CopyCap { source, new_slot } => {
                &&& source < before.cdt.parents@.len()
                &&& before.cdt.present@[source as int]
                &&& new_cap == before.cdt.parents@.len()
                &&& after.cdt.parents@ == before.cdt.parents@.push(Some(source))
                &&& after.cdt.slots@ == before.cdt.slots@.push(new_slot)
                &&& after.cdt.present@ == before.cdt.present@.push(true)
            }
            _ => false,
        },
        Ok(Sel4InvocationAccepted::MapPage) => match inv {
            Sel4InvocationStep::MapPage { vspace, frame, vaddr, size, cap_rights, requested_rights } => {
                &&& vspace < before.vspaces@.len()
                &&& after.vspaces@[vspace as int].mapping_view()
                    == crate::sel4_vspace::map_transition_spec(
                        before.vspaces@[vspace as int].mapping_view(),
                        vaddr,
                        size,
                        frame,
                        cap_rights,
                        requested_rights,
                    )
            }
            _ => false,
        },
        Ok(Sel4InvocationAccepted::EndpointSend { receiver }) => match inv {
            Sel4InvocationStep::EndpointSend { endpoint, sender, badge } => {
                &&& endpoint < before.endpoints@.len()
                &&& sender < before.tcbs@.len()
                &&& before.endpoints@[endpoint as int].receivers@.len() > 0
                &&& receiver == before.endpoints@[endpoint as int].receivers@[0]
                &&& after.endpoints@[endpoint as int].senders@ =~= before.endpoints@[endpoint as int].senders@
                &&& after.endpoints@[endpoint as int].receivers@
                    =~= before.endpoints@[endpoint as int].receivers@.subrange(
                        1,
                        before.endpoints@[endpoint as int].receivers@.len() as int,
                    )
            }
            _ => false,
        },
        Ok(Sel4InvocationAccepted::BlockSenderOnEndpoint) => match inv {
            Sel4InvocationStep::BlockSenderOnEndpoint { endpoint, sender, badge } => {
                &&& endpoint < before.endpoints@.len()
                &&& sender < before.tcbs@.len()
                &&& before.endpoints@[endpoint as int].receivers@.len() == 0
                &&& after.endpoints@[endpoint as int].senders@
                    == before.endpoints@[endpoint as int].senders@.push(
                        crate::sel4_endpoint::SenderWaiter { tcb: sender, badge },
                    )
                &&& after.endpoints@[endpoint as int].receivers@ =~= before.endpoints@[endpoint as int].receivers@
            }
            _ => false,
        },
        Err(_) => false,
    }
}

// r[impl sel4-invocation-step.reject]
// r[depends sel4-invocation-step.reject]
pub open spec fn sel4_invocation_step_reject_preserves_spec(
    before: KernelState,
    after: KernelState,
    outcome: Sel4InvocationStepOutcome,
) -> bool {
    match outcome {
        Err(_) => after == before,
        Ok(_) => true,
    }
}

// r[impl sel4-invocation-step.refines-sources]
// r[depends sel4-invocation-step.refines-sources]
pub open spec fn sel4_invocation_step_refines_sources_spec(
    before: KernelState,
    after: KernelState,
    inv: Sel4InvocationStep,
    outcome: Sel4InvocationStepOutcome,
) -> bool {
    &&& match outcome {
        Err(Sel4InvocationReject::OutOfScope) => !sel4_invocation_in_scope_spec(inv) && after == before,
        Err(Sel4InvocationReject::Invalid) => sel4_invocation_in_scope_spec(inv) && after == before,
        Ok(_) => sel4_invocation_step_accepted_spec(before, after, inv, outcome),
    }
    &&& (kernel_invariant_spec(before) && outcome is Ok ==> kernel_invariant_spec(after))
}

// r[verify sel4-invocation-step.accepted]
pub proof fn sel4_invocation_step_accepted_is_in_scope(
    before: KernelState,
    after: KernelState,
    inv: Sel4InvocationStep,
    outcome: Sel4InvocationStepOutcome,
)
    requires
        sel4_invocation_step_accepted_spec(before, after, inv, outcome),
    ensures
        sel4_invocation_in_scope_spec(inv),
{
}

// r[verify sel4-invocation-step.reject]
pub proof fn sel4_invocation_step_reject_preserves_state(
    before: KernelState,
    after: KernelState,
    outcome: Sel4InvocationStepOutcome,
)
    requires
        sel4_invocation_step_reject_preserves_spec(before, after, outcome),
        outcome is Err,
    ensures
        after == before,
{
}

// r[verify sel4-invocation-step.refines-sources]
pub proof fn sel4_invocation_step_refinement_exposes_reject_or_accept(
    before: KernelState,
    after: KernelState,
    inv: Sel4InvocationStep,
    outcome: Sel4InvocationStepOutcome,
)
    requires
        sel4_invocation_step_refines_sources_spec(before, after, inv, outcome),
    ensures
        match outcome {
            Ok(_) => sel4_invocation_step_accepted_spec(before, after, inv, outcome),
            Err(_) => after == before,
        },
{
}

} // verus!

fn endpoint_has_sender(endpoint: &crate::sel4_endpoint::Endpoint, sender: usize) -> bool {
    endpoint.senders.iter().any(|waiter| waiter.tcb == sender)
}

fn endpoint_has_receiver(endpoint: &crate::sel4_endpoint::Endpoint, receiver: usize) -> bool {
    endpoint.receivers.contains(&receiver)
}

fn endpoint_can_enqueue_sender_runtime(endpoint: &crate::sel4_endpoint::Endpoint, sender: usize) -> bool {
    !endpoint_has_sender(endpoint, sender) && !endpoint_has_receiver(endpoint, sender)
}

fn sender_is_running(state: &KernelState, sender: usize) -> bool {
    sender < state.tcbs.len() && matches!(state.tcbs[sender].state, TcbState::Running)
}

/// r[impl sel4-invocation-step.accepted]
/// r[impl sel4-invocation-step.reject]
/// r[impl sel4-invocation-step.refines-sources]
/// r[impl composite-primitives.sel4-invocation-step]
/// Composite facade over the currently available source APIs:
/// `sel4_cdt::copy_cap`, `sel4_vspace::VSpace::map`,
/// `sel4_invariants::step_endpoint_send_rendezvous`, and
/// `sel4_invariants::step_endpoint_send_block`.
pub fn sel4_invocation_step(state: &mut KernelState, inv: Sel4InvocationStep) -> Sel4InvocationStepOutcome {
    match inv {
        Sel4InvocationStep::CopyCap { source, new_slot } => match copy_cap(&mut state.cdt, source, new_slot) {
            Some(new_cap) => Ok(Sel4InvocationAccepted::CopyCap { new_cap }),
            None => Err(Sel4InvocationReject::Invalid),
        },
        Sel4InvocationStep::MapPage {
            vspace,
            frame,
            vaddr,
            size,
            cap_rights,
            requested_rights,
        } => {
            if vspace >= state.vspaces.len() {
                return Err(Sel4InvocationReject::Invalid);
            }
            if state.vspaces[vspace].map(vaddr, size, frame, cap_rights, requested_rights) {
                Ok(Sel4InvocationAccepted::MapPage)
            } else {
                Err(Sel4InvocationReject::Invalid)
            }
        }
        Sel4InvocationStep::EndpointSend {
            endpoint,
            sender,
            badge,
        } => {
            if endpoint >= state.endpoints.len() || !sender_is_running(state, sender) {
                return Err(Sel4InvocationReject::Invalid);
            }
            if state.endpoints[endpoint].receivers.is_empty()
                || !endpoint_can_enqueue_sender_runtime(&state.endpoints[endpoint], sender)
            {
                return Err(Sel4InvocationReject::Invalid);
            }
            let receiver = state.endpoints[endpoint].receivers[0];
            if receiver >= state.tcbs.len()
                || !matches!(state.tcbs[receiver].state, TcbState::BlockedOnReceive { endpoint: ep } if ep == endpoint)
                || state.tcbs[receiver].priority as usize >= state.ready_queues.queues.len()
            {
                return Err(Sel4InvocationReject::Invalid);
            }
            let matched = step_endpoint_send_rendezvous(state, endpoint, sender, badge);
            Ok(Sel4InvocationAccepted::EndpointSend {
                receiver: matched.receiver,
            })
        }
        Sel4InvocationStep::BlockSenderOnEndpoint {
            endpoint,
            sender,
            badge,
        } => {
            if endpoint >= state.endpoints.len()
                || !sender_is_running(state, sender)
                || !state.endpoints[endpoint].receivers.is_empty()
                || !endpoint_can_enqueue_sender_runtime(&state.endpoints[endpoint], sender)
            {
                return Err(Sel4InvocationReject::Invalid);
            }
            step_endpoint_send_block(state, endpoint, sender, badge);
            Ok(Sel4InvocationAccepted::BlockSenderOnEndpoint)
        }
        Sel4InvocationStep::OutOfScope => Err(Sel4InvocationReject::OutOfScope),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sel4_asid::AsidPool;
    use crate::sel4_cdt::Cdt;
    use crate::sel4_cspace::CNode;
    use crate::sel4_cspace::CSpace;
    use crate::sel4_endpoint::Endpoint;
    use crate::sel4_ready_queue::new_ready_queues;
    use crate::sel4_tcb::Tcb;
    use crate::sel4_tcb::new_tcb;
    use crate::sel4_untyped::UntypedMemory;
    use crate::sel4_vspace::SEL4_VM_READ;
    use crate::sel4_vspace::SEL4_VM_WRITE;
    use crate::sel4_vspace::VSpace;

    fn state_with(tcbs: Vec<Tcb>, endpoints: Vec<Endpoint>) -> KernelState {
        KernelState {
            tcbs,
            ready_queues: new_ready_queues(4),
            endpoints,
            notification_count: 0,
            reply_count: 0,
            cdt: Cdt {
                parents: vec![None],
                slots: vec![10],
                present: vec![true],
            },
            cspace: CSpace {
                nodes: vec![CNode {
                    guard: vec![],
                    slots: vec![],
                }],
                root: 0,
            },
            untypeds: vec![UntypedMemory::new(0x1000, 0x1000)],
            asid_pool: AsidPool::new(1),
            vspaces: vec![VSpace::new()],
            irqs: vec![],
        }
    }

    // r[verify composite-primitives.sel4-invocation-step]
    #[test]
    fn test_copy_cap_accepted() {
        let mut state = state_with(vec![new_tcb(0, 0)], vec![Endpoint {
            senders: vec![],
            receivers: vec![],
        }]);
        let out = sel4_invocation_step(&mut state, Sel4InvocationStep::CopyCap {
            source: 0,
            new_slot: 44,
        });
        assert_eq!(out, Ok(Sel4InvocationAccepted::CopyCap { new_cap: 1 }));
        assert_eq!(state.cdt.parents, vec![None, Some(0)]);
        assert_eq!(state.cdt.slots, vec![10, 44]);
    }

    #[test]
    fn test_map_page_accepted() {
        let mut state = state_with(vec![new_tcb(0, 0)], vec![Endpoint {
            senders: vec![],
            receivers: vec![],
        }]);
        let out = sel4_invocation_step(&mut state, Sel4InvocationStep::MapPage {
            vspace: 0,
            frame: 7,
            vaddr: 0x4000,
            size: 0x1000,
            cap_rights: SEL4_VM_READ | SEL4_VM_WRITE,
            requested_rights: SEL4_VM_READ,
        });
        assert_eq!(out, Ok(Sel4InvocationAccepted::MapPage));
        assert_eq!(state.vspaces[0].mappings.len(), 1);
        assert_eq!(state.vspaces[0].mappings[0].frame, 7);
    }

    #[test]
    fn test_endpoint_send_accepted() {
        let mut sender = new_tcb(0, 0);
        sender.state = TcbState::Running;
        let mut receiver = new_tcb(1, 1);
        receiver.state = TcbState::BlockedOnReceive { endpoint: 0 };
        let mut state = state_with(vec![sender, receiver], vec![Endpoint {
            senders: vec![],
            receivers: vec![1],
        }]);
        let out = sel4_invocation_step(&mut state, Sel4InvocationStep::EndpointSend {
            endpoint: 0,
            sender: 0,
            badge: 99,
        });
        assert_eq!(out, Ok(Sel4InvocationAccepted::EndpointSend { receiver: 1 }));
        assert!(state.endpoints[0].receivers.is_empty());
        assert!(matches!(state.tcbs[1].state, TcbState::Runnable));
    }

    #[test]
    fn test_block_sender_on_endpoint_accepted() {
        let mut sender = new_tcb(0, 0);
        sender.state = TcbState::Running;
        let mut state = state_with(vec![sender], vec![Endpoint {
            senders: vec![],
            receivers: vec![],
        }]);
        let out = sel4_invocation_step(&mut state, Sel4InvocationStep::BlockSenderOnEndpoint {
            endpoint: 0,
            sender: 0,
            badge: 55,
        });
        assert_eq!(out, Ok(Sel4InvocationAccepted::BlockSenderOnEndpoint));
        assert_eq!(state.endpoints[0].senders.len(), 1);
        assert!(matches!(state.tcbs[0].state, TcbState::BlockedOnSend { endpoint: 0 }));
    }

    #[test]
    fn test_out_of_scope_rejected_without_mutation() {
        let mut state = state_with(vec![new_tcb(0, 0)], vec![Endpoint {
            senders: vec![],
            receivers: vec![],
        }]);
        let old_len = state.cdt.parents.len();
        let out = sel4_invocation_step(&mut state, Sel4InvocationStep::OutOfScope);
        assert_eq!(out, Err(Sel4InvocationReject::OutOfScope));
        assert_eq!(state.cdt.parents.len(), old_len);
    }

    #[test]
    fn test_invalid_map_rejected_without_vspace_mutation() {
        let mut state = state_with(vec![new_tcb(0, 0)], vec![Endpoint {
            senders: vec![],
            receivers: vec![],
        }]);
        let out = sel4_invocation_step(&mut state, Sel4InvocationStep::MapPage {
            vspace: 0,
            frame: 7,
            vaddr: 0x4100,
            size: 0x1000,
            cap_rights: SEL4_VM_READ,
            requested_rights: SEL4_VM_READ,
        });
        assert_eq!(out, Err(Sel4InvocationReject::Invalid));
        assert!(state.vspaces[0].mappings.is_empty());
    }
}
