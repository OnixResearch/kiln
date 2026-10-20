//! Verified parsed UCAN header field admission.
//!
//! This module proves only byte-level equality over shell/serde-supplied parsed
//! header fields. Runtime JSON parsing, UTF-8/string handling, duplicate-key
//! semantics, canonicalization, base64url decoding, BLAKE3, and Ed25519 remain
//! explicit trusted/runtime boundaries in callers.

use vstd::prelude::*;

verus! {

pub const HEADER_ALG_E: u8 = 69;
pub const HEADER_ALG_D: u8 = 100;
pub const HEADER_ALG_CAP_D: u8 = 68;
pub const HEADER_ALG_S: u8 = 83;
pub const HEADER_ALG_CAP_A: u8 = 65;

pub const HEADER_TYP_J: u8 = 74;
pub const HEADER_TYP_W: u8 = 87;
pub const HEADER_TYP_T: u8 = 84;

/// Supported UCAN compact-token signature algorithm bytes: `EdDSA`.
// r[impl ucan-compact.parsed-header-admission]
pub open spec fn supported_algorithm_spec() -> Seq<u8> {
    seq![HEADER_ALG_E, HEADER_ALG_D, HEADER_ALG_CAP_D, HEADER_ALG_S, HEADER_ALG_CAP_A]
}

/// Supported UCAN compact-token type bytes: `JWT`.
// r[impl ucan-compact.parsed-header-admission]
pub open spec fn supported_token_type_spec() -> Seq<u8> {
    seq![HEADER_TYP_J, HEADER_TYP_W, HEADER_TYP_T]
}

/// Byte equality over finite parsed header fields.
// r[impl ucan-compact.parsed-header-admission]
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

/// Construct the supported algorithm bytes while proving the exact value.
// r[impl ucan-compact.parsed-header-admission]
pub fn supported_algorithm_bytes() -> (bytes: Vec<u8>)
    ensures bytes@ == supported_algorithm_spec(),
{
    vec![HEADER_ALG_E, HEADER_ALG_D, HEADER_ALG_CAP_D, HEADER_ALG_S, HEADER_ALG_CAP_A]
}

/// Construct the supported token type bytes while proving the exact value.
// r[impl ucan-compact.parsed-header-admission]
pub fn supported_token_type_bytes() -> (bytes: Vec<u8>)
    ensures bytes@ == supported_token_type_spec(),
{
    vec![HEADER_TYP_J, HEADER_TYP_W, HEADER_TYP_T]
}

/// Signer algorithm admission over shell/signer-supplied field bytes.
// r[impl ucan-compact.signer-algorithm-admission]
pub open spec fn supported_algorithm_admission_spec(algorithm: Seq<u8>) -> bool {
    algorithm == supported_algorithm_spec()
}

/// Check exact supported UCAN signer algorithm admission.
///
/// This does not prove signer honesty, signer I/O, key provenance, signing,
/// verify cryptographic signatures, or token provenance.
// r[impl ucan-compact.signer-algorithm-admission]
// r[verify ucan-compact.signer-algorithm-admission]
pub fn supported_algorithm_is_valid(algorithm: &Vec<u8>) -> (result: bool)
    ensures result == supported_algorithm_admission_spec(algorithm@),
{
    let expected_algorithm = supported_algorithm_bytes();
    bytes_equal(algorithm, &expected_algorithm)
}

/// Parsed header admission over shell/serde-supplied field bytes.
// r[impl ucan-compact.parsed-header-admission]
pub open spec fn parsed_header_admission_spec(algorithm: Seq<u8>, token_type: Seq<u8>) -> bool {
    supported_algorithm_admission_spec(algorithm) && token_type == supported_token_type_spec()
}

/// Check exact parsed UCAN header field admission.
///
/// This does not parse JSON, validate UTF-8/canonicalization, decode base64url,
/// hash, sign, verify cryptographic signatures, or prove token provenance.
// r[impl ucan-compact.parsed-header-admission]
// r[verify ucan-compact.parsed-header-admission]
pub fn parsed_header_admission_is_valid(algorithm: &Vec<u8>, token_type: &Vec<u8>) -> (result: bool)
    ensures result == parsed_header_admission_spec(algorithm@, token_type@),
{
    let expected_algorithm = supported_algorithm_bytes();
    if !bytes_equal(algorithm, &expected_algorithm) {
        return false;
    }
    let expected_token_type = supported_token_type_bytes();
    if !bytes_equal(token_type, &expected_token_type) {
        return false;
    }
    true
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_algorithm_admission_accepts_eddsa() {
        assert!(supported_algorithm_is_valid(&b"EdDSA".to_vec()));
    }

    #[test]
    fn supported_algorithm_admission_rejects_other_algorithms() {
        assert!(!supported_algorithm_is_valid(&b"ES256".to_vec()));
        assert!(!supported_algorithm_is_valid(&b"eddsa".to_vec()));
        assert!(!supported_algorithm_is_valid(&b"EdDSA\0".to_vec()));
    }

    #[test]
    fn parsed_header_admission_accepts_supported_fields() {
        assert!(parsed_header_admission_is_valid(&b"EdDSA".to_vec(), &b"JWT".to_vec()));
    }

    #[test]
    fn parsed_header_admission_rejects_unsupported_algorithm() {
        assert!(!parsed_header_admission_is_valid(&b"ES256".to_vec(), &b"JWT".to_vec()));
        assert!(!parsed_header_admission_is_valid(&b"eddsa".to_vec(), &b"JWT".to_vec()));
        assert!(!parsed_header_admission_is_valid(&b"EdDSA\0".to_vec(), &b"JWT".to_vec()));
    }

    #[test]
    fn parsed_header_admission_rejects_unsupported_type() {
        assert!(!parsed_header_admission_is_valid(&b"EdDSA".to_vec(), &b"JWS".to_vec()));
        assert!(!parsed_header_admission_is_valid(&b"EdDSA".to_vec(), &b"jwt".to_vec()));
        assert!(!parsed_header_admission_is_valid(&b"EdDSA".to_vec(), &b"JWT\0".to_vec()));
    }
}
