//! Finite global choreography primitives.
//!
//! Defines the runtime AST plus proof-friendly spec views for participants,
//! communication-prefix shape, and global well-formedness.

#![allow(dead_code)]

use vstd::prelude::*;

verus! {

pub type RoleId = u32;
pub type LabelId = u32;
pub type PayloadTag = u32;

#[cfg_attr(not(verus_keep_ghost), derive(Clone))]
#[derive(Debug, PartialEq, Eq)]
pub struct GlobalBranch {
    pub label: LabelId,
    pub body: GlobalChoreo,
}

#[cfg_attr(not(verus_keep_ghost), derive(Clone))]
#[derive(Debug, PartialEq, Eq)]
pub enum GlobalChoreo {
    End,
    Comm {
        from: RoleId,
        to: RoleId,
        label: LabelId,
        payload_tag: PayloadTag,
        next: Box<GlobalChoreo>,
    },
    Choice {
        decider: RoleId,
        branches: Vec<GlobalBranch>,
    },
}

pub(crate) struct GBranch {
    pub label: LabelId,
    pub body: GChoreo,
}

pub(crate) enum GChoreo {
    End,
    Comm {
        from: RoleId,
        to: RoleId,
        label: LabelId,
        payload_tag: PayloadTag,
        next: Box<GChoreo>,
    },
    Choice {
        decider: RoleId,
        branches: Seq<GBranch>,
    },
}

pub(crate) open spec fn gbranch_suffix_size(branches: Seq<GBranch>, from: int) -> nat
    decreases if from < branches.len() { branches.len() - from } else { 0 }
{
    if from < 0 || from >= branches.len() {
        0
    } else {
        1 + global_size(branches[from].body) + gbranch_suffix_size(branches, from + 1)
    }
}

pub(crate) open spec fn global_choice_branch_count(global: GlobalChoreo) -> int {
    match global {
        GlobalChoreo::Choice { branches, .. } => branches@.len() as int,
        _ => 0,
    }
}

pub(crate) open spec fn global_choice_branches_view(global: GlobalChoreo, from: int) -> Seq<GBranch>
    decreases global, 0int, if from < global_choice_branch_count(global) { global_choice_branch_count(global) - from } else { 0int }
{
    match global {
        GlobalChoreo::Choice { branches, .. } => {
            if from < 0 || from >= branches@.len() {
                seq![]
            } else {
                seq![GBranch {
                    label: branches[from].label,
                    body: global_view(branches[from].body),
                }] + global_choice_branches_view(global, from + 1)
            }
        }
        _ => seq![],
    }
}

pub(crate) open spec fn gbranches_view(branches: Seq<GlobalBranch>) -> Seq<GBranch>
    decreases branches.len()
{
    if branches.len() == 0 {
        seq![]
    } else {
        seq![GBranch {
            label: branches[0].label,
            body: global_view(branches[0].body),
        }] + gbranches_view(branches.subrange(1, branches.len() as int))
    }
}

pub(crate) open spec fn global_view(global: GlobalChoreo) -> GChoreo
    decreases global, 1int, 0int
{
    match global {
        GlobalChoreo::End => GChoreo::End,
        GlobalChoreo::Comm { from, to, label, payload_tag, next } => GChoreo::Comm {
            from,
            to,
            label,
            payload_tag,
            next: Box::new(global_view(*next)),
        },
        GlobalChoreo::Choice { decider, .. } => GChoreo::Choice {
            decider,
            branches: global_choice_branches_view(global, 0),
        },
    }
}

pub(crate) open spec fn gchoreo_branch_count(global: GChoreo) -> int {
    match global {
        GChoreo::Choice { branches, .. } => branches.len() as int,
        _ => 0,
    }
}

pub(crate) open spec fn global_choice_branch_size(global: GChoreo, from: int) -> nat
    decreases global, 0int, if from < gchoreo_branch_count(global) { gchoreo_branch_count(global) - from } else { 0int }
{
    match global {
        GChoreo::Choice { branches, .. } => {
            if from < 0 || from >= branches.len() {
                0
            } else {
                1 + global_size(branches[from].body) + global_choice_branch_size(global, from + 1)
            }
        }
        _ => 0,
    }
}

pub(crate) open spec fn global_size(global: GChoreo) -> nat
    decreases global, 1int, 0int
{
    match global {
        GChoreo::End => 1,
        GChoreo::Comm { next, .. } => 1 + global_size(*next),
        GChoreo::Choice { .. } => 1 + global_choice_branch_size(global, 0),
    }
}

pub(crate) open spec fn role_in_list_spec(roles: Seq<RoleId>, role: RoleId) -> bool {
    exists|i: int| #![trigger roles[i]] 0 <= i < roles.len() && roles[i] == role
}

pub(crate) open spec fn unique_roles_spec(roles: Seq<RoleId>) -> bool {
    forall|i: int, j: int| #![trigger roles[i], roles[j]]
        0 <= i < j < roles.len() ==> roles[i] != roles[j]
}

pub(crate) open spec fn insert_role_spec(roles: Seq<RoleId>, role: RoleId) -> Seq<RoleId> {
    if role_in_list_spec(roles, role) {
        roles
    } else {
        roles.push(role)
    }
}

pub(crate) open spec fn union_roles_prefix_spec(left: Seq<RoleId>, right: Seq<RoleId>, upto: int) -> Seq<RoleId>
    decreases if upto > 0 { upto } else { 0 }
{
    if upto <= 0 {
        left
    } else {
        insert_role_spec(union_roles_prefix_spec(left, right, upto - 1), right[upto - 1])
    }
}

pub(crate) open spec fn union_roles_spec(left: Seq<RoleId>, right: Seq<RoleId>) -> Seq<RoleId> {
    union_roles_prefix_spec(left, right, right.len() as int)
}

pub(crate) open spec fn role_in_choice_branches_prefix_spec(role: RoleId, global: GChoreo, upto: int) -> bool
    decreases global, 0int, if upto > 0 { upto } else { 0int }
{
    match global {
        GChoreo::Choice { branches, .. } => {
            if upto <= 0 || upto > branches.len() {
                false
            } else {
                role_in_choice_branches_prefix_spec(role, global, upto - 1)
                    || role_in_global_spec(role, branches[upto - 1].body)
            }
        }
        _ => false,
    }
}

pub(crate) open spec fn role_in_global_spec(role: RoleId, global: GChoreo) -> bool
    decreases global, 1int, 0int
{
    match global {
        GChoreo::End => false,
        GChoreo::Comm { from, to, next, .. } => role == from || role == to || role_in_global_spec(role, *next),
        GChoreo::Choice { decider, branches } => {
            role == decider || role_in_choice_branches_prefix_spec(role, global, branches.len() as int)
        }
    }
}

pub(crate) open spec fn role_in_branches_spec(role: RoleId, branches: Seq<GBranch>) -> bool
    decreases branches.len()
{
    if branches.len() == 0 {
        false
    } else {
        role_in_global_spec(role, branches[0].body)
            || role_in_branches_spec(role, branches.subrange(1, branches.len() as int))
    }
}

pub(crate) open spec fn participants_choice_branches_prefix_spec(global: GChoreo, upto: int) -> Seq<RoleId>
    decreases global, 0int, if upto > 0 { upto } else { 0int }
{
    match global {
        GChoreo::Choice { branches, .. } => {
            if upto <= 0 || upto > branches.len() {
                seq![]
            } else {
                union_roles_spec(
                    participants_choice_branches_prefix_spec(global, upto - 1),
                    participants_spec(branches[upto - 1].body),
                )
            }
        }
        _ => seq![],
    }
}

pub(crate) open spec fn participants_branches_prefix_spec(branches: Seq<GBranch>, upto: int) -> Seq<RoleId>
    decreases if upto > 0 { upto } else { 0 }
{
    if upto <= 0 {
        seq![]
    } else {
        union_roles_spec(
            participants_branches_prefix_spec(branches, upto - 1),
            participants_spec(branches[upto - 1].body),
        )
    }
}

pub(crate) open spec fn participants_branches_spec(branches: Seq<GBranch>) -> Seq<RoleId>
    decreases branches.len()
{
    participants_branches_prefix_spec(branches, branches.len() as int)
}

// r[depends choreography.participants]
// r[impl choreography.participants]
pub(crate) open spec fn participants_spec(global: GChoreo) -> Seq<RoleId>
    decreases global, 1int, 0int
{
    match global {
        GChoreo::End => seq![],
        GChoreo::Comm { from, to, next, .. } => {
            insert_role_spec(insert_role_spec(participants_spec(*next), from), to)
        }
        GChoreo::Choice { decider, branches } => {
            insert_role_spec(participants_choice_branches_prefix_spec(global, branches.len() as int), decider)
        }
    }
}

pub(crate) open spec fn global_branch_labels_strict_spec(branches: Seq<GBranch>) -> bool {
    forall|i: int, j: int| #![trigger branches[i], branches[j]]
        0 <= i < j < branches.len() ==> branches[i].label < branches[j].label
}

pub(crate) open spec fn well_formed_choice_prefix_spec(global: GChoreo, upto: int) -> bool
    decreases global, 0int, if upto > 0 { upto } else { 0int }
{
    match global {
        GChoreo::Choice { branches, .. } => {
            if upto <= 0 || upto > branches.len() {
                upto <= 0
            } else {
                well_formed_choice_prefix_spec(global, upto - 1)
                    && well_formed_global_spec(branches[upto - 1].body)
            }
        }
        _ => false,
    }
}

pub(crate) open spec fn well_formed_branches_spec(branches: Seq<GBranch>) -> bool {
    branches.len() > 0
        && global_branch_labels_strict_spec(branches)
        && forall|i: int| #![trigger branches[i]] 0 <= i < branches.len() ==> well_formed_global_spec(branches[i].body)
}

// r[depends choreography.communication]
// r[impl choreography.communication]
pub(crate) open spec fn comm_parts_spec(global: GChoreo) -> Option<(RoleId, RoleId, LabelId, PayloadTag, GChoreo)>
    decreases global
{
    match global {
        GChoreo::Comm { from, to, label, payload_tag, next } => Some((from, to, label, payload_tag, *next)),
        _ => None,
    }
}

// r[depends choreography.participants]
// r[impl choreography.participants]
pub(crate) open spec fn well_formed_global_spec(global: GChoreo) -> bool
    decreases global
{
    match global {
        GChoreo::End => true,
        GChoreo::Comm { from, to, next, .. } => from != to && well_formed_global_spec(*next),
        GChoreo::Choice { branches, .. } => {
            branches.len() > 0
                && global_branch_labels_strict_spec(branches)
                && well_formed_choice_prefix_spec(global, branches.len() as int)
        }
    }
}

pub(crate) open spec fn participants_branches_exec_prefix_spec(branches: Seq<GlobalBranch>, upto: int) -> Seq<RoleId>
    decreases if upto > 0 { upto } else { 0int }
{
    if upto <= 0 || upto > branches.len() {
        seq![]
    } else {
        union_roles_spec(
            participants_branches_exec_prefix_spec(branches, upto - 1),
            participants_spec(global_view(branches[upto - 1].body)),
        )
    }
}

pub(crate) open spec fn participants_choice_exec_spec(branches: Seq<GlobalBranch>, decider: RoleId) -> Seq<RoleId> {
    insert_role_spec(participants_branches_exec_prefix_spec(branches, branches.len() as int), decider)
}

// r[impl choreography.participants]
pub closed spec fn participants_exec_spec(global: GlobalChoreo) -> Seq<RoleId> {
    participants_spec(global_view(global))
}

// r[impl choreography.communication]
pub closed spec fn comm_parts_exec_spec(global: GlobalChoreo) -> Option<(RoleId, RoleId, LabelId, PayloadTag, GlobalChoreo)> {
    match global {
        GlobalChoreo::Comm { from, to, label, payload_tag, next } => Some((from, to, label, payload_tag, *next)),
        _ => None,
    }
}

// r[impl choreography.communication]
pub closed spec fn comm_parts_post_spec(global: GlobalChoreo, result: Option<(RoleId, RoleId, LabelId, PayloadTag, GlobalChoreo)>) -> bool {
    match global {
        GlobalChoreo::Comm { from, to, label, payload_tag, next } => {
            match result {
                Some((r_from, r_to, r_label, r_payload, r_next)) => {
                    r_from == from
                        && r_to == to
                        && r_label == label
                        && r_payload == payload_tag
                        && global_view(r_next) == global_view(*next)
                }
                None => false,
            }
        }
        _ => result.is_none(),
    }
}

// r[impl choreography.participants]
pub closed spec fn well_formed_global_exec_spec(global: GlobalChoreo) -> bool {
    well_formed_global_spec(global_view(global))
}

pub open spec fn global_exec_branch_labels_strict_spec(branches: Seq<GlobalBranch>) -> bool {
    forall|i: int, j: int| #![trigger branches[i], branches[j]]
        0 <= i < j < branches.len() ==> branches[i].label < branches[j].label
}

pub open spec fn well_formed_global_runtime_spec(global: GlobalChoreo) -> bool
    decreases global
{
    match global {
        GlobalChoreo::End => true,
        GlobalChoreo::Comm { from, to, next, .. } => from != to && well_formed_global_runtime_spec(*next),
        GlobalChoreo::Choice { branches, .. } =>
            branches.len() > 0
                && global_exec_branch_labels_strict_spec(branches@)
                && forall|i: int| #![trigger branches@[i]]
                    0 <= i < branches@.len() ==> well_formed_global_runtime_spec(branches@[i].body),
    }
}

proof fn adjacent_violation_exec_not_strict(branches: Seq<GlobalBranch>, bad: int)
    requires
        0 < bad < branches.len(),
        branches[bad - 1].label >= branches[bad].label,
    ensures
        !global_exec_branch_labels_strict_spec(branches)
{
    assert(!global_exec_branch_labels_strict_spec(branches)) by {
        assert(0 <= bad - 1);
        assert(bad - 1 < bad);
        assert(bad < branches.len());
        assert(branches[bad - 1].label >= branches[bad].label);
    }
}

proof fn adjacent_increasing_prefix_implies_pairwise_prefix(branches: Seq<GlobalBranch>, n: int)
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
        adjacent_increasing_prefix_implies_pairwise_prefix(branches, n - 1);
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

proof fn choice_adjacent_violation_not_strict(choice: GlobalChoreo, bad: int)
    requires
        match choice {
            GlobalChoreo::Choice { branches, .. } =>
                0 < bad < branches@.len() && branches@[bad - 1].label >= branches@[bad].label,
            _ => false,
        }
    ensures
        match global_view(choice) {
            GChoreo::Choice { branches, .. } => !global_branch_labels_strict_spec(branches),
            _ => false,
        }
{
    if let GlobalChoreo::Choice { branches, .. } = choice {
        let viewed = global_choice_branches_view(choice, 0);
        global_choice_branches_view_len(choice, 0);
        global_choice_branches_view_nth(choice, 0, bad - 1);
        global_choice_branches_view_nth(choice, 0, bad);
        assert(!global_branch_labels_strict_spec(viewed)) by {
            assert(0 <= bad - 1);
            assert(bad - 1 < bad);
            assert(bad < viewed.len());
            assert(viewed[bad - 1].label == branches@[bad - 1].label);
            assert(viewed[bad].label == branches@[bad].label);
            assert(viewed[bad - 1].label >= viewed[bad].label);
        }
    }
}

proof fn choice_adjacent_exec_labels_imply_strict(choice: GlobalChoreo)
    requires
        match choice {
            GlobalChoreo::Choice { branches, .. } =>
                forall|j: int| #![trigger branches@[j]]
                    0 < j < branches@.len() ==> branches@[j - 1].label < branches@[j].label,
            _ => false,
        }
    ensures
        match global_view(choice) {
            GChoreo::Choice { branches, .. } => global_branch_labels_strict_spec(branches),
            _ => false,
        }
{
    if let GlobalChoreo::Choice { branches, .. } = choice {
        let viewed = global_choice_branches_view(choice, 0);
        adjacent_increasing_prefix_implies_pairwise_prefix(branches@, branches@.len() as int);
        global_choice_branches_view_len(choice, 0);
        assert forall|i: int, j: int| #![trigger viewed[i], viewed[j]]
            0 <= i < j < viewed.len() implies viewed[i].label < viewed[j].label by {
            global_choice_branches_view_nth(choice, 0, i);
            global_choice_branches_view_nth(choice, 0, j);
            assert(viewed[i].label == branches@[i].label);
            assert(viewed[j].label == branches@[j].label);
            assert(branches@[i].label < branches@[j].label);
        }
    }
}

proof fn well_formed_choice_prefix_exec_iff(choice: GlobalChoreo, upto: int)
    requires
        match choice {
            GlobalChoreo::Choice { branches, .. } => 0 <= upto <= branches@.len(),
            _ => false,
        }
    ensures
        match choice {
            GlobalChoreo::Choice { branches, .. } =>
                well_formed_choice_prefix_spec(global_view(choice), upto)
                    <==> (forall|j: int| #![trigger branches@[j]]
                        0 <= j < upto ==> well_formed_global_exec_spec(branches@[j].body)),
            _ => false,
        }
    decreases if upto > 0 { upto } else { 0int }
{
    if let GlobalChoreo::Choice { decider, branches } = choice {
        reveal(well_formed_global_exec_spec);
        if upto > 0 {
            well_formed_choice_prefix_exec_iff(choice, upto - 1);
            global_choice_branches_view_nth(choice, 0, upto - 1);
            assert(global_view(choice) == GChoreo::Choice {
                decider,
                branches: global_choice_branches_view(choice, 0),
            });
            assert(well_formed_choice_prefix_spec(global_view(choice), upto)
                <==> (well_formed_choice_prefix_spec(global_view(choice), upto - 1)
                    && well_formed_global_exec_spec(branches@[upto - 1].body)));
            assert((forall|j: int| #![trigger branches@[j]]
                0 <= j < upto ==> well_formed_global_exec_spec(branches@[j].body))
                <==> ((forall|j: int| #![trigger branches@[j]]
                    0 <= j < upto - 1 ==> well_formed_global_exec_spec(branches@[j].body))
                    && well_formed_global_exec_spec(branches@[upto - 1].body))) by {
                if (forall|j: int| #![trigger branches@[j]]
                    0 <= j < upto ==> well_formed_global_exec_spec(branches@[j].body)) {
                    assert(well_formed_global_exec_spec(branches@[upto - 1].body));
                }
                if ((forall|j: int| #![trigger branches@[j]]
                    0 <= j < upto - 1 ==> well_formed_global_exec_spec(branches@[j].body))
                    && well_formed_global_exec_spec(branches@[upto - 1].body)) {
                    assert forall|j: int| #![trigger branches@[j]]
                        0 <= j < upto implies well_formed_global_exec_spec(branches@[j].body) by {
                        if j < upto - 1 {
                        } else {
                            assert(j == upto - 1);
                        }
                    };
                }
            }
        } else {
            assert(well_formed_choice_prefix_spec(global_view(choice), 0));
            assert(forall|j: int| #![trigger branches@[j]]
                0 <= j < 0 ==> well_formed_global_exec_spec(branches@[j].body));
        }
    }
}

proof fn insert_role_membership(roles: Seq<RoleId>, role: RoleId, target: RoleId)
    ensures role_in_list_spec(insert_role_spec(roles, role), target) <==> (target == role || role_in_list_spec(roles, target))
{
    if role_in_list_spec(roles, role) {
        // insert_role_spec returns roles unchanged
        assert(insert_role_spec(roles, role) == roles);
        if target == role {
            // role is already in roles
            let i = choose|i: int| 0 <= i < roles.len() && roles[i] == role;
            assert(roles[i] == target);
        }
    } else {
        // insert_role_spec returns roles.push(role)
        assert(insert_role_spec(roles, role) == roles.push(role));
        if role_in_list_spec(roles.push(role), target) {
            let i = choose|i: int| 0 <= i < roles.push(role).len() && roles.push(role)[i] == target;
            if i < roles.len() {
                assert(roles[i] == target);
            } else {
                assert(target == role);
            }
        }
        if target == role {
            assert(roles.push(role)[roles.len() as int] == role);
        }
        if role_in_list_spec(roles, target) {
            let i = choose|i: int| 0 <= i < roles.len() && roles[i] == target;
            assert(roles.push(role)[i] == target);
        }
    }
}

proof fn union_roles_prefix_membership(left: Seq<RoleId>, right: Seq<RoleId>, upto: int, target: RoleId)
    requires 0 <= upto <= right.len()
    ensures role_in_list_spec(union_roles_prefix_spec(left, right, upto), target)
        <==> (role_in_list_spec(left, target)
            || exists|i: int| #![trigger right[i]] 0 <= i < upto && right[i] == target)
    decreases if upto > 0 { upto } else { 0 }
{
    if upto > 0 {
        union_roles_prefix_membership(left, right, upto - 1, target);
        insert_role_membership(union_roles_prefix_spec(left, right, upto - 1), right[upto - 1], target);
    }
}

proof fn role_in_choice_prefix_membership(role: RoleId, global: GChoreo, upto: int)
    requires
        match global {
            GChoreo::Choice { branches, .. } => 0 <= upto <= branches.len(),
            _ => false,
        },
    ensures
        role_in_choice_branches_prefix_spec(role, global, upto)
            <==> role_in_list_spec(participants_choice_branches_prefix_spec(global, upto), role)
    decreases global, 0int, if upto > 0 { upto } else { 0int }
{
    match global {
        GChoreo::Choice { branches, .. } => {
            if upto > 0 {
                role_in_choice_prefix_membership(role, global, upto - 1);
                participants_membership_exact(role, branches[upto - 1].body);
                union_roles_prefix_membership(
                    participants_choice_branches_prefix_spec(global, upto - 1),
                    participants_spec(branches[upto - 1].body),
                    participants_spec(branches[upto - 1].body).len() as int,
                    role,
                );
            }
        }
        _ => {}
    }
}

// r[verify choreography.participants]
proof fn participants_membership_exact(role: RoleId, global: GChoreo)
    ensures role_in_global_spec(role, global) <==> role_in_list_spec(participants_spec(global), role)
    decreases global
{
    match global {
        GChoreo::End => {}
        GChoreo::Comm { from, to, next, .. } => {
            participants_membership_exact(role, *next);
            insert_role_membership(participants_spec(*next), from, role);
            insert_role_membership(insert_role_spec(participants_spec(*next), from), to, role);
        }
        GChoreo::Choice { decider, branches } => {
            role_in_choice_prefix_membership(role, global, branches.len() as int);
            insert_role_membership(participants_choice_branches_prefix_spec(global, branches.len() as int), decider, role);
        }
    }
}

// r[verify choreography.participants]
proof fn self_send_not_well_formed(label: LabelId, payload_tag: PayloadTag, next: GChoreo, role: RoleId)
    ensures !well_formed_global_spec(GChoreo::Comm {
        from: role,
        to: role,
        label,
        payload_tag,
        next: Box::new(next),
    })
{
}

// r[verify choreography.participants]
proof fn duplicate_adjacent_labels_not_well_formed(decider: RoleId, left: GChoreo, right: GChoreo, label: LabelId)
    ensures !well_formed_global_spec(GChoreo::Choice {
        decider,
        branches: seq![
            GBranch { label, body: left },
            GBranch { label, body: right },
        ],
    })
{
}

// r[verify choreography.communication]
proof fn comm_parts_exact(from: RoleId, to: RoleId, label: LabelId, payload_tag: PayloadTag, next: GChoreo)
    ensures comm_parts_spec(GChoreo::Comm {
        from,
        to,
        label,
        payload_tag,
        next: Box::new(next),
    }) == Some((from, to, label, payload_tag, next))
{
}

pub(crate) fn contains_role(roles: &[RoleId], role: RoleId) -> (result: bool)
    ensures result == role_in_list_spec(roles@, role)
{
    let mut i: usize = 0;
    while i < roles.len()
        invariant
            0 <= i <= roles@.len(),
            forall|j: int| #![trigger roles@[j]] 0 <= j < i as int ==> roles@[j] != role,
        decreases roles@.len() - i,
    {
        if roles[i] == role {
            return true;
        }
        i += 1;
    }
    false
}

fn push_unique(roles: &mut Vec<RoleId>, role: RoleId)
    ensures roles@ == insert_role_spec(old(roles)@, role)
{
    if !contains_role(roles, role) {
        roles.push(role);
    }
}

fn append_unique(roles: &mut Vec<RoleId>, more: &[RoleId])
    ensures roles@ == union_roles_spec(old(roles)@, more@)
{
    let ghost start = old(roles)@;
    let mut i: usize = 0;
    while i < more.len()
        invariant
            0 <= i <= more@.len(),
            roles@ == union_roles_prefix_spec(start, more@, i as int),
        decreases more@.len() - i,
    {
        push_unique(roles, more[i]);
        i += 1;
    }
}

#[verifier::exec_allows_no_decreases_clause]
fn clone_global_branch(branch: &GlobalBranch) -> (result: GlobalBranch)
    ensures
        result.label == branch.label,
        global_view(result.body) == global_view(branch.body)
{
    GlobalBranch { label: branch.label, body: clone_global(&branch.body) }
}

#[verifier::exec_allows_no_decreases_clause]
pub(crate) fn clone_global(global: &GlobalChoreo) -> (result: GlobalChoreo)
    ensures global_view(result) == global_view(*global)
{
    match global {
        GlobalChoreo::End => GlobalChoreo::End,
        GlobalChoreo::Comm { from, to, label, payload_tag, next } => GlobalChoreo::Comm {
            from: *from,
            to: *to,
            label: *label,
            payload_tag: *payload_tag,
            next: Box::new(clone_global(next)),
        },
        GlobalChoreo::Choice { decider, branches } => {
            let mut out: Vec<GlobalBranch> = Vec::new();
            let mut i: usize = 0;
            while i < branches.len()
                invariant
                    0 <= i <= branches@.len(),
                    out@.len() == i as int,
                    forall|j: int| #![trigger out@[j], branches@[j]]
                        0 <= j < i as int ==> out@[j].label == branches@[j].label
                            && global_view(out@[j].body) == global_view(branches@[j].body),
                decreases branches@.len() - i,
            {
                out.push(clone_global_branch(&branches[i]));
                i += 1;
            }
            let result = GlobalChoreo::Choice { decider: *decider, branches: out };
            proof {
                global_choice_branches_view_len(*global, 0);
                global_choice_branches_view_len(result, 0);
                assert forall|k: int|
                    #![trigger global_choice_branches_view(result, 0)[k], global_choice_branches_view(*global, 0)[k]]
                    0 <= k < branches@.len()
                    implies global_choice_branches_view(result, 0)[k] == global_choice_branches_view(*global, 0)[k]
                by {
                    global_choice_branches_view_nth(result, 0, k);
                    global_choice_branches_view_nth(*global, 0, k);
                    assert(out@[k].label == branches@[k].label);
                    assert(global_view(out@[k].body) == global_view(branches@[k].body));
                }
                assert(global_choice_branches_view(result, 0) == global_choice_branches_view(*global, 0));
                assert(global_view(result) == global_view(*global));
            }
            result
        }
    }
}

proof fn global_choice_branches_view_len(choice: GlobalChoreo, from: int)
    requires
        match choice {
            GlobalChoreo::Choice { branches, .. } => 0 <= from <= branches@.len(),
            _ => false,
        },
    ensures
        match choice {
            GlobalChoreo::Choice { branches, .. } =>
                global_choice_branches_view(choice, from).len() == branches@.len() - from,
            _ => false,
        }
    decreases match choice {
        GlobalChoreo::Choice { branches, .. } => branches@.len() - from,
        _ => 0int,
    }
{
    if let GlobalChoreo::Choice { branches, .. } = choice {
        if from < branches@.len() {
            global_choice_branches_view_len(choice, from + 1);
        }
    }
}

proof fn global_choice_branches_view_nth(choice: GlobalChoreo, from: int, k: int)
    requires
        match choice {
            GlobalChoreo::Choice { branches, .. } => 0 <= from <= k < branches@.len(),
            _ => false,
        },
    ensures ({
        let viewed = global_choice_branches_view(choice, from);
        match choice {
            GlobalChoreo::Choice { branches, .. } => {
                &&& viewed.len() == branches@.len() - from
                &&& viewed[k - from].label == branches[k].label
                &&& viewed[k - from].body == global_view(branches[k].body)
            }
            _ => false,
        }
    })
    decreases k - from
{
    if let GlobalChoreo::Choice { branches, .. } = choice {
        if from == k {
            global_choice_branches_view_len(choice, from);
        } else {
            global_choice_branches_view_nth(choice, from + 1, k);
            global_choice_branches_view_len(choice, from);
        }
    }
}

proof fn participants_prefix_view_eq(choice: GlobalChoreo, n: int)
    requires
        match choice {
            GlobalChoreo::Choice { branches, .. } => 0 <= n <= branches@.len(),
            _ => false,
        },
    ensures
        match choice {
            GlobalChoreo::Choice { branches, .. } =>
                participants_choice_branches_prefix_spec(global_view(choice), n)
                    == participants_branches_exec_prefix_spec(branches@, n),
            _ => false,
        }
    decreases n
{
    if n > 0 {
        participants_prefix_view_eq(choice, n - 1);
        global_choice_branches_view_nth(choice, 0, n - 1);
    }
}

proof fn participants_exec_choice_eq(choice: GlobalChoreo)
    requires
        match choice {
            GlobalChoreo::Choice { .. } => true,
            _ => false,
        }
    ensures
        participants_exec_spec(choice) == match choice {
            GlobalChoreo::Choice { decider, branches } => participants_choice_exec_spec(branches@, decider),
            _ => seq![],
        }
{
    reveal(participants_exec_spec);
    if let GlobalChoreo::Choice { decider, branches } = choice {
        participants_prefix_view_eq(choice, branches@.len() as int);
        assert(participants_choice_branches_prefix_spec(global_view(choice), branches@.len() as int)
            == participants_branches_exec_prefix_spec(branches@, branches@.len() as int));
        assert(global_view(choice) == GChoreo::Choice {
            decider,
            branches: global_choice_branches_view(choice, 0),
        });
        global_choice_branches_view_len(choice, 0);
        assert(global_choice_branches_view(choice, 0).len() == branches@.len());
        assert(participants_spec(GChoreo::Choice {
            decider,
            branches: global_choice_branches_view(choice, 0),
        })
            == insert_role_spec(
                participants_choice_branches_prefix_spec(
                    GChoreo::Choice { decider, branches: global_choice_branches_view(choice, 0) },
                    branches@.len() as int,
                ),
                decider,
            ));
        assert(participants_spec(global_view(choice))
            == insert_role_spec(
                participants_choice_branches_prefix_spec(global_view(choice), branches@.len() as int),
                decider,
            ));
        assert(participants_exec_spec(choice) == participants_spec(global_view(choice)));
        assert(participants_exec_spec(choice)
            == insert_role_spec(
                participants_choice_branches_prefix_spec(global_view(choice), branches@.len() as int),
                decider,
            ));
        assert(participants_exec_spec(choice) == participants_choice_exec_spec(branches@, decider));
    }
}

#[verifier::exec_allows_no_decreases_clause]
fn participants_branches(branches: &[GlobalBranch]) -> (result: Vec<RoleId>)
    ensures result@ == participants_branches_exec_prefix_spec(branches@, branches.len() as int)
{
    let mut roles: Vec<RoleId> = Vec::new();
    let mut i: usize = 0;
    while i < branches.len()
        invariant
            0 <= i <= branches@.len(),
            roles@ == participants_branches_exec_prefix_spec(branches@, i as int),
        decreases branches@.len() - i,
    {
        let branch_roles = participants(&branches[i].body);
        append_unique(&mut roles, &branch_roles);
        i += 1;
    }
    roles
}

// r[impl choreography.participants]
#[verifier::exec_allows_no_decreases_clause]
pub fn participants(global: &GlobalChoreo) -> (result: Vec<RoleId>)
    ensures result@ == participants_exec_spec(*global)
{
    match global {
        GlobalChoreo::End => {
            proof {
                reveal(participants_exec_spec);
            }
            Vec::new()
        }
        GlobalChoreo::Comm { from, to, next, .. } => {
            let mut roles = participants(next);
            push_unique(&mut roles, *from);
            push_unique(&mut roles, *to);
            proof {
                reveal(participants_exec_spec);
            }
            roles
        }
        GlobalChoreo::Choice { decider, branches } => {
            let mut roles = participants_branches(branches);
            push_unique(&mut roles, *decider);
            proof {
                participants_exec_choice_eq(*global);
                reveal(participants_exec_spec);
            }
            assert(roles@ == participants_choice_exec_spec(branches@, *decider));
            assert(participants_exec_spec(*global) == participants_choice_exec_spec(branches@, *decider));
            roles
        }
    }
}

// r[impl choreography.communication]
pub fn comm_parts(global: &GlobalChoreo) -> (result: Option<(RoleId, RoleId, LabelId, PayloadTag, GlobalChoreo)>)
    ensures comm_parts_post_spec(*global, result)
{
    match global {
        GlobalChoreo::Comm { from, to, label, payload_tag, next } => {
            Some((*from, *to, *label, *payload_tag, clone_global(next)))
        }
        _ => None,
    }
}

#[verifier::exec_allows_no_decreases_clause]
fn well_formed_global_choice(branches: &[GlobalBranch]) -> (result: bool)
    ensures
        result == (branches@.len() > 0
            && global_exec_branch_labels_strict_spec(branches@)
            && forall|j: int| #![trigger branches@[j]]
                0 <= j < branches@.len() ==> well_formed_global_runtime_spec(branches@[j].body))
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
                0 <= j < i as int ==> well_formed_global_runtime_spec(branches@[j].body),
        decreases branches@.len() - i,
    {
        if i > 0 && branches[i - 1].label >= branches[i].label {
            proof {
                adjacent_violation_exec_not_strict(branches@, i as int);
                assert(!(branches@.len() > 0
                    && global_exec_branch_labels_strict_spec(branches@)
                    && forall|j: int| #![trigger branches@[j]]
                        0 <= j < branches@.len() ==> well_formed_global_runtime_spec(branches@[j].body)));
            }
            return false;
        }
        if !well_formed_global(&branches[i].body) {
            proof {
                assert(!well_formed_global_runtime_spec(branches@[i as int].body));
                assert(!(forall|j: int| #![trigger branches@[j]]
                    0 <= j < branches@.len() ==> well_formed_global_runtime_spec(branches@[j].body))) by {
                    assert((i as int) < branches@.len());
                    assert(!well_formed_global_runtime_spec(branches@[i as int].body));
                }
                assert(!(branches@.len() > 0
                    && global_exec_branch_labels_strict_spec(branches@)
                    && forall|j: int| #![trigger branches@[j]]
                        0 <= j < branches@.len() ==> well_formed_global_runtime_spec(branches@[j].body)));
            }
            return false;
        }
        proof {
            assert(well_formed_global_runtime_spec(branches@[i as int].body));
            if i > 0 {
                assert(branches@[i as int - 1].label < branches@[i as int].label);
            }
        }
        i += 1;
    }
    proof {
        adjacent_increasing_prefix_implies_pairwise_prefix(branches@, branches@.len() as int);
        assert(global_exec_branch_labels_strict_spec(branches@));
        assert(forall|j: int| #![trigger branches@[j]]
            0 <= j < branches@.len() ==> well_formed_global_runtime_spec(branches@[j].body));
    }
    true
}

// r[impl choreography.participants]
#[verifier::exec_allows_no_decreases_clause]
pub fn well_formed_global(global: &GlobalChoreo) -> (result: bool)
    ensures result == well_formed_global_runtime_spec(*global)
{
    match global {
        GlobalChoreo::End => {
            proof { reveal(well_formed_global_runtime_spec); }
            true
        }
        GlobalChoreo::Comm { from, to, next, .. } => {
            let result = *from != *to && well_formed_global(next);
            proof { reveal(well_formed_global_runtime_spec); }
            result
        }
        GlobalChoreo::Choice { branches, .. } => {
            let result = well_formed_global_choice(branches);
            proof { reveal(well_formed_global_runtime_spec); }
            result
        }
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn branch(label: LabelId, body: GlobalChoreo) -> GlobalBranch {
        GlobalBranch { label, body }
    }

    fn roles_set(mut roles: Vec<RoleId>) -> Vec<RoleId> {
        roles.sort_unstable();
        roles.dedup();
        roles
    }

    fn brute_participants(global: &GlobalChoreo) -> Vec<RoleId> {
        match global {
            GlobalChoreo::End => vec![],
            GlobalChoreo::Comm { from, to, next, .. } => {
                let mut roles = brute_participants(next);
                roles.push(*from);
                roles.push(*to);
                roles_set(roles)
            }
            GlobalChoreo::Choice { decider, branches } => {
                let mut roles = vec![*decider];
                for branch in branches {
                    roles.extend(brute_participants(&branch.body));
                }
                roles_set(roles)
            }
        }
    }

    fn arb_global(depth: u32) -> impl Strategy<Value = GlobalChoreo> {
        let leaf = Just(GlobalChoreo::End);
        leaf.prop_recursive(depth, depth * 8, 2, |inner| {
            prop_oneof![
                (0u32..5, 0u32..5, 0u32..5, 0u32..5, inner.clone()).prop_map(|(from, to, label, payload_tag, next)| {
                    GlobalChoreo::Comm {
                        from,
                        to,
                        label,
                        payload_tag,
                        next: Box::new(next),
                    }
                }),
                (0u32..5, prop::collection::vec((0u32..8, inner.clone()), 1..=3)).prop_map(|(decider, mut raw)| {
                    raw.sort_by_key(|(label, _)| *label);
                    raw.dedup_by(|a, b| a.0 == b.0);
                    let branches = raw.into_iter().map(|(label, body)| branch(label, body)).collect();
                    GlobalChoreo::Choice { decider, branches }
                }),
            ]
        })
    }

    // r[verify choreography.communication]
    #[test]
    fn test_comm_parts_exact() {
        let next = GlobalChoreo::End;
        let global = GlobalChoreo::Comm {
            from: 1,
            to: 4,
            label: 9,
            payload_tag: 7,
            next: Box::new(next.clone()),
        };
        assert_eq!(comm_parts(&global), Some((1, 4, 9, 7, next)));
    }

    // r[verify choreography.participants]
    #[test]
    fn test_self_send_rejected() {
        let global = GlobalChoreo::Comm {
            from: 3,
            to: 3,
            label: 1,
            payload_tag: 0,
            next: Box::new(GlobalChoreo::End),
        };
        assert!(!well_formed_global(&global));
    }

    // r[verify choreography.participants]
    #[test]
    fn test_duplicate_labels_rejected() {
        let global = GlobalChoreo::Choice {
            decider: 0,
            branches: vec![branch(4, GlobalChoreo::End), branch(4, GlobalChoreo::End)],
        };
        assert!(!well_formed_global(&global));
    }

    // r[verify choreography.participants]
    #[test]
    fn test_participants_exact() {
        let global = GlobalChoreo::Choice {
            decider: 9,
            branches: vec![
                branch(1, GlobalChoreo::Comm {
                    from: 1,
                    to: 4,
                    label: 10,
                    payload_tag: 0,
                    next: Box::new(GlobalChoreo::End),
                }),
                branch(2, GlobalChoreo::Comm {
                    from: 4,
                    to: 1,
                    label: 11,
                    payload_tag: 1,
                    next: Box::new(GlobalChoreo::End),
                }),
            ],
        };
        let mut got = participants(&global);
        got.sort_unstable();
        assert_eq!(got, vec![1, 4, 9]);
    }

    proptest! {
        // r[verify choreography.participants]
        #[test]
        fn prop_participants_match_bruteforce(global in arb_global(3)) {
            let mut got = participants(&global);
            got.sort_unstable();
            got.dedup();
            prop_assert_eq!(got, brute_participants(&global));
        }

        // r[verify choreography.participants]
        #[test]
        fn prop_duplicate_adjacent_label_choice_rejected(
            decider in 0u32..5,
            label in 0u32..8,
            body1 in arb_global(2),
            body2 in arb_global(2),
        ) {
            let global = GlobalChoreo::Choice {
                decider,
                branches: vec![branch(label, body1), branch(label, body2)],
            };
            prop_assert!(!well_formed_global(&global));
        }
    }
}
