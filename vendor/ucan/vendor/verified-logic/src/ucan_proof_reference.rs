//! Verified proof-reference digest admission for UCAN proof references.
//!
//! Models the shell boundary after a runtime hash implementation has produced
//! digest bytes. The cryptographic hash computation is trusted runtime behavior;
//! this module verifies only fixed-width digest admission and exact
//! digest/reference byte equality.

#![allow(clippy::ptr_arg)]

use vstd::prelude::*;

verus! {

/// BLAKE3 digest width used by the UCAN shell for proof references.
pub const BLAKE3_DIGEST_LEN: usize = 32;

/// Shell-supplied proof-reference digest bytes.
pub struct ProofReferenceDigest {
    pub bytes: Vec<u8>,
}

/// A proof-reference digest is well formed exactly when it has the expected
/// fixed digest width.
// r[impl ucan-proof-reference.digest-length]
pub open spec fn proof_reference_digest_bytes_valid_spec(bytes: Seq<u8>) -> bool {
    bytes.len() == BLAKE3_DIGEST_LEN
}

/// A proof-reference digest is well formed exactly when its bytes are valid.
// r[impl ucan-proof-reference.digest-length]
pub open spec fn proof_reference_digest_valid_spec(digest: ProofReferenceDigest) -> bool {
    proof_reference_digest_bytes_valid_spec(digest.bytes@)
}

/// A proof reference admits a digest exactly when the digest is fixed-width and
/// the reference bytes equal the digest bytes.
// r[impl ucan-proof-reference.digest-admission]
pub open spec fn proof_reference_admits_digest_spec(reference: Seq<u8>, digest: ProofReferenceDigest) -> bool {
    proof_reference_digest_valid_spec(digest) && reference == digest.bytes@
}

/// Construct a digest only from fixed-width bytes.
// r[impl ucan-proof-reference.digest-length]
// r[verify ucan-proof-reference.digest-length]
pub fn proof_reference_digest_from_bytes(bytes: &Vec<u8>) -> (result: Option<ProofReferenceDigest>)
    ensures
        match result {
            Some(digest) => digest.bytes@ == bytes@ && proof_reference_digest_valid_spec(digest),
            None => !proof_reference_digest_bytes_valid_spec(bytes@),
        },
{
    if bytes.len() == BLAKE3_DIGEST_LEN {
        Some(ProofReferenceDigest { bytes: bytes.clone() })
    } else {
        None
    }
}

/// Check exact proof-reference/digest admission.
// r[impl ucan-proof-reference.digest-admission]
// r[verify ucan-proof-reference.digest-admission]
pub fn proof_reference_admits_digest(reference: &Vec<u8>, digest: &ProofReferenceDigest) -> (result: bool)
    ensures result == proof_reference_admits_digest_spec(reference@, *digest),
{
    if digest.bytes.len() != BLAKE3_DIGEST_LEN {
        assert(!proof_reference_digest_valid_spec(*digest));
        false
    } else if *reference == digest.bytes {
        assert(proof_reference_digest_valid_spec(*digest));
        assert(reference@ == digest.bytes@);
        true
    } else {
        assert(proof_reference_digest_valid_spec(*digest));
        assert(reference@ != digest.bytes@);
        false
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_admission_accepts_exact_32_byte_reference() {
        let bytes = vec![7u8; BLAKE3_DIGEST_LEN];
        let digest = proof_reference_digest_from_bytes(&bytes).expect("fixed-width digest");
        assert!(proof_reference_admits_digest(&bytes, &digest));
    }

    #[test]
    fn digest_admission_rejects_wrong_digest_length() {
        let bytes = vec![7u8; BLAKE3_DIGEST_LEN - 1];
        assert!(proof_reference_digest_from_bytes(&bytes).is_none());
    }

    #[test]
    fn digest_admission_rejects_reference_mismatch() {
        let bytes = vec![7u8; BLAKE3_DIGEST_LEN];
        let digest = proof_reference_digest_from_bytes(&bytes).expect("fixed-width digest");
        let mut reference = bytes.clone();
        reference[0] = 8;
        assert!(!proof_reference_admits_digest(&reference, &digest));
    }
}
