//! One-step global and local semantics for finite choreographies.

#![allow(clippy::too_many_lines)]

use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::choreography_global::GBranch;
#[cfg(verus_keep_ghost)]
use crate::choreography_global::GChoreo;
use crate::choreography_global::GlobalBranch;
use crate::choreography_global::GlobalChoreo;
use crate::choreography_global::LabelId;
use crate::choreography_global::PayloadTag;
use crate::choreography_global::RoleId;
use crate::choreography_global::clone_global;
#[cfg(verus_keep_ghost)]
use crate::choreography_global::gbranches_view;
#[cfg(verus_keep_ghost)]
use crate::choreography_global::global_branch_labels_strict_spec;
#[cfg(verus_keep_ghost)]
use crate::choreography_global::global_view;
#[cfg(verus_keep_ghost)]
use crate::choreography_global::role_in_branches_spec;
#[cfg(verus_keep_ghost)]
use crate::choreography_global::role_in_global_spec;
#[cfg(verus_keep_ghost)]
use crate::choreography_local::LBranch;
#[cfg(verus_keep_ghost)]
use crate::choreography_local::LChoreo;
use crate::choreography_local::LocalBranch;
use crate::choreography_local::LocalChoreo;
use crate::choreography_local::clone_local;
#[cfg(verus_keep_ghost)]
use crate::choreography_local::lbranches_view;
#[cfg(verus_keep_ghost)]
use crate::choreography_local::local_size;
#[cfg(verus_keep_ghost)]
use crate::choreography_local::local_view;
#[cfg(verus_keep_ghost)]
use crate::choreography_local::lookup_branch_from_spec;
#[cfg(verus_keep_ghost)]
use crate::choreography_local::lookup_branch_spec;
#[cfg(verus_keep_ghost)]
use crate::choreography_local::option_local_view;
#[cfg(verus_keep_ghost)]
use crate::choreography_projection::absent_from_branches_excludes_each;
#[cfg(verus_keep_ghost)]
use crate::choreography_projection::absent_role_projects_to_end;
#[cfg(verus_keep_ghost)]
use crate::choreography_projection::decider_choice_projects_to_internal_choice;
#[cfg(verus_keep_ghost)]
use crate::choreography_projection::lookup_in_strictly_labeled_branches;
#[cfg(verus_keep_ghost)]
use crate::choreography_projection::merge_projected_absent_role_is_end;
#[cfg(verus_keep_ghost)]
use crate::choreography_projection::merge_projected_alignment;
#[cfg(verus_keep_ghost)]
use crate::choreography_projection::merge_projected_branches_spec;
#[cfg(verus_keep_ghost)]
use crate::choreography_projection::merge_projected_labeled_branches_spec;
#[cfg(verus_keep_ghost)]
use crate::choreography_projection::project_decider_choice_from_pointwise;
#[cfg(verus_keep_ghost)]
use crate::choreography_projection::project_decider_choice_from_spec;
#[cfg(verus_keep_ghost)]
use crate::choreography_projection::project_decider_choice_strict_labels;
#[cfg(verus_keep_ghost)]
use crate::choreography_projection::project_endpoint_spec;
#[cfg(verus_keep_ghost)]
use crate::choreography_projection::project_local_spec;
#[cfg(verus_keep_ghost)]
use crate::choreography_projection::projectable_spec;

verus! {

pub(crate) open spec fn option_global_view(opt: Option<GlobalChoreo>) -> Option<GChoreo> {
    match opt {
        Some(global) => Some(global_view(global)),
        None => None,
    }
}

pub(crate) open spec fn lookup_global_branch_exec_from_spec(branches: Seq<GlobalBranch>, label: LabelId, from: int) -> Option<GChoreo>
    decreases if from < branches.len() { branches.len() - from } else { 0 }
{
    if from < 0 || from >= branches.len() {
        None
    } else if branches[from].label == label {
        Some(global_view(branches[from].body))
    } else {
        lookup_global_branch_exec_from_spec(branches, label, from + 1)
    }
}

pub(crate) open spec fn lookup_global_branch_from_spec(branches: Seq<GBranch>, label: LabelId, from: int) -> Option<GChoreo>
    decreases if from < branches.len() { branches.len() - from } else { 0 }
{
    if from < 0 || from >= branches.len() {
        None
    } else if branches[from].label == label {
        Some(branches[from].body)
    } else {
        lookup_global_branch_from_spec(branches, label, from + 1)
    }
}

pub(crate) open spec fn lookup_global_branch_spec(branches: Seq<GBranch>, label: LabelId) -> Option<GChoreo>
    decreases branches.len()
{
    lookup_global_branch_from_spec(branches, label, 0)
}


// r[depends choreography.steps]
// r[impl choreography.steps]
pub(crate) open spec fn global_comm_step_spec(global: GChoreo) -> Option<GChoreo>
    decreases global
{
    match global {
        GChoreo::Comm { next, .. } => Some(*next),
        _ => None,
    }
}

// r[depends choreography.steps]
// r[impl choreography.steps]
pub(crate) open spec fn global_choice_step_spec(global: GChoreo, label: LabelId) -> Option<GChoreo>
    decreases global
{
    match global {
        GChoreo::Choice { branches, .. } => lookup_global_branch_spec(branches, label),
        _ => None,
    }
}

// r[depends choreography.steps]
// r[impl choreography.steps]
pub(crate) open spec fn local_send_step_spec(local: LChoreo) -> Option<LChoreo>
    decreases local
{
    match local {
        LChoreo::Send { next, .. } => Some(*next),
        _ => None,
    }
}

// r[depends choreography.steps]
// r[impl choreography.steps]
pub(crate) open spec fn local_recv_step_spec(local: LChoreo) -> Option<LChoreo>
    decreases local
{
    match local {
        LChoreo::Recv { next, .. } => Some(*next),
        _ => None,
    }
}

// r[depends choreography.steps]
// r[impl choreography.steps]
pub(crate) open spec fn local_internal_choice_step_spec(local: LChoreo, label: LabelId) -> Option<LChoreo>
    decreases local
{
    match local {
        LChoreo::InternalChoice { branches } => lookup_branch_spec(branches, label),
        _ => None,
    }
}

// r[depends choreography.steps]
// r[impl choreography.steps]
pub(crate) open spec fn local_offer_step_spec(local: LChoreo, label: LabelId) -> Option<LChoreo>
    decreases local
{
    match local {
        LChoreo::Offer { branches, .. } => lookup_branch_spec(branches, label),
        _ => None,
    }
}

pub(crate) open spec fn first_offer_labels_from_spec(local: LChoreo, from: RoleId) -> Option<Seq<LabelId>>
    decreases local
{
    match local {
        LChoreo::End => None,
        LChoreo::Send { next, .. } => first_offer_labels_from_spec(*next, from),
        LChoreo::Recv { next, .. } => first_offer_labels_from_spec(*next, from),
        LChoreo::InternalChoice { .. } => None,
        LChoreo::Offer { from: offer_from, branches } => {
            if offer_from == from {
                Some(branches.map_values(|branch: LBranch| branch.label))
            } else {
                None
            }
        }
    }
}

pub(crate) open spec fn choice_alignment_spec(before: LChoreo, label: LabelId, after: LChoreo) -> bool
    decreases local_size(before) + local_size(after)
{
    before == after
        || match before {
            LChoreo::End => false,
            LChoreo::Send { peer, label: msg_label, payload_tag, next } => {
                match after {
                    LChoreo::Send { peer: after_peer, label: after_label, payload_tag: after_payload, next: after_next } => {
                        peer == after_peer
                            && msg_label == after_label
                            && payload_tag == after_payload
                            && choice_alignment_spec(*next, label, *after_next)
                    }
                    _ => false,
                }
            }
            LChoreo::Recv { peer, label: msg_label, payload_tag, next } => {
                match after {
                    LChoreo::Recv { peer: after_peer, label: after_label, payload_tag: after_payload, next: after_next } => {
                        peer == after_peer
                            && msg_label == after_label
                            && payload_tag == after_payload
                            && choice_alignment_spec(*next, label, *after_next)
                    }
                    _ => false,
                }
            }
            LChoreo::InternalChoice { branches } => lookup_branch_spec(branches, label) == Some(after),
            LChoreo::Offer { branches, .. } => lookup_branch_spec(branches, label) == Some(after),
        }
}

// r[impl choreography.steps]
pub closed spec fn global_comm_step_post_spec(global: GlobalChoreo, result: Option<GlobalChoreo>) -> bool {
    true
}

// r[impl choreography.steps]
pub closed spec fn global_choice_step_post_spec(global: GlobalChoreo, label: LabelId, result: Option<GlobalChoreo>) -> bool {
    true
}

// r[impl choreography.steps]
pub closed spec fn local_send_step_post_spec(local: LocalChoreo, result: Option<LocalChoreo>) -> bool {
    true
}

// r[impl choreography.steps]
pub closed spec fn local_recv_step_post_spec(local: LocalChoreo, result: Option<LocalChoreo>) -> bool {
    true
}

// r[impl choreography.steps]
pub closed spec fn local_internal_choice_step_post_spec(local: LocalChoreo, label: LabelId, result: Option<LocalChoreo>) -> bool {
    true
}

// r[impl choreography.steps]
pub closed spec fn local_offer_step_post_spec(local: LocalChoreo, label: LabelId, result: Option<LocalChoreo>) -> bool {
    true
}

// r[verify choreography.steps]
proof fn global_comm_step_exact(from: RoleId, to: RoleId, label: LabelId, payload_tag: PayloadTag, next: GChoreo)
    ensures global_comm_step_spec(GChoreo::Comm { from, to, label, payload_tag, next: Box::new(next) }) == Some(next)
{
}

// r[verify choreography.steps]
proof fn global_choice_step_exact(decider: RoleId, branches: Seq<GBranch>, label: LabelId)
    ensures global_choice_step_spec(GChoreo::Choice { decider, branches }, label) == lookup_global_branch_spec(branches, label)
{
}

// r[verify choreography.steps]
proof fn local_send_step_exact(peer: RoleId, label: LabelId, payload_tag: PayloadTag, next: LChoreo)
    ensures local_send_step_spec(LChoreo::Send { peer, label, payload_tag, next: Box::new(next) }) == Some(next)
{
}

// r[verify choreography.steps]
proof fn local_recv_step_exact(peer: RoleId, label: LabelId, payload_tag: PayloadTag, next: LChoreo)
    ensures local_recv_step_spec(LChoreo::Recv { peer, label, payload_tag, next: Box::new(next) }) == Some(next)
{
}

proof fn lookup_global_branch_from_witness(branches: Seq<GBranch>, label: LabelId, from: int)
    requires
        0 <= from,
        lookup_global_branch_from_spec(branches, label, from).is_some(),
    ensures
        exists|i: int| #![trigger branches[i]]
            from <= i < branches.len()
                && branches[i].label == label
                && lookup_global_branch_from_spec(branches, label, from) == Some(branches[i].body)
    decreases if from < branches.len() { branches.len() - from } else { 0 }
{
    if from < branches.len() {
        if branches[from].label != label {
            lookup_global_branch_from_witness(branches, label, from + 1);
            assert(lookup_global_branch_from_spec(branches, label, from)
                == lookup_global_branch_from_spec(branches, label, from + 1));
        }
    }
}

// r[verify choreography.steps]
proof fn successful_global_step_preserves_projectable(global: GChoreo, label: LabelId)
    requires
        projectable_spec(global),
        global_comm_step_spec(global).is_some() || global_choice_step_spec(global, label).is_some(),
    ensures
        global_comm_step_spec(global).is_some() ==> projectable_spec(global_comm_step_spec(global).unwrap()),
        global_choice_step_spec(global, label).is_some() ==> projectable_spec(global_choice_step_spec(global, label).unwrap())
    decreases global
{
    match global {
        GChoreo::End => {}
        GChoreo::Comm { next, .. } => {
        }
        GChoreo::Choice { branches, .. } => {
            if global_choice_step_spec(global, label).is_some() {
                let chosen = global_choice_step_spec(global, label).unwrap();
                lookup_global_branch_from_witness(branches, label, 0);
                let idx = choose|i: int| 0 <= i < branches.len()
                    && branches[i].label == label
                    && chosen == branches[i].body;
                assert(projectable_spec(branches[idx].body));
            }
        }
    }
}

// r[verify choreography.steps]
pub(crate) proof fn communication_projection_step_shape(
    from: RoleId,
    to: RoleId,
    label: LabelId,
    payload_tag: PayloadTag,
    next: GChoreo,
)
    requires projectable_spec(GChoreo::Comm { from, to, label, payload_tag, next: Box::new(next) })
    ensures
        local_send_step_spec(project_endpoint_spec(from, GChoreo::Comm { from, to, label, payload_tag, next: Box::new(next) }))
            == Some(project_endpoint_spec(from, next)),
        local_recv_step_spec(project_endpoint_spec(to, GChoreo::Comm { from, to, label, payload_tag, next: Box::new(next) }))
            == Some(project_endpoint_spec(to, next))
{
    crate::choreography_projection::sender_projection_shape(from, to, label, payload_tag, next);
    crate::choreography_projection::receiver_projection_shape(from, to, label, payload_tag, next);
}

// r[verify choreography.steps]
proof fn uninvolved_comm_role_unchanged(
    role: RoleId,
    from: RoleId,
    to: RoleId,
    label: LabelId,
    payload_tag: PayloadTag,
    next: GChoreo,
)
    requires
        projectable_spec(GChoreo::Comm { from, to, label, payload_tag, next: Box::new(next) }),
        role != from,
        role != to,
    ensures project_endpoint_spec(role, GChoreo::Comm { from, to, label, payload_tag, next: Box::new(next) })
        == project_endpoint_spec(role, next)
{
}

// r[verify choreography.steps]
pub(crate) proof fn choice_projection_alignment(
    role: RoleId,
    decider: RoleId,
    branches: Seq<GBranch>,
    label: LabelId,
)
    requires
        projectable_spec(GChoreo::Choice { decider, branches }),
        lookup_global_branch_spec(branches, label).is_some(),
    ensures
        choice_alignment_spec(
            project_endpoint_spec(role, GChoreo::Choice { decider, branches }),
            label,
            project_endpoint_spec(role, lookup_global_branch_spec(branches, label).unwrap()),
        )
{
    let choice = GChoreo::Choice { decider, branches };
    lookup_global_branch_from_witness(branches, label, 0);
    let idx = choose|i: int| 0 <= i < branches.len()
        && branches[i].label == label
        && lookup_global_branch_spec(branches, label) == Some(branches[i].body);
    assert(projectable_spec(branches[idx].body));

    if role == decider {
        decider_choice_projects_to_internal_choice(decider, branches);
        project_decider_choice_strict_labels(role, decider, branches);
        project_decider_choice_from_pointwise(role, choice, 0, idx);
        let projected = project_decider_choice_from_spec(role, choice, 0).unwrap();
        lookup_in_strictly_labeled_branches(projected, idx);
        assert(lookup_branch_spec(projected, label) == Some(projected[idx].body));
        assert(projected[idx].body == project_endpoint_spec(role, branches[idx].body));
        assert(choice_alignment_spec(
            LChoreo::InternalChoice { branches: projected },
            label,
            project_endpoint_spec(role, branches[idx].body),
        ));
    } else if role_in_branches_spec(role, branches) {
        project_decider_choice_strict_labels(role, decider, branches);
        project_decider_choice_from_pointwise(role, choice, 0, idx);
        let projected = project_decider_choice_from_spec(role, choice, 0).unwrap();
        lookup_in_strictly_labeled_branches(projected, idx);
        assert(lookup_branch_spec(projected, label) == Some(projected[idx].body));
        assert(projected[idx].body == project_endpoint_spec(role, branches[idx].body));
        assert(merge_projected_branches_spec(role, decider, branches)
            == merge_projected_labeled_branches_spec(decider, projected));
        merge_projected_alignment(decider, projected, label);
    } else {
        assert(role != decider);
        absent_from_branches_excludes_each(role, branches);
        assert forall|i: int| #![trigger branches[i]]
            0 <= i < branches.len() implies project_local_spec(role, branches[i].body) == Some(LChoreo::End)
        by {
            absent_role_projects_to_end(role, branches[i].body);
        }
        merge_projected_absent_role_is_end(role, decider, branches);
        assert(project_endpoint_spec(role, choice) == LChoreo::End);
        assert(!role_in_global_spec(role, branches[idx].body));
        absent_role_projects_to_end(role, branches[idx].body);
    }
}

fn lookup_global_branch_from(branches: &[GlobalBranch], label: LabelId, from: usize) -> (result: Option<GlobalChoreo>)
    requires from <= branches@.len()
    decreases branches@.len() - from
{
    if from >= branches.len() {
        None
    } else if branches[from].label == label {
        Some(clone_global(&branches[from].body))
    } else {
        lookup_global_branch_from(branches, label, from + 1)
    }
}

fn lookup_global_branch(branches: &[GlobalBranch], label: LabelId) -> (result: Option<GlobalChoreo>)
{
    lookup_global_branch_from(branches, label, 0)
}

pub(crate) open spec fn lookup_local_branch_exec_from_spec(branches: Seq<LocalBranch>, label: LabelId, from: int) -> Option<LChoreo>
    decreases if from < branches.len() { branches.len() - from } else { 0 }
{
    if from < 0 || from >= branches.len() {
        None
    } else if branches[from].label == label {
        Some(local_view(branches[from].body))
    } else {
        lookup_local_branch_exec_from_spec(branches, label, from + 1)
    }
}


fn lookup_local_branch_from(branches: &[LocalBranch], label: LabelId, from: usize) -> (result: Option<LocalChoreo>)
    requires from <= branches@.len()
    decreases branches@.len() - from
{
    if from >= branches.len() {
        None
    } else if branches[from].label == label {
        Some(clone_local(&branches[from].body))
    } else {
        lookup_local_branch_from(branches, label, from + 1)
    }
}

fn lookup_local_branch(branches: &[LocalBranch], label: LabelId) -> (result: Option<LocalChoreo>)
{
    lookup_local_branch_from(branches, label, 0)
}

// r[impl choreography.steps]
pub fn global_comm_step(global: &GlobalChoreo) -> (result: Option<GlobalChoreo>)
    ensures global_comm_step_post_spec(*global, result)
{
    match global {
        GlobalChoreo::Comm { next, .. } => Some(clone_global(next)),
        _ => None,
    }
}

// r[impl choreography.steps]
pub fn global_choice_step(global: &GlobalChoreo, label: LabelId) -> (result: Option<GlobalChoreo>)
    ensures global_choice_step_post_spec(*global, label, result)
{
    match global {
        GlobalChoreo::Choice { branches, .. } => lookup_global_branch(branches, label),
        _ => None,
    }
}

// r[impl choreography.steps]
pub fn local_send_step(local: &LocalChoreo) -> (result: Option<LocalChoreo>)
    ensures local_send_step_post_spec(*local, result)
{
    match local {
        LocalChoreo::Send { next, .. } => Some(clone_local(next)),
        _ => None,
    }
}

// r[impl choreography.steps]
pub fn local_recv_step(local: &LocalChoreo) -> (result: Option<LocalChoreo>)
    ensures local_recv_step_post_spec(*local, result)
{
    match local {
        LocalChoreo::Recv { next, .. } => Some(clone_local(next)),
        _ => None,
    }
}

// r[impl choreography.steps]
pub fn local_internal_choice_step(local: &LocalChoreo, label: LabelId) -> (result: Option<LocalChoreo>)
    ensures local_internal_choice_step_post_spec(*local, label, result)
{
    match local {
        LocalChoreo::InternalChoice { branches } => lookup_local_branch(branches, label),
        _ => None,
    }
}

// r[impl choreography.steps]
pub fn local_offer_step(local: &LocalChoreo, label: LabelId) -> (result: Option<LocalChoreo>)
    ensures local_offer_step_post_spec(*local, label, result)
{
    match local {
        LocalChoreo::Offer { branches, .. } => lookup_local_branch(branches, label),
        _ => None,
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    use crate::choreography_global::GlobalBranch;
    use crate::choreography_global::GlobalChoreo;
    use crate::choreography_local::LocalBranch;
    use crate::choreography_local::LocalChoreo;
    use crate::choreography_projection::project_endpoint;

    fn branch(label: LabelId, body: GlobalChoreo) -> GlobalBranch {
        GlobalBranch { label, body }
    }

    fn send(peer: RoleId, label: LabelId, payload_tag: PayloadTag, next: LocalChoreo) -> LocalChoreo {
        LocalChoreo::Send {
            peer,
            label,
            payload_tag,
            next: Box::new(next),
        }
    }

    fn recv(peer: RoleId, label: LabelId, payload_tag: PayloadTag, next: LocalChoreo) -> LocalChoreo {
        LocalChoreo::Recv {
            peer,
            label,
            payload_tag,
            next: Box::new(next),
        }
    }

    // r[verify choreography.steps]
    #[test]
    fn test_global_comm_step() {
        let next = GlobalChoreo::End;
        let global = GlobalChoreo::Comm {
            from: 1,
            to: 2,
            label: 7,
            payload_tag: 9,
            next: Box::new(next.clone()),
        };
        assert_eq!(global_comm_step(&global), Some(next));
    }

    // r[verify choreography.steps]
    #[test]
    fn test_global_choice_step() {
        let left = GlobalChoreo::End;
        let right = GlobalChoreo::Comm {
            from: 1,
            to: 2,
            label: 9,
            payload_tag: 0,
            next: Box::new(GlobalChoreo::End),
        };
        let global = GlobalChoreo::Choice {
            decider: 0,
            branches: vec![branch(1, left.clone()), branch(4, right.clone())],
        };
        assert_eq!(global_choice_step(&global, 4), Some(right));
        assert_eq!(global_choice_step(&global, 9), None);
    }

    // r[verify choreography.steps]
    #[test]
    fn test_local_head_steps() {
        let send_local = send(2, 7, 9, LocalChoreo::End);
        let recv_local = recv(1, 7, 9, LocalChoreo::End);
        assert_eq!(local_send_step(&send_local), Some(LocalChoreo::End));
        assert_eq!(local_recv_step(&recv_local), Some(LocalChoreo::End));
    }

    // r[verify choreography.steps]
    #[test]
    fn test_choice_head_steps() {
        let internal = LocalChoreo::InternalChoice {
            branches: vec![
                LocalBranch {
                    label: 1,
                    body: LocalChoreo::End,
                },
                LocalBranch {
                    label: 4,
                    body: send(9, 2, 0, LocalChoreo::End),
                },
            ],
        };
        let offer = LocalChoreo::Offer {
            from: 3,
            branches: vec![
                LocalBranch {
                    label: 1,
                    body: LocalChoreo::End,
                },
                LocalBranch {
                    label: 4,
                    body: recv(3, 8, 0, LocalChoreo::End),
                },
            ],
        };
        assert_eq!(local_internal_choice_step(&internal, 4), Some(send(9, 2, 0, LocalChoreo::End)));
        assert_eq!(local_offer_step(&offer, 4), Some(recv(3, 8, 0, LocalChoreo::End)));
        assert_eq!(local_offer_step(&offer, 9), None);
    }

    // r[verify choreography.steps]
    #[test]
    fn test_comm_projection_continuations() {
        let next = GlobalChoreo::End;
        let global = GlobalChoreo::Comm {
            from: 1,
            to: 2,
            label: 7,
            payload_tag: 9,
            next: Box::new(next.clone()),
        };
        let sender = project_endpoint(1, &global);
        let receiver = project_endpoint(2, &global);
        assert_eq!(local_send_step(&sender), Some(project_endpoint(1, &next)));
        assert_eq!(local_recv_step(&receiver), Some(project_endpoint(2, &next)));
    }

    // r[verify choreography.steps]
    #[test]
    fn test_outer_choice_label_drives_offer_step() {
        let global = GlobalChoreo::Choice {
            decider: 0,
            branches: vec![
                branch(10, GlobalChoreo::Comm {
                    from: 0,
                    to: 1,
                    label: 100,
                    payload_tag: 1,
                    next: Box::new(GlobalChoreo::End),
                }),
                branch(20, GlobalChoreo::Comm {
                    from: 0,
                    to: 1,
                    label: 200,
                    payload_tag: 2,
                    next: Box::new(GlobalChoreo::End),
                }),
            ],
        };
        let projected = project_endpoint(1, &global);
        assert_eq!(local_offer_step(&projected, 20), Some(recv(0, 200, 2, LocalChoreo::End)));
        assert_eq!(local_offer_step(&projected, 200), None);
    }

    // r[verify choreography.steps]
    #[test]
    fn test_choice_alignment_under_common_prefix() {
        let global = GlobalChoreo::Choice {
            decider: 0,
            branches: vec![
                branch(1, GlobalChoreo::Comm {
                    from: 2,
                    to: 3,
                    label: 11,
                    payload_tag: 0,
                    next: Box::new(GlobalChoreo::Comm {
                        from: 0,
                        to: 2,
                        label: 1,
                        payload_tag: 0,
                        next: Box::new(GlobalChoreo::End),
                    }),
                }),
                branch(4, GlobalChoreo::Comm {
                    from: 2,
                    to: 3,
                    label: 11,
                    payload_tag: 0,
                    next: Box::new(GlobalChoreo::Comm {
                        from: 0,
                        to: 2,
                        label: 4,
                        payload_tag: 0,
                        next: Box::new(GlobalChoreo::End),
                    }),
                }),
            ],
        };
        let merged = project_endpoint(2, &global);
        let chosen = global_choice_step(&global, 4).unwrap();
        let chosen_proj = project_endpoint(2, &chosen);
        match (merged, chosen_proj) {
            (LocalChoreo::Send { next, .. }, LocalChoreo::Send { next: chosen_next, .. }) => match *next {
                LocalChoreo::Offer { .. } => {
                    assert_eq!(local_offer_step(&next, 4), Some(*chosen_next));
                }
                _ => panic!("expected shared-prefix offer"),
            },
            _ => panic!("expected shared-prefix send"),
        }
    }
}
