//! Verified structural admission for base64url-no-pad encoded UCAN segments.
//!
//! This module proves only encoded-byte shape facts: URL-safe alphabet bytes and
//! the unpadded length class that base64url can represent. Runtime decoding and
//! decoded-byte correctness remain explicit trusted codec boundaries in callers.

use vstd::prelude::*;

verus! {

pub const BASE64URL_HYPHEN: u8 = 45;
pub const BASE64URL_UNDERSCORE: u8 = 95;

pub open spec fn is_uppercase_letter(byte: u8) -> bool {
    65 <= byte && byte <= 90
}

pub open spec fn is_lowercase_letter(byte: u8) -> bool {
    97 <= byte && byte <= 122
}

pub open spec fn is_decimal_digit(byte: u8) -> bool {
    48 <= byte && byte <= 57
}

/// URL-safe base64 alphabet byte without padding.
// r[impl ucan-compact.base64url-segment-admission]
pub open spec fn is_base64url_no_pad_byte_spec(byte: u8) -> bool {
    is_uppercase_letter(byte)
        || is_lowercase_letter(byte)
        || is_decimal_digit(byte)
        || byte == BASE64URL_HYPHEN
        || byte == BASE64URL_UNDERSCORE
}

/// A finite encoded segment uses only base64url-no-pad alphabet bytes.
// r[impl ucan-compact.base64url-segment-admission]
pub open spec fn base64url_no_pad_alphabet_spec(bytes: Seq<u8>) -> bool {
    forall|i: int| 0 <= i < bytes.len() ==> is_base64url_no_pad_byte_spec(bytes[i])
}

/// Unpadded base64url cannot have encoded length congruent to 1 modulo 4.
// r[impl ucan-compact.base64url-segment-admission]
pub open spec fn base64url_no_pad_length_spec(len: nat) -> bool {
    len % 4 != 1
}

/// Structural admission for a base64url-no-pad segment.
// r[impl ucan-compact.base64url-segment-admission]
pub open spec fn base64url_no_pad_segment_spec(bytes: Seq<u8>) -> bool {
    base64url_no_pad_alphabet_spec(bytes) && base64url_no_pad_length_spec(bytes.len())
}

// r[impl ucan-compact.base64url-segment-admission]
pub fn is_base64url_no_pad_byte(byte: u8) -> (result: bool)
    ensures result == is_base64url_no_pad_byte_spec(byte),
{
    (65 <= byte && byte <= 90)
        || (97 <= byte && byte <= 122)
        || (48 <= byte && byte <= 57)
        || byte == BASE64URL_HYPHEN
        || byte == BASE64URL_UNDERSCORE
}

// r[impl ucan-compact.base64url-segment-admission]
pub fn base64url_no_pad_length_is_valid(len: usize) -> (result: bool)
    ensures result == base64url_no_pad_length_spec(len as nat),
{
    len % 4 != 1
}

/// Check the structural base64url-no-pad shape of caller/shell-supplied encoded bytes.
///
/// This does not decode base64url, parse JSON, hash, sign, or verify
/// cryptographic signatures.
// r[impl ucan-compact.base64url-segment-admission]
pub fn base64url_no_pad_segment_is_valid(bytes: &Vec<u8>) -> (result: bool)
    ensures result == base64url_no_pad_segment_spec(bytes@),
{
    let mut i: usize = 0;
    while i < bytes.len()
        invariant
            0 <= i <= bytes.len(),
            forall|k: int| 0 <= k < i ==> is_base64url_no_pad_byte_spec(bytes@[k]),
        decreases bytes.len() - i,
    {
        if !is_base64url_no_pad_byte(bytes[i]) {
            assert(!is_base64url_no_pad_byte_spec(bytes@[i as int]));
            assert(!base64url_no_pad_alphabet_spec(bytes@));
            return false;
        }
        i = i + 1;
    }
    assert(base64url_no_pad_alphabet_spec(bytes@));
    if !base64url_no_pad_length_is_valid(bytes.len()) {
        assert(!base64url_no_pad_length_spec(bytes@.len()));
        return false;
    }
    true
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64url_segment_accepts_url_safe_no_pad_bytes() {
        assert!(base64url_no_pad_segment_is_valid(&b"ABCDEFGHIJKLMNOPQRSTUVWXYZ".to_vec()));
        assert!(base64url_no_pad_segment_is_valid(&b"abcdefghijklmnopqrstuvwxyz".to_vec()));
        assert!(base64url_no_pad_segment_is_valid(&b"0123456789-_".to_vec()));
        assert!(base64url_no_pad_segment_is_valid(&Vec::new()));
    }

    #[test]
    fn base64url_segment_rejects_padding_standard_alphabet_and_period() {
        assert!(!base64url_no_pad_segment_is_valid(&b"abc=".to_vec()));
        assert!(!base64url_no_pad_segment_is_valid(&b"abc+".to_vec()));
        assert!(!base64url_no_pad_segment_is_valid(&b"abc/".to_vec()));
        assert!(!base64url_no_pad_segment_is_valid(&b"abc.".to_vec()));
    }

    #[test]
    fn base64url_segment_rejects_impossible_unpadded_length_class() {
        assert!(!base64url_no_pad_segment_is_valid(&b"A".to_vec()));
        assert!(base64url_no_pad_segment_is_valid(&b"AA".to_vec()));
        assert!(base64url_no_pad_segment_is_valid(&b"AAA".to_vec()));
        assert!(base64url_no_pad_segment_is_valid(&b"AAAA".to_vec()));
    }
}
