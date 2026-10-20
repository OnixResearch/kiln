//! Verified UCAN revocation checking.
//!
//! Models a CID-keyed revocation set that integrates into the validation
//! pipeline. Proves that revoking a CID invalidates any chain containing
//! that token, and that revocation is monotone (adding revocations never
//! re-enables previously revoked tokens).

#![allow(clippy::ptr_arg)]

use vstd::prelude::*;

use crate::ucan_chain::ChainEntry;
use crate::ucan_chain::check_chain_structural_valid;

verus! {

/// A revocation set: CID bytes that have been revoked.
/// Modeled as a sequence of revoked CID byte sequences.
pub struct RevocationSet {
    pub entries: Vec<Vec<u8>>,
}

// --- Spec: revocation predicates ---

/// Check if a specific CID is in the revocation set.
// r[impl ucan-revocation.cid-lookup]
pub open spec fn is_revoked_spec(revset: Seq<Seq<u8>>, cid: Seq<u8>) -> bool {
    exists|i: int| 0 <= i < revset.len() && #[trigger] revset[i] =~= cid
}

/// Helper: CID matches at a specific index.
pub open spec fn revoked_at_spec(revset: Seq<Seq<u8>>, cid: Seq<u8>, idx: int) -> bool {
    0 <= idx < revset.len() && revset[idx] =~= cid
}

/// A chain is revocation-free if none of its proof CIDs appear in the revocation set.
// r[impl ucan-revocation.chain-integration]
pub open spec fn chain_not_revoked_spec(
    revset: Seq<Seq<u8>>,
    chain_cids: Seq<Seq<u8>>,
) -> bool {
    forall|i: int| #![trigger chain_cids[i]]
        0 <= i < chain_cids.len() ==> !is_revoked_spec(revset, chain_cids[i])
}

/// Revocation set view helper.
pub open spec fn revset_view(rs: &RevocationSet) -> Seq<Seq<u8>> {
    rs.entries@.map_values(|e: Vec<u8>| e@)
}

/// Superset relation: b contains all entries of a.
// r[impl ucan-revocation.monotonicity]
pub open spec fn revset_superset_spec(a: Seq<Seq<u8>>, b: Seq<Seq<u8>>) -> bool {
    forall|i: int| #![trigger a[i]]
        0 <= i < a.len() ==>
        exists|j: int| 0 <= j < b.len() && #[trigger] b[j] =~= a[i]
}

/// Exec-facing revocation-free predicate over caller-provided CID bytes.
// r[impl ucan-revocation.chain-integration]
pub open spec fn chain_not_revoked_entries_spec(
    revset: Seq<Seq<u8>>,
    chain_cids: Seq<Vec<u8>>,
) -> bool {
    forall|i: int| #![trigger chain_cids[i]]
        0 <= i < chain_cids.len() ==> !is_revoked_spec(revset, chain_cids[i]@)
}

/// Structural chain validation plus CID revocation admission.
// r[impl ucan-revocation.chain-integration]
pub open spec fn chain_structural_revocation_valid_spec(
    revset: Seq<Seq<u8>>,
    chain: Seq<ChainEntry>,
    chain_cids: Seq<Vec<u8>>,
    now: u64,
    tolerance: u64,
) -> bool {
    chain_cids.len() == chain.len()
    && crate::ucan_chain::chain_structural_valid_entries_spec(chain, now, tolerance)
    && chain_not_revoked_entries_spec(revset, chain_cids)
}

// --- Exec: revocation check ---

/// Check if a CID is in the revocation set.
// r[impl ucan-revocation.cid-lookup]
pub fn is_revoked(revset: &RevocationSet, cid: &Vec<u8>) -> (result: bool)
    ensures result == is_revoked_spec(revset_view(revset), cid@),
{
    let mut i: usize = 0;
    while i < revset.entries.len()
        invariant
            0 <= i <= revset.entries.len(),
            forall|k: int| #![trigger revset.entries@[k]]
                0 <= k < i ==> !(revset.entries[k]@ =~= cid@),
        decreases revset.entries.len() - i,
    {
        let entry = &revset.entries[i];
        if entry.len() == cid.len() {
            let mut j: usize = 0;
            let mut mismatch = false;
            while j < entry.len()
                invariant
                    0 <= j <= entry.len(),
                    entry.len() == cid.len(),
                    entry@ === revset.entries@[i as int]@,
                    !mismatch ==> forall|k: int| 0 <= k < j ==> entry@[k] == cid@[k],
                    mismatch ==> !(entry@ =~= cid@),
                decreases entry.len() - j,
            {
                if entry[j] != cid[j] {
                    assert(entry@[j as int] != cid@[j as int]);
                    assert(!(entry@ =~= cid@));
                    mismatch = true;
                }
                j = j + 1;
            }
            if !mismatch {
                assert(entry@ =~= cid@);
                assert(revset_view(revset)[i as int] =~= cid@);
                return true;
            }
        }
        i = i + 1;
    }
    // Need to show !is_revoked_spec(...)
    assert forall|k: int| 0 <= k < revset_view(revset).len()
    implies !(#[trigger] revset_view(revset)[k] =~= cid@) by {
        assert(revset.entries@[k].view() === revset_view(revset)[k]);
    }
    false
}

/// Check that no CID in the chain is revoked.
// r[impl ucan-revocation.chain-integration]
pub fn check_chain_not_revoked(revset: &RevocationSet, chain_cids: &Vec<Vec<u8>>) -> (result: bool)
    ensures
        result ==> chain_not_revoked_entries_spec(revset_view(revset), chain_cids@),
        !result ==> exists|k: int| #![trigger chain_cids@[k]]
            0 <= k < chain_cids@.len()
            && is_revoked_spec(revset_view(revset), chain_cids@[k]@),
{
    let mut i: usize = 0;
    while i < chain_cids.len()
        invariant
            0 <= i <= chain_cids.len(),
            forall|k: int| #![trigger chain_cids@[k]]
                0 <= k < i ==>
                !is_revoked_spec(revset_view(revset), chain_cids@[k]@),
        decreases chain_cids.len() - i,
    {
        if is_revoked(revset, &chain_cids[i]) {
            return false;
        }
        i = i + 1;
    }
    true
}

/// Check structural chain validity and CID revocation admission together.
///
/// This composes the finite structural chain checker with a caller-provided list
/// of resolved proof CIDs. The checker rejects if the CID vector does not align
/// one-to-one with the chain entries or if any CID is present in the revocation
/// set. It does not claim revocation-list freshness or external registry
/// correctness.
// r[impl ucan-revocation.chain-integration]
pub fn check_chain_structural_valid_not_revoked(
    revset: &RevocationSet,
    chain: &Vec<ChainEntry>,
    chain_cids: &Vec<Vec<u8>>,
    now: u64,
    tolerance: u64,
) -> (result: bool)
    ensures result == chain_structural_revocation_valid_spec(
        revset_view(revset),
        chain@,
        chain_cids@,
        now,
        tolerance,
    ),
{
    if chain_cids.len() != chain.len() {
        return false;
    }
    if !check_chain_structural_valid(chain, now, tolerance) {
        return false;
    }
    if !check_chain_not_revoked(revset, chain_cids) {
        return false;
    }
    true
}

// --- Proofs ---

/// Revoking a CID means any chain containing it is not revocation-free.
// r[verify ucan-revocation.cid-lookup]
pub proof fn revoked_cid_blocks_chain(
    revset: Seq<Seq<u8>>,
    chain_cids: Seq<Seq<u8>>,
    bad_idx: int,
)
    requires
        0 <= bad_idx < chain_cids.len(),
        is_revoked_spec(revset, chain_cids[bad_idx]),
    ensures
        !chain_not_revoked_spec(revset, chain_cids),
{
    // chain_not_revoked requires !is_revoked for all, but bad_idx is revoked
}

/// Monotonicity: adding entries to the revocation set never un-revokes a CID.
// r[verify ucan-revocation.monotonicity]
pub proof fn revocation_monotone(
    small: Seq<Seq<u8>>,
    big: Seq<Seq<u8>>,
    cid: Seq<u8>,
)
    requires
        revset_superset_spec(small, big),
        is_revoked_spec(small, cid),
    ensures
        is_revoked_spec(big, cid),
{
    // is_revoked_spec(small, cid) gives us some i where small[i] =~= cid
    // revset_superset_spec gives us some j where big[j] =~= small[i]
    // transitivity of =~= gives big[j] =~= cid
    let i = choose|i: int| 0 <= i < small.len() && small[i] =~= cid;
    assert(small[i] =~= cid);
    let j = choose|j: int| 0 <= j < big.len() && big[j] =~= small[i];
    assert(big[j] =~= cid);
}

/// An empty revocation set revokes nothing.
// r[verify ucan-revocation.chain-integration]
pub proof fn empty_revset_revokes_nothing(cid: Seq<u8>)
    ensures !is_revoked_spec(Seq::<Seq<u8>>::empty(), cid),
{
}

/// Extending a revocation set with a new CID revokes exactly that CID
/// (in addition to everything already revoked).
pub proof fn extend_revokes_new(revset: Seq<Seq<u8>>, new_cid: Seq<u8>)
    ensures is_revoked_spec(revset.push(new_cid), new_cid),
{
    let ext = revset.push(new_cid);
    assert(ext[revset.len() as int] =~= new_cid);
}

/// Extending preserves existing revocations.
pub proof fn extend_preserves_revoked(revset: Seq<Seq<u8>>, new_cid: Seq<u8>, old_cid: Seq<u8>)
    requires is_revoked_spec(revset, old_cid),
    ensures is_revoked_spec(revset.push(new_cid), old_cid),
{
    let i = choose|i: int| 0 <= i < revset.len() && revset[i] =~= old_cid;
    let ext = revset.push(new_cid);
    assert(ext[i] =~= old_cid);
}

} // verus!

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    fn make_revset(cids: &[&[u8]]) -> RevocationSet {
        RevocationSet {
            entries: cids.iter().map(|c| c.to_vec()).collect(),
        }
    }

    #[test]
    fn test_empty_revset() {
        let rs = make_revset(&[]);
        assert!(!is_revoked(&rs, &b"cid1".to_vec()));
    }

    #[test]
    fn test_revoked_cid_found() {
        let rs = make_revset(&[b"cid1", b"cid2", b"cid3"]);
        assert!(is_revoked(&rs, &b"cid2".to_vec()));
    }

    #[test]
    fn test_unrevoked_cid_not_found() {
        let rs = make_revset(&[b"cid1", b"cid2"]);
        assert!(!is_revoked(&rs, &b"cid3".to_vec()));
    }

    #[test]
    fn test_chain_all_clear() {
        let rs = make_revset(&[b"revoked1"]);
        let chain = vec![b"cid1".to_vec(), b"cid2".to_vec()];
        assert!(check_chain_not_revoked(&rs, &chain));
    }

    #[test]
    fn test_chain_has_revoked() {
        let rs = make_revset(&[b"cid2"]);
        let chain = vec![b"cid1".to_vec(), b"cid2".to_vec(), b"cid3".to_vec()];
        assert!(!check_chain_not_revoked(&rs, &chain));
    }

    #[test]
    fn test_extend_revokes() {
        let rs = make_revset(&[b"cid1"]);
        assert!(!is_revoked(&rs, &b"cid2".to_vec()));
        let mut rs2 = rs;
        rs2.entries.push(b"cid2".to_vec());
        assert!(is_revoked(&rs2, &b"cid2".to_vec()));
        // cid1 still revoked
        assert!(is_revoked(&rs2, &b"cid1".to_vec()));
    }

    fn entry(iss: &[u8], aud: &[u8], resource: &[u8], ability: &[u8]) -> ChainEntry {
        ChainEntry {
            iss: iss.to_vec(),
            aud: aud.to_vec(),
            att: vec![(resource.to_vec(), ability.to_vec())],
            exp: 200,
            nbf: 0,
            sig_valid: true,
        }
    }

    #[test]
    fn test_structural_chain_not_revoked_accepts_valid_clear_chain() {
        let rs = make_revset(&[b"revoked"]);
        let chain = vec![
            entry(b"root", b"alice", b"store://", b"read"),
            entry(b"alice", b"bob", b"store://bucket", b"read"),
        ];
        let cids = vec![b"root-cid".to_vec(), b"leaf-cid".to_vec()];
        assert!(check_chain_structural_valid_not_revoked(&rs, &chain, &cids, 100, 0));
    }

    #[test]
    fn test_structural_chain_not_revoked_rejects_revoked_cid() {
        let rs = make_revset(&[b"leaf-cid"]);
        let chain = vec![
            entry(b"root", b"alice", b"store://", b"read"),
            entry(b"alice", b"bob", b"store://bucket", b"read"),
        ];
        let cids = vec![b"root-cid".to_vec(), b"leaf-cid".to_vec()];
        assert!(!check_chain_structural_valid_not_revoked(&rs, &chain, &cids, 100, 0));
    }

    #[test]
    fn test_structural_chain_not_revoked_rejects_cid_length_mismatch() {
        let rs = make_revset(&[]);
        let chain = vec![entry(b"root", b"alice", b"store://", b"read")];
        let cids = vec![];
        assert!(!check_chain_structural_valid_not_revoked(&rs, &chain, &cids, 100, 0));
    }

    #[test]
    fn test_structural_chain_not_revoked_rejects_invalid_chain() {
        let rs = make_revset(&[]);
        let chain = vec![
            entry(b"root", b"alice", b"store://", b"read"),
            entry(b"mallory", b"bob", b"store://bucket", b"read"),
        ];
        let cids = vec![b"root-cid".to_vec(), b"leaf-cid".to_vec()];
        assert!(!check_chain_structural_valid_not_revoked(&rs, &chain, &cids, 100, 0));
    }
}
