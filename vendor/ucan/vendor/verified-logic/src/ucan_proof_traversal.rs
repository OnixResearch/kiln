//! Verified UCAN proof traversal transcript primitives.
//!
//! These helpers check finite, caller-supplied traversal transcripts. They do not
//! fetch proofs, parse compact tokens, verify signatures, query revocation
//! backends, persist state, sign, replay-protect, or read an ambient clock.

#![allow(clippy::ptr_arg)]

use vstd::prelude::*;

use crate::ucan_ability;

verus! {

/// Capability entry carried in a resolved traversal transcript.
pub struct TraversalCapability {
    pub resource: Vec<u8>,
    pub ability: ucan_ability::Ability,
}

/// A resolved proof traversal entry supplied by the shell.
pub struct TraversalEntry {
    pub proof_ref: Vec<u8>,
    pub issuer: Vec<u8>,
    pub audience: Vec<u8>,
    pub capabilities: Vec<TraversalCapability>,
    pub caveat_ids: Vec<Vec<u8>>,
    pub nbf: u64,
    pub exp: u64,
    pub signature_valid: bool,
    pub revoked: bool,
}

/// Explicit traversal budgets supplied by the shell.
pub struct TraversalBudget {
    pub max_depth: u64,
    pub max_proofs: u64,
    pub max_input_bytes: u64,
}

/// Requested authorization target for the leaf of a traversal transcript.
pub struct TraversalAuthorizationTarget {
    pub resource: Vec<u8>,
    pub ability: ucan_ability::Ability,
    pub now: u64,
    pub tolerance: u64,
}

/// A finite shell-produced proof traversal transcript.
pub struct TraversalTranscript {
    pub entries: Vec<TraversalEntry>,
    pub visited_refs: Vec<Vec<u8>>,
    pub total_input_bytes: u64,
    pub target: TraversalAuthorizationTarget,
}

/// A finite topologically ordered proof DAG transcript.
pub struct TraversalDagTranscript {
    pub entries: Vec<TraversalEntry>,
    pub parent_indices: Vec<Vec<usize>>,
    pub visited_refs: Vec<Vec<u8>>,
    pub total_input_bytes: u64,
    pub target: TraversalAuthorizationTarget,
}

pub open spec fn bytes_equal_spec(a: Seq<u8>, b: Seq<u8>) -> bool {
    a =~= b
}

pub open spec fn cap_view(c: &TraversalCapability) -> (Seq<u8>, Seq<Seq<u8>>) {
    (c.resource@, c.ability.segments@.map_values(|s: Vec<u8>| s@))
}

// r[impl ucan-proof-traversal.capability-containment]
pub open spec fn capability_contains_spec(parent: &TraversalCapability, child: &TraversalCapability) -> bool {
    child.resource@.len() >= parent.resource@.len()
    && child.resource@.subrange(0, parent.resource@.len() as int) =~= parent.resource@
    && ucan_ability::ability_contains_spec2(parent.ability, child.ability)
}

pub open spec fn capability_authorizes_spec(cap: &TraversalCapability, target: &TraversalAuthorizationTarget) -> bool {
    target.resource@.len() >= cap.resource@.len()
    && target.resource@.subrange(0, cap.resource@.len() as int) =~= cap.resource@
    && ucan_ability::ability_contains_spec2(cap.ability, target.ability)
}

pub open spec fn capability_set_contains_child_spec(parent: Seq<TraversalCapability>, child: &TraversalCapability) -> bool {
    exists|i: int| 0 <= i < parent.len() && #[trigger] capability_contains_spec(&parent[i], child)
}

pub open spec fn capability_set_authorizes_spec(caps: Seq<TraversalCapability>, target: &TraversalAuthorizationTarget) -> bool {
    exists|i: int| 0 <= i < caps.len() && #[trigger] capability_authorizes_spec(&caps[i], target)
}

pub open spec fn child_capabilities_attenuate_spec(parent: &TraversalEntry, child: &TraversalEntry) -> bool {
    forall|i: int| #![trigger child.capabilities@[i]] 0 <= i < child.capabilities@.len() ==>
        capability_set_contains_child_spec(parent.capabilities@, &child.capabilities@[i])
}

pub open spec fn parent_caveats_preserved_spec(parent: &TraversalEntry, child: &TraversalEntry) -> bool {
    child.caveat_ids@.len() >= parent.caveat_ids@.len()
    && forall|i: int| #![trigger parent.caveat_ids@[i]] 0 <= i < parent.caveat_ids@.len() ==>
        bytes_equal_spec(parent.caveat_ids@[i]@, child.caveat_ids@[i]@)
}

// r[impl ucan-proof-traversal.graph-well-formedness]
pub open spec fn proof_edge_well_formed_spec(parent: &TraversalEntry, child: &TraversalEntry) -> bool {
    parent.audience@ =~= child.issuer@
    && parent.nbf <= child.nbf
    && child.exp <= parent.exp
    && child_capabilities_attenuate_spec(parent, child)
    && parent_caveats_preserved_spec(parent, child)
}

pub open spec fn ref_seen_before_spec(refs: Seq<Vec<u8>>, upto: int, candidate: Seq<u8>) -> bool {
    exists|i: int| 0 <= i < upto && i < refs.len() && #[trigger] bytes_equal_spec(refs[i]@, candidate)
}

// r[impl ucan-proof-traversal.bounded-traversal]
pub open spec fn no_duplicate_refs_spec(refs: Seq<Vec<u8>>) -> bool {
    forall|i: int, j: int| 0 <= i < j < refs.len() ==> !(#[trigger] bytes_equal_spec(refs[i]@, refs[j]@))
}

pub open spec fn entry_refs_match_visited_spec(entries: Seq<TraversalEntry>, refs: Seq<Vec<u8>>) -> bool {
    entries.len() == refs.len()
    && forall|i: int| #![trigger entries[i]] 0 <= i < entries.len() ==> bytes_equal_spec(entries[i].proof_ref@, refs[i]@)
}

// r[impl ucan-proof-traversal.bounded-traversal]
pub open spec fn traversal_within_budget_spec(t: &TraversalTranscript, budget: &TraversalBudget) -> bool {
    t.entries@.len() > 0
    && t.entries@.len() <= budget.max_proofs
    && (t.entries@.len() - 1) <= budget.max_depth
    && t.total_input_bytes <= budget.max_input_bytes
    && entry_refs_match_visited_spec(t.entries@, t.visited_refs@)
    && no_duplicate_refs_spec(t.visited_refs@)
}

pub open spec fn all_edges_well_formed_spec(entries: Seq<TraversalEntry>) -> bool {
    forall|i: int| #![trigger entries[i]] 0 <= i < entries.len() - 1 ==>
        proof_edge_well_formed_spec(&entries[i], &entries[i + 1])
}

// r[impl ucan-proof-traversal.effective-delegation-folding]
pub open spec fn effective_delegation_fold_valid_spec(t: &TraversalTranscript) -> bool {
    t.entries@.len() > 0 && all_edges_well_formed_spec(t.entries@)
}

pub open spec fn entry_temporal_valid_spec(entry: &TraversalEntry, target: &TraversalAuthorizationTarget) -> bool {
    (target.now as int) <= (entry.exp as int) + (target.tolerance as int)
    && (entry.nbf as int) <= (target.now as int) + (target.tolerance as int)
}

pub open spec fn all_entries_temporal_valid_spec(entries: Seq<TraversalEntry>, target: &TraversalAuthorizationTarget) -> bool {
    forall|i: int| #![trigger entries[i]] 0 <= i < entries.len() ==> entry_temporal_valid_spec(&entries[i], target)
}

// r[impl ucan-proof-traversal.chain-authorization]
pub open spec fn proof_chain_authorizes_spec(t: &TraversalTranscript) -> bool {
    t.entries@.len() > 0
    && effective_delegation_fold_valid_spec(t)
    && all_entries_temporal_valid_spec(t.entries@, &t.target)
    && capability_set_authorizes_spec(t.entries@[t.entries@.len() - 1].capabilities@, &t.target)
}

// r[impl ucan-proof-traversal.revocation-aware-chain]
pub open spec fn traversal_not_revoked_spec(t: &TraversalTranscript) -> bool {
    forall|i: int| #![trigger t.entries@[i]] 0 <= i < t.entries@.len() ==> !t.entries@[i].revoked
}

// r[impl ucan-proof-traversal.transcript-consistency]
pub open spec fn traversal_transcript_valid_spec(t: &TraversalTranscript, budget: &TraversalBudget) -> bool {
    traversal_within_budget_spec(t, budget)
    && proof_chain_authorizes_spec(t)
    && traversal_not_revoked_spec(t)
}

pub open spec fn dag_entry_refs_match_visited_spec(entries: Seq<TraversalEntry>, refs: Seq<Vec<u8>>) -> bool {
    entries.len() == refs.len()
    && forall|i: int| #![trigger entries[i]] 0 <= i < entries.len() ==> bytes_equal_spec(entries[i].proof_ref@, refs[i]@)
}

// r[impl ucan-proof-traversal.dag-transcript-consistency]
pub open spec fn dag_within_budget_spec(t: &TraversalDagTranscript, budget: &TraversalBudget) -> bool {
    t.entries@.len() > 0
    && t.entries@.len() <= budget.max_proofs
    && (t.entries@.len() - 1) <= budget.max_depth
    && t.total_input_bytes <= budget.max_input_bytes
    && dag_entry_refs_match_visited_spec(t.entries@, t.visited_refs@)
    && no_duplicate_refs_spec(t.visited_refs@)
}

pub open spec fn dag_parent_indices_well_formed_spec(t: &TraversalDagTranscript) -> bool {
    t.entries@.len() == t.parent_indices@.len()
    && t.entries@.len() > 0
    && forall|child: int| #![trigger t.parent_indices@[child]] 0 <= child < t.parent_indices@.len() ==>
        forall|slot: int| #![trigger t.parent_indices@[child][slot]] 0 <= slot < t.parent_indices@[child].len() ==>
            (t.parent_indices@[child][slot] as int) < child
}

pub open spec fn parent_link_well_formed_spec(parent: &TraversalEntry, child: &TraversalEntry) -> bool {
    parent.audience@ =~= child.issuer@
    && parent.nbf <= child.nbf
    && child.exp <= parent.exp
}

pub open spec fn dag_child_capability_has_parent_spec(
    t: &TraversalDagTranscript,
    child: int,
    capability: &TraversalCapability,
) -> bool {
    exists|slot: int| 0 <= slot < t.parent_indices@[child].len()
        && capability_set_contains_child_spec(t.entries@[(#[trigger] t.parent_indices@[child][slot]) as int].capabilities@, capability)
}

pub open spec fn dag_child_capabilities_attenuate_spec(t: &TraversalDagTranscript, child: int) -> bool {
    forall|cap: int| #![trigger t.entries@[child].capabilities@[cap]] 0 <= cap < t.entries@[child].capabilities@.len() ==>
        dag_child_capability_has_parent_spec(t, child, &t.entries@[child].capabilities@[cap])
}

pub open spec fn dag_parent_caveats_preserved_spec(t: &TraversalDagTranscript, child: int) -> bool {
    forall|slot: int| #![trigger t.parent_indices@[child][slot]] 0 <= slot < t.parent_indices@[child].len() ==>
        parent_caveats_preserved_spec(&t.entries@[t.parent_indices@[child][slot] as int], &t.entries@[child])
}

// r[impl ucan-proof-traversal.dag-transcript-consistency]
pub open spec fn dag_child_edges_well_formed_spec(t: &TraversalDagTranscript, child: int) -> bool {
    (forall|slot: int| #![trigger t.parent_indices@[child][slot]] 0 <= slot < t.parent_indices@[child].len() ==>
        parent_link_well_formed_spec(&t.entries@[t.parent_indices@[child][slot] as int], &t.entries@[child]))
    && dag_child_capabilities_attenuate_spec(t, child)
    && dag_parent_caveats_preserved_spec(t, child)
}

pub open spec fn dag_edges_well_formed_spec(t: &TraversalDagTranscript) -> bool {
    dag_parent_indices_well_formed_spec(t)
    && forall|child: int| #![trigger t.entries@[child]] 0 <= child < t.entries@.len() && t.parent_indices@[child].len() > 0 ==>
        dag_child_edges_well_formed_spec(t, child)
}

pub open spec fn dag_proof_authorizes_spec(t: &TraversalDagTranscript) -> bool {
    t.entries@.len() > 0
    && dag_edges_well_formed_spec(t)
    && all_entries_temporal_valid_spec(t.entries@, &t.target)
    && capability_set_authorizes_spec(t.entries@[t.entries@.len() - 1].capabilities@, &t.target)
}

pub open spec fn dag_not_revoked_spec(t: &TraversalDagTranscript) -> bool {
    forall|i: int| #![trigger t.entries@[i]] 0 <= i < t.entries@.len() ==> !t.entries@[i].revoked
}

// r[impl ucan-proof-traversal.dag-transcript-consistency]
pub open spec fn dag_traversal_transcript_valid_spec(t: &TraversalDagTranscript, budget: &TraversalBudget) -> bool {
    dag_within_budget_spec(t, budget)
    && dag_proof_authorizes_spec(t)
    && dag_not_revoked_spec(t)
}

// r[impl ucan-proof-traversal.byte-equality]
pub fn bytes_equal(a: &Vec<u8>, b: &Vec<u8>) -> (result: bool)
    ensures result == bytes_equal_spec(a@, b@),
{
    if a.len() != b.len() {
        return false;
    }
    let mut i: usize = 0;
    while i < a.len()
        invariant
            0 <= i <= a.len(),
            a.len() == b.len(),
            forall|k: int| 0 <= k < i ==> a@[k] == b@[k],
        decreases a.len() - i,
    {
        if a[i] != b[i] {
            assert(a@[i as int] != b@[i as int]);
            assert(!(a@ =~= b@));
            return false;
        }
        i = i + 1;
    }
    assert(a@ =~= b@);
    true
}

// r[impl ucan-proof-traversal.capability-containment]
pub fn resource_has_prefix(resource: &Vec<u8>, prefix: &Vec<u8>) -> (result: bool)
    ensures result == (resource@.len() >= prefix@.len()
        && resource@.subrange(0, prefix@.len() as int) =~= prefix@),
{
    if resource.len() < prefix.len() {
        return false;
    }
    let mut i: usize = 0;
    while i < prefix.len()
        invariant
            0 <= i <= prefix.len(),
            resource.len() >= prefix.len(),
            forall|k: int| 0 <= k < i ==> resource@[k] == prefix@[k],
        decreases prefix.len() - i,
    {
        if resource[i] != prefix[i] {
            assert(resource@[i as int] != prefix@[i as int]);
            assert(!(resource@.subrange(0, prefix@.len() as int) =~= prefix@));
            return false;
        }
        i = i + 1;
    }
    assert(resource@.subrange(0, prefix@.len() as int) =~= prefix@);
    true
}

// r[impl ucan-proof-traversal.capability-containment]
pub fn capability_contains(parent: &TraversalCapability, child: &TraversalCapability) -> (result: bool)
    ensures result == capability_contains_spec(parent, child),
{
    if !resource_has_prefix(&child.resource, &parent.resource) {
        return false;
    }
    if !ucan_ability::ability_subsumes(&parent.ability, &child.ability) {
        return false;
    }
    true
}

// r[impl ucan-proof-traversal.chain-authorization]
pub fn capability_authorizes(cap: &TraversalCapability, target: &TraversalAuthorizationTarget) -> (result: bool)
    ensures result == capability_authorizes_spec(cap, target),
{
    if !resource_has_prefix(&target.resource, &cap.resource) {
        return false;
    }
    if !ucan_ability::ability_subsumes(&cap.ability, &target.ability) {
        return false;
    }
    true
}

// r[impl ucan-proof-traversal.effective-delegation-folding]
pub fn child_capability_has_parent(parent_caps: &Vec<TraversalCapability>, child: &TraversalCapability) -> (result: bool)
    ensures result == capability_set_contains_child_spec(parent_caps@, child),
{
    let mut i: usize = 0;
    while i < parent_caps.len()
        invariant
            0 <= i <= parent_caps.len(),
            forall|k: int| 0 <= k < i ==> !capability_contains_spec(&parent_caps@[k], child),
        decreases parent_caps.len() - i,
    {
        if capability_contains(&parent_caps[i], child) {
            assert(capability_contains_spec(&parent_caps@[i as int], child));
            return true;
        }
        i = i + 1;
    }
    false
}

// r[impl ucan-proof-traversal.effective-delegation-folding]
pub fn child_capabilities_attenuate(parent: &TraversalEntry, child: &TraversalEntry) -> (result: bool)
    ensures result == child_capabilities_attenuate_spec(parent, child),
{
    let mut i: usize = 0;
    while i < child.capabilities.len()
        invariant
            0 <= i <= child.capabilities.len(),
            forall|k: int| 0 <= k < i ==> capability_set_contains_child_spec(parent.capabilities@, &child.capabilities@[k]),
        decreases child.capabilities.len() - i,
    {
        if !child_capability_has_parent(&parent.capabilities, &child.capabilities[i]) {
            assert(!capability_set_contains_child_spec(parent.capabilities@, &child.capabilities@[i as int]));
            assert(!child_capabilities_attenuate_spec(parent, child));
            return false;
        }
        i = i + 1;
    }
    true
}

// r[impl ucan-proof-traversal.effective-delegation-folding]
pub fn caveat_id_present(caveats: &Vec<Vec<u8>>, candidate: &Vec<u8>) -> (result: bool)
    ensures result == (exists|i: int| 0 <= i < caveats@.len() && #[trigger] bytes_equal_spec(caveats@[i]@, candidate@)),
{
    let mut i: usize = 0;
    while i < caveats.len()
        invariant
            0 <= i <= caveats.len(),
            forall|k: int| 0 <= k < i ==> !bytes_equal_spec(caveats@[k]@, candidate@),
        decreases caveats.len() - i,
    {
        if bytes_equal(&caveats[i], candidate) {
            assert(bytes_equal_spec(caveats@[i as int]@, candidate@));
            return true;
        }
        i = i + 1;
    }
    false
}

// r[impl ucan-proof-traversal.effective-delegation-folding]
pub fn parent_caveats_preserved(parent: &TraversalEntry, child: &TraversalEntry) -> (result: bool)
    ensures result == parent_caveats_preserved_spec(parent, child),
{
    if child.caveat_ids.len() < parent.caveat_ids.len() {
        return false;
    }
    let mut i: usize = 0;
    while i < parent.caveat_ids.len()
        invariant
            0 <= i <= parent.caveat_ids.len(),
            child.caveat_ids.len() >= parent.caveat_ids.len(),
            forall|k: int| #![trigger parent.caveat_ids@[k]] 0 <= k < i ==>
                bytes_equal_spec(parent.caveat_ids@[k]@, child.caveat_ids@[k]@),
        decreases parent.caveat_ids.len() - i,
    {
        if !bytes_equal(&parent.caveat_ids[i], &child.caveat_ids[i]) {
            assert(!parent_caveats_preserved_spec(parent, child));
            return false;
        }
        i = i + 1;
    }
    assert(parent_caveats_preserved_spec(parent, child));
    true
}

// r[impl ucan-proof-traversal.graph-well-formedness]
pub fn proof_edge_well_formed(parent: &TraversalEntry, child: &TraversalEntry) -> (result: bool)
    ensures result == proof_edge_well_formed_spec(parent, child),
{
    if !bytes_equal(&parent.audience, &child.issuer) {
        return false;
    }
    if parent.nbf > child.nbf {
        return false;
    }
    if child.exp > parent.exp {
        return false;
    }
    if !child_capabilities_attenuate(parent, child) {
        return false;
    }
    if !parent_caveats_preserved(parent, child) {
        return false;
    }
    true
}

// r[impl ucan-proof-traversal.bounded-traversal]
pub fn no_duplicate_refs(refs: &Vec<Vec<u8>>) -> (result: bool)
    ensures result == no_duplicate_refs_spec(refs@),
{
    let mut i: usize = 0;
    while i < refs.len()
        invariant
            0 <= i <= refs.len(),
            forall|a: int, b: int| 0 <= a < b < i ==> !bytes_equal_spec(refs@[a]@, refs@[b]@),
        decreases refs.len() - i,
    {
        let mut j: usize = 0;
        while j < i
            invariant
                0 <= j <= i,
                i < refs.len(),
                forall|k: int| 0 <= k < j ==> !bytes_equal_spec(refs@[k]@, refs@[i as int]@),
            decreases i - j,
        {
            if bytes_equal(&refs[j], &refs[i]) {
                assert(bytes_equal_spec(refs@[j as int]@, refs@[i as int]@));
                assert(!no_duplicate_refs_spec(refs@));
                return false;
            }
            j = j + 1;
        }
        i = i + 1;
    }
    true
}

// r[impl ucan-proof-traversal.transcript-consistency]
pub fn entry_refs_match_visited(entries: &Vec<TraversalEntry>, refs: &Vec<Vec<u8>>) -> (result: bool)
    ensures result == entry_refs_match_visited_spec(entries@, refs@),
{
    if entries.len() != refs.len() {
        return false;
    }
    let mut i: usize = 0;
    while i < entries.len()
        invariant
            0 <= i <= entries.len(),
            entries.len() == refs.len(),
            forall|k: int| 0 <= k < i ==> bytes_equal_spec(entries@[k].proof_ref@, refs@[k]@),
        decreases entries.len() - i,
    {
        if !bytes_equal(&entries[i].proof_ref, &refs[i]) {
            return false;
        }
        i = i + 1;
    }
    true
}

// r[impl ucan-proof-traversal.bounded-traversal]
pub fn traversal_within_budget(t: &TraversalTranscript, budget: &TraversalBudget) -> (result: bool)
    ensures result == traversal_within_budget_spec(t, budget),
{
    if t.entries.len() == 0 {
        return false;
    }
    if (t.entries.len() as u128) > (budget.max_proofs as u128) {
        return false;
    }
    if ((t.entries.len() - 1) as u128) > (budget.max_depth as u128) {
        return false;
    }
    if t.total_input_bytes > budget.max_input_bytes {
        return false;
    }
    if !entry_refs_match_visited(&t.entries, &t.visited_refs) {
        return false;
    }
    if !no_duplicate_refs(&t.visited_refs) {
        return false;
    }
    true
}

// r[impl ucan-proof-traversal.effective-delegation-folding]
pub fn effective_delegation_fold_valid(t: &TraversalTranscript) -> (result: bool)
    ensures result == effective_delegation_fold_valid_spec(t),
{
    if t.entries.len() == 0 {
        return false;
    }
    if t.entries.len() <= 1 {
        return true;
    }
    let mut i: usize = 0;
    while i < t.entries.len() - 1
        invariant
            0 <= i <= t.entries.len() - 1,
            forall|k: int| #![trigger t.entries@[k]] 0 <= k < i ==>
                proof_edge_well_formed_spec(&t.entries@[k], &t.entries@[k + 1]),
        decreases t.entries.len() - 1 - i,
    {
        if !proof_edge_well_formed(&t.entries[i], &t.entries[i + 1]) {
            return false;
        }
        i = i + 1;
    }
    true
}

// r[impl ucan-proof-traversal.chain-authorization]
pub fn all_entries_temporal_valid(entries: &Vec<TraversalEntry>, target: &TraversalAuthorizationTarget) -> (result: bool)
    ensures result == all_entries_temporal_valid_spec(entries@, target),
{
    let mut i: usize = 0;
    while i < entries.len()
        invariant
            0 <= i <= entries.len(),
            forall|k: int| #![trigger entries@[k]] 0 <= k < i ==> entry_temporal_valid_spec(&entries@[k], target),
        decreases entries.len() - i,
    {
        let e = &entries[i];
        if (target.now as u128) > (e.exp as u128) + (target.tolerance as u128) {
            return false;
        }
        if (e.nbf as u128) > (target.now as u128) + (target.tolerance as u128) {
            return false;
        }
        i = i + 1;
    }
    true
}

// r[impl ucan-proof-traversal.chain-authorization]
pub fn capability_set_authorizes(caps: &Vec<TraversalCapability>, target: &TraversalAuthorizationTarget) -> (result: bool)
    ensures result == capability_set_authorizes_spec(caps@, target),
{
    let mut i: usize = 0;
    while i < caps.len()
        invariant
            0 <= i <= caps.len(),
            forall|k: int| 0 <= k < i ==> !capability_authorizes_spec(&caps@[k], target),
        decreases caps.len() - i,
    {
        if capability_authorizes(&caps[i], target) {
            return true;
        }
        i = i + 1;
    }
    false
}

// r[impl ucan-proof-traversal.chain-authorization]
pub fn proof_chain_authorizes(t: &TraversalTranscript) -> (result: bool)
    ensures result == proof_chain_authorizes_spec(t),
{
    if t.entries.len() == 0 {
        return false;
    }
    if !effective_delegation_fold_valid(t) {
        return false;
    }
    if !all_entries_temporal_valid(&t.entries, &t.target) {
        return false;
    }
    let leaf = t.entries.len() - 1;
    if !capability_set_authorizes(&t.entries[leaf].capabilities, &t.target) {
        return false;
    }
    true
}

// r[impl ucan-proof-traversal.revocation-aware-chain]
pub fn traversal_not_revoked(t: &TraversalTranscript) -> (result: bool)
    ensures result == traversal_not_revoked_spec(t),
{
    let mut i: usize = 0;
    while i < t.entries.len()
        invariant
            0 <= i <= t.entries.len(),
            forall|k: int| #![trigger t.entries@[k]] 0 <= k < i ==> !t.entries@[k].revoked,
        decreases t.entries.len() - i,
    {
        if t.entries[i].revoked {
            return false;
        }
        i = i + 1;
    }
    true
}

// r[impl ucan-proof-traversal.transcript-consistency]
pub fn traversal_transcript_valid(t: &TraversalTranscript, budget: &TraversalBudget) -> (result: bool)
    ensures result == traversal_transcript_valid_spec(t, budget),
{
    if !traversal_within_budget(t, budget) {
        return false;
    }
    if !proof_chain_authorizes(t) {
        return false;
    }
    if !traversal_not_revoked(t) {
        return false;
    }
    true
}

// r[impl ucan-proof-traversal.dag-transcript-consistency]
pub fn dag_entry_refs_match_visited(entries: &Vec<TraversalEntry>, refs: &Vec<Vec<u8>>) -> (result: bool)
    ensures result == dag_entry_refs_match_visited_spec(entries@, refs@),
{
    entry_refs_match_visited(entries, refs)
}

// r[impl ucan-proof-traversal.dag-transcript-consistency]
pub fn dag_within_budget(t: &TraversalDagTranscript, budget: &TraversalBudget) -> (result: bool)
    ensures result == dag_within_budget_spec(t, budget),
{
    if t.entries.len() == 0 {
        return false;
    }
    if (t.entries.len() as u128) > (budget.max_proofs as u128) {
        return false;
    }
    if ((t.entries.len() - 1) as u128) > (budget.max_depth as u128) {
        return false;
    }
    if t.total_input_bytes > budget.max_input_bytes {
        return false;
    }
    if !dag_entry_refs_match_visited(&t.entries, &t.visited_refs) {
        return false;
    }
    if !no_duplicate_refs(&t.visited_refs) {
        return false;
    }
    true
}

// r[impl ucan-proof-traversal.dag-transcript-consistency]
pub fn dag_parent_indices_well_formed(t: &TraversalDagTranscript) -> (result: bool)
    ensures result == dag_parent_indices_well_formed_spec(t),
{
    if t.entries.len() == 0 {
        return false;
    }
    if t.entries.len() != t.parent_indices.len() {
        return false;
    }
    let mut child: usize = 0;
    while child < t.parent_indices.len()
        invariant
            0 <= child <= t.parent_indices.len(),
            t.entries.len() == t.parent_indices.len(),
            t.entries.len() > 0,
            forall|c: int| #![trigger t.parent_indices@[c]] 0 <= c < child ==>
                forall|slot: int| #![trigger t.parent_indices@[c][slot]] 0 <= slot < t.parent_indices@[c].len() ==>
                    (t.parent_indices@[c][slot] as int) < c,
        decreases t.parent_indices.len() - child,
    {
        let mut slot: usize = 0;
        while slot < t.parent_indices[child].len()
            invariant
                0 <= slot <= t.parent_indices@[child as int].len(),
                0 <= child < t.parent_indices.len(),
                forall|s: int| #![trigger t.parent_indices@[child as int][s]] 0 <= s < slot ==>
                    (t.parent_indices@[child as int][s] as int) < child,
            decreases t.parent_indices@[child as int].len() - slot,
        {
            if t.parent_indices[child][slot] >= child {
                return false;
            }
            slot = slot + 1;
        }
        child = child + 1;
    }
    true
}

// r[impl ucan-proof-traversal.dag-transcript-consistency]
pub fn parent_link_well_formed(parent: &TraversalEntry, child: &TraversalEntry) -> (result: bool)
    ensures result == parent_link_well_formed_spec(parent, child),
{
    if !bytes_equal(&parent.audience, &child.issuer) {
        return false;
    }
    if parent.nbf > child.nbf {
        return false;
    }
    if child.exp > parent.exp {
        return false;
    }
    true
}

// r[impl ucan-proof-traversal.dag-transcript-consistency]
pub fn dag_child_capability_has_parent(
    t: &TraversalDagTranscript,
    child: usize,
    capability: &TraversalCapability,
) -> (result: bool)
    requires
        child < t.parent_indices@.len(),
        forall|slot: int| #![trigger t.parent_indices@[child as int][slot]] 0 <= slot < t.parent_indices@[child as int].len() ==>
            (t.parent_indices@[child as int][slot] as int) < t.entries@.len(),
    ensures result == dag_child_capability_has_parent_spec(t, child as int, capability),
{
    let mut slot: usize = 0;
    while slot < t.parent_indices[child].len()
        invariant
            child < t.parent_indices@.len(),
            0 <= slot <= t.parent_indices@[child as int].len(),
            forall|s: int| #![trigger t.parent_indices@[child as int][s]] 0 <= s < t.parent_indices@[child as int].len() ==>
                (t.parent_indices@[child as int][s] as int) < t.entries@.len(),
            forall|s: int| #![trigger t.parent_indices@[child as int][s]] 0 <= s < slot ==>
                !capability_set_contains_child_spec(
                    t.entries@[(t.parent_indices@[child as int][s]) as int].capabilities@,
                    capability,
                ),
        decreases t.parent_indices@[child as int].len() - slot,
    {
        let parent_index = t.parent_indices[child][slot];
        if child_capability_has_parent(&t.entries[parent_index].capabilities, capability) {
            return true;
        }
        slot = slot + 1;
    }
    false
}

// r[impl ucan-proof-traversal.dag-transcript-consistency]
pub fn dag_child_capabilities_attenuate(t: &TraversalDagTranscript, child: usize) -> (result: bool)
    requires
        child < t.entries@.len(),
        child < t.parent_indices@.len(),
        forall|slot: int| #![trigger t.parent_indices@[child as int][slot]] 0 <= slot < t.parent_indices@[child as int].len() ==>
            (t.parent_indices@[child as int][slot] as int) < t.entries@.len(),
    ensures result == dag_child_capabilities_attenuate_spec(t, child as int),
{
    let mut cap: usize = 0;
    while cap < t.entries[child].capabilities.len()
        invariant
            child < t.entries@.len(),
            child < t.parent_indices@.len(),
            0 <= cap <= t.entries@[child as int].capabilities@.len(),
            forall|s: int| #![trigger t.parent_indices@[child as int][s]] 0 <= s < t.parent_indices@[child as int].len() ==>
                (t.parent_indices@[child as int][s] as int) < t.entries@.len(),
            forall|k: int| #![trigger t.entries@[child as int].capabilities@[k]] 0 <= k < cap ==>
                dag_child_capability_has_parent_spec(t, child as int, &t.entries@[child as int].capabilities@[k]),
        decreases t.entries@[child as int].capabilities@.len() - cap,
    {
        if !dag_child_capability_has_parent(t, child, &t.entries[child].capabilities[cap]) {
            return false;
        }
        cap = cap + 1;
    }
    true
}

// r[impl ucan-proof-traversal.dag-transcript-consistency]
pub fn dag_parent_caveats_preserved(t: &TraversalDagTranscript, child: usize) -> (result: bool)
    requires
        child < t.entries@.len(),
        child < t.parent_indices@.len(),
        forall|slot: int| #![trigger t.parent_indices@[child as int][slot]] 0 <= slot < t.parent_indices@[child as int].len() ==>
            (t.parent_indices@[child as int][slot] as int) < t.entries@.len(),
    ensures result == dag_parent_caveats_preserved_spec(t, child as int),
{
    let mut slot: usize = 0;
    while slot < t.parent_indices[child].len()
        invariant
            child < t.entries@.len(),
            child < t.parent_indices@.len(),
            0 <= slot <= t.parent_indices@[child as int].len(),
            forall|s: int| #![trigger t.parent_indices@[child as int][s]] 0 <= s < t.parent_indices@[child as int].len() ==>
                (t.parent_indices@[child as int][s] as int) < t.entries@.len(),
            forall|s: int| #![trigger t.parent_indices@[child as int][s]] 0 <= s < slot ==>
                parent_caveats_preserved_spec(&t.entries@[t.parent_indices@[child as int][s] as int], &t.entries@[child as int]),
        decreases t.parent_indices@[child as int].len() - slot,
    {
        let parent_index = t.parent_indices[child][slot];
        if !parent_caveats_preserved(&t.entries[parent_index], &t.entries[child]) {
            return false;
        }
        slot = slot + 1;
    }
    true
}

// r[impl ucan-proof-traversal.dag-transcript-consistency]
pub fn dag_child_edges_well_formed(t: &TraversalDagTranscript, child: usize) -> (result: bool)
    requires
        child < t.entries@.len(),
        child < t.parent_indices@.len(),
        forall|slot: int| #![trigger t.parent_indices@[child as int][slot]] 0 <= slot < t.parent_indices@[child as int].len() ==>
            (t.parent_indices@[child as int][slot] as int) < t.entries@.len(),
    ensures result == dag_child_edges_well_formed_spec(t, child as int),
{
    let mut slot: usize = 0;
    while slot < t.parent_indices[child].len()
        invariant
            child < t.entries@.len(),
            child < t.parent_indices@.len(),
            0 <= slot <= t.parent_indices@[child as int].len(),
            forall|s: int| #![trigger t.parent_indices@[child as int][s]] 0 <= s < t.parent_indices@[child as int].len() ==>
                (t.parent_indices@[child as int][s] as int) < t.entries@.len(),
            forall|s: int| #![trigger t.parent_indices@[child as int][s]] 0 <= s < slot ==>
                parent_link_well_formed_spec(&t.entries@[t.parent_indices@[child as int][s] as int], &t.entries@[child as int]),
        decreases t.parent_indices@[child as int].len() - slot,
    {
        let parent_index = t.parent_indices[child][slot];
        if !parent_link_well_formed(&t.entries[parent_index], &t.entries[child]) {
            return false;
        }
        slot = slot + 1;
    }
    if !dag_child_capabilities_attenuate(t, child) {
        return false;
    }
    if !dag_parent_caveats_preserved(t, child) {
        return false;
    }
    true
}

// r[impl ucan-proof-traversal.dag-transcript-consistency]
pub fn dag_edges_well_formed(t: &TraversalDagTranscript) -> (result: bool)
    ensures result == dag_edges_well_formed_spec(t),
{
    if !dag_parent_indices_well_formed(t) {
        return false;
    }
    let mut child: usize = 0;
    while child < t.entries.len()
        invariant
            0 <= child <= t.entries.len(),
            dag_parent_indices_well_formed_spec(t),
            forall|c: int| #![trigger t.entries@[c]] 0 <= c < child && t.parent_indices@[c].len() > 0 ==>
                dag_child_edges_well_formed_spec(t, c),
        decreases t.entries.len() - child,
    {
        if t.parent_indices[child].len() > 0 && !dag_child_edges_well_formed(t, child) {
            return false;
        }
        child = child + 1;
    }
    true
}

// r[impl ucan-proof-traversal.dag-transcript-consistency]
pub fn dag_proof_authorizes(t: &TraversalDagTranscript) -> (result: bool)
    ensures result == dag_proof_authorizes_spec(t),
{
    if t.entries.len() == 0 {
        return false;
    }
    if !dag_edges_well_formed(t) {
        return false;
    }
    if !all_entries_temporal_valid(&t.entries, &t.target) {
        return false;
    }
    let leaf = t.entries.len() - 1;
    if !capability_set_authorizes(&t.entries[leaf].capabilities, &t.target) {
        return false;
    }
    true
}

// r[impl ucan-proof-traversal.dag-transcript-consistency]
pub fn dag_not_revoked(t: &TraversalDagTranscript) -> (result: bool)
    ensures result == dag_not_revoked_spec(t),
{
    let mut i: usize = 0;
    while i < t.entries.len()
        invariant
            0 <= i <= t.entries.len(),
            forall|k: int| #![trigger t.entries@[k]] 0 <= k < i ==> !t.entries@[k].revoked,
        decreases t.entries.len() - i,
    {
        if t.entries[i].revoked {
            return false;
        }
        i = i + 1;
    }
    true
}

// r[impl ucan-proof-traversal.dag-transcript-consistency]
pub fn dag_traversal_transcript_valid(t: &TraversalDagTranscript, budget: &TraversalBudget) -> (result: bool)
    ensures result == dag_traversal_transcript_valid_spec(t, budget),
{
    if !dag_within_budget(t, budget) {
        return false;
    }
    if !dag_proof_authorizes(t) {
        return false;
    }
    if !dag_not_revoked(t) {
        return false;
    }
    true
}

// r[verify ucan-proof-traversal.byte-equality]
pub proof fn byte_equality_refl(bytes: Seq<u8>)
    ensures bytes_equal_spec(bytes, bytes),
{
}

// r[verify ucan-proof-traversal.capability-containment]
pub proof fn capability_containment_sound(parent: TraversalCapability, child: TraversalCapability)
    requires capability_contains_spec(&parent, &child),
    ensures
        child.resource@.len() >= parent.resource@.len(),
        ucan_ability::ability_contains_spec2(parent.ability, child.ability),
{
}

// r[verify ucan-proof-traversal.graph-well-formedness]
pub proof fn well_formed_edge_links(parent: TraversalEntry, child: TraversalEntry)
    requires proof_edge_well_formed_spec(&parent, &child),
    ensures
        parent.audience@ =~= child.issuer@,
        parent.nbf <= child.nbf,
        child.exp <= parent.exp,
{
}

// r[verify ucan-proof-traversal.bounded-traversal]
pub proof fn bounded_traversal_has_unique_ref_trace(t: TraversalTranscript, budget: TraversalBudget)
    requires traversal_within_budget_spec(&t, &budget),
    ensures
        t.entries@.len() > 0,
        t.entries@.len() <= budget.max_proofs,
        (t.entries@.len() - 1) <= budget.max_depth,
        entry_refs_match_visited_spec(t.entries@, t.visited_refs@),
        no_duplicate_refs_spec(t.visited_refs@),
{
}

// r[verify ucan-proof-traversal.effective-delegation-folding]
pub proof fn effective_fold_implies_well_formed_edges(t: TraversalTranscript)
    requires effective_delegation_fold_valid_spec(&t),
    ensures
        t.entries@.len() > 0,
        all_edges_well_formed_spec(t.entries@),
{
}

// r[verify ucan-proof-traversal.chain-authorization]
pub proof fn authorized_chain_has_leaf_authority(t: TraversalTranscript)
    requires proof_chain_authorizes_spec(&t),
    ensures
        t.entries@.len() > 0,
        effective_delegation_fold_valid_spec(&t),
        all_entries_temporal_valid_spec(t.entries@, &t.target),
        capability_set_authorizes_spec(t.entries@[t.entries@.len() - 1].capabilities@, &t.target),
{
}

// r[verify ucan-proof-traversal.revocation-aware-chain]
pub proof fn not_revoked_excludes_revoked_entries(t: TraversalTranscript)
    requires traversal_not_revoked_spec(&t),
    ensures forall|i: int| #![trigger t.entries@[i]] 0 <= i < t.entries@.len() ==> !t.entries@[i].revoked,
{
}

// r[verify ucan-proof-traversal.transcript-consistency]
pub proof fn valid_transcript_composes_checks(t: TraversalTranscript, budget: TraversalBudget)
    requires traversal_transcript_valid_spec(&t, &budget),
    ensures
        traversal_within_budget_spec(&t, &budget),
        proof_chain_authorizes_spec(&t),
        traversal_not_revoked_spec(&t),
{
}

// r[verify ucan-proof-traversal.dag-transcript-consistency]
pub proof fn valid_dag_transcript_composes_checks(t: TraversalDagTranscript, budget: TraversalBudget)
    requires dag_traversal_transcript_valid_spec(&t, &budget),
    ensures
        dag_within_budget_spec(&t, &budget),
        dag_proof_authorizes_spec(&t),
        dag_not_revoked_spec(&t),
{
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    fn ability(path: &[u8]) -> ucan_ability::Ability {
        ucan_ability::Ability {
            segments: path.split(|byte| *byte == b'/').map(<[u8]>::to_vec).collect(),
        }
    }

    fn cap(resource: &[u8], ability_path: &[u8]) -> TraversalCapability {
        TraversalCapability {
            resource: resource.to_vec(),
            ability: ability(ability_path),
        }
    }

    fn entry(reference: &[u8], issuer: &[u8], audience: &[u8], resource: &[u8], revoked: bool) -> TraversalEntry {
        TraversalEntry {
            proof_ref: reference.to_vec(),
            issuer: issuer.to_vec(),
            audience: audience.to_vec(),
            capabilities: vec![cap(resource, b"read")],
            caveat_ids: vec![b"geo".to_vec()],
            nbf: 10,
            exp: 100,
            signature_valid: true,
            revoked,
        }
    }

    fn transcript() -> TraversalTranscript {
        let mut parent = entry(b"root", b"alice", b"bob", b"store", false);
        parent.caveat_ids = vec![b"geo".to_vec()];
        let mut child = entry(b"leaf", b"bob", b"carol", b"store/docs", false);
        child.caveat_ids = vec![b"geo".to_vec(), b"rate".to_vec()];
        TraversalTranscript {
            entries: vec![parent, child],
            visited_refs: vec![b"root".to_vec(), b"leaf".to_vec()],
            total_input_bytes: 64,
            target: TraversalAuthorizationTarget {
                resource: b"store/docs/file".to_vec(),
                ability: ability(b"read"),
                now: 50,
                tolerance: 0,
            },
        }
    }

    fn dag_transcript() -> TraversalDagTranscript {
        let root = entry(b"root", b"alice", b"bob", b"store", false);
        let mut proof_a = entry(b"proof-a", b"bob", b"carol", b"store/docs", false);
        proof_a.caveat_ids = vec![b"geo".to_vec(), b"rate".to_vec()];
        let mut proof_b = entry(b"proof-b", b"bob", b"carol", b"store/docs", false);
        proof_b.caveat_ids = vec![b"geo".to_vec(), b"rate".to_vec(), b"region".to_vec()];
        let mut leaf = entry(b"leaf", b"carol", b"dave", b"store/docs/reports", false);
        leaf.caveat_ids = vec![b"geo".to_vec(), b"rate".to_vec(), b"region".to_vec(), b"ttl".to_vec()];
        TraversalDagTranscript {
            entries: vec![root, proof_a, proof_b, leaf],
            parent_indices: vec![vec![], vec![0], vec![0], vec![1, 2]],
            visited_refs: vec![
                b"root".to_vec(),
                b"proof-a".to_vec(),
                b"proof-b".to_vec(),
                b"leaf".to_vec(),
            ],
            total_input_bytes: 128,
            target: TraversalAuthorizationTarget {
                resource: b"store/docs/reports/q1".to_vec(),
                ability: ability(b"read"),
                now: 50,
                tolerance: 0,
            },
        }
    }

    #[test]
    fn valid_transcript_accepts() {
        let t = transcript();
        let b = TraversalBudget {
            max_depth: 4,
            max_proofs: 8,
            max_input_bytes: 1024,
        };
        assert!(traversal_transcript_valid(&t, &b));
    }

    #[test]
    fn valid_dag_transcript_accepts_and_rejects_bad_edges() {
        let b = TraversalBudget {
            max_depth: 4,
            max_proofs: 8,
            max_input_bytes: 1024,
        };
        let t = dag_transcript();
        assert!(dag_traversal_transcript_valid(&t, &b));

        let mut cycle = dag_transcript();
        cycle.parent_indices[1] = vec![1];
        assert!(!dag_traversal_transcript_valid(&cycle, &b));

        let mut missing_parent_caveat = dag_transcript();
        missing_parent_caveat.entries[3].caveat_ids.retain(|id| id != b"region");
        assert!(!dag_traversal_transcript_valid(&missing_parent_caveat, &b));

        let mut wrong_parent = dag_transcript();
        wrong_parent.entries[2].audience = b"mallory".to_vec();
        assert!(!dag_traversal_transcript_valid(&wrong_parent, &b));
    }

    #[test]
    fn wildcard_ability_transcript_accepts_and_rejects_namespace_escape() {
        let mut t = transcript();
        t.entries[0].capabilities[0] = cap(b"store", b"store/*");
        t.entries[1].capabilities[0] = cap(b"store/docs", b"store/read");
        t.target.ability = ability(b"store/read");
        let b = TraversalBudget {
            max_depth: 4,
            max_proofs: 8,
            max_input_bytes: 1024,
        };
        assert!(traversal_transcript_valid(&t, &b));

        t.entries[1].capabilities[0] = cap(b"store/docs", b"admin/read");
        t.target.ability = ability(b"admin/read");
        assert!(!traversal_transcript_valid(&t, &b));
    }

    #[test]
    fn rejects_broken_link_cycle_revocation_and_overbroad_child() {
        let b = TraversalBudget {
            max_depth: 4,
            max_proofs: 8,
            max_input_bytes: 1024,
        };

        let mut broken = transcript();
        broken.entries[1].issuer = b"mallory".to_vec();
        assert!(!traversal_transcript_valid(&broken, &b));

        let mut duplicate = transcript();
        duplicate.visited_refs[1] = b"root".to_vec();
        assert!(!traversal_transcript_valid(&duplicate, &b));

        let mut revoked = transcript();
        revoked.entries[0].revoked = true;
        assert!(!traversal_transcript_valid(&revoked, &b));

        let mut overbroad = transcript();
        overbroad.entries[1].capabilities[0] = cap(b"other", b"read");
        assert!(!traversal_transcript_valid(&overbroad, &b));
    }

    #[test]
    fn rejects_budget_and_caveat_preservation_failures() {
        let mut t = transcript();
        let tight_depth = TraversalBudget {
            max_depth: 0,
            max_proofs: 8,
            max_input_bytes: 1024,
        };
        assert!(!traversal_transcript_valid(&t, &tight_depth));

        let tight_bytes = TraversalBudget {
            max_depth: 4,
            max_proofs: 8,
            max_input_bytes: 1,
        };
        assert!(!traversal_transcript_valid(&t, &tight_bytes));

        t.entries[1].caveat_ids.clear();
        let b = TraversalBudget {
            max_depth: 4,
            max_proofs: 8,
            max_input_bytes: 1024,
        };
        assert!(!traversal_transcript_valid(&t, &b));
    }
}
