//! Verified did:key encoding and decoding for UCAN tokens.
//!
//! Models the `did:key:z...` encoding: multicodec varint prefix for the
//! key type (0xed for Ed25519) followed by the raw public key bytes,
//! encoded as base58btc with the multibase 'z' prefix.
//!
//! Varint encode/decode is fully internalized. Base58btc is external_body
//! with assumed round-trip. The proof surface covers DID round-trip,
//! injectivity, codec preservation, and prefix validity.

#![allow(clippy::ptr_arg)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::vec_init_then_push)]

use vstd::prelude::*;

// --- Base58btc runtime (invisible to Verus) ---

#[cfg(not(verus_keep_ghost))]
fn base58btc_encode_runtime(bytes: &[u8]) -> Vec<u8> {
    bs58::encode(bytes).into_vec()
}

#[cfg(verus_keep_ghost)]
fn base58btc_encode_runtime(_bytes: &[u8]) -> Vec<u8> {
    Vec::new()
}

#[cfg(not(verus_keep_ghost))]
fn base58btc_decode_runtime(encoded: &[u8]) -> Option<Vec<u8>> {
    bs58::decode(encoded).into_vec().ok()
}

#[cfg(verus_keep_ghost)]
fn base58btc_decode_runtime(_encoded: &[u8]) -> Option<Vec<u8>> {
    None
}

verus! {

/// Multicodec code for Ed25519 public keys.
pub const CODEC_ED25519: u64 = 0xed;

/// did:key method prefix bytes: "did:key:z"
pub const DID_KEY_PREFIX_LEN: usize = 9;

/// LEB128 numeric base for one payload group.
pub const VARINT_BASE: u64 = 128;
/// Continuation bit mask for a LEB128 byte.
pub const VARINT_CONTINUATION_BIT: u8 = 128;
/// Payload mask for a LEB128 byte.
pub const VARINT_PAYLOAD_MASK: u8 = 127;
/// Continuation byte with all payload bits set.
pub const VARINT_MAX_CONTINUATION_BYTE: u8 = 255;
/// Final payload that overflows at the last u64 varint byte.
pub const VARINT_OVERFLOW_FINAL_PAYLOAD: u8 = 2;
/// Maximum number of LEB128 bytes needed for any u64.
pub const MAX_U64_VARINT_BYTES: usize = 10;

/// Parsed did:key with decoded public key bytes and codec.
pub struct DidKey {
    pub public_key_bytes: Vec<u8>,
    pub codec: u64,
    pub encoded: Vec<u8>,
}

// --- Varint spec (LEB128, fully internalized) ---

/// Varint encode spec: LEB128 encoding of a u64.
// r[impl ucan-did.varint-round-trip]
pub open spec fn varint_encode_spec(n: u64) -> Seq<u8>
    decreases n,
{
    if n < VARINT_BASE {
        seq![n as u8]
    } else {
        seq![(n % VARINT_BASE + VARINT_CONTINUATION_BIT as u64) as u8]
            + varint_encode_spec((n / VARINT_BASE) as u64)
    }
}

pub open spec fn checked_mul_u64_spec(a: u64, b: u64) -> Option<u64> {
    if a as int * b as int > u64::MAX as int {
        None
    } else {
        Some((a as int * b as int) as u64)
    }
}

pub open spec fn checked_add_u64_spec(a: u64, b: u64) -> Option<u64> {
    if a as int + b as int > u64::MAX as int {
        None
    } else {
        Some((a as int + b as int) as u64)
    }
}

pub open spec fn varint_payload_spec(byte: u8) -> u64 {
    if byte < VARINT_CONTINUATION_BIT {
        byte as u64
    } else {
        (byte - VARINT_CONTINUATION_BIT) as u64
    }
}

pub open spec fn varint_decode_fuel_spec(bytes: Seq<u8>, fuel: nat) -> Option<(u64, nat)>
    decreases fuel,
{
    if fuel == 0 || bytes.len() == 0 {
        None
    } else {
        let byte = bytes[0];
        if byte < VARINT_CONTINUATION_BIT {
            Some((byte as u64, 1nat))
        } else {
            let tail = bytes.subrange(1, bytes.len() as int);
            match varint_decode_fuel_spec(tail, (fuel - 1) as nat) {
                None => None,
                Some((rest_value, rest_len)) => {
                    let low = varint_payload_spec(byte);
                    match checked_mul_u64_spec(rest_value, VARINT_BASE) {
                        None => None,
                        Some(scaled_rest) => match checked_add_u64_spec(low, scaled_rest) {
                            None => None,
                            Some(value) => Some((value, rest_len + 1)),
                        },
                    }
                }
            }
        }
    }
}

/// Varint decode spec: returns (value, bytes_consumed) or None.
// r[impl ucan-did.varint-round-trip]
// r[impl ucan-did.varint-bounded-u64]
// r[impl verification-gaps.close-ucan-did-varint-decode]
pub open spec fn varint_decode_spec(bytes: Seq<u8>) -> Option<(u64, nat)> {
    varint_decode_fuel_spec(bytes, MAX_U64_VARINT_BYTES as nat)
}

proof fn checked_mul_u64_fits(a: u64, b: u64)
    requires a as int * b as int <= u64::MAX as int,
    ensures checked_mul_u64_spec(a, b) == Some((a as int * b as int) as u64),
{
}

proof fn checked_add_u64_fits(a: u64, b: u64)
    requires a as int + b as int <= u64::MAX as int,
    ensures checked_add_u64_spec(a, b) == Some((a as int + b as int) as u64),
{
}

pub open spec fn pow128_spec(exp: nat) -> int
    decreases exp,
{
    if exp == 0 { 1int } else { VARINT_BASE as int * pow128_spec((exp - 1) as nat) }
}

proof fn pow128_positive(exp: nat)
    ensures pow128_spec(exp) >= 1,
    decreases exp,
{
    if exp > 0 {
        pow128_positive((exp - 1) as nat);
    }
}

proof fn varint_encode_len_bounded_by(n: u64, budget: nat)
    requires
        budget > 0,
        (n as int) < pow128_spec(budget),
    ensures varint_encode_spec(n).len() <= budget,
    decreases budget,
{
    if n < VARINT_BASE {
        assert(varint_encode_spec(n).len() == 1);
    } else {
        assert(budget > 1) by {
            if budget == 1 {
                assert((budget - 1) as nat == 0nat);
                assert(pow128_spec((budget - 1) as nat) == 1int);
                assert(pow128_spec(budget) == VARINT_BASE as int * pow128_spec((budget - 1) as nat));
                assert(pow128_spec(budget) == VARINT_BASE as int);
                assert(n as int >= VARINT_BASE as int);
            }
        }
        let next_budget = (budget - 1) as nat;
        pow128_positive(next_budget);
        assert(pow128_spec(budget) == VARINT_BASE as int * pow128_spec(next_budget));
        assert(((n / VARINT_BASE) as int) < pow128_spec(next_budget)) by(nonlinear_arith)
            requires
                (n as int) < (VARINT_BASE as int) * pow128_spec(next_budget),
                pow128_spec(next_budget) >= 1,
                n >= VARINT_BASE;
        varint_encode_len_bounded_by((n / VARINT_BASE) as u64, next_budget);
        assert(varint_encode_spec(n).len() == 1 + varint_encode_spec((n / VARINT_BASE) as u64).len());
    }
}

proof fn varint_encode_len_bounded(n: u64)
    ensures varint_encode_spec(n).len() <= MAX_U64_VARINT_BYTES as nat,
{
    assert(pow128_spec(MAX_U64_VARINT_BYTES as nat) > u64::MAX as int) by(compute_only);
    varint_encode_len_bounded_by(n, MAX_U64_VARINT_BYTES as nat);
}

proof fn varint_decode_fuel_prefix_free(n: u64, suffix: Seq<u8>, fuel: nat)
    requires varint_encode_spec(n).len() <= fuel,
    ensures
        varint_decode_fuel_spec(varint_encode_spec(n) + suffix, fuel)
            == Some((n, varint_encode_spec(n).len())),
    decreases n,
{
    let enc = varint_encode_spec(n);
    if n < VARINT_BASE {
        assert(enc =~= seq![n as u8]);
        assert(varint_decode_fuel_spec(enc + suffix, fuel) == Some((n, 1nat)));
    } else {
        let tail_val = (n / VARINT_BASE) as u64;
        let low = (n % VARINT_BASE) as u64;
        let low_byte = (low + VARINT_CONTINUATION_BIT as u64) as u8;
        let tail_enc = varint_encode_spec(tail_val);
        assert(enc =~= seq![low_byte] + tail_enc);
        assert(enc.len() == 1 + tail_enc.len());
        assert(tail_enc.len() <= fuel - 1);
        assert(varint_payload_spec(low_byte) == low);
        varint_decode_fuel_prefix_free(tail_val, suffix, (fuel - 1) as nat);
        assert(varint_decode_fuel_spec(tail_enc + suffix, (fuel - 1) as nat)
            == Some((tail_val, tail_enc.len())));
        assert(tail_val as int * VARINT_BASE as int <= n as int) by(nonlinear_arith)
            requires tail_val == n / VARINT_BASE, n >= VARINT_BASE;
        assert(tail_val as int * VARINT_BASE as int <= u64::MAX as int) by(nonlinear_arith)
            requires tail_val as int * VARINT_BASE as int <= n as int;
        checked_mul_u64_fits(tail_val, VARINT_BASE);
        let scaled_int = tail_val as int * VARINT_BASE as int;
        assert(tail_val as int >= 0);
        assert(VARINT_BASE as int > 0);
        assert(scaled_int >= 0) by(nonlinear_arith)
            requires tail_val as int >= 0, VARINT_BASE as int > 0, scaled_int == tail_val as int * VARINT_BASE as int;
        assert(scaled_int <= u64::MAX as int);
        let scaled = scaled_int as u64;
        assert(scaled as int == scaled_int);
        assert(scaled as int == tail_val as int * VARINT_BASE as int);
        assert(low as int + scaled as int <= u64::MAX as int) by(nonlinear_arith)
            requires
                low == n % VARINT_BASE,
                tail_val == n / VARINT_BASE,
                scaled as int == tail_val as int * VARINT_BASE as int;
        checked_add_u64_fits(low, scaled);
        assert(low + scaled == n) by(nonlinear_arith)
            requires
                low == n % VARINT_BASE,
                tail_val == n / VARINT_BASE,
                scaled as int == tail_val as int * VARINT_BASE as int,
                n >= VARINT_BASE;
        assert((enc + suffix).subrange(1, (enc + suffix).len() as int) =~= tail_enc + suffix);
        assert(varint_decode_fuel_spec(enc + suffix, fuel) == Some((n, enc.len())));
    }
}

/// Varint round-trip: encoding then decoding recovers the value.
// r[verify ucan-did.varint-round-trip]
pub proof fn varint_round_trip(n: u64)
    ensures varint_decode_spec(varint_encode_spec(n)) == Some((n, varint_encode_spec(n).len())),
{
    varint_encode_len_bounded(n);
    varint_decode_fuel_prefix_free(n, Seq::<u8>::empty(), MAX_U64_VARINT_BYTES as nat);
}

/// Varint decode rejects empty input.
// r[impl ucan-did.varint-rejects-empty]
// r[verify ucan-did.varint-rejects-empty]
pub proof fn varint_decode_rejects_empty()
    ensures varint_decode_spec(Seq::<u8>::empty()) == None::<(u64, nat)>,
{
}

/// Varint decode rejects a truncated continuation byte.
// r[verify ucan-did.varint-rejects-empty]
pub proof fn varint_decode_rejects_single_continuation()
    ensures varint_decode_spec(seq![VARINT_CONTINUATION_BIT]) == None::<(u64, nat)>,
{
    assert(varint_decode_spec(seq![VARINT_CONTINUATION_BIT]) == None::<(u64, nat)>) by(compute_only);
}

/// Varint decode accepts non-minimal representable zero.
// r[verify ucan-did.varint-bounded-u64]
pub proof fn varint_decode_accepts_nonminimal_zero()
    ensures varint_decode_spec(seq![VARINT_CONTINUATION_BIT, 0u8]) == Some((0u64, 2nat)),
{
    assert(varint_decode_spec(seq![VARINT_CONTINUATION_BIT, 0u8]) == Some((0u64, 2nat))) by(compute_only);
}

/// Varint decode rejects continuation-only input at the u64 byte budget.
// r[verify ucan-did.varint-bounded-u64]
pub proof fn varint_decode_rejects_overlong_continuations()
    ensures varint_decode_spec(seq![
        VARINT_CONTINUATION_BIT,
        VARINT_CONTINUATION_BIT,
        VARINT_CONTINUATION_BIT,
        VARINT_CONTINUATION_BIT,
        VARINT_CONTINUATION_BIT,
        VARINT_CONTINUATION_BIT,
        VARINT_CONTINUATION_BIT,
        VARINT_CONTINUATION_BIT,
        VARINT_CONTINUATION_BIT,
        VARINT_CONTINUATION_BIT,
    ]) == None::<(u64, nat)>,
{
    assert(varint_decode_spec(seq![
        VARINT_CONTINUATION_BIT,
        VARINT_CONTINUATION_BIT,
        VARINT_CONTINUATION_BIT,
        VARINT_CONTINUATION_BIT,
        VARINT_CONTINUATION_BIT,
        VARINT_CONTINUATION_BIT,
        VARINT_CONTINUATION_BIT,
        VARINT_CONTINUATION_BIT,
        VARINT_CONTINUATION_BIT,
        VARINT_CONTINUATION_BIT,
    ]) == None::<(u64, nat)>) by(compute_only);
}

/// Varint decode rejects a final payload that overflows u64.
// r[verify ucan-did.varint-bounded-u64]
pub proof fn varint_decode_rejects_overflowing_final_payload()
    ensures varint_decode_spec(seq![
        VARINT_MAX_CONTINUATION_BYTE,
        VARINT_MAX_CONTINUATION_BYTE,
        VARINT_MAX_CONTINUATION_BYTE,
        VARINT_MAX_CONTINUATION_BYTE,
        VARINT_MAX_CONTINUATION_BYTE,
        VARINT_MAX_CONTINUATION_BYTE,
        VARINT_MAX_CONTINUATION_BYTE,
        VARINT_MAX_CONTINUATION_BYTE,
        VARINT_MAX_CONTINUATION_BYTE,
        VARINT_OVERFLOW_FINAL_PAYLOAD,
    ]) == None::<(u64, nat)>,
{
    assert(varint_decode_spec(seq![
        VARINT_MAX_CONTINUATION_BYTE,
        VARINT_MAX_CONTINUATION_BYTE,
        VARINT_MAX_CONTINUATION_BYTE,
        VARINT_MAX_CONTINUATION_BYTE,
        VARINT_MAX_CONTINUATION_BYTE,
        VARINT_MAX_CONTINUATION_BYTE,
        VARINT_MAX_CONTINUATION_BYTE,
        VARINT_MAX_CONTINUATION_BYTE,
        VARINT_MAX_CONTINUATION_BYTE,
        VARINT_OVERFLOW_FINAL_PAYLOAD,
    ]) == None::<(u64, nat)>) by(compute_only);
}

/// Varint encode always produces at least one byte.
// r[verify ucan-did.varint-round-trip]
pub proof fn varint_encode_nonempty(n: u64)
    ensures varint_encode_spec(n).len() >= 1,
    decreases n,
{
}

// --- Varint exec ---

/// Encode a u64 as LEB128 varint.
// r[impl ucan-did.varint-round-trip]
pub fn varint_encode(n: u64) -> (result: Vec<u8>)
    ensures result@ == varint_encode_spec(n),
{
    let mut out = Vec::new();
    let mut val = n;
    loop
        invariant
            val <= n,
            out@ + varint_encode_spec(val) == varint_encode_spec(n),
        decreases val,
    {
        if val < VARINT_BASE {
            out.push(val as u8);
            assert(out@ =~= out@.subrange(0, out@.len() - 1) + seq![val as u8]);
            return out;
        }
        let low = (val % VARINT_BASE + VARINT_CONTINUATION_BIT as u64) as u8;
        out.push(low);
        val = val / VARINT_BASE;
    }
}

fn checked_mul_u64(a: u64, b: u64) -> (result: Option<u64>)
    ensures result == checked_mul_u64_spec(a, b)
{
    a.checked_mul(b)
}

fn checked_add_u64(a: u64, b: u64) -> (result: Option<u64>)
    ensures result == checked_add_u64_spec(a, b)
{
    a.checked_add(b)
}

fn varint_decode_fuel(
    bytes: &Vec<u8>,
    offset: usize,
    fuel: usize,
) -> (result: Option<(u64, usize)>)
    requires
        offset <= bytes@.len(),
        fuel <= MAX_U64_VARINT_BYTES,
        offset + fuel <= usize::MAX,
    ensures
        match result {
            Some((val, len)) => varint_decode_fuel_spec(
                    bytes@.subrange(offset as int, bytes@.len() as int),
                    fuel as nat,
                ) == Some((val, len as nat))
                && len <= bytes@.len() - offset,
            None => varint_decode_fuel_spec(
                bytes@.subrange(offset as int, bytes@.len() as int),
                fuel as nat,
            ).is_none(),
        },
    decreases fuel,
{
    if fuel == 0 || offset >= bytes.len() {
        return None;
    }

    let byte = bytes[offset];
    proof {
        let view = bytes@.subrange(offset as int, bytes@.len() as int);
        assert(view.len() > 0);
        assert(view[0] == byte);
    }
    if byte < VARINT_CONTINUATION_BIT {
        proof {
            let view = bytes@.subrange(offset as int, bytes@.len() as int);
            assert(varint_decode_fuel_spec(view, fuel as nat) == Some((byte as u64, 1nat)));
        }
        return Some((byte as u64, 1usize));
    }

    proof {
        assert(offset + 1 <= bytes@.len());
        assert((offset + 1) + (fuel - 1) == offset + fuel);
        let view = bytes@.subrange(offset as int, bytes@.len() as int);
        assert(view.subrange(1, view.len() as int) =~= bytes@.subrange((offset + 1) as int, bytes@.len() as int));
    }
    let tail = varint_decode_fuel(bytes, offset + 1, fuel - 1);
    match tail {
        None => {
            proof {
                let view = bytes@.subrange(offset as int, bytes@.len() as int);
                let tail_view = bytes@.subrange((offset + 1) as int, bytes@.len() as int);
                assert(view.subrange(1, view.len() as int) =~= tail_view);
                assert(varint_decode_fuel_spec(tail_view, (fuel - 1) as nat).is_none());
                assert(varint_decode_fuel_spec(view, fuel as nat).is_none());
            }
            None
        },
        Some((rest_value, rest_len)) => {
            let low = (byte - VARINT_CONTINUATION_BIT) as u64;
            proof {
                assert(varint_payload_spec(byte) == low);
            }
            let scaled_rest = match checked_mul_u64(rest_value, VARINT_BASE) {
                Some(value) => value,
                None => {
                    proof {
                        let view = bytes@.subrange(offset as int, bytes@.len() as int);
                        let tail_view = bytes@.subrange((offset + 1) as int, bytes@.len() as int);
                        assert(view.subrange(1, view.len() as int) =~= tail_view);
                        assert(varint_decode_fuel_spec(tail_view, (fuel - 1) as nat)
                            == Some((rest_value, rest_len as nat)));
                        assert(checked_mul_u64_spec(rest_value, VARINT_BASE).is_none());
                        assert(varint_decode_fuel_spec(view, fuel as nat).is_none());
                    }
                    return None;
                },
            };
            let value = match checked_add_u64(low, scaled_rest) {
                Some(value) => value,
                None => {
                    proof {
                        let view = bytes@.subrange(offset as int, bytes@.len() as int);
                        let tail_view = bytes@.subrange((offset + 1) as int, bytes@.len() as int);
                        assert(view.subrange(1, view.len() as int) =~= tail_view);
                        assert(varint_decode_fuel_spec(tail_view, (fuel - 1) as nat)
                            == Some((rest_value, rest_len as nat)));
                        assert(checked_mul_u64_spec(rest_value, VARINT_BASE) == Some(scaled_rest));
                        assert(checked_add_u64_spec(low, scaled_rest).is_none());
                        assert(varint_decode_fuel_spec(view, fuel as nat).is_none());
                    }
                    return None;
                },
            };
            proof {
                let view = bytes@.subrange(offset as int, bytes@.len() as int);
                let tail_view = bytes@.subrange((offset + 1) as int, bytes@.len() as int);
                assert(view.subrange(1, view.len() as int) =~= tail_view);
                assert(varint_decode_fuel_spec(tail_view, (fuel - 1) as nat)
                    == Some((rest_value, rest_len as nat)));
                assert(checked_mul_u64_spec(rest_value, VARINT_BASE) == Some(scaled_rest));
                assert(checked_add_u64_spec(low, scaled_rest) == Some(value));
                assert(varint_decode_fuel_spec(view, fuel as nat) == Some((value, rest_len as nat + 1)));
                assert(rest_len + 1 <= bytes@.len() - offset) by(nonlinear_arith)
                    requires rest_len <= bytes@.len() - (offset + 1);
            }
            Some((value, rest_len + 1))
        }
    }
}

/// Decode a LEB128 varint from the start of a byte slice.
// r[impl ucan-did.varint-round-trip]
// r[impl ucan-did.varint-bounded-u64]
// r[impl verification-gaps.close-ucan-did-varint-decode]
// r[verify verification-gaps.close-ucan-did-varint-decode]
// r[impl verification-gaps.inventory-after-ucan-did-varint]
// r[verify verification-gaps.inventory-after-ucan-did-varint]
pub fn varint_decode(bytes: &Vec<u8>) -> (result: Option<(u64, usize)>)
    ensures
        match result {
            Some((val, len)) => varint_decode_spec(bytes@) == Some((val, len as nat))
                && len <= bytes@.len(),
            None => varint_decode_spec(bytes@).is_none(),
        },
{
    let result = varint_decode_fuel(bytes, 0usize, MAX_U64_VARINT_BYTES);
    proof {
        assert(bytes@.subrange(0, bytes@.len() as int) =~= bytes@);
        assert(varint_decode_spec(bytes@) == varint_decode_fuel_spec(bytes@, MAX_U64_VARINT_BYTES as nat));
    }
    result
}

// --- Multicodec framing spec ---

/// Multicodec-prefixed key bytes.
// r[impl ucan-did.codec-preserved]
pub open spec fn multicodec_prefix_spec(codec: u64, key_bytes: Seq<u8>) -> Seq<u8> {
    varint_encode_spec(codec) + key_bytes
}

/// Parse multicodec prefix: extract (codec, key_bytes).
// r[impl ucan-did.codec-preserved]
pub open spec fn multicodec_parse_spec(prefixed: Seq<u8>) -> Option<(u64, Seq<u8>)> {
    match varint_decode_spec(prefixed) {
        None => None,
        Some((codec, len)) => Some((codec, prefixed.subrange(len as int, prefixed.len() as int))),
    }
}

/// Multicodec round-trip: prefix then parse recovers codec and key.
// r[verify ucan-did.codec-preserved]
pub proof fn multicodec_round_trip(codec: u64, key_bytes: Seq<u8>)
    ensures multicodec_parse_spec(multicodec_prefix_spec(codec, key_bytes)) == Some((codec, key_bytes)),
{
    let enc = varint_encode_spec(codec);
    let prefixed = enc + key_bytes;
    varint_round_trip(codec);
    varint_encode_nonempty(codec);
    varint_decode_prefix_free(codec, key_bytes);
    let vlen = enc.len();
    assert(prefixed.subrange(vlen as int, prefixed.len() as int) =~= key_bytes);
}

/// Varint decode is prefix-free: extra trailing bytes don't change the result.
// r[verify ucan-did.varint-round-trip]
proof fn varint_decode_prefix_free(n: u64, suffix: Seq<u8>)
    ensures
        varint_decode_spec(varint_encode_spec(n) + suffix)
            == Some((n, varint_encode_spec(n).len())),
    decreases n,
{
    varint_encode_len_bounded(n);
    varint_decode_fuel_prefix_free(n, suffix, MAX_U64_VARINT_BYTES as nat);
}

// --- Base58btc spec (external body) ---

/// Uninterpreted base58btc encoding model.
// r[depends ucan-did.round-trip]
pub uninterp spec fn base58btc_encode_byte_spec(bytes: Seq<u8>, idx: int) -> u8;

/// Base58btc encoded output.
// r[impl ucan-did.round-trip]
pub uninterp spec fn base58btc_encode_spec(bytes: Seq<u8>) -> Seq<u8>;

/// One-byte Bitcoin/base58 alphabet membership.
// r[impl ucan-did.base58btc-alphabet]
pub open spec fn base58btc_byte_valid_spec(byte: u8) -> bool {
    (0x31u8 <= byte <= 0x39u8) // 1-9
        || (0x41u8 <= byte <= 0x48u8) // A-H
        || (0x4au8 <= byte <= 0x4eu8) // J-N, excludes I
        || (0x50u8 <= byte <= 0x5au8) // P-Z, excludes O
        || (0x61u8 <= byte <= 0x6bu8) // a-k
        || (0x6du8 <= byte <= 0x7au8) // m-z, excludes l
}

/// Base58btc alphabet validity for an encoded byte sequence.
// r[impl ucan-did.base58btc-alphabet]
pub open spec fn base58btc_chars_valid_spec(encoded: Seq<u8>) -> bool {
    forall|i: int| 0 <= i < encoded.len() ==> base58btc_byte_valid_spec(encoded[i])
}

/// Trusted base58btc decode core, reached only after verified alphabet screening.
pub uninterp spec fn base58btc_decode_core_spec(encoded: Seq<u8>) -> Option<Seq<u8>>;

/// Base58btc decode model.
// r[impl ucan-did.round-trip]
// r[impl ucan-did.base58btc-alphabet]
pub open spec fn base58btc_decode_spec(encoded: Seq<u8>) -> Option<Seq<u8>> {
    if base58btc_chars_valid_spec(encoded) {
        base58btc_decode_core_spec(encoded)
    } else {
        None
    }
}

/// Axiom: base58btc round-trips.
// r[impl ucan-did.base58btc-round-trip]
// r[verify ucan-did.base58btc-round-trip]
#[verifier::external_body]
pub proof fn axiom_base58btc_round_trip(bytes: Seq<u8>)
    ensures base58btc_decode_spec(base58btc_encode_spec(bytes)) == Some(bytes),
{
}

/// Axiom: base58btc is injective.
// r[verify ucan-did.injective]
#[verifier::external_body]
pub proof fn axiom_base58btc_injective(a: Seq<u8>, b: Seq<u8>)
    requires a != b,
    ensures base58btc_encode_spec(a) != base58btc_encode_spec(b),
{
}

/// Base58btc encode exec.
// r[impl ucan-did.round-trip]
#[verifier::external_body]
pub fn base58btc_encode(bytes: &Vec<u8>) -> (result: Vec<u8>)
    ensures result@ == base58btc_encode_spec(bytes@),
{
    base58btc_encode_runtime(bytes)
}

/// Base58btc decode exec.
// r[impl ucan-did.round-trip]
#[verifier::external_body]
pub fn base58btc_decode(encoded: &Vec<u8>) -> (result: Option<Vec<u8>>)
    ensures
        match result {
            Some(v) => base58btc_decode_spec(encoded@) == Some(v@),
            None => base58btc_decode_spec(encoded@).is_none(),
        },
{
    base58btc_decode_runtime(encoded)
}

// --- DID encode/decode spec ---

/// The did:key prefix string bytes: "did:key:z".
// r[impl ucan-did.prefix-valid]
pub open spec fn did_key_prefix_spec() -> Seq<u8> {
    seq![
        0x64u8, 0x69u8, 0x64u8, 0x3au8, // "did:"
        0x6bu8, 0x65u8, 0x79u8, 0x3au8, // "key:"
        0x7au8,                           // "z"
    ]
}

/// Encode a public key and codec as a did:key string.
// r[impl ucan-did.round-trip]
// r[impl ucan-did.prefix-valid]
pub open spec fn encode_did_key_spec(public_key: Seq<u8>, codec: u64) -> Seq<u8> {
    did_key_prefix_spec() + base58btc_encode_spec(multicodec_prefix_spec(codec, public_key))
}

/// Decode a did:key string to recover public key bytes and codec.
// r[impl ucan-did.round-trip]
pub open spec fn decode_did_key_spec(did_bytes: Seq<u8>) -> Option<(Seq<u8>, u64)> {
    let prefix = did_key_prefix_spec();
    if did_bytes.len() < prefix.len() {
        None
    } else if did_bytes.subrange(0, prefix.len() as int) != prefix {
        None
    } else {
        let b58_part = did_bytes.subrange(prefix.len() as int, did_bytes.len() as int);
        match base58btc_decode_spec(b58_part) {
            None => None,
            Some(decoded) => {
                match multicodec_parse_spec(decoded) {
                    None => None,
                    Some((codec, key_bytes)) => Some((key_bytes, codec)),
                }
            }
        }
    }
}

/// DID round-trip: encode then decode recovers key and codec.
// r[verify ucan-did.round-trip]
pub proof fn did_key_round_trip(public_key: Seq<u8>, codec: u64)
    ensures decode_did_key_spec(encode_did_key_spec(public_key, codec)) == Some((public_key, codec)),
{
    let prefix = did_key_prefix_spec();
    let mc = multicodec_prefix_spec(codec, public_key);
    let b58 = base58btc_encode_spec(mc);
    let full = prefix + b58;

    // Prefix matches
    assert(full.subrange(0, prefix.len() as int) =~= prefix);

    // Base58 part
    let b58_part = full.subrange(prefix.len() as int, full.len() as int);
    assert(b58_part =~= b58);

    // Base58 round-trip
    axiom_base58btc_round_trip(mc);
    assert(base58btc_decode_spec(b58) == Some(mc));

    // Multicodec round-trip
    multicodec_round_trip(codec, public_key);
    assert(multicodec_parse_spec(mc) == Some((codec, public_key)));
}

/// DID encoding always starts with "did:key:z".
// r[verify ucan-did.prefix-valid]
pub proof fn did_key_has_valid_prefix(public_key: Seq<u8>, codec: u64)
    ensures
        encode_did_key_spec(public_key, codec).len() >= 9,
        encode_did_key_spec(public_key, codec).subrange(0, 9) == did_key_prefix_spec(),
{
    let prefix = did_key_prefix_spec();
    let full = prefix + base58btc_encode_spec(multicodec_prefix_spec(codec, public_key));
    assert(full.subrange(0, prefix.len() as int) =~= prefix);
}

/// Distinct keys produce distinct DIDs (injectivity).
// r[impl ucan-did.injective]
// r[verify ucan-did.injective]
pub proof fn did_key_injective(pk1: Seq<u8>, pk2: Seq<u8>, codec: u64)
    requires pk1 != pk2,
    ensures encode_did_key_spec(pk1, codec) != encode_did_key_spec(pk2, codec),
{
    let mc1 = multicodec_prefix_spec(codec, pk1);
    let mc2 = multicodec_prefix_spec(codec, pk2);
    // Multicodec prefix is the same, key bytes differ => mc1 != mc2
    assert(mc1 != mc2) by {
        let vlen = varint_encode_spec(codec).len();
        varint_encode_nonempty(codec);
        if mc1 =~= mc2 {
            assert(mc1.subrange(vlen as int, mc1.len() as int) =~= pk1);
            assert(mc2.subrange(vlen as int, mc2.len() as int) =~= pk2);
        }
    }
    // Base58btc is injective
    axiom_base58btc_injective(mc1, mc2);
    let prefix = did_key_prefix_spec();
    let full1 = prefix + base58btc_encode_spec(mc1);
    let full2 = prefix + base58btc_encode_spec(mc2);
    // Different suffixes => different full strings
    assert(full1 != full2) by {
        if full1 =~= full2 {
            assert(full1.subrange(prefix.len() as int, full1.len() as int) =~= base58btc_encode_spec(mc1));
            assert(full2.subrange(prefix.len() as int, full2.len() as int) =~= base58btc_encode_spec(mc2));
        }
    }
}

/// Codec is preserved through DID round-trip.
// r[verify ucan-did.codec-preserved]
pub proof fn did_key_codec_preserved(public_key: Seq<u8>, codec: u64)
    ensures
        decode_did_key_spec(encode_did_key_spec(public_key, codec)) matches Some(pair)
            ==> pair.1 == codec,
{
    did_key_round_trip(public_key, codec);
}

// --- Exec functions ---

fn base58btc_byte_valid(byte: u8) -> (result: bool)
    ensures result == base58btc_byte_valid_spec(byte)
{
    (0x31u8 <= byte && byte <= 0x39u8)
        || (0x41u8 <= byte && byte <= 0x48u8)
        || (0x4au8 <= byte && byte <= 0x4eu8)
        || (0x50u8 <= byte && byte <= 0x5au8)
        || (0x61u8 <= byte && byte <= 0x6bu8)
        || (0x6du8 <= byte && byte <= 0x7au8)
}

fn base58btc_chars_valid(encoded: &Vec<u8>) -> (result: bool)
    ensures result == base58btc_chars_valid_spec(encoded@)
{
    let mut i: usize = 0;
    let mut valid = true;
    while i < encoded.len()
        invariant
            i <= encoded.len(),
            valid == (forall|j: int| 0 <= j < i as int ==> base58btc_byte_valid_spec(encoded@[j])),
        decreases encoded.len() - i,
    {
        valid = valid && base58btc_byte_valid(encoded[i]);
        i = i + 1;
    }
    proof {
        if valid {
            assert forall|j: int| 0 <= j < encoded@.len() implies base58btc_byte_valid_spec(encoded@[j]) by {
                assert(j < i as int);
            }
        }
        if base58btc_chars_valid_spec(encoded@) {
            assert forall|j: int| 0 <= j < i as int implies base58btc_byte_valid_spec(encoded@[j]) by {
                assert(j < encoded@.len());
            }
        }
    }
    valid
}

fn did_key_prefix_bytes() -> (result: Vec<u8>)
    ensures result@ == did_key_prefix_spec()
{
    let mut result = Vec::new();
    result.push(0x64u8); // d
    result.push(0x69u8); // i
    result.push(0x64u8); // d
    result.push(0x3au8); // :
    result.push(0x6bu8); // k
    result.push(0x65u8); // e
    result.push(0x79u8); // y
    result.push(0x3au8); // :
    result.push(0x7au8); // z
    proof {
        assert(result@ =~= did_key_prefix_spec());
    }
    result
}

fn bytes_subrange_from(bytes: &Vec<u8>, start: usize) -> (result: Vec<u8>)
    requires start <= bytes@.len()
    ensures result@ == bytes@.subrange(start as int, bytes@.len() as int)
{
    let mut result = Vec::new();
    let ghost suffix = bytes@.subrange(start as int, bytes@.len() as int);
    let mut i = start;
    while i < bytes.len()
        invariant
            start <= i <= bytes.len(),
            suffix == bytes@.subrange(start as int, bytes@.len() as int),
            suffix.len() == bytes.len() - start,
            result@ == suffix.subrange(0, (i - start) as int),
        decreases bytes.len() - i,
    {
        proof {
            let k = (i - start) as int;
            assert(k >= 0);
            assert(i - start < bytes.len() - start) by(nonlinear_arith)
                requires start <= i, i < bytes.len();
            assert(k < suffix.len()) by(nonlinear_arith)
                requires k == (i - start) as int, i - start < bytes.len() - start, suffix.len() == bytes.len() - start;
            assert(start as int + k == i as int) by(nonlinear_arith)
                requires start <= i, k == (i - start) as int;
            assert(suffix[k] == bytes@[i as int]);
        }
        result.push(bytes[i]);
        i = i + 1;
        proof {
            assert(result@ =~= suffix.subrange(0, (i - start) as int));
        }
    }
    proof {
        assert((bytes.len() - start) as int == suffix.len());
        assert(result@ =~= suffix);
    }
    result
}

fn did_key_has_prefix(bytes: &Vec<u8>) -> (result: bool)
    ensures result == (bytes@.len() >= did_key_prefix_spec().len()
        && bytes@.subrange(0, did_key_prefix_spec().len() as int) == did_key_prefix_spec())
{
    let prefix = did_key_prefix_bytes();
    if bytes.len() < prefix.len() {
        return false;
    }
    let mut i: usize = 0;
    let mut matched = true;
    while i < prefix.len()
        invariant
            prefix@ == did_key_prefix_spec(),
            prefix.len() == did_key_prefix_spec().len(),
            bytes.len() >= prefix.len(),
            i <= prefix.len(),
            matched == (forall|j: int| 0 <= j < i as int ==> bytes@[j] == prefix@[j]),
        decreases prefix.len() - i,
    {
        matched = matched && bytes[i] == prefix[i];
        i = i + 1;
    }
    proof {
        if matched {
            assert forall|j: int| 0 <= j < did_key_prefix_spec().len() implies bytes@.subrange(0, did_key_prefix_spec().len() as int)[j] == did_key_prefix_spec()[j] by {
                assert(bytes@[j] == prefix@[j]);
                assert(prefix@[j] == did_key_prefix_spec()[j]);
            }
            assert(bytes@.subrange(0, did_key_prefix_spec().len() as int) =~= did_key_prefix_spec());
        }
        if bytes@.subrange(0, did_key_prefix_spec().len() as int) == did_key_prefix_spec() {
            assert forall|j: int| 0 <= j < prefix.len() as int implies bytes@[j] == prefix@[j] by {
                assert(bytes@.subrange(0, did_key_prefix_spec().len() as int)[j] == bytes@[j]);
                assert(prefix@[j] == did_key_prefix_spec()[j]);
            }
        }
    }
    matched
}

/// Encode a public key and codec as a did:key byte string.
// r[impl ucan-did.round-trip]
// r[impl ucan-did.prefix-valid]
pub fn encode_did_key(public_key: &Vec<u8>, codec: u64) -> (result: DidKey)
    ensures
        result.public_key_bytes@ == public_key@,
        result.codec == codec,
        result.encoded@ == encode_did_key_spec(public_key@, codec),
{
    let mc_prefix = varint_encode(codec);
    let mut mc_bytes = mc_prefix;
    let mut i: usize = 0;
    while i < public_key.len()
        invariant
            0 <= i <= public_key.len(),
            mc_bytes@ == varint_encode_spec(codec) + public_key@.subrange(0, i as int),
        decreases public_key.len() - i,
    {
        mc_bytes.push(public_key[i]);
        i = i + 1;
    }
    assert(mc_bytes@ =~= multicodec_prefix_spec(codec, public_key@));
    let b58 = base58btc_encode(&mc_bytes);
    // Build "did:key:z" + b58
    let mut encoded = Vec::new();
    encoded.push(0x64u8); // d
    encoded.push(0x69u8); // i
    encoded.push(0x64u8); // d
    encoded.push(0x3au8); // :
    encoded.push(0x6bu8); // k
    encoded.push(0x65u8); // e
    encoded.push(0x79u8); // y
    encoded.push(0x3au8); // :
    encoded.push(0x7au8); // z
    let mut j: usize = 0;
    while j < b58.len()
        invariant
            0 <= j <= b58.len(),
            encoded@ == did_key_prefix_spec() + b58@.subrange(0, j as int),
        decreases b58.len() - j,
    {
        encoded.push(b58[j]);
        j = j + 1;
    }
    assert(encoded@ =~= did_key_prefix_spec() + b58@);
    assert(encoded@ =~= encode_did_key_spec(public_key@, codec));
    DidKey {
        public_key_bytes: public_key.clone(),
        codec,
        encoded,
    }
}

/// Decode a did:key byte string into its public-key bytes and codec.
// r[impl ucan-did.round-trip]
// r[impl ucan-did.codec-preserved]
pub fn decode_did_key(encoded: &Vec<u8>) -> (result: Option<DidKey>)
    ensures
        match result {
            Some(did) => decode_did_key_spec(encoded@) == Some((did.public_key_bytes@, did.codec))
                && did.encoded@ == encoded@,
            None => decode_did_key_spec(encoded@).is_none(),
        },
{
    if !did_key_has_prefix(encoded) {
        return None;
    }

    let prefix = did_key_prefix_bytes();
    let b58_part = bytes_subrange_from(encoded, prefix.len());
    proof {
        assert(prefix@ == did_key_prefix_spec());
        assert(b58_part@ == encoded@.subrange(did_key_prefix_spec().len() as int, encoded@.len() as int));
    }
    if !base58btc_chars_valid(&b58_part) {
        proof {
            assert(!base58btc_chars_valid_spec(b58_part@));
            assert(base58btc_decode_spec(b58_part@).is_none());
            assert(decode_did_key_spec(encoded@).is_none());
        }
        return None;
    }

    let decoded = match base58btc_decode(&b58_part) {
        Some(v) => v,
        None => {
            proof {
                assert(base58btc_decode_spec(b58_part@).is_none());
                assert(decode_did_key_spec(encoded@).is_none());
            }
            return None;
        },
    };

    let parsed = match varint_decode(&decoded) {
        Some(v) => v,
        None => {
            proof {
                assert(varint_decode_spec(decoded@).is_none());
                assert(multicodec_parse_spec(decoded@).is_none());
                assert(decode_did_key_spec(encoded@).is_none());
            }
            return None;
        },
    };
    let (codec, prefix_len) = parsed;
    let public_key_bytes = bytes_subrange_from(&decoded, prefix_len);
    proof {
        assert(varint_decode_spec(decoded@) == Some((codec, prefix_len as nat)));
        assert(multicodec_parse_spec(decoded@) == Some((codec, public_key_bytes@)));
        assert(decode_did_key_spec(encoded@) == Some((public_key_bytes@, codec)));
    }

    Some(DidKey {
        public_key_bytes,
        codec,
        encoded: encoded.clone(),
    })
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_SINGLE_BYTE_VALUE: u64 = 0x55;
    const SAMPLE_MULTI_BYTE_VALUE: u64 = 0x1200;
    const HALF_DIVISOR: u64 = 2;
    const SAMPLE_BOUNDARY_VALUES: [u64; 9] = [
        0,
        1,
        VARINT_PAYLOAD_MASK as u64,
        VARINT_BASE,
        VARINT_MAX_CONTINUATION_BYTE as u64,
        VARINT_BASE * HALF_DIVISOR,
        CODEC_ED25519,
        SAMPLE_MULTI_BYTE_VALUE,
        u64::MAX / HALF_DIVISOR,
    ];
    const ED25519_VARINT: &[u8] = &[CODEC_ED25519 as u8, (CODEC_ED25519 / VARINT_BASE) as u8];
    const SUFFIX_BYTE: u8 = VARINT_MAX_CONTINUATION_BYTE;
    const NON_MINIMAL_ZERO: &[u8] = &[VARINT_CONTINUATION_BIT, 0];
    const ED25519_PUBLIC_KEY_LEN: usize = 32;
    const ED25519_MULTICODEC_KEY_LEN: usize = ED25519_PUBLIC_KEY_LEN + ED25519_VARINT.len();

    #[test]
    fn test_varint_encode_single_byte() {
        let encoded = varint_encode(SAMPLE_SINGLE_BYTE_VALUE);
        assert_eq!(encoded, vec![SAMPLE_SINGLE_BYTE_VALUE as u8]);
    }

    #[test]
    fn test_varint_encode_two_bytes() {
        let encoded = varint_encode(CODEC_ED25519);
        assert_eq!(encoded, ED25519_VARINT);
    }

    #[test]
    fn test_varint_round_trip() {
        for n in SAMPLE_BOUNDARY_VALUES {
            let encoded = varint_encode(n);
            let decoded = varint_decode(&encoded);
            assert!(decoded.is_some(), "varint_decode failed for {n}");
            let (val, len) = decoded.unwrap();
            assert_eq!(val, n, "round-trip failed for {n}");
            assert_eq!(len, encoded.len(), "length mismatch for {n}");
        }
    }

    #[test]
    fn test_varint_decode_empty() {
        let empty = Vec::new();
        assert!(varint_decode(&empty).is_none());
    }

    #[test]
    fn test_varint_decode_truncated_continuation() {
        let truncated = vec![VARINT_CONTINUATION_BIT];
        assert!(varint_decode(&truncated).is_none());
    }

    #[test]
    fn test_varint_decode_ed25519_consumes_two_bytes() {
        let encoded = ED25519_VARINT.to_vec();
        assert_eq!(varint_decode(&encoded), Some((CODEC_ED25519, ED25519_VARINT.len())));
    }

    #[test]
    fn test_varint_decode_trailing_suffix_not_consumed() {
        let mut encoded = varint_encode(CODEC_ED25519);
        let encoded_len = encoded.len();
        encoded.push(SUFFIX_BYTE);
        assert_eq!(varint_decode(&encoded), Some((CODEC_ED25519, encoded_len)));
    }

    #[test]
    fn test_varint_decode_accepts_non_minimal_representable() {
        let encoded = NON_MINIMAL_ZERO.to_vec();
        assert_eq!(varint_decode(&encoded), Some((0, NON_MINIMAL_ZERO.len())));
    }

    #[test]
    fn test_varint_decode_rejects_overlong_without_terminator() {
        let overlong = vec![VARINT_CONTINUATION_BIT; MAX_U64_VARINT_BYTES];
        assert!(varint_decode(&overlong).is_none());
    }

    #[test]
    fn test_varint_decode_rejects_overflowing_final_payload() {
        let mut encoded = vec![VARINT_MAX_CONTINUATION_BYTE; MAX_U64_VARINT_BYTES - 1];
        encoded.push(VARINT_OVERFLOW_FINAL_PAYLOAD);
        assert!(varint_decode(&encoded).is_none());
    }

    #[test]
    fn test_did_key_encode_ed25519() {
        let pk = vec![1u8; ED25519_PUBLIC_KEY_LEN];
        let did = encode_did_key(&pk, CODEC_ED25519);
        // Should start with "did:key:z"
        let prefix = b"did:key:z";
        assert_eq!(&did.encoded[..DID_KEY_PREFIX_LEN], prefix);
        assert_eq!(did.public_key_bytes, pk);
        assert_eq!(did.codec, CODEC_ED25519);
    }

    // r[verify ucan-did.round-trip]
    // r[verify ucan-did.codec-preserved]
    #[test]
    fn test_did_key_decode_round_trip() {
        let pk = vec![7u8; ED25519_PUBLIC_KEY_LEN];
        let did = encode_did_key(&pk, CODEC_ED25519);
        let decoded = decode_did_key(&did.encoded).expect("encoded DID should decode");
        assert_eq!(decoded.public_key_bytes, pk);
        assert_eq!(decoded.codec, CODEC_ED25519);
        assert_eq!(decoded.encoded, did.encoded);
    }

    // r[verify ucan-did.prefix-valid]
    #[test]
    fn test_did_key_decode_rejects_bad_prefix() {
        let mut encoded = encode_did_key(&vec![9u8; ED25519_PUBLIC_KEY_LEN], CODEC_ED25519).encoded;
        encoded[0] ^= 0xff;
        assert!(decode_did_key(&encoded).is_none());
    }

    // r[verify ucan-did.base58btc-alphabet]
    #[test]
    fn test_base58btc_alphabet_validator_rejects_forbidden_chars() {
        assert!(base58btc_chars_valid(&b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz".to_vec()));
        for forbidden in *b"0OIl+" {
            assert!(!base58btc_chars_valid(&vec![forbidden]));
        }
    }

    // r[verify ucan-did.base58btc-alphabet]
    #[test]
    fn test_did_key_decode_rejects_invalid_base58_char() {
        let mut encoded = b"did:key:0".to_vec();
        assert!(decode_did_key(&encoded).is_none());
        encoded = b"did:key:z0".to_vec();
        assert!(decode_did_key(&encoded).is_none());
    }

    #[test]
    fn test_did_key_base58_round_trip() {
        const SAMPLE_KEY_BYTE: u8 = 42;
        let data = vec![SAMPLE_KEY_BYTE; ED25519_MULTICODEC_KEY_LEN]; // multicodec prefix + key
        let encoded = base58btc_encode(&data);
        let decoded = base58btc_decode(&encoded);
        assert!(decoded.is_some());
        assert_eq!(decoded.unwrap(), data);
    }

    #[test]
    fn test_did_key_distinct_keys() {
        let pk1 = vec![1u8; ED25519_PUBLIC_KEY_LEN];
        let pk2 = vec![2u8; ED25519_PUBLIC_KEY_LEN];
        let did1 = encode_did_key(&pk1, CODEC_ED25519);
        let did2 = encode_did_key(&pk2, CODEC_ED25519);
        assert_ne!(did1.encoded, did2.encoded);
    }
}
