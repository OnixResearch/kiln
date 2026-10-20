//! Verified web-of-trust rooted resolution.
//!
//! Resolves trust from a root set with a depth bound. The root base case
//! uses the empty witness; non-root resolutions use bounded search over
//! finite attestation-index witnesses.

#![allow(clippy::ptr_arg)]

use vstd::prelude::*;

use crate::web_of_trust_attestation::Attestation;
#[cfg(verus_keep_ghost)]
use crate::web_of_trust_attestation::attestation_record_ids_match_bytes_spec;
#[cfg(verus_keep_ghost)]
use crate::web_of_trust_attestation::invalid_attestation_is_inactive;
#[cfg(verus_keep_ghost)]
use crate::web_of_trust_chain::chain_indices_in_range_from_spec;
#[cfg(verus_keep_ghost)]
use crate::web_of_trust_chain::chain_indices_in_range_spec;
use crate::web_of_trust_chain::chain_valid_at;
#[cfg(verus_keep_ghost)]
use crate::web_of_trust_chain::chain_valid_at_spec;
#[cfg(verus_keep_ghost)]
use crate::web_of_trust_chain::chain_valid_requires_active_edges;
#[cfg(verus_keep_ghost)]
use crate::web_of_trust_chain::root_member_from_spec;
#[cfg(verus_keep_ghost)]
use crate::web_of_trust_chain::root_member_spec;
use crate::web_of_trust_principal::PrincipalId;
use crate::web_of_trust_revocation::Revocation;

verus! {

// r[depends web-of-trust.rooted-resolution]
/// Existential rooted-resolution semantics over attestation-index witnesses.
pub open spec fn resolves_trusted_witness_spec(
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    depth: nat,
    t: u64,
) -> bool {
    &&& root_member_spec(roots, subject)
        || exists|witness: Seq<usize>|
            0 < witness.len() <= depth as int
                && chain_valid_at_spec(witness, roots, attestations, revocations, subject, t)
}

// r[impl web-of-trust.rooted-resolution]
pub open spec fn exact_suffix_from_spec(
    prefix: Seq<usize>,
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    t: u64,
    remaining: nat,
    next: nat,
) -> bool {
    exists|suffix: Seq<usize>|
        suffix.len() == remaining as int
            && chain_valid_at_spec(prefix + suffix, roots, attestations, revocations, subject, t)
            && (
                remaining == 0
                    || suffix[0] == next as usize
                    || next < suffix[0] as nat
            )
}

// r[impl web-of-trust.rooted-resolution]
pub open spec fn search_exact_len_from_spec(
    prefix: Seq<usize>,
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    t: u64,
    remaining: nat,
    next: nat,
) -> bool
    recommends attestations.len() <= usize::MAX as nat
    decreases remaining, if next <= attestations.len() { attestations.len() - next } else { 0 }
{
    if remaining == 0 {
        chain_valid_at_spec(prefix, roots, attestations, revocations, subject, t)
    } else if next >= attestations.len() {
        false
    } else {
        search_exact_len_from_spec(
            prefix + seq![next as usize],
            roots,
            attestations,
            revocations,
            subject,
            t,
            (remaining - 1) as nat,
            0,
        ) || search_exact_len_from_spec(
            prefix,
            roots,
            attestations,
            revocations,
            subject,
            t,
            remaining,
            next + 1,
        )
    }
}

// r[impl web-of-trust.rooted-resolution]
pub open spec fn search_upto_len_from_spec(
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    depth: nat,
    t: u64,
    current: nat,
) -> bool
    recommends attestations.len() <= usize::MAX as nat
{
    exists|len: nat|
        current <= len <= depth
            && search_exact_len_from_spec(
                Seq::empty(),
                roots,
                attestations,
                revocations,
                subject,
                t,
                len,
                0,
            )
}

// r[depends web-of-trust.rooted-resolution]
/// Exact rooted-resolution decision with the empty-chain root base case.
pub open spec fn resolves_trusted_at_spec(
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    depth: nat,
    t: u64,
) -> bool
    recommends attestations.len() <= usize::MAX as nat
{
    &&& root_member_spec(roots, subject)
        || search_upto_len_from_spec(roots, attestations, revocations, subject, depth, t, 1)
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

fn search_exact_len_from(
    prefix: &Vec<usize>,
    roots: &Vec<PrincipalId>,
    attestations: &Vec<Attestation>,
    revocations: &Vec<Revocation>,
    subject: &PrincipalId,
    t: u64,
    remaining: usize,
    next: usize,
) -> (result: bool)
    requires next <= attestations.len()
    ensures result == search_exact_len_from_spec(prefix@, roots@, attestations@, revocations@, *subject, t, remaining as nat, next as nat)
    decreases remaining, attestations.len() - next
{
    if remaining == 0 {
        chain_valid_at(prefix, roots, attestations, revocations, subject, t)
    } else if next >= attestations.len() {
        false
    } else {
        let extended = clone_append(prefix, next);
        search_exact_len_from(&extended, roots, attestations, revocations, subject, t, remaining - 1, 0)
            || search_exact_len_from(prefix, roots, attestations, revocations, subject, t, remaining, next + 1)
    }
}

fn search_upto_len_from(
    roots: &Vec<PrincipalId>,
    attestations: &Vec<Attestation>,
    revocations: &Vec<Revocation>,
    subject: &PrincipalId,
    depth: usize,
    t: u64,
    current: usize,
) -> (result: bool)
    requires
        current >= 1,
        attestations@.len() <= usize::MAX as int,
    ensures result == search_upto_len_from_spec(roots@, attestations@, revocations@, *subject, depth as nat, t, current as nat)
    decreases if current <= depth { depth - current + 1 } else { 0 }
{
    proof {
        assert(attestations@.len() == attestations.len() as int);
        assert(attestations.len() as int <= usize::MAX as int);
    }
    if current > depth {
        false
    } else {
        let exact = search_exact_len_from(
            &Vec::new(),
            roots,
            attestations,
            revocations,
            subject,
            t,
            current,
            0,
        );
        if current < depth {
            exact || search_upto_len_from(roots, attestations, revocations, subject, depth, t, current + 1)
        } else {
            exact
        }
    }
}

// r[impl web-of-trust.rooted-resolution]
/// Resolve whether `subject` is trusted from `roots` at time `t` within `depth`.
pub fn resolves_trusted_at(
    roots: &Vec<PrincipalId>,
    attestations: &Vec<Attestation>,
    revocations: &Vec<Revocation>,
    subject: &PrincipalId,
    depth: usize,
    t: u64,
) -> (result: bool)
    requires attestations@.len() <= usize::MAX as int
    ensures result == resolves_trusted_at_spec(roots@, attestations@, revocations@, *subject, depth as nat, t)
{
    proof {
        assert(attestations@.len() == attestations.len() as int);
        assert(attestations.len() as int <= usize::MAX as int);
    }
    root_member_from(roots, subject, 0)
        || search_upto_len_from(roots, attestations, revocations, subject, depth, t, 1)
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

proof fn root_member_from_cons_shift(
    head: PrincipalId,
    tail: Seq<PrincipalId>,
    principal: PrincipalId,
    from: nat,
)
    ensures
        root_member_from_spec(seq![head] + tail, principal, from as int + 1)
            == root_member_from_spec(tail, principal, from as int),
    decreases if from < tail.len() { tail.len() - from } else { 0 }
{
    if from >= tail.len() {
    } else {
        root_member_from_cons_shift(head, tail, principal, from + 1);
    }
}

proof fn root_member_respects_digest_eq(
    roots: Seq<PrincipalId>,
    left: PrincipalId,
    right: PrincipalId,
    from: int,
)
    requires
        left.digest@ == right.digest@,
        0 <= from,
    ensures
        root_member_from_spec(roots, left, from) == root_member_from_spec(roots, right, from),
    decreases if from < roots.len() { roots.len() - from } else { 0 }
{
    if from >= roots.len() {
    } else {
        root_member_respects_digest_eq(roots, left, right, from + 1);
    }
}

proof fn root_member_prepend_existing_same(
    roots: Seq<PrincipalId>,
    duplicate: PrincipalId,
    principal: PrincipalId,
)
    requires root_member_spec(roots, duplicate)
    ensures root_member_spec(seq![duplicate] + roots, principal) == root_member_spec(roots, principal)
{
    root_member_from_cons_shift(duplicate, roots, principal, 0);
    assert(root_member_spec(seq![duplicate] + roots, principal)
        == (duplicate.digest@ == principal.digest@ || root_member_spec(roots, principal)));
    if duplicate.digest@ == principal.digest@ {
        root_member_respects_digest_eq(roots, duplicate, principal, 0);
        assert(root_member_spec(roots, principal));
    }
}

proof fn search_exact_len_sound_from(
    prefix: Seq<usize>,
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    t: u64,
    remaining: nat,
    next: nat,
)
    requires
        attestations.len() <= usize::MAX as nat,
        next <= attestations.len(),
        search_exact_len_from_spec(prefix, roots, attestations, revocations, subject, t, remaining, next),
    ensures
        exact_suffix_from_spec(prefix, roots, attestations, revocations, subject, t, remaining, next),
    decreases remaining, if next <= attestations.len() { attestations.len() - next } else { 0 }
{
    if remaining == 0 {
        assert(chain_valid_at_spec(prefix, roots, attestations, revocations, subject, t));
        let suffix = Seq::<usize>::empty();
        assert(prefix + suffix == prefix);
        assert(suffix.len() == 0);
        assert(chain_valid_at_spec(prefix + suffix, roots, attestations, revocations, subject, t));
        assert(exact_suffix_from_spec(prefix, roots, attestations, revocations, subject, t, remaining, next));
    } else if next >= attestations.len() {
        assert(false);
    } else if search_exact_len_from_spec(
        prefix + seq![next as usize],
        roots,
        attestations,
        revocations,
        subject,
        t,
        (remaining - 1) as nat,
        0,
    ) {
        search_exact_len_sound_from(
            prefix + seq![next as usize],
            roots,
            attestations,
            revocations,
            subject,
            t,
            (remaining - 1) as nat,
            0,
        );
        let tail = choose|tail: Seq<usize>|
            tail.len() == (remaining - 1) as int
                && chain_valid_at_spec((prefix + seq![next as usize]) + tail, roots, attestations, revocations, subject, t)
                && (((remaining - 1) as nat) == 0 || tail[0] == 0usize || 0 < tail[0] as nat);
        let suffix = seq![next as usize] + tail;
        assert(suffix.len() == remaining as int);
        assert(prefix + suffix == (prefix + seq![next as usize]) + tail);
        assert(chain_valid_at_spec(prefix + suffix, roots, attestations, revocations, subject, t));
        assert(suffix[0] == next as usize);
        assert(exact_suffix_from_spec(prefix, roots, attestations, revocations, subject, t, remaining, next));
    } else {
        search_exact_len_sound_from(prefix, roots, attestations, revocations, subject, t, remaining, next + 1);
        let suffix = choose|suffix: Seq<usize>|
            suffix.len() == remaining as int
                && chain_valid_at_spec(prefix + suffix, roots, attestations, revocations, subject, t)
                && (remaining == 0 || suffix[0] == (next + 1) as usize || (next + 1) < suffix[0] as nat);
        assert(suffix.len() == remaining as int);
        assert(chain_valid_at_spec(prefix + suffix, roots, attestations, revocations, subject, t));
        if remaining > 0 {
            if suffix[0] == (next + 1) as usize {
                assert(next + 1 <= usize::MAX as nat);
                assert(next < suffix[0] as nat);
            } else {
                assert((next + 1) < suffix[0] as nat);
                assert(next < suffix[0] as nat);
            }
        }
        assert(exact_suffix_from_spec(prefix, roots, attestations, revocations, subject, t, remaining, next));
    }
}

proof fn search_exact_len_complete_from(
    prefix: Seq<usize>,
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    t: u64,
    remaining: nat,
    next: nat,
    suffix: Seq<usize>,
)
    requires
        attestations.len() <= usize::MAX as nat,
        next <= attestations.len(),
        suffix.len() == remaining as int,
        chain_valid_at_spec(prefix + suffix, roots, attestations, revocations, subject, t),
        remaining == 0 || suffix[0] == next as usize || next < suffix[0] as nat,
    ensures
        search_exact_len_from_spec(prefix, roots, attestations, revocations, subject, t, remaining, next),
    decreases remaining, if remaining > 0 && next <= suffix[0] as nat { suffix[0] as nat - next + 1 } else { 0 }
{
    if remaining == 0 {
    } else {
        let witness = prefix + suffix;
        assert(chain_indices_in_range_spec(witness, attestations));
        chain_indices_in_range_from_implies_index(witness, attestations, 0, prefix.len() as int);
        assert((suffix[0] as int) < attestations.len());
        let first = suffix[0] as nat;
        if suffix[0] == next as usize {
            seq_head_tail_usize(suffix);
            let tail = suffix.subrange(1, suffix.len() as int);
            assert((prefix + seq![first as usize]) + tail == prefix + suffix);
            assert(tail.len() == (remaining - 1) as int);
            assert(chain_valid_at_spec((prefix + seq![first as usize]) + tail, roots, attestations, revocations, subject, t));
            if tail.len() > 0 {
                let new_prefix = prefix + seq![first as usize];
                let new_witness = new_prefix + tail;
                assert(new_witness == witness);
                chain_indices_in_range_from_implies_index(new_witness, attestations, 0, new_prefix.len() as int);
                assert((tail[0] as int) < attestations.len());
            }
            search_exact_len_complete_from(
                prefix + seq![first as usize],
                roots,
                attestations,
                revocations,
                subject,
                t,
                (remaining - 1) as nat,
                0,
                tail,
            );
        } else {
            search_exact_len_complete_from(prefix, roots, attestations, revocations, subject, t, remaining, next + 1, suffix);
        }
    }
}

proof fn search_upto_len_sound_from(
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    depth: nat,
    t: u64,
    current: nat,
)
    requires
        attestations.len() <= usize::MAX as nat,
        current >= 1,
        search_upto_len_from_spec(roots, attestations, revocations, subject, depth, t, current),
    ensures
        exists|witness: Seq<usize>|
            0 < witness.len() <= depth as int
                && chain_valid_at_spec(witness, roots, attestations, revocations, subject, t),
    decreases if current <= depth { depth - current + 1 } else { 0 }
{
    if current > depth {
        assert(false);
    } else if search_exact_len_from_spec(Seq::empty(), roots, attestations, revocations, subject, t, current, 0) {
        search_exact_len_sound_from(Seq::empty(), roots, attestations, revocations, subject, t, current, 0);
        let witness = choose|witness: Seq<usize>|
            witness.len() == current as int
                && chain_valid_at_spec(witness, roots, attestations, revocations, subject, t)
                && (current == 0 || 0 <= witness[0] as nat);
        assert(0 < witness.len());
        assert(witness.len() <= depth as int);
    } else {
        search_upto_len_sound_from(roots, attestations, revocations, subject, depth, t, current + 1);
    }
}

proof fn search_upto_len_complete_from(
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    depth: nat,
    t: u64,
    current: nat,
    witness: Seq<usize>,
)
    requires
        attestations.len() <= usize::MAX as nat,
        current >= 1,
        current <= witness.len() as nat,
        witness.len() <= depth as int,
        chain_valid_at_spec(witness, roots, attestations, revocations, subject, t),
    ensures
        search_upto_len_from_spec(roots, attestations, revocations, subject, depth, t, current),
    decreases if current <= witness.len() as nat { witness.len() as nat - current + 1 } else { 0 }
{
    if current == witness.len() as nat {
        search_exact_len_complete_from(Seq::empty(), roots, attestations, revocations, subject, t, current, 0, witness);
    } else {
        search_upto_len_complete_from(roots, attestations, revocations, subject, depth, t, current + 1, witness);
    }
}

// r[verify web-of-trust.rooted-resolution]
/// Rooted-resolution search is sound for the existential witness semantics.
pub proof fn resolves_trusted_at_sound(
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    depth: nat,
    t: u64,
)
    requires
        attestations.len() <= usize::MAX as nat,
        resolves_trusted_at_spec(roots, attestations, revocations, subject, depth, t)
    ensures resolves_trusted_witness_spec(roots, attestations, revocations, subject, depth, t)
{
    if root_member_spec(roots, subject) {
    } else {
        search_upto_len_sound_from(roots, attestations, revocations, subject, depth, t, 1);
    }
}

// r[verify web-of-trust.rooted-resolution]
/// Every existential rooted-resolution witness is found by the bounded search.
pub proof fn resolves_trusted_at_complete(
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    depth: nat,
    t: u64,
)
    requires
        attestations.len() <= usize::MAX as nat,
        resolves_trusted_witness_spec(roots, attestations, revocations, subject, depth, t)
    ensures resolves_trusted_at_spec(roots, attestations, revocations, subject, depth, t)
{
    if root_member_spec(roots, subject) {
    } else {
        let witness = choose|witness: Seq<usize>|
            0 < witness.len() <= depth as int
                && chain_valid_at_spec(witness, roots, attestations, revocations, subject, t);
        search_upto_len_complete_from(roots, attestations, revocations, subject, depth, t, 1, witness);
    }
}

// r[verify web-of-trust.rooted-resolution]
/// Prepending an already-present root does not change rooted resolution.
pub proof fn duplicate_roots_ignored(
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    depth: nat,
    t: u64,
    duplicate: PrincipalId,
)
    requires
        attestations.len() <= usize::MAX as nat,
        root_member_spec(roots, duplicate)
    ensures
        resolves_trusted_at_spec(seq![duplicate] + roots, attestations, revocations, subject, depth, t)
            == resolves_trusted_at_spec(roots, attestations, revocations, subject, depth, t)
{
    assert(resolves_trusted_at_spec(seq![duplicate] + roots, attestations, revocations, subject, depth, t)
        ==> resolves_trusted_at_spec(roots, attestations, revocations, subject, depth, t)) by {
        if resolves_trusted_at_spec(seq![duplicate] + roots, attestations, revocations, subject, depth, t) {
            resolves_trusted_at_sound(seq![duplicate] + roots, attestations, revocations, subject, depth, t);
            if root_member_spec(seq![duplicate] + roots, subject) {
                root_member_prepend_existing_same(roots, duplicate, subject);
            } else {
                let witness = choose|witness: Seq<usize>|
                    0 < witness.len() <= depth as int
                        && chain_valid_at_spec(witness, seq![duplicate] + roots, attestations, revocations, subject, t);
                root_member_prepend_existing_same(roots, duplicate, attestations[witness[0] as int].issuer);
                assert(chain_valid_at_spec(witness, roots, attestations, revocations, subject, t));
                assert(resolves_trusted_witness_spec(roots, attestations, revocations, subject, depth, t));
                resolves_trusted_at_complete(roots, attestations, revocations, subject, depth, t);
            }
        }
    }
    assert(resolves_trusted_at_spec(roots, attestations, revocations, subject, depth, t)
        ==> resolves_trusted_at_spec(seq![duplicate] + roots, attestations, revocations, subject, depth, t)) by {
        if resolves_trusted_at_spec(roots, attestations, revocations, subject, depth, t) {
            resolves_trusted_at_sound(roots, attestations, revocations, subject, depth, t);
            if root_member_spec(roots, subject) {
                root_member_prepend_existing_same(roots, duplicate, subject);
            } else {
                let witness = choose|witness: Seq<usize>|
                    0 < witness.len() <= depth as int
                        && chain_valid_at_spec(witness, roots, attestations, revocations, subject, t);
                root_member_prepend_existing_same(roots, duplicate, attestations[witness[0] as int].issuer);
                assert(chain_valid_at_spec(witness, seq![duplicate] + roots, attestations, revocations, subject, t));
                assert(resolves_trusted_witness_spec(seq![duplicate] + roots, attestations, revocations, subject, depth, t));
                resolves_trusted_at_complete(seq![duplicate] + roots, attestations, revocations, subject, depth, t);
            }
        }
    }
}

// r[verify web-of-trust.rooted-resolution]
/// Any witness that uses an invalid attestation record is ignored by rooted resolution.
pub proof fn invalid_records_ignored_by_resolution(
    witness: Seq<usize>,
    roots: Seq<PrincipalId>,
    attestations: Seq<Attestation>,
    revocations: Seq<Revocation>,
    subject: PrincipalId,
    t: u64,
    pos: nat,
)
    requires
        pos < witness.len(),
        (witness[pos as int] as int) < attestations.len(),
        !attestation_record_ids_match_bytes_spec(attestations[witness[pos as int] as int]),
    ensures
        !chain_valid_at_spec(witness, roots, attestations, revocations, subject, t)
{
    if chain_valid_at_spec(witness, roots, attestations, revocations, subject, t) {
        chain_valid_requires_active_edges(witness, roots, attestations, revocations, subject, t, pos);
        invalid_attestation_is_inactive(attestations[witness[pos as int] as int], revocations, t);
        assert(false);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::web_of_trust_attestation::Attestation;
    use crate::web_of_trust_attestation::TRUST_ROLE_TAG;
    use crate::web_of_trust_attestation::attestation_active_at;
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

    fn rooted_resolution_oracle_dfs(
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
            if rooted_resolution_oracle_dfs(&att.issuer, roots, attestations, revocations, depth - 1, t, visited) {
                return true;
            }
            visited.pop();
        }
        false
    }

    fn rooted_resolution_oracle(
        roots: &[PrincipalId],
        attestations: &[Attestation],
        revocations: &Vec<Revocation>,
        subject: &PrincipalId,
        depth: usize,
        t: u64,
    ) -> bool {
        if roots.iter().any(|root| root.digest == subject.digest) {
            return true;
        }

        let mut visited = vec![subject.digest.clone()];
        rooted_resolution_oracle_dfs(subject, roots, attestations, revocations, depth, t, &mut visited)
    }

    // r[verify web-of-trust.rooted-resolution]
    #[test]
    fn test_root_resolves_at_depth_zero() {
        let root = principal_id_from_bytes(&[1, 2, 3]);
        let roots = vec![root];
        assert!(resolves_trusted_at(&roots, &Vec::new(), &Vec::new(), &roots[0], 0, 5));
    }

    // r[verify web-of-trust.rooted-resolution]
    #[test]
    fn test_direct_root_cert_resolves_at_depth_one() {
        let root_key = vec![1, 2, 3];
        let subject_key = vec![4, 5, 6];
        let att = signed_attestation(root_key.clone(), subject_key.clone(), TRUST_ROLE_TAG, 0, 20, 1);
        let roots = vec![principal_id_from_bytes(&root_key)];
        let subject = principal_id_from_bytes(&subject_key);
        assert!(resolves_trusted_at(&roots, &vec![att], &Vec::new(), &subject, 1, 5));
    }

    // r[verify web-of-trust.rooted-resolution]
    #[test]
    fn test_existing_valid_chain_succeeds() {
        let root_key = vec![1, 2, 3];
        let mid_key = vec![4, 5, 6];
        let subject_key = vec![7, 8, 9];
        let att1 = signed_attestation(root_key.clone(), mid_key.clone(), TRUST_ROLE_TAG, 0, 20, 1);
        let att2 = signed_attestation(mid_key, subject_key.clone(), TRUST_ROLE_TAG, 0, 20, 2);
        let roots = vec![principal_id_from_bytes(&root_key)];
        let subject = principal_id_from_bytes(&subject_key);
        assert!(resolves_trusted_at(&roots, &vec![att1, att2], &Vec::new(), &subject, 2, 5));
    }

    // r[verify web-of-trust.rooted-resolution]
    #[test]
    fn test_too_small_depth_rejects_longer_path() {
        let root_key = vec![1, 2, 3];
        let a_key = vec![4, 5, 6];
        let b_key = vec![7, 8, 9];
        let subject_key = vec![10, 11, 12];
        let att1 = signed_attestation(root_key.clone(), a_key.clone(), TRUST_ROLE_TAG, 0, 20, 1);
        let att2 = signed_attestation(a_key, b_key.clone(), TRUST_ROLE_TAG, 0, 20, 2);
        let att3 = signed_attestation(b_key, subject_key.clone(), TRUST_ROLE_TAG, 0, 20, 3);
        let roots = vec![principal_id_from_bytes(&root_key)];
        let subject = principal_id_from_bytes(&subject_key);
        assert!(!resolves_trusted_at(&roots, &vec![att1, att2, att3], &Vec::new(), &subject, 2, 5));
    }

    // r[verify web-of-trust.rooted-resolution]
    #[test]
    fn test_duplicate_roots_are_ignored() {
        let root_key = vec![1, 2, 3];
        let subject_key = vec![4, 5, 6];
        let att = signed_attestation(root_key.clone(), subject_key.clone(), TRUST_ROLE_TAG, 0, 20, 1);
        let root = principal_id_from_bytes(&root_key);
        let roots = vec![root, principal_id_from_bytes(&root_key)];
        let subject = principal_id_from_bytes(&subject_key);
        let attestations = vec![att];
        assert_eq!(
            resolves_trusted_at(&roots, &attestations, &Vec::new(), &subject, 1, 5),
            resolves_trusted_at(&vec![principal_id_from_bytes(&root_key)], &attestations, &Vec::new(), &subject, 1, 5),
        );
    }

    // r[verify web-of-trust.rooted-resolution]
    #[test]
    fn test_invalid_attestation_is_ignored_by_resolution() {
        let root_key = vec![1, 2, 3];
        let mid_key = vec![4, 5, 6];
        let subject_key = vec![7, 8, 9];
        let att1 = signed_attestation(root_key.clone(), mid_key.clone(), TRUST_ROLE_TAG, 0, 20, 1);
        let att2 = signed_attestation(mid_key, subject_key.clone(), TRUST_ROLE_TAG, 0, 20, 2);
        let invalid = Attestation {
            subject_key_bytes: vec![7, 8, 0],
            ..att2
        };
        let roots = vec![principal_id_from_bytes(&root_key)];
        let subject = principal_id_from_bytes(&subject_key);
        assert!(!resolves_trusted_at(&roots, &vec![att1, invalid], &Vec::new(), &subject, 2, 5));
    }

    // r[verify web-of-trust.rooted-resolution]
    #[test]
    fn test_invalid_revocation_is_ignored_by_resolution() {
        let root_key = vec![1, 2, 3];
        let subject_key = vec![4, 5, 6];
        let att = signed_attestation(root_key.clone(), subject_key.clone(), TRUST_ROLE_TAG, 0, 20, 1);
        let rev = signed_family_revocation(root_key.clone(), subject_key.clone(), TRUST_ROLE_TAG, 5);
        let invalid = Revocation {
            family_subject_key_bytes: vec![4, 5, 0],
            ..rev
        };
        let roots = vec![principal_id_from_bytes(&root_key)];
        let subject = principal_id_from_bytes(&subject_key);
        assert!(resolves_trusted_at(&roots, &vec![att], &vec![invalid], &subject, 1, 5));
    }

    proptest! {
        // r[verify web-of-trust.rooted-resolution]
        #[test]
        fn prop_rooted_resolution_oracle(
            subject_choice in 0usize..4,
            depth in 0usize..4,
            time in 0u64..20,
            duplicate_root in any::<bool>(),
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
            let mut roots = vec![principal_id_from_bytes(&root_key)];
            if duplicate_root {
                roots.push(principal_id_from_bytes(&root_key));
            }
            let subjects = [
                principal_id_from_bytes(&root_key),
                principal_id_from_bytes(&a_key),
                principal_id_from_bytes(&b_key),
                principal_id_from_bytes(&c_key),
            ];
            let subject = &subjects[subject_choice];
            let revocations = Vec::new();

            let oracle = rooted_resolution_oracle(&roots, &attestations, &revocations, subject, depth, time);
            prop_assert_eq!(resolves_trusted_at(&roots, &attestations, &revocations, subject, depth, time), oracle);
        }
    }
}
