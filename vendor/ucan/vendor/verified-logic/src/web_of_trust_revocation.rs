//! Verified web-of-trust revocation payloads and matching semantics.
//!
//! Models exact canonical payload bytes for revocations over
//! `(revocation_issuer_bytes, target_kind_tag, target_encoding,
//! effective_time)` plus same-issuer exact-payload and family-target
//! matching.

#![allow(clippy::ptr_arg)]

use vstd::prelude::*;

use crate::serialize_inj::encode_u64;
#[cfg(verus_keep_ghost)]
use crate::serialize_inj::encode_u64_spec;
use crate::web_of_trust_attestation::Attestation;
use crate::web_of_trust_attestation::attestation_payload_bytes;
use crate::web_of_trust_attestation::attestation_payload_matches;
#[cfg(verus_keep_ghost)]
use crate::web_of_trust_attestation::attestation_payload_of_spec;
#[cfg(verus_keep_ghost)]
use crate::web_of_trust_attestation::attestation_record_ids_match_bytes_spec;
#[cfg(verus_keep_ghost)]
use crate::web_of_trust_attestation::canonical_byte_string_spec;
use crate::web_of_trust_attestation::encode_u32_le;
#[cfg(verus_keep_ghost)]
use crate::web_of_trust_attestation::encode_u32_le_spec;
#[cfg(verus_keep_ghost)]
use crate::web_of_trust_attestation::length_fits_u32_spec;
use crate::web_of_trust_principal::PrincipalId;
#[cfg(verus_keep_ghost)]
use crate::web_of_trust_principal::principal_id_bytes_spec;

#[cfg(verus_keep_ghost)]
pub(crate) fn revocation_signature_bytes_runtime(_issuer_key_bytes: &[u8], _payload_bytes: &[u8]) -> Vec<u8> {
    Vec::new()
}

#[cfg(not(verus_keep_ghost))]
#[allow(dead_code)]
pub(crate) fn revocation_signature_bytes_runtime(issuer_key_bytes: &[u8], payload_bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(0xa2);
    let issuer_len = u32::try_from(issuer_key_bytes.len()).expect("issuer key bytes must fit u32");
    out.extend_from_slice(&issuer_len.to_le_bytes());
    out.extend_from_slice(issuer_key_bytes);
    out.extend_from_slice(payload_bytes);
    out
}

verus! {

pub const EXACT_PAYLOAD_TARGET_KIND_TAG: u8 = 0x01;
pub const FAMILY_TARGET_KIND_TAG: u8 = 0x02;
pub const REVOCATION_SIGNATURE_DOMAIN_TAG: u8 = 0xa2;

/// Signed revocation record.
pub struct Revocation {
    pub issuer_key_bytes: Vec<u8>,
    pub issuer: PrincipalId,
    pub target_kind_tag: u8,
    pub exact_payload_target: Vec<u8>,
    pub family_subject_key_bytes: Vec<u8>,
    pub family_subject: PrincipalId,
    pub family_role_tag: u8,
    pub effective_time: u64,
    pub signature: Vec<u8>,
}

// r[impl web-of-trust.revocation]
pub open spec fn family_target_encoding_spec(
    issuer_key_bytes: Seq<u8>,
    subject_key_bytes: Seq<u8>,
    role_tag: u8,
) -> Option<Seq<u8>> {
    match (canonical_byte_string_spec(issuer_key_bytes), canonical_byte_string_spec(subject_key_bytes)) {
        (Some(issuer_enc), Some(subject_enc)) => Some(issuer_enc + subject_enc + seq![role_tag]),
        _ => None,
    }
}

// r[impl web-of-trust.revocation]
pub open spec fn exact_payload_target_encoding_spec(exact_payload_target: Seq<u8>) -> Option<Seq<u8>> {
    canonical_byte_string_spec(exact_payload_target)
}

// r[impl web-of-trust.revocation]
pub open spec fn revocation_target_encoding_spec(rev: Revocation) -> Option<Seq<u8>> {
    if rev.target_kind_tag == EXACT_PAYLOAD_TARGET_KIND_TAG {
        exact_payload_target_encoding_spec(rev.exact_payload_target@)
    } else if rev.target_kind_tag == FAMILY_TARGET_KIND_TAG {
        family_target_encoding_spec(rev.issuer_key_bytes@, rev.family_subject_key_bytes@, rev.family_role_tag)
    } else {
        None
    }
}

// r[depends web-of-trust.revocation]
/// Canonical revocation payload-byte grammar.
pub open spec fn revocation_payload_bytes_spec(
    issuer_key_bytes: Seq<u8>,
    target_kind_tag: u8,
    target_encoding: Seq<u8>,
    effective_time: u64,
) -> Option<Seq<u8>> {
    match (canonical_byte_string_spec(issuer_key_bytes), canonical_byte_string_spec(target_encoding)) {
        (Some(issuer_enc), Some(target_enc)) => Some(
            issuer_enc
                + seq![target_kind_tag]
                + target_enc
                + encode_u64_spec(effective_time),
        ),
        _ => None,
    }
}

// r[depends web-of-trust.revocation]
/// Canonical payload for a full revocation record.
pub open spec fn revocation_payload_of_spec(rev: Revocation) -> Option<Seq<u8>> {
    match revocation_target_encoding_spec(rev) {
        Some(target_encoding) => revocation_payload_bytes_spec(
            rev.issuer_key_bytes@,
            rev.target_kind_tag,
            target_encoding,
            rev.effective_time,
        ),
        None => None,
    }
}

// r[impl web-of-trust.revocation]
pub open spec fn revocation_record_ids_match_bytes_spec(rev: Revocation) -> bool {
    &&& rev.issuer.digest@ == principal_id_bytes_spec(rev.issuer_key_bytes@)
    &&& rev.family_subject.digest@ == principal_id_bytes_spec(rev.family_subject_key_bytes@)
}

// r[impl web-of-trust.revocation]
pub open spec fn revocation_signature_input_spec(
    issuer_key_bytes: Seq<u8>,
    payload_bytes: Seq<u8>,
) -> Option<Seq<u8>> {
    match canonical_byte_string_spec(issuer_key_bytes) {
        Some(issuer_enc) => Some(seq![REVOCATION_SIGNATURE_DOMAIN_TAG] + issuer_enc + payload_bytes),
        None => None,
    }
}

// r[depends web-of-trust.revocation]
/// Exact revocation-signature validity surface over canonical bytes.
pub open spec fn revocation_signature_valid_bytes_spec(
    issuer_key_bytes: Seq<u8>,
    payload_bytes: Seq<u8>,
    signature: Seq<u8>,
) -> bool {
    match revocation_signature_input_spec(issuer_key_bytes, payload_bytes) {
        Some(expected) => signature == expected,
        None => false,
    }
}

// r[depends web-of-trust.revocation]
/// Exact revocation-signature validity for a full record.
pub open spec fn revocation_signature_valid_spec(rev: Revocation) -> bool {
    match revocation_payload_of_spec(rev) {
        Some(payload) => revocation_signature_valid_bytes_spec(rev.issuer_key_bytes@, payload, rev.signature@),
        None => false,
    }
}

// r[impl web-of-trust.revocation]
pub open spec fn same_principal_spec(rev: Revocation, att: Attestation) -> bool {
    rev.issuer.digest@ == att.issuer.digest@
}

// r[impl web-of-trust.revocation]
pub open spec fn revocation_matches_exact_payload_spec(rev: Revocation, att: Attestation) -> bool {
    &&& rev.target_kind_tag == EXACT_PAYLOAD_TARGET_KIND_TAG
    &&& attestation_payload_of_spec(att) == Some(rev.exact_payload_target@)
    &&& same_principal_spec(rev, att)
}

// r[impl web-of-trust.revocation]
pub open spec fn revocation_matches_family_spec(rev: Revocation, att: Attestation) -> bool {
    &&& rev.target_kind_tag == FAMILY_TARGET_KIND_TAG
    &&& same_principal_spec(rev, att)
    &&& rev.family_subject.digest@ == att.subject.digest@
    &&& rev.family_role_tag == att.role_tag
}

// r[depends web-of-trust.revocation]
/// Semantic target match for exact-payload and family revocations.
pub open spec fn revocation_matches_spec(rev: Revocation, att: Attestation) -> bool {
    if rev.target_kind_tag == EXACT_PAYLOAD_TARGET_KIND_TAG {
        match attestation_payload_of_spec(att) {
            Some(payload) => rev.issuer.digest@ == att.issuer.digest@ && rev.exact_payload_target@ == payload,
            None => false,
        }
    } else if rev.target_kind_tag == FAMILY_TARGET_KIND_TAG {
        rev.issuer.digest@ == att.issuer.digest@
            && rev.family_subject.digest@ == att.subject.digest@
            && rev.family_role_tag == att.role_tag
    } else {
        false
    }
}

// r[depends web-of-trust.revocation]
/// Effective revocation semantics at evaluation time `t`.
pub open spec fn revocation_effective_at_spec(rev: Revocation, att: Attestation, t: u64) -> bool {
    &&& revocation_record_ids_match_bytes_spec(rev)
    &&& attestation_record_ids_match_bytes_spec(att)
    &&& revocation_signature_valid_spec(rev)
    &&& revocation_matches_spec(rev, att)
    &&& rev.effective_time <= t
}

proof fn concat_right_cancel(prefix: Seq<u8>, a: Seq<u8>, b: Seq<u8>)
    requires
        prefix + a == prefix + b,
    ensures
        a == b,
{
    assert((prefix + a).len() == prefix.len() + a.len());
    assert((prefix + b).len() == prefix.len() + b.len());
    assert(a.len() == b.len());
    assert forall|i: int| 0 <= i < a.len() implies a[i] == b[i] by {
        assert((prefix + a)[prefix.len() + i] == a[i]);
        assert((prefix + b)[prefix.len() + i] == b[i]);
    }
    assert(a =~= b);
}

fn append_vec(dst: &mut Vec<u8>, src: &Vec<u8>)
    ensures dst@ == old(dst)@ + src@
{
    let ghost old_dst = dst@;
    let mut i: usize = 0;
    while i < src.len()
        invariant
            i <= src.len(),
            dst@ == old_dst + src@.subrange(0, i as int),
        decreases src.len() - i,
    {
        dst.push(src[i]);
        i += 1;
    }
}

fn bytes_equal(a: &Vec<u8>, b: &Vec<u8>) -> (result: bool)
    ensures result == (a@ =~= b@)
{
    if a.len() != b.len() {
        false
    } else {
        let mut i: usize = 0;
        let mut eq = true;
        while i < a.len()
            invariant
                i <= a.len(),
                a.len() == b.len(),
                eq == (forall|j: int| 0 <= j < i as int ==> a@[j] == b@[j]),
            decreases a.len() - i,
        {
            eq = eq && a[i] == b[i];
            i += 1;
        }
        eq
    }
}

#[allow(clippy::checked_conversions)]
fn length_fits_u32(len: usize) -> (result: bool)
    ensures result == length_fits_u32_spec(len as nat)
{
    len <= u32::MAX as usize
}

#[allow(clippy::cast_possible_truncation)]
fn canonical_byte_string(bytes: &Vec<u8>) -> (result: Option<Vec<u8>>)
    ensures match result {
        Some(v) => canonical_byte_string_spec(bytes@) == Some(v@),
        None => canonical_byte_string_spec(bytes@).is_none(),
    }
{
    if !length_fits_u32(bytes.len()) {
        return None;
    }

    let len_u32 = bytes.len() as u32;
    let mut result = encode_u32_le(len_u32);
    let ghost prefix = encode_u32_le_spec(len_u32);
    let mut i: usize = 0;
    while i < bytes.len()
        invariant
            i <= bytes.len(),
            bytes.len() == len_u32 as usize,
            result@ == prefix + bytes@.subrange(0, i as int),
        decreases bytes.len() - i,
    {
        result.push(bytes[i]);
        i += 1;
    }
    Some(result)
}

#[allow(clippy::question_mark)]
fn family_target_encoding(rev: &Revocation) -> (result: Option<Vec<u8>>)
    ensures match result {
        Some(v) => family_target_encoding_spec(rev.issuer_key_bytes@, rev.family_subject_key_bytes@, rev.family_role_tag) == Some(v@),
        None => family_target_encoding_spec(rev.issuer_key_bytes@, rev.family_subject_key_bytes@, rev.family_role_tag).is_none(),
    }
{
    let issuer_enc = canonical_byte_string(&rev.issuer_key_bytes);
    if issuer_enc.is_none() {
        return None;
    }
    let subject_enc = canonical_byte_string(&rev.family_subject_key_bytes);
    if subject_enc.is_none() {
        return None;
    }

    let mut result = issuer_enc.unwrap();
    let subject = subject_enc.unwrap();
    append_vec(&mut result, &subject);
    result.push(rev.family_role_tag);
    Some(result)
}

fn exact_payload_target_encoding(rev: &Revocation) -> (result: Option<Vec<u8>>)
    ensures match result {
        Some(v) => exact_payload_target_encoding_spec(rev.exact_payload_target@) == Some(v@),
        None => exact_payload_target_encoding_spec(rev.exact_payload_target@).is_none(),
    }
{
    canonical_byte_string(&rev.exact_payload_target)
}

fn revocation_target_encoding(rev: &Revocation) -> (result: Option<Vec<u8>>)
    ensures match result {
        Some(v) => revocation_target_encoding_spec(*rev) == Some(v@),
        None => revocation_target_encoding_spec(*rev).is_none(),
    }
{
    if rev.target_kind_tag == EXACT_PAYLOAD_TARGET_KIND_TAG {
        exact_payload_target_encoding(rev)
    } else if rev.target_kind_tag == FAMILY_TARGET_KIND_TAG {
        family_target_encoding(rev)
    } else {
        None
    }
}

// r[impl web-of-trust.revocation]
/// Exact canonical payload-byte builder for a revocation record.
#[allow(clippy::question_mark)]
pub fn revocation_payload_bytes(rev: &Revocation) -> (result: Option<Vec<u8>>)
    ensures match result {
        Some(v) => revocation_payload_of_spec(*rev) == Some(v@),
        None => revocation_payload_of_spec(*rev).is_none(),
    }
{
    let issuer_enc = canonical_byte_string(&rev.issuer_key_bytes);
    if issuer_enc.is_none() {
        return None;
    }
    let target_encoding = revocation_target_encoding(rev);
    if target_encoding.is_none() {
        return None;
    }
    let target_enc_outer = canonical_byte_string(&target_encoding.unwrap());
    if target_enc_outer.is_none() {
        return None;
    }

    let mut result = issuer_enc.unwrap();
    result.push(rev.target_kind_tag);
    let target_bytes = target_enc_outer.unwrap();
    append_vec(&mut result, &target_bytes);
    let effective_time = encode_u64(rev.effective_time);
    append_vec(&mut result, &effective_time);
    Some(result)
}

#[allow(clippy::question_mark)]
fn revocation_signature_input_bytes(
    issuer_key_bytes: &Vec<u8>,
    payload_bytes: &Vec<u8>,
) -> (result: Option<Vec<u8>>)
    ensures match result {
        Some(v) => revocation_signature_input_spec(issuer_key_bytes@, payload_bytes@) == Some(v@),
        None => revocation_signature_input_spec(issuer_key_bytes@, payload_bytes@).is_none(),
    }
{
    let issuer_enc = canonical_byte_string(issuer_key_bytes);
    if issuer_enc.is_none() {
        return None;
    }

    let mut result = vec![REVOCATION_SIGNATURE_DOMAIN_TAG];
    proof {
        assert(result@ =~= seq![REVOCATION_SIGNATURE_DOMAIN_TAG]);
    }
    let issuer_bytes = issuer_enc.unwrap();
    append_vec(&mut result, &issuer_bytes);
    append_vec(&mut result, payload_bytes);
    Some(result)
}

// r[impl web-of-trust.revocation]
/// Exact revocation-signature verification over the canonical signature input.
fn revocation_signature_verify_bridge(
    issuer_key_bytes: &Vec<u8>,
    payload_bytes: &Vec<u8>,
    signature: &Vec<u8>,
) -> (result: bool)
    ensures result == revocation_signature_valid_bytes_spec(issuer_key_bytes@, payload_bytes@, signature@)
{
    match revocation_signature_input_bytes(issuer_key_bytes, payload_bytes) {
        Some(expected) => bytes_equal(&expected, signature),
        None => false,
    }
}

// r[impl web-of-trust.revocation]
/// Check that a revocation signature binds the exact canonical payload.
pub fn revocation_signature_valid(rev: &Revocation) -> (result: bool)
    ensures result == revocation_signature_valid_spec(*rev)
{
    match revocation_payload_bytes(rev) {
        Some(payload) => revocation_signature_verify_bridge(&rev.issuer_key_bytes, &payload, &rev.signature),
        None => false,
    }
}

fn revocation_record_ids_match_bytes_exec(rev: &Revocation) -> (result: bool)
    ensures result == revocation_record_ids_match_bytes_spec(*rev)
{
    let issuer = crate::web_of_trust_principal::principal_id_from_bytes(&rev.issuer_key_bytes);
    let family_subject = crate::web_of_trust_principal::principal_id_from_bytes(&rev.family_subject_key_bytes);
    let result = rev.issuer.digest == issuer.digest && rev.family_subject.digest == family_subject.digest;
    proof {
        assert(issuer.digest@ == principal_id_bytes_spec(rev.issuer_key_bytes@));
        assert(family_subject.digest@ == principal_id_bytes_spec(rev.family_subject_key_bytes@));
        if result {
            assert(rev.issuer.digest@ == issuer.digest@);
            assert(rev.family_subject.digest@ == family_subject.digest@);
        }
        if revocation_record_ids_match_bytes_spec(*rev) {
            assert(rev.issuer.digest@ == issuer.digest@);
            assert(rev.family_subject.digest@ == family_subject.digest@);
        }
    }
    result
}

// r[impl web-of-trust.revocation]
/// Check semantic exact-payload or family-target matching.
pub fn revocation_matches(rev: &Revocation, att: &Attestation) -> (result: bool)
    ensures result == revocation_matches_spec(*rev, *att)
{
    if rev.target_kind_tag == EXACT_PAYLOAD_TARGET_KIND_TAG {
        bytes_equal(&rev.issuer.digest, &att.issuer.digest)
            && attestation_payload_matches(att, &rev.exact_payload_target)
    } else if rev.target_kind_tag == FAMILY_TARGET_KIND_TAG {
        bytes_equal(&rev.issuer.digest, &att.issuer.digest)
            && bytes_equal(&rev.family_subject.digest, &att.subject.digest)
            && rev.family_role_tag == att.role_tag
    } else {
        false
    }
}

// r[impl web-of-trust.revocation]
/// Check whether a revocation is effective against an attestation at time `t`.
pub fn revocation_effective_at(rev: &Revocation, att: &Attestation, t: u64) -> (result: bool)
    ensures result == revocation_effective_at_spec(*rev, *att, t)
{
    let rev_ids_match = revocation_record_ids_match_bytes_exec(rev);
    let att_ids_match = crate::web_of_trust_attestation::attestation_record_ids_match_bytes_exec(att);
    rev_ids_match && att_ids_match && revocation_signature_valid(rev) && revocation_matches(rev, att) && rev.effective_time <= t
}

// r[verify web-of-trust.revocation]
/// Canonical revocation payload bytes use the exact required field order.
pub proof fn revocation_payload_field_order(
    issuer_key_bytes: Seq<u8>,
    target_kind_tag: u8,
    target_encoding: Seq<u8>,
    effective_time: u64,
)
    requires
        length_fits_u32_spec(issuer_key_bytes.len()),
        length_fits_u32_spec(target_encoding.len()),
    ensures
        revocation_payload_bytes_spec(
            issuer_key_bytes,
            target_kind_tag,
            target_encoding,
            effective_time,
        ) == Some(
            canonical_byte_string_spec(issuer_key_bytes).unwrap()
                + seq![target_kind_tag]
                + canonical_byte_string_spec(target_encoding).unwrap()
                + encode_u64_spec(effective_time),
        ),
{
}

// r[verify web-of-trust.revocation]
/// Effective time uses exact little-endian `u64` encoding at the end of the payload.
pub proof fn revocation_payload_little_endian(
    issuer_key_bytes: Seq<u8>,
    target_kind_tag: u8,
    target_encoding: Seq<u8>,
    effective_time: u64,
)
    requires
        length_fits_u32_spec(issuer_key_bytes.len()),
        length_fits_u32_spec(target_encoding.len()),
    ensures
        revocation_payload_bytes_spec(
            issuer_key_bytes,
            target_kind_tag,
            target_encoding,
            effective_time,
        ).unwrap().subrange(
            (canonical_byte_string_spec(issuer_key_bytes).unwrap().len() + 1 + canonical_byte_string_spec(target_encoding).unwrap().len()) as int,
            (canonical_byte_string_spec(issuer_key_bytes).unwrap().len() + 1 + canonical_byte_string_spec(target_encoding).unwrap().len() + 8) as int,
        ) == encode_u64_spec(effective_time),
{
    revocation_payload_field_order(issuer_key_bytes, target_kind_tag, target_encoding, effective_time);
}

// r[verify web-of-trust.revocation]
/// Family targets encode exact `(issuer, subject, role)` bytes.
pub proof fn revocation_family_target_encoding(
    issuer_key_bytes: Seq<u8>,
    subject_key_bytes: Seq<u8>,
    role_tag: u8,
)
    requires
        length_fits_u32_spec(issuer_key_bytes.len()),
        length_fits_u32_spec(subject_key_bytes.len()),
    ensures
        family_target_encoding_spec(issuer_key_bytes, subject_key_bytes, role_tag)
            == Some(
                canonical_byte_string_spec(issuer_key_bytes).unwrap()
                    + canonical_byte_string_spec(subject_key_bytes).unwrap()
                    + seq![role_tag],
            ),
{
}

// r[verify web-of-trust.revocation]
/// Exact-payload targets encode one length-prefixed canonical attestation payload byte string.
pub proof fn revocation_exact_payload_target_encoding(exact_payload_target: Seq<u8>)
    requires length_fits_u32_spec(exact_payload_target.len())
    ensures exact_payload_target_encoding_spec(exact_payload_target) == Some(canonical_byte_string_spec(exact_payload_target).unwrap())
{
}

// r[verify web-of-trust.revocation]
/// Target-kind tags distinguish exact-payload and family targets.
pub proof fn revocation_target_kind_tags()
    ensures
        EXACT_PAYLOAD_TARGET_KIND_TAG != FAMILY_TARGET_KIND_TAG,
        EXACT_PAYLOAD_TARGET_KIND_TAG == 0x01,
        FAMILY_TARGET_KIND_TAG == 0x02,
{
}

// r[verify web-of-trust.revocation]
/// Revocation issuer and family-subject IDs match their canonical key bytes.
pub proof fn revocation_record_ids_match_bytes(rev: Revocation)
    requires revocation_record_ids_match_bytes_spec(rev)
    ensures
        rev.issuer.digest@ == principal_id_bytes_spec(rev.issuer_key_bytes@),
        rev.family_subject.digest@ == principal_id_bytes_spec(rev.family_subject_key_bytes@),
{
}

// r[verify web-of-trust.revocation]
/// Invalid revocations can never become effective.
pub proof fn invalid_revocation_never_effective(rev: Revocation, att: Attestation, t: u64)
    requires !revocation_record_ids_match_bytes_spec(rev) || !attestation_record_ids_match_bytes_spec(att)
    ensures !revocation_effective_at_spec(rev, att, t)
{
}

// r[verify web-of-trust.revocation]
/// A fixed signature cannot validate two different canonical revocation payloads for the same issuer bytes.
pub proof fn revocation_signature_binds_payload(
    issuer_key_bytes: Seq<u8>,
    payload1: Seq<u8>,
    payload2: Seq<u8>,
    signature: Seq<u8>,
)
    requires
        length_fits_u32_spec(issuer_key_bytes.len()),
        revocation_signature_valid_bytes_spec(issuer_key_bytes, payload1, signature),
        revocation_signature_valid_bytes_spec(issuer_key_bytes, payload2, signature),
    ensures
        payload1 == payload2,
{
    assert(revocation_signature_input_spec(issuer_key_bytes, payload1) == Some(signature));
    assert(revocation_signature_input_spec(issuer_key_bytes, payload2) == Some(signature));
    assert(revocation_signature_input_spec(issuer_key_bytes, payload1)
        == revocation_signature_input_spec(issuer_key_bytes, payload2));
    let prefix = seq![REVOCATION_SIGNATURE_DOMAIN_TAG]
        + canonical_byte_string_spec(issuer_key_bytes).unwrap();
    concat_right_cancel(prefix, payload1, payload2);
}

// r[verify web-of-trust.revocation]
/// Once effective, a revocation remains effective at later times.
pub proof fn revocation_monotone(rev: Revocation, att: Attestation, t1: u64, t2: u64)
    requires
        revocation_effective_at_spec(rev, att, t1),
        t1 <= t2,
    ensures
        revocation_effective_at_spec(rev, att, t2),
{
}

// r[verify web-of-trust.revocation]
/// Exact-payload revocations match exactly the canonical attestation payload bytes.
pub proof fn revocation_exact_payload_correct(rev: Revocation, att: Attestation)
    requires revocation_matches_exact_payload_spec(rev, att)
    ensures
        rev.target_kind_tag == EXACT_PAYLOAD_TARGET_KIND_TAG,
        attestation_payload_of_spec(att) == Some(rev.exact_payload_target@),
{
}

// r[verify web-of-trust.revocation]
/// Family-target revocations match exactly one `(issuer, subject, role)` family.
pub proof fn revocation_family_target_correct(rev: Revocation, att: Attestation)
    requires revocation_matches_family_spec(rev, att)
    ensures
        rev.target_kind_tag == FAMILY_TARGET_KIND_TAG,
        rev.family_subject.digest@ == att.subject.digest@,
        rev.family_role_tag == att.role_tag,
{
}

// r[verify web-of-trust.revocation]
/// Invalid target-kind tags are rejected as non-canonical.
pub proof fn revocation_invalid_kind_rejected(rev: Revocation)
    requires rev.target_kind_tag != EXACT_PAYLOAD_TARGET_KIND_TAG && rev.target_kind_tag != FAMILY_TARGET_KIND_TAG
    ensures revocation_target_encoding_spec(rev).is_none(),
{
}

// r[verify web-of-trust.revocation]
/// Overlong canonical fields are rejected as non-canonical.
pub proof fn revocation_overlong_rejected(
    issuer_key_bytes: Seq<u8>,
    target_encoding: Seq<u8>,
    target_kind_tag: u8,
    effective_time: u64,
)
    requires
        !length_fits_u32_spec(issuer_key_bytes.len()) || !length_fits_u32_spec(target_encoding.len()),
    ensures
        revocation_payload_bytes_spec(issuer_key_bytes, target_kind_tag, target_encoding, effective_time).is_none(),
{
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::web_of_trust_attestation::Attestation;
    use crate::web_of_trust_attestation::TRUST_ROLE_TAG;
    use crate::web_of_trust_principal::principal_id_from_bytes;

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
        let signature = crate::web_of_trust_attestation::attestation_signature_bytes_runtime(
            unsigned.issuer_key_bytes.as_slice(),
            payload.as_slice(),
        );
        Attestation { signature, ..unsigned }
    }

    fn signed_exact_payload_revocation(att: &Attestation, effective_time: u64) -> Revocation {
        let issuer = principal_id_from_bytes(&att.issuer_key_bytes);
        let family_subject = principal_id_from_bytes(&att.subject_key_bytes);
        let exact_payload_target = attestation_payload_bytes(att).unwrap();
        let unsigned = Revocation {
            issuer_key_bytes: att.issuer_key_bytes.clone(),
            issuer,
            target_kind_tag: EXACT_PAYLOAD_TARGET_KIND_TAG,
            exact_payload_target,
            family_subject_key_bytes: att.subject_key_bytes.clone(),
            family_subject,
            family_role_tag: att.role_tag,
            effective_time,
            signature: Vec::new(),
        };
        let payload = revocation_payload_bytes(&unsigned).unwrap();
        let signature = revocation_signature_bytes_runtime(unsigned.issuer_key_bytes.as_slice(), payload.as_slice());
        Revocation { signature, ..unsigned }
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

    // r[verify web-of-trust.revocation]
    #[test]
    fn test_exact_payload_target_disables_one_attestation() {
        let att = signed_attestation(vec![1, 2, 3], vec![4, 5, 6], TRUST_ROLE_TAG, 10, 30, 1);
        let rev = signed_exact_payload_revocation(&att, 20);
        assert!(!revocation_effective_at(&rev, &att, 19));
        assert!(revocation_effective_at(&rev, &att, 20));
    }

    // r[verify web-of-trust.revocation]
    #[test]
    fn test_exact_payload_target_disables_identical_payload_copies() {
        let att1 = signed_attestation(vec![9, 8, 7], vec![1, 2, 3], TRUST_ROLE_TAG, 5, 25, 9);
        let att2 = signed_attestation(vec![9, 8, 7], vec![1, 2, 3], TRUST_ROLE_TAG, 5, 25, 9);
        let rev = signed_exact_payload_revocation(&att1, 7);
        assert!(revocation_effective_at(&rev, &att1, 7));
        assert!(revocation_effective_at(&rev, &att2, 7));
    }

    // r[verify web-of-trust.revocation]
    #[test]
    fn test_family_target_disables_reissued_attestations_in_one_family() {
        let att1 = signed_attestation(vec![7, 7, 7], vec![8, 8, 8], TRUST_ROLE_TAG, 0, 10, 1);
        let att2 = signed_attestation(vec![7, 7, 7], vec![8, 8, 8], TRUST_ROLE_TAG, 10, 20, 2);
        let rev = signed_family_revocation(vec![7, 7, 7], vec![8, 8, 8], TRUST_ROLE_TAG, 3);
        assert!(revocation_effective_at(&rev, &att1, 3));
        assert!(revocation_effective_at(&rev, &att2, 3));
    }

    // r[verify web-of-trust.revocation]
    #[test]
    fn test_tampered_effective_time_invalidates_revocation_verification() {
        let att = signed_attestation(vec![1, 1], vec![2, 2], TRUST_ROLE_TAG, 0, 10, 4);
        let rev = signed_exact_payload_revocation(&att, 8);
        let tampered = Revocation {
            effective_time: 9,
            ..rev
        };
        assert!(!revocation_signature_valid(&tampered));
    }

    // r[verify web-of-trust.revocation]
    #[test]
    fn test_tampered_issuer_invalidates_revocation_verification() {
        let rev = signed_family_revocation(vec![1, 2, 3], vec![4, 5, 6], TRUST_ROLE_TAG, 8);
        let tampered_issuer = vec![9, 2, 3];
        let tampered = Revocation {
            issuer_key_bytes: tampered_issuer.clone(),
            issuer: principal_id_from_bytes(&tampered_issuer),
            ..rev
        };
        assert!(!revocation_signature_valid(&tampered));
    }

    // r[verify web-of-trust.revocation]
    #[test]
    fn test_tampered_target_invalidates_revocation_verification() {
        let rev = signed_family_revocation(vec![1, 2, 3], vec![4, 5, 6], TRUST_ROLE_TAG, 8);
        let tampered_subject = vec![4, 5, 9];
        let tampered = Revocation {
            family_subject_key_bytes: tampered_subject.clone(),
            family_subject: principal_id_from_bytes(&tampered_subject),
            ..rev
        };
        assert!(!revocation_signature_valid(&tampered));
    }

    // r[verify web-of-trust.revocation]
    #[test]
    fn test_invalid_kind_is_rejected() {
        let rev = Revocation {
            issuer_key_bytes: vec![1, 2, 3],
            issuer: principal_id_from_bytes(&[1, 2, 3]),
            target_kind_tag: 0xff,
            exact_payload_target: vec![1],
            family_subject_key_bytes: vec![4, 5, 6],
            family_subject: principal_id_from_bytes(&[4, 5, 6]),
            family_role_tag: TRUST_ROLE_TAG,
            effective_time: 1,
            signature: vec![1],
        };
        assert!(revocation_payload_bytes(&rev).is_none());
        assert!(!revocation_signature_valid(&rev));
    }

    // r[verify web-of-trust.revocation]
    #[test]
    fn test_wrong_issuer_revocation_does_not_disable_target() {
        let att = signed_attestation(vec![1, 2, 3], vec![4, 5, 6], TRUST_ROLE_TAG, 0, 10, 1);
        let rev = signed_family_revocation(vec![9, 9, 9], vec![4, 5, 6], TRUST_ROLE_TAG, 1);
        assert!(!revocation_effective_at(&rev, &att, 2));
    }

    // r[verify web-of-trust.revocation]
    #[test]
    fn test_unrelated_attestation_stays_unaffected() {
        let att_a = signed_attestation(vec![1, 2, 3], vec![4, 5, 6], TRUST_ROLE_TAG, 0, 10, 1);
        let att_b = signed_attestation(vec![1, 2, 3], vec![7, 8, 9], TRUST_ROLE_TAG, 0, 10, 2);
        let rev = signed_family_revocation(vec![1, 2, 3], vec![4, 5, 6], TRUST_ROLE_TAG, 1);
        assert!(revocation_effective_at(&rev, &att_a, 1));
        assert!(!revocation_effective_at(&rev, &att_b, 1));
    }

    // r[verify web-of-trust.revocation]
    #[test]
    fn test_overlong_revocation_fields_are_rejected_as_noncanonical() {
        if usize::BITS > 32 {
            let too_long = (u32::MAX as usize) + 1;
            assert!(!length_fits_u32(too_long));
        }
    }

    proptest! {
        // r[verify web-of-trust.revocation]
        #[test]
        fn prop_revocation_signature_oracle(
            issuer in prop::collection::vec(any::<u8>(), 0..64),
            subject in prop::collection::vec(any::<u8>(), 0..64),
            effective_time in any::<u64>(),
        ) {
            let rev = signed_family_revocation(issuer.clone(), subject, TRUST_ROLE_TAG, effective_time);
            let payload = revocation_payload_bytes(&rev).unwrap();
            let oracle = revocation_signature_bytes_runtime(issuer.as_slice(), payload.as_slice()) == rev.signature;
            prop_assert_eq!(revocation_signature_valid(&rev), oracle);
        }

        // r[verify web-of-trust.revocation]
        #[test]
        fn prop_revocation_effective_family_oracle(
            issuer in prop::collection::vec(any::<u8>(), 0..32),
            subject in prop::collection::vec(any::<u8>(), 0..32),
            effective_time in any::<u64>(),
            sample_time in any::<u64>(),
        ) {
            let att = signed_attestation(issuer.clone(), subject.clone(), TRUST_ROLE_TAG, 0, u64::MAX, 1);
            let rev = signed_family_revocation(issuer, subject, TRUST_ROLE_TAG, effective_time);
            let oracle = revocation_signature_valid(&rev)
                && rev.issuer.digest == att.issuer.digest
                && rev.family_subject.digest == att.subject.digest
                && rev.family_role_tag == att.role_tag
                && effective_time <= sample_time;
            prop_assert_eq!(revocation_effective_at(&rev, &att, sample_time), oracle);
        }

        // r[verify web-of-trust.revocation]
        #[test]
        fn prop_revocation_monotone_runtime(
            issuer in prop::collection::vec(any::<u8>(), 0..32),
            subject in prop::collection::vec(any::<u8>(), 0..32),
            effective_time in any::<u64>(),
            later_delta in any::<u16>(),
        ) {
            let att = signed_attestation(issuer.clone(), subject.clone(), TRUST_ROLE_TAG, 0, u64::MAX, 1);
            let rev = signed_family_revocation(issuer, subject, TRUST_ROLE_TAG, effective_time);
            let later = effective_time.saturating_add(u64::from(later_delta));
            if revocation_effective_at(&rev, &att, effective_time) {
                prop_assert!(revocation_effective_at(&rev, &att, later));
            }
        }
    }
}
