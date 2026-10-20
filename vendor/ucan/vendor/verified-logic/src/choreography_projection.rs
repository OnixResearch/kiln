//! Projectability and endpoint projection for finite choreographies.

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
#[cfg(verus_keep_ghost)]
use crate::choreography_global::gbranches_view;
#[cfg(verus_keep_ghost)]
use crate::choreography_global::global_branch_labels_strict_spec;
#[cfg(verus_keep_ghost)]
use crate::choreography_global::global_view;
#[cfg(verus_keep_ghost)]
use crate::choreography_global::role_in_branches_spec;
#[cfg(verus_keep_ghost)]
use crate::choreography_global::role_in_choice_branches_prefix_spec;
#[cfg(verus_keep_ghost)]
use crate::choreography_global::role_in_global_spec;
#[cfg(verus_keep_ghost)]
use crate::choreography_global::well_formed_global_spec;
#[cfg(verus_keep_ghost)]
use crate::choreography_local::LBranch;
#[cfg(verus_keep_ghost)]
use crate::choreography_local::LChoreo;
use crate::choreography_local::LocalBranch;
use crate::choreography_local::LocalChoreo;
#[cfg(verus_keep_ghost)]
use crate::choreography_local::branch_labels_spec;
#[cfg(verus_keep_ghost)]
use crate::choreography_local::branch_labels_strict_spec;
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
use crate::choreography_local::option_branches_view;
#[cfg(verus_keep_ghost)]
use crate::choreography_local::option_local_view;

verus! {

pub(crate) open spec fn gchoice_branch_count(global: GChoreo) -> int {
    match global {
        GChoreo::Choice { branches, .. } => branches.len() as int,
        _ => 0,
    }
}

pub(crate) open spec fn project_decider_choice_from_spec(role: RoleId, global: GChoreo, from: int) -> Option<Seq<LBranch>>
    decreases global, 1int, if from < gchoice_branch_count(global) { gchoice_branch_count(global) - from } else { 0int }
{
    match global {
        GChoreo::Choice { branches, .. } => {
            if from < 0 {
                None
            } else if from >= branches.len() {
                Some(seq![])
            } else {
                match project_local_spec(role, branches[from].body) {
                    Some(body) => match project_decider_choice_from_spec(role, global, from + 1) {
                        Some(tail) => Some(seq![LBranch { label: branches[from].label, body }] + tail),
                        None => None,
                    },
                    None => None,
                }
            }
        }
        _ => None,
    }
}


pub(crate) open spec fn projected_branch_suffix_size(branches: Seq<LBranch>, from: int) -> nat
    decreases if from < branches.len() { branches.len() - from } else { 0 }
{
    if from < 0 || from >= branches.len() {
        0
    } else {
        1 + local_size(branches[from].body) + projected_branch_suffix_size(branches, from + 1)
    }
}

pub(crate) open spec fn same_projected_bodies_spec(branches: Seq<LBranch>) -> bool {
    branches.len() > 0
        && forall|i: int| #![trigger branches[i]]
            0 <= i < branches.len() ==> branches[i].body == branches[0].body
}

pub(crate) open spec fn decider_guarded_local_spec(decider: RoleId, local: LChoreo) -> bool {
    match local {
        LChoreo::Recv { peer, .. } => peer == decider,
        LChoreo::Offer { from, .. } => from == decider,
        _ => false,
    }
}

pub(crate) open spec fn all_decider_guarded_spec(decider: RoleId, branches: Seq<LBranch>) -> bool {
    forall|i: int| #![trigger branches[i]] 0 <= i < branches.len() ==> decider_guarded_local_spec(decider, branches[i].body)
}

pub(crate) open spec fn common_send_tails_from_spec(
    branches: Seq<LBranch>,
    peer: RoleId,
    label: LabelId,
    payload_tag: PayloadTag,
    from: int,
) -> Option<Seq<LBranch>>
    decreases if from < branches.len() { branches.len() - from } else { 0 }
{
    if from < 0 || from >= branches.len() {
        Some(seq![])
    } else {
        match branches[from].body {
            LChoreo::Send { peer: p, label: l, payload_tag: t, next } => {
                if p == peer && l == label && t == payload_tag {
                    match common_send_tails_from_spec(branches, peer, label, payload_tag, from + 1) {
                        Some(tails) => Some(seq![LBranch { label: branches[from].label, body: *next }] + tails),
                        None => None,
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

pub(crate) open spec fn common_send_tails_spec(branches: Seq<LBranch>) -> Option<(RoleId, LabelId, PayloadTag, Seq<LBranch>)>
    decreases branches.len()
{
    if branches.len() == 0 {
        None
    } else {
        match branches[0].body {
            LChoreo::Send { peer, label, payload_tag, .. } => {
                match common_send_tails_from_spec(branches, peer, label, payload_tag, 0) {
                    Some(tails) => Some((peer, label, payload_tag, tails)),
                    None => None,
                }
            }
            _ => None,
        }
    }
}

pub(crate) open spec fn common_recv_tails_from_spec(
    branches: Seq<LBranch>,
    peer: RoleId,
    label: LabelId,
    payload_tag: PayloadTag,
    from: int,
) -> Option<Seq<LBranch>>
    decreases if from < branches.len() { branches.len() - from } else { 0 }
{
    if from < 0 || from >= branches.len() {
        Some(seq![])
    } else {
        match branches[from].body {
            LChoreo::Recv { peer: p, label: l, payload_tag: t, next } => {
                if p == peer && l == label && t == payload_tag {
                    match common_recv_tails_from_spec(branches, peer, label, payload_tag, from + 1) {
                        Some(tails) => Some(seq![LBranch { label: branches[from].label, body: *next }] + tails),
                        None => None,
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

pub(crate) open spec fn common_recv_tails_spec(branches: Seq<LBranch>) -> Option<(RoleId, LabelId, PayloadTag, Seq<LBranch>)>
    decreases branches.len()
{
    if branches.len() == 0 {
        None
    } else {
        match branches[0].body {
            LChoreo::Recv { peer, label, payload_tag, .. } => {
                match common_recv_tails_from_spec(branches, peer, label, payload_tag, 0) {
                    Some(tails) => Some((peer, label, payload_tag, tails)),
                    None => None,
                }
            }
            _ => None,
        }
    }
}

pub(crate) open spec fn merge_projected_labeled_branches_fuel(decider: RoleId, branches: Seq<LBranch>, fuel: nat) -> Option<LChoreo>
    decreases fuel
{
    if fuel == 0 {
        None
    } else if branches.len() == 0 {
        None
    } else if same_projected_bodies_spec(branches) {
        Some(branches[0].body)
    } else {
        match common_send_tails_spec(branches) {
            Some((peer, label, payload_tag, tails)) => {
                match merge_projected_labeled_branches_fuel(decider, tails, (fuel - 1) as nat) {
                    Some(next) => Some(LChoreo::Send { peer, label, payload_tag, next: Box::new(next) }),
                    None => None,
                }
            }
            None => match common_recv_tails_spec(branches) {
                Some((peer, label, payload_tag, tails)) => {
                    match merge_projected_labeled_branches_fuel(decider, tails, (fuel - 1) as nat) {
                        Some(next) => Some(LChoreo::Recv { peer, label, payload_tag, next: Box::new(next) }),
                        None => None,
                    }
                }
                None => {
                    if branch_labels_strict_spec(branches) && all_decider_guarded_spec(decider, branches) {
                        Some(LChoreo::Offer { from: decider, branches })
                    } else {
                        None
                    }
                }
            }
        }
    }
}

pub(crate) open spec fn merge_projected_labeled_branches_spec(decider: RoleId, branches: Seq<LBranch>) -> Option<LChoreo> {
    merge_projected_labeled_branches_fuel(decider, branches, projected_branch_suffix_size(branches, 0))
}

pub(crate) open spec fn merge_projected_branches_spec(role: RoleId, decider: RoleId, branches: Seq<GBranch>) -> Option<LChoreo>
    decreases (GChoreo::Choice { decider, branches }), 1int, branches.len() as int + 1
{
    match project_decider_choice_from_spec(role, GChoreo::Choice { decider, branches }, 0) {
        Some(local_branches) => merge_projected_labeled_branches_spec(decider, local_branches),
        None => None,
    }
}

pub(crate) open spec fn project_local_spec(role: RoleId, global: GChoreo) -> Option<LChoreo>
    decreases global, 2int, 0int
{
    match global {
        GChoreo::End => Some(LChoreo::End),
        GChoreo::Comm { from, to, label, payload_tag, next } => {
            if role == from {
                match project_local_spec(role, *next) {
                    Some(next_local) => Some(LChoreo::Send {
                        peer: to,
                        label,
                        payload_tag,
                        next: Box::new(next_local),
                    }),
                    None => None,
                }
            } else if role == to {
                match project_local_spec(role, *next) {
                    Some(next_local) => Some(LChoreo::Recv {
                        peer: from,
                        label,
                        payload_tag,
                        next: Box::new(next_local),
                    }),
                    None => None,
                }
            } else {
                project_local_spec(role, *next)
            }
        }
        GChoreo::Choice { decider, branches } => {
            if branches.len() == 0 {
                None
            } else if role == decider {
                match project_decider_choice_from_spec(role, global, 0) {
                    Some(local_branches) => Some(LChoreo::InternalChoice { branches: local_branches }),
                    None => None,
                }
            } else {
                merge_projected_branches_spec(role, decider, branches)
            }
        }
    }
}

// r[depends choreography.projectability]
// r[impl choreography.projectability]
pub(crate) open spec fn projectable_spec(global: GChoreo) -> bool
    decreases global
{
    match global {
        GChoreo::End => true,
        GChoreo::Comm { from, to, next, .. } => from != to && projectable_spec(*next),
        GChoreo::Choice { decider, branches } => {
            branches.len() > 0
                && global_branch_labels_strict_spec(branches)
                && forall|i: int| #![trigger branches[i]] 0 <= i < branches.len() ==> projectable_spec(branches[i].body)
                && forall|role: RoleId| role != decider && role_in_branches_spec(role, branches)
                    ==> merge_projected_branches_spec(role, decider, branches).is_some()
        }
    }
}

// r[depends choreography.projection]
// r[impl choreography.projection]
pub(crate) open spec fn project_endpoint_spec(role: RoleId, global: GChoreo) -> LChoreo
    recommends projectable_spec(global)
{
    project_local_spec(role, global).unwrap()
}

proof fn lookup_branch_witness(branches: Seq<LBranch>, label: LabelId, from: int)
    requires
        0 <= from,
        lookup_branch_from_spec(branches, label, from).is_some(),
    ensures
        exists|i: int| #![trigger branches[i]]
            from <= i < branches.len()
                && branches[i].label == label
                && lookup_branch_from_spec(branches, label, from) == Some(branches[i].body)
    decreases if from < branches.len() { branches.len() - from } else { 0 }
{
    if from < branches.len() {
        if branches[from].label != label {
            lookup_branch_witness(branches, label, from + 1);
            assert(lookup_branch_from_spec(branches, label, from)
                == lookup_branch_from_spec(branches, label, from + 1));
        }
    }
}

proof fn lookup_from_strictly_labeled(branches: Seq<LBranch>, k: int, from: int)
    requires
        branch_labels_strict_spec(branches),
        0 <= from <= k < branches.len(),
    ensures
        lookup_branch_from_spec(branches, branches[k].label, from) == Some(branches[k].body)
    decreases k - from
{
    if from < k {
        assert(branches[from].label < branches[k].label);
        lookup_from_strictly_labeled(branches, k, from + 1);
        assert(lookup_branch_from_spec(branches, branches[k].label, from)
            == lookup_branch_from_spec(branches, branches[k].label, from + 1));
    } else {
        assert(from == k);
        assert(lookup_branch_from_spec(branches, branches[k].label, from) == Some(branches[k].body));
    }
}

pub(crate) proof fn lookup_in_strictly_labeled_branches(branches: Seq<LBranch>, k: int)
    requires
        branch_labels_strict_spec(branches),
        0 <= k < branches.len(),
    ensures
        lookup_branch_spec(branches, branches[k].label) == Some(branches[k].body)
    decreases branches.len()
{
    lookup_from_strictly_labeled(branches, k, 0);
}

proof fn projected_branch_suffix_size_cons_shift(branch: LBranch, tail: Seq<LBranch>, from: int)
    requires 0 <= from <= tail.len()
    ensures projected_branch_suffix_size(seq![branch] + tail, from + 1) == projected_branch_suffix_size(tail, from)
    decreases tail.len() - from
{
    if from < tail.len() {
        projected_branch_suffix_size_cons_shift(branch, tail, from + 1);
        assert((seq![branch] + tail)[from + 1] == tail[from]);
    }
}

proof fn projected_branch_suffix_size_cons(branch: LBranch, tail: Seq<LBranch>)
    ensures
        projected_branch_suffix_size(seq![branch] + tail, 0)
            == 1 + local_size(branch.body) + projected_branch_suffix_size(tail, 0)
{
    projected_branch_suffix_size_cons_shift(branch, tail, 0);
    assert((seq![branch] + tail)[0] == branch);
    assert(projected_branch_suffix_size(seq![branch] + tail, 0)
        == 1 + local_size((seq![branch] + tail)[0].body) + projected_branch_suffix_size(seq![branch] + tail, 1));
}

proof fn common_send_tails_size_exact(
    branches: Seq<LBranch>,
    peer: RoleId,
    label: LabelId,
    payload_tag: PayloadTag,
    from: int,
)
    requires
        0 <= from <= branches.len(),
        common_send_tails_from_spec(branches, peer, label, payload_tag, from).is_some(),
    ensures
        projected_branch_suffix_size(branches, from)
            == projected_branch_suffix_size(common_send_tails_from_spec(branches, peer, label, payload_tag, from).unwrap(), 0)
                + (branches.len() - from) as nat
    decreases branches.len() - from
{
    if from < branches.len() {
        match branches[from].body {
            LChoreo::Send { peer: p, label: l, payload_tag: t, next } => {
                assert(p == peer && l == label && t == payload_tag);
                common_send_tails_size_exact(branches, peer, label, payload_tag, from + 1);
                let rest = common_send_tails_from_spec(branches, peer, label, payload_tag, from + 1).unwrap();
                let head = LBranch { label: branches[from].label, body: *next };
                assert(common_send_tails_from_spec(branches, peer, label, payload_tag, from).unwrap()
                    == seq![head] + rest);
                projected_branch_suffix_size_cons(head, rest);
                assert(projected_branch_suffix_size(branches, from)
                    == 1 + local_size(branches[from].body) + projected_branch_suffix_size(branches, from + 1));
                assert(local_size(branches[from].body) == 1 + local_size(*next));
            }
            _ => {
                assert(false);
            }
        }
    } else {
        assert(common_send_tails_from_spec(branches, peer, label, payload_tag, from).unwrap().len() == 0);
    }
}

proof fn common_send_tails_size_decreases(
    branches: Seq<LBranch>,
    peer: RoleId,
    label: LabelId,
    payload_tag: PayloadTag,
    tails: Seq<LBranch>,
)
    requires common_send_tails_spec(branches) == Some((peer, label, payload_tag, tails))
    ensures projected_branch_suffix_size(tails, 0) < projected_branch_suffix_size(branches, 0)
{
    assert(branches.len() > 0);
    assert(common_send_tails_from_spec(branches, peer, label, payload_tag, 0) == Some(tails));
    common_send_tails_size_exact(branches, peer, label, payload_tag, 0);
}

proof fn common_recv_tails_size_exact(
    branches: Seq<LBranch>,
    peer: RoleId,
    label: LabelId,
    payload_tag: PayloadTag,
    from: int,
)
    requires
        0 <= from <= branches.len(),
        common_recv_tails_from_spec(branches, peer, label, payload_tag, from).is_some(),
    ensures
        projected_branch_suffix_size(branches, from)
            == projected_branch_suffix_size(common_recv_tails_from_spec(branches, peer, label, payload_tag, from).unwrap(), 0)
                + (branches.len() - from) as nat
    decreases branches.len() - from
{
    if from < branches.len() {
        match branches[from].body {
            LChoreo::Recv { peer: p, label: l, payload_tag: t, next } => {
                assert(p == peer && l == label && t == payload_tag);
                common_recv_tails_size_exact(branches, peer, label, payload_tag, from + 1);
                let rest = common_recv_tails_from_spec(branches, peer, label, payload_tag, from + 1).unwrap();
                let head = LBranch { label: branches[from].label, body: *next };
                assert(common_recv_tails_from_spec(branches, peer, label, payload_tag, from).unwrap()
                    == seq![head] + rest);
                projected_branch_suffix_size_cons(head, rest);
                assert(projected_branch_suffix_size(branches, from)
                    == 1 + local_size(branches[from].body) + projected_branch_suffix_size(branches, from + 1));
                assert(local_size(branches[from].body) == 1 + local_size(*next));
            }
            _ => {
                assert(false);
            }
        }
    } else {
        assert(common_recv_tails_from_spec(branches, peer, label, payload_tag, from).unwrap().len() == 0);
    }
}

proof fn common_recv_tails_size_decreases(
    branches: Seq<LBranch>,
    peer: RoleId,
    label: LabelId,
    payload_tag: PayloadTag,
    tails: Seq<LBranch>,
)
    requires common_recv_tails_spec(branches) == Some((peer, label, payload_tag, tails))
    ensures projected_branch_suffix_size(tails, 0) < projected_branch_suffix_size(branches, 0)
{
    assert(branches.len() > 0);
    assert(common_recv_tails_from_spec(branches, peer, label, payload_tag, 0) == Some(tails));
    common_recv_tails_size_exact(branches, peer, label, payload_tag, 0);
}

proof fn same_projected_bodies_chosen_eq_first(branches: Seq<LBranch>, label: LabelId)
    requires
        branches.len() > 0,
        same_projected_bodies_spec(branches),
        lookup_branch_spec(branches, label).is_some(),
    ensures lookup_branch_spec(branches, label).unwrap() == branches[0].body
{
    lookup_branch_witness(branches, label, 0);
    let idx = choose|i: int| 0 <= i < branches.len()
        && branches[i].label == label
        && lookup_branch_spec(branches, label) == Some(branches[i].body);
    assert(branches[idx].body == branches[0].body);
}

proof fn common_send_tails_from_len(
    branches: Seq<LBranch>,
    peer: RoleId,
    msg_label: LabelId,
    payload_tag: PayloadTag,
    from: int,
)
    requires
        0 <= from <= branches.len(),
        common_send_tails_from_spec(branches, peer, msg_label, payload_tag, from).is_some(),
    ensures
        common_send_tails_from_spec(branches, peer, msg_label, payload_tag, from).unwrap().len()
            == branches.len() - from
    decreases branches.len() - from
{
    if from < branches.len() {
        common_send_tails_from_len(branches, peer, msg_label, payload_tag, from + 1);
        match branches[from].body {
            LChoreo::Send { peer: p, label: l, payload_tag: t, next } => {
                assert(p == peer && l == msg_label && t == payload_tag);
                let rest = common_send_tails_from_spec(branches, peer, msg_label, payload_tag, from + 1).unwrap();
                let head = LBranch { label: branches[from].label, body: *next };
                assert(rest.len() == branches.len() - (from + 1));
                assert(common_send_tails_from_spec(branches, peer, msg_label, payload_tag, from).unwrap()
                    == seq![head] + rest);
                assert((seq![head] + rest).len() == 1 + rest.len());
                assert(1 + (branches.len() - (from + 1)) == branches.len() - from);
            }
            _ => { assert(false); }
        }
    } else {
        assert(from == branches.len());
        assert(common_send_tails_from_spec(branches, peer, msg_label, payload_tag, from).unwrap().len() == 0);
    }
}

proof fn common_send_tails_from_pointwise(
    branches: Seq<LBranch>,
    peer: RoleId,
    msg_label: LabelId,
    payload_tag: PayloadTag,
    from: int,
    k: int,
)
    requires
        0 <= from,
        0 <= k,
        from + k < branches.len(),
        common_send_tails_from_spec(branches, peer, msg_label, payload_tag, from).is_some(),
    ensures ({
        let tails = common_send_tails_from_spec(branches, peer, msg_label, payload_tag, from).unwrap();
        k < tails.len()
            && tails[k].label == branches[from + k].label
            && ({
                match branches[from + k].body {
                    LChoreo::Send { peer: p, label: l, payload_tag: t, next } =>
                        p == peer && l == msg_label && t == payload_tag && tails[k].body == *next,
                    _ => false,
                }
            })
    })
    decreases branches.len() - from
{
    common_send_tails_from_len(branches, peer, msg_label, payload_tag, from);
    if k == 0 {
        match branches[from].body {
            LChoreo::Send { peer: p, label: l, payload_tag: t, next } => {
                let rest = common_send_tails_from_spec(branches, peer, msg_label, payload_tag, from + 1).unwrap();
                let head = LBranch { label: branches[from].label, body: *next };
                let tails = common_send_tails_from_spec(branches, peer, msg_label, payload_tag, from).unwrap();
                assert(p == peer && l == msg_label && t == payload_tag);
                assert(tails == seq![head] + rest);
                assert(tails[0] == head);
            }
            _ => { assert(false); }
        }
    } else {
        assert(k > 0);
        common_send_tails_from_len(branches, peer, msg_label, payload_tag, from + 1);
        common_send_tails_from_pointwise(branches, peer, msg_label, payload_tag, from + 1, k - 1);
        let rest = common_send_tails_from_spec(branches, peer, msg_label, payload_tag, from + 1).unwrap();
        match branches[from].body {
            LChoreo::Send { next, .. } => {
                let head = LBranch { label: branches[from].label, body: *next };
                let tails = common_send_tails_from_spec(branches, peer, msg_label, payload_tag, from).unwrap();
                assert(tails == seq![head] + rest);
                assert(0 <= k - 1 < rest.len());
                assert((seq![head] + rest)[k] == rest[k - 1]);
                assert(tails[k] == rest[k - 1]);
                assert(branches[from + k] == branches[(from + 1) + (k - 1)]);
            }
            _ => { assert(false); }
        }
    }
}

proof fn common_send_tails_strict_labels(
    branches: Seq<LBranch>,
    peer: RoleId,
    msg_label: LabelId,
    payload_tag: PayloadTag,
    tails: Seq<LBranch>,
)
    requires
        branch_labels_strict_spec(branches),
        common_send_tails_spec(branches) == Some((peer, msg_label, payload_tag, tails)),
    ensures
        branch_labels_strict_spec(tails)
{
    common_send_tails_from_len(branches, peer, msg_label, payload_tag, 0);
    assert forall|i: int, j: int| #![trigger tails[i], tails[j]]
        0 <= i < j < tails.len() implies tails[i].label < tails[j].label
    by {
        common_send_tails_from_pointwise(branches, peer, msg_label, payload_tag, 0, i);
        common_send_tails_from_pointwise(branches, peer, msg_label, payload_tag, 0, j);
    }
}

proof fn common_send_tails_lookup_strip(
    branches: Seq<LBranch>,
    peer: RoleId,
    msg_label: LabelId,
    payload_tag: PayloadTag,
    tails: Seq<LBranch>,
    chosen_label: LabelId,
)
    requires
        branch_labels_strict_spec(branches),
        common_send_tails_spec(branches) == Some((peer, msg_label, payload_tag, tails)),
        lookup_branch_spec(branches, chosen_label).is_some(),
    ensures
        branch_labels_strict_spec(tails),
        lookup_branch_spec(tails, chosen_label).is_some(),
        lookup_branch_spec(branches, chosen_label).unwrap() == (LChoreo::Send {
            peer,
            label: msg_label,
            payload_tag,
            next: Box::new(lookup_branch_spec(tails, chosen_label).unwrap()),
        })
{
    common_send_tails_strict_labels(branches, peer, msg_label, payload_tag, tails);
    lookup_branch_witness(branches, chosen_label, 0);
    let k = choose|i: int| 0 <= i < branches.len()
        && branches[i].label == chosen_label
        && lookup_branch_spec(branches, chosen_label) == Some(branches[i].body);
    common_send_tails_from_len(branches, peer, msg_label, payload_tag, 0);
    common_send_tails_from_pointwise(branches, peer, msg_label, payload_tag, 0, k);
    assert(k < tails.len());
    assert(tails[k].label == chosen_label);
    lookup_in_strictly_labeled_branches(tails, k);
    assert(lookup_branch_spec(tails, chosen_label) == Some(tails[k].body));
    match branches[k].body {
        LChoreo::Send { peer: p, label: l, payload_tag: t, next } => {
            assert(p == peer && l == msg_label && t == payload_tag);
            assert(tails[k].body == *next);
        }
        _ => { assert(false); }
    }
}

proof fn common_recv_tails_from_len(
    branches: Seq<LBranch>,
    peer: RoleId,
    msg_label: LabelId,
    payload_tag: PayloadTag,
    from: int,
)
    requires
        0 <= from <= branches.len(),
        common_recv_tails_from_spec(branches, peer, msg_label, payload_tag, from).is_some(),
    ensures
        common_recv_tails_from_spec(branches, peer, msg_label, payload_tag, from).unwrap().len()
            == branches.len() - from
    decreases branches.len() - from
{
    if from < branches.len() {
        common_recv_tails_from_len(branches, peer, msg_label, payload_tag, from + 1);
        match branches[from].body {
            LChoreo::Recv { peer: p, label: l, payload_tag: t, next } => {
                assert(p == peer && l == msg_label && t == payload_tag);
                let rest = common_recv_tails_from_spec(branches, peer, msg_label, payload_tag, from + 1).unwrap();
                let head = LBranch { label: branches[from].label, body: *next };
                assert(rest.len() == branches.len() - (from + 1));
                assert(common_recv_tails_from_spec(branches, peer, msg_label, payload_tag, from).unwrap()
                    == seq![head] + rest);
                assert((seq![head] + rest).len() == 1 + rest.len());
                assert(1 + (branches.len() - (from + 1)) == branches.len() - from);
            }
            _ => { assert(false); }
        }
    } else {
        assert(from == branches.len());
        assert(common_recv_tails_from_spec(branches, peer, msg_label, payload_tag, from).unwrap().len() == 0);
    }
}

proof fn common_recv_tails_from_pointwise(
    branches: Seq<LBranch>,
    peer: RoleId,
    msg_label: LabelId,
    payload_tag: PayloadTag,
    from: int,
    k: int,
)
    requires
        0 <= from,
        0 <= k,
        from + k < branches.len(),
        common_recv_tails_from_spec(branches, peer, msg_label, payload_tag, from).is_some(),
    ensures ({
        let tails = common_recv_tails_from_spec(branches, peer, msg_label, payload_tag, from).unwrap();
        k < tails.len()
            && tails[k].label == branches[from + k].label
            && ({
                match branches[from + k].body {
                    LChoreo::Recv { peer: p, label: l, payload_tag: t, next } =>
                        p == peer && l == msg_label && t == payload_tag && tails[k].body == *next,
                    _ => false,
                }
            })
    })
    decreases branches.len() - from
{
    common_recv_tails_from_len(branches, peer, msg_label, payload_tag, from);
    if k == 0 {
        match branches[from].body {
            LChoreo::Recv { peer: p, label: l, payload_tag: t, next } => {
                let rest = common_recv_tails_from_spec(branches, peer, msg_label, payload_tag, from + 1).unwrap();
                let head = LBranch { label: branches[from].label, body: *next };
                let tails = common_recv_tails_from_spec(branches, peer, msg_label, payload_tag, from).unwrap();
                assert(p == peer && l == msg_label && t == payload_tag);
                assert(tails == seq![head] + rest);
                assert(tails[0] == head);
            }
            _ => { assert(false); }
        }
    } else {
        assert(k > 0);
        common_recv_tails_from_len(branches, peer, msg_label, payload_tag, from + 1);
        common_recv_tails_from_pointwise(branches, peer, msg_label, payload_tag, from + 1, k - 1);
        let rest = common_recv_tails_from_spec(branches, peer, msg_label, payload_tag, from + 1).unwrap();
        match branches[from].body {
            LChoreo::Recv { next, .. } => {
                let head = LBranch { label: branches[from].label, body: *next };
                let tails = common_recv_tails_from_spec(branches, peer, msg_label, payload_tag, from).unwrap();
                assert(tails == seq![head] + rest);
                assert(0 <= k - 1 < rest.len());
                assert((seq![head] + rest)[k] == rest[k - 1]);
                assert(tails[k] == rest[k - 1]);
                assert(branches[from + k] == branches[(from + 1) + (k - 1)]);
            }
            _ => { assert(false); }
        }
    }
}

proof fn common_recv_tails_strict_labels(
    branches: Seq<LBranch>,
    peer: RoleId,
    msg_label: LabelId,
    payload_tag: PayloadTag,
    tails: Seq<LBranch>,
)
    requires
        branch_labels_strict_spec(branches),
        common_recv_tails_spec(branches) == Some((peer, msg_label, payload_tag, tails)),
    ensures
        branch_labels_strict_spec(tails)
{
    common_recv_tails_from_len(branches, peer, msg_label, payload_tag, 0);
    assert forall|i: int, j: int| #![trigger tails[i], tails[j]]
        0 <= i < j < tails.len() implies tails[i].label < tails[j].label
    by {
        common_recv_tails_from_pointwise(branches, peer, msg_label, payload_tag, 0, i);
        common_recv_tails_from_pointwise(branches, peer, msg_label, payload_tag, 0, j);
    }
}

proof fn common_recv_tails_lookup_strip(
    branches: Seq<LBranch>,
    peer: RoleId,
    msg_label: LabelId,
    payload_tag: PayloadTag,
    tails: Seq<LBranch>,
    chosen_label: LabelId,
)
    requires
        branch_labels_strict_spec(branches),
        common_recv_tails_spec(branches) == Some((peer, msg_label, payload_tag, tails)),
        lookup_branch_spec(branches, chosen_label).is_some(),
    ensures
        branch_labels_strict_spec(tails),
        lookup_branch_spec(tails, chosen_label).is_some(),
        lookup_branch_spec(branches, chosen_label).unwrap() == (LChoreo::Recv {
            peer,
            label: msg_label,
            payload_tag,
            next: Box::new(lookup_branch_spec(tails, chosen_label).unwrap()),
        })
{
    common_recv_tails_strict_labels(branches, peer, msg_label, payload_tag, tails);
    lookup_branch_witness(branches, chosen_label, 0);
    let k = choose|i: int| 0 <= i < branches.len()
        && branches[i].label == chosen_label
        && lookup_branch_spec(branches, chosen_label) == Some(branches[i].body);
    common_recv_tails_from_len(branches, peer, msg_label, payload_tag, 0);
    common_recv_tails_from_pointwise(branches, peer, msg_label, payload_tag, 0, k);
    assert(k < tails.len());
    assert(tails[k].label == chosen_label);
    lookup_in_strictly_labeled_branches(tails, k);
    assert(lookup_branch_spec(tails, chosen_label) == Some(tails[k].body));
    match branches[k].body {
        LChoreo::Recv { peer: p, label: l, payload_tag: t, next } => {
            assert(p == peer && l == msg_label && t == payload_tag);
            assert(tails[k].body == *next);
        }
        _ => { assert(false); }
    }
}

proof fn merge_projected_alignment_fuel(decider: RoleId, branches: Seq<LBranch>, label: LabelId, fuel: nat)
    requires
        branches.len() > 0,
        branch_labels_strict_spec(branches),
        merge_projected_labeled_branches_fuel(decider, branches, fuel).is_some(),
        lookup_branch_spec(branches, label).is_some(),
    ensures crate::choreography_step::choice_alignment_spec(
        merge_projected_labeled_branches_fuel(decider, branches, fuel).unwrap(),
        label,
        lookup_branch_spec(branches, label).unwrap(),
    )
    decreases fuel
{
    if fuel == 0 {
        assert(false);
    } else if same_projected_bodies_spec(branches) {
        same_projected_bodies_chosen_eq_first(branches, label);
    } else {
        match common_send_tails_spec(branches) {
            Some((peer, msg_label, payload_tag, tails)) => {
                match merge_projected_labeled_branches_fuel(decider, tails, (fuel - 1) as nat) {
                    Some(next) => {
                        common_send_tails_lookup_strip(branches, peer, msg_label, payload_tag, tails, label);
                        merge_projected_alignment_fuel(decider, tails, label, (fuel - 1) as nat);
                        assert(crate::choreography_step::choice_alignment_spec(
                            LChoreo::Send { peer, label: msg_label, payload_tag, next: Box::new(next) },
                            label,
                            lookup_branch_spec(branches, label).unwrap(),
                        ));
                    }
                    None => { assert(false); }
                }
            }
            None => match common_recv_tails_spec(branches) {
                Some((peer, msg_label, payload_tag, tails)) => {
                    match merge_projected_labeled_branches_fuel(decider, tails, (fuel - 1) as nat) {
                        Some(next) => {
                            common_recv_tails_lookup_strip(branches, peer, msg_label, payload_tag, tails, label);
                            merge_projected_alignment_fuel(decider, tails, label, (fuel - 1) as nat);
                            assert(crate::choreography_step::choice_alignment_spec(
                                LChoreo::Recv { peer, label: msg_label, payload_tag, next: Box::new(next) },
                                label,
                                lookup_branch_spec(branches, label).unwrap(),
                            ));
                        }
                        None => { assert(false); }
                    }
                }
                None => {
                    assert(crate::choreography_step::choice_alignment_spec(
                        LChoreo::Offer { from: decider, branches },
                        label,
                        lookup_branch_spec(branches, label).unwrap(),
                    ));
                }
            }
        }
    }
}

pub(crate) proof fn merge_projected_alignment(decider: RoleId, branches: Seq<LBranch>, label: LabelId)
    requires
        branches.len() > 0,
        branch_labels_strict_spec(branches),
        merge_projected_labeled_branches_spec(decider, branches).is_some(),
        lookup_branch_spec(branches, label).is_some(),
    ensures crate::choreography_step::choice_alignment_spec(
        merge_projected_labeled_branches_spec(decider, branches).unwrap(),
        label,
        lookup_branch_spec(branches, label).unwrap(),
    )
    decreases projected_branch_suffix_size(branches, 0)
{
    let fuel = projected_branch_suffix_size(branches, 0);
    merge_projected_alignment_fuel(decider, branches, label, fuel);
}

proof fn absent_choice_prefix_excludes_role(role: RoleId, global: GChoreo, upto: int)
    requires
        match global {
            GChoreo::Choice { branches, .. } => 0 <= upto <= branches.len(),
            _ => false,
        },
        !role_in_choice_branches_prefix_spec(role, global, upto),
    ensures
        match global {
            GChoreo::Choice { branches, .. } =>
                forall|i: int| #![trigger branches[i]] 0 <= i < upto ==> !role_in_global_spec(role, branches[i].body),
            _ => false,
        }
    decreases global, 0int, if upto > 0 { upto } else { 0int }
{
    match global {
        GChoreo::Choice { branches, .. } => {
            if upto > 0 {
                assert(!role_in_choice_branches_prefix_spec(role, global, upto - 1));
                assert(!role_in_global_spec(role, branches[upto - 1].body));
                absent_choice_prefix_excludes_role(role, global, upto - 1);
            }
        }
        _ => {}
    }
}

pub(crate) proof fn absent_from_branches_excludes_each(role: RoleId, branches: Seq<GBranch>)
    requires !role_in_branches_spec(role, branches)
    ensures forall|i: int| #![trigger branches[i]] 0 <= i < branches.len() ==> !role_in_global_spec(role, branches[i].body)
    decreases branches.len()
{
    if branches.len() > 0 {
        assert(!role_in_global_spec(role, branches[0].body));
        assert(!role_in_branches_spec(role, branches.subrange(1, branches.len() as int)));
        absent_from_branches_excludes_each(role, branches.subrange(1, branches.len() as int));
        assert forall|i: int| #![trigger branches[i]] 0 <= i < branches.len() implies !role_in_global_spec(role, branches[i].body) by {
            if i == 0 {
                assert(!role_in_global_spec(role, branches[0].body));
            } else {
                assert(0 <= i - 1 < branches.subrange(1, branches.len() as int).len());
                assert(branches.subrange(1, branches.len() as int)[i - 1] == branches[i]);
            }
        }
    }
}

pub(crate) open spec fn labeled_end_branches_from_spec(branches: Seq<GBranch>, from: int) -> Seq<LBranch>
    decreases if from < branches.len() { branches.len() - from } else { 0 }
{
    if from < 0 || from >= branches.len() {
        seq![]
    } else {
        seq![LBranch { label: branches[from].label, body: LChoreo::End }]
            + labeled_end_branches_from_spec(branches, from + 1)
    }
}

proof fn labeled_end_branches_all_end(branches: Seq<GBranch>, from: int)
    requires 0 <= from <= branches.len()
    ensures forall|i: int| #![trigger labeled_end_branches_from_spec(branches, from)[i]]
        0 <= i < labeled_end_branches_from_spec(branches, from).len()
            ==> labeled_end_branches_from_spec(branches, from)[i].body == LChoreo::End
    decreases branches.len() - from
{
    if from < branches.len() {
        labeled_end_branches_all_end(branches, from + 1);
        assert forall|i: int| #![trigger labeled_end_branches_from_spec(branches, from)[i]]
            0 <= i < labeled_end_branches_from_spec(branches, from).len()
                implies labeled_end_branches_from_spec(branches, from)[i].body == LChoreo::End
        by {
            if i == 0 {
            } else {
                assert(0 <= i - 1 < labeled_end_branches_from_spec(branches, from + 1).len());
            }
        }
    }
}

proof fn project_decider_choice_all_end(role: RoleId, global: GChoreo, from: int)
    requires
        0 <= from,
        match global {
            GChoreo::Choice { branches, .. } =>
                forall|i: int| #![trigger branches[i]] from <= i < branches.len()
                    ==> project_local_spec(role, branches[i].body) == Some(LChoreo::End),
            _ => false,
        },
    ensures
        match global {
            GChoreo::Choice { branches, .. } =>
                project_decider_choice_from_spec(role, global, from) == Some(labeled_end_branches_from_spec(branches, from)),
            _ => false,
        }
    decreases global, 1int, if from < gchoice_branch_count(global) { gchoice_branch_count(global) - from } else { 0int }
{
    match global {
        GChoreo::Choice { branches, .. } => {
            if from < branches.len() {
                assert(project_local_spec(role, branches[from].body) == Some(LChoreo::End));
                project_decider_choice_all_end(role, global, from + 1);
            }
        }
        _ => {}
    }
}

// r[verify choreography.projectability]
proof fn projectable_implies_projection_some(role: RoleId, global: GChoreo)
    requires projectable_spec(global)
    ensures project_local_spec(role, global).is_some()
    decreases global
{
    match global {
        GChoreo::End => {}
        GChoreo::Comm { from, to, next, .. } => {
            projectable_implies_projection_some(role, *next);
        }
        GChoreo::Choice { decider, branches } => {
            if role == decider {
                project_decider_choice_from_some(role, global, 0);
            } else if role_in_branches_spec(role, branches) {
                assert(merge_projected_branches_spec(role, decider, branches).is_some());
            } else {
                absent_from_branches_excludes_each(role, branches);
                assert forall|i: int| #![trigger branches[i]]
                    0 <= i < branches.len() implies project_local_spec(role, branches[i].body) == Some(LChoreo::End)
                by {
                    assert(projectable_spec(branches[i].body));
                    absent_role_projects_to_end(role, branches[i].body);
                }
                merge_projected_absent_role_is_end(role, decider, branches);
            }
        }
    }
}

// r[impl choreography.projectability]
pub closed spec fn projectable_exec_spec(global: GlobalChoreo) -> bool {
    projectable_spec(global_view(global))
}

// r[impl choreography.projection]
pub closed spec fn project_endpoint_post_spec(role: RoleId, global: GlobalChoreo, result: LocalChoreo) -> bool {
    local_view(result) == project_endpoint_spec(role, global_view(global))
}

pub closed spec fn project_endpoint_shape_post_spec(role: RoleId, global: GlobalChoreo, result: LocalChoreo) -> bool {
    match global {
        GlobalChoreo::End => result == LocalChoreo::End,
        GlobalChoreo::Comm { from, to, label, payload_tag, .. } => {
            if role == from {
                match result {
                    LocalChoreo::Send { peer, label: out_label, payload_tag: out_tag, .. } => {
                        peer == to && out_label == label && out_tag == payload_tag
                    }
                    _ => false,
                }
            } else if role == to {
                match result {
                    LocalChoreo::Recv { peer, label: out_label, payload_tag: out_tag, .. } => {
                        peer == from && out_label == label && out_tag == payload_tag
                    }
                    _ => false,
                }
            } else {
                true
            }
        }
        GlobalChoreo::Choice { .. } => true,
    }
}

proof fn project_decider_choice_from_some(role: RoleId, global: GChoreo, from: int)
    requires
        0 <= from,
        match global {
            GChoreo::Choice { branches, .. } =>
                forall|i: int| #![trigger branches[i]] from <= i < branches.len() ==> projectable_spec(branches[i].body),
            _ => false,
        },
    ensures project_decider_choice_from_spec(role, global, from).is_some()
    decreases global, 1int, if from < gchoice_branch_count(global) { gchoice_branch_count(global) - from } else { 0int }
{
    match global {
        GChoreo::Choice { branches, .. } => {
            if from < branches.len() {
                projectable_implies_projection_some(role, branches[from].body);
                project_decider_choice_from_some(role, global, from + 1);
            }
        }
        _ => {}
    }
}

pub(crate) proof fn project_decider_choice_from_len(role: RoleId, global: GChoreo, from: int)
    requires
        0 <= from,
        match global {
            GChoreo::Choice { branches, .. } =>
                from <= branches.len()
                    && forall|i: int| #![trigger branches[i]] from <= i < branches.len() ==> projectable_spec(branches[i].body),
            _ => false,
        },
    ensures
        match global {
            GChoreo::Choice { branches, .. } =>
                project_decider_choice_from_spec(role, global, from).unwrap().len() == branches.len() - from,
            _ => false,
        }
    decreases global, 1int, if from < gchoice_branch_count(global) { gchoice_branch_count(global) - from } else { 0int }
{
    project_decider_choice_from_some(role, global, from);
    match global {
        GChoreo::Choice { branches, .. } => {
            if from < branches.len() {
                projectable_implies_projection_some(role, branches[from].body);
                project_decider_choice_from_len(role, global, from + 1);
                let rest = project_decider_choice_from_spec(role, global, from + 1).unwrap();
                let head = LBranch {
                    label: branches[from].label,
                    body: project_endpoint_spec(role, branches[from].body),
                };
                assert(project_local_spec(role, branches[from].body) == Some(head.body));
                assert(project_decider_choice_from_spec(role, global, from).unwrap() == seq![head] + rest);
                assert(rest.len() == branches.len() - (from + 1));
                assert((seq![head] + rest).len() == 1 + rest.len());
                assert(1 + (branches.len() - (from + 1)) == branches.len() - from);
            } else {
                assert(from == branches.len());
                assert(project_decider_choice_from_spec(role, global, from).unwrap().len() == 0);
            }
        }
        _ => {}
    }
}

pub(crate) proof fn project_decider_choice_from_pointwise(role: RoleId, global: GChoreo, from: int, k: int)
    requires
        0 <= from,
        0 <= k,
        match global {
            GChoreo::Choice { branches, .. } =>
                from <= branches.len()
                    && from + k < branches.len()
                    && forall|i: int| #![trigger branches[i]] from <= i < branches.len() ==> projectable_spec(branches[i].body),
            _ => false,
        },
    ensures
        match global {
            GChoreo::Choice { branches, .. } => ({
                let projected = project_decider_choice_from_spec(role, global, from).unwrap();
                k < projected.len()
                    && projected[k].label == branches[from + k].label
                    && projected[k].body == project_endpoint_spec(role, branches[from + k].body)
            }),
            _ => false,
        }
    decreases global, 1int, if from < gchoice_branch_count(global) { gchoice_branch_count(global) - from } else { 0int }
{
    project_decider_choice_from_len(role, global, from);
    match global {
        GChoreo::Choice { branches, .. } => {
            assert(from < branches.len());
            if k == 0 {
                projectable_implies_projection_some(role, branches[from].body);
                project_decider_choice_from_some(role, global, from + 1);
                match project_decider_choice_from_spec(role, global, from + 1) {
                    Some(rest) => {
                        let head = LBranch {
                            label: branches[from].label,
                            body: project_local_spec(role, branches[from].body).unwrap(),
                        };
                        assert(head.body == project_endpoint_spec(role, branches[from].body));
                        assert(match project_local_spec(role, branches[from].body) {
                            Some(body) => match project_decider_choice_from_spec(role, global, from + 1) {
                                Some(tail) => Some(seq![LBranch { label: branches[from].label, body }] + tail),
                                None => None,
                            },
                            None => None,
                        } == Some(seq![head] + rest));
                        assert(project_decider_choice_from_spec(role, global, from)
                            == match project_local_spec(role, branches[from].body) {
                                Some(body) => match project_decider_choice_from_spec(role, global, from + 1) {
                                    Some(tail) => Some(seq![LBranch { label: branches[from].label, body }] + tail),
                                    None => None,
                                },
                                None => None,
                            });
                        assert(project_decider_choice_from_spec(role, global, from) == Some(seq![head] + rest));
                        assert(project_decider_choice_from_spec(role, global, from).unwrap()[0] == head);
                    }
                    None => { assert(false); }
                }
            } else {
                assert(k > 0);
                projectable_implies_projection_some(role, branches[from].body);
                project_decider_choice_from_len(role, global, from + 1);
                project_decider_choice_from_pointwise(role, global, from + 1, k - 1);
                project_decider_choice_from_some(role, global, from + 1);
                match project_decider_choice_from_spec(role, global, from + 1) {
                    Some(rest) => {
                        let head = LBranch {
                            label: branches[from].label,
                            body: project_local_spec(role, branches[from].body).unwrap(),
                        };
                        let projected = project_decider_choice_from_spec(role, global, from).unwrap();
                        assert(head.body == project_endpoint_spec(role, branches[from].body));
                        assert(match project_local_spec(role, branches[from].body) {
                            Some(body) => match project_decider_choice_from_spec(role, global, from + 1) {
                                Some(tail) => Some(seq![LBranch { label: branches[from].label, body }] + tail),
                                None => None,
                            },
                            None => None,
                        } == Some(seq![head] + rest));
                        assert(project_decider_choice_from_spec(role, global, from)
                            == match project_local_spec(role, branches[from].body) {
                                Some(body) => match project_decider_choice_from_spec(role, global, from + 1) {
                                    Some(tail) => Some(seq![LBranch { label: branches[from].label, body }] + tail),
                                    None => None,
                                },
                                None => None,
                            });
                        assert(project_decider_choice_from_spec(role, global, from) == Some(seq![head] + rest));
                        assert(projected == seq![head] + rest);
                        assert(0 <= k - 1 < rest.len());
                        assert(projected[k] == rest[k - 1]);
                        assert(branches[from + k] == branches[(from + 1) + (k - 1)]);
                    }
                    None => { assert(false); }
                }
            }
        }
        _ => {}
    }
}

pub(crate) proof fn project_decider_choice_strict_labels(role: RoleId, decider: RoleId, branches: Seq<GBranch>)
    requires projectable_spec(GChoreo::Choice { decider, branches })
    ensures branch_labels_strict_spec(project_decider_choice_from_spec(role, GChoreo::Choice { decider, branches }, 0).unwrap())
{
    let choice = GChoreo::Choice { decider, branches };
    project_decider_choice_from_len(role, choice, 0);
    assert forall|i: int, j: int| #![trigger project_decider_choice_from_spec(role, choice, 0).unwrap()[i], project_decider_choice_from_spec(role, choice, 0).unwrap()[j]]
        0 <= i < j < project_decider_choice_from_spec(role, choice, 0).unwrap().len()
            implies project_decider_choice_from_spec(role, choice, 0).unwrap()[i].label
                < project_decider_choice_from_spec(role, choice, 0).unwrap()[j].label
    by {
        project_decider_choice_from_pointwise(role, choice, 0, i);
        project_decider_choice_from_pointwise(role, choice, 0, j);
    }
}

pub(crate) proof fn merge_projected_absent_role_is_end(role: RoleId, decider: RoleId, branches: Seq<GBranch>)
    requires
        branches.len() > 0,
        global_branch_labels_strict_spec(branches),
        forall|i: int| #![trigger branches[i]] 0 <= i < branches.len() ==> project_local_spec(role, branches[i].body) == Some(LChoreo::End),
    ensures merge_projected_branches_spec(role, decider, branches) == Some(LChoreo::End)
    decreases branches.len()
{
    let choice = GChoreo::Choice { decider, branches };
    project_decider_choice_all_end(role, choice, 0);
    let projected = labeled_end_branches_from_spec(branches, 0);
    labeled_end_branches_all_end(branches, 0);
    assert(projected.len() > 0);
    assert(projected[0].body == LChoreo::End);
    assert(forall|i: int| #![trigger projected[i]] 0 <= i < projected.len() ==> projected[i].body == LChoreo::End);
    assert(same_projected_bodies_spec(projected));
    assert(projected_branch_suffix_size(projected, 0) > 0);
}

// r[verify choreography.projection]
pub(crate) proof fn sender_projection_shape(from: RoleId, to: RoleId, label: LabelId, payload_tag: PayloadTag, next: GChoreo)
    requires projectable_spec(GChoreo::Comm { from, to, label, payload_tag, next: Box::new(next) })
    ensures project_endpoint_spec(from, GChoreo::Comm { from, to, label, payload_tag, next: Box::new(next) })
        == (LChoreo::Send { peer: to, label, payload_tag, next: Box::new(project_endpoint_spec(from, next)) })
{
    projectable_implies_projection_some(from, next);
}

// r[verify choreography.projection]
pub(crate) proof fn receiver_projection_shape(from: RoleId, to: RoleId, label: LabelId, payload_tag: PayloadTag, next: GChoreo)
    requires projectable_spec(GChoreo::Comm { from, to, label, payload_tag, next: Box::new(next) })
    ensures project_endpoint_spec(to, GChoreo::Comm { from, to, label, payload_tag, next: Box::new(next) })
        == (LChoreo::Recv { peer: from, label, payload_tag, next: Box::new(project_endpoint_spec(to, next)) })
{
    projectable_implies_projection_some(to, next);
}

// r[verify choreography.projection]
pub(crate) proof fn decider_choice_projects_to_internal_choice(decider: RoleId, branches: Seq<GBranch>)
    requires
        projectable_spec(GChoreo::Choice { decider, branches }),
    ensures
        project_endpoint_spec(decider, GChoreo::Choice { decider, branches })
            == (LChoreo::InternalChoice {
                branches: project_decider_choice_from_spec(decider, GChoreo::Choice { decider, branches }, 0).unwrap(),
            })
{
    let choice = GChoreo::Choice { decider, branches };
    assert forall|i: int| #![trigger branches[i]] 0 <= i < branches.len() ==> projectable_spec(branches[i].body) by {};
    project_decider_choice_from_some(decider, choice, 0);
}

// r[verify choreography.projection]
pub(crate) proof fn absent_role_projects_to_end(role: RoleId, global: GChoreo)
    requires
        projectable_spec(global),
        !role_in_global_spec(role, global),
    ensures
        project_local_spec(role, global) == Some(LChoreo::End),
        project_endpoint_spec(role, global) == LChoreo::End
    decreases global
{
    match global {
        GChoreo::End => {}
        GChoreo::Comm { from, to, next, .. } => {
            assert(role != from);
            assert(role != to);
            assert(!role_in_global_spec(role, *next));
            absent_role_projects_to_end(role, *next);
        }
        GChoreo::Choice { decider, branches } => {
            assert(role != decider);
            assert(!role_in_choice_branches_prefix_spec(role, global, branches.len() as int));
            absent_choice_prefix_excludes_role(role, global, branches.len() as int);
            assert forall|i: int| #![trigger branches[i]]
                0 <= i < branches.len() implies project_local_spec(role, branches[i].body) == Some(LChoreo::End)
            by {
                assert(projectable_spec(branches[i].body));
                absent_role_projects_to_end(role, branches[i].body);
            }
            merge_projected_absent_role_is_end(role, decider, branches);
        }
    }
}

// r[verify choreography.projectability]
proof fn non_mergeable_choice_not_projectable(role: RoleId, decider: RoleId, branches: Seq<GBranch>)
    requires
        well_formed_global_spec(GChoreo::Choice { decider, branches }),
        role != decider,
        role_in_branches_spec(role, branches),
        merge_projected_branches_spec(role, decider, branches).is_none(),
    ensures !projectable_spec(GChoreo::Choice { decider, branches })
{
}

#[verifier::exec_allows_no_decreases_clause]
fn project_decider_branches_prefix(
    role: RoleId,
    branches: &[GlobalBranch],
    upto: usize,
) -> (result: Option<Vec<LocalBranch>>)
    requires upto <= branches@.len()
{
    if upto == 0 {
        return Some(Vec::new());
    }
    match project_decider_branches_prefix(role, branches, upto - 1) {
        Some(mut prefix) => match project_local(role, &branches[upto - 1].body) {
            Some(body) => {
                prefix.push(LocalBranch { label: branches[upto - 1].label, body });
                Some(prefix)
            }
            None => None,
        },
        None => None,
    }
}

#[verifier::exec_allows_no_decreases_clause]
fn project_decider_branches(role: RoleId, branches: &[GlobalBranch]) -> (result: Option<Vec<LocalBranch>>)
{
    project_decider_branches_prefix(role, branches, branches.len())
}

#[verifier::exec_allows_no_decreases_clause]
fn clone_projected_branch(branch: &LocalBranch) -> LocalBranch {
    LocalBranch { label: branch.label, body: clone_local(&branch.body) }
}

#[verifier::exec_allows_no_decreases_clause]
fn clone_projected_branches(branches: &[LocalBranch]) -> Vec<LocalBranch> {
    let mut out = Vec::new();
    let mut i: usize = 0;
    while i < branches.len() {
        out.push(clone_projected_branch(&branches[i]));
        i += 1;
    }
    out
}

#[verifier::exec_allows_no_decreases_clause]
fn projected_bodies_identical(branches: &[LocalBranch]) -> bool {
    if branches.is_empty() {
        return false;
    }
    let first = &branches[0].body;
    let mut i: usize = 1;
    while i < branches.len() {
        if branches[i].body != *first {
            return false;
        }
        i += 1;
    }
    true
}

#[verifier::exec_allows_no_decreases_clause]
fn common_send_tails(branches: &[LocalBranch]) -> Option<(RoleId, LabelId, PayloadTag, Vec<LocalBranch>)> {
    if branches.is_empty() {
        return None;
    }
    if let LocalChoreo::Send { peer, label, payload_tag, .. } = &branches[0].body {
        let mut tails: Vec<LocalBranch> = Vec::new();
        let mut i: usize = 0;
        while i < branches.len() {
            if let LocalChoreo::Send { peer: p, label: l, payload_tag: t, next } = &branches[i].body {
                if *p != *peer || *l != *label || *t != *payload_tag {
                    return None;
                }
                tails.push(LocalBranch { label: branches[i].label, body: clone_local(next) });
            } else {
                return None;
            }
            i += 1;
        }
        Some((*peer, *label, *payload_tag, tails))
    } else {
        None
    }
}

#[verifier::exec_allows_no_decreases_clause]
fn common_recv_tails(branches: &[LocalBranch]) -> Option<(RoleId, LabelId, PayloadTag, Vec<LocalBranch>)> {
    if branches.is_empty() {
        return None;
    }
    if let LocalChoreo::Recv { peer, label, payload_tag, .. } = &branches[0].body {
        let mut tails: Vec<LocalBranch> = Vec::new();
        let mut i: usize = 0;
        while i < branches.len() {
            if let LocalChoreo::Recv { peer: p, label: l, payload_tag: t, next } = &branches[i].body {
                if *p != *peer || *l != *label || *t != *payload_tag {
                    return None;
                }
                tails.push(LocalBranch { label: branches[i].label, body: clone_local(next) });
            } else {
                return None;
            }
            i += 1;
        }
        Some((*peer, *label, *payload_tag, tails))
    } else {
        None
    }
}

fn decider_guarded_local(decider: RoleId, local: &LocalChoreo) -> bool {
    match local {
        LocalChoreo::Recv { peer, .. } => *peer == decider,
        LocalChoreo::Offer { from, .. } => *from == decider,
        _ => false,
    }
}

#[verifier::exec_allows_no_decreases_clause]
fn all_decider_guarded(decider: RoleId, branches: &[LocalBranch]) -> bool {
    let mut i: usize = 0;
    while i < branches.len() {
        if !decider_guarded_local(decider, &branches[i].body) {
            return false;
        }
        i += 1;
    }
    true
}

#[verifier::exec_allows_no_decreases_clause]
fn merge_projected_labeled_branches(decider: RoleId, branches: &[LocalBranch]) -> Option<LocalChoreo> {
    if branches.is_empty() {
        return None;
    }
    if projected_bodies_identical(branches) {
        return Some(clone_local(&branches[0].body));
    }
    if let Some((peer, label, payload_tag, tails)) = common_send_tails(branches) {
        if let Some(next) = merge_projected_labeled_branches(decider, &tails) {
            return Some(LocalChoreo::Send { peer, label, payload_tag, next: Box::new(next) });
        }
        return None;
    }
    if let Some((peer, label, payload_tag, tails)) = common_recv_tails(branches) {
        if let Some(next) = merge_projected_labeled_branches(decider, &tails) {
            return Some(LocalChoreo::Recv { peer, label, payload_tag, next: Box::new(next) });
        }
        return None;
    }
    if all_decider_guarded(decider, branches) {
        return Some(LocalChoreo::Offer { from: decider, branches: clone_projected_branches(branches) });
    }
    None
}

#[verifier::exec_allows_no_decreases_clause]
fn merge_projected_branches(role: RoleId, decider: RoleId, branches: &[GlobalBranch]) -> (result: Option<LocalChoreo>)
{
    match project_decider_branches(role, branches) {
        Some(local_branches) => merge_projected_labeled_branches(decider, &local_branches),
        None => None,
    }
}

#[verifier::exec_allows_no_decreases_clause]
fn project_local(role: RoleId, global: &GlobalChoreo) -> (result: Option<LocalChoreo>)
{
    match global {
        GlobalChoreo::End => Some(LocalChoreo::End),
        GlobalChoreo::Comm { from, to, label, payload_tag, next } => {
            if role == *from {
                match project_local(role, next) {
                    Some(next_local) => Some(LocalChoreo::Send {
                        peer: *to,
                        label: *label,
                        payload_tag: *payload_tag,
                        next: Box::new(next_local),
                    }),
                    None => None,
                }
            } else if role == *to {
                match project_local(role, next) {
                    Some(next_local) => Some(LocalChoreo::Recv {
                        peer: *from,
                        label: *label,
                        payload_tag: *payload_tag,
                        next: Box::new(next_local),
                    }),
                    None => None,
                }
            } else {
                project_local(role, next)
            }
        }
        GlobalChoreo::Choice { decider, branches } => {
            if branches.is_empty() {
                None
            } else if role == *decider {
                match project_decider_branches(role, branches) {
                    Some(local_branches) => Some(LocalChoreo::InternalChoice { branches: local_branches }),
                    None => None,
                }
            } else {
                merge_projected_branches(role, *decider, branches)
            }
        }
    }
}

fn participants_in_branches(branches: &[GlobalBranch]) -> Vec<RoleId> {
    let mut roles = Vec::new();
    let mut i: usize = 0;
    while i < branches.len()
        decreases branches.len() - i,
    {
        let branch_roles = crate::choreography_global::participants(&branches[i].body);
        let mut j: usize = 0;
        while j < branch_roles.len()
            decreases branch_roles.len() - j,
        {
            if !crate::choreography_global::contains_role(&roles, branch_roles[j]) {
                roles.push(branch_roles[j]);
            }
            j += 1;
        }
        i += 1;
    }
    roles
}

// r[impl choreography.projectability]
#[verifier::exec_allows_no_decreases_clause]
pub fn projectable(global: &GlobalChoreo) -> (result: bool)
    ensures result ==> crate::choreography_global::well_formed_global_runtime_spec(*global)
{
    match global {
        GlobalChoreo::End => {
            proof {
                reveal(crate::choreography_global::well_formed_global_runtime_spec);
            }
            true
        }
        GlobalChoreo::Comm { from, to, next, .. } => {
            let result = *from != *to && projectable(next);
            proof {
                reveal(crate::choreography_global::well_formed_global_runtime_spec);
                if result {
                    assert(*from != *to);
                    assert(crate::choreography_global::well_formed_global_runtime_spec(**next));
                }
            }
            result
        }
        GlobalChoreo::Choice { decider, branches } => {
            if branches.is_empty() {
                return false;
            }
            if !crate::choreography_global::well_formed_global(global) {
                return false;
            }
            let mut i: usize = 0;
            while i < branches.len()
                decreases branches@.len() - i,
            {
                if !projectable(&branches[i].body) {
                    return false;
                }
                i += 1;
            }
            let roles = participants_in_branches(branches);
            let mut i: usize = 0;
            while i < roles.len()
                decreases roles@.len() - i,
            {
                if roles[i] != *decider && merge_projected_branches(roles[i], *decider, branches).is_none() {
                    return false;
                }
                i += 1;
            }
            proof {
                assert(crate::choreography_global::well_formed_global_runtime_spec(*global));
            }
            true
        }
    }
}

// r[impl choreography.projection]
#[verifier::exec_allows_no_decreases_clause]
pub fn project_endpoint(role: RoleId, global: &GlobalChoreo) -> (result: LocalChoreo)
    requires projectable_exec_spec(*global)
    ensures project_endpoint_shape_post_spec(role, *global, result)
{
    match global {
        GlobalChoreo::End => LocalChoreo::End,
        GlobalChoreo::Comm { from, to, label, payload_tag, next } => {
            proof {
                reveal(projectable_exec_spec);
            }
            if role == *from {
                LocalChoreo::Send {
                    peer: *to,
                    label: *label,
                    payload_tag: *payload_tag,
                    next: Box::new(project_endpoint(role, next)),
                }
            } else if role == *to {
                LocalChoreo::Recv {
                    peer: *from,
                    label: *label,
                    payload_tag: *payload_tag,
                    next: Box::new(project_endpoint(role, next)),
                }
            } else {
                project_endpoint(role, next)
            }
        }
        GlobalChoreo::Choice { decider, branches } => {
            if role == *decider {
                match project_decider_branches(role, branches) {
                    Some(local_branches) => LocalChoreo::InternalChoice { branches: local_branches },
                    None => LocalChoreo::End,
                }
            } else {
                match merge_projected_branches(role, *decider, branches) {
                    Some(local) => local,
                    None => LocalChoreo::End,
                }
            }
        }
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

    // r[verify choreography.projectability]
    #[test]
    fn test_projectable_common_behavior() {
        let global = GlobalChoreo::Choice {
            decider: 0,
            branches: vec![
                branch(1, GlobalChoreo::Comm {
                    from: 1,
                    to: 2,
                    label: 9,
                    payload_tag: 0,
                    next: Box::new(GlobalChoreo::End),
                }),
                branch(3, GlobalChoreo::Comm {
                    from: 1,
                    to: 2,
                    label: 9,
                    payload_tag: 0,
                    next: Box::new(GlobalChoreo::End),
                }),
            ],
        };
        assert!(projectable(&global));
    }

    // r[verify choreography.projection]
    #[test]
    fn test_sender_projection() {
        let global = GlobalChoreo::Comm {
            from: 1,
            to: 2,
            label: 7,
            payload_tag: 9,
            next: Box::new(GlobalChoreo::End),
        };
        let projected = project_endpoint(1, &global);
        assert_eq!(projected, send(2, 7, 9, LocalChoreo::End));
    }

    // r[verify choreography.projection]
    #[test]
    fn test_receiver_projection() {
        let global = GlobalChoreo::Comm {
            from: 1,
            to: 2,
            label: 7,
            payload_tag: 9,
            next: Box::new(GlobalChoreo::End),
        };
        let projected = project_endpoint(2, &global);
        assert_eq!(projected, recv(1, 7, 9, LocalChoreo::End));
    }

    // r[verify choreography.projection]
    #[test]
    fn test_decider_projection() {
        let global = GlobalChoreo::Choice {
            decider: 3,
            branches: vec![branch(1, GlobalChoreo::End), branch(4, GlobalChoreo::End)],
        };
        let projected = project_endpoint(3, &global);
        assert_eq!(projected, LocalChoreo::InternalChoice {
            branches: vec![
                LocalBranch {
                    label: 1,
                    body: LocalChoreo::End
                },
                LocalBranch {
                    label: 4,
                    body: LocalChoreo::End
                },
            ],
        });
    }

    // r[verify choreography.projection]
    #[test]
    fn test_nondecider_offer_uses_outer_choice_labels() {
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
        assert_eq!(projected, LocalChoreo::Offer {
            from: 0,
            branches: vec![
                LocalBranch {
                    label: 10,
                    body: recv(0, 100, 1, LocalChoreo::End)
                },
                LocalBranch {
                    label: 20,
                    body: recv(0, 200, 2, LocalChoreo::End)
                },
            ],
        });
    }

    // r[verify choreography.projection]
    #[test]
    fn test_nonparticipant_projects_to_end() {
        let global = GlobalChoreo::Comm {
            from: 1,
            to: 2,
            label: 7,
            payload_tag: 9,
            next: Box::new(GlobalChoreo::End),
        };
        let projected = project_endpoint(9, &global);
        assert_eq!(projected, LocalChoreo::End);
    }

    // r[verify choreography.projectability]
    #[test]
    fn test_non_projectable_branch_conflict() {
        let global = GlobalChoreo::Choice {
            decider: 0,
            branches: vec![
                branch(1, GlobalChoreo::Comm {
                    from: 1,
                    to: 2,
                    label: 9,
                    payload_tag: 0,
                    next: Box::new(GlobalChoreo::End),
                }),
                branch(3, GlobalChoreo::Comm {
                    from: 2,
                    to: 1,
                    label: 9,
                    payload_tag: 0,
                    next: Box::new(GlobalChoreo::End),
                }),
            ],
        };
        assert!(!projectable(&global));
    }
}
