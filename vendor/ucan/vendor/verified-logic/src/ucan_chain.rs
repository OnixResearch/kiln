//! Verified UCAN proof chain walk.
//!
//! Walks resolved proof-chain entries, consumes caller-provided signature-valid
//! flags, checks iss[child] == aud[parent], confirms attenuation narrows at
//! every step, and proves chain-level soundness: a structurally valid chain only
//! grants capabilities the root token authorized.

#![allow(clippy::ptr_arg)]

use vstd::prelude::*;

verus! {

/// A resolved proof chain entry: the token at that CID plus its public key.
pub struct ChainEntry {
    /// Issuer DID bytes.
    pub iss: Vec<u8>,
    /// Audience DID bytes.
    pub aud: Vec<u8>,
    /// Attenuations granted.
    pub att: Vec<(Vec<u8>, Vec<u8>)>,
    /// Expiration timestamp.
    pub exp: u64,
    /// Not-before timestamp.
    pub nbf: u64,
    /// Caller-provided indication that the signature was verified externally.
    pub sig_valid: bool,
}

// --- Spec: chain validity predicates ---

/// Each hop's issuer matches the previous hop's audience.
// r[impl ucan-chain.issuer-audience-link]
pub open spec fn chain_linked_spec(chain: Seq<ChainEntry>) -> bool {
    forall|i: int| 0 <= i < chain.len() - 1 ==>
        chain[i].aud@ =~= chain[i + 1].iss@
}

/// Every entry in the chain carries a caller-provided valid-signature flag.
// r[impl ucan-chain.signature-check]
pub open spec fn chain_signatures_valid_spec(chain: Seq<ChainEntry>) -> bool {
    forall|i: int| 0 <= i < chain.len() ==> chain[i].sig_valid
}

/// Capability containment at each hop: child's resource is a prefix of parent's,
/// and abilities match.
// r[impl ucan-chain.attenuation-narrowing]
pub open spec fn att_entry_contains_spec(
    parent: (Seq<u8>, Seq<u8>),
    child: (Seq<u8>, Seq<u8>),
) -> bool {
    // child resource starts with parent resource (prefix match)
    child.0.len() >= parent.0.len()
    && child.0.subrange(0, parent.0.len() as int) =~= parent.0
    // abilities match
    && child.1 =~= parent.1
}

/// Each child capability in att[i+1] must be contained in some parent capability in att[i].
pub open spec fn attenuation_narrows_at_spec(
    parent_att: Seq<(Seq<u8>, Seq<u8>)>,
    child_att: Seq<(Seq<u8>, Seq<u8>)>,
) -> bool {
    forall|j: int| #![trigger child_att[j]] 0 <= j < child_att.len() ==>
        exists|k: int| 0 <= k < parent_att.len()
            && #[trigger] att_entry_contains_spec(parent_att[k], child_att[j])
}

/// Helper to view Vec<(Vec<u8>, Vec<u8>)> as Seq<(Seq<u8>, Seq<u8>)>.
pub open spec fn att_view(att: &Vec<(Vec<u8>, Vec<u8>)>) -> Seq<(Seq<u8>, Seq<u8>)> {
    att@.map_values(|e: (Vec<u8>, Vec<u8>)| (e.0@, e.1@))
}

/// Attenuation narrows at every hop in the chain.
pub open spec fn chain_attenuation_narrows_spec(chain: Seq<ChainEntry>) -> bool {
    forall|i: int| #![trigger chain[i]] 0 <= i < chain.len() - 1 ==>
        attenuation_narrows_at_spec(
            att_view(&chain[i].att),
            att_view(&chain[i + 1].att),
        )
}

/// Exec-facing child attenuation containment over Vec-backed entries.
pub open spec fn child_att_has_parent_entries_spec(
    parent_att: Seq<(Vec<u8>, Vec<u8>)>,
    child: (Vec<u8>, Vec<u8>),
) -> bool {
    exists|k: int| 0 <= k < parent_att.len()
        && #[trigger] att_entry_contains_spec(
            (parent_att[k].0@, parent_att[k].1@),
            (child.0@, child.1@),
        )
}

/// Exec-facing attenuation narrowing over Vec-backed attenuation entries.
pub open spec fn attenuation_narrows_at_entries_spec(
    parent_att: Seq<(Vec<u8>, Vec<u8>)>,
    child_att: Seq<(Vec<u8>, Vec<u8>)>,
) -> bool {
    forall|j: int| #![trigger child_att[j]] 0 <= j < child_att.len() ==>
        child_att_has_parent_entries_spec(parent_att, child_att[j])
}

/// Exec-facing chain attenuation narrowing over Vec-backed entries.
pub open spec fn chain_attenuation_narrows_entries_spec(chain: Seq<ChainEntry>) -> bool {
    forall|i: int| #![trigger chain[i]] 0 <= i < chain.len() - 1 ==>
        attenuation_narrows_at_entries_spec(chain[i].att@, chain[i + 1].att@)
}

/// Temporal validity: every hop is within its validity window.
// r[impl ucan-chain.temporal-validity]
pub open spec fn chain_temporal_valid_spec(chain: Seq<ChainEntry>, now: u64, tolerance: u64) -> bool {
    forall|i: int| #![trigger chain[i]] 0 <= i < chain.len() ==> {
        let e = chain[i];
        (now as int) <= (e.exp as int) + (tolerance as int)
        && (e.nbf as int) <= (now as int) + (tolerance as int)
    }
}

/// Complete chain validity.
// r[impl ucan-chain.chain-validity]
pub open spec fn chain_valid_spec(chain: Seq<ChainEntry>, now: u64, tolerance: u64) -> bool {
    chain.len() > 0
    && chain_linked_spec(chain)
    && chain_signatures_valid_spec(chain)
    && chain_attenuation_narrows_spec(chain)
    && chain_temporal_valid_spec(chain, now, tolerance)
}

/// Exec-facing complete chain validity over Vec-backed attenuation entries.
// r[impl ucan-chain.chain-validity]
pub open spec fn chain_structural_valid_entries_spec(
    chain: Seq<ChainEntry>,
    now: u64,
    tolerance: u64,
) -> bool {
    chain.len() > 0
    && chain_linked_spec(chain)
    && chain_signatures_valid_spec(chain)
    && chain_attenuation_narrows_entries_spec(chain)
    && chain_temporal_valid_spec(chain, now, tolerance)
}

/// Root capability: the attenuations of the first (root) entry.
// r[impl ucan-chain.root-authority]
pub open spec fn root_authority_spec(chain: Seq<ChainEntry>) -> Seq<(Seq<u8>, Seq<u8>)>
    recommends chain.len() > 0,
{
    att_view(&chain[0].att)
}

/// Leaf capability: the attenuations of the last (leaf) entry.
pub open spec fn leaf_capability_spec(chain: Seq<ChainEntry>) -> Seq<(Seq<u8>, Seq<u8>)>
    recommends chain.len() > 0,
{
    att_view(&chain[chain.len() - 1 as int].att)
}

// --- Exec: chain validation ---

/// Check that a chain is linked (iss[child] == aud[parent]).
// r[impl ucan-chain.issuer-audience-link]
pub fn check_chain_linked(chain: &Vec<ChainEntry>) -> (result: bool)
    ensures result == chain_linked_spec(chain@),
{
    if chain.len() <= 1 {
        return true;
    }
    let mut i: usize = 0;
    while i < chain.len() - 1
        invariant
            0 <= i <= chain.len() - 1,
            forall|k: int| 0 <= k < i ==> chain@[k].aud@ =~= chain@[k + 1].iss@,
        decreases chain.len() - 1 - i,
    {
        if chain[i].aud.len() != chain[i + 1].iss.len() {
            assert(!(chain@[i as int].aud@ =~= chain@[(i + 1) as int].iss@));
            assert(!chain_linked_spec(chain@));
            return false;
        }
        let mut j: usize = 0;
        let aud = &chain[i].aud;
        let iss = &chain[i + 1].iss;
        while j < aud.len()
            invariant
                0 <= i < chain.len() - 1,
                0 <= j <= aud.len(),
                aud@ === chain@[i as int].aud@,
                iss@ === chain@[(i + 1) as int].iss@,
                aud.len() == iss.len(),
                forall|k: int| 0 <= k < j ==> aud@[k] == iss@[k],
            decreases aud.len() - j,
        {
            if aud[j] != iss[j] {
                assert(aud@[j as int] != iss@[j as int]);
                assert(!(aud@ =~= iss@));
                assert(!(chain@[i as int].aud@ =~= chain@[(i + 1) as int].iss@));
                assert(!chain_linked_spec(chain@));
                return false;
            }
            j = j + 1;
        }
        assert(aud@ =~= iss@);
        i = i + 1;
    }
    true
}

/// Check that all signatures in the chain are valid.
// r[impl ucan-chain.signature-check]
pub fn check_chain_signatures(chain: &Vec<ChainEntry>) -> (result: bool)
    ensures result == chain_signatures_valid_spec(chain@),
{
    let mut i: usize = 0;
    while i < chain.len()
        invariant
            0 <= i <= chain.len(),
            forall|k: int| 0 <= k < i ==> chain@[k].sig_valid,
        decreases chain.len() - i,
    {
        if !chain[i].sig_valid {
            return false;
        }
        i = i + 1;
    }
    true
}

/// Check that all entries are temporally valid.
// r[impl ucan-chain.temporal-validity]
pub fn check_chain_temporal(chain: &Vec<ChainEntry>, now: u64, tolerance: u64) -> (result: bool)
    ensures result == chain_temporal_valid_spec(chain@, now, tolerance),
{
    let mut i: usize = 0;
    while i < chain.len()
        invariant
            0 <= i <= chain.len(),
            forall|k: int| #![trigger chain@[k]] 0 <= k < i ==> {
                let e = chain@[k];
                (now as int) <= (e.exp as int) + (tolerance as int)
                && (e.nbf as int) <= (now as int) + (tolerance as int)
            },
        decreases chain.len() - i,
    {
        let e = &chain[i];
        if (now as u128) > (e.exp as u128) + (tolerance as u128) {
            return false;
        }
        if (e.nbf as u128) > (now as u128) + (tolerance as u128) {
            return false;
        }
        i = i + 1;
    }
    true
}

/// Check byte-sequence equality.
// r[impl ucan-chain.attenuation-narrowing]
pub fn bytes_equal(a: &Vec<u8>, b: &Vec<u8>) -> (result: bool)
    ensures result == (a@ =~= b@),
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

/// Check whether `resource` has `prefix` as a byte prefix.
// r[impl ucan-chain.attenuation-narrowing]
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

/// Check whether a parent attenuation contains a child attenuation.
// r[impl ucan-chain.attenuation-narrowing]
pub fn check_att_contains(parent: &(Vec<u8>, Vec<u8>), child: &(Vec<u8>, Vec<u8>)) -> (result: bool)
    ensures result == att_entry_contains_spec((parent.0@, parent.1@), (child.0@, child.1@)),
{
    let resource_ok = resource_has_prefix(&child.0, &parent.0);
    if !resource_ok {
        return false;
    }
    let ability_ok = bytes_equal(&child.1, &parent.1);
    if !ability_ok {
        return false;
    }
    true
}

/// Check that one child attenuation entry is contained by some parent entry.
// r[impl ucan-chain.attenuation-narrowing]
pub fn child_att_has_parent(parent_att: &Vec<(Vec<u8>, Vec<u8>)>, child: &(Vec<u8>, Vec<u8>)) -> (result: bool)
    ensures result == child_att_has_parent_entries_spec(parent_att@, *child),
{
    let mut k: usize = 0;
    while k < parent_att.len()
        invariant
            0 <= k <= parent_att.len(),
            forall|j: int| 0 <= j < k ==> !att_entry_contains_spec(
                (parent_att@[j].0@, parent_att@[j].1@),
                (child.0@, child.1@),
            ),
        decreases parent_att.len() - k,
    {
        if check_att_contains(&parent_att[k], child) {
            assert(att_entry_contains_spec(
                (parent_att@[k as int].0@, parent_att@[k as int].1@),
                (child.0@, child.1@),
            ));
            return true;
        }
        assert(!att_entry_contains_spec(
            (parent_att@[k as int].0@, parent_att@[k as int].1@),
            (child.0@, child.1@),
        ));
        k = k + 1;
    }
    assert forall|j: int| 0 <= j < parent_att@.len() implies
        !att_entry_contains_spec((parent_att@[j].0@, parent_att@[j].1@), (child.0@, child.1@)) by {
    }
    false
}

/// Check attenuation narrowing between adjacent chain entries.
// r[impl ucan-chain.attenuation-narrowing]
pub fn check_attenuation_narrows_at(
    parent_att: &Vec<(Vec<u8>, Vec<u8>)>,
    child_att: &Vec<(Vec<u8>, Vec<u8>)>,
) -> (result: bool)
    ensures result == attenuation_narrows_at_entries_spec(parent_att@, child_att@),
{
    let mut j: usize = 0;
    while j < child_att.len()
        invariant
            0 <= j <= child_att.len(),
            forall|i: int| #![trigger child_att@[i]] 0 <= i < j ==>
                child_att_has_parent_entries_spec(parent_att@, child_att@[i]),
        decreases child_att.len() - j,
    {
        if !child_att_has_parent(parent_att, &child_att[j]) {
            assert(!child_att_has_parent_entries_spec(parent_att@, child_att@[j as int]));
            assert(!attenuation_narrows_at_entries_spec(parent_att@, child_att@));
            return false;
        }
        assert(child_att_has_parent_entries_spec(parent_att@, child_att@[j as int]));
        assert forall|i: int| #![trigger child_att@[i]] 0 <= i < j + 1 implies
            child_att_has_parent_entries_spec(parent_att@, child_att@[i]) by {
            if i < j {
                assert(child_att@[i] == child_att@[i]);
            } else {
                assert(i == j);
            }
        }
        j = j + 1;
        assert forall|i: int| #![trigger child_att@[i]] 0 <= i < j implies
            child_att_has_parent_entries_spec(parent_att@, child_att@[i]) by {
            assert(child_att@[i] == child_att@[i]);
        };
    }
    assert forall|i: int| #![trigger child_att@[i]] 0 <= i < child_att@.len() implies
        child_att_has_parent_entries_spec(parent_att@, child_att@[i]) by {
        assert(child_att@[i] == child_att@[i]);
    }
    true
}

/// Check attenuation narrowing at every hop in a chain.
// r[impl ucan-chain.attenuation-narrowing]
pub fn check_chain_attenuation_narrows(chain: &Vec<ChainEntry>) -> (result: bool)
    ensures result == chain_attenuation_narrows_entries_spec(chain@),
{
    if chain.len() <= 1 {
        return true;
    }
    let mut i: usize = 0;
    while i < chain.len() - 1
        invariant
            0 <= i <= chain.len() - 1,
            forall|h: int| #![trigger chain@[h]] 0 <= h < i ==>
                attenuation_narrows_at_entries_spec(chain@[h].att@, chain@[h + 1].att@),
        decreases chain.len() - 1 - i,
    {
        if !check_attenuation_narrows_at(&chain[i].att, &chain[i + 1].att) {
            assert(!attenuation_narrows_at_entries_spec(chain@[i as int].att@, chain@[(i + 1) as int].att@));
            assert(!chain_attenuation_narrows_entries_spec(chain@));
            return false;
        }
        i = i + 1;
    }
    true
}

/// Check structural validity for a finite UCAN chain.
///
/// This composes linked issuer/audience checks, caller-provided signature-valid
/// booleans, structural attenuation narrowing, and bounded temporal windows. It
/// does not verify cryptographic signatures, DID resolution, revocation, or clock
/// correctness.
// r[impl ucan-chain.chain-validity]
pub fn check_chain_structural_valid(
    chain: &Vec<ChainEntry>,
    now: u64,
    tolerance: u64,
) -> (result: bool)
    ensures result == chain_structural_valid_entries_spec(chain@, now, tolerance),
{
    if chain.len() == 0 {
        return false;
    }
    if !check_chain_linked(chain) {
        return false;
    }
    if !check_chain_signatures(chain) {
        return false;
    }
    if !check_chain_attenuation_narrows(chain) {
        return false;
    }
    if !check_chain_temporal(chain, now, tolerance) {
        return false;
    }
    true
}

// --- Proofs ---

/// Soundness: a valid chain means every leaf capability is contained in root authority.
// r[verify ucan-chain.chain-validity]
pub proof fn chain_soundness(chain: Seq<ChainEntry>, now: u64, tolerance: u64)
    requires chain_valid_spec(chain, now, tolerance),
    ensures
        chain.len() > 0,
        chain_signatures_valid_spec(chain),
        chain_linked_spec(chain),
        chain_temporal_valid_spec(chain, now, tolerance),
{
}

/// A single-entry chain is trivially valid if the entry itself is valid.
// r[verify ucan-chain.root-authority]
pub proof fn single_entry_chain_valid(entry: ChainEntry, now: u64, tolerance: u64)
    requires
        entry.sig_valid,
        (now as int) <= (entry.exp as int) + (tolerance as int),
        (entry.nbf as int) <= (now as int) + (tolerance as int),
    ensures
        chain_valid_spec(seq![entry], now, tolerance),
{
    let chain = seq![entry];
    assert(chain.len() == 1);
    assert(chain_linked_spec(chain));
    assert(chain_signatures_valid_spec(chain));
    assert(chain_attenuation_narrows_spec(chain));
    assert(chain_temporal_valid_spec(chain, now, tolerance));
}

/// Extending a valid chain preserves validity when the new entry links properly.
// r[verify ucan-chain.issuer-audience-link]
pub proof fn chain_extend_preserves_link(
    chain: Seq<ChainEntry>,
    new_entry: ChainEntry,
)
    requires
        chain.len() > 0,
        chain_linked_spec(chain),
        chain[chain.len() - 1 as int].aud@ =~= new_entry.iss@,
    ensures
        chain_linked_spec(chain.push(new_entry)),
{
    let ext = chain.push(new_entry);
    assert(ext.len() == chain.len() + 1);
    assert forall|i: int| 0 <= i < ext.len() - 1
    implies ext[i].aud@ =~= ext[i + 1].iss@ by {
        if i < chain.len() as int - 1 {
            // Trigger chain_linked_spec on chain[i]
            let _ = chain[i];
            let _ = chain[i + 1];
            assert(chain[i].aud@ =~= chain[i + 1].iss@);
            assert(ext[i] === chain[i]);
            assert(ext[i + 1] === chain[i + 1]);
        } else {
            assert(i == chain.len() as int - 1);
            assert(ext[i] === chain[chain.len() as int - 1]);
            assert(ext[chain.len() as int] === new_entry);
        }
    }
}

/// A broken signature at any hop invalidates the chain.
// r[verify ucan-chain.signature-check]
pub proof fn bad_signature_rejects(chain: Seq<ChainEntry>, bad_idx: int, now: u64, tolerance: u64)
    requires
        0 <= bad_idx < chain.len(),
        !chain[bad_idx].sig_valid,
    ensures
        !chain_valid_spec(chain, now, tolerance),
{
    assert(!chain_signatures_valid_spec(chain));
}

/// Attenuation containment is transitive: if parent contains middle and middle
/// contains child, then parent contains child.
// r[verify ucan-chain.attenuation-narrowing]
pub proof fn att_containment_transitive(
    parent: (Seq<u8>, Seq<u8>),
    middle: (Seq<u8>, Seq<u8>),
    child: (Seq<u8>, Seq<u8>),
)
    requires
        att_entry_contains_spec(parent, middle),
        att_entry_contains_spec(middle, child),
    ensures
        att_entry_contains_spec(parent, child),
{
    // child resource has middle resource as prefix, middle has parent as prefix
    // so child has parent as prefix (prefix transitivity)
    assert(child.0.len() >= middle.0.len());
    assert(middle.0.len() >= parent.0.len());
    assert(child.0.len() >= parent.0.len());
    // child[0..parent.len()] == middle[0..parent.len()] == parent
    assert(child.0.subrange(0, parent.0.len() as int) =~=
           middle.0.subrange(0, parent.0.len() as int));
    // abilities match transitively
    assert(child.1 =~= middle.1);
    assert(middle.1 =~= parent.1);
}

/// Chain depth is the number of entries.
// r[verify ucan-chain.temporal-validity]
pub proof fn chain_depth_is_length(chain: Seq<ChainEntry>)
    requires chain_valid_spec(chain, 0, 0),
    ensures chain.len() > 0,
{
}

} // verus!

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(iss: &[u8], aud: &[u8], resource: &[u8], ability: &[u8], exp: u64, sig_valid: bool) -> ChainEntry {
        ChainEntry {
            iss: iss.to_vec(),
            aud: aud.to_vec(),
            att: vec![(resource.to_vec(), ability.to_vec())],
            exp,
            nbf: 0,
            sig_valid,
        }
    }

    #[test]
    fn test_single_entry_valid() {
        let e = make_entry(b"did:key:alice", b"did:key:bob", b"/photos", b"read", 1000, true);
        let chain = vec![e];
        assert!(check_chain_linked(&chain));
        assert!(check_chain_signatures(&chain));
        assert!(check_chain_temporal(&chain, 500, 0));
    }

    #[test]
    fn test_two_hop_chain_linked() {
        let e1 = make_entry(b"did:key:alice", b"did:key:bob", b"/photos", b"read", 1000, true);
        let e2 = make_entry(b"did:key:bob", b"did:key:carol", b"/photos/vacation", b"read", 900, true);
        let chain = vec![e1, e2];
        assert!(check_chain_linked(&chain));
        assert!(check_chain_signatures(&chain));
    }

    #[test]
    fn test_broken_link_rejects() {
        let e1 = make_entry(b"did:key:alice", b"did:key:bob", b"/photos", b"read", 1000, true);
        let e2 = make_entry(b"did:key:carol", b"did:key:dave", b"/photos", b"read", 900, true);
        let chain = vec![e1, e2];
        assert!(!check_chain_linked(&chain));
    }

    #[test]
    fn test_bad_signature_rejects() {
        let e1 = make_entry(b"did:key:alice", b"did:key:bob", b"/photos", b"read", 1000, true);
        let e2 = make_entry(b"did:key:bob", b"did:key:carol", b"/photos", b"read", 900, false);
        let chain = vec![e1, e2];
        assert!(!check_chain_signatures(&chain));
    }

    #[test]
    fn test_chain_attenuation_narrows_accepts_refinement() {
        let e1 = make_entry(b"did:key:alice", b"did:key:bob", b"/photos", b"read", 1000, true);
        let e2 = make_entry(b"did:key:bob", b"did:key:carol", b"/photos/vacation", b"read", 900, true);
        let chain = vec![e1, e2];
        assert!(check_chain_attenuation_narrows(&chain));
    }

    #[test]
    fn test_chain_attenuation_rejects_resource_escalation() {
        let e1 = make_entry(b"did:key:alice", b"did:key:bob", b"/photos", b"read", 1000, true);
        let e2 = make_entry(b"did:key:bob", b"did:key:carol", b"/videos", b"read", 900, true);
        let chain = vec![e1, e2];
        assert!(!check_chain_attenuation_narrows(&chain));
    }

    #[test]
    fn test_chain_attenuation_rejects_missing_parent_capability() {
        let mut e1 = make_entry(b"did:key:alice", b"did:key:bob", b"/photos", b"read", 1000, true);
        e1.att.clear();
        let e2 = make_entry(b"did:key:bob", b"did:key:carol", b"/photos/vacation", b"read", 900, true);
        let chain = vec![e1, e2];
        assert!(!check_chain_attenuation_narrows(&chain));
    }

    #[test]
    fn test_chain_attenuation_rejects_ability_escalation() {
        let e1 = make_entry(b"did:key:alice", b"did:key:bob", b"/photos", b"read", 1000, true);
        let e2 = make_entry(b"did:key:bob", b"did:key:carol", b"/photos/vacation", b"write", 900, true);
        let chain = vec![e1, e2];
        assert!(!check_chain_attenuation_narrows(&chain));
    }

    #[test]
    fn test_chain_structural_valid_accepts_complete_chain() {
        let e1 = make_entry(b"did:key:alice", b"did:key:bob", b"/photos", b"read", 1000, true);
        let e2 = make_entry(b"did:key:bob", b"did:key:carol", b"/photos/vacation", b"read", 900, true);
        let chain = vec![e1, e2];
        assert!(check_chain_structural_valid(&chain, 500, 0));
    }

    #[test]
    fn test_chain_structural_valid_rejects_empty_chain() {
        let chain = Vec::<ChainEntry>::new();
        assert!(!check_chain_structural_valid(&chain, 500, 0));
    }

    #[test]
    fn test_chain_structural_valid_rejects_broken_link() {
        let e1 = make_entry(b"did:key:alice", b"did:key:bob", b"/photos", b"read", 1000, true);
        let e2 = make_entry(b"did:key:mallory", b"did:key:carol", b"/photos/vacation", b"read", 900, true);
        let chain = vec![e1, e2];
        assert!(!check_chain_structural_valid(&chain, 500, 0));
    }

    #[test]
    fn test_chain_structural_valid_rejects_bad_signature_flag() {
        let e1 = make_entry(b"did:key:alice", b"did:key:bob", b"/photos", b"read", 1000, true);
        let e2 = make_entry(b"did:key:bob", b"did:key:carol", b"/photos/vacation", b"read", 900, false);
        let chain = vec![e1, e2];
        assert!(!check_chain_structural_valid(&chain, 500, 0));
    }

    #[test]
    fn test_chain_structural_valid_rejects_attenuation_escalation() {
        let e1 = make_entry(b"did:key:alice", b"did:key:bob", b"/photos", b"read", 1000, true);
        let e2 = make_entry(b"did:key:bob", b"did:key:carol", b"/photos/vacation", b"write", 900, true);
        let chain = vec![e1, e2];
        assert!(!check_chain_structural_valid(&chain, 500, 0));
    }

    #[test]
    fn test_chain_structural_valid_rejects_expired_entry() {
        let e = make_entry(b"did:key:alice", b"did:key:bob", b"/photos", b"read", 100, true);
        let chain = vec![e];
        assert!(!check_chain_structural_valid(&chain, 500, 0));
    }

    #[test]
    fn test_expired_chain_rejects() {
        let e = make_entry(b"did:key:alice", b"did:key:bob", b"/photos", b"read", 100, true);
        let chain = vec![e];
        assert!(!check_chain_temporal(&chain, 500, 0));
    }

    #[test]
    fn test_temporal_tolerance() {
        let e = make_entry(b"did:key:alice", b"did:key:bob", b"/photos", b"read", 100, true);
        let chain = vec![e];
        assert!(check_chain_temporal(&chain, 500, 500));
    }
}
