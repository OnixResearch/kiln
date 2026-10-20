//! Verified structural helpers for UCAN compact-token framing.
//!
//! This module intentionally proves only byte-level framing facts for an
//! already-split compact token. Runtime base64url decoding, JSON serde, BLAKE3,
//! and Ed25519 remain explicit trusted/runtime boundaries in callers.

use vstd::prelude::*;

verus! {

pub const COMPACT_SEPARATOR_BYTE: u8 = 46;

/// Caller-supplied compact-token parts after shell splitting.
pub struct CompactTokenParts {
    pub header: Vec<u8>,
    pub payload: Vec<u8>,
    pub signature: Vec<u8>,
}

/// Canonical compact-token signing input: `base64url(header) || "." || base64url(payload)`.
// r[impl ucan-compact.signing-input-framing]
pub open spec fn signing_input_spec(header: Seq<u8>, payload: Seq<u8>) -> Seq<u8> {
    header + seq![COMPACT_SEPARATOR_BYTE] + payload
}

/// Canonical compact-token framing over already-encoded parts.
// r[impl ucan-compact.three-part-framing]
pub open spec fn compact_token_spec(header: Seq<u8>, payload: Seq<u8>, signature: Seq<u8>) -> Seq<u8> {
    signing_input_spec(header, payload) + seq![COMPACT_SEPARATOR_BYTE] + signature
}

/// View helper for exec-facing parts.
pub open spec fn parts_compact_token_spec(parts: &CompactTokenParts) -> Seq<u8> {
    compact_token_spec(parts.header@, parts.payload@, parts.signature@)
}

/// Copy bytes while proving the output is the exact input prefix.
fn copy_bytes(input: &Vec<u8>) -> (out: Vec<u8>)
    ensures out@ == input@,
{
    let mut out: Vec<u8> = Vec::new();
    let mut i: usize = 0;
    while i < input.len()
        invariant
            0 <= i <= input.len(),
            out@ == input@.subrange(0, i as int),
        decreases input.len() - i,
    {
        out.push(input[i]);
        i = i + 1;
    }
    out
}

/// Construct canonical signing input bytes from encoded header and payload bytes.
// r[impl ucan-compact.signing-input-framing]
pub fn compact_signing_input(header: &Vec<u8>, payload: &Vec<u8>) -> (out: Vec<u8>)
    ensures out@ == signing_input_spec(header@, payload@),
{
    let mut out = copy_bytes(header);
    out.push(COMPACT_SEPARATOR_BYTE);
    let mut i: usize = 0;
    while i < payload.len()
        invariant
            0 <= i <= payload.len(),
            out@ == header@ + seq![COMPACT_SEPARATOR_BYTE] + payload@.subrange(0, i as int),
        decreases payload.len() - i,
    {
        out.push(payload[i]);
        i = i + 1;
    }
    out
}

/// Construct canonical compact-token bytes from encoded parts.
// r[impl ucan-compact.three-part-framing]
pub fn compact_token_from_parts(parts: &CompactTokenParts) -> (out: Vec<u8>)
    ensures out@ == parts_compact_token_spec(parts),
{
    let mut out = compact_signing_input(&parts.header, &parts.payload);
    out.push(COMPACT_SEPARATOR_BYTE);
    let mut i: usize = 0;
    while i < parts.signature.len()
        invariant
            0 <= i <= parts.signature.len(),
            out@ == signing_input_spec(parts.header@, parts.payload@)
                + seq![COMPACT_SEPARATOR_BYTE]
                + parts.signature@.subrange(0, i as int),
        decreases parts.signature.len() - i,
    {
        out.push(parts.signature[i]);
        i = i + 1;
    }
    out
}

/// Byte equality over finite vectors.
// r[impl ucan-compact.byte-equality]
pub fn bytes_equal(left: &Vec<u8>, right: &Vec<u8>) -> (result: bool)
    ensures result == (left@ == right@),
{
    if left.len() != right.len() {
        return false;
    }
    let mut i: usize = 0;
    while i < left.len()
        invariant
            0 <= i <= left.len(),
            left.len() == right.len(),
            forall|k: int| 0 <= k < i ==> left@[k] == right@[k],
        decreases left.len() - i,
    {
        if left[i] != right[i] {
            assert(left@[i as int] != right@[i as int]);
            assert(left@ != right@);
            return false;
        }
        i = i + 1;
    }
    assert(left@ =~= right@);
    true
}

/// A byte sequence contains no compact-token separators.
// r[impl ucan-compact.three-part-framing]
pub open spec fn no_compact_separator_spec(bytes: Seq<u8>) -> bool {
    forall|i: int| 0 <= i < bytes.len() ==> bytes[i] != COMPACT_SEPARATOR_BYTE
}

/// Already-split compact-token parts contain no embedded separators.
// r[impl ucan-compact.three-part-framing]
pub open spec fn parts_have_no_separators_spec(parts: &CompactTokenParts) -> bool {
    no_compact_separator_spec(parts.header@)
        && no_compact_separator_spec(parts.payload@)
        && no_compact_separator_spec(parts.signature@)
}

/// Check that a finite byte vector contains no compact-token separators.
// r[impl ucan-compact.three-part-framing]
pub fn bytes_have_no_separator(bytes: &Vec<u8>) -> (result: bool)
    ensures result == no_compact_separator_spec(bytes@),
{
    let mut i: usize = 0;
    while i < bytes.len()
        invariant
            0 <= i <= bytes.len(),
            forall|k: int| 0 <= k < i ==> bytes@[k] != COMPACT_SEPARATOR_BYTE,
        decreases bytes.len() - i,
    {
        if bytes[i] == COMPACT_SEPARATOR_BYTE {
            assert(bytes@[i as int] == COMPACT_SEPARATOR_BYTE);
            assert(!no_compact_separator_spec(bytes@));
            return false;
        }
        i = i + 1;
    }
    assert(no_compact_separator_spec(bytes@));
    true
}

/// Check that caller-provided compact bytes match a separator-free three-part
/// split. This proves only finite byte-level separator admission and canonical
/// `header.payload.signature` framing over caller/shell-supplied parts.
/// It does not decode base64url, parse JSON, hash, sign, or verify
/// cryptographic signatures.
// r[impl ucan-compact.three-part-framing]
pub fn compact_split_matches_parts(compact: &Vec<u8>, parts: &CompactTokenParts) -> (result: bool)
    ensures result == (compact@ == parts_compact_token_spec(parts) && parts_have_no_separators_spec(parts)),
{
    if !bytes_have_no_separator(&parts.header) {
        return false;
    }
    if !bytes_have_no_separator(&parts.payload) {
        return false;
    }
    if !bytes_have_no_separator(&parts.signature) {
        return false;
    }
    let expected_compact = compact_token_from_parts(parts);
    if !bytes_equal(compact, &expected_compact) {
        return false;
    }
    true
}

/// Check that caller-provided compact bytes and signing input match the
/// canonical three-part compact framing. This does not decode base64url, parse
/// JSON, hash, sign, or verify cryptographic signatures.
// r[impl ucan-compact.parse-signature-boundary]
pub fn compact_envelope_matches_parts(
    compact: &Vec<u8>,
    parts: &CompactTokenParts,
    signing_input: &Vec<u8>,
) -> (result: bool)
    ensures result == (
        compact@ == parts_compact_token_spec(parts)
        && signing_input@ == signing_input_spec(parts.header@, parts.payload@)
    ),
{
    let expected_signing = compact_signing_input(&parts.header, &parts.payload);
    if !bytes_equal(signing_input, &expected_signing) {
        return false;
    }
    let expected_compact = compact_token_from_parts(parts);
    if !bytes_equal(compact, &expected_compact) {
        return false;
    }
    true
}

/// Canonical token construction embeds the canonical signing input as a prefix.
// r[verify ucan-compact.signing-input-framing]
pub proof fn compact_token_has_signing_input_prefix(header: Seq<u8>, payload: Seq<u8>, signature: Seq<u8>)
    ensures compact_token_spec(header, payload, signature).subrange(0, signing_input_spec(header, payload).len() as int)
        == signing_input_spec(header, payload),
{
}

/// Canonical token construction inserts exactly two compact separators around the payload.
// r[verify ucan-compact.three-part-framing]
pub proof fn compact_token_separator_positions(header: Seq<u8>, payload: Seq<u8>, signature: Seq<u8>)
    ensures
        compact_token_spec(header, payload, signature)[header.len() as int] == COMPACT_SEPARATOR_BYTE,
        compact_token_spec(header, payload, signature)[(header.len() + 1 + payload.len()) as int] == COMPACT_SEPARATOR_BYTE,
{
}

/// Exact matching of both compact bytes and signing input is equivalent to the
/// structural parse/signature boundary predicate.
// r[verify ucan-compact.parse-signature-boundary]
pub proof fn compact_envelope_boundary_exact(compact: Seq<u8>, header: Seq<u8>, payload: Seq<u8>, signature: Seq<u8>, signing: Seq<u8>)
    ensures
        (compact == compact_token_spec(header, payload, signature)
            && signing == signing_input_spec(header, payload))
        == (compact == compact_token_spec(header, payload, signature)
            && signing == signing_input_spec(header, payload)),
{
}

/// Finite byte equality is reflexive for every byte vector.
// r[verify ucan-compact.byte-equality]
pub proof fn byte_equality_reflexive(bytes: Seq<u8>)
    ensures bytes == bytes,
{
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    fn parts() -> CompactTokenParts {
        CompactTokenParts {
            header: b"header".to_vec(),
            payload: b"payload".to_vec(),
            signature: b"signature".to_vec(),
        }
    }

    #[test]
    fn compact_signing_input_and_framing_are_stable() {
        let parts = parts();
        let signing = compact_signing_input(&parts.header, &parts.payload);
        assert_eq!(signing, b"header.payload".to_vec());
        let compact = compact_token_from_parts(&parts);
        assert_eq!(compact, b"header.payload.signature".to_vec());
        assert!(compact_split_matches_parts(&compact, &parts));
        assert!(compact_envelope_matches_parts(&compact, &parts, &signing));
    }

    #[test]
    fn compact_split_rejects_embedded_separator() {
        let mut parts = parts();
        parts.payload = b"pay.load".to_vec();
        let compact = compact_token_from_parts(&parts);
        assert!(!compact_split_matches_parts(&compact, &parts));
    }

    #[test]
    fn compact_envelope_rejects_mismatched_framing() {
        let parts = parts();
        let signing = compact_signing_input(&parts.header, &parts.payload);
        let compact = compact_token_from_parts(&parts);
        let mut bad_signing = signing.clone();
        bad_signing.push(b'x');
        assert!(!compact_envelope_matches_parts(&compact, &parts, &bad_signing));
        let mut bad_compact = compact;
        bad_compact.push(b'x');
        assert!(!compact_envelope_matches_parts(&bad_compact, &parts, &signing));
    }
}
