//! Finite local choreography primitives and exact branch-merge rules.

#![allow(dead_code, clippy::too_many_lines)]

use vstd::prelude::*;

use crate::choreography_global::LabelId;
use crate::choreography_global::PayloadTag;
use crate::choreography_global::RoleId;

verus! {

#[cfg_attr(not(verus_keep_ghost), derive(Clone))]
#[derive(Debug, PartialEq, Eq)]
pub struct LocalBranch {
    pub label: LabelId,
    pub body: LocalChoreo,
}

#[cfg_attr(not(verus_keep_ghost), derive(Clone))]
#[derive(Debug, PartialEq, Eq)]
pub enum LocalChoreo {
    End,
    Send {
        peer: RoleId,
        label: LabelId,
        payload_tag: PayloadTag,
        next: Box<LocalChoreo>,
    },
    Recv {
        peer: RoleId,
        label: LabelId,
        payload_tag: PayloadTag,
        next: Box<LocalChoreo>,
    },
    InternalChoice {
        branches: Vec<LocalBranch>,
    },
    Offer {
        from: RoleId,
        branches: Vec<LocalBranch>,
    },
}

pub(crate) struct LBranch {
    pub label: LabelId,
    pub body: LChoreo,
}

pub(crate) enum LChoreo {
    End,
    Send {
        peer: RoleId,
        label: LabelId,
        payload_tag: PayloadTag,
        next: Box<LChoreo>,
    },
    Recv {
        peer: RoleId,
        label: LabelId,
        payload_tag: PayloadTag,
        next: Box<LChoreo>,
    },
    InternalChoice {
        branches: Seq<LBranch>,
    },
    Offer {
        from: RoleId,
        branches: Seq<LBranch>,
    },
}

pub(crate) open spec fn local_exec_branch_count(local: LocalChoreo) -> int {
    match local {
        LocalChoreo::InternalChoice { branches } => branches@.len() as int,
        LocalChoreo::Offer { branches, .. } => branches@.len() as int,
        _ => 0,
    }
}

pub(crate) open spec fn local_exec_branch_view(local: LocalChoreo, from: int) -> Seq<LBranch>
    decreases local, 0int, if from < local_exec_branch_count(local) { local_exec_branch_count(local) - from } else { 0int }
{
    match local {
        LocalChoreo::InternalChoice { branches } | LocalChoreo::Offer { branches, .. } => {
            if from < 0 || from >= branches@.len() {
                seq![]
            } else {
                seq![LBranch {
                    label: branches[from].label,
                    body: local_view(branches[from].body),
                }] + local_exec_branch_view(local, from + 1)
            }
        }
        _ => seq![],
    }
}

pub(crate) open spec fn lbranches_view(branches: Seq<LocalBranch>) -> Seq<LBranch>
    decreases branches.len()
{
    if branches.len() == 0 {
        seq![]
    } else {
        seq![LBranch {
            label: branches[0].label,
            body: local_view(branches[0].body),
        }] + lbranches_view(branches.subrange(1, branches.len() as int))
    }
}

pub(crate) open spec fn local_view(local: LocalChoreo) -> LChoreo
    decreases local, 1int, 0int
{
    match local {
        LocalChoreo::End => LChoreo::End,
        LocalChoreo::Send { peer, label, payload_tag, next } => LChoreo::Send {
            peer,
            label,
            payload_tag,
            next: Box::new(local_view(*next)),
        },
        LocalChoreo::Recv { peer, label, payload_tag, next } => LChoreo::Recv {
            peer,
            label,
            payload_tag,
            next: Box::new(local_view(*next)),
        },
        LocalChoreo::InternalChoice { .. } => LChoreo::InternalChoice {
            branches: local_exec_branch_view(local, 0),
        },
        LocalChoreo::Offer { from, .. } => LChoreo::Offer {
            from,
            branches: local_exec_branch_view(local, 0),
        },
    }
}

pub(crate) open spec fn option_local_view(opt: Option<LocalChoreo>) -> Option<LChoreo> {
    match opt {
        Some(local) => Some(local_view(local)),
        None => None,
    }
}

pub(crate) open spec fn option_branches_view(opt: Option<Vec<LocalBranch>>) -> Option<Seq<LBranch>> {
    match opt {
        Some(branches) => Some(lbranches_view(branches@)),
        None => None,
    }
}

pub(crate) open spec fn branch_labels_spec(branches: Seq<LBranch>) -> Seq<LabelId> {
    branches.map_values(|branch: LBranch| branch.label)
}

pub(crate) open spec fn branch_labels_strict_spec(branches: Seq<LBranch>) -> bool {
    forall|i: int, j: int| #![trigger branches[i], branches[j]]
        0 <= i < j < branches.len() ==> branches[i].label < branches[j].label
}

pub(crate) open spec fn same_branch_labels_spec(left: Seq<LBranch>, right: Seq<LBranch>) -> bool {
    left.len() == right.len()
        && forall|i: int| #![trigger left[i], right[i]] 0 <= i < left.len() ==> left[i].label == right[i].label
}

pub(crate) open spec fn local_branch_suffix_size(branches: Seq<LBranch>, from: int) -> nat
    decreases if from < branches.len() { branches.len() - from } else { 0 }
{
    if from < 0 || from >= branches.len() {
        0
    } else {
        1 + local_size(branches[from].body) + local_branch_suffix_size(branches, from + 1)
    }
}

pub(crate) open spec fn lchoreo_branch_count(local: LChoreo) -> int {
    match local {
        LChoreo::InternalChoice { branches } => branches.len() as int,
        LChoreo::Offer { branches, .. } => branches.len() as int,
        _ => 0,
    }
}

pub(crate) open spec fn local_choice_branch_size(local: LChoreo, from: int) -> nat
    decreases local, 0int, if from < lchoreo_branch_count(local) { lchoreo_branch_count(local) - from } else { 0int }
{
    match local {
        LChoreo::InternalChoice { branches } | LChoreo::Offer { branches, .. } => {
            if from < 0 || from >= branches.len() {
                0
            } else {
                1 + local_size(branches[from].body) + local_choice_branch_size(local, from + 1)
            }
        }
        _ => 0,
    }
}

pub(crate) open spec fn local_size(local: LChoreo) -> nat
    decreases local, 1int, 0int
{
    match local {
        LChoreo::End => 1,
        LChoreo::Send { next, .. } => 1 + local_size(*next),
        LChoreo::Recv { next, .. } => 1 + local_size(*next),
        LChoreo::InternalChoice { .. } => 1 + local_choice_branch_size(local, 0),
        LChoreo::Offer { .. } => 1 + local_choice_branch_size(local, 0),
    }
}

pub(crate) open spec fn lookup_branch_from_spec(branches: Seq<LBranch>, label: LabelId, from: int) -> Option<LChoreo>
    decreases if from < branches.len() { branches.len() - from } else { 0 }
{
    if from < 0 || from >= branches.len() {
        None
    } else if branches[from].label == label {
        Some(branches[from].body)
    } else {
        lookup_branch_from_spec(branches, label, from + 1)
    }
}

pub(crate) open spec fn lookup_branch_spec(branches: Seq<LBranch>, label: LabelId) -> Option<LChoreo>
    decreases branches.len()
{
    lookup_branch_from_spec(branches, label, 0)
}

pub(crate) open spec fn well_formed_local_branches_spec(branches: Seq<LBranch>) -> bool
    decreases branches.len()
{
    branches.len() > 0
        && branch_labels_strict_spec(branches)
        && forall|i: int| #![trigger branches[i]] 0 <= i < branches.len() ==> well_formed_local_spec(branches[i].body)
}

pub(crate) open spec fn well_formed_local_choice_prefix_spec(local: LChoreo, upto: int) -> bool
    decreases local, 0int, if upto > 0 { upto } else { 0int }
{
    match local {
        LChoreo::InternalChoice { branches } | LChoreo::Offer { branches, .. } => {
            if upto <= 0 || upto > branches.len() {
                upto <= 0
            } else {
                well_formed_local_choice_prefix_spec(local, upto - 1)
                    && well_formed_local_spec(branches[upto - 1].body)
            }
        }
        _ => false,
    }
}

// r[depends choreography.merge]
// r[impl choreography.merge]
pub(crate) open spec fn well_formed_local_spec(local: LChoreo) -> bool
    decreases local, 1int, 0int
{
    match local {
        LChoreo::End => true,
        LChoreo::Send { next, .. } => well_formed_local_spec(*next),
        LChoreo::Recv { next, .. } => well_formed_local_spec(*next),
        LChoreo::InternalChoice { branches } => {
            branches.len() > 0
                && branch_labels_strict_spec(branches)
                && well_formed_local_choice_prefix_spec(local, branches.len() as int)
        }
        LChoreo::Offer { branches, .. } => {
            branches.len() > 0
                && branch_labels_strict_spec(branches)
                && well_formed_local_choice_prefix_spec(local, branches.len() as int)
        }
    }
}

// Branch merge for InternalChoice: takes parent LChoreo for structural decrease
pub(crate) open spec fn merge_choice_branches_from_spec(
    decider: RoleId,
    left_parent: LChoreo,
    right_parent: LChoreo,
    from: int,
) -> Option<Seq<LBranch>>
    decreases left_parent, 0int, right_parent, 0int,
        if from >= 0 && from < lchoreo_branch_count(left_parent) { (lchoreo_branch_count(left_parent) - from) as nat } else { 0nat }
{
    match left_parent {
        LChoreo::InternalChoice { branches: left } => match right_parent {
            LChoreo::InternalChoice { branches: right } => {
                if from < 0 || from > left.len() || from > right.len() {
                    None
                } else if from == left.len() && from == right.len() {
                    Some(seq![])
                } else if from < left.len() && from < right.len() && left[from].label == right[from].label {
                    match merge_local_spec(decider, left[from].body, right[from].body) {
                        Some(body) => match merge_choice_branches_from_spec(
                            decider,
                            LChoreo::InternalChoice { branches: left },
                            LChoreo::InternalChoice { branches: right },
                            from + 1,
                        ) {
                            Some(tail) => Some(seq![LBranch { label: left[from].label, body }] + tail),
                            None => None,
                        },
                        None => None,
                    }
                } else {
                    None
                }
            },
            _ => None,
        },
        _ => None,
    }
}

// Branch merge for Offer: takes parent LChoreo for structural decrease
pub(crate) open spec fn merge_offer_branches_from_spec(
    decider: RoleId,
    left_parent: LChoreo,
    right_parent: LChoreo,
    li: int,
    ri: int,
) -> Option<Seq<LBranch>>
    decreases left_parent, 0int, right_parent, 0int,
        if li >= 0 && ri >= 0 && li <= lchoreo_branch_count(left_parent) && ri <= lchoreo_branch_count(right_parent)
        { (lchoreo_branch_count(left_parent) - li + lchoreo_branch_count(right_parent) - ri) as nat } else { 0nat }
{
    match left_parent {
        LChoreo::Offer { from: left_from, branches: left } => match right_parent {
            LChoreo::Offer { from: right_from, branches: right } => {
                if li < 0 || ri < 0 || li > left.len() || ri > right.len() {
                    None
                } else if li == left.len() && ri == right.len() {
                    Some(seq![])
                } else if li == left.len() {
                    Some(right.subrange(ri, right.len() as int))
                } else if ri == right.len() {
                    Some(left.subrange(li, left.len() as int))
                } else if left[li].label < right[ri].label {
                    match merge_offer_branches_from_spec(
                        decider,
                        LChoreo::Offer { from: left_from, branches: left },
                        LChoreo::Offer { from: right_from, branches: right },
                        li + 1, ri,
                    ) {
                        Some(tail) => Some(seq![left[li]] + tail),
                        None => None,
                    }
                } else if right[ri].label < left[li].label {
                    match merge_offer_branches_from_spec(
                        decider,
                        LChoreo::Offer { from: left_from, branches: left },
                        LChoreo::Offer { from: right_from, branches: right },
                        li, ri + 1,
                    ) {
                        Some(tail) => Some(seq![right[ri]] + tail),
                        None => None,
                    }
                } else {
                    match merge_local_spec(decider, left[li].body, right[ri].body) {
                        Some(body) => match merge_offer_branches_from_spec(
                            decider,
                            LChoreo::Offer { from: left_from, branches: left },
                            LChoreo::Offer { from: right_from, branches: right },
                            li + 1, ri + 1,
                        ) {
                            Some(tail) => Some(seq![LBranch { label: left[li].label, body }] + tail),
                            None => None,
                        },
                        None => None,
                    }
                }
            },
            _ => None,
        },
        _ => None,
    }
}

// r[depends choreography.merge]
// r[impl choreography.merge]
pub(crate) open spec fn merge_local_spec(decider: RoleId, left: LChoreo, right: LChoreo) -> Option<LChoreo>
    decreases left, 1int, right, 1int, 0nat
{
    match (left, right) {
        (LChoreo::End, LChoreo::End) => Some(LChoreo::End),
        (
            LChoreo::Send { peer: l_peer, label: l_label, payload_tag: l_payload, next: l_next },
            LChoreo::Send { peer: r_peer, label: r_label, payload_tag: r_payload, next: r_next },
        ) => {
            if l_peer == r_peer && l_label == r_label && l_payload == r_payload {
                match merge_local_spec(decider, *l_next, *r_next) {
                    Some(next) => Some(LChoreo::Send {
                        peer: l_peer,
                        label: l_label,
                        payload_tag: l_payload,
                        next: Box::new(next),
                    }),
                    None => None,
                }
            } else {
                None
            }
        }
        (
            LChoreo::Recv { peer: l_peer, label: l_label, payload_tag: l_payload, next: l_next },
            LChoreo::Recv { peer: r_peer, label: r_label, payload_tag: r_payload, next: r_next },
        ) => {
            if l_peer == r_peer && l_label == r_label && l_payload == r_payload {
                match merge_local_spec(decider, *l_next, *r_next) {
                    Some(next) => Some(LChoreo::Recv {
                        peer: l_peer,
                        label: l_label,
                        payload_tag: l_payload,
                        next: Box::new(next),
                    }),
                    None => None,
                }
            } else if l_peer == decider && r_peer == decider && l_label != r_label {
                let left_branch = LBranch {
                    label: l_label,
                    body: LChoreo::Recv {
                        peer: l_peer,
                        label: l_label,
                        payload_tag: l_payload,
                        next: l_next,
                    },
                };
                let right_branch = LBranch {
                    label: r_label,
                    body: LChoreo::Recv {
                        peer: r_peer,
                        label: r_label,
                        payload_tag: r_payload,
                        next: r_next,
                    },
                };
                if l_label < r_label {
                    Some(LChoreo::Offer { from: decider, branches: seq![left_branch, right_branch] })
                } else {
                    Some(LChoreo::Offer { from: decider, branches: seq![right_branch, left_branch] })
                }
            } else {
                None
            }
        }
        (LChoreo::InternalChoice { branches: left_branches }, LChoreo::InternalChoice { branches: right_branches }) => {
            match merge_choice_branches_from_spec(
                decider,
                LChoreo::InternalChoice { branches: left_branches },
                LChoreo::InternalChoice { branches: right_branches },
                0,
            ) {
                Some(branches) => Some(LChoreo::InternalChoice { branches }),
                None => None,
            }
        }
        (
            LChoreo::Offer { from: left_from, branches: left_branches },
            LChoreo::Offer { from: right_from, branches: right_branches },
        ) => {
            if left_from == decider && right_from == decider {
                match merge_offer_branches_from_spec(
                    decider,
                    LChoreo::Offer { from: left_from, branches: left_branches },
                    LChoreo::Offer { from: right_from, branches: right_branches },
                    0, 0,
                ) {
                    Some(branches) => Some(LChoreo::Offer { from: decider, branches }),
                    None => None,
                }
            } else {
                None
            }
        }
        _ => None,
    }
}

// r[impl choreography.merge]
pub closed spec fn well_formed_local_exec_spec(local: LocalChoreo) -> bool {
    well_formed_local_spec(local_view(local))
}

pub open spec fn local_exec_branch_labels_strict_spec(branches: Seq<LocalBranch>) -> bool {
    forall|i: int, j: int| #![trigger branches[i], branches[j]]
        0 <= i < j < branches.len() ==> branches[i].label < branches[j].label
}

pub closed spec fn well_formed_local_runtime_branches_spec(branches: Seq<LocalBranch>) -> bool {
    branches.len() > 0
        && local_exec_branch_labels_strict_spec(branches)
        && forall|i: int| #![trigger branches[i]]
            0 <= i < branches.len() ==> well_formed_local_result_spec(branches[i].body)
}

pub open spec fn well_formed_local_result_spec(local: LocalChoreo) -> bool
    decreases local, 1int, 0int
{
    match local {
        LocalChoreo::End => true,
        LocalChoreo::Send { next, .. } => well_formed_local_result_spec(*next),
        LocalChoreo::Recv { next, .. } => well_formed_local_result_spec(*next),
        LocalChoreo::InternalChoice { branches } =>
            branches.len() > 0
                && local_exec_branch_labels_strict_spec(branches@)
                && forall|i: int| #![trigger branches@[i]]
                    0 <= i < branches@.len() ==> well_formed_local_result_spec(branches@[i].body),
        LocalChoreo::Offer { branches, .. } =>
            branches.len() > 0
                && local_exec_branch_labels_strict_spec(branches@)
                && forall|i: int| #![trigger branches@[i]]
                    0 <= i < branches@.len() ==> well_formed_local_result_spec(branches@[i].body),
    }
}

pub open spec fn well_formed_local_runtime_spec(local: LocalChoreo) -> bool
    decreases local, 1int, 0int
{
    match local {
        LocalChoreo::End => true,
        LocalChoreo::Send { next, .. } => well_formed_local_runtime_spec(*next),
        LocalChoreo::Recv { next, .. } => well_formed_local_runtime_spec(*next),
        LocalChoreo::InternalChoice { branches } | LocalChoreo::Offer { branches, .. } =>
            branches.len() > 0
                && local_exec_branch_labels_strict_spec(branches@)
                && forall|i: int| #![trigger branches@[i]]
                    0 <= i < branches@.len() ==> well_formed_local_runtime_spec(branches@[i].body),
    }
}

pub closed spec fn well_formed_local_post_spec(local: LocalChoreo, result: bool) -> bool {
    match local {
        LocalChoreo::End => result == true,
        LocalChoreo::Send { next, .. } => result == well_formed_local_runtime_spec(*next),
        LocalChoreo::Recv { next, .. } => result == well_formed_local_runtime_spec(*next),
        LocalChoreo::InternalChoice { branches } | LocalChoreo::Offer { branches, .. } =>
            result == (branches.len() > 0
                && local_exec_branch_labels_strict_spec(branches@)
                && forall|i: int| #![trigger branches@[i]]
                    0 <= i < branches@.len() ==> well_formed_local_runtime_spec(branches@[i].body)),
    }
}

proof fn local_adjacent_increasing_prefix_implies_pairwise_prefix(branches: Seq<LocalBranch>, n: int)
    requires
        0 <= n <= branches.len(),
        forall|j: int| #![trigger branches[j]]
            0 < j < n ==> branches[j - 1].label < branches[j].label,
    ensures
        forall|i: int, j: int| #![trigger branches[i], branches[j]]
            0 <= i < j < n ==> branches[i].label < branches[j].label
    decreases if n > 0 { n } else { 0int }
{
    if n > 1 {
        local_adjacent_increasing_prefix_implies_pairwise_prefix(branches, n - 1);
        assert forall|i: int, j: int| #![trigger branches[i], branches[j]]
            0 <= i < j < n implies branches[i].label < branches[j].label by {
            if j < n - 1 {
                assert(branches[i].label < branches[j].label);
            } else {
                assert(j == n - 1);
                if i == n - 2 {
                    assert(branches[i].label < branches[j].label);
                } else {
                    assert(0 <= i);
                    assert(i < n - 2);
                    assert(n - 2 < n - 1);
                    assert(branches[i].label < branches[n - 2].label);
                    assert(branches[n - 2].label < branches[n - 1].label);
                }
            }
        };
    }
}

proof fn local_adjacent_violation_exec_not_strict(branches: Seq<LocalBranch>, bad: int)
    requires
        0 < bad < branches.len(),
        branches[bad - 1].label >= branches[bad].label,
    ensures
        !local_exec_branch_labels_strict_spec(branches)
{
    assert(!local_exec_branch_labels_strict_spec(branches)) by {
        assert(0 <= bad - 1);
        assert(bad - 1 < bad);
        assert(bad < branches.len());
        assert(branches[bad - 1].label >= branches[bad].label);
    }
}

// r[impl choreography.merge]
pub closed spec fn merge_local_post_spec(
    decider: RoleId,
    left: LocalChoreo,
    right: LocalChoreo,
    result: Option<LocalChoreo>,
) -> bool {
    match result {
        None => true,
        Some(local) => {
            match (left, right) {
                (LocalChoreo::End, LocalChoreo::End) => local == LocalChoreo::End,
                (
                    LocalChoreo::Send { peer: left_peer, label: left_label, payload_tag: left_payload, .. },
                    LocalChoreo::Send { peer: right_peer, label: right_label, payload_tag: right_payload, .. },
                ) => {
                    left_peer == right_peer && left_label == right_label && left_payload == right_payload
                        && match local {
                            LocalChoreo::Send { peer, label, payload_tag, .. } => {
                                peer == left_peer && label == left_label && payload_tag == left_payload
                            }
                            _ => false,
                        }
                }
                (
                    LocalChoreo::Recv { peer: left_peer, label: left_label, payload_tag: left_payload, .. },
                    LocalChoreo::Recv { peer: right_peer, label: right_label, payload_tag: right_payload, .. },
                ) => {
                    if left_peer == right_peer && left_label == right_label && left_payload == right_payload {
                        match local {
                            LocalChoreo::Recv { peer, label, payload_tag, .. } => {
                                peer == left_peer && label == left_label && payload_tag == left_payload
                            }
                            _ => false,
                        }
                    } else if left_peer == decider && right_peer == decider && left_label != right_label {
                        match local {
                            LocalChoreo::Offer { from, branches } => from == decider && branches.len() == 2,
                            _ => false,
                        }
                    } else {
                        false
                    }
                }
                (LocalChoreo::InternalChoice { .. }, LocalChoreo::InternalChoice { .. }) => {
                    match local {
                        LocalChoreo::InternalChoice { .. } => true,
                        _ => false,
                    }
                }
                (
                    LocalChoreo::Offer { from: left_from, .. },
                    LocalChoreo::Offer { from: right_from, .. },
                ) => {
                    left_from == decider && right_from == decider
                        && match local {
                            LocalChoreo::Offer { from, .. } => from == decider,
                            _ => false,
                        }
                }
                _ => false,
            }
        }
    }
}

proof fn prepend_branch_view(branch: LBranch, tail: Seq<LBranch>)
    ensures (seq![branch] + tail)[0] == branch
{
}

proof fn clone_suffix_view(branches: Seq<LBranch>, from: int)
    requires 0 <= from <= branches.len()
    ensures branches.subrange(from, branches.len() as int).len() == branches.len() - from
{
}

proof fn prepend_preserves_strict_labels(head: LBranch, tail: Seq<LBranch>)
    requires
        branch_labels_strict_spec(tail),
        tail.len() > 0 ==> head.label < tail[0].label,
    ensures
        branch_labels_strict_spec(seq![head] + tail)
{
    let result = seq![head] + tail;
    assert forall|i: int, j: int| #![trigger result[i], result[j]]
        0 <= i < j < result.len() implies result[i].label < result[j].label
    by {
        if i == 0 {
            assert(result[0] == head);
            assert(result[j] == tail[j - 1]);
            if j == 1 {
                assert(result[j] == tail[0]);
            } else {
                assert(0 < j - 1 < tail.len());
                assert(tail[0].label < tail[j - 1].label);
            }
        } else {
            assert(result[i] == tail[i - 1]);
            assert(result[j] == tail[j - 1]);
            assert(0 <= i - 1 < j - 1 < tail.len());
        }
    }
}

proof fn subrange_preserves_strict_labels(branches: Seq<LBranch>, from: int, to: int)
    requires
        branch_labels_strict_spec(branches),
        0 <= from <= to <= branches.len(),
    ensures
        branch_labels_strict_spec(branches.subrange(from, to))
{
    let sub = branches.subrange(from, to);
    assert forall|i: int, j: int| #![trigger sub[i], sub[j]]
        0 <= i < j < sub.len() implies sub[i].label < sub[j].label
    by {
        assert(sub[i] == branches[from + i]);
        assert(sub[j] == branches[from + j]);
        assert(0 <= from + i < from + j < branches.len());
    }
}

// r[verify choreography.merge]
proof fn merge_identical_prefix_exact(
    decider: RoleId,
    peer: RoleId,
    label: LabelId,
    payload_tag: PayloadTag,
    left_next: LChoreo,
    right_next: LChoreo,
)
    ensures
        merge_local_spec(
            decider,
            LChoreo::Recv { peer, label, payload_tag, next: Box::new(left_next) },
            LChoreo::Recv { peer, label, payload_tag, next: Box::new(right_next) },
        ) == match merge_local_spec(decider, left_next, right_next) {
            Some(next) => Some(LChoreo::Recv { peer, label, payload_tag, next: Box::new(next) }),
            None => None,
        }
{
}

proof fn well_formed_prefix_implies_branch_wf(local: LChoreo, n: int, idx: int)
    requires
        0 <= idx < n,
        well_formed_local_choice_prefix_spec(local, n),
        match local {
            LChoreo::InternalChoice { branches } | LChoreo::Offer { branches, .. } =>
                idx < branches.len(),
            _ => false,
        },
    ensures
        match local {
            LChoreo::InternalChoice { branches } | LChoreo::Offer { branches, .. } =>
                well_formed_local_spec(branches[idx].body),
            _ => false,
        }
    decreases n - idx
{
    if idx == n - 1 {
        // well_formed_local_choice_prefix_spec(local, n) directly gives branches[n-1].body
    } else {
        // well_formed_local_choice_prefix_spec(local, n) implies well_formed_local_choice_prefix_spec(local, n-1)
        well_formed_prefix_implies_branch_wf(local, n - 1, idx);
    }
}

proof fn well_formed_local_implies_branch_wf(local: LChoreo)
    requires well_formed_local_spec(local)
    ensures
        match local {
            LChoreo::InternalChoice { branches } | LChoreo::Offer { branches, .. } =>
                branches.len() > 0
                && branch_labels_strict_spec(branches)
                && forall|i: int| #![trigger branches[i]] 0 <= i < branches.len()
                    ==> well_formed_local_spec(branches[i].body),
            _ => true,
        }
{
    match local {
        LChoreo::InternalChoice { branches } | LChoreo::Offer { branches, .. } => {
            assert forall|i: int| #![trigger branches[i]]
                0 <= i < branches.len()
                implies well_formed_local_spec(branches[i].body)
            by {
                well_formed_prefix_implies_branch_wf(local, branches.len() as int, i);
            }
        }
        _ => {}
    }
}

// r[verify choreography.merge]
proof fn merge_recv_distinct_labels_into_offer(
    decider: RoleId,
    left_label: LabelId,
    right_label: LabelId,
    left_payload: PayloadTag,
    right_payload: PayloadTag,
    left_next: LChoreo,
    right_next: LChoreo,
)
    requires left_label != right_label
    ensures
        merge_local_spec(
            decider,
            LChoreo::Recv { peer: decider, label: left_label, payload_tag: left_payload, next: Box::new(left_next) },
            LChoreo::Recv { peer: decider, label: right_label, payload_tag: right_payload, next: Box::new(right_next) },
        ) == if left_label < right_label {
            Some(LChoreo::Offer {
                from: decider,
                branches: seq![
                    LBranch {
                        label: left_label,
                        body: LChoreo::Recv { peer: decider, label: left_label, payload_tag: left_payload, next: Box::new(left_next) },
                    },
                    LBranch {
                        label: right_label,
                        body: LChoreo::Recv { peer: decider, label: right_label, payload_tag: right_payload, next: Box::new(right_next) },
                    },
                ],
            })
        } else {
            Some(LChoreo::Offer {
                from: decider,
                branches: seq![
                    LBranch {
                        label: right_label,
                        body: LChoreo::Recv { peer: decider, label: right_label, payload_tag: right_payload, next: Box::new(right_next) },
                    },
                    LBranch {
                        label: left_label,
                        body: LChoreo::Recv { peer: decider, label: left_label, payload_tag: left_payload, next: Box::new(left_next) },
                    },
                ],
            })
        }
{
}

// r[verify choreography.merge]
proof fn merge_conflicting_heads_fail(
    decider: RoleId,
    send_peer: RoleId,
    recv_peer: RoleId,
    send_label: LabelId,
    recv_label: LabelId,
    send_payload: PayloadTag,
    recv_payload: PayloadTag,
    send_next: LChoreo,
    recv_next: LChoreo,
)
    ensures
        merge_local_spec(
            decider,
            LChoreo::Send { peer: send_peer, label: send_label, payload_tag: send_payload, next: Box::new(send_next) },
            LChoreo::Recv { peer: recv_peer, label: recv_label, payload_tag: recv_payload, next: Box::new(recv_next) },
        ).is_none()
{
}

proof fn merge_choice_branches_well_formed(
    decider: RoleId,
    left_parent: LChoreo,
    right_parent: LChoreo,
    from: int,
)
    requires
        well_formed_local_spec(left_parent),
        well_formed_local_spec(right_parent),
        0 <= from,
        merge_choice_branches_from_spec(decider, left_parent, right_parent, from).is_some(),
    ensures
        branch_labels_strict_spec(
            merge_choice_branches_from_spec(decider, left_parent, right_parent, from).unwrap(),
        ),
        forall|i: int|
            #![trigger merge_choice_branches_from_spec(decider, left_parent, right_parent, from).unwrap()[i]]
            0 <= i < merge_choice_branches_from_spec(decider, left_parent, right_parent, from).unwrap().len()
                ==> well_formed_local_spec(
                    merge_choice_branches_from_spec(decider, left_parent, right_parent, from).unwrap()[i].body,
                ),
        match (left_parent, right_parent) {
            (LChoreo::InternalChoice { branches: left }, LChoreo::InternalChoice { branches: right }) =>
                merge_choice_branches_from_spec(decider, left_parent, right_parent, from).unwrap().len() == left.len() - from
                    && merge_choice_branches_from_spec(decider, left_parent, right_parent, from).unwrap().len() == right.len() - from
                    && (from < left.len() ==> merge_choice_branches_from_spec(decider, left_parent, right_parent, from).unwrap()[0].label == left[from].label),
            _ => true,
        },
    decreases left_parent, 0int, right_parent, 0int,
        if from >= 0 && from < lchoreo_branch_count(left_parent) { (lchoreo_branch_count(left_parent) - from) as nat } else { 0nat }
{
    match left_parent {
        LChoreo::InternalChoice { branches: left } => match right_parent {
            LChoreo::InternalChoice { branches: right } => {
                well_formed_local_implies_branch_wf(left_parent);
                well_formed_local_implies_branch_wf(right_parent);
                if from == left.len() && from == right.len() {
                    let result = merge_choice_branches_from_spec(decider, left_parent, right_parent, from).unwrap();
                    assert(result.len() == 0);
                } else {
                    assert(from < left.len() && from < right.len() && left[from].label == right[from].label);
                    assert(well_formed_local_choice_prefix_spec(left_parent, left.len() as int));
                    assert(well_formed_local_choice_prefix_spec(right_parent, right.len() as int));
                    well_formed_prefix_implies_branch_wf(left_parent, left.len() as int, from);
                    well_formed_prefix_implies_branch_wf(right_parent, right.len() as int, from);
                    assert(well_formed_local_spec(left[from].body));
                    assert(well_formed_local_spec(right[from].body));
                    merge_preserves_well_formed_local(decider, left[from].body, right[from].body);
                    merge_choice_branches_well_formed(decider, left_parent, right_parent, from + 1);
                    let body = merge_local_spec(decider, left[from].body, right[from].body).unwrap();
                    let head = LBranch { label: left[from].label, body };
                    let tail = merge_choice_branches_from_spec(decider, left_parent, right_parent, from + 1).unwrap();
                    let result = merge_choice_branches_from_spec(decider, left_parent, right_parent, from).unwrap();
                    assert(result == seq![head] + tail);
                    assert(result.len() == left.len() - from);
                    assert(result.len() == right.len() - from);
                    assert(result[0].label == left[from].label);
                    if tail.len() > 0 {
                        assert(from + 1 < left.len());
                        assert(tail[0].label == left[from + 1].label);
                        assert(left[from].label < left[from + 1].label);
                    }
                    prepend_preserves_strict_labels(head, tail);
                    assert forall|i: int| #![trigger result[i]]
                        0 <= i < result.len() implies well_formed_local_spec(result[i].body)
                    by {
                        if i == 0 {
                            assert(result[0] == head);
                        } else {
                            assert(0 <= i - 1 < tail.len());
                            assert(result[i] == tail[i - 1]);
                        }
                    }
                }
            }
            _ => {}
        },
        _ => {}
    }
}

proof fn internal_choice_prefix_from_all_wf(branches: Seq<LBranch>, n: int)
    requires
        0 <= n <= branches.len(),
        forall|i: int| #![trigger branches[i]] 0 <= i < n ==> well_formed_local_spec(branches[i].body),
    ensures
        well_formed_local_choice_prefix_spec(LChoreo::InternalChoice { branches }, n)
    decreases n
{
    if n > 0 {
        internal_choice_prefix_from_all_wf(branches, n - 1);
        assert(well_formed_local_spec(branches[n - 1].body));
        assert(well_formed_local_choice_prefix_spec(LChoreo::InternalChoice { branches }, n)
            == (well_formed_local_choice_prefix_spec(LChoreo::InternalChoice { branches }, n - 1)
                && well_formed_local_spec(branches[n - 1].body)));
    }
}

proof fn offer_prefix_from_all_wf(decider: RoleId, branches: Seq<LBranch>, n: int)
    requires
        0 <= n <= branches.len(),
        forall|i: int| #![trigger branches[i]] 0 <= i < n ==> well_formed_local_spec(branches[i].body),
    ensures
        well_formed_local_choice_prefix_spec(LChoreo::Offer { from: decider, branches }, n)
    decreases n
{
    if n > 0 {
        offer_prefix_from_all_wf(decider, branches, n - 1);
        assert(well_formed_local_spec(branches[n - 1].body));
        assert(well_formed_local_choice_prefix_spec(LChoreo::Offer { from: decider, branches }, n)
            == (well_formed_local_choice_prefix_spec(LChoreo::Offer { from: decider, branches }, n - 1)
                && well_formed_local_spec(branches[n - 1].body)));
    }
}

proof fn merge_offer_branches_well_formed(
    decider: RoleId,
    left_parent: LChoreo,
    right_parent: LChoreo,
    li: int,
    ri: int,
)
    requires
        well_formed_local_spec(left_parent),
        well_formed_local_spec(right_parent),
        0 <= li,
        0 <= ri,
        merge_offer_branches_from_spec(decider, left_parent, right_parent, li, ri).is_some(),
    ensures
        branch_labels_strict_spec(
            merge_offer_branches_from_spec(decider, left_parent, right_parent, li, ri).unwrap(),
        ),
        forall|i: int|
            #![trigger merge_offer_branches_from_spec(decider, left_parent, right_parent, li, ri).unwrap()[i]]
            0 <= i < merge_offer_branches_from_spec(decider, left_parent, right_parent, li, ri).unwrap().len()
                ==> well_formed_local_spec(
                    merge_offer_branches_from_spec(decider, left_parent, right_parent, li, ri).unwrap()[i].body,
                ),
        match (left_parent, right_parent) {
            (LChoreo::Offer { branches: left, .. }, LChoreo::Offer { branches: right, .. }) => ({
                let result = merge_offer_branches_from_spec(decider, left_parent, right_parent, li, ri).unwrap();
                ((li < left.len() || ri < right.len()) ==> result.len() > 0)
                    && (result.len() > 0 ==> (
                        (li < left.len() && result[0].label == left[li].label)
                        || (ri < right.len() && result[0].label == right[ri].label)
                    ))
            }),
            _ => true,
        },
    decreases left_parent, 0int, right_parent, 0int,
        if li >= 0 && ri >= 0 && li <= lchoreo_branch_count(left_parent) && ri <= lchoreo_branch_count(right_parent)
        { (lchoreo_branch_count(left_parent) - li + lchoreo_branch_count(right_parent) - ri) as nat } else { 0nat }
{
    match left_parent {
        LChoreo::Offer { from: left_from, branches: left } => match right_parent {
            LChoreo::Offer { from: right_from, branches: right } => {
                well_formed_local_implies_branch_wf(left_parent);
                well_formed_local_implies_branch_wf(right_parent);
                if li == left.len() && ri == right.len() {
                    let result = merge_offer_branches_from_spec(decider, left_parent, right_parent, li, ri).unwrap();
                    assert(result.len() == 0);
                } else if li == left.len() {
                    let result = merge_offer_branches_from_spec(decider, left_parent, right_parent, li, ri).unwrap();
                    assert(result == right.subrange(ri, right.len() as int));
                    assert(well_formed_local_choice_prefix_spec(right_parent, right.len() as int));
                    clone_suffix_view(right, ri);
                    subrange_preserves_strict_labels(right, ri, right.len() as int);
                    assert(result.len() > 0);
                    assert(result[0] == right[ri]);
                    assert forall|i: int| #![trigger result[i]]
                        0 <= i < result.len() implies well_formed_local_spec(result[i].body)
                    by {
                        assert(result[i] == right[ri + i]);
                        assert(0 <= ri + i < right.len());
                        well_formed_prefix_implies_branch_wf(right_parent, right.len() as int, ri + i);
                        assert(well_formed_local_spec(right[ri + i].body));
                    }
                } else if ri == right.len() {
                    let result = merge_offer_branches_from_spec(decider, left_parent, right_parent, li, ri).unwrap();
                    assert(result == left.subrange(li, left.len() as int));
                    assert(well_formed_local_choice_prefix_spec(left_parent, left.len() as int));
                    clone_suffix_view(left, li);
                    subrange_preserves_strict_labels(left, li, left.len() as int);
                    assert(result.len() > 0);
                    assert(result[0] == left[li]);
                    assert forall|i: int| #![trigger result[i]]
                        0 <= i < result.len() implies well_formed_local_spec(result[i].body)
                    by {
                        assert(result[i] == left[li + i]);
                        assert(0 <= li + i < left.len());
                        well_formed_prefix_implies_branch_wf(left_parent, left.len() as int, li + i);
                        assert(well_formed_local_spec(left[li + i].body));
                    }
                } else if left[li].label < right[ri].label {
                    assert(well_formed_local_choice_prefix_spec(left_parent, left.len() as int));
                    well_formed_prefix_implies_branch_wf(left_parent, left.len() as int, li);
                    merge_offer_branches_well_formed(decider, left_parent, right_parent, li + 1, ri);
                    let head = left[li];
                    let tail = merge_offer_branches_from_spec(decider, left_parent, right_parent, li + 1, ri).unwrap();
                    let result = merge_offer_branches_from_spec(decider, left_parent, right_parent, li, ri).unwrap();
                    assert(result == seq![head] + tail);
                    assert(result.len() > 0);
                    assert(result[0].label == left[li].label);
                    if tail.len() > 0 {
                        assert((li + 1 < left.len() && tail[0].label == left[li + 1].label)
                            || (ri < right.len() && tail[0].label == right[ri].label));
                        if li + 1 < left.len() && tail[0].label == left[li + 1].label {
                            assert(head.label < tail[0].label);
                        }
                        if ri < right.len() && tail[0].label == right[ri].label {
                            assert(head.label < tail[0].label);
                        }
                    }
                    prepend_preserves_strict_labels(head, tail);
                    assert forall|i: int| #![trigger result[i]]
                        0 <= i < result.len() implies well_formed_local_spec(result[i].body)
                    by {
                        if i == 0 {
                            assert(result[0] == head);
                            assert(well_formed_local_spec(head.body));
                        } else {
                            assert(0 <= i - 1 < tail.len());
                            assert(result[i] == tail[i - 1]);
                            assert(well_formed_local_spec(tail[i - 1].body));
                        }
                    }
                } else if right[ri].label < left[li].label {
                    assert(well_formed_local_choice_prefix_spec(right_parent, right.len() as int));
                    well_formed_prefix_implies_branch_wf(right_parent, right.len() as int, ri);
                    merge_offer_branches_well_formed(decider, left_parent, right_parent, li, ri + 1);
                    let head = right[ri];
                    let tail = merge_offer_branches_from_spec(decider, left_parent, right_parent, li, ri + 1).unwrap();
                    let result = merge_offer_branches_from_spec(decider, left_parent, right_parent, li, ri).unwrap();
                    assert(result == seq![head] + tail);
                    assert(result.len() > 0);
                    assert(result[0].label == right[ri].label);
                    if tail.len() > 0 {
                        assert((li < left.len() && tail[0].label == left[li].label)
                            || (ri + 1 < right.len() && tail[0].label == right[ri + 1].label));
                        if li < left.len() && tail[0].label == left[li].label {
                            assert(head.label < tail[0].label);
                        }
                        if ri + 1 < right.len() && tail[0].label == right[ri + 1].label {
                            assert(head.label < tail[0].label);
                        }
                    }
                    prepend_preserves_strict_labels(head, tail);
                    assert forall|i: int| #![trigger result[i]]
                        0 <= i < result.len() implies well_formed_local_spec(result[i].body)
                    by {
                        if i == 0 {
                            assert(result[0] == head);
                            assert(well_formed_local_spec(head.body));
                        } else {
                            assert(0 <= i - 1 < tail.len());
                            assert(result[i] == tail[i - 1]);
                            assert(well_formed_local_spec(tail[i - 1].body));
                        }
                    }
                } else {
                    assert(left[li].label == right[ri].label);
                    assert(well_formed_local_choice_prefix_spec(left_parent, left.len() as int));
                    assert(well_formed_local_choice_prefix_spec(right_parent, right.len() as int));
                    well_formed_prefix_implies_branch_wf(left_parent, left.len() as int, li);
                    well_formed_prefix_implies_branch_wf(right_parent, right.len() as int, ri);
                    assert(well_formed_local_spec(left[li].body));
                    assert(well_formed_local_spec(right[ri].body));
                    merge_preserves_well_formed_local(decider, left[li].body, right[ri].body);
                    merge_offer_branches_well_formed(decider, left_parent, right_parent, li + 1, ri + 1);
                    let body = merge_local_spec(decider, left[li].body, right[ri].body).unwrap();
                    let head = LBranch { label: left[li].label, body };
                    let tail = merge_offer_branches_from_spec(decider, left_parent, right_parent, li + 1, ri + 1).unwrap();
                    let result = merge_offer_branches_from_spec(decider, left_parent, right_parent, li, ri).unwrap();
                    assert(result == seq![head] + tail);
                    assert(result.len() > 0);
                    assert(result[0].label == left[li].label);
                    if tail.len() > 0 {
                        assert((li + 1 < left.len() && tail[0].label == left[li + 1].label)
                            || (ri + 1 < right.len() && tail[0].label == right[ri + 1].label));
                        if li + 1 < left.len() && tail[0].label == left[li + 1].label {
                            assert(head.label < tail[0].label);
                        }
                        if ri + 1 < right.len() && tail[0].label == right[ri + 1].label {
                            assert(right[ri].label < right[ri + 1].label);
                            assert(head.label < tail[0].label);
                        }
                    }
                    prepend_preserves_strict_labels(head, tail);
                    assert forall|i: int| #![trigger result[i]]
                        0 <= i < result.len() implies well_formed_local_spec(result[i].body)
                    by {
                        if i == 0 {
                            assert(result[0] == head);
                            assert(well_formed_local_spec(head.body));
                        } else {
                            assert(0 <= i - 1 < tail.len());
                            assert(result[i] == tail[i - 1]);
                            assert(well_formed_local_spec(tail[i - 1].body));
                        }
                    }
                }
            }
            _ => {}
        },
        _ => {}
    }
}

proof fn two_branch_offer_well_formed(decider: RoleId, first: LBranch, second: LBranch)
    requires
        first.label < second.label,
        well_formed_local_spec(first.body),
        well_formed_local_spec(second.body),
    ensures
        well_formed_local_spec(LChoreo::Offer { from: decider, branches: seq![first, second] })
{
    let offer = LChoreo::Offer { from: decider, branches: seq![first, second] };
    let branches = seq![first, second];
    assert(branches.len() == 2);
    assert(branch_labels_strict_spec(branches));
    assert(well_formed_local_choice_prefix_spec(offer, 0));
    assert(well_formed_local_choice_prefix_spec(offer, 1)
        == (well_formed_local_choice_prefix_spec(offer, 0) && well_formed_local_spec(branches[0].body)));
    assert(well_formed_local_choice_prefix_spec(offer, 1));
    assert(well_formed_local_choice_prefix_spec(offer, 2)
        == (well_formed_local_choice_prefix_spec(offer, 1) && well_formed_local_spec(branches[1].body)));
    assert(well_formed_local_choice_prefix_spec(offer, 2));
}

// r[verify choreography.merge]
proof fn merge_preserves_well_formed_local(decider: RoleId, left: LChoreo, right: LChoreo)
    requires
        well_formed_local_spec(left),
        well_formed_local_spec(right),
        merge_local_spec(decider, left, right).is_some(),
    ensures well_formed_local_spec(merge_local_spec(decider, left, right).unwrap())
    decreases left, 1int, right, 1int, 0nat
{
    match (left, right) {
        (LChoreo::End, LChoreo::End) => {}
        (
            LChoreo::Send { peer: l_peer, label: l_label, payload_tag: l_payload, next: l_next },
            LChoreo::Send { peer: r_peer, label: r_label, payload_tag: r_payload, next: r_next },
        ) => {
            if l_peer == r_peer && l_label == r_label && l_payload == r_payload {
                merge_preserves_well_formed_local(decider, *l_next, *r_next);
                assert(merge_local_spec(decider, left, right)
                    == match merge_local_spec(decider, *l_next, *r_next) {
                        Some(next) => Some(LChoreo::Send {
                            peer: l_peer,
                            label: l_label,
                            payload_tag: l_payload,
                            next: Box::new(next),
                        }),
                        None => None,
                    });
            }
        }
        (
            LChoreo::Recv { peer: l_peer, label: l_label, payload_tag: l_payload, next: l_next },
            LChoreo::Recv { peer: r_peer, label: r_label, payload_tag: r_payload, next: r_next },
        ) => {
            if l_peer == r_peer && l_label == r_label && l_payload == r_payload {
                merge_preserves_well_formed_local(decider, *l_next, *r_next);
                merge_identical_prefix_exact(decider, l_peer, l_label, l_payload, *l_next, *r_next);
            } else if l_peer == decider && r_peer == decider && l_label != r_label {
                let left_branch = LBranch {
                    label: l_label,
                    body: LChoreo::Recv {
                        peer: l_peer,
                        label: l_label,
                        payload_tag: l_payload,
                        next: l_next,
                    },
                };
                let right_branch = LBranch {
                    label: r_label,
                    body: LChoreo::Recv {
                        peer: r_peer,
                        label: r_label,
                        payload_tag: r_payload,
                        next: r_next,
                    },
                };
                if l_label < r_label {
                    merge_recv_distinct_labels_into_offer(decider, l_label, r_label, l_payload, r_payload, *l_next, *r_next);
                    two_branch_offer_well_formed(decider, left_branch, right_branch);
                } else {
                    merge_recv_distinct_labels_into_offer(decider, l_label, r_label, l_payload, r_payload, *l_next, *r_next);
                    two_branch_offer_well_formed(decider, right_branch, left_branch);
                }
            }
        }
        (LChoreo::InternalChoice { branches: left_branches }, LChoreo::InternalChoice { branches: right_branches }) => {
            merge_choice_branches_well_formed(
                decider,
                LChoreo::InternalChoice { branches: left_branches },
                LChoreo::InternalChoice { branches: right_branches },
                0,
            );
            let merged = merge_choice_branches_from_spec(
                decider,
                LChoreo::InternalChoice { branches: left_branches },
                LChoreo::InternalChoice { branches: right_branches },
                0,
            ).unwrap();
            assert(merged.len() > 0);
            internal_choice_prefix_from_all_wf(merged, merged.len() as int);
            assert(well_formed_local_spec(LChoreo::InternalChoice { branches: merged }));
        }
        (
            LChoreo::Offer { from: left_from, branches: left_branches },
            LChoreo::Offer { from: right_from, branches: right_branches },
        ) => {
            if left_from == decider && right_from == decider {
                merge_offer_branches_well_formed(
                    decider,
                    LChoreo::Offer { from: left_from, branches: left_branches },
                    LChoreo::Offer { from: right_from, branches: right_branches },
                    0,
                    0,
                );
                let merged = merge_offer_branches_from_spec(
                    decider,
                    LChoreo::Offer { from: left_from, branches: left_branches },
                    LChoreo::Offer { from: right_from, branches: right_branches },
                    0,
                    0,
                ).unwrap();
                assert(merged.len() > 0);
                offer_prefix_from_all_wf(decider, merged, merged.len() as int);
                assert(well_formed_local_spec(LChoreo::Offer { from: decider, branches: merged }));
            }
        }
        _ => {}
    }
}

#[verifier::exec_allows_no_decreases_clause]
fn clone_local_branch(branch: &LocalBranch) -> (result: LocalBranch)
{
    LocalBranch { label: branch.label, body: clone_local(&branch.body) }
}

#[verifier::exec_allows_no_decreases_clause]
pub(crate) fn clone_local(local: &LocalChoreo) -> (result: LocalChoreo)
{
    match local {
        LocalChoreo::End => LocalChoreo::End,
        LocalChoreo::Send { peer, label, payload_tag, next } => LocalChoreo::Send {
            peer: *peer,
            label: *label,
            payload_tag: *payload_tag,
            next: Box::new(clone_local(next)),
        },
        LocalChoreo::Recv { peer, label, payload_tag, next } => LocalChoreo::Recv {
            peer: *peer,
            label: *label,
            payload_tag: *payload_tag,
            next: Box::new(clone_local(next)),
        },
        LocalChoreo::InternalChoice { branches } => {
            let mut out: Vec<LocalBranch> = Vec::new();
            let mut i: usize = 0;
            while i < branches.len()
                decreases branches@.len() - i,
            {
                out.push(clone_local_branch(&branches[i]));
                i += 1;
            }
            LocalChoreo::InternalChoice { branches: out }
        }
        LocalChoreo::Offer { from, branches } => {
            let mut out: Vec<LocalBranch> = Vec::new();
            let mut i: usize = 0;
            while i < branches.len()
                decreases branches@.len() - i,
            {
                out.push(clone_local_branch(&branches[i]));
                i += 1;
            }
            LocalChoreo::Offer { from: *from, branches: out }
        }
    }
}

fn clone_branches_from(branches: &[LocalBranch], from: usize) -> (result: Vec<LocalBranch>)
    requires from <= branches@.len()
{
    let mut result: Vec<LocalBranch> = Vec::new();
    let mut i: usize = from;
    while i < branches.len()
        decreases branches@.len() - i,
    {
        result.push(clone_local_branch(&branches[i]));
        i += 1;
    }
    result
}

fn prepend_branch(branch: LocalBranch, tail: &[LocalBranch]) -> (result: Vec<LocalBranch>)
{
    let mut result: Vec<LocalBranch> = Vec::new();
    result.push(branch);
    let mut i: usize = 0;
    while i < tail.len()
        decreases tail@.len() - i,
    {
        result.push(clone_local_branch(&tail[i]));
        i += 1;
    }
    result
}

#[verifier::exec_allows_no_decreases_clause]
fn merge_choice_branches_from(
    decider: RoleId,
    left: &[LocalBranch],
    right: &[LocalBranch],
    from: usize,
) -> (result: Option<Vec<LocalBranch>>)
    requires from <= left@.len(), from <= right@.len()
{
    if from == left.len() && from == right.len() {
        return Some(Vec::new());
    }
    if from < left.len() && from < right.len() && left[from].label == right[from].label {
        if let Some(body) = merge_local(decider, &left[from].body, &right[from].body) {
            if let Some(tail) = merge_choice_branches_from(decider, left, right, from + 1) {
                let branch = LocalBranch { label: left[from].label, body };
                return Some(prepend_branch(branch, &tail));
            }
        }
    }
    None
}

#[verifier::exec_allows_no_decreases_clause]
fn merge_offer_branches_from(
    decider: RoleId,
    left: &[LocalBranch],
    right: &[LocalBranch],
    li: usize,
    ri: usize,
) -> (result: Option<Vec<LocalBranch>>)
    requires li <= left@.len(), ri <= right@.len()
{
    if li == left.len() && ri == right.len() {
        return Some(Vec::new());
    }
    if li == left.len() {
        return Some(clone_branches_from(right, ri));
    }
    if ri == right.len() {
        return Some(clone_branches_from(left, li));
    }
    if left[li].label < right[ri].label {
        if let Some(tail) = merge_offer_branches_from(decider, left, right, li + 1, ri) {
            return Some(prepend_branch(clone_local_branch(&left[li]), &tail));
        }
    } else if right[ri].label < left[li].label {
        if let Some(tail) = merge_offer_branches_from(decider, left, right, li, ri + 1) {
            return Some(prepend_branch(clone_local_branch(&right[ri]), &tail));
        }
    } else if let Some(body) = merge_local(decider, &left[li].body, &right[ri].body) {
        if let Some(tail) = merge_offer_branches_from(decider, left, right, li + 1, ri + 1) {
            let branch = LocalBranch { label: left[li].label, body };
            return Some(prepend_branch(branch, &tail));
        }
    }
    None
}

#[verifier::exec_allows_no_decreases_clause]
fn well_formed_local_branches(branches: &[LocalBranch]) -> (result: bool)
    ensures
        result == (branches@.len() > 0
            && local_exec_branch_labels_strict_spec(branches@)
            && forall|j: int| #![trigger branches@[j]]
                0 <= j < branches@.len() ==> well_formed_local_result_spec(branches@[j].body))
{
    if branches.is_empty() {
        return false;
    }
    let mut i: usize = 0;
    while i < branches.len()
        invariant
            0 < branches@.len(),
            0 <= i <= branches@.len(),
            forall|j: int| #![trigger branches@[j]]
                0 < j < i as int ==> branches@[j - 1].label < branches@[j].label,
            forall|j: int| #![trigger branches@[j]]
                0 <= j < i as int ==> well_formed_local_result_spec(branches@[j].body),
        decreases branches@.len() - i,
    {
        if i > 0 && branches[i - 1].label >= branches[i].label {
            proof {
                local_adjacent_violation_exec_not_strict(branches@, i as int);
                assert(!(branches@.len() > 0
                    && local_exec_branch_labels_strict_spec(branches@)
                    && forall|j: int| #![trigger branches@[j]]
                        0 <= j < branches@.len() ==> well_formed_local_result_spec(branches@[j].body)));
            }
            return false;
        }
        if !well_formed_local(&branches[i].body) {
            proof {
                assert(!well_formed_local_result_spec(branches@[i as int].body));
                assert(!(forall|j: int| #![trigger branches@[j]]
                    0 <= j < branches@.len() ==> well_formed_local_result_spec(branches@[j].body))) by {
                    assert((i as int) < branches@.len());
                    assert(!well_formed_local_result_spec(branches@[i as int].body));
                }
                assert(!(branches@.len() > 0
                    && local_exec_branch_labels_strict_spec(branches@)
                    && forall|j: int| #![trigger branches@[j]]
                        0 <= j < branches@.len() ==> well_formed_local_result_spec(branches@[j].body)));
            }
            return false;
        }
        proof {
            assert(well_formed_local_result_spec(branches@[i as int].body));
            if i > 0 {
                assert(branches@[i as int - 1].label < branches@[i as int].label);
            }
        }
        i += 1;
    }
    proof {
        local_adjacent_increasing_prefix_implies_pairwise_prefix(branches@, branches@.len() as int);
        assert(local_exec_branch_labels_strict_spec(branches@));
        assert(forall|j: int| #![trigger branches@[j]]
            0 <= j < branches@.len() ==> well_formed_local_result_spec(branches@[j].body));
    }
    true
}

// r[impl choreography.merge]
#[verifier::exec_allows_no_decreases_clause]
pub fn well_formed_local(local: &LocalChoreo) -> (result: bool)
    ensures result == well_formed_local_result_spec(*local)
{
    match local {
        LocalChoreo::End => {
            proof { reveal(well_formed_local_result_spec); }
            true
        }
        LocalChoreo::Send { next, .. } | LocalChoreo::Recv { next, .. } => {
            let result = well_formed_local(next);
            proof { reveal(well_formed_local_result_spec); }
            result
        }
        LocalChoreo::InternalChoice { branches } | LocalChoreo::Offer { branches, .. } => {
            let result = well_formed_local_branches(branches);
            proof { reveal(well_formed_local_result_spec); }
            result
        }
    }
}

// r[impl choreography.merge]
#[verifier::exec_allows_no_decreases_clause]
pub fn merge_local(decider: RoleId, left: &LocalChoreo, right: &LocalChoreo) -> (result: Option<LocalChoreo>)
    ensures merge_local_post_spec(decider, *left, *right, result)
{
    match (left, right) {
        (LocalChoreo::End, LocalChoreo::End) => Some(LocalChoreo::End),
        (
            LocalChoreo::Send { peer: left_peer, label: left_label, payload_tag: left_payload, next: left_next },
            LocalChoreo::Send { peer: right_peer, label: right_label, payload_tag: right_payload, next: right_next },
        ) => {
            if left_peer == right_peer && left_label == right_label && left_payload == right_payload {
                match merge_local(decider, left_next, right_next) {
                    Some(next) => Some(LocalChoreo::Send {
                        peer: *left_peer,
                        label: *left_label,
                        payload_tag: *left_payload,
                        next: Box::new(next),
                    }),
                    None => None,
                }
            } else {
                None
            }
        }
        (
            LocalChoreo::Recv { peer: left_peer, label: left_label, payload_tag: left_payload, next: left_next },
            LocalChoreo::Recv { peer: right_peer, label: right_label, payload_tag: right_payload, next: right_next },
        ) => {
            if left_peer == right_peer && left_label == right_label && left_payload == right_payload {
                match merge_local(decider, left_next, right_next) {
                    Some(next) => Some(LocalChoreo::Recv {
                        peer: *left_peer,
                        label: *left_label,
                        payload_tag: *left_payload,
                        next: Box::new(next),
                    }),
                    None => None,
                }
            } else if *left_peer == decider && *right_peer == decider && left_label != right_label {
                let left_branch = LocalBranch {
                    label: *left_label,
                    body: LocalChoreo::Recv {
                        peer: *left_peer,
                        label: *left_label,
                        payload_tag: *left_payload,
                        next: Box::new(clone_local(left_next)),
                    },
                };
                let right_branch = LocalBranch {
                    label: *right_label,
                    body: LocalChoreo::Recv {
                        peer: *right_peer,
                        label: *right_label,
                        payload_tag: *right_payload,
                        next: Box::new(clone_local(right_next)),
                    },
                };
                if left_label < right_label {
                    Some(LocalChoreo::Offer { from: decider, branches: vec![left_branch, right_branch] })
                } else {
                    Some(LocalChoreo::Offer { from: decider, branches: vec![right_branch, left_branch] })
                }
            } else {
                None
            }
        }
        (LocalChoreo::InternalChoice { branches: left_branches }, LocalChoreo::InternalChoice { branches: right_branches }) => {
            match merge_choice_branches_from(decider, left_branches, right_branches, 0) {
                Some(branches) => Some(LocalChoreo::InternalChoice { branches }),
                None => None,
            }
        }
        (
            LocalChoreo::Offer { from: left_from, branches: left_branches },
            LocalChoreo::Offer { from: right_from, branches: right_branches },
        ) => {
            if *left_from == decider && *right_from == decider {
                match merge_offer_branches_from(decider, left_branches, right_branches, 0, 0) {
                    Some(branches) => Some(LocalChoreo::Offer { from: decider, branches }),
                    None => None,
                }
            } else {
                None
            }
        }
        _ => None,
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn recv(peer: RoleId, label: LabelId, payload_tag: PayloadTag, next: LocalChoreo) -> LocalChoreo {
        LocalChoreo::Recv {
            peer,
            label,
            payload_tag,
            next: Box::new(next),
        }
    }

    fn send(peer: RoleId, label: LabelId, payload_tag: PayloadTag, next: LocalChoreo) -> LocalChoreo {
        LocalChoreo::Send {
            peer,
            label,
            payload_tag,
            next: Box::new(next),
        }
    }

    // r[verify choreography.merge]
    #[test]
    fn test_identical_prefix_merge() {
        let left = recv(3, 7, 1, LocalChoreo::End);
        let right = recv(3, 7, 1, LocalChoreo::End);
        let merged = merge_local(9, &left, &right);
        assert_eq!(merged, Some(recv(3, 7, 1, LocalChoreo::End)));
    }

    // r[verify choreography.merge]
    #[test]
    fn test_decider_recvs_form_offer() {
        let left = recv(5, 1, 9, LocalChoreo::End);
        let right = recv(5, 4, 11, LocalChoreo::End);
        let merged = merge_local(5, &left, &right).unwrap();
        match merged {
            LocalChoreo::Offer { from, branches } => {
                assert_eq!(from, 5);
                assert_eq!(branches.len(), 2);
                assert_eq!(branches[0].label, 1);
                assert_eq!(branches[1].label, 4);
            }
            _ => panic!("expected offer"),
        }
    }

    // r[verify choreography.merge]
    #[test]
    fn test_offer_union_preserves_shared_and_unshared_labels() {
        let left = LocalChoreo::Offer {
            from: 2,
            branches: vec![
                LocalBranch {
                    label: 1,
                    body: LocalChoreo::End,
                },
                LocalBranch {
                    label: 3,
                    body: send(8, 9, 0, LocalChoreo::End),
                },
            ],
        };
        let right = LocalChoreo::Offer {
            from: 2,
            branches: vec![
                LocalBranch {
                    label: 2,
                    body: LocalChoreo::End,
                },
                LocalBranch {
                    label: 3,
                    body: send(8, 9, 0, LocalChoreo::End),
                },
            ],
        };
        let merged = merge_local(2, &left, &right).unwrap();
        match merged {
            LocalChoreo::Offer { branches, .. } => {
                assert_eq!(branches.iter().map(|b| b.label).collect::<Vec<_>>(), vec![1, 2, 3]);
            }
            _ => panic!("expected offer"),
        }
    }

    // r[verify choreography.merge]
    #[test]
    fn test_conflicting_heads_fail_merge() {
        let left = send(1, 2, 3, LocalChoreo::End);
        let right = recv(1, 2, 3, LocalChoreo::End);
        assert!(merge_local(1, &left, &right).is_none());
    }

    proptest! {
        // r[verify choreography.merge]
        #[test]
        fn prop_offer_labels_stay_sorted(l1 in 0u32..8, l2 in 0u32..8) {
            prop_assume!(l1 != l2);
            let left = recv(6, l1, 0, LocalChoreo::End);
            let right = recv(6, l2, 1, LocalChoreo::End);
            let merged = merge_local(6, &left, &right).unwrap();
            match merged {
                LocalChoreo::Offer { branches, .. } => {
                    prop_assert!(branches[0].label < branches[1].label);
                }
                _ => prop_assert!(false),
            }
        }
    }
}
