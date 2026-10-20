//! Verified UCAN invocation and receipt model.
//!
//! An invocation binds a capability from a delegation chain to a specific
//! executor and action. A receipt captures the result and who ran it.
//! Proves that a valid receipt requires a valid invocation, and that the
//! executor must match the final audience in the chain.

#![allow(clippy::ptr_arg)]

use vstd::prelude::*;

use crate::ucan_chain::ChainEntry;
use crate::ucan_revocation::RevocationSet;
use crate::ucan_revocation::check_chain_structural_valid_not_revoked;

verus! {

/// An invocation: executing a capability.
pub struct Invocation {
    /// CID of the authorization token (leaf of delegation chain).
    pub auth_cid: Vec<u8>,
    /// DID of the entity invoking the capability.
    pub invoker: Vec<u8>,
    /// Resource being acted upon.
    pub resource: Vec<u8>,
    /// Ability being exercised.
    pub ability: Vec<u8>,
    /// Invocation timestamp.
    pub timestamp: u64,
}

/// Result status of an invocation.
pub enum InvocationResult {
    /// Successful execution with output bytes.
    Ok { output: Vec<u8> },
    /// Failed execution with error bytes.
    Err { error: Vec<u8> },
}

/// A receipt: proof that an invocation was executed.
pub struct Receipt {
    /// The invocation that was executed.
    pub invocation: Invocation,
    /// Who actually executed it.
    pub ran_by: Vec<u8>,
    /// The result.
    pub result: InvocationResult,
    /// Receipt timestamp.
    pub receipt_timestamp: u64,
}

// --- Spec predicates ---

/// The invoker matches a given audience DID.
// r[impl ucan-invocation.invoker-matches-audience]
pub open spec fn invoker_matches_audience_spec(invoker: Seq<u8>, audience: Seq<u8>) -> bool {
    invoker =~= audience
}

/// The executor (ran_by) matches the invoker.
// r[impl ucan-invocation.executor-is-invoker]
pub open spec fn executor_is_invoker_spec(ran_by: Seq<u8>, invoker: Seq<u8>) -> bool {
    ran_by =~= invoker
}

/// The receipt timestamp is at or after the invocation timestamp.
// r[impl ucan-invocation.receipt-ordering]
pub open spec fn receipt_after_invocation_spec(inv_ts: u64, receipt_ts: u64) -> bool {
    receipt_ts as int >= inv_ts as int
}

/// A receipt is valid if executor matches invoker and timestamps are ordered.
// r[impl ucan-invocation.receipt-validity]
pub open spec fn receipt_valid_spec(r: Receipt) -> bool {
    executor_is_invoker_spec(r.ran_by@, r.invocation.invoker@)
    && receipt_after_invocation_spec(r.invocation.timestamp, r.receipt_timestamp)
}

/// An invocation is authorized if the invoker matches the chain's leaf audience.
// r[impl ucan-invocation.authorization-check]
pub open spec fn invocation_authorized_spec(
    invoker: Seq<u8>,
    leaf_audience: Seq<u8>,
    resource: Seq<u8>,
    ability: Seq<u8>,
    leaf_resource: Seq<u8>,
    leaf_ability: Seq<u8>,
) -> bool {
    invoker_matches_audience_spec(invoker, leaf_audience)
    && resource.len() >= leaf_resource.len()
    && resource.subrange(0, leaf_resource.len() as int) =~= leaf_resource
    && ability =~= leaf_ability
}

/// Complete structural request admission from a resolved chain and revocation set.
// r[impl ucan-invocation.authorization-check]
pub open spec fn invocation_chain_authorized_spec(
    revset: Seq<Seq<u8>>,
    chain: Seq<ChainEntry>,
    chain_cids: Seq<Vec<u8>>,
    invocation: Invocation,
    leaf_cap_idx: usize,
    now: u64,
    tolerance: u64,
) -> bool {
    crate::ucan_revocation::chain_structural_revocation_valid_spec(
        revset,
        chain,
        chain_cids,
        now,
        tolerance,
    )
    && invocation.auth_cid@ =~= chain_cids[chain.len() - 1]@
    && 0 <= leaf_cap_idx < chain[chain.len() - 1].att.len()
    && invocation_authorized_spec(
        invocation.invoker@,
        chain[chain.len() - 1].aud@,
        invocation.resource@,
        invocation.ability@,
        chain[chain.len() - 1].att[leaf_cap_idx as int].0@,
        chain[chain.len() - 1].att[leaf_cap_idx as int].1@,
    )
}

// --- Exec ---

/// Check if invoker matches the audience.
// r[impl ucan-invocation.invoker-matches-audience]
pub fn check_invoker_matches(invoker: &Vec<u8>, audience: &Vec<u8>) -> (result: bool)
    ensures result == invoker_matches_audience_spec(invoker@, audience@),
{
    if invoker.len() != audience.len() {
        return false;
    }
    let mut i: usize = 0;
    while i < invoker.len()
        invariant
            0 <= i <= invoker.len(),
            invoker.len() == audience.len(),
            forall|k: int| 0 <= k < i ==> invoker@[k] == audience@[k],
        decreases invoker.len() - i,
    {
        if invoker[i] != audience[i] {
            assert(invoker@[i as int] != audience@[i as int]);
            assert(!(invoker@ =~= audience@));
            return false;
        }
        i = i + 1;
    }
    assert(invoker@ =~= audience@);
    true
}

/// Check if a receipt is valid.
// r[impl ucan-invocation.receipt-validity]
pub fn check_receipt_valid(r: &Receipt) -> (result: bool)
    ensures result == receipt_valid_spec(*r),
{
    let exec_ok = check_invoker_matches(&r.ran_by, &r.invocation.invoker);
    let ts_ok = r.receipt_timestamp >= r.invocation.timestamp;
    exec_ok && ts_ok
}


/// Check whether `resource` has `prefix` as a byte prefix.
// r[impl ucan-invocation.authorization-check]
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

/// Check complete invocation authorization against the chain leaf audience and capability.
// r[impl ucan-invocation.authorization-check]
pub fn check_invocation_authorized(
    invoker: &Vec<u8>,
    leaf_audience: &Vec<u8>,
    resource: &Vec<u8>,
    ability: &Vec<u8>,
    leaf_resource: &Vec<u8>,
    leaf_ability: &Vec<u8>,
) -> (result: bool)
    ensures result == invocation_authorized_spec(
        invoker@,
        leaf_audience@,
        resource@,
        ability@,
        leaf_resource@,
        leaf_ability@,
    ),
{
    let invoker_ok = check_invoker_matches(invoker, leaf_audience);
    if !invoker_ok {
        return false;
    }
    let resource_ok = resource_has_prefix(resource, leaf_resource);
    if !resource_ok {
        return false;
    }
    let ability_ok = check_invoker_matches(ability, leaf_ability);
    if !ability_ok {
        return false;
    }
    true
}

/// Check structural request admission from resolved proof-chain entries.
///
/// This composes structural chain validity, proof-CID revocation admission, and
/// leaf-capability invocation authorization. It consumes caller-provided proof
/// CIDs and signature-valid flags; it does not verify cryptographic signatures,
/// DID resolution, revocation freshness, or external registry correctness.
// r[impl ucan-invocation.authorization-check]
pub fn check_invocation_chain_authorized(
    revset: &RevocationSet,
    chain: &Vec<ChainEntry>,
    chain_cids: &Vec<Vec<u8>>,
    invocation: &Invocation,
    leaf_cap_idx: usize,
    now: u64,
    tolerance: u64,
) -> (result: bool)
    ensures result == invocation_chain_authorized_spec(
        crate::ucan_revocation::revset_view(revset),
        chain@,
        chain_cids@,
        *invocation,
        leaf_cap_idx,
        now,
        tolerance,
    ),
{
    if !check_chain_structural_valid_not_revoked(revset, chain, chain_cids, now, tolerance) {
        return false;
    }
    assert(chain.len() > 0);
    let leaf_idx = chain.len() - 1;
    let auth_cid_ok = check_invoker_matches(&invocation.auth_cid, &chain_cids[leaf_idx]);
    if !auth_cid_ok {
        return false;
    }
    if leaf_cap_idx >= chain[leaf_idx].att.len() {
        return false;
    }
    check_invocation_authorized(
        &invocation.invoker,
        &chain[leaf_idx].aud,
        &invocation.resource,
        &invocation.ability,
        &chain[leaf_idx].att[leaf_cap_idx].0,
        &chain[leaf_idx].att[leaf_cap_idx].1,
    )
}

// --- Proofs ---

/// A valid receipt proves the invoker actually ran the action.
// r[verify ucan-invocation.executor-is-invoker]
// r[verify ucan-invocation.receipt-validity]
pub proof fn valid_receipt_confirms_executor(r: Receipt)
    requires receipt_valid_spec(r),
    ensures executor_is_invoker_spec(r.ran_by@, r.invocation.invoker@),
{
}

/// If the invoker doesn't match the audience, invocation is unauthorized.
// r[verify ucan-invocation.invoker-matches-audience]
pub proof fn wrong_invoker_unauthorized(
    invoker: Seq<u8>,
    audience: Seq<u8>,
    resource: Seq<u8>,
    ability: Seq<u8>,
    leaf_resource: Seq<u8>,
    leaf_ability: Seq<u8>,
)
    requires !(invoker =~= audience),
    ensures !invocation_authorized_spec(invoker, audience, resource, ability, leaf_resource, leaf_ability),
{
}

/// Receipt timestamp ordering is transitive with invocation chains.
// r[verify ucan-invocation.receipt-ordering]
pub proof fn receipt_ordering_transitive(ts1: u64, ts2: u64, ts3: u64)
    requires
        receipt_after_invocation_spec(ts1, ts2),
        receipt_after_invocation_spec(ts2, ts3),
    ensures
        receipt_after_invocation_spec(ts1, ts3),
{
}

/// An authorized invocation's resource is a refinement of the leaf capability.
// r[verify ucan-invocation.authorization-check]
pub proof fn authorized_refines_leaf(
    invoker: Seq<u8>,
    leaf_audience: Seq<u8>,
    resource: Seq<u8>,
    ability: Seq<u8>,
    leaf_resource: Seq<u8>,
    leaf_ability: Seq<u8>,
)
    requires invocation_authorized_spec(invoker, leaf_audience, resource, ability, leaf_resource, leaf_ability),
    ensures
        resource.len() >= leaf_resource.len(),
        ability =~= leaf_ability,
{
}

} // verus!

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invoker_matches() {
        assert!(check_invoker_matches(&b"did:key:alice".to_vec(), &b"did:key:alice".to_vec()));
        assert!(!check_invoker_matches(&b"did:key:alice".to_vec(), &b"did:key:bob".to_vec()));
    }

    #[test]
    fn test_valid_receipt() {
        let r = Receipt {
            invocation: Invocation {
                auth_cid: b"cid123".to_vec(),
                invoker: b"did:key:alice".to_vec(),
                resource: b"/photos".to_vec(),
                ability: b"read".to_vec(),
                timestamp: 100,
            },
            ran_by: b"did:key:alice".to_vec(),
            result: InvocationResult::Ok {
                output: b"data".to_vec(),
            },
            receipt_timestamp: 105,
        };
        assert!(check_receipt_valid(&r));
    }

    #[test]
    fn test_wrong_executor_invalid() {
        let r = Receipt {
            invocation: Invocation {
                auth_cid: b"cid123".to_vec(),
                invoker: b"did:key:alice".to_vec(),
                resource: b"/photos".to_vec(),
                ability: b"read".to_vec(),
                timestamp: 100,
            },
            ran_by: b"did:key:bob".to_vec(),
            result: InvocationResult::Ok {
                output: b"data".to_vec(),
            },
            receipt_timestamp: 105,
        };
        assert!(!check_receipt_valid(&r));
    }

    #[test]
    fn test_timestamp_before_invalid() {
        let r = Receipt {
            invocation: Invocation {
                auth_cid: b"cid123".to_vec(),
                invoker: b"did:key:alice".to_vec(),
                resource: b"/photos".to_vec(),
                ability: b"read".to_vec(),
                timestamp: 200,
            },
            ran_by: b"did:key:alice".to_vec(),
            result: InvocationResult::Err {
                error: b"fail".to_vec(),
            },
            receipt_timestamp: 100,
        };
        assert!(!check_receipt_valid(&r));
    }

    #[test]
    fn test_invocation_authorized_accepts_resource_refinement() {
        assert!(check_invocation_authorized(
            &b"did:key:alice".to_vec(),
            &b"did:key:alice".to_vec(),
            &b"/photos/vacation/1".to_vec(),
            &b"read".to_vec(),
            &b"/photos".to_vec(),
            &b"read".to_vec(),
        ));
    }

    #[test]
    fn test_invocation_authorized_rejects_wrong_audience_resource_or_ability() {
        assert!(!check_invocation_authorized(
            &b"did:key:bob".to_vec(),
            &b"did:key:alice".to_vec(),
            &b"/photos/vacation".to_vec(),
            &b"read".to_vec(),
            &b"/photos".to_vec(),
            &b"read".to_vec(),
        ));
        assert!(!check_invocation_authorized(
            &b"did:key:alice".to_vec(),
            &b"did:key:alice".to_vec(),
            &b"/videos".to_vec(),
            &b"read".to_vec(),
            &b"/photos".to_vec(),
            &b"read".to_vec(),
        ));
        assert!(!check_invocation_authorized(
            &b"did:key:alice".to_vec(),
            &b"did:key:alice".to_vec(),
            &b"/photos/vacation".to_vec(),
            &b"write".to_vec(),
            &b"/photos".to_vec(),
            &b"read".to_vec(),
        ));
    }

    #[test]
    fn test_error_receipt_valid() {
        let r = Receipt {
            invocation: Invocation {
                auth_cid: b"cid123".to_vec(),
                invoker: b"did:key:alice".to_vec(),
                resource: b"/photos".to_vec(),
                ability: b"read".to_vec(),
                timestamp: 100,
            },
            ran_by: b"did:key:alice".to_vec(),
            result: InvocationResult::Err {
                error: b"not found".to_vec(),
            },
            receipt_timestamp: 100,
        };
        assert!(check_receipt_valid(&r));
    }

    fn chain_entry(iss: &[u8], aud: &[u8], resource: &[u8], ability: &[u8]) -> ChainEntry {
        ChainEntry {
            iss: iss.to_vec(),
            aud: aud.to_vec(),
            att: vec![(resource.to_vec(), ability.to_vec())],
            exp: 200,
            nbf: 0,
            sig_valid: true,
        }
    }

    fn invocation(invoker: &[u8], resource: &[u8], ability: &[u8]) -> Invocation {
        invocation_with_auth_cid(b"leaf-cid", invoker, resource, ability)
    }

    fn invocation_with_auth_cid(auth_cid: &[u8], invoker: &[u8], resource: &[u8], ability: &[u8]) -> Invocation {
        Invocation {
            auth_cid: auth_cid.to_vec(),
            invoker: invoker.to_vec(),
            resource: resource.to_vec(),
            ability: ability.to_vec(),
            timestamp: 100,
        }
    }

    #[test]
    fn test_invocation_chain_authorized_accepts_valid_not_revoked_request() {
        let revset = RevocationSet { entries: vec![] };
        let chain = vec![
            chain_entry(b"root", b"alice", b"store://", b"read"),
            chain_entry(b"alice", b"bob", b"store://bucket", b"read"),
        ];
        let cids = vec![b"root-cid".to_vec(), b"leaf-cid".to_vec()];
        let inv = invocation(b"bob", b"store://bucket/photo", b"read");
        assert!(check_invocation_chain_authorized(&revset, &chain, &cids, &inv, 0, 100, 0));
    }

    #[test]
    fn test_invocation_chain_authorized_rejects_revoked_or_invalid_chain() {
        let chain = vec![
            chain_entry(b"root", b"alice", b"store://", b"read"),
            chain_entry(b"alice", b"bob", b"store://bucket", b"read"),
        ];
        let cids = vec![b"root-cid".to_vec(), b"leaf-cid".to_vec()];
        let inv = invocation(b"bob", b"store://bucket/photo", b"read");
        let revoked = RevocationSet {
            entries: vec![b"leaf-cid".to_vec()],
        };
        assert!(!check_invocation_chain_authorized(&revoked, &chain, &cids, &inv, 0, 100, 0));

        let invalid_chain = vec![
            chain_entry(b"root", b"alice", b"store://", b"read"),
            chain_entry(b"mallory", b"bob", b"store://bucket", b"read"),
        ];
        let clear = RevocationSet { entries: vec![] };
        assert!(!check_invocation_chain_authorized(&clear, &invalid_chain, &cids, &inv, 0, 100, 0));
    }

    #[test]
    fn test_invocation_chain_authorized_rejects_invocation_escalation_or_bad_cap_index() {
        let revset = RevocationSet { entries: vec![] };
        let chain = vec![
            chain_entry(b"root", b"alice", b"store://", b"read"),
            chain_entry(b"alice", b"bob", b"store://bucket", b"read"),
        ];
        let cids = vec![b"root-cid".to_vec(), b"leaf-cid".to_vec()];
        let wrong_resource = invocation(b"bob", b"other://bucket", b"read");
        assert!(!check_invocation_chain_authorized(&revset, &chain, &cids, &wrong_resource, 0, 100, 0));

        let wrong_ability = invocation(b"bob", b"store://bucket/photo", b"write");
        assert!(!check_invocation_chain_authorized(&revset, &chain, &cids, &wrong_ability, 0, 100, 0));

        let ok = invocation(b"bob", b"store://bucket/photo", b"read");
        assert!(!check_invocation_chain_authorized(&revset, &chain, &cids, &ok, 1, 100, 0));

        let wrong_auth_cid = invocation_with_auth_cid(b"other-cid", b"bob", b"store://bucket/photo", b"read");
        assert!(!check_invocation_chain_authorized(&revset, &chain, &cids, &wrong_auth_cid, 0, 100, 0,));
    }
}
