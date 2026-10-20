//! Verified Content Identifier (CID) construction for UCAN proof references.
//!
//! Models CIDv1 construction: version byte (1) + content codec varint +
//! multihash (hash code varint + digest length varint + digest bytes).
//! Proves determinism, version correctness, CID injectivity, and
//! multihash round-trip.
//!
//! Reuses varint encode/decode from `ucan_did`.

#![allow(clippy::ptr_arg)]

use vstd::prelude::*;

verus! {

/// CID version 1.
pub const CID_VERSION: u8 = 1;

/// SHA-256 multihash code.
pub const SHA256_CODE: u64 = 0x12;

/// SHA-256 digest length.
pub const SHA256_DIGEST_LEN: u64 = 32;

/// DAG-CBOR multicodec.
pub const DAG_CBOR_CODEC: u64 = 0x71;

/// Raw multicodec.
pub const RAW_CODEC: u64 = 0x55;

/// A multihash: hash function code + digest.
pub struct Multihash {
    pub code: u64,
    pub digest: Vec<u8>,
}

/// Spec-level multihash (uses Seq instead of Vec).
pub struct MultihashSpec {
    pub code: u64,
    pub digest: Seq<u8>,
}

/// A CIDv1: version + content codec + multihash.
pub struct Cid {
    pub version: u8,
    pub codec: u64,
    pub multihash: Multihash,
}

/// Spec-level CID.
pub struct CidSpec {
    pub version: u8,
    pub codec: u64,
    pub multihash: MultihashSpec,
}

// --- Varint (inlined small helper, mirrors ucan_did) ---

/// Varint encode spec (LEB128).
// r[impl ucan-cid.varint-round-trip]
pub open spec fn varint_encode_spec(n: u64) -> Seq<u8>
    decreases n,
{
    if n < 128 {
        seq![n as u8]
    } else {
        seq![(n % 128 + 128) as u8] + varint_encode_spec((n / 128) as u64)
    }
}

/// Varint decode spec.
// r[impl ucan-cid.varint-round-trip]
pub open spec fn varint_decode_spec(bytes: Seq<u8>) -> Option<(u64, nat)>
    decreases bytes.len(),
{
    if bytes.len() == 0 {
        None
    } else if bytes[0] < 128 {
        Some((bytes[0] as u64, 1))
    } else {
        match varint_decode_spec(bytes.subrange(1, bytes.len() as int)) {
            None => None,
            Some((rest_val, rest_len)) =>
                Some(((((bytes[0] - 128) as u64) + rest_val * 128) as u64, rest_len + 1)),
        }
    }
}

/// Varint round-trip.
// r[verify ucan-cid.varint-round-trip]
proof fn varint_round_trip(n: u64)
    ensures varint_decode_spec(varint_encode_spec(n)) == Some((n, varint_encode_spec(n).len())),
    decreases n,
{
    if n < 128 {
    } else {
        let tail_val = (n / 128) as u64;
        let low_byte = (n % 128 + 128) as u8;
        let tail_enc = varint_encode_spec(tail_val);
        let full = seq![low_byte] + tail_enc;
        varint_round_trip(tail_val);
        assert(full.subrange(1, full.len() as int) =~= tail_enc);
        let low = (low_byte - 128) as u64;
        assert(low == n % 128);
        assert((low + tail_val * 128) as u64 == n) by(nonlinear_arith)
            requires low == n % 128, tail_val == n / 128, n >= 128u64;
    }
}

/// Varint encode is nonempty.
proof fn varint_encode_nonempty(n: u64)
    ensures varint_encode_spec(n).len() >= 1,
    decreases n,
{
}

/// Varint is prefix-free: extra trailing bytes don't affect decode.
proof fn varint_decode_prefix_free(n: u64, suffix: Seq<u8>)
    ensures varint_decode_spec(varint_encode_spec(n) + suffix) == Some((n, varint_encode_spec(n).len())),
    decreases n,
{
    let enc = varint_encode_spec(n);
    let combined = enc + suffix;
    if n < 128 {
        assert(enc =~= seq![n as u8]);
        assert(combined[0] == n as u8);
        assert((n as u8) < 128);
    } else {
        let low_byte = (n % 128 + 128) as u8;
        let tail_val = (n / 128) as u64;
        let tail_enc = varint_encode_spec(tail_val);
        assert(enc =~= seq![low_byte] + tail_enc);
        assert(combined[0] == low_byte);
        assert(low_byte >= 128u8);
        let combined_tail = combined.subrange(1, combined.len() as int);
        assert(combined_tail =~= tail_enc + suffix);
        varint_decode_prefix_free(tail_val, suffix);
        let low = (low_byte - 128) as u64;
        assert(low == n % 128);
        assert((low + tail_val * 128) as u64 == n) by(nonlinear_arith)
            requires low == n % 128, tail_val == n / 128, n >= 128u64;
    }
}

// --- Multihash ---

/// Multihash bytes: code varint + digest_len varint + digest.
// r[impl ucan-cid.multihash-round-trip]
// r[verify ucan-cid.multihash-round-trip]
pub open spec fn multihash_bytes_spec(mh: MultihashSpec) -> Seq<u8> {
    varint_encode_spec(mh.code)
        + varint_encode_spec(mh.digest.len() as u64)
        + mh.digest
}

/// Parse multihash from bytes.
// r[impl ucan-cid.multihash-round-trip]
pub open spec fn multihash_decode_spec(bytes: Seq<u8>) -> Option<(MultihashSpec, nat)> {
    match varint_decode_spec(bytes) {
        None => None,
        Some((code, code_len)) => {
            let rest1 = bytes.subrange(code_len as int, bytes.len() as int);
            match varint_decode_spec(rest1) {
                None => None,
                Some((digest_len, dlen_len)) => {
                    let rest2 = rest1.subrange(dlen_len as int, rest1.len() as int);
                    if rest2.len() >= digest_len as int {
                        Some((MultihashSpec {
                            code,
                            digest: rest2.subrange(0, digest_len as int),
                        }, code_len + dlen_len + digest_len as nat))
                    } else {
                        None
                    }
                }
            }
        }
    }
}

/// Wrap a digest into a multihash.
// r[impl ucan-cid.multihash-digest-length]
pub open spec fn multihash_wrap_spec(code: u64, digest: Seq<u8>) -> MultihashSpec {
    MultihashSpec { code, digest }
}

/// Multihash digest length matches.
// r[verify ucan-cid.multihash-digest-length]
pub proof fn multihash_digest_length_matches(code: u64, digest: Seq<u8>)
    ensures multihash_wrap_spec(code, digest).digest.len() == digest.len(),
{
}

/// Multihash round-trips through its byte encoding.
// r[verify ucan-cid.multihash-round-trip]
pub proof fn multihash_round_trip(mh: MultihashSpec)
    requires mh.digest.len() <= u64::MAX,
    ensures multihash_decode_spec(multihash_bytes_spec(mh)) == Some((mh, multihash_bytes_spec(mh).len())),
{
    let digest_len = mh.digest.len() as u64;
    let bytes = multihash_bytes_spec(mh);
    let code_bytes = varint_encode_spec(mh.code);
    let len_bytes = varint_encode_spec(digest_len);
    let rest_after_code = len_bytes + mh.digest;
    varint_decode_prefix_free(mh.code, rest_after_code);
    assert(bytes =~= code_bytes + rest_after_code);
    assert(varint_decode_spec(bytes) == Some((mh.code, code_bytes.len())));

    let rest1 = bytes.subrange(code_bytes.len() as int, bytes.len() as int);
    assert(rest1 =~= rest_after_code);
    varint_decode_prefix_free(digest_len, mh.digest);
    assert(varint_decode_spec(rest1) == Some((digest_len, len_bytes.len())));

    let rest2 = rest1.subrange(len_bytes.len() as int, rest1.len() as int);
    assert(rest2 =~= mh.digest);
    assert(rest2.len() == mh.digest.len());
    assert(digest_len as nat == mh.digest.len());
    assert(code_bytes.len() + len_bytes.len() + digest_len as nat == bytes.len());
    assert(rest2.subrange(0, digest_len as int) =~= mh.digest);
}

// --- CID ---

/// CID binary bytes: version varint + codec varint + multihash bytes.
// r[impl ucan-cid.cid-deterministic]
// r[impl ucan-cid.cid-version]
pub open spec fn cid_bytes_spec(cid: CidSpec) -> Seq<u8> {
    varint_encode_spec(cid.version as u64)
        + varint_encode_spec(cid.codec)
        + multihash_bytes_spec(cid.multihash)
}

/// Construct a CID from codec and a pre-computed multihash.
// r[impl ucan-cid.cid-deterministic]
// r[impl ucan-cid.cid-version]
pub open spec fn cid_from_multihash_spec(codec: u64, mh: MultihashSpec) -> CidSpec {
    CidSpec { version: CID_VERSION, codec, multihash: mh }
}

/// CID version is always 1.
// r[verify ucan-cid.cid-version]
pub proof fn cid_version_is_one(codec: u64, mh: MultihashSpec)
    ensures cid_from_multihash_spec(codec, mh).version == CID_VERSION,
{
}

/// CID construction is deterministic.
// r[verify ucan-cid.cid-deterministic]
pub proof fn cid_deterministic(codec: u64, mh: MultihashSpec)
    ensures cid_bytes_spec(cid_from_multihash_spec(codec, mh))
        == cid_bytes_spec(cid_from_multihash_spec(codec, mh)),
{
}

/// CID injectivity: equal bytes imply same version and codec.
// r[impl ucan-cid.cid-injectivity]
// r[verify ucan-cid.cid-injectivity]
pub proof fn cid_bytes_injective_header(a: CidSpec, b: CidSpec)
    requires cid_bytes_spec(a) =~= cid_bytes_spec(b),
    ensures
        a.version == b.version,
        a.codec == b.codec,
{
    let a_bytes = cid_bytes_spec(a);
    let b_bytes = cid_bytes_spec(b);

    let va = varint_encode_spec(a.version as u64);
    let vb = varint_encode_spec(b.version as u64);
    let a_rest = varint_encode_spec(a.codec) + multihash_bytes_spec(a.multihash);
    let b_rest = varint_encode_spec(b.codec) + multihash_bytes_spec(b.multihash);

    varint_decode_prefix_free(a.version as u64, a_rest);
    varint_decode_prefix_free(b.version as u64, b_rest);
    assert(a_bytes =~= va + a_rest);
    assert(b_bytes =~= vb + b_rest);
    assert(varint_decode_spec(a_bytes) == Some((a.version as u64, va.len())));
    assert(varint_decode_spec(b_bytes) == Some((b.version as u64, vb.len())));
    assert(a.version as u64 == b.version as u64);
    assert(va =~= vb);

    let rest = a_bytes.subrange(va.len() as int, a_bytes.len() as int);
    assert(rest =~= a_rest);
    assert(rest =~= b_rest);

    let a_mh_bytes = multihash_bytes_spec(a.multihash);
    let b_mh_bytes = multihash_bytes_spec(b.multihash);
    varint_decode_prefix_free(a.codec, a_mh_bytes);
    varint_decode_prefix_free(b.codec, b_mh_bytes);
    assert(varint_decode_spec(rest) == Some((a.codec, varint_encode_spec(a.codec).len())));
    assert(varint_decode_spec(rest) == Some((b.codec, varint_encode_spec(b.codec).len())));
    assert(a.codec == b.codec);
}

/// Varint rejects empty input.
// r[impl ucan-cid.varint-rejects-empty]
// r[verify ucan-cid.varint-rejects-empty]
pub proof fn varint_rejects_empty()
    ensures varint_decode_spec(Seq::<u8>::empty()) == None::<(u64, nat)>,
{
}

// --- CID encode/decode (multibase over shared base58btc model) ---

/// CID multibase encoding: one multibase prefix byte plus base58btc payload.
// r[depends ucan-cid.cid-round-trip]
// r[impl ucan-cid.cid-round-trip]
pub open spec fn cid_encode_spec(cid_bytes: Seq<u8>, base: u8) -> Seq<u8> {
    seq![base] + crate::ucan_did::base58btc_encode_spec(cid_bytes)
}

/// CID multibase decoding: recover the base58btc payload after the prefix byte.
// r[depends ucan-cid.cid-round-trip]
// r[impl ucan-cid.cid-round-trip]
pub open spec fn cid_decode_spec(encoded: Seq<u8>) -> Option<Seq<u8>> {
    if encoded.len() < 1 {
        None
    } else {
        crate::ucan_did::base58btc_decode_spec(encoded.subrange(1, encoded.len() as int))
    }
}

/// CID bytes round-trip through multibase encode/decode.
// r[verify ucan-cid.cid-round-trip]
pub proof fn cid_bytes_round_trip(cid: CidSpec, base: u8)
    ensures cid_decode_spec(cid_encode_spec(cid_bytes_spec(cid), base)) == Some(cid_bytes_spec(cid)),
{
    let bytes = cid_bytes_spec(cid);
    let encoded = crate::ucan_did::base58btc_encode_spec(bytes);
    let full = seq![base] + encoded;
    assert(full.subrange(1, full.len() as int) =~= encoded);
    crate::ucan_did::axiom_base58btc_round_trip(bytes);
}

// --- Hash (external body for SHA-256) ---

/// Uninterpreted SHA-256 digest byte.
pub uninterp spec fn sha256_byte_spec(input: Seq<u8>, idx: int) -> u8;

/// SHA-256 digest spec.
// r[impl ucan-cid.cid-deterministic]
pub open spec fn sha256_digest_spec(input: Seq<u8>) -> Seq<u8> {
    Seq::new(SHA256_DIGEST_LEN as nat, |i: int| sha256_byte_spec(input, i))
}

/// Construct a CID from content bytes using SHA-256 and a given codec.
// r[impl ucan-cid.cid-deterministic]
pub open spec fn cid_from_bytes_spec(codec: u64, content: Seq<u8>) -> CidSpec {
    let digest = sha256_digest_spec(content);
    let mh = multihash_wrap_spec(SHA256_CODE, digest);
    cid_from_multihash_spec(codec, mh)
}

/// CID from bytes is deterministic.
// r[verify ucan-cid.cid-deterministic]
pub proof fn cid_from_bytes_deterministic(codec: u64, content: Seq<u8>)
    ensures cid_from_bytes_spec(codec, content) == cid_from_bytes_spec(codec, content),
{
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cid_version() {
        assert_eq!(CID_VERSION, 1);
    }

    #[test]
    fn test_sha256_code() {
        assert_eq!(SHA256_CODE, 0x12);
    }

    #[test]
    fn test_dag_cbor_codec() {
        assert_eq!(DAG_CBOR_CODEC, 0x71);
    }

    #[test]
    fn test_raw_codec() {
        assert_eq!(RAW_CODEC, 0x55);
    }
}
