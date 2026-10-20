//! Verified web-of-trust chain validity.
//!
//! Models simple active `trust`-edge chains whose witnesses are finite
//! sequences of attestation indices.

#![allow(clippy::ptr_arg)]

use vstd::prelude::*;

use crate::web_of_trust_attestation::Attestation;
use crate::web_of_trust_attestation::TRUST_ROLE_TAG;
use crate::web_of_trust_attestation::attestation_active_at;
#[cfg(verus_keep_ghost)]
use crate::web_of_trust_attestation::attestation_active_at_spec;
#[cfg(verus_keep_ghost)]
use crate::web_of_trust_attestation::attestation_record_ids_match_bytes_spec;
use crate::web_of_trust_principal::PrincipalId;
use crate::web_of_trust_revocation::Revocation;

verus! {

// r[impl web-of-trust.chain-validity]
pub open spec fn root_member_from_spec(roots: Seq<PrincipalId>, principal: PrincipalId, from: int) -> bool
    decreases if from < roots.len() { roots.len() - from } else { 0 }
{
    if from >= roots.len() {
        false
    } else {
        roots[from].digest@ == principal.digest@ || root_member_from_spec(roots, principal, from + 1)
    }
}

// r[impl web-of-trust.chain-validity]
pub open spec fn root_member_spec(roots: Seq<PrincipalId>, principal: PrincipalId) -> bool {
    root_member_from_spec(roots, principal, 0)
}

// r[impl web-of-trust.chain-validity]
pub open spec fn seq_contains_bytes_spec(values: Seq<Seq<u8>>, digest: Seq<u8>) -> bool {
    exists|i: int| 0 <= i < values.len() && values[i] == digest
}

// r[impl web-of-trust.chain-validity]
pub open spec fn chain_indices_in_range_from_spec(
    witness: Seq<usize>,
    attestations: Seq<Attestation>,
    from: int,
) -> bool
    decreases if from < witness.len() { witness.len() - from } else { 0 }
{
    if from >= witness.len() {
        true
    } else {
        (witness[from] as int) < attestations.len()
            && chain_indices_in_range_from_spec(witness, attestations, from + 1)
    }
}

// r[impl web-of-trust.chain-validity]
pub open spec fn chain_indices_in_range_spec(witness: Seq<usize>, attestations: Seq<Attestation>) -> bool {
    chain_indices_in_range_from_spec(witness, attestations, 0)
}

// r[impl web-of-trust.chain-validity]
pub open spec fn chain_edges_active_from_spec(
    witness: Seq<usize>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    t: u64,
    from: int,
) -> bool
    decreases if from < witness.len() { witness.len() - from } else { 0 }
{
    if from >= witness.len() {
        true
    } else if (witness[from] as int) >= attestations.len() {
        false
    } else {
        let att = attestations[witness[from] as int];
        att.role_tag == TRUST_ROLE_TAG
            && attestation_active_at_spec(att, revocations, t)
            && chain_edges_active_from_spec(witness, attestations, revocations, t, from + 1)
    }
}

// r[impl web-of-trust.chain-validity]
pub open spec fn chain_adjacency_from_spec(
    witness: Seq<usize>,
    attestations: Seq<Attestation>,
    from: int,
) -> bool
    decreases if from + 1 < witness.len() { witness.len() - from } else { 0 }
{
    if from + 1 >= witness.len() {
        true
    } else if (witness[from] as int) >= attestations.len() || (witness[from + 1] as int) >= attestations.len() {
        false
    } else {
        let left = attestations[witness[from] as int];
        let right = attestations[witness[from + 1] as int];
        left.subject.digest@ == right.issuer.digest@
            && chain_adjacency_from_spec(witness, attestations, from + 1)
    }
}

// r[impl web-of-trust.chain-validity]
pub open spec fn chain_principal_at_spec(
    witness: Seq<usize>,
    attestations: Seq<Attestation>,
    pos: int,
) -> Seq<u8> {
    if witness.len() == 0 || pos < 0 || pos > witness.len() || !chain_indices_in_range_spec(witness, attestations) {
        Seq::empty()
    } else if pos == 0 {
        attestations[witness[0] as int].issuer.digest@
    } else {
        attestations[witness[pos - 1] as int].subject.digest@
    }
}

// r[impl web-of-trust.chain-validity]
pub open spec fn subject_fresh_upto_spec(
    witness: Seq<usize>,
    attestations: Seq<Attestation>,
    edge_idx: int,
    upto: int,
) -> bool
    decreases if upto >= 0 { upto + 1 } else { 0 }
{
    if upto < 0 {
        true
    } else {
        chain_principal_at_spec(witness, attestations, upto)
            != attestations[witness[edge_idx] as int].subject.digest@
            && subject_fresh_upto_spec(witness, attestations, edge_idx, upto - 1)
    }
}

// r[impl web-of-trust.chain-validity]
pub open spec fn no_repeated_from_spec(
    witness: Seq<usize>,
    attestations: Seq<Attestation>,
    from: int,
) -> bool
    decreases if from < witness.len() { witness.len() - from } else { 0 }
{
    if from >= witness.len() {
        true
    } else {
        subject_fresh_upto_spec(witness, attestations, from, from)
            && no_repeated_from_spec(witness, attestations, from + 1)
    }
}

// r[impl web-of-trust.chain-validity]
pub open spec fn no_repeated_principal_spec(witness: Seq<usize>, attestations: Seq<Attestation>) -> bool {
    if witness.len() == 0 || !chain_indices_in_range_spec(witness, attestations) {
        false
    } else {
        no_repeated_from_spec(witness, attestations, 0)
    }
}

// r[depends web-of-trust.chain-validity]
/// Exact chain-validity semantics for a witness of attestation indices.
pub open spec fn chain_valid_at_spec(
    witness: Seq<usize>,
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    t: u64,
) -> bool {
    &&& witness.len() > 0
    &&& chain_indices_in_range_spec(witness, attestations)
    &&& root_member_spec(roots, attestations[witness[0] as int].issuer)
    &&& chain_edges_active_from_spec(witness, attestations, revocations, t, 0)
    &&& chain_adjacency_from_spec(witness, attestations, 0)
    &&& no_repeated_principal_spec(witness, attestations)
    &&& attestations[witness[witness.len() - 1] as int].subject.digest@ == subject.digest@
}

fn digest_equal(left: &Vec<u8>, right: &Vec<u8>) -> (result: bool)
    ensures result == (left@ =~= right@)
{
    if left.len() != right.len() {
        false
    } else {
        let mut i: usize = 0;
        let mut eq = true;
        while i < left.len()
            invariant
                i <= left.len(),
                left.len() == right.len(),
                eq == (forall|j: int| 0 <= j < i as int ==> left@[j] == right@[j]),
            decreases left.len() - i,
        {
            eq = eq && left[i] == right[i];
            i += 1;
        }
        eq
    }
}


proof fn chain_indices_in_range_from_implies_index(
    witness: Seq<usize>,
    attestations: Seq<Attestation>,
    from: int,
    idx: int,
)
    requires
        chain_indices_in_range_from_spec(witness, attestations, from),
        from <= idx < witness.len(),
    ensures
        (witness[idx] as int) < attestations.len(),
    decreases if from <= idx { idx - from + 1 } else { 0 }
{
    if from == idx {
    } else {
        chain_indices_in_range_from_implies_index(witness, attestations, from + 1, idx);
    }
}

fn root_member_from(roots: &Vec<PrincipalId>, principal: &PrincipalId, from: usize) -> (result: bool)
    requires from <= roots.len()
    ensures result == root_member_from_spec(roots@, *principal, from as int)
    decreases roots.len() - from
{
    if from >= roots.len() {
        false
    } else {
        digest_equal(&roots[from].digest, &principal.digest)
            || root_member_from(roots, principal, from + 1)
    }
}

fn chain_indices_in_range_from(
    witness: &Vec<usize>,
    attestations: &Vec<Attestation>,
    from: usize,
) -> (result: bool)
    requires from <= witness.len()
    ensures result == chain_indices_in_range_from_spec(witness@, attestations@, from as int)
    decreases witness.len() - from
{
    if from >= witness.len() {
        true
    } else {
        witness[from] < attestations.len() && chain_indices_in_range_from(witness, attestations, from + 1)
    }
}

fn chain_edges_active_from(
    witness: &Vec<usize>,
    attestations: &Vec<Attestation>,
    revocations: &Vec<Revocation>,
    t: u64,
    from: usize,
) -> (result: bool)
    requires
        from <= witness.len(),
        chain_indices_in_range_from_spec(witness@, attestations@, from as int),
    ensures result == chain_edges_active_from_spec(witness@, attestations@, revocations@, t, from as int)
    decreases witness.len() - from
{
    if from >= witness.len() {
        true
    } else {
        proof {
            chain_indices_in_range_from_implies_index(witness@, attestations@, from as int, from as int);
        }
        let att = &attestations[witness[from]];
        (att.role_tag == TRUST_ROLE_TAG && attestation_active_at(att, revocations, t))
            && chain_edges_active_from(witness, attestations, revocations, t, from + 1)
    }
}

fn chain_adjacency_from(
    witness: &Vec<usize>,
    attestations: &Vec<Attestation>,
    from: usize,
) -> (result: bool)
    requires
        from <= witness.len(),
        chain_indices_in_range_spec(witness@, attestations@),
    ensures result == chain_adjacency_from_spec(witness@, attestations@, from as int)
    decreases witness.len() - from
{
    if from >= witness.len() {
        true
    } else if from == witness.len() - 1 {
        true
    } else {
        proof {
            chain_indices_in_range_from_implies_index(witness@, attestations@, 0, from as int);
            chain_indices_in_range_from_implies_index(witness@, attestations@, 0, (from + 1) as int);
        }
        let left = &attestations[witness[from]];
        let right = &attestations[witness[from + 1]];
        digest_equal(&left.subject.digest, &right.issuer.digest)
            && chain_adjacency_from(witness, attestations, from + 1)
    }
}

fn subject_fresh_upto(
    witness: &Vec<usize>,
    attestations: &Vec<Attestation>,
    edge_idx: usize,
    upto: usize,
) -> (result: bool)
    requires
        edge_idx < witness.len(),
        upto <= edge_idx,
        chain_indices_in_range_spec(witness@, attestations@),
    ensures result == subject_fresh_upto_spec(witness@, attestations@, edge_idx as int, upto as int)
    decreases upto + 1
{
    proof {
        chain_indices_in_range_from_implies_index(witness@, attestations@, 0, edge_idx as int);
        if upto > 0 {
            chain_indices_in_range_from_implies_index(witness@, attestations@, 0, (upto - 1) as int);
        }
    }
    let subject = &attestations[witness[edge_idx]].subject.digest;
    let current = if upto == 0 {
        &attestations[witness[0]].issuer.digest
    } else {
        &attestations[witness[upto - 1]].subject.digest
    };
    proof {
        assert(subject@ == attestations@[witness@[edge_idx as int] as int].subject.digest@);
        if upto == 0 {
            assert(current@ == chain_principal_at_spec(witness@, attestations@, 0));
        } else {
            assert(current@ == chain_principal_at_spec(witness@, attestations@, upto as int));
        }
    }
    if !digest_equal(subject, current) {
        proof {
            assert(subject@ != current@);
        }
        if upto == 0 {
            proof {
                assert(subject_fresh_upto_spec(witness@, attestations@, edge_idx as int, -1));
                assert(subject_fresh_upto_spec(witness@, attestations@, edge_idx as int, upto as int));
            }
            true
        } else {
            let rest = subject_fresh_upto(witness, attestations, edge_idx, upto - 1);
            proof {
                assert(rest == subject_fresh_upto_spec(witness@, attestations@, edge_idx as int, (upto - 1) as int));
                assert(chain_principal_at_spec(witness@, attestations@, upto as int) != attestations@[witness@[edge_idx as int] as int].subject.digest@);
                assert(subject_fresh_upto_spec(witness@, attestations@, edge_idx as int, upto as int)
                    == (chain_principal_at_spec(witness@, attestations@, upto as int)
                        != attestations@[witness@[edge_idx as int] as int].subject.digest@
                        && subject_fresh_upto_spec(witness@, attestations@, edge_idx as int, (upto - 1) as int)));
            }
            rest
        }
    } else {
        proof {
            assert(subject@ == current@);
            assert(!subject_fresh_upto_spec(witness@, attestations@, edge_idx as int, upto as int));
        }
        false
    }
}

fn no_repeated_from(
    witness: &Vec<usize>,
    attestations: &Vec<Attestation>,
    from: usize,
) -> (result: bool)
    requires
        from <= witness.len(),
        chain_indices_in_range_spec(witness@, attestations@),
    ensures result == no_repeated_from_spec(witness@, attestations@, from as int)
    decreases witness.len() - from
{
    if from >= witness.len() {
        true
    } else {
        subject_fresh_upto(witness, attestations, from, from)
            && no_repeated_from(witness, attestations, from + 1)
    }
}

// r[impl web-of-trust.chain-validity]
/// Check whether an attestation-index witness forms a valid active trust chain.
pub fn chain_valid_at(
    witness: &Vec<usize>,
    roots: &Vec<PrincipalId>,
    attestations: &Vec<Attestation>,
    revocations: &Vec<Revocation>,
    subject: &PrincipalId,
    t: u64,
) -> (result: bool)
    ensures result == chain_valid_at_spec(witness@, roots@, attestations@, revocations@, *subject, t)
{
    if witness.is_empty() {
        false
    } else {
        let in_range = chain_indices_in_range_from(witness, attestations, 0);
        if !in_range {
            false
        } else {
            let first = witness[0];
            proof {
                chain_indices_in_range_from_implies_index(witness@, attestations@, 0, 0);
            }
            root_member_from(roots, &attestations[first].issuer, 0)
                && chain_edges_active_from(witness, attestations, revocations, t, 0)
                && chain_adjacency_from(witness, attestations, 0)
                && no_repeated_from(witness, attestations, 0)
                && {
                    proof {
                        chain_indices_in_range_from_implies_index(
                            witness@,
                            attestations@,
                            0,
                            (witness.len() - 1) as int,
                        );
                    }
                    digest_equal(&attestations[witness[witness.len() - 1]].subject.digest, &subject.digest)
                }
        }
    }
}

proof fn chain_edges_active_from_implies_index(
    witness: Seq<usize>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    t: u64,
    from: int,
    idx: int,
)
    requires
        chain_indices_in_range_from_spec(witness, attestations, from),
        chain_edges_active_from_spec(witness, attestations, revocations, t, from),
        from <= idx < witness.len(),
    ensures
        attestations[witness[idx] as int].role_tag == TRUST_ROLE_TAG,
        attestation_active_at_spec(attestations[witness[idx] as int], revocations, t),
    decreases if from <= idx { idx - from + 1 } else { 0 }
{
    if from == idx {
    } else {
        chain_edges_active_from_implies_index(witness, attestations, revocations, t, from + 1, idx);
    }
}

proof fn chain_adjacency_from_implies_index(
    witness: Seq<usize>,
    attestations: Seq<Attestation>,
    from: int,
    idx: int,
)
    requires
        chain_indices_in_range_spec(witness, attestations),
        chain_adjacency_from_spec(witness, attestations, from),
        from <= idx,
        idx + 1 < witness.len(),
    ensures
        attestations[witness[idx] as int].subject.digest@
            == attestations[witness[idx + 1] as int].issuer.digest@,
    decreases if from <= idx { idx - from + 1 } else { 0 }
{
    if from == idx {
    } else {
        chain_adjacency_from_implies_index(witness, attestations, from + 1, idx);
    }
}

// r[verify web-of-trust.chain-validity]
/// Valid chains start from a root issuer.
pub proof fn chain_valid_root_issuer(
    witness: Seq<usize>,
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    t: u64,
)
    requires chain_valid_at_spec(witness, roots, attestations, revocations, subject, t)
    ensures root_member_spec(roots, attestations[witness[0] as int].issuer)
{
}

// r[verify web-of-trust.chain-validity]
/// Adjacent attestations line up subject-to-next-issuer.
pub proof fn chain_valid_adjacency(
    witness: Seq<usize>,
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    t: u64,
    i: nat,
)
    requires
        chain_valid_at_spec(witness, roots, attestations, revocations, subject, t),
        i + 1 < witness.len(),
    ensures
        attestations[witness[i as int] as int].subject.digest@
            == attestations[witness[(i + 1) as int] as int].issuer.digest@
{
    chain_adjacency_from_implies_index(witness, attestations, 0, i as int);
}

// r[verify web-of-trust.chain-validity]
/// Valid chains are simple paths with no repeated principals.
pub proof fn chain_valid_simple_path(
    witness: Seq<usize>,
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    t: u64,
)
    requires chain_valid_at_spec(witness, roots, attestations, revocations, subject, t)
    ensures no_repeated_principal_spec(witness, attestations)
{
}

// r[verify web-of-trust.chain-validity]
/// Only operational `trust` attestations contribute chain edges.
pub proof fn chain_valid_role_filtered(
    witness: Seq<usize>,
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    t: u64,
    i: nat,
)
    requires
        chain_valid_at_spec(witness, roots, attestations, revocations, subject, t),
        i < witness.len(),
    ensures
        attestations[witness[i as int] as int].role_tag == TRUST_ROLE_TAG
{
    chain_edges_active_from_implies_index(witness, attestations, revocations, t, 0, i as int);
}

// r[verify web-of-trust.chain-validity]
/// Every chain edge is active at the evaluation time.
pub proof fn chain_valid_requires_active_edges(
    witness: Seq<usize>,
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    t: u64,
    i: nat,
)
    requires
        chain_valid_at_spec(witness, roots, attestations, revocations, subject, t),
        i < witness.len(),
    ensures
        attestation_active_at_spec(attestations[witness[i as int] as int], revocations, t)
{
    chain_edges_active_from_implies_index(witness, attestations, revocations, t, 0, i as int);
}

// r[verify web-of-trust.chain-validity]
/// Invalid attestation records can never contribute a valid chain edge.
pub proof fn invalid_records_have_no_chain_edge(att: Attestation, revocations: Seq<Revocation>, t: u64)
    requires !attestation_record_ids_match_bytes_spec(att)
    ensures !(att.role_tag == TRUST_ROLE_TAG && attestation_active_at_spec(att, revocations, t))
{
    if att.role_tag == TRUST_ROLE_TAG && attestation_active_at_spec(att, revocations, t) {
        assert(attestation_record_ids_match_bytes_spec(att));
        assert(false);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::web_of_trust_attestation::attestation_payload_bytes;
    use crate::web_of_trust_attestation::attestation_signature_bytes_runtime;
    use crate::web_of_trust_principal::principal_id_from_bytes;
    use crate::web_of_trust_revocation::FAMILY_TARGET_KIND_TAG;
    use crate::web_of_trust_revocation::Revocation;
    use crate::web_of_trust_revocation::revocation_payload_bytes;
    use crate::web_of_trust_revocation::revocation_signature_bytes_runtime;

    fn signed_attestation(
        issuer_key_bytes: Vec<u8>,
        subject_key_bytes: Vec<u8>,
        role_tag: u8,
        not_before: u64,
        not_after: u64,
        sequence_no: u64,
    ) -> Attestation {
        let issuer = principal_id_from_bytes(&issuer_key_bytes);
        let subject = principal_id_from_bytes(&subject_key_bytes);
        let unsigned = Attestation {
            issuer_key_bytes,
            issuer,
            subject_key_bytes,
            subject,
            role_tag,
            not_before,
            not_after,
            sequence_no,
            signature: Vec::new(),
        };
        let payload = attestation_payload_bytes(&unsigned).unwrap();
        let signature = attestation_signature_bytes_runtime(unsigned.issuer_key_bytes.as_slice(), payload.as_slice());
        Attestation { signature, ..unsigned }
    }

    fn signed_family_revocation(
        issuer_key_bytes: Vec<u8>,
        subject_key_bytes: Vec<u8>,
        role_tag: u8,
        effective_time: u64,
    ) -> Revocation {
        let issuer = principal_id_from_bytes(&issuer_key_bytes);
        let family_subject = principal_id_from_bytes(&subject_key_bytes);
        let unsigned = Revocation {
            issuer_key_bytes,
            issuer,
            target_kind_tag: FAMILY_TARGET_KIND_TAG,
            exact_payload_target: Vec::new(),
            family_subject_key_bytes: subject_key_bytes,
            family_subject,
            family_role_tag: role_tag,
            effective_time,
            signature: Vec::new(),
        };
        let payload = revocation_payload_bytes(&unsigned).unwrap();
        let signature = revocation_signature_bytes_runtime(unsigned.issuer_key_bytes.as_slice(), payload.as_slice());
        Revocation { signature, ..unsigned }
    }

    fn chain_valid_oracle(
        witness: &[usize],
        roots: &[PrincipalId],
        attestations: &[Attestation],
        revocations: &Vec<Revocation>,
        subject: &PrincipalId,
        t: u64,
    ) -> bool {
        if witness.is_empty() {
            return false;
        }
        if witness.iter().any(|&idx| idx >= attestations.len()) {
            return false;
        }
        let first = &attestations[witness[0]];
        if !roots.iter().any(|root| root.digest == first.issuer.digest) {
            return false;
        }
        let mut seen = vec![first.issuer.digest.clone()];
        for (pos, idx) in witness.iter().copied().enumerate() {
            let att = &attestations[idx];
            if att.role_tag != TRUST_ROLE_TAG
                || !crate::web_of_trust_attestation::attestation_active_at(att, revocations, t)
            {
                return false;
            }
            if pos > 0 {
                let prev = &attestations[witness[pos - 1]];
                if prev.subject.digest != att.issuer.digest {
                    return false;
                }
            }
            if seen.contains(&att.subject.digest) {
                return false;
            }
            seen.push(att.subject.digest.clone());
        }
        attestations[*witness.last().unwrap()].subject.digest == subject.digest
    }

    // r[verify web-of-trust.chain-validity]
    #[test]
    fn test_active_root_to_subject_path_is_valid() {
        let root_key = vec![1, 2, 3];
        let mid_key = vec![4, 5, 6];
        let subject_key = vec![7, 8, 9];
        let att1 = signed_attestation(root_key.clone(), mid_key.clone(), TRUST_ROLE_TAG, 0, 20, 1);
        let att2 = signed_attestation(mid_key, subject_key.clone(), TRUST_ROLE_TAG, 0, 20, 2);
        let roots = vec![principal_id_from_bytes(&root_key)];
        let attestations = vec![att1, att2];
        let subject = principal_id_from_bytes(&subject_key);
        assert!(chain_valid_at(&vec![0, 1], &roots, &attestations, &Vec::new(), &subject, 5));
    }

    // r[verify web-of-trust.chain-validity]
    #[test]
    fn test_wrong_root_issuer_rejects_chain() {
        let att1 = signed_attestation(vec![1, 1, 1], vec![2, 2, 2], TRUST_ROLE_TAG, 0, 20, 1);
        let roots = vec![principal_id_from_bytes(&[9, 9, 9])];
        let subject = principal_id_from_bytes(&[2, 2, 2]);
        assert!(!chain_valid_at(&vec![0], &roots, &vec![att1], &Vec::new(), &subject, 5));
    }

    // r[verify web-of-trust.chain-validity]
    #[test]
    fn test_broken_adjacency_rejects_chain() {
        let att1 = signed_attestation(vec![1, 2, 3], vec![4, 5, 6], TRUST_ROLE_TAG, 0, 20, 1);
        let att2 = signed_attestation(vec![7, 8, 9], vec![9, 8, 7], TRUST_ROLE_TAG, 0, 20, 2);
        let roots = vec![principal_id_from_bytes(&[1, 2, 3])];
        let subject = principal_id_from_bytes(&[9, 8, 7]);
        assert!(!chain_valid_at(&vec![0, 1], &roots, &vec![att1, att2], &Vec::new(), &subject, 5));
    }

    // r[verify web-of-trust.chain-validity]
    #[test]
    fn test_cycle_makes_chain_invalid() {
        let root_key = vec![1, 2, 3];
        let mid_key = vec![4, 5, 6];
        let att1 = signed_attestation(root_key.clone(), mid_key.clone(), TRUST_ROLE_TAG, 0, 20, 1);
        let att2 = signed_attestation(mid_key, root_key.clone(), TRUST_ROLE_TAG, 0, 20, 2);
        let roots = vec![principal_id_from_bytes(&root_key)];
        let subject = principal_id_from_bytes(&root_key);
        assert!(!chain_valid_at(&vec![0, 1], &roots, &vec![att1, att2], &Vec::new(), &subject, 5));
    }

    // r[verify web-of-trust.chain-validity]
    #[test]
    fn test_non_trust_attestation_is_not_edge() {
        let att = signed_attestation(vec![1, 2, 3], vec![4, 5, 6], 0x55, 0, 20, 1);
        let roots = vec![principal_id_from_bytes(&[1, 2, 3])];
        let subject = principal_id_from_bytes(&[4, 5, 6]);
        assert!(!chain_valid_at(&vec![0], &roots, &vec![att], &Vec::new(), &subject, 5));
    }

    // r[verify web-of-trust.chain-validity]
    #[test]
    fn test_inactive_edge_rejects_chain() {
        let att = signed_attestation(vec![1, 2, 3], vec![4, 5, 6], TRUST_ROLE_TAG, 10, 20, 1);
        let roots = vec![principal_id_from_bytes(&[1, 2, 3])];
        let subject = principal_id_from_bytes(&[4, 5, 6]);
        assert!(!chain_valid_at(&vec![0], &roots, &vec![att], &Vec::new(), &subject, 9));
    }

    // r[verify web-of-trust.chain-validity]
    #[test]
    fn test_revoked_edge_rejects_chain() {
        let root_key = vec![1, 2, 3];
        let subject_key = vec![4, 5, 6];
        let att = signed_attestation(root_key.clone(), subject_key.clone(), TRUST_ROLE_TAG, 0, 20, 1);
        let rev = signed_family_revocation(root_key.clone(), subject_key, TRUST_ROLE_TAG, 5);
        let roots = vec![principal_id_from_bytes(&root_key)];
        let subject = principal_id_from_bytes(&[4, 5, 6]);
        assert!(!chain_valid_at(&vec![0], &roots, &vec![att], &vec![rev], &subject, 5));
    }

    // r[verify web-of-trust.chain-validity]
    #[test]
    fn test_invalid_record_contributes_no_chain_edge() {
        let att = signed_attestation(vec![1, 2, 3], vec![4, 5, 6], TRUST_ROLE_TAG, 0, 20, 1);
        let tampered_subject_bytes = vec![4, 5, 9];
        let bad = Attestation {
            subject_key_bytes: tampered_subject_bytes,
            ..att
        };
        let roots = vec![principal_id_from_bytes(&[1, 2, 3])];
        let subject = principal_id_from_bytes(&[4, 5, 6]);
        assert!(!chain_valid_at(&vec![0], &roots, &vec![bad], &Vec::new(), &subject, 5));
    }

    proptest! {
        // r[verify web-of-trust.chain-validity]
        #[test]
        fn prop_chain_validity_oracle(
            witness in prop::collection::vec(0usize..4, 0..5),
            subject_choice in 0usize..4,
            time in 0u64..20,
        ) {
            let root_key = vec![1, 2, 3];
            let a_key = vec![4, 5, 6];
            let b_key = vec![7, 8, 9];
            let c_key = vec![10, 11, 12];

            let attestations = vec![
                signed_attestation(root_key.clone(), a_key.clone(), TRUST_ROLE_TAG, 0, 20, 1),
                signed_attestation(a_key.clone(), b_key.clone(), TRUST_ROLE_TAG, 0, 20, 2),
                signed_attestation(b_key.clone(), c_key.clone(), TRUST_ROLE_TAG, 0, 20, 3),
                signed_attestation(a_key.clone(), root_key.clone(), TRUST_ROLE_TAG, 0, 20, 4),
            ];
            let roots = vec![principal_id_from_bytes(&root_key)];
            let subjects = [
                principal_id_from_bytes(&root_key),
                principal_id_from_bytes(&a_key),
                principal_id_from_bytes(&b_key),
                principal_id_from_bytes(&c_key),
            ];
            let subject = &subjects[subject_choice];
            let revocations = Vec::new();

            let oracle = chain_valid_oracle(&witness, &roots, &attestations, &revocations, subject, time);
            prop_assert_eq!(chain_valid_at(&witness, &roots, &attestations, &revocations, subject, time), oracle);
        }
    }
}
