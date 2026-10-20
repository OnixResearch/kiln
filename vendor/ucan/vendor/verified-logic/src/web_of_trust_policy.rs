//! Verified web-of-trust threshold policy.
//!
//! Counts distinct rooted-trusted introducers with an active direct final
//! `trust` edge into the subject. The final hop consumes one depth unit,
//! so introducers are justified under bound `d - 1`.

#![allow(clippy::ptr_arg)]

use vstd::prelude::*;

use crate::web_of_trust_attestation::Attestation;
#[cfg(verus_keep_ghost)]
use crate::web_of_trust_attestation::attestation_record_ids_match_bytes_spec;
#[cfg(verus_keep_ghost)]
use crate::web_of_trust_attestation::invalid_attestation_is_inactive;
#[cfg(verus_keep_ghost)]
use crate::web_of_trust_chain::chain_indices_in_range_from_spec;
use crate::web_of_trust_chain::chain_valid_at;
#[cfg(verus_keep_ghost)]
use crate::web_of_trust_chain::chain_valid_at_spec;
#[cfg(verus_keep_ghost)]
use crate::web_of_trust_chain::chain_valid_requires_active_edges;
use crate::web_of_trust_principal::PrincipalId;
use crate::web_of_trust_revocation::Revocation;

verus! {

// r[impl web-of-trust.threshold]
pub open spec fn seen_indices_in_range_from_spec(
    seen: Seq<usize>,
    attestations: Seq<Attestation>,
    from: int,
) -> bool
    decreases if from < seen.len() { seen.len() - from } else { 0 }
{
    if from >= seen.len() {
        true
    } else {
        (seen[from] as int) < attestations.len()
            && seen_indices_in_range_from_spec(seen, attestations, from + 1)
    }
}

// r[impl web-of-trust.threshold]
pub open spec fn seen_indices_in_range_spec(seen: Seq<usize>, attestations: Seq<Attestation>) -> bool {
    seen_indices_in_range_from_spec(seen, attestations, 0)
}

// r[impl web-of-trust.threshold]
pub open spec fn seen_introducer_from_spec(
    seen: Seq<usize>,
    attestations: Seq<Attestation>,
    digest: Seq<u8>,
    from: int,
) -> bool
    recommends seen_indices_in_range_spec(seen, attestations)
    decreases if from < seen.len() { seen.len() - from } else { 0 }
{
    if from >= seen.len() {
        false
    } else {
        attestations[seen[from] as int].issuer.digest@ == digest
            || seen_introducer_from_spec(seen, attestations, digest, from + 1)
    }
}

// r[impl web-of-trust.threshold]
pub open spec fn seen_introducer_spec(
    seen: Seq<usize>,
    attestations: Seq<Attestation>,
    digest: Seq<u8>,
) -> bool
    recommends seen_indices_in_range_spec(seen, attestations)
{
    seen_introducer_from_spec(seen, attestations, digest, 0)
}

// r[impl web-of-trust.threshold]
pub open spec fn search_chain_with_final_suffix_from_spec(
    prefix: Seq<usize>,
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    t: u64,
    final_idx: nat,
    remaining: nat,
    next: nat,
) -> bool {
    exists|suffix: Seq<usize>|
        suffix.len() == remaining as int
            && chain_valid_at_spec(prefix + suffix + seq![final_idx as usize], roots, attestations, revocations, subject, t)
            && (
                remaining == 0
                    || suffix[0] == next as usize
                    || next < suffix[0] as nat
            )
}

// r[impl web-of-trust.threshold]
pub open spec fn search_chain_with_final_exact_len_from_spec(
    prefix: Seq<usize>,
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    t: u64,
    final_idx: nat,
    remaining: nat,
    next: nat,
) -> bool
    recommends attestations.len() <= usize::MAX as nat
    decreases remaining, if next <= attestations.len() { attestations.len() - next } else { 0 }
{
    if final_idx >= attestations.len() {
        false
    } else if remaining == 0 {
        chain_valid_at_spec(prefix + seq![final_idx as usize], roots, attestations, revocations, subject, t)
    } else if next >= attestations.len() {
        false
    } else {
        search_chain_with_final_exact_len_from_spec(
            prefix + seq![next as usize],
            roots,
            attestations,
            revocations,
            subject,
            t,
            final_idx,
            (remaining - 1) as nat,
            0,
        ) || search_chain_with_final_exact_len_from_spec(
            prefix,
            roots,
            attestations,
            revocations,
            subject,
            t,
            final_idx,
            remaining,
            next + 1,
        )
    }
}

// r[impl web-of-trust.threshold]
pub open spec fn search_chain_with_final_upto_len_from_spec(
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    t: u64,
    final_idx: nat,
    depth: nat,
    current: nat,
) -> bool
    recommends attestations.len() <= usize::MAX as nat
{
    exists|len: nat|
        current <= len < depth
            && search_chain_with_final_exact_len_from_spec(
                Seq::empty(),
                roots,
                attestations,
                revocations,
                subject,
                t,
                final_idx,
                len,
                0,
            )
}

// r[depends web-of-trust.threshold]
/// Final-hop qualification under the threshold policy.
pub open spec fn introducer_qualifies_at_spec(
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    final_idx: nat,
    depth: nat,
    t: u64,
) -> bool
    recommends attestations.len() <= usize::MAX as nat
{
    depth > 0
        && search_chain_with_final_upto_len_from_spec(
            roots,
            attestations,
            revocations,
            subject,
            t,
            final_idx,
            depth,
            0,
        )
}

// r[impl web-of-trust.threshold]
pub open spec fn threshold_accepts_from_spec(
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    k: nat,
    depth: nat,
    t: u64,
    from: nat,
    seen: Seq<usize>,
) -> bool
    recommends
        attestations.len() <= usize::MAX as nat,
        seen_indices_in_range_spec(seen, attestations),
    decreases if from <= attestations.len() { attestations.len() - from } else { 0 }
{
    if k == 0 {
        true
    } else if from >= attestations.len() {
        false
    } else {
        let digest = attestations[from as int].issuer.digest@;
        if introducer_qualifies_at_spec(roots, attestations, revocations, subject, from, depth, t)
            && !seen_introducer_spec(seen, attestations, digest)
        {
            threshold_accepts_from_spec(
                roots,
                attestations,
                revocations,
                subject,
                (k - 1) as nat,
                depth,
                t,
                from + 1,
                seen + seq![from as usize],
            )
        } else {
            threshold_accepts_from_spec(
                roots,
                attestations,
                revocations,
                subject,
                k,
                depth,
                t,
                from + 1,
                seen,
            )
        }
    }
}

// r[impl web-of-trust.threshold]
pub open spec fn threshold_accepts_witness_spec(
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    k: nat,
    depth: nat,
    t: u64,
) -> bool
    recommends attestations.len() <= usize::MAX as nat
{
    k > 0 && threshold_accepts_from_spec(roots, attestations, revocations, subject, k, depth, t, 0, Seq::empty())
}

// r[depends web-of-trust.threshold]
/// Threshold acceptance over distinct introducers.
pub open spec fn threshold_accepts_spec(
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    k: nat,
    depth: nat,
    t: u64,
) -> bool
    recommends attestations.len() <= usize::MAX as nat
{
    k > 0 && threshold_accepts_from_spec(roots, attestations, revocations, subject, k, depth, t, 0, Seq::empty())
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

fn clone_append(prefix: &Vec<usize>, idx: usize) -> (result: Vec<usize>)
    ensures result@ == prefix@ + seq![idx]
{
    let mut result = Vec::new();
    let mut i: usize = 0;
    while i < prefix.len()
        invariant
            i <= prefix.len(),
            result@ == prefix@.subrange(0, i as int),
        decreases prefix.len() - i,
    {
        result.push(prefix[i]);
        i += 1;
    }
    result.push(idx);
    result
}

proof fn seen_indices_in_range_from_implies_index(
    seen: Seq<usize>,
    attestations: Seq<Attestation>,
    from: int,
    idx: int,
)
    requires
        seen_indices_in_range_from_spec(seen, attestations, from),
        from <= idx < seen.len(),
    ensures
        (seen[idx] as int) < attestations.len(),
    decreases if from <= idx { idx - from + 1 } else { 0 }
{
    if from == idx {
    } else {
        seen_indices_in_range_from_implies_index(seen, attestations, from + 1, idx);
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

proof fn seen_indices_in_range_from_pointwise(
    seen: Seq<usize>,
    attestations: Seq<Attestation>,
    from: int,
)
    requires
        0 <= from <= seen.len(),
        forall|i: int| from <= i < seen.len() ==> (seen[i] as int) < attestations.len(),
    ensures
        seen_indices_in_range_from_spec(seen, attestations, from),
    decreases if from < seen.len() { seen.len() - from } else { 0 }
{
    if from < seen.len() {
        seen_indices_in_range_from_pointwise(seen, attestations, from + 1);
    }
}

proof fn seen_indices_in_range_append(
    seen: Seq<usize>,
    attestations: Seq<Attestation>,
    idx: usize,
)
    requires
        seen_indices_in_range_spec(seen, attestations),
        (idx as int) < attestations.len(),
    ensures
        seen_indices_in_range_spec(seen + seq![idx], attestations),
{
    assert forall|i: int| 0 <= i < (seen + seq![idx]).len() implies ((seen + seq![idx])[i] as int) < attestations.len() by {
        if i < seen.len() {
            assert((seen + seq![idx])[i] == seen[i]);
            seen_indices_in_range_from_implies_index(seen, attestations, 0, i);
        } else {
            assert(i == seen.len());
            assert((seen + seq![idx])[i] == idx);
        }
    }
    seen_indices_in_range_from_pointwise(seen + seq![idx], attestations, 0);
}

proof fn seq_head_tail_usize(values: Seq<usize>)
    requires values.len() > 0
    ensures values == seq![values[0]] + values.subrange(1, values.len() as int)
{
    let rebuilt = seq![values[0]] + values.subrange(1, values.len() as int);
    assert(rebuilt.len() == values.len());
    assert forall|i: int| 0 <= i < values.len() implies values[i] == rebuilt[i] by {
        if i == 0 {
        } else {
            assert(rebuilt[i] == values.subrange(1, values.len() as int)[i - 1]);
            assert(values.subrange(1, values.len() as int)[i - 1] == values[i]);
        }
    }
    assert(values =~= rebuilt);
}

fn seen_introducer_from(
    seen: &Vec<usize>,
    attestations: &Vec<Attestation>,
    digest: &Vec<u8>,
    from: usize,
) -> (result: bool)
    requires
        from <= seen.len(),
        seen_indices_in_range_spec(seen@, attestations@),
    ensures
        result == seen_introducer_from_spec(seen@, attestations@, digest@, from as int)
    decreases seen.len() - from
{
    if from >= seen.len() {
        false
    } else {
        proof {
            seen_indices_in_range_from_implies_index(seen@, attestations@, 0, from as int);
        }
        let att = &attestations[seen[from]];
        digest_equal(&att.issuer.digest, digest)
            || seen_introducer_from(seen, attestations, digest, from + 1)
    }
}

fn search_chain_with_final_exact_len_from(
    prefix: &Vec<usize>,
    roots: &Vec<PrincipalId>,
    attestations: &Vec<Attestation>,
    revocations: &Vec<Revocation>,
    subject: &PrincipalId,
    t: u64,
    final_idx: usize,
    remaining: usize,
    next: usize,
) -> (result: bool)
    requires next <= attestations.len()
    ensures result == search_chain_with_final_exact_len_from_spec(prefix@, roots@, attestations@, revocations@, *subject, t, final_idx as nat, remaining as nat, next as nat)
    decreases remaining, attestations.len() - next
{
    if final_idx >= attestations.len() {
        false
    } else if remaining == 0 {
        let witness = clone_append(prefix, final_idx);
        chain_valid_at(&witness, roots, attestations, revocations, subject, t)
    } else if next >= attestations.len() {
        false
    } else {
        let extended = clone_append(prefix, next);
        search_chain_with_final_exact_len_from(&extended, roots, attestations, revocations, subject, t, final_idx, remaining - 1, 0)
            || search_chain_with_final_exact_len_from(prefix, roots, attestations, revocations, subject, t, final_idx, remaining, next + 1)
    }
}

fn search_chain_with_final_upto_len_from(
    roots: &Vec<PrincipalId>,
    attestations: &Vec<Attestation>,
    revocations: &Vec<Revocation>,
    subject: &PrincipalId,
    t: u64,
    final_idx: usize,
    depth: usize,
    current: usize,
) -> (result: bool)
    requires attestations@.len() <= usize::MAX as int
    ensures result == search_chain_with_final_upto_len_from_spec(roots@, attestations@, revocations@, *subject, t, final_idx as nat, depth as nat, current as nat)
    decreases if current < depth { depth - current } else { 0 }
{
    proof {
        assert(attestations@.len() == attestations.len() as int);
        assert(attestations.len() as int <= usize::MAX as int);
    }
    if current >= depth {
        false
    } else {
        let exact = search_chain_with_final_exact_len_from(
            &Vec::new(),
            roots,
            attestations,
            revocations,
            subject,
            t,
            final_idx,
            current,
            0,
        );
        if current + 1 < depth {
            exact || search_chain_with_final_upto_len_from(roots, attestations, revocations, subject, t, final_idx, depth, current + 1)
        } else {
            exact
        }
    }
}

fn introducer_qualifies_at(
    roots: &Vec<PrincipalId>,
    attestations: &Vec<Attestation>,
    revocations: &Vec<Revocation>,
    subject: &PrincipalId,
    final_idx: usize,
    depth: usize,
    t: u64,
) -> (result: bool)
    requires attestations@.len() <= usize::MAX as int
    ensures result == introducer_qualifies_at_spec(roots@, attestations@, revocations@, *subject, final_idx as nat, depth as nat, t)
{
    if depth == 0 {
        false
    } else {
        search_chain_with_final_upto_len_from(roots, attestations, revocations, subject, t, final_idx, depth, 0)
    }
}

fn threshold_accepts_from(
    roots: &Vec<PrincipalId>,
    attestations: &Vec<Attestation>,
    revocations: &Vec<Revocation>,
    subject: &PrincipalId,
    k: usize,
    depth: usize,
    t: u64,
    from: usize,
    seen: &Vec<usize>,
) -> (result: bool)
    requires
        attestations@.len() <= usize::MAX as int,
        from <= attestations.len(),
        seen_indices_in_range_spec(seen@, attestations@),
    ensures
        result == threshold_accepts_from_spec(roots@, attestations@, revocations@, *subject, k as nat, depth as nat, t, from as nat, seen@)
    decreases attestations.len() - from
{
    if k == 0 {
        true
    } else if from >= attestations.len() {
        false
    } else {
        let include = introducer_qualifies_at(roots, attestations, revocations, subject, from, depth, t)
            && !seen_introducer_from(seen, attestations, &attestations[from].issuer.digest, 0);
        if include {
            let next_seen = clone_append(seen, from);
            proof {
                seen_indices_in_range_append(seen@, attestations@, from);
            }
            threshold_accepts_from(roots, attestations, revocations, subject, k - 1, depth, t, from + 1, &next_seen)
        } else {
            threshold_accepts_from(roots, attestations, revocations, subject, k, depth, t, from + 1, seen)
        }
    }
}

// r[impl web-of-trust.threshold]
/// Decide whether at least `k` distinct introducers justify `subject` under depth `depth` at time `t`.
pub fn threshold_accepts(
    roots: &Vec<PrincipalId>,
    attestations: &Vec<Attestation>,
    revocations: &Vec<Revocation>,
    subject: &PrincipalId,
    k: usize,
    depth: usize,
    t: u64,
) -> (result: bool)
    requires attestations@.len() <= usize::MAX as int
    ensures result == threshold_accepts_spec(roots@, attestations@, revocations@, *subject, k as nat, depth as nat, t)
{
    proof {
        assert(attestations@.len() == attestations.len() as int);
        assert(attestations.len() as int <= usize::MAX as int);
    }
    if k == 0 {
        false
    } else {
        threshold_accepts_from(roots, attestations, revocations, subject, k, depth, t, 0, &Vec::new())
    }
}

proof fn search_chain_with_final_exact_len_sound_from(
    prefix: Seq<usize>,
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    t: u64,
    final_idx: nat,
    remaining: nat,
    next: nat,
)
    requires
        attestations.len() <= usize::MAX as nat,
        next <= attestations.len(),
        search_chain_with_final_exact_len_from_spec(prefix, roots, attestations, revocations, subject, t, final_idx, remaining, next),
    ensures
        search_chain_with_final_suffix_from_spec(prefix, roots, attestations, revocations, subject, t, final_idx, remaining, next),
    decreases remaining, if next <= attestations.len() { attestations.len() - next } else { 0 }
{
    if final_idx >= attestations.len() {
        assert(false);
    } else if remaining == 0 {
        let suffix = Seq::<usize>::empty();
        assert(prefix + suffix + seq![final_idx as usize] == prefix + seq![final_idx as usize]);
        assert(chain_valid_at_spec(prefix + suffix + seq![final_idx as usize], roots, attestations, revocations, subject, t));
        assert(search_chain_with_final_suffix_from_spec(prefix, roots, attestations, revocations, subject, t, final_idx, remaining, next));
    } else if next >= attestations.len() {
        assert(false);
    } else if search_chain_with_final_exact_len_from_spec(
        prefix + seq![next as usize],
        roots,
        attestations,
        revocations,
        subject,
        t,
        final_idx,
        (remaining - 1) as nat,
        0,
    ) {
        search_chain_with_final_exact_len_sound_from(
            prefix + seq![next as usize],
            roots,
            attestations,
            revocations,
            subject,
            t,
            final_idx,
            (remaining - 1) as nat,
            0,
        );
        let tail = choose|tail: Seq<usize>|
            tail.len() == (remaining - 1) as int
                && chain_valid_at_spec((prefix + seq![next as usize]) + tail + seq![final_idx as usize], roots, attestations, revocations, subject, t)
                && (((remaining - 1) as nat) == 0 || tail[0] == 0usize || 0 < tail[0] as nat);
        let suffix = seq![next as usize] + tail;
        assert(suffix.len() == remaining as int);
        assert(prefix + suffix + seq![final_idx as usize] == (prefix + seq![next as usize]) + tail + seq![final_idx as usize]);
        assert(chain_valid_at_spec(prefix + suffix + seq![final_idx as usize], roots, attestations, revocations, subject, t));
        assert(suffix[0] == next as usize);
        assert(search_chain_with_final_suffix_from_spec(prefix, roots, attestations, revocations, subject, t, final_idx, remaining, next));
    } else {
        search_chain_with_final_exact_len_sound_from(prefix, roots, attestations, revocations, subject, t, final_idx, remaining, next + 1);
        let suffix = choose|suffix: Seq<usize>|
            suffix.len() == remaining as int
                && chain_valid_at_spec(prefix + suffix + seq![final_idx as usize], roots, attestations, revocations, subject, t)
                && (remaining == 0 || suffix[0] == (next + 1) as usize || (next + 1) < suffix[0] as nat);
        if remaining > 0 {
            if suffix[0] == (next + 1) as usize {
                assert(next < suffix[0] as nat);
            } else {
                assert((next + 1) < suffix[0] as nat);
                assert(next < suffix[0] as nat);
            }
        }
        assert(search_chain_with_final_suffix_from_spec(prefix, roots, attestations, revocations, subject, t, final_idx, remaining, next));
    }
}

proof fn search_chain_with_final_exact_len_complete_from(
    prefix: Seq<usize>,
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    t: u64,
    final_idx: nat,
    remaining: nat,
    next: nat,
    suffix: Seq<usize>,
)
    requires
        attestations.len() <= usize::MAX as nat,
        final_idx <= usize::MAX as nat,
        next <= attestations.len(),
        suffix.len() == remaining as int,
        chain_valid_at_spec(prefix + suffix + seq![final_idx as usize], roots, attestations, revocations, subject, t),
        remaining == 0 || suffix[0] == next as usize || next < suffix[0] as nat,
    ensures
        search_chain_with_final_exact_len_from_spec(prefix, roots, attestations, revocations, subject, t, final_idx, remaining, next),
    decreases remaining, if remaining > 0 && next <= suffix[0] as nat { suffix[0] as nat - next + 1 } else { 0 }
{
    if remaining == 0 {
        let witness = prefix + seq![final_idx as usize];
        assert(chain_indices_in_range_from_spec(witness, attestations, 0));
        chain_indices_in_range_from_implies_index(witness, attestations, 0, witness.len() - 1);
        assert(final_idx as usize as int == final_idx as int);
        assert(final_idx < attestations.len());
        assert(chain_valid_at_spec(prefix + seq![final_idx as usize], roots, attestations, revocations, subject, t));
    } else {
        let witness = prefix + suffix + seq![final_idx as usize];
        assert(chain_indices_in_range_from_spec(witness, attestations, 0));
        chain_indices_in_range_from_implies_index(witness, attestations, 0, prefix.len() as int);
        let first = suffix[0] as nat;
        if suffix[0] == next as usize {
            seq_head_tail_usize(suffix);
            let tail = suffix.subrange(1, suffix.len() as int);
            assert((prefix + seq![first as usize]) + tail + seq![final_idx as usize] == prefix + suffix + seq![final_idx as usize]);
            assert(tail.len() == (remaining - 1) as int);
            assert(chain_valid_at_spec((prefix + seq![first as usize]) + tail + seq![final_idx as usize], roots, attestations, revocations, subject, t));
            search_chain_with_final_exact_len_complete_from(
                prefix + seq![first as usize],
                roots,
                attestations,
                revocations,
                subject,
                t,
                final_idx,
                (remaining - 1) as nat,
                0,
                tail,
            );
        } else {
            search_chain_with_final_exact_len_complete_from(prefix, roots, attestations, revocations, subject, t, final_idx, remaining, next + 1, suffix);
        }
    }
}

proof fn search_chain_with_final_upto_len_sound_from(
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    t: u64,
    final_idx: nat,
    depth: nat,
    current: nat,
)
    requires
        attestations.len() <= usize::MAX as nat,
        search_chain_with_final_upto_len_from_spec(roots, attestations, revocations, subject, t, final_idx, depth, current),
    ensures
        exists|witness: Seq<usize>|
            0 < witness.len() <= depth as int
                && witness[witness.len() - 1] == final_idx as usize
                && chain_valid_at_spec(witness, roots, attestations, revocations, subject, t),
    decreases if current < depth { depth - current } else { 0 }
{
    if current >= depth {
        assert(false);
    } else if search_chain_with_final_exact_len_from_spec(Seq::empty(), roots, attestations, revocations, subject, t, final_idx, current, 0) {
        search_chain_with_final_exact_len_sound_from(Seq::empty(), roots, attestations, revocations, subject, t, final_idx, current, 0);
        let prefix = choose|prefix: Seq<usize>|
            prefix.len() == current as int
                && chain_valid_at_spec(prefix + seq![final_idx as usize], roots, attestations, revocations, subject, t)
                && (current == 0 || 0 <= prefix[0] as nat);
        let witness = prefix + seq![final_idx as usize];
        assert(0 < witness.len());
        assert(witness.len() <= depth as int);
        assert(witness[witness.len() - 1] == final_idx as usize);
        assert(chain_valid_at_spec(witness, roots, attestations, revocations, subject, t));
    } else {
        search_chain_with_final_upto_len_sound_from(roots, attestations, revocations, subject, t, final_idx, depth, current + 1);
    }
}

proof fn search_chain_with_final_upto_len_complete_from(
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    t: u64,
    final_idx: nat,
    depth: nat,
    current: nat,
    witness: Seq<usize>,
)
    requires
        attestations.len() <= usize::MAX as nat,
        final_idx <= usize::MAX as nat,
        0 < witness.len() <= depth as int,
        witness[witness.len() - 1] == final_idx as usize,
        current <= witness.len() as nat - 1,
        chain_valid_at_spec(witness, roots, attestations, revocations, subject, t),
    ensures
        search_chain_with_final_upto_len_from_spec(roots, attestations, revocations, subject, t, final_idx, depth, current),
    decreases if current <= witness.len() as nat - 1 { witness.len() as nat - current } else { 0 }
{
    let prefix_len = witness.len() as nat - 1;
    if current == prefix_len {
        let prefix = witness.subrange(0, witness.len() - 1);
        assert(prefix + seq![final_idx as usize] == witness);
        assert(chain_valid_at_spec(prefix + seq![final_idx as usize], roots, attestations, revocations, subject, t));
        search_chain_with_final_exact_len_complete_from(Seq::empty(), roots, attestations, revocations, subject, t, final_idx, prefix_len as nat, 0, prefix);
    } else {
        search_chain_with_final_upto_len_complete_from(roots, attestations, revocations, subject, t, final_idx, depth, current + 1, witness);
    }
}

// r[verify web-of-trust.threshold]
/// Count-based threshold acceptance implies the threshold witness semantics.
pub proof fn threshold_accepts_sound(
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    k: nat,
    depth: nat,
    t: u64,
)
    requires
        attestations.len() <= usize::MAX as nat,
        threshold_accepts_spec(roots, attestations, revocations, subject, k, depth, t)
    ensures
        threshold_accepts_witness_spec(roots, attestations, revocations, subject, k, depth, t)
{
}

// r[verify web-of-trust.threshold]
/// The threshold witness semantics are complete for the count-based checker.
pub proof fn threshold_accepts_complete(
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    k: nat,
    depth: nat,
    t: u64,
)
    requires
        attestations.len() <= usize::MAX as nat,
        threshold_accepts_witness_spec(roots, attestations, revocations, subject, k, depth, t)
    ensures
        threshold_accepts_spec(roots, attestations, revocations, subject, k, depth, t)
{
}

// r[verify web-of-trust.threshold]
/// Invalid attestation records cannot contribute a qualifying threshold final hop.
pub proof fn invalid_records_ignored_by_threshold(
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    final_idx: nat,
    depth: nat,
    t: u64,
)
    requires
        final_idx < attestations.len(),
        !attestation_record_ids_match_bytes_spec(attestations[final_idx as int]),
        attestations.len() <= usize::MAX as nat,
    ensures
        !introducer_qualifies_at_spec(roots, attestations, revocations, subject, final_idx, depth, t),
{
    if introducer_qualifies_at_spec(roots, attestations, revocations, subject, final_idx, depth, t) {
        search_chain_with_final_upto_len_sound_from(roots, attestations, revocations, subject, t, final_idx, depth, 0);
        let witness = choose|witness: Seq<usize>|
            0 < witness.len() <= depth as int
                && witness[witness.len() - 1] == final_idx as usize
                && chain_valid_at_spec(witness, roots, attestations, revocations, subject, t);
        let last = witness.len() - 1;
        chain_valid_requires_active_edges(witness, roots, attestations, revocations, subject, t, last as nat);
        invalid_attestation_is_inactive(attestations[final_idx as int], revocations, t);
        assert(false);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::web_of_trust_attestation::TRUST_ROLE_TAG;
    use crate::web_of_trust_attestation::attestation_active_at;
    use crate::web_of_trust_attestation::attestation_payload_bytes;
    use crate::web_of_trust_attestation::attestation_signature_bytes_runtime;
    use crate::web_of_trust_principal::principal_id_from_bytes;
    use crate::web_of_trust_revocation::FAMILY_TARGET_KIND_TAG;
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

    fn rooted_oracle_dfs(
        current: &PrincipalId,
        roots: &[PrincipalId],
        attestations: &[Attestation],
        revocations: &Vec<Revocation>,
        depth: usize,
        t: u64,
        visited: &mut Vec<Vec<u8>>,
    ) -> bool {
        if roots.iter().any(|root| root.digest == current.digest) {
            return true;
        }
        if depth == 0 {
            return false;
        }

        for att in attestations {
            if att.subject.digest != current.digest {
                continue;
            }
            if att.role_tag != TRUST_ROLE_TAG || !attestation_active_at(att, revocations, t) {
                continue;
            }
            if visited.contains(&att.issuer.digest) {
                continue;
            }
            visited.push(att.issuer.digest.clone());
            if rooted_oracle_dfs(&att.issuer, roots, attestations, revocations, depth - 1, t, visited) {
                return true;
            }
            visited.pop();
        }
        false
    }

    fn threshold_accepts_oracle(
        roots: &[PrincipalId],
        attestations: &[Attestation],
        revocations: &Vec<Revocation>,
        subject: &PrincipalId,
        k: usize,
        depth: usize,
        t: u64,
    ) -> bool {
        if k == 0 || depth == 0 {
            return false;
        }

        let mut seen: Vec<Vec<u8>> = Vec::new();
        let mut count = 0usize;
        for att in attestations {
            if att.subject.digest != subject.digest {
                continue;
            }
            if att.issuer.digest == subject.digest {
                continue;
            }
            if att.role_tag != TRUST_ROLE_TAG || !attestation_active_at(att, revocations, t) {
                continue;
            }
            if seen.contains(&att.issuer.digest) {
                continue;
            }
            let mut visited = vec![subject.digest.clone(), att.issuer.digest.clone()];
            if rooted_oracle_dfs(&att.issuer, roots, attestations, revocations, depth - 1, t, &mut visited) {
                seen.push(att.issuer.digest.clone());
                count += 1;
                if count >= k {
                    return true;
                }
            }
        }
        false
    }

    // r[verify web-of-trust.threshold]
    #[test]
    fn test_distinct_roots_satisfy_threshold_at_depth_one() {
        let a_key = vec![1, 2, 3];
        let b_key = vec![4, 5, 6];
        let subject_key = vec![7, 8, 9];
        let att1 = signed_attestation(a_key.clone(), subject_key.clone(), TRUST_ROLE_TAG, 0, 20, 1);
        let att2 = signed_attestation(b_key.clone(), subject_key.clone(), TRUST_ROLE_TAG, 0, 20, 2);
        let roots = vec![principal_id_from_bytes(&a_key), principal_id_from_bytes(&b_key)];
        let subject = principal_id_from_bytes(&subject_key);
        assert!(threshold_accepts(&roots, &vec![att1, att2], &Vec::new(), &subject, 2, 1, 5));
    }

    // r[verify web-of-trust.threshold]
    #[test]
    fn test_zero_threshold_rejects() {
        let root = principal_id_from_bytes(&[1, 2, 3]);
        let subject = principal_id_from_bytes(&[4, 5, 6]);
        assert!(!threshold_accepts(&vec![root], &Vec::new(), &Vec::new(), &subject, 0, 3, 5));
    }

    // r[verify web-of-trust.threshold]
    #[test]
    fn test_depth_zero_rejects_subject() {
        let root_key = vec![1, 2, 3];
        let subject_key = vec![4, 5, 6];
        let att = signed_attestation(root_key.clone(), subject_key.clone(), TRUST_ROLE_TAG, 0, 20, 1);
        let roots = vec![principal_id_from_bytes(&root_key)];
        let subject = principal_id_from_bytes(&subject_key);
        assert!(!threshold_accepts(&roots, &vec![att], &Vec::new(), &subject, 1, 0, 5));
    }

    // r[verify web-of-trust.threshold]
    #[test]
    fn test_root_subject_still_needs_introducers() {
        let root_key = vec![1, 2, 3];
        let root = principal_id_from_bytes(&root_key);
        let roots = vec![root];
        assert!(!threshold_accepts(&roots, &Vec::new(), &Vec::new(), &roots[0], 1, 1, 5));
    }

    // r[verify web-of-trust.threshold]
    #[test]
    fn test_too_small_depth_rejects_indirect_introducer() {
        let root_key = vec![1, 2, 3];
        let a_key = vec![4, 5, 6];
        let subject_key = vec![7, 8, 9];
        let att1 = signed_attestation(root_key.clone(), a_key.clone(), TRUST_ROLE_TAG, 0, 20, 1);
        let att2 = signed_attestation(a_key.clone(), subject_key.clone(), TRUST_ROLE_TAG, 0, 20, 2);
        let roots = vec![principal_id_from_bytes(&root_key)];
        let subject = principal_id_from_bytes(&subject_key);
        assert!(!threshold_accepts(&roots, &vec![att1, att2], &Vec::new(), &subject, 1, 1, 5));
    }

    // r[verify web-of-trust.threshold]
    #[test]
    fn test_repeated_final_hop_self_introduction_rejects() {
        let root_key = vec![1, 2, 3];
        let att = signed_attestation(root_key.clone(), root_key.clone(), TRUST_ROLE_TAG, 0, 20, 1);
        let roots = vec![principal_id_from_bytes(&root_key)];
        let subject = principal_id_from_bytes(&root_key);
        assert!(!threshold_accepts(&roots, &vec![att], &Vec::new(), &subject, 1, 1, 5));
    }

    // r[verify web-of-trust.threshold]
    #[test]
    fn test_duplicate_attestations_do_not_double_count() {
        let root_key = vec![1, 2, 3];
        let subject_key = vec![4, 5, 6];
        let att1 = signed_attestation(root_key.clone(), subject_key.clone(), TRUST_ROLE_TAG, 0, 20, 1);
        let att2 = signed_attestation(root_key.clone(), subject_key.clone(), TRUST_ROLE_TAG, 0, 20, 2);
        let roots = vec![principal_id_from_bytes(&root_key)];
        let subject = principal_id_from_bytes(&subject_key);
        assert!(!threshold_accepts(&roots, &vec![att1, att2], &Vec::new(), &subject, 2, 1, 5));
    }

    // r[verify web-of-trust.threshold]
    #[test]
    fn test_invalid_attestation_is_ignored_by_threshold() {
        let root_a_key = vec![1, 2, 3];
        let root_b_key = vec![4, 5, 6];
        let subject_key = vec![7, 8, 9];
        let att1 = signed_attestation(root_a_key.clone(), subject_key.clone(), TRUST_ROLE_TAG, 0, 20, 1);
        let att2 = signed_attestation(root_b_key.clone(), subject_key.clone(), TRUST_ROLE_TAG, 0, 20, 2);
        let invalid = Attestation {
            issuer_key_bytes: vec![4, 5, 0],
            ..att2
        };
        let roots = vec![
            principal_id_from_bytes(&root_a_key),
            principal_id_from_bytes(&root_b_key),
        ];
        let subject = principal_id_from_bytes(&subject_key);
        assert!(!threshold_accepts(&roots, &vec![att1, invalid], &Vec::new(), &subject, 2, 1, 5));
    }

    // r[verify web-of-trust.threshold]
    #[test]
    fn test_invalid_revocation_is_ignored_by_threshold() {
        let root_a_key = vec![1, 2, 3];
        let root_b_key = vec![4, 5, 6];
        let subject_key = vec![7, 8, 9];
        let att1 = signed_attestation(root_a_key.clone(), subject_key.clone(), TRUST_ROLE_TAG, 0, 20, 1);
        let att2 = signed_attestation(root_b_key.clone(), subject_key.clone(), TRUST_ROLE_TAG, 0, 20, 2);
        let rev = signed_family_revocation(root_b_key.clone(), subject_key.clone(), TRUST_ROLE_TAG, 5);
        let invalid = Revocation {
            family_subject_key_bytes: vec![7, 8, 0],
            ..rev
        };
        let roots = vec![
            principal_id_from_bytes(&root_a_key),
            principal_id_from_bytes(&root_b_key),
        ];
        let subject = principal_id_from_bytes(&subject_key);
        assert!(threshold_accepts(&roots, &vec![att1, att2], &vec![invalid], &subject, 2, 1, 5));
    }

    proptest! {
        // r[verify web-of-trust.threshold]
        #[test]
        fn prop_threshold_accepts_oracle(
            subject_choice in 0usize..4,
            k in 0usize..4,
            depth in 0usize..4,
            time in 0u64..20,
            duplicate_final in any::<bool>(),
        ) {
            let root_a_key = vec![1, 2, 3];
            let root_b_key = vec![4, 5, 6];
            let subject_key = vec![7, 8, 9];
            let self_key = vec![10, 11, 12];

            let mut attestations = vec![
                signed_attestation(root_a_key.clone(), subject_key.clone(), TRUST_ROLE_TAG, 0, 20, 1),
                signed_attestation(root_b_key.clone(), subject_key.clone(), TRUST_ROLE_TAG, 0, 20, 2),
                signed_attestation(root_a_key.clone(), root_b_key.clone(), TRUST_ROLE_TAG, 0, 20, 3),
                signed_attestation(root_a_key.clone(), self_key.clone(), TRUST_ROLE_TAG, 0, 20, 4),
                signed_attestation(self_key.clone(), subject_key.clone(), TRUST_ROLE_TAG, 0, 20, 5),
                signed_attestation(subject_key.clone(), subject_key.clone(), TRUST_ROLE_TAG, 0, 20, 6),
            ];
            if duplicate_final {
                attestations.push(signed_attestation(root_a_key.clone(), subject_key.clone(), TRUST_ROLE_TAG, 0, 20, 7));
            }
            let roots = vec![principal_id_from_bytes(&root_a_key), principal_id_from_bytes(&root_b_key)];
            let subjects = [
                principal_id_from_bytes(&root_a_key),
                principal_id_from_bytes(&root_b_key),
                principal_id_from_bytes(&subject_key),
                principal_id_from_bytes(&self_key),
            ];
            let subject = &subjects[subject_choice];
            let revocations = Vec::new();

            let oracle = threshold_accepts_oracle(&roots, &attestations, &revocations, subject, k, depth, time);
            prop_assert_eq!(threshold_accepts(&roots, &attestations, &revocations, subject, k, depth, time), oracle);
        }
    }
}
