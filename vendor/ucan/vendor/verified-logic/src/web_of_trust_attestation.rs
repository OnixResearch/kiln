//! Verified web-of-trust attestation payloads and signature binding.
//!
//! Models exact canonical payload bytes for signed attestations over
//! `(issuer_bytes, subject_bytes, role_tag, not_before, not_after,
//! sequence_no)`.

#![allow(clippy::ptr_arg)]

use vstd::prelude::*;

use crate::serialize_inj::encode_u64;
#[cfg(verus_keep_ghost)]
use crate::serialize_inj::encode_u64_spec;
use crate::web_of_trust_principal::PrincipalId;
#[cfg(verus_keep_ghost)]
use crate::web_of_trust_principal::principal_id_bytes_spec;
use crate::web_of_trust_revocation::Revocation;
use crate::web_of_trust_revocation::revocation_effective_at;
#[cfg(verus_keep_ghost)]
use crate::web_of_trust_revocation::revocation_effective_at_spec;

#[cfg(verus_keep_ghost)]
pub(crate) fn attestation_signature_bytes_runtime(_issuer_key_bytes: &[u8], _payload_bytes: &[u8]) -> Vec<u8> {
    Vec::new()
}

#[cfg(not(verus_keep_ghost))]
#[allow(dead_code)]
pub(crate) fn attestation_signature_bytes_runtime(issuer_key_bytes: &[u8], payload_bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(0xa1);
    let issuer_len = u32::try_from(issuer_key_bytes.len()).expect("issuer key bytes must fit u32");
    out.extend_from_slice(&issuer_len.to_le_bytes());
    out.extend_from_slice(issuer_key_bytes);
    out.extend_from_slice(payload_bytes);
    out
}

verus! {

pub const TRUST_ROLE_TAG: u8 = 0x01;
pub const ATTESTATION_SIGNATURE_DOMAIN_TAG: u8 = 0xa1;

/// Signed attestation record.
pub struct Attestation {
    pub issuer_key_bytes: Vec<u8>,
    pub issuer: PrincipalId,
    pub subject_key_bytes: Vec<u8>,
    pub subject: PrincipalId,
    pub role_tag: u8,
    pub not_before: u64,
    pub not_after: u64,
    pub sequence_no: u64,
    pub signature: Vec<u8>,
}

pub closed spec fn encode_u32_le_spec(v: u32) -> Seq<u8> {
    seq![
        (v & 0xff) as u8,
        ((v >> 8u32) & 0xff) as u8,
        ((v >> 16u32) & 0xff) as u8,
        ((v >> 24u32) & 0xff) as u8,
    ]
}

// r[impl web-of-trust.attestation-signature]
pub open spec fn length_fits_u32_spec(len: nat) -> bool {
    len <= u32::MAX as nat
}

// r[impl web-of-trust.attestation-signature]
pub open spec fn canonical_byte_string_spec(bytes: Seq<u8>) -> Option<Seq<u8>> {
    if length_fits_u32_spec(bytes.len()) {
        Some(encode_u32_le_spec(bytes.len() as u32) + bytes)
    } else {
        None
    }
}

// r[impl web-of-trust.attestation-signature]
pub open spec fn attestation_payload_prefix_spec(issuer_key_bytes: Seq<u8>, subject_key_bytes: Seq<u8>) -> Seq<u8>
    recommends
        length_fits_u32_spec(issuer_key_bytes.len()),
        length_fits_u32_spec(subject_key_bytes.len()),
{
    canonical_byte_string_spec(issuer_key_bytes).unwrap()
        + canonical_byte_string_spec(subject_key_bytes).unwrap()
}

// r[depends web-of-trust.attestation-signature]
/// Canonical attestation payload-byte grammar.
pub open spec fn attestation_payload_bytes_spec(
    issuer_key_bytes: Seq<u8>,
    subject_key_bytes: Seq<u8>,
    role_tag: u8,
    not_before: u64,
    not_after: u64,
    sequence_no: u64,
) -> Option<Seq<u8>> {
    match (canonical_byte_string_spec(issuer_key_bytes), canonical_byte_string_spec(subject_key_bytes)) {
        (Some(issuer_enc), Some(subject_enc)) => Some(
            issuer_enc
                + subject_enc
                + seq![role_tag]
                + encode_u64_spec(not_before)
                + encode_u64_spec(not_after)
                + encode_u64_spec(sequence_no),
        ),
        _ => None,
    }
}

// r[depends web-of-trust.attestation-signature]
/// Canonical attestation payload for a full record.
pub open spec fn attestation_payload_of_spec(att: Attestation) -> Option<Seq<u8>> {
    attestation_payload_bytes_spec(
        att.issuer_key_bytes@,
        att.subject_key_bytes@,
        att.role_tag,
        att.not_before,
        att.not_after,
        att.sequence_no,
    )
}

// r[impl web-of-trust.attestation-signature]
pub open spec fn attestation_signature_input_spec(
    issuer_key_bytes: Seq<u8>,
    payload_bytes: Seq<u8>,
) -> Option<Seq<u8>> {
    match canonical_byte_string_spec(issuer_key_bytes) {
        Some(issuer_enc) => Some(seq![ATTESTATION_SIGNATURE_DOMAIN_TAG] + issuer_enc + payload_bytes),
        None => None,
    }
}

// r[depends web-of-trust.attestation-signature]
/// Exact attestation-signature validity surface over canonical bytes.
pub open spec fn attestation_signature_valid_bytes_spec(
    issuer_key_bytes: Seq<u8>,
    payload_bytes: Seq<u8>,
    signature: Seq<u8>,
) -> bool {
    match attestation_signature_input_spec(issuer_key_bytes, payload_bytes) {
        Some(expected) => signature == expected,
        None => false,
    }
}

// r[depends web-of-trust.attestation-signature]
/// Exact attestation-signature validity for a full record.
pub open spec fn attestation_signature_valid_spec(att: Attestation) -> bool {
    match attestation_payload_of_spec(att) {
        Some(payload) => attestation_signature_valid_bytes_spec(att.issuer_key_bytes@, payload, att.signature@),
        None => false,
    }
}

// r[impl web-of-trust.attestation-window]
pub open spec fn attestation_record_ids_match_bytes_spec(att: Attestation) -> bool {
    &&& att.issuer.digest@ == principal_id_bytes_spec(att.issuer_key_bytes@)
    &&& att.subject.digest@ == principal_id_bytes_spec(att.subject_key_bytes@)
}

// r[impl web-of-trust.attestation-window]
pub open spec fn no_effective_revocation_prefix_spec(
    revocations: Seq<Revocation>,
    att: Attestation,
    t: u64,
    upto: nat,
) -> bool
    recommends upto <= revocations.len()
    decreases upto
{
    if upto == 0 {
        true
    } else {
        no_effective_revocation_prefix_spec(revocations, att, t, (upto - 1) as nat)
            && !revocation_effective_at_spec(revocations[(upto - 1) as int], att, t)
    }
}

// r[depends web-of-trust.attestation-window]
/// Exact active-attestation semantics at time `t`.
pub open spec fn attestation_active_at_spec(att: Attestation, revocations: Seq<Revocation>, t: u64) -> bool {
    &&& attestation_record_ids_match_bytes_spec(att)
    &&& attestation_signature_valid_spec(att)
    &&& att.not_before <= t
    &&& t < att.not_after
    &&& no_effective_revocation_prefix_spec(revocations, att, t, revocations.len())
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

pub(crate) fn encode_u32_le(v: u32) -> (result: Vec<u8>)
    ensures result@ == encode_u32_le_spec(v)
{
    let b0 = (v & 0xff) as u8;
    let b1 = ((v >> 8u32) & 0xff) as u8;
    let b2 = ((v >> 16u32) & 0xff) as u8;
    let b3 = ((v >> 24u32) & 0xff) as u8;
    let result = vec![b0, b1, b2, b3];
    proof {
        assert(result@ =~= encode_u32_le_spec(v));
    }
    result
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

// r[impl web-of-trust.attestation-signature]
/// Exact canonical payload-byte builder for an attestation record.
#[allow(clippy::question_mark)]
pub fn attestation_payload_bytes(att: &Attestation) -> (result: Option<Vec<u8>>)
    ensures match result {
        Some(v) => attestation_payload_of_spec(*att) == Some(v@),
        None => attestation_payload_of_spec(*att).is_none(),
    }
{
    let issuer_enc = canonical_byte_string(&att.issuer_key_bytes);
    if issuer_enc.is_none() {
        return None;
    }
    let subject_enc = canonical_byte_string(&att.subject_key_bytes);
    if subject_enc.is_none() {
        return None;
    }

    let mut result = issuer_enc.unwrap();
    let subject_bytes = subject_enc.unwrap();
    append_vec(&mut result, &subject_bytes);
    result.push(att.role_tag);

    let not_before = encode_u64(att.not_before);
    append_vec(&mut result, &not_before);
    let not_after = encode_u64(att.not_after);
    append_vec(&mut result, &not_after);
    let sequence_no = encode_u64(att.sequence_no);
    append_vec(&mut result, &sequence_no);

    Some(result)
}

#[allow(clippy::question_mark)]
fn attestation_signature_input_bytes(
    issuer_key_bytes: &Vec<u8>,
    payload_bytes: &Vec<u8>,
) -> (result: Option<Vec<u8>>)
    ensures match result {
        Some(v) => attestation_signature_input_spec(issuer_key_bytes@, payload_bytes@) == Some(v@),
        None => attestation_signature_input_spec(issuer_key_bytes@, payload_bytes@).is_none(),
    }
{
    let issuer_enc = canonical_byte_string(issuer_key_bytes);
    if issuer_enc.is_none() {
        return None;
    }

    let mut result = vec![ATTESTATION_SIGNATURE_DOMAIN_TAG];
    proof {
        assert(result@ =~= seq![ATTESTATION_SIGNATURE_DOMAIN_TAG]);
    }
    let issuer_bytes = issuer_enc.unwrap();
    append_vec(&mut result, &issuer_bytes);
    append_vec(&mut result, payload_bytes);
    Some(result)
}

// r[impl web-of-trust.attestation-signature]
/// Exact attestation-signature verification over the canonical signature input.
fn attestation_signature_verify_bridge(
    issuer_key_bytes: &Vec<u8>,
    payload_bytes: &Vec<u8>,
    signature: &Vec<u8>,
) -> (result: bool)
    ensures result == attestation_signature_valid_bytes_spec(issuer_key_bytes@, payload_bytes@, signature@)
{
    match attestation_signature_input_bytes(issuer_key_bytes, payload_bytes) {
        Some(expected) => bytes_equal(&expected, signature),
        None => false,
    }
}

// r[impl web-of-trust.attestation-signature]
/// Check that an attestation signature binds the exact canonical payload.
pub fn attestation_signature_valid(att: &Attestation) -> (result: bool)
    ensures result == attestation_signature_valid_spec(*att)
{
    match attestation_payload_bytes(att) {
        Some(payload) => attestation_signature_verify_bridge(&att.issuer_key_bytes, &payload, &att.signature),
        None => false,
    }
}

pub(crate) fn attestation_record_ids_match_bytes_exec(att: &Attestation) -> (result: bool)
    ensures result == attestation_record_ids_match_bytes_spec(*att)
{
    let issuer = crate::web_of_trust_principal::principal_id_from_bytes(&att.issuer_key_bytes);
    let subject = crate::web_of_trust_principal::principal_id_from_bytes(&att.subject_key_bytes);
    let result = att.issuer.digest == issuer.digest && att.subject.digest == subject.digest;
    proof {
        assert(issuer.digest@ == principal_id_bytes_spec(att.issuer_key_bytes@));
        assert(subject.digest@ == principal_id_bytes_spec(att.subject_key_bytes@));
        if result {
            assert(att.issuer.digest@ == issuer.digest@);
            assert(att.subject.digest@ == subject.digest@);
        }
        if attestation_record_ids_match_bytes_spec(*att) {
            assert(att.issuer.digest@ == issuer.digest@);
            assert(att.subject.digest@ == subject.digest@);
        }
    }
    result
}

pub(crate) fn attestation_payload_matches(att: &Attestation, payload: &Vec<u8>) -> (result: bool)
    ensures result == (attestation_payload_of_spec(*att) == Some(payload@))
{
    match attestation_payload_bytes(att) {
        Some(actual) => {
            let result = actual == *payload;
            proof {
                assert(attestation_payload_of_spec(*att) == Some(actual@));
                if result {
                    assert(actual@ == payload@);
                }
                if attestation_payload_of_spec(*att) == Some(payload@) {
                    assert(actual@ == payload@);
                }
            }
            result
        }
        None => {
            proof {
                assert(attestation_payload_of_spec(*att).is_none());
            }
            false
        }
    }
}

// r[impl web-of-trust.attestation-window]
/// Check whether an attestation is active at time `t`.
pub fn attestation_active_at(att: &Attestation, revocations: &Vec<Revocation>, t: u64) -> (result: bool)
    ensures result == attestation_active_at_spec(*att, revocations@, t)
{
    let ids_match = attestation_record_ids_match_bytes_exec(att);
    let signature_valid = attestation_signature_valid(att);
    let in_window = att.not_before <= t && t < att.not_after;

    let mut clear = true;
    let mut i: usize = 0;
    while i < revocations.len()
        invariant
            i <= revocations.len(),
            clear == no_effective_revocation_prefix_spec(revocations@, *att, t, i as nat),
        decreases revocations.len() - i,
    {
        clear = clear && !revocation_effective_at(&revocations[i], att, t);
        i += 1;
    }

    ids_match && signature_valid && in_window && clear
}

// r[verify web-of-trust.attestation-signature]
/// Canonical payload bytes use the exact required field order.
pub proof fn attestation_payload_field_order(
    issuer_key_bytes: Seq<u8>,
    subject_key_bytes: Seq<u8>,
    role_tag: u8,
    not_before: u64,
    not_after: u64,
    sequence_no: u64,
)
    requires
        length_fits_u32_spec(issuer_key_bytes.len()),
        length_fits_u32_spec(subject_key_bytes.len()),
    ensures
        attestation_payload_bytes_spec(
            issuer_key_bytes,
            subject_key_bytes,
            role_tag,
            not_before,
            not_after,
            sequence_no,
        ) == Some(
            attestation_payload_prefix_spec(issuer_key_bytes, subject_key_bytes)
                + seq![role_tag]
                + encode_u64_spec(not_before)
                + encode_u64_spec(not_after)
                + encode_u64_spec(sequence_no),
        ),
{
}

// r[verify web-of-trust.attestation-signature]
/// Time and sequence fields use exact little-endian `u64` encodings.
pub proof fn attestation_payload_little_endian(
    issuer_key_bytes: Seq<u8>,
    subject_key_bytes: Seq<u8>,
    role_tag: u8,
    not_before: u64,
    not_after: u64,
    sequence_no: u64,
)
    requires
        length_fits_u32_spec(issuer_key_bytes.len()),
        length_fits_u32_spec(subject_key_bytes.len()),
    ensures
        attestation_payload_bytes_spec(
            issuer_key_bytes,
            subject_key_bytes,
            role_tag,
            not_before,
            not_after,
            sequence_no,
        ).unwrap().subrange(
            (attestation_payload_prefix_spec(issuer_key_bytes, subject_key_bytes).len() + 1) as int,
            (attestation_payload_prefix_spec(issuer_key_bytes, subject_key_bytes).len() + 9) as int,
        ) == encode_u64_spec(not_before),
        attestation_payload_bytes_spec(
            issuer_key_bytes,
            subject_key_bytes,
            role_tag,
            not_before,
            not_after,
            sequence_no,
        ).unwrap().subrange(
            (attestation_payload_prefix_spec(issuer_key_bytes, subject_key_bytes).len() + 9) as int,
            (attestation_payload_prefix_spec(issuer_key_bytes, subject_key_bytes).len() + 17) as int,
        ) == encode_u64_spec(not_after),
        attestation_payload_bytes_spec(
            issuer_key_bytes,
            subject_key_bytes,
            role_tag,
            not_before,
            not_after,
            sequence_no,
        ).unwrap().subrange(
            (attestation_payload_prefix_spec(issuer_key_bytes, subject_key_bytes).len() + 17) as int,
            (attestation_payload_prefix_spec(issuer_key_bytes, subject_key_bytes).len() + 25) as int,
        ) == encode_u64_spec(sequence_no),
{
    attestation_payload_field_order(
        issuer_key_bytes,
        subject_key_bytes,
        role_tag,
        not_before,
        not_after,
        sequence_no,
    );
}

// r[verify web-of-trust.attestation-signature]
/// Role-tag encoding is exact and occupies the single byte after the two key-byte strings.
pub proof fn attestation_payload_tag_encoding(
    issuer_key_bytes: Seq<u8>,
    subject_key_bytes: Seq<u8>,
    role_tag: u8,
    not_before: u64,
    not_after: u64,
    sequence_no: u64,
)
    requires
        length_fits_u32_spec(issuer_key_bytes.len()),
        length_fits_u32_spec(subject_key_bytes.len()),
    ensures
        attestation_payload_bytes_spec(
            issuer_key_bytes,
            subject_key_bytes,
            role_tag,
            not_before,
            not_after,
            sequence_no,
        ).unwrap()[attestation_payload_prefix_spec(issuer_key_bytes, subject_key_bytes).len() as int] == role_tag,
{
    attestation_payload_field_order(
        issuer_key_bytes,
        subject_key_bytes,
        role_tag,
        not_before,
        not_after,
        sequence_no,
    );
}

// r[verify web-of-trust.attestation-signature]
/// Overlong canonical key-byte strings are rejected as non-canonical.
pub proof fn attestation_payload_overlong_rejected(
    issuer_key_bytes: Seq<u8>,
    subject_key_bytes: Seq<u8>,
    role_tag: u8,
    not_before: u64,
    not_after: u64,
    sequence_no: u64,
)
    requires
        !length_fits_u32_spec(issuer_key_bytes.len()) || !length_fits_u32_spec(subject_key_bytes.len()),
    ensures
        attestation_payload_bytes_spec(
            issuer_key_bytes,
            subject_key_bytes,
            role_tag,
            not_before,
            not_after,
            sequence_no,
        ).is_none(),
{
}

// r[verify web-of-trust.attestation-signature]
/// A fixed signature cannot validate two different canonical payloads for the same issuer bytes.
pub proof fn attestation_signature_binds_payload(
    issuer_key_bytes: Seq<u8>,
    payload1: Seq<u8>,
    payload2: Seq<u8>,
    signature: Seq<u8>,
)
    requires
        length_fits_u32_spec(issuer_key_bytes.len()),
        attestation_signature_valid_bytes_spec(issuer_key_bytes, payload1, signature),
        attestation_signature_valid_bytes_spec(issuer_key_bytes, payload2, signature),
    ensures
        payload1 == payload2,
{
    assert(attestation_signature_input_spec(issuer_key_bytes, payload1) == Some(signature));
    assert(attestation_signature_input_spec(issuer_key_bytes, payload2) == Some(signature));
    assert(attestation_signature_input_spec(issuer_key_bytes, payload1)
        == attestation_signature_input_spec(issuer_key_bytes, payload2));
    let prefix = seq![ATTESTATION_SIGNATURE_DOMAIN_TAG]
        + canonical_byte_string_spec(issuer_key_bytes).unwrap();
    concat_right_cancel(prefix, payload1, payload2);
}

// r[verify web-of-trust.attestation-window]
/// Attestation issuer and subject IDs match their canonical key bytes.
pub proof fn attestation_record_ids_match_bytes(att: Attestation)
    requires attestation_record_ids_match_bytes_spec(att)
    ensures
        att.issuer.digest@ == principal_id_bytes_spec(att.issuer_key_bytes@),
        att.subject.digest@ == principal_id_bytes_spec(att.subject_key_bytes@),
{
}

// r[verify web-of-trust.attestation-window]
/// Invalid attestation records are never active.
pub proof fn invalid_attestation_is_inactive(att: Attestation, revocations: Seq<Revocation>, t: u64)
    requires !attestation_record_ids_match_bytes_spec(att)
    ensures !attestation_active_at_spec(att, revocations, t)
{
}

// r[verify web-of-trust.attestation-window]
/// Active attestations require a valid signature.
pub proof fn attestation_active_requires_signature(att: Attestation, revocations: Seq<Revocation>, t: u64)
    requires attestation_active_at_spec(att, revocations, t)
    ensures attestation_signature_valid_spec(att)
{
}

// r[verify web-of-trust.attestation-window]
/// Active attestations use the exact half-open validity window.
pub proof fn attestation_active_half_open_window(att: Attestation, revocations: Seq<Revocation>, t: u64)
    requires attestation_active_at_spec(att, revocations, t)
    ensures
        att.not_before <= t,
        t < att.not_after,
{
}

proof fn clear_prefix_excludes_index(att: Attestation, revocations: Seq<Revocation>, t: u64, upto: nat, idx: nat)
    requires
        idx < upto,
        upto <= revocations.len(),
        no_effective_revocation_prefix_spec(revocations, att, t, upto),
    ensures
        !revocation_effective_at_spec(revocations[idx as int], att, t),
    decreases upto,
{
    if idx + 1 == upto {
    } else {
        clear_prefix_excludes_index(att, revocations, t, (upto - 1) as nat, idx);
    }
}

// r[verify web-of-trust.attestation-window]
/// Any effective revocation makes the attestation inactive at that time.
pub proof fn attestation_active_respects_revocation(att: Attestation, revocations: Seq<Revocation>, t: u64)
    requires exists|i: int| 0 <= i < revocations.len() && revocation_effective_at_spec(revocations[i], att, t)
    ensures !attestation_active_at_spec(att, revocations, t)
{
    let i = choose|i: int| 0 <= i < revocations.len() && revocation_effective_at_spec(revocations[i], att, t);
    if attestation_active_at_spec(att, revocations, t) {
        clear_prefix_excludes_index(att, revocations, t, revocations.len(), i as nat);
        assert(false);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::web_of_trust_principal::principal_id_from_bytes;
    use crate::web_of_trust_revocation::FAMILY_TARGET_KIND_TAG;
    use crate::web_of_trust_revocation::Revocation;
    use crate::web_of_trust_revocation::revocation_payload_bytes;

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

    // r[verify web-of-trust.attestation-signature]
    #[test]
    fn test_exact_payload_verifies() {
        let att = signed_attestation(vec![0x10, 0x20, 0x30], vec![0x40, 0x50, 0x60], TRUST_ROLE_TAG, 7, 19, 23);
        let payload = attestation_payload_bytes(&att).unwrap();
        assert_eq!(payload[11], 0x40);
        assert!(attestation_signature_valid(&att));
    }

    // r[verify web-of-trust.attestation-signature]
    #[test]
    fn test_tampered_subject_invalidates_signature() {
        let att = signed_attestation(vec![0x01, 0x02, 0x03], vec![0x04, 0x05, 0x06], TRUST_ROLE_TAG, 11, 29, 31);
        let tampered_subject = vec![0x04, 0x05, 0x99];
        let tampered = Attestation {
            issuer_key_bytes: att.issuer_key_bytes.clone(),
            issuer: principal_id_from_bytes(&att.issuer_key_bytes),
            subject_key_bytes: tampered_subject.clone(),
            subject: principal_id_from_bytes(&tampered_subject),
            role_tag: att.role_tag,
            not_before: att.not_before,
            not_after: att.not_after,
            sequence_no: att.sequence_no,
            signature: att.signature.clone(),
        };
        assert!(!attestation_signature_valid(&tampered));
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
        let signature = crate::web_of_trust_revocation::revocation_signature_bytes_runtime(
            unsigned.issuer_key_bytes.as_slice(),
            payload.as_slice(),
        );
        Revocation { signature, ..unsigned }
    }

    // r[verify web-of-trust.attestation-signature]
    #[test]
    fn test_overlong_identity_bytes_are_rejected_as_noncanonical() {
        if usize::BITS > 32 {
            let too_long = (u32::MAX as usize) + 1;
            assert!(!length_fits_u32(too_long));
        }
    }

    // r[verify web-of-trust.attestation-window]
    #[test]
    fn test_in_window_unrevoked_attestation_is_active() {
        let att = signed_attestation(vec![1, 2, 3], vec![4, 5, 6], TRUST_ROLE_TAG, 10, 20, 1);
        assert!(attestation_active_at(&att, &Vec::new(), 15));
    }

    // r[verify web-of-trust.attestation-window]
    #[test]
    fn test_expired_or_future_attestation_is_inactive() {
        let att = signed_attestation(vec![1, 2, 3], vec![4, 5, 6], TRUST_ROLE_TAG, 10, 20, 1);
        assert!(!attestation_active_at(&att, &Vec::new(), 9));
        assert!(!attestation_active_at(&att, &Vec::new(), 20));
    }

    // r[verify web-of-trust.attestation-window]
    #[test]
    fn test_revoked_at_time_attestation_is_inactive() {
        let att = signed_attestation(vec![7, 7, 7], vec![8, 8, 8], TRUST_ROLE_TAG, 0, 20, 1);
        let rev = signed_family_revocation(vec![7, 7, 7], vec![8, 8, 8], TRUST_ROLE_TAG, 10);
        let revocations = vec![rev];
        assert!(!attestation_active_at(&att, &revocations, 10));
    }

    // r[verify web-of-trust.attestation-window]
    #[test]
    fn test_wrong_issuer_revocation_does_not_disable_attestation() {
        let att = signed_attestation(vec![7, 7, 7], vec![8, 8, 8], TRUST_ROLE_TAG, 0, 20, 1);
        let wrong = signed_family_revocation(vec![9, 9, 9], vec![8, 8, 8], TRUST_ROLE_TAG, 10);
        let revocations = vec![wrong];
        assert!(attestation_active_at(&att, &revocations, 10));
    }

    // r[verify web-of-trust.attestation-window]
    #[test]
    fn test_invalid_attestation_ids_are_inactive() {
        let att = signed_attestation(vec![1, 2, 3], vec![4, 5, 6], TRUST_ROLE_TAG, 0, 10, 1);
        let invalid = Attestation {
            issuer: principal_id_from_bytes(&[9, 9, 9]),
            ..att
        };
        assert!(!attestation_active_at(&invalid, &Vec::new(), 5));
    }

    proptest! {
        // r[verify web-of-trust.attestation-signature]
        #[test]
        fn prop_attestation_signature_oracle(
            issuer in prop::collection::vec(any::<u8>(), 0..64),
            subject in prop::collection::vec(any::<u8>(), 0..64),
            role_tag in any::<u8>(),
            not_before in any::<u64>(),
            not_after in any::<u64>(),
            sequence_no in any::<u64>(),
        ) {
            let att = signed_attestation(
                issuer.clone(),
                subject,
                role_tag,
                not_before,
                not_after,
                sequence_no,
            );
            let payload = attestation_payload_bytes(&att).unwrap();
            let oracle = attestation_signature_bytes_runtime(issuer.as_slice(), payload.as_slice()) == att.signature;
            prop_assert_eq!(attestation_signature_valid(&att), oracle);
        }

        // r[verify web-of-trust.attestation-signature]
        #[test]
        fn prop_tampered_payload_fails_verification(
            issuer in prop::collection::vec(any::<u8>(), 0..64),
            mut subject in prop::collection::vec(any::<u8>(), 0..64),
            role_tag in any::<u8>(),
            not_before in any::<u64>(),
            not_after in any::<u64>(),
            sequence_no in any::<u64>(),
        ) {
            let att = signed_attestation(
                issuer.clone(),
                subject.clone(),
                role_tag,
                not_before,
                not_after,
                sequence_no,
            );
            if subject.is_empty() {
                subject.push(0xff);
            } else {
                let last = subject.len() - 1;
                subject[last] ^= 0xff;
            }
            let tampered = Attestation {
                issuer_key_bytes: issuer.clone(),
                issuer: principal_id_from_bytes(&issuer),
                subject_key_bytes: subject.clone(),
                subject: principal_id_from_bytes(&subject),
                role_tag,
                not_before,
                not_after,
                sequence_no,
                signature: att.signature.clone(),
            };
            prop_assert!(!attestation_signature_valid(&tampered));
        }

        // r[verify web-of-trust.attestation-window]
        #[test]
        fn prop_active_window_oracle(
            issuer in prop::collection::vec(any::<u8>(), 0..32),
            subject in prop::collection::vec(any::<u8>(), 0..32),
            role_tag in any::<u8>(),
            not_before in any::<u64>(),
            width in any::<u8>(),
            sample in any::<u64>(),
        ) {
            let not_after = not_before.saturating_add(u64::from(width) + 1);
            let att = signed_attestation(issuer, subject, role_tag, not_before, not_after, 1);
            let oracle = att.not_before <= sample && sample < att.not_after;
            prop_assert_eq!(attestation_active_at(&att, &Vec::new(), sample), oracle);
        }
    }
}
